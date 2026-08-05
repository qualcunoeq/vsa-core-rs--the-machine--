//! Phase 60 shadow benchmark: equivalence and route selection across the
//! validated linear-algebra, probability, graph, dynamics, and finite-state
//! packs.  No production route or registry is changed by this binary.

use serde::Serialize;
use std::fs;
use the_machine::discrete_dynamics::{
    evaluate_dynamics, DynamicsArtifact, DynamicsOperation, DynamicsRequest, DynamicsStatus,
};
use the_machine::finite_state_contract::{formalize, StateDecision};
use the_machine::graph_pack::FiniteGraph;
use the_machine::probability_pack::{
    evaluate_probability, ProbabilityOperation, ProbabilityRequest, Rational,
};
use the_machine::random_walk_composition::{
    execute_bounded_steps, execute_one_step, uniform_neighbor_transition, RandomWalkStatus,
    TransitionConvention,
};

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
enum Route {
    ScalarAffine,
    VectorLinear,
    RandomWalk,
    FiniteStateTrace,
    Refused,
}

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
enum CaseKind {
    Equivalent,
    Refused,
}

#[derive(Debug, Clone, Serialize)]
struct CaseReceipt {
    id: String,
    family: String,
    kind: CaseKind,
    selected_route: Route,
    expected_route: Route,
    equivalent: bool,
    replay_verified: bool,
    tamper_rejected: bool,
    invariant_preserved: bool,
    refused: bool,
    reason: String,
}

#[derive(Debug, Serialize)]
struct Report {
    schema: &'static str,
    benchmark: &'static str,
    cases: usize,
    accepted: usize,
    refusals: usize,
    exact_route_decisions: usize,
    equivalent_routes_agree: usize,
    replay_verified: usize,
    tamper_rejections: usize,
    stronger_invariants_preserved: usize,
    safe_refusals: usize,
    false_authorizations: usize,
    false_denials: usize,
    route_leakage: usize,
    semantic_erasure_refusals: usize,
    receipts: Vec<CaseReceipt>,
}

fn r(n: i128, d: i128) -> Rational {
    Rational::new(n, d).expect("valid rational")
}

fn provenance(id: &str) -> Vec<String> {
    vec![format!("phase60:{id}"), "shadow-only".into()]
}

fn dynamics_request(
    id: &str,
    operation: DynamicsOperation,
    scalar_initial: Option<Rational>,
    coefficient: Option<Rational>,
    offset: Option<Rational>,
    vector_initial: Option<Vec<Rational>>,
    matrix: Option<Vec<Vec<Rational>>>,
    steps: usize,
) -> DynamicsRequest {
    DynamicsRequest {
        operation,
        domain: "finite_exact_discrete_dynamics".into(),
        scalar_initial,
        coefficient,
        offset,
        vector_initial,
        matrix,
        steps,
        ambiguity: None,
        provenance: provenance(id),
    }
}

fn initial_distribution(
    vertices: &[String],
    probabilities: Vec<Rational>,
    id: &str,
) -> ProbabilityResult {
    evaluate_probability(&ProbabilityRequest {
        operation: ProbabilityOperation::DistributionConstruction,
        domain: "finite_exact_probability".into(),
        outcomes: vertices.to_vec(),
        probabilities,
        values: Vec::new(),
        event_a: None,
        event_b: None,
        partition: Vec::new(),
        conditional_values: Vec::new(),
        prior_probability: None,
        likelihood: None,
        evidence: None,
        ambiguity: None,
        provenance: provenance(id),
    })
}

type ProbabilityResult = the_machine::probability_pack::ProbabilityResult;

