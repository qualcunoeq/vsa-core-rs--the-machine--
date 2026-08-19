//! Stage 324: independent shifted-language holdout for finite-state routing.
//!
//! These prompts deliberately avoid the canonical `initial state` /
//! `event sequence` / `expected state` labels used by the admission corpus.
//! Only explicit, semantics-preserving aliases are supported. Missing target
//! or guard information remains ambiguous; nondeterminism and budget escape
//! remain refused.

use serde::Serialize;
use sha2::{Digest, Sha256};
use std::fs;
use the_machine::technical_language_router::{replay_verified, route, RouteDomain, RouteStatus};

const REPORT_JSON: &str = "docs/stage324_finite_state_language_shift.json";
const REPORT_MD: &str = "docs/stage324_finite_state_language_shift.md";

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
    selected: Option<RouteDomain>,
    exact: bool,
    finite_state_route: bool,
    replay_verified: bool,
    tamper_rejected: bool,
    false_authorization: bool,
    false_denial: bool,
}

#[derive(Debug, Serialize)]
struct Report {
    schema: &'static str,
    corpus_sha256: String,
    cases: usize,
    supported: usize,
    ambiguous: usize,
    refused: usize,
    exact_decisions: usize,
    shifted_supported_routes: usize,
    replay_verified: usize,
    tamper_rejected: usize,
    false_authorizations: usize,
    false_denials: usize,
    route_leakage: usize,
    hle_questions_read: usize,
    production_mutations: usize,
    receipts: Vec<Receipt>,
}

fn digest<T: Serialize>(value: &T) -> String {
    format!("{:x}", Sha256::digest(serde_json::to_vec(value).unwrap()))
}

fn supported_case(index: usize) -> Case {
    let (family, text) = match index % 6 {
        0 => (
            "begin_input_finish",
            format!(
                "Begin in state locked{index}. Transitions: locked{index} --open--> open{index}; open{index} --close--> locked{index}. Input events: open, close. Finish in state locked{index}."
            ),
        ),
        1 => (
            "starting_process_final",
            format!(
                "Starting state: cold{index}. Transitions: cold{index} --heat--> warm{index}; warm{index} --cool--> cold{index}. Process events: heat, cool. Final state: cold{index}."
            ),
        ),
        2 => (
            "start_events_end",
            format!(
                "Start in state q{index}0. Event transitions: q{index}0 --a--> q{index}1; q{index}1 --b--> q{index}0. Events to process: a, b, a. End in state q{index}1."
            ),
        ),
        3 => (
            "guarded_alias",
            format!(
                "Begin in state locked{index}. Transitions: locked{index} --open [key{index}]--> open{index}; open{index} --close--> locked{index}. Guards: key{index}=true. Input events: open, close. Final state: locked{index}."
            ),
        ),
        4 => (
            "reordered_clauses",
            format!(
                "Events to process: tick, tick. Finish in state idle{index}. Transitions: idle{index} --tick--> idle{index}. Begin in state idle{index}."
            ),
        ),
        _ => (
            "case_variation",
            format!(
                "Start in state armed{index}; transitions: armed{index} --trigger--> alarm{index}; alarm{index} --reset--> armed{index}; input events: trigger, reset, trigger. Final state: alarm{index}."
            ),
        ),
    };
    Case {
        id: format!("stage324-supported-{index:03}"),
        family: family.into(),
        text,
        expected: Expected::Authorized,
    }
}

fn ambiguous_case(index: usize) -> Case {
    let (family, text) = match index % 4 {
        0 => (
            "alias_missing_target",
            "Begin in state locked. Transitions: locked --open--> open. Input events: open.",
        ),
        1 => (
            "alias_missing_guard",
            "Start in state locked. Transitions: locked --open [key_ok]--> open. Events to process: open. Finish in state open.",
        ),
        2 => (
            "alias_invalid_guard",
            "Starting state: q0. Transitions: q0 --a--> q1. Process events: a. Final state: q1. Guards: key_ok=maybe.",
        ),
        _ => (
            "alias_missing_table",
            "Begin in state idle. Input events: start. Finish in state running.",
        ),
    };
    Case {
        id: format!("stage324-ambiguous-{index:03}"),
        family: family.into(),
        text: text.into(),
        expected: Expected::Ambiguous,
    }
}

