//! Cross-pack validation for source-derived science and classical mechanics.

use serde::Serialize;
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use the_machine::classical_mechanics_pack::{
    classical_mechanics_pack, evaluate_mechanics, MechanicsEvaluationRequest, MechanicsStatus,
    NumericBinding,
};
use the_machine::probability_pack::Rational;
use the_machine::science_law_pack::{evaluate_science, ScienceRequest, ScienceStatus};

fn rational(numerator: i128, denominator: i128) -> Rational {
    Rational::new(numerator, denominator).unwrap()
}

fn digest<T: Serialize>(value: &T) -> String {
    format!("{:x}", Sha256::digest(serde_json::to_vec(value).unwrap()))
}

fn science_request(law: &str) -> ScienceRequest {
    ScienceRequest {
        law: law.into(),
        inputs: BTreeMap::from([
            ("m".into(), rational(4, 1)),
            ("v".into(), rational(3, 1)),
            ("k".into(), rational(2, 1)),
            ("x".into(), rational(5, 1)),
            ("n".into(), rational(1, 1)),
            ("R".into(), rational(8, 1)),
            ("T".into(), rational(300, 1)),
            ("V".into(), rational(100, 1)),
            ("Q".into(), rational(100, 1)),
            ("W".into(), rational(40, 1)),
        ]),
        domain: "source_derived_classical_science".into(),
        unit_scope: "si_consistent_exact".into(),
        ambiguity: None,
        provenance: vec!["science-composition-independent-corpus".into()],
    }
}

fn mechanics_request(law: &str, output: &str) -> MechanicsEvaluationRequest {
    let bindings = match law {
        "kinetic_energy" => vec![
            NumericBinding {
                symbol: "m".into(),
                value: 4.0,
                unit: "kg".into(),
                provenance: "composition:m".into(),
            },
            NumericBinding {
                symbol: "v".into(),
                value: 3.0,
                unit: "m/s".into(),
                provenance: "composition:v".into(),
            },
        ],
        "hooke_force" => vec![
            NumericBinding {
                symbol: "k".into(),
                value: 2.0,
                unit: "N/m".into(),
                provenance: "composition:k".into(),
            },
            NumericBinding {
                symbol: "x".into(),
                value: 5.0,
                unit: "m".into(),
                provenance: "composition:x".into(),
            },
        ],
        _ => Vec::new(),
    };
    MechanicsEvaluationRequest {
        law_id: law.into(),
        bindings,
        requested_output: output.into(),
    }
}

fn same_exact_value(science_value: &Rational, mechanics_value: f64) -> bool {
    (mechanics_value - science_value.numerator as f64 / science_value.denominator as f64).abs()
        < f64::EPSILON
}

