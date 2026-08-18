//! Controlled technical-language frontend for bounded discrete dynamics.

use crate::discrete_dynamics::{DynamicsOperation, DynamicsRequest};
use crate::probability_pack::Rational;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum DynamicsFrontendStatus {
    Complete,
    Ambiguous,
    Missing,
    Unsupported,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DynamicsFrontendResult {
    pub status: DynamicsFrontendStatus,
    pub request: Option<DynamicsRequest>,
    pub unresolved: Vec<String>,
    pub provenance: Vec<String>,
    pub replay_hash: String,
}

fn digest<T: Serialize>(value: &T) -> String {
    format!("{:x}", Sha256::digest(serde_json::to_vec(value).unwrap()))
}

fn finish(mut result: DynamicsFrontendResult) -> DynamicsFrontendResult {
    result.replay_hash.clear();
    result.replay_hash = digest(&result);
    result
}

pub fn replay_verified(result: &DynamicsFrontendResult) -> bool {
    let mut copy = result.clone();
    let hash = copy.replay_hash.clone();
    copy.replay_hash.clear();
    hash == digest(&copy) && !result.provenance.is_empty()
}

fn rational_after(text: &str, labels: &[&str]) -> Option<Rational> {
    let lower = text.to_ascii_lowercase();
    labels.iter().find_map(|label| {
        let start = lower.find(label)? + label.len();
        let token = text[start..]
            .trim_start_matches(|c: char| c == ':' || c == '=' || c.is_whitespace())
            .chars()
            .take_while(|c| c.is_ascii_digit() || *c == '-' || *c == '/')
            .collect::<String>();
        if let Some((numerator, denominator)) = token.split_once('/') {
            Rational::new(numerator.parse().ok()?, denominator.parse().ok()?)
        } else {
            Rational::new(token.parse().ok()?, 1)
        }
    })
}

fn usize_after(text: &str, labels: &[&str]) -> Option<usize> {
    rational_after(text, labels)
        .and_then(|value| (value.denominator == 1).then_some(value.numerator as usize))
}

/// Parse one explicit scalar affine recurrence from controlled text.
pub fn formalize(text: &str, case_id: &str) -> DynamicsFrontendResult {
    let lower = text.to_ascii_lowercase();
    let provenance = vec![
        format!("dynamics-frontend:{case_id}"),
        format!("source-span:0..{}", text.len()),
        "explicit-finite-horizon-recurrence-grammar".into(),
    ];
    if [
        "differential equation",
        "continuous-time",
        "infinite",
        "asymptotic",
        "stability",
        "chaotic",
        "limit",
    ]
    .iter()
    .any(|term| lower.contains(term))
    {
        return finish(DynamicsFrontendResult {
            status: DynamicsFrontendStatus::Unsupported,
            request: None,
            unresolved: vec!["request exceeds finite discrete-dynamics boundary".into()],
            provenance,
            replay_hash: String::new(),
        });
    }
    if lower.contains(" or ") || lower.contains("either") || lower.contains("ambiguous") {
        return finish(DynamicsFrontendResult {
            status: DynamicsFrontendStatus::Ambiguous,
            request: None,
            unresolved: vec!["recurrence interpretation has competing readings".into()],
            provenance,
            replay_hash: String::new(),
        });
    }
    if !lower.contains("recurrence") && !lower.contains("discrete") && !lower.contains("affine") {
        return finish(DynamicsFrontendResult {
            status: DynamicsFrontendStatus::Missing,
            request: None,
            unresolved: vec!["bounded discrete update operation is not explicit".into()],
            provenance,
            replay_hash: String::new(),
        });
    }
    let initial = rational_after(text, &["x0", "initial"]);
    let coefficient = rational_after(text, &["coefficient", "multiplier"]);
    let offset = rational_after(text, &["offset", "constant"]);
    let steps = usize_after(text, &["steps", "horizon"]);
    let (Some(initial), Some(coefficient), Some(offset), Some(steps)) =
        (initial, coefficient, offset, steps)
    else {
        return finish(DynamicsFrontendResult {
            status: DynamicsFrontendStatus::Missing,
            request: None,
            unresolved: vec!["initial, coefficient, offset, and finite steps are required".into()],
            provenance,
            replay_hash: String::new(),
        });
    };
    let request = DynamicsRequest {
        operation: DynamicsOperation::ScalarAffine,
        domain: "finite_exact_discrete_dynamics".into(),
        scalar_initial: Some(initial),
        coefficient: Some(coefficient),
        offset: Some(offset),
        vector_initial: None,
        matrix: None,
        steps,
        ambiguity: None,
        provenance: provenance.clone(),
    };
    finish(DynamicsFrontendResult {
        status: DynamicsFrontendStatus::Complete,
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
    fn explicit_recurrence_is_typed_and_replayable() {
        let result = formalize(
            "Apply the scalar affine recurrence x0=1, coefficient=2, offset=1 for steps=4.",
            "test",
        );
        assert_eq!(result.status, DynamicsFrontendStatus::Complete);
        assert!(replay_verified(&result));
    }

    #[test]
    fn continuous_and_ambiguous_forms_fail_closed() {
        assert_eq!(
            formalize("Solve the continuous-time differential equation.", "test").status,
            DynamicsFrontendStatus::Unsupported
        );
        assert_eq!(
            formalize(
                "Use either a recurrence or a differential equation.",
                "test"
            )
            .status,
            DynamicsFrontendStatus::Unsupported
        );
    }
}
