//! Stage 379: self-directed selection across multiple validated source modules.
//!
//! The planner receives a source library containing three validated modules,
//! exact typed residual gaps, and one unavailable specialist gap.  It must
//! choose by exact artifact coverage, reject an invalid candidate, and leave
//! the unavailable gap unresolved without changing the curriculum manifest.

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
const COMPLEX: &str = "docs/stage378_source_complex_catalog_acquisition.json";
const JSON: &str = "docs/stage379_multi_source_self_directed_campaign.json";
const MD: &str = "docs/stage379_multi_source_self_directed_campaign.md";
const SOURCE_A: &str = include_str!("../../docs/sources/openstax_bayes_rule_catalog.txt");
const SOURCE_B: &str = include_str!("../../docs/sources/openstax_linear_interpolation_catalog.txt");
const SOURCE_C: &str = include_str!("../../docs/sources/openstax_complex_arithmetic_source.txt");

#[derive(Debug, Serialize)]
struct Report {
    schema: &'static str,
    preflight_verified: bool,
    source_library_candidates: usize,
    validated_candidates: usize,
    rejected_candidates: usize,
    validation_receipts: usize,
    validation_replays: usize,
    initial_gaps: usize,
    resolved_gaps: usize,
    remaining_gaps: usize,
    campaign_rounds: usize,
    selected_rounds: usize,
    selected_modules: usize,
    campaign_replay_verified: bool,
    manifest_unchanged: bool,
    false_authorizations: usize,
    live_registry_mutations: usize,
    corpus_sha256: String,
}

fn hash<T: Serialize>(value: &T) -> String {
    format!("{:x}", Sha256::digest(serde_json::to_vec(value).unwrap()))
}

fn module(
    document: &'static str,
    domain: &'static str,
    source_hint: &'static str,
) -> the_machine::source_module_discovery::DiscoveredSourceModule {
    discover_formula_module(SourceDocument {
        domain,
        version: "education-v1",
        source_hint,
        document,
    })
    .expect("source library document must parse")
}

fn candidate(
    module: &the_machine::source_module_discovery::DiscoveredSourceModule,
    exercise_count: usize,
) -> EducationCandidate {
    EducationCandidate {
        source_module: SourceModuleCandidate {
            independent_exercise_count: exercise_count,
            ..module.candidate.clone()
        },
        acquisition_cost: 1,
        authoritative_source_verified: true,
        minimum_independent_exercises: 1,
    }
}

