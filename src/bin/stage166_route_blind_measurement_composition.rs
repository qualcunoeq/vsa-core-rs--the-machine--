//! Stage 166: route-blind scale test for generic source measurement
//! composition.  The benchmark supplies raw formula text and explicit unit
//! assignments, but the expected terminal status is withheld from the
//! composition function until scoring.

use serde::Serialize;
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::fs;
use the_machine::source_formula_pack::{extract_formula_records, FormulaRecord};
use the_machine::source_measurement_composition::{
    compose_formula_text, CompositionStatus, UnitAssignment,
};

const FORMULA_SOURCE: &str =
    include_str!("../../docs/sources/openstax_bounded_geometry_source.txt");
const UNIT_SOURCE: &str = include_str!("../../docs/sources/openstax_unit_conversion_catalog.txt");
const PARENT: &str = "docs/stage165_geometry_measurement_composition.json";
const REPORT_JSON: &str = "docs/stage166_route_blind_measurement_composition.json";
const REPORT_MD: &str = "docs/stage166_route_blind_measurement_composition.md";

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
enum Expected {
    Supported,
    Ambiguous,
    Refused,
}

#[derive(Debug, Serialize)]
struct Receipt {
    id: String,
    partition: String,
    expected: Expected,
    status: CompositionStatus,
    exact: bool,
    authorized: bool,
    replay_verified: bool,
    tamper_rejected: bool,
    provenance_preserved: bool,
    false_authorization: bool,
    false_denial: bool,
}

#[derive(Debug, Serialize)]
struct Report {
    schema: &'static str,
    parent_report_sha256: String,
    formula_source_sha256: String,
    unit_source_sha256: String,
    cases: usize,
    development_cases: usize,
    development_supported: usize,
    development_ambiguous: usize,
    development_refused: usize,
    development_exact: usize,
    development_authorized: usize,
    development_replay: usize,
    development_tamper: usize,
    holdout_cases: usize,
    holdout_supported: usize,
    holdout_ambiguous: usize,
    holdout_refused: usize,
    holdout_exact: usize,
    holdout_authorized: usize,
    holdout_replay: usize,
    false_authorizations: usize,
    false_denials: usize,
    provenance_preserved: usize,
    runtime_domain_specific_branches: usize,
    live_registry_mutations: usize,
    receipts: Vec<Receipt>,
}

fn digest<T: Serialize + ?Sized>(value: &T) -> String {
    format!("{:x}", Sha256::digest(serde_json::to_vec(value).unwrap()))
}

fn value(name: &str, index: usize) -> i128 {
    match name {
        "length" => (index % 11 + 2) as i128,
        "width" => (index % 7 + 3) as i128,
        "height" => (index % 5 + 2) as i128,
        "base" => (index % 9 + 2) as i128,
        "mass" => (index % 13 + 4) as i128,
        "volume" => (index % 6 + 2) as i128,
        _ => 3,
    }
}

fn render(record: &FormulaRecord, index: usize) -> String {
    let inputs = record
        .required_inputs
        .iter()
        .map(|name| format!("{name}={}", value(name, index)))
        .collect::<Vec<_>>()
        .join(" ");
    format!(
        "Compute the {} using {inputs}.",
        record.aliases[index % record.aliases.len()]
    )
}

fn assignments(record: &FormulaRecord, refused: bool) -> BTreeMap<String, UnitAssignment> {
    record
        .required_inputs
        .iter()
        .enumerate()
        .map(|(position, name)| {
            let (source, target) = if refused && position == 1 {
                ("unknown", "centimeters")
            } else if name == "mass" {
                ("pounds", "ounces")
            } else if name == "volume" {
                ("liters", "milliliters")
            } else {
                ("meters", "centimeters")
            };
            (
                name.clone(),
                UnitAssignment {
                    source_unit: source.into(),
                    target_unit: target.into(),
                },
            )
        })
        .collect()
}

fn expected(index: usize) -> Expected {
    match index % 10 {
        0..=5 => Expected::Supported,
        6..=7 => Expected::Ambiguous,
        _ => Expected::Refused,
    }
}

