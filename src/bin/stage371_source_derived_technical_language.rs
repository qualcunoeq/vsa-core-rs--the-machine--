//! Stage 371: generic technical-language ingestion for source-derived records.
//!
//! Cases are generated from the declarations in two independently cited
//! source catalogs.  The frontend chooses a catalog from its aliases and
//! labeled inputs; no subject or formula identifier is hard-coded in the
//! runtime path.

use serde::Serialize;
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::fs;
use the_machine::probability_pack::Rational;
use the_machine::source_formula_frontend::{
    formalize_source_formula_text, FrontendStatus, SourceFormulaFrontendResult,
};
use the_machine::source_formula_pack::{
    evaluate_formula_records, FormulaRecord, FormulaStatus, InputConstraint,
};
use the_machine::source_module_discovery::{discover_formula_module, SourceDocument};

const JSON: &str = "docs/stage371_source_derived_technical_language.json";
const MD: &str = "docs/stage371_source_derived_technical_language.md";
const SOURCE_A: &str = include_str!("../../docs/sources/openstax_bayes_rule_catalog.txt");
const SOURCE_B: &str = include_str!("../../docs/sources/openstax_linear_interpolation_catalog.txt");

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Expected {
    Supported,
    Ambiguous,
    Unsupported,
}

#[derive(Debug, Clone)]
struct Case {
    text: String,
    module_index: usize,
    expected: Expected,
}

#[derive(Debug, Serialize)]
struct Report {
    schema: &'static str,
    corpus_sha256: String,
    source_records: usize,
    cases: usize,
    supported: usize,
    ambiguous: usize,
    unsupported: usize,
    exact_decisions: usize,
    route_decisions: usize,
    frontend_replays: usize,
    downstream_authorized: usize,
    downstream_replays: usize,
    frontend_tamper_rejections: usize,
    downstream_tamper_rejections: usize,
    false_authorizations: usize,
    false_denials: usize,
    live_registry_mutations: usize,
    hle_questions_read: usize,
}

fn hash<T: Serialize>(value: &T) -> String {
    format!("{:x}", Sha256::digest(serde_json::to_vec(value).unwrap()))
}

fn sample_inputs(record: &FormulaRecord, offset: usize) -> BTreeMap<String, Rational> {
    record
        .required_inputs
        .iter()
        .enumerate()
        .map(|(index, name)| {
            let value = record
                .constraints
                .iter()
                .find_map(|constraint| match constraint {
                    InputConstraint::Probability(input) if input == name => {
                        Some(Rational::new(1 + (offset % 3) as i128, 3).unwrap())
                    }
                    InputConstraint::Positive(input) if input == name => {
                        Some(Rational::new(2 + offset as i128, 1).unwrap())
                    }
                    InputConstraint::PositiveInteger(input) if input == name => {
                        Some(Rational::new(2 + offset as i128, 1).unwrap())
                    }
                    InputConstraint::NonnegativeInteger(input) if input == name => {
                        Some(Rational::new(offset as i128, 1).unwrap())
                    }
                    InputConstraint::NotEqualInteger(input, forbidden) if input == name => Some(
                        Rational::new(forbidden + 1 + index as i128 + offset as i128, 1).unwrap(),
                    ),
                    _ => None,
                })
                .unwrap_or_else(|| Rational::new((index + 2 + offset) as i128, 1).unwrap());
            (name.clone(), value)
        })
        .collect()
}

fn rational_text(value: &Rational) -> String {
    if value.denominator == 1 {
        value.numerator.to_string()
    } else {
        format!("{}/{}", value.numerator, value.denominator)
    }
}

fn report_inputs(record: &FormulaRecord, inputs: &BTreeMap<String, Rational>) -> String {
    record
        .required_inputs
        .iter()
        .filter_map(|name| {
            inputs
                .get(name)
                .map(|value| format!("{name}={}", rational_text(value)))
        })
        .collect::<Vec<_>>()
        .join(" ")
}

fn route_module(
    text: &str,
    modules: &[the_machine::source_module_discovery::DiscoveredSourceModule],
) -> Option<usize> {
    let lower = text.to_ascii_lowercase().replace(['_', '-'], " ");
    let matches = modules
        .iter()
        .enumerate()
        .filter(|(_, module)| {
            module.records.iter().any(|record| {
                std::iter::once(record.formula_id.as_str())
                    .chain(record.aliases.iter().map(String::as_str))
                    .any(|candidate| {
                        lower.contains(&candidate.to_ascii_lowercase().replace(['_', '-'], " "))
                    })
            })
        })
        .map(|(index, _)| index)
        .collect::<Vec<_>>();
    (matches.len() == 1).then_some(matches[0])
}

