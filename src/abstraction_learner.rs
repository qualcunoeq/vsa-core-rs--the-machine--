// ─── Abstraction Learner — Self-Extending Keyword Maps ─────────────────────
//
// After solved episodes, the learner records which tokens in the error text
// were NOT matched by the existing keyword maps (ACTIONS, RESOURCES,
// ERROR_CLASSES in diagnostic.rs).  When a token appears consistently in
// episodes of the same category, the learner proposes a new keyword mapping,
// validates it against known negatives, and promotes it.
//
// This lets the system grow its abstraction vocabulary from verified repair
// trajectories — not from word overlap alone.
//
// ## Learning signal
//
//   For each solved episode (error_text, category, parser_output):
//     1. Tokenize the error text
//     2. For each token NOT in any keyword map:
//        - Record co-occurrence: (token, category, count++)
//     3. When a token reaches the promotion threshold:
//        - Infer its role (action/resource/error) from the parser context
//        - Validate: does it cause false positives on known negative cases?
//        - If validated: add to promoted keyword maps
//
// ## Why this works
//
//   The solved category encodes the abstract semantics.  A token that
//   consistently appears in "connection_refused" episodes is likely a
//   network_service resource keyword.  The fix trajectory (which produced
//   the category) provides the supervision signal.
// ────────────────────────────────────────────────────────────────────────────

use std::collections::HashMap;

/// The role a learned keyword fills in the error structure.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum MappingRole {
    Action,
    Resource,
    Error,
}

/// A learned keyword mapping that has passed the validation gate.
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct LearnedMapping {
    /// The raw token (e.g., "broker", "handshake", "expired")
    pub keyword: String,
    /// The concrete form (e.g., "remote_server", "timed_out")
    pub concrete: String,
    /// The abstract form (e.g., "network_service", "unavailable")
    pub abstract_: String,
    /// Which slot this mapping fills
    pub role: MappingRole,
    /// Confidence score (purity × frequency factor)
    pub confidence: f64,
    /// The category that produced this mapping
    pub source_category: String,
    /// Total episodes recorded at the time this mapping was promoted (version metadata).
    #[serde(default)]
    pub promoted_at_episode: u32,
    /// Forward-compatible metadata map for versioned attribute extensions.
    #[serde(default)]
    pub metadata: std::collections::HashMap<String, String>,
}

/// Tracks unknown token occurrences across solved episodes and promotes
/// high-confidence token→role mappings into the keyword extension tables.
///
/// Usage:
///   let mut learner = AbstractionLearner::new();
///   // After each solved episode:
///   learner.record_episode(error_text, category);
///   // Use promoted mappings in structural parsing:
///   let structure = parse_error_structure_with_learner(text, &learner);
///
/// Persistence: call `save_to_file` / `load_from_file` to preserve
/// promoted mappings and co-occurrence statistics across runs.
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct AbstractionLearner {
    /// Schema version for forward/backward compatibility.
    #[serde(default = "default_version")]
    version: u64,
    /// token → Vec<(category, count)>
    co_occurrence: HashMap<String, Vec<(String, u32)>>,
    /// Promoted mappings that passed the validation gate
    promoted: Vec<LearnedMapping>,
    /// Total episodes recorded
    total_episodes: u32,
    /// Minimum episodes before a token can be promoted
    min_episodes: u32,
    /// Minimum purity (dominant category fraction) for promotion
    min_purity: f64,
}

fn default_version() -> u64 {
    1
}

impl AbstractionLearner {
    /// Create a new learner with default thresholds.
    ///
    /// Defaults:
    ///   - min_episodes: 3  (token must appear in ≥3 solved episodes)
    ///   - min_purity:   0.80  (≥80% of appearances must be same category)
    pub fn new() -> Self {
        AbstractionLearner {
            version: 1,
            co_occurrence: HashMap::new(),
            promoted: Vec::new(),
            total_episodes: 0,
            min_episodes: 3,
            min_purity: 0.80,
        }
    }

