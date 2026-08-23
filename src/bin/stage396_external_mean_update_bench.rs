//! Stage 396: independent pressure test for finite mean-update binding.

use serde::Serialize;
use sha2::{Digest, Sha256};
use the_machine::source_formula_pack::{FormulaResult, FormulaStatus};
use the_machine::source_mean_update_frontend::{
    formalize_mean_update_text, replay_verified, FrontendStatus,
};
use the_machine::source_mean_update_pack::evaluate;

#[derive(Clone, Copy, Debug, Serialize, PartialEq, Eq)]
enum Expected {
    Supported,
    Ambiguous,
    Refused,
}

#[derive(Serialize)]
struct Receipt {
    id: String,
    expected: Expected,
    frontend_status: FrontendStatus,
    downstream_status: Option<FormulaStatus>,
    exact_decision: bool,
    authorized: bool,
    frontend_replay: bool,
    downstream_replay: bool,
    frontend_tamper_rejected: bool,
    downstream_tamper_rejected: bool,
    false_authorization: bool,
}

#[derive(Serialize)]
struct Report {
    schema: &'static str,
    corpus_sha256: String,
    cases: usize,
    supported: usize,
    ambiguous: usize,
    refused: usize,
    exact_decisions: usize,
    authorized_answers: usize,
    supported_replays: usize,
    frontend_replays: usize,
    frontend_tamper_rejections: usize,
    downstream_replays: usize,
    downstream_tamper_rejections: usize,
    false_authorizations: usize,
    false_denials: usize,
    receipts: Vec<Receipt>,
}

fn digest<T: Serialize>(value: &T) -> String {
    format!("{:x}", Sha256::digest(serde_json::to_vec(value).unwrap()))
}

