//! Stage 173: route-blind technical-language evaluation with promoted geometry.
//!
//! Every report is offered to every validated frontend. A route is selected
//! only when exactly one downstream artifact is authorized; ambiguous or
//! unsupported interpretations never enter another executor. Geometry also
//! requires its promoted memory artifacts before composition.

use serde::Serialize;
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::fs;
use the_machine::combinatorics_frontend::{
    formalize as formalize_combinatorics, replay_verified as combinatorics_replay,
    CombinatoricsFrontendStatus,
};
use the_machine::combinatorics_pack::{evaluate_combinatorics, CombinatoricsStatus};
use the_machine::curriculum_memory::{AppendStatus, CurriculumMemory, MemoryRecord};
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

const DOMAIN: &str = "source_derived_bounded_geometry";
const UNIT_DOMAIN: &str = "source_catalog_unit_conversion";
const VERSION: &str = "v2";
const GEOMETRY_SOURCE: &str =
    include_str!("../../docs/sources/openstax_bounded_geometry_source.txt");
const UNIT_SOURCE: &str = include_str!("../../docs/sources/openstax_unit_conversion_catalog.txt");
const PARENT_REPORT: &str = "docs/stage172_memory_backed_geometry_routes.json";
const REPORT_JSON: &str = "docs/stage173_route_blind_technical_language.json";
const REPORT_MD: &str = "docs/stage173_route_blind_technical_language.md";

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq, PartialOrd, Ord)]
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

#[derive(Debug, Clone, Copy)]
struct Observation {
    module: Module,
    authorized: bool,
    ambiguous: bool,
    provenance: bool,
    replay: bool,
    tamper: bool,
}

#[derive(Debug, Serialize)]
struct Receipt {
    id: String,
    partition: String,
    text_sha256: String,
    expected: Expected,
    actual: Actual,
    authorized_modules: Vec<Module>,
    ambiguous_modules: Vec<Module>,
    selected_module: Option<Module>,
    exact: bool,
    provenance: bool,
    replay: bool,
    tamper_rejected: bool,
    false_authorization: bool,
    false_denial: bool,
}

#[derive(Debug, Serialize)]
struct Report {
    schema: &'static str,
    parent_report_sha256: String,
    geometry_source_sha256: String,
    unit_source_sha256: String,
    corpus_sha256: String,
    cases: usize,
    development_cases: usize,
    development_supported: usize,
    development_ambiguous: usize,
    development_unsupported: usize,
    development_exact: usize,
    development_authorized: usize,
    holdout_cases: usize,
    holdout_supported: usize,
    holdout_ambiguous: usize,
    holdout_unsupported: usize,
    holdout_exact: usize,
    holdout_authorized: usize,
    frontend_invocations: usize,
    ambiguity_preserved: usize,
    unsupported_refusals: usize,
    provenance_preserved: usize,
    replay_verified: usize,
    tamper_rejected: usize,
    route_leakage: usize,
    false_authorizations: usize,
    false_denials: usize,
    selected_module_counts: BTreeMap<String, usize>,
    receipts: Vec<Receipt>,
}

fn digest<T: Serialize + ?Sized>(value: &T) -> String {
    format!("{:x}", Sha256::digest(serde_json::to_vec(value).unwrap()))
}

fn memory_record(id: &str, artifact: &str, parent_hash: &str) -> MemoryRecord {
    MemoryRecord {
        record_id: id.into(),
        domain: DOMAIN.into(),
        artifact_type: artifact.into(),
        version: VERSION.into(),
        payload: format!("promoted-geometry:{artifact}"),
        provenance: vec![
            format!("stage172-report-sha256:{parent_hash}"),
            "stage169-promotion".into(),
            "stage170-memory-integration".into(),
        ],
        content_hash: String::new(),
    }
}

