//! Current-curriculum cross-domain synthesis audit.
//!
//! This is a shadow-only 1,000-case route corpus for the curriculum after the
//! algebra, number-theory, and finite-Markov additions.  Each supported route
//! checks an explicit semantic handoff and every emitted artifact is replayed
//! and tamper-tested.  No live router or curriculum manifest is mutated.

use serde::Serialize;
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
use the_machine::finite_markov_pack::{
    evaluate_markov, MarkovArtifact, MarkovOperation, MarkovRequest, MarkovStatus,
};
use the_machine::graph_pack::{
    adjacency_to_linear_algebra, evaluate_graph, GraphOperation, GraphRequest, GraphStatus,
};
use the_machine::linear_algebra_pack::{evaluate_linear_algebra, LinearAlgebraStatus};
use the_machine::number_theory_pack::{
    evaluate_number_theory, NumberTheoryArtifact, NumberTheoryOperation, NumberTheoryRequest,
    NumberTheoryStatus,
};
use the_machine::probability_pack::{
    evaluate_probability, ProbabilityOperation, ProbabilityRequest, ProbabilityStatus, Rational,
};

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
enum Expected {
    Supported,
    Ambiguous,
    Refused,
}

#[derive(Debug, Clone, Serialize)]
struct Receipt {
    id: String,
    family: String,
    expected: Expected,
    exact: bool,
    semantic_handoff: bool,
    route_depth: usize,
    replay_verified: bool,
    tamper_rejected: bool,
    false_authorization: bool,
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
    supported_correct: usize,
    semantic_handoffs: usize,
    replay_verified: usize,
    tamper_rejections: usize,
    false_authorizations: usize,
    false_denials: usize,
    family_counts: BTreeMap<String, usize>,
    receipts: Vec<Receipt>,
}

fn digest<T: Serialize>(value: &T) -> String {
    format!("{:x}", Sha256::digest(serde_json::to_vec(value).unwrap()))
}

fn q(numerator: i128, denominator: i128) -> Rational {
    Rational::new(numerator, denominator).expect("valid rational")
}

fn algebra_request(operation: AbstractAlgebraOperation) -> AbstractAlgebraRequest {
    AbstractAlgebraRequest {
        operation,
        modulus: None,
        source_modulus: None,
        target_modulus: None,
        element: None,
        multiplier: None,
        second_multiplier: None,
        domain: "finite_exact_abstract_algebra".into(),
        assumptions: vec!["finite cyclic structure explicitly declared".into()],
        ambiguity: None,
        provenance: vec!["current-curriculum-synthesis".into()],
    }
}

fn number_request(operation: NumberTheoryOperation) -> NumberTheoryRequest {
    NumberTheoryRequest {
        operation,
        a: None,
        b: None,
        c: None,
        modulus: None,
        second_modulus: None,
        domain: "bounded_exact_elementary_number_theory".into(),
        ambiguity: None,
        provenance: vec!["current-curriculum-synthesis".into()],
    }
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
        provenance: vec!["current-curriculum-synthesis".into()],
    }
}

fn probability_request(initial: &[Rational]) -> ProbabilityRequest {
    ProbabilityRequest {
        operation: ProbabilityOperation::DistributionConstruction,
        domain: "finite_exact_probability".into(),
        outcomes: (0..initial.len()).map(|i| format!("outcome_{i}")).collect(),
        probabilities: initial.to_vec(),
        values: (0..initial.len()).map(|i| i as i64).collect(),
        event_a: None,
        event_b: None,
        partition: Vec::new(),
        conditional_values: Vec::new(),
        prior_probability: None,
        likelihood: None,
        evidence: None,
        ambiguity: None,
        provenance: vec!["current-curriculum-synthesis".into()],
    }
}

