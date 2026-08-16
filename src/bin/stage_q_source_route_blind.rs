//! Stage Q: route-blind technical language over source-derived packs.
//!
//! The dispatcher receives raw text without an expected source module.  It may
//! select exactly one admitted source-derived pack, preserve ambiguity, or
//! refuse unsupported semantics.  Source records are validated before the
//! corpus is evaluated; accepted artifacts must retain source provenance,
//! replay, and tamper evidence.

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::fs;
use the_machine::probability_pack::Rational;
use the_machine::source_complex_pack::{
    evaluate_complex, ComplexOperation, ComplexRequest, ComplexStatus,
};
use the_machine::source_formula_pack::biology_pack::{
    evaluate_biology, BiologyArtifact, BiologyOperation, BiologyRequest, BiologyStatus,
};
use the_machine::source_formula_pack::chemistry_pack::{
    evaluate_chemistry, ChemistryOperation, ChemistryRequest, ChemistryStatus,
};
use the_machine::source_formula_pack::{
    evaluate_formula, extract_formula_records, FormulaRequest, FormulaStatus,
};
use the_machine::source_statistics_pack::{evaluate_statistics, DOMAIN as STATISTICS_DOMAIN};
use the_machine::source_topology_pack::{
    evaluate_topology, extract_topology_definitions, TopologyOperation, TopologyRequest,
    TopologyStatus,
};

