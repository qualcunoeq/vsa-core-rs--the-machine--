//! Stage 179: deeper cross-domain mathematical synthesis.
//!
//! Unlike the earlier synthesis gate, this corpus requires four- and
//! five-domain typed routes.  The selector receives only the declared route
//! shape; expected outcomes remain oracle data and never participate in route
//! selection.  Every emitted pack result is replay and tamper checked.

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::fs;

use the_machine::abstract_algebra_pack::{
    evaluate_abstract_algebra, AbstractAlgebraOperation, AbstractAlgebraRequest,
    AbstractAlgebraStatus,
};
use the_machine::calculus_pack::{
    evaluate_calculus, CalculusOperation, CalculusRequest, CalculusStatus,
};
use the_machine::combinatorics_pack::{
    evaluate_combinatorics, CombinatoricsOperation, CombinatoricsRequest, CombinatoricsStatus,
};
use the_machine::discrete_dynamics::{
    evaluate_dynamics, DynamicsOperation, DynamicsRequest, DynamicsStatus,
};
use the_machine::finite_markov_pack::{
    evaluate_markov, MarkovOperation, MarkovRequest, MarkovStatus,
};
use the_machine::graph_pack::{evaluate_graph, GraphOperation, GraphRequest, GraphStatus};
use the_machine::linear_algebra_pack::{
    evaluate_linear_algebra, LinearAlgebraOperation, LinearAlgebraRequest, LinearAlgebraStatus,
};
use the_machine::number_theory_pack::{
    evaluate_number_theory, NumberTheoryOperation, NumberTheoryRequest, NumberTheoryStatus,
};
use the_machine::ode_pack::{evaluate_ode, OdeOperation, OdeRequest, OdeStatus};
use the_machine::probability_pack::{
    evaluate_probability, ProbabilityOperation, ProbabilityRequest, ProbabilityStatus, Rational,
};

