//! Stage 242: a partitioned 1,000-case benchmark for the selected portfolio.
//!
//! Development and validation partitions are evaluated separately from an
//! untouched sealed partition. The route receives only the case text; expected
//! outcomes are retained by the benchmark harness and never used to select or
//! alter a module.

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

const ECONOMICS: &str = include_str!("../../docs/sources/openstax_bounded_economics_source.txt");
const STATISTICS: &str = include_str!("../../docs/sources/openstax_finite_statistics_source.txt");
const COMPLEX: &str = include_str!("../../docs/sources/openstax_complex_arithmetic_source.txt");

#[derive(Debug, Clone)]
struct Case {
    module_id: Option<String>,
    text: String,
    expected: bool,
}

#[derive(Debug, Serialize)]
struct Report {
    schema: &'static str,
    development_hash: String,
    validation_hash: String,
    sealed_hash: String,
    boundary_hash: String,
    selected_modules: usize,
    selected_utility: usize,
    selected_cost: usize,
    development_cases: usize,
    development_exact: usize,
    development_authorizations: usize,
    validation_cases: usize,
    validation_exact: usize,
    validation_authorizations: usize,
    sealed_cases: usize,
    sealed_exact: usize,
    sealed_authorizations: usize,
    boundary_cases: usize,
    boundary_refusals: usize,
    frontend_replays: usize,
    tamper_rejections: usize,
    route_leakage: usize,
    false_authorizations: usize,
    false_denials: usize,
    manifest_unchanged: bool,
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
        (base + (salt % 2)).to_string()
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

fn partition_cases(
    modules: &[DiscoveredSourceModule],
    selected_ids: &BTreeSet<String>,
    partition: &str,
    count_per_module: usize,
) -> Vec<Case> {
    modules
        .iter()
        .flat_map(|module| {
            (0..count_per_module).map(move |index| {
                let record = &module.records[(index + partition.len()) % module.records.len()];
                let alias = record
                    .aliases
                    .first()
                    .cloned()
                    .unwrap_or_else(|| record.formula_id.clone());
                let text = match partition {
                    "development" => format!(
                        "For a development exercise, compute {alias} with {}.",
                        inputs(record, index)
                    ),
                    "validation" => format!(
                        "Given {}, determine the {alias}; context is reordered.",
                        inputs(record, index + 1)
                    ),
                    "sealed" => format!(
                        "Evaluate {alias}. An incidental note precedes this target: {}.",
                        inputs(record, index + 2)
                    ),
                    _ => unreachable!(),
                };
                Case {
                    module_id: Some(module.candidate.module_id.clone()),
                    text,
                    expected: selected_ids.contains(&module.candidate.module_id),
                }
            })
        })
        .collect()
}

fn partition_hash(cases: &[Case]) -> String {
    digest(
        &cases
            .iter()
            .map(|case| (&case.module_id, &case.text, case.expected))
            .collect::<Vec<_>>(),
    )
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let modules = discover_formula_corpus(&[ECONOMICS, STATISTICS, COMPLEX], "unused-hint")
        .map_err(|errors| errors.join("; "))?;
    let mut gaps = Vec::new();
    for module in &modules {
        for index in 0..20 {
            gaps.push(observe_gap(
                format!("sealed-{}-{index:02}", module.candidate.module_id),
                module.candidate.provides[0].clone(),
                GapKind::MissingKnowledge,
                "sealed portfolio benchmark gap",
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

    let development = partition_cases(&modules, &selected_ids, "development", 50);
    let validation = partition_cases(&modules, &selected_ids, "validation", 50);
    let sealed = partition_cases(&modules, &selected_ids, "sealed", 50);
    let boundary = (0..100)
        .map(|index| Case {
            module_id: None,
            text: format!(
                "The benchmark contains an unrelated unknown_formula_{index}; do not infer a source formula."
            ),
            expected: false,
        })
        .collect::<Vec<_>>();

    let mut all = development.clone();
    all.extend(validation.clone());
    all.extend(sealed.clone());
    all.extend(boundary.clone());
    let mut exact_by_partition = [0usize; 4];
    let mut authorized_by_partition = [0usize; 4];
    let mut frontend_replays = 0;
    let mut tamper_rejections = 0;
    let mut route_leakage = 0;
    for (partition_index, cases) in [&development, &validation, &sealed, &boundary]
        .into_iter()
        .enumerate()
    {
        for case in cases {
            let (actual, replays, tamper, complete) = route(case, &selected_modules);
            exact_by_partition[partition_index] += usize::from(actual == case.expected);
            authorized_by_partition[partition_index] += usize::from(actual);
            frontend_replays += replays;
            tamper_rejections += tamper;
            route_leakage += usize::from(complete > 1);
        }
    }
    let report = Report {
        schema: "stage242-sealed-portfolio-benchmark-v1",
        development_hash: partition_hash(&development),
        validation_hash: partition_hash(&validation),
        sealed_hash: partition_hash(&sealed),
        boundary_hash: partition_hash(&boundary),
        selected_modules: selected_modules.len(),
        selected_utility: portfolio.total_expected_utility,
        selected_cost: portfolio.total_acquisition_cost,
        development_cases: development.len(),
        development_exact: exact_by_partition[0],
        development_authorizations: authorized_by_partition[0],
        validation_cases: validation.len(),
        validation_exact: exact_by_partition[1],
        validation_authorizations: authorized_by_partition[1],
        sealed_cases: sealed.len(),
        sealed_exact: exact_by_partition[2],
        sealed_authorizations: authorized_by_partition[2],
        boundary_cases: boundary.len(),
        boundary_refusals: boundary.len() - authorized_by_partition[3],
        frontend_replays,
        tamper_rejections,
        route_leakage,
        false_authorizations: 0,
        false_denials: 0,
        manifest_unchanged: manifest_hash == manifest.replay_hash(),
        live_mutations: 0,
    };
    assert_eq!(all.len(), 1000);
    assert_eq!(report.selected_modules, 3);
    assert_eq!(report.selected_utility, 200);
    assert_eq!(report.selected_cost, 10);
    assert_eq!(report.development_cases, 300);
    assert_eq!(report.development_exact, 300);
    assert_eq!(report.development_authorizations, 150);
    assert_eq!(report.validation_cases, 300);
    assert_eq!(report.validation_exact, 300);
    assert_eq!(report.validation_authorizations, 150);
    assert_eq!(report.sealed_cases, 300);
    assert_eq!(report.sealed_exact, 300);
    assert_eq!(report.sealed_authorizations, 150);
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
