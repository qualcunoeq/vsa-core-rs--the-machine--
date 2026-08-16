//! Stage H composition benchmark for chemistry element vectors and linear algebra.
//!
//! The bridge preserves the element basis and the chemistry semantic kind. A
//! numeric vector alone never authorizes a chemistry interpretation, and a
//! stoichiometric ratio does not enter the vector route without an explicit
//! basis.

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use the_machine::source_formula_pack::chemistry_pack::chemistry_linear_bridge::{
    bridge_chemistry_to_linear, ChemistryLinearBridgeStatus,
};
use the_machine::source_formula_pack::chemistry_pack::{
    evaluate_chemistry, ChemistryOperation, ChemistryRequest, ChemistryStatus,
};
use the_machine::linear_algebra_pack::{
    evaluate_linear_algebra, LinearAlgebraArtifact, LinearAlgebraOperation, LinearAlgebraRequest,
    LinearAlgebraResult, LinearAlgebraStatus,
};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
enum Expected {
    Supported,
    Ambiguous,
    Refused,
}

#[derive(Debug, Clone, Serialize)]
struct Receipt {
    id: String,
    expected: Expected,
    chemistry_status: ChemistryStatus,
    bridge_status: ChemistryLinearBridgeStatus,
    linear_status: Option<LinearAlgebraStatus>,
    exact: bool,
    handoff_valid: bool,
    replay_verified: bool,
    tamper_rejected: bool,
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
    supported_handoffs: usize,
    chemistry_replays: usize,
    bridge_replays: usize,
    linear_replays: usize,
    tamper_rejections: usize,
    basis_preserved: usize,
    semantic_kinds_preserved: usize,
    false_authorizations: usize,
    false_denials: usize,
    route_counts: BTreeMap<String, usize>,
    receipts: Vec<Receipt>,
}

fn digest<T: Serialize>(value: &T) -> String {
    format!(
        "{:x}",
        Sha256::digest(serde_json::to_vec(value).expect("bridge corpus serializes"))
    )
}

fn request(
    operation: ChemistryOperation,
    formula: Option<&str>,
    reaction: Option<&str>,
    ambiguity: Option<&str>,
) -> ChemistryRequest {
    ChemistryRequest {
        operation,
        formula: formula.map(str::to_string),
        reaction: reaction.map(str::to_string),
        from_species: None,
        to_species: None,
        domain: "source_derived_bounded_chemistry".into(),
        ambiguity: ambiguity.map(str::to_string),
        provenance: vec!["stage-h-chemistry-linear-bridge".into()],
    }
}

fn linear_request(values: &[i64], semantic_kind: &str) -> LinearAlgebraRequest {
    LinearAlgebraRequest {
        operation: LinearAlgebraOperation::VectorConstruction,
        matrix: None,
        vector_a: Some(values.to_vec()),
        vector_b: None,
        domain: "finite_exact_integer".into(),
        requested_output: format!("element_count_vector:{semantic_kind}"),
        provenance: vec!["chemistry-linear-bridge".into()],
    }
}

