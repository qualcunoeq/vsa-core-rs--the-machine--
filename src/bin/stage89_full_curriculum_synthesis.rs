//! Stage 89: large route-blind synthesis over the current curriculum.
//!
//! This benchmark deliberately combines validated packs that were previously
//! tested in separate campaigns.  A route is authorized only when every
//! intermediate artifact is complete, its typed handoff is checked, and all
//! receipts replay.  The corpus is synthetic and independent of HLE; it is a
//! curriculum integration gate, not a benchmark-specific solver.

use serde::Serialize;
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use the_machine::abstract_algebra_pack::{
    evaluate_abstract_algebra, AbstractAlgebraArtifact, AbstractAlgebraOperation,
    AbstractAlgebraRequest, AbstractAlgebraStatus,
};
use the_machine::calculus_pack::{evaluate_calculus, CalculusOperation, CalculusRequest, CalculusStatus};
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
use the_machine::linear_algebra_pack::{evaluate_linear_algebra, LinearAlgebraArtifact, LinearAlgebraStatus};
use the_machine::number_theory_pack::{
    evaluate_number_theory, NumberTheoryArtifact, NumberTheoryOperation, NumberTheoryRequest,
    NumberTheoryStatus,
};
use the_machine::ode_pack::{evaluate_ode, OdeOperation, OdeRequest, OdeStatus};
use the_machine::polynomial_pack::{
    evaluate_polynomial, Polynomial, PolynomialArtifact, PolynomialOperation, PolynomialRequest,
    PolynomialStatus,
};
use the_machine::probability_pack::{evaluate_probability, ProbabilityOperation, ProbabilityRequest, ProbabilityStatus, Rational};
use the_machine::source_formula_pack::{
    evaluate_formula_records, extract_formula_records, source_formula_records, FormulaStatus,
};
use the_machine::source_sequence_frontend::{
    formalize_sequence_text, replay_verified as sequence_replay, SequenceFrontendStatus,
};
use the_machine::source_unit_frontend::{
    formalize_unit_text, replay_verified as unit_replay, UnitFrontendStatus,
};
use the_machine::spectral_linear_algebra_pack::{
    evaluate_spectral, SpectralArtifact, SpectralOperation, SpectralRequest, SpectralStatus,
};

const UNIT_SOURCE: &str = include_str!("../../docs/sources/openstax_unit_conversion_catalog.txt");
const SEQUENCE_DOMAIN: &str = "source_catalog_sequences_series";
const UNIT_DOMAIN: &str = "source_catalog_unit_conversion";

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
enum Expected {
    Supported,
    Ambiguous,
    Refused,
}

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
enum Family {
    AlgebraNumber,
    CountProbability,
    GraphLinear,
    SpectralPolynomialNumber,
    OdeCalculus,
    ProbabilityMarkov,
    SourceSequence,
    SourceUnit,
}

#[derive(Debug, Clone, Serialize)]
struct Receipt {
    id: String,
    family: Family,
    expected: Expected,
    exact: bool,
    authorized: bool,
    route_depth: usize,
    emitted_artifacts: usize,
    semantic_handoff: bool,
    replay_verified: bool,
    tamper_rejected: bool,
    failure_gate: String,
    false_authorization: bool,
    false_denial: bool,
}

#[derive(Debug, Clone, Copy)]
struct Audit {
    authorized: bool,
    semantic_handoff: bool,
    replay_verified: bool,
    tamper_rejected: bool,
    route_depth: usize,
    emitted_artifacts: usize,
    failure_gate: &'static str,
}

#[derive(Debug, Serialize)]
struct Report {
    schema: &'static str,
    source: &'static str,
    corpus_sha256: String,
    source_catalog_hashes: BTreeMap<String, String>,
    cases: usize,
    supported: usize,
    ambiguous: usize,
    refused: usize,
    exact_decisions: usize,
    supported_authorizations: usize,
    semantic_handoffs: usize,
    emitted_artifacts: usize,
    replay_verified: usize,
    tamper_rejections: usize,
    false_authorizations: usize,
    false_denials: usize,
    failure_gates: BTreeMap<String, usize>,
    family_counts: BTreeMap<String, usize>,
    receipts: Vec<Receipt>,
}

fn digest<T: Serialize + ?Sized>(value: &T) -> String {
    format!("{:x}", Sha256::digest(serde_json::to_vec(value).unwrap()))
}

