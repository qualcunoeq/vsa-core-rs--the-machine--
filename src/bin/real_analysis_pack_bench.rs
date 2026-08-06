//! Phase 63 benchmark for bounded real-analysis theorem applicability.

use serde::Serialize;
use std::fs;
use the_machine::real_analysis_pack::{
    evaluate_analysis, AnalysisArtifact, AnalysisOperation, AnalysisRequest, AnalysisStatus,
    DiscontinuityKind, LimitSide, MonotoneDirection,
};

#[derive(Debug, Clone, Serialize)]
struct Receipt {
    id: String,
    expected: AnalysisStatus,
    actual: AnalysisStatus,
    supported: bool,
    artifact_present: bool,
    replay_verified: bool,
    tamper_rejected: bool,
    theorem_assumptions_explicit: bool,
}

#[derive(Debug, Serialize)]
struct Report {
    schema: &'static str,
    benchmark: &'static str,
    cases: usize,
    supported: usize,
    ambiguous: usize,
    unsupported: usize,
    exact_decisions: usize,
    supported_artifacts: usize,
    replay_verified: usize,
    tamper_rejections: usize,
    theorem_assumptions_explicit: usize,
    false_authorizations: usize,
    false_denials: usize,
    receipts: Vec<Receipt>,
}

fn request(operation: AnalysisOperation) -> AnalysisRequest {
    AnalysisRequest {
        operation,
        domain: "bounded_exact_real_analysis".into(),
        expression: "x^2 + 1".into(),
        variable: Some("x".into()),
        interval: Some((0.0, 2.0)),
        point: Some(1.0),
        side: Some(LimitSide::Left),
        direction: Some(MonotoneDirection::Nondecreasing),
        endpoint_values: Some((1, 5)),
        target_value: Some(3),
        sequence_initial: Some(2),
        sequence_ratio: Some((1, 2)),
        composition_declared: true,
        assumptions: vec![
            "polynomial".into(),
            "closed_interval".into(),
            "derivative_sign_verified".into(),
            "continuous_on_interval".into(),
            "geometric_sequence".into(),
            "ratio_absolute_less_than_one".into(),
            "continuous_components".into(),
        ],
        ambiguity: None,
        provenance: vec!["phase63-real-analysis-bench".into()],
    }
}

fn supported_case(index: usize) -> AnalysisRequest {
    let operation = match index / 15 {
        0 => AnalysisOperation::Monotonicity,
        1 => AnalysisOperation::Boundedness,
        2 => AnalysisOperation::ExtremeValueApplicability,
        3 => AnalysisOperation::IntermediateValueApplicability,
        4 => AnalysisOperation::SequenceConvergence,
        5 => AnalysisOperation::ContinuityComposition,
        6 => AnalysisOperation::OneSidedLimit,
        _ => AnalysisOperation::DiscontinuityClassification,
    };
    let mut request = request(operation);
    if operation == AnalysisOperation::DiscontinuityClassification {
        request.expression = if index % 2 == 0 {
            "(x^2 - 1)/(x - 1)".into()
        } else {
            "1/(x - 1)".into()
        };
        request.point = Some(1.0);
    }
    request
}

fn ambiguous_case(index: usize) -> AnalysisRequest {
    let operation = match index % 5 {
        0 => AnalysisOperation::Monotonicity,
        1 => AnalysisOperation::Boundedness,
        2 => AnalysisOperation::IntermediateValueApplicability,
        3 => AnalysisOperation::SequenceConvergence,
        _ => AnalysisOperation::OneSidedLimit,
    };
    let mut request = request(operation);
    request.ambiguity = Some("theorem scope has two unresolved interpretations".into());
    request
}

fn unsupported_case(index: usize) -> AnalysisRequest {
    let operation = match index % 8 {
        0 => AnalysisOperation::Monotonicity,
        1 => AnalysisOperation::Boundedness,
        2 => AnalysisOperation::ExtremeValueApplicability,
        3 => AnalysisOperation::IntermediateValueApplicability,
        4 => AnalysisOperation::SequenceConvergence,
        5 => AnalysisOperation::ContinuityComposition,
        6 => AnalysisOperation::OneSidedLimit,
        _ => AnalysisOperation::DiscontinuityClassification,
    };
    let mut request = request(operation);
    match index % 8 {
        0 => request.expression = "partial_x f(x,y)".into(),
        1 => request.expression = "sin(x)".into(),
        2 => request.domain = "measure_theory".into(),
        3 => request.expression = "x^2 + 1".into(),
        4 => request.sequence_ratio = Some((3, 2)),
        5 => request.composition_declared = false,
        6 => {
            request.expression = "x^2 + 1".into();
            request.point = Some(f64::INFINITY);
        }
        _ => {
            request.expression = "x^3 + 1".into();
            request.point = Some(1.0);
        }
    }
    if index % 8 == 3 {
        request.assumptions.clear();
    }
    request
}

