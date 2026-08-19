//! Bounded technical-language frontend for source-derived linear interpolation.
//!
//! The frontend emits a generic `FormulaRequest`; it does not evaluate the
//! relation.  It requires all five bindings, distinct endpoint coordinates,
//! and a target inside the endpoint interval.  Extrapolation, nonlinear
//! interpolation, unknown points, and multiple target interpretations remain
//! fail-closed.

use crate::probability_pack::Rational;
use crate::source_formula_pack::FormulaRequest;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::cmp::Ordering;
use std::collections::BTreeMap;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum InterpolationFrontendStatus {
    Complete,
    Ambiguous,
    Missing,
    Unsupported,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct InterpolationFrontendResult {
    pub status: InterpolationFrontendStatus,
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

fn finish(mut result: InterpolationFrontendResult) -> InterpolationFrontendResult {
    result.replay_hash.clear();
    result.replay_hash = digest(&result);
    result
}

pub fn replay_verified(result: &InterpolationFrontendResult) -> bool {
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
    Rational::new(token.parse().ok()?, 1)
}

fn find_binding(text: &str, label: &str) -> Option<Rational> {
    let start = text.find(label)? + label.len();
    let token = text[start..]
        .trim_start_matches(|ch: char| ch == ' ' || ch == '=' || ch == ':' || ch == '(')
        .split(|ch: char| !(ch.is_ascii_digit() || ch == '-' || ch == '/'))
        .next()?;
    parse_rational(token)
}

fn compare(left: &Rational, right: &Rational) -> Ordering {
    (left.numerator * right.denominator).cmp(&(right.numerator * left.denominator))
}

fn result(
    status: InterpolationFrontendStatus,
    request: Option<FormulaRequest>,
    bindings: BTreeMap<String, Rational>,
    evidence: Vec<String>,
    unresolved: Vec<String>,
    provenance: Vec<String>,
) -> InterpolationFrontendResult {
    finish(InterpolationFrontendResult {
        status,
        request,
        bindings,
        evidence,
        unresolved,
        provenance,
        replay_hash: String::new(),
    })
}

pub fn formalize_interpolation_text(text: &str, case_id: &str) -> InterpolationFrontendResult {
    let lower = text.to_ascii_lowercase();
    let provenance = vec![format!("source-interpolation-frontend:{case_id}")];
    if lower.contains(" or ") || lower.matches("at x=").count() > 1 {
        return result(
            InterpolationFrontendStatus::Ambiguous,
            None,
            BTreeMap::new(),
            Vec::new(),
            vec!["interpolation operation or target is not unique".into()],
            provenance,
        );
    }
    if [
        "quadratic",
        "spline",
        "polynomial",
        "unknown point",
        "approx",
        "extrapolat",
    ]
    .iter()
    .any(|term| lower.contains(term))
    {
        return result(
            InterpolationFrontendStatus::Unsupported,
            None,
            BTreeMap::new(),
            Vec::new(),
            vec!["nonlinear, approximate, unknown, or extrapolation semantics are outside the bounded relation".into()],
            provenance,
        );
    }
    if !lower.contains("interpol") {
        return result(
            InterpolationFrontendStatus::Unsupported,
            None,
            BTreeMap::new(),
            Vec::new(),
            vec!["linear interpolation is not explicitly requested".into()],
            provenance,
        );
    }
    let labels = ["x1", "y1", "x2", "y2", "x="];
    let mut bindings = BTreeMap::new();
    for label in labels {
        let key = if label == "x=" { "x" } else { label };
        if let Some(value) = find_binding(&lower, label) {
            bindings.insert(key.into(), value);
        }
    }
    if bindings.len() != 5 {
        return result(
            InterpolationFrontendStatus::Missing,
            None,
            bindings,
            Vec::new(),
            vec!["x, x1, y1, x2, and y2 must be explicitly bound".into()],
            provenance,
        );
    }
    let x = bindings["x"].clone();
    let x1 = bindings["x1"].clone();
    let x2 = bindings["x2"].clone();
    if x1 == x2 {
        return result(
            InterpolationFrontendStatus::Unsupported,
            None,
            bindings,
            Vec::new(),
            vec!["endpoint x coordinates must be distinct".into()],
            provenance,
        );
    }
    let low = if compare(&x1, &x2) == Ordering::Less {
        x1.clone()
    } else {
        x2.clone()
    };
    let high = if compare(&x1, &x2) == Ordering::Less {
        x2.clone()
    } else {
        x1.clone()
    };
    if compare(&x, &low) == Ordering::Less || compare(&x, &high) == Ordering::Greater {
        return result(
            InterpolationFrontendStatus::Unsupported,
            None,
            bindings,
            Vec::new(),
            vec!["target lies outside the interpolation interval".into()],
            provenance,
        );
    }
    let request = FormulaRequest {
        formula: "linear_interpolation".into(),
        inputs: bindings.clone(),
        domain: "source_catalog_linear_interpolation".into(),
        ambiguity: None,
        provenance: provenance.clone(),
    };
    result(
        InterpolationFrontendStatus::Complete,
        Some(request),
        bindings,
        vec!["explicit interpolation target and endpoint bindings".into()],
        Vec::new(),
        provenance,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn binds_exact_interpolation_request() {
        let result = formalize_interpolation_text(
            "Linearly interpolate at x=5 between x1=0,y1=10 and x2=10,y2=30.",
            "test",
        );
        assert_eq!(result.status, InterpolationFrontendStatus::Complete);
        assert_eq!(
            result.request.as_ref().unwrap().formula,
            "linear_interpolation"
        );
        assert!(replay_verified(&result));
    }

    #[test]
    fn rejects_extrapolation_and_ambiguity() {
        let extrapolation = formalize_interpolation_text(
            "Linearly interpolate at x=15 between x1=0,y1=10 and x2=10,y2=30.",
            "outside",
        );
        assert_eq!(
            extrapolation.status,
            InterpolationFrontendStatus::Unsupported
        );
        let ambiguous = formalize_interpolation_text(
            "Interpolate or extrapolate at x=5 between x1=0,y1=10 and x2=10,y2=30.",
            "ambiguous",
        );
        assert_eq!(ambiguous.status, InterpolationFrontendStatus::Ambiguous);
        assert!(replay_verified(&ambiguous));
    }
}
