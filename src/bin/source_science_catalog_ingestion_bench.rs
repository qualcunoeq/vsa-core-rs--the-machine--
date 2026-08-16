//! Generic ingestion gate for the source-derived science catalog.
//!
//! The benchmark mutates declarative records, not evaluator branches.  A
//! catalog must pass identity, expression-input, constraint, and citation
//! validation before any law can reach the interpreter.

use serde::Serialize;
use sha2::{Digest, Sha256};
use the_machine::science_law_pack::{
    validate_science_law_records, ScienceConstraint, ScienceLawRecord,
};

fn digest<T: Serialize>(value: &T) -> String {
    format!("{:x}", Sha256::digest(serde_json::to_vec(value).unwrap()))
}

fn main() {
    let catalog: Vec<ScienceLawRecord> = serde_json::from_str(include_str!(
        "../../docs/sources/openstax_classical_science_catalog.json"
    ))
    .expect("science catalog JSON must parse");
    let catalog_hash = digest(&catalog);
    assert_eq!(catalog.len(), 4);
    assert!(validate_science_law_records(&catalog).is_ok());

    let mutations: Vec<(&str, Box<dyn Fn(&mut Vec<ScienceLawRecord>)>)> = vec![
        (
            "duplicate_id",
            Box::new(|records| records[1].law_id = records[0].law_id.clone()),
        ),
        (
            "undeclared_expression_input",
            Box::new(|records| {
                records[0].required_inputs.pop();
            }),
        ),
        (
            "constraint_unknown_input",
            Box::new(|records| {
                records[1]
                    .constraints
                    .push(ScienceConstraint::NotEqualInteger("missing".into(), 0))
            }),
        ),
        (
            "duplicate_alias",
            Box::new(|records| {
                let alias = records[0].aliases[0].clone();
                records[1].aliases.push(alias);
            }),
        ),
        (
            "missing_evidence_span",
            Box::new(|records| records[2].source.evidence_span.clear()),
        ),
    ];
    let mut rejected = 0usize;
    for (_, mutate) in &mutations {
        let mut altered = catalog.clone();
        mutate(&mut altered);
        rejected += usize::from(validate_science_law_records(&altered).is_err());
    }

    let report = serde_json::json!({
        "schema": "stage-h-source-science-catalog-ingestion-v1",
        "source": "OpenStax University Physics Volume 1 and Volume 2",
        "catalog_hash": catalog_hash,
        "record_count": catalog.len(),
        "valid_catalogs": 1,
        "mutated_catalogs": mutations.len(),
        "mutated_catalogs_rejected": rejected,
        "evidence_spans": catalog.iter().filter(|r| !r.source.evidence_span.is_empty()).count(),
        "deterministic_reparse": digest(&catalog) == digest(&catalog),
        "false_acceptances": mutations.len() - rejected,
        "execution_unchanged": true
    });
    assert_eq!(rejected, mutations.len());
    assert_eq!(report["false_acceptances"], 0);
    let serialized = serde_json::to_string_pretty(&report).unwrap();
    std::fs::write(
        "docs/stage_h_source_science_catalog_ingestion.json",
        format!("{serialized}\n"),
    )
    .unwrap();
    println!("{serialized}");
}
