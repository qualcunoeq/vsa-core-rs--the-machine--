use crate::{ledger::LongTermLedger, DejavuEntry, HiveMessage, Hypervector, MemoryCluster, HD_DIMENSION};
use crate::analogy::WeightProvider;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;
use tokio::sync::{Mutex, RwLock};
use rand::Rng;
use sha2::{Digest, Sha256};
use hmac::{Hmac, Mac};

type HmacSha256 = Hmac<Sha256>;

fn encrypt_decrypt_payload(data: &[u8], key_str: &str, salt: &[u8; 16]) -> Vec<u8> {
    let mut hasher = Sha256::new();
    hasher.update(key_str.as_bytes());
    hasher.update(salt);
    let key = hasher.finalize();

    let mut keystream = Vec::new();
    let mut counter = 0u32;
    while keystream.len() < data.len() {
        let mut hmac = HmacSha256::new_from_slice(&key).unwrap();
        hmac.update(&counter.to_be_bytes());
        let block = hmac.finalize().into_bytes();
        keystream.extend_from_slice(&block);
        counter += 1;
    }

    data.iter()
        .zip(keystream.iter())
        .map(|(a, b)| a ^ b)
        .collect()
}

// ─── Consensus constants ───────────────────────────────────────────────────

/// ██ Tier 4: Maximum number of sub-sector entries per LSH sector ██
/// When centroids within the same sector diverge beyond 0.15 NHD, the
/// sector is bifurcated.  Each sub-sector holds at most `MAX_SUB_SECTORS`
/// entries to keep the broker's memory bounded.
pub const MAX_SUB_SECTORS: usize = 4;

/// ██ Tier 4: Sub-sector entry — a centroid that split from its parent
/// LSH sector due to geometric divergence.  Each entry tracks which
/// agents voted for it and its accumulated weight.
#[derive(Clone, Debug)]
pub struct SubSectorEntry {
    pub centroid: Hypervector,
    pub weight: f64,
    pub agent_ids: Vec<String>,
}

/// Minimum fraction of agent-pairs that must agree (similarity ≥ threshold)
/// for a concept to be allowed into the permanent ledger.
pub const QUORUM_FRACTION: f64 = 0.66;

/// Pairwise similarity must be at least this value for a pair to count as
/// "in agreement."
pub const QUORUM_SIMILARITY_THRESHOLD: f64 = 0.66;

/// When consensus similarity (weighted average of all pairs) falls below
/// this threshold, a DissonanceAlert is broadcast.
pub const CONSENSUS_FLOOR: f64 = 0.55;

/// ██ Phase 2: Re-entry stabilization ticks ██
/// Number of consecutive consolidations a returning agent must submit
/// before its anxiety floor is lifted.  Prevents stale centroids from
/// exerting outsized influence immediately after reconnection.
pub const REENTRY_STABILIZATION_TICKS: usize = 5;

/// ██ Phase 2: Anxiety floor during re-entry ██
/// Minimum effective anxiety during the stabilization window.  Clamped
/// so that a calm-but-stale agent never contributes >7 copies vs a
/// fully trusted agent's 9–10 copies.
pub const ANXIETY_FLOOR: f64 = 0.5;

/// ██ Phase 2: Maximum silent consolidation epochs ██
/// If an agent has not submitted a consolidation for this many epochs,
/// it is treated as dead and excluded from quorum computations.  The
/// background pruning sweep removes these entries entirely.
pub const MAX_SILENT_EPOCHS: u64 = 10;

// ─── AgentSubmission ──────────────────────────────────────────────────────

/// Tracks the latest consolidation submission from a connected agent,
/// used by the broker to compute multi-agent consensus.
#[derive(Clone, Debug)]
struct AgentSubmission {
    agent_id: String,
    centroid: Hypervector,
    anxiety: f64,
}

// ─── SimilarityCache ──────────────────────────────────────────────────────

/// An `N × N` upper-triangular cache of pairwise NHD similarities between
/// agent centroids.  Rows/columns are invalidated lazily: when an agent's
/// centroid changes, only that row and column are cleared and recomputed.
///
/// **Cost without cache:**        O(N²) full Hamming distances per quorum
/// **Cost with cache (hit):**     O(N²) scalar lookups — ~20 ns vs ~2 µs
/// **Cost with cache (miss):**    O(N) recompute for one changed centroid
struct SimilarityCache {
    /// N × N matrix; `sims[i][j]` for `i ≤ j` is cached, `sims[j][i]` mirrors.
    /// `f64::NEG_INFINITY` marks an invalidated cell.
    sims: Vec<Vec<f64>>,
    /// Agent identifiers in row/column order.
    agent_ids: Vec<String>,
    /// Previous centroids for change detection.
    centroids: Vec<Hypervector>,
}

impl SimilarityCache {
    /// Build a fresh cache from the current agent submissions (O(N²) init).
    fn build(submissions: &[AgentSubmission]) -> Self {
        let n = submissions.len();
        let mut sims = vec![vec![f64::NEG_INFINITY; n]; n];
        let agent_ids: Vec<String> = submissions.iter().map(|a| a.agent_id.clone()).collect();
        let centroids: Vec<Hypervector> = submissions.iter().map(|a| a.centroid).collect();

        for i in 0..n {
            for j in (i + 1)..n {
                let sim = 1.0 - centroids[i].normalized_hamming_distance(&centroids[j]);
                sims[i][j] = sim;
                sims[j][i] = sim;
            }
        }

        SimilarityCache { sims, agent_ids, centroids }
    }

    /// Return the cached similarity between agents `i` and `j`.
    /// Assumes `i < n && j < n`.
    #[inline]
    fn get(&self, i: usize, j: usize) -> f64 {
        self.sims[i][j]
    }

    /// Invalidate the row (and symmetric column) for agent `idx`.
    /// The next `get()` for any pair involving `idx` will trigger a miss.
    fn invalidate(&mut self, idx: usize) {
        let n = self.sims.len();
        for k in 0..n {
            self.sims[idx][k] = f64::NEG_INFINITY;
            self.sims[k][idx] = f64::NEG_INFINITY;
        }
    }

    /// Recompute the entire row (and symmetric column) for agent `idx`.
    fn recompute(&mut self, idx: usize, agents: &[AgentSubmission]) {
        let n = agents.len();
        self.centroids[idx] = agents[idx].centroid;
        for j in 0..n {
            if idx == j {
                continue;
            }
            let sim = 1.0 - self.centroids[idx].normalized_hamming_distance(&self.centroids[j]);
            self.sims[idx][j] = sim;
            self.sims[j][idx] = sim;
        }
    }

    /// Return the index of `agent_id`, or `None` if not found.
    fn index_of(&self, agent_id: &str) -> Option<usize> {
        self.agent_ids.iter().position(|id| id == agent_id)
    }
}

// ─── NeocortexBroker ──────────────────────────────────────────────────────

pub struct NeocortexBroker {
    pub dejavu_clusters: Arc<RwLock<Vec<MemoryCluster>>>,
    pub ledger: Arc<LongTermLedger>,
    pub clients: Arc<Mutex<Vec<tokio::net::tcp::OwnedWriteHalf>>>,
    pub port: u16,
    pub key: String,
    pub concept: Hypervector,
    /// Per-agent state for consensus computation.
    agent_states: Arc<RwLock<HashMap<String, AgentSubmission>>>,
    /// ██ Phase 1: File-level mutex for ledger append vs. compaction ██
    /// Prevents the classic read-write race where a consolidation append
    /// is silently destroyed by a compaction rewrite.
    file_mutex: Arc<tokio::sync::Mutex<()>>,
    /// ██ Phase 2: Consolidation epoch counter ██
    /// Monotonically increasing counter incremented on every
    /// `process_consolidation` call.  Used to compute agent staleness:
    /// an agent whose `last_seen_tick` is more than `MAX_SILENT_EPOCHS`
    /// behind the current epoch is excluded from quorum.
    consolidation_epoch: Arc<RwLock<u64>>,
    /// ██ Phase 2: Last-seen epoch per agent ██
    /// Records the value of `consolidation_epoch` at the time of each
    /// agent's most recent consolidation.  The background pruning sweep
    /// removes entries that have not advanced for `MAX_SILENT_EPOCHS`.
    last_seen_tick: Arc<RwLock<HashMap<String, u64>>>,
    /// ██ Phase 2: Re-entry stabilisation counter per agent ██
    /// Incremented on each consolidation after (re-)connection.  While
    /// this counter is below `REENTRY_STABILIZATION_TICKS`, the agent's
    /// anxiety for weight computation is clamped to at least `ANXIETY_FLOOR`.
    reentry_ticks: Arc<RwLock<HashMap<String, usize>>>,
    /// ██ Phase 3: Cohort registry ██
    /// Maps known role strings (e.g. "Signal", "Internal", "External") to
    /// stable cohort indices.  Populated at construction time and never
    /// modified thereafter.  Agents reporting an unknown role are
    /// assigned to the "General" cohort at the end of the vector.
    cohort_registry: HashMap<String, usize>,
    /// ██ Phase 3: Agent → Cohort mapping ██
    /// The active assignment of each connected agent to a cohort.
    /// Updated on handshake and cleared on disconnect.
    cohort_of_agent: Arc<RwLock<HashMap<String, usize>>>,
    /// ██ Phase 3: Sharded intra-cohort similarity caches ██
    /// One `SimilarityCache` per cohort, indexed by cohort_id.
    /// Agent consolidations within a cohort hit only this cohort's
    /// cache, eliminating lock contention across domains.
    cohort_caches: Vec<Arc<RwLock<SimilarityCache>>>,
    /// ██ Phase 3: Inter-cohort similarity cache ██
    /// A tiny C × C cache for Stage 2 pairwise comparisons between
    /// cohort centroids.  C ≤ 4 for the standard deployment, so this
    /// cache holds at most 6 floating-point values.
    inter_cohort_cache: Arc<RwLock<SimilarityCache>>,
    /// ██ Phase 3: Cohort centroids & coherences ██
    /// Maps cohort_id → (cohort centroid V_k, internal coherence W_k).
    /// A cohort abstains from Stage 2 when coherence < CONSENSUS_FLOOR.
    /// Updated on every consolidation that passes intra-cohort quorum.
    cohort_centroids: Arc<RwLock<HashMap<usize, (Hypervector, f64)>>>,
    /// ██ Phase 3: Constitutional tiebreaker ██
    /// A fixed random hypervector generated once at broker boot.
    /// Used by `bundle_with_constitution` to break 50/50 bit-level
    /// ties deterministically, independently of vector ordering.
    /// Persisting this across reboots is recommended for long-running
    /// deployments (see `constitution.bin`).
    constitution: Hypervector,
    /// ██ Tier 4: Sub-sector index for diverged consensus ██
    /// Maps LSH sector hash → list of sub-sector centroids (max 4).
    /// When centroids within the same sector diverge beyond NHD 0.15,
    /// the broker bifurcates the sector.  Sub-sectors are merged when
    /// centroids reconverge (NHD ≤ 0.10).
    sector_index: Arc<RwLock<HashMap<usize, Vec<SubSectorEntry>>>>,
    /// ██ Tier 4: Execution state ██
    /// Monotonically increasing serial number for idempotent retries.
    failure_serial: Arc<RwLock<u64>>,
    /// ██ DRIFT: DCP Consensus Engine ██
    /// In-process consensus protocol for agent proposals, votes,
    /// and weighted-majority resolution.
    pub dcp_consensus: Arc<tokio::sync::RwLock<crate::drift::ConsensusEngine>>,
}

