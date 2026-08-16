//! Fail-closed technical-language frontend for source-derived regression.
//!
//! It accepts only explicit operation evidence and labeled rational inputs.
//! Familiar regression vocabulary without the required quantities remains
//! ambiguous or missing; it never invents a design matrix or statistical
//! assumptions.

use crate::probability_pack::Rational;
use crate::source_formula_pack::FormulaRequest;
use crate::source_regression_pack::DOMAIN;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum FrontendStatus {
    Complete,
    Ambiguous,
    Unsupported,
    Missing,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RegressionFrontendResult {
    pub status: FrontendStatus,
    pub formula: Option<String>,
    pub request: Option<FormulaRequest>,
    pub provenance_spans: Vec<String>,
    pub alternatives: Vec<String>,
    pub reasons: Vec<String>,
    pub replay_hash: String,
}

fn digest<T: Serialize>(value: &T) -> String {
    format!(
        "{:x}",
        Sha256::digest(serde_json::to_vec(value).expect("regression frontend serializes"))
    )
}

fn payload(result: &RegressionFrontendResult) -> impl Serialize + '_ {
    (
        result.status,
        &result.formula,
        &result.request,
        &result.provenance_spans,
        &result.alternatives,
        &result.reasons,
    )
}

fn output(
    status: FrontendStatus,
    formula: Option<String>,
    request: Option<FormulaRequest>,
    spans: Vec<String>,
    alternatives: Vec<String>,
    reasons: Vec<String>,
) -> RegressionFrontendResult {
    let mut result = RegressionFrontendResult {
        status,
        formula,
        request,
        provenance_spans: spans,
        alternatives,
        reasons,
        replay_hash: String::new(),
    };
    let replay_hash = digest(&payload(&result));
    result.replay_hash = replay_hash;
    result
}

fn parse_rational(token: &str) -> Option<Rational> {
    let token = token.trim_matches(|character: char| {
        !character.is_ascii_digit() && character != '-' && character != '/'
    });
    if let Some((numerator, denominator)) = token.split_once('/') {
        return Rational::new(numerator.parse().ok()?, denominator.parse().ok()?);
    }
    Rational::new(token.parse().ok()?, 1)
}

fn labeled_value(text: &str, labels: &[&str]) -> Option<(String, Rational)> {
    let tokens: Vec<&str> = text.split_whitespace().collect();
    for (index, token) in tokens.iter().enumerate() {
        let normalized = token.trim_matches(|character: char| {
            !character.is_ascii_alphanumeric()
                && character != '_'
                && character != '='
                && character != '-'
        });
        for label in labels {
            let prefixes = [format!("{label}="), format!("{label}:")];
            for prefix in prefixes {
                if let Some(value) = normalized.strip_prefix(&prefix) {
                    if let Some(value) = parse_rational(value) {
                        return Some((normalized.into(), value));
                    }
                }
            }
            if normalized == *label {
                let mut value_index = index + 1;
                while matches!(tokens.get(value_index), Some(&"=" | &":")) {
                    value_index += 1;
                }
                if let Some(value) = tokens
                    .get(value_index)
                    .and_then(|value| parse_rational(value))
                {
                    return Some((tokens[index..=value_index].join(" "), value));
                }
            }
        }
    }
    None
}

fn has_operation_word(text: &str, word: &str) -> bool {
    text.split_whitespace().any(|token| {
        if token.contains('=') {
            return false;
        }
        let token = token.split(':').next().unwrap_or(token);
        token.trim_matches(|character: char| !character.is_ascii_alphabetic()) == word
    })
}

fn with_request(
    formula: &str,
    inputs: BTreeMap<String, Rational>,
    spans: Vec<String>,
) -> RegressionFrontendResult {
    output(
        FrontendStatus::Complete,
        Some(formula.into()),
        Some(FormulaRequest {
            formula: formula.into(),
            inputs,
            domain: DOMAIN.into(),
            ambiguity: None,
            provenance: spans.clone(),
        }),
        spans,
        Vec::new(),
        Vec::new(),
    )
}

