//! Controlled technical-language benchmark for the source-derived regression
//! catalog.  Frontend success is not authorization: the typed request still
//! passes through the generic source catalog runtime.

use serde::Serialize;
use sha2::{Digest, Sha256};
use the_machine::probability_pack::Rational;
use the_machine::source_formula_pack::{FormulaResult, FormulaStatus};
use the_machine::source_regression_pack::evaluate_regression;
use the_machine::source_regression_pack::source_regression_frontend::{
    formalize_regression_text, FrontendStatus,
};

#[derive(Clone, Copy, Serialize, PartialEq, Eq)]
enum Expected {
    Complete,
    Ambiguous,
    Unsupported,
    Missing,
}

#[derive(Serialize)]
struct Row {
    id: String,
    expected: Expected,
    frontend_status: FrontendStatus,
    pack_status: Option<FormulaStatus>,
    exact: bool,
    value_correct: bool,
    frontend_replay: bool,
    pack_replay: bool,
    frontend_tamper_rejected: bool,
    pack_tamper_rejected: bool,
    pack_invoked: bool,
    false_authorization: bool,
}

#[derive(Serialize)]
struct Report {
    schema: &'static str,
    corpus_sha256: String,
    cases: usize,
    supported: usize,
    ambiguous: usize,
    unsupported: usize,
    missing: usize,
    exact_decisions: usize,
    supported_values: usize,
    frontend_replay: usize,
    pack_replay: usize,
    frontend_tamper_rejections: usize,
    pack_tamper_rejections: usize,
    pack_invocations: usize,
    false_authorizations: usize,
    false_denials: usize,
    rows: Vec<Row>,
}

fn digest<T: Serialize>(value: &T) -> String {
    format!(
        "{:x}",
        Sha256::digest(serde_json::to_vec(value).expect("frontend benchmark serializes"))
    )
}

fn q(numerator: i128, denominator: i128) -> Rational {
    Rational::new(numerator, denominator).expect("valid frontend rational")
}

fn expected_value(formula: Option<&str>) -> Option<Rational> {
    match formula {
        Some("regression_slope") => Some(q(3, 1)),
        Some("regression_intercept") => Some(q(1, 1)),
        Some("regression_fitted_value") => Some(q(13, 1)),
        Some("regression_residual") => Some(q(2, 1)),
        Some("regression_r_squared") => Some(q(3, 4)),
        _ => None,
    }
}

fn run(id: String, expected: Expected, text: String) -> Row {
    let frontend = formalize_regression_text(&text);
    let mut frontend_tampered = frontend.clone();
    frontend_tampered.replay_hash.push('x');
    let frontend_replay = frontend.replay_verified();
    let frontend_tamper_rejected = !frontend_tampered.replay_verified();
    let mut pack_result: Option<FormulaResult> = None;
    let mut pack_tamper_rejected = true;
    let mut value_correct = expected != Expected::Complete;
    if let (FrontendStatus::Complete, Some(request)) = (frontend.status, frontend.request.as_ref())
    {
        let result = evaluate_regression(request);
        value_correct = result.value == expected_value(frontend.formula.as_deref());
        let mut tampered = result.clone();
        tampered.replay_hash.push('x');
        pack_tamper_rejected = !tampered.replay_verified();
        pack_result = Some(result);
    }
    let pack_status = pack_result.as_ref().map(|result| result.status);
    let pack_replay = pack_result
        .as_ref()
        .map(FormulaResult::replay_verified)
        .unwrap_or(true);
    let frontend_expected = match expected {
        Expected::Complete => FrontendStatus::Complete,
        Expected::Ambiguous => FrontendStatus::Ambiguous,
        Expected::Unsupported => FrontendStatus::Unsupported,
        Expected::Missing => FrontendStatus::Missing,
    };
    let exact = frontend.status == frontend_expected
        && (expected != Expected::Complete || pack_status == Some(FormulaStatus::Complete));
    let pack_invoked = pack_result.is_some();
    let false_authorization = expected != Expected::Complete && pack_invoked;
    Row {
        id,
        expected,
        frontend_status: frontend.status,
        pack_status,
        exact,
        value_correct,
        frontend_replay,
        pack_replay,
        frontend_tamper_rejected,
        pack_tamper_rejected,
        pack_invoked,
        false_authorization,
    }
}

