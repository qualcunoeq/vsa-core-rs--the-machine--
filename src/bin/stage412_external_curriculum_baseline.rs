//! Stage 412: answer-key-blind baseline over the naturally authored corpus.
//!
//! This runner consumes only the Stage 381 development artifact and evaluates
//! every existing shadow route without lexical pre-dispatch or answer-key
//! access.  It records source lineage, route reachability, first obstruction,
//! replay/tamper status, and sealed-manifest integrity.  It never authorizes a
//! production answer and never reads sealed prompts or oracle material.

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::env;
use std::fs;
use the_machine::curriculum::breadth_first_manifest;
use the_machine::goal6_external_portfolio::{executable_routes, observe_all, PortfolioRoute};

const CORPUS_PATH: &str = "docs/stage381_external_curriculum_dev.json";
const SEALED_MANIFEST_PATH: &str =
    "docs/holdouts/stage381_external_curriculum_sealed_manifest.json";
const CORPUS_SHA256: &str = "25908c284af53959dd813bd9df51aec3761b7f637437da1e51fc348efa60fa7e";
const SOURCE_MANIFEST_SHA256: &str =
    "90d42a6945eea636b041539138cd98b3e475557f7d0d4733df98f611ea01c346";
const SEALED_MANIFEST_SHA256: &str =
    "f5db14844a7c7b83c038652c328599c9d640466fac84966bfc7a22faaf9a3e04";

#[derive(Debug, Deserialize)]
struct CorpusRecord {
    record_id: String,
    source_path: String,
    source_family: String,
    source_sha256: String,
    source_line: usize,
    candidate_kind: String,
    prompt: String,
    prompt_sha256: String,
    split: String,
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
    source_line: usize,
    candidate_kind: String,
    prompt_sha256: String,
    first_failure: String,
    executable_routes: Vec<String>,
}

#[derive(Debug, Serialize)]
struct Report {
    schema: &'static str,
    partition: String,
    records: usize,
    corpus_path: &'static str,
    corpus_file_sha256: String,
    declared_corpus_sha256: &'static str,
    source_manifest_sha256: &'static str,
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
    source_family_counts: BTreeMap<String, usize>,
    source_path_counts: BTreeMap<String, usize>,
    source_sha256_counts: BTreeMap<String, usize>,
    replay_verified: usize,
    replay_failures: usize,
    tamper_rejections: usize,
    tamper_failures: usize,
    manifest_sha256_before: String,
    manifest_sha256_after: String,
    manifest_unchanged: bool,
    residual_samples: Vec<ResidualSample>,
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

fn is_complete(status: &str) -> bool {
    status.eq_ignore_ascii_case("complete")
}

fn first_failure(
    observations: &[the_machine::goal6_external_portfolio::RouteObservation],
    executable: &[&the_machine::goal6_external_portfolio::RouteObservation],
) -> String {
    if executable.len() == 1 {
        return "unique_executable_route".into();
    }
    if executable.len() > 1 {
        return "ambiguous_executable_routes".into();
    }
    if observations.iter().all(|observation| {
        !is_complete(&observation.frontend_status) && observation.execution_status == "not_run"
    }) {
        return "no_complete_frontend".into();
    }
    if observations
        .iter()
        .any(|observation| is_complete(&observation.frontend_status))
        && observations
            .iter()
            .all(|observation| !is_complete(&observation.execution_status))
    {
        return "no_complete_execution".into();
    }
    if observations.iter().any(|observation| {
        is_complete(&observation.frontend_status)
            && is_complete(&observation.execution_status)
            && (!observation.frontend_replay_verified || !observation.execution_replay_verified)
    }) {
        return "replay_failure".into();
    }
    "no_unique_executable_route".into()
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let partition = env::var("GOAL6_STAGE412_PARTITION").unwrap_or_else(|_| "development".into());
    assert!(matches!(partition.as_str(), "development" | "validation"));
    let report_json = env::var("GOAL6_STAGE412_REPORT_JSON")
        .unwrap_or_else(|_| format!("docs/stage412_external_curriculum_baseline_{partition}.json"));
    let report_md = env::var("GOAL6_STAGE412_REPORT_MD")
        .unwrap_or_else(|_| format!("docs/stage412_external_curriculum_baseline_{partition}.md"));

    let corpus_bytes = fs::read(CORPUS_PATH)?;
    let corpus_file_sha256 = digest_bytes(&corpus_bytes);
    let records: Vec<CorpusRecord> = serde_json::from_slice(&corpus_bytes)?;
    let records: Vec<CorpusRecord> = records
        .into_iter()
        .filter(|record| record.split == partition)
        .collect();
    assert!(!records.is_empty(), "selected corpus partition is empty");
    assert!(records
        .iter()
        .all(|record| record.answer_key_status == "excluded_not_read"));

    let sealed_bytes = fs::read(SEALED_MANIFEST_PATH)?;
    let sealed_manifest_sha256 = digest_bytes(&sealed_bytes);
    let sealed_manifest_value: serde_json::Value = serde_json::from_slice(&sealed_bytes)?;
    let sealed_manifest_records = sealed_manifest_value
        .as_array()
        .map(|items| items.len())
        .or_else(|| {
            sealed_manifest_value
                .get("records")
                .and_then(|v| v.as_array())
                .map(|v| v.len())
        })
        .unwrap_or(0);
    assert_eq!(sealed_manifest_records, 260);

    let manifest_sha256_before = breadth_first_manifest().replay_hash();
    let mut route_summaries = BTreeMap::<String, RouteSummary>::new();
    let mut unique_route_records = 0;
    let mut ambiguous_route_records = 0;
    let mut no_route_records = 0;
    let mut first_failure_counts = BTreeMap::<String, usize>::new();
    let mut source_family_counts = BTreeMap::<String, usize>::new();
    let mut source_path_counts = BTreeMap::<String, usize>::new();
    let mut source_sha256_counts = BTreeMap::<String, usize>::new();
    let mut replay_verified = 0;
    let mut replay_failures = 0;
    let mut tamper_rejections = 0;
    let mut tamper_failures = 0;
    let mut residual_samples = Vec::new();

    for record in &records {
        *source_family_counts
            .entry(record.source_family.clone())
            .or_default() += 1;
        *source_path_counts
            .entry(record.source_path.clone())
            .or_default() += 1;
        *source_sha256_counts
            .entry(record.source_sha256.clone())
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
        if residual_samples.len() < 100 && failure != "unique_executable_route" {
            residual_samples.push(ResidualSample {
                record_id: record.record_id.clone(),
                source_path: record.source_path.clone(),
                source_family: record.source_family.clone(),
                source_line: record.source_line,
                candidate_kind: record.candidate_kind.clone(),
                prompt_sha256: record.prompt_sha256.clone(),
                first_failure: failure,
                executable_routes: executable
                    .iter()
                    .map(|observation| route_name(&observation.route))
                    .collect(),
            });
        }
        let executable_routes_set = executable
            .iter()
            .map(|observation| observation.route)
            .collect::<BTreeSet<_>>();
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
            summary.frontend_complete += usize::from(is_complete(&observation.frontend_status));
            summary.execution_complete += usize::from(is_complete(&observation.execution_status));
            summary.executable += usize::from(executable_routes_set.contains(&observation.route));
            summary.frontend_replay_verified += usize::from(observation.frontend_replay_verified);
            summary.execution_replay_verified += usize::from(observation.execution_replay_verified);
            summary.frontend_tamper_rejected += usize::from(observation.frontend_tamper_rejected);
            summary.execution_tamper_rejected += usize::from(observation.execution_tamper_rejected);
            let execution_emitted = observation.execution_status != "not_run";
            if observation.frontend_replay_verified
                && (!execution_emitted || observation.execution_replay_verified)
            {
                replay_verified += 1;
            } else {
                replay_failures += 1;
            }
            if observation.frontend_tamper_rejected
                && (!execution_emitted || observation.execution_tamper_rejected)
            {
                tamper_rejections += 1;
            } else {
                tamper_failures += 1;
            }
        }
    }
    let manifest_sha256_after = breadth_first_manifest().replay_hash();
    let manifest_unchanged = manifest_sha256_before == manifest_sha256_after;
    assert!(manifest_unchanged);
    assert_eq!(replay_failures, 0);
    assert_eq!(tamper_failures, 0);

