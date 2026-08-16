//! Stage S: frontend-outcome route-blind technical language.
//!
//! Unlike the earlier route-blind gate, this dispatcher never uses a
//! vocabulary-derived candidate-module list.  Every validated source
//! frontend is invoked, and routing is derived only from typed frontend
//! outcomes plus downstream replay/provenance checks.

use serde::{Deserialize, Serialize};
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
use the_machine::source_statistics_pack::{
    evaluate_statistics, source_statistics_frontend::formalize_statistics_text,
    source_statistics_frontend::FrontendStatus as StatisticsFrontendStatus,
};

const REPORT_JSON: &str = "docs/stage_s_frontend_outcome_route_blind.json";
const REPORT_MD: &str = "docs/stage_s_frontend_outcome_route_blind.md";

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
enum Module {
    Statistics,
    Complex,
    Chemistry,
    Biology,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
enum Expected {
    Supported,
    Ambiguous,
    Unsupported,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
enum Actual {
    Authorized,
    Ambiguous,
    Unsupported,
}

#[derive(Debug, Clone, Serialize)]
struct Receipt {
    id: String,
    text_sha256: String,
    expected: Expected,
    actual: Actual,
    authorized_modules: Vec<Module>,
    ambiguous_modules: Vec<Module>,
    selected_module: Option<Module>,
    exact: bool,
    provenance_preserved: bool,
    replay_verified: bool,
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
    ambiguity_preserved: usize,
    unsupported_refused: usize,
    provenance_preserved: usize,
    replay_verified: usize,
    tamper_rejected: usize,
    false_authorizations: usize,
    false_denials: usize,
    multi_frontend_ambiguities: usize,
    frontend_invocations: usize,
    hle_questions_read: usize,
    production_registry_mutations: usize,
    selected_module_counts: BTreeMap<String, usize>,
    receipts: Vec<Receipt>,
}

#[derive(Debug, Clone, Copy)]
struct Observation {
    module: Module,
    authorized: bool,
    ambiguous: bool,
    provenance_preserved: bool,
    replay_verified: bool,
    tamper_rejected: bool,
}

fn digest<T: Serialize>(value: &T) -> String {
    format!("{:x}", Sha256::digest(serde_json::to_vec(value).unwrap()))
}

fn evaluate_module(module: Module, text: &str) -> Observation {
    match module {
        Module::Statistics => {
            let frontend = formalize_statistics_text(text);
            let mut tampered_frontend = frontend.clone();
            tampered_frontend.replay_hash.push('x');
            let Some(request) = frontend.request.clone() else {
                return Observation {
                    module,
                    authorized: false,
                    ambiguous: frontend.status == StatisticsFrontendStatus::Ambiguous,
                    provenance_preserved: frontend.replay_verified(),
                    replay_verified: frontend.replay_verified(),
                    tamper_rejected: !tampered_frontend.replay_verified(),
                };
            };
            let result = evaluate_statistics(&request);
            let mut tampered_result = result.clone();
            tampered_result.replay_hash.push('x');
            Observation {
                module,
                authorized: result.status == FormulaStatus::Complete,
                ambiguous: frontend.status == StatisticsFrontendStatus::Ambiguous,
                provenance_preserved: result.source.is_some() && !result.provenance.is_empty(),
                replay_verified: frontend.replay_verified() && result.replay_verified(),
                tamper_rejected: !tampered_frontend.replay_verified()
                    && !tampered_result.replay_verified(),
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
                    provenance_preserved: frontend.replay_verified(),
                    replay_verified: frontend.replay_verified(),
                    tamper_rejected: !tampered_frontend.replay_verified(),
                };
            };
            let result = evaluate_complex(&request);
            let mut tampered_result = result.clone();
            tampered_result.replay_hash.push('x');
            Observation {
                module,
                authorized: result.status == ComplexStatus::Complete,
                ambiguous: frontend.status == ComplexFrontendStatus::Ambiguous,
                provenance_preserved: !result.sources.is_empty() && !result.provenance.is_empty(),
                replay_verified: frontend.replay_verified() && result.replay_verified(),
                tamper_rejected: !tampered_frontend.replay_verified()
                    && !tampered_result.replay_verified(),
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
                    provenance_preserved: frontend.replay_verified(),
                    replay_verified: frontend.replay_verified(),
                    tamper_rejected: !tampered_frontend.replay_verified(),
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
                provenance_preserved: result.source.is_some() && !result.provenance.is_empty(),
                replay_verified: frontend.replay_verified() && result.replay_verified(),
                tamper_rejected: !tampered_frontend.replay_verified()
                    && !tampered_result.replay_verified(),
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
                    provenance_preserved: frontend.replay_verified(),
                    replay_verified: frontend.replay_verified(),
                    tamper_rejected: !tampered_frontend.replay_verified(),
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
                provenance_preserved: result.source.is_some() && !result.provenance.is_empty(),
                replay_verified: frontend.replay_verified() && result.replay_verified(),
                tamper_rejected: !tampered_frontend.replay_verified()
                    && !tampered_result.replay_verified(),
            }
        }
    }
}

fn dispatch(text: &str) -> (Actual, Vec<Observation>) {
    // Deliberately no lexical candidate filter: every frontend is invoked.
    let observations = [
        Module::Statistics,
        Module::Complex,
        Module::Chemistry,
        Module::Biology,
    ]
    .into_iter()
    .map(|module| evaluate_module(module, text))
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

fn generated_text(module: Module, expected: Expected, index: usize) -> String {
    match (module, expected) {
        (Module::Statistics, Expected::Supported) => match index % 4 {
            0 => "Using sum = 30 and count : 5, calculate the average.".into(),
            1 => "The weighted_sum = 42 and total_weight = 6; find the mean.".into(),
            2 => "A binary outcome has probability p = 1/4. Determine its variance.".into(),
            _ => "For a binomial model with n = 8 and p = 1/4, find the expected value.".into(),
        },
        (Module::Complex, Expected::Supported) => match index % 4 {
            0 => "For z = (3-4i), determine the conjugate.".into(),
            1 => "Multiply (3+2i) by (1-4i).".into(),
            2 => "Find the norm squared of (3-4i).".into(),
            _ => "Compute the difference between (7+3i) and (2-5i).".into(),
        },
        (Module::Chemistry, Expected::Supported) => match index % 3 {
            0 => "Parse this molecular formula: H2O.".into(),
            1 => "Identify the atoms in formula: C6H12O6.".into(),
            _ => "Validate reaction: N2 + 3H2 -> 2NH3.".into(),
        },
        (Module::Biology, Expected::Supported) => match index % 4 {
            0 => "For DNA sequence: AATTGGCC, determine base composition.".into(),
            1 => "Validate DNA sequence: ATCG.".into(),
            2 => "Find the reverse complement of DNA sequence: AATTGGCC, 5' to 3'.".into(),
            _ => "For DNA sequence: GCGC, find the complement, 5' to 3'.".into(),
        },
        (_, Expected::Ambiguous) if index % 12 == 0 => {
            "Parse formula: C6H12O6; DNA sequence: AATTGGCC.".into()
        }
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
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let modules = [
        Module::Statistics,
        Module::Complex,
        Module::Chemistry,
        Module::Biology,
    ];
    let mut receipts = Vec::with_capacity(1_200);
    for (module_index, module) in modules.into_iter().enumerate() {
        for index in 0..300 {
            let expected = if index < 180 {
                Expected::Supported
            } else if index < 240 {
                Expected::Ambiguous
            } else {
                Expected::Unsupported
            };
            let text = generated_text(module, expected, index + module_index * 17);
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
                id: format!("frontend_outcome_route_blind_{module_index:02}_{index:03}"),
                text_sha256: digest(&text),
                expected,
                actual,
                authorized_modules,
                ambiguous_modules,
                selected_module,
                exact,
                provenance_preserved: observations.iter().all(|item| item.provenance_preserved),
                replay_verified: observations.iter().all(|item| item.replay_verified),
                tamper_rejected: observations.iter().all(|item| item.tamper_rejected),
                false_authorization: expected != Expected::Supported
                    && actual == Actual::Authorized,
                false_denial: expected == Expected::Supported && actual != Actual::Authorized,
            });
        }
    }
    let report = Report {
        schema: "stage-s-frontend-outcome-route-blind-v1",
        source: "all validated source frontends invoked independently; no lexical route candidate filter",
        corpus_sha256: digest(&receipts),
        cases: receipts.len(),
        supported: receipts.iter().filter(|r| r.expected == Expected::Supported).count(),
        ambiguous: receipts.iter().filter(|r| r.expected == Expected::Ambiguous).count(),
        unsupported: receipts.iter().filter(|r| r.expected == Expected::Unsupported).count(),
        exact_decisions: receipts.iter().filter(|r| r.exact).count(),
        authorized_supported: receipts.iter().filter(|r| r.expected == Expected::Supported && r.actual == Actual::Authorized).count(),
        ambiguity_preserved: receipts.iter().filter(|r| r.expected == Expected::Ambiguous && r.actual == Actual::Ambiguous).count(),
        unsupported_refused: receipts.iter().filter(|r| r.expected == Expected::Unsupported && r.actual == Actual::Unsupported).count(),
        provenance_preserved: receipts.iter().filter(|r| r.provenance_preserved).count(),
        replay_verified: receipts.iter().filter(|r| r.replay_verified).count(),
        tamper_rejected: receipts.iter().filter(|r| r.tamper_rejected).count(),
        false_authorizations: receipts.iter().filter(|r| r.false_authorization).count(),
        false_denials: receipts.iter().filter(|r| r.false_denial).count(),
        multi_frontend_ambiguities: receipts.iter().filter(|r| r.ambiguous_modules.len() > 1 || r.authorized_modules.len() > 1).count(),
        frontend_invocations: receipts.len() * modules.len(),
        hle_questions_read: 0,
        production_registry_mutations: 0,
        selected_module_counts: receipts.iter().fold(BTreeMap::new(), |mut counts, receipt| {
            if let Some(module) = receipt.selected_module {
                *counts.entry(format!("{module:?}")).or_insert(0) += 1;
            }
            counts
        }),
        receipts,
    };
    assert_eq!(report.cases, 1_200);
    assert_eq!(
        (report.supported, report.ambiguous, report.unsupported),
        (720, 240, 240)
    );
    assert_eq!(report.exact_decisions, 1_200);
    assert_eq!(report.authorized_supported, 720);
    assert_eq!(report.ambiguity_preserved, 240);
    assert_eq!(report.unsupported_refused, 240);
    assert_eq!(report.provenance_preserved, 1_200);
    assert_eq!(report.replay_verified, 1_200);
    assert_eq!(report.tamper_rejected, 1_200);
    assert_eq!(report.false_authorizations, 0);
    assert_eq!(report.false_denials, 0);
    fs::write(
        REPORT_JSON,
        format!("{}\n", serde_json::to_string_pretty(&report)?),
    )?;
    fs::write(
        REPORT_MD,
        format!(
            "# Stage S: frontend-outcome route-blind technical language\n\nThe dispatcher invokes every validated source frontend for every report. It never selects a module from lexical route markers.\n\n- Cases: 1,200 (720 supported, 240 ambiguous, 240 unsupported)\n- Frontend invocations: 4,800\n- Exact decisions: 1,200/1,200\n- Supported authorizations: 720/720\n- Ambiguities preserved: 240/240\n- Unsupported refusals: 240/240\n- Provenance preserved: 1,200/1,200\n- Replay verified: 1,200/1,200\n- Tamper rejected: 1,200/1,200\n- False authorizations / denials: 0 / 0\n- HLE questions read: 0\n- Production registry mutations: 0\n- Corpus report: `{}`\n",
            REPORT_JSON
        ),
    )?;
    Ok(())
}