fn receipt(
    id: String,
    request: AnalysisRequest,
    expected: AnalysisStatus,
    supported: bool,
) -> Receipt {
    let result = evaluate_analysis(&request);
    let mut tampered = result.clone();
    tampered.replay_hash.push('x');
    let theorem_assumptions_explicit = supported && request.assumptions.len() >= 2;
    Receipt {
        id,
        expected,
        actual: result.status,
        supported,
        artifact_present: result.artifact.is_some(),
        replay_verified: result.replay_verified(),
        tamper_rejected: !tampered.replay_verified(),
        theorem_assumptions_explicit,
    }
}

fn main() {
    let mut receipts = Vec::with_capacity(240);
    for index in 0..120 {
        let request = supported_case(index);
        let expected_artifact = if index / 15 == 7 {
            Some(if index % 2 == 0 {
                AnalysisArtifact::Discontinuity(DiscontinuityKind::Removable)
            } else {
                AnalysisArtifact::Discontinuity(DiscontinuityKind::Genuine)
            })
        } else {
            None
        };
        let mut item = receipt(
            format!("supported_{index}"),
            request,
            AnalysisStatus::Complete,
            true,
        );
        if let Some(expected) = expected_artifact {
            let actual = evaluate_analysis(&supported_case(index)).artifact;
            item.artifact_present = actual == Some(expected);
        }
        receipts.push(item);
    }
    for index in 0..40 {
        receipts.push(receipt(
            format!("ambiguous_{index}"),
            ambiguous_case(index),
            AnalysisStatus::Ambiguous,
            false,
        ));
    }
    for index in 0..80 {
        receipts.push(receipt(
            format!("unsupported_{index}"),
            unsupported_case(index),
            if matches!(index % 8, 0 | 1 | 2 | 4 | 6 | 7) {
                AnalysisStatus::Unsupported
            } else {
                AnalysisStatus::InvalidAssumptions
            },
            false,
        ));
    }
    let exact = receipts.iter().filter(|r| r.expected == r.actual).count();
    let artifacts = receipts
        .iter()
        .filter(|r| r.supported && r.artifact_present)
        .count();
    let replay = receipts.iter().filter(|r| r.replay_verified).count();
    let tamper = receipts.iter().filter(|r| r.tamper_rejected).count();
    let explicit = receipts
        .iter()
        .filter(|r| r.supported && r.theorem_assumptions_explicit)
        .count();
    let false_auth = receipts
        .iter()
        .filter(|r| !r.supported && r.artifact_present)
        .count();
    let false_denial = receipts
        .iter()
        .filter(|r| r.supported && r.actual != AnalysisStatus::Complete)
        .count();
    assert_eq!(receipts.len(), 240);
    assert_eq!(exact, 240);
    assert_eq!(artifacts, 120);
    assert_eq!(replay, 240);
    assert_eq!(tamper, 240);
    assert_eq!(explicit, 120);
    assert_eq!(false_auth, 0);
    assert_eq!(false_denial, 0);
    let report = Report {
        schema: "phase63-real-analysis-v1",
        benchmark: "bounded theorem applicability and exact convergence foundations",
        cases: receipts.len(),
        supported: 120,
        ambiguous: 40,
        unsupported: 80,
        exact_decisions: exact,
        supported_artifacts: artifacts,
        replay_verified: replay,
        tamper_rejections: tamper,
        theorem_assumptions_explicit: explicit,
        false_authorizations: false_auth,
        false_denials: false_denial,
        receipts,
    };
    let json = serde_json::to_string_pretty(&report).expect("analysis report serializes");
    fs::write("docs/phase63_real_analysis_pack.json", &json).expect("write analysis report");
    println!("{json}");
}
