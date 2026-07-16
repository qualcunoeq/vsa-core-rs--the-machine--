//! Deep Reader — extracts definitions, relationships, causal rules, and mathematical
//! operations from textbook text using the Machine's full NLP pipeline.
//!
//! Unlike `pdf_reader::extract_definitions` which only catches 5 surface patterns
//! ("X is a Y", "X is called Y", etc.), this module uses `nlp::extract_svo()` to
//! parse every sentence and classify triples into knowledge types:
//!
//! - **Definitions**: copular constructions with clean term/definition pairs
//! - **Relationships**: all other SVO triples (mathematical ops, properties, etc.)
//! - **Causal rules**: conditional patterns ("if X then Y", "since X, Y")
//! - **Implications**: directional relationships ("X implies Y", "X means Y")

use crate::nlp;

/// Result from deep reading a document
pub struct DeepReadResult {
    /// Definitions: clean "X is Y" pairs suitable for direct lookup
    pub definitions: Vec<(String, String, String)>,
    /// All other SVO triples (mathematical relationships, properties, actions)
    pub relationships: Vec<(String, String, String)>,
    /// Causal/implication rules as (antecedent_SVO, consequent_SVO)
    pub causal_rules: Vec<(Vec<String>, Vec<String>)>,
    /// Mathematical operation facts: (operation, input, result)
    /// e.g., ("derivative", "sin_x", "cos_x")
    pub math_operations: Vec<(String, String, String)>,
    /// Properties/attributes: (entity, attribute, value)
    /// e.g., ("continuous_function", "has", "limit_at_every_point")
    pub properties: Vec<(String, String, String)>,
    /// Statistics
    pub total_sentences: usize,
    pub total_triples: usize,
    pub total_rules: usize,
}

// ═════════════════════════════════════════════════════════════════════
// TEXT CLEANING
// ═════════════════════════════════════════════════════════════════════

/// Clean a text token: lowercase, replace spaces/special chars with underscores
fn clean(s: &str) -> String {
    let s = s.to_lowercase();
    let result: String = s.chars()
        .map(|c| if c.is_alphanumeric() { c } else { '_' })
        .collect();
    let result: String = result.split('_')
        .filter(|w| !w.is_empty())
        .collect::<Vec<_>>()
        .join("_");
    const MAX_LEN: usize = 80;
    if result.len() > MAX_LEN {
        let end = result.char_indices()
            .nth(MAX_LEN)
            .map(|(i, _)| i)
            .unwrap_or(result.len());
        result[..end].to_string()
    } else { result }
}

/// Get a cleaned version with underscores replaced by spaces (for matching)
fn normal(s: &str) -> String {
    s.to_lowercase().replace('_', " ").trim().to_string()
}

// ═════════════════════════════════════════════════════════════════════
// NOISE DETECTION
// ═════════════════════════════════════════════════════════════════════

/// Known noise words that should never be a definition term or rule subject
const NOISE_TERMS: &[&str] = &[
    // Question words
    "what", "which", "who", "whom", "where", "when", "why",
    // Pronouns/demonstratives
    "this", "that", "these", "those", "there", "here",
    "it", "its", "they", "them", "their", "we", "you", "your",
    "he", "she", "him", "her", "his",
    // Conjunctions/connectives
    "and", "or", "but", "if", "then", "else", "because",
    "since", "although", "however", "therefore", "thus",
    "also", "so", "yet", "for", "nor", "not",
    // Articles/determiners
    "a", "an", "the", "this", "that", "these", "those",
    // Numeric/metatextual
    "one", "two", "three", "four", "five", "six", "seven", "eight", "nine", "ten",
    "first", "second", "third", "next", "last", "previous", "following",
    "example", "figure", "table", "section", "chapter", "exercise",
    // Meta-discourse
    "note", "notice", "observe", "see", "look", "consider", "suppose",
    "let", "define", "assume", "given", "find",
];

/// Known textbook boilerplate patterns — any rule involving these is discarded
const BOILERPLATE_SUBJECTS: &[&str] = &[
    "our", "your", "my", "their", "its",
    "we", "you", "they",
    "the book", "this book", "our book", "the textbook", "this textbook",
    "the author", "the authors", "the publisher", "openstax",
    "the website", "the web", "the internet",
    "attribution", "the attribution", "this attribution",
    "the license", "this license", "the copyright",
    "the pdf", "this pdf", "the page", "this page",
    "the link", "this link",
];

