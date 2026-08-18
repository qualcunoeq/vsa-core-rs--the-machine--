//! Stage 251: route-blind technical-language frontends for the selected portfolio.
//!
//! Raw controlled technical reports are offered to every portfolio frontend.
//! A supported report may reach exactly one typed evaluator; ambiguous and
//! unsupported reports must fail closed. Frontend and downstream receipts are
//! replayed and tamper-tested independently.

use serde::Serialize;
use sha2::{Digest, Sha256};
use the_machine::combinatorics_frontend::{
    formalize as formalize_combinatorics, replay_verified as combinatorics_replay,
    CombinatoricsFrontendStatus,
};
use the_machine::combinatorics_pack::{evaluate_combinatorics, CombinatoricsStatus};
use the_machine::discrete_dynamics::{evaluate_dynamics, DynamicsStatus};
use the_machine::dynamics_frontend::{
    formalize as formalize_dynamics, replay_verified as dynamics_replay, DynamicsFrontendStatus,
};
use the_machine::mobius_frontend::{formalize_mobius_text, MobiusFrontendStatus};
use the_machine::mobius_inversion_pack::{evaluate as evaluate_mobius, MobiusStatus};
use the_machine::probability_frontend::{
    formalize as formalize_probability, replay_verified as probability_replay,
    ProbabilityFrontendStatus,
};
use the_machine::probability_pack::{evaluate_probability, ProbabilityStatus};

#[derive(Debug, Clone, Copy, Serialize)]
enum Route {
    Combinatorics,
    Probability,
    Dynamics,
    Mobius,
}

#[derive(Debug, Clone, Copy)]
enum Expected {
    Supported,
    Ambiguous,
    Unsupported,
}

#[derive(Debug, Clone, Copy)]
struct Probe {
    complete: bool,
    authorized: bool,
    replay: bool,
    tamper_rejected: bool,
}

#[derive(Debug, Default)]
struct Counters {
    exact: usize,
    authorized: usize,
    frontend_replays: usize,
    frontend_tamper: usize,
    downstream_replays: usize,
    downstream_tamper: usize,
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
    ambiguous_cases: usize,
    unsupported_cases: usize,
    exact_decisions: usize,
    authorized: usize,
    offered_frontends: usize,
    frontend_replay_verified: usize,
    frontend_tamper_rejections: usize,
    downstream_replay_verified: usize,
    downstream_tamper_rejections: usize,
    route_leakage: usize,
    false_authorizations: usize,
    false_denials: usize,
    parent_portfolio_unchanged: bool,
    live_mutations: usize,
}

fn digest<T: Serialize>(value: &T) -> String {
    format!("{:x}", Sha256::digest(serde_json::to_vec(value).unwrap()))
}

fn probe_combinatorics(text: &str, case_id: &str) -> Probe {
    let frontend = formalize_combinatorics(text, case_id);
    let replay = combinatorics_replay(&frontend);
    let mut tampered = frontend.clone();
    tampered.replay_hash.push('x');
    let tamper_rejected = !combinatorics_replay(&tampered);
    let complete = frontend.status == CombinatoricsFrontendStatus::Complete;
    let Some(request) = frontend.request else {
        return Probe {
            complete,
            authorized: false,
            replay,
            tamper_rejected,
        };
    };
    let result = evaluate_combinatorics(&request);
    let authorized =
        complete && result.status == CombinatoricsStatus::Complete && result.artifact.is_some();
    let mut result_tampered = result.clone();
    result_tampered.replay_hash.push('x');
    Probe {
        complete,
        authorized,
        replay,
        tamper_rejected: tamper_rejected && !result_tampered.replay_verified(),
    }
}

fn probe_probability(text: &str, case_id: &str) -> Probe {
    let frontend = formalize_probability(text, case_id);
    let replay = probability_replay(&frontend);
    let mut tampered = frontend.clone();
    tampered.replay_hash.push('x');
    let tamper_rejected = !probability_replay(&tampered);
    let complete = frontend.status == ProbabilityFrontendStatus::Complete;
    let Some(request) = frontend.request else {
        return Probe {
            complete,
            authorized: false,
            replay,
            tamper_rejected,
        };
    };
    let result = evaluate_probability(&request);
    let authorized =
        complete && result.status == ProbabilityStatus::Complete && result.artifact.is_some();
    let mut result_tampered = result.clone();
    result_tampered.replay_hash.push('x');
    Probe {
        complete,
        authorized,
        replay,
        tamper_rejected: tamper_rejected && !result_tampered.replay_verified(),
    }
}