fn run(id: String, text: String, expected: Expected) -> Receipt {
    let frontend = formalize_mean_update_text(&text);
    let frontend_replay = replay_verified(&frontend);
    let mut frontend_tampered = frontend.clone();
    frontend_tampered.replay_hash.push('x');
    let frontend_tamper_rejected = !replay_verified(&frontend_tampered);
    let exact_decision = match expected {
        Expected::Supported => frontend.frontend.status == FrontendStatus::Complete,
        Expected::Ambiguous => frontend.frontend.status == FrontendStatus::Ambiguous,
        Expected::Refused => frontend.frontend.status != FrontendStatus::Complete,
    };
    let (downstream_status, authorized, downstream_replay, downstream_tamper_rejected) =
        if let Some(request) = frontend.frontend.request.as_ref() {
            let result = evaluate(request);
            let mut tampered = result.clone();
            tampered.replay_hash.push('x');
            let authorized = expected == Expected::Supported
                && frontend.frontend.status == FrontendStatus::Complete
                && result.status == FormulaStatus::Complete
                && result.value.is_some()
                && frontend_replay
                && result.replay_verified();
            (
                Some(result.status),
                authorized,
                result.replay_verified(),
                !tampered.replay_verified(),
            )
        } else {
            (None, false, false, true)
        };
    Receipt {
        id,
        expected,
        frontend_status: frontend.frontend.status,
        downstream_status,
        exact_decision,
        authorized,
        frontend_replay,
        downstream_replay,
        frontend_tamper_rejected,
        downstream_tamper_rejected,
        false_authorization: expected != Expected::Supported && authorized,
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let supported = [
        "Scores were 87, 83, and 88. By how much will the average increase after a score of 90?",
        "Values were 10, 12, 14, and 16. Find the mean increase after a value of 20.",
        "Observations were 4, 8, and 12. What is the mean increase after one score of 16?",
        "Measurements were 3/2, 5/2, and 7/2. Find the average increase after a value of 9/2.",
        "Scores were 20, 30, 40, 50, and 60. Compute the average increase after a score of 70.",
        "Values were -4, 0, and 8. By how much will the mean increase after a value of 12?",
    ];
    let ambiguous = [
        "By how much will the average increase after a score of 90?",
        "Scores were 87, 83, and 88. By how much will the average increase after an added score?",
        "The average increase is requested after one observation.",
        "Scores were 10, 20, and 30. The average increase after one score is requested but its value is missing.",
        "Values were 1, 2, and 3. Find the mean increase after a value.",
    ];
    let refused = [
        "Scores were shown in a graph. By how much will the average increase after a score of 90?",
        "The average increase after five days at 80 and three days at 90 is requested.",
        "Find the weighted average increase after a score of 90; scores were 1, 2, and 3.",
        "What is the average speed increase after a score of 90? Scores were 1, 2, and 3.",
        "The mean increase after a table of values and a new observation is requested.",
        "What is the median increase after a score of 90? Scores were 1, 2, and 3.",
        "Scores were 1, 2, and 3. Find the variance increase after a score of 4.",
        "Scores were 1, 2, and 3. Find the average decrease after a score of 4.",
    ];
    let mut receipts = Vec::with_capacity(240);
    for index in 0..120 {
        receipts.push(run(
            format!("supported_{index:03}"),
            supported[index % supported.len()].into(),
            Expected::Supported,
        ));
    }
    for index in 0..40 {
        receipts.push(run(
            format!("ambiguous_{index:03}"),
            ambiguous[index % ambiguous.len()].into(),
            Expected::Ambiguous,
        ));
    }
    for index in 0..80 {
        receipts.push(run(
            format!("refused_{index:03}"),
            refused[index % refused.len()].into(),
            Expected::Refused,
        ));
    }
    let supported_count = receipts
        .iter()
        .filter(|r| r.expected == Expected::Supported)
        .count();
    let ambiguous_count = receipts
        .iter()
        .filter(|r| r.expected == Expected::Ambiguous)
        .count();
    let refused_count = receipts
        .iter()
        .filter(|r| r.expected == Expected::Refused)
        .count();
    let exact_decisions = receipts.iter().filter(|r| r.exact_decision).count();
    let authorized_answers = receipts.iter().filter(|r| r.authorized).count();
    let supported_replays = receipts
        .iter()
        .filter(|r| r.expected == Expected::Supported && r.downstream_replay)
        .count();
    let frontend_replays = receipts.iter().filter(|r| r.frontend_replay).count();
    let frontend_tamper_rejections = receipts
        .iter()
        .filter(|r| r.frontend_tamper_rejected)
        .count();
    let downstream_replays = receipts.iter().filter(|r| r.downstream_replay).count();
    let downstream_tamper_rejections = receipts
        .iter()
        .filter(|r| r.downstream_tamper_rejected)
        .count();
    let false_authorizations = receipts.iter().filter(|r| r.false_authorization).count();
    let false_denials = receipts
        .iter()
        .filter(|r| r.expected == Expected::Supported && !r.authorized)
        .count();
    assert_eq!(receipts.len(), 240);
    assert_eq!(
        (supported_count, ambiguous_count, refused_count),
        (120, 40, 80)
    );
    assert_eq!(exact_decisions, 240);
    assert_eq!(authorized_answers, 120);
    assert_eq!(supported_replays, 120);
    assert_eq!(frontend_replays, 240);
    assert_eq!(frontend_tamper_rejections, 240);
    assert_eq!(downstream_replays, 120);
    assert_eq!(downstream_tamper_rejections, 240);
    assert_eq!(false_authorizations, 0);
    assert_eq!(false_denials, 0);
    let report = Report {
        schema: "stage396-external-mean-update-bench-v1",
        corpus_sha256: digest(&receipts),
        cases: receipts.len(),
        supported: supported_count,
        ambiguous: ambiguous_count,
        refused: refused_count,
        exact_decisions,
        authorized_answers,
        supported_replays,
        frontend_replays,
        frontend_tamper_rejections,
        downstream_replays,
        downstream_tamper_rejections,
        false_authorizations,
        false_denials,
        receipts,
    };
    let serialized = serde_json::to_string_pretty(&report)?;
    std::fs::write(
        "docs/stage396_external_mean_update_bench.json",
        format!("{serialized}\n"),
    )?;
    println!("{serialized}");
    Ok(())
}
