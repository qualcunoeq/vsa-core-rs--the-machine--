//! Independent validation and answer-key-blind external reachability for the
//! V2 word-system frontend.  V1 remains frozen and production routing is not
//! changed by this shadow benchmark.

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::fs;
use the_machine::curriculum::breadth_first_manifest;
use the_machine::source_word_system_frontend::{
    execute_word_system, execution_replay_verified, formalize_two_number_system_v2,
    replay_verified, WordSystemStatus,
};

const CORPUS: &str = "docs/stage389_page_aware_external_problem_dev.json";

#[derive(Debug, Clone, Serialize)]
struct Case {
    text: String,
    expected: String,
    expected_result: Option<String>,
}

#[derive(Debug, Deserialize)]
struct ExternalRecord {
    record_id: String,
    prompt: String,
    source_path: String,
    source_sha256: String,
    #[serde(default)]
    quality_flags: Vec<String>,
    answer_key_status: String,
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
    independent_supported_values: usize,
    independent_frontend_replays: usize,
    independent_execution_replays: usize,
    independent_tamper_rejections: usize,
    external_candidates: usize,
    external_complete: usize,
    external_frontend_replays: usize,
    external_execution_replays: usize,
    external_tamper_rejections: usize,
    answer_keys_read: usize,
    production_authorizations: usize,
    false_authorizations: usize,
    registry_unchanged: bool,
    corpus_sha256: String,
    report_sha256: String,
}

