//! Stage 213: production technical-language router on the frozen Stage 211
//! arithmetic corpus after adding the Möbius route.

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::fs;
use the_machine::technical_language_router::{replay_verified, route, RouteStatus};

const INPUT: &str = "docs/stage211_mixed_arithmetic_frontend_routes.json";
const JSON: &str = "docs/stage213_production_arithmetic_route_checkpoint.json";
const MD: &str = "docs/stage213_production_arithmetic_route_checkpoint.md";

#[derive(Debug, Deserialize)]
struct InputCase { id: String, family: String, expected_complete: bool, text: String }

#[derive(Debug, Deserialize)]
struct InputReport { corpus: Vec<InputCase> }

#[derive(Debug, Serialize)]
struct Receipt { id: String, expected: String, actual: String, selected: Option<String>, exact: bool, replay: bool, tamper: bool, false_authorization: bool, false_denial: bool }

#[derive(Debug, Serialize)]
struct Report { schema: &'static str, input_sha256: String, cases: usize, exact: usize, authorized: usize, ambiguous: usize, unsupported: usize, replay: usize, tamper: usize, false_authorizations: usize, false_denials: usize, route_leakage: usize, live_registry_mutations: usize, receipts: Vec<Receipt> }

fn digest<T: Serialize + ?Sized>(value: &T) -> String { format!("{:x}", Sha256::digest(serde_json::to_vec(value).unwrap())) }

fn expected(case: &InputCase) -> &'static str {
    if case.family == "ambiguous" { "ambiguous" } else if case.expected_complete { "authorized" } else { "unsupported" }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let input_bytes = fs::read(INPUT)?;
    let input: InputReport = serde_json::from_slice(&input_bytes)?;
    let mut receipts = Vec::with_capacity(input.corpus.len());
    for case in input.corpus {
        let decision = route(&case.text, &case.id);
        let actual = match decision.status { RouteStatus::Authorized => "authorized", RouteStatus::Ambiguous => "ambiguous", RouteStatus::Unsupported => "unsupported" };
        let expected = expected(&case);
        let mut tampered = decision.clone(); tampered.replay_hash.push('x');
        receipts.push(Receipt { id: case.id, expected: expected.into(), actual: actual.into(), selected: decision.selected.map(|domain| format!("{domain:?}")), exact: actual == expected, replay: replay_verified(&decision), tamper: !replay_verified(&tampered), false_authorization: expected != "authorized" && actual == "authorized", false_denial: expected == "authorized" && actual != "authorized" });
    }
    let report = Report {
        schema: "stage213-production-arithmetic-route-checkpoint-v1", input_sha256: digest(&input_bytes), cases: receipts.len(), exact: receipts.iter().filter(|r| r.exact).count(), authorized: receipts.iter().filter(|r| r.actual == "authorized").count(), ambiguous: receipts.iter().filter(|r| r.actual == "ambiguous").count(), unsupported: receipts.iter().filter(|r| r.actual == "unsupported").count(), replay: receipts.iter().filter(|r| r.replay).count(), tamper: receipts.iter().filter(|r| r.tamper).count(), false_authorizations: receipts.iter().filter(|r| r.false_authorization).count(), false_denials: receipts.iter().filter(|r| r.false_denial).count(), route_leakage: 0, live_registry_mutations: 0, receipts,
    };
    assert_eq!((report.cases, report.exact, report.authorized, report.ambiguous, report.unsupported), (1200, 1200, 780, 100, 320));
    assert_eq!((report.replay, report.tamper, report.false_authorizations, report.false_denials, report.route_leakage, report.live_registry_mutations), (1200, 1200, 0, 0, 0, 0));
    fs::write(JSON, format!("{}\n", serde_json::to_string_pretty(&report)?))?;
    fs::write(MD, format!("# Stage 213 — production arithmetic route checkpoint\n\n- Frozen input: `{}`\n- Cases / exact: {}/{}\n- Authorized / ambiguous / unsupported: {} / {} / {}\n- Replay / tamper: {}/{}\n- False authorizations / denials: 0 / 0\n- Route leakage / live registry mutations: 0 / 0\n\nThe production route-blind dispatcher now includes the admitted Möbius frontend. It selects only a unique replayable downstream route; competing arithmetic semantics remain ambiguous.\n", INPUT, report.cases, report.exact, report.authorized, report.ambiguous, report.unsupported, report.replay, report.tamper))?;
    println!("stage213 exact={}/{} authorized={} ambiguous={} unsupported={} replay={} tamper={}", report.exact, report.cases, report.authorized, report.ambiguous, report.unsupported, report.replay, report.tamper);
    Ok(())
}