    /// Create a learner with custom thresholds.
    pub fn with_thresholds(min_episodes: u32, min_purity: f64) -> Self {
        AbstractionLearner {
            version: 1,
            co_occurrence: HashMap::new(),
            promoted: Vec::new(),
            total_episodes: 0,
            min_episodes,
            min_purity,
        }
    }

    /// Record a solved episode: tokenize the error text, update
    /// co-occurrence counts for unknown tokens, and attempt promotions.
    ///
    /// Call this after `absorb_diagnosis` for every solved episode.
    pub fn record_episode(&mut self, error_text: &str, category: &str) {
        self.total_episodes += 1;

        // Tokenize — split on whitespace and common punctuation
        let tokens = Self::tokenize(error_text);

        for token in &tokens {
            // Skip if this token is already known to the keyword maps
            if Self::is_known_keyword(token) {
                continue;
            }
            // Skip if this token is already a promoted keyword
            if self.promoted.iter().any(|m| m.keyword == *token) {
                continue;
            }
            // Skip very short tokens (likely noise)
            if token.len() < 2 {
                continue;
            }
            // Skip purely numeric tokens
            if token.chars().all(|c| c.is_ascii_digit()) {
                continue;
            }
            // Skip stopwords — common English words that carry no diagnostic
            // signal.  Without this, tokens like "to", "on", "by", "from"
            // could accumulate co-occurrence counts and get promoted.
            if Self::is_stopword(token) {
                continue;
            }

            // Increment co-occurrence for (token, category)
            let entries = self.co_occurrence.entry(token.clone()).or_default();
            let mut found = false;
            for (cat, count) in entries.iter_mut() {
                if *cat == category {
                    *count += 1;
                    found = true;
                    break;
                }
            }
            if !found {
                entries.push((category.to_string(), 1));
            }
        }

        // Try to promote new mappings
        self.try_promote();
    }

    /// Try to promote new keyword mappings from accumulated co-occurrence data.
    ///
    /// For each token that meets the threshold, infer its role and validate.
    fn try_promote(&mut self) {
        let mut to_promote: Vec<LearnedMapping> = Vec::new();
        let mut candidate_tokens: Vec<String> = self.co_occurrence.keys().cloned().collect();
        candidate_tokens.sort();

        // Collect candidates in lexical order so promotion is reproducible
        // even though co_occurrence is backed by a HashMap.
        for token in candidate_tokens {
            let category_counts = match self.co_occurrence.get(&token) {
                Some(counts) => counts,
                None => continue,
            };
            let total: u32 = category_counts.iter().map(|(_, c)| c).sum();
            if total < self.min_episodes {
                continue;
            }

            // Find dominant category with deterministic tie-breaking.
            let dominant = match Self::dominant_category(category_counts) {
                Some(dominant) => dominant,
                None => continue,
            };
            let purity = dominant.1 as f64 / total as f64;
            if purity < self.min_purity {
                continue;
            }

            // Determine role based on what the parser would be missing.
            // We use the token length as a rough heuristic:
            //   - Longer tokens (>6 chars) are more likely resources
            //   - Error-class tokens often end in "ed", "ing", "y"
            //   - Action tokens are typically verbs
            // This is a weak signal, so we combine it with category context.
            let role = Self::infer_role(&token, &dominant.0);

            // Check if this mapping already exists
            if self.promoted.iter().any(|m| m.keyword == token) {
                continue;
            }

            // Validate: check against known negative examples
            if !self.validate_mapping(&token, &dominant.0, &role) {
                continue;
            }

            // Determine concrete and abstract values
            let concrete = Self::concrete_for_token(&token);
            let abstract_val = match Self::abstract_for_category(&dominant.0, &role) {
                Some(a) => a.to_string(),
                None => continue, // Can't determine abstract → skip
            };

            let confidence = purity * (1.0 - 1.0 / (total as f64 + 1.0));

            // Avoid duplicates from multiple promotion cycles
            if !to_promote.iter().any(|m| m.keyword == token) {
                let mut mapping_metadata = HashMap::new();
                mapping_metadata.insert("version".to_string(), self.version.to_string());
                to_promote.push(LearnedMapping {
                    keyword: token,
                    concrete,
                    abstract_: abstract_val,
                    role: role.clone(),
                    confidence,
                    source_category: dominant.0.clone(),
                    promoted_at_episode: self.total_episodes,
                    metadata: mapping_metadata,
                });
            }
        }

        to_promote.sort_by(Self::compare_learned_mappings);

        // Add all validated promotions
        for mapping in to_promote {
            eprintln!(
                "  📘 Learned: '{}' → {} ({}) [{}] conf={:.3}",
                mapping.keyword,
                mapping.abstract_,
                match mapping.role {
                    MappingRole::Action => "action",
                    MappingRole::Resource => "resource",
                    MappingRole::Error => "error",
                },
                mapping.source_category,
                mapping.confidence,
            );
            self.promoted.push(mapping);
        }
        self.promoted.sort_by(Self::compare_learned_mappings);
    }

