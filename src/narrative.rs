// ─── Narrative Generator: Pure Rule-Based NLG (No ML) ──────────────────
//
// Takes internal system state and produces fluent English sentences.
// No machine learning, no LLMs. Uses:
//   - Morphology lookup tables (plural, tense, articles)
//   - Template frames keyed by cognitive state
//   - Resonator vocabulary for factorizing hypervectors back to words
//
// ## Architecture
//
//   SystemState (aggregate of system's internal state)
//        │
//        ▼
//   NarrativeGenerator::generate()
//        │
//        ├─ Walk frames by priority (most specific first)
//        ├─ Find FIRST frame whose condition() returns true
//        ├─ Fill slots via SlotSource resolvers
//        ├─ Apply inflection rules per SlotDef
//        └─ Return filled template string
//
// ## Test Coverage
//
// 1. test_morphology_plural      — Pluralization rules
// 2. test_morphology_past_tense   — Past tense conjugation
// 3. test_morphology_article      — a/an/the choice
// 4. test_morphology_present      — Present tense conjugation
// 5. test_frame_selection         — Frame priority ordering
// 6. test_slot_filling            — Slot resolver correctness
// 7. test_cognitive_mode_frames   — All 8 modes produce coherent text
// 8. test_crisis_override         — Crisis frame beats mode frame
// 9. test_sleep_narrative         — Sleep narrative output
// 10. test_first_tick             — Boot narrative
//
// ────────────────────────────────────────────────────────────────────────────

use crate::drift::{CognitiveMode, Need, Emotion, Stance, Mood, Archetype};
use crate::resonator::ResonatorVocabulary;
use crate::self_model::{SelfModel, SelfNarrative};
use crate::workspace::{GlobalWorkspace, AttentionReport};
use crate::drives::{IntrinsicMotivation, DriveId};
use crate::sleep::WakeNarrative;
use crate::Hypervector;
use std::collections::HashMap;

// ═══════════════════════════════════════════════════════════════════════════
// SYSTEM STATE — Aggregate of everything we can verbalize
// ═══════════════════════════════════════════════════════════════════════════

/// A snapshot of the system's internal state for narrative generation.
///
/// Collects references to key system modules so the narrative generator
/// can read a coherent picture of "what is happening right now."
#[derive(Clone)]
pub struct SystemState<'a> {
    /// The self-model (mode, deficit, error, stability, crisis).
    pub self_model: &'a SelfModel,
    /// The global workspace attention report.
    pub attention: &'a AttentionReport,
    /// The workspace itself (for broadcast / idle state).
    pub workspace: &'a GlobalWorkspace,
    /// The intrinsic drives.
    pub drives: &'a IntrinsicMotivation,
    /// The dominant archetype (from shadow system), if known.
    pub dominant_archetype: Option<Archetype>,
    /// The current emotion, if known.
    pub emotion: Option<Emotion>,
    /// The current stance, if known.
    pub stance: Option<Stance>,
    /// The current mood, if known.
    pub mood: Option<Mood>,
    /// A narrative from a recent sleep cycle, if any.
    pub sleep_narrative: Option<&'a WakeNarrative>,
    /// Number of transitions in the last sleep replay.
    pub sleep_transitions: usize,
    /// Number of L3 concepts formed in the last sleep cycle.
    pub sleep_l3_formed: usize,
    /// Whether this is the very first tick (boot).
    pub is_first_tick: bool,
    /// The current tick number.
    pub tick: u64,
    /// Whether the system is currently asleep.
    pub is_sleeping: bool,
    /// Reason for the last sleep trigger.
    pub sleep_reason: Option<String>,
}

// ═══════════════════════════════════════════════════════════════════════════
// MORPHOLOGY — Pure lookup tables, no ML
// ═══════════════════════════════════════════════════════════════════════════

/// Return the plural form of a noun.
pub fn pluralize(noun: &str) -> String {
    // Irregular plurals (most common in financial/technical domains)
    match noun.to_lowercase().as_str() {
        "crisis"    => "crises".to_string(),
        "analysis"  => "analyses".to_string(),
        "thesis"    => "theses".to_string(),
        "hypothesis" => "hypotheses".to_string(),
        "index"     => "indices".to_string(),
        "vertex"    => "vertices".to_string(),
        "matrix"    => "matrices".to_string(),
        "appendix"  => "appendices".to_string(),
        "datum"     => "data".to_string(),
        "criterion" => "criteria".to_string(),
        "phenomenon" => "phenomena".to_string(),
        "foot"      => "feet".to_string(),
        "tooth"     => "teeth".to_string(),
        "mouse"     => "mice".to_string(),
        "goose"     => "geese".to_string(),
        "child"     => "children".to_string(),
        "man"       => "men".to_string(),
        "woman"     => "women".to_string(),
        "person"    => "people".to_string(),
        "ox"        => "oxen".to_string(),
        "leaf"      => "leaves".to_string(),
        "knife"     => "knives".to_string(),
        "life"      => "lives".to_string(),
        "shelf"     => "shelves".to_string(),
        "wolf"      => "wolves".to_string(),
        "self"      => "selves".to_string(),
        // -f → -ves
        s if s.ends_with("fe") => format!("{}ves", &s[..s.len()-2]),
        s if s.ends_with("f") && !s.ends_with("ff") => format!("{}ves", &s[..s.len()-1]),
        // -is → -es (already handled above for known ones, catch-all)
        s if s.ends_with("is") => format!("{}es", &s[..s.len()-2]),
        // -on → -a
        s if s.ends_with("on") => format!("{}a", &s[..s.len()-2]),
        // -ex → -ices
        s if s.ends_with("ex") => format!("{}ices", &s[..s.len()-2]),
        // -us → -i
        s if s.ends_with("us") => format!("{}i", &s[..s.len()-2]),
        // -um → -a
        s if s.ends_with("um") => format!("{}a", &s[..s.len()-2]),
        // -ix → -ices
        s if s.ends_with("ix") => format!("{}ices", &s[..s.len()-2]),
        // General rules
        s if s.ends_with("sh") || s.ends_with("ch") || s.ends_with("ss")
            || s.ends_with("x") || s.ends_with("z") => format!("{}es", s),
        s if s.ends_with("y") && s.len() > 2
            && !matches!(s.chars().nth(s.len()-2), Some('a'|'e'|'i'|'o'|'u'))
            => format!("{}ies", &s[..s.len()-1]),
        s => format!("{}s", s),
    }
}

/// Return the past tense form of a verb.
pub fn past_tense(verb: &str) -> String {
    let lower = verb.to_lowercase();
    match lower.as_str() {
        // Strong verbs (irregular past)
        "be" | "am" | "is" | "are" => "was".to_string(),
        "begin"   => "began".to_string(),
        "break"   => "broke".to_string(),
        "bring"   => "brought".to_string(),
        "build"   => "built".to_string(),
        "buy"     => "bought".to_string(),
        "catch"   => "caught".to_string(),
        "choose"  => "chose".to_string(),
        "come"    => "came".to_string(),
        "cut"     => "cut".to_string(),
        "deal"    => "dealt".to_string(),
        "do"      => "did".to_string(),
        "draw"    => "drew".to_string(),
        "drink"   => "drank".to_string(),
        "drive"   => "drove".to_string(),
        "eat"     => "ate".to_string(),
        "fall"    => "fell".to_string(),
        "feed"    => "fed".to_string(),
        "feel"    => "felt".to_string(),
        "fight"   => "fought".to_string(),
        "find"    => "found".to_string(),
        "fly"     => "flew".to_string(),
        "forget"  => "forgot".to_string(),
        "get"     => "got".to_string(),
        "give"    => "gave".to_string(),
        "go"      => "went".to_string(),
        "grow"    => "grew".to_string(),
        "have"    => "had".to_string(),
        "hear"    => "heard".to_string(),
        "hide"    => "hid".to_string(),
        "hit"     => "hit".to_string(),
        "hold"    => "held".to_string(),
        "keep"    => "kept".to_string(),
        "know"    => "knew".to_string(),
        "lead"    => "led".to_string(),
        "leave"   => "left".to_string(),
        "lend"    => "lent".to_string(),
        "let"     => "let".to_string(),
        "lie"     => "lay".to_string(),
        "lose"    => "lost".to_string(),
        "make"    => "made".to_string(),
        "mean"    => "meant".to_string(),
        "meet"    => "met".to_string(),
        "pay"     => "paid".to_string(),
        "put"     => "put".to_string(),
        "quit"    => "quit".to_string(),
        "read"    => "read".to_string(),
        "ride"    => "rode".to_string(),
        "ring"    => "rang".to_string(),
        "rise"    => "rose".to_string(),
        "run"     => "ran".to_string(),
        "say"     => "said".to_string(),
        "see"     => "saw".to_string(),
        "seek"    => "sought".to_string(),
        "sell"    => "sold".to_string(),
        "send"    => "sent".to_string(),
        "set"     => "set".to_string(),
        "shake"   => "shook".to_string(),
        "shine"   => "shone".to_string(),
        "shoot"   => "shot".to_string(),
        "show"    => "showed".to_string(),
        "shut"    => "shut".to_string(),
        "sing"    => "sang".to_string(),
        "sink"    => "sank".to_string(),
        "sit"     => "sat".to_string(),
        "sleep"   => "slept".to_string(),
        "speak"   => "spoke".to_string(),
        "spend"   => "spent".to_string(),
        "stand"   => "stood".to_string(),
        "steal"   => "stole".to_string(),
        "stick"   => "stuck".to_string(),
        "strike"  => "struck".to_string(),
        "swim"    => "swam".to_string(),
        "take"    => "took".to_string(),
        "teach"   => "taught".to_string(),
        "tell"    => "told".to_string(),
        "think"   => "thought".to_string(),
        "throw"   => "threw".to_string(),
        "understand" => "understood".to_string(),
        "wake"    => "woke".to_string(),
        "wear"    => "wore".to_string(),
        "win"     => "won".to_string(),
        "write"   => "wrote".to_string(),
        "bind"    => "bound".to_string(),
        "encode"  => "encoded".to_string(),
        "decode"  => "decoded".to_string(),
        "factorize" => "factorized".to_string(),
        "bundle"  => "bundled".to_string(),
        "project" => "projected".to_string(),
        "register" => "registered".to_string(),
        "observe" => "observed".to_string(),
        "detect"  => "detected".to_string(),
        "consolidate" => "consolidated".to_string(),
        "prune"   => "pruned".to_string(),
        "crawl"   => "crawled".to_string(),
        "trigger" => "triggered".to_string(),
        "process" => "processed".to_string(),
        "form"    => "formed".to_string(),
        "learn"   => "learned".to_string(),
        "adjust"  => "adjusted".to_string(),
        "shift"   => "shifted".to_string(),
        // Regular: ends in e → +d
        s if s.ends_with("e") => format!("{}d", s),
        // Regular: ends in consonant+y → +ied
        s if s.ends_with("y") && s.len() > 2
            && !matches!(s.chars().nth(s.len()-2), Some('a'|'e'|'i'|'o'|'u'))
            => format!("{}ied", &s[..s.len()-1]),
        // Regular: ends in consonant-vowel-consonant and short syllable → double +ed
        // This is a simplification; for the system's vocabulary it's rare
        // Regular: +ed
        s => format!("{}ed", s),
    }
}

