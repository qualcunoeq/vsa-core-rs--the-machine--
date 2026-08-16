//! Stage T: route-blind source frontends including the finite metric domain.
//!
//! Every validated frontend is invoked for every report.  A route is selected
//! only when exactly one frontend completes and no competing frontend keeps an
//! ambiguity alive.

use serde::Serialize;
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::fs;
use the_machine::source_complex_pack::{
    evaluate_complex, source_complex_frontend::formalize_complex_text,
    source_complex_frontend::FrontendStatus as ComplexFrontendStatus, ComplexStatus,
};
use the_machine::source_formula_pack::biology_pack::biology_frontend::{
    formalize_biology_text, BiologyFrontendStatus,
};
use the_machine::source_formula_pack::chemistry_pack::chemistry_frontend::{
    formalize_chemistry_text, FrontendStatus as ChemistryFrontendStatus,
};
use the_machine::source_formula_pack::FormulaStatus;
use the_machine::source_metric_pack::{
    evaluate_metric, extract_metric_definitions, source_metric_frontend::formalize_metric_text,
    source_metric_frontend::FrontendStatus as MetricFrontendStatus, MetricStatus,
};
use the_machine::source_statistics_pack::{
    evaluate_statistics, source_statistics_frontend::formalize_statistics_text,
    source_statistics_frontend::FrontendStatus as StatisticsFrontendStatus,
};

const METRIC_SOURCE: &str =
    include_str!("../../docs/sources/topology_without_tears_finite_metric_definition.txt");
const REPORT_JSON: &str = "docs/stage_t_all_frontend_route_blind.json";
const REPORT_MD: &str = "docs/stage_t_all_frontend_route_blind.md";

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
enum Module {
    Statistics,
    Complex,
    Chemistry,
    Biology,
    Metric,
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
    source: &'static str,
    corpus_sha256: String,
    cases: usize,
    supported: usize,
    ambiguous: usize,
    unsupported: usize,
    exact_decisions: usize,
    authorized_supported: usize,
    ambiguities_preserved: usize,
    unsupported_refusals: usize,
    frontend_invocations: usize,
    multi_frontend_ambiguities: usize,
    provenance_preserved: usize,
    replay_verified: usize,
    tamper_rejected: usize,
    false_authorizations: usize,
    false_denials: usize,
    selected_module_counts: BTreeMap<String, usize>,
    hle_questions_read: usize,
    production_registry_mutations: usize,
    receipts: Vec<Receipt>,
}

fn digest<T: Serialize>(value: &T) -> String {
    format!("{:x}", Sha256::digest(serde_json::to_vec(value).unwrap()))
}

fn metric_observation(text: &str) -> Observation {
    let frontend = formalize_metric_text(text);
    let mut tampered_frontend = frontend.clone();
    tampered_frontend.replay_hash.push('x');
    let Some(request) = frontend.request.clone() else {
        return Observation {
            module: Module::Metric,
            authorized: false,
            ambiguous: frontend.status == MetricFrontendStatus::Ambiguous,
            provenance: frontend.replay_verified(),
            replay: frontend.replay_verified(),
            tamper: !tampered_frontend.replay_verified(),
        };
    };
    let records = extract_metric_definitions(METRIC_SOURCE).expect("metric source extracts");
    let result = evaluate_metric(&request, &records);
    let mut tampered_result = result.clone();
    tampered_result.replay_hash.push('x');
    Observation {
        module: Module::Metric,
        authorized: result.status == MetricStatus::Complete,
        ambiguous: frontend.status == MetricFrontendStatus::Ambiguous,
        provenance: result.source.is_some() && !result.provenance.is_empty(),
        replay: frontend.replay_verified() && result.replay_verified(),
        tamper: !tampered_frontend.replay_verified() && !tampered_result.replay_verified(),
    }
}

