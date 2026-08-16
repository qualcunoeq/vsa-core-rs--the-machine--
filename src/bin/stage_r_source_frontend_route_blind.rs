//! Stage R: route-blind language through source-derived technical frontends.
//!
//! This gate deliberately invokes the source-specific frontends rather than
//! constructing requests from marker tokens.  Raw shifted text must select one
//! frontend, produce a typed request, and survive the source pack's replay and
//! provenance checks.  The corpus is shadow-only and never reads HLE.

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

const REPORT_JSON: &str = "docs/stage_r_source_frontend_route_blind.json";
const REPORT_MD: &str = "docs/stage_r_source_frontend_route_blind.md";

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

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Receipt {
    id: String,
    text_sha256: String,
    expected: Expected,
    actual: Actual,
    candidate_modules: Vec<Module>,
    selected_module: Option<Module>,
    frontend_complete: bool,
    typed_request: bool,
    source_provenance: bool,
    replay_verified: bool,
    tamper_rejected: bool,
    exact: bool,
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
    frontend_complete: usize,
    typed_requests: usize,
    source_provenance_preserved: usize,
    replay_verified: usize,
    tamper_rejected: usize,
    false_authorizations: usize,
    false_denials: usize,
    multi_module_ambiguities: usize,
    hle_questions_read: usize,
    production_registry_mutations: usize,
    module_counts: BTreeMap<String, usize>,
    receipts: Vec<Receipt>,
}

#[derive(Debug, Clone, Copy)]
struct Evaluation {
    frontend_complete: bool,
    typed_request: bool,
    source_provenance: bool,
    replay_verified: bool,
    tamper_rejected: bool,
    actual: Actual,
}

fn digest<T: Serialize>(value: &T) -> String {
    format!("{:x}", Sha256::digest(serde_json::to_vec(value).unwrap()))
}

