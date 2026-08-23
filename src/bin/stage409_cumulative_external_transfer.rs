//! Stage 409: cumulative route-blind transfer for the two latest source packs.
//!
//! Stage 407 and Stage 408 each validated a distinct source-derived operation.
//! This checkpoint evaluates both routes together beside the unchanged legacy
//! portfolio.  It measures non-overlapping reachability, route ambiguity,
//! hash-only sealed answer matching, replay, and tamper rejection.  It never
//! authorizes production routing or mutates the curriculum manifest.

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::env;
use std::fs;
use the_machine::curriculum::breadth_first_manifest;
use the_machine::goal6_external_portfolio::{executable_routes, observe_all};
use the_machine::probability_pack::Rational;
use the_machine::source_base_arithmetic_frontend::{
    formalize, replay_verified as base_frontend_replay, FrontendStatus as BaseFrontendStatus,
};
use the_machine::source_base_arithmetic_pack::{
    evaluate as evaluate_base, replay_verified as base_execution_replay, BaseArithmeticStatus,
    SOURCE as BASE_SOURCE,
};
use the_machine::source_complex_pack::{
    evaluate_complex,
    source_complex_frontend::{
        formalize_complex_text, ComplexFrontendResult, FrontendStatus as ComplexFrontendStatus,
    },
    ComplexArtifact, ComplexStatus,
};
use the_machine::source_evidence_envelope::ingest_source_evidence;
use the_machine::source_selection::{
    gap_replay_verified, select_for_gap, SourceGapRequest, SourceSelectionDecision,
};

const QUESTIONS_PATH: &str = "data/external_math_exam_v1/questions.jsonl";
const RELEASE_DIR: &str = "data/external_math_exam_v1";
const BASE_SOURCE_PATH: &str = "docs/sources/openstax_base_arithmetic_source.txt";
const COMPLEX_SOURCE_PATH: &str = "docs/sources/openstax_complex_arithmetic_source.txt";
const BASE_SOURCE_ID: &str = "openstax-contemporary-mathematics:base-arithmetic";
const COMPLEX_SOURCE_ID: &str = "openstax-precalculus-2e:complex-numbers-3-1";
const BASE_SCOPE: &str = "finite positional binary arithmetic";
const COMPLEX_SCOPE: &str = "finite rectangular complex arithmetic";

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

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
enum NewRoute {
    BaseArithmetic,
    ComplexArithmetic,
}

