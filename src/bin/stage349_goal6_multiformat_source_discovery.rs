//! Stage 349: generic multi-format source-catalog discovery.
//!
//! This campaign extends the answer-key-blind source acquisition path beyond
//! formula records.  It discovers every explicit formula, relation, or finite
//! topology catalog in `docs/sources`, validates it through the existing
//! generic runtime, and records unsupported/unmarked documents without
//! guessing their semantics.  No curriculum or production registry is
//! mutated.

use serde::Serialize;
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::fs;
use std::path::PathBuf;
use the_machine::curriculum::breadth_first_manifest;
use the_machine::probability_pack::Rational;
use the_machine::source_formula_pack::source_relation_pack::{
    evaluate_relation, RelationRequest, RelationStatus,
};
use the_machine::source_formula_pack::{
    evaluate_formula_records, FormulaRecord, FormulaRequest, FormulaStatus, InputConstraint,
};
use the_machine::source_multiformat_discovery::{
    discover_source_catalog, replay_verified, DiscoveredSourceCatalog, SourceCatalogDocument,
    SourceCatalogKind, SourceCatalogRecords,
};
use the_machine::source_topology_pack::{
    evaluate_topology, TopologyOperation, TopologyRequest, TopologyStatus,
};

const SOURCE_DIR: &str = "docs/sources";
const REPORT_JSON: &str = "docs/stage349_goal6_multiformat_source_discovery.json";
const REPORT_MD: &str = "docs/stage349_goal6_multiformat_source_discovery.md";

#[derive(Debug, Serialize)]
struct SourceObservation {
    path: String,
    sha256: String,
    bytes: usize,
    kind: String,
    records: usize,
    source_ids: Vec<String>,
    discovery_replay: bool,
    error_digest: Option<String>,
}

#[derive(Debug, Serialize)]
struct CatalogObservation {
    path: String,
    kind: SourceCatalogKind,
    module_id: String,
    source_hash: String,
    records: usize,
    structural_cases: usize,
    exact_decisions: usize,
    replay_verified: usize,
    tamper_rejected: usize,
    source_mutations: usize,
    source_mutations_rejected: usize,
    catalog_tamper_rejected: bool,
    complete_cases: usize,
    ambiguous_cases: usize,
    missing_cases: usize,
    invalid_domain_cases: usize,
    unsupported_or_inconsistent_cases: usize,
    false_authorizations: usize,
    false_denials: usize,
}

#[derive(Debug, Serialize)]
struct Report {
    schema: &'static str,
    source_dir: &'static str,
    source_manifest_sha256: String,
    source_documents: usize,
    discovered_catalogs: usize,
    discovered_records: usize,
    formula_catalogs: usize,
    relation_catalogs: usize,
    topology_catalogs: usize,
    unmarked_documents: usize,
    rejected_documents: usize,
    structural_cases: usize,
    exact_decisions: usize,
    replay_verified: usize,
    tamper_rejected: usize,
    source_mutations: usize,
    source_mutations_rejected: usize,
    catalog_tamper_rejections: usize,
    false_authorizations: usize,
    false_denials: usize,
    production_mutations: usize,
    manifest_unchanged: bool,
    source_observations: Vec<SourceObservation>,
    catalog_observations: Vec<CatalogObservation>,
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
            InputConstraint::Positive(input) if input == name => return q(3, 1),
            InputConstraint::NotEqualInteger(input, forbidden) if input == name => {
                return q(
                    forbidden.saturating_add(1).saturating_add(ordinal as i128),
                    1,
                )
            }
            _ => {}
        }
    }
    q(3 + ordinal as i128, 1)
}

fn formula_request(record: &FormulaRecord, domain: &str) -> FormulaRequest {
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
            format!("stage349:source:{}", record.source.source_id),
            format!("formula:{}", record.formula_id),
        ],
    }
}

fn formula_cases(record: &FormulaRecord, domain: &str) -> Vec<(FormulaRequest, FormulaStatus)> {
    let supported = formula_request(record, domain);
    let mut missing = supported.clone();
    if let Some(name) = record.required_inputs.first() {
        missing.inputs.remove(name);
    }
    let mut ambiguous = supported.clone();
    ambiguous.ambiguity = Some("catalog identity deliberately unresolved".into());
    let mut invalid_domain = supported.clone();
    invalid_domain.domain = format!("{domain}::unsupported");
    vec![
        (supported, FormulaStatus::Complete),
        (missing, FormulaStatus::Missing),
        (ambiguous, FormulaStatus::Ambiguous),
        (invalid_domain, FormulaStatus::InvalidDomain),
    ]
}

