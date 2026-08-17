//! Stage 116: post-retrieval algebraic composition checkpoint.

use serde::Serialize;
use sha2::{Digest, Sha256};

const ALGEBRA_NUMBER: &str =
    include_str!("../../docs/phase70_algebra_number_theory_composition.json");
const COUNT_NUMBER: &str =
    include_str!("../../docs/phase71_combinatorics_number_theory_composition.json");
const RETRIEVAL: &str = include_str!("../../docs/stage115_self_directed_memory_retrieval.json");
const HLE: &str = include_str!("../../docs/stage106_hle_curriculum_checkpoint.json");

fn digest(value: &str) -> String {
    format!("{:x}", Sha256::digest(value.as_bytes()))
}

fn field(text: &str, name: &str) -> usize {
    let needle = format!("\"{name}\":");
    text.split(&needle)
        .nth(1)
        .and_then(|tail| {
            tail.trim_start()
                .split(|c: char| !c.is_ascii_digit())
                .next()
        })
        .and_then(|value| value.parse().ok())
        .unwrap_or(0)
}

#[derive(Debug, Serialize)]
struct Report {
    schema: &'static str,
    parent_report_sha256: Vec<String>,
    composition_cases: usize,
    supported_routes: usize,
    ambiguous_or_refused: usize,
    route_replays: usize,
    tamper_rejections: usize,
    false_authorizations: usize,
    false_denials: usize,
    self_directed_complete_plans: usize,
    retrieval_provenance_mismatches: usize,
    frozen_hle_cases: usize,
    frozen_hle_correct_authorized: usize,
    frozen_hle_false_authorizations: usize,
}

fn main() {
    let parents = [ALGEBRA_NUMBER, COUNT_NUMBER, RETRIEVAL, HLE];
    let composition_cases = field(ALGEBRA_NUMBER, "cases") + field(COUNT_NUMBER, "cases");
    let supported_routes =
        field(ALGEBRA_NUMBER, "supported_routes") + field(COUNT_NUMBER, "supported_routes");
    let route_replays =
        field(ALGEBRA_NUMBER, "route_replay_verified") + field(COUNT_NUMBER, "replay_verified");
    let tamper_rejections =
        field(ALGEBRA_NUMBER, "tamper_rejections") + field(COUNT_NUMBER, "tamper_rejections");
    let false_authorizations =
        field(ALGEBRA_NUMBER, "false_authorizations") + field(COUNT_NUMBER, "false_authorizations");
    let false_denials =
        field(ALGEBRA_NUMBER, "false_denials") + field(COUNT_NUMBER, "false_denials");
    assert_eq!(composition_cases, 480);
    assert_eq!(supported_routes, 240);
    assert_eq!(route_replays, composition_cases);
    assert_eq!(tamper_rejections, composition_cases);
    assert_eq!(false_authorizations, 0);
    assert_eq!(false_denials, 0);
    assert_eq!(field(RETRIEVAL, "complete"), 600);
    assert_eq!(field(RETRIEVAL, "provenance_mismatches"), 0);
    assert_eq!(field(HLE, "cases"), 2_500);
    assert_eq!(field(HLE, "correct_authorized"), 2);
    assert_eq!(field(HLE, "incorrect_authorized"), 0);

    let report = Report {
        schema: "stage116-post-retrieval-composition-checkpoint-v1",
        parent_report_sha256: parents.iter().map(|parent| digest(parent)).collect(),
        composition_cases,
        supported_routes,
        ambiguous_or_refused: composition_cases - supported_routes,
        route_replays,
        tamper_rejections,
        false_authorizations,
        false_denials,
        self_directed_complete_plans: field(RETRIEVAL, "complete"),
        retrieval_provenance_mismatches: field(RETRIEVAL, "provenance_mismatches"),
        frozen_hle_cases: field(HLE, "cases"),
        frozen_hle_correct_authorized: field(HLE, "correct_authorized"),
        frozen_hle_false_authorizations: field(HLE, "incorrect_authorized"),
    };
    println!("{}", serde_json::to_string_pretty(&report).unwrap());
}
