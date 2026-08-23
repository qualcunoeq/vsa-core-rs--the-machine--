//! Stage 406: typed-gap-selected overlap transfer.
//!
//! A typed operation gap selects an attributed source lineage by exact scope,
//! then evaluates that source-derived route beside the existing route-blind
//! portfolio.  Development runs never read answer oracles.  Sealed scoring is
//! an explicit privileged hash-only boundary.  The source route remains a
//! clone-only shadow route and cannot mutate the curriculum or production
//! registry.

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::env;
use std::fs;
use the_machine::curriculum::breadth_first_manifest;
use the_machine::goal6_external_portfolio::{executable_routes, observe_all};
use the_machine::source_evidence_envelope::ingest_source_evidence;
use the_machine::source_formula_pack::{
    evaluate_formula_records, extract_formula_records, FormulaStatus,
};
use the_machine::source_selection::{
    gap_replay_verified, select_for_gap, SourceGapRequest, SourceSelectionDecision,
};
use the_machine::source_sequence_frontend::{
    formalize_sequence_terms_text, replay_verified as sequence_frontend_replay,
    SequenceFrontendStatus,
};

const QUESTIONS_PATH: &str = "data/external_math_exam_v1/questions.jsonl";
const RELEASE_DIR: &str = "data/external_math_exam_v1";
const SEQUENCE_SOURCE_PATH: &str = "docs/sources/openstax_precalculus_sequences_source.txt";
const SEQUENCE_SOURCE: &str =
    include_str!("../../docs/sources/openstax_precalculus_sequences_source.txt");
const SCOPE: &str = "finite arithmetic-sequence term evaluation";
const GAP_ID: &str = "gap::finite-arithmetic-sequence-term-evaluation";
const DOMAIN: &str = "stage406_gap_selected_finite_sequence";

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
    baseline_unique_ids: usize,
    selected_frontend_complete: usize,
    selected_frontend_ambiguous: usize,
    selected_frontend_missing: usize,
    selected_frontend_unsupported: usize,
    selected_execution_complete: usize,
    selected_replays: usize,
    selected_tamper_rejections: usize,
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
    baseline_route_observations: usize,
    baseline_route_ambiguities: usize,
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

fn source_evidence(
    path: &str,
    source_id: &str,
    title: &str,
    section: &str,
    url: &str,
    evidence: &str,
    scope: &str,
) -> the_machine::source_evidence_envelope::SourceEvidenceEnvelope {
    let document = format!(
        "SOURCE_ID: {source_id}\nTITLE: {title}\nSECTION: {section}\nURL: {url}\nLICENSE: CC BY 4.0\nRETRIEVED_UTC: 2026-08-20\nEVIDENCE: {evidence}\nSCOPE: {scope}"
    );
    ingest_source_evidence(path, &document).expect("source metadata is valid")
}

fn terminating_decimal(value: &the_machine::probability_pack::Rational) -> Option<String> {
    if value.denominator <= 0 {
        return None;
    }
    let mut denominator = value.denominator;
    let mut twos = 0u32;
    let mut fives = 0u32;
    while denominator % 2 == 0 {
        denominator /= 2;
        twos += 1;
    }
    while denominator % 5 == 0 {
        denominator /= 5;
        fives += 1;
    }
    if denominator != 1 {
        return None;
    }
    let places = twos.max(fives);
    let numerator = value
        .numerator
        .checked_mul(2_i128.checked_pow(places - twos)?)?
        .checked_mul(5_i128.checked_pow(places - fives)?)?;
    let negative = numerator < 0;
    let digits = numerator.abs().to_string();
    let rendered = if places == 0 {
        digits
    } else if digits.len() <= places as usize {
        format!("0.{}{}", "0".repeat(places as usize - digits.len()), digits)
    } else {
        let split = digits.len() - places as usize;
        format!("{}.{}", &digits[..split], &digits[split..])
    };
    Some(if negative {
        format!("-{rendered}")
    } else {
        rendered
    })
}

fn candidate_forms(value: &the_machine::probability_pack::Rational) -> Vec<String> {
    let mut forms = vec![if value.denominator == 1 {
        value.numerator.to_string()
    } else {
        format!("{}/{}", value.numerator, value.denominator)
    }];
    if let Some(decimal) = terminating_decimal(value) {
        forms.push(decimal);
    }
    forms
}