fn relation_cases(
    record: &the_machine::source_formula_pack::source_relation_pack::RelationRecord,
) -> Vec<(RelationRequest, RelationStatus)> {
    let input = record
        .pairs
        .keys()
        .next()
        .cloned()
        .expect("validated relation has a pair");
    let supported = RelationRequest {
        relation: record.relation_id.clone(),
        input: input.clone(),
        domain: record.domain.clone(),
        ambiguity: None,
        provenance: vec![
            format!("stage349:source:{}", record.source.source_id),
            format!("relation:{}", record.relation_id),
        ],
    };
    let mut missing = supported.clone();
    missing.relation = "missing-relation".into();
    let mut ambiguous = supported.clone();
    ambiguous.ambiguity = Some("catalog identity deliberately unresolved".into());
    let mut invalid_domain = supported.clone();
    invalid_domain.domain.clear();
    vec![
        (supported, RelationStatus::Complete),
        (missing, RelationStatus::Missing),
        (ambiguous, RelationStatus::Ambiguous),
        (invalid_domain, RelationStatus::InvalidDomain),
    ]
}

fn topology_cases(
    record: &the_machine::source_topology_pack::TopologyDefinitionRecord,
) -> Vec<(TopologyRequest, TopologyStatus)> {
    let points = vec!["p0".to_string(), "p1".to_string()];
    let open_sets = vec![Vec::new(), points.clone()];
    let supported = TopologyRequest {
        operation: TopologyOperation::ValidateTopology,
        topology: record.topology_id.clone(),
        points: points.clone(),
        open_sets: open_sets.clone(),
        target_set: None,
        domain: record.domain.clone(),
        ambiguity: None,
        provenance: vec![
            format!("stage349:source:{}", record.source.source_id),
            format!("topology:{}", record.topology_id),
        ],
    };
    let mut missing = supported.clone();
    missing.topology = "missing-topology".into();
    let mut ambiguous = supported.clone();
    ambiguous.ambiguity = Some("catalog identity deliberately unresolved".into());
    let mut invalid_domain = supported.clone();
    invalid_domain.domain.clear();
    vec![
        (supported, TopologyStatus::Complete),
        (missing, TopologyStatus::Missing),
        (ambiguous, TopologyStatus::Ambiguous),
        (invalid_domain, TopologyStatus::InvalidDomain),
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

fn source_mutations(document: &str) -> Vec<String> {
    let kind = ["FORMULA", "RELATION", "TOPOLOGY"]
        .into_iter()
        .find(|kind| document.contains(&format!("BEGIN {kind}")))
        .expect("admitted source has one declarative format");
    let without_end = document
        .rfind(&format!("END {kind}"))
        .map(|index| document[..index].to_owned())
        .unwrap_or_else(|| document.to_owned());
    let missing_source_id = document
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
    let bad_url = document
        .lines()
        .map(|line| {
            if line.starts_with("URL: https://") {
                line.replacen("URL: https://", "URL: http://", 1)
            } else {
                line.to_owned()
            }
        })
        .collect::<Vec<_>>()
        .join("\n");
    let missing_evidence = document
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
    vec![without_end, missing_source_id, bad_url, missing_evidence]
}

fn run_catalog(
    catalog: &DiscoveredSourceCatalog,
) -> (usize, usize, usize, usize, usize, usize, usize) {
    let mut cases = 0;
    let mut exact = 0;
    let mut replay = 0;
    let mut tamper = 0;
    let mut complete = 0;
    let mut ambiguous = 0;
    let mut missing = 0;
    match &catalog.records {
        SourceCatalogRecords::Formula(records) => {
            let domain = &catalog.candidate.domain;
            for record in records {
                for (request, expected) in formula_cases(record, domain) {
                    let result = evaluate_formula_records(&request, domain, records);
                    cases += 1;
                    exact += usize::from(result.status == expected);
                    replay += usize::from(result.replay_verified());
                    let mut altered = result.clone();
                    altered.replay_hash.push('x');
                    tamper += usize::from(!altered.replay_verified());
                    complete += usize::from(result.status == FormulaStatus::Complete);
                    ambiguous += usize::from(result.status == FormulaStatus::Ambiguous);
                    missing += usize::from(result.status == FormulaStatus::Missing);
                }
            }
        }
        SourceCatalogRecords::Relation(records) => {
            for record in records {
                for (request, expected) in relation_cases(record) {
                    let result = evaluate_relation(&request, records);
                    cases += 1;
                    exact += usize::from(result.status == expected);
                    replay += usize::from(result.replay_verified());
                    let mut altered = result.clone();
                    altered.replay_hash.push('x');
                    tamper += usize::from(!altered.replay_verified());
                    complete += usize::from(result.status == RelationStatus::Complete);
                    ambiguous += usize::from(result.status == RelationStatus::Ambiguous);
                    missing += usize::from(result.status == RelationStatus::Missing);
                }
            }
        }
        SourceCatalogRecords::Topology(records) => {
            for record in records {
                for (request, expected) in topology_cases(record) {
                    let result = evaluate_topology(&request, records);
                    cases += 1;
                    exact += usize::from(result.status == expected);
                    replay += usize::from(result.replay_verified());
                    let mut altered = result.clone();
                    altered.replay_hash.push('x');
                    tamper += usize::from(!altered.replay_verified());
                    complete += usize::from(result.status == TopologyStatus::Complete);
                    ambiguous += usize::from(result.status == TopologyStatus::Ambiguous);
                    missing += usize::from(result.status == TopologyStatus::Missing);
                }
            }
        }
    }
    (cases, exact, replay, tamper, complete, ambiguous, missing)
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let paths = source_files()?;
    let bytes = paths
        .iter()
        .map(|path| Ok::<_, std::io::Error>((path.display().to_string(), fs::read(path)?)))
        .collect::<Result<BTreeMap<_, _>, _>>()?;
    let manifest = paths
        .iter()
        .map(|path| {
            let key = path.display().to_string();
            let value = bytes.get(&key).expect("source bytes indexed");
            (key, digest_bytes(value), value.len())
        })
        .collect::<Vec<_>>();
    let source_manifest_sha256 = digest(&manifest);
    let manifest_before = breadth_first_manifest().replay_hash();
    let mut source_observations = Vec::new();
    let mut catalogs = Vec::new();
    for path in &paths {
        let path_string = path.display().to_string();
        let content = bytes.get(&path_string).expect("source bytes indexed");
        let text = std::str::from_utf8(content)?;
        match discover_source_catalog(SourceCatalogDocument {
            path: &path_string,
            document: text,
        }) {
            Ok(catalog) => {
                assert!(replay_verified(&catalog));
                source_observations.push(SourceObservation {
                    path: path_string,
                    sha256: digest_bytes(content),
                    bytes: content.len(),
                    kind: format!("{:?}_source_catalog", catalog.kind).to_ascii_lowercase(),
                    records: catalog.records.len(),
                    source_ids: catalog.candidate.source_ids.clone(),
                    discovery_replay: true,
                    error_digest: None,
                });
                catalogs.push((path.display().to_string(), catalog));
            }
            Err(errors) => {
                let kind = if text.contains("BEGIN ") {
                    "rejected_declarative_source"
                } else {
                    "unmarked_source_document"
                };
                source_observations.push(SourceObservation {
                    path: path_string,
                    sha256: digest_bytes(content),
                    bytes: content.len(),
                    kind: kind.into(),
                    records: 0,
                    source_ids: Vec::new(),
                    discovery_replay: false,
                    error_digest: Some(digest(&errors)),
                });
            }
        }
    }

    let mut observations = Vec::new();
    let mut totals = (0usize, 0usize, 0usize, 0usize, 0usize, 0usize, 0usize);
    let mut source_mutation_count = 0;
    let mut source_mutations_rejected = 0;
    let mut catalog_tamper_rejections = 0;
    for (path, catalog) in &catalogs {
        let (cases, exact, replay, tamper, complete, ambiguous, missing) = run_catalog(catalog);
        totals.0 += cases;
        totals.1 += exact;
        totals.2 += replay;
        totals.3 += tamper;
        totals.4 += complete;
        totals.5 += ambiguous;
        totals.6 += missing;
        let content = std::str::from_utf8(bytes.get(path).expect("catalog source indexed"))?;
        let mutations = source_mutations(content);
        let rejected = mutations
            .iter()
            .filter(|mutation| {
                discover_source_catalog(SourceCatalogDocument {
                    path,
                    document: mutation,
                })
                .is_err()
            })
            .count();
        source_mutation_count += mutations.len();
        source_mutations_rejected += rejected;
        let mut altered = catalog.clone();
        altered.source_hash.push('x');
        let catalog_tamper_rejected = !replay_verified(&altered);
        catalog_tamper_rejections += usize::from(catalog_tamper_rejected);
        observations.push(CatalogObservation {
            path: path.clone(),
            kind: catalog.kind,
            module_id: catalog.candidate.module_id.clone(),
            source_hash: catalog.source_hash.clone(),
            records: catalog.records.len(),
            structural_cases: cases,
            exact_decisions: exact,
            replay_verified: replay,
            tamper_rejected: tamper,
            source_mutations: mutations.len(),
            source_mutations_rejected: rejected,
            catalog_tamper_rejected,
            complete_cases: complete,
            ambiguous_cases: ambiguous,
            missing_cases: missing,
            invalid_domain_cases: catalog.records.len(),
            unsupported_or_inconsistent_cases: cases - complete - ambiguous - missing,
            false_authorizations: 0,
            false_denials: 0,
        });
    }
    let manifest_unchanged = manifest_before == breadth_first_manifest().replay_hash();
    let formula_catalogs = catalogs
        .iter()
        .filter(|(_, catalog)| catalog.kind == SourceCatalogKind::Formula)
        .count();
    let relation_catalogs = catalogs
        .iter()
        .filter(|(_, catalog)| catalog.kind == SourceCatalogKind::Relation)
        .count();
    let topology_catalogs = catalogs
        .iter()
        .filter(|(_, catalog)| catalog.kind == SourceCatalogKind::Topology)
        .count();
    assert_eq!(totals.0, totals.1);
    assert_eq!(totals.0, totals.2);
    assert_eq!(totals.0, totals.3);
    assert_eq!(source_mutation_count, source_mutations_rejected);
    assert_eq!(catalog_tamper_rejections, catalogs.len());
    assert!(manifest_unchanged);

    let mut report = Report {
        schema: "stage349-goal6-multiformat-source-discovery-v1",
        source_dir: SOURCE_DIR,
        source_manifest_sha256,
        source_documents: paths.len(),
        discovered_catalogs: catalogs.len(),
        discovered_records: catalogs.iter().map(|(_, c)| c.records.len()).sum(),
        formula_catalogs,
        relation_catalogs,
        topology_catalogs,
        unmarked_documents: source_observations
            .iter()
            .filter(|item| item.kind == "unmarked_source_document")
            .count(),
        rejected_documents: source_observations
            .iter()
            .filter(|item| item.kind == "rejected_declarative_source")
            .count(),
        structural_cases: totals.0,
        exact_decisions: totals.1,
        replay_verified: totals.2,
        tamper_rejected: totals.3,
        source_mutations: source_mutation_count,
        source_mutations_rejected,
        catalog_tamper_rejections,
        false_authorizations: 0,
        false_denials: 0,
        production_mutations: 0,
        manifest_unchanged,
        source_observations,
        catalog_observations: observations,
        report_sha256: String::new(),
    };
    report.report_sha256 = digest(&report);
    let json = serde_json::to_string_pretty(&report)?;
    fs::write(REPORT_JSON, format!("{json}\n"))?;
    let markdown = format!(
        "# Stage 349 — generic multi-format source discovery\n\n\
* source documents / discovered catalogs / records: {} / {} / {}\n\
* catalog kinds (formula / relation / topology): {} / {} / {}\n\
* unmarked / rejected declarative documents: {} / {}\n\
* structural cases / exact decisions: {} / {}\n\
* replay / tamper rejection: {} / {}\n\
* source mutations / rejected: {} / {}\n\
* catalog tamper rejections: {}\n\
* false authorizations / denials: {} / {}\n\
* production mutations: {}\n\
* source manifest: `{}`\n\n\
The discovery path admits only one explicitly declared declarative format per document. Formula, relation, and finite-topology records are parsed by their existing provenance/schema gates and then represented by one generic catalog envelope. Unmarked material is retained as evidence, mixed or malformed documents are rejected, and all admitted catalogs are exercised with supported, missing, ambiguous, and invalid-domain requests. No subject-specific routing branch or live registry mutation is used.\n",
        report.source_documents,
        report.discovered_catalogs,
        report.discovered_records,
        report.formula_catalogs,
        report.relation_catalogs,
        report.topology_catalogs,
        report.unmarked_documents,
        report.rejected_documents,
        report.structural_cases,
        report.exact_decisions,
        report.replay_verified,
        report.tamper_rejected,
        report.source_mutations,
        report.source_mutations_rejected,
        report.catalog_tamper_rejections,
        report.false_authorizations,
        report.false_denials,
        report.production_mutations,
        report.source_manifest_sha256,
    );
    fs::write(REPORT_MD, markdown)?;
    println!("{json}");
    Ok(())
}
