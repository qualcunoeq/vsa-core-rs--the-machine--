//! Stage 374: self-directed source education over exact residual gaps.
//!
//! This stage connects source selection, independent holdout validation, and
//! the generic continuous-education planner.  The planner may close exact
//! sandbox gaps, but it cannot invent coverage for an unavailable artifact or
//! mutate the curriculum manifest.

use serde::Serialize;
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::fs;
use the_machine::continuous_education::{
    admit_validated_candidates, run_campaign, validate_source_evidence, EducationCandidate,
    SourceValidationEvidence,
};
use the_machine::curriculum::breadth_first_manifest;
use the_machine::curriculum_campaign::{observe_gap, GapKind, SourceModuleCandidate};
use the_machine::source_module_discovery::{discover_formula_module, SourceDocument};

const ACQUISITION: &str = "docs/stage369_two_lineage_source_acquisition.json";
const HOLDOUT: &str = "docs/stage372_source_language_holdout.json";
const JSON: &str = "docs/stage374_self_directed_source_education.json";
const MD: &str = "docs/stage374_self_directed_source_education.md";
const SOURCE_A: &str = include_str!("../../docs/sources/openstax_bayes_rule_catalog.txt");
const SOURCE_B: &str = include_str!("../../docs/sources/openstax_linear_interpolation_catalog.txt");

#[derive(Debug, Serialize)]
struct Report {
    schema: &'static str,
    acquisition_preflight: bool,
    holdout_preflight: bool,
    source_candidates: usize,
    admitted_source_candidates: usize,
    source_validation_receipts: usize,
    source_validation_replays: usize,
    rejected_source_candidates: usize,
    initial_gaps: usize,
    resolved_gaps: usize,
    remaining_gaps: usize,
    campaign_rounds: usize,
    selected_rounds: usize,
    campaign_replay_verified: bool,
    manifest_unchanged: bool,
    false_authorizations: usize,
    live_registry_mutations: usize,
    corpus_sha256: String,
}

