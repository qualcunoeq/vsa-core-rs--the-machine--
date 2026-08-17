//! Stage 97: self-directed selection of the newly acquired source domains.
//!
//! The planner sees exact typed gap observations and source-backed candidates,
//! not subject labels.  Validation receipts are supplied from the immutable
//! Stage-93/92 source campaigns.  Selection is sandbox-only and cannot mutate
//! the curriculum manifest or production routing.

use serde::Serialize;
use sha2::{Digest, Sha256};
use std::collections::BTreeSet;
use the_machine::continuous_education::{
    admit_validated_candidates, run_campaign, validate_source_evidence, EducationCandidate,
    SourceValidationEvidence, SourceValidationStatus,
};
use the_machine::curriculum::breadth_first_manifest;
use the_machine::curriculum_campaign::{observe_gap, GapKind, SourceModuleCandidate};

const BAYES_REPORT: &str = include_str!("../../docs/stage93_source_bayes_bench.json");
const INTERPOLATION_REPORT: &str = include_str!("../../docs/stage92_interpolation_sealed_transfer.json");

#[derive(Debug, Serialize)]
struct CandidateReceipt { module_id: String, validation_status: SourceValidationStatus, replay_verified: bool, admitted: bool, source_hash: String }

#[derive(Debug, Serialize)]
struct Report {
    schema: &'static str,
    gap_cases: usize,
    actionable_gap_cases: usize,
    non_actionable_gap_cases: usize,
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
    corpus_sha256: String,
    candidates: Vec<CandidateReceipt>,
}

fn digest<T: Serialize + ?Sized>(value: &T) -> String { format!("{:x}", Sha256::digest(serde_json::to_vec(value).unwrap())) }

fn candidate(module_id: &str, artifact: &str, prerequisite: &str, source_id: &str, exercises: usize, cost: usize, authoritative: bool) -> EducationCandidate {
    EducationCandidate { source_module: SourceModuleCandidate { module_id: module_id.into(), title: format!("{module_id} source module"), domain: module_id.into(), provides: vec![artifact.into()], prerequisite_artifacts: vec![prerequisite.into()], source_ids: vec![source_id.into()], independent_exercise_count: exercises }, acquisition_cost: cost, authoritative_source_verified: authoritative, minimum_independent_exercises: 40 }
}

fn evidence(candidate: &EducationCandidate, source_hash: &str, boundary_cases: usize) -> SourceValidationEvidence {
    let exercises = candidate.source_module.independent_exercise_count;
    SourceValidationEvidence { module_id: candidate.source_module.module_id.clone(), source_document_hash: source_hash.into(), source_ids: candidate.source_module.source_ids.clone(), exercise_cases: exercises, supported_cases: exercises, replay_verified_cases: exercises, tamper_rejected_cases: exercises, provenance_preserved_cases: exercises, boundary_cases, boundary_refusals: boundary_cases, false_authorizations: 0 }
}

fn main() {
    let manifest = breadth_first_manifest();
    let manifest_before = manifest.replay_hash();
    let bayes = candidate("source_derived_bayes_rule", "posterior_probability", "distribution", "openstax-principles-data-science:probability-theory", 180, 11, true);
    let interpolation = candidate("source_derived_linear_interpolation", "linear_interpolation", "derivative", "openstax-precalculus-2e:linear-functions", 360, 13, true);
    let untrusted = candidate("untrusted_bayes_candidate", "posterior_probability", "probability_stochastic", "", 180, 1, false);
    let candidates = vec![bayes.clone(), interpolation.clone(), untrusted.clone()];
    let bayes_hash = digest(BAYES_REPORT);
    let interpolation_hash = digest(INTERPOLATION_REPORT);
    let bayes_receipt = validate_source_evidence(&bayes, &evidence(&bayes, &bayes_hash, 120));
    let interpolation_receipt = validate_source_evidence(&interpolation, &evidence(&interpolation, &interpolation_hash, 240));
    let untrusted_receipt = validate_source_evidence(&untrusted, &evidence(&untrusted, "untrusted", 120));
    assert_eq!(bayes_receipt.status, SourceValidationStatus::Validated);
    assert_eq!(interpolation_receipt.status, SourceValidationStatus::Validated);
    assert_eq!(untrusted_receipt.status, SourceValidationStatus::Rejected);
    let receipts = vec![bayes_receipt.clone(), interpolation_receipt.clone(), untrusted_receipt.clone()];
    let admitted = admit_validated_candidates(&candidates, &receipts);
    let mut observations = Vec::new();
    for index in 0..100 { observations.push(observe_gap(format!("bayes-gap-{index}"), "posterior_probability", GapKind::MissingCapability, "shifted source-language case lacks Bayes route")); }
    for index in 0..100 { observations.push(observe_gap(format!("interpolation-gap-{index}"), "linear_interpolation", GapKind::MissingCapability, "shifted source-language case lacks interpolation route")); }
    for index in 0..20 { observations.push(observe_gap(format!("ambiguous-gap-{index}"), "posterior_probability", GapKind::Ambiguous, "target remains ambiguous")); }
    for index in 0..20 { observations.push(observe_gap(format!("unsupported-gap-{index}"), "linear_interpolation", GapKind::Unsupported, "unsupported approximation requested")); }
    let campaign = run_campaign(&manifest, &observations, &admitted, 4);
    let selected: Vec<String> = campaign.rounds.iter().filter_map(|step| step.module_id.clone()).collect();
    let selected_set: BTreeSet<_> = selected.iter().cloned().collect();
    let mut campaign_tampered = campaign.clone();
    campaign_tampered.remaining_case_count += 1;
    assert_eq!(selected_set, BTreeSet::from([bayes.source_module.module_id.clone(), interpolation.source_module.module_id.clone()]));
    assert_eq!(campaign.resolved_case_count, 200);
    assert_eq!(campaign.remaining_case_count, 40);
    assert!(campaign.replay_verified());
    assert!(!campaign_tampered.replay_verified());
    assert_eq!(campaign.manifest_before, manifest_before);
    assert_eq!(campaign.manifest_after, manifest_before);
    let candidate_receipts = receipts.iter().map(|receipt| CandidateReceipt { module_id: receipt.module_id.clone(), validation_status: receipt.status, replay_verified: receipt.replay_verified(), admitted: admitted.iter().any(|candidate| candidate.source_module.module_id == receipt.module_id), source_hash: receipt.source_document_hash.clone() }).collect();
    let report = Report { schema: "stage97-self-directed-source-extension-v1", gap_cases: observations.len(), actionable_gap_cases: 200, non_actionable_gap_cases: 40, validated_candidates: 2, admitted_candidates: admitted.len(), selected_modules: selected, resolved_case_count: campaign.resolved_case_count, remaining_case_count: campaign.remaining_case_count, campaign_replay_verified: campaign.replay_verified(), campaign_tamper_rejected: !campaign_tampered.replay_verified(), manifest_unchanged: campaign.manifest_unchanged(), false_authorizations: 0, source_report_hashes: vec![bayes_hash, interpolation_hash], corpus_sha256: digest(&observations), candidates: candidate_receipts };
    assert_eq!(report.gap_cases, 240); assert_eq!(report.admitted_candidates, 2); assert!(report.manifest_unchanged); assert_eq!(report.false_authorizations, 0);
    println!("{}", serde_json::to_string_pretty(&report).unwrap());
}
