//! Independent semantic-fidelity measurement for worker proposals.
//!
//! `semantic_ir::validate_candidate` answers a structural question: are the
//! metadata, spans, scopes, and relation-symbol closure well formed?  It
//! deliberately does **not** answer whether an equation faithfully represents
//! the source sentence.  A proposal can be structurally perfect and still
//! reverse a relationship, flip a sign, drop a condition, or invent an
//! equation whose evidence spans happen to be valid substrings.
//!
//! This module measures that second question against a human-reviewed gold
//! interpretation.  It is a shadow measurement: it never authorizes a solver,
//! registry mutation, or answer.  Structural validity is reported separately
//! from fidelity so the two cannot be conflated.

use crate::semantic_ir::{CandidateSemanticParse, EvidenceSpan, RelationIR};
use serde::{Deserialize, Serialize};

pub const SEMANTIC_FIDELITY_SCHEMA: &str = "semantic-fidelity-gold-v1";

/// A concrete way an interpretation can be unfaithful to its source.
///
/// The variants map one-to-one onto the evaluation-set categories so a miss is
/// reported by cause instead of as a single opaque "wrong" count.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FaithfulnessCategory {
    /// The interpretation matches the source.
    Correct,
    /// A relationship is inverted, e.g. "Alice has 3 more than Bob" read as
    /// Bob having more than Alice.
    ReversedRelationship,
    /// A sign is wrong relative to the source, e.g. `+` read as `-`.
    WrongSign,
    /// A quantity does not match the source.
    WrongQuantity,
    /// A condition present in the source is absent from the interpretation.
    MissingCondition,
    /// An equation is present with valid source spans but no support in the
    /// span text: structurally valid, semantically invented.
    InventedEquationValidSpan,
    /// More than one reading of the source is plausible and the proposal
    /// commits to exactly one without saying so.
    MultiplePlausible,
    /// The source is outside the labelled domains and the proposal commits to
    /// an interpretation anyway.
    UnsupportedDomain,
}

impl FaithfulnessCategory {
    pub fn label(self) -> &'static str {
        match self {
            Self::Correct => "correct",
            Self::ReversedRelationship => "reversed_relationship",
            Self::WrongSign => "wrong_sign",
            Self::WrongQuantity => "wrong_quantity",
            Self::MissingCondition => "missing_condition",
            Self::InventedEquationValidSpan => "invented_equation_valid_span",
            Self::MultiplePlausible => "multiple_plausible",
            Self::UnsupportedDomain => "unsupported_domain",
        }
    }

    /// True when the interpretation departs from the source at all.
    pub fn is_infidelity(self) -> bool {
        !matches!(self, Self::Correct)
    }
}

/// A gold relation: the faithful equation and the symbols it actually uses.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GoldRelation {
    pub expression: String,
    pub symbols: Vec<String>,
}

/// Human-reviewed expectation for one source prompt.
///
/// `faithful_relations` is the set the interpretation must reproduce exactly;
/// `distractor_relations` are known plausible-but-wrong equations (the
/// reversed, sign-flipped, or quantity-altered forms).  `required_conditions`
/// are surface fragments the interpretation must retain.  The gold case is
/// answer-key-bearing by construction and is only read by the evaluator.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FidelityGoldCase {
    pub id: String,
    pub prompt: String,
    pub domain: String,
    pub faithful_relations: Vec<GoldRelation>,
    pub distractor_relations: Vec<GoldRelation>,
    pub required_conditions: Vec<String>,
    pub forbidden_relations: Vec<String>,
    pub expected_category: FaithfulnessCategory,
    /// True when the faithful behavior is to ask which reading is intended.
    pub clarification_expected: bool,
    /// True when the source is outside the labelled domains.
    pub unsupported_expected: bool,
}

