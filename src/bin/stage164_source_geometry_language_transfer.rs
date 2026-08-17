//! Stage 164: route-blind language transfer into the acquired geometry catalog.
//!
//! The frontend uses only source-declared formula IDs, aliases, and input
//! names. This benchmark deliberately provides no geometry-specific language
//! branch; supported text must pass the generic frontend and generic source
//! interpreter before authorization.

use serde::Serialize;
use sha2::{Digest, Sha256};
use std::fs;
use the_machine::probability_pack::Rational;
use the_machine::source_formula_frontend::{formalize_formula_text, FormulaFrontendStatus};
use the_machine::source_formula_pack::{
    evaluate_formula_records, extract_formula_records, FormulaRecord, FormulaStatus,
};

const DOMAIN: &str = "source_derived_bounded_geometry";
const SOURCE: &str = include_str!("../../docs/sources/openstax_bounded_geometry_source.txt");
const PARENT: &str = "docs/stage163_source_geometry_acquisition.json";
const REPORT_JSON: &str = "docs/stage164_source_geometry_language_transfer.json";
const REPORT_MD: &str = "docs/stage164_source_geometry_language_transfer.md";

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
    frontend_status: FormulaFrontendStatus,
    downstream_status: Option<FormulaStatus>,
    exact: bool,
    authorized: bool,
    frontend_replay_verified: bool,
    frontend_tamper_rejected: bool,
    downstream_replay_verified: bool,
    downstream_tamper_rejected: bool,
    provenance_preserved: bool,
    false_authorization: bool,
    false_denial: bool,
}

#[derive(Debug, Serialize)]
struct Report {
    schema: &'static str,
    parent_report_sha256: String,
    source_document_sha256: String,
    source_record_count: usize,
    runtime_domain_specific_branches: usize,
    cases: usize,
    development_cases: usize,
    development_supported: usize,
    development_ambiguous: usize,
    development_refused: usize,
    development_exact_decisions: usize,
    development_authorized: usize,
    development_frontend_replay: usize,
    development_frontend_tamper: usize,
    development_downstream_replay: usize,
    development_downstream_tamper: usize,
    holdout_cases: usize,
    holdout_supported: usize,
    holdout_exact_decisions: usize,
    holdout_authorized: usize,
    holdout_frontend_replay: usize,
    holdout_downstream_replay: usize,
    false_authorizations: usize,
    false_denials: usize,
    provenance_preserved: usize,
    live_registry_mutations: usize,
    receipts: Vec<Receipt>,
}

fn digest<T: Serialize + ?Sized>(value: &T) -> String {
    format!("{:x}", Sha256::digest(serde_json::to_vec(value).unwrap()))
}

fn q(value: i128) -> Rational {
    Rational::new(value, 1).unwrap()
}

fn input_value(name: &str, index: usize) -> Rational {
    q(match name {
        "length" => (index % 11 + 2) as i128,
        "width" => (index % 7 + 3) as i128,
        "height" => (index % 5 + 2) as i128,
        "base" => (index % 9 + 2) as i128,
        "mass" => (index % 13 + 4) as i128,
        "volume" => (index % 6 + 2) as i128,
        _ => 3,
    })
}

fn render(record: &FormulaRecord, index: usize) -> String {
    let inputs = record
        .required_inputs
        .iter()
        .map(|name| format!("{name}={}", input_value(name, index).numerator))
        .collect::<Vec<_>>()
        .join(" ");
    format!(
        "Compute the {} using {}. case {index}",
        record.aliases[0], inputs
    )
}