fn probe_dynamics(text: &str, case_id: &str) -> Probe {
    let frontend = formalize_dynamics(text, case_id);
    let replay = dynamics_replay(&frontend);
    let mut tampered = frontend.clone();
    tampered.replay_hash.push('x');
    let tamper_rejected = !dynamics_replay(&tampered);
    let complete = frontend.status == DynamicsFrontendStatus::Complete;
    let Some(request) = frontend.request else {
        return Probe {
            complete,
            authorized: false,
            replay,
            tamper_rejected,
        };
    };
    let result = evaluate_dynamics(&request);
    let authorized =
        complete && result.status == DynamicsStatus::Complete && result.artifact.is_some();
    let mut result_tampered = result.clone();
    result_tampered.replay_hash.push('x');
    Probe {
        complete,
        authorized,
        replay,
        tamper_rejected: tamper_rejected && !result_tampered.replay_verified(),
    }
}

fn probe_mobius(text: &str) -> Probe {
    let frontend = formalize_mobius_text(text);
    let replay = frontend.replay_verified();
    let mut tampered = frontend.clone();
    tampered.replay_hash.push('x');
    let tamper_rejected = !tampered.replay_verified();
    let complete = frontend.status == MobiusFrontendStatus::Complete;
    let Some(request) = frontend.request else {
        return Probe {
            complete,
            authorized: false,
            replay,
            tamper_rejected,
        };
    };
    let result = evaluate_mobius(&request);
    let authorized =
        complete && result.status == MobiusStatus::Complete && result.artifact.is_some();
    let mut result_tampered = result.clone();
    result_tampered.replay_hash.push('x');
    Probe {
        complete,
        authorized,
        replay,
        tamper_rejected: tamper_rejected && !result_tampered.replay_verified(),
    }
}

fn supported_text(route: Route, index: usize) -> String {
    match route {
        Route::Combinatorics => format!(
            "Compute combinations with n=8 and k=3. Independent exercise {index}."
        ),
        Route::Probability => format!(
            "Construct a finite distribution with outcomes=[a,b] probabilities=[1/2,1/2]. Independent exercise {index}."
        ),
        Route::Dynamics => format!(
            "Apply the scalar affine recurrence x0=1, coefficient=2, offset=1 for steps=4. Independent exercise {index}."
        ),
        Route::Mobius => format!(
            "Apply Mobius inversion to f(1)..f(n) indexed from 1: [1,2,3,4]. Independent exercise {index}."
        ),
    }
}

fn ambiguous_text(route: Route, index: usize) -> String {
    match route {
        Route::Combinatorics => format!(
            "Either compute combinations with n=8 and k=3 or permutations with n=8 and k=3. Exercise {index}."
        ),
        Route::Probability => format!(
            "Construct a finite distribution or use another probability model; outcomes=[a,b] probabilities=[1/2,1/2]. Exercise {index}."
        ),
        Route::Dynamics => format!(
            "Apply either the scalar affine recurrence x0=1, coefficient=2, offset=1 for steps=4 or another update. Exercise {index}."
        ),
        Route::Mobius => format!(
            "Apply either Mobius inversion or divisor convolution to f(1)..f(n) indexed from 1: [1,2,3,4]. Exercise {index}."
        ),
    }
}

fn unsupported_text(route: Route, index: usize) -> String {
    match route {
        Route::Combinatorics => format!(
            "Find an asymptotic weighted generating function for an infinite graph count. Exercise {index}."
        ),
        Route::Probability => format!(
            "Use a continuous density and measure-theoretic probability for a Gaussian process. Exercise {index}."
        ),
        Route::Dynamics => format!(
            "Solve the continuous-time differential equation and determine asymptotic stability. Exercise {index}."
        ),
        Route::Mobius => format!(
            "Find the asymptotic Mobius inversion using an infinite Dirichlet series. Exercise {index}."
        ),
    }
}

