//! Independent pressure corpus for source-derived category-constrained counting.
//!
//! The corpus is generated from the source contract, not from HLE answers. It
//! contains explicit supported cases plus ambiguity and out-of-scope cases.

use serde::Serialize;
use sha2::{Digest, Sha256};
use std::fs;
use the_machine::source_category_selection_frontend::{
    formalize_category_selection_text, replay_verified as frontend_replay,
    CategorySelectionFrontendStatus,
};
use the_machine::source_category_selection_pack::{
    evaluate_category_selection, replay_verified as execution_replay, CategorySelectionStatus,
};

const REPORT_JSON: &str = "docs/stage400_external_category_selection_bench.json";
const REPORT_MD: &str = "docs/stage400_external_category_selection_bench.md";

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
enum Expected {
    Supported,
    Ambiguous,
    Refused,
}

#[derive(Debug, Serialize)]
struct Receipt {
    id: String,
    prompt_sha256: String,
    expected: Expected,
    frontend_status: String,
    execution_status: Option<String>,
    expected_count: Option<u128>,
    actual_count: Option<u128>,
    exact_decision: bool,
    value_correct: bool,
    frontend_replay: bool,
    execution_replay: bool,
    frontend_tamper_rejected: bool,
    execution_tamper_rejected: bool,
    false_authorization: bool,
    false_denial: bool,
}

#[derive(Debug, Serialize)]
struct Report {
    schema: &'static str,
    source_sha256: String,
    corpus_sha256: String,
    cases: usize,
    supported: usize,
    ambiguous: usize,
    refused: usize,
    exact_decisions: usize,
    authorized_answers: usize,
    supported_values: usize,
    frontend_replays: usize,
    execution_replays: usize,
    frontend_tamper_rejections: usize,
    execution_tamper_rejections: usize,
    false_authorizations: usize,
    false_denials: usize,
    receipts: Vec<Receipt>,
}

#[derive(Debug, Clone)]
struct Case {
    id: String,
    prompt: String,
    expected: Expected,
    expected_count: Option<u128>,
}