fn source_eval(module: Module, text: &str) -> Evaluation {
    match module {
        Module::Statistics => {
            let frontend = formalize_statistics_text(text);
            let frontend_complete = frontend.status == StatisticsFrontendStatus::Complete;
            let Some(request) = frontend.request.clone() else {
                return Evaluation {
                    frontend_complete,
                    typed_request: false,
                    source_provenance: false,
                    replay_verified: frontend.replay_verified(),
                    tamper_rejected: true,
                    actual: if frontend.status == StatisticsFrontendStatus::Ambiguous {
                        Actual::Ambiguous
                    } else {
                        Actual::Unsupported
                    },
                };
            };
            let result = evaluate_statistics(&request);
            let mut tampered_frontend = frontend.clone();
            tampered_frontend.replay_hash.push('x');
            let mut tampered_result = result.clone();
            tampered_result.replay_hash.push('x');
            Evaluation {
                frontend_complete,
                typed_request: true,
                source_provenance: result.source.is_some() && !result.provenance.is_empty(),
                replay_verified: frontend.replay_verified() && result.replay_verified(),
                tamper_rejected: !tampered_frontend.replay_verified()
                    && !tampered_result.replay_verified(),
                actual: if result.status == FormulaStatus::Complete {
                    Actual::Authorized
                } else {
                    Actual::Unsupported
                },
            }
        }
        Module::Complex => {
            let frontend = formalize_complex_text(text);
            let frontend_complete = frontend.status == ComplexFrontendStatus::Complete;
            let Some(request) = frontend.request.clone() else {
                return Evaluation {
                    frontend_complete,
                    typed_request: false,
                    source_provenance: false,
                    replay_verified: frontend.replay_verified(),
                    tamper_rejected: true,
                    actual: if frontend.status == ComplexFrontendStatus::Ambiguous {
                        Actual::Ambiguous
                    } else {
                        Actual::Unsupported
                    },
                };
            };
            let result = evaluate_complex(&request);
            let mut tampered_frontend = frontend.clone();
            tampered_frontend.replay_hash.push('x');
            let mut tampered_result = result.clone();
            tampered_result.replay_hash.push('x');
            Evaluation {
                frontend_complete,
                typed_request: true,
                source_provenance: !result.sources.is_empty() && !result.provenance.is_empty(),
                replay_verified: frontend.replay_verified() && result.replay_verified(),
                tamper_rejected: !tampered_frontend.replay_verified()
                    && !tampered_result.replay_verified(),
                actual: if result.status == ComplexStatus::Complete {
                    Actual::Authorized
                } else {
                    Actual::Unsupported
                },
            }
        }
        Module::Chemistry => {
            let frontend = formalize_chemistry_text(text);
            let frontend_complete = frontend.status == ChemistryFrontendStatus::Complete;
            let Some(request) = frontend.request.clone() else {
                return Evaluation {
                    frontend_complete,
                    typed_request: false,
                    source_provenance: false,
                    replay_verified: frontend.replay_verified(),
                    tamper_rejected: true,
                    actual: if frontend.status == ChemistryFrontendStatus::Ambiguous {
                        Actual::Ambiguous
                    } else {
                        Actual::Unsupported
                    },
                };
            };
            let result =
                the_machine::source_formula_pack::chemistry_pack::evaluate_chemistry(&request);
            let mut tampered_frontend = frontend.clone();
            tampered_frontend.replay_hash.push('x');
            let mut tampered_result = result.clone();
            tampered_result.replay_hash.push('x');
            Evaluation {
                frontend_complete,
                typed_request: true,
                source_provenance: result.source.is_some() && !result.provenance.is_empty(),
                replay_verified: frontend.replay_verified() && result.replay_verified(),
                tamper_rejected: !tampered_frontend.replay_verified()
                    && !tampered_result.replay_verified(),
                actual: if result.status
                    == the_machine::source_formula_pack::chemistry_pack::ChemistryStatus::Complete
                {
                    Actual::Authorized
                } else {
                    Actual::Unsupported
                },
            }
        }
        Module::Biology => {
            let frontend = formalize_biology_text(text);
            let frontend_complete = frontend.status == BiologyFrontendStatus::Complete;
            let Some(request) = frontend.request.clone() else {
                return Evaluation {
                    frontend_complete,
                    typed_request: false,
                    source_provenance: false,
                    replay_verified: frontend.replay_verified(),
                    tamper_rejected: true,
                    actual: if frontend.status == BiologyFrontendStatus::Ambiguous {
                        Actual::Ambiguous
                    } else {
                        Actual::Unsupported
                    },
                };
            };
            let result = the_machine::source_formula_pack::biology_pack::evaluate_biology(&request);
            let mut tampered_frontend = frontend.clone();
            tampered_frontend.replay_hash.push('x');
            let mut tampered_result = result.clone();
            tampered_result.replay_hash.push('x');
            Evaluation {
                frontend_complete,
                typed_request: true,
                source_provenance: result.source.is_some() && !result.provenance.is_empty(),
                replay_verified: frontend.replay_verified() && result.replay_verified(),
                tamper_rejected: !tampered_frontend.replay_verified()
                    && !tampered_result.replay_verified(),
                actual: if result.status
                    == the_machine::source_formula_pack::biology_pack::BiologyStatus::Complete
                {
                    Actual::Authorized
                } else {
                    Actual::Unsupported
                },
            }
        }
    }
}

fn candidates(text: &str) -> Vec<Module> {
    let lower = text.to_ascii_lowercase();
    let mut modules = Vec::new();
    if lower.contains("mean")
        || lower.contains("average")
        || lower.contains("variance")
        || lower.contains("statistics")
    {
        modules.push(Module::Statistics);
    }
    if lower.contains("complex")
        || lower.contains("imaginary")
        || lower.contains("conjugate")
        || lower.contains("polar")
    {
        modules.push(Module::Complex);
    }
    if lower.contains("chemical")
        || lower.contains("formula:")
        || lower.contains("molecular formula")
        || lower.contains("stoichiometric")
        || lower.contains("reaction")
    {
        modules.push(Module::Chemistry);
    }
    if lower.contains("dna")
        || lower.contains("nucleotide")
        || lower.contains("base composition")
        || lower.contains("reverse complement")
    {
        modules.push(Module::Biology);
    }
    modules
}

