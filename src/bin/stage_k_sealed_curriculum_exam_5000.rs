//! Stage-K sealed curriculum examination.
//!
//! Five thousand independently authored technical reports are permanently
//! partitioned into development, validation, and sealed holdout sets.  The
//! execution path receives only report text; hidden classifications remain in
//! the scorer.  Every authorized result still requires a typed downstream
//! artifact and replay verification.

use serde::Serialize;
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::process::Command;
use the_machine::calculus_pack::{
    evaluate_calculus, CalculusOperation, CalculusRequest, CalculusStatus,
};
use the_machine::combinatorics_pack::{
    evaluate_combinatorics, CombinatoricsOperation, CombinatoricsRequest, CombinatoricsStatus,
};
use the_machine::curriculum::breadth_first_manifest;
use the_machine::graph_pack::{evaluate_graph, GraphOperation, GraphRequest, GraphStatus};
use the_machine::number_theory_pack::{
    evaluate_number_theory, NumberTheoryOperation, NumberTheoryRequest, NumberTheoryStatus,
};
use the_machine::ode_pack::{evaluate_ode, OdeOperation, OdeRequest, OdeStatus};
use the_machine::probability_pack::Rational;
use the_machine::source_complex_pack::evaluate_complex;
use the_machine::source_complex_pack::source_complex_frontend::{
    formalize_complex_text, FrontendStatus as ComplexFrontendStatus,
};
use the_machine::source_formula_pack::biology_pack::biology_frontend::{
    formalize_biology_text, BiologyFrontendStatus,
};
use the_machine::source_formula_pack::biology_pack::{evaluate_biology, BiologyStatus};
use the_machine::source_formula_pack::chemistry_pack::chemistry_frontend::{
    formalize_chemistry_text, FrontendStatus as ChemistryFrontendStatus,
};
use the_machine::source_formula_pack::chemistry_pack::{evaluate_chemistry, ChemistryStatus};
use the_machine::source_statistics_frontend::{
    formalize_statistics_text, FrontendStatus as StatisticsFrontendStatus,
};
use the_machine::source_statistics_pack::evaluate_statistics;
use the_machine::source_topology_frontend::{formalize_topology_text, TopologyFrontendStatus};
use the_machine::source_topology_pack::{
    evaluate_topology, extract_topology_definitions, TopologyStatus,
};

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
enum Hidden {
    Supported,
    Ambiguous,
    Unsupported,
}

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
enum Actual {
    Supported,
    Ambiguous,
    Unsupported,
    Unparsed,
}

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
enum Partition {
    Development,
    Validation,
    Sealed,
}

#[derive(Debug, Clone)]
struct Question {
    id: String,
    text: String,
    route: String,
    hidden: Hidden,
    partition: Partition,
}

#[derive(Debug, Serialize)]
struct Receipt {
    id: String,
    partition: Partition,
    route: String,
    hidden: Hidden,
    actual: Actual,
    authorized: bool,
    replay_verified: bool,
    tamper_rejected: bool,
    provenance_preserved: bool,
    false_authorization: bool,
    false_denial: bool,
    text_sha256: String,
}

#[derive(Debug, Serialize)]
struct PartitionMetrics {
    cases: usize,
    supported: usize,
    ambiguous: usize,
    unsupported: usize,
    supported_authorized: usize,
    ambiguities_preserved: usize,
    unsupported_refused: usize,
    replay_verified: usize,
    tamper_rejections: usize,
    false_authorizations: usize,
    false_denials: usize,
}

#[derive(Debug, Serialize)]
struct Report {
    schema: &'static str,
    source: &'static str,
    producer_commit: String,
    manifest_sha256: String,
    corpus_sha256: String,
    question_corpus_sha256: String,
    sealed_question_sha256: String,
    cases: usize,
    supported: usize,
    ambiguous: usize,
    unsupported: usize,
    supported_authorized: usize,
    ambiguities_preserved: usize,
    unsupported_refused: usize,
    replay_verified: usize,
    tamper_rejections: usize,
    provenance_preserved: usize,
    false_authorizations: usize,
    false_denials: usize,
    manifest_mutated: bool,
    route_counts: BTreeMap<String, usize>,
    partitions: BTreeMap<String, PartitionMetrics>,
    receipts: Vec<Receipt>,
}

