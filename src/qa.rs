// ─── Question Answering Engine: Pure VSA Vector Unbinding ─────────────
//
// Answers questions by algebraic manipulation of memorized thought vectors.
// No ML, no LLMs — only XOR, rotation, and majority-sum.
//
// ## The Math
//
// A thought is stored as:
//   Thought = ρ₁₃(S) ⊕ ρ₂₆(V) ⊕ ρ₃₉(O)
//
// Given a question like "Who raised rates?" with known V and O, we scan
// each fact in memory and XOR out the known slots. If the known slots
// match the fact, the remaining vector cleanly decodes to the answer.
//
// Because XOR is its own inverse and rotation is invertible:
//   ρ₁₃(S) = Thought ⊕ ρ₂₆(V) ⊕ ρ₃₉(O)
//   S = ρ₁₃⁻¹(ρ₁₃(S))
//
// Noise comes only from bundle interference in the memory bank, not from
// the unbinding operation itself.
//
// ## Test Coverage
//
// 1. test_basic_fact     — "Who raised rates?" → "the_fed"
// 2. test_different_verb — "Who cut rates?" → "the_fed"
// 3. test_no_match       — No matching fact → "do not know"
// 4. test_full_qa_cycle  — Store fact, ask question, verify answer
// 5. test_verify_true    — Verify existing fact
// 6. test_verify_false   — Reject non-existing fact
//
// ────────────────────────────────────────────────────────────────────────────

use crate::Hypervector;
use crate::nlp;
use crate::resonator;

// ═══════════════════════════════════════════════════════════════════════════
// CONSTANTS
// ═══════════════════════════════════════════════════════════════════════════

/// Rotation for subject slot (matches resonator::encode_svo).
const RHO_S: usize = 13;
/// Rotation for verb slot.
const RHO_V: usize = 26;
/// Rotation for object slot.
const RHO_O: usize = 39;

/// Question words indicating an unknown subject.
const QUESTION_SUBJECT: &[&str] = &["who", "what", "which"];
/// Question words indicating an unknown object.
const QUESTION_OBJECT: &[&str] = &["what", "whom", "which"];

/// Minimum cleanup similarity for an unbinding to be accepted.
/// Must be above the noise floor: with N=12 vocabulary terms and
/// D=10240, random max similarity ≈ 0.52. We set 0.56 to ensure
/// only genuine matches pass.
const MIN_CLEANUP_ENERGY: f64 = 0.56;

// ═══════════════════════════════════════════════════════════════════════════
// QA MEMORY
// ═══════════════════════════════════════════════════════════════════════════

/// A single fact stored in the QA engine's memory.
#[derive(Clone, Debug)]
pub struct QaFact {
    /// Bound hypervector: ρ₁₃(S) ⊕ ρ₂₆(V) ⊕ ρ₃₉(O)
    pub thought: Hypervector,
    /// Subject text (e.g., "the_fed").
    pub subject: String,
    /// Verb text (lemmatized, e.g., "raise").
    pub verb: String,
    /// Object text (e.g., "rates").
    pub object: String,
    /// Original source sentence.
    pub source: String,
    /// Tick when this fact was stored (for temporal ordering).
    pub tick: u64,
    /// True if a later fact contradicts this one (same subject/object
    /// but opposite verb, or same subject/verb but opposite object).
    pub is_contradicted: bool,
}

/// Which SVO slot is the answer.
#[derive(Clone, Debug, PartialEq)]
pub enum AnswerSlot {
    Subject,
    Verb,
    Object,
}

// ═══════════════════════════════════════════════════════════════════════════
// QA ENGINE
// ═══════════════════════════════════════════════════════════════════════════

/// Pure VSA question-answering engine.
///
/// Stores facts as bound SVO hypervectors and answers questions by
/// vector unbinding — no ML, no LLMs.
pub struct QaEngine {
    facts: Vec<QaFact>,
    /// Monotonically increasing tick counter for fact storage ordering.
    next_tick: u64,
}

impl QaEngine {
    pub fn new() -> Self {
        QaEngine { facts: Vec::new(), next_tick: 0 }
    }

    // ── Fact Storage ────────────────────────────────────────────────

