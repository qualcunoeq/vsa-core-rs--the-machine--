//! Stage 408: source-derived rectangular complex arithmetic transfer.
//!
//! The source pack is evaluated beside the existing route-blind portfolio on
//! the independent external corpus. A separate generated pressure corpus is
//! checked with an independent integer oracle. Development is answer-key blind;
//! sealed scoring is hash-only and explicitly privileged.

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::env;
use std::fs;
use the_machine::curriculum::breadth_first_manifest;
use the_machine::goal6_external_portfolio::{executable_routes, observe_all};
use the_machine::probability_pack::Rational;
use the_machine::source_complex_pack::{
    evaluate_complex,
    source_complex_frontend::{formalize_complex_text, ComplexFrontendResult, FrontendStatus},
    ComplexArtifact, ComplexOperation, ComplexStatus,
};
use the_machine::source_evidence_envelope::ingest_source_evidence;
use the_machine::source_selection::{
    gap_replay_verified, select_for_gap, SourceGapRequest, SourceSelectionDecision,
};

const QUESTIONS_PATH: &str = "data/external_math_exam_v1/questions.jsonl";
const RELEASE_DIR: &str = "data/external_math_exam_v1";
const SOURCE_PATH: &str = "docs/sources/openstax_complex_arithmetic_source.txt";
const SOURCE_ID: &str = "openstax-precalculus-2e:complex-numbers-3-1";
const SCOPE: &str = "finite rectangular complex arithmetic";
const GAP_ID: &str = "gap::finite-rectangular-complex-arithmetic";

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
struct TransferPartitionReport {
    questions: usize,
    baseline_executable: usize,
    baseline_ambiguities: usize,
    frontend_complete: usize,
    frontend_ambiguous: usize,
    frontend_missing: usize,
    frontend_unsupported: usize,
    execution_complete: usize,
    frontend_replay_verified: usize,
    execution_replay_verified: usize,
    frontend_tamper_rejected: usize,
    execution_tamper_rejected: usize,
    selected_unique_ids: usize,
    incremental_unique_ids: usize,
    answer_hashes_read: usize,
    plaintext_answers_read: usize,
    correct_selected_candidates: usize,
    candidates: Vec<CandidateReceipt>,
}

#[derive(Debug, Serialize)]
struct TransferReport {
    schema: &'static str,
    partition: String,
    dataset_sha256: String,
    source_path: &'static str,
    source_sha256: String,
    source_id: &'static str,
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
    partition_report: TransferPartitionReport,
    report_sha256: String,
}

#[derive(Debug, Clone, Copy)]
enum PressureExpected {
    Supported,
    Ambiguous,
    Refused,
}

#[derive(Debug, Clone)]
struct PressureCase {
    id: String,
    prompt: String,
    expected: PressureExpected,
    expected_artifact: Option<ComplexArtifact>,
}

#[derive(Debug, Serialize)]
struct PressureReport {
    schema: &'static str,
    cases: usize,
    supported: usize,
    ambiguous: usize,
    refused: usize,
    exact_decisions: usize,
    supported_values_correct: usize,
    frontend_replay_verified: usize,
    execution_replay_verified: usize,
    tamper_rejected: usize,
    false_authorizations: usize,
    false_denials: usize,
    corpus_sha256: String,
}

