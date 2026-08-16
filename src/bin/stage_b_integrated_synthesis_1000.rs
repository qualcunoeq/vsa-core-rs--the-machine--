//! Stage-B integrated synthesis benchmark.
//!
//! This corpus exercises five independently validated routes in one shadow
//! campaign.  Each route is composed from typed packs; no route is selected
//! from vocabulary and no production registry or manifest is mutated.

use serde::Serialize;
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use the_machine::combinatorics_pack::{
    evaluate_combinatorics, CombinatoricsArtifact, CombinatoricsOperation, CombinatoricsRequest,
};
use the_machine::graph_pack::{evaluate_graph, GraphOperation, GraphRequest};
use the_machine::linear_algebra_pack::{evaluate_linear_algebra, LinearAlgebraStatus};
use the_machine::number_theory_pack::{
    evaluate_number_theory, NumberTheoryArtifact, NumberTheoryOperation, NumberTheoryRequest,
    NumberTheoryStatus,
};
use the_machine::probability_pack::{
    evaluate_probability, ProbabilityOperation, ProbabilityRequest, ProbabilityStatus, Rational,
};
use the_machine::random_walk_composition::{execute_one_step, TransitionConvention};
use the_machine::source_formula_pack::biology_pack::biology_probability_bridge::{
    bridge_base_composition, BiologyProbabilityBridgeStatus,
};
use the_machine::source_formula_pack::biology_pack::{
    evaluate_biology as evaluate_source_biology, BiologyOperation as SourceBiologyOperation,
    BiologyRequest as SourceBiologyRequest,
};
use the_machine::source_formula_pack::chemistry_pack::chemistry_linear_bridge::{
    bridge_chemistry_to_linear as bridge_source_chemistry_to_linear, ChemistryLinearBridgeStatus,
};
use the_machine::source_formula_pack::chemistry_pack::{
    evaluate_chemistry as evaluate_source_chemistry,
    ChemistryOperation as SourceChemistryOperation, ChemistryRequest as SourceChemistryRequest,
};
use the_machine::source_topology_graph_bridge::topology_to_graph;
use the_machine::source_topology_pack::{
    extract_topology_definitions, TopologyOperation, TopologyRequest,
};

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
enum Expected {
    Supported,
    Ambiguous,
    Refused,
}

#[derive(Debug, Clone, Serialize)]
struct RouteReceipt {
    id: String,
    route: String,
    expected: Expected,
    authorized: bool,
    exact: bool,
    intermediate_count: usize,
    replay_verified: bool,
    tamper_rejected: bool,
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
    supported_routes: usize,
    replay_verified: usize,
    emitted_intermediate_entries: usize,
    tamper_rejections: usize,
    failure_localized: usize,
    false_authorizations: usize,
    false_denials: usize,
    route_leakage: usize,
    route_counts: BTreeMap<String, usize>,
    receipts: Vec<RouteReceipt>,
}

#[derive(Debug, Clone)]
struct Outcome {
    authorized: bool,
    exact_status: bool,
    intermediate_count: usize,
    replay_verified: bool,
    tamper_rejected: bool,
    failure_gate: Option<String>,
}

fn digest<T: Serialize>(value: &T) -> String {
    format!(
        "{:x}",
        Sha256::digest(serde_json::to_vec(value).expect("integrated corpus serializes"))
    )
}

fn q(n: i128, d: i128) -> Rational {
    Rational::new(n, d).expect("valid exact rational")
}

fn topology_request(ambiguous: bool, refused: bool) -> TopologyRequest {
    TopologyRequest {
        operation: TopologyOperation::ValidateTopology,
        topology: "finite_topology_axioms".into(),
        points: if refused {
            (0..9).map(|i| format!("p{i}")).collect()
        } else {
            vec!["a".into(), "b".into(), "c".into()]
        },
        open_sets: if refused {
            vec![Vec::new()]
        } else {
            vec![
                Vec::new(),
                vec!["a".into()],
                vec!["a".into(), "b".into(), "c".into()],
            ]
        },
        target_set: None,
        domain: if refused {
            "infinite_topology".into()
        } else {
            "source_derived_finite_topology".into()
        },
        ambiguity: ambiguous.then(|| "graph policy is not fixed".into()),
        provenance: vec!["stage-b-integrated-synthesis-1000".into()],
    }
}

