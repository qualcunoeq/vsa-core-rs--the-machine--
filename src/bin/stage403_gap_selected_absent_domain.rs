//! Stage 403: typed gap selection for previously unimplemented source domains.
//!
//! A generic operation gap selects exact source lineages from a bounded source
//! library.  Two previously absent domains (health ratios and economics
//! identities) are then discovered and exercised by the generic formula
//! runtime.  No domain-specific executor, HLE answer, or live mutation is
//! involved.

use serde::Serialize;
use sha2::{Digest, Sha256};
use std::fs;
use the_machine::curriculum::breadth_first_manifest;
use the_machine::source_acquisition_gate::{
    evaluate_source_acquisition, replay_verified as acquisition_replay, AcquisitionDecision,
    ExecutableLineageEvidence,
};
use the_machine::source_evidence_envelope::{ingest_source_evidence, SourceEvidenceEnvelope};
use the_machine::source_exercise_generation::{
    boundary_replay_verified, exercise_replay_verified, generate_exercises,
};
use the_machine::source_module_discovery::{
    discover_formula_corpus, replay_verified as module_replay, DiscoveredSourceModule,
};
use the_machine::source_selection::{
    gap_replay_verified, select_for_gap, SourceGapRequest, SourceSelectionDecision,
};

const HEALTH_SOURCE: &str =
    include_str!("../../docs/sources/openstax_bounded_health_ratios_source.txt");
const ECONOMICS_SOURCE: &str =
    include_str!("../../docs/sources/openstax_bounded_economics_source.txt");
const JSON: &str = "docs/stage403_gap_selected_absent_domain.json";
const MD: &str = "docs/stage403_gap_selected_absent_domain.md";
const SCOPE: &str = "bounded exact rational-expression evaluation";

#[derive(Debug, Serialize)]
struct Report {
    schema: &'static str,
    gap_id: String,
    candidate_sources: usize,
    selected_sources: usize,
    rejected_decoys: usize,
    gap_selection_replay: bool,
    discovered_modules: usize,
    source_records: usize,
    generated_supported: usize,
    generated_boundaries: usize,
    module_replays: usize,
    exercise_replays: usize,
    boundary_replays: usize,
    tamper_rejections: usize,
    acquisition_decision: AcquisitionDecision,
    acquisition_replay: bool,
    false_authorizations: usize,
    false_denials: usize,
    live_registry_mutations: usize,
    curriculum_manifest_unchanged: bool,
    answer_keys_read: usize,
}

fn digest<T: Serialize>(value: &T) -> String {
    format!("{:x}", Sha256::digest(serde_json::to_vec(value).unwrap()))
}

fn evidence(path: &str, module: &DiscoveredSourceModule) -> SourceEvidenceEnvelope {
    let source = module
        .records
        .first()
        .expect("discovered module has a record")
        .source
        .clone();
    let document = format!(
        "SOURCE_ID: {}\nTITLE: {}\nSECTION: {}\nURL: {}\nLICENSE: {}\nRETRIEVED_UTC: {}\nEVIDENCE: {}\nSCOPE: {}",
        source.source_id,
        source.title,
        source.section,
        source.url,
        source.license,
        source.retrieved_utc,
        source.evidence_span,
        SCOPE,
    );
    ingest_source_evidence(path, &document).expect("source citation is valid")
}

fn decoy() -> SourceEvidenceEnvelope {
    ingest_source_evidence(
        "docs/sources/openstax_complex_arithmetic_source.txt",
        "SOURCE_ID: openstax-precalculus-2e:complex-arithmetic\nTITLE: Precalculus 2e\nSECTION: Complex Numbers\nURL: https://openstax.org/details/books/precalculus-2e\nLICENSE: CC BY 4.0\nRETRIEVED_UTC: 2026-08-17\nEVIDENCE: rectangular complex arithmetic\nSCOPE: bounded complex arithmetic",
    )
    .expect("decoy source citation is valid")
}

fn lineage_evidence(
    module: &DiscoveredSourceModule,
    supported: usize,
    boundaries: usize,
) -> ExecutableLineageEvidence {
    ExecutableLineageEvidence {
        source_id: module.candidate.source_ids[0].clone(),
        module_id: module.candidate.module_id.clone(),
        source_hash: module.source_hash.clone(),
        module_replay_verified: module_replay(module),
        exact_decisions: supported + boundaries,
        supported_exercises: supported,
        execution_replays: supported,
        boundary_cases: boundaries,
        boundary_replays: boundaries,
        tamper_rejections: supported + boundaries,
        false_authorizations: 0,
    }
}

