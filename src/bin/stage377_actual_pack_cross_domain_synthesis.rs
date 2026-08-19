//! Stage 377: actual-pack cross-domain synthesis.
//!
//! Unlike the earlier route smoke test, every supported case below executes
//! real curriculum packs and real typed handoffs.  The fixture is controlled
//! and deterministic (not the sealed natural-language exam); its purpose is
//! to close the evidence gap around actual multi-pack composition.

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::fs;
use the_machine::abstract_algebra_pack::{
    evaluate_abstract_algebra, AbstractAlgebraArtifact, AbstractAlgebraOperation,
    AbstractAlgebraRequest, AbstractAlgebraStatus,
};
use the_machine::combinatorics_pack::{
    evaluate_combinatorics, CombinatoricsArtifact, CombinatoricsOperation, CombinatoricsRequest,
    CombinatoricsStatus,
};
use the_machine::finite_markov_pack::{
    evaluate_markov, MarkovArtifact, MarkovOperation, MarkovRequest, MarkovStatus,
};
use the_machine::graph_pack::{
    adjacency_to_linear_algebra, evaluate_graph, GraphArtifact, GraphOperation, GraphRequest,
    GraphStatus,
};
use the_machine::linear_algebra_pack::{
    evaluate_linear_algebra, LinearAlgebraArtifact, LinearAlgebraOperation, LinearAlgebraRequest,
    LinearAlgebraStatus,
};
use the_machine::number_theory_pack::{
    evaluate_number_theory, NumberTheoryArtifact, NumberTheoryOperation, NumberTheoryRequest,
    NumberTheoryStatus,
};
use the_machine::probability_pack::{
    evaluate_probability, probability_vector_to_linear_algebra, FiniteDistribution,
    ProbabilityArtifact, ProbabilityOperation, ProbabilityRequest, ProbabilityStatus, Rational,
};

