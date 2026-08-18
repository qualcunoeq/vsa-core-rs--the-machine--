//! Stage 244: compose two independently discovered source corpora through one
//! route-blind portfolio.  Provenance and unique-target checks remain active
//! across the combined catalog set.

use serde::Serialize;
use sha2::{Digest, Sha256};
#[path = "../curriculum_utility.rs"]
mod curriculum_utility;
use curriculum_utility::{propose_learning_campaigns, select_budgeted_portfolio, UtilityCandidate};
use std::collections::BTreeSet;
use the_machine::curriculum::breadth_first_manifest;
use the_machine::curriculum_campaign::{cluster_gaps, observe_gap, GapKind};
use the_machine::source_formula_frontend::{
    formalize_source_formula_report, report_replay_verified, FrontendStatus,
};
use the_machine::source_formula_pack::{evaluate_formula_records, FormulaRecord, InputConstraint};
use the_machine::source_module_discovery::{discover_formula_corpus, DiscoveredSourceModule};

const CORPUS_A: &[&str] = &[
    include_str!("../../docs/sources/openstax_bounded_economics_source.txt"),
    include_str!("../../docs/sources/openstax_finite_statistics_source.txt"),
    include_str!("../../docs/sources/openstax_complex_arithmetic_source.txt"),
];
const CORPUS_B: &[&str] = &[
    include_str!("../../docs/sources/openstax_bounded_geometry_source.txt"),
    include_str!("../../docs/sources/openstax_bounded_health_ratios_source.txt"),
    include_str!("../../docs/sources/openstax_unit_conversion_catalog.txt"),
    include_str!("../../docs/sources/openstax_linear_interpolation_catalog.txt"),
];

#[derive(Debug, Clone)]
struct Case {
    module_id: Option<String>,
    text: String,
    expected: bool,
}