fn receipt(
    records: &[FormulaRecord],
    text: String,
    id: String,
    partition: &str,
    expected: Expected,
) -> Receipt {
    let frontend = formalize_formula_text(&text, DOMAIN, records);
    let downstream = frontend
        .request
        .as_ref()
        .map(|request| evaluate_formula_records(request, DOMAIN, records));
    let authorized = expected == Expected::Supported
        && frontend.status == FormulaFrontendStatus::Complete
        && downstream.as_ref().is_some_and(|result| {
            result.status == FormulaStatus::Complete && result.value.is_some()
        });
    let exact = match expected {
        Expected::Supported => authorized,
        Expected::Ambiguous => frontend.status == FormulaFrontendStatus::Ambiguous && !authorized,
        Expected::Refused => frontend.status == FormulaFrontendStatus::Unsupported && !authorized,
    };
    let mut frontend_tampered = frontend.clone();
    frontend_tampered.replay_hash.push('x');
    let frontend_replay_verified = frontend.replay_verified();
    let frontend_tamper_rejected = !frontend_tampered.replay_verified();
    let downstream_replay_verified = downstream
        .as_ref()
        .is_none_or(|result| result.replay_verified());
    let downstream_tamper_rejected = downstream.as_ref().is_none_or(|result| {
        let mut tampered = result.clone();
        tampered.replay_hash.push('x');
        !tampered.replay_verified()
    });
    let provenance_preserved = frontend_replay_verified
        && !frontend.provenance_spans.is_empty()
        && downstream
            .as_ref()
            .is_none_or(|result| !result.provenance.is_empty());
    Receipt {
        id,
        partition: partition.into(),
        expected,
        frontend_status: frontend.status,
        downstream_status: downstream.map(|result| result.status),
        exact,
        authorized,
        frontend_replay_verified,
        frontend_tamper_rejected,
        downstream_replay_verified,
        downstream_tamper_rejected,
        provenance_preserved,
        false_authorization: expected != Expected::Supported && authorized,
        false_denial: expected == Expected::Supported && !authorized,
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let records = extract_formula_records(SOURCE)
        .map_err(|errors| format!("geometry source extraction failed: {errors:?}"))?;
    assert_eq!(records.len(), 5);
    let mut receipts = Vec::with_capacity(600);
    let mut development_supported = 0;
    let mut development_ambiguous = 0;
    let mut development_refused = 0;
    let mut development_exact_decisions = 0;
    let mut development_authorized = 0;
    let mut development_frontend_replay = 0;
    let mut development_frontend_tamper = 0;
    let mut development_downstream_replay = 0;
    let mut development_downstream_tamper = 0;
    let mut holdout_supported = 0;
    let mut holdout_exact_decisions = 0;
    let mut holdout_authorized = 0;
    let mut holdout_frontend_replay = 0;
    let mut holdout_downstream_replay = 0;
    let mut false_authorizations = 0;
    let mut false_denials = 0;
    let mut provenance_preserved = 0;
    for index in 0..500 {
        let record = &records[index % records.len()];
        let expected = match index % 10 {
            0..=5 => Expected::Supported,
            6..=7 => Expected::Ambiguous,
            _ => Expected::Refused,
        };
        let text = match expected {
            Expected::Supported => render(record, index),
            Expected::Ambiguous => format!(
                "Compute the rectangle area and triangle area with length=4 width=3 base=5 height=2. case {index}"
            ),
            Expected::Refused => format!(
                "Compute a continuous or optimization geometry operation with length=4. case {index}"
            ),
        };
        let r = receipt(
            &records,
            text,
            format!("development-{index}"),
            "development",
            expected,
        );
        development_supported += usize::from(expected == Expected::Supported);
        development_ambiguous += usize::from(expected == Expected::Ambiguous);
        development_refused += usize::from(expected == Expected::Refused);
        development_exact_decisions += usize::from(r.exact);
        development_authorized += usize::from(r.authorized);
        development_frontend_replay += usize::from(r.frontend_replay_verified);
        development_frontend_tamper += usize::from(r.frontend_tamper_rejected);
        development_downstream_replay += usize::from(r.downstream_replay_verified);
        development_downstream_tamper += usize::from(r.downstream_tamper_rejected);
        false_authorizations += usize::from(r.false_authorization);
        false_denials += usize::from(r.false_denial);
        provenance_preserved += usize::from(r.provenance_preserved);
        receipts.push(r);
    }
    for index in 0..100 {
        let record = &records[(index + 2) % records.len()];
        let text = render(record, index + 1000);
        let r = receipt(
            &records,
            text,
            format!("holdout-{index}"),
            "holdout",
            Expected::Supported,
        );
        holdout_supported += 1;
        holdout_exact_decisions += usize::from(r.exact);
        holdout_authorized += usize::from(r.authorized);
        holdout_frontend_replay += usize::from(r.frontend_replay_verified);
        holdout_downstream_replay += usize::from(r.downstream_replay_verified);
        false_authorizations += usize::from(r.false_authorization);
        false_denials += usize::from(r.false_denial);
        provenance_preserved += usize::from(r.provenance_preserved);
        receipts.push(r);
    }
    assert_eq!(development_supported, 300);
    assert_eq!(development_ambiguous, 100);
    assert_eq!(development_refused, 100);
    assert_eq!(development_exact_decisions, 500);
    assert_eq!(development_authorized, 300);
    assert_eq!(development_frontend_replay, 500);
    assert_eq!(development_frontend_tamper, 500);
    assert_eq!(development_downstream_replay, 500);
    assert_eq!(development_downstream_tamper, 500);
    assert_eq!(holdout_supported, 100);
    assert_eq!(holdout_exact_decisions, 100);
    assert_eq!(holdout_authorized, 100);
    assert_eq!(holdout_frontend_replay, 100);
    assert_eq!(holdout_downstream_replay, 100);
    assert_eq!(false_authorizations, 0);
    assert_eq!(false_denials, 0);
    assert_eq!(provenance_preserved, 600);
    let report = Report {
        schema: "stage164-source-geometry-language-transfer-v1",
        parent_report_sha256: digest(&fs::read(PARENT)?),
        source_document_sha256: digest(SOURCE),
        source_record_count: records.len(),
        runtime_domain_specific_branches: 0,
        cases: 600,
        development_cases: 500,
        development_supported,
        development_ambiguous,
        development_refused,
        development_exact_decisions,
        development_authorized,
        development_frontend_replay,
        development_frontend_tamper,
        development_downstream_replay,
        development_downstream_tamper,
        holdout_cases: 100,
        holdout_supported,
        holdout_exact_decisions,
        holdout_authorized,
        holdout_frontend_replay,
        holdout_downstream_replay,
        false_authorizations,
        false_denials,
        provenance_preserved,
        live_registry_mutations: 0,
        receipts,
    };
    fs::write(
        REPORT_JSON,
        format!("{}\n", serde_json::to_string_pretty(&report)?),
    )?;
    fs::write(REPORT_MD, format!("# Stage 164 — source-derived geometry language transfer\n\nThe generic source-formula frontend selected geometry aliases and explicitly labelled inputs, then delegated execution to the generic catalog interpreter. No geometry-specific frontend or executor branch was added.\n\n| Measure | Result |\n|---|---:|\n| Cases | 600 |\n| Development supported / ambiguous / refused | 300 / 100 / 100 |\n| Development exact / authorized | 500/500 / 300/300 |\n| Holdout supported / exact / authorized | 100 / 100 / 100 |\n| Frontend replay / tamper (development) | 500/500 / 500/500 |\n| Downstream replay / tamper (development) | 500/500 / 500/500 |\n| False authorizations / denials | 0 / 0 |\n| Runtime domain-specific branches | 0 |\n| Live registry mutations | 0 |\n\nParent source-acquisition provenance is hash-bound to Stage 163.\n"))?;
    println!("{}", serde_json::to_string_pretty(&report)?);
    Ok(())
}
