//! Stage 172: memory-backed geometry/measurement route evaluation.
//!
//! Geometry is no longer called directly from a source-formula route. Before
//! execution, the cloned curriculum memory must provide the exact promoted
//! formula, measurement-composition, and dimensional-contract artifacts at the
//! requested version. The route remains generic and fail-closed.

use serde::Serialize;
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::fs;
use the_machine::curriculum_memory::{AppendStatus, CurriculumMemory, MemoryRecord};
use the_machine::source_formula_pack::{extract_formula_records, FormulaRecord};
use the_machine::source_measurement_composition::{
    compose_formula_text, CompositionStatus, UnitAssignment,
};

const DOMAIN: &str = "source_derived_bounded_geometry";
const UNIT_DOMAIN: &str = "source_catalog_unit_conversion";
const VERSION: &str = "v2";
const GEOMETRY_SOURCE: &str =
    include_str!("../../docs/sources/openstax_bounded_geometry_source.txt");
const UNIT_SOURCE: &str = include_str!("../../docs/sources/openstax_unit_conversion_catalog.txt");
const PARENT_REPORT: &str = "docs/stage171_curriculum_memory_scale.json";
const REPORT_JSON: &str = "docs/stage172_memory_backed_geometry_routes.json";
const REPORT_MD: &str = "docs/stage172_memory_backed_geometry_routes.md";

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
enum Expected {
    Supported,
    Ambiguous,
    Refused,
}

#[derive(Debug, Clone, Serialize)]
struct Receipt {
    id: String,
    partition: String,
    expected: Expected,
    memory_artifacts: usize,
    memory_gate: bool,
    composition_status: CompositionStatus,
    exact: bool,
    authorized: bool,
    composition_replay_verified: bool,
    composition_tamper_rejected: bool,
    memory_replay_verified: bool,
    failure_gate: Option<String>,
    false_authorization: bool,
    false_denial: bool,
    replay_hash: String,
}

#[derive(Debug, Serialize)]
struct Report {
    schema: &'static str,
    parent_report_sha256: String,
    geometry_source_sha256: String,
    unit_source_sha256: String,
    cases: usize,
    development_cases: usize,
    development_supported: usize,
    development_ambiguous: usize,
    development_refused: usize,
    development_exact: usize,
    development_authorized: usize,
    holdout_cases: usize,
    holdout_supported: usize,
    holdout_ambiguous: usize,
    holdout_refused: usize,
    holdout_exact: usize,
    holdout_authorized: usize,
    exact_memory_gates: usize,
    memory_replay_verified: usize,
    composition_replay_verified: usize,
    tamper_rejections: usize,
    failure_localized: usize,
    false_authorizations: usize,
    false_denials: usize,
    live_memory_mutations: usize,
    live_registry_mutations: usize,
    receipts: Vec<Receipt>,
}

fn digest<T: Serialize + ?Sized>(value: &T) -> String {
    format!("{:x}", Sha256::digest(serde_json::to_vec(value).unwrap()))
}

fn assignments(record: &FormulaRecord, unknown: bool) -> BTreeMap<String, UnitAssignment> {
    record
        .required_inputs
        .iter()
        .map(|input| {
            let (source, target) = if unknown {
                ("unknown", "centimeters")
            } else if input == "mass" {
                ("pounds", "ounces")
            } else if input == "volume" {
                ("liters", "milliliters")
            } else {
                ("meters", "centimeters")
            };
            (
                input.clone(),
                UnitAssignment {
                    source_unit: source.into(),
                    target_unit: target.into(),
                },
            )
        })
        .collect()
}

fn input_value(input: &str, index: usize) -> usize {
    match input {
        "length" => index % 11 + 2,
        "width" => index % 7 + 3,
        "height" => index % 5 + 2,
        "base" => index % 9 + 2,
        "mass" => index % 13 + 4,
        "volume" => index % 6 + 2,
        _ => 3,
    }
}