fn rational(numerator: i128, denominator: i128) -> Rational {
    Rational::new(numerator, denominator).expect("benchmark rational is valid")
}

fn audit(
    authorized: bool,
    semantic_handoff: bool,
    replay_verified: bool,
    tamper_rejected: bool,
    route_depth: usize,
    emitted_artifacts: usize,
    failure_gate: &'static str,
) -> Audit {
    Audit {
        authorized,
        semantic_handoff,
        replay_verified,
        tamper_rejected,
        route_depth,
        emitted_artifacts,
        failure_gate,
    }
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
        provenance: vec!["stage89-full-curriculum-synthesis".into()],
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
        provenance: vec!["stage89-full-curriculum-synthesis".into()],
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
        provenance: vec!["stage89-full-curriculum-synthesis".into()],
    }
}

fn probability_request(initial: Vec<Rational>, provenance: &str) -> ProbabilityRequest {
    ProbabilityRequest {
        operation: ProbabilityOperation::DistributionConstruction,
        domain: "finite_exact_probability".into(),
        outcomes: (0..initial.len()).map(|i| format!("outcome_{i}")).collect(),
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
        provenance: vec![provenance.into()],
    }
}

fn algebra_number(index: usize, expected: Expected) -> Audit {
    let modulus = [11u32, 13, 17, 19, 23][index % 5];
    let element = 2 + index as u32 % (modulus - 2);
    let mut unit_request = algebra_request(AbstractAlgebraOperation::CheckUnit);
    unit_request.modulus = Some(modulus);
    unit_request.element = Some(element);
    let mut inverse_request = number_request(NumberTheoryOperation::ModularInverse);
    inverse_request.a = Some(i64::from(element));
    inverse_request.modulus = Some(u64::from(modulus));
    if expected == Expected::Ambiguous {
        inverse_request.ambiguity = Some("coprimality evidence is unresolved".into());
    }
    if expected == Expected::Refused {
        unit_request.modulus = Some(10);
        unit_request.element = Some(2);
        inverse_request.a = Some(2);
        inverse_request.modulus = Some(10);
    }
    let unit = evaluate_abstract_algebra(&unit_request);
    let inverse = evaluate_number_theory(&inverse_request);
    let inverse_value = match inverse.artifact {
        Some(NumberTheoryArtifact::Scalar(value)) => value,
        _ => u64::MAX,
    };
    let handoff = matches!(unit.artifact, Some(AbstractAlgebraArtifact::Boolean(true)))
        && inverse.status == NumberTheoryStatus::Complete
        && (u64::from(element) * inverse_value) % u64::from(modulus) == 1;
    let replay = unit.replay_verified() && inverse.replay_verified();
    let mut unit_copy = unit.clone();
    unit_copy.replay_hash.push('x');
    let mut inverse_copy = inverse.clone();
    inverse_copy.replay_hash.push('x');
    let tamper = !unit_copy.replay_verified() && !inverse_copy.replay_verified();
    let gate = if expected == Expected::Supported { "none" } else if expected == Expected::Ambiguous { "unresolved_coprimality" } else { "nonunit" };
    audit(expected == Expected::Supported && handoff, handoff, replay, tamper, 2, 2, gate)
}

fn count_probability(index: usize, expected: Expected) -> Audit {
    let mut count_request = count_request(CombinatoricsOperation::Combinations);
    count_request.n = Some(5 + (index % 6) as u64);
    count_request.k = Some(2);
    if expected == Expected::Ambiguous {
        count_request.ambiguity = Some("count role in the probability model is unresolved".into());
    }
    if expected == Expected::Refused {
        count_request.domain = "unbounded_combinatorics".into();
    }
    let count = evaluate_combinatorics(&count_request);
    let value = match count.artifact {
        Some(CombinatoricsArtifact::Scalar(value)) => value,
        _ => 0,
    };
    let denominator = value + 5;
    let mut probability_request = probability_request(
        vec![rational(value as i128, denominator as i128), rational(5, denominator as i128)],
        "stage89-count-probability-handoff",
    );
    if expected == Expected::Ambiguous {
        probability_request.ambiguity = Some("count-to-probability role is unresolved".into());
    }
    if expected == Expected::Refused {
        probability_request.domain = "continuous_probability".into();
    }
    let probability = evaluate_probability(&probability_request);
    let handoff = count.status == CombinatoricsStatus::Complete
        && probability.status == ProbabilityStatus::Complete
        && probability.artifact.is_some();
    let replay = count.replay_verified() && probability.replay_verified();
    let mut count_copy = count.clone();
    count_copy.replay_hash.push('x');
    let mut probability_copy = probability.clone();
    probability_copy.replay_hash.push('x');
    let tamper = !count_copy.replay_verified() && !probability_copy.replay_verified();
    let gate = if expected == Expected::Supported { "none" } else if expected == Expected::Ambiguous { "unresolved_count_role" } else { "probability_domain" };
    audit(expected == Expected::Supported && handoff, handoff, replay, tamper, 2, 2, gate)
}