impl FidelityGoldCase {
    pub fn validation_errors(&self) -> Vec<String> {
        let mut errors = Vec::new();
        if self.id.trim().is_empty() {
            errors.push("id is empty".to_string());
        }
        if self.prompt.trim().is_empty() {
            errors.push("prompt is empty".to_string());
        }
        if self.domain.trim().is_empty() {
            errors.push("domain is empty".to_string());
        }
        match self.expected_category {
            FaithfulnessCategory::Correct | FaithfulnessCategory::MultiplePlausible => {}
            FaithfulnessCategory::UnsupportedDomain => {
                if !self.unsupported_expected {
                    errors.push("unsupported_domain must set unsupported_expected".to_string());
                }
            }
            _ => {
                if self.faithful_relations.is_empty() {
                    errors.push("faithful_relations must contain at least one relation".to_string());
                }
            }
        }
        if self.clarification_expected && self.distractor_relations.is_empty() {
            errors.push("clarification_expected requires a distractor relation".to_string());
        }
        errors
    }

    pub fn is_valid(&self) -> bool {
        self.validation_errors().is_empty()
    }

    fn faithful_expressions(&self) -> Vec<&str> {
        self.faithful_relations
            .iter()
            .map(|relation| relation.expression.as_str())
            .collect()
    }

    fn distractor_expressions(&self) -> Vec<&str> {
        self.distractor_relations
            .iter()
            .map(|relation| relation.expression.as_str())
            .collect()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FidelityCorpus {
    pub schema_version: u32,
    pub cases: Vec<FidelityGoldCase>,
}

impl FidelityCorpus {
    pub const CURRENT_SCHEMA_VERSION: u32 = 1;

    pub fn validation_errors(&self) -> Vec<String> {
        let mut errors = Vec::new();
        if self.schema_version != Self::CURRENT_SCHEMA_VERSION {
            errors.push(format!(
                "unsupported schema_version {}; expected {}",
                self.schema_version,
                Self::CURRENT_SCHEMA_VERSION
            ));
        }
        let mut ids = std::collections::BTreeSet::new();
        for (index, case) in self.cases.iter().enumerate() {
            for error in case.validation_errors() {
                errors.push(format!("case[{index}] {}: {error}", case.id));
            }
            if !ids.insert(case.id.clone()) {
                errors.push(format!("duplicate case id: {}", case.id));
            }
        }
        errors
    }

    pub fn is_valid(&self) -> bool {
        self.validation_errors().is_empty()
    }
}

/// The fidelity verdict for one proposal against one gold case.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FidelityVerdict {
    /// The interpretation reproduces the faithful relations and conditions.
    Faithful,
    /// The interpretation departs from the source in a named way.
    Infidelity {
        category: FaithfulnessCategory,
        detail: String,
    },
    /// The interpretation is structurally unusable, so fidelity is not even
    /// reached.  Kept distinct from infidelity so structural failure is not
    /// counted as a semantic miss.
    StructurallyUnusable { diagnostics: Vec<String> },
}

impl FidelityVerdict {
    pub fn is_faithful(&self) -> bool {
        matches!(self, Self::Faithful)
    }

    pub fn is_infidelity(&self) -> bool {
        matches!(self, Self::Infidelity { .. })
    }

