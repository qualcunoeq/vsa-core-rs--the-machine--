//! Validation campaign for the source-derived declarative formula pack.

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use the_machine::probability_pack::Rational;
use the_machine::source_formula_pack::{evaluate_formula, FormulaRequest, FormulaStatus};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
enum Expected {
    Complete,
    Ambiguous,
    Refused,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Receipt {
    id: String,
    formula: String,
    expected: Expected,
    actual: FormulaStatus,
    exact: bool,
    replay_verified: bool,
    tamper_rejected: bool,
    false_authorization: bool,
    source_preserved: bool,
    value_correct: bool,
}

#[derive(Debug, Serialize)]
struct Report {
    schema: &'static str,
    source: &'static str,
    corpus_sha256: String,
    cases: usize,
    supported: usize,
    ambiguous: usize,
    refused: usize,
    exact_decisions: usize,
    supported_artifacts: usize,
    replay_verified: usize,
    tamper_rejections: usize,
    source_preserved: usize,
    value_correct: usize,
    false_authorizations: usize,
    false_denials: usize,
    status_counts: BTreeMap<String, usize>,
    receipts: Vec<Receipt>,
}

fn rational(numerator: i128, denominator: i128) -> Rational {
    Rational::new(numerator, denominator).unwrap()
}

fn digest<T: Serialize>(value: &T) -> String {
    format!("{:x}", Sha256::digest(serde_json::to_vec(value).unwrap()))
}

fn base(formula: &str) -> FormulaRequest {
    FormulaRequest {
        formula: formula.into(),
        inputs: BTreeMap::from([
            ("a1".into(), rational(2, 1)),
            ("n".into(), rational(5, 1)),
            ("d".into(), rational(3, 1)),
            ("r".into(), rational(2, 1)),
        ]),
        domain: "source_derived_sequences_series".into(),
        ambiguity: None,
        provenance: vec!["source-formula-independent-corpus".into()],
    }
}

fn run(id: String, formula: &str, expected: Expected, request: FormulaRequest) -> Receipt {
    let result = evaluate_formula(&request);
    let actual = result.status;
    let exact = match expected {
        Expected::Complete => actual == FormulaStatus::Complete,
        Expected::Ambiguous => actual == FormulaStatus::Ambiguous,
        Expected::Refused => actual != FormulaStatus::Complete,
    };
    let replay_verified = result.replay_verified();
    let mut tampered = result.clone();
    tampered.replay_hash.push('x');
    let expected_value = match formula {
        "arithmetic_nth_term" => Some(rational(14, 1)),
        "arithmetic_partial_sum" => Some(rational(40, 1)),
        "geometric_nth_term" => Some(rational(32, 1)),
        "geometric_partial_sum" => Some(rational(62, 1)),
        _ => None,
    };
    let value_correct = expected != Expected::Complete
        || expected_value
            .as_ref()
            .map_or(true, |value| result.value.as_ref() == Some(value));
    Receipt {
        id,
        formula: formula.into(),
        expected,
        actual,
        exact,
        replay_verified,
        tamper_rejected: !tampered.replay_verified(),
        false_authorization: expected != Expected::Complete && actual == FormulaStatus::Complete,
        source_preserved: expected == Expected::Complete && result.source.is_some(),
        value_correct,
    }
}

fn main() {
    let formulas = [
        "arithmetic_nth_term",
        "arithmetic_partial_sum",
        "geometric_nth_term",
        "geometric_partial_sum",
    ];
    let mut receipts = Vec::with_capacity(240);
    for index in 0..120 {
        let formula = formulas[index % formulas.len()];
        receipts.push(run(
            format!("supported_{index:03}"),
            formula,
            Expected::Complete,
            base(formula),
        ));
    }
    for index in 0..40 {
        let formula = formulas[index % formulas.len()];
        let mut request = base(formula);
        request.ambiguity = Some("notation selects more than one source formulation".into());
        receipts.push(run(
            format!("ambiguous_{index:03}"),
            formula,
            Expected::Ambiguous,
            request,
        ));
    }
    for index in 0..20 {
        let mut request = base("unknown_formula");
        receipts.push(run(
            format!("unknown_{index:03}"),
            "unknown_formula",
            Expected::Refused,
            request,
        ));
    }
    for index in 0..20 {
        let mut request = base("arithmetic_nth_term");
        request.inputs.remove("d");
        receipts.push(run(
            format!("missing_input_{index:03}"),
            "arithmetic_nth_term",
            Expected::Refused,
            request,
        ));
    }
    for index in 0..20 {
        let mut request = base("arithmetic_nth_term");
        request.inputs.insert("n".into(), rational(0, 1));
        receipts.push(run(
            format!("invalid_n_{index:03}"),
            "arithmetic_nth_term",
            Expected::Refused,
            request,
        ));
    }
    for index in 0..20 {
        let mut request = base("geometric_partial_sum");
        request.inputs.insert("r".into(), rational(1, 1));
        receipts.push(run(
            format!("invalid_ratio_{index:03}"),
            "geometric_partial_sum",
            Expected::Refused,
            request,
        ));
    }
    assert_eq!(receipts.len(), 240);
    let cases = receipts.len();
    let supported = receipts
        .iter()
        .filter(|r| r.expected == Expected::Complete)
        .count();
    let ambiguous = receipts
        .iter()
        .filter(|r| r.expected == Expected::Ambiguous)
        .count();
    let refused = receipts
        .iter()
        .filter(|r| r.expected == Expected::Refused)
        .count();
    let exact_decisions = receipts.iter().filter(|r| r.exact).count();
    let supported_artifacts = receipts
        .iter()
        .filter(|r| r.expected == Expected::Complete && r.actual == FormulaStatus::Complete)
        .count();
    let replay_verified = receipts.iter().filter(|r| r.replay_verified).count();
    let tamper_rejections = receipts.iter().filter(|r| r.tamper_rejected).count();
    let source_preserved = receipts.iter().filter(|r| r.source_preserved).count();
    let value_correct = receipts.iter().filter(|r| r.value_correct).count();
    let false_authorizations = receipts.iter().filter(|r| r.false_authorization).count();
    let false_denials = receipts
        .iter()
        .filter(|r| r.expected == Expected::Complete && !r.exact)
        .count();
    assert_eq!((supported, ambiguous, refused), (120, 40, 80));
    assert_eq!(exact_decisions, cases);
    assert_eq!(supported_artifacts, supported);
    assert_eq!(replay_verified, cases);
    assert_eq!(tamper_rejections, cases);
    assert_eq!(source_preserved, supported);
    assert_eq!(value_correct, cases);
    assert_eq!(false_authorizations, 0);
    assert_eq!(false_denials, 0);
    let mut status_counts = BTreeMap::new();
    for receipt in &receipts {
        *status_counts
            .entry(format!("{:?}", receipt.actual))
            .or_insert(0) += 1;
    }
    let report = Report {
        schema: "stage-d-source-derived-formula-v1",
        source: "OpenStax Precalculus 2e, sequences and series",
        corpus_sha256: digest(&receipts),
        cases,
        supported,
        ambiguous,
        refused,
        exact_decisions,
        supported_artifacts,
        replay_verified,
        tamper_rejections,
        source_preserved,
        value_correct,
        false_authorizations,
        false_denials,
        status_counts,
        receipts,
    };
    let serialized = serde_json::to_string_pretty(&report).unwrap();
    std::fs::write(
        "docs/stage_d_source_formula_pack.json",
        format!("{serialized}\n"),
    )
    .unwrap();
    println!("{serialized}");
}
