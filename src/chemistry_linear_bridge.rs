//! Typed chemistry-to-linear-algebra handoff.
//!
//! A molecular formula or a validated balanced reaction can expose an exact
//! element-count vector.  The vector retains its element basis and semantic
//! kind; it is not treated as an arbitrary numerical vector.  Stoichiometric
//! ratios and incomplete chemistry results do not silently enter this bridge.

use super::{ChemistryArtifact, ChemistryResult, ChemistryStatus};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ChemistryLinearBridgeStatus {
    Complete,
    Ambiguous,
    Unsupported,
    Missing,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ElementCountVector {
    pub basis: Vec<String>,
    pub values: Vec<i64>,
    pub semantic_kind: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ChemistryLinearBridgeResult {
    pub status: ChemistryLinearBridgeStatus,
    pub artifact: Option<ElementCountVector>,
    pub chemistry_replay_hash: String,
    pub reasons: Vec<String>,
    pub provenance: Vec<String>,
    pub replay_hash: String,
}

fn digest<T: Serialize>(value: &T) -> String {
    format!(
        "{:x}",
        Sha256::digest(serde_json::to_vec(value).expect("chemistry bridge serializes"))
    )
}

fn payload(result: &ChemistryLinearBridgeResult) -> impl Serialize + '_ {
    (
        result.status,
        &result.artifact,
        &result.chemistry_replay_hash,
        &result.reasons,
        &result.provenance,
    )
}

fn output(
    status: ChemistryLinearBridgeStatus,
    artifact: Option<ElementCountVector>,
    chemistry: &ChemistryResult,
    reasons: Vec<String>,
) -> ChemistryLinearBridgeResult {
    let mut result = ChemistryLinearBridgeResult {
        status,
        artifact,
        chemistry_replay_hash: chemistry.replay_hash.clone(),
        reasons,
        provenance: chemistry
            .provenance
            .iter()
            .map(|value| format!("chemistry:{value}"))
            .collect(),
        replay_hash: String::new(),
    };
    let replay_hash = digest(&payload(&result));
    result.replay_hash = replay_hash;
    result
}

/// Lower an authorized chemistry artifact into a semantically labeled exact
/// element-count vector.  No chemistry artifact is authorized by this bridge;
/// the caller must still validate the resulting linear-algebra handoff.
pub fn bridge_chemistry_to_linear(chemistry: &ChemistryResult) -> ChemistryLinearBridgeResult {
    if chemistry.status != ChemistryStatus::Complete || !chemistry.replay_verified() {
        let status = match chemistry.status {
            ChemistryStatus::Ambiguous => ChemistryLinearBridgeStatus::Ambiguous,
            ChemistryStatus::Missing => ChemistryLinearBridgeStatus::Missing,
            ChemistryStatus::Unsupported
            | ChemistryStatus::InvalidDomain
            | ChemistryStatus::Inconsistent => ChemistryLinearBridgeStatus::Unsupported,
            ChemistryStatus::Complete => ChemistryLinearBridgeStatus::Unsupported,
        };
        return output(
            status,
            None,
            chemistry,
            vec!["only a replayable complete chemistry artifact may cross the bridge".into()],
        );
    }

    let (atoms, semantic_kind) = match chemistry.artifact.as_ref() {
        Some(ChemistryArtifact::MolecularFormula { atoms }) => (atoms, "molecular_atom_counts"),
        Some(ChemistryArtifact::BalancedReaction { atom_totals, .. }) => {
            (atom_totals, "conserved_reaction_atom_totals")
        }
        Some(ChemistryArtifact::StoichiometricRatio { .. }) => {
            return output(
                ChemistryLinearBridgeStatus::Unsupported,
                None,
                chemistry,
                vec![
                    "a stoichiometric ratio is not an element-count vector without a declared basis".into(),
                ],
            );
        }
        None => {
            return output(
                ChemistryLinearBridgeStatus::Missing,
                None,
                chemistry,
                vec!["complete chemistry result has no artifact".into()],
            );
        }
    };

    if atoms.is_empty() || atoms.len() > 32 {
        return output(
            ChemistryLinearBridgeStatus::Unsupported,
            None,
            chemistry,
            vec!["element basis is empty or exceeds the bounded vector width".into()],
        );
    }
    let basis = atoms.keys().cloned().collect::<Vec<_>>();
    let values = atoms
        .values()
        .map(|value| i64::from(*value))
        .collect::<Vec<_>>();
    output(
        ChemistryLinearBridgeStatus::Complete,
        Some(ElementCountVector {
            basis,
            values,
            semantic_kind: semantic_kind.into(),
        }),
        chemistry,
        Vec::new(),
    )
}

impl ChemistryLinearBridgeResult {
    pub fn replay_verified(&self) -> bool {
        self.replay_hash == digest(&payload(self))
            && !self.provenance.is_empty()
            && (self.status != ChemistryLinearBridgeStatus::Complete || self.artifact.is_some())
    }

    pub fn authorized(&self) -> bool {
        self.status == ChemistryLinearBridgeStatus::Complete && self.replay_verified()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::source_formula_pack::chemistry_pack::{
        evaluate_chemistry, ChemistryOperation, ChemistryRequest,
    };

    fn request(operation: ChemistryOperation) -> ChemistryRequest {
        ChemistryRequest {
            operation,
            formula: Some("H2O".into()),
            reaction: Some("2H2 + O2 -> 2H2O".into()),
            from_species: None,
            to_species: None,
            domain: "source_derived_bounded_chemistry".into(),
            ambiguity: None,
            provenance: vec!["bridge-test".into()],
        }
    }

    #[test]
    fn formula_vector_preserves_element_basis() {
        let chemistry = evaluate_chemistry(&request(ChemistryOperation::ParseFormula));
        let bridge = bridge_chemistry_to_linear(&chemistry);
        assert!(bridge.authorized());
        let vector = bridge.artifact.expect("vector");
        assert_eq!(vector.basis, vec!["H", "O"]);
        assert_eq!(vector.values, vec![2, 1]);
    }

    #[test]
    fn ratio_does_not_gain_vector_semantics() {
        let chemistry = evaluate_chemistry(&ChemistryRequest {
            operation: ChemistryOperation::StoichiometricRatio,
            formula: None,
            reaction: Some("N2 + 3H2 -> 2NH3".into()),
            from_species: Some("H2".into()),
            to_species: Some("NH3".into()),
            domain: "source_derived_bounded_chemistry".into(),
            ambiguity: None,
            provenance: vec!["bridge-test".into()],
        });
        let bridge = bridge_chemistry_to_linear(&chemistry);
        assert_eq!(bridge.status, ChemistryLinearBridgeStatus::Unsupported);
        assert!(!bridge.authorized());
        assert!(bridge.replay_verified());
    }
}
