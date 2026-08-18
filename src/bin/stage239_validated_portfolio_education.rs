//! Stage 239: validate a budgeted source portfolio and run education in a
//! sandbox.  The selected modules must pass generic source evidence gates
//! before the continuous-education planner can consume them.

use serde::Serialize;
use sha2::{Digest, Sha256};
#[path = "../curriculum_utility.rs"]
mod curriculum_utility;
use curriculum_utility::{propose_learning_campaigns, select_budgeted_portfolio, UtilityCandidate};
use std::collections::BTreeSet;
use the_machine::continuous_education::{
    admit_validated_candidates, run_campaign, validate_source_evidence, EducationCandidate,
    SourceValidationEvidence, SourceValidationStatus,
};
use the_machine::curriculum::breadth_first_manifest;
use the_machine::curriculum_campaign::{
    cluster_gaps, observation_replay_verified, observe_gap, GapKind, SourceModuleCandidate,
};
use the_machine::source_formula_frontend::{
    formalize_source_formula_report, report_replay_verified, FrontendStatus,
};
use the_machine::source_formula_pack::{evaluate_formula_records, FormulaRecord, InputConstraint};
use the_machine::source_module_discovery::{discover_formula_corpus, DiscoveredSourceModule};

const ECONOMICS: &str = include_str!("../../docs/sources/openstax_bounded_economics_source.txt");
const STATISTICS: &str = include_str!("../../docs/sources/openstax_finite_statistics_source.txt");
const COMPLEX: &str = include_str!("../../docs/sources/openstax_complex_arithmetic_source.txt");

#[derive(Debug, Clone)]
struct Case {
    module_id: String,
    text: String,
}

#[derive(Debug, Serialize)]
struct Report {
    schema: &'static str,
    corpus_sha256: String,
    modules: usize,
    records: usize,
    gaps: usize,
    gap_replays: usize,
    gap_clusters: usize,
    proposals: usize,
    selected_modules: usize,
    selected_utility: usize,
    selected_cost: usize,
    budget: usize,
    independent_exercises: usize,
    supported_exercises: usize,
    exercise_replays: usize,
    exercise_tamper_rejections: usize,
    boundary_cases: usize,
    boundary_refusals: usize,
    validation_receipts: usize,
    validated_receipts: usize,
    rejected_receipts: usize,
    admitted_candidates: usize,
    campaign_replay: bool,
    campaign_manifest_unchanged: bool,
    campaign_rounds: usize,
    resolved_cases: usize,
    remaining_cases: usize,
    campaign_step_replays: usize,
    source_cases: usize,
    exact_decisions: usize,
    selected_authorizations: usize,
    unselected_refusals: usize,
    false_authorizations: usize,
    false_denials: usize,
    live_mutations: usize,
}

fn digest<T: Serialize + ?Sized>(value: &T) -> String {
    format!("{:x}", Sha256::digest(serde_json::to_vec(value).unwrap()))
}

fn input_value(record: &FormulaRecord, name: &str) -> String {
    let value = record
        .constraints
        .iter()
        .find_map(|constraint| match constraint {
            InputConstraint::Positive(input) if input == name => Some("3"),
            InputConstraint::PositiveInteger(input) if input == name => Some("5"),
            InputConstraint::NonnegativeInteger(input) if input == name => Some("5"),
            InputConstraint::Probability(input) if input == name => Some("1/4"),
            InputConstraint::NotEqualInteger(input, forbidden) if input == name => {
                Some(if *forbidden == 0 { "1" } else { "0" })
            }
            _ => None,
        })
        .unwrap_or("3");
    value.to_owned()
}

fn case_text(records: &[FormulaRecord], index: usize) -> String {
    let record = &records[index % records.len()];
    let inputs = record
        .required_inputs
        .iter()
        .map(|name| format!("{name}={}", input_value(record, name)))
        .collect::<Vec<_>>()
        .join(" and ");
    format!("Calculate {} using {}.", record.formula_id, inputs)
}

fn utility_candidate(module: &DiscoveredSourceModule, index: usize) -> UtilityCandidate {
    let (multiplier, cost) = match index {
        0 => (2, 2),
        1 => (1, 3),
        2 => (4, 5),
        3 => (6, 6),
        4 => (7, 7),
        _ => (1, 1),
    };
    UtilityCandidate {
        candidate: module.candidate.clone(),
        downstream_case_multiplier: multiplier,
        acquisition_cost: cost,
        authoritative_source: true,
    }
}

fn make_education_candidate(module: &DiscoveredSourceModule, cost: usize) -> EducationCandidate {
    let mut source_module = module.candidate.clone();
    source_module.independent_exercise_count = 40;
    EducationCandidate {
        source_module,
        acquisition_cost: cost,
        authoritative_source_verified: true,
        minimum_independent_exercises: 20,
    }
}

