//! Independent validation and answer-key-blind external probe for finite-list
//! mean grounding.
//!
//! The bridge lowers only an explicitly enumerated numeric list to the already
//! validated `arithmetic_mean` source record.  Ranges, filtering, variables,
//! and optimization statements remain outside this contract.  The external
//! probe reads development question text only; it never reads an oracle or
//! authorizes a candidate.

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::fs;
use the_machine::curriculum::breadth_first_manifest;
use the_machine::probability_pack::Rational;
use the_machine::source_statistics_pack::{
    evaluate_statistics, records, source_statistics_frontend::formalize_finite_list_mean_text,
    source_statistics_frontend::FrontendStatus,
};
use the_machine::source_formula_pack::FormulaStatus;

const QUESTIONS: &str = "data/external_math_exam_v1/questions.jsonl";
const REPORT_JSON: &str = "docs/goal6_external_list_mean_frontend.json";
const REPORT_MD: &str = "docs/goal6_external_list_mean_frontend.md";

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
enum ExpectedStatus {
    Supported,
    Ambiguous,
    Unsupported,
}

#[derive(Debug, Clone, Serialize)]
struct IndependentCase {
    id: String,
    text: String,
    expected: ExpectedStatus,
    expected_value: Option<Rational>,
}

#[derive(Debug, Deserialize)]
struct Question {
    id: String,
    original_prompt: String,
}

#[derive(Debug, Serialize)]
struct ExternalCandidate {
    id: String,
    status: String,
    candidate_value: Option<Rational>,
    replay_verified: bool,
}

#[derive(Debug, Serialize)]
struct Report {
    schema: &'static str,
    source_domain: &'static str,
    independent_cases: usize,
    independent_supported: usize,
    independent_ambiguous: usize,
    independent_unsupported: usize,
    independent_exact_decisions: usize,
    independent_supported_values: usize,
    independent_frontend_replays: usize,
    independent_tamper_rejections: usize,
    external_questions_read: usize,
    external_answer_keys_read: usize,
    external_mean_signals: usize,
    external_complete_frontends: usize,
    external_executable_candidates: usize,
    external_candidate_replays: usize,
    external_candidates: Vec<ExternalCandidate>,
    production_authorizations: usize,
    false_authorizations: usize,
    manifest_sha256_before: String,
    manifest_sha256_after: String,
    manifest_unchanged: bool,
    corpus_sha256: String,
    report_sha256: String,
}

fn digest<T: Serialize>(value: &T) -> String {
    format!("{:x}", Sha256::digest(serde_json::to_vec(value).unwrap()))
}

fn q(numerator: i128, denominator: i128) -> Rational {
    Rational::new(numerator, denominator).unwrap()
}

