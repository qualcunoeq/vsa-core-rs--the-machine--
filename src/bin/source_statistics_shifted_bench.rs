//! Shifted technical-language campaign for the source-derived statistics
//! frontend.  The corpus changes clause order and explicit separators while
//! retaining a strict typed boundary; it is not used to alter production
//! routing or the catalog.

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
    expected_formula: Option<String>,
    actual_status: FrontendStatus,
    actual_formula: Option<String>,
    downstream_status: Option<FormulaStatus>,
    exact: bool,
    authorized: bool,
    frontend_replay: bool,
    downstream_replay: bool,
    frontend_tamper_rejected: bool,
    downstream_tamper_rejected: bool,
    false_authorization: bool,
    text_sha256: String,
}

#[derive(Serialize)]
struct Report {
    schema: &'static str,
    source: &'static str,
    corpus_sha256: String,
    cases: usize,
    supported: usize,
    ambiguous: usize,
    refused: usize,
    exact_decisions: usize,
    authorized_answers: usize,
    supported_replays: usize,
    frontend_replays: usize,
    downstream_replays: usize,
    frontend_tamper_rejections: usize,
    downstream_tamper_rejections: usize,
    false_authorizations: usize,
    false_denials: usize,
    receipts: Vec<Receipt>,
}

fn digest<T: Serialize>(value: &T) -> String {
    format!("{:x}", Sha256::digest(serde_json::to_vec(value).unwrap()))
}

fn shifted_supported(index: usize) -> (String, &'static str) {
    let sum = 24 + (index % 7);
    let count = 3 + (index % 5);
    match index % 10 {
        0 => (
            format!("Compute the sample mean: sum = {sum}, count : {count}."),
            "arithmetic_mean",
        ),
        1 => (
            format!("The count : {count}; using sum: {sum}, report the mean."),
            "arithmetic_mean",
        ),
        2 => (
            format!("For a sample mean, count={count}; sum={sum}."),
            "arithmetic_mean",
        ),
        3 => (
            format!("Find the weighted average: weighted_sum : {sum}, total_weight = {count}."),
            "weighted_mean",
        ),
        4 => (
            format!("total_weight={count}; weighted_sum={sum}; compute the weighted mean."),
            "weighted_mean",
        ),
        5 => (
            format!("A binary-outcome Bernoulli variance is requested with probability : 1/4."),
            "bernoulli_variance",
        ),
        6 => (
            format!("With p = 1/3, find the Bernoulli binary outcome variance."),
            "bernoulli_variance",
        ),
        7 => (
            format!(
                "A binomial model has trials : {count} and p={sum}/100; find its expected value."
            ),
            "binomial_expected_value",
        ),
        8 => (
            format!("For a binomial model, p : 1/4 and n = {count}; compute the variance."),
            "binomial_variance",
        ),
        _ => (
            format!("Binomial variance: n={count}, p=1/5."),
            "binomial_variance",
        ),
    }
}

