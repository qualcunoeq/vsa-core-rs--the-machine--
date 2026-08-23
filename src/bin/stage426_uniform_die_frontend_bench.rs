//! Stage 426: independent validation of the bounded uniform-die frontend.

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
        let (description, event) = match index % 8 {
            0 => (
                format!("a number less than {}", 2 + (index % sides)),
                EventPredicate::LessThan((2 + index % sides) as i64),
            ),
            1 => (
                format!("a number less than or equal to {}", 1 + (index % sides)),
                EventPredicate::LessThanOrEqual((1 + index % sides) as i64),
            ),
            2 => (
                format!("an {}", 1 + (index % sides)),
                EventPredicate::Equal((1 + index % sides) as i64),
            ),
            3 => (
                format!("not rolling a {}", 1 + (index % sides)),
                EventPredicate::NotEqual((1 + index % sides) as i64),
            ),
            4 => ("an even number".into(), EventPredicate::Even),
            5 => ("an odd number".into(), EventPredicate::Odd),
            6 => (
                format!("a multiple of {}", 2 + index % 3),
                EventPredicate::MultipleOf((2 + index % 3) as i64),
            ),
            _ => ("a negative number".into(), EventPredicate::Negative),
        };
        let event_phrase = if index % 8 == 3 {
            description
        } else {
            format!("rolling {description}")
        };
        cases.push(Case {
            text: format!(
                "Compute the probability of {event_phrase} on a standard {sides}-sided die."
            ),
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
                0 => "What is the probability of rolling an even number on a 6-sided die?".into(),
                1 => "What is the probability of rolling an even number on a standard die?".into(),
                2 => "What is the probability of rolling a number less than 4 on a fair die?".into(),
                _ => "What is the probability of rolling an even number or a multiple of 3 on a standard 8-sided die?".into(),
            };
            Case { text, expected: Expected::Ambiguous, value: None }
        })
        .collect()
}

fn unsupported_cases() -> Vec<Case> {
    (0..80)
        .map(|index| {
            let text = match index % 5 {
                0 => "What is the probability that two dice have a sum less than 8?".into(),
                1 => "What is the probability of rolling an even number on a special labeled 6-sided die?".into(),
                2 => "What are the odds in favor of rolling a 4 on a standard 6-sided die?".into(),
                3 => "What is the empirical probability of rolling a 4 on a standard 6-sided die?".into(),
                _ => "What is the probability of rolling a 4 on a standard 6-sided die over 10 repeated rolls?".into(),
            };
            Case { text, expected: Expected::Unsupported, value: None }
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
    let mut supported = 0;
    let mut incorrect_values = 0;
    let mut frontend_replay = 0;
    let mut frontend_tamper = 0;
    let mut execution_replay = 0;
    let mut execution_tamper = 0;
    let mut false_authorizations = 0;
    let mut false_denials = 0;
    for (index, case) in cases.iter().enumerate() {
        let result = formalize(&case.text, &format!("stage426-{index:03}"));
        if replay_verified(&result) {
            frontend_replay += 1;
        }
        let mut tampered = result.clone();
        tampered.replay_hash.push('x');
        if !replay_verified(&tampered) {
            frontend_tamper += 1;
        }
        let expected_status = match case.expected {
            Expected::Supported => FrontendStatus::Complete,
            Expected::Ambiguous => FrontendStatus::Ambiguous,
            Expected::Unsupported => FrontendStatus::Unsupported,
        };
        if result.status == expected_status {
            exact += 1;
        } else {
            match case.expected {
                Expected::Supported => false_denials += 1,
                Expected::Ambiguous | Expected::Unsupported => false_authorizations += 1,
            }
        }
        if case.matches_expected(&result) {
            supported += usize::from(matches!(case.expected, Expected::Supported));
        }
        if matches!(case.expected, Expected::Supported) {
            let Some(request) = result.request.as_ref() else {
                incorrect_values += 1;
                continue;
            };
            let execution = execute(request);
            if execution_replay_verified(&execution) {
                execution_replay += 1;
            }
            let mut execution_tampered = execution.clone();
            execution_tampered.replay_hash.push('x');
            if !execution_replay_verified(&execution_tampered) {
                execution_tamper += 1;
            }
            if execution.status != FrontendStatus::Complete || execution.value != case.value {
                incorrect_values += 1;
            }
        }
    }
    let after = breadth_first_manifest().replay_hash();
    assert_eq!(before, after);
    let report = Report {
        schema: "stage426-uniform-die-frontend-bench-v1",
        cases: cases.len(),
        supported_cases: 120,
        ambiguous_cases: 40,
        unsupported_cases: 80,
        exact_statuses: exact,
        supported_values: supported,
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
        "docs/stage426_uniform_die_frontend_bench.json",
        serde_json::to_vec_pretty(&report)?,
    )?;
    std::fs::write(
        "docs/stage426_uniform_die_frontend_bench.md",
        format!(
            "# Stage 426 — bounded uniform-die frontend\n\n- cases / supported / ambiguous / unsupported: 240 / 120 / 40 / 80\n- exact statuses: {}/240\n- supported values / incorrect values: {}/120 / {}\n- frontend replay / tamper: {}/240 / {}/240\n- execution replay / tamper: {}/120 / {}/120\n- false authorizations / false denials: {} / {}\n- manifest unchanged: {}\n- corpus SHA-256: `{}`\n",
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
        "Stage 426 — exact={}/240 values={}/120 replay={}/240 execution_replay={}/120 false_auth={}",
        report.exact_statuses,
        report.supported_values,
        report.frontend_replay_verified,
        report.execution_replay_verified,
        report.false_authorizations,
    );
    Ok(())
}

impl Case {
    fn matches_expected(&self, result: &the_machine::uniform_die_frontend::FrontendResult) -> bool {
        matches!(self.expected, Expected::Supported) == (result.status == FrontendStatus::Complete)
    }
}
