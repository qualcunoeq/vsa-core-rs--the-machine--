//! Stage 343: generic source-catalog discovery across the bounded source set.
//!
//! This campaign deliberately does not receive a subject list or a route
//! table.  It inventories the source documents, admits only documents whose
//! declarative formula records pass the existing provenance/schema gate, and
//! validates every admitted record through the generic formula runtime.  A
//! document without formula records is recorded as non-formula material; a
//! malformed formula document is rejected.  No source is promoted and no
//! curriculum/production registry is mutated.

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::PathBuf;
use the_machine::curriculum::breadth_first_manifest;
use the_machine::probability_pack::Rational;
use the_machine::source_formula_pack::{
    evaluate_formula_records, FormulaRecord, FormulaRequest, FormulaStatus, InputConstraint,
};
use the_machine::source_module_discovery::{
    discover_formula_module, replay_verified, SourceDocument,
};

const SOURCE_DIR: &str = "docs/sources";
const REPORT_JSON: &str = "docs/stage343_goal6_source_catalog_discovery.json";
const REPORT_MD: &str = "docs/stage343_goal6_source_catalog_discovery.md";

#[derive(Debug, Serialize, Deserialize)]
struct SourceObservation {
    path: String,
    sha256: String,
    bytes: usize,
    kind: String,
    records: usize,
    source_ids: Vec<String>,
    input_bindings: usize,
    module_id: Option<String>,
    discovery_replay: bool,
    error_digest: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
struct ModuleObservation {
    module_id: String,
    path: String,
    source_hash: String,
    replay_hash: String,
    records: usize,
    independent_structural_cases: usize,
    complete_cases: usize,
    ambiguous_cases: usize,
    missing_cases: usize,
    invalid_domain_cases: usize,
    exact_decisions: usize,
    replay_verified: usize,
    tamper_rejected: usize,
    false_authorizations: usize,
    false_denials: usize,
}

#[derive(Debug, Serialize, Deserialize)]
struct Report {
    schema: &'static str,
    source_dir: &'static str,
    source_manifest_sha256: String,
    source_documents: usize,
    formula_documents: usize,
    non_formula_documents: usize,
    rejected_formula_documents: usize,
    discovered_modules: usize,
    discovered_records: usize,
    source_observations: Vec<SourceObservation>,
    module_observations: Vec<ModuleObservation>,
    structural_cases: usize,
    exact_decisions: usize,
    replay_verified: usize,
    tamper_rejected: usize,
    false_authorizations: usize,
    false_denials: usize,
    production_mutations: usize,
    manifest_unchanged: bool,
    report_sha256: String,
}

fn digest<T: Serialize + ?Sized>(value: &T) -> String {
    format!("{:x}", Sha256::digest(serde_json::to_vec(value).unwrap()))
}

fn digest_bytes(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

fn q(numerator: i128, denominator: i128) -> Rational {
    Rational::new(numerator, denominator).expect("bounded benchmark rational is valid")
}

fn input_value(record: &FormulaRecord, name: &str, ordinal: usize) -> Rational {
    for constraint in &record.constraints {
        match constraint {
            InputConstraint::Probability(input) if input == name => return q(1, 4),
            InputConstraint::PositiveInteger(input) if input == name => return q(5, 1),
            InputConstraint::NonnegativeInteger(input) if input == name => return q(5, 1),
            InputConstraint::NotEqualInteger(input, forbidden) if input == name => {
                return q(
                    if *forbidden == i128::MAX {
                        0
                    } else {
                        (*forbidden)
                            .saturating_add(1)
                            .saturating_add(ordinal as i128)
                    },
                    1,
                )
            }
            InputConstraint::Positive(input) if input == name => return q(3, 1),
            _ => {}
        }
    }
    q(3 + ordinal as i128, 1)
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
            format!("stage343:source:{}", record.source.source_id),
            format!("formula:{}", record.formula_id),
        ],
    }
}

fn structural_cases(record: &FormulaRecord, domain: &str) -> Vec<(FormulaRequest, FormulaStatus)> {
    let supported = request(record, domain);
    let mut missing = supported.clone();
    if let Some(name) = record.required_inputs.first() {
        missing.inputs.remove(name);
    }
    let mut ambiguous = supported.clone();
    ambiguous.ambiguity = Some("source catalog identity deliberately unresolved".into());
    let mut invalid_domain = supported.clone();
    invalid_domain.domain = format!("{domain}::unsupported");
    vec![
        (supported, FormulaStatus::Complete),
        (missing, FormulaStatus::Missing),
        (ambiguous, FormulaStatus::Ambiguous),
        (invalid_domain, FormulaStatus::InvalidDomain),
    ]
}