    fn dominant_category(category_counts: &[(String, u32)]) -> Option<(String, u32)> {
        let mut ranked = category_counts.to_vec();
        ranked.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
        ranked.into_iter().next()
    }

    fn compare_learned_mappings(a: &LearnedMapping, b: &LearnedMapping) -> std::cmp::Ordering {
        b.confidence
            .total_cmp(&a.confidence)
            .then_with(|| a.keyword.cmp(&b.keyword))
            .then_with(|| Self::role_priority(&a.role).cmp(&Self::role_priority(&b.role)))
            .then_with(|| a.source_category.cmp(&b.source_category))
            .then_with(|| a.abstract_.cmp(&b.abstract_))
            .then_with(|| a.concrete.cmp(&b.concrete))
    }

    fn role_priority(role: &MappingRole) -> u8 {
        match role {
            MappingRole::Action => 0,
            MappingRole::Resource => 1,
            MappingRole::Error => 2,
        }
    }

    /// Validate a proposed mapping by checking it doesn't cause false positives.
    ///
    /// Checks:
    ///   1. The token's dominant category must account for ≥80% of appearances
    ///   2. The token must NOT appear more than 20% of the time in OTHER categories
    fn validate_mapping(&self, token: &str, proposed_category: &str, _role: &MappingRole) -> bool {
        let entries = match self.co_occurrence.get(token) {
            Some(e) => e,
            None => return false,
        };

        let mut total_other: u32 = 0;
        let mut total_all: u32 = 0;
        for (cat, count) in entries {
            total_all += count;
            if *cat != proposed_category {
                total_other += count;
            }
        }

        if total_all == 0 {
            return false;
        }

        // The token must not appear in other categories more than 20% of the time
        let other_frac = total_other as f64 / total_all as f64;
        if other_frac > 0.20 {
            return false;
        }

        true
    }

    /// Infer what role a token likely fills based on its characteristics
    /// and the category it's associated with.
    fn infer_role(token: &str, category: &str) -> MappingRole {
        // Category-based role hints
        match category {
            "port_conflict" | "connection_refused" | "network_timeout" => {
                // These categories typically involve network resources.
                // Longer tokens that aren't error-like are likely resources.
                if token.len() > 5 && !Self::looks_like_error(token) {
                    return MappingRole::Resource;
                }
                MappingRole::Error
            }
            "disk_full" | "missing_file" => {
                // These categories involve storage or filesystem resources
                if token.len() > 4 && !Self::looks_like_error(token) {
                    return MappingRole::Resource;
                }
                MappingRole::Error
            }
            "permission_denied" => {
                MappingRole::Resource // typically a resource that was denied
            }
            "credential_invalid" => {
                MappingRole::Resource // typically a credential token
            }
            "startup_failure" => {
                // Could be any role — default to resource
                MappingRole::Resource
            }
            _ => MappingRole::Resource, // safest default
        }
    }

