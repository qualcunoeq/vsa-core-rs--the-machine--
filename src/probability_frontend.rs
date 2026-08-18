//! Controlled technical-language frontend for finite exact probability.
//!
//! The frontend accepts only explicit finite outcomes and exact probabilities.
//! It constructs a typed request but never authorizes an answer by itself.

use crate::probability_pack::{ProbabilityOperation, ProbabilityRequest, Rational};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ProbabilityFrontendStatus {
    Complete,
    Ambiguous,
    Missing,
    Unsupported,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ProbabilityFrontendResult {
    pub status: ProbabilityFrontendStatus,
    pub request: Option<ProbabilityRequest>,
    pub unresolved: Vec<String>,
    pub provenance: Vec<String>,
    pub replay_hash: String,
}

fn digest<T: Serialize>(value: &T) -> String {
    format!("{:x}", Sha256::digest(serde_json::to_vec(value).unwrap()))
}

fn finish(mut result: ProbabilityFrontendResult) -> ProbabilityFrontendResult {
    result.replay_hash.clear();
    result.replay_hash = digest(&result);
    result
}

pub fn replay_verified(result: &ProbabilityFrontendResult) -> bool {
    let mut copy = result.clone();
    let hash = copy.replay_hash.clone();
    copy.replay_hash.clear();
    hash == digest(&copy) && !result.provenance.is_empty()
}

fn list_after<'a>(text: &'a str, marker: &str) -> Option<&'a str> {
    let lower = text.to_ascii_lowercase();
    let start = lower.find(marker)? + marker.len();
    let tail =
        text[start..].trim_start_matches(|c: char| c == ':' || c == '=' || c.is_whitespace());
    Some(tail.split([';', '.']).next().unwrap_or(tail))
}

fn parse_outcomes(text: &str) -> Vec<String> {
    let Some(body) = list_after(text, "outcomes") else {
        return Vec::new();
    };
    let body = body.split("probabilities").next().unwrap_or(body);
    body.trim_matches(|c: char| c == '[' || c == ']' || c == '(' || c == ')')
        .split(',')
        .map(|value| value.trim().trim_matches(['[', ']', '(', ')']).to_string())
        .filter(|value| !value.is_empty())
        .collect()
}

fn parse_rational(token: &str) -> Option<Rational> {
    let token = token.trim().trim_matches(['[', ']', '(', ')']);
    if let Some((numerator, denominator)) = token.split_once('/') {
        return Rational::new(
            numerator.trim().parse().ok()?,
            denominator.trim().parse().ok()?,
        );
    }
    Rational::new(token.parse().ok()?, 1)
}

fn parse_probabilities(text: &str) -> Vec<Rational> {
    let Some(body) = list_after(text, "probabilities") else {
        return Vec::new();
    };
    body.trim_matches(|c: char| c == '[' || c == ']' || c == '(' || c == ')')
        .split(',')
        .filter_map(parse_rational)
        .collect()
}

/// Parse one explicit finite distribution request from controlled text.
pub fn formalize(text: &str, case_id: &str) -> ProbabilityFrontendResult {
    let lower = text.to_ascii_lowercase();
    let provenance = vec![
        format!("probability-frontend:{case_id}"),
        format!("source-span:0..{}", text.len()),
        "explicit-finite-distribution-grammar".into(),
    ];
    if [
        "continuous",
        "density",
        "measure-theoretic",
        "gaussian",
        "normal distribution",
        "asymptotic",
        "stochastic process",
    ]
    .iter()
    .any(|term| lower.contains(term))
    {
        return finish(ProbabilityFrontendResult {
            status: ProbabilityFrontendStatus::Unsupported,
            request: None,
            unresolved: vec!["request exceeds finite exact probability boundary".into()],
            provenance,
            replay_hash: String::new(),
        });
    }
    if lower.contains(" or ") || lower.contains("either") || lower.contains("ambiguous") {
        return finish(ProbabilityFrontendResult {
            status: ProbabilityFrontendStatus::Ambiguous,
            request: None,
            unresolved: vec!["probability interpretation has competing readings".into()],
            provenance,
            replay_hash: String::new(),
        });
    }
    if !lower.contains("distribution") && !lower.contains("probability mass") {
        return finish(ProbabilityFrontendResult {
            status: ProbabilityFrontendStatus::Missing,
            request: None,
            unresolved: vec!["finite distribution operation is not explicit".into()],
            provenance,
            replay_hash: String::new(),
        });
    }
    if lower.matches("outcomes").count() > 1 || lower.matches("probabilities").count() > 1 {
        return finish(ProbabilityFrontendResult {
            status: ProbabilityFrontendStatus::Ambiguous,
            request: None,
            unresolved: vec!["outcome or probability binding has multiple scopes".into()],
            provenance,
            replay_hash: String::new(),
        });
    }
    let outcomes = parse_outcomes(text);
    let probabilities = parse_probabilities(text);
    if outcomes.is_empty() || probabilities.is_empty() || outcomes.len() != probabilities.len() {
        return finish(ProbabilityFrontendResult {
            status: ProbabilityFrontendStatus::Missing,
            request: None,
            unresolved: vec![
                "explicit outcomes and probabilities with matching lengths are required".into(),
            ],
            provenance,
            replay_hash: String::new(),
        });
    }
    let request = ProbabilityRequest {
        operation: ProbabilityOperation::DistributionConstruction,
        domain: "finite_exact_probability".into(),
        values: (0..outcomes.len() as i64).collect(),
        outcomes,
        probabilities,
        event_a: None,
        event_b: None,
        partition: Vec::new(),
        conditional_values: Vec::new(),
        prior_probability: None,
        likelihood: None,
        evidence: None,
        ambiguity: None,
        provenance: provenance.clone(),
    };
    finish(ProbabilityFrontendResult {
        status: ProbabilityFrontendStatus::Complete,
        request: Some(request),
        unresolved: Vec::new(),
        provenance,
        replay_hash: String::new(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn explicit_distribution_is_typed_and_replayable() {
        let result = formalize(
            "Construct a finite distribution with outcomes=[a,b] probabilities=[1/2,1/2].",
            "test",
        );
        assert_eq!(result.status, ProbabilityFrontendStatus::Complete);
        assert!(replay_verified(&result));
    }

    #[test]
    fn continuous_and_competing_forms_fail_closed() {
        assert_eq!(
            formalize("Use a continuous density for the distribution.", "test").status,
            ProbabilityFrontendStatus::Unsupported
        );
        assert_eq!(
            formalize(
                "The probability is either a finite distribution or a density.",
                "test"
            )
            .status,
            ProbabilityFrontendStatus::Unsupported
        );
    }
}
