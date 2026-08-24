//! Independent pressure benchmark for the canonical linear-congruence bridge.
//!
//! Only one literal congruence with a unique residue is executable.  Systems,
//! non-canonical targets, symbolic expressions, and multi-solution classes are
//! deliberately retained as boundaries.  The corpus is generated here and
//! never reads an answer key or mutable registry state.

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

fn unique_residue(coefficient: u64, right: u64, modulus: u64) -> Option<u64> {
    (0..modulus).find(|residue| (coefficient * residue) % modulus == right % modulus)
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut cases = Vec::new();
    for index in 0..120_u64 {
        let modulus = 17 + (index * 29 % 479);
        let mut coefficient = 3 + (index * 11 % (modulus - 2));
        while gcd(coefficient, modulus) != 1 {
            coefficient += 1;
            if coefficient >= modulus {
                coefficient = 1;
            }
        }
        let mut right = 5 + (index * 31 % (modulus - 5));
        let residue = loop {
            let candidate = unique_residue(coefficient, right, modulus).unwrap();
            if candidate != 0 {
                break candidate;
            }
            right = (right + 1) % modulus;
        };
        let text = match index % 3 {
            0 => format!(
                "What is the smallest positive integer satisfying the congruence ${coefficient}x ≡ {right} (mod {modulus})$?"
            ),
            1 => format!(
                "Find the least nonnegative integer x such that ${coefficient}x \\equiv {right} \\pmod{{{modulus}}}$."
            ),
            _ => format!(
                "What integer n satisfies 0\\le n<{modulus} and ${coefficient}n\\equiv{right}\\pmod{{{modulus}}}$?"
            ),
        };
        cases.push((text, Expected::Supported, Some(residue)));
    }

    for index in 0..40_u64 {
        let left_modulus = 7 + index % 17;
        let right_modulus = 11 + index % 19;
        cases.push((
            format!(
                "Solve the simultaneous congruences x ≡ {} (mod {left_modulus}) and x ≡ {} (mod {right_modulus}) and report the least nonnegative solution.",
                2 + index % left_modulus,
                3 + index % right_modulus
            ),
            Expected::Ambiguous,
            None,
        ));
    }

    for index in 0..80_u64 {
        let modulus = 20 + index % 37;
        let text = match index % 5 {
            0 => format!(
                "Find the largest negative integer x satisfying 3x+{} ≡ 2 (mod {modulus}).",
                index % 7
            ),
            1 => format!(
                "Find the smallest positive integer satisfying 6x ≡ {} (mod {modulus}).",
                2 + index % 5
            ),
            2 => {
                format!("Find the least nonnegative solution of {index}x + 2 ≡ 3 (mod {modulus}).")
            }
            3 => format!(
                "Find the least nonnegative integer x satisfying (3x + 2) ≡ {} (mod {modulus}).",
                4 + index % 5
            ),
            _ => format!("Find the least nonnegative integer x satisfying ax ≡ {index} (mod m)."),
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
        let frontend = formalize_number_theory_text(text, &format!("stage477-{index}"));
        frontend_replay_verified += usize::from(frontend_replay(&frontend));
        let mut frontend_tampered = frontend.clone();
        frontend_tampered.replay_hash.push('x');
        tamper_rejections += usize::from(!frontend_replay(&frontend_tampered));

        let mut observed = match frontend.status {
            NumberTheoryFrontendStatus::Ambiguous => Expected::Ambiguous,
            _ => Expected::Unsupported,
        };
        let mut candidate_value = None;
        if let Some(request) = frontend.request.as_ref() {
            let execution = evaluate_number_theory(request);
            execution_replay_verified += usize::from(execution.replay_verified());
            let mut execution_tampered = execution.clone();
            execution_tampered.replay_hash.push('x');
            tamper_rejections += usize::from(!execution_tampered.replay_verified());
            if execution.status == NumberTheoryStatus::Complete {
                if let Some(NumberTheoryArtifact::CongruenceClass {
                    residue,
                    solution_count,
                    ..
                }) = execution.artifact
                {
                    if solution_count == 1 {
                        observed = Expected::Supported;
                        candidate_value = Some(residue);
                    }
                }
            }
        }
        if std::mem::discriminant(&observed) == std::mem::discriminant(expected) {
            exact_decisions += 1;
            if matches!(observed, Expected::Supported)
                && candidate_value == expected_value.map(|value| value as u64)
            {
                supported_values += 1;
            }
        } else if matches!(observed, Expected::Supported) {
            false_authorizations += 1;
        } else if matches!(expected, Expected::Supported) {
            false_denials += 1;
        }
    }

    let mut report = Report {
        schema: "stage477-linear-congruence-bench-v1",
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
        "/tmp/stage477_linear_congruence_bench.json",
        format!("{}\n", serde_json::to_string_pretty(&report)?),
    )?;
    Ok(())
}