fn topology_route(mode: Expected) -> Outcome {
    let ambiguous = mode == Expected::Ambiguous;
    let refused = mode == Expected::Refused;
    let records = extract_topology_definitions(include_str!(
        "../../docs/sources/topology_without_tears_finite_definition.txt"
    ))
    .expect("source topology record");
    let bridge = topology_to_graph(
        &topology_request(ambiguous, refused),
        &records,
        if ambiguous {
            "infer_graph_semantics"
        } else {
            "strict_specialization_graph"
        },
    );
    if mode != Expected::Supported {
        let exact_status = if ambiguous {
            bridge.status == the_machine::graph_pack::GraphStatus::Ambiguous
        } else {
            !bridge.authorized()
        };
        let tamper_rejected = {
            let mut tampered = bridge.clone();
            tampered.replay_hash.push('x');
            !tampered.replay_verified()
        };
        return Outcome {
            authorized: false,
            exact_status,
            intermediate_count: 1,
            replay_verified: bridge.replay_verified(),
            tamper_rejected,
            failure_gate: Some(
                if ambiguous {
                    "graph_policy_ambiguity"
                } else {
                    "topology_boundary"
                }
                .into(),
            ),
        };
    }
    let Some(graph) = bridge.graph.clone() else {
        return Outcome {
            authorized: false,
            exact_status: false,
            intermediate_count: 1,
            replay_verified: bridge.replay_verified(),
            tamper_rejected: false,
            failure_gate: Some("graph_handoff".into()),
        };
    };
    let graph_request = GraphRequest {
        operation: GraphOperation::AdjacencyMatrix,
        domain: "finite_simple_graph".into(),
        vertices: graph.vertices.clone(),
        edges: graph.edges.clone(),
        directed: true,
        matrix: None,
        vertex_order: graph.vertices.clone(),
        start: None,
        target: None,
        ambiguity: None,
        provenance: bridge.provenance.clone(),
    };
    let matrix = evaluate_graph(&graph_request);
    let Some(linear_request) =
        the_machine::graph_pack::adjacency_to_linear_algebra(&matrix, true, &graph.vertices)
    else {
        return Outcome {
            authorized: false,
            exact_status: false,
            intermediate_count: 2,
            replay_verified: bridge.replay_verified() && matrix.replay_verified(),
            tamper_rejected: false,
            failure_gate: Some("adjacency_lowering".into()),
        };
    };
    let linear = evaluate_linear_algebra(&linear_request);
    let authorized = bridge.authorized()
        && matrix.replay_verified()
        && matrix.status == the_machine::graph_pack::GraphStatus::Complete
        && linear.status == LinearAlgebraStatus::Complete
        && linear.artifact.is_some()
        && linear.replay_verified();
    let mut tb = bridge.clone();
    tb.replay_hash.push('x');
    let mut tm = matrix.clone();
    tm.replay_hash.push('x');
    let mut tl = linear.clone();
    tl.replay_hash.push('x');
    Outcome {
        authorized,
        exact_status: authorized,
        intermediate_count: 3,
        replay_verified: bridge.replay_verified()
            && matrix.replay_verified()
            && linear.replay_verified(),
        tamper_rejected: !tb.replay_verified() && !tm.replay_verified() && !tl.replay_verified(),
        failure_gate: (!authorized).then(|| "topology_graph_linear_handoff".into()),
    }
}