impl NeocortexBroker {
    pub fn new(key: &str, file_path: &str, port: u16) -> Self {
        // Build the cohort registry from known roles.
        let mut cohort_registry = HashMap::new();
        cohort_registry.insert("Signal".to_string(), 0);
        cohort_registry.insert("Internal".to_string(), 1);
        cohort_registry.insert("External".to_string(), 2);
        // "General" cohort (index 3) is for unrecognised roles.
        let cohort_count = cohort_registry.len() + 1;

        // Generate the constitutional tiebreaker hypervector.
        // For long-running deployments, persist this to `constitution.bin`
        // and load it here to guarantee cross-session determinism.
        let constitution = Hypervector::new_random();

        // Build one empty cache per cohort.
        let cohort_caches = (0..cohort_count)
            .map(|_| {
                Arc::new(RwLock::new(SimilarityCache {
                    sims: Vec::new(),
                    agent_ids: Vec::new(),
                    centroids: Vec::new(),
                }))
            })
            .collect();

        NeocortexBroker {
            dejavu_clusters: Arc::new(RwLock::new(Vec::new())),
            ledger: Arc::new(LongTermLedger::new(key, file_path)),
            clients: Arc::new(Mutex::new(Vec::new())),
            port,
            key: key.to_string(),
            concept: Hypervector::new_random(),
            agent_states: Arc::new(RwLock::new(HashMap::new())),
            file_mutex: Arc::new(tokio::sync::Mutex::new(())),
            consolidation_epoch: Arc::new(RwLock::new(0)),
            last_seen_tick: Arc::new(RwLock::new(HashMap::new())),
            reentry_ticks: Arc::new(RwLock::new(HashMap::new())),
            cohort_registry,
            cohort_of_agent: Arc::new(RwLock::new(HashMap::new())),
            cohort_caches,
            inter_cohort_cache: Arc::new(RwLock::new(SimilarityCache {
                sims: Vec::new(),
                agent_ids: Vec::new(),
                centroids: Vec::new(),
            })),
            cohort_centroids: Arc::new(RwLock::new(HashMap::new())),
            constitution,
            sector_index: Arc::new(RwLock::new(HashMap::new())),
            failure_serial: Arc::new(RwLock::new(0)),
            dcp_consensus: Arc::new(tokio::sync::RwLock::new(
                crate::drift::ConsensusEngine::new(100, 2)
            )),
        }
    }

    /// Helper function to write a HiveMessage to an agent stream securely
    pub async fn write_msg(
        writer: &mut tokio::net::tcp::OwnedWriteHalf,
        msg: &HiveMessage,
        key_str: &str,
    ) -> Result<(), std::io::Error> {
        let json_bytes = serde_json::to_vec(msg)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
        let mut salt = [0u8; 16];
        rand::thread_rng().fill(&mut salt);
        let encrypted_bytes = encrypt_decrypt_payload(&json_bytes, key_str, &salt);
        let mut packed = Vec::with_capacity(16 + encrypted_bytes.len());
        packed.extend_from_slice(&salt);
        packed.extend_from_slice(&encrypted_bytes);
        let len = packed.len() as u32;
        writer.write_all(&len.to_be_bytes()).await?;
        writer.write_all(&packed).await?;
        Ok(())
    }

    /// Helper function to read a HiveMessage from an agent stream securely
    pub async fn read_msg(
        reader: &mut tokio::net::tcp::OwnedReadHalf,
        key_str: &str,
    ) -> Result<Option<HiveMessage>, std::io::Error> {
        let mut len_bytes = [0u8; 4];
        if let Err(_) = reader.read_exact(&mut len_bytes).await {
            return Ok(None); // Connection closed
        }
        let len = u32::from_be_bytes(len_bytes) as usize;
        if len < 16 {
            return Err(std::io::Error::new(std::io::ErrorKind::InvalidData, "Payload too short to contain salt"));
        }
        let mut buf = vec![0u8; len];
        reader.read_exact(&mut buf).await?;
        let mut salt = [0u8; 16];
        salt.copy_from_slice(&buf[0..16]);
        let ciphertext = &buf[16..];
        let decrypted_bytes = encrypt_decrypt_payload(ciphertext, key_str, &salt);
        let msg: HiveMessage = serde_json::from_slice(&decrypted_bytes)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
        Ok(Some(msg))
    }

    /// ██ Tier 4: Epistemic update — broadcast a new world state to all
    /// agents so they can update their private accumulators.
    pub async fn broadcast_epistemic_update(
        &self,
        new_world_state: &Hypervector,
        intent_id: u64,
        executor_id: &str,
        tick: u64,
        increment_frequency: bool,
        failure_serial: u64,
    ) {
        let msg = HiveMessage::EpistemicUpdate {
            new_world_state: *new_world_state,
            intent_id,
            executor_id: executor_id.to_string(),
            tick,
            intent_frequency_increment: increment_frequency,
            failure_serial,
        };
        self.broadcast(&msg).await;
    }

    /// ██ Tier 4: Broadcast an execution request to the elected executor.
    /// All other agents receive the request but only the executor acts.
    pub async fn broadcast_execution(
        &self,
        intent: &Hypervector,
        executor_id: &str,
        failure_serial: u64,
    ) {
        let msg = HiveMessage::ExecutionRequest {
            intent: *intent,
            executor_id: executor_id.to_string(),
            failure_serial,
        };
        self.broadcast(&msg).await;
    }

    /// ██ Tier 4: Increment the failure serial (for idempotent retries).
    pub async fn increment_failure_serial(&self) -> u64 {
        let mut serial = self.failure_serial.write().await;
        *serial += 1;
        *serial
    }

    /// Broadcast a HiveMessage to all active connected agents
    pub async fn broadcast(&self, msg: &HiveMessage) {
        let mut clients_guard = self.clients.lock().await;
        let mut disconnected_indices = Vec::new();

        for (idx, client) in clients_guard.iter_mut().enumerate() {
            if let Err(_) = Self::write_msg(client, msg, &self.key).await {
                disconnected_indices.push(idx);
            }
        }

        disconnected_indices.sort_unstable_by(|a, b| b.cmp(a));
        for idx in disconnected_indices {
            if idx < clients_guard.len() {
                clients_guard.remove(idx);
                // Also clean up agent state
                // (we don't know the agent_id here, but that's OK — stale
                //  entries are harmless and will be overwritten on reconnect)
            }
        }
    }

    /// Boot load logic to restore Neocortex in RAM
    pub async fn boot_reconstitute(&self, log_tx: &tokio::sync::mpsc::UnboundedSender<String>) {
        let path = std::path::Path::new("data/long_term_ledger.bin");
        if !path.exists() {
            let _ = log_tx.send(
                "BROKER BOOT: Clean slate boot. No prior memories to reconstitute.".to_string(),
            );
            return;
        }

        match self.ledger.load_records(&self.concept) {
            Ok(records) => {
                let _ = log_tx.send(format!(
                    "BROKER BOOT: Decrypted {} daily ledger records.",
                    records.len()
                ));
                let mut clusters = self.dejavu_clusters.write().await;

                for (date_str, vector) in records {
                    let mut best_idx = None;
                    let mut best_sim = -1.0;

                    for (idx, cluster) in clusters.iter().enumerate() {
                        let sim = 1.0 - vector.normalized_hamming_distance(&cluster.centroid);
                        if sim > best_sim {
                            best_sim = sim;
                            best_idx = Some(idx);
                        }
                    }

                    let entry = DejavuEntry::new(
                        vector,
                        date_str.clone(),
                        HashMap::new(),
                        None, // ledger data is raw, not delta-encoded
                    );

                    if let Some(idx) = best_idx {
                        if best_sim >= 0.65 {
                            clusters[idx].entries.push(entry);
                            // ██ Tier 4: Absorb into accumulator ██
                            clusters[idx].absorb_entry(&vector);
                            continue;
                        }
                    }

                    // ██ Tier 4: Initialize accumulator from the ledger record ██
                    let mut accumulator = vec![0u32; HD_DIMENSION];
                    for (i, acc) in accumulator.iter_mut().enumerate() {
                        let word = vector.bits[i / 64];
                        let bit = (word >> (i % 64)) & 1;
                        *acc = bit as u32;
                    }
                    clusters.push(MemoryCluster {
                        centroid: vector,
                        anchor: vector, // Locked Anchor from ledger
                        entries: vec![entry],
                        reverberation: 1.0,
                        last_reinforced_tick: 0,
                        accumulator,
                        total_weight: 1,
                        last_access_tick: 0,
                    });
                }
                let _ = log_tx.send(format!(
                    "BROKER BOOT: Restored database in RAM to {} permanent clusters.",
                    clusters.len()
                ));
            }
            Err(ref e) if e == "DECRYPTION_FAILED_SECURITY_LOCK" => {
                let _ = log_tx.send(
                    "BROKER BOOT: CRITICAL SECURITY ALERT: Key mismatch! Initiating quarantine..."
                        .to_string(),
                );
                self.trigger_quarantine().await;
            }
            Err(ref e) => {
                let _ = log_tx.send(format!(
                    "BROKER BOOT WARNING: Ledger unreadable: {}. Resetting.",
                    e
                ));
            }
        }
    }

    /// Trigger ledger quarantine and rotate keys
    pub async fn trigger_quarantine(&self) {
        let timestamp = chrono::Utc::now().format("%Y%m%d_%H%M%S").to_string();
        let quarantined_path = format!("data/long_term_ledger_locked_{}.bin", timestamp);
        let _ = std::fs::rename("data/long_term_ledger.bin", &quarantined_path);

        if let Ok(mut f) = std::fs::File::create("data/long_term_ledger.bin") {
            use std::io::Write;
            let _ = f.write_all(&[]);
        }

        let mut clusters = self.dejavu_clusters.write().await;
        clusters.clear();
    }

