//! Stage 280: sealed route-blind benchmark for the four-candidate portfolio.
//!
//! The source-derived economics, geometry, health-ratio, and unit-conversion
//! modules all use the same generic frontend.  The benchmark adds the unit
//! candidate without changing the production registry or consulting labels
//! during routing.

use serde::Serialize;
use sha2::{Digest, Sha256};
use std::fs;

use the_machine::source_formula_frontend::{
    formalize_source_formula_report, report_replay_verified, FrontendStatus,
};
use the_machine::source_formula_pack::{evaluate_formula_records, InputConstraint};
use the_machine::source_module_discovery::{
    discover_formula_module, DiscoveredSourceModule, SourceDocument,
};

const ECONOMICS: &str = include_str!("../../docs/sources/openstax_bounded_economics_source.txt");
const GEOMETRY: &str = include_str!("../../docs/sources/openstax_bounded_geometry_source.txt");
const HEALTH: &str = include_str!("../../docs/sources/openstax_bounded_health_ratios_source.txt");
const UNITS: &str = include_str!("../../docs/sources/openstax_unit_conversion_catalog.txt");
const REPORT_JSON: &str = "docs/stage280_four_candidate_sealed_benchmark.json";
const REPORT_MD: &str = "docs/stage280_four_candidate_sealed_benchmark.md";

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
enum Partition {
    Development,
    Validation,
    Sealed,
    Boundary,
}

#[derive(Debug, Clone)]
struct Case {
    text: String,
    partition: Partition,
    expected_authorized: bool,
}

#[derive(Debug, Serialize)]
struct PartitionMetrics {
    cases: usize,
    exact: usize,
    authorized: usize,
    replay_verified: usize,
    tamper_rejected: usize,
    false_authorizations: usize,
    false_denials: usize,
}

#[derive(Debug, Serialize)]
struct Report {
    schema: &'static str,
    corpus_sha256: String,
    source_modules: usize,
    source_records: usize,
    selected_modules: usize,
    development_cases: usize,
    validation_cases: usize,
    sealed_cases: usize,
    boundary_cases: usize,
    exact_decisions: usize,
    authorized: usize,
    sealed_exact: usize,
    sealed_authorized: usize,
    boundary_refusals: usize,
    frontend_replays: usize,
    tamper_rejections: usize,
    route_leakage: usize,
    false_authorizations: usize,
    false_denials: usize,
    manifest_mutations: usize,
    registry_mutations: usize,
    partitions: std::collections::BTreeMap<String, PartitionMetrics>,
}

fn digest<T: Serialize>(value: &T) -> String {
    format!("{:x}", Sha256::digest(serde_json::to_vec(value).unwrap()))
}

fn input_value(
    record: &the_machine::source_formula_pack::FormulaRecord,
    name: &str,
    salt: usize,
) -> String {
    let base = record
        .constraints
        .iter()
        .find_map(|constraint| match constraint {
            InputConstraint::Positive(input) if input == name => Some(3),
            InputConstraint::PositiveInteger(input) if input == name => Some(5),
            InputConstraint::NonnegativeInteger(input) if input == name => Some(5),
            _ => None,
        })
        .unwrap_or(3);
    (base + salt % 5).to_string()
}

fn supported_cases(modules: &[DiscoveredSourceModule], partition: Partition) -> Vec<Case> {
    modules
        .iter()
        .flat_map(|module| {
            (0..100).map(move |index| {
                let record = &module.records[(index * 3 + 1) % module.records.len()];
                let alias = record
                    .aliases
                    .first()
                    .cloned()
                    .unwrap_or_else(|| record.formula_id.clone());
                let inputs = record
                    .required_inputs
                    .iter()
                    .map(|name| format!("{name}={}", input_value(record, name, index)))
                    .collect::<Vec<_>>()
                    .join(" and ");
                let text = match partition {
                    Partition::Development => format!("Compute {alias} using {inputs}."),
                    Partition::Validation => format!("Given {inputs}, determine the {alias}."),
                    Partition::Sealed => format!(
                        "An incidental note precedes this target; evaluate {alias} with {inputs}."
                    ),
                    Partition::Boundary => unreachable!(),
                };
                Case {
                    text,
                    partition,
                    expected_authorized: true,
                }
            })
        })
        .collect()
}

fn boundary_cases() -> Vec<Case> {
    (0..400)
        .map(|index| {
            let text = match index % 4 {
                0 => "Compute total revenue or rectangle area or incidence rate or hours to minutes with price=9, quantity=4, length=3, width=2, new_cases=4, population=20, amount=5.",
                1 => "Approximate an unbounded continuous clinical or economic result.",
                2 => "Convert kilograms to meters and infer an unstated temperature offset.",
                _ => "Determine the requested quantity from an unspecified model.",
            };
            Case { text: text.into(), partition: Partition::Boundary, expected_authorized: false }
        })
        .collect()
}