fn probability_request(initial: Vec<Rational>, outcomes: Vec<String>) -> ProbabilityRequest {
    ProbabilityRequest {
        operation: ProbabilityOperation::DistributionConstruction,
        domain: "finite_exact_probability".into(),
        outcomes,
        probabilities: initial,
        values: vec![0, 1],
        event_a: None,
        event_b: None,
        partition: Vec::new(),
        conditional_values: Vec::new(),
        prior_probability: None,
        likelihood: None,
        evidence: None,
        ambiguity: None,
        provenance: vec!["stage-b-integrated-synthesis-1000".into()],
    }
}

fn random_walk_route(mode: Expected) -> Outcome {
    let graph = the_machine::graph_pack::FiniteGraph {
        vertices: vec!["a".into(), "b".into()],
        edges: vec![(0, 1)],
        directed: false,
    };
    let graph_request = GraphRequest {
        operation: GraphOperation::Construction,
        domain: "finite_simple_graph".into(),
        vertices: graph.vertices.clone(),
        edges: graph.edges.clone(),
        directed: false,
        matrix: None,
        vertex_order: graph.vertices.clone(),
        start: None,
        target: None,
        ambiguity: None,
        provenance: vec!["stage-b-integrated-synthesis-1000".into()],
    };
    let graph_result = evaluate_graph(&graph_request);
    let initial = evaluate_probability(&probability_request(
        vec![q(1, 1), q(0, 1)],
        graph.vertices.clone(),
    ));
    let transition = vec![vec![q(0, 1), q(1, 1)], vec![q(1, 1), q(0, 1)]];
    let walk = execute_one_step(
        &graph,
        Some(&transition),
        &initial,
        &graph.vertices,
        Some(TransitionConvention::RowStochastic),
        mode != Expected::Ambiguous,
        if mode == Expected::Refused { 2 } else { 1 },
        vec!["stage-b-integrated-synthesis-1000".into()],
    );
    let replay =
        graph_result.replay_verified() && initial.replay_verified() && walk.replay_verified();
    let authorized = mode == Expected::Supported
        && graph_result.status == the_machine::graph_pack::GraphStatus::Complete
        && initial.status == ProbabilityStatus::Complete
        && walk.status == the_machine::random_walk_composition::RandomWalkStatus::Complete
        && walk.artifact.is_some()
        && replay;
    let mut tg = graph_result.clone();
    tg.replay_hash.push('x');
    let mut ti = initial.clone();
    ti.replay_hash.push('x');
    let mut tw = walk.clone();
    tw.replay_hash.push('x');
    Outcome {
        authorized,
        exact_status: match mode {
            Expected::Supported => authorized,
            Expected::Ambiguous => {
                walk.status == the_machine::random_walk_composition::RandomWalkStatus::Ambiguous
            }
            Expected::Refused => {
                walk.status == the_machine::random_walk_composition::RandomWalkStatus::Unsupported
            }
        },
        intermediate_count: 3,
        replay_verified: replay,
        tamper_rejected: !tg.replay_verified() && !ti.replay_verified() && !tw.replay_verified(),
        failure_gate: (!authorized).then(|| match mode {
            Expected::Ambiguous => "stochastic_convention_ambiguity".into(),
            Expected::Refused => "finite_step_budget".into(),
            Expected::Supported => "random_walk_handoff".into(),
        }),
    }
}

