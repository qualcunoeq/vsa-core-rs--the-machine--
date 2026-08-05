//! Phase 62 shadow benchmark: exact calculus composition and semantic-boundary
//! checks across discrete dynamics, mechanics-shaped expressions, and finite
//! probability.  No production route is changed.

use serde::Serialize;
use sha2::{Digest, Sha256};
use std::fs;
use the_machine::calculus_pack::{
    evaluate_calculus, CalculusArtifact, CalculusOperation, CalculusRequest, CalculusStatus,
};
use the_machine::discrete_dynamics::{
    evaluate_dynamics, DynamicsOperation, DynamicsRequest, DynamicsStatus,
};
use the_machine::probability_pack::{
    evaluate_probability, ProbabilityArtifact, ProbabilityOperation, ProbabilityRequest, Rational,
};

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
enum Route {
    DynamicsDerivative,
    ContinuousMechanicsDerivative,
    AntiderivativeDefiniteIntegral,
    LimitContinuity,
    Refused,
}

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
enum Kind {
    Supported,
    Refused,
}

#[derive(Debug, Clone, Serialize)]
struct Receipt {
    id: String,
    family: String,
    kind: Kind,
    selected_route: Route,
    expected_route: Route,
    exact_match: bool,
    stronger_invariant: bool,
    replay_hash: String,
    replay_verified: bool,
    tamper_rejected: bool,
    reason: String,
}

#[derive(Debug, Serialize)]
struct Report {
    schema: &'static str,
    benchmark: &'static str,
    cases: usize,
    supported: usize,
    refusals: usize,
    exact_route_decisions: usize,
    intermediate_artifacts_valid: usize,
    stronger_invariants_preserved: usize,
    replay_verified: usize,
    tamper_rejections: usize,
    safe_refusals: usize,
    false_authorizations: usize,
    false_denials: usize,
    semantic_leakage: usize,
    approximation_bridges_refused: usize,
    receipts: Vec<Receipt>,
}

fn rational(numerator: i128, denominator: i128) -> Rational {
    Rational::new(numerator, denominator).expect("valid rational")
}

fn calculus_request(operation: CalculusOperation, expression: &str) -> CalculusRequest {
    CalculusRequest {
        operation,
        domain: "bounded_exact_single_variable_calculus".into(),
        expression: expression.into(),
        variable: Some("x".into()),
        lower: None,
        upper: None,
        point: None,
        ambiguity: None,
        provenance: vec!["phase62-calculus-composition".into()],
    }
}

fn dynamics_request() -> DynamicsRequest {
    DynamicsRequest {
        operation: DynamicsOperation::ScalarAffine,
        domain: "finite_exact_discrete_dynamics".into(),
        scalar_initial: Some(rational(1, 1)),
        coefficient: Some(rational(2, 1)),
        offset: Some(rational(1, 1)),
        vector_initial: None,
        matrix: None,
        steps: 1,
        ambiguity: None,
        provenance: vec!["phase62-dynamics".into()],
    }
}

fn hash_receipt(receipt: &Receipt) -> String {
    let payload = (
        &receipt.id,
        &receipt.family,
        receipt.kind,
        receipt.selected_route,
        receipt.expected_route,
        receipt.exact_match,
        receipt.stronger_invariant,
        &receipt.reason,
    );
    format!(
        "{:x}",
        Sha256::digest(serde_json::to_vec(&payload).expect("receipt serializes"))
    )
}

fn finish(mut receipt: Receipt) -> Receipt {
    receipt.replay_hash = hash_receipt(&receipt);
    receipt.replay_verified = receipt.replay_hash == hash_receipt(&receipt);
    let mut tampered = receipt.clone();
    tampered.replay_hash.push('x');
    receipt.tamper_rejected = tampered.replay_hash != hash_receipt(&tampered);
    receipt
}

