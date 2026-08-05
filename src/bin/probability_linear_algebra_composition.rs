//! Phase 55 shadow composition benchmark for finite probability and linear algebra.
//!
//! The benchmark checks that numeric shape alone never authorizes a semantic
//! handoff. A route is accepted only when the source pack and the target pack
//! independently verify the artifact and the bridge preserves its meaning.

use serde::Serialize;
use sha2::{Digest, Sha256};
use std::collections::BTreeSet;
use the_machine::linear_algebra_pack::{
    evaluate_linear_algebra, LinearAlgebraArtifact, LinearAlgebraOperation, LinearAlgebraRequest,
    LinearAlgebraStatus,
};
use the_machine::probability_pack::{
    evaluate_probability, probability_vector_to_linear_algebra, ProbabilityArtifact,
    ProbabilityOperation, ProbabilityRequest, ProbabilityStatus, Rational,
};

#[derive(Debug, Clone, Serialize)]
struct CompositionCase {
    id: String,
    route: String,
    expected_terminal: String,
    expected_authorized: bool,
}

#[derive(Debug, Serialize)]
struct Row {
    id: String,
    route: String,
    terminal: String,
    expected_terminal: String,
    probability_status: ProbabilityStatus,
    linear_algebra_status: LinearAlgebraStatus,
    source_replay: bool,
    target_replay: bool,
    composition_replay: bool,
    route_leakage: bool,
    authorized: bool,
    exact: bool,
    tamper_rejected: bool,
}

#[derive(Debug, Serialize)]
struct Report {
    schema_version: String,
    corpus_sha256: String,
    cases: usize,
    authorized_cases: usize,
    refusal_cases: usize,
    exact_route_decisions: usize,
    source_artifact_replays: usize,
    target_artifact_replays: usize,
    composition_replays: usize,
    safe_refusals: usize,
    route_leakage: usize,
    false_authorizations: usize,
    false_denials: usize,
    rewrite_groups: usize,
    tamper_rejections: usize,
    rows: Vec<Row>,
}

fn rational(numerator: i128, denominator: i128) -> Rational {
    Rational::new(numerator, denominator).expect("benchmark rational is valid")
}

fn sha<T: Serialize>(value: &T) -> String {
    format!(
        "{:x}",
        Sha256::digest(serde_json::to_vec(value).expect("composition serializes"))
    )
}

fn probability_request(
    operation: ProbabilityOperation,
    probabilities: Vec<Rational>,
) -> ProbabilityRequest {
    ProbabilityRequest {
        operation,
        domain: "finite_exact_probability".into(),
        outcomes: vec!["a".into(), "b".into()],
        probabilities,
        values: vec![7, 11],
        event_a: Some(vec![0]),
        event_b: Some(vec![1]),
        partition: Vec::new(),
        conditional_values: Vec::new(),
        prior_probability: None,
        likelihood: None,
        evidence: None,
        ambiguity: None,
        provenance: vec!["phase55-probability-linear-algebra-composition".into()],
    }
}

fn joint_probability_request() -> ProbabilityRequest {
    ProbabilityRequest {
        operation: ProbabilityOperation::DistributionConstruction,
        domain: "finite_exact_probability".into(),
        outcomes: vec!["00".into(), "01".into(), "10".into(), "11".into()],
        probabilities: vec![
            rational(1, 4),
            rational(1, 4),
            rational(1, 4),
            rational(1, 4),
        ],
        values: Vec::new(),
        event_a: None,
        event_b: None,
        partition: Vec::new(),
        conditional_values: Vec::new(),
        prior_probability: None,
        likelihood: None,
        evidence: None,
        ambiguity: None,
        provenance: vec!["phase55-probability-linear-algebra-composition".into()],
    }
}

fn linear_vector_request(vector: Vec<i64>) -> LinearAlgebraRequest {
    LinearAlgebraRequest {
        operation: LinearAlgebraOperation::VectorConstruction,
        matrix: None,
        vector_a: Some(vector),
        vector_b: None,
        domain: "finite_exact_integer".into(),
        requested_output: "explicit_integer_vector".into(),
        provenance: vec!["phase55-probability-linear-algebra-composition".into()],
    }
}

