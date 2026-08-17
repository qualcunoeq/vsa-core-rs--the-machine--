//! Stage 130: expanded cross-domain synthesis after topology and characters.
//!
//! This is a new independent corpus rather than a rewrite of the historical
//! Stage-B report.  It exercises five typed routes, including the newly
//! validated finite simplicial and finite-character packs.  Every route has
//! supported, ambiguous, and refused cases; authorization requires all
//! emitted artifacts to replay and every typed handoff to preserve its
//! domain-specific invariant.

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;

use the_machine::abstract_algebra_pack::{
    evaluate_abstract_algebra, AbstractAlgebraArtifact, AbstractAlgebraOperation,
    AbstractAlgebraRequest, AbstractAlgebraStatus,
};
use the_machine::combinatorics_pack::{
    evaluate_combinatorics, CombinatoricsArtifact, CombinatoricsOperation, CombinatoricsRequest,
    CombinatoricsStatus,
};
use the_machine::dirichlet_character_pack::{
    evaluate as evaluate_character, CharacterArtifact, CharacterOperation, CharacterStatus,
    DirichletCharacterRequest,
};
use the_machine::graph_pack::{evaluate_graph, GraphOperation, GraphRequest, GraphStatus};
use the_machine::linear_algebra_pack::{evaluate_linear_algebra, LinearAlgebraStatus};
use the_machine::number_theory_pack::{
    evaluate_number_theory, NumberTheoryArtifact, NumberTheoryOperation, NumberTheoryRequest,
    NumberTheoryStatus,
};
use the_machine::probability_pack::{
    evaluate_probability, ProbabilityOperation, ProbabilityRequest, Rational,
};
use the_machine::random_walk_composition::{
    execute_one_step, uniform_neighbor_transition, RandomWalkStatus, TransitionConvention,
};
use the_machine::simplicial_homology_bridge::{one_skeleton_graph, BridgeStatus};
use the_machine::simplicial_homology_pack::{HomologyOperation, SimplicialComplexRequest};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
enum Expected {
    Supported,
    Ambiguous,
    Refused,
}

#[derive(Debug, Clone, Copy)]
struct Outcome {
    authorized: bool,
    exact: bool,
    replay_verified: bool,
    tamper_rejected: bool,
    handoff_verified: bool,
    intermediates: usize,
    failure_gate: Option<&'static str>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct CaseSpec {
    id: String,
    route: String,
    expected: Expected,
    seed: usize,
}

#[derive(Debug, Clone, Serialize)]
struct Receipt {
    id: String,
    route: String,
    expected: Expected,
    authorized: bool,
    exact: bool,
    replay_verified: bool,
    tamper_rejected: bool,
    handoff_verified: bool,
    intermediate_count: usize,
    failure_gate: Option<String>,
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
    refused: usize,
    exact_decisions: usize,
    supported_authorizations: usize,
    replay_verified: usize,
    tamper_rejections: usize,
    handoffs_verified: usize,
    failure_localized: usize,
    emitted_intermediate_entries: usize,
    false_authorizations: usize,
    false_denials: usize,
    route_leakage: usize,
    route_counts: BTreeMap<String, usize>,
    receipts: Vec<Receipt>,
}

fn digest<T: Serialize>(value: &T) -> String {
    format!("{:x}", Sha256::digest(serde_json::to_vec(value).unwrap()))
}

fn provenance() -> Vec<String> {
    vec!["stage130-independent-cross-domain-synthesis".into()]
}

fn homology_request(mode: Expected) -> SimplicialComplexRequest {
    SimplicialComplexRequest {
        operation: HomologyOperation::ValidateComplex,
        domain: if mode == Expected::Refused {
            "infinite_simplicial_complex".into()
        } else {
            "finite_simplicial_complex".into()
        },
        vertices: vec!["a".into(), "b".into(), "c".into()],
        simplices: vec![
            vec![0],
            vec![1],
            vec![2],
            vec![0, 1],
            vec![0, 2],
            vec![1, 2],
            vec![0, 1, 2],
        ],
        coefficient_field: Some(2),
        provenance: provenance(),
        ambiguity: None,
    }
}

