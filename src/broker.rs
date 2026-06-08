use crate::hnsw::HnswIndex;
use crate::{ledger::LongTermLedger, DejavuEntry, HiveMessage, Hypervector, MemoryCluster};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
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

// ══════════════════════════════════════════════════════════════════════════
// FEDERATED DHT PEER (Distributed Hash Table for P2P Memory)
// ══════════════════════════════════════════════════════════════════════════

/// A peer node in the federated memory network.
///
/// Each broker instance is a DHT node. Peers form a distributed hash table
/// where memory clusters are keyed by their centroid hypervectors.
/// The DHT enables:
/// - Asynchronous sync between brokers
/// - Localized consensus clusters (agents form affinity groups)
/// - No single point of failure
/// - Planetary-scale horizontal scaling
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct DhtPeerInfo {
    /// Unique peer ID (derived from identity hypervector)
    pub peer_id: String,
    /// Host address for peer-to-peer connections
    pub host: String,
    /// DHT port for inter-broker communication
    pub dht_port: u16,
    /// Broker's memory port for agent connections
    pub broker_port: u16,
    /// Hypervector fingerprint of this peer
    pub fingerprint: Hypervector,
    /// Known memory keys this peer hosts (centroid hypervectors as bytes)
    pub hosted_keys: Vec<Vec<u8>>,
}

/// Internal DHT routing table entry
#[derive(Clone, Debug)]
#[allow(dead_code)]
struct DhtRoutingEntry {
    peer: DhtPeerInfo,
    last_seen: chrono::DateTime<chrono::Utc>,
    latency_ms: f64,
}

/// The DHT node implementation for federated memory.
pub struct DhtNode {
    /// Our own peer info
    pub local_info: DhtPeerInfo,
    /// Routing table: peer_id → routing entry
    routing_table: Arc<RwLock<HashMap<String, DhtRoutingEntry>>>,
    /// Local key-value store: centroid_hash → (centroid, MemoryCluster)
    local_store: Arc<RwLock<HashMap<String, MemoryCluster>>>,
    /// Maximum routing table size
    max_routes: usize,
    /// DHT listener port
    dht_port: u16,
    /// Shared secret for inter-broker authentication
    #[allow(dead_code)]
    cluster_secret: String,
    /// HNSW index for O(log n) peer routing.
    /// Maps peer fingerprint hypervector → peer index in the routing table.
    /// Rebuilt when the routing table changes significantly.
    hnsw_peer_index: Arc<std::sync::Mutex<Option<HnswIndex>>>,
    /// Peer ID → HNSW index mapping (used for result translation)
    peer_id_to_hnsw: Arc<RwLock<HashMap<String, usize>>>,
    /// HNSW index → Peer ID mapping
    hnsw_to_peer_id: Arc<RwLock<Vec<String>>>,
}

impl DhtNode {
    pub fn new(
        peer_id: &str,
        host: &str,
        dht_port: u16,
        broker_port: u16,
        cluster_secret: &str,
    ) -> Self {
        let mut hasher = Sha256::new();
        hasher.update(peer_id.as_bytes());
        let hash = hasher.finalize();
        let mut bits = [0u64; 157];
        for i in 0..3 {
            bits[i] = u64::from_be_bytes([
                hash[i * 8], hash[i * 8 + 1], hash[i * 8 + 2], hash[i * 8 + 3],
                hash[i * 8 + 4], hash[i * 8 + 5], hash[i * 8 + 6], hash[i * 8 + 7],
            ]);
        }
        let fingerprint = Hypervector { bits };

        DhtNode {
            local_info: DhtPeerInfo {
                peer_id: peer_id.to_string(),
                host: host.to_string(),
                dht_port,
                broker_port,
                fingerprint,
                hosted_keys: Vec::new(),
            },
            routing_table: Arc::new(RwLock::new(HashMap::new())),
            local_store: Arc::new(RwLock::new(HashMap::new())),
            max_routes: 100,
            dht_port,
            cluster_secret: cluster_secret.to_string(),
            hnsw_peer_index: Arc::new(std::sync::Mutex::new(None)),
            peer_id_to_hnsw: Arc::new(RwLock::new(HashMap::new())),
            hnsw_to_peer_id: Arc::new(RwLock::new(Vec::new())),
        }
    }

