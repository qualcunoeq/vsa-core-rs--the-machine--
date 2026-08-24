//! Independent pressure benchmark for the literal modular-inverse frontend.
//!
//! The corpus separates direct literal inverses from ambiguity, symbolic or
//! compound operands, supplied-answer statements, and non-coprime failures.
//! It never reads Goal 6 answer keys or mutates the curriculum.

use serde::Serialize;
use sha2::{Digest, Sha256};
use std::fs;
use the_machine::number_theory_frontend::{
    formalize_number_theory_text, replay_verified as frontend_replay, NumberTheoryFrontendStatus,
};
use the_machine::number_theory_pack::{
    evaluate_number_theory, NumberTheoryArtifact, NumberTheoryStatus,
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

fn gcd(mut a: u64, mut b: u64) -> u64 {
    while b != 0 {
        let remainder = a % b;
        a = b;
        b = remainder;
    }
    a
}

fn inverse_oracle(value: u64, modulus: u64) -> Option<u64> {
    (0..modulus).find(|candidate| (value * candidate) % modulus == 1)
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut cases = Vec::new();

    // 120 supported direct literal inverse requests with varied prose and
    // notation. Values are selected independently and filtered by coprimality.
    let mut index = 0_u64;
    while cases.len() < 120 {
        let value = 3 + (index * 37 % 997);
        let modulus = 1009 + (index * 53 % 700);
        index += 1;
        if gcd(value, modulus) != 1 {
            continue;
        }
        let inverse = inverse_oracle(value, modulus).expect("coprime inverse");
        let text = match cases.len() % 3 {
            0 => format!("Find the modular inverse of {value} modulo {modulus}."),
            1 => {
                format!("Find the multiplicative inverse to {value} modulo {modulus} as a residue.")
            }
            _ => format!("Find ${value}^{{-1}} \\pmod{{{modulus}}}$."),
        };
        cases.push((text, Expected::Supported, Some(inverse)));
    }

    // 40 cases where inverse and another operation are explicitly competing.
    for index in 0..40_u64 {
        cases.push((
            format!(
                "Either the modular inverse of {} modulo {} or a linear congruence is intended, case {index}.",
                4 + index,
                35 + index
            ),
            Expected::Ambiguous,
            None,
        ));
    }

    // 80 refusal cases: non-coprime, symbolic, compound, or supplied-answer
    // statements must never become executable direct inverse requests.
    for index in 0..80_u64 {
        let text = match index % 4 {
            0 => format!("Find the modular inverse of 6 modulo 15, case {index}."),
            1 => format!("Find the modular inverse of a modulo m, case {index}."),
            2 => format!("Find the modular inverse of 4*7 modulo 35, case {index}."),
            _ => format!(
                "Given that 13 inverse is 29 modulo 47, find another inverse, case {index}."
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
        let frontend = formalize_number_theory_text(text, &format!("stage473-{index}"));
        if frontend_replay(&frontend) {
            frontend_replay_verified += 1;
        }
        let mut frontend_tampered = frontend.clone();
        frontend_tampered.replay_hash.push('x');
        if !frontend_replay(&frontend_tampered) {
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
            if execution.status == NumberTheoryStatus::Complete {
                if let Some(NumberTheoryArtifact::Scalar(value)) = execution.artifact {
                    if *expected_value == Some(value) {
                        supported_values += 1;
                    }
                }
                Expected::Supported
            } else {
                Expected::Unsupported
            }
        } else if frontend.status == NumberTheoryFrontendStatus::Ambiguous {
            Expected::Ambiguous
        } else {
            Expected::Unsupported
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
        schema: "stage473-modular-inverse-bench-v1",
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
        "/tmp/stage473_modular_inverse_bench.json",
        format!("{}\n", serde_json::to_string_pretty(&report)?),
    )?;
    Ok(())
}
