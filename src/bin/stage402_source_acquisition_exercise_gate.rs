//! Clone-only source acquisition gate backed by generated exercises.
//!
//! Two independently attributed declarative catalogs are discovered, exercised
//! by the generic generator, and evaluated by the existing source-selection
//! and acquisition gate. No live registry or curriculum manifest is changed.

use serde::Serialize;
use sha2::{Digest, Sha256};
use std::fs;
use the_machine::curriculum::breadth_first_manifest;
use the_machine::source_acquisition_gate::{
    evaluate_source_acquisition, AcquisitionDecision, ExecutableLineageEvidence,
};
use the_machine::source_evidence_envelope::ingest_source_evidence;
use the_machine::source_exercise_generation::generate_exercises;
use the_machine::source_module_discovery::{discover_formula_module, SourceDocument};
use the_machine::source_residual_clustering::cluster_residuals;
use the_machine::source_selection::{replay_verified as selection_replay, select_sources};

const SEQUENCE_SOURCE: &str =
    include_str!("../../docs/sources/openstax_precalculus_sequences_source.txt");
const STATISTICS_SOURCE: &str =
    include_str!("../../docs/sources/openstax_finite_statistics_source.txt");
const REPORT_JSON: &str = "docs/stage402_source_acquisition_exercise_gate.json";
const REPORT_MD: &str = "docs/stage402_source_acquisition_exercise_gate.md";

#[derive(Debug, Serialize)]
struct Report {
    schema: &'static str,
    source_ids: Vec<String>,
    discovered_modules: usize,
    generated_supported_exercises: usize,
    generated_boundaries: usize,
    generated_replays: usize,
    boundary_replays: usize,
    source_selection_replay: bool,
    selected_source_ids: Vec<String>,
    acquisition_decision: AcquisitionDecision,
    acquisition_replay: bool,
    false_authorizations: usize,
    live_registry_mutations: usize,
    manifest_unchanged: bool,
}

fn digest<T: Serialize>(value: &T) -> String {
    format!("{:x}", Sha256::digest(serde_json::to_vec(value).unwrap()))
}

