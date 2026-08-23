//! Generic exercise generation for declarative source-formula records.
//!
//! This module knows only the typed constraint vocabulary declared by a
//! `FormulaRecord`. It does not interpret formula identifiers or inject
//! subject-specific input values. Generated exercises remain shadow evidence
//! until they pass the ordinary source and boundary gates.

use crate::probability_pack::Rational;
use crate::source_formula_pack::{
    evaluate_formula_records, FormulaRecord, FormulaRequest, FormulaResult, FormulaStatus,
    InputConstraint,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct GeneratedExercise {
    pub exercise_id: String,
    pub formula_id: String,
    pub inputs: BTreeMap<String, Rational>,
    pub expected: Rational,
    pub source_ids: Vec<String>,
    pub replay_hash: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct GeneratedBoundary {
    pub boundary_id: String,
    pub formula_id: String,
    pub inputs: BTreeMap<String, Rational>,
    pub expected_status: FormulaStatus,
    pub actual_status: FormulaStatus,
    pub source_ids: Vec<String>,
    pub replay_hash: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ExerciseGenerationReport {
    pub schema: String,
    pub expected_domain: String,
    pub source_ids: Vec<String>,
    pub requested_per_record: usize,
    pub exercises: Vec<GeneratedExercise>,
    pub boundaries: Vec<GeneratedBoundary>,
    pub replay_hash: String,
}

fn digest<T: Serialize>(value: &T) -> String {
    format!("{:x}", Sha256::digest(serde_json::to_vec(value).unwrap()))
}

fn exercise_payload(exercise: &GeneratedExercise) -> impl Serialize + '_ {
    (
        &exercise.exercise_id,
        &exercise.formula_id,
        &exercise.inputs,
        &exercise.expected,
        &exercise.source_ids,
    )
}

fn boundary_payload(boundary: &GeneratedBoundary) -> impl Serialize + '_ {
    (
        &boundary.boundary_id,
        &boundary.formula_id,
        &boundary.inputs,
        boundary.expected_status,
        boundary.actual_status,
        &boundary.source_ids,
    )
}

fn report_payload(report: &ExerciseGenerationReport) -> impl Serialize + '_ {
    (
        &report.schema,
        &report.expected_domain,
        &report.source_ids,
        report.requested_per_record,
        &report.exercises,
        &report.boundaries,
    )
}

fn candidate_values(record: &FormulaRecord, input: &str) -> Vec<Rational> {
    let mut values = vec![
        Rational::new(1, 1).unwrap(),
        Rational::new(2, 1).unwrap(),
        Rational::new(3, 1).unwrap(),
        Rational::new(5, 1).unwrap(),
        Rational::new(7, 1).unwrap(),
    ];
    for constraint in &record.constraints {
        match constraint {
            InputConstraint::PositiveInteger(name) | InputConstraint::NonnegativeInteger(name)
                if name == input =>
            {
                values = vec![
                    Rational::new(1, 1).unwrap(),
                    Rational::new(2, 1).unwrap(),
                    Rational::new(4, 1).unwrap(),
                    Rational::new(7, 1).unwrap(),
                    Rational::new(11, 1).unwrap(),
                ];
            }
            InputConstraint::Probability(name) if name == input => {
                values = vec![
                    Rational::new(1, 4).unwrap(),
                    Rational::new(1, 2).unwrap(),
                    Rational::new(3, 4).unwrap(),
                ];
            }
            _ => {}
        }
    }
    values
}

fn inputs_for(record: &FormulaRecord, attempt: usize) -> BTreeMap<String, Rational> {
    record
        .required_inputs
        .iter()
        .enumerate()
        .map(|(index, input)| {
            let values = candidate_values(record, input);
            (
                input.clone(),
                values[(attempt + index) % values.len()].clone(),
            )
        })
        .collect()
}

fn request(
    record: &FormulaRecord,
    inputs: BTreeMap<String, Rational>,
    domain: &str,
    suffix: &str,
) -> FormulaRequest {
    FormulaRequest {
        formula: record.formula_id.clone(),
        inputs,
        domain: domain.into(),
        ambiguity: None,
        provenance: vec![format!(
            "generated-source-exercise:{}:{suffix}",
            record.formula_id
        )],
    }
}

fn record_source_ids(record: &FormulaRecord) -> Vec<String> {
    vec![record.source.source_id.clone()]
}

fn boundary_input(record: &FormulaRecord, index: usize) -> Option<BTreeMap<String, Rational>> {
    let mut inputs = inputs_for(record, index);
    let (input, value) = record
        .constraints
        .iter()
        .find_map(|constraint| match constraint {
            InputConstraint::Positive(name) | InputConstraint::PositiveInteger(name) => {
                Some((name.clone(), Rational::zero()))
            }
            InputConstraint::NonnegativeInteger(name) => {
                Some((name.clone(), Rational::new(-1, 1).unwrap()))
            }
            InputConstraint::Probability(name) => {
                Some((name.clone(), Rational::new(5, 4).unwrap()))
            }
            InputConstraint::NotEqualInteger(name, forbidden) => {
                Some((name.clone(), Rational::new(*forbidden, 1).unwrap()))
            }
        })?;
    inputs.insert(input, value);
    Some(inputs)
}