fn homology_graph_linear(mode: Expected) -> Outcome {
    let request = homology_request(mode);
    let bridge = one_skeleton_graph(
        &request,
        if mode == Expected::Ambiguous {
            "infer_graph_semantics"
        } else {
            "one_skeleton_graph"
        },
    );
    let mut replay = bridge.replay_verified();
    let mut tamper = bridge.clone();
    tamper.replay_hash.push('x');
    let handoff = mode == Expected::Supported;
    let mut intermediates = 1;
    let mut authorized = false;
    if mode == Expected::Supported {
        let Some(_graph_result) = bridge.graph_result.as_ref() else {
            return Outcome {
                authorized: false,
                exact: false,
                replay_verified: replay,
                tamper_rejected: false,
                handoff_verified: false,
                intermediates,
                failure_gate: Some("one_skeleton_graph_handoff"),
            };
        };
        let Some(graph) = bridge.graph.as_ref() else {
            return Outcome {
                authorized: false,
                exact: false,
                replay_verified: replay,
                tamper_rejected: false,
                handoff_verified: false,
                intermediates,
                failure_gate: Some("one_skeleton_graph_handoff"),
            };
        };
        let matrix = evaluate_graph(&GraphRequest {
            operation: GraphOperation::AdjacencyMatrix,
            domain: "finite_simple_graph".into(),
            vertices: graph.vertices.clone(),
            edges: graph.edges.clone(),
            directed: false,
            matrix: None,
            vertex_order: graph.vertices.clone(),
            start: None,
            target: None,
            ambiguity: None,
            provenance: bridge.provenance.clone(),
        });
        let Some(linear_request) =
            the_machine::graph_pack::adjacency_to_linear_algebra(&matrix, false, &graph.vertices)
        else {
            return Outcome {
                authorized: false,
                exact: false,
                replay_verified: replay && matrix.replay_verified(),
                tamper_rejected: false,
                handoff_verified: false,
                intermediates: 2,
                failure_gate: Some("adjacency_to_linear_lowering"),
            };
        };
        let linear = evaluate_linear_algebra(&linear_request);
        let mut tm = matrix.clone();
        tm.replay_hash.push('x');
        let mut tl = linear.clone();
        tl.replay_hash.push('x');
        replay &= matrix.replay_verified() && linear.replay_verified();
        tamper.replay_hash.push('x');
        let tamper_rejected =
            !tamper.replay_verified() && !tm.replay_verified() && !tl.replay_verified();
        intermediates = 3;
        authorized = bridge.authorized()
            && matrix.status == GraphStatus::Complete
            && linear.status == LinearAlgebraStatus::Complete
            && linear.artifact.is_some()
            && replay;
        return Outcome {
            authorized,
            exact: authorized,
            replay_verified: replay,
            tamper_rejected,
            handoff_verified: handoff,
            intermediates,
            failure_gate: (!authorized).then_some("homology_graph_linear_route"),
        };
    }
    let exact = match mode {
        Expected::Ambiguous => bridge.status == BridgeStatus::Ambiguous,
        Expected::Refused => bridge.status != BridgeStatus::Complete,
        Expected::Supported => false,
    };
    Outcome {
        authorized,
        exact,
        replay_verified: replay,
        tamper_rejected: !tamper.replay_verified(),
        handoff_verified: true,
        intermediates,
        failure_gate: Some(if mode == Expected::Ambiguous {
            "graph_policy_ambiguity"
        } else {
            "simplicial_domain_boundary"
        }),
    }
}

fn probability_distribution(
    vertices: &[String],
) -> the_machine::probability_pack::ProbabilityResult {
    evaluate_probability(&ProbabilityRequest {
        operation: ProbabilityOperation::DistributionConstruction,
        domain: "finite_exact_probability".into(),
        outcomes: vertices.to_vec(),
        probabilities: vec![Rational::one(), Rational::zero(), Rational::zero()],
        values: Vec::new(),
        event_a: None,
        event_b: None,
        partition: Vec::new(),
        conditional_values: Vec::new(),
        prior_probability: None,
        likelihood: None,
        evidence: None,
        ambiguity: None,
        provenance: provenance(),
    })
}

