//! Independent pressure benchmark for canonical bounded Euler-totient input.
//!
//! The unit-count phrasing is accepted only when the stated interval is
//! exactly `0..n-1` modulo `n`; general coprimality counts remain outside the
//! route. No external answer keys or mutable registry state are read.

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

fn gcd(mut left: u64, mut right: u64) -> u64 {
    while right != 0 {
        let remainder = left % right;
        left = right;
        right = remainder;
    }
    left
}

fn totient_oracle(value: u64) -> u64 {
    (0..value)
        .filter(|candidate| gcd(*candidate, value) == 1)
        .count() as u64
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut cases = Vec::new();
    for index in 0..120_u64 {
        let modulus = 2 + (index * 37 % 497);
        let text = match index % 3 {
            0 => format!("Compute Euler's totient of {modulus}."),
            1 => format!("Compute totient({modulus})."),
            _ => format!(
                "How many integers between 0 and {} inclusive have an inverse modulo {modulus}?",
                modulus - 1
            ),
        };
        cases.push((text, Expected::Supported, Some(totient_oracle(modulus))));
    }
    for index in 0..40_u64 {
        cases.push((
            format!(
                "Either Euler's totient of {} or a modular inverse is intended, case {index}.",
                9 + index
            ),
            Expected::Ambiguous,
            None,
        ));
    }
    for index in 0..80_u64 {
        let text = match index % 4 {
            0 => format!(
                "How many integers between 1 and {} are relatively prime to {}?",
                15 + index,
                15 + index
            ),
            1 => format!("Compute Euler's totient of n, case {index}."),
            2 => format!("Compute the asymptotic totient behavior for case {index}."),
            _ => format!(
                "How many integers between 0 and 8 have an inverse modulo 9 or 11, case {index}?"
            ),
        };
        let expected = if index % 4 == 3 {
            Expected::Ambiguous
        } else {
            Expected::Unsupported
        };
        cases.push((text, expected, None));
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
        let frontend = formalize_number_theory_text(text, &format!("stage475-{index}"));
        frontend_replay_verified += usize::from(frontend_replay(&frontend));
        let mut frontend_tampered = frontend.clone();
        frontend_tampered.replay_hash.push('x');
        tamper_rejections += usize::from(!frontend_replay(&frontend_tampered));
        let observed = if let Some(request) = frontend.request.as_ref() {
            let execution = evaluate_number_theory(request);
            execution_replay_verified += usize::from(execution.replay_verified());
            let mut execution_tampered = execution.clone();
            execution_tampered.replay_hash.push('x');
            tamper_rejections += usize::from(!execution_tampered.replay_verified());
            if execution.status == NumberTheoryStatus::Complete {
                if let Some(NumberTheoryArtifact::Scalar(value)) = execution.artifact {
                    supported_values += usize::from(*expected_value == Some(value));
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
        schema: "stage475-totient-frontend-bench-v1",
        corpus_sha256: digest_bytes(&corpus_bytes),
        cases: cases.len(),
        supported: 120,
        ambiguous: 60,
        unsupported: 60,
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
        "/tmp/stage475_totient_frontend_bench.json",
        format!("{}\n", serde_json::to_string_pretty(&report)?),
    )?;
    Ok(())
}