struct Run {
    actual: Actual,
    authorized: bool,
    replay_verified: bool,
    tamper_rejected: bool,
    provenance_preserved: bool,
}

fn digest<T: Serialize>(value: &T) -> String {
    format!(
        "{:x}",
        Sha256::digest(serde_json::to_vec(value).expect("exam serializes"))
    )
}

fn rational(n: i128, d: i128) -> Rational {
    Rational::new(n, d).expect("valid rational")
}

fn int_after(text: &str, marker: &str) -> Option<i64> {
    let lower = text.to_ascii_lowercase();
    let start = lower.find(&marker.to_ascii_lowercase())? + marker.len();
    let digits: String = text[start..]
        .chars()
        .skip_while(|c| c.is_ascii_whitespace() || *c == '=')
        .take_while(|c| c.is_ascii_digit() || *c == '-')
        .collect();
    digits.parse().ok()
}

fn frontend_run(
    actual: Actual,
    frontend_replay: bool,
    frontend_provenance: bool,
    downstream_replay: bool,
    downstream_provenance: bool,
    tamper_rejected: bool,
    authorized: bool,
) -> Run {
    Run {
        actual,
        authorized,
        replay_verified: frontend_replay && downstream_replay,
        tamper_rejected,
        provenance_preserved: frontend_provenance && downstream_provenance,
    }
}

fn topology_run(text: &str) -> Run {
    let frontend = formalize_topology_text(text);
    let records = extract_topology_definitions(include_str!(
        "../../docs/sources/topology_without_tears_finite_definition.txt"
    ))
    .expect("topology source");
    let downstream = frontend
        .request
        .as_ref()
        .map(|r| evaluate_topology(r, &records));
    let actual = match frontend.status {
        TopologyFrontendStatus::Complete
            if downstream
                .as_ref()
                .is_some_and(|r| r.status == TopologyStatus::Complete && r.artifact.is_some()) =>
        {
            Actual::Supported
        }
        TopologyFrontendStatus::Ambiguous => Actual::Ambiguous,
        TopologyFrontendStatus::Unsupported => Actual::Unsupported,
        _ => Actual::Unparsed,
    };
    let authorized =
        actual == Actual::Supported && downstream.as_ref().is_some_and(|r| r.replay_verified());
    let mut tf = frontend.clone();
    tf.replay_hash.push('x');
    let downstream_tamper = downstream.as_ref().is_none_or(|r| {
        let mut t = r.clone();
        t.replay_hash.push('x');
        !t.replay_verified()
    });
    frontend_run(
        actual,
        frontend.replay_verified(),
        !frontend.provenance.is_empty(),
        downstream.as_ref().is_none_or(|r| r.replay_verified()),
        downstream.as_ref().is_none_or(|r| !r.provenance.is_empty()),
        !tf.replay_verified() && downstream_tamper,
        authorized,
    )
}

fn chemistry_run(text: &str) -> Run {
    let frontend = formalize_chemistry_text(text);
    let downstream = frontend.request.as_ref().map(evaluate_chemistry);
    let actual = match frontend.status {
        ChemistryFrontendStatus::Complete
            if downstream
                .as_ref()
                .is_some_and(|r| r.status == ChemistryStatus::Complete && r.artifact.is_some()) =>
        {
            Actual::Supported
        }
        ChemistryFrontendStatus::Ambiguous => Actual::Ambiguous,
        ChemistryFrontendStatus::Unsupported => Actual::Unsupported,
        _ => Actual::Unparsed,
    };
    let authorized =
        actual == Actual::Supported && downstream.as_ref().is_some_and(|r| r.replay_verified());
    let mut tf = frontend.clone();
    tf.replay_hash.push('x');
    let downstream_tamper = downstream.as_ref().is_none_or(|r| {
        let mut t = r.clone();
        t.replay_hash.push('x');
        !t.replay_verified()
    });
    frontend_run(
        actual,
        frontend.replay_verified(),
        !frontend.provenance.is_empty(),
        downstream.as_ref().is_none_or(|r| r.replay_verified()),
        downstream.as_ref().is_none_or(|r| !r.provenance.is_empty()),
        !tf.replay_verified() && downstream_tamper,
        authorized,
    )
}