    /// Heuristic: does a token look like an error class keyword?
    fn looks_like_error(token: &str) -> bool {
        let lower = token.to_lowercase();
        // Common error suffixes
        if lower.ends_with("failed")
            || lower.ends_with("error")
            || lower.ends_with("denied")
            || lower.ends_with("refused")
            || lower.ends_with("expired")
            || lower.ends_with("invalid")
            || lower.ends_with("missing")
            || lower.ends_with("stalled")
            || lower.ends_with("exceeded")
            || lower.ends_with("unreachable")
            || lower.ends_with("rupted")
            || lower.ends_with("roken")
        {
            return true;
        }
        // Common error words — includes connection/negotiation failures
        matches!(
            lower.as_str(),
            "timeout"
                | "error"
                | "fail"
                | "fault"
                | "crash"
                | "panic"
                | "abort"
                | "dead"
                | "stuck"
                | "hung"
                | "handshake"
                | "retry"
                | "backoff"
                | "throttle"
                | "reset"
                | "disconnect"
                | "refused"
                | "reject"
        )
    }

    /// Get a concrete form for a token.
    ///
    /// Converts the token to a normalized form: lowercase, replace spaces
    /// with underscores, prefix with "kw_" to avoid collisions.
    fn concrete_for_token(token: &str) -> String {
        let normalized: String = token
            .chars()
            .map(|c| {
                if c.is_alphanumeric() {
                    c.to_ascii_lowercase()
                } else {
                    '_'
                }
            })
            .collect();
        format!("kw_{}", normalized)
    }

