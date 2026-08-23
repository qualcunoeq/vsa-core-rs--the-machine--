//! Independent validation for the bounded finite-experiment frontend.
//!
//! The corpus is generated from explicit semantic records rather than by
//! asking the frontend to label its own examples.  The oracle enumerates the
//! finite sample space independently of the production executor.

use serde::Serialize;
use sha2::{Digest, Sha256};
use std::fs;
use the_machine::probability_pack::Rational;
use the_machine::source_finite_experiment_frontend::{
    execute, execution_replay_verified, formalize, replay_verified, FrontendStatus, JointPredicate,
};
use the_machine::uniform_die_frontend::EventPredicate;

const REPORT_JSON: &str = "docs/stage439_finite_experiment_frontend_bench.json";
const REPORT_MD: &str = "docs/stage439_finite_experiment_frontend_bench.md";
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
    provenance_complete: usize,
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

fn one_matches(predicate: &EventPredicate, value: i64) -> bool {
    match predicate {
        EventPredicate::Negative => value < 0,
        EventPredicate::LessThan(bound) => value < *bound,
        EventPredicate::LessThanOrEqual(bound) => value <= *bound,
        EventPredicate::Equal(target) => value == *target,
        EventPredicate::NotEqual(target) => value != *target,
        EventPredicate::Even => value % 2 == 0,
        EventPredicate::Odd => value % 2 != 0,
        EventPredicate::MultipleOf(divisor) => value % divisor == 0,
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
        JointPredicate::FirstAndSecond { .. } => false,
    }
}

fn oracle_single(sides: u32, predicate: EventPredicate) -> Rational {
    let count = (1..=sides as i64)
        .filter(|value| one_matches(&predicate, *value))
        .count() as i128;
    Rational::new(count, sides as i128).unwrap()
}

fn oracle_joint(sides: u32, predicate: JointPredicate) -> Rational {
    let count = (1..=sides as i64)
        .flat_map(|left| (1..=sides as i64).map(move |right| (left, right)))
        .filter(|(left, right)| joint_matches(&predicate, *left, *right))
        .count() as i128;
    Rational::new(count, (sides as i128) * (sides as i128)).unwrap()
}

fn supported_cases() -> Vec<Case> {
    let mut cases = Vec::new();
    let sides = [4_u32, 6, 8, 10, 12];
    for index in 0..60 {
        let n = sides[index % sides.len()];
        let (text, predicate) = match index % 6 {
            0 => (
                format!("A fair {n}-sided die is rolled. What is the probability of an even outcome?"),
                EventPredicate::Even,
            ),
            1 => (
                format!("A standard {n}-sided die is rolled. What is the probability of an odd result?"),
                EventPredicate::Odd,
            ),
            2 => (
                format!("A uniform {n}-sided die is rolled. What is the probability of a result less than 3?"),
                EventPredicate::LessThan(3),
            ),
            3 => (
                format!("A fair {n}-sided die is rolled. What is the probability of rolling a {n}?"),
                EventPredicate::Equal(n as i64),
            ),
            4 => (
                format!("A standard {n}-sided die is rolled. What is the probability of a multiple of 2?"),
                EventPredicate::MultipleOf(2),
            ),
            _ => (
                format!("A fair {n}-sided die is rolled. What is the probability of a value less than or equal to 4?"),
                EventPredicate::LessThanOrEqual(4),
            ),
        };
        cases.push(Case {
            text,
            expected: FrontendStatus::Complete,
            oracle: Some(oracle_single(n, predicate)),
        });
    }
    for index in 0..60 {
        let n = sides[index % sides.len()];
        let (text, predicate) = match index % 8 {
            0 => (
                format!("Two fair {n}-sided dice are rolled. What is the probability that their sum is 7?"),
                JointPredicate::SumEquals(7),
            ),
            1 => (
                format!("Two standard {n}-sided dice are rolled. What is the probability that the sum is less than 8?"),
                JointPredicate::SumLessThan(8),
            ),
            2 => (
                format!("Two uniform {n}-sided dice are rolled. What is the probability that the sum is greater than 5?"),
                JointPredicate::SumGreaterThan(5),
            ),
            3 => (
                format!("A pair of fair {n}-sided dice is rolled. What is the probability that at least one die shows 1?"),
                JointPredicate::AtLeastOneEquals(1),
            ),
            4 => (
                format!("Two fair {n}-sided dice are rolled. What is the probability that their numbers differ by 2?"),
                JointPredicate::DifferenceEquals(2),
            ),
            5 => (
                format!("Two standard {n}-sided dice are rolled. What is the probability that their product is even?"),
                JointPredicate::ProductEven,
            ),
            6 => (
                format!("Two fair {n}-sided dice are rolled. What is the probability that their product is odd?"),
                JointPredicate::ProductOdd,
            ),
            _ => (
                format!("Two uniform {n}-sided dice are rolled. What is the probability that their sum is prime?"),
                JointPredicate::SumPrime,
            ),
        };
        cases.push(Case {
            text,
            expected: FrontendStatus::Complete,
            oracle: Some(oracle_joint(n, predicate)),
        });
    }
    cases
}