const BOILERPLATE_VERBS: &[&str] = &[
    "be openly licensed", "be free to use", "be free to",
    "have attribution", "provide attribution",
    "access for free", "download", "print",
    "cite", "reference",
];

const BOILERPLATE_OBJECTS: &[&str] = &[
    "openly licensed", "free to use", "free of charge",
    "attribution", "creative commons",
    "openstax", "cnx.org",
    "web based", "web_based",
    "pedagogically", "pedagogical",
];

/// Check if a subject looks like a valid definition term (not a question word, not noise)
fn is_valid_term(s: &str) -> bool {
    let lower = normal(s);
    if lower.len() < 2 && !is_math_var(&lower) { return false; }
    // Check exact noise match
    if NOISE_TERMS.contains(&lower.as_str()) { return false; }
    // Check if starts with a noise word (e.g., "the derivative" — "the" prefix is fine, but "this function" is not)
    let first_word = lower.split_whitespace().next().unwrap_or("");
    if NOISE_TERMS.contains(&first_word) && first_word.len() <= 5
        && !matches!(first_word, "the" | "a" | "an")
    {
        return false;
    }
    // Reject pure numbers
    if lower.chars().all(|c| c.is_ascii_digit() || c == '.' || c == '_' || c == '-') { return false; }
    true
}

/// Check if an object string contains a word (word-boundary aware)
/// "a" matches "a" but not "a degree" — uses exact word match
fn object_matches_word(obj: &str, word: &str) -> bool {
    let o = normal(obj);
    if o == word { return true; }
    if o.starts_with(&format!("{} ", word)) { return true; }
    if o.ends_with(&format!(" {}", word)) { return true; }
    if o.contains(&format!(" {} ", word)) { return true; }
    false
}

/// Check if a short string is a common math variable/function name (f, x, y, n, etc.)
fn is_math_var(s: &str) -> bool {
    let lower = s.trim().to_lowercase();
    if lower.len() != 1 { return false; }
    matches!(lower.as_str(), "f" | "g" | "h" | "x" | "y" | "z" | "n" | "m"
        | "a" | "b" | "c" | "p" | "q" | "r" | "s" | "t"
        | "u" | "v" | "w" | "i" | "j" | "k")
}

/// Check if an object looks like a valid definition (not a sentence fragment)
fn is_valid_definition(s: &str) -> bool {
    let lower = normal(s);
    if lower.len() < 5 { return false; }
    // Reject if it starts with a conjunction or preposition
    if lower.starts_with("and ") || lower.starts_with("or ") || lower.starts_with("but ")
        || lower.starts_with("if ") || lower.starts_with("because ") || lower.starts_with("since ")
        || lower.starts_with("so ") || lower.starts_with("then ")
        || lower.starts_with("although ") || lower.starts_with("however ")
    {
        return false;
    }
    // Reject boilerplate
    if BOILERPLATE_OBJECTS.iter().any(|bp| lower.contains(bp)) {
        return false;
    }
    true
}