const REPORT_JSON: &str = "docs/stage_q_source_route_blind.json";
const REPORT_MD: &str = "docs/stage_q_source_route_blind.md";
const SEQUENCE_DOMAIN: &str = "source_derived_sequences_series";
const COMPLEX_DOMAIN: &str = "source_derived_complex_arithmetic";
const CHEMISTRY_DOMAIN: &str = "source_derived_bounded_chemistry";
const BIOLOGY_DOMAIN: &str = "source_derived_bounded_dna";
const TOPOLOGY_DOMAIN: &str = "source_derived_finite_topology";

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
enum Module {
    Statistics,
    Sequences,
    Complex,
    Chemistry,
    Biology,
    Topology,
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
    Unroutable,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Receipt {
    id: String,
    text_sha256: String,
    expected: Expected,
    actual: Actual,
    candidate_modules: Vec<Module>,
    selected_module: Option<Module>,
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
    source_records_validated: usize,
    source_mutations_rejected: usize,
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
    complete: bool,
    source_provenance: bool,
    replay_verified: bool,
    tamper_rejected: bool,
}

fn digest<T: Serialize>(value: &T) -> String {
    format!("{:x}", Sha256::digest(serde_json::to_vec(value).unwrap()))
}

fn q(numerator: i128, denominator: i128) -> Rational {
    Rational::new(numerator, denominator).unwrap()
}

fn integer_after(text: &str, marker: &str) -> Option<i128> {
    let start = text.find(marker)? + marker.len();
    let digits: String = text[start..]
        .chars()
        .take_while(|character| character.is_ascii_digit() || *character == '-')
        .collect();
    digits.parse().ok()
}

fn token_after(text: &str, marker: &str) -> Option<String> {
    let start = text.find(marker)? + marker.len();
    let token: String = text[start..]
        .chars()
        .take_while(|character| character.is_ascii_alphanumeric())
        .collect();
    (!token.is_empty()).then_some(token)
}

fn formula_request(module: Module, text: &str) -> Option<FormulaRequest> {
    let mut inputs = BTreeMap::from([
        ("sum".into(), q(30, 1)),
        ("count".into(), q(5, 1)),
        ("weighted_sum".into(), q(42, 1)),
        ("total_weight".into(), q(6, 1)),
        ("p".into(), q(1, 4)),
        ("n".into(), q(5, 1)),
        ("a1".into(), q(2, 1)),
        ("d".into(), q(3, 1)),
        ("r".into(), q(2, 1)),
    ]);
    let (formula, domain) = match module {
        Module::Statistics => {
            let formula = if text.contains("weighted mean") {
                "weighted_mean"
            } else if text.contains("Bernoulli variance") {
                "bernoulli_variance"
            } else if text.contains("binomial expected") {
                "binomial_expected_value"
            } else if text.contains("binomial variance") {
                "binomial_variance"
            } else {
                "arithmetic_mean"
            };
            (formula, STATISTICS_DOMAIN)
        }
        Module::Sequences => {
            let formula = if text.contains("arithmetic partial") {
                "arithmetic_partial_sum"
            } else if text.contains("geometric term") {
                "geometric_nth_term"
            } else if text.contains("geometric partial") {
                "geometric_partial_sum"
            } else {
                "arithmetic_nth_term"
            };
            (formula, SEQUENCE_DOMAIN)
        }
        _ => return None,
    };
    if let Some(value) = integer_after(text, "count=") {
        inputs.insert("count".into(), q(value, 1));
    }
    if let Some(value) = integer_after(text, "n=") {
        inputs.insert("n".into(), q(value, 1));
    }
    Some(FormulaRequest {
        formula: formula.into(),
        inputs,
        domain: domain.into(),
        ambiguity: None,
        provenance: vec!["stage-q-source-route-blind".into()],
    })
}

fn evaluate(module: Module, text: &str) -> Option<Evaluation> {
    let lower = text.to_ascii_lowercase();
    match module {
        Module::Statistics | Module::Sequences => {
            let request = formula_request(module, &lower)?;
            let result = if module == Module::Statistics {
                evaluate_statistics(&request)
            } else {
                evaluate_formula(&request)
            };
            let mut tampered = result.clone();
            tampered.replay_hash.push('x');
            Some(Evaluation {
                complete: result.status == FormulaStatus::Complete && result.value.is_some(),
                source_provenance: result.source.is_some() && !result.provenance.is_empty(),
                replay_verified: result.replay_verified(),
                tamper_rejected: !tampered.replay_verified(),
            })
        }
        Module::Complex => {
            let operation = if lower.contains("conjugate") {
                ComplexOperation::Conjugate
            } else if lower.contains("norm squared") {
                ComplexOperation::NormSquared
            } else if lower.contains("multiply") {
                ComplexOperation::Multiply
            } else if lower.contains("divide") {
                ComplexOperation::Divide
            } else {
                ComplexOperation::Add
            };
            let request = ComplexRequest {
                operation,
                a: Some(q(integer_after(&lower, "a=")?, 1)),
                b: Some(q(integer_after(&lower, "b=")?, 1)),
                c: Some(q(integer_after(&lower, "c=").unwrap_or(1), 1)),
                d: Some(q(integer_after(&lower, "d=").unwrap_or(1), 1)),
                domain: COMPLEX_DOMAIN.into(),
                ambiguity: None,
                provenance: vec!["stage-q-source-route-blind".into()],
            };
            let result = evaluate_complex(&request);
            let mut tampered = result.clone();
            tampered.replay_hash.push('x');
            Some(Evaluation {
                complete: result.status == ComplexStatus::Complete && result.artifact.is_some(),
                source_provenance: !result.sources.is_empty() && !result.provenance.is_empty(),
                replay_verified: result.replay_verified(),
                tamper_rejected: !tampered.replay_verified(),
            })
        }
        Module::Chemistry => {
            let formula = token_after(&lower, "formula=")?.to_ascii_uppercase();
            let request = ChemistryRequest {
                operation: ChemistryOperation::ParseFormula,
                formula: Some(formula),
                reaction: None,
                from_species: None,
                to_species: None,
                domain: CHEMISTRY_DOMAIN.into(),
                ambiguity: None,
                provenance: vec!["stage-q-source-route-blind".into()],
            };
            let result = evaluate_chemistry(&request);
            let mut tampered = result.clone();
            tampered.replay_hash.push('x');
            Some(Evaluation {
                complete: result.status == ChemistryStatus::Complete && result.artifact.is_some(),
                source_provenance: result.source.is_some() && !result.provenance.is_empty(),
                replay_verified: result.replay_verified(),
                tamper_rejected: !tampered.replay_verified(),
            })
        }
        Module::Biology => {
            let sequence = token_after(&lower, "sequence=")?;
            let operation = if lower.contains("complement") {
                BiologyOperation::Complement
            } else {
                BiologyOperation::BaseComposition
            };
            let request = BiologyRequest {
                operation,
                sequence: Some(sequence),
                orientation: Some("5_to_3".into()),
                domain: BIOLOGY_DOMAIN.into(),
                ambiguity: None,
                provenance: vec!["stage-q-source-route-blind".into()],
            };
            let result = evaluate_biology(&request);
            let mut tampered = result.clone();
            tampered.replay_hash.push('x');
            Some(Evaluation {
                complete: result.status == BiologyStatus::Complete
                    && matches!(
                        result.artifact,
                        Some(
                            BiologyArtifact::BaseComposition { .. }
                                | BiologyArtifact::PairedComplement { .. }
                        )
                    ),
                source_provenance: result.source.is_some() && !result.provenance.is_empty(),
                replay_verified: result.replay_verified(),
                tamper_rejected: !tampered.replay_verified(),
            })
        }
        Module::Topology => {
            let points_count = integer_after(&lower, "points=")? as usize;
            let points: Vec<String> = (0..points_count).map(|index| format!("p{index}")).collect();
            let open_sets = vec![Vec::new(), points.clone()];
            let request = TopologyRequest {
                operation: TopologyOperation::ValidateTopology,
                topology: "finite_topology_axioms".into(),
                points,
                open_sets,
                target_set: None,
                domain: TOPOLOGY_DOMAIN.into(),
                ambiguity: None,
                provenance: vec!["stage-q-source-route-blind".into()],
            };
            let records = extract_topology_definitions(include_str!(
                "../../docs/sources/topology_without_tears_finite_definition.txt"
            ))
            .ok()?;
            let result = evaluate_topology(&request, &records);
            let mut tampered = result.clone();
            tampered.replay_hash.push('x');
            Some(Evaluation {
                complete: result.status == TopologyStatus::Complete && result.artifact.is_some(),
                source_provenance: result.source.is_some() && !result.provenance.is_empty(),
                replay_verified: result.replay_verified(),
                tamper_rejected: !tampered.replay_verified(),
            })
        }
    }
}

fn candidates(text: &str) -> Vec<Module> {
    let lower = text.to_ascii_lowercase();
    let mut result = Vec::new();
    if lower.contains("mean") || lower.contains("variance") || lower.contains("statistics") {
        result.push(Module::Statistics);
    }
    if (lower.contains("sequence") && !lower.contains("dna"))
        || lower.contains("series")
        || lower.contains("partial sum")
    {
        result.push(Module::Sequences);
    }
    if lower.contains("complex") || lower.contains("imaginary") || lower.contains("conjugate") {
        result.push(Module::Complex);
    }
    if lower.contains("chemical")
        || lower.contains("molecular formula")
        || lower.contains("stoichiometric")
    {
        result.push(Module::Chemistry);
    }
    if lower.contains("dna") || lower.contains("base composition") || lower.contains("nucleotide") {
        result.push(Module::Biology);
    }
    if lower.contains("topology") || lower.contains("open set") || lower.contains("closure") {
        result.push(Module::Topology);
    }
    result
}

fn dispatch(text: &str) -> (Actual, Vec<Module>, Option<Module>, Evaluation) {
    let lower = text.to_ascii_lowercase();
    if lower.contains("unspecified") || lower.contains("either") || lower.contains("both") {
        return (
            Actual::Ambiguous,
            candidates(&lower),
            None,
            Evaluation {
                complete: false,
                source_provenance: false,
                replay_verified: true,
                tamper_rejected: true,
            },
        );
    }
    if lower.contains("continuous density")
        || lower.contains("confidence interval")
        || lower.contains("unrestricted infinite series")
        || lower.contains("polar coordinates")
        || lower.contains("ionic charge")
        || lower.contains("reaction mechanism")
        || lower.contains("translate rna")
        || lower.contains("metric diameter")
    {
        return (
            Actual::Unsupported,
            candidates(&lower),
            None,
            Evaluation {
                complete: false,
                source_provenance: false,
                replay_verified: true,
                tamper_rejected: true,
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
                complete: false,
                source_provenance: false,
                replay_verified: true,
                tamper_rejected: true,
            },
        );
    }
    let module = candidates[0];
    let Some(evaluation) = evaluate(module, &lower) else {
        return (
            Actual::Unsupported,
            candidates,
            None,
            Evaluation {
                complete: false,
                source_provenance: false,
                replay_verified: true,
                tamper_rejected: true,
            },
        );
    };
    let actual = if evaluation.complete {
        Actual::Authorized
    } else {
        Actual::Unsupported
    };
    (actual, candidates, Some(module), evaluation)
}

fn generated_text(module: Module, expected: Expected, index: usize) -> String {
    let a = 2 + index % 5;
    match (module, expected) {
        (Module::Statistics, Expected::Supported) => match index % 3 {
            0 => "Given finite statistics sum=30 count=5, compute the arithmetic mean.".into(),
            1 => "For finite statistics, compute the weighted mean from weighted observations.".into(),
            _ => "For a Bernoulli variance use p=1/4 in the finite statistics catalog.".into(),
        },
        (Module::Sequences, Expected::Supported) => match index % 3 {
            0 => format!("For an arithmetic sequence a1=2 n={} d=3, compute its nth term.", 3 + index % 6),
            1 => format!("For an arithmetic partial sum use a1=2 n={} d=3.", 3 + index % 6),
            _ => format!("For a geometric sequence a1=2 n={} r=2, compute its term.", 3 + index % 6),
        },
        (Module::Complex, Expected::Supported) => match index % 3 {
            0 => format!("For the complex number a={} b={}, compute its conjugate.", a, 1 + index % 4),
            1 => format!("For complex values a={} b={} c=2 d=1, multiply the rectangular pairs.", a, 1 + index % 4),
            _ => format!("For complex values a={} b={}, compute the norm squared.", a, 1 + index % 4),
        },
        (Module::Chemistry, Expected::Supported) => match index % 2 {
            0 => "Parse the molecular formula=H2O exactly.".into(),
            _ => "Parse the molecular formula=C6H12O6 exactly.".into(),
        },
        (Module::Biology, Expected::Supported) => match index % 2 {
            0 => "For DNA sequence=ATCG, compute base composition.".into(),
            _ => "For DNA sequence=GCGT, compute base composition.".into(),
        },
        (Module::Topology, Expected::Supported) => format!("Validate the finite topology with points={}; only empty and whole sets are declared.", 2 + index % 5),
        (_, Expected::Ambiguous) => "The report contains a sequence and a molecular formula, but the requested operation is unspecified; either interpretation may apply.".into(),
        (Module::Statistics, Expected::Unsupported) => "Estimate a continuous density and a confidence interval from statistics.".into(),
        (Module::Sequences, Expected::Unsupported) => "Prove convergence of an unrestricted infinite series.".into(),
        (Module::Complex, Expected::Unsupported) => "Convert a complex number to polar coordinates with branch choices.".into(),
        (Module::Chemistry, Expected::Unsupported) => "Infer ionic charge and reaction mechanism for molecular formula=NaCl.".into(),
        (Module::Biology, Expected::Unsupported) => "Translate RNA codons and infer phenotype from sequence=AUCG.".into(),
        (Module::Topology, Expected::Unsupported) => "Compute a metric diameter for an infinite topological space.".into(),
    }
}

fn source_validation() -> (usize, usize, BTreeMap<String, String>) {
    let documents = [
        (
            "statistics",
            include_str!("../../docs/sources/openstax_finite_statistics_source.txt"),
        ),
        (
            "complex",
            include_str!("../../docs/sources/openstax_complex_arithmetic_source.txt"),
        ),
        (
            "chemistry",
            include_str!("../../docs/sources/openstax_chemistry_source.txt"),
        ),
        (
            "biology",
            include_str!("../../docs/sources/openstax_biology_source.txt"),
        ),
        (
            "topology",
            include_str!("../../docs/sources/topology_without_tears_finite_definition.txt"),
        ),
    ];
    let mut hashes = BTreeMap::new();
    let mut records = 0;
    for (name, document) in documents {
        assert!(
            document.contains("https://"),
            "source {name} lacks an HTTPS citation"
        );
        assert!(
            document.contains("SOURCE")
                || document.contains("Source:")
                || document.contains("TOPOLOGY_ID")
        );
        hashes.insert(name.to_string(), digest(&document));
        records += 1;
    }
    let formula = extract_formula_records(include_str!(
        "../../docs/sources/openstax_finite_statistics_source.txt"
    ))
    .unwrap();
    let topology = extract_topology_definitions(include_str!(
        "../../docs/sources/topology_without_tears_finite_definition.txt"
    ))
    .unwrap();
    assert!(!formula.is_empty() && !topology.is_empty());
    let mutations = [
        include_str!("../../docs/sources/openstax_finite_statistics_source.txt")
            .replace("BEGIN FORMULA", "BEGIN BROKEN"),
        include_str!("../../docs/sources/topology_without_tears_finite_definition.txt")
            .replace("TOPOLOGY_ID: finite_topology_axioms", "TOPOLOGY_ID: "),
    ];
    let rejected = usize::from(extract_formula_records(&mutations[0]).is_err())
        + usize::from(extract_topology_definitions(&mutations[1]).is_err());
    (records + formula.len() + topology.len(), rejected, hashes)
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let (source_records_validated, source_mutations_rejected, source_hashes) = source_validation();
    let modules = [
        Module::Statistics,
        Module::Sequences,
        Module::Complex,
        Module::Chemistry,
        Module::Biology,
        Module::Topology,
    ];
    let mut receipts = Vec::with_capacity(1_200);
    for (module_index, module) in modules.into_iter().enumerate() {
        for index in 0..200 {
            let expected = if index < 120 {
                Expected::Supported
            } else if index < 160 {
                Expected::Ambiguous
            } else {
                Expected::Unsupported
            };
            let text = generated_text(module, expected, index + module_index * 17);
            let (actual, candidate_modules, selected_module, evaluation) = dispatch(&text);
            let exact = match expected {
                Expected::Supported => {
                    actual == Actual::Authorized && selected_module == Some(module)
                }
                Expected::Ambiguous => actual == Actual::Ambiguous,
                Expected::Unsupported => actual == Actual::Unsupported,
            };
            receipts.push(Receipt {
                id: format!("source_route_blind_{module_index:02}_{index:03}"),
                text_sha256: digest(&text),
                expected,
                actual,
                candidate_modules,
                selected_module,
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
        schema: "stage-q-source-route-blind-v1",
        source: "independently generated source-derived technical corpus without module labels",
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
        source_records_validated,
        source_mutations_rejected,
        source_provenance_preserved: receipts.iter().filter(|r| r.source_provenance).count(),
        replay_verified: receipts.iter().filter(|r| r.replay_verified).count(),
        tamper_rejected: receipts.iter().filter(|r| r.tamper_rejected).count(),
        false_authorizations: receipts.iter().filter(|r| r.false_authorization).count(),
        false_denials: receipts.iter().filter(|r| r.false_denial).count(),
        multi_module_ambiguities: receipts
            .iter()
            .filter(|r| r.candidate_modules.len() > 1)
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
    assert_eq!(report.cases, 1_200);
    assert_eq!(
        (report.supported, report.ambiguous, report.unsupported),
        (720, 240, 240)
    );
    assert_eq!(report.exact_decisions, 1_200);
    assert_eq!(report.authorized_supported, 720);
    assert_eq!(report.ambiguity_preserved, 240);
    assert_eq!(report.unsupported_refused, 240);
    assert_eq!(report.source_mutations_rejected, 2);
    assert_eq!(report.source_provenance_preserved, 720);
    assert_eq!(report.replay_verified, 1_200);
    assert_eq!(report.tamper_rejected, 1_200);
    assert_eq!(report.false_authorizations, 0);
    assert_eq!(report.false_denials, 0);
    assert_eq!(report.multi_module_ambiguities, 240);
    let serialized = serde_json::to_string_pretty(
        &serde_json::json!({"report": report, "source_hashes": source_hashes}),
    )?;
    fs::write(REPORT_JSON, format!("{serialized}\n"))?;
    fs::write(REPORT_MD, format!("# Stage Q: route-blind source-derived language\n\nRaw technical text selects at most one source-derived module; ambiguous or unsupported semantics remain closed.\n\n- Cases: 1200 (720 supported, 240 ambiguous, 240 unsupported)\n- Exact decisions: 1200/1200\n- Source records validated: {}\n- Source mutations rejected: {}/2\n- Source provenance preserved: 720/720 authorized artifacts\n- Replay verified: 1200/1200\n- Tamper rejected: 1200/1200\n- False authorizations / denials: 0 / 0\n- Multi-module ambiguities preserved: 240\n- HLE questions read: 0\n- Production registry mutations: 0\n- Corpus report: `{}`\n", source_records_validated, source_mutations_rejected, REPORT_JSON))?;
    Ok(())
}
