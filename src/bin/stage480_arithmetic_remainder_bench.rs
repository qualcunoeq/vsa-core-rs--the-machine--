//! Independent pressure benchmark for the bounded arithmetic-remainder bridge.
//!
//! This benchmark is separate from the frozen literal-remainder contract. It
//! accepts a complete integer expression in a restricted grammar and refuses
//! symbolic, polynomial, division, factorial, implicit-multiplication, and
//! oversized-power forms.

use serde::Serialize;
use sha2::{Digest, Sha256};
use std::fs;
use the_machine::number_theory_frontend::{
    formalize_arithmetic_remainder_text, replay_verified as frontend_replay,
    NumberTheoryFrontendStatus,
};
use the_machine::number_theory_pack::{
    evaluate_number_theory, NumberTheoryArtifact, NumberTheoryOperation, NumberTheoryStatus,
};

#[derive(Clone, Copy, Debug)]
enum Expected {
    Supported,
    Ambiguous,
    Unsupported,
}

#[derive(Serialize)]
struct Report {
    schema: &'static str,
    corpus_sha256: String,
    cases: usize,
    supported: usize,
    ambiguous: usize,
    unsupported: usize,
    exact_decisions: usize,
    supported_values: usize,
    frontend_replay_verified: usize,
    execution_replay_verified: usize,
    tamper_rejections: usize,
    false_authorizations: usize,
    false_denials: usize,
    report_sha256: String,
}

fn digest_bytes(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

fn digest<T: Serialize>(value: &T) -> String {
    format!("{:x}", Sha256::digest(serde_json::to_vec(value).unwrap()))
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut cases = Vec::new();
    for index in 0..120_i64 {
        let a = 2 + index % 17;
        let b = 3 + index % 13;
        let c = 2 + index % 7;
        let divisor = 3 + (index as u64 * 11 % 97);
        let (text, value) = match index % 4 {
            0 => (
                format!("What is the remainder when ({a} + {b}) * {c} is divided by {divisor}?"),
                (a + b) * c,
            ),
            1 => (
                format!(
                    "Find the remainder when {a}^{exp} + {b}^{exp} is divided by {divisor}.",
                    exp = 2
                ),
                a * a + b * b,
            ),
            2 => (
                format!("What is the remainder of {a} * ({b} - {c}) divided by {divisor}?"),
                a * (b - c),
            ),
            _ => (
                format!(
                    "Determine the remainder when ({a} - {b}) * ({c} + 4) is divided by {divisor}."
                ),
                (a - b) * (c + 4),
            ),
        };
        cases.push((
            text,
            Expected::Supported,
            Some(value.rem_euclid(divisor as i64) as u64),
        ));
    }
    for index in 0..40_i64 {
        cases.push((
            format!("Find the remainder when {index} + 5 is divided by 7 or 9, case {index}."),
            Expected::Ambiguous,
            None,
        ));
    }
    for index in 0..80_i64 {
        let text = match index % 5 {
            0 => format!("Find the remainder when x + {index} is divided by 7."),
            1 => format!("Find the polynomial remainder when x^{index} is divided by x+1."),
            2 => format!("Find the remainder when {index}! is divided by 11."),
            3 => format!(
                "Find the remainder when {index}/{} is divided by 5.",
                index + 1
            ),
            _ => format!(
                "Find the remainder when 2^{exponent} is divided by 11.",
                exponent = 13 + index
            ),
        };
        cases.push((text, Expected::Unsupported, None));
    }

    let corpus_bytes = serde_json::to_vec(
        &cases
            .iter()
            .map(|(text, expected, value)| (text, format!("{expected:?}"), value))
            .collect::<Vec<_>>(),
    )?;
    let mut exact_decisions = 0;
    let mut supported_values = 0;
    let mut frontend_replay_verified = 0;
    let mut execution_replay_verified = 0;
    let mut tamper_rejections = 0;
    let mut false_authorizations = 0;
    let mut false_denials = 0;

    for (index, (text, expected, expected_value)) in cases.iter().enumerate() {
        let frontend = formalize_arithmetic_remainder_text(text, &format!("stage480-{index}"));
        frontend_replay_verified += usize::from(frontend_replay(&frontend));
        let mut frontend_tampered = frontend.clone();
        frontend_tampered.replay_hash.push('x');
        tamper_rejections += usize::from(!frontend_replay(&frontend_tampered));
        let mut observed = match frontend.status {
            NumberTheoryFrontendStatus::Ambiguous => Expected::Ambiguous,
            _ => Expected::Unsupported,
        };
        let mut observed_value = None;
        if let Some(request) = frontend.request.as_ref() {
            let execution = evaluate_number_theory(request);
            execution_replay_verified += usize::from(execution.replay_verified());
            let mut execution_tampered = execution.clone();
            execution_tampered.replay_hash.push('x');
            tamper_rejections += usize::from(!execution_tampered.replay_verified());
            if request.operation == NumberTheoryOperation::ArithmeticRemainder
                && execution.status == NumberTheoryStatus::Complete
            {
                if let Some(NumberTheoryArtifact::Scalar(value)) = execution.artifact {
                    observed = Expected::Supported;
                    observed_value = Some(value);
                }
            }
        }
        if std::mem::discriminant(&observed) == std::mem::discriminant(expected) {
            exact_decisions += 1;
            supported_values += usize::from(
                matches!(observed, Expected::Supported)
                    && observed_value == expected_value.map(|value| value as u64),
            );
        } else if matches!(observed, Expected::Supported) {
            false_authorizations += 1;
        } else if matches!(expected, Expected::Supported) {
            false_denials += 1;
        }
    }

    let mut report = Report {
        schema: "stage480-arithmetic-remainder-bench-v1",
        corpus_sha256: digest_bytes(&corpus_bytes),
        cases: cases.len(),
        supported: 120,
        ambiguous: 40,
        unsupported: 80,
        exact_decisions,
        supported_values,
        frontend_replay_verified,
        execution_replay_verified,
        tamper_rejections,
        false_authorizations,
        false_denials,
        report_sha256: String::new(),
    };
    report.report_sha256 = digest(&report);
    println!("{}", serde_json::to_string_pretty(&report)?);
    fs::write(
        "/tmp/stage480_arithmetic_remainder_bench.json",
        format!("{}\n", serde_json::to_string_pretty(&report)?),
    )?;
    Ok(())
}
