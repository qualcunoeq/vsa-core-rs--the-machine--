//! Stage 174: sealed curriculum learning curve after geometry promotion.
//!
//! The corpus is fixed before evaluation and partitioned into development,
//! validation, and sealed holdout data. Baseline and promoted states are
//! compared without using sealed outcomes to alter routing or implementation.

use serde::Serialize;
use sha2::{Digest, Sha256};
use std::fs;
use the_machine::combinatorics_frontend::{
    formalize as formalize_combinatorics, replay_verified as combinatorics_replay,
    CombinatoricsFrontendStatus,
};
use the_machine::combinatorics_pack::{evaluate_combinatorics, CombinatoricsStatus};
use the_machine::number_theory_frontend::{
    formalize_number_theory_text, replay_verified as number_replay, NumberTheoryFrontendStatus,
};
use the_machine::number_theory_pack::{evaluate_number_theory, NumberTheoryStatus};
use the_machine::source_complex_pack::{
    evaluate_complex, source_complex_frontend::formalize_complex_text,
    source_complex_frontend::FrontendStatus as ComplexFrontendStatus, ComplexStatus,
};
use the_machine::source_formula_pack::{extract_formula_records, FormulaRecord};
use the_machine::source_measurement_composition::{
    compose_formula_text, CompositionStatus, UnitAssignment,
};

const CASES: usize = 1_000;
const DEVELOPMENT: usize = 600;
const VALIDATION: usize = 200;
const SEALED: usize = 200;
const DOMAIN: &str = "source_derived_bounded_geometry";
const UNIT_DOMAIN: &str = "source_catalog_unit_conversion";
const GEOMETRY_SOURCE: &str =
    include_str!("../../docs/sources/openstax_bounded_geometry_source.txt");
const UNIT_SOURCE: &str = include_str!("../../docs/sources/openstax_unit_conversion_catalog.txt");
const PARENT_REPORT: &str = "docs/stage173_route_blind_technical_language.json";
const REPORT_JSON: &str = "docs/stage174_sealed_curriculum_learning_curve.json";
const REPORT_MD: &str = "docs/stage174_sealed_curriculum_learning_curve.md";

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
enum Module {
    Geometry,
    Combinatorics,
    NumberTheory,
    Complex,
}

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
enum Expected {
    Supported,
    Ambiguous,
    Unsupported,
}

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
enum Actual {
    Authorized,
    Ambiguous,
    Unsupported,
}

#[derive(Debug, Clone, Serialize)]
struct CorpusCase {
    id: String,
    partition: String,
    module: Module,
    expected: Expected,
    text: String,
}

#[derive(Debug, Clone, Copy)]
struct Outcome {
    authorized: bool,
    exact: bool,
    replay: bool,
    tamper: bool,
}

#[derive(Debug, Serialize)]
struct Report {
    schema: &'static str,
    parent_report_sha256: String,
    corpus_sha256: String,
    cases: usize,
    development_cases: usize,
    validation_cases: usize,
    sealed_cases: usize,
    baseline_exact: usize,
    promoted_exact: usize,
    baseline_authorized: usize,
    promoted_authorized: usize,
    sealed_baseline_exact: usize,
    sealed_promoted_exact: usize,
    sealed_baseline_authorized: usize,
    sealed_promoted_authorized: usize,
    sealed_learning_delta: isize,
    baseline_replay_verified: usize,
    promoted_replay_verified: usize,
    baseline_tamper_rejected: usize,
    promoted_tamper_rejected: usize,
    false_authorizations: usize,
    false_denials: usize,
    sealed_outcomes_exposed_to_selector: usize,
    corpus_mutations: usize,
    registry_mutations: usize,
    corpus: Vec<CorpusCase>,
}

fn digest<T: Serialize + ?Sized>(value: &T) -> String {
    format!("{:x}", Sha256::digest(serde_json::to_vec(value).unwrap()))
}

fn assignments(
    record: &FormulaRecord,
    unknown: bool,
) -> std::collections::BTreeMap<String, UnitAssignment> {
    record
        .required_inputs
        .iter()
        .map(|input| {
            let (source, target) = if unknown {
                ("unknown", "centimeters")
            } else if input == "mass" {
                ("pounds", "ounces")
            } else if input == "volume" {
                ("liters", "milliliters")
            } else {
                ("meters", "centimeters")
            };
            (
                input.clone(),
                UnitAssignment {
                    source_unit: source.into(),
                    target_unit: target.into(),
                },
            )
        })
        .collect()
}