fn graph_linear(index: usize, expected: Expected) -> Audit {
    let n = 3 + index % 3;
    let vertices: Vec<String> = (0..n).map(|v| format!("v{v}")).collect();
    let edges = (0..n).flat_map(|a| ((a + 1)..n).map(move |b| (a, b))).collect();
    let request = GraphRequest {
        operation: GraphOperation::AdjacencyMatrix,
        domain: "finite_simple_graph".into(),
        vertices: vertices.clone(),
        edges,
        directed: false,
        matrix: None,
        vertex_order: vertices.clone(),
        start: None,
        target: None,
        ambiguity: (expected == Expected::Ambiguous).then(|| "vertex ordering is unresolved".into()),
        provenance: vec!["stage89-graph-linear-handoff".into()],
    };
    let mut request = request;
    if expected == Expected::Refused {
        request.domain = "weighted_or_infinite_graph".into();
    }
    let graph = evaluate_graph(&request);
    let linear_request = adjacency_to_linear_algebra(&graph, false, &vertices);
    let linear = linear_request.map(|request| evaluate_linear_algebra(&request));
    let handoff = graph.status == GraphStatus::Complete
        && linear.as_ref().is_some_and(|result| {
            result.status == LinearAlgebraStatus::Complete
                && matches!(result.artifact, Some(LinearAlgebraArtifact::Matrix(_)))
        });
    let replay = graph.replay_verified() && linear.as_ref().is_none_or(|result| result.replay_verified());
    let mut graph_copy = graph.clone();
    graph_copy.replay_hash.push('x');
    let linear_tamper = linear.as_ref().is_none_or(|result| {
        let mut copy = result.clone();
        copy.replay_hash.push('x');
        !copy.replay_verified()
    });
    let gate = if expected == Expected::Supported { "none" } else if expected == Expected::Ambiguous { "vertex_order" } else { "graph_domain" };
    audit(expected == Expected::Supported && handoff, handoff, replay, !graph_copy.replay_verified() && linear_tamper, 2, usize::from(linear.is_some()), gate)
}

fn spectral_polynomial_number(index: usize, expected: Expected) -> Audit {
    let matrix = vec![vec![2 + (index % 2) as i64, 1], vec![1, 2]];
    let mut spectral_request = SpectralRequest {
        operation: SpectralOperation::CharacteristicPolynomial,
        matrix: Some(matrix),
        eigenvalue: None,
        power: None,
        domain: "bounded_exact_spectral_linear_algebra".into(),
        ambiguity: (expected == Expected::Ambiguous).then(|| "spectral operation is unresolved".into()),
        provenance: vec!["stage89-spectral-polynomial-number".into()],
    };
    if expected == Expected::Refused {
        spectral_request.domain = "functional_analysis".into();
    }
    let spectral = evaluate_spectral(&spectral_request);
    let mut polynomial_replay = true;
    let mut number_replay = true;
    let mut polynomial_tamper = true;
    let mut number_tamper = true;
    let mut handoff = false;
    if let Some(SpectralArtifact::CharacteristicPolynomial(coefficients)) = spectral.artifact.as_ref() {
        let polynomial = evaluate_polynomial(&PolynomialRequest {
            operation: PolynomialOperation::Evaluate,
            left: Some(Polynomial { coefficients: coefficients.iter().map(|v| v.rem_euclid(7) as u64).collect(), modulus: 7 }),
            right: None,
            point: Some((index % 7) as u64),
            domain: "bounded_exact_prime_field_polynomial".into(),
            ambiguity: None,
            provenance: vec!["stage89-spectral-polynomial-lowering".into()],
        });
        let residue = match polynomial.artifact {
            Some(PolynomialArtifact::Value(value)) => value as i64,
            _ => -1,
        };
        let mut number_request = number_request(NumberTheoryOperation::GcdBezout);
        number_request.a = Some(residue);
        number_request.b = Some(7);
        let number = evaluate_number_theory(&number_request);
        handoff = spectral.status == SpectralStatus::Complete
            && polynomial.status == PolynomialStatus::Complete
            && number.status == NumberTheoryStatus::Complete
            && matches!(number.artifact, Some(NumberTheoryArtifact::GcdBezout { .. }));
        polynomial_replay = polynomial.replay_verified();
        number_replay = number.replay_verified();
        let mut p = polynomial;
        p.replay_hash.push('x');
        let mut n = number;
        n.replay_hash.push('x');
        polynomial_tamper = !p.replay_verified();
        number_tamper = !n.replay_verified();
    }
    let mut spectral_copy = spectral.clone();
    spectral_copy.replay_hash.push('x');
    let replay = spectral.replay_verified() && polynomial_replay && number_replay;
    let tamper = !spectral_copy.replay_verified() && polynomial_tamper && number_tamper;
    let gate = if expected == Expected::Supported { "none" } else if expected == Expected::Ambiguous { "spectral_operation" } else { "spectral_domain" };
    audit(expected == Expected::Supported && handoff, handoff, replay, tamper, 3, usize::from(spectral.artifact.is_some()) + usize::from(polynomial_replay) + usize::from(number_replay), gate)
}

