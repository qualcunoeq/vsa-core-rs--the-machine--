//! Independent validation campaign for the source-derived bounded chemistry pack.

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use the_machine::source_formula_pack::chemistry_pack::{
    evaluate_chemistry, ChemistryArtifact, ChemistryOperation, ChemistryRequest, ChemistryStatus,
};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
enum Expected {
    Complete,
    Ambiguous,
    Refused,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Receipt {
    id: String,
    operation: ChemistryOperation,
    expected: Expected,
    actual: ChemistryStatus,
    exact: bool,
    replay_verified: bool,
    tamper_rejected: bool,
    source_preserved: bool,
    artifact_shape_valid: bool,
    false_authorization: bool,
    false_denial: bool,
}

#[derive(Debug, Serialize)]
struct Report {
    schema: &'static str,
    source: &'static str,
    corpus_sha256: String,
    cases: usize,
    supported: usize,
    ambiguous: usize,
    refused: usize,
    exact_decisions: usize,
    supported_artifacts: usize,
    replay_verified: usize,
    tamper_rejections: usize,
    source_preserved: usize,
    artifact_shape_valid: usize,
    false_authorizations: usize,
    false_denials: usize,
    status_counts: BTreeMap<String, usize>,
    receipts: Vec<Receipt>,
}

const DOMAIN: &str = "source_derived_bounded_chemistry";

fn digest<T: Serialize>(value: &T) -> String {
    format!("{:x}", Sha256::digest(serde_json::to_vec(value).expect("chemistry serializes")))
}

fn request(operation: ChemistryOperation) -> ChemistryRequest {
    ChemistryRequest {
        operation,
        formula: None,
        reaction: None,
        from_species: None,
        to_species: None,
        domain: DOMAIN.into(),
        ambiguity: None,
        provenance: vec!["chemistry-independent-corpus".into()],
    }
}

fn run(id: String, request: ChemistryRequest, expected: Expected) -> Receipt {
    let result = evaluate_chemistry(&request);
    let artifact_shape_valid = match (&result.status, &result.artifact, request.operation) {
        (ChemistryStatus::Complete, Some(ChemistryArtifact::MolecularFormula { atoms }), ChemistryOperation::ParseFormula) => !atoms.is_empty(),
        (ChemistryStatus::Complete, Some(ChemistryArtifact::BalancedReaction { reactants, products, atom_totals }), ChemistryOperation::ValidateReaction) => !reactants.is_empty() && !products.is_empty() && !atom_totals.is_empty(),
        (ChemistryStatus::Complete, Some(ChemistryArtifact::StoichiometricRatio { from, to, from_coefficient, to_coefficient }), ChemistryOperation::StoichiometricRatio) => !from.is_empty() && !to.is_empty() && *from_coefficient > 0 && *to_coefficient > 0,
        (ChemistryStatus::Complete, _, _) => false,
        (_, None, _) => true,
        (_, Some(_), _) => false,
    };
    let exact = match expected {
        Expected::Complete => result.status == ChemistryStatus::Complete && artifact_shape_valid,
        Expected::Ambiguous => result.status == ChemistryStatus::Ambiguous,
        Expected::Refused => result.status != ChemistryStatus::Complete,
    };
    let replay_verified = result.replay_verified();
    let mut tampered = result.clone();
    tampered.replay_hash.push('x');
    Receipt {
        id,
        operation: request.operation,
        expected,
        actual: result.status,
        exact,
        replay_verified,
        tamper_rejected: !tampered.replay_verified(),
        source_preserved: expected == Expected::Complete && result.source.is_some(),
        artifact_shape_valid,
        false_authorization: expected != Expected::Complete && result.authorized(),
        false_denial: expected == Expected::Complete && !result.authorized(),
    }
}

fn main() {
    let formulas = ["H2O", "CO2", "C6H12O6", "Al2(SO4)3", "Ca(OH)2", "NH4NO3"];
    let mut receipts = Vec::with_capacity(240);

    for index in 0..60 {
        let mut req = request(ChemistryOperation::ParseFormula);
        req.formula = Some(formulas[index % formulas.len()].into());
        receipts.push(run(format!("formula_{index:03}"), req, Expected::Complete));
    }

    let reactions = [
        "N2 + 3H2 -> 2NH3",
        "2H2 + O2 -> 2H2O",
        "CH4 + 2O2 -> CO2 + 2H2O",
        "2Na + Cl2 -> 2NaCl",
        "CaCO3 -> CaO + CO2",
    ];
    for index in 0..40 {
        let mut req = request(ChemistryOperation::ValidateReaction);
        req.reaction = Some(reactions[index % reactions.len()].into());
        receipts.push(run(format!("reaction_{index:03}"), req, Expected::Complete));
    }

    for index in 0..20 {
        let mut req = request(ChemistryOperation::StoichiometricRatio);
        req.reaction = Some("N2 + 3H2 -> 2NH3".into());
        req.from_species = Some("H2".into());
        req.to_species = Some("NH3".into());
        receipts.push(run(format!("ratio_{index:03}"), req, Expected::Complete));
    }

    for index in 0..40 {
        let mut req = request(ChemistryOperation::ParseFormula);
        req.formula = Some("X".into());
        req.ambiguity = Some("symbol or notation family is unresolved".into());
        receipts.push(run(format!("ambiguous_{index:03}"), req, Expected::Ambiguous));
    }

    for index in 0..20 {
        let mut req = request(ChemistryOperation::ParseFormula);
        req.formula = Some("H2O)".into());
        receipts.push(run(format!("malformed_formula_{index:03}"), req, Expected::Refused));
    }
    for index in 0..20 {
        let mut req = request(ChemistryOperation::ParseFormula);
        req.formula = Some("Na+".into());
        receipts.push(run(format!("unsupported_charge_{index:03}"), req, Expected::Refused));
    }
    for index in 0..20 {
        let mut req = request(ChemistryOperation::ValidateReaction);
        req.reaction = Some("H2 + O2 -> H2O".into());
        receipts.push(run(format!("unbalanced_{index:03}"), req, Expected::Refused));
    }
    for index in 0..20 {
        let mut req = request(ChemistryOperation::StoichiometricRatio);
        req.reaction = Some("N2 + 3H2 -> 2NH3".into());
        req.from_species = Some("H2".into());
        req.to_species = None;
        receipts.push(run(format!("missing_ratio_target_{index:03}"), req, Expected::Refused));
    }

    assert_eq!(receipts.len(), 240);
    let cases = receipts.len();
    let supported = receipts.iter().filter(|r| r.expected == Expected::Complete).count();
    let ambiguous = receipts.iter().filter(|r| r.expected == Expected::Ambiguous).count();
    let refused = receipts.iter().filter(|r| r.expected == Expected::Refused).count();
    let exact_decisions = receipts.iter().filter(|r| r.exact).count();
    let supported_artifacts = receipts
        .iter()
        .filter(|r| r.expected == Expected::Complete && r.artifact_shape_valid)
        .count();
    let replay_verified = receipts.iter().filter(|r| r.replay_verified).count();
    let tamper_rejections = receipts.iter().filter(|r| r.tamper_rejected).count();
    let source_preserved = receipts.iter().filter(|r| r.source_preserved).count();
    let artifact_shape_valid = receipts.iter().filter(|r| r.artifact_shape_valid).count();
    let false_authorizations = receipts.iter().filter(|r| r.false_authorization).count();
    let false_denials = receipts.iter().filter(|r| r.false_denial).count();
    assert_eq!((supported, ambiguous, refused), (120, 40, 80));
    assert_eq!(exact_decisions, cases);
    assert_eq!(supported_artifacts, supported);
    assert_eq!(replay_verified, cases);
    assert_eq!(tamper_rejections, cases);
    assert_eq!(source_preserved, supported);
    assert_eq!(artifact_shape_valid, cases);
    assert_eq!(false_authorizations, 0);
    assert_eq!(false_denials, 0);
    let mut status_counts = BTreeMap::new();
    for receipt in &receipts {
        *status_counts.entry(format!("{:?}", receipt.actual)).or_insert(0usize) += 1;
    }
    let report = Report {
        schema: "stage-h-source-chemistry-pack-v1",
        source: "OpenStax Chemistry 2e sections 2.4, 4.1, and 4.3",
        corpus_sha256: digest(&receipts),
        cases,
        supported,
        ambiguous,
        refused,
        exact_decisions,
        supported_artifacts,
        replay_verified,
        tamper_rejections,
        source_preserved,
        artifact_shape_valid,
        false_authorizations,
        false_denials,
        status_counts,
        receipts,
    };
    let serialized = serde_json::to_string_pretty(&report).expect("chemistry report serializes");
    std::fs::write("docs/stage_h_source_chemistry_pack.json", format!("{serialized}\n"))
        .expect("chemistry report writes");
    println!("{serialized}");
}