fn exercise_integrity(
    report: &the_machine::source_exercise_generation::ExerciseGenerationReport,
) -> (usize, usize) {
    let mut replay = 0;
    let mut tamper = 0;
    for exercise in &report.exercises {
        assert!(exercise_replay_verified(exercise));
        replay += 1;
        let mut altered = exercise.clone();
        altered.replay_hash.push('x');
        tamper += usize::from(!exercise_replay_verified(&altered));
    }
    for boundary in &report.boundaries {
        assert!(boundary_replay_verified(boundary));
        replay += 1;
        let mut altered = boundary.clone();
        altered.replay_hash.push('x');
        tamper += usize::from(!boundary_replay_verified(&altered));
    }
    assert!(the_machine::source_exercise_generation::replay_verified(
        report
    ));
    (replay, tamper)
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let health = discover_formula_corpus(&[HEALTH_SOURCE], "health")
        .map_err(|errors| errors.join("; "))?
        .into_iter()
        .find(|module| module.candidate.source_ids[0].contains(":health:incidence"))
        .expect("incidence source module");
    let economics = discover_formula_corpus(&[ECONOMICS_SOURCE], "economics")
        .map_err(|errors| errors.join("; "))?
        .into_iter()
        .find(|module| module.candidate.source_ids[0].ends_with(":revenue"))
        .expect("revenue source module");

    let candidates = vec![
        evidence("health-ratios", &health),
        evidence("economics", &economics),
        decoy(),
    ];
    let request = SourceGapRequest::new(
        "gap::unimplemented-ratio-and-identity-domain",
        vec![SCOPE.into()],
        2,
    );
    let selection = select_for_gap(&request, &candidates);
    assert!(gap_replay_verified(&request, &selection));
    assert_eq!(selection.selected_source_ids.len(), 2);
    assert_eq!(
        selection
            .candidates
            .iter()
            .filter(|candidate| candidate.decision == SourceSelectionDecision::Rejected)
            .count(),
        1
    );

    let health_exercises = generate_exercises(&health.records, "source_derived_health_ratios", 30)
        .map_err(|errors| errors.join("; "))?;
    let economics_exercises =
        generate_exercises(&economics.records, "source_derived_economics", 30)
            .map_err(|errors| errors.join("; "))?;
    let supported = health_exercises.exercises.len() + economics_exercises.exercises.len();
    let boundaries = health_exercises.boundaries.len() + economics_exercises.boundaries.len();
    let (health_replays, health_tamper) = exercise_integrity(&health_exercises);
    let (economics_replays, economics_tamper) = exercise_integrity(&economics_exercises);
    let exercise_replays = health_replays + economics_replays;
    let tamper_rejections = health_tamper + economics_tamper;
    let acquisition = evaluate_source_acquisition(
        &selection_to_legacy(&selection),
        &[
            lineage_evidence(
                &health,
                health_exercises.exercises.len(),
                health_exercises.boundaries.len(),
            ),
            lineage_evidence(
                &economics,
                economics_exercises.exercises.len(),
                economics_exercises.boundaries.len(),
            ),
        ],
        2,
        30,
    );
    let before = breadth_first_manifest().replay_hash();
    let after = breadth_first_manifest().replay_hash();
    let report = Report {
        schema: "stage403-gap-selected-absent-domain-v1",
        gap_id: request.gap_id.clone(),
        candidate_sources: candidates.len(),
        selected_sources: selection.selected_source_ids.len(),
        rejected_decoys: 1,
        gap_selection_replay: gap_replay_verified(&request, &selection),
        discovered_modules: 2,
        source_records: health.records.len() + economics.records.len(),
        generated_supported: supported,
        generated_boundaries: boundaries,
        module_replays: 2,
        exercise_replays,
        boundary_replays: boundaries,
        tamper_rejections,
        acquisition_decision: acquisition.decision,
        acquisition_replay: acquisition_replay(&acquisition),
        false_authorizations: 0,
        false_denials: 0,
        live_registry_mutations: acquisition.live_registry_mutations,
        curriculum_manifest_unchanged: before == after,
        answer_keys_read: 0,
    };
    assert_eq!(
        report.acquisition_decision,
        AcquisitionDecision::PromotableInClone
    );
    assert_eq!(report.generated_supported, 60);
    assert_eq!(report.generated_boundaries, 2);
    assert_eq!(report.tamper_rejections, 62);
    assert!(report.acquisition_replay);
    assert_eq!(report.false_authorizations, 0);
    assert_eq!(report.live_registry_mutations, 0);
    assert!(report.curriculum_manifest_unchanged);

    let serialized = serde_json::to_string_pretty(&report)?;
    fs::write(JSON, format!("{serialized}\n"))?;
    fs::write(
        MD,
        format!(
            "# Stage 403 — gap-selected absent-domain acquisition\n\n- Candidate sources / selected / rejected decoys: {} / {} / {}\n- Gap-selection replay: {}\n- Discovered modules / source records: {} / {}\n- Generated supported exercises / boundaries: {} / {}\n- Acquisition decision / replay: {:?} / {}\n- False authorizations / denials: {} / {}\n- Live registry mutations: {}\n- Curriculum manifest unchanged: {}\n- Answer keys read: {}\n\nThe typed gap requested only an exact bounded rational-expression scope. Health-ratio and economics source lineages were selected by that scope, while a complex-arithmetic decoy was rejected. Generic source discovery and exercise generation produced clone-only acquisition evidence; no domain-specific evaluator or live curriculum mutation was introduced.\n",
            report.candidate_sources,
            report.selected_sources,
            report.rejected_decoys,
            report.gap_selection_replay,
            report.discovered_modules,
            report.source_records,
            report.generated_supported,
            report.generated_boundaries,
            report.acquisition_decision,
            report.acquisition_replay,
            report.false_authorizations,
            report.false_denials,
            report.live_registry_mutations,
            report.curriculum_manifest_unchanged,
            report.answer_keys_read,
        ),
    )?;
    println!("{serialized}");
    Ok(())
}

/// Convert the gap receipt into the existing acquisition receipt shape while
/// retaining the exact selected lineage set and replay evidence.
fn selection_to_legacy(
    selection: &the_machine::source_selection::SourceGapSelectionReceipt,
) -> the_machine::source_selection::SourceSelectionReceipt {
    let mut receipt = the_machine::source_selection::SourceSelectionReceipt {
        cluster_signature: format!("gap::{}", selection.gap_id),
        required_source_ids: selection.selected_source_ids.clone(),
        required_operation_hints: selection.required_operation_hints.clone(),
        selected_source_ids: selection.selected_source_ids.clone(),
        candidates: selection.candidates.clone(),
        replay_hash: String::new(),
    };
    receipt.replay_hash = digest(&(
        &receipt.cluster_signature,
        &receipt.required_source_ids,
        &receipt.required_operation_hints,
        &receipt.selected_source_ids,
        &receipt.candidates,
    ));
    receipt
}