/// Return the present tense form (3rd person singular) of a verb.
///
/// For 3rd person singular subjects (he/she/it): "raise" → "raises"
/// For other subjects (I/you/we/they): use the base form directly.
pub fn present_tense(verb: &str, third_person_singular: bool) -> String {
    if !third_person_singular {
        return verb.to_string();
    }
    let lower = verb.to_lowercase();
    match lower.as_str() {
        "be"  => "is".to_string(),
        "have" => "has".to_string(),
        "do"   => "does".to_string(),
        "go"   => "goes".to_string(),
        s if s.ends_with("sh") || s.ends_with("ch") || s.ends_with("ss")
            || s.ends_with("x") || s.ends_with("z") || s.ends_with("o")
            => format!("{}es", s),
        s if s.ends_with("y") && s.len() > 2
            && !matches!(s.chars().nth(s.len()-2), Some('a'|'e'|'i'|'o'|'u'))
            => format!("{}ies", &s[..s.len()-1]),
        s => format!("{}s", s),
    }
}

/// Choose the appropriate article for a noun phrase.
///
/// * `indefinite=true` → "a" or "an"
/// * `indefinite=false` → "the"
pub fn choose_article(noun: &str, indefinite: bool) -> &'static str {
    if !indefinite {
        return "the";
    }
    // "an" before vowel sounds (simplified — just checks vowel letter)
    // For the system's vocabulary, this heuristic is sufficient.
    let first = noun.chars().next().map(|c| c.to_ascii_lowercase());
    match first {
        Some('a' | 'e' | 'i' | 'o' | 'u') => "an",
        _ => "a",
    }
}

/// Capitalize the first letter of a string.
pub fn capitalize_sentence(s: &str) -> String {
    let mut chars = s.chars();
    match chars.next() {
        Some(c) if c.is_ascii_alphabetic() => {
            let upper = c.to_ascii_uppercase();
            let mut result = String::with_capacity(s.len());
            result.push(upper);
            result.push_str(chars.as_str());
            result
        }
        Some(c) => {
            // First char is not a letter; still capitalize the first letter found
            let mut result = String::with_capacity(s.len());
            result.push(c);
            let rest = chars.as_str();
            // Find first letter and capitalize it
            let mut found = false;
            for ch in rest.chars() {
                if !found && ch.is_ascii_alphabetic() {
                    result.push(ch.to_ascii_uppercase());
                    found = true;
                } else {
                    result.push(ch);
                }
            }
            result
        }
        None => s.to_string(),
    }
}

/// Verbalize a scalar in [0, 1] into a human-readable intensity phrase.
pub fn verbalize_intensity(value: f64) -> &'static str {
    if value < 0.05 {
        "minimal"
    } else if value < 0.15 {
        "very low"
    } else if value < 0.30 {
        "low"
    } else if value < 0.45 {
        "moderate"
    } else if value < 0.55 {
        "moderate"
    } else if value < 0.70 {
        "elevated"
    } else if value < 0.85 {
        "high"
    } else if value < 0.95 {
        "very high"
    } else {
        "critical"
    }
}

/// Verbalize a stability NHD value.
pub fn verbalize_stability(stability: f64) -> &'static str {
    if stability < 0.05 {
        "stable"
    } else if stability < 0.10 {
        "shifting gradually"
    } else if stability < 0.20 {
        "changing significantly"
    } else {
        "in shock"
    }
}

/// Label for a homeostatic need, in lowercase human form.
pub fn need_label_lower(need: &Need) -> &'static str {
    match need {
        Need::Energy      => "energy",
        Need::Coherence   => "coherence",
        Need::Integration => "integration",
        Need::Connection  => "connection",
        Need::Growth      => "growth",
        Need::Autonomy    => "autonomy",
        Need::Integrity   => "integrity",
    }
}

/// Human-readable label for a drive.
pub fn drive_label(drive: &DriveId) -> &'static str {
    match drive {
        DriveId::PredictiveMastery => "predictive mastery",
        DriveId::Coherence         => "coherence",
        DriveId::Abstraction       => "abstraction",
        DriveId::SelfPreservation  => "self-preservation",
    }
}

/// Human-readable label for an emotion.
pub fn emotion_label(emotion: &Emotion) -> &'static str {
    match emotion {
        Emotion::Joy      => "joy",
        Emotion::Sadness  => "sadness",
        Emotion::Anger    => "anger",
        Emotion::Fear     => "fear",
        Emotion::Surprise => "surprise",
        Emotion::Disgust  => "disgust",
        Emotion::Neutral  => "neutral",
    }
}

/// Human-readable label for a stance.
pub fn stance_label(stance: &Stance) -> &'static str {
    match stance {
        Stance::Open    => "open",
        Stance::Guarded => "guarded",
        Stance::Curious => "curious",
        Stance::Distant => "distant",
    }
}

/// Human-readable label for a mood.
pub fn mood_label(mood: &Mood) -> &'static str {
    match mood {
        Mood::Warm       => "warm",
        Mood::Playful    => "playful",
        Mood::Somber     => "somber",
        Mood::Alert      => "alert",
        Mood::Defensive  => "defensive",
        Mood::Withdrawn  => "withdrawn",
        Mood::Curious    => "curious",
        Mood::Analytical => "analytical",
        Mood::Neutral    => "neutral",
    }
}

/// Human-readable label for an archetype.
pub fn archetype_label(arch: &Archetype) -> &'static str {
    match arch {
        Archetype::Hero      => "hero",
        Archetype::Shadow    => "shadow",
        Archetype::Sage      => "sage",
        Archetype::Trickster => "trickster",
        Archetype::Caregiver => "caregiver",
        Archetype::Orphan    => "orphan",
    }
}

/// Antonym dictionary for verb-based contradiction detection.
///
/// When the QA engine detects two facts with the same subject and object
/// but opposite verbs (e.g., "raise" vs "lower"), it flags a contradiction.
/// This table defines which verb pairs are opposites.
///
/// Extend this table when new antonymic verb pairs are introduced.
pub fn is_antonym(a: &str, b: &str) -> bool {
    let a = a.trim().to_lowercase();
    let b = b.trim().to_lowercase();
    if a == b { return false; }
    // Normalize both to lemma first
    let a_lemma = crate::nlp::verb_lemma(&a);
    let b_lemma = crate::nlp::verb_lemma(&b);
    if a_lemma == b_lemma { return false; }

    // Antonynm pairs (both directions)
    let pairs: &[(&str, &str)] = &[
        ("raise", "lower"),
        ("raise", "cut"),
        ("raise", "reduce"),
        ("raise", "decrease"),
        ("increase", "decrease"),
        ("increase", "reduce"),
        ("increase", "cut"),
        ("rise", "fall"),
        ("rise", "decline"),
        ("rise", "drop"),
        ("grow", "shrink"),
        ("grow", "decline"),
        ("expand", "contract"),
        ("tighten", "loosen"),
        ("tighten", "ease"),
        ("buy", "sell"),
        ("lend", "borrow"),
        ("push", "pull"),
        ("start", "stop"),
        ("start", "halt"),
        ("begin", "end"),
        ("open", "close"),
        ("enter", "exit"),
        ("add", "remove"),
        ("add", "subtract"),
        ("gain", "lose"),
        ("win", "lose"),
        ("create", "destroy"),
        ("create", "delete"),
        ("build", "destroy"),
        ("build", "demolish"),
        ("encode", "decode"),
        ("bind", "unbind"),
        ("lock", "unlock"),
        ("load", "unload"),
        ("mount", "unmount"),
        ("attach", "detach"),
        ("connect", "disconnect"),
        ("include", "exclude"),
        ("import", "export"),
        ("approve", "reject"),
        ("approve", "deny"),
        ("accept", "refuse"),
        ("accept", "reject"),
        ("allow", "block"),
        ("allow", "forbid"),
        ("enable", "disable"),
        ("enable", "suppress"),
        ("activate", "deactivate"),
        ("arm", "disarm"),
        ("engage", "disengage"),
        ("invest", "divest"),
        ("inflate", "deflate"),
        ("accelerate", "decelerate"),
        ("advance", "retreat"),
        ("arrive", "depart"),
        ("ascend", "descend"),
        ("attack", "defend"),
        ("remember", "forget"),
        ("show", "hide"),
        ("reveal", "conceal"),
        ("praise", "criticize"),
        ("support", "oppose"),
        ("agree", "disagree"),
        ("succeed", "fail"),
        ("pass", "fail"),
        ("satisfy", "dissatisfy"),
    ];

    pairs.iter().any(|(x, y)| {
        (crate::nlp::verb_lemma(x) == a_lemma && crate::nlp::verb_lemma(y) == b_lemma)
            || (crate::nlp::verb_lemma(x) == b_lemma && crate::nlp::verb_lemma(y) == a_lemma)
    })
}