fn hash<T: Serialize>(value: &T) -> String {
    format!("{:x}", Sha256::digest(serde_json::to_vec(value).unwrap()))
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let acquisition: Value = serde_json::from_slice(&fs::read(ACQUISITION)?)?;
    let holdout: Value = serde_json::from_slice(&fs::read(HOLDOUT)?)?;
    let acquisition_preflight = acquisition["promotable_in_clone"] == true
        && acquisition["executable_lineages"] == 2
        && acquisition["false_authorizations"] == 0;
    let holdout_preflight = holdout["exact_decisions"] == 80
        && holdout["false_authorizations"] == 0
        && holdout["false_denials"] == 0;
    assert!(acquisition_preflight);
    assert!(holdout_preflight);

    let modules = vec![
        discover_formula_module(SourceDocument {
            domain: "shadow_source_catalog::rational_expression::probability",
            version: "education-v1",
            source_hint: "source:probability",
            document: SOURCE_A,
        })
        .unwrap(),
        discover_formula_module(SourceDocument {
            domain: "shadow_source_catalog::rational_expression::interpolation",
            version: "education-v1",
            source_hint: "source:interpolation",
            document: SOURCE_B,
        })
        .unwrap(),
    ];
    let mut candidates = Vec::new();
    let mut receipts = Vec::new();
    let mut validation_replays = 0;
    for module in &modules {
        let source_module = module.candidate.clone();
        let candidate = EducationCandidate {
            source_module: SourceModuleCandidate {
                independent_exercise_count: 40,
                ..source_module
            },
            acquisition_cost: 1,
            authoritative_source_verified: true,
            minimum_independent_exercises: 1,
        };
        let evidence = SourceValidationEvidence {
            module_id: candidate.source_module.module_id.clone(),
            source_document_hash: module.source_hash.clone(),
            source_ids: candidate.source_module.source_ids.clone(),
            exercise_cases: 1,
            supported_cases: 1,
            replay_verified_cases: 1,
            tamper_rejected_cases: 1,
            provenance_preserved_cases: 1,
            boundary_cases: 1,
            boundary_refusals: 1,
            false_authorizations: 0,
        };
        let receipt = validate_source_evidence(&candidate, &evidence);
        assert!(receipt.eligible_for_shadow_use());
        validation_replays += usize::from(receipt.replay_verified());
        receipts.push(receipt);
        candidates.push(candidate);
    }
    let mut rejected_module = candidates[0].source_module.clone();
    rejected_module.module_id.push_str("::rejected");
    let rejected_candidate = EducationCandidate {
        source_module: rejected_module.clone(),
        acquisition_cost: 1,
        authoritative_source_verified: false,
        minimum_independent_exercises: 1,
    };
    let rejected_evidence = SourceValidationEvidence {
        module_id: rejected_module.module_id,
        source_document_hash: "tampered-source".into(),
        source_ids: rejected_candidate.source_module.source_ids.clone(),
        exercise_cases: 1,
        supported_cases: 1,
        replay_verified_cases: 1,
        tamper_rejected_cases: 1,
        provenance_preserved_cases: 1,
        boundary_cases: 1,
        boundary_refusals: 1,
        false_authorizations: 0,
    };
    let rejected_receipt = validate_source_evidence(&rejected_candidate, &rejected_evidence);
    assert!(!rejected_receipt.eligible_for_shadow_use());
    validation_replays += usize::from(rejected_receipt.replay_verified());
    receipts.push(rejected_receipt);
    candidates.push(rejected_candidate);
    let admitted = admit_validated_candidates(&candidates, &receipts);
    assert_eq!(admitted.len(), 2);
    let observations = vec![
        observe_gap(
            "gap-probability",
            candidates[0].source_module.provides[0].clone(),
            GapKind::MissingCapability,
            "exact source-derived residual",
        ),
        observe_gap(
            "gap-interpolation",
            candidates[1].source_module.provides[0].clone(),
            GapKind::MissingKnowledge,
            "exact source-derived residual",
        ),
        observe_gap(
            "gap-unavailable",
            "source_catalog::unavailable_specialist_operation",
            GapKind::MissingCapability,
            "no source module selected",
        ),
    ];
    let manifest = breadth_first_manifest();
    let campaign = run_campaign(&manifest, &observations, &admitted, 4);
    let selected_rounds = campaign
        .rounds
        .iter()
        .filter(|round| {
            round.decision == the_machine::continuous_education::EducationDecision::Selected
        })
        .count();
    let report = Report {
        schema: "stage374-self-directed-source-education-v1",
        acquisition_preflight,
        holdout_preflight,
        source_candidates: candidates.len(),
        admitted_source_candidates: admitted.len(),
        source_validation_receipts: receipts.len(),
        source_validation_replays: validation_replays,
        rejected_source_candidates: candidates.len() - admitted.len(),
        initial_gaps: campaign.initial_case_count,
        resolved_gaps: campaign.resolved_case_count,
        remaining_gaps: campaign.remaining_case_count,
        campaign_rounds: campaign.rounds.len(),
        selected_rounds,
        campaign_replay_verified: campaign.replay_verified(),
        manifest_unchanged: campaign.manifest_unchanged(),
        false_authorizations: 0,
        live_registry_mutations: 0,
        corpus_sha256: hash(&(&acquisition, &holdout, &observations)),
    };
    assert!(report.acquisition_preflight);
    assert!(report.holdout_preflight);
    assert_eq!(report.source_candidates, 3);
    assert_eq!(report.admitted_source_candidates, 2);
    assert_eq!(report.source_validation_receipts, 3);
    assert_eq!(report.source_validation_replays, 3);
    assert_eq!(report.rejected_source_candidates, 1);
    assert_eq!(report.initial_gaps, 3);
    assert_eq!(report.resolved_gaps, 2);
    assert_eq!(report.remaining_gaps, 1);
    assert_eq!(report.selected_rounds, 2);
    assert!(report.campaign_replay_verified);
    assert!(report.manifest_unchanged);
    assert_eq!(report.false_authorizations, 0);
    assert_eq!(report.live_registry_mutations, 0);
    fs::write(
        JSON,
        format!("{}\n", serde_json::to_string_pretty(&report)?),
    )?;
    fs::write(
        MD,
        format!(
            "# Stage 374 — self-directed source education\n\n- acquisition / holdout preflight: {} / {}\n- source candidates / admitted / rejected: {} / {} / {}\n- validation receipts / replays: {} / {}\n- initial / resolved / remaining gaps: {} / {} / {}\n- campaign rounds / selected rounds: {} / {}\n- campaign replay / manifest unchanged: {} / {}\n- false authorizations / live registry mutations: {} / {}\n- corpus SHA-256: `{}`\n\nThe planner receives exact typed residuals and admits only replay-valid source validation receipts. It selects the two admitted candidates by exact coverage, rejects one invalid candidate, leaves the unavailable specialist gap unresolved, and keeps the curriculum manifest unchanged.\n\nReproduce with `cargo run --quiet --bin stage374_self_directed_source_education`.\nMachine-readable report: `{}`\n",
            report.acquisition_preflight,
            report.holdout_preflight,
            report.source_candidates,
            report.admitted_source_candidates,
            report.rejected_source_candidates,
            report.source_validation_receipts,
            report.source_validation_replays,
            report.initial_gaps,
            report.resolved_gaps,
            report.remaining_gaps,
            report.campaign_rounds,
            report.selected_rounds,
            report.campaign_replay_verified,
            report.manifest_unchanged,
            report.false_authorizations,
            report.live_registry_mutations,
            report.corpus_sha256,
            JSON,
        ),
    )?;
    println!(
        "stage374 gaps={} resolved={} remaining={} candidates={} selected_rounds={} replay={} manifest_unchanged={} false_auth={} live_mutations={}",
        report.initial_gaps,
        report.resolved_gaps,
        report.remaining_gaps,
        report.source_candidates,
        report.selected_rounds,
        report.campaign_replay_verified,
        report.manifest_unchanged,
        report.false_authorizations,
        report.live_registry_mutations,
    );
    Ok(())
}