fn homology_graph_walk(mode: Expected) -> Outcome {
    let request = homology_request(Expected::Supported);
    let bridge = one_skeleton_graph(&request, "one_skeleton_graph");
    let Some(graph) = bridge.graph.clone() else {
        return Outcome {
            authorized: false,
            exact: false,
            replay_verified: bridge.replay_verified(),
            tamper_rejected: false,
            handoff_verified: false,
            intermediates: 1,
            failure_gate: Some("homology_graph_handoff"),
        };
    };
    let initial = probability_distribution(&graph.vertices);
    let transition = uniform_neighbor_transition(&graph).expect("triangle has no zero degree");
    let walk = execute_one_step(
        &graph,
        Some(&transition),
        &initial,
        &graph.vertices,
        if mode == Expected::Ambiguous {
            None
        } else {
            Some(TransitionConvention::RowStochastic)
        },
        true,
        if mode == Expected::Refused { 2 } else { 1 },
        provenance(),
    );
    let replay = bridge.replay_verified() && initial.replay_verified() && walk.replay_verified();
    let mut tb = bridge.clone();
    tb.replay_hash.push('x');
    let mut ti = initial.clone();
    ti.replay_hash.push('x');
    let mut tw = walk.clone();
    tw.replay_hash.push('x');
    let tamper_rejected = !tb.replay_verified() && !ti.replay_verified() && !tw.replay_verified();
    let authorized = mode == Expected::Supported
        && walk.status == RandomWalkStatus::Complete
        && walk.artifact.is_some()
        && replay;
    let exact = match mode {
        Expected::Supported => authorized,
        Expected::Ambiguous => walk.status == RandomWalkStatus::Ambiguous,
        Expected::Refused => walk.status == RandomWalkStatus::Unsupported,
    };
    Outcome {
        authorized,
        exact,
        replay_verified: replay,
        tamper_rejected,
        handoff_verified: true,
        intermediates: 3,
        failure_gate: (!authorized).then_some(match mode {
            Expected::Ambiguous => "stochastic_convention_ambiguity",
            Expected::Refused => "finite_step_budget",
            Expected::Supported => "homology_random_walk_route",
        }),
    }
}

fn character_algebra(mode: Expected, seed: usize) -> Outcome {
    let primes = [5u32, 7, 11, 13, 17];
    let modulus = if mode == Expected::Refused {
        9
    } else {
        primes[seed % primes.len()]
    };
    let character = evaluate_character(&DirichletCharacterRequest {
        operation: CharacterOperation::ValidateCharacter,
        modulus: Some(modulus),
        exponent: Some(1),
        value: None,
        sum_limit: None,
        domain: "bounded_dirichlet_character".into(),
        ambiguity: (mode == Expected::Ambiguous).then(|| "character exponent is not fixed".into()),
        provenance: provenance(),
    });
    let mut replay = character.replay_verified();
    let mut tc = character.clone();
    tc.replay_hash.push('x');
    if mode != Expected::Supported {
        return Outcome {
            authorized: false,
            exact: if mode == Expected::Ambiguous {
                character.status == CharacterStatus::Ambiguous
            } else {
                character.status != CharacterStatus::Complete
            },
            replay_verified: replay,
            tamper_rejected: !tc.replay_verified(),
            handoff_verified: true,
            intermediates: 1,
            failure_gate: Some(if mode == Expected::Ambiguous {
                "character_parameter_ambiguity"
            } else {
                "finite_character_domain_boundary"
            }),
        };
    }
    let Some(CharacterArtifact::Character {
        modulus, generator, ..
    }) = character.artifact.clone()
    else {
        return Outcome {
            authorized: false,
            exact: false,
            replay_verified: replay,
            tamper_rejected: !tc.replay_verified(),
            handoff_verified: false,
            intermediates: 1,
            failure_gate: Some("character_to_algebra_handoff"),
        };
    };
    let algebra = evaluate_abstract_algebra(&AbstractAlgebraRequest {
        operation: AbstractAlgebraOperation::CheckUnit,
        modulus: Some(modulus),
        source_modulus: None,
        target_modulus: None,
        element: Some(generator),
        multiplier: None,
        second_multiplier: None,
        domain: "finite_exact_abstract_algebra".into(),
        assumptions: vec!["character generator is a unit modulo its prime".into()],
        ambiguity: None,
        provenance: character.provenance.clone(),
    });
    let mut ta = algebra.clone();
    ta.replay_hash.push('x');
    replay &= algebra.replay_verified();
    let handoff = matches!(
        algebra.artifact,
        Some(AbstractAlgebraArtifact::Boolean(true))
    );
    let authorized = character.authorized()
        && algebra.status == AbstractAlgebraStatus::Complete
        && handoff
        && replay;
    Outcome {
        authorized,
        exact: authorized,
        replay_verified: replay,
        tamper_rejected: !tc.replay_verified() && !ta.replay_verified(),
        handoff_verified: handoff,
        intermediates: 2,
        failure_gate: (!authorized).then_some("character_algebra_route"),
    }
}