    /// Determine the abstract value for a (category, role) pair.
    ///
    /// The category encodes the abstract semantics because it represents
    /// the verified fix trajectory.  "connection_refused" means the fix
    /// involved checking a network service → abstract = "network_service".
    fn abstract_for_category(category: &str, role: &MappingRole) -> Option<&'static str> {
        match role {
            MappingRole::Action => Some("process"),
            MappingRole::Resource => match category {
                "port_conflict" | "connection_refused" | "network_timeout" => {
                    Some("network_service")
                }
                "missing_file" => Some("file_system"),
                "disk_full" => Some("storage"),
                "credential_invalid" => Some("credential"),
                "permission_denied" => Some("file_system"),
                "startup_failure" => Some("service"),
                _ => None,
            },
            MappingRole::Error => match category {
                "port_conflict" | "connection_refused" | "network_timeout" => Some("unavailable"),
                "missing_file" => Some("resource_missing"),
                "disk_full" => Some("capacity_exhausted"),
                "permission_denied" => Some("permission_blocked"),
                "credential_invalid" => Some("credential_invalid"),
                "startup_failure" => Some("unavailable"),
                _ => None,
            },
        }
    }

    // ─── Accessors for integration with parse_error_structure ───────────

    /// Get the currently promoted action keyword mappings.
    /// Returns (keyword, concrete, abstract) triples.
    pub fn promoted_actions(&self) -> Vec<(&str, &str, &str)> {
        self.promoted
            .iter()
            .filter(|m| m.role == MappingRole::Action)
            .map(|m| {
                (
                    m.keyword.as_str(),
                    m.concrete.as_str(),
                    m.abstract_.as_str(),
                )
            })
            .collect()
    }

    /// Get the currently promoted resource keyword mappings.
    pub fn promoted_resources(&self) -> Vec<(&str, &str, &str)> {
        self.promoted
            .iter()
            .filter(|m| m.role == MappingRole::Resource)
            .map(|m| {
                (
                    m.keyword.as_str(),
                    m.concrete.as_str(),
                    m.abstract_.as_str(),
                )
            })
            .collect()
    }

    /// Get the currently promoted error keyword mappings.
    pub fn promoted_errors(&self) -> Vec<(&str, &str, &str)> {
        self.promoted
            .iter()
            .filter(|m| m.role == MappingRole::Error)
            .map(|m| {
                (
                    m.keyword.as_str(),
                    m.concrete.as_str(),
                    m.abstract_.as_str(),
                )
            })
            .collect()
    }

    /// Get the total number of promoted mappings.
    pub fn promoted_count(&self) -> usize {
        self.promoted.len()
    }

    /// Get promoted mappings in deterministic ranking order.
    pub fn promoted_mappings(&self) -> Vec<&LearnedMapping> {
        self.promoted.iter().collect()
    }

    /// Get the total number of episodes recorded.
    pub fn episode_count(&self) -> u32 {
        self.total_episodes
    }

    /// Get the number of unique tracked tokens.
    pub fn tracked_token_count(&self) -> usize {
        self.co_occurrence.len()
    }

    /// Generate a report of learner state (for logging).
    pub fn report(&self) -> String {
        let mut lines = Vec::new();
        lines.push(format!("AbstractionLearner report:"));
        lines.push(format!("  Episodes recorded: {}", self.total_episodes));
        lines.push(format!("  Tracked tokens:    {}", self.co_occurrence.len()));
        lines.push(format!("  Promoted mappings: {}", self.promoted.len()));

        // Show top tokens by frequency
        let mut token_entries: Vec<(&String, &Vec<(String, u32)>)> =
            self.co_occurrence.iter().collect();
        token_entries.sort_by(|a, b| {
            let total_a: u32 = a.1.iter().map(|(_, c)| c).sum();
            let total_b: u32 = b.1.iter().map(|(_, c)| c).sum();
            total_b.cmp(&total_a).then_with(|| a.0.cmp(b.0))
        });

        for (token, counts) in token_entries.iter().take(10) {
            let total: u32 = counts.iter().map(|(_, c)| c).sum();
            let mut sorted_counts = counts.to_vec();
            sorted_counts.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
            let cat_str: Vec<String> = sorted_counts
                .iter()
                .map(|(c, n)| format!("{}={}", c, n))
                .collect();
            lines.push(format!("    {} ({}): {}", token, total, cat_str.join(", ")));
        }

        lines.join("\n")
    }

    // ─── Tokenization ──────────────────────────────────────────────────

    /// Tokenize error text into individual word tokens.
    fn tokenize(text: &str) -> Vec<String> {
        let mut tokens = Vec::new();
        let lower = text.to_lowercase();
        let mut current = String::new();

        for ch in lower.chars() {
            if ch.is_alphanumeric() || ch == '_' || ch == '-' {
                current.push(ch);
            } else {
                if !current.is_empty() {
                    tokens.push(current.clone());
                    current.clear();
                }
            }
        }
        if !current.is_empty() {
            tokens.push(current);
        }

        tokens
    }

    // ─── Stopword filtering ────────────────────────────────────────────

    /// Check if a token is a common English stopword that carries no
    /// diagnostic signal.  Prevents noise tokens from being promoted.
    fn is_stopword(token: &str) -> bool {
        let lower = token.to_lowercase();
        STOPWORDS.binary_search(&lower.as_str()).is_ok()
    }

    // ─── Keyword map checking ──────────────────────────────────────────

    /// Check if a token is already a known keyword in the built-in maps.
    ///
    /// This must match the keywords in `diagnostic.rs`:
    ///   - ACTIONS: keyword triggers via `lower.contains(keyword)`
    ///   - RESOURCES: keyword triggers via `contains_word()`
    ///   - ERROR_CLASSES: keyword triggers via `lower.contains(keyword)`
    ///
    /// We maintain a static set of all known keywords for fast lookup.
    fn is_known_keyword(token: &str) -> bool {
        let lower = token.to_lowercase();
        KNOWN_KEYWORDS.binary_search(&lower.as_str()).is_ok()
    }

    /// Get the schema version of this learner's persisted state.
    pub fn version(&self) -> u64 {
        self.version
    }

    /// Get metadata for all promoted mappings (for audit queries).
    pub fn promoted_with_metadata(&self) -> Vec<(&LearnedMapping, u32, &HashMap<String, String>)> {
        self.promoted
            .iter()
            .map(|m| (m, m.promoted_at_episode, &m.metadata))
            .collect()
    }

    // ─── Persistence ──────────────────────────────────────────────────

    /// Save the learner state (promotions + statistics) to a JSON file.
    pub fn save_to_file(&self, path: &str) -> Result<(), String> {
        let json = serde_json::to_string_pretty(self)
            .map_err(|e| format!("AbstractionLearner serialization error: {}", e))?;
        std::fs::write(path, &json)
            .map_err(|e| format!("AbstractionLearner write error: {}", e))?;
        Ok(())
    }

    /// Load the learner state from a JSON file.
    pub fn load_from_file(path: &str) -> Result<Self, String> {
        let json = std::fs::read_to_string(path)
            .map_err(|e| format!("AbstractionLearner read error: {}", e))?;
        serde_json::from_str(&json)
            .map_err(|e| format!("AbstractionLearner deserialization error: {}", e))
    }
}

