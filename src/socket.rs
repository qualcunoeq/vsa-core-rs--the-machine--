use crate::{defense::DefenseSystem, Hypervector, VSABrain};
use crate::qa::QaEngine;
use std::sync::Arc;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::TcpListener;
use tokio::sync::RwLock;

#[cfg(test)]
mod tests {
    use super::*;

    /// Verify AdminSocketServer constructs without panicking.
    #[test]
    fn test_admin_socket_new() {
        let intent = Arc::new(RwLock::new(Hypervector::new_zero()));
        let defense = DefenseSystem::new(0);
        let brain = Arc::new(RwLock::new(VSABrain::new(0.43)));
        let qa = Arc::new(RwLock::new(QaEngine::new()));
        let _server = AdminSocketServer::new(intent, defense, brain, qa);
        // If we reach here, construction succeeded
    }
}

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
                                let _ = writer.write_all(b"Commands: OVERRIDE, QUERY, ASK, STORE, STORE_RULE, CHAIN, ABDUCE, ANALOGY, CULL, FACTS, SAVE, LOAD, EXIT\n").await;

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
                                                let mut qa_guard = qa_clone.write().await;
                                                let episode = qa_guard.answer_combined_episode(
                                                    format!("socket-ask-{}", std::time::SystemTime::now()
                                                        .duration_since(std::time::UNIX_EPOCH)
                                                        .unwrap_or_default()
                                                        .as_millis()),
                                                    question,
                                                );
                                                let answer = episode.answer.as_deref().unwrap_or("I do not know.");
                                                let response = format!("ANSWER: {} [confidence={:.2}]\n", answer, episode.confidence);
                                                drop(qa_guard);
                                                let _ = writer.write_all(response.as_bytes()).await;
                                                let _ = log_tx_clone.send(format!("ADMIN: ASK '{}' → '{}' (conf={:.2})", question, answer, episode.confidence));

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
                                                let (count, rules) = {
                                                    let qa_guard = qa_clone.read().await;
                                                    (qa_guard.fact_count(), qa_guard.rule_count())
                                                };
                                                let response = format!("FACTS: {} facts, {} causal rules in memory.\n", count, rules);
                                                let _ = writer.write_all(response.as_bytes()).await;

                                            } else if command.starts_with("CHAIN ") {
                                                let question = &command[6..];
                                                let mut qa_guard = qa_clone.write().await;
                                                let episode = qa_guard.answer_chain_episode(
                                                    format!("socket-chain-{}", std::time::SystemTime::now()
                                                        .duration_since(std::time::UNIX_EPOCH)
                                                        .unwrap_or_default()
                                                        .as_millis()),
                                                    question,
                                                );
                                                let answer = episode.answer.as_deref().unwrap_or("I do not know.");
                                                let response = format!("CHAIN: {} [confidence={:.2}]\n", answer, episode.confidence);
                                                drop(qa_guard);
                                                let _ = writer.write_all(response.as_bytes()).await;
                                                let _ = log_tx_clone.send(format!("ADMIN: CHAIN '{}' → '{}' (conf={:.2})", question, answer, episode.confidence));

                                            } else if command.starts_with("STORE_RULE ") {
                                                let rule_text = &command[11..];
                                                // Format: "IF subject verb object THEN subject verb object"
                                                let lower = rule_text.to_lowercase();
                                                if let Some(if_rest) = lower.strip_prefix("if ") {
                                                    if let Some(then_pos) = if_rest.find(" then ") {
                                                        let antecedent = &if_rest[..then_pos].trim();
                                                        let consequent = &if_rest[then_pos + 6..].trim();
                                                        let ante_triples = crate::nlp::extract_svo(antecedent);
                                                        let cons_triples = crate::nlp::extract_svo(consequent);
                                                        if !ante_triples.is_empty() && !cons_triples.is_empty() {
                                                            let mut qa_guard = qa_clone.write().await;
                                                            qa_guard.store_rule(
                                                                &ante_triples[0].subject, &ante_triples[0].verb, &ante_triples[0].object,
                                                                &cons_triples[0].subject, &cons_triples[0].verb, &cons_triples[0].object,
                                                                "admin_socket",
                                                            );
                                                            let response = format!("RULE STORED: {} {} {} → {} {} {}\n",
                                                                ante_triples[0].subject, ante_triples[0].verb, ante_triples[0].object,
                                                                cons_triples[0].subject, cons_triples[0].verb, cons_triples[0].object);
                                                            let _ = writer.write_all(response.as_bytes()).await;
                                                        } else {
                                                            let _ = writer.write_all(b"ERROR: Could not extract SVO from antecedent or consequent.\n").await;
                                                        }
                                                    } else {
                                                        let _ = writer.write_all(b"ERROR: Use format: IF subject verb object THEN subject verb object\n").await;
                                                    }
                                                } else {
                                                    let _ = writer.write_all(b"ERROR: Use format: IF subject verb object THEN subject verb object\n").await;
                                                }

                                            } else if command.starts_with("ABDUCE ") {
                                                let observation = &command[7..];
                                                let triples = crate::nlp::extract_svo(observation);
                                                if triples.is_empty() {
                                                    let _ = writer.write_all(b"ERROR: Could not extract SVO from observation.\n").await;
                                                } else {
                                                    let hypotheses = {
                                                        let qa_guard = qa_clone.read().await;
                                                        qa_guard.abduce(
                                                            &triples[0].subject,
                                                            &triples[0].verb,
                                                            &triples[0].object,
                                                        )
                                                    };
                                                    if hypotheses.is_empty() {
                                                        let _ = writer.write_all(b"NO HYPOTHESES: No known rule could have produced this observation.\n").await;
                                                    } else {
                                                        let mut response = format!("ABDUCTION: {} possible causes for '{} {} {}'\n",
                                                            hypotheses.len(),
                                                            triples[0].subject, triples[0].verb, triples[0].object,
                                                        );
                                                        for (i, (s, v, o, e)) in hypotheses.iter().take(5).enumerate() {
                                                            response.push_str(&format!("  {}. {} {} {} (E={:.4})\n", i + 1, s, v, o, e));
                                                        }
                                                        let _ = writer.write_all(response.as_bytes()).await;
                                                        let _ = log_tx_clone.send(format!(
                                                            "ADMIN: ABDUCE '{}' → {} hypotheses",
                                                            observation, hypotheses.len()
                                                        ));
                                                    }
                                                }

                                            } else if command.starts_with("ANALOGY ") {
                                                let query = &command[8..];
                                                let triples = crate::nlp::extract_svo(query);
                                                if triples.is_empty() {
                                                    let _ = writer.write_all(b"ERROR: Could not extract SVO from query.\n").await;
                                                } else {
                                                    let result = {
                                                        let qa_guard = qa_clone.read().await;
                                                        qa_guard.analogical_reason_chain(
                                                            &triples[0].subject,
                                                            &triples[0].verb,
                                                            &triples[0].object,
                                                        )
                                                    };
                                                    match result {
                                                        Some((s, v, o, e)) => {
                                                            let response = format!("ANALOGY: {} {} {} (E={:.4})\n", s, v, o, e);
                                                            let _ = writer.write_all(response.as_bytes()).await;
                                                        }
                                                        None => {
                                                            let _ = writer.write_all(b"ANALOGY: No analogical match found.\n").await;
                                                        }
                                                    }
                                                }

                                            } else if command == "CULL" {
                                                let culled = {
                                                    let mut qa_guard = qa_clone.write().await;
                                                    qa_guard.cull_low_confidence_rules(0.20)
                                                };
                                                let response = format!("CULLED: {} low-confidence rules removed.\n", culled);
                                                let _ = writer.write_all(response.as_bytes()).await;

                                            } else if command == "SAVE" {
                                                let result = {
                                                    let qa_guard = qa_clone.read().await;
                                                    qa_guard.save_to_file("data/qa_memory.json")
                                                };
                                                match result {
                                                    Ok(()) => {
                                                        let _ = writer.write_all(b"SAVED: QA memory persisted to data/qa_memory.json\n").await;
                                                        let _ = log_tx_clone.send("ADMIN: QA memory saved.".to_string());
                                                    }
                                                    Err(e) => {
                                                        let _ = writer.write_all(format!("ERROR: Save failed: {}\n", e).as_bytes()).await;
                                                    }
                                                }

                                            } else if command == "LOAD" {
                                                let result = QaEngine::load_from_file("data/qa_memory.json");
                                                match result {
                                                    Ok(loaded) => {
                                                        let mut qa_guard = qa_clone.write().await;
                                                        *qa_guard = loaded;
                                                        let count = qa_guard.fact_count();
                                                        let _ = writer.write_all(format!("LOADED: {} facts from data/qa_memory.json\n", count).as_bytes()).await;
                                                        let _ = log_tx_clone.send(format!("ADMIN: QA memory loaded ({} facts).", count));
                                                    }
                                                    Err(e) => {
                                                        let _ = writer.write_all(format!("ERROR: Load failed: {}\n", e).as_bytes()).await;
                                                    }
                                                }

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