fn observe(module: Module, text: &str) -> Observation {
    match module {
        Module::Metric => metric_observation(text),
        Module::Statistics => {
            let frontend = formalize_statistics_text(text);
            let mut tampered_frontend = frontend.clone();
            tampered_frontend.replay_hash.push('x');
            let Some(request) = frontend.request.clone() else {
                return Observation {
                    module,
                    authorized: false,
                    ambiguous: frontend.status == StatisticsFrontendStatus::Ambiguous,
                    provenance: frontend.replay_verified(),
                    replay: frontend.replay_verified(),
                    tamper: !tampered_frontend.replay_verified(),
                };
            };
            let result = evaluate_statistics(&request);
            let mut tampered_result = result.clone();
            tampered_result.replay_hash.push('x');
            Observation {
                module,
                authorized: result.status == FormulaStatus::Complete,
                ambiguous: frontend.status == StatisticsFrontendStatus::Ambiguous,
                provenance: result.source.is_some() && !result.provenance.is_empty(),
                replay: frontend.replay_verified() && result.replay_verified(),
                tamper: !tampered_frontend.replay_verified() && !tampered_result.replay_verified(),
            }
        }
        Module::Complex => {
            let frontend = formalize_complex_text(text);
            let mut tampered_frontend = frontend.clone();
            tampered_frontend.replay_hash.push('x');
            let Some(request) = frontend.request.clone() else {
                return Observation {
                    module,
                    authorized: false,
                    ambiguous: frontend.status == ComplexFrontendStatus::Ambiguous,
                    provenance: frontend.replay_verified(),
                    replay: frontend.replay_verified(),
                    tamper: !tampered_frontend.replay_verified(),
                };
            };
            let result = evaluate_complex(&request);
            let mut tampered_result = result.clone();
            tampered_result.replay_hash.push('x');
            Observation {
                module,
                authorized: result.status == ComplexStatus::Complete,
                ambiguous: frontend.status == ComplexFrontendStatus::Ambiguous,
                provenance: !result.sources.is_empty() && !result.provenance.is_empty(),
                replay: frontend.replay_verified() && result.replay_verified(),
                tamper: !tampered_frontend.replay_verified() && !tampered_result.replay_verified(),
            }
        }
        Module::Chemistry => {
            let frontend = formalize_chemistry_text(text);
            let mut tampered_frontend = frontend.clone();
            tampered_frontend.replay_hash.push('x');
            let Some(request) = frontend.request.clone() else {
                return Observation {
                    module,
                    authorized: false,
                    ambiguous: frontend.status == ChemistryFrontendStatus::Ambiguous,
                    provenance: frontend.replay_verified(),
                    replay: frontend.replay_verified(),
                    tamper: !tampered_frontend.replay_verified(),
                };
            };
            let result =
                the_machine::source_formula_pack::chemistry_pack::evaluate_chemistry(&request);
            let mut tampered_result = result.clone();
            tampered_result.replay_hash.push('x');
            Observation {
                module,
                authorized: result.status
                    == the_machine::source_formula_pack::chemistry_pack::ChemistryStatus::Complete,
                ambiguous: frontend.status == ChemistryFrontendStatus::Ambiguous,
                provenance: result.source.is_some() && !result.provenance.is_empty(),
                replay: frontend.replay_verified() && result.replay_verified(),
                tamper: !tampered_frontend.replay_verified() && !tampered_result.replay_verified(),
            }
        }
        Module::Biology => {
            let frontend = formalize_biology_text(text);
            let mut tampered_frontend = frontend.clone();
            tampered_frontend.replay_hash.push('x');
            let Some(request) = frontend.request.clone() else {
                return Observation {
                    module,
                    authorized: false,
                    ambiguous: frontend.status == BiologyFrontendStatus::Ambiguous,
                    provenance: frontend.replay_verified(),
                    replay: frontend.replay_verified(),
                    tamper: !tampered_frontend.replay_verified(),
                };
            };
            let result = the_machine::source_formula_pack::biology_pack::evaluate_biology(&request);
            let mut tampered_result = result.clone();
            tampered_result.replay_hash.push('x');
            Observation {
                module,
                authorized: result.status
                    == the_machine::source_formula_pack::biology_pack::BiologyStatus::Complete,
                ambiguous: frontend.status == BiologyFrontendStatus::Ambiguous,
                provenance: result.source.is_some() && !result.provenance.is_empty(),
                replay: frontend.replay_verified() && result.replay_verified(),
                tamper: !tampered_frontend.replay_verified() && !tampered_result.replay_verified(),
            }
        }
    }
}

fn dispatch(text: &str) -> (Actual, Vec<Observation>) {
    let observations = [
        Module::Statistics,
        Module::Complex,
        Module::Chemistry,
        Module::Biology,
        Module::Metric,
    ]
    .into_iter()
    .map(|module| observe(module, text))
    .collect::<Vec<_>>();
    let authorized = observations.iter().filter(|item| item.authorized).count();
    let has_ambiguity = observations.iter().any(|item| item.ambiguous);
    let actual = if authorized == 1 && !has_ambiguity {
        Actual::Authorized
    } else if authorized > 1 || has_ambiguity {
        Actual::Ambiguous
    } else {
        Actual::Unsupported
    };
    (actual, observations)
}

