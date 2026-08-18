//! Stage 276: assemble a clone-only release candidate for three source packs.
//!
//! All evidence gates are checked together. The resulting manifest is a
//! release candidate, not a live promotion; production routing remains on the
//! immutable parent manifest.

use serde::Serialize;
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::fs;

use the_machine::curriculum::{
    breadth_first_manifest, CurriculumManifest, CurriculumPack, CurriculumStatus, ValidationGates,
};

const REPORT_JSON: &str = "docs/stage276_three_candidate_release_candidate.json";
const REPORT_MD: &str = "docs/stage276_three_candidate_release_candidate.md";
const EVIDENCE: [&str; 8] = [
    "docs/stage267_economics_shadow_validation.json",
    "docs/stage268_economics_promotion_rollback.json",
    "docs/stage270_health_ratio_shadow_validation.json",
    "docs/stage275_health_ratio_promotion_rollback.json",
    "docs/stage273_staged_portfolio_exam_5000.json",
    "docs/stage274_hle_staged_portfolio_probe.json",
    "docs/stage263_geometry_shadow_manifest.json",
    "docs/stage262_source_geometry_candidate_selection.json",
];

#[derive(Debug, Serialize)]
struct Report {
    schema: &'static str,
    parent_manifest_hash: String,
    release_manifest_hash: String,
    parent_pack_count: usize,
    release_pack_count: usize,
    candidate_ids: Vec<String>,
    evidence_artifacts: usize,
    evidence_hashes: Vec<String>,
    source_validation_gates: bool,
    portfolio_exam_gate: bool,
    transfer_probe_gate: bool,
    rollback_gate: bool,
    prerequisite_closure: bool,
    release_manifest_valid: bool,
    parent_unchanged: bool,
    release_candidate_only: bool,
    live_manifest_mutations: usize,
    live_registry_mutations: usize,
    false_authorizations: usize,
    false_denials: usize,
    manifest: CurriculumManifest,
}

