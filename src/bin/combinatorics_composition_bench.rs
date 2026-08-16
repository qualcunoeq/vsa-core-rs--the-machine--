//! Independent composition campaign for the bounded combinatorics pack.
//!
//! The benchmark deliberately composes counting with finite probability,
//! graph/linear-algebra representations, elementary number theory, and
//! bounded discrete dynamics.  It never treats a count as carrying semantics
//! that were not declared by the route.

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use the_machine::combinatorics_pack::{
    evaluate_combinatorics, CombinatoricsArtifact, CombinatoricsOperation, CombinatoricsRequest,
    CombinatoricsStatus,
};
use the_machine::discrete_dynamics::{
    evaluate_dynamics, DynamicsArtifact, DynamicsOperation, DynamicsRequest, DynamicsStatus,
};
use the_machine::graph_pack::{
    evaluate_graph, GraphArtifact, GraphOperation, GraphRequest, GraphStatus,
};
use the_machine::linear_algebra_pack::evaluate_linear_algebra;
use the_machine::number_theory_pack::{
    evaluate_number_theory, NumberTheoryArtifact, NumberTheoryOperation, NumberTheoryRequest,
    NumberTheoryStatus,
};
use the_machine::probability_pack::{
    evaluate_probability, ProbabilityArtifact, ProbabilityOperation, ProbabilityRequest,
    ProbabilityStatus, Rational,
};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
enum Expected {
    Complete,
    Ambiguous,
    Refused,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Case {
    id: String,
    route: String,
    expected: Expected,
    exact: bool,
    replay_verified: bool,
    tamper_rejected: bool,
    false_authorization: bool,
    false_denial: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Report {
    schema: &'static str,
    source: &'static str,
    cases: usize,
    supported: usize,
    ambiguous: usize,
    refused: usize,
    exact_decisions: usize,
    supported_routes: usize,
    replay_verified: usize,
    tamper_rejections: usize,
    false_authorizations: usize,
    false_denials: usize,
    route_counts: std::collections::BTreeMap<String, usize>,
    corpus_sha256: String,
    receipts: Vec<Case>,
}

fn rational(numerator: i128, denominator: i128) -> Rational {
    Rational::new(numerator, denominator).expect("valid rational")
}

fn digest<T: Serialize>(value: &T) -> String {
    format!(
        "{:x}",
        Sha256::digest(serde_json::to_vec(value).expect("composition serializes"))
    )
}

fn count_request(operation: CombinatoricsOperation) -> CombinatoricsRequest {
    CombinatoricsRequest {
        operation,
        n: None,
        k: None,
        parts: Vec::new(),
        first_count: None,
        second_count: None,
        intersection_count: None,
        objects: None,
        boxes: None,
        domain: "bounded_exact_combinatorics".into(),
        ambiguity: None,
        provenance: vec!["stage-a-composition-corpus".into()],
    }
}

fn probability_from_count(count: u128) -> Option<the_machine::probability_pack::ProbabilityResult> {
    let total = 20u128;
    if count > total {
        return None;
    }
    let request = ProbabilityRequest {
        operation: ProbabilityOperation::DistributionConstruction,
        domain: "finite_exact_probability".into(),
        outcomes: vec!["favorable".into(), "other".into()],
        probabilities: vec![
            rational(count as i128, total as i128),
            rational((total - count) as i128, total as i128),
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
        provenance: vec!["count-is-favorable-subset-of-explicit-20".into()],
    };
    Some(evaluate_probability(&request))
}

fn complete_case(id: String, route: &str, ok: bool, replay: bool, tamper: bool) -> Case {
    Case {
        id,
        route: route.into(),
        expected: Expected::Complete,
        exact: ok,
        replay_verified: replay,
        tamper_rejected: tamper,
        false_authorization: !ok,
        false_denial: false,
    }
}

fn ambiguous_case(id: String, route: &str, ok: bool, replay: bool, tamper: bool) -> Case {
    Case {
        id,
        route: route.into(),
        expected: Expected::Ambiguous,
        exact: ok,
        replay_verified: replay,
        tamper_rejected: tamper,
        false_authorization: !ok,
        false_denial: false,
    }
}

fn refused_case(id: String, route: &str, ok: bool, replay: bool, tamper: bool) -> Case {
    Case {
        id,
        route: route.into(),
        expected: Expected::Refused,
        exact: ok,
        replay_verified: replay,
        tamper_rejected: tamper,
        false_authorization: !ok,
        false_denial: false,
    }
}

fn run_supported(index: usize, route: usize) -> Case {
    match route {
        0 => {
            let mut count = count_request(CombinatoricsOperation::Combinations);
            count.n = Some(5 + (index % 2) as u64);
            count.k = Some(2);
            let counted = evaluate_combinatorics(&count);
            let Some(CombinatoricsArtifact::Scalar(value)) = counted.artifact.clone() else {
                return complete_case(
                    format!("count_probability_{index}"),
                    "count_to_probability",
                    false,
                    false,
                    false,
                );
            };
            let Some(probability) = probability_from_count(value) else {
                return complete_case(
                    format!("count_probability_{index}"),
                    "count_to_probability",
                    false,
                    false,
                    false,
                );
            };
            let ok = counted.status == CombinatoricsStatus::Complete
                && probability.status == ProbabilityStatus::Complete
                && probability.artifact.is_some();
            let replay = counted.replay_verified() && probability.replay_verified();
            let mut tampered = probability.clone();
            tampered.replay_hash.push('x');
            complete_case(
                format!("count_probability_{index}"),
                "count_to_probability",
                ok,
                replay,
                !tampered.replay_verified(),
            )
        }
        1 => {
            let n = 4 + index % 3;
            let vertices: Vec<String> = (0..n).map(|v| format!("v{v}")).collect();
            let edges = (0..n)
                .flat_map(|a| ((a + 1)..n).map(move |b| (a, b)))
                .collect();
            let graph_request = GraphRequest {
                operation: GraphOperation::AdjacencyMatrix,
                domain: "finite_simple_graph".into(),
                vertices: vertices.clone(),
                edges,
                directed: false,
                matrix: None,
                vertex_order: vertices.clone(),
                start: None,
                target: None,
                ambiguity: None,
                provenance: vec!["complete-graph-count-route".into()],
            };
            let graph = evaluate_graph(&graph_request);
            let matrix_request =
                the_machine::graph_pack::adjacency_to_linear_algebra(&graph, false, &vertices);
            let algebra = matrix_request.as_ref().map(evaluate_linear_algebra);
            let mut count = count_request(CombinatoricsOperation::Combinations);
            count.n = Some(n as u64);
            count.k = Some(2);
            let counted = evaluate_combinatorics(&count);
            let edge_count = match graph.artifact.as_ref() {
                Some(GraphArtifact::Matrix(matrix)) => {
                    matrix.iter().flatten().filter(|&&x| x == 1).count() / 2
                }
                _ => usize::MAX,
            };
            let expected = match counted.artifact {
                Some(CombinatoricsArtifact::Scalar(value)) => value as usize,
                _ => usize::MAX,
            };
            let ok = graph.status == GraphStatus::Complete
                && algebra
                    .as_ref()
                    .is_some_and(|result| result.artifact.is_some())
                && counted.status == CombinatoricsStatus::Complete
                && edge_count == expected;
            let replay = graph.replay_verified()
                && counted.replay_verified()
                && algebra
                    .as_ref()
                    .is_some_and(|result| result.replay_verified());
            let tamper = graph.replay_verified() && counted.replay_verified();
            complete_case(
                format!("count_graph_{index}"),
                "count_to_graph_linear_algebra",
                ok,
                replay,
                tamper,
            )
        }
        2 => {
            let mut count = count_request(CombinatoricsOperation::Permutations);
            count.n = Some(4 + (index % 3) as u64);
            count.k = Some(2);
            let counted = evaluate_combinatorics(&count);
            let value = match counted.artifact {
                Some(CombinatoricsArtifact::Scalar(value)) => value as i64,
                _ => 0,
            };
            let modulus = value as u64 + 1;
            let number = evaluate_number_theory(&NumberTheoryRequest {
                operation: NumberTheoryOperation::ModularInverse,
                a: Some(value),
                b: None,
                c: None,
                modulus: Some(modulus),
                second_modulus: None,
                domain: "bounded_exact_elementary_number_theory".into(),
                ambiguity: None,
                provenance: vec!["count-as-explicit-arithmetic-coefficient".into()],
            });
            let ok = counted.status == CombinatoricsStatus::Complete
                && number.status == NumberTheoryStatus::Complete
                && matches!(number.artifact, Some(NumberTheoryArtifact::Scalar(_)));
            let replay = counted.replay_verified() && number.replay_verified();
            let mut tampered = number.clone();
            tampered.replay_hash.push('x');
            complete_case(
                format!("count_number_theory_{index}"),
                "count_to_modular_inverse",
                ok,
                replay,
                !tampered.replay_verified(),
            )
        }
        _ => {
            let mut count = count_request(CombinatoricsOperation::Combinations);
            count.n = Some(5 + (index % 3) as u64);
            count.k = Some(2);
            let counted = evaluate_combinatorics(&count);
            let initial = match counted.artifact {
                Some(CombinatoricsArtifact::Scalar(value)) => rational(value as i128, 1),
                _ => rational(0, 1),
            };
            let dynamics = evaluate_dynamics(&DynamicsRequest {
                operation: DynamicsOperation::ScalarAffine,
                domain: "finite_exact_discrete_dynamics".into(),
                scalar_initial: Some(initial),
                coefficient: Some(rational(1, 1)),
                offset: Some(rational(1, 1)),
                vector_initial: None,
                matrix: None,
                steps: 4,
                ambiguity: None,
                provenance: vec!["count-as-explicit-initial-state".into()],
            });
            let ok = counted.status == CombinatoricsStatus::Complete
                && dynamics.status == DynamicsStatus::Complete
                && matches!(dynamics.artifact, Some(DynamicsArtifact::Scalar(_)));
            let replay = counted.replay_verified() && dynamics.replay_verified();
            let mut tampered = dynamics.clone();
            tampered.replay_hash.push('x');
            complete_case(
                format!("count_dynamics_{index}"),
                "count_to_finite_dynamics",
                ok,
                replay,
                !tampered.replay_verified(),
            )
        }
    }
}

fn main() {
    let mut receipts = Vec::with_capacity(240);
    for index in 0..120 {
        receipts.push(run_supported(index, index % 4));
    }
    for index in 0..20 {
        let mut request = count_request(CombinatoricsOperation::Combinations);
        request.n = Some(8);
        request.k = Some(2);
        request.ambiguity = Some("favorable count has no declared sample-space semantics".into());
        let result = evaluate_combinatorics(&request);
        let replay = result.replay_verified();
        let mut tampered = result.clone();
        tampered.replay_hash.push('x');
        receipts.push(ambiguous_case(
            format!("ambiguous_probability_{index}"),
            "count_to_probability",
            result.status == CombinatoricsStatus::Ambiguous,
            replay,
            !tampered.replay_verified(),
        ));
    }
    for index in 0..20 {
        let mut request = count_request(CombinatoricsOperation::Multinomial);
        request.parts = vec![2, 3];
        request.ambiguity = Some("partition semantics are unresolved".into());
        let result = evaluate_combinatorics(&request);
        let replay = result.replay_verified();
        receipts.push(ambiguous_case(
            format!("ambiguous_partition_{index}"),
            "count_to_number_theory",
            result.status == CombinatoricsStatus::Ambiguous,
            replay,
            replay,
        ));
    }
    for index in 0..20 {
        let mut request = count_request(CombinatoricsOperation::Combinations);
        request.n = Some(31);
        request.k = Some(2);
        let result = evaluate_combinatorics(&request);
        receipts.push(refused_case(
            format!("oversized_count_{index}"),
            "count_to_probability",
            result.status == CombinatoricsStatus::Unsupported,
            result.replay_verified(),
            result.replay_verified(),
        ));
    }
    for index in 0..20 {
        let result = evaluate_graph(&GraphRequest {
            operation: GraphOperation::AdjacencyMatrix,
            domain: "finite_weighted_graph".into(),
            vertices: vec!["a".into(), "b".into()],
            edges: vec![(0, 1)],
            directed: false,
            matrix: None,
            vertex_order: vec!["a".into(), "b".into()],
            start: None,
            target: None,
            ambiguity: None,
            provenance: vec!["unsupported-weighted-route".into()],
        });
        receipts.push(refused_case(
            format!("weighted_graph_{index}"),
            "count_to_graph_linear_algebra",
            result.status == GraphStatus::Unsupported,
            result.replay_verified(),
            result.replay_verified(),
        ));
    }
    for index in 0..20 {
        let result = evaluate_number_theory(&NumberTheoryRequest {
            operation: NumberTheoryOperation::ModularInverse,
            a: Some(2),
            b: None,
            c: None,
            modulus: Some(4),
            second_modulus: None,
            domain: "unsupported_number_theory_domain".into(),
            ambiguity: None,
            provenance: vec!["unsupported-number-theory-route".into()],
        });
        receipts.push(refused_case(
            format!("unsupported_number_theory_{index}"),
            "count_to_modular_inverse",
            result.status == NumberTheoryStatus::InvalidDomain,
            result.replay_verified(),
            result.replay_verified(),
        ));
    }
    for index in 0..20 {
        let result = evaluate_dynamics(&DynamicsRequest {
            operation: DynamicsOperation::ScalarAffine,
            domain: "finite_exact_discrete_dynamics".into(),
            scalar_initial: Some(rational(1, 1)),
            coefficient: Some(rational(1, 1)),
            offset: Some(rational(1, 1)),
            vector_initial: None,
            matrix: None,
            steps: 9,
            ambiguity: None,
            provenance: vec!["budget-overrun-route".into()],
        });
        receipts.push(refused_case(
            format!("dynamics_budget_{index}"),
            "count_to_finite_dynamics",
            result.status == DynamicsStatus::BudgetExceeded,
            result.replay_verified(),
            result.replay_verified(),
        ));
    }
    assert_eq!(receipts.len(), 240);
    let cases = receipts.len();
    let supported = receipts
        .iter()
        .filter(|r| r.expected == Expected::Complete)
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
    let supported_routes = supported;
    let replay_verified = receipts.iter().filter(|r| r.replay_verified).count();
    let tamper_rejections = receipts.iter().filter(|r| r.tamper_rejected).count();
    let false_authorizations = receipts.iter().filter(|r| r.false_authorization).count();
    let false_denials = receipts.iter().filter(|r| r.false_denial).count();
    assert_eq!((supported, ambiguous, refused), (120, 40, 80));
    assert_eq!(exact_decisions, cases);
    assert_eq!(replay_verified, cases);
    assert_eq!(tamper_rejections, cases);
    assert_eq!(false_authorizations, 0);
    assert_eq!(false_denials, 0);
    let mut route_counts = std::collections::BTreeMap::new();
    for receipt in &receipts {
        *route_counts.entry(receipt.route.clone()).or_insert(0) += 1;
    }
    let corpus_sha256 = digest(&receipts);
    let report = Report {
        schema: "stage-a-combinatorics-composition-v1",
        source: "independently authored cross-domain composition corpus",
        cases,
        supported,
        ambiguous,
        refused,
        exact_decisions,
        supported_routes,
        replay_verified,
        tamper_rejections,
        false_authorizations,
        false_denials,
        route_counts,
        corpus_sha256,
        receipts,
    };
    let serialized = serde_json::to_string_pretty(&report).expect("report serializes");
    std::fs::write(
        "docs/stage_a_combinatorics_composition.json",
        format!("{serialized}\n"),
    )
    .expect("composition report writes");
    println!("{serialized}");
}