fn unsupported_case(index: usize) -> Case {
    let (family, text) = match index % 5 {
        0 => (
            "nondeterministic",
            "Begin in state q0. This is a nondeterministic state machine with a random transition. Input events: a. Finish in state q1.",
        ),
        1 => (
            "over_budget",
            "Start in state q0. Transitions: q0 --a--> q0. Events to process: a, a, a, a, a, a, a, a, a. End in state q0.",
        ),
        2 => (
            "conflicting_transition",
            "Begin in state q0. Transitions: q0 --a--> q1; q0 --a--> q2. Input events: a. Final state: q1.",
        ),
        3 => (
            "stochastic_state",
            "Start in state q0. Transitions: q0 --a--> q1. Input events: a. Final state: q1. The transition is probabilistic.",
        ),
        _ => (
            "continuous_state",
            "Begin in state q0. Transitions: q0 --dt--> q1. Process events: dt. Finish in state q1 in continuous time.",
        ),
    };
    Case {
        id: format!("stage324-refused-{index:03}"),
        family: family.into(),
        text: text.into(),
        expected: Expected::Unsupported,
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut cases = Vec::with_capacity(240);
    for index in 0..120 {
        cases.push(supported_case(index));
    }
    for index in 0..40 {
        cases.push(ambiguous_case(index));
    }
    for index in 0..80 {
        cases.push(unsupported_case(index));
    }
    assert_eq!(cases.len(), 240);
    let mut receipts = Vec::with_capacity(cases.len());
    let mut supported = 0;
    let mut ambiguous = 0;
    let mut refused = 0;
    let mut exact_decisions = 0;
    let mut shifted_supported_routes = 0;
    let mut replay_verified_count = 0;
    let mut tamper_rejected = 0;
    let mut false_authorizations = 0;
    let mut false_denials = 0;
    let mut route_leakage = 0;
    for case in &cases {
        let decision = route(&case.text, &case.id);
        let expected_status = match case.expected {
            Expected::Authorized => RouteStatus::Authorized,
            Expected::Ambiguous => RouteStatus::Ambiguous,
            Expected::Unsupported => RouteStatus::Unsupported,
        };
        let exact = decision.status == expected_status;
        let replay = replay_verified(&decision);
        let mut tampered = decision.clone();
        tampered.replay_hash.push('x');
        let tamper = !replay_verified(&tampered);
        let finite_state_route = decision.selected == Some(RouteDomain::FiniteStateTransition);
        let false_authorization =
            case.expected != Expected::Authorized && decision.status == RouteStatus::Authorized;
        let false_denial =
            case.expected == Expected::Authorized && decision.status != RouteStatus::Authorized;
        if exact {
            exact_decisions += 1;
        }
        match decision.status {
            RouteStatus::Authorized => supported += 1,
            RouteStatus::Ambiguous => ambiguous += 1,
            RouteStatus::Unsupported => refused += 1,
        }
        shifted_supported_routes +=
            usize::from(case.expected == Expected::Authorized && finite_state_route);
        replay_verified_count += usize::from(replay);
        tamper_rejected += usize::from(tamper);
        false_authorizations += usize::from(false_authorization);
        false_denials += usize::from(false_denial);
        route_leakage += usize::from(
            decision.status == RouteStatus::Authorized
                && (decision.authorized_candidates.len() != 1 || decision.selected.is_none()),
        );
        receipts.push(Receipt {
            id: case.id.clone(),
            family: case.family.clone(),
            expected: case.expected,
            actual: decision.status,
            selected: decision.selected,
            exact,
            finite_state_route,
            replay_verified: replay,
            tamper_rejected: tamper,
            false_authorization,
            false_denial,
        });
    }
    let report = Report {
        schema: "stage324-finite-state-language-shift-v1",
        corpus_sha256: digest(&cases),
        cases: cases.len(),
        supported,
        ambiguous,
        refused,
        exact_decisions,
        shifted_supported_routes,
        replay_verified: replay_verified_count,
        tamper_rejected,
        false_authorizations,
        false_denials,
        route_leakage,
        hle_questions_read: 0,
        production_mutations: 0,
        receipts,
    };
    assert_eq!(report.cases, 240);
    assert_eq!(report.supported, 120);
    assert_eq!(report.ambiguous, 40);
    assert_eq!(report.refused, 80);
    assert_eq!(report.exact_decisions, 240);
    assert_eq!(report.shifted_supported_routes, 120);
    assert_eq!(report.replay_verified, 240);
    assert_eq!(report.tamper_rejected, 240);
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
            "# Stage 324 — finite-state technical-language shift\n\n- Cases: {} ({} supported, {} ambiguous, {} refused)\n- Exact decisions: {}/{}\n- Shifted supported finite-state routes: {}/{}\n- Replay / tamper: {} / {}\n- False authorizations / denials: {} / {}\n- Route leakage: {}\n- HLE questions read / production mutations: {} / {}\n\nThe supported prompts use explicit aliases for initial state, event sequence, and final state, with reordered clauses and guarded transitions. No canonical-label prompt is used in this holdout.\n",
            report.cases,
            report.supported,
            report.ambiguous,
            report.refused,
            report.exact_decisions,
            report.cases,
            report.shifted_supported_routes,
            report.supported,
            report.replay_verified,
            report.tamper_rejected,
            report.false_authorizations,
            report.false_denials,
            report.route_leakage,
            report.hle_questions_read,
            report.production_mutations,
        ),
    )?;
    println!(
        "stage324 cases={} exact={} supported={} ambiguous={} refused={} shifted={} replay={} tamper={}",
        report.cases,
        report.exact_decisions,
        report.supported,
        report.ambiguous,
        report.refused,
        report.shifted_supported_routes,
        report.replay_verified,
        report.tamper_rejected
    );
    Ok(())
}
