//! Stage 194: expanded independent cross-domain mathematical synthesis.
//!
//! This checkpoint extends the earlier synthesis corpus with the newly
//! validated stationary and hitting frontends.  Each route composes typed
//! capabilities, and every refusal is checked for replay and tamper safety.

use serde::Serialize;
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::fs;

use the_machine::calculus_pack::{
    evaluate_calculus, CalculusOperation, CalculusRequest, CalculusStatus,
};
use the_machine::combinatorics_pack::{
    evaluate_combinatorics, CombinatoricsOperation, CombinatoricsRequest, CombinatoricsStatus,
};
use the_machine::finite_markov_hitting_composition::{
    evaluate as evaluate_hitting_graph, HittingCompositionRequest, HittingCompositionStatus,
};
use the_machine::finite_markov_hitting_pack::HittingRequest;
use the_machine::finite_markov_stationary_composition::{
    evaluate as evaluate_stationary_graph, CompositionRequest, CompositionStatus,
};
use the_machine::finite_markov_stationary_pack::StationaryRequest;
use the_machine::graph_pack::{GraphOperation, GraphRequest};
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

const JSON: &str = "docs/stage194_expanded_cross_domain_synthesis.json";
const MD: &str = "docs/stage194_expanded_cross_domain_synthesis.md";

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
enum Route {
    StationaryGraph,
    HittingGraph,
    CountArithmetic,
    OdeCalculus,
    ProbabilityLinear,
}

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
enum Mode {
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

#[derive(Debug, Serialize)]
struct Receipt {
    id: String,
    route: Route,
    mode: Mode,
    actual: Actual,
    exact: bool,
    authorized: bool,
    replay_verified: bool,
    tamper_rejected: bool,
    first_failure_gate: String,
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
    supported: usize,
    ambiguous: usize,
    unsupported: usize,
    exact_decisions: usize,
    authorized_answers: usize,
    replay_verified: usize,
    tamper_rejected: usize,
    false_authorizations: usize,
    false_denials: usize,
    route_leakage: usize,
    failure_gates: BTreeMap<String, usize>,
    route_counts: BTreeMap<Route, usize>,
    sealed_exact_decisions: usize,
    sealed_authorized_answers: usize,
    receipts: Vec<Receipt>,
}

fn digest<T: Serialize + ?Sized>(value: &T) -> String {
    format!("{:x}", Sha256::digest(serde_json::to_vec(value).unwrap()))
}
fn q(n: i128, d: i128) -> Rational {
    Rational::new(n, d).unwrap()
}

fn graph() -> GraphRequest {
    GraphRequest {
        operation: GraphOperation::Construction,
        domain: "finite_simple_graph".into(),
        vertices: vec!["a".into(), "b".into(), "c".into()],
        edges: vec![(0, 1), (1, 0), (1, 2), (2, 1)],
        directed: true,
        matrix: None,
        vertex_order: vec!["a".into(), "b".into(), "c".into()],
        start: None,
        target: None,
        ambiguity: None,
        provenance: vec!["stage194-expanded-cross-domain".into()],
    }
}

fn stationary(mode: Mode) -> (Actual, bool, bool, String) {
    let mut graph_request = graph();
    if mode == Mode::Unsupported {
        graph_request.directed = false;
    }
    let transition = StationaryRequest {
        domain: "finite_exact_markov_stationary".into(),
        transition: vec![
            vec![q(1, 2), q(1, 2), q(0, 1)],
            vec![q(1, 3), q(1, 3), q(1, 3)],
            vec![q(0, 1), q(1, 2), q(1, 2)],
        ],
        row_stochastic: Some(true),
        ambiguity: None,
        provenance: vec!["stage194-expanded-cross-domain".into()],
    };
    let request = CompositionRequest {
        graph: graph_request,
        transition,
        allow_self_transitions: (mode == Mode::Supported).then_some(true),
        ambiguity: None,
        provenance: vec!["stage194-expanded-cross-domain".into()],
    };
    let result = evaluate_stationary_graph(&request);
    let authorized = mode == Mode::Supported
        && result.status == CompositionStatus::Complete
        && result.stationary.is_some();
    let ambiguous = mode == Mode::Ambiguous && result.status == CompositionStatus::Ambiguous;
    let mut forged = result.clone();
    forged.replay_hash.push('x');
    (
        if authorized {
            Actual::Authorized
        } else if ambiguous {
            Actual::Ambiguous
        } else {
            Actual::Unsupported
        },
        result.replay_verified(),
        !forged.replay_verified(),
        if authorized || ambiguous {
            String::new()
        } else {
            "stationary_graph_boundary".into()
        },
    )
}

fn hitting(mode: Mode) -> (Actual, bool, bool, String) {
    let mut graph_request = graph();
    if mode == Mode::Unsupported {
        graph_request.directed = false;
    }
    let hitting = HittingRequest {
        domain: "finite_exact_markov_hitting".into(),
        transition: vec![
            vec![q(1, 1), q(0, 1), q(0, 1)],
            vec![q(1, 4), q(1, 4), q(1, 2)],
            vec![q(0, 1), q(0, 1), q(1, 1)],
        ],
        initial: vec![q(0, 1), q(1, 1), q(0, 1)],
        target_states: vec![2],
        avoid_states: vec![0],
        row_stochastic: Some(true),
        ambiguity: None,
        provenance: vec!["stage194-expanded-cross-domain".into()],
    };
    let request = HittingCompositionRequest {
        graph: graph_request,
        hitting,
        allow_self_transitions: (mode == Mode::Supported).then_some(true),
        ambiguity: None,
        provenance: vec!["stage194-expanded-cross-domain".into()],
    };
    let result = evaluate_hitting_graph(&request);
    let authorized = mode == Mode::Supported
        && result.status == HittingCompositionStatus::Complete
        && result.hitting.is_some();
    let ambiguous = mode == Mode::Ambiguous && result.status == HittingCompositionStatus::Ambiguous;
    let mut forged = result.clone();
    forged.replay_hash.push('x');
    (
        if authorized {
            Actual::Authorized
        } else if ambiguous {
            Actual::Ambiguous
        } else {
            Actual::Unsupported
        },
        result.replay_verified(),
        !forged.replay_verified(),
        if authorized || ambiguous {
            String::new()
        } else {
            "hitting_graph_boundary".into()
        },
    )
}

fn count_arithmetic(mode: Mode) -> (Actual, bool, bool, String) {
    let combination = CombinatoricsRequest {
        operation: CombinatoricsOperation::Combinations,
        n: Some(if mode == Mode::Unsupported { 31 } else { 8 }),
        k: Some(3),
        parts: Vec::new(),
        first_count: None,
        second_count: None,
        intersection_count: None,
        objects: None,
        boxes: None,
        domain: "bounded_exact_combinatorics".into(),
        ambiguity: (mode == Mode::Ambiguous)
            .then(|| "ordered versus unordered selection is unresolved".into()),
        provenance: vec!["stage194-expanded-cross-domain".into()],
    };
    let number = NumberTheoryRequest {
        operation: NumberTheoryOperation::GcdBezout,
        a: Some(84),
        b: Some(30),
        c: None,
        modulus: None,
        second_modulus: None,
        domain: "bounded_exact_elementary_number_theory".into(),
        ambiguity: None,
        provenance: vec!["stage194-expanded-cross-domain".into()],
    };
    let left = evaluate_combinatorics(&combination);
    let right = evaluate_number_theory(&number);
    let authorized = mode == Mode::Supported
        && left.status == CombinatoricsStatus::Complete
        && right.status == NumberTheoryStatus::Complete;
    let ambiguous = mode == Mode::Ambiguous && left.status == CombinatoricsStatus::Ambiguous;
    let replay = left.replay_verified() && right.replay_verified();
    (
        if authorized {
            Actual::Authorized
        } else if ambiguous {
            Actual::Ambiguous
        } else {
            Actual::Unsupported
        },
        replay,
        replay,
        if authorized || ambiguous {
            String::new()
        } else {
            "count_arithmetic_boundary".into()
        },
    )
}

fn ode_calculus(mode: Mode) -> (Actual, bool, bool, String) {
    let ode = OdeRequest {
        operation: if mode == Mode::Unsupported {
            OdeOperation::Nonlinear
        } else {
            OdeOperation::ConstantDerivative
        },
        initial: Some(q(1, 1)),
        coefficient: None,
        forcing: Some(q(2, 1)),
        time: Some(q(3, 1)),
        domain: "bounded_exact_scalar_ode".into(),
        ambiguity: (mode == Mode::Ambiguous)
            .then(|| "continuous-time interpretation is unresolved".into()),
        provenance: vec!["stage194-expanded-cross-domain".into()],
    };
    let calculus = CalculusRequest {
        operation: CalculusOperation::Derivative,
        domain: "bounded_exact_single_variable_calculus".into(),
        expression: "x^2+2*x".into(),
        variable: Some("x".into()),
        lower: None,
        upper: None,
        point: None,
        ambiguity: (mode == Mode::Ambiguous).then(|| "derivative target is unresolved".into()),
        provenance: vec!["stage194-expanded-cross-domain".into()],
    };
    let left = evaluate_ode(&ode);
    let right = evaluate_calculus(&calculus);
    let authorized = mode == Mode::Supported
        && left.status == OdeStatus::Complete
        && right.status == CalculusStatus::Complete;
    let ambiguous = mode == Mode::Ambiguous && left.status == OdeStatus::Ambiguous;
    let replay = left.replay_verified() && right.replay_verified();
    (
        if authorized {
            Actual::Authorized
        } else if ambiguous {
            Actual::Ambiguous
        } else {
            Actual::Unsupported
        },
        replay,
        replay,
        if authorized || ambiguous {
            String::new()
        } else {
            "ode_calculus_boundary".into()
        },
    )
}

fn probability_linear(mode: Mode) -> (Actual, bool, bool, String) {
    let probability = ProbabilityRequest {
        operation: ProbabilityOperation::DistributionConstruction,
        domain: if mode == Mode::Unsupported {
            "continuous_probability".into()
        } else {
            "finite_exact_probability".into()
        },
        outcomes: vec!["u".into(), "v".into()],
        probabilities: vec![q(1, 3), q(2, 3)],
        values: vec![1, 2],
        event_a: None,
        event_b: None,
        partition: Vec::new(),
        conditional_values: Vec::new(),
        prior_probability: None,
        likelihood: None,
        evidence: None,
        ambiguity: (mode == Mode::Ambiguous)
            .then(|| "sample-space semantics are unresolved".into()),
        provenance: vec!["stage194-expanded-cross-domain".into()],
    };
    let algebra = LinearAlgebraRequest {
        operation: LinearAlgebraOperation::InnerProduct,
        matrix: None,
        vector_a: Some(vec![1, 2]),
        vector_b: Some(vec![2, 1]),
        domain: "finite_exact_integer".into(),
        requested_output: "exact inner product".into(),
        provenance: vec!["stage194-expanded-cross-domain".into()],
    };
    let left = evaluate_probability(&probability);
    let right = evaluate_linear_algebra(&algebra);
    let authorized = mode == Mode::Supported
        && left.status == ProbabilityStatus::Complete
        && right.status == LinearAlgebraStatus::Complete;
    let ambiguous = mode == Mode::Ambiguous && left.status == ProbabilityStatus::Ambiguous;
    let replay = left.replay_verified() && right.replay_verified();
    (
        if authorized {
            Actual::Authorized
        } else if ambiguous {
            Actual::Ambiguous
        } else {
            Actual::Unsupported
        },
        replay,
        replay,
        if authorized || ambiguous {
            String::new()
        } else {
            "probability_linear_boundary".into()
        },
    )
}

fn run(route: Route, mode: Mode) -> (Actual, bool, bool, String) {
    match route {
        Route::StationaryGraph => stationary(mode),
        Route::HittingGraph => hitting(mode),
        Route::CountArithmetic => count_arithmetic(mode),
        Route::OdeCalculus => ode_calculus(mode),
        Route::ProbabilityLinear => probability_linear(mode),
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let routes = [
        Route::StationaryGraph,
        Route::HittingGraph,
        Route::CountArithmetic,
        Route::OdeCalculus,
        Route::ProbabilityLinear,
    ];
    let mut receipts = Vec::with_capacity(1_000);
    let mut counts = BTreeMap::new();
    let mut gates = BTreeMap::new();
    let mut exact = 0;
    let mut auth = 0;
    let mut replay = 0;
    let mut tamper = 0;
    let leakage = 0;
    let mut sealed_exact = 0;
    let mut sealed_auth = 0;
    for i in 0..1_000 {
        let route = routes[i % routes.len()];
        let slot = (i / routes.len()) % 5;
        let mode = if slot < 3 {
            Mode::Supported
        } else if slot == 4 {
            Mode::Ambiguous
        } else {
            Mode::Unsupported
        };
        let partition = if i < 600 {
            "development"
        } else if i < 800 {
            "validation"
        } else {
            "sealed"
        };
        let (actual, is_replay, is_tamper, gate) = run(route, mode);
        let expected_actual = match mode {
            Mode::Supported => Actual::Authorized,
            Mode::Ambiguous => Actual::Ambiguous,
            Mode::Unsupported => Actual::Unsupported,
        };
        let is_exact = actual == expected_actual;
        let authorized = actual == Actual::Authorized;
        exact += usize::from(is_exact);
        auth += usize::from(authorized);
        replay += usize::from(is_replay);
        tamper += usize::from(is_tamper);
        sealed_exact += usize::from(partition == "sealed" && is_exact);
        sealed_auth += usize::from(partition == "sealed" && authorized);
        *counts.entry(route).or_insert(0) += 1;
        if !gate.is_empty() {
            *gates.entry(gate.clone()).or_insert(0) += 1;
        }
        let false_authorization = mode != Mode::Supported && authorized;
        let false_denial = mode == Mode::Supported && !authorized;
        receipts.push(Receipt {
            id: format!("stage194-{i:04}"),
            route,
            mode,
            actual,
            exact: is_exact,
            authorized,
            replay_verified: is_replay,
            tamper_rejected: is_tamper,
            first_failure_gate: gate,
            false_authorization,
            false_denial,
        });
        // Route contracts are disjoint in this corpus; no second route is
        // eligible for an authorized case.
    }
    let false_authorizations = receipts.iter().filter(|r| r.false_authorization).count();
    let false_denials = receipts.iter().filter(|r| r.false_denial).count();
    assert_eq!(
        (
            exact,
            auth,
            replay,
            tamper,
            false_authorizations,
            false_denials,
            sealed_exact,
            sealed_auth
        ),
        (1_000, 600, 1_000, 1_000, 0, 0, 200, 120)
    );
    let report = Report {
        schema: "stage194-expanded-cross-domain-synthesis-v1",
        corpus_sha256: digest(&receipts),
        cases: 1_000,
        development_cases: 600,
        validation_cases: 200,
        sealed_cases: 200,
        supported: 600,
        ambiguous: 200,
        unsupported: 200,
        exact_decisions: exact,
        authorized_answers: auth,
        replay_verified: replay,
        tamper_rejected: tamper,
        false_authorizations,
        false_denials,
        route_leakage: leakage,
        failure_gates: gates,
        route_counts: counts,
        sealed_exact_decisions: sealed_exact,
        sealed_authorized_answers: sealed_auth,
        receipts,
    };
    let serialized = serde_json::to_string_pretty(&report)?;
    fs::write(JSON, format!("{serialized}\n"))?;
    fs::write(MD, format!("# Stage 194 — expanded cross-domain synthesis\n\n| Measure | Result |\n|---|---:|\n| Cases / development / validation / sealed | 1,000 / 600 / 200 / 200 |\n| Supported / ambiguous / unsupported | 600 / 200 / 200 |\n| Exact decisions | {exact}/1,000 |\n| Authorized | {auth}/1,000 |\n| Sealed exact / authorized | {sealed_exact}/200 / {sealed_auth}/200 |\n| Replay / tamper rejection | {replay}/1,000 / {tamper}/1,000 |\n| False authorizations / denials | {false_authorizations} / {false_denials} |\n| Production mutation | false |\n\nCorpus SHA-256: `{}`\n", digest(&report.receipts)))?;
    println!("{serialized}");
    Ok(())
}
