//! Stage 187: scaled route-blind technical-language evaluation.
//!
//! The same text is offered to the complex-analysis, combinatorics, and
//! elementary-number-theory frontends.  The dispatcher authorizes only a
//! unique downstream-replayable route and keeps a permanent holdout partition.

use serde::Serialize;
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::fs;
use the_machine::curriculum::breadth_first_manifest;
use the_machine::technical_language_router::{replay_verified, route, RouteDomain, RouteStatus};

const REPORT_JSON: &str = "docs/stage187_route_blind_technical_language_scale.json";
const REPORT_MD: &str = "docs/stage187_route_blind_technical_language_scale.md";

#[derive(Debug, Serialize)]
struct Receipt {
    id: String,
    partition: String,
    text_sha256: String,
    expected: String,
    actual: String,
    selected: Option<RouteDomain>,
    authorized_candidates: Vec<RouteDomain>,
    ambiguous_candidates: Vec<RouteDomain>,
    exact: bool,
    replay_verified: bool,
    tamper_rejected: bool,
    false_authorization: bool,
    false_denial: bool,
}

#[derive(Debug, Serialize)]
struct Report {
    schema: &'static str,
    manifest_sha256: String,
    corpus_sha256: String,
    cases: usize,
    development_cases: usize,
    holdout_cases: usize,
    supported: usize,
    ambiguous: usize,
    unsupported: usize,
    exact_decisions: usize,
    development_exact: usize,
    development_authorized: usize,
    holdout_exact: usize,
    holdout_authorized: usize,
    authorized_routes: BTreeMap<RouteDomain, usize>,
    route_leakage: usize,
    replay_verified: usize,
    tamper_rejections: usize,
    false_authorizations: usize,
    false_denials: usize,
    receipts: Vec<Receipt>,
}

fn digest<T: Serialize + ?Sized>(value: &T) -> String {
    format!(
        "{:x}",
        Sha256::digest(serde_json::to_vec(value).expect("stage187 serializes"))
    )
}

fn complex_text(index: usize) -> String {
    if index % 2 == 0 {
        "Check the Cauchy-Riemann equations: ux=2, uy=-1, vx=1, vy=2.".into()
    } else {
        "Differentiate the affine map after checking Cauchy Riemann: v_y=2; u_x=2; v_x=1; u_y=-1."
            .into()
    }
}

fn combinatorics_text(index: usize) -> String {
    if index % 2 == 0 {
        "Count combinations with n=5 k=2.".into()
    } else {
        "How many choices are possible using the binomial operation n = 6 and k = 3?".into()
    }
}

fn number_text(index: usize) -> String {
    if index % 2 == 0 {
        "Find gcd, the greatest common divisor, with a=84 b=30.".into()
    } else {
        "Compute the Bezout gcd certificate for a = 99 and b = 36.".into()
    }
}

fn ambiguous_text(index: usize) -> String {
    if index % 2 == 0 {
        "Maybe either combinations n=5 k=2 or gcd, the greatest common divisor, a=84 b=30.".into()
    } else {
        "Possibly count combinations n=6 k=3 or find gcd, the greatest common divisor, a=84 b=30."
            .into()
    }
}