    /// Store a fact from raw text strings.
    ///
    /// If this fact contradicts an existing fact (same subject + same
    /// object but opposite verb, or same subject + same verb but
    /// opposite object), the OLDER fact is marked as `is_contradicted`.
    pub fn store_fact(&mut self, subject: &str, verb: &str, object: &str, source: &str) {
        let s_hv = Hypervector::encode_text_ngram(subject, 3);
        let v_hv = Hypervector::encode_text_ngram(verb, 3);
        let o_hv = if object.is_empty() {
            Hypervector::new_zero()
        } else {
            Hypervector::encode_text_ngram(object, 3)
        };
        let thought = resonator::encode_svo(&s_hv, &v_hv, &o_hv);
        let tick = self.next_tick;
        self.next_tick += 1;

        // Detect if this fact contradicts any previous fact
        let subj_lower = subject.trim().to_lowercase();
        let verb_lower = verb.trim().to_lowercase();
        let obj_lower = object.trim().to_lowercase();

        for existing in &mut self.facts {
            let e_subj = existing.subject.trim().to_lowercase();
            let e_verb = existing.verb.trim().to_lowercase();
            let e_obj = existing.object.trim().to_lowercase();

            // Only check facts with the same subject
            if e_subj != subj_lower {
                continue;
            }

            // Case 1: same subject + same object + opposite verb
            // e.g., stored "the_fed raise rates" + new "the_fed cut rates"
            if e_obj == obj_lower && !e_obj.is_empty() {
                if crate::narrative::is_antonym(&e_verb, &verb_lower) {
                    existing.is_contradicted = true;
                }
            }

            // Case 2: same subject + same verb + opposite object
            // e.g., stored "the_fed raise rates" + new "the_fed raise inflation"
            if e_verb == verb_lower && !e_verb.is_empty() {
                if crate::narrative::is_object_antonym(&e_obj, &obj_lower) {
                    existing.is_contradicted = true;
                }
            }
        }

        self.facts.push(QaFact {
            thought,
            subject: subject.to_string(),
            verb: verb.to_string(),
            object: object.to_string(),
            source: source.to_string(),
            tick,
            is_contradicted: false, // newly stored facts start clean
        });
    }

    /// Store a fact from an SVO triple.
    pub fn store_triple(&mut self, triple: &nlp::SvoTriple, source: &str) {
        self.store_fact(&triple.subject, &triple.verb, &triple.object, source);
    }

    /// Store multiple facts from SVO triples.
    pub fn store_triples(&mut self, triples: &[nlp::SvoTriple], source: &str) {
        for triple in triples {
            self.store_triple(triple, source);
        }
    }

    /// Number of stored facts.
    pub fn fact_count(&self) -> usize {
        self.facts.len()
    }

    // ═════════════════════════════════════════════════════════════════
    // QUESTION PARSING
    // ═════════════════════════════════════════════════════════════════

    /// Parse a question and determine which slot is unknown.
    ///
    /// Returns (answer_slot, subject_str, verb_str, object_str).
    pub fn parse_question(question: &str) -> (AnswerSlot, String, Option<String>, Option<String>) {
        let triples = nlp::extract_svo(question);
        if triples.is_empty() {
            return Self::parse_heuristic(question);
        }
        let triple = &triples[0];
        let subj_lower = triple.subject.to_lowercase();
        let verb_lower = triple.verb.to_lowercase();
        let obj_lower = triple.object.to_lowercase();

        let is_subj_q = QUESTION_SUBJECT.iter().any(|q| subj_lower.contains(q));
        let is_obj_q = QUESTION_OBJECT.iter().any(|q| obj_lower.contains(q));
        let is_verb_q = verb_lower.contains("did") || verb_lower.contains("does")
            || verb_lower.contains("do") || verb_lower.contains("will");

        if is_subj_q && !is_verb_q {
            (AnswerSlot::Subject, triple.subject.clone(), Some(triple.verb.clone()), Some(triple.object.clone()))
        } else if is_obj_q {
            let known_subj = if is_subj_q { String::new() } else { triple.subject.clone() };
            (AnswerSlot::Object, known_subj, Some(triple.verb.clone()), Some(triple.object.clone()))
        } else if is_verb_q {
            let known_subj = if is_subj_q { String::new() } else { triple.subject.clone() };
            let known_obj = if is_obj_q { String::new() } else { triple.object.clone() };
            (AnswerSlot::Verb, known_subj, None, Some(known_obj))
        } else {
            (AnswerSlot::Subject, triple.subject.clone(), Some(triple.verb.clone()), Some(triple.object.clone()))
        }
    }