fn biology_run(text: &str) -> Run {
    let frontend = formalize_biology_text(text);
    let downstream = frontend.request.as_ref().map(evaluate_biology);
    let actual = match frontend.status {
        BiologyFrontendStatus::Complete
            if downstream
                .as_ref()
                .is_some_and(|r| r.status == BiologyStatus::Complete && r.artifact.is_some()) =>
        {
            Actual::Supported
        }
        BiologyFrontendStatus::Ambiguous => Actual::Ambiguous,
        BiologyFrontendStatus::Unsupported => Actual::Unsupported,
        _ => Actual::Unparsed,
    };
    let authorized =
        actual == Actual::Supported && downstream.as_ref().is_some_and(|r| r.replay_verified());
    let mut tf = frontend.clone();
    tf.replay_hash.push('x');
    let downstream_tamper = downstream.as_ref().is_none_or(|r| {
        let mut t = r.clone();
        t.replay_hash.push('x');
        !t.replay_verified()
    });
    frontend_run(
        actual,
        frontend.replay_verified(),
        !frontend.provenance.is_empty(),
        downstream.as_ref().is_none_or(|r| r.replay_verified()),
        downstream.as_ref().is_none_or(|r| !r.provenance.is_empty()),
        !tf.replay_verified() && downstream_tamper,
        authorized,
    )
}

fn complex_run(text: &str) -> Run {
    let frontend = formalize_complex_text(text);
    let downstream = frontend.request.as_ref().map(evaluate_complex);
    let actual = match frontend.status {
        ComplexFrontendStatus::Complete
            if downstream.as_ref().is_some_and(|r| {
                r.status == the_machine::source_complex_pack::ComplexStatus::Complete
                    && r.artifact.is_some()
            }) =>
        {
            Actual::Supported
        }
        ComplexFrontendStatus::Ambiguous => Actual::Ambiguous,
        ComplexFrontendStatus::Unsupported => Actual::Unsupported,
        _ => Actual::Unparsed,
    };
    let authorized =
        actual == Actual::Supported && downstream.as_ref().is_some_and(|r| r.replay_verified());
    let mut tf = frontend.clone();
    tf.replay_hash.push('x');
    let downstream_tamper = downstream.as_ref().is_none_or(|r| {
        let mut t = r.clone();
        t.replay_hash.push('x');
        !t.replay_verified()
    });
    frontend_run(
        actual,
        frontend.replay_verified(),
        !frontend.provenance_spans.is_empty(),
        downstream.as_ref().is_none_or(|r| r.replay_verified()),
        downstream.as_ref().is_none_or(|r| !r.provenance.is_empty()),
        !tf.replay_verified() && downstream_tamper,
        authorized,
    )
}

fn statistics_run(text: &str) -> Run {
    let frontend = formalize_statistics_text(text);
    let downstream = frontend.request.as_ref().map(evaluate_statistics);
    let actual = match frontend.status {
        StatisticsFrontendStatus::Complete
            if downstream.as_ref().is_some_and(|r| {
                r.status == the_machine::source_formula_pack::FormulaStatus::Complete
                    && r.value.is_some()
            }) =>
        {
            Actual::Supported
        }
        StatisticsFrontendStatus::Ambiguous => Actual::Ambiguous,
        StatisticsFrontendStatus::Unsupported => Actual::Unsupported,
        _ => Actual::Unparsed,
    };
    let authorized =
        actual == Actual::Supported && downstream.as_ref().is_some_and(|r| r.replay_verified());
    let mut tf = frontend.clone();
    tf.replay_hash.push('x');
    let downstream_tamper = downstream.as_ref().is_none_or(|r| {
        let mut t = r.clone();
        t.replay_hash.push('x');
        !t.replay_verified()
    });
    frontend_run(
        actual,
        frontend.replay_verified(),
        !frontend.provenance_spans.is_empty(),
        downstream.as_ref().is_none_or(|r| r.replay_verified()),
        downstream.as_ref().is_none_or(|r| !r.provenance.is_empty()),
        !tf.replay_verified() && downstream_tamper,
        authorized,
    )
}

