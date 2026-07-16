// ─── MetaCognition — Know What It Knows ──────────────────────────────────
//
// Three facets of metacognitive awareness:
//
//   1. KNOWLEDGE AWARENESS — what concepts the system knows well, what it
//      barely knows, and what it doesn't know at all.
//
//   2. CONTRADICTION DETECTION — surfaces conflicting facts so the system
//      can flag uncertainty rather than giving confident wrong answers.
//
//   3. EPISTEMIC QUESTIONING — when faced with a weak or unknown concept,
//      formulate a concrete question the system could investigate.
//
// All three run on pure rule-based analysis of the QA engine. No ML, no LLMs.
//
// ────────────────────────────────────────────────────────────────────────────

use crate::qa::QaEngine;
use std::collections::{HashMap, HashSet, VecDeque};

/// Numeric rank for a knowledge level (higher = more known).
pub fn level_rank(level: &KnowledgeLevel) -> usize {
    match level {
        KnowledgeLevel::Unknown => 0,
        KnowledgeLevel::Weak => 1,
        KnowledgeLevel::Adequate => 2,
        KnowledgeLevel::Strong => 3,
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// CONSTANTS
// ═══════════════════════════════════════════════════════════════════════════

/// A concept is "well-defined" if it has at least this many definitions.
const MIN_DEFINITIONS: usize = 1;
/// A concept is "well-defined" if it has at least this many properties.
const MIN_PROPERTIES: usize = 1;
/// A concept with fewer than this many total facts is "weakly known".
const WEAK_KNOWLEDGE_THRESHOLD: usize = 3;
/// A concept with zero facts is "unknown" (never seen).
const UNKNOWN_KNOWLEDGE_THRESHOLD: usize = 0;
/// Maximum questions to retain in the queue.
const MAX_QUESTIONS: usize = 50;
/// Maximum question history to retain.
const MAX_QUESTION_HISTORY: usize = 200;
/// Cooldown ticks before the same topic can be searched again.
const EPISTEMIC_SEARCH_COOLDOWN_TICKS: u64 = 100;

/// Questions older than this many ticks are expired and purged from the queue.
/// Prevents the "quantum_stuff tick=0" problem — a question from 10 000 ticks
/// ago is almost certainly stale, even if the concept is still Unknown.
const QUESTIONS_TTL: u64 = 2000;

/// How many most recent `question_trend` samples to retain for trend analysis.
const TREND_WINDOW: usize = 20;

/// Minimum number of new facts required to trigger a full re-assessment.
/// If fewer than this many facts were added since the last full scan,
/// `assess()` skips the expensive `concept_summary` per concept and
/// only updates uncertainty metrics from existing state.
const MIN_NEW_FACTS_FOR_ASSESSMENT: usize = 10;

/// Even if few facts changed, force a full assessment every N calls to
/// prevent stale knowledge states from persisting indefinitely.
const FORCE_FULL_ASSESSMENT_EVERY: usize = 5;

// ═══════════════════════════════════════════════════════════════════════════
// TYPES
// ═══════════════════════════════════════════════════════════════════════════

/// How well the system knows a given concept.
#[derive(Clone, Debug, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum KnowledgeLevel {
    /// No facts found for this concept at all.
    Unknown,
    /// Only 1–2 facts — barely knows it.
    Weak,
    /// Has definitions and properties but could know more.
    Adequate,
    /// Multiple definitions, properties, rules — well understood.
    Strong,
}

/// The reason the system is uncertain and generated a question.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum QuestionReason {
    /// This concept has never been encountered.
    UnknownConcept,
    /// The concept is mentioned but has no definition (verb="be").
    MissingDefinition,
    /// Contradictory evidence exists for this concept.
    ContradictoryEvidence,
    /// The answer confidence was below threshold.
    LowConfidence,
    /// There's a detectable gap between related concepts.
    GapInKnowledge,
    /// Multiple possible interpretations exist.
    Ambiguous,
}

/// A question the system would like to answer.
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct EpistemicQuestion {
    /// The natural-language question.
    pub question: String,
    /// The topic/concept this question is about.
    pub topic: String,
    /// How uncertain we are (0 = certain, 1 = completely uncertain).
    pub uncertainty: f64,
    /// Why this question was generated.
    pub reason: QuestionReason,
    /// Tick when this question was generated.
    pub tick: u64,
    /// Whether this question has been addressed.
    pub answered: bool,
}

/// A detected contradiction between two facts.
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct DetectedContradiction {
    /// The subject involved in the contradiction.
    pub subject: String,
    /// First fact's verb.
    pub verb_a: String,
    /// First fact's object.
    pub object_a: String,
    /// Second fact's verb (the opposite).
    pub verb_b: String,
    /// Second fact's object.
    pub object_b: String,
    /// Tick when first detected.
    pub tick: u64,
    /// Whether this contradiction has been acknowledged/resolved.
    pub resolved: bool,
}

/// Knowledge state for a single concept.
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct ConceptKnowledge {
    /// How many definition facts exist (verb="be", concept as object).
    pub definitions: usize,
    /// How many "is_defined_as" facts exist (concept as subject of "be").
    pub is_defined_as: usize,
    /// How many property facts exist.
    pub properties: usize,
    /// How many operations involve this concept.
    pub operations: usize,
    /// Total facts involving this concept.
    pub total_facts: usize,
    /// How many causal rules involve this concept.
    pub rules_involving: usize,
    /// Whether the concept has at least one definition AND one property.
    pub is_well_defined: bool,
    /// The knowledge level.
    pub level: KnowledgeLevel,
    /// Tick of last assessment.
    pub last_assessed: u64,
    /// True if this concept's level was boosted by the `is_a` hierarchy
    /// (e.g., "calculus" promoted because "derivative" is_a "calculus_concept"
    /// and derivative is well-known).
    pub hierarchy_boosted: bool,
}

/// Overall epistemic uncertainty snapshot.
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct EpistemicUncertainty {
    /// Composite uncertainty score [0, 1].
    pub overall: f64,
    /// Number of concepts with level=Weak or level=Unknown.
    pub weak_concept_count: usize,
    /// Number of concepts with level=Unknown.
    pub unknown_concept_count: usize,
    /// Number of unresolved contradictions.
    pub unresolved_contradictions: usize,
    /// Fraction of known concepts that are well-defined [0, 1].
    pub knowledge_coverage: f64,
    /// Total unique concepts in the KB.
    pub total_concepts: usize,
    /// Concepts with the highest uncertainty (up to 5).
    pub most_uncertain: Vec<String>,
}

/// Structural gaps in the knowledge graph.
///
/// Built from the bipartite subject↔object graph. A healthy knowledge base
/// has most concepts appearing as BOTH subjects and objects (symmetric).
/// Concepts that are only subjects or only objects are structural gaps —
/// they indicate missing cross-references or dangling definitions.
///
/// # Note
/// Examples are generated on-the-fly by `examples()` methods — the struct
/// stores the full vectors so the report can slice the first 10 entries
/// without duplicating them in separate fields.
#[derive(Clone, Debug, Default, serde::Serialize, serde::Deserialize)]
pub struct StructuralGaps {
    /// Concepts that appear only as subjects (never as an object of any fact).
    /// These can never be reached by queries starting from an object.
    pub subject_only: Vec<String>,
    /// Concepts that appear only as objects (never as a subject of any fact).
    /// These have no definitions and are "dangling" — the system references
    /// them but doesn't know what they are.
    pub object_only: Vec<String>,
    /// Concepts that appear as both subject AND object (healthy).
    pub symmetric_count: usize,
    /// Total unique concepts in the bipartite graph.
    pub total_unique: usize,
    /// How many of the subject-only concepts are undefined (have no "be" or
    /// "is" fact as subject).
    pub undefined_subjects: usize,
}

impl StructuralGaps {
    /// Up to `n` examples from `subject_only`.
    pub fn subject_only_examples(&self, n: usize) -> Vec<String> {
        self.subject_only.iter().take(n).cloned().collect()
    }

    /// Up to `n` examples from `object_only`.
    pub fn object_only_examples(&self, n: usize) -> Vec<String> {
        self.object_only.iter().take(n).cloned().collect()
    }

    /// Ratio of symmetric concepts to total unique. 1.0 = every concept
    /// appears in both roles. 0.0 = no concept appears in both roles.
    pub fn symmetry_ratio(&self) -> f64 {
        if self.total_unique == 0 {
            return 1.0;
        }
        self.symmetric_count as f64 / self.total_unique as f64
    }

    /// Short human-readable summary.
    pub fn summary(&self) -> String {
        format!(
            "structural gaps: {}+{} asymmetric vs {} symmetric (ratio={:.2}), {} dangling, {} undefined",
            self.subject_only.len(),
            self.object_only.len(),
            self.symmetric_count,
            self.symmetry_ratio(),
            self.object_only.len(),
            self.undefined_subjects,
        )
    }
}

/// Record of a question being asked and its outcome.
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct QuestionRecord {
    /// The question text.
    pub question: String,
    /// Tick when it was generated.
    pub tick: u64,
    /// Tick when it was answered (0 if still unanswered).
    pub answered_tick: u64,
    /// Whether the answer was satisfactory.
    pub satisfactorily_answered: bool,
}

// ═══════════════════════════════════════════════════════════════════════════
// METACOGNITION ENGINE
// ═══════════════════════════════════════════════════════════════════════════

/// The metacognition engine tracks what the system knows, doesn't know,
/// and is uncertain about.
///
/// Usage:
/// ```
/// let mut metacog = MetaCognition::new();
/// metacog.assess(&qa);              // scan QA engine for knowledge state
/// metacog.detect_contradictions(&qa); // find conflicting facts
/// metacog.generate_questions(&qa);   // formulate questions about gaps
/// let report = metacog.knowledge_report(&qa);
/// ```
#[derive(serde::Serialize, serde::Deserialize)]
pub struct MetaCognition {
    /// Per-concept knowledge state.
    concept_knowledge: HashMap<String, ConceptKnowledge>,
    /// Detected contradictions.
    contradictions: Vec<DetectedContradiction>,
    /// Generated but unanswered questions.
    questions: VecDeque<EpistemicQuestion>,
    /// History of questions and their outcomes.
    question_history: Vec<QuestionRecord>,
    /// Current epistemic uncertainty snapshot.
    pub uncertainty: EpistemicUncertainty,
    /// Tick counter.
    tick: u64,

    /// Cached set of known concept terms (updated on each assess()).
    known_terms: HashSet<String>,
    /// Whether the engine has been initialized with at least one assess() call.
    initialized: bool,

    // ── Incremental assessment tracking ─────────────────────────────
    /// Fact count from the last full assessment (to detect new facts).
    last_assessed_fact_count: usize,
    /// Number of times `assess()` has been called (for periodic forcing).
    assess_call_count: usize,

    // ── Epistemic search cooldown ───────────────────────────────────
    /// Topics that have been recently searched (cooldown to avoid loops).
    /// Maps topic → tick when it was last searched.
    searched_topics: HashMap<String, u64>,