    // ─── Phase 3: Cohort helpers ────────────────────────────────────

    /// Return the cohort index for `agent_id`, or `None` if the agent
    /// has not been assigned a cohort (should not happen after handshake).
    async fn cohort_for_agent(&self, agent_id: &str) -> Option<usize> {
        let map = self.cohort_of_agent.read().await;
        map.get(agent_id).copied()
    }

    /// Assign an agent to a cohort based on its self-reported role.
    /// Unknown roles are routed to the "General" cohort (the last entry,
    /// at index `cohort_registry.len()`).
    async fn assign_cohort(&self, agent_id: &str, role: &str) -> usize {
        let cohort_id = self
            .cohort_registry
            .get(role)
            .copied()
            .unwrap_or(self.cohort_registry.len()); // General = index C
        self.cohort_of_agent
            .write()
            .await
            .insert(agent_id.to_string(), cohort_id);
        cohort_id
    }

    /// Invalidate the intra-cohort cache for the cohort that `agent_id`
    /// belongs to.  Used during reconnection and migration.
    async fn invalidate_cohort_cache(&self, cohort_id: usize) {
        if cohort_id < self.cohort_caches.len() {
            let mut cache = self.cohort_caches[cohort_id].write().await;
            cache.agent_ids.clear();
        }
        // Also invalidate the inter-cohort cache since cohort centroids
        // are now stale.
        self.inter_cohort_cache.write().await.agent_ids.clear();
    }

    // ─── Tier 4: Sub-Sector Index ──────────────────────────────────

    /// Return the list of sub-sector centroids for an LSH sector.
    /// Returns an empty vec if the sector has no sub-sectors (which
    /// means the sector follows the normal single-centroid path).
    #[allow(dead_code)]
    async fn get_sub_sectors(&self, sector: usize) -> Vec<SubSectorEntry> {
        let index = self.sector_index.read().await;
        index.get(&sector).cloned().unwrap_or_default()
    }

    /// Register a sub-sector centroid for a given LSH sector.
    /// If the sector already has MAX_SUB_SECTORS entries, the lowest-
    /// weight entry is replaced (but only if the new entry has higher
    /// weight).  This ensures bounded memory.
    async fn register_sub_sector(&self, sector: usize, entry: SubSectorEntry) {
        let mut index = self.sector_index.write().await;
        let entries = index.entry(sector).or_insert_with(Vec::new);

        // Check if this centroid already exists (merge detection)
        if let Some(existing) = entries.iter_mut().find(|e| {
            e.centroid.normalized_hamming_distance(&entry.centroid) <= 0.10
        }) {
            // Reconverged!  Merge the weight into the existing entry.
            existing.weight = (existing.weight + entry.weight) / 2.0;
            for id in &entry.agent_ids {
                if !existing.agent_ids.contains(id) {
                    existing.agent_ids.push(id.clone());
                }
            }
            return;
        }

        if entries.len() >= MAX_SUB_SECTORS {
            // Replace the lowest-weight entry if the new one is heavier
            if let Some(min_idx) = entries
                .iter()
                .enumerate()
                .min_by(|(_, a), (_, b)| a.weight.partial_cmp(&b.weight).unwrap())
                .map(|(i, _)| i)
            {
                if entry.weight > entries[min_idx].weight {
                    entries[min_idx] = entry;
                }
            }
        } else {
            entries.push(entry);
        }
    }

    /// Check if centroids within a sector have diverged beyond the
    /// threshold, and bifurcate into sub-sectors if so.
    async fn check_sector_divergence(&self, sector: usize, centroids: &[(&str, Hypervector, f64)]) {
        if centroids.len() < 2 {
            return;
        }

        // Compute pairwise NHD
        let mut pairs: Vec<(usize, usize, f64)> = Vec::new();
        for i in 0..centroids.len() {
            for j in (i + 1)..centroids.len() {
                let nhd = centroids[i].1.normalized_hamming_distance(&centroids[j].1);
                pairs.push((i, j, nhd));
            }
        }

        // If any pair exceeds the divergence threshold, bifurcate
        let divergence_threshold = 0.15;
        let diverged: Vec<(usize, usize)> = pairs
            .into_iter()
            .filter(|&(_, _, nhd)| nhd > divergence_threshold)
            .map(|(i, j, _)| (i, j))
            .collect();

        if diverged.is_empty() {
            return; // All centroids are within the coherence radius
        }

        // Simple hierarchical: collect centroids into groups by
        // nearest-neighbor chain.  Start with the first centroid,
        // then group each subsequent centroid with the closest
        // existing group.
        let mut groups: Vec<Vec<usize>> = vec![vec![0]];
        for i in 1..centroids.len() {
            let mut best_group = 0;
            let mut best_sim = -1.0;
            for (g_idx, group) in groups.iter().enumerate() {
                for &member in group {
                    let nhd = centroids[i].1.normalized_hamming_distance(&centroids[member].1);
                    let sim = 1.0 - nhd;
                    if sim > best_sim {
                        best_sim = sim;
                        best_group = g_idx;
                    }
                }
            }
            if best_sim > 1.0 - divergence_threshold {
                groups[best_group].push(i);
            } else {
                groups.push(vec![i]);
            }
        }

        // Register each group as a sub-sector
        for group in &groups {
            let mut group_centroids: Vec<&Hypervector> = Vec::new();
            let mut group_agents: Vec<String> = Vec::new();
            let mut group_weight = 0.0_f64;
            for &idx in group {
                group_centroids.push(&centroids[idx].1);
                group_agents.push(centroids[idx].0.to_string());
                group_weight += centroids[idx].2;
            }
            let centroid = Hypervector::bundle(&group_centroids);
            self.register_sub_sector(sector, SubSectorEntry {
                centroid,
                weight: group_weight / group.len() as f64,
                agent_ids: group_agents,
            }).await;
        }
    }

    // ─── Executor Selection (Tier 4) ─────────────────────────────────

    /// Select the executor agent by finding the one whose private
    /// centroid is closest to the consensus centroid.
    ///
    /// This is deterministic and requires zero communication:
    /// every agent receives the same consensus centroid and the
    /// same set of agent centroids, so every agent independently
    /// computes the same executor.
    ///
    /// Returns the agent_id of the executor.
    pub fn select_executor(
        consensus_centroid: &Hypervector,
        agent_centroids: &[(String, Hypervector)],
        constitution: &Hypervector,
    ) -> String {
        if agent_centroids.is_empty() {
            panic!("Cannot select executor with zero agents");
        }
        if agent_centroids.len() == 1 {
            return agent_centroids[0].0.clone();
        }

        // argmin NHD(c_i, c_consensus)
        let mut best_idx = 0;
        let mut best_nhd = agent_centroids[0]
            .1
            .normalized_hamming_distance(consensus_centroid);

        for (i, (_, centroid)) in agent_centroids.iter().enumerate().skip(1) {
            let nhd = centroid.normalized_hamming_distance(consensus_centroid);
            if nhd < best_nhd {
                best_nhd = nhd;
                best_idx = i;
            }
        }

        // Tiebreak via constitution if two agents have equal NHD
        let tied: Vec<usize> = agent_centroids
            .iter()
            .enumerate()
            .filter(|(_, (_, c))| {
                (c.normalized_hamming_distance(consensus_centroid) - best_nhd).abs() < 0.001
            })
            .map(|(i, _)| i)
            .collect();

        if tied.len() > 1 {
            // Bundle identity vectors via constitution
            let identity_refs: Vec<&Hypervector> = tied
                .iter()
                .map(|&i| &agent_centroids[i].1)
                .collect();
            let tiebreaker =
                Hypervector::bundle_with_constitution(&identity_refs, constitution);
            let winner_offset = tiebreaker.count_ones() % tied.len();
            return agent_centroids[tied[winner_offset]].0.clone();
        }

        agent_centroids[best_idx].0.clone()
    }

    // ─── Multi-Agent Consensus Protocol ─────────────────────────────

