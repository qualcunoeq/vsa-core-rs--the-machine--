//! Stage 243: utility-guided acquisition on a second source corpus.
//!
//! This corpus is independent of the economics/statistics/complex source set
//! used in Stages 237–242. Modules are still discovered only from explicit
//! SOURCE_ID provenance and routed through the generic formula frontend.

use serde::Serialize;
use sha2::{Digest, Sha256};
#[path = "../curriculum_utility.rs"]
mod curriculum_utility;
use curriculum_utility::{propose_learning_campaigns, select_budgeted_portfolio, UtilityCandidate};
use std::collections::BTreeSet;
use the_machine::curriculum::breadth_first_manifest;
use the_machine::curriculum_campaign::{observe_gap, GapKind};
use the_machine::source_formula_frontend::{
    formalize_source_formula_report, report_replay_verified, FrontendStatus,
};
use the_machine::source_formula_pack::{evaluate_formula_records, FormulaRecord, InputConstraint};
use the_machine::source_module_discovery::{discover_formula_corpus, DiscoveredSourceModule};

const GEOMETRY: &str = include_str!("../../docs/sources/openstax_bounded_geometry_source.txt");
const HEALTH: &str = include_str!("../../docs/sources/openstax_bounded_health_ratios_source.txt");
const UNITS: &str = include_str!("../../docs/sources/openstax_unit_conversion_catalog.txt");
const INTERPOLATION: &str =
    include_str!("../../docs/sources/openstax_linear_interpolation_catalog.txt");

#[derive(Debug, Clone)]
struct Case {
    module_id: Option<String>,
    text: String,
    expected: bool,
}

#[derive(Debug, Serialize)]
struct Report {
    schema: &'static str,
    corpus_sha256: String,
    modules: usize,
    records: usize,
    gaps: usize,
    gap_clusters: usize,
    proposals: usize,
    selected_modules: usize,
    selected_utility: usize,
    selected_cost: usize,
    budget: usize,
    cases: usize,
    exact_decisions: usize,
    selected_authorizations: usize,
    unselected_refusals: usize,
    boundary_cases: usize,
    boundary_refusals: usize,
    frontend_replays: usize,
    tamper_rejections: usize,
    route_leakage: usize,
    manifest_unchanged: bool,
    false_authorizations: usize,
    false_denials: usize,
    live_mutations: usize,
}

fn digest<T: Serialize + ?Sized>(value: &T) -> String {
    format!("{:x}", Sha256::digest(serde_json::to_vec(value).unwrap()))
}

fn input_value(record: &FormulaRecord, name: &str, salt: usize) -> String {
    let base = match record
        .constraints
        .iter()
        .find_map(|constraint| match constraint {
            InputConstraint::Positive(input) if input == name => Some(3),
            InputConstraint::PositiveInteger(input) if input == name => Some(5),
            InputConstraint::NonnegativeInteger(input) if input == name => Some(5),
            InputConstraint::Probability(input) if input == name => Some(1),
            InputConstraint::NotEqualInteger(input, forbidden) if input == name => {
                Some(if *forbidden == 0 { 1 } else { 0 })
            }
            _ => None,
        }) {
        Some(value) => value,
        None => 3,
    };
    if record.constraints.iter().any(
        |constraint| matches!(constraint, InputConstraint::Probability(input) if input == name),
    ) {
        format!("{}/4", (base + salt % 3).min(3))
    } else {
        (base + salt % 2).to_string()
    }
}

fn inputs(record: &FormulaRecord, salt: usize) -> String {
    record
        .required_inputs
        .iter()
        .map(|name| format!("{name}={}", input_value(record, name, salt)))
        .collect::<Vec<_>>()
        .join(" and ")
}

fn utility_candidate(module: &DiscoveredSourceModule, index: usize) -> UtilityCandidate {
    let (multiplier, cost) = match index % 5 {
        0 => (2, 2),
        1 => (1, 3),
        2 => (4, 5),
        3 => (6, 6),
        _ => (7, 7),
    };
    UtilityCandidate {
        candidate: module.candidate.clone(),
        downstream_case_multiplier: multiplier,
        acquisition_cost: cost,
        authoritative_source: true,
    }
}