    // ── Question trend tracking ────────────────────────────────────
    /// Ring buffer of pending-question-count snapshots, recorded each
    /// time `generate_questions()` runs.  Used to compute the trend
    /// direction (increasing → more confusion, decreasing → progress).
    question_trend: VecDeque<usize>,
    /// Tick at which each trend sample was taken (for average age calc).
    trend_ticks: VecDeque<u64>,
    /// Fast EWMA (α = 0.30) — reacts quickly to recent changes.
    trend_ema_short: f64,
    /// Slow EWMA (α = 0.10) — tracks the long-term baseline.
    trend_ema_long: f64,
    /// Whether the EWMA has been seeded with at least one sample.
    trend_ema_seeded: bool,
}

impl MetaCognition {
    /// Create a new blank metacognition engine.
    pub fn new() -> Self {
        MetaCognition {
            concept_knowledge: HashMap::new(),
            contradictions: Vec::new(),
            questions: VecDeque::new(),
            question_history: Vec::new(),
            uncertainty: EpistemicUncertainty {
                overall: 0.0,
                weak_concept_count: 0,
                unknown_concept_count: 0,
                unresolved_contradictions: 0,
                knowledge_coverage: 0.0,
                total_concepts: 0,
                most_uncertain: Vec::new(),
            },
            tick: 0,
            known_terms: HashSet::new(),
            initialized: false,
            last_assessed_fact_count: 0,
            assess_call_count: 0,
            searched_topics: HashMap::new(),
            question_trend: VecDeque::with_capacity(TREND_WINDOW),
            trend_ticks: VecDeque::with_capacity(TREND_WINDOW),
            trend_ema_short: 0.0,
            trend_ema_long: 0.0,
            trend_ema_seeded: false,
        }
    }

    /// Increment the internal tick counter.
    pub fn tick(&mut self) {
        self.tick = self.tick.saturating_add(1);
    }

    // ── Knowledge Assessment ──────────────────────────────────────────

    /// Scan the QA engine and assess knowledge state for every concept.
    ///
    /// **Incremental**: if fewer than `MIN_NEW_FACTS_FOR_ASSESSMENT` facts
    /// have been added since the last full scan, the expensive
    /// `concept_summary` per concept is **skipped** and only the uncertainty
    /// metrics are updated from existing state.  A full scan is forced every
    /// `FORCE_FULL_ASSESSMENT_EVERY` calls regardless.
    ///
    /// Call `force_assess()` to always do the full scan (e.g., on METACOG
    /// admin command).
    ///
    /// Takes `&mut QaEngine` because `concept_summary` requires mutable access
    /// (it lazily builds the term vector cache).
    pub fn assess(&mut self, qa: &mut QaEngine) {
        self.tick();
        self.assess_call_count = self.assess_call_count.saturating_add(1);

        let total_facts = qa.fact_count();
        let new_facts = total_facts.saturating_sub(self.last_assessed_fact_count);
        let force_full = self.assess_call_count % FORCE_FULL_ASSESSMENT_EVERY == 0;

        // Incremental path: skip full concept_summary scan if few new facts.
        if !self.initialized || new_facts >= MIN_NEW_FACTS_FOR_ASSESSMENT || force_full {
            // Full assessment: scan every unique concept via concept_summary.
            let subjects: HashSet<String> = qa.unique_subjects().into_iter().collect();
            let objects: HashSet<String> = qa.unique_objects().into_iter().collect();
            let all_terms: HashSet<String> = subjects.union(&objects).cloned().collect();
            self.known_terms = all_terms.clone();

            let mut new_knowledge: HashMap<String, ConceptKnowledge> = HashMap::with_capacity(all_terms.len());

            for term in &all_terms {
                let summary = qa.concept_summary(term);
                let total = summary.definitions.len()
                    + summary.properties.len()
                    + summary.operations.len()
                    + summary.relationships.len();
                let well_defined = summary.definitions.len() >= MIN_DEFINITIONS
                    && summary.properties.len() >= MIN_PROPERTIES;
                let level = if total == UNKNOWN_KNOWLEDGE_THRESHOLD {
                    KnowledgeLevel::Unknown
                } else if total < WEAK_KNOWLEDGE_THRESHOLD {
                    KnowledgeLevel::Weak
                } else if well_defined && summary.rules_involving.len() >= 2 {
                    KnowledgeLevel::Strong
                } else {
                    KnowledgeLevel::Adequate
                };

                new_knowledge.insert(
                    term.clone(),
                    ConceptKnowledge {
                        definitions: summary.definitions.len(),
                        is_defined_as: summary.is_defined_as.len(),
                        properties: summary.properties.len(),
                        operations: summary.operations.len(),
                        total_facts: total,
                        rules_involving: summary.rules_involving.len(),
                        is_well_defined: well_defined,
                        level,
                        last_assessed: self.tick,
                        hierarchy_boosted: false,
                    },
                );
            }

            self.concept_knowledge = new_knowledge;
            // Propagate knowledge levels up the is_a hierarchy.
            // This boosts parents (e.g., "calculus") when children
            // (e.g., "derivative") are well-known.
            self.propagate_hierarchy(qa);
            self.last_assessed_fact_count = total_facts;
            self.initialized = true;
        } else {
            // Lightweight path: only update contradiction state.
            // Uncertainty is updated from existing concept_knowledge + contradictions.
        }

        self.update_uncertainty();
    }

    /// Force a full re-assessment regardless of incremental skip logic.
    /// Useful for the METACOG admin command or when the user explicitly
    /// requests a fresh knowledge picture.
    pub fn force_assess(&mut self, qa: &mut QaEngine) {
        // Reset the tracking counter so the next regular assess() call
        // will also do a full scan (avoids stale data).
        self.last_assessed_fact_count = 0;
        self.assess(qa);
    }

    // ── Persistence ──────────────────────────────────────────────────

    /// Save the metacognition state to a JSON file.
    ///
    /// Serializes all tracked state: concept knowledge, contradictions,
    /// questions, question history, search cooldowns, and uncertainty.
    pub fn save_to_file(&self, path: &str) -> Result<(), String> {
        let json = serde_json::to_string_pretty(self)
            .map_err(|e| format!("Metacognition serialization error: {}", e))?;
        std::fs::write(path, &json)
            .map_err(|e| format!("Metacognition write error: {}", e))?;
        Ok(())
    }