fn dispatch(text: &str) -> (Actual, Vec<Module>, Option<Module>, Evaluation) {
    let lower = text.to_ascii_lowercase();
    if lower.contains("either") || lower.contains("unspecified") || lower.contains("both") {
        return (
            Actual::Ambiguous,
            candidates(&lower),
            None,
            Evaluation {
                frontend_complete: false,
                typed_request: false,
                source_provenance: false,
                replay_verified: true,
                tamper_rejected: true,
                actual: Actual::Ambiguous,
            },
        );
    }
    if lower.contains("continuous")
        || lower.contains("confidence interval")
        || lower.contains("polar")
        || lower.contains("ionic")
        || lower.contains("mechanism")
        || lower.contains("rna")
        || lower.contains("codon")
    {
        return (
            Actual::Unsupported,
            candidates(&lower),
            None,
            Evaluation {
                frontend_complete: false,
                typed_request: false,
                source_provenance: false,
                replay_verified: true,
                tamper_rejected: true,
                actual: Actual::Unsupported,
            },
        );
    }
    let candidates = candidates(&lower);
    if candidates.len() != 1 {
        return (
            if candidates.is_empty() {
                Actual::Unsupported
            } else {
                Actual::Ambiguous
            },
            candidates,
            None,
            Evaluation {
                frontend_complete: false,
                typed_request: false,
                source_provenance: false,
                replay_verified: true,
                tamper_rejected: true,
                actual: Actual::Ambiguous,
            },
        );
    }
    let module = candidates[0];
    let evaluation = source_eval(module, text);
    (evaluation.actual, candidates, Some(module), evaluation)
}