fn boundary_cases() -> Vec<Case> {
    let mut cases = Vec::new();
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
    cases
}

fn unsupported_cases() -> Vec<Case> {
    let templates = [
        "Three fair six-sided dice are rolled. What is the probability that their sum is 9?",
        "A loaded six-sided die is rolled. What is the probability of an even outcome?",
        "A fair six-sided die is rolled repeatedly. What is the probability of a six?",
        "Two fair six-sided dice are rolled without replacement. What is the probability their sum is 7?",
        "A fair six-sided die is rolled. What is the expected value of the outcome?",
    ];
    (0..80)
        .map(|index| Case {
            text: templates[index % templates.len()].into(),
            expected: FrontendStatus::Unsupported,
            oracle: None,
        })
        .collect()
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut cases = supported_cases();
    cases.extend(boundary_cases());
    cases.extend(unsupported_cases());
    let corpus = cases
        .iter()
        .map(|case| case.text.as_str())
        .collect::<Vec<_>>()
        .join("\n");
    let corpus_sha256 = sha(corpus.as_bytes());
    let source_sha256 = sha(SOURCE.as_bytes());
    let mut exact_decisions = 0;
    let mut supported_values = 0;
    let mut incorrect_values = 0;
    let mut frontend_replays = 0;
    let mut frontend_tamper_rejections = 0;
    let mut execution_replays = 0;
    let mut execution_tamper_rejections = 0;
    let mut false_authorizations = 0;
    let mut false_denials = 0;
    let mut provenance_complete = 0;
    for (index, case) in cases.iter().enumerate() {
        let result = formalize(&case.text, &format!("stage439-{index}"));
        exact_decisions += usize::from(result.status == case.expected);
        frontend_replays += usize::from(replay_verified(&result));
        provenance_complete += usize::from(!result.provenance.is_empty());
        let mut frontend_tampered = result.clone();
        frontend_tampered.replay_hash.push('x');
        frontend_tamper_rejections += usize::from(!replay_verified(&frontend_tampered));
        let execution = execute(&result);
        if case.expected == FrontendStatus::Complete {
            let Some(execution) = execution else {
                false_denials += 1;
                continue;
            };
            if execution.status != FrontendStatus::Complete {
                false_denials += 1;
                continue;
            }
            if execution.value == case.oracle {
                supported_values += 1;
            } else {
                incorrect_values += 1;
            }
            execution_replays += usize::from(execution_replay_verified(&execution));
            let mut execution_tampered = execution.clone();
            execution_tampered.replay_hash.push('x');
            execution_tamper_rejections +=
                usize::from(!execution_replay_verified(&execution_tampered));
        } else if execution.is_some() {
            false_authorizations += 1;
        }
    }
    let mut report = Report {
        schema: "stage439-finite-experiment-frontend-v1",
        cases: cases.len(),
        supported: 120,
        ambiguous: 40,
        unsupported: 80,
        exact_decisions,
        supported_values,
        incorrect_values,
        frontend_replays,
        frontend_tamper_rejections,
        execution_replays,
        execution_tamper_rejections,
        false_authorizations,
        false_denials,
        provenance_complete,
        source_sha256,
        corpus_sha256,
        report_sha256: String::new(),
    };
    report.report_sha256 = sha(&serde_json::to_vec(&report)?);
    assert_eq!(report.cases, 240);
    assert_eq!(report.exact_decisions, report.cases);
    assert_eq!(report.supported_values, report.supported);
    assert_eq!(report.incorrect_values, 0);
    assert_eq!(report.frontend_replays, report.cases);
    assert_eq!(report.frontend_tamper_rejections, report.cases);
    assert_eq!(report.execution_replays, report.supported);
    assert_eq!(report.execution_tamper_rejections, report.supported);
    assert_eq!(report.false_authorizations, 0);
    assert_eq!(report.false_denials, 0);
    assert_eq!(report.provenance_complete, report.cases);
    fs::write(
        REPORT_JSON,
        format!("{}\n", serde_json::to_string_pretty(&report)?),
    )?;
    fs::write(
        REPORT_MD,
        format!(
            "# Stage 439 — bounded finite-experiment frontend\n\n- cases / supported / ambiguous / unsupported: {} / {} / {} / {}\n- exact decisions: {}/{}\n- supported values / incorrect values: {}/{}\n- frontend replay / tamper: {}/{} / {}/{}\n- execution replay / tamper: {}/{} / {}/{}\n- provenance: {}/{}\n- false authorizations / denials: {} / {}\n- source SHA-256: `{}`\n- corpus SHA-256: `{}`\n\nThe independent corpus covers one explicitly uniform die and two independent explicitly uniform dice with bounded joint predicates. Multi-die, loaded, repeated, replacement, and expectation requests remain outside the contract.\n",
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
            report.provenance_complete,
            report.cases,
            report.false_authorizations,
            report.false_denials,
            report.source_sha256,
            report.corpus_sha256,
        ),
    )?;
    println!("{}", serde_json::to_string_pretty(&report)?);
    Ok(())
}