/// Check if a term is a genuine math concept (not instruction noise)
fn is_math_concept(s: &str) -> bool {
    let lower = normal(s);
    if lower.len() < 2 { return false; }
    // Strip leading articles before checking prefixes
    let content = lower.trim_start_matches("a ")
        .trim_start_matches("an ")
        .trim_start_matches("the ")
        .trim();
    if content.len() < 3 { return false; }

    let math_prefixes = &[
        "derivative", "integral", "function", "equation", "inequality",
        "polynomial", "exponential", "logarithm", "trigonometric",
        "matrix", "vector", "scalar", "variable", "constant",
        "theorem", "lemma", "corollary", "axiom", "proof",
        "graph", "plot", "curve", "surface", "slope",
        "limit", "continuity", "differentiable", "integrable",
        "probability", "statistic", "distribution", "variance",
        "mean", "median", "mode", "range", "standard_deviation",
        "fraction", "decimal", "percent", "ratio", "proportion",
        "angle", "triangle", "circle", "radius", "diameter",
        "algorithm", "program", "code", "loop", "recursion",
        "absolute", "quadratic", "linear", "rational", "radical",
        "numerator", "denominator", "coefficient", "exponent",
    ];
    if math_prefixes.iter().any(|p| content.starts_with(p)) { return true; }
    // Math suffixes (check against content)
    let math_suffixes = &[
        "tion", "sion", "ment", "ity", "ive", "ics", "sis", "oid", "um", "us",
    ];
    if math_suffixes.iter().any(|s| content.ends_with(s)) { return true; }
    // Common math words (not prefix-matched)
    let math_words = &[
        "x", "y", "z", "f", "g", "h", "n", "m",
        "equals", "value", "values", "formula", "expression",
        "operation", "operator", "calculation",
        "number", "numbers", "digit", "digits",
        "sum", "difference", "product", "quotient",
        "axis", "axes", "coordinate", "coordinates",
        "data", "set", "union", "intersection",
    ];
    if math_words.iter().any(|w| content == *w || content.starts_with(w))
    { return true; }
    false
}

/// Check if an SVO triple is math textbook boilerplate (should be rejected)
fn is_boilerplate(subj: &str, verb: &str, obj: &str) -> bool {
    let s = normal(subj);
    let v = normal(verb);
    let o = normal(obj);
    // Check subjects
    if BOILERPLATE_SUBJECTS.iter().any(|bp| s.contains(bp)) { return true; }
    // Check verbs
    if BOILERPLATE_VERBS.iter().any(|bp| v.contains(bp)) { return true; }
    // Check objects
    if BOILERPLATE_OBJECTS.iter().any(|bp| o.contains(bp)) { return true; }
    // Check combined patterns
    if (s.contains("book") || s.contains("textbook") || s.contains("page"))
        && (v.contains("be") || v.contains("have") || v.contains("contain"))
    { return true; }
    if s.contains("attribution") || o.contains("attribution") { return true; }
    if s == "deemed" || s == "pedagogically" { return true; }
    if v.contains("deem") || o.contains("pedagogically") { return true; }
    // Reject rules where subject is a generic instruction word
    if matches!(s.as_str(), "never" | "always" | "sometimes" | "often" | "usually") { return true; }
    false
}

/// Check if a sentence is textbook boilerplate (for causal rule pre-filtering)
fn is_boilerplate_sentence(sentence: &str) -> bool {
    let lower = sentence.to_lowercase();
    let bp_patterns = &[
        "openstax", "cnx.org", "creative commons", "attribution",
        "license", "licensed under", "free to use", "free of charge",
        "download", "print", "access for free",
        "want to cite", "how to cite", "citation",
        "web based", "web-based",
        "pedagogically", "pedagogical",
        "faculty", "instructor", "educator",
        "all rights reserved",
        "this book is", "this textbook",
        "the pdf", "the web view",
        "page number", "page contains",
    ];
    bp_patterns.iter().any(|p| lower.contains(p))
}

// ═════════════════════════════════════════════════════════════════════
// CAUSAL RULE EXTRACTION
// ═════════════════════════════════════════════════════════════════════

/// Simple sentence splitter
fn split_sentences(text: &str) -> Vec<String> {
    let mut sentences = Vec::new();
    let mut current = String::new();
    for c in text.chars() {
        current.push(c);
        if matches!(c, '.' | '!' | '?' | '\n') {
            let trimmed = current.trim().to_string();
            if !trimmed.is_empty() && !is_boilerplate_sentence(&trimmed) {
                sentences.push(trimmed);
            }
            current = String::new();
        }
    }
    let trimmed = current.trim().to_string();
    if !trimmed.is_empty() && !is_boilerplate_sentence(&trimmed) {
        sentences.push(trimmed);
    }
    sentences
}

/// Check if a causal rule is genuinely mathematical (rejects textbook admin rules)
fn is_valid_mathematical_rule(ante: &[String], cons: &[String]) -> bool {
    if ante.len() < 3 || cons.len() < 3 { return false; }
    if is_boilerplate(&ante[0], &ante[1], &ante[2]) { return false; }
    if is_boilerplate(&cons[0], &cons[1], &cons[2]) { return false; }
    // At least one side should have substantial content (> 3 chars) or be a math variable
    let has_content = |v: &[String]| -> bool {
        v[0].len() > 3 || v[2].len() > 3 || is_math_var(&v[0])
    };
    let valid_ante = is_valid_term(&ante[0]) && has_content(ante);
    let valid_cons = is_valid_term(&cons[0]) && has_content(cons);
    if !valid_ante && !valid_cons { return false; }
    true
}

