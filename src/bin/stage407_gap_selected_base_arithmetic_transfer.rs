//! Stage 407: typed-gap-selected positional arithmetic transfer.
//!
//! This runner evaluates a source-derived finite positional arithmetic route
//! beside the existing route-blind portfolio. Development never opens answer
//! oracles; sealed evaluation is an explicit hash-only privileged boundary.
//! The route is shadow-only and cannot mutate production state.

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::env;
use std::fs;
use the_machine::curriculum::breadth_first_manifest;
use the_machine::goal6_external_portfolio::{executable_routes, observe_all};
use the_machine::source_base_arithmetic_frontend::{
    formalize, replay_verified as frontend_replay, FrontendStatus,
};
use the_machine::source_base_arithmetic_pack::{
    evaluate, replay_verified as execution_replay, BaseArithmeticStatus, SOURCE,
};
use the_machine::source_evidence_envelope::ingest_source_evidence;
use the_machine::source_selection::{
    gap_replay_verified, select_for_gap, SourceGapRequest, SourceSelectionDecision,
};

const QUESTIONS_PATH: &str = "data/external_math_exam_v1/questions.jsonl";
const RELEASE_DIR: &str = "data/external_math_exam_v1";
const SOURCE_PATH: &str = "docs/sources/openstax_base_arithmetic_source.txt";
const SCOPE: &str = "finite positional binary arithmetic";
const GAP_ID: &str = "gap::finite-positional-arithmetic";

#[derive(Debug, Deserialize)]
struct Question {
    id: String,
    original_prompt: String,
    split: String,
}

#[derive(Debug, Deserialize)]
struct Oracle {
    id: String,
    answer_sha256: String,
}

#[derive(Debug, Serialize)]
struct CandidateReceipt {
    id: String,
    candidate_hash: String,
    baseline_also_executable: bool,
    reference_match: Option<bool>,
    replay_verified: bool,
}

#[derive(Debug, Serialize)]
struct PartitionReport {
    questions: usize,
    baseline_executable: usize,
    baseline_ambiguities: usize,
    frontend_complete: usize,
    frontend_ambiguous: usize,
    frontend_missing: usize,
    frontend_unsupported: usize,
    execution_complete: usize,
    replay_verified: usize,
    tamper_rejected: usize,
    selected_unique_ids: usize,
    incremental_unique_ids: usize,
    answer_hashes_read: usize,
    plaintext_answers_read: usize,
    correct_selected_candidates: usize,
    candidates: Vec<CandidateReceipt>,
}

#[derive(Debug, Serialize)]
struct Report {
    schema: &'static str,
    partition: String,
    dataset_sha256: String,
    source_path: &'static str,
    source_sha256: String,
    source_id: String,
    source_records: usize,
    gap_id: &'static str,
    operation_scope: &'static str,
    candidate_sources: usize,
    selected_sources: usize,
    rejected_decoys: usize,
    gap_selection_replay: bool,
    manifest_sha256_before: String,
    manifest_sha256_after: String,
    manifest_unchanged: bool,
    answer_keys_read: usize,
    production_authorizations: usize,
    false_authorizations: usize,
    partition_report: PartitionReport,
    report_sha256: String,
}

