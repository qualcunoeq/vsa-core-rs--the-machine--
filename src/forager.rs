use crate::compression::{CountingBloomFilter, CappedVecDeque};
use crate::resonator::ResonatorVocabulary;
use crate::Hypervector;
use crate::analogy::{self, RoleDictionary,
    AnalogicalIndex, MetaIndex};
use scraper::{Html, Selector};
use std::collections::{HashMap, HashSet};
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
    /// ██ FIX v2.6: Replaced HashSet<String> with fixed-memory Bloom filter ██
    /// Uses ~4 MB regardless of how many URLs are visited.
    /// ~0.1% false positive rate for ~1M URLs.
    pub visited: CountingBloomFilter,
    pub client: reqwest::Client,
    pub crawl_speed_ms: u64,
    pub brain: Option<Arc<RwLock<crate::VSABrain>>>,

    // ── Semantic targeting (upgraded) ─────────────────────────────
    /// The unbound parameter vector P from the current structured
    /// intent I = A ⊕ P.  Updated externally by the AutonomyDrive loop.
    /// When `None`, the forager falls back to raw-intent scoring.
    pub target_parameter: Arc<RwLock<Option<Hypervector>>>,

    // ── Dynamic vocabulary learning ───────────────────────────────
    /// Shared vocabulary reference.  When `Some`, the forager dynamically
    /// registers novel multi-word terms from mission-critical scraped
    /// content, letting the ontology grow from observation.
    pub vocab: Option<Arc<RwLock<ResonatorVocabulary>>>,

    // ── IDF-based semantic weighting ──────────────────────────────
    /// Document frequency counter: word → number of pages containing it.
    /// Updated after each successful crawl step.
    /// Used to compute inverse-document-frequency weights for
    /// `encode_text_weighted`, replacing the static stop-word heuristic.
    pub doc_frequency: HashMap<String, usize>,
    /// Total number of documents (pages) processed so far.
    pub total_documents: usize,

    // ── Layers 3–5 integration: SVO frame store ────────────────────
    /// Shared AnalogicalIndex — stores SVO frames from scraped text.
    pub primary: Option<Arc<RwLock<AnalogicalIndex>>>,
    /// Shared MetaIndex — epistemic tracking and curiosity.
    pub meta: Option<Arc<RwLock<MetaIndex>>>,
    /// Monotonic frame counter for label generation.
    pub frame_counter: Option<Arc<RwLock<usize>>>,

    // ── Curiosity-driven search queue ──────────────────────────────
    /// Queue of seed URLs for curiosity-driven exploration.
    /// When the forager reaches a dead end, it pops a seed URL from
    /// this queue and starts crawling from there.  The agent loop
    /// pushes DuckDuckGo search URLs (decoded from curiosity targets).
    /// ██ FIX v2.6: Capped at MAX_SEED_URLS to prevent unbounded growth.
    pub seed_urls: Arc<RwLock<CappedVecDeque<String>>>,
}

impl VSAForager {
    /// Maximum number of seed URLs to queue before evicting oldest.
    const MAX_SEED_URLS: usize = 50_000;

    /// ██ FIX v2.6: Decay interval for doc_frequency (every 200 documents) ██
    const DOC_FREQ_DECAY_INTERVAL: usize = 200;

    /// ██ FIX v2.6: Minimum doc frequency to retain after decay ██
    const DOC_FREQ_MIN_RETAIN: usize = 2;

