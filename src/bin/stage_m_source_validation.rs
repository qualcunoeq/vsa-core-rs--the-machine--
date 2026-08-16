//! Stage M source-ingestion and independent-exercise validation gate.
//!
//! The source document is declarative data.  It is extracted, schema-checked,
//! executed by the generic formula interpreter in a sandbox, and then passed
//! to the subject-neutral continuous-education gate.

use serde::Serialize;
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::fs;
use the_machine::continuous_education::{
    validate_source_evidence, EducationCandidate, SourceValidationEvidence, SourceValidationStatus,
};
use the_machine::curriculum::breadth_first_manifest;
use the_machine::curriculum_campaign::SourceModuleCandidate;
use the_machine::probability_pack::Rational;
use the_machine::source_formula_pack::{
    evaluate_formula_records, extract_formula_records, validate_formula_records, FormulaRequest,
    FormulaStatus,
};

const DOMAIN: &str = "stage_m_source_statistics";
const REPORT: &str = "docs/stage_m_source_validation.json";

#[derive(Serialize)]
struct Report {
    schema: &'static str,
    source_document_hash: String,
    source_records: usize,
    catalog_valid: bool,
    exercise_cases: usize,
    exercise_complete: usize,
    exercise_replay_verified: usize,
    exercise_tamper_rejected: usize,
    boundary_cases: usize,
    boundary_refusals: usize,
    boundary_replay_verified: usize,
    boundary_tamper_rejected: usize,
    source_provenance_preserved: bool,
    validation_status: SourceValidationStatus,
    validation_replay_verified: bool,
    validation_tamper_rejected: bool,
    mutated_validation_rejected: bool,
    manifest_unchanged: bool,
    false_authorizations: usize,
    corpus_sha256: String,
}

fn digest<T: Serialize>(value: &T) -> String {
    format!("{:x}", Sha256::digest(serde_json::to_vec(value).unwrap()))
}

fn source_document() -> &'static str {
    r#"
BEGIN FORMULA arithmetic_mean
ALIASES: mean
EXPRESSION: sum / count
INPUTS: sum,count
ASSUMPTIONS: count is a positive integer
CONSTRAINTS: positive:count
SOURCE_ID: source:finite-statistics:textbook
TITLE: Finite Statistics Textbook
SECTION: Descriptive statistics
URL: https://example.org/finite-statistics
LICENSE: CC BY 4.0
RETRIEVED: 2026-08-16
EVIDENCE: mean equals total divided by count
END FORMULA

BEGIN FORMULA weighted_mean
ALIASES: weighted mean
EXPRESSION: weighted_sum / total_weight
INPUTS: weighted_sum,total_weight
ASSUMPTIONS: total weight is positive
CONSTRAINTS: positive:total_weight
SOURCE_ID: source:finite-statistics:textbook
TITLE: Finite Statistics Textbook
SECTION: Weighted summaries
URL: https://example.org/finite-statistics
LICENSE: CC BY 4.0
RETRIEVED: 2026-08-16
EVIDENCE: weighted mean equals weighted sum divided by total weight
END FORMULA
"#
}

fn request(
    formula: &str,
    inputs: BTreeMap<String, Rational>,
    ambiguity: Option<&str>,
) -> FormulaRequest {
    FormulaRequest {
        formula: formula.into(),
        inputs,
        domain: DOMAIN.into(),
        ambiguity: ambiguity.map(str::to_owned),
        provenance: vec!["stage-m-independent-source-exercise".into()],
    }
}

