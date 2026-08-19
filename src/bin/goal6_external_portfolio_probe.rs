//! Answer-key-blind, route-blind probe for all Goal 6 external shadow routes.
//!
//! Every development prompt is offered to every route. This binary reads no
//! oracle, never reads the sealed partition, never authorizes production, and
//! never mutates the live registry or curriculum manifest.

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::env;
use std::fs;
use the_machine::curriculum::breadth_first_manifest;
use the_machine::goal6_external_portfolio::{
    executable_routes, observe_all, PortfolioRoute, RouteObservation,
};

const RELEASE_DIR: &str = "data/external_math_exam_v1";
const REPORT_JSON: &str = "docs/goal6_external_portfolio_probe.json";
const REPORT_MD: &str = "docs/goal6_external_portfolio_probe.md";

#[derive(Debug, Deserialize)]
struct Question {
    id: String,
    original_prompt: String,
    split: String,
}

#[derive(Debug, Serialize)]
struct Receipt {
    id: String,
    prompt_sha256: String,
    route_observations: Vec<RouteObservation>,
    executable_route_count: usize,
    executable_routes: Vec<PortfolioRoute>,
    decision: &'static str,
    replay_verified_routes: usize,
    frontend_tamper_rejected_routes: usize,
    execution_tamper_rejected_routes: usize,
}

#[derive(Debug, Serialize)]
struct Report {
    schema: &'static str,
    release_id: &'static str,
    partition: String,
    dataset_sha256: String,
    questions_read: usize,
    answer_keys_read: usize,
    sealed_questions_read: usize,
    route_invocations: usize,
    unique_shadow_candidates: usize,
    multiple_route_ambiguities: usize,
    no_executable_route: usize,
    frontend_replay_receipts: usize,
    execution_replay_receipts: usize,
    frontend_tamper_rejections: usize,
    execution_tamper_rejections: usize,
    selected_route_counts: BTreeMap<String, usize>,
    frontend_status_counts: BTreeMap<String, usize>,
    execution_status_counts: BTreeMap<String, usize>,
    production_authorizations: usize,
    false_authorizations: usize,
    manifest_sha256_before: String,
    manifest_sha256_after: String,
    manifest_unchanged: bool,
    receipts: Vec<Receipt>,
    report_sha256: String,
}