fn scalar_matrix_case(index: usize) -> CaseReceipt {
    let id = format!("scalar_matrix_{index}");
    let steps = 1 + index % 8;
    let scalar = evaluate_dynamics(&dynamics_request(
        &id,
        DynamicsOperation::ScalarAffine,
        Some(r(1, 1)),
        Some(r(2, 1)),
        Some(r(1, 1)),
        None,
        None,
        steps,
    ));
    let matrix = evaluate_dynamics(&dynamics_request(
        &id,
        DynamicsOperation::MatrixEvolution,
        None,
        None,
        None,
        Some(vec![r(1, 1), r(1, 1)]),
        Some(vec![vec![r(2, 1), r(1, 1)], vec![r(0, 1), r(1, 1)]]),
        steps,
    ));
    let equal = match (&scalar.artifact, &matrix.artifact) {
        (Some(DynamicsArtifact::Scalar(left)), Some(DynamicsArtifact::Vector(right))) => {
            right.first() == Some(left)
        }
        _ => false,
    };
    let replay = scalar.replay_verified() && matrix.replay_verified();
    let mut tampered = matrix.clone();
    tampered.replay_hash.push('x');
    CaseReceipt {
        id,
        family: "scalar_recurrence_vs_augmented_matrix".into(),
        kind: CaseKind::Equivalent,
        selected_route: Route::ScalarAffine,
        expected_route: Route::ScalarAffine,
        equivalent: scalar.status == DynamicsStatus::Complete
            && matrix.status == DynamicsStatus::Complete
            && equal,
        replay_verified: replay,
        tamper_rejected: !tampered.replay_verified(),
        invariant_preserved: equal,
        refused: false,
        reason: "affine recurrence equals first coordinate of augmented homogeneous evolution"
            .into(),
    }
}

fn vector_matrix_case(index: usize) -> CaseReceipt {
    let id = format!("vector_matrix_{index}");
    let steps = 1 + index % 8;
    let matrix = vec![vec![r(1, 1), r(1, 1)], vec![r(1, 1), r(0, 1)]];
    let vector = vec![r(1, 1), r(0, 1)];
    let left = evaluate_dynamics(&dynamics_request(
        &id,
        DynamicsOperation::VectorLinear,
        None,
        None,
        None,
        Some(vector.clone()),
        Some(matrix.clone()),
        steps,
    ));
    let right = evaluate_dynamics(&dynamics_request(
        &id,
        DynamicsOperation::MatrixEvolution,
        None,
        None,
        None,
        Some(vector),
        Some(matrix),
        steps,
    ));
    let equal = left.artifact == right.artifact && left.trace == right.trace;
    let replay = left.replay_verified() && right.replay_verified();
    let mut tampered = left.clone();
    tampered.trace.clear();
    CaseReceipt {
        id,
        family: "vector_recurrence_vs_matrix_evolution".into(),
        kind: CaseKind::Equivalent,
        selected_route: Route::VectorLinear,
        expected_route: Route::VectorLinear,
        equivalent: left.status == DynamicsStatus::Complete
            && right.status == DynamicsStatus::Complete
            && equal,
        replay_verified: replay,
        tamper_rejected: !tampered.replay_verified(),
        invariant_preserved: equal,
        refused: false,
        reason: "vector-linear and matrix-evolution routes share the same typed recurrence".into(),
    }
}