fn source_files() -> Result<Vec<PathBuf>, Box<dyn std::error::Error>> {
    let mut paths = fs::read_dir(SOURCE_DIR)?
        .filter_map(|entry| entry.ok().map(|entry| entry.path()))
        .filter(|path| path.extension().is_some_and(|extension| extension == "txt"))
        .collect::<Vec<_>>();
    paths.sort();
    Ok(paths)
}

fn source_manifest(
    paths: &[PathBuf],
    bytes: &BTreeMap<String, Vec<u8>>,
) -> Vec<(String, String, usize)> {
    paths
        .iter()
        .map(|path| {
            let path_string = path.display().to_string();
            let content = bytes.get(&path_string).expect("source bytes indexed");
            (path_string, digest_bytes(content), content.len())
        })
        .collect()
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let paths = source_files()?;
    let bytes = paths
        .iter()
        .map(|path| Ok::<_, std::io::Error>((path.display().to_string(), fs::read(path)?)))
        .collect::<Result<BTreeMap<_, _>, _>>()?;
    let manifest = source_manifest(&paths, &bytes);
    let source_manifest_sha256 = digest(&manifest);
    let manifest_before = breadth_first_manifest().replay_hash();

    let mut observations = Vec::new();
    let mut modules = Vec::new();
    for path in &paths {
        let path_string = path.display().to_string();
        let content = bytes.get(&path_string).expect("source bytes indexed");
        let text = std::str::from_utf8(content)?;
        if !text.contains("BEGIN FORMULA") {
            observations.push(SourceObservation {
                path: path_string,
                sha256: digest_bytes(content),
                bytes: content.len(),
                kind: "non_formula_source_document".into(),
                records: 0,
                source_ids: Vec::new(),
                input_bindings: 0,
                module_id: None,
                discovery_replay: false,
                error_digest: None,
            });
            continue;
        }
        match discover_formula_module(SourceDocument {
            domain: "discovered_source_catalog",
            version: &digest_bytes(content),
            source_hint: "",
            document: text,
        }) {
            Ok(module) => {
                let mut source_ids = BTreeSet::new();
                let mut input_bindings = 0;
                for record in &module.records {
                    source_ids.insert(record.source.source_id.clone());
                    input_bindings += record.input_bindings.len();
                }
                let discovery_replay = replay_verified(&module);
                assert!(discovery_replay);
                observations.push(SourceObservation {
                    path: path_string.clone(),
                    sha256: digest_bytes(content),
                    bytes: content.len(),
                    kind: "formula_source_module".into(),
                    records: module.records.len(),
                    source_ids: source_ids.into_iter().collect(),
                    input_bindings,
                    module_id: Some(module.candidate.module_id.clone()),
                    discovery_replay,
                    error_digest: None,
                });
                modules.push((path_string, module));
            }
            Err(errors) => {
                observations.push(SourceObservation {
                    path: path_string,
                    sha256: digest_bytes(content),
                    bytes: content.len(),
                    kind: "rejected_formula_source".into(),
                    records: 0,
                    source_ids: Vec::new(),
                    input_bindings: 0,
                    module_id: None,
                    discovery_replay: false,
                    error_digest: Some(digest(&errors)),
                });
            }
        }
    }

    let mut module_observations = Vec::new();
    let mut structural_case_count = 0;
    let mut exact_decisions = 0;
    let mut replay_verified_count = 0;
    let mut tamper_rejected = 0;
    for (path, module) in &modules {
        let domain = &module.candidate.domain;
        let mut observation = ModuleObservation {
            module_id: module.candidate.module_id.clone(),
            path: path.clone(),
            source_hash: module.source_hash.clone(),
            replay_hash: module.replay_hash.clone(),
            records: module.records.len(),
            independent_structural_cases: 0,
            complete_cases: 0,
            ambiguous_cases: 0,
            missing_cases: 0,
            invalid_domain_cases: 0,
            exact_decisions: 0,
            replay_verified: 0,
            tamper_rejected: 0,
            false_authorizations: 0,
            false_denials: 0,
        };
        for record in &module.records {
            for (request, expected) in structural_cases(record, domain) {
                let result = evaluate_formula_records(&request, domain, &module.records);
                let exact = result.status == expected
                    && (expected != FormulaStatus::Complete
                        || (result.value.is_some() && result.source.is_some()));
                let replay = result.replay_verified();
                let mut altered = result.clone();
                altered.replay_hash.push('x');
                let tamper = !altered.replay_verified();
                observation.independent_structural_cases += 1;
                structural_case_count += 1;
                observation.exact_decisions += usize::from(exact);
                exact_decisions += usize::from(exact);
                observation.replay_verified += usize::from(replay);
                replay_verified_count += usize::from(replay);
                observation.tamper_rejected += usize::from(tamper);
                tamper_rejected += usize::from(tamper);
                match result.status {
                    FormulaStatus::Complete => observation.complete_cases += 1,
                    FormulaStatus::Ambiguous => observation.ambiguous_cases += 1,
                    FormulaStatus::Missing => observation.missing_cases += 1,
                    FormulaStatus::InvalidDomain => observation.invalid_domain_cases += 1,
                    _ => {}
                }
                if expected == FormulaStatus::Complete && !exact {
                    observation.false_denials += 1;
                }
                if expected != FormulaStatus::Complete && result.status == FormulaStatus::Complete {
                    observation.false_authorizations += 1;
                }
            }
        }
        module_observations.push(observation);
    }
    let manifest_unchanged = manifest_before == breadth_first_manifest().replay_hash();
    let false_authorizations = module_observations
        .iter()
        .map(|observation| observation.false_authorizations)
        .sum();
    let false_denials = module_observations
        .iter()
        .map(|observation| observation.false_denials)
        .sum();
    assert_eq!(false_authorizations, 0);
    assert_eq!(false_denials, 0);
    assert_eq!(exact_decisions, structural_case_count);
    assert_eq!(replay_verified_count, structural_case_count);
    assert_eq!(tamper_rejected, structural_case_count);
    assert!(manifest_unchanged);

    let mut report = Report {
        schema: "stage343-goal6-source-catalog-discovery-v1",
        source_dir: SOURCE_DIR,
        source_manifest_sha256,
        source_documents: observations.len(),
        formula_documents: observations
            .iter()
            .filter(|observation| observation.kind == "formula_source_module")
            .count(),
        non_formula_documents: observations
            .iter()
            .filter(|observation| observation.kind == "non_formula_source_document")
            .count(),
        rejected_formula_documents: observations
            .iter()
            .filter(|observation| observation.kind == "rejected_formula_source")
            .count(),
        discovered_modules: modules.len(),
        discovered_records: modules.iter().map(|(_, module)| module.records.len()).sum(),
        source_observations: observations,
        module_observations,
        structural_cases: structural_case_count,
        exact_decisions,
        replay_verified: replay_verified_count,
        tamper_rejected,
        false_authorizations,
        false_denials,
        production_mutations: 0,
        manifest_unchanged,
        report_sha256: String::new(),
    };
    report.report_sha256 = digest(&report);
    let json = serde_json::to_string_pretty(&report)?;
    fs::write(REPORT_JSON, format!("{json}\n"))?;
    let markdown = format!(
        "# Stage 343 — generic source-catalog discovery\n\n\
* source documents / formula documents / non-formula / rejected: {} / {} / {} / {}\n\
* discovered modules / records: {} / {}\n\
* structural cases / exact decisions: {} / {}\n\
* replay / tamper rejection: {} / {}\n\
* false authorizations / denials: {} / {}\n\
* production mutations: {}\n\
* source manifest: `{}`\n\n\
The catalog was discovered from the bounded source directory without a subject list or route-specific dispatcher. Non-formula source documents were retained as non-formula evidence; formula documents were admitted only after provenance and schema validation. Every admitted record was exercised through the generic formula runtime with supported, missing-input, ambiguous, and invalid-domain cases. No catalog, curriculum manifest, router, or production registry was mutated.\n",
        report.source_documents,
        report.formula_documents,
        report.non_formula_documents,
        report.rejected_formula_documents,
        report.discovered_modules,
        report.discovered_records,
        report.structural_cases,
        report.exact_decisions,
        report.replay_verified,
        report.tamper_rejected,
        report.false_authorizations,
        report.false_denials,
        report.production_mutations,
        report.source_manifest_sha256
    );
    fs::write(REPORT_MD, markdown)?;
    println!("{json}");
    Ok(())
}