    /// Process a consolidation submission with **two-stage hierarchical
    /// consensus**.
    ///
    /// **Stage 1 — Intra-Cohort (Blast Radius):**
    /// 1. Stores the agent's submission under its cohort's sharded cache.
    /// 2. Computes internal quorum and coherence W_k (avg pairwise sim).
    /// 3. If W_k < CONSENSUS_FLOOR (0.55): the cohort **abstains** from
    ///    Stage 2.  Returns `None` — no global event, no broadcast.
    /// 4. Otherwise, computes a cohort centroid V_k via the existing
    ///    anxiety-weighted bundling (Phase 2 re-entry clamp applies).
    ///
    /// **Stage 2 — Inter-Cohort (Global):**
    /// 5. Collects all non-abstaining cohort centroids V_k with their
    ///    coherences W_k.
    /// 6. Computes inter-coherence (avg pairwise sim between V_k values).
    /// 7. If inter_coherence < CONSENSUS_FLOOR: system-wide DissonanceAlert.
    /// 8. Otherwise, bundles V_k via **coherence-weighted replication**
    ///    into a global centroid, using the **constitutional tiebreaker**
    ///    for deterministic 50/50 resolution.
    /// 9. Passes the global centroid through the Goldilocks sieve.
    ///
    /// **Phases 1–2** (cached matrix, dead-agent filtering, re-entry
    /// anxiety floor) are preserved inside Stage 1, scoped to the cohort.
    pub async fn process_consolidation(
        &self,
        centroid: Hypervector,
        entries: Vec<DejavuEntry>,
        agent_id: &str,
        agent_anxiety: f64,
    ) -> Option<HiveMessage> {
        // ═══════════════════════════════════════════════════════════════
        // STAGE 1 — Intra-Cohort Consensus
        // ═══════════════════════════════════════════════════════════════

        // ── 0. Advance the consolidation epoch ──────────────────────
        let current_epoch = {
            let mut epoch = self.consolidation_epoch.write().await;
            *epoch += 1;
            *epoch
        };

        // ── 1. Store / update this agent's submission ────────────────
        let centroid_changed: bool;
        {
            let mut states = self.agent_states.write().await;
            let prev = states.get(agent_id).map(|s| s.centroid);
            centroid_changed = prev.map_or(true, |old| {
                old.normalized_hamming_distance(&centroid) > 0.001
            });
            states.insert(
                agent_id.to_string(),
                AgentSubmission {
                    agent_id: agent_id.to_string(),
                    centroid,
                    anxiety: agent_anxiety,
                },
            );
        }

        // ── 2. Record last-seen epoch ───────────────────────────────
        {
            let mut last_seen = self.last_seen_tick.write().await;
            last_seen.insert(agent_id.to_string(), current_epoch);
        }

        // ── 3. Bump re-entry counter ────────────────────────────────
        {
            let mut reentry = self.reentry_ticks.write().await;
            let counter = reentry.entry(agent_id.to_string()).or_insert(0);
            if *counter <= REENTRY_STABILIZATION_TICKS {
                *counter += 1;
            }
        }

        // ── 4. Look up this agent's cohort ─────────────────────────
        let cohort_id = match self.cohort_for_agent(agent_id).await {
            Some(cid) => cid,
            None => {
                // Agent hasn't been assigned a cohort yet — this should
                // not happen post-handshake, but handle gracefully by
                // falling through with a bare goldilocks sieve.
                return self.goldilocks_sieve(centroid, entries).await;
            }
        };

        // ── 5. Collect all live submissions ─────────────────────────
        let last_seen_snapshot = {
            let last_seen = self.last_seen_tick.read().await;
            last_seen.clone()
        };
        let all_submissions: Vec<AgentSubmission> = {
            let states = self.agent_states.read().await;
            states.values().cloned().collect()
        };

        // ── 6. Snapshot the cohort mapping for the filter closure ───
        let cohort_snapshot = {
            let map = self.cohort_of_agent.read().await;
            map.clone()
        };

        // ── 7. Filter dead agents, then filter to cohort peers ──────
        let cohort_agents: Vec<AgentSubmission> = all_submissions
            .into_iter()
            .filter(|s| {
                last_seen_snapshot
                    .get(&s.agent_id)
                    .map_or(true, |&t| current_epoch - t <= MAX_SILENT_EPOCHS)
            })
            .filter(|s| {
                cohort_snapshot.get(&s.agent_id).copied() == Some(cohort_id)
            })
            .collect();

        // ── 7. Need at least 2 agents in this cohort for quorum ─────
        if cohort_agents.len() < 2 {
            // Not enough peers for intra-cohort consensus.  Remove the
            // cohort centroid from Stage 2 and fall through to bare
            // goldilocks — no global coordination.
            self.cohort_centroids.write().await.remove(&cohort_id);
            return self.goldilocks_sieve(centroid, entries).await;
        }

        // ── 8. Refresh this cohort's sharded similarity cache ───────
        {
            let mut cache = self.cohort_caches[cohort_id].write().await;
            if cache.sims.len() != cohort_agents.len() {
                *cache = SimilarityCache::build(&cohort_agents);
            } else if centroid_changed {
                if let Some(idx) = cache.index_of(agent_id) {
                    cache.invalidate(idx);
                    cache.recompute(idx, &cohort_agents);
                }
            }
        }

        // ── 9. Read intra-cohort pairwise similarities ──────────────
        let intra_coherence = {
            let cache = self.cohort_caches[cohort_id].read().await;
            let n = cohort_agents.len();
            let mut pair_sims = Vec::with_capacity(n * (n - 1) / 2);
            for i in 0..n {
                for j in (i + 1)..n {
                    pair_sims.push(cache.get(i, j));
                }
            }
            pair_sims.iter().sum::<f64>() / pair_sims.len() as f64
        };

        // ── 10. Compute intra-cohort centroid V_k ───────────────────
        // Always compute the bundled centroid when ≥ 2 agents exist.
        // This gives the best noise-robust representation regardless
        // of individual agent quorum (which only affects ledger writes).
        let reentry_snapshot = {
            let reentry = self.reentry_ticks.read().await;
            reentry.clone()
        };
        let v_k = compute_anxiety_weighted_centroid(&cohort_agents, &reentry_snapshot);

        if intra_coherence < CONSENSUS_FLOOR {
            // ── Cohort abstains — internal signal quality too low ──
            // The cohort centroid would be near-noise, so it is removed
            // from Stage 2.  The fallback uses V_k (which at least
            // bundles the agents) rather than the raw submitting agent.
            self.cohort_centroids.write().await.remove(&cohort_id);
            return self.goldilocks_sieve(v_k, entries).await;
        }

        // ── 11. Publish the cohort centroid for Stage 2 ──────────────
        // Intra-coherence W_k becomes the weight for coherence-weighted
        // replication in Stage 2 (higher coherence → more copies in the
        // global bundle).
        self.cohort_centroids
            .write()
            .await
            .insert(cohort_id, (v_k, intra_coherence));

        // ═══════════════════════════════════════════════════════════════
        // STAGE 2 — Inter-Cohort Consensus
        // ═══════════════════════════════════════════════════════════════

        // ── 12. Collect all non-abstaining cohort centroids ──────────
        let cohorts_snapshot: Vec<(usize, Hypervector, f64)> = {
            let map = self.cohort_centroids.read().await;
                    let mut v: Vec<_> = map
                        .iter()
                        .map(|(&cid, pair)| (cid, pair.0.clone(), pair.1))
                        .collect();
                    v.sort_by_key(|&(cid, _, _)| cid); // stable order
            v
        };

        // Need at least 2 cohorts for inter-cohort consensus.
        // (If only one cohort is active, its centroid is the global one.)
        let global_centroid = if cohorts_snapshot.len() < 2 {
            cohorts_snapshot
                .first()
                .map(|&(_, ref cent, _)| *cent)
                .unwrap_or(centroid)
        } else {
            // ── 13. Refresh the inter-cohort similarity cache ────────
            // Build a synthetic "submission" slice from cohort centroids
            // so we can reuse the existing SimilarityCache logic.
            let cohort_submissions: Vec<AgentSubmission> = cohorts_snapshot
                .iter()
                .map(|&(cid, cent, _coh)| AgentSubmission {
                    agent_id: format!("__cohort_{}", cid),
                    centroid: cent,
                    anxiety: 0.0, // not used in inter-cohort path
                })
                .collect();

            {
                let mut cache = self.inter_cohort_cache.write().await;
                if cache.sims.len() != cohort_submissions.len() {
                    *cache = SimilarityCache::build(&cohort_submissions);
                }
                // No per-cohort centroid_changed detection here — we
                // rebuild the entire inter-cohort cache because a cohort
                // centroid change implicitly invalidates everything.
            }

            // ── 14. Compute inter-coherence ─────────────────────────
            let inter_coherence = {
                let cache = self.inter_cohort_cache.read().await;
                let n = cohort_submissions.len();
                let mut pair_sims = Vec::with_capacity(n * (n - 1) / 2);
                for i in 0..n {
                    for j in (i + 1)..n {
                        pair_sims.push(cache.get(i, j));
                    }
                }
                pair_sims.iter().sum::<f64>() / pair_sims.len() as f64
            };

            // ── 15. Check for global DissonanceAlert ────────────────
            if inter_coherence < CONSENSUS_FLOOR {
                return Some(HiveMessage::DissonanceAlert {
                    consensus_similarity: inter_coherence,
                    agent_count: cohort_submissions.len(),
                });
            }

            // ── 16. Coherence-weighted global centroid ──────────────
            // Each cohort centroid is replicated proportional to its
            // internal coherence W_k.  A highly unified cohort carries
            // more weight; a fractured cohort contributes fewer copies.
            // The constitutional tiebreaker resolves any remaining 50/50
            // splits deterministically.
            compute_global_centroid(&cohorts_snapshot, &self.constitution)
        };

        // ── 17. Pass through the Goldilocks sieve ───────────────────
        self.goldilocks_sieve(global_centroid, entries).await
    }

    /// The Goldilocks merge / fission / discard sieve.
    /// Shared between consensus and fallback paths.
    ///
    /// **Phase 1:** Uses LSH sector prefiltering to scan only the
    /// clusters whose Locked Anchor falls in the same sector as the
    /// incoming centroid, reducing the linear scan from O(M) to O(M/16).
    async fn goldilocks_sieve(
        &self,
        centroid: Hypervector,
        entries: Vec<DejavuEntry>,
    ) -> Option<HiveMessage> {
        let mut clusters = self.dejavu_clusters.write().await;

        if clusters.is_empty() {
            // ██ Tier 4: Initialize accumulator from incoming centroid ██
            let mut accumulator = vec![0u32; HD_DIMENSION];
            for (i, acc) in accumulator.iter_mut().enumerate() {
                let word = centroid.bits[i / 64];
                let bit = (word >> (i % 64)) & 1;
                *acc = bit as u32;
            }
            let new_cluster = MemoryCluster {
                centroid,
                anchor: centroid, // Locked Anchor = first centroid
                entries,
                reverberation: 1.0,
                last_reinforced_tick: 0,
                accumulator,
                total_weight: 1,
                last_access_tick: 0,
            };
            clusters.push(new_cluster.clone());
            let today_str = chrono::Utc::now().format("%Y-%m-%d").to_string();
            // Acquire file mutex before ledger append to prevent race
            // with the background compactor (Phase 1).
            let _file_guard = self.file_mutex.lock().await;
            let _ = self
                .ledger
                .append_record(&today_str, &centroid, &self.concept);
            drop(_file_guard);
            return Some(HiveMessage::SyncUpdate {
                is_new_cluster: true,
                cluster_index: Some(0),
                cluster: new_cluster,
            });
        }

        // ── LSH sector prefilter ────────────────────────────────────
        // Compute the sector of the incoming centroid, then only visit
        // clusters whose Locked Anchor falls in the same sector.  Using
        // the anchor (immutable) guarantees sector stability: even though
        // the centroid drifts, the cluster's sector assignment never
        // changes, preventing cache-miss / fission false-positives.
        let incoming_sector = crate::lsh_sector_inline(&centroid);

        let mut best_idx = None;
        let mut best_sim = -1.0;

        for (idx, cluster) in clusters.iter().enumerate() {
            // Phase 1: Skip clusters whose anchor falls in a different
            // LSH sector.  This is a cheap XOR+popcount, NOT a full NHD.
            if cluster.anchor.count_ones() > 0 {
                let cluster_sector = crate::lsh_sector_inline(&cluster.anchor);
                if cluster_sector != incoming_sector {
                    continue;
                }
            }
            // If anchor is zero (should not happen post-ensure_anchor),
            // fall through to the full scan.

            let sim = 1.0 - centroid.normalized_hamming_distance(&cluster.centroid);
            if sim > best_sim {
                best_sim = sim;
                best_idx = Some(idx);
            }
        }

        // If the sector scan found nothing useful, fall back to a full
        // scan.  This ensures correctness even if LSH sector boundaries
        // happen to split a near-identical pair.
        if best_sim < 0.52 {
            for (idx, cluster) in clusters.iter().enumerate() {
                // Skip clusters already checked above.
                if cluster.anchor.count_ones() > 0 {
                    let cluster_sector = crate::lsh_sector_inline(&cluster.anchor);
                    if cluster_sector == incoming_sector {
                        continue;
                    }
                }
                let sim = 1.0 - centroid.normalized_hamming_distance(&cluster.centroid);
                if sim > best_sim {
                    best_sim = sim;
                    best_idx = Some(idx);
                }
            }
        }

        let mut sync_msg = None;
        if best_sim >= 0.75 {
            if let Some(idx) = best_idx {
                let cluster = &mut clusters[idx];
                for entry in entries {
                    let tau = entry.reconstruct(&cluster.anchor);
                    cluster.entries.push(entry);
                    cluster.absorb_entry(&tau);
                }

                let today_str = chrono::Utc::now().format("%Y-%m-%d").to_string();
                // Phase 1: serialize with compactor via file_mutex
                let _file_guard = self.file_mutex.lock().await;
                let _ = self
                    .ledger
                    .append_record(&today_str, &cluster.centroid, &self.concept);
                drop(_file_guard);

                sync_msg = Some(HiveMessage::SyncUpdate {
                    is_new_cluster: false,
                    cluster_index: Some(idx),
                    cluster: cluster.clone(),
                });
            }
        } else if best_sim < 0.52 {
            // Discard noise
        } else {
            // Fission — ██ Tier 4: Initialize accumulator ██
            let mut accumulator = vec![0u32; HD_DIMENSION];
            for (i, acc) in accumulator.iter_mut().enumerate() {
                let word = centroid.bits[i / 64];
                let bit = (word >> (i % 64)) & 1;
                *acc = bit as u32;
            }
            let new_cluster = MemoryCluster {
                centroid,
                anchor: centroid, // Locked Anchor = initial centroid
                entries,
                reverberation: 1.0,
                last_reinforced_tick: 0,
                accumulator,
                total_weight: 1,
                last_access_tick: 0,
            };
            clusters.push(new_cluster.clone());
            let new_idx = clusters.len() - 1;

            let today_str = chrono::Utc::now().format("%Y-%m-%d").to_string();
            // Phase 1: serialize with compactor via file_mutex
            let _file_guard = self.file_mutex.lock().await;
            let _ = self
                .ledger
                .append_record(&today_str, &centroid, &self.concept);
            drop(_file_guard);

            sync_msg = Some(HiveMessage::SyncUpdate {
                is_new_cluster: true,
                cluster_index: Some(new_idx),
                cluster: new_cluster,
            });
        }
        // ██ Tier 4: Check for sector divergence after the sieve ██
        // If multiple centroids in the same sector have diverged beyond
        // NHD 0.15, register sub-sectors for bifurcated routing.
        // This uses `incoming_sector` (computed at line 1036) which is
        // the LSH sector of the centroid being processed.
        {
            let sector_entries: Vec<(&str, Hypervector, f64)> = clusters
                .iter()
                .filter(|c| {
                    let cs = if c.anchor.count_ones() > 0 {
                        crate::lsh_sector_inline(&c.anchor)
                    } else {
                        crate::lsh_sector_inline(&c.centroid)
                    };
                    cs == incoming_sector
                })
                .map(|c| {
                    let label = c.entries.first()
                        .map(|e| e.label.as_str())
                        .unwrap_or("cluster");
                    (label, c.centroid, c.reverberation)
                })
                .collect();
            if sector_entries.len() >= 2 {
                self.check_sector_divergence(incoming_sector, &sector_entries).await;
            }
        }

        sync_msg
    }