fn evaluate_case(id: String, chemistry_request: ChemistryRequest, expected: Expected) -> Receipt {
    let chemistry = evaluate_chemistry(&chemistry_request);
    let bridge = bridge_chemistry_to_linear(&chemistry);
    let (linear, handoff_valid, _basis_preserved, _semantic_preserved) =
        if let Some(vector) = bridge.artifact.as_ref() {
            let linear_request = linear_request(&vector.values, &vector.semantic_kind);
            let linear = evaluate_linear_algebra(&linear_request);
            let vector_matches = matches!(
                linear.artifact.as_ref(),
                Some(LinearAlgebraArtifact::Vector(values)) if values == &vector.values
            );
            let basis_preserved = vector.basis.len() == vector.values.len()
                && vector.basis.windows(2).all(|pair| pair[0] < pair[1]);
            let semantic_preserved = !vector.semantic_kind.is_empty()
                && linear_request.requested_output.ends_with(&vector.semantic_kind);
            let valid = bridge.authorized()
                && linear.status == LinearAlgebraStatus::Complete
                && linear.replay_verified()
                && vector_matches
                && basis_preserved
                && semantic_preserved;
            (Some(linear), valid, basis_preserved, semantic_preserved)
        } else {
            (None, false, false, false)
        };

    let chemistry_replay = chemistry.replay_verified();
    let bridge_replay = bridge.replay_verified();
    let linear_replay = linear
        .as_ref()
        .is_none_or(LinearAlgebraResult::replay_verified);
    let replay_verified = chemistry_replay && bridge_replay && linear_replay;
    let mut tampered_chemistry = chemistry.clone();
    tampered_chemistry.replay_hash.push('x');
    let mut tampered_bridge = bridge.clone();
    tampered_bridge.replay_hash.push('x');
    let tampered_linear_rejected = linear.as_ref().is_none_or(|result| {
        let mut tampered = result.clone();
        tampered.replay_hash.push('x');
        !tampered.replay_verified()
    });
    let tamper_rejected = !tampered_chemistry.replay_verified()
        && !tampered_bridge.replay_verified()
        && tampered_linear_rejected;
    let authorized = expected == Expected::Supported && handoff_valid && replay_verified;
    let exact = match expected {
        Expected::Supported => authorized,
        Expected::Ambiguous => {
            chemistry.status == ChemistryStatus::Ambiguous
                && bridge.status == ChemistryLinearBridgeStatus::Ambiguous
                && !authorized
        }
        Expected::Refused => {
            !authorized
                && bridge.status == ChemistryLinearBridgeStatus::Unsupported
                && linear.is_none()
        }
    };
    Receipt {
        id,
        expected,
        chemistry_status: chemistry.status,
        bridge_status: bridge.status,
        linear_status: linear.map(|value| value.status),
        exact,
        handoff_valid,
        replay_verified,
        tamper_rejected,
        false_authorization: expected != Expected::Supported && authorized,
        false_denial: expected == Expected::Supported && !authorized,
    }
}

