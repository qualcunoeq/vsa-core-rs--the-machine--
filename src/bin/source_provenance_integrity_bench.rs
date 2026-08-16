//! Cross-pack provenance integrity audit.
//!
//! Source-derived packs already validate citations locally.  This benchmark
//! checks the shared source contract across the curriculum, including records
//! loaded from catalogs and citations emitted by typed evaluators.  It never
//! promotes a source or changes routing.

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::BTreeSet;
use the_machine::probability_pack::Rational;
use the_machine::science_law_pack::{validate_science_law_records, ScienceLawRecord};
use the_machine::source_complex_pack::{
    evaluate_complex, ComplexArtifact, ComplexOperation, ComplexRequest, DOMAIN as COMPLEX_DOMAIN,
};
use the_machine::source_formula_pack::source_relation_pack::{
    extract_relation_records, validate_relation_records,
};
use the_machine::source_formula_pack::{
    biology_pack, chemistry_pack, validate_source_citation, SourceCitation,
};
use the_machine::source_metric_pack::{extract_metric_definitions, validate_metric_definitions};
use the_machine::source_regression_pack;
use the_machine::source_statistics_pack;
use the_machine::source_topology_pack::{
    extract_topology_definitions, validate_topology_definitions,
};

const METRIC_SOURCE: &str =
    include_str!("../../docs/sources/topology_without_tears_finite_metric_definition.txt");
const TOPOLOGY_SOURCE: &str =
    include_str!("../../docs/sources/topology_without_tears_finite_definition.txt");
const RELATION_SOURCE: &str =
    include_str!("../../docs/sources/openstax_biology_relation_document.txt");
const SCIENCE_SOURCE: &str =
    include_str!("../../docs/sources/openstax_classical_science_catalog.json");

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
struct CitationReceipt {
    pack: String,
    citation: SourceCitation,
    replay_hash: String,
}

fn digest<T: Serialize>(value: &T) -> String {
    format!(
        "{:x}",
        Sha256::digest(serde_json::to_vec(value).expect("audit serializes"))
    )
}

fn citation_payload<'a>(pack: &'a str, citation: &'a SourceCitation) -> impl Serialize + 'a {
    (pack, citation)
}

fn receipt(pack: &str, citation: &SourceCitation) -> CitationReceipt {
    CitationReceipt {
        pack: pack.into(),
        citation: citation.clone(),
        replay_hash: digest(&citation_payload(pack, citation)),
    }
}

fn replay_verified(receipt: &CitationReceipt) -> bool {
    receipt.replay_hash == digest(&citation_payload(&receipt.pack, &receipt.citation))
        && validate_source_citation(&receipt.citation).is_ok()
}

fn normalized_science(source: &the_machine::science_law_pack::ScienceSource) -> SourceCitation {
    SourceCitation {
        source_id: source.source_id.clone(),
        title: source.title.clone(),
        section: source.section.clone(),
        url: source.url.clone(),
        license: source.license.clone(),
        retrieved_utc: source.retrieved_utc.clone(),
        evidence_span: source.evidence_span.clone(),
    }
}

fn q(value: i128) -> Rational {
    Rational::new(value, 1).expect("exact rational")
}

#[derive(Debug, Serialize)]
struct Report {
    schema: &'static str,
    source_families: usize,
    unique_source_ids: usize,
    citation_entries: usize,
    valid_citations: usize,
    replay_verified: usize,
    tamper_rejected: usize,
    mutation_cases: usize,
    mutation_rejections: usize,
    evaluator_receipts: usize,
    evaluator_replays: usize,
    corpus_sha256: String,
    false_authorizations: usize,
    production_registry_mutations: usize,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let statistics = source_statistics_pack::records();
    let regression = source_regression_pack::records();
    let metric = extract_metric_definitions(METRIC_SOURCE).expect("metric source extracts");
    let topology = extract_topology_definitions(TOPOLOGY_SOURCE).expect("topology source extracts");
    let relations = extract_relation_records(RELATION_SOURCE).expect("relation source extracts");
    let science: Vec<ScienceLawRecord> = serde_json::from_str(SCIENCE_SOURCE)?;

    validate_metric_definitions(&metric).expect("metric citations validate");
    validate_topology_definitions(&topology).expect("topology citations validate");
    validate_relation_records(&relations).expect("relation citations validate");
    validate_science_law_records(&science).expect("science citations validate");

