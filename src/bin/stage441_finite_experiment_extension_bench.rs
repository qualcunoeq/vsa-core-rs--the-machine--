//! Independent shifted validation for the finite-experiment extension.

use serde::Serialize;
use sha2::{Digest, Sha256};
use std::fs;
use the_machine::probability_pack::Rational;
use the_machine::source_finite_experiment_frontend::{
    execute, execution_replay_verified, formalize, replay_verified, DieCondition, FrontendStatus,
    JointPredicate,
};

const REPORT_JSON: &str = "docs/stage441_finite_experiment_extension_bench.json";
const REPORT_MD: &str = "docs/stage441_finite_experiment_extension_bench.md";
const SOURCE: &str = include_str!("../source_finite_experiment_frontend.rs");

#[derive(Clone)]
struct Case {
    text: String,
    expected: FrontendStatus,
    oracle: Option<Rational>,
}

#[derive(Serialize)]
struct Report {
    schema: &'static str,
    cases: usize,
    supported: usize,
    ambiguous: usize,
    unsupported: usize,
    exact_decisions: usize,
    supported_values: usize,
    incorrect_values: usize,
    frontend_replays: usize,
    frontend_tamper_rejections: usize,
    execution_replays: usize,
    execution_tamper_rejections: usize,
    false_authorizations: usize,
    false_denials: usize,
    source_sha256: String,
    corpus_sha256: String,
    report_sha256: String,
}