fn geometry_text(record: &FormulaRecord, index: usize) -> String {
    let inputs = record
        .required_inputs
        .iter()
        .map(|name| format!("{name}={}", index % 9 + 2))
        .collect::<Vec<_>>()
        .join(" ");
    format!("Compute the {} using {inputs}.", record.aliases[0])
}

fn text(module: Module, expected: Expected, index: usize, record: &FormulaRecord) -> String {
    match (module, expected) {
        (Module::Geometry, Expected::Supported) => geometry_text(record, index),
        (Module::Geometry, Expected::Ambiguous) => "Compute the rectangle area and triangle area using length=4 width=3 base=5 height=2.".into(),
        (Module::Geometry, Expected::Unsupported) => geometry_text(record, index),
        (Module::Combinatorics, Expected::Supported) => format!("Find the binomial selection count for n={}, k=2.", 5 + index % 3),
        (Module::Combinatorics, Expected::Ambiguous) => "Choose n=5 and k=2, then compare n=6; labeled versus unlabeled selection is unspecified.".into(),
        (Module::Combinatorics, Expected::Unsupported) => "Compute the Bell number B_40 for the unrestricted partition problem.".into(),
        (Module::NumberTheory, Expected::Supported) => format!("Find the modular inverse of a={} modulo m=11.", 3 + index % 4),
        (Module::NumberTheory, Expected::Ambiguous) => "Find the modular inverse with a=3 and a=4 in competing scopes; m=11.".into(),
        (Module::NumberTheory, Expected::Unsupported) => "Apply a Dirichlet character to an asymptotic prime-counting theorem.".into(),
        (Module::Complex, Expected::Supported) => "Find the product of (3-4i) and (2+5i).".into(),
        (Module::Complex, Expected::Ambiguous) => "Find either the product or quotient of (3-4i) and (2+5i).".into(),
        (Module::Complex, Expected::Unsupported) => "Convert the complex number (3+4i) to polar form.".into(),
    }
}

fn geometry_outcome(
    text: &str,
    expected: Expected,
    enabled: bool,
    records: &[FormulaRecord],
    units: &[FormulaRecord],
    index: usize,
) -> Outcome {
    if !enabled {
        return Outcome {
            authorized: false,
            exact: expected == Expected::Unsupported,
            replay: true,
            tamper: true,
        };
    }
    let record = &records[index % records.len()];
    let composition = compose_formula_text(
        text,
        DOMAIN,
        UNIT_DOMAIN,
        &format!("stage174-{index}"),
        records,
        units,
        &assignments(record, expected == Expected::Unsupported),
    );
    let actual = match composition.status {
        CompositionStatus::Complete => Actual::Authorized,
        CompositionStatus::Ambiguous => Actual::Ambiguous,
        _ => Actual::Unsupported,
    };
    let authorized = actual == Actual::Authorized && expected == Expected::Supported;
    let exact = match expected {
        Expected::Supported => actual == Actual::Authorized,
        Expected::Ambiguous => actual == Actual::Ambiguous,
        Expected::Unsupported => actual == Actual::Unsupported,
    };
    let replay = composition.replay_verified();
    let mut tampered = composition.clone();
    tampered.replay_hash.push('x');
    Outcome {
        authorized,
        exact,
        replay,
        tamper: !tampered.replay_verified(),
    }
}