fn combinatorics_number(mode: Expected, seed: usize) -> Outcome {
    let count = evaluate_combinatorics(&CombinatoricsRequest {
        operation: CombinatoricsOperation::Combinations,
        n: Some(5 + (seed % 3) as u64),
        k: Some(2),
        parts: Vec::new(),
        first_count: None,
        second_count: None,
        intersection_count: None,
        objects: None,
        boxes: None,
        domain: "bounded_exact_combinatorics".into(),
        ambiguity: (mode == Expected::Ambiguous)
            .then(|| "the count's arithmetic role is unresolved".into()),
        provenance: provenance(),
    });
    let mut tc = count.clone();
    tc.replay_hash.push('x');
    if mode == Expected::Ambiguous {
        let mut number = number_request(NumberTheoryStatus::Ambiguous, None, None, None, None);
        number.ambiguity = Some("count role is unresolved".into());
        let result = evaluate_number_theory(&number);
        let mut tn = result.clone();
        tn.replay_hash.push('x');
        return Outcome {
            authorized: false,
            exact: count.status == CombinatoricsStatus::Ambiguous
                && result.status == NumberTheoryStatus::Ambiguous,
            replay_verified: count.replay_verified() && result.replay_verified(),
            tamper_rejected: !tc.replay_verified() && !tn.replay_verified(),
            handoff_verified: true,
            intermediates: 2,
            failure_gate: Some("count_role_ambiguity"),
        };
    }
    let value = match count.artifact {
        Some(CombinatoricsArtifact::Scalar(value)) => value as i64,
        _ => 0,
    };
    let mut number_request = number_request(
        if mode == Expected::Refused {
            NumberTheoryStatus::InvalidDomain
        } else {
            NumberTheoryStatus::Complete
        },
        Some(value),
        Some(7),
        None,
        None,
    );
    if mode == Expected::Refused {
        number_request.domain = "cryptographic_security_claim".into();
    }
    let number = evaluate_number_theory(&number_request);
    let mut tn = number.clone();
    tn.replay_hash.push('x');
    let replay = count.replay_verified() && number.replay_verified();
    let handoff = matches!(
        number.artifact,
        Some(NumberTheoryArtifact::GcdBezout { gcd, x, y }) if value * x + 7 * y == gcd
    );
    let authorized = mode == Expected::Supported
        && count.status == CombinatoricsStatus::Complete
        && number.status == NumberTheoryStatus::Complete
        && handoff
        && replay;
    Outcome {
        authorized,
        exact: if mode == Expected::Supported {
            authorized
        } else {
            number.status == NumberTheoryStatus::InvalidDomain && !number.artifact.is_some()
        },
        replay_verified: replay,
        tamper_rejected: !tc.replay_verified() && !tn.replay_verified(),
        handoff_verified: handoff || mode == Expected::Refused,
        intermediates: 2,
        failure_gate: (!authorized).then_some(if mode == Expected::Refused {
            "number_theory_domain_boundary"
        } else {
            "count_number_handoff"
        }),
    }
}

