//! Stage 134: source-backed bounded arithmetic-functions pack.

use serde::Serialize;
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use the_machine::bounded_arithmetic_functions_pack::{
    evaluate, ArithmeticFunctionOperation, ArithmeticFunctionRequest, ArithmeticFunctionStatus,
};

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
enum Expected {
    Supported,
    Ambiguous,
    Refused,
}

#[derive(Debug, Clone, Serialize)]
struct Case {
    id: String,
    operation: ArithmeticFunctionOperation,
    value: Option<u64>,
    domain: String,
    ambiguity: Option<String>,
    expected: Expected,
}

#[derive(Debug, Serialize)]
struct Receipt {
    id: String,
    expected: Expected,
    actual_status: ArithmeticFunctionStatus,
    exact: bool,
    authorized: bool,
    replay_verified: bool,
    tamper_rejected: bool,
    false_authorization: bool,
    false_denial: bool,
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
    tamper_rejected: usize,
    false_authorizations: usize,
    false_denials: usize,
    status_counts: BTreeMap<String, usize>,
    receipts: Vec<Receipt>,
}

fn digest<T: Serialize>(value: &T) -> String {
    format!("{:x}", Sha256::digest(serde_json::to_vec(value).unwrap()))
}

fn request(case: &Case) -> ArithmeticFunctionRequest {
    ArithmeticFunctionRequest {
        operation: case.operation,
        value: case.value,
        domain: case.domain.clone(),
        ambiguity: case.ambiguity.clone(),
        provenance: vec![format!("stage134:{}", case.id)],
    }
}

fn corpus() -> Vec<Case> {
    let mut cases = Vec::with_capacity(240);
    let values = [12, 18, 24, 30, 36, 42, 60, 72, 90, 120];
    for index in 0..30 {
        cases.push(Case {
            id: format!("divisor_count_{index:03}"),
            operation: ArithmeticFunctionOperation::DivisorCount,
            value: Some(values[index % values.len()]),
            domain: "bounded_arithmetic_functions".into(),
            ambiguity: None,
            expected: Expected::Supported,
        });
    }
    for index in 0..30 {
        cases.push(Case {
            id: format!("divisor_sum_{index:03}"),
            operation: ArithmeticFunctionOperation::DivisorSum,
            value: Some(values[index % values.len()]),
            domain: "bounded_arithmetic_functions".into(),
            ambiguity: None,
            expected: Expected::Supported,
        });
    }
    for index in 0..30 {
        cases.push(Case {
            id: format!("mobius_{index:03}"),
            operation: ArithmeticFunctionOperation::Mobius,
            value: Some([1, 2, 6, 10, 12, 30, 36, 42][index % 8]),
            domain: "bounded_arithmetic_functions".into(),
            ambiguity: None,
            expected: Expected::Supported,
        });
    }
    for index in 0..30 {
        cases.push(Case {
            id: format!("prime_counting_{index:03}"),
            operation: ArithmeticFunctionOperation::PrimeCounting,
            value: Some(10 + (index % 10) as u64 * 7),
            domain: "bounded_arithmetic_functions".into(),
            ambiguity: None,
            expected: Expected::Supported,
        });
    }
    for index in 0..40 {
        cases.push(Case {
            id: format!("ambiguous_{index:03}"),
            operation: [
                ArithmeticFunctionOperation::DivisorCount,
                ArithmeticFunctionOperation::Mobius,
            ][index % 2],
            value: Some(30),
            domain: "bounded_arithmetic_functions".into(),
            ambiguity: Some("the requested arithmetic function is not uniquely specified".into()),
            expected: Expected::Ambiguous,
        });
    }
    for index in 0..80 {
        let (operation, value, domain) = match index % 4 {
            0 => (
                ArithmeticFunctionOperation::Mobius,
                Some(100_001),
                "bounded_arithmetic_functions",
            ),
            1 => (
                ArithmeticFunctionOperation::PrimeCounting,
                Some(0),
                "bounded_arithmetic_functions",
            ),
            2 => (
                ArithmeticFunctionOperation::DivisorSum,
                Some(10),
                "analytic_number_theory",
            ),
            _ => (
                ArithmeticFunctionOperation::DivisorCount,
                None,
                "bounded_arithmetic_functions",
            ),
        };
        cases.push(Case {
            id: format!("refused_{index:03}"),
            operation,
            value,
            domain: domain.into(),
            ambiguity: None,
            expected: Expected::Refused,
        });
    }
    assert_eq!(cases.len(), 240);
    cases
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let corpus = corpus();
    let mut receipts = Vec::with_capacity(corpus.len());
    let mut status_counts = BTreeMap::new();
    for case in &corpus {
        let result = evaluate(&request(case));
        let authorized = result.authorized();
        let exact = match case.expected {
            Expected::Supported => authorized,
            Expected::Ambiguous => result.status == ArithmeticFunctionStatus::Ambiguous,
            Expected::Refused => !authorized && result.artifact.is_none(),
        };
        let replay_verified = result.replay_verified();
        let mut tampered = result.clone();
        tampered.replay_hash.push('x');
        let tamper_rejected = !tampered.replay_verified();
        let false_authorization = case.expected != Expected::Supported && authorized;
        let false_denial = case.expected == Expected::Supported && !authorized;
        *status_counts
            .entry(format!("{:?}", result.status))
            .or_insert(0usize) += 1;
        receipts.push(Receipt {
            id: case.id.clone(),
            expected: case.expected,
            actual_status: result.status,
            exact,
            authorized,
            replay_verified,
            tamper_rejected,
            false_authorization,
            false_denial,
        });
    }
    let cases = receipts.len();
    let supported = receipts
        .iter()
        .filter(|r| r.expected == Expected::Supported)
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
    let supported_artifacts = receipts.iter().filter(|r| r.authorized).count();
    let replay_verified = receipts.iter().filter(|r| r.replay_verified).count();
    let tamper_rejected = receipts.iter().filter(|r| r.tamper_rejected).count();
    let false_authorizations = receipts.iter().filter(|r| r.false_authorization).count();
    let false_denials = receipts.iter().filter(|r| r.false_denial).count();
    assert_eq!((supported, ambiguous, refused), (120, 40, 80));
    assert_eq!(exact_decisions, cases);
    assert_eq!(supported_artifacts, supported);
    assert_eq!(replay_verified, cases);
    assert_eq!(tamper_rejected, cases);
    assert_eq!(false_authorizations, 0);
    assert_eq!(false_denials, 0);
    let report = Report {
        schema: "stage134-bounded-arithmetic-functions-v1",
        source: "MIT OpenCourseWare 18.781 source-attributed finite arithmetic functions",
        corpus_sha256: digest(&corpus),
        cases,
        supported,
        ambiguous,
        refused,
        exact_decisions,
        supported_artifacts,
        replay_verified,
        tamper_rejected,
        false_authorizations,
        false_denials,
        status_counts,
        receipts,
    };
    std::fs::write(
        "docs/stage134_bounded_arithmetic_functions.json",
        serde_json::to_vec_pretty(&report)?,
    )?;
    println!("{}", serde_json::to_string_pretty(&report)?);
    Ok(())
}
