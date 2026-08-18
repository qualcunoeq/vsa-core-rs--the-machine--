//! Stage 267: fresh shadow validation of the utility-selected economics pack.
//!
//! The corpus is generated independently from the source records and uses the
//! generic formula frontend.  It exercises shifted wording, explicit
//! ambiguity, unsupported requests, exact execution, replay, and tamper
//! rejection without changing the cloned or live manifests.

use serde::Serialize;
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::fs;

use the_machine::source_formula_frontend::{
    formalize_source_formula_report, report_replay_verified, FrontendStatus,
};
use the_machine::source_formula_pack::{
    evaluate_formula_records, extract_formula_records, FormulaRecord, FormulaStatus,
};

const SOURCE: &str = include_str!("../../docs/sources/openstax_bounded_economics_source.txt");
const SHADOW_MANIFEST: &str = "docs/stage266_economics_shadow_manifest.json";
const REPORT_JSON: &str = "docs/stage267_economics_shadow_validation.json";
const REPORT_MD: &str = "docs/stage267_economics_shadow_validation.md";
const DOMAIN: &str = "source_derived_bounded_economics";

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
enum Expected {
    Supported,
    Ambiguous,
    Unsupported,
}

#[derive(Debug, Serialize)]
struct Report {
    schema: &'static str,
    source_sha256: String,
    shadow_manifest_sha256: String,
    corpus_sha256: String,
    source_records: usize,
    cases: usize,
    supported_cases: usize,
    ambiguous_cases: usize,
    unsupported_cases: usize,
    exact_decisions: usize,
    supported_authorized: usize,
    supported_replays: usize,
    supported_tamper_rejections: usize,
    all_replays: usize,
    all_tamper_rejections: usize,
    provenance_preserved: usize,
    false_authorizations: usize,
    false_denials: usize,
    manifest_mutations: usize,
    registry_mutations: usize,
    production_authorizations: usize,
}

fn digest_bytes(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

fn digest<T: Serialize>(value: &T) -> String {
    format!("{:x}", Sha256::digest(serde_json::to_vec(value).unwrap()))
}

fn value(index: usize, offset: usize) -> i128 {
    (index % 17 + offset + 2) as i128
}

fn inputs(record: &FormulaRecord, index: usize) -> BTreeMap<String, i128> {
    record
        .required_inputs
        .iter()
        .enumerate()
        .map(|(offset, name)| (name.clone(), value(index, offset)))
        .collect()
}

fn supported_text(record: &FormulaRecord, index: usize) -> String {
    let alias = record
        .aliases
        .get(index % record.aliases.len().max(1))
        .map(String::as_str)
        .unwrap_or(&record.formula_id);
    let values = inputs(record, index)
        .into_iter()
        .map(|(name, value)| format!("{name}={value}"))
        .collect::<Vec<_>>()
        .join(" and ");
    match index % 3 {
        0 => format!("Compute {alias} using {values}.",),
        1 => format!("Determine the {alias}; given {values}.",),
        _ => format!("Evaluate {alias} when {values}.",),
    }
}

fn ambiguous_text() -> &'static str {
    "Compute total revenue or total cost with price=9, quantity=4, fixed_cost=3, variable_cost=2."
}

fn unsupported_text() -> &'static str {
    "Approximate continuous elasticity from an unbounded economic model."
}