/// Check if two objects are opposites (contradictory).
/// Uses a smaller table for noun/object opposites.
pub fn is_object_antonym(a: &str, b: &str) -> bool {
    let a_lower = a.trim().to_lowercase();
    let b_lower = b.trim().to_lowercase();
    if a_lower == b_lower { return false; }
    let pairs: &[(&str, &str)] = &[
        ("rates", "rates"), // same object but verb changes — handled by verb antonym
        ("tight", "loose"),
        ("risk_on", "risk_off"),
        ("bullish", "bearish"),
        ("long", "short"),
        ("overweight", "underweight"),
        ("expansion", "recession"),
        ("boom", "bust"),
        ("inflation", "deflation"),
        ("surplus", "deficit"),
        ("assets", "liabilities"),
        ("income", "expenses"),
        ("revenue", "costs"),
        ("profit", "loss"),
        ("gain", "loss"),
        ("credit", "debit"),
    ];
    pairs.iter().any(|(x, y)| {
        (x == &a_lower && y == &b_lower) || (x == &b_lower && y == &a_lower)
    })
}

// ═══════════════════════════════════════════════════════════════════════════
// PHASE 2: DEPENDENCY LINEARIZATION
// ═══════════════════════════════════════════════════════════════════════════
//
// Takes a structured set of dependency relations and linearizes them into
// a grammatical English sentence using deterministic ordering rules.
//
// ## Dependency Relation Types (Universal Dependencies subset)
//
//   nsubj  — nominal subject    ("The Fed")
//   verb   — root verb          ("raised")
//   dobj   — direct object      ("rates")
//   iobj   — indirect object    ("the market")
//   obl    — oblique nominal    ("by 25bp")
//   amod   — adjectival modifier ("aggressive")
//   advmod — adverbial modifier ("sharply")
//   det    — determiner         ("the", "a")
//   aux    — auxiliary          ("has", "will")
//   mark   — subordinator       ("because", "while")
//   conj   — conjunct           ("and")
//   prep   — preposition        ("in", "on", "by")
//   temp   — temporal modifier  ("today", "in March")
//   loc    — locative modifier  ("in the market")
//   neg    — negation           ("not")

/// A single dependency relation: (relation_type, value).
#[derive(Clone, Debug)]
pub struct DepRel {
    /// The dependency relation type (e.g., "nsubj", "verb", "dobj").
    pub rel: &'static str,
    /// The surface value (e.g., "the_fed", "raise", "rate").
    pub value: String,
    /// Inflection rules for this value.
    pub inflection: InflectionRules,
}

impl DepRel {
    pub fn new(rel: &'static str, value: &str) -> Self {
        DepRel { rel, value: value.to_string(), inflection: InflectionRules::EMPTY }
    }

    pub fn with_inflection(rel: &'static str, value: &str, inflection: InflectionRules) -> Self {
        DepRel { rel, value: value.to_string(), inflection }
    }
}

/// A dependency graph: a set of relation-value pairs.
///
/// The linearizer orders these by a deterministic rule table:
///   1. nsubj (subject)
///   2. aux (auxiliary verb)
///   3. neg (negation)
///   4. advmod (adverb)
///   5. verb (root verb)
///   6. dobj (direct object)
///   7. iobj (indirect object)
///   8. obl (oblique)
///   9. prep + prep_obj
///   10. temp (temporal)
///   11. loc (locative)
///
/// This follows English SVO word order without needing a parser.
#[derive(Clone, Debug)]
pub struct DepGraph {
    relations: Vec<DepRel>,
}

impl DepGraph {
    pub fn new() -> Self {
        DepGraph { relations: Vec::new() }
    }

    /// Add a dependency relation. Duplicates of the same type overwrite.
    pub fn add(&mut self, rel: DepRel) {
        // If this relation type already exists, overwrite it
        if let Some(existing) = self.relations.iter_mut().find(|r| r.rel == rel.rel) {
            *existing = rel;
        } else {
            self.relations.push(rel);
        }
    }

    /// Get the value for a relation type.
    pub fn get(&self, rel: &str) -> Option<&str> {
        self.relations.iter().find(|r| r.rel == rel).map(|r| r.value.as_str())
    }

    /// Check if a relation type exists.
    pub fn has(&self, rel: &str) -> bool {
        self.relations.iter().any(|r| r.rel == rel)
    }

    /// Linearize the dependency graph into an English sentence.
    ///
    /// Uses a fixed ordering table (Universal Dependencies → linear position)
    /// with English-specific rules:
    ///   - Subject before verb
    ///   - Object after verb
    ///   - Oblique arguments after object
    ///   - Temporal/locative modifiers at the end
    ///   - Negation before verb
    ///   - Adverbs before verb (for manner)
    pub fn linearize(&self) -> String {
        // Define the English linearization order
        let order: &[&str] = &[
            "nsubj",    // 1. Subject
            "aux",      // 2. Auxiliary verb
            "neg",      // 3. Negation
            "advmod",   // 4. Adverb
            "verb",     // 5. Root verb
            "dobj",     // 6. Direct object
            "iobj",     // 7. Indirect object
            "obl",      // 8. Oblique
            "conj",     // 9. Conjunction
            "temp",     // 10. Temporal
            "loc",      // 11. Location
        ];

        let mut words: Vec<String> = Vec::new();

        for rel_type in order {
            if let Some(rel) = self.relations.iter().find(|r| r.rel == *rel_type) {
                let inflected = rel.inflection.apply(&rel.value);
                words.push(inflected);
            }
        }

        // Add any remaining relations not in the standard order
        for rel in &self.relations {
            if !order.contains(&rel.rel) {
                let inflected = rel.inflection.apply(&rel.value);
                words.push(inflected);
            }
        }

        if words.is_empty() {
            return String::new();
        }

        // Capitalize first word
        words[0] = capitalize_sentence(&words[0]);

        // Join with spaces and add period
        let mut sentence = words.join(" ");
        if !sentence.ends_with('.') && !sentence.ends_with('!') && !sentence.ends_with('?') {
            sentence.push('.');
        }

        sentence
    }
}

impl Default for DepGraph {
    fn default() -> Self { Self::new() }
}

/// Build a dependency graph from the system state describing what the
/// system is currently doing.
pub fn build_action_dep_graph(state: &SystemState) -> DepGraph {
    let mut deps = DepGraph::new();

    // Subject is always "I"
    deps.add(DepRel::new("nsubj", "I"));

    // The verb depends on the cognitive mode
    let verb = match state.self_model.mode {
        CognitiveMode::Quiet => "am",
        CognitiveMode::Companion => "remember",
        CognitiveMode::Regulated => "regulate",
        CognitiveMode::Explorer => "explore",
        CognitiveMode::Task => "work",
        CognitiveMode::Resonant => "connect",
        CognitiveMode::Frontier => "push",
        CognitiveMode::FullCouncil => "engage",
    };
    deps.add(DepRel::new("verb", verb));

    // Add attention focus as direct object
    if state.attention.winner_id.is_some() && state.attention.winner_label != "none" {
        let label = state.attention.winner_label.to_lowercase();
        deps.add(DepRel::with_inflection("dobj", &label, InflectionRules::def()));
    }

    // Add temporal: tick number
    deps.add(DepRel::new("temp", &format!("at tick {}", state.tick)));

    // Add emotion as adverbial modifier
    if let Some(emotion) = &state.emotion {
        let emo_str = emotion_label(emotion);
        if emo_str != "neutral" {
            deps.add(DepRel::new("advmod", &format!("with {}", emo_str)));
        }
    }

    deps
}

/// Build a dependency graph describing the system's current state.
pub fn build_state_dep_graph(state: &SystemState) -> DepGraph {
    let mut deps = DepGraph::new();

    deps.add(DepRel::new("nsubj", "my"));

    // Mode as attribute
    let mode_str = state.self_model.mode.label().to_lowercase();
    deps.add(DepRel::new("verb", "be"));
    deps.add(DepRel::with_inflection("amod", &mode_str, InflectionRules::indef()));

    // Deficit level
    let deficit_bucket = resolve_slot(&SlotSource::DeficitBucket, state);
    deps.add(DepRel::new("conj", "with"));
    deps.add(DepRel::new("obl", &format!("{} deficit", deficit_bucket)));

    // Error level
    let error_bucket = resolve_slot(&SlotSource::ErrorBucket, state);
    deps.add(DepRel::new("conj", "and"));
    deps.add(DepRel::new("obl", &format!("{} prediction error", error_bucket)));

    deps
}

