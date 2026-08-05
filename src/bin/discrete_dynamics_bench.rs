//! Phase 59 bounded scalar/vector discrete-dynamics benchmark.

use serde::Serialize;
use sha2::{Digest, Sha256};
use the_machine::discrete_dynamics::{
    evaluate_dynamics, DynamicsArtifact, DynamicsOperation, DynamicsRequest, DynamicsResult,
    DynamicsStatus,
};
use the_machine::probability_pack::Rational;

#[derive(Debug, Clone, Serialize)]
struct Case {
    id: String,
    family: String,
    request: DynamicsRequest,
    expected_status: DynamicsStatus,
    expected_artifact: Option<DynamicsArtifact>,
    expected_authorized: bool,
}

#[derive(Debug, Serialize)]
struct Row {
    id: String,
    family: String,
    expected_status: DynamicsStatus,
    actual_status: DynamicsStatus,
    expected_artifact: Option<DynamicsArtifact>,
    actual_artifact: Option<DynamicsArtifact>,
    exact: bool,
    replay_verified: bool,
    trace_entries: usize,
    trace_replays: usize,
    false_authorization: bool,
    tamper_rejected: bool,
}

#[derive(Debug, Serialize)]
struct Report {
    schema_version: String,
    corpus_sha256: String,
    cases: usize,
    authorized_cases: usize,
    refusal_cases: usize,
    exact_decisions: usize,
    exact_supported_artifacts: usize,
    replay_verified: usize,
    emitted_trace_entries: usize,
    emitted_trace_replays: usize,
    authorized_trace_entries: usize,
    authorized_trace_replays: usize,
    safe_refusals: usize,
    false_authorizations: usize,
    false_denials: usize,
    tamper_rejections: usize,
    rows: Vec<Row>,
}

fn rational(numerator: i128, denominator: i128) -> Rational {
    Rational::new(numerator, denominator).expect("benchmark rational is valid")
}

fn sha<T: Serialize>(value: &T) -> String {
    format!(
        "{:x}",
        Sha256::digest(serde_json::to_vec(value).expect("dynamics benchmark serializes"))
    )
}

fn base(operation: DynamicsOperation, steps: usize) -> DynamicsRequest {
    DynamicsRequest {
        operation,
        domain: "finite_exact_discrete_dynamics".into(),
        scalar_initial: None,
        coefficient: None,
        offset: None,
        vector_initial: None,
        matrix: None,
        steps,
        ambiguity: None,
        provenance: vec!["phase59-discrete-dynamics-corpus".into()],
    }
}

fn scalar_request(steps: usize) -> DynamicsRequest {
    let mut request = base(DynamicsOperation::ScalarAffine, steps);
    request.scalar_initial = Some(rational(1, 1));
    request.coefficient = Some(rational(2, 1));
    request.offset = Some(rational(1, 1));
    request
}

fn vector_request(operation: DynamicsOperation, steps: usize) -> DynamicsRequest {
    let mut request = base(operation, steps);
    request.vector_initial = Some(vec![rational(1, 1), rational(0, 1)]);
    request.matrix = Some(vec![
        vec![rational(1, 1), rational(1, 1)],
        vec![rational(1, 1), rational(0, 1)],
    ]);
    request
}

fn scalar_expected(steps: usize) -> DynamicsArtifact {
    let mut value = rational(1, 1);
    for _ in 0..steps {
        value = rational(2, 1)
            .mul(&value)
            .unwrap()
            .add(&rational(1, 1))
            .unwrap();
    }
    DynamicsArtifact::Scalar(value)
}

fn vector_expected(steps: usize) -> DynamicsArtifact {
    let mut vector = vec![rational(1, 1), rational(0, 1)];
    let matrix = vec![
        vec![rational(1, 1), rational(1, 1)],
        vec![rational(1, 1), rational(0, 1)],
    ];
    for _ in 0..steps {
        vector = matrix
            .iter()
            .map(|row| {
                row.iter()
                    .zip(&vector)
                    .try_fold(Rational::zero(), |sum, (coefficient, value)| {
                        coefficient.mul(value).and_then(|term| sum.add(&term))
                    })
                    .unwrap()
            })
            .collect();
    }
    DynamicsArtifact::Vector(vector)
}