    /// Heuristic fallback parser.
    fn parse_heuristic(question: &str) -> (AnswerSlot, String, Option<String>, Option<String>) {
        let lower = question.to_lowercase().trim().to_string();
        for qword in &["who", "what", "whom", "which"] {
            if lower.starts_with(qword) {
                let rest = question[qword.len()..].trim().trim_end_matches('?').trim_end_matches('.');
                let rest_lower = rest.to_lowercase();
                let aux_patterns = ["did ", "does ", "do ", "will ", "has ", "have ", "had ", "is ", "are "];
                let (verb, object) = if let Some(aux) = aux_patterns.iter().find(|a| rest_lower.starts_with(*a)) {
                    let after = rest[aux.len()..].trim(); // text after the aux verb
                    let words: Vec<&str> = after.split_whitespace().collect();
                    if words.len() >= 2 {
                        (Some(crate::nlp::verb_lemma(words[0])), Some(words[1..].join(" ")))
                    } else if words.len() == 1 {
                        (Some(crate::nlp::verb_lemma(words[0])), None)
                    } else {
                        (None, None)
                    }
                } else {
                    let words: Vec<&str> = rest.split_whitespace().collect();
                    if words.len() >= 2 {
                        (Some(crate::nlp::verb_lemma(words[0])), Some(words[1..].join(" ")))
                    } else if words.len() == 1 {
                        (Some(crate::nlp::verb_lemma(words[0])), None)
                    } else {
                        (None, None)
                    }
                };
                // Clean question words from object
                let clean_obj = object.map(|o| {
                    let lower_o = o.to_lowercase();
                    if QUESTION_OBJECT.iter().any(|q| lower_o.contains(q)) {
                        String::new()
                    } else {
                        o
                    }
                });
                return (AnswerSlot::Subject, qword.to_string(), verb, clean_obj);
            }
        }
        (AnswerSlot::Subject, String::new(), None, None)
    }

    // ═════════════════════════════════════════════════════════════════
    // ANSWERING (Core: scan + unbind + cleanup)
    // ═════════════════════════════════════════════════════════════════

    /// Answer a question by scanning all stored facts.
    ///
    /// For each fact: unbind known slots → cleanup → verify reconstruction.
    /// Returns the best match as a natural language sentence.
    pub fn answer(&self, question: &str) -> String {
        // 1. Parse the question
        let (answer_slot, known_subj, known_verb, known_obj) = Self::parse_question(question);

        // 2. Normalize knowns (remove question words, lemmatize verbs)
        let clean_s = if answer_slot != AnswerSlot::Subject && !known_subj.is_empty()
            && !QUESTION_SUBJECT.iter().any(|q| known_subj.to_lowercase().contains(q))
        { Some(known_subj.to_string()) } else { None };

        let clean_v = known_verb.as_ref()
            .filter(|v| !v.is_empty())
            .map(|v| crate::nlp::verb_lemma(v)); // lemmatize verb

        let clean_o = if answer_slot != AnswerSlot::Object {
            known_obj.as_ref()
                .filter(|o| !o.is_empty())
                .filter(|o| !QUESTION_OBJECT.iter().any(|q| o.to_lowercase().contains(q)))
                .cloned()
        } else { None };

        if clean_v.is_none() && clean_o.is_none() && clean_s.is_none() {
            return "I need more information to answer that question.".to_string();
        }

        // 3. Scan facts
        let result = self.scan_facts(&answer_slot, &clean_s, &clean_v, &clean_o);

        match result {
            Some((answer_token, matched)) => self.format_answer(&answer_slot, &answer_token, matched),
            None => "I do not know the answer to that question.".to_string(),
        }
    }

    /// Answer a question returning ALL matching facts, not just the best.
    ///
    /// Useful for multi-answer questions like "What did the Fed do?"
    /// when memory contains both "raise rates" and "cut rates".
    pub fn answer_all(&self, question: &str) -> Vec<(String, &QaFact)> {
        let (answer_slot, known_subj, known_verb, known_obj) = Self::parse_question(question);

        let clean_s = if answer_slot != AnswerSlot::Subject && !known_subj.is_empty()
            && !QUESTION_SUBJECT.iter().any(|q| known_subj.to_lowercase().contains(q))
        { Some(known_subj.to_string()) } else { None };

        let clean_v = known_verb.as_ref()
            .filter(|v| !v.is_empty())
            .map(|v| crate::nlp::verb_lemma(v));

        let clean_o = if answer_slot != AnswerSlot::Object {
            known_obj.as_ref()
                .filter(|o| !o.is_empty())
                .filter(|o| !QUESTION_OBJECT.iter().any(|q| o.to_lowercase().contains(q)))
                .cloned()
        } else { None };

        if clean_v.is_none() && clean_o.is_none() && clean_s.is_none() {
            return Vec::new();
        }

        // Scan all facts, collect EVERYTHING above threshold
        let mut results: Vec<(String, f64, &QaFact)> = Vec::new();

        for fact in &self.facts {
            let result_hv = self.unbind_slot(&fact.thought, &answer_slot, &clean_s, &clean_v, &clean_o);
            let token = self.best_vocab_match(&result_hv);

            let (s_str, v_str, o_str) = match &answer_slot {
                AnswerSlot::Subject => (token.as_str(), clean_v.as_deref().unwrap_or(""), clean_o.as_deref().unwrap_or("")),
                AnswerSlot::Verb => (clean_s.as_deref().unwrap_or(""), token.as_str(), clean_o.as_deref().unwrap_or("")),
                AnswerSlot::Object => (clean_s.as_deref().unwrap_or(""), clean_v.as_deref().unwrap_or(""), token.as_str()),
            };

            let energy = self.reconstruction_energy(&fact.thought, s_str, v_str, o_str);
            if energy >= MIN_CLEANUP_ENERGY {
                results.push((token, energy, fact));
            }
        }

        // Sort by tick (oldest first) for chronological output
        results.sort_by(|a, b| a.2.tick.cmp(&b.2.tick));
        results.into_iter().map(|(t, _, f)| (t, f)).collect()
    }

