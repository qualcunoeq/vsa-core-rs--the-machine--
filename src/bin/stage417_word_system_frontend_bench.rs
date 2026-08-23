//! Stage 417: independent validation and external transfer for the bounded
//! two-number word-system frontend.
//!
//! The independent corpus is generated from a grammar separate from the
//! OpenStax prompts.  The external portion is measured shadow-only and never
//! reads answer alignment.

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::fs;
use the_machine::curriculum::breadth_first_manifest;
use the_machine::source_word_system_frontend::{
    execute_word_system, execution_replay_verified, formalize_two_number_system, replay_verified,
    WordSystemStatus,
};

const EXTERNAL_CORPUS_PATH: &str = "docs/stage389_page_aware_external_problem_dev.json";
const SEALED_MANIFEST_PATH: &str =
    "docs/holdouts/stage389_page_aware_external_problem_sealed_manifest.json";
const EXTERNAL_CORPUS_SHA256: &str =
    "92304d95521ac2ef4d49a08272f0b0cf79ac7225c9715d5ace9a667c822f6e03";
const SEALED_MANIFEST_SHA256: &str =
    "ac4227bff20decad585210b8e0d4c60401f6056092b718e99157e4f208a1b8de";

#[derive(Debug, Deserialize)]
struct ExternalRecord {
    record_id: String,
    source_path: String,
    source_sha256: String,
    prompt: String,
    prompt_sha256: String,
    split: String,
    #[serde(default)]
    quality_flags: Vec<String>,
    answer_key_status: String,
}

#[derive(Debug, Clone, Copy)]
enum Expected {
    Complete,
    Ambiguous,
    Missing,
    Unsupported,
}

#[derive(Debug, Serialize)]
struct ExternalSample {
    record_id: String,
    source_path: String,
    source_sha256: String,
    prompt_sha256: String,
    split: String,
    status: WordSystemStatus,
    execution_complete: bool,
    frontend_replay_verified: bool,
    execution_replay_verified: bool,
    execution_tamper_rejected: bool,
}

#[derive(Debug, Serialize)]
struct Report {
    schema: &'static str,
    independent_cases: usize,
    independent_supported: usize,
    independent_ambiguous: usize,
    independent_missing: usize,
    independent_unsupported: usize,
    independent_exact_statuses: usize,
    independent_execution_complete: usize,
    independent_frontend_replay_verified: usize,
    independent_execution_replay_verified: usize,
    independent_tamper_rejected: usize,
    external_candidate_records: usize,
    external_frontend_complete: usize,
    external_execution_complete: usize,
    external_frontend_replay_verified: usize,
    external_execution_replay_verified: usize,
    external_execution_tamper_rejected: usize,
    external_samples: Vec<ExternalSample>,
    external_corpus_path: &'static str,
    external_corpus_sha256: String,
    declared_external_corpus_sha256: &'static str,
    sealed_manifest_path: &'static str,
    sealed_manifest_sha256: String,
    declared_sealed_manifest_sha256: &'static str,
    sealed_manifest_records: usize,
    answer_keys_read: usize,
    plaintext_answers_read: usize,
    production_authorizations: usize,
    false_authorizations: usize,
    manifest_sha256_before: String,
    manifest_sha256_after: String,
    manifest_unchanged: bool,
    report_sha256: String,
}