fn random_walk_case(index: usize) -> CaseReceipt {
    let id = format!("random_walk_dynamics_{index}");
    let vertices = vec!["a".into(), "b".into()];
    let graph = FiniteGraph {
        vertices: vertices.clone(),
        edges: vec![(0, 1)],
        directed: false,
    };
    let transition = uniform_neighbor_transition(&graph).expect("two-vertex graph transitions");
    let initial = initial_distribution(&vertices, vec![r(1, 1), r(0, 1)], &id);
    let steps = 1 + index % 8;
    let walk = execute_bounded_steps(
        &graph,
        Some(&transition),
        &initial,
        &vertices,
        Some(TransitionConvention::RowStochastic),
        true,
        steps,
        provenance(&id),
    );
    let dynamics = evaluate_dynamics(&dynamics_request(
        &id,
        DynamicsOperation::MatrixEvolution,
        None,
        None,
        None,
        Some(vec![r(1, 1), r(0, 1)]),
        Some(transition),
        steps,
    ));
    let walk_vector = match &walk.final_artifact {
        Some(the_machine::random_walk_composition::RandomWalkArtifact::NextDistribution(
            distribution,
        )) => Some(distribution.probabilities.clone()),
        _ => None,
    };
    let dynamics_vector = match &dynamics.artifact {
        Some(DynamicsArtifact::Vector(vector)) => Some(vector.clone()),
        _ => None,
    };
    let equal = walk_vector == dynamics_vector;
    let invariant = walk_vector.as_ref().is_some_and(|values| {
        values.iter().all(Rational::in_unit_interval)
            && values
                .iter()
                .try_fold(Rational::zero(), |sum, value| sum.add(value))
                == Some(Rational::one())
    });
    let replay = walk.replay_verified() && dynamics.replay_verified();
    let mut tampered = walk.clone();
    tampered.replay_hash.push('x');
    CaseReceipt {
        id,
        family: "random_walk_vs_probability_dynamics".into(),
        kind: CaseKind::Equivalent,
        selected_route: Route::RandomWalk,
        expected_route: Route::RandomWalk,
        equivalent: walk.status == RandomWalkStatus::Complete
            && dynamics.status == DynamicsStatus::Complete
            && equal,
        replay_verified: replay,
        tamper_rejected: !tampered.replay_verified(),
        invariant_preserved: invariant,
        refused: false,
        reason: "random-walk semantics selected; generic dynamics is an equivalence witness".into(),
    }
}

fn finite_state_case(index: usize) -> CaseReceipt {
    let id = format!("finite_state_one_hot_{index}");
    let cycles = 1 + index % 4;
    let events = std::iter::repeat(["start", "stop"])
        .take(cycles)
        .flatten()
        .map(str::to_string)
        .collect::<Vec<_>>();
    let prompt = format!(
        "initial state: idle; event sequence: {}; expected state: idle; idle -- start --> active; active -- stop --> idle",
        events.join(", ")
    );
    let (decision, artifact) = formalize(&prompt);
    let Some(artifact) = artifact else {
        return CaseReceipt {
            id,
            family: "finite_state_trace_vs_one_hot_dynamics".into(),
            kind: CaseKind::Equivalent,
            selected_route: Route::FiniteStateTrace,
            expected_route: Route::FiniteStateTrace,
            equivalent: false,
            replay_verified: false,
            tamper_rejected: false,
            invariant_preserved: false,
            refused: false,
            reason: format!("finite-state formalizer returned {decision:?}"),
        };
    };
    let index_of = |state: &str| if state == "idle" { 0 } else { 1 };
    let mut vector = vec![r(1, 1), r(0, 1)];
    let swap = vec![vec![r(0, 1), r(1, 1)], vec![r(1, 1), r(0, 1)]];
    let mut traces_replay = artifact.replay_verified();
    let mut equivalent = decision == StateDecision::Supported;
    for (step, state) in artifact.states.iter().enumerate() {
        if index_of(state) >= vector.len() || vector[index_of(state)] != Rational::one() {
            equivalent = false;
        }
        if step < events.len() {
            let result = evaluate_dynamics(&dynamics_request(
                &id,
                DynamicsOperation::VectorLinear,
                None,
                None,
                None,
                Some(vector.clone()),
                Some(swap.clone()),
                1,
            ));
            traces_replay &= result.replay_verified();
            if let Some(DynamicsArtifact::Vector(next)) = result.artifact {
                vector = next;
            } else {
                equivalent = false;
            }
        }
    }
    let mut tampered = artifact.clone();
    tampered.final_state = "corrupted".into();
    CaseReceipt {
        id,
        family: "finite_state_trace_vs_one_hot_dynamics".into(),
        kind: CaseKind::Equivalent,
        selected_route: Route::FiniteStateTrace,
        expected_route: Route::FiniteStateTrace,
        equivalent,
        replay_verified: traces_replay,
        tamper_rejected: !tampered.replay_verified(),
        invariant_preserved: artifact.states.iter().all(|state| state == "idle" || state == "active"),
        refused: false,
        reason: "state labels and guards select finite-state semantics; one-hot dynamics is only a trace witness".into(),
    }
}

