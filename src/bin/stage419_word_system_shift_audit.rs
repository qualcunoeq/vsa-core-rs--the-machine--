//! Independent shifted-language audit for the frozen Stage 417 frontend.
//!
//! This is diagnostic only: it does not change the frontend, read external
//! answer keys, authorize production routes, or mutate the curriculum.

use serde::Serialize;
use sha2::{Digest, Sha256};
use std::fs;
use the_machine::curriculum::breadth_first_manifest;
use the_machine::source_word_system_frontend::{
    execute_word_system, execution_replay_verified, formalize_two_number_system, replay_verified,
    WordSystemStatus,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
enum Expected {
    Complete,
    Ambiguous,
    Missing,
    Unsupported,
}

#[derive(Debug, Serialize)]
struct Report {
    schema: &'static str,
    cases: usize,
    expected_complete: usize,
    expected_ambiguous: usize,
    expected_missing: usize,
    expected_unsupported: usize,
    exact_statuses: usize,
    complete_executions: usize,
    frontend_replays: usize,
    frontend_tamper_rejections: usize,
    execution_replays: usize,
    execution_tamper_rejections: usize,
    false_authorizations: usize,
    false_denials: usize,
    registry_unchanged: bool,
    corpus_sha256: String,
    report_sha256: String,
}

fn digest<T: Serialize>(value: &T) -> String {
    format!("{:x}", Sha256::digest(serde_json::to_vec(value).unwrap()))
}

fn small_word(value: i128) -> String {
    const UNITS: [&str; 20] = [
        "zero",
        "one",
        "two",
        "three",
        "four",
        "five",
        "six",
        "seven",
        "eight",
        "nine",
        "ten",
        "eleven",
        "twelve",
        "thirteen",
        "fourteen",
        "fifteen",
        "sixteen",
        "seventeen",
        "eighteen",
        "nineteen",
    ];
    const TENS: [&str; 8] = [
        "twenty", "thirty", "forty", "fifty", "sixty", "seventy", "eighty", "ninety",
    ];
    if (0..20).contains(&value) {
        return UNITS[value as usize].into();
    }
    if (20..100).contains(&value) {
        let tens = value / 10;
        let unit = value % 10;
        return if unit == 0 {
            TENS[(tens - 2) as usize].into()
        } else {
            format!("{}-{}", TENS[(tens - 2) as usize], UNITS[unit as usize])
        };
    }
    value.to_string()
}

fn cases() -> Vec<(String, Expected)> {
    let mut cases = Vec::new();
    for index in 0..120_i128 {
        let sum = 20 + index;
        let offset = 1 + index % 9;
        let sum_text = if index % 3 == 0 {
            sum.to_string()
        } else {
            small_word(sum)
        };
        let offset_text = if index % 2 == 0 {
            offset.to_string()
        } else {
            small_word(offset)
        };
        let relation = match index % 4 {
            0 => format!("one number is {offset_text} less than the other"),
            1 => format!("one number is {offset_text} more than the other"),
            2 => format!("one number is {offset_text} less than twice the other"),
            _ => format!("one number is {offset_text} less than three times the other"),
        };
        let prefix = match index % 4 {
            0 => "In this exercise, ",
            1 => "Given the following statement: ",
            2 => "For two unknown integers, ",
            _ => "Use the information below. ",
        };
        cases.push((
            format!("{prefix}the sum of two numbers is {sum_text}. {relation}. Find the numbers."),
            Expected::Complete,
        ));
    }
    for index in 0..30 {
        cases.push((
            format!(
                "The sum of two numbers is {}. One number is an unknown amount less than the other.",
                30 + index
            ),
            Expected::Ambiguous,
        ));
    }
    for index in 0..30 {
        cases.push((
            format!(
                "The sum of two numbers is {}. One number is 4 related to the other.",
                40 + index
            ),
            Expected::Unsupported,
        ));
    }
    for index in 0..30 {
        cases.push((
            format!("The sum of two numbers is {}. Find the numbers.", 2 + index),
            Expected::Missing,
        ));
    }
    for index in 0..30 {
        cases.push((
            format!(
                "The sum of three numbers is {}. One number is 4 less than the other.",
                50 + index
            ),
            Expected::Unsupported,
        ));
    }
    cases
}

fn observed(status: WordSystemStatus) -> Expected {
    match status {
        WordSystemStatus::Complete => Expected::Complete,
        WordSystemStatus::Ambiguous => Expected::Ambiguous,
        WordSystemStatus::Missing => Expected::Missing,
        WordSystemStatus::Unsupported => Expected::Unsupported,
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let before = breadth_first_manifest().replay_hash();
    let cases = cases();
    let mut exact_statuses = 0;
    let mut complete_executions = 0;
    let mut frontend_replays = 0;
    let mut frontend_tamper_rejections = 0;
    let mut execution_replays = 0;
    let mut execution_tamper_rejections = 0;
    let mut false_authorizations = 0;
    let mut false_denials = 0;
    for (index, (text, expected)) in cases.iter().enumerate() {
        let result = formalize_two_number_system(text, &format!("shift-{index}"));
        let observed = observed(result.status);
        exact_statuses += usize::from(observed == *expected);
        let frontend_ok = replay_verified(&result);
        frontend_replays += usize::from(frontend_ok);
        let mut tampered = result.clone();
        tampered.replay_hash.push('x');
        frontend_tamper_rejections += usize::from(!replay_verified(&tampered));
        let execution = execute_word_system(&result);
        complete_executions += usize::from(execution.is_some());
        if observed == Expected::Complete && execution.is_none() {
            false_denials += 1;
        }
        if observed != Expected::Complete && execution.is_some() {
            false_authorizations += 1;
        }
        if let Some(receipt) = execution {
            execution_replays += usize::from(execution_replay_verified(&receipt));
            let mut tampered = receipt.clone();
            tampered.result.push('x');
            execution_tamper_rejections += usize::from(!execution_replay_verified(&tampered));
        }
    }
    let after = breadth_first_manifest().replay_hash();
    let mut report = Report {
        schema: "stage419-word-system-shift-audit-v1",
        cases: cases.len(),
        expected_complete: cases
            .iter()
            .filter(|(_, e)| *e == Expected::Complete)
            .count(),
        expected_ambiguous: cases
            .iter()
            .filter(|(_, e)| *e == Expected::Ambiguous)
            .count(),
        expected_missing: cases
            .iter()
            .filter(|(_, e)| *e == Expected::Missing)
            .count(),
        expected_unsupported: cases
            .iter()
            .filter(|(_, e)| *e == Expected::Unsupported)
            .count(),
        exact_statuses,
        complete_executions,
        frontend_replays,
        frontend_tamper_rejections,
        execution_replays,
        execution_tamper_rejections,
        false_authorizations,
        false_denials,
        registry_unchanged: before == after,
        corpus_sha256: digest(&cases),
        report_sha256: String::new(),
    };
    report.report_sha256 = digest(&report);
    assert_eq!(report.cases, 240);
    assert_eq!(report.expected_complete, 120);
    assert_eq!(report.expected_ambiguous, 30);
    assert_eq!(report.expected_missing, 30);
    assert_eq!(report.expected_unsupported, 60);
    assert_eq!(report.exact_statuses, report.cases);
    assert_eq!(report.frontend_replays, report.cases);
    assert_eq!(report.frontend_tamper_rejections, report.cases);
    assert_eq!(report.complete_executions, report.expected_complete);
    assert_eq!(report.execution_replays, report.complete_executions);
    assert_eq!(
        report.execution_tamper_rejections,
        report.complete_executions
    );
    assert_eq!(report.false_authorizations, 0);
    assert_eq!(report.false_denials, 0);
    assert!(report.registry_unchanged);
    fs::write(
        "docs/stage419_word_system_shift_audit.json",
        format!("{}\n", serde_json::to_string_pretty(&report)?),
    )?;
    fs::write(
        "docs/stage419_word_system_shift_audit.md",
        format!(
            "# Stage 419 — shifted-language audit\n\n- cases / expected complete / ambiguous / missing / unsupported: {} / {} / {} / {} / {}\n- exact statuses: {}/{}\n- frontend replay / tamper rejection: {} / {}\n- complete executions / execution replay / tamper: {} / {} / {}\n- false authorizations / denials: {} / {}\n- registry unchanged: {}\n\nThis freezes the Stage 417 implementation and measures independently authored paraphrase, lexical, incomplete, and unsupported forms before any frontend broadening.\n",
            report.cases,
            report.expected_complete,
            report.expected_ambiguous,
            report.expected_missing,
            report.expected_unsupported,
            report.exact_statuses,
            report.cases,
            report.frontend_replays,
            report.frontend_tamper_rejections,
            report.complete_executions,
            report.execution_replays,
            report.execution_tamper_rejections,
            report.false_authorizations,
            report.false_denials,
            report.registry_unchanged,
        ),
    )?;
    println!(
        "Stage 419 — exact={}/{} complete={} false_auth={} false_denials={}",
        report.exact_statuses,
        report.cases,
        report.complete_executions,
        report.false_authorizations,
        report.false_denials
    );
    Ok(())
}
