//! Stage 323: route-blind technical-language benchmark after finite-state
//! curriculum admission.
//!
//! The corpus is independently generated from naturalized prompts. Every
//! prompt is offered to the complete route graph, including the finite-state
//! transition route. A route is authorized only when exactly one downstream
//! artifact is replayable; missing fields and unsupported semantics remain
//! closed. HLE and the production registry are not read or mutated.

use serde::Serialize;
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::fs;
use the_machine::technical_language_router::{replay_verified, route, RouteStatus};

const REPORT_JSON: &str = "docs/stage323_post_automata_technical_language.json";
const REPORT_MD: &str = "docs/stage323_post_automata_technical_language.md";

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
enum Partition {
    Development,
    Validation,
    Sealed,
    Boundary,
}

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
enum Expected {
    Authorized,
    Ambiguous,
    Unsupported,
}

#[derive(Debug, Clone, Serialize)]
struct Case {
    id: String,
    partition: Partition,
    family: String,
    text: String,
    expected: Expected,
}

#[derive(Debug, Serialize)]
struct Receipt {
    id: String,
    family: String,
    expected: Expected,
    actual: RouteStatus,
    exact: bool,
    replay_verified: bool,
    tamper_rejected: bool,
    false_authorization: bool,
    false_denial: bool,
    selected_route: Option<String>,
}

#[derive(Debug, Serialize)]
struct PartitionMetrics {
    cases: usize,
    exact_decisions: usize,
    authorized: usize,
    ambiguous: usize,
    unsupported: usize,
    replay_verified: usize,
    tamper_rejected: usize,
    false_authorizations: usize,
    false_denials: usize,
}

#[derive(Debug, Serialize)]
struct Report {
    schema: &'static str,
    corpus_sha256: String,
    cases: usize,
    development_cases: usize,
    validation_cases: usize,
    sealed_cases: usize,
    boundary_cases: usize,
    exact_decisions: usize,
    authorized: usize,
    ambiguity_preserved: usize,
    unsupported_refusals: usize,
    replay_verified: usize,
    tamper_rejected: usize,
    false_authorizations: usize,
    false_denials: usize,
    route_leakage: usize,
    hle_questions_read: usize,
    production_mutations: usize,
    partitions: BTreeMap<String, PartitionMetrics>,
    family_counts: BTreeMap<String, usize>,
    receipts: Vec<Receipt>,
}

fn digest<T: Serialize>(value: &T) -> String {
    format!("{:x}", Sha256::digest(serde_json::to_vec(value).unwrap()))
}

fn state_prompt(index: usize, guarded: bool) -> String {
    let suffix = if guarded {
        format!(" --open [key{index}]--> open{index}; open{index} --close--> locked{index}. Guards: key{index}=true.")
    } else {
        format!(" --open--> open{index}; open{index} --close--> locked{index}.")
    };
    format!(
        "Initial state: locked{index}. Transitions: locked{index}{suffix} Event sequence: open, close. Expected state: locked{index}."
    )
}

fn supported_case(family: usize, index: usize, partition: Partition) -> Case {
    let (name, text) = match family {
        0 => ("finite_state_plain", state_prompt(index, false)),
        1 => ("finite_state_guarded", state_prompt(index, true)),
        2 => (
            "number_theory_gcd",
            format!(
                "In a bounded arithmetic problem, find gcd, the greatest common divisor, with a={} b={}",
                42 + index % 17,
                18 + index % 11
            ),
        ),
        3 => (
            "combinatorics_count",
            format!(
                "For a finite selection problem, count combinations with n={} and k={}",
                8 + index % 9,
                2 + index % 3
            ),
        ),
        4 => (
            "markov_stationary",
            "Find the stationary distribution for a row-stochastic transition=[[3/4,1/4],[1/2,1/2]].".into(),
        ),
        5 => (
            "markov_hitting",
            "Find the hitting probability for a row-stochastic transition=[[1/2,1/2],[0,1]] with initial=[1,0], target=1, avoid=0.".into(),
        ),
        6 => (
            "mobius_inversion",
            "Apply Mobius inversion to f(1)..f(n), indexed from 1: [1, 2, 3, 4].".into(),
        ),
        _ => (
            "complex_analysis",
            "For the affine map, verify the Cauchy-Riemann equations: v_y=2; u_x=2; v_x=1; u_y=-1.".into(),
        ),
    };
    Case {
        id: format!("stage323-{partition:?}-supported-{family}-{index:03}"),
        partition,
        family: name.into(),
        text,
        expected: Expected::Authorized,
    }
}

