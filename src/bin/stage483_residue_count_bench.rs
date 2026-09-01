//! Independent pressure benchmark for finite inclusive residue counting.
//!
//! The frontend requires an explicit range and one residue condition. It does
//! not infer digit restrictions, divisor sets, or missing bounds.

use serde::Serialize;
use sha2::{Digest, Sha256};
use std::fs;
use the_machine::number_theory_frontend::{
    formalize_residue_count_text, replay_verified as frontend_replay, NumberTheoryFrontendStatus,
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

fn count_residues(lower: i64, upper: i64, residue: i64, modulus: u64) -> u64 {
    (lower..=upper)
        .filter(|value| value.rem_euclid(modulus as i64) == residue.rem_euclid(modulus as i64))
        .count() as u64
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut cases = Vec::new();
    for index in 0..120_i64 {
        let modulus = 3 + (index as u64 * 17 % 89);
        let residue = index % modulus as i64;
        let lower = 10 + index % 41;
        let upper = lower + 70 + index % 37;
        let expected = count_residues(lower, upper, residue, modulus);
        let text = match index % 4 {
            0 => format!(
                "How many integers between {lower} and {upper} are divisible by {modulus}?"
            ),
            1 => format!(
                "How many integers from {lower} to {upper} are congruent to {residue} (mod {modulus})?"
            ),
            2 => format!(
                "How many positive integers less than {} are congruent to {residue} (mod {modulus})?",
                upper + 1
            ),
            _ => format!(
                "How many positive two-digit integers leave a remainder of {residue} when divided by {modulus}?"
            ),
        };
        let (expected, expected_value) = if index % 4 == 0 {
            (
                Expected::Supported,
                Some(count_residues(lower, upper, 0, modulus)),
            )
        } else if index % 4 == 2 {
            (
                Expected::Supported,
                Some(count_residues(1, upper, residue, modulus)),
            )
        } else if index % 4 == 3 {
            (
                Expected::Supported,
                Some(count_residues(10, 99, residue, modulus)),
            )
        } else {
            (Expected::Supported, Some(expected))
        };
        cases.push((text, expected, expected_value));
    }
    for index in 0..40_i64 {
        cases.push((
            format!(
                "How many integers between {} and {} are divisible by {} or {}?",
                10 + index,
                80 + index,
                5 + index % 7,
                7 + index % 11
            ),
            Expected::Ambiguous,
            None,
        ));
    }
    for index in 0..80_i64 {
        let text = match index % 4 {
            0 => format!(
                "How many positive three-digit integers with each digit greater than 4 are divisible by {}?",
                3 + index % 9
            ),
            1 => format!(
                "How many positive divisors of {} are not divisible by {}?",
                120 + index,
                3 + index % 7
            ),
            2 => format!("How many integers are congruent to {} (mod 11)?", index % 11),
            _ => format!(
                "How many integers between {} and {} satisfy a remainder condition?",
                index,
                index + 20
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
        let frontend = formalize_residue_count_text(text, &format!("stage483-{index}"));
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
            if request.operation == NumberTheoryOperation::ResidueCount
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
        schema: "stage483-residue-count-bench-v1",
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
        "/tmp/stage483_residue_count_bench.json",
        format!("{}\n", serde_json::to_string_pretty(&report)?),
    )?;
    Ok(())
}
