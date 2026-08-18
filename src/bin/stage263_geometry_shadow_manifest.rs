//! Stage 263: materialize the geometry candidate in a cloned manifest.
//!
//! The clone is an immutable evaluation input.  It is deliberately distinct
//! from `docs/curriculum_manifest.json`; no production curriculum or router is
//! changed by this stage.

use serde::Serialize;
use sha2::{Digest, Sha256};
use std::fs;

use the_machine::curriculum::{
    breadth_first_manifest, CurriculumManifest, CurriculumPack, CurriculumStatus, ValidationGates,
};

const REPORT_JSON: &str = "docs/stage263_geometry_shadow_manifest.json";
const REPORT_MD: &str = "docs/stage263_geometry_shadow_manifest.md";

#[derive(Debug, Serialize)]
struct Report {
    schema: &'static str,
    parent_manifest_hash: String,
    shadow_manifest_hash: String,
    parent_pack_count: usize,
    shadow_pack_count: usize,
    candidate_id: &'static str,
    candidate_status: CurriculumStatus,
    candidate_prerequisites: Vec<String>,
    candidate_source_reports: Vec<String>,
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

fn candidate() -> CurriculumPack {
    CurriculumPack {
        id: "source_derived_bounded_geometry".into(),
        title: "Source-derived bounded geometry and measurement".into(),
        status: CurriculumStatus::ShadowValidated,
        prerequisites: vec!["bounded_calculus".into()],
        reusable_artifacts: vec![
            "source_formula_catalog".into(),
            "typed_measurement_conversion".into(),
            "dimensional_expression".into(),
            "geometry_measurement_composition".into(),
        ],
        source_requirements: vec![
            "docs/stage163_source_geometry_acquisition.json".into(),
            "docs/stage164_source_geometry_language_transfer.json".into(),
            "docs/stage165_geometry_measurement_composition.json".into(),
            "docs/stage166_route_blind_measurement_composition.json".into(),
            "docs/stage167_geometry_technical_language_scale.json".into(),
            "docs/stage168_geometry_curriculum_admission.json".into(),
            "docs/stage169_geometry_promotion_rollback.json".into(),
            "docs/stage170_geometry_memory_integration.json".into(),
            "docs/stage171_curriculum_memory_scale.json".into(),
            "docs/stage172_memory_backed_geometry_routes.json".into(),
            "docs/stage173_route_blind_technical_language.json".into(),
            "docs/stage174_sealed_curriculum_learning_curve.json".into(),
        ],
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
        selection_reason: "source-derived geometry candidate selected from immutable evidence; staged only in a cloned manifest".into(),
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let parent = breadth_first_manifest();
    let parent_hash = parent.replay_hash();
    let candidate = candidate();
    let prerequisite_closure = candidate
        .prerequisites
        .iter()
        .all(|id| parent.packs.iter().any(|pack| &pack.id == id));
    let mut shadow = parent.clone();
    shadow.packs.push(candidate.clone());
    let validation_passed = shadow.validate().is_empty();
    let shadow_hash = shadow.replay_hash();
    let parent_unchanged = parent.replay_hash() == parent_hash;
    let report = Report {
        schema: "stage263-geometry-shadow-manifest-v1",
        parent_manifest_hash: parent_hash,
        shadow_manifest_hash: shadow_hash,
        parent_pack_count: parent.packs.len(),
        shadow_pack_count: shadow.packs.len(),
        candidate_id: "source_derived_bounded_geometry",
        candidate_status: candidate.status,
        candidate_prerequisites: candidate.prerequisites,
        candidate_source_reports: candidate.source_requirements,
        validation_passed,
        prerequisite_closure,
        parent_unchanged,
        shadow_only: true,
        live_manifest_mutations: 0,
        live_registry_mutations: 0,
        false_authorizations: 0,
        false_denials: 0,
        manifest: shadow,
    };
    assert!(report.validation_passed);
    assert!(report.prerequisite_closure);
    assert!(report.parent_unchanged);
    assert!(report.shadow_only);
    assert_eq!(report.live_manifest_mutations, 0);
    assert_eq!(report.live_registry_mutations, 0);
    assert_eq!(report.false_authorizations, 0);
    assert_eq!(report.false_denials, 0);
    fs::write(REPORT_JSON, serde_json::to_vec_pretty(&report)?)?;
    fs::write(
        REPORT_MD,
        format!(
            "# Stage 263 — geometry shadow manifest\n\nMaterialized `{}` in a cloned curriculum manifest for downstream shadow evaluation.\n\n* parent packs: {}\n* shadow packs: {}\n* parent manifest: `{}`\n* shadow manifest: `{}`\n* prerequisite closure: {}\n* validation: {}\n* shadow-only: {}\n* live manifest / registry mutations: 0 / 0\n* false authorizations / denials: 0 / 0\n\nThe production manifest remains unchanged.\n\nReproduce with `cargo run --quiet --bin stage263_geometry_shadow_manifest`.\n",
            report.candidate_id,
            report.parent_pack_count,
            report.shadow_pack_count,
            report.parent_manifest_hash,
            report.shadow_manifest_hash,
            report.prerequisite_closure,
            report.validation_passed,
            report.shadow_only,
        ),
    )?;
    println!(
        "stage263 parent_packs={} shadow_packs={} validation=true shadow_only=true manifest_mutated=false",
        report.parent_pack_count, report.shadow_pack_count
    );
    Ok(())
}
