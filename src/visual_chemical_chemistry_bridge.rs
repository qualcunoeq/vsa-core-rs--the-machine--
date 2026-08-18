//! Bridge explicit visual atom inventories into the source-derived chemistry
//! formula parser.
//!
//! Only direct atom counting under an explicit `single_molecule` scope is
//! supported.  Bonds are preserved but are not interpreted as valence,
//! charge, stereochemistry, or reaction semantics.

use crate::source_formula_pack::chemistry_pack::{
    evaluate_chemistry, ChemistryArtifact, ChemistryOperation, ChemistryRequest, ChemistryResult,
    ChemistryStatus,
};
use crate::vision::visual_chemical::{
    ChemicalVisualStatus, VisualChemicalArtifact, VisualChemicalResult,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum BridgeStatus {
    Complete,
    Missing,
    Ambiguous,
    Unsupported,
    Invalid,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ChemicalBridgeRequest {
    pub operation: ChemistryOperation,
    pub ambiguity: Option<String>,
    pub provenance: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ChemicalBridgeResult {
    pub status: BridgeStatus,
    pub formula: Option<String>,
    pub chemistry_result: Option<ChemistryResult>,
    pub visual_status: ChemicalVisualStatus,
    pub reasons: Vec<String>,
    pub provenance: Vec<String>,
    pub replay_hash: String,
}

fn digest<T: Serialize>(value: &T) -> String {
    format!(
        "{:x}",
        Sha256::digest(serde_json::to_vec(value).expect("chemical bridge serializes"))
    )
}

fn payload(result: &ChemicalBridgeResult) -> impl Serialize + '_ {
    (
        result.status,
        &result.formula,
        &result.chemistry_result,
        result.visual_status,
        &result.reasons,
        &result.provenance,
    )
}

fn finish(
    status: BridgeStatus,
    formula: Option<String>,
    chemistry_result: Option<ChemistryResult>,
    visual_status: ChemicalVisualStatus,
    reasons: Vec<String>,
    provenance: Vec<String>,
) -> ChemicalBridgeResult {
    let mut result = ChemicalBridgeResult {
        status,
        formula,
        chemistry_result,
        visual_status,
        reasons,
        provenance,
        replay_hash: String::new(),
    };
    let replay_hash = digest(&(
        result.status,
        &result.formula,
        &result.chemistry_result,
        result.visual_status,
        &result.reasons,
        &result.provenance,
    ));
    result.replay_hash = replay_hash;
    result
}

fn formula_from_atoms(artifact: &VisualChemicalArtifact) -> String {
    let mut counts: BTreeMap<String, u32> = BTreeMap::new();
    for atom in &artifact.atoms {
        *counts.entry(atom.element.clone()).or_default() += 1;
    }
    let mut output = String::new();
    for element in ["C", "H"] {
        if let Some(count) = counts.remove(element) {
            output.push_str(element);
            if count != 1 {
                output.push_str(&count.to_string());
            }
        }
    }
    for (element, count) in counts {
        output.push_str(&element);
        if count != 1 {
            output.push_str(&count.to_string());
        }
    }
    output
}

/// Convert an already validated visual atom inventory into a chemistry
/// formula.  The bridge does not inspect bond order or infer any chemistry.
pub fn evaluate_chemical_structure(
    visual: &VisualChemicalResult,
    request: &ChemicalBridgeRequest,
) -> ChemicalBridgeResult {
    let mut provenance = visual.provenance.clone();
    provenance.extend(request.provenance.clone());
    if visual.status == ChemicalVisualStatus::Ambiguous {
        return finish(
            BridgeStatus::Ambiguous,
            None,
            None,
            visual.status,
            visual.reasons.clone(),
            provenance,
        );
    }
    if visual.artifact.is_none() {
        return finish(
            if visual.status == ChemicalVisualStatus::Unsupported {
                BridgeStatus::Unsupported
            } else {
                BridgeStatus::Missing
            },
            None,
            None,
            visual.status,
            visual.reasons.clone(),
            provenance,
        );
    }
    if visual.status != ChemicalVisualStatus::Complete || !visual.replay_verified() {
        return finish(
            BridgeStatus::Invalid,
            None,
            None,
            visual.status,
            vec!["visual artifact is not complete and replayable".into()],
            provenance,
        );
    }
    if visual
        .artifact
        .as_ref()
        .is_none_or(|artifact| artifact.scope != "single_molecule")
    {
        return finish(
            BridgeStatus::Unsupported,
            None,
            None,
            visual.status,
            vec!["formula conversion requires explicit single_molecule scope".into()],
            provenance,
        );
    }
    if request.provenance.is_empty() || visual.provenance.is_empty() {
        return finish(
            BridgeStatus::Missing,
            None,
            None,
            visual.status,
            vec!["visual and bridge provenance are required".into()],
            provenance,
        );
    }
    if let Some(ambiguity) = &request.ambiguity {
        return finish(
            BridgeStatus::Ambiguous,
            None,
            None,
            visual.status,
            vec![ambiguity.clone()],
            provenance,
        );
    }
    if request.operation != ChemistryOperation::ParseFormula {
        return finish(
            BridgeStatus::Unsupported,
            None,
            None,
            visual.status,
            vec!["visual atom inventories do not authorize reaction or ratio operations".into()],
            provenance,
        );
    }
    let artifact = visual.artifact.as_ref().expect("checked above");
    let formula = formula_from_atoms(artifact);
    let chemistry = evaluate_chemistry(&ChemistryRequest {
        operation: ChemistryOperation::ParseFormula,
        formula: Some(formula.clone()),
        reaction: None,
        from_species: None,
        to_species: None,
        domain: "source_derived_bounded_chemistry".into(),
        ambiguity: None,
        provenance: provenance.clone(),
    });
    let status = match chemistry.status {
        ChemistryStatus::Complete if chemistry.replay_verified() => BridgeStatus::Complete,
        ChemistryStatus::Ambiguous => BridgeStatus::Ambiguous,
        ChemistryStatus::Unsupported | ChemistryStatus::InvalidDomain => BridgeStatus::Unsupported,
        ChemistryStatus::Missing => BridgeStatus::Missing,
        ChemistryStatus::Inconsistent => BridgeStatus::Invalid,
        ChemistryStatus::Complete => BridgeStatus::Invalid,
    };
    finish(
        status,
        Some(formula),
        Some(chemistry),
        visual.status,
        Vec::new(),
        provenance,
    )
}

impl ChemicalBridgeResult {
    pub fn replay_verified(&self) -> bool {
        self.replay_hash == digest(&payload(self))
    }

    pub fn authorized(&self) -> bool {
        self.status == BridgeStatus::Complete
            && self.replay_verified()
            && self.chemistry_result.as_ref().is_some_and(|result| {
                result.status == ChemistryStatus::Complete
                    && result.replay_verified()
                    && matches!(
                        result.artifact,
                        Some(ChemistryArtifact::MolecularFormula { .. })
                    )
            })
            && !self.provenance.is_empty()
    }
}