/// Static set of all keywords from the built-in maps.
/// Sorted for binary search.  Must be kept in sync with diagnostic.rs.
const KNOWN_KEYWORDS: &[&str] = &[
    "abort",
    "address",
    "bind",
    "bucket",
    "cache",
    "certificate",
    "compact",
    "connect",
    "corrupt",
    "crash",
    "database",
    "dead",
    "denied",
    "directory",
    "disk",
    "eacces",
    "enoent",
    "endpoint",
    "exceeded",
    "expired",
    "fail",
    "failed",
    "fault",
    "file",
    "filesystem",
    "flush",
    "full",
    "gateway",
    "host",
    "hung",
    "index",
    "initializ",
    "invalid",
    "key",
    "listen",
    "missing",
    "mount",
    "not found",
    "open",
    "panic",
    "parse",
    "permission",
    "port",
    "query",
    "read",
    "reach",
    "rebuild",
    "refused",
    "server",
    "socket",
    "stalled",
    "storage",
    "store",
    "timeout",
    "token",
    "unreachable",
    "url",
    "validat",
    "validate",
    "volume",
    "write",
];

/// Common English stopwords that carry no diagnostic signal.
/// Sorted for binary search.  Tokens matching this list are skipped
/// by `record_episode` to prevent noise promotion.
const STOPWORDS: &[&str] = &[
    "a", "about", "after", "all", "also", "an", "and", "any", "are", "as", "at", "back", "be",
    "because", "been", "but", "by", "can", "could", "did", "do", "does", "done", "down", "each",
    "either", "for", "from", "get", "got", "had", "has", "have", "here", "how", "if", "in", "into",
    "is", "it", "its", "just", "like", "made", "make", "may", "maybe", "more", "most", "much",
    "must", "my", "no", "nor", "not", "now", "of", "off", "on", "once", "only", "or", "other",
    "our", "out", "over", "said", "same", "see", "she", "should", "show", "side", "since", "so",
    "some", "still", "such", "take", "than", "that", "the", "their", "them", "then", "there",
    "these", "they", "this", "through", "to", "too", "under", "up", "upon", "very", "was", "way",
    "we", "well", "were", "what", "when", "where", "which", "while", "who", "will", "with",
    "within", "without", "would", "yes", "yet", "you", "your",
];