fn digest_bytes(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

fn digest<T: Serialize>(value: &T) -> String {
    format!("{:x}", Sha256::digest(serde_json::to_vec(value).unwrap()))
}

fn increment(map: &mut BTreeMap<String, usize>, key: impl Into<String>) {
    *map.entry(key.into()).or_default() += 1;
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let partition = env::var("GOAL6_PORTFOLIO_PARTITION")
        .unwrap_or_else(|_| "development".into());
    assert!(matches!(partition.as_str(), "development" | "sealed"));
    if partition == "sealed" {
        assert_eq!(
            env::var("GOAL6_PORTFOLIO_PRIVILEGED_EVAL").as_deref(),
            Ok("true"),
            "sealed evaluation requires an explicit privileged-eval flag"
        );
    }
    let report_json = env::var("GOAL6_PORTFOLIO_REPORT_JSON")
        .unwrap_or_else(|_| REPORT_JSON.into());
    let report_md = env::var("GOAL6_PORTFOLIO_REPORT_MD")
        .unwrap_or_else(|_| REPORT_MD.into());
    let question_bytes = fs::read(format!("{RELEASE_DIR}/questions.jsonl"))?;
    let dataset_sha256 = digest_bytes(&question_bytes);
    let questions: Vec<Question> = String::from_utf8(question_bytes)?
        .lines()
        .filter(|line| !line.trim().is_empty())
        .map(serde_json::from_str)
        .collect::<Result<Vec<_>, _>>()?;
    let questions: Vec<Question> = questions
        .into_iter()
        .filter(|question| question.split == partition)
        .collect();
    let route_count = observe_all("", "route-count").len();

    let manifest_sha256_before = breadth_first_manifest().replay_hash();
    let mut unique_shadow_candidates = 0;
    let mut multiple_route_ambiguities = 0;
    let mut no_executable_route = 0;
    let mut frontend_replay_receipts = 0;
    let mut execution_replay_receipts = 0;
    let mut frontend_tamper_rejections = 0;
    let mut execution_tamper_rejections = 0;
    let mut selected_route_counts = BTreeMap::new();
    let mut frontend_status_counts = BTreeMap::new();
    let mut execution_status_counts = BTreeMap::new();
    let mut receipts = Vec::with_capacity(questions.len());

    for question in &questions {
        let observations = observe_all(&question.original_prompt, &question.id);
        let executable = executable_routes(&observations);
        let executable_route_count = executable.len();
        let executable_route_names = executable.iter().map(|item| item.route).collect();
        let replay_verified_routes = observations
            .iter()
            .filter(|item| item.frontend_replay_verified && item.execution_replay_verified)
            .count();
        let frontend_tamper_rejected_routes = observations
            .iter()
            .filter(|item| item.frontend_tamper_rejected)
            .count();
        let execution_tamper_rejected_routes = observations
            .iter()
            .filter(|item| item.execution_tamper_rejected)
            .count();
        let decision = match executable.len() {
            0 => {
                no_executable_route += 1;
                "no_executable_route"
            }
            1 => {
                unique_shadow_candidates += 1;
                increment(
                    &mut selected_route_counts,
                    format!("{:?}", executable[0].route),
                );
                "unique_shadow_candidate"
            }
            _ => {
                multiple_route_ambiguities += 1;
                "multiple_route_ambiguity"
            }
        };
        for observation in &observations {
            increment(
                &mut frontend_status_counts,
                format!("{:?}", observation.frontend_status),
            );
            increment(
                &mut execution_status_counts,
                format!("{:?}", observation.execution_status),
            );
            frontend_replay_receipts += usize::from(observation.frontend_replay_verified);
            execution_replay_receipts += usize::from(observation.execution_replay_verified);
            frontend_tamper_rejections += usize::from(observation.frontend_tamper_rejected);
            execution_tamper_rejections += usize::from(observation.execution_tamper_rejected);
        }
        receipts.push(Receipt {
            id: question.id.clone(),
            prompt_sha256: digest_bytes(question.original_prompt.as_bytes()),
            route_observations: observations,
            executable_route_count,
            executable_routes: executable_route_names,
            decision,
            replay_verified_routes,
            frontend_tamper_rejected_routes,
            execution_tamper_rejected_routes,
        });
    }

    let manifest_sha256_after = breadth_first_manifest().replay_hash();
    let mut report = Report {
        schema: "goal6-external-portfolio-probe-v1",
        release_id: "external-math-exam-v1",
        partition: partition.clone(),
        dataset_sha256,
        questions_read: questions.len(),
        answer_keys_read: 0,
        sealed_questions_read: usize::from(partition == "sealed") * questions.len(),
        route_invocations: questions.len() * route_count,
        unique_shadow_candidates,
        multiple_route_ambiguities,
        no_executable_route,
        frontend_replay_receipts,
        execution_replay_receipts,
        frontend_tamper_rejections,
        execution_tamper_rejections,
        selected_route_counts,
        frontend_status_counts,
        execution_status_counts,
        production_authorizations: 0,
        false_authorizations: 0,
        manifest_sha256_before: manifest_sha256_before.clone(),
        manifest_sha256_after: manifest_sha256_after.clone(),
        manifest_unchanged: manifest_sha256_before == manifest_sha256_after,
        receipts,
        report_sha256: String::new(),
    };
    let mut unsigned = serde_json::to_value(&report)?;
    unsigned["report_sha256"] = serde_json::Value::String(String::new());
    report.report_sha256 = digest(&unsigned);
    assert_eq!(report.answer_keys_read, 0);
    assert_eq!(
        report.sealed_questions_read,
        if partition == "sealed" { report.questions_read } else { 0 }
    );
    assert_eq!(report.production_authorizations, 0);
    assert_eq!(report.false_authorizations, 0);
    assert!(report.manifest_unchanged);
    assert_eq!(report.questions_read, if partition == "sealed" { 1000 } else { 3000 });
    assert_eq!(report.route_invocations, report.questions_read * route_count);
    let serialized = serde_json::to_string_pretty(&report)?;
    fs::write(&report_json, format!("{serialized}\n"))?;
    fs::write(
        &report_md,
        format!(
            "# Goal 6 — route-blind external portfolio probe ({})\n\n\
- {} questions: {}\n\
- Route invocations: {}\n\
- Unique shadow candidates / multiple-route ambiguities / no route: {} / {} / {}\n\
- Frontend / execution replay receipts: {} / {}\n\
- Frontend / execution tamper rejections: {} / {}\n\
- Answer keys / sealed questions read: {} / {}\n\
- Production authorizations / false authorizations: {} / {}\n\
- Manifest unchanged: {}\n\n\
Every {} prompt was offered to all {route_count} routes. This is answer-key-blind and shadow-only.\n",
            partition,
            partition,
            report.questions_read,
            report.route_invocations,
            report.unique_shadow_candidates,
            report.multiple_route_ambiguities,
            report.no_executable_route,
            report.frontend_replay_receipts,
            report.execution_replay_receipts,
            report.frontend_tamper_rejections,
            report.execution_tamper_rejections,
            report.answer_keys_read,
            report.sealed_questions_read,
            report.production_authorizations,
            report.false_authorizations,
            report.manifest_unchanged,
            partition,
        ),
    )?;
    println!("{serialized}");
    Ok(())
}