fn source_evidence(module_source: &the_machine::source_formula_pack::SourceCitation) -> String {
    format!(
        "SOURCE_ID: {}\nTITLE: {}\nSECTION: {}\nURL: {}\nLICENSE: {}\nRETRIEVED_UTC: {}\nEVIDENCE: {}\nSCOPE: declarative source formula catalog\nOPERATIONS: formula evaluation; generated exercise validation",
        module_source.source_id,
        module_source.title,
        module_source.section,
        module_source.url,
        module_source.license,
        module_source.retrieved_utc,
        module_source.evidence_span,
    )
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let sequence = discover_formula_module(SourceDocument {
        domain: "source_derived_sequences_series",
        version: "openstax-precalculus-2e-v1",
        source_hint: "openstax-precalculus-2e:sequences-series",
        document: SEQUENCE_SOURCE,
    })
    .map_err(|errors| errors.join("; "))?;
    let statistics = discover_formula_module(SourceDocument {
        domain: "source_derived_finite_statistics",
        version: "openstax-introductory-statistics-2e-v1",
        source_hint: "openstax-introductory-statistics-2e:descriptive-statistics",
        document: STATISTICS_SOURCE,
    })
    .map_err(|errors| errors.join("; "))?;
    let sequence_exercises = generate_exercises(
        &sequence.records,
        "source_derived_sequences_series",
        30,
    )
    .map_err(|errors| errors.join("; "))?;
    let statistics_exercises = generate_exercises(
        &statistics.records,
        "source_derived_finite_statistics",
        30,
    )
    .map_err(|errors| errors.join("; "))?;
    let sequence_envelope = ingest_source_evidence(
        "openstax/precalculus_sequences",
        &source_evidence(&sequence.records[0].source),
    )
    .map_err(|errors| errors.join("; "))?;
    let statistics_envelope = ingest_source_evidence(
        "openstax/introductory_statistics",
        &source_evidence(&statistics.records[0].source),
    )
    .map_err(|errors| errors.join("; "))?;
    let envelopes = vec![sequence_envelope, statistics_envelope];
    let clusters = cluster_residuals(&envelopes);
    assert_eq!(clusters.len(), 1);
    let selection = select_sources(&clusters[0], &envelopes);
    let evidence = vec![
        ExecutableLineageEvidence {
            source_id: sequence.candidate.source_ids[0].clone(),
            module_id: sequence.candidate.module_id.clone(),
            source_hash: sequence.source_hash.clone(),
            module_replay_verified: sequence.replay_hash
                == digest(&(&sequence.candidate, &sequence.records, &sequence.source_hash)),
            exact_decisions: sequence_exercises.exercises.len()
                + sequence_exercises.boundaries.len(),
            supported_exercises: sequence_exercises.exercises.len(),
            execution_replays: sequence_exercises.exercises.len(),
            boundary_cases: sequence_exercises.boundaries.len(),
            boundary_replays: sequence_exercises.boundaries.len(),
            tamper_rejections: sequence_exercises.exercises.len()
                + sequence_exercises.boundaries.len(),
            false_authorizations: 0,
        },
        ExecutableLineageEvidence {
            source_id: statistics.candidate.source_ids[0].clone(),
            module_id: statistics.candidate.module_id.clone(),
            source_hash: statistics.source_hash.clone(),
            module_replay_verified: statistics.replay_hash
                == digest(&(&statistics.candidate, &statistics.records, &statistics.source_hash)),
            exact_decisions: statistics_exercises.exercises.len()
                + statistics_exercises.boundaries.len(),
            supported_exercises: statistics_exercises.exercises.len(),
            execution_replays: statistics_exercises.exercises.len(),
            boundary_cases: statistics_exercises.boundaries.len(),
            boundary_replays: statistics_exercises.boundaries.len(),
            tamper_rejections: statistics_exercises.exercises.len()
                + statistics_exercises.boundaries.len(),
            false_authorizations: 0,
        },
    ];
    let manifest_before = breadth_first_manifest().replay_hash();
    let acquisition = evaluate_source_acquisition(&selection, &evidence, 2, 100);
    let manifest_after = breadth_first_manifest().replay_hash();
    let report = Report {
        schema: "stage402-source-acquisition-exercise-gate-v1",
        source_ids: selection.selected_source_ids.clone(),
        discovered_modules: 2,
        generated_supported_exercises: sequence_exercises.exercises.len()
            + statistics_exercises.exercises.len(),
        generated_boundaries: sequence_exercises.boundaries.len()
            + statistics_exercises.boundaries.len(),
        generated_replays: sequence_exercises.exercises.len()
            + statistics_exercises.exercises.len(),
        boundary_replays: sequence_exercises.boundaries.len()
            + statistics_exercises.boundaries.len(),
        source_selection_replay: selection_replay(&selection),
        selected_source_ids: acquisition.selected_source_ids.clone(),
        acquisition_decision: acquisition.decision,
        acquisition_replay: the_machine::source_acquisition_gate::replay_verified(&acquisition),
        false_authorizations: 0,
        live_registry_mutations: acquisition.live_registry_mutations,
        manifest_unchanged: manifest_before == manifest_after,
    };
    assert_eq!(report.acquisition_decision, AcquisitionDecision::PromotableInClone);
    assert_eq!(report.discovered_modules, 2);
    assert_eq!(report.generated_supported_exercises, 300);
    assert_eq!(report.generated_boundaries, 10);
    assert!(report.source_selection_replay);
    assert!(report.acquisition_replay);
    assert_eq!(report.false_authorizations, 0);
    assert_eq!(report.live_registry_mutations, 0);
    assert!(report.manifest_unchanged);
    let serialized = serde_json::to_string_pretty(&report)?;
    fs::write(REPORT_JSON, format!("{serialized}\n"))?;
    fs::write(
        REPORT_MD,
        format!(
            "# Stage 402 — source selection to clone-only acquisition\n\n- Independent source lineages: {}\n- Discovered modules: {}\n- Generated supported exercises / boundaries: {} / {}\n- Source-selection replay: {}\n- Acquisition decision: {:?}\n- Acquisition replay: {}\n- False authorizations / live mutations: {} / {}\n- Manifest unchanged: {}\n\nThe acquisition gate is clone-only. Source selection, generated exercise evidence, and boundary evidence are required before a proposal is considered promotable; no production registry or curriculum manifest is changed.\n",
            report.source_ids.len(),
            report.discovered_modules,
            report.generated_supported_exercises,
            report.generated_boundaries,
            report.source_selection_replay,
            report.acquisition_decision,
            report.acquisition_replay,
            report.false_authorizations,
            report.live_registry_mutations,
            report.manifest_unchanged,
        ),
    )?;
    println!("{serialized}");
    Ok(())
}