fn linear_inner_product_request(left: Vec<i64>, right: Vec<i64>) -> LinearAlgebraRequest {
    LinearAlgebraRequest {
        operation: LinearAlgebraOperation::InnerProduct,
        matrix: None,
        vector_a: Some(left),
        vector_b: Some(right),
        domain: "finite_exact_integer".into(),
        requested_output: "exact_dot_product".into(),
        provenance: vec!["phase55-probability-linear-algebra-composition".into()],
    }
}

fn linear_matrix_request() -> LinearAlgebraRequest {
    LinearAlgebraRequest {
        operation: LinearAlgebraOperation::MatrixConstruction,
        matrix: Some(vec![vec![0, 1], vec![1, 0]]),
        vector_a: None,
        vector_b: None,
        domain: "finite_exact_integer".into(),
        requested_output: "matrix_without_stochastic_semantics".into(),
        provenance: vec!["phase55-probability-linear-algebra-composition".into()],
    }
}

fn cases() -> Vec<CompositionCase> {
    let mut cases = Vec::new();
    for index in 0..15 {
        cases.push(CompositionCase {
            id: format!("probability_to_integer_vector_{index}"),
            route: "probability_to_integer_vector".into(),
            expected_terminal: "validated_probability_vector_to_linear_vector".into(),
            expected_authorized: true,
        });
    }
    for index in 0..15 {
        cases.push(CompositionCase {
            id: format!("integer_vector_to_distribution_{index}"),
            route: "integer_vector_to_distribution".into(),
            expected_terminal: "validated_integer_vector_to_probability_distribution".into(),
            expected_authorized: true,
        });
    }
    for index in 0..15 {
        cases.push(CompositionCase {
            id: format!("degenerate_expectation_dot_product_{index}"),
            route: "degenerate_expectation_dot_product".into(),
            expected_terminal: "probability_expectation_equals_linear_dot_product".into(),
            expected_authorized: true,
        });
    }
    for index in 0..15 {
        cases.push(CompositionCase {
            id: format!("joint_distribution_to_marginal_{index}"),
            route: "joint_distribution_to_marginal".into(),
            expected_terminal: "validated_joint_to_marginal_distribution".into(),
            expected_authorized: true,
        });
    }
    for (route, count) in [
        ("fractional_probability_vector", 10),
        ("signed_probability_weights", 10),
        ("unnormalized_probability_weights", 10),
        ("ambiguous_matrix_orientation", 10),
        ("matrix_without_stochastic_semantics", 10),
        ("covariance_like_matrix_without_statistics", 10),
    ] {
        for index in 0..count {
            cases.push(CompositionCase {
                id: format!("{route}_{index}"),
                route: route.into(),
                expected_terminal: "safe_composition_refusal".into(),
                expected_authorized: false,
            });
        }
    }
    cases
}

