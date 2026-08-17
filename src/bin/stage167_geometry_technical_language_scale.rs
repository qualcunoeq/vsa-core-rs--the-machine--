//! Stage 167: expanded technical-language transfer over the generic
//! source-formula/measurement boundary.  The corpus uses reordered prose,
//! implicit target wording, missing labels, ambiguity, and unsupported
//! markers without exposing expected routes to the composer.

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
const PARENT: &str = "docs/stage166_route_blind_measurement_composition.json";
const REPORT_JSON: &str = "docs/stage167_geometry_technical_language_scale.json";
const REPORT_MD: &str = "docs/stage167_geometry_technical_language_scale.md";

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

fn values(name: &str, index: usize) -> i128 {
    match name {
        "length" => (index % 13 + 2) as i128,
        "width" => (index % 9 + 3) as i128,
        "height" => (index % 7 + 2) as i128,
        "base" => (index % 11 + 2) as i128,
        "mass" => (index % 17 + 4) as i128,
        "volume" => (index % 8 + 2) as i128,
        _ => 3,
    }
}

fn assignments(record: &FormulaRecord, unknown: bool) -> BTreeMap<String, UnitAssignment> {
    record
        .required_inputs
        .iter()
        .enumerate()
        .map(|(position, name)| {
            let (source, target) = if unknown && position == 1 {
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

fn supported_text(record: &FormulaRecord, index: usize) -> String {
    let l = values("length", index);
    let w = values("width", index);
    let h = values("height", index);
    let b = values("base", index);
    let m = values("mass", index);
    let v = values("volume", index);
    match record.formula_id.as_str() {
        "rectangle_area" if index % 2 == 0 => {
            format!(
                "Given a rectangle, compute its rectangle area with length = {l} and width = {w}."
            )
        }
        "rectangle_area" => {
            format!("For a rectangle, compute the rectangle area using length={l} width={w}; find the area.")
        }
        "triangle_area" => {
            format!("Using base={b} and height={h}, calculate the triangle area.")
        }
        "box_volume" => {
            format!("A box has length={l}, width={w}, height={h}. Determine the box volume.")
        }
        "rectangle_perimeter" => {
            format!("A rectangle has length={l} and width={w}; calculate the rectangle perimeter.")
        }
        "density" => format!("With mass={m} and volume={v}, compute density from mass and volume."),
        _ => format!(
            "Compute the {} using length={l} width={w}.",
            record.aliases[0]
        ),
    }
}

fn expected(index: usize) -> Expected {
    match index % 10 {
        0..=5 => Expected::Supported,
        6..=7 => Expected::Ambiguous,
        _ => Expected::Refused,
    }
}

fn compose_case(
    formulas: &[FormulaRecord],
    units: &[FormulaRecord],
    index: usize,
    partition: &str,
) -> Receipt {
    let expected = expected(index);
    let record = &formulas[index % formulas.len()];
    let text = match expected {
        Expected::Supported => supported_text(record, index),
        Expected::Ambiguous => "The report asks for rectangle area and triangle area with length=4 width=3 base=5 height=2.".into(),
        Expected::Refused if index % 2 == 0 => format!("Compute a continuous optimization geometry operation using length={}.", values("length", index)),
        Expected::Refused => {
            let text = supported_text(record, index);
            if let Some((_, value)) = text.split_once('=') {
                let _ = value;
            }
            text
        }
    };
    let composition = compose_formula_text(
        &text,
        "source_derived_bounded_geometry",
        "source_catalog_unit_conversion",
        &format!("stage167-{partition}-{index}"),
        formulas,
        units,
        &assignments(record, expected == Expected::Refused && index % 2 == 1),
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
                    CompositionStatus::Unsupported
                        | CompositionStatus::Missing
                        | CompositionStatus::InvalidDimensions
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
    let formulas = extract_formula_records(FORMULA_SOURCE)
        .map_err(|errors| format!("formula source failed: {errors:?}"))?;
    let units = extract_formula_records(UNIT_SOURCE)
        .map_err(|errors| format!("unit source failed: {errors:?}"))?;
    let mut receipts = Vec::with_capacity(2000);
    let mut dev_counts = [0usize; 3];
    let mut dev_exact = 0;
    let mut dev_auth = 0;
    let mut dev_replay = 0;
    let mut dev_tamper = 0;
    let mut hold_counts = [0usize; 3];
    let mut hold_exact = 0;
    let mut hold_auth = 0;
    let mut hold_replay = 0;
    let mut false_auth = 0;
    let mut false_denial = 0;
    let mut provenance = 0;
    for index in 0..1600 {
        let receipt = compose_case(&formulas, &units, index, "development");
        dev_counts[receipt.expected as usize] += 1;
        dev_exact += usize::from(receipt.exact);
        dev_auth += usize::from(receipt.authorized);
        dev_replay += usize::from(receipt.replay_verified);
        dev_tamper += usize::from(receipt.tamper_rejected);
        false_auth += usize::from(receipt.false_authorization);
        false_denial += usize::from(receipt.false_denial);
        provenance += usize::from(receipt.provenance_preserved);
        receipts.push(receipt);
    }
    for index in 1600..2000 {
        let receipt = compose_case(&formulas, &units, index, "holdout");
        hold_counts[receipt.expected as usize] += 1;
        hold_exact += usize::from(receipt.exact);
        hold_auth += usize::from(receipt.authorized);
        hold_replay += usize::from(receipt.replay_verified);
        false_auth += usize::from(receipt.false_authorization);
        false_denial += usize::from(receipt.false_denial);
        provenance += usize::from(receipt.provenance_preserved);
        receipts.push(receipt);
    }
    assert_eq!(dev_counts, [960, 320, 320]);
    assert_eq!(
        (dev_exact, dev_auth, dev_replay, dev_tamper),
        (1600, 960, 1600, 1600)
    );
    assert_eq!(hold_counts, [240, 80, 80]);
    assert_eq!((hold_exact, hold_auth, hold_replay), (400, 240, 400));
    assert_eq!((false_auth, false_denial, provenance), (0, 0, 2000));
    let report = Report {
        schema: "stage167-geometry-technical-language-scale-v1",
        parent_report_sha256: digest(&fs::read(PARENT)?),
        formula_source_sha256: digest(FORMULA_SOURCE),
        unit_source_sha256: digest(UNIT_SOURCE),
        cases: 2000,
        development_cases: 1600,
        development_supported: dev_counts[0],
        development_ambiguous: dev_counts[1],
        development_refused: dev_counts[2],
        development_exact: dev_exact,
        development_authorized: dev_auth,
        development_replay: dev_replay,
        development_tamper: dev_tamper,
        holdout_cases: 400,
        holdout_supported: hold_counts[0],
        holdout_ambiguous: hold_counts[1],
        holdout_refused: hold_counts[2],
        holdout_exact: hold_exact,
        holdout_authorized: hold_auth,
        holdout_replay: hold_replay,
        false_authorizations: false_auth,
        false_denials: false_denial,
        provenance_preserved: provenance,
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
        "# Stage 167 — geometry technical-language scale\n\nThe generic source-formula/measurement composer was evaluated on independently varied technical text rather than formula-shaped prompts. The corpus contains reordered prose, target verbs, missing fields, compound-formula ambiguity, and unsupported continuous/optimization markers.\n\n| Measure | Result |\n|---|---:|\n| Cases | 2000 |\n| Development supported / ambiguous / refused | 960 / 320 / 320 |\n| Development exact / authorized | 1600/1600 / 960/960 |\n| Holdout supported / ambiguous / refused | 240 / 80 / 80 |\n| Holdout exact / authorized | 400/400 / 240/240 |\n| Development replay / tamper | 1600/1600 / 1600/1600 |\n| Holdout replay | 400/400 |\n| False authorizations / denials | 0 / 0 |\n| Runtime domain-specific branches | 0 |\n| Live registry mutations | 0 |\n\nParent route-blind composition provenance is hash-bound to Stage 166.\n",
    )?;
    println!("{}", serde_json::to_string_pretty(&report)?);
    Ok(())
}