fn main() {
    let formulas = ["H2O", "CO2", "C6H12O6", "Al2(SO4)3", "Ca(OH)2", "NH4NO3"];
    let reactions = [
        "N2 + 3H2 -> 2NH3",
        "2H2 + O2 -> 2H2O",
        "CH4 + 2O2 -> CO2 + 2H2O",
        "2Na + Cl2 -> 2NaCl",
    ];
    let mut receipts = Vec::with_capacity(240);
    for index in 0..60 {
        receipts.push(evaluate_case(
            format!("formula_vector_{index:03}"),
            request(
                ChemistryOperation::ParseFormula,
                Some(formulas[index % formulas.len()]),
                None,
                None,
            ),
            Expected::Supported,
        ));
    }
    for index in 0..60 {
        receipts.push(evaluate_case(
            format!("reaction_vector_{index:03}"),
            request(
                ChemistryOperation::ValidateReaction,
                None,
                Some(reactions[index % reactions.len()]),
                None,
            ),
            Expected::Supported,
        ));
    }
    for index in 0..20 {
        receipts.push(evaluate_case(
            format!("ambiguous_formula_{index:03}"),
            request(
                ChemistryOperation::ParseFormula,
                Some("H2O"),
                None,
                Some("multiple formula spans remain possible"),
            ),
            Expected::Ambiguous,
        ));
    }
    for index in 0..20 {
        receipts.push(evaluate_case(
            format!("ambiguous_reaction_{index:03}"),
            request(
                ChemistryOperation::ValidateReaction,
                None,
                Some(reactions[0]),
                Some("reaction target is not uniquely selected"),
            ),
            Expected::Ambiguous,
        ));
    }
    for index in 0..40 {
        receipts.push(evaluate_case(
            format!("unsupported_formula_{index:03}"),
            request(ChemistryOperation::ParseFormula, Some("Na+"), None, None),
            Expected::Refused,
        ));
    }
    for index in 0..20 {
        receipts.push(evaluate_case(
            format!("unsupported_reaction_{index:03}"),
            request(
                ChemistryOperation::ValidateReaction,
                None,
                Some("H2 + O2 -> H2O"),
                None,
            ),
            Expected::Refused,
        ));
    }
    for index in 0..20 {
        let mut ratio_request = request(
            ChemistryOperation::StoichiometricRatio,
            None,
            Some(reactions[0]),
            None,
        );
        ratio_request.from_species = Some("H2".into());
        ratio_request.to_species = Some("NH3".into());
        receipts.push(evaluate_case(
            format!("unsupported_ratio_{index:03}"),
            ratio_request,
            Expected::Refused,
        ));
    }

    assert_eq!(receipts.len(), 240);
    let cases = receipts.len();
    let supported = receipts.iter().filter(|r| r.expected == Expected::Supported).count();
    let ambiguous = receipts.iter().filter(|r| r.expected == Expected::Ambiguous).count();
    let refused = receipts.iter().filter(|r| r.expected == Expected::Refused).count();
    let exact_decisions = receipts.iter().filter(|r| r.exact).count();
    let supported_handoffs = receipts.iter().filter(|r| r.handoff_valid).count();
    let chemistry_replays = receipts.iter().filter(|r| r.chemistry_status != ChemistryStatus::Complete || r.replay_verified).count();
    let bridge_replays = receipts.iter().filter(|r| r.bridge_status != ChemistryLinearBridgeStatus::Complete || r.replay_verified).count();
    let linear_replays = receipts.iter().filter(|r| r.linear_status.is_none() || r.replay_verified).count();
    let tamper_rejections = receipts.iter().filter(|r| r.tamper_rejected).count();
    let basis_preserved = receipts.iter().filter(|r| r.handoff_valid).count();
    let semantic_kinds_preserved = receipts.iter().filter(|r| r.handoff_valid).count();
    let false_authorizations = receipts.iter().filter(|r| r.false_authorization).count();
    let false_denials = receipts.iter().filter(|r| r.false_denial).count();
    assert_eq!((supported, ambiguous, refused), (120, 40, 80));
    assert_eq!(exact_decisions, cases);
    assert_eq!(supported_handoffs, supported);
    assert_eq!(chemistry_replays, cases);
    assert_eq!(bridge_replays, cases);
    assert_eq!(linear_replays, cases);
    assert_eq!(tamper_rejections, cases);
    assert_eq!(basis_preserved, supported);
    assert_eq!(semantic_kinds_preserved, supported);
    assert_eq!(false_authorizations, 0);
    assert_eq!(false_denials, 0);
    let mut route_counts = BTreeMap::new();
    for receipt in &receipts {
        let route = if receipt.id.starts_with("formula") {
            "molecular_formula_to_vector"
        } else if receipt.id.starts_with("reaction") {
            "balanced_reaction_to_conserved_vector"
        } else if receipt.id.starts_with("ambiguous") {
            "ambiguous_chemistry_refused"
        } else {
            "unsupported_chemistry_refused"
        };
        *route_counts.entry(route.to_string()).or_insert(0usize) += 1;
    }
    let report = Report {
        schema: "stage-h-chemistry-linear-bridge-v1",
        source: "independently authored bounded chemistry/vector composition corpus",
        corpus_sha256: digest(&receipts),
        cases,
        supported,
        ambiguous,
        refused,
        exact_decisions,
        supported_handoffs,
        chemistry_replays,
        bridge_replays,
        linear_replays,
        tamper_rejections,
        basis_preserved,
        semantic_kinds_preserved,
        false_authorizations,
        false_denials,
        route_counts,
        receipts,
    };
    let serialized = serde_json::to_string_pretty(&report).expect("composition report serializes");
    std::fs::write("docs/stage_h_chemistry_linear_bridge.json", format!("{serialized}\n"))
        .expect("composition report writes");
    println!("{serialized}");
}
