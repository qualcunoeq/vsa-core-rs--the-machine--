use rand::Rng;
use std::collections::HashMap;

use crate::hnsw::HnswIndex;

pub mod action;
pub mod autonomy;
pub mod broker;
pub mod defense;
pub mod forager;
pub mod fpe;
pub mod graph;
pub mod hnsw;
pub mod ledger;
pub mod observer;
pub mod planning;
pub mod resonator;
pub mod sensory;
pub mod socket;

pub const HD_DIMENSION: usize = 10048;
pub const U64_BLOCKS: usize = 157;

impl Hypervector {
    pub fn role_market() -> Self {
        Self::encode_text_ngram("ROLE_MARKET_STATE", 3)
    }

    pub fn role_news() -> Self {
        Self::encode_text_ngram("ROLE_NEWS_STATE", 3)
    }

    pub fn role_infra() -> Self {
        Self::encode_text_ngram("ROLE_INFRA_STATE", 3)
    }
}

mod array_u64_157 {
    use serde::{Deserialize, Deserializer, Serialize, Serializer};

    pub fn serialize<S>(array: &[u64; 157], serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let vec: Vec<u64> = array.iter().cloned().collect();
        vec.serialize(serializer)
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<[u64; 157], D::Error>
    where
        D: Deserializer<'de>,
    {
        let vec = Vec::<u64>::deserialize(deserializer)?;
        if vec.len() != 157 {
            return Err(serde::de::Error::custom(format!(
                "Expected array of size 157, found {}",
                vec.len()
            )));
        }
        let mut array = [0u64; 157];
        array.copy_from_slice(&vec);
        Ok(array)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub struct Hypervector {
    #[serde(with = "array_u64_157")]
    pub bits: [u64; U64_BLOCKS],
}

impl Hypervector {
    pub fn new_random() -> Self {
        let mut rng = rand::thread_rng();
        let mut bits = [0u64; U64_BLOCKS];
        for b in bits.iter_mut() {
            *b = rng.gen();
        }
        Hypervector { bits }
    }

    pub fn new_zero() -> Self {
        Hypervector {
            bits: [0u64; U64_BLOCKS],
        }
    }

    pub fn count_ones(&self) -> usize {
        self.bits.iter().map(|b| b.count_ones() as usize).sum()
    }

    pub fn bitwise_xor(&self, other: &Self) -> Self {
        let mut result = [0u64; U64_BLOCKS];
        for i in 0..U64_BLOCKS {
            result[i] = self.bits[i] ^ other.bits[i];
        }
        Hypervector { bits: result }
    }

    pub fn normalized_hamming_distance(&self, other: &Self) -> f64 {
        let mut diff_count = 0;
        for i in 0..U64_BLOCKS {
            let xor_val = self.bits[i] ^ other.bits[i];
            diff_count += xor_val.count_ones(); // Native CPU popcount
        }
        (diff_count as f64) / (HD_DIMENSION as f64)
    }

    pub fn rotate_left(&self, shift: usize) -> Self {
        let shift = shift % HD_DIMENSION;
        if shift == 0 {
            return *self;
        }
        let word_shift = shift / 64;
        let bit_shift = shift % 64;

        let mut result = [0u64; U64_BLOCKS];
        for i in 0..U64_BLOCKS {
            let src_idx1 = (i + word_shift) % U64_BLOCKS;
            let src_idx2 = (i + word_shift + 1) % U64_BLOCKS;

            if bit_shift == 0 {
                result[i] = self.bits[src_idx1];
            } else {
                let part1 = self.bits[src_idx1] >> bit_shift;
                let part2 = self.bits[src_idx2] << (64 - bit_shift);
                result[i] = part1 | part2;
            }
        }
        Hypervector { bits: result }
    }

    /// Proprietary Chaotic Shift-XOR Character Encoder
    pub fn encode_char(c: char, index_seed: usize) -> Self {
        let mut hv = [0u64; U64_BLOCKS];
        let char_val = c as u64;

        for i in 0..U64_BLOCKS {
            // A proprietary LCG/Xorshift cascade to generate pseudo-random bits
            let mut x = char_val
                .wrapping_add(i as u64)
                .wrapping_mul(0x9E3779B97F4A7C15);
            x ^= x >> 30;
            x = x.wrapping_mul(0xBF58476D1CE4E5B9);
            x ^= x >> 27;
            hv[i] = x ^ (index_seed as u64);
        }
        Hypervector { bits: hv }
    }

    /// Sequence-Preserving N-Gram Permutation with Page-Level Majority Bundling
    pub fn encode_text_ngram(text: &str, k: usize) -> Self {
        if text.is_empty() {
            return Self::new_zero();
        }

        let chars: Vec<char> = text.chars().collect();
        if chars.len() < k {
            let mut ngram_vec = Self::new_zero();
            for (i, &c) in chars.iter().enumerate() {
                let char_vec = Self::encode_char(c, i);
                let rotated = char_vec.rotate_left(i);
                if i == 0 {
                    ngram_vec = rotated;
                } else {
                    ngram_vec = ngram_vec.bitwise_xor(&rotated);
                }
            }
            return ngram_vec;
        }

        let mut ngrams = Vec::new();
        for i in 0..=(chars.len() - k) {
            let mut ngram_vec = Self::new_zero();
            for j in 0..k {
                let char_vec = Self::encode_char(chars[i + j], j);
                let rotated = char_vec.rotate_left(j);
                if j == 0 {
                    ngram_vec = rotated;
                } else {
                    ngram_vec = ngram_vec.bitwise_xor(&rotated);
                }
            }
            ngrams.push(ngram_vec);
        }

        let refs: Vec<&Hypervector> = ngrams.iter().collect();
        Self::bundle(&refs)
    }

    /// Bit-Parallel Majority Bundling with deterministic noise injection for tie-breaking
    pub fn bundle(vectors: &[&Self]) -> Self {
        if vectors.is_empty() {
            return Self::new_zero();
        }
        if vectors.len() == 1 {
            return *vectors[0];
        }

        let mut result_bits = [0u64; U64_BLOCKS];
        let num_vectors = vectors.len();
        let halfway = num_vectors / 2;
        let is_even = num_vectors % 2 == 0;

        let noise_vector = if is_even {
            // A deterministic pseudo-random vector derived from the input vectors
            // to ensure reproducible bundling behavior across restarts and trials.
            let mut bits = [0u64; U64_BLOCKS];
            let first_bits = vectors.first().map(|v| v.bits[0]).unwrap_or(0);
            for i in 0..U64_BLOCKS {
                let mut x = first_bits
                    .wrapping_add(i as u64)
                    .wrapping_mul(0x9E3779B97F4A7C15);
                x ^= x >> 30;
                x = x.wrapping_mul(0xBF58476D1CE4E5B9);
                x ^= x >> 27;
                bits[i] = x;
            }
            Hypervector { bits }
        } else {
            Self::new_zero()
        };

        for block_idx in 0..U64_BLOCKS {
            let mut block_consensus = 0u64;
            for bit_idx in 0..64 {
                let mut bit_count = 0;
                for vec in vectors {
                    if ((vec.bits[block_idx] >> bit_idx) & 1) == 1 {
                        bit_count += 1;
                    }
                }

                if is_even && bit_count == halfway {
                    let noise_bit = ((noise_vector.bits[block_idx] >> bit_idx) & 1) == 1;
                    if noise_bit {
                        block_consensus |= 1 << bit_idx;
                    }
                } else if bit_count > halfway {
                    block_consensus |= 1 << bit_idx;
                }
            }
            result_bits[block_idx] = block_consensus;
        }
        Hypervector { bits: result_bits }
    }

    /// Continuous Value Interpolation Mapping
    pub fn encode_continuous(config: &VarConfig, val: f64) -> Self {
        let clamped = val.clamp(config.min_val, config.max_val);
        let fraction = (clamped - config.min_val) / (config.max_val - config.min_val);
        let num_bits_max = (fraction * (HD_DIMENSION as f64)).round() as usize;
        let num_bits_min = HD_DIMENSION - num_bits_max;

        let mut result = [0u64; U64_BLOCKS];
        for i in 0..U64_BLOCKS {
            let start_bit = i * 64;
            let end_bit = start_bit + 64;

            if end_bit <= num_bits_min {
                result[i] = config.base_min.bits[i];
            } else if start_bit >= num_bits_min {
                result[i] = config.base_max.bits[i];
            } else {
                let split = num_bits_min - start_bit;
                let mask_min = (1u64 << split) - 1;
                let part_min = config.base_min.bits[i] & mask_min;
                let part_max = (config.base_max.bits[i] >> split) << split;
                result[i] = part_min | part_max;
            }
        }
        Hypervector { bits: result }
    }

    /// Convert Hypervector to a raw 1256-byte buffer (using 157 * 8 bytes = 1256 bytes)
    pub fn to_bytes(&self) -> [u8; 1256] {
        let mut bytes = [0u8; 1256];
        for i in 0..157 {
            let block_bytes = self.bits[i].to_le_bytes();
            bytes[i * 8..(i + 1) * 8].copy_from_slice(&block_bytes);
        }
        bytes
    }

    /// Parse Hypervector from a raw 1256-byte buffer
    pub fn from_bytes(bytes: &[u8; 1256]) -> Self {
        let mut bits = [0u64; U64_BLOCKS];
        for i in 0..157 {
            let mut block_bytes = [0u8; 8];
            block_bytes.copy_from_slice(&bytes[i * 8..(i + 1) * 8]);
            bits[i] = u64::from_le_bytes(block_bytes);
        }
        Hypervector { bits }
    }

    /// Sequence-Preserving Sentence Encoder with Word-Level Permutations
    pub fn encode_sentence(text: &str) -> Self {
        let words: Vec<&str> = text
            .split_whitespace()
            .map(|w| w.trim_matches(|c: char| c.is_ascii_punctuation()))
            .filter(|w| !w.is_empty())
            .collect();
        if words.is_empty() {
            return Self::new_zero();
        }
        let mut word_vectors = Vec::new();
        for (i, word) in words.iter().enumerate() {
            // Encode each word as a 3-gram character hypervector
            let word_hv = Self::encode_text_ngram(word, 3);
            // Permute the word vector by rotating left based on its position index
            let rotated = word_hv.rotate_left(i * 13);
            word_vectors.push(rotated);
        }
        let refs: Vec<&Hypervector> = word_vectors.iter().collect();
        Self::bundle(&refs)
    }
}

#[derive(Clone, Debug)]
pub struct VarConfig {
    pub id: Hypervector,
    pub min_val: f64,
    pub max_val: f64,
    pub base_min: Hypervector,
    pub base_max: Hypervector,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct DejavuEntry {
    pub vector: Hypervector,
    pub label: String,
    pub metadata: HashMap<String, String>,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct MemoryCluster {
    pub centroid: Hypervector,
    pub entries: Vec<DejavuEntry>,
    /// Accumulated reinforcement across access events.
    /// Decayed each tick by `decay_permanent_clusters`.
    /// When this drops below threshold the cluster is pruned.
    #[serde(default)]
    pub reverberation: f64,
    /// Last brain tick at which this cluster was accessed.
    /// Used to detect staleness for demotion.
    #[serde(default)]
    pub last_reinforced_tick: usize,
}

#[derive(serde::Serialize, serde::Deserialize, Clone, Debug)]
pub enum HiveMessage {
    HandshakeRequest {
        agent_id: String,
        role: String,
    },
    HandshakeResponse {
        permanent_clusters: Vec<MemoryCluster>,
    },
    ConsolidateRequest {
        centroid: Hypervector,
        entries: Vec<DejavuEntry>,
        /// Current cognitive anxiety of the submitting agent [0, 1].
        /// Used by the broker for anxiety-weighted consensus bundling.
        agent_anxiety: f64,
    },
    SyncUpdate {
        is_new_cluster: bool,
        cluster_index: Option<usize>,
        cluster: MemoryCluster,
    },
    PanicLockdown {
        attacker_info: String,
    },
    /// Broadcast by the broker when consensus between agents falls below
    /// the structural coherence threshold.  Forces all agents to rotate
    /// active intents and re-sample the environment.
    DissonanceAlert {
        /// Average pairwise similarity across agent submissions
        consensus_similarity: f64,
        /// Number of active agents that contributed to the consensus check
        agent_count: usize,
    },
}

#[derive(Clone, Debug)]
pub struct TransientCluster {
    pub centroid: Hypervector,
    pub entries: Vec<DejavuEntry>,
    pub reverberation: f64,
    pub last_reinforced_tick: usize,
}

pub struct VSABrain {
    pub variables: HashMap<String, VarConfig>,
    pub concepts: HashMap<String, Hypervector>,
    pub dejavu_clusters: Vec<MemoryCluster>,
    pub transient_clusters: Vec<TransientCluster>,
    pub threshold: f64,
    pub tick_counter: usize,
    pub anxiety: f64,
    pub experiences: Vec<Hypervector>,
    /// HNSW spatial index for O(log n) memory retrieval.
    /// Rebuilt when stale (incremental rebuild on tick boundaries).
    hnsw_index: Option<HnswIndex>,
    /// Tracks the cluster count at last HNSW rebuild
    hnsw_last_rebuild_count: usize,
}

impl VSABrain {
    pub fn new(threshold: f64) -> Self {
        VSABrain {
            variables: HashMap::new(),
            concepts: HashMap::new(),
            dejavu_clusters: Vec::new(),
            transient_clusters: Vec::new(),
            threshold,
            tick_counter: 0,
            anxiety: 0.0,
            experiences: Vec::new(),
            hnsw_index: None,
            hnsw_last_rebuild_count: 0,
        }
    }

    pub fn generate_vector(&self) -> Hypervector {
        Hypervector::new_random()
    }

    pub fn bind(&self, v1: &Hypervector, v2: &Hypervector) -> Hypervector {
        v1.bitwise_xor(v2)
    }

    pub fn unbind(&self, v1: &Hypervector, v2: &Hypervector) -> Hypervector {
        v1.bitwise_xor(v2)
    }

    pub fn similarity(&self, v1: &Hypervector, v2: &Hypervector) -> f64 {
        1.0 - v1.normalized_hamming_distance(v2)
    }

    pub fn register_variable(&mut self, name: &str, min_val: f64, max_val: f64) {
        let id = Hypervector::new_random();
        let base_min = Hypervector::new_random();
        let base_max = Hypervector::new_random();
        self.variables.insert(
            name.to_string(),
            VarConfig {
                id,
                min_val,
                max_val,
                base_min,
                base_max,
            },
        );
    }

    pub fn register_concept(&mut self, name: &str) -> Hypervector {
        let vec = Hypervector::new_random();
        self.concepts.insert(name.to_string(), vec);
        vec
    }

    pub fn encode_continuous(&self, name: &str, val: f64) -> Option<Hypervector> {
        let config = self.variables.get(name)?;
        Some(Hypervector::encode_continuous(config, val))
    }

    pub fn encode_and_bind_variable(&self, name: &str, val: f64) -> Option<Hypervector> {
        let config = self.variables.get(name)?;
        let val_vector = Hypervector::encode_continuous(config, val);
        Some(config.id.bitwise_xor(&val_vector))
    }

    pub fn bundle(&self, vectors: &[Hypervector]) -> Hypervector {
        let refs: Vec<&Hypervector> = vectors.iter().collect();
        Hypervector::bundle(&refs)
    }

    pub fn compile_state_vector(&self, telemetry: &HashMap<String, f64>) -> Hypervector {
        let mut bound_vectors = Vec::new();
        for (name, val) in telemetry {
            if let Some(bound) = self.encode_and_bind_variable(name, *val) {
                bound_vectors.push(bound);
            }
        }
        let refs: Vec<&Hypervector> = bound_vectors.iter().collect();
        Hypervector::bundle(&refs)
    }

    pub fn add_to_dejavu_db(
        &mut self,
        vector: Hypervector,
        label: &str,
        metadata: HashMap<String, String>,
    ) {
        let entry = DejavuEntry {
            vector,
            label: label.to_string(),
            metadata,
        };

        let cluster_threshold = 0.65; // Similarity threshold to group under same centroid
        let mut best_idx = None;
        let mut best_sim = -1.0;

        for (idx, cluster) in self.dejavu_clusters.iter().enumerate() {
            let sim = 1.0 - vector.normalized_hamming_distance(&cluster.centroid);
            if sim > best_sim {
                best_sim = sim;
                best_idx = Some(idx);
            }
        }

        if let Some(idx) = best_idx {
            if best_sim >= cluster_threshold {
                self.dejavu_clusters[idx].entries.push(entry);
                // Recompute centroid
                let refs: Vec<&Hypervector> = self.dejavu_clusters[idx]
                    .entries
                    .iter()
                    .map(|e| &e.vector)
                    .collect();
                self.dejavu_clusters[idx].centroid = Hypervector::bundle(&refs);
                // Reinforce: successful access boosts reverberation
                self.dejavu_clusters[idx].reverberation =
                    (self.dejavu_clusters[idx].reverberation + 0.2).min(1.0);
                self.dejavu_clusters[idx].last_reinforced_tick = self.tick_counter;
                return;
            }
        }

        // Spawn new cluster
        self.dejavu_clusters.push(MemoryCluster {
            centroid: vector,
            entries: vec![entry],
            reverberation: 1.0,
            last_reinforced_tick: self.tick_counter,
        });
    }

    /// Collect centroids from permanent clusters whose entries bear
    /// the `learned_crisis_pattern` metadata tag.  These are injected
    /// into the crisis_concepts slice before every planning call so
    /// that experience feedback actually affects future action costs.
    pub fn collect_learned_crisis_concepts(&self) -> Vec<Hypervector> {
        let mut concepts = Vec::new();
        for cluster in &self.dejavu_clusters {
            for entry in &cluster.entries {
                if entry.metadata.get("type") == Some(&"learned_crisis_pattern".to_string()) {
                    concepts.push(cluster.centroid);
                    break;  // one centroid per cluster regardless of how many entries match
                }
            }
        }
        concepts
    }

    /// Periodically decay all permanent clusters.
    /// Clusters whose reverberation drops below `theta_retain` are removed
    /// (demoted from planning influence).  This prevents old crisis patterns
    /// from permanently distorting the cost landscape after the regime passes.
    pub fn decay_permanent_clusters(&mut self, lambda: f64, theta_retain: f64) {
        for cluster in self.dejavu_clusters.iter_mut() {
            cluster.reverberation *= lambda;
        }
        // Remove clusters that have both decayed below threshold AND
        // haven't been reinforced in the last 50 ticks (avoid flapping).
        let now = self.tick_counter;
        self.dejavu_clusters.retain(|c| {
            c.reverberation >= theta_retain
                || now.saturating_sub(c.last_reinforced_tick) <= 50
        });
    }

    pub fn add_transient_fact(
        &mut self,
        vector: Hypervector,
        label: &str,
        metadata: HashMap<String, String>,
    ) {
        let entry = DejavuEntry {
            vector,
            label: label.to_string(),
            metadata,
        };

        let cluster_threshold = 0.65;
        let mut best_idx = None;
        let mut best_sim = -1.0;

        for (idx, cluster) in self.transient_clusters.iter().enumerate() {
            let sim = 1.0 - vector.normalized_hamming_distance(&cluster.centroid);
            if sim > best_sim {
                best_sim = sim;
                best_idx = Some(idx);
            }
        }

        if let Some(idx) = best_idx {
            if best_sim >= cluster_threshold {
                self.transient_clusters[idx].entries.push(entry);
                self.transient_clusters[idx].last_reinforced_tick = self.tick_counter;
                self.transient_clusters[idx].reverberation += best_sim;

                // Recompute centroid
                let refs: Vec<&Hypervector> = self.transient_clusters[idx]
                    .entries
                    .iter()
                    .map(|e| &e.vector)
                    .collect();
                self.transient_clusters[idx].centroid = Hypervector::bundle(&refs);
                return;
            }
        }

        self.transient_clusters.push(TransientCluster {
            centroid: vector,
            entries: vec![entry],
            reverberation: 1.0,
            last_reinforced_tick: self.tick_counter,
        });
    }

    pub fn decay_transient_clusters(
        &mut self,
        lambda: f64,
        theta_resonance: f64,
        theta_coherence: f64,
    ) {
        self.tick_counter = self.tick_counter.wrapping_add(1);

        // 1. Decay all transient clusters
        for cluster in self.transient_clusters.iter_mut() {
            cluster.reverberation *= lambda;
        }

        // 2. Evaluate Three-Stage Consolidation Pipeline
        let mut consolidated_indices = Vec::new();
        for (idx, cluster) in self.transient_clusters.iter().enumerate() {
            // Stage 1: Temporal Resonance Gate
            if cluster.reverberation > theta_resonance {
                let num_entries = cluster.entries.len();
                if num_entries == 0 {
                    consolidated_indices.push(idx);
                    continue;
                }

                // Stage 2: Clarity Gate (Unanimity Ratio)
                let mut unanimity_count = 0;
                for block_idx in 0..157 {
                    let mut bit_agreement = [0u32; 64];
                    for entry in &cluster.entries {
                        for bit_idx in 0..64 {
                            if ((entry.vector.bits[block_idx] >> bit_idx) & 1) == 1 {
                                bit_agreement[bit_idx] += 1;
                            }
                        }
                    }
                    for bit_idx in 0..64 {
                        let ones = bit_agreement[bit_idx] as f64 / num_entries as f64;
                        if ones > 0.80 || ones < 0.20 {
                            unanimity_count += 1;
                        }
                    }
                }

                let unanimity_ratio = unanimity_count as f64 / 10048.0;

                // Stage 3: Structural Router (Goldilocks Sieve)
                if unanimity_ratio > theta_coherence {
                    let (best_label, sim, _) = self.query_dejavu(&cluster.centroid);

                    if sim >= 0.75 {
                        // Merge into matching permanent cluster
                        if let Some(best_lbl) = best_label {
                            if let Some(p_idx) = self
                                .dejavu_clusters
                                .iter()
                                .enumerate()
                                .find(|(_, pc)| {
                                    pc.entries.iter().any(|e| e.label == best_lbl)
                                        || pc
                                            .entries
                                            .first()
                                            .map(|fe| fe.label.clone())
                                            .unwrap_or_default()
                                            == best_lbl
                                })
                                .map(|(i, _)| i)
                            {
                                for entry in &cluster.entries {
                                    self.dejavu_clusters[p_idx].entries.push(entry.clone());
                                }
                                let refs: Vec<&Hypervector> = self.dejavu_clusters[p_idx]
                                    .entries
                                    .iter()
                                    .map(|e| &e.vector)
                                    .collect();
                                self.dejavu_clusters[p_idx].centroid = Hypervector::bundle(&refs);
                            } else {
                                self.dejavu_clusters.push(MemoryCluster {
                                    centroid: cluster.centroid,
                                    entries: cluster.entries.clone(),
                                    reverberation: cluster.reverberation,
                                    last_reinforced_tick: self.tick_counter,
                                });
                            }
                        } else {
                            self.dejavu_clusters.push(MemoryCluster {
                                centroid: cluster.centroid,
                                entries: cluster.entries.clone(),
                                reverberation: cluster.reverberation,
                                last_reinforced_tick: self.tick_counter,
                            });
                        }
                    } else if sim < 0.52 {
                        // Reject as noise
                    } else {
                        // Consolidate into new permanent cluster
                        self.dejavu_clusters.push(MemoryCluster {
                            centroid: cluster.centroid,
                            entries: cluster.entries.clone(),
                            reverberation: cluster.reverberation,
                            last_reinforced_tick: self.tick_counter,
                        });
                    }
                }

                consolidated_indices.push(idx);
            }
        }

        // 3. Epsilon and Step Pruning
        for (idx, cluster) in self.transient_clusters.iter().enumerate() {
            if consolidated_indices.contains(&idx) {
                continue;
            }
            if cluster.reverberation < 0.05
                || (self
                    .tick_counter
                    .saturating_sub(cluster.last_reinforced_tick))
                    > 50
            {
                consolidated_indices.push(idx);
            }
        }

        consolidated_indices.sort_unstable_by(|a, b| b.cmp(a));
        for idx in consolidated_indices {
            if idx < self.transient_clusters.len() {
                self.transient_clusters.remove(idx);
            }
        }

        // 4. Update Cognitive Anxiety: A(t) = tanh( 0.2 * Sum( R_j / theta_resonance ) )
        let sum_reverberation: f64 = self
            .transient_clusters
            .iter()
            .map(|c| c.reverberation)
            .sum();
        let normalized_sum = sum_reverberation / theta_resonance;
        self.anxiety = (0.2 * normalized_sum).tanh();
    }

    pub fn decay_transient_clusters_distributed(
        &mut self,
        lambda: f64,
        theta_resonance: f64,
        theta_coherence: f64,
    ) -> Vec<(Hypervector, Vec<DejavuEntry>)> {
        self.tick_counter = self.tick_counter.wrapping_add(1);
        let mut consolidated = Vec::new();

        // 1. Decay all transient clusters
        for cluster in self.transient_clusters.iter_mut() {
            cluster.reverberation *= lambda;
        }

        // 2. Evaluate Gates
        let mut consolidated_indices = Vec::new();
        for (idx, cluster) in self.transient_clusters.iter().enumerate() {
            if cluster.reverberation > theta_resonance {
                let num_entries = cluster.entries.len();
                if num_entries == 0 {
                    consolidated_indices.push(idx);
                    continue;
                }

                // Stage 2: Clarity Gate (Unanimity Ratio)
                let mut unanimity_count = 0;
                for block_idx in 0..157 {
                    let mut bit_agreement = [0u32; 64];
                    for entry in &cluster.entries {
                        for bit_idx in 0..64 {
                            if ((entry.vector.bits[block_idx] >> bit_idx) & 1) == 1 {
                                bit_agreement[bit_idx] += 1;
                            }
                        }
                    }
                    for bit_idx in 0..64 {
                        let ones = bit_agreement[bit_idx] as f64 / num_entries as f64;
                        if ones > 0.80 || ones < 0.20 {
                            unanimity_count += 1;
                        }
                    }
                }

                let unanimity_ratio = unanimity_count as f64 / 10048.0;

                if unanimity_ratio > theta_coherence {
                    consolidated.push((cluster.centroid, cluster.entries.clone()));
                }
                consolidated_indices.push(idx);
            }
        }

        // 3. Epsilon and Step Pruning
        for (idx, cluster) in self.transient_clusters.iter().enumerate() {
            if consolidated_indices.contains(&idx) {
                continue;
            }
            if cluster.reverberation < 0.05
                || (self
                    .tick_counter
                    .saturating_sub(cluster.last_reinforced_tick))
                    > 50
            {
                consolidated_indices.push(idx);
            }
        }

        consolidated_indices.sort_unstable_by(|a, b| b.cmp(a));
        for idx in consolidated_indices {
            if idx < self.transient_clusters.len() {
                self.transient_clusters.remove(idx);
            }
        }

        // 4. Update Cognitive Anxiety
        let sum_reverberation: f64 = self
            .transient_clusters
            .iter()
            .map(|c| c.reverberation)
            .sum();
        let normalized_sum = sum_reverberation / theta_resonance;
        self.anxiety = (0.2 * normalized_sum).tanh();

        consolidated
    }

    pub fn query_dejavu(
        &self,
        vector: &Hypervector,
    ) -> (Option<String>, f64, HashMap<String, String>) {
        if self.dejavu_clusters.is_empty() && self.transient_clusters.is_empty() {
            return (None, 0.0, HashMap::new());
        }

        let mut best_label = None;
        let mut best_sim = -1.0;
        let mut best_meta = HashMap::new();

        // A. Search permanent clusters
        let mut perm_similarities = Vec::new();
        let mut best_perm_centroid_sim = -1.0;
        for (idx, cluster) in self.dejavu_clusters.iter().enumerate() {
            let sim = 1.0 - vector.normalized_hamming_distance(&cluster.centroid);
            perm_similarities.push((idx, sim));
            if sim > best_perm_centroid_sim {
                best_perm_centroid_sim = sim;
            }
        }

        let search_threshold_perm = best_perm_centroid_sim - 0.08;
        for &(idx, sim) in &perm_similarities {
            if sim >= search_threshold_perm {
                let cluster = &self.dejavu_clusters[idx];
                for entry in &cluster.entries {
                    let entry_sim = 1.0 - vector.normalized_hamming_distance(&entry.vector);
                    if entry_sim > best_sim {
                        best_sim = entry_sim;
                        best_label = Some(entry.label.clone());
                        best_meta = entry.metadata.clone();
                    }
                }
            }
        }

        // B. Search transient clusters
        let mut trans_similarities = Vec::new();
        let mut best_trans_centroid_sim = -1.0;
        for (idx, cluster) in self.transient_clusters.iter().enumerate() {
            let sim = 1.0 - vector.normalized_hamming_distance(&cluster.centroid);
            trans_similarities.push((idx, sim));
            if sim > best_trans_centroid_sim {
                best_trans_centroid_sim = sim;
            }
        }

        let search_threshold_trans = best_trans_centroid_sim - 0.08;
        for &(idx, sim) in &trans_similarities {
            if sim >= search_threshold_trans {
                let cluster = &self.transient_clusters[idx];
                for entry in &cluster.entries {
                    let entry_sim = 1.0 - vector.normalized_hamming_distance(&entry.vector);
                    if entry_sim > best_sim {
                        best_sim = entry_sim;
                        best_label = Some(entry.label.clone());
                        best_meta = entry.metadata.clone();
                    }
                }
            }
        }

        (best_label, best_sim, best_meta)
    }

    pub fn evaluate_deja_vu(&self, vector: &Hypervector) -> (Option<String>, f64) {
        if self.dejavu_clusters.is_empty() && self.transient_clusters.is_empty() {
            return (None, 1.0);
        }

        let mut best_label = None;
        let mut min_dist = 1.0;

        // A. Search permanent clusters
        let mut perm_similarities = Vec::new();
        let mut best_perm_centroid_sim = -1.0;
        for (idx, cluster) in self.dejavu_clusters.iter().enumerate() {
            let sim = 1.0 - vector.normalized_hamming_distance(&cluster.centroid);
            perm_similarities.push((idx, sim));
            if sim > best_perm_centroid_sim {
                best_perm_centroid_sim = sim;
            }
        }

        let search_threshold_perm = best_perm_centroid_sim - 0.08;
        for &(idx, sim) in &perm_similarities {
            if sim >= search_threshold_perm {
                let cluster = &self.dejavu_clusters[idx];
                for entry in &cluster.entries {
                    let dist = vector.normalized_hamming_distance(&entry.vector);
                    if dist < min_dist {
                        min_dist = dist;
                        best_label = Some(entry.label.clone());
                    }
                }
            }
        }

        // B. Search transient clusters
        let mut trans_similarities = Vec::new();
        let mut best_trans_centroid_sim = -1.0;
        for (idx, cluster) in self.transient_clusters.iter().enumerate() {
            let sim = 1.0 - vector.normalized_hamming_distance(&cluster.centroid);
            trans_similarities.push((idx, sim));
            if sim > best_trans_centroid_sim {
                best_trans_centroid_sim = sim;
            }
        }

        let search_threshold_trans = best_trans_centroid_sim - 0.08;
        for &(idx, sim) in &trans_similarities {
            if sim >= search_threshold_trans {
                let cluster = &self.transient_clusters[idx];
                for entry in &cluster.entries {
                    let dist = vector.normalized_hamming_distance(&entry.vector);
                    if dist < min_dist {
                        min_dist = dist;
                        best_label = Some(entry.label.clone());
                    }
                }
            }
        }

        if min_dist <= self.threshold {
            (best_label, min_dist)
        } else {
            (None, min_dist)
        }
    }

    // ─── HNSW-Accelerated Spatial Index ──────────────────────────────

    /// Rebuild the HNSW index from the current permanent clusters.
    /// Uses the centroid of each cluster as the indexed vector.
    /// Maps HNSW entry index → cluster index for result translation.
    pub fn rebuild_hnsw_index(&mut self) {
        if self.dejavu_clusters.is_empty() {
            self.hnsw_index = None;
            self.hnsw_last_rebuild_count = 0;
            return;
        }

        let mut index = HnswIndex::with_config(crate::hnsw::HnswConfig {
            use_heuristic: true,
            ..crate::hnsw::HnswConfig::default()
        });

        let mut cluster_indices: Vec<usize> = Vec::new();

        for (ci, cluster) in self.dejavu_clusters.iter().enumerate() {
            let hv = &cluster.centroid;
            let idx = index.insert(&hv.bits);
            // HNSW assigns sequential indices matching our insert order
            // Map HNSW index → cluster index
            cluster_indices.push(ci);
        }

        self.hnsw_index = Some(index);
        self.hnsw_last_rebuild_count = self.dejavu_clusters.len();
    }

    /// Ensure the HNSW index is fresh. Rebuilds if clusters have changed.
    pub fn ensure_hnsw_index(&mut self) {
        let needs_rebuild = match self.hnsw_index {
            Some(_) => self.hnsw_last_rebuild_count != self.dejavu_clusters.len(),
            None => !self.dejavu_clusters.is_empty(),
        };
        if needs_rebuild {
            self.rebuild_hnsw_index();
        }
    }

    /// Query using HNSW-accelerated nearest-neighbor search.
    /// Falls back to linear scan if index is unavailable.
    pub fn query_dejavu_hnsw(
        &mut self,
        vector: &Hypervector,
        ef: usize,
    ) -> (Option<String>, f64, HashMap<String, String>) {
        self.ensure_hnsw_index();

        if let Some(ref index) = self.hnsw_index {
            let result = index.search_by_hypervector(vector, ef);
            if !result.is_empty() {
                let (hnsw_idx, dist) = result.closest().unwrap();
                if hnsw_idx < self.dejavu_clusters.len() {
                    let cluster = &self.dejavu_clusters[hnsw_idx];
                    // Search within the best cluster's entries
                    let mut best_label = None;
                    let mut best_sim = -1.0;
                    let mut best_meta = HashMap::new();

                    for entry in &cluster.entries {
                        let sim = 1.0 - vector.normalized_hamming_distance(&entry.vector);
                        if sim > best_sim {
                            best_sim = sim;
                            best_label = Some(entry.label.clone());
                            best_meta = entry.metadata.clone();
                        }
                    }

                    if best_sim > 0.0 {
                        return (best_label, best_sim, best_meta);
                    }
                }
            }
        }

        // Fallback to linear scan
        self.query_dejavu(vector)
    }

    pub fn decode_variable(
        &self,
        state_vector: &Hypervector,
        var_name: &str,
        resolution: usize,
    ) -> Option<f64> {
        let config = self.variables.get(var_name)?;
        let unbound = state_vector.bitwise_xor(&config.id);

        let mut best_val = config.min_val;
        let mut max_sim = -1.0;

        for step in 0..=resolution {
            let fraction = (step as f64) / (resolution as f64);
            let val = config.min_val + fraction * (config.max_val - config.min_val);
            let encoded = Hypervector::encode_continuous(config, val);

            let sim = 1.0 - unbound.normalized_hamming_distance(&encoded);
            if sim > max_sim {
                max_sim = sim;
                best_val = val;
            }
        }
        Some(best_val)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hamming_distance_ident() {
        let v1 = Hypervector::new_random();
        assert_eq!(v1.normalized_hamming_distance(&v1), 0.0);
        let v2 = Hypervector::new_random();
        let dist = v1.normalized_hamming_distance(&v2);
        assert!(
            dist > 0.40 && dist < 0.60,
            "Hamming distance of random vectors should be around 0.5, got {}",
            dist
        );
    }

    #[test]
    fn test_char_encoder() {
        let v1 = Hypervector::encode_char('a', 42);
        let v2 = Hypervector::encode_char('a', 42);
        let v3 = Hypervector::encode_char('b', 42);
        assert_eq!(v1, v2);
        assert_ne!(v1, v3);
    }

    #[test]
    fn test_ledger_serialization() {
        let v1 = Hypervector::new_random();
        let bytes = v1.to_bytes_1250();
        let v2 = Hypervector::from_bytes_1250(&bytes);
        for i in 0..156 {
            assert_eq!(v1.bits[i], v2.bits[i]);
        }
        let mask = (1u64 << 16) - 1;
        assert_eq!(v1.bits[156] & mask, v2.bits[156] & mask);
    }

    #[test]
    fn test_continuous_encoding() {
        let config = VarConfig {
            id: Hypervector::new_random(),
            min_val: -3.0,
            max_val: 3.0,
            base_min: Hypervector::new_random(),
            base_max: Hypervector::new_random(),
        };
        let v_min = Hypervector::encode_continuous(&config, -3.0);
        let v_max = Hypervector::encode_continuous(&config, 3.0);
        let v_mid = Hypervector::encode_continuous(&config, 0.0);

        let d_min_max = v_min.normalized_hamming_distance(&v_max);
        let d_min_mid = v_min.normalized_hamming_distance(&v_mid);
        let d_mid_max = v_mid.normalized_hamming_distance(&v_max);

        // Distance should scale linearly
        assert!(d_min_mid < d_min_max);
        assert!(d_mid_max < d_min_max);
    }

    #[test]
    fn test_encode_sentence() {
        let s1 = Hypervector::encode_sentence("the hacker breached the server");
        let s2 = Hypervector::encode_sentence("the hacker breached the server");
        let s3 = Hypervector::encode_sentence("the server breached the hacker");

        assert_eq!(s1, s2);
        assert_ne!(s1, s3);

        let dist = s1.normalized_hamming_distance(&s3);
        assert!(
            dist > 0.20,
            "Sentences with different word order should have high Hamming distance, got {}",
            dist
        );
    }

    #[test]
    fn test_hierarchical_clustering() {
        let mut brain = VSABrain::new(0.43);

        let mut meta = HashMap::new();
        meta.insert("test".to_string(), "1".to_string());

        // Add 10 random vectors
        for i in 0..10 {
            let v = Hypervector::new_random();
            brain.add_to_dejavu_db(v, &format!("label_{}", i), meta.clone());
        }

        assert!(!brain.dejavu_clusters.is_empty());

        let query_vec = Hypervector::new_random();
        let (label, sim, _) = brain.query_dejavu(&query_vec);
        println!("Best query match: {:?}, sim: {}", label, sim);
    }

    #[test]
    fn test_transient_consolidation() {
        let mut brain = VSABrain::new(0.43);
        let mut meta = HashMap::new();
        meta.insert("test".to_string(), "1".to_string());

        let fact_vec = Hypervector::new_random();

        // Add 6 times to cross resonance
        for i in 0..6 {
            brain.add_transient_fact(fact_vec, &format!("persistent_fact_{}", i), meta.clone());
        }

        assert_eq!(brain.transient_clusters.len(), 1);
        assert!(brain.transient_clusters[0].reverberation > 3.0);

        // Run decay loop with low thresholds for test
        brain.decay_transient_clusters(0.95, 3.0, 0.10);

        // Should be promoted
        assert_eq!(brain.transient_clusters.len(), 0);
        assert!(!brain.dejavu_clusters.is_empty());
        assert_eq!(brain.dejavu_clusters[0].entries.len(), 6);
    }

    #[tokio::test]
    async fn test_multi_agent_sync() {
        use crate::broker::NeocortexBroker;
        use crate::HiveMessage;
        use std::sync::Arc;
        use tokio::net::TcpStream;
        use tokio::sync::mpsc;

        let port = 19050;
        let ledger_path = "data/temp_test_broker_ledger.bin";
        let _ = std::fs::remove_file(ledger_path);

        let broker = Arc::new(NeocortexBroker::new("test_secret_key", ledger_path, port));
        let broker_clone = Arc::clone(&broker);
        let (log_tx, _log_rx) = mpsc::unbounded_channel();

        tokio::spawn(async move {
            let _ = broker_clone.run(log_tx).await;
        });

        tokio::time::sleep(tokio::time::Duration::from_millis(200)).await;

        let stream = TcpStream::connect(format!("127.0.0.1:{}", port))
            .await
            .unwrap();
        let (reader, writer) = stream.into_split();
        let mut reader = reader;
        let mut writer = writer;

        let handshake = HiveMessage::HandshakeRequest {
            agent_id: "agent_test_1".to_string(),
            role: "News".to_string(),
        };
        NeocortexBroker::write_msg(&mut writer, &handshake, "test_secret_key")
            .await
            .unwrap();

        match NeocortexBroker::read_msg(&mut reader, "test_secret_key")
            .await
            .unwrap()
            .unwrap()
        {
            HiveMessage::HandshakeResponse { permanent_clusters } => {
                assert!(permanent_clusters.is_empty());
            }
            _ => panic!("Expected HandshakeResponse"),
        }

        let centroid = Hypervector::new_random();
        let entry = DejavuEntry {
            vector: centroid,
            label: "test_fact".to_string(),
            metadata: HashMap::new(),
        };
        let consolidate = HiveMessage::ConsolidateRequest {
            centroid,
            entries: vec![entry],
            agent_anxiety: 0.0,
        };
        NeocortexBroker::write_msg(&mut writer, &consolidate, "test_secret_key")
            .await
            .unwrap();

        match NeocortexBroker::read_msg(&mut reader, "test_secret_key")
            .await
            .unwrap()
            .unwrap()
        {
            HiveMessage::SyncUpdate {
                is_new_cluster,
                cluster,
                ..
            } => {
                assert!(is_new_cluster);
                assert_eq!(cluster.entries.len(), 1);
                assert_eq!(cluster.entries[0].label, "test_fact");
            }
            _ => panic!("Expected SyncUpdate"),
        }

        let _ = std::fs::remove_file(ledger_path);
    }

    #[tokio::test]
    async fn test_lockdown_propagation() {
        use crate::broker::NeocortexBroker;
        use crate::defense::DefenseSystem;
        use crate::socket::AdminSocketServer;
        use crate::HiveMessage;
        use std::sync::Arc;
        use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
        use tokio::net::TcpStream;
        use tokio::sync::{mpsc, Mutex, RwLock};

        let broker_port = 19053;
        let admin_port = 19006;
        let ledger_path = "data/temp_test_lockdown_ledger.bin";
        let _ = std::fs::remove_file(ledger_path);

        // 1. Start Broker
        let broker = Arc::new(NeocortexBroker::new(
            "test_secret_key",
            ledger_path,
            broker_port,
        ));
        let broker_clone = Arc::clone(&broker);
        let (log_tx, mut log_rx) = mpsc::unbounded_channel();

        tokio::spawn(async move {
            let _ = broker_clone.run(log_tx).await;
        });

        // Drain logs in background so mpsc queue doesn't fill up
        tokio::spawn(async move { while let Some(_) = log_rx.recv().await {} });

        tokio::time::sleep(tokio::time::Duration::from_millis(200)).await;

        // 2. Connect Mock Agent to Broker
        let stream = TcpStream::connect(format!("127.0.0.1:{}", broker_port))
            .await
            .unwrap();
        let (reader, writer) = stream.into_split();
        let mut reader = reader;
        let writer = Arc::new(Mutex::new(writer));

        // 3. Handshake
        let handshake = HiveMessage::HandshakeRequest {
            agent_id: "agent_lockdown_test".to_string(),
            role: "News".to_string(),
        };
        {
            let mut writer_guard = writer.lock().await;
            NeocortexBroker::write_msg(&mut writer_guard, &handshake, "test_secret_key")
                .await
                .unwrap();
        }

        let initial_clusters = match NeocortexBroker::read_msg(&mut reader, "test_secret_key")
            .await
            .unwrap()
            .unwrap()
        {
            HiveMessage::HandshakeResponse { permanent_clusters } => permanent_clusters,
            _ => panic!("Expected HandshakeResponse"),
        };

        // 4. Initialize Local Mock Agent components
        let mut brain = VSABrain::new(0.43);
        brain.dejavu_clusters = initial_clusters;
        let brain_shared = Arc::new(RwLock::new(brain));

        let initial_intent = Hypervector::new_random();
        let active_intent = Arc::new(RwLock::new(initial_intent));
        let defense = DefenseSystem::new(admin_port);

        // 5. Spawn background listener task for mock agent to receive broker messages
        let intent_recv = Arc::clone(&active_intent);
        let defense_recv = defense.clone();
        let lockdown_received = Arc::new(RwLock::new(false));
        let lr_clone = Arc::clone(&lockdown_received);
        tokio::spawn(async move {
            let mut reader = reader;
            loop {
                match NeocortexBroker::read_msg(&mut reader, "test_secret_key").await {
                    Ok(Some(HiveMessage::PanicLockdown { .. })) => {
                        *lr_clone.write().await = true;

                        // Rotates port and intent
                        let mut port = defense_recv.active_port.write().await;
                        *port = 19007; // dummy new port
                        *defense_recv.stealth_mode.write().await = true;

                        let mut intent_guard = intent_recv.write().await;
                        *intent_guard = Hypervector::new_random();
                    }
                    _ => break,
                }
            }
        });

        // 6. Spawn Admin Socket Server
        let admin_server = AdminSocketServer::new(
            Arc::clone(&active_intent),
            defense.clone(),
            Arc::clone(&brain_shared),
        );
        let (admin_log_tx, mut admin_log_rx) = mpsc::unbounded_channel();
        tokio::spawn(async move {
            let _ = admin_server.run(admin_log_tx).await;
        });
        tokio::spawn(async move { while let Some(_) = admin_log_rx.recv().await {} });

        tokio::time::sleep(tokio::time::Duration::from_millis(200)).await;

        // 7. Spawn Subconscious loop simulation to check threat level and propagate PanicLockdown
        let writer_subconscious = Arc::clone(&writer);
        let defense_subconscious = defense.clone();
        tokio::spawn(async move {
            let mut sent_lockdown = false;
            loop {
                tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
                let threat_level = *defense_subconscious.threat_level.read().await;
                if threat_level >= 1.0 && !sent_lockdown {
                    sent_lockdown = true;
                    let request = HiveMessage::PanicLockdown {
                        attacker_info: "Mock Attack".to_string(),
                    };
                    let mut writer_guard = writer_subconscious.lock().await;
                    let _ = NeocortexBroker::write_msg(&mut writer_guard, &request, "test_secret_key").await;
                }
            }
        });

        // 8. Connect client to Admin Socket and spam invalid commands to trigger threat increase
        let mut admin_conn = TcpStream::connect(format!("127.0.0.1:{}", admin_port))
            .await
            .unwrap();

        // Read header
        let mut admin_reader = BufReader::new(&mut admin_conn);
        let mut banner = String::new();
        let _ = admin_reader.read_line(&mut banner).await;

        // Send unrecognized commands to increase threat level (7 commands of 0.15 = 1.05 > 1.0)
        for _ in 0..8 {
            let _ = admin_conn.write_all(b"SPAM\n").await;
            tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;
        }

        // Wait for lockdown propagation to occur
        tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;

        // Check verification states
        assert!(
            *lockdown_received.read().await,
            "Lockdown broadcast should be received by agent"
        );
        assert_ne!(
            *active_intent.read().await,
            initial_intent,
            "Active intent should be rotated (amnesia)"
        );
        assert_eq!(
            *defense.active_port.read().await,
            19007,
            "Port should be rotated"
        );
        assert!(
            *defense.stealth_mode.read().await,
            "Stealth mode should be active"
        );

        let _ = std::fs::remove_file(ledger_path);
    }

    #[test]
    fn test_resonator_unbinding() {
        use crate::resonator::{factorize_svo, ResonatorVocabulary};
        let mut vocab = ResonatorVocabulary::new();
        vocab.register_term("Finch");
        vocab.register_term("write");
        vocab.register_term("ledger");

        let s_hv = vocab.get_vector("Finch").unwrap();
        let v_hv = vocab.get_vector("write").unwrap();
        let o_hv = vocab.get_vector("ledger").unwrap();

        // T = S_rot1 ^ V_rot2 ^ O_rot3
        let t = s_hv
            .rotate_left(1 * 13)
            .bitwise_xor(&v_hv.rotate_left(2 * 13))
            .bitwise_xor(&o_hv.rotate_left(3 * 13));

        let subjects = vec![
            "Finch".to_string(),
            "Agent-1".to_string(),
            "Broker".to_string(),
        ];
        let verbs = vec!["write".to_string(), "read".to_string(), "panic".to_string()];
        let objects = vec![
            "ledger".to_string(),
            "hosts".to_string(),
            "server".to_string(),
        ];

        let res = factorize_svo(&t, &vocab, &subjects, &verbs, &objects, 30);
        assert!(res.is_some(), "Resonator should resolve the thought vector");
        let (s, v, o, energy) = res.unwrap();
        assert_eq!(s, "Finch");
        assert_eq!(v, "write");
        assert_eq!(o, "ledger");
        assert!(
            energy >= 0.65,
            "Reconstruction energy should pass hallucination filter: {}",
            energy
        );
    }

    #[test]
    fn test_sensory_modalities() {
        use crate::sensory::{
            NetworkTrafficModality, SensoryModality, SystemTelemetryModality, TextSensoryModality,
        };

        let m1 = TextSensoryModality::new("text_mod", "What is the crisis");
        let mut m2 = SystemTelemetryModality::new("telemetry_mod");
        m2.set_reading("cpu_utilization", 45.2);
        let m3 = NetworkTrafficModality::new("network_mod");

        let v1 = m1.encode();
        let v2 = m2.encode();
        let v3 = m3.encode();

        let world_state = Hypervector::bundle(&[&v1, &v2, &v3]);

        // Assert that the bundled vector has some similarity to each component
        let sim1 = 1.0 - world_state.normalized_hamming_distance(&v1);
        let sim2 = 1.0 - world_state.normalized_hamming_distance(&v2);
        let sim3 = 1.0 - world_state.normalized_hamming_distance(&v3);

        assert!(
            sim1 > 0.55,
            "Should have high similarity to text modality, got {}",
            sim1
        );
        assert!(
            sim2 > 0.55,
            "Should have high similarity to telemetry modality, got {}",
            sim2
        );
        assert!(
            sim3 > 0.55,
            "Should have high similarity to network modality, got {}",
            sim3
        );
    }

    #[test]
    fn test_action_execution() {
        use crate::action::{execute_action, ActionRegistry};
        use crate::resonator::ResonatorVocabulary;

        std::fs::create_dir_all("data").unwrap();
        let reg = ActionRegistry::new();
        let mut vocab = ResonatorVocabulary::new();
        vocab.register_term("sys_write");
        vocab.register_term("hello_world_payload");
        vocab.register_term("data/temp_test_write.txt");

        // Action = sys_write, Parameter = data/temp_test_write.txt (we can simplify to just write hello_world_payload)
        // Let's bind them: Intent = sys_write ^ Param
        let act_hv = reg.get_action_vector("sys_write").unwrap();
        let param_hv = vocab.get_vector("data/temp_test_write.txt").unwrap();
        let intent = act_hv.bitwise_xor(param_hv);

        // Decode intent
        let decoded = reg.decode_intent(&intent, &vocab);
        assert!(decoded.is_some());
        let (action_name, p_hv) = decoded.unwrap();
        assert_eq!(action_name, "sys_write");

        // Execute action (creates data/dynamic_output.txt)
        let _ = std::fs::remove_file("data/dynamic_output.txt");
        let res = execute_action(&action_name, &p_hv, &vocab);
        assert!(res.is_ok(), "Action execution should succeed: {:?}", res);

        let content = std::fs::read_to_string("data/dynamic_output.txt").unwrap();
        assert_eq!(content, "data/temp_test_write.txt");

        let _ = std::fs::remove_file("data/dynamic_output.txt");
    }

    #[test]
    fn test_temporal_planning() {
        use crate::action::ActionRegistry;
        use crate::planning::find_optimal_trajectory;
        use crate::resonator::ResonatorVocabulary;

        let reg = ActionRegistry::new();
        let mut vocab = ResonatorVocabulary::new();
        vocab.register_term("hosts");
        vocab.register_term("cargo check");

        let s0 = Hypervector::new_random();

        // Target: We want to execute "sys_read" on "hosts" (Step 1) and "execute_bash" on "cargo check" (Step 2)
        let act1_hv = reg.get_action_vector("sys_read").unwrap();
        let param1_hv = vocab.get_vector("hosts").unwrap();
        let step1 = act1_hv.bitwise_xor(param1_hv);

        let act2_hv = reg.get_action_vector("execute_bash").unwrap();
        let param2_hv = vocab.get_vector("cargo check").unwrap();
        let step2 = act2_hv.bitwise_xor(param2_hv);

        // Goal state: S2 = \rho(\rho(S0) ^ step1) ^ step2  (zero drift assumed)
        let s1 = s0.rotate_left(13).bitwise_xor(&step1);
        let goal_state = s1.rotate_left(13).bitwise_xor(&step2);

        // Run planning solver with zero-drift sequence
        let drift_seq = vec![Hypervector::new_zero(); 2];
        let traj_opt = find_optimal_trajectory(&s0, &goal_state, &drift_seq, &reg, &vocab, 2, &[], 0.0, &[]);
        assert!(traj_opt.is_some(), "Should find a valid trajectory");

        let traj = traj_opt.unwrap();
        assert_eq!(
            traj.steps.len(),
            2,
            "Trajectory should have exactly 2 steps"
        );

        assert_eq!(traj.steps[0].action, "sys_read");
        assert_eq!(traj.steps[0].parameter, "hosts");
        assert_eq!(traj.steps[1].action, "execute_bash");
        assert_eq!(traj.steps[1].parameter, "cargo check");
    }

    #[test]
    fn test_planning_cost_optimization() {
        use crate::action::ActionRegistry;
        use crate::planning::find_optimal_trajectory;
        use crate::resonator::ResonatorVocabulary;

        let reg = ActionRegistry::new();
        let mut vocab = ResonatorVocabulary::new();
        vocab.register_term("hosts");
        vocab.register_term("cargo check");

        let s0 = Hypervector::new_random();

        let act_read = reg.get_action_vector("sys_read").unwrap();
        let param_hosts = vocab.get_vector("hosts").unwrap();
        let step_read = act_read.bitwise_xor(param_hosts);

        let goal = s0.rotate_left(13).bitwise_xor(&step_read);

        let drift_seq = vec![Hypervector::new_zero(); 1];
        let traj = find_optimal_trajectory(&s0, &goal, &drift_seq, &reg, &vocab, 1, &[], 0.0, &[]).unwrap();
        assert_eq!(traj.steps.len(), 1);
        assert_eq!(traj.steps[0].action, "sys_read");
    }

    #[test]
    fn test_order_preservation_and_non_commutativity() {
        use crate::action::ActionRegistry;
        use crate::planning::find_optimal_trajectory;
        use crate::resonator::ResonatorVocabulary;

        let reg = ActionRegistry::new();
        let mut vocab = ResonatorVocabulary::new();
        vocab.register_term("hosts");
        vocab.register_term("cargo check");

        let s0 = Hypervector::new_random();

        let act1_hv = reg.get_action_vector("sys_read").unwrap();
        let param1_hv = vocab.get_vector("hosts").unwrap();
        let step1 = act1_hv.bitwise_xor(param1_hv);

        let act2_hv = reg.get_action_vector("execute_bash").unwrap();
        let param2_hv = vocab.get_vector("cargo check").unwrap();
        let step2 = act2_hv.bitwise_xor(param2_hv);

        // Correct order target: S2 = \rho(\rho(S0) ^ step1) ^ step2
        let goal_correct = s0
            .rotate_left(13)
            .bitwise_xor(&step1)
            .rotate_left(13)
            .bitwise_xor(&step2);

        // Inverted order target: S2 = \rho(\rho(S0) ^ step2) ^ step1
        let goal_inverted = s0
            .rotate_left(13)
            .bitwise_xor(&step2)
            .rotate_left(13)
            .bitwise_xor(&step1);

        // Assert orthogonality of correct and inverted target states (Hamming distance approx 0.50)
        let diff = goal_correct.normalized_hamming_distance(&goal_inverted);
        assert!(diff > 0.40, "Ordered states must be orthogonal: {}", diff);

        let drift_seq = vec![Hypervector::new_zero(); 2];

        // Test pathfinder on goal_correct: should ONLY find [step1, step2] in order
        let traj_correct =
            find_optimal_trajectory(&s0, &goal_correct, &drift_seq, &reg, &vocab, 2, &[], 0.0, &[]).unwrap();
        assert_eq!(traj_correct.steps.len(), 2);
        assert_eq!(traj_correct.steps[0].action, "sys_read");
        assert_eq!(traj_correct.steps[1].action, "execute_bash");

        // Test pathfinder on goal_inverted: should ONLY find [step2, step1] in order
        let traj_inverted =
            find_optimal_trajectory(&s0, &goal_inverted, &drift_seq, &reg, &vocab, 2, &[], 0.0, &[]).unwrap();
        assert_eq!(traj_inverted.steps.len(), 2);
        assert_eq!(traj_inverted.steps[0].action, "execute_bash");
        assert_eq!(traj_inverted.steps[1].action, "sys_read");
    }

    #[test]
    fn test_threat_forecasting_and_correction() {
        use crate::action::ActionRegistry;
        use crate::planning::{find_optimal_trajectory, simulate_threat_trajectory, DriftForecast};
        use crate::resonator::ResonatorVocabulary;

        let reg = ActionRegistry::new();
        let mut vocab = ResonatorVocabulary::new();

        let s0 = Hypervector::new_random();
        let s_stable = Hypervector::new_random();
        let c_crisis = Hypervector::new_random();

        // Setup mock drift pulling the state to crisis in 1 step: S1 = \rho(S0) ^ E_world = c_crisis
        let e_world = c_crisis.bitwise_xor(&s0.rotate_left(13));

        // 1. Forecaster checks active threat and detects crisis prediction at step 1
        let mut forecast = DriftForecast::new();
        forecast.add_regime("test", 1.0, vec![e_world, e_world, e_world]);
        let horizon = simulate_threat_trajectory(&s0, &forecast, &[c_crisis], 0.80);
        assert_eq!(horizon, Some(1.0), "Threat forecaster should detect crisis");

        // 2. We want a corrective action that steers the state from c_crisis to s_stable
        // S1_correct = \rho(S0) ^ A_c ^ E_world = c_crisis ^ A_c \approx s_stable
        // \implies A_c \approx c_crisis ^ s_stable
        let act_read = reg.get_action_vector("sys_read").unwrap();
        let param_vector = c_crisis.bitwise_xor(&s_stable).bitwise_xor(&act_read);

        // Inject custom param vector matching our corrective step into vocabulary terms map
        vocab.terms.insert("hosts".to_string(), param_vector);

        // 3. Run pathfinder targeting s_stable under e_world drift
        let drift_seq = vec![e_world; 1];
        let traj = find_optimal_trajectory(&s0, &s_stable, &drift_seq, &reg, &vocab, 1, &[c_crisis], 0.0, &[]).unwrap();
        assert_eq!(traj.steps.len(), 1);
        assert_eq!(traj.steps[0].action, "sys_read");
        assert_eq!(traj.steps[0].parameter, "hosts");
    }
}