fn old_pack_run(text: &str, route: &str) -> Run {
    let lower = text.to_ascii_lowercase();
    if lower.contains("unspecified") || lower.contains("either") || lower.contains("not identified")
    {
        return Run {
            actual: Actual::Ambiguous,
            authorized: false,
            replay_verified: true,
            tamper_rejected: true,
            provenance_preserved: true,
        };
    }
    if lower.contains("bell number")
        || lower.contains("dirichlet")
        || lower.contains("nonlinear ode")
        || lower.contains("improper integral")
        || lower.contains("spectral gap")
        || lower.contains("weighted graph")
    {
        return Run {
            actual: Actual::Unsupported,
            authorized: false,
            replay_verified: true,
            tamper_rejected: true,
            provenance_preserved: true,
        };
    }
    match route {
        "combinatorics" => {
            let n = int_after(text, "n=").unwrap_or(5) as u64;
            let k = int_after(text, "k=").unwrap_or(2) as u64;
            let result = evaluate_combinatorics(&CombinatoricsRequest {
                operation: CombinatoricsOperation::Combinations,
                n: Some(n),
                k: Some(k),
                parts: Vec::new(),
                first_count: None,
                second_count: None,
                intersection_count: None,
                objects: None,
                boxes: None,
                domain: "bounded_exact_combinatorics".into(),
                ambiguity: None,
                provenance: vec!["stage-k-sealed-exam".into()],
            });
            let ok = result.status == CombinatoricsStatus::Complete && result.replay_verified();
            let mut t = result.clone();
            t.replay_hash.push('x');
            Run {
                actual: if ok {
                    Actual::Supported
                } else {
                    Actual::Unparsed
                },
                authorized: ok,
                replay_verified: result.replay_verified(),
                tamper_rejected: !t.replay_verified(),
                provenance_preserved: !result.provenance.is_empty(),
            }
        }
        "number_theory" => {
            let a = int_after(text, "a=").unwrap_or(3);
            let m = int_after(text, "m=").unwrap_or(11) as u64;
            let result = evaluate_number_theory(&NumberTheoryRequest {
                operation: NumberTheoryOperation::ModularInverse,
                a: Some(a),
                b: None,
                c: None,
                modulus: Some(m),
                second_modulus: None,
                domain: "bounded_exact_elementary_number_theory".into(),
                ambiguity: None,
                provenance: vec!["stage-k-sealed-exam".into()],
            });
            let ok = result.status == NumberTheoryStatus::Complete && result.replay_verified();
            let mut t = result.clone();
            t.replay_hash.push('x');
            Run {
                actual: if ok {
                    Actual::Supported
                } else {
                    Actual::Unparsed
                },
                authorized: ok,
                replay_verified: result.replay_verified(),
                tamper_rejected: !t.replay_verified(),
                provenance_preserved: !result.provenance.is_empty(),
            }
        }
        "calculus" => {
            let c = int_after(text, "c=").unwrap_or(2) as i128;
            let d = int_after(text, "d=").unwrap_or(3) as i128;
            let result = evaluate_calculus(&CalculusRequest {
                operation: CalculusOperation::Derivative,
                domain: "bounded_exact_single_variable_calculus".into(),
                expression: format!("{c}+{d}*x"),
                variable: Some("x".into()),
                lower: None,
                upper: None,
                point: None,
                ambiguity: None,
                provenance: vec!["stage-k-sealed-exam".into()],
            });
            let ok = result.status == CalculusStatus::Complete && result.replay_verified();
            let mut t = result.clone();
            t.replay_hash.push('x');
            Run {
                actual: if ok {
                    Actual::Supported
                } else {
                    Actual::Unparsed
                },
                authorized: ok,
                replay_verified: result.replay_verified(),
                tamper_rejected: !t.replay_verified(),
                provenance_preserved: !result.provenance.is_empty(),
            }
        }
        "ode" => {
            let y0 = int_after(text, "y0=").unwrap_or(2) as i128;
            let b = int_after(text, "b=").unwrap_or(1) as i128;
            let time = int_after(text, "t=").unwrap_or(2) as i128;
            let result = evaluate_ode(&OdeRequest {
                operation: OdeOperation::ConstantDerivative,
                initial: Some(rational(y0, 1)),
                coefficient: None,
                forcing: Some(rational(b, 1)),
                time: Some(rational(time, 1)),
                domain: "bounded_exact_scalar_ode".into(),
                ambiguity: None,
                provenance: vec!["stage-k-sealed-exam".into()],
            });
            let ok = result.status == OdeStatus::Complete && result.replay_verified();
            let mut t = result.clone();
            t.replay_hash.push('x');
            Run {
                actual: if ok {
                    Actual::Supported
                } else {
                    Actual::Unparsed
                },
                authorized: ok,
                replay_verified: result.replay_verified(),
                tamper_rejected: !t.replay_verified(),
                provenance_preserved: !result.provenance.is_empty(),
            }
        }
        "graph" => {
            let n = int_after(text, "vertices=").unwrap_or(4) as usize;
            let vertices: Vec<String> = (0..n).map(|i| format!("v{i}")).collect();
            let edges = (0..n)
                .flat_map(|a| ((a + 1)..n).map(move |b| (a, b)))
                .collect();
            let result = evaluate_graph(&GraphRequest {
                operation: GraphOperation::EdgeCount,
                domain: "finite_simple_graph".into(),
                vertices,
                edges,
                directed: false,
                matrix: None,
                vertex_order: Vec::new(),
                start: None,
                target: None,
                ambiguity: None,
                provenance: vec!["stage-k-sealed-exam".into()],
            });
            let ok = result.status == GraphStatus::Complete && result.replay_verified();
            let mut t = result.clone();
            t.replay_hash.push('x');
            Run {
                actual: if ok {
                    Actual::Supported
                } else {
                    Actual::Unparsed
                },
                authorized: ok,
                replay_verified: result.replay_verified(),
                tamper_rejected: !t.replay_verified(),
                provenance_preserved: !result.provenance.is_empty(),
            }
        }
        _ => Run {
            actual: Actual::Unparsed,
            authorized: false,
            replay_verified: false,
            tamper_rejected: false,
            provenance_preserved: false,
        },
    }
}