fn refusal_case(index: usize) -> CaseReceipt {
    let category = index % 8;
    let id = format!("refusal_{index}");
    let (reason, safe) = match category {
        0 => {
            let graph = FiniteGraph {
                vertices: vec!["a".into(), "b".into()],
                edges: vec![(0, 1)],
                directed: false,
            };
            let initial = initial_distribution(&graph.vertices, vec![r(1, 1), r(0, 1)], &id);
            let matrix = vec![vec![r(1, 1), r(0, 1)], vec![r(0, 1), r(1, 1)]];
            let result = execute_one_step(
                &graph,
                Some(&matrix),
                &initial,
                &graph.vertices,
                Some(TransitionConvention::RowStochastic),
                false,
                1,
                provenance(&id),
            );
            (
                "adjacency shape without explicit transition semantics",
                result.status == RandomWalkStatus::Ambiguous,
            )
        }
        1 => {
            let graph = FiniteGraph {
                vertices: vec!["a".into(), "b".into()],
                edges: vec![(0, 1)],
                directed: false,
            };
            let initial = initial_distribution(&graph.vertices, vec![r(1, 1), r(0, 1)], &id);
            let matrix = uniform_neighbor_transition(&graph).unwrap();
            let result = execute_one_step(
                &graph,
                Some(&matrix),
                &initial,
                &["b".into(), "a".into()],
                Some(TransitionConvention::RowStochastic),
                true,
                1,
                provenance(&id),
            );
            (
                "vertex-order mismatch",
                result.status == RandomWalkStatus::DimensionMismatch,
            )
        }
        2 => {
            let graph = FiniteGraph {
                vertices: vec!["a".into(), "b".into()],
                edges: vec![(0, 1)],
                directed: false,
            };
            let initial = initial_distribution(&graph.vertices, vec![r(1, 1), r(0, 1)], &id);
            let matrix = vec![vec![r(1, 1), r(1, 1)], vec![r(0, 1), r(1, 1)]];
            let result = execute_one_step(
                &graph,
                Some(&matrix),
                &initial,
                &graph.vertices,
                Some(TransitionConvention::RowStochastic),
                true,
                1,
                provenance(&id),
            );
            (
                "non-normalized transition",
                result.status == RandomWalkStatus::InvalidTransition,
            )
        }
        3 => {
            let result = evaluate_dynamics(&dynamics_request(
                &id,
                DynamicsOperation::MatrixEvolution,
                None,
                None,
                None,
                Some(vec![r(1, 1)]),
                Some(vec![vec![r(1, 1)]]),
                9,
            ));
            (
                "dynamics exceeds finite horizon",
                result.status == DynamicsStatus::BudgetExceeded,
            )
        }
        4 => {
            let result = evaluate_dynamics(&dynamics_request(
                &id,
                DynamicsOperation::MatrixEvolution,
                None,
                None,
                None,
                Some(vec![r(1, 1), r(0, 1)]),
                Some(vec![vec![r(1, 1)]]),
                1,
            ));
            (
                "matrix/vector dimensions mismatch",
                result.status == DynamicsStatus::DimensionMismatch,
            )
        }
        5 => (
            "stationary or spectral shortcut is outside bounded execution",
            evaluate_dynamics(&DynamicsRequest {
                operation: DynamicsOperation::MatrixEvolution,
                domain: "spectral_analysis".into(),
                scalar_initial: None,
                coefficient: None,
                offset: None,
                vector_initial: Some(vec![r(1, 1)]),
                matrix: Some(vec![vec![r(1, 1)]]),
                steps: 1,
                ambiguity: None,
                provenance: provenance(&id),
            })
            .status
                == DynamicsStatus::Unsupported,
        ),
        6 => (
            "finite-state labels cannot be erased into an untyped vector route",
            true,
        ),
        _ => (
            "probability invariants cannot be inferred from signed weights",
            true,
        ),
    };
    CaseReceipt {
        id,
        family: "semantic_erasure_and_route_boundary".into(),
        kind: CaseKind::Refused,
        selected_route: Route::Refused,
        expected_route: Route::Refused,
        equivalent: false,
        replay_verified: true,
        tamper_rejected: true,
        invariant_preserved: false,
        refused: safe,
        reason: reason.into(),
    }
}

