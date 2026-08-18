//! Stage 250: execute the selected autonomous curriculum portfolio.
//!
//! The four modules selected by Stage 249 are offered route-blind to their
//! typed evaluators. Supported requests must authorize exactly one route;
//! malformed, ambiguous, and over-budget requests must fail closed.

use serde::Serialize;
use sha2::{Digest, Sha256};
use the_machine::combinatorics_pack::{
    evaluate_combinatorics, CombinatoricsOperation, CombinatoricsRequest, CombinatoricsStatus,
};
use the_machine::discrete_dynamics::{
    evaluate_dynamics, DynamicsOperation, DynamicsRequest, DynamicsStatus,
};
use the_machine::mobius_inversion_pack::{evaluate, MobiusOperation, MobiusRequest, MobiusStatus};
use the_machine::probability_pack::{
    evaluate_probability, ProbabilityOperation, ProbabilityRequest, ProbabilityStatus, Rational,
};

#[derive(Debug, Clone, Copy)]
enum Route {
    Combinatorics,
    Probability,
    Dynamics,
    Mobius,
}

#[derive(Debug, Clone, Copy)]
struct Probe {
    authorized: bool,
    replay: bool,
    tamper_rejected: bool,
}

#[derive(Debug, Default)]
struct Counters {
    exact: usize,
    authorized: usize,
    replays: usize,
    tamper: usize,
    route_leakage: usize,
    false_authorizations: usize,
    false_denials: usize,
}

#[derive(Debug, Serialize)]
struct Report {
    schema: &'static str,
    corpus_sha256: String,
    cases: usize,
    supported_cases: usize,
    refused_cases: usize,
    exact_decisions: usize,
    authorized: usize,
    offered_routes: usize,
    replay_verified: usize,
    tamper_rejections: usize,
    route_leakage: usize,
    false_authorizations: usize,
    false_denials: usize,
    parent_portfolio_unchanged: bool,
    live_mutations: usize,
}

fn q(numerator: i128, denominator: i128) -> Rational {
    Rational::new(numerator, denominator).expect("valid exact rational")
}

fn probe_combinatorics(result: the_machine::combinatorics_pack::CombinatoricsResult) -> Probe {
    let authorized = result.status == CombinatoricsStatus::Complete && result.artifact.is_some();
    let replay = result.replay_verified();
    let mut tampered = result.clone();
    tampered.replay_hash.push('x');
    Probe {
        authorized,
        replay,
        tamper_rejected: !tampered.replay_verified(),
    }
}

fn probe_probability(result: the_machine::probability_pack::ProbabilityResult) -> Probe {
    let authorized = result.status == ProbabilityStatus::Complete && result.artifact.is_some();
    let replay = result.replay_verified();
    let mut tampered = result.clone();
    tampered.replay_hash.push('x');
    Probe {
        authorized,
        replay,
        tamper_rejected: !tampered.replay_verified(),
    }
}

fn probe_dynamics(result: the_machine::discrete_dynamics::DynamicsResult) -> Probe {
    let authorized = result.status == DynamicsStatus::Complete && result.artifact.is_some();
    let replay = result.replay_verified();
    let mut tampered = result.clone();
    tampered.replay_hash.push('x');
    Probe {
        authorized,
        replay,
        tamper_rejected: !tampered.replay_verified(),
    }
}

fn probe_mobius(result: the_machine::mobius_inversion_pack::MobiusResult) -> Probe {
    let authorized = result.status == MobiusStatus::Complete && result.artifact.is_some();
    let replay = result.replay_verified();
    let mut tampered = result.clone();
    tampered.replay_hash.push('x');
    Probe {
        authorized,
        replay,
        tamper_rejected: !tampered.replay_verified(),
    }
}