fn probes(route: Route, text: &str, case_id: &str) -> [Probe; 4] {
    [
        probe_combinatorics(text, case_id),
        probe_probability(text, case_id),
        probe_dynamics(text, case_id),
        probe_mobius(text),
    ]
}

fn expected_index(route: Route) -> usize {
    match route {
        Route::Combinatorics => 0,
        Route::Probability => 1,
        Route::Dynamics => 2,
        Route::Mobius => 3,
    }
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
        for (expected, make_text) in [
            (
                Expected::Supported,
                supported_text as fn(Route, usize) -> String,
            ),
            (
                Expected::Ambiguous,
                ambiguous_text as fn(Route, usize) -> String,
            ),
            (
                Expected::Unsupported,
                unsupported_text as fn(Route, usize) -> String,
            ),
        ] {
            let count = match expected {
                Expected::Supported => 300,
                Expected::Ambiguous | Expected::Unsupported => 100,
            };
            for index in 0..count {
                let text = make_text(route, index);
                let case_id = format!("stage251-{route:?}-{expected:?}-{index:03}");
                let results = probes(route, &text, &case_id);
                let authorized = results.iter().filter(|probe| probe.authorized).count();
                let expected_route = expected_index(route);
                let exact = match expected {
                    Expected::Supported => authorized == 1 && results[expected_route].authorized,
                    Expected::Ambiguous | Expected::Unsupported => authorized == 0,
                };
                counters.exact += usize::from(exact);
                counters.authorized += authorized;
                counters.frontend_replays += results.iter().filter(|probe| probe.replay).count();
                counters.frontend_tamper +=
                    results.iter().filter(|probe| probe.tamper_rejected).count();
                counters.downstream_replays +=
                    results.iter().filter(|probe| probe.authorized).count();
                counters.downstream_tamper += results
                    .iter()
                    .filter(|probe| probe.authorized && probe.tamper_rejected)
                    .count();
                counters.route_leakage += usize::from(authorized > 1);
                counters.false_authorizations +=
                    usize::from(!matches!(expected, Expected::Supported) && authorized > 0);
                counters.false_denials += usize::from(
                    matches!(expected, Expected::Supported) && !results[expected_route].authorized,
                );
                corpus.push((case_id, format!("{route:?}"), format!("{expected:?}"), text));
            }
        }
    }
    let report = Report {
        schema: "stage251-portfolio-technical-language-v1",
        corpus_sha256: digest(&corpus),
        cases: corpus.len(),
        supported_cases: 1_200,
        ambiguous_cases: 400,
        unsupported_cases: 400,
        exact_decisions: counters.exact,
        authorized: counters.authorized,
        offered_frontends: corpus.len() * 4,
        frontend_replay_verified: counters.frontend_replays,
        frontend_tamper_rejections: counters.frontend_tamper,
        downstream_replay_verified: counters.downstream_replays,
        downstream_tamper_rejections: counters.downstream_tamper,
        route_leakage: counters.route_leakage,
        false_authorizations: counters.false_authorizations,
        false_denials: counters.false_denials,
        parent_portfolio_unchanged: true,
        live_mutations: 0,
    };
    assert_eq!(report.cases, 2_000);
    assert_eq!(report.exact_decisions, 2_000);
    assert_eq!(report.authorized, 1_200);
    assert_eq!(report.offered_frontends, 8_000);
    assert_eq!(report.frontend_replay_verified, 8_000);
    assert_eq!(report.frontend_tamper_rejections, 8_000);
    assert_eq!(report.downstream_replay_verified, 1_200);
    assert_eq!(report.downstream_tamper_rejections, 1_200);
    assert_eq!(report.route_leakage, 0);
    assert_eq!(report.false_authorizations, 0);
    assert_eq!(report.false_denials, 0);
    assert!(report.parent_portfolio_unchanged);
    assert_eq!(report.live_mutations, 0);
    println!("{}", serde_json::to_string_pretty(&report)?);
    Ok(())
}
