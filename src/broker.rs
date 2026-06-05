use crate::{ledger::LongTermLedger, DejavuEntry, HiveMessage, Hypervector, MemoryCluster};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;
use tokio::sync::{Mutex, RwLock};

pub struct NeocortexBroker {
    pub dejavu_clusters: Arc<RwLock<Vec<MemoryCluster>>>,
    pub ledger: Arc<LongTermLedger>,
    pub clients: Arc<Mutex<Vec<tokio::net::tcp::OwnedWriteHalf>>>,
    pub port: u16,
    pub key: String,
    pub concept: Hypervector,
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
        }
    }

    /// Helper function to write a HiveMessage to an agent stream
    pub async fn write_msg(
        writer: &mut tokio::net::tcp::OwnedWriteHalf,
        msg: &HiveMessage,
    ) -> Result<(), std::io::Error> {
        let json_bytes = serde_json::to_vec(msg)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
        let len = json_bytes.len() as u32;
        writer.write_all(&len.to_be_bytes()).await?;
        writer.write_all(&json_bytes).await?;
        Ok(())
    }

    /// Helper function to read a HiveMessage from an agent stream
    pub async fn read_msg(
        reader: &mut tokio::net::tcp::OwnedReadHalf,
    ) -> Result<Option<HiveMessage>, std::io::Error> {
        let mut len_bytes = [0u8; 4];
        if let Err(_) = reader.read_exact(&mut len_bytes).await {
            return Ok(None); // Connection closed
        }
        let len = u32::from_be_bytes(len_bytes) as usize;
        let mut buf = vec![0u8; len];
        reader.read_exact(&mut buf).await?;
        let msg: HiveMessage = serde_json::from_slice(&buf)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
        Ok(Some(msg))
    }

    /// Broadcast a HiveMessage to all active connected agents
    pub async fn broadcast(&self, msg: &HiveMessage) {
        let mut clients_guard = self.clients.lock().await;
        let mut disconnected_indices = Vec::new();

        for (idx, client) in clients_guard.iter_mut().enumerate() {
            if let Err(_) = Self::write_msg(client, msg).await {
                disconnected_indices.push(idx);
            }
        }

        // Clean up disconnected clients
        disconnected_indices.sort_unstable_by(|a, b| b.cmp(a));
        for idx in disconnected_indices {
            if idx < clients_guard.len() {
                clients_guard.remove(idx);
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

                // Reconstitute into RAM clusters
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

    /// Processes an agent consolidation submission through the global Goldilocks Sieve
    pub async fn process_consolidation(
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
                let _ =
                    self.ledger
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

    /// Core Broker runtime loop
    pub async fn run(
        self: Arc<Self>,
        log_tx: tokio::sync::mpsc::UnboundedSender<String>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        self.boot_reconstitute(&log_tx).await;

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
                let (agent_id, _agent_role) = match Self::read_msg(&mut reader).await {
                    Ok(Some(HiveMessage::HandshakeRequest { agent_id: id, role })) => {
                        let _ = log_tx_clone
                            .send(format!("BROKER: Connection from Agent {} ({})", id, role));

                        // Send back current memory cache
                        let current_clusters = {
                            let db = broker_clone.dejavu_clusters.read().await;
                            db.clone()
                        };
                        let response = HiveMessage::HandshakeResponse {
                            permanent_clusters: current_clusters,
                        };
                        if let Err(_) = Self::write_msg(&mut writer, &response).await {
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
                    match Self::read_msg(&mut reader).await {
                        Ok(Some(HiveMessage::ConsolidateRequest { centroid, entries })) => {
                            let _ = log_tx_clone.send(format!(
                                "BROKER: Processing Consolidation Request from Agent {}",
                                agent_id
                            ));
                            if let Some(sync_update) =
                                broker_clone.process_consolidation(centroid, entries).await
                            {
                                broker_clone.broadcast(&sync_update).await;
                                let _ = log_tx_clone.send(format!(
                                    "BROKER: Broadcasted memory SyncUpdate to all agents."
                                ));
                            }
                        }
                        Ok(Some(HiveMessage::PanicLockdown { attacker_info })) => {
                            let _ = log_tx_clone.send(format!(
                                "BROKER: CRITICAL PANIC received from Agent {}! Attacker: {}. Sealing Ledger.",
                                agent_id, attacker_info
                            ));
                            broker_clone.trigger_quarantine().await;

                            // Broadcast PanicLockdown
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