    /// Compute the DHT key for a centroid hypervector.
    fn compute_key(centroid: &Hypervector) -> String {
        let bytes = centroid.to_bytes();
        let mut hasher = Sha256::new();
        hasher.update(&bytes[..]);
        let hash = hasher.finalize();
        Self::hex_encode(&hash)
    }

    /// Generate a hex ID from bytes (simple hex encoding without external crate)
    fn hex_encode(bytes: &[u8]) -> String {
        let hex_chars = b"0123456789abcdef";
        let mut result = String::with_capacity(bytes.len() * 2);
        for &byte in bytes {
            result.push(hex_chars[(byte >> 4) as usize] as char);
            result.push(hex_chars[(byte & 0x0f) as usize] as char);
        }
        result
    }

    /// Register a peer in the routing table.
    pub async fn register_peer(&self, peer: DhtPeerInfo) {
        let mut table = self.routing_table.write().await;
        let entry = DhtRoutingEntry {
            peer,
            last_seen: chrono::Utc::now(),
            latency_ms: 0.0,
        };
        table.insert(entry.peer.peer_id.clone(), entry);

        // Prune oldest entry if table is full
        if table.len() > self.max_routes {
            let oldest_id = table.iter()
                .min_by_key(|(_, e)| e.last_seen)
                .map(|(id, _)| id.clone());
            if let Some(id) = oldest_id {
                table.remove(&id);
            }
        }
    }

    /// Remove a peer from the routing table.
    pub async fn remove_peer(&self, peer_id: &str) {
        self.routing_table.write().await.remove(peer_id);
    }

    /// Rebuild the HNSW peer index for O(log n) routing.
    pub async fn rebuild_peer_hnsw_index(&self) {
        let table = self.routing_table.read().await;
        if table.len() < 10 {
            return; // Too small for HNSW to be beneficial
        }

        let mut index = HnswIndex::with_config(crate::hnsw::HnswConfig {
            use_heuristic: true,
            ..crate::hnsw::HnswConfig::default()
        });

        let mut id_to_hnsw = HashMap::new();
        let mut hnsw_to_id = Vec::new();

        for (peer_id, entry) in table.iter() {
            let fp = &entry.peer.fingerprint;
            let idx = index.insert(&fp.bits);
            id_to_hnsw.insert(peer_id.clone(), idx);
            hnsw_to_id.push(peer_id.clone());
        }

        let mut guard = self.hnsw_peer_index.lock().unwrap();
        *guard = Some(index);
        *self.peer_id_to_hnsw.write().await = id_to_hnsw;
        *self.hnsw_to_peer_id.write().await = hnsw_to_id;
    }

    /// Find the closest peers using HNSW-accelerated hypervector similarity.
    ///
    /// Unlike `find_closest_peers` (which uses Kademlia XOR distance on string keys),
    /// this method uses the full 10,048-bit hypervector fingerprint distance,
    /// enabling semantic peer discovery: "find peers with similar memory profiles."
    ///
    /// Returns `None` if the HNSW index hasn't been built yet.
    pub async fn find_closest_peers_by_fingerprint(
        &self,
        query_fingerprint: &Hypervector,
        count: usize,
    ) -> Option<Vec<DhtPeerInfo>> {
        let guard = self.hnsw_peer_index.lock().unwrap();
        let index = guard.as_ref()?;

        let result = index.find_k_nearest(&query_fingerprint.bits, count);
        if result.is_empty() {
            return None;
        }

        let table = self.routing_table.read().await;
        let hnsw_to_id = self.hnsw_to_peer_id.read().await;

        let mut peers = Vec::new();
        for hnsw_idx in &result.indices {
            if *hnsw_idx < hnsw_to_id.len() {
                let peer_id = &hnsw_to_id[*hnsw_idx];
                if let Some(entry) = table.get(peer_id) {
                    peers.push(entry.peer.clone());
                }
            }
        }
        Some(peers)
    }

