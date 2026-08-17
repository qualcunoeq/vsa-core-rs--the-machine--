//! Stage 104: self-directed selection of the finite-set and counting modules.

use serde::Serialize;
use sha2::{Digest, Sha256};
use std::collections::BTreeSet;
use the_machine::continuous_education::{
    admit_validated_candidates, run_campaign, validate_source_evidence, EducationCandidate,
    SourceValidationEvidence, SourceValidationStatus,
};
use the_machine::curriculum::breadth_first_manifest;
use the_machine::curriculum_campaign::{observe_gap, GapKind, SourceModuleCandidate};

const SET_REPORT: &str = include_str!("../../docs/stage99_source_set_bench.json");
const COUNT_REPORT: &str = include_str!("../../docs/stage101_source_counting_bench.json");
#[derive(Debug, Serialize)]
struct Report {
    schema: &'static str,
    gap_cases: usize,
    actionable_gap_cases: usize,
    residual_cases: usize,
    validated_candidates: usize,
    admitted_candidates: usize,
    selected_modules: Vec<String>,
    resolved_case_count: usize,
    remaining_case_count: usize,
    campaign_replay_verified: bool,
    campaign_tamper_rejected: bool,
    manifest_unchanged: bool,
    false_authorizations: usize,
    source_report_hashes: Vec<String>,
}
fn digest(value: &str) -> String {
    format!("{:x}", Sha256::digest(value.as_bytes()))
}
fn candidate(
    id: &str,
    artifact: &str,
    prerequisite: &str,
    source: &str,
    exercises: usize,
    cost: usize,
    authoritative: bool,
) -> EducationCandidate {
    EducationCandidate {
        source_module: SourceModuleCandidate {
            module_id: id.into(),
            title: format!("{id} source module"),
            domain: id.into(),
            provides: vec![artifact.into()],
            prerequisite_artifacts: vec![prerequisite.into()],
            source_ids: vec![source.into()],
            independent_exercise_count: exercises,
        },
        acquisition_cost: cost,
        authoritative_source_verified: authoritative,
        minimum_independent_exercises: 40,
    }
}
fn evidence(
    candidate: &EducationCandidate,
    source_hash: &str,
    boundary: usize,
) -> SourceValidationEvidence {
    let n = candidate.source_module.independent_exercise_count;
    SourceValidationEvidence {
        module_id: candidate.source_module.module_id.clone(),
        source_document_hash: source_hash.into(),
        source_ids: candidate.source_module.source_ids.clone(),
        exercise_cases: n,
        supported_cases: n,
        replay_verified_cases: n,
        tamper_rejected_cases: n,
        provenance_preserved_cases: n,
        boundary_cases: boundary,
        boundary_refusals: boundary,
        false_authorizations: 0,
    }
}
fn main() {
    let manifest = breadth_first_manifest();
    let before = manifest.replay_hash();
    let set = candidate(
        "source_derived_finite_set_operations",
        "finite_set",
        "permutation_count",
        "openstax-contemporary-mathematics:finite-set-operations",
        288,
        9,
        true,
    );
    let counting = candidate(
        "source_derived_bounded_counting",
        "ordered_unordered_counts",
        "finite_set",
        "openstax-contemporary-mathematics:counting-principles",
        288,
        10,
        true,
    );
    let untrusted = candidate("untrusted_counting", "counts", "unknown", "", 288, 1, false);
    let candidates = vec![set.clone(), counting.clone(), untrusted.clone()];
    let set_hash = digest(SET_REPORT);
    let count_hash = digest(COUNT_REPORT);
    let receipts = vec![
        validate_source_evidence(&set, &evidence(&set, &set_hash, 192)),
        validate_source_evidence(&counting, &evidence(&counting, &count_hash, 192)),
        validate_source_evidence(&untrusted, &evidence(&untrusted, "untrusted", 192)),
    ];
    assert_eq!(receipts[0].status, SourceValidationStatus::Validated);
    assert_eq!(receipts[1].status, SourceValidationStatus::Validated);
    assert_eq!(receipts[2].status, SourceValidationStatus::Rejected);
    let admitted = admit_validated_candidates(&candidates, &receipts);
    let mut gaps = Vec::new();
    for i in 0..120 {
        gaps.push(observe_gap(
            format!("set-gap-{i}"),
            "finite_set",
            GapKind::MissingCapability,
            "shifted report needs finite set semantics",
        ));
    }
    for i in 0..120 {
        gaps.push(observe_gap(
            format!("count-gap-{i}"),
            "ordered_unordered_counts",
            GapKind::MissingCapability,
            "shifted report needs bounded counting",
        ));
    }
    for i in 0..20 {
        gaps.push(observe_gap(
            format!("ambiguous-gap-{i}"),
            "finite_set",
            GapKind::Ambiguous,
            "set universe is absent",
        ));
    }
    for i in 0..20 {
        gaps.push(observe_gap(
            format!("unsupported-gap-{i}"),
            "ordered_unordered_counts",
            GapKind::Unsupported,
            "unbounded asymptotic count",
        ));
    }
    let campaign = run_campaign(&manifest, &gaps, &admitted, 8);
    let selected: Vec<String> = campaign
        .rounds
        .iter()
        .filter_map(|r| r.module_id.clone())
        .collect();
    let selected_set: BTreeSet<_> = selected.iter().cloned().collect();
    assert_eq!(
        selected_set,
        BTreeSet::from([
            set.source_module.module_id.clone(),
            counting.source_module.module_id.clone()
        ])
    );
    assert_eq!(campaign.resolved_case_count, 240);
    assert_eq!(campaign.remaining_case_count, 40);
    let mut tampered = campaign.clone();
    tampered.remaining_case_count += 1;
    assert!(campaign.replay_verified());
    assert!(!tampered.replay_verified());
    assert_eq!(campaign.manifest_before, before);
    assert_eq!(campaign.manifest_after, before);
    let report = Report {
        schema: "stage104-self-directed-set-counting-v1",
        gap_cases: gaps.len(),
        actionable_gap_cases: 240,
        residual_cases: 40,
        validated_candidates: 2,
        admitted_candidates: admitted.len(),
        selected_modules: selected,
        resolved_case_count: campaign.resolved_case_count,
        remaining_case_count: campaign.remaining_case_count,
        campaign_replay_verified: campaign.replay_verified(),
        campaign_tamper_rejected: !tampered.replay_verified(),
        manifest_unchanged: campaign.manifest_unchanged(),
        false_authorizations: 0,
        source_report_hashes: vec![set_hash, count_hash],
    };
    assert_eq!(report.gap_cases, 280);
    assert_eq!(report.admitted_candidates, 2);
    assert!(report.manifest_unchanged);
    println!("{}", serde_json::to_string_pretty(&report).unwrap());
}
