use crate::{defense::DefenseSystem, Hypervector, VSABrain};
use crate::qa::QaEngine;
use std::sync::Arc;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::TcpListener;
use tokio::sync::RwLock;

pub struct AdminSocketServer {
    intent: Arc<RwLock<Hypervector>>,
    defense: DefenseSystem,
    brain: Arc<RwLock<VSABrain>>,
    qa: Arc<RwLock<QaEngine>>,
}

impl AdminSocketServer {
    pub fn new(
        intent: Arc<RwLock<Hypervector>>,
        defense: DefenseSystem,
        brain: Arc<RwLock<VSABrain>>,
        qa: Arc<RwLock<QaEngine>>,
    ) -> Self {
        AdminSocketServer { intent, defense, brain, qa }
    }

    pub async fn run(
        &self,
        log_tx: tokio::sync::mpsc::UnboundedSender<String>,
    ) -> Result<(), String> {
        let mut current_port = { *self.defense.active_port.read().await };
        let mut listener = TcpListener::bind(format!("127.0.0.1:{}", current_port))
            .await
            .map_err(|e| format!("Failed to bind TCP listener to {}: {}", current_port, e))?;

        let msg = format!(
            "ADMIN SOCKET: Listening on tcp://127.0.0.1:{}",
            current_port
        );
        let _ = log_tx.send(msg);

        loop {
            tokio::select! {
                accept_res = listener.accept() => {
                    match accept_res {
                        Ok((mut socket, _)) => {
                            let intent_clone = Arc::clone(&self.intent);
                            let log_tx_clone = log_tx.clone();
                            let defense_clone = self.defense.clone();
                            let brain_clone = Arc::clone(&self.brain);
                            let qa_clone = Arc::clone(&self.qa);

                            tokio::spawn(async move {
                                let (reader, mut writer) = socket.split();
                                let mut buf_reader = BufReader::new(reader);
                                let mut line = String::new();

                                let _ = writer.write_all(b"--- THE MACHINE ADMIN INTERFACE ---\n").await;
                                let _ = writer.write_all(b"Commands: OVERRIDE <seed>, QUERY <text>, ASK <question>, EXIT\n").await;

                                loop {
                                    let threat_val = *defense_clone.threat_level.read().await;
                                    let prompt = format!("THE MACHINE [Threat: {:.2}] > ", threat_val);
                                    let _ = writer.write_all(prompt.as_bytes()).await;

                                    line.clear();
                                    match buf_reader.read_line(&mut line).await {
                                        Ok(0) => break,
                                        Ok(_) => {
                                            let command = line.trim();
                                            if command.is_empty() { continue; }

                                            if command.starts_with("OVERRIDE ") {
                                                let seed = &command[9..];
                                                let seed_vector = Hypervector::encode_text_ngram(seed, 3);
                                                let mut intent_guard = intent_clone.write().await;
                                                *intent_guard = intent_guard.bitwise_xor(&seed_vector);

                                                let response = format!("SUCCESS: Exogenous override seed bound.\n");
                                                let _ = writer.write_all(response.as_bytes()).await;
                                                let _ = log_tx_clone.send(format!("ADMIN: Override seed '{}' bound.", seed));

                                            } else if command.starts_with("QUERY ") {
                                                let question = &command[6..];
                                                let query_vector = Hypervector::encode_sentence(question);

                                                let (match_label, similarity, metadata) = {
                                                    let brain_guard = brain_clone.read().await;
                                                    brain_guard.query_dejavu(&query_vector)
                                                };

                                                let mut response = String::new();
                                                if let Some(label) = match_label {
                                                    response.push_str(&format!("MATCH FOUND (Similarity: {:.4}):\n", similarity));
                                                    response.push_str(&format!("  Fact: {}\n", label));
                                                    if let Some(src) = metadata.get("source_url") {
                                                        response.push_str(&format!("  Source: {}\n", src));
                                                    }
                                                } else {
                                                    response.push_str("NO MATCHING SEMANTIC RECORD FOUND.\n");
                                                }
                                                let _ = writer.write_all(response.as_bytes()).await;

                                            } else if command.starts_with("ASK ") {
                                                let question = &command[4..];
                                                let answer = {
                                                    let qa_guard = qa_clone.read().await;
                                                    qa_guard.answer_combined(question)
                                                };
                                                let response = format!("ANSWER: {}\n", answer);
                                                let _ = writer.write_all(response.as_bytes()).await;
                                                let _ = log_tx_clone.send(format!("ADMIN: ASK '{}' → '{}'", question, answer));

                                            } else if command.starts_with("STORE ") {
                                                let fact = &command[6..];
                                                // Store a fact from raw text
                                                let triples = crate::nlp::extract_svo(fact);
                                                if triples.is_empty() {
                                                    let _ = writer.write_all(b"ERROR: Could not extract any facts from that text.\n").await;
                                                } else {
                                                    let mut qa_guard = qa_clone.write().await;
                                                    qa_guard.store_triples(&triples, "admin_socket");
                                                    let response = format!("STORED: {} fact(s) from '{}'\n", triples.len(), fact);
                                                    let _ = writer.write_all(response.as_bytes()).await;
                                                    let _ = log_tx_clone.send(format!("ADMIN: STORE '{}' ({} triples)", fact, triples.len()));
                                                }

                                            } else if command == "FACTS" {
                                                let count = {
                                                    let qa_guard = qa_clone.read().await;
                                                    qa_guard.fact_count()
                                                };
                                                let response = format!("FACTS: {} facts in memory.\n", count);
                                                let _ = writer.write_all(response.as_bytes()).await;

                                            } else if command == "EXIT" || command == "QUIT" {
                                                let _ = writer.write_all(b"Terminating session.\n").await;
                                                break;
                                            } else {
                                                defense_clone.increment_threat(0.15).await;
                                                let _ = writer.write_all(b"ERROR: Command unrecognized. Threat level incremented.\n").await;
                                                let _ = log_tx_clone.send("WARNING: Unrecognized command on socket. Incrementing threat level.".to_string());
                                            }
                                        }
                                        Err(_) => break,
                                    }
                                }
                            });
                        }
                        Err(e) => {
                            let _ = log_tx.send(format!("ADMIN SOCKET ERROR: {}", e));
                        }
                    }
                }
                _ = tokio::time::sleep(tokio::time::Duration::from_millis(500)) => {
                    let active = *self.defense.active_port.read().await;
                    if active != current_port {
                        let msg = format!("ADMIN SOCKET: Port migration detected. Relocating from {} to {}...", current_port, active);
                        let _ = log_tx.send(msg);
                        current_port = active;
                        listener = TcpListener::bind(format!("127.0.0.1:{}", current_port))
                            .await
                            .map_err(|e| format!("Failed to bind TCP listener to {}: {}", current_port, e))?;
                        let msg = format!("ADMIN SOCKET: Bound to new port tcp://127.0.0.1:{}", current_port);
                        let _ = log_tx.send(msg);
                    }
                }
            }
        }
    }
}
