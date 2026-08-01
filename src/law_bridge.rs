//! Shadow-only typed bridges for law lookup and equation binding.
//!
//! The catalog is supplied by the caller. This module contains no law facts,
//! no retrieval backend, and no router/registry integration.

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct LawRecord {
    pub law_id: String,
    pub aliases: Vec<String>,
    pub domain: String,
    pub equation: String,
    pub variables: Vec<String>,
    pub assumptions: Vec<String>,
    pub validity_domain: String,
    pub unit_constraints: Vec<String>,
    pub provenance: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct LawLookupRequest {
    pub name_or_alias: String,
    pub domain: Option<String>,
    pub requested_variables: Vec<String>,
    pub context: String,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum LookupStatus {
    Unique,
    Ambiguous,
    Unsupported,
    Missing,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct LawLookupResult {
    pub status: LookupStatus,
    pub candidates: Vec<LawRecord>,
    pub reasons: Vec<String>,
    pub replay_hash: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct QuantityBinding {
    pub symbol: String,
    pub value: String,
    pub unit: Option<String>,
    pub provenance: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct EquationBindingRequest {
    pub law: LawRecord,
    pub bindings: Vec<QuantityBinding>,
    pub requested_output: String,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum BindingStatus {
    Complete,
    Missing,
    Ambiguous,
    UnitMismatch,
    Unsupported,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct BoundEquationArtifact {
    pub law_id: String,
    pub equation: String,
    pub known_bindings: Vec<QuantityBinding>,
    pub unknown_symbol: String,
    pub assumptions: Vec<String>,
    pub validity_domain: String,
    pub provenance: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct EquationBindingResult {
    pub status: BindingStatus,
    pub artifact: Option<BoundEquationArtifact>,
    pub reasons: Vec<String>,
    pub replay_hash: String,
}

fn digest<T: Serialize>(value: &T) -> String {
    format!(
        "{:x}",
        Sha256::digest(serde_json::to_vec(value).expect("bridge value serializes"))
    )
}

fn normalized(value: &str) -> String {
    value.trim().to_ascii_lowercase()
}

pub fn lookup_law(request: &LawLookupRequest, catalog: &[LawRecord]) -> LawLookupResult {
    let name = normalized(&request.name_or_alias);
    let domain = request.domain.as_deref().map(normalized);
    let mut candidates = catalog
        .iter()
        .filter(|record| {
            let name_match = normalized(&record.law_id) == name
                || record.aliases.iter().any(|alias| normalized(alias) == name);
            let domain_match = domain
                .as_deref()
                .map(|expected| normalized(&record.domain) == expected)
                .unwrap_or(true);
            name_match && domain_match
        })
        .cloned()
        .collect::<Vec<_>>();
    candidates.sort_by(|left, right| left.law_id.cmp(&right.law_id));
    let (status, reasons) = match candidates.len() {
        0 if name.is_empty() => (LookupStatus::Missing, vec!["law name is missing".into()]),
        0 => (
            LookupStatus::Unsupported,
            vec!["no uniquely matching law record".into()],
        ),
        1 => {
            let candidate = &candidates[0];
            let missing = request
                .requested_variables
                .iter()
                .filter(|variable| !candidate.variables.contains(variable))
                .count();
            if missing > 0 {
                (
                    LookupStatus::Unsupported,
                    vec!["requested variable is outside law record".into()],
                )
            } else {
                (
                    LookupStatus::Unique,
                    vec!["unique provenance-bearing law record".into()],
                )
            }
        }
        _ => (
            LookupStatus::Ambiguous,
            vec!["alias maps to multiple domain-compatible law records".into()],
        ),
    };
    let replay_hash = digest(&(&status, &candidates, &reasons));
    LawLookupResult {
        status,
        candidates,
        reasons,
        replay_hash,
    }
}

pub fn replay_lookup(result: &LawLookupResult) -> bool {
    digest(&(&result.status, &result.candidates, &result.reasons)) == result.replay_hash
}

pub fn bind_equation(request: &EquationBindingRequest) -> EquationBindingResult {
    let mut reasons = Vec::new();
    let mut bindings = request.bindings.clone();
    bindings.sort_by(|left, right| left.symbol.cmp(&right.symbol));
    let duplicate = bindings
        .windows(2)
        .any(|pair| pair[0].symbol == pair[1].symbol);
    if duplicate {
        reasons.push("a symbol has multiple competing bindings".into());
    }
    if !request.law.variables.contains(&request.requested_output) {
        reasons.push("requested output is not a variable in the law record".into());
    }
    let unknown_binding = bindings
        .iter()
        .any(|binding| !request.law.variables.contains(&binding.symbol));
    if unknown_binding {
        reasons.push("binding references a symbol outside the law record".into());
    }
    let missing = request
        .law
        .variables
        .iter()
        .filter(|variable| **variable != request.requested_output)
        .filter(|variable| !bindings.iter().any(|binding| &binding.symbol == *variable))
        .count();
    if missing > 0 {
        reasons.push("one or more required law variables are unbound".into());
    }
    let unitless = bindings.iter().any(|binding| binding.unit.is_none());
    if unitless && !request.law.unit_constraints.is_empty() {
        reasons.push("unit-constrained law has a binding without a unit".into());
    }
    let incompatible_unit = bindings.iter().any(|binding| {
        let expected = request.law.unit_constraints.iter().find_map(|constraint| {
            let (symbol, unit) = constraint.split_once('=')?;
            (symbol.trim() == binding.symbol).then_some(unit.trim())
        });
        expected
            .zip(binding.unit.as_deref())
            .is_some_and(|(expected, actual)| normalized(expected) != normalized(actual))
    });
    if incompatible_unit {
        reasons.push("binding unit is incompatible with the law constraint".into());
    }
    let status = if duplicate {
        BindingStatus::Ambiguous
    } else if unknown_binding {
        BindingStatus::Unsupported
    } else if missing > 0 {
        BindingStatus::Missing
    } else if (unitless && !request.law.unit_constraints.is_empty()) || incompatible_unit {
        BindingStatus::UnitMismatch
    } else {
        BindingStatus::Complete
    };
    let artifact = (status == BindingStatus::Complete).then(|| BoundEquationArtifact {
        law_id: request.law.law_id.clone(),
        equation: request.law.equation.clone(),
        known_bindings: bindings.clone(),
        unknown_symbol: request.requested_output.clone(),
        assumptions: request.law.assumptions.clone(),
        validity_domain: request.law.validity_domain.clone(),
        provenance: std::iter::once(request.law.provenance.clone())
            .chain(bindings.iter().map(|binding| binding.provenance.clone()))
            .collect(),
    });
    let replay_hash = digest(&(&status, &artifact, &reasons));
    EquationBindingResult {
        status,
        artifact,
        reasons,
        replay_hash,
    }
}

pub fn replay_binding(result: &EquationBindingResult) -> bool {
    digest(&(&result.status, &result.artifact, &result.reasons)) == result.replay_hash
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ohm() -> LawRecord {
        LawRecord {
            law_id: "ohms_law".into(),
            aliases: vec!["ohm law".into()],
            domain: "physics".into(),
            equation: "V = I * R".into(),
            variables: vec!["V".into(), "I".into(), "R".into()],
            assumptions: vec!["constant resistance".into()],
            validity_domain: "lumped resistive circuit".into(),
            unit_constraints: vec!["V=volt".into(), "I=ampere".into(), "R=ohm".into()],
            provenance: "independent textbook:physics-01".into(),
        }
    }

    #[test]
    fn lookup_requires_unique_domain_compatible_record() {
        let result = lookup_law(
            &LawLookupRequest {
                name_or_alias: "Ohm law".into(),
                domain: Some("physics".into()),
                requested_variables: vec!["V".into(), "I".into(), "R".into()],
                context: "resistor".into(),
            },
            &[ohm()],
        );
        assert_eq!(result.status, LookupStatus::Unique);
        assert!(replay_lookup(&result));
    }

    #[test]
    fn binding_refuses_missing_or_unitless_inputs() {
        let result = bind_equation(&EquationBindingRequest {
            law: ohm(),
            bindings: vec![QuantityBinding {
                symbol: "I".into(),
                value: "2".into(),
                unit: None,
                provenance: "prompt:current".into(),
            }],
            requested_output: "V".into(),
        });
        assert_eq!(result.status, BindingStatus::Missing);
        assert!(replay_binding(&result));
    }

    #[test]
    fn binding_rejects_incompatible_units() {
        let result = bind_equation(&EquationBindingRequest {
            law: ohm(),
            bindings: vec![
                QuantityBinding {
                    symbol: "I".into(),
                    value: "2".into(),
                    unit: Some("volt".into()),
                    provenance: "prompt:I".into(),
                },
                QuantityBinding {
                    symbol: "R".into(),
                    value: "5".into(),
                    unit: Some("ohm".into()),
                    provenance: "prompt:R".into(),
                },
            ],
            requested_output: "V".into(),
        });
        assert_eq!(result.status, BindingStatus::UnitMismatch);
        assert!(replay_binding(&result));
    }
}
