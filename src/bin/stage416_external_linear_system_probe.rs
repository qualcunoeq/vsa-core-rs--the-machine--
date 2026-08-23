//! Stage 416: probe existing formalization against external linear-system
//! word problems.  This is an answer-key-blind bridge diagnostic; it does not
//! synthesize or promote a new capability.

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::fs;
use the_machine::curriculum::breadth_first_manifest;
use the_machine::formalization::{assess_prompt, FormalizedFact};
use the_machine::linear_system::{execute_linear_system, replay_linear_system};

const CORPUS_PATH: &str = "docs/stage389_page_aware_external_problem_dev.json";
const SEALED_MANIFEST_PATH: &str =
    "docs/holdouts/stage389_page_aware_external_problem_sealed_manifest.json";
const CORPUS_SHA256: &str = "92304d95521ac2ef4d49a08272f0b0cf79ac7225c9715d5ace9a667c822f6e03";
const SEALED_MANIFEST_SHA256: &str =
    "ac4227bff20decad585210b8e0d4c60401f6056092b718e99157e4f208a1b8de";

#[derive(Debug, Deserialize)]
struct Record {
    record_id: String,
    source_path: String,
    source_sha256: String,
    prompt: String,
    prompt_sha256: String,
    split: String,
    #[serde(default)]
    quality_flags: Vec<String>,
    answer_key_status: String,
}

#[derive(Debug, Serialize)]
struct Sample {
    record_id: String,
    source_path: String,
    source_sha256: String,
    prompt_sha256: String,
    split: String,
    equation_fact_count: usize,
    formalization_complete: bool,
    equation_source_hash: Option<String>,
    failure: String,
}

#[derive(Debug, Serialize)]
struct PartitionReport {
    records: usize,
    formalization_complete: usize,
    equation_fact_complete: usize,
    unique_solver_complete: usize,
    execution_replay_verified: usize,
    execution_tamper_rejected: usize,
    failure_counts: BTreeMap<String, usize>,
    samples: Vec<Sample>,
}

#[derive(Debug, Serialize)]
struct Report {
    schema: &'static str,
    corpus_path: &'static str,
    corpus_file_sha256: String,
    declared_corpus_sha256: &'static str,
    source_records: usize,
    quality_clean_records: usize,
    quality_rejected_records: usize,
    candidate_records: usize,
    answer_keys_read: usize,
    plaintext_answers_read: usize,
    production_authorizations: usize,
    false_authorizations: usize,
    sealed_manifest_path: &'static str,
    sealed_manifest_sha256: String,
    declared_sealed_manifest_sha256: &'static str,
    sealed_manifest_records: usize,
    development: PartitionReport,
    validation: PartitionReport,
    manifest_sha256_before: String,
    manifest_sha256_after: String,
    manifest_unchanged: bool,
    report_sha256: String,
}