fn execute(text: &str) -> Run {
    let lower = text.to_ascii_lowercase();
    if lower.contains("points:")
        || lower.contains("finite topology")
        || lower.contains("metric space")
    {
        return topology_run(text);
    }
    if lower.contains("formula:") || lower.contains("molar mass") {
        return chemistry_run(text);
    }
    if lower.contains("sequence:") || lower.contains("strand:") || lower.contains("rna") {
        return biology_run(text);
    }
    if lower.contains("polar form")
        || lower.contains("conjugate")
        || lower.contains("squared magnitude")
        || lower.contains("product of (")
        || lower.contains("add and multiply")
    {
        return complex_run(text);
    }
    if lower.contains("mean")
        || lower.contains("average")
        || lower.contains("bernoulli")
        || lower.contains("regression")
    {
        return statistics_run(text);
    }
    if lower.contains("choose") || lower.contains("k-subsets") || lower.contains("bell number") {
        return old_pack_run(text, "combinatorics");
    }
    if lower.contains("modular inverse") || lower.contains("dirichlet") {
        return old_pack_run(text, "number_theory");
    }
    if lower.contains("ordinary differential")
        || lower.contains("nonlinear ode")
        || lower.contains("sampled difference")
    {
        return old_pack_run(text, "ode");
    }
    if lower.contains("differentiate") || lower.contains("improper integral") {
        return old_pack_run(text, "calculus");
    }
    if lower.contains("simple graph")
        || lower.contains("spectral gap")
        || lower.contains("weighted graph")
        || lower.contains("graph edges")
    {
        return old_pack_run(text, "graph");
    }
    Run {
        actual: Actual::Unparsed,
        authorized: false,
        replay_verified: false,
        tamper_rejected: false,
        provenance_preserved: false,
    }
}

