//! Controlled technical-language frontend for the source-derived statistics
//! catalog. It extracts only explicitly labeled quantities and fails closed on
//! underspecified or unsupported statistical language.

use crate::probability_pack::Rational;
use crate::source_formula_pack::FormulaRequest;
use crate::source_statistics_pack::DOMAIN;
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
pub struct StatisticsFrontendResult {
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
        Sha256::digest(serde_json::to_vec(value).expect("frontend serializes"))
    )
}

fn payload(result: &StatisticsFrontendResult) -> impl Serialize + '_ {
    (
        result.status,
        &result.formula,
        &result.request,
        &result.provenance_spans,
        &result.alternatives,
        &result.reasons,
    )
}

fn rational_token(token: &str) -> Option<Rational> {
    let cleaned = token.trim_matches(|character: char| {
        !character.is_ascii_digit() && character != '-' && character != '/'
    });
    if let Some((numerator, denominator)) = cleaned.split_once('/') {
        return Rational::new(numerator.parse().ok()?, denominator.parse().ok()?);
    }
    Rational::new(cleaned.parse().ok()?, 1)
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
            if normalized == *label {
                // Accept an explicit separator as its own token, but never
                // infer a value from unlabeled prose.  This covers
                // `sum = 30` and `sum : 30` while retaining the same
                // fail-closed label boundary as `sum=30`.
                let mut value_index = index + 1;
                while matches!(tokens.get(value_index), Some(&"=" | &":")) {
                    value_index += 1;
                }
                if let Some(next) = tokens
                    .get(value_index)
                    .and_then(|value| rational_token(value))
                {
                    return (
                        format!("{label} {}", tokens[index + 1..=value_index].join(" ")),
                        next,
                    )
                        .into();
                }
            }
            let prefix = format!("{label}=");
            if let Some(value) = normalized.strip_prefix(&prefix) {
                if let Some(parsed) = rational_token(value) {
                    return (normalized.to_string(), parsed).into();
                }
            }
            let prefix = format!("{label}:");
            if let Some(value) = normalized.strip_prefix(&prefix) {
                if let Some(parsed) = rational_token(value) {
                    return (normalized.to_string(), parsed).into();
                }
            }
        }
    }
    None
}

fn result(
    status: FrontendStatus,
    formula: Option<String>,
    request: Option<FormulaRequest>,
    spans: Vec<String>,
    alternatives: Vec<String>,
    reasons: Vec<String>,
) -> StatisticsFrontendResult {
    let mut output = StatisticsFrontendResult {
        status,
        formula,
        request,
        provenance_spans: spans,
        alternatives,
        reasons,
        replay_hash: String::new(),
    };
    let replay_hash = digest(&payload(&output));
    output.replay_hash = replay_hash;
    output
}

fn with_request(
    formula: &str,
    inputs: BTreeMap<String, Rational>,
    spans: Vec<String>,
) -> StatisticsFrontendResult {
    result(
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

/// Parse a deliberately bounded set of labeled finite-statistics statements.
/// Unlabeled prose is never converted into a fact by lexical resemblance.
pub fn formalize_statistics_text(text: &str) -> StatisticsFrontendResult {
    let lower = text.to_ascii_lowercase();
    let unsupported_marker = [
        "continuous",
        "sample standard deviation",
        "confidence interval",
        "hypothesis test",
        "regression",
        "normal distribution",
        "density",
    ];
    if unsupported_marker
        .iter()
        .any(|marker| lower.contains(marker))
    {
        return result(
            FrontendStatus::Unsupported,
            None,
            None,
            vec![text.into()],
            Vec::new(),
            vec!["request is outside the finite source-statistics catalog".into()],
        );
    }
    let has_weighted = lower.contains("weighted") || lower.contains("weight");
    let has_mean = lower.contains("mean") || lower.contains("average");
    let has_bernoulli = lower.contains("bernoulli") || lower.contains("binary outcome");
    let has_binomial = lower.contains("binomial");
    if has_mean && has_weighted {
        let weighted_sum = labeled_value(&lower, &["weighted_sum", "weighted-sum"]);
        let total_weight = labeled_value(&lower, &["total_weight", "total-weight"]);
        if let (Some((sum_span, sum)), Some((weight_span, weight))) = (weighted_sum, total_weight) {
            return with_request(
                "weighted_mean",
                BTreeMap::from([
                    ("weighted_sum".into(), sum),
                    ("total_weight".into(), weight),
                ]),
                vec![sum_span, weight_span],
            );
        }
    }
    if has_mean && !has_weighted {
        let sum = labeled_value(&lower, &["sum"]);
        let count = labeled_value(&lower, &["count", "n"]);
        if let (Some((sum_span, sum)), Some((count_span, count))) = (sum, count) {
            return with_request(
                "arithmetic_mean",
                BTreeMap::from([("sum".into(), sum), ("count".into(), count)]),
                vec![sum_span, count_span],
            );
        }
    }
    if has_bernoulli && lower.contains("variance") {
        if let Some((span, probability)) = labeled_value(&lower, &["p", "probability"]) {
            return with_request(
                "bernoulli_variance",
                BTreeMap::from([("p".into(), probability)]),
                vec![span],
            );
        }
    }
    if has_binomial {
        let n = labeled_value(&lower, &["n", "trials"]);
        let probability = labeled_value(&lower, &["p", "probability"]);
        let formula = if lower.contains("variance") {
            "binomial_variance"
        } else if lower.contains("expected") || lower.contains("mean") {
            "binomial_expected_value"
        } else {
            return result(
                FrontendStatus::Ambiguous,
                None,
                None,
                vec![text.into()],
                vec!["binomial_expected_value".into(), "binomial_variance".into()],
                vec!["requested binomial output is not identified".into()],
            );
        };
        if let (Some((n_span, n)), Some((p_span, probability))) = (n, probability) {
            return with_request(
                formula,
                BTreeMap::from([("n".into(), n), ("p".into(), probability)]),
                vec![n_span, p_span],
            );
        }
    }
    if has_mean {
        return result(
            FrontendStatus::Ambiguous,
            None,
            None,
            vec![text.into()],
            vec!["arithmetic_mean".into(), "weighted_mean".into()],
            vec!["mean is present but the required labeled quantities do not identify one formulation".into()],
        );
    }
    result(
        FrontendStatus::Missing,
        None,
        None,
        vec![text.into()],
        Vec::new(),
        vec!["no supported finite-statistics target was identified".into()],
    )
}

impl StatisticsFrontendResult {
    pub fn replay_verified(&self) -> bool {
        self.replay_hash == digest(&payload(self))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn labeled_mean_is_typed_and_ambiguous_mean_is_preserved() {
        let complete = formalize_statistics_text("Find the mean from sum=30 and count=5.");
        assert_eq!(complete.status, FrontendStatus::Complete);
        assert_eq!(complete.formula.as_deref(), Some("arithmetic_mean"));
        assert!(complete.replay_verified());
        let shifted = formalize_statistics_text("Using count : 5, compute the mean from sum = 30.");
        assert_eq!(shifted.status, FrontendStatus::Complete);
        assert_eq!(shifted.formula.as_deref(), Some("arithmetic_mean"));
        assert!(shifted.replay_verified());
        let ambiguous = formalize_statistics_text("Find the average from total=30 and count=5.");
        assert_eq!(ambiguous.status, FrontendStatus::Ambiguous);
        assert!(ambiguous.replay_verified());
    }
}