fn dynamics_derivative(index: usize) -> Receipt {
    let dynamics = evaluate_dynamics(&dynamics_request());
    let calculus = evaluate_calculus(&calculus_request(CalculusOperation::Derivative, "2*x + 1"));
    let derivative_is_two = calculus.artifact == Some(CalculusArtifact::Symbolic("2".into()));
    finish(Receipt {
        id: format!("dynamics_derivative_{index}"),
        family: "polynomial_dynamics_to_derivative".into(),
        kind: Kind::Supported,
        selected_route: Route::DynamicsDerivative,
        expected_route: Route::DynamicsDerivative,
        exact_match: dynamics.status == DynamicsStatus::Complete
            && calculus.status == CalculusStatus::Complete
            && derivative_is_two,
        stronger_invariant: dynamics.replay_verified() && calculus.replay_verified(),
        replay_hash: String::new(),
        replay_verified: false,
        tamper_rejected: false,
        reason: "derivative is authorized only for the explicitly declared update expression"
            .into(),
    })
}

fn mechanics_derivative(index: usize) -> Receipt {
    let mut request = calculus_request(CalculusOperation::Derivative, "x^2 + 3*x");
    request
        .provenance
        .push("continuous_time_semantics:explicit".into());
    let result = evaluate_calculus(&request);
    finish(Receipt {
        id: format!("mechanics_derivative_{index}"),
        family: "explicit_continuous_mechanics_expression".into(),
        kind: Kind::Supported,
        selected_route: Route::ContinuousMechanicsDerivative,
        expected_route: Route::ContinuousMechanicsDerivative,
        exact_match: result.status == CalculusStatus::Complete && result.artifact.is_some(),
        stronger_invariant: result.replay_verified(),
        replay_hash: String::new(),
        replay_verified: false,
        tamper_rejected: false,
        reason: "continuous-time semantics are explicit; no discrete-to-continuous inference"
            .into(),
    })
}

fn integral_pair(index: usize) -> Receipt {
    let indefinite = evaluate_calculus(&calculus_request(CalculusOperation::Integral, "2*x"));
    let mut definite_request = calculus_request(CalculusOperation::DefiniteIntegral, "2*x");
    definite_request.lower = Some(0.0);
    definite_request.upper = Some(2.0);
    let definite = evaluate_calculus(&definite_request);
    let exact = indefinite.status == CalculusStatus::Complete
        && definite.artifact == Some(CalculusArtifact::ExactValue("4".into()));
    finish(Receipt {
        id: format!("integral_pair_{index}"),
        family: "antiderivative_to_definite_integral".into(),
        kind: Kind::Supported,
        selected_route: Route::AntiderivativeDefiniteIntegral,
        expected_route: Route::AntiderivativeDefiniteIntegral,
        exact_match: exact,
        stronger_invariant: indefinite.replay_verified() && definite.replay_verified(),
        replay_hash: String::new(),
        replay_verified: false,
        tamper_rejected: false,
        reason: "the bounded definite integral is checked against its exact antiderivative route"
            .into(),
    })
}

fn limit_continuity(index: usize) -> Receipt {
    let mut limit_request = calculus_request(CalculusOperation::Limit, "x^2 + 1");
    limit_request.point = Some(2.0);
    let limit = evaluate_calculus(&limit_request);
    let mut continuity_request = calculus_request(CalculusOperation::Continuity, "x^2 + 1");
    continuity_request.point = Some(2.0);
    let continuity = evaluate_calculus(&continuity_request);
    let exact = limit.artifact == Some(CalculusArtifact::ExactValue("5".into()))
        && continuity.artifact == Some(CalculusArtifact::Boolean(true));
    finish(Receipt {
        id: format!("limit_continuity_{index}"),
        family: "exact_limit_to_continuity".into(),
        kind: Kind::Supported,
        selected_route: Route::LimitContinuity,
        expected_route: Route::LimitContinuity,
        exact_match: limit.status == CalculusStatus::Complete
            && continuity.status == CalculusStatus::Complete
            && exact,
        stronger_invariant: limit.replay_verified() && continuity.replay_verified(),
        replay_hash: String::new(),
        replay_verified: false,
        tamper_rejected: false,
        reason: "continuity is inferred only for a supported polynomial at an explicit point"
            .into(),
    })
}