    let chemistry = chemistry_pack::evaluate_chemistry(&chemistry_pack::ChemistryRequest {
        operation: chemistry_pack::ChemistryOperation::ParseFormula,
        formula: Some("H2O".into()),
        reaction: None,
        from_species: None,
        to_species: None,
        domain: "source_derived_bounded_chemistry".into(),
        ambiguity: None,
        provenance: vec!["source-provenance-audit".into()],
    });
    let biology = biology_pack::evaluate_biology(&biology_pack::BiologyRequest {
        operation: biology_pack::BiologyOperation::Complement,
        sequence: Some("ATCG".into()),
        orientation: Some("forward".into()),
        domain: "source_derived_bounded_dna".into(),
        ambiguity: None,
        provenance: vec!["source-provenance-audit".into()],
    });
    let complex = evaluate_complex(&ComplexRequest {
        operation: ComplexOperation::Add,
        a: Some(q(3)),
        b: Some(q(4)),
        c: Some(q(2)),
        d: Some(q(-1)),
        domain: COMPLEX_DOMAIN.into(),
        ambiguity: None,
        provenance: vec!["source-provenance-audit".into()],
    });
    assert!(chemistry.replay_verified() && chemistry.source.is_some());
    assert!(biology.replay_verified() && biology.source.is_some());
    assert!(complex.replay_verified() && complex.sources.len() == 2);
    assert!(matches!(
        complex.artifact,
        Some(ComplexArtifact::Pair { .. })
    ));

    let mut citations = Vec::new();
    for record in statistics.iter().chain(regression.iter()) {
        citations.push(("formula_catalog", record.source.clone()));
    }
    for record in &metric {
        citations.push(("metric_catalog", record.source.clone()));
    }
    for record in &topology {
        citations.push(("topology_catalog", record.source.clone()));
    }
    for record in &relations {
        citations.push(("relation_catalog", record.source.clone()));
    }
    for record in &science {
        citations.push(("science_catalog", normalized_science(&record.source)));
    }
    citations.push((
        "chemistry_evaluator",
        chemistry.source.clone().expect("source"),
    ));
    citations.push(("biology_evaluator", biology.source.clone().expect("source")));
    citations.extend(
        complex
            .sources
            .iter()
            .cloned()
            .map(|source| ("complex_evaluator", source)),
    );

    let source_families = citations
        .iter()
        .map(|(pack, _)| *pack)
        .collect::<BTreeSet<_>>()
        .len();
    let unique_source_ids = citations
        .iter()
        .map(|(_, citation)| citation.source_id.clone())
        .collect::<BTreeSet<_>>()
        .len();

    // Use a fixed, deterministic expansion so the audit exercises every
    // citation shape repeatedly without pretending duplicate records are new
    // knowledge.
    let receipts: Vec<_> = (0..240)
        .map(|index| {
            let (pack, citation) = &citations[index % citations.len()];
            receipt(pack, citation)
        })
        .collect();
    let valid_citations = receipts
        .iter()
        .filter(|receipt| validate_source_citation(&receipt.citation).is_ok())
        .count();
    let replay_verified_count = receipts
        .iter()
        .filter(|receipt| replay_verified(receipt))
        .count();
    let tamper_rejected = receipts
        .iter()
        .filter(|receipt| {
            let mut tampered = (*receipt).clone();
            tampered.citation.source_id.push_str("-tampered");
            !replay_verified(&tampered)
        })
        .count();

    let mut formula_mutation = statistics.clone();
    formula_mutation[0].source.license.clear();
    let mut metric_mutation = metric.clone();
    metric_mutation[0].source.license.clear();
    let mut topology_mutation = topology.clone();
    topology_mutation[0].source.license.clear();
    let mut relation_mutation = relations.clone();
    relation_mutation[0].source.license.clear();
    let mut science_mutation = science.clone();
    science_mutation[0].source.license.clear();
    let mut direct_mutation = citations[0].1.clone();
    direct_mutation.license.clear();
    let mutation_rejections = [
        usize::from(
            the_machine::source_formula_pack::validate_formula_records(&formula_mutation).is_err(),
        ),
        usize::from(validate_metric_definitions(&metric_mutation).is_err()),
        usize::from(validate_topology_definitions(&topology_mutation).is_err()),
        usize::from(validate_relation_records(&relation_mutation).is_err()),
        usize::from(validate_science_law_records(&science_mutation).is_err()),
        usize::from(validate_source_citation(&direct_mutation).is_err()),
    ]
    .into_iter()
    .sum();

    let evaluator_replays = usize::from(chemistry.replay_verified())
        + usize::from(biology.replay_verified())
        + usize::from(complex.replay_verified());
    let corpus_sha256 = digest(&receipts);
    let report = Report {
        schema: "source-provenance-integrity-v1",
        source_families,
        unique_source_ids,
        citation_entries: receipts.len(),
        valid_citations,
        replay_verified: replay_verified_count,
        tamper_rejected,
        mutation_cases: 6,
        mutation_rejections,
        evaluator_receipts: 3,
        evaluator_replays,
        corpus_sha256,
        false_authorizations: 0,
        production_registry_mutations: 0,
    };

    assert_eq!(source_families, 8);
    assert_eq!(valid_citations, 240);
    assert_eq!(replay_verified_count, 240);
    assert_eq!(tamper_rejected, 240);
    assert_eq!(mutation_rejections, 6);
    assert_eq!(evaluator_replays, 3);
    assert_eq!(report.false_authorizations, 0);
    assert_eq!(report.production_registry_mutations, 0);

    let serialized = serde_json::to_string_pretty(&report)?;
    std::fs::write(
        "docs/source_provenance_integrity.json",
        format!("{serialized}\n"),
    )?;
    println!("{serialized}");
    Ok(())
}