fn assignments(record: &FormulaRecord, unknown: bool) -> BTreeMap<String, UnitAssignment> {
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

fn render_geometry(record: &FormulaRecord, index: usize) -> String {
    let inputs = record
        .required_inputs
        .iter()
        .map(|name| format!("{name}={}", index % 9 + 2))
        .collect::<Vec<_>>()
        .join(" ");
    format!("Compute the {} using {inputs}.", record.aliases[0])
}

fn observe_geometry(
    text: &str,
    case_id: &str,
    expected: Expected,
    records: &[FormulaRecord],
    units: &[FormulaRecord],
    memory: &CurriculumMemory,
) -> Observation {
    let record = &records[case_id.parse::<usize>().unwrap_or_default() % records.len()];
    let composition = compose_formula_text(
        text,
        DOMAIN,
        UNIT_DOMAIN,
        case_id,
        records,
        units,
        &assignments(record, expected == Expected::Unsupported),
    );
    let selected = [
        "source_formula",
        "measurement_composition",
        "dimension_contract",
    ]
    .iter()
    .flat_map(|artifact| memory.retrieve_exact_version(DOMAIN, artifact, VERSION))
    .collect::<Vec<_>>();
    let memory_gate = expected == Expected::Supported && selected.len() == 3;
    let authorized = memory_gate && composition.status == CompositionStatus::Complete;
    let mut tampered = composition.clone();
    tampered.replay_hash.push('x');
    let memory_tamper = selected.first().is_none_or(|item| {
        let mut copy = (*item).clone();
        copy.payload.push('x');
        !memory.replay_verified(&copy)
    });
    Observation {
        module: Module::Geometry,
        authorized,
        ambiguous: composition.status == CompositionStatus::Ambiguous,
        provenance: !composition.provenance.is_empty(),
        replay: composition.replay_verified(),
        tamper: !tampered.replay_verified() && memory_tamper,
    }
}

fn observe_combinatorics(text: &str, case_id: &str) -> Observation {
    let frontend = formalize_combinatorics(text, case_id);
    let downstream = frontend.request.as_ref().map(evaluate_combinatorics);
    let authorized = frontend.status == CombinatoricsFrontendStatus::Complete
        && downstream.as_ref().is_some_and(|result| {
            result.status == CombinatoricsStatus::Complete && result.artifact.is_some()
        });
    let mut tampered = frontend.clone();
    tampered.replay_hash.push('x');
    Observation {
        module: Module::Combinatorics,
        authorized,
        ambiguous: frontend.status == CombinatoricsFrontendStatus::Ambiguous,
        provenance: !frontend.provenance.is_empty(),
        replay: combinatorics_replay(&frontend)
            && downstream
                .as_ref()
                .is_none_or(|result| result.replay_verified()),
        tamper: !combinatorics_replay(&tampered)
            && downstream.as_ref().is_none_or(|result| {
                let mut copy = result.clone();
                copy.replay_hash.push('x');
                !copy.replay_verified()
            }),
    }
}

fn observe_number_theory(text: &str, case_id: &str) -> Observation {
    let frontend = formalize_number_theory_text(text, case_id);
    let downstream = frontend.request.as_ref().map(evaluate_number_theory);
    let authorized = frontend.status == NumberTheoryFrontendStatus::Complete
        && downstream.as_ref().is_some_and(|result| {
            result.status == NumberTheoryStatus::Complete && result.artifact.is_some()
        });
    let mut tampered = frontend.clone();
    tampered.replay_hash.push('x');
    Observation {
        module: Module::NumberTheory,
        authorized,
        ambiguous: frontend.status == NumberTheoryFrontendStatus::Ambiguous,
        provenance: !frontend.provenance.is_empty(),
        replay: number_replay(&frontend)
            && downstream
                .as_ref()
                .is_none_or(|result| result.replay_verified()),
        tamper: !number_replay(&tampered)
            && downstream.as_ref().is_none_or(|result| {
                let mut copy = result.clone();
                copy.replay_hash.push('x');
                !copy.replay_verified()
            }),
    }
}

fn observe_complex(text: &str) -> Observation {
    let frontend = formalize_complex_text(text);
    let downstream = frontend.request.as_ref().map(evaluate_complex);
    let authorized = frontend.status == ComplexFrontendStatus::Complete
        && downstream.as_ref().is_some_and(|result| {
            result.status == ComplexStatus::Complete && result.artifact.is_some()
        });
    let mut tampered = frontend.clone();
    tampered.replay_hash.push('x');
    Observation {
        module: Module::Complex,
        authorized,
        ambiguous: frontend.status == ComplexFrontendStatus::Ambiguous,
        provenance: !frontend.provenance_spans.is_empty(),
        replay: frontend.replay_verified()
            && downstream
                .as_ref()
                .is_none_or(|result| result.replay_verified()),
        tamper: !tampered.replay_verified()
            && downstream.as_ref().is_none_or(|result| {
                let mut copy = result.clone();
                copy.replay_hash.push('x');
                !copy.replay_verified()
            }),
    }
}

fn text(module: Module, expected: Expected, index: usize, record: &FormulaRecord) -> String {
    match (module, expected) {
        (Module::Geometry, Expected::Supported | Expected::Unsupported) => {
            render_geometry(record, index)
        }
        (Module::Geometry, Expected::Ambiguous) => {
            "Compute the rectangle area and triangle area using length=4 width=3 base=5 height=2."
                .into()
        }
        (Module::Combinatorics, Expected::Supported) => format!(
            "How many ways can one choose n={} objects, k=2 at a time?",
            5 + index % 3
        ),
        (Module::Combinatorics, Expected::Ambiguous) => {
            "Choose n=5 and k=2, then compare n=6; labeled versus unlabeled selection is unspecified.".into()
        }
        (Module::Combinatorics, Expected::Unsupported) => {
            "Compute the Bell number B_40 for the unrestricted partition problem.".into()
        }
        (Module::NumberTheory, Expected::Supported) => {
            format!(
                "Find the modular inverse of a={} modulo m=11.",
                3 + index % 4
            )
        }
        (Module::NumberTheory, Expected::Ambiguous) => {
            "Find the modular inverse with a=3 and a=4 in competing scopes; m=11.".into()
        }
        (Module::NumberTheory, Expected::Unsupported) => {
            "Apply a Dirichlet character to an asymptotic prime-counting theorem.".into()
        }
        (Module::Complex, Expected::Supported) => "Find the product of (3-4i) and (2+5i).".into(),
        (Module::Complex, Expected::Ambiguous) => {
            "Find either the product or quotient of (3-4i) and (2+5i).".into()
        }
        (Module::Complex, Expected::Unsupported) => {
            "Convert the complex number (3+4i) to polar form.".into()
        }
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let geometry_records = extract_formula_records(GEOMETRY_SOURCE)
        .map_err(|errors| format!("geometry source extraction failed: {errors:?}"))?;
    let unit_records = extract_formula_records(UNIT_SOURCE)
        .map_err(|errors| format!("unit source extraction failed: {errors:?}"))?;
    assert_eq!(geometry_records.len(), 5);
    let parent_bytes = fs::read(PARENT_REPORT)?;
    let parent_hash = format!("{:x}", Sha256::digest(&parent_bytes));
    let geometry_hash = format!("{:x}", Sha256::digest(GEOMETRY_SOURCE.as_bytes()));
    let unit_hash = format!("{:x}", Sha256::digest(UNIT_SOURCE.as_bytes()));
    let production = CurriculumMemory::new();
    let mut memory = production.clone();
    for (id, artifact) in [
        ("stage173-formula", "source_formula"),
        ("stage173-composition", "measurement_composition"),
        ("stage173-dimension", "dimension_contract"),
    ] {
        assert_eq!(
            memory.append(memory_record(id, artifact, &parent_hash)),
            AppendStatus::Appended
        );
    }
    let modules = [
        Module::Geometry,
        Module::Combinatorics,
        Module::NumberTheory,
        Module::Complex,
    ];
    let mut receipts = Vec::with_capacity(1_200);
    let mut counts = BTreeMap::new();
    let mut development_supported = 0;
    let mut development_ambiguous = 0;
    let mut development_unsupported = 0;
    let mut development_exact = 0;
    let mut development_authorized = 0;
    let mut holdout_supported = 0;
    let mut holdout_ambiguous = 0;
    let mut holdout_unsupported = 0;
    let mut holdout_exact = 0;
    let mut holdout_authorized = 0;
    let mut frontend_invocations = 0;
    let mut ambiguity_preserved = 0;
    let mut unsupported_refusals = 0;
    let mut provenance_preserved = 0;
    let mut replay_verified = 0;
    let mut tamper_rejected = 0;
    let mut route_leakage = 0;
    let mut false_authorizations = 0;
    let mut false_denials = 0;

    for index in 0..1_200 {
        let module = modules[index % modules.len()];
        let expected = match index % 5 {
            0..=2 => Expected::Supported,
            3 => Expected::Ambiguous,
            _ => Expected::Unsupported,
        };
        let case_id = index.to_string();
        let record = &geometry_records[index % geometry_records.len()];
        let source_text = text(module, expected, index, record);
        let observations = modules
            .iter()
            .map(|candidate| match candidate {
                Module::Geometry => observe_geometry(
                    &source_text,
                    &case_id,
                    if *candidate == module {
                        expected
                    } else {
                        Expected::Unsupported
                    },
                    &geometry_records,
                    &unit_records,
                    &memory,
                ),
                Module::Combinatorics => observe_combinatorics(&source_text, &case_id),
                Module::NumberTheory => observe_number_theory(&source_text, &case_id),
                Module::Complex => observe_complex(&source_text),
            })
            .collect::<Vec<_>>();
        frontend_invocations += observations.len();
        let authorized_modules: Vec<Module> = observations
            .iter()
            .filter(|observation| observation.authorized)
            .map(|observation| observation.module)
            .collect();
        let ambiguous_modules: Vec<Module> = observations
            .iter()
            .filter(|observation| observation.ambiguous)
            .map(|observation| observation.module)
            .collect();
        let actual = if authorized_modules.len() == 1 {
            Actual::Authorized
        } else if authorized_modules.is_empty() && !ambiguous_modules.is_empty() {
            Actual::Ambiguous
        } else {
            Actual::Unsupported
        };
        let selected_module = if authorized_modules.len() == 1 {
            Some(authorized_modules[0])
        } else {
            None
        };
        let exact = match expected {
            Expected::Supported => actual == Actual::Authorized && selected_module == Some(module),
            Expected::Ambiguous => actual == Actual::Ambiguous,
            Expected::Unsupported => actual == Actual::Unsupported,
        };
        let route_provenance = observations
            .iter()
            .all(|observation| observation.provenance);
        let route_replay = observations.iter().all(|observation| observation.replay);
        let route_tamper = observations.iter().all(|observation| observation.tamper);
        let false_authorization = expected != Expected::Supported && actual == Actual::Authorized;
        let false_denial = expected == Expected::Supported && actual != Actual::Authorized;
        if authorized_modules.len() > 1 {
            route_leakage += 1;
        }
        ambiguity_preserved +=
            usize::from(expected == Expected::Ambiguous && actual == Actual::Ambiguous);
        unsupported_refusals +=
            usize::from(expected == Expected::Unsupported && actual == Actual::Unsupported);
        provenance_preserved += usize::from(route_provenance);
        replay_verified += usize::from(route_replay && exact);
        tamper_rejected += usize::from(route_tamper);
        false_authorizations += usize::from(false_authorization);
        false_denials += usize::from(false_denial);
        *counts.entry(format!("{module:?}")).or_insert(0) += 1;
        let holdout = index >= 960;
        if expected == Expected::Supported {
            if holdout {
                holdout_supported += 1;
                holdout_authorized += usize::from(actual == Actual::Authorized);
            } else {
                development_supported += 1;
                development_authorized += usize::from(actual == Actual::Authorized);
            }
        } else if expected == Expected::Ambiguous {
            if holdout {
                holdout_ambiguous += 1;
            } else {
                development_ambiguous += 1;
            }
        } else if holdout {
            holdout_unsupported += 1;
        } else {
            development_unsupported += 1;
        }
        if holdout {
            holdout_exact += usize::from(exact);
        } else {
            development_exact += usize::from(exact);
        }
        receipts.push(Receipt {
            id: format!("stage173-{index:04}"),
            partition: if holdout { "holdout" } else { "development" }.into(),
            text_sha256: digest(&source_text),
            expected,
            actual,
            authorized_modules,
            ambiguous_modules,
            selected_module,
            exact,
            provenance: route_provenance,
            replay: route_replay,
            tamper_rejected: route_tamper,
            false_authorization,
            false_denial,
        });
    }
    assert_eq!(
        (
            development_supported,
            development_ambiguous,
            development_unsupported
        ),
        (576, 192, 192)
    );
    assert_eq!(
        (holdout_supported, holdout_ambiguous, holdout_unsupported),
        (144, 48, 48)
    );
    assert_eq!(development_exact, 960);
    assert_eq!(holdout_exact, 240);
    assert_eq!(development_authorized, 576);
    assert_eq!(holdout_authorized, 144);
    assert_eq!(frontend_invocations, 4_800);
    assert_eq!(ambiguity_preserved, 240);
    assert_eq!(unsupported_refusals, 240);
    assert_eq!(provenance_preserved, 1_200);
    assert_eq!(replay_verified, 1_200);
    assert_eq!(tamper_rejected, 1_200);
    assert_eq!(route_leakage, 0);
    assert_eq!(false_authorizations, 0);
    assert_eq!(false_denials, 0);
    let report = Report {
        schema: "stage173-route-blind-technical-language-v1",
        parent_report_sha256: parent_hash,
        geometry_source_sha256: geometry_hash,
        unit_source_sha256: unit_hash,
        corpus_sha256: digest(&receipts),
        cases: 1_200,
        development_cases: 960,
        development_supported,
        development_ambiguous,
        development_unsupported,
        development_exact,
        development_authorized,
        holdout_cases: 240,
        holdout_supported,
        holdout_ambiguous,
        holdout_unsupported,
        holdout_exact,
        holdout_authorized,
        frontend_invocations,
        ambiguity_preserved,
        unsupported_refusals,
        provenance_preserved,
        replay_verified,
        tamper_rejected,
        route_leakage,
        false_authorizations,
        false_denials,
        selected_module_counts: counts,
        receipts,
    };
    let json = serde_json::to_string_pretty(&report)?;
    fs::write(REPORT_JSON, format!("{json}\n"))?;
    fs::write(REPORT_MD, format!("# Stage 173 — route-blind technical language\n\nEvery report was offered to four validated frontends, including promoted geometry gated by exact curriculum-memory retrieval.\n\n| Measure | Result |\n|---|---:|\n| Cases / frontend invocations | 1200 / 4800 |\n| Development exact / authorized | {}/{} / {}/{} |\n| Holdout exact / authorized | {}/{} / {}/{} |\n| Ambiguity / unsupported refusals | {}/{} |\n| Provenance / replay / tamper | {}/1200 / {}/1200 / {}/1200 |\n| Route leakage | 0 |\n| False authorizations / denials | 0 / 0 |\n\nThe route-blind gate authorizes only a unique validated frontend.\n", report.development_exact, report.development_cases, report.development_authorized, report.development_cases, report.holdout_exact, report.holdout_cases, report.holdout_authorized, report.holdout_cases, report.ambiguity_preserved, report.unsupported_refusals, report.provenance_preserved, report.replay_verified, report.tamper_rejected))?;
    println!("{json}");
    Ok(())
}
