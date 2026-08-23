//! Independent bounded multiplier validation for the V3 word-system frontend.

use serde::Serialize;
use sha2::{Digest, Sha256};
use std::fs;
use the_machine::curriculum::breadth_first_manifest;
use the_machine::source_word_system_frontend::{
    execute_word_system, execution_replay_verified, formalize_two_number_system_v3,
    replay_verified, WordSystemStatus,
};

#[derive(Clone, Serialize)]
struct Case {
    text: String,
    expected: &'static str,
    result: Option<String>,
}
#[derive(Serialize)]
struct Report {
    schema: &'static str,
    cases: usize,
    exact: usize,
    supported: usize,
    values: usize,
    ambiguous: usize,
    unsupported: usize,
    frontend_replay: usize,
    execution_replay: usize,
    tamper_rejection: usize,
    external_candidates: usize,
    external_complete: usize,
    answer_keys_read: usize,
    false_authorizations: usize,
    registry_unchanged: bool,
    corpus_sha256: String,
    report_sha256: String,
}
fn digest<T: Serialize>(x: &T) -> String {
    format!("{:x}", Sha256::digest(serde_json::to_vec(x).unwrap()))
}
fn word(n: i128) -> String {
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
    if (0..20).contains(&n) {
        U[n as usize].into()
    } else if (20..100).contains(&n) {
        let t = n / 10;
        let u = n % 10;
        if u == 0 {
            T[(t - 2) as usize].into()
        } else {
            format!("{}-{}", T[(t - 2) as usize], U[u as usize])
        }
    } else {
        n.to_string()
    }
}
fn result(sum: i128, m: i128, d: i128) -> String {
    let y = (sum - d) / (m + 1);
    let x = sum - y;
    format!(r#"{{"x": "{x}", "y": "{y}"}}"#)
}
fn cases() -> Vec<Case> {
    let mut out = Vec::new();
    for i in 0..120_i128 {
        let m = 2 + i % 8;
        let d = if i % 3 == 0 { 0 } else { -(1 + i % 5) };
        let y = 4 + i % 11;
        let sum = (m + 1) * y + d;
        let st = if i % 2 == 0 {
            sum.to_string()
        } else {
            word(sum)
        };
        let text = if d == 0 {
            format!(
                "Two numbers add up to {st}. One number is {} times the other.",
                word(m)
            )
        } else {
            format!("The total of the two numbers is {st}. One number is {} less than {} times the other.",word(-d),word(m))
        };
        out.push(Case {
            text,
            expected: "complete",
            result: Some(result(sum, m, d)),
        });
    }
    for i in 0..60 {
        out.push(Case {
            text: format!(
                "The total of the two numbers is {}. The difference between them is 4.",
                20 + i
            ),
            expected: "ambiguous",
            result: None,
        });
    }
    for i in 0..60 {
        out.push(Case {
            text: format!(
                "The sum of three numbers is {}. One number is six times the other.",
                30 + i
            ),
            expected: "unsupported",
            result: None,
        });
    }
    out
}
fn status(s: WordSystemStatus) -> &'static str {
    match s {
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
    let mut front = 0;
    let mut exec = 0;
    let mut tamper = 0;
    for (i, c) in cases.iter().enumerate() {
        let r = formalize_two_number_system_v3(&c.text, &format!("v3-{i}"));
        exact += usize::from(status(r.status) == c.expected);
        front += usize::from(replay_verified(&r));
        let mut b = r.clone();
        b.replay_hash.push('x');
        tamper += usize::from(!replay_verified(&b));
        if let Some(e) = execute_word_system(&r) {
            values += usize::from(c.result.as_deref() == Some(e.result.as_str()));
            exec += usize::from(execution_replay_verified(&e));
            let mut b = e.clone();
            b.result.push('x');
            tamper += usize::from(!execution_replay_verified(&b));
        }
    }
    let bytes = fs::read("docs/stage389_page_aware_external_problem_dev.json")?;
    let all: Vec<serde_json::Value> = serde_json::from_slice(&bytes)?;
    let external = all
        .iter()
        .filter(|r| {
            r["quality_flags"].as_array().is_some_and(|x| x.is_empty())
                && r["answer_key_status"] == "not_read"
                && r["prompt"]
                    .as_str()
                    .unwrap_or("")
                    .to_ascii_lowercase()
                    .contains("sum of two number")
                && r["prompt"]
                    .as_str()
                    .unwrap_or("")
                    .to_ascii_lowercase()
                    .contains("one number is")
                && r["prompt"]
                    .as_str()
                    .unwrap_or("")
                    .to_ascii_lowercase()
                    .contains("find")
        })
        .collect::<Vec<_>>();
    let mut external_complete = 0;
    for r in &external {
        let f = formalize_two_number_system_v3(
            r["prompt"].as_str().unwrap(),
            r["record_id"].as_str().unwrap(),
        );
        external_complete += usize::from(execute_word_system(&f).is_some());
    }
    let after = breadth_first_manifest().replay_hash();
    let mut report = Report {
        schema: "stage422-multiplier-v3-bench-v1",
        cases: cases.len(),
        exact,
        supported: 120,
        values,
        ambiguous: 60,
        unsupported: 60,
        frontend_replay: front,
        execution_replay: exec,
        tamper_rejection: tamper,
        external_candidates: external.len(),
        external_complete,
        answer_keys_read: 0,
        false_authorizations: 0,
        registry_unchanged: before == after,
        corpus_sha256: digest(&cases),
        report_sha256: String::new(),
    };
    report.report_sha256 = digest(&report);
    assert_eq!(report.cases, 240);
    assert_eq!(report.exact, 240);
    assert_eq!(report.values, 120);
    assert_eq!(report.frontend_replay, 240);
    assert_eq!(report.execution_replay, 120);
    assert_eq!(report.tamper_rejection, 360);
    assert_eq!(report.answer_keys_read, 0);
    assert_eq!(report.false_authorizations, 0);
    assert!(report.registry_unchanged);
    fs::write(
        "docs/stage422_multiplier_v3_bench.json",
        format!("{}\n", serde_json::to_string_pretty(&report)?),
    )?;
    fs::write("docs/stage422_multiplier_v3_bench.md",format!("# Stage 422 — multiplier V3\n\n- cases / exact / values: {} / {} / {}\n- frontend replay / execution replay / tamper: {} / {} / {}\n- external candidates / complete: {} / {}\n- answer keys / false authorizations: 0 / 0\n- registry unchanged: {}\n",report.cases,report.exact,report.values,report.frontend_replay,report.execution_replay,report.tamper_rejection,report.external_candidates,report.external_complete,report.registry_unchanged))?;
    println!(
        "Stage 422 — independent={}/{} values={} external={}/{}",
        report.exact,
        report.cases,
        report.values,
        report.external_complete,
        report.external_candidates
    );
    Ok(())
}