fn sha(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

fn prime(value: i64) -> bool {
    value >= 2 && (2..=((value as f64).sqrt() as i64)).all(|d| value % d != 0)
}

fn condition_matches(condition: &DieCondition, value: i64) -> bool {
    match condition {
        DieCondition::LessThan(bound) => value < *bound,
        DieCondition::GreaterThan(bound) => value > *bound,
        DieCondition::Equal(target) => value == *target,
        DieCondition::Even => value % 2 == 0,
        DieCondition::Odd => value % 2 != 0,
    }
}

fn joint_matches(predicate: &JointPredicate, left: i64, right: i64) -> bool {
    match predicate {
        JointPredicate::SumEquals(value) => left + right == *value,
        JointPredicate::SumLessThan(value) => left + right < *value,
        JointPredicate::SumGreaterThan(value) => left + right > *value,
        JointPredicate::AtLeastOneEquals(value) => left == *value || right == *value,
        JointPredicate::DifferenceEquals(value) => (left - right).abs() == *value,
        JointPredicate::ProductEven => (left * right) % 2 == 0,
        JointPredicate::ProductOdd => (left * right) % 2 != 0,
        JointPredicate::SumPrime => prime(left + right),
        JointPredicate::FirstAndSecond { first, second } => {
            condition_matches(first, left) && condition_matches(second, right)
        }
    }
}

fn oracle(sides: u32, predicate: JointPredicate) -> Rational {
    let count = (1..=sides as i64)
        .flat_map(|left| (1..=sides as i64).map(move |right| (left, right)))
        .filter(|(left, right)| joint_matches(&predicate, *left, *right))
        .count() as i128;
    Rational::new(count, (sides as i128) * (sides as i128)).unwrap()
}

fn cases() -> Vec<Case> {
    let mut cases = Vec::new();
    for index in 0..40 {
        let threshold = 2 + (index % 3) as i64;
        let greater = 4 + (index % 2) as i64;
        let predicate = JointPredicate::FirstAndSecond {
            first: DieCondition::LessThan(threshold),
            second: DieCondition::GreaterThan(greater),
        };
        cases.push(Case {
            text: format!(
                "Two fair six-sided dice are rolled. What is the probability that the first die is less than {threshold} and the second die is greater than {greater}?"
            ),
            expected: FrontendStatus::Complete,
            oracle: Some(oracle(6, predicate)),
        });
    }
    for index in 0..40 {
        let target = 9 + (index % 7) as i64;
        cases.push(Case {
            text: format!(
                "A pair of fair octahedral dice with faces labeled with digits 1 through 8 is rolled. What is the probability that their sum is {target}?"
            ),
            expected: FrontendStatus::Complete,
            oracle: Some(oracle(8, JointPredicate::SumEquals(target))),
        });
    }
    for index in 0..40 {
        let target = 5 + (index % 6) as i64;
        let predicate = if index % 2 == 0 {
            JointPredicate::SumEquals(target)
        } else {
            JointPredicate::SumPrime
        };
        let text = if index % 2 == 0 {
            format!(
                "Two standard 6-sided dice are tossed. What is the probability that their sum is {target}?"
            )
        } else {
            "Two uniform six-sided dice are rolled. What is the probability that their sum is prime?".into()
        };
        cases.push(Case {
            text,
            expected: FrontendStatus::Complete,
            oracle: Some(oracle(6, predicate)),
        });
    }
    for index in 0..20 {
        cases.push(Case {
            text: format!(
                "Two six-sided dice are rolled. What is the probability that their sum is {}?",
                5 + index
            ),
            expected: FrontendStatus::Ambiguous,
            oracle: None,
        });
    }
    for index in 0..20 {
        cases.push(Case {
            text: format!(
                "Two fair six-sided dice are rolled. What is the probability that their sum is 7 and their product is {}?",
                if index % 2 == 0 { "even" } else { "odd" }
            ),
            expected: FrontendStatus::Ambiguous,
            oracle: None,
        });
    }
    let unsupported = [
        "Three fair six-sided dice are rolled. What is the probability that their sum is 9?",
        "Two biased six-sided dice are rolled. What is the probability that their sum is 7?",
        "A fair six-sided die is rolled repeatedly. What is the probability of a six?",
        "Two fair six-sided dice are rolled without replacement. What is the probability their sum is 7?",
    ];
    for index in 0..80 {
        cases.push(Case {
            text: unsupported[index % unsupported.len()].into(),
            expected: FrontendStatus::Unsupported,
            oracle: None,
        });
    }
    cases
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cases = cases();
    let corpus = cases
        .iter()
        .map(|case| case.text.as_str())
        .collect::<Vec<_>>()
        .join("\n");
    let mut exact = 0;
    let mut values = 0;
    let mut incorrect = 0;
    let mut frontend_replays = 0;
    let mut frontend_tamper = 0;
    let mut execution_replays = 0;
    let mut execution_tamper = 0;
    let mut false_auth = 0;
    let mut false_denials = 0;
    for (index, case) in cases.iter().enumerate() {
        let result = formalize(&case.text, &format!("stage441-{index}"));
        exact += usize::from(result.status == case.expected);
        frontend_replays += usize::from(replay_verified(&result));
        let mut bad = result.clone();
        bad.replay_hash.push('x');
        frontend_tamper += usize::from(!replay_verified(&bad));
        let execution = execute(&result);
        if case.expected == FrontendStatus::Complete {
            let Some(execution) = execution else {
                false_denials += 1;
                continue;
            };
            if execution.value == case.oracle {
                values += 1;
            } else {
                incorrect += 1;
            }
            execution_replays += usize::from(execution_replay_verified(&execution));
            let mut execution_bad = execution.clone();
            execution_bad.replay_hash.push('x');
            execution_tamper += usize::from(!execution_replay_verified(&execution_bad));
        } else if execution.is_some() {
            false_auth += 1;
        }
    }
    let mut report = Report {
        schema: "stage441-finite-experiment-extension-v1",
        cases: cases.len(),
        supported: 120,
        ambiguous: 40,
        unsupported: 80,
        exact_decisions: exact,
        supported_values: values,
        incorrect_values: incorrect,
        frontend_replays,
        frontend_tamper_rejections: frontend_tamper,
        execution_replays,
        execution_tamper_rejections: execution_tamper,
        false_authorizations: false_auth,
        false_denials,
        source_sha256: sha(SOURCE.as_bytes()),
        corpus_sha256: sha(corpus.as_bytes()),
        report_sha256: String::new(),
    };
    report.report_sha256 = sha(&serde_json::to_vec(&report)?);
    assert_eq!(report.cases, 240);
    assert_eq!(report.exact_decisions, 240);
    assert_eq!(report.supported_values, 120);
    assert_eq!(report.incorrect_values, 0);
    assert_eq!(report.frontend_replays, 240);
    assert_eq!(report.frontend_tamper_rejections, 240);
    assert_eq!(report.execution_replays, 120);
    assert_eq!(report.execution_tamper_rejections, 120);
    assert_eq!(report.false_authorizations, 0);
    assert_eq!(report.false_denials, 0);
    fs::write(
        REPORT_JSON,
        format!("{}\n", serde_json::to_string_pretty(&report)?),
    )?;
    fs::write(
        REPORT_MD,
        format!(
            "# Stage 441 — finite-experiment extension benchmark\n\n- cases / supported / ambiguous / unsupported: {} / {} / {} / {}\n- exact decisions: {}/{}\n- supported values / incorrect values: {}/{}\n- frontend replay / tamper: {}/{} / {}/{}\n- execution replay / tamper: {}/{} / {}/{}\n- false authorizations / denials: {} / {}\n- source SHA-256: `{}`\n- corpus SHA-256: `{}`\n\nThe shifted corpus validates separate first/second die predicates and explicit labeled octahedral pairs while retaining fail-closed boundaries for loaded, repeated, replacement, and over-budget experiments.\n",
            report.cases,
            report.supported,
            report.ambiguous,
            report.unsupported,
            report.exact_decisions,
            report.cases,
            report.supported_values,
            report.incorrect_values,
            report.frontend_replays,
            report.cases,
            report.frontend_tamper_rejections,
            report.cases,
            report.execution_replays,
            report.supported,
            report.execution_tamper_rejections,
            report.supported,
            report.false_authorizations,
            report.false_denials,
            report.source_sha256,
            report.corpus_sha256,
        ),
    )?;
    println!("{}", serde_json::to_string_pretty(&report)?);
    Ok(())
}
