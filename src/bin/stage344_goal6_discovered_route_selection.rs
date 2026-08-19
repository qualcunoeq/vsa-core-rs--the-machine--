//! Stage 344: answer-key-blind route selection over discovered source modules.
//!
//! Stage 343 only built the catalog inventory.  This stage consumes that
//! inventory and evaluates every admitted module against the external
//! development/sealed portfolio.  The route is selected by generic frontend
//! and execution coverage on development only; no source subject, formula
//! name, answer key, or plaintext value is used by the selector.

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::fs;
use the_machine::curriculum::breadth_first_manifest;
use the_machine::source_formula_frontend::{
    formalize_source_formula_report, report_replay_verified, FrontendStatus,
};
use the_machine::source_formula_pack::{evaluate_formula_records, FormulaStatus};
use the_machine::source_module_discovery::{
    discover_formula_module, replay_verified, SourceDocument,
};

const DISCOVERY_REPORT: &str = "docs/stage343_goal6_source_catalog_discovery.json";
const QUESTIONS_PATH: &str = "data/external_math_exam_v1/questions.jsonl";
const REPORT_JSON: &str = "docs/stage344_goal6_discovered_route_selection.json";
const REPORT_MD: &str = "docs/stage344_goal6_discovered_route_selection.md";

#[derive(Debug, Deserialize)]
struct DiscoveryReport {
    schema: String,
    source_manifest_sha256: String,
    source_observations: Vec<SourceObservation>,
}

#[derive(Debug, Deserialize)]
struct SourceObservation {
    path: String,
    sha256: String,
    kind: String,
}

#[derive(Debug, Deserialize)]
struct Question {
    original_prompt: String,
    split: String,
}

#[derive(Debug, Clone, Serialize)]
struct RouteObservation {
    module_id: String,
    source_path: String,
    source_sha256: String,
    source_record_count: usize,
    development_questions: usize,
    development_frontend_complete: usize,
    development_execution_complete: usize,
    development_frontend_replay_verified: usize,
    development_frontend_tamper_rejected: usize,
    development_execution_replay_verified: usize,
    development_execution_tamper_rejected: usize,
    sealed_questions: usize,
    sealed_frontend_complete: usize,
    sealed_execution_complete: usize,
    sealed_frontend_replay_verified: usize,
    sealed_frontend_tamper_rejected: usize,
    sealed_execution_replay_verified: usize,
    sealed_execution_tamper_rejected: usize,
    development_candidate_hashes: Vec<String>,
    sealed_candidate_hashes: Vec<String>,
}

#[derive(Debug, Serialize)]
struct Report {
    schema: &'static str,
    discovery_report_sha256: String,
    source_manifest_sha256: String,
    dataset_sha256: String,
    routes_evaluated: usize,
    route_observations: Vec<RouteObservation>,
    selected_module_id: String,
    selected_source_path: String,
    selected_development_candidates: usize,
    selected_sealed_candidates: usize,
    answer_keys_read: usize,
    plaintext_answers_read: usize,
    production_mutations: usize,
    manifest_unchanged: bool,
    false_authorizations: usize,
    report_sha256: String,
}

fn digest<T: Serialize + ?Sized>(value: &T) -> String {
    format!("{:x}", Sha256::digest(serde_json::to_vec(value).unwrap()))
}

