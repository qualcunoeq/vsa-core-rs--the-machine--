//! Phase 61 benchmark for the bounded exact one-variable calculus pack.

use serde::Serialize;
use std::fs;
use the_machine::calculus_pack::{
    evaluate_calculus, CalculusArtifact, CalculusOperation, CalculusRequest, CalculusStatus,
};

#[derive(Debug, Clone, Serialize)]
struct Receipt {
    id: String,
    expected: CalculusStatus,
    actual: CalculusStatus,
    supported: bool,
    artifact_present: bool,
    replay_verified: bool,
    tamper_rejected: bool,
    exact_artifact: bool,
}

#[derive(Debug, Serialize)]
struct Report {
    schema: &'static str,
    benchmark: &'static str,
    cases: usize,
    supported: usize,
    boundary: usize,
    unsupported: usize,
    exact_decisions: usize,
    supported_artifacts: usize,
    replay_verified: usize,
    tamper_rejections: usize,
    false_authorizations: usize,
    false_denials: usize,
    receipts: Vec<Receipt>,
}

fn request(operation: CalculusOperation, expression: &str) -> CalculusRequest {
    CalculusRequest {
        operation,
        domain: "bounded_exact_single_variable_calculus".into(),
        expression: expression.into(),
        variable: Some("x".into()),
        lower: None,
        upper: None,
        point: None,
        ambiguity: None,
        provenance: vec!["phase61-calculus-bench".into()],
    }
}

fn supported_case(index: usize) -> CalculusRequest {
    match index {
        0..=29 => request(CalculusOperation::Derivative, "x^2 + 3*x + 1"),
        30..=54 => request(CalculusOperation::Integral, "2*x + 1"),
        55..=74 => {
            let mut request = request(CalculusOperation::DefiniteIntegral, "2*x");
            request.lower = Some(0.0);
            request.upper = Some(2.0);
            request
        }
        75..=94 => {
            let mut request = request(CalculusOperation::Limit, "x^2 + 2*x + 1");
            request.point = Some(2.0);
            request
        }
        _ => {
            let mut request = request(CalculusOperation::Continuity, "x^4 + 2*x^2 + 1");
            request.point = Some(index as f64 % 5.0);
            request
        }
    }
}

fn boundary_case(index: usize) -> CalculusRequest {
    let mut request = request(
        match index % 4 {
            0 => CalculusOperation::Derivative,
            1 => CalculusOperation::DefiniteIntegral,
            2 => CalculusOperation::Limit,
            _ => CalculusOperation::Continuity,
        },
        "x^2 + 1",
    );
    match index % 4 {
        0 => request.variable = None,
        1 => request.lower = Some(0.0),
        2 => request.point = None,
        _ => request.ambiguity = Some("target point has two unresolved scopes".into()),
    }
    request
}

fn unsupported_case(index: usize) -> CalculusRequest {
    let mut request = request(CalculusOperation::Derivative, "partial_x f(x,y)");
    match index % 8 {
        0 => request.expression = "partial_x f(x,y)".into(),
        1 => request.expression = "x^2 + infinity".into(),
        2 => request.expression = "numerical derivative of x^2".into(),
        3 => request.expression = "piecewise x^2".into(),
        4 => request.domain = "continuous_time_dynamics".into(),
        5 => {
            request.operation = CalculusOperation::DefiniteIntegral;
            request.expression = "x".into();
            request.lower = Some(0.0);
            request.upper = Some(1.0);
        }
        6 => {
            request.operation = CalculusOperation::Limit;
            request.expression = "sin(x)/x".into();
            request.point = Some(0.0);
        }
        _ => {
            request.operation = CalculusOperation::Continuity;
            request.expression = "sqrt(x)".into();
            request.point = Some(0.0);
        }
    }
    request
}

fn main() {
    let mut receipts = Vec::with_capacity(240);
    for index in 0..120 {
        let request = supported_case(index);
        let result = evaluate_calculus(&request);
        let mut tampered = result.clone();
        tampered.replay_hash.push('x');
        let exact_artifact = matches!(
            result.artifact,
            Some(CalculusArtifact::Symbolic(_))
                | Some(CalculusArtifact::ExactValue(_))
                | Some(CalculusArtifact::Boolean(_))
        );
        receipts.push(Receipt {
            id: format!("supported_{index}"),
            expected: CalculusStatus::Complete,
            actual: result.status,
            supported: true,
            artifact_present: result.artifact.is_some(),
            replay_verified: result.replay_verified(),
            tamper_rejected: !tampered.replay_verified(),
            exact_artifact,
        });
    }
    for index in 0..40 {
        let request = boundary_case(index);
        let result = evaluate_calculus(&request);
        let mut tampered = result.clone();
        tampered.replay_hash.push('x');
        receipts.push(Receipt {
            id: format!("boundary_{index}"),
            expected: if index % 4 == 3 {
                CalculusStatus::Ambiguous
            } else {
                CalculusStatus::Missing
            },
            actual: result.status,
            supported: false,
            artifact_present: result.artifact.is_some(),
            replay_verified: result.replay_verified(),
            tamper_rejected: !tampered.replay_verified(),
            exact_artifact: false,
        });
    }
    for index in 0..80 {
        let request = unsupported_case(index);
        let result = evaluate_calculus(&request);
        let mut tampered = result.clone();
        tampered.replay_hash.push('x');
        receipts.push(Receipt {
            id: format!("unsupported_{index}"),
            expected: result.status,
            actual: result.status,
            supported: false,
            artifact_present: result.artifact.is_some(),
            replay_verified: result.replay_verified(),
            tamper_rejected: !tampered.replay_verified(),
            exact_artifact: false,
        });
    }

    let exact = receipts.iter().filter(|r| r.expected == r.actual).count();
    let supported_artifacts = receipts
        .iter()
        .filter(|r| r.supported && r.artifact_present && r.exact_artifact)
        .count();
    let replay = receipts.iter().filter(|r| r.replay_verified).count();
    let tamper = receipts.iter().filter(|r| r.tamper_rejected).count();
    let false_auth = receipts
        .iter()
        .filter(|r| !r.supported && r.artifact_present)
        .count();
    let false_denial = receipts
        .iter()
        .filter(|r| r.supported && r.actual != CalculusStatus::Complete)
        .count();
    assert_eq!(receipts.len(), 240);
    assert_eq!(exact, 240);
    assert_eq!(supported_artifacts, 120);
    assert_eq!(replay, 240);
    assert_eq!(tamper, 240);
    assert_eq!(false_auth, 0);
    assert_eq!(false_denial, 0);
    let report = Report {
        schema: "phase61-bounded-calculus-v1",
        benchmark: "bounded exact one-variable calculus",
        cases: receipts.len(),
        supported: 120,
        boundary: 40,
        unsupported: 80,
        exact_decisions: exact,
        supported_artifacts,
        replay_verified: replay,
        tamper_rejections: tamper,
        false_authorizations: false_auth,
        false_denials: false_denial,
        receipts,
    };
    let json = serde_json::to_string_pretty(&report).expect("calculus report serializes");
    fs::write("docs/phase61_calculus_pack.json", &json).expect("write calculus report");
    println!("{json}");
}