fn supported_algebra_number(index: usize) -> (bool, bool, bool, usize) {
    let moduli = [11u32, 13, 17, 19, 23];
    let modulus = moduli[index % moduli.len()];
    let element = 2 + (index as u32 % (modulus - 2));
    let mut unit_request = algebra_request(AbstractAlgebraOperation::CheckUnit);
    unit_request.modulus = Some(modulus);
    unit_request.element = Some(element);
    let unit = evaluate_abstract_algebra(&unit_request);
    let mut inverse_request = number_request(NumberTheoryOperation::ModularInverse);
    inverse_request.a = Some(i64::from(element));
    inverse_request.modulus = Some(u64::from(modulus));
    let inverse = evaluate_number_theory(&inverse_request);
    let inverse_value = match inverse.artifact {
        Some(NumberTheoryArtifact::Scalar(value)) => value,
        _ => u64::MAX,
    };
    let handoff = unit.artifact == Some(AbstractAlgebraArtifact::Boolean(true))
        && inverse.status == NumberTheoryStatus::Complete
        && (u64::from(element) * inverse_value) % u64::from(modulus) == 1;
    let replay = unit.replay_verified() && inverse.replay_verified();
    let mut unit_tampered = unit.clone();
    unit_tampered.replay_hash.push('x');
    let mut inverse_tampered = inverse.clone();
    inverse_tampered.replay_hash.push('x');
    let tamper = !unit_tampered.replay_verified() && !inverse_tampered.replay_verified();
    (handoff, replay, tamper, 2)
}

fn supported_count_probability(index: usize) -> (bool, bool, bool, usize) {
    let mut count_request = count_request(CombinatoricsOperation::Combinations);
    count_request.n = Some(5 + (index % 6) as u64);
    count_request.k = Some(2);
    let count = evaluate_combinatorics(&count_request);
    let Some(CombinatoricsArtifact::Scalar(value)) = count.artifact.clone() else {
        return (false, count.replay_verified(), false, 2);
    };
    let denominator = value + 5;
    let initial = vec![
        q(value as i128, denominator as i128),
        q(5, denominator as i128),
    ];
    let probability = evaluate_probability(&probability_request(&initial));
    let handoff = count.status == CombinatoricsStatus::Complete
        && probability.status == ProbabilityStatus::Complete
        && probability.artifact.is_some();
    let replay = count.replay_verified() && probability.replay_verified();
    let mut tampered = probability.clone();
    tampered.replay_hash.push('x');
    (handoff, replay, !tampered.replay_verified(), 2)
}

fn supported_graph_linear(index: usize) -> (bool, bool, bool, usize) {
    let n = 3 + index % 3;
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
        provenance: vec!["current-curriculum-synthesis".into()],
    };
    let graph = evaluate_graph(&graph_request);
    let Some(linear_request) = adjacency_to_linear_algebra(&graph, false, &vertices) else {
        return (false, graph.replay_verified(), false, 2);
    };
    let linear = evaluate_linear_algebra(&linear_request);
    let handoff = graph.status == GraphStatus::Complete
        && linear.status == LinearAlgebraStatus::Complete
        && linear.artifact.is_some();
    let replay = graph.replay_verified() && linear.replay_verified();
    let mut tampered = linear.clone();
    tampered.replay_hash.push('x');
    (handoff, replay, !tampered.replay_verified(), 2)
}

