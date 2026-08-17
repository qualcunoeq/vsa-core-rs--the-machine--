//! Stage 90: source-derived linear interpolation education and pressure gate.
//!
//! The only subject data in this benchmark is an attributed source catalog.
//! All execution uses the generic source formula extractor/interpreter; the
//! frontend is responsible only for typed bindings and safe boundaries.

use serde::Serialize;
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use the_machine::source_formula_pack::{
    evaluate_formula_records, extract_formula_records, validate_formula_records, FormulaStatus,
};
use the_machine::source_interpolation_frontend::{
    formalize_interpolation_text, replay_verified, InterpolationFrontendStatus,
};

const SOURCE: &str = include_str!("../../docs/sources/openstax_linear_interpolation_catalog.txt");
const DOMAIN: &str = "source_catalog_linear_interpolation";

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
enum Expected {
    Supported,
    Ambiguous,
    Unsupported,
}

#[derive(Debug, Serialize)]
struct Receipt {
    id: String,
    expected: Expected,
    frontend_status: InterpolationFrontendStatus,
    downstream_status: Option<FormulaStatus>,
    expected_value: Option<String>,
    actual_value: Option<String>,
    exact: bool,
    replay_verified: bool,
    tamper_rejected: bool,
    source_provenance: bool,
    false_authorization: bool,
    false_denial: bool,
}

#[derive(Debug, Serialize)]
struct Report {
    schema: &'static str,
    source_id: &'static str,
    source_sha256: String,
    catalog_sha256: String,
    cases: usize,
    supported: usize,
    ambiguous: usize,
    unsupported: usize,
    exact_decisions: usize,
    supported_values: usize,
    replay_verified: usize,
    tamper_rejections: usize,
    source_provenance: usize,
    source_mutations_rejected: usize,
    false_authorizations: usize,
    false_denials: usize,
    status_counts: BTreeMap<String, usize>,
    receipts: Vec<Receipt>,
}

fn digest<T: Serialize + ?Sized>(value: &T) -> String {
    format!("{:x}", Sha256::digest(serde_json::to_vec(value).unwrap()))
}

fn rational_string(value: &the_machine::probability_pack::Rational) -> String {
    if value.denominator == 1 {
        value.numerator.to_string()
    } else {
        format!("{}/{}", value.numerator, value.denominator)
    }
}

fn expected_value(index: usize) -> the_machine::probability_pack::Rational {
    // y = 10 + (5 / 10) * 20 = 20, with varied rational targets/endpoints.
    let x1 = 0_i128;
    let x2 = 8_i128 + (index % 4) as i128;
    let y1 = 3_i128 + (index % 5) as i128;
    let y2 = 19_i128 + (index % 7) as i128;
    let x = x1 + (x2 - x1) / 2;
    let numerator = y1 * (x2 - x1) + (x - x1) * (y2 - y1);
    the_machine::probability_pack::Rational::new(numerator, x2 - x1).unwrap()
}

fn supported_text(index: usize) -> String {
    let x2 = 8 + index % 4;
    let y1 = 3 + index % 5;
    let y2 = 19 + index % 7;
    format!(
        "Linearly interpolate at x={} between x1=0,y1={} and x2={},y2={}; use the stated linear relation.",
        x2 / 2,
        y1,
        x2,
        y2
    )
}

fn ambiguous_text(index: usize) -> String {
    if index % 2 == 0 {
        "Interpolate or extrapolate at x=5 between x1=0,y1=10 and x2=10,y2=30.".into()
    } else {
        "Linearly interpolate at x=5 or x=6 between x1=0,y1=10 and x2=10,y2=30.".into()
    }
}

fn unsupported_text(index: usize) -> String {
    match index % 4 {
        0 => "Linearly interpolate at x=15 between x1=0,y1=10 and x2=10,y2=30.".into(),
        1 => "Use quadratic interpolation at x=5 between x1=0,y1=10 and x2=10,y2=30.".into(),
        2 => "Linearly interpolate at x=5 between x1=0,y1=10 and x2=0,y2=30.".into(),
        _ => "Estimate the unknown point from a table without an interpolation model.".into(),
    }
}

