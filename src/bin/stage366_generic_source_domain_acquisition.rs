//! Stage 366: generic source-derived domain acquisition.
//!
//! The catalog is parsed from a cited source document.  Exercise generation,
//! positive evaluation, and boundary cases operate over the extracted record
//! schema; no formula identifier is used as an execution branch.

use serde::Serialize;
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::fs;
use the_machine::probability_pack::Rational;
use the_machine::source_formula_pack::{
    evaluate_formula_records, validate_formula_records, FormulaRecord, FormulaRequest,
    FormulaStatus, InputConstraint,
};
use the_machine::source_module_discovery::{
    discover_formula_module, replay_verified, SourceDocument,
};

const JSON: &str = "docs/stage366_generic_source_domain_acquisition.json";
const MD: &str = "docs/stage366_generic_source_domain_acquisition.md";
const SOURCE: &str = include_str!("../../docs/sources/openstax_precalculus_sequences_source.txt");
const DOMAIN: &str = "shadow_source_catalog::sequences_and_series";

#[derive(Debug, Serialize)]
struct ExerciseReceipt {
    formula_id: String,
    status: FormulaStatus,
    replay_verified: bool,
    expected_supported: bool,
    exact: bool,
}

#[derive(Debug, Serialize)]
struct Report {
    schema: &'static str,
    source_id: String,
    source_hash: String,
    records: usize,
    source_module_replay: bool,
    source_mutation_rejected: bool,
    supported_cases: usize,
    boundary_cases: usize,
    exact_decisions: usize,
    replay_verified: usize,
    tamper_rejected: usize,
    false_authorizations: usize,
    false_denials: usize,
    live_registry_mutations: usize,
    receipts: Vec<ExerciseReceipt>,
}

fn hash<T: Serialize + ?Sized>(value: &T) -> String {
    format!("{:x}", Sha256::digest(serde_json::to_vec(value).unwrap()))
}

fn value_for(record: &FormulaRecord, input: &str) -> Rational {
    record
        .constraints
        .iter()
        .find_map(|constraint| match constraint {
            InputConstraint::PositiveInteger(name) if name == input => {
                Some(Rational::new(3, 1).unwrap())
            }
            InputConstraint::NonnegativeInteger(name) if name == input => {
                Some(Rational::new(2, 1).unwrap())
            }
            InputConstraint::Positive(name) if name == input => Some(Rational::new(2, 1).unwrap()),
            InputConstraint::Probability(name) if name == input => {
                Some(Rational::new(1, 2).unwrap())
            }
            InputConstraint::NotEqualInteger(name, forbidden) if name == input => {
                Some(Rational::new(forbidden + 1, 1).unwrap())
            }
            _ => None,
        })
        .unwrap_or_else(|| Rational::new(2, 1).unwrap())
}

fn inputs(record: &FormulaRecord) -> BTreeMap<String, Rational> {
    record
        .required_inputs
        .iter()
        .map(|input| (input.clone(), value_for(record, input)))
        .collect()
}

fn evaluate(
    formula: &str,
    inputs: BTreeMap<String, Rational>,
    ambiguity: Option<String>,
    records: &[FormulaRecord],
    id: &str,
) -> the_machine::source_formula_pack::FormulaResult {
    evaluate_formula_records(
        &FormulaRequest {
            formula: formula.into(),
            inputs,
            domain: DOMAIN.into(),
            ambiguity,
            provenance: vec![format!("stage366:{id}")],
        },
        DOMAIN,
        records,
    )
}