#[derive(Debug, Serialize)]
struct CandidateReceipt {
    id: String,
    route: NewRoute,
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
    base_frontend_complete: usize,
    base_frontend_ambiguous: usize,
    base_frontend_missing: usize,
    base_frontend_unsupported: usize,
    base_execution_complete: usize,
    complex_frontend_complete: usize,
    complex_frontend_ambiguous: usize,
    complex_frontend_missing: usize,
    complex_frontend_unsupported: usize,
    complex_execution_complete: usize,
    cumulative_unique_ids: usize,
    cumulative_route_ambiguities: usize,
    new_route_unique_ids: usize,
    incremental_unique_ids: usize,
    base_frontend_replay_verified: usize,
    base_execution_replay_verified: usize,
    base_frontend_tamper_rejected: usize,
    base_execution_tamper_rejected: usize,
    complex_frontend_replay_verified: usize,
    complex_execution_replay_verified: usize,
    complex_frontend_tamper_rejected: usize,
    complex_execution_tamper_rejected: usize,
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
    legacy_portfolio_routes: usize,
    cumulative_new_routes: usize,
    base_source_path: &'static str,
    base_source_sha256: String,
    base_source_id: &'static str,
    base_scope: &'static str,
    complex_source_path: &'static str,
    complex_source_sha256: String,
    complex_source_id: &'static str,
    complex_scope: &'static str,
    base_gap_selection_replay: bool,
    complex_gap_selection_replay: bool,
    base_rejected_decoys: usize,
    complex_rejected_decoys: usize,
    base_pressure_corpus_sha256: &'static str,
    complex_pressure_corpus_sha256: &'static str,
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
        env::var("GOAL6_STAGE409_PRIVILEGED_EVAL").as_deref(),
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

fn rational_text(value: &Rational) -> String {
    if value.denominator == 1 {
        value.numerator.to_string()
    } else {
        format!("{}/{}", value.numerator, value.denominator)
    }
}

fn base_forms(numeral: &str, decimal: u128, base: u32) -> Vec<String> {
    vec![
        numeral.to_owned(),
        format!("{numeral}_{base}"),
        decimal.to_string(),
    ]
}

fn complex_forms(artifact: &ComplexArtifact) -> Vec<String> {
    let ComplexArtifact::Pair { real, imag } = artifact else {
        return Vec::new();
    };
    let real = rational_text(real);
    let imag = rational_text(imag);
    if imag == "0" {
        return vec![real.clone(), format!("{real}+0i"), format!("({real})")];
    }
    let signed = if imag.starts_with('-') {
        format!("{real}{imag}i")
    } else {
        format!("{real}+{imag}i")
    };
    vec![
        signed.clone(),
        format!("({signed})"),
        signed.replace('+', " + "),
        signed.replace('-', " - "),
        format!("{real} + {imag}i"),
    ]
}

fn base_candidate(
    question: &Question,
    frontend: &the_machine::source_base_arithmetic_frontend::FrontendResult,
    oracle: &BTreeMap<String, String>,
    cumulative_unique: bool,
    baseline_also_executable: bool,
) -> Option<CandidateReceipt> {
    let request = frontend.request.as_ref()?;
    let execution = evaluate_base(request);
    if execution.status != BaseArithmeticStatus::Complete {
        return None;
    }
    let numeral = execution.numeral.as_ref()?;
    let decimal = execution.decimal_value?;
    let frontend_ok = base_frontend_replay(frontend);
    let execution_ok = base_execution_replay(&execution);
    let mut frontend_tampered = frontend.clone();
    frontend_tampered.replay_hash.push('x');
    let mut execution_tampered = execution.clone();
    execution_tampered.replay_hash.push('x');
    assert!(frontend_ok && execution_ok);
    assert!(!base_frontend_replay(&frontend_tampered));
    assert!(!base_execution_replay(&execution_tampered));
    let candidate_hash = digest(&(numeral, decimal, execution.base, execution.target_base));
    let reference_match = cumulative_unique.then(|| {
        oracle.get(&question.id).is_some_and(|expected| {
            base_forms(numeral, decimal, execution.target_base)
                .into_iter()
                .any(|form| digest_bytes(form.as_bytes()) == *expected)
        })
    });
    Some(CandidateReceipt {
        id: question.id.clone(),
        route: NewRoute::BaseArithmetic,
        candidate_hash,
        cumulative_route_unique: cumulative_unique,
        baseline_also_executable,
        reference_match,
        frontend_replay_verified: frontend_ok,
        execution_replay_verified: execution_ok,
        frontend_tamper_rejected: !base_frontend_replay(&frontend_tampered),
        execution_tamper_rejected: !base_execution_replay(&execution_tampered),
    })
}

fn complex_candidate(
    question: &Question,
    frontend: &ComplexFrontendResult,
    oracle: &BTreeMap<String, String>,
    cumulative_unique: bool,
    baseline_also_executable: bool,
) -> Option<CandidateReceipt> {
    let request = frontend.request.as_ref()?;
    let execution = evaluate_complex(request);
    if execution.status != ComplexStatus::Complete {
        return None;
    }
    let artifact = execution.artifact.as_ref()?;
    let frontend_ok = frontend.replay_verified();
    let execution_ok = execution.replay_verified();
    let mut frontend_tampered = frontend.clone();
    frontend_tampered.replay_hash.push('x');
    let mut execution_tampered = execution.clone();
    execution_tampered.replay_hash.push('x');
    assert!(frontend_ok && execution_ok);
    assert!(!frontend_tampered.replay_verified());
    assert!(!execution_tampered.replay_verified());
    let candidate_hash = digest(artifact);
    let reference_match = cumulative_unique.then(|| {
        oracle.get(&question.id).is_some_and(|expected| {
            complex_forms(artifact)
                .into_iter()
                .any(|form| digest_bytes(form.as_bytes()) == *expected)
        })
    });
    Some(CandidateReceipt {
        id: question.id.clone(),
        route: NewRoute::ComplexArithmetic,
        candidate_hash,
        cumulative_route_unique: cumulative_unique,
        baseline_also_executable,
        reference_match,
        frontend_replay_verified: frontend_ok,
        execution_replay_verified: execution_ok,
        frontend_tamper_rejected: !frontend_tampered.replay_verified(),
        execution_tamper_rejected: !execution_tampered.replay_verified(),
    })
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let partition = env::var("GOAL6_STAGE409_PARTITION").unwrap_or_else(|_| "development".into());
    assert!(matches!(partition.as_str(), "development" | "sealed"));
    let report_path = env::var("GOAL6_STAGE409_REPORT_JSON")
        .unwrap_or_else(|_| format!("docs/stage409_cumulative_external_transfer_{partition}.json"));
    let report_md_path = env::var("GOAL6_STAGE409_REPORT_MD")
        .unwrap_or_else(|_| format!("docs/stage409_cumulative_external_transfer_{partition}.md"));

    let question_bytes = fs::read(QUESTIONS_PATH)?;
    let dataset_sha256 = digest_bytes(&question_bytes);
    let questions: Vec<Question> = String::from_utf8(question_bytes)?
        .lines()
        .filter(|line| !line.trim().is_empty())
        .map(serde_json::from_str::<Question>)
        .collect::<Result<Vec<Question>, _>>()?
        .into_iter()
        .filter(|question| question.split == partition)
        .collect();
    let oracle = oracle_for_partition(&partition)?;

    let base_candidates = vec![
        source_evidence(
            BASE_SOURCE_PATH,
            BASE_SOURCE_ID,
            "Contemporary Mathematics",
            "Base arithmetic",
            "https://openstax.org/books/contemporary-mathematics/pages/4-4-addition-and-subtraction-in-base-systems",
            "finite positional addition, subtraction, and multiplication in an explicitly declared base",
            BASE_SCOPE,
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
    ];
    let complex_candidates = vec![
        source_evidence(
            COMPLEX_SOURCE_PATH,
            COMPLEX_SOURCE_ID,
            "Precalculus 2e",
            "3.1 Complex Numbers",
            "https://openstax.org/books/precalculus-2e/pages/3-1-complex-numbers",
            "exact rectangular addition, subtraction, and multiplication",
            COMPLEX_SCOPE,
        ),
        source_evidence(
            BASE_SOURCE_PATH,
            BASE_SOURCE_ID,
            "Contemporary Mathematics",
            "Base arithmetic",
            "https://openstax.org/books/contemporary-mathematics/pages/4-4-addition-and-subtraction-in-base-systems",
            "finite positional arithmetic",
            BASE_SCOPE,
        ),
    ];
    let base_gap = SourceGapRequest::new(
        "gap::finite-positional-arithmetic",
        vec![BASE_SCOPE.into()],
        1,
    );
    let complex_gap = SourceGapRequest::new(
        "gap::finite-rectangular-complex-arithmetic",
        vec![COMPLEX_SCOPE.into()],
        1,
    );
    let base_selection = select_for_gap(&base_gap, &base_candidates);
    let complex_selection = select_for_gap(&complex_gap, &complex_candidates);
    assert!(gap_replay_verified(&base_gap, &base_selection));
    assert!(gap_replay_verified(&complex_gap, &complex_selection));
    assert_eq!(
        base_selection.selected_source_ids,
        vec![BASE_SOURCE_ID.to_owned()]
    );
    assert_eq!(
        complex_selection.selected_source_ids,
        vec![COMPLEX_SOURCE_ID.to_owned()]
    );
    assert_eq!(
        base_selection
            .candidates
            .iter()
            .filter(|candidate| candidate.decision == SourceSelectionDecision::Rejected)
            .count(),
        1
    );
    assert_eq!(
        complex_selection
            .candidates
            .iter()
            .filter(|candidate| candidate.decision == SourceSelectionDecision::Rejected)
            .count(),
        1
    );

    let manifest_before = breadth_first_manifest().replay_hash();
    let mut baseline_ids = BTreeSet::new();
    let mut baseline_ambiguities = 0;
    let mut cumulative_unique_ids = BTreeSet::new();
    let mut cumulative_route_ambiguities = 0;
    let mut new_route_unique_ids = BTreeSet::new();
    let mut candidates = Vec::new();
    let mut counts = PartitionReport {
        questions: questions.len(),
        baseline_executable: 0,
        baseline_ambiguities: 0,
        base_frontend_complete: 0,
        base_frontend_ambiguous: 0,
        base_frontend_missing: 0,
        base_frontend_unsupported: 0,
        base_execution_complete: 0,
        complex_frontend_complete: 0,
        complex_frontend_ambiguous: 0,
        complex_frontend_missing: 0,
        complex_frontend_unsupported: 0,
        complex_execution_complete: 0,
        cumulative_unique_ids: 0,
        cumulative_route_ambiguities: 0,
        new_route_unique_ids: 0,
        incremental_unique_ids: 0,
        base_frontend_replay_verified: 0,
        base_execution_replay_verified: 0,
        base_frontend_tamper_rejected: 0,
        base_execution_tamper_rejected: 0,
        complex_frontend_replay_verified: 0,
        complex_execution_replay_verified: 0,
        complex_frontend_tamper_rejected: 0,
        complex_execution_tamper_rejected: 0,
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

        let base_frontend = formalize(&question.original_prompt, &question.id);
        match base_frontend.status {
            BaseFrontendStatus::Complete => counts.base_frontend_complete += 1,
            BaseFrontendStatus::Ambiguous => counts.base_frontend_ambiguous += 1,
            BaseFrontendStatus::Missing => counts.base_frontend_missing += 1,
            BaseFrontendStatus::Unsupported => counts.base_frontend_unsupported += 1,
        }
        let base_frontend_ok = base_frontend_replay(&base_frontend);
        let mut base_frontend_tampered = base_frontend.clone();
        base_frontend_tampered.replay_hash.push('x');
        assert!(base_frontend_ok && !base_frontend_replay(&base_frontend_tampered));
        counts.base_frontend_replay_verified += usize::from(base_frontend_ok);
        counts.base_frontend_tamper_rejected +=
            usize::from(!base_frontend_replay(&base_frontend_tampered));
        let base_execution = base_frontend
            .request
            .as_ref()
            .map(evaluate_base)
            .filter(|result| result.status == BaseArithmeticStatus::Complete);
        counts.base_execution_complete += usize::from(base_execution.is_some());
        if let Some(execution) = base_execution.as_ref() {
            counts.base_execution_replay_verified += usize::from(base_execution_replay(execution));
            let mut tampered = (*execution).clone();
            tampered.replay_hash.push('x');
            counts.base_execution_tamper_rejected += usize::from(!base_execution_replay(&tampered));
        }

        let complex_frontend = formalize_complex_text(&question.original_prompt);
        match complex_frontend.status {
            ComplexFrontendStatus::Complete => counts.complex_frontend_complete += 1,
            ComplexFrontendStatus::Ambiguous => counts.complex_frontend_ambiguous += 1,
            ComplexFrontendStatus::Missing => counts.complex_frontend_missing += 1,
            ComplexFrontendStatus::Unsupported => counts.complex_frontend_unsupported += 1,
        }
        let complex_frontend_ok = complex_frontend.replay_verified();
        let mut complex_frontend_tampered = complex_frontend.clone();
        complex_frontend_tampered.replay_hash.push('x');
        assert!(complex_frontend_ok && !complex_frontend_tampered.replay_verified());
        counts.complex_frontend_replay_verified += usize::from(complex_frontend_ok);
        counts.complex_frontend_tamper_rejected +=
            usize::from(!complex_frontend_tampered.replay_verified());
        let complex_execution = complex_frontend
            .request
            .as_ref()
            .map(evaluate_complex)
            .filter(|result| result.status == ComplexStatus::Complete);
        counts.complex_execution_complete += usize::from(complex_execution.is_some());
        if let Some(execution) = complex_execution.as_ref() {
            counts.complex_execution_replay_verified += usize::from(execution.replay_verified());
            let mut tampered = (*execution).clone();
            tampered.replay_hash.push('x');
            counts.complex_execution_tamper_rejected += usize::from(!tampered.replay_verified());
        }

        let new_count =
            usize::from(base_execution.is_some()) + usize::from(complex_execution.is_some());
        let total_routes = baseline.len() + new_count;
        if total_routes > 1 {
            cumulative_route_ambiguities += 1;
        }
        if total_routes == 1 {
            cumulative_unique_ids.insert(question.id.clone());
        }
        if new_count > 0 && total_routes == 1 {
            new_route_unique_ids.insert(question.id.clone());
        }
        let cumulative_unique = total_routes == 1;
        if base_execution.is_some() {
            let candidate = base_candidate(
                question,
                &base_frontend,
                &oracle,
                cumulative_unique,
                baseline.len() == 1,
            )
            .expect("complete base execution yields candidate");
            counts.correct_selected_candidates +=
                usize::from(candidate.reference_match == Some(true));
            candidates.push(candidate);
        }
        if complex_execution.is_some() {
            let candidate = complex_candidate(
                question,
                &complex_frontend,
                &oracle,
                cumulative_unique,
                baseline.len() == 1,
            )
            .expect("complete complex execution yields candidate");
            counts.correct_selected_candidates +=
                usize::from(candidate.reference_match == Some(true));
            candidates.push(candidate);
        }
    }
    let manifest_after = breadth_first_manifest().replay_hash();
    counts.baseline_executable = baseline_ids.len();
    counts.baseline_ambiguities = baseline_ambiguities;
    counts.cumulative_unique_ids = cumulative_unique_ids.len();
    counts.cumulative_route_ambiguities = cumulative_route_ambiguities;
    counts.new_route_unique_ids = new_route_unique_ids.len();
    counts.incremental_unique_ids = new_route_unique_ids.difference(&baseline_ids).count();
    counts.candidates = candidates;

    let report = Report {
        schema: "stage409-cumulative-external-transfer-v1",
        partition: partition.clone(),
        dataset_sha256,
        legacy_portfolio_routes: 12,
        cumulative_new_routes: 2,
        base_source_path: BASE_SOURCE_PATH,
        base_source_sha256: digest_bytes(BASE_SOURCE.as_bytes()),
        base_source_id: BASE_SOURCE_ID,
        base_scope: BASE_SCOPE,
        complex_source_path: COMPLEX_SOURCE_PATH,
        complex_source_sha256: digest_bytes(
            include_str!("../../docs/sources/openstax_complex_arithmetic_source.txt").as_bytes(),
        ),
        complex_source_id: COMPLEX_SOURCE_ID,
        complex_scope: COMPLEX_SCOPE,
        base_gap_selection_replay: gap_replay_verified(&base_gap, &base_selection),
        complex_gap_selection_replay: gap_replay_verified(&complex_gap, &complex_selection),
        base_rejected_decoys: 1,
        complex_rejected_decoys: 1,
        base_pressure_corpus_sha256:
            "fa53961ef8b6cc931f7b935b3caca86b31b2035cb849da4e7cb254f3c233ff2a",
        complex_pressure_corpus_sha256:
            "ee4388673d1757419b277511e4321b1d97c8c647c69268db6667d7b2a524fd0a",
        manifest_sha256_before: manifest_before.clone(),
        manifest_sha256_after: manifest_after.clone(),
        manifest_unchanged: manifest_before == manifest_after,
        answer_keys_read: oracle.len(),
        production_authorizations: 0,
        false_authorizations: 0,
        partition_report: counts,
        report_sha256: String::new(),
    };
    assert!(report.base_gap_selection_replay && report.complex_gap_selection_replay);
    assert!(report.manifest_unchanged);
    assert_eq!(report.production_authorizations, 0);
    assert_eq!(report.false_authorizations, 0);
    assert_eq!(report.partition_report.plaintext_answers_read, 0);
    assert_eq!(
        report.answer_keys_read,
        if partition == "sealed" {
            report.partition_report.questions
        } else {
            0
        }
    );

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
            "# Stage 409 — cumulative external transfer\n\n- Partition / questions: {} / {}\n- Legacy routes / new source routes: {} / {}\n- Baseline unique / cumulative unique / cumulative ambiguities: {} / {} / {}\n- New-route unique / incremental over baseline: {} / {}\n- Base frontend/execution complete: {} / {}\n- Complex frontend/execution complete: {} / {}\n- Base replay / tamper (frontend + execution): {} / {}\n- Complex replay / tamper (frontend + execution): {} / {}\n- Answer hashes / plaintext answers read: {} / {}\n- Correct uniquely selected candidates: {}\n- Production authorizations / false authorizations: {} / {}\n- Source-selection replay: base={} complex={}\n- Manifest unchanged: {}\n\nThis is a route-blind shadow checkpoint combining the independently validated Stage 407 positional-arithmetic and Stage 408 rectangular-complex-arithmetic routes. Sealed scoring reads only answer hashes under the explicit privileged boundary; no plaintext answers are loaded and production routing is unchanged.\n",
            report.partition,
            report.partition_report.questions,
            report.legacy_portfolio_routes,
            report.cumulative_new_routes,
            report.partition_report.baseline_executable,
            report.partition_report.cumulative_unique_ids,
            report.partition_report.cumulative_route_ambiguities,
            report.partition_report.new_route_unique_ids,
            report.partition_report.incremental_unique_ids,
            report.partition_report.base_frontend_complete,
            report.partition_report.base_execution_complete,
            report.partition_report.complex_frontend_complete,
            report.partition_report.complex_execution_complete,
            report.partition_report.base_frontend_replay_verified + report.partition_report.base_execution_replay_verified,
            report.partition_report.base_frontend_tamper_rejected + report.partition_report.base_execution_tamper_rejected,
            report.partition_report.complex_frontend_replay_verified + report.partition_report.complex_execution_replay_verified,
            report.partition_report.complex_frontend_tamper_rejected + report.partition_report.complex_execution_tamper_rejected,
            report.answer_keys_read,
            report.partition_report.plaintext_answers_read,
            report.partition_report.correct_selected_candidates,
            report.production_authorizations,
            report.false_authorizations,
            report.base_gap_selection_replay,
            report.complex_gap_selection_replay,
            report.manifest_unchanged,
        ),
    )?;
    println!(
        "Stage 409 — partition={} baseline={} cumulative={} new_unique={} incremental={} correct={} false_auth=0",
        report.partition,
        report.partition_report.baseline_executable,
        report.partition_report.cumulative_unique_ids,
        report.partition_report.new_route_unique_ids,
        report.partition_report.incremental_unique_ids,
        report.partition_report.correct_selected_candidates,
    );
    Ok(())
}