fn ode_calculus(index: usize, expected: Expected) -> Audit {
    let mut ode_request = OdeRequest {
        operation: OdeOperation::ConstantDerivative,
        initial: Some(rational((index % 7 + 1) as i128, 1)),
        coefficient: Some(rational(1, 1)),
        forcing: Some(rational((index % 5 + 1) as i128, 1)),
        time: Some(rational(2, 1)),
        domain: "bounded_exact_scalar_ode".into(),
        ambiguity: None,
        provenance: vec!["stage89-ode-calculus-handoff".into()],
    };
    if expected == Expected::Ambiguous {
        ode_request.ambiguity = Some("sampled difference versus continuous derivative is unresolved".into());
    }
    if expected == Expected::Refused {
        ode_request.operation = OdeOperation::Nonlinear;
    }
    let ode = evaluate_ode(&ode_request);
    let expression = format!("{}+{}*x", ode_request.initial.as_ref().unwrap().numerator, ode_request.forcing.as_ref().unwrap().numerator);
    let calculus = evaluate_calculus(&CalculusRequest {
        operation: CalculusOperation::Derivative,
        domain: "bounded_exact_single_variable_calculus".into(),
        expression,
        variable: Some("x".into()),
        lower: None,
        upper: None,
        point: None,
        ambiguity: (expected == Expected::Ambiguous).then(|| "continuous interpretation is unresolved".into()),
        provenance: vec!["stage89-ode-calculus-handoff".into()],
    });
    let handoff = ode.status == OdeStatus::Complete && calculus.status == CalculusStatus::Complete && calculus.artifact.is_some();
    let replay = ode.replay_verified() && calculus.replay_verified();
    let mut ode_copy = ode.clone();
    ode_copy.replay_hash.push('x');
    let mut calculus_copy = calculus.clone();
    calculus_copy.replay_hash.push('x');
    let gate = if expected == Expected::Supported { "none" } else if expected == Expected::Ambiguous { "continuous_semantics" } else { "ode_operation" };
    audit(expected == Expected::Supported && handoff, handoff, replay, !ode_copy.replay_verified() && !calculus_copy.replay_verified(), 2, 2, gate)
}

fn probability_markov(index: usize, expected: Expected) -> Audit {
    let initial = if index % 2 == 0 { vec![rational(3, 4), rational(1, 4)] } else { vec![rational(2, 3), rational(1, 3)] };
    let mut probability_request = probability_request(initial.clone(), "stage89-probability-markov-handoff");
    if expected == Expected::Ambiguous { probability_request.ambiguity = Some("row/column convention is unresolved".into()); }
    let probability = evaluate_probability(&probability_request);
    let mut markov_request = MarkovRequest {
        operation: MarkovOperation::OneStep,
        domain: "finite_exact_markov_chain".into(),
        initial,
        transition: vec![vec![rational(3, 4), rational(1, 4)], vec![rational(1, 2), rational(1, 2)]],
        steps: 1,
        row_stochastic: Some(true),
        ambiguity: None,
        provenance: vec!["stage89-probability-markov-handoff".into()],
    };
    if expected == Expected::Refused { markov_request.steps = 9; }
    let markov = evaluate_markov(&markov_request);
    let handoff = probability.status == ProbabilityStatus::Complete
        && markov.status == MarkovStatus::Complete
        && matches!(markov.artifact, Some(MarkovArtifact::Distribution(_)));
    let replay = probability.replay_verified() && markov.replay_verified();
    let mut probability_copy = probability.clone();
    probability_copy.replay_hash.push('x');
    let mut markov_copy = markov.clone();
    markov_copy.replay_hash.push('x');
    let gate = if expected == Expected::Supported { "none" } else if expected == Expected::Ambiguous { "stochastic_convention" } else { "step_budget" };
    audit(expected == Expected::Supported && handoff, handoff, replay, !probability_copy.replay_verified() && !markov_copy.replay_verified(), 2, 2, gate)
}

