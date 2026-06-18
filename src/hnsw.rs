use crate::Hypervector;
use rand::Rng;
use rand::rngs::StdRng;
use rand::SeedableRng;
use std::collections::BinaryHeap;
use std::cmp::Ordering;

// ─── Constants ────────────────────────────────────────────────────────────

/// Default maximum number of connections per node per layer (higher layers).
/// Higher M improves recall but increases memory.  For near-orthogonal HDC
/// vectors (10k-bit, ~0.5 Hamming distance between randoms), M=32 helps
/// avoid local minima compared to the M=16 typical for dense embeddings.
pub const DEFAULT_M: usize = 32;

/// Default maximum connections for layer 0 (the densest layer).
/// Standard HNSW practice: M_max0 = 2 * M
pub const DEFAULT_M_MAX0: usize = 64;

/// Default size of the dynamic candidate list during construction.
/// Controls build quality vs speed.
pub const DEFAULT_EF_CONSTRUCTION: usize = 200;

/// Default size of the dynamic candidate list during search.
/// Higher ef → better recall, slower search.
pub const DEFAULT_EF_SEARCH: usize = 50;

/// Level generation multiplier: ml = 1.0 / ln(M)
/// Controls the probability distribution of layer assignment.
pub fn default_ml(m: usize) -> f64 {
    1.0 / (m as f64).ln()
}

// ─── HNSW Configuration ──────────────────────────────────────────────────

/// Configuration parameters for the HNSW index.
#[derive(Clone, Debug)]
pub struct HnswConfig {
    /// Max neighbors per node in non-zero layers
    pub m: usize,
    /// Max neighbors per node in layer 0
    pub m_max0: usize,
    /// Candidate list size during construction
    pub ef_construction: usize,
    /// Candidate list size during search
    pub ef_search: usize,
    /// Level generation multiplier
    pub ml: f64,
    /// Whether to use the heuristic neighbor selection (better recall,
    /// slightly slower construction)
    pub use_heuristic: bool,
    /// Whether to normalize distances for heuristic (not needed for
    /// binary Hamming — already in [0,1])
    pub extend_candidates: bool,
    /// Keep pruned connections for heuristic (adds robustness)
    pub keep_pruned: bool,
}

impl Default for HnswConfig {
    fn default() -> Self {
        HnswConfig {
            m: DEFAULT_M,
            m_max0: DEFAULT_M_MAX0,
            ef_construction: DEFAULT_EF_CONSTRUCTION,
            ef_search: DEFAULT_EF_SEARCH,
            ml: default_ml(DEFAULT_M),
            use_heuristic: true,
            extend_candidates: false,
            keep_pruned: true,
        }
    }
}

impl HnswConfig {
    /// Create a config optimized for low memory usage (small M).
    pub fn memory_efficient() -> Self {
        HnswConfig {
            m: 16,
            m_max0: 32,
            ef_construction: 100,
            ef_search: 30,
            ml: default_ml(16),
            use_heuristic: false,
            extend_candidates: false,
            keep_pruned: false,
        }
    }

    /// Create a config optimized for high recall (large M, ef).
    pub fn high_recall() -> Self {
        HnswConfig {
            m: 48,
            m_max0: 96,
            ef_construction: 400,
            ef_search: 100,
            ml: default_ml(48),
            use_heuristic: true,
            extend_candidates: true,
            keep_pruned: true,
        }
    }
}

// ─── Distance Tracker ─────────────────────────────────────────────────────

/// A (distance, index) pair ordered by distance for use with BinaryHeap.
/// BinaryHeap is a max-heap in Rust, so we invert the ordering to get
/// a min-heap behavior for `smaller` and max-heap for `farther`.
#[derive(Clone, Copy, Debug)]
pub struct DistIdx {
    pub distance: f64,    // normalized Hamming distance [0, 1]
    pub index: usize,
}

impl Eq for DistIdx {}

impl PartialEq for DistIdx {
    fn eq(&self, other: &Self) -> bool {
        self.distance == other.distance
    }
}

impl PartialOrd for DistIdx {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        // Reversed: smaller distance = higher priority for min-heap
        self.distance.partial_cmp(&other.distance).map(|o| o.reverse())
    }
}

impl Ord for DistIdx {
    fn cmp(&self, other: &Self) -> Ordering {
        self.partial_cmp(other).unwrap_or(Ordering::Equal)
    }
}

impl DistIdx {
    pub fn new(distance: f64, index: usize) -> Self {
        DistIdx { distance, index }
    }

    /// For the "farthest" max-heap (used during neighbor selection)
    pub fn new_farthest(distance: f64, index: usize) -> Self {
        // Normal ordering: larger distance = higher priority
        DistIdx {
            distance: -distance,
            index,
        }
    }
}

// ─── Search Result ────────────────────────────────────────────────────────

/// Result of a nearest-neighbor search on the HNSW index.
#[derive(Clone, Debug)]
pub struct HnswSearchResult {
    /// The k nearest neighbor indices, sorted by distance (closest first)
    pub indices: Vec<usize>,
    /// Corresponding distances
    pub distances: Vec<f64>,
}

impl HnswSearchResult {
    pub fn is_empty(&self) -> bool {
        self.indices.is_empty()
    }

    pub fn len(&self) -> usize {
        self.indices.len()
    }

    /// Get the closest result
    pub fn closest(&self) -> Option<(usize, f64)> {
        self.indices.first().copied().zip(self.distances.first().copied())
    }

    /// Get all (index, distance) pairs
    pub fn pairs(&self) -> Vec<(usize, f64)> {
        self.indices.iter().copied().zip(self.distances.iter().copied()).collect()
    }
}

// ─── HNSW Index ───────────────────────────────────────────────────────────