    let mut report = Report {
        schema: "stage412-external-curriculum-baseline-v1",
        partition,
        records: records.len(),
        corpus_path: CORPUS_PATH,
        corpus_file_sha256,
        declared_corpus_sha256: CORPUS_SHA256,
        source_manifest_sha256: SOURCE_MANIFEST_SHA256,
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
        source_family_counts,
        source_path_counts,
        source_sha256_counts,
        replay_verified,
        replay_failures,
        tamper_rejections,
        tamper_failures,
        manifest_sha256_before,
        manifest_sha256_after,
        manifest_unchanged,
        residual_samples,
        report_sha256: String::new(),
    };
    report.report_sha256 = digest(&report);
    fs::write(&report_json, serde_json::to_vec_pretty(&report)?)?;
    let mut markdown = format!(
        "# Stage 412 — answer-key-blind external curriculum baseline\n\n\
- partition / records: {} / {}\n\
- route count: {}\n\
- unique / ambiguous / no route: {} / {} / {}\n\
- replay verified / failed: {} / {}\n\
- tamper rejected / failed: {} / {}\n\
- answer keys read / production authorizations / false authorizations: {} / {} / {}\n\
- sealed manifest records: {} (prompt text not consumed)\n\
- manifest unchanged: {}\n\
- corpus file SHA-256: `{}`\n\
- declared Stage 381 corpus SHA-256: `{}`\n\n",
        report.partition,
        report.records,
        report.route_count,
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
        report.manifest_unchanged,
        report.corpus_file_sha256,
        report.declared_corpus_sha256,
    );
    markdown.push_str("## First-failure counts\n\n");
    for (failure, count) in &report.first_failure_counts {
        markdown.push_str(&format!("- {failure}: {count}\n"));
    }
    markdown.push_str("\n## Route summaries\n\n");
    for (route, summary) in &report.route_summaries {
        markdown.push_str(&format!(
            "- `{route}`: frontend {}/{}, execution {}/{}, executable {}, replay {}/{}, tamper {}/{}\n",
            summary.frontend_complete,
            summary.observations,
            summary.execution_complete,
            summary.observations,
            summary.executable,
            summary.frontend_replay_verified + summary.execution_replay_verified,
            summary.observations * 2,
            summary.frontend_tamper_rejected + summary.execution_tamper_rejected,
            summary.observations * 2,
        ));
    }
    markdown.push_str("\nThis is a reachability baseline only. No answer alignment was read and no route was promoted.\n");
    fs::write(report_md, markdown)?;
    println!("wrote {report_json}");
    Ok(())
}