    /// Find the closest peers to a given key using XOR distance (Kademlia-style).
    pub async fn find_closest_peers(&self, key: &str, count: usize) -> Vec<DhtPeerInfo> {
        let table = self.routing_table.read().await;
        let key_bytes = key.as_bytes();

        let mut peers: Vec<(DhtPeerInfo, usize)> = table
            .values()
            .map(|entry| {
                let peer_key_bytes = entry.peer.peer_id.as_bytes();
                // XOR distance: count differing bytes
                let dist = key_bytes.iter()
                    .zip(peer_key_bytes.iter())
                    .map(|(a, b)| (*a ^ *b).count_ones() as usize)
                    .sum::<usize>();
                (entry.peer.clone(), dist)
            })
            .collect();

        peers.sort_by(|a, b| a.1.cmp(&b.1));
        peers.truncate(count);
        peers.into_iter().map(|(p, _)| p).collect()
    }

    /// Store a memory cluster locally and announce to the DHT.
    pub async fn store_local(&self, cluster: MemoryCluster) {
        let key = Self::compute_key(&cluster.centroid);
        self.local_store.write().await.insert(key.clone(), cluster.clone());

        // Update hosted keys
        let mut info = self.local_info.clone();
        let key_bytes = key.as_bytes().to_vec();
        if !info.hosted_keys.contains(&key_bytes) {
            info.hosted_keys.push(key_bytes);
        }
    }

    /// Retrieve a memory cluster by centroid key.
    pub async fn retrieve_local(&self, key: &str) -> Option<MemoryCluster> {
        self.local_store.read().await.get(key).cloned()
    }

    /// Get all locally stored clusters.
    pub async fn all_local_clusters(&self) -> Vec<MemoryCluster> {
        self.local_store.read().await.values().cloned().collect()
    }

    /// Get known peers.
    pub async fn known_peers(&self) -> Vec<DhtPeerInfo> {
        self.routing_table.read().await
            .values().map(|e| e.peer.clone()).collect()
    }

    /// Start the DHT listener for inter-broker communication.
    pub async fn run_dht_listener(
        self: Arc<Self>,
        log_tx: tokio::sync::mpsc::UnboundedSender<String>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let listener = TcpListener::bind(format!("{}:{}", self.local_info.host, self.dht_port)).await?;
        let _ = log_tx.send(format!(
            "DHT NODE: Listening for peer connections on {}:{}",
            self.local_info.host, self.dht_port
        ));

        loop {
            let (socket, addr) = listener.accept().await?;
            let node = Arc::clone(&self);
            let log = log_tx.clone();

            tokio::spawn(async move {
                let (mut reader, mut writer) = tokio::io::split(socket);

                // Read DHT message
                let mut len_bytes = [0u8; 4];
                if reader.read_exact(&mut len_bytes).await.is_err() {
                    return;
                }
                let len = u32::from_be_bytes(len_bytes) as usize;
                let mut buf = vec![0u8; len];
                if reader.read_exact(&mut buf).await.is_err() {
                    return;
                }

                // Parse DHT message
                if let Ok(msg) = serde_json::from_slice::<DhtMessage>(&buf) {
                    let response = node.handle_dht_message(msg).await;

                    if let Some(resp) = response {
                        if let Ok(json) = serde_json::to_vec(&resp) {
                            let _ = writer.write_all(&(json.len() as u32).to_be_bytes()).await;
                            let _ = writer.write_all(&json).await;
                        }
                    }
                }

                let _ = log.send(format!("DHT: Peer exchange completed with {}", addr));
            });
        }
    }