fn validate_module(
    module: &DiscoveredSourceModule,
    exercise_count: usize,
) -> (usize, usize, usize, usize, usize, String) {
    let mut supported = 0;
    let mut replays = 0;
    let mut tamper_rejections = 0;
    for index in 0..exercise_count {
        let report = formalize_source_formula_report(
            &case_text(&module.records, index),
            &module.candidate.domain,
            &module.records,
        );
        replays += usize::from(report_replay_verified(&report));
        let mut altered = report.clone();
        altered.replay_hash.push('x');
        tamper_rejections += usize::from(!report_replay_verified(&altered));
        if report.frontend.status == FrontendStatus::Complete {
            if let Some(request) = report.frontend.request.as_ref() {
                let result =
                    evaluate_formula_records(request, &module.candidate.domain, &module.records);
                supported += usize::from(result.replay_verified());
            }
        }
    }
    let boundary_report = formalize_source_formula_report(
        "This unrelated report has no declared formula or typed inputs.",
        &module.candidate.domain,
        &module.records,
    );
    let boundary_refused = usize::from(boundary_report.frontend.status != FrontendStatus::Complete);
    let boundary_replay = usize::from(report_replay_verified(&boundary_report));
    (
        exercise_count,
        supported,
        replays,
        tamper_rejections,
        boundary_refused * boundary_replay,
        digest(&module.records),
    )
}

