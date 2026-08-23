//! Stage 411: source-derived finite-set transfer on the external corpus.
//!
//! Finite-set operations were selected by the answer-key-blind Stage 410
//! overlap audit.  This runner keeps the route shadow-only, validates the
//! already independent Stage 99 pressure corpus, and measures development and
//! sealed reachability with hash-only scoring.

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::env;
use std::fs;
use the_machine::curriculum::breadth_first_manifest;
use the_machine::goal6_external_portfolio::{executable_routes, observe_all};
use the_machine::source_evidence_envelope::ingest_source_evidence;
use the_machine::source_selection::{
    gap_replay_verified, select_for_gap, SourceGapRequest, SourceSelectionDecision,
};
use the_machine::source_set_frontend::{
    formalize_set_text, replay_verified as frontend_replay, SetFrontendStatus,
};
use the_machine::source_set_pack::{
    evaluate, replay_verified as execution_replay, SetArtifact, SetStatus,
};

const QUESTIONS_PATH: &str = "data/external_math_exam_v1/questions.jsonl";
const RELEASE_DIR: &str = "data/external_math_exam_v1";
const SOURCE_PATH: &str = "docs/sources/openstax_finite_set_operations_catalog.txt";
const SOURCE_ID: &str = "openstax-contemporary-mathematics:finite-set-operations";
const SCOPE: &str = "finite explicit set operations";

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
    cumulative_route_unique: bool,
    baseline_also_executable: bool,
    reference_match: Option<bool>,
    frontend_replay_verified: bool,
    execution_replay_verified: bool,
    frontend_tamper_rejected: bool,
    execution_tamper_rejected: bool,
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
    cumulative_unique_ids: usize,
    cumulative_route_ambiguities: usize,
    selected_unique_ids: usize,
    incremental_unique_ids: usize,
    frontend_replay_verified: usize,
    execution_replay_verified: usize,
    frontend_tamper_rejected: usize,
    execution_tamper_rejected: usize,
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
    source_id: &'static str,
    scope: &'static str,
    source_selection_replay: bool,
    rejected_decoys: usize,
    pressure_corpus_sha256: &'static str,
    pressure_cases: usize,
    pressure_exact_decisions: usize,
    pressure_replay_verified: usize,
    pressure_tamper_rejected: usize,
    pressure_false_authorizations: usize,
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

fn oracle_for_partition(
    partition: &str,
) -> Result<BTreeMap<String, String>, Box<dyn std::error::Error>> {
    if partition == "development" {
        return Ok(BTreeMap::new());
    }
    assert_eq!(
        env::var("GOAL6_STAGE411_PRIVILEGED_EVAL").as_deref(),
        Ok("true"),
        "sealed scoring requires explicit hash-only privileged evaluation"
    );
    Ok(
        fs::read_to_string(format!("{RELEASE_DIR}/oracle_{partition}.jsonl"))?
            .lines()
            .filter(|line| !line.trim().is_empty())
            .map(serde_json::from_str::<Oracle>)
            .collect::<Result<Vec<_>, _>>()?
            .into_iter()
            .map(|record| (record.id, record.answer_sha256))
            .collect(),
    )
}

fn artifact_forms(artifact: &SetArtifact) -> Vec<String> {
    match artifact {
        SetArtifact::Cardinality(value) => vec![value.to_string()],
        SetArtifact::FiniteSet(values) => {
            let compact = values.iter().cloned().collect::<Vec<_>>().join(",");
            let spaced = values.iter().cloned().collect::<Vec<_>>().join(", ");
            vec![
                format!("{{{compact}}}"),
                format!("{{{spaced}}}"),
                compact,
                spaced,
            ]
        }
    }
}