fn independent_cases() -> Vec<IndependentCase> {
    let mut cases = Vec::new();
    for index in 0..80 {
        let a = index as i128 - 20;
        let b = (index as i128 % 13) - 6;
        let c = (index as i128 % 7) + 1;
        let d = (index as i128 % 5) - 2;
        let values = [a, b, c, d];
        let sum: i128 = values.iter().sum();
        let text = if index % 2 == 0 {
            format!(
                "Find the arithmetic mean of {{{}, {}, {}, {}}}.",
                a, b, c, d
            )
        } else {
            format!(
                "The scores are {}, {}, {}, and {}. What is their average?",
                a, b, c, d
            )
        };
        cases.push(IndependentCase {
            id: format!("list-supported-{index:03}"),
            text,
            expected: ExpectedStatus::Supported,
            expected_value: Some(q(sum, values.len() as i128)),
        });
    }
    for index in 0..20 {
        let text = match index % 3 {
            0 => "What is the arithmetic mean?".into(),
            1 => "Find the average of the listed values.".into(),
            _ => "The arithmetic mean is requested, but the values are not supplied.".into(),
        };
        cases.push(IndependentCase {
            id: format!("list-ambiguous-{index:03}"),
            text,
            expected: ExpectedStatus::Ambiguous,
            expected_value: None,
        });
    }
    for index in 0..20 {
        let text = match index % 4 {
            0 => "Find the arithmetic mean of the integers from -4 through 5.".into(),
            1 => "Find the arithmetic mean of the prime numbers in this list: 3, 5, 7.".into(),
            2 => "Find the mean of positive two-digit multiples of 7.".into(),
            _ => "The arithmetic mean of x + 8, 15, and 2x is 24.".into(),
        };
        cases.push(IndependentCase {
            id: format!("list-unsupported-{index:03}"),
            text,
            expected: ExpectedStatus::Unsupported,
            expected_value: None,
        });
    }
    cases
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let manifest_before = breadth_first_manifest().replay_hash();
    let _source_records = records();
    let cases = independent_cases();
    let mut independent_exact_decisions = 0;
    let mut independent_supported_values = 0;
    let mut independent_frontend_replays = 0;
    let mut independent_tamper_rejections = 0;
    for case in &cases {
        let result = formalize_finite_list_mean_text(&case.text);
        independent_frontend_replays += usize::from(result.replay_verified());
        let mut tampered = result.clone();
        tampered.replay_hash.push('x');
        independent_tamper_rejections += usize::from(!tampered.replay_verified());
        let observed = match result.status {
            FrontendStatus::Complete => ExpectedStatus::Supported,
            FrontendStatus::Ambiguous | FrontendStatus::Missing => ExpectedStatus::Ambiguous,
            FrontendStatus::Unsupported => ExpectedStatus::Unsupported,
        };
        if observed == case.expected {
            independent_exact_decisions += 1;
        }
        if observed == ExpectedStatus::Supported {
            let request = result.request.as_ref().expect("complete has request");
            let execution = evaluate_statistics(request);
            if execution.status == FormulaStatus::Complete
                && execution.value == case.expected_value
                && execution.replay_verified()
            {
                independent_supported_values += 1;
            }
        }
    }

    let questions = fs::read_to_string(QUESTIONS)?
        .lines()
        .filter(|line| !line.trim().is_empty())
        .map(serde_json::from_str::<Question>)
        .collect::<Result<Vec<_>, _>>()?
        .into_iter()
        .filter(|question| question.id.contains("development"))
        .collect::<Vec<_>>();
    let mut external_mean_signals = 0;
    let mut external_complete_frontends = 0;
    let mut external_executable_candidates = 0;
    let mut external_candidate_replays = 0;
    let mut external_candidates = Vec::new();
    for question in &questions {
        if question.original_prompt.to_ascii_lowercase().contains("mean")
            || question.original_prompt.to_ascii_lowercase().contains("average")
        {
            external_mean_signals += 1;
        }
        let result = formalize_finite_list_mean_text(&question.original_prompt);
        let replay_verified = result.replay_verified();
        if result.status != FrontendStatus::Complete {
            continue;
        }
        external_complete_frontends += 1;
        let request = result.request.as_ref().expect("complete has request");
        let execution = evaluate_statistics(request);
        if execution.status == FormulaStatus::Complete {
            external_executable_candidates += 1;
            external_candidate_replays += usize::from(execution.replay_verified());
            external_candidates.push(ExternalCandidate {
                id: question.id.clone(),
                status: "shadow_candidate".into(),
                candidate_value: execution.value,
                replay_verified,
            });
        }
    }
    let manifest_after = breadth_first_manifest().replay_hash();
    let mut report = Report {
        schema: "external-list-mean-frontend-v1",
        source_domain: "source_derived_finite_statistics",
        independent_cases: cases.len(),
        independent_supported: cases
            .iter()
            .filter(|case| case.expected == ExpectedStatus::Supported)
            .count(),
        independent_ambiguous: cases
            .iter()
            .filter(|case| case.expected == ExpectedStatus::Ambiguous)
            .count(),
        independent_unsupported: cases
            .iter()
            .filter(|case| case.expected == ExpectedStatus::Unsupported)
            .count(),
        independent_exact_decisions,
        independent_supported_values,
        independent_frontend_replays,
        independent_tamper_rejections,
        external_questions_read: questions.len(),
        external_answer_keys_read: 0,
        external_mean_signals,
        external_complete_frontends,
        external_executable_candidates,
        external_candidate_replays,
        external_candidates,
        production_authorizations: 0,
        false_authorizations: 0,
        manifest_sha256_before: manifest_before.clone(),
        manifest_sha256_after: manifest_after.clone(),
        manifest_unchanged: manifest_before == manifest_after,
        corpus_sha256: digest(&cases),
        report_sha256: String::new(),
    };
    let mut unsigned = serde_json::to_value(&report)?;
    unsigned["report_sha256"] = serde_json::Value::String(String::new());
    report.report_sha256 = digest(&unsigned);
    assert_eq!(report.independent_cases, 120);
    assert_eq!(report.independent_exact_decisions, 120);
    assert_eq!(report.independent_supported_values, 80);
    assert_eq!(report.independent_frontend_replays, 120);
    assert_eq!(report.independent_tamper_rejections, 120);
    assert_eq!(report.external_answer_keys_read, 0);
    assert_eq!(report.production_authorizations, 0);
    assert_eq!(report.false_authorizations, 0);
    assert!(report.manifest_unchanged);
    let serialized = serde_json::to_string_pretty(&report)?;
    fs::write(REPORT_JSON, format!("{serialized}\n"))?;
    fs::write(
        REPORT_MD,
        format!(
            "# Goal 6 — finite-list mean frontend\n\n\
             - Independent corpus: {} cases\n\
             - Independent exact / supported values: {} / {}\n\
             - Independent replay / tamper: {} / {}\n\
             - External development questions / mean signals: {} / {}\n\
             - External complete frontends / executable candidates: {} / {}\n\
             - External candidate replays: {}\n\
             - Answer keys read: {}\n\
             - Production authorizations / false authorizations: {} / {}\n\
             - Manifest unchanged: {}\n\n\
             External candidates are shadow-only and are not compared with\
             answer hashes during this acquisition run.\n",
            report.independent_cases,
            report.independent_exact_decisions,
            report.independent_supported_values,
            report.independent_frontend_replays,
            report.independent_tamper_rejections,
            report.external_questions_read,
            report.external_mean_signals,
            report.external_complete_frontends,
            report.external_executable_candidates,
            report.external_candidate_replays,
            report.external_answer_keys_read,
            report.production_authorizations,
            report.false_authorizations,
            report.manifest_unchanged,
        ),
    )?;
    println!("{serialized}");
    Ok(())
}
