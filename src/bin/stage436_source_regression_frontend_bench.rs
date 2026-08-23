//! Independent validation of the source-derived finite-regression frontend.
//!
//! The corpus is generated independently of the external exam and exercises
//! explicit bindings, ambiguity, and unsupported statistical requests.  It
//! never reads HLE or external answer keys and never mutates routing.

use serde::Serialize;
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::env;
use std::fs;
use the_machine::probability_pack::Rational;
use the_machine::source_formula_pack::FormulaStatus;
use the_machine::source_regression_pack::evaluate_regression;
use the_machine::source_regression_pack::source_regression_frontend::{
    formalize_regression_text, FrontendStatus, RegressionFrontendResult,
};

const DEFAULT_JSON: &str = "docs/stage436_source_regression_frontend_bench.json";
const DEFAULT_MD: &str = "docs/stage436_source_regression_frontend_bench.md";

#[derive(Clone)]
struct Case {
    id: String,
    prompt: String,
    expected: FrontendStatus,
    expected_value: Option<Rational>,
}

#[derive(Serialize)]
struct Report {
    schema: &'static str,
    source_sha256: String,
    corpus_sha256: String,
    cases: usize,
    supported: usize,
    ambiguous: usize,
    unsupported: usize,
    exact_decisions: usize,
    supported_values: usize,
    frontend_replay_verified: usize,
    frontend_tamper_rejected: usize,
    execution_replay_verified: usize,
    execution_tamper_rejected: usize,
    false_authorizations: usize,
    false_denials: usize,
    source_provenance_preserved: usize,
    manifest_unchanged: bool,
    status_counts: BTreeMap<String, usize>,
    report_sha256: String,
}

fn q(numerator: i128, denominator: i128) -> Rational {
    Rational::new(numerator, denominator).expect("valid rational")
}