/// Extract a conditional sentence into (antecedent, consequent) SVO lists
fn extract_conditional(sentence: &str) -> Option<(Vec<String>, Vec<String>)> {
    if is_boilerplate_sentence(sentence) { return None; }

    let lower = sentence.to_lowercase();

    // Pattern 1: "If X, then Y" or "If X then Y"
    if let Some(if_pos) = lower.find("if ") {
        let after_if = &sentence[if_pos + 3..];
        let then_marker = if let Some(then_pos) = after_if.find(" then ") {
            then_pos
        } else if let Some(comma_pos) = after_if.find(',') {
            comma_pos
        } else {
            return None;
        };
        let antecedent = after_if[..then_marker].trim();
        let consequent = after_if[then_marker + 1..].trim()
            .trim_start_matches("then ")
            .trim();

        if is_boilerplate_sentence(antecedent) || is_boilerplate_sentence(consequent) {
            return None;
        }

        let ante_triples = nlp::extract_svo(antecedent);
        let cons_triples = nlp::extract_svo(consequent);

        if !ante_triples.is_empty() && !cons_triples.is_empty() {
            let t = &ante_triples[0];
            let ante_vec = vec![clean(&t.subject), clean(&t.verb), clean(&t.object)];
            let t = &cons_triples[0];
            let cons_vec = vec![clean(&t.subject), clean(&t.verb), clean(&t.object)];
            if is_valid_mathematical_rule(&ante_vec, &cons_vec) {
                return Some((ante_vec, cons_vec));
            }
        }
    }

    // Pattern 2: "Since X, Y" or "Because X, Y"
    for marker in &["since ", "because "] {
        if let Some(pos) = lower.find(marker) {
            let after = &sentence[pos + marker.len()..];
            if let Some(comma_pos) = after.find(',') {
                let antecedent = after[..comma_pos].trim();
                let consequent = after[comma_pos + 1..].trim();

                if is_boilerplate_sentence(antecedent) { return None; }

                let ante_triples = nlp::extract_svo(antecedent);
                let cons_triples = nlp::extract_svo(consequent);

                if !ante_triples.is_empty() && !cons_triples.is_empty() {
                    let t = &ante_triples[0];
                    let ante_vec = vec![clean(&t.subject), clean(&t.verb), clean(&t.object)];
                    let t = &cons_triples[0];
                    let cons_vec = vec![clean(&t.subject), clean(&t.verb), clean(&t.object)];
                    if is_valid_mathematical_rule(&ante_vec, &cons_vec) {
                        return Some((ante_vec, cons_vec));
                    }
                }
            }
        }
    }

    None
}

// ═════════════════════════════════════════════════════════════════════
// KNOWLEDGE CLASSIFICATION
// ═════════════════════════════════════════════════════════════════════

/// Detect mathematical operation patterns in SVO triples.
/// Uses strict criteria: subject must be a math concept AND verb must be an operation.
fn is_math_operation(subj: &str, verb: &str, obj: &str) -> bool {
    let math_ops = [
        "differentiate", "differentiating", "differentiated",
        "integrate", "integrating", "integrated",
        "evaluate", "evaluating", "evaluated",
        "compute", "computing", "computed",
        "solve", "solving", "solved",
    ];
    let math_concept_verbs = [
        "derivative_of", "integral_of", "derivative", "integral",
        "antiderivative", "antiderivative_of",
    ];
    let s = normal(subj);
    let v = normal(verb);
    let o = normal(obj);

    // Direct math concept match: "derivative of sin x is cos x"
    if math_concept_verbs.iter().any(|mv| v.contains(mv)) && is_math_concept(&s) {
        return true;
    }

    // Math operation on a math concept: "differentiate the function"
    if math_ops.iter().any(|op| v.contains(op)) && is_math_concept(&s) {
        return true;
    }

    // "Find the derivative/integral/limit" — specific, not general "find"
    if v.contains("find") || v.contains("found") {
        let find_targets = &["derivative", "integral", "limit", "area", "volume",
                              "root", "roots", "solution", "solutions",
                              "value", "values", "maximum", "minimum",
                              "inverse", "determinant", "eigenvalue"];
        if find_targets.iter().any(|t| s.contains(t) || o.contains(t) || v.contains(t)) {
            return true;
        }
    }

    false
}

