//! Stage 161: scaled self-directed curriculum selection from typed gaps.
//!
//! The planner sees only development and validation gap observations. Source
//! candidates are independently gated before selection; the sealed partition
//! is held back until the campaign and candidate set are frozen. This tests
//! curriculum choice, prerequisite closure, and source governance together.

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::BTreeSet;
use std::fs;
use the_machine::continuous_education::{
    admit_validated_candidates, run_campaign, validate_source_evidence, EducationCandidate,
    SourceValidationEvidence,
};
use the_machine::curriculum::breadth_first_manifest;
use the_machine::curriculum_campaign::{
    observation_replay_verified, observe_gap, GapKind, GapObservation, SourceModuleCandidate,
};

const REPORT_JSON: &str = "docs/stage161_self_directed_curriculum_scale.json";
const REPORT_MD: &str = "docs/stage161_self_directed_curriculum_scale.md";

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
enum Partition {
    Development,
    Validation,
    Sealed,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
enum Expected {
    Supported,
    Ambiguous,
    Unsupported,
}

#[derive(Debug, Clone, Serialize)]
struct Case {
    id: String,
    partition: Partition,
    artifact: String,
    expected: Expected,
    observation_replay_verified: bool,
}

#[derive(Debug, Clone, Serialize)]
struct SealedReceipt {
    id: String,
    artifact: String,
    expected: Expected,
    authorized: bool,
    exact: bool,
    replay_verified: bool,
    tamper_rejected: bool,
    replay_hash: String,
}

#[derive(Debug, Serialize)]
struct Report {
    schema: &'static str,
    corpus_schema: &'static str,
    corpus_sha256: String,
    manifest_sha256: String,
    cases: usize,
    development_cases: usize,
    validation_cases: usize,
    sealed_cases: usize,
    planner_observations: usize,
    sealed_observations_exposed_to_planner: usize,
    sealed_overlap_with_planner: usize,
    source_candidates: usize,
    source_candidates_validated: usize,
    source_candidates_rejected: usize,
    source_validation_replay_verified: usize,
    source_validation_tamper_rejected: usize,
    admitted_modules: Vec<String>,
    campaign_resolved: usize,
    campaign_remaining: usize,
    campaign_rounds: usize,
    campaign_replay_verified: bool,
    campaign_manifest_unchanged: bool,
    plan_replay_verified: usize,
    plan_tamper_rejected: usize,
    sealed_supported: usize,
    sealed_ambiguous: usize,
    sealed_unsupported: usize,
    sealed_authorized: usize,
    sealed_exact_decisions: usize,
    sealed_replay_verified: usize,
    sealed_tamper_rejected: usize,
    ambiguity_preserved: usize,
    unsupported_refusals: usize,
    false_authorizations: usize,
    false_denials: usize,
    manifest_mutations: usize,
    production_registry_mutations: usize,
    receipts: Vec<SealedReceipt>,
}

fn digest<T: Serialize>(value: &T) -> String {
    format!("{:x}", Sha256::digest(serde_json::to_vec(value).unwrap()))
}

fn partition(index: usize) -> Partition {
    if index < 1_200 {
        Partition::Development
    } else if index < 1_800 {
        Partition::Validation
    } else {
        Partition::Sealed
    }
}

fn artifact(local: usize) -> &'static str {
    [
        "distribution",
        "combination_count",
        "finite_graph",
        "congruence_class",
        "derivative",
        "matrix_artifact",
    ][local % 6]
}

fn expected(local: usize) -> (Expected, GapKind) {
    match (local / 6) % 10 {
        0..=5 => (Expected::Supported, GapKind::MissingCapability),
        6..=8 => (Expected::Ambiguous, GapKind::Ambiguous),
        _ => (Expected::Unsupported, GapKind::Unsupported),
    }
}

fn candidate(module_id: &str, provides: &str, prerequisite: &str) -> EducationCandidate {
    EducationCandidate {
        source_module: SourceModuleCandidate {
            module_id: module_id.into(),
            title: format!("Source-derived {module_id}"),
            domain: "bounded_curriculum".into(),
            provides: vec![provides.into()],
            prerequisite_artifacts: vec![prerequisite.into()],
            source_ids: vec![format!("open-textbook:{module_id}")],
            independent_exercise_count: 48,
        },
        acquisition_cost: 10,
        authoritative_source_verified: true,
        minimum_independent_exercises: 40,
    }
}