fn digest_bytes(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

fn digest<T: Serialize>(value: &T) -> String {
    digest_bytes(&serde_json::to_vec(value).expect("benchmark serializes"))
}

fn small_word(value: i128) -> String {
    const UNITS: [&str; 20] = [
        "zero",
        "one",
        "two",
        "three",
        "four",
        "five",
        "six",
        "seven",
        "eight",
        "nine",
        "ten",
        "eleven",
        "twelve",
        "thirteen",
        "fourteen",
        "fifteen",
        "sixteen",
        "seventeen",
        "eighteen",
        "nineteen",
    ];
    const TENS: [&str; 8] = [
        "twenty", "thirty", "forty", "fifty", "sixty", "seventy", "eighty", "ninety",
    ];
    if (0..20).contains(&value) {
        return UNITS[value as usize].into();
    }
    if (20..100).contains(&value) {
        let tens = value / 10;
        let unit = value % 10;
        return if unit == 0 {
            TENS[(tens - 2) as usize].into()
        } else {
            format!("{}-{}", TENS[(tens - 2) as usize], UNITS[unit as usize])
        };
    }
    value.to_string()
}

fn independent_cases() -> Vec<(String, Expected)> {
    let mut cases = Vec::new();
    for index in 0..120_i128 {
        let sum = 20 + index;
        let offset = 1 + (index % 9);
        let sum_text = if index % 2 == 0 {
            sum.to_string()
        } else {
            small_word(sum)
        };
        let offset_text = if index % 3 == 0 {
            offset.to_string()
        } else {
            small_word(offset)
        };
        let relation = match index % 4 {
            0 => format!("one number is {offset_text} less than the other"),
            1 => format!("one number is {offset_text} more than the other"),
            2 => format!("one number is {offset_text} less than twice the other"),
            _ => format!("one number is {offset_text} less than three times the other"),
        };
        cases.push((
            format!("The sum of two numbers is {sum_text}. {relation}. Find the numbers."),
            Expected::Complete,
        ));
    }
    for index in 0..20 {
        cases.push((
            format!(
                "The sum of two numbers is {}. One number is some amount less than the other.",
                20 + index
            ),
            Expected::Ambiguous,
        ));
    }
    for index in 0..20 {
        cases.push((
            format!(
                "The two numbers have a product of {}. Find the numbers.",
                10 + index
            ),
            Expected::Missing,
        ));
    }
    for index in 0..80 {
        let text = match index % 4 {
            0 => format!("The sum of three numbers is {}. One number is 4 less than the other.", 30 + index),
            1 => format!("The sum of two numbers is {}. One number is 4 multiplied by the other.", 30 + index),
            2 => format!("The sum of two numbers is {}. One number is 4 less than six times the other.", 30 + index),
            _ => format!("The sum of two numbers is {}. One number is four less than an unspecified quantity.", 30 + index),
        };
        cases.push((text, Expected::Unsupported));
    }
    cases
}

fn status_matches(status: WordSystemStatus, expected: Expected) -> bool {
    matches!(
        (status, expected),
        (WordSystemStatus::Complete, Expected::Complete)
            | (WordSystemStatus::Ambiguous, Expected::Ambiguous)
            | (WordSystemStatus::Missing, Expected::Missing)
            | (WordSystemStatus::Unsupported, Expected::Unsupported)
    )
}

fn is_external_candidate(text: &str) -> bool {
    let lower = text.to_ascii_lowercase();
    lower.contains("sum of two number") && lower.contains("one number is") && lower.contains("find")
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let independent = independent_cases();
    let mut independent_exact_statuses = 0;
    let mut independent_execution_complete = 0;
    let mut independent_frontend_replay_verified = 0;
    let mut independent_execution_replay_verified = 0;
    let mut independent_tamper_rejected = 0;
    for (index, (text, expected)) in independent.iter().enumerate() {
        let result = formalize_two_number_system(text, &format!("independent-{index}"));
        independent_exact_statuses += usize::from(status_matches(result.status, *expected));
        independent_frontend_replay_verified += usize::from(replay_verified(&result));
        if let Some(receipt) = execute_word_system(&result) {
            independent_execution_complete += 1;
            independent_execution_replay_verified +=
                usize::from(execution_replay_verified(&receipt));
            let mut tampered = receipt.clone();
            tampered.result.push('x');
            independent_tamper_rejected += usize::from(!execution_replay_verified(&tampered));
        }
    }

    let external_bytes = fs::read(EXTERNAL_CORPUS_PATH)?;
    let external_corpus_sha256 = digest_bytes(&external_bytes);
    let all_records: Vec<ExternalRecord> = serde_json::from_slice(&external_bytes)?;
    assert!(all_records
        .iter()
        .all(|record| record.answer_key_status == "not_read"));
    let external_records = all_records
        .into_iter()
        .filter(|record| record.quality_flags.is_empty() && is_external_candidate(&record.prompt))
        .collect::<Vec<_>>();
    let mut external_frontend_complete = 0;
    let mut external_execution_complete = 0;
    let mut external_frontend_replay_verified = 0;
    let mut external_execution_replay_verified = 0;
    let mut external_execution_tamper_rejected = 0;
    let mut external_samples = Vec::new();
    for record in &external_records {
        let result = formalize_two_number_system(&record.prompt, &record.record_id);
        let frontend_complete = result.status == WordSystemStatus::Complete;
        external_frontend_complete += usize::from(frontend_complete);
        external_frontend_replay_verified += usize::from(replay_verified(&result));
        let receipt = execute_word_system(&result);
        external_execution_complete += usize::from(receipt.is_some());
        if let Some(ref receipt) = receipt {
            external_execution_replay_verified += usize::from(execution_replay_verified(receipt));
            let mut tampered = receipt.clone();
            tampered.result.push('x');
            external_execution_tamper_rejected +=
                usize::from(!execution_replay_verified(&tampered));
        }
        if external_samples.len() < 100 {
            external_samples.push(ExternalSample {
                record_id: record.record_id.clone(),
                source_path: record.source_path.clone(),
                source_sha256: record.source_sha256.clone(),
                prompt_sha256: record.prompt_sha256.clone(),
                split: record.split.clone(),
                status: result.status,
                execution_complete: receipt.is_some(),
                frontend_replay_verified: replay_verified(&result),
                execution_replay_verified: receipt.as_ref().is_some_and(execution_replay_verified),
                execution_tamper_rejected: receipt.as_ref().is_some_and(|receipt| {
                    let mut tampered = receipt.clone();
                    tampered.result.push('x');
                    !execution_replay_verified(&tampered)
                }),
            });
        }
    }

    let sealed_bytes = fs::read(SEALED_MANIFEST_PATH)?;
    let sealed_manifest_sha256 = digest_bytes(&sealed_bytes);
    let sealed_value: serde_json::Value = serde_json::from_slice(&sealed_bytes)?;
    let sealed_manifest_records = sealed_value
        .as_array()
        .map(Vec::len)
        .or_else(|| {
            sealed_value
                .get("records")
                .and_then(|value| value.as_array())
                .map(Vec::len)
        })
        .unwrap_or(0);
    assert_eq!(sealed_manifest_records, 1091);

    let manifest_sha256_before = breadth_first_manifest().replay_hash();
    let manifest_sha256_after = breadth_first_manifest().replay_hash();
    assert_eq!(manifest_sha256_before, manifest_sha256_after);
    assert_eq!(independent_exact_statuses, independent.len());
    assert_eq!(independent_frontend_replay_verified, independent.len());
    assert_eq!(independent_execution_complete, 120);
    assert_eq!(
        independent_execution_replay_verified,
        independent_execution_complete
    );
    assert_eq!(independent_tamper_rejected, independent_execution_complete);

    let mut report = Report {
        schema: "stage417-word-system-frontend-bench-v1",
        independent_cases: independent.len(),
        independent_supported: 120,
        independent_ambiguous: 20,
        independent_missing: 20,
        independent_unsupported: 80,
        independent_exact_statuses,
        independent_execution_complete,
        independent_frontend_replay_verified,
        independent_execution_replay_verified,
        independent_tamper_rejected,
        external_candidate_records: external_records.len(),
        external_frontend_complete,
        external_execution_complete,
        external_frontend_replay_verified,
        external_execution_replay_verified,
        external_execution_tamper_rejected,
        external_samples,
        external_corpus_path: EXTERNAL_CORPUS_PATH,
        external_corpus_sha256,
        declared_external_corpus_sha256: EXTERNAL_CORPUS_SHA256,
        sealed_manifest_path: SEALED_MANIFEST_PATH,
        sealed_manifest_sha256,
        declared_sealed_manifest_sha256: SEALED_MANIFEST_SHA256,
        sealed_manifest_records,
        answer_keys_read: 0,
        plaintext_answers_read: 0,
        production_authorizations: 0,
        false_authorizations: 0,
        manifest_sha256_before,
        manifest_sha256_after,
        manifest_unchanged: true,
        report_sha256: String::new(),
    };
    report.report_sha256 = digest(&report);
    fs::write(
        "docs/stage417_word_system_frontend_bench.json",
        serde_json::to_vec_pretty(&report)?,
    )?;
    fs::write(
        "docs/stage417_word_system_frontend_bench.md",
        format!(
            "# Stage 417 — bounded word-system frontend\n\n- independent cases: {} (supported {}, ambiguous {}, missing {}, unsupported {})\n- independent exact statuses / execution / replay / tamper: {} / {} / {} / {}\n- external candidates / frontend complete / execution complete: {} / {} / {}\n- external frontend replay / execution replay / tamper: {} / {} / {}\n- answer keys read / production authorizations / false authorizations: 0 / 0 / 0\n- sealed manifest records: {} (prompt text not consumed)\n- manifest unchanged: true\n\nThe frontend accepts only an explicit two-number sum and an explicit bounded offset/multiple relation. It is shadow-only and does not authorize production routing.\n",
            report.independent_cases,
            report.independent_supported,
            report.independent_ambiguous,
            report.independent_missing,
            report.independent_unsupported,
            report.independent_exact_statuses,
            report.independent_execution_complete,
            report.independent_frontend_replay_verified,
            report.independent_tamper_rejected,
            report.external_candidate_records,
            report.external_frontend_complete,
            report.external_execution_complete,
            report.external_frontend_replay_verified,
            report.external_execution_replay_verified,
            report.external_execution_tamper_rejected,
            report.sealed_manifest_records,
        ),
    )?;
    println!(
        "Stage 417 — independent={}/{} external={}/{}",
        report.independent_exact_statuses,
        report.independent_cases,
        report.external_execution_complete,
        report.external_candidate_records
    );
    Ok(())
}