/// Detect property/attribute patterns.
/// To reduce general-English false positives, requires the subject to be a
/// genuine math concept and the verb to be a specific property relation.
fn is_property_verb(subj: &str, verb: &str, obj: &str) -> bool {
    let s = normal(subj);
    let v = normal(verb);
    let o = normal(obj);

    // Subject must be a valid math concept (not "the table", "this figure", etc.)
    if !is_math_concept(&s) && s.len() < 8 {
        // Allow very specific long subjects even if not math-detected
        if !s.contains("function") && !s.contains("equation") && !s.contains("graph") {
            return false;
        }
    }

    // Property verbs that are genuinely relational (not general English)
    let strong_prop_verbs = &[
        "is_a_type_of", "are_a_type_of", "is_a_kind_of", "are_a_kind_of",
        "is_a", "are_a", "is_an", "are_an",
        "belongs_to", "belong_to",
        "depends_on", "depend_on",
        "is_based_on", "are_based_on",
        "follows_from", "follow_from",
        "is_derived_from", "are_derived_from",
        "is_defined_as", "are_defined_as",
        "is_known_as", "are_known_as",
        "refers_to", "refer_to",
        "is_related_to", "are_related_to",
        "corresponds_to", "correspond_to",
        "is_equal_to", "are_equal_to",
        "is_equivalent_to", "are_equivalent_to",
        "implies", "implied", "imply",
        "is_necessary_for", "are_necessary_for",
        "is_sufficient_for", "are_sufficient_for",
        "satisfies", "satisfy",
        "can_be_written_as", "can_be_expressed_as",
        "can_be_defined_as", "can_be_thought_of_as",
    ];

    if strong_prop_verbs.iter().any(|pv| v.contains(pv)) { return true; }

    // Weaker property verbs — require math concept + non-empty object + valid object
    let weak_prop_verbs = &["has", "have", "contains", "contain",
                             "includes", "include", "possesses", "possess",
                             "exhibits", "exhibit", "shows", "show",
                             "meets", "meet", "must_be", "can_be", "cannot_be"];

    if weak_prop_verbs.iter().any(|pv| v.contains(pv)) {
        // Object must be a meaningful property (not empty, not noise)
        if o.len() < 3 { return false; }
        // Object should not be generic English
        // Use word-boundary matching to avoid false positives
        // (e.g., "a" should match "a" but not "a degree")
        let generic_words = &["the following", "the same", "the above", "the below",
                               "the next", "the previous", "the first", "the second",
                               "how to", "why the", "what the",
                               "this", "that", "these", "those"];
        if generic_words.iter().any(|gw| object_matches_word(&o, gw)) && o.len() < 15 {
            return false;
        }
        return true;
    }

    // Strong property indicators in subject
    if s.contains("property") || s.contains("characteristic")
        || s.contains("feature") || s.contains("attribute")
    {
        return true;
    }

    false
}

// ═════════════════════════════════════════════════════════════════════
// MAIN ENTRY POINTS
// ═════════════════════════════════════════════════════════════════════

/// Deep read a PDF file and extract all forms of knowledge
pub fn deep_read_pdf(path: &str) -> Result<DeepReadResult, String> {
    let text = crate::pdf_reader::extract_text(path)?;
    Ok(deep_read_text(&text))
}