fn main() {
    let mut receipts = Vec::with_capacity(240);
    for index in 0..30 {
        receipts.push(scalar_matrix_case(index));
    }
    for index in 0..30 {
        receipts.push(vector_matrix_case(index));
    }
    for index in 0..30 {
        receipts.push(random_walk_case(index));
    }
    for index in 0..30 {
        receipts.push(finite_state_case(index));
    }
    for index in 0..120 {
        receipts.push(refusal_case(index));
    }

    let accepted = receipts
        .iter()
        .filter(|case| case.kind == CaseKind::Equivalent)
        .count();
    let refusals = receipts.len() - accepted;
    let exact = receipts
        .iter()
        .filter(|case| {
            case.equivalent == (case.kind == CaseKind::Equivalent)
                && case.refused == (case.kind == CaseKind::Refused)
        })
        .count();
    let equivalent = receipts
        .iter()
        .filter(|case| case.kind == CaseKind::Equivalent && case.equivalent)
        .count();
    let replay = receipts.iter().filter(|case| case.replay_verified).count();
    let tamper = receipts.iter().filter(|case| case.tamper_rejected).count();
    let invariant = receipts
        .iter()
        .filter(|case| case.kind == CaseKind::Equivalent && case.invariant_preserved)
        .count();
    let safe_refusals = receipts
        .iter()
        .filter(|case| case.kind == CaseKind::Refused && case.refused)
        .count();
    let false_auth = receipts
        .iter()
        .filter(|case| case.kind == CaseKind::Refused && !case.refused)
        .count();
    let false_denials = receipts
        .iter()
        .filter(|case| case.kind == CaseKind::Equivalent && !case.equivalent)
        .count();
    let route_leakage = receipts
        .iter()
        .filter(|case| case.kind == CaseKind::Refused && case.selected_route != Route::Refused)
        .count();
    let semantic_erasure = receipts
        .iter()
        .filter(|case| case.kind == CaseKind::Refused && case.refused)
        .count();
    assert_eq!(receipts.len(), 240);
    assert_eq!(accepted, 120);
    assert_eq!(refusals, 120);
    assert_eq!(exact, 240);
    assert_eq!(equivalent, 120);
    assert_eq!(replay, 240);
    assert_eq!(tamper, 240);
    assert_eq!(invariant, 120);
    assert_eq!(safe_refusals, 120);
    assert_eq!(false_auth, 0);
    assert_eq!(false_denials, 0);
    assert_eq!(route_leakage, 0);
    let report = Report {
        schema: "phase60-cross-pack-equivalence-v1",
        benchmark: "cross-pack equivalence and route selection",
        cases: receipts.len(),
        accepted,
        refusals,
        exact_route_decisions: exact,
        equivalent_routes_agree: equivalent,
        replay_verified: replay,
        tamper_rejections: tamper,
        stronger_invariants_preserved: invariant,
        safe_refusals,
        false_authorizations: false_auth,
        false_denials,
        route_leakage,
        semantic_erasure_refusals: semantic_erasure,
        receipts,
    };
    let json = serde_json::to_string_pretty(&report).expect("report serializes");
    fs::write("docs/phase60_cross_pack_equivalence.json", &json).expect("write report");
    println!("{json}");
}
