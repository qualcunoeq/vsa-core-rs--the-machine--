use crate::Hypervector;
use scraper::{Html, Selector};
use std::collections::HashSet;
use std::sync::Arc;
use tokio::sync::RwLock;
use tokio::time::{sleep, Duration};

/// VSAForager — Semantic web crawler with structured intent pursuit.
///
/// Instead of scoring links against the raw intent vector, the upgraded
/// forager decodes structured intents (I = A ⊕ P) to isolate the target
/// **parameter** P, and scores link text against that semantic anchor.
///
/// Facts ingested from pages are also evaluated against P; those with
/// high resonance (≥ 0.75) are tagged as "mission_critical" and
/// fast-tracked into permanent memory.
pub struct VSAForager {
    pub intent: Arc<RwLock<Hypervector>>,
    pub current_url: Arc<RwLock<String>>,
    pub visited: HashSet<String>,
    pub client: reqwest::Client,
    pub crawl_speed_ms: u64,
    pub brain: Option<Arc<RwLock<crate::VSABrain>>>,

    // ── Semantic targeting (upgraded) ─────────────────────────────
    /// The unbound parameter vector P from the current structured
    /// intent I = A ⊕ P.  Updated externally by the AutonomyDrive loop.
    /// When `None`, the forager falls back to raw-intent scoring.
    pub target_parameter: Arc<RwLock<Option<Hypervector>>>,
}

impl VSAForager {
    pub fn new(initial_intent: Hypervector, start_url: String, crawl_speed_ms: u64) -> Self {
        VSAForager {
            intent: Arc::new(RwLock::new(initial_intent)),
            current_url: Arc::new(RwLock::new(start_url)),
            visited: HashSet::new(),
            client: reqwest::Client::builder()
                .timeout(Duration::from_secs(5))
                .user_agent("The-Machine-VSA-Forager/1.0")
                .build()
                .unwrap(),
            crawl_speed_ms,
            brain: None,
            target_parameter: Arc::new(RwLock::new(None)),
        }
    }

    /// Resolves standard relative URLs to absolute URLs
    pub fn resolve_url(base: &str, relative: &str) -> Option<String> {
        let relative = relative.trim();
        if relative.is_empty() || relative.starts_with('#') || relative.starts_with("javascript:") {
            return None;
        }
        if relative.starts_with("http://") || relative.starts_with("https://") {
            return Some(relative.to_string());
        }
        if relative.starts_with("//") {
            let protocol = if base.starts_with("https:") {
                "https:"
            } else {
                "http:"
            };
            return Some(format!("{}{}", protocol, relative));
        }

        let base_parts: Vec<&str> = base.split("://").collect();
        if base_parts.len() < 2 {
            return None;
        }
        let protocol = base_parts[0];
        let rest = base_parts[1];

        if relative.starts_with('/') {
            let host = rest.split('/').next().unwrap_or(rest);
            return Some(format!("{}://{}{}", protocol, host, relative));
        }

        let mut path_parts: Vec<&str> = rest.split('/').collect();
        if !rest.ends_with('/') && path_parts.len() > 1 {
            path_parts.pop(); // Remove file name
        }
        let dir = path_parts.join("/");
        Some(format!(
            "{}://{}/{}",
            protocol,
            dir.trim_end_matches('/'),
            relative.trim_start_matches('/')
        ))
    }