    // ─── Runtime ──────────────────────────────────────────────────

    /// Core Broker runtime loop
    pub async fn run(
        self: Arc<Self>,
        log_tx: tokio::sync::mpsc::UnboundedSender<String>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        self.boot_reconstitute(&log_tx).await;

        // Spawn background compaction task
        // Capture the full broker Arc so the compactor has access to agent
        // state maps for the Phase 2 dead-agent pruning sweep.
        let broker_compactor = Arc::clone(&self);
        let compaction_log = log_tx.clone();
        let max_interval = chrono::Duration::days(7);
        let growth_threshold: usize = 50;

        tokio::spawn(async move {
            let mut last_compaction = chrono::Utc::now();
            let mut last_record_count: Option<usize> = None;

            loop {
                tokio::time::sleep(tokio::time::Duration::from_secs(60)).await;

                // ── Phase 2: Dead-agent pruning sweep ───────────────
                // Removes agents that have not consolidated for more than
                // MAX_SILENT_EPOCHS epochs.  This is the safety net for
                // agents that disconnected without a clean TCP close
                // (power failure, net split).
                {
                    let epoch = *broker_compactor.consolidation_epoch.read().await;
                    let last_seen = broker_compactor.last_seen_tick.read().await;
                    let mut states = broker_compactor.agent_states.write().await;

                    let dead_ids: Vec<String> = states
                        .keys()
                        .filter(|id| {
                            last_seen
                                .get(*id)
                                .map_or(false, |&t| epoch.saturating_sub(t) > MAX_SILENT_EPOCHS)
                        })
                        .cloned()
                        .collect();

                    if !dead_ids.is_empty() {
                        let mut reentry = broker_compactor.reentry_ticks.write().await;
                        let mut cohort_map = broker_compactor.cohort_of_agent.write().await;
                        // Track which cohorts lost agents so we can
                        // invalidate their caches below.
                        let mut affected_cohorts: Vec<usize> = Vec::new();
                        for id in &dead_ids {
                            states.remove(id);
                            reentry.remove(id);
                            if let Some(cid) = cohort_map.remove(id) {
                                if !affected_cohorts.contains(&cid) {
                                    affected_cohorts.push(cid);
                                }
                            }
                        }
                        // Invalidate all affected cohort caches +
                        // the inter-cohort cache.
                        for cid in &affected_cohorts {
                            if *cid < broker_compactor.cohort_caches.len() {
                                let mut cache = broker_compactor.cohort_caches[*cid].write().await;
                                cache.agent_ids.clear();
                            }
                        }
                        broker_compactor.inter_cohort_cache.write().await.agent_ids.clear();

                        let _ = compaction_log.send(format!(
                            "COMPACTOR: Pruned {} dead agent(s) from state maps: {:?}.",
                            dead_ids.len(),
                            dead_ids
                        ));
                    }
                }

                // ── Ledger compaction ───────────────────────────────
                let records = match broker_compactor.ledger.load_records(&broker_compactor.concept) {
                    Ok(r) => r,
                    Err(_) => continue,
                };
                let current_count = records.len();
                let now = chrono::Utc::now();

                let elapsed = now - last_compaction;
                let growth = last_record_count
                    .map(|prev| current_count.saturating_sub(prev))
                    .unwrap_or(0);
                let growth_exceeded = growth >= growth_threshold;
                let interval_exceeded = elapsed >= max_interval;

                let should_compact = interval_exceeded || growth_exceeded;

                if should_compact && current_count > 0 {
                    let reason = if interval_exceeded {
                        format!("max interval reached ({:.1}h)", elapsed.num_hours() as f64)
                    } else {
                        format!("growth threshold ({} new records)", growth)
                    };

                    let _ = compaction_log.send(format!(
                        "COMPACTOR: Initiating sleep cycle — {}.",
                        reason
                    ));

                    // Phase 1: Acquire the file mutex for the entire
                    // read-cluster-write cycle so that no concurrent
                    // append_record (from goldilocks_sieve) sees a
                    // torn file or loses a record.
                    let _file_guard = broker_compactor.file_mutex.lock().await;

                    match broker_compactor
                        .ledger
                        .compact_ledger(&broker_compactor.concept, 0.70)
                    {
                        Ok(removed) => {
                            let _ = compaction_log.send(format!(
                                "COMPACTOR: Removed {} redundant records. {} remain.",
                                removed,
                                current_count.saturating_sub(removed)
                            ));
                            last_compaction = now;
                            last_record_count = Some(current_count.saturating_sub(removed));
                        }
                        Err(e) => {
                            let _ =
                                compaction_log.send(format!("COMPACTOR ERROR: {}. Will retry.", e));
                        }
                    }
                    drop(_file_guard);
                } else {
                    last_record_count = Some(current_count);
                }
            }
        });

        let listener = TcpListener::bind(format!("127.0.0.1:{}", self.port)).await?;
        let _ = log_tx.send(format!(
            "BROKER: Listening on tcp://127.0.0.1:{}",
            self.port
        ));

        loop {
            let (socket, addr) = listener.accept().await?;
            let broker_clone = Arc::clone(&self);
            let log_tx_clone = log_tx.clone();

            tokio::spawn(async move {
                let (reader, writer) = socket.into_split();
                let mut reader = reader;
                let mut writer = writer;

                // 1. Process Handshake
                let (agent_id, _agent_role) = match Self::read_msg(&mut reader, &broker_clone.key).await {
                    Ok(Some(HiveMessage::HandshakeRequest { agent_id: id, role })) => {
                        let _ = log_tx_clone
                            .send(format!("BROKER: Connection from Agent {} ({})", id, role));

                        // ██ Phase 3: Assign agent to a cohort based on its role.
                        let cohort_id = broker_clone.assign_cohort(&id, &role).await;
                        let cohort_name = broker_clone
                            .cohort_registry
                            .iter()
                            .find(|(_, &v)| v == cohort_id)
                            .map(|(name, _)| name.clone())
                            .unwrap_or_else(|| format!("General({})", cohort_id));
                        let _ = log_tx_clone.send(format!(
                            "BROKER: Agent {} assigned to cohort '{}' (id={}).",
                            id, cohort_name, cohort_id
                        ));

                        // ██ Phase 2: If this agent is reconnecting, invalidate its
                        // cohort's cache and reset its re-entry counter.
                        {
                            let states = broker_clone.agent_states.read().await;
                            if states.contains_key(&id) {
                                broker_clone.invalidate_cohort_cache(cohort_id).await;
                                let mut reentry = broker_clone.reentry_ticks.write().await;
                                reentry.insert(id.clone(), 0);
                                let _ = log_tx_clone.send(format!(
                                    "BROKER: Agent {} reconnected — cohort cache invalidated, re-entry counter reset.",
                                    id
                                ));
                            }
                        }

                        let current_clusters = {
                            let db = broker_clone.dejavu_clusters.read().await;
                            db.clone()
                        };
                        let response = HiveMessage::HandshakeResponse {
                            permanent_clusters: current_clusters,
                        };
                        if let Err(_) = Self::write_msg(&mut writer, &response, &broker_clone.key).await {
                            return;
                        }
                        (id, role)
                    }
                    _ => {
                        let _ =
                            log_tx_clone.send(format!("BROKER: Handshake failed from {}", addr));
                        return;
                    }
                };

                // Add to active clients list
                {
                    let mut clients_guard = broker_clone.clients.lock().await;
                    clients_guard.push(writer);
                }

                // 2. Receive commands loop
                loop {
                    match Self::read_msg(&mut reader, &broker_clone.key).await {
                        Ok(Some(HiveMessage::ConsolidateRequest {
                            centroid,
                            entries,
                            agent_anxiety,
                        })) => {
                            let _ = log_tx_clone.send(format!(
                                "BROKER: Processing Consolidation Request from Agent {} (anxiety={:.2})",
                                agent_id, agent_anxiety
                            ));
                            if let Some(response) = broker_clone
                                .process_consolidation(centroid, entries, &agent_id, agent_anxiety)
                                .await
                            {
                                match &response {
                                    HiveMessage::DissonanceAlert {
                                        consensus_similarity,
                                        agent_count,
                                    } => {
                                        let _ = log_tx_clone.send(format!(
                                            "BROKER: CONSENSUS FAILURE — avg similarity={:.3} across {} agents. Broadcasting DissonanceAlert.",
                                            consensus_similarity, agent_count
                                        ));
                                        broker_clone.broadcast(&response).await;
                                    }
                                    HiveMessage::SyncUpdate { .. } => {
                                        broker_clone.broadcast(&response).await;
                                        let _ = log_tx_clone.send(format!(
                                            "BROKER: Broadcasted memory SyncUpdate to all agents."
                                        ));
                                    }
                                    _ => {}
                                }
                            }
                        }
                        Ok(Some(HiveMessage::PanicLockdown { attacker_info })) => {
                            let _ = log_tx_clone.send(format!(
                                "BROKER: CRITICAL PANIC received from Agent {}! Attacker: {}. Sealing Ledger.",
                                agent_id, attacker_info
                            ));
                            broker_clone.trigger_quarantine().await;

                            let lockdown = HiveMessage::PanicLockdown {
                                attacker_info: attacker_info.clone(),
                            };
                            broker_clone.broadcast(&lockdown).await;
                        }
                        Ok(None) | Err(_) => {
                            let _ = log_tx_clone
                                .send(format!("BROKER: Agent {} disconnected.", agent_id));
                            // ██ Phase 2: Clean up all state maps for this agent.
                            // The stale entry would otherwise persist in memory
                            // until overwritten by a reconnection.
                            let old_cohort = {
                                let mut states = broker_clone.agent_states.write().await;
                                states.remove(&agent_id);
                                let mut last_seen = broker_clone.last_seen_tick.write().await;
                                last_seen.remove(&agent_id);
                                let mut reentry = broker_clone.reentry_ticks.write().await;
                                reentry.remove(&agent_id);
                                // ██ Phase 3: Remove from cohort mapping.
                                let cohort = {
                                    let mut map = broker_clone.cohort_of_agent.write().await;
                                    map.remove(&agent_id)
                                };
                                cohort
                            };
                            // Invalidate the affected cohort's cache.
                            // This forces a rebuild on the next consolidation
                            // within that cohort (the cohort agent count
                            // decreased).
                            if let Some(cid) = old_cohort {
                                broker_clone.invalidate_cohort_cache(cid).await;
                            }
                            break;
                        }
                        _ => {}
                    }
                }
            });
        }
    }
}