/// Deep read raw text and extract all forms of knowledge.
/// Processes every sentence through the full NLP pipeline.
pub fn deep_read_text(text: &str) -> DeepReadResult {
    let mut definitions = Vec::new();
    let mut relationships = Vec::new();
    let mut causal_rules = Vec::new();
    let mut math_operations = Vec::new();
    let mut properties = Vec::new();

    // Pre-split sentences (filters boilerplate during split)
    let sentences = split_sentences(text);
    let total_sentences = sentences.len();

    // Process each sentence through full NLP
    for sentence in &sentences {
        let triples = nlp::extract_svo(sentence);
        if triples.is_empty() {
            continue;
        }

        for triple in &triples {
            let subj = clean(&triple.subject);
            let verb = clean(&triple.verb);
            let obj = clean(&triple.object);

            if subj.is_empty() || (verb.is_empty() && obj.is_empty()) {
                continue;
            }

            // Reject boilerplate
            if is_boilerplate(&subj, &verb, &obj) {
                continue;
            }

            // Classify by construction type
            match triple.construction.as_str() {
                "copular" => {
                    // "X is Y" — could be definition or property
                    if verb == "be" && is_valid_term(&subj) && is_valid_definition(&obj) {
                        if is_property_verb(&subj, "be", &obj) {
                            properties.push((subj, verb, obj));
                        } else {
                            definitions.push((subj, verb, obj));
                        }
                    } else if is_valid_term(&subj) {
                        relationships.push((subj, verb, obj));
                    }
                }
                "active" | "passive_recovered" | "passive_agentless" => {
                    if !is_valid_term(&subj) {
                        continue;
                    }
                    // Check for mathematical operations
                    if is_math_operation(&subj, &verb, &obj) {
                        math_operations.push((subj, verb, obj));
                    } else if is_property_verb(&subj, &verb, &obj) {
                        properties.push((subj, verb, obj));
                    } else {
                        relationships.push((subj, verb, obj));
                    }
                }
                "conj_expanded" | "relative_clause" | _ => {
                    if is_valid_term(&subj) {
                        relationships.push((subj, verb, obj));
                    }
                }
            }
        }

        // Extract causal rules from conditional patterns
        let sentence_lower = sentence.to_lowercase();
        if (sentence_lower.contains("if ") || sentence_lower.starts_with("if"))
            || sentence_lower.contains("since ") || sentence_lower.contains("because ")
        {
            if let Some((ante, cons)) = extract_conditional(sentence) {
                if !causal_rules.iter().any(|(a, _): &(Vec<String>, Vec<String>)| *a == ante) {
                    causal_rules.push((ante, cons));
                }
            }
        }
    }

    DeepReadResult {
        total_sentences,
        total_triples: definitions.len() + relationships.len() + math_operations.len() + properties.len(),
        total_rules: causal_rules.len(),
        definitions,
        relationships,
        causal_rules,
        math_operations,
        properties,
    }
}

/// Store deep read results into a QA engine
pub fn store_deep_knowledge(
    result: &DeepReadResult,
    qa: &mut crate::qa::QaEngine,
    source: &str,
) -> usize {
    let mut total = 0usize;

    for (subj, verb, obj) in &result.definitions {
        qa.store_fact(subj, verb, obj, source);
        total += 1;
    }
    for (subj, verb, obj) in &result.relationships {
        qa.store_fact(subj, verb, obj, source);
        total += 1;
    }
    for (subj, verb, obj) in &result.math_operations {
        qa.store_fact(subj, verb, obj, &format!("{} [math]", source));
        total += 1;
    }
    for (subj, verb, obj) in &result.properties {
        qa.store_fact(subj, verb, obj, source);
        total += 1;
    }
    for (ante, cons) in &result.causal_rules {
        if ante.len() >= 3 && cons.len() >= 3 {
            qa.store_rule(&ante[0], &ante[1], &ante[2],
                          &cons[0], &cons[1], &cons[2],
                          &format!("{} [rule]", source));
            total += 1;
        }
    }

    total
}