    /// Handle an incoming DHT protocol message.
    async fn handle_dht_message(&self, msg: DhtMessage) -> Option<DhtMessage> {
        match msg {
            DhtMessage::Ping { sender_info } => {
                self.register_peer(sender_info.clone()).await;
                Some(DhtMessage::Pong {
                    sender_info: self.local_info.clone(),
                })
            }
            DhtMessage::FindNode { target_key, sender_info } => {
                self.register_peer(sender_info).await;
                let closest = self.find_closest_peers(&target_key, 8).await;
                Some(DhtMessage::NodesFound {
                    peers: closest,
                })
            }
            DhtMessage::Store { key, cluster, sender_info } => {
                self.register_peer(sender_info).await;
                self.store_local(cluster).await;
                Some(DhtMessage::StoreAck { key })
            }
            DhtMessage::Retrieve { key, sender_info } => {
                self.register_peer(sender_info).await;
                let cluster = self.retrieve_local(&key).await;
                Some(DhtMessage::RetrieveResponse { key, cluster })
            }
            DhtMessage::SyncRequest { known_keys, sender_info } => {
                self.register_peer(sender_info).await;
                // Return clusters whose keys the requester doesn't know about
                let local = self.all_local_clusters().await;
                let new_clusters: Vec<MemoryCluster> = local.into_iter()
                    .filter(|c| {
                        let k = Self::compute_key(&c.centroid);
                        !known_keys.contains(&k)
                    })
                    .collect();
                Some(DhtMessage::SyncResponse { clusters: new_clusters })
            }
            DhtMessage::Pong { .. } | DhtMessage::NodesFound { .. }
            | DhtMessage::StoreAck { .. } | DhtMessage::RetrieveResponse { .. }
            | DhtMessage::SyncResponse { .. } => {
                // These are responses we don't need to reply to
                None
            }
        }
    }

    /// Connect to a peer and send a DHT message, returning the response.
    pub async fn send_dht_message(
        &self,
        host: &str,
        port: u16,
        msg: &DhtMessage,
    ) -> Result<DhtMessage, String> {
        let addr = format!("{}:{}", host, port);
        let mut stream = TcpStream::connect(&addr)
            .await
            .map_err(|e| format!("Failed to connect to DHT peer {}: {}", addr, e))?;

        let json = serde_json::to_vec(msg)
            .map_err(|e| format!("Serialization failed: {}", e))?;

        stream.write_all(&(json.len() as u32).to_be_bytes())
            .await
            .map_err(|e| format!("Write failed: {}", e))?;
        stream.write_all(&json)
            .await
            .map_err(|e| format!("Write failed: {}", e))?;

        // Read response
        let (mut reader, _) = stream.into_split();
        let mut len_bytes = [0u8; 4];
        reader.read_exact(&mut len_bytes).await
            .map_err(|e| format!("Read failed: {}", e))?;
        let len = u32::from_be_bytes(len_bytes) as usize;
        let mut buf = vec![0u8; len];
        reader.read_exact(&mut buf).await
            .map_err(|e| format!("Read failed: {}", e))?;

        serde_json::from_slice(&buf)
            .map_err(|e| format!("Deserialization failed: {}", e))
    }

    /// Bootstrap by connecting to a known peer and discovering the network.
    pub async fn bootstrap(&self, seed_host: &str, seed_port: u16) -> Result<(), String> {
        let ping = DhtMessage::Ping {
            sender_info: self.local_info.clone(),
        };
        let response = self.send_dht_message(seed_host, seed_port, &ping).await?;
        match response {
            DhtMessage::Pong { sender_info } => {
                self.register_peer(sender_info).await;
                Ok(())
            }
            _ => Err("Unexpected bootstrap response".to_string()),
        }
    }

    /// Sync with all known peers (pull missing clusters).
    pub async fn sync_with_network(&self) -> Result<usize, String> {
        let peers = self.known_peers().await;
        if peers.is_empty() {
            return Ok(0);
        }

        let local = self.all_local_clusters().await;
        let known_keys: Vec<String> = local.iter()
            .map(|c| Self::compute_key(&c.centroid))
            .collect();

        let mut total_synced = 0;
        for peer in &peers {
            let sync_msg = DhtMessage::SyncRequest {
                known_keys: known_keys.clone(),
                sender_info: self.local_info.clone(),
            };

            if let Ok(response) = self.send_dht_message(&peer.host, peer.dht_port, &sync_msg).await {
                if let DhtMessage::SyncResponse { clusters } = response {
                    for cluster in clusters {
                        self.store_local(cluster).await;
                        total_synced += 1;
                    }
                }
            }
        }

        Ok(total_synced)
    }
}