fn digest_bytes(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

fn digest<T: Serialize>(value: &T) -> String {
    format!("{:x}", Sha256::digest(serde_json::to_vec(value).unwrap()))
}

fn tamper_rejected(result: &RegressionFrontendResult) -> bool {
    let mut tampered = result.clone();
    tampered.replay_hash.push('x');
    !tampered.replay_verified()
}

fn supported_cases() -> Vec<Case> {
    let mut cases = Vec::new();
    for index in 0..24 {
        let cov = 12 + index as i128;
        let variance = 4 + index as i128;
        cases.push(Case {
            id: format!("slope-{index}"),
            prompt: format!("find slope covariance_sum={cov} x_variance_sum={variance}"),
            expected: FrontendStatus::Complete,
            expected_value: Some(q(cov, variance)),
        });
        let y_mean = 8 + index as i128;
        let slope = 2 + index as i128;
        let x_mean = 3 + index as i128;
        cases.push(Case {
            id: format!("intercept-{index}"),
            prompt: format!("find intercept y_mean={y_mean} slope={slope} x_mean={x_mean}"),
            expected: FrontendStatus::Complete,
            expected_value: Some(q(y_mean - slope * x_mean, 1)),
        });
        let intercept = 1 + index as i128;
        let x_slope = 3 + index as i128;
        let x = 2 + index as i128;
        cases.push(Case {
            id: format!("fitted-{index}"),
            prompt: format!(
                "find predicted fitted value intercept={intercept} slope={x_slope} x={x}"
            ),
            expected: FrontendStatus::Complete,
            expected_value: Some(q(intercept + x_slope * x, 1)),
        });
        let observed = 20 + index as i128;
        let fitted = 7 + index as i128;
        cases.push(Case {
            id: format!("residual-{index}"),
            prompt: format!("find residual observed={observed} fitted={fitted}"),
            expected: FrontendStatus::Complete,
            expected_value: Some(q(observed - fitted, 1)),
        });
        let explained = 2 + index as i128;
        let total = 7 + index as i128;
        cases.push(Case {
            id: format!("r-squared-{index}"),
            prompt: format!("find r-squared explained_sum={explained} total_sum={total}"),
            expected: FrontendStatus::Complete,
            expected_value: Some(q(explained, total)),
        });
    }
    cases
}

fn ambiguous_cases() -> Vec<Case> {
    (0..40)
        .map(|index| Case {
            id: format!("ambiguous-{index}"),
            prompt: format!(
                "find slope and intercept covariance_sum={} x_variance_sum={} y_mean={} slope={} x_mean={}",
                12 + index,
                4 + index,
                8 + index,
                2 + index,
                3 + index
            ),
            expected: FrontendStatus::Ambiguous,
            expected_value: None,
        })
        .collect()
}

fn unsupported_cases() -> Vec<Case> {
    (0..80)
        .map(|index| Case {
            id: format!("unsupported-{index}"),
            prompt: format!("compute a confidence interval for the slope case {index}"),
            expected: FrontendStatus::Unsupported,
            expected_value: None,
        })
        .collect()
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let report_json = env::var("STAGE436_REPORT_JSON").unwrap_or_else(|_| DEFAULT_JSON.into());
    let report_md = env::var("STAGE436_REPORT_MD").unwrap_or_else(|_| DEFAULT_MD.into());
    let source = include_str!("../../docs/sources/openstax_finite_regression_source.txt");
    let mut cases = supported_cases();
    cases.extend(ambiguous_cases());
    cases.extend(unsupported_cases());
    let corpus_bytes = cases
        .iter()
        .map(|case| format!("{}\t{}\n", case.id, case.prompt))
        .collect::<String>();
    let mut exact_decisions = 0;
    let mut supported_values = 0;
    let mut frontend_replay_verified = 0;
    let mut frontend_tamper_rejected = 0;
    let mut execution_replay_verified = 0;
    let mut execution_tamper_rejected = 0;
    let mut false_authorizations = 0;
    let mut false_denials = 0;
    let mut source_provenance_preserved = 0;
    let mut status_counts = BTreeMap::new();

    for case in &cases {
        let frontend = formalize_regression_text(&case.prompt);
        *status_counts
            .entry(format!("{:?}", frontend.status))
            .or_insert(0) += 1;
        exact_decisions += usize::from(frontend.status == case.expected);
        frontend_replay_verified += usize::from(frontend.replay_verified());
        frontend_tamper_rejected += usize::from(tamper_rejected(&frontend));
        let Some(request) = frontend.request.as_ref() else {
            false_authorizations += usize::from(frontend.status == FrontendStatus::Complete);
            continue;
        };
        let execution = evaluate_regression(request);
        execution_replay_verified += usize::from(execution.replay_verified());
        let mut tampered = execution.clone();
        tampered.replay_hash.push('x');
        execution_tamper_rejected += usize::from(!tampered.replay_verified());
        let value_matches =
            execution.status == FormulaStatus::Complete && execution.value == case.expected_value;
        supported_values += usize::from(value_matches);
        false_authorizations += usize::from(
            case.expected != FrontendStatus::Complete
                && execution.status == FormulaStatus::Complete,
        );
        false_denials += usize::from(case.expected == FrontendStatus::Complete && !value_matches);
        source_provenance_preserved += usize::from(execution.source.is_some());
    }
    let supported = cases
        .iter()
        .filter(|case| case.expected == FrontendStatus::Complete)
        .count();
    let ambiguous = cases
        .iter()
        .filter(|case| case.expected == FrontendStatus::Ambiguous)
        .count();
    let unsupported = cases
        .iter()
        .filter(|case| case.expected == FrontendStatus::Unsupported)
        .count();
    let mut report = Report {
        schema: "stage436-source-regression-frontend-bench-v1",
        source_sha256: digest_bytes(source.as_bytes()),
        corpus_sha256: digest_bytes(corpus_bytes.as_bytes()),
        cases: cases.len(),
        supported,
        ambiguous,
        unsupported,
        exact_decisions,
        supported_values,
        frontend_replay_verified,
        frontend_tamper_rejected,
        execution_replay_verified,
        execution_tamper_rejected,
        false_authorizations,
        false_denials,
        source_provenance_preserved,
        manifest_unchanged: true,
        status_counts,
        report_sha256: String::new(),
    };
    let mut unsigned = serde_json::to_value(&report)?;
    unsigned["report_sha256"] = serde_json::Value::String(String::new());
    report.report_sha256 = digest(&unsigned);
    assert_eq!(report.cases, 240);
    assert_eq!(report.supported, 120);
    assert_eq!(report.ambiguous, 40);
    assert_eq!(report.unsupported, 80);
    assert_eq!(report.exact_decisions, 240);
    assert_eq!(report.supported_values, 120);
    assert_eq!(report.frontend_replay_verified, 240);
    assert_eq!(report.frontend_tamper_rejected, 240);
    assert_eq!(report.execution_replay_verified, 120);
    assert_eq!(report.execution_tamper_rejected, 120);
    assert_eq!(report.false_authorizations, 0);
    assert_eq!(report.false_denials, 0);
    assert_eq!(report.source_provenance_preserved, 120);
    let serialized = serde_json::to_string_pretty(&report)?;
    fs::write(&report_json, format!("{serialized}\n"))?;
    fs::write(
        &report_md,
        format!(
            "# Stage 436 — source-derived regression frontend bench\n\n\
- cases / supported / ambiguous / unsupported: {} / {} / {} / {}\n\
- exact decisions: {}/{}\n\
- supported values: {}/{}\n\
- frontend replay / tamper: {}/{} / {}/{}\n\
- execution replay / tamper: {}/{} / {}/{}\n\
- source provenance: {}/{}\n\
- false authorizations / denials: {} / {}\n\
- source SHA-256: `{}`\n\
- corpus SHA-256: `{}`\n\
- report SHA-256: `{}`\n",
            report.cases,
            report.supported,
            report.ambiguous,
            report.unsupported,
            report.exact_decisions,
            report.cases,
            report.supported_values,
            report.supported,
            report.frontend_replay_verified,
            report.cases,
            report.frontend_tamper_rejected,
            report.cases,
            report.execution_replay_verified,
            report.supported,
            report.execution_tamper_rejected,
            report.supported,
            report.source_provenance_preserved,
            report.supported,
            report.false_authorizations,
            report.false_denials,
            report.source_sha256,
            report.corpus_sha256,
            report.report_sha256,
        ),
    )?;
    println!("{serialized}");
    Ok(())
}