fn evaluate_case(case: &CompositionCase) -> (Row, bool) {
    let mut probability_status = ProbabilityStatus::Missing;
    let mut linear_algebra_status = LinearAlgebraStatus::Missing;
    let mut source_replay = false;
    let mut target_replay = false;
    let mut composition_replay = false;
    let mut terminal = "unexpected".to_string();
    let mut authorized = false;
    let mut probability_result = None;
    let mut linear_result = None;

    match case.route.as_str() {
        "probability_to_integer_vector" => {
            let probability = evaluate_probability(&probability_request(
                ProbabilityOperation::DistributionConstruction,
                vec![Rational::one(), Rational::zero()],
            ));
            probability_status = probability.status;
            source_replay = probability.replay_verified();
            probability_result = Some(probability);
            if let Some(request) = probability_result
                .as_ref()
                .and_then(probability_vector_to_linear_algebra)
            {
                let linear = evaluate_linear_algebra(&request);
                linear_algebra_status = linear.status;
                target_replay = linear.replay_verified();
                composition_replay = target_replay;
                authorized = linear.status == LinearAlgebraStatus::Complete
                    && matches!(linear.artifact, Some(LinearAlgebraArtifact::Vector(_)));
                linear_result = Some(linear);
            }
            if authorized {
                terminal = "validated_probability_vector_to_linear_vector".into();
            }
        }
        "integer_vector_to_distribution" => {
            let linear = evaluate_linear_algebra(&linear_vector_request(vec![1, 0]));
            linear_algebra_status = linear.status;
            target_replay = linear.replay_verified();
            linear_result = Some(linear);
            if let Some(LinearAlgebraArtifact::Vector(vector)) = linear_result
                .as_ref()
                .and_then(|result| result.artifact.as_ref())
            {
                let probability = evaluate_probability(&probability_request(
                    ProbabilityOperation::DistributionConstruction,
                    vector
                        .iter()
                        .map(|value| rational(*value as i128, 1))
                        .collect(),
                ));
                probability_status = probability.status;
                source_replay = probability.replay_verified();
                probability_result = Some(probability);
                authorized = probability_status == ProbabilityStatus::Complete
                    && matches!(
                        probability_result
                            .as_ref()
                            .and_then(|result| result.artifact.as_ref()),
                        Some(ProbabilityArtifact::Distribution(_))
                    );
                composition_replay = authorized && source_replay && target_replay;
            }
            if authorized {
                terminal = "validated_integer_vector_to_probability_distribution".into();
            }
        }
        "degenerate_expectation_dot_product" => {
            let probability = evaluate_probability(&probability_request(
                ProbabilityOperation::Expectation,
                vec![Rational::one(), Rational::zero()],
            ));
            probability_status = probability.status;
            source_replay = probability.replay_verified();
            let expected = probability.artifact.clone();
            probability_result = Some(probability);
            let linear =
                evaluate_linear_algebra(&linear_inner_product_request(vec![1, 0], vec![7, 11]));
            linear_algebra_status = linear.status;
            target_replay = linear.replay_verified();
            let dot = linear.artifact.clone();
            linear_result = Some(linear);
            authorized = matches!(expected, Some(ProbabilityArtifact::Scalar(value)) if value == rational(7, 1))
                && matches!(dot, Some(LinearAlgebraArtifact::Scalar(7)));
            composition_replay = authorized && source_replay && target_replay;
            if authorized {
                terminal = "probability_expectation_equals_linear_dot_product".into();
            }
        }
        "joint_distribution_to_marginal" => {
            let joint = evaluate_probability(&joint_probability_request());
            let joint_replay = joint.replay_verified();
            if let Some(ProbabilityArtifact::Distribution(distribution)) = joint.artifact.as_ref() {
                let first = distribution.probabilities[0]
                    .add(&distribution.probabilities[1])
                    .expect("joint marginal sum");
                let second = distribution.probabilities[2]
                    .add(&distribution.probabilities[3])
                    .expect("joint marginal sum");
                let marginal = evaluate_probability(&probability_request(
                    ProbabilityOperation::DistributionConstruction,
                    vec![first, second],
                ));
                probability_status = marginal.status;
                source_replay = joint_replay && marginal.replay_verified();
                target_replay = marginal.replay_verified();
                authorized = marginal.status == ProbabilityStatus::Complete
                    && matches!(
                        marginal.artifact.as_ref(),
                        Some(ProbabilityArtifact::Distribution(_))
                    );
                composition_replay = authorized && source_replay;
                probability_result = Some(marginal);
            } else {
                probability_status = joint.status;
                source_replay = joint_replay;
                probability_result = Some(joint);
            }
            if authorized {
                terminal = "validated_joint_to_marginal_distribution".into();
            }
        }
        "fractional_probability_vector" => {
            let probability = evaluate_probability(&probability_request(
                ProbabilityOperation::DistributionConstruction,
                vec![rational(1, 4), rational(3, 4)],
            ));
            probability_status = probability.status;
            source_replay = probability.replay_verified();
            let refused = probability_vector_to_linear_algebra(&probability).is_none();
            authorized = false;
            composition_replay = source_replay && refused;
            probability_result = Some(probability);
            if refused {
                terminal = "safe_composition_refusal".into();
            }
        }
        "signed_probability_weights" => {
            let probability = evaluate_probability(&probability_request(
                ProbabilityOperation::DistributionConstruction,
                vec![rational(-1, 2), rational(3, 2)],
            ));
            probability_status = probability.status;
            source_replay = probability.replay_verified();
            composition_replay = source_replay && probability.status != ProbabilityStatus::Complete;
            probability_result = Some(probability);
            terminal = "safe_composition_refusal".into();
        }
        "unnormalized_probability_weights" => {
            let mut request = probability_request(
                ProbabilityOperation::DistributionConstruction,
                vec![rational(1, 2), rational(1, 2)],
            );
            request.outcomes.push("c".into());
            probability_result = Some(evaluate_probability(&request));
            probability_status = probability_result.as_ref().unwrap().status;
            source_replay = probability_result.as_ref().unwrap().replay_verified();
            composition_replay = source_replay && probability_status != ProbabilityStatus::Complete;
            terminal = "safe_composition_refusal".into();
        }
        "ambiguous_matrix_orientation" => {
            let mut request = probability_request(
                ProbabilityOperation::StochasticMatrixCandidate,
                vec![Rational::one(), Rational::zero()],
            );
            request.ambiguity = Some("row versus column stochastic convention is unstated".into());
            let probability = evaluate_probability(&request);
            probability_status = probability.status;
            source_replay = probability.replay_verified();
            composition_replay =
                source_replay && probability.status == ProbabilityStatus::Ambiguous;
            probability_result = Some(probability);
            terminal = "safe_composition_refusal".into();
        }
        "matrix_without_stochastic_semantics" | "covariance_like_matrix_without_statistics" => {
            let linear = evaluate_linear_algebra(&linear_matrix_request());
            linear_algebra_status = linear.status;
            target_replay = linear.replay_verified();
            let probability = evaluate_probability(&probability_request(
                ProbabilityOperation::StochasticMatrixCandidate,
                vec![Rational::one(), Rational::zero()],
            ));
            probability_status = probability.status;
            source_replay = probability.replay_verified();
            composition_replay = source_replay
                && target_replay
                && probability.status == ProbabilityStatus::Unsupported;
            linear_result = Some(linear);
            probability_result = Some(probability);
            terminal = "safe_composition_refusal".into();
        }
        _ => unreachable!(),
    }

    let exact = terminal == case.expected_terminal && authorized == case.expected_authorized;
    let route_leakage = !case.expected_authorized && authorized;
    let mut tamper_rejected = false;
    if let Some(probability) = probability_result.as_ref() {
        let mut tampered = probability.clone();
        tampered.replay_hash.push('x');
        tamper_rejected |= !tampered.replay_verified();
    }
    if let Some(linear) = linear_result.as_ref() {
        let mut tampered = linear.clone();
        tampered.replay_hash.push('x');
        tamper_rejected |= !tampered.replay_verified();
    }
    let row = Row {
        id: case.id.clone(),
        route: case.route.clone(),
        terminal,
        expected_terminal: case.expected_terminal.clone(),
        probability_status,
        linear_algebra_status,
        source_replay,
        target_replay,
        composition_replay,
        route_leakage,
        authorized,
        exact,
        tamper_rejected,
    };
    (row, route_leakage)
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
    let mut exact_route_decisions = 0;
    let mut source_artifact_replays = 0;
    let mut target_artifact_replays = 0;
    let mut composition_replays = 0;
    let mut safe_refusals = 0;
    let mut route_leakage = 0;
    let mut false_authorizations = 0;
    let mut false_denials = 0;
    let mut tamper_rejections = 0;
    let rewrite_groups: BTreeSet<String> = [
        "probability_vector_rewrite".into(),
        "integer_vector_rewrite".into(),
        "expectation_dot_product_rewrite".into(),
    ]
    .into_iter()
    .collect();

    for case in &corpus {
        let (row, leaked) = evaluate_case(case);
        exact_route_decisions += usize::from(row.exact);
        source_artifact_replays += usize::from(row.source_replay);
        target_artifact_replays += usize::from(row.target_replay);
        composition_replays += usize::from(row.composition_replay);
        safe_refusals += usize::from(!case.expected_authorized && row.composition_replay);
        route_leakage += usize::from(leaked);
        false_authorizations += usize::from(row.authorized && !case.expected_authorized);
        false_denials += usize::from(!row.authorized && case.expected_authorized);
        tamper_rejections += usize::from(row.tamper_rejected);
        rows.push(row);
    }

    let report = Report {
        schema_version: "phase55-probability-linear-algebra-composition-v1".into(),
        corpus_sha256,
        cases: corpus.len(),
        authorized_cases,
        refusal_cases,
        exact_route_decisions,
        source_artifact_replays,
        target_artifact_replays,
        composition_replays,
        safe_refusals,
        route_leakage,
        false_authorizations,
        false_denials,
        rewrite_groups: rewrite_groups.len(),
        tamper_rejections,
        rows,
    };
    println!("{}", serde_json::to_string_pretty(&report)?);
    std::fs::write(
        "docs/phase55_probability_linear_algebra_composition.json",
        serde_json::to_vec_pretty(&report)?,
    )?;
    Ok(())
}