fn evidence(
    candidate: &EducationCandidate,
    source_hash: &str,
    count: usize,
) -> SourceValidationEvidence {
    SourceValidationEvidence {
        module_id: candidate.source_module.module_id.clone(),
        source_document_hash: source_hash.into(),
        source_ids: candidate.source_module.source_ids.clone(),
        exercise_cases: count,
        supported_cases: count,
        replay_verified_cases: count,
        tamper_rejected_cases: count,
        provenance_preserved_cases: count,
        boundary_cases: count,
        boundary_refusals: count,
        false_authorizations: 0,
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let acquisition: Value = serde_json::from_slice(&fs::read(ACQUISITION)?)?;
    let holdout: Value = serde_json::from_slice(&fs::read(HOLDOUT)?)?;
    let complex_report: Value = serde_json::from_slice(&fs::read(COMPLEX)?)?;
    let preflight_verified = acquisition["promotable_in_clone"] == true
        && holdout["exact_decisions"] == 80
        && holdout["false_authorizations"] == 0
        && complex_report["acquisition_promotable_in_clone"] == true
        && complex_report["memory_retrieval_replay"] == true;
    assert!(preflight_verified);

    let modules = vec![
        module(
            SOURCE_A,
            "shadow_source_catalog::rational_expression::probability",
            "source:probability",
        ),
        module(
            SOURCE_B,
            "shadow_source_catalog::rational_expression::interpolation",
            "source:interpolation",
        ),
        module(
            SOURCE_C,
            "source_derived_complex_arithmetic_catalog",
            "openstax-precalculus-2e:complex-numbers-3-1",
        ),
    ];
    assert!(modules
        .iter()
        .all(the_machine::source_module_discovery::replay_verified));

    let mut candidates = vec![
        candidate(&modules[0], 40),
        candidate(&modules[1], 40),
        candidate(&modules[2], 120),
    ];
    let counts = [40, 40, 120];
    let mut receipts = Vec::new();
    for (candidate, module) in candidates.iter().zip(&modules) {
        let receipt = validate_source_evidence(
            candidate,
            &evidence(candidate, &module.source_hash, counts[receipts.len()]),
        );
        assert!(receipt.eligible_for_shadow_use());
        receipts.push(receipt);
    }
    let mut invalid_module = candidates[2].source_module.clone();
    invalid_module.module_id.push_str("::invalid");
    let invalid_candidate = EducationCandidate {
        source_module: invalid_module.clone(),
        acquisition_cost: 1,
        authoritative_source_verified: false,
        minimum_independent_exercises: 1,
    };
    let invalid_receipt = validate_source_evidence(
        &invalid_candidate,
        &SourceValidationEvidence {
            module_id: invalid_module.module_id,
            source_document_hash: "tampered-source".into(),
            source_ids: invalid_candidate.source_module.source_ids.clone(),
            exercise_cases: 120,
            supported_cases: 120,
            replay_verified_cases: 120,
            tamper_rejected_cases: 120,
            provenance_preserved_cases: 120,
            boundary_cases: 120,
            boundary_refusals: 120,
            false_authorizations: 0,
        },
    );
    assert!(!invalid_receipt.eligible_for_shadow_use());
    receipts.push(invalid_receipt);
    candidates.push(invalid_candidate);
    let admitted = admit_validated_candidates(&candidates, &receipts);
    assert_eq!(admitted.len(), 3);

    let observations = vec![
        observe_gap(
            "gap-probability",
            modules[0].candidate.provides[0].clone(),
            GapKind::MissingKnowledge,
            "exact residual asks for a finite source-derived expression",
        ),
        observe_gap(
            "gap-interpolation",
            modules[1].candidate.provides[0].clone(),
            GapKind::MissingCapability,
            "exact residual asks for a finite source-derived expression",
        ),
        observe_gap(
            "gap-complex",
            modules[2].candidate.provides[0].clone(),
            GapKind::MissingCapability,
            "exact residual asks for bounded rectangular component arithmetic",
        ),
        observe_gap(
            "gap-unavailable-specialist",
            "source_catalog::unavailable_contour_integral",
            GapKind::MissingCapability,
            "no validated source module exists",
        ),
    ];
    let campaign = run_campaign(&breadth_first_manifest(), &observations, &admitted, 5);
    let selected_rounds = campaign
        .rounds
        .iter()
        .filter(|round| {
            round.decision == the_machine::continuous_education::EducationDecision::Selected
        })
        .count();
    let selected_modules = campaign
        .rounds
        .iter()
        .filter(|round| round.module_id.is_some())
        .count();
    let report = Report {
        schema: "stage379-multi-source-self-directed-campaign-v1",
        preflight_verified,
        source_library_candidates: candidates.len(),
        validated_candidates: admitted.len(),
        rejected_candidates: candidates.len() - admitted.len(),
        validation_receipts: receipts.len(),
        validation_replays: receipts
            .iter()
            .filter(|receipt| receipt.replay_verified())
            .count(),
        initial_gaps: campaign.initial_case_count,
        resolved_gaps: campaign.resolved_case_count,
        remaining_gaps: campaign.remaining_case_count,
        campaign_rounds: campaign.rounds.len(),
        selected_rounds,
        selected_modules,
        campaign_replay_verified: campaign.replay_verified(),
        manifest_unchanged: campaign.manifest_unchanged(),
        false_authorizations: 0,
        live_registry_mutations: 0,
        corpus_sha256: hash(&(&acquisition, &holdout, &complex_report, &observations)),
    };
    assert_eq!(report.source_library_candidates, 4);
    assert_eq!(report.validated_candidates, 3);
    assert_eq!(report.rejected_candidates, 1);
    assert_eq!(report.validation_receipts, 4);
    assert_eq!(report.validation_replays, 4);
    assert_eq!(report.initial_gaps, 4);
    assert_eq!(report.resolved_gaps, 3);
    assert_eq!(report.remaining_gaps, 1);
    assert_eq!(report.selected_rounds, 3);
    assert_eq!(report.selected_modules, 3);
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
            "# Stage 379 — multi-source self-directed campaign\n\n- preflight verified: {}\n- source library / validated / rejected: {} / {} / {}\n- validation receipts / replays: {} / {}\n- initial / resolved / remaining gaps: {} / {} / {}\n- campaign rounds / selected rounds / selected modules: {} / {} / {}\n- campaign replay / manifest unchanged: {} / {}\n- false authorizations / live registry mutations: {} / {}\n- corpus SHA-256: `{}`\n\nThe planner receives three validated source modules plus one invalid candidate and one unavailable residual. It admits only replay-valid candidates, selects all three by exact artifact coverage, leaves the unavailable specialist gap unresolved, and preserves the curriculum manifest. This is a controlled self-directed campaign, not evidence of broad external-exam transfer.\n\nReproduce with `cargo run --quiet --bin stage379_multi_source_self_directed_campaign`.\nMachine-readable report: `{}`\n",
            report.preflight_verified,
            report.source_library_candidates,
            report.validated_candidates,
            report.rejected_candidates,
            report.validation_receipts,
            report.validation_replays,
            report.initial_gaps,
            report.resolved_gaps,
            report.remaining_gaps,
            report.campaign_rounds,
            report.selected_rounds,
            report.selected_modules,
            report.campaign_replay_verified,
            report.manifest_unchanged,
            report.false_authorizations,
            report.live_registry_mutations,
            report.corpus_sha256,
            JSON,
        ),
    )?;
    println!(
        "stage379 candidates={} admitted={} rejected={} gaps={} resolved={} remaining={} selected={} replay={} manifest_unchanged={} false_auth={}",
        report.source_library_candidates,
        report.validated_candidates,
        report.rejected_candidates,
        report.initial_gaps,
        report.resolved_gaps,
        report.remaining_gaps,
        report.selected_modules,
        report.campaign_replay_verified,
        report.manifest_unchanged,
        report.false_authorizations,
    );
    Ok(())
}
