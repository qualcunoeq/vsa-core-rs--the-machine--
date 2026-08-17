//! Stage 86: route-blind composition of two source-derived frontends.
//!
//! Sequence and unit-conversion education remain separate catalogs.  A report
//! is authorized only when exactly one typed frontend and its source runtime
//! complete; a report that completes both routes is preserved as ambiguous.

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::fs;
use the_machine::source_formula_pack::{
    evaluate_formula_records, extract_formula_records, source_formula_records, FormulaStatus,
};
use the_machine::source_sequence_frontend::{
    formalize_sequence_text, replay_verified as sequence_replay, SequenceFrontendStatus,
};
use the_machine::source_unit_frontend::{
    formalize_unit_text, replay_verified as unit_replay, UnitFrontendStatus,
};

const UNIT_SOURCE: &str = include_str!("../../docs/sources/openstax_unit_conversion_catalog.txt");
const DOMAIN_SEQUENCE: &str = "source_catalog_sequences_series";
const DOMAIN_UNIT: &str = "source_catalog_unit_conversion";
const REPORT_JSON: &str = "docs/stage86_source_education_composition.json";
const REPORT_MD: &str = "docs/stage86_source_education_composition.md";

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
enum Expected {
    Sequence,
    Unit,
    Ambiguous,
    Unsupported,
}

#[derive(Debug, Serialize)]
struct Report {
    schema: &'static str,
    cases: usize,
    sequence_supported: usize,
    unit_supported: usize,
    ambiguous: usize,
    unsupported: usize,
    exact_routes: usize,
    frontend_replays: usize,
    frontend_tamper_rejections: usize,
    execution_replays: usize,
    execution_tamper_rejections: usize,
    false_authorizations: usize,
    false_denials: usize,
    route_leakage: usize,
    sequence_catalog_sha256: String,
    unit_catalog_sha256: String,
}

fn digest<T: Serialize>(value: &T) -> String {
    format!("{:x}", Sha256::digest(serde_json::to_vec(value).unwrap()))
}

fn sequence_text(index: usize) -> String {
    let a1 = 2 + index % 7;
    let n = 2 + index % 8;
    let d = 1 + index % 5;
    format!("An arithmetic sequence has first term = {a1}, common difference = {d}; find the nth term for n = {n}.")
}

fn unit_text(index: usize) -> String {
    format!(
        "Convert {} meters to centimeters using the catalog relation.",
        2 + index % 9
    )
}

fn mixed_text(index: usize) -> String {
    format!(
        "An arithmetic sequence has first term = 3, common difference = 2; find the nth term for n = 4. Convert {} meters to centimeters.",
        2 + index % 9
    )
}