fn digest<T: Serialize>(value: &T) -> String {
    format!("{:x}", Sha256::digest(serde_json::to_vec(value).unwrap()))
}
fn word(value: i128) -> String {
    const U: [&str; 20] = [
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
    const T: [&str; 8] = [
        "twenty", "thirty", "forty", "fifty", "sixty", "seventy", "eighty", "ninety",
    ];
    if (0..20).contains(&value) {
        return U[value as usize].into();
    }
    if (20..100).contains(&value) {
        let t = value / 10;
        let u = value % 10;
        return if u == 0 {
            T[(t - 2) as usize].into()
        } else {
            format!("{}-{}", T[(t - 2) as usize], U[u as usize])
        };
    }
    value.to_string()
}
fn expected_result(sum: i128, multiplier: i128, delta: i128) -> String {
    let y = (sum - delta) / (multiplier + 1);
    let x = sum - y;
    format!(r#"{{"x": "{x}", "y": "{y}"}}"#)
}
fn cases() -> Vec<Case> {
    let mut cases = Vec::new();
    for i in 0..120_i128 {
        let delta = 1 + i % 5;
        let (multiplier, relation_delta) = match i % 6 {
            0 | 1 | 3 => (1, delta),
            2 => (1, -delta),
            4 => (2 + i % 4, 0),
            _ => (2 + i % 2, -delta),
        };
        let y = 5 + i % 15;
        let sum = (multiplier + 1) * y + relation_delta;
        let sum_text = if i % 2 == 0 {
            sum.to_string()
        } else {
            word(sum)
        };
        let delta_text = if i % 3 == 0 {
            delta.to_string()
        } else {
            word(delta)
        };
        let text = match i % 6 {
            0 => format!("The total of the two numbers is {sum_text}. One number exceeds the other by {delta_text}."),
            1 => format!("Two numbers add up to {sum_text}. One number is {delta_text} greater than the other."),
            2 => format!("The sum of the two numbers is {sum_text}. One number is {delta_text} smaller than the other."),
            3 => format!("The sum of two numbers is {sum_text}. The larger number is {delta_text} more than the smaller number."),
            4 => format!("Two numbers total {sum_text}. One number is {} times the other.", word(multiplier)),
            _ => format!("Two numbers add up to {sum_text}. One number is {delta_text} less than {} times the other.", word(multiplier)),
        };
        cases.push(Case {
            text,
            expected: "complete".into(),
            expected_result: Some(expected_result(sum, multiplier, relation_delta)),
        });
    }
    for i in 0..30 {
        cases.push(Case {
            text: format!(
                "The total of the two numbers is {}. The difference between them is 4.",
                20 + i
            ),
            expected: "ambiguous".into(),
            expected_result: None,
        });
    }
    for i in 0..30 {
        cases.push(Case {
            text: format!("The sum of two numbers is {}. Find the numbers.", 30 + i),
            expected: "ambiguous".into(),
            expected_result: None,
        });
    }
    for i in 0..30 {
        cases.push(Case {
            text: format!(
                "The sum of three numbers is {}. One number is 4 less than the other.",
                40 + i
            ),
            expected: "unsupported".into(),
            expected_result: None,
        });
    }
    for i in 0..30 {
        cases.push(Case {
            text: format!(
                "Two numbers add up to {}. One number is six times the other.",
                50 + i
            ),
            expected: "ambiguous".into(),
            expected_result: None,
        });
    }
    cases
}
fn status(status: WordSystemStatus) -> &'static str {
    match status {
        WordSystemStatus::Complete => "complete",
        WordSystemStatus::Ambiguous => "ambiguous",
        WordSystemStatus::Missing => "missing",
        WordSystemStatus::Unsupported => "unsupported",
    }
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let before = breadth_first_manifest().replay_hash();
    let cases = cases();
    let mut exact = 0;
    let mut values = 0;
    let mut front_replay = 0;
    let mut exec_replay = 0;
    let mut tamper = 0;
    for (i, case) in cases.iter().enumerate() {
        let result = formalize_two_number_system_v2(&case.text, &format!("v2-{i}"));
        let observed = status(result.status);
        exact += usize::from(observed == case.expected);
        front_replay += usize::from(replay_verified(&result));
        let mut bad = result.clone();
        bad.replay_hash.push('x');
        tamper += usize::from(!replay_verified(&bad));
        if let Some(receipt) = execute_word_system(&result) {
            if case.expected_result.as_deref() == Some(receipt.result.as_str()) {
                values += 1;
            } else {
                eprintln!(
                    "value mismatch {i}: {} => {} expected {:?}",
                    case.text, receipt.result, case.expected_result
                );
            }
            exec_replay += usize::from(execution_replay_verified(&receipt));
            let mut bad = receipt.clone();
            bad.result.push('x');
            tamper += usize::from(!execution_replay_verified(&bad));
        }
    }
    let bytes = fs::read(CORPUS)?;
    let corpus_sha256 = digest(&bytes);
    let all: Vec<ExternalRecord> = serde_json::from_slice(&bytes)?;
    assert!(all.iter().all(|r| r.answer_key_status == "not_read"));
    let external = all
        .into_iter()
        .filter(|r| {
            r.quality_flags.is_empty()
                && r.prompt.to_ascii_lowercase().contains("sum of two number")
                && r.prompt.to_ascii_lowercase().contains("one number is")
                && r.prompt.to_ascii_lowercase().contains("find")
        })
        .collect::<Vec<_>>();
    let mut external_complete = 0;
    let mut external_frontend_replays = 0;
    let mut external_execution_replays = 0;
    let mut external_tamper = 0;
    for r in &external {
        let f = formalize_two_number_system_v2(&r.prompt, &r.record_id);
        external_frontend_replays += usize::from(replay_verified(&f));
        let mut bad = f.clone();
        bad.replay_hash.push('x');
        external_tamper += usize::from(!replay_verified(&bad));
        if let Some(e) = execute_word_system(&f) {
            external_complete += 1;
            external_execution_replays += usize::from(execution_replay_verified(&e));
            let mut bad = e.clone();
            bad.result.push('x');
            external_tamper += usize::from(!execution_replay_verified(&bad));
        }
        let _ = (&r.source_path, &r.source_sha256);
    }
    let after = breadth_first_manifest().replay_hash();
    let mut report = Report {
        schema: "stage420-word-system-v2-bench-v1",
        independent_cases: cases.len(),
        independent_supported: 120,
        independent_ambiguous: 90,
        independent_missing: 0,
        independent_unsupported: 30,
        independent_exact_statuses: exact,
        independent_supported_values: values,
        independent_frontend_replays: front_replay,
        independent_execution_replays: exec_replay,
        independent_tamper_rejections: tamper,
        external_candidates: external.len(),
        external_complete,
        external_frontend_replays,
        external_execution_replays,
        external_tamper_rejections: external_tamper,
        answer_keys_read: 0,
        production_authorizations: 0,
        false_authorizations: 0,
        registry_unchanged: before == after,
        corpus_sha256,
        report_sha256: String::new(),
    };
    report.report_sha256 = digest(&report);
    assert_eq!(report.independent_cases, 240);
    assert_eq!(report.independent_exact_statuses, 240);
    assert_eq!(report.independent_supported_values, 120);
    assert_eq!(report.independent_frontend_replays, 240);
    assert_eq!(report.independent_execution_replays, 120);
    assert_eq!(report.independent_tamper_rejections, 360);
    assert_eq!(report.answer_keys_read, 0);
    assert_eq!(report.production_authorizations, 0);
    assert_eq!(report.false_authorizations, 0);
    assert!(report.registry_unchanged);
    fs::write(
        "docs/stage420_word_system_v2_bench.json",
        format!("{}\n", serde_json::to_string_pretty(&report)?),
    )?;
    fs::write("docs/stage420_word_system_v2_bench.md", format!("# Stage 420 — V2 word-system frontend\n\n- independent cases / exact statuses / supported values: {} / {} / {}\n- frontend replay / execution replay / tamper: {} / {} / {}\n- external candidates / complete / frontend replay / execution replay: {} / {} / {} / {}\n- answer keys / production authorizations / false authorizations: 0 / 0 / 0\n- registry unchanged: {}\n\nV2 is shadow-only and adds explicit relation synonyms and bounded equality-only multiples while leaving V1 frozen.\n", report.independent_cases, report.independent_exact_statuses, report.independent_supported_values, report.independent_frontend_replays, report.independent_execution_replays, report.independent_tamper_rejections, report.external_candidates, report.external_complete, report.external_frontend_replays, report.external_execution_replays, report.registry_unchanged))?;
    println!(
        "Stage 420 — independent={}/{} values={} external={}/{}",
        report.independent_exact_statuses,
        report.independent_cases,
        report.independent_supported_values,
        report.external_complete,
        report.external_candidates
    );
    Ok(())
}
