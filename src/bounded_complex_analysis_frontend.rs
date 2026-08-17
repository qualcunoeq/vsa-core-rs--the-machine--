//! Narrow technical-language frontend for bounded complex analysis.
//!
//! It accepts explicit rectangular affine derivative data and Cauchy--Riemann
//! requests.  It never infers derivatives from a nearby word such as
//! "analytic", and it preserves ambiguity or unsupported operation markers.

use crate::bounded_complex_analysis_pack::{
    ComplexAnalysisOperation, ComplexAnalysisRequest, DOMAIN,
};
use crate::probability_pack::Rational;
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
pub struct FrontendResult {
    pub status: FrontendStatus,
    pub request: Option<ComplexAnalysisRequest>,
    pub operation: Option<ComplexAnalysisOperation>,
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

fn payload(result: &FrontendResult) -> impl Serialize + '_ {
    (
        result.status,
        &result.request,
        result.operation,
        &result.provenance_spans,
        &result.alternatives,
        &result.reasons,
    )
}

fn finish(
    status: FrontendStatus,
    request: Option<ComplexAnalysisRequest>,
    operation: Option<ComplexAnalysisOperation>,
    text: &str,
    alternatives: Vec<String>,
    reasons: Vec<String>,
) -> FrontendResult {
    let mut result = FrontendResult {
        status,
        request,
        operation,
        provenance_spans: vec![text.to_owned()],
        alternatives,
        reasons,
        replay_hash: String::new(),
    };
    let replay_hash = digest(&(
        result.status,
        result.request.clone(),
        result.operation,
        result.provenance_spans.clone(),
        result.alternatives.clone(),
        result.reasons.clone(),
    ));
    result.replay_hash = replay_hash;
    result
}

fn parse_rational(value: &str) -> Option<Rational> {
    let value = value.trim_matches(|character: char| {
        !character.is_ascii_digit() && character != '-' && character != '/'
    });
    if let Some((numerator, denominator)) = value.split_once('/') {
        Rational::new(numerator.parse().ok()?, denominator.parse().ok()?)
    } else {
        Rational::new(value.parse().ok()?, 1)
    }
}

fn explicit_derivatives(text: &str) -> BTreeMap<String, Rational> {
    let normalized = text
        .replace('=', " = ")
        .replace(',', " ")
        .replace(';', " ")
        .replace(':', " ");
    let tokens: Vec<&str> = normalized.split_whitespace().collect();
    let mut values = BTreeMap::new();
    let names = ["ux", "uy", "vx", "vy", "u_x", "u_y", "v_x", "v_y"];
    for index in 0..tokens.len().saturating_sub(1) {
        let key = tokens[index].to_ascii_lowercase();
        if !names.contains(&key.as_str()) || tokens[index + 1] != "=" {
            continue;
        }
        if let Some(value) = tokens
            .get(index + 2)
            .and_then(|token| parse_rational(token))
        {
            let canonical = key.replace('_', "");
            values.insert(canonical, value);
        }
    }
    values
}

/// Formalize a bounded explicit Cauchy--Riemann or affine-derivative request.
pub fn formalize(text: &str, case_id: &str) -> FrontendResult {
    let lower = text.to_ascii_lowercase();
    if [
        "polar",
        "contour",
        "infinite series",
        "approx",
        "numerical",
        "branch",
    ]
    .iter()
    .any(|marker| lower.contains(marker))
    {
        return finish(
            FrontendStatus::Unsupported,
            None,
            None,
            text,
            Vec::new(),
            vec!["request requires complex-analysis semantics outside the bounded frontend".into()],
        );
    }
    if ["maybe", "possibly", "either", "ambiguous", "unclear"]
        .iter()
        .any(|marker| lower.contains(marker))
    {
        return finish(
            FrontendStatus::Ambiguous,
            None,
            None,
            text,
            vec![
                "cauchy_riemann_check".into(),
                "affine_holomorphic_derivative".into(),
            ],
            vec!["the requested analytic operation is explicitly unresolved".into()],
        );
    }
    if !(lower.contains("cauchy-riemann")
        || lower.contains("cauchy riemann")
        || lower.contains("cr equations"))
    {
        return finish(
            FrontendStatus::Missing,
            None,
            None,
            text,
            Vec::new(),
            vec!["no explicit bounded Cauchy-Riemann operation was identified".into()],
        );
    }
    let values = explicit_derivatives(text);
    let required = ["ux", "uy", "vx", "vy"];
    if required.iter().any(|name| !values.contains_key(*name)) {
        return finish(
            FrontendStatus::Missing,
            None,
            None,
            text,
            Vec::new(),
            vec!["all four explicit affine partial derivatives are required".into()],
        );
    }
    let operation = if lower.contains("derivative") || lower.contains("differentiate") {
        ComplexAnalysisOperation::AffineHolomorphicDerivative
    } else {
        ComplexAnalysisOperation::CauchyRiemannCheck
    };
    let request = ComplexAnalysisRequest {
        operation,
        coefficients: Vec::new(),
        point: None,
        ux: values.get("ux").cloned(),
        uy: values.get("uy").cloned(),
        vx: values.get("vx").cloned(),
        vy: values.get("vy").cloned(),
        domain: DOMAIN.into(),
        ambiguity: None,
        provenance: vec![format!("stage186:{case_id}:source-span")],
    };
    finish(
        FrontendStatus::Complete,
        Some(request),
        Some(operation),
        text,
        Vec::new(),
        Vec::new(),
    )
}

pub fn replay_verified(result: &FrontendResult) -> bool {
    result.replay_hash == digest(&payload(result)) && !result.provenance_spans.is_empty()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shifted_cauchy_riemann_phrasing_binds_all_partials() {
        let result = formalize(
            "For the affine map, verify the Cauchy-Riemann equations: v_y=2; u_x=2; v_x=1; u_y=-1.",
            "unit-supported",
        );
        assert_eq!(result.status, FrontendStatus::Complete);
        assert!(replay_verified(&result));
        assert_eq!(
            result
                .request
                .as_ref()
                .and_then(|request| request.ux.as_ref())
                .unwrap()
                .numerator,
            2
        );
    }

    #[test]
    fn ambiguity_and_unsupported_markers_are_preserved() {
        let ambiguous = formalize(
            "Maybe check either Cauchy-Riemann or the derivative.",
            "ambiguous",
        );
        assert_eq!(ambiguous.status, FrontendStatus::Ambiguous);
        let unsupported = formalize("Use a contour integral and polar branch.", "unsupported");
        assert_eq!(unsupported.status, FrontendStatus::Unsupported);
        assert!(replay_verified(&ambiguous));
        assert!(replay_verified(&unsupported));
    }
}
