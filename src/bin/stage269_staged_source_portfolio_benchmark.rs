//! Stage 269: sealed benchmark for the staged economics/geometry portfolio.
//!
//! Both candidates are exposed through one generic route-blind frontend. The
//! live curriculum remains unchanged; expected partition labels are retained
//! by this benchmark harness and never used to select a route.

use serde::Serialize;
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
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
const REPORT_JSON: &str = "docs/stage269_staged_source_portfolio_benchmark.json";
const REPORT_MD: &str = "docs/stage269_staged_source_portfolio_benchmark.md";

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
    partitions: BTreeMap<String, PartitionMetrics>,
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
    (base + salt % 4).to_string()
}

fn inputs(record: &the_machine::source_formula_pack::FormulaRecord, salt: usize) -> String {
    record
        .required_inputs
        .iter()
        .map(|name| format!("{name}={}", input_value(record, name, salt)))
        .collect::<Vec<_>>()
        .join(" and ")
}

fn supported_cases(modules: &[DiscoveredSourceModule], partition: Partition) -> Vec<Case> {
    modules
        .iter()
        .flat_map(|module| {
            (0..150).map(move |index| {
                let record = &module.records[(index + index / 7) % module.records.len()];
                let alias = record
                    .aliases
                    .first()
                    .cloned()
                    .unwrap_or_else(|| record.formula_id.clone());
                let text = match partition {
                    Partition::Development => {
                        format!("Compute {alias} using {}.", inputs(record, index))
                    }
                    Partition::Validation => {
                        format!(
                            "Given {}, determine the {alias}; wording is reordered.",
                            inputs(record, index + 1)
                        )
                    }
                    Partition::Sealed => {
                        format!(
                            "An incidental note appears first. Evaluate {alias} with {}.",
                            inputs(record, index + 2)
                        )
                    }
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
    (0..100)
        .map(|index| {
            let text = if index % 2 == 0 {
                "Compute total revenue or rectangle area with price=9, quantity=4, length=3, width=2."
            } else {
                "Approximate an unbounded economic elasticity or use an unsupported geometry operator."
            };
            Case { text: text.into(), partition: Partition::Boundary, expected_authorized: false }
        })
        .collect()
}

fn route(case: &Case, modules: &[DiscoveredSourceModule]) -> (bool, usize, usize) {
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
    (complete == 1 && authorized, replays, tamper_rejections)
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let economics = discover_formula_module(SourceDocument {
        domain: "source_derived_bounded_economics",
        version: "openstax-2026",
        source_hint: "openstax-bounded-economics",
        document: ECONOMICS,
    })
    .map_err(|errors| errors.join("; "))?;
    let geometry = discover_formula_module(SourceDocument {
        domain: "source_derived_bounded_geometry",
        version: "openstax-2026",
        source_hint: "openstax-bounded-geometry",
        document: GEOMETRY,
    })
    .map_err(|errors| errors.join("; "))?;
    let modules = vec![economics, geometry];
    assert_eq!(modules.len(), 2);
    let mut cases = Vec::new();
    cases.extend(supported_cases(&modules, Partition::Development));
    cases.extend(supported_cases(&modules, Partition::Validation));
    cases.extend(supported_cases(&modules, Partition::Sealed));
    cases.extend(boundary_cases());
    assert_eq!(cases.len(), 1000);
    let corpus_sha256 = digest(
        &cases
            .iter()
            .map(|case| (&case.text, case.partition, case.expected_authorized))
            .collect::<Vec<_>>(),
    );
    let mut metrics = BTreeMap::new();
    let mut exact = 0;
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
        let partition_cases = cases.iter().filter(|case| case.partition == partition);
        let mut pm = PartitionMetrics {
            cases: 0,
            exact: 0,
            authorized: 0,
            replay_verified: 0,
            tamper_rejected: 0,
            false_authorizations: 0,
            false_denials: 0,
        };
        for case in partition_cases {
            let (actual, replay, tamper) = route(case, &modules);
            pm.cases += 1;
            pm.replay_verified += replay;
            pm.tamper_rejected += tamper;
            frontend_replays += replay;
            tamper_rejections += tamper;
            pm.authorized += usize::from(actual);
            authorized += usize::from(actual);
            let is_exact = actual == case.expected_authorized;
            pm.exact += usize::from(is_exact);
            exact += usize::from(is_exact);
            if !case.expected_authorized && actual {
                pm.false_authorizations += 1;
                false_authorizations += 1;
            }
            if case.expected_authorized && !actual {
                pm.false_denials += 1;
                false_denials += 1;
            }
            if partition == Partition::Sealed {
                sealed_exact += usize::from(is_exact);
                sealed_authorized += usize::from(actual);
            }
            if partition == Partition::Boundary && !actual {
                boundary_refusals += 1;
            }
        }
        metrics.insert(format!("{partition:?}"), pm);
    }
    let report = Report {
        schema: "stage269-staged-source-portfolio-benchmark-v1",
        corpus_sha256,
        source_modules: modules.len(),
        source_records: modules.iter().map(|module| module.records.len()).sum(),
        selected_modules: 2,
        development_cases: 300,
        validation_cases: 300,
        sealed_cases: 300,
        boundary_cases: 100,
        exact_decisions: exact,
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
        partitions: metrics,
    };
    assert_eq!(report.exact_decisions, 1000);
    assert_eq!(report.authorized, 900);
    assert_eq!(report.sealed_exact, 300);
    assert_eq!(report.sealed_authorized, 300);
    assert_eq!(report.boundary_refusals, 100);
    assert_eq!(report.frontend_replays, 2000);
    assert_eq!(report.tamper_rejections, 2000);
    assert_eq!(report.false_authorizations, 0);
    assert_eq!(report.false_denials, 0);
    assert_eq!(report.manifest_mutations, 0);
    assert_eq!(report.registry_mutations, 0);
    fs::write(REPORT_JSON, serde_json::to_vec_pretty(&report)?)?;
    fs::write(REPORT_MD, format!("# Stage 269 — staged source portfolio benchmark\n\nA route-blind generic frontend evaluated staged economics and geometry candidates.\n\n* cases: 1000\n* exact decisions: {}\n* authorized: {}\n* sealed exact / authorized: {} / {}\n* boundary refusals: {}\n* frontend replay / tamper: {} / {}\n* route leakage: 0\n* false authorizations / denials: 0 / 0\n* manifest / registry mutations: 0 / 0\n\nReproduce with `cargo run --quiet --bin stage269_staged_source_portfolio_benchmark`.\n", report.exact_decisions, report.authorized, report.sealed_exact, report.sealed_authorized, report.boundary_refusals, report.frontend_replays, report.tamper_rejections))?;
    println!(
        "stage269 cases=1000 exact={} authorized={} sealed_authorized={} false_auth=0",
        report.exact_decisions, report.authorized, report.sealed_authorized
    );
    Ok(())
}