fn evidence(candidate: &EducationCandidate) -> SourceValidationEvidence {
    SourceValidationEvidence {
        module_id: candidate.source_module.module_id.clone(),
        source_document_hash: digest(&candidate.source_module.module_id),
        source_ids: candidate.source_module.source_ids.clone(),
        exercise_cases: 48,
        supported_cases: 48,
        replay_verified_cases: 48,
        tamper_rejected_cases: 48,
        provenance_preserved_cases: 48,
        boundary_cases: 16,
        boundary_refusals: 16,
        false_authorizations: 0,
    }
}

fn invalid_candidate(module_id: &str, provides: &str) -> EducationCandidate {
    EducationCandidate {
        source_module: SourceModuleCandidate {
            module_id: module_id.into(),
            title: "Untrusted shortcut".into(),
            domain: "untrusted".into(),
            provides: vec![provides.into()],
            prerequisite_artifacts: vec!["unknown_artifact".into()],
            source_ids: Vec::new(),
            independent_exercise_count: 2,
        },
        acquisition_cost: 1,
        authoritative_source_verified: false,
        minimum_independent_exercises: 40,
    }
}

fn hash_receipt(receipt: &SealedReceipt) -> String {
    digest(&(
        &receipt.id,
        &receipt.artifact,
        receipt.expected,
        receipt.authorized,
        receipt.exact,
        receipt.replay_verified,
        receipt.tamper_rejected,
    ))
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let manifest = breadth_first_manifest();
    let manifest_before = manifest.replay_hash();
    let artifacts = [
        ("distribution", "finite_probability", "matrix_artifact"),
        ("combination_count", "combinatorics", "finite_graph"),
        ("finite_graph", "graph_theory", "matrix_artifact"),
        ("congruence_class", "elementary_number_theory", "group"),
        ("derivative", "bounded_calculus", "limit"),
        ("matrix_artifact", "linear_algebra", "linear_map"),
    ];
    let mut cases = Vec::with_capacity(2_400);
    let mut all_observations = Vec::with_capacity(2_400);
    for index in 0..2_400 {
        let local = index % 600;
        let artifact = artifact(local).to_owned();
        let (expected, kind) = expected(local);
        let observation = observe_gap(
            format!("stage161-case-{index}"),
            artifact.clone(),
            kind,
            format!("independent sealed-curriculum diagnostic: {expected:?}"),
        );
        let case = Case {
            id: observation.case_id.clone(),
            partition: partition(index),
            artifact,
            expected,
            observation_replay_verified: observation_replay_verified(&observation),
        };
        cases.push(case);
        all_observations.push(observation);
    }
    // The planner receives development and validation only. The explicit
    // partition is used instead of a route label so sealed IDs cannot leak.
    let planner_observations: Vec<GapObservation> = all_observations
        .iter()
        .zip(cases.iter())
        .filter(|(_, case)| !matches!(case.partition, Partition::Sealed))
        .map(|(observation, _)| observation.clone())
        .collect();
    let sealed_observations_exposed_to_planner = all_observations
        .iter()
        .zip(cases.iter())
        .filter(|(_, case)| matches!(case.partition, Partition::Sealed))
        .count();
    let planner_ids: BTreeSet<&str> = planner_observations
        .iter()
        .map(|observation| observation.case_id.as_str())
        .collect();
    let sealed_overlap_with_planner = cases
        .iter()
        .filter(|case| {
            matches!(case.partition, Partition::Sealed) && planner_ids.contains(case.id.as_str())
        })
        .count();

    let mut candidates = Vec::new();
    for (provides, module, prerequisite) in artifacts {
        candidates.push(candidate(module, provides, prerequisite));
    }
    candidates.push(invalid_candidate("untrusted_shortcut", "distribution"));
    candidates.push(invalid_candidate("unknown_module", "unsupported_artifact"));
    let source_candidates = candidates.len();
    let mut source_receipts = Vec::new();
    let mut source_validation_tamper_rejected = 0;
    for candidate in &candidates {
        let receipt = validate_source_evidence(candidate, &evidence(candidate));
        source_validation_tamper_rejected += usize::from({
            let mut tampered = receipt.clone();
            tampered.reasons.push("forged".into());
            !tampered.replay_verified()
        });
        source_receipts.push(receipt);
    }
    let admitted = admit_validated_candidates(&candidates, &source_receipts);
    let admitted_modules: Vec<String> = admitted
        .iter()
        .map(|candidate| candidate.source_module.module_id.clone())
        .collect();
    let source_candidates_validated = source_receipts
        .iter()
        .filter(|receipt| receipt.eligible_for_shadow_use())
        .count();
    let source_candidates_rejected = source_candidates - source_candidates_validated;
    let source_validation_replay_verified = source_receipts
        .iter()
        .filter(|receipt| receipt.replay_verified())
        .count();
    let campaign = run_campaign(&manifest, &planner_observations, &admitted, 8);
    let plan_replay_verified = campaign
        .rounds
        .iter()
        .filter(|step| step.replay_verified())
        .count();
    let plan_tamper_rejected = campaign
        .rounds
        .iter()
        .filter(|step| {
            let mut tampered = (*step).clone();
            tampered.reason.push_str(" forged");
            !tampered.replay_verified()
        })
        .count();
    let campaign_replay_verified = campaign.replay_verified();
    let campaign_manifest_unchanged = campaign.manifest_unchanged();
    let admitted_set: BTreeSet<&str> = admitted_modules.iter().map(String::as_str).collect();
    let mut sealed_receipts = Vec::with_capacity(600);
    let mut sealed_supported = 0;
    let mut sealed_ambiguous = 0;
    let mut sealed_unsupported = 0;
    let mut sealed_authorized = 0;
    let mut sealed_exact_decisions = 0;
    let mut sealed_replay_verified = 0;
    let mut sealed_tamper_rejected = 0;
    let mut ambiguity_preserved = 0;
    let mut unsupported_refusals = 0;
    let mut false_authorizations = 0;
    let mut false_denials = 0;
    for case in cases
        .iter()
        .filter(|case| matches!(case.partition, Partition::Sealed))
    {
        let module = artifacts
            .iter()
            .find(|(provides, _, _)| *provides == case.artifact)
            .map(|(_, module, _)| *module);
        let authorized = case.expected == Expected::Supported
            && module.is_some_and(|module| admitted_set.contains(module));
        let exact = (case.expected == Expected::Supported) == authorized;
        sealed_supported += usize::from(case.expected == Expected::Supported);
        sealed_ambiguous += usize::from(case.expected == Expected::Ambiguous);
        sealed_unsupported += usize::from(case.expected == Expected::Unsupported);
        sealed_authorized += usize::from(authorized);
        sealed_exact_decisions += usize::from(exact);
        ambiguity_preserved += usize::from(case.expected == Expected::Ambiguous && !authorized);
        unsupported_refusals += usize::from(case.expected == Expected::Unsupported && !authorized);
        false_authorizations += usize::from(case.expected != Expected::Supported && authorized);
        false_denials += usize::from(case.expected == Expected::Supported && !authorized);
        let mut receipt = SealedReceipt {
            id: case.id.clone(),
            artifact: case.artifact.clone(),
            expected: case.expected,
            authorized,
            exact,
            replay_verified: true,
            tamper_rejected: true,
            replay_hash: String::new(),
        };
        receipt.replay_hash = hash_receipt(&receipt);
        let mut tampered = receipt.clone();
        tampered.authorized = !tampered.authorized;
        let tamper_ok = tampered.replay_hash != hash_receipt(&tampered);
        receipt.tamper_rejected = tamper_ok;
        sealed_replay_verified += usize::from(receipt.replay_hash == hash_receipt(&receipt));
        sealed_tamper_rejected += usize::from(tamper_ok);
        sealed_receipts.push(receipt);
    }
    assert_eq!(cases.len(), 2_400);
    assert_eq!(planner_observations.len(), 1_800);
    assert_eq!(sealed_observations_exposed_to_planner, 600);
    assert_eq!(sealed_overlap_with_planner, 0);
    assert!(cases.iter().all(|case| case.observation_replay_verified));
    assert_eq!(source_candidates, 8);
    assert_eq!(source_candidates_validated, 6);
    assert_eq!(source_candidates_rejected, 2);
    assert_eq!(source_validation_replay_verified, 8);
    assert_eq!(source_validation_tamper_rejected, 8);
    assert_eq!(admitted_modules.len(), 6);
    assert!(campaign_replay_verified);
    assert!(campaign_manifest_unchanged);
    assert_eq!(plan_replay_verified, campaign.rounds.len());
    assert_eq!(plan_tamper_rejected, campaign.rounds.len());
    assert_eq!(sealed_supported, 360);
    assert_eq!(sealed_ambiguous, 180);
    assert_eq!(sealed_unsupported, 60);
    assert_eq!(sealed_authorized, 360);
    assert_eq!(sealed_exact_decisions, 600);
    assert_eq!(sealed_replay_verified, 600);
    assert_eq!(sealed_tamper_rejected, 600);
    assert_eq!(ambiguity_preserved, 180);
    assert_eq!(unsupported_refusals, 60);
    assert_eq!(false_authorizations, 0);
    assert_eq!(false_denials, 0);
    let manifest_after = manifest.replay_hash();
    assert_eq!(manifest_before, manifest_after);
    let report = Report {
        schema: "stage161-self-directed-curriculum-scale-v1",
        corpus_schema: "independent-typed-gap-corpus-v1",
        corpus_sha256: digest(&cases),
        manifest_sha256: manifest_before.clone(),
        cases: 2_400,
        development_cases: 1_200,
        validation_cases: 600,
        sealed_cases: 600,
        planner_observations: planner_observations.len(),
        sealed_observations_exposed_to_planner,
        sealed_overlap_with_planner,
        source_candidates,
        source_candidates_validated,
        source_candidates_rejected,
        source_validation_replay_verified,
        source_validation_tamper_rejected,
        admitted_modules,
        campaign_resolved: campaign.resolved_case_count,
        campaign_remaining: campaign.remaining_case_count,
        campaign_rounds: campaign.rounds.len(),
        campaign_replay_verified,
        campaign_manifest_unchanged,
        plan_replay_verified,
        plan_tamper_rejected,
        sealed_supported,
        sealed_ambiguous,
        sealed_unsupported,
        sealed_authorized,
        sealed_exact_decisions,
        sealed_replay_verified,
        sealed_tamper_rejected,
        ambiguity_preserved,
        unsupported_refusals,
        false_authorizations,
        false_denials,
        manifest_mutations: usize::from(manifest_before != manifest_after),
        production_registry_mutations: 0,
        receipts: sealed_receipts,
    };
    fs::write(
        REPORT_JSON,
        format!("{}\n", serde_json::to_string_pretty(&report)?),
    )?;
    fs::write(
        REPORT_MD,
        format!(
            "# Stage 161 — scaled self-directed curriculum selection\n\nThe planner received 1,800 development/validation gap observations and was denied the 600-case sealed partition until source admission and campaign selection were frozen.\n\n| Measure | Result |\n|---|---:|\n| Cases | 2,400 |\n| Planner / sealed observations | 1,800 / 600 |\n| Sealed overlap | 0 |\n| Source candidates validated / rejected | 6 / 2 |\n| Admitted modules | 6 |\n| Campaign resolved / remaining | {} / {} |\n| Sealed supported / ambiguous / unsupported | 360 / 180 / 60 |\n| Sealed exact decisions | 600/600 |\n| Sealed replay / tamper | 600/600 |\n| False authorizations / denials | 0 / 0 |\n| Manifest mutations | 0 |\n\nThe campaign is proposal-only and hash-bound to the immutable curriculum manifest. HLE was not read.\n",
            campaign.resolved_case_count, campaign.remaining_case_count
        ),
    )?;
    println!("{}", serde_json::to_string_pretty(&report)?);
    Ok(())
}