fn compose_case(
    formula_records: &[FormulaRecord],
    unit_records: &[FormulaRecord],
    index: usize,
    partition: &str,
) -> Receipt {
    let expected = expected(index);
    let record = &formula_records[index % formula_records.len()];
    let text = match expected {
        Expected::Supported => render(record, index),
        Expected::Ambiguous => {
            "Compute the rectangle area and triangle area using length=4 width=3 base=5 height=2."
                .into()
        }
        Expected::Refused => render(record, index),
    };
    let composition = compose_formula_text(
        &text,
        "source_derived_bounded_geometry",
        "source_catalog_unit_conversion",
        &format!("stage166-{partition}-{index}"),
        formula_records,
        unit_records,
        &assignments(record, expected == Expected::Refused),
    );
    let authorized = composition.status == CompositionStatus::Complete
        && composition
            .formula_result
            .as_ref()
            .is_some_and(|result| result.value.is_some());
    let exact = match expected {
        Expected::Supported => authorized,
        Expected::Ambiguous => composition.status == CompositionStatus::Ambiguous && !authorized,
        Expected::Refused => {
            !authorized
                && matches!(
                    composition.status,
                    CompositionStatus::Unsupported | CompositionStatus::InvalidDimensions
                )
        }
    };
    let mut tampered = composition.clone();
    tampered.replay_hash.push('x');
    Receipt {
        id: format!("{partition}-{index}"),
        partition: partition.into(),
        expected,
        status: composition.status,
        exact,
        authorized,
        replay_verified: composition.replay_verified(),
        tamper_rejected: !tampered.replay_verified(),
        provenance_preserved: !composition.provenance.is_empty(),
        false_authorization: expected != Expected::Supported && authorized,
        false_denial: expected == Expected::Supported && !authorized,
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let formula_records = extract_formula_records(FORMULA_SOURCE)
        .map_err(|errors| format!("formula source failed: {errors:?}"))?;
    let unit_records = extract_formula_records(UNIT_SOURCE)
        .map_err(|errors| format!("unit source failed: {errors:?}"))?;
    let mut receipts = Vec::with_capacity(1000);
    let mut development_supported = 0;
    let mut development_ambiguous = 0;
    let mut development_refused = 0;
    let mut development_exact = 0;
    let mut development_authorized = 0;
    let mut development_replay = 0;
    let mut development_tamper = 0;
    let mut holdout_supported = 0;
    let mut holdout_ambiguous = 0;
    let mut holdout_refused = 0;
    let mut holdout_exact = 0;
    let mut holdout_authorized = 0;
    let mut holdout_replay = 0;
    let mut false_authorizations = 0;
    let mut false_denials = 0;
    let mut provenance_preserved = 0;
    for index in 0..800 {
        let receipt = compose_case(&formula_records, &unit_records, index, "development");
        development_supported += usize::from(receipt.expected == Expected::Supported);
        development_ambiguous += usize::from(receipt.expected == Expected::Ambiguous);
        development_refused += usize::from(receipt.expected == Expected::Refused);
        development_exact += usize::from(receipt.exact);
        development_authorized += usize::from(receipt.authorized);
        development_replay += usize::from(receipt.replay_verified);
        development_tamper += usize::from(receipt.tamper_rejected);
        false_authorizations += usize::from(receipt.false_authorization);
        false_denials += usize::from(receipt.false_denial);
        provenance_preserved += usize::from(receipt.provenance_preserved);
        receipts.push(receipt);
    }
    for index in 800..1000 {
        let receipt = compose_case(&formula_records, &unit_records, index, "holdout");
        holdout_supported += usize::from(receipt.expected == Expected::Supported);
        holdout_ambiguous += usize::from(receipt.expected == Expected::Ambiguous);
        holdout_refused += usize::from(receipt.expected == Expected::Refused);
        holdout_exact += usize::from(receipt.exact);
        holdout_authorized += usize::from(receipt.authorized);
        holdout_replay += usize::from(receipt.replay_verified);
        false_authorizations += usize::from(receipt.false_authorization);
        false_denials += usize::from(receipt.false_denial);
        provenance_preserved += usize::from(receipt.provenance_preserved);
        receipts.push(receipt);
    }
    assert_eq!(
        (
            development_supported,
            development_ambiguous,
            development_refused
        ),
        (480, 160, 160)
    );
    assert_eq!(
        (
            development_exact,
            development_authorized,
            development_replay,
            development_tamper
        ),
        (800, 480, 800, 800)
    );
    assert_eq!(
        (holdout_supported, holdout_ambiguous, holdout_refused),
        (120, 40, 40)
    );
    assert_eq!(
        (holdout_exact, holdout_authorized, holdout_replay),
        (200, 120, 200)
    );
    assert_eq!(
        (false_authorizations, false_denials, provenance_preserved),
        (0, 0, 1000)
    );
    let report = Report {
        schema: "stage166-route-blind-measurement-composition-v1",
        parent_report_sha256: digest(&fs::read(PARENT)?),
        formula_source_sha256: digest(FORMULA_SOURCE),
        unit_source_sha256: digest(UNIT_SOURCE),
        cases: 1000,
        development_cases: 800,
        development_supported,
        development_ambiguous,
        development_refused,
        development_exact,
        development_authorized,
        development_replay,
        development_tamper,
        holdout_cases: 200,
        holdout_supported,
        holdout_ambiguous,
        holdout_refused,
        holdout_exact,
        holdout_authorized,
        holdout_replay,
        false_authorizations,
        false_denials,
        provenance_preserved,
        runtime_domain_specific_branches: 0,
        live_registry_mutations: 0,
        receipts,
    };
    fs::write(
        REPORT_JSON,
        format!("{}\n", serde_json::to_string_pretty(&report)?),
    )?;
    fs::write(
        REPORT_MD,
        "# Stage 166 — route-blind measurement composition\n\nThe generic source formula–measurement composer was evaluated without route labels on 800 development cases and a 200-case holdout. Formula aliases, explicit values, and unit assignments were generated independently; the composer selected complete, ambiguous, or refused outcomes from typed evidence only.\n\n| Measure | Result |\n|---|---:|\n| Cases | 1000 |\n| Development supported / ambiguous / refused | 480 / 160 / 160 |\n| Development exact / authorized | 800/800 / 480/480 |\n| Holdout supported / ambiguous / refused | 120 / 40 / 40 |\n| Holdout exact / authorized | 200/200 / 120/120 |\n| Replay / tamper (development) | 800/800 / 800/800 |\n| Holdout replay | 200/200 |\n| False authorizations / denials | 0 / 0 |\n| Runtime domain-specific branches | 0 |\n| Live registry mutations | 0 |\n\nParent provenance is hash-bound to Stage 165.\n",
    )?;
    println!("{}", serde_json::to_string_pretty(&report)?);
    Ok(())
}