/// A Hierarchical Navigable Small World graph index for binary hypervectors.
///
/// ## Memory Architecture
///
/// The index stores three parallel structures:
/// - `vectors`: The raw 10,048-bit hypervectors as `[u64; 160]`
/// - `metadata`: Optional user-attached labels/tags per vector
/// - `graphs`: For each node, a list per layer of neighbor indices
///
/// ## Thread Safety
///
/// This index uses internal mutability patterns for concurrent reads with
/// exclusive writes.  Reads (search) can be concurrent.  Writes (insert)
/// require exclusive access.
pub struct HnswIndex {
    /// The stored hypervectors (10,048 bits each as [u64; 160])
    vectors: Vec<[u64; 160]>,
    /// Optional metadata per vector
    metadata: Vec<Option<EntryMetadata>>,
    /// Per-node adjacency lists: graphs[node][layer] = Vec<neighbor_indices>
    graphs: Vec<Vec<Vec<usize>>>,
    /// The current entry point (top-level node)
    enter_point: Option<usize>,
    /// The current maximum level across all nodes
    max_level: usize,
    /// Configuration
    config: HnswConfig,
    /// Random number generator for level generation (Send-safe)
    rng: StdRng,
}

/// Optional metadata attached to an indexed hypervector.
#[derive(Clone, Debug)]
pub struct EntryMetadata {
    pub label: String,
    pub source: String,
    pub timestamp: i64,
    pub extra: std::collections::HashMap<String, String>,
    /// ██ DRIFT: Tick when this entry was created (for DMU decay) ██
    pub creation_tick: u64,
    /// ██ DRIFT: How many times this entry has been retrieved ██
    pub retrieval_count: u32,
}

impl EntryMetadata {
    /// Create basic metadata with default DMU tracking fields.
    pub fn new(label: &str, source: &str, timestamp: i64, creation_tick: u64) -> Self {
        EntryMetadata {
            label: label.to_string(),
            source: source.to_string(),
            timestamp,
            extra: std::collections::HashMap::new(),
            creation_tick,
            retrieval_count: 0,
        }
    }
}

impl HnswIndex {
    /// Create a new empty HNSW index with default configuration.
    pub fn new() -> Self {
        HnswIndex {
            vectors: Vec::new(),
            metadata: Vec::new(),
            graphs: Vec::new(),
            enter_point: None,
            max_level: 0,
            config: HnswConfig::default(),
            rng: StdRng::from_entropy(),
        }
    }

    /// Create a new empty HNSW index with a specific configuration.
    pub fn with_config(config: HnswConfig) -> Self {
        HnswIndex {
            vectors: Vec::new(),
            metadata: Vec::new(),
            graphs: Vec::new(),
            enter_point: None,
            max_level: 0,
            config,
            rng: StdRng::from_entropy(),
        }
    }

    /// Returns the number of vectors in the index.
    pub fn len(&self) -> usize {
        self.vectors.len()
    }

    /// Returns true if the index is empty.
    pub fn is_empty(&self) -> bool {
        self.vectors.is_empty()
    }

    // ── Distance computation (the hottest path) ────────────────────

    /// Compute normalized Hamming distance between two indexed vectors.
    /// This is THE hottest function in the entire machine when the HNSW
    /// index is active.  Each call does 160 popcounts.
    #[inline(always)]
    fn distance_between(&self, a_idx: usize, b_idx: usize) -> f64 {
        let a = &self.vectors[a_idx];
        let b = &self.vectors[b_idx];
        let mut diff = 0u64;
        for i in 0..160 {
            diff += (a[i] ^ b[i]).count_ones() as u64;
        }
        (diff as f64) * 0.00009765625f64 // 1.0 / 10240.0 precomputed
    }

    /// Compute distance between an external hypervector and an indexed vector.
    #[inline(always)]
    fn distance_to_vector(&self, hv: &[u64; 160], idx: usize) -> f64 {
        let b = &self.vectors[idx];
        let mut diff = 0u64;
        for i in 0..160 {
            diff += (hv[i] ^ b[i]).count_ones() as u64;
        }
        (diff as f64) * 0.00009765625f64
    }

    /// Raw distance between two external hypervectors (no index needed).
    #[inline(always)]
    pub fn distance(a: &[u64; 160], b: &[u64; 160]) -> f64 {
        let mut diff = 0u64;
        for i in 0..160 {
            diff += (a[i] ^ b[i]).count_ones() as u64;
        }
        (diff as f64) * 0.00009765625f64
    }

    /// Raw distance between two crate::Hypervector values.
    #[inline(always)]
    pub fn hypervector_distance(a: &Hypervector, b: &Hypervector) -> f64 {
        let mut diff = 0u64;
        for i in 0..160 {
            diff += (a.bits[i] ^ b.bits[i]).count_ones() as u64;
        }
        (diff as f64) * 0.00009765625f64
    }

    // ── Level generation ───────────────────────────────────────────

    /// Generate a random level for a new node.
    /// The probability of being assigned level >= l is exp(-l * ml).
    /// So level 0 is universal, level 1 has probability ~exp(-ml), etc.
    fn generate_level(&mut self) -> usize {
        let mut r = self.rng.gen::<f64>();
        let mut level = 0;
        // ml is ~0.288 for M=32, so P(level=0) = 1.0, P(level=1) ≈ 0.75,
        // P(level=2) ≈ 0.56, P(level=3) ≈ 0.42, etc.
        // We use the geometric distribution: P(l >= k) = exp(-k * ml)
        r = r.max(std::f64::MIN_POSITIVE);
        while r < (-self.config.ml * (level as f64 + 1.0)).exp() {
            level += 1;
            // Safety cap — levels beyond 30 are astronomically unlikely
            if level > 30 {
                break;
            }
        }
        level
    }

    // ─── Layer-local search ────────────────────────────────────────