fn digest_bytes(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

fn evaluate_module(
    path: &str,
    source_sha256: &str,
    questions: &[Question],
) -> Result<RouteObservation, Box<dyn std::error::Error>> {
    let bytes = fs::read(path)?;
    if digest_bytes(&bytes) != source_sha256 {
        return Err(format!("source hash changed for {path}").into());
    }
    let text = std::str::from_utf8(&bytes)?;
    let module = discover_formula_module(SourceDocument {
        domain: "discovered_source_catalog",
        version: source_sha256,
        source_hint: "",
        document: text,
    })
    .map_err(|errors| format!("source discovery failed for {path}: {errors:?}"))?;
    if !replay_verified(&module) {
        return Err(format!("discovery replay failed for {path}").into());
    }
    let domain = &module.candidate.domain;
    let mut observation = RouteObservation {
        module_id: module.candidate.module_id,
        source_path: path.to_owned(),
        source_sha256: source_sha256.to_owned(),
        source_record_count: module.records.len(),
        development_questions: 0,
        development_frontend_complete: 0,
        development_execution_complete: 0,
        development_frontend_replay_verified: 0,
        development_frontend_tamper_rejected: 0,
        development_execution_replay_verified: 0,
        development_execution_tamper_rejected: 0,
        sealed_questions: 0,
        sealed_frontend_complete: 0,
        sealed_execution_complete: 0,
        sealed_frontend_replay_verified: 0,
        sealed_frontend_tamper_rejected: 0,
        sealed_execution_replay_verified: 0,
        sealed_execution_tamper_rejected: 0,
        development_candidate_hashes: Vec::new(),
        sealed_candidate_hashes: Vec::new(),
    };
    for question in questions {
        let is_development = question.split == "development";
        let is_sealed = question.split == "sealed";
        if !is_development && !is_sealed {
            continue;
        }
        let report =
            formalize_source_formula_report(&question.original_prompt, domain, &module.records);
        let frontend_replay = report_replay_verified(&report);
        let mut frontend_tampered = report.clone();
        frontend_tampered.replay_hash.push('x');
        let frontend_tamper_rejected = !report_replay_verified(&frontend_tampered);
        if is_development {
            observation.development_questions += 1;
            observation.development_frontend_replay_verified += usize::from(frontend_replay);
            observation.development_frontend_tamper_rejected +=
                usize::from(frontend_tamper_rejected);
        } else {
            observation.sealed_questions += 1;
            observation.sealed_frontend_replay_verified += usize::from(frontend_replay);
            observation.sealed_frontend_tamper_rejected += usize::from(frontend_tamper_rejected);
        }
        if report.frontend.status != FrontendStatus::Complete {
            continue;
        }
        if is_development {
            observation.development_frontend_complete += 1;
        } else {
            observation.sealed_frontend_complete += 1;
        }
        let Some(request) = report.frontend.request.as_ref() else {
            continue;
        };
        let execution = evaluate_formula_records(request, domain, &module.records);
        if execution.status != FormulaStatus::Complete {
            continue;
        }
        let execution_replay = execution.replay_verified();
        let mut execution_tampered = execution.clone();
        execution_tampered.replay_hash.push('x');
        let execution_tamper_rejected = !execution_tampered.replay_verified();
        if is_development {
            observation.development_execution_complete += 1;
            observation.development_execution_replay_verified += usize::from(execution_replay);
            observation.development_execution_tamper_rejected +=
                usize::from(execution_tamper_rejected);
            if execution_replay && execution_tamper_rejected {
                observation
                    .development_candidate_hashes
                    .push(digest(&execution.value));
            }
        } else {
            observation.sealed_execution_complete += 1;
            observation.sealed_execution_replay_verified += usize::from(execution_replay);
            observation.sealed_execution_tamper_rejected += usize::from(execution_tamper_rejected);
            if execution_replay && execution_tamper_rejected {
                observation
                    .sealed_candidate_hashes
                    .push(digest(&execution.value));
            }
        }
    }
    Ok(observation)
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let discovery_bytes = fs::read(DISCOVERY_REPORT)?;
    let discovery: DiscoveryReport = serde_json::from_slice(&discovery_bytes)?;
    assert_eq!(
        discovery.schema,
        "stage343-goal6-source-catalog-discovery-v1"
    );
    let question_bytes = fs::read(QUESTIONS_PATH)?;
    let questions: Vec<Question> = std::str::from_utf8(&question_bytes)?
        .lines()
        .filter(|line| !line.trim().is_empty())
        .map(serde_json::from_str)
        .collect::<Result<Vec<_>, _>>()?;
    assert_eq!(
        questions
            .iter()
            .filter(|q| q.split == "development")
            .count(),
        3000
    );
    assert_eq!(
        questions.iter().filter(|q| q.split == "sealed").count(),
        1000
    );
    let manifest_before = breadth_first_manifest().replay_hash();
    let mut routes = Vec::new();
    for source in discovery
        .source_observations
        .iter()
        .filter(|source| source.kind == "formula_source_module")
    {
        routes.push(evaluate_module(&source.path, &source.sha256, &questions)?);
    }
    assert!(!routes.is_empty());
    let selected_index = routes
        .iter()
        .enumerate()
        .max_by_key(|(_, route)| {
            (
                route.development_execution_complete,
                route.development_frontend_complete,
                std::cmp::Reverse(route.source_path.clone()),
            )
        })
        .map(|(index, _)| index)
        .unwrap();
    let selected = routes[selected_index].clone();
    let manifest_unchanged = manifest_before == breadth_first_manifest().replay_hash();
    let mut report = Report {
        schema: "stage344-goal6-discovered-route-selection-v1",
        discovery_report_sha256: digest_bytes(&discovery_bytes),
        source_manifest_sha256: discovery.source_manifest_sha256,
        dataset_sha256: digest_bytes(&question_bytes),
        routes_evaluated: routes.len(),
        route_observations: routes,
        selected_module_id: selected.module_id,
        selected_source_path: selected.source_path,
        selected_development_candidates: selected.development_candidate_hashes.len(),
        selected_sealed_candidates: selected.sealed_candidate_hashes.len(),
        answer_keys_read: 0,
        plaintext_answers_read: 0,
        production_mutations: 0,
        manifest_unchanged,
        false_authorizations: 0,
        report_sha256: String::new(),
    };
    assert!(report.manifest_unchanged);
    let mut unsigned = serde_json::to_value(&report)?;
    unsigned["report_sha256"] = serde_json::Value::String(String::new());
    report.report_sha256 = digest(&unsigned);
    let json = serde_json::to_string_pretty(&report)?;
    fs::write(REPORT_JSON, format!("{json}\n"))?;
    fs::write(
        REPORT_MD,
        format!(
            "# Stage 344 — discovered source route selection\n\n\
* discovered routes evaluated: {}\n\
* selected module: `{}`\n\
* selected source: `{}`\n\
* selected development/sealed candidates: {} / {}\n\
* answer keys / plaintext answers / production mutations: {} / {} / {}\n\
* manifest unchanged: {}\n\n\
Every admitted route came from the Stage 343 source inventory. Selection used only generic frontend/execution coverage on development; sealed candidate values remain hashes and were not used to choose the route. No source subject list, answer key, curriculum mutation, or production authorization was used.\n",
            report.routes_evaluated,
            report.selected_module_id,
            report.selected_source_path,
            report.selected_development_candidates,
            report.selected_sealed_candidates,
            report.answer_keys_read,
            report.plaintext_answers_read,
            report.production_mutations,
            report.manifest_unchanged,
        ),
    )?;
    println!(
        "Stage 344 — routes={} selected={} dev_candidates={} sealed_candidates={} false_auth=0",
        report.routes_evaluated,
        report.selected_source_path,
        report.selected_development_candidates,
        report.selected_sealed_candidates
    );
    Ok(())
}