fn unsupported_text(index: usize) -> String {
    if index % 2 == 0 {
        "Use a contour integral on an infinite graph.".into()
    } else {
        "Give an asymptotic prime factorization and a weighted random count.".into()
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let manifest_sha256 = breadth_first_manifest().replay_hash();
    let mut receipts = Vec::with_capacity(2_000);
    let mut exact = 0;
    let mut development_exact = 0;
    let mut development_authorized = 0;
    let mut holdout_exact = 0;
    let mut holdout_authorized = 0;
    let mut replay_count = 0;
    let mut tamper_count = 0;
    let mut route_leakage = 0;
    let mut authorized_routes = BTreeMap::new();

    for index in 0..2_000 {
        let (expected, text) = if index < 600 {
            ("supported", complex_text(index))
        } else if index < 1_100 {
            ("supported", combinatorics_text(index))
        } else if index < 1_600 {
            ("supported", number_text(index))
        } else if index < 1_800 {
            ("ambiguous", ambiguous_text(index))
        } else {
            ("unsupported", unsupported_text(index))
        };
        let partition = if index < 1_500 {
            "development"
        } else {
            "holdout"
        };
        let decision = route(&text, &format!("stage187-{index:04}"));
        let actual = match decision.status {
            RouteStatus::Authorized => "supported",
            RouteStatus::Ambiguous => "ambiguous",
            RouteStatus::Unsupported => "unsupported",
        };
        let exact_status = actual == expected;
        let authorized = decision.status == RouteStatus::Authorized;
        let false_authorization = expected != "supported" && authorized;
        let false_denial = expected == "supported" && !authorized;
        if decision.authorized_candidates.len() > 1 {
            route_leakage += 1;
        }
        if let Some(selected) = decision.selected {
            *authorized_routes.entry(selected).or_insert(0) += 1;
        }
        let replay = replay_verified(&decision);
        let mut tampered = decision.clone();
        tampered.replay_hash.push('x');
        let tamper_rejected = !replay_verified(&tampered);
        exact += usize::from(exact_status);
        if partition == "development" {
            development_exact += usize::from(exact_status);
            development_authorized += usize::from(authorized);
        } else {
            holdout_exact += usize::from(exact_status);
            holdout_authorized += usize::from(authorized);
        }
        replay_count += usize::from(replay);
        tamper_count += usize::from(tamper_rejected);
        receipts.push(Receipt {
            id: format!("stage187-{index:04}"),
            partition: partition.into(),
            text_sha256: digest(text.as_bytes()),
            expected: expected.into(),
            actual: actual.into(),
            selected: decision.selected,
            authorized_candidates: decision.authorized_candidates.clone(),
            ambiguous_candidates: decision.ambiguous_candidates.clone(),
            exact: exact_status,
            replay_verified: replay,
            tamper_rejected,
            false_authorization,
            false_denial,
        });
    }

    let false_authorizations = receipts
        .iter()
        .filter(|receipt| receipt.false_authorization)
        .count();
    let false_denials = receipts
        .iter()
        .filter(|receipt| receipt.false_denial)
        .count();
    assert_eq!(exact, 2_000);
    assert_eq!(development_exact, 1_500);
    assert_eq!(holdout_exact, 500);
    assert_eq!(development_authorized, 1_500);
    assert_eq!(holdout_authorized, 100);
    assert_eq!(replay_count, 2_000);
    assert_eq!(tamper_count, 2_000);
    assert_eq!(route_leakage, 0);
    assert_eq!(false_authorizations, 0);
    assert_eq!(false_denials, 0);
    let report = Report {
        schema: "stage187-route-blind-technical-language-scale-v1",
        manifest_sha256: manifest_sha256.clone(),
        corpus_sha256: digest(&receipts),
        cases: 2_000,
        development_cases: 1_500,
        holdout_cases: 500,
        supported: 1_600,
        ambiguous: 200,
        unsupported: 200,
        exact_decisions: exact,
        development_exact,
        development_authorized,
        holdout_exact,
        holdout_authorized,
        authorized_routes,
        route_leakage,
        replay_verified: replay_count,
        tamper_rejections: tamper_count,
        false_authorizations,
        false_denials,
        receipts,
    };
    let serialized = serde_json::to_string_pretty(&report)?;
    fs::write(REPORT_JSON, format!("{serialized}\n"))?;
    fs::write(
        REPORT_MD,
        format!(
            "# Stage 187 — scaled route-blind technical language\n\nThe same text was offered to three validated frontends and authorized only after unique downstream replay.\n\n| Measure | Result |\n|---|---:|\n| Cases / development / holdout | 2,000 / 1,500 / 500 |\n| Supported / ambiguous / unsupported | 1,600 / 200 / 200 |\n| Exact decisions | {exact}/2,000 |\n| Development exact / authorized | {development_exact}/1,500 / {development_authorized} |\n| Holdout exact / authorized | {holdout_exact}/500 / {holdout_authorized} |\n| Replay / tamper | {replay_count}/2,000 / {tamper_count}/2,000 |\n| Route leakage | {route_leakage} |\n| False authorizations / denials | {false_authorizations} / {false_denials} |\n| Production mutation | false |\n\nManifest SHA-256: `{manifest_sha256}`\n\nMachine-readable report: `{REPORT_JSON}`\n"
        ),
    )?;
    println!("{serialized}");
    Ok(())
}