// ═══════════════════════════════════════════════════════════════════════════
// PHASE 3: VSA N-GRAM CHAIN
// ═══════════════════════════════════════════════════════════════════════════
//
// Tracks transitions between cognitive states using a VSA transition matrix.
// Each observed transition A→B is stored as:
//
//   T(A, B) = ρ₁(HV_A) ⊕ ρ₂(HV_B)
//
// where ρ₁, ρ₂ are distinct rotation amounts and ⊕ is XOR binding.
// All transitions are bundled into a single transition matrix.
//
// To predict the next state from current state A:
//
//   HV_B_est = TM ⊕ ρ₁(HV_A)   → unbind A, recovering B
//   cleanup(HV_B_est)           → nearest known state label
//
// This is a pure VSA associative memory: no statistics, no gradient descent.

/// Rotation amount for encoding the "from" state in a transition.
const TRANSITION_FROM_RHO: usize = 7;
/// Rotation amount for encoding the "to" state in a transition.
const TRANSITION_TO_RHO: usize = 13;

/// A VSA-based n-gram transition tracker.
///
/// Records observed transitions between named states and can predict
/// the most likely next state given the current one.
///
/// ## Usage
///
/// ```ignore
/// let mut chain = NgramChain::new();
/// chain.observe("explorer", "task");
/// chain.observe("task", "regulated");
/// let next = chain.predict("task");
/// assert_eq!(next, Some("regulated"));
/// ```
pub struct NgramChain {
    /// The bundled transition matrix: bundle of all T(A, B) vectors.
    transition_matrix: Option<Hypervector>,
    /// Count of transitions observed (for diagnostics).
    transition_count: usize,
    /// Last `order` states seen (for n>2 gram prediction).
    recent_states: Vec<String>,
    /// How many past states to consider for trigram+ prediction.
    order: usize,
    /// All known state labels, for cleanup/prediction.
    known_states: Vec<String>,
    /// State label → hypervector cache.
    state_vectors: HashMap<String, Hypervector>,
}

impl NgramChain {
    /// Create a new n-gram chain with the given order.
    ///
    /// `order=2` tracks bigrams (current → next).
    /// `order=3` tracks trigrams (prev, current → next).
    pub fn new(order: usize) -> Self {
        NgramChain {
            transition_matrix: None,
            transition_count: 0,
            recent_states: Vec::new(),
            order: order.max(2),
            known_states: Vec::new(),
            state_vectors: HashMap::new(),
        }
    }

    /// Create a bigram chain (default).
    pub fn bigram() -> Self {
        NgramChain::new(2)
    }

    /// Register a state label so it can be predicted later.
    pub fn register_state(&mut self, label: &str) {
        if !self.state_vectors.contains_key(label) {
            let hv = Hypervector::encode_text_ngram(label, 5); // 5-gram for better separation
            self.state_vectors.insert(label.to_string(), hv);
            self.known_states.push(label.to_string());
        }
    }

    /// Register multiple states at once.
    pub fn register_states(&mut self, labels: &[&str]) {
        for label in labels {
            self.register_state(label);
        }
    }

    /// Get the hypervector for a state label.
    fn hv_for(&self, label: &str) -> Option<&Hypervector> {
        self.state_vectors.get(label)
    }

    /// Observe a transition from `from` to `to`.
    ///
    /// Updates the VSA transition matrix by bundling the new
    /// transition vector with the existing matrix.
    pub fn observe(&mut self, from: &str, to: &str) {
        // Ensure both states are registered
        self.register_state(from);
        self.register_state(to);

        let hv_from = self.hv_for(from).unwrap();
        let hv_to = self.hv_for(to).unwrap();

        // Encode transition: T = ρ₁(HV_from) ⊕ ρ₂(HV_to)
        let from_rot = hv_from.rotate_left(TRANSITION_FROM_RHO);
        let to_rot = hv_to.rotate_left(TRANSITION_TO_RHO);
        let transition_hv = from_rot.bitwise_xor(&to_rot);

        // Bundle into transition matrix
        self.transition_matrix = Some(match &self.transition_matrix {
            Some(existing) => {
                let refs = [existing, &transition_hv];
                Hypervector::bundle(&refs)
            }
            None => transition_hv,
        });

        self.transition_count += 1;
        self.recent_states.push(from.to_string());
        if self.recent_states.len() > self.order {
            self.recent_states.remove(0);
        }
    }

    /// Predict the most likely next state given a current state.
    ///
    /// Uses VSA unbinding: `prediction = TM ⊕ ρ₁(HV_current)`
    /// then reverse-rotate by ρ₂ to recover the unbound state,
    /// then cleanup against all known states.
    ///
    /// Returns `None` if no transitions have been observed.
    pub fn predict(&self, current: &str) -> Option<String> {
        let tm = self.transition_matrix.as_ref()?;
        let hv_current = self.hv_for(current)?;

        // Unbind: estimate HV_next_rotated = TM ⊕ ρ₁(HV_current)
        let from_rot = hv_current.rotate_left(TRANSITION_FROM_RHO);
        let next_rotated = tm.bitwise_xor(&from_rot);

        // Reverse the ρ₂ rotation to recover the unbound state vector.
        // rotate_left by (D - shift) is equivalent to rotate_right by shift.
        let next_estimate = next_rotated.rotate_left(crate::HD_DIMENSION - TRANSITION_TO_RHO);

        // Cleanup: find nearest known state
        let mut best_label: Option<String> = None;
        let mut best_sim = 0.0_f64;

        for (label, hv) in &self.state_vectors {
            let sim = 1.0 - next_estimate.normalized_hamming_distance(hv);
            if sim > best_sim {
                best_sim = sim;
                best_label = Some(label.clone());
            }
        }

        // Only return if similarity is meaningful (avoids noise predictions)
        if best_sim > 0.48 {
            best_label
        } else {
            None
        }
    }

    /// Generate a sequence of predicted states starting from a seed.
    pub fn generate_sequence(&self, seed: &str, length: usize) -> Vec<String> {
        let mut sequence = Vec::new();
        let mut current = seed.to_string();

        for _ in 0..length {
            match self.predict(&current) {
                Some(next) => {
                    sequence.push(next.clone());
                    current = next;
                }
                None => break,
            }
        }

        sequence
    }

    /// Get a narrative description of what the system is likely to do next.
    pub fn prediction_narrative(&self, current: &str) -> String {
        match self.predict(current) {
            Some(next) => {
                // Build a dependency graph for the prediction
                let mut deps = DepGraph::new();
                deps.add(DepRel::new("nsubj", "I"));
                deps.add(DepRel::new("advmod", "likely"));
                deps.add(DepRel::new("verb", "transition"));
                deps.add(DepRel::new("obl", &format!("to {}", next)));
                deps.linearize()
            }
            None => String::new(),
        }
    }

    /// Number of transitions observed.
    pub fn transition_count(&self) -> usize {
        self.transition_count
    }