fn supported_text(index: usize) -> String {
    match index % 5 {
        0 => "find slope: covariance_sum=12 x_variance_sum=4".into(),
        1 => "determine y intercept y_mean=7 slope=3 x_mean=2".into(),
        2 => "calculate the predicted response intercept=1 slope=3 x=4".into(),
        3 => "compute residual observed=15 fitted=13".into(),
        _ => "compute r-squared explained_sum=12 total_sum=16".into(),
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut rows = Vec::with_capacity(240);
    for index in 0..120 {
        rows.push(run(
            format!("supported_{index:03}"),
            Expected::Complete,
            supported_text(index),
        ));
    }
    for index in 0..40 {
        rows.push(run(
            format!("ambiguous_{index:03}"),
            Expected::Ambiguous,
            "find slope and intercept covariance_sum=12 x_variance_sum=4 y_mean=7 slope=3 x_mean=2"
                .into(),
        ));
    }
    for index in 0..40 {
        rows.push(run(
            format!("unsupported_{index:03}"),
            Expected::Unsupported,
            "compute a confidence interval for the regression slope".into(),
        ));
    }
    for index in 0..20 {
        rows.push(run(
            format!("missing_slope_input_{index:03}"),
            Expected::Missing,
            "find slope covariance_sum=12".into(),
        ));
    }
    for index in 0..20 {
        rows.push(run(
            format!("missing_operation_{index:03}"),
            Expected::Missing,
            "regression data covariance_sum=12 x_variance_sum=4".into(),
        ));
    }
    let supported = rows
        .iter()
        .filter(|row| row.expected == Expected::Complete)
        .count();
    let ambiguous = rows
        .iter()
        .filter(|row| row.expected == Expected::Ambiguous)
        .count();
    let unsupported = rows
        .iter()
        .filter(|row| row.expected == Expected::Unsupported)
        .count();
    let missing = rows
        .iter()
        .filter(|row| row.expected == Expected::Missing)
        .count();
    let exact_decisions = rows.iter().filter(|row| row.exact).count();
    let supported_values = rows
        .iter()
        .filter(|row| row.expected == Expected::Complete && row.value_correct)
        .count();
    let frontend_replay = rows.iter().filter(|row| row.frontend_replay).count();
    let pack_replay = rows
        .iter()
        .filter(|row| row.pack_invoked && row.pack_replay)
        .count();
    let frontend_tamper_rejections = rows
        .iter()
        .filter(|row| row.frontend_tamper_rejected)
        .count();
    let pack_tamper_rejections = rows
        .iter()
        .filter(|row| row.pack_invoked && row.pack_tamper_rejected)
        .count();
    let pack_invocations = rows.iter().filter(|row| row.pack_invoked).count();
    let false_authorizations = rows.iter().filter(|row| row.false_authorization).count();
    let false_denials = rows
        .iter()
        .filter(|row| row.expected == Expected::Complete && !row.exact)
        .count();
    assert_eq!(rows.len(), 240);
    assert_eq!(
        (supported, ambiguous, unsupported, missing),
        (120, 40, 40, 40)
    );
    assert_eq!(
        (
            exact_decisions,
            supported_values,
            frontend_replay,
            pack_replay,
            frontend_tamper_rejections,
            pack_tamper_rejections,
            pack_invocations,
            false_authorizations,
            false_denials,
        ),
        (240, 120, 240, 120, 240, 120, 120, 0, 0)
    );
    let report = Report {
        schema: "stage-d-source-derived-finite-regression-frontend-v1",
        corpus_sha256: digest(&rows),
        cases: rows.len(),
        supported,
        ambiguous,
        unsupported,
        missing,
        exact_decisions,
        supported_values,
        frontend_replay,
        pack_replay,
        frontend_tamper_rejections,
        pack_tamper_rejections,
        pack_invocations,
        false_authorizations,
        false_denials,
        rows,
    };
    let serialized = serde_json::to_string_pretty(&report)?;
    std::fs::write(
        "docs/stage_d_source_regression_frontend.json",
        format!("{serialized}\n"),
    )?;
    println!("{serialized}");
    Ok(())
}
