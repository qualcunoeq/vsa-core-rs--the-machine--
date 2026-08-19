//! Stage C/D integration benchmark for the finite-statistics frontend.

use serde::Serialize;
use sha2::{Digest, Sha256};
use the_machine::source_formula_pack::{FormulaResult, FormulaStatus};
use the_machine::source_statistics_frontend::{
    formalize_statistics_text, FrontendStatus, StatisticsFrontendResult,
};
use the_machine::source_statistics_pack::evaluate_statistics;

#[derive(Clone, Copy, Serialize, PartialEq, Eq)]
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
    exact_frontend: bool,
    authorized: bool,
    value_replay: bool,
    frontend_replay: bool,
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
    exact_frontend_decisions: usize,
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
    let frontend: StatisticsFrontendResult = formalize_statistics_text(&text);
    let exact_frontend = match expected {
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
        exact_frontend,
        authorized,
        value_replay,
        frontend_replay,
        frontend_tamper_rejected: !frontend_tampered.replay_verified(),
        downstream_tamper_rejected,
        false_authorization: expected != Expected::Complete && authorized,
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let supported_texts = [
        "Compute the mean: sum=30 count=5.",
        "Find the weighted average, weighted_sum=30 total_weight=5.",
        "Bernoulli binary outcome variance with p=1/4.",
        "For a binomial model, find the expected value for n=8 p=1/4.",
        "For a binomial model, find the variance for n=8 p=1/4.",
    ];
    let mut receipts = Vec::with_capacity(240);
    for index in 0..120 {
        receipts.push(run(
            format!("supported_{index:03}"),
            supported_texts[index % supported_texts.len()].into(),
            Expected::Complete,
        ));
    }
    for index in 0..20 {
        receipts.push(run(
            format!("ambiguous_mean_{index:03}"),
            "Find the average from total=30 and count=5.".into(),
            Expected::Ambiguous,
        ));
    }
    for index in 0..20 {
        receipts.push(run(
            format!("ambiguous_binomial_{index:03}"),
            "For a binomial model with n=8 p=1/4, determine the result.".into(),
            Expected::Ambiguous,
        ));
    }
    for index in 0..20 {
        receipts.push(run(
            format!("unsupported_{index:03}"),
            "Compute a confidence interval for a normal distribution.".into(),
            Expected::Refused,
        ));
    }
    for index in 0..20 {
        receipts.push(run(
            format!("missing_labels_{index:03}"),
            "Find the mean of these observations: 1, 2, 3.".into(),
            Expected::Refused,
        ));
    }
    for index in 0..20 {
        receipts.push(run(
            format!("missing_binomial_input_{index:03}"),
            "For a binomial model, find the expected value for n=8.".into(),
            Expected::Refused,
        ));
    }
    for index in 0..20 {
        receipts.push(run(
            format!("unsupported_regression_{index:03}"),
            "Fit a regression and report the standard error.".into(),
            Expected::Refused,
        ));
    }
    assert_eq!(receipts.len(), 240);
    let cases = receipts.len();
    let supported = receipts
        .iter()
        .filter(|r| r.expected == Expected::Complete)
        .count();
    let ambiguous = receipts
        .iter()
        .filter(|r| r.expected == Expected::Ambiguous)
        .count();
    let refused = receipts
        .iter()
        .filter(|r| r.expected == Expected::Refused)
        .count();
    let exact_frontend_decisions = receipts.iter().filter(|r| r.exact_frontend).count();
    let authorized_answers = receipts.iter().filter(|r| r.authorized).count();
    let supported_values_replayed = receipts
        .iter()
        .filter(|r| r.expected == Expected::Complete && r.value_replay)
        .count();
    let frontend_replay_verified = receipts.iter().filter(|r| r.frontend_replay).count();
    let frontend_tamper_rejections = receipts
        .iter()
        .filter(|r| r.frontend_tamper_rejected)
        .count();
    let downstream_tamper_rejections = receipts
        .iter()
        .filter(|r| r.downstream_tamper_rejected)
        .count();
    let false_authorizations = receipts.iter().filter(|r| r.false_authorization).count();
    let false_denials = receipts
        .iter()
        .filter(|r| r.expected == Expected::Complete && !r.authorized)
        .count();
    assert_eq!((supported, ambiguous, refused), (120, 40, 80));
    assert_eq!(
        (
            exact_frontend_decisions,
            authorized_answers,
            supported_values_replayed,
            frontend_replay_verified,
            frontend_tamper_rejections,
            downstream_tamper_rejections,
            false_authorizations,
            false_denials
        ),
        (240, 120, 120, 240, 240, 240, 0, 0)
    );
    let report = Report {
        schema: "stage-c-d-source-statistics-language-frontend-v1",
        corpus_sha256: digest(&receipts),
        cases,
        supported,
        ambiguous,
        refused,
        exact_frontend_decisions,
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
        "docs/stage_c_d_source_statistics_frontend.json",
        format!("{serialized}\n"),
    )?;
    println!("{serialized}");
    Ok(())
}
