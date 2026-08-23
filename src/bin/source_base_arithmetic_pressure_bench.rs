//! Independent pressure corpus for the bounded positional-arithmetic route.
//!
//! The expected arithmetic values are computed by a small independent integer
//! oracle in this benchmark, not by the production pack.  The corpus contains
//! supported, ambiguous, and unsupported language boundaries and never reads
//! HLE or external answer keys.

use serde::Serialize;
use sha2::{Digest, Sha256};
use std::fs;
use the_machine::source_base_arithmetic_frontend::{formalize, replay_verified, FrontendStatus};
use the_machine::source_base_arithmetic_pack::{
    evaluate, replay_verified as execution_replay, BaseArithmeticOperation, BaseArithmeticStatus,
};

const REPORT_JSON: &str = "docs/source_base_arithmetic_pressure_bench.json";
const REPORT_MD: &str = "docs/source_base_arithmetic_pressure_bench.md";

#[derive(Debug, Clone, Copy)]
enum Expected {
    Supported,
    Ambiguous,
    Unsupported,
}

#[derive(Debug, Clone)]
struct Case {
    id: String,
    prompt: String,
    expected: Expected,
    expected_numeral: Option<String>,
}

#[derive(Debug, Serialize)]
struct Report {
    schema: &'static str,
    cases: usize,
    supported: usize,
    ambiguous: usize,
    unsupported: usize,
    exact_decisions: usize,
    replay_verified: usize,
    tamper_rejected: usize,
    supported_values_correct: usize,
    false_authorizations: usize,
    false_denials: usize,
    corpus_sha256: String,
}

fn digest<T: Serialize>(value: &T) -> String {
    format!("{:x}", Sha256::digest(serde_json::to_vec(value).unwrap()))
}

fn digit(value: u32) -> char {
    match value {
        0..=9 => (b'0' + value as u8) as char,
        10..=35 => (b'A' + (value as u8 - 10)) as char,
        _ => unreachable!(),
    }
}

fn render(mut value: u128, base: u32) -> String {
    if value == 0 {
        return "0".into();
    }
    let mut output = Vec::new();
    while value > 0 {
        output.push(digit((value % base as u128) as u32));
        value /= base as u128;
    }
    output.into_iter().rev().collect()
}

fn supported_cases() -> Vec<Case> {
    let bases = [2u32, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12];
    let operations = [
        (BaseArithmeticOperation::Add, "Add", 0u128),
        (
            BaseArithmeticOperation::Subtract,
            "Find the difference between",
            4,
        ),
        (BaseArithmeticOperation::Multiply, "Find the product of", 8),
    ];
    let mut cases = Vec::new();
    for index in 0..120usize {
        let base = bases[index % bases.len()];
        let (operation, phrase, offset) = operations[index / 40];
        let left = 7 + (index as u128 * 13) + offset;
        let right = match operation {
            BaseArithmeticOperation::Subtract => 1 + (index as u128 % 7),
            _ => 2 + (index as u128 * 5 % 19),
        };
        let expected = match operation {
            BaseArithmeticOperation::Add => left + right,
            BaseArithmeticOperation::Subtract => left - right,
            BaseArithmeticOperation::Multiply => left * right,
        };
        let left_numeral = render(left, base);
        let right_numeral = render(right, base);
        let prompt = if operation == BaseArithmeticOperation::Add {
            format!(
                "{phrase} {left_numeral}_{base} and {right_numeral}_{base}; express the result in base {base}."
            )
        } else {
            format!("{phrase} {left_numeral}_{base} and {right_numeral}_{base} in base {base}.")
        };
        cases.push(Case {
            id: format!("supported-{index:03}"),
            prompt,
            expected: Expected::Supported,
            expected_numeral: Some(render(expected, base)),
        });
    }
    cases
}

fn boundary_cases() -> Vec<Case> {
    let bases = [2u32, 4, 6, 8, 10, 12];
    (0..40usize)
        .map(|index| {
            let base = bases[index % bases.len()];
            let other = if base == 12 { 2 } else { base + 1 };
            let left = render(5 + index as u128, base);
            let right = render(2 + (index % 4) as u128, base);
            Case {
                id: format!("ambiguous-{index:03}"),
                prompt: format!(
                    "Add {left}_{base} and {right}_{base}; express the result in base {base} or base {other}."
                ),
                expected: Expected::Ambiguous,
                expected_numeral: None,
            }
        })
        .collect()
}

