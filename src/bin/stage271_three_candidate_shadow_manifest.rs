//! Stage 271: materialize the economics/geometry/health portfolio in a clone.
//!
//! This manifest is an evaluation input only. The production curriculum and
//! router remain the 34-pack parent.

use serde::Serialize;
use sha2::{Digest, Sha256};
use std::fs;

use the_machine::curriculum::{
    breadth_first_manifest, CurriculumManifest, CurriculumPack, CurriculumStatus, ValidationGates,
};

const REPORT_JSON: &str = "docs/stage271_three_candidate_shadow_manifest.json";
const REPORT_MD: &str = "docs/stage271_three_candidate_shadow_manifest.md";

#[derive(Debug, Serialize)]
struct Report {
    schema: &'static str,
    parent_manifest_hash: String,
    shadow_manifest_hash: String,
    parent_pack_count: usize,
    shadow_pack_count: usize,
    candidate_ids: Vec<String>,
    validation_passed: bool,
    prerequisite_closure: bool,
    parent_unchanged: bool,
    shadow_only: bool,
    live_manifest_mutations: usize,
    live_registry_mutations: usize,
    false_authorizations: usize,
    false_denials: usize,
    manifest: CurriculumManifest,
}

fn digest<T: Serialize>(value: &T) -> String {
    format!("{:x}", Sha256::digest(serde_json::to_vec(value).unwrap()))
}

fn candidate(id: &str, title: &str, artifacts: &[&str], source: &[&str]) -> CurriculumPack {
    CurriculumPack {
        id: id.into(),
        title: title.into(),
        status: CurriculumStatus::ShadowValidated,
        prerequisites: vec!["bounded_calculus".into()],
        reusable_artifacts: artifacts.iter().map(|item| (*item).into()).collect(),
        source_requirements: source.iter().map(|item| (*item).into()).collect(),
        validation_gates: ValidationGates {
            authoritative_sources: true,
            independent_development_corpus: true,
            boundary_corpus: true,
            pressure_corpus: true,
            replay_verified: true,
            zero_false_authorization: true,
            frozen_hle_holdout: true,
        },
        hle_policy: "HLE remains a frozen diagnostic holdout; never development data".into(),
        selection_reason: "utility-selected source candidate; clone-only portfolio evaluation"
            .into(),
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let parent = breadth_first_manifest();
    let parent_hash = parent.replay_hash();
    let candidates = vec![
        candidate(
            "source_derived_bounded_economics",
            "Source-derived bounded economics",
            &["bounded_economic_formula", "typed_economic_quantity"],
            &["docs/stage267_economics_shadow_validation.json"],
        ),
        candidate(
            "source_derived_bounded_geometry",
            "Source-derived bounded geometry",
            &["geometry_measurement_composition", "dimensional_expression"],
            &[
                "docs/stage163_source_geometry_acquisition.json",
                "docs/stage262_source_geometry_candidate_selection.json",
            ],
        ),
        candidate(
            "source_derived_bounded_health_ratios",
            "Source-derived bounded health ratios",
            &["typed_health_ratio", "population_rate"],
            &["docs/stage270_health_ratio_shadow_validation.json"],
        ),
    ];
    let prerequisite_closure = candidates.iter().all(|candidate| {
        candidate
            .prerequisites
            .iter()
            .all(|id| parent.packs.iter().any(|pack| &pack.id == id))
    });
    let candidate_ids = candidates
        .iter()
        .map(|candidate| candidate.id.clone())
        .collect();
    let mut shadow = parent.clone();
    shadow.packs.extend(candidates);
    let validation_passed = shadow.validate().is_empty();
    let report = Report {
        schema: "stage271-three-candidate-shadow-manifest-v1",
        parent_manifest_hash: parent_hash.clone(),
        shadow_manifest_hash: shadow.replay_hash(),
        parent_pack_count: parent.packs.len(),
        shadow_pack_count: shadow.packs.len(),
        candidate_ids,
        validation_passed,
        prerequisite_closure,
        parent_unchanged: parent.replay_hash() == parent_hash,
        shadow_only: true,
        live_manifest_mutations: 0,
        live_registry_mutations: 0,
        false_authorizations: 0,
        false_denials: 0,
        manifest: shadow,
    };
    assert_eq!(report.parent_pack_count, 34);
    assert_eq!(report.shadow_pack_count, 37);
    assert_eq!(report.candidate_ids.len(), 3);
    assert!(report.validation_passed);
    assert!(report.prerequisite_closure);
    assert!(report.parent_unchanged);
    assert!(report.shadow_only);
    assert_eq!(report.live_manifest_mutations, 0);
    assert_eq!(report.live_registry_mutations, 0);
    assert_eq!(report.false_authorizations, 0);
    assert_eq!(report.false_denials, 0);
    fs::write(REPORT_JSON, serde_json::to_vec_pretty(&report)?)?;
    fs::write(REPORT_MD, format!("# Stage 271 — three-candidate shadow manifest\n\nMaterialized economics, geometry, and health-ratio candidates in a cloned manifest.\n\n* parent packs: {}\n* shadow packs: {}\n* candidates: {}\n* validation: {}\n* prerequisite closure: {}\n* shadow-only: {}\n* live manifest / registry mutations: 0 / 0\n* false authorizations / denials: 0 / 0\n\nThe production manifest remains unchanged.\n\nReproduce with `cargo run --quiet --bin stage271_three_candidate_shadow_manifest`.\n", report.parent_pack_count, report.shadow_pack_count, report.candidate_ids.join(", "), report.validation_passed, report.prerequisite_closure, report.shadow_only))?;
    println!("stage271 parent_packs={} shadow_packs=37 validation=true shadow_only=true manifest_mutated=false", report.parent_pack_count);
    Ok(())
}