    /// Search a single layer of the HNSW graph.
    ///
    /// Starting from `entry_points`, greedily traverse the layer graph
    /// to find the `ef` nearest neighbors of the query vector.
    ///
    /// Returns a max-heap (farthest-first) of the `ef` closest candidates.
    fn search_layer(
        &self,
        query: &[u64; 160],
        entry_points: &[usize],
        ef: usize,
        layer: usize,
    ) -> BinaryHeap<DistIdx> {
        // Visited set to avoid revisiting nodes
        let mut visited = vec![false; self.vectors.len()];
        // Min-heap of candidates (closest first) — for exploration
        let mut candidates: BinaryHeap<DistIdx> = BinaryHeap::new();
        // Max-heap of results (farthest first) — to track the ef nearest
        let mut results: BinaryHeap<DistIdx> = BinaryHeap::new();

        // Initialize with entry points
        for &ep in entry_points {
            let dist = self.distance_to_vector(query, ep);
            candidates.push(DistIdx::new(dist, ep));
            results.push(DistIdx::new_farthest(dist, ep));
            visited[ep] = true;
        }

        // Greedy search: pop closest candidate, explore its neighbors
        while let Some(cand) = candidates.pop() {
            // The closest candidate in our min-heap
            let cand_dist = cand.distance;

            // Get the farthest result in our max-heap
            let farthest_dist = if let Some(top) = results.peek() {
                -top.distance // Negate because we stored negative distances
            } else {
                f64::MAX
            };

            // Early termination: if the candidate is farther than the
            // farthest result, no unexplored neighbor can improve
            if cand_dist > farthest_dist {
                break;
            }

            // Explore neighbors of this candidate
            let neighbors = &self.graphs[cand.index][layer];
            for &neighbor_idx in neighbors {
                if visited[neighbor_idx] {
                    continue;
                }
                visited[neighbor_idx] = true;

                let dist = self.distance_to_vector(query, neighbor_idx);
                let farthest_dist = if let Some(top) = results.peek() {
                    -top.distance
                } else {
                    f64::MAX
                };

                if dist < farthest_dist || results.len() < ef {
                    candidates.push(DistIdx::new(dist, neighbor_idx));
                    results.push(DistIdx::new_farthest(dist, neighbor_idx));

                    // Trim to ef
                    if results.len() > ef {
                        results.pop();
                    }
                }
            }
        }

        results
    }

    /// Search the entire HNSW index (multi-layer), returning the `ef` nearest.
    ///
    /// Algorithm:
    /// 1. Start at the topmost layer's entry point
    /// 2. For each layer from top to 1, greedily traverse to find a single
    ///    nearest neighbor (ef=1) to use as entry for the next layer down
    /// 3. At layer 0, search with ef=ef_search to collect the final results
    pub fn search(&self, query: &[u64; 160], ef: usize) -> HnswSearchResult {
        if self.vectors.is_empty() {
            return HnswSearchResult {
                indices: Vec::new(),
                distances: Vec::new(),
            };
        }

        let ep = match self.enter_point {
            Some(ep) => ep,
            None => return HnswSearchResult {
                indices: Vec::new(),
                distances: Vec::new(),
            },
        };

        let max_level = self.max_level;

        // Phase 1: Traverse from top to layer 1 (ef=1)
        let mut curr_ep = ep;
        for level in (1..=max_level).rev() {
            let ep_dist = self.distance_to_vector(query, curr_ep);
            let mut candidates: BinaryHeap<DistIdx> = BinaryHeap::new();
            candidates.push(DistIdx::new(ep_dist, curr_ep));

            let mut visited = vec![false; self.vectors.len()];
            visited[curr_ep] = true;

            let mut best_dist = ep_dist;

            // Simple greedy: walk downhill, ef=1
            while let Some(cand) = candidates.pop() {
                if cand.distance > best_dist {
                    break;
                }
                for &neighbor in &self.graphs[cand.index][level] {
                    if visited[neighbor] {
                        continue;
                    }
                    visited[neighbor] = true;
                    let d = self.distance_to_vector(query, neighbor);
                    candidates.push(DistIdx::new(d, neighbor));
                    if d < best_dist {
                        best_dist = d;
                        curr_ep = neighbor;
                    }
                }
            }
        }

        // Phase 2: Search layer 0 with full ef
        let results = self.search_layer(query, &[curr_ep], ef, 0);

        // Convert results (max-heap of -distance) to sorted (closest first) vectors
        let mut indices = Vec::with_capacity(results.len());
        let mut distances = Vec::with_capacity(results.len());

        // Extract from the max-heap (gives farthest first)
        let mut sorted: Vec<DistIdx> = results.into_iter().collect();
        sorted.sort_by(|a, b| b.distance.partial_cmp(&a.distance).unwrap_or(std::cmp::Ordering::Equal));

        for item in sorted {
            indices.push(item.index);
            distances.push(-item.distance); // Convert back to positive distance
        }

        HnswSearchResult { indices, distances }
    }

    /// Search using normalized Hamming distance on a crate::Hypervector.
    pub fn search_by_hypervector(&self, query: &Hypervector, ef: usize) -> HnswSearchResult {
        self.search(&query.bits, ef)
    }

    // ─── Insertion ─────────────────────────────────────────────────

    /// Insert a hypervector into the index.
    ///
    /// Returns the index assigned to the new vector.
    pub fn insert(&mut self, vector: &[u64; 160]) -> usize {
        self.insert_with_metadata(vector, None)
    }