fn unsupported_cases() -> Vec<Case> {
    let mut cases = Vec::new();
    for index in 0..20usize {
        let base = 2 + (index as u32 % 10);
        let other = if base == 12 { 2 } else { base + 1 };
        let left = render(9 + index as u128, base);
        let right = render(3 + index as u128, other);
        cases.push(Case {
            id: format!("mixed-base-{index:03}"),
            prompt: format!(
                "Find the difference between {left}_{base} and {right}_{other} in base 10."
            ),
            expected: Expected::Unsupported,
            expected_numeral: None,
        });
    }
    for index in 0..20usize {
        let base = 2 + (index as u32 % 10);
        let a = render(2 + index as u128, base);
        let b = render(3 + index as u128, base);
        let c = render(4 + index as u128, base);
        cases.push(Case {
            id: format!("multi-operand-{index:03}"),
            prompt: format!("Add {a}_{base} + {b}_{base} + {c}_{base}; express in base {base}."),
            expected: Expected::Unsupported,
            expected_numeral: None,
        });
    }
    for index in 0..20usize {
        let base = 2 + (index as u32 % 10);
        let a = render(2 + index as u128, base);
        let b = render(3 + index as u128, base);
        cases.push(Case {
            id: format!("fraction-{index:03}"),
            prompt: format!(
                "Add {a}_{base} and {b}_{base}; express the fractional result in base {base}."
            ),
            expected: Expected::Unsupported,
            expected_numeral: None,
        });
    }
    for index in 0..20usize {
        let base = 2 + (index as u32 % 10);
        let a = render(2 + index as u128, base);
        let b = render(3 + index as u128, base);
        cases.push(Case {
            id: format!("digit-statistic-{index:03}"),
            prompt: format!(
                "Find the number of even digits in {a}_{base} and {b}_{base}, expressed in base {base}."
            ),
            expected: Expected::Unsupported,
            expected_numeral: None,
        });
    }
    cases
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut cases = supported_cases();
    cases.extend(boundary_cases());
    cases.extend(unsupported_cases());
    assert_eq!(cases.len(), 240);
    let corpus_sha256 = digest(
        &cases
            .iter()
            .map(|case| (&case.id, &case.prompt))
            .collect::<Vec<_>>(),
    );
    let mut exact_decisions = 0;
    let mut replay_verified_count = 0;
    let mut tamper_rejected = 0;
    let mut supported_values_correct = 0;
    let mut false_authorizations = 0;
    let mut false_denials = 0;
    for case in &cases {
        let frontend = formalize(&case.prompt, &case.id);
        let actual = match frontend.status {
            FrontendStatus::Complete => {
                let request = frontend.request.as_ref().expect("complete has request");
                let execution = evaluate(request);
                if execution.status == BaseArithmeticStatus::Complete {
                    if case.expected_numeral.as_deref() == execution.numeral.as_deref() {
                        supported_values_correct += 1;
                    }
                    "supported"
                } else {
                    "unsupported"
                }
            }
            FrontendStatus::Ambiguous => "ambiguous",
            FrontendStatus::Missing | FrontendStatus::Unsupported => "unsupported",
        };
        let expected = match case.expected {
            Expected::Supported => "supported",
            Expected::Ambiguous => "ambiguous",
            Expected::Unsupported => "unsupported",
        };
        if actual == expected {
            exact_decisions += 1;
        } else if expected == "supported" && actual == "unsupported" {
            false_denials += 1;
        } else if expected != "supported" && actual == "supported" {
            false_authorizations += 1;
        }
        let mut frontend_tampered = frontend.clone();
        frontend_tampered.replay_hash.push('x');
        let frontend_replay_ok = replay_verified(&frontend);
        let frontend_tamper_ok = !replay_verified(&frontend_tampered);
        replay_verified_count += usize::from(frontend_replay_ok);
        tamper_rejected += usize::from(frontend_tamper_ok);
        assert!(frontend_replay_ok && frontend_tamper_ok);
        if let Some(request) = frontend.request.as_ref() {
            let execution = evaluate(request);
            let mut execution_tampered = execution.clone();
            execution_tampered.replay_hash.push('x');
            assert!(execution_replay(&execution));
            assert!(!execution_replay(&execution_tampered));
        }
    }
    let report = Report {
        schema: "source-base-arithmetic-pressure-v1",
        cases: cases.len(),
        supported: 120,
        ambiguous: 40,
        unsupported: 80,
        exact_decisions,
        replay_verified: replay_verified_count,
        tamper_rejected,
        supported_values_correct,
        false_authorizations,
        false_denials,
        corpus_sha256,
    };
    assert_eq!(report.exact_decisions, report.cases);
    assert_eq!(report.supported_values_correct, report.supported);
    assert_eq!(report.replay_verified, report.cases);
    assert_eq!(report.tamper_rejected, report.cases);
    assert_eq!(report.false_authorizations, 0);
    assert_eq!(report.false_denials, 0);
    fs::write(
        REPORT_JSON,
        format!("{}\n", serde_json::to_string_pretty(&report)?),
    )?;
    fs::write(
        REPORT_MD,
        format!(
            "# Source-derived positional arithmetic pressure benchmark\n\n- Cases: {} ({} supported, {} ambiguous, {} unsupported)\n- Exact decisions: {}/{}\n- Supported values correct: {}/{}\n- Replay / tamper rejection: {} / {}\n- False authorizations / denials: {} / {}\n- Corpus SHA-256: `{}`\n\nThe arithmetic oracle is independent integer arithmetic in this benchmark. The production pack remains bounded to two finite same-base numerals and one binary operation; no HLE or external answer keys were read.\n",
            report.cases,
            report.supported,
            report.ambiguous,
            report.unsupported,
            report.exact_decisions,
            report.cases,
            report.supported_values_correct,
            report.supported,
            report.replay_verified,
            report.tamper_rejected,
            report.false_authorizations,
            report.false_denials,
            report.corpus_sha256,
        ),
    )?;
    println!(
        "positional arithmetic pressure: {}/{} decisions, {}/{} values, false_auth=0",
        report.exact_decisions, report.cases, report.supported_values_correct, report.supported
    );
    Ok(())
}
