//! Stage 413: route-blind reachability over page-aware external candidates.
//!
//! Stage 412 showed that the line-level Stage 381 artifact had no overlap with
//! the existing portfolio.  This checkpoint consumes only the independently
//! assembled, quality-clean page-aware candidates from Stage 389.  It still
//! does not read answers or sealed prompts, and it never authorizes routing.

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::env;
use std::fs;
use the_machine::curriculum::breadth_first_manifest;
use the_machine::goal6_external_portfolio::{executable_routes, observe_all, PortfolioRoute};

const CORPUS_PATH: &str = "docs/stage389_page_aware_external_problem_dev.json";
const SEALED_MANIFEST_PATH: &str =
    "docs/holdouts/stage389_page_aware_external_problem_sealed_manifest.json";
const CORPUS_SHA256: &str = "92304d95521ac2ef4d49a08272f0b0cf79ac7225c9715d5ace9a667c822f6e03";
const SEALED_MANIFEST_SHA256: &str =
    "ac4227bff20decad585210b8e0d4c60401f6056092b718e99157e4f208a1b8de";

#[derive(Debug, Deserialize)]
struct PageRecord {
    record_id: String,
    source_path: String,
    source_family: String,
    source_sha256: String,
    source_line: usize,
    #[serde(default)]
    source_page: Option<usize>,
    prompt: String,
    prompt_sha256: String,
    split: String,
    #[serde(default)]
    quality_flags: Vec<String>,
    answer_key_status: String,
}

#[derive(Debug, Serialize)]
struct RouteSummary {
    observations: usize,
    frontend_complete: usize,
    execution_complete: usize,
    executable: usize,
    frontend_replay_verified: usize,
    execution_replay_verified: usize,
    frontend_tamper_rejected: usize,
    execution_tamper_rejected: usize,
}

#[derive(Debug, Serialize)]
struct ResidualSample {
    record_id: String,
    source_path: String,
    source_family: String,
    source_sha256: String,
    source_line: usize,
    source_page: Option<usize>,
    prompt_sha256: String,
    first_failure: String,
    executable_routes: Vec<String>,
}

#[derive(Debug, Serialize)]
struct Report {
    schema: &'static str,
    partition: String,
    source_records: usize,
    quality_clean_records: usize,
    quality_rejected_records: usize,
    corpus_path: &'static str,
    corpus_file_sha256: String,
    declared_corpus_sha256: &'static str,
    sealed_manifest_path: &'static str,
    sealed_manifest_sha256: String,
    declared_sealed_manifest_sha256: &'static str,
    sealed_manifest_records: usize,
    answer_keys_read: usize,
    plaintext_answers_read: usize,
    production_authorizations: usize,
    false_authorizations: usize,
    route_count: usize,
    route_summaries: BTreeMap<String, RouteSummary>,
    unique_route_records: usize,
    ambiguous_route_records: usize,
    no_route_records: usize,
    first_failure_counts: BTreeMap<String, usize>,
    source_path_counts: BTreeMap<String, usize>,
    replay_verified: usize,
    replay_failures: usize,
    tamper_rejections: usize,
    tamper_failures: usize,
    manifest_sha256_before: String,
    manifest_sha256_after: String,
    manifest_unchanged: bool,
    residual_samples: Vec<ResidualSample>,
    unique_samples: Vec<ResidualSample>,
    report_sha256: String,
}

