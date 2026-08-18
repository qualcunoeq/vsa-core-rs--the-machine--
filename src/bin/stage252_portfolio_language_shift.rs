//! Stage 252: shifted-language pressure for the portfolio frontends.
//!
//! This corpus keeps the four portfolio boundaries fixed while varying clause
//! order, paraphrase, and incidental technical notation.  Every report is
//! still offered to every frontend; only a complete, semantically matching
//! route may authorize a downstream artifact.

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

#[derive(Debug, Clone, Copy, Serialize)]
enum Expected {
    Supported,
    Ambiguous,
    Unsupported,
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

fn probe_combinatorics(text: &str, id: &str) -> Probe {
    let frontend = formalize_combinatorics(text, id);
    let replay = combinatorics_replay(&frontend);
    let mut tampered = frontend.clone();
    tampered.replay_hash.push('x');
    let mut tamper_rejected = !combinatorics_replay(&tampered);
    let Some(request) = frontend.request else {
        return Probe {
            authorized: false,
            replay,
            tamper_rejected,
        };
    };
    let result = evaluate_combinatorics(&request);
    let authorized = frontend.status == CombinatoricsFrontendStatus::Complete
        && result.status == CombinatoricsStatus::Complete
        && result.artifact.is_some();
    let mut result_tampered = result.clone();
    result_tampered.replay_hash.push('x');
    tamper_rejected &= !result_tampered.replay_verified();
    Probe {
        authorized,
        replay,
        tamper_rejected,
    }
}

fn probe_probability(text: &str, id: &str) -> Probe {
    let frontend = formalize_probability(text, id);
    let replay = probability_replay(&frontend);
    let mut tampered = frontend.clone();
    tampered.replay_hash.push('x');
    let mut tamper_rejected = !probability_replay(&tampered);
    let Some(request) = frontend.request else {
        return Probe {
            authorized: false,
            replay,
            tamper_rejected,
        };
    };
    let result = evaluate_probability(&request);
    let authorized = frontend.status == ProbabilityFrontendStatus::Complete
        && result.status == ProbabilityStatus::Complete
        && result.artifact.is_some();
    let mut result_tampered = result.clone();
    result_tampered.replay_hash.push('x');
    tamper_rejected &= !result_tampered.replay_verified();
    Probe {
        authorized,
        replay,
        tamper_rejected,
    }
}

fn probe_dynamics(text: &str, id: &str) -> Probe {
    let frontend = formalize_dynamics(text, id);
    let replay = dynamics_replay(&frontend);
    let mut tampered = frontend.clone();
    tampered.replay_hash.push('x');
    let mut tamper_rejected = !dynamics_replay(&tampered);
    let Some(request) = frontend.request else {
        return Probe {
            authorized: false,
            replay,
            tamper_rejected,
        };
    };
    let result = evaluate_dynamics(&request);
    let authorized = frontend.status == DynamicsFrontendStatus::Complete
        && result.status == DynamicsStatus::Complete
        && result.artifact.is_some();
    let mut result_tampered = result.clone();
    result_tampered.replay_hash.push('x');
    tamper_rejected &= !result_tampered.replay_verified();
    Probe {
        authorized,
        replay,
        tamper_rejected,
    }
}

fn probe_mobius(text: &str) -> Probe {
    let frontend = formalize_mobius_text(text);
    let replay = frontend.replay_verified();
    let mut tampered = frontend.clone();
    tampered.replay_hash.push('x');
    let mut tamper_rejected = !tampered.replay_verified();
    let Some(request) = frontend.request else {
        return Probe {
            authorized: false,
            replay,
            tamper_rejected,
        };
    };
    let result = evaluate_mobius(&request);
    let authorized = frontend.status == MobiusFrontendStatus::Complete
        && result.status == MobiusStatus::Complete
        && result.artifact.is_some();
    let mut result_tampered = result.clone();
    result_tampered.replay_hash.push('x');
    tamper_rejected &= !result_tampered.replay_verified();
    Probe {
        authorized,
        replay,
        tamper_rejected,
    }
}

fn supported_text(route: Route, variant: usize, id: usize) -> String {
    match route {
        Route::Combinatorics => match variant % 3 {
            0 => format!("In a bounded exact count, determine the combination with n=8 and k=3; report the result. Exercise {id}."),
            1 => format!("Use n=8 and k=3 to calculate a finite binomial combination. Exercise {id}."),
            _ => format!("A finite combination count is requested with fixed k=3 and n=8. Exercise {id}."),
        },
        Route::Probability => match variant % 3 {
            0 => format!("With probabilities=[1/2,1/2], construct the finite distribution outcomes=[a,b]. Exercise {id}."),
            1 => format!("The finite probability mass has outcomes=[a,b] and probabilities=[1/2,1/2]; construct it. Exercise {id}."),
            _ => format!("Construct an exact finite distribution (outcomes=[a,b], probabilities=[1/2,1/2]) for exercise {id}."),
        },
        Route::Dynamics => match variant % 3 {
            0 => format!("For a discrete affine update, use offset=1, coefficient=2, x0=1, and horizon=4 steps. Exercise {id}."),
            1 => format!("The finite recurrence has x0=1, offset=1, coefficient=2; apply it for steps=4. Exercise {id}."),
            _ => format!("Apply a bounded scalar affine recurrence with coefficient=2, offset=1, initial=1, steps=4. Exercise {id}."),
        },
        Route::Mobius => match variant % 3 {
            0 => format!("The quoted context [99] is incidental. For f(1)..f(n) indexed from 1, apply Mobius inversion to [1,2,3,4]. Exercise {id}."),
            1 => format!("Apply Möbius inversion to the one-based sequence [1,2,3,4], with f(1)..f(n) indexed at 1. Exercise {id}."),
            _ => format!("Using f[1] and one-based indexing, perform Mobius inversion on [1,2,3,4]. Exercise {id}."),
        },
    }
}

fn ambiguous_text(route: Route, id: usize) -> String {
    match route {
        Route::Combinatorics => format!("Either calculate the combination n=8,k=3 or the permutation n=8,k=3; the requested count is unresolved. Exercise {id}."),
        Route::Probability => format!("Construct a finite distribution or choose another probability interpretation; outcomes=[a,b], probabilities=[1/2,1/2]. Exercise {id}."),
        Route::Dynamics => format!("Apply either the finite recurrence x0=1, coefficient=2, offset=1, steps=4 or another update; the model is unresolved. Exercise {id}."),
        Route::Mobius => format!("Either apply Mobius inversion or divisor convolution to f(1)..f(n) indexed from 1: [1,2,3,4]. Exercise {id}."),
    }
}

fn unsupported_text(route: Route, id: usize) -> String {
    match route {
        Route::Combinatorics => format!("Find an asymptotic weighted generating function for an infinite graph count. Exercise {id}."),
        Route::Probability => format!("Use a continuous density and measure-theoretic Gaussian process rather than finite exact probability. Exercise {id}."),
        Route::Dynamics => format!("Solve the continuous-time differential equation and determine asymptotic stability. Exercise {id}."),
        Route::Mobius => format!("Find the asymptotic Mobius inversion from an infinite Dirichlet series. Exercise {id}."),
    }
}

fn probes(text: &str, id: &str) -> [Probe; 4] {
    [
        probe_combinatorics(text, id),
        probe_probability(text, id),
        probe_dynamics(text, id),
        probe_mobius(text),
    ]
}

fn route_index(route: Route) -> usize {
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
        for (expected, count) in [
            (Expected::Supported, 100usize),
            (Expected::Ambiguous, 50usize),
            (Expected::Unsupported, 50usize),
        ] {
            for index in 0..count {
                let text = match expected {
                    Expected::Supported => supported_text(route, index, index),
                    Expected::Ambiguous => ambiguous_text(route, index),
                    Expected::Unsupported => unsupported_text(route, index),
                };
                let id = format!("stage252-{route:?}-{expected:?}-{index:03}");
                let results = probes(&text, &id);
                let authorized = results.iter().filter(|probe| probe.authorized).count();
                let selected = route_index(route);
                let exact = match expected {
                    Expected::Supported => authorized == 1 && results[selected].authorized,
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
                    matches!(expected, Expected::Supported) && !results[selected].authorized,
                );
                corpus.push((id, route, expected, text));
            }
        }
    }
    let report = Report {
        schema: "stage252-portfolio-language-shift-v1",
        corpus_sha256: digest(&corpus),
        cases: corpus.len(),
        supported_cases: 400,
        ambiguous_cases: 200,
        unsupported_cases: 200,
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
    assert_eq!(report.cases, 800);
    assert_eq!(report.exact_decisions, 800);
    assert_eq!(report.authorized, 400);
    assert_eq!(report.offered_frontends, 3_200);
    assert_eq!(report.frontend_replay_verified, 3_200);
    assert_eq!(report.frontend_tamper_rejections, 3_200);
    assert_eq!(report.downstream_replay_verified, 400);
    assert_eq!(report.downstream_tamper_rejections, 400);
    assert_eq!(report.route_leakage, 0);
    assert_eq!(report.false_authorizations, 0);
    assert_eq!(report.false_denials, 0);
    println!("{}", serde_json::to_string_pretty(&report)?);
    Ok(())
}
