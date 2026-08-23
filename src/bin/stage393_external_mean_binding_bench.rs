//! Stage 393: pressure-test the source-backed finite-mean sum/count binding.
//!
//! The corpus is independent of the frozen external exam and answer-key
//! blind. It validates only the generic `sum`/`count` lowering into the
//! source-declared arithmetic-mean record.

use serde::Serialize;
use sha2::{Digest, Sha256};
use the_machine::source_formula_pack::{FormulaResult, FormulaStatus};
use the_machine::source_statistics_frontend::{
    formalize_finite_list_mean_text, FrontendStatus, StatisticsFrontendResult,
};
use the_machine::source_statistics_pack::evaluate_statistics;

#[derive(Clone, Copy, Debug, Serialize, PartialEq, Eq)]
enum Expected {
    Complete,
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
    value_replay: bool,
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
    supported_values_replayed: usize,
    frontend_replay_verified: usize,
    frontend_tamper_rejections: usize,
    downstream_tamper_rejections: usize,
    false_authorizations: usize,
    false_denials: usize,
    receipts: Vec<Receipt>,
}

fn digest<T: Serialize>(value: &T) -> String {
    format!("{:x}", Sha256::digest(serde_json::to_vec(value).unwrap()))
}

fn run(id: String, text: String, expected: Expected) -> Receipt {
    let frontend: StatisticsFrontendResult = formalize_finite_list_mean_text(&text);
    let exact_decision = match expected {
        Expected::Complete => frontend.status == FrontendStatus::Complete,
        Expected::Ambiguous => frontend.status == FrontendStatus::Ambiguous,
        Expected::Refused => frontend.status != FrontendStatus::Complete,
    };
    let frontend_replay = frontend.replay_verified();
    let mut frontend_tampered = frontend.clone();
    frontend_tampered.replay_hash.push('x');
    let (downstream_status, authorized, value_replay, downstream_tamper_rejected) =
        if let Some(request) = &frontend.request {
            let result: FormulaResult = evaluate_statistics(request);
            let mut tampered = result.clone();
            tampered.replay_hash.push('x');
            let authorized = frontend.status == FrontendStatus::Complete
                && result.status == FormulaStatus::Complete
                && result.value.is_some()
                && frontend_replay
                && result.replay_verified();
            (
                Some(result.status),
                authorized,
                authorized && result.replay_verified(),
                !tampered.replay_verified(),
            )
        } else {
            (None, false, false, true)
        };
    Receipt {
        id,
        expected,
        frontend_status: frontend.status,
        downstream_status,
        exact_decision,
        authorized,
        frontend_replay,
        value_replay,
        frontend_tamper_rejected: !frontend_tampered.replay_verified(),
        downstream_tamper_rejected,
        false_authorization: expected != Expected::Complete && authorized,
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let supported = [
        "The sum of four numbers is one-half. What is the mean of the four numbers?",
        "The sum of 6 values is 30. Find their average.",
        "The sum of seven terms is 105. Compute their arithmetic mean.",
        "The sum of ten observations is 3/2. What is their mean?",
        "The sum of 5 measurements is -20. Find the average.",
        "The sum of twenty scores is 100. What is the mean?",
    ];
    let ambiguous = [
        "The sum of x is 30. Find the mean.",
        "The sum of four probabilities is one-half. Find the mean.",
        "The sum of several values is 30. Find the average.",
    ];
    let refused = [
        "Find the arithmetic mean of the integers from -4 through 5.",
        "Find the mean of x + 8, 15, and 2x.",
        "What is the average speed over the trip?",
        "Compute the median of four numbers.",
        "The sum of four numbers is 10. Find the median.",
        "The sum of four numbers is 10. Find the variance.",
        "Compute a confidence interval for a normal distribution.",
        "Find the weighted average without declared weights.",
    ];

    let mut receipts = Vec::with_capacity(240);
    for index in 0..120 {
        receipts.push(run(
            format!("supported_{index:03}"),
            supported[index % supported.len()].into(),
            Expected::Complete,
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

    let supported = receipts
        .iter()
        .filter(|receipt| receipt.expected == Expected::Complete)
        .count();
    let ambiguous = receipts
        .iter()
        .filter(|receipt| receipt.expected == Expected::Ambiguous)
        .count();
    let refused = receipts
        .iter()
        .filter(|receipt| receipt.expected == Expected::Refused)
        .count();
    let exact_decisions = receipts
        .iter()
        .filter(|receipt| receipt.exact_decision)
        .count();
    let authorized_answers = receipts.iter().filter(|receipt| receipt.authorized).count();
    let supported_values_replayed = receipts
        .iter()
        .filter(|receipt| receipt.expected == Expected::Complete && receipt.value_replay)
        .count();
    let frontend_replay_verified = receipts
        .iter()
        .filter(|receipt| receipt.frontend_replay)
        .count();
    let frontend_tamper_rejections = receipts
        .iter()
        .filter(|receipt| receipt.frontend_tamper_rejected)
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
        .filter(|receipt| receipt.expected == Expected::Complete && !receipt.authorized)
        .count();

    assert_eq!(receipts.len(), 240);
    assert_eq!((supported, ambiguous, refused), (120, 40, 80));
    assert_eq!(
        (
            exact_decisions,
            authorized_answers,
            supported_values_replayed,
            frontend_replay_verified,
            frontend_tamper_rejections,
            downstream_tamper_rejections,
            false_authorizations,
            false_denials,
        ),
        (240, 120, 120, 240, 240, 240, 0, 0)
    );

    let report = Report {
        schema: "stage393-external-mean-binding-bench-v1",
        corpus_sha256: digest(&receipts),
        cases: receipts.len(),
        supported,
        ambiguous,
        refused,
        exact_decisions,
        authorized_answers,
        supported_values_replayed,
        frontend_replay_verified,
        frontend_tamper_rejections,
        downstream_tamper_rejections,
        false_authorizations,
        false_denials,
        receipts,
    };
    let serialized = serde_json::to_string_pretty(&report)?;
    std::fs::write(
        "docs/stage393_external_mean_binding_bench.json",
        format!("{serialized}\n"),
    )?;
    println!("{serialized}");
    Ok(())
}