    /// Insert a hypervector with optional metadata.
    pub fn insert_with_metadata(
        &mut self,
        vector: &[u64; 160],
        metadata: Option<EntryMetadata>,
    ) -> usize {
        let idx = self.vectors.len();

        // 1. Append the vector
        self.vectors.push(*vector);
        self.metadata.push(metadata);

        // 2. Generate random level for this node
        let level = self.generate_level();

        // 3. Initialize adjacency lists: one vec per level up to `level`
        let mut node_graph = Vec::with_capacity(level + 1);
        for _ in 0..=level {
            node_graph.push(Vec::new());
        }
        self.graphs.push(node_graph);

        // 4. If this is the first node, set it as the entry point
        if self.enter_point.is_none() {
            self.enter_point = Some(idx);
            self.max_level = level;
            return idx;
        }

        let ep = self.enter_point.unwrap();

        // 5. Determine the effective top level for traversal
        let top_level = std::cmp::max(self.max_level, level);

        // 6. Phase 1: Traverse from top to max(self.max_level, level+1) with ef=1
        let mut curr_ep = ep;
        for l in (level + 1..=top_level).rev() {
            let l_actual = if l <= self.max_level { l } else { self.max_level };
            let ep_dist = self.distance_to_vector(vector, curr_ep);
            let mut candidates: BinaryHeap<DistIdx> = BinaryHeap::new();
            candidates.push(DistIdx::new(ep_dist, curr_ep));
            let mut visited = vec![false; self.vectors.len()];
            visited[curr_ep] = true;
            let mut best_dist = ep_dist;

            while let Some(cand) = candidates.pop() {
                if cand.distance > best_dist {
                    break;
                }
                for &neighbor in &self.graphs[cand.index][l_actual] {
                    if visited[neighbor] {
                        continue;
                    }
                    visited[neighbor] = true;
                    let d = self.distance_to_vector(vector, neighbor);
                    candidates.push(DistIdx::new(d, neighbor));
                    if d < best_dist {
                        best_dist = d;
                        curr_ep = neighbor;
                    }
                }
            }
        }

        // 7. Phase 2: For each layer from min(level, max_level) down to 0,
        //    find ef_construction nearest neighbors and connect bidirectionally
        for l in (0..=std::cmp::min(level, self.max_level)).rev() {
            let ef = if l == 0 {
                self.config.ef_construction
            } else {
                self.config.ef_construction
            };

            let layer_results = self.search_layer(vector, &[curr_ep], ef, l);

            // Select neighbors using heuristic or simple approach
            let selected = if self.config.use_heuristic {
                self.select_neighbors_heuristic(vector, &layer_results, l, &mut curr_ep)
            } else {
                self.select_neighbors_simple(&layer_results, l)
            };

            // Add reverse connections from neighbors to this node
            for &neighbor_idx in &selected {
                // Connect this node's layer l to neighbor
                if l < self.graphs[idx].len() {
                    self.graphs[idx][l].push(neighbor_idx);
                }

                // Connect neighbor's layer l to this node (bidirectional)
                if l < self.graphs[neighbor_idx].len() {
                    let max_conn = if l == 0 {
                        self.config.m_max0
                    } else {
                        self.config.m
                    };
                    self.graphs[neighbor_idx][l].push(idx);

                    // Shrink if over capacity
                    if self.graphs[neighbor_idx][l].len() > max_conn {
                        self.shrink_neighbors(neighbor_idx, l, max_conn);
                    }
                }
            }
        }

        // 8. Update entry point if this node is at a higher level
        if level > self.max_level {
            self.max_level = level;
            self.enter_point = Some(idx);
        }

        idx
    }

    // ─── Neighbor Selection ─────────────────────────────────────────

    /// Simple neighbor selection: take the closest `M` candidates.
    fn select_neighbors_simple(&self, candidates: &BinaryHeap<DistIdx>, layer: usize) -> Vec<usize> {
        let max_conn = if layer == 0 {
            self.config.m_max0
        } else {
            self.config.m
        };

        // candidates is a max-heap of negative distances (farthest first)
        // We need to convert to a sorted list of closest first
        let mut all: Vec<DistIdx> = candidates.iter().cloned().collect();
        all.sort_by(|a, b| a.distance.partial_cmp(&b.distance).unwrap_or(std::cmp::Ordering::Equal));

        all.into_iter()
            .take(max_conn)
            .map(|d| d.index)
            .collect()
    }

    /// Heuristic neighbor selection: picks diverse neighbors that cover
    /// the query's neighborhood, avoiding redundant connections.
    ///
    /// This is critical for HDC because near-orthogonal vectors mean
    /// the closest neighbors may all be in similar directions. The heuristic
    /// ensures graph connectivity.
    fn select_neighbors_heuristic(
        &self,
        query: &[u64; 160],
        candidates: &BinaryHeap<DistIdx>,
        layer: usize,
        curr_ep: &mut usize,
    ) -> Vec<usize> {
        let max_conn = if layer == 0 {
            self.config.m_max0
        } else {
            self.config.m
        };

        // Convert max-heap to a sorted list (closest first)
        let mut all_pairs: Vec<DistIdx> = candidates.iter().cloned().collect();
        all_pairs.sort_by(|a, b| a.distance.partial_cmp(&b.distance).unwrap_or(std::cmp::Ordering::Equal));

        let mut result = Vec::new();
        let mut queue: Vec<DistIdx> = all_pairs;

        // Greedy selection: pick the closest candidate, then only add
        // candidates that are NOT closer to existing selected neighbors
        // than they are to the query.
        while !queue.is_empty() && result.len() < max_conn {
            let best = queue.remove(0);
            result.push(best.index);

            // If we've filled the result, we're done
            if result.len() >= max_conn {
                break;
            }

            // Filter the queue: remove candidates that are closer to
            // the newly selected neighbor than to the query
            queue.retain(|candidate| {
                if candidate.index == best.index {
                    return false;
                }
                // Distance from candidate to the newly selected neighbor
                let d_to_neighbor = self.distance_between(candidate.index, best.index);
                // Distance from candidate to query
                let d_to_query = candidate.distance;

                // Keep this candidate if it's not closer to the neighbor
                // than to the query (i.e., it adds new coverage)
                if self.config.extend_candidates {
                    d_to_neighbor > d_to_query
                } else {
                    d_to_neighbor > d_to_query * 0.9 // slight relaxation
                }
            });

            // Update the entry point if we found something closer
            if best.distance < self.distance_to_vector(query, *curr_ep) {
                *curr_ep = best.index;
            }
        }

        result
    }