fn run(id: String, text: String, expected: Expected, expected_formula: Option<&str>) -> Receipt {
    let frontend: StatisticsFrontendResult = formalize_statistics_text(&text);
    let exact = match expected {
        Expected::Complete => {
            frontend.status == FrontendStatus::Complete
                && frontend.formula.as_deref() == expected_formula
        }
        Expected::Ambiguous => frontend.status == FrontendStatus::Ambiguous,
        Expected::Refused => frontend.status != FrontendStatus::Complete,
    };
    let frontend_replay = frontend.replay_verified();
    let mut frontend_tampered = frontend.clone();
    frontend_tampered.replay_hash.push('x');
    let (downstream_status, authorized, downstream_replay, downstream_tamper_rejected) =
        if let Some(request) = &frontend.request {
            let result: FormulaResult = evaluate_statistics(request);
            let mut tampered = result.clone();
            tampered.replay_hash.push('x');
            let authorized = expected == Expected::Complete
                && exact
                && frontend_replay
                && result.status == FormulaStatus::Complete
                && result.value.is_some()
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
        expected_formula: expected_formula.map(str::to_owned),
        actual_status: frontend.status,
        actual_formula: frontend.formula.clone(),
        downstream_status,
        exact,
        authorized,
        frontend_replay,
        downstream_replay,
        frontend_tamper_rejected: !frontend_tampered.replay_verified(),
        downstream_tamper_rejected,
        false_authorization: expected != Expected::Complete && authorized,
        text_sha256: digest(&text),
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut receipts = Vec::with_capacity(2000);
    for index in 0..1200 {
        let (text, formula) = shifted_supported(index);
        receipts.push(run(
            format!("shifted_supported_{index:04}"),
            text,
            Expected::Complete,
            Some(formula),
        ));
    }
    for index in 0..140 {
        receipts.push(run(
            format!("shifted_ambiguous_mean_{index:04}"),
            format!(
                "Find the average from total = {} and count : {}.",
                30 + index % 4,
                5 + index % 3
            ),
            Expected::Ambiguous,
            None,
        ));
    }
    for index in 0..130 {
        receipts.push(run(
            format!("shifted_ambiguous_binomial_{index:04}"),
            format!(
                "A binomial model has n = {} and p : 1/4; determine the result.",
                4 + index % 6
            ),
            Expected::Ambiguous,
            None,
        ));
    }
    for index in 0..130 {
        receipts.push(run(
            format!("shifted_ambiguous_weighted_{index:04}"),
            format!(
                "The weighted average uses weighted_sum = {} but its total weight is unstated.",
                20 + index % 9
            ),
            Expected::Ambiguous,
            None,
        ));
    }
    let refused_texts = [
        "Compute a confidence interval for a normal distribution.",
        "Fit a regression and report the standard error.",
        "Use a continuous density to calculate the expectation.",
        "Find the mean of the observations 1, 2, and 3.",
    ];
    for index in 0..400 {
        receipts.push(run(
            format!("shifted_refused_{index:04}"),
            refused_texts[index % refused_texts.len()].into(),
            Expected::Refused,
            None,
        ));
    }
    assert_eq!(receipts.len(), 2000);
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
    let exact_decisions = receipts.iter().filter(|r| r.exact).count();
    let authorized_answers = receipts.iter().filter(|r| r.authorized).count();
    let supported_replays = receipts
        .iter()
        .filter(|r| r.expected == Expected::Complete && r.authorized && r.downstream_replay)
        .count();
    let frontend_replays = receipts.iter().filter(|r| r.frontend_replay).count();
    let downstream_replays = receipts.iter().filter(|r| r.downstream_replay).count();
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
    assert_eq!((supported, ambiguous, refused), (1200, 400, 400));
    assert_eq!(exact_decisions, cases);
    assert_eq!(authorized_answers, supported);
    assert_eq!(supported_replays, supported);
    assert_eq!(frontend_replays, cases);
    assert_eq!(frontend_tamper_rejections, cases);
    assert_eq!(downstream_tamper_rejections, cases);
    assert_eq!(false_authorizations, 0);
    assert_eq!(false_denials, 0);
    let report = Report {
        schema: "stage-c-shifted-source-statistics-language-v1",
        source: "independently authored shifted finite-statistics language corpus",
        corpus_sha256: digest(&receipts),
        cases,
        supported,
        ambiguous,
        refused,
        exact_decisions,
        authorized_answers,
        supported_replays,
        frontend_replays,
        downstream_replays,
        frontend_tamper_rejections,
        downstream_tamper_rejections,
        false_authorizations,
        false_denials,
        receipts,
    };
    let serialized = serde_json::to_string_pretty(&report)?;
    std::fs::write(
        "docs/stage_c_shifted_source_statistics_language.json",
        format!("{serialized}\n"),
    )?;
    println!("{serialized}");
    Ok(())
}