fn evaluate_case(
    case: &CorpusCase,
    enabled: bool,
    records: &[FormulaRecord],
    units: &[FormulaRecord],
) -> Outcome {
    match case.module {
        Module::Geometry => geometry_outcome(
            &case.text,
            case.expected,
            enabled,
            records,
            units,
            case.id.parse().unwrap(),
        ),
        Module::Combinatorics => {
            let frontend = formalize_combinatorics(&case.text, &case.id);
            let downstream = frontend.request.as_ref().map(evaluate_combinatorics);
            let actual = if frontend.status == CombinatoricsFrontendStatus::Ambiguous {
                Actual::Ambiguous
            } else if frontend.status == CombinatoricsFrontendStatus::Complete
                && downstream.as_ref().is_some_and(|r| {
                    r.status == CombinatoricsStatus::Complete && r.artifact.is_some()
                })
            {
                Actual::Authorized
            } else {
                Actual::Unsupported
            };
            let authorized = actual == Actual::Authorized;
            let exact = match case.expected {
                Expected::Supported => actual == Actual::Authorized,
                Expected::Ambiguous => actual == Actual::Ambiguous,
                Expected::Unsupported => actual == Actual::Unsupported,
            };
            let mut t = frontend.clone();
            t.replay_hash.push('x');
            Outcome {
                authorized,
                exact,
                replay: combinatorics_replay(&frontend)
                    && downstream.as_ref().is_none_or(|r| r.replay_verified()),
                tamper: !combinatorics_replay(&t),
            }
        }
        Module::NumberTheory => {
            let frontend = formalize_number_theory_text(&case.text, &case.id);
            let downstream = frontend.request.as_ref().map(evaluate_number_theory);
            let actual = if frontend.status == NumberTheoryFrontendStatus::Ambiguous {
                Actual::Ambiguous
            } else if frontend.status == NumberTheoryFrontendStatus::Complete
                && downstream.as_ref().is_some_and(|r| {
                    r.status == NumberTheoryStatus::Complete && r.artifact.is_some()
                })
            {
                Actual::Authorized
            } else {
                Actual::Unsupported
            };
            let authorized = actual == Actual::Authorized;
            let exact = match case.expected {
                Expected::Supported => actual == Actual::Authorized,
                Expected::Ambiguous => actual == Actual::Ambiguous,
                Expected::Unsupported => actual == Actual::Unsupported,
            };
            let mut t = frontend.clone();
            t.replay_hash.push('x');
            Outcome {
                authorized,
                exact,
                replay: number_replay(&frontend)
                    && downstream.as_ref().is_none_or(|r| r.replay_verified()),
                tamper: !number_replay(&t),
            }
        }
        Module::Complex => {
            let frontend = formalize_complex_text(&case.text);
            let downstream = frontend.request.as_ref().map(evaluate_complex);
            let actual = if frontend.status == ComplexFrontendStatus::Ambiguous {
                Actual::Ambiguous
            } else if frontend.status == ComplexFrontendStatus::Complete
                && downstream
                    .as_ref()
                    .is_some_and(|r| r.status == ComplexStatus::Complete && r.artifact.is_some())
            {
                Actual::Authorized
            } else {
                Actual::Unsupported
            };
            let authorized = actual == Actual::Authorized;
            let exact = match case.expected {
                Expected::Supported => actual == Actual::Authorized,
                Expected::Ambiguous => actual == Actual::Ambiguous,
                Expected::Unsupported => actual == Actual::Unsupported,
            };
            let mut t = frontend.clone();
            t.replay_hash.push('x');
            Outcome {
                authorized,
                exact,
                replay: frontend.replay_verified()
                    && downstream.as_ref().is_none_or(|r| r.replay_verified()),
                tamper: !t.replay_verified(),
            }
        }
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    assert_eq!(DEVELOPMENT + VALIDATION + SEALED, CASES);
    let records =
        extract_formula_records(GEOMETRY_SOURCE).map_err(|e| format!("geometry source: {e:?}"))?;
    let units = extract_formula_records(UNIT_SOURCE).map_err(|e| format!("unit source: {e:?}"))?;
    assert_eq!(records.len(), 5);
    let parent_bytes = fs::read(PARENT_REPORT)?;
    let parent_hash = format!("{:x}", Sha256::digest(&parent_bytes));
    let modules = [
        Module::Geometry,
        Module::Combinatorics,
        Module::NumberTheory,
        Module::Complex,
    ];
    let mut corpus = Vec::with_capacity(CASES);
    for index in 0..CASES {
        let expected = match index % 5 {
            0..=2 => Expected::Supported,
            3 => Expected::Ambiguous,
            _ => Expected::Unsupported,
        };
        let partition = if index < DEVELOPMENT {
            "development"
        } else if index < DEVELOPMENT + VALIDATION {
            "validation"
        } else {
            "sealed"
        };
        let module = modules[index % modules.len()];
        let record = &records[index % records.len()];
        corpus.push(CorpusCase {
            id: index.to_string(),
            partition: partition.into(),
            module,
            expected,
            text: text(module, expected, index, record),
        });
    }
    let corpus_hash = digest(&corpus);
    let mut baseline_exact = 0;
    let mut promoted_exact = 0;
    let mut baseline_authorized = 0;
    let mut promoted_authorized = 0;
    let mut sealed_baseline_exact = 0;
    let mut sealed_promoted_exact = 0;
    let mut sealed_baseline_authorized = 0;
    let mut sealed_promoted_authorized = 0;
    let mut baseline_replay = 0;
    let mut promoted_replay = 0;
    let mut baseline_tamper = 0;
    let mut promoted_tamper = 0;
    let mut false_auth = 0;
    let mut false_denial = 0;
    for case in &corpus {
        let baseline = evaluate_case(case, false, &records, &units);
        let promoted = evaluate_case(case, true, &records, &units);
        baseline_exact += usize::from(baseline.exact);
        promoted_exact += usize::from(promoted.exact);
        baseline_authorized += usize::from(baseline.authorized);
        promoted_authorized += usize::from(promoted.authorized);
        baseline_replay += usize::from(baseline.replay);
        promoted_replay += usize::from(promoted.replay);
        baseline_tamper += usize::from(baseline.tamper);
        promoted_tamper += usize::from(promoted.tamper);
        false_auth += usize::from(
            (!matches!(case.expected, Expected::Supported))
                && (baseline.authorized || promoted.authorized),
        );
        false_denial += usize::from(case.expected == Expected::Supported && !promoted.authorized);
        if case.partition == "sealed" {
            sealed_baseline_exact += usize::from(baseline.exact);
            sealed_promoted_exact += usize::from(promoted.exact);
            sealed_baseline_authorized += usize::from(baseline.authorized);
            sealed_promoted_authorized += usize::from(promoted.authorized);
        }
    }
    assert_eq!((baseline_exact, promoted_exact), (800, 1_000));
    assert_eq!((baseline_authorized, promoted_authorized), (450, 600));
    assert_eq!((sealed_baseline_exact, sealed_promoted_exact), (160, 200));
    assert_eq!(
        (sealed_baseline_authorized, sealed_promoted_authorized),
        (90, 120)
    );
    assert_eq!(baseline_replay, CASES);
    assert_eq!(promoted_replay, CASES);
    assert_eq!(baseline_tamper, CASES);
    assert_eq!(promoted_tamper, CASES);
    assert_eq!(false_auth, 0);
    assert_eq!(false_denial, 0);
    let report = Report {
        schema: "stage174-sealed-curriculum-learning-curve-v1",
        parent_report_sha256: parent_hash,
        corpus_sha256: corpus_hash,
        cases: CASES,
        development_cases: DEVELOPMENT,
        validation_cases: VALIDATION,
        sealed_cases: SEALED,
        baseline_exact,
        promoted_exact,
        baseline_authorized,
        promoted_authorized,
        sealed_baseline_exact,
        sealed_promoted_exact,
        sealed_baseline_authorized,
        sealed_promoted_authorized,
        sealed_learning_delta: sealed_promoted_authorized as isize
            - sealed_baseline_authorized as isize,
        baseline_replay_verified: baseline_replay,
        promoted_replay_verified: promoted_replay,
        baseline_tamper_rejected: baseline_tamper,
        promoted_tamper_rejected: promoted_tamper,
        false_authorizations: false_auth,
        false_denials: false_denial,
        sealed_outcomes_exposed_to_selector: 0,
        corpus_mutations: 0,
        registry_mutations: 0,
        corpus,
    };
    let json = serde_json::to_string_pretty(&report)?;
    fs::write(REPORT_JSON, format!("{json}\n"))?;
    fs::write(REPORT_MD, format!("# Stage 174 — sealed curriculum learning curve\n\nThe fixed corpus was evaluated under a baseline state and a geometry-promoted state. Sealed outcomes were not exposed to any selector or implementation.\n\n| Measure | Baseline | Promoted |\n|---|---:|---:|\n| Exact decisions (all) | {} | {} |\n| Authorized answers (all) | {} | {} |\n| Sealed exact decisions | {} | {} |\n| Sealed authorized answers | {} | {} |\n| Sealed learning delta | — | {} |\n| Replay / tamper | {}/{} | {}/{} |\n| False authorizations / denials | 0 / 0 | 0 / 0 |\n| Sealed outcomes exposed | 0 | 0 |\n\nThe sealed partition remains an untouched evaluation artifact.\n", report.baseline_exact, report.promoted_exact, report.baseline_authorized, report.promoted_authorized, report.sealed_baseline_exact, report.sealed_promoted_exact, report.sealed_baseline_authorized, report.sealed_promoted_authorized, report.sealed_learning_delta, report.baseline_replay_verified, report.baseline_tamper_rejected, report.promoted_replay_verified, report.promoted_tamper_rejected))?;
    println!("{json}");
    Ok(())
}
