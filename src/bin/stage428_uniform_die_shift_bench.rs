//! Stage 428: shifted-language pressure test for the uniform-die frontend.
//!
//! The corpus uses independently authored paraphrase forms and explicit
//! boundary cases.  It exercises the frontend grammar, not the answer-key
//! path, and never mutates the curriculum manifest.

use serde::Serialize;
use sha2::{Digest, Sha256};
use the_machine::curriculum::breadth_first_manifest;
use the_machine::probability_pack::Rational;
use the_machine::uniform_die_frontend::{
    execute, execution_replay_verified, formalize, replay_verified, EventPredicate, FrontendStatus,
};

#[derive(Clone, Copy)]
enum Expected {
    Supported,
    Ambiguous,
    Unsupported,
}

struct Case {
    text: String,
    expected: Expected,
    value: Option<Rational>,
}

#[derive(Debug, Serialize)]
struct Report {
    schema: &'static str,
    cases: usize,
    supported_cases: usize,
    ambiguous_cases: usize,
    unsupported_cases: usize,
    exact_statuses: usize,
    supported_values: usize,
    incorrect_values: usize,
    frontend_replay_verified: usize,
    frontend_tamper_rejected: usize,
    execution_replay_verified: usize,
    execution_tamper_rejected: usize,
    false_authorizations: usize,
    false_denials: usize,
    manifest_unchanged: bool,
    corpus_sha256: String,
}

fn digest<T: Serialize>(value: &T) -> String {
    format!("{:x}", Sha256::digest(serde_json::to_vec(value).unwrap()))
}

fn expected_value(sides: u32, event: &EventPredicate) -> Rational {
    let count = (1..=sides as i64)
        .filter(|value| match event {
            EventPredicate::Negative => *value < 0,
            EventPredicate::LessThan(bound) => *value < *bound,
            EventPredicate::LessThanOrEqual(bound) => *value <= *bound,
            EventPredicate::Equal(target) => *value == *target,
            EventPredicate::NotEqual(target) => *value != *target,
            EventPredicate::Even => *value % 2 == 0,
            EventPredicate::Odd => *value % 2 != 0,
            EventPredicate::MultipleOf(divisor) => *value % *divisor == 0,
        })
        .count() as i128;
    Rational::new(count, sides as i128).unwrap()
}

fn supported_cases() -> Vec<Case> {
    let mut cases = Vec::new();
    for index in 0..120u32 {
        let sides = 6 + (index % 5) * 2;
        let threshold = 2 + (index % sides);
        let target = 1 + (index % sides);
        let (description, event) = match index % 8 {
            0 => (
                format!("a value below {threshold}"),
                EventPredicate::LessThan(threshold as i64),
            ),
            1 => (
                format!("a value no greater than {threshold}"),
                EventPredicate::LessThanOrEqual(threshold as i64),
            ),
            2 => (
                format!("the outcome equals {target}"),
                EventPredicate::Equal(target as i64),
            ),
            3 => (
                format!("anything except {target}"),
                EventPredicate::NotEqual(target as i64),
            ),
            4 => ("an even outcome".into(), EventPredicate::Even),
            5 => ("an odd result".into(), EventPredicate::Odd),
            6 => {
                let divisor = 2 + index % 3;
                (
                    format!("a value divisible by {divisor}"),
                    EventPredicate::MultipleOf(divisor as i64),
                )
            }
            _ => ("a negative outcome".into(), EventPredicate::Negative),
        };
        let experiment = match index % 8 {
            0 => format!("A fair {sides}-sided die is rolled; compute the chance of {description}."),
            1 => format!("A die with {sides} faces, each equally likely, is rolled. Find the probability of {description}."),
            2 => format!("The outcomes of a uniform {sides}-sided die are equally likely. What is the probability of {description}?"),
            3 => format!("Each of the {sides} outcomes of a fair die is equally likely. Find the probability of {description}."),
            4 => format!("Roll a standard die with {sides} faces. What is the probability of {description}?"),
            5 => format!("A uniform die with {sides} equally likely faces is used. Determine the probability of {description}."),
            6 => format!("For a fair die with {sides} faces, calculate the chance of {description}."),
            _ => format!("A standard {sides}-sided die is used. Determine the chance of {description}."),
        };
        cases.push(Case {
            text: experiment,
            expected: Expected::Supported,
            value: Some(expected_value(sides, &event)),
        });
    }
    cases
}

fn ambiguous_cases() -> Vec<Case> {
    (0..40)
        .map(|index| {
            let text = match index % 4 {
                0 => "A 6-sided die is rolled. What is the probability of an even outcome?".into(),
                1 => "A fair die is rolled. What is the probability of an even outcome?".into(),
                2 => "A standard 8-sided die is rolled. What is the probability of an even outcome or a value below 4?".into(),
                _ => "A die with 10 faces is rolled. What is the probability of an odd result?".into(),
            };
            Case {
                text,
                expected: Expected::Ambiguous,
                value: None,
            }
        })
        .collect()
}