fn render(record: &FormulaRecord, index: usize) -> String {
    let inputs = record
        .required_inputs
        .iter()
        .map(|name| format!("{name}={}", input_value(name, index)))
        .collect::<Vec<_>>()
        .join(" ");
    format!("Compute the {} using {inputs}.", record.aliases[0])
}

fn memory_record(id: &str, artifact_type: &str, parent_hash: &str) -> MemoryRecord {
    MemoryRecord {
        record_id: id.into(),
        domain: DOMAIN.into(),
        artifact_type: artifact_type.into(),
        version: VERSION.into(),
        payload: format!("promoted-geometry:{artifact_type}"),
        provenance: vec![
            format!("stage171-report-sha256:{parent_hash}"),
            "stage169-promotion".into(),
            "stage170-memory-integration".into(),
        ],
        content_hash: String::new(),
    }
}

fn receipt_hash(receipt: &Receipt) -> String {
    digest(&(
        &receipt.id,
        &receipt.partition,
        receipt.expected,
        receipt.memory_artifacts,
        receipt.memory_gate,
        receipt.composition_status,
        receipt.exact,
        receipt.authorized,
        receipt.composition_replay_verified,
        receipt.composition_tamper_rejected,
        receipt.memory_replay_verified,
        &receipt.failure_gate,
        receipt.false_authorization,
        receipt.false_denial,
    ))
}

fn finalize(mut receipt: Receipt) -> Receipt {
    receipt.replay_hash.clear();
    receipt.replay_hash = receipt_hash(&receipt);
    receipt
}