/// Generate exactly `per_record` replayable supported exercises per record,
/// plus one constraint boundary whenever the record declares one.
pub fn generate_exercises(
    records: &[FormulaRecord],
    expected_domain: &str,
    per_record: usize,
) -> Result<ExerciseGenerationReport, Vec<String>> {
    if records.is_empty() || per_record == 0 || expected_domain.trim().is_empty() {
        return Err(vec![
            "records, expected domain, and positive exercise count are required".into(),
        ]);
    }
    let mut errors = Vec::new();
    let mut exercises = Vec::new();
    let mut boundaries = Vec::new();
    let mut source_ids = BTreeSet::new();
    for record in records {
        source_ids.insert(record.source.source_id.clone());
        let mut produced = 0;
        for attempt in 0..per_record.saturating_mul(20) {
            if produced == per_record {
                break;
            }
            let inputs = inputs_for(record, attempt);
            let result = evaluate_formula_records(
                &request(
                    record,
                    inputs.clone(),
                    expected_domain,
                    &format!("{attempt}"),
                ),
                expected_domain,
                records,
            );
            if result.status != FormulaStatus::Complete {
                continue;
            }
            let Some(expected) = result.value.clone() else {
                continue;
            };
            let mut exercise = GeneratedExercise {
                exercise_id: format!("{}-{:03}", record.formula_id, produced),
                formula_id: record.formula_id.clone(),
                inputs,
                expected,
                source_ids: record_source_ids(record),
                replay_hash: String::new(),
            };
            let replay_hash = digest(&exercise_payload(&exercise));
            exercise.replay_hash = replay_hash;
            exercises.push(exercise);
            produced += 1;
        }
        if produced != per_record {
            errors.push(format!(
                "record {} produced {produced}/{per_record} supported exercises",
                record.formula_id
            ));
        }
        if let Some(inputs) = boundary_input(record, 0) {
            let boundary_request = request(record, inputs.clone(), expected_domain, "boundary");
            let result: FormulaResult =
                evaluate_formula_records(&boundary_request, expected_domain, records);
            let mut boundary = GeneratedBoundary {
                boundary_id: format!("{}-boundary", record.formula_id),
                formula_id: record.formula_id.clone(),
                inputs,
                expected_status: FormulaStatus::Inconsistent,
                actual_status: result.status,
                source_ids: record_source_ids(record),
                replay_hash: String::new(),
            };
            let replay_hash = digest(&boundary_payload(&boundary));
            boundary.replay_hash = replay_hash;
            boundaries.push(boundary);
        }
    }
    if errors.is_empty() {
        let mut report = ExerciseGenerationReport {
            schema: "source-exercise-generation-v1".into(),
            expected_domain: expected_domain.into(),
            source_ids: source_ids.into_iter().collect(),
            requested_per_record: per_record,
            exercises,
            boundaries,
            replay_hash: String::new(),
        };
        let replay_hash = digest(&report_payload(&report));
        report.replay_hash = replay_hash;
        Ok(report)
    } else {
        Err(errors)
    }
}

pub fn exercise_replay_verified(exercise: &GeneratedExercise) -> bool {
    exercise.replay_hash == digest(&exercise_payload(exercise))
        && !exercise.source_ids.is_empty()
        && !exercise.expected.denominator.eq(&0)
}

pub fn boundary_replay_verified(boundary: &GeneratedBoundary) -> bool {
    boundary.replay_hash == digest(&boundary_payload(boundary))
        && boundary.actual_status != FormulaStatus::Complete
        && !boundary.source_ids.is_empty()
}

pub fn replay_verified(report: &ExerciseGenerationReport) -> bool {
    report.replay_hash == digest(&report_payload(report))
        && report.exercises.iter().all(exercise_replay_verified)
        && report.boundaries.iter().all(boundary_replay_verified)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::source_formula_pack::source_formula_records;

    #[test]
    fn generates_replayable_source_exercises_and_boundaries() {
        let records = source_formula_records();
        let report = generate_exercises(&records, "source_derived_sequences_series", 8).unwrap();
        assert_eq!(report.exercises.len(), records.len() * 8);
        assert_eq!(report.boundaries.len(), records.len());
        assert!(replay_verified(&report));
    }

    #[test]
    fn rejects_empty_generation_requests() {
        assert!(generate_exercises(&[], "domain", 1).is_err());
        assert!(generate_exercises(&source_formula_records(), "domain", 0).is_err());
    }
}