    /// Number of known states.
    pub fn state_count(&self) -> usize {
        self.known_states.len()
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// SLOT SYSTEM — Template placeholders and how to fill them
// ═══════════════════════════════════════════════════════════════════════════

/// How a template slot obtains its value.
#[derive(Clone, Debug)]
pub enum SlotSource {
    /// A fixed literal string.
    Literal(&'static str),
    /// The cognitive mode label (lowercase).
    CognitiveMode,
    /// A homeostatic need label.
    NeedLabel(Need),
    /// The overall deficit bucket ("low", "elevated", "critical").
    DeficitBucket,
    /// The prediction error bucket.
    ErrorBucket,
    /// The identity stability descriptor.
    StabilityDescriptor,
    /// The highest-deficit need label.
    HighestNeed,
    /// The lowest-deficit need label.
    LowestNeed,
    /// The starved drive label.
    StarvedDrive,
    /// The strongest drive label.
    StrongestDrive,
    /// The number of L2 concepts (as words: "few", "many", etc.).
    L2ConceptCount,
    /// The total number of frames ingested.
    FramesCount,
    /// Rules total.
    RulesTotal,
    /// Rules trusted.
    RulesTrusted,
    /// The dominant archetype label.
    DominantArchetype,
    /// Emotion label.
    Emotion,
    /// Stance label.
    Stance,
    /// Mood label.
    Mood,
    /// Attention winner module label.
    AttentionWinner,
    /// Attention similarity as a percentage phrase.
    AttentionSimilarity,
    /// Tick counter.
    TickCount,
    /// Number of sleep transitions.
    SleepTransitions,
    /// Number of L3 concepts formed during sleep.
    SleepL3Formed,
    /// Sleep trigger reason.
    SleepReason,
    /// Whether workspace is idle.
    WorkspaceIdle,
    /// The current mode as a first-person statement (pre-written).
    ModeStatement,
}

/// Inflection rules to apply to a slot value before insertion.
#[derive(Clone, Debug, Default)]
pub struct InflectionRules {
    /// Apply pluralization to the value.
    pub pluralize: bool,
    /// Apply past tense to the value.
    pub past_tense: bool,
    /// Apply present tense (3rd person singular) to the value.
    pub present_3sg: bool,
    /// Wrap with a determiner: Some(true) = "the", Some(false) = "a/an".
    pub determiner: Option<bool>, // true = definite (the), false = indefinite (a/an)
    /// Capitalize the first letter.
    pub capitalize: bool,
    /// Prefix string (e.g., "not ", "very ").
    pub prefix: Option<&'static str>,
    /// Suffix string (e.g., "!")
    pub suffix: Option<&'static str>,
}

impl InflectionRules {
    pub fn apply(&self, value: &str) -> String {
        let mut s = value.to_string();

        // Pluralize
        if self.pluralize {
            s = pluralize(&s);
        }

        // Past tense
        if self.past_tense {
            s = past_tense(&s);
        }

        // Present 3sg
        if self.present_3sg {
            s = present_tense(&s, true);
        }

        // Determiner
        if let Some(definite) = self.determiner {
            let article = choose_article(&s, !definite);
            s = format!("{} {}", article, s);
        }

        // Prefix
        if let Some(pre) = self.prefix {
            s = format!("{}{}", pre, s);
        }

        // Suffix
        if let Some(suf) = self.suffix {
            s = format!("{}{}", s, suf);
        }

        // Capitalize
        if self.capitalize {
            s = capitalize_sentence(&s);
        }

        s
    }
}

/// Definition of a single template slot.
#[derive(Clone, Debug)]
pub struct SlotDef {
    /// The placeholder name in the template string.
    pub key: &'static str,
    /// How to obtain the value.
    pub source: SlotSource,
    /// Inflection rules to apply.
    pub inflection: InflectionRules,
}

impl SlotDef {
    pub const fn new(key: &'static str, source: SlotSource) -> Self {
        SlotDef { key, source, inflection: InflectionRules::EMPTY }
    }

    pub const fn with_inflection(key: &'static str, source: SlotSource, inflection: InflectionRules) -> Self {
        SlotDef { key, source, inflection }
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// NARRATIVE FRAME — A template with conditions and slots
// ═══════════════════════════════════════════════════════════════════════════

/// A narrative frame: a template string with a condition and slot definitions.
///
/// Frames are tried in priority order. The FIRST frame whose `condition`
/// returns true is selected. This ensures specificity wins: crisis frames
/// fire before mode frames, sleep frames fire before routine frames, etc.
pub struct NarrativeFrame {
    /// Description of when this frame fires (for debugging).
    pub description: &'static str,
    /// Priority: higher = checked first.
    pub priority: u8,
    /// Condition: returns true if this frame should fire.
    pub condition: fn(&SystemState) -> bool,
    /// Template string with {slot_key} placeholders.
    pub template: &'static str,
    /// Slot definitions for each placeholder.
    pub slots: Vec<SlotDef>,
}

impl NarrativeFrame {
    /// Fill the template with resolved slot values for the given system state.
    fn fill(&self, state: &SystemState) -> String {
        let mut result = self.template.to_string();

        for slot in &self.slots {
            let value = resolve_slot(&slot.source, state);
            let inflected = slot.inflection.apply(&value);
            result = result.replace(&format!("{{{}}}", slot.key), &inflected);
        }

        capitalize_sentence(&result)
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// SLOT RESOLVER — Maps SlotSource → concrete string value
// ═══════════════════════════════════════════════════════════════════════════

fn resolve_slot(source: &SlotSource, state: &SystemState) -> String {
    match source {
        SlotSource::Literal(s) => s.to_string(),

        SlotSource::CognitiveMode => {
            state.self_model.mode.label().to_lowercase()
        }

        SlotSource::NeedLabel(need) => need_label_lower(need).to_string(),

        SlotSource::DeficitBucket => {
            let d = state.self_model.homeostasis.overall_deficit;
            if d < 0.20 { "low".to_string() }
            else if d < 0.40 { "moderate".to_string() }
            else if d < 0.60 { "elevated".to_string() }
            else if d < 0.80 { "high".to_string() }
            else { "critical".to_string() }
        }

        SlotSource::ErrorBucket => {
            let e = state.self_model.global_error;
            if e < 0.05 { "very low".to_string() }
            else if e < 0.15 { "low".to_string() }
            else if e < 0.25 { "moderate".to_string() }
            else if e < 0.40 { "elevated".to_string() }
            else if e < 0.60 { "high".to_string() }
            else { "very high".to_string() }
        }

        SlotSource::StabilityDescriptor => {
            verbalize_stability(state.self_model.identity_stability()).to_string()
        }

        SlotSource::HighestNeed => {
            let profile = &state.self_model.homeostasis;
            let needs = [
                (Need::Energy, profile.energy),
                (Need::Coherence, profile.coherence),
                (Need::Integration, profile.integration),
                (Need::Connection, profile.connection),
                (Need::Growth, profile.growth),
                (Need::Autonomy, profile.autonomy),
                (Need::Integrity, profile.integrity),
            ];
            needs.into_iter()
                .max_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal))
                .map(|(n, _)| need_label_lower(&n).to_string())
                .unwrap_or_default()
        }

        SlotSource::LowestNeed => {
            let profile = &state.self_model.homeostasis;
            let needs = [
                (Need::Energy, profile.energy),
                (Need::Coherence, profile.coherence),
                (Need::Integration, profile.integration),
                (Need::Connection, profile.connection),
                (Need::Growth, profile.growth),
                (Need::Autonomy, profile.autonomy),
                (Need::Integrity, profile.integrity),
            ];
            needs.into_iter()
                .min_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal))
                .map(|(n, _)| need_label_lower(&n).to_string())
                .unwrap_or_default()
        }

        SlotSource::StarvedDrive => {
            let starved = state.drives.starved_drive();
            drive_label(&starved).to_string()
        }

        SlotSource::StrongestDrive => {
            // Find the drive with highest intensity
            state.drives.drives.iter()
                .max_by(|a, b| a.intensity.partial_cmp(&b.intensity).unwrap_or(std::cmp::Ordering::Equal))
                .map(|d| drive_label(&d.id).to_string())
                .unwrap_or_else(|| "none".to_string())
        }

        SlotSource::L2ConceptCount => {
            // This needs external info; we use a placeholder heuristic
            // The actual count is set externally via SystemState if available.
            // For now, check if drives module tracks it.
            let count = state.drives.drives.len().saturating_mul(10); // rough proxy
            match count {
                0..=5 => "very few".to_string(),
                6..=20 => "a few".to_string(),
                21..=50 => "many".to_string(),
                _ => "a large number of".to_string(),
            }
        }

        SlotSource::FramesCount => {
            // Placeholder — actual frame count is set externally
            "(frames)".to_string()
        }

        SlotSource::RulesTotal => "(rules)".to_string(),
        SlotSource::RulesTrusted => "(trusted)".to_string(),

        SlotSource::DominantArchetype => {
            state.dominant_archetype
                .as_ref()
                .map(archetype_label)
                .unwrap_or("unknown")
                .to_string()
        }

        SlotSource::Emotion => {
            state.emotion
                .as_ref()
                .map(emotion_label)
                .unwrap_or("neutral")
                .to_string()
        }

        SlotSource::Stance => {
            state.stance
                .as_ref()
                .map(stance_label)
                .unwrap_or("open")
                .to_string()
        }

        SlotSource::Mood => {
            state.mood
                .as_ref()
                .map(mood_label)
                .unwrap_or("neutral")
                .to_string()
        }

        SlotSource::AttentionWinner => {
            if state.attention.winner_id.is_some() {
                state.attention.winner_label.clone()
            } else {
                "nothing".to_string()
            }
        }

        SlotSource::AttentionSimilarity => {
            let sim = state.attention.winner_similarity;
            format!("{:.0}%", sim * 100.0)
        }

        SlotSource::TickCount => {
            format!("{}", state.tick)
        }

        SlotSource::SleepTransitions => {
            format!("{}", state.sleep_transitions)
        }

        SlotSource::SleepL3Formed => {
            format!("{}", state.sleep_l3_formed)
        }

        SlotSource::SleepReason => {
            state.sleep_reason.clone().unwrap_or_else(|| "unknown".to_string())
        }

        SlotSource::WorkspaceIdle => {
            if state.workspace.is_idle() { "idle".to_string() } else { "active".to_string() }
        }

        SlotSource::ModeStatement => {
            make_mode_statement(&state.self_model.mode)
        }
    }
}

/// Generate a first-person statement describing a cognitive mode.
fn make_mode_statement(mode: &CognitiveMode) -> String {
    match mode {
        CognitiveMode::Quiet => {
            "I am quiet. Nothing demands my attention right now."
        }
        CognitiveMode::Companion => {
            "I am remembering. I am drawing on past experience."
        }
        CognitiveMode::Regulated => {
            "I am regulating. I am correcting my internal balance."
        }
        CognitiveMode::Explorer => {
            "I am exploring. I sense novelty and want to understand it."
        }
        CognitiveMode::Task => {
            "I am focused on a task. Memory and regulation guide me."
        }
        CognitiveMode::Resonant => {
            "I am in a resonant state. New patterns connect to things I have seen before."
        }
        CognitiveMode::Frontier => {
            "I am pushing into a frontier. I am regulating while exploring the unknown."
        }
        CognitiveMode::FullCouncil => {
            "I am fully engaged. Memory, regulation, and novelty are all active."
        }
    }.to_string()
}

// ═══════════════════════════════════════════════════════════════════════════
// INFLECTION RULES — Const definitions for the empty/inflected patterns
// ═══════════════════════════════════════════════════════════════════════════

impl InflectionRules {
    /// Empty rules: no inflection applied.
    pub const EMPTY: InflectionRules = InflectionRules {
        pluralize: false,
        past_tense: false,
        present_3sg: false,
        determiner: None,
        capitalize: false,
        prefix: None,
        suffix: None,
    };

    /// Past tense inflection.
    pub const fn past() -> Self {
        InflectionRules {
            past_tense: true,
            ..InflectionRules::EMPTY
        }
    }

    /// Capitalize the first letter.
    pub const fn cap() -> Self {
        InflectionRules {
            capitalize: true,
            ..InflectionRules::EMPTY
        }
    }

    /// Pluralize.
    pub const fn plural() -> Self {
        InflectionRules {
            pluralize: true,
            ..InflectionRules::EMPTY
        }
    }

    /// Present tense 3rd person singular.
    pub const fn pres3() -> Self {
        InflectionRules {
            present_3sg: true,
            ..InflectionRules::EMPTY
        }
    }

    /// Wrap with definite article "the".
    pub const fn def() -> Self {
        InflectionRules {
            determiner: Some(true),
            ..InflectionRules::EMPTY
        }
    }

    /// Wrap with indefinite article "a/an".
    pub const fn indef() -> Self {
        InflectionRules {
            determiner: Some(false),
            ..InflectionRules::EMPTY
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// FRAME DEFINITIONS — All narrative templates
// ═══════════════════════════════════════════════════════════════════════════

// Frame conditions
fn cond_first_tick(s: &SystemState) -> bool { s.is_first_tick }
fn cond_sleeping(s: &SystemState) -> bool { s.is_sleeping }
fn cond_sleep_narrative(s: &SystemState) -> bool { s.sleep_narrative.is_some() }
fn cond_crisis(s: &SystemState) -> bool { s.self_model.homeostasis.crisis }
fn cond_high_error(s: &SystemState) -> bool {
    s.self_model.global_error >= 0.40 && !s.self_model.homeostasis.crisis
}
fn cond_shock(s: &SystemState) -> bool {
    s.self_model.identity_stability() > 0.20
}
fn cond_workspace_idle(s: &SystemState) -> bool {
    s.workspace.is_idle() && !s.self_model.homeostasis.crisis
}
fn cond_always(_: &SystemState) -> bool { true }

/// The complete list of narrative frames, ordered by priority (descending).
///
/// FIRST MATCH WINS. Frames with higher priority are checked first.
/// This means crisis frames beat mode frames, shock beats routine, etc.
pub fn all_frames() -> Vec<NarrativeFrame> {
    vec![
        // ── Priority 100: Boot ──
        NarrativeFrame {
            description: "First tick — system just booted",
            priority: 100,
            condition: cond_first_tick,
            template: "I am awake. This is tick {tick}. I am beginning to observe the world around me.",
            slots: vec![SlotDef::new("tick", SlotSource::TickCount)],
        },

        // ── Priority 90: Crisis ──
        NarrativeFrame {
            description: "Homeostatic crisis — 2+ needs critical",
            priority: 90,
            condition: cond_crisis,
            template: "I am in crisis. My {highest_deficit} need is critical. \
                       My overall deficit is {deficit}. I must restore balance.",
            slots: vec![
                SlotDef::new("highest_deficit", SlotSource::HighestNeed),
                SlotDef::new("deficit", SlotSource::DeficitBucket),
            ],
        },

        // ── Priority 85: Cognitive shock ──
        NarrativeFrame {
            description: "Identity stability > 0.20 — cognitive shock",
            priority: 85,
            condition: cond_shock,
            template: "Something has changed dramatically. My sense of self is {stability}. \
                       I am reassessing my understanding.",
            slots: vec![
                SlotDef::with_inflection("stability", SlotSource::StabilityDescriptor, InflectionRules::def()),
            ],
        },

        // ── Priority 80: High prediction error ──
        NarrativeFrame {
            description: "Prediction error >= 0.40 — confused",
            priority: 80,
            condition: cond_high_error,
            template: "I do not understand what is happening. My prediction error is {error}. \
                       I need more information.",
            slots: vec![
                SlotDef::new("error", SlotSource::ErrorBucket),
            ],
        },

        // ── Priority 75: Actively sleeping ──
        NarrativeFrame {
            description: "System is currently sleeping",
            priority: 75,
            condition: cond_sleeping,
            template: "I am sleeping. I am consolidating what I have learned.",
            slots: vec![],
        },

        // ── Priority 72: Fear/Anxiety — triggered by high threat ──
        NarrativeFrame {
            description: "High fear or threat detected",
            priority: 72,
            condition: |s| {
                s.emotion == Some(Emotion::Fear) || s.mood == Some(Mood::Defensive)
            },
            template: "I feel afraid. My {mood} mood is making me cautious. \
                       I am attending to {attention} carefully.",
            slots: vec![
                SlotDef::new("mood", SlotSource::Mood),
                SlotDef::new("attention", SlotSource::AttentionWinner),
            ],
        },

        // ── Priority 71: Strong positive emotion ──
        NarrativeFrame {
            description: "High joy or curiosity",
            priority: 71,
            condition: |s| {
                s.emotion == Some(Emotion::Joy) || s.mood == Some(Mood::Curious)
                    || s.mood == Some(Mood::Playful)
            },
            template: "I feel {emotion}. My {mood} mood is driving me forward. \
                       I am eager to learn more about {attention}.",
            slots: vec![
                SlotDef::new("emotion", SlotSource::Emotion),
                SlotDef::new("mood", SlotSource::Mood),
                SlotDef::new("attention", SlotSource::AttentionWinner),
            ],
        },

        // ── Priority 70: Sleep narrative (after wake) ──
        NarrativeFrame {
            description: "Post-sleep narrative report",
            priority: 70,
            condition: cond_sleep_narrative,
            template: "I have just woken from sleep. I processed {transitions} significant transitions \
                       and formed {l3_formed} new meta-concepts. The sleep was triggered by {reason}.",
            slots: vec![
                SlotDef::new("transitions", SlotSource::SleepTransitions),
                SlotDef::new("l3_formed", SlotSource::SleepL3Formed),
                SlotDef::new("reason", SlotSource::SleepReason),
            ],
        },

        // ── Priority 60: Workspace idle ──
        NarrativeFrame {
            description: "Workspace idle — no module attended",
            priority: 60,
            condition: cond_workspace_idle,
            template: "My attention is diffuse. I am not focusing on anything in particular. \
                       I am waiting for something to catch my interest.",
            slots: vec![],
        },

        // ── Priority 50: Cognitive mode ──
        NarrativeFrame {
            description: "Default cognitive mode statement with emotion and archetype",
            priority: 50,
            condition: cond_always,
            template: "{mode_statement} I feel {emotion} and my mood is {mood}. \
                       My {archetype} archetype is active. \
                       My prediction error is {error}. \
                       My internal state is {stability}. \
                       I am attending to {attention} ({attention_sim} similarity).",
            slots: vec![
                SlotDef::new("mode_statement", SlotSource::ModeStatement),
                SlotDef::new("emotion", SlotSource::Emotion),
                SlotDef::new("mood", SlotSource::Mood),
                SlotDef::with_inflection("archetype", SlotSource::DominantArchetype, InflectionRules::indef()),
                SlotDef::new("error", SlotSource::ErrorBucket),
                SlotDef::with_inflection("stability", SlotSource::StabilityDescriptor, InflectionRules::def()),
                SlotDef::new("attention", SlotSource::AttentionWinner),
                SlotDef::new("attention_sim", SlotSource::AttentionSimilarity),
            ],
        },
    ]
}

// ═══════════════════════════════════════════════════════════════════════════
// NARRATIVE GENERATOR — The main entry point
// ═══════════════════════════════════════════════════════════════════════════

/// A pure rule-based narrative generator. No ML, no LLMs.
///
/// Usage:
/// ```ignore
/// let generator = NarrativeGenerator::new(&vocab);
/// let narrative = generator.generate(&system_state);
/// println!("{}", narrative);
/// ```
pub struct NarrativeGenerator {
    /// Ordered list of narrative frames (highest priority first).
    frames: Vec<NarrativeFrame>,
    /// Reference to the resonator vocabulary (for factorizing hypervectors).
    vocab: Option<ResonatorVocabulary>,
}

impl NarrativeGenerator {
    /// Create a new narrative generator with default frames and no vocabulary.
    ///
    /// To enable focus/term factorization, call `with_vocab()` or set vocab later.
    pub fn new() -> Self {
        let mut frames = all_frames();
        // Sort by priority descending
        frames.sort_by(|a, b| b.priority.cmp(&a.priority));
        NarrativeGenerator { frames, vocab: None }
    }

    /// Set the resonator vocabulary for term factorization.
    pub fn with_vocab(mut self, vocab: ResonatorVocabulary) -> Self {
        self.vocab = Some(vocab);
        self
    }

    /// Set or replace the frames list.
    pub fn with_frames(mut self, frames: Vec<NarrativeFrame>) -> Self {
        self.frames = frames;
        self.frames.sort_by(|a, b| b.priority.cmp(&a.priority));
        self
    }

    /// Get a reference to the vocabulary, if set.
    pub fn vocab(&self) -> Option<&ResonatorVocabulary> {
        self.vocab.as_ref()
    }

    /// Generate a narrative from the current system state.
    ///
    /// Walks frames in priority order, finds the first match, fills slots,
    /// and returns the completed sentence.
    pub fn generate(&self, state: &SystemState) -> String {
        for frame in &self.frames {
            if (frame.condition)(state) {
                return frame.fill(state);
            }
        }
        // Fallback: should never reach here since the mode frame has `cond_always`
        "I am functioning within normal parameters.".to_string()
    }

    /// Generate a short one-line summary (for HUD display).
    pub fn generate_summary(&self, state: &SystemState) -> String {
        let mode_label = state.self_model.mode.label();
        let deficit = state.self_model.homeostasis.overall_deficit;
        let error = state.self_model.global_error;
        let stability = state.self_model.identity_stability();
        let attention = if state.attention.winner_id.is_some() {
            &state.attention.winner_label
        } else {
            "idle"
        };

        format!(
            "NARRATIVE: mode={} deficit={:.2} error={:.2} stability={:.3} attending={}",
            mode_label.to_lowercase(),
            deficit,
            error,
            stability,
            attention,
        )
    }
}

impl Default for NarrativeGenerator {
    fn default() -> Self {
        Self::new()
    }
}

/// Generate a quick one-shot narrative from a SelfNarrative diagnostic struct.
///
/// This is a convenience function for the existing diagnostic pipeline.
/// It produces a short sentence without needing a full NarrativeGenerator.
pub fn quick_narrative(sn: &SelfNarrative) -> String {
    let mode_lower = sn.mode.to_lowercase();
    let deficit_bucket = if sn.overall_deficit < 0.20 { "low" }
        else if sn.overall_deficit < 0.40 { "moderate" }
        else if sn.overall_deficit < 0.60 { "elevated" }
        else if sn.overall_deficit < 0.80 { "high" }
        else { "critical" };

    let error_bucket = if sn.error < 0.05 { "very low" }
        else if sn.error < 0.15 { "low" }
        else if sn.error < 0.25 { "moderate" }
        else if sn.error < 0.40 { "elevated" }
        else if sn.error < 0.60 { "high" }
        else { "very high" };

    if sn.crisis {
        format!(
            "I am in crisis. My deficit is {} and my prediction error is {}. \
             I need to restore internal balance.",
            deficit_bucket, error_bucket,
        )
    } else if sn.is_confused {
        format!(
            "I am confused. My prediction error is {} and I am in {} mode. \
             My identity is {}.",
            error_bucket, mode_lower,
            if sn.stability < 0.05 { "stable" }
            else if sn.stability < 0.10 { "shifting gradually" }
            else if sn.stability < 0.20 { "changing significantly" }
            else { "in shock" },
        )
    } else {
        format!(
            "I am in {} mode. My deficit is {} and my prediction error is {}. \
             I am {}.",
            mode_lower, deficit_bucket, error_bucket,
            if sn.stability < 0.05 { "stable" }
            else if sn.stability < 0.10 { "shifting gradually" }
            else { "changing" },
        )
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// TESTS
// ═══════════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;
    use crate::drift::{CognitiveMode, HomeostaticRegulator};
    use crate::self_model::{HomeostaticProfile, SelfModel, SelfNarrative};
    use crate::workspace::GlobalWorkspace;

    // ── Morphology Tests ─────────────────────────────────────────────

    #[test]
    fn test_morphology_plural() {
        assert_eq!(pluralize("crisis"), "crises");
        assert_eq!(pluralize("index"), "indices");
        assert_eq!(pluralize("datum"), "data");
        assert_eq!(pluralize("analysis"), "analyses");
        assert_eq!(pluralize("dog"), "dogs");
        assert_eq!(pluralize("box"), "boxes");
        assert_eq!(pluralize("church"), "churches");
        assert_eq!(pluralize("fly"), "flies");
        assert_eq!(pluralize("key"), "keys");  // vowel+y → +s
        assert_eq!(pluralize("child"), "children");
        assert_eq!(pluralize("leaf"), "leaves");
        assert_eq!(pluralize("knife"), "knives");
    }

    #[test]
    fn test_morphology_past_tense() {
        assert_eq!(past_tense("raise"), "raised");
        assert_eq!(past_tense("fall"), "fell");
        assert_eq!(past_tense("rise"), "rose");
        assert_eq!(past_tense("bind"), "bound");
        assert_eq!(past_tense("encode"), "encoded");
        assert_eq!(past_tense("run"), "ran");
        assert_eq!(past_tense("set"), "set");
        assert_eq!(past_tense("write"), "wrote");
        assert_eq!(past_tense("read"), "read");
        assert_eq!(past_tense("be"), "was");
        assert_eq!(past_tense("go"), "went");
        assert_eq!(past_tense("think"), "thought");
        assert_eq!(past_tense("trigger"), "triggered");
        assert_eq!(past_tense("observe"), "observed");
        assert_eq!(past_tense("study"), "studied"); // consonant+y → ied
        assert_eq!(past_tense("play"), "played");   // vowel+y → ed
    }

    #[test]
    fn test_morphology_present_tense() {
        assert_eq!(present_tense("raise", true), "raises");
        assert_eq!(present_tense("raise", false), "raise");
        assert_eq!(present_tense("go", true), "goes");
        assert_eq!(present_tense("have", true), "has");
        assert_eq!(present_tense("be", true), "is");
        assert_eq!(present_tense("push", true), "pushes");
        assert_eq!(present_tense("study", true), "studies");
        assert_eq!(present_tense("play", true), "plays");
        assert_eq!(present_tense("run", true), "runs");
    }

    #[test]
    fn test_morphology_article() {
        assert_eq!(choose_article("crisis", true), "a");
        assert_eq!(choose_article("anomaly", true), "an");
        assert_eq!(choose_article("idea", true), "an");
        assert_eq!(choose_article("universe", true), "an");
        assert_eq!(choose_article("crisis", false), "the");
        assert_eq!(choose_article("anomaly", false), "the");
    }

    #[test]
    fn test_morphology_capitalize() {
        assert_eq!(capitalize_sentence("hello world"), "Hello world");
        assert_eq!(capitalize_sentence("already capitalized"), "Already capitalized");
        assert_eq!(capitalize_sentence(""), "");
        assert_eq!(capitalize_sentence("123 hello"), "123 Hello");
    }

    #[test]
    fn test_verbalize_intensity() {
        assert_eq!(verbalize_intensity(0.01), "minimal");
        assert_eq!(verbalize_intensity(0.10), "very low");
        assert_eq!(verbalize_intensity(0.25), "low");
        assert_eq!(verbalize_intensity(0.50), "moderate");
        assert_eq!(verbalize_intensity(0.75), "high");
        assert_eq!(verbalize_intensity(0.90), "very high");
        assert_eq!(verbalize_intensity(0.98), "critical");
    }

    #[test]
    fn test_verbalize_stability() {
        assert_eq!(verbalize_stability(0.03), "stable");
        assert_eq!(verbalize_stability(0.07), "shifting gradually");
        assert_eq!(verbalize_stability(0.15), "changing significantly");
        assert_eq!(verbalize_stability(0.25), "in shock");
    }

    // ── Slot Resolver Tests ──────────────────────────────────────────

    /// Helper: create a minimal system state for testing.
    /// Uses `Box::leak` to get 'static references — safe in tests.
    fn test_state() -> SystemState<'static> {
        let mut sm = Box::new(SelfModel::new());
        let profile = HomeostaticProfile::satisfied();
        let mode = CognitiveMode::Explorer;
        let focus = Hypervector::new_zero();
        sm.tick(0.10, profile, mode, focus);

        let mut ws = Box::new(GlobalWorkspace::with_defaults());
        ws.register_module("TEST_MOD", true);
        ws.update_module(0, Hypervector::new_random());
        let attention = ws.evaluate_attention(&sm.current_identity);

        let drives = Box::new(IntrinsicMotivation::new());

        // Leak the boxes to get 'static references (safe in tests)
        let sm_ref: &'static mut SelfModel = Box::leak(sm);
        let ws_ref: &'static mut GlobalWorkspace = Box::leak(ws);
        let drives_ref: &'static mut IntrinsicMotivation = Box::leak(drives);
        let attention_box = Box::new(attention);
        let attention_ref: &'static AttentionReport = Box::leak(attention_box);

        SystemState {
            self_model: sm_ref,
            attention: attention_ref,
            workspace: ws_ref,
            drives: drives_ref,
            dominant_archetype: Some(Archetype::Sage),
            emotion: Some(Emotion::Neutral),
            stance: Some(Stance::Curious),
            mood: Some(Mood::Analytical),
            sleep_narrative: None,
            sleep_transitions: 0,
            sleep_l3_formed: 0,
            is_first_tick: false,
            tick: 1,
            is_sleeping: false,
            sleep_reason: None,
        }
    }

    // ── Quick Narrative Tests ────────────────────────────────────────

    #[test]
    fn test_quick_narrative_crisis() {
        let sn = SelfNarrative {
            tick: 42,
            mode: "EXPLORER".to_string(),
            overall_deficit: 0.85,
            crisis: true,
            error: 0.45,
            is_confused: true,
            stability: 0.15,
            is_transitioning: false,
            weights: [0.25; 4],
        };
        let n = quick_narrative(&sn);
        assert!(n.contains("crisis"), "Crisis narrative: {}", n);
    }

    #[test]
    fn test_quick_narrative_confused() {
        let sn = SelfNarrative {
            tick: 42,
            mode: "REGULATED".to_string(),
            overall_deficit: 0.30,
            crisis: false,
            error: 0.35,
            is_confused: true,
            stability: 0.08,
            is_transitioning: false,
            weights: [0.35, 0.35, 0.10, 0.20],
        };
        let n = quick_narrative(&sn);
        assert!(n.contains("confused"), "Confused narrative: {}", n);
    }

    #[test]
    fn test_quick_narrative_normal() {
        let sn = SelfNarrative {
            tick: 100,
            mode: "TASK".to_string(),
            overall_deficit: 0.15,
            crisis: false,
            error: 0.08,
            is_confused: false,
            stability: 0.03,
            is_transitioning: false,
            weights: [0.25; 4],
        };
        let n = quick_narrative(&sn);
        assert!(n.contains("task"), "Normal narrative: {}", n);
    }

    // ── Inflection Rules Tests ───────────────────────────────────────

    #[test]
    fn test_inflection_rules() {
        let rules = InflectionRules {
            pluralize: true,
            ..InflectionRules::EMPTY
        };
        assert_eq!(rules.apply("crisis"), "crises");

        let rules = InflectionRules {
            past_tense: true,
            ..InflectionRules::EMPTY
        };
        assert_eq!(rules.apply("rise"), "rose");

        let rules = InflectionRules {
            determiner: Some(false),
            ..InflectionRules::EMPTY
        };
        assert_eq!(rules.apply("anomaly"), "an anomaly");

        let rules = InflectionRules {
            capitalize: true,
            ..InflectionRules::EMPTY
        };
        assert_eq!(rules.apply("hello"), "Hello");
    }

    // ── Mode Statement Tests ─────────────────────────────────────────

    #[test]
    fn test_mode_statements_all() {
        let modes = [
            (CognitiveMode::Quiet, "quiet"),
            (CognitiveMode::Companion, "remembering"),
            (CognitiveMode::Regulated, "regulating"),
            (CognitiveMode::Explorer, "exploring"),
            (CognitiveMode::Task, "focused"),
            (CognitiveMode::Resonant, "resonant"),
            (CognitiveMode::Frontier, "frontier"),
            (CognitiveMode::FullCouncil, "fully engaged"),
        ];
        for (mode, keyword) in &modes {
            let statement = make_mode_statement(mode);
            assert!(
                statement.to_lowercase().contains(keyword),
                "Mode {:?} statement should contain '{}': got '{}'",
                mode, keyword, statement,
            );
        }
    }

    // ── Edge Case Tests ──────────────────────────────────────────────

    #[test]
    fn test_generator_default_not_panic() {
        let generator = NarrativeGenerator::new();
        // Calling generate with a dummy state should always produce output
        // (the default mode frame has cond_always)
        let mut sm = Box::new(SelfModel::new());
        sm.tick(0.0, HomeostaticProfile::satisfied(), CognitiveMode::Quiet, Hypervector::new_zero());
        let ws_ref: &'static mut GlobalWorkspace = Box::leak(Box::new(GlobalWorkspace::with_defaults()));
        let attention_ref: &'static AttentionReport = Box::leak(Box::new(AttentionReport::new()));
        let sm_ref: &'static mut SelfModel = Box::leak(sm);
        let drives_ref: &'static mut IntrinsicMotivation = Box::leak(Box::new(IntrinsicMotivation::new()));

        let state = SystemState {
            self_model: sm_ref,
            attention: attention_ref,
            workspace: ws_ref,
            drives: drives_ref,
            dominant_archetype: None,
            emotion: None,
            stance: None,
            mood: None,
            sleep_narrative: None,
            sleep_transitions: 0,
            sleep_l3_formed: 0,
            is_first_tick: false,
            tick: 0,
            is_sleeping: false,
            sleep_reason: None,
        };

        let narrative = generator.generate(&state);
        assert!(!narrative.is_empty(), "Generator should always produce output");
    }

    #[test]
    fn test_empty_vocab_no_panic() {
        let generator = NarrativeGenerator::new();
        // Without a vocabulary, focus factorization should gracefully degrade
        assert!(generator.vocab().is_none(), "No vocab by default");
    }

    #[test]
    fn test_summary_format() {
        let state = test_state();
        let generator = NarrativeGenerator::new();
        let summary = generator.generate_summary(&state);
        assert!(summary.starts_with("NARRATIVE:"), "Summary should start with NARRATIVE:");
        assert!(summary.contains("mode="), "Summary should contain mode");
        assert!(summary.contains("deficit="), "Summary should contain deficit");
    }

    #[test]
    fn test_need_labels_all() {
        for need in Need::all() {
            let label = need_label_lower(&need);
            assert!(!label.is_empty(), "Need label should not be empty");
        }
    }

    #[test]
    fn test_archetype_labels_all() {
        let archs = [
            Archetype::Hero, Archetype::Shadow, Archetype::Sage,
            Archetype::Trickster, Archetype::Caregiver, Archetype::Orphan,
        ];
        for arch in &archs {
            let label = archetype_label(arch);
            assert!(!label.is_empty(), "Archetype label should not be empty");
        }
    }

    // ── Dependency Linearization Tests (Phase 2) ─────────────────────

    #[test]
    fn test_dep_graph_empty() {
        let deps = DepGraph::new();
        let sentence = deps.linearize();
        assert_eq!(sentence, "");
    }

    #[test]
    fn test_dep_graph_simple_svo() {
        let mut deps = DepGraph::new();
        deps.add(DepRel::new("nsubj", "the system"));
        deps.add(DepRel::new("verb", "explore"));
        deps.add(DepRel::new("dobj", "new data"));
        let sentence = deps.linearize();
        assert!(sentence.contains("The system"));
        assert!(sentence.contains("explore"));
        assert!(sentence.contains("new data"));
        assert!(sentence.ends_with('.'));
    }

    #[test]
    fn test_dep_graph_with_negation() {
        let mut deps = DepGraph::new();
        deps.add(DepRel::new("nsubj", "I"));
        deps.add(DepRel::new("neg", "not"));
        deps.add(DepRel::new("verb", "know"));
        let sentence = deps.linearize();
        assert!(sentence.contains("I"));
        assert!(sentence.contains("not"));
        assert!(sentence.contains("know"));
    }

    #[test]
    fn test_dep_graph_order() {
        let mut deps = DepGraph::new();
        deps.add(DepRel::new("nsubj", "I"));
        deps.add(DepRel::new("verb", "transition"));
        deps.add(DepRel::new("obl", "to quiet"));
        let sentence = deps.linearize();
        assert!(sentence.contains("I"));
        assert!(sentence.contains("transition"));
        assert!(sentence.contains("to quiet"));
    }

    #[test]
    fn test_dep_graph_overwrite() {
        let mut deps = DepGraph::new();
        deps.add(DepRel::new("verb", "run"));
        deps.add(DepRel::new("verb", "walk")); // should overwrite
        assert_eq!(deps.get("verb"), Some("walk"));
    }

    #[test]
    fn test_build_action_dep_graph() {
        let state = test_state();
        let deps = build_action_dep_graph(&state);
        let sentence = deps.linearize();
        assert!(!sentence.is_empty());
        assert!(sentence.contains("explore") || sentence.contains("am"));
    }

    // ── VSA N-gram Chain Tests (Phase 3) ─────────────────────────────

    #[test]
    fn test_ngram_chain_register() {
        let chain = NgramChain::bigram();
        assert_eq!(chain.state_count(), 0);
        assert_eq!(chain.transition_count(), 0);
    }

    #[test]
    fn test_ngram_chain_observe() {
        let mut chain = NgramChain::bigram();
        chain.observe("explorer", "task");
        assert_eq!(chain.transition_count(), 1);
    }

    #[test]
    fn test_ngram_chain_predict_after_observe() {
        let mut chain = NgramChain::bigram();
        chain.observe("explorer", "task");
        chain.observe("task", "regulated");
        chain.observe("task", "regulated");
        chain.observe("task", "regulated");

        // After observing task→regulated 3 times, predict(task) should return regulated
        let prediction = chain.predict("task");
        assert_eq!(prediction, Some("regulated".to_string()));
    }

    #[test]
    fn test_ngram_chain_predict_unknown() {
        let chain = NgramChain::bigram();
        let prediction = chain.predict("unknown");
        assert!(prediction.is_none());
    }

    #[test]
    fn test_ngram_chain_generate_sequence() {
        let mut chain = NgramChain::bigram();
        chain.observe("quiet", "explorer");
        chain.observe("explorer", "task");
        chain.observe("task", "regulated");

        let sequence = chain.generate_sequence("quiet", 3);
        assert!(!sequence.is_empty());
        assert_eq!(sequence[0], "explorer");
    }

    #[test]
    fn test_ngram_chain_prediction_narrative() {
        let mut chain = NgramChain::bigram();
        chain.observe("quiet", "explorer");
        chain.observe("explorer", "task");

        let narrative = chain.prediction_narrative("quiet");
        assert!(narrative.contains("I"));
        assert!(narrative.contains("explorer"));
    }

    #[test]
    fn test_ngram_chain_register_states() {
        let mut chain = NgramChain::bigram();
        chain.register_states(&["alpha", "beta", "gamma"]);
        assert_eq!(chain.state_count(), 3);
    }

    #[test]
    fn test_ngram_chain_different_orders() {
        // Test that the chain correctly predicts the most common transition
        let mut chain = NgramChain::bigram();
        // A→B twice, A→C once
        chain.observe("a", "b");
        chain.observe("a", "b");
        chain.observe("a", "c");

        let prediction = chain.predict("a");
        assert_eq!(prediction, Some("b".to_string()));
    }
}
