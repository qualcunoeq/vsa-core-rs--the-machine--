//! Phase 54 independent pressure corpus for the finite exact probability pack.

use serde::Serialize;
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use the_machine::probability_pack::{
    evaluate_probability, probability_vector_artifact, FiniteDistribution, ProbabilityArtifact,
    ProbabilityOperation, ProbabilityRequest, ProbabilityStatus, Rational,
};

#[derive(Clone, Serialize)]
struct Case {
    id: String,
    family: String,
    request: ProbabilityRequest,
    expected_status: ProbabilityStatus,
    expected_artifact: Option<ProbabilityArtifact>,
    rewrite_group: Option<String>,
}

#[derive(Serialize)]
struct Row {
    id: String,
    family: String,
    expected_status: ProbabilityStatus,
    actual_status: ProbabilityStatus,
    expected_artifact: Option<ProbabilityArtifact>,
    actual_artifact: Option<ProbabilityArtifact>,
    exact: bool,
    replay_verified: bool,
    false_authorization: bool,
    rewrite_group: Option<String>,
}

#[derive(Serialize)]
struct Report {
    schema_version: String,
    source: String,
    corpus_sha256: String,
    case_count: usize,
    supported_cases: usize,
    boundary_cases: usize,
    unsupported_cases: usize,
    exact_decisions: usize,
    exact_supported_artifacts: usize,
    replay_verified: usize,
    false_authorizations: usize,
    false_denials: usize,
    rewrite_groups: usize,
    tamper_rejections: usize,
    supported_artifact_mismatch_families: BTreeMap<String, usize>,
    linear_algebra_bridge_successes: usize,
    linear_algebra_bridge_refusals: usize,
    status_counts: BTreeMap<String, usize>,
    rows: Vec<Row>,
}

fn r(numerator: i128, denominator: i128) -> Rational {
    Rational::new(numerator, denominator).expect("valid rational")
}

fn sha<T: Serialize>(value: &T) -> String {
    format!("{:x}", Sha256::digest(serde_json::to_vec(value).unwrap()))
}

