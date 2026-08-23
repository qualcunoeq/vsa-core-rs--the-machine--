//! Independent pressure benchmark for bounded literal integer remainders.

use serde::Serialize;
use sha2::{Digest, Sha256};
use std::fs;
use the_machine::number_theory_frontend::{
    formalize_number_theory_text, replay_verified, NumberTheoryFrontendStatus,
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
        let dividend = index * 7919 - 200;
        let divisor = 2 + (index as u64 * 13 % 97);
        cases.push((
            format!("What is the remainder when {dividend} is divided by {divisor}?"),
            Expected::Supported,
            Some(dividend.rem_euclid(divisor as i64) as u64),
        ));
    }
    for index in 0..40 {
        cases.push((
            format!("Find the remainder when 12 is divided by 5 or 7, case {index}."),
            Expected::Ambiguous,
            None,
        ));
    }
    for index in 0..80 {
        let text = match index % 4 {
            0 => format!("Find the remainder when x is divided by 7, case {index}."),
            1 => format!("Find the remainder when 1^2 + 2^2 is divided by 11, case {index}."),
            2 => format!("Find the polynomial remainder when x^3 is divided by x+1, case {index}."),
            _ => format!("Determine the quotient when 8 is divided by 3, case {index}."),
        };
        cases.push((text, Expected::Unsupported, None));
    }
    let corpus = serde_json::to_vec(
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
        let frontend = formalize_number_theory_text(text, &format!("stage468-{index}"));
        if replay_verified(&frontend) {
            frontend_replay_verified += 1;
        }
        let mut frontend_tampered = frontend.clone();
        frontend_tampered.replay_hash.push('x');
        if !replay_verified(&frontend_tampered) {
            tamper_rejections += 1;
        }
        let observed = if let Some(request) = frontend.request.as_ref() {
            let execution = evaluate_number_theory(request);
            if execution.replay_verified() {
                execution_replay_verified += 1;
            }
            let mut execution_tampered = execution.clone();
            execution_tampered.replay_hash.push('x');
            if !execution_tampered.replay_verified() {
                tamper_rejections += 1;
            }
            if request.operation == NumberTheoryOperation::Remainder
                && execution.status == NumberTheoryStatus::Complete
            {
                if let Some(NumberTheoryArtifact::Scalar(value)) = execution.artifact {
                    if *expected_value == Some(value) {
                        supported_values += 1;
                    }
                }
                Expected::Supported
            } else {
                Expected::Unsupported
            }
        } else {
            match frontend.status {
                NumberTheoryFrontendStatus::Ambiguous => Expected::Ambiguous,
                _ => Expected::Unsupported,
            }
        };
        if std::mem::discriminant(&observed) == std::mem::discriminant(expected) {
            exact_decisions += 1;
        } else if matches!(observed, Expected::Supported) {
            false_authorizations += 1;
        } else if matches!(expected, Expected::Supported) {
            false_denials += 1;
        }
    }
    let mut report = Report {
        schema: "stage468-remainder-frontend-bench-v1",
        corpus_sha256: digest_bytes(&corpus),
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
    fs::write(
        "/tmp/stage468_remainder_frontend_bench.json",
        format!("{}\n", serde_json::to_string_pretty(&report)?),
    )?;
    println!("{}", serde_json::to_string_pretty(&report)?);
    Ok(())
}