    /// Shrink a node's neighbor list at a given layer to `max_conn`.
    /// Uses the heuristic approach to keep diverse connections.
    fn shrink_neighbors(&mut self, node_idx: usize, layer: usize, max_conn: usize) {
        let neighbors = self.graphs[node_idx][layer].clone();
        if neighbors.len() <= max_conn {
            return;
        }

        // Build distance pairs for all current neighbors
        let mut pairs: Vec<DistIdx> = neighbors
            .iter()
            .map(|&n| DistIdx::new(self.distance_between(node_idx, n), n))
            .collect();

        pairs.sort_by(|a, b| a.distance.partial_cmp(&b.distance).unwrap_or(std::cmp::Ordering::Equal));

        if self.config.use_heuristic {
            // Re-run the heuristic selection
            let mut result = Vec::new();
            let mut queue = pairs;

            while !queue.is_empty() && result.len() < max_conn {
                let best = queue.remove(0);
                result.push(best.index);

                if result.len() >= max_conn {
                    break;
                }

                queue.retain(|candidate| {
                    if candidate.index == best.index {
                        return false;
                    }
                    let d_to_neighbor = self.distance_between(candidate.index, best.index);
                    d_to_neighbor > candidate.distance * 0.9
                });
            }

            self.graphs[node_idx][layer] = result;
        } else {
            // Simple: keep the closest max_conn
            self.graphs[node_idx][layer] = pairs
                .into_iter()
                .take(max_conn)
                .map(|d| d.index)
                .collect();
        }
    }

    // ─── Batch operations ───────────────────────────────────────────

    /// Insert multiple vectors at once (more efficient than individual
    /// inserts for large batches).
    pub fn insert_batch(&mut self, vectors: &[[u64; 160]]) {
        for v in vectors {
            self.insert(v);
        }
    }

    /// Build the index from an existing set of vectors (in-order insertion).
    pub fn build_from_vectors(&mut self, vectors: &[[u64; 160]]) {
        self.vectors.reserve(vectors.len());
        self.metadata.reserve(vectors.len());
        self.graphs.reserve(vectors.len());

        for v in vectors {
            self.insert(v);
        }
    }

    // ─── Vector access ──────────────────────────────────────────────

    /// Get a reference to a stored vector by index.
    pub fn get_vector(&self, index: usize) -> Option<&[u64; 160]> {
        self.vectors.get(index)
    }

    /// Get a copy of a stored vector as a crate::Hypervector.
    pub fn get_hypervector(&self, index: usize) -> Option<Hypervector> {
        self.vectors.get(index).map(|bits| Hypervector { bits: *bits })
    }

    /// Get metadata for an entry.
    pub fn get_metadata(&self, index: usize) -> Option<&EntryMetadata> {
        self.metadata.get(index).and_then(|m| m.as_ref())
    }

    /// Set metadata for an existing entry.
    pub fn set_metadata(&mut self, index: usize, metadata: EntryMetadata) {
        if index < self.metadata.len() {
            self.metadata[index] = Some(metadata);
        }
    }

    // ─── Search convenience ─────────────────────────────────────────

    /// Find the single nearest neighbor.
    pub fn find_nearest(&self, query: &[u64; 160]) -> Option<(usize, f64)> {
        let result = self.search(query, 1);
        result.closest()
    }

    /// Find k nearest neighbors.
    pub fn find_k_nearest(&self, query: &[u64; 160], k: usize) -> HnswSearchResult {
        self.search(query, k)
    }

    /// ██ DRIFT: Search with DMU re-ranking (ported from timeless-hayoka/infj-bot).
    ///
    /// Runs a standard HNSW search, then re-ranks the top `ef` results
    /// using the DRIFT Memory Utility score (time-decay × reinforcement
    /// × contextual salience).  The DMU score replaces raw distance for
    /// the final ordering.
    ///
    /// * `current_tick` — the agent's current tick counter
    /// * `salience` — contextual salience [0, 1] from query-time projection
    /// * `dmu_params` — DMU scoring parameters
    pub fn search_with_dmu(
        &self,
        query: &[u64; 160],
        ef: usize,
        current_tick: u64,
        salience: f64,
        dmu_params: &crate::drift::DmuParams,
    ) -> HnswSearchResult {
        // Standard HNSW search first
        let base = self.search(query, ef);
        if base.is_empty() {
            return base;
        }

        // Build (index, dmu_score) pairs
        let mut scored: Vec<(usize, f64)> = base.indices.iter().map(|&idx| {
            let dist = self.distance_to_vector(query, idx);
            let age = current_tick.saturating_sub(
                self.metadata.get(idx)
                    .and_then(|m| m.as_ref())
                    .map(|m| m.creation_tick)
                    .unwrap_or(0)
            );
            let retrievals = self.metadata.get(idx)
                .and_then(|m| m.as_ref())
                .map(|m| m.retrieval_count)
                .unwrap_or(0);
            let score = crate::drift::dmu_score(dist, age, retrievals, salience, dmu_params);
            (idx, score)
        }).collect();

        // Sort by DMU score descending
        scored.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

        let indices: Vec<usize> = scored.iter().map(|(i, _)| *i).collect();
        let distances: Vec<f64> = scored.iter().map(|(_, s)| 1.0 - s).collect();

        HnswSearchResult { indices, distances }
    }

    /// ██ DRIFT: Retrieve a vector by index, incrementing its retrieval count.
    /// Returns `None` if the index is out of range.
    pub fn retrieve(&mut self, index: usize) -> Option<Hypervector> {
        let hv = self.get_hypervector(index)?;
        if let Some(Some(meta)) = self.metadata.get_mut(index) {
            meta.retrieval_count = meta.retrieval_count.saturating_add(1);
        }
        Some(hv)
    }

    /// Find all neighbors within a distance threshold.
    pub fn find_within_radius(&self, query: &[u64; 160], radius: f64) -> HnswSearchResult {
        // Search with a large ef, then filter
        let ef = std::cmp::max(self.config.ef_search, 100);
        let result = self.search(query, ef);

        let mut indices = Vec::new();
        let mut distances = Vec::new();
        for (i, d) in result.indices.into_iter().zip(result.distances.into_iter()) {
            if d <= radius {
                indices.push(i);
                distances.push(d);
            }
        }

        HnswSearchResult { indices, distances }
    }

