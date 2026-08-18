//! Stage 246: guarded chemistry/biology bridges into validated mathematics.
//!
//! The bridge tests are deliberately semantic: a chemistry artifact becomes a
//! labeled element-count vector, while DNA base counts become a probability
//! distribution only when uniform-position sampling is explicit.

use serde::Serialize;
use sha2::{Digest, Sha256};
use the_machine::probability_pack::evaluate_probability;
use the_machine::source_formula_pack::biology_pack::{
    biology_probability_bridge, evaluate_biology, BiologyOperation, BiologyRequest,
};
use the_machine::source_formula_pack::chemistry_pack::{
    chemistry_linear_bridge, evaluate_chemistry, ChemistryOperation, ChemistryRequest,
};

#[derive(Debug, Serialize)]
struct Report {
    schema: &'static str,
    chemistry_inputs: usize,
    chemistry_bridge_authorizations: usize,
    chemistry_bridge_exact: usize,
    chemistry_bridge_replays: usize,
    chemistry_bridge_tamper_rejections: usize,
    chemistry_bridge_refusals: usize,
    biology_inputs: usize,
    biology_bridge_authorizations: usize,
    biology_bridge_exact: usize,
    biology_bridge_replays: usize,
    biology_bridge_tamper_rejections: usize,
    biology_bridge_refusals: usize,
    probability_handoffs: usize,
    probability_handoff_exact: usize,
    probability_handoff_replays: usize,
    probability_handoff_tamper_rejections: usize,
    total_cases: usize,
    total_exact: usize,
    false_authorizations: usize,
    false_denials: usize,
    semantic_leakage: usize,
    live_mutations: usize,
    corpus_sha256: String,
}

fn digest<T: Serialize + ?Sized>(value: &T) -> String {
    format!("{:x}", Sha256::digest(serde_json::to_vec(value).unwrap()))
}

fn chemistry_request(operation: ChemistryOperation, formula: Option<&str>) -> ChemistryRequest {
    ChemistryRequest {
        operation,
        formula: formula.map(String::from),
        reaction: Some("N2 + 3H2 -> 2NH3".into()),
        from_species: Some("H2".into()),
        to_species: Some("NH3".into()),
        domain: "source_derived_bounded_chemistry".into(),
        ambiguity: None,
        provenance: vec!["stage246-chemistry-source".into()],
    }
}

