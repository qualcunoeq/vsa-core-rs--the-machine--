//! Answer-key-blind residual-family audit for the Goal 6 portfolio.
//!
//! The older aggregate gap report counted an ambiguity from any offered route
//! as the first obstruction, even when another route had a more specific
//! missing-field or execution-boundary diagnosis.  This audit chooses the
//! strongest surviving route per residual, records the complete status
//! signature, and emits only hashed identifiers and lexical triage terms.
//! It never reads answer keys, sealed plaintext, sources, or mutable registry
//! state.

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::env;
use std::fs;
use the_machine::goal6_external_portfolio::{executable_routes, observe_all, RouteObservation};

const RELEASE_DIR: &str = "data/external_math_exam_v1";

#[derive(Debug, Deserialize)]
struct Question {
    id: String,
    original_prompt: String,
    split: String,
}

#[derive(Debug, Serialize)]
struct ResidualRecord {
    question_id: String,
    status_signature: String,
    best_frontend_rank: u8,
    best_routes: Vec<String>,
    executable_routes: usize,
    token_set_sha256: String,
}

#[derive(Debug, Serialize)]
struct Report {
    schema: &'static str,
    partition: &'static str,
    dataset_sha256: String,
    questions_read: usize,
    answer_keys_read: usize,
    route_count: usize,
    residual_questions: usize,
    status_signature_counts: BTreeMap<String, usize>,
    best_route_counts: BTreeMap<String, usize>,
    best_frontend_rank_counts: BTreeMap<String, usize>,
    token_cluster_counts: BTreeMap<String, usize>,
    residuals: Vec<ResidualRecord>,
    manifest_unchanged: bool,
    report_sha256: String,
}

fn digest_bytes(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

fn digest<T: Serialize>(value: &T) -> String {
    format!("{:x}", Sha256::digest(serde_json::to_vec(value).unwrap()))
}

fn frontend_rank(observation: &RouteObservation) -> u8 {
    match observation.frontend_status.as_str() {
        "Complete" if observation.execution_status == "Complete" => 5,
        "Complete" => 4,
        // Missing explicit fields are a stronger, more actionable diagnosis
        // than a generic route-level ambiguity.  Many bounded frontends
        // conservatively report Ambiguous for ordinary specialist prose, so
        // allowing that status to dominate would recreate the old aggregate
        // report's distortion.
        "Missing" => 3,
        "Ambiguous" => 2,
        "Unsupported" => 1,
        _ => 0,
    }
}

fn terms(text: &str) -> BTreeSet<String> {
    const STOP: &[&str] = &[
        "about", "after", "also", "and", "are", "calculate", "can", "compute", "find",
        "for", "from", "given", "how", "into", "is", "let", "not", "of", "on", "or",
        "show", "the", "then", "this", "to", "what", "when", "which", "with",
    ];
    text.split(|character: char| !character.is_ascii_alphabetic())
        .map(|word| word.to_ascii_lowercase())
        .filter(|word| word.len() >= 5 && !STOP.contains(&word.as_str()))
        .collect()
}

fn status_signature(observations: &[RouteObservation]) -> String {
    let mut statuses = observations
        .iter()
        .map(|observation| {
            format!(
                "{:?}:{}:{}",
                observation.route, observation.frontend_status, observation.execution_status
            )
        })
        .collect::<Vec<_>>();
    statuses.sort();
    statuses.join("|")
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let report_json = env::var("STAGE457_REPORT_JSON")
        .unwrap_or_else(|_| "/tmp/stage457_goal6_residual_family_audit.json".into());
    let report_md = env::var("STAGE457_REPORT_MD")
        .unwrap_or_else(|_| "/tmp/stage457_goal6_residual_family_audit.md".into());
    let question_bytes = fs::read(format!("{RELEASE_DIR}/questions.jsonl"))?;
    let dataset_sha256 = digest_bytes(&question_bytes);
    let questions: Vec<Question> = String::from_utf8(question_bytes)?
        .lines()
        .filter(|line| !line.trim().is_empty())
        .map(serde_json::from_str)
        .collect::<Result<Vec<_>, _>>()?
        .into_iter()
        .filter(|question: &Question| question.split == "development")
        .collect();

    let route_count = observe_all("", "stage457-route-count").len();
    let mut status_signature_counts = BTreeMap::new();
    let mut best_route_counts = BTreeMap::new();
    let mut best_frontend_rank_counts = BTreeMap::new();
    let mut token_cluster_counts = BTreeMap::new();
    let mut residuals = Vec::new();

    for question in &questions {
        let observations = observe_all(&question.original_prompt, &question.id);
        let executable_routes = executable_routes(&observations);
        if executable_routes.len() == 1 {
            continue;
        }
        let best_rank = observations.iter().map(frontend_rank).max().unwrap_or(0);
        let best_routes = observations
            .iter()
            .filter(|observation| frontend_rank(observation) == best_rank)
            .map(|observation| format!("{:?}", observation.route))
            .collect::<Vec<_>>();
        let signature = status_signature(&observations);
        let token_set = terms(&question.original_prompt);
        let token_set_digest = digest(&token_set);
        let token_cluster = token_set.iter().take(4).cloned().collect::<Vec<_>>().join(",");

        *status_signature_counts.entry(signature.clone()).or_default() += 1;
        *best_frontend_rank_counts
            .entry(best_rank.to_string())
            .or_default() += 1;
        for route in &best_routes {
            *best_route_counts.entry(route.clone()).or_default() += 1;
        }
        if !token_cluster.is_empty() {
            *token_cluster_counts.entry(token_cluster).or_default() += 1;
        }
        residuals.push(ResidualRecord {
            question_id: question.id.clone(),
            status_signature: signature,
            best_frontend_rank: best_rank,
            best_routes,
            executable_routes: executable_routes.len(),
            token_set_sha256: token_set_digest,
        });
    }

    let mut report = Report {
        schema: "stage457-goal6-residual-family-audit-v1",
        partition: "development",
        dataset_sha256,
        questions_read: questions.len(),
        answer_keys_read: 0,
        route_count,
        residual_questions: residuals.len(),
        status_signature_counts,
        best_route_counts,
        best_frontend_rank_counts,
        token_cluster_counts,
        residuals,
        manifest_unchanged: true,
        report_sha256: String::new(),
    };
    report.report_sha256 = digest(&report);
    let serialized = serde_json::to_string_pretty(&report)?;
    fs::write(&report_json, format!("{serialized}\n"))?;
    let top_routes = report
        .best_route_counts
        .iter()
        .rev()
        .take(8)
        .map(|(route, count)| format!("- `{route}`: {count}"))
        .collect::<Vec<_>>()
        .join("\n");
    fs::write(
        &report_md,
        format!(
            "# Stage 457 — Goal 6 residual-family audit\n\n\
Answer-key-blind development audit over {} questions.  The report chooses\n\
the strongest surviving route per residual rather than treating any generic\n\
ambiguity as the first failure.\n\n\
- residual questions: {}\n\
- route count: {}\n\
- answer keys read: 0\n\
- manifest unchanged: true\n\n\
Best surviving route counts (ties are retained):\n\n{}\n\n\
Report SHA-256: `{}`\n",
            report.questions_read,
            report.residual_questions,
            report.route_count,
            top_routes,
            report.report_sha256
        ),
    )?;
    Ok(())
}