fn digest_bytes(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

fn digest<T: Serialize>(value: &T) -> String {
    digest_bytes(&serde_json::to_vec(value).expect("serializable report"))
}

fn route_name(route: &PortfolioRoute) -> String {
    serde_json::to_string(route)
        .expect("route serializes")
        .trim_matches('"')
        .to_owned()
}

fn complete(status: &str) -> bool {
    status.eq_ignore_ascii_case("complete")
}

fn first_failure(
    observations: &[the_machine::goal6_external_portfolio::RouteObservation],
    executable: &[&the_machine::goal6_external_portfolio::RouteObservation],
) -> String {
    match executable.len() {
        1 => return "unique_executable_route".into(),
        n if n > 1 => return "ambiguous_executable_routes".into(),
        _ => {}
    }
    if observations
        .iter()
        .all(|observation| !complete(&observation.frontend_status))
    {
        "no_complete_frontend".into()
    } else if observations.iter().any(|observation| {
        complete(&observation.frontend_status)
            && observation.execution_status != "not_run"
            && !complete(&observation.execution_status)
    }) {
        "execution_incomplete".into()
    } else if observations.iter().any(|observation| {
        complete(&observation.frontend_status)
            && complete(&observation.execution_status)
            && (!observation.frontend_replay_verified || !observation.execution_replay_verified)
    }) {
        "replay_failure".into()
    } else {
        "no_unique_executable_route".into()
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let partition = env::var("GOAL6_STAGE413_PARTITION").unwrap_or_else(|_| "development".into());
    assert!(matches!(partition.as_str(), "development" | "validation"));
    let report_json = env::var("GOAL6_STAGE413_REPORT_JSON").unwrap_or_else(|_| {
        format!("docs/stage413_page_aware_external_reachability_{partition}.json")
    });
    let report_md = env::var("GOAL6_STAGE413_REPORT_MD").unwrap_or_else(|_| {
        format!("docs/stage413_page_aware_external_reachability_{partition}.md")
    });

    let corpus_bytes = fs::read(CORPUS_PATH)?;
    let corpus_file_sha256 = digest_bytes(&corpus_bytes);
    let all_records: Vec<PageRecord> = serde_json::from_slice(&corpus_bytes)?;
    let source_records = all_records
        .iter()
        .filter(|record| record.split == partition)
        .count();
    let rejected_records = all_records
        .iter()
        .filter(|record| record.split == partition && !record.quality_flags.is_empty())
        .count();
    let records: Vec<PageRecord> = all_records
        .into_iter()
        .filter(|record| record.split == partition && record.quality_flags.is_empty())
        .collect();
    assert!(!records.is_empty());
    assert!(records
        .iter()
        .all(|record| record.answer_key_status == "not_read"));

    let sealed_bytes = fs::read(SEALED_MANIFEST_PATH)?;
    let sealed_manifest_sha256 = digest_bytes(&sealed_bytes);
    let sealed_value: serde_json::Value = serde_json::from_slice(&sealed_bytes)?;
    let sealed_manifest_records = sealed_value
        .as_array()
        .map(Vec::len)
        .or_else(|| {
            sealed_value
                .get("records")
                .and_then(|v| v.as_array())
                .map(Vec::len)
        })
        .unwrap_or(0);
    assert_eq!(sealed_manifest_records, 1091);

    let manifest_sha256_before = breadth_first_manifest().replay_hash();
    let mut route_summaries = BTreeMap::<String, RouteSummary>::new();
    let mut unique_route_records = 0;
    let mut ambiguous_route_records = 0;
    let mut no_route_records = 0;
    let mut first_failure_counts = BTreeMap::<String, usize>::new();
    let mut source_path_counts = BTreeMap::<String, usize>::new();
    let mut replay_verified = 0;
    let mut replay_failures = 0;
    let mut tamper_rejections = 0;
    let mut tamper_failures = 0;
    let mut residual_samples = Vec::new();
    let mut unique_samples = Vec::new();

    for record in &records {
        *source_path_counts
            .entry(record.source_path.clone())
            .or_default() += 1;
        let observations = observe_all(&record.prompt, &record.record_id);
        let executable = executable_routes(&observations);
        match executable.len() {
            0 => no_route_records += 1,
            1 => unique_route_records += 1,
            _ => ambiguous_route_records += 1,
        }
        let failure = first_failure(&observations, &executable);
        *first_failure_counts.entry(failure.clone()).or_default() += 1;
        let sample = ResidualSample {
            record_id: record.record_id.clone(),
            source_path: record.source_path.clone(),
            source_family: record.source_family.clone(),
            source_sha256: record.source_sha256.clone(),
            source_line: record.source_line,
            source_page: record.source_page,
            prompt_sha256: record.prompt_sha256.clone(),
            first_failure: failure,
            executable_routes: executable
                .iter()
                .map(|observation| route_name(&observation.route))
                .collect(),
        };
        if sample.first_failure == "unique_executable_route" {
            if unique_samples.len() < 100 {
                unique_samples.push(sample);
            }
        } else if residual_samples.len() < 100 {
            residual_samples.push(sample);
        }
        let executable_set = executable
            .iter()
            .map(|observation| observation.route)
            .collect::<std::collections::BTreeSet<_>>();
        for observation in &observations {
            let name = route_name(&observation.route);
            let summary = route_summaries.entry(name).or_insert(RouteSummary {
                observations: 0,
                frontend_complete: 0,
                execution_complete: 0,
                executable: 0,
                frontend_replay_verified: 0,
                execution_replay_verified: 0,
                frontend_tamper_rejected: 0,
                execution_tamper_rejected: 0,
            });
            summary.observations += 1;
            summary.frontend_complete += usize::from(complete(&observation.frontend_status));
            summary.execution_complete += usize::from(complete(&observation.execution_status));
            summary.executable += usize::from(executable_set.contains(&observation.route));
            summary.frontend_replay_verified += usize::from(observation.frontend_replay_verified);
            summary.execution_replay_verified += usize::from(observation.execution_replay_verified);
            summary.frontend_tamper_rejected += usize::from(observation.frontend_tamper_rejected);
            summary.execution_tamper_rejected += usize::from(observation.execution_tamper_rejected);
            let emitted = observation.execution_status != "not_run";
            if observation.frontend_replay_verified
                && (!emitted || observation.execution_replay_verified)
            {
                replay_verified += 1;
            } else {
                replay_failures += 1;
            }
            if observation.frontend_tamper_rejected
                && (!emitted || observation.execution_tamper_rejected)
            {
                tamper_rejections += 1;
            } else {
                tamper_failures += 1;
            }
        }
    }
    let manifest_sha256_after = breadth_first_manifest().replay_hash();
    assert_eq!(replay_failures, 0);
    assert_eq!(tamper_failures, 0);
    assert_eq!(manifest_sha256_before, manifest_sha256_after);

    let mut report = Report {
        schema: "stage413-page-aware-external-reachability-v1",
        partition,
        source_records,
        quality_clean_records: records.len(),
        quality_rejected_records: rejected_records,
        corpus_path: CORPUS_PATH,
        corpus_file_sha256,
        declared_corpus_sha256: CORPUS_SHA256,
        sealed_manifest_path: SEALED_MANIFEST_PATH,
        sealed_manifest_sha256,
        declared_sealed_manifest_sha256: SEALED_MANIFEST_SHA256,
        sealed_manifest_records,
        answer_keys_read: 0,
        plaintext_answers_read: 0,
        production_authorizations: 0,
        false_authorizations: 0,
        route_count: route_summaries.len(),
        route_summaries,
        unique_route_records,
        ambiguous_route_records,
        no_route_records,
        first_failure_counts,
        source_path_counts,
        replay_verified,
        replay_failures,
        tamper_rejections,
        tamper_failures,
        manifest_sha256_before,
        manifest_sha256_after,
        manifest_unchanged: true,
        residual_samples,
        unique_samples,
        report_sha256: String::new(),
    };
    report.report_sha256 = digest(&report);
    fs::write(&report_json, serde_json::to_vec_pretty(&report)?)?;
    let mut markdown = format!(
        "# Stage 413 — page-aware external reachability\n\n\
- partition / source records / clean candidates / rejected: {} / {} / {} / {}\n\
- unique / ambiguous / no route: {} / {} / {}\n\
- replay verified / failed: {} / {}\n\
- tamper rejected / failed: {} / {}\n\
- answer keys read / production authorizations / false authorizations: {} / {} / {}\n\
- sealed manifest records: {} (prompt text not consumed)\n\
- manifest unchanged: true\n\
- corpus file SHA-256: `{}`\n\n",
        report.partition,
        report.source_records,
        report.quality_clean_records,
        report.quality_rejected_records,
        report.unique_route_records,
        report.ambiguous_route_records,
        report.no_route_records,
        report.replay_verified,
        report.replay_failures,
        report.tamper_rejections,
        report.tamper_failures,
        report.answer_keys_read,
        report.production_authorizations,
        report.false_authorizations,
        report.sealed_manifest_records,
        report.corpus_file_sha256,
    );
    markdown.push_str("## First-failure counts\n\n");
    for (failure, count) in &report.first_failure_counts {
        markdown.push_str(&format!("- {failure}: {count}\n"));
    }
    markdown.push_str("\nThis is a reachability baseline only. No answer alignment was read and no route was promoted.\n");
    fs::write(report_md, markdown)?;
    println!("wrote {report_json}");
    Ok(())
}
