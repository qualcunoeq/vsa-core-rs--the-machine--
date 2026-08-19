//! Stage 325: source-derived metric/topology technical-language routes.
//!
//! This independent corpus exercises two newly exposed source-derived
//! frontends through the shared route-blind dispatcher. Explicit finite
//! carriers are supported; missing operations, competing interpretations,
//! invalid axioms, and advanced topology remain closed.

use serde::Serialize;
use sha2::{Digest, Sha256};
use std::fs;
use the_machine::technical_language_router::{replay_verified, route, RouteDomain, RouteStatus};

const REPORT_JSON: &str = "docs/stage325_metric_topology_language_routes.json";
const REPORT_MD: &str = "docs/stage325_metric_topology_language_routes.md";

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
    metric_routes: usize,
    topology_routes: usize,
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

fn metric_supported(index: usize) -> String {
    match index % 3 {
        0 => format!(
            "For a finite metric on points: p0,p1,p2; distances: p0-p0=0,p0-p1=1,p0-p2=2,p1-p1=0,p1-p2=1,p2-p2=0; determine the distance from p0 to p2."
        ),
        1 => "Validate the finite metric on points: p0,p1,p2; distances: p0-p0=0,p0-p1=1,p0-p2=2,p1-p1=0,p1-p2=1,p2-p2=0.".into(),
        _ => "For a finite metric on points: p0,p1,p2; distances: p0-p0=0,p0-p1=1,p0-p2=2,p1-p1=0,p1-p2=1,p2-p2=0; determine the diameter.".into(),
    }
}

fn topology_supported(index: usize) -> String {
    match index % 3 {
        0 => "Validate topology: points: {a,b,c}; open sets: {}; open sets: {a}; open sets: {a,b,c}.".into(),
        1 => "Is open: points: {a,b,c}; target: {a}; open sets: {}; open sets: {a}; open sets: {a,b,c}.".into(),
        _ => "Find the closure. Points: {a,b,c}; target: {a}; open sets: {}; open sets: {a}; open sets: {a,b,c}.".into(),
    }
}

fn ambiguous(index: usize) -> (String, String) {
    match index % 4 {
        0 => ("metric_missing_operation".into(), "For a finite metric on points: p0,p1; distances: p0-p0=0,p0-p1=1,p1-p1=0.".into()),
        1 => ("metric_competing_operation".into(), "Either validate the metric or determine the distance from p0 to p1; points: p0,p1; distances: p0-p0=0,p0-p1=1,p1-p1=0.".into()),
        2 => ("topology_duplicate_carrier".into(), "Find the interior; points: {a,b}; points: {a,b,c}; target: {a}; open sets: {}; open sets: {a}; open sets: {a,b,c}.".into()),
        _ => ("topology_missing_target".into(), "Find the interior; points: {a,b,c}; open sets: {}; open sets: {a}; open sets: {a,b,c}.".into()),
    }
}

