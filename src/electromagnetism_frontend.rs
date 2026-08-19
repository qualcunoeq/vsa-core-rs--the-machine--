//! Explicit technical-language frontend for source-derived bounded
//! electromagnetism laws.
//!
//! Law identity, SI scope, and every required quantity must be stated.  The
//! frontend never infers circuit sign conventions or selects a law from a
//! generic mention of voltage, current, or energy.

use crate::electromagnetism_pack::{evaluate, EmRequest, EmResult, EmStatus};
use crate::probability_pack::Rational;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum EmFrontendStatus {
    Complete,
    Ambiguous,
    Missing,
    Unsupported,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct EmFrontendResult {
    pub status: EmFrontendStatus,
    pub request: Option<EmRequest>,
    pub law_candidates: Vec<String>,
    pub reasons: Vec<String>,
    pub provenance: Vec<String>,
    pub replay_hash: String,
}

fn digest<T: Serialize>(value: &T) -> String {
    format!("{:x}", Sha256::digest(serde_json::to_vec(value).unwrap()))
}

fn payload(result: &EmFrontendResult) -> impl Serialize + '_ {
    (
        result.status,
        &result.request,
        &result.law_candidates,
        &result.reasons,
        &result.provenance,
    )
}

fn finish(
    status: EmFrontendStatus,
    request: Option<EmRequest>,
    law_candidates: Vec<String>,
    reasons: Vec<String>,
    text: &str,
) -> EmFrontendResult {
    let mut result = EmFrontendResult {
        status,
        request,
        law_candidates,
        reasons,
        provenance: vec![format!("electromagnetism-source-span:0..{}", text.len())],
        replay_hash: String::new(),
    };
    let replay_hash = digest(&payload(&result));
    result.replay_hash = replay_hash;
    result
}

fn rational_after(text: &str, label: &str) -> Option<Rational> {
    let lower = text.to_ascii_lowercase();
    let marker = format!("{label}=");
    let start = lower.find(&marker)? + marker.len();
    let token: String = lower[start..]
        .chars()
        .take_while(|c| c.is_ascii_digit() || matches!(c, '-' | '/'))
        .collect();
    let mut parts = token.split('/');
    let numerator = parts.next()?.parse::<i128>().ok()?;
    let denominator = parts.next().unwrap_or("1").parse::<i128>().ok()?;
    Rational::new(numerator, denominator)
}

fn law_candidates(lower: &str) -> Vec<(&'static str, &'static [&'static str])> {
    [
        (
            "ohms_law_voltage",
            &[
                "ohm's law",
                "ohms law",
                "voltage from current and resistance",
            ][..],
        ),
        (
            "electric_power",
            &[
                "electric power",
                "electrical power",
                "power from voltage and current",
            ][..],
        ),
        (
            "charge_from_current",
            &["charge from constant current", "current time charge"][..],
        ),
        (
            "capacitor_charge",
            &[
                "capacitor charge",
                "charge on a capacitor",
                "capacitor charge voltage",
            ][..],
        ),
    ]
    .into_iter()
    .filter(|(_, aliases)| aliases.iter().any(|alias| lower.contains(alias)))
    .collect()
}

/// Parse an explicit source-derived electromagnetism law request.
pub fn formalize_em_text(text: &str, case_id: &str) -> EmFrontendResult {
    let lower = text.to_ascii_lowercase();
    if [
        "thermodynamic",
        "quantum",
        "field theory",
        "circuit simulation",
        "alternating",
    ]
    .iter()
    .any(|marker| lower.contains(marker))
    {
        return finish(
            EmFrontendStatus::Unsupported,
            None,
            Vec::new(),
            vec!["request exceeds the four-law bounded electromagnetism scope".into()],
            text,
        );
    }
    let candidates = law_candidates(&lower);
    if candidates.len() != 1 {
        return finish(
            if candidates.is_empty() {
                EmFrontendStatus::Missing
            } else {
                EmFrontendStatus::Ambiguous
            },
            None,
            candidates.iter().map(|(law, _)| (*law).into()).collect(),
            vec!["exactly one source-derived law identity is required".into()],
            text,
        );
    }
    if !(lower.contains("si-consistent") || lower.contains("si consistent")) {
        return finish(
            EmFrontendStatus::Ambiguous,
            None,
            vec![candidates[0].0.into()],
            vec!["SI-consistent unit scope must be stated".into()],
            text,
        );
    }
    let law = candidates[0].0;
    let required: &[&str] = match law {
        "ohms_law_voltage" => &["i", "r"],
        "electric_power" => &["v", "i"],
        "charge_from_current" => &["i", "t"],
        "capacitor_charge" => &["c", "v"],
        _ => unreachable!(),
    };
    let mut inputs = BTreeMap::new();
    for label in required {
        let Some(value) = rational_after(text, label) else {
            return finish(
                EmFrontendStatus::Missing,
                None,
                vec![law.into()],
                vec![format!("required quantity {label}= is absent or not exact")],
                text,
            );
        };
        let key = match *label {
            "i" => "I",
            "r" => "R",
            "v" => "V",
            "c" => "C",
            "t" => "t",
            _ => unreachable!(),
        };
        inputs.insert(key.into(), value);
    }
    let request = EmRequest {
        law: law.into(),
        inputs,
        domain: "source_derived_bounded_electromagnetism".into(),
        unit_scope: "si_consistent_exact".into(),
        ambiguity: None,
        provenance: vec![format!("case:{case_id}"), text.into()],
    };
    finish(
        EmFrontendStatus::Complete,
        Some(request),
        vec![law.into()],
        Vec::new(),
        text,
    )
}

pub fn replay_verified(result: &EmFrontendResult) -> bool {
    result.replay_hash == digest(&payload(result))
        && !result.provenance.is_empty()
        && (result.status != EmFrontendStatus::Complete || result.request.is_some())
}

pub fn downstream_replay(result: &EmFrontendResult) -> bool {
    result
        .request
        .as_ref()
        .map(|request| {
            let evaluated: EmResult = evaluate(request);
            evaluated.replay_verified()
                && (result.status != EmFrontendStatus::Complete
                    || evaluated.status == EmStatus::Complete)
        })
        .unwrap_or(true)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn explicit_law_and_si_scope_replay() {
        let result = formalize_em_text(
            "Apply Ohm's law with I=2 and R=5 in SI-consistent exact units.",
            "em-1",
        );
        assert_eq!(result.status, EmFrontendStatus::Complete);
        assert!(replay_verified(&result));
        assert!(downstream_replay(&result));
    }

    #[test]
    fn electromagnetism_boundaries_fail_closed() {
        assert_eq!(
            formalize_em_text("Use electric power with V=3 and I=2.", "missing-scope").status,
            EmFrontendStatus::Ambiguous
        );
        assert_eq!(
            formalize_em_text(
                "Use thermodynamic electromagnetic field theory.",
                "unsupported"
            )
            .status,
            EmFrontendStatus::Unsupported
        );
    }
}