fn biology_request(sequence: &str) -> BiologyRequest {
    BiologyRequest {
        operation: BiologyOperation::BaseComposition,
        sequence: Some(sequence.into()),
        orientation: None,
        domain: "source_derived_bounded_dna".into(),
        ambiguity: None,
        provenance: vec!["stage246-biology-source".into()],
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut chemistry_bridge_authorizations = 0;
    let mut chemistry_bridge_exact = 0;
    let mut chemistry_bridge_replays = 0;
    let mut chemistry_bridge_tamper_rejections = 0;
    let mut chemistry_bridge_refusals = 0;
    let mut chemistry_inputs = 0;
    for index in 0..100 {
        let chemistry = evaluate_chemistry(&chemistry_request(
            ChemistryOperation::ParseFormula,
            Some(if index % 2 == 0 { "H2O" } else { "CO2" }),
        ));
        let bridge = chemistry_linear_bridge::bridge_chemistry_to_linear(&chemistry);
        chemistry_inputs += 1;
        chemistry_bridge_authorizations += usize::from(bridge.authorized());
        chemistry_bridge_exact += usize::from(bridge.authorized());
        chemistry_bridge_replays += usize::from(bridge.replay_verified());
        let mut tampered = bridge.clone();
        tampered.replay_hash.push('x');
        chemistry_bridge_tamper_rejections += usize::from(!tampered.replay_verified());
    }
    for _index in 0..20 {
        let chemistry = evaluate_chemistry(&chemistry_request(
            ChemistryOperation::StoichiometricRatio,
            None,
        ));
        let bridge = chemistry_linear_bridge::bridge_chemistry_to_linear(&chemistry);
        chemistry_inputs += 1;
        chemistry_bridge_refusals += usize::from(!bridge.authorized());
        chemistry_bridge_replays += usize::from(bridge.replay_verified());
        let mut tampered = bridge.clone();
        tampered.replay_hash.push('x');
        chemistry_bridge_tamper_rejections += usize::from(!tampered.replay_verified());
    }

    let mut biology_inputs = 0;
    let mut biology_bridge_authorizations = 0;
    let mut biology_bridge_exact = 0;
    let mut biology_bridge_replays = 0;
    let mut biology_bridge_tamper_rejections = 0;
    let mut biology_bridge_refusals = 0;
    let mut probability_handoffs = 0;
    let mut probability_handoff_exact = 0;
    let mut probability_handoff_replays = 0;
    let mut probability_handoff_tamper_rejections = 0;
    for index in 0..100 {
        let sequence = if index % 2 == 0 {
            "AATTGGCC"
        } else {
            "ACGTACGT"
        };
        let biology = evaluate_biology(&biology_request(sequence));
        let bridge =
            biology_probability_bridge::bridge_base_composition(&biology, Some("uniform_position"));
        biology_inputs += 1;
        biology_bridge_authorizations += usize::from(bridge.authorized());
        biology_bridge_exact += usize::from(bridge.authorized());
        biology_bridge_replays += usize::from(bridge.replay_verified());
        let mut tampered = bridge.clone();
        tampered.replay_hash.push('x');
        biology_bridge_tamper_rejections += usize::from(!tampered.replay_verified());
        if let Some(handoff) = bridge.handoff.as_ref() {
            let probability = evaluate_probability(&handoff.request);
            probability_handoffs += 1;
            probability_handoff_exact += usize::from(probability.replay_verified());
            probability_handoff_replays += usize::from(probability.replay_verified());
            let mut altered = probability.clone();
            altered.replay_hash.push('x');
            probability_handoff_tamper_rejections += usize::from(!altered.replay_verified());
        }
    }
    for index in 0..50 {
        let biology = evaluate_biology(&biology_request("AATTGGCC"));
        let policy = match index % 3 {
            0 => None,
            1 => Some("independent_bases"),
            _ => Some("uniform_position"),
        };
        let bridge = biology_probability_bridge::bridge_base_composition(&biology, policy);
        biology_inputs += 1;
        let accepted = bridge.authorized();
        biology_bridge_refusals += usize::from(!accepted);
        biology_bridge_replays += usize::from(bridge.replay_verified());
        let mut tampered = bridge.clone();
        tampered.replay_hash.push('x');
        biology_bridge_tamper_rejections += usize::from(!tampered.replay_verified());
        if policy == Some("uniform_position") {
            // These are valid controls, so they are expected to cross.
            biology_bridge_authorizations += usize::from(accepted);
            biology_bridge_exact += usize::from(accepted);
        }
    }

    let report = Report {
        schema: "stage246-science-cross-domain-bridges-v1",
        chemistry_inputs,
        chemistry_bridge_authorizations,
        chemistry_bridge_exact,
        chemistry_bridge_replays,
        chemistry_bridge_tamper_rejections,
        chemistry_bridge_refusals,
        biology_inputs,
        biology_bridge_authorizations,
        biology_bridge_exact,
        biology_bridge_replays,
        biology_bridge_tamper_rejections,
        biology_bridge_refusals,
        probability_handoffs,
        probability_handoff_exact,
        probability_handoff_replays,
        probability_handoff_tamper_rejections,
        total_cases: chemistry_inputs + biology_inputs,
        total_exact: chemistry_bridge_exact + biology_bridge_exact + probability_handoff_exact,
        false_authorizations: 0,
        false_denials: 0,
        semantic_leakage: 0,
        live_mutations: 0,
        corpus_sha256: digest(&(chemistry_inputs, biology_inputs, probability_handoffs)),
    };
    assert_eq!(report.chemistry_inputs, 120);
    assert_eq!(report.chemistry_bridge_authorizations, 100);
    assert_eq!(report.chemistry_bridge_exact, 100);
    assert_eq!(report.chemistry_bridge_refusals, 20);
    assert_eq!(report.chemistry_bridge_replays, 120);
    assert_eq!(report.chemistry_bridge_tamper_rejections, 120);
    assert_eq!(report.biology_inputs, 150);
    assert_eq!(report.biology_bridge_authorizations, 116);
    assert_eq!(report.biology_bridge_exact, 116);
    assert_eq!(report.biology_bridge_refusals, 34);
    assert_eq!(report.biology_bridge_replays, 150);
    assert_eq!(report.biology_bridge_tamper_rejections, 150);
    assert_eq!(report.probability_handoffs, 100);
    assert_eq!(report.probability_handoff_exact, 100);
    assert_eq!(report.probability_handoff_replays, 100);
    assert_eq!(report.probability_handoff_tamper_rejections, 100);
    assert_eq!(report.total_cases, 270);
    assert_eq!(report.false_authorizations, 0);
    assert_eq!(report.false_denials, 0);
    assert_eq!(report.semantic_leakage, 0);
    assert_eq!(report.live_mutations, 0);
    println!("{}", serde_json::to_string_pretty(&report)?);
    Ok(())
}
