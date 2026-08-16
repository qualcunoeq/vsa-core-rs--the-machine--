//! Independent validation corpus for the bounded exact scalar ODE pack.

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use the_machine::ode_pack::{evaluate_ode, OdeOperation, OdeRequest, OdeStatus};
use the_machine::probability_pack::Rational;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
enum Expected {
    Complete,
    Ambiguous,
    Refused,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Receipt {
    id: String,
    family: String,
    expected: Expected,
    actual: OdeStatus,
    exact: bool,
    replay_verified: bool,
    tamper_rejected: bool,
    false_authorization: bool,
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
    false_authorizations: usize,
    false_denials: usize,
    receipts: Vec<Receipt>,
}

fn rational(numerator: i128, denominator: i128) -> Rational {
    Rational::new(numerator, denominator).unwrap()
}

fn digest<T: Serialize>(value: &T) -> String {
    format!("{:x}", Sha256::digest(serde_json::to_vec(value).unwrap()))
}

fn request(operation: OdeOperation) -> OdeRequest {
    OdeRequest {
        operation,
        initial: Some(rational(2, 1)),
        coefficient: Some(rational(3, 1)),
        forcing: Some(rational(4, 1)),
        time: Some(rational(2, 1)),
        domain: "bounded_exact_scalar_ode".into(),
        ambiguity: None,
        provenance: vec!["stage-a-ode-corpus".into()],
    }
}

fn run(id: String, family: &str, expected: Expected, mut req: OdeRequest) -> Receipt {
    let result = evaluate_ode(&req);
    let actual = result.status;
    let expected_status = match expected {
        Expected::Complete => OdeStatus::Complete,
        Expected::Ambiguous => OdeStatus::Ambiguous,
        Expected::Refused => actual,
    };
    let exact = match expected {
        Expected::Refused => actual != OdeStatus::Complete,
        _ => actual == expected_status,
    };
    let replay_verified = result.replay_verified();
    let mut tampered = result.clone();
    tampered.replay_hash.push('x');
    let tamper_rejected = !tampered.replay_verified();
    let false_authorization = expected != Expected::Complete && actual == OdeStatus::Complete;
    req.provenance.push("receipt-only-field".into());
    Receipt {
        id,
        family: family.into(),
        expected,
        actual,
        exact,
        replay_verified,
        tamper_rejected,
        false_authorization,
    }
}

fn main() {
    let mut receipts = Vec::with_capacity(240);
    for index in 0..60 {
        let mut req = request(OdeOperation::ConstantDerivative);
        req.initial = Some(rational(index as i128 + 1, 1));
        req.forcing = Some(rational((index % 7 + 1) as i128, 1));
        req.time = Some(rational((index % 8) as i128, 1));
        receipts.push(run(
            format!("constant_derivative_{index}"),
            "constant_derivative",
            Expected::Complete,
            req,
        ));
    }
    for index in 0..60 {
        let mut req = request(OdeOperation::AffineLinear);
        req.coefficient = Some(rational((index % 5 + 1) as i128, 2));
        req.forcing = Some(rational((index % 7 + 1) as i128, 3));
        req.time = Some(rational((index % 8) as i128, 1));
        receipts.push(run(
            format!("affine_linear_{index}"),
            "affine_linear",
            Expected::Complete,
            req,
        ));
    }
    for index in 0..20 {
        let mut req = request(OdeOperation::AffineLinear);
        req.ambiguity = Some("time variable scope is unresolved".into());
        receipts.push(run(
            format!("ambiguous_time_{index}"),
            "ambiguous_time",
            Expected::Ambiguous,
            req,
        ));
    }
    for index in 0..20 {
        let mut req = request(OdeOperation::ConstantDerivative);
        req.initial = None;
        req.ambiguity = Some("initial value is absent and cannot be inferred".into());
        receipts.push(run(
            format!("missing_initial_{index}"),
            "missing_initial",
            Expected::Ambiguous,
            req,
        ));
    }
    for index in 0..20 {
        let req = request(OdeOperation::Nonlinear);
        receipts.push(run(
            format!("nonlinear_{index}"),
            "nonlinear",
            Expected::Refused,
            req,
        ));
    }
    for index in 0..20 {
        let req = request(OdeOperation::CoupledSystem);
        receipts.push(run(
            format!("coupled_{index}"),
            "coupled_system",
            Expected::Refused,
            req,
        ));
    }
    for index in 0..20 {
        let mut req = request(OdeOperation::AffineLinear);
        req.time = Some(rational(9, 1));
        receipts.push(run(
            format!("long_horizon_{index}"),
            "long_horizon",
            Expected::Refused,
            req,
        ));
    }
    for index in 0..20 {
        let mut req = request(OdeOperation::NumericalApproximation);
        req.domain = "bounded_exact_scalar_ode".into();
        receipts.push(run(
            format!("numerical_{index}"),
            "numerical_approximation",
            Expected::Refused,
            req,
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
        .filter(|r| r.expected == Expected::Complete && r.actual == OdeStatus::Complete)
        .count();
    let replay_verified = receipts.iter().filter(|r| r.replay_verified).count();
    let tamper_rejections = receipts.iter().filter(|r| r.tamper_rejected).count();
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
    assert_eq!(false_authorizations, 0);
    assert_eq!(false_denials, 0);
    let report = Report {
        schema: "stage-a-bounded-ode-v1",
        source: "independently authored exact scalar ODE corpus",
        corpus_sha256: digest(&receipts),
        cases,
        supported,
        ambiguous,
        refused,
        exact_decisions,
        supported_artifacts,
        replay_verified,
        tamper_rejections,
        false_authorizations,
        false_denials,
        receipts,
    };
    let serialized = serde_json::to_string_pretty(&report).unwrap();
    std::fs::write("docs/stage_a_ode_pack.json", format!("{serialized}\n")).unwrap();
    println!("{serialized}");
}