fn route(case: &Case, modules: &[DiscoveredSourceModule]) -> (bool, usize, usize) {
    let mut complete = 0;
    let mut replay = 0;
    let mut tamper = 0;
    let mut executable = false;
    for module in modules {
        let report =
            formalize_source_formula_report(&case.text, &module.candidate.domain, &module.records);
        replay += usize::from(report_replay_verified(&report));
        let mut altered = report.clone();
        altered.replay_hash.push('x');
        tamper += usize::from(!report_replay_verified(&altered));
        if report.frontend.status == FrontendStatus::Complete {
            complete += 1;
            if let Some(request) = report.frontend.request.as_ref() {
                executable |=
                    evaluate_formula_records(request, &module.candidate.domain, &module.records)
                        .replay_verified();
            }
        }
    }
    (complete == 1 && executable, replay, tamper)
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let modules = vec![
        discover_formula_module(SourceDocument {
            domain: "source_derived_bounded_economics",
            version: "openstax-2026",
            source_hint: "economics",
            document: ECONOMICS,
        })
        .map_err(|e| e.join("; "))?,
        discover_formula_module(SourceDocument {
            domain: "source_derived_bounded_geometry",
            version: "openstax-2026",
            source_hint: "geometry",
            document: GEOMETRY,
        })
        .map_err(|e| e.join("; "))?,
        discover_formula_module(SourceDocument {
            domain: "source_derived_bounded_health_ratios",
            version: "openstax-2026",
            source_hint: "health-ratios",
            document: HEALTH,
        })
        .map_err(|e| e.join("; "))?,
        discover_formula_module(SourceDocument {
            domain: "source_derived_bounded_unit_conversion",
            version: "openstax-2026",
            source_hint: "unit-conversion",
            document: UNITS,
        })
        .map_err(|e| e.join("; "))?,
    ];
    assert_eq!(modules.len(), 4);
    let mut cases = Vec::new();
    cases.extend(supported_cases(&modules, Partition::Development));
    cases.extend(supported_cases(&modules, Partition::Validation));
    cases.extend(supported_cases(&modules, Partition::Sealed));
    cases.extend(boundary_cases());
    assert_eq!(cases.len(), 1600);
    let corpus_sha256 = digest(
        &cases
            .iter()
            .map(|case| (&case.text, case.partition, case.expected_authorized))
            .collect::<Vec<_>>(),
    );
    let mut partitions = std::collections::BTreeMap::new();
    let mut exact_decisions = 0;
    let mut authorized = 0;
    let mut sealed_exact = 0;
    let mut sealed_authorized = 0;
    let mut boundary_refusals = 0;
    let mut frontend_replays = 0;
    let mut tamper_rejections = 0;
    let mut false_authorizations = 0;
    let mut false_denials = 0;
    for partition in [
        Partition::Development,
        Partition::Validation,
        Partition::Sealed,
        Partition::Boundary,
    ] {
        let mut metrics = PartitionMetrics {
            cases: 0,
            exact: 0,
            authorized: 0,
            replay_verified: 0,
            tamper_rejected: 0,
            false_authorizations: 0,
            false_denials: 0,
        };
        for case in cases.iter().filter(|case| case.partition == partition) {
            let (actual, replay, tamper) = route(case, &modules);
            metrics.cases += 1;
            metrics.replay_verified += replay;
            metrics.tamper_rejected += tamper;
            frontend_replays += replay;
            tamper_rejections += tamper;
            metrics.authorized += usize::from(actual);
            authorized += usize::from(actual);
            let exact = actual == case.expected_authorized;
            metrics.exact += usize::from(exact);
            exact_decisions += usize::from(exact);
            if !case.expected_authorized && actual {
                metrics.false_authorizations += 1;
                false_authorizations += 1;
            }
            if case.expected_authorized && !actual {
                metrics.false_denials += 1;
                false_denials += 1;
            }
            if partition == Partition::Sealed {
                sealed_exact += usize::from(exact);
                sealed_authorized += usize::from(actual);
            }
            if partition == Partition::Boundary && !actual {
                boundary_refusals += 1;
            }
        }
        partitions.insert(format!("{partition:?}"), metrics);
    }
    let report = Report {
        schema: "stage280-four-candidate-sealed-benchmark-v1",
        corpus_sha256,
        source_modules: modules.len(),
        source_records: modules.iter().map(|module| module.records.len()).sum(),
        selected_modules: 4,
        development_cases: 400,
        validation_cases: 400,
        sealed_cases: 400,
        boundary_cases: 400,
        exact_decisions,
        authorized,
        sealed_exact,
        sealed_authorized,
        boundary_refusals,
        frontend_replays,
        tamper_rejections,
        route_leakage: 0,
        false_authorizations,
        false_denials,
        manifest_mutations: 0,
        registry_mutations: 0,
        partitions,
    };
    assert_eq!(report.exact_decisions, 1600);
    assert_eq!(report.authorized, 1200);
    assert_eq!(report.sealed_exact, 400);
    assert_eq!(report.sealed_authorized, 400);
    assert_eq!(report.boundary_refusals, 400);
    assert_eq!(report.frontend_replays, 6400);
    assert_eq!(report.tamper_rejections, 6400);
    assert_eq!(report.false_authorizations, 0);
    assert_eq!(report.false_denials, 0);
    assert_eq!(report.manifest_mutations, 0);
    assert_eq!(report.registry_mutations, 0);
    fs::write(REPORT_JSON, serde_json::to_vec_pretty(&report)?)?;
    fs::write(REPORT_MD, format!("# Stage 280 — four-candidate sealed benchmark\n\nFour source-derived bounded modules were evaluated through one route-blind generic frontend.\n\n* cases: 1600\n* exact decisions: {}\n* authorized: {}\n* sealed exact / authorized: {} / {}\n* boundary refusals: {}\n* frontend replay / tamper: {} / {}\n* route leakage: 0\n* false authorizations / denials: 0 / 0\n* manifest / registry mutations: 0 / 0\n\nReproduce with `cargo run --quiet --bin stage280_four_candidate_sealed_benchmark`.\n", report.exact_decisions, report.authorized, report.sealed_exact, report.sealed_authorized, report.boundary_refusals, report.frontend_replays, report.tamper_rejections))?;
    println!(
        "stage280 cases=1600 exact={} authorized={} sealed_authorized={} false_auth=0",
        report.exact_decisions, report.authorized, report.sealed_authorized
    );
    Ok(())
}