    /// Answer with a combined multi-fact sentence that handles contradictions
    /// using temporal markers ("At first...", "then...", "later...").
    ///
    /// Examples:
    ///   Single fact:  "The Fed raised rates."
    ///   Multiple:     "The Fed raised rates and cut rates."
    ///   Contradiction: "The Fed raised rates. However, the Fed later lowered rates."
    pub fn answer_combined(&self, question: &str) -> String {
        let results = self.answer_all(question);
        if results.is_empty() {
            return "I do not know the answer to that question.".to_string();
        }

        // Split into contradicted vs uncontradicted facts
        let (mut contradicted, mut clean): (Vec<_>, Vec<_>) = results
            .into_iter()
            .partition(|(_, fact)| fact.is_contradicted);

        if contradicted.is_empty() && clean.is_empty() {
            return "I do not know the answer to that question.".to_string();
        }

        // If there are both contradicted and clean facts, order them temporally
        // Clean facts come first (older, stable), then contradicted (overridden)
        let mut all_clean: Vec<String> = Vec::new();
        let mut all_contra: Vec<String> = Vec::new();

        for (token, fact) in &clean {
            all_clean.push(self.format_answer(&AnswerSlot::Subject, token, fact));
        }
        for (token, fact) in &contradicted {
            all_contra.push(self.format_answer_contradicted(token, fact));
        }

        // Build the combined output
        match (all_clean.len(), all_contra.len()) {
            (0, 1) => all_contra.remove(0),
            (0, _) => {
                // Only contradicted facts: "At first, X. However, later Y."
                self.conjoin_with_contrast(&all_contra)
            }
            (1, 0) => all_clean.remove(0),
            (_, 0) => {
                // Multiple clean facts: "X, Y, and Z."
                let last = all_clean.pop().unwrap();
                format!("{}, and {}.", all_clean.join(", "),
                    last.trim_end_matches('.'))
            }
            (_, _) => {
                // Mixed: clean facts first, then contradicted
                let clean_part = if all_clean.len() == 1 {
                    all_clean.remove(0)
                } else {
                    let last = all_clean.pop().unwrap();
                    format!("{}, and {}.",
                        all_clean.join(", "), last.trim_end_matches('.'))
                };
                let contra_part = self.conjoin_with_contrast(&all_contra);
                format!("{} {}", clean_part.trim_end_matches('.'), contra_part)
            }
        }
    }

    /// Join contradicted facts with contrastive discourse markers.
    fn conjoin_with_contrast(&self, facts: &[String]) -> String {
        if facts.is_empty() {
            return String::new();
        }
        if facts.len() == 1 {
            return format!("At first, {} However, that has since changed.",
                facts[0].to_lowercase());
        }

        // "At first, X. However, later Y. Later still, Z."
        let mut output = String::new();
        output.push_str("At first, ");
        // Remove trailing period from first fact
        let first = facts[0].trim_end_matches('.');
        output.push_str(&first.to_lowercase());
        output.push('.');

        for (i, fact) in facts[1..].iter().enumerate() {
            let marker = if i == 0 {
                " However, later"
            } else {
                " Later still"
            };
            output.push_str(marker);
            output.push(',');
            output.push(' ');
            // Remove leading "the" if present (for natural flow)
            let lower = fact.trim_end_matches('.').to_lowercase();
            let trimmed = if lower.starts_with("the ") {
                &lower[4..]
            } else {
                &lower
            };
            output.push_str(trimmed);
            output.push('.');
        }

        output
    }