    /// Load metacognition state from a JSON file.
    ///
    /// Returns `None` if the file does not exist (first run).
    /// Returns an error if the file exists but is malformed.
    pub fn load_from_file(path: &str) -> Result<Option<Self>, String> {
        match std::fs::read_to_string(path) {
            Ok(json) => {
                let engine: Self = serde_json::from_str(&json)
                    .map_err(|e| format!("Metacognition deserialization error: {}", e))?;
                Ok(Some(engine))
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(e) => Err(format!("Metacognition read error: {}", e)),
        }
    }

    /// Quick scan: only check a specific concept (lighter than full assess).
    pub fn assess_concept(&mut self, qa: &mut QaEngine, concept: &str) -> ConceptKnowledge {
        let summary = qa.concept_summary(concept);
        let total = summary.definitions.len()
            + summary.properties.len()
            + summary.operations.len()
            + summary.relationships.len();
        let well_defined = summary.definitions.len() >= MIN_DEFINITIONS
            && summary.properties.len() >= MIN_PROPERTIES;
        let level = if total == UNKNOWN_KNOWLEDGE_THRESHOLD {
            KnowledgeLevel::Unknown
        } else if total < WEAK_KNOWLEDGE_THRESHOLD {
            KnowledgeLevel::Weak
        } else if well_defined && summary.rules_involving.len() >= 2 {
            KnowledgeLevel::Strong
        } else {
            KnowledgeLevel::Adequate
        };

        let ck = ConceptKnowledge {
            definitions: summary.definitions.len(),
            is_defined_as: summary.is_defined_as.len(),
            properties: summary.properties.len(),
            operations: summary.operations.len(),
            total_facts: total,
            rules_involving: summary.rules_involving.len(),
            is_well_defined: well_defined,
            level,
            last_assessed: self.tick,
            hierarchy_boosted: false,
        };
        self.concept_knowledge
            .insert(concept.to_string(), ck.clone());
        self.update_uncertainty();
        ck
    }

    /// Recalculate the epistemic uncertainty summary from current knowledge.
    fn update_uncertainty(&mut self) {
        let total = self.concept_knowledge.len();
        if total == 0 {
            self.uncertainty = EpistemicUncertainty {
                overall: 0.5, // moderate uncertainty when nothing is known
                weak_concept_count: 0,
                unknown_concept_count: 0,
                unresolved_contradictions: self
                    .contradictions
                    .iter()
                    .filter(|c| !c.resolved)
                    .count(),
                knowledge_coverage: 0.0,
                total_concepts: 0,
                most_uncertain: Vec::new(),
            };
            return;
        }

        let weak_count = self
            .concept_knowledge
            .values()
            .filter(|k| k.level == KnowledgeLevel::Weak)
            .count();
        let unknown_count = self
            .concept_knowledge
            .values()
            .filter(|k| k.level == KnowledgeLevel::Unknown)
            .count();
        let well_defined_count = self
            .concept_knowledge
            .values()
            .filter(|k| k.is_well_defined)
            .count();
        let unresolved = self
            .contradictions
            .iter()
            .filter(|c| !c.resolved)
            .count();

        // Coverage: fraction of concepts that are well-defined
        let coverage = if total > 0 {
            well_defined_count as f64 / total as f64
        } else {
            0.0
        };

        // Uncertainty factors:
        //   - weak/unknown fraction (0..1)
        //   - unresolved contradiction penalty (0..0.2)
        //   - inverse coverage penalty (0..1)
        let weak_unknown_frac = (weak_count + unknown_count) as f64 / total as f64;
        let contradiction_penalty = (unresolved as f64).min(5.0) * 0.04; // max 0.2
        let coverage_penalty = 1.0 - coverage;
        let overall =
            (weak_unknown_frac * 0.5 + contradiction_penalty + coverage_penalty * 0.3).min(1.0);

        // Find most uncertain concepts (weakest knowledge)
        let mut uncertain: Vec<(String, f64)> = Vec::new();
        for (term, k) in &self.concept_knowledge {
            let score = match k.level {
                KnowledgeLevel::Unknown => 1.0,
                KnowledgeLevel::Weak => 0.7 - (k.total_facts as f64 * 0.1),
                KnowledgeLevel::Adequate => 0.3,
                KnowledgeLevel::Strong => 0.05,
            };
            uncertain.push((term.clone(), score));
        }
        uncertain.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        let most_uncertain: Vec<String> = uncertain
            .into_iter()
            .take(5)
            .map(|(t, _)| t)
            .collect();

        self.uncertainty = EpistemicUncertainty {
            overall,
            weak_concept_count: weak_count,
            unknown_concept_count: unknown_count,
            unresolved_contradictions: unresolved,
            knowledge_coverage: coverage,
            total_concepts: total,
            most_uncertain,
        };
    }

    // ── Contradiction Detection ───────────────────────────────────────

    /// Scan the QA engine for contradictory facts and update the
    /// contradictions list.
    ///
    /// Uses `qa.all_fact_triples()` — a single O(N) pass over all facts with
    /// **no NHD, no term cache, no `concept_summary` per subject**.  Just
    /// string comparison for subject grouping and antonym detection.
    ///
    /// Scans ALL facts by subject group and checks for antonym verb pairs
    /// (e.g., "raise" vs "cut") or antonym object pairs with the same verb.
    pub fn detect_contradictions(&mut self, qa: &QaEngine) {
        self.tick();

        // ── 1. Single O(N) pass: get all triples ──────────────────────
        let triples = qa.all_fact_triples();

        // ── 2. Group by normalized subject ─────────────────────────────
        // This is pure string comparison — no NHD at all.
        let mut by_subject: HashMap<String, Vec<(String, String)>> = HashMap::new();
        for (subj, verb, obj) in &triples {
            by_subject
                .entry(subj.clone())
                .or_default()
                .push((verb.clone(), obj.clone()));
        }

        // ── 3. For each subject, check every (verb, object) pair ──────
        let mut new_contradictions: Vec<DetectedContradiction> = Vec::new();

        for (subj_lower, pairs) in &by_subject {
            for i in 0..pairs.len() {
                for j in (i + 1)..pairs.len() {
                    let (v_a, o_a) = &pairs[i];
                    let (v_b, o_b) = &pairs[j];
                    let a_verb = v_a.trim().to_lowercase();
                    let b_verb = v_b.trim().to_lowercase();
                    let a_obj = o_a.trim().to_lowercase();
                    let b_obj = o_b.trim().to_lowercase();

                    // Same object + antonym verb → contradiction
                    // OR same verb + antonym object → contradiction
                    let is_contra = (a_obj == b_obj && !a_obj.is_empty()
                        && crate::narrative::is_antonym(&a_verb, &b_verb))
                        || (a_verb == b_verb && !a_verb.is_empty()
                            && crate::narrative::is_object_antonym(&a_obj, &b_obj));
                    if !is_contra {
                        continue;
                    }

                    // Check if this pair is already recorded (order-independent).
                    let already_recorded = self.contradictions.iter().any(|c| {
                        c.subject == *subj_lower
                            && ((c.verb_a == a_verb && c.verb_b == b_verb)
                                || (c.verb_a == b_verb && c.verb_b == a_verb))
                    });
                    if !already_recorded {
                        new_contradictions.push(DetectedContradiction {
                            subject: subj_lower.clone(),
                            verb_a: a_verb,
                            object_a: a_obj,
                            verb_b: b_verb,
                            object_b: b_obj,
                            tick: self.tick,
                            resolved: false,
                        });
                    }
                }
            }
        }

        self.contradictions.extend(new_contradictions);
        self.update_uncertainty();
    }

    /// Return all unresolved contradictions.
    pub fn unresolved_contradictions(&self) -> Vec<&DetectedContradiction> {
        self.contradictions
            .iter()
            .filter(|c| !c.resolved)
            .collect()
    }

    /// Mark a contradiction as resolved (e.g., by acquiring clarifying info).
    pub fn resolve_contradiction(&mut self, index: usize) -> bool {
        if let Some(c) = self.contradictions.get_mut(index) {
            c.resolved = true;
            self.update_uncertainty();
            true
        } else {
            false
        }
    }

    // ── Question Generation ───────────────────────────────────────────

    /// Generate questions about concepts the system is uncertain about.
    ///
    /// Scans concepts by knowledge level and formulates questions:
    ///   - Unknown concepts → "What is X?"
    ///   - Weak concepts → "What are the properties of X?" / "What is X used for?"
    ///   - Contradicted → "Is X Y or Z?" (depends on evidence)
    ///   - Known but isolated → "How does X relate to Y?"
    ///
    /// **TTL purge**: Questions older than `QUESTIONS_TTL` ticks are removed
    /// automatically.  Questions about concepts that have since gained
    /// knowledge (e.g., Unknown → Adequate) are also removed — no point
    /// asking "What is X?" when we already know.
    pub fn generate_questions(&mut self, _qa: &QaEngine) {
        self.tick();

        // ── Phase 0: TTL purge — remove stale or resolved-by-time questions ──
        let ttl = self.tick.saturating_sub(QUESTIONS_TTL);
        let before_purge = self.questions.len();
        self.questions.retain(|q| {
            // Keep if: still within TTL AND (unanswered or concept still Weak/Unknown)
            if q.tick < ttl {
                return false; // expired by age
            }
            // Re-evaluate: if the concept is now known, drop the question.
            if let Some(k) = self.concept_knowledge.get(&q.topic) {
                match k.level {
                    KnowledgeLevel::Adequate | KnowledgeLevel::Strong => {
                        // We now know this concept — question is resolved by time.
                        return false;
                    }
                    KnowledgeLevel::Weak => {
                        // Still weak — keep the question (but check reason relevance).
                        // If the question was about a missing definition and we now
                        // have one, it's partially resolved.
                        if q.reason == QuestionReason::MissingDefinition
                            && k.definitions > 0
                        {
                            return false;
                        }
                    }
                    KnowledgeLevel::Unknown => {
                        // Still unknown — keep.
                    }
                }
            }
            true
        });
        let purged = before_purge.saturating_sub(self.questions.len());
        if purged > 0 {
            // log only when we actually purge (avoid noise in testing)
        }

        let mut new_questions: Vec<EpistemicQuestion> = Vec::new();

        for (term, k) in &self.concept_knowledge {
            match k.level {
                KnowledgeLevel::Unknown => {
                    new_questions.push(EpistemicQuestion {
                        question: format!("What is {}?", term),
                        topic: term.clone(),
                        uncertainty: 1.0,
                        reason: QuestionReason::UnknownConcept,
                        tick: self.tick,
                        answered: false,
                    });
                }
                KnowledgeLevel::Weak => {
                    // 0–2 facts: ask for definition or properties.
                    if k.definitions == 0 {
                        new_questions.push(EpistemicQuestion {
                            question: format!("What is {}? I have only {} facts about it.", term, k.total_facts),
                            topic: term.clone(),
                            uncertainty: 0.7,
                            reason: QuestionReason::MissingDefinition,
                            tick: self.tick,
                            answered: false,
                        });
                    } else {
                        new_questions.push(EpistemicQuestion {
                            question: format!("What are the properties of {}? I know {} facts but none are properties.", term, k.total_facts),
                            topic: term.clone(),
                            uncertainty: 0.65,
                            reason: QuestionReason::GapInKnowledge,
                            tick: self.tick,
                            answered: false,
                        });
                    }
                }
                KnowledgeLevel::Adequate => {
                    // Known but check for contradictions and gaps.
                    let has_contra = self.contradictions.iter().any(|c| {
                        !c.resolved && (c.subject == *term
                            || c.verb_a == *term || c.verb_b == *term
                            || c.object_a == *term || c.object_b == *term)
                    });
                    if has_contra {
                        new_questions.push(EpistemicQuestion {
                            question: format!(
                                "Is {} contradictory? I have conflicting evidence about it.",
                                term
                            ),
                            topic: term.clone(),
                            uncertainty: 0.5,
                            reason: QuestionReason::ContradictoryEvidence,
                            tick: self.tick,
                            answered: false,
                        });
                    }

                    // If it has no rules, ask about relationships.
                    if k.rules_involving == 0 && k.total_facts >= 3 {
                        new_questions.push(EpistemicQuestion {
                            question: format!(
                                "What causes changes in {}? I know facts but no causal rules.",
                                term
                            ),
                            topic: term.clone(),
                            uncertainty: 0.4,
                            reason: QuestionReason::GapInKnowledge,
                            tick: self.tick,
                            answered: false,
                        });
                    }
                }
                KnowledgeLevel::Strong => {
                    // Strongly known — no questions needed.
                }
            }
        }

        // Add questions from contradictions that don't map to a specific concept.
        for c in &self.contradictions {
            if c.resolved {
                continue;
            }
            let already_covered = new_questions.iter().any(|q| {
                q.reason == QuestionReason::ContradictoryEvidence
                    && (q.topic == c.subject
                        || q.topic == c.verb_a
                        || q.topic == c.object_a)
            });
            if !already_covered {
                new_questions.push(EpistemicQuestion {
                    question: format!(
                        "I have contradictory evidence about {}: {} {} vs {} {}. Which is correct?",
                        c.subject,
                        c.verb_a,
                        c.object_a,
                        c.verb_b,
                        c.object_b,
                    ),
                    topic: c.subject.clone(),
                    uncertainty: 0.6,
                    reason: QuestionReason::ContradictoryEvidence,
                    tick: self.tick,
                    answered: false,
                });
            }
        }

        // Merge into the question queue: new questions go to the front,
        // but we skip duplicates.
        for q in new_questions {
            let is_duplicate = self.questions.iter().any(|existing| {
                existing.topic == q.topic && existing.reason == q.reason
            });
            if !is_duplicate {
                if self.questions.len() >= MAX_QUESTIONS {
                    self.questions.pop_back(); // drop oldest
                }
                self.questions.push_front(q);
            }
        }

        // ── Record question count trend sample ───────────────────────────
        let pending_count = self.questions.iter().filter(|q| !q.answered).count();
        self.question_trend.push_back(pending_count);
        self.trend_ticks.push_back(self.tick);
        while self.question_trend.len() > TREND_WINDOW {
            self.question_trend.pop_front();
            self.trend_ticks.pop_front();
        }

        // Update EWMA trend indicators.
        // Fast EMA (α=0.30) catches quick shifts; slow EMA (α=0.10) is the
        // long-term baseline.  Direction = sign(fast - slow).
        const EMA_ALPHA_SHORT: f64 = 0.30;
        const EMA_ALPHA_LONG: f64 = 0.10;
        let count = pending_count as f64;
        if !self.trend_ema_seeded {
            self.trend_ema_short = count;
            self.trend_ema_long = count;
            self.trend_ema_seeded = true;
        } else {
            self.trend_ema_short = EMA_ALPHA_SHORT * count + (1.0 - EMA_ALPHA_SHORT) * self.trend_ema_short;
            self.trend_ema_long = EMA_ALPHA_LONG * count + (1.0 - EMA_ALPHA_LONG) * self.trend_ema_long;
        }
    }

    /// Mark a question as answered.
    pub fn answer_question(&mut self, index: usize, satisfactorily: bool) -> bool {
        if let Some(q) = self.questions.get_mut(index) {
            q.answered = true;
            self.question_history.push(QuestionRecord {
                question: q.question.clone(),
                tick: q.tick,
                answered_tick: self.tick,
                satisfactorily_answered: satisfactorily,
            });
            if self.question_history.len() > MAX_QUESTION_HISTORY {
                self.question_history.remove(0);
            }
            // Remove from active queue
            self.questions.remove(index);
            true
        } else {
            false
        }
    }

    /// Get pending unanswered questions.
    pub fn pending_questions(&self) -> Vec<&EpistemicQuestion> {
        self.questions.iter().filter(|q| !q.answered).collect()
    }

    /// Compute a composite priority score for a question.
    ///
    /// Factors (each contributing additively to the base score):
    ///
    /// | Factor | Weight | Source |
    /// |--------|--------|--------|
    /// | Reason base | 0.3–1.0 | `QuestionReason` (Unknown > MissingDef > Contra > Gap > LowConf) |
    /// | Access frequency | +0.15 max | `total_facts / 50`, capped — frequently-used concepts matter more |
    /// | Causal chain | +0.10 max | `rules_involving / 10`, capped — unblocking rules has leverage |
    /// | Contradiction severity | +0.10 max | `# unresolved contradictions / 5`, capped — more conflicts = more urgent |
    /// | Age tiebreaker | +0.01 | Older questions win ties |
    ///
    /// Range: **0.30** (fresh LowConfidence about an orphan concept) to **~1.30**
    /// (unknown concept with 50+ facts, 10+ rules, 5+ contradictions).
    fn score_question(&self, q: &EpistemicQuestion) -> f64 {
        let base = match q.reason {
            QuestionReason::UnknownConcept => 1.0,
            QuestionReason::MissingDefinition => 0.7,
            QuestionReason::ContradictoryEvidence => 0.5,
            QuestionReason::GapInKnowledge => 0.4,
            QuestionReason::LowConfidence | QuestionReason::Ambiguous => 0.3,
        };

        // Look up concept knowledge for frequency and rule involvement.
        let knowledge = self.concept_knowledge.get(&q.topic);

        // Access frequency bonus: concepts referenced in more facts are more
        // important to understand correctly.
        let freq_bonus = knowledge
            .map(|k| (k.total_facts as f64).min(50.0) / 50.0 * 0.15)
            .unwrap_or(0.0);

        // Causal chain bonus: concepts that appear in rules are leverage
        // points — understanding them unblocks causal reasoning chains.
        let rule_bonus = knowledge
            .map(|k| (k.rules_involving as f64).min(10.0) / 10.0 * 0.10)
            .unwrap_or(0.0);

        // Contradiction severity bonus: more contradictory pairs = more urgent.
        let contra_count = self
            .contradictions
            .iter()
            .filter(|c| {
                !c.resolved
                    && (c.subject == q.topic
                        || c.verb_a == q.topic
                        || c.object_a == q.topic)
            })
            .count();
        let contra_bonus = (contra_count as f64).min(5.0) / 5.0 * 0.10;

        // Age tiebreaker: older questions are slightly higher.
        let age_bonus = 1.0 / (q.tick.max(1) as f64) * 0.01;

        base + freq_bonus + rule_bonus + contra_bonus + age_bonus
    }

    /// Return the single highest-priority unanswered question, or `None`.
    ///
    /// Uses `score_question()` which composites:
    ///   - Reason base priority (0.3–1.0)
    ///   + Access frequency bonus (up to +0.15)
    ///   + Causal chain bonus (up to +0.10)
    ///   + Contradiction severity bonus (up to +0.10)
    ///   + Age tiebreaker (up to +0.01)
    ///
    /// This means a frequently-used Unknown concept (score ~1.15) strongly
    /// outranks an orphan Weak concept (score ~0.70).
    pub fn top_question(&self) -> Option<&EpistemicQuestion> {
        self.questions
            .iter()
            .filter(|q| !q.answered)
            .max_by(|a, b| {
                let sa = self.score_question(a);
                let sb = self.score_question(b);
                sa.partial_cmp(&sb).unwrap_or(std::cmp::Ordering::Equal)
            })
    }

    /// Return the top N questions sorted by score descending.
    pub fn top_questions(&self, n: usize) -> Vec<(&EpistemicQuestion, f64)> {
        let mut scored: Vec<(&EpistemicQuestion, f64)> = self
            .questions
            .iter()
            .filter(|q| !q.answered)
            .map(|q| (q, self.score_question(q)))
            .collect();
        scored.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        scored.truncate(n);
        scored
    }

    // ── Question Trend ──────────────────────────────────────────────

    /// Aggregate question quality metrics.
    ///
    /// Returns `(current_count, recent_trend, avg_age, trend_direction)` where:
    /// - `current_count`: unanswered questions right now
    /// - `recent_trend`: [`TREND_WINDOW`] most recent counts (oldest first)
    /// - `avg_age`: average age of unanswered questions in ticks
    /// - `trend_direction`: -1 = decreasing (improving), 0 = flat, +1 = increasing (worsening)
    pub fn question_trend_metrics(&self) -> (usize, Vec<usize>, f64, i8) {
        let current_count = self.questions.iter().filter(|q| !q.answered).count();

        let trend: Vec<usize> = self.question_trend.iter().copied().collect();

        // Average age of unanswered questions.
        let mut total_age: u64 = 0;
        let unanswered: Vec<&EpistemicQuestion> =
            self.questions.iter().filter(|q| !q.answered).collect();
        for q in &unanswered {
            total_age = total_age.saturating_add(self.tick.saturating_sub(q.tick));
        }
        let avg_age = if unanswered.is_empty() {
            0.0
        } else {
            total_age as f64 / unanswered.len() as f64
        };

        // Trend direction via EWMA: fast EMA vs slow EMA baseline.
        // A threshold (±1.5 questions) prevents noise from flipping the signal.
        let direction = if !self.trend_ema_seeded || self.question_trend.len() < 3 {
            0
        } else {
            let diff = self.trend_ema_short - self.trend_ema_long;
            if diff > 1.5 {
                1  // increasing (worsening)
            } else if diff < -1.5 {
                -1 // decreasing (improving)
            } else {
                0  // flat
            }
        };

        (current_count, trend, avg_age, direction)
    }

    // ── Structural Gap Analysis ───────────────────────────────────────

    /// Analyse the knowledge graph's bipartite subject↔object structure.
    ///
    /// Uses `qa.all_fact_triples()` — a single O(N) pass with no NHD.
    /// Builds sets of subjects and objects, then identifies:
    ///
    ///   - **Subject-only**: concepts that appear as subjects but never as
    ///     objects. These are "roots" in the graph.
    ///   - **Object-only**: concepts that appear as objects but never as
    ///     subjects. These are "dangling" — referenced but undefined.
    ///   - **Symmetric**: concepts that play both roles (healthy).
    ///   - **Undefined subjects**: subject-only concepts that have no "be"
    ///     or "is" definition fact.
    pub fn structural_gaps(&self, qa: &QaEngine) -> StructuralGaps {
        let triples = qa.all_fact_triples();

        // Build sets of all subjects and objects.
        let mut subjects: std::collections::BTreeSet<String> = std::collections::BTreeSet::new();
        let mut objects: std::collections::BTreeSet<String> = std::collections::BTreeSet::new();
        // Track which subjects have a "be" or "is" definition.
        let mut defined_subjects: std::collections::BTreeSet<String> =
            std::collections::BTreeSet::new();

        for (subj, verb, obj) in &triples {
            if !subj.is_empty() {
                subjects.insert(subj.clone());
            }
            if !obj.is_empty() {
                objects.insert(obj.clone());
            }
            // A subject that has a "be" or "is" fact about itself counts as
            // having a self-definition (e.g., "the_fed be a_central_bank").
            let v = verb.trim().to_lowercase();
            if (v == "be" || v == "is") && !subj.is_empty() {
                defined_subjects.insert(subj.clone());
            }
        }

        // Compute the three sets.
        let subject_only: Vec<String> = subjects
            .difference(&objects)
            .cloned()
            .collect();
        let object_only: Vec<String> = objects
            .difference(&subjects)
            .cloned()
            .collect();
        let symmetric: Vec<&String> = subjects
            .intersection(&objects)
            .collect();

        let total_unique = subjects.len() + objects.len() - symmetric.len();
        let undefined_subjects = subject_only
            .iter()
            .filter(|s| !defined_subjects.contains(*s))
            .count();

        StructuralGaps {
            total_unique,
            symmetric_count: symmetric.len(),
            subject_only,
            object_only,
            undefined_subjects,
        }
    }

    // ── Hierarchy Propagation ─────────────────────────────────────────

    /// Propagate knowledge levels upward through the `is_a` hierarchy.
    ///
    /// If "derivative" is Strong and "derivative is_a calculus_concept",
    /// then "calculus_concept" should be boosted — the system knows something
    /// about it indirectly through its well-known child.
    ///
    /// Uses `qa.all_fact_triples()` — a single O(N) pass to extract `is_a`
    /// relationships, then walks the parent→children map.
    ///
    /// Rules:
    ///   - A parent is boosted ONE level if ANY child is 2+ levels higher.
    ///   - Boosted concepts cap at `Adequate` (never pushed to Strong).
    ///   - The `hierarchy_boosted` flag is set so reports can show the diff.
    ///   - Only concepts already in `concept_knowledge` are considered.
    pub fn propagate_hierarchy(&mut self, qa: &QaEngine) {
        let triples = qa.all_fact_triples();

        // ── 1. Build parent → [children] from is_a facts ──────────────
        let mut parent_to_children: std::collections::HashMap<String, Vec<String>> =
            std::collections::HashMap::new();

        for (subj, verb, obj) in &triples {
            let v = verb.trim().to_lowercase();
            if v == "is_a" || v == "is_an" || v == "are_a"
                || v == "is_a_type_of" || v == "is_a_kind_of"
                || v.contains("type_of") || v.contains("kind_of")
            {
                if !subj.is_empty() && !obj.is_empty() {
                    parent_to_children
                        .entry(obj.clone())
                        .or_default()
                        .push(subj.clone());
                }
            } else if v == "be"
                && (obj.starts_with("a_") || obj.starts_with("an_") || obj.starts_with("the_"))
            {
                // "square be a_rectangle" → square is_a rectangle
                let parent = obj.trim_start_matches("a_")
                    .trim_start_matches("an_")
                    .trim_start_matches("the_")
                    .to_string();
                if !parent.is_empty() && parent != *subj {
                    parent_to_children
                        .entry(parent)
                        .or_default()
                        .push(subj.clone());
                }
            }
        }

        // ── 2. Iterative propagation until fixed point ────────────────
        // We iterate because boosting one parent may create a cascade:
        //   square (Adequate) → rectangle (boosted) → polygon (boosted)
        // HashMap iteration order is not deterministic, so a single pass
        // may miss chains.  We loop until no further changes occur.
        loop {
            let mut changed = false;
            let parents: Vec<String> = self.concept_knowledge.keys()
                .filter(|k| parent_to_children.contains_key(*k))
                .cloned()
                .collect();

            for parent in &parents {
                let children = match parent_to_children.get(parent) {
                    Some(c) => c,
                    None => continue,
                };

                // Find the highest child level.
                let max_child_level = children
                    .iter()
                    .filter_map(|c| self.concept_knowledge.get(c))
                    .map(|k| &k.level)
                    .max_by(|a, b| level_rank(a).cmp(&level_rank(b)))
                    .cloned();

                let parent_knowledge = match self.concept_knowledge.get_mut(parent) {
                    Some(k) => k,
                    None => continue,
                };

                // Boost the parent ONE level if ANY child is at a higher level.
                // This catches cases like "derivative" (well-known) being a child
                // of "calculus" (only referenced as object, weakly known).
                // Never boost past Adequate (hierarchy alone doesn't make Strong).
                if let Some(ref child_level) = max_child_level {
                    let parent_rank = level_rank(&parent_knowledge.level);
                    let child_rank = level_rank(child_level);
                    if child_rank > parent_rank && parent_rank < level_rank(&KnowledgeLevel::Adequate) {
                        parent_knowledge.level = KnowledgeLevel::Adequate;
                        parent_knowledge.hierarchy_boosted = true;
                        changed = true;
                    }
                }
            }

            if !changed {
                break;
            }
        }
    }

    /// Check whether a topic is on cooldown (recently searched).
    pub fn is_topic_on_cooldown(&self, topic: &str) -> bool {
        self.searched_topics
            .get(topic)
            .map(|last_searched| self.tick.saturating_sub(*last_searched) < EPISTEMIC_SEARCH_COOLDOWN_TICKS)
            .unwrap_or(false)
    }

    /// Mark a topic as searched (starts its cooldown).
    pub fn mark_topic_searched(&mut self, topic: &str) {
        self.searched_topics.insert(topic.to_string(), self.tick);
    }

    /// Return topics that are eligible for search (not on cooldown).
    pub fn searchable_topics(&self) -> Vec<String> {
        let mut topics: Vec<String> = self
            .questions
            .iter()
            .filter(|q| !q.answered && !self.is_topic_on_cooldown(&q.topic))
            .map(|q| q.topic.clone())
            .collect();
        topics.sort();
        topics.dedup();
        topics
    }

    // ── Knowledge Report Generation ───────────────────────────────────

    /// Generate a full knowledge report as a formatted string.
    pub fn knowledge_report(&self, qa: &QaEngine) -> String {
        let mut report = String::new();

        // Header
        report.push_str(&format!(
            "═══ METACOGNITIVE KNOWLEDGE REPORT ═══\n"
        ));
        report.push_str(&format!(
            "Tick: {}  |  Initialized: {}\n\n",
            self.tick,
            self.initialized
        ));

        // Overall statistics
        report.push_str(&format!(
            "── Knowledge Base Statistics ──\n"
        ));
        report.push_str(&format!(
            "  Total facts:     {}\n",
            qa.fact_count()
        ));
        report.push_str(&format!(
            "  Causal rules:    {}\n",
            qa.rule_count()
        ));
        report.push_str(&format!(
            "  Unique concepts: {}\n",
            self.uncertainty.total_concepts
        ));
        report.push_str(&format!(
            "  Contradictions:  {} ({} unresolved)\n",
            self.contradictions.len(),
            self.uncertainty.unresolved_contradictions
        ));
        report.push('\n');

        // Epistemic uncertainty
        report.push_str(&format!("── Epistemic Uncertainty ──\n"));
        report.push_str(&format!(
            "  Overall uncertainty:    {:.2}\n",
            self.uncertainty.overall
        ));
        report.push_str(&format!(
            "  Weak concepts:          {}\n",
            self.uncertainty.weak_concept_count
        ));
        report.push_str(&format!(
            "  Unknown concepts:       {}\n",
            self.uncertainty.unknown_concept_count
        ));
        report.push_str(&format!(
            "  Knowledge coverage:     {:.1}%\n",
            self.uncertainty.knowledge_coverage * 100.0
        ));
        report.push('\n');

        // Most uncertain concepts
        if !self.uncertainty.most_uncertain.is_empty() {
            report.push_str(&format!("── Most Uncertain Concepts ──\n"));
            for term in &self.uncertainty.most_uncertain {
                if let Some(k) = self.concept_knowledge.get(term) {
                    let level_str = match k.level {
                        KnowledgeLevel::Unknown => "UNKNOWN",
                        KnowledgeLevel::Weak => "WEAK",
                        KnowledgeLevel::Adequate => {
                            if k.hierarchy_boosted {
                                "ADEQUATE (boosted via children)"
                            } else {
                                "ADEQUATE"
                            }
                        }
                        KnowledgeLevel::Strong => "STRONG",
                    };
                    report.push_str(&format!(
                        "  {} — {} ({} facts, {} defs, {} props)\n",
                        term,
                        level_str,
                        k.total_facts,
                        k.definitions,
                        k.properties,
                    ));
                }
            }
            report.push('\n');
        }

        // Unresolved contradictions
        let unresolved: Vec<&DetectedContradiction> = self
            .contradictions
            .iter()
            .filter(|c| !c.resolved)
            .collect();
        if !unresolved.is_empty() {
            report.push_str(&format!("── Unresolved Contradictions ──\n"));
            for c in &unresolved {
                report.push_str(&format!(
                    "  {}: {} {} vs {} {}\n",
                    c.subject, c.verb_a, c.object_a, c.verb_b, c.object_b,
                ));
            }
            report.push('\n');
        }

        // Pending questions
        let pending: Vec<&EpistemicQuestion> = self
            .questions
            .iter()
            .filter(|q| !q.answered)
            .collect();
        if !pending.is_empty() {
            report.push_str(&format!("── Pending Questions ──\n"));
            for (i, q) in pending.iter().enumerate().take(10) {
                let reason_str = match q.reason {
                    QuestionReason::UnknownConcept => "unknown",
                    QuestionReason::MissingDefinition => "no definition",
                    QuestionReason::ContradictoryEvidence => "contradiction",
                    QuestionReason::LowConfidence => "low confidence",
                    QuestionReason::GapInKnowledge => "knowledge gap",
                    QuestionReason::Ambiguous => "ambiguous",
                };
                report.push_str(&format!(
                    "  {}. [{}] {} (uncertainty={:.2})\n",
                    i + 1,
                    reason_str,
                    q.question,
                    q.uncertainty,
                ));
            }
            if pending.len() > 10 {
                report.push_str(&format!(
                    "  ... and {} more\n",
                    pending.len() - 10
                ));
            }
            report.push('\n');
        }

        // Knowledge level distribution
        let mut by_level: HashMap<KnowledgeLevel, usize> = HashMap::new();
        let mut boosted_count = 0usize;
        for k in self.concept_knowledge.values() {
            *by_level.entry(k.level.clone()).or_insert(0) += 1;
            if k.hierarchy_boosted {
                boosted_count += 1;
            }
        }
        report.push_str(&format!("── Knowledge Distribution ──\n"));
        let adequate_total = by_level.get(&KnowledgeLevel::Adequate).unwrap_or(&0);
        let adequate_direct = adequate_total.saturating_sub(boosted_count);
        report.push_str(&format!(
            "  Strong:   {}\n",
            by_level.get(&KnowledgeLevel::Strong).unwrap_or(&0)
        ));
        report.push_str(&format!(
            "  Adequate: {} ({} direct, {} boosted via children)\n",
            adequate_total,
            adequate_direct,
            boosted_count,
        ));
        report.push_str(&format!(
            "  Weak:     {}\n",
            by_level.get(&KnowledgeLevel::Weak).unwrap_or(&0)
        ));
        report.push_str(&format!(
            "  Unknown:  {}\n",
            by_level.get(&KnowledgeLevel::Unknown).unwrap_or(&0)
        ));
        report.push('\n');

        // Structural gaps
        let gaps = self.structural_gaps(qa);
        report.push_str(&format!("── Structural Gaps ──\n"));
        report.push_str(&format!(
            "  Graph symmetry ratio: {:.1}%  ({} symmetric / {} unique)\n",
            gaps.symmetry_ratio() * 100.0,
            gaps.symmetric_count,
            gaps.total_unique,
        ));
        report.push_str(&format!(
            "  Subject-only (roots):  {} ({} undefined)\n",
            gaps.subject_only.len(),
            gaps.undefined_subjects,
        ));
        report.push_str(&format!(
            "  Object-only (dangling): {}\n",
            gaps.object_only.len(),
        ));
        let subj_examples = gaps.subject_only_examples(10);
        if !subj_examples.is_empty() {
            report.push_str(&format!(
                "  Subject-only examples: {}\n",
                subj_examples.join(", "),
            ));
        }
        let obj_examples = gaps.object_only_examples(10);
        if !obj_examples.is_empty() {
            report.push_str(&format!(
                "  Object-only examples: {}\n",
                obj_examples.join(", "),
            ));
        }

        report
    }

    /// Generate a concise one-line epistemic narrative for HUD/log output.
    pub fn epistemic_narrative(&self) -> String {
        if !self.initialized {
            return "I have not yet assessed what I know.".to_string();
        }

        let u = &self.uncertainty;
        if u.total_concepts == 0 {
            return "I know nothing yet.".to_string();
        }

        let certainty_phrase = if u.overall < 0.15 {
            "confident in my knowledge"
        } else if u.overall < 0.30 {
            "mostly confident"
        } else if u.overall < 0.50 {
            "moderately uncertain"
        } else if u.overall < 0.70 {
            "uncertain about many things"
        } else {
            "very uncertain — I need to learn more"
        };

        let mut narrative = format!(
            "I know {} concepts ({:.1}% coverage). I am {}.",
            u.total_concepts,
            u.knowledge_coverage * 100.0,
            certainty_phrase,
        );

        if u.unresolved_contradictions > 0 {
            narrative.push_str(&format!(
                " I have {} unresolved contradictions.",
                u.unresolved_contradictions
            ));
        }

        if u.weak_concept_count > 0 && u.weak_concept_count <= 3 {
            let weak_list = self
                .uncertainty
                .most_uncertain
                .iter()
                .take(3)
                .cloned()
                .collect::<Vec<_>>()
                .join(", ");
            narrative.push_str(&format!(" I should learn more about {}.", weak_list));
        }

        let pending_count = self.questions.iter().filter(|q| !q.answered).count();
        if pending_count > 0 {
            narrative.push_str(&format!(" I have {} open questions.", pending_count));

            // Trend: is the question queue growing or shrinking?
            let (_, _, avg_age, direction) = self.question_trend_metrics();
            let trend_phrase = match direction {
                1 => "growing",
                -1 => "declining",
                _ => "stable",
            };
            narrative.push_str(&format!(
                " Questions are {} (avg age {:.0} ticks).",
                trend_phrase, avg_age,
            ));
        }

        // Mention hierarchy-boosted concepts if they're significant.
        let boosted_count = self.concept_knowledge.values()
            .filter(|k| k.hierarchy_boosted).count();
        if boosted_count >= 3 {
            narrative.push_str(&format!(
                " {} concepts are Adequate through child knowledge.",
                boosted_count
            ));
        }

        narrative
    }

    /// Generate a short string listing the system's doubts (uncertain concepts).
    pub fn doubts_string(&self) -> String {
        if !self.initialized {
            return "No knowledge assessment performed yet.".to_string();
        }

        let weak: Vec<String> = self
            .concept_knowledge
            .iter()
            .filter(|(_, k)| k.level == KnowledgeLevel::Weak)
            .take(20)
            .map(|(t, _)| t.clone())
            .collect();
        let unknown: Vec<String> = self
            .concept_knowledge
            .iter()
            .filter(|(_, k)| k.level == KnowledgeLevel::Unknown)
            .take(10)
            .map(|(t, _)| t.clone())
            .collect();

        let mut s = String::new();
        s.push_str(&format!(
            "── Doubts (uncertainty={:.2}) ──\n",
            self.uncertainty.overall
        ));
        if !weak.is_empty() {
            s.push_str(&format!("Weakly known ({}): ", weak.len()));
            s.push_str(&weak.join(", "));
            s.push('\n');
        }
        if !unknown.is_empty() {
            s.push_str(&format!("Unknown ({}): ", unknown.len()));
            s.push_str(&unknown.join(", "));
            s.push('\n');
        }
        if weak.is_empty() && unknown.is_empty() {
            s.push_str("No doubts — all concepts are adequately known.\n");
        }
        s
    }

    /// Return all detected contradictions (resolved and unresolved).
    pub fn all_contradictions(&self) -> &[DetectedContradiction] {
        &self.contradictions
    }

    /// Number of detected contradictions.
    pub fn contradiction_count(&self) -> usize {
        self.contradictions.len()
    }

    /// Return the knowledge state for a specific concept, if assessed.
    pub fn concept_knowledge(&self, concept: &str) -> Option<&ConceptKnowledge> {
        self.concept_knowledge.get(concept)
    }

    /// Check if a concept is known at all.
    pub fn knows_concept(&self, concept: &str) -> bool {
        self.concept_knowledge
            .get(concept)
            .map(|k| k.level != KnowledgeLevel::Unknown)
            .unwrap_or(false)
    }

    /// Get the number of unique concepts known.
    pub fn known_concept_count(&self) -> usize {
        self.concept_knowledge.len()
    }

    /// Reset all state (for testing).
    pub fn reset(&mut self) {
        self.concept_knowledge.clear();
        self.contradictions.clear();
        self.questions.clear();
        self.question_history.clear();
        self.known_terms.clear();
        self.searched_topics.clear();
        self.question_trend.clear();
        self.trend_ticks.clear();
        self.trend_ema_short = 0.0;
        self.trend_ema_long = 0.0;
        self.trend_ema_seeded = false;
        self.initialized = false;
        self.tick = 0;
        self.last_assessed_fact_count = 0;
        self.assess_call_count = 0;
        self.update_uncertainty();
    }
}

impl Default for MetaCognition {
    fn default() -> Self {
        Self::new()
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// TESTS
// ═══════════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;
    use crate::qa::QaEngine;

    fn seeded_qa() -> QaEngine {
        let mut qa = QaEngine::new();
        qa.store_fact("the_fed", "raise", "rates", "test source");
        qa.store_fact("the_fed", "be", "a_central_bank", "test source");
        qa.store_fact("the_fed", "control", "monetary_policy", "test source");
        qa.store_fact("rates", "be", "a_price_of_money", "test source");
        qa.store_fact("rates", "affect", "borrowing_costs", "test source");
        qa.store_fact("yields", "be", "a_return_on_bonds", "test source");
        qa.store_fact("yields", "rise", "when_rates_rise", "test source");
        qa.store_fact("yields", "reflect", "market_sentiment", "test source");
        qa.store_fact("inflation", "be", "a_rise_in_prices", "test source");
        qa.store_fact("inflation", "erode", "purchasing_power", "test source");
        qa.store_fact("the_fed", "cut", "rates", "contradictory source");
        qa
    }

    // ── Basic Construction ────────────────────────────────────────────

    #[test]
    fn test_metacognition_new() {
        let mc = MetaCognition::new();
        assert_eq!(mc.known_concept_count(), 0);
        assert!(!mc.initialized);
        assert!(mc.pending_questions().is_empty());
    }

    #[test]
    fn test_metacognition_default() {
        let mc: MetaCognition = Default::default();
        assert_eq!(mc.known_concept_count(), 0);
    }

    // ── Knowledge Assessment ──────────────────────────────────────────

    #[test]
    fn test_assess_populates_knowledge() {
        let mut qa = seeded_qa();
        let mut mc = MetaCognition::new();
        mc.assess(&mut qa);

        assert!(mc.initialized);
        assert!(mc.known_concept_count() > 0);
        assert!(mc.uncertainty.total_concepts > 0);
    }

    #[test]
    fn test_assess_concept_returns_knowledge() {
        let mut qa = seeded_qa();
        let mut mc = MetaCognition::new();

        let k = mc.assess_concept(&mut qa, "the_fed");
        assert!(k.total_facts >= 3, "Should have at least 3 facts about the_fed, got {}", k.total_facts);
        // "the_fed" has: raise rates, be a_central_bank, control monetary_policy, cut rates
        // These are relationships (not properties/definitions), so is_well_defined
        // requires properties which we may not have.
        assert_eq!(k.level, KnowledgeLevel::Adequate);
    }

    #[test]
    fn test_unknown_concept_is_unknown() {
        let mut qa = seeded_qa();
        let mut mc = MetaCognition::new();

        let k = mc.assess_concept(&mut qa, "quantum_computing");
        assert_eq!(k.total_facts, 0);
        assert_eq!(k.level, KnowledgeLevel::Unknown);
        assert!(!mc.knows_concept("quantum_computing"));
    }

    #[test]
    fn test_weak_concept_detection() {
        let mut qa = QaEngine::new();
        qa.store_fact("obscure_thing", "appear", "once", "test");
        let mut mc = MetaCognition::new();
        mc.assess(&mut qa);

        let k = mc.concept_knowledge("obscure_thing");
        assert!(k.is_some());
        assert_eq!(k.unwrap().level, KnowledgeLevel::Weak);
    }

    // ── Contradiction Detection ───────────────────────────────────────

    #[test]
    fn test_detect_contradictions_finds_them() {
        let mut qa = seeded_qa(); // has both "the_fed raise rates" and "the_fed cut rates"
        let mut mc = MetaCognition::new();

        // First assess so we have concepts
        mc.assess(&mut qa);
        mc.detect_contradictions(&qa);

        let unresolved = mc.unresolved_contradictions();
        assert!(!unresolved.is_empty(), "Should detect raise/cut contradiction");

        let fed_contra = unresolved.iter().find(|c| c.subject == "the_fed");
        assert!(fed_contra.is_some(), "Should find contradiction about the_fed");
    }

    #[test]
    fn test_resolve_contradiction() {
        let mut qa = seeded_qa();
        let mut mc = MetaCognition::new();
        mc.assess(&mut qa);
        mc.detect_contradictions(&qa);

        let count_before = mc.unresolved_contradictions().len();
        assert!(count_before > 0);

        // Resolve the first one
        let resolved = mc.resolve_contradiction(0);
        assert!(resolved);

        let count_after = mc.unresolved_contradictions().len();
        assert_eq!(count_after, count_before - 1);
    }

    #[test]
    fn test_no_contradictions_on_consistent_kb() {
        let mut qa = QaEngine::new();
        qa.store_fact("a", "be", "b", "test");
        qa.store_fact("c", "be", "d", "test");
        let mut mc = MetaCognition::new();
        mc.assess(&mut qa);
        mc.detect_contradictions(&qa);

        assert!(mc.unresolved_contradictions().is_empty());
    }

    // ── Question Generation ───────────────────────────────────────────

    #[test]
    fn test_generates_questions_for_unknown() {
        let mut qa = QaEngine::new();
        qa.store_fact("known_thing", "be", "a_concept", "test");
        qa.store_fact("known_thing", "have", "properties", "test");
        let mut mc = MetaCognition::new();
        mc.assess(&mut qa);
        mc.generate_questions(&qa);

        let questions = mc.pending_questions();
        // known_thing should be Adequate, so no "what is" question for it.
        // The weak threshold is 3, known_thing has 2 facts.
        let known_qs = questions.iter().filter(|q| q.topic == "known_thing");
        // Should have at least one gap question (no rules, few properties)
        // Actually known_thing has 2 facts, so it's Weak (below 3).
        // Weak with definitions → asks about properties.
        assert!(known_qs.count() >= 1);
    }

    #[test]
    fn test_no_questions_on_empty_kb() {
        let mut qa = QaEngine::new();
        let mut mc = MetaCognition::new();
        mc.assess(&mut qa);
        mc.generate_questions(&qa);

        assert!(mc.pending_questions().is_empty());
    }

    #[test]
    fn test_question_deduplication() {
        let mut qa = QaEngine::new();
        qa.store_fact("mystery", "appear", "once", "test");
        let mut mc = MetaCognition::new();
        mc.assess(&mut qa);
        mc.generate_questions(&qa);
        mc.generate_questions(&qa); // second call should not duplicate

        let mystery_qs = mc
            .pending_questions()
            .iter()
            .filter(|q| q.topic == "mystery")
            .count();
        assert_eq!(mystery_qs, 1, "Should not duplicate questions");
    }

    #[test]
    fn test_answer_question_removes_it() {
        let mut qa = QaEngine::new();
        qa.store_fact("mystery", "appear", "once", "test");
        let mut mc = MetaCognition::new();
        mc.assess(&mut qa);
        mc.generate_questions(&qa);

        let before = mc.pending_questions().len();
        assert!(before > 0);

        let answered = mc.answer_question(0, true);
        assert!(answered);

        let after = mc.pending_questions().len();
        assert_eq!(after, before - 1);
    }

    #[test]
    fn test_top_question_returns_highest_uncertainty() {
        let mut qa = QaEngine::new();
        // Populate with multiple well-defined concepts plus one unknown.
        // The unknown should win top_question regardless of iteration order.
        qa.store_fact("math", "be", "a_discipline", "test");
        qa.store_fact("math", "has", "theorems", "test");
        qa.store_fact("physics", "be", "a_science", "test");
        qa.store_fact("physics", "has", "laws", "test");
        qa.store_fact("chemistry", "be", "a_field", "test");
        qa.store_fact("chemistry", "has", "reactions", "test");
        let mut mc = MetaCognition::new();
        mc.assess(&mut qa);
        mc.generate_questions(&qa);
        // Inject an Unknown question after generation so it sits at the front.
        mc.questions.push_front(EpistemicQuestion {
            question: "What is quantum_stuff?".to_string(),
            topic: "quantum_stuff".to_string(),
            uncertainty: 1.0,
            reason: QuestionReason::UnknownConcept,
            tick: 0,
            answered: false,
        });

        let top = mc.top_question();
        assert!(top.is_some(), "Should have a top question");
        assert_eq!(
            top.unwrap().topic, "quantum_stuff",
            "UnknownConcept (quantum_stuff) should outrank all Weak/Adequate questions"
        );
    }

    #[test]
    fn test_top_question_unknown_beats_weak() {
        let mut qa = QaEngine::new();
        qa.store_fact("weak_thing", "exist", "barely", "test");
        let mut mc = MetaCognition::new();
        mc.assess(&mut qa);
        // Manually inject an unknown-concept question to test priority.
        mc.questions.push_front(EpistemicQuestion {
            question: "What is unknown_thing?".to_string(),
            topic: "unknown_thing".to_string(),
            uncertainty: 1.0,
            reason: QuestionReason::UnknownConcept,
            tick: 0,
            answered: false,
        });
        mc.generate_questions(&qa);

        let top = mc.top_question();
        assert!(top.is_some());
        assert_eq!(top.unwrap().topic, "unknown_thing",
            "UnknownConcept should outrank Weak");
    }

    #[test]
    fn test_score_question_increases_with_frequency() {
        // Directly test the score_question method, bypassing concept_summary noise.
        // Two questions at the same reason level but with different concept knowledge.
        let mut mc = MetaCognition::new();
        mc.assess_call_count = 1;
        mc.initialized = true;

        let q_rare = EpistemicQuestion {
            question: "What is rare_thing?".to_string(),
            topic: "rare_thing".to_string(),
            uncertainty: 0.7,
            reason: QuestionReason::MissingDefinition,
            tick: 1,
            answered: false,
        };
        let q_freq = EpistemicQuestion {
            question: "What is frequent_thing?".to_string(),
            topic: "frequent_thing".to_string(),
            uncertainty: 0.7,
            reason: QuestionReason::MissingDefinition,
            tick: 2,
            answered: false,
        };

        // Inject concept knowledge with different fact counts.
        mc.concept_knowledge.insert("rare_thing".to_string(), ConceptKnowledge {
            definitions: 0, is_defined_as: 0, properties: 0, operations: 0,
            total_facts: 1, rules_involving: 0, is_well_defined: false,
            level: KnowledgeLevel::Weak, last_assessed: 1, hierarchy_boosted: false,
        });
        mc.concept_knowledge.insert("frequent_thing".to_string(), ConceptKnowledge {
            definitions: 0, is_defined_as: 0, properties: 0, operations: 0,
            total_facts: 20, rules_involving: 0, is_well_defined: false,
            level: KnowledgeLevel::Weak, last_assessed: 1, hierarchy_boosted: false,
        });

        mc.questions.push_back(q_rare.clone());
        mc.questions.push_back(q_freq.clone());

        let s_rare = mc.score_question(&q_rare);
        let s_freq = mc.score_question(&q_freq);

        assert!(
            s_freq > s_rare,
            "frequent_thing (20 facts) should score higher than rare_thing (1 fact): {:.6} vs {:.6}",
            s_freq, s_rare,
        );
        // The frequency bonus is 0.15 * min(facts, 50) / 50.
        // 20 facts → bonus = 0.15 * 20/50 = 0.06
        // 1 fact  → bonus = 0.15 * 1/50  = 0.003
        assert!((s_freq - s_rare - 0.057).abs() < 0.005,
            "Frequency difference should be ~0.057, got {:.6}", s_freq - s_rare);
    }

    #[test]
    fn test_score_question_increases_with_rules() {
        let mut mc = MetaCognition::new();
        mc.assess_call_count = 1;
        mc.initialized = true;

        let q_plain = EpistemicQuestion {
            question: "What is plain_thing?".to_string(),
            topic: "plain_thing".to_string(),
            uncertainty: 0.7,
            reason: QuestionReason::MissingDefinition,
            tick: 1,
            answered: false,
        };
        let q_rules = EpistemicQuestion {
            question: "What is rule_thing?".to_string(),
            topic: "rule_thing".to_string(),
            uncertainty: 0.7,
            reason: QuestionReason::MissingDefinition,
            tick: 2,
            answered: false,
        };

        // Same fact count, but rule_thing has rules involving it.
        mc.concept_knowledge.insert("plain_thing".to_string(), ConceptKnowledge {
            definitions: 0, is_defined_as: 0, properties: 0, operations: 0,
            total_facts: 5, rules_involving: 0, is_well_defined: false,
            level: KnowledgeLevel::Weak, last_assessed: 1, hierarchy_boosted: false,
        });
        mc.concept_knowledge.insert("rule_thing".to_string(), ConceptKnowledge {
            definitions: 0, is_defined_as: 0, properties: 0, operations: 0,
            total_facts: 5, rules_involving: 5, is_well_defined: false,
            level: KnowledgeLevel::Weak, last_assessed: 1, hierarchy_boosted: false,
        });

        mc.questions.push_back(q_plain.clone());
        mc.questions.push_back(q_rules.clone());

        let s_plain = mc.score_question(&q_plain);
        let s_rules = mc.score_question(&q_rules);

        assert!(
            s_rules > s_plain,
            "rule_thing (5 rules) should score higher than plain_thing (0 rules): {:.6} vs {:.6}",
            s_rules, s_plain,
        );
        // Rule bonus = 0.10 * min(5, 10) / 10 = 0.05
        assert!((s_rules - s_plain - 0.05).abs() < 0.005,
            "Rule difference should be ~0.05, got {:.6}", s_rules - s_plain);
    }

    #[test]
    fn test_top_questions_returns_sorted_descending() {
        // Three questions with clearly different scores should be returned
        // in descending order regardless of insertion order.
        let mut mc = MetaCognition::new();
        mc.initialized = true;

        // Push them in reverse priority order.
        mc.questions.push_back(EpistemicQuestion {
            question: "Low priority?".to_string(), topic: "low".to_string(),
            uncertainty: 0.3, reason: QuestionReason::LowConfidence, tick: 1, answered: false,
        });
        mc.questions.push_back(EpistemicQuestion {
            question: "High priority?".to_string(), topic: "high".to_string(),
            uncertainty: 1.0, reason: QuestionReason::UnknownConcept, tick: 2, answered: false,
        });
        mc.questions.push_back(EpistemicQuestion {
            question: "Mid priority?".to_string(), topic: "mid".to_string(),
            uncertainty: 0.5, reason: QuestionReason::ContradictoryEvidence, tick: 3, answered: false,
        });

        let scored = mc.top_questions(10);
        assert_eq!(scored.len(), 3);
        assert_eq!(scored[0].0.topic, "high", "UnknownConcept should be first");
        assert_eq!(scored[1].0.topic, "mid", "Contradiction should be second");
        assert_eq!(scored[2].0.topic, "low", "LowConfidence should be third");
    }
    #[test]
    fn test_topic_cooldown() {
        let mut mc = MetaCognition::new();
        assert!(!mc.is_topic_on_cooldown("never_searched"));
        mc.mark_topic_searched("just_searched");
        assert!(mc.is_topic_on_cooldown("just_searched"),
            "Should be on cooldown immediately after marking");
    }

    #[test]
    fn test_top_question_none_when_all_answered() {
        let mc = MetaCognition::new();
        assert!(mc.top_question().is_none(),
            "Empty metacog should have no top question");
    }

    #[test]
    fn test_searchable_topics_excludes_cooldown() {
        let mut qa = QaEngine::new();
        qa.store_fact("mystery", "appear", "once", "test");
        let mut mc = MetaCognition::new();
        mc.assess(&mut qa);
        mc.generate_questions(&qa);

        let before = mc.searchable_topics();
        assert!(before.contains(&"mystery".to_string()),
            "mystery should be searchable at first");

        mc.mark_topic_searched("mystery");
        let after = mc.searchable_topics();
        assert!(!after.contains(&"mystery".to_string()),
            "mystery should not be searchable after marking");
    }

    // ── Reports ───────────────────────────────────────────────────────

    #[test]
    fn test_knowledge_report_contains_stats() {
        let mut qa = seeded_qa();
        let mut mc = MetaCognition::new();
        mc.assess(&mut qa);
        mc.detect_contradictions(&qa);
        mc.generate_questions(&qa);

        let report = mc.knowledge_report(&qa);
        assert!(report.contains("METACOGNITIVE"));
        assert!(report.contains("Knowledge Base Statistics"));
        assert!(report.contains("Epistemic Uncertainty"));
    }

    #[test]
    fn test_epistemic_narrative_produces_output() {
        let mut qa = seeded_qa();
        let mut mc = MetaCognition::new();
        let narrative_before = mc.epistemic_narrative();
        assert!(narrative_before.contains("not yet assessed"));

        mc.assess(&mut qa);
        let narrative_after = mc.epistemic_narrative();
        assert!(!narrative_after.is_empty());
        assert!(!narrative_after.contains("not yet assessed"));
    }

    #[test]
    fn test_doubts_string() {
        let mut qa = QaEngine::new();
        qa.store_fact("barely_known", "exist", "somewhere", "test");
        let mut mc = MetaCognition::new();
        let s_before = mc.doubts_string();
        assert!(s_before.contains("No knowledge assessment"));

        mc.assess(&mut qa);
        let s = mc.doubts_string();
        assert!(s.contains("barely_known"));
    }

    // ── Reset ─────────────────────────────────────────────────────────

    #[test]
    fn test_reset_clears_state() {
        let mut qa = seeded_qa();
        let mut mc = MetaCognition::new();
        mc.assess(&mut qa);
        mc.detect_contradictions(&qa);
        mc.generate_questions(&qa);

        assert!(mc.initialized);
        assert!(mc.known_concept_count() > 0);

        mc.reset();
        assert!(!mc.initialized);
        assert_eq!(mc.known_concept_count(), 0);
        assert!(mc.pending_questions().is_empty());
    }

    // ── Persistence ───────────────────────────────────────────────────

    #[test]
    fn test_save_and_load_roundtrip() {
        let mut qa = seeded_qa();
        let mut mc = MetaCognition::new();
        mc.assess(&mut qa);
        mc.detect_contradictions(&qa);
        mc.generate_questions(&qa);
        mc.force_assess(&mut qa);

        let path = "/tmp/test_metacognition_save.json";
        let _ = std::fs::remove_file(path);
        mc.save_to_file(path).expect("Save should succeed");

        let loaded = MetaCognition::load_from_file(path)
            .expect("Load should succeed")
            .expect("File exists so should return Some");

        assert_eq!(loaded.initialized, mc.initialized);
        assert_eq!(loaded.known_concept_count(), mc.known_concept_count());
        assert_eq!(loaded.contradiction_count(), mc.contradiction_count());
        assert_eq!(loaded.pending_questions().len(), mc.pending_questions().len());
        assert!((loaded.uncertainty.overall - mc.uncertainty.overall).abs() < 0.001);

        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn test_load_from_missing_file_returns_none() {
        let path = "/tmp/test_metacognition_nonexistent.json";
        let _ = std::fs::remove_file(path);
        let result = MetaCognition::load_from_file(path)
            .expect("Missing file should not error");
        assert!(result.is_none(), "Missing file should return None");
    }

    // ── Structural Gaps ──────────────────────────────────────────────

    #[test]
    fn test_structural_gaps_empty_kb() {
        let qa = QaEngine::new();
        let mc = MetaCognition::new();
        let gaps = mc.structural_gaps(&qa);
        assert_eq!(gaps.total_unique, 0);
        assert_eq!(gaps.symmetry_ratio(), 1.0);
        assert!(gaps.subject_only.is_empty());
        assert!(gaps.object_only.is_empty());
    }

    #[test]
    fn test_structural_gaps_symmetric_is_healthy() {
        let mut qa = QaEngine::new();
        // A appears as both subject and object → symmetric.
        qa.store_fact("a", "be", "b", "test");
        qa.store_fact("b", "be", "a", "test");
        let mc = MetaCognition::new();
        let gaps = mc.structural_gaps(&qa);
        assert_eq!(gaps.symmetric_count, 2, "a and b are both symmetric");
        assert_eq!(gaps.subject_only.len(), 0);
        assert_eq!(gaps.object_only.len(), 0);
        assert!((gaps.symmetry_ratio() - 1.0).abs() < 0.001);
    }

    #[test]
    fn test_structural_gaps_subject_only() {
        let mut qa = QaEngine::new();
        // a is subject, b is object, c is both.
        qa.store_fact("a", "likes", "c", "test");
        qa.store_fact("b", "hates", "c", "test");
        qa.store_fact("c", "be", "a_concept", "test");
        let mc = MetaCognition::new();
        let gaps = mc.structural_gaps(&qa);

        // a and b are subject-only (never appear as objects)
        // c is symmetric (appears as both subject and object)
        // "a_concept" is object-only (only appears as object of "be")
        assert_eq!(gaps.subject_only.len(), 2, "a and b are subject-only");
        assert!(gaps.subject_only.contains(&"a".to_string()));
        assert!(gaps.subject_only.contains(&"b".to_string()));
        assert_eq!(gaps.object_only.len(), 1, "a_concept is object-only");
        assert!(gaps.object_only.contains(&"a_concept".to_string()));
        assert_eq!(gaps.symmetric_count, 1, "c is symmetric");
    }

    #[test]
    fn test_structural_gaps_object_only_are_dangling() {
        let mut qa = QaEngine::new();
        // "dangling_thing" appears only as an object, never as a subject.
        qa.store_fact("system", "references", "dangling_thing", "test");
        qa.store_fact("system", "uses", "another_dangling", "test");
        let mc = MetaCognition::new();
        let gaps = mc.structural_gaps(&qa);

        assert!(gaps.object_only.contains(&"dangling_thing".to_string()),
            "dangling_thing should be object-only");
        assert_eq!(gaps.object_only.len(), 2, "two dangling objects");
        assert_eq!(gaps.symmetric_count, 0, "no symmetric concepts");
        // "system" is subject-only
        assert_eq!(gaps.subject_only.len(), 1, "system is subject-only");
    }

    #[test]
    fn test_structural_gaps_undefined_subjects() {
        let mut qa = QaEngine::new();
        // "defined_thing" has a "be" fact → defined.
        // "undefined_thing" only has non-"be" facts → undefined.
        qa.store_fact("defined_thing", "be", "a_good_concept", "test");
        qa.store_fact("defined_thing", "has", "properties", "test");
        qa.store_fact("undefined_thing", "does", "stuff", "test");
        let mc = MetaCognition::new();
        let gaps = mc.structural_gaps(&qa);

        // Both are subject-only (never appear as objects).
        assert_eq!(gaps.subject_only.len(), 2);
        assert_eq!(gaps.undefined_subjects, 1,
            "undefined_thing has no 'be' fact");
    }

    #[test]
    fn test_structural_gaps_symmetry_ratio() {
        let mut qa = QaEngine::new();
        // 2 symmetric, 1 subject-only, 1 object-only → 4 unique, 2 symmetric
        qa.store_fact("sym_a", "be", "sym_b", "test");
        qa.store_fact("sym_b", "be", "sym_a", "test");
        qa.store_fact("root", "points_to", "sym_a", "test");
        qa.store_fact("sym_b", "points_to", "leaf", "test");
        let mc = MetaCognition::new();
        let gaps = mc.structural_gaps(&qa);

        // Unique concepts: sym_a, sym_b, root, leaf → 4
        // Symmetric: sym_a, sym_b → 2
        assert_eq!(gaps.total_unique, 4);
        assert_eq!(gaps.symmetric_count, 2);
        assert!((gaps.symmetry_ratio() - 0.5).abs() < 0.001,
            "2/4 = 0.5, got {}", gaps.symmetry_ratio());
    }

    // ── Hierarchy Propagation ────────────────────────────────────────

    #[test]
    fn test_propagate_hierarchy_boosts_parent() {
        // "derivative is_a calculus" → calculus should be boosted
        // if derivative is well-known (Adequate+) and calculus is not.
        let mut qa = QaEngine::new();
        qa.store_fact("derivative", "is_a", "calculus", "test");
        qa.store_fact("a_rate_of_change", "be", "derivative", "test"); // definition
        qa.store_fact("derivative", "has", "rules", "test");           // property
        qa.store_fact("derivative", "satisfies", "chain_rule", "test"); // property
        qa.store_fact("derivative", "used_in", "optimization", "test");
        // derivative: 1 def + 2 props + 1 relationship = 4 total, well_defined
        // → Adequate (rank 2), not Strong (no causal rules).
        // calculus appears only as object of is_a → Unknown (rank 0).
        // diff = 2 ≥ 2 → boost calculus to Adequate.

        let mut mc = MetaCognition::new();
        mc.assess(&mut qa);

        let deriv_k = mc.concept_knowledge("derivative");
        assert!(deriv_k.is_some());
        let dk = deriv_k.unwrap();
        assert!(dk.total_facts >= 4, "derivative should have 4+ facts");
        assert_eq!(dk.level, KnowledgeLevel::Adequate,
            "derivative should be Adequate (well-defined with >=4 facts)");

        let calc_knowledge = mc.concept_knowledge("calculus");
        assert!(calc_knowledge.is_some());

        let ck = calc_knowledge.unwrap();
        assert!(ck.hierarchy_boosted,
            "calculus should be hierarchy-boosted from Unknown to Adequate");
        assert_eq!(ck.level, KnowledgeLevel::Adequate,
            "calculus should be Adequate (boosted from Unknown)");
    }

    #[test]
    fn test_propagate_hierarchy_boosts_unknown_parent() {
        // Parent object (only appears as object) should be boosted
        // when a well-known child references it.
        let mut qa = QaEngine::new();
        qa.store_fact("derivative", "is_a", "calculus", "test");
        qa.store_fact("a_rate_of_change", "be", "derivative", "test");
        qa.store_fact("derivative", "has", "rules", "test");
        qa.store_fact("derivative", "satisfies", "chain_rule", "test");
        // derivative → Adequate (4 total, 1 def + 2 props, well_defined)
        // calculus only appears as object → Unknown (rank 0).
        // diff = 2 ≥ 2 → boost calculus to Adequate.

        let mut mc = MetaCognition::new();
        mc.assess(&mut qa);

        let calc_knowledge = mc.concept_knowledge("calculus");
        assert!(calc_knowledge.is_some());
        let ck = calc_knowledge.unwrap();

        // calculus is Weak (1 fact as object). derivative is Adequate.
        // child_rank (2) > parent_rank (1) → boost!
        assert!(ck.hierarchy_boosted,
            "calculus should be hierarchy-boosted from Weak to Adequate");
        assert_eq!(ck.level, KnowledgeLevel::Adequate,
            "calculus should be Adequate (boosted from Weak)");
    }

    #[test]
    fn test_propagate_hierarchy_chain() {
        // A simple 2-level chain: "derivative is_a calculus"
        // and "calculus is_a mathematics".
        // If derivative is Adequate, calculus gets boosted to Adequate.
        // But mathematics (via calculus) only gets a boost if calculus
        // was independently Adequate — since calculus was boosted (not
        // independently known), mathematics stays Weak.
        let mut qa = QaEngine::new();
        qa.store_fact("derivative", "is_a", "calculus", "test");
        qa.store_fact("calculus", "is_a", "mathematics", "test");
        qa.store_fact("a_rate_of_change", "be", "derivative", "test"); // def
        qa.store_fact("derivative", "has", "rules", "test");           // prop
        qa.store_fact("derivative", "satisfies", "chain_rule", "test"); // prop
        // derivative → Adequate (4 total, well_defined)
        // calculus → only appears as object of "is_a" and subject of "is_a"
        //   calculus has 1 fact (calculus is_a mathematics).
        //   Also: derivative is_a calculus → calculus is object.
        //   So calculus total = 1 (relationship as subject) → Weak (rank 1).
        // mathematics → only object of "is_a" → Unknown (rank 0).

        let mut mc = MetaCognition::new();
        mc.assess(&mut qa);

        // calculus has child derivative (Adequate, rank 2).
        // Parent calculus is Weak (rank 1). child_rank > parent_rank → boost!
        // calculus should be promoted to Adequate.
        let calc_k = mc.concept_knowledge("calculus");
        assert!(calc_k.is_some());
        let ck = calc_k.unwrap();
        assert!(ck.hierarchy_boosted,
            "calculus should be boosted (child derivative is Adequate > Weak)");
        assert_eq!(ck.level, KnowledgeLevel::Adequate,
            "calculus should be Adequate (boosted from Weak)");

        // mathematics has child calculus (now Adequate, rank 2 after boost).
        // Parent mathematics is Unknown (rank 0). child_rank > parent_rank → boost!
        let math_k = mc.concept_knowledge("mathematics");
        assert!(math_k.is_some());
        let mk = math_k.unwrap();
        assert!(mk.hierarchy_boosted,
            "mathematics should be boosted via calculus (Adequate > Unknown)");
        assert_eq!(mk.level, KnowledgeLevel::Adequate,
            "mathematics should be Adequate (boosted from Unknown)");
    }

    #[test]
    fn test_propagate_hierarchy_no_boost_if_not_enough_gap() {
        // "weak_child is_a parent" but weak_child is the same level
        // as parent → no boost.
        let mut qa = QaEngine::new();
        qa.store_fact("weak_child", "is_a", "parent", "test");
        qa.store_fact("weak_child", "exist", "barely", "test");
        qa.store_fact("parent", "be", "a_concept", "test");
        // Both weak_child and parent have 1 fact → Weak (rank 1).
        // diff = 1 - 1 = 0 < 2 → no boost.

        let mut mc = MetaCognition::new();
        mc.assess(&mut qa);

        let parent_k = mc.concept_knowledge("parent");
        assert!(parent_k.is_some());
        assert!(!parent_k.unwrap().hierarchy_boosted,
            "parent should NOT be boosted (child is same level)");
        assert_eq!(parent_k.unwrap().level, KnowledgeLevel::Weak,
            "parent should stay Weak");
    }

    #[test]
    fn test_propagate_hierarchy_already_adequate_not_needed() {
        // parent is already Adequate, no boost needed even if child is
        // at a higher level. The propagation only boosts parents that
        // are Unknown or Weak.
        let mut qa = QaEngine::new();
        // Give parent a definition by making it the object of "be".
        qa.store_fact("a_strong_concept", "be", "parent_already_adequate", "test");
        qa.store_fact("parent_already_adequate", "be", "a_concept", "test");
        qa.store_fact("parent_already_adequate", "has", "properties", "test");
        qa.store_fact("parent_already_adequate", "satisfies", "rules", "test");
        // parent → 1 def (a_strong_concept be parent_already_adequate)
        //           + 1 is_defined_as + 2 props = 4 total, well_defined → Adequate.
        qa.store_fact("child", "is_a", "parent_already_adequate", "test");
        qa.store_fact("child", "be", "a_weak_thing", "test");
        // child → Weak (1 fact)

        let mut mc = MetaCognition::new();
        mc.assess(&mut qa);

        let parent_k = mc.concept_knowledge("parent_already_adequate");
        assert!(parent_k.is_some());
        let pk = parent_k.unwrap();
        assert_eq!(pk.level, KnowledgeLevel::Adequate,
            "parent should be Adequate independently");
        assert!(!pk.hierarchy_boosted,
            "already-Adequate parent should not be marked boosted");
    }

    // ── Tick ──────────────────────────────────────────────────────────

    #[test]
    fn test_tick_increments() {
        let mut mc = MetaCognition::new();
        assert_eq!(mc.tick, 0);
        mc.tick();
        assert_eq!(mc.tick, 1);
        mc.tick();
        assert_eq!(mc.tick, 2);
    }
}