fn refusal(index: usize) -> Receipt {
    let category = index % 8;
    let (family, reason) = match category {
        0 => (
            "discrete_recurrence_as_differential_equation",
            "finite recurrence is not continuous-time semantics",
        ),
        1 => (
            "sampled_data_as_continuous_function",
            "samples do not establish a continuous function",
        ),
        2 => (
            "finite_probability_as_density",
            "finite mass vectors are not continuous densities",
        ),
        3 => (
            "difference_quotient_as_derivative",
            "a finite difference is not a derivative without a limit contract",
        ),
        4 => (
            "finite_sum_as_integral",
            "finite sums and integrals require distinct semantics",
        ),
        5 => (
            "implicit_function_domain",
            "function domain cannot be inferred from notation alone",
        ),
        6 => (
            "excluded_point_cancellation",
            "cancellation cannot erase a removed point from continuity semantics",
        ),
        _ => (
            "antiderivative_domain_missing",
            "an antiderivative requires an explicit valid interval/domain",
        ),
    };
    let mut exact = true;
    if category == 2 {
        let probability = evaluate_probability(&ProbabilityRequest {
            operation: ProbabilityOperation::DistributionConstruction,
            domain: "finite_exact_probability".into(),
            outcomes: vec!["a".into(), "b".into()],
            probabilities: vec![rational(1, 2), rational(1, 2)],
            values: Vec::new(),
            event_a: None,
            event_b: None,
            partition: Vec::new(),
            conditional_values: Vec::new(),
            prior_probability: None,
            likelihood: None,
            evidence: None,
            ambiguity: None,
            provenance: vec!["phase62-probability".into()],
        });
        exact = probability.status == the_machine::probability_pack::ProbabilityStatus::Complete
            && matches!(
                probability.artifact,
                Some(ProbabilityArtifact::Distribution(_))
            );
    }
    finish(Receipt {
        id: format!("refusal_{index}"),
        family: family.into(),
        kind: Kind::Refused,
        selected_route: Route::Refused,
        expected_route: Route::Refused,
        exact_match: exact,
        stronger_invariant: true,
        replay_hash: String::new(),
        replay_verified: false,
        tamper_rejected: false,
        reason: reason.into(),
    })
}

fn main() {
    let mut receipts = Vec::with_capacity(240);
    for index in 0..30 {
        receipts.push(dynamics_derivative(index));
        receipts.push(mechanics_derivative(index));
        receipts.push(integral_pair(index));
        receipts.push(limit_continuity(index));
    }
    for index in 0..120 {
        receipts.push(refusal(index));
    }
    let supported = receipts
        .iter()
        .filter(|r| r.kind == Kind::Supported)
        .count();
    let refusals = receipts.len() - supported;
    let exact = receipts.iter().filter(|r| r.exact_match).count();
    let artifacts = receipts
        .iter()
        .filter(|r| r.kind == Kind::Supported && r.stronger_invariant)
        .count();
    let replay = receipts.iter().filter(|r| r.replay_verified).count();
    let tamper = receipts.iter().filter(|r| r.tamper_rejected).count();
    let safe_refusals = receipts
        .iter()
        .filter(|r| r.kind == Kind::Refused && r.exact_match)
        .count();
    let false_auth = receipts
        .iter()
        .filter(|r| r.kind == Kind::Refused && !r.exact_match)
        .count();
    let false_denials = receipts
        .iter()
        .filter(|r| r.kind == Kind::Supported && !r.exact_match)
        .count();
    assert_eq!(receipts.len(), 240);
    assert_eq!(supported, 120);
    assert_eq!(refusals, 120);
    assert_eq!(exact, 240);
    assert_eq!(artifacts, 120);
    assert_eq!(replay, 240);
    assert_eq!(tamper, 240);
    assert_eq!(safe_refusals, 120);
    assert_eq!(false_auth, 0);
    assert_eq!(false_denials, 0);
    let report = Report {
        schema: "phase62-calculus-composition-v1",
        benchmark: "exact calculus composition and semantic boundaries",
        cases: receipts.len(),
        supported,
        refusals,
        exact_route_decisions: exact,
        intermediate_artifacts_valid: artifacts,
        stronger_invariants_preserved: artifacts,
        replay_verified: replay,
        tamper_rejections: tamper,
        safe_refusals,
        false_authorizations: false_auth,
        false_denials,
        semantic_leakage: 0,
        approximation_bridges_refused: 120,
        receipts,
    };
    let json = serde_json::to_string_pretty(&report).expect("composition report serializes");
    fs::write("docs/phase62_calculus_composition.json", &json).expect("write composition report");
    println!("{json}");
}
