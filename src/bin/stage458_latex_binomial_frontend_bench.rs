//! Independent pressure benchmark for the bounded LaTeX binomial frontend.
//!
//! The corpus is generated independently of the external portfolio and checks
//! numeric notation, symbolic/malformed ambiguity, range and domain refusal,
//! replay, and tamper resistance.  It never reads an answer key or mutates a
//! registry.

use serde::Serialize;
use sha2::{Digest, Sha256};
use std::fs;
use the_machine::source_counting_frontend::{
    formalize_counting_text, replay_verified, CountingFrontendStatus,
};
use the_machine::source_counting_pack::{evaluate, CountingArtifact, CountingStatus};

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

fn choose(n: u64, r: u64) -> u128 {
    let r = r.min(n - r);
    (0..r).fold(1_u128, |acc, index| {
        acc * (n - index) as u128 / (index + 1) as u128
    })
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut cases = Vec::new();
    // 120 supported numeric \\binom/\\dbinom cases, including boundaries.
    for index in 0..120_u64 {
        let n = 2 + (index * 7 % 19);
        let r = (index * 11 % (n + 1)) as u64;
        let command = if index % 2 == 0 { "\\binom" } else { "\\dbinom" };
        cases.push((
            format!("Compute ${command}{{{n}}}{{{r}}}$."),
            Expected::Supported,
            Some(choose(n, r)),
        ));
    }
    // 40 cases where the notation or requested semantics are not unique.
    for index in 0..40_u64 {
        let text = match index % 4 {
            0 => format!("Interpret $\\binom{{5}}{{2}}$ as an ordered permutation, case {index}."),
            1 => format!("Either $\\binom{{5}}{{2}}$ or a permutation is intended, case {index}."),
            2 => format!("Use either the binomial coefficient or an ordered count, case {index}."),
            _ => format!("The notation $\\binom{{5}}{{2}}$ is ambiguous here, case {index}."),
        };
        cases.push((text, Expected::Ambiguous, None));
    }
    // 80 unsupported or out-of-range cases.  The frontend may complete the
    // notation, but the typed pack must still reject the bounded domain.
    for index in 0..80_u64 {
        let text = match index % 4 {
            0 => format!("Compute $\\binom{{21}}{{2}}$ for case {index}."),
            1 => format!("Compute the infinite binomial series for case {index}."),
            2 => format!("What is the symbolic coefficient $\\binom{{n}}{{k}}$ for case {index}?"),
            _ => format!("Compute $\\binom{{a+b}}{{2}}$ for case {index}."),
        };
        cases.push((text, Expected::Unsupported, None));
    }
    let corpus_bytes = serde_json::to_vec(
        &cases
            .iter()
            .map(|(text, expected, value)| {
                (text, format!("{expected:?}"), value)
            })
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
        let frontend = formalize_counting_text(text, &format!("stage458-{index}"));
        if replay_verified(&frontend) {
            frontend_replay_verified += 1;
        }
        let mut frontend_tampered = frontend.clone();
        frontend_tampered.replay_hash.push('x');
        if !replay_verified(&frontend_tampered) {
            tamper_rejections += 1;
        }
        let observed = if let Some(request) = frontend.request.as_ref() {
            let execution = evaluate(request);
            if the_machine::source_counting_pack::replay_verified(&execution) {
                execution_replay_verified += 1;
            }
            let mut execution_tampered = execution.clone();
            execution_tampered.replay_hash.push('x');
            if !the_machine::source_counting_pack::replay_verified(&execution_tampered) {
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
        } else {
            match frontend.status {
                CountingFrontendStatus::Ambiguous => Expected::Ambiguous,
                CountingFrontendStatus::Missing | CountingFrontendStatus::Unsupported => {
                    Expected::Unsupported
                }
                CountingFrontendStatus::Complete => Expected::Unsupported,
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
        schema: "stage458-latex-binomial-frontend-bench-v1",
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
        "/tmp/stage458_latex_binomial_frontend_bench.json",
        format!("{}\n", serde_json::to_string_pretty(&report)?),
    )?;
    Ok(())
}