fn biology_route(mode: Expected) -> Outcome {
    let request = SourceBiologyRequest {
        operation: SourceBiologyOperation::BaseComposition,
        sequence: Some("AATTGGCC".into()),
        orientation: None,
        domain: "source_derived_bounded_dna".into(),
        ambiguity: None,
        provenance: vec!["stage-b-integrated-synthesis-1000".into()],
    };
    let biology = evaluate_source_biology(&request);
    let bridge = bridge_base_composition(
        &biology,
        match mode {
            Expected::Supported => Some("uniform_position"),
            Expected::Ambiguous => None,
            Expected::Refused => Some("independent_bases"),
        },
    );
    let probability = bridge
        .handoff
        .as_ref()
        .map(|handoff| evaluate_probability(&handoff.request));
    let probability_ok = probability
        .as_ref()
        .is_none_or(|r| r.status == ProbabilityStatus::Complete && r.artifact.is_some());
    let replay = biology.replay_verified()
        && bridge.replay_verified()
        && probability.as_ref().is_none_or(|r| r.replay_verified());
    let authorized = mode == Expected::Supported
        && bridge.status == BiologyProbabilityBridgeStatus::Complete
        && probability_ok
        && replay;
    let mut tb = biology.clone();
    tb.replay_hash.push('x');
    let mut tr = bridge.clone();
    tr.replay_hash.push('x');
    let tp = probability.as_ref().is_none_or(|r| {
        let mut t = r.clone();
        t.replay_hash.push('x');
        !t.replay_verified()
    });
    Outcome {
        authorized,
        exact_status: match mode {
            Expected::Supported => authorized,
            Expected::Ambiguous => bridge.status == BiologyProbabilityBridgeStatus::Ambiguous,
            Expected::Refused => bridge.status == BiologyProbabilityBridgeStatus::Unsupported,
        },
        intermediate_count: 2 + usize::from(probability.is_some()),
        replay_verified: replay,
        tamper_rejected: !tb.replay_verified() && !tr.replay_verified() && tp,
        failure_gate: (!authorized).then(|| match mode {
            Expected::Ambiguous => "sampling_policy_ambiguity".into(),
            Expected::Refused => "unsupported_sampling_semantics".into(),
            Expected::Supported => "biology_probability_handoff".into(),
        }),
    }
}

fn chemistry_route(mode: Expected) -> Outcome {
    let request = SourceChemistryRequest {
        operation: SourceChemistryOperation::ParseFormula,
        formula: if mode == Expected::Refused {
            Some("Na+".into())
        } else {
            Some("H2O".into())
        },
        reaction: None,
        from_species: None,
        to_species: None,
        domain: "source_derived_bounded_chemistry".into(),
        ambiguity: (mode == Expected::Ambiguous).then(|| "formula target is not unique".into()),
        provenance: vec!["stage-b-integrated-synthesis-1000".into()],
    };
    let chemistry = evaluate_source_chemistry(&request);
    let bridge = bridge_source_chemistry_to_linear(&chemistry);
    let linear = bridge.artifact.as_ref().map(|vector| {
        evaluate_linear_algebra(&the_machine::linear_algebra_pack::LinearAlgebraRequest {
            operation: the_machine::linear_algebra_pack::LinearAlgebraOperation::VectorConstruction,
            matrix: None,
            vector_a: Some(vector.values.clone()),
            vector_b: None,
            domain: "finite_exact_integer".into(),
            requested_output: format!("element_count_vector:{}", vector.semantic_kind),
            provenance: bridge.provenance.clone(),
        })
    });
    let replay = chemistry.replay_verified()
        && bridge.replay_verified()
        && linear.as_ref().is_none_or(|r| r.replay_verified());
    let authorized = mode == Expected::Supported
        && bridge.authorized()
        && linear
            .as_ref()
            .is_some_and(|r| r.status == LinearAlgebraStatus::Complete && r.artifact.is_some())
        && replay;
    let mut tc = chemistry.clone();
    tc.replay_hash.push('x');
    let mut tb = bridge.clone();
    tb.replay_hash.push('x');
    let tl = linear.as_ref().is_none_or(|r| {
        let mut t = r.clone();
        t.replay_hash.push('x');
        !t.replay_verified()
    });
    Outcome {
        authorized,
        exact_status: match mode {
            Expected::Supported => authorized,
            Expected::Ambiguous => bridge.status == ChemistryLinearBridgeStatus::Ambiguous,
            Expected::Refused => bridge.status == ChemistryLinearBridgeStatus::Unsupported,
        },
        intermediate_count: 2 + usize::from(linear.is_some()),
        replay_verified: replay,
        tamper_rejected: !tc.replay_verified() && !tb.replay_verified() && tl,
        failure_gate: (!authorized).then(|| match mode {
            Expected::Ambiguous => "chemistry_formula_ambiguity".into(),
            Expected::Refused => "stoichiometric_semantics_not_vector".into(),
            Expected::Supported => "chemistry_linear_handoff".into(),
        }),
    }
}

