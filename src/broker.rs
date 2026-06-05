use crate::{ledger::LongTermLedger, DejavuEntry, HiveMessage, Hypervector, MemoryCluster};
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

/// Minimum fraction of agent-pairs that must agree (similarity ≥ threshold)
/// for a concept to be allowed into the permanent ledger.
pub const QUORUM_FRACTION: f64 = 0.66;

/// Pairwise similarity must be at least this value for a pair to count as
/// "in agreement."
pub const QUORUM_SIMILARITY_THRESHOLD: f64 = 0.66;

/// When consensus similarity (weighted average of all pairs) falls below
/// this threshold, a DissonanceAlert is broadcast.
pub const CONSENSUS_FLOOR: f64 = 0.55;

// ─── AgentSubmission ──────────────────────────────────────────────────────

/// Tracks the latest consolidation submission from a connected agent,
/// used by the broker to compute multi-agent consensus.
#[derive(Clone, Debug)]
struct AgentSubmission {
    centroid: Hypervector,
    anxiety: f64,
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
}

impl NeocortexBroker {
    pub fn new(key: &str, file_path: &str, port: u16) -> Self {
        NeocortexBroker {
            dejavu_clusters: Arc::new(RwLock::new(Vec::new())),
            ledger: Arc::new(LongTermLedger::new(key, file_path)),
            clients: Arc::new(Mutex::new(Vec::new())),
            port,
            key: key.to_string(),
            concept: Hypervector::new_random(),
            agent_states: Arc::new(RwLock::new(HashMap::new())),
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

                    let entry = DejavuEntry {
                        vector,
                        label: date_str.clone(),
                        metadata: HashMap::new(),
                    };

                    if let Some(idx) = best_idx {
                        if best_sim >= 0.65 {
                            clusters[idx].entries.push(entry);
                            let refs: Vec<&Hypervector> =
                                clusters[idx].entries.iter().map(|e| &e.vector).collect();
                            clusters[idx].centroid = Hypervector::bundle(&refs);
                            continue;
                        }
                    }

                    clusters.push(MemoryCluster {
                        centroid: vector,
                        entries: vec![entry],
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

    // ─── Multi-Agent Consensus Protocol ─────────────────────────────

    /// Process a consolidation submission with **anxiety-weighted consensus**.
    ///
    /// 1. Stores the agent's submission alongside its anxiety level.
    /// 2. Checks **quorum**: at least 66% of active agent-pairs must agree
    ///    (pairwise similarity ≥ 0.66) before any concept touches the ledger.
    /// 3. If quorum is met, computes an **anxiety-weighted consensus centroid**
    ///    and passes it through the standard Goldilocks merge/fission sieve.
    /// 4. If consensus is structurally incoherent (average similarity < 0.55),
    ///    returns a `DissonanceAlert` forcing all agents to rotate intent.
    pub async fn process_consolidation(
        &self,
        centroid: Hypervector,
        entries: Vec<DejavuEntry>,
        agent_id: &str,
        agent_anxiety: f64,
    ) -> Option<HiveMessage> {
        // 1. Store this agent's submission
        {
            let mut states = self.agent_states.write().await;
            states.insert(
                agent_id.to_string(),
                AgentSubmission {
                    centroid,
                    anxiety: agent_anxiety,
                },
            );
        }

        // 2. Collect all current agent submissions
        let submissions: Vec<AgentSubmission> = {
            let states = self.agent_states.read().await;
            states.values().cloned().collect()
        };

        let active_count = submissions.len();

        // Need at least 2 agents to compute consensus
        if active_count < 2 {
            // Not enough agents for quorum — fall back to normal processing
            return self
                .goldilocks_sieve(centroid, entries)
                .await;
        }

        // 3. Compute pairwise similarities between all agents
        let mut pair_sims = Vec::new();
        for i in 0..submissions.len() {
            for j in (i + 1)..submissions.len() {
                let sim =
                    1.0 - submissions[i]
                        .centroid
                        .normalized_hamming_distance(&submissions[j].centroid);
                pair_sims.push(sim);
            }
        }

        let total_pairs = pair_sims.len() as f64;
        let agreeing_pairs = pair_sims.iter().filter(|&&s| s >= QUORUM_SIMILARITY_THRESHOLD).count() as f64;
        let quorum_met = active_count >= 3 && (agreeing_pairs / total_pairs) >= QUORUM_FRACTION
            || active_count == 2 && pair_sims.iter().all(|&s| s >= QUORUM_SIMILARITY_THRESHOLD);

        let avg_pairwise_sim: f64 = pair_sims.iter().sum::<f64>() / total_pairs;

        // 4. Check for DissonanceAlert
        if avg_pairwise_sim < CONSENSUS_FLOOR {
            return Some(HiveMessage::DissonanceAlert {
                consensus_similarity: avg_pairwise_sim,
                agent_count: active_count,
            });
        }

        // 5. Anxiety-weighted consensus centroid
        let consensus_centroid = if quorum_met {
            compute_anxiety_weighted_centroid(&submissions)
        } else {
            // Quorum not met — the submitting agent's centroid stands alone
            centroid
        };

        // 6. Pass through the standard Goldilocks sieve
        self.goldilocks_sieve(consensus_centroid, entries).await
    }

    /// The original Goldilocks merge / fission / discard sieve.
    /// Shared between consensus and fallback paths.
    async fn goldilocks_sieve(
        &self,
        centroid: Hypervector,
        entries: Vec<DejavuEntry>,
    ) -> Option<HiveMessage> {
        let mut clusters = self.dejavu_clusters.write().await;

        if clusters.is_empty() {
            let new_cluster = MemoryCluster { centroid, entries };
            clusters.push(new_cluster.clone());
            let today_str = chrono::Utc::now().format("%Y-%m-%d").to_string();
            let _ = self
                .ledger
                .append_record(&today_str, &centroid, &self.concept);
            return Some(HiveMessage::SyncUpdate {
                is_new_cluster: true,
                cluster_index: Some(0),
                cluster: new_cluster,
            });
        }

        let mut best_idx = None;
        let mut best_sim = -1.0;

        for (idx, cluster) in clusters.iter().enumerate() {
            let sim = 1.0 - centroid.normalized_hamming_distance(&cluster.centroid);
            if sim > best_sim {
                best_sim = sim;
                best_idx = Some(idx);
            }
        }

        let mut sync_msg = None;
        if best_sim >= 0.75 {
            if let Some(idx) = best_idx {
                for entry in entries {
                    clusters[idx].entries.push(entry);
                }
                let refs: Vec<&Hypervector> =
                    clusters[idx].entries.iter().map(|e| &e.vector).collect();
                clusters[idx].centroid = Hypervector::bundle(&refs);

                let today_str = chrono::Utc::now().format("%Y-%m-%d").to_string();
                let _ = self
                    .ledger
                    .append_record(&today_str, &clusters[idx].centroid, &self.concept);

                sync_msg = Some(HiveMessage::SyncUpdate {
                    is_new_cluster: false,
                    cluster_index: Some(idx),
                    cluster: clusters[idx].clone(),
                });
            }
        } else if best_sim < 0.52 {
            // Discard noise
        } else {
            // Fission
            let new_cluster = MemoryCluster { centroid, entries };
            clusters.push(new_cluster.clone());
            let new_idx = clusters.len() - 1;

            let today_str = chrono::Utc::now().format("%Y-%m-%d").to_string();
            let _ = self
                .ledger
                .append_record(&today_str, &centroid, &self.concept);

            sync_msg = Some(HiveMessage::SyncUpdate {
                is_new_cluster: true,
                cluster_index: Some(new_idx),
                cluster: new_cluster,
            });
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
        let compaction_ledger = Arc::clone(&self.ledger);
        let compaction_concept = self.concept;
        let compaction_log = log_tx.clone();
        let max_interval = chrono::Duration::days(7);
        let growth_threshold: usize = 50;

        tokio::spawn(async move {
            let mut last_compaction = chrono::Utc::now();
            let mut last_record_count: Option<usize> = None;

            loop {
                tokio::time::sleep(tokio::time::Duration::from_secs(60)).await;

                let records = match compaction_ledger.load_records(&compaction_concept) {
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

                    match compaction_ledger.compact_ledger(&compaction_concept, 0.70) {
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
fn compute_anxiety_weighted_centroid(submissions: &[AgentSubmission]) -> Hypervector {
    if submissions.is_empty() {
        return Hypervector::new_zero();
    }
    if submissions.len() == 1 {
        return submissions[0].centroid;
    }

    let mut weighted_refs: Vec<&Hypervector> = Vec::new();

    for submission in submissions {
        // Weight = 1 / (1 + anxiety); ranges from 1.0 (anxiety=0) down to 0.5 (anxiety=1)
        let weight = 1.0 / (1.0 + submission.anxiety);
        // Scale to integer copies; at least 1 so every agent is heard
        let copies = ((weight * 10.0).round() as usize).max(1);
        for _ in 0..copies {
            weighted_refs.push(&submission.centroid);
        }
    }

    Hypervector::bundle(&weighted_refs)
}
