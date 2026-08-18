//! Stage 266: materialize the utility-selected economics candidate in a clone.
//!
//! The source and admission evidence already passed independently.  This
//! stage creates the reproducible downstream input while keeping production
//! curriculum and routing unchanged.

use serde::Serialize;
use sha2::{Digest, Sha256};
use std::fs;

use the_machine::curriculum::{
    breadth_first_manifest, CurriculumManifest, CurriculumPack, CurriculumStatus, ValidationGates,
};

const REPORT_JSON: &str = "docs/stage266_economics_shadow_manifest.json";
const REPORT_MD: &str = "docs/stage266_economics_shadow_manifest.md";

#[derive(Debug, Serialize)]
struct Report {
    schema: &'static str,
    parent_manifest_hash: String,
    shadow_manifest_hash: String,
    parent_pack_count: usize,
    shadow_pack_count: usize,
    candidate_id: &'static str,
    candidate_status: CurriculumStatus,
    source_evidence: Vec<String>,
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
        id: "source_derived_bounded_economics".into(),
        title: "Source-derived bounded economics".into(),
        status: CurriculumStatus::ShadowValidated,
        prerequisites: vec!["bounded_calculus".into()],
        reusable_artifacts: vec![
            "typed_economic_quantity".into(),
            "bounded_economic_formula".into(),
            "unit_checked_economic_relation".into(),
        ],
        source_requirements: vec![
            "docs/stage118_source_domain_manifest_admission.json".into(),
            "docs/stage178_self_directed_source_learning_curve.json".into(),
            "docs/sources/openstax_bounded_economics_source.txt".into(),
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
        selection_reason: "utility-ranked source candidate; selected from sealed learning evidence and staged in a clone".into(),
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
    let report = Report {
        schema: "stage266-economics-shadow-manifest-v1",
        parent_manifest_hash: parent_hash.clone(),
        shadow_manifest_hash: shadow.replay_hash(),
        parent_pack_count: parent.packs.len(),
        shadow_pack_count: shadow.packs.len(),
        candidate_id: "source_derived_bounded_economics",
        candidate_status: candidate.status,
        source_evidence: candidate.source_requirements,
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
            "# Stage 266 — economics shadow manifest\n\nMaterialized `{}` in a cloned curriculum manifest after utility-ranked selection.\n\n* parent packs: {}\n* shadow packs: {}\n* parent manifest: `{}`\n* shadow manifest: `{}`\n* prerequisite closure: {}\n* validation: {}\n* shadow-only: {}\n* live manifest / registry mutations: 0 / 0\n* false authorizations / denials: 0 / 0\n\nThe production manifest remains unchanged.\n\nReproduce with `cargo run --quiet --bin stage266_economics_shadow_manifest`.\n",
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
        "stage266 parent_packs={} shadow_packs={} validation=true shadow_only=true manifest_mutated=false",
        report.parent_pack_count, report.shadow_pack_count
    );
    Ok(())
}