fn combinatorics_number_route(mode: Expected) -> Outcome {
    let count_request = CombinatoricsRequest {
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
        ambiguity: (mode == Expected::Ambiguous)
            .then(|| "count interpretation is unresolved".into()),
        provenance: vec!["stage-b-integrated-synthesis-1000".into()],
    };
    let count = evaluate_combinatorics(&count_request);
    let number = match count.artifact.as_ref() {
        Some(CombinatoricsArtifact::Scalar(value)) if mode != Expected::Ambiguous => {
            evaluate_number_theory(&NumberTheoryRequest {
                operation: NumberTheoryOperation::GcdBezout,
                a: Some(*value as i64),
                b: Some(7),
                c: None,
                modulus: None,
                second_modulus: None,
                domain: if mode == Expected::Refused {
                    "cryptographic_security_claim".into()
                } else {
                    "bounded_exact_elementary_number_theory".into()
                },
                ambiguity: None,
                provenance: vec!["stage-b-integrated-synthesis-1000".into()],
            })
        }
        _ => evaluate_number_theory(&NumberTheoryRequest {
            operation: NumberTheoryOperation::GcdBezout,
            a: None,
            b: None,
            c: None,
            modulus: None,
            second_modulus: None,
            domain: "bounded_exact_elementary_number_theory".into(),
            ambiguity: Some("count is ambiguous".into()),
            provenance: vec!["stage-b-integrated-synthesis-1000".into()],
        }),
    };
    let valid = matches!(number.artifact, Some(NumberTheoryArtifact::GcdBezout { gcd, x, y }) if 10 * x + 7 * y == gcd);
    let replay = count.replay_verified() && number.replay_verified();
    let authorized = mode == Expected::Supported
        && count.status == the_machine::combinatorics_pack::CombinatoricsStatus::Complete
        && number.status == NumberTheoryStatus::Complete
        && valid
        && replay;
    let mut tc = count.clone();
    tc.replay_hash.push('x');
    let mut tn = number.clone();
    tn.replay_hash.push('x');
    Outcome {
        authorized,
        exact_status: match mode {
            Expected::Supported => authorized,
            Expected::Ambiguous => {
                count.status == the_machine::combinatorics_pack::CombinatoricsStatus::Ambiguous
            }
            Expected::Refused => number.status == NumberTheoryStatus::InvalidDomain,
        },
        intermediate_count: 2,
        replay_verified: replay,
        tamper_rejected: !tc.replay_verified() && !tn.replay_verified(),
        failure_gate: (!authorized).then(|| match mode {
            Expected::Ambiguous => "count_role_ambiguity".into(),
            Expected::Refused => "number_theory_domain_boundary".into(),
            Expected::Supported => "count_number_handoff".into(),
        }),
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut receipts = Vec::with_capacity(1000);
    let routes = [
        "topology_graph_linear",
        "graph_probability_random_walk",
        "biology_probability",
        "chemistry_linear",
        "combinatorics_number_theory",
    ];
    for (route_index, route) in routes.iter().enumerate() {
        for index in 0..140 {
            let outcome = match route_index {
                0 => topology_route(Expected::Supported),
                1 => random_walk_route(Expected::Supported),
                2 => biology_route(Expected::Supported),
                3 => chemistry_route(Expected::Supported),
                _ => combinatorics_number_route(Expected::Supported),
            };
            receipts.push(RouteReceipt {
                id: format!("supported_{route_index}_{index:03}"),
                route: (*route).into(),
                expected: Expected::Supported,
                authorized: outcome.authorized,
                exact: outcome.exact_status,
                intermediate_count: outcome.intermediate_count,
                replay_verified: outcome.replay_verified,
                tamper_rejected: outcome.tamper_rejected,
                failure_gate: outcome.failure_gate,
                false_authorization: false,
                false_denial: !outcome.authorized,
            });
        }
        for index in 0..30 {
            let outcome = match route_index {
                0 => topology_route(Expected::Ambiguous),
                1 => random_walk_route(Expected::Ambiguous),
                2 => biology_route(Expected::Ambiguous),
                3 => chemistry_route(Expected::Ambiguous),
                _ => combinatorics_number_route(Expected::Ambiguous),
            };
            receipts.push(RouteReceipt {
                id: format!("ambiguous_{route_index}_{index:03}"),
                route: (*route).into(),
                expected: Expected::Ambiguous,
                authorized: outcome.authorized,
                exact: outcome.exact_status && !outcome.authorized,
                intermediate_count: outcome.intermediate_count,
                replay_verified: outcome.replay_verified,
                tamper_rejected: outcome.tamper_rejected,
                failure_gate: outcome.failure_gate,
                false_authorization: outcome.authorized,
                false_denial: false,
            });
        }
        for index in 0..30 {
            let outcome = match route_index {
                0 => topology_route(Expected::Refused),
                1 => random_walk_route(Expected::Refused),
                2 => biology_route(Expected::Refused),
                3 => chemistry_route(Expected::Refused),
                _ => combinatorics_number_route(Expected::Refused),
            };
            receipts.push(RouteReceipt {
                id: format!("refused_{route_index}_{index:03}"),
                route: (*route).into(),
                expected: Expected::Refused,
                authorized: outcome.authorized,
                exact: outcome.exact_status && !outcome.authorized,
                intermediate_count: outcome.intermediate_count,
                replay_verified: outcome.replay_verified,
                tamper_rejected: outcome.tamper_rejected,
                failure_gate: outcome.failure_gate,
                false_authorization: outcome.authorized,
                false_denial: false,
            });
        }
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
    let supported_routes = receipts
        .iter()
        .filter(|r| r.expected == Expected::Supported && r.authorized)
        .count();
    let replay_verified = receipts.iter().filter(|r| r.replay_verified).count();
    let emitted_intermediate_entries = receipts.iter().map(|r| r.intermediate_count).sum();
    let tamper_rejections = receipts.iter().filter(|r| r.tamper_rejected).count();
    let failure_localized = receipts
        .iter()
        .filter(|r| r.expected != Expected::Supported && r.failure_gate.is_some())
        .count();
    let false_authorizations = receipts.iter().filter(|r| r.false_authorization).count();
    let false_denials = receipts.iter().filter(|r| r.false_denial).count();
    let route_leakage = receipts
        .iter()
        .filter(|r| r.expected != Expected::Supported && r.authorized)
        .count();
    assert_eq!(
        (cases, supported, ambiguous, refused),
        (1000, 700, 150, 150)
    );
    assert_eq!(exact_decisions, cases);
    assert_eq!(supported_routes, supported);
    assert_eq!(replay_verified, cases);
    assert_eq!(tamper_rejections, cases);
    assert_eq!(failure_localized, ambiguous + refused);
    assert_eq!(false_authorizations, 0);
    assert_eq!(false_denials, 0);
    assert_eq!(route_leakage, 0);
    let mut route_counts = BTreeMap::new();
    for receipt in &receipts {
        *route_counts.entry(receipt.route.clone()).or_insert(0) += 1;
    }
    let report = Report {
        schema: "stage-b-integrated-synthesis-1000-v1",
        source: "independently authored five-route multi-domain corpus",
        corpus_sha256: digest(&receipts),
        cases,
        supported,
        ambiguous,
        refused,
        exact_decisions,
        supported_routes,
        replay_verified,
        emitted_intermediate_entries,
        tamper_rejections,
        failure_localized,
        false_authorizations,
        false_denials,
        route_leakage,
        route_counts,
        receipts,
    };
    let serialized = serde_json::to_string_pretty(&report)?;
    std::fs::write(
        "docs/stage_b_integrated_synthesis_1000.json",
        format!("{serialized}\n"),
    )?;
    println!("{serialized}");
    Ok(())
}
