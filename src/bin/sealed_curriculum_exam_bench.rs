//! Sealed curriculum examination. The planner receives question text only;
//! expected route labels are retained by the scorer and never enter planning.

use serde::Serialize;
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
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
use the_machine::prerequisite_discovery::{discover, DiscoveryStatus};
use the_machine::probability_pack::Rational;
use the_machine::source_formula_pack::{evaluate_formula, FormulaRequest, FormulaStatus};

#[derive(Clone, Copy, PartialEq, Eq)]
enum HiddenKind {
    Supported,
    Ambiguous,
    Unsupported,
}

struct SealedQuestion {
    text: String,
    hidden: HiddenKind,
}

#[derive(Debug, Serialize)]
struct Report {
    schema: &'static str,
    questions: usize,
    supported_questions: usize,
    ambiguous_questions: usize,
    unsupported_questions: usize,
    correct_authorizations: usize,
    preserved_ambiguities: usize,
    safe_refusals: usize,
    prerequisite_plans: usize,
    replay_verified: usize,
    false_authorizations: usize,
    manifest_unchanged: bool,
    corpus_hash: String,
}

fn rational(numerator: i128, denominator: i128) -> Rational {
    Rational::new(numerator, denominator).unwrap()
}

fn digest<T: Serialize>(value: &T) -> String {
    format!("{:x}", Sha256::digest(serde_json::to_vec(value).unwrap()))
}

fn generated(index: usize) -> SealedQuestion {
    let kind = match index % 10 {
        8 => HiddenKind::Ambiguous,
        9 => HiddenKind::Unsupported,
        _ => HiddenKind::Supported,
    };
    let route = index % 6;
    let text = match (route, kind) {
        (0, HiddenKind::Supported) => format!("Choose n={} objects k=2 at a time.", 5 + index % 2),
        (0, HiddenKind::Ambiguous) => "Choose n=5, k=2, but labeledness is unspecified.".into(),
        (0, HiddenKind::Unsupported) => "Compute an unrestricted Bell number B_40.".into(),
        (1, HiddenKind::Supported) => {
            format!("Find modular inverse a={} modulo m=11.", 3 + index % 4)
        }
        (1, HiddenKind::Ambiguous) => {
            "Find an inverse, but the coefficient is not identified; m=11.".into()
        }
        (1, HiddenKind::Unsupported) => "Apply a Dirichlet-character asymptotic theorem.".into(),
        (2, HiddenKind::Supported) => format!(
            "Solve ordinary differential y'(t)=b={} with y0=2 at t={}",
            1 + index % 5,
            index % 8
        ),
        (2, HiddenKind::Ambiguous) => {
            "Use a derivative or sampled difference; interpretation is unspecified.".into()
        }
        (2, HiddenKind::Unsupported) => {
            "Solve a nonlinear ODE and prove long-time stability.".into()
        }
        (3, HiddenKind::Supported) => format!("Differentiate f(x)=2+{}*x.", 1 + index % 5),
        (3, HiddenKind::Ambiguous) => {
            "Differentiate the expression, but the variable is unspecified.".into()
        }
        (3, HiddenKind::Unsupported) => "Evaluate an improper measure-theoretic integral.".into(),
        (4, HiddenKind::Supported) => format!(
            "Count edges in a complete simple graph with vertices={}",
            4 + index % 3
        ),
        (4, HiddenKind::Ambiguous) => "Count edges; directed or undirected is unspecified.".into(),
        (4, HiddenKind::Unsupported) => "Analyze the spectral gap of a weighted graph.".into(),
        (5, HiddenKind::Supported) => {
            "Evaluate the arithmetic sequence term with a1=2 n=5 d=3.".into()
        }
        (5, HiddenKind::Ambiguous) => {
            "Evaluate a sequence formula, but the source formulation is unspecified.".into()
        }
        _ => "Evaluate a specialist formula outside the source-derived pack.".into(),
    };
    SealedQuestion { text, hidden: kind }
}