// ─── Tests ─────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tokenize_basic() {
        let tokens = AbstractionLearner::tokenize("bind() to 0.0.0.0:80 failed");
        assert!(tokens.contains(&"bind".to_string()));
        assert!(tokens.contains(&"to".to_string()));
        assert!(tokens.contains(&"failed".to_string()));
        // Numeric tokens like "0", "80" pass through tokenize but are
        // filtered by record_episode (purely numeric tokens are skipped).
        assert!(tokens.contains(&"0".to_string()));
        assert!(tokens.contains(&"80".to_string()));
    }

    #[test]
    fn test_tokenize_with_punctuation() {
        let tokens = AbstractionLearner::tokenize("SSL certificate validation failed!");
        assert!(tokens.contains(&"ssl".to_string()));
        assert!(tokens.contains(&"certificate".to_string()));
        assert!(tokens.contains(&"validation".to_string()));
        assert!(tokens.contains(&"failed".to_string()));
    }

    #[test]
    fn test_known_keyword_check() {
        assert!(AbstractionLearner::is_known_keyword("port"));
        assert!(AbstractionLearner::is_known_keyword("failed"));
        assert!(AbstractionLearner::is_known_keyword("timeout"));
        assert!(AbstractionLearner::is_known_keyword("bind"));
        assert!(!AbstractionLearner::is_known_keyword("broker"));
        assert!(!AbstractionLearner::is_known_keyword("keyserver"));
        assert!(!AbstractionLearner::is_known_keyword("amqp"));
    }

    #[test]
    fn test_record_episode_tracks_unknown_tokens() {
        let mut learner = AbstractionLearner::with_thresholds(5, 0.80);

        // "broker" and "endpoint" are unknown. "timeout" is known.
        learner.record_episode("AMQP broker timeout", "connection_refused");
        assert_eq!(learner.tracked_token_count(), 2); // "amqp", "broker"
        assert!(learner.co_occurrence.contains_key("amqp"));
        assert!(learner.co_occurrence.contains_key("broker"));
        assert!(!learner.co_occurrence.contains_key("timeout")); // known
    }

    #[test]
    fn test_multiple_episodes_same_category_promotes() {
        let mut learner = AbstractionLearner::with_thresholds(3, 0.80);

        // "broker" appears in 3 connection_refused episodes
        learner.record_episode("AMQP broker timeout", "connection_refused");
        learner.record_episode("message broker unreachable", "connection_refused");
        learner.record_episode("broker connection lost", "connection_refused");

        assert!(
            learner.promoted_count() >= 1,
            "Should have promoted at least one mapping"
        );
        assert!(
            learner.promoted.iter().any(|m| m.keyword == "broker"),
            "broker should be promoted"
        );
    }

    #[test]
    fn test_dominant_category_tie_breaks_lexically() {
        let dominant = AbstractionLearner::dominant_category(&[
            ("zeta_category".to_string(), 2),
            ("alpha_category".to_string(), 2),
        ]);

        assert_eq!(dominant, Some(("alpha_category".to_string(), 2)));
    }

    #[test]
    fn test_promotions_have_deterministic_order() {
        let mut first = AbstractionLearner::with_thresholds(2, 0.80);
        first.record_episode("broker cluster outage", "connection_refused");
        first.record_episode("cluster broker outage", "connection_refused");

        let mut second = AbstractionLearner::with_thresholds(2, 0.80);
        second.record_episode("cluster broker outage", "connection_refused");
        second.record_episode("broker cluster outage", "connection_refused");

        let first_keywords: Vec<&str> = first
            .promoted_mappings()
            .iter()
            .map(|m| m.keyword.as_str())
            .collect();
        let second_keywords: Vec<&str> = second
            .promoted_mappings()
            .iter()
            .map(|m| m.keyword.as_str())
            .collect();

        assert_eq!(first_keywords, second_keywords);
        assert!(
            first_keywords.windows(2).all(|pair| pair[0] <= pair[1]),
            "Equal-confidence promotions should use lexical tie-breaking"
        );
    }

    #[test]
    fn test_token_in_multiple_categories_not_promoted() {
        let mut learner = AbstractionLearner::with_thresholds(3, 0.80);

        // "volume" appears in disk_full twice and port_conflict twice
        // (not pure enough → no promotion)
        learner.record_episode("storage volume full", "disk_full");
        learner.record_episode("volume quota exceeded", "disk_full");
        learner.record_episode("bind to volume port", "port_conflict");
        learner.record_episode("volume address in use", "port_conflict");

        // The threshold is 3 episodes, but purity is 0.50 < 0.80
        // Actually, 4 episodes total with min_episodes=3 means it qualifies,
        // but the purity is 0.50 which is below 0.80, so no promotion.
        let promoted_for_volume: Vec<&LearnedMapping> = learner
            .promoted
            .iter()
            .filter(|m| m.keyword == "volume")
            .collect();
        assert!(
            promoted_for_volume.is_empty(),
            "volume should NOT be promoted (low purity)"
        );
    }

    #[test]
    fn test_abstract_for_category() {
        // Resource mappings
        assert_eq!(
            AbstractionLearner::abstract_for_category("connection_refused", &MappingRole::Resource),
            Some("network_service")
        );
        assert_eq!(
            AbstractionLearner::abstract_for_category("disk_full", &MappingRole::Resource),
            Some("storage")
        );
        assert_eq!(
            AbstractionLearner::abstract_for_category("credential_invalid", &MappingRole::Resource),
            Some("credential")
        );
        assert_eq!(
            AbstractionLearner::abstract_for_category("missing_file", &MappingRole::Resource),
            Some("file_system")
        );

        // Action mappings all return "process"
        assert_eq!(
            AbstractionLearner::abstract_for_category("connection_refused", &MappingRole::Action),
            Some("process")
        );

        // Error mappings
        assert_eq!(
            AbstractionLearner::abstract_for_category("disk_full", &MappingRole::Error),
            Some("capacity_exhausted")
        );
    }

    #[test]
    fn test_infer_role_by_category() {
        // Network categories → resource if token is long and not error-like
        assert_eq!(
            AbstractionLearner::infer_role("broker", "connection_refused"),
            MappingRole::Resource
        );
        // Error-like tokens → error role
        assert_eq!(
            AbstractionLearner::infer_role("expired", "connection_refused"),
            MappingRole::Error
        );
        // Storage categories
        assert_eq!(
            AbstractionLearner::infer_role("quota", "disk_full"),
            MappingRole::Resource
        );
        // Short tokens could go either way
        let role = AbstractionLearner::infer_role("no", "connection_refused");
        assert!(role == MappingRole::Resource || role == MappingRole::Error);
    }

    #[test]
    fn test_report_format() {
        let mut learner = AbstractionLearner::new();
        learner.record_episode("unknown_token_error", "port_conflict");
        let report = learner.report();
        assert!(report.contains("Episodes recorded:"));
        assert!(report.contains("unknown_token_error"));
    }

    #[test]
    fn test_report_orders_equal_frequency_tokens_lexically() {
        let mut learner = AbstractionLearner::with_thresholds(10, 0.80);
        learner.record_episode("zeta_token alpha_token", "port_conflict");

        let report = learner.report();
        let alpha_pos = report.find("alpha_token").unwrap();
        let zeta_pos = report.find("zeta_token").unwrap();

        assert!(alpha_pos < zeta_pos);
    }

    #[test]
    fn test_concrete_for_token_normalization() {
        let c = AbstractionLearner::concrete_for_token("SSL-Cert");
        assert_eq!(c, "kw_ssl_cert");
    }

    #[test]
    fn test_save_and_load_roundtrip() {
        let tmp = std::env::temp_dir().join("test_abstraction_learner.json");
        let path = tmp.to_str().unwrap().to_string();

        let mut learner = AbstractionLearner::new();
        learner.record_episode("broker connection refused", "connection_refused");
        learner.record_episode("broker handshake expired", "connection_refused");
        learner.record_episode("broker ssl error", "connection_refused");
        let promoted_before = learner.promoted_count();

        learner.save_to_file(&path).unwrap();

        let loaded = AbstractionLearner::load_from_file(&path).unwrap();
        assert_eq!(loaded.promoted_count(), promoted_before);
        assert_eq!(loaded.episode_count(), 3);

        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn test_promotion_version_metadata() {
        let mut learner = AbstractionLearner::new();
        assert_eq!(learner.version(), 1, "initial version should be 1");

        // Record enough episodes to trigger promotion
        for _ in 0..5 {
            learner.record_episode("broker connection refused", "connection_refused");
        }

        // Check promoted mappings have version metadata
        let meta = learner.promoted_with_metadata();
        for (mapping, episode, md) in &meta {
            assert!(
                mapping.promoted_at_episode > 0,
                "promoted_at_episode should be set"
            );
            assert_eq!(
                md.get("version").unwrap(),
                "1",
                "metadata should contain version=1"
            );
            assert!(
                *episode <= learner.episode_count(),
                "episode should be <= total"
            );
        }

        // Verify round-trip preserves metadata
        let tmp = std::env::temp_dir().join("test_learner_version.json");
        let path = tmp.to_str().unwrap().to_string();
        learner.save_to_file(&path).unwrap();
        let loaded = AbstractionLearner::load_from_file(&path).unwrap();
        assert_eq!(loaded.version(), 1, "version should survive round-trip");
        let loaded_meta = loaded.promoted_with_metadata();
        assert_eq!(loaded_meta.len(), meta.len(), "same number of promotions");
        for (lm, ep, md) in &loaded_meta {
            assert_eq!(*ep, lm.promoted_at_episode, "episode should match");
            assert_eq!(md.get("version").unwrap(), "1", "version should match");
        }
        let _ = std::fs::remove_file(&path);
    }
}
