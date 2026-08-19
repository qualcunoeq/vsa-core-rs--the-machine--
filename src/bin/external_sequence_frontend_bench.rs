//! Answer-key-blind validation of the natural-language arithmetic-sequence
//! frontend against an independent corpus, followed by a text-only scan of
//! the public MATH development partition.  This binary never reads an oracle
//! and never authorizes or mutates a production route.

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::fs;
use the_machine::source_formula_pack::{
    evaluate_formula_records, source_formula_records, validate_formula_records, FormulaStatus,
};
use the_machine::source_sequence_frontend::{
    formalize_sequence_terms_text, replay_verified, SequenceFrontendStatus,
};
use the_machine::probability_pack::Rational;

const QUESTIONS: &str = "data/external_math_exam_v1/questions.jsonl";
const DOMAIN: &str = "external-source-sequence-shadow";
const REPORT_JSON: &str = "docs/goal6_external_sequence_frontend.json";
const REPORT_MD: &str = "docs/goal6_external_sequence_frontend.md";

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
    split: String,
}

#[derive(Debug, Serialize)]
struct ExternalCandidate {
    id: String,
    status: String,
    value: Option<Rational>,
    replay_verified: bool,
}

#[derive(Debug, Serialize)]
struct Report {
    schema: &'static str,
    source_domain: &'static str,
    source_catalog_sha256: String,
    independent_cases: usize,
    independent_supported: usize,
    independent_ambiguous: usize,
    independent_unsupported: usize,
    independent_exact_decisions: usize,
    independent_supported_values: usize,
    independent_frontend_replays: usize,
    independent_execution_replays: usize,
    independent_frontend_tamper_rejections: usize,
    independent_execution_tamper_rejections: usize,
    independent_false_authorizations: usize,
    independent_false_denials: usize,
    external_questions_read: usize,
    external_answer_keys_read: usize,
    external_sequence_signals: usize,
    external_complete_frontends: usize,
    external_executable_candidates: usize,
    external_candidate_replays: usize,
    external_candidates: Vec<ExternalCandidate>,
    production_authorizations: usize,
    false_authorizations: usize,
    manifest_unchanged: bool,
    report_sha256: String,
}

fn digest<T: Serialize>(value: &T) -> String {
    format!("{:x}", Sha256::digest(serde_json::to_vec(value).unwrap()))
}

fn q(numerator: i128) -> Rational {
    Rational::new(numerator, 1).unwrap()
}

fn expected_term(a1: i128, d: i128, n: i128) -> Rational {
    q(a1).add(&q(n - 1).mul(&q(d)).unwrap()).unwrap()
}

fn independent_cases() -> Vec<IndependentCase> {
    let mut cases = Vec::new();
    for index in 0..80 {
        let a1 = index as i128 - 17;
        let d = (index as i128 % 11) - 5;
        let n = 2 + (index as i128 % 47);
        let text = match index % 4 {
            0 => format!(
                "The first three terms of an arithmetic sequence are {}, {} and {}, respectively. Find the {}th term.",
                a1,
                a1 + d,
                a1 + 2 * d,
                n
            ),
            1 => format!(
                "Consider the arithmetic sequence {}, {}, {}, {}, ... . What is the {}th term?",
                a1,
                a1 + d,
                a1 + 2 * d,
                a1 + 3 * d,
                n
            ),
            2 => format!(
                "The first and thirteenth terms of an arithmetic sequence are {} and {}. What is the {}th term?",
                a1,
                a1 + 12 * d,
                n
            ),
            _ => format!(
                "For the arithmetic sequence {}, {}, {}, ... determine the value of the {}th term.",
                a1,
                a1 + d,
                a1 + 2 * d,
                n
            ),
        };
        cases.push(IndependentCase {
            id: format!("sequence-supported-{index:03}"),
            text,
            expected: ExpectedStatus::Supported,
            expected_value: Some(expected_term(a1, d, n)),
        });
    }
    for index in 0..20 {
        let text = match index % 3 {
            0 => "The first three terms of an arithmetic sequence are 1, 4 and 7. Find the term.".into(),
            1 => "The first three terms of an arithmetic sequence are 1, 5 and 7. Find the 10th term.".into(),
            _ => "An arithmetic sequence has first term 3 and common difference 2. Find a term.".into(),
        };
        cases.push(IndependentCase {
            id: format!("sequence-ambiguous-{index:03}"),
            text,
            expected: ExpectedStatus::Ambiguous,
            expected_value: None,
        });
    }
    for index in 0..20 {
        let text = match index % 4 {
            0 => "Determine whether the infinite geometric series converges.".into(),
            1 => "A geometric sequence has first term 2 and ratio 3. Find its 8th term.".into(),
            2 => "Find the sum of the first 10 terms of an arithmetic sequence.".into(),
            _ => "A recurrence defines the sequence. Determine its long-run limit.".into(),
        };
        cases.push(IndependentCase {
            id: format!("sequence-unsupported-{index:03}"),
            text,
            expected: ExpectedStatus::Unsupported,
            expected_value: None,
        });
    }
    cases
}