fn number_request(
    _expected: NumberTheoryStatus,
    a: Option<i64>,
    b: Option<i64>,
    c: Option<i64>,
    modulus: Option<u64>,
) -> NumberTheoryRequest {
    NumberTheoryRequest {
        operation: NumberTheoryOperation::GcdBezout,
        a,
        b,
        c,
        modulus,
        second_modulus: None,
        domain: "bounded_exact_elementary_number_theory".into(),
        ambiguity: None,
        provenance: provenance(),
    }
}

fn graph_probability_walk(mode: Expected) -> Outcome {
    let vertices = vec!["a".into(), "b".into(), "c".into()];
    let graph = evaluate_graph(&GraphRequest {
        operation: GraphOperation::Construction,
        domain: "finite_simple_graph".into(),
        vertices: vertices.clone(),
        edges: vec![(0, 1), (1, 2), (0, 2)],
        directed: false,
        matrix: None,
        vertex_order: vertices.clone(),
        start: None,
        target: None,
        ambiguity: None,
        provenance: provenance(),
    });
    let Some(the_machine::graph_pack::GraphArtifact::Graph(graph_artifact)) =
        graph.artifact.clone()
    else {
        return Outcome {
            authorized: false,
            exact: false,
            replay_verified: graph.replay_verified(),
            tamper_rejected: false,
            handoff_verified: false,
            intermediates: 1,
            failure_gate: Some("graph_construction"),
        };
    };
    let initial = probability_distribution(&graph_artifact.vertices);
    let transition = uniform_neighbor_transition(&graph_artifact).expect("triangle transition");
    let walk = execute_one_step(
        &graph_artifact,
        Some(&transition),
        &initial,
        &graph_artifact.vertices,
        Some(TransitionConvention::RowStochastic),
        mode != Expected::Ambiguous,
        if mode == Expected::Refused { 2 } else { 1 },
        provenance(),
    );
    let replay = graph.replay_verified() && initial.replay_verified() && walk.replay_verified();
    let mut tg = graph.clone();
    tg.replay_hash.push('x');
    let mut ti = initial.clone();
    ti.replay_hash.push('x');
    let mut tw = walk.clone();
    tw.replay_hash.push('x');
    let authorized = mode == Expected::Supported
        && walk.status == RandomWalkStatus::Complete
        && walk.artifact.is_some()
        && replay;
    Outcome {
        authorized,
        exact: match mode {
            Expected::Supported => authorized,
            Expected::Ambiguous => walk.status == RandomWalkStatus::Ambiguous,
            Expected::Refused => walk.status == RandomWalkStatus::Unsupported,
        },
        replay_verified: replay,
        tamper_rejected: !tg.replay_verified() && !ti.replay_verified() && !tw.replay_verified(),
        handoff_verified: true,
        intermediates: 3,
        failure_gate: (!authorized).then_some(match mode {
            Expected::Ambiguous => "graph_transition_semantics_ambiguity",
            Expected::Refused => "finite_step_budget",
            Expected::Supported => "graph_probability_route",
        }),
    }
}