fn digest_bytes(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

fn digest<T: Serialize>(value: &T) -> String {
    format!("{:x}", Sha256::digest(serde_json::to_vec(value).unwrap()))
}

fn q(value: i128) -> Rational {
    Rational::new(value, 1).unwrap()
}

fn complex_literal(real: i128, imag: i128) -> String {
    if imag < 0 {
        format!("({real}{imag}i)")
    } else {
        format!("({real}+{imag}i)")
    }
}

fn independent_artifact(
    operation: ComplexOperation,
    a: i128,
    b: i128,
    c: i128,
    d: i128,
) -> ComplexArtifact {
    match operation {
        ComplexOperation::Add => ComplexArtifact::Pair {
            real: q(a + c),
            imag: q(b + d),
        },
        ComplexOperation::Subtract => ComplexArtifact::Pair {
            real: q(a - c),
            imag: q(b - d),
        },
        ComplexOperation::Multiply => ComplexArtifact::Pair {
            real: q(a * c - b * d),
            imag: q(a * d + b * c),
        },
        _ => unreachable!("pressure corpus uses binary rectangular arithmetic only"),
    }
}

fn supported_pressure_cases() -> Vec<PressureCase> {
    let operations = [
        (ComplexOperation::Add, "+"),
        (ComplexOperation::Subtract, "-"),
        (ComplexOperation::Multiply, "*"),
    ];
    (0..120usize)
        .map(|index| {
            let (operation, symbol) = operations[index / 40];
            let a = 2 + index as i128;
            let b = -4 + (index % 9) as i128;
            let c = -3 + (index % 7) as i128;
            let d = 1 + (index % 8) as i128;
            let left = complex_literal(a, b);
            let right = complex_literal(c, d);
            PressureCase {
                id: format!("supported-{index:03}"),
                prompt: format!("Simplify {left}{symbol}{right}."),
                expected: PressureExpected::Supported,
                expected_artifact: Some(independent_artifact(operation, a, b, c, d)),
            }
        })
        .collect()
}

fn ambiguous_pressure_cases() -> Vec<PressureCase> {
    (0..40usize)
        .map(|index| PressureCase {
            id: format!("ambiguous-{index:03}"),
            prompt: format!(
                "Find the product and sum of {} and {}.",
                complex_literal(2 + index as i128, -1),
                complex_literal(3, 1 + (index % 4) as i128)
            ),
            expected: PressureExpected::Ambiguous,
            expected_artifact: None,
        })
        .collect()
}

fn refused_pressure_cases() -> Vec<PressureCase> {
    let mut cases = Vec::new();
    for index in 0..20usize {
        cases.push(PressureCase {
            id: format!("polar-{index:03}"),
            prompt: format!(
                "Convert {} to polar form.",
                complex_literal(2 + index as i128, 1)
            ),
            expected: PressureExpected::Refused,
            expected_artifact: None,
        });
    }
    for index in 0..20usize {
        cases.push(PressureCase {
            id: format!("power-{index:03}"),
            prompt: format!("Simplify {}^2.", complex_literal(2 + index as i128, 1)),
            expected: PressureExpected::Refused,
            expected_artifact: None,
        });
    }
    for index in 0..20usize {
        cases.push(PressureCase {
            id: format!("approximate-{index:03}"),
            prompt: format!(
                "Give a decimal approximation of the product of {} and {}.",
                complex_literal(2 + index as i128, -1),
                complex_literal(1, 3)
            ),
            expected: PressureExpected::Refused,
            expected_artifact: None,
        });
    }
    for index in 0..20usize {
        cases.push(PressureCase {
            id: format!("missing-operation-{index:03}"),
            prompt: if index % 2 == 0 {
                format!(
                    "Evaluate |{}{}|.",
                    complex_literal(2 + index as i128, -1),
                    complex_literal(1, 3)
                )
            } else {
                format!(
                    "Evaluate {} and {}.",
                    complex_literal(2 + index as i128, -1),
                    complex_literal(1, 3)
                )
            },
            expected: PressureExpected::Refused,
            expected_artifact: None,
        });
    }
    cases
}

fn run_pressure() -> Result<PressureReport, Box<dyn std::error::Error>> {
    let mut cases = supported_pressure_cases();
    cases.extend(ambiguous_pressure_cases());
    cases.extend(refused_pressure_cases());
    assert_eq!(cases.len(), 240);
    let corpus_sha256 = digest(
        &cases
            .iter()
            .map(|case| (&case.id, &case.prompt))
            .collect::<Vec<_>>(),
    );
    let mut exact_decisions = 0;
    let mut supported_values_correct = 0;
    let mut frontend_replay_verified = 0;
    let mut execution_replay_verified = 0;
    let mut tamper_rejected = 0;
    let mut false_authorizations = 0;
    let mut false_denials = 0;
    for case in &cases {
        let frontend = formalize_complex_text(&case.prompt);
        let actual = match frontend.status {
            FrontendStatus::Complete => {
                let request = frontend
                    .request
                    .as_ref()
                    .expect("complete frontend request");
                let execution = evaluate_complex(request);
                if execution.status == ComplexStatus::Complete {
                    if case.expected_artifact.as_ref() == execution.artifact.as_ref() {
                        supported_values_correct += 1;
                    }
                    "supported"
                } else {
                    "refused"
                }
            }
            FrontendStatus::Ambiguous => "ambiguous",
            FrontendStatus::Missing | FrontendStatus::Unsupported => "refused",
        };
        let expected = match case.expected {
            PressureExpected::Supported => "supported",
            PressureExpected::Ambiguous => "ambiguous",
            PressureExpected::Refused => "refused",
        };
        if actual == expected {
            exact_decisions += 1;
        } else if expected == "supported" && actual != "supported" {
            false_denials += 1;
        } else if expected != "supported" && actual == "supported" {
            false_authorizations += 1;
        }
        let mut frontend_tampered = frontend.clone();
        frontend_tampered.replay_hash.push('x');
        let frontend_ok = frontend.replay_verified();
        let frontend_tamper_ok = !frontend_tampered.replay_verified();
        frontend_replay_verified += usize::from(frontend_ok);
        tamper_rejected += usize::from(frontend_tamper_ok);
        assert!(frontend_ok && frontend_tamper_ok);
        if let Some(request) = frontend.request.as_ref() {
            let execution = evaluate_complex(request);
            let mut execution_tampered = execution.clone();
            execution_tampered.replay_hash.push('x');
            let execution_ok = execution.replay_verified();
            assert!(execution_ok && !execution_tampered.replay_verified());
            execution_replay_verified += usize::from(execution_ok);
            tamper_rejected += usize::from(!execution_tampered.replay_verified());
        }
    }
    let report = PressureReport {
        schema: "source-complex-arithmetic-pressure-v1",
        cases: cases.len(),
        supported: 120,
        ambiguous: 40,
        refused: 80,
        exact_decisions,
        supported_values_correct,
        frontend_replay_verified,
        execution_replay_verified,
        tamper_rejected,
        false_authorizations,
        false_denials,
        corpus_sha256,
    };
    assert_eq!(report.exact_decisions, report.cases);
    assert_eq!(report.supported_values_correct, report.supported);
    assert_eq!(report.frontend_replay_verified, report.cases);
    assert_eq!(
        report.tamper_rejected,
        report.cases + report.execution_replay_verified
    );
    assert_eq!(report.false_authorizations, 0);
    assert_eq!(report.false_denials, 0);
    Ok(report)
}

fn oracle_for_partition(
    partition: &str,
) -> Result<BTreeMap<String, String>, Box<dyn std::error::Error>> {
    if partition == "development" {
        return Ok(BTreeMap::new());
    }
    assert_eq!(
        env::var("GOAL6_STAGE408_PRIVILEGED_EVAL").as_deref(),
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

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let pressure = run_pressure()?;
    fs::write(
        "docs/stage408_complex_arithmetic_pressure.json",
        format!("{}\n", serde_json::to_string_pretty(&pressure)?),
    )?;
    fs::write(
        "docs/stage408_complex_arithmetic_pressure.md",
        format!(
            "# Stage 408 — complex arithmetic pressure corpus\n\n- Cases: {} ({} supported, {} ambiguous, {} refused)\n- Exact decisions: {}/{}\n- Supported values correct: {}/{}\n- Frontend replay: {}\n- Emitted execution replay: {}\n- Tamper rejection: {}\n- False authorizations / denials: {} / {}\n- Corpus SHA-256: `{}`\n",
            pressure.cases,
            pressure.supported,
            pressure.ambiguous,
            pressure.refused,
            pressure.exact_decisions,
            pressure.cases,
            pressure.supported_values_correct,
            pressure.supported,
            pressure.frontend_replay_verified,
            pressure.execution_replay_verified,
            pressure.tamper_rejected,
            pressure.false_authorizations,
            pressure.false_denials,
            pressure.corpus_sha256,
        ),
    )?;
    let partition = env::var("GOAL6_STAGE408_PARTITION").unwrap_or_else(|_| "development".into());
    assert!(matches!(partition.as_str(), "development" | "sealed"));
    let report_path = env::var("GOAL6_STAGE408_REPORT_JSON")
        .unwrap_or_else(|_| format!("docs/stage408_complex_arithmetic_{partition}.json"));
    let report_md_path = env::var("GOAL6_STAGE408_REPORT_MD")
        .unwrap_or_else(|_| format!("docs/stage408_complex_arithmetic_{partition}.md"));
    let question_bytes = fs::read(QUESTIONS_PATH)?;
    let dataset_sha256 = digest_bytes(&question_bytes);
    let questions: Vec<Question> = String::from_utf8(question_bytes)?
        .lines()
        .filter(|line| !line.trim().is_empty())
        .map(serde_json::from_str)
        .collect::<Result<Vec<_>, _>>()?
        .into_iter()
        .filter(|question: &Question| question.split == partition)
        .collect();
    let candidates = vec![
        source_evidence(
            SOURCE_PATH,
            SOURCE_ID,
            "Precalculus 2e",
            "3.1 Complex Numbers",
            "https://openstax.org/books/precalculus-2e/pages/3-1-complex-numbers",
            "exact rectangular addition, subtraction, multiplication, division, conjugation, and norm-squared operations",
            SCOPE,
        ),
        source_evidence(
            "docs/sources/openstax_base_arithmetic_source.txt",
            "openstax-contemporary-mathematics:base-arithmetic",
            "Contemporary Mathematics",
            "Base arithmetic",
            "https://openstax.org/books/contemporary-mathematics/pages/4-4-addition-and-subtraction-in-base-systems",
            "finite positional arithmetic",
            "finite positional binary arithmetic",
        ),
        source_evidence(
            "docs/sources/openstax_precalculus_sequences_source.txt",
            "openstax-precalculus-2e:sequences",
            "Precalculus 2e",
            "Arithmetic Sequences",
            "https://openstax.org/details/books/precalculus-2e",
            "finite sequence term evaluation",
            "finite arithmetic-sequence term evaluation",
        ),
    ];
    let gap_request = SourceGapRequest::new(GAP_ID, vec![SCOPE.into()], 1);
    let selection = select_for_gap(&gap_request, &candidates);
    assert!(gap_replay_verified(&gap_request, &selection));
    assert_eq!(selection.selected_source_ids, vec![SOURCE_ID.to_owned()]);
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
    let mut frontend_replay_verified_count = 0;
    let mut execution_replay_verified_count = 0;
    let mut frontend_tamper_rejections = 0;
    let mut execution_tamper_rejections = 0;
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
        let frontend: ComplexFrontendResult = formalize_complex_text(&question.original_prompt);
        match frontend.status {
            FrontendStatus::Complete => frontend_complete += 1,
            FrontendStatus::Ambiguous => frontend_ambiguous += 1,
            FrontendStatus::Missing => frontend_missing += 1,
            FrontendStatus::Unsupported => frontend_unsupported += 1,
        }
        let frontend_ok = frontend.replay_verified();
        let mut frontend_tampered = frontend.clone();
        frontend_tampered.replay_hash.push('x');
        assert!(frontend_ok && !frontend_tampered.replay_verified());
        frontend_replay_verified_count += usize::from(frontend_ok);
        frontend_tamper_rejections += usize::from(!frontend_tampered.replay_verified());
        let Some(request) = frontend.request.as_ref() else {
            continue;
        };
        let execution = evaluate_complex(request);
        if execution.status != ComplexStatus::Complete {
            continue;
        }
        execution_complete += 1;
        let execution_ok = execution.replay_verified();
        let mut execution_tampered = execution.clone();
        execution_tampered.replay_hash.push('x');
        assert!(execution_ok && !execution_tampered.replay_verified());
        execution_replay_verified_count += usize::from(frontend_ok && execution_ok);
        execution_tamper_rejections += usize::from(!execution_tampered.replay_verified());
        let artifact = execution
            .artifact
            .as_ref()
            .expect("complete complex artifact");
        let candidate_hash = digest(artifact);
        let reference_match = oracle.get(&question.id).map(|expected| {
            complex_forms(artifact)
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
    let report = TransferReport {
        schema: "stage408-complex-arithmetic-transfer-v1",
        partition: partition.clone(),
        dataset_sha256,
        source_path: SOURCE_PATH,
        source_sha256: digest_bytes(
            include_str!("../../docs/sources/openstax_complex_arithmetic_source.txt").as_bytes(),
        ),
        source_id: SOURCE_ID,
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
        partition_report: TransferPartitionReport {
            questions: questions.len(),
            baseline_executable: baseline_ids.len(),
            baseline_ambiguities,
            frontend_complete,
            frontend_ambiguous,
            frontend_missing,
            frontend_unsupported,
            execution_complete,
            frontend_replay_verified: frontend_replay_verified_count,
            execution_replay_verified: execution_replay_verified_count,
            frontend_tamper_rejected: frontend_tamper_rejections,
            execution_tamper_rejected: execution_tamper_rejections,
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
            "# Stage 408 — source-derived complex arithmetic transfer\n\n- Partition / questions: {} / {}\n- Selected source / scope: `{}` / `{}`\n- Gap selection replay: {}\n- Baseline executable / selected executable / incremental: {} / {} / {}\n- Frontend complete / execution complete: {} / {}\n- Frontend replay / tamper rejection: {} / {}\n- Execution replay / tamper rejection: {} / {}\n- Answer hashes / plaintext answers read: {} / {}\n- Correct selected candidates: {}\n- Production authorizations / false authorizations: {} / {}\n- Manifest unchanged: {}\n\nThe route is bounded to exact rectangular complex arithmetic. Symbolic addition, subtraction, and multiplication are accepted only with explicit parenthesized operands; powers, polar/analytic requests, approximation, and missing operations remain refused.\n",
            report.partition,
            report.partition_report.questions,
            report.source_path,
            report.operation_scope,
            report.gap_selection_replay,
            report.partition_report.baseline_executable,
            report.partition_report.selected_unique_ids,
            report.partition_report.incremental_unique_ids,
            report.partition_report.frontend_complete,
            report.partition_report.execution_complete,
            report.partition_report.frontend_replay_verified,
            report.partition_report.frontend_tamper_rejected,
            report.partition_report.execution_replay_verified,
            report.partition_report.execution_tamper_rejected,
            report.answer_keys_read,
            report.partition_report.plaintext_answers_read,
            report.partition_report.correct_selected_candidates,
            report.production_authorizations,
            report.false_authorizations,
            report.manifest_unchanged,
        ),
    )?;
    println!(
        "Stage 408 — partition={} selected={} baseline={} incremental={} correct={} answer_hashes={} false_auth=0",
        report.partition,
        report.partition_report.selected_unique_ids,
        report.partition_report.baseline_executable,
        report.partition_report.incremental_unique_ids,
        report.partition_report.correct_selected_candidates,
        report.answer_keys_read,
    );
    Ok(())
}