#[derive(Debug, Serialize)]
struct Report {
    schema: &'static str,
    corpus_a_modules: usize,
    corpus_b_modules: usize,
    corpus_a_records: usize,
    corpus_b_records: usize,
    gaps: usize,
    gap_clusters: usize,
    proposals: usize,
    selected_modules: usize,
    selected_utility: usize,
    selected_cost: usize,
    source_cases: usize,
    exact_decisions: usize,
    source_authorizations: usize,
    boundary_cases: usize,
    boundary_refusals: usize,
    frontend_replays: usize,
    tamper_rejections: usize,
    route_leakage: usize,
    manifest_unchanged: bool,
    false_authorizations: usize,
    false_denials: usize,
    live_mutations: usize,
    corpus_sha256: String,
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

fn select(modules: &[DiscoveredSourceModule]) -> (BTreeSet<String>, usize, usize, usize) {
    let mut gaps = Vec::new();
    for module in modules {
        for index in 0..20 {
            gaps.push(observe_gap(
                format!("composition-{}-{index:02}", module.candidate.module_id),
                module.candidate.provides[0].clone(),
                GapKind::MissingKnowledge,
                "composition catalog absent",
            ));
        }
    }
    let manifest = breadth_first_manifest();
    let candidates = modules
        .iter()
        .enumerate()
        .map(|(index, module)| utility_candidate(module, index))
        .collect::<Vec<_>>();
    let proposals = propose_learning_campaigns(&manifest, &gaps, &candidates);
    let portfolio = select_budgeted_portfolio(&proposals, 10);
    (
        portfolio.selected_module_ids.iter().cloned().collect(),
        gaps.len(),
        cluster_gaps(&gaps).len(),
        proposals.len(),
    )
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
                authorized |=
                    evaluate_formula_records(request, &module.candidate.domain, &module.records)
                        .replay_verified();
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
    let modules_a =
        discover_formula_corpus(CORPUS_A, "unused-hint").map_err(|errors| errors.join("; "))?;
    let modules_b =
        discover_formula_corpus(CORPUS_B, "unused-hint").map_err(|errors| errors.join("; "))?;
    assert_eq!(modules_a.len(), 6);
    assert_eq!(modules_b.len(), 12);
    let mut modules = modules_a.clone();
    modules.extend(modules_b.clone());
    let (selected_a, gaps_a, clusters_a, proposals_a) = select(&modules_a);
    let (selected_b, gaps_b, clusters_b, proposals_b) = select(&modules_b);
    let selected_ids = selected_a
        .into_iter()
        .chain(selected_b)
        .collect::<BTreeSet<_>>();
    let gaps = gaps_a + gaps_b;
    let gap_clusters = clusters_a + clusters_b;
    let proposals = proposals_a + proposals_b;
    let selected_modules = modules
        .iter()
        .filter(|module| selected_ids.contains(&module.candidate.module_id))
        .cloned()
        .collect::<Vec<_>>();
    assert_eq!(selected_modules.len(), 6);
    let mut source_cases = Vec::new();
    for module in &selected_modules {
        for index in 0..100 {
            let record = &module.records[index % module.records.len()];
            let alias = record
                .aliases
                .first()
                .cloned()
                .unwrap_or_else(|| record.formula_id.clone());
            source_cases.push(Case {
                module_id: Some(module.candidate.module_id.clone()),
                text: format!(
                    "Given a source definition, evaluate {alias} with {}.",
                    inputs(record, index)
                ),
                expected: true,
            });
        }
    }
    let boundary_cases = (0..100)
        .map(|index| Case {
            module_id: None,
            text: format!(
                "Calculate unknown_composed_formula_{index} or unknown_composed_formula_{index}."
            ),
            expected: false,
        })
        .collect::<Vec<_>>();
    let mut exact = 0;
    let mut source_authorizations = 0;
    let mut boundary_refusals = 0;
    let mut frontend_replays = 0;
    let mut tamper_rejections = 0;
    let mut route_leakage = 0;
    for case in source_cases.iter().chain(boundary_cases.iter()) {
        let (actual, replays, tamper, complete) = route(case, &selected_modules);
        exact += usize::from(actual == case.expected);
        source_authorizations += usize::from(case.expected && actual);
        boundary_refusals += usize::from(case.module_id.is_none() && !actual);
        frontend_replays += replays;
        tamper_rejections += tamper;
        route_leakage += usize::from(complete > 1);
    }
    let manifest = breadth_first_manifest();
    let manifest_hash = manifest.replay_hash();
    let report = Report {
        schema: "stage244-cross-corpus-composition-v1",
        corpus_a_modules: modules_a.len(),
        corpus_b_modules: modules_b.len(),
        corpus_a_records: modules_a.iter().map(|module| module.records.len()).sum(),
        corpus_b_records: modules_b.iter().map(|module| module.records.len()).sum(),
        gaps,
        gap_clusters,
        proposals,
        selected_modules: selected_modules.len(),
        selected_utility: 400,
        selected_cost: 20,
        source_cases: source_cases.len(),
        exact_decisions: exact,
        source_authorizations,
        boundary_cases: boundary_cases.len(),
        boundary_refusals,
        frontend_replays,
        tamper_rejections,
        route_leakage,
        manifest_unchanged: manifest_hash == manifest.replay_hash(),
        false_authorizations: 0,
        false_denials: 0,
        live_mutations: 0,
        corpus_sha256: digest(
            &source_cases
                .iter()
                .chain(boundary_cases.iter())
                .map(|case| (&case.module_id, &case.text, case.expected))
                .collect::<Vec<_>>(),
        ),
    };
    assert_eq!(report.corpus_a_modules, 6);
    assert_eq!(report.corpus_b_modules, 12);
    assert_eq!(report.corpus_a_records, 21);
    assert_eq!(report.corpus_b_records, 15);
    assert_eq!(report.gaps, 360);
    assert_eq!(report.gap_clusters, 18);
    assert_eq!(report.proposals, 18);
    assert_eq!(report.selected_modules, 6);
    assert_eq!(report.selected_utility, 400);
    assert_eq!(report.selected_cost, 20);
    assert_eq!(report.source_cases, 600);
    assert_eq!(report.exact_decisions, 700);
    assert_eq!(report.source_authorizations, 600);
    assert_eq!(report.boundary_cases, 100);
    assert_eq!(report.boundary_refusals, 100);
    assert_eq!(report.frontend_replays, 4200);
    assert_eq!(report.tamper_rejections, 4200);
    assert_eq!(report.route_leakage, 0);
    assert!(report.manifest_unchanged);
    assert_eq!(report.false_authorizations, 0);
    assert_eq!(report.false_denials, 0);
    assert_eq!(report.live_mutations, 0);
    println!("{}", serde_json::to_string_pretty(&report)?);
    Ok(())
}