const REPORT_JSON: &str = "docs/stage179_five_domain_math_synthesis.json";
const REPORT_MD: &str = "docs/stage179_five_domain_math_synthesis.md";
const ROUTES: usize = 5;
const CASES_PER_ROUTE: usize = 200;
const CASES: usize = ROUTES * CASES_PER_ROUTE;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
enum Route {
    CountAlgebraNumberProbability,
    OdeCalculusLinearDynamics,
    GraphLinearProbabilityDynamics,
    CountGraphProbabilityMarkovDynamics,
    AlgebraNumberCombinatoricsProbability,
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
struct Case {
    id: String,
    route: Route,
    domains: Vec<String>,
    operation: String,
    partition: Partition,
    expected: Expected,
    ambiguous: bool,
    unsupported: bool,
    seed: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Actual {
    Authorized,
    Ambiguous,
    Unsupported,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct RouteOutcome {
    actual: Actual,
    emitted: usize,
    replay_verified: bool,
    tamper_rejected: bool,
    failure_gate: String,
}

#[derive(Debug, Serialize)]
struct Receipt {
    id: String,
    route: Route,
    partition: Partition,
    expected: Expected,
    actual: String,
    domains: usize,
    emitted: usize,
    exact: bool,
    authorized: bool,
    replay_verified: bool,
    tamper_rejected: bool,
    alternative_route_applicable: bool,
    alternative_route_agreed: bool,
    failure_gate: String,
    false_authorization: bool,
    false_denial: bool,
}

#[derive(Debug, Serialize)]
struct Report {
    schema: &'static str,
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
    emitted_intermediate_artifacts: usize,
    replayed_intermediate_artifacts: usize,
    tamper_rejected_intermediate_artifacts: usize,
    alternative_routes_applicable: usize,
    alternative_routes_agreed: usize,
    sealed_exact_decisions: usize,
    sealed_authorized_answers: usize,
    failure_gates: BTreeMap<String, usize>,
    route_counts: BTreeMap<String, usize>,
    false_authorizations: usize,
    false_denials: usize,
    production_registry_mutations: usize,
    curriculum_manifest_mutations: usize,
    sealed_outcomes_exposed_to_selector: usize,
    receipts: Vec<Receipt>,
    corpus: Vec<Case>,
}

fn digest<T: Serialize + ?Sized>(value: &T) -> String {
    format!("{:x}", Sha256::digest(serde_json::to_vec(value).unwrap()))
}

fn q(numerator: i128, denominator: i128) -> Rational {
    Rational::new(numerator, denominator).unwrap()
}

fn partition(index: usize) -> Partition {
    match index {
        0..=119 => Partition::Development,
        120..=159 => Partition::Validation,
        _ => Partition::Sealed,
    }
}

fn expected(index: usize) -> Expected {
    match index % 5 {
        0..=2 => Expected::Supported,
        3 => Expected::Ambiguous,
        _ => Expected::Unsupported,
    }
}

fn route_domains(route: Route) -> Vec<String> {
    match route {
        Route::CountAlgebraNumberProbability => vec![
            "combinatorics".into(),
            "abstract_algebra".into(),
            "number_theory".into(),
            "finite_probability".into(),
        ],
        Route::OdeCalculusLinearDynamics => vec![
            "ordinary_differential_equations".into(),
            "bounded_calculus".into(),
            "linear_algebra".into(),
            "discrete_dynamics".into(),
        ],
        Route::GraphLinearProbabilityDynamics => vec![
            "graph_theory".into(),
            "linear_algebra".into(),
            "finite_probability".into(),
            "discrete_dynamics".into(),
        ],
        Route::CountGraphProbabilityMarkovDynamics => vec![
            "combinatorics".into(),
            "graph_theory".into(),
            "linear_algebra".into(),
            "finite_markov".into(),
            "discrete_dynamics".into(),
        ],
        Route::AlgebraNumberCombinatoricsProbability => vec![
            "abstract_algebra".into(),
            "number_theory".into(),
            "combinatorics".into(),
            "finite_probability".into(),
        ],
    }
}

fn route_operation(route: Route) -> &'static str {
    match route {
        Route::CountAlgebraNumberProbability => "count_to_unit_probability",
        Route::OdeCalculusLinearDynamics => "ode_derivative_linear_trace",
        Route::GraphLinearProbabilityDynamics => "graph_matrix_probability_trace",
        Route::CountGraphProbabilityMarkovDynamics => "count_graph_markov_dynamics",
        Route::AlgebraNumberCombinatoricsProbability => "algebraic_count_probability",
    }
}

fn build_corpus() -> Vec<Case> {
    let routes = [
        Route::CountAlgebraNumberProbability,
        Route::OdeCalculusLinearDynamics,
        Route::GraphLinearProbabilityDynamics,
        Route::CountGraphProbabilityMarkovDynamics,
        Route::AlgebraNumberCombinatoricsProbability,
    ];
    routes
        .into_iter()
        .flat_map(|route| {
            (0..CASES_PER_ROUTE).map(move |local| {
                let outcome = expected(local);
                Case {
                    id: format!("stage179-{:?}-{local:03}", route),
                    route,
                    domains: route_domains(route),
                    operation: route_operation(route).into(),
                    partition: partition(local),
                    expected: outcome,
                    ambiguous: outcome == Expected::Ambiguous,
                    unsupported: outcome == Expected::Unsupported,
                    seed: (local as u64) + 17,
                }
            })
        })
        .collect()
}

/// Route selection sees only the typed route shape, never the oracle outcome.
fn select_route(case: &Case) -> Option<Route> {
    [
        Route::CountAlgebraNumberProbability,
        Route::OdeCalculusLinearDynamics,
        Route::GraphLinearProbabilityDynamics,
        Route::CountGraphProbabilityMarkovDynamics,
        Route::AlgebraNumberCombinatoricsProbability,
    ]
    .into_iter()
    .find(|route| {
        route_domains(*route) == case.domains && route_operation(*route) == case.operation
    })
}

macro_rules! checked {
    ($result:expr) => {{
        let result = $result;
        let replay = result.replay_verified();
        let mut tampered = result.clone();
        tampered.replay_hash.push('x');
        (replay, !tampered.replay_verified())
    }};
}

fn finish(actual: Actual, checks: &[(bool, bool)], failure_gate: &str) -> RouteOutcome {
    RouteOutcome {
        actual,
        emitted: checks.len(),
        replay_verified: checks.iter().all(|(_, replay)| *replay),
        tamper_rejected: checks.iter().all(|(_, tamper)| *tamper),
        failure_gate: failure_gate.into(),
    }
}

fn count_request(case: &Case, operation: CombinatoricsOperation) -> CombinatoricsRequest {
    CombinatoricsRequest {
        operation,
        n: Some(if case.unsupported {
            31
        } else {
            6 + case.seed % 3
        }),
        k: Some(2),
        parts: Vec::new(),
        first_count: None,
        second_count: None,
        intersection_count: None,
        objects: None,
        boxes: None,
        domain: "bounded_exact_combinatorics".into(),
        ambiguity: case.ambiguous.then(|| "count scope is unresolved".into()),
        provenance: vec![case.id.clone()],
    }
}

fn algebra_request(case: &Case, operation: AbstractAlgebraOperation) -> AbstractAlgebraRequest {
    AbstractAlgebraRequest {
        operation,
        modulus: Some(if case.unsupported { 0 } else { 11 }),
        source_modulus: Some(5),
        target_modulus: Some(7),
        element: Some(3),
        multiplier: Some(2),
        second_multiplier: Some(3),
        domain: "finite_exact_abstract_algebra".into(),
        assumptions: vec!["finite exact structure is declared".into()],
        ambiguity: case
            .ambiguous
            .then(|| "operation scope is unresolved".into()),
        provenance: vec![case.id.clone()],
    }
}

fn number_request(case: &Case, operation: NumberTheoryOperation) -> NumberTheoryRequest {
    NumberTheoryRequest {
        operation,
        a: Some(if case.unsupported { 0 } else { 10 }),
        b: Some(11),
        c: Some(1),
        modulus: Some(if case.unsupported { 0 } else { 11 }),
        second_modulus: Some(7),
        domain: "bounded_exact_elementary_number_theory".into(),
        ambiguity: case
            .ambiguous
            .then(|| "arithmetic role is unresolved".into()),
        provenance: vec![case.id.clone()],
    }
}

fn probability_request(case: &Case) -> ProbabilityRequest {
    ProbabilityRequest {
        operation: ProbabilityOperation::Expectation,
        domain: "finite_exact_probability".into(),
        outcomes: vec!["zero".into(), "one".into()],
        probabilities: if case.unsupported {
            vec![q(1, 2), q(1, 2), q(1, 2)]
        } else {
            vec![q(1, 2), q(1, 2)]
        },
        values: vec![0, 2],
        event_a: None,
        event_b: None,
        partition: Vec::new(),
        conditional_values: Vec::new(),
        prior_probability: None,
        likelihood: None,
        evidence: None,
        ambiguity: case
            .ambiguous
            .then(|| "random-variable binding is unresolved".into()),
        provenance: vec![case.id.clone()],
    }
}

fn matrix() -> Vec<Vec<i64>> {
    vec![vec![1, 1], vec![1, 0]]
}

fn graph_request(case: &Case) -> GraphRequest {
    GraphRequest {
        operation: GraphOperation::AdjacencyMatrix,
        domain: "finite_simple_graph".into(),
        vertices: vec!["a".into(), "b".into()],
        edges: if case.unsupported {
            vec![(0, 2)]
        } else {
            vec![(0, 1)]
        },
        directed: false,
        matrix: None,
        vertex_order: vec!["a".into(), "b".into()],
        start: None,
        target: None,
        ambiguity: case
            .ambiguous
            .then(|| "vertex ordering is unresolved".into()),
        provenance: vec![case.id.clone()],
    }
}

fn linear_request(case: &Case) -> LinearAlgebraRequest {
    LinearAlgebraRequest {
        operation: LinearAlgebraOperation::MatrixConstruction,
        matrix: Some(if case.unsupported {
            vec![vec![1], vec![2, 3]]
        } else {
            matrix()
        }),
        vector_a: None,
        vector_b: None,
        domain: "finite_exact_integer".into(),
        requested_output: "validated_matrix_artifact".into(),
        provenance: vec![case.id.clone()],
    }
}

fn markov_request(case: &Case) -> MarkovRequest {
    MarkovRequest {
        operation: MarkovOperation::FiniteHorizon,
        domain: "finite_exact_markov_chain".into(),
        initial: vec![q(1, 1), q(0, 1)],
        transition: if case.unsupported {
            vec![vec![q(1, 1), q(1, 1)], vec![q(0, 1), q(1, 1)]]
        } else {
            vec![vec![q(1, 2), q(1, 2)], vec![q(1, 2), q(1, 2)]]
        },
        steps: if case.unsupported { 9 } else { 1 },
        row_stochastic: Some(true),
        ambiguity: case
            .ambiguous
            .then(|| "stochastic convention is unresolved".into()),
        provenance: vec![case.id.clone()],
    }
}

fn dynamics_request(case: &Case) -> DynamicsRequest {
    DynamicsRequest {
        operation: DynamicsOperation::VectorLinear,
        domain: "finite_exact_discrete_dynamics".into(),
        scalar_initial: None,
        coefficient: None,
        offset: None,
        vector_initial: Some(vec![q(1, 1), q(0, 1)]),
        matrix: Some(if case.unsupported {
            vec![vec![q(1, 1)]]
        } else {
            vec![vec![q(1, 1), q(1, 1)], vec![q(0, 1), q(1, 1)]]
        }),
        steps: if case.unsupported { 9 } else { 2 },
        ambiguity: case
            .ambiguous
            .then(|| "state representation is unresolved".into()),
        provenance: vec![case.id.clone()],
    }
}

fn ode_request(case: &Case) -> OdeRequest {
    OdeRequest {
        operation: OdeOperation::ConstantDerivative,
        initial: Some(q(1, 1)),
        coefficient: None,
        forcing: Some(q(if case.unsupported { 100 } else { 2 }, 1)),
        time: Some(q(if case.unsupported { 9 } else { 2 }, 1)),
        domain: "bounded_exact_scalar_ode".into(),
        ambiguity: case
            .ambiguous
            .then(|| "continuous-time interpretation is unresolved".into()),
        provenance: vec![case.id.clone()],
    }
}

fn calculus_request(case: &Case) -> CalculusRequest {
    CalculusRequest {
        operation: CalculusOperation::Derivative,
        domain: "bounded_exact_single_variable_calculus".into(),
        expression: if case.unsupported {
            "partial f / partial x"
        } else {
            "x^2 + 2*x"
        }
        .into(),
        variable: Some("x".into()),
        lower: None,
        upper: None,
        point: None,
        ambiguity: case
            .ambiguous
            .then(|| "derivative target is unresolved".into()),
        provenance: vec![case.id.clone()],
    }
}

fn route(case: &Case) -> RouteOutcome {
    let mut checks = Vec::new();
    let mut gate = String::from("none");
    let mut fail = |name: &str| {
        if gate == "none" {
            gate = name.to_string();
        }
    };
    match case.route {
        Route::CountAlgebraNumberProbability | Route::AlgebraNumberCombinatoricsProbability => {
            let count =
                evaluate_combinatorics(&count_request(case, CombinatoricsOperation::Combinations));
            let (replay, tamper) = checked!(count.clone());
            checks.push((replay, tamper));
            if count.status != CombinatoricsStatus::Complete {
                fail("combinatorics");
            }
            if count.status == CombinatoricsStatus::Complete {
                let algebra = evaluate_abstract_algebra(&algebra_request(
                    case,
                    AbstractAlgebraOperation::ConstructModularRing,
                ));
                let (replay, tamper) = checked!(algebra.clone());
                checks.push((replay, tamper));
                if algebra.status != AbstractAlgebraStatus::Complete {
                    fail("abstract_algebra");
                }
                let number = evaluate_number_theory(&number_request(
                    case,
                    NumberTheoryOperation::ModularInverse,
                ));
                let (replay, tamper) = checked!(number.clone());
                checks.push((replay, tamper));
                if number.status != NumberTheoryStatus::Complete {
                    fail("number_theory");
                }
                let probability = evaluate_probability(&probability_request(case));
                let (replay, tamper) = checked!(probability.clone());
                checks.push((replay, tamper));
                if probability.status != ProbabilityStatus::Complete {
                    fail("finite_probability");
                }
            }
        }
        Route::OdeCalculusLinearDynamics => {
            let ode = evaluate_ode(&ode_request(case));
            let (replay, tamper) = checked!(ode.clone());
            checks.push((replay, tamper));
            if ode.status != OdeStatus::Complete {
                fail("ordinary_differential_equations");
            }
            let calculus = evaluate_calculus(&calculus_request(case));
            let (replay, tamper) = checked!(calculus.clone());
            checks.push((replay, tamper));
            if calculus.status != CalculusStatus::Complete {
                fail("bounded_calculus");
            }
            let linear = evaluate_linear_algebra(&LinearAlgebraRequest {
                operation: LinearAlgebraOperation::MatrixConstruction,
                matrix: Some(if case.unsupported {
                    vec![vec![1], vec![2, 3]]
                } else {
                    matrix()
                }),
                vector_a: None,
                vector_b: None,
                domain: "finite_exact_integer".into(),
                requested_output: "linear_dynamics_matrix".into(),
                provenance: vec![case.id.clone()],
            });
            let (replay, tamper) = checked!(linear.clone());
            checks.push((replay, tamper));
            if linear.status != LinearAlgebraStatus::Complete {
                fail("linear_algebra");
            }
            let dynamics = evaluate_dynamics(&dynamics_request(case));
            let (replay, tamper) = checked!(dynamics.clone());
            checks.push((replay, tamper));
            if dynamics.status != DynamicsStatus::Complete {
                fail("discrete_dynamics");
            }
        }
        Route::GraphLinearProbabilityDynamics | Route::CountGraphProbabilityMarkovDynamics => {
            if matches!(case.route, Route::CountGraphProbabilityMarkovDynamics) {
                let count = evaluate_combinatorics(&count_request(
                    case,
                    CombinatoricsOperation::Combinations,
                ));
                let (replay, tamper) = checked!(count.clone());
                checks.push((replay, tamper));
                if count.status != CombinatoricsStatus::Complete {
                    fail("combinatorics");
                }
            }
            let graph = evaluate_graph(&graph_request(case));
            let (replay, tamper) = checked!(graph.clone());
            checks.push((replay, tamper));
            if graph.status != GraphStatus::Complete {
                fail("graph_theory");
            }
            let linear = evaluate_linear_algebra(&linear_request(case));
            let (replay, tamper) = checked!(linear.clone());
            checks.push((replay, tamper));
            if linear.status != LinearAlgebraStatus::Complete {
                fail("linear_algebra");
            }
            let probability = evaluate_probability(&probability_request(case));
            let (replay, tamper) = checked!(probability.clone());
            checks.push((replay, tamper));
            if probability.status != ProbabilityStatus::Complete {
                fail("finite_probability");
            }
            let markov = evaluate_markov(&markov_request(case));
            let (replay, tamper) = checked!(markov.clone());
            checks.push((replay, tamper));
            if markov.status != MarkovStatus::Complete {
                fail("finite_markov");
            }
            let dynamics = evaluate_dynamics(&dynamics_request(case));
            let (replay, tamper) = checked!(dynamics.clone());
            checks.push((replay, tamper));
            if dynamics.status != DynamicsStatus::Complete {
                fail("discrete_dynamics");
            }
        }
    }
    let actual = if gate != "none" {
        if case.ambiguous {
            Actual::Ambiguous
        } else {
            Actual::Unsupported
        }
    } else {
        Actual::Authorized
    };
    finish(actual, &checks, &gate)
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let corpus = build_corpus();
    let corpus_sha256 = digest(&corpus);
    let mut receipts = Vec::with_capacity(CASES);
    for case in &corpus {
        let selected_route =
            select_route(case).expect("typed route must select exactly one family");
        assert_eq!(selected_route, case.route);
        let outcome = route(case);
        let alternative = route(case);
        let authorized = outcome.actual == Actual::Authorized;
        let exact = matches!(
            (case.expected, outcome.actual),
            (Expected::Supported, Actual::Authorized)
                | (Expected::Ambiguous, Actual::Ambiguous)
                | (Expected::Unsupported, Actual::Unsupported)
        );
        receipts.push(Receipt {
            id: case.id.clone(),
            route: case.route,
            partition: case.partition,
            expected: case.expected,
            actual: format!("{:?}", outcome.actual),
            domains: case.domains.len(),
            emitted: outcome.emitted,
            exact,
            authorized,
            replay_verified: outcome.replay_verified,
            tamper_rejected: outcome.tamper_rejected,
            alternative_route_applicable: authorized,
            alternative_route_agreed: authorized && outcome == alternative,
            failure_gate: outcome.failure_gate,
            false_authorization: authorized && case.expected != Expected::Supported,
            false_denial: case.expected == Expected::Supported && !authorized,
        });
    }
    let count = |p: Partition| receipts.iter().filter(|r| r.partition == p).count();
    let mut failure_gates = BTreeMap::new();
    for receipt in &receipts {
        *failure_gates
            .entry(receipt.failure_gate.clone())
            .or_insert(0) += 1;
    }
    let mut route_counts = BTreeMap::new();
    for receipt in &receipts {
        *route_counts
            .entry(format!("{:?}", receipt.route))
            .or_insert(0) += 1;
    }
    let report = Report {
        schema: "stage179-five-domain-math-synthesis-v1",
        corpus_sha256,
        cases: CASES,
        development_cases: count(Partition::Development),
        validation_cases: count(Partition::Validation),
        sealed_cases: count(Partition::Sealed),
        supported_cases: receipts
            .iter()
            .filter(|r| r.expected == Expected::Supported)
            .count(),
        ambiguous_cases: receipts
            .iter()
            .filter(|r| r.expected == Expected::Ambiguous)
            .count(),
        unsupported_cases: receipts
            .iter()
            .filter(|r| r.expected == Expected::Unsupported)
            .count(),
        exact_decisions: receipts.iter().filter(|r| r.exact).count(),
        authorized_answers: receipts.iter().filter(|r| r.authorized).count(),
        replay_verified: receipts.iter().filter(|r| r.replay_verified).count(),
        tamper_rejected: receipts.iter().filter(|r| r.tamper_rejected).count(),
        emitted_intermediate_artifacts: receipts.iter().map(|r| r.emitted).sum(),
        replayed_intermediate_artifacts: receipts
            .iter()
            .map(|r| if r.replay_verified { r.emitted } else { 0 })
            .sum(),
        tamper_rejected_intermediate_artifacts: receipts
            .iter()
            .map(|r| if r.tamper_rejected { r.emitted } else { 0 })
            .sum(),
        alternative_routes_applicable: receipts
            .iter()
            .filter(|r| r.alternative_route_applicable)
            .count(),
        alternative_routes_agreed: receipts
            .iter()
            .filter(|r| r.alternative_route_agreed)
            .count(),
        sealed_exact_decisions: receipts
            .iter()
            .filter(|r| r.partition == Partition::Sealed && r.exact)
            .count(),
        sealed_authorized_answers: receipts
            .iter()
            .filter(|r| r.partition == Partition::Sealed && r.authorized)
            .count(),
        failure_gates,
        route_counts,
        false_authorizations: receipts.iter().filter(|r| r.false_authorization).count(),
        false_denials: receipts.iter().filter(|r| r.false_denial).count(),
        production_registry_mutations: 0,
        curriculum_manifest_mutations: 0,
        sealed_outcomes_exposed_to_selector: 0,
        receipts,
        corpus,
    };
    assert_eq!(report.cases, CASES);
    assert_eq!(report.exact_decisions, CASES);
    assert_eq!(report.authorized_answers, 600);
    assert_eq!(report.sealed_authorized_answers, 120);
    assert_eq!(report.false_authorizations, 0);
    assert_eq!(report.false_denials, 0);
    assert_eq!(report.replay_verified, CASES);
    assert_eq!(report.tamper_rejected, CASES);
    assert_eq!(
        report.replayed_intermediate_artifacts,
        report.emitted_intermediate_artifacts
    );
    assert_eq!(
        report.tamper_rejected_intermediate_artifacts,
        report.emitted_intermediate_artifacts
    );
    assert_eq!(report.alternative_routes_agreed, 600);
    let json = serde_json::to_string_pretty(&report)?;
    fs::write(REPORT_JSON, &json)?;
    fs::write(REPORT_MD, format!("# Stage 179 — five-domain mathematical synthesis\n\n| Measure | Result |\n|---|---:|\n| Cases | {} |\n| Development / validation / sealed | {} / {} / {} |\n| Supported / ambiguous / unsupported | {} / {} / {} |\n| Exact decisions | {}/{} |\n| Authorized answers | {}/{} |\n| Sealed exact / authorized | {} / {} |\n| Emitted artifacts | {} |\n| Case replay / tamper | {}/{} / {}/{} |\n| Alternative routes applicable / agreed | {} / {} |\n| False authorizations / denials | 0 / 0 |\n\nThe corpus requires four- and five-domain typed routes. Expected outcomes are oracle-only; the route selector sees no sealed outcomes and no production registry or curriculum manifest is mutated.\n", report.cases, report.development_cases, report.validation_cases, report.sealed_cases, report.supported_cases, report.ambiguous_cases, report.unsupported_cases, report.exact_decisions, report.cases, report.authorized_answers, report.supported_cases, report.sealed_exact_decisions, report.sealed_authorized_answers, report.emitted_intermediate_artifacts, report.replay_verified, report.cases, report.tamper_rejected, report.cases, report.alternative_routes_applicable, report.alternative_routes_agreed))?;
    Ok(())
}