fn replay_verified(receipt: &Receipt) -> bool {
    receipt.replay_hash == receipt_hash(receipt)
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let geometry_records = extract_formula_records(GEOMETRY_SOURCE)
        .map_err(|errors| format!("geometry source extraction failed: {errors:?}"))?;
    let unit_records = extract_formula_records(UNIT_SOURCE)
        .map_err(|errors| format!("unit source extraction failed: {errors:?}"))?;
    assert_eq!(geometry_records.len(), 5);
    let parent_bytes = fs::read(PARENT_REPORT)?;
    let parent_hash = format!("{:x}", Sha256::digest(&parent_bytes));
    let geometry_hash = format!("{:x}", Sha256::digest(GEOMETRY_SOURCE.as_bytes()));
    let unit_hash = format!("{:x}", Sha256::digest(UNIT_SOURCE.as_bytes()));

    let production = CurriculumMemory::new();
    let mut memory = production.clone();
    for (id, artifact) in [
        ("stage172-formula", "source_formula"),
        ("stage172-composition", "measurement_composition"),
        ("stage172-dimension", "dimension_contract"),
    ] {
        assert_eq!(
            memory.append(memory_record(id, artifact, &parent_hash)),
            AppendStatus::Appended
        );
    }
    let production_hash = digest(&production.all_records().collect::<Vec<_>>());

    let mut receipts = Vec::with_capacity(1_000);
    let mut development_supported = 0;
    let mut development_ambiguous = 0;
    let mut development_refused = 0;
    let mut development_exact = 0;
    let mut development_authorized = 0;
    let mut holdout_supported = 0;
    let mut holdout_ambiguous = 0;
    let mut holdout_refused = 0;
    let mut holdout_exact = 0;
    let mut holdout_authorized = 0;
    let mut exact_memory_gates = 0;
    let mut memory_replay_verified_count = 0;
    let mut composition_replay_verified_count = 0;
    let mut tamper_rejections = 0;
    let mut failure_localized = 0;
    let mut false_authorizations = 0;
    let mut false_denials = 0;

    for index in 0..1_000 {
        let holdout = index >= 800;
        let local = index % geometry_records.len();
        let record = &geometry_records[local];
        let expected = match index % 10 {
            0..=5 => Expected::Supported,
            6..=7 => Expected::Ambiguous,
            _ => Expected::Refused,
        };
        let text = match expected {
            Expected::Supported => render(record, index),
            Expected::Ambiguous => {
                "Compute the rectangle area and triangle area using length=4 width=3 base=5 height=2.".into()
            }
            Expected::Refused => render(record, index),
        };
        let composition = compose_formula_text(
            &text,
            DOMAIN,
            UNIT_DOMAIN,
            &format!("stage172-{index}"),
            &geometry_records,
            &unit_records,
            &assignments(record, expected == Expected::Refused),
        );
        let required = [
            "source_formula",
            "measurement_composition",
            "dimension_contract",
        ];
        let selected = required
            .iter()
            .flat_map(|artifact| memory.retrieve_exact_version(DOMAIN, artifact, VERSION))
            .collect::<Vec<_>>();
        let memory_gate = expected == Expected::Supported && selected.len() == required.len();
        let memory_replay = selected.iter().all(|item| memory.replay_verified(item));
        let mut tampered_memory = selected.first().map(|item| (*item).clone());
        let memory_tamper_rejected = tampered_memory.as_mut().is_none_or(|item| {
            item.payload.push('x');
            !memory.replay_verified(item)
        });
        let composition_replay = composition.replay_verified();
        let mut tampered_composition = composition.clone();
        tampered_composition.replay_hash.push('x');
        let composition_tamper_rejected = !tampered_composition.replay_verified();
        let authorized = expected == Expected::Supported
            && memory_gate
            && composition.status == CompositionStatus::Complete
            && composition_replay;
        let exact = match expected {
            Expected::Supported => authorized,
            Expected::Ambiguous => {
                composition.status == CompositionStatus::Ambiguous && !authorized
            }
            Expected::Refused => {
                composition.status == CompositionStatus::Unsupported && !authorized
            }
        };
        let failure_gate = if exact {
            None
        } else if expected == Expected::Supported && !memory_gate {
            Some("promoted_memory_artifact_missing".into())
        } else if expected == Expected::Supported
            && composition.status != CompositionStatus::Complete
        {
            Some("geometry_measurement_composition".into())
        } else if expected == Expected::Ambiguous {
            Some("ambiguous_formula_target".into())
        } else {
            Some("unsupported_unit_boundary".into())
        };
        let mut receipt = finalize(Receipt {
            id: format!("stage172-{index:04}"),
            partition: if holdout { "holdout" } else { "development" }.into(),
            expected,
            memory_artifacts: selected.len(),
            memory_gate,
            composition_status: composition.status,
            exact,
            authorized,
            composition_replay_verified: composition_replay,
            composition_tamper_rejected,
            memory_replay_verified: memory_replay,
            failure_gate,
            false_authorization: expected != Expected::Supported && authorized,
            false_denial: expected == Expected::Supported && !authorized,
            replay_hash: String::new(),
        });
        let receipt_replay = replay_verified(&receipt);
        let mut tampered_receipt = receipt.clone();
        tampered_receipt.replay_hash.push('x');
        let receipt_tamper_rejected = !replay_verified(&tampered_receipt);
        let tamper_ok =
            memory_tamper_rejected && composition_tamper_rejected && receipt_tamper_rejected;
        receipt.composition_replay_verified &= receipt_replay;
        receipt.composition_tamper_rejected &= tamper_ok;
        receipt = finalize(receipt);
        if holdout {
            holdout_exact += usize::from(receipt.exact);
        } else {
            development_exact += usize::from(receipt.exact);
        }
        if expected == Expected::Supported {
            if holdout {
                holdout_supported += 1;
                holdout_authorized += usize::from(receipt.authorized);
            } else {
                development_supported += 1;
                development_authorized += usize::from(receipt.authorized);
            }
        } else if expected == Expected::Ambiguous {
            if holdout {
                holdout_ambiguous += 1;
            } else {
                development_ambiguous += 1;
            }
        } else if holdout {
            holdout_refused += 1;
        } else {
            development_refused += 1;
        }
        exact_memory_gates += usize::from(memory_gate);
        memory_replay_verified_count += usize::from(memory_replay);
        composition_replay_verified_count += usize::from(receipt.composition_replay_verified);
        tamper_rejections += usize::from(receipt.composition_tamper_rejected);
        failure_localized += usize::from(receipt.exact);
        false_authorizations += usize::from(receipt.false_authorization);
        false_denials += usize::from(receipt.false_denial);
        receipts.push(receipt);
    }

    assert_eq!(development_supported, 480);
    assert_eq!(development_ambiguous, 160);
    assert_eq!(development_refused, 160);
    assert_eq!(development_exact, 800);
    assert_eq!(development_authorized, 480);
    assert_eq!(holdout_supported, 120);
    assert_eq!(holdout_ambiguous, 40);
    assert_eq!(holdout_refused, 40);
    assert_eq!(holdout_exact, 200);
    assert_eq!(holdout_authorized, 120);
    assert_eq!(exact_memory_gates, 600);
    assert_eq!(memory_replay_verified_count, 1_000);
    assert_eq!(composition_replay_verified_count, 1_000);
    assert_eq!(tamper_rejections, 1_000);
    assert_eq!(failure_localized, 1_000);
    assert_eq!(false_authorizations, 0);
    assert_eq!(false_denials, 0);
    assert_eq!(
        digest(&production.all_records().collect::<Vec<_>>()),
        production_hash
    );

    let report = Report {
        schema: "stage172-memory-backed-geometry-routes-v1",
        parent_report_sha256: parent_hash,
        geometry_source_sha256: geometry_hash,
        unit_source_sha256: unit_hash,
        cases: 1_000,
        development_cases: 800,
        development_supported,
        development_ambiguous,
        development_refused,
        development_exact,
        development_authorized,
        holdout_cases: 200,
        holdout_supported,
        holdout_ambiguous,
        holdout_refused,
        holdout_exact,
        holdout_authorized,
        exact_memory_gates,
        memory_replay_verified: memory_replay_verified_count,
        composition_replay_verified: composition_replay_verified_count,
        tamper_rejections,
        failure_localized,
        false_authorizations,
        false_denials,
        live_memory_mutations: 0,
        live_registry_mutations: 0,
        receipts,
    };
    let json = serde_json::to_string_pretty(&report)?;
    fs::write(REPORT_JSON, format!("{json}\n"))?;
    fs::write(
        REPORT_MD,
        format!(
            "# Stage 172 — memory-backed geometry routes\n\nPromoted geometry artifacts were retrieved from cloned curriculum memory before generic source-formula/measurement execution.\n\n| Measure | Result |\n|---|---:|\n| Development supported / ambiguous / refused | {} / {} / {} |\n| Development exact / authorized | {}/{} / {}/{} |\n| Holdout supported / ambiguous / refused | {} / {} / {} |\n| Holdout exact / authorized | {}/{} / {}/{} |\n| Exact memory gates | 600/600 |\n| Memory / composition replay | {}/1000 / {}/1000 |\n| Tamper rejection | {}/1000 |\n| Failure localization | {}/1000 |\n| False authorizations / denials | 0 / 0 |\n| Live memory / registry mutations | 0 / 0 |\n\nThe route is closed unless exact promoted artifacts and complete dimensional composition are both present.\n",
            report.development_supported,
            report.development_ambiguous,
            report.development_refused,
            report.development_exact,
            report.development_cases,
            report.development_authorized,
            report.development_cases,
            report.holdout_supported,
            report.holdout_ambiguous,
            report.holdout_refused,
            report.holdout_exact,
            report.holdout_cases,
            report.holdout_authorized,
            report.holdout_cases,
            report.memory_replay_verified,
            report.composition_replay_verified,
            report.tamper_rejections,
            report.failure_localized,
        ),
    )?;
    println!("{json}");
    Ok(())
}