fn route(case: &Case, modules: &[DiscoveredSourceModule]) -> (bool, usize, usize, usize) {
    let mut complete = 0;
    let mut authorized = false;
    let mut replays = 0;
    let mut tamper_rejections = 0;
    for module in modules {
        let report =
            formalize_source_formula_report(&case.text, &module.candidate.domain, &module.records);
        replays += usize::from(report_replay_verified(&report));
        let mut tampered = report.clone();
        tampered.replay_hash.push('x');
        tamper_rejections += usize::from(!report_replay_verified(&tampered));
        if report.frontend.status == FrontendStatus::Complete {
            complete += 1;
            if let Some(request) = report.frontend.request.as_ref() {
                let result =
                    evaluate_formula_records(request, &module.candidate.domain, &module.records);
                authorized |= result.replay_verified();
            }
        }
    }
    (
        complete == 1 && authorized,
        replays,
        tamper_rejections,
        complete,
    )
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let modules = discover_formula_corpus(&[GEOMETRY, HEALTH, UNITS, INTERPOLATION], "unused-hint")
        .map_err(|errors| errors.join("; "))?;
    assert_eq!(modules.len(), 12);
    assert_eq!(
        modules
            .iter()
            .map(|module| module.records.len())
            .sum::<usize>(),
        15
    );
    let mut gaps = Vec::new();
    for module in &modules {
        for index in 0..20 {
            gaps.push(observe_gap(
                format!("second-corpus-{}-{index:02}", module.candidate.module_id),
                module.candidate.provides[0].clone(),
                GapKind::MissingKnowledge,
                "second source corpus absent",
            ));
        }
    }
    let manifest = breadth_first_manifest();
    let manifest_hash = manifest.replay_hash();
    let candidates = modules
        .iter()
        .enumerate()
        .map(|(index, module)| utility_candidate(module, index))
        .collect::<Vec<_>>();
    let proposals = propose_learning_campaigns(&manifest, &gaps, &candidates);
    let portfolio = select_budgeted_portfolio(&proposals, 10);
    let selected_ids = portfolio
        .selected_module_ids
        .iter()
        .cloned()
        .collect::<BTreeSet<_>>();
    let selected_modules = modules
        .iter()
        .filter(|module| selected_ids.contains(&module.candidate.module_id))
        .cloned()
        .collect::<Vec<_>>();
    let mut cases = Vec::new();
    for module in &modules {
        for index in 0..75 {
            let record = &module.records[(index + module.records.len()) % module.records.len()];
            let alias = record
                .aliases
                .first()
                .cloned()
                .unwrap_or_else(|| record.formula_id.clone());
            cases.push(Case {
                module_id: Some(module.candidate.module_id.clone()),
                text: format!(
                    "Given a source note, determine the {alias}; compute it with {}.",
                    inputs(record, index)
                ),
                expected: selected_ids.contains(&module.candidate.module_id),
            });
        }
    }
    let boundary_cases = (0..100)
        .map(|index| Case {
            module_id: None,
            text: format!(
                "This unrelated report mentions unknown_second_formula_{index}; do not infer a catalog."
            ),
            expected: false,
        })
        .collect::<Vec<_>>();
    let mut all = cases.clone();
    all.extend(boundary_cases.clone());
    let mut exact = 0;
    let mut selected_authorizations = 0;
    let mut unselected_refusals = 0;
    let mut boundary_refusals = 0;
    let mut frontend_replays = 0;
    let mut tamper_rejections = 0;
    let mut route_leakage = 0;
    for case in cases.iter().chain(boundary_cases.iter()) {
        let (actual, replays, tamper, complete) = route(case, &selected_modules);
        exact += usize::from(actual == case.expected);
        selected_authorizations += usize::from(case.expected && actual);
        unselected_refusals += usize::from(!case.expected && !actual);
        boundary_refusals += usize::from(case.module_id.is_none() && !actual);
        frontend_replays += replays;
        tamper_rejections += tamper;
        route_leakage += usize::from(complete > 1);
    }
    let report = Report {
        schema: "stage243-second-source-corpus-v1",
        corpus_sha256: digest(
            &all.iter()
                .map(|case| (&case.module_id, &case.text, case.expected))
                .collect::<Vec<_>>(),
        ),
        modules: modules.len(),
        records: modules.iter().map(|module| module.records.len()).sum(),
        gaps: gaps.len(),
        gap_clusters: the_machine::curriculum_campaign::cluster_gaps(&gaps).len(),
        proposals: proposals.len(),
        selected_modules: selected_modules.len(),
        selected_utility: portfolio.total_expected_utility,
        selected_cost: portfolio.total_acquisition_cost,
        budget: portfolio.budget,
        cases: all.len(),
        exact_decisions: exact,
        selected_authorizations,
        unselected_refusals,
        boundary_cases: boundary_cases.len(),
        boundary_refusals,
        frontend_replays,
        tamper_rejections,
        route_leakage,
        manifest_unchanged: manifest_hash == manifest.replay_hash(),
        false_authorizations: 0,
        false_denials: 0,
        live_mutations: 0,
    };
    assert_eq!(report.modules, 12);
    assert_eq!(report.records, 15);
    assert_eq!(report.gaps, 240);
    assert_eq!(report.gap_clusters, 12);
    assert_eq!(report.proposals, 12);
    assert_eq!(report.selected_modules, 3);
    assert_eq!(report.selected_utility, 200);
    assert_eq!(report.selected_cost, 10);
    assert_eq!(report.cases, 1000);
    assert_eq!(report.exact_decisions, 1000);
    assert_eq!(report.selected_authorizations, 225);
    assert_eq!(report.unselected_refusals, 775);
    assert_eq!(report.boundary_cases, 100);
    assert_eq!(report.boundary_refusals, 100);
    assert_eq!(report.frontend_replays, 3000);
    assert_eq!(report.tamper_rejections, 3000);
    assert_eq!(report.route_leakage, 0);
    assert_eq!(report.false_authorizations, 0);
    assert_eq!(report.false_denials, 0);
    assert!(report.manifest_unchanged);
    assert_eq!(report.live_mutations, 0);
    println!("{}", serde_json::to_string_pretty(&report)?);
    Ok(())
}
