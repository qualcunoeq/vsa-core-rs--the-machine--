//! Bounded technical-language frontend for the source-derived Bayes catalog.
//!
//! The frontend only constructs an explicit prior/likelihood/evidence request.
//! It does not infer probabilities from medical, diagnostic, or causal prose.
//! Missing, conflicting, continuous, or multiply-targeted interpretations stay
//! fail-closed.

use crate::probability_pack::Rational;
use crate::source_formula_pack::FormulaRequest;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum BayesFrontendStatus { Complete, Ambiguous, Missing, Unsupported }

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct BayesFrontendResult {
    pub status: BayesFrontendStatus,
    pub request: Option<FormulaRequest>,
    pub bindings: BTreeMap<String, Rational>,
    pub evidence: Vec<String>,
    pub unresolved: Vec<String>,
    pub provenance: Vec<String>,
    pub replay_hash: String,
}

fn digest<T: Serialize>(value: &T) -> String {
    format!("{:x}", Sha256::digest(serde_json::to_vec(value).unwrap()))
}

fn finish(mut result: BayesFrontendResult) -> BayesFrontendResult {
    result.replay_hash.clear();
    result.replay_hash = digest(&result);
    result
}

pub fn replay_verified(result: &BayesFrontendResult) -> bool {
    let mut copy = result.clone();
    let hash = copy.replay_hash.clone();
    copy.replay_hash.clear();
    hash == digest(&copy) && !result.provenance.is_empty()
}

fn parse_rational(token: &str) -> Option<Rational> {
    let token = token.trim_matches(|ch: char| !ch.is_ascii_digit() && ch != '-' && ch != '/');
    if let Some((numerator, denominator)) = token.split_once('/') {
        return Rational::new(numerator.parse().ok()?, denominator.parse().ok()?);
    }
    if let Some((whole, fraction)) = token.split_once('.') {
        let scale = 10_i128.checked_pow(fraction.len() as u32)?;
        let numerator = whole.parse::<i128>().ok()?.checked_mul(scale)?
            .checked_add(fraction.parse::<i128>().ok()?)?;
        return Rational::new(numerator, scale);
    }
    Rational::new(token.parse().ok()?, 1)
}

fn find_binding(text: &str, labels: &[&str]) -> Option<Rational> {
    for label in labels {
        let Some(start) = text.find(label) else { continue };
        let token = text[start + label.len()..]
            .trim_start_matches(|ch: char| ch == ' ' || ch == '=' || ch == ':' || ch == '(')
            .split(|ch: char| !(ch.is_ascii_digit() || ch == '-' || ch == '/' || ch == '.'))
            .next()?;
        if let Some(value) = parse_rational(token) { return Some(value); }
    }
    None
}

fn result(
    status: BayesFrontendStatus,
    request: Option<FormulaRequest>,
    bindings: BTreeMap<String, Rational>,
    evidence: Vec<String>,
    unresolved: Vec<String>,
    provenance: Vec<String>,
) -> BayesFrontendResult {
    finish(BayesFrontendResult { status, request, bindings, evidence, unresolved, provenance, replay_hash: String::new() })
}

pub fn formalize_bayes_text(text: &str, case_id: &str) -> BayesFrontendResult {
    let lower = text.to_ascii_lowercase();
    let provenance = vec![format!("source-bayes-frontend:{case_id}")];
    if lower.contains(" or ") || lower.matches("posterior").count() > 1
        || lower.contains("either") || lower.contains("two possible") {
        return result(BayesFrontendStatus::Ambiguous, None, BTreeMap::new(), Vec::new(),
            vec!["Bayes target or interpretation is not unique".into()], provenance);
    }
    if ["continuous", "density", "asymptotic", "simulation", "approx", "unknown prior",
        "unknown likelihood", "unknown evidence", "independent assumption"].iter()
        .any(|term| lower.contains(term)) {
        return result(BayesFrontendStatus::Unsupported, None, BTreeMap::new(), Vec::new(),
            vec!["continuous, approximate, inferred-independence, or missing-input semantics are outside the bounded catalog".into()], provenance);
    }
    if !(lower.contains("bayes") || lower.contains("posterior") || lower.contains("given")) {
        return result(BayesFrontendStatus::Unsupported, None, BTreeMap::new(), Vec::new(),
            vec!["a Bayes posterior request is not explicit".into()], provenance);
    }
    let mut bindings = BTreeMap::new();
    let labels = [
        ("prior", &["prior", "p(a)", "p(a) ="] as &[&str]),
        ("likelihood", &["likelihood", "p(b|a)", "p(b | a)"] as &[&str]),
        ("evidence", &["evidence", "p(b)", "p(b) ="] as &[&str]),
    ];
    for (name, aliases) in labels {
        if let Some(value) = find_binding(&lower, aliases) { bindings.insert(name.into(), value); }
    }
    if bindings.len() != 3 {
        return result(BayesFrontendStatus::Missing, None, bindings, Vec::new(),
            vec!["prior, likelihood, and positive evidence must be explicitly bound".into()], provenance);
    }
    if bindings.values().any(|value| !value.in_unit_interval()) || !bindings["evidence"].positive() {
        return result(BayesFrontendStatus::Unsupported, None, bindings, Vec::new(),
            vec!["all inputs must be probabilities and evidence must be positive".into()], provenance);
    }
    let request = FormulaRequest {
        formula: "bayes_posterior".into(),
        inputs: bindings.clone(),
        domain: "source_catalog_bayes_rule".into(),
        ambiguity: None,
        provenance: provenance.clone(),
    };
    result(BayesFrontendStatus::Complete, Some(request), bindings,
        vec!["explicit prior, likelihood, and evidence bindings".into()], Vec::new(), provenance)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn binds_exact_bayes_request() {
        let result = formalize_bayes_text(
            "Use Bayes theorem with prior=3/100, likelihood=3/4, evidence=1/5 to find the posterior.",
            "test",
        );
        assert_eq!(result.status, BayesFrontendStatus::Complete);
        assert!(replay_verified(&result));
    }

    #[test]
    fn refuses_missing_or_ambiguous_inputs() {
        let missing = formalize_bayes_text("Find the posterior using prior=1/4 and likelihood=1/2.", "missing");
        assert_eq!(missing.status, BayesFrontendStatus::Missing);
        let ambiguous = formalize_bayes_text("Use Bayes or another rule with prior=1/4, likelihood=1/2, evidence=1/3.", "ambiguous");
        assert_eq!(ambiguous.status, BayesFrontendStatus::Ambiguous);
        assert!(replay_verified(&ambiguous));
    }
}