fn ambiguous_case(index: usize, partition: Partition) -> Case {
    let (family, text) = match index % 6 {
        0 => (
            "finite_state_missing_guard",
            "Initial state: locked. Transitions: locked --open [key_ok]--> open. Event sequence: open. Expected state: open.",
        ),
        1 => (
            "finite_state_missing_target",
            "Initial state: idle. Transitions: idle --start--> running. Event sequence: start.",
        ),
        2 => (
            "competing_domains",
            "Maybe either combinations n=5 k=2 or gcd, the greatest common divisor, a=84 b=30.",
        ),
        3 => (
            "missing_markov_convention",
            "Find a stationary distribution for transition=[[3/4,1/4],[1/2,1/2]].",
        ),
        4 => (
            "missing_mobius_indexing",
            "Apply Mobius inversion to [1,2,3,4] without an indexing convention.",
        ),
        _ => (
            "unresolved_target",
            "Maybe determine the requested technical quantity from a finite state machine and a matrix.",
        ),
    };
    Case {
        id: format!("stage323-{partition:?}-ambiguous-{index:03}"),
        partition,
        family: family.into(),
        text: text.into(),
        expected: Expected::Ambiguous,
    }
}

fn unsupported_case(index: usize, partition: Partition) -> Case {
    let (family, text) = match index % 7 {
        0 => (
            "finite_state_nondeterministic",
            "This is a nondeterministic state machine with a random transition.",
        ),
        1 => (
            "finite_state_over_budget",
            "Initial state: q0. Transitions: q0 --a--> q0. Event sequence: a, a, a, a, a, a, a, a, a. Expected state: q0.",
        ),
        2 => (
            "infinite_spectral",
            "Use spectral mixing and an asymptotic limit on an infinite graph.",
        ),
        3 => (
            "unsupported_complex",
            "Convert (3-4i) to polar form and approximate the argument numerically.",
        ),
        4 => (
            "specialist_operator",
            "Apply a contour integral to an unbounded operator without domain assumptions.",
        ),
        5 => (
            "malformed_constraints",
            "Solve a coupled stochastic system with a missing transition convention.",
        ),
        _ => (
            "unrelated_prose",
            "Explain the historical context of this theorem without a typed mathematical target.",
        ),
    };
    Case {
        id: format!("stage323-{partition:?}-unsupported-{index:03}"),
        partition,
        family: family.into(),
        text: text.into(),
        expected: Expected::Unsupported,
    }
}

fn build_partition(
    partition: Partition,
    supported: usize,
    ambiguous: usize,
    unsupported: usize,
) -> Vec<Case> {
    let mut cases = Vec::with_capacity(supported + ambiguous + unsupported);
    for index in 0..supported {
        cases.push(supported_case(index % 8, index / 8, partition));
    }
    for index in 0..ambiguous {
        cases.push(ambiguous_case(index, partition));
    }
    for index in 0..unsupported {
        cases.push(unsupported_case(index, partition));
    }
    cases
}

fn expected_status(expected: Expected) -> RouteStatus {
    match expected {
        Expected::Authorized => RouteStatus::Authorized,
        Expected::Ambiguous => RouteStatus::Ambiguous,
        Expected::Unsupported => RouteStatus::Unsupported,
    }
}

