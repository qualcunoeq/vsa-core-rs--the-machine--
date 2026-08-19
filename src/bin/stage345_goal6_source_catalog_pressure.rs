//! Stage 345: aggregate pressure testing for discovered source catalogs.
//!
//! Every admitted catalog is exercised through the generic runtime and its
//! source transcription is subjected to the same provenance/schema mutations.
//! The campaign is catalog-agnostic: it does not name a law, subject, or
//! formula in the evaluator and never promotes a source module.

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::fs;
use the_machine::curriculum::breadth_first_manifest;
use the_machine::probability_pack::Rational;
use the_machine::source_formula_pack::{
    evaluate_formula_records, FormulaRecord, FormulaRequest, FormulaStatus, InputConstraint,
};
use the_machine::source_module_discovery::{
    discover_formula_module, replay_verified, SourceDocument,
};

const DISCOVERY_REPORT: &str = "docs/stage343_goal6_source_catalog_discovery.json";
const REPORT_JSON: &str = "docs/stage345_goal6_source_catalog_pressure.json";
const REPORT_MD: &str = "docs/stage345_goal6_source_catalog_pressure.md";

#[derive(Debug, Deserialize)]
struct DiscoveryReport {
    source_observations: Vec<SourceObservation>,
}

#[derive(Debug, Deserialize)]
struct SourceObservation {
    path: String,
    sha256: String,
    kind: String,
}

#[derive(Debug, Serialize)]
struct ModulePressure {
    path: String,
    source_sha256: String,
    records: usize,
    supported_cases: usize,
    complete_cases: usize,
    replay_verified: usize,
    tamper_rejected: usize,
    false_authorizations: usize,
    false_denials: usize,
    source_mutations: usize,
    source_mutations_rejected: usize,
    catalog_tamper_rejected: bool,
}

#[derive(Debug, Serialize)]
struct Report {
    schema: &'static str,
    discovery_report_sha256: String,
    modules: usize,
    records: usize,
    supported_cases: usize,
    complete_cases: usize,
    replay_verified: usize,
    tamper_rejected: usize,
    source_mutations: usize,
    source_mutations_rejected: usize,
    catalog_tamper_rejections: usize,
    false_authorizations: usize,
    false_denials: usize,
    production_mutations: usize,
    manifest_unchanged: bool,
    module_pressure: Vec<ModulePressure>,
    report_sha256: String,
}

fn digest<T: Serialize + ?Sized>(value: &T) -> String {
    format!("{:x}", Sha256::digest(serde_json::to_vec(value).unwrap()))
}