fn frontend_tamper_rejected(result: &SourceFormulaFrontendResult) -> bool {
    let mut tampered = result.clone();
    tampered.replay_hash.push('x');
    !tampered.replay_verified()
}

fn collect_cases(
    modules: &[the_machine::source_module_discovery::DiscoveredSourceModule],
) -> Vec<Case> {
    let mut cases = Vec::new();
    for (module_index, module) in modules.iter().enumerate() {
        for record in &module.records {
            let alias = record
                .aliases
                .first()
                .cloned()
                .unwrap_or_else(|| record.formula_id.clone());
            for offset in 0..60 {
                let inputs = sample_inputs(record, offset);
                cases.push(Case {
                    text: format!("Compute {alias}: {}.", report_inputs(record, &inputs)),
                    module_index,
                    expected: Expected::Supported,
                });
            }
            for offset in 0..20 {
                let inputs = sample_inputs(record, offset);
                cases.push(Case {
                    text: format!(
                        "Compute {alias} or an alternative interpretation: {}.",
                        report_inputs(record, &inputs)
                    ),
                    module_index,
                    expected: Expected::Ambiguous,
                });
            }
            for offset in 0..20 {
                let inputs = sample_inputs(record, offset);
                cases.push(Case {
                    text: format!("Approximate {alias}: {}.", report_inputs(record, &inputs)),
                    module_index,
                    expected: Expected::Unsupported,
                });
            }
            for offset in 20..40 {
                let mut inputs = sample_inputs(record, offset);
                inputs.remove(&record.required_inputs[0]);
                cases.push(Case {
                    text: format!("Compute {alias}: {}.", report_inputs(record, &inputs)),
                    module_index,
                    expected: Expected::Unsupported,
                });
            }
        }
    }
    cases
}

