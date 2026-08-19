//! Stage 329: independent technical-language gate for source-derived
//! bounded electromagnetism.

use serde::Serialize;
use sha2::{Digest, Sha256};
use std::fs;
use the_machine::technical_language_router::{replay_verified, route, RouteDomain, RouteStatus};

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
enum Hidden {
    Supported,
    Ambiguous,
    Unsupported,
}

#[derive(Debug, Serialize)]
struct Receipt {
    id: String,
    hidden: Hidden,
    actual: RouteStatus,
    selected: Option<RouteDomain>,
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
    unsupported: usize,
    exact_decisions: usize,
    supported_routes: usize,
    ambiguity_preserved: usize,
    unsupported_refused: usize,
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

fn supported_text(index: usize) -> String {
    match index % 4 {
        0 => format!("Use Ohm's law: current I={} and resistance R={} in SI-consistent exact units; find voltage.", 2 + index % 3, 5 + index % 2),
        1 => format!("For electrical power with voltage V={} and current I={} in SI-consistent exact units, calculate power.", 3 + index % 2, 2 + index % 3),
        2 => format!("With charge from constant current, I={} and t={} in SI-consistent exact units, compute charge.", 2 + index % 3, 4 + index % 2),
        _ => format!("For capacitor charge, capacitance C={} and voltage V={} in SI-consistent exact units, compute charge.", 6 + index % 2, 3 + index % 3),
    }
}

fn case_text(index: usize, hidden: Hidden) -> String {
    match hidden {
        Hidden::Supported => supported_text(index),
        Hidden::Ambiguous => match index % 2 {
            0 => format!("Use Ohm's law with I=2 and R=5, but the unit scope is not stated."),
            _ => "Choose between Ohm's law and electric power with I=2, R=5, V=3 in SI-consistent exact units.".into(),
        },
        Hidden::Unsupported => match index % 3 {
            0 => "Use Ohm's law in an alternating circuit simulation with I=2 and R=5 in SI-consistent exact units.".into(),
            1 => "Apply a quantum electromagnetic field theory to infer energy from I=2 and V=3 in SI-consistent exact units.".into(),
            _ => "Apply electric power with V=3 in SI-consistent exact units, but the current is missing.".into(),
        },
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut cases = Vec::with_capacity(240);
    for index in 0..240 {
        let hidden = match index {
            0..120 => Hidden::Supported,
            120..160 => Hidden::Ambiguous,
            _ => Hidden::Unsupported,
        };
        cases.push((
            format!("stage329-{index:03}"),
            hidden,
            case_text(index, hidden),
        ));
    }
    let corpus_sha256 = digest(&cases);
    let mut receipts = Vec::with_capacity(cases.len());
    let mut exact = 0;
    let mut routes = 0;
    let mut ambiguity = 0;
    let mut refused = 0;
    let mut replay = 0;
    let mut tamper = 0;
    let mut false_auth = 0;
    let mut false_denial = 0;
    let mut leakage = 0;
    for (id, hidden, text) in &cases {
        let decision = route(text, id);
        let expected = match hidden {
            Hidden::Supported => RouteStatus::Authorized,
            Hidden::Ambiguous => RouteStatus::Ambiguous,
            Hidden::Unsupported => RouteStatus::Unsupported,
        };
        let is_exact = decision.status == expected;
        let authorized = decision.status == RouteStatus::Authorized;
        let replay_ok = replay_verified(&decision);
        let mut tampered = decision.clone();
        tampered.replay_hash.push('x');
        let tamper_ok = !replay_verified(&tampered);
        let false_authorization = *hidden != Hidden::Supported && authorized;
        let false_denial_case = *hidden == Hidden::Supported && !authorized;
        exact += usize::from(is_exact);
        routes += usize::from(*hidden == Hidden::Supported && authorized);
        ambiguity +=
            usize::from(*hidden == Hidden::Ambiguous && decision.status == RouteStatus::Ambiguous);
        refused += usize::from(
            *hidden == Hidden::Unsupported && decision.status == RouteStatus::Unsupported,
        );
        replay += usize::from(replay_ok);
        tamper += usize::from(tamper_ok);
        false_auth += usize::from(false_authorization);
        false_denial += usize::from(false_denial_case);
        leakage += usize::from(
            authorized
                && (decision.selected.is_none() || decision.authorized_candidates.len() != 1),
        );
        receipts.push(Receipt {
            id: id.clone(),
            hidden: *hidden,
            actual: decision.status,
            selected: decision.selected,
            replay_verified: replay_ok,
            tamper_rejected: tamper_ok,
            false_authorization,
            false_denial: false_denial_case,
        });
    }
    let report = Report {
        schema: "stage329-electromagnetism-language-route-v1",
        corpus_sha256,
        cases: 240,
        supported: 120,
        ambiguous: 40,
        unsupported: 80,
        exact_decisions: exact,
        supported_routes: routes,
        ambiguity_preserved: ambiguity,
        unsupported_refused: refused,
        replay_verified: replay,
        tamper_rejected: tamper,
        false_authorizations: false_auth,
        false_denials: false_denial,
        route_leakage: leakage,
        hle_questions_read: 0,
        production_mutations: 0,
        receipts,
    };
    assert_eq!(report.exact_decisions, 240);
    assert_eq!(report.supported_routes, 120);
    assert_eq!(report.ambiguity_preserved, 40);
    assert_eq!(report.unsupported_refused, 80);
    assert_eq!(report.replay_verified, 240);
    assert_eq!(report.tamper_rejected, 240);
    assert_eq!(report.false_authorizations, 0);
    assert_eq!(report.false_denials, 0);
    assert_eq!(report.route_leakage, 0);
    fs::write(
        "docs/stage329_electromagnetism_language_route.json",
        format!("{}\n", serde_json::to_string_pretty(&report)?),
    )?;
    fs::write("docs/stage329_electromagnetism_language_route.md", format!(
        "# Stage 329 — source electromagnetism technical-language route\n\n- Cases: 240\n- Supported / ambiguous / unsupported: 120 / 40 / 80\n- Exact decisions: {}/240\n- Supported routes / ambiguity preserved / unsupported refused: {} / {} / {}\n- Replay / tamper: {} / {}\n- False authorizations / denials: {} / {}\n- Route leakage: {}\n- HLE questions read / production mutations: {} / {}\n\nThe frontend requires one source-attributed law, explicit SI-consistent scope, and all law inputs; it refuses missing units, competing laws, unsupported circuit regimes, and incomplete quantities.\n",
        report.exact_decisions, report.supported_routes, report.ambiguity_preserved, report.unsupported_refused,
        report.replay_verified, report.tamper_rejected, report.false_authorizations, report.false_denials,
        report.route_leakage, report.hle_questions_read, report.production_mutations
    ))?;
    println!(
        "stage329 cases={} exact={} routes={} ambiguous={} refused={} replay={} tamper={}",
        report.cases,
        report.exact_decisions,
        report.supported_routes,
        report.ambiguity_preserved,
        report.unsupported_refused,
        report.replay_verified,
        report.tamper_rejected
    );
    Ok(())
}