fn hash_bytes(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

fn hash_text(text: &str) -> String {
    hash_bytes(text.as_bytes())
}

fn choose(n: u64, r: u64) -> u128 {
    let r = r.min(n - r);
    (0..r).fold(1_u128, |value, index| {
        value * (n - index) as u128 / (index + 1) as u128
    })
}

fn expected_count(category_count: u64, choices_per_category: u64, selected: u64) -> u128 {
    choose(category_count, selected) * (choices_per_category as u128).pow(selected as u32)
}

fn supported_cases() -> Vec<Case> {
    let values = [
        (4, 3, 1),
        (4, 5, 2),
        (4, 13, 3),
        (5, 2, 2),
        (5, 7, 4),
        (6, 3, 2),
        (6, 4, 5),
        (7, 2, 3),
        (8, 5, 2),
        (9, 3, 4),
    ];
    (0..120)
        .map(|index| {
            let (categories, choices, selected) = values[index % values.len()];
            let prompt = match index % 4 {
                0 => format!(
                    "Choose {selected} categories from {categories} categories, with {choices} choices per category, one from each selected category; order does not matter."
                ),
                1 => format!(
                    "Select {selected} groups out of {categories} groups; each group has {choices} options. Take one from each selected group and order does not matter."
                ),
                2 => format!(
                    "Pick {selected} types from {categories} types, with {choices} items per category; one from each, unordered."
                ),
                _ => format!(
                    "Selecting {selected} suits from {categories} suits, each suit has {choices} choices, one from each; order does not matter."
                ),
            };
            Case {
                id: format!("supported_{index:03}"),
                prompt,
                expected: Expected::Supported,
                expected_count: Some(expected_count(categories, choices, selected)),
            }
        })
        .collect()
}

fn ambiguous_cases() -> Vec<Case> {
    (0..40)
        .map(|index| {
            let categories = 4 + (index % 4);
            let choices = 3 + (index % 5);
            let selected = 2 + (index % 3);
            let prompt = match index % 4 {
                0 => format!(
                    "Choose {selected} categories from {categories} categories, with {choices} choices per category, one from each selected category."
                ),
                1 => format!(
                    "Choose {selected} categories from {categories} categories, with {choices} choices per category; order matters or does not matter."
                ),
                2 => format!(
                    "Choose {selected} categories from {categories} categories, with {choices} choices per category, but the target object is not specified."
                ),
                _ => format!(
                    "Choose {selected} categories from {categories} categories, with {choices} choices per category, one from each selected category."
                ),
            };
            Case {
                id: format!("ambiguous_{index:03}"),
                prompt,
                expected: Expected::Ambiguous,
                expected_count: None,
            }
        })
        .collect()
}

fn refused_cases() -> Vec<Case> {
    (0..80)
        .map(|index| {
            let categories = 4 + (index % 5);
            let choices = 3 + (index % 6);
            let selected = if index % 8 == 6 {
                categories + 1
            } else {
                2 + (index % 3)
            };
            let prompt = match index % 8 {
                0 => format!(
                    "Choose at least {selected} categories from {categories} categories, with {choices} choices per category, one from each; order does not matter."
                ),
                1 => format!(
                    "Choose {selected} categories from {categories} unequal groups, with {choices} choices per category, one from each; order does not matter."
                ),
                2 => format!(
                    "Choose {selected} categories from {categories} categories, with {choices} choices per category, multiple from each category; order does not matter."
                ),
                3 => format!(
                    "Choose {selected} categories from {categories} categories with replacement, with {choices} choices per category, one from each; order does not matter."
                ),
                4 => format!(
                    "Choose {selected} categories from {categories} categories, with {choices} choices per category, one from each; what is the probability, order does not matter?"
                ),
                5 => format!(
                    "Choose {selected} categories from infinitely many categories, with {choices} choices per category, one from each; order does not matter."
                ),
                6 => format!(
                    "Choose {selected} categories from {categories} categories, with {choices} choices per category, one from each; order does not matter."
                ),
                _ => format!(
                    "Choose {selected} categories from {categories} categories, with {choices} choices per category, one from each; order is not specified."
                ),
            };
            let expected = if index % 8 == 6 {
                Expected::Refused
            } else if index % 8 == 7 {
                Expected::Ambiguous
            } else {
                Expected::Refused
            };
            Case {
                id: format!("refused_{index:03}"),
                prompt,
                expected,
                expected_count: None,
            }
        })
        .collect()
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut cases = supported_cases();
    cases.extend(ambiguous_cases());
    cases.extend(refused_cases());
    assert_eq!(cases.len(), 240);
    let corpus_bytes =
        serde_json::to_vec(&cases.iter().map(|case| &case.prompt).collect::<Vec<_>>())?;
    let source_sha256 = hash_text(the_machine::source_category_selection_pack::SOURCE);
    let corpus_sha256 = hash_bytes(&corpus_bytes);
    let mut receipts = Vec::with_capacity(cases.len());
    let mut exact_decisions = 0;
    let mut authorized_answers = 0;
    let mut supported_values = 0;
    let mut frontend_replays = 0;
    let mut execution_replays = 0;
    let mut frontend_tamper_rejections = 0;
    let mut execution_tamper_rejections = 0;
    let mut false_authorizations = 0;
    let mut false_denials = 0;

    for case in cases {
        let frontend = formalize_category_selection_text(&case.prompt, &case.id);
        let mut frontend_tampered = frontend.clone();
        frontend_tampered.replay_hash.push('x');
        let frontend_replay_verified = frontend_replay(&frontend);
        let frontend_tamper_rejected = !frontend_replay(&frontend_tampered);
        frontend_replays += usize::from(frontend_replay_verified);
        frontend_tamper_rejections += usize::from(frontend_tamper_rejected);
        let (execution_status, actual_count, execution_replay_verified, execution_tamper_rejected) =
            if let Some(request) = frontend.request.as_ref() {
                let execution = evaluate_category_selection(request);
                let mut tampered = execution.clone();
                tampered.replay_hash.push('x');
                let replay = execution_replay(&execution);
                let tamper = !execution_replay(&tampered);
                execution_replays += usize::from(replay);
                execution_tamper_rejections += usize::from(tamper);
                (
                    Some(format!("{:?}", execution.status)),
                    execution.count,
                    replay,
                    tamper,
                )
            } else {
                (None, None, false, false)
            };
        let frontend_complete = frontend.status == CategorySelectionFrontendStatus::Complete;
        let execution_complete = execution_status.as_deref() == Some("Complete");
        let exact_decision = match case.expected {
            Expected::Supported => frontend_complete && execution_complete,
            Expected::Ambiguous => frontend.status == CategorySelectionFrontendStatus::Ambiguous,
            Expected::Refused => {
                frontend.status == CategorySelectionFrontendStatus::Unsupported
                    || (frontend_complete && !execution_complete)
            }
        };
        let value_correct = case.expected == Expected::Supported
            && actual_count == case.expected_count
            && execution_complete;
        let false_authorization = case.expected != Expected::Supported && execution_complete;
        let false_denial = case.expected == Expected::Supported && !value_correct;
        exact_decisions += usize::from(exact_decision);
        authorized_answers +=
            usize::from(execution_complete && case.expected == Expected::Supported);
        supported_values += usize::from(value_correct);
        false_authorizations += usize::from(false_authorization);
        false_denials += usize::from(false_denial);
        receipts.push(Receipt {
            id: case.id,
            prompt_sha256: hash_text(&case.prompt),
            expected: case.expected,
            frontend_status: format!("{:?}", frontend.status),
            execution_status,
            expected_count: case.expected_count,
            actual_count,
            exact_decision,
            value_correct,
            frontend_replay: frontend_replay_verified,
            execution_replay: execution_replay_verified,
            frontend_tamper_rejected,
            execution_tamper_rejected,
            false_authorization,
            false_denial,
        });
    }
    let report = Report {
        schema: "stage400-external-category-selection-bench-v1",
        source_sha256,
        corpus_sha256,
        cases: receipts.len(),
        supported: 120,
        ambiguous: 40,
        refused: 80,
        exact_decisions,
        authorized_answers,
        supported_values,
        frontend_replays,
        execution_replays,
        frontend_tamper_rejections,
        execution_tamper_rejections,
        false_authorizations,
        false_denials,
        receipts,
    };
    assert_eq!(report.exact_decisions, report.cases);
    assert_eq!(report.authorized_answers, report.supported);
    assert_eq!(report.supported_values, report.supported);
    assert_eq!(report.frontend_replays, report.cases);
    assert_eq!(report.frontend_tamper_rejections, report.cases);
    assert_eq!(report.false_authorizations, 0);
    assert_eq!(report.false_denials, 0);
    let serialized = serde_json::to_string_pretty(&report)?;
    fs::write(REPORT_JSON, format!("{serialized}\n"))?;
    fs::write(
        REPORT_MD,
        format!(
            "# Stage 400 — external source-derived distinct-category selection\n\n- Cases: {}\n- Supported / ambiguous / refused: {} / {} / {}\n- Exact decisions: {}/{}\n- Authorized supported values: {}/{}\n- Frontend replay / tamper rejection: {}/{}\n- Emitted execution replay / tamper rejection: {}/{}\n- False authorizations / denials: {} / {}\n\nThe corpus is source-derived and independent of HLE answer keys. The rule remains shadow-only and requires explicit equal category sizes, one item per selected category, and unordered category selection.\n",
            report.cases,
            report.supported,
            report.ambiguous,
            report.refused,
            report.exact_decisions,
            report.cases,
            report.authorized_answers,
            report.supported,
            report.frontend_replays,
            report.frontend_tamper_rejections,
            report.execution_replays,
            report.execution_tamper_rejections,
            report.false_authorizations,
            report.false_denials,
        ),
    )?;
    println!("{serialized}");
    Ok(())
}