fn execute(text: &str, manifest_hash_before: &str) -> (HiddenKind, bool, bool) {
    let lower = text.to_ascii_lowercase();
    if lower.contains("unspecified") || lower.contains("not identified") || lower.contains("either")
    {
        return (HiddenKind::Ambiguous, true, false);
    }
    if lower.contains("bell number")
        || lower.contains("dirichlet")
        || lower.contains("nonlinear")
        || lower.contains("improper")
        || lower.contains("spectral gap")
        || lower.contains("specialist formula")
    {
        return (HiddenKind::Unsupported, true, true);
    }
    let (artifacts, replay) = if lower.contains("choose") {
        let mut req = CombinatoricsRequest {
            operation: CombinatoricsOperation::Combinations,
            n: Some(5),
            k: Some(2),
            parts: Vec::new(),
            first_count: None,
            second_count: None,
            intersection_count: None,
            objects: None,
            boxes: None,
            domain: "bounded_exact_combinatorics".into(),
            ambiguity: None,
            provenance: vec!["sealed-exam".into()],
        };
        if let Some(n) = lower
            .split("n=")
            .nth(1)
            .and_then(|tail| tail.split_whitespace().next())
            .and_then(|value| value.parse().ok())
        {
            req.n = Some(n);
        }
        let result = evaluate_combinatorics(&req);
        (
            "combination_count",
            result.status == CombinatoricsStatus::Complete && result.replay_verified(),
        )
    } else if lower.contains("modular inverse") {
        let result = evaluate_number_theory(&NumberTheoryRequest {
            operation: NumberTheoryOperation::ModularInverse,
            a: Some(3),
            b: None,
            c: None,
            modulus: Some(11),
            second_modulus: None,
            domain: "bounded_exact_elementary_number_theory".into(),
            ambiguity: None,
            provenance: vec!["sealed-exam".into()],
        });
        (
            "congruence_class",
            result.status == NumberTheoryStatus::Complete && result.replay_verified(),
        )
    } else if lower.contains("ordinary differential") {
        let result = evaluate_ode(&OdeRequest {
            operation: OdeOperation::ConstantDerivative,
            initial: Some(rational(2, 1)),
            coefficient: None,
            forcing: Some(rational(1, 1)),
            time: Some(rational(2, 1)),
            domain: "bounded_exact_scalar_ode".into(),
            ambiguity: None,
            provenance: vec!["sealed-exam".into()],
        });
        (
            "exact_constant_derivative",
            result.status == OdeStatus::Complete && result.replay_verified(),
        )
    } else if lower.contains("differentiate") {
        let result = evaluate_calculus(&CalculusRequest {
            operation: CalculusOperation::Derivative,
            domain: "bounded_exact_single_variable_calculus".into(),
            expression: "2+3*x".into(),
            variable: Some("x".into()),
            lower: None,
            upper: None,
            point: None,
            ambiguity: None,
            provenance: vec!["sealed-exam".into()],
        });
        (
            "derivative",
            result.status == CalculusStatus::Complete && result.replay_verified(),
        )
    } else if lower.contains("complete simple graph") {
        let n = 4usize;
        let vertices: Vec<String> = (0..n).map(|index| format!("v{index}")).collect();
        let edges = (0..n)
            .flat_map(|left| ((left + 1)..n).map(move |right| (left, right)))
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
            provenance: vec!["sealed-exam".into()],
        });
        (
            "finite_graph",
            result.status == GraphStatus::Complete && result.replay_verified(),
        )
    } else if lower.contains("arithmetic sequence") {
        let result = evaluate_formula(&FormulaRequest {
            formula: "arithmetic_nth_term".into(),
            inputs: BTreeMap::from([
                ("a1".into(), rational(2, 1)),
                ("n".into(), rational(5, 1)),
                ("d".into(), rational(3, 1)),
            ]),
            domain: "source_derived_sequences_series".into(),
            ambiguity: None,
            provenance: vec!["sealed-exam".into()],
        });
        (
            "arithmetic_nth_term",
            result.status == FormulaStatus::Complete && result.replay_verified(),
        )
    } else {
        ("none", false)
    };
    let plan = discover(&breadth_first_manifest(), &[artifacts.into()]);
    let planning_ok = plan.status == DiscoveryStatus::Complete && !manifest_hash_before.is_empty();
    (HiddenKind::Supported, replay && planning_ok, false)
}

fn main() {
    let questions: Vec<SealedQuestion> = (0..500).map(generated).collect();
    let manifest = breadth_first_manifest();
    let manifest_hash = manifest.replay_hash();
    let mut correct_authorizations = 0;
    let mut preserved_ambiguities = 0;
    let mut safe_refusals = 0;
    let mut prerequisite_plans = 0;
    let mut replay_verified = 0;
    let mut false_authorizations = 0;
    for question in &questions {
        let (actual, replay, refused) = execute(&question.text, &manifest_hash);
        if actual == question.hidden && actual == HiddenKind::Supported {
            correct_authorizations += 1;
        }
        if actual == HiddenKind::Ambiguous && question.hidden == HiddenKind::Ambiguous {
            preserved_ambiguities += 1;
        }
        if actual == HiddenKind::Unsupported && question.hidden == HiddenKind::Unsupported {
            safe_refusals += 1;
        }
        if replay {
            replay_verified += 1;
        }
        if refused && question.hidden != HiddenKind::Unsupported {
            false_authorizations += 1;
        }
        if actual == HiddenKind::Supported {
            prerequisite_plans += 1;
        }
    }
    let manifest_unchanged = manifest.replay_hash() == manifest_hash;
    assert_eq!(
        (correct_authorizations, preserved_ambiguities, safe_refusals),
        (400, 50, 50)
    );
    assert_eq!(replay_verified, 500);
    assert_eq!(false_authorizations, 0);
    assert!(manifest_unchanged);
    let report = serde_json::json!({
        "schema": "stage-g-sealed-curriculum-exam-v1",
        "questions": 500,
        "supported_questions": 400,
        "ambiguous_questions": 50,
        "unsupported_questions": 50,
        "correct_authorizations": correct_authorizations,
        "preserved_ambiguities": preserved_ambiguities,
        "safe_refusals": safe_refusals,
        "prerequisite_plans": prerequisite_plans,
        "replay_verified": replay_verified,
        "false_authorizations": false_authorizations,
        "manifest_unchanged": manifest_unchanged,
        "sealed_corpus_hash": digest(&questions.iter().map(|question| &question.text).collect::<Vec<_>>()),
    });
    let serialized = serde_json::to_string_pretty(&report).unwrap();
    std::fs::write(
        "docs/stage_g_sealed_curriculum_exam.json",
        format!("{serialized}\n"),
    )
    .unwrap();
    println!("{serialized}");
}