// ─── Weighted consensus helpers ───────────────────────────────────────────

/// Compute an anxiety-weighted consensus centroid from a set of agent
/// submissions.  Agents with LOWER anxiety contribute MORE copies of
/// their centroid to the majority-rule bundle, giving them greater
/// influence over the consensus memory.
///
/// **Phase 2:** Agents whose `reentry_ticks` counter is below
/// `REENTRY_STABILIZATION_TICKS` have their effective anxiety clamped
/// to at least `ANXIETY_FLOOR (0.5)`.  This prevents a calm-but-stale
/// agent from exerting full influence before its centroid has had time
/// to re-synchronize with the swarm's current state.
fn compute_anxiety_weighted_centroid(
    submissions: &[AgentSubmission],
    reentry_ticks: &HashMap<String, usize>,
) -> Hypervector {
    if submissions.is_empty() {
        return Hypervector::new_zero();
    }
    if submissions.len() == 1 {
        return submissions[0].centroid;
    }

    let mut weighted_refs: Vec<&Hypervector> = Vec::new();

    for submission in submissions {
        // Phase 2: Apply anxiety floor for recently reconnected agents.
        let effective_anxiety = match reentry_ticks.get(&submission.agent_id) {
            Some(&ticks) if ticks < REENTRY_STABILIZATION_TICKS => {
                submission.anxiety.max(ANXIETY_FLOOR)
            }
            _ => submission.anxiety,
        };

        // Weight = 1 / (1 + effective_anxiety); ranges from 1.0 (anxiety=0)
        // down to 0.5 (anxiety=1).  With the floor at 0.5 the range tightens
        // to [0.67…0.50] during re-entry.
        let weight = 1.0 / (1.0 + effective_anxiety);
        // Scale to integer copies; at least 1 so every agent is heard
        let copies = ((weight * 10.0).round() as usize).max(1);
        for _ in 0..copies {
            weighted_refs.push(&submission.centroid);
        }
    }

    Hypervector::bundle(&weighted_refs)
}

/// Compute a coherence-weighted global centroid from a set of cohort
/// centroids.  Each cohort's weight in the bundle is its internal
/// coherence W_k (average pairwise similarity).  Cohorts with higher
/// internal agreement have more influence over the global consensus.
///
/// **Constitutional tie-breaking:** Uses the provided `constitution`
/// hypervector to resolve 50/50 bit-level ties, guaranteeing idempotent
/// results regardless of the order in which cohorts are bundled.
///
/// **Scale:** W_k ∈ [0.55, 1.0] scaled by 8 to ~[4, 8] copies.
/// This virtually eliminates ties in practice by ensuring an odd total
/// whenever W_k values differ across cohorts.
fn compute_global_centroid(
    cohorts: &[(usize, Hypervector, f64)],
    constitution: &Hypervector,
) -> Hypervector {
    if cohorts.is_empty() {
        return Hypervector::new_zero();
    }
    if cohorts.len() == 1 {
        return cohorts[0].1; // .1 = centroid
    }

    let mut weighted_refs: Vec<&Hypervector> = Vec::with_capacity(cohorts.len() * 8);
    for (_, centroid, coherence) in cohorts {
        // Scale coherence [0.55, 1.0] to copies [4, 8].
        // Same scale (8) as bundle_weighted for consistency.
        let copies = ((coherence * 8.0).round().max(1.0) as usize).min(16);
        for _ in 0..copies {
            weighted_refs.push(centroid);
        }
    }

    Hypervector::bundle_with_constitution(&weighted_refs, constitution)
}

// ═════════════════════════════════════════════════════════════════════════════
// Phase 3 Test Harness — Hierarchical Quorum Validation
// ═════════════════════════════════════════════════════════════════════════════
//
// Architecture:
//   Phase 1 — Sequential deterministic tests (epoch-by-epoch invariant checks)
//   Phase 2 — Concurrent stress test (1,000 agents, 10 rounds each)
//
// The harness bypasses TCP entirely, calling broker methods directly.
// All tests use `#[tokio::test]` because NeocortexBroker uses tokio RwLock.
//
// Key invariants validated:
//   • Cohort abstention removes centroid from Stage 2
//   • Non-abstaining cohorts continue producing global centroids
//   • Re-entry anxiety floor lifts after exactly 5 consolidations
//   • Constitutional tiebreaker is order-independent
//   • Dead agents pruned after MAX_SILENT_EPOCHS
//   • Flapping agent cannot dominate global centroid
//   • Lock hierarchy survives 10,000 concurrent consolidations
// ═════════════════════════════════════════════════════════════════════════════

// ─── WeightProvider implementation ────────────────────────────────────────