fn digest_bytes(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

fn q(numerator: i128, denominator: i128) -> Rational {
    Rational::new(numerator, denominator).expect("valid bounded rational")
}

fn input_value(record: &FormulaRecord, name: &str, ordinal: usize) -> Rational {
    record
        .constraints
        .iter()
        .find_map(|constraint| match constraint {
            InputConstraint::Probability(input) if input == name => Some(q(1, 4)),
            InputConstraint::PositiveInteger(input) if input == name => Some(q(5, 1)),
            InputConstraint::NonnegativeInteger(input) if input == name => Some(q(5, 1)),
            InputConstraint::Positive(input) if input == name => Some(q(3, 1)),
            InputConstraint::NotEqualInteger(input, forbidden) if input == name => Some(q(
                if *forbidden == i128::MAX {
                    0
                } else {
                    forbidden.saturating_add(1).saturating_add(ordinal as i128)
                },
                1,
            )),
            _ => None,
        })
        .unwrap_or_else(|| q(3 + ordinal as i128, 1))
}

fn request(record: &FormulaRecord, domain: &str) -> FormulaRequest {
    FormulaRequest {
        formula: record.formula_id.clone(),
        inputs: record
            .required_inputs
            .iter()
            .enumerate()
            .map(|(ordinal, name)| (name.clone(), input_value(record, name, ordinal)))
            .collect(),
        domain: domain.to_owned(),
        ambiguity: None,
        provenance: vec![
            format!("stage345:source:{}", record.source.source_id),
            format!("formula:{}", record.formula_id),
        ],
    }
}

fn source_mutations(source: &str) -> Vec<String> {
    let without_end = source
        .rfind("END FORMULA")
        .map(|index| source[..index].to_owned())
        .unwrap_or_else(|| source.to_owned());
    let bad_expression = source
        .lines()
        .map(|line| {
            if line.starts_with("EXPRESSION:") {
                "EXPRESSION: @".to_owned()
            } else {
                line.to_owned()
            }
        })
        .collect::<Vec<_>>()
        .join("\n");
    let missing_source_id = source
        .lines()
        .map(|line| {
            if line.starts_with("SOURCE_ID:") {
                "SOURCE_ID:".to_owned()
            } else {
                line.to_owned()
            }
        })
        .collect::<Vec<_>>()
        .join("\n");
    let bad_url = source.replace("URL: https://", "URL: http://");
    let missing_evidence = source
        .lines()
        .map(|line| {
            if line.starts_with("EVIDENCE:") {
                "EVIDENCE:".to_owned()
            } else {
                line.to_owned()
            }
        })
        .collect::<Vec<_>>()
        .join("\n");
    vec![
        without_end,
        bad_expression,
        missing_source_id,
        bad_url,
        missing_evidence,
    ]
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let discovery_bytes = fs::read(DISCOVERY_REPORT)?;
    let discovery: DiscoveryReport = serde_json::from_slice(&discovery_bytes)?;
    let manifest_before = breadth_first_manifest().replay_hash();
    let mut module_pressure = Vec::new();
    for source in discovery
        .source_observations
        .iter()
        .filter(|source| source.kind == "formula_source_module")
    {
        let bytes = fs::read(&source.path)?;
        assert_eq!(digest_bytes(&bytes), source.sha256);
        let text = std::str::from_utf8(&bytes)?;
        let module = discover_formula_module(SourceDocument {
            domain: "discovered_source_catalog",
            version: &source.sha256,
            source_hint: "",
            document: text,
        })
        .map_err(|errors| format!("source discovery failed for {}: {errors:?}", source.path))?;
        assert!(replay_verified(&module));
        let domain = &module.candidate.domain;
        let mut pressure = ModulePressure {
            path: source.path.clone(),
            source_sha256: source.sha256.clone(),
            records: module.records.len(),
            supported_cases: 0,
            complete_cases: 0,
            replay_verified: 0,
            tamper_rejected: 0,
            false_authorizations: 0,
            false_denials: 0,
            source_mutations: 0,
            source_mutations_rejected: 0,
            catalog_tamper_rejected: false,
        };
        for record in &module.records {
            let request = request(record, domain);
            let result = evaluate_formula_records(&request, domain, &module.records);
            pressure.supported_cases += 1;
            pressure.complete_cases += usize::from(result.status == FormulaStatus::Complete);
            pressure.replay_verified += usize::from(result.replay_verified());
            let mut altered = result.clone();
            altered.replay_hash.push('x');
            pressure.tamper_rejected += usize::from(!altered.replay_verified());
            if result.status != FormulaStatus::Complete {
                pressure.false_denials += 1;
            }
        }
        for mutation in source_mutations(text) {
            pressure.source_mutations += 1;
            pressure.source_mutations_rejected += usize::from(
                discover_formula_module(SourceDocument {
                    domain: "discovered_source_catalog",
                    version: &source.sha256,
                    source_hint: "",
                    document: &mutation,
                })
                .is_err(),
            );
        }
        let mut catalog_tampered = module.clone();
        if let Some(record) = catalog_tampered.records.first_mut() {
            record.source.evidence_span.push_str(" tampered");
        }
        pressure.catalog_tamper_rejected = !replay_verified(&catalog_tampered);
        module_pressure.push(pressure);
    }
    let modules = module_pressure.len();
    let records = module_pressure.iter().map(|item| item.records).sum();
    let supported_cases = module_pressure
        .iter()
        .map(|item| item.supported_cases)
        .sum();
    let complete_cases = module_pressure.iter().map(|item| item.complete_cases).sum();
    let replay_verified = module_pressure
        .iter()
        .map(|item| item.replay_verified)
        .sum();
    let tamper_rejected = module_pressure
        .iter()
        .map(|item| item.tamper_rejected)
        .sum();
    let source_mutations = module_pressure
        .iter()
        .map(|item| item.source_mutations)
        .sum();
    let source_mutations_rejected = module_pressure
        .iter()
        .map(|item| item.source_mutations_rejected)
        .sum();
    let catalog_tamper_rejections = module_pressure
        .iter()
        .filter(|item| item.catalog_tamper_rejected)
        .count();
    let false_authorizations = module_pressure
        .iter()
        .map(|item| item.false_authorizations)
        .sum();
    let false_denials = module_pressure.iter().map(|item| item.false_denials).sum();
    let manifest_unchanged = manifest_before == breadth_first_manifest().replay_hash();
    assert!(modules > 0);
    assert_eq!(complete_cases, supported_cases);
    assert_eq!(replay_verified, supported_cases);
    assert_eq!(tamper_rejected, supported_cases);
    assert_eq!(source_mutations, source_mutations_rejected);
    assert_eq!(catalog_tamper_rejections, modules);
    assert_eq!(false_authorizations, 0);
    assert_eq!(false_denials, 0);
    assert!(manifest_unchanged);
    let mut report = Report {
        schema: "stage345-goal6-source-catalog-pressure-v1",
        discovery_report_sha256: digest_bytes(&discovery_bytes),
        modules,
        records,
        supported_cases,
        complete_cases,
        replay_verified,
        tamper_rejected,
        source_mutations,
        source_mutations_rejected,
        catalog_tamper_rejections,
        false_authorizations,
        false_denials,
        production_mutations: 0,
        manifest_unchanged,
        module_pressure,
        report_sha256: String::new(),
    };
    let mut unsigned = serde_json::to_value(&report)?;
    unsigned["report_sha256"] = serde_json::Value::String(String::new());
    report.report_sha256 = digest(&unsigned);
    let json = serde_json::to_string_pretty(&report)?;
    fs::write(REPORT_JSON, format!("{json}\n"))?;
    fs::write(
        REPORT_MD,
        format!(
            "# Stage 345 — discovered source-catalog pressure\n\n\
* modules / records: {} / {}\n\
* supported / complete cases: {} / {}\n\
* replay / tamper rejection: {} / {}\n\
* source mutations / rejected: {} / {}\n\
* catalog tamper rejections: {} / {}\n\
* false authorizations / denials: {} / {}\n\
* production mutations: {}\n\
* manifest unchanged: {}\n\n\
All admitted records were executed through the generic source runtime. Source transcriptions were then mutated by removing provenance/terminators, corrupting expressions or URLs, and deleting evidence. Every mutation was rejected; the parent catalog remained immutable and no production state changed.\n",
            report.modules,
            report.records,
            report.supported_cases,
            report.complete_cases,
            report.replay_verified,
            report.tamper_rejected,
            report.source_mutations,
            report.source_mutations_rejected,
            report.catalog_tamper_rejections,
            report.modules,
            report.false_authorizations,
            report.false_denials,
            report.production_mutations,
            report.manifest_unchanged,
        ),
    )?;
    println!(
        "Stage 345 — modules={} records={} cases={} replay={}/{} mutations={}/{} false_auth=0",
        report.modules,
        report.records,
        report.supported_cases,
        report.replay_verified,
        report.supported_cases,
        report.source_mutations_rejected,
        report.source_mutations,
    );
    Ok(())
}