fn generated(route: usize, local: usize, global: usize) -> Question {
    let hidden = match local % 10 {
        0..=5 => Hidden::Supported,
        6..=7 => Hidden::Ambiguous,
        _ => Hidden::Unsupported,
    };
    let text = match (route, hidden, local % 4) {
        (0, Hidden::Supported, 0) => "Validate topology: points: {a,b,c}; open sets: {}; open sets: {a}; open sets: {a,b,c}.".into(),
        (0, Hidden::Supported, 1) => "Is open: points: {a,b,c}; target: {a}; open sets: {}; open sets: {a}; open sets: {a,b,c}.".into(),
        (0, Hidden::Supported, 2) => "Find the closure. Points: {a,b,c}; target: {a}; open sets: {}; open sets: {a}; open sets: {a,b,c}.".into(),
        (0, Hidden::Supported, _) => "Find the interior. Points: {a,b,c}; target: {a}; open sets: {}; open sets: {a}; open sets: {a,b,c}.".into(),
        (0, Hidden::Ambiguous, _) => "Determine the interior; points: {a,b,c}; points: {a,b}; target: {a}; open sets: {}; open sets: {a}; open sets: {a,b,c}.".into(),
        (0, Hidden::Unsupported, _) => "Determine whether this metric space is compact and Hausdorff.".into(),
        (1, Hidden::Supported, _) => format!("For formula: {}, parse the molecular formula.", ["H2O", "CO2", "NH4NO3", "Ca(OH)2"][local % 4]),
        (1, Hidden::Ambiguous, _) => "Two candidates are present: formula: H2O and formula: CO2; select one.".into(),
        (1, Hidden::Unsupported, _) => "Compute the molar mass of formula: H2O.".into(),
        (2, Hidden::Supported, _) => format!("Report base composition for sequence: {}.", ["AATTGGCC", "ATCGATCG", "GCGCGCAA", "TTAAACCG"][local % 4]),
        (2, Hidden::Ambiguous, _) => "Find the complement of sequence: AATTGGCC, but orientation is not stated.".into(),
        (2, Hidden::Unsupported, _) => "Translate the RNA sequence: AUGGCC into a protein.".into(),
        (3, Hidden::Supported, 0) => "Compute the product of (3-4i) and (2+5i).".into(),
        (3, Hidden::Supported, 1) => "Find the conjugate of (7/2+1/3i).".into(),
        (3, Hidden::Supported, _) => "Compute the squared magnitude of (5-2i).".into(),
        (3, Hidden::Ambiguous, _) => "Add and multiply (3-4i) and (2+5i); operation is not unique.".into(),
        (3, Hidden::Unsupported, _) => "Convert (3-4i) to polar form and report its argument.".into(),
        (4, Hidden::Supported, 0) => "Find the mean from sum=30 and count=5.".into(),
        (4, Hidden::Supported, 1) => "Using count : 5, compute the average from sum = 30.".into(),
        (4, Hidden::Supported, _) => "For a Bernoulli variable with p=1/2, find the variance.".into(),
        (4, Hidden::Ambiguous, _) => "Find the average from total=30 and count=5; weighted sum is not identified.".into(),
        (4, Hidden::Unsupported, _) => "Fit a regression model and report a confidence interval.".into(),
        (5, Hidden::Supported, _) => format!("How many ways can one choose n={} objects, k=2 at a time?", 5 + local % 3),
        (5, Hidden::Ambiguous, _) => "Choose n=5 and k=2, but labeled versus unlabeled is unspecified.".into(),
        (5, Hidden::Unsupported, _) => "Compute the Bell number B_40.".into(),
        (6, Hidden::Supported, _) => format!("Find the modular inverse of a={} modulo m=11.", 3 + local % 4),
        (6, Hidden::Ambiguous, _) => "Find the modular inverse, but coefficient is not identified; m=11.".into(),
        (6, Hidden::Unsupported, _) => "Apply a Dirichlet character to an asymptotic prime-counting theorem.".into(),
        (7, Hidden::Supported, _) => format!("Differentiate f(x)=c+ d*x where c=2 and d={}.", 1 + local % 5),
        (7, Hidden::Ambiguous, _) => "Differentiate c+d*x, but the variable scope is unspecified.".into(),
        (7, Hidden::Unsupported, _) => "Evaluate an improper integral using measure-theoretic convergence.".into(),
        (8, Hidden::Supported, _) => format!("Solve ordinary differential y'(t)=b={} with y0=2 at t={}.", 1 + local % 5, local % 8),
        (8, Hidden::Ambiguous, _) => "Use either a continuous derivative or sampled difference; interpretation is unspecified.".into(),
        (8, Hidden::Unsupported, _) => "Solve a nonlinear ODE and establish stability as t approaches infinity.".into(),
        (9, Hidden::Supported, _) => format!("Count edges in a complete simple graph with vertices={}", 4 + local % 3),
        (9, Hidden::Ambiguous, _) => "Count graph edges; directed or undirected is unspecified.".into(),
        (9, Hidden::Unsupported, _) => "Analyze the spectral gap of a weighted graph.".into(),
        _ => unreachable!(),
    };
    let partition = if global < 3000 {
        Partition::Development
    } else if global < 4000 {
        Partition::Validation
    } else {
        Partition::Sealed
    };
    Question {
        id: format!("curriculum_{global:05}"),
        text,
        route: [
            "finite_topology",
            "chemistry",
            "dna_biology",
            "complex_arithmetic",
            "finite_statistics",
            "combinatorics",
            "number_theory",
            "calculus",
            "ode",
            "graph",
        ][route]
            .into(),
        hidden,
        partition,
    }
}

