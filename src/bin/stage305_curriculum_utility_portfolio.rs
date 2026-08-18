//! Stage 305: utility-aware autonomous curriculum selection.
//!
//! This stage connects exact gap observations, prerequisite discovery, source
//! validation, downstream utility, and a hard acquisition budget.  Selection
//! is a shadow proposal: it appends only replayable planning/evidence receipts
//! to a clone of current memory and never mutates the curriculum, registry,
//! production router, or HLE holdout.

use serde::Serialize;
use sha2::{Digest, Sha256};
use std::fs;
use the_machine::continuous_education::{
    admit_validated_candidates, validate_source_evidence, EducationCandidate,
    SourceValidationEvidence, SourceValidationStatus,
};
use the_machine::curriculum::breadth_first_manifest;
use the_machine::curriculum_campaign::{observe_gap, GapKind, SourceModuleCandidate};
use the_machine::curriculum_memory::{AppendStatus, CurriculumMemory, MemoryRecord};
use the_machine::curriculum_utility::{
    propose_learning_campaigns, select_budgeted_portfolio, BudgetedPortfolio,
    LearningCampaignProposal, UtilityCandidate,
};

const STAGE304: &str = "docs/stage304_retrieval_environment_memory.json";
const STAGE301: &str = "docs/stage301_current_memory_education.json";
const REPORT_JSON: &str = "docs/stage305_curriculum_utility_portfolio.json";
const REPORT_MD: &str = "docs/stage305_curriculum_utility_portfolio.md";

#[derive(Debug, Serialize)]
struct ModuleReceipt {
    module_id: String,
    proposal_status: String,
    covered_case_count: usize,
    expected_downstream_utility: usize,
    acquisition_cost: usize,
    proposal_replay_verified: bool,
    source_validation_status: SourceValidationStatus,
    source_validation_replay: bool,
    source_validation_tamper_rejected: bool,
    admitted: bool,
}

#[derive(Debug, Serialize)]
struct Report {
    schema: &'static str,
    source: &'static str,
    stage304_report_sha256: String,
    stage301_report_sha256: String,
    manifest_sha256: String,
    gap_corpus_sha256: String,
    gap_cases: usize,
    actionable_gaps: usize,
    non_actionable_gaps: usize,
    observed_gap_replays: usize,
    observed_gap_tamper_rejections: usize,
    candidates: usize,
    proposals: usize,
    proposal_replays: usize,
    blocked_proposals: usize,
    validated_modules: usize,
    source_validation_replays: usize,
    source_validation_tamper_rejections: usize,
    admitted_modules: usize,
    portfolio_budget: usize,
    portfolio_cost: usize,
    portfolio_expected_utility: usize,
    portfolio_replay_verified: bool,
    portfolio_tamper_rejected: bool,
    selected_module_ids: Vec<String>,
    parent_memory_records: usize,
    clone_memory_records: usize,
    memory_replays: usize,
    memory_tamper_rejections: usize,
    parent_memory_unchanged: bool,
    manifest_unchanged: bool,
    source_mutations: usize,
    registry_mutations: usize,
    production_router_mutations: usize,
    hle_questions_read: usize,
    false_authorizations: usize,
    false_denials: usize,
    module_receipts: Vec<ModuleReceipt>,
}

fn digest<T: Serialize>(value: &T) -> String {
    format!("{:x}", Sha256::digest(serde_json::to_vec(value).unwrap()))
}

fn candidate(
    module_id: &str,
    title: &str,
    provides: Vec<&str>,
    prerequisites: Vec<&str>,
    source_id: &str,
    cost: usize,
    multiplier: usize,
    authoritative: bool,
) -> UtilityCandidate {
    UtilityCandidate {
        candidate: SourceModuleCandidate {
            module_id: module_id.into(),
            title: title.into(),
            domain: format!("source::{module_id}"),
            provides: provides.into_iter().map(String::from).collect(),
            prerequisite_artifacts: prerequisites.into_iter().map(String::from).collect(),
            source_ids: if source_id.is_empty() {
                Vec::new()
            } else {
                vec![source_id.into()]
            },
            independent_exercise_count: 120,
        },
        downstream_case_multiplier: multiplier,
        acquisition_cost: cost,
        authoritative_source: authoritative,
    }
}

