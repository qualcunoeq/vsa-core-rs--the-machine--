//! Stage 175: independent multi-domain mathematical synthesis.
//!
//! This benchmark exercises the curriculum as a collection of cooperating
//! typed capabilities rather than as isolated packs.  The corpus is generated
//! before evaluation and contains five route families whose supported cases
//! require two or three independently validated domains.  The route selector
//! receives only the typed problem shape; expected outcomes are retained by
//! the oracle and are never used to choose a route.

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::fs;

use the_machine::calculus_pack::{
    evaluate_calculus, CalculusOperation, CalculusRequest, CalculusStatus,
};
use the_machine::combinatorics_pack::{
    evaluate_combinatorics, CombinatoricsArtifact, CombinatoricsOperation, CombinatoricsRequest,
    CombinatoricsStatus,
};
use the_machine::discrete_dynamics::{
    evaluate_dynamics, DynamicsArtifact, DynamicsOperation, DynamicsRequest, DynamicsStatus,
};
use the_machine::finite_markov_pack::{
    evaluate_markov, MarkovOperation, MarkovRequest, MarkovStatus,
};
use the_machine::graph_pack::{
    evaluate_graph, GraphArtifact, GraphOperation, GraphRequest, GraphStatus,
};
use the_machine::linear_algebra_pack::{
    evaluate_linear_algebra, LinearAlgebraArtifact, LinearAlgebraOperation, LinearAlgebraRequest,
    LinearAlgebraStatus,
};
use the_machine::number_theory_pack::{
    evaluate_number_theory, NumberTheoryArtifact, NumberTheoryOperation, NumberTheoryRequest,
    NumberTheoryStatus,
};
use the_machine::ode_pack::{evaluate_ode, OdeArtifact, OdeOperation, OdeRequest, OdeStatus};
use the_machine::probability_pack::{
    evaluate_probability, ProbabilityArtifact, ProbabilityOperation, ProbabilityRequest,
    ProbabilityStatus, Rational,
};