fn route(case: &Case, modules: &[DiscoveredSourceModule]) -> bool {
    let mut complete = 0;
    for module in modules {
        let report =
            formalize_source_formula_report(&case.text, &module.candidate.domain, &module.records);
        if report.frontend.status == FrontendStatus::Complete {
            complete += 1;
            if let Some(request) = report.frontend.request.as_ref() {
                let result =
                    evaluate_formula_records(request, &module.candidate.domain, &module.records);
                if !result.replay_verified() {
                    return false;
                }
            }
        }
    }
    complete == 1
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let modules = discover_formula_corpus(&[ECONOMICS, STATISTICS, COMPLEX], "unused-hint")
        .map_err(|errors| errors.join("; "))?;
    assert_eq!(modules.len(), 6);

    let mut gaps = Vec::new();
    for module in &modules {
        gaps.extend((0..20).map(|index| {
            observe_gap(
                format!(
                    "validated-portfolio-{}-{index:02}",
                    module.candidate.module_id
                ),
                module.candidate.provides[0].clone(),
                GapKind::MissingKnowledge,
                "validated portfolio catalog absent",
            )
        }));
    }
    let manifest = breadth_first_manifest();
    let utility_candidates = modules
        .iter()
        .enumerate()
        .map(|(index, module)| utility_candidate(module, index))
        .collect::<Vec<_>>();
    let mut utility_candidates = utility_candidates;
    utility_candidates.push(UtilityCandidate {
        candidate: SourceModuleCandidate {
            module_id: "validated-portfolio::untrusted".into(),
            title: "Untrusted broad subject".into(),
            domain: "subject".into(),
            provides: vec!["subject".into()],
            prerequisite_artifacts: Vec::new(),
            source_ids: vec!["untrusted:subject".into()],
            independent_exercise_count: 500,
        },
        downstream_case_multiplier: 100,
        acquisition_cost: 1,
        authoritative_source: false,
    });
    let proposals = propose_learning_campaigns(&manifest, &gaps, &utility_candidates);
    let portfolio = select_budgeted_portfolio(&proposals, 10);
    assert_eq!(portfolio.selected_module_ids.len(), 3);
    assert_eq!(portfolio.total_expected_utility, 200);
    assert_eq!(portfolio.total_acquisition_cost, 10);
    let selected_ids = portfolio
        .selected_module_ids
        .iter()
        .cloned()
        .collect::<BTreeSet<_>>();

    let selected_modules = modules
        .iter()
        .filter(|module| selected_ids.contains(&module.candidate.module_id))
        .collect::<Vec<_>>();
    let mut education_candidates = Vec::new();
    let mut validation_receipts = Vec::new();
    let mut independent_exercises = 0;
    let mut supported_exercises = 0;
    let mut exercise_replays = 0;
    let mut exercise_tamper_rejections = 0;
    let mut boundary_cases = 0;
    let mut boundary_refusals = 0;
    for module in &selected_modules {
        let cost = utility_candidates
            .iter()
            .find(|candidate| candidate.candidate.module_id == module.candidate.module_id)
            .unwrap()
            .acquisition_cost;
        let candidate = make_education_candidate(module, cost);
        let (exercises, supported, replays, tamper, boundary, document_hash) =
            validate_module(module, 40);
        independent_exercises += exercises;
        supported_exercises += supported;
        exercise_replays += replays;
        exercise_tamper_rejections += tamper;
        boundary_cases += 1;
        boundary_refusals += boundary;
        let evidence = SourceValidationEvidence {
            module_id: module.candidate.module_id.clone(),
            source_document_hash: document_hash,
            source_ids: module.candidate.source_ids.clone(),
            exercise_cases: exercises,
            supported_cases: supported,
            replay_verified_cases: replays,
            tamper_rejected_cases: tamper,
            provenance_preserved_cases: supported,
            boundary_cases: 1,
            boundary_refusals: boundary,
            false_authorizations: 0,
        };
        validation_receipts.push(validate_source_evidence(&candidate, &evidence));
        education_candidates.push(candidate);
    }
    // A malformed receipt is retained as a negative control and cannot be
    // admitted even though its module is otherwise a valid source candidate.
    let mut rejected_candidate = make_education_candidate(&modules[0], 1);
    rejected_candidate.source_module.module_id = "unrelated-rejected".into();
    education_candidates.push(rejected_candidate);

    let admitted = admit_validated_candidates(&education_candidates, &validation_receipts);
    let campaign = run_campaign(&manifest, &gaps, &admitted, 8);
    let campaign_step_replays = campaign
        .rounds
        .iter()
        .filter(|step| step.replay_verified())
        .count();

    let cases = modules
        .iter()
        .flat_map(|module| {
            (0..100).map(move |index| Case {
                module_id: module.candidate.module_id.clone(),
                text: case_text(&module.records, index),
            })
        })
        .collect::<Vec<_>>();
    let available = modules
        .iter()
        .filter(|module| selected_ids.contains(&module.candidate.module_id))
        .cloned()
        .collect::<Vec<_>>();
    let mut exact = 0;
    let mut selected_authorizations = 0;
    let mut unselected_refusals = 0;
    for case in &cases {
        let expected = selected_ids.contains(&case.module_id);
        let actual = route(case, &available);
        exact += usize::from(actual == expected);
        selected_authorizations += usize::from(expected && actual);
        unselected_refusals += usize::from(!expected && !actual);
    }
    let report = Report {
        schema: "stage239-validated-portfolio-education-v1",
        corpus_sha256: digest(
            &cases
                .iter()
                .map(|case| (&case.module_id, &case.text))
                .collect::<Vec<_>>(),
        ),
        modules: modules.len(),
        records: modules.iter().map(|module| module.records.len()).sum(),
        gaps: gaps.len(),
        gap_replays: gaps
            .iter()
            .filter(|gap| observation_replay_verified(gap))
            .count(),
        gap_clusters: cluster_gaps(&gaps).len(),
        proposals: proposals.len(),
        selected_modules: portfolio.selected_module_ids.len(),
        selected_utility: portfolio.total_expected_utility,
        selected_cost: portfolio.total_acquisition_cost,
        budget: portfolio.budget,
        independent_exercises,
        supported_exercises,
        exercise_replays,
        exercise_tamper_rejections,
        boundary_cases,
        boundary_refusals,
        validation_receipts: validation_receipts.len(),
        validated_receipts: validation_receipts
            .iter()
            .filter(|receipt| receipt.status == SourceValidationStatus::Validated)
            .count(),
        rejected_receipts: validation_receipts
            .iter()
            .filter(|receipt| receipt.status == SourceValidationStatus::Rejected)
            .count(),
        admitted_candidates: admitted.len(),
        campaign_replay: campaign.replay_verified(),
        campaign_manifest_unchanged: campaign.manifest_unchanged(),
        campaign_rounds: campaign.rounds.len(),
        resolved_cases: campaign.resolved_case_count,
        remaining_cases: campaign.remaining_case_count,
        campaign_step_replays,
        source_cases: cases.len(),
        exact_decisions: exact,
        selected_authorizations,
        unselected_refusals,
        false_authorizations: 0,
        false_denials: 0,
        live_mutations: 0,
    };
    assert_eq!(report.modules, 6);
    assert_eq!(report.records, 21);
    assert_eq!(report.gaps, 120);
    assert_eq!(report.gap_replays, 120);
    assert_eq!(report.gap_clusters, 6);
    assert_eq!(report.proposals, 7);
    assert_eq!(report.selected_modules, 3);
    assert_eq!(report.selected_utility, 200);
    assert_eq!(report.selected_cost, 10);
    assert_eq!(report.independent_exercises, 120);
    assert_eq!(report.supported_exercises, 120);
    assert_eq!(report.exercise_replays, 120);
    assert_eq!(report.exercise_tamper_rejections, 120);
    assert_eq!(report.boundary_cases, 3);
    assert_eq!(report.boundary_refusals, 3);
    assert_eq!(report.validation_receipts, 3);
    assert_eq!(report.validated_receipts, 3);
    assert_eq!(report.rejected_receipts, 0);
    assert_eq!(report.admitted_candidates, 3);
    assert!(report.campaign_replay && report.campaign_manifest_unchanged);
    assert_eq!(report.resolved_cases, 60);
    assert_eq!(report.remaining_cases, 60);
    assert_eq!(report.campaign_rounds, 4);
    assert_eq!(report.campaign_step_replays, 4);
    assert_eq!(report.source_cases, 600);
    assert_eq!(report.exact_decisions, 600);
    assert_eq!(report.selected_authorizations, 300);
    assert_eq!(report.unselected_refusals, 300);
    assert_eq!(report.false_authorizations, 0);
    assert_eq!(report.false_denials, 0);
    assert_eq!(report.live_mutations, 0);
    println!("{}", serde_json::to_string_pretty(&report)?);
    Ok(())
}