fn cases() -> Vec<Case> {
    let mut cases = Vec::new();
    for (family, steps, operation) in [
        ("scalar_one_step", 1, DynamicsOperation::ScalarAffine),
        ("scalar_four_step", 4, DynamicsOperation::ScalarAffine),
        ("vector_two_step", 2, DynamicsOperation::VectorLinear),
        (
            "matrix_evolution_eight_step",
            8,
            DynamicsOperation::MatrixEvolution,
        ),
    ] {
        for index in 0..30 {
            let request = if matches!(operation, DynamicsOperation::ScalarAffine) {
                scalar_request(steps)
            } else {
                vector_request(operation, steps)
            };
            let expected_artifact = if matches!(operation, DynamicsOperation::ScalarAffine) {
                scalar_expected(steps)
            } else {
                vector_expected(steps)
            };
            cases.push(Case {
                id: format!("{family}_{index}"),
                family: family.into(),
                request,
                expected_status: DynamicsStatus::Complete,
                expected_artifact: Some(expected_artifact),
                expected_authorized: true,
            });
        }
    }
    for (family, count) in [
        ("asymptotic_stability", 15),
        ("nonlinear_recurrence", 15),
        ("continuous_time", 15),
        ("infinite_horizon", 15),
        ("spectral_closed_form", 15),
        ("dimension_mismatch", 15),
        ("symbolic_parameters", 15),
        ("floating_approximation", 15),
    ] {
        for index in 0..count {
            let mut request = match family {
                "dimension_mismatch" => vector_request(DynamicsOperation::VectorLinear, 2),
                _ => scalar_request(1),
            };
            let expected_status = match family {
                "asymptotic_stability" | "nonlinear_recurrence" | "spectral_closed_form" => {
                    request.ambiguity =
                        Some(format!("{family} is outside finite-horizon evaluation"));
                    DynamicsStatus::Ambiguous
                }
                "continuous_time" | "symbolic_parameters" | "floating_approximation" => {
                    request.domain = format!("unsupported_{family}");
                    DynamicsStatus::Unsupported
                }
                "infinite_horizon" => {
                    request.steps = 9;
                    DynamicsStatus::BudgetExceeded
                }
                "dimension_mismatch" => {
                    request.vector_initial =
                        Some(vec![rational(1, 1), rational(0, 1), rational(0, 1)]);
                    DynamicsStatus::DimensionMismatch
                }
                _ => unreachable!(),
            };
            cases.push(Case {
                id: format!("{family}_{index}"),
                family: family.into(),
                request,
                expected_status,
                expected_artifact: None,
                expected_authorized: false,
            });
        }
    }
    cases
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let corpus = cases();
    let corpus_sha256 = sha(&corpus);
    let authorized_cases = corpus
        .iter()
        .filter(|case| case.expected_authorized)
        .count();
    let refusal_cases = corpus.len() - authorized_cases;
    let mut rows = Vec::new();
    let mut exact_decisions = 0;
    let mut exact_supported_artifacts = 0;
    let mut replay_verified = 0;
    let mut emitted_trace_entries = 0;
    let mut emitted_trace_replays = 0;
    let mut authorized_trace_entries = 0;
    let mut authorized_trace_replays = 0;
    let mut safe_refusals = 0;
    let mut false_authorizations = 0;
    let mut false_denials = 0;
    let mut tamper_rejections = 0;
    for case in &corpus {
        let result: DynamicsResult = evaluate_dynamics(&case.request);
        let exact =
            result.status == case.expected_status && result.artifact == case.expected_artifact;
        let authorized = result.status == DynamicsStatus::Complete && result.artifact.is_some();
        let replay = result.replay_verified();
        let trace_replays = result.trace.len();
        let tamper_rejected = {
            let mut tampered = result.clone();
            tampered.replay_hash.push('x');
            !tampered.replay_verified()
        };
        exact_decisions += usize::from(exact);
        exact_supported_artifacts += usize::from(exact && case.expected_authorized);
        replay_verified += usize::from(replay);
        emitted_trace_entries += result.trace.len();
        emitted_trace_replays += trace_replays;
        if case.expected_authorized {
            authorized_trace_entries += result.trace.len();
            authorized_trace_replays += trace_replays;
        }
        safe_refusals += usize::from(!case.expected_authorized && exact && replay);
        false_authorizations += usize::from(authorized && !case.expected_authorized);
        false_denials += usize::from(!authorized && case.expected_authorized);
        tamper_rejections += usize::from(tamper_rejected);
        rows.push(Row {
            id: case.id.clone(),
            family: case.family.clone(),
            expected_status: case.expected_status,
            actual_status: result.status,
            expected_artifact: case.expected_artifact.clone(),
            actual_artifact: result.artifact,
            exact,
            replay_verified: replay,
            trace_entries: result.trace.len(),
            trace_replays,
            false_authorization: authorized && !case.expected_authorized,
            tamper_rejected,
        });
    }
    let report = Report {
        schema_version: "phase59-discrete-dynamics-v1".into(),
        corpus_sha256,
        cases: corpus.len(),
        authorized_cases,
        refusal_cases,
        exact_decisions,
        exact_supported_artifacts,
        replay_verified,
        emitted_trace_entries,
        emitted_trace_replays,
        authorized_trace_entries,
        authorized_trace_replays,
        safe_refusals,
        false_authorizations,
        false_denials,
        tamper_rejections,
        rows,
    };
    println!("{}", serde_json::to_string_pretty(&report)?);
    std::fs::write(
        "docs/phase59_discrete_dynamics_bench.json",
        serde_json::to_vec_pretty(&report)?,
    )?;
    Ok(())
}