fn unsupported(index: usize) -> (String, String) {
    match index % 4 {
        0 => ("infinite_metric".into(), "Prove completeness of an infinite geodesic metric space.".into()),
        1 => ("invalid_metric".into(), "Validate the finite metric on points: p0,p1,p2; distances: p0-p0=0,p0-p1=1,p0-p2=1,p1-p1=0,p1-p2=5,p2-p2=0.".into()),
        2 => ("topology_homology".into(), "Find the homology of the topology; points: {a,b}; open sets: {}; open sets: {a,b}.".into()),
        _ => ("topology_over_bound".into(), "Validate topology: points: {a,b,c,d,e,f,g,h,i}; open sets: {}; open sets: {a,b,c,d,e,f,g,h,i}.".into()),
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut cases = Vec::with_capacity(240);
    for index in 0..60 {
        cases.push(Case {
            id: format!("stage325-metric-supported-{index:03}"),
            family: "metric_supported".into(),
            text: metric_supported(index),
            expected: Expected::Authorized,
        });
        cases.push(Case {
            id: format!("stage325-topology-supported-{index:03}"),
            family: "topology_supported".into(),
            text: topology_supported(index),
            expected: Expected::Authorized,
        });
    }
    for index in 0..40 {
        let (family, text) = ambiguous(index);
        cases.push(Case {
            id: format!("stage325-ambiguous-{index:03}"),
            family,
            text,
            expected: Expected::Ambiguous,
        });
    }
    for index in 0..80 {
        let (family, text) = unsupported(index);
        cases.push(Case {
            id: format!("stage325-unsupported-{index:03}"),
            family,
            text,
            expected: Expected::Unsupported,
        });
    }
    assert_eq!(cases.len(), 240);
    let mut receipts = Vec::with_capacity(cases.len());
    let mut supported = 0;
    let mut ambiguous_count = 0;
    let mut refused = 0;
    let mut exact_decisions = 0;
    let mut metric_routes = 0;
    let mut topology_routes = 0;
    let mut replay_count = 0;
    let mut tamper_count = 0;
    let mut false_auth = 0;
    let mut false_denial = 0;
    let mut route_leakage = 0;
    for case in &cases {
        let decision = route(&case.text, &case.id);
        let expected = match case.expected {
            Expected::Authorized => RouteStatus::Authorized,
            Expected::Ambiguous => RouteStatus::Ambiguous,
            Expected::Unsupported => RouteStatus::Unsupported,
        };
        let exact = decision.status == expected;
        let replay = replay_verified(&decision);
        let mut tampered = decision.clone();
        tampered.replay_hash.push('x');
        let tamper = !replay_verified(&tampered);
        let false_authorization =
            case.expected != Expected::Authorized && decision.status == RouteStatus::Authorized;
        let false_denial_case =
            case.expected == Expected::Authorized && decision.status != RouteStatus::Authorized;
        exact_decisions += usize::from(exact);
        match decision.status {
            RouteStatus::Authorized => supported += 1,
            RouteStatus::Ambiguous => ambiguous_count += 1,
            RouteStatus::Unsupported => refused += 1,
        }
        metric_routes += usize::from(decision.selected == Some(RouteDomain::FiniteMetric));
        topology_routes += usize::from(decision.selected == Some(RouteDomain::FiniteTopology));
        replay_count += usize::from(replay);
        tamper_count += usize::from(tamper);
        false_auth += usize::from(false_authorization);
        false_denial += usize::from(false_denial_case);
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
            replay_verified: replay,
            tamper_rejected: tamper,
            false_authorization,
            false_denial: false_denial_case,
        });
    }
    let report = Report {
        schema: "stage325-metric-topology-language-routes-v1",
        corpus_sha256: digest(&cases),
        cases: cases.len(),
        supported,
        ambiguous: ambiguous_count,
        refused,
        exact_decisions,
        metric_routes,
        topology_routes,
        replay_verified: replay_count,
        tamper_rejected: tamper_count,
        false_authorizations: false_auth,
        false_denials: false_denial,
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
    assert_eq!(report.metric_routes, 60);
    assert_eq!(report.topology_routes, 60);
    assert_eq!(report.replay_verified, 240);
    assert_eq!(report.tamper_rejected, 240);
    assert_eq!(report.false_authorizations, 0);
    assert_eq!(report.false_denials, 0);
    assert_eq!(report.route_leakage, 0);
    fs::write(
        REPORT_JSON,
        format!("{}\n", serde_json::to_string_pretty(&report)?),
    )?;
    fs::write(REPORT_MD, format!("# Stage 325 — metric/topology technical-language routes\n\n- Cases: {} ({} supported, {} ambiguous, {} refused)\n- Exact decisions: {}/{}\n- Metric/topology routes: {} / {}\n- Replay / tamper: {} / {}\n- False authorizations / denials: {} / {}\n- Route leakage: {}\n- HLE questions read / production mutations: {} / {}\n", report.cases, report.supported, report.ambiguous, report.refused, report.exact_decisions, report.cases, report.metric_routes, report.topology_routes, report.replay_verified, report.tamper_rejected, report.false_authorizations, report.false_denials, report.route_leakage, report.hle_questions_read, report.production_mutations))?;
    println!("stage325 cases={} exact={} supported={} ambiguous={} refused={} metric={} topology={} replay={} tamper={}", report.cases, report.exact_decisions, report.supported, report.ambiguous, report.refused, report.metric_routes, report.topology_routes, report.replay_verified, report.tamper_rejected);
    Ok(())
}
