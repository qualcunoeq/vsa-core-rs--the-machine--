//! Stage 351: independent pressure corpus for discovered source catalogs.
//!
//! The corpus is generated from the typed catalog interfaces, not from source
//! subject names or answer keys.  It deliberately combines formula,
//! relation, and finite-topology requests with valid, ambiguous, malformed,
//! and out-of-bound cases.  Every result must match its declared status and
//! survive receipt tampering; no result authorizes a live route.

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::fs;
use the_machine::curriculum::breadth_first_manifest;
use the_machine::probability_pack::Rational;
use the_machine::source_formula_pack::source_relation_pack::{
    evaluate_relation, RelationRecord, RelationRequest, RelationStatus,
};
use the_machine::source_formula_pack::{
    evaluate_formula_records, FormulaRecord, FormulaRequest, FormulaStatus, InputConstraint,
};
use the_machine::source_multiformat_discovery::{
    discover_source_catalog, SourceCatalogDocument, SourceCatalogKind, SourceCatalogRecords,
};
use the_machine::source_topology_pack::{
    evaluate_topology, TopologyDefinitionRecord, TopologyOperation, TopologyRequest, TopologyStatus,
};

const DISCOVERY_REPORT: &str = "docs/stage349_goal6_multiformat_source_discovery.json";
const REPORT_JSON: &str = "docs/stage351_goal7_multiformat_pressure.json";
const REPORT_MD: &str = "docs/stage351_goal7_multiformat_pressure.md";

#[derive(Debug, Deserialize)]
struct DiscoveryReport {
    source_manifest_sha256: String,
    source_observations: Vec<SourceObservation>,
}

#[derive(Debug, Deserialize)]
struct SourceObservation {
    path: String,
    kind: String,
    sha256: String,
}

#[derive(Debug, Default, Serialize)]
struct KindSummary {
    catalogs: usize,
    records: usize,
    cases: usize,
    exact_decisions: usize,
    replay_verified: usize,
    tamper_rejected: usize,
    complete_cases: usize,
    ambiguous_cases: usize,
    missing_cases: usize,
    unsupported_or_invalid_cases: usize,
    false_authorizations: usize,
    false_denials: usize,
}

#[derive(Debug, Serialize)]
struct Report {
    schema: &'static str,
    discovery_report_sha256: String,
    source_manifest_sha256: String,
    independent_corpus_sha256: String,
    catalogs: usize,
    records: usize,
    cases: usize,
    exact_decisions: usize,
    replay_verified: usize,
    tamper_rejected: usize,
    complete_cases: usize,
    ambiguous_cases: usize,
    missing_cases: usize,
    unsupported_or_invalid_cases: usize,
    false_authorizations: usize,
    false_denials: usize,
    production_mutations: usize,
    manifest_unchanged: bool,
    by_kind: BTreeMap<String, KindSummary>,
    report_sha256: String,
}

fn digest<T: Serialize + ?Sized>(value: &T) -> String {
    format!("{:x}", Sha256::digest(serde_json::to_vec(value).unwrap()))
}