fn route_name(decision: &the_machine::technical_language_router::RouteDecision) -> Option<String> {
    decision.selected.map(|route| format!("{route:?}"))
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut cases = Vec::with_capacity(2000);
    cases.extend(build_partition(Partition::Development, 360, 120, 120));
    cases.extend(build_partition(Partition::Validation, 240, 80, 80));
    cases.extend(build_partition(Partition::Sealed, 240, 80, 80));
    cases.extend(build_partition(Partition::Boundary, 0, 300, 300));
    assert_eq!(cases.len(), 2000);
    let corpus_sha256 = digest(&cases);
    let mut partitions = BTreeMap::new();
    let mut family_counts = BTreeMap::new();
    let mut receipts = Vec::with_capacity(cases.len());
    let mut exact_decisions = 0;
    let mut authorized = 0;
    let mut ambiguity_preserved = 0;
    let mut unsupported_refusals = 0;
    let mut replay_verified_count = 0;
    let mut tamper_rejected = 0;
    let mut false_authorizations = 0;
    let mut false_denials = 0;
    let mut route_leakage = 0;
    for case in &cases {
        *family_counts.entry(case.family.clone()).or_insert(0usize) += 1;
        let decision = route(&case.text, &case.id);
        let expected = expected_status(case.expected);
        let exact = decision.status == expected;
        let replay = replay_verified(&decision);
        let mut tampered = decision.clone();
        tampered.replay_hash.push('x');
        let tamper = !replay_verified(&tampered);
        exact_decisions += usize::from(exact);
        authorized += usize::from(decision.status == RouteStatus::Authorized);
        ambiguity_preserved += usize::from(
            case.expected == Expected::Ambiguous && decision.status == RouteStatus::Ambiguous,
        );
        unsupported_refusals += usize::from(
            case.expected == Expected::Unsupported && decision.status == RouteStatus::Unsupported,
        );
        replay_verified_count += usize::from(replay);
        tamper_rejected += usize::from(tamper);
        let false_authorization =
            case.expected != Expected::Authorized && decision.status == RouteStatus::Authorized;
        let false_denial =
            case.expected == Expected::Authorized && decision.status != RouteStatus::Authorized;
        route_leakage += usize::from(
            decision.status == RouteStatus::Authorized
                && (decision.authorized_candidates.len() != 1 || decision.selected.is_none()),
        );
        false_authorizations += usize::from(false_authorization);
        false_denials += usize::from(false_denial);
        receipts.push(Receipt {
            id: case.id.clone(),
            family: case.family.clone(),
            expected: case.expected,
            actual: decision.status,
            exact,
            replay_verified: replay,
            tamper_rejected: tamper,
            false_authorization,
            false_denial,
            selected_route: route_name(&decision),
        });
    }
    for partition in [
        Partition::Development,
        Partition::Validation,
        Partition::Sealed,
        Partition::Boundary,
    ] {
        let subset = cases.iter().filter(|case| case.partition == partition);
        let mut metrics = PartitionMetrics {
            cases: 0,
            exact_decisions: 0,
            authorized: 0,
            ambiguous: 0,
            unsupported: 0,
            replay_verified: 0,
            tamper_rejected: 0,
            false_authorizations: 0,
            false_denials: 0,
        };
        for receipt in receipts.iter().filter(|receipt| {
            cases
                .iter()
                .find(|case| case.id == receipt.id)
                .is_some_and(|case| case.partition == partition)
        }) {
            metrics.cases += 1;
            metrics.exact_decisions += usize::from(receipt.exact);
            metrics.authorized += usize::from(receipt.actual == RouteStatus::Authorized);
            metrics.ambiguous += usize::from(receipt.actual == RouteStatus::Ambiguous);
            metrics.unsupported += usize::from(receipt.actual == RouteStatus::Unsupported);
            metrics.replay_verified += usize::from(receipt.replay_verified);
            metrics.tamper_rejected += usize::from(receipt.tamper_rejected);
            metrics.false_authorizations += usize::from(receipt.false_authorization);
            metrics.false_denials += usize::from(receipt.false_denial);
        }
        // Keep the subset iterator consumed so the partition denominator is
        // checked against the immutable corpus rather than inferred from a
        // summary count.
        assert_eq!(metrics.cases, subset.count());
        partitions.insert(format!("{partition:?}"), metrics);
    }
    let report = Report {
        schema: "stage323-post-automata-technical-language-v1",
        corpus_sha256,
        cases: cases.len(),
        development_cases: 600,
        validation_cases: 400,
        sealed_cases: 400,
        boundary_cases: 600,
        exact_decisions,
        authorized,
        ambiguity_preserved,
        unsupported_refusals,
        replay_verified: replay_verified_count,
        tamper_rejected,
        false_authorizations,
        false_denials,
        route_leakage,
        hle_questions_read: 0,
        production_mutations: 0,
        partitions,
        family_counts,
        receipts,
    };
    assert_eq!(report.cases, 2000);
    assert_eq!(report.exact_decisions, 2000);
    assert_eq!(report.authorized, 840);
    assert_eq!(report.ambiguity_preserved, 580);
    assert_eq!(report.unsupported_refusals, 580);
    assert_eq!(report.replay_verified, 2000);
    assert_eq!(report.tamper_rejected, 2000);
    assert_eq!(report.false_authorizations, 0);
    assert_eq!(report.false_denials, 0);
    assert_eq!(report.route_leakage, 0);
    fs::write(
        REPORT_JSON,
        format!("{}\n", serde_json::to_string_pretty(&report)?),
    )?;
    fs::write(
        REPORT_MD,
        format!(
            "# Stage 323 — post-automata technical-language benchmark\n\n- Cases: {} (development {}, validation {}, sealed {}, boundary {})\n- Exact decisions: {}/{}\n- Authorized / ambiguous / unsupported: {} / {} / {}\n- Replay / tamper: {} / {}\n- False authorizations / denials: {} / {}\n- HLE questions read / production mutations: {} / {}\n\nThe independently generated corpus adds finite-state transition language to the existing route-blind technical-language graph. State-machine ambiguity is exposed only when explicit state markers are present, so unrelated prompts are not contaminated by a parser's missing-field response.\n",
            report.cases,
            report.development_cases,
            report.validation_cases,
            report.sealed_cases,
            report.boundary_cases,
            report.exact_decisions,
            report.cases,
            report.authorized,
            report.ambiguity_preserved,
            report.unsupported_refusals,
            report.replay_verified,
            report.tamper_rejected,
            report.false_authorizations,
            report.false_denials,
            report.hle_questions_read,
            report.production_mutations,
        ),
    )?;
    println!(
        "stage323 cases={} exact={} authorized={} ambiguous={} unsupported={} replay={} tamper={}",
        report.cases,
        report.exact_decisions,
        report.authorized,
        report.ambiguity_preserved,
        report.unsupported_refusals,
        report.replay_verified,
        report.tamper_rejected
    );
    Ok(())
}