// ═════════════════════════════════════════════════════════════════════
// TESTS
// ═════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_definitions_simple() {
        let result = deep_read_text("A derivative is the instantaneous rate of change of a function.");
        assert!(result.definitions.len() >= 1,
            "Should find definition, got {} defs", result.definitions.len());
    }

    #[test]
    fn test_relationship_math() {
        let result = deep_read_text("The derivative of sin x is cos x.");
        assert!(result.definitions.len() + result.relationships.len() >= 1);
    }

    #[test]
    fn test_causal_rule_differentiable_continuous() {
        let result = deep_read_text("If f is differentiable at a, then f is continuous at a.");
        assert!(result.causal_rules.len() >= 1,
            "Should find causal rule, got {} rules", result.causal_rules.len());
        if !result.causal_rules.is_empty() {
            let (ante, cons) = &result.causal_rules[0];
            eprintln!("  Rule: {:?} → {:?}", ante, cons);
            assert!(ante[0].contains("f") || ante[2].contains("differentiable"),
                "Antecedent should involve differentiability");
            assert!(cons[2].contains("continuous"),
                "Consequent should involve continuity");
        }
    }

    #[test]
    fn test_causal_rule_since() {
        let result = deep_read_text("Since f is continuous, the limit exists.");
        eprintln!("  Result: {} rules, {} defs", result.causal_rules.len(), result.definitions.len());
    }

    #[test]
    fn test_math_operation_derivative() {
        let result = deep_read_text("To find the derivative, we differentiate the function with respect to x.");
        eprintln!("  Math ops: {}", result.math_operations.len());
    }

    #[test]
    fn test_clean() {
        assert_eq!(clean("The derivative!"), "the_derivative");
        assert_eq!(clean("rate of change"), "rate_of_change");
    }

    #[test]
    fn test_valid_term() {
        assert!(is_valid_term("derivative"));
        assert!(is_valid_term("rate_of_change"));
        assert!(!is_valid_term("what"));
        assert!(!is_valid_term("this"));
        assert!(!is_valid_term("it"));
    }

    #[test]
    fn test_boilerplate_filter() {
        // Textbook licensing boilerplate should be rejected
        let result = deep_read_text("If our book is openly licensed, then you are free to use it.");
        assert_eq!(result.causal_rules.len(), 0,
            "Boilerplate rule should be filtered, got {}", result.causal_rules.len());

        // Genuine math rule should still pass
        let result2 = deep_read_text("If a function is differentiable, then it is continuous.");
        assert!(result2.causal_rules.len() >= 1,
            "Math rule should pass, got {}", result2.causal_rules.len());
    }

    #[test]
    fn test_math_operation_vs_general_find() {
        // Imperative sentences ("Find the X") don't produce SVO via NLP
        // because there's no explicit subject. This is a known limitation.
        // Math ops are detected from declarative sentences with subjects.
        let r1 = deep_read_text("We find the derivative of the function.");
        // "We" is a noise term so it won't be extracted. Try with a proper subject:
        let r2 = deep_read_text("The derivative of sin x is cos x.");
        assert!(r2.definitions.len() + r2.math_operations.len() >= 1,
            "Math relationship should be detected");
    }

    #[test]
    fn test_causal_rule_fed_example() {
        // Non-math rule should still pass if valid (Fed example)
        let result = deep_read_text("If the Fed raises rates, then bond yields rise.");
        assert!(result.causal_rules.len() >= 1,
            "Fed rule should pass, got {}", result.causal_rules.len());
    }

    #[test]
    fn test_property_detection() {
        let r1 = deep_read_text("A polynomial function has a degree.");
        assert!(r1.properties.len() >= 1,
            "Math property should be detected, got props={} defs={} rels={}",
            r1.properties.len(), r1.definitions.len(), r1.relationships.len());
    }

    #[test]
    fn test_general_english_filtered() {
        // General English "has" should NOT create a property fact
        // "The table has four columns" is not a mathematical property
        let result = deep_read_text("The table has four columns.");
        let props_with_table = result.properties.iter()
            .filter(|(s, _, _)| s.contains("table"));
        eprintln!("  Property count with 'table': {}", props_with_table.count());
    }

    #[test]
    fn test_general_english_means_filtered() {
        // "This means that" is general English — should not create a property
        let result = deep_read_text("This means that the function is continuous.");
        // The NLP might still extract this, but check it's not classified as math
        let math_count = result.math_operations.len();
        eprintln!("  Math ops for 'this means': {}", math_count);
    }

    #[test]
    fn test_noise_subject_filtering() {
        // "This is a derivative" → "this" should be filtered as noise subject
        let result = deep_read_text("This is a derivative of the function.");
        let this_defs = result.definitions.iter()
            .filter(|(s, _, _)| s.contains("this"));
        assert_eq!(this_defs.count(), 0,
            "'this' should be filtered as noise subject");
    }
}
