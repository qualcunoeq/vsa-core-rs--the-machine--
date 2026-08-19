//! Answer-key-blind residual samples for the Goal 6 portfolio.
//!
//! This report preserves a small, hash-bound sample of each route's first
//! frontend statuses.  It is diagnostic only: no oracle, sealed question,
//! source ingestion, promotion, or production mutation is permitted.

use serde::Serialize;
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::fs;
use the_machine::curriculum::breadth_first_manifest;
use the_machine::goal6_external_portfolio::{observe_all, PortfolioRoute};

const RELEASE_DIR: &str = "data/external_math_exam_v1";
const REPORT_JSON: &str = "docs/goal6_external_portfolio_residual_samples.json";
const REPORT_MD: &str = "docs/goal6_external_portfolio_residual_samples.md";
const SAMPLE_LIMIT: usize = 8;

#[derive(Debug, serde::Deserialize)]
struct Question {
    id: String,
    original_prompt: String,
    split: String,
}

#[derive(Debug, Serialize)]
struct Sample {
    case_id: String,
    prompt_sha256: String,
    prompt_preview: String,
    frontend_status: String,
    execution_status: String,
}

#[derive(Debug, Serialize)]
struct RouteSummary {
    route: PortfolioRoute,
    frontend_status_counts: BTreeMap<String, usize>,
    execution_status_counts: BTreeMap<String, usize>,
    samples_by_frontend_status: BTreeMap<String, Vec<Sample>>,
}

#[derive(Debug, Serialize)]
struct Report {
    schema: &'static str,
    release_id: &'static str,
    partition: &'static str,
    dataset_sha256: String,
    questions_read: usize,
    answer_keys_read: usize,
    sealed_questions_read: usize,
    route_invocations: usize,
    sample_limit: usize,
    route_summaries: Vec<RouteSummary>,
    manifest_sha256_before: String,
    manifest_sha256_after: String,
    manifest_unchanged: bool,
    production_mutations: usize,
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

fn preview(text: &str) -> String {
    let compact = text.split_whitespace().collect::<Vec<_>>().join(" ");
    compact.chars().take(240).collect()
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let question_bytes = fs::read(format!("{RELEASE_DIR}/questions.jsonl"))?;
    let dataset_sha256 = digest_bytes(&question_bytes);
    let questions: Vec<Question> = String::from_utf8(question_bytes)?
        .lines()
        .filter(|line| !line.trim().is_empty())
        .map(serde_json::from_str)
        .collect::<Result<Vec<_>, _>>()?;
    let questions: Vec<Question> = questions
        .into_iter()
        .filter(|question| question.split == "development")
        .collect();

    let mut summaries: BTreeMap<PortfolioRoute, RouteSummary> = BTreeMap::new();
    for question in &questions {
        for observation in observe_all(&question.original_prompt, &question.id) {
            let summary = summaries
                .entry(observation.route)
                .or_insert_with(|| RouteSummary {
                    route: observation.route,
                    frontend_status_counts: BTreeMap::new(),
                    execution_status_counts: BTreeMap::new(),
                    samples_by_frontend_status: BTreeMap::new(),
                });
            increment(
                &mut summary.frontend_status_counts,
                observation.frontend_status.clone(),
            );
            increment(
                &mut summary.execution_status_counts,
                observation.execution_status.clone(),
            );
            let samples = summary
                .samples_by_frontend_status
                .entry(observation.frontend_status.clone())
                .or_default();
            if samples.len() < SAMPLE_LIMIT {
                samples.push(Sample {
                    case_id: question.id.clone(),
                    prompt_sha256: digest_bytes(question.original_prompt.as_bytes()),
                    prompt_preview: preview(&question.original_prompt),
                    frontend_status: observation.frontend_status,
                    execution_status: observation.execution_status,
                });
            }
        }
    }

    let manifest_before = breadth_first_manifest().replay_hash();
    let manifest_after = breadth_first_manifest().replay_hash();
    let mut report = Report {
        schema: "goal6-external-portfolio-residual-samples-v1",
        release_id: "external-math-exam-v1",
        partition: "development",
        dataset_sha256,
        questions_read: questions.len(),
        answer_keys_read: 0,
        sealed_questions_read: 0,
        route_invocations: questions.len() * observe_all("", "route-count").len(),
        sample_limit: SAMPLE_LIMIT,
        route_summaries: summaries.into_values().collect(),
        manifest_sha256_before: manifest_before.clone(),
        manifest_sha256_after: manifest_after.clone(),
        manifest_unchanged: manifest_before == manifest_after,
        production_mutations: 0,
        report_sha256: String::new(),
    };
    let mut unsigned = serde_json::to_value(&report)?;
    unsigned["report_sha256"] = serde_json::Value::String(String::new());
    report.report_sha256 = digest(&unsigned);
    assert_eq!(report.answer_keys_read, 0);
    assert_eq!(report.sealed_questions_read, 0);
    assert_eq!(report.production_mutations, 0);
    assert!(report.manifest_unchanged);
    assert_eq!(report.questions_read, 3000);

    fs::write(REPORT_JSON, format!("{}\n", serde_json::to_string_pretty(&report)?))?;
    let markdown = report
        .route_summaries
        .iter()
        .map(|summary| {
            format!(
                "- **{:?}** frontend {:?}; execution {:?}; sampled statuses {:?}",
                summary.route,
                summary.frontend_status_counts,
                summary.execution_status_counts,
                summary
                    .samples_by_frontend_status
                    .iter()
                    .map(|(status, entries)| format!("{status}:{}", entries.len()))
                    .collect::<Vec<_>>()
                    .join(", ")
            )
        })
        .collect::<Vec<_>>()
        .join("\n");
    fs::write(
        REPORT_MD,
        format!(
            "# Goal 6 — answer-key-blind residual samples\n\n\
             - Development questions / route invocations: {} / {}\n\
             - Answer keys / sealed questions read: {} / {}\n\
             - Sample limit per route/status: {}\n\
             - Production mutations: {}\n\
             - Manifest unchanged: {}\n\n{}\n\n\
             Samples are prompt previews bound to hashes for diagnosis only;\
             they do not authorize routes or expose answer keys.\n",
            report.questions_read,
            report.route_invocations,
            report.answer_keys_read,
            report.sealed_questions_read,
            report.sample_limit,
            report.production_mutations,
            report.manifest_unchanged,
            markdown,
        ),
    )?;
    println!("{}", serde_json::to_string_pretty(&report)?);
    Ok(())
}