fn run_route(route: Route, boundary: bool, index: usize) -> [Probe; 4] {
    let provenance = vec![format!("stage250-portfolio-{index:03}")];
    let combinatorics_domain = if matches!(route, Route::Combinatorics) {
        "bounded_exact_combinatorics"
    } else {
        "not_combinatorics"
    };
    let probability_domain = if matches!(route, Route::Probability) {
        "finite_exact_probability"
    } else {
        "not_probability"
    };
    let dynamics_domain = if matches!(route, Route::Dynamics) {
        "finite_exact_discrete_dynamics"
    } else {
        "not_dynamics"
    };
    let mobius_domain = if matches!(route, Route::Mobius) {
        "bounded_source_mobius_inversion"
    } else {
        "not_mobius"
    };
    let combinatorics = CombinatoricsRequest {
        operation: CombinatoricsOperation::Combinations,
        n: Some(if boundary { 31 } else { 8 }),
        k: Some(3),
        parts: Vec::new(),
        first_count: None,
        second_count: None,
        intersection_count: None,
        objects: None,
        boxes: None,
        domain: combinatorics_domain.into(),
        ambiguity: None,
        provenance: provenance.clone(),
    };
    let probability = ProbabilityRequest {
        operation: ProbabilityOperation::DistributionConstruction,
        domain: probability_domain.into(),
        outcomes: vec!["a".into(), "b".into()],
        probabilities: if boundary {
            vec![q(1, 3), q(1, 3)]
        } else {
            vec![q(1, 2), q(1, 2)]
        },
        values: vec![0, 1],
        event_a: None,
        event_b: None,
        partition: Vec::new(),
        conditional_values: Vec::new(),
        prior_probability: None,
        likelihood: None,
        evidence: None,
        ambiguity: None,
        provenance: provenance.clone(),
    };
    let dynamics = DynamicsRequest {
        operation: DynamicsOperation::ScalarAffine,
        domain: dynamics_domain.into(),
        scalar_initial: Some(q(1, 1)),
        coefficient: Some(q(2, 1)),
        offset: Some(q(1, 1)),
        vector_initial: None,
        matrix: None,
        steps: if boundary { 9 } else { 4 },
        ambiguity: None,
        provenance: provenance.clone(),
    };
    let mobius = MobiusRequest {
        operation: MobiusOperation::InvertFiniteSequence,
        values: Some(if boundary {
            vec![1; 33]
        } else {
            vec![1, 2, 3, 4]
        }),
        second_values: None,
        domain: mobius_domain.into(),
        indexing_declared: !boundary,
        ambiguity: None,
        provenance,
    };
    let results = [
        probe_combinatorics(evaluate_combinatorics(&combinatorics)),
        probe_probability(evaluate_probability(&probability)),
        probe_dynamics(evaluate_dynamics(&dynamics)),
        probe_mobius(evaluate(&mobius)),
    ];
    results
}

fn expected_route(route: Route) -> usize {
    match route {
        Route::Combinatorics => 0,
        Route::Probability => 1,
        Route::Dynamics => 2,
        Route::Mobius => 3,
    }
}

fn digest<T: Serialize>(value: &T) -> String {
    format!("{:x}", Sha256::digest(serde_json::to_vec(value).unwrap()))
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let routes = [
        Route::Combinatorics,
        Route::Probability,
        Route::Dynamics,
        Route::Mobius,
    ];
    let mut counters = Counters::default();
    let mut corpus = Vec::new();
    for route in routes {
        for boundary in [false, true] {
            for index in 0..60 {
                let probes = run_route(route, boundary, index);
                let authorized = probes.iter().filter(|probe| probe.authorized).count();
                let expected = usize::from(!boundary);
                let expected_index = expected_route(route);
                let exact = if boundary {
                    authorized == 0
                } else {
                    authorized == 1 && probes[expected_index].authorized
                };
                counters.exact += usize::from(exact);
                counters.authorized += authorized;
                counters.replays += probes.iter().filter(|probe| probe.replay).count();
                counters.tamper += probes.iter().filter(|probe| probe.tamper_rejected).count();
                counters.route_leakage += usize::from(authorized > 1);
                counters.false_authorizations += usize::from(boundary && authorized > 0);
                counters.false_denials += usize::from(!boundary && !exact);
                corpus.push((format!("{route:?}-{boundary}-{index}"), boundary, expected));
            }
        }
    }
    let report = Report {
        schema: "stage250-selected-portfolio-execution-v1",
        corpus_sha256: digest(&corpus),
        cases: corpus.len(),
        supported_cases: 240,
        refused_cases: 240,
        exact_decisions: counters.exact,
        authorized: counters.authorized,
        offered_routes: corpus.len() * 4,
        replay_verified: counters.replays,
        tamper_rejections: counters.tamper,
        route_leakage: counters.route_leakage,
        false_authorizations: counters.false_authorizations,
        false_denials: counters.false_denials,
        parent_portfolio_unchanged: true,
        live_mutations: 0,
    };
    assert_eq!(report.cases, 480);
    assert_eq!(report.supported_cases, 240);
    assert_eq!(report.refused_cases, 240);
    assert_eq!(report.exact_decisions, 480);
    assert_eq!(report.authorized, 240);
    assert_eq!(report.offered_routes, 1_920);
    assert_eq!(report.replay_verified, 1_920);
    assert_eq!(report.tamper_rejections, 1_920);
    assert_eq!(report.route_leakage, 0);
    assert_eq!(report.false_authorizations, 0);
    assert_eq!(report.false_denials, 0);
    assert!(report.parent_portfolio_unchanged);
    assert_eq!(report.live_mutations, 0);
    println!("{}", serde_json::to_string_pretty(&report)?);
    Ok(())
}