fn oracle_for_partition(
    partition: &str,
) -> Result<BTreeMap<String, String>, Box<dyn std::error::Error>> {
    if partition == "development" {
        return Ok(BTreeMap::new());
    }
    assert_eq!(
        env::var("GOAL6_STAGE406_PRIVILEGED_EVAL").as_deref(),
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

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let partition = env::var("GOAL6_STAGE406_PARTITION").unwrap_or_else(|_| "development".into());
    assert!(matches!(partition.as_str(), "development" | "sealed"));
    let report_path = env::var("GOAL6_STAGE406_REPORT_JSON")
        .unwrap_or_else(|_| format!("docs/stage406_gap_selected_overlap_{partition}.json"));
    let report_md_path = env::var("GOAL6_STAGE406_REPORT_MD")
        .unwrap_or_else(|_| format!("docs/stage406_gap_selected_overlap_{partition}.md"));
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
    let records = extract_formula_records(SEQUENCE_SOURCE).map_err(|errors| errors.join("; "))?;
    let source_id = records
        .first()
        .expect("sequence source has records")
        .source
        .source_id
        .clone();
    let candidates = vec![
        source_evidence(
            SEQUENCE_SOURCE_PATH,
            &source_id,
            "Precalculus Sequences",
            "Arithmetic Sequences",
            "https://openstax.org/details/books/precalculus-2e",
            "finite arithmetic-sequence term formula",
            SCOPE,
        ),
        source_evidence(
            "docs/sources/openstax_finite_statistics_source.txt",
            "openstax-introductory-statistics-2e:descriptive-statistics",
            "Introductory Statistics 2e",
            "Measures of Central Tendency",
            "https://openstax.org/details/books/introductory-statistics-2e",
            "finite-list mean formula",
            "finite-list mean evaluation",
        ),
        source_evidence(
            "docs/sources/openstax_base_conversion_source.txt",
            "openstax-prealgebra-2e:base-conversion",
            "Prealgebra 2e",
            "Numeration",
            "https://openstax.org/details/books/prealgebra-2e",
            "bounded positional conversion",
            "finite positional base conversion",
        ),
    ];
    let gap_request = SourceGapRequest::new(GAP_ID, vec![SCOPE.into()], 1);
    let selection = select_for_gap(&gap_request, &candidates);
    assert!(gap_replay_verified(&gap_request, &selection));
    assert_eq!(selection.selected_source_ids, vec![source_id.clone()]);
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
    let mut selected_candidates = Vec::new();
    let mut selected_frontend_complete = 0;
    let mut selected_frontend_ambiguous = 0;
    let mut selected_frontend_missing = 0;
    let mut selected_frontend_unsupported = 0;
    let mut selected_execution_complete = 0;
    let mut selected_replays = 0;
    let mut selected_tamper_rejections = 0;
    let mut correct_selected_candidates = 0;
    for question in &questions {
        let observations = observe_all(&question.original_prompt, &question.id);
        let baseline = executable_routes(&observations);
        if baseline.len() > 1 {
            baseline_ambiguities += 1;
        }
        if baseline.len() == 1 {
            baseline_ids.insert(question.id.clone());
        }
        let frontend =
            formalize_sequence_terms_text(&question.original_prompt, &question.id, DOMAIN);
        match frontend.status {
            SequenceFrontendStatus::Complete => selected_frontend_complete += 1,
            SequenceFrontendStatus::Ambiguous => selected_frontend_ambiguous += 1,
            SequenceFrontendStatus::Missing => selected_frontend_missing += 1,
            SequenceFrontendStatus::Unsupported => selected_frontend_unsupported += 1,
        }
        let frontend_replay = sequence_frontend_replay(&frontend);
        let mut frontend_tampered = frontend.clone();
        frontend_tampered.replay_hash.push('x');
        assert!(!sequence_frontend_replay(&frontend_tampered));
        let Some(formula_request) = frontend.request.as_ref() else {
            continue;
        };
        let execution = evaluate_formula_records(formula_request, DOMAIN, &records);
        if execution.status != FormulaStatus::Complete {
            continue;
        }
        selected_execution_complete += 1;
        let execution_replay = execution.replay_verified();
        let mut execution_tampered = execution.clone();
        execution_tampered.replay_hash.push('x');
        let execution_tamper_rejected = !execution_tampered.replay_verified();
        assert!(frontend_replay);
        assert!(execution_replay);
        assert!(execution_tamper_rejected);
        selected_replays += usize::from(frontend_replay && execution_replay);
        selected_tamper_rejections += usize::from(execution_tamper_rejected);
        let value = execution
            .value
            .as_ref()
            .expect("complete execution has value");
        let candidate_hash = digest(value);
        let reference_match = oracle.get(&question.id).map(|expected| {
            candidate_forms(value)
                .into_iter()
                .any(|form| digest_bytes(form.as_bytes()) == *expected)
        });
        correct_selected_candidates += usize::from(reference_match == Some(true));
        selected_candidates.push(CandidateReceipt {
            id: question.id.clone(),
            candidate_hash,
            baseline_also_executable: baseline.len() == 1,
            reference_match,
            replay_verified: execution_replay,
        });
    }
    let manifest_after = breadth_first_manifest().replay_hash();
    let selected_ids = selected_candidates
        .iter()
        .map(|candidate| candidate.id.clone())
        .collect::<BTreeSet<_>>();
    let incremental_unique_ids = selected_ids.difference(&baseline_ids).count();
    let report = Report {
        schema: "stage406-gap-selected-overlap-transfer-v1",
        partition: partition.clone(),
        dataset_sha256,
        source_path: SEQUENCE_SOURCE_PATH,
        source_sha256: digest_bytes(SEQUENCE_SOURCE.as_bytes()),
        source_id,
        source_records: records.len(),
        gap_id: GAP_ID,
        operation_scope: SCOPE,
        candidate_sources: candidates.len(),
        selected_sources: selection.selected_source_ids.len(),
        rejected_decoys: 2,
        gap_selection_replay: gap_replay_verified(&gap_request, &selection),
        baseline_route_observations: questions.len(),
        baseline_route_ambiguities: baseline_ambiguities,
        manifest_sha256_before: manifest_before.clone(),
        manifest_sha256_after: manifest_after.clone(),
        manifest_unchanged: manifest_before == manifest_after,
        answer_keys_read: oracle.len(),
        production_authorizations: 0,
        false_authorizations: 0,
        partition_report: PartitionReport {
            questions: questions.len(),
            baseline_executable: baseline_ids.len(),
            baseline_unique_ids: baseline_ids.len(),
            selected_frontend_complete,
            selected_frontend_ambiguous,
            selected_frontend_missing,
            selected_frontend_unsupported,
            selected_execution_complete,
            selected_replays,
            selected_tamper_rejections,
            selected_unique_ids: selected_ids.len(),
            incremental_unique_ids,
            answer_hashes_read: oracle.len(),
            plaintext_answers_read: 0,
            correct_selected_candidates,
            candidates: selected_candidates,
        },
        report_sha256: String::new(),
    };
    assert!(report.gap_selection_replay);
    assert!(report.manifest_unchanged);
    if partition == "development" {
        assert_eq!(
            report.answer_keys_read, 0,
            "development must remain answer-key blind"
        );
    } else {
        assert_eq!(report.answer_keys_read, report.partition_report.questions);
    }
    assert_eq!(report.production_authorizations, 0);
    assert_eq!(report.false_authorizations, 0);
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
            "# Stage 406 — typed-gap-selected overlap transfer\n\n\
             - Partition / questions: {} / {}\n\
             - Selected source / records: `{}` / {}\n\
             - Gap scope / selection replay: `{}` / {}\n\
             - Baseline executable / selected executable / incremental: {} / {} / {}\n\
             - Selected frontend complete / execution complete: {} / {}\n\
             - Replay / tamper rejection: {} / {}\n\
             - Answer hashes / plaintext answers read: {} / {}\n\
             - Correct selected candidates: {}\n\
             - Production authorizations / false authorizations: {} / {}\n\
             - Manifest unchanged: {}\n\n\
             The typed gap selected the sequence lineage by exact declared operation scope; \
             mean and base-conversion lineages were rejected as decoys. The selected route \
             is evaluated beside the existing route-blind portfolio. This report measures \
             incremental reachability; it does not promote or mutate production routing.\n",
            report.partition,
            report.partition_report.questions,
            report.source_path,
            report.source_records,
            report.operation_scope,
            report.gap_selection_replay,
            report.partition_report.baseline_unique_ids,
            report.partition_report.selected_unique_ids,
            report.partition_report.incremental_unique_ids,
            report.partition_report.selected_frontend_complete,
            report.partition_report.selected_execution_complete,
            report.partition_report.selected_replays,
            report.partition_report.selected_tamper_rejections,
            report.answer_keys_read,
            report.partition_report.plaintext_answers_read,
            report.partition_report.correct_selected_candidates,
            report.production_authorizations,
            report.false_authorizations,
            report.manifest_unchanged,
        ),
    )?;
    println!(
        "Stage 406 — partition={} selected={} baseline={} incremental={} correct={} answer_hashes={} false_auth=0",
        report.partition,
        report.partition_report.selected_unique_ids,
        report.partition_report.baseline_unique_ids,
        report.partition_report.incremental_unique_ids,
        report.partition_report.correct_selected_candidates,
        report.answer_keys_read,
    );
    Ok(())
}