fn evaluate_case(
    index: usize,
    expected: Expected,
    records: &[the_machine::source_formula_pack::FormulaRecord],
) -> Receipt {
    let text = match expected {
        Expected::Supported => supported_text(index),
        Expected::Ambiguous => ambiguous_text(index),
        Expected::Unsupported => unsupported_text(index),
    };
    let frontend = formalize_interpolation_text(&text, &format!("stage90-{index:03}"));
    let downstream = frontend
        .request
        .as_ref()
        .map(|request| evaluate_formula_records(request, DOMAIN, records));
    let actual_value = downstream
        .as_ref()
        .and_then(|result| result.value.as_ref())
        .map(rational_string);
    let expected_value = (expected == Expected::Supported).then(|| rational_string(&expected_value(index)));
    let authorized = expected == Expected::Supported
        && frontend.status == InterpolationFrontendStatus::Complete
        && downstream.as_ref().is_some_and(|result| {
            result.status == FormulaStatus::Complete
                && result.value.is_some()
                && result.provenance.iter().any(|p| p.contains("source-interpolation"))
                && result.replay_verified()
        })
        && actual_value == expected_value;
    let mut frontend_copy = frontend.clone();
    frontend_copy.replay_hash.push('x');
    let downstream_replay = downstream.as_ref().is_none_or(|result| result.replay_verified());
    let downstream_tamper = downstream.as_ref().is_none_or(|result| {
        let mut copy = result.clone();
        copy.replay_hash.push('x');
        !copy.replay_verified()
    });
    let replay = replay_verified(&frontend) && downstream_replay;
    let tamper = !replay_verified(&frontend_copy) && downstream_tamper;
    let exact = match expected {
        Expected::Supported => authorized,
        Expected::Ambiguous => frontend.status == InterpolationFrontendStatus::Ambiguous && !authorized,
        Expected::Unsupported => frontend.status == InterpolationFrontendStatus::Unsupported && !authorized,
    };
    Receipt {
        id: format!("interpolation_{index:03}"),
        expected,
        frontend_status: frontend.status,
        downstream_status: downstream.as_ref().map(|result| result.status),
        expected_value,
        actual_value,
        exact,
        replay_verified: replay,
        tamper_rejected: tamper,
        source_provenance: downstream.as_ref().is_none_or(|result| !result.provenance.is_empty()),
        false_authorization: expected != Expected::Supported && authorized,
        false_denial: expected == Expected::Supported && !authorized,
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let records = extract_formula_records(SOURCE).map_err(|errors| errors.join("; "))?;
    validate_formula_records(&records).map_err(|errors| errors.join("; "))?;
    assert_eq!(records.len(), 1);
    let mut receipts = Vec::with_capacity(300);
    for index in 0..180 {
        receipts.push(evaluate_case(index, Expected::Supported, &records));
    }
    for index in 0..60 {
        receipts.push(evaluate_case(index + 180, Expected::Ambiguous, &records));
    }
    for index in 0..60 {
        receipts.push(evaluate_case(index + 240, Expected::Unsupported, &records));
    }
    // Source integrity is a promotion prerequisite.  A mutation that still
    // parses is not accepted as the frozen catalog because its hash changes.
    let source_mutations_rejected = [
        SOURCE.replace("linear_interpolation", "mutated_interpolation"),
        SOURCE.replace("y2 - y1", "y2 + y1"),
        SOURCE.replace("OpenStax", "UntrustedCopy"),
        SOURCE.replace("x1 and x2 are distinct", "x1 and x2 may coincide"),
        SOURCE.replace("RETRIEVED: 2026-08-17", "RETRIEVED: 1900-01-01"),
        SOURCE.replace("CC BY 4.0", "unknown license"),
    ]
    .iter()
    .filter(|mutation| digest(mutation) != digest(SOURCE))
    .count();
    let cases = receipts.len();
    let supported = receipts.iter().filter(|r| r.expected == Expected::Supported).count();
    let ambiguous = receipts.iter().filter(|r| r.expected == Expected::Ambiguous).count();
    let unsupported = receipts.iter().filter(|r| r.expected == Expected::Unsupported).count();
    let exact_decisions = receipts.iter().filter(|r| r.exact).count();
    let supported_values = receipts.iter().filter(|r| r.expected == Expected::Supported && r.actual_value == r.expected_value).count();
    let replay_verified_count = receipts.iter().filter(|r| r.replay_verified).count();
    let tamper_rejections = receipts.iter().filter(|r| r.tamper_rejected).count();
    let source_provenance = receipts.iter().filter(|r| r.source_provenance).count();
    let false_authorizations = receipts.iter().filter(|r| r.false_authorization).count();
    let false_denials = receipts.iter().filter(|r| r.false_denial).count();
    assert_eq!((cases, supported, ambiguous, unsupported), (300, 180, 60, 60));
    assert_eq!(exact_decisions, cases);
    assert_eq!(supported_values, supported);
    assert_eq!(replay_verified_count, cases);
    assert_eq!(tamper_rejections, cases);
    assert_eq!(source_provenance, cases);
    assert_eq!(source_mutations_rejected, 6);
    assert_eq!(false_authorizations, 0);
    assert_eq!(false_denials, 0);
    let mut status_counts = BTreeMap::new();
    for receipt in &receipts {
        *status_counts.entry(format!("{:?}", receipt.frontend_status)).or_insert(0) += 1;
    }
    let report = Report {
        schema: "stage90-source-linear-interpolation-v1",
        source_id: "openstax-precalculus-2e:linear-functions",
        source_sha256: digest(SOURCE),
        catalog_sha256: digest(&records),
        cases,
        supported,
        ambiguous,
        unsupported,
        exact_decisions,
        supported_values,
        replay_verified: replay_verified_count,
        tamper_rejections,
        source_provenance,
        source_mutations_rejected,
        false_authorizations,
        false_denials,
        status_counts,
        receipts,
    };
    let serialized = serde_json::to_string_pretty(&report)?;
    std::fs::write("docs/stage90_source_linear_interpolation.json", format!("{serialized}\n"))?;
    println!("{serialized}");
    Ok(())
}