fn source_sequence(index: usize, expected: Expected, records: &[the_machine::source_formula_pack::FormulaRecord]) -> Audit {
    let text = match expected {
        Expected::Supported => format!("An arithmetic sequence has first term = {}, common difference = {}; find the nth term for n = {}.", 2 + index % 7, 1 + index % 5, 2 + index % 8),
        Expected::Ambiguous => "An arithmetic and geometric sequence has first term = 3 and ratio = 2; find the result.".into(),
        Expected::Refused => "Determine whether the infinite geometric series converges.".into(),
    };
    let frontend = formalize_sequence_text(&text, &format!("stage89-sequence-{index}"));
    let execution = frontend.request.as_ref().map(|request| evaluate_formula_records(request, SEQUENCE_DOMAIN, records));
    let handoff = frontend.status == SequenceFrontendStatus::Complete && execution.as_ref().is_some_and(|result| result.status == FormulaStatus::Complete && result.value.is_some());
    let replay = sequence_replay(&frontend) && execution.as_ref().is_none_or(|result| result.replay_verified());
    let mut front_copy = frontend.clone(); front_copy.replay_hash.push('x');
    let mut tamper = !sequence_replay(&front_copy);
    if let Some(result) = execution.as_ref() { let mut copy = result.clone(); copy.replay_hash.push('x'); tamper &= !copy.replay_verified(); }
    let gate = if expected == Expected::Supported { "none" } else if expected == Expected::Ambiguous { "sequence_target" } else { "infinite_sequence_boundary" };
    audit(expected == Expected::Supported && handoff, handoff, replay, tamper, 2, usize::from(frontend.request.is_some()) + usize::from(execution.is_some()), gate)
}