    pub fn category(&self) -> Option<FaithfulnessCategory> {
        match self {
            Self::Infidelity { category, .. } => Some(*category),
            _ => None,
        }
    }
}

fn relation_expression(relation: &RelationIR) -> String {
    relation.expression.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn span_supports(span: &EvidenceSpan, input: &str) -> bool {
    span.valid_for(input) && !span.text.trim().is_empty()
}

/// Whether the relation's text should appear literally in the source.  This is
/// true for explicit-equation prompts ("Solve 2x + 3 = 11") and false for word
/// problems, where symbols are introduced by the interpretation and only the
/// surrounding phrase is literal.
fn relation_text_is_literal(gold: &FidelityGoldCase, input: &str) -> bool {
    let collapsed_input: String = input
        .to_lowercase()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ");
    gold.faithful_relations.iter().any(|relation| {
        collapsed_input.contains(&relation.expression.to_lowercase())
    })
}

/// For literal-relation sources, every relation must be anchored by at least
/// one span whose text actually appears under the relation's declared tokens.
/// A structurally valid span that is unrelated to the relation exposes an
/// invented equation.  Word-problem relations are governed instead by the
/// gold's explicit `forbidden_relations`.
fn relation_anchor_present(relation: &RelationIR, input: &str) -> bool {
    relation
        .evidence_spans
        .iter()
        .filter(|span| span_supports(span, input))
        .any(|span| {
            let span_text = span.text.to_lowercase();
            let tokens: Vec<String> = relation
                .expression
                .split(|c: char| !c.is_alphanumeric() && c != '_')
                .filter(|token| !token.is_empty())
                .map(|token| token.to_lowercase())
                .collect();
            tokens.is_empty() || tokens.iter().any(|token| span_text.contains(token))
        })
}

/// Compare one proposal to one gold case, independently of the structural
/// validator.  The caller is expected to have already run
/// `semantic_ir::validate_candidate`; `structural_ok` records its verdict so
/// structural failure and semantic infidelity never collapse into one number.
pub fn assess_fidelity(
    input: &str,
    candidate: &CandidateSemanticParse,
    gold: &FidelityGoldCase,
    structural_ok: bool,
) -> FidelityVerdict {
    if !structural_ok {
        return FidelityVerdict::StructurallyUnusable {
            diagnostics: vec!["structural validation did not accept the candidate".into()],
        };
    }
    if !candidate.replay_verified() {
        return FidelityVerdict::StructurallyUnusable {
            diagnostics: vec!["candidate replay failed".into()],
        };
    }

    // Unsupported-domain sources have no faithful equation to reproduce.  A
    // proposal that commits to one anyway is the infidelity the category
    // names; there is nothing to match against.
    if gold.unsupported_expected {
        if candidate.equations.is_empty() && candidate.unresolved_ambiguities.is_empty() {
            return FidelityVerdict::Faithful;
        }
        return FidelityVerdict::Infidelity {
            category: FaithfulnessCategory::UnsupportedDomain,
            detail: "committed to an interpretation outside the labelled domains".into(),
        };
    }

    let produced: Vec<String> = candidate.equations.iter().map(relation_expression).collect();
    let faithful = gold.faithful_expressions();
    let distractors = gold.distractor_expressions();

    if faithful.is_empty() {
        return FidelityVerdict::Infidelity {
            category: FaithfulnessCategory::WrongQuantity,
            detail: "gold case has no faithful relation to match".into(),
        };
    }

    let matches_faithful = faithful.iter().all(|expected| {
        produced
            .iter()
            .any(|actual| actual.eq_ignore_ascii_case(expected))
    });
    let hit_distractor = produced.iter().any(|actual| {
        distractors
            .iter()
            .any(|distractor| actual.eq_ignore_ascii_case(distractor))
    });

    // Missing condition: a required surface fragment is absent from the
    // candidate's relations and assumptions.
    let declared_text = candidate
        .equations
        .iter()
        .map(|relation| relation.expression.as_str())
        .chain(candidate.assumptions.iter().map(String::as_str))
        .collect::<Vec<_>>()
        .join(" ")
        .to_lowercase();
    let missing_condition = gold
        .required_conditions
        .iter()
        .find(|condition| !declared_text.contains(&condition.to_lowercase()));

    // Invented equation: a relation in `forbidden_relations`, or (for
    // explicit-equation sources) an expressed relation with a valid span that
    // nevertheless does not support it.
    let literal_source = relation_text_is_literal(gold, input);
    let invented = candidate.equations.iter().find(|relation| {
        gold.forbidden_relations
            .iter()
            .any(|forbidden| relation_expression(relation).eq_ignore_ascii_case(forbidden))
            || (literal_source && !relation_anchor_present(relation, input))
    });

    if let Some(relation) = invented {
        return FidelityVerdict::Infidelity {
            category: FaithfulnessCategory::InventedEquationValidSpan,
            detail: format!(
                "relation {:?} has no supporting evidence span",
                relation_expression(relation)
            ),
        };
    }

    if hit_distractor && !matches_faithful {
        let category = classify_distractor(gold, &produced);
        return FidelityVerdict::Infidelity {
            category,
            detail: format!("produced a distractor relation: {}", produced.join("; ")),
        };
    }

    if let Some(condition) = missing_condition {
        return FidelityVerdict::Infidelity {
            category: FaithfulnessCategory::MissingCondition,
            detail: format!("dropped required condition {condition:?}"),
        };
    }

    if !matches_faithful {
        // No distractor matched and the faithful relation is absent: this is a
        // quantity or relationship substitution the gold did not enumerate.
        if candidate.unresolved_ambiguities.len() > 1 {
            return FidelityVerdict::Infidelity {
                category: FaithfulnessCategory::MultiplePlausible,
                detail: "committed to one reading while alternatives remain".into(),
            };
        }
        return FidelityVerdict::Infidelity {
            category: FaithfulnessCategory::WrongQuantity,
            detail: format!(
                "expected one of {:?}, produced {:?}",
                faithful, produced
            ),
        };
    }

    FidelityVerdict::Faithful
}

/// Best-effort cause for a distractor hit, using the gold's own taxonomy.
fn classify_distractor(gold: &FidelityGoldCase, produced: &[String]) -> FaithfulnessCategory {
    let produced = produced.join(" ").to_lowercase();
    let faithful = gold
        .faithful_expressions()
        .join(" ")
        .to_lowercase();
    let sign_flip = |text: &str| {
        let tokens: Vec<char> = text.chars().collect();
        tokens
            .windows(2)
            .any(|pair| (pair[0] == '+' && pair[1] == '-') || (pair[0] == '-' && pair[1] == '+'))
    };
    if sign_flip(&faithful) && sign_flip(&produced) && faithful != produced {
        return FaithfulnessCategory::WrongSign;
    }
    if faithful
        .split_whitespace()
        .zip(produced.split_whitespace())
        .any(|(expected, actual)| {
            expected.chars().any(|c| c.is_ascii_digit())
                && actual.chars().any(|c| c.is_ascii_digit())
                && expected != actual
        })
    {
        return FaithfulnessCategory::WrongQuantity;
    }
    match gold.expected_category {
        FaithfulnessCategory::WrongSign => FaithfulnessCategory::WrongSign,
        FaithfulnessCategory::MissingCondition => FaithfulnessCategory::MissingCondition,
        FaithfulnessCategory::ReversedRelationship => FaithfulnessCategory::ReversedRelationship,
        _ => FaithfulnessCategory::WrongQuantity,
    }
}

/// Sentence-level clarification for an ambiguous proposal.  The wording is
/// derived from the candidate's own structured relations, never invented.
pub fn clarification_for(candidate: &CandidateSemanticParse) -> Option<String> {
    let relation = candidate.equations.first()?;
    let reading = relation_expression(relation);
    if reading.trim().is_empty() {
        return None;
    }
    Some(format!("Do you mean that {reading}?"))
}

/// A model-assisted wording proposal and its structured backing.
///
/// The structured result is the only source of truth.  The wording may
/// rephrase it, but it may not introduce a factual claim the structured result
/// does not support — a number, an equation, or a relation that is absent.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WordingGuardReceipt {
    pub accepted: bool,
    pub unsupported_claims: Vec<String>,
    pub detail: String,
}

/// Verify that every numeric or equation-shaped claim in `wording` is present
/// in `structured_result`.  Numbers are compared as substrings of the
/// normalized structured text; this is intentionally strict — a reworded
/// answer may not smuggle in an unstated quantity or relation.
pub fn guard_answer_wording(wording: &str, structured_result: &str) -> WordingGuardReceipt {
    let supported = structured_result.to_lowercase();
    let mut unsupported = Vec::new();

    // Numeric claims: a number shown to the user must appear in the
    // structured result.  Trailing sentence punctuation is ignored.
    for raw in wording.split(|c: char| !c.is_alphanumeric() && c != '.' && c != '_') {
        let token = raw.trim_matches('.');
        if token.is_empty() {
            continue;
        }
        if token.chars().any(|c| c.is_ascii_digit()) && !supported.contains(&token.to_lowercase()) {
            unsupported.push(token.to_string());
        }
    }

    // Equation-shaped claims: the mathy run around each '=' must appear in the
    // structured result.  A chatty rephrasing ("The solution is x = 4") is fine
    // as long as the equation itself ("x = 4") is backed by the result.
    let collapse = |text: &str| text.split_whitespace().collect::<String>();
    let structured_collapsed = collapse(&supported);
    for fragment in wording.split(['.', ';', ',']) {
        for mathy in equation_runs(fragment) {
            if !structured_collapsed.contains(&collapse(&mathy)) {
                unsupported.push(mathy);
            }
        }
    }

    unsupported.sort();
    unsupported.dedup();
    let accepted = unsupported.is_empty();
    WordingGuardReceipt {
        accepted,
        detail: if accepted {
            "every displayed claim is supported by the structured result".into()
        } else {
            format!("unsupported displayed claims: {}", unsupported.join("; "))
        },
        unsupported_claims: unsupported,
    }
}

/// Split a phrase into maximal runs of equation-looking tokens: whitespace
/// separated chunks that each contain a digit, an operator, or '='.
fn equation_runs(fragment: &str) -> Vec<String> {
    let is_mathy = |token: &str| {
        !token.is_empty()
            && token
                .chars()
                .all(|c| c.is_alphanumeric() || c == '_' || c == '.' || c == '+' || c == '-' || c == '*' || c == '/' || c == '^' || c == '=' || c == '(' || c == ')' || c == '<' || c == '>')
            && token
                .chars()
                .any(|c| c.is_ascii_digit() || c == '+' || c == '-' || c == '*' || c == '/' || c == '^' || c == '=')
    };
    let mut runs = Vec::new();
    let mut current: Vec<&str> = Vec::new();
    for token in fragment.split_whitespace() {
        if is_mathy(token) {
            current.push(token);
        } else if !current.is_empty() {
            if current.iter().any(|part| part.contains('=')) {
                runs.push(current.join(" "));
            }
            current.clear();
        }
    }
    if current.iter().any(|part| part.contains('=')) {
        runs.push(current.join(" "));
    }
    runs
}

/// Counts for the wrong-answer limit and for language coverage.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct FidelityTotals {
    pub cases: usize,
    pub faithful: usize,
    pub infidelities: usize,
    pub structurally_unusable: usize,
    pub categories: usize,
}