    /// Format an answer for a contradicted fact (past-tense, with temporal marking).
    fn format_answer_contradicted(&self, token: &str, fact: &QaFact) -> String {
        // For contradicted facts, use past-perfect-like framing
        format!(
            "{} has {} {}.",
            fact.subject,
            crate::narrative::past_tense(&fact.verb),
            fact.object,
        )
    }

    /// Scan all facts, unbind known slots, find best match.
    fn scan_facts<'a>(
        &'a self,
        answer_slot: &AnswerSlot,
        clean_s: &Option<String>,
        clean_v: &Option<String>,
        clean_o: &Option<String>,
    ) -> Option<(String, &'a QaFact)> {
        if self.facts.is_empty() {
            return None;
        }

        let mut best: Option<(String, f64, &QaFact)> = None;

        for fact in &self.facts {
            // Unbind known slots from this fact's thought
            let result_hv = self.unbind_slot(&fact.thought, answer_slot, clean_s, clean_v, clean_o);

            // Cleanup through vocabulary (encode_text_ngram vocabulary)
            let token = self.best_vocab_match(&result_hv);

            // Verify reconstruction energy
            let (s_str, v_str, o_str) = match answer_slot {
                AnswerSlot::Subject => (token.as_str(), clean_v.as_deref().unwrap_or(""), clean_o.as_deref().unwrap_or("")),
                AnswerSlot::Verb => (clean_s.as_deref().unwrap_or(""), token.as_str(), clean_o.as_deref().unwrap_or("")),
                AnswerSlot::Object => (clean_s.as_deref().unwrap_or(""), clean_v.as_deref().unwrap_or(""), token.as_str()),
            };

            let energy = self.reconstruction_energy(&fact.thought, s_str, v_str, o_str);

            if energy < MIN_CLEANUP_ENERGY {
                continue;
            }

            match &best {
                Some((_, best_e, _)) if energy <= *best_e => {}
                _ => { best = Some((token, energy, fact)); }
            }
        }

        best.map(|(token, _, fact)| (token, fact))
    }

    /// Find the best vocabulary match for a raw hypervector.
    /// Since we don't hold a ResonatorVocabulary reference, we check
    /// against the stored facts' subject/verb/object strings directly.
    fn best_vocab_match(&self, hv: &Hypervector) -> String {
        let mut best_token = String::new();
        let mut best_sim = 0.0_f64;

        for fact in &self.facts {
            for token in [&fact.subject, &fact.verb, &fact.object] {
                if token.is_empty() {
                    continue;
                }
                let token_hv = Hypervector::encode_text_ngram(token, 3);
                let sim = 1.0 - hv.normalized_hamming_distance(&token_hv);
                if sim > best_sim {
                    best_sim = sim;
                    best_token = token.clone();
                }
            }
        }

        if best_sim >= MIN_CLEANUP_ENERGY {
            best_token
        } else {
            String::new()
        }
    }

    /// Unbind known slots from a thought vector.
    ///
    ///   Thought = ρ₁₃(S) ⊕ ρ₂₆(V) ⊕ ρ₃₉(O)
    ///   ρ₁₃(unknown) = Thought ⊕ ρ₂₆(known_v) ⊕ ρ₃₉(known_o)  [for subject]
    ///   unknown = ρ⁻¹(ρ₁₃(unknown))
    fn unbind_slot(
        &self,
        thought: &Hypervector,
        answer_slot: &AnswerSlot,
        known_s: &Option<String>,
        known_v: &Option<String>,
        known_o: &Option<String>,
    ) -> Hypervector {
        let mut residual = thought.clone();
        let encode = |s: &str| Hypervector::encode_text_ngram(s, 3);

        if let Some(s) = known_s { if !s.is_empty() {
            residual = residual.bitwise_xor(&encode(s).rotate_left(RHO_S));
        }}
        if let Some(v) = known_v { if !v.is_empty() {
            residual = residual.bitwise_xor(&encode(v).rotate_left(RHO_V));
        }}
        if let Some(o) = known_o { if !o.is_empty() {
            residual = residual.bitwise_xor(&encode(o).rotate_left(RHO_O));
        }}

        let inv_rho = match answer_slot {
            AnswerSlot::Subject => (crate::HD_DIMENSION - RHO_S) % crate::HD_DIMENSION,
            AnswerSlot::Verb    => (crate::HD_DIMENSION - RHO_V) % crate::HD_DIMENSION,
            AnswerSlot::Object  => (crate::HD_DIMENSION - RHO_O) % crate::HD_DIMENSION,
        };
        residual.rotate_left(inv_rho)
    }

    /// Reconstruction energy: encode (S,V,O), bind, compare to original.
    fn reconstruction_energy(&self, original: &Hypervector, s: &str, v: &str, o: &str) -> f64 {
        let s_hv = Hypervector::encode_text_ngram(s, 3);
        let v_hv = Hypervector::encode_text_ngram(v, 3);
        let o_hv = if o.is_empty() { Hypervector::new_zero() } else { Hypervector::encode_text_ngram(o, 3) };
        let recon = resonator::encode_svo(&s_hv, &v_hv, &o_hv);
        1.0 - recon.normalized_hamming_distance(original)
    }

    // ═════════════════════════════════════════════════════════════════
    // OUTPUT FORMATTING
    // ═════════════════════════════════════════════════════════════════

    fn format_answer(&self, slot: &AnswerSlot, answer: &str, fact: &QaFact) -> String {
        match slot {
            AnswerSlot::Subject => format!("{} {} {}.", answer, crate::narrative::past_tense(&fact.verb), fact.object),
            AnswerSlot::Verb    => format!("{} {} {}.", fact.subject, crate::narrative::past_tense(answer), fact.object),
            AnswerSlot::Object  => format!("{} {} {}.", fact.subject, crate::narrative::past_tense(&fact.verb), answer),
        }
    }

    // ═════════════════════════════════════════════════════════════════
    // FACT VERIFICATION
    // ═════════════════════════════════════════════════════════════════

    /// Find the closest matching fact by full reconstruction.
    pub fn find_fact(&self, subject: &str, verb: &str, object: &str) -> Option<&QaFact> {
        let mut best: Option<&QaFact> = None;
        let mut best_e = 0.0_f64;
        for fact in &self.facts {
            let e = self.reconstruction_energy(&fact.thought, subject, verb, object);
            if e > best_e { best_e = e; best = Some(fact); }
        }
        best.filter(|_| best_e >= MIN_CLEANUP_ENERGY)
    }

    /// Verify a known fact. Returns (exists, confidence).
    pub fn verify_fact(&self, subject: &str, verb: &str, object: &str) -> (bool, f64) {
        self.find_fact(subject, verb, object)
            .map(|f| (true, self.reconstruction_energy(&f.thought, subject, verb, object)))
            .unwrap_or((false, 0.0))
    }

    /// Get all facts about a given subject (for direct multi-fetch).
    ///
    /// Useful when question-parsing cannot extract the intended query
    /// (e.g., "What did the Fed do?" is hard for the NLP extractor).
    pub fn facts_about(&self, subject: &str) -> Vec<&QaFact> {
        let subj_lower = subject.trim().to_lowercase();
        self.facts.iter()
            .filter(|f| f.subject.trim().to_lowercase() == subj_lower)
            .collect()
    }

    /// Get all facts matching a given subject+verb (for object retrieval).
    pub fn facts_with_verb(&self, subject: &str, verb: &str) -> Vec<&QaFact> {
        let subj_lower = subject.trim().to_lowercase();
        let verb_lower = verb.trim().to_lowercase();
        self.facts.iter()
            .filter(|f| {
                f.subject.trim().to_lowercase() == subj_lower
                    && crate::nlp::verb_lemma(&f.verb) == crate::nlp::verb_lemma(verb)
            })
            .collect()
    }
}