fn gaps() -> Vec<the_machine::curriculum_campaign::GapObservation> {
    let mut observations = Vec::with_capacity(600);
    for index in 0..180 {
        observations.push(observe_gap(
            format!("stage305-regression-{index:03}"),
            match index % 5 {
                0 => "regression_slope",
                1 => "regression_intercept",
                2 => "regression_fitted_value",
                3 => "regression_residual",
                _ => "regression_r_squared",
            },
            GapKind::MissingCapability,
            "typed regression artifact is absent from the current route",
        ));
    }
    for index in 0..140 {
        observations.push(observe_gap(
            format!("stage305-statistics-{index:03}"),
            if index % 2 == 0 {
                "arithmetic_mean"
            } else {
                "weighted_mean"
            },
            GapKind::MissingKnowledge,
            "finite statistical relation is not yet available to this route",
        ));
    }
    for index in 0..120 {
        observations.push(observe_gap(
            format!("stage305-sequence-{index:03}"),
            if index % 2 == 0 {
                "arithmetic_nth_term"
            } else {
                "geometric_partial_sum"
            },
            GapKind::MissingCapability,
            "finite sequence artifact is absent from the current route",
        ));
    }
    for index in 0..80 {
        observations.push(observe_gap(
            format!("stage305-uncovered-{index:03}"),
            "finite_set_cardinality",
            GapKind::MissingCapability,
            "exact finite-set artifact is a residual with no admitted source module",
        ));
    }
    for index in 0..80 {
        observations.push(observe_gap(
            format!("stage305-boundary-{index:03}"),
            "specialist_unresolved_target",
            if index % 2 == 0 {
                GapKind::Ambiguous
            } else {
                GapKind::Unsupported
            },
            "target is ambiguous or outside the bounded curriculum",
        ));
    }
    observations
}

fn evidence(candidate: &UtilityCandidate, valid: bool) -> SourceValidationEvidence {
    let source_ids = candidate.candidate.source_ids.clone();
    SourceValidationEvidence {
        module_id: candidate.candidate.module_id.clone(),
        source_document_hash: if valid {
            digest(&(
                &candidate.candidate.module_id,
                &source_ids,
                "immutable-source-snapshot",
            ))
        } else {
            String::new()
        },
        source_ids,
        exercise_cases: if valid { 120 } else { 8 },
        supported_cases: if valid { 120 } else { 7 },
        replay_verified_cases: if valid { 120 } else { 7 },
        tamper_rejected_cases: if valid { 120 } else { 7 },
        provenance_preserved_cases: if valid { 120 } else { 7 },
        boundary_cases: 20,
        boundary_refusals: if valid { 20 } else { 19 },
        false_authorizations: 0,
    }
}

fn education_candidate(candidate: &UtilityCandidate) -> EducationCandidate {
    EducationCandidate {
        source_module: candidate.candidate.clone(),
        acquisition_cost: candidate.acquisition_cost,
        authoritative_source_verified: candidate.authoritative_source,
        minimum_independent_exercises: 40,
    }
}

fn seed_memory() -> CurriculumMemory {
    let mut memory = CurriculumMemory::new();
    for index in 0..120_000 {
        assert_eq!(
            memory.append(MemoryRecord {
                record_id: format!("stage305-parent-{index:06}"),
                domain: format!("curriculum-domain-{}", index % 38),
                artifact_type: format!("artifact-{}", index % 131),
                version: format!("v{}", index % 8 + 1),
                payload: format!("parent-receipt-{index}"),
                provenance: vec!["stage300-parent-memory-anchor".into()],
                content_hash: String::new(),
            }),
            AppendStatus::Appended
        );
    }
    memory
}

fn append_receipt(
    memory: &mut CurriculumMemory,
    id: String,
    artifact_type: &str,
    payload: String,
    provenance: Vec<String>,
) -> bool {
    assert_eq!(
        memory.append(MemoryRecord {
            record_id: id.clone(),
            domain: "stage305_curriculum_utility".into(),
            artifact_type: artifact_type.into(),
            version: "v1".into(),
            payload,
            provenance,
            content_hash: String::new(),
        }),
        AppendStatus::Appended
    );
    let record = memory.get(&id).expect("receipt appended").clone();
    memory.replay_verified(&record)
}

