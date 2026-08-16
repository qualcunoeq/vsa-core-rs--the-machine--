//! Generic source-catalog ingestion gate.
//!
//! The catalog is data loaded from a cited artifact.  This benchmark mutates
//! copies to verify that malformed identities, expressions, constraints, and
//! citations are rejected before the generic interpreter can execute them.

use serde::Serialize;
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use the_machine::probability_pack::Rational;
use the_machine::source_formula_pack::{
    evaluate_formula_records, validate_formula_records, Expr, FormulaRecord, FormulaRequest,
    FormulaStatus, InputConstraint, SourceCitation,
};
use the_machine::source_statistics_pack::{records, DOMAIN};

#[derive(Serialize)]
struct Report {
    schema: &'static str,
    source: &'static str,
    catalog_sha256: String,
    record_count: usize,
    valid_catalogs: usize,
    mutation_cases: usize,
    mutation_rejections: usize,
    generated_exercises: usize,
    generated_exercises_complete: usize,
    generated_exercise_replays: usize,
    evidence_spans_preserved: usize,
    replay_stable: bool,
    false_acceptances: usize,
}

fn digest<T: Serialize>(value: &T) -> String {
    format!("{:x}", Sha256::digest(serde_json::to_vec(value).unwrap()))
}

fn generated_inputs(record: &FormulaRecord) -> BTreeMap<String, Rational> {
    let mut inputs = BTreeMap::new();
    for name in &record.required_inputs {
        inputs.insert(name.clone(), Rational::new(2, 1).unwrap());
    }
    for constraint in &record.constraints {
        let (name, value) = match constraint {
            InputConstraint::Positive(name) => (name, Rational::new(3, 1).unwrap()),
            InputConstraint::PositiveInteger(name) => (name, Rational::new(4, 1).unwrap()),
            InputConstraint::NonnegativeInteger(name) => (name, Rational::new(4, 1).unwrap()),
            InputConstraint::Probability(name) => (name, Rational::new(1, 4).unwrap()),
            InputConstraint::NotEqualInteger(name, forbidden) => {
                (name, Rational::new(forbidden + 1, 1).unwrap())
            }
        };
        inputs.insert(name.clone(), value);
    }
    inputs
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let catalog = records();
    let catalog_sha256 = digest(&catalog);
    let valid_catalogs = usize::from(validate_formula_records(&catalog).is_ok());
    let mut mutations: Vec<Vec<FormulaRecord>> = Vec::new();

    let mut duplicate_id = catalog.clone();
    duplicate_id[1].formula_id = duplicate_id[0].formula_id.clone();
    mutations.push(duplicate_id);

    let mut undeclared_input = catalog.clone();
    undeclared_input[0].expression = Expr::Input("not_declared".into());
    mutations.push(undeclared_input);

    let mut duplicate_alias = catalog.clone();
    let alias = duplicate_alias[0].aliases[0].clone();
    duplicate_alias[1].aliases.push(alias);
    mutations.push(duplicate_alias);

    let mut undeclared_constraint = catalog.clone();
    undeclared_constraint[0]
        .constraints
        .push(InputConstraint::Positive("not_declared".into()));
    mutations.push(undeclared_constraint);

    let mut bad_citation = catalog.clone();
    bad_citation[0].source = SourceCitation {
        source_id: String::new(),
        ..bad_citation[0].source.clone()
    };
    mutations.push(bad_citation);

    let mutation_rejections = mutations
        .iter()
        .filter(|candidate| validate_formula_records(candidate).is_err())
        .count();
    let generated_exercises = catalog.len();
    let evidence_spans_preserved = catalog
        .iter()
        .filter(|record| !record.source.evidence_span.is_empty())
        .count();
    let mut generated_exercises_complete = 0;
    let mut generated_exercise_replays = 0;
    for record in &catalog {
        let request = FormulaRequest {
            formula: record.formula_id.clone(),
            inputs: generated_inputs(record),
            domain: DOMAIN.into(),
            ambiguity: None,
            provenance: vec!["generic-constraint-exercise-generator".into()],
        };
        let result = evaluate_formula_records(&request, DOMAIN, &catalog);
        if result.status == FormulaStatus::Complete && result.value.is_some() {
            generated_exercises_complete += 1;
        }
        if result.replay_verified() {
            generated_exercise_replays += 1;
        }
    }
    let replay_stable = digest(&records()) == catalog_sha256;
    let false_acceptances = mutations.len() - mutation_rejections;
    assert_eq!(valid_catalogs, 1);
    assert_eq!(catalog.len(), 5);
    assert_eq!(mutations.len(), 5);
    assert_eq!(mutation_rejections, 5);
    assert_eq!(generated_exercises_complete, generated_exercises);
    assert_eq!(generated_exercise_replays, generated_exercises);
    assert_eq!(evidence_spans_preserved, catalog.len());
    assert!(replay_stable);
    assert_eq!(false_acceptances, 0);

    let report = Report {
        schema: "stage-d-source-catalog-ingestion-v1",
        source: "OpenStax finite-statistics catalog artifact",
        catalog_sha256,
        record_count: catalog.len(),
        valid_catalogs,
        mutation_cases: mutations.len(),
        mutation_rejections,
        generated_exercises,
        generated_exercises_complete,
        generated_exercise_replays,
        evidence_spans_preserved,
        replay_stable,
        false_acceptances,
    };
    let serialized = serde_json::to_string_pretty(&report)?;
    std::fs::write(
        "docs/stage_d_source_catalog_ingestion.json",
        format!("{serialized}\n"),
    )?;
    println!("{serialized}");
    Ok(())
}