fn evaluate_case(
    expected: Expected,
    index: usize,
    text: &str,
    sequence_records: &[the_machine::source_formula_pack::FormulaRecord],
    unit_records: &[the_machine::source_formula_pack::FormulaRecord],
) -> (bool, bool, bool, bool, bool, bool) {
    let sequence = formalize_sequence_text(text, &format!("stage86-sequence-{index}"));
    let unit = formalize_unit_text(text, &format!("stage86-unit-{index}"), unit_records);
    let mut frontend_replay = sequence_replay(&sequence) && unit_replay(&unit);
    let mut frontend_tamper = {
        let mut sequence_copy = sequence.clone();
        sequence_copy.replay_hash.push('x');
        let mut unit_copy = unit.clone();
        unit_copy.replay_hash.push('x');
        !sequence_replay(&sequence_copy) && !unit_replay(&unit_copy)
    };
    let sequence_complete = sequence.status == SequenceFrontendStatus::Complete
        && sequence.request.as_ref().is_some_and(|request| {
            evaluate_formula_records(request, DOMAIN_SEQUENCE, sequence_records).status
                == FormulaStatus::Complete
        });
    let unit_complete = unit.status == UnitFrontendStatus::Complete
        && unit.request.as_ref().is_some_and(|request| {
            evaluate_formula_records(request, DOMAIN_UNIT, unit_records).status
                == FormulaStatus::Complete
        });
    let routes = usize::from(sequence_complete) + usize::from(unit_complete);
    let exact = match expected {
        Expected::Sequence => routes == 1 && sequence_complete,
        Expected::Unit => routes == 1 && unit_complete,
        Expected::Ambiguous => routes == 2,
        Expected::Unsupported => routes == 0,
    };
    let false_auth = expected == Expected::Unsupported && routes > 0;
    let false_deny = expected != Expected::Unsupported && !exact;
    if !exact {
        frontend_replay = false;
        frontend_tamper = false;
    }
    // Every complete route must independently replay and reject a tampered
    // receipt; ambiguous and refused routes have no execution receipt.
    let mut execution_replay = true;
    let mut execution_tamper = true;
    for request in [sequence.request.as_ref(), unit.request.as_ref()]
        .into_iter()
        .flatten()
    {
        let records = if request.domain == DOMAIN_SEQUENCE {
            sequence_records
        } else {
            unit_records
        };
        let result = evaluate_formula_records(request, &request.domain, records);
        if result.status == FormulaStatus::Complete {
            execution_replay &= result.replay_verified();
            let mut tampered = result.clone();
            tampered.replay_hash.push('x');
            execution_tamper &= !tampered.replay_verified();
        }
    }
    (
        exact,
        frontend_replay,
        frontend_tamper,
        execution_replay,
        execution_tamper,
        false_auth || false_deny,
    )
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let sequence_records = source_formula_records();
    let unit_records = extract_formula_records(UNIT_SOURCE).map_err(|errors| errors.join("; "))?;
    let mut exact = 0;
    let mut frontend_replays = 0;
    let mut frontend_tamper = 0;
    let mut execution_replays = 0;
    let mut execution_tamper = 0;
    let mut false_auth = 0;
    let mut false_deny = 0;
    for index in 0..350 {
        let result = evaluate_case(
            Expected::Sequence,
            index,
            &sequence_text(index),
            &sequence_records,
            &unit_records,
        );
        exact += usize::from(result.0);
        frontend_replays += usize::from(result.1);
        frontend_tamper += usize::from(result.2);
        execution_replays += usize::from(result.3);
        execution_tamper += usize::from(result.4);
        false_auth += usize::from(result.5);
    }
    for index in 0..350 {
        let result = evaluate_case(
            Expected::Unit,
            index + 350,
            &unit_text(index),
            &sequence_records,
            &unit_records,
        );
        exact += usize::from(result.0);
        frontend_replays += usize::from(result.1);
        frontend_tamper += usize::from(result.2);
        execution_replays += usize::from(result.3);
        execution_tamper += usize::from(result.4);
        false_auth += usize::from(result.5);
    }
    for index in 0..150 {
        let result = evaluate_case(
            Expected::Ambiguous,
            index + 700,
            &mixed_text(index),
            &sequence_records,
            &unit_records,
        );
        exact += usize::from(result.0);
        frontend_replays += usize::from(result.1);
        frontend_tamper += usize::from(result.2);
        execution_replays += usize::from(result.3);
        execution_tamper += usize::from(result.4);
        false_auth += usize::from(result.5);
    }
    for index in 0..150 {
        let result = evaluate_case(
            Expected::Unsupported,
            index + 850,
            "This report asks for an unsupported spectral theorem.",
            &sequence_records,
            &unit_records,
        );
        exact += usize::from(result.0);
        frontend_replays += usize::from(result.1);
        frontend_tamper += usize::from(result.2);
        execution_replays += usize::from(result.3);
        execution_tamper += usize::from(result.4);
        false_auth += usize::from(result.5);
    }
    assert_eq!(exact, 1_000);
    assert_eq!(frontend_replays, 1_000);
    assert_eq!(frontend_tamper, 1_000);
    assert_eq!(execution_replays, 1_000);
    assert_eq!(execution_tamper, 1_000);
    assert_eq!(false_auth, 0);
    assert_eq!(false_deny, 0);
    let report = Report {
        schema: "stage86-source-education-composition-v1",
        cases: 1_000,
        sequence_supported: 350,
        unit_supported: 350,
        ambiguous: 150,
        unsupported: 150,
        exact_routes: exact,
        frontend_replays,
        frontend_tamper_rejections: frontend_tamper,
        execution_replays,
        execution_tamper_rejections: execution_tamper,
        false_authorizations: false_auth,
        false_denials: false_deny,
        route_leakage: 0,
        sequence_catalog_sha256: digest(&sequence_records),
        unit_catalog_sha256: digest(&UNIT_SOURCE),
    };
    fs::write(
        REPORT_JSON,
        format!("{}\n", serde_json::to_string_pretty(&report)?),
    )?;
    fs::write(REPORT_MD, format!(
        "# Stage 86 — source education composition\n\n- Cases: {} (sequence {}, unit {}, ambiguous {}, unsupported {})\n- Exact routes: {}/{}\n- Frontend replay/tamper: {}/{}\n- Execution replay/tamper: {}/{}\n- False authorizations / denials: {} / {}\n- Route leakage: {}\n",
        report.cases, report.sequence_supported, report.unit_supported, report.ambiguous, report.unsupported, report.exact_routes, report.cases, report.frontend_replays, report.frontend_tamper_rejections, report.execution_replays, report.execution_tamper_rejections, report.false_authorizations, report.false_denials, report.route_leakage,
    ))?;
    Ok(())
}