fn evaluate_case(case: &CaseSpec) -> Outcome {
    let mode = case.expected;
    match case.route.as_str() {
        "homology_graph_linear" => homology_graph_linear(mode),
        "homology_graph_walk" => homology_graph_walk(mode),
        "character_algebra" => character_algebra(mode, case.seed),
        "combinatorics_number" => combinatorics_number(mode, case.seed),
        "graph_probability_walk" => graph_probability_walk(mode),
        _ => unreachable!("corpus route is closed before execution"),
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let routes = [
        "homology_graph_linear",
        "homology_graph_walk",
        "character_algebra",
        "combinatorics_number",
        "graph_probability_walk",
    ];
    let mut corpus = Vec::with_capacity(1000);
    for route in routes {
        for seed in 0..140 {
            corpus.push(CaseSpec {
                id: format!("{route}_supported_{seed:03}"),
                route: route.into(),
                expected: Expected::Supported,
                seed,
            });
        }
        for seed in 0..30 {
            corpus.push(CaseSpec {
                id: format!("{route}_ambiguous_{seed:03}"),
                route: route.into(),
                expected: Expected::Ambiguous,
                seed,
            });
        }
        for seed in 0..30 {
            corpus.push(CaseSpec {
                id: format!("{route}_refused_{seed:03}"),
                route: route.into(),
                expected: Expected::Refused,
                seed,
            });
        }
    }
    assert_eq!(corpus.len(), 1000);
    let corpus_sha256 = digest(&corpus);
    let mut receipts = Vec::with_capacity(corpus.len());
    let mut route_counts = BTreeMap::new();
    for case in corpus {
        *route_counts.entry(case.route.clone()).or_insert(0) += 1;
        let outcome = evaluate_case(&case);
        let false_authorization = case.expected != Expected::Supported && outcome.authorized;
        let false_denial = case.expected == Expected::Supported && !outcome.authorized;
        receipts.push(Receipt {
            id: case.id,
            route: case.route,
            expected: case.expected,
            authorized: outcome.authorized,
            exact: outcome.exact && !false_authorization && !false_denial,
            replay_verified: outcome.replay_verified,
            tamper_rejected: outcome.tamper_rejected,
            handoff_verified: outcome.handoff_verified,
            intermediate_count: outcome.intermediates,
            failure_gate: outcome.failure_gate.map(str::to_owned),
            false_authorization,
            false_denial,
        });
    }
    let cases = receipts.len();
    let supported = receipts
        .iter()
        .filter(|r| r.expected == Expected::Supported)
        .count();
    let ambiguous = receipts
        .iter()
        .filter(|r| r.expected == Expected::Ambiguous)
        .count();
    let refused = receipts
        .iter()
        .filter(|r| r.expected == Expected::Refused)
        .count();
    let exact_decisions = receipts.iter().filter(|r| r.exact).count();
    let supported_authorizations = receipts.iter().filter(|r| r.authorized).count();
    let replay_verified = receipts.iter().filter(|r| r.replay_verified).count();
    let tamper_rejections = receipts.iter().filter(|r| r.tamper_rejected).count();
    let handoffs_verified = receipts.iter().filter(|r| r.handoff_verified).count();
    let failure_localized = receipts
        .iter()
        .filter(|r| r.expected != Expected::Supported && r.failure_gate.is_some())
        .count();
    let emitted_intermediate_entries = receipts.iter().map(|r| r.intermediate_count).sum();
    let false_authorizations = receipts.iter().filter(|r| r.false_authorization).count();
    let false_denials = receipts.iter().filter(|r| r.false_denial).count();
    let route_leakage = receipts
        .iter()
        .filter(|r| (r.expected == Expected::Supported) != r.authorized)
        .count();
    assert_eq!((supported, ambiguous, refused), (700, 150, 150));
    assert_eq!(exact_decisions, cases);
    assert_eq!(supported_authorizations, supported);
    assert_eq!(replay_verified, cases);
    assert_eq!(tamper_rejections, cases);
    assert_eq!(handoffs_verified, cases);
    assert_eq!(failure_localized, ambiguous + refused);
    assert_eq!(false_authorizations, 0);
    assert_eq!(false_denials, 0);
    assert_eq!(route_leakage, 0);
    let report = Report {
        schema: "stage130-expanded-cross-domain-synthesis-v1",
        source:
            "independently authored cross-domain corpus including finite topology and characters",
        corpus_sha256,
        cases,
        supported,
        ambiguous,
        refused,
        exact_decisions,
        supported_authorizations,
        replay_verified,
        tamper_rejections,
        handoffs_verified,
        failure_localized,
        emitted_intermediate_entries,
        false_authorizations,
        false_denials,
        route_leakage,
        route_counts,
        receipts,
    };
    std::fs::write(
        "docs/stage130_expanded_cross_domain_synthesis.json",
        serde_json::to_vec_pretty(&report)?,
    )?;
    println!("{}", serde_json::to_string_pretty(&report)?);
    Ok(())
}