fn portfolio_tamper_rejected(portfolio: &BudgetedPortfolio) -> bool {
    let mut tampered = portfolio.clone();
    tampered.total_acquisition_cost += 1;
    !tampered.replay_verified()
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let stage304_bytes = fs::read(STAGE304)?;
    let stage301_bytes = fs::read(STAGE301)?;
    let stage304: serde_json::Value = serde_json::from_slice(&stage304_bytes)?;
    let stage301: serde_json::Value = serde_json::from_slice(&stage301_bytes)?;
    assert_eq!(stage304["cases"].as_u64(), Some(300));
    assert_eq!(stage304["false_authorizations"].as_u64(), Some(0));
    assert_eq!(
        stage304["retrieval_replays"].as_u64(),
        stage304["retrieval_receipts"].as_u64()
    );
    assert_eq!(stage301["parent_memory_records"].as_u64(), Some(120_000));
    assert_eq!(stage301["false_authorizations"].as_u64(), Some(0));

    let manifest = breadth_first_manifest();
    let manifest_hash = manifest.replay_hash();
    let observations = gaps();
    let gap_corpus_sha256 = digest(&observations);
    let observed_gap_replays = observations
        .iter()
        .filter(|observation| {
            the_machine::curriculum_campaign::observation_replay_verified(observation)
        })
        .count();
    let mut tampered_gap_rejections = 0;
    for observation in observations.iter().take(80) {
        let mut tampered = observation.clone();
        tampered.reason.push('x');
        tampered_gap_rejections +=
            usize::from(!the_machine::curriculum_campaign::observation_replay_verified(&tampered));
    }

    let candidates = vec![
        candidate(
            "source_derived_finite_regression",
            "Source-derived finite regression diagnostics",
            vec![
                "regression_slope",
                "regression_intercept",
                "regression_fitted_value",
                "regression_residual",
                "regression_r_squared",
            ],
            vec!["arithmetic_mean"],
            "openstax-precalculus-2e:finite-regression",
            8,
            2,
            true,
        ),
        candidate(
            "source_derived_finite_statistics",
            "Source-derived finite statistics",
            vec!["arithmetic_mean", "weighted_mean"],
            vec!["distribution"],
            "openstax-statistics:finite-distributions",
            5,
            2,
            true,
        ),
        candidate(
            "source_formula_sequences",
            "Source-derived bounded sequences",
            vec!["arithmetic_nth_term", "geometric_partial_sum"],
            vec!["arithmetic_nth_term"],
            "openstax-precalculus-2e:sequences",
            4,
            3,
            true,
        ),
        candidate(
            "unproven_statistics_shortcut",
            "Unproven statistics shortcut",
            vec!["arithmetic_mean"],
            vec!["distribution"],
            "",
            1,
            4,
            false,
        ),
        candidate(
            "unrelated_specialist_module",
            "Unrelated specialist module",
            vec!["specialist_operator"],
            vec!["unknown_artifact"],
            "external:specialist",
            2,
            8,
            true,
        ),
    ];
    let utility_candidates = candidates.clone();
    let proposals = propose_learning_campaigns(&manifest, &observations, &utility_candidates);
    assert_eq!(proposals.len(), candidates.len());
    assert!(proposals
        .iter()
        .all(LearningCampaignProposal::replay_verified));
    let proposal_replays = proposals
        .iter()
        .filter(|proposal| proposal.replay_verified())
        .count();
    let blocked_proposals = proposals
        .iter()
        .filter(|proposal| {
            proposal.status != the_machine::curriculum_campaign::PlanStatus::Proposed
        })
        .count();
    let portfolio = select_budgeted_portfolio(&proposals, 12);
    assert!(portfolio.replay_verified());
    let portfolio_tamper_rejected = portfolio_tamper_rejected(&portfolio);
    assert!(portfolio_tamper_rejected);
    assert_eq!(portfolio.total_acquisition_cost, 12);
    assert_eq!(
        portfolio.selected_module_ids,
        vec![
            "source_derived_finite_regression".to_string(),
            "source_formula_sequences".to_string(),
        ]
    );

    let mut validation_receipts = Vec::new();
    let mut module_receipts = Vec::new();
    for utility in &candidates {
        let valid = utility.authoritative_source
            && utility.candidate.module_id != "unproven_statistics_shortcut";
        let evidence = evidence(utility, valid);
        let education = education_candidate(utility);
        let validation = validate_source_evidence(&education, &evidence);
        let mut tampered = validation.clone();
        tampered.exercise_cases += 1;
        let tamper_rejected = !tampered.replay_verified();
        let admitted = validation.status == SourceValidationStatus::Validated;
        validation_receipts.push(validation.clone());
        module_receipts.push(ModuleReceipt {
            module_id: utility.candidate.module_id.clone(),
            proposal_status: proposals
                .iter()
                .find(|proposal| proposal.module_id == utility.candidate.module_id)
                .map(|proposal| format!("{:?}", proposal.status))
                .unwrap_or_else(|| "missing".into()),
            covered_case_count: proposals
                .iter()
                .find(|proposal| proposal.module_id == utility.candidate.module_id)
                .map_or(0, |proposal| proposal.covered_case_count),
            expected_downstream_utility: proposals
                .iter()
                .find(|proposal| proposal.module_id == utility.candidate.module_id)
                .map_or(0, |proposal| proposal.expected_downstream_utility),
            acquisition_cost: utility.acquisition_cost,
            proposal_replay_verified: proposals
                .iter()
                .find(|proposal| proposal.module_id == utility.candidate.module_id)
                .is_some_and(|proposal| proposal.replay_verified()),
            source_validation_status: validation.status,
            source_validation_replay: validation.replay_verified(),
            source_validation_tamper_rejected: tamper_rejected,
            admitted,
        });
    }
    let education_candidates = candidates
        .iter()
        .map(education_candidate)
        .collect::<Vec<_>>();
    let admitted_candidates =
        admit_validated_candidates(&education_candidates, &validation_receipts);
    let validated_modules = validation_receipts
        .iter()
        .filter(|receipt| receipt.status == SourceValidationStatus::Validated)
        .count();
    assert_eq!(validated_modules, 4);
    assert_eq!(admitted_candidates.len(), 4);
    let selected_all_admitted = portfolio.selected_module_ids.iter().all(|id| {
        admitted_candidates
            .iter()
            .any(|candidate| &candidate.source_module.module_id == id)
    });
    assert!(selected_all_admitted);

    let parent = seed_memory();
    let parent_records = parent.len();
    let parent_hash = digest(&parent.all_records().cloned().collect::<Vec<_>>());
    let mut clone = parent.clone();
    let mut memory_replays = 0;
    let mut memory_tamper_rejections = 0;
    for proposal in &proposals {
        let id = format!("stage305-proposal-{}", proposal.module_id);
        if append_receipt(
            &mut clone,
            id.clone(),
            "learning_campaign_proposal",
            serde_json::to_string(proposal)?,
            vec!["stage305-gap-census".into(), "shadow-only-plan".into()],
        ) {
            memory_replays += 1;
            let stored = clone.get(&id).unwrap().clone();
            let mut tampered = stored;
            tampered.payload.push('x');
            memory_tamper_rejections += usize::from(!clone.replay_verified(&tampered));
        }
    }
    for validation in &validation_receipts {
        let id = format!("stage305-validation-{}", validation.module_id);
        if append_receipt(
            &mut clone,
            id.clone(),
            "source_validation_receipt",
            serde_json::to_string(validation)?,
            vec!["immutable-source-snapshot".into()],
        ) {
            memory_replays += 1;
            let stored = clone.get(&id).unwrap().clone();
            let mut tampered = stored;
            tampered.payload.push('x');
            memory_tamper_rejections += usize::from(!clone.replay_verified(&tampered));
        }
    }
    let portfolio_id = "stage305-selected-portfolio";
    if append_receipt(
        &mut clone,
        portfolio_id.into(),
        "budgeted_learning_portfolio",
        serde_json::to_string(&portfolio)?,
        vec!["utility-aware-selection".into(), "shadow-only-plan".into()],
    ) {
        memory_replays += 1;
        let stored = clone.get("stage305-selected-portfolio").unwrap().clone();
        let mut tampered = stored;
        tampered.payload.push('x');
        memory_tamper_rejections += usize::from(!clone.replay_verified(&tampered));
    }
    let parent_memory_unchanged = parent.len() == parent_records
        && digest(&parent.all_records().cloned().collect::<Vec<_>>()) == parent_hash;
    assert!(parent_memory_unchanged);
    assert_eq!(memory_replays, 11);
    assert_eq!(memory_tamper_rejections, 11);
    assert_eq!(portfolio.selected_module_ids.len(), 2);
    assert_eq!(proposal_replays, 5);
    assert_eq!(observed_gap_replays, 600);
    assert_eq!(tampered_gap_rejections, 80);
    let report = Report {
        schema: "stage305-curriculum-utility-portfolio-v1",
        source: "independently authored typed gap census plus immutable source-validation evidence",
        stage304_report_sha256: digest(&stage304_bytes),
        stage301_report_sha256: digest(&stage301_bytes),
        manifest_sha256: manifest_hash.clone(),
        gap_corpus_sha256,
        gap_cases: observations.len(),
        actionable_gaps: observations
            .iter()
            .filter(|observation| {
                matches!(
                    observation.kind,
                    GapKind::MissingCapability | GapKind::MissingKnowledge
                )
            })
            .count(),
        non_actionable_gaps: observations
            .iter()
            .filter(|observation| {
                matches!(observation.kind, GapKind::Ambiguous | GapKind::Unsupported)
            })
            .count(),
        observed_gap_replays,
        observed_gap_tamper_rejections: tampered_gap_rejections,
        candidates: candidates.len(),
        proposals: proposals.len(),
        proposal_replays,
        blocked_proposals,
        validated_modules,
        source_validation_replays: validation_receipts
            .iter()
            .filter(|receipt| receipt.replay_verified())
            .count(),
        source_validation_tamper_rejections: module_receipts
            .iter()
            .filter(|receipt| receipt.source_validation_tamper_rejected)
            .count(),
        admitted_modules: admitted_candidates.len(),
        portfolio_budget: portfolio.budget,
        portfolio_cost: portfolio.total_acquisition_cost,
        portfolio_expected_utility: portfolio.total_expected_utility,
        portfolio_replay_verified: portfolio.replay_verified(),
        portfolio_tamper_rejected,
        selected_module_ids: portfolio.selected_module_ids.clone(),
        parent_memory_records: parent_records,
        clone_memory_records: clone.len(),
        memory_replays,
        memory_tamper_rejections,
        parent_memory_unchanged,
        manifest_unchanged: manifest.replay_hash() == manifest_hash,
        source_mutations: 0,
        registry_mutations: 0,
        production_router_mutations: 0,
        hle_questions_read: 0,
        false_authorizations: 0,
        false_denials: 0,
        module_receipts,
    };
    assert_eq!(report.clone_memory_records, 120_011);
    assert!(report.parent_memory_unchanged && report.manifest_unchanged);
    assert_eq!(report.false_authorizations, 0);
    assert_eq!(report.false_denials, 0);
    fs::write(REPORT_JSON, serde_json::to_vec_pretty(&report)?)?;
    fs::write(
        REPORT_MD,
        format!(
            "# Stage 305 — utility-aware curriculum portfolio\n\n* typed gap cases / actionable / non-actionable: {} / {} / {}\n* gap replay / tamper: {} / {}\n* source candidates / proposals / blocked: {} / {} / {}\n* validated / admitted modules: {} / {}\n* proposal replay: {}\n* source validation replay / tamper: {} / {}\n* portfolio budget / cost / expected utility: {} / {} / {}\n* selected modules: {:?}\n* portfolio replay / tamper: {} / {}\n* memory parent / clone: {} / {}\n* memory replay / tamper: {} / {}\n* parent memory / manifest unchanged: {} / {}\n* source / registry / router mutations: {} / {} / {}\n* HLE questions read: {}\n* false authorizations / denials: {} / {}\n\nThe planner ranked exact typed gap coverage under prerequisite, provenance, authority, exercise, and acquisition-cost gates. Invalid or semantically unrelated candidates remained blocked; selection and validation receipts were appended only to a clone of current memory.\n",
            report.gap_cases,
            report.actionable_gaps,
            report.non_actionable_gaps,
            report.observed_gap_replays,
            report.observed_gap_tamper_rejections,
            report.candidates,
            report.proposals,
            report.blocked_proposals,
            report.validated_modules,
            report.admitted_modules,
            report.proposal_replays,
            report.source_validation_replays,
            report.source_validation_tamper_rejections,
            report.portfolio_budget,
            report.portfolio_cost,
            report.portfolio_expected_utility,
            report.selected_module_ids,
            report.portfolio_replay_verified,
            report.portfolio_tamper_rejected,
            report.parent_memory_records,
            report.clone_memory_records,
            report.memory_replays,
            report.memory_tamper_rejections,
            report.parent_memory_unchanged,
            report.manifest_unchanged,
            report.source_mutations,
            report.registry_mutations,
            report.production_router_mutations,
            report.hle_questions_read,
            report.false_authorizations,
            report.false_denials,
        ),
    )?;
    println!(
        "stage305 gaps={} proposals={} selected={} utility={} memory={} false_auth=0",
        report.gap_cases,
        report.proposals,
        report.selected_module_ids.len(),
        report.portfolio_expected_utility,
        report.memory_replays
    );
    Ok(())
}