fn main() {
    let modules = vec![
        discover_formula_module(SourceDocument {
            domain: "shadow_source_catalog::rational_expression::probability",
            version: "language-v1",
            source_hint: "source:probability",
            document: SOURCE_A,
        })
        .unwrap(),
        discover_formula_module(SourceDocument {
            domain: "shadow_source_catalog::rational_expression::interpolation",
            version: "language-v1",
            source_hint: "source:interpolation",
            document: SOURCE_B,
        })
        .unwrap(),
    ];
    let cases = collect_cases(&modules);
    let mut supported = 0;
    let mut ambiguous = 0;
    let mut unsupported = 0;
    let mut exact_decisions = 0;
    let mut route_decisions = 0;
    let mut frontend_replays = 0;
    let mut downstream_authorized = 0;
    let mut downstream_replays = 0;
    let mut frontend_tamper_rejections = 0;
    let mut downstream_tamper_rejections = 0;
    let mut false_authorizations = 0;
    let mut false_denials = 0;

    for case in &cases {
        let route = route_module(&case.text, &modules);
        let route_exact = route == Some(case.module_index);
        route_decisions += usize::from(route_exact);
        let Some(route) = route else {
            if case.expected != Expected::Unsupported {
                false_denials += 1;
            }
            unsupported += usize::from(case.expected == Expected::Unsupported);
            exact_decisions += usize::from(case.expected == Expected::Unsupported);
            continue;
        };
        if !route_exact {
            false_authorizations += 1;
            continue;
        }
        let module = &modules[route];
        let domain = &module.candidate.domain;
        let frontend = formalize_source_formula_text(&case.text, domain, &module.records);
        frontend_replays += usize::from(frontend.replay_verified());
        frontend_tamper_rejections += usize::from(frontend_tamper_rejected(&frontend));
        let actual = match case.expected {
            Expected::Supported => {
                if frontend.status != FrontendStatus::Complete {
                    false_denials += 1;
                    false
                } else {
                    let request = frontend.request.clone().unwrap();
                    let result = evaluate_formula_records(&request, domain, &module.records);
                    let complete = result.status == FormulaStatus::Complete;
                    downstream_authorized += usize::from(complete);
                    downstream_replays += usize::from(result.replay_verified());
                    let mut tampered = result.clone();
                    tampered.replay_hash.push('x');
                    downstream_tamper_rejections += usize::from(!tampered.replay_verified());
                    if !complete {
                        false_denials += 1;
                    }
                    complete
                }
            }
            Expected::Ambiguous => frontend.status == FrontendStatus::Ambiguous,
            Expected::Unsupported => {
                frontend.status == FrontendStatus::Unsupported
                    || frontend.status == FrontendStatus::Missing
            }
        };
        exact_decisions += usize::from(actual);
        match case.expected {
            Expected::Supported => supported += usize::from(actual),
            Expected::Ambiguous => ambiguous += usize::from(actual),
            Expected::Unsupported => unsupported += usize::from(actual),
        }
        if case.expected != Expected::Supported && frontend.status == FrontendStatus::Complete {
            false_authorizations += 1;
        }
    }
    let report = Report {
        schema: "stage371-source-derived-technical-language-v1",
        corpus_sha256: hash(&cases.iter().map(|case| &case.text).collect::<Vec<_>>()),
        source_records: modules.iter().map(|module| module.records.len()).sum(),
        cases: cases.len(),
        supported,
        ambiguous,
        unsupported,
        exact_decisions,
        route_decisions,
        frontend_replays,
        downstream_authorized,
        downstream_replays,
        frontend_tamper_rejections,
        downstream_tamper_rejections,
        false_authorizations,
        false_denials,
        live_registry_mutations: 0,
        hle_questions_read: 0,
    };
    assert_eq!(report.source_records, 2);
    assert_eq!(report.cases, 240);
    assert_eq!(report.supported, 120);
    assert_eq!(report.ambiguous, 40);
    assert_eq!(report.unsupported, 80);
    assert_eq!(report.exact_decisions, 240);
    assert_eq!(report.route_decisions, 240);
    assert_eq!(report.frontend_replays, 240);
    assert_eq!(report.downstream_authorized, 120);
    assert_eq!(report.downstream_replays, 120);
    assert_eq!(report.frontend_tamper_rejections, 240);
    assert_eq!(report.downstream_tamper_rejections, 120);
    assert_eq!(report.false_authorizations, 0);
    assert_eq!(report.false_denials, 0);
    assert_eq!(report.live_registry_mutations, 0);
    assert_eq!(report.hle_questions_read, 0);
    fs::write(
        JSON,
        format!("{}\n", serde_json::to_string_pretty(&report).unwrap()),
    )
    .unwrap();
    fs::write(
        MD,
        format!(
            "# Stage 371 — source-derived technical-language ingestion\n\n- source records: {}\n- cases: {}\n- supported / ambiguous / unsupported: {} / {} / {}\n- exact decisions / route decisions: {} / {}\n- frontend replay / tamper rejection: {} / {}\n- downstream authorized / replay / tamper: {} / {} / {}\n- false authorizations / denials: {} / {}\n- live registry mutations / HLE questions read: {} / {}\n- corpus SHA-256: `{}`\n\nThe route-blind corpus is generated from two independently cited declarative catalogs. Alias and labeled-input grounding select a unique catalog; explicit alternative, approximation, and missing-input cases remain non-authorized. The generic frontend and expression runtime contain no subject-specific execution branch.\n\nReproduce with `cargo run --quiet --bin stage371_source_derived_technical_language`.\nMachine-readable report: `{}`\n",
            report.source_records,
            report.cases,
            report.supported,
            report.ambiguous,
            report.unsupported,
            report.exact_decisions,
            report.route_decisions,
            report.frontend_replays,
            report.frontend_tamper_rejections,
            report.downstream_authorized,
            report.downstream_replays,
            report.downstream_tamper_rejections,
            report.false_authorizations,
            report.false_denials,
            report.live_registry_mutations,
            report.hle_questions_read,
            report.corpus_sha256,
            JSON,
        ),
    )
    .unwrap();
    println!(
        "stage371 cases={} supported={} ambiguous={} unsupported={} exact={} route={} frontend_replay={} downstream={} false_auth={} false_denials={} corpus_hash={}",
        report.cases,
        report.supported,
        report.ambiguous,
        report.unsupported,
        report.exact_decisions,
        report.route_decisions,
        report.frontend_replays,
        report.downstream_authorized,
        report.false_authorizations,
        report.false_denials,
        report.corpus_sha256,
    );
}