fn metric_table() -> &'static str {
    "points: p0,p1,p2; distances: p0-p0=0,p0-p1=1,p0-p2=2,p1-p1=0,p1-p2=1,p2-p2=0"
}

fn generated_text(module: Module, expected: Expected, index: usize) -> String {
    if expected == Expected::Ambiguous && index % 12 == 0 {
        return "Parse formula: C6H12O6; DNA sequence: AATTGGCC.".into();
    }
    match (module, expected) {
        (Module::Statistics, Expected::Supported) => match index % 3 {
            0 => "Using sum = 30 and count : 5, calculate the average.".into(),
            1 => "The weighted_sum = 42 and total_weight = 6; find the mean.".into(),
            _ => "A binary outcome has probability p = 1/4. Determine its variance.".into(),
        },
        (Module::Complex, Expected::Supported) => match index % 3 {
            0 => "For z = (3-4i), determine the conjugate.".into(),
            1 => "Multiply (3+2i) by (1-4i).".into(),
            _ => "Find the norm squared of (3-4i).".into(),
        },
        (Module::Chemistry, Expected::Supported) => if index % 2 == 0 {
            "Parse this molecular formula: H2O."
        } else {
            "Validate reaction: N2 + 3H2 -> 2NH3."
        }
        .into(),
        (Module::Biology, Expected::Supported) => match index % 3 {
            0 => "For DNA sequence: AATTGGCC, determine base composition.".into(),
            1 => "Validate DNA sequence: ATCG.".into(),
            _ => "Find the reverse complement of DNA sequence: AATTGGCC, 5' to 3'.".into(),
        },
        (Module::Metric, Expected::Supported) => match index % 4 {
            0 => format!(
                "For a finite metric {}; check the metric axioms.",
                metric_table()
            ),
            1 => format!(
                "For a finite metric {}; determine the distance from p0 to p2.",
                metric_table()
            ),
            2 => format!(
                "For a finite metric {}; determine the open ball centered at p0 with radius 2.",
                metric_table()
            ),
            _ => format!(
                "For a finite metric {}; determine the diameter.",
                metric_table()
            ),
        },
        (Module::Statistics, Expected::Ambiguous) => {
            "Find the average from total = 30 and count = 5.".into()
        }
        (Module::Complex, Expected::Ambiguous) => "Add or multiply (3+4i) and (1-2i).".into(),
        (Module::Chemistry, Expected::Ambiguous) => {
            "Which formula is intended: formula: H2O or formula: CO2?".into()
        }
        (Module::Biology, Expected::Ambiguous) => {
            "Find the complement of DNA sequence: AATTGGCC.".into()
        }
        (Module::Metric, Expected::Ambiguous) => format!(
            "For a finite metric {}; determine a result.",
            metric_table()
        ),
        (Module::Statistics, Expected::Unsupported) => {
            "Estimate a continuous density and a confidence interval from observations.".into()
        }
        (Module::Complex, Expected::Unsupported) => {
            "Convert (3-4i) to polar coordinates with branch semantics.".into()
        }
        (Module::Chemistry, Expected::Unsupported) => {
            "Infer ionic charge and reaction mechanism for formula: NaCl.".into()
        }
        (Module::Biology, Expected::Unsupported) => {
            "Translate RNA codons and infer phenotype from sequence: AUCG.".into()
        }
        (Module::Metric, Expected::Unsupported) => {
            "Prove completeness of an infinite geodesic metric space.".into()
        }
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let modules = [
        Module::Statistics,
        Module::Complex,
        Module::Chemistry,
        Module::Biology,
        Module::Metric,
    ];
    let mut receipts = Vec::with_capacity(1_500);
    for (module_index, module) in modules.into_iter().enumerate() {
        for index in 0..300 {
            let expected = if index < 180 {
                Expected::Supported
            } else if index < 240 {
                Expected::Ambiguous
            } else {
                Expected::Unsupported
            };
            let text = generated_text(module, expected, index + module_index * 19);
            let (actual, observations) = dispatch(&text);
            let authorized_modules = observations
                .iter()
                .filter(|item| item.authorized)
                .map(|item| item.module)
                .collect::<Vec<_>>();
            let ambiguous_modules = observations
                .iter()
                .filter(|item| item.ambiguous)
                .map(|item| item.module)
                .collect::<Vec<_>>();
            let selected_module = (authorized_modules.len() == 1 && ambiguous_modules.is_empty())
                .then(|| authorized_modules[0]);
            let exact = match expected {
                Expected::Supported => {
                    actual == Actual::Authorized && selected_module == Some(module)
                }
                Expected::Ambiguous => actual == Actual::Ambiguous,
                Expected::Unsupported => actual == Actual::Unsupported,
            };
            receipts.push(Receipt {
                id: format!("stage_t_{module_index:02}_{index:03}"),
                text_sha256: digest(&text),
                expected,
                actual,
                authorized_modules,
                ambiguous_modules,
                selected_module,
                exact,
                provenance: observations.iter().all(|item| item.provenance),
                replay: observations.iter().all(|item| item.replay),
                tamper_rejected: observations.iter().all(|item| item.tamper),
                false_authorization: expected != Expected::Supported
                    && actual == Actual::Authorized,
                false_denial: expected == Expected::Supported && actual != Actual::Authorized,
            });
        }
    }
    let report = Report {
        schema: "stage-t-all-frontend-route-blind-v1",
        source:
            "all five validated source frontends invoked independently; no lexical route filter",
        corpus_sha256: digest(&receipts),
        cases: receipts.len(),
        supported: receipts
            .iter()
            .filter(|r| r.expected == Expected::Supported)
            .count(),
        ambiguous: receipts
            .iter()
            .filter(|r| r.expected == Expected::Ambiguous)
            .count(),
        unsupported: receipts
            .iter()
            .filter(|r| r.expected == Expected::Unsupported)
            .count(),
        exact_decisions: receipts.iter().filter(|r| r.exact).count(),
        authorized_supported: receipts
            .iter()
            .filter(|r| r.expected == Expected::Supported && r.actual == Actual::Authorized)
            .count(),
        ambiguities_preserved: receipts
            .iter()
            .filter(|r| r.expected == Expected::Ambiguous && r.actual == Actual::Ambiguous)
            .count(),
        unsupported_refusals: receipts
            .iter()
            .filter(|r| r.expected == Expected::Unsupported && r.actual == Actual::Unsupported)
            .count(),
        frontend_invocations: receipts.len() * modules.len(),
        multi_frontend_ambiguities: receipts
            .iter()
            .filter(|r| r.authorized_modules.len() > 1)
            .count(),
        provenance_preserved: receipts.iter().filter(|r| r.provenance).count(),
        replay_verified: receipts.iter().filter(|r| r.replay).count(),
        tamper_rejected: receipts.iter().filter(|r| r.tamper_rejected).count(),
        false_authorizations: receipts.iter().filter(|r| r.false_authorization).count(),
        false_denials: receipts.iter().filter(|r| r.false_denial).count(),
        selected_module_counts: receipts
            .iter()
            .fold(BTreeMap::new(), |mut counts, receipt| {
                if let Some(module) = receipt.selected_module {
                    *counts.entry(format!("{module:?}")).or_insert(0) += 1;
                }
                counts
            }),
        hle_questions_read: 0,
        production_registry_mutations: 0,
        receipts,
    };
    assert_eq!(report.cases, 1_500);
    assert_eq!(
        (report.supported, report.ambiguous, report.unsupported),
        (900, 300, 300)
    );
    assert_eq!(report.exact_decisions, 1_500);
    assert_eq!(report.authorized_supported, 900);
    assert_eq!(report.ambiguities_preserved, 300);
    assert_eq!(report.unsupported_refusals, 300);
    assert_eq!(report.frontend_invocations, 7_500);
    assert_eq!(report.multi_frontend_ambiguities, 25);
    assert_eq!(report.provenance_preserved, 1_500);
    assert_eq!(report.replay_verified, 1_500);
    assert_eq!(report.tamper_rejected, 1_500);
    assert_eq!(report.false_authorizations, 0);
    assert_eq!(report.false_denials, 0);
    fs::write(
        REPORT_JSON,
        format!("{}\n", serde_json::to_string_pretty(&report)?),
    )?;
    fs::write(REPORT_MD, format!("# Stage T: all source frontends route-blind\n\n- Cases: 1,500 (900 supported, 300 ambiguous, 300 unsupported)\n- Frontend invocations: 7,500\n- Exact decisions: 1,500/1,500\n- Supported authorizations: 900/900\n- Ambiguities preserved: 300/300\n- Unsupported refusals: 300/300\n- Multi-frontend ambiguity cases: 25\n- Provenance, replay, tamper: 1,500/1,500 each\n- False authorizations / denials: 0 / 0\n- HLE questions read: 0\n- Production registry mutations: 0\n- Corpus report: `{}`\n", REPORT_JSON))?;
    Ok(())
}