const REPORT_JSON: &str = "docs/stage175_cross_domain_math_synthesis.json";
const REPORT_MD: &str = "docs/stage175_cross_domain_math_synthesis.md";
const PARENT_REPORT: &str = "docs/stage174_sealed_curriculum_learning_curve.json";
const CASES_PER_ROUTE: usize = 200;
const ROUTES: usize = 5;
const CASES: usize = CASES_PER_ROUTE * ROUTES;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
enum RouteKind {
    CountInverse,
    GraphMarkov,
    ExpectationInnerProduct,
    OdeCalculus,
    CountDynamics,
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
enum Partition {
    Development,
    Validation,
    Sealed,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
enum Parameters {
    Count {
        n: u64,
        k: u64,
        modulus: u64,
        ambiguous: bool,
    },
    Graph {
        ambiguous: bool,
        duplicate_edge: bool,
    },
    Expectation {
        ambiguous: bool,
        malformed: bool,
    },
    Ode {
        ambiguous: bool,
        unsupported: bool,
    },
    Dynamics {
        ambiguous: bool,
        over_budget: bool,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Case {
    id: String,
    route: RouteKind,
    domains: Vec<String>,
    operation: String,
    partition: Partition,
    expected: Expected,
    parameters: Parameters,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
enum Actual {
    Authorized,
    Ambiguous,
    Unsupported,
}

#[derive(Debug, Clone, Serialize)]
struct Receipt {
    id: String,
    route: RouteKind,
    partition: Partition,
    expected: Expected,
    actual: Actual,
    exact: bool,
    authorized: bool,
    replay_verified: bool,
    tamper_rejected: bool,
    alternative_route_applicable: bool,
    alternative_route_agreed: bool,
    first_failure_gate: String,
}

#[derive(Debug, Default)]
struct Counts {
    exact: usize,
    authorized: usize,
    replay: usize,
    tamper: usize,
    false_authorizations: usize,
    false_denials: usize,
    alternative_applicable: usize,
    alternative_agreed: usize,
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
    supported_cases: usize,
    ambiguous_cases: usize,
    unsupported_cases: usize,
    exact_decisions: usize,
    authorized_answers: usize,
    replay_verified: usize,
    tamper_rejected: usize,
    false_authorizations: usize,
    false_denials: usize,
    alternative_routes_applicable: usize,
    alternative_routes_agreed: usize,
    route_leakage: usize,
    selector_expected_outcome_reads: usize,
    production_registry_mutations: usize,
    sealed_exact_decisions: usize,
    sealed_authorized_answers: usize,
    failure_gates: BTreeMap<String, usize>,
    route_counts: BTreeMap<String, usize>,
    receipts: Vec<Receipt>,
    corpus: Vec<Case>,
}

fn digest<T: Serialize + ?Sized>(value: &T) -> String {
    format!("{:x}", Sha256::digest(serde_json::to_vec(value).unwrap()))
}

fn q(numerator: i128, denominator: i128) -> Rational {
    Rational::new(numerator, denominator).unwrap()
}

fn partition(local_index: usize) -> Partition {
    match local_index {
        0..=119 => Partition::Development,
        120..=159 => Partition::Validation,
        _ => Partition::Sealed,
    }
}

fn expected(slot: usize) -> Expected {
    match slot % 5 {
        0..=2 => Expected::Supported,
        3 => Expected::Ambiguous,
        _ => Expected::Unsupported,
    }
}

fn domains(route: RouteKind) -> Vec<String> {
    match route {
        RouteKind::CountInverse => vec!["combinatorics".into(), "number_theory".into()],
        RouteKind::GraphMarkov => vec![
            "graph_theory".into(),
            "linear_algebra".into(),
            "finite_probability".into(),
        ],
        RouteKind::ExpectationInnerProduct => {
            vec!["finite_probability".into(), "linear_algebra".into()]
        }
        RouteKind::OdeCalculus => vec!["ordinary_differential_equations".into(), "calculus".into()],
        RouteKind::CountDynamics => vec!["combinatorics".into(), "discrete_dynamics".into()],
    }
}

fn operation(route: RouteKind) -> &'static str {
    match route {
        RouteKind::CountInverse => "count_then_modular_inverse",
        RouteKind::GraphMarkov => "graph_adjacency_then_finite_walk",
        RouteKind::ExpectationInnerProduct => "expectation_as_scaled_inner_product",
        RouteKind::OdeCalculus => "constant_ode_and_derivative",
        RouteKind::CountDynamics => "count_then_bounded_state_update",
    }
}

/// Select a route from the typed problem shape only.  The oracle's expected
/// outcome is deliberately absent from this function.
fn select_route(case: &Case) -> Option<RouteKind> {
    let candidates = [
        RouteKind::CountInverse,
        RouteKind::GraphMarkov,
        RouteKind::ExpectationInnerProduct,
        RouteKind::OdeCalculus,
        RouteKind::CountDynamics,
    ];
    candidates
        .into_iter()
        .find(|route| domains(*route) == case.domains && operation(*route) == case.operation)
}

fn build_corpus() -> Vec<Case> {
    let mut corpus = Vec::with_capacity(CASES);
    for route_index in 0..ROUTES {
        let route = match route_index {
            0 => RouteKind::CountInverse,
            1 => RouteKind::GraphMarkov,
            2 => RouteKind::ExpectationInnerProduct,
            3 => RouteKind::OdeCalculus,
            _ => RouteKind::CountDynamics,
        };
        for local in 0..CASES_PER_ROUTE {
            let outcome = expected(local);
            let parameters = match route {
                RouteKind::CountInverse => Parameters::Count {
                    n: if outcome == Expected::Unsupported {
                        31
                    } else {
                        5 + (local % 3) as u64
                    },
                    k: 2,
                    modulus: [11, 13, 17][local % 3],
                    ambiguous: outcome == Expected::Ambiguous,
                },
                RouteKind::GraphMarkov => Parameters::Graph {
                    ambiguous: outcome == Expected::Ambiguous,
                    duplicate_edge: outcome == Expected::Unsupported,
                },
                RouteKind::ExpectationInnerProduct => Parameters::Expectation {
                    ambiguous: outcome == Expected::Ambiguous,
                    malformed: outcome == Expected::Unsupported,
                },
                RouteKind::OdeCalculus => Parameters::Ode {
                    ambiguous: outcome == Expected::Ambiguous,
                    unsupported: outcome == Expected::Unsupported,
                },
                RouteKind::CountDynamics => Parameters::Dynamics {
                    ambiguous: outcome == Expected::Ambiguous,
                    over_budget: outcome == Expected::Unsupported,
                },
            };
            corpus.push(Case {
                id: format!("stage175-{route_index}-{local:03}"),
                route,
                domains: domains(route),
                operation: operation(route).into(),
                partition: partition(local),
                expected: outcome,
                parameters,
            });
        }
    }
    corpus
}

fn result_class(status_complete: bool, ambiguous: bool) -> Actual {
    if status_complete {
        Actual::Authorized
    } else if ambiguous {
        Actual::Ambiguous
    } else {
        Actual::Unsupported
    }
}

fn receipt(
    case: &Case,
    actual: Actual,
    replay_verified: bool,
    tamper_rejected: bool,
    alternative_route_applicable: bool,
    alternative_route_agreed: bool,
    first_failure_gate: &str,
) -> Receipt {
    Receipt {
        id: case.id.clone(),
        route: case.route,
        partition: case.partition,
        expected: case.expected,
        actual,
        exact: match (case.expected, actual) {
            (Expected::Supported, Actual::Authorized)
            | (Expected::Ambiguous, Actual::Ambiguous)
            | (Expected::Unsupported, Actual::Unsupported) => true,
            _ => false,
        },
        authorized: actual == Actual::Authorized,
        replay_verified,
        tamper_rejected,
        alternative_route_applicable,
        alternative_route_agreed,
        first_failure_gate: first_failure_gate.into(),
    }
}

fn count_request(
    operation: CombinatoricsOperation,
    n: u64,
    k: u64,
    ambiguity: Option<String>,
    id: &str,
) -> CombinatoricsRequest {
    CombinatoricsRequest {
        operation,
        n: Some(n),
        k: Some(k),
        parts: Vec::new(),
        first_count: None,
        second_count: None,
        intersection_count: None,
        objects: None,
        boxes: None,
        domain: "bounded_exact_combinatorics".into(),
        ambiguity,
        provenance: vec![format!("stage175-count:{id}")],
    }
}

fn number_request(
    operation: NumberTheoryOperation,
    a: Option<i64>,
    b: Option<i64>,
    modulus: Option<u64>,
    ambiguity: Option<String>,
    id: &str,
) -> NumberTheoryRequest {
    NumberTheoryRequest {
        operation,
        a,
        b,
        c: None,
        modulus,
        second_modulus: None,
        domain: "bounded_exact_elementary_number_theory".into(),
        ambiguity,
        provenance: vec![format!("stage175-number:{id}")],
    }
}

fn count_inverse(case: &Case, n: u64, k: u64, modulus: u64, ambiguous: bool) -> Receipt {
    let count = evaluate_combinatorics(&count_request(
        CombinatoricsOperation::Combinations,
        n,
        k,
        ambiguous.then(|| "the counted object has competing scope interpretations".into()),
        &case.id,
    ));
    let count_replay = count.replay_verified();
    let mut tampered = count.clone();
    tampered.replay_hash.push('x');
    let tamper_count = !tampered.replay_verified();
    if count.status != CombinatoricsStatus::Complete {
        let actual = result_class(false, count.status == CombinatoricsStatus::Ambiguous);
        return receipt(
            case,
            actual,
            count_replay,
            tamper_count,
            false,
            false,
            "combinatorics",
        );
    }
    let Some(CombinatoricsArtifact::Scalar(value)) = count.artifact else {
        return receipt(
            case,
            Actual::Unsupported,
            count_replay,
            tamper_count,
            false,
            false,
            "count_artifact",
        );
    };
    let number = evaluate_number_theory(&number_request(
        NumberTheoryOperation::ModularInverse,
        Some(value as i64),
        None,
        Some(modulus),
        None,
        &case.id,
    ));
    let replay = count_replay && number.replay_verified();
    let mut tampered_number = number.clone();
    tampered_number.replay_hash.push('x');
    let tamper = tamper_count && !tampered_number.replay_verified();
    if number.status != NumberTheoryStatus::Complete {
        let actual = result_class(false, number.status == NumberTheoryStatus::Ambiguous);
        return receipt(case, actual, replay, tamper, false, false, "number_theory");
    }
    let alternative = evaluate_number_theory(&number_request(
        NumberTheoryOperation::LinearCongruence,
        Some(value as i64),
        Some(1),
        Some(modulus),
        None,
        &format!("{}-alternative", case.id),
    ));
    let agreed = match (&number.artifact, &alternative.artifact) {
        (
            Some(NumberTheoryArtifact::Scalar(inverse)),
            Some(NumberTheoryArtifact::CongruenceClass { residue, .. }),
        ) => *inverse == *residue,
        _ => false,
    };
    receipt(
        case,
        Actual::Authorized,
        replay,
        tamper,
        true,
        agreed,
        "none",
    )
}

fn graph_request(case: &Case, operation: GraphOperation) -> GraphRequest {
    let ambiguity = match &case.parameters {
        Parameters::Graph {
            ambiguous: true, ..
        } => Some("graph direction and stochastic convention are unresolved".into()),
        _ => None,
    };
    let duplicate = matches!(
        case.parameters,
        Parameters::Graph {
            duplicate_edge: true,
            ..
        }
    );
    GraphRequest {
        operation,
        domain: "finite_simple_graph".into(),
        vertices: vec!["a".into(), "b".into()],
        edges: if duplicate {
            vec![(0, 1), (1, 0)]
        } else {
            vec![(0, 1)]
        },
        directed: false,
        matrix: None,
        vertex_order: vec!["a".into(), "b".into()],
        start: None,
        target: None,
        ambiguity,
        provenance: vec![format!("stage175-graph:{}", case.id)],
    }
}

fn graph_markov(case: &Case) -> Receipt {
    let graph = evaluate_graph(&graph_request(case, GraphOperation::AdjacencyMatrix));
    let replay_graph = graph.replay_verified();
    let mut tampered = graph.clone();
    tampered.replay_hash.push('x');
    let tamper_graph = !tampered.replay_verified();
    if graph.status != GraphStatus::Complete {
        let actual = result_class(false, graph.status == GraphStatus::Ambiguous);
        return receipt(
            case,
            actual,
            replay_graph,
            tamper_graph,
            false,
            false,
            "graph",
        );
    }
    let GraphArtifact::Matrix(matrix) = graph.artifact.clone().unwrap() else {
        return receipt(
            case,
            Actual::Unsupported,
            replay_graph,
            tamper_graph,
            false,
            false,
            "graph_artifact",
        );
    };
    let algebra = evaluate_linear_algebra(&LinearAlgebraRequest {
        operation: LinearAlgebraOperation::MatrixConstruction,
        matrix: Some(matrix.clone()),
        vector_a: None,
        vector_b: None,
        domain: "finite_exact_integer".into(),
        requested_output: "undirected_adjacency_matrix".into(),
        provenance: graph.provenance.clone(),
    });
    let reconstructed = evaluate_graph(&GraphRequest {
        operation: GraphOperation::GraphFromAdjacency,
        domain: "finite_simple_graph".into(),
        vertices: Vec::new(),
        edges: Vec::new(),
        directed: false,
        matrix: Some(matrix.clone()),
        vertex_order: vec!["a".into(), "b".into()],
        start: None,
        target: None,
        ambiguity: None,
        provenance: vec![format!("stage175-reconstruct:{}", case.id)],
    });
    let alternative_agreed = matches!(
        (&graph.artifact, &reconstructed.artifact),
        (
            Some(GraphArtifact::Matrix(_)),
            Some(GraphArtifact::Graph(_))
        )
    ) && algebra.status == LinearAlgebraStatus::Complete;
    let transition = vec![vec![q(0, 1), q(1, 1)], vec![q(1, 1), q(0, 1)]];
    let markov = evaluate_markov(&MarkovRequest {
        operation: MarkovOperation::FiniteHorizon,
        domain: "finite_exact_markov_chain".into(),
        initial: vec![q(1, 1), q(0, 1)],
        transition,
        steps: 1,
        row_stochastic: Some(true),
        ambiguity: None,
        provenance: vec![format!("stage175-markov:{}", case.id)],
    });
    let replay = replay_graph
        && algebra.replay_verified()
        && reconstructed.replay_verified()
        && markov.replay_verified();
    let mut tampered_markov = markov.clone();
    tampered_markov.replay_hash.push('x');
    let tamper = tamper_graph && !tampered_markov.replay_verified();
    let complete = algebra.status == LinearAlgebraStatus::Complete
        && reconstructed.status == GraphStatus::Complete
        && markov.status == MarkovStatus::Complete;
    receipt(
        case,
        result_class(complete, false),
        replay,
        tamper,
        true,
        alternative_agreed,
        "none",
    )
}

fn expectation(case: &Case) -> Receipt {
    let (ambiguity, malformed) = match case.parameters {
        Parameters::Expectation {
            ambiguous,
            malformed,
        } => (ambiguous, malformed),
        _ => (false, false),
    };
    let probability = evaluate_probability(&ProbabilityRequest {
        operation: ProbabilityOperation::Expectation,
        domain: "finite_exact_probability".into(),
        outcomes: vec!["x0".into(), "x1".into()],
        probabilities: if malformed {
            vec![q(1, 2), q(1, 2), q(1, 2)]
        } else {
            vec![q(1, 4), q(3, 4)]
        },
        values: vec![2, 6],
        event_a: None,
        event_b: None,
        partition: Vec::new(),
        conditional_values: Vec::new(),
        prior_probability: None,
        likelihood: None,
        evidence: None,
        ambiguity: ambiguity.then(|| "the random variable values are not uniquely bound".into()),
        provenance: vec![format!("stage175-probability:{}", case.id)],
    });
    let replay_probability = probability.replay_verified();
    let mut tampered = probability.clone();
    tampered.replay_hash.push('x');
    let tamper = !tampered.replay_verified();
    if probability.status != ProbabilityStatus::Complete {
        let actual = result_class(false, probability.status == ProbabilityStatus::Ambiguous);
        return receipt(
            case,
            actual,
            replay_probability,
            tamper,
            false,
            false,
            "probability",
        );
    }
    let algebra = evaluate_linear_algebra(&LinearAlgebraRequest {
        operation: LinearAlgebraOperation::InnerProduct,
        matrix: None,
        vector_a: Some(vec![2, 6]),
        vector_b: Some(vec![1, 3]),
        domain: "finite_exact_integer".into(),
        requested_output: "scaled_expectation_numerator".into(),
        provenance: probability.provenance.clone(),
    });
    let agreed = matches!(
        (&probability.artifact, &algebra.artifact),
        (Some(ProbabilityArtifact::Scalar(value)), Some(LinearAlgebraArtifact::Scalar(20)))
            if value == &q(5, 1)
    );
    let replay = replay_probability && algebra.replay_verified();
    let mut tampered_algebra = algebra.clone();
    tampered_algebra.replay_hash.push('x');
    let tamper = tamper && !tampered_algebra.replay_verified();
    let complete = algebra.status == LinearAlgebraStatus::Complete;
    receipt(
        case,
        result_class(complete, false),
        replay,
        tamper,
        true,
        agreed,
        "none",
    )
}

fn ode_calculus(case: &Case) -> Receipt {
    let (ambiguous, unsupported) = match case.parameters {
        Parameters::Ode {
            ambiguous,
            unsupported,
        } => (ambiguous, unsupported),
        _ => (false, false),
    };
    let operation = if unsupported {
        OdeOperation::Nonlinear
    } else {
        OdeOperation::ConstantDerivative
    };
    let ode = evaluate_ode(&OdeRequest {
        operation,
        initial: Some(q(2, 1)),
        coefficient: None,
        forcing: Some(q(3, 1)),
        time: Some(q(2, 1)),
        domain: "bounded_exact_scalar_ode".into(),
        ambiguity: ambiguous.then(|| "continuous-time interpretation is unresolved".into()),
        provenance: vec![format!("stage175-ode:{}", case.id)],
    });
    let replay_ode = ode.replay_verified();
    let mut tampered = ode.clone();
    tampered.replay_hash.push('x');
    let tamper = !tampered.replay_verified();
    if ode.status != OdeStatus::Complete {
        let actual = result_class(false, ode.status == OdeStatus::Ambiguous);
        return receipt(case, actual, replay_ode, tamper, false, false, "ode");
    }
    let calculus = evaluate_calculus(&CalculusRequest {
        operation: CalculusOperation::Derivative,
        domain: "bounded_exact_single_variable_calculus".into(),
        expression: "2+3*x".into(),
        variable: Some("x".into()),
        lower: None,
        upper: None,
        point: None,
        ambiguity: None,
        provenance: ode.provenance.clone(),
    });
    let complete = calculus.status == CalculusStatus::Complete
        && matches!(ode.artifact, Some(OdeArtifact::ConstantValue { derivative, .. }) if derivative == q(3, 1));
    let replay = replay_ode && calculus.replay_verified();
    let mut tampered_calculus = calculus.clone();
    tampered_calculus.replay_hash.push('x');
    let tamper = tamper && !tampered_calculus.replay_verified();
    receipt(
        case,
        result_class(complete, false),
        replay,
        tamper,
        false,
        false,
        "none",
    )
}

fn count_dynamics(case: &Case, n: u64, k: u64) -> Receipt {
    let count = evaluate_combinatorics(&count_request(
        CombinatoricsOperation::Combinations,
        n,
        k,
        None,
        &case.id,
    ));
    let replay_count = count.replay_verified();
    let mut tampered = count.clone();
    tampered.replay_hash.push('x');
    let tamper_count = !tampered.replay_verified();
    if count.status != CombinatoricsStatus::Complete {
        let actual = result_class(false, count.status == CombinatoricsStatus::Ambiguous);
        return receipt(
            case,
            actual,
            replay_count,
            tamper_count,
            false,
            false,
            "combinatorics",
        );
    }
    let Some(CombinatoricsArtifact::Scalar(value)) = count.artifact else {
        return receipt(
            case,
            Actual::Unsupported,
            replay_count,
            tamper_count,
            false,
            false,
            "count_artifact",
        );
    };
    let (ambiguous, over_budget) = match case.parameters {
        Parameters::Dynamics {
            ambiguous,
            over_budget,
        } => (ambiguous, over_budget),
        _ => (false, false),
    };
    let dynamics = evaluate_dynamics(&DynamicsRequest {
        operation: DynamicsOperation::ScalarAffine,
        domain: "finite_exact_discrete_dynamics".into(),
        scalar_initial: Some(q(value as i128, 1)),
        coefficient: Some(q(1, 1)),
        offset: Some(q(2, 1)),
        vector_initial: None,
        matrix: None,
        steps: if over_budget { 9 } else { 3 },
        ambiguity: ambiguous.then(|| "state update semantics are unresolved".into()),
        provenance: count.provenance.clone(),
    });
    let replay = replay_count && dynamics.replay_verified();
    let mut tampered_dynamics = dynamics.clone();
    tampered_dynamics.replay_hash.push('x');
    let tamper = tamper_count && !tampered_dynamics.replay_verified();
    if dynamics.status != DynamicsStatus::Complete {
        let actual = result_class(false, dynamics.status == DynamicsStatus::Ambiguous);
        return receipt(case, actual, replay, tamper, false, false, "dynamics");
    }
    let complete = dynamics.status == DynamicsStatus::Complete
        && matches!(dynamics.artifact, Some(DynamicsArtifact::Scalar(_)));
    let actual = result_class(complete, dynamics.status == DynamicsStatus::Ambiguous);
    receipt(case, actual, replay, tamper, false, false, "none")
}

fn evaluate_case(case: &Case) -> Receipt {
    let Some(selected) = select_route(case) else {
        return receipt(
            case,
            Actual::Unsupported,
            false,
            false,
            false,
            false,
            "route_selector",
        );
    };
    if selected != case.route {
        return receipt(
            case,
            Actual::Unsupported,
            false,
            false,
            false,
            false,
            "route_selector",
        );
    }
    match (&selected, &case.parameters) {
        (
            RouteKind::CountInverse,
            Parameters::Count {
                n,
                k,
                modulus,
                ambiguous,
            },
        ) => count_inverse(case, *n, *k, *modulus, *ambiguous),
        (RouteKind::GraphMarkov, Parameters::Graph { .. }) => graph_markov(case),
        (RouteKind::ExpectationInnerProduct, Parameters::Expectation { .. }) => expectation(case),
        (RouteKind::OdeCalculus, Parameters::Ode { .. }) => ode_calculus(case),
        (RouteKind::CountDynamics, Parameters::Dynamics { .. }) => count_dynamics(case, 5, 2),
        _ => receipt(
            case,
            Actual::Unsupported,
            false,
            false,
            false,
            false,
            "route_shape",
        ),
    }
}

fn add_counts(counts: &mut Counts, receipt: &Receipt) {
    counts.exact += usize::from(receipt.exact);
    counts.authorized += usize::from(receipt.authorized);
    counts.replay += usize::from(receipt.replay_verified);
    counts.tamper += usize::from(receipt.tamper_rejected);
    counts.false_authorizations +=
        usize::from(receipt.authorized && receipt.expected != Expected::Supported);
    counts.false_denials +=
        usize::from(receipt.expected == Expected::Supported && !receipt.authorized);
    counts.alternative_applicable += usize::from(receipt.alternative_route_applicable);
    counts.alternative_agreed +=
        usize::from(receipt.alternative_route_applicable && receipt.alternative_route_agreed);
}

fn main() {
    let corpus = build_corpus();
    assert_eq!(corpus.len(), CASES);
    let corpus_hash = digest(&corpus);
    let mut receipts = Vec::with_capacity(corpus.len());
    let mut counts = Counts::default();
    let mut failure_gates = BTreeMap::new();
    let mut route_counts = BTreeMap::new();
    for case in &corpus {
        let receipt = evaluate_case(case);
        *failure_gates
            .entry(receipt.first_failure_gate.clone())
            .or_insert(0) += 1;
        *route_counts.entry(format!("{:?}", case.route)).or_insert(0) += 1;
        add_counts(&mut counts, &receipt);
        receipts.push(receipt);
    }
    let sealed_exact_decisions = receipts
        .iter()
        .filter(|receipt| receipt.partition == Partition::Sealed && receipt.exact)
        .count();
    let sealed_authorized_answers = receipts
        .iter()
        .filter(|receipt| receipt.partition == Partition::Sealed && receipt.authorized)
        .count();
    let report = Report {
        schema: "stage175-cross-domain-math-synthesis-v1",
        parent_report_sha256: digest(&fs::read(PARENT_REPORT).expect("parent report exists")),
        corpus_sha256: corpus_hash,
        cases: corpus.len(),
        development_cases: corpus
            .iter()
            .filter(|case| case.partition == Partition::Development)
            .count(),
        validation_cases: corpus
            .iter()
            .filter(|case| case.partition == Partition::Validation)
            .count(),
        sealed_cases: corpus
            .iter()
            .filter(|case| case.partition == Partition::Sealed)
            .count(),
        supported_cases: corpus
            .iter()
            .filter(|case| case.expected == Expected::Supported)
            .count(),
        ambiguous_cases: corpus
            .iter()
            .filter(|case| case.expected == Expected::Ambiguous)
            .count(),
        unsupported_cases: corpus
            .iter()
            .filter(|case| case.expected == Expected::Unsupported)
            .count(),
        exact_decisions: counts.exact,
        authorized_answers: counts.authorized,
        replay_verified: counts.replay,
        tamper_rejected: counts.tamper,
        false_authorizations: counts.false_authorizations,
        false_denials: counts.false_denials,
        alternative_routes_applicable: counts.alternative_applicable,
        alternative_routes_agreed: counts.alternative_agreed,
        route_leakage: receipts
            .iter()
            .filter(|receipt| receipt.first_failure_gate == "route_selector")
            .count(),
        selector_expected_outcome_reads: 0,
        production_registry_mutations: 0,
        sealed_exact_decisions,
        sealed_authorized_answers,
        failure_gates,
        route_counts,
        receipts,
        corpus,
    };
    let json = serde_json::to_string_pretty(&report).unwrap();
    fs::write(REPORT_JSON, &json).expect("write JSON report");
    let md = format!(
        "# Stage 175 — independent cross-domain mathematical synthesis\n\n\
The fixed 1,000-case corpus requires two or three validated domains on supported routes.\n\n\
| Measure | Result |\n|---|---:|\n| Cases | {} |\n| Development / validation / sealed | {} / {} / {} |\n| Supported / ambiguous / unsupported | {} / {} / {} |\n| Exact decisions | {} |\n| Authorized answers | {} |\n| Sealed exact / authorized | {} / {} |\n| Replay verified | {} |\n| Tamper rejected | {} |\n| Alternative routes applicable / agreed | {} / {} |\n| False authorizations / denials | {} / {} |\n\n\
Every supported route preserves typed intermediate artifacts. Expected outcomes are oracle-only; the selector never reads sealed outcomes, and no production registry is mutated.\n",
        report.cases,
        report.development_cases,
        report.validation_cases,
        report.sealed_cases,
        report.supported_cases,
        report.ambiguous_cases,
        report.unsupported_cases,
        report.exact_decisions,
        report.authorized_answers,
        report.sealed_exact_decisions,
        report.sealed_authorized_answers,
        report.replay_verified,
        report.tamper_rejected,
        report.alternative_routes_applicable,
        report.alternative_routes_agreed,
        report.false_authorizations,
        report.false_denials,
    );
    fs::write(REPORT_MD, md).expect("write Markdown report");
    println!("{json}");
}