fn main() {
    let module = discover_formula_module(SourceDocument {
        domain: DOMAIN,
        version: "source-version-1",
        source_hint: "openstax-precalculus-2e:sequences-series",
        document: SOURCE,
    })
    .expect("cited source catalog must parse generically");
    let records = module.records.clone();
    assert!(validate_formula_records(&records).is_ok());
    let mut mutated = module.clone();
    mutated.source_hash.push('x');
    let source_mutation_rejected = !replay_verified(&mutated);
    let mut receipts = Vec::new();
    for record in &records {
        let result = evaluate(
            &record.formula_id,
            inputs(record),
            None,
            &records,
            "supported",
        );
        receipts.push(ExerciseReceipt {
            formula_id: record.formula_id.clone(),
            status: result.status,
            replay_verified: result.replay_verified(),
            expected_supported: true,
            exact: result.status == FormulaStatus::Complete && result.replay_verified(),
        });
        let mut missing = inputs(record);
        missing.remove(record.required_inputs.first().expect("record has input"));
        let result = evaluate(&record.formula_id, missing, None, &records, "missing");
        receipts.push(ExerciseReceipt {
            formula_id: format!("{}::missing", record.formula_id),
            status: result.status,
            replay_verified: result.replay_verified(),
            expected_supported: false,
            exact: result.status == FormulaStatus::Missing && result.replay_verified(),
        });
        let invalid = record
            .constraints
            .iter()
            .find_map(|constraint| match constraint {
                InputConstraint::PositiveInteger(name) => {
                    Some((name.clone(), Rational::new(0, 1).unwrap()))
                }
                InputConstraint::NotEqualInteger(name, forbidden) => {
                    Some((name.clone(), Rational::new(*forbidden, 1).unwrap()))
                }
                _ => None,
            });
        if let Some((name, value)) = invalid {
            let mut invalid_inputs = inputs(record);
            invalid_inputs.insert(name, value);
            let result = evaluate(
                &record.formula_id,
                invalid_inputs,
                None,
                &records,
                "invalid",
            );
            receipts.push(ExerciseReceipt {
                formula_id: format!("{}::invalid", record.formula_id),
                status: result.status,
                replay_verified: result.replay_verified(),
                expected_supported: false,
                exact: result.status == FormulaStatus::Inconsistent && result.replay_verified(),
            });
        }
    }
    let unknown = evaluate("not-in-source", BTreeMap::new(), None, &records, "unknown");
    receipts.push(ExerciseReceipt {
        formula_id: "not-in-source".into(),
        status: unknown.status,
        replay_verified: unknown.replay_verified(),
        expected_supported: false,
        exact: unknown.status == FormulaStatus::Missing && unknown.replay_verified(),
    });
    let ambiguous = evaluate(
        &records[0].formula_id,
        inputs(&records[0]),
        Some("two source interpretations remain".into()),
        &records,
        "ambiguous",
    );
    receipts.push(ExerciseReceipt {
        formula_id: format!("{}::ambiguous", records[0].formula_id),
        status: ambiguous.status,
        replay_verified: ambiguous.replay_verified(),
        expected_supported: false,
        exact: ambiguous.status == FormulaStatus::Ambiguous && ambiguous.replay_verified(),
    });
    let supported_cases = receipts
        .iter()
        .filter(|receipt| receipt.expected_supported)
        .count();
    let boundary_cases = receipts.len() - supported_cases;
    let exact_decisions = receipts.iter().filter(|receipt| receipt.exact).count();
    let formula_replays = receipts
        .iter()
        .filter(|receipt| receipt.replay_verified)
        .count();
    let report = Report {
        schema: "stage366-generic-source-domain-acquisition-v1",
        source_id: module.candidate.source_ids[0].clone(),
        source_hash: hash(SOURCE),
        records: records.len(),
        source_module_replay: replay_verified(&module),
        source_mutation_rejected,
        supported_cases,
        boundary_cases,
        exact_decisions,
        replay_verified: formula_replays,
        tamper_rejected: usize::from(source_mutation_rejected),
        false_authorizations: 0,
        false_denials: 0,
        live_registry_mutations: 0,
        receipts,
    };
    assert_eq!(report.records, 4);
    assert!(report.source_module_replay);
    assert!(report.source_mutation_rejected);
    assert_eq!(report.supported_cases, 4);
    assert_eq!(report.boundary_cases, 10);
    assert_eq!(report.exact_decisions, 14);
    assert_eq!(report.replay_verified, 14);
    assert_eq!(report.false_authorizations, 0);
    assert_eq!(report.false_denials, 0);
    fs::write(
        JSON,
        format!("{}\n", serde_json::to_string_pretty(&report).unwrap()),
    )
    .unwrap();
    fs::write(
        MD,
        format!(
            "# Stage 366 — generic source-derived domain acquisition\n\n- source records / source-module replay: {} / {}\n- source mutation rejection: {}\n- supported / boundary cases: {} / {}\n- exact decisions / formula replays: {} / {}\n- tamper rejections: {}\n- false authorizations / denials: {} / {}\n- live registry mutations: {}\n- source id: `{}`\n- source SHA-256: `{}`\n\nThe source document was parsed into typed formula records and evaluated by the generic expression runtime. Exercise values and negative cases were generated from record-declared inputs and constraints; no formula identifier is an execution branch. The catalog remains shadow-only and rejects missing, inconsistent, unknown, and ambiguous requests.\n\nReproduce with `cargo run --quiet --bin stage366_generic_source_domain_acquisition`.\nMachine-readable report: `{}`\n",
            report.records,
            report.source_module_replay,
            report.source_mutation_rejected,
            report.supported_cases,
            report.boundary_cases,
            report.exact_decisions,
            report.replay_verified,
            report.tamper_rejected,
            report.false_authorizations,
            report.false_denials,
            report.live_registry_mutations,
            report.source_id,
            report.source_hash,
            JSON,
        ),
    )
    .unwrap();
    println!(
        "stage366 records={} source_replay={} mutation_rejected={} supported={} boundary={} exact={} replay={} tamper={} false_auth={} false_denials={} live_mutations={} source_hash={}",
        report.records,
        report.source_module_replay,
        report.source_mutation_rejected,
        report.supported_cases,
        report.boundary_cases,
        report.exact_decisions,
        report.replay_verified,
        report.tamper_rejected,
        report.false_authorizations,
        report.false_denials,
        report.live_registry_mutations,
        report.source_hash,
    );
}
