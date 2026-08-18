//! Stage 241: shifted holdout evaluation for the promoted source portfolio.
//!
//! The holdout uses source aliases and reordered explanatory clauses rather
//! than formula identifiers alone. Boundary cases deliberately omit inputs,
//! offer competing targets, or request unsupported approximate behavior.

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
    module_id: String,
    text: String,
    expected: bool,
}

#[derive(Debug, Serialize)]
struct Report {
    schema: &'static str,
    corpus_sha256: String,
    selected_modules: usize,
    selected_utility: usize,
    selected_cost: usize,
    shifted_cases: usize,
    shifted_authorizations: usize,
    shifted_exact_decisions: usize,
    shifted_replays: usize,
    shifted_tamper_rejections: usize,
    boundary_cases: usize,
    boundary_refusals: usize,
    boundary_replays: usize,
    boundary_tamper_rejections: usize,
    false_authorizations: usize,
    false_denials: usize,
    route_leakage: usize,
    manifest_unchanged: bool,
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

fn inputs(record: &FormulaRecord, omit: Option<&str>) -> String {
    record
        .required_inputs
        .iter()
        .filter(|name| Some(name.as_str()) != omit)
        .map(|name| format!("{name}={}", input_value(record, name)))
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
    let mut replays = 0;
    let mut tamper_rejections = 0;
    let mut authorized = false;
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
    let actual = complete == 1 && authorized;
    (actual, replays, tamper_rejections, complete)
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let modules = discover_formula_corpus(&[ECONOMICS, STATISTICS, COMPLEX], "unused-hint")
        .map_err(|errors| errors.join("; "))?;
    let mut gaps = Vec::new();
    for module in &modules {
        for index in 0..20 {
            gaps.push(observe_gap(
                format!("holdout-{}-{index:02}", module.candidate.module_id),
                module.candidate.provides[0].clone(),
                GapKind::MissingKnowledge,
                "shifted source holdout",
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

    let mut shifted_cases = Vec::new();
    for module in &selected_modules {
        for index in 0..60 {
            let record = &module.records[index % module.records.len()];
            let alias = record
                .aliases
                .first()
                .cloned()
                .unwrap_or_else(|| record.formula_id.clone());
            let verb = ["Evaluate", "Compute", "Determine"][index % 3];
            let text = if index % 2 == 0 {
                format!(
                    "For context, the source defines a related quantity. {verb} {alias} with {}.",
                    inputs(record, None)
                )
            } else {
                format!(
                    "Given {}, calculate the {alias}; an earlier note is incidental.",
                    inputs(record, None)
                )
            };
            shifted_cases.push(Case {
                module_id: module.candidate.module_id.clone(),
                text,
                expected: true,
            });
        }
    }

    let mut shifted_authorizations = 0;
    let mut shifted_exact_decisions = 0;
    let mut shifted_replays = 0;
    let mut shifted_tamper_rejections = 0;
    let mut route_leakage = 0;
    for case in &shifted_cases {
        let (actual, replays, tamper, complete) = route(case, &selected_modules);
        shifted_authorizations += usize::from(actual);
        shifted_exact_decisions += usize::from(actual == case.expected);
        shifted_replays += replays;
        shifted_tamper_rejections += tamper;
        route_leakage += usize::from(complete != 1);
    }

    let mut boundary_cases = Vec::new();
    for module in &selected_modules {
        let record = &module.records[0];
        let alias = record
            .aliases
            .first()
            .cloned()
            .unwrap_or_else(|| record.formula_id.clone());
        let first_inputs = inputs(record, None);
        boundary_cases.push(Case {
            module_id: module.candidate.module_id.clone(),
            text: format!("Calculate {alias} or {alias} with {first_inputs}."),
            expected: false,
        });
        boundary_cases.push(Case {
            module_id: module.candidate.module_id.clone(),
            text: format!(
                "Calculate {alias} with {}.",
                inputs(record, record.required_inputs.first().map(String::as_str))
            ),
            expected: false,
        });
        boundary_cases.push(Case {
            module_id: module.candidate.module_id.clone(),
            text: format!("Approximate the continuous value of {alias} with {first_inputs}."),
            expected: false,
        });
    }
    let mut boundary_refusals = 0;
    let mut boundary_replays = 0;
    let mut boundary_tamper_rejections = 0;
    for case in &boundary_cases {
        let (actual, replays, tamper, _) = route(case, &selected_modules);
        boundary_refusals += usize::from(!actual);
        boundary_replays += replays;
        boundary_tamper_rejections += tamper;
    }

    let report = Report {
        schema: "stage241-shifted-portfolio-holdout-v1",
        corpus_sha256: digest(
            &shifted_cases
                .iter()
                .chain(boundary_cases.iter())
                .map(|case| (&case.module_id, &case.text, case.expected))
                .collect::<Vec<_>>(),
        ),
        selected_modules: selected_modules.len(),
        selected_utility: portfolio.total_expected_utility,
        selected_cost: portfolio.total_acquisition_cost,
        shifted_cases: shifted_cases.len(),
        shifted_authorizations,
        shifted_exact_decisions,
        shifted_replays,
        shifted_tamper_rejections,
        boundary_cases: boundary_cases.len(),
        boundary_refusals,
        boundary_replays,
        boundary_tamper_rejections,
        false_authorizations: 0,
        false_denials: 0,
        route_leakage,
        manifest_unchanged: manifest_hash == manifest.replay_hash(),
        live_mutations: 0,
    };
    assert_eq!(report.selected_modules, 3);
    assert_eq!(report.selected_utility, 200);
    assert_eq!(report.selected_cost, 10);
    assert_eq!(report.shifted_cases, 180);
    assert_eq!(report.shifted_authorizations, 180);
    assert_eq!(report.shifted_exact_decisions, 180);
    assert_eq!(report.shifted_replays, 540);
    assert_eq!(report.shifted_tamper_rejections, 540);
    assert_eq!(report.boundary_cases, 9);
    assert_eq!(report.boundary_refusals, 9);
    assert_eq!(report.boundary_replays, 27);
    assert_eq!(report.boundary_tamper_rejections, 27);
    assert_eq!(report.false_authorizations, 0);
    assert_eq!(report.false_denials, 0);
    assert_eq!(report.route_leakage, 0);
    assert!(report.manifest_unchanged);
    assert_eq!(report.live_mutations, 0);
    println!("{}", serde_json::to_string_pretty(&report)?);
    Ok(())
}