    pub fn new(initial_intent: Hypervector, start_url: String, crawl_speed_ms: u64) -> Self {
        VSAForager {
            intent: Arc::new(RwLock::new(initial_intent)),
            current_url: Arc::new(RwLock::new(start_url)),
            visited: CountingBloomFilter::default_large(),
            client: reqwest::Client::builder()
                .timeout(Duration::from_secs(15))
                .user_agent("Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36")
                .build()
                .unwrap(),
            crawl_speed_ms,
            brain: None,
            target_parameter: Arc::new(RwLock::new(None)),
            vocab: None,
            doc_frequency: HashMap::new(),
            total_documents: 0,
            primary: None,
            meta: None,
            frame_counter: None,
            seed_urls: Arc::new(RwLock::new(CappedVecDeque::new(Self::MAX_SEED_URLS))),
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

        self.visited.insert(&url);

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

        // ── LAYERS 3-5 INTEGRATION: Extract SVO frames from scraped text ──
        // Bridge the gap: scraped text → SVO triples → AnalogicalIndex frames.
        if let Some(ref primary_arc) = self.primary {
            if let Some(ref meta_arc) = self.meta {
                if let Some(ref fc_arc) = self.frame_counter {
                    let combined_text = paragraphs.join(" ");
                    if combined_text.len() > 30 {
                        let _result = crate::bridge::ingest_text(
                            &combined_text,
                            &mut *primary_arc.write().await,
                            &mut *meta_arc.write().await,
                            0.05,
                            &mut *fc_arc.write().await,
                        );
                    }
                }
            }
        }

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

                    // ── Dynamic vocabulary learning ─────────────
                    // Extract novel content words from the high-value
                    // sentence and register them so the ontology grows
                    // from observation.
                    if let Some(ref vocab_arc) = self.vocab {
                        let mut vocab_guard = vocab_arc.write().await;
                        for word in sentence.split_whitespace() {
                            let clean = word.trim_matches(|c: char| c.is_ascii_punctuation());
                            if clean.len() >= 4
                                && !clean.chars().all(|c| c.is_ascii_digit())
                            {
                                vocab_guard.learn_term(clean);
                            }
                        }
                    }

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
            if self.visited.maybe_contains(&resolved_url) {
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
            // ── Check for curiosity-driven seed URLs ────────────
            let seeds = self.seed_urls.read().await;
            if !seeds.is_empty() {
                let seed = seeds.get(0).cloned().unwrap_or_default();
                drop(seeds);
                self.seed_urls.write().await.pop_front();
                self.visited.clear();
                *self.current_url.write().await = seed.clone();
                return Ok((seed, 0.0, 0));
            }
            self.visited.clear();
            return Err("Dead end or all links visited. Resetting crawl history.".to_string());
        }

        let next_url = best_url.unwrap();

        // ── Update IDF weights from the just-fetched page ────────
        // Extract all unique words from the HTML content and update
        // the document frequency counter so future calls to
        // encode_text_weighted reflect the true crawl corpus.
        {
            let text = Html::parse_document(&html_content);
            let p_selector = Selector::parse("p").unwrap();
            let mut page_words: HashSet<String> = HashSet::new();
            for element in text.select(&p_selector) {
                for word in element.text().collect::<String>().split_whitespace() {
                    let clean = word
                        .trim_matches(|c: char| c.is_ascii_punctuation())
                        .to_lowercase();
                    if clean.len() >= 2 {
                        page_words.insert(clean);
                    }
                }
            }
            for w in page_words {
                *self.doc_frequency.entry(w).or_insert(0) += 1;
            }
            self.total_documents += 1;

            // ██ FIX v2.6: Periodic doc_frequency decay ██
            // Every DOC_FREQ_DECAY_INTERVAL documents, apply exponential
            // decay to all entries and evict those below the retain threshold.
            // This prevents unbounded growth of the HashMap.
            if self.total_documents % Self::DOC_FREQ_DECAY_INTERVAL == 0 {
                let decay_factor: f64 = 0.85; // gentle decay
                self.doc_frequency.retain(|_, count| {
                    let new_count = (*count as f64 * decay_factor).round() as usize;
                    *count = new_count;
                    new_count >= Self::DOC_FREQ_MIN_RETAIN
                });
            }
        }

        {
            let mut url_guard = self.current_url.write().await;
            *url_guard = next_url.clone();
        }

        Ok((next_url, min_distance, scored_count))
    }

    /// Evaluate semantic resonance using **IDF-weighted bundling**.
    fn compute_resonance(&self, text: &str) -> f64 {
        let target_guard = self.target_parameter.try_read().ok();
        let target = match target_guard {
            Some(ref g) => g.as_ref().copied(),
            None => return 0.5,
        };

        match target {
            Some(param) => {
                let weighted_hv = self.encode_text_weighted(text);
                1.0 - weighted_hv.normalized_hamming_distance(&param)
            }
            None => 0.5,
        }
    }

    /// IDF-weighted sentence encoder.
    ///
    /// Uses the inverse document frequency log-ratio computed from the
    /// forager's own crawl history.  Words appearing in nearly every
    /// document get ~1 copy (grammatical glue); words found in only a
    /// few documents get up to ~10 copies (semantically distinctive).
    fn encode_text_weighted(&self, text: &str) -> Hypervector {
        let words: Vec<&str> = text
            .split_whitespace()
            .map(|w| w.trim_matches(|c: char| c.is_ascii_punctuation()))
            .filter(|w| !w.is_empty() && w.len() > 1)
            .collect();

        if words.is_empty() {
            return Hypervector::new_zero();
        }

        // ── Precompute a small on-the-fly IDF map to avoid repeated
        // hash lookups for duplicate words in the same sentence.
        let total = if self.total_documents > 0 {
            self.total_documents
        } else {
            1
        };

        let mut word_vectors: Vec<Hypervector> = Vec::with_capacity(words.len() * 4);

        for (i, word) in words.iter().enumerate() {
            let word_hv = Hypervector::encode_text_ngram(word, 3);
            let rotated = word_hv.rotate_left(i * 7);

            // IDF(t) = ln( total_documents / (1 + doc_frequency(t)) )
            // Clamped so rare words get at most 10× influence, common
            // words get at least 1×.
            let df = self.doc_frequency.get(&word.to_lowercase()).copied().unwrap_or(0);
            let idf = ((total as f64) / (1.0 + df as f64)).ln().max(0.0);
            let copies = (idf * 2.0).round().max(1.0).min(10.0) as usize;

            for _ in 0..copies {
                word_vectors.push(rotated);
            }
        }

        let refs: Vec<&Hypervector> = word_vectors.iter().collect();
        Hypervector::bundle(&refs)
    }

    /// Convert a curiosity target hypervector into a structured forager intent.
    ///
    /// The bridge from the epistemic self-model (MetaIndex) to the perception
    /// system (VSAForager). Given a curiosity target HV (which represents a
    /// concept the system believes should exist but hasn't observed):
    ///
    /// 1. Factorize the target against the forager's vocabulary using the
    ///    resonator, recovering (subject, verb, object) strings
    /// 2. If factorizable → set a structured intent with a param P for the
    ///    object, enabling semantic link scoring
    /// 3. If not factorizable → set intent to the raw target HV (lower
    ///    confidence, broader match)
    /// 4. Returns `true` if the target was factorizable (high-confidence
    ///    intent), `false` if fallback to raw intent
    ///
    /// ## Why the bridge is hard
    ///
    /// A curiosity target that doesn't factorize cleanly is a genuine gap —
    /// something the system knows should exist but cannot articulate. The
    /// fallback to raw intent is a best-effort: the forager will score links
    /// against the unfactorized vector, which may produce broad but shallow
    /// matches. This is the closest approximation to "curiosity about the
    /// inexpressible" the architecture currently supports.
    pub fn set_curiosity_intent(
        &self,
        target_hv: &Hypervector,
        roles: &RoleDictionary,
        vocab: &ResonatorVocabulary,
        subj_candidates: &[String],
        verb_candidates: &[String],
        obj_candidates: &[String],
    ) -> bool {
        // Attempt to factorize the target into known terms
        let factorization = analogy::factorize_triple(
            target_hv,
            roles,
            vocab,
            subj_candidates,
            verb_candidates,
            obj_candidates,
            30,
        );

        match factorization {
            Some((subj_str, verb_str, obj_str, energy)) if energy >= 0.65 => {
                // High-confidence factorization — set structured intent.
                // Intent I = A ⊕ P where A = verb, P = object.
                // The forager scores links against P.
                let subj_hv = vocab.get_vector(&subj_str)
                    .cloned()
                    .unwrap_or_else(|| Hypervector::encode_text_ngram(&subj_str, 3));
                let verb_hv = vocab.get_vector(&verb_str)
                    .cloned()
                    .unwrap_or_else(|| Hypervector::encode_text_ngram(&verb_str, 3));
                let obj_hv = vocab.get_vector(&obj_str)
                    .cloned()
                    .unwrap_or_else(|| Hypervector::encode_text_ngram(&obj_str, 3));

                let intent_hv = roles.bind_triple(&subj_hv, &verb_hv, &obj_hv);

                // Set both the composite intent and the target parameter
                {
                    let mut intent_guard = self.intent.blocking_write();
                    *intent_guard = intent_hv;
                }
                {
                    let mut param_guard = self.target_parameter.blocking_write();
                    *param_guard = Some(obj_hv);
                }

                true
            }
            _ => {
                // Not factorizable — fallback to raw intent.
                // The forager will score links against the unfactorized
                // vector. This is better than nothing but less precise.
                {
                    let mut intent_guard = self.intent.blocking_write();
                    *intent_guard = *target_hv;
                }
                {
                    let mut param_guard = self.target_parameter.blocking_write();
                    *param_guard = None; // fall back to raw intent scoring
                }

                false
            }
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
                    // Try to pop from seed queue or skip to next unvisited link.
                    // Otherwise we'd retry the same failing URL forever.
                    let seeds = guard.seed_urls.read().await;
                    if !seeds.is_empty() {
                        let seed = seeds.get(0).cloned().unwrap_or_default();
                        drop(seeds);
                        guard.seed_urls.write().await.pop_front();
                        guard.visited.clear();
                        *guard.current_url.write().await = seed.clone();
                        let _ = log_tx.send(format!("CRAWLER: Failover to seed URL: {}", seed));
                    } else {
                        drop(seeds);
                        // Clear visited so the forager tries a different link
                        // (the current URL is still in visited, but clearing
                        // the set lets us retry other links on the same page).
                        guard.visited.clear();
                        let _ = log_tx.send("CRAWLER: Cleared visited set — will retry with fresh links.".to_string());
                    }
                    sleep(Duration::from_secs(2)).await;
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_resolve_url_absolute_unchanged() {
        let url = VSAForager::resolve_url("http://example.com/page", "http://other.com/img.png");
        assert_eq!(url, Some("http://other.com/img.png".to_string()));
    }

    #[test]
    fn test_resolve_url_relative_path() {
        let url = VSAForager::resolve_url("http://example.com/page", "img.png");
        assert_eq!(url, Some("http://example.com/img.png".to_string()));
    }

    #[test]
    fn test_resolve_url_root_relative() {
        let url = VSAForager::resolve_url("http://example.com/page", "/images/pic.jpg");
        assert_eq!(url, Some("http://example.com/images/pic.jpg".to_string()));
    }

    #[test]
    fn test_resolve_url_protocol_relative() {
        let url = VSAForager::resolve_url("https://example.com/page", "//cdn.example.com/img.png");
        assert_eq!(url, Some("https://cdn.example.com/img.png".to_string()));
    }

    #[test]
    fn test_resolve_url_fragment_returns_none() {
        let url = VSAForager::resolve_url("http://example.com/page", "#section");
        assert_eq!(url, None);
    }

    #[test]
    fn test_resolve_url_javascript_returns_none() {
        let url = VSAForager::resolve_url("http://example.com/page", "javascript:void(0)");
        assert_eq!(url, None);
    }

    #[test]
    fn test_resolve_url_empty_returns_none() {
        let url = VSAForager::resolve_url("http://example.com/page", "");
        assert_eq!(url, None);
    }
}