fn digest_bytes(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

fn digest<T: Serialize>(value: &T) -> String {
    format!("{:x}", Sha256::digest(serde_json::to_vec(value).unwrap()))
}

fn source_id() -> String {
    SOURCE
        .lines()
        .find_map(|line| line.strip_prefix("SOURCE_ID: ").map(str::to_owned))
        .expect("source record has SOURCE_ID")
}

fn source_records() -> usize {
    SOURCE
        .lines()
        .filter(|line| line.starts_with("EVIDENCE: "))
        .count()
}

fn source_evidence(
    path: &str,
    id: &str,
    title: &str,
    section: &str,
    url: &str,
    evidence: &str,
    scope: &str,
) -> the_machine::source_evidence_envelope::SourceEvidenceEnvelope {
    let document = format!(
        "SOURCE_ID: {id}\nTITLE: {title}\nSECTION: {section}\nURL: {url}\nLICENSE: CC BY 4.0\nRETRIEVED_UTC: 2026-08-23\nEVIDENCE: {evidence}\nSCOPE: {scope}"
    );
    ingest_source_evidence(path, &document).expect("source metadata is valid")
}

fn oracle_for_partition(
    partition: &str,
) -> Result<BTreeMap<String, String>, Box<dyn std::error::Error>> {
    if partition == "development" {
        return Ok(BTreeMap::new());
    }
    assert_eq!(
        env::var("GOAL6_STAGE407_PRIVILEGED_EVAL").as_deref(),
        Ok("true"),
        "sealed scoring requires explicit hash-only privileged evaluation"
    );
    let path = format!("{RELEASE_DIR}/oracle_{partition}.jsonl");
    Ok(fs::read_to_string(path)?
        .lines()
        .filter(|line| !line.trim().is_empty())
        .map(serde_json::from_str::<Oracle>)
        .collect::<Result<Vec<_>, _>>()?
        .into_iter()
        .map(|record| (record.id, record.answer_sha256))
        .collect())
}

fn candidate_forms(numeral: &str, decimal: u128, base: u32) -> Vec<String> {
    vec![
        numeral.to_owned(),
        format!("{numeral}_{base}"),
        decimal.to_string(),
    ]
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let partition = env::var("GOAL6_STAGE407_PARTITION").unwrap_or_else(|_| "development".into());
    assert!(matches!(partition.as_str(), "development" | "sealed"));
    let report_path = env::var("GOAL6_STAGE407_REPORT_JSON")
        .unwrap_or_else(|_| format!("docs/stage407_gap_selected_base_arithmetic_{partition}.json"));
    let report_md_path = env::var("GOAL6_STAGE407_REPORT_MD")
        .unwrap_or_else(|_| format!("docs/stage407_gap_selected_base_arithmetic_{partition}.md"));
    let question_bytes = fs::read(QUESTIONS_PATH)?;
    let dataset_sha256 = digest_bytes(&question_bytes);
    let questions: Vec<Question> = String::from_utf8(question_bytes)?
        .lines()
        .filter(|line| !line.trim().is_empty())
        .map(serde_json::from_str)
        .collect::<Result<Vec<_>, _>>()?;
    let questions = questions
        .into_iter()
        .filter(|question| question.split == partition)
        .collect::<Vec<_>>();
    let arithmetic_source_id = source_id();
    let candidates = vec![
        source_evidence(
            SOURCE_PATH,
            &arithmetic_source_id,
            "Contemporary Mathematics",
            "Base arithmetic",
            "https://openstax.org/books/contemporary-mathematics/pages/4-4-addition-and-subtraction-in-base-systems",
            "finite binary addition, subtraction, and multiplication in an explicitly declared base",
            SCOPE,
        ),
        source_evidence(
            "docs/sources/openstax_precalculus_sequences_source.txt",
            "openstax-precalculus-2e:sequences",
            "Precalculus 2e",
            "Arithmetic Sequences",
            "https://openstax.org/details/books/precalculus-2e",
            "finite arithmetic-sequence term evaluation",
            "finite arithmetic-sequence term evaluation",
        ),
        source_evidence(
            "docs/sources/openstax_finite_statistics_source.txt",
            "openstax-introductory-statistics-2e:descriptive-statistics",
            "Introductory Statistics 2e",
            "Measures of Central Tendency",
            "https://openstax.org/details/books/introductory-statistics-2e",
            "finite-list mean evaluation",
            "finite-list mean evaluation",
        ),
    ];
    let gap_request = SourceGapRequest::new(GAP_ID, vec![SCOPE.into()], 1);
    let selection = select_for_gap(&gap_request, &candidates);
    assert!(gap_replay_verified(&gap_request, &selection));
    assert_eq!(
        selection.selected_source_ids,
        vec![arithmetic_source_id.clone()]
    );
    assert_eq!(
        selection
            .candidates
            .iter()
            .filter(|candidate| candidate.decision == SourceSelectionDecision::Rejected)
            .count(),
        2
    );
    let oracle = oracle_for_partition(&partition)?;
    let manifest_before = breadth_first_manifest().replay_hash();
    let mut baseline_ids = BTreeSet::new();
    let mut baseline_ambiguities = 0;
    let mut frontend_complete = 0;
    let mut frontend_ambiguous = 0;
    let mut frontend_missing = 0;
    let mut frontend_unsupported = 0;
    let mut execution_complete = 0;
    let mut replay_verified_count = 0;
    let mut tamper_rejected = 0;
    let mut correct_selected_candidates = 0;
    let mut selected_candidates = Vec::new();
    for question in &questions {
        let observations = observe_all(&question.original_prompt, &question.id);
        let baseline = executable_routes(&observations);
        if baseline.len() > 1 {
            baseline_ambiguities += 1;
        }
        if baseline.len() == 1 {
            baseline_ids.insert(question.id.clone());
        }
        let frontend = formalize(&question.original_prompt, &question.id);
        match frontend.status {
            FrontendStatus::Complete => frontend_complete += 1,
            FrontendStatus::Ambiguous => frontend_ambiguous += 1,
            FrontendStatus::Missing => frontend_missing += 1,
            FrontendStatus::Unsupported => frontend_unsupported += 1,
        }
        let frontend_ok = frontend_replay(&frontend);
        let mut frontend_tampered = frontend.clone();
        frontend_tampered.replay_hash.push('x');
        assert!(!frontend_replay(&frontend_tampered));
        let Some(request) = frontend.request.as_ref() else {
            continue;
        };
        let execution = evaluate(request);
        if execution.status != BaseArithmeticStatus::Complete {
            continue;
        }
        execution_complete += 1;
        let execution_ok = execution_replay(&execution);
        let mut execution_tampered = execution.clone();
        execution_tampered.replay_hash.push('x');
        let tamper_ok = !execution_replay(&execution_tampered);
        assert!(frontend_ok && execution_ok && tamper_ok);
        replay_verified_count += usize::from(frontend_ok && execution_ok);
        tamper_rejected += usize::from(tamper_ok);
        let numeral = execution
            .numeral
            .as_ref()
            .expect("complete result has numeral");
        let decimal = execution
            .decimal_value
            .expect("complete result has decimal value");
        let candidate_hash = digest(&(numeral, decimal, execution.base, execution.target_base));
        let reference_match = oracle.get(&question.id).map(|expected| {
            candidate_forms(numeral, decimal, execution.target_base)
                .into_iter()
                .any(|form| digest_bytes(form.as_bytes()) == *expected)
        });
        correct_selected_candidates += usize::from(reference_match == Some(true));
        selected_candidates.push(CandidateReceipt {
            id: question.id.clone(),
            candidate_hash,
            baseline_also_executable: baseline.len() == 1,
            reference_match,
            replay_verified: execution_ok,
        });
    }
    let manifest_after = breadth_first_manifest().replay_hash();
    let selected_ids = selected_candidates
        .iter()
        .map(|candidate| candidate.id.clone())
        .collect::<BTreeSet<_>>();
    let incremental_unique_ids = selected_ids.difference(&baseline_ids).count();
    let report = Report {
        schema: "stage407-gap-selected-base-arithmetic-transfer-v1",
        partition: partition.clone(),
        dataset_sha256,
        source_path: SOURCE_PATH,
        source_sha256: digest_bytes(SOURCE.as_bytes()),
        source_id: arithmetic_source_id,
        source_records: source_records(),
        gap_id: GAP_ID,
        operation_scope: SCOPE,
        candidate_sources: candidates.len(),
        selected_sources: selection.selected_source_ids.len(),
        rejected_decoys: 2,
        gap_selection_replay: gap_replay_verified(&gap_request, &selection),
        manifest_sha256_before: manifest_before.clone(),
        manifest_sha256_after: manifest_after.clone(),
        manifest_unchanged: manifest_before == manifest_after,
        answer_keys_read: oracle.len(),
        production_authorizations: 0,
        false_authorizations: 0,
        partition_report: PartitionReport {
            questions: questions.len(),
            baseline_executable: baseline_ids.len(),
            baseline_ambiguities,
            frontend_complete,
            frontend_ambiguous,
            frontend_missing,
            frontend_unsupported,
            execution_complete,
            replay_verified: replay_verified_count,
            tamper_rejected,
            selected_unique_ids: selected_ids.len(),
            incremental_unique_ids,
            answer_hashes_read: oracle.len(),
            plaintext_answers_read: 0,
            correct_selected_candidates,
            candidates: selected_candidates,
        },
        report_sha256: String::new(),
    };
    assert!(report.gap_selection_replay && report.manifest_unchanged);
    assert_eq!(report.production_authorizations, 0);
    assert_eq!(report.false_authorizations, 0);
    if partition == "development" {
        assert_eq!(report.answer_keys_read, 0);
    } else {
        assert_eq!(report.answer_keys_read, report.partition_report.questions);
    }
    let mut unsigned = serde_json::to_value(&report)?;
    unsigned["report_sha256"] = serde_json::Value::String(String::new());
    let mut report = report;
    report.report_sha256 = digest(&unsigned);
    fs::write(
        &report_path,
        format!("{}\n", serde_json::to_string_pretty(&report)?),
    )?;
    fs::write(
        &report_md_path,
        format!(
            "# Stage 407 — typed-gap-selected positional arithmetic transfer\n\n- Partition / questions: {} / {}\n- Selected source / records: `{}` / {}\n- Gap scope / selection replay: `{}` / {}\n- Baseline executable / selected executable / incremental: {} / {} / {}\n- Frontend complete / execution complete: {} / {}\n- Replay / tamper rejection: {} / {}\n- Answer hashes / plaintext answers read: {} / {}\n- Correct selected candidates: {}\n- Production authorizations / false authorizations: {} / {}\n- Manifest unchanged: {}\n\nThe selected OpenStax base-arithmetic source is evaluated beside the existing route-blind portfolio. The route is bounded to two same-base finite numerals and one explicit binary operation; conversion, fractions, mixed bases, digit statistics, and multi-operand expressions remain outside scope. This is shadow-only and measures incremental reachability without mutating production routing.\n",
            report.partition,
            report.partition_report.questions,
            report.source_path,
            report.source_records,
            report.operation_scope,
            report.gap_selection_replay,
            report.partition_report.baseline_executable,
            report.partition_report.selected_unique_ids,
            report.partition_report.incremental_unique_ids,
            report.partition_report.frontend_complete,
            report.partition_report.execution_complete,
            report.partition_report.replay_verified,
            report.partition_report.tamper_rejected,
            report.answer_keys_read,
            report.partition_report.plaintext_answers_read,
            report.partition_report.correct_selected_candidates,
            report.production_authorizations,
            report.false_authorizations,
            report.manifest_unchanged,
        ),
    )?;
    println!(
        "Stage 407 — partition={} selected={} baseline={} incremental={} correct={} answer_hashes={} false_auth=0",
        report.partition,
        report.partition_report.selected_unique_ids,
        report.partition_report.baseline_executable,
        report.partition_report.incremental_unique_ids,
        report.partition_report.correct_selected_candidates,
        report.answer_keys_read,
    );
    Ok(())
}