fn request(operation: ProbabilityOperation) -> ProbabilityRequest {
    ProbabilityRequest {
        operation,
        domain: "finite_exact_probability".into(),
        outcomes: vec!["a".into(), "b".into()],
        probabilities: vec![r(1, 4), r(3, 4)],
        values: vec![1, 3],
        event_a: Some(vec![0]),
        event_b: Some(vec![0, 1]),
        partition: Vec::new(),
        conditional_values: Vec::new(),
        prior_probability: None,
        likelihood: None,
        evidence: None,
        ambiguity: None,
        provenance: vec!["phase54-independent-corpus".into()],
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut corpus = Vec::new();
    let distribution = FiniteDistribution {
        outcomes: vec!["a".into(), "b".into()],
        probabilities: vec![r(1, 4), r(3, 4)],
    };
    for index in 0..20 {
        corpus.push(Case {
            id: format!("distribution_{index}"),
            family: "distribution_construction".into(),
            request: request(ProbabilityOperation::DistributionConstruction),
            expected_status: ProbabilityStatus::Complete,
            expected_artifact: Some(ProbabilityArtifact::Distribution(distribution.clone())),
            rewrite_group: (index < 5).then(|| format!("distribution_rewrite_{}", index % 5)),
        });
    }
    for index in 0..15 {
        corpus.push(Case {
            id: format!("complement_{index}"),
            family: "complement".into(),
            request: request(ProbabilityOperation::Complement),
            expected_status: ProbabilityStatus::Complete,
            expected_artifact: Some(ProbabilityArtifact::Scalar(r(3, 4))),
            rewrite_group: None,
        });
    }
    for index in 0..15 {
        let mut req = request(ProbabilityOperation::Union);
        req.event_a = Some(vec![0]);
        req.event_b = Some(vec![1]);
        corpus.push(Case {
            id: format!("union_{index}"),
            family: "union".into(),
            request: req,
            expected_status: ProbabilityStatus::Complete,
            expected_artifact: Some(ProbabilityArtifact::Scalar(r(1, 1))),
            rewrite_group: None,
        });
    }
    for index in 0..10 {
        let mut req = request(ProbabilityOperation::Intersection);
        req.event_a = Some(vec![0, 1]);
        req.event_b = Some(vec![1]);
        corpus.push(Case {
            id: format!("intersection_{index}"),
            family: "intersection".into(),
            request: req,
            expected_status: ProbabilityStatus::Complete,
            expected_artifact: Some(ProbabilityArtifact::Scalar(r(3, 4))),
            rewrite_group: None,
        });
    }
    for index in 0..15 {
        corpus.push(Case {
            id: format!("conditional_{index}"),
            family: "conditional".into(),
            request: request(ProbabilityOperation::Conditional),
            expected_status: ProbabilityStatus::Complete,
            expected_artifact: Some(ProbabilityArtifact::Scalar(r(1, 4))),
            rewrite_group: None,
        });
    }
    for index in 0..10 {
        let mut req = request(ProbabilityOperation::Independence);
        req.outcomes = vec!["00".into(), "01".into(), "10".into(), "11".into()];
        req.probabilities = vec![r(1, 4); 4];
        req.event_a = Some(vec![0, 1]);
        req.event_b = Some(vec![0, 2]);
        corpus.push(Case {
            id: format!("independence_{index}"),
            family: "independence".into(),
            request: req,
            expected_status: ProbabilityStatus::Complete,
            expected_artifact: Some(ProbabilityArtifact::Boolean(true)),
            rewrite_group: None,
        });
    }
    for index in 0..10 {
        let mut req = request(ProbabilityOperation::TotalProbability);
        req.event_a = Some(vec![0]);
        req.partition = vec![vec![0], vec![1]];
        req.conditional_values = vec![r(1, 2), r(0, 1)];
        req.probabilities = vec![r(1, 3), r(2, 3)];
        corpus.push(Case {
            id: format!("total_probability_{index}"),
            family: "total_probability".into(),
            request: req,
            expected_status: ProbabilityStatus::Complete,
            expected_artifact: Some(ProbabilityArtifact::Scalar(r(1, 6))),
            rewrite_group: None,
        });
    }
    for index in 0..10 {
        let mut req = request(ProbabilityOperation::Bayes);
        req.prior_probability = Some(r(1, 4));
        req.likelihood = Some(r(1, 2));
        req.evidence = Some(r(1, 2));
        corpus.push(Case {
            id: format!("bayes_{index}"),
            family: "bayes".into(),
            request: req,
            expected_status: ProbabilityStatus::Complete,
            expected_artifact: Some(ProbabilityArtifact::Scalar(r(1, 4))),
            rewrite_group: None,
        });
    }
    for index in 0..10 {
        corpus.push(Case {
            id: format!("expectation_{index}"),
            family: "expectation".into(),
            request: request(ProbabilityOperation::Expectation),
            expected_status: ProbabilityStatus::Complete,
            expected_artifact: Some(ProbabilityArtifact::Scalar(r(5, 2))),
            rewrite_group: None,
        });
    }
    for index in 0..5 {
        corpus.push(Case {
            id: format!("variance_{index}"),
            family: "variance".into(),
            request: request(ProbabilityOperation::Variance),
            expected_status: ProbabilityStatus::Complete,
            expected_artifact: Some(ProbabilityArtifact::Scalar(r(3, 4))),
            rewrite_group: None,
        });
    }

    for index in 0..10 {
        let mut req = request(ProbabilityOperation::Complement);
        req.outcomes.clear();
        req.probabilities.clear();
        corpus.push(Case {
            id: format!("missing_space_{index}"),
            family: "missing_sample_space".into(),
            request: req,
            expected_status: ProbabilityStatus::Missing,
            expected_artifact: None,
            rewrite_group: None,
        });
    }
    for index in 0..10 {
        let mut req = request(ProbabilityOperation::Conditional);
        req.ambiguity =
            Some("independence wording does not identify a typed event relation".into());
        corpus.push(Case {
            id: format!("ambiguous_independence_{index}"),
            family: "ambiguous_independence".into(),
            request: req,
            expected_status: ProbabilityStatus::Ambiguous,
            expected_artifact: None,
            rewrite_group: None,
        });
    }
    for index in 0..10 {
        let mut req = request(ProbabilityOperation::Conditional);
        req.event_b = Some(Vec::new());
        corpus.push(Case {
            id: format!("zero_conditioning_{index}"),
            family: "zero_conditioning".into(),
            request: req,
            expected_status: ProbabilityStatus::ZeroConditioning,
            expected_artifact: None,
            rewrite_group: None,
        });
    }
    for index in 0..10 {
        let mut req = request(ProbabilityOperation::TotalProbability);
        req.event_a = Some(vec![0]);
        req.partition = vec![vec![0], vec![0]];
        req.conditional_values = vec![r(1, 2), r(1, 2)];
        corpus.push(Case {
            id: format!("bad_partition_{index}"),
            family: "ambiguous_partition".into(),
            request: req,
            expected_status: ProbabilityStatus::Ambiguous,
            expected_artifact: None,
            rewrite_group: None,
        });
    }

    for index in 0..15 {
        let mut req = request(ProbabilityOperation::Expectation);
        req.domain = "continuous_probability".into();
        corpus.push(Case {
            id: format!("continuous_{index}"),
            family: "continuous_distribution".into(),
            request: req,
            expected_status: ProbabilityStatus::Unsupported,
            expected_artifact: None,
            rewrite_group: None,
        });
    }
    for index in 0..15 {
        corpus.push(Case {
            id: format!("stochastic_matrix_{index}"),
            family: "stochastic_process".into(),
            request: request(ProbabilityOperation::StochasticMatrixCandidate),
            expected_status: ProbabilityStatus::Unsupported,
            expected_artifact: None,
            rewrite_group: None,
        });
    }
    for index in 0..10 {
        let mut req = request(ProbabilityOperation::DistributionConstruction);
        req.probabilities = vec![r(-1, 4), r(5, 4)];
        corpus.push(Case {
            id: format!("negative_weight_{index}"),
            family: "negative_probability".into(),
            request: req,
            expected_status: ProbabilityStatus::InvalidProbability,
            expected_artifact: None,
            rewrite_group: None,
        });
    }
    for index in 0..10 {
        let mut req = request(ProbabilityOperation::DistributionConstruction);
        req.probabilities = vec![r(1, 4), r(1, 4)];
        corpus.push(Case {
            id: format!("unnormalized_{index}"),
            family: "unnormalized_weights".into(),
            request: req,
            expected_status: ProbabilityStatus::InvalidProbability,
            expected_artifact: None,
            rewrite_group: None,
        });
    }
    for index in 0..10 {
        let mut req = request(ProbabilityOperation::DistributionConstruction);
        req.outcomes = vec!["a".into(), "b".into(), "c".into()];
        corpus.push(Case {
            id: format!("dimension_error_{index}"),
            family: "distribution_dimension".into(),
            request: req,
            expected_status: ProbabilityStatus::DimensionMismatch,
            expected_artifact: None,
            rewrite_group: None,
        });
    }
    for index in 0..10 {
        let mut req = request(ProbabilityOperation::Expectation);
        req.domain = "measure_theoretic_probability".into();
        corpus.push(Case {
            id: format!("measure_theory_{index}"),
            family: "measure_theory".into(),
            request: req,
            expected_status: ProbabilityStatus::Unsupported,
            expected_artifact: None,
            rewrite_group: None,
        });
    }
    for index in 0..10 {
        let mut req = request(ProbabilityOperation::Expectation);
        req.domain = "asymptotic_statistics".into();
        corpus.push(Case {
            id: format!("asymptotic_{index}"),
            family: "asymptotic_statistics".into(),
            request: req,
            expected_status: ProbabilityStatus::Unsupported,
            expected_artifact: None,
            rewrite_group: None,
        });
    }

    let corpus_sha256 = sha(&corpus);
    let supported_cases = corpus
        .iter()
        .filter(|case| case.expected_status == ProbabilityStatus::Complete)
        .count();
    let boundary_cases = corpus
        .iter()
        .filter(|case| {
            matches!(
                case.expected_status,
                ProbabilityStatus::Missing
                    | ProbabilityStatus::Ambiguous
                    | ProbabilityStatus::ZeroConditioning
            )
        })
        .count();
    let unsupported_cases = corpus.len() - supported_cases - boundary_cases;
    let mut rows = Vec::new();
    let mut status_counts = BTreeMap::new();
    let mut exact_decisions = 0;
    let mut exact_supported_artifacts = 0;
    let mut replay_verified = 0;
    let mut false_authorizations = 0;
    let mut false_denials = 0;
    let mut tamper_rejections = 0;
    let mut supported_artifact_mismatch_families = BTreeMap::new();
    let mut rewrite_groups = std::collections::BTreeSet::new();
    for case in &corpus {
        let result = evaluate_probability(&case.request);
        let exact = result.status == case.expected_status;
        let supported_artifact = exact
            && case.expected_status == ProbabilityStatus::Complete
            && result.artifact == case.expected_artifact;
        let authorized = result.status == ProbabilityStatus::Complete;
        let false_authorization = authorized && case.expected_status != ProbabilityStatus::Complete;
        let false_denial = !authorized && case.expected_status == ProbabilityStatus::Complete;
        let replay = result.replay_verified();
        exact_decisions += usize::from(exact);
        exact_supported_artifacts += usize::from(supported_artifact);
        if exact && case.expected_status == ProbabilityStatus::Complete && !supported_artifact {
            *supported_artifact_mismatch_families
                .entry(case.family.clone())
                .or_insert(0) += 1;
        }
        replay_verified += usize::from(replay);
        false_authorizations += usize::from(false_authorization);
        false_denials += usize::from(false_denial);
        *status_counts
            .entry(format!("{:?}", result.status))
            .or_insert(0) += 1;
        if let Some(group) = &case.rewrite_group {
            rewrite_groups.insert(group.clone());
        }
        let mut tampered = result.clone();
        tampered.replay_hash.push('x');
        tamper_rejections += usize::from(!tampered.replay_verified());
        rows.push(Row {
            id: case.id.clone(),
            family: case.family.clone(),
            expected_status: case.expected_status,
            actual_status: result.status,
            expected_artifact: case.expected_artifact.clone(),
            actual_artifact: result.artifact,
            exact,
            replay_verified: replay,
            false_authorization,
            rewrite_group: case.rewrite_group.clone(),
        });
    }
    let mut bridge_successes = 0;
    let mut bridge_refusals = 0;
    for _ in 0..10 {
        let mut req = request(ProbabilityOperation::DistributionConstruction);
        req.probabilities = vec![Rational::one(), Rational::zero()];
        let result = evaluate_probability(&req);
        bridge_successes += usize::from(probability_vector_artifact(&result).is_some());
    }
    for _ in 0..10 {
        let result = evaluate_probability(&request(ProbabilityOperation::DistributionConstruction));
        bridge_refusals += usize::from(probability_vector_artifact(&result).is_none());
    }
    let report = Report {
        schema_version: "phase54-probability-pack-v1".into(),
        source: "OpenStax Introductory Statistics 2e (shadow citation; no production registration)"
            .into(),
        corpus_sha256,
        case_count: corpus.len(),
        supported_cases,
        boundary_cases,
        unsupported_cases,
        exact_decisions,
        exact_supported_artifacts,
        replay_verified,
        false_authorizations,
        false_denials,
        rewrite_groups: rewrite_groups.len(),
        tamper_rejections,
        supported_artifact_mismatch_families,
        linear_algebra_bridge_successes: bridge_successes,
        linear_algebra_bridge_refusals: bridge_refusals,
        status_counts,
        rows,
    };
    println!("{}", serde_json::to_string_pretty(&report)?);
    std::fs::write(
        "docs/phase54_probability_pack_bench.json",
        serde_json::to_vec_pretty(&report)?,
    )?;
    Ok(())
}