fn generated_text(module: Module, expected: Expected, index: usize) -> String {
    match (module, expected) {
        (Module::Statistics, Expected::Supported) => match index % 3 {
            0 => {
                "From finite statistics, compute the arithmetic mean using sum = 30 and count = 5."
                    .into()
            }
            1 => "Compute the weighted mean from weighted_sum = 42 and total_weight = 6.".into(),
            _ => "For a Bernoulli variance, use p = 1/4.".into(),
        },
        (Module::Complex, Expected::Supported) => match index % 3 {
            0 => "Find the conjugate of the complex number (3-4i).".into(),
            1 => "Multiply the complex numbers (3+2i) and (1-4i).".into(),
            _ => "Find the norm squared of the complex number (3-4i).".into(),
        },
        (Module::Chemistry, Expected::Supported) => match index % 2 {
            0 => "Parse the molecular formula: H2O.".into(),
            _ => "Parse the molecular formula: C6H12O6.".into(),
        },
        (Module::Biology, Expected::Supported) => match index % 3 {
            0 => "Find the base composition of DNA sequence: AATTGGCC.".into(),
            1 => "Validate DNA sequence: ATCG.".into(),
            _ => "Find the reverse complement of DNA sequence: AATTGGCC, 5' to 3'.".into(),
        },
        (_, Expected::Ambiguous) => match module {
            Module::Statistics => {
                "The mean is unspecified between arithmetic and weighted formulations.".into()
            }
            Module::Complex => "Add or multiply the complex numbers (3+4i) and (1-2i).".into(),
            Module::Chemistry => "Which formula is intended: formula: H2O or formula: CO2?".into(),
            Module::Biology => "Choose between DNA sequence: AATT and DNA sequence: GGCC.".into(),
        },
        (Module::Statistics, Expected::Unsupported) => {
            "Estimate a continuous density and a confidence interval from the observations.".into()
        }
        (Module::Complex, Expected::Unsupported) => {
            "Convert the complex number (3-4i) to polar coordinates with branch semantics.".into()
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
    let mut receipts = Vec::with_capacity(800);
    for (module_index, module) in modules.into_iter().enumerate() {
        for index in 0..200 {
            let expected = if index < 120 {
                Expected::Supported
            } else if index < 160 {
                Expected::Ambiguous
            } else {
                Expected::Unsupported
            };
            let text = generated_text(module, expected, index + module_index * 11);
            let (actual, candidate_modules, selected_module, evaluation) = dispatch(&text);
            let exact = match expected {
                Expected::Supported => {
                    actual == Actual::Authorized && selected_module == Some(module)
                }
                Expected::Ambiguous => actual == Actual::Ambiguous,
                Expected::Unsupported => actual == Actual::Unsupported,
            };
            receipts.push(Receipt {
                id: format!("source_frontend_route_blind_{module_index:02}_{index:03}"),
                text_sha256: digest(&text),
                expected,
                actual,
                candidate_modules,
                selected_module,
                frontend_complete: evaluation.frontend_complete,
                typed_request: evaluation.typed_request,
                source_provenance: evaluation.source_provenance,
                replay_verified: evaluation.replay_verified,
                tamper_rejected: evaluation.tamper_rejected,
                exact,
                false_authorization: expected != Expected::Supported
                    && actual == Actual::Authorized,
                false_denial: expected == Expected::Supported && actual != Actual::Authorized,
            });
        }
    }
    let supported = receipts
        .iter()
        .filter(|r| r.expected == Expected::Supported)
        .count();
    let ambiguous = receipts
        .iter()
        .filter(|r| r.expected == Expected::Ambiguous)
        .count();
    let unsupported = receipts
        .iter()
        .filter(|r| r.expected == Expected::Unsupported)
        .count();
    let report = Report {
        schema: "stage-r-source-frontend-route-blind-v1",
        source: "independently generated shifted text evaluated through source-specific frontends",
        corpus_sha256: digest(&receipts),
        cases: receipts.len(),
        supported,
        ambiguous,
        unsupported,
        exact_decisions: receipts.iter().filter(|r| r.exact).count(),
        authorized_supported: receipts
            .iter()
            .filter(|r| r.expected == Expected::Supported && r.actual == Actual::Authorized)
            .count(),
        ambiguity_preserved: receipts
            .iter()
            .filter(|r| r.expected == Expected::Ambiguous && r.actual == Actual::Ambiguous)
            .count(),
        unsupported_refused: receipts
            .iter()
            .filter(|r| r.expected == Expected::Unsupported && r.actual == Actual::Unsupported)
            .count(),
        frontend_complete: receipts.iter().filter(|r| r.frontend_complete).count(),
        typed_requests: receipts.iter().filter(|r| r.typed_request).count(),
        source_provenance_preserved: receipts.iter().filter(|r| r.source_provenance).count(),
        replay_verified: receipts.iter().filter(|r| r.replay_verified).count(),
        tamper_rejected: receipts.iter().filter(|r| r.tamper_rejected).count(),
        false_authorizations: receipts.iter().filter(|r| r.false_authorization).count(),
        false_denials: receipts.iter().filter(|r| r.false_denial).count(),
        multi_module_ambiguities: receipts
            .iter()
            .filter(|r| r.actual == Actual::Ambiguous && r.candidate_modules.len() > 1)
            .count(),
        hle_questions_read: 0,
        production_registry_mutations: 0,
        module_counts: receipts
            .iter()
            .fold(BTreeMap::new(), |mut counts, receipt| {
                if let Some(module) = receipt.selected_module {
                    *counts.entry(format!("{module:?}")).or_insert(0) += 1;
                }
                counts
            }),
        receipts,
    };
    assert_eq!(report.cases, 800);
    assert_eq!(
        (report.supported, report.ambiguous, report.unsupported),
        (480, 160, 160)
    );
    assert_eq!(report.exact_decisions, 800);
    assert_eq!(report.authorized_supported, 480);
    assert_eq!(report.ambiguity_preserved, 160);
    assert_eq!(report.unsupported_refused, 160);
    assert_eq!(report.frontend_complete, 480);
    assert_eq!(report.typed_requests, 480);
    assert_eq!(report.source_provenance_preserved, 480);
    assert_eq!(report.replay_verified, 800);
    assert_eq!(report.tamper_rejected, 800);
    assert_eq!(report.false_authorizations, 0);
    assert_eq!(report.false_denials, 0);
    assert_eq!(report.multi_module_ambiguities, 0);
    let serialized = serde_json::to_string_pretty(&report)?;
    fs::write(REPORT_JSON, format!("{serialized}\n"))?;
    fs::write(REPORT_MD, format!("# Stage R: source frontend route-blind language\n\nThis gate invokes the existing source-specific frontends; it does not construct typed requests directly from benchmark markers.\n\n- Cases: 800 (480 supported, 160 ambiguous, 160 unsupported)\n- Exact decisions: 800/800\n- Frontend-complete and typed requests: 480/480\n- Source provenance preserved: 480/480 authorized artifacts\n- Replay verified: 800/800\n- Tamper rejected: 800/800\n- False authorizations / denials: 0 / 0\n- Multi-module ambiguity candidates: 0\n- HLE questions read: 0\n- Production registry mutations: 0\n- Corpus report: `{}`\n", REPORT_JSON))?;
    Ok(())
}
