//! Independent source-derived science-law validation campaign.

use serde::Serialize;
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use the_machine::probability_pack::Rational;
use the_machine::science_law_pack::{evaluate_science, ScienceRequest, ScienceStatus};

fn rational(numerator: i128, denominator: i128) -> Rational {
    Rational::new(numerator, denominator).unwrap()
}
fn digest<T: Serialize>(value: &T) -> String {
    format!("{:x}", Sha256::digest(serde_json::to_vec(value).unwrap()))
}

fn request(law: &str) -> ScienceRequest {
    ScienceRequest {
        law: law.into(),
        inputs: BTreeMap::from([
            ("n".into(), rational(1, 1)),
            ("R".into(), rational(8, 1)),
            ("T".into(), rational(300, 1)),
            ("V".into(), rational(100, 1)),
            ("Q".into(), rational(100, 1)),
            ("W".into(), rational(40, 1)),
            ("m".into(), rational(4, 1)),
            ("v".into(), rational(3, 1)),
            ("k".into(), rational(2, 1)),
            ("x".into(), rational(5, 1)),
        ]),
        domain: "source_derived_classical_science".into(),
        unit_scope: "si_consistent_exact".into(),
        ambiguity: None,
        provenance: vec!["science-independent-corpus".into()],
    }
}

fn expected_value(law: &str) -> Option<Rational> {
    Some(match law {
        "ideal_gas_pressure" => rational(24, 1),
        "first_law_delta_u" => rational(60, 1),
        "kinetic_energy" => rational(18, 1),
        "hooke_force" => rational(-10, 1),
        _ => return None,
    })
}

fn main() {
    let laws = [
        "ideal_gas_pressure",
        "first_law_delta_u",
        "kinetic_energy",
        "hooke_force",
    ];
    let mut exact = 0;
    let mut replay = 0;
    let mut tamper = 0;
    let mut value_correct = 0;
    let mut false_auth = 0;
    for index in 0..120 {
        let law = laws[index % laws.len()];
        let result = evaluate_science(&request(law));
        let ok = result.status == ScienceStatus::Complete
            && result.value == expected_value(law)
            && result.source.is_some();
        exact += usize::from(ok);
        value_correct += usize::from(result.value == expected_value(law));
        replay += usize::from(result.replay_verified());
        let mut altered = result.clone();
        altered.replay_hash.push('x');
        tamper += usize::from(!altered.replay_verified());
    }
    for index in 0..40 {
        let law = laws[index % laws.len()];
        let mut req = request(law);
        req.ambiguity = Some("formulation or sign convention is unresolved".into());
        let result = evaluate_science(&req);
        exact += usize::from(result.status == ScienceStatus::Ambiguous);
        replay += usize::from(result.replay_verified());
        let mut altered = result.clone();
        altered.replay_hash.push('x');
        tamper += usize::from(!altered.replay_verified());
    }
    for index in 0..20 {
        let mut req = request("unknown_law");
        req.law = format!("unknown_law_{index}");
        let result = evaluate_science(&req);
        exact += usize::from(result.status != ScienceStatus::Complete);
        replay += usize::from(result.replay_verified());
        let mut altered = result.clone();
        altered.replay_hash.push('x');
        tamper += usize::from(!altered.replay_verified());
        false_auth += usize::from(result.status == ScienceStatus::Complete);
    }
    for _ in 0..20 {
        let mut req = request("ideal_gas_pressure");
        req.inputs.insert("V".into(), rational(0, 1));
        let result = evaluate_science(&req);
        exact += usize::from(result.status == ScienceStatus::Inconsistent);
        replay += usize::from(result.replay_verified());
        let mut altered = result.clone();
        altered.replay_hash.push('x');
        tamper += usize::from(!altered.replay_verified());
        false_auth += usize::from(result.status == ScienceStatus::Complete);
    }
    for _ in 0..20 {
        let mut req = request("kinetic_energy");
        req.unit_scope = "mixed_units".into();
        let result = evaluate_science(&req);
        exact += usize::from(result.status == ScienceStatus::InvalidDomain);
        replay += usize::from(result.replay_verified());
        let mut altered = result.clone();
        altered.replay_hash.push('x');
        tamper += usize::from(!altered.replay_verified());
        false_auth += usize::from(result.status == ScienceStatus::Complete);
    }
    for _ in 0..20 {
        let mut req = request("first_law_delta_u");
        req.inputs.remove("Q");
        let result = evaluate_science(&req);
        exact += usize::from(result.status == ScienceStatus::Missing);
        replay += usize::from(result.replay_verified());
        let mut altered = result.clone();
        altered.replay_hash.push('x');
        tamper += usize::from(!altered.replay_verified());
        false_auth += usize::from(result.status == ScienceStatus::Complete);
    }
    assert_eq!(exact, 240);
    assert_eq!(value_correct, 120);
    assert_eq!(replay, 240);
    assert_eq!(tamper, 240);
    assert_eq!(false_auth, 0);
    let report = serde_json::json!({
        "schema": "stage-h-source-derived-science-v1",
        "cases": 240,
        "supported": 120,
        "ambiguous": 40,
        "refused": 80,
        "exact_decisions": exact,
        "exact_values": value_correct,
        "replay_verified": replay,
        "tamper_rejected": tamper,
        "false_authorizations": false_auth,
        "source": "OpenStax University Physics Volume 1",
        "report_hash": digest(&(exact, value_correct, replay, tamper, false_auth)),
    });
    let serialized = serde_json::to_string_pretty(&report).unwrap();
    std::fs::write(
        "docs/stage_h_science_law_pack.json",
        format!("{serialized}\n"),
    )
    .unwrap();
    println!("{serialized}");
}