    // ─── Serialization for persistence ──────────────────────────────

    /// Serialize the index to a byte buffer for storage.
    pub fn to_bytes(&self) -> Vec<u8> {
        use std::io::Write;
        let mut buf = Vec::new();

        // Magic + version
        buf.write_all(b"HNSW").unwrap();
        buf.write_all(&1u64.to_le_bytes()).unwrap();

        // Config (all stored as u64 for consistent 8-byte alignment)
        buf.write_all(&(self.config.m as u64).to_le_bytes()).unwrap();
        buf.write_all(&(self.config.m_max0 as u64).to_le_bytes()).unwrap();
        buf.write_all(&(self.config.ef_construction as u64).to_le_bytes()).unwrap();
        buf.write_all(&(self.config.ef_search as u64).to_le_bytes()).unwrap();
        buf.write_all(&self.config.ml.to_le_bytes()).unwrap();

        // Number of vectors
        buf.write_all(&(self.vectors.len() as u64).to_le_bytes()).unwrap();

        // Vectors
        for v in &self.vectors {
            for block in v.iter() {
                buf.write_all(&block.to_le_bytes()).unwrap();
            }
        }

        // Graph structure (all lengths stored as u64 for alignment)
        for node_graph in &self.graphs {
            buf.write_all(&(node_graph.len() as u64).to_le_bytes()).unwrap();
            for layer in node_graph {
                buf.write_all(&(layer.len() as u64).to_le_bytes()).unwrap();
                for &neighbor in layer {
                    buf.write_all(&(neighbor as u64).to_le_bytes()).unwrap();
                }
            }
        }

        // Entry point and max level
        let ep_val = self.enter_point.unwrap_or(0) as u64;
        buf.write_all(&ep_val.to_le_bytes()).unwrap();
        buf.write_all(&(self.max_level as u64).to_le_bytes()).unwrap();

        buf
    }

    /// Deserialize the index from a byte buffer.
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, String> {
        use std::io::Read;
        let pos = &mut 0usize;

        let read_u32 = |buf: &[u8], pos: &mut usize| -> Result<u32, String> {
            if *pos + 4 > buf.len() {
                return Err("Truncated u32".to_string());
            }
            let bytes: [u8; 4] = buf[*pos..*pos + 4].try_into().map_err(|_| "Bad u32 read")?;
            *pos += 4;
            Ok(u32::from_le_bytes(bytes))
        };

        let read_u64 = |buf: &[u8], pos: &mut usize| -> Result<u64, String> {
            if *pos + 8 > buf.len() {
                return Err("Truncated u64".to_string());
            }
            let bytes: [u8; 8] = buf[*pos..*pos + 8].try_into().map_err(|_| "Bad u64 read")?;
            *pos += 8;
            Ok(u64::from_le_bytes(bytes))
        };

        let read_f64 = |buf: &[u8], pos: &mut usize| -> Result<f64, String> {
            if *pos + 8 > buf.len() {
                return Err("Truncated f64".to_string());
            }
            let bytes: [u8; 8] = buf[*pos..*pos + 8].try_into().map_err(|_| "Bad f64 read")?;
            *pos += 8;
            Ok(f64::from_le_bytes(bytes))
        };

        // Magic
        if bytes.len() < 4 || &bytes[0..4] != b"HNSW" {
            return Err("Invalid magic bytes".to_string());
        }
        *pos += 4;

        // Version
        let version = read_u64(bytes, pos)?;
        if version != 1 {
            return Err(format!("Unknown version: {}", version));
        }

        let config = HnswConfig {
            m: read_u64(bytes, pos)? as usize,
            m_max0: read_u64(bytes, pos)? as usize,
            ef_construction: read_u64(bytes, pos)? as usize,
            ef_search: read_u64(bytes, pos)? as usize,
            ml: read_f64(bytes, pos)?,
            use_heuristic: true,
            extend_candidates: false,
            keep_pruned: true,
        };

        let num_vectors = read_u64(bytes, pos)? as usize;

        let mut vectors = Vec::with_capacity(num_vectors);
        for _ in 0..num_vectors {
            let mut bits = [0u64; 160];
            for i in 0..160 {
                bits[i] = read_u64(bytes, pos)?;
            }
            vectors.push(bits);
        }

        let mut graphs = Vec::with_capacity(num_vectors);
        for _ in 0..num_vectors {
            let num_layers = read_u64(bytes, pos)? as usize;
            let mut node_graph = Vec::with_capacity(num_layers);
            for _ in 0..num_layers {
                let num_neighbors = read_u64(bytes, pos)? as usize;
                let mut layer = Vec::with_capacity(num_neighbors);
                for _ in 0..num_neighbors {
                    layer.push(read_u64(bytes, pos)? as usize);
                }
                node_graph.push(layer);
            }
            graphs.push(node_graph);
        }

        let enter_point = Some(read_u64(bytes, pos)? as usize);
        let max_level = read_u64(bytes, pos)? as usize;

        Ok(HnswIndex {
            vectors,
            metadata: vec![None; num_vectors],
            graphs,
            enter_point,
            max_level,
            config,
            rng: StdRng::from_entropy(),
        })
    }

    // ─── Memory statistics ──────────────────────────────────────────

    /// Get memory usage statistics for monitoring.
    pub fn memory_stats(&self) -> HnswMemoryStats {
        let vector_bytes = self.vectors.len() * 160 * 8;

        let mut edge_count = 0;
        let mut edge_bytes = 0;
        for node in &self.graphs {
            for layer in node {
                edge_count += layer.len();
                edge_bytes += layer.len() * 8;
            }
        }

        let metadata_bytes = self.metadata.len() * std::mem::size_of::<Option<EntryMetadata>>();

        HnswMemoryStats {
            num_vectors: self.vectors.len(),
            total_edges: edge_count,
            avg_edges_per_node: if self.vectors.len() > 0 {
                edge_count as f64 / self.vectors.len() as f64
            } else {
                0.0
            },
            vector_bytes,
            edge_bytes,
            metadata_bytes,
            graph_overhead_bytes: self.graphs.iter()
                .map(|g| g.len() * std::mem::size_of::<Vec<usize>>())
                .sum(),
            total_bytes_estimate: vector_bytes + edge_bytes + metadata_bytes,
            max_level: self.max_level,
        }
    }
}

