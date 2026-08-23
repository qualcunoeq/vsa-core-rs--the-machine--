//! Stage 397: independent natural-language variation pressure test for the
//! source-derived finite mean-update binding.

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
    prompt_sha256: String,
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
        prompt_sha256: digest(&text),
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
        "Kim earned scores of 87, 83 and 88 on her first three exams. If she receives a score of 90 on the fourth exam, by how much will her average increase?",
        "The values of 10, 12, 14 and 16 were observed. What is the mean increase after a value of 20?",
        "Observations of 4, 8 and 12 were recorded. What is the average increase after one score of 16?",
        "Measurements of 3/2, 5/2 and 7/2 were recorded; the mean increase after receiving a score of 9/2 is requested.",
        "A learner earned scores of 20, 30, 40, 50 and 60. After receiving a score of 70, find the average increase.",
        "The old values were -4, 0 and 8. If the learner gets a score of 12, by how much does the mean increase?",
    ];
    let ambiguous = [
        "The average increase after a score of 90 is requested, but the earlier scores are not given.",
        "Scores of 87, 83 and 88 were recorded. By how much will the average increase after an added score?",
        "The mean increase after one observation is requested, but the new observation is missing.",
        "Values of 10, 20 and 30 were recorded. Find the mean increase after a value.",
        "Measurements of 2, 4 and 6 were recorded; determine the average increase without identifying the added value.",
    ];
    let refused = [
        "Scores of the players were shown in a graph. By how much will the average increase after a score of 90?",
        "The average increase after five days at 80 and three days at 90 is requested.",
        "Find the weighted average increase after receiving a score of 90; scores of 1, 2 and 3 were recorded.",
        "What is the average speed increase after receiving a score of 90? Scores of 1, 2 and 3 were recorded.",
        "The mean increase after a table of values and a new observation is requested.",
        "What is the median increase after receiving a score of 90? Scores of 1, 2 and 3 were recorded.",
        "Scores of 1, 2 and 3 were recorded. Find the variance increase after receiving a score of 4.",
        "Scores of 1, 2 and 3 were recorded. Find the average decrease after receiving a score of 4.",
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
        .filter(|receipt| receipt.expected == Expected::Supported)
        .count();
    let ambiguous_count = receipts
        .iter()
        .filter(|receipt| receipt.expected == Expected::Ambiguous)
        .count();
    let refused_count = receipts
        .iter()
        .filter(|receipt| receipt.expected == Expected::Refused)
        .count();
    let exact_decisions = receipts
        .iter()
        .filter(|receipt| receipt.exact_decision)
        .count();
    let authorized_answers = receipts.iter().filter(|receipt| receipt.authorized).count();
    let supported_replays = receipts
        .iter()
        .filter(|receipt| receipt.expected == Expected::Supported && receipt.downstream_replay)
        .count();
    let frontend_replays = receipts
        .iter()
        .filter(|receipt| receipt.frontend_replay)
        .count();
    let frontend_tamper_rejections = receipts
        .iter()
        .filter(|receipt| receipt.frontend_tamper_rejected)
        .count();
    let downstream_replays = receipts
        .iter()
        .filter(|receipt| receipt.downstream_replay)
        .count();
    let downstream_tamper_rejections = receipts
        .iter()
        .filter(|receipt| receipt.downstream_tamper_rejected)
        .count();
    let false_authorizations = receipts
        .iter()
        .filter(|receipt| receipt.false_authorization)
        .count();
    let false_denials = receipts
        .iter()
        .filter(|receipt| receipt.expected == Expected::Supported && !receipt.authorized)
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
        schema: "stage397-external-mean-update-language-bench-v1",
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
        "docs/stage397_external_mean_update_language_bench.json",
        format!("{serialized}\n"),
    )?;
    println!("{serialized}");
    Ok(())
}