fn unsupported_cases() -> Vec<Case> {
    (0..80)
        .map(|index| {
            let text = match index % 8 {
                0 => "Two fair 6-sided dice are rolled. Find the probability of a sum below 8.".into(),
                1 => "A fair 6-sided die has faces labeled A-F. Find the probability of an even result.".into(),
                2 => "What are the odds in favor of rolling a 4 on a standard 6-sided die?".into(),
                3 => "What is the empirical probability of rolling a 4 on a standard 6-sided die?".into(),
                4 => "A fair 6-sided die is rolled repeatedly. Find the probability of exactly two even results.".into(),
                5 => "Draw a face without replacement from a fair 6-sided die. Find the probability of a 4.".into(),
                6 => "A special 6-sided die is rolled. Find the probability of an even result.".into(),
                _ => "Two fair 8-sided dice are rolled. Find the probability of a sum no greater than 5.".into(),
            };
            Case {
                text,
                expected: Expected::Unsupported,
                value: None,
            }
        })
        .collect()
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut cases = supported_cases();
    cases.extend(ambiguous_cases());
    cases.extend(unsupported_cases());
    assert_eq!(cases.len(), 240);
    let corpus_sha256 = digest(&cases.iter().map(|case| &case.text).collect::<Vec<_>>());
    let before = breadth_first_manifest().replay_hash();
    let mut exact = 0;
    let mut supported_values = 0;
    let mut incorrect_values = 0;
    let mut frontend_replay = 0;
    let mut frontend_tamper = 0;
    let mut execution_replay = 0;
    let mut execution_tamper = 0;
    let mut false_authorizations = 0;
    let mut false_denials = 0;
    for (index, case) in cases.iter().enumerate() {
        let result = formalize(&case.text, &format!("stage428-{index:03}"));
        frontend_replay += usize::from(replay_verified(&result));
        let mut tampered = result.clone();
        tampered.replay_hash.push('x');
        frontend_tamper += usize::from(!replay_verified(&tampered));
        let expected_status = match case.expected {
            Expected::Supported => FrontendStatus::Complete,
            Expected::Ambiguous => FrontendStatus::Ambiguous,
            Expected::Unsupported => FrontendStatus::Unsupported,
        };
        if result.status == expected_status {
            exact += 1;
        } else if matches!(case.expected, Expected::Supported) {
            false_denials += 1;
        } else {
            false_authorizations += 1;
        }
        if matches!(case.expected, Expected::Supported) {
            let Some(request) = result.request.as_ref() else {
                incorrect_values += 1;
                continue;
            };
            let execution = execute(request);
            execution_replay += usize::from(execution_replay_verified(&execution));
            let mut execution_tampered = execution.clone();
            execution_tampered.replay_hash.push('x');
            execution_tamper += usize::from(!execution_replay_verified(&execution_tampered));
            if execution.status == FrontendStatus::Complete && execution.value == case.value {
                supported_values += 1;
            } else {
                incorrect_values += 1;
            }
        }
    }
    let after = breadth_first_manifest().replay_hash();
    assert_eq!(before, after);
    let report = Report {
        schema: "stage428-uniform-die-shift-bench-v1",
        cases: cases.len(),
        supported_cases: 120,
        ambiguous_cases: 40,
        unsupported_cases: 80,
        exact_statuses: exact,
        supported_values,
        incorrect_values,
        frontend_replay_verified: frontend_replay,
        frontend_tamper_rejected: frontend_tamper,
        execution_replay_verified: execution_replay,
        execution_tamper_rejected: execution_tamper,
        false_authorizations,
        false_denials,
        manifest_unchanged: before == after,
        corpus_sha256,
    };
    std::fs::write(
        "docs/stage428_uniform_die_shift_bench.json",
        serde_json::to_vec_pretty(&report)?,
    )?;
    std::fs::write(
        "docs/stage428_uniform_die_shift_bench.md",
        format!(
            "# Stage 428 — shifted uniform-die pressure benchmark\n\n- cases / supported / ambiguous / unsupported: 240 / 120 / 40 / 80\n- exact statuses: {}/240\n- supported values / incorrect values: {}/120 / {}\n- frontend replay / tamper: {}/240 / {}/240\n- execution replay / tamper: {}/120 / {}/120\n- false authorizations / false denials: {} / {}\n- manifest unchanged: {}\n- corpus SHA-256: `{}`\n",
            report.exact_statuses,
            report.supported_values,
            report.incorrect_values,
            report.frontend_replay_verified,
            report.frontend_tamper_rejected,
            report.execution_replay_verified,
            report.execution_tamper_rejected,
            report.false_authorizations,
            report.false_denials,
            report.manifest_unchanged,
            report.corpus_sha256,
        ),
    )?;
    println!(
        "Stage 428 — exact={}/240 values={}/120 replay={}/240 execution_replay={}/120 false_auth={}",
        report.exact_statuses,
        report.supported_values,
        report.frontend_replay_verified,
        report.execution_replay_verified,
        report.false_authorizations,
    );
    Ok(())
}