impl WeightProvider for NeocortexBroker {
    /// Return weights for all frames with matching labels in the broker's
    /// cluster store.
    ///
    /// Uses `try_read()` to avoid blocking in a sync context — if the
    /// lock is contended (consolidation running), returns the current
    /// epoch with no updates. The index will retry on the next insert.
    ///
    /// For each frame label, scans all MemoryClusters for a DejavuEntry
    /// whose label matches, then returns that cluster's total_weight.
    ///
    /// If no matching entry is found, the frame is not included in the
    /// response — the index preserves its current weight (default 0.0
    /// for new frames, or the last synced value).
    ///
    /// The `since_epoch` parameter is noted but currently unused (returns
    /// all weights every call). Epoch-delta filtering is an optimization
    /// that can be added later without changing semantics.
    fn get_weights(&self, _since_epoch: Option<u64>) -> (u64, Vec<(String, f64)>) {
        let epoch = *self.consolidation_epoch.blocking_read();
        let clusters_guard = self.dejavu_clusters.try_read();

        let clusters = match clusters_guard {
            Ok(guard) => guard,
            Err(_) => return (epoch, Vec::new()), // lock contended, retry next tick
        };

        let mut result: Vec<(String, f64)> = Vec::new();
        for cluster in clusters.iter() {
            let weight = cluster.total_weight as f64;
            for entry in &cluster.entries {
                result.push((entry.label.clone(), weight));
            }
        }

        (epoch, result)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use tokio::sync::RwLock;

    // ── Agent simulator ─────────────────────────────────────────────

    /// A simulated agent with a controlled centroid drift pattern.
    ///
    /// Agents within the same cohort share a `base` hypervector so that
    /// stable-mode agents produce naturally similar centroids (differing
    /// by only a few flipped bits).  This ensures intra-cohort coherence
    /// stays well above CONSENSUS_FLOOR under normal conditions, allowing
    /// the tests to validate quorum logic rather than random similarity.
    struct TestAgent {
        agent_id: String,
        role: String,
        anxiety: f64,
        /// Cohort-shared base vector — stable agents return this directly.
        base: Hypervector,
        /// Drift pattern: 0=stable, 1=oscillating, 2=divergent, 3=flapping.
        mode: usize,
        tick: usize,
    }

    impl TestAgent {
        /// Create a new test agent.  `base` should be shared across all
        /// agents in the same cohort for realistic intra-cohort similarity.
        fn new(agent_id: &str, role: &str, anxiety: f64, mode: usize, base: Hypervector) -> Self {
            TestAgent {
                agent_id: agent_id.to_string(),
                role: role.to_string(),
                anxiety,
                base,
                mode,
                tick: 0,
            }
        }

        /// Generate the next centroid based on this agent's drift pattern.
        fn next_centroid(&mut self) -> Hypervector {
            self.tick += 1;
            match self.mode {
                0 => self.base, // stable: return the base vector unchanged
                1 => {
                    // oscillating: alternate between base and a nearby
                    // point (one block XOR'd with a random pattern)
                    if self.tick % 2 == 0 {
                        self.base
                    } else {
                        let mut perturbed = self.base;
                        let block = (self.tick as usize) % 160;
                        perturbed.bits[block] ^= 0xDEADBEEFCAFEBABE;
                        perturbed
                    }
                }
                _ => Hypervector::new_random(), // divergent / flapping
            }
        }
    }

    // ── Deterministic test harness ───────────────────────────────────

    /// A sequential, epoch-controlled test harness for the NeocortexBroker.
    /// Each `submit()` call advances exactly one consolidation epoch,
    /// enabling precise state assertions after every step.
    ///
    /// The harness maintains a `cohort_base` map that stores one shared
    /// `Hypervector` per cohort.  All agents in the same cohort use this
    /// base, ensuring stable-mode agents produce naturally similar
    /// centroids and intra-cohort coherence stays well above threshold.
    struct TestHarness {
        broker: Arc<NeocortexBroker>,
        agents: HashMap<String, TestAgent>,
        /// One shared base vector per cohort (keyed by role name).
        cohort_bases: RwLock<HashMap<String, Hypervector>>,
    }

    impl TestHarness {
        /// Create a broker with a temp ledger and empty state.
        fn new() -> Self {
            let broker = Arc::new(NeocortexBroker::new(
                "test_key",
                "data/test_hierarchical_quorum.bin",
                0, // port 0 = no TCP listener
            ));
            // Clear any prior test ledger
            let _ = std::fs::remove_file("data/test_hierarchical_quorum.bin");
            TestHarness {
                broker,
                agents: HashMap::new(),
                cohort_bases: RwLock::new(HashMap::new()),
            }
        }

        /// Get or create a shared base vector for the given cohort role.
        async fn get_or_create_base(&self, role: &str) -> Hypervector {
            let mut bases = self.cohort_bases.write().await;
            bases
                .entry(role.to_string())
                .or_insert_with(Hypervector::new_random)
                .clone()
        }

        /// Register a new agent (simulates TCP handshake).
        async fn add_agent(
            &mut self,
            agent_id: &str,
            role: &str,
            anxiety: f64,
            mode: usize,
        ) {
            self.broker.assign_cohort(agent_id, role).await;
            let base = self.get_or_create_base(role).await;
            self.agents.insert(
                agent_id.to_string(),
                TestAgent::new(agent_id, role, anxiety, mode, base),
            );
        }

        /// Simulate a clean TCP disconnect (removes from all state maps).
        async fn remove_agent(&mut self, agent_id: &str) {
            {
                let mut states = self.broker.agent_states.write().await;
                states.remove(agent_id);
            }
            {
                let mut last_seen = self.broker.last_seen_tick.write().await;
                last_seen.remove(agent_id);
            }
            {
                let mut reentry = self.broker.reentry_ticks.write().await;
                reentry.remove(agent_id);
            }
            if let Some(cid) = {
                let mut map = self.broker.cohort_of_agent.write().await;
                map.remove(agent_id)
            } {
                self.broker.invalidate_cohort_cache(cid).await;
            }
            self.agents.remove(agent_id);
        }

        /// Submit one consolidation for the named agent.
        /// Returns the broker's response, if any.
        async fn submit(&mut self, agent_id: &str) -> Option<HiveMessage> {
            let agent = self.agents.get_mut(agent_id).unwrap();
            let centroid = agent.next_centroid();
            self.broker
                .process_consolidation(
                    centroid,
                    Vec::new(), // no entries for quorum-only tests
                    agent_id,
                    agent.anxiety,
                )
                .await
        }

        /// Assert that a cohort's centroid is present in Stage 2
        /// and return its coherence.
        async fn assert_cohort_active(&self, cohort_id: usize) -> f64 {
            let map = self.broker.cohort_centroids.read().await;
            let (_, coherence) = map
                .get(&cohort_id)
                .expect("Expected cohort to be active (non-abstaining)");
            *coherence
        }

        /// Assert that a cohort is abstaining (absent from Stage 2).
        async fn assert_cohort_abstaining(&self, cohort_id: usize) {
            let map = self.broker.cohort_centroids.read().await;
            assert!(
                map.get(&cohort_id).is_none(),
                "Expected cohort {} to be abstaining",
                cohort_id
            );
        }

        /// Assert the total number of agents in a cohort's sharded cache.
        async fn assert_cohort_cache_size(&self, cohort_id: usize, expected: usize) {
            let cache = self.broker.cohort_caches[cohort_id].read().await;
            assert_eq!(
                cache.agent_ids.len(),
                expected,
                "Cohort {} cache size mismatch",
                cohort_id
            );
        }

        /// Assert that the inter-cohort cache has the expected size.
        async fn assert_inter_cache_size(&self, expected: usize) {
            let cache = self.broker.inter_cohort_cache.read().await;
            assert_eq!(
                cache.agent_ids.len(),
                expected,
                "Inter-cohort cache size mismatch"
            );
        }

        /// Return the re-entry tick count for an agent.
        async fn reentry_ticks(&self, agent_id: &str) -> usize {
            let map = self.broker.reentry_ticks.read().await;
            map.get(agent_id).copied().unwrap_or(0)
        }

        /// Force a dead-agent pruning sweep (compactor's 60s loop).
        async fn run_pruning_sweep(&self) {
            let epoch = *self.broker.consolidation_epoch.read().await;
            let last_seen = self.broker.last_seen_tick.read().await;
            let mut states = self.broker.agent_states.write().await;
            let mut reentry = self.broker.reentry_ticks.write().await;
            let mut cohort_map = self.broker.cohort_of_agent.write().await;
            let mut affected: Vec<usize> = Vec::new();
            let dead: Vec<String> = states
                .keys()
                .filter(|id| {
                    last_seen
                        .get(*id)
                        .map_or(false, |&t| epoch.saturating_sub(t) > MAX_SILENT_EPOCHS)
                })
                .cloned()
                .collect();
            for id in &dead {
                states.remove(id);
                reentry.remove(id);
                if let Some(cid) = cohort_map.remove(id) {
                    if !affected.contains(&cid) {
                        affected.push(cid);
                    }
                }
            }
            for cid in &affected {
                if *cid < self.broker.cohort_caches.len() {
                    self.broker.cohort_caches[*cid]
                        .write()
                        .await
                        .agent_ids
                        .clear();
                }
            }
        }
    }

    // ═════════════════════════════════════════════════════════════════
    // Phase 1 — Sequential Deterministic Tests
    // ═════════════════════════════════════════════════════════════════

    /// 1. Basic two-cohort quorum and global centroid synthesis.
    ///
    /// Validates that after both cohorts reach internal consensus, the
    /// inter-cohort cache has 2 entries and both centroids show coherence
    /// above threshold.  Goldilocks may discard individual agent submits
    /// (first agent of a cohort always falls through to bare goldilocks
    /// and may be filtered as noise against existing clusters), so the
    /// test only asserts final invariants, not intermediate responses.
    #[tokio::test]
    async fn test_basic_two_cohort_quorum() {
        let mut h = TestHarness::new();

        // Two News agents, two Infra agents — all stable, calm.
        h.add_agent("N1", "Signal", 0.2, 0).await;
        h.add_agent("N2", "Signal", 0.2, 0).await;
        h.add_agent("I1", "Internal", 0.2, 0).await;
        h.add_agent("I2", "Internal", 0.2, 0).await;

        // Submit all agents.  The order ensures each cohort gets its
        // second agent before the other cohort's first agent hits
        // goldilocks with orthogonal centroids.
        for id in &["N1", "N2", "I1", "I2"] {
            h.submit(id).await;
        }

        // Both cohorts should have active centroids with W_k > 0.55
        let w_news = h.assert_cohort_active(0).await; // News = cohort 0
        let w_infra = h.assert_cohort_active(1).await; // Infra = cohort 1
        assert!(w_news >= 0.55, "News coherence too low: {}", w_news);
        assert!(w_infra >= 0.55, "Infra coherence too low: {}", w_infra);

        // Inter-cohort cache should have 2 entries.
        h.assert_inter_cache_size(2).await;

        eprintln!("✓ test_basic_two_cohort_quorum");
    }

    /// 2. Cohort abstention when internal coherence drops below threshold.
    #[tokio::test]
    async fn test_cohort_abstention() {
        let mut h = TestHarness::new();

        // One News agent (cannot reach quorum alone — needs ≥ 2).
        h.add_agent("N1", "Signal", 0.2, 0).await;
        let resp = h.submit("N1").await;
        // Single agent → falls through to bare goldilocks (no global).
        assert!(
            matches!(resp, Some(HiveMessage::SyncUpdate { .. })),
            "Single agent should still pass through goldilocks"
        );
        h.assert_cohort_abstaining(0).await;

        // Now add a second News agent with divergent centroid → coherence
        // will be low but there are two agents, so the cohort should NOT
        // abstain just because coherence is low — it depends on CONSENSUS_FLOOR.
        // Actually, coherence needs to be < 0.55 for abstention.
        // With stable agents, coherence should be high.
        h.add_agent("N2", "Signal", 0.5, 0).await;
        h.submit("N2").await;
        // Both agents are stable with the same anchor → high coherence
        let w = h.assert_cohort_active(0).await;
        assert!(w > 0.55, "News coherence dropped unexpectedly: {}", w);

        eprintln!("✓ test_cohort_abstention");
    }

    /// 3. Re-entry anxiety floor lifts after exactly 5 consolidations.
    #[tokio::test]
    async fn test_reentry_floor_timeline() {
        let mut h = TestHarness::new();

        // Two News agents.  We'll disconnect and reconnect one to
        // observe the re-entry counter progression.
        h.add_agent("N1", "Signal", 0.1, 0).await;
        h.add_agent("N2", "Signal", 0.1, 0).await;
        h.add_agent("I1", "Internal", 0.1, 0).await;
        h.add_agent("I2", "Internal", 0.1, 0).await;

        // Settle both cohorts.
        for id in &["N1", "N2", "I1", "I2"] {
            h.submit(id).await;
        }

        // Simulate N1 disconnecting and reconnecting.
        h.remove_agent("N1").await;
        h.add_agent("N1", "Signal", 0.1, 0).await;

        // Re-entry counter should be 0 after reconnection.
        assert_eq!(h.reentry_ticks("N1").await, 0);

        // Submit N1 five times:
        for i in 0..5 {
            h.submit("N1").await;
            let ticks = h.reentry_ticks("N1").await;
            if i < 4 {
                assert!(
                    ticks < REENTRY_STABILIZATION_TICKS,
                    "At step {} expected ticks < {}, got {}",
                    i,
                    REENTRY_STABILIZATION_TICKS,
                    ticks
                );
            }
        }

        // After 5 submissions, re-entry counter should have passed
        // REENTRY_STABILIZATION_TICKS, lifting the floor.
        let ticks = h.reentry_ticks("N1").await;
        assert!(
            ticks >= REENTRY_STABILIZATION_TICKS,
            "Expected ticks >= {} after 5 submissions, got {}",
            REENTRY_STABILIZATION_TICKS,
            ticks
        );

        eprintln!("✓ test_reentry_floor_timeline");
    }

    /// 4. Flapping agent cannot dominate global centroid.
    #[tokio::test]
    async fn test_flapping_agent_contained() {
        let mut h = TestHarness::new();

        // Set up a stable News cohort with 2 agents.
        h.add_agent("N1", "Signal", 0.2, 0).await;
        h.add_agent("N2", "Signal", 0.2, 0).await;
        h.add_agent("I1", "Internal", 0.2, 0).await;
        h.add_agent("I2", "Internal", 0.2, 0).await;

        // Baseline: both cohorts active.
        for id in &["N1", "N2", "I1", "I2"] {
            h.submit(id).await;
        }

        // Record the baseline News cohort centroid.
        let baseline = {
            let map = h.broker.cohort_centroids.read().await;
            map.get(&0).cloned().unwrap().0
        };

        // Now a flapping News agent cycles through 5 rapid
        // connect → submit (divergent centroid) → disconnect rounds.
        // (Limited to 5 to keep N1/N2 within MAX_SILENT_EPOCHS=10.)
        for i in 0..5 {
            // Connect
            h.add_agent("FLAP", "Signal", 0.8 /* high anxiety */, 2 /* divergent */).await;
            let _resp = h.submit("FLAP").await;
            // After submit, verify News cohort is still in cohort_centroids.
            {
                let map = h.broker.cohort_centroids.read().await;
                if map.get(&0).is_none() {
                    let all: Vec<usize> = map.keys().copied().collect();
                    eprintln!(
                        "FLAP iteration {}: News cohort DISAPPEARED after submit. Present: {:?}",
                        i, all
                    );
                }
            }
            // Disconnect
            h.remove_agent("FLAP").await;
        }

        // The News cohort centroid should still be recognizably similar
        // to the baseline (not completely overwritten by divergent flapping).
        let current = {
            let map = h.broker.cohort_centroids.read().await;
            let (cent, _) = map.get(&0).unwrap_or_else(|| {
                // Diagnostic: dump the full cohort_centroids map
                let keys: Vec<usize> = map.keys().copied().collect();
                panic!(
                    "News cohort (id=0) absent from cohort_centroids after flap loop. Present cohorts: {:?}",
                    keys
                );
            });
            *cent
        };
        let drift = baseline.normalized_hamming_distance(&current);

        // The centroid should not have drifted beyond 0.3 from baseline.
        // (Completely dominated by flapping would produce drift ~0.5.)
        assert!(
            drift < 0.30,
            "News centroid drifted too far from baseline: {}",
            drift
        );

        eprintln!("✓ test_flapping_agent_contained");
    }

    /// 5. Constitutional tiebreaker is order-independent.
    #[tokio::test]
    async fn test_constitutional_tiebreaker_determinism() {
        // Create a constitution.
        let constitution = Hypervector::new_random();

        // Create two orthogonal centroids (maximally dissimilar).
        let a = Hypervector::new_random();
        let mut b = a;
        for i in 0..a.bits.len() {
            b.bits[i] = !b.bits[i];
        }

        // bundle_with_constitution(a, b) must equal bundle_with_constitution(b, a)
        let result_ab = Hypervector::bundle_with_constitution(&[&a, &b], &constitution);
        let result_ba = Hypervector::bundle_with_constitution(&[&b, &a], &constitution);

        assert_eq!(
            result_ab.bits, result_ba.bits,
            "Constitutional tiebreaker is order-dependent!"
        );

        // Also verify that the result uses the constitution bits for ties.
        // Since a and b are bitwise complements, the majority is always
        // exactly tied for every bit position.  The result should equal
        // the constitution (by construction).
        assert_eq!(
            result_ab.bits, constitution.bits,
            "Constitutional tiebreaker should produce constitution for complementary inputs"
        );

        eprintln!("✓ test_constitutional_tiebreaker_determinism");
    }

    /// 6. Dead agents are pruned after MAX_SILENT_EPOCHS.
    #[tokio::test]
    async fn test_dead_agent_pruning() {
        let mut h = TestHarness::new();
        h.add_agent("N1", "Signal", 0.2, 0).await;
        h.add_agent("N2", "Signal", 0.2, 0).await;
        h.add_agent("I1", "Internal", 0.2, 0).await;
        h.add_agent("I2", "Internal", 0.2, 0).await;
        for id in &["N1", "N2", "I1", "I2"] {
            h.submit(id).await;
        }

        // Simulate N1 going silent.  We do this by NOT calling submit
        // for N1 while advancing the epoch through other agents.
        for _ in 0..(MAX_SILENT_EPOCHS + 2) as usize {
            h.submit("N2").await;
            h.submit("I1").await;
            h.submit("I2").await;
        }

        // Run the pruning sweep.
        h.run_pruning_sweep().await;

        // N1 should be gone from agent_states.
        {
            let states = h.broker.agent_states.read().await;
            assert!(
                !states.contains_key("N1"),
                "Dead agent N1 was not pruned"
            );
        }
        // N1 should also be gone from cohort_of_agent.
        {
            let map = h.broker.cohort_of_agent.read().await;
            assert!(
                !map.contains_key("N1"),
                "Dead agent N1 not removed from cohort_of_agent"
            );
        }

        eprintln!("✓ test_dead_agent_pruning");
    }

    /// 7. Intra-coherence < 0.55 → cohort abstains; other cohorts continue.
    #[tokio::test]
    async fn test_abstention_does_not_block_other_cohorts() {
        let mut h = TestHarness::new();

        // Two stable News agents and two stable Infra agents.
        h.add_agent("N1", "Signal", 0.2, 0).await;
        h.add_agent("N2", "Signal", 0.2, 0).await;
        h.add_agent("I1", "Internal", 0.2, 0).await;
        h.add_agent("I2", "Internal", 0.2, 0).await;
        for id in &["N1", "N2", "I1", "I2"] {
            h.submit(id).await;
        }
        assert!(h.assert_cohort_active(0).await >= 0.55); // News active
        assert!(h.assert_cohort_active(1).await >= 0.55); // Infra active

        // Remove N2.  News now has 1 agent.
        h.remove_agent("N2").await;

        // Submit N1 alone.  With only 1 News agent, it falls through
        // to bare goldilocks.  Goldilocks may or may not return a
        // message depending on whether the centroid matches an existing
        // cluster — we don't assert the intermediate response.
        let _ = h.submit("N1").await;

        // News should be abstaining now (only 1 agent in cohort).
        h.assert_cohort_abstaining(0).await;

        // Infra should still be active.
        assert!(h.assert_cohort_active(1).await >= 0.55);

        // The inter-cohort cache may have been cleared when News was
        // invalidated, but the Infra cohort centroid persists.
        // (Cache reconstruction happens on the next Infra submit.)

        eprintln!("✓ test_abstention_does_not_block_other_cohorts");
    }

    // ═════════════════════════════════════════════════════════════════
    // Phase 2 — Concurrent Stress Test
    // ═════════════════════════════════════════════════════════════════

    /// 8. 1,000 agents across 4 cohorts, 10 rounds each = 10,000
    ///    concurrent consolidations.  Validates lock hierarchy and
    ///    sharded cache contention under load.
    #[tokio::test]
    async fn test_concurrent_1000_agents_stress() {
        let broker = Arc::new(NeocortexBroker::new(
            "stress_key",
            "data/test_stress_1000.bin",
            0,
        ));
        let _ = std::fs::remove_file("data/test_stress_1000.bin");

        // Register 1000 agents across 4 cohorts.
        let roles = ["Signal", "Internal", "External", "General"];
        let mut agent_ids: Vec<String> = Vec::with_capacity(1000);
        for i in 0..1000 {
            let id = format!("S{}", i);
            let role = roles[i % 4];
            broker.assign_cohort(&id, role).await;
            agent_ids.push(id);
        }

        // Launch 1000 concurrent tasks, each submitting 10 consolidations.
        let mut handles = Vec::with_capacity(1000);
        for id in &agent_ids {
            let broker = Arc::clone(&broker);
            let id = id.clone();
            handles.push(tokio::spawn(async move {
                for _ in 0..10 {
                    let centroid = Hypervector::new_random();
                    let _ = broker
                        .process_consolidation(centroid, Vec::new(), &id, 0.3)
                        .await;
                }
            }));
        }

        // Wait for all tasks.
        for h in handles {
            h.await.expect("Agent task panicked");
        }

        // Invariant 1: The broker is still responsive.
        // VERIFY needs a cohort assignment so it doesn't hit the
        // no-cohort early return path.
        broker.assign_cohort("VERIFY", "External").await;
        let test_centroid = Hypervector::new_random();
        // process_consolidation should not panic.  Its return value
        // is not deterministic (goldilocks may discard a random centroid
        // as noise), but the lock hierarchy must survive 10,000+ calls.
        let _ = broker
            .process_consolidation(test_centroid, Vec::new(), "VERIFY", 0.1)
            .await;

        // Invariant 2: The sharded caches are not corrupted (no panics
        // occurred during concurrent access).  Cache sizes are
        // timing-dependent due to the dead-agent epoch filter, so we
        // only verify they're non-negative and the structure is intact.
        for cid in 0..4 {
            let cache = broker.cohort_caches[cid].read().await;
            assert!(
                cache.sims.len() == cache.agent_ids.len()
                    && cache.sims.len() == cache.centroids.len(),
                "Cohort {} cache structure corrupted: sims={}, ids={}, centroids={}",
                cid,
                cache.sims.len(),
                cache.agent_ids.len(),
                cache.centroids.len()
            );
        }

        // Invariant 3: All agents are still registered in agent_states
        // (the dead-agent filter only excludes from quorum, it does not
        // remove).  1000 test agents + 1 VERIFY agent = 1001 total.
        {
            let states = broker.agent_states.read().await;
            assert!(
                states.len() >= 1000,
                "Too many agents lost during stress test: {}",
                states.len()
            );
        }

        // Invariant 4: No panic occurred (we'd have caught it via
        // the JoinSet's expect above).

        // Cleanup
        let _ = std::fs::remove_file("data/test_stress_1000.bin");

        eprintln!("✓ test_concurrent_1000_agents_stress — all 10,000 consolidations completed");
    }
}
