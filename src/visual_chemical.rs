//! Conservative visual chemical-structure frontend.
//!
//! The frontend records explicit atoms and bonds with provenance.  It does
//! not infer molecular formulae, valence, charges, aromaticity,
//! stereochemistry, reaction products, or molecular properties.

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::BTreeSet;

pub const DOMAIN: &str = "visual_bounded_chemical_structure";
const MAX_ATOMS: usize = 64;
const MAX_BONDS: usize = 128;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ChemicalVisualStatus {
    Complete,
    Missing,
    Ambiguous,
    Unsupported,
    Invalid,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct VisualAtomObservation {
    pub id: String,
    pub element: String,
    pub x: i32,
    pub y: i32,
    pub confidence: u8,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct VisualBondObservation {
    pub id: String,
    pub from: String,
    pub to: String,
    pub order: Option<u8>,
    pub confidence: u8,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct VisualChemicalObservation {
    pub semantic_label: Option<String>,
    pub scope: Option<String>,
    pub atoms: Vec<VisualAtomObservation>,
    pub bonds: Vec<VisualBondObservation>,
    pub ambiguity: Option<String>,
    pub provenance: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct VisualChemicalArtifact {
    pub scope: String,
    pub atoms: Vec<VisualAtomObservation>,
    pub bonds: Vec<VisualBondObservation>,
    pub provenance: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct VisualChemicalResult {
    pub status: ChemicalVisualStatus,
    pub artifact: Option<VisualChemicalArtifact>,
    pub alternatives: Vec<String>,
    pub reasons: Vec<String>,
    pub provenance: Vec<String>,
    pub replay_hash: String,
}

fn digest<T: Serialize>(value: &T) -> String {
    format!(
        "{:x}",
        Sha256::digest(serde_json::to_vec(value).expect("visual chemistry serializes"))
    )
}

fn payload(result: &VisualChemicalResult) -> impl Serialize + '_ {
    (
        result.status,
        &result.artifact,
        &result.alternatives,
        &result.reasons,
        &result.provenance,
    )
}

fn finish(
    status: ChemicalVisualStatus,
    artifact: Option<VisualChemicalArtifact>,
    alternatives: Vec<String>,
    reasons: Vec<String>,
    provenance: Vec<String>,
) -> VisualChemicalResult {
    let mut result = VisualChemicalResult {
        status,
        artifact,
        alternatives,
        reasons,
        provenance,
        replay_hash: String::new(),
    };
    let replay_hash = digest(&(
        result.status,
        &result.artifact,
        &result.alternatives,
        &result.reasons,
        &result.provenance,
    ));
    result.replay_hash = replay_hash;
    result
}

fn supported_element(element: &str) -> bool {
    matches!(
        element,
        "H" | "He"
            | "B"
            | "C"
            | "N"
            | "O"
            | "F"
            | "Ne"
            | "Na"
            | "Mg"
            | "Al"
            | "Si"
            | "P"
            | "S"
            | "Cl"
            | "Ar"
            | "K"
            | "Ca"
            | "Fe"
            | "Cu"
            | "Zn"
            | "Br"
            | "I"
    )
}

pub fn formalize_visual_chemical(input: &VisualChemicalObservation) -> VisualChemicalResult {
    if input.provenance.is_empty() {
        return finish(
            ChemicalVisualStatus::Missing,
            None,
            Vec::new(),
            vec!["chemical-structure observations need provenance".into()],
            input.provenance.clone(),
        );
    }
    if let Some(ambiguity) = &input.ambiguity {
        return finish(
            ChemicalVisualStatus::Ambiguous,
            None,
            vec![ambiguity.clone()],
            vec!["visual extractor reported unresolved atom or bond alternatives".into()],
            input.provenance.clone(),
        );
    }
    if input.semantic_label.as_deref() != Some("bounded_chemical_structure") {
        return finish(
            ChemicalVisualStatus::Unsupported,
            None,
            Vec::new(),
            vec!["visual structure does not establish bounded chemical semantics".into()],
            input.provenance.clone(),
        );
    }
    let scope = match input.scope.as_deref() {
        Some(scope) if !scope.trim().is_empty() => scope.to_owned(),
        _ => {
            return finish(
                ChemicalVisualStatus::Missing,
                None,
                Vec::new(),
                vec!["single-structure scope must be explicit".into()],
                input.provenance.clone(),
            )
        }
    };
    if input.atoms.is_empty() {
        return finish(
            ChemicalVisualStatus::Missing,
            None,
            Vec::new(),
            vec!["at least one explicit atom is required".into()],
            input.provenance.clone(),
        );
    }
    if input.atoms.len() > MAX_ATOMS || input.bonds.len() > MAX_BONDS {
        return finish(
            ChemicalVisualStatus::Unsupported,
            None,
            Vec::new(),
            vec!["structure exceeds the bounded atom or bond budget".into()],
            input.provenance.clone(),
        );
    }
    let mut atom_ids = BTreeSet::new();
    for atom in &input.atoms {
        if atom.id.trim().is_empty() || !atom_ids.insert(atom.id.clone()) {
            return finish(
                ChemicalVisualStatus::Invalid,
                None,
                Vec::new(),
                vec!["atom identities must be unique and nonempty".into()],
                input.provenance.clone(),
            );
        }
        if !supported_element(&atom.element) {
            return finish(
                ChemicalVisualStatus::Unsupported,
                None,
                Vec::new(),
                vec![format!(
                    "element {} is outside the bounded visual vocabulary",
                    atom.element
                )],
                input.provenance.clone(),
            );
        }
        if atom.confidence < 80 {
            return finish(
                ChemicalVisualStatus::Ambiguous,
                None,
                vec![atom.id.clone()],
                vec!["atom confidence is below the semantic boundary".into()],
                input.provenance.clone(),
            );
        }
    }
    let mut bond_ids = BTreeSet::new();
    let mut edges = BTreeSet::new();
    for bond in &input.bonds {
        if bond.id.trim().is_empty() || !bond_ids.insert(bond.id.clone()) {
            return finish(
                ChemicalVisualStatus::Invalid,
                None,
                Vec::new(),
                vec!["bond identities must be unique and nonempty".into()],
                input.provenance.clone(),
            );
        }
        if !atom_ids.contains(&bond.from) || !atom_ids.contains(&bond.to) || bond.from == bond.to {
            return finish(
                ChemicalVisualStatus::Invalid,
                None,
                Vec::new(),
                vec!["bond endpoints must be distinct explicit atoms".into()],
                input.provenance.clone(),
            );
        }
        if bond.order.is_some_and(|order| !(1..=3).contains(&order)) {
            return finish(
                ChemicalVisualStatus::Unsupported,
                None,
                Vec::new(),
                vec!["only explicit single, double, or triple bond orders are supported".into()],
                input.provenance.clone(),
            );
        }
        if bond.confidence < 80 {
            return finish(
                ChemicalVisualStatus::Ambiguous,
                None,
                vec![bond.id.clone()],
                vec!["bond confidence is below the semantic boundary".into()],
                input.provenance.clone(),
            );
        }
        let edge = if bond.from <= bond.to {
            (bond.from.clone(), bond.to.clone())
        } else {
            (bond.to.clone(), bond.from.clone())
        };
        if !edges.insert(edge) {
            return finish(
                ChemicalVisualStatus::Invalid,
                None,
                Vec::new(),
                vec!["duplicate atom connections are not identity-safe".into()],
                input.provenance.clone(),
            );
        }
    }
    finish(
        ChemicalVisualStatus::Complete,
        Some(VisualChemicalArtifact {
            scope,
            atoms: input.atoms.clone(),
            bonds: input.bonds.clone(),
            provenance: input.provenance.clone(),
        }),
        Vec::new(),
        Vec::new(),
        input.provenance.clone(),
    )
}

impl VisualChemicalResult {
    pub fn replay_verified(&self) -> bool {
        self.replay_hash == digest(&payload(self))
    }

    pub fn authorized(&self) -> bool {
        self.status == ChemicalVisualStatus::Complete
            && self.replay_verified()
            && self.artifact.as_ref().is_some_and(|artifact| {
                artifact.scope == "single_molecule" && !artifact.provenance.is_empty()
            })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn observation() -> VisualChemicalObservation {
        VisualChemicalObservation {
            semantic_label: Some("bounded_chemical_structure".into()),
            scope: Some("single_molecule".into()),
            atoms: vec![VisualAtomObservation {
                id: "O1".into(),
                element: "O".into(),
                x: 0,
                y: 0,
                confidence: 99,
            }],
            bonds: Vec::new(),
            ambiguity: None,
            provenance: vec!["chemical-visual:test".into()],
        }
    }

    #[test]
    fn explicit_atoms_replay_without_chemical_inference() {
        let result = formalize_visual_chemical(&observation());
        assert_eq!(result.status, ChemicalVisualStatus::Complete);
        assert!(result.authorized());
        let mut tampered = result.clone();
        tampered.replay_hash.push('x');
        assert!(!tampered.replay_verified());
    }

    #[test]
    fn missing_scope_is_fail_closed() {
        let mut input = observation();
        input.scope = None;
        assert_eq!(
            formalize_visual_chemical(&input).status,
            ChemicalVisualStatus::Missing
        );
    }
}