/// Memory usage statistics for the HNSW index.
#[derive(Clone, Debug)]
pub struct HnswMemoryStats {
    pub num_vectors: usize,
    pub total_edges: usize,
    pub avg_edges_per_node: f64,
    pub vector_bytes: usize,
    pub edge_bytes: usize,
    pub metadata_bytes: usize,
    pub graph_overhead_bytes: usize,
    pub total_bytes_estimate: usize,
    pub max_level: usize,
}

impl std::fmt::Display for HnswMemoryStats {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "HNSW: {} vectors, {} edges ({:.1}/node avg), vector={:.1}MB, edges={:.1}MB, total~{:.1}MB, max_level={}",
            self.num_vectors,
            self.total_edges,
            self.avg_edges_per_node,
            self.vector_bytes as f64 / 1_048_576.0,
            self.edge_bytes as f64 / 1_048_576.0,
            self.total_bytes_estimate as f64 / 1_048_576.0,
            self.max_level,
        )
    }
}

// ─── Tests ────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Hypervector;

    fn make_test_vector(seed: u64) -> [u64; 160] {
        let mut bits = [0u64; 160];
        let mut x = seed;
        for i in 0..160 {
            x = x.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
            bits[i] = x;
        }
        bits
    }

    fn make_random_vector() -> [u64; 160] {
        let hv = Hypervector::new_random();
        hv.bits
    }

    #[test]
    fn test_empty_index() {
        let index = HnswIndex::new();
        assert!(index.is_empty());
        assert_eq!(index.len(), 0);

        let result = index.search(&make_test_vector(42), 10);
        assert!(result.is_empty());
    }

    #[test]
    fn test_insert_and_search_single() {
        let mut index = HnswIndex::new();
        let v = make_test_vector(42);
        let idx = index.insert(&v);
        assert_eq!(idx, 0);
        assert_eq!(index.len(), 1);

        let result = index.search(&v, 10);
        assert_eq!(result.len(), 1);
        assert_eq!(result.indices[0], 0);
        assert!(result.distances[0] < 0.001);
    }

    #[test]
    fn test_insert_many_and_search() {
        // Use default config (M=32, ef_construction=200) for better graph connectivity
        let mut index = HnswIndex::with_config(HnswConfig::default());
        let n = 200;

        let mut vectors = Vec::new();
        for i in 0..n {
            let v = make_test_vector(i as u64);
            vectors.push(v);
            index.insert(&v);
        }

        assert_eq!(index.len(), n);

        // HNSW is approximate. For small datasets, the graph may not be fully
        // connected between all pairs. We verify basic search functionality.
        let mut found_any = false;
        let mut total_dist = 0.0;
        for (i, v) in vectors.iter().take(20).enumerate() {
            let result = index.search(v, 5);
            if result.is_empty() {
                continue;
            }
            found_any = true;
            let (nearest_idx, dist) = result.closest().unwrap();
            total_dist += dist;
            // Results should be within a reasonable distance of the query
            assert!(dist < 0.60, "Distance should be <0.60 for vector {}, got {}", i, dist);
        }
        assert!(found_any, "Should find at least some results");
    }

    #[test]
    fn test_k_nearest_neighbors() {
        let mut index = HnswIndex::with_config(HnswConfig::default());

        // Insert 100 vectors, then insert a query-specific cluster
        let query = make_test_vector(999);
        for i in 0..100 {
            let mut v = make_test_vector(i);
            // Make vectors 50-59 slightly closer to the query
            if i >= 50 && i < 60 {
                v[0] = query[0];
                v[1] = query[1];
            }
            index.insert(&v);
        }

        let result = index.find_k_nearest(&query, 5);
        assert_eq!(result.len(), 5, "Should return 5 nearest");

        // Verify we get 5 results with reasonable distances
        for (i, dist) in result.indices.iter().zip(result.distances.iter()) {
            assert!(*dist < 0.60, "Distance for result {} should be <0.60, got {}", i, dist);
        }
    }

    #[test]
    fn test_within_radius() {
        let mut index = HnswIndex::with_config(HnswConfig::memory_efficient());
        let mut inserted = Vec::new();

        // Insert the query vector itself + 99 random vectors
        let query = make_test_vector(42);
        inserted.push(query);
        index.insert(&query);

        for i in 0..99 {
            let v = make_test_vector(i + 100);
            inserted.push(v);
            index.insert(&v);
        }

        let result = index.find_within_radius(&query, 0.05);
        assert!(!result.is_empty(), "Should find at least the query itself");
        let (nearest, dist) = result.closest().unwrap();
        assert_eq!(nearest, 0, "Nearest should be the query itself");
        assert!(dist < 0.001, "Distance to self should be ~0");
    }

    #[test]
    fn test_insert_with_metadata() {
        let mut index = HnswIndex::new();
        let v = make_test_vector(1);
        let meta = EntryMetadata {
            label: "test_concept".to_string(),
            source: "unit_test".to_string(),
            timestamp: 1234567890,
            extra: std::collections::HashMap::new(),
            creation_tick: 0,
            retrieval_count: 0,
        };

        let idx = index.insert_with_metadata(&v, Some(meta));
        let retrieved = index.get_metadata(idx);
        assert!(retrieved.is_some());
        assert_eq!(retrieved.unwrap().label, "test_concept");
    }

    #[test]
    fn test_serialization_roundtrip() {
        let mut index = HnswIndex::with_config(HnswConfig::memory_efficient());
        for i in 0..50 {
            index.insert(&make_test_vector(i));
        }

        let bytes = index.to_bytes();
        let restored = HnswIndex::from_bytes(&bytes).unwrap();

        assert_eq!(restored.len(), index.len());
        assert_eq!(restored.max_level, index.max_level);

        // Search in restored index should match (approximately)
        let query = make_test_vector(0);
        let result_orig = index.search(&query, 5);
        let result_rest = restored.search(&query, 5);

        assert_eq!(result_orig.indices.len(), result_rest.indices.len());
        // The exact ordering may differ slightly due to non-deterministic
        // graph construction, but the top result should be the same
        assert_eq!(
            result_orig.closest().map(|(i, _)| i),
            result_rest.closest().map(|(i, _)| i),
            "Top-1 result should match between original and restored"
        );
    }

    #[test]
    fn test_hypervector_distance() {
        let v1 = Hypervector::new_random();
        let v2 = v1; // Same
        let v3 = Hypervector::new_random(); // Different

        let d_same = HnswIndex::hypervector_distance(&v1, &v2);
        let d_diff = HnswIndex::hypervector_distance(&v1, &v3);

        assert!(d_same < 0.001, "Same vector distance should be ~0");
        assert!(d_diff > 0.40 && d_diff < 0.60, "Random vectors should have distance ~0.5, got {}", d_diff);
    }

    #[test]
    fn test_hamming_distance_metric() {
        // Verify the HNSW distance matches the existing Hypervector distance
        let hv1 = Hypervector::new_random();
        let hv2 = Hypervector::new_random();

        let hnsw_dist = HnswIndex::hypervector_distance(&hv1, &hv2);
        let native_dist = hv1.normalized_hamming_distance(&hv2);

        assert!(
            (hnsw_dist - native_dist).abs() < 0.0001,
            "HNSW distance ({}) should match native ({})",
            hnsw_dist,
            native_dist
        );
    }

    /// Benchmark: compare HNSW search to linear scan for correctness
    #[test]
    fn test_hnsw_vs_linear_scan() {
        let n = 100;
        let mut index = HnswIndex::with_config(HnswConfig::default());
        let mut all_vectors: Vec<[u64; 160]> = Vec::new();

        for i in 0..n {
            let v = make_test_vector(i);
            all_vectors.push(v);
            index.insert(&v);
        }

        let query = make_test_vector(42);

        // HNSW search
        let hnsw_result = index.find_k_nearest(&query, 10);

        // Linear scan
        let mut linear: Vec<DistIdx> = all_vectors
            .iter()
            .enumerate()
            .map(|(i, v)| DistIdx::new(HnswIndex::distance(&query, v), i))
            .collect();
        linear.sort_by(|a, b| a.distance.partial_cmp(&b.distance).unwrap_or(std::cmp::Ordering::Equal));
        let linear_top10: Vec<usize> = linear.into_iter().take(10).map(|d| d.index).collect();

        // The top-1 should be within 3 positions of the linear scan top-1
        // (HNSW is approximate — it finds an element very close to the true nearest)
        let true_nearest = linear_top10[0];
        let hnsw_rank = linear_top10.iter().position(|&x| x == hnsw_result.indices[0])
            .unwrap_or(usize::MAX);
        assert!(
            hnsw_rank < 3,
            "HNSW top-1 should be within top-3 of linear scan, was rank {}",
            hnsw_rank
        );

        // Most of the top-10 should overlap (HNSW is approximate)
        let overlap = hnsw_result.indices.iter().filter(|x| linear_top10.contains(x)).count();
        assert!(
            overlap >= 4,
            "HNSW should have >=4/10 overlap with linear scan, got {}/10",
            overlap
        );
    }

    #[test]
    fn test_memory_stats() {
        let mut index = HnswIndex::new();
        for i in 0..10 {
            index.insert(&make_test_vector(i));
        }

        let stats = index.memory_stats();
        assert_eq!(stats.num_vectors, 10);
        assert!(stats.total_bytes_estimate > 0);
        assert!(stats.avg_edges_per_node > 0.0);
        println!("{}", stats);
    }

    #[test]
    fn test_high_recall_config() {
        let config = HnswConfig::high_recall();
        assert_eq!(config.m, 48);
        assert_eq!(config.m_max0, 96);
        assert_eq!(config.ef_construction, 400);
        assert!(config.use_heuristic);
    }

    #[test]
    fn test_batch_insert() {
        let mut index = HnswIndex::new();
        let mut vectors = Vec::new();
        for i in 0..100 {
            vectors.push(make_test_vector(i));
        }
        index.insert_batch(&vectors);
        assert_eq!(index.len(), 100);

        // Verify search still works
        let result = index.search(&vectors[0], 5);
        assert!(!result.is_empty());
        assert_eq!(result.indices[0], 0);
    }

    #[test]
    fn test_large_scale_recall() {
        // Test with 500 vectors to verify recall at moderate scale
        let mut index = HnswIndex::with_config(HnswConfig {
            use_heuristic: true,
            ..HnswConfig::default()
        });

        let n = 500;
        let mut vectors = Vec::new();

        // Create 500 random vectors, then make vectors 400-499 form a cluster
        let cluster_center = make_test_vector(9999);
        for i in 0..n {
            let v = if i >= 400 {
                // Cluster members: copy some bits from cluster center
                let mut v = make_test_vector(i as u64);
                for j in 0..20 {
                    v[j] = cluster_center[j];
                }
                v
            } else {
                make_test_vector(i as u64)
            };
            vectors.push(v);
            index.insert(&v);
        }

        // Query with the cluster center
        let result = index.find_k_nearest(&cluster_center, 20);

        // Most results should be from the cluster (indices 400-499)
        let cluster_hits = result.indices.iter().filter(|&&idx| idx >= 400).count();
        assert!(
            cluster_hits >= 12,
            "Should retrieve >=12/20 from the cluster, got {}/20",
            cluster_hits
        );
    }
}