/// Parse explicit finite-regression requests without inferring a model from
/// generic statistical language.
pub fn formalize_regression_text(text: &str) -> RegressionFrontendResult {
    let lower = text.to_ascii_lowercase();
    let unsupported = [
        "confidence interval",
        "hypothesis test",
        "p-value",
        "logistic",
        "nonlinear",
        "standard error",
        "significance",
    ];
    if unsupported.iter().any(|marker| lower.contains(marker)) {
        return output(
            FrontendStatus::Unsupported,
            None,
            None,
            vec![text.into()],
            Vec::new(),
            vec!["request is outside the bounded source regression catalog".into()],
        );
    }
    let mut candidates: Vec<(&str, Vec<(&str, &[&str])>)> = Vec::new();
    if has_operation_word(&lower, "slope") {
        candidates.push((
            "regression_slope",
            vec![
                ("covariance_sum", &["covariance_sum", "covariance-sum"]),
                ("x_variance_sum", &["x_variance_sum", "x-variance-sum"]),
            ],
        ));
    }
    if has_operation_word(&lower, "intercept") {
        candidates.push((
            "regression_intercept",
            vec![
                ("y_mean", &["y_mean", "y-mean", "ybar"]),
                ("slope", &["slope"]),
                ("x_mean", &["x_mean", "x-mean", "xbar"]),
            ],
        ));
    }
    if has_operation_word(&lower, "fitted") || has_operation_word(&lower, "predicted") {
        candidates.push((
            "regression_fitted_value",
            vec![
                ("intercept", &["intercept"]),
                ("slope", &["slope"]),
                ("x", &["x"]),
            ],
        ));
    }
    if has_operation_word(&lower, "residual") || lower.contains("prediction error") {
        candidates.push((
            "regression_residual",
            vec![("observed", &["observed"]), ("fitted", &["fitted"])],
        ));
    }
    if lower.contains("r-squared")
        || lower.contains("r squared")
        || lower.contains("coefficient of determination")
    {
        candidates.push((
            "regression_r_squared",
            vec![
                ("explained_sum", &["explained_sum", "explained-sum"]),
                ("total_sum", &["total_sum", "total-sum"]),
            ],
        ));
    }
    if candidates.is_empty() {
        return output(
            FrontendStatus::Missing,
            None,
            None,
            vec![text.into()],
            Vec::new(),
            vec!["no explicit supported regression operation was identified".into()],
        );
    }
    if candidates.len() != 1 {
        return output(
            FrontendStatus::Ambiguous,
            None,
            None,
            vec![text.into()],
            candidates
                .iter()
                .map(|(formula, _)| (*formula).into())
                .collect(),
            vec!["multiple regression targets are present".into()],
        );
    }
    let (formula, fields) = candidates.pop().expect("one candidate");
    let mut values = BTreeMap::new();
    let mut spans = Vec::new();
    for (name, labels) in fields {
        let Some((span, value)) = labeled_value(&lower, labels) else {
            return output(
                FrontendStatus::Missing,
                Some(formula.into()),
                None,
                vec![text.into()],
                Vec::new(),
                vec![format!(
                    "required labeled regression quantity {name} is missing"
                )],
            );
        };
        values.insert(name.into(), value);
        spans.push(span);
    }
    with_request(formula, values, spans)
}

impl RegressionFrontendResult {
    pub fn replay_verified(&self) -> bool {
        self.replay_hash == digest(&payload(self))
            && !self.provenance_spans.is_empty()
            && (self.status != FrontendStatus::Complete || self.request.is_some())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn explicit_slope_is_formalized() {
        let result = formalize_regression_text("find slope covariance_sum=12 x_variance_sum=4");
        assert_eq!(result.status, FrontendStatus::Complete);
        assert!(result.replay_verified());
    }

    #[test]
    fn unsupported_statistical_claims_refuse() {
        let result = formalize_regression_text("compute a confidence interval for the slope");
        assert_eq!(result.status, FrontendStatus::Unsupported);
        assert!(result.replay_verified());
    }
}