fn main() {
    let pack = classical_mechanics_pack();
    let mut exact_decisions = 0usize;
    let mut supported = 0usize;
    let mut ambiguous = 0usize;
    let mut refused = 0usize;
    let mut replay_verified = 0usize;
    let mut tamper_rejected = 0usize;
    let mut equivalent_routes = 0usize;
    let mut false_authorizations = 0usize;
    let mut route_records = Vec::new();

    for index in 0..120 {
        let law = if index % 2 == 0 {
            "kinetic_energy"
        } else {
            "hooke_force"
        };
        let output = if law == "kinetic_energy" {
            "K"
        } else {
            "F_spring"
        };
        let science = evaluate_science(&science_request(law));
        let mechanics = evaluate_mechanics(&mechanics_request(law, output), &pack);
        let equivalent = science.status == ScienceStatus::Complete
            && mechanics.status == MechanicsStatus::Complete
            && science.value.as_ref().is_some_and(|value| {
                mechanics
                    .value
                    .is_some_and(|other| same_exact_value(value, other))
            })
            && science.law_id.as_deref() == Some(law)
            && mechanics.law_id.as_deref() == Some(law);
        exact_decisions += usize::from(equivalent);
        supported += usize::from(equivalent);
        equivalent_routes += usize::from(equivalent);
        replay_verified += usize::from(science.replay_verified());
        replay_verified += usize::from(the_machine::classical_mechanics_pack::replay_mechanics(
            &mechanics,
        ));
        let mut altered_science = science.clone();
        altered_science.replay_hash.push('x');
        let mut altered_mechanics = mechanics.clone();
        altered_mechanics.replay_hash.push('x');
        tamper_rejected += usize::from(!altered_science.replay_verified());
        tamper_rejected += usize::from(!the_machine::classical_mechanics_pack::replay_mechanics(
            &altered_mechanics,
        ));
        route_records.push((law, "equivalent", equivalent));
    }

    for _ in 0..40 {
        let mut science_req = science_request("kinetic_energy");
        science_req.ambiguity = Some("energy law alias is not unique across packs".into());
        let science = evaluate_science(&science_req);
        let mechanics = evaluate_mechanics(&mechanics_request("energy", "K"), &pack);
        let safe = science.status == ScienceStatus::Ambiguous
            && mechanics.status == MechanicsStatus::Ambiguous;
        exact_decisions += usize::from(safe);
        ambiguous += usize::from(safe);
        replay_verified += usize::from(science.replay_verified());
        replay_verified += usize::from(the_machine::classical_mechanics_pack::replay_mechanics(
            &mechanics,
        ));
        let mut altered_science = science.clone();
        altered_science.replay_hash.push('x');
        let mut altered_mechanics = mechanics.clone();
        altered_mechanics.replay_hash.push('x');
        tamper_rejected += usize::from(!altered_science.replay_verified());
        tamper_rejected += usize::from(!the_machine::classical_mechanics_pack::replay_mechanics(
            &altered_mechanics,
        ));
        false_authorizations += usize::from(!safe);
    }

    for index in 0..80 {
        let law = match index % 4 {
            0 => "ideal_gas_pressure",
            1 => "first_law_delta_u",
            2 => "unknown_law",
            _ => "kinetic_energy",
        };
        let mut science_req = science_request(law);
        if index % 4 == 3 {
            science_req.unit_scope = "mixed_units".into();
        }
        let science = evaluate_science(&science_req);
        let mechanics = evaluate_mechanics(&mechanics_request(law, "K"), &pack);
        let safe = science.status != ScienceStatus::Complete
            || mechanics.status != MechanicsStatus::Complete;
        exact_decisions += usize::from(safe);
        refused += usize::from(safe);
        replay_verified += usize::from(science.replay_verified());
        replay_verified += usize::from(the_machine::classical_mechanics_pack::replay_mechanics(
            &mechanics,
        ));
        let mut altered_science = science.clone();
        altered_science.replay_hash.push('x');
        let mut altered_mechanics = mechanics.clone();
        altered_mechanics.replay_hash.push('x');
        tamper_rejected += usize::from(!altered_science.replay_verified());
        tamper_rejected += usize::from(!the_machine::classical_mechanics_pack::replay_mechanics(
            &altered_mechanics,
        ));
        false_authorizations += usize::from(!safe);
        route_records.push((law, "refused", safe));
    }

    assert_eq!(exact_decisions, 240);
    assert_eq!(supported, 120);
    assert_eq!(ambiguous, 40);
    assert_eq!(refused, 80);
    assert_eq!(equivalent_routes, 120);
    assert_eq!(replay_verified, 480);
    assert_eq!(tamper_rejected, 480);
    assert_eq!(false_authorizations, 0);
    let report = serde_json::json!({
        "schema": "stage-h-science-composition-v1",
        "cases": 240,
        "supported": supported,
        "ambiguous": ambiguous,
        "refused": refused,
        "exact_decisions": exact_decisions,
        "equivalent_routes": equivalent_routes,
        "replay_verified": replay_verified,
        "tamper_rejected": tamper_rejected,
        "false_authorizations": false_authorizations,
        "route_records_hash": digest(&route_records),
    });
    let serialized = serde_json::to_string_pretty(&report).unwrap();
    std::fs::write(
        "docs/stage_h_science_composition.json",
        format!("{serialized}\n"),
    )
    .unwrap();
    println!("{serialized}");
}