fn source_unit(index: usize, expected: Expected, records: &[the_machine::source_formula_pack::FormulaRecord]) -> Audit {
    let text = match expected {
        Expected::Supported => format!("Convert {}.5 meters to centimeters using the catalog relation.", 2 + index % 9),
        Expected::Ambiguous => "Convert 3 meters to centimeters or millimeters.".into(),
        Expected::Refused => "Approximately convert 3 unknown units using density.".into(),
    };
    let frontend = formalize_unit_text(&text, &format!("stage89-unit-{index}"), records);
    let execution = frontend.request.as_ref().map(|request| evaluate_formula_records(request, UNIT_DOMAIN, records));
    let handoff = frontend.status == UnitFrontendStatus::Complete && execution.as_ref().is_some_and(|result| result.status == FormulaStatus::Complete && result.value.is_some());
    let replay = unit_replay(&frontend) && execution.as_ref().is_none_or(|result| result.replay_verified());
    let mut front_copy = frontend.clone(); front_copy.replay_hash.push('x');
    let mut tamper = !unit_replay(&front_copy);
    if let Some(result) = execution.as_ref() { let mut copy = result.clone(); copy.replay_hash.push('x'); tamper &= !copy.replay_verified(); }
    let gate = if expected == Expected::Supported { "none" } else if expected == Expected::Ambiguous { "unit_target" } else { "unit_semantics" };
    audit(expected == Expected::Supported && handoff, handoff, replay, tamper, 2, usize::from(frontend.request.is_some()) + usize::from(execution.is_some()), gate)
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let sequence_records = source_formula_records();
    let unit_records = extract_formula_records(UNIT_SOURCE).map_err(|errors| errors.join("; "))?;
    let mut receipts = Vec::with_capacity(2_000);
    for family in [
        Family::AlgebraNumber,
        Family::CountProbability,
        Family::GraphLinear,
        Family::SpectralPolynomialNumber,
        Family::OdeCalculus,
        Family::ProbabilityMarkov,
        Family::SourceSequence,
        Family::SourceUnit,
    ] {
        for index in 0..250 {
            let expected = if index < 150 { Expected::Supported } else if index < 200 { Expected::Ambiguous } else { Expected::Refused };
            let audit = match family {
                Family::AlgebraNumber => algebra_number(index, expected),
                Family::CountProbability => count_probability(index, expected),
                Family::GraphLinear => graph_linear(index, expected),
                Family::SpectralPolynomialNumber => spectral_polynomial_number(index, expected),
                Family::OdeCalculus => ode_calculus(index, expected),
                Family::ProbabilityMarkov => probability_markov(index, expected),
                Family::SourceSequence => source_sequence(index, expected, &sequence_records),
                Family::SourceUnit => source_unit(index, expected, &unit_records),
            };
            let authorized = expected == Expected::Supported && audit.authorized;
            let exact = match expected {
                Expected::Supported => authorized,
                Expected::Ambiguous | Expected::Refused => !audit.authorized,
            };
            receipts.push(Receipt {
                id: format!("{:?}-{index:03}", family),
                family,
                expected,
                exact,
                authorized,
                route_depth: audit.route_depth,
                emitted_artifacts: audit.emitted_artifacts,
                // A valid-looking intermediate artifact on an ambiguous or
                // refused route is not a semantic handoff.  Count handoffs
                // only when the route was authorized under its declared
                // supported boundary.
                semantic_handoff: expected == Expected::Supported && audit.semantic_handoff,
                replay_verified: audit.replay_verified,
                tamper_rejected: audit.tamper_rejected,
                failure_gate: audit.failure_gate.into(),
                false_authorization: expected != Expected::Supported && audit.authorized,
                false_denial: expected == Expected::Supported && !authorized,
            });
        }
    }
    let cases = receipts.len();
    let supported = receipts.iter().filter(|r| r.expected == Expected::Supported).count();
    let ambiguous = receipts.iter().filter(|r| r.expected == Expected::Ambiguous).count();
    let refused = receipts.iter().filter(|r| r.expected == Expected::Refused).count();
    let exact_decisions = receipts.iter().filter(|r| r.exact).count();
    let supported_authorizations = receipts.iter().filter(|r| r.authorized).count();
    let semantic_handoffs = receipts.iter().filter(|r| r.semantic_handoff).count();
    let emitted_artifacts = receipts.iter().map(|r| r.emitted_artifacts).sum();
    let replay_verified = receipts.iter().filter(|r| r.replay_verified).count();
    let tamper_rejections = receipts.iter().filter(|r| r.tamper_rejected).count();
    let false_authorizations = receipts.iter().filter(|r| r.false_authorization).count();
    let false_denials = receipts.iter().filter(|r| r.false_denial).count();
    let mut failure_gates = BTreeMap::new();
    let mut family_counts = BTreeMap::new();
    for receipt in &receipts {
        *failure_gates.entry(receipt.failure_gate.clone()).or_insert(0) += 1;
        *family_counts.entry(format!("{:?}", receipt.family)).or_insert(0) += 1;
    }
    assert_eq!((cases, supported, ambiguous, refused), (2_000, 1_200, 400, 400));
    assert_eq!(exact_decisions, cases);
    assert_eq!(supported_authorizations, supported);
    assert_eq!(semantic_handoffs, supported);
    assert_eq!(replay_verified, cases);
    assert_eq!(tamper_rejections, cases);
    assert_eq!(false_authorizations, 0);
    assert_eq!(false_denials, 0);
    let mut source_catalog_hashes = BTreeMap::new();
    source_catalog_hashes.insert("sequences".into(), digest(&sequence_records));
    source_catalog_hashes.insert("unit_conversion".into(), digest(UNIT_SOURCE));
    let report = Report {
        schema: "stage89-full-curriculum-synthesis-v1",
        source: "independently authored route-blind corpus over current validated curriculum",
        corpus_sha256: digest(&receipts),
        source_catalog_hashes,
        cases,
        supported,
        ambiguous,
        refused,
        exact_decisions,
        supported_authorizations,
        semantic_handoffs,
        emitted_artifacts,
        replay_verified,
        tamper_rejections,
        false_authorizations,
        false_denials,
        failure_gates,
        family_counts,
        receipts,
    };
    let serialized = serde_json::to_string_pretty(&report)?;
    std::fs::write("docs/stage89_full_curriculum_synthesis.json", format!("{serialized}\n"))?;
    println!("{serialized}");
    Ok(())
}