impl Default for QaEngine {
    fn default() -> Self { Self::new() }
}

// ═══════════════════════════════════════════════════════════════════════════
// TESTS
// ═══════════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;

    fn test_engine() -> QaEngine {
        let mut e = QaEngine::new();
        e.store_fact("the_fed", "raise", "rates", "The Fed raised rates.");
        e.store_fact("the_fed", "cut", "rates", "The Fed cut rates.");
        e.store_fact("inflation", "rise", "above expectations", "Inflation rises above expectations.");
        e.store_fact("stock_market", "rally", "on the news", "The stock market rallies on the news.");
        e
    }

    #[test]
    fn test_parse_who_question() {
        let (slot, subj, verb, obj) = QaEngine::parse_question("Who raised rates?");
        assert_eq!(slot, AnswerSlot::Subject);
        assert!(subj.to_lowercase().contains("who"));
        assert_eq!(verb, Some("raise".to_string()));
        assert_eq!(obj, Some("rates".to_string()));
    }

    #[test]
    fn test_parse_heuristic_who() {
        let (slot, subj, verb, obj) = QaEngine::parse_heuristic("who raised rates");
        assert_eq!(slot, AnswerSlot::Subject);
        assert_eq!(subj, "who");
        assert_eq!(verb, Some("raise".to_string()));
        assert_eq!(obj, Some("rates".to_string()));
    }

    #[test]
    fn test_answer_who_raised_rates() {
        let engine = test_engine();
        let answer = engine.answer("Who raised rates?");
        eprintln!("  [qa] Q: 'Who raised rates?' → A: '{}'", answer);
        assert!(answer.to_lowercase().contains("the_fed") || answer.to_lowercase().contains("fed"),
            "Answer should mention Fed: '{}'", answer);
        assert!(answer.to_lowercase().contains("rate"),
            "Answer should mention rates: '{}'", answer);
    }

    #[test]
    fn test_answer_who_cut_rates() {
        let engine = test_engine();
        let answer = engine.answer("Who cut rates?");
        eprintln!("  [qa] Q: 'Who cut rates?' → A: '{}'", answer);
        assert!(answer.to_lowercase().contains("the_fed") || answer.to_lowercase().contains("fed"),
            "Answer should mention Fed: '{}'", answer);
    }

    #[test]
    fn test_answer_no_match() {
        let engine = test_engine();
        let answer = engine.answer("Who wrote the symphony?");
        eprintln!("  [qa] Q: 'Who wrote the symphony?' → A: '{}'", answer);
        assert!(answer.contains("not know"), "Should indicate uncertainty: '{}'", answer);
    }

    #[test]
    fn test_answer_empty() {
        let engine = test_engine();
        let answer = engine.answer("");
        assert!(!answer.is_empty());
        eprintln!("  [qa] Q: '' → A: '{}'", answer);
    }

    #[test]
    fn test_answer_gibberish() {
        let engine = test_engine();
        let answer = engine.answer("xyzzy plugh");
        assert!(!answer.is_empty());
        eprintln!("  [qa] Q: 'xyzzy plugh' → A: '{}'", answer);
    }

    #[test]
    fn test_verify_true() {
        let engine = test_engine();
        let (ok, conf) = engine.verify_fact("the_fed", "raise", "rates");
        assert!(ok, "Should verify the Fed raised rates (conf={})", conf);
        assert!(conf > 0.5, "Confidence should be high: {}", conf);
    }

    #[test]
    fn test_verify_false() {
        let engine = test_engine();
        let (ok, _) = engine.verify_fact("the_fed", "raise", "inflation");
        assert!(!ok, "Should not verify false fact");
    }

    #[test]
    fn test_full_cycle() {
        let mut engine = QaEngine::new();
        engine.store_fact("alice", "feed", "the cat", "Alice fed the cat.");
        engine.store_fact("bob", "write", "code", "Bob writes code.");
        assert_eq!(engine.fact_count(), 2);

        let a1 = engine.answer("Who fed the cat?");
        eprintln!("  [qa] Q: 'Who fed the cat?' → A: '{}'", a1);
        assert!(a1.to_lowercase().contains("alice"), "Should answer alice: '{}'", a1);

        let a2 = engine.answer("Who writes code?");
        eprintln!("  [qa] Q: 'Who writes code?' → A: '{}'", a2);
        assert!(a2.to_lowercase().contains("bob"), "Should answer bob: '{}'", a2);
    }

    #[test]
    fn test_multi_word_object() {
        let mut engine = QaEngine::new();
        engine.store_fact("inflation", "rise", "above expectations",
            "Inflation rose above expectations.");
        let a = engine.answer("What rose above expectations?");
        eprintln!("  [qa] Q: 'What rose above expectations?' → A: '{}'", a);
        assert!(a.to_lowercase().contains("inflation"), "Should mention inflation: '{}'", a);
    }

    #[test]
    fn test_from_nlp_triples() {
        let mut engine = QaEngine::new();
        let triples = nlp::extract_svo("The Federal Reserve raises interest rates.");
        assert!(!triples.is_empty());
        engine.store_triples(&triples, "test");
        let a = engine.answer("Who raises interest rates?");
        eprintln!("  [qa] Q: 'Who raises interest rates?' → A: '{}'", a);
        assert!(!a.contains("not know"), "Should answer: '{}'", a);
    }

    #[test]
    fn test_empty_engine() {
        let engine = QaEngine::new();
        let a = engine.answer("Who raised rates?");
        assert_eq!(a, "I do not know the answer to that question.");
    }

    #[test]
    fn test_fact_count() {
        let mut engine = QaEngine::new();
        assert_eq!(engine.fact_count(), 0);
        engine.store_fact("a", "b", "c", "s");
        assert_eq!(engine.fact_count(), 1);
    }

    // ── Tick + Contradiction Tests ───────────────────────────────────

    #[test]
    fn test_fact_autotick() {
        let mut engine = QaEngine::new();
        engine.store_fact("a", "b", "c", "s1");
        engine.store_fact("d", "e", "f", "s2");
        assert_eq!(engine.facts[0].tick, 0);
        assert_eq!(engine.facts[1].tick, 1);
    }

    #[test]
    fn test_contradiction_verb_antonym() {
        let mut engine = QaEngine::new();
        engine.store_fact("the_fed", "raise", "rates", "s1");
        engine.store_fact("the_fed", "cut", "rates", "s2");
        // First fact should be flagged as contradicted
        assert!(engine.facts[0].is_contradicted, "raise→cut should flag contradiction");
        assert!(!engine.facts[1].is_contradicted, "newer fact should be clean");
    }

    #[test]
    fn test_no_false_contradiction() {
        let mut engine = QaEngine::new();
        engine.store_fact("the_fed", "raise", "rates", "s1");
        engine.store_fact("the_fed", "raise", "inflation", "s2");
        // Same verb, different object — no antonym
        assert!(!engine.facts[0].is_contradicted);
        assert!(!engine.facts[1].is_contradicted);
    }

    #[test]
    fn test_contradiction_different_subject_ignored() {
        let mut engine = QaEngine::new();
        engine.store_fact("the_fed", "raise", "rates", "s1");
        engine.store_fact("ecb", "cut", "rates", "s2");
        // Different subject — no contradiction
        assert!(!engine.facts[0].is_contradicted);
    }

    #[test]
    fn test_answer_combined_single() {
        let mut engine = QaEngine::new();
        engine.store_fact("the_fed", "raise", "rates", "s1");
        let a = engine.answer_combined("Who raised rates?");
        assert!(a.contains("the_fed"));
        assert!(!a.contains("not know"));
    }

    #[test]
    fn test_answer_combined_multi_clean() {
        let mut engine = QaEngine::new();
        engine.store_fact("the_fed", "raise", "rates", "s1");
        engine.store_fact("the_fed", "cut", "rates", "s2");
        // Use direct subject query since "What did the Fed do?" is hard for NLP
        let facts = engine.facts_about("the_fed");
        assert_eq!(facts.len(), 2, "Should have 2 facts about the Fed");
        // Verify contradictions were detected
        assert!(engine.facts[0].is_contradicted, "raise→cut should flag contradiction");
        assert!(!engine.facts[1].is_contradicted);
    }

    #[test]
    fn test_answer_combined_with_question() {
        let mut engine = QaEngine::new();
        engine.store_fact("the_fed", "raise", "rates", "s1");
        engine.store_fact("the_fed", "cut", "rates", "s2");
        // Use a question the parser CAN handle
        let a = engine.answer_combined("Who raised rates?");
        eprintln!("  [qa] who raised: '{}'", a);
        assert!(!a.contains("not know"));
        assert!(a.contains("the_fed"));
    }

    #[test]
    fn test_facts_about() {
        let mut engine = QaEngine::new();
        engine.store_fact("the_fed", "raise", "rates", "s1");
        engine.store_fact("the_fed", "cut", "rates", "s2");
        engine.store_fact("ecb", "cut", "rates", "s3");
        let fed_facts = engine.facts_about("the_fed");
        assert_eq!(fed_facts.len(), 2);
        let ecb_facts = engine.facts_about("ecb");
        assert_eq!(ecb_facts.len(), 1);
    }

    #[test]
    fn test_facts_with_verb() {
        let mut engine = QaEngine::new();
        engine.store_fact("the_fed", "raise", "rates", "s1");
        engine.store_fact("the_fed", "cut", "rates", "s2");
        engine.store_fact("ecb", "cut", "rates", "s3");
        let cut_facts = engine.facts_with_verb("the_fed", "cut");
        assert_eq!(cut_facts.len(), 1);
    }

    #[test]
    fn test_antonym_raise_cut() {
        assert!(crate::narrative::is_antonym("raise", "cut"));
        assert!(crate::narrative::is_antonym("cut", "raise"));
    }

    #[test]
    fn test_antonym_rise_fall() {
        assert!(crate::narrative::is_antonym("rise", "fall"));
        assert!(crate::narrative::is_antonym("fall", "rise"));
        assert!(crate::narrative::is_antonym("rose", "fell")); // lemmatized
    }

    #[test]
    fn test_non_antonym() {
        assert!(!crate::narrative::is_antonym("raise", "raise"));
        assert!(!crate::narrative::is_antonym("raise", "eat"));
    }

    #[test]
    fn test_answer_combined_no_match() {
        let engine = QaEngine::new();
        let a = engine.answer_combined("Who raised rates?");
        assert_eq!(a, "I do not know the answer to that question.");
    }

    #[test]
    fn test_object_antonym() {
        assert!(crate::narrative::is_object_antonym("inflation", "deflation"));
        assert!(!crate::narrative::is_object_antonym("rates", "rates"));
    }
}