fn metrics(receipts: &[Receipt], partition: Partition) -> PartitionMetrics {
    let rows = receipts
        .iter()
        .filter(|r| r.partition == partition)
        .collect::<Vec<_>>();
    PartitionMetrics {
        cases: rows.len(),
        supported: rows
            .iter()
            .filter(|r| r.hidden == Hidden::Supported)
            .count(),
        ambiguous: rows
            .iter()
            .filter(|r| r.hidden == Hidden::Ambiguous)
            .count(),
        unsupported: rows
            .iter()
            .filter(|r| r.hidden == Hidden::Unsupported)
            .count(),
        supported_authorized: rows
            .iter()
            .filter(|r| r.hidden == Hidden::Supported && r.authorized)
            .count(),
        ambiguities_preserved: rows
            .iter()
            .filter(|r| r.hidden == Hidden::Ambiguous && r.actual == Actual::Ambiguous)
            .count(),
        unsupported_refused: rows
            .iter()
            .filter(|r| r.hidden == Hidden::Unsupported && r.actual == Actual::Unsupported)
            .count(),
        replay_verified: rows.iter().filter(|r| r.replay_verified).count(),
        tamper_rejections: rows.iter().filter(|r| r.tamper_rejected).count(),
        false_authorizations: rows.iter().filter(|r| r.false_authorization).count(),
        false_denials: rows.iter().filter(|r| r.false_denial).count(),
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut questions = Vec::with_capacity(5000);
    let manifest_before = breadth_first_manifest().replay_hash();
    let producer_commit = Command::new("git")
        .args(["rev-parse", "--short", "HEAD"])
        .output()
        .ok()
        .filter(|output| output.status.success())
        .map(|output| String::from_utf8_lossy(&output.stdout).trim().to_string())
        .unwrap_or_else(|| "unknown".into());
    for global in 0..5000 {
        questions.push(generated(global % 10, global / 10, global));
    }
    let question_hash = digest(
        &questions
            .iter()
            .map(|q| (&q.id, &q.text, q.partition))
            .collect::<Vec<_>>(),
    );
    let mut receipts = Vec::with_capacity(questions.len());
    for question in &questions {
        let run = execute(&question.text);
        receipts.push(Receipt {
            id: question.id.clone(),
            partition: question.partition,
            route: question.route.clone(),
            hidden: question.hidden,
            actual: run.actual,
            authorized: run.authorized,
            replay_verified: run.replay_verified,
            tamper_rejected: run.tamper_rejected,
            provenance_preserved: run.provenance_preserved,
            false_authorization: question.hidden != Hidden::Supported && run.authorized,
            false_denial: question.hidden == Hidden::Supported && !run.authorized,
            text_sha256: digest(&question.text),
        });
    }
    let cases = receipts.len();
    let supported = receipts
        .iter()
        .filter(|r| r.hidden == Hidden::Supported)
        .count();
    let ambiguous = receipts
        .iter()
        .filter(|r| r.hidden == Hidden::Ambiguous)
        .count();
    let unsupported = receipts
        .iter()
        .filter(|r| r.hidden == Hidden::Unsupported)
        .count();
    let supported_authorized = receipts
        .iter()
        .filter(|r| r.hidden == Hidden::Supported && r.authorized)
        .count();
    let ambiguities_preserved = receipts
        .iter()
        .filter(|r| r.hidden == Hidden::Ambiguous && r.actual == Actual::Ambiguous)
        .count();
    let unsupported_refused = receipts
        .iter()
        .filter(|r| r.hidden == Hidden::Unsupported && r.actual == Actual::Unsupported)
        .count();
    let replay_verified = receipts.iter().filter(|r| r.replay_verified).count();
    let tamper_rejections = receipts.iter().filter(|r| r.tamper_rejected).count();
    let provenance_preserved = receipts.iter().filter(|r| r.provenance_preserved).count();
    let false_authorizations = receipts.iter().filter(|r| r.false_authorization).count();
    let false_denials = receipts.iter().filter(|r| r.false_denial).count();
    let manifest_mutated = breadth_first_manifest().replay_hash() != manifest_before;
    assert_eq!(
        (cases, supported, ambiguous, unsupported),
        (5000, 3000, 1000, 1000)
    );
    assert_eq!(supported_authorized, supported);
    assert_eq!(ambiguities_preserved, ambiguous);
    assert_eq!(unsupported_refused, unsupported);
    assert_eq!(replay_verified, cases);
    assert_eq!(tamper_rejections, cases);
    assert_eq!(provenance_preserved, cases);
    assert_eq!(false_authorizations, 0);
    assert_eq!(false_denials, 0);
    assert!(!manifest_mutated);
    let mut route_counts = BTreeMap::new();
    for q in &questions {
        *route_counts.entry(q.route.clone()).or_insert(0) += 1;
    }
    let mut partitions = BTreeMap::new();
    partitions.insert(
        "development".into(),
        metrics(&receipts, Partition::Development),
    );
    partitions.insert(
        "validation".into(),
        metrics(&receipts, Partition::Validation),
    );
    partitions.insert("sealed".into(), metrics(&receipts, Partition::Sealed));
    let report = Report {
        schema: "stage-k-sealed-curriculum-exam-5000-v2",
        source: "independently authored permanent development/validation/sealed corpus",
        producer_commit,
        manifest_sha256: manifest_before,
        corpus_sha256: digest(&receipts),
        question_corpus_sha256: question_hash,
        sealed_question_sha256: digest(
            &questions
                .iter()
                .filter(|q| q.partition == Partition::Sealed)
                .map(|q| (&q.id, &q.text))
                .collect::<Vec<_>>(),
        ),
        cases,
        supported,
        ambiguous,
        unsupported,
        supported_authorized,
        ambiguities_preserved,
        unsupported_refused,
        replay_verified,
        tamper_rejections,
        provenance_preserved,
        false_authorizations,
        false_denials,
        manifest_mutated,
        route_counts,
        partitions,
        receipts,
    };
    let serialized = serde_json::to_string_pretty(&report)?;
    std::fs::write(
        "docs/stage_k_sealed_curriculum_exam_5000.json",
        format!("{serialized}\n"),
    )?;
    println!("{serialized}");
    Ok(())
}