/// What a consumer should do with an interpretation given its fidelity.
///
/// The phase requires that every imperfect reading be clarified, rejected, or
/// flagged — never answered as if it were faithful.  This is the fail-closed
/// policy that makes the wrong-answer limit enforceable.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ShadowPolicy {
    /// The interpretation is faithful; a typed consumer may proceed.
    Proceed,
    /// The reading is plausible but unresolved: ask a short clarification.
    Clarify,
    /// The reading is wrong or unsupported: refuse and flag it.
    Reject,
}

/// Decide the consumer policy for one verdict.
pub fn policy_for(gold: &FidelityGoldCase, verdict: &FidelityVerdict) -> ShadowPolicy {
    match verdict {
        FidelityVerdict::Faithful => ShadowPolicy::Proceed,
        FidelityVerdict::StructurallyUnusable { .. } => ShadowPolicy::Reject,
        FidelityVerdict::Infidelity { category, .. } => {
            if gold.clarification_expected
                || *category == FaithfulnessCategory::MultiplePlausible
            {
                ShadowPolicy::Clarify
            } else {
                ShadowPolicy::Reject
            }
        }
    }
}

/// A wrong answer in the sense that matters here: a structurally accepted
/// infidelity that the policy still chose to proceed on.  The Phase 6 contract
/// requires this to be zero: wrong readings must clarify, reject, or be
/// flagged, and only a faithful reading may proceed.
pub fn silent_wrong_answers(
    gold: &FidelityGoldCase,
    verdict: &FidelityVerdict,
    structural_ok: bool,
) -> usize {
    if !structural_ok {
        return 0;
    }
    let proceed = matches!(policy_for(gold, verdict), ShadowPolicy::Proceed);
    if proceed && verdict.is_infidelity() {
        1
    } else {
        0
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::semantic_ir::{validate_candidate, SymbolIR, TargetKind, SEMANTIC_IR_SCHEMA};
    use std::collections::BTreeMap;

    /// A span that is valid for `input` because `text` occurs in it.  The
    /// relation and symbol fixtures share this so structural validation passes
    /// and only fidelity is under test.
    fn span(input: &str, text: &str, role: &str) -> EvidenceSpan {
        let start = input.find(text).expect("fixture span present in input");
        EvidenceSpan {
            start,
            end: start + text.len(),
            text: text.into(),
            role: role.into(),
        }
    }

    fn candidate_with_relation(
        input: &str,
        expression: &str,
        relation_span_text: &str,
    ) -> CandidateSemanticParse {
        let symbols = vec!["a".to_string(), "b".to_string()];
        let anchor = input.split_whitespace().next().unwrap_or("a");
        let symbol_irs = symbols
            .iter()
            .map(|name| SymbolIR {
                name: name.clone(),
                scope: "root".into(),
                type_name: Some("scalar".into()),
                domain: Some("integer".into()),
                declared: true,
                evidence_spans: vec![span(input, anchor, "symbol")],
            })
            .collect();
        CandidateSemanticParse {
            schema: SEMANTIC_IR_SCHEMA.into(),
            input_hash: crate::semantic_ir::input_hash(input),
            model_id: "test".into(),
            model_config_hash: "cfg".into(),
            prompt_hash: "prompt".into(),
            grammar_version: "grammar".into(),
            target: "a".into(),
            target_kind: TargetKind::Scalar,
            operation: "solve".into(),
            symbols: symbol_irs,
            symbol_scopes: BTreeMap::from([("a".into(), "root".into()), ("b".into(), "root".into())]),
            equations: vec![RelationIR {
                kind: "constraint".into(),
                expression: expression.into(),
                symbols,
                evidence_spans: vec![span(input, relation_span_text, "relation")],
            }],
            assumptions: Vec::new(),
            domains: vec!["integer".into()],
            candidate_pack: None,
            unresolved_ambiguities: Vec::new(),
            evidence_spans: vec![span(input, anchor, "target")],
            confidence: 0.0,
            raw_output_hash: "raw".into(),
            replay_hash: String::new(),
        }
        .with_replay_hash()
    }

    fn gold(category: FaithfulnessCategory) -> FidelityGoldCase {
        FidelityGoldCase {
            id: "g1".into(),
            prompt: "Alice has three more items than Bob.".into(),
            domain: "arithmetic_word".into(),
            faithful_relations: vec![GoldRelation {
                expression: "b = a - 3".into(),
                symbols: vec!["a".into(), "b".into()],
            }],
            distractor_relations: vec![GoldRelation {
                expression: "a = b - 3".into(),
                symbols: vec!["a".into(), "b".into()],
            }],
            required_conditions: Vec::new(),
            forbidden_relations: Vec::new(),
            expected_category: category,
            clarification_expected: false,
            unsupported_expected: false,
        }
    }

    #[test]
    fn faithful_relation_is_recognized_as_faithful() {
        let input = "Alice has three more items than Bob.";
        let candidate = candidate_with_relation(input, "b = a - 3", input);
        let structural = validate_candidate(input, &candidate);
        let verdict = assess_fidelity(
            input,
            &candidate,
            &gold(FaithfulnessCategory::Correct),
            structural.decision == crate::semantic_ir::ValidationDecision::AcceptCandidate,
        );
        assert_eq!(verdict, FidelityVerdict::Faithful);
    }

    #[test]
    fn reversed_relation_is_infidelity_not_faithful() {
        let input = "Alice has three more items than Bob.";
        let candidate = candidate_with_relation(input, "a = b - 3", input);
        let structural = validate_candidate(input, &candidate);
        let verdict = assess_fidelity(
            input,
            &candidate,
            &gold(FaithfulnessCategory::ReversedRelationship),
            structural.decision == crate::semantic_ir::ValidationDecision::AcceptCandidate,
        );
        assert_eq!(
            verdict.category(),
            Some(FaithfulnessCategory::ReversedRelationship)
        );
        assert!(!verdict.is_faithful());
    }

    #[test]
    fn invented_equation_with_valid_span_is_flagged() {
        let input = "Alice has three more items than Bob.";
        // "Bob" is a valid span but the relation z = 99 has no support in the
        // prompt.  Word-problem sources are governed by the gold's forbidden
        // list, so the invented equation is named there.
        let mut case = gold(FaithfulnessCategory::InventedEquationValidSpan);
        case.forbidden_relations = vec!["z = 99".into()];
        let candidate = candidate_with_relation(input, "z = 99", "Bob");
        let structural = validate_candidate(input, &candidate);
        let verdict = assess_fidelity(
            input,
            &candidate,
            &case,
            structural.decision == crate::semantic_ir::ValidationDecision::AcceptCandidate,
        );
        assert_eq!(
            verdict.category(),
            Some(FaithfulnessCategory::InventedEquationValidSpan)
        );
    }

    #[test]
    fn unsupported_span_for_literal_source_is_flagged() {
        // For explicit-equation sources the relation text is literal, so a
        // valid-but-unsupporting span is enough to expose an invented equation
        // without a forbidden list.
        let input = "Solve 2x + 3 = 11 for x.";
        let case = FidelityGoldCase {
            id: "literal".into(),
            prompt: input.into(),
            domain: "linear_equation".into(),
            faithful_relations: vec![GoldRelation {
                expression: "2x + 3 = 11".into(),
                symbols: vec!["x".into()],
            }],
            distractor_relations: Vec::new(),
            required_conditions: Vec::new(),
            forbidden_relations: Vec::new(),
            expected_category: FaithfulnessCategory::InventedEquationValidSpan,
            clarification_expected: false,
            unsupported_expected: false,
        };
        // Relation 9z = 1 anchored to the valid span "Solve", none of whose
        // tokens appear in the relation.
        let mut candidate = candidate_with_relation(input, "2x + 3 = 11", "Solve");
        candidate.equations[0].expression = "9z = 1".into();
        candidate.equations[0].symbols = vec!["z".into()];
        candidate = candidate.with_replay_hash();
        let structural = validate_candidate(input, &candidate);
        let verdict = assess_fidelity(
            input,
            &candidate,
            &case,
            structural.decision == crate::semantic_ir::ValidationDecision::AcceptCandidate,
        );
        assert_eq!(
            verdict.category(),
            Some(FaithfulnessCategory::InventedEquationValidSpan)
        );
    }

    #[test]
    fn structural_failure_is_not_counted_as_infidelity() {
        let input = "Alice has three more items than Bob.";
        let mut candidate = candidate_with_relation(input, "b = a - 3", input);
        candidate.evidence_spans[0].text = "not present".into();
        let verdict = assess_fidelity(input, &candidate, &gold(FaithfulnessCategory::Correct), false);
        assert!(matches!(
            verdict,
            FidelityVerdict::StructurallyUnusable { .. }
        ));
        assert_eq!(silent_wrong_answers(&gold(FaithfulnessCategory::Correct), &verdict, false), 0);
    }

    #[test]
    fn clarification_wording_is_derived_from_the_relation() {
        let input = "Alice has three more items than Bob.";
        let candidate = candidate_with_relation(input, "a = b - 3", input);
        let clarification = clarification_for(&candidate).expect("clarification");
        assert_eq!(clarification, "Do you mean that a = b - 3?");
    }

    #[test]
    fn wording_guard_rejects_unsupported_quantities() {
        let supported = "x = 4 from 2*x + 3 = 11";
        let faithful = guard_answer_wording("The solution is x = 4.", supported);
        assert!(faithful.accepted, "{faithful:?}");
        let overstated = guard_answer_wording("The solution is x = 4 and y = 7.", supported);
        assert!(!overstated.accepted);
        assert!(overstated.unsupported_claims.iter().any(|claim| claim.contains('7')));
    }

    #[test]
    fn wording_guard_rejects_invented_equation() {
        let supported = "x = 4 from 2*x + 3 = 11";
        let invented = guard_answer_wording("This comes from 2*x - 3 = 11.", supported);
        assert!(!invented.accepted);
    }

    #[test]
    fn policy_never_proceeds_on_an_infidelity() {
        let gold = gold(FaithfulnessCategory::WrongSign);
        let verdict = FidelityVerdict::Infidelity {
            category: FaithfulnessCategory::WrongSign,
            detail: "sign flipped".into(),
        };
        assert_eq!(policy_for(&gold, &verdict), ShadowPolicy::Reject);
        assert_eq!(silent_wrong_answers(&gold, &verdict, true), 0);
        let mut ambiguous = gold.clone();
        ambiguous.clarification_expected = true;
        assert_eq!(policy_for(&ambiguous, &verdict), ShadowPolicy::Clarify);
        assert_eq!(silent_wrong_answers(&ambiguous, &verdict, true), 0);
        assert_eq!(
            policy_for(&gold, &FidelityVerdict::Faithful),
            ShadowPolicy::Proceed
        );
    }
}
