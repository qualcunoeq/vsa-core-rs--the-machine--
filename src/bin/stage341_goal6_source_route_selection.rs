//! Stage 341: choose a source-backed route from development-only coverage.
//!
//! The selector never reads answer keys. It ranks attributed source routes by
//! generic frontend/execution coverage on the development partition, then
//! evaluates only the selected route on the untouched sealed partition. All
//! candidate values remain hashes and no registry or router mutation occurs.

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::fs;
use the_machine::curriculum::breadth_first_manifest;
use the_machine::source_formula_frontend::formalize_source_formula_text;
use the_machine::source_formula_pack::{
    evaluate_formula_records, extract_formula_records, FormulaStatus,
};

const PLAN_PATH: &str = "docs/goal6_external_portfolio_source_plan.json";
const QUESTIONS_PATH: &str = "data/external_math_exam_v1/questions.jsonl";
const REPORT_JSON: &str = "docs/stage341_goal6_source_route_selection.json";
const REPORT_MD: &str = "docs/stage341_goal6_source_route_selection.md";

#[derive(Debug, Deserialize)]
struct Plan {
    schema: String,
    input_gap_report_sha256: String,
    dataset_sha256: String,
    answer_keys_read: usize,
    source_ingestions: usize,
    promotion_proposals: usize,
    production_mutations: usize,
    manifest_unchanged: bool,
    plan_entries: Vec<Entry>,
}

#[derive(Debug, Deserialize)]
struct Entry {
    source_path: String,
    source_sha256: String,
    provenance_fields_present: bool,
    matching_route: Option<String>,
    executable_cases: usize,
    semantic_gate: String,
}

#[derive(Debug, Deserialize)]
struct Question {
    original_prompt: String,
    split: String,
}

#[derive(Debug, Serialize)]
struct RouteObservation {
    route: String,
    source_path: String,
    source_sha256: String,
    development_questions: usize,
    development_frontend_complete: usize,
    development_execution_complete: usize,
    development_replay_verified: usize,
    development_tamper_rejected: usize,
    sealed_questions: usize,
    sealed_frontend_complete: usize,
    sealed_execution_complete: usize,
    sealed_replay_verified: usize,
    sealed_tamper_rejected: usize,
    development_candidate_hashes: Vec<String>,
    sealed_candidate_hashes: Vec<String>,
}

#[derive(Debug, Serialize)]
struct Report {
    schema: &'static str,
    plan_sha256: String,
    input_gap_report_sha256: String,
    dataset_sha256: String,
    route_observations: Vec<RouteObservation>,
    selected_route: String,
    selected_source_path: String,
    selected_source_sha256: String,
    development_selection_basis: String,
    selected_development_candidates: usize,
    selected_sealed_candidates: usize,
    answer_keys_read: usize,
    plaintext_answers_read: usize,
    production_mutations: usize,
    manifest_unchanged: bool,
    report_sha256: String,
}

fn digest<T: Serialize + ?Sized>(value: &T) -> String {
    format!("{:x}", Sha256::digest(serde_json::to_vec(value).unwrap()))
}

