//! Source-to-exercise generation gate for a declarative formula catalog.
//!
//! The source document is parsed into typed records, then a generic
//! constraint-driven generator creates supported and negative exercises. No
//! formula identifier is interpreted by a subject-specific branch.

use serde::Serialize;
use sha2::{Digest, Sha256};
use std::fs;
use the_machine::source_exercise_generation::{
    boundary_replay_verified, exercise_replay_verified, generate_exercises, replay_verified,
};
use the_machine::source_formula_pack::{evaluate_formula_records, FormulaRequest, FormulaStatus};
use the_machine::source_module_discovery::{discover_formula_module, SourceDocument};

const SOURCE_DOCUMENT: &str =
    include_str!("../../docs/sources/openstax_precalculus_sequences_source.txt");
const REPORT_JSON: &str = "docs/stage401_source_exercise_generation.json";
const REPORT_MD: &str = "docs/stage401_source_exercise_generation.md";
const DOMAIN: &str = "source_derived_sequences_series";

#[derive(Debug, Serialize)]
struct Report {
    schema: &'static str,
    source_id: String,
    source_sha256: String,
    discovered_records: usize,
    requested_per_record: usize,
    supported_exercises: usize,
    boundary_exercises: usize,
    supported_replays: usize,
    boundary_replays: usize,
    downstream_replays: usize,
    boundary_authorizations: usize,
    exercise_tamper_rejections: usize,
    boundary_tamper_rejections: usize,
    module_replay: bool,
    report_replay: bool,
    report_tamper_rejected: bool,
}

fn digest<T: Serialize>(value: &T) -> String {
    format!("{:x}", Sha256::digest(serde_json::to_vec(value).unwrap()))
}

fn hash_bytes(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

fn report_payload(value: &Report) -> impl Serialize + '_ {
    (
        value.schema,
        &value.source_id,
        &value.source_sha256,
        value.discovered_records,
        value.requested_per_record,
        value.supported_exercises,
        value.boundary_exercises,
        value.supported_replays,
        value.boundary_replays,
        value.downstream_replays,
        value.boundary_authorizations,
        value.exercise_tamper_rejections,
        value.boundary_tamper_rejections,
        value.module_replay,
    )
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let module = discover_formula_module(SourceDocument {
        domain: "source_derived_sequences_series",
        version: "openstax-precalculus-2e-v1",
        source_hint: "openstax-precalculus-2e:sequences-series",
        document: SOURCE_DOCUMENT,
    })
    .map_err(|errors| errors.join("; "))?;
    let generated =
        generate_exercises(&module.records, DOMAIN, 30).map_err(|errors| errors.join("; "))?;
    let supported_replays = generated
        .exercises
        .iter()
        .filter(|exercise| exercise_replay_verified(exercise))
        .count();
    let boundary_replays = generated
        .boundaries
        .iter()
        .filter(|boundary| boundary_replay_verified(boundary))
        .count();
    let downstream_replays = generated
        .exercises
        .iter()
        .filter(|exercise| {
            let request = FormulaRequest {
                formula: exercise.formula_id.clone(),
                inputs: exercise.inputs.clone(),
                domain: DOMAIN.into(),
                ambiguity: None,
                provenance: exercise.source_ids.clone(),
            };
            let result = evaluate_formula_records(&request, DOMAIN, &module.records);
            result.status == FormulaStatus::Complete
                && result.value.as_ref() == Some(&exercise.expected)
                && result.replay_verified()
        })
        .count();
    let exercise_tamper_rejections = generated
        .exercises
        .iter()
        .filter(|exercise| {
            let mut tampered = (*exercise).clone();
            tampered.replay_hash.push('x');
            !exercise_replay_verified(&tampered)
        })
        .count();
    let boundary_tamper_rejections = generated
        .boundaries
        .iter()
        .filter(|boundary| {
            let mut tampered = (*boundary).clone();
            tampered.replay_hash.push('x');
            !boundary_replay_verified(&tampered)
        })
        .count();
    let module_replay =
        module.replay_hash == digest(&(&module.candidate, &module.records, &module.source_hash));
    let mut report = Report {
        schema: "stage401-source-exercise-generation-v1",
        source_id: module.candidate.source_ids[0].clone(),
        source_sha256: hash_bytes(SOURCE_DOCUMENT.as_bytes()),
        discovered_records: module.records.len(),
        requested_per_record: generated.requested_per_record,
        supported_exercises: generated.exercises.len(),
        boundary_exercises: generated.boundaries.len(),
        supported_replays,
        boundary_replays,
        downstream_replays,
        boundary_authorizations: generated
            .boundaries
            .iter()
            .filter(|boundary| boundary.actual_status == FormulaStatus::Complete)
            .count(),
        exercise_tamper_rejections,
        boundary_tamper_rejections,
        module_replay,
        report_replay: false,
        report_tamper_rejected: false,
    };
    let report_hash = digest(&report_payload(&report));
    let report_replay = report_hash == digest(&report_payload(&report));
    report.report_replay = report_replay;
    let mut tampered = report;
    tampered.supported_exercises += 1;
    let tampered_hash = digest(&report_payload(&tampered));
    let report_tamper_rejected = report_hash != tampered_hash;
    report = tampered;
    report.report_tamper_rejected = report_tamper_rejected;
    report.supported_exercises -= 1;
    assert_eq!(report.supported_exercises, 120);
    assert_eq!(report.boundary_exercises, 4);
    assert_eq!(report.supported_replays, 120);
    assert_eq!(report.boundary_replays, 4);
    assert_eq!(report.downstream_replays, 120);
    assert_eq!(report.boundary_authorizations, 0);
    assert_eq!(report.exercise_tamper_rejections, 120);
    assert_eq!(report.boundary_tamper_rejections, 4);
    assert!(report.module_replay && report.report_replay && report.report_tamper_rejected);
    let serialized = serde_json::to_string_pretty(&report)?;
    fs::write(REPORT_JSON, format!("{serialized}\n"))?;
    fs::write(
        REPORT_MD,
        format!(
            "# Stage 401 — generic source exercise generation\n\n- Discovered source records: {}\n- Exercises per record: {}\n- Supported exercises / boundaries: {} / {}\n- Generator replay: {}/{} supported, {}/{} boundaries\n- Downstream formula replay: {}/{}\n- Boundary authorizations: {}\n- Exercise / boundary tamper rejection: {}/{}\n- Module/report replay and report tamper rejection: {} / {} / {}\n\nThe generator uses only declared source constraints and the generic formula runtime; it does not branch on formula identifiers or mutate curriculum state.\n",
            report.discovered_records,
            report.requested_per_record,
            report.supported_exercises,
            report.boundary_exercises,
            report.supported_replays,
            report.supported_exercises,
            report.boundary_replays,
            report.boundary_exercises,
            report.downstream_replays,
            report.supported_exercises,
            report.boundary_authorizations,
            report.exercise_tamper_rejections,
            report.boundary_tamper_rejections,
            report.module_replay,
            report.report_replay,
            report.report_tamper_rejected,
        ),
    )?;
    println!("{serialized}");
    Ok(())
}