fn run_case(
    records: &[the_machine::source_formula_pack::FormulaRecord],
    case: &IndependentCase,
) -> (bool, bool, bool, bool, bool, bool, bool) {
    let frontend = formalize_sequence_terms_text(&case.text, &case.id, DOMAIN);
    let status_exact = match case.expected {
        ExpectedStatus::Supported => frontend.status == SequenceFrontendStatus::Complete,
        ExpectedStatus::Ambiguous => matches!(
            frontend.status,
            SequenceFrontendStatus::Ambiguous | SequenceFrontendStatus::Missing
        ),
        ExpectedStatus::Unsupported => frontend.status == SequenceFrontendStatus::Unsupported,
    };
    let frontend_replay = replay_verified(&frontend);
    let mut frontend_tampered = frontend.clone();
    frontend_tampered.replay_hash.push('x');
    let frontend_tamper = !replay_verified(&frontend_tampered);
    let mut execution_replay = false;
    let mut execution_tamper = false;
    let mut value_correct = false;
    let execution_ok = if let Some(request) = frontend.request.as_ref() {
        let result = evaluate_formula_records(request, DOMAIN, records);
        execution_replay = result.replay_verified();
        let mut tampered = result.clone();
        tampered.replay_hash.push('x');
        execution_tamper = !tampered.replay_verified();
        value_correct = case.expected == ExpectedStatus::Supported
            && result.status == FormulaStatus::Complete
            && result.value == case.expected_value;
        case.expected == ExpectedStatus::Supported && value_correct
    } else {
        case.expected != ExpectedStatus::Supported
    };
    let false_authorization = case.expected != ExpectedStatus::Supported
        && frontend.status == SequenceFrontendStatus::Complete
        && execution_ok;
    let false_denial = case.expected == ExpectedStatus::Supported && !execution_ok;
    (
        status_exact && execution_ok,
        frontend_replay,
        execution_replay,
        frontend_tamper,
        execution_tamper,
        value_correct,
        false_authorization || false_denial,
    )
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let records = source_formula_records();
    validate_formula_records(&records).map_err(|errors| errors.join("; "))?;
    let cases = independent_cases();
    assert_eq!(cases.len(), 120);
    let mut exact = 0;
    let mut values = 0;
    let mut frontend_replays = 0;
    let mut execution_replays = 0;
    let mut frontend_tamper = 0;
    let mut execution_tamper = 0;
    let mut errors = 0;
    for case in &cases {
        let result = run_case(&records, case);
        exact += usize::from(result.0);
        frontend_replays += usize::from(result.1);
        execution_replays += usize::from(result.2);
        frontend_tamper += usize::from(result.3);
        execution_tamper += usize::from(result.4);
        values += usize::from(result.5);
        errors += usize::from(result.6);
    }
    let supported = cases
        .iter()
        .filter(|case| case.expected == ExpectedStatus::Supported)
        .count();
    let ambiguous = cases
        .iter()
        .filter(|case| case.expected == ExpectedStatus::Ambiguous)
        .count();
    let unsupported = cases
        .iter()
        .filter(|case| case.expected == ExpectedStatus::Unsupported)
        .count();
    assert_eq!((supported, ambiguous, unsupported), (80, 20, 20));
    assert_eq!(exact, 120);
    assert_eq!(values, 80);
    assert_eq!(frontend_replays, 120);
    assert_eq!(execution_replays, 80);
    assert_eq!(frontend_tamper, 120);
    assert_eq!(execution_tamper, 80);
    assert_eq!(errors, 0);

    let mut external_questions_read = 0;
    let mut external_sequence_signals = 0;
    let mut external_complete_frontends = 0;
    let mut external_executable_candidates = 0;
    let mut external_candidate_replays = 0;
    let mut external_candidates = Vec::new();
    for line in fs::read_to_string(QUESTIONS)?
        .lines()
        .filter(|line| line.contains("\"split\": \"development\""))
    {
        let question: Question = serde_json::from_str(line)?;
        if question.split != "development" {
            continue;
        }
        external_questions_read += 1;
        let lower = question.original_prompt.to_ascii_lowercase();
        if lower.contains("arithmetic sequence") {
            external_sequence_signals += 1;
        }
        let frontend = formalize_sequence_terms_text(
            &question.original_prompt,
            &question.id,
            DOMAIN,
        );
        if frontend.status == SequenceFrontendStatus::Complete {
            external_complete_frontends += 1;
            if let Some(request) = frontend.request.as_ref() {
                let result = evaluate_formula_records(request, DOMAIN, &records);
                if result.status == FormulaStatus::Complete {
                    external_executable_candidates += 1;
                    let candidate_replay = replay_verified(&frontend) && result.replay_verified();
                    external_candidate_replays += usize::from(candidate_replay);
                    external_candidates.push(ExternalCandidate {
                        id: question.id,
                        status: format!("{:?}", result.status),
                        value: result.value.clone(),
                        replay_verified: candidate_replay,
                    });
                }
            }
        }
    }
    let mut report = Report {
        schema: "goal6-external-sequence-frontend-v1",
        source_domain: DOMAIN,
        source_catalog_sha256: digest(&records),
        independent_cases: cases.len(),
        independent_supported: supported,
        independent_ambiguous: ambiguous,
        independent_unsupported: unsupported,
        independent_exact_decisions: exact,
        independent_supported_values: values,
        independent_frontend_replays: frontend_replays,
        independent_execution_replays: execution_replays,
        independent_frontend_tamper_rejections: frontend_tamper,
        independent_execution_tamper_rejections: execution_tamper,
        independent_false_authorizations: 0,
        independent_false_denials: 0,
        external_questions_read,
        external_answer_keys_read: 0,
        external_sequence_signals,
        external_complete_frontends,
        external_executable_candidates,
        external_candidate_replays,
        external_candidates,
        production_authorizations: 0,
        false_authorizations: 0,
        manifest_unchanged: true,
        report_sha256: String::new(),
    };
    report.report_sha256 = digest(&report);
    fs::write(REPORT_JSON, format!("{}\n", serde_json::to_string_pretty(&report)?))?;
    fs::write(
        REPORT_MD,
        format!(
            "# Goal 6 — external arithmetic-sequence frontend\n\n- Independent cases: {}/{} exact; supported values {}/{}; frontend replay/tamper {}/{}, execution replay/tamper {}/{}\n- Independent false authorizations / denials: 0 / 0\n- External questions read / answer keys read: {} / 0\n- Sequence signals / complete frontends / executable candidates: {} / {} / {}\n- External candidate replay: {}\n- Production authorizations / false authorizations: 0 / 0\n- Manifest unchanged: true\n- Source catalog SHA-256: `{}`\n",
            report.independent_exact_decisions,
            report.independent_cases,
            report.independent_supported_values,
            report.independent_supported,
            report.independent_frontend_replays,
            report.independent_frontend_tamper_rejections,
            report.independent_execution_replays,
            report.independent_execution_tamper_rejections,
            report.external_questions_read,
            report.external_sequence_signals,
            report.external_complete_frontends,
            report.external_executable_candidates,
            report.external_candidate_replays,
            report.source_catalog_sha256,
        ),
    )?;
    Ok(())
}