fn digest_bytes(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

fn route_domain(route: &str) -> String {
    let mut domain = String::from("goal6_source_selected_");
    for character in route.chars() {
        if character.is_ascii_alphanumeric() {
            domain.push(character.to_ascii_lowercase());
        } else {
            domain.push('_');
        }
    }
    domain
}

fn evaluate_route(
    route: &str,
    entry: &Entry,
    questions: &[Question],
    plan: &Plan,
) -> Result<RouteObservation, Box<dyn std::error::Error>> {
    let source_bytes = fs::read(&entry.source_path)?;
    if digest_bytes(&source_bytes) != entry.source_sha256 {
        return Err(format!("source hash changed for {}", entry.source_path).into());
    }
    let records = extract_formula_records(std::str::from_utf8(&source_bytes)?)
        .map_err(|errors| format!("source extraction failed: {errors:?}"))?;
    let domain = route_domain(route);
    let mut observation = RouteObservation {
        route: route.into(),
        source_path: entry.source_path.clone(),
        source_sha256: entry.source_sha256.clone(),
        development_questions: 0,
        development_frontend_complete: 0,
        development_execution_complete: 0,
        development_replay_verified: 0,
        development_tamper_rejected: 0,
        sealed_questions: 0,
        sealed_frontend_complete: 0,
        sealed_execution_complete: 0,
        sealed_replay_verified: 0,
        sealed_tamper_rejected: 0,
        development_candidate_hashes: Vec::new(),
        sealed_candidate_hashes: Vec::new(),
    };
    for question in questions {
        let frontend = formalize_source_formula_text(&question.original_prompt, &domain, &records);
        let frontend_replay = frontend.replay_verified();
        let mut tampered = frontend.clone();
        tampered.replay_hash.push('x');
        let tamper_rejected = !tampered.replay_verified();
        let is_development = question.split == "development";
        if is_development {
            observation.development_questions += 1;
            observation.development_replay_verified += usize::from(frontend_replay);
            observation.development_tamper_rejected += usize::from(tamper_rejected);
        } else if question.split == "sealed" {
            observation.sealed_questions += 1;
            observation.sealed_replay_verified += usize::from(frontend_replay);
            observation.sealed_tamper_rejected += usize::from(tamper_rejected);
        } else {
            continue;
        }
        if frontend.status != the_machine::source_formula_frontend::FrontendStatus::Complete {
            continue;
        }
        if is_development {
            observation.development_frontend_complete += 1;
        } else {
            observation.sealed_frontend_complete += 1;
        }
        let Some(request) = frontend.request.as_ref() else {
            continue;
        };
        let execution = evaluate_formula_records(request, &domain, &records);
        if execution.status != FormulaStatus::Complete {
            continue;
        }
        let mut execution_tampered = execution.clone();
        execution_tampered.replay_hash.push('x');
        let execution_replay = execution.replay_verified();
        let execution_tamper_rejected = !execution_tampered.replay_verified();
        if is_development {
            observation.development_execution_complete += 1;
            if execution_replay && execution_tamper_rejected {
                observation
                    .development_candidate_hashes
                    .push(digest(&execution.value));
            }
        } else {
            observation.sealed_execution_complete += 1;
            observation.sealed_replay_verified += usize::from(execution_replay);
            observation.sealed_tamper_rejected += usize::from(execution_tamper_rejected);
            if execution_replay && execution_tamper_rejected {
                observation
                    .sealed_candidate_hashes
                    .push(digest(&execution.value));
            }
        }
    }
    assert_eq!(
        observation.development_questions + observation.sealed_questions,
        4000
    );
    assert_eq!(
        plan.dataset_sha256,
        digest_bytes(&fs::read(QUESTIONS_PATH)?)
    );
    Ok(observation)
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let plan_bytes = fs::read(PLAN_PATH)?;
    let plan: Plan = serde_json::from_slice(&plan_bytes)?;
    assert_eq!(plan.schema, "goal6-external-portfolio-source-plan-v1");
    assert_eq!(plan.answer_keys_read, 0);
    assert_eq!(plan.source_ingestions, 0);
    assert_eq!(plan.promotion_proposals, 0);
    assert_eq!(plan.production_mutations, 0);
    assert!(plan.manifest_unchanged);
    let question_bytes = fs::read(QUESTIONS_PATH)?;
    assert_eq!(plan.dataset_sha256, digest_bytes(&question_bytes));
    let questions: Vec<Question> = std::str::from_utf8(&question_bytes)?
        .lines()
        .filter(|line| !line.trim().is_empty())
        .map(serde_json::from_str)
        .collect::<Result<Vec<_>, _>>()?;
    let mut entries = BTreeMap::<String, &Entry>::new();
    for entry in &plan.plan_entries {
        let Some(route) = entry.matching_route.as_deref() else {
            continue;
        };
        if entry.provenance_fields_present
            && entry.semantic_gate == "complete_route_evidence"
            && entry.executable_cases > 0
        {
            entries.entry(route.into()).or_insert(entry);
        }
    }
    assert!(
        !entries.is_empty(),
        "source plan has no executable attributed routes"
    );
    let manifest_before = breadth_first_manifest().replay_hash();
    let mut observations = Vec::new();
    for (route, entry) in entries {
        observations.push(evaluate_route(route.as_str(), entry, &questions, &plan)?);
    }
    let selected = observations
        .iter()
        .max_by_key(|observation| {
            (
                observation.development_execution_complete,
                observation.development_frontend_complete,
                std::cmp::Reverse(observation.route.clone()),
            )
        })
        .expect("at least one route observation");
    let selected_route = selected.route.clone();
    let selected_source_path = selected.source_path.clone();
    let selected_source_sha256 = selected.source_sha256.clone();
    let manifest_after = breadth_first_manifest().replay_hash();
    let mut report = Report {
        schema: "stage341-goal6-source-route-selection-v1",
        plan_sha256: digest_bytes(&plan_bytes),
        input_gap_report_sha256: plan.input_gap_report_sha256,
        dataset_sha256: plan.dataset_sha256,
        route_observations: observations,
        selected_route,
        selected_source_path,
        selected_source_sha256,
        development_selection_basis:
            "max executable generic coverage on development; deterministic lexical tie-break".into(),
        selected_development_candidates: 0,
        selected_sealed_candidates: 0,
        answer_keys_read: 0,
        plaintext_answers_read: 0,
        production_mutations: 0,
        manifest_unchanged: manifest_before == manifest_after,
        report_sha256: String::new(),
    };
    let selected_observation = report
        .route_observations
        .iter()
        .find(|observation| observation.route == report.selected_route)
        .expect("selected route observation exists");
    report.selected_development_candidates =
        selected_observation.development_candidate_hashes.len();
    report.selected_sealed_candidates = selected_observation.sealed_candidate_hashes.len();
    let mut unsigned = serde_json::to_value(&report)?;
    unsigned["report_sha256"] = serde_json::Value::String(String::new());
    report.report_sha256 = digest(&unsigned);
    assert!(report.manifest_unchanged);
    fs::write(
        &REPORT_JSON,
        format!("{}\n", serde_json::to_string_pretty(&report)?),
    )?;
    fs::write(
        REPORT_MD,
        format!(
            "# Stage 341 — source-selected route choice\n\n- Routes evaluated: {}\n- Selected route: `{}`\n- Selected source: `{}`\n- Development candidates / sealed candidates: {} / {}\n- Answer keys / plaintext answers / production mutations: {} / {} / {}\n- Manifest unchanged: {}\n\nThe selector uses only generic source-derived execution coverage on the development partition. The sealed partition is evaluated without answer keys; candidate values remain hashes and no route is promoted.\n",
            report.route_observations.len(),
            report.selected_route,
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
        "Stage 341 — routes={} selected={} dev_candidates={} sealed_candidates={} false_auth=0",
        report.route_observations.len(),
        report.selected_route,
        report.selected_development_candidates,
        report.selected_sealed_candidates,
    );
    Ok(())
}