fn digest_bytes(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

fn digest<T: Serialize>(value: &T) -> String {
    digest_bytes(&serde_json::to_vec(value).expect("probe serializes"))
}

fn is_candidate(text: &str) -> bool {
    let lower = text.to_ascii_lowercase();
    (lower.contains("system of equations") || lower.contains("simultaneous equations"))
        && (lower.contains("solve") || lower.contains("solution"))
}

fn equation_source(facts: &[FormalizedFact]) -> Option<String> {
    let equations = facts
        .iter()
        .filter_map(|fact| match fact {
            FormalizedFact::Equation {
                lhs, relation, rhs, ..
            } if relation == "=" => Some(format!("{lhs} = {rhs}")),
            _ => None,
        })
        .collect::<Vec<_>>();
    (equations.len() >= 2).then(|| format!("Solve {}", equations.join(" and ")))
}

fn run_partition(records: &[Record], split: &str) -> PartitionReport {
    let candidates = records
        .iter()
        .filter(|record| {
            record.split == split && record.quality_flags.is_empty() && is_candidate(&record.prompt)
        })
        .collect::<Vec<_>>();
    let mut report = PartitionReport {
        records: candidates.len(),
        formalization_complete: 0,
        equation_fact_complete: 0,
        unique_solver_complete: 0,
        execution_replay_verified: 0,
        execution_tamper_rejected: 0,
        failure_counts: BTreeMap::new(),
        samples: Vec::new(),
    };
    for record in candidates {
        let trace = assess_prompt(&record.record_id, &record.prompt, "Math", false);
        let formalization_complete = trace.target_completion.complete;
        report.formalization_complete += usize::from(formalization_complete);
        let equations = trace
            .formalized_facts
            .iter()
            .filter(
                |fact| matches!(fact, FormalizedFact::Equation { relation, .. } if relation == "="),
            )
            .count();
        report.equation_fact_complete += usize::from(equations >= 2);
        let source = equation_source(&trace.formalized_facts);
        let mut failure = if !formalization_complete {
            "formalization_incomplete"
        } else if equations < 2 {
            "fewer_than_two_equation_facts"
        } else {
            "solver_unsupported"
        };
        let mut source_hash = None;
        if let Some(source) = source {
            source_hash = Some(digest_bytes(source.as_bytes()));
            if let Ok(receipt) = execute_linear_system(&source) {
                report.unique_solver_complete += 1;
                let replay = replay_linear_system(&receipt);
                let mut tampered = receipt.clone();
                tampered.result.push('x');
                let tamper_rejected = !replay_linear_system(&tampered);
                report.execution_replay_verified += usize::from(replay);
                report.execution_tamper_rejected += usize::from(tamper_rejected);
                if replay && tamper_rejected {
                    failure = "complete_replayable_linear_system";
                } else {
                    failure = "execution_replay_or_tamper_failure";
                }
            }
        }
        *report.failure_counts.entry(failure.into()).or_default() += 1;
        if report.samples.len() < 100 {
            report.samples.push(Sample {
                record_id: record.record_id.clone(),
                source_path: record.source_path.clone(),
                source_sha256: record.source_sha256.clone(),
                prompt_sha256: record.prompt_sha256.clone(),
                split: record.split.clone(),
                equation_fact_count: equations,
                formalization_complete,
                equation_source_hash: source_hash,
                failure: failure.into(),
            });
        }
    }
    report
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let corpus_bytes = fs::read(CORPUS_PATH)?;
    let corpus_file_sha256 = digest_bytes(&corpus_bytes);
    let all_records: Vec<Record> = serde_json::from_slice(&corpus_bytes)?;
    let source_records = all_records.len();
    let quality_rejected_records = all_records
        .iter()
        .filter(|record| !record.quality_flags.is_empty())
        .count();
    let clean_records = all_records
        .iter()
        .filter(|record| record.quality_flags.is_empty())
        .count();
    assert!(all_records
        .iter()
        .all(|record| record.answer_key_status == "not_read"));
    let candidate_records = all_records
        .iter()
        .filter(|record| record.quality_flags.is_empty() && is_candidate(&record.prompt))
        .count();
    let sealed_bytes = fs::read(SEALED_MANIFEST_PATH)?;
    let sealed_manifest_sha256 = digest_bytes(&sealed_bytes);
    let sealed_value: serde_json::Value = serde_json::from_slice(&sealed_bytes)?;
    let sealed_manifest_records = sealed_value
        .as_array()
        .map(Vec::len)
        .or_else(|| {
            sealed_value
                .get("records")
                .and_then(|value| value.as_array())
                .map(Vec::len)
        })
        .unwrap_or(0);
    assert_eq!(sealed_manifest_records, 1091);

    let manifest_sha256_before = breadth_first_manifest().replay_hash();
    let development = run_partition(&all_records, "development");
    let validation = run_partition(&all_records, "validation");
    let manifest_sha256_after = breadth_first_manifest().replay_hash();
    assert_eq!(manifest_sha256_before, manifest_sha256_after);
    let mut report = Report {
        schema: "stage416-external-linear-system-probe-v1",
        corpus_path: CORPUS_PATH,
        corpus_file_sha256,
        declared_corpus_sha256: CORPUS_SHA256,
        source_records,
        quality_clean_records: clean_records,
        quality_rejected_records,
        candidate_records,
        answer_keys_read: 0,
        plaintext_answers_read: 0,
        production_authorizations: 0,
        false_authorizations: 0,
        sealed_manifest_path: SEALED_MANIFEST_PATH,
        sealed_manifest_sha256,
        declared_sealed_manifest_sha256: SEALED_MANIFEST_SHA256,
        sealed_manifest_records,
        development,
        validation,
        manifest_sha256_before,
        manifest_sha256_after,
        manifest_unchanged: true,
        report_sha256: String::new(),
    };
    report.report_sha256 = digest(&report);
    fs::write(
        "docs/stage416_external_linear_system_probe.json",
        serde_json::to_vec_pretty(&report)?,
    )?;
    fs::write(
        "docs/stage416_external_linear_system_probe.md",
        format!(
            "# Stage 416 — external linear-system probe\n\n- candidate records / development / validation: {} / {} / {}\n- development formalization / two-equation / unique execution: {} / {} / {}\n- validation formalization / two-equation / unique execution: {} / {} / {}\n- replay / tamper: {}/{} / {}/{}\n- answer keys read / production authorizations / false authorizations: 0 / 0 / 0\n- sealed manifest records: {} (prompt text not consumed)\n- manifest unchanged: true\n\nThis is a bridge diagnostic only; no source-derived capability was promoted.\n",
            report.candidate_records,
            report.development.records,
            report.validation.records,
            report.development.formalization_complete,
            report.development.equation_fact_complete,
            report.development.unique_solver_complete,
            report.validation.formalization_complete,
            report.validation.equation_fact_complete,
            report.validation.unique_solver_complete,
            report.development.execution_replay_verified,
            report.validation.execution_replay_verified,
            report.development.execution_tamper_rejected,
            report.validation.execution_tamper_rejected,
            report.sealed_manifest_records,
        ),
    )?;
    println!(
        "Stage 416 — candidates={} dev_exec={} val_exec={}",
        report.candidate_records,
        report.development.unique_solver_complete,
        report.validation.unique_solver_complete
    );
    Ok(())
}