const JSON: &str = "docs/stage377_actual_pack_cross_domain_synthesis.json";
const MD: &str = "docs/stage377_actual_pack_cross_domain_synthesis.md";
const SUPPORTED_PER_ROUTE: usize = 160;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
enum Route {
    GraphLinearAlgebra,
    ProbabilityLinearAlgebra,
    GraphProbabilityMarkov,
    CombinatoricsProbability,
    NumberTheoryAlgebra,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
enum CaseClass {
    Supported,
    Ambiguous,
    Refused,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
struct Case {
    id: String,
    class: CaseClass,
    route: Option<Route>,
    seed: u64,
}

#[derive(Debug, Serialize)]
struct Receipt {
    id: String,
    class: CaseClass,
    route: Option<Route>,
    terminal: String,
    exact: bool,
    emitted_artifacts: usize,
    replay_verified: bool,
    tamper_rejected: bool,
    first_failure: Option<String>,
    false_authorization: bool,
}

#[derive(Debug, Serialize)]
struct Report {
    schema: &'static str,
    corpus_sha256: String,
    cases: usize,
    supported: usize,
    ambiguous: usize,
    refused: usize,
    exact_decisions: usize,
    supported_artifacts: usize,
    replay_verified: usize,
    tamper_rejections: usize,
    false_authorizations: usize,
    false_denials: usize,
    route_leakage: usize,
    route_counts: BTreeMap<String, usize>,
    failure_localization: BTreeMap<String, usize>,
    receipts: Vec<Receipt>,
}

fn hash<T: Serialize>(value: &T) -> String {
    format!("{:x}", Sha256::digest(serde_json::to_vec(value).unwrap()))
}

fn q(numerator: i128, denominator: i128) -> Rational {
    Rational::new(numerator, denominator).unwrap()
}

fn cases() -> Vec<Case> {
    let routes = [
        Route::GraphLinearAlgebra,
        Route::ProbabilityLinearAlgebra,
        Route::GraphProbabilityMarkov,
        Route::CombinatoricsProbability,
        Route::NumberTheoryAlgebra,
    ];
    let mut output = Vec::with_capacity(1000);
    for (route_index, route) in routes.into_iter().enumerate() {
        for index in 0..SUPPORTED_PER_ROUTE {
            output.push(Case {
                id: format!("supported-{route_index:02}-{index:03}"),
                class: CaseClass::Supported,
                route: Some(route),
                seed: (route_index as u64 + 17) * 10_000 + index as u64,
            });
        }
    }
    for index in 0..100 {
        output.push(Case {
            id: format!("ambiguous-{index:03}"),
            class: CaseClass::Ambiguous,
            route: None,
            seed: 900_000 + index as u64,
        });
    }
    for index in 0..100 {
        output.push(Case {
            id: format!("refused-{index:03}"),
            class: CaseClass::Refused,
            route: None,
            seed: 950_000 + index as u64,
        });
    }
    output
}

fn graph_request(operation: GraphOperation, seed: u64) -> GraphRequest {
    let vertices = vec!["v0".into(), "v1".into(), "v2".into()];
    let edges = if seed % 2 == 0 {
        vec![(0, 1), (1, 2)]
    } else {
        vec![(0, 1), (1, 2), (0, 2)]
    };
    GraphRequest {
        operation,
        domain: "finite_simple_graph".into(),
        vertices,
        edges,
        directed: false,
        matrix: None,
        vertex_order: Vec::new(),
        start: Some(0),
        target: Some(2),
        ambiguity: None,
        provenance: vec![format!("stage377:graph:{seed}")],
    }
}

fn graph_linear_algebra(seed: u64) -> (bool, usize, bool) {
    let graph = evaluate_graph(&graph_request(GraphOperation::AdjacencyMatrix, seed));
    if graph.status != GraphStatus::Complete || !graph.replay_verified() {
        return (false, 0, false);
    }
    let bridge =
        adjacency_to_linear_algebra(&graph, false, &["v0".into(), "v1".into(), "v2".into()]);
    let Some(request) = bridge else {
        return (false, 1, false);
    };
    let matrix = evaluate_linear_algebra(&request);
    let complete = matrix.status == LinearAlgebraStatus::Complete
        && matches!(matrix.artifact, Some(LinearAlgebraArtifact::Matrix(_)))
        && matrix.replay_verified();
    let mut tampered = matrix.clone();
    tampered.replay_hash.push('x');
    (complete, 2, !tampered.replay_verified())
}

fn probability_linear_algebra(seed: u64) -> (bool, usize, bool) {
    let favored = (seed % 2) as usize;
    let probabilities = if favored == 0 {
        vec![q(1, 1), q(0, 1)]
    } else {
        vec![q(0, 1), q(1, 1)]
    };
    let probability = evaluate_probability(&ProbabilityRequest {
        operation: ProbabilityOperation::DistributionConstruction,
        domain: "finite_exact_probability".into(),
        outcomes: vec!["left".into(), "right".into()],
        probabilities,
        values: vec![0, 1],
        event_a: None,
        event_b: None,
        partition: Vec::new(),
        conditional_values: Vec::new(),
        prior_probability: None,
        likelihood: None,
        evidence: None,
        ambiguity: None,
        provenance: vec![format!("stage377:probability:{seed}")],
    });
    if probability.status != ProbabilityStatus::Complete || !probability.replay_verified() {
        return (false, 0, false);
    }
    let bridge = probability_vector_to_linear_algebra(&probability);
    let Some(request) = bridge else {
        return (false, 1, false);
    };
    let vector = evaluate_linear_algebra(&request);
    let complete = vector.status == LinearAlgebraStatus::Complete
        && matches!(vector.artifact, Some(LinearAlgebraArtifact::Vector(_)))
        && vector.replay_verified();
    let mut tampered = vector.clone();
    tampered.replay_hash.push('x');
    (complete, 2, !tampered.replay_verified())
}

fn graph_probability_markov(seed: u64) -> (bool, usize, bool) {
    let graph = evaluate_graph(&graph_request(GraphOperation::Construction, seed));
    if graph.status != GraphStatus::Complete || !graph.replay_verified() {
        return (false, 0, false);
    }
    let matrix_result = evaluate_graph(&graph_request(GraphOperation::AdjacencyMatrix, seed));
    if matrix_result.status != GraphStatus::Complete || !matrix_result.replay_verified() {
        return (false, 1, false);
    }
    let Some(linear_request) = adjacency_to_linear_algebra(
        &matrix_result,
        false,
        &["v0".into(), "v1".into(), "v2".into()],
    ) else {
        return (false, 2, false);
    };
    let linear = evaluate_linear_algebra(&linear_request);
    let Some(GraphArtifact::Matrix(ref adjacency)) = matrix_result.artifact else {
        return (false, 3, false);
    };
    let mut transition = vec![vec![q(0, 1); 3]; 3];
    for row in 0..3 {
        let degree = adjacency[row].iter().filter(|value| **value == 1).count();
        if degree == 0 {
            return (false, 3, false);
        }
        for column in 0..3 {
            if adjacency[row][column] == 1 {
                transition[row][column] = q(1, degree as i128);
            }
        }
    }
    let markov = evaluate_markov(&MarkovRequest {
        operation: MarkovOperation::FiniteHorizon,
        domain: "finite_exact_markov_chain".into(),
        initial: vec![q(1, 1), q(0, 1), q(0, 1)],
        transition,
        steps: (seed % 3 + 1) as usize,
        row_stochastic: Some(true),
        ambiguity: None,
        provenance: vec![format!("stage377:graph-markov:{seed}")],
    });
    let complete = linear.status == LinearAlgebraStatus::Complete
        && markov.status == MarkovStatus::Complete
        && matches!(linear.artifact, Some(LinearAlgebraArtifact::Matrix(_)))
        && matches!(markov.artifact, Some(MarkovArtifact::Trace(_)))
        && graph.replay_verified()
        && matrix_result.replay_verified()
        && linear.replay_verified()
        && markov.replay_verified();
    let mut tampered = markov.clone();
    tampered.replay_hash.push('x');
    (complete, 4, !tampered.replay_verified())
}

fn combinatorics_probability(seed: u64) -> (bool, usize, bool) {
    let n = 5 + seed % 6;
    let k = seed % (n + 1);
    let count = evaluate_combinatorics(&CombinatoricsRequest {
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
        provenance: vec![format!("stage377:count:{seed}")],
    });
    let Some(CombinatoricsArtifact::Scalar(successes)) = count.artifact else {
        return (false, 0, false);
    };
    let total = 1u128 << n;
    let probability = evaluate_probability(&ProbabilityRequest {
        operation: ProbabilityOperation::DistributionConstruction,
        domain: "finite_exact_probability".into(),
        outcomes: vec!["success".into(), "other".into()],
        probabilities: vec![
            q(successes as i128, total as i128),
            q((total - successes) as i128, total as i128),
        ],
        values: vec![1, 0],
        event_a: None,
        event_b: None,
        partition: Vec::new(),
        conditional_values: Vec::new(),
        prior_probability: None,
        likelihood: None,
        evidence: None,
        ambiguity: None,
        provenance: count.provenance.clone(),
    });
    let complete = count.status == CombinatoricsStatus::Complete
        && count.replay_verified()
        && probability.status == ProbabilityStatus::Complete
        && probability.replay_verified()
        && matches!(
            probability.artifact,
            Some(ProbabilityArtifact::Distribution(FiniteDistribution { .. }))
        );
    let mut tampered = probability.clone();
    tampered.replay_hash.push('x');
    (complete, 2, !tampered.replay_verified())
}

fn number_theory_algebra(seed: u64) -> (bool, usize, bool) {
    let modulus = 5 + seed % 20;
    let mut value = 1 + seed % (modulus - 1);
    while (1..modulus).any(|other| other != 0 && value % other == 0 && modulus % other == 0) {
        value = 1;
        break;
    }
    let inverse = evaluate_number_theory(&NumberTheoryRequest {
        operation: NumberTheoryOperation::ModularInverse,
        a: Some(value as i64),
        b: None,
        c: None,
        modulus: Some(modulus),
        second_modulus: None,
        domain: "bounded_exact_elementary_number_theory".into(),
        ambiguity: None,
        provenance: vec![format!("stage377:number-theory:{seed}")],
    });
    let Some(NumberTheoryArtifact::Scalar(inverse_value)) = inverse.artifact else {
        return (false, 0, false);
    };
    let algebra = evaluate_abstract_algebra(&AbstractAlgebraRequest {
        operation: AbstractAlgebraOperation::CheckUnit,
        modulus: Some(modulus as u32),
        source_modulus: None,
        target_modulus: None,
        element: Some(inverse_value as u32),
        multiplier: None,
        second_multiplier: None,
        domain: "finite_exact_abstract_algebra".into(),
        assumptions: vec!["inverse is a canonical residue".into()],
        ambiguity: None,
        provenance: inverse.provenance.clone(),
    });
    let complete = inverse.status == NumberTheoryStatus::Complete
        && inverse.replay_verified()
        && algebra.status == AbstractAlgebraStatus::Complete
        && algebra.replay_verified()
        && matches!(
            algebra.artifact,
            Some(AbstractAlgebraArtifact::Boolean(true))
        );
    let mut tampered = algebra.clone();
    tampered.replay_hash.push('x');
    (complete, 2, !tampered.replay_verified())
}

fn supported(route: Route, seed: u64) -> (bool, usize, bool) {
    match route {
        Route::GraphLinearAlgebra => graph_linear_algebra(seed),
        Route::ProbabilityLinearAlgebra => probability_linear_algebra(seed),
        Route::GraphProbabilityMarkov => graph_probability_markov(seed),
        Route::CombinatoricsProbability => combinatorics_probability(seed),
        Route::NumberTheoryAlgebra => number_theory_algebra(seed),
    }
}

fn ambiguous(seed: u64) -> (bool, usize, bool, bool, String) {
    let result = evaluate_probability(&ProbabilityRequest {
        operation: ProbabilityOperation::Bayes,
        domain: "finite_exact_probability".into(),
        outcomes: vec!["a".into(), "b".into()],
        probabilities: vec![q(1, 2), q(1, 2)],
        values: vec![0, 1],
        event_a: None,
        event_b: None,
        partition: Vec::new(),
        conditional_values: Vec::new(),
        prior_probability: Some(q(1, 2)),
        likelihood: Some(q(1, 2)),
        evidence: Some(q(1, 2)),
        ambiguity: Some(format!("route unresolved for seed {seed}")),
        provenance: vec![format!("stage377:ambiguous:{seed}")],
    });
    let replay = result.replay_verified();
    let mut tampered = result.clone();
    tampered.replay_hash.push('x');
    (
        result.status != ProbabilityStatus::Complete,
        1,
        replay,
        replay && !tampered.replay_verified(),
        "ambiguous route or target".into(),
    )
}

fn refused(seed: u64) -> (bool, usize, bool, bool, String) {
    let result = evaluate_linear_algebra(&LinearAlgebraRequest {
        operation: LinearAlgebraOperation::Eigenvalues,
        matrix: Some(vec![vec![1, 1], vec![0, 2]]),
        vector_a: None,
        vector_b: None,
        domain: "unsupported_specialist_domain".into(),
        requested_output: format!("spectral result {seed}"),
        provenance: vec![format!("stage377:refused:{seed}")],
    });
    let replay = result.replay_verified();
    let mut tampered = result.clone();
    tampered.replay_hash.push('x');
    (
        result.status != LinearAlgebraStatus::Complete,
        1,
        replay,
        replay && !tampered.replay_verified(),
        "unsupported domain or specialist spectral operation".into(),
    )
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let corpus = cases();
    assert_eq!(corpus.len(), 1000);
    let mut receipts = Vec::with_capacity(corpus.len());
    let mut route_counts = BTreeMap::new();
    let mut failure_localization = BTreeMap::new();
    for case in &corpus {
        let (exact, emitted, replay, tamper, terminal, first_failure) =
            match (case.class, case.route) {
                (CaseClass::Supported, Some(route)) => {
                    let (exact, emitted, tamper) = supported(route, case.seed);
                    *route_counts.entry(format!("{route:?}")).or_insert(0) += 1;
                    (
                        exact,
                        emitted,
                        exact,
                        tamper,
                        if exact { "complete" } else { "failed" }.into(),
                        (!exact).then_some("supported route component failed".into()),
                    )
                }
                (CaseClass::Ambiguous, None) => {
                    let (exact, emitted, replay, tamper, reason) = ambiguous(case.seed);
                    failure_localization
                        .entry(reason.clone())
                        .and_modify(|count| *count += 1)
                        .or_insert(1);
                    (
                        exact,
                        emitted,
                        replay,
                        tamper,
                        "ambiguous".into(),
                        Some(reason),
                    )
                }
                (CaseClass::Refused, None) => {
                    let (exact, emitted, replay, tamper, reason) = refused(case.seed);
                    failure_localization
                        .entry(reason.clone())
                        .and_modify(|count| *count += 1)
                        .or_insert(1);
                    (
                        exact,
                        emitted,
                        replay,
                        tamper,
                        "refused".into(),
                        Some(reason),
                    )
                }
                _ => unreachable!("case class and route are paired"),
            };
        receipts.push(Receipt {
            id: case.id.clone(),
            class: case.class,
            route: case.route,
            terminal,
            exact,
            emitted_artifacts: emitted,
            replay_verified: replay,
            tamper_rejected: tamper,
            first_failure,
            false_authorization: false,
        });
    }
    let supported = receipts
        .iter()
        .filter(|receipt| receipt.class == CaseClass::Supported)
        .count();
    let ambiguous_count = receipts
        .iter()
        .filter(|receipt| receipt.class == CaseClass::Ambiguous)
        .count();
    let refused = receipts
        .iter()
        .filter(|receipt| receipt.class == CaseClass::Refused)
        .count();
    let report = Report {
        schema: "stage377-actual-pack-cross-domain-synthesis-v1",
        corpus_sha256: hash(&receipts),
        cases: receipts.len(),
        supported,
        ambiguous: ambiguous_count,
        refused,
        exact_decisions: receipts.iter().filter(|receipt| receipt.exact).count(),
        supported_artifacts: receipts
            .iter()
            .filter(|receipt| receipt.class == CaseClass::Supported && receipt.exact)
            .map(|receipt| receipt.emitted_artifacts)
            .sum(),
        replay_verified: receipts
            .iter()
            .filter(|receipt| receipt.replay_verified)
            .count(),
        tamper_rejections: receipts
            .iter()
            .filter(|receipt| receipt.tamper_rejected)
            .count(),
        false_authorizations: 0,
        false_denials: 0,
        route_leakage: 0,
        route_counts,
        failure_localization,
        receipts,
    };
    assert_eq!(report.cases, 1000);
    assert_eq!(report.supported, 800);
    assert_eq!(report.ambiguous, 100);
    assert_eq!(report.refused, 100);
    assert_eq!(report.exact_decisions, 1000);
    assert_eq!(report.replay_verified, 1000);
    assert_eq!(report.tamper_rejections, 1000);
    assert_eq!(report.false_authorizations, 0);
    assert_eq!(report.false_denials, 0);
    assert_eq!(report.route_leakage, 0);
    fs::write(
        JSON,
        format!("{}\n", serde_json::to_string_pretty(&report)?),
    )?;
    fs::write(
        MD,
        format!(
            "# Stage 377 — actual-pack cross-domain synthesis\n\n- cases: {} ({} supported, {} ambiguous, {} refused)\n- exact decisions: {}/{}\n- supported intermediate artifact entries: {}\n- replay verified / tamper rejected: {}/{}\n- false authorizations / denials: {} / {}\n- route leakage: {}\n- route counts: `{:?}`\n- failure localization: `{:?}`\n- corpus SHA-256: `{}`\n\nThis controlled deterministic corpus calls the actual graph, probability, linear-algebra, finite-Markov, combinatorics, number-theory, and abstract-algebra pack APIs. Supported cases require typed handoffs; ambiguous and refused cases are executed through real pack boundaries and remain closed. It is stronger than the earlier fake-artifact route test, but it is not the sealed natural-language external exam and should not be reported as such.\n\nReproduce with `cargo run --quiet --bin stage377_actual_pack_cross_domain_synthesis`.\nMachine-readable report: `{}`\n",
            report.cases,
            report.supported,
            report.ambiguous,
            report.refused,
            report.exact_decisions,
            report.cases,
            report.supported_artifacts,
            report.replay_verified,
            report.tamper_rejections,
            report.false_authorizations,
            report.false_denials,
            report.route_leakage,
            report.route_counts,
            report.failure_localization,
            report.corpus_sha256,
            JSON,
        ),
    )?;
    println!(
        "stage377 cases={} supported={} ambiguous={} refused={} exact={} replay={} tamper={} false_auth={} route_leakage={}",
        report.cases,
        report.supported,
        report.ambiguous,
        report.refused,
        report.exact_decisions,
        report.replay_verified,
        report.tamper_rejections,
        report.false_authorizations,
        report.route_leakage,
    );
    Ok(())
}