/// Protocol messages for the DHT inter-broker communication.
#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub enum DhtMessage {
    /// Discover a peer / health check
    Ping { sender_info: DhtPeerInfo },
    /// Response to Ping
    Pong { sender_info: DhtPeerInfo },
    /// Find peers close to a key
    FindNode { target_key: String, sender_info: DhtPeerInfo },
    /// Response with closest peers
    NodesFound { peers: Vec<DhtPeerInfo> },
    /// Store a cluster under a key
    Store { key: String, cluster: MemoryCluster, sender_info: DhtPeerInfo },
    /// Acknowledge storage
    StoreAck { key: String },
    /// Retrieve a cluster by key
    Retrieve { key: String, sender_info: DhtPeerInfo },
    /// Response with retrieved cluster (or None)
    RetrieveResponse { key: String, cluster: Option<MemoryCluster> },
    /// Request sync: here are my known keys, send me new ones
    SyncRequest { known_keys: Vec<String>, sender_info: DhtPeerInfo },
    /// Response with new clusters
    SyncResponse { clusters: Vec<MemoryCluster> },
}

// ══════════════════════════════════════════════════════════════════════════
// NEOCORTEX BROKER (with DHT integration)
// ══════════════════════════════════════════════════════════════════════════

pub struct NeocortexBroker {
    pub dejavu_clusters: Arc<RwLock<Vec<MemoryCluster>>>,
    pub ledger: Arc<LongTermLedger>,
    pub clients: Arc<Mutex<Vec<tokio::net::tcp::OwnedWriteHalf>>>,
    pub port: u16,
    pub key: String,
    pub concept: Hypervector,
    /// Per-agent state for consensus computation.
    agent_states: Arc<RwLock<HashMap<String, AgentSubmission>>>,
    /// DHT node for federated memory
    pub dht_node: Option<Arc<DhtNode>>,
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
            dht_node: None,
        }
    }

    /// Initialize the DHT node for this broker.
    pub async fn init_dht(
        &mut self,
        peer_id: &str,
        host: &str,
        dht_port: u16,
        seed_host: Option<&str>,
        seed_port: Option<u16>,
    ) {
        let dht_node = Arc::new(DhtNode::new(
            peer_id,
            host,
            dht_port,
            self.port,
            &self.key,
        ));

        // Bootstrap from seed if provided
        if let (Some(sh), Some(sp)) = (seed_host, seed_port) {
            match dht_node.bootstrap(sh, sp).await {
                Ok(()) => {
                    // Sync with the network
                    match dht_node.sync_with_network().await {
                        Ok(n) => println!("DHT: Bootstrapped. Synced {} new clusters from network.", n),
                        Err(e) => eprintln!("DHT: Sync error: {}", e),
                    }
                }
                Err(e) => eprintln!("DHT: Bootstrap error: {}", e),
            }
        }

        self.dht_node = Some(dht_node);
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
                        reverberation: 1.0,
                        last_reinforced_tick: 0,
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
            centroid
        };

        // 6. Optionally replicate to DHT
        if let Some(ref dht) = self.dht_node {
            let cluster = MemoryCluster {
                centroid: consensus_centroid,
                entries: entries.clone(),
                reverberation: 1.0,
                last_reinforced_tick: 0,
            };
            dht.store_local(cluster).await;
        }

        // 7. Pass through the standard Goldilocks sieve
        self.goldilocks_sieve(consensus_centroid, entries).await
    }

    /// The original Goldilocks merge / fission / discard sieve.
    async fn goldilocks_sieve(
        &self,
        centroid: Hypervector,
        entries: Vec<DejavuEntry>,
    ) -> Option<HiveMessage> {
        let mut clusters = self.dejavu_clusters.write().await;

        if clusters.is_empty() {
            let new_cluster = MemoryCluster {
                centroid,
                entries,
                reverberation: 1.0,
                last_reinforced_tick: 0,
            };
            clusters.push(new_cluster.clone());
            let today_str = chrono::Utc::now().format("%Y-%m-%d").to_string();
            let _ = self
                .ledger
                .append_record(&today_str, &centroid, &self.concept);

            // Also replicate to DHT if available
            if let Some(ref dht) = self.dht_node {
                dht.store_local(new_cluster.clone()).await;
            }

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

                if let Some(ref dht) = self.dht_node {
                    dht.store_local(clusters[idx].clone()).await;
                }

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
            let new_cluster = MemoryCluster {
                centroid,
                entries,
                reverberation: 1.0,
                last_reinforced_tick: 0,
            };
            clusters.push(new_cluster.clone());
            let new_idx = clusters.len() - 1;

            let today_str = chrono::Utc::now().format("%Y-%m-%d").to_string();
            let _ = self
                .ledger
                .append_record(&today_str, &centroid, &self.concept);

            if let Some(ref dht) = self.dht_node {
                dht.store_local(new_cluster.clone()).await;
            }

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

// ─── Tests ────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_dht_node_creation() {
        let node = DhtNode::new(
            "test_node_1",
            "127.0.0.1",
            19001,
            9050,
            "test_secret",
        );
        assert_eq!(node.local_info.peer_id, "test_node_1");
        assert_eq!(node.local_info.dht_port, 19001);
        assert_eq!(node.local_info.broker_port, 9050);
    }

    #[tokio::test]
    async fn test_dht_peer_registration() {
        let node = DhtNode::new("node_a", "127.0.0.1", 19001, 9050, "secret");

        let peer_b = DhtPeerInfo {
            peer_id: "node_b".to_string(),
            host: "127.0.0.1".to_string(),
            dht_port: 19002,
            broker_port: 9051,
            fingerprint: Hypervector::new_random(),
            hosted_keys: Vec::new(),
        };

        node.register_peer(peer_b.clone()).await;

        let peers = node.known_peers().await;
        assert_eq!(peers.len(), 1);
        assert_eq!(peers[0].peer_id, "node_b");
    }

    #[tokio::test]
    async fn test_dht_store_and_retrieve() {
        let node = DhtNode::new("store_test", "127.0.0.1", 19003, 9052, "secret");

        let cluster = MemoryCluster {
            centroid: Hypervector::new_random(),
            entries: vec![],
            reverberation: 1.0,
            last_reinforced_tick: 0,
        };

        node.store_local(cluster.clone()).await;

        let key = DhtNode::compute_key(&cluster.centroid);
        let retrieved = node.retrieve_local(&key).await;
        assert!(retrieved.is_some());

        let dist = retrieved.unwrap().centroid.normalized_hamming_distance(&cluster.centroid);
        assert!(dist < 0.01);
    }

    #[tokio::test]
    async fn test_dht_find_closest_peers() {
        let node = DhtNode::new("node_a", "127.0.0.1", 19004, 9053, "secret");

        for i in 0..5 {
            let peer = DhtPeerInfo {
                peer_id: format!("node_{}", i),
                host: "127.0.0.1".to_string(),
                dht_port: 19010 + i as u16,
                broker_port: 9060 + i as u16,
                fingerprint: Hypervector::new_random(),
                hosted_keys: Vec::new(),
            };
            node.register_peer(peer).await;
        }

        let key = "some_test_key_for_routing";
        let closest = node.find_closest_peers(key, 3).await;
        assert_eq!(closest.len(), 3);
    }

    #[test]
    fn test_dht_compute_key() {
        let hv = Hypervector::new_random();
        let key1 = DhtNode::compute_key(&hv);
        let key2 = DhtNode::compute_key(&hv);
        assert_eq!(key1, key2); // Deterministic

        let hv2 = Hypervector::new_random();
        let key3 = DhtNode::compute_key(&hv2);
        assert_ne!(key1, key3); // Different for different vectors
    }

    #[test]
    fn test_hex_encode() {
        let input = [0xde, 0xad, 0xbe, 0xef];
        let hex = DhtNode::hex_encode(&input);
        assert_eq!(hex, "deadbeef");
    }
}