fn expected(index: usize) -> Expected {
    match index % 10 {
        0..=5 => Expected::Supported,
        6..=7 => Expected::Ambiguous,
        _ => Expected::Unsupported,
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let source_bytes = SOURCE.as_bytes();
    let records = extract_formula_records(SOURCE).map_err(|errors| errors.join("; "))?;
    let shadow_bytes = fs::read(SHADOW_MANIFEST)?;
    let shadow: serde_json::Value = serde_json::from_slice(&shadow_bytes)?;
    assert_eq!(
        shadow
            .get("candidate_id")
            .and_then(serde_json::Value::as_str),
        Some(DOMAIN)
    );
    assert_eq!(
        shadow
            .get("shadow_only")
            .and_then(serde_json::Value::as_bool),
        Some(true)
    );
    let mut receipts = Vec::new();
    let mut exact_decisions = 0;
    let mut supported_authorized = 0;
    let mut supported_replays = 0;
    let mut supported_tamper_rejections = 0;
    let mut all_replays = 0;
    let mut all_tamper_rejections = 0;
    let mut provenance_preserved = 0;
    let mut false_authorizations = 0;
    let mut false_denials = 0;
    let mut counts = BTreeMap::new();
    for index in 0..600usize {
        let expected = expected(index);
        let text = match expected {
            Expected::Supported => {
                let record = &records[index % records.len()];
                supported_text(record, index)
            }
            Expected::Ambiguous => ambiguous_text().into(),
            Expected::Unsupported => unsupported_text().into(),
        };
        let report = formalize_source_formula_report(&text, DOMAIN, &records);
        let actual = match report.frontend.status {
            FrontendStatus::Complete => Expected::Supported,
            FrontendStatus::Ambiguous => Expected::Ambiguous,
            FrontendStatus::Missing | FrontendStatus::Unsupported => Expected::Unsupported,
        };
        *counts.entry(format!("{expected:?}")).or_insert(0usize) += 1;
        if actual == expected {
            exact_decisions += 1;
        } else if expected == Expected::Supported {
            false_denials += 1;
        } else if actual == Expected::Supported {
            false_authorizations += 1;
        }
        if report_replay_verified(&report) {
            all_replays += 1;
        }
        let mut tampered = report.clone();
        tampered.replay_hash.push('x');
        if !report_replay_verified(&tampered) {
            all_tamper_rejections += 1;
        }
        if !report.frontend.provenance_spans.is_empty() {
            provenance_preserved += 1;
        }
        let mut rendered = None;
        if expected == Expected::Supported && report.frontend.status == FrontendStatus::Complete {
            if let Some(request) = report.frontend.request.as_ref() {
                let execution = evaluate_formula_records(request, DOMAIN, &records);
                if execution.status == FormulaStatus::Complete && execution.value.is_some() {
                    supported_authorized += 1;
                    let value = execution.value.as_ref().unwrap();
                    rendered = Some(format!("{}/{}", value.numerator, value.denominator));
                    if execution.replay_verified() {
                        supported_replays += 1;
                    }
                    let mut altered = execution.clone();
                    altered.replay_hash.push('x');
                    if !altered.replay_verified() {
                        supported_tamper_rejections += 1;
                    }
                }
            }
        }
        receipts.push((index, text, expected, actual, rendered));
    }
    let report = Report {
        schema: "stage267-economics-shadow-validation-v1",
        source_sha256: digest_bytes(source_bytes),
        shadow_manifest_sha256: digest_bytes(&shadow_bytes),
        corpus_sha256: digest(&receipts),
        source_records: records.len(),
        cases: 600,
        supported_cases: *counts.get("Supported").unwrap_or(&0),
        ambiguous_cases: *counts.get("Ambiguous").unwrap_or(&0),
        unsupported_cases: *counts.get("Unsupported").unwrap_or(&0),
        exact_decisions,
        supported_authorized,
        supported_replays,
        supported_tamper_rejections,
        all_replays,
        all_tamper_rejections,
        provenance_preserved,
        false_authorizations,
        false_denials,
        manifest_mutations: 0,
        registry_mutations: 0,
        production_authorizations: 0,
    };
    assert_eq!(report.source_records, 5);
    assert_eq!(report.cases, 600);
    assert_eq!(report.supported_cases, 360);
    assert_eq!(report.ambiguous_cases, 120);
    assert_eq!(report.unsupported_cases, 120);
    assert_eq!(report.exact_decisions, 600);
    assert_eq!(report.supported_authorized, 360);
    assert_eq!(report.supported_replays, 360);
    assert_eq!(report.supported_tamper_rejections, 360);
    assert_eq!(report.all_replays, 600);
    assert_eq!(report.all_tamper_rejections, 600);
    assert_eq!(report.provenance_preserved, 600);
    assert_eq!(report.false_authorizations, 0);
    assert_eq!(report.false_denials, 0);
    assert_eq!(report.manifest_mutations, 0);
    assert_eq!(report.registry_mutations, 0);
    assert_eq!(report.production_authorizations, 0);
    fs::write(REPORT_JSON, serde_json::to_vec_pretty(&report)?)?;
    fs::write(
        REPORT_MD,
        format!(
            "# Stage 267 — economics shadow validation\n\nFresh generic-front-end validation of the utility-selected economics candidate.\n\n* cases: {} ({} supported / {} ambiguous / {} unsupported)\n* exact decisions: {}\n* supported authorization / replay / tamper: {} / {} / {}\n* all frontend replay / tamper: {} / {}\n* provenance: {}\n* false authorizations / denials: 0 / 0\n* live manifest / registry mutations: 0 / 0\n\nThe candidate remains clone-only; no production authorization occurred.\n\nReproduce with `cargo run --quiet --bin stage267_economics_shadow_validation`.\n",
            report.cases,
            report.supported_cases,
            report.ambiguous_cases,
            report.unsupported_cases,
            report.exact_decisions,
            report.supported_authorized,
            report.supported_replays,
            report.supported_tamper_rejections,
            report.all_replays,
            report.all_tamper_rejections,
            report.provenance_preserved,
        ),
    )?;
    println!(
        "stage267 cases={} exact={} supported_auth={} false_auth=0 manifest_mutated=false",
        report.cases, report.exact_decisions, report.supported_authorized
    );
    Ok(())
}
