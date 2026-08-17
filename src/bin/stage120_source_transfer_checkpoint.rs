//! Stage 120: source-backed language, bridge, and memory transfer checkpoint.

use serde::Serialize;
use sha2::{Digest, Sha256};

const ECON_FRONTEND: &str = include_str!("../../docs/stage_ag_source_formula_frontend.json");
const ECON_PROBABILITY: &str =
    include_str!("../../docs/stage_ah_source_probability_composition.json");
const ROUTE_BLIND: &str =
    include_str!("../../docs/stage_ai_generic_source_catalog_route_blind.json");
const NUMBER_LANGUAGE: &str =
    include_str!("../../docs/stage117_number_theory_language_transfer.json");
const ADMISSION: &str = include_str!("../../docs/stage118_source_domain_manifest_admission.json");
const MEMORY: &str = include_str!("../../docs/stage119_source_memory_ingestion.json");

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
    source_frontend_development_cases: usize,
    source_frontend_exact_decisions: usize,
    source_frontend_holdout_cases: usize,
    source_frontend_holdout_exact: usize,
    source_probability_cases: usize,
    source_probability_complete: usize,
    route_blind_cases: usize,
    route_blind_exact: usize,
    number_language_cases: usize,
    number_language_downstream_complete: usize,
    number_language_false_authorizations: usize,
    source_records: usize,
    source_memory_total: usize,
    source_memory_replay: usize,
    source_memory_tamper_rejected: usize,
    aggregate_false_authorizations: usize,
    aggregate_false_denials: usize,
    live_mutations: usize,
}

fn main() {
    let parents = [
        ECON_FRONTEND,
        ECON_PROBABILITY,
        ROUTE_BLIND,
        NUMBER_LANGUAGE,
        ADMISSION,
        MEMORY,
    ];
    let aggregate_false_authorizations = field(ECON_FRONTEND, "false_authorizations")
        + field(ECON_PROBABILITY, "false_authorizations")
        + field(ROUTE_BLIND, "false_authorizations")
        + field(NUMBER_LANGUAGE, "false_authorizations")
        + field(ADMISSION, "false_authorizations")
        + field(MEMORY, "source_contamination");
    let aggregate_false_denials = field(ECON_FRONTEND, "false_denials")
        + field(ECON_PROBABILITY, "false_denials")
        + field(ROUTE_BLIND, "false_denials")
        + field(NUMBER_LANGUAGE, "false_denials")
        + field(ADMISSION, "false_denials");
    assert_eq!(aggregate_false_authorizations, 0);
    assert_eq!(aggregate_false_denials, 0);
    assert_eq!(field(ADMISSION, "source_records"), 5);
    assert_eq!(field(MEMORY, "total_records"), 100_005);
    assert_eq!(field(MEMORY, "replay_verified"), 5);
    assert_eq!(field(MEMORY, "tamper_rejected"), 5);

    let report = Report {
        schema: "stage120-source-transfer-checkpoint-v1",
        parent_report_sha256: parents.iter().map(|parent| digest(parent)).collect(),
        source_frontend_development_cases: field(ECON_FRONTEND, "development_cases"),
        source_frontend_exact_decisions: field(ECON_FRONTEND, "frontend_exact_decisions"),
        source_frontend_holdout_cases: field(ECON_FRONTEND, "holdout_cases"),
        source_frontend_holdout_exact: field(ECON_FRONTEND, "holdout_frontend_exact"),
        source_probability_cases: field(ECON_PROBABILITY, "cases"),
        source_probability_complete: field(ECON_PROBABILITY, "complete_expectations"),
        route_blind_cases: field(ROUTE_BLIND, "cases"),
        route_blind_exact: field(ROUTE_BLIND, "exact_route_decisions"),
        number_language_cases: field(NUMBER_LANGUAGE, "cases"),
        number_language_downstream_complete: field(NUMBER_LANGUAGE, "downstream_complete"),
        number_language_false_authorizations: field(NUMBER_LANGUAGE, "false_authorizations"),
        source_records: field(ADMISSION, "source_records"),
        source_memory_total: field(MEMORY, "total_records"),
        source_memory_replay: field(MEMORY, "replay_verified"),
        source_memory_tamper_rejected: field(MEMORY, "tamper_rejected"),
        aggregate_false_authorizations,
        aggregate_false_denials,
        live_mutations: field(ECON_FRONTEND, "live_mutations")
            + field(ECON_PROBABILITY, "live_mutations")
            + field(ROUTE_BLIND, "live_mutations")
            + field(NUMBER_LANGUAGE, "live_mutations")
            + field(ADMISSION, "live_route_mutations")
            + field(MEMORY, "live_route_mutations"),
    };
    assert_eq!(report.live_mutations, 0);
    println!("{}", serde_json::to_string_pretty(&report).unwrap());
}
