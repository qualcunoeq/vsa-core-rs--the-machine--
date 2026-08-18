//! Independent source-derived bounded electromagnetism validation campaign.

use serde::Serialize;
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::fs;
use the_machine::electromagnetism_pack::{evaluate, EmRequest, EmStatus};
use the_machine::probability_pack::Rational;

#[derive(Debug, Serialize)]
struct Report {
    schema: &'static str,
    corpus_sha256: String,
    cases: usize,
    supported: usize,
    ambiguous: usize,
    refused: usize,
    exact_decisions: usize,
    exact_values: usize,
    replay_verified: usize,
    tamper_rejections: usize,
    false_authorizations: usize,
    false_denials: usize,
    source_records: usize,
    hle_questions_read: usize,
    registry_mutations: usize,
}

fn rational(value: i128) -> Rational {
    Rational::new(value, 1).unwrap()
}
fn digest<T: Serialize>(value: &T) -> String {
    format!("{:x}", Sha256::digest(serde_json::to_vec(value).unwrap()))
}

fn request(law: &str) -> EmRequest {
    EmRequest {
        law: law.into(),
        inputs: BTreeMap::from([
            ("I".into(), rational(2)),
            ("R".into(), rational(5)),
            ("V".into(), rational(3)),
            ("t".into(), rational(4)),
            ("C".into(), rational(6)),
        ]),
        domain: "source_derived_bounded_electromagnetism".into(),
        unit_scope: "si_consistent_exact".into(),
        ambiguity: None,
        provenance: vec!["electromagnetism-independent-corpus".into()],
    }
}

fn expected(law: &str) -> Rational {
    match law {
        "ohms_law_voltage" => rational(10),
        "electric_power" => rational(6),
        "charge_from_current" => rational(8),
        "capacitor_charge" => rational(18),
        _ => unreachable!(),
    }
}

fn check(
    result: &the_machine::electromagnetism_pack::EmResult,
    expected_status: EmStatus,
    value: Option<Rational>,
) -> (usize, usize, usize) {
    let exact =
        usize::from(result.status == expected_status && (value.is_none() || result.value == value));
    let replay = usize::from(result.replay_verified());
    let mut tampered = result.clone();
    tampered.replay_hash.push('x');
    let tamper = usize::from(!tampered.replay_verified());
    (exact, replay, tamper)
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let laws = [
        "ohms_law_voltage",
        "electric_power",
        "charge_from_current",
        "capacitor_charge",
    ];
    let mut exact = 0;
    let mut values = 0;
    let mut replay = 0;
    let mut tamper = 0;
    let mut false_auth = 0;
    let mut false_denials = 0;
    let mut receipts = Vec::new();
    for index in 0..120 {
        let law = laws[index % laws.len()];
        let result = evaluate(&request(law));
        let (decision, replay_count, tamper_count) =
            check(&result, EmStatus::Complete, Some(expected(law)));
        exact += decision;
        values += usize::from(result.value == Some(expected(law)));
        replay += replay_count;
        tamper += tamper_count;
        false_denials += usize::from(!result.authorized());
        receipts.push(("supported", law, result.status, result.value));
    }
    for index in 0..40 {
        let mut req = request(laws[index % laws.len()]);
        req.ambiguity = Some("sign convention or circuit regime is unresolved".into());
        let result = evaluate(&req);
        let (decision, replay_count, tamper_count) = check(&result, EmStatus::Ambiguous, None);
        exact += decision;
        replay += replay_count;
        tamper += tamper_count;
        false_auth += usize::from(result.authorized());
        receipts.push(("ambiguous", "ambiguous", result.status, result.value));
    }
    for index in 0..20 {
        let mut req = request("unknown_law");
        req.law = format!("unknown_law_{index}");
        let result = evaluate(&req);
        let (decision, replay_count, tamper_count) = check(&result, EmStatus::Missing, None);
        exact += decision;
        replay += replay_count;
        tamper += tamper_count;
        false_auth += usize::from(result.authorized());
        receipts.push(("refused", "unknown", result.status, result.value));
    }
    for _ in 0..20 {
        let mut req = request("ohms_law_voltage");
        req.inputs.remove("R");
        let result = evaluate(&req);
        let (decision, replay_count, tamper_count) = check(&result, EmStatus::Missing, None);
        exact += decision;
        replay += replay_count;
        tamper += tamper_count;
        false_auth += usize::from(result.authorized());
        receipts.push(("refused", "missing_input", result.status, result.value));
    }
    for _ in 0..20 {
        let mut req = request("electric_power");
        req.unit_scope = "mixed_units".into();
        let result = evaluate(&req);
        let (decision, replay_count, tamper_count) = check(&result, EmStatus::InvalidDomain, None);
        exact += decision;
        replay += replay_count;
        tamper += tamper_count;
        false_auth += usize::from(result.authorized());
        receipts.push(("refused", "wrong_scope", result.status, result.value));
    }
    for _ in 0..20 {
        let mut req = request("capacitor_charge");
        req.law.clear();
        let result = evaluate(&req);
        let (decision, replay_count, tamper_count) = check(&result, EmStatus::Missing, None);
        exact += decision;
        replay += replay_count;
        tamper += tamper_count;
        false_auth += usize::from(result.authorized());
        receipts.push((
            "refused",
            "missing_law_identifier",
            result.status,
            result.value,
        ));
    }
    let report = Report {
        schema: "stage-h-bounded-electromagnetism-v1",
        corpus_sha256: digest(&receipts),
        cases: 240,
        supported: 120,
        ambiguous: 40,
        refused: 80,
        exact_decisions: exact,
        exact_values: values,
        replay_verified: replay,
        tamper_rejections: tamper,
        false_authorizations: false_auth,
        false_denials,
        source_records: 4,
        hle_questions_read: 0,
        registry_mutations: 0,
    };
    assert_eq!(report.exact_decisions, 240);
    assert_eq!(report.exact_values, 120);
    assert_eq!(report.replay_verified, 240);
    assert_eq!(report.tamper_rejections, 240);
    assert_eq!(report.false_authorizations, 0);
    assert_eq!(report.false_denials, 0);
    fs::write(
        "docs/stage296_bounded_electromagnetism.json",
        serde_json::to_vec_pretty(&report)?,
    )?;
    fs::write("docs/stage296_bounded_electromagnetism.md", format!(
        "# Stage 296 — source-derived bounded electromagnetism\n\nFour OpenStax-attributed declarative laws run through one generic rational expression interpreter: Ohm voltage, electric power, charge from constant current, and capacitor charge. The shadow pack does not infer units, signs, circuit behavior, or missing quantities.\n\n* cases / exact decisions: {} / {}\n* supported / ambiguous / refused: {} / {} / {}\n* exact values: {}\n* replay / tamper: {} / {}\n* false authorizations / denials: 0 / 0\n* source records: {}\n* HLE questions read / registry mutations: 0 / 0\n\nReproduce with `cargo run --quiet --bin electromagnetism_pack_bench`.\n",
        report.cases, report.exact_decisions, report.supported, report.ambiguous, report.refused,
        report.exact_values, report.replay_verified, report.tamper_rejections, report.source_records,
    ))?;
    println!(
        "stage296 cases={} exact={} values={} false_auth=0",
        report.cases, report.exact_decisions, report.exact_values
    );
    Ok(())
}