    /// Fetches HTML, extracts links, scores them semantically, and transitions
    pub async fn step(&mut self) -> Result<(String, f64, usize), String> {
        let url = {
            let url_guard = self.current_url.read().await;
            url_guard.clone()
        };

        self.visited.insert(url.clone());

        // Fetch URL content
        let response = self
            .client
            .get(&url)
            .send()
            .await
            .map_err(|e| format!("Request failed: {}", e))?;

        let html_content = response
            .text()
            .await
            .map_err(|e| format!("Failed to read body: {}", e))?;

        // ── 1. Scrape paragraphs and ingest facts ─────────────────
        let paragraphs = {
            let document = Html::parse_document(&html_content);
            let p_selector = Selector::parse("p").unwrap();
            let mut paragraphs = Vec::new();
            for element in document.select(&p_selector) {
                let text = element
                    .text()
                    .collect::<Vec<_>>()
                    .join(" ")
                    .trim()
                    .to_string();
                if !text.is_empty() {
                    paragraphs.push(text);
                }
            }
            paragraphs
        };

        let mut sentences: Vec<(String, f64)> = Vec::new(); // (text, resonance)
        for p in paragraphs {
            let mut start = 0;
            let chars: Vec<char> = p.chars().collect();
            for i in 0..chars.len() {
                let c = chars[i];
                if c == '.' || c == '?' || c == '!' {
                    if i + 1 == chars.len()
                        || (i + 1 < chars.len() && chars[i + 1].is_whitespace())
                    {
                        let sentence: String = chars[start..=i].iter().collect();
                        let cleaned = sentence.trim().to_string();
                        if cleaned.split_whitespace().count() >= 3 {
                            // ── Semantic resonance against target ──
                            let resonance = self.compute_resonance(&cleaned);
                            sentences.push((cleaned, resonance));
                        }
                        start = i + 1;
                    }
                }
            }
            if start < chars.len() {
                let sentence: String = chars[start..].iter().collect();
                let cleaned = sentence.trim().to_string();
                if cleaned.split_whitespace().count() >= 3 {
                    let resonance = self.compute_resonance(&cleaned);
                    sentences.push((cleaned, resonance));
                }
            }
        }

        // ── 2. Ingest facts with mission-critical flagging ─────────
        let mut ingested_count = 0;
        if let Some(ref brain_arc) = self.brain {
            let mut brain_guard = brain_arc.write().await;

            for (sentence, resonance) in &sentences {
                if sentence.len() < 15 || sentence.len() > 250 {
                    continue;
                }

                let sentence_vector = Hypervector::encode_sentence(sentence);
                let source_url_vector = Hypervector::encode_text_ngram(&url, 3);
                let fact_vector = sentence_vector.bitwise_xor(&source_url_vector);

                let mut metadata = std::collections::HashMap::new();
                metadata.insert("source_url".to_string(), url.clone());
                metadata.insert("text".to_string(), sentence.clone());
                metadata.insert("type".to_string(), "web_scraped_fact".to_string());

                // ── Mission Critical Objective ──
                // If resonance with the target parameter ≥ 0.75,
                // fast-track this fact by tagging it for priority
                // consolidation.
                if *resonance >= 0.75 {
                    metadata.insert(
                        "priority".to_string(),
                        "mission_critical".to_string(),
                    );
                    // Also log it as a high-value finding
                    let label = format!("CRITICAL: {}", sentence);
                    brain_guard.add_transient_fact(fact_vector, &label, metadata);
                } else {
                    brain_guard.add_transient_fact(fact_vector, sentence, metadata);
                }

                ingested_count += 1;
                if ingested_count >= 15 {
                    break;
                }
            }
        }

        // ── 3. Parse candidates for URL transition ────────────────
        let candidates = {
            let document = Html::parse_document(&html_content);
            let selector = Selector::parse("a").unwrap();

            let mut candidates = Vec::new();
            for element in document.select(&selector) {
                if let Some(href) = element.value().attr("href") {
                    if let Some(resolved) = Self::resolve_url(&url, href) {
                        let mut text = element
                            .text()
                            .collect::<Vec<_>>()
                            .join(" ")
                            .trim()
                            .to_string();
                        if text.is_empty() {
                            text = resolved.split('/').last().unwrap_or("link").to_string();
                        }
                        candidates.push((resolved, text));
                    }
                }
            }
            candidates
        };

        if candidates.is_empty() {
            return Err("No links found on page to transition to".to_string());
        }

        // ── 4. Semantic Anchor Scoring ────────────────────────────
        let current_intent = {
            let intent_guard = self.intent.read().await;
            *intent_guard
        };

        // Determine the semantic target: prefer the decoded parameter,
        // fall back to the raw intent vector.
        let semantic_target = {
            let target_guard = self.target_parameter.read().await;
            target_guard.unwrap_or(current_intent)
        };

        let mut best_url = None;
        let mut min_distance = 1.0;
        let mut scored_count = 0;

        for (resolved_url, anchor_text) in candidates {
            if self.visited.contains(&resolved_url) {
                continue;
            }

            let action_vector = Hypervector::encode_text_ngram(&anchor_text, 3);

            // Score against the semantic target (parameter vector P)
            // instead of the raw composite intent.
            let distance = action_vector.normalized_hamming_distance(&semantic_target);

            scored_count += 1;
            if distance < min_distance {
                min_distance = distance;
                best_url = Some(resolved_url);
            }
        }

        if best_url.is_none() {
            self.visited.clear();
            return Err("Dead end or all links visited. Resetting crawl history.".to_string());
        }

        let next_url = best_url.unwrap();

        {
            let mut url_guard = self.current_url.write().await;
            *url_guard = next_url.clone();
        }

        Ok((next_url, min_distance, scored_count))
    }

    /// Evaluate the semantic resonance of a text string against the
    /// current target parameter vector using Hamming similarity.
    fn compute_resonance(&self, text: &str) -> f64 {
        // If no target parameter is set, resonance is neutral (0.5)
        let target_guard = self.target_parameter.try_read().ok();
        let target = match target_guard {
            Some(ref g) => g.as_ref().copied(),
            None => return 0.5,
        };

        match target {
            Some(param) => {
                let text_hv = Hypervector::encode_sentence(text);
                1.0 - text_hv.normalized_hamming_distance(&param)
            }
            None => 0.5,
        }
    }

    /// Continuous run loop for the forager
    pub async fn run_loop(
        forager_arc: Arc<tokio::sync::Mutex<Self>>,
        log_tx: tokio::sync::mpsc::UnboundedSender<String>,
    ) {
        loop {
            let speed = {
                let guard = forager_arc.lock().await;
                guard.crawl_speed_ms
            };
            sleep(Duration::from_millis(speed)).await;

            let mut guard = forager_arc.lock().await;
            match guard.step().await {
                Ok((next_url, dist, count)) => {
                    let target_info = {
                        let tg = guard.target_parameter.read().await;
                        if tg.is_some() {
                            " [semantic]"
                        } else {
                            " [raw]"
                        }
                    };
                    let log_msg = format!(
                        "CRAWLER: Transited to {} | Hamming Dist: {:.4} | Parsed {} links{}",
                        next_url, dist, count, target_info
                    );
                    let _ = log_tx.send(log_msg);
                }
                Err(e) => {
                    let log_msg = format!("CRAWLER WARN: {}", e);
                    let _ = log_tx.send(log_msg);
                    sleep(Duration::from_secs(2)).await;
                }
            }
        }
    }
}
