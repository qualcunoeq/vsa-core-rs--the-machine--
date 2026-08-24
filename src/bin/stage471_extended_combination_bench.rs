//! Independent pressure benchmark for the extended exact-combination scope.
//!
//! The ordinary counting evaluator keeps its frozen `n <= 20` contract.  This
//! corpus validates the explicit opt-in extension for larger numeric LaTeX
//! binomials, including overflow, range, ambiguity, and compound-expression
//! boundaries.  It never reads benchmark answer keys or mutates a registry.

use serde::Serialize;
use sha2::{Digest, Sha256};
use std::fs;
use the_machine::source_counting_frontend::{
    formalize_counting_text, replay_verified as frontend_replay, CountingFrontendStatus,
};
use the_machine::source_counting_pack::{
    evaluate_extended_combination, replay_verified, CountingArtifact, CountingStatus,
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

fn oracle_combination(n: u64, r: u64) -> u128 {
    let terms = r.min(n - r);
    (1..=terms).fold(1_u128, |value, index| {
        value * (n - terms + index) as u128 / index as u128
    })
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut cases = Vec::new();

    // 120 supported larger-n edge and low-order coefficients.  These values
    // are deliberately varied while remaining independently calculable in
    // the exact-u128 boundary.
    for index in 0..120_u64 {
        let n = 21 + (index * 791 % 4_900);
        let r = match index % 6 {
            0 => 0,
            1 => 1,
            2 => 2,
            3 => n - 2,
            4 => n - 1,
            _ => n,
        };
        let marker = if index % 2 == 0 { "\\binom" } else { "\\dbinom" };
        cases.push((
            format!("Compute ${marker}{{{n}}}{{{r}}}$."),
            Expected::Supported,
            Some(oracle_combination(n, r)),
        ));
    }

    // 40 explicit interpretation conflicts.  The extension must preserve
    // ambiguity rather than reinterpret a binomial as an ordered operation.
    for index in 0..40_u64 {
        let text = match index % 4 {
            0 => format!(
                "Interpret $\\binom{{{}}}{{2}}$ as an ordered permutation, case {index}.",
                30 + index
            ),
            1 => format!(
                "Either $\\dbinom{{{}}}{{1}}$ or an ordered count is intended, case {index}.",
                40 + index
            ),
            2 => format!(
                "Use either the binomial coefficient or a permutation in case {index}."
            ),
            _ => format!(
                "The notation $\\binom{{{}}}{{3}}$ is ambiguous here, case {index}.",
                50 + index
            ),
        };
        cases.push((text, Expected::Ambiguous, None));
    }

    // 80 unsupported boundaries: range, exact-overflow, symbolic, and
    // compound expressions.
    for index in 0..80_u64 {
        let text = match index % 4 {
            0 => format!("Compute $\\binom{{100001}}{{1}}$ for case {index}."),
            1 => format!("Compute $\\binom{{500}}{{250}}$ for case {index}."),
            2 => format!(
                "Compute $\\binom{{{}}}{{2}}\\times\\binom{{7}}{{2}}$ for case {index}.",
                30 + index
            ),
            _ => format!("Compute the symbolic coefficient $\\binom{{n}}{{k}}$ for case {index}."),
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
        let frontend = formalize_counting_text(text, &format!("stage471-{index}"));
        if frontend_replay(&frontend) {
            frontend_replay_verified += 1;
        }
        let mut frontend_tampered = frontend.clone();
        frontend_tampered.replay_hash.push('x');
        if !frontend_replay(&frontend_tampered) {
            tamper_rejections += 1;
        }

        let observed = if let Some(request) = frontend.request.as_ref() {
            let execution = evaluate_extended_combination(request);
            if replay_verified(&execution) {
                execution_replay_verified += 1;
            }
            let mut execution_tampered = execution.clone();
            execution_tampered.replay_hash.push('x');
            if !replay_verified(&execution_tampered) {
                tamper_rejections += 1;
            }
            if execution.status == CountingStatus::Complete {
                if let Some(CountingArtifact::ExactCount(value)) = execution.artifact {
                    if *expected_value == Some(value) {
                        supported_values += 1;
                    }
                }
                Expected::Supported
            } else {
                Expected::Unsupported
            }
        } else if frontend.status == CountingFrontendStatus::Ambiguous {
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
        schema: "stage471-extended-combination-bench-v1",
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
        "/tmp/stage471_extended_combination_bench.json",
        format!("{}\n", serde_json::to_string_pretty(&report)?),
    )?;
    Ok(())
}