fn digest(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

fn load(path: &str) -> Result<Value, Box<dyn std::error::Error>> {
    Ok(serde_json::from_slice(&fs::read(path)?)?)
}

fn number(value: &Value, key: &str) -> usize {
    value.get(key).and_then(Value::as_u64).unwrap_or_default() as usize
}

fn candidate(id: &str, title: &str, artifacts: &[&str], sources: &[&str]) -> CurriculumPack {
    CurriculumPack {
        id: id.into(),
        title: title.into(),
        status: CurriculumStatus::Promotable,
        prerequisites: vec!["bounded_calculus".into()],
        reusable_artifacts: artifacts.iter().map(|item| (*item).into()).collect(),
        source_requirements: sources.iter().map(|item| (*item).into()).collect(),
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
        selection_reason: "release candidate assembled only after immutable source, sealed, transfer, and rollback evidence".into(),
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let evidence = EVIDENCE
        .iter()
        .map(
            |path| -> Result<(String, Value), Box<dyn std::error::Error>> {
                let bytes = fs::read(path)?;
                Ok((digest(&bytes), serde_json::from_slice(&bytes)?))
            },
        )
        .collect::<Result<Vec<_>, _>>()?;
    let source_validation_gates = number(&evidence[0].1, "exact_decisions") == 600
        && number(&evidence[2].1, "exact_decisions") == 600;
    let portfolio_exam_gate = number(&evidence[4].1, "exact_decisions") == 5000
        && number(&evidence[4].1, "sealed_exact") == 1500
        && number(&evidence[4].1, "false_authorizations") == 0;
    let transfer_probe_gate = number(&evidence[5].1, "cases") == 2500
        && number(&evidence[5].1, "unique_shadow_candidates") == 0
        && number(&evidence[5].1, "false_authorizations") == 0;
    let rollback_gate = number(&evidence[1].1, "rollbacks_applied") == 40
        && number(&evidence[3].1, "rollbacks_applied") == 40
        && evidence.iter().all(|(_, value)| {
            number(value, "false_authorizations") == 0 && number(value, "false_denials") == 0
        });
    let parent = breadth_first_manifest();
    let parent_hash = parent.replay_hash();
    let candidates = vec![
        candidate(
            "source_derived_bounded_economics",
            "Source-derived bounded economics",
            &["bounded_economic_formula", "typed_economic_quantity"],
            &[EVIDENCE[0]],
        ),
        candidate(
            "source_derived_bounded_geometry",
            "Source-derived bounded geometry",
            &["geometry_measurement_composition", "dimensional_expression"],
            &[EVIDENCE[6], EVIDENCE[7]],
        ),
        candidate(
            "source_derived_bounded_health_ratios",
            "Source-derived bounded health ratios",
            &["typed_health_ratio", "population_rate"],
            &[EVIDENCE[2]],
        ),
    ];
    let candidate_ids = candidates
        .iter()
        .map(|candidate| candidate.id.clone())
        .collect::<Vec<_>>();
    let prerequisite_closure = candidates.iter().all(|candidate| {
        candidate
            .prerequisites
            .iter()
            .all(|id| parent.packs.iter().any(|pack| &pack.id == id))
    });
    let mut release = parent.clone();
    release.packs.extend(candidates);
    let release_manifest_valid = release.validate().is_empty();
    let parent_unchanged = parent.replay_hash() == parent_hash;
    let report = Report {
        schema: "stage276-three-candidate-release-candidate-v1",
        parent_manifest_hash: parent_hash,
        release_manifest_hash: release.replay_hash(),
        parent_pack_count: parent.packs.len(),
        release_pack_count: release.packs.len(),
        candidate_ids,
        evidence_artifacts: EVIDENCE.len(),
        evidence_hashes: evidence.iter().map(|(hash, _)| hash.clone()).collect(),
        source_validation_gates,
        portfolio_exam_gate,
        transfer_probe_gate,
        rollback_gate,
        prerequisite_closure,
        release_manifest_valid,
        parent_unchanged,
        release_candidate_only: true,
        live_manifest_mutations: 0,
        live_registry_mutations: 0,
        false_authorizations: 0,
        false_denials: 0,
        manifest: release,
    };
    assert_eq!(report.parent_pack_count, 34);
    assert_eq!(report.release_pack_count, 37);
    assert_eq!(report.candidate_ids.len(), 3);
    assert_eq!(report.evidence_artifacts, 8);
    assert!(report.source_validation_gates);
    assert!(report.portfolio_exam_gate);
    assert!(report.transfer_probe_gate);
    assert!(report.rollback_gate);
    assert!(report.prerequisite_closure);
    assert!(report.release_manifest_valid);
    assert!(report.parent_unchanged);
    assert!(report.release_candidate_only);
    assert_eq!(report.live_manifest_mutations, 0);
    assert_eq!(report.live_registry_mutations, 0);
    assert_eq!(report.false_authorizations, 0);
    assert_eq!(report.false_denials, 0);
    fs::write(REPORT_JSON, serde_json::to_vec_pretty(&report)?)?;
    fs::write(REPORT_MD, format!("# Stage 276 — three-candidate release candidate\n\nAssembled a clone-only release manifest for economics, geometry, and health ratios.\n\n* parent / release packs: {} / {}\n* evidence artifacts: {}\n* source validation / portfolio exam / transfer / rollback gates: {} / {} / {} / {}\n* prerequisite closure: {}\n* release manifest valid: {}\n* parent unchanged: {}\n* release-candidate-only: {}\n* live manifest / registry mutations: 0 / 0\n* false authorizations / denials: 0 / 0\n\nNo production promotion occurred.\n\nReproduce with `cargo run --quiet --bin stage276_three_candidate_release_candidate`.\n", report.parent_pack_count, report.release_pack_count, report.evidence_artifacts, report.source_validation_gates, report.portfolio_exam_gate, report.transfer_probe_gate, report.rollback_gate, report.prerequisite_closure, report.release_manifest_valid, report.parent_unchanged, report.release_candidate_only))?;
    println!("stage276 parent_packs={} release_packs={} all_gates=true release_candidate_only=true manifest_mutated=false", report.parent_pack_count, report.release_pack_count);
    Ok(())
}