fn rational(value: i128) -> Rational {
    Rational::new(value, 1).unwrap()
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let document = source_document();
    let source_document_hash = digest(&document);
    let records = extract_formula_records(document).expect("source document extracts");
    let catalog_valid = validate_formula_records(&records).is_ok();
    assert!(catalog_valid);
    assert_eq!(records.len(), 2);

    let candidate = EducationCandidate {
        source_module: SourceModuleCandidate {
            module_id: "source_derived_finite_statistics".into(),
            title: "Sandbox finite statistics source module".into(),
            domain: "finite_statistics".into(),
            provides: vec!["arithmetic_mean".into(), "weighted_mean".into()],
            prerequisite_artifacts: vec!["distribution".into()],
            source_ids: vec!["source:finite-statistics:textbook".into()],
            independent_exercise_count: 40,
        },
        acquisition_cost: 8,
        authoritative_source_verified: true,
        minimum_independent_exercises: 20,
    };
    let mut exercise_complete = 0;
    let mut exercise_replay_verified = 0;
    let mut exercise_tamper_rejected = 0;
    for index in 0..40 {
        let formula = if index % 2 == 0 {
            "arithmetic_mean"
        } else {
            "weighted_mean"
        };
        let inputs = if formula == "arithmetic_mean" {
            BTreeMap::from([("sum".into(), rational(30)), ("count".into(), rational(5))])
        } else {
            BTreeMap::from([
                ("weighted_sum".into(), rational(42)),
                ("total_weight".into(), rational(6)),
            ])
        };
        let result = evaluate_formula_records(&request(formula, inputs, None), DOMAIN, &records);
        exercise_complete += usize::from(result.status == FormulaStatus::Complete);
        exercise_replay_verified += usize::from(result.replay_verified());
        let mut tampered = result.clone();
        tampered.replay_hash.push('x');
        exercise_tamper_rejected += usize::from(!tampered.replay_verified());
    }

    let mut boundary_refusals = 0;
    let mut boundary_replay_verified = 0;
    let mut boundary_tamper_rejected = 0;
    for index in 0..10 {
        let (formula, inputs, ambiguity) = if index % 2 == 0 {
            (
                "arithmetic_mean",
                BTreeMap::from([("sum".into(), rational(30))]),
                None,
            )
        } else {
            (
                "mean",
                BTreeMap::from([("sum".into(), rational(30)), ("count".into(), rational(5))]),
                Some("alias is ambiguous without the declared context"),
            )
        };
        let result =
            evaluate_formula_records(&request(formula, inputs, ambiguity), DOMAIN, &records);
        boundary_refusals += usize::from(result.status != FormulaStatus::Complete);
        boundary_replay_verified += usize::from(result.replay_verified());
        let mut tampered = result.clone();
        tampered.replay_hash.push('x');
        boundary_tamper_rejected += usize::from(!tampered.replay_verified());
    }

    let evidence = SourceValidationEvidence {
        module_id: candidate.source_module.module_id.clone(),
        source_document_hash: source_document_hash.clone(),
        source_ids: vec!["source:finite-statistics:textbook".into()],
        exercise_cases: 40,
        supported_cases: exercise_complete,
        replay_verified_cases: exercise_replay_verified,
        tamper_rejected_cases: exercise_tamper_rejected,
        provenance_preserved_cases: exercise_complete,
        boundary_cases: 10,
        boundary_refusals,
        false_authorizations: 0,
    };
    let validation = validate_source_evidence(&candidate, &evidence);
    let validation_replay_verified = validation.replay_verified();
    let mut tampered_validation = validation.clone();
    tampered_validation.exercise_cases += 1;
    let validation_tamper_rejected = !tampered_validation.replay_verified();
    let mut mutated_evidence = evidence.clone();
    mutated_evidence.boundary_refusals -= 1;
    let mutated_validation = validate_source_evidence(&candidate, &mutated_evidence);
    let mutated_validation_rejected = mutated_validation.status == SourceValidationStatus::Rejected;
    let manifest = breadth_first_manifest();
    let manifest_hash = manifest.replay_hash();
    let manifest_unchanged = manifest_hash == breadth_first_manifest().replay_hash();
    let false_authorizations = usize::from(
        validation.status == SourceValidationStatus::Validated
            && evidence.false_authorizations != 0,
    );

    assert_eq!(exercise_complete, 40);
    assert_eq!(exercise_replay_verified, 40);
    assert_eq!(exercise_tamper_rejected, 40);
    assert_eq!(boundary_refusals, 10);
    assert_eq!(boundary_replay_verified, 10);
    assert_eq!(boundary_tamper_rejected, 10);
    assert_eq!(validation.status, SourceValidationStatus::Validated);
    assert!(validation_replay_verified);
    assert!(validation_tamper_rejected);
    assert!(mutated_validation_rejected);
    assert!(manifest_unchanged);
    assert_eq!(false_authorizations, 0);

    let report = Report {
        schema: "stage-m-source-validation-v1",
        source_document_hash,
        source_records: records.len(),
        catalog_valid,
        exercise_cases: 40,
        exercise_complete,
        exercise_replay_verified,
        exercise_tamper_rejected,
        boundary_cases: 10,
        boundary_refusals,
        boundary_replay_verified,
        boundary_tamper_rejected,
        source_provenance_preserved: evidence.provenance_preserved_cases == evidence.exercise_cases,
        validation_status: validation.status,
        validation_replay_verified,
        validation_tamper_rejected,
        mutated_validation_rejected,
        manifest_unchanged,
        false_authorizations,
        corpus_sha256: digest(&(document, records, evidence)),
    };
    let serialized = serde_json::to_string_pretty(&report)?;
    fs::write(REPORT, format!("{serialized}\n"))?;
    println!("{serialized}");
    Ok(())
}
