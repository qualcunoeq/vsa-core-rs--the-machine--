//! Stage D source-derived finite-statistics validation.

use serde::Serialize;
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use the_machine::probability_pack::Rational;
use the_machine::source_formula_pack::{FormulaRequest, FormulaResult, FormulaStatus};
use the_machine::source_statistics_pack::{evaluate_statistics, DOMAIN};

#[derive(Clone, Copy, Serialize, PartialEq, Eq)]
enum Expected {
    Complete,
    Ambiguous,
    Refused,
}

#[derive(Serialize)]
struct Receipt {
    id: String,
    expected: Expected,
    actual: FormulaStatus,
    expected_value: Option<Rational>,
    actual_value: Option<Rational>,
    exact: bool,
    value_correct: bool,
    source_preserved: bool,
    replay_verified: bool,
    tamper_rejected: bool,
    false_authorization: bool,
}

#[derive(Serialize)]
struct Report {
    schema: &'static str,
    source: &'static str,
    corpus_sha256: String,
    cases: usize,
    supported: usize,
    ambiguous: usize,
    refused: usize,
    exact_decisions: usize,
    supported_values: usize,
    source_preserved: usize,
    replay_verified: usize,
    tamper_rejections: usize,
    false_authorizations: usize,
    false_denials: usize,
    receipts: Vec<Receipt>,
}

fn q(numerator: i128, denominator: i128) -> Rational {
    Rational::new(numerator, denominator).unwrap()
}

fn digest<T: Serialize>(value: &T) -> String {
    format!("{:x}", Sha256::digest(serde_json::to_vec(value).unwrap()))
}

fn request(formula: &str) -> FormulaRequest {
    FormulaRequest {
        formula: formula.into(),
        inputs: BTreeMap::from([
            ("sum".into(), q(30, 1)),
            ("count".into(), q(5, 1)),
            ("weighted_sum".into(), q(30, 1)),
            ("total_weight".into(), q(5, 1)),
            ("p".into(), q(1, 4)),
            ("n".into(), q(8, 1)),
        ]),
        domain: DOMAIN.into(),
        ambiguity: None,
        provenance: vec!["stage-d-independent-statistics-corpus".into()],
    }
}

fn expected_value(formula: &str) -> Option<Rational> {
    match formula {
        "arithmetic_mean" | "weighted_mean" => Some(q(6, 1)),
        "bernoulli_variance" => Some(q(3, 16)),
        "binomial_expected_value" => Some(q(2, 1)),
        "binomial_variance" => Some(q(3, 2)),
        _ => None,
    }
}

fn run(id: String, expected: Expected, request: FormulaRequest, value: Option<Rational>) -> Receipt {
    let result: FormulaResult = evaluate_statistics(&request);
    let mut tampered = result.clone();
    tampered.replay_hash.push('x');
    let exact = match expected {
        Expected::Complete => result.status == FormulaStatus::Complete,
        Expected::Ambiguous => result.status == FormulaStatus::Ambiguous,
        Expected::Refused => result.status != FormulaStatus::Complete,
    };
    let value_correct = expected != Expected::Complete || result.value == value;
    Receipt {
        id,
        expected,
        actual: result.status,
        expected_value: value,
        actual_value: result.value.clone(),
        exact,
        value_correct,
        source_preserved: expected == Expected::Complete && result.source.is_some(),
        replay_verified: result.replay_verified(),
        tamper_rejected: !tampered.replay_verified(),
        false_authorization: expected != Expected::Complete && result.status == FormulaStatus::Complete,
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let formulas = [
        "arithmetic_mean",
        "weighted_mean",
        "bernoulli_variance",
        "binomial_expected_value",
        "binomial_variance",
    ];
    let mut receipts = Vec::with_capacity(240);
    for index in 0..120 {
        let formula = formulas[index % formulas.len()];
        let request = request(formula);
        receipts.push(run(
            format!("supported_{index:03}"),
            Expected::Complete,
            request,
            expected_value(formula),
        ));
    }
    for index in 0..40 {
        let formula = formulas[index % formulas.len()];
        let mut request = request(formula);
        request.ambiguity = Some("source notation selects multiple statistical formulations".into());
        receipts.push(run(format!("ambiguous_{index:03}"), Expected::Ambiguous, request, expected_value(formula)));
    }
    for index in 0..20 {
        receipts.push(run(format!("unknown_{index:03}"), Expected::Refused, request("unknown_statistics_formula"), None));
    }
    for index in 0..20 {
        let mut request = request("arithmetic_mean");
        request.inputs.remove("count");
        receipts.push(run(format!("missing_input_{index:03}"), Expected::Refused, request, None));
    }
    for index in 0..20 {
        let mut request = request("weighted_mean");
        request.inputs.insert("total_weight".into(), q(0, 1));
        receipts.push(run(format!("zero_weight_{index:03}"), Expected::Refused, request, None));
    }
    for index in 0..10 {
        let mut request = request("binomial_variance");
        request.domain = "unsupported_continuous_statistics".into();
        receipts.push(run(format!("unsupported_domain_{index:03}"), Expected::Refused, request, None));
    }
    for index in 0..10 {
        let mut request = request("bernoulli_variance");
        request.inputs.insert("p".into(), q(5, 4));
        receipts.push(run(format!("invalid_probability_{index:03}"), Expected::Refused, request, None));
    }
    assert_eq!(receipts.len(), 240);
    let cases = receipts.len();
    let supported = receipts.iter().filter(|r| r.expected == Expected::Complete).count();
    let ambiguous = receipts.iter().filter(|r| r.expected == Expected::Ambiguous).count();
    let refused = receipts.iter().filter(|r| r.expected == Expected::Refused).count();
    let exact_decisions = receipts.iter().filter(|r| r.exact).count();
    let supported_values = receipts.iter().filter(|r| r.expected == Expected::Complete && r.value_correct).count();
    let source_preserved = receipts.iter().filter(|r| r.source_preserved).count();
    let replay_verified = receipts.iter().filter(|r| r.replay_verified).count();
    let tamper_rejections = receipts.iter().filter(|r| r.tamper_rejected).count();
    let false_authorizations = receipts.iter().filter(|r| r.false_authorization).count();
    let false_denials = receipts.iter().filter(|r| r.expected == Expected::Complete && !r.exact).count();
    assert_eq!((supported, ambiguous, refused), (120, 40, 80));
    assert_eq!((exact_decisions, supported_values, source_preserved, replay_verified, tamper_rejections, false_authorizations, false_denials), (240, 120, 120, 240, 240, 0, 0));
    let report = Report {
        schema: "stage-d-source-derived-finite-statistics-v1",
        source: "OpenStax Introductory Statistics 2e, descriptive statistics",
        corpus_sha256: digest(&receipts),
        cases,
        supported,
        ambiguous,
        refused,
        exact_decisions,
        supported_values,
        source_preserved,
        replay_verified,
        tamper_rejections,
        false_authorizations,
        false_denials,
        receipts,
    };
    let serialized = serde_json::to_string_pretty(&report)?;
    std::fs::write("docs/stage_d_source_statistics_pack.json", format!("{serialized}\n"))?;
    println!("{serialized}");
    Ok(())
}