fn digest_bytes(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

fn q(numerator: i128, denominator: i128) -> Rational {
    Rational::new(numerator, denominator).expect("bounded rational is valid")
}

fn formula_input(record: &FormulaRecord, name: &str, ordinal: usize) -> Rational {
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

fn formula_cases(record: &FormulaRecord, domain: &str) -> Vec<(FormulaRequest, FormulaStatus)> {
    let supported = FormulaRequest {
        formula: record.formula_id.clone(),
        inputs: record
            .required_inputs
            .iter()
            .enumerate()
            .map(|(ordinal, name)| (name.clone(), formula_input(record, name, ordinal)))
            .collect(),
        domain: domain.into(),
        ambiguity: None,
        provenance: vec![format!("stage351:source:{}", record.source.source_id)],
    };
    let mut missing = supported.clone();
    let missing_expected = if let Some(name) = record.required_inputs.first() {
        missing.inputs.remove(name);
        FormulaStatus::Missing
    } else {
        missing.formula = "missing-formula".into();
        FormulaStatus::Missing
    };
    let mut ambiguous = supported.clone();
    ambiguous.ambiguity = Some("independent corpus leaves catalog identity unresolved".into());
    let mut invalid_domain = supported.clone();
    invalid_domain.domain = format!("{domain}::outside");
    vec![
        (supported, FormulaStatus::Complete),
        (missing, missing_expected),
        (ambiguous, FormulaStatus::Ambiguous),
        (invalid_domain, FormulaStatus::InvalidDomain),
    ]
}

fn relation_cases(record: &RelationRecord) -> Vec<(RelationRequest, RelationStatus)> {
    let mut cases = Vec::new();
    for input in record.pairs.keys() {
        let supported = RelationRequest {
            relation: record.relation_id.clone(),
            input: input.clone(),
            domain: record.domain.clone(),
            ambiguity: None,
            provenance: vec![format!("stage351:source:{}", record.source.source_id)],
        };
        cases.push((supported.clone(), RelationStatus::Complete));
        let mut unknown_input = supported.clone();
        unknown_input.input = "unknown-symbol".into();
        cases.push((unknown_input, RelationStatus::Unsupported));
        let mut unknown_relation = supported.clone();
        unknown_relation.relation = "unknown-relation".into();
        cases.push((unknown_relation, RelationStatus::Missing));
        let mut ambiguous = supported.clone();
        ambiguous.ambiguity = Some("independent relation identity is unresolved".into());
        cases.push((ambiguous, RelationStatus::Ambiguous));
        let mut invalid_domain = supported;
        invalid_domain.domain.clear();
        cases.push((invalid_domain, RelationStatus::InvalidDomain));
    }
    cases
}

fn points(n: usize) -> Vec<String> {
    (0..n).map(|index| format!("p{index}")).collect()
}

fn topology_request(
    record: &TopologyDefinitionRecord,
    operation: TopologyOperation,
    carrier: Vec<String>,
    open_sets: Vec<Vec<String>>,
    target_set: Option<Vec<String>>,
) -> TopologyRequest {
    TopologyRequest {
        operation,
        topology: record.topology_id.clone(),
        points: carrier,
        open_sets,
        target_set,
        domain: record.domain.clone(),
        ambiguity: None,
        provenance: vec![format!("stage351:source:{}", record.source.source_id)],
    }
}

fn topology_cases(record: &TopologyDefinitionRecord) -> Vec<(TopologyRequest, TopologyStatus)> {
    let mut cases = Vec::new();
    // Independent valid carriers: indiscrete and discrete topologies at
    // several bounded sizes.
    for size in 1..=4 {
        let carrier = points(size);
        cases.push((
            topology_request(
                record,
                TopologyOperation::ValidateTopology,
                carrier.clone(),
                vec![Vec::new(), carrier.clone()],
                None,
            ),
            TopologyStatus::Complete,
        ));
        let discrete_sets = (0..(1usize << size))
            .map(|mask| {
                carrier
                    .iter()
                    .enumerate()
                    .filter(|(index, _)| (mask & (1 << index)) != 0)
                    .map(|(_, point)| point.clone())
                    .collect::<Vec<_>>()
            })
            .collect::<Vec<_>>();
        cases.push((
            topology_request(
                record,
                TopologyOperation::ValidateTopology,
                carrier,
                discrete_sets,
                None,
            ),
            TopologyStatus::Complete,
        ));
    }
    let two = points(2);
    let sierpinski = vec![Vec::new(), vec!["p0".into()], two.clone()];
    for operation in [
        TopologyOperation::IsOpen,
        TopologyOperation::IsClosed,
        TopologyOperation::Interior,
        TopologyOperation::Closure,
    ] {
        cases.push((
            topology_request(
                record,
                operation,
                two.clone(),
                sierpinski.clone(),
                Some(vec!["p0".into()]),
            ),
            TopologyStatus::Complete,
        ));
    }
    // Invalid finite structures and explicit scope failures.
    for open_sets in [
        vec![Vec::new()],
        vec![two.clone(), vec!["p0".into()]],
        vec![Vec::new(), two.clone(), vec!["outside".into()]],
        vec![Vec::new(), two.clone(), vec!["p0".into(), "outside".into()]],
    ] {
        cases.push((
            topology_request(
                record,
                TopologyOperation::ValidateTopology,
                two.clone(),
                open_sets,
                None,
            ),
            TopologyStatus::Inconsistent,
        ));
    }
    let mut duplicate_points = topology_request(
        record,
        TopologyOperation::ValidateTopology,
        vec!["p0".into(), "p0".into()],
        vec![Vec::new(), vec!["p0".into()]],
        None,
    );
    cases.push((duplicate_points.clone(), TopologyStatus::Inconsistent));
    duplicate_points.points = points(9);
    duplicate_points.open_sets = vec![Vec::new(), points(9)];
    cases.push((duplicate_points, TopologyStatus::Unsupported));
    let mut missing = topology_request(
        record,
        TopologyOperation::ValidateTopology,
        two.clone(),
        vec![Vec::new(), two.clone()],
        None,
    );
    missing.topology = "missing-topology".into();
    cases.push((missing, TopologyStatus::Missing));
    let mut ambiguous = topology_request(
        record,
        TopologyOperation::ValidateTopology,
        two.clone(),
        vec![Vec::new(), two.clone()],
        None,
    );
    ambiguous.ambiguity = Some("independent topology identity is unresolved".into());
    cases.push((ambiguous, TopologyStatus::Ambiguous));
    let mut invalid_domain = topology_request(
        record,
        TopologyOperation::ValidateTopology,
        two,
        vec![Vec::new(), points(2)],
        None,
    );
    invalid_domain.domain.clear();
    cases.push((invalid_domain, TopologyStatus::InvalidDomain));
    cases
}

fn count_record_set(records: &SourceCatalogRecords) -> usize {
    match records {
        SourceCatalogRecords::Formula(records) => records.len(),
        SourceCatalogRecords::Relation(records) => records.len(),
        SourceCatalogRecords::Topology(records) => records.len(),
        SourceCatalogRecords::Metric(records) => records.len(),
    }
}

fn kind_name(kind: SourceCatalogKind) -> &'static str {
    match kind {
        SourceCatalogKind::Formula => "formula",
        SourceCatalogKind::Relation => "relation",
        SourceCatalogKind::Topology => "topology",
        SourceCatalogKind::Metric => "metric",
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let discovery_bytes = fs::read(DISCOVERY_REPORT)?;
    let discovery: DiscoveryReport = serde_json::from_slice(&discovery_bytes)?;
    let manifest_before = breadth_first_manifest().replay_hash();
    let mut by_kind = BTreeMap::<String, KindSummary>::new();
    let mut total = KindSummary::default();
    let mut corpus_identity = Vec::new();
    for observation in discovery
        .source_observations
        .iter()
        .filter(|observation| observation.kind.ends_with("_source_catalog"))
    {
        let bytes = fs::read(&observation.path)?;
        assert_eq!(digest_bytes(&bytes), observation.sha256);
        let text = std::str::from_utf8(&bytes)?;
        let catalog = discover_source_catalog(SourceCatalogDocument {
            path: &observation.path,
            document: text,
        })
        .map_err(|errors| format!("catalog rediscovery failed: {errors:?}"))?;
        let kind = kind_name(catalog.kind).to_string();
        let mut summary = by_kind.remove(&kind).unwrap_or_default();
        summary.catalogs += 1;
        summary.records += count_record_set(&catalog.records);
        total.catalogs += 1;
        total.records += count_record_set(&catalog.records);
        let mut run = |expected: String, actual: String, replay: bool, tamper: bool| {
            total.cases += 1;
            summary.cases += 1;
            let exact = expected == actual;
            total.exact_decisions += usize::from(exact);
            summary.exact_decisions += usize::from(exact);
            total.replay_verified += usize::from(replay);
            summary.replay_verified += usize::from(replay);
            total.tamper_rejected += usize::from(tamper);
            summary.tamper_rejected += usize::from(tamper);
            total.false_authorizations +=
                usize::from(expected != "complete" && actual == "complete");
            summary.false_authorizations +=
                usize::from(expected != "complete" && actual == "complete");
            total.false_denials += usize::from(expected == "complete" && actual != "complete");
            summary.false_denials += usize::from(expected == "complete" && actual != "complete");
            match actual.as_str() {
                "complete" => {
                    total.complete_cases += 1;
                    summary.complete_cases += 1;
                }
                "ambiguous" => {
                    total.ambiguous_cases += 1;
                    summary.ambiguous_cases += 1;
                }
                "missing" => {
                    total.missing_cases += 1;
                    summary.missing_cases += 1;
                }
                _ => {
                    total.unsupported_or_invalid_cases += 1;
                    summary.unsupported_or_invalid_cases += 1;
                }
            }
        };
        match &catalog.records {
            SourceCatalogRecords::Formula(records) => {
                for record in records {
                    for (request, expected) in formula_cases(record, &catalog.candidate.domain) {
                        let result =
                            evaluate_formula_records(&request, &catalog.candidate.domain, records);
                        let mut tampered = result.clone();
                        tampered.replay_hash.push('x');
                        run(
                            format!("{:?}", expected).to_ascii_lowercase(),
                            format!("{:?}", result.status).to_ascii_lowercase(),
                            result.replay_verified(),
                            !tampered.replay_verified(),
                        );
                    }
                }
            }
            SourceCatalogRecords::Relation(records) => {
                for record in records {
                    for (request, expected) in relation_cases(record) {
                        let result = evaluate_relation(&request, records);
                        let mut tampered = result.clone();
                        tampered.replay_hash.push('x');
                        run(
                            format!("{:?}", expected).to_ascii_lowercase(),
                            format!("{:?}", result.status).to_ascii_lowercase(),
                            result.replay_verified(),
                            !tampered.replay_verified(),
                        );
                    }
                }
            }
            SourceCatalogRecords::Topology(records) => {
                for record in records {
                    for (request, expected) in topology_cases(record) {
                        let result = evaluate_topology(&request, records);
                        let mut tampered = result.clone();
                        tampered.replay_hash.push('x');
                        run(
                            format!("{:?}", expected).to_ascii_lowercase(),
                            format!("{:?}", result.status).to_ascii_lowercase(),
                            result.replay_verified(),
                            !tampered.replay_verified(),
                        );
                    }
                }
            }
            SourceCatalogRecords::Metric(_) => {}
        }
        corpus_identity.push((
            observation.path.clone(),
            observation.sha256.clone(),
            summary.cases,
        ));
        by_kind.insert(kind, summary);
    }
    let manifest_unchanged = manifest_before == breadth_first_manifest().replay_hash();
    let independent_corpus_sha256 = digest(&corpus_identity);
    assert_eq!(total.cases, total.exact_decisions);
    assert_eq!(total.cases, total.replay_verified);
    assert_eq!(total.cases, total.tamper_rejected);
    assert_eq!(total.false_authorizations, 0);
    assert_eq!(total.false_denials, 0);
    assert!(manifest_unchanged);
    let mut report = Report {
        schema: "stage351-goal7-multiformat-pressure-v1",
        discovery_report_sha256: digest_bytes(&discovery_bytes),
        source_manifest_sha256: discovery.source_manifest_sha256,
        independent_corpus_sha256,
        catalogs: total.catalogs,
        records: total.records,
        cases: total.cases,
        exact_decisions: total.exact_decisions,
        replay_verified: total.replay_verified,
        tamper_rejected: total.tamper_rejected,
        complete_cases: total.complete_cases,
        ambiguous_cases: total.ambiguous_cases,
        missing_cases: total.missing_cases,
        unsupported_or_invalid_cases: total.unsupported_or_invalid_cases,
        false_authorizations: total.false_authorizations,
        false_denials: total.false_denials,
        production_mutations: 0,
        manifest_unchanged,
        by_kind,
        report_sha256: String::new(),
    };
    report.report_sha256 = digest(&report);
    let json = serde_json::to_string_pretty(&report)?;
    fs::write(REPORT_JSON, format!("{json}\n"))?;
    fs::write(
        REPORT_MD,
        format!(
            "# Stage 351 — independent multi-format source pressure\n\n\
* catalogs / records / cases: {} / {} / {}\n\
* exact decisions / replay / tamper rejection: {} / {} / {}\n\
* complete / ambiguous / missing / unsupported-or-invalid: {} / {} / {} / {}\n\
* false authorizations / denials: {} / {}\n\
* production mutations: {}\n\
* manifest unchanged: {}\n\
* independent corpus: `{}`\n\n\
The pressure corpus was generated from typed catalog interfaces and exercised formula, relation, and finite-topology semantics without subject-specific routing. Valid structures, missing identifiers, unresolved identities, malformed constraints, and boundedness violations were all required to preserve their declared status. Every emitted receipt replayed and every result tamper was rejected.\n",
            report.catalogs,
            report.records,
            report.cases,
            report.exact_decisions,
            report.replay_verified,
            report.tamper_rejected,
            report.complete_cases,
            report.ambiguous_cases,
            report.missing_cases,
            report.unsupported_or_invalid_cases,
            report.false_authorizations,
            report.false_denials,
            report.production_mutations,
            report.manifest_unchanged,
            report.independent_corpus_sha256,
        ),
    )?;
    println!("{json}");
    Ok(())
}