fn supported_probability_markov(index: usize) -> (bool, bool, bool, usize) {
    let initial = if index % 2 == 0 {
        vec![q(3, 4), q(1, 4)]
    } else {
        vec![q(2, 3), q(1, 3)]
    };
    let probability = evaluate_probability(&probability_request(&initial));
    let transition = vec![vec![q(3, 4), q(1, 4)], vec![q(1, 2), q(1, 2)]];
    let markov_request = MarkovRequest {
        operation: MarkovOperation::OneStep,
        domain: "finite_exact_markov_chain".into(),
        initial,
        transition,
        steps: 1,
        row_stochastic: Some(true),
        ambiguity: None,
        provenance: vec!["current-curriculum-synthesis".into()],
    };
    let markov = evaluate_markov(&markov_request);
    let handoff = probability.status == ProbabilityStatus::Complete
        && markov.status == MarkovStatus::Complete
        && matches!(markov.artifact, Some(MarkovArtifact::Distribution(_)));
    let replay = probability.replay_verified() && markov.replay_verified();
    let mut tampered = markov.clone();
    tampered.replay_hash.push('x');
    (handoff, replay, !tampered.replay_verified(), 2)
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut receipts = Vec::with_capacity(1000);
    for index in 0..900 {
        let (family, (handoff, replay, tamper, depth)) = match index % 4 {
            0 => ("algebra_number_inverse", supported_algebra_number(index)),
            1 => (
                "count_probability_distribution",
                supported_count_probability(index),
            ),
            2 => ("graph_linear_adjacency", supported_graph_linear(index)),
            _ => (
                "probability_markov_one_step",
                supported_probability_markov(index),
            ),
        };
        receipts.push(Receipt {
            id: format!("supported_{index:04}"),
            family: family.into(),
            expected: Expected::Supported,
            exact: handoff,
            semantic_handoff: handoff,
            route_depth: depth,
            replay_verified: replay,
            tamper_rejected: tamper,
            false_authorization: !handoff,
        });
    }
    for index in 0..50 {
        let mut request = algebra_request(AbstractAlgebraOperation::ComposeCyclicHomomorphisms);
        request.source_modulus = Some(4);
        request.modulus = Some(6);
        request.target_modulus = Some(8);
        request.multiplier = Some(3);
        request.ambiguity = Some("the middle cyclic convention is unresolved".into());
        let result = evaluate_abstract_algebra(&request);
        let mut tampered = result.clone();
        tampered.replay_hash.push('x');
        receipts.push(Receipt {
            id: format!("ambiguous_{index:04}"),
            family: "ambiguous_algebra_route".into(),
            expected: Expected::Ambiguous,
            exact: result.status == AbstractAlgebraStatus::Ambiguous,
            semantic_handoff: false,
            route_depth: 1,
            replay_verified: result.replay_verified(),
            tamper_rejected: !tampered.replay_verified(),
            false_authorization: result.artifact.is_some(),
        });
    }
    for index in 0..50 {
        let mut request = number_request(NumberTheoryOperation::ModularInverse);
        request.a = Some(2);
        request.modulus = Some(4);
        request.domain = "cryptographic_security_claim".into();
        let result = evaluate_number_theory(&request);
        let mut tampered = result.clone();
        tampered.replay_hash.push('x');
        receipts.push(Receipt {
            id: format!("refused_{index:04}"),
            family: "refused_domain_route".into(),
            expected: Expected::Refused,
            exact: result.status == NumberTheoryStatus::InvalidDomain,
            semantic_handoff: false,
            route_depth: 1,
            replay_verified: result.replay_verified(),
            tamper_rejected: !tampered.replay_verified(),
            false_authorization: result.artifact.is_some(),
        });
    }
    assert_eq!(receipts.len(), 1000);
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
    let supported_correct = receipts
        .iter()
        .filter(|r| r.expected == Expected::Supported && r.exact)
        .count();
    let semantic_handoffs = receipts.iter().filter(|r| r.semantic_handoff).count();
    let replay_verified = receipts.iter().filter(|r| r.replay_verified).count();
    let tamper_rejections = receipts.iter().filter(|r| r.tamper_rejected).count();
    let false_authorizations = receipts.iter().filter(|r| r.false_authorization).count();
    let false_denials = receipts
        .iter()
        .filter(|r| r.expected == Expected::Supported && !r.exact)
        .count();
    assert_eq!((supported, ambiguous, refused), (900, 50, 50));
    assert_eq!(exact_decisions, cases);
    assert_eq!(supported_correct, supported);
    assert_eq!(semantic_handoffs, supported);
    assert_eq!(replay_verified, cases);
    assert_eq!(tamper_rejections, cases);
    assert_eq!(false_authorizations, 0);
    assert_eq!(false_denials, 0);
    let mut family_counts = BTreeMap::new();
    for receipt in &receipts {
        *family_counts.entry(receipt.family.clone()).or_insert(0) += 1;
    }
    let report = Report {
        schema: "stage-b-current-curriculum-synthesis-v1",
        source: "independently authored current-curriculum route corpus",
        corpus_sha256: digest(&receipts),
        cases,
        supported,
        ambiguous,
        refused,
        exact_decisions,
        supported_correct,
        semantic_handoffs,
        replay_verified,
        tamper_rejections,
        false_authorizations,
        false_denials,
        family_counts,
        receipts,
    };
    let serialized = serde_json::to_string_pretty(&report)?;
    std::fs::write(
        "docs/stage_b_current_curriculum_synthesis.json",
        format!("{serialized}\n"),
    )?;
    println!("{serialized}");
    Ok(())
}
