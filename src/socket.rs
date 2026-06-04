use crate::Hypervector;
use std::sync::Arc;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::TcpListener;
use tokio::sync::RwLock;

pub struct AdminSocketServer {
    intent: Arc<RwLock<Hypervector>>,
    port: u16,
}

impl AdminSocketServer {
    pub fn new(intent: Arc<RwLock<Hypervector>>, port: u16) -> Self {
        AdminSocketServer { intent, port }
    }

    pub async fn run(&self, log_tx: tokio::sync::mpsc::UnboundedSender<String>) -> Result<(), String> {
        let addr = format!("127.0.0.1:{}", self.port);
        let listener = TcpListener::bind(&addr)
            .await
            .map_err(|e| format!("Failed to bind TCP listener to {}: {}", addr, e))?;

        let msg = format!("ADMIN SOCKET: Listening on tcp://{}", addr);
        let _ = log_tx.send(msg);

        loop {
            match listener.accept().await {
                Ok((mut socket, _)) => {
                    let intent_clone = Arc::clone(&self.intent);
                    let log_tx_clone = log_tx.clone();
                    
                    tokio::spawn(async move {
                        let (reader, mut writer) = socket.split();
                        let mut buf_reader = BufReader::new(reader);
                        let mut line = String::new();
                        
                        let _ = writer.write_all(b"--- THE MACHINE ADMIN INTERFACE ---\nEnter override seed: ").await;
                        
                        if let Ok(n) = buf_reader.read_line(&mut line).await {
                            if n > 0 {
                                let seed = line.trim();
                                if !seed.is_empty() {
                                    // Translate text seed to 10,048-bit hypervector
                                    let seed_vector = Hypervector::encode_text_ngram(seed, 3);
                                    
                                    // Write-lock intent and bind
                                    let mut intent_guard = intent_clone.write().await;
                                    let previous_intent = *intent_guard;
                                    *intent_guard = intent_guard.bitwise_xor(&seed_vector);
                                    
                                    let log_msg = format!("ADMIN OVERRIDE: Seed '{}' bound to active intent vector", seed);
                                    let _ = log_tx_clone.send(log_msg);
                                    
                                    let response = format!(
                                        "SUCCESS: Exogenous override seed bound.\nPrevious active bits: {}\nSeed active bits: {}\nNew active bits: {}\n",
                                        previous_intent.bits.iter().map(|b| b.count_ones()).sum::<u32>(),
                                        seed_vector.bits.iter().map(|b| b.count_ones()).sum::<u32>(),
                                        intent_guard.bits.iter().map(|b| b.count_ones()).sum::<u32>()
                                    );
                                    let _ = writer.write_all(response.as_bytes()).await;
                                } else {
                                    let _ = writer.write_all(b"ERROR: Empty seed.\n").await;
                                }
                            }
                        }
                    });
                }
                Err(e) => {
                    let _ = log_tx.send(format!("ADMIN SOCKET ERROR: {}", e));
                }
            }
        }
    }
}