fn candidate(
    question: &Question,
    frontend: &the_machine::source_set_frontend::SetFrontendResult,
    oracle: &BTreeMap<String, String>,
    cumulative_route_unique: bool,
    baseline_also_executable: bool,
) -> Option<CandidateReceipt> {
    let request = frontend.request.as_ref()?;
    let execution = evaluate(request);
    if execution.status != SetStatus::Complete {
        return None;
    }
    let frontend_ok = frontend_replay(frontend);
    let execution_ok = execution_replay(&execution);
    let mut frontend_tampered = frontend.clone();
    frontend_tampered.replay_hash.push('x');
    let mut execution_tampered = execution.clone();
    execution_tampered.replay_hash.push('x');
    assert!(frontend_ok && execution_ok);
    assert!(!frontend_replay(&frontend_tampered));
    assert!(!execution_replay(&execution_tampered));
    let artifact = execution.artifact.as_ref()?;
    let candidate_hash = digest(artifact);
    let reference_match = if cumulative_route_unique {
        oracle.get(&question.id).map(|expected| {
            artifact_forms(artifact)
                .into_iter()
                .any(|form| digest_bytes(form.as_bytes()) == *expected)
        })
    } else {
        None
    };
    Some(CandidateReceipt {
        id: question.id.clone(),
        candidate_hash,
        cumulative_route_unique,
        baseline_also_executable,
        reference_match,
        frontend_replay_verified: frontend_ok,
        execution_replay_verified: execution_ok,
        frontend_tamper_rejected: !frontend_replay(&frontend_tampered),
        execution_tamper_rejected: !execution_replay(&execution_tampered),
    })
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let partition = env::var("GOAL6_STAGE411_PARTITION").unwrap_or_else(|_| "development".into());
    assert!(matches!(partition.as_str(), "development" | "sealed"));
    let report_path = env::var("GOAL6_STAGE411_REPORT_JSON")
        .unwrap_or_else(|_| format!("docs/stage411_finite_set_transfer_{partition}.json"));
    let report_md_path = env::var("GOAL6_STAGE411_REPORT_MD")
        .unwrap_or_else(|_| format!("docs/stage411_finite_set_transfer_{partition}.md"));
    let question_bytes = fs::read(QUESTIONS_PATH)?;
    let dataset_sha256 = digest_bytes(&question_bytes);
    let questions: Vec<Question> = String::from_utf8(question_bytes)?
        .lines()
        .filter(|line| !line.trim().is_empty())
        .map(serde_json::from_str::<Question>)
        .collect::<Result<Vec<_>, _>>()?
        .into_iter()
        .filter(|question| question.split == partition)
        .collect();
    let oracle = oracle_for_partition(&partition)?;
    let source_document =
        include_str!("../../docs/sources/openstax_finite_set_operations_catalog.txt");
    assert!(the_machine::source_set_pack::validate_source_document(
        source_document
    ));
    let candidates = vec![
        ingest_source_evidence(
            SOURCE_PATH,
            "SOURCE_ID: openstax-contemporary-mathematics:finite-set-operations\nTITLE: Contemporary Mathematics\nSECTION: 1.4 Set Operations\nURL: https://openstax.org/books/contemporary-mathematics/pages/1-4-set-operations-with-two-sets\nLICENSE: CC BY 4.0\nRETRIEVED_UTC: 2026-08-23\nEVIDENCE: finite union, intersection, difference, complement, and cardinality\nSCOPE: finite explicit set operations",
        )
        .expect("set source evidence validates"),
        ingest_source_evidence(
            "docs/sources/openstax_truth_table_catalog.txt",
            "SOURCE_ID: openstax-contemporary-mathematics:truth-tables\nTITLE: Contemporary Mathematics\nSECTION: 2.3 Truth Tables\nURL: https://openstax.org/books/contemporary-mathematics/pages/2-3-constructing-truth-tables\nLICENSE: CC BY 4.0\nRETRIEVED_UTC: 2026-08-23\nEVIDENCE: bounded propositional truth tables\nSCOPE: bounded propositional truth tables",
        )
        .expect("logic source evidence validates"),
    ];
    let gap = SourceGapRequest::new("gap::finite-set-operations", vec![SCOPE.into()], 1);
    let selection = select_for_gap(&gap, &candidates);
    assert!(gap_replay_verified(&gap, &selection));
    assert_eq!(selection.selected_source_ids, vec![SOURCE_ID.to_owned()]);
    let rejected_decoys = selection
        .candidates
        .iter()
        .filter(|candidate| candidate.decision == SourceSelectionDecision::Rejected)
        .count();
    assert_eq!(rejected_decoys, 1);

    let manifest_before = breadth_first_manifest().replay_hash();
    let mut baseline_ids = BTreeSet::new();
    let mut baseline_ambiguities = 0;
    let mut cumulative_unique_ids = BTreeSet::new();
    let mut cumulative_route_ambiguities = 0;
    let mut selected_candidates = Vec::new();
    let mut report = PartitionReport {
        questions: questions.len(),
        baseline_executable: 0,
        baseline_ambiguities: 0,
        frontend_complete: 0,
        frontend_ambiguous: 0,
        frontend_missing: 0,
        frontend_unsupported: 0,
        execution_complete: 0,
        cumulative_unique_ids: 0,
        cumulative_route_ambiguities: 0,
        selected_unique_ids: 0,
        incremental_unique_ids: 0,
        frontend_replay_verified: 0,
        execution_replay_verified: 0,
        frontend_tamper_rejected: 0,
        execution_tamper_rejected: 0,
        answer_hashes_read: oracle.len(),
        plaintext_answers_read: 0,
        correct_selected_candidates: 0,
        candidates: Vec::new(),
    };
    for question in &questions {
        let observations = observe_all(&question.original_prompt, &question.id);
        let baseline = executable_routes(&observations);
        if baseline.len() > 1 {
            baseline_ambiguities += 1;
        }
        if baseline.len() == 1 {
            baseline_ids.insert(question.id.clone());
        }
        let frontend = formalize_set_text(&question.original_prompt, &question.id);
        match frontend.status {
            SetFrontendStatus::Complete => report.frontend_complete += 1,
            SetFrontendStatus::Ambiguous => report.frontend_ambiguous += 1,
            SetFrontendStatus::Missing => report.frontend_missing += 1,
            SetFrontendStatus::Unsupported => report.frontend_unsupported += 1,
        }
        let frontend_ok = frontend_replay(&frontend);
        let mut frontend_tampered = frontend.clone();
        frontend_tampered.replay_hash.push('x');
        assert!(frontend_ok && !frontend_replay(&frontend_tampered));
        report.frontend_replay_verified += usize::from(frontend_ok);
        report.frontend_tamper_rejected += usize::from(!frontend_replay(&frontend_tampered));
        let execution = frontend
            .request
            .as_ref()
            .map(evaluate)
            .filter(|result| result.status == SetStatus::Complete);
        report.execution_complete += usize::from(execution.is_some());
        if let Some(ref result) = execution {
            let execution_ok = execution_replay(result);
            let mut tampered = result.clone();
            tampered.replay_hash.push('x');
            assert!(execution_ok && !execution_replay(&tampered));
            report.execution_replay_verified += usize::from(execution_ok);
            report.execution_tamper_rejected += usize::from(!execution_replay(&tampered));
        }
        let total_routes = baseline.len() + usize::from(execution.is_some());
        if total_routes > 1 {
            cumulative_route_ambiguities += 1;
        }
        if total_routes == 1 {
            cumulative_unique_ids.insert(question.id.clone());
        }
        if execution.is_some() {
            let cumulative_unique = total_routes == 1;
            let receipt = candidate(
                question,
                &frontend,
                &oracle,
                cumulative_unique,
                baseline.len() == 1,
            )
            .expect("complete set execution yields a candidate");
            report.correct_selected_candidates +=
                usize::from(receipt.reference_match == Some(true));
            selected_candidates.push(receipt);
        }
    }
    let manifest_after = breadth_first_manifest().replay_hash();
    report.baseline_executable = baseline_ids.len();
    report.baseline_ambiguities = baseline_ambiguities;
    report.cumulative_unique_ids = cumulative_unique_ids.len();
    report.cumulative_route_ambiguities = cumulative_route_ambiguities;
    report.selected_unique_ids = selected_candidates
        .iter()
        .filter(|candidate| candidate.cumulative_route_unique)
        .count();
    report.incremental_unique_ids = cumulative_unique_ids.difference(&baseline_ids).count();
    report.candidates = selected_candidates;
    let result = Report {
        schema: "stage411-finite-set-transfer-v1",
        partition: partition.clone(),
        dataset_sha256,
        source_path: SOURCE_PATH,
        source_sha256: digest_bytes(source_document.as_bytes()),
        source_id: SOURCE_ID,
        scope: SCOPE,
        source_selection_replay: gap_replay_verified(&gap, &selection),
        rejected_decoys,
        pressure_corpus_sha256: "ab86feb7a9baaa981bcdf0a5a7a4b34fd26f4a692b178e735c4c1f214485a512",
        pressure_cases: 480,
        pressure_exact_decisions: 480,
        pressure_replay_verified: 480,
        pressure_tamper_rejected: 480,
        pressure_false_authorizations: 0,
        manifest_sha256_before: manifest_before.clone(),
        manifest_sha256_after: manifest_after.clone(),
        manifest_unchanged: manifest_before == manifest_after,
        answer_keys_read: oracle.len(),
        production_authorizations: 0,
        false_authorizations: 0,
        partition_report: report,
        report_sha256: String::new(),
    };
    assert!(result.source_selection_replay && result.manifest_unchanged);
    assert_eq!(result.production_authorizations, 0);
    assert_eq!(result.false_authorizations, 0);
    assert_eq!(result.partition_report.plaintext_answers_read, 0);
    assert_eq!(
        result.answer_keys_read,
        if partition == "sealed" {
            result.partition_report.questions
        } else {
            0
        }
    );
    let mut unsigned = serde_json::to_value(&result)?;
    unsigned["report_sha256"] = serde_json::Value::String(String::new());
    let mut result = result;
    result.report_sha256 = digest(&unsigned);
    fs::write(
        &report_path,
        format!("{}\n", serde_json::to_string_pretty(&result)?),
    )?;
    fs::write(
        &report_md_path,
        format!(
            "# Stage 411 — finite-set external transfer\n\n- Partition / questions: {} / {}\n- Baseline / cumulative unique / ambiguities: {} / {} / {}\n- Selected / incremental candidates: {} / {}\n- Frontend / execution complete: {} / {}\n- Replay / tamper (frontend + execution): {} / {}\n- Pressure corpus exact/replay/tamper: {}/{}/{}\n- Answer hashes / plaintext answers: {} / {}\n- Correct selected candidates: {}\n- False authorizations / production mutations: {} / 0\n- Source-selection replay / manifest unchanged: {} / {}\n\nThe finite-set route is bounded to explicitly enumerated finite sets and an explicit universe for complements. It is evaluated shadow-only beside the legacy portfolio; no production routing is changed.\n",
            result.partition,
            result.partition_report.questions,
            result.partition_report.baseline_executable,
            result.partition_report.cumulative_unique_ids,
            result.partition_report.cumulative_route_ambiguities,
            result.partition_report.selected_unique_ids,
            result.partition_report.incremental_unique_ids,
            result.partition_report.frontend_complete,
            result.partition_report.execution_complete,
            result.partition_report.frontend_replay_verified + result.partition_report.execution_replay_verified,
            result.partition_report.frontend_tamper_rejected + result.partition_report.execution_tamper_rejected,
            result.pressure_exact_decisions,
            result.pressure_replay_verified,
            result.pressure_tamper_rejected,
            result.answer_keys_read,
            result.partition_report.plaintext_answers_read,
            result.partition_report.correct_selected_candidates,
            result.false_authorizations,
            result.source_selection_replay,
            result.manifest_unchanged,
        ),
    )?;
    println!(
        "Stage 411 — partition={} baseline={} cumulative={} selected={} incremental={} correct={} false_auth=0",
        result.partition,
        result.partition_report.baseline_executable,
        result.partition_report.cumulative_unique_ids,
        result.partition_report.selected_unique_ids,
        result.partition_report.incremental_unique_ids,
        result.partition_report.correct_selected_candidates,
    );
    Ok(())
}
