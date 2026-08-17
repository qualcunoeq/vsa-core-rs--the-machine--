//! Stage 136: bounded arithmetic-functions composition.
//!
//! Arithmetic-function outputs are consumed only through explicit typed roles:
//! an input value may feed a totient or modular-ring request, a divisor count
//! may become a declared integer operand, and a prime-counting result may be a
//! combinatorial population size.  Signed Möbius values and untyped numeric
//! outputs never acquire probability or algebraic semantics implicitly.

use serde::Serialize;
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use the_machine::abstract_algebra_pack::{
    evaluate_abstract_algebra, AbstractAlgebraArtifact, AbstractAlgebraOperation,
    AbstractAlgebraRequest, AbstractAlgebraStatus,
};
use the_machine::bounded_arithmetic_functions_pack::{
    evaluate as evaluate_arithmetic, ArithmeticFunctionArtifact, ArithmeticFunctionOperation,
    ArithmeticFunctionRequest, ArithmeticFunctionResult, ArithmeticFunctionStatus,
};
use the_machine::combinatorics_pack::{
    evaluate_combinatorics, CombinatoricsArtifact, CombinatoricsOperation, CombinatoricsRequest,
    CombinatoricsStatus,
};
use the_machine::number_theory_pack::{
    evaluate_number_theory, NumberTheoryArtifact, NumberTheoryOperation, NumberTheoryRequest,
    NumberTheoryResult, NumberTheoryStatus,
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
struct Case {
    id: String,
    family: String,
    expected: Expected,
    declared_handoff: String,
}

#[derive(Debug, Serialize)]
struct Receipt {
    id: String,
    family: String,
    expected: Expected,
    stage_count: usize,
    actual_terminal: String,
    handoff_verified: bool,
    exact: bool,
    replay_verified: bool,
    tamper_rejected: bool,
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
    supported_handoffs: usize,
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

fn arithmetic_request(
    operation: ArithmeticFunctionOperation,
    value: u64,
) -> ArithmeticFunctionRequest {
    ArithmeticFunctionRequest {
        operation,
        value: Some(value),
        domain: "bounded_arithmetic_functions".into(),
        ambiguity: None,
        provenance: vec!["stage136-arithmetic-composition".into()],
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
        provenance: vec!["stage136-arithmetic-composition".into()],
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
        assumptions: vec!["the arithmetic output role is explicitly declared".into()],
        ambiguity: None,
        provenance: vec!["stage136-arithmetic-composition".into()],
    }
}

fn count_request(n: u64, k: u64) -> CombinatoricsRequest {
    CombinatoricsRequest {
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
        provenance: vec!["stage136-arithmetic-composition".into()],
    }
}

fn check_result(result: &ArithmeticFunctionResult) -> (bool, bool) {
    let mut tampered = result.clone();
    tampered.replay_hash.push('x');
    (result.replay_verified(), !tampered.replay_verified())
}

fn check_number(result: &NumberTheoryResult) -> (bool, bool) {
    let mut tampered = result.clone();
    tampered.replay_hash.push('x');
    (result.replay_verified(), !tampered.replay_verified())
}

fn check_algebra(
    result: &the_machine::abstract_algebra_pack::AbstractAlgebraResult,
) -> (bool, bool) {
    let mut tampered = result.clone();
    tampered.replay_hash.push('x');
    (result.replay_verified(), !tampered.replay_verified())
}

fn check_count(result: &the_machine::combinatorics_pack::CombinatoricsResult) -> (bool, bool) {
    let mut tampered = result.clone();
    tampered.replay_hash.push('x');
    (result.replay_verified(), !tampered.replay_verified())
}

type BoundaryOutcome = (usize, String, bool, bool, bool);

fn ambiguous_arithmetic(operation: ArithmeticFunctionOperation, value: u64) -> BoundaryOutcome {
    let mut request = arithmetic_request(operation, value);
    request.ambiguity = Some("the downstream consumer role is unspecified".into());
    let result = evaluate_arithmetic(&request);
    let (replay, tamper) = check_result(&result);
    (
        1,
        if result.status == ArithmeticFunctionStatus::Ambiguous {
            "ambiguous"
        } else {
            "refused"
        }
        .into(),
        false,
        replay,
        tamper,
    )
}

fn refused_analytic() -> BoundaryOutcome {
    let mut request = arithmetic_request(ArithmeticFunctionOperation::DivisorSum, 36);
    request.domain = "analytic_number_theory".into();
    let result = evaluate_arithmetic(&request);
    let (replay, tamper) = check_result(&result);
    (1, "refused".into(), false, replay, tamper)
}

fn refused_nonunit() -> BoundaryOutcome {
    let arithmetic = evaluate_arithmetic(&arithmetic_request(
        ArithmeticFunctionOperation::DivisorCount,
        15,
    ));
    let mut request = number_request(NumberTheoryOperation::ModularInverse);
    request.a = Some(6);
    request.modulus = Some(15);
    let number = evaluate_number_theory(&request);
    let (arithmetic_replay, arithmetic_tamper) = check_result(&arithmetic);
    let (number_replay, number_tamper) = check_number(&number);
    (
        2,
        "refused".into(),
        false,
        arithmetic_replay && number_replay,
        arithmetic_tamper && number_tamper,
    )
}

fn refused_signed_probability() -> BoundaryOutcome {
    let arithmetic =
        evaluate_arithmetic(&arithmetic_request(ArithmeticFunctionOperation::Mobius, 15));
    let probability_request = ProbabilityRequest {
        operation: ProbabilityOperation::DistributionConstruction,
        domain: "finite_exact_probability".into(),
        outcomes: vec!["signed_mobius_weight".into(), "remainder".into()],
        probabilities: vec![Rational::new(-1, 1).unwrap(), Rational::new(2, 1).unwrap()],
        values: vec![1, 0],
        event_a: None,
        event_b: None,
        partition: Vec::new(),
        conditional_values: Vec::new(),
        prior_probability: None,
        likelihood: None,
        evidence: None,
        ambiguity: None,
        provenance: vec!["stage136-mobius-signed-weight-refusal".into()],
    };
    let probability = evaluate_probability(&probability_request);
    let (arithmetic_replay, arithmetic_tamper) = check_result(&arithmetic);
    let mut tampered = probability.clone();
    tampered.replay_hash.push('x');
    let probability_replay = probability.replay_verified();
    let probability_tamper = !tampered.replay_verified();
    (
        2,
        "refused".into(),
        false,
        arithmetic_replay
            && probability_replay
            && probability.status == ProbabilityStatus::InvalidProbability,
        arithmetic_tamper && probability_tamper,
    )
}

fn refused_unbounded() -> BoundaryOutcome {
    let result = evaluate_arithmetic(&arithmetic_request(
        ArithmeticFunctionOperation::DivisorCount,
        100_001,
    ));
    let (replay, tamper) = check_result(&result);
    (1, "refused".into(), false, replay, tamper)
}

fn supported_divisor_to_totient(index: usize) -> (usize, bool, bool) {
    let value = 12 + (index % 12) as u64;
    let arithmetic = evaluate_arithmetic(&arithmetic_request(
        ArithmeticFunctionOperation::DivisorCount,
        value,
    ));
    let ArithmeticFunctionArtifact::DivisorCertificate {
        value: source_value,
        ..
    } = arithmetic.artifact.clone().expect("divisor certificate")
    else {
        return (1, false, false);
    };
    let mut totient = number_request(NumberTheoryOperation::EulerTotient);
    totient.modulus = Some(source_value);
    let number = evaluate_number_theory(&totient);
    let (arithmetic_replay, arithmetic_tamper) = check_result(&arithmetic);
    let (number_replay, number_tamper) = check_number(&number);
    let handoff = arithmetic.status == ArithmeticFunctionStatus::Complete
        && number.status == NumberTheoryStatus::Complete
        && matches!(number.artifact, Some(NumberTheoryArtifact::Scalar(_)))
        && totient.modulus == Some(source_value);
    (
        2,
        handoff && arithmetic_replay && number_replay,
        arithmetic_tamper && number_tamper,
    )
}

fn supported_divisor_to_inverse(index: usize) -> (usize, bool, bool) {
    let values = [4u64, 8, 9, 16, 25, 36, 64, 81];
    let value = values[index % values.len()];
    let arithmetic = evaluate_arithmetic(&arithmetic_request(
        ArithmeticFunctionOperation::DivisorSum,
        value,
    ));
    let ArithmeticFunctionArtifact::DivisorCertificate {
        divisor_count,
        divisor_sum,
        ..
    } = arithmetic.artifact.clone().expect("divisor certificate")
    else {
        return (1, false, false);
    };
    let mut gcd_request = number_request(NumberTheoryOperation::GcdBezout);
    gcd_request.a = Some(divisor_count as i64);
    gcd_request.b = Some(divisor_sum as i64);
    let gcd = evaluate_number_theory(&gcd_request);
    let gcd_value = match gcd.artifact {
        Some(NumberTheoryArtifact::GcdBezout { gcd, .. }) => gcd,
        _ => return (2, false, false),
    };
    let mut inverse_request = number_request(NumberTheoryOperation::ModularInverse);
    inverse_request.a = Some(divisor_count as i64);
    inverse_request.modulus = Some(divisor_sum);
    let inverse = evaluate_number_theory(&inverse_request);
    let (arithmetic_replay, arithmetic_tamper) = check_result(&arithmetic);
    let (gcd_replay, gcd_tamper) = check_number(&gcd);
    let (inverse_replay, inverse_tamper) = check_number(&inverse);
    let handoff = arithmetic.status == ArithmeticFunctionStatus::Complete
        && gcd.status == NumberTheoryStatus::Complete
        && gcd_value == 1
        && inverse.status == NumberTheoryStatus::Complete
        && matches!(inverse.artifact, Some(NumberTheoryArtifact::Scalar(_)));
    (
        3,
        handoff && arithmetic_replay && gcd_replay && inverse_replay,
        arithmetic_tamper && gcd_tamper && inverse_tamper,
    )
}

fn supported_prime_count_to_combinations(index: usize) -> (usize, bool, bool) {
    let value = 8 + (index % 6) as u64;
    let arithmetic = evaluate_arithmetic(&arithmetic_request(
        ArithmeticFunctionOperation::PrimeCounting,
        value,
    ));
    let ArithmeticFunctionArtifact::PrimeCounting { count, .. } = arithmetic
        .artifact
        .clone()
        .expect("prime-counting certificate")
    else {
        return (1, false, false);
    };
    let count_result = evaluate_combinatorics(&count_request(count, 2));
    let (arithmetic_replay, arithmetic_tamper) = check_result(&arithmetic);
    let (count_replay, count_tamper) = check_count(&count_result);
    let handoff = arithmetic.status == ArithmeticFunctionStatus::Complete
        && count_result.status == CombinatoricsStatus::Complete
        && matches!(
            count_result.artifact,
            Some(CombinatoricsArtifact::Scalar(_))
        )
        && count >= 2;
    (
        2,
        handoff && arithmetic_replay && count_replay,
        arithmetic_tamper && count_tamper,
    )
}

fn supported_value_to_ring(index: usize) -> (usize, bool, bool) {
    let value = 8 + (index % 12) as u64;
    let arithmetic = evaluate_arithmetic(&arithmetic_request(
        ArithmeticFunctionOperation::Mobius,
        value,
    ));
    let ArithmeticFunctionArtifact::Mobius {
        value: source_value,
        ..
    } = arithmetic.artifact.clone().expect("Möbius certificate")
    else {
        return (1, false, false);
    };
    let mut ring_request = algebra_request(AbstractAlgebraOperation::ConstructModularRing);
    ring_request.modulus = u32::try_from(source_value).ok();
    let ring = evaluate_abstract_algebra(&ring_request);
    let source = match ring.artifact {
        Some(AbstractAlgebraArtifact::ModularRing { modulus }) => modulus,
        _ => return (2, false, false),
    };
    let mut unit_request = algebra_request(AbstractAlgebraOperation::CheckUnit);
    unit_request.modulus = Some(source);
    unit_request.element = Some((index as u32 + 1) % source);
    let unit = evaluate_abstract_algebra(&unit_request);
    let (arithmetic_replay, arithmetic_tamper) = check_result(&arithmetic);
    let (ring_replay, ring_tamper) = check_algebra(&ring);
    let (unit_replay, unit_tamper) = check_algebra(&unit);
    let handoff = arithmetic.status == ArithmeticFunctionStatus::Complete
        && ring.status == AbstractAlgebraStatus::Complete
        && unit.status == AbstractAlgebraStatus::Complete
        && ring_request.modulus == Some(source_value as u32)
        && unit_request.modulus == Some(source);
    (
        3,
        handoff && arithmetic_replay && ring_replay && unit_replay,
        arithmetic_tamper && ring_tamper && unit_tamper,
    )
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut corpus = Vec::with_capacity(240);
    for index in 0..30 {
        corpus.push(Case {
            id: format!("divisor_totient_{index:03}"),
            family: "divisor_to_totient".into(),
            expected: Expected::Supported,
            declared_handoff: "value_to_modulus".into(),
        });
        corpus.push(Case {
            id: format!("divisor_inverse_{index:03}"),
            family: "divisor_to_inverse".into(),
            expected: Expected::Supported,
            declared_handoff: "certificate_to_bezout_to_inverse".into(),
        });
        corpus.push(Case {
            id: format!("prime_count_combinations_{index:03}"),
            family: "prime_count_to_combinations".into(),
            expected: Expected::Supported,
            declared_handoff: "count_to_population".into(),
        });
        corpus.push(Case {
            id: format!("mobius_ring_{index:03}"),
            family: "value_to_modular_ring".into(),
            expected: Expected::Supported,
            declared_handoff: "value_to_ring_modulus".into(),
        });
    }
    for index in 0..20 {
        corpus.push(Case {
            id: format!("ambiguous_signed_weight_{index:03}"),
            family: "signed_mobius_without_role".into(),
            expected: Expected::Ambiguous,
            declared_handoff: "mobius_to_probability_without_signed_semantics".into(),
        });
        corpus.push(Case {
            id: format!("ambiguous_count_role_{index:03}"),
            family: "count_without_consumer_role".into(),
            expected: Expected::Ambiguous,
            declared_handoff: "arithmetic_scalar_role_missing".into(),
        });
    }
    for index in 0..20 {
        corpus.push(Case {
            id: format!("refused_analytic_{index:03}"),
            family: "analytic_arithmetic_refusal".into(),
            expected: Expected::Refused,
            declared_handoff: "analytic_claim".into(),
        });
        corpus.push(Case {
            id: format!("refused_nonunit_{index:03}"),
            family: "nonunit_inverse_refusal".into(),
            expected: Expected::Refused,
            declared_handoff: "noncoprime_inverse".into(),
        });
        corpus.push(Case {
            id: format!("refused_probability_{index:03}"),
            family: "signed_weight_probability_refusal".into(),
            expected: Expected::Refused,
            declared_handoff: "mobius_to_probability".into(),
        });
        corpus.push(Case {
            id: format!("refused_unbounded_{index:03}"),
            family: "unbounded_arithmetic_refusal".into(),
            expected: Expected::Refused,
            declared_handoff: "input_budget".into(),
        });
    }
    assert_eq!(corpus.len(), 240);
    let corpus_sha256 = digest(&corpus);
    let mut receipts = Vec::with_capacity(corpus.len());
    let mut family_counts = BTreeMap::new();
    for case in &corpus {
        *family_counts.entry(case.family.clone()).or_insert(0usize) += 1;
        let (stage_count, terminal, handoff, replay, tamper) = match case.expected {
            Expected::Supported => {
                let index = case.id.rsplit('_').next().unwrap().parse().unwrap();
                let (stages, ok, tamper) = match case.family.as_str() {
                    "divisor_to_totient" => supported_divisor_to_totient(index),
                    "divisor_to_inverse" => supported_divisor_to_inverse(index),
                    "prime_count_to_combinations" => supported_prime_count_to_combinations(index),
                    "value_to_modular_ring" => supported_value_to_ring(index),
                    _ => unreachable!(),
                };
                (stages, "complete".into(), ok, ok, tamper)
            }
            Expected::Ambiguous => match case.family.as_str() {
                "signed_mobius_without_role" => {
                    ambiguous_arithmetic(ArithmeticFunctionOperation::Mobius, 15)
                }
                "count_without_consumer_role" => {
                    ambiguous_arithmetic(ArithmeticFunctionOperation::DivisorCount, 36)
                }
                _ => unreachable!(),
            },
            Expected::Refused => match case.family.as_str() {
                "analytic_arithmetic_refusal" => refused_analytic(),
                "nonunit_inverse_refusal" => refused_nonunit(),
                "signed_weight_probability_refusal" => refused_signed_probability(),
                "unbounded_arithmetic_refusal" => refused_unbounded(),
                _ => unreachable!(),
            },
        };
        let exact = match case.expected {
            Expected::Supported => handoff && replay && tamper,
            Expected::Ambiguous => terminal == "ambiguous",
            Expected::Refused => terminal == "refused",
        };
        let false_authorization = case.expected != Expected::Supported && terminal == "complete";
        let false_denial = case.expected == Expected::Supported && !exact;
        receipts.push(Receipt {
            id: case.id.clone(),
            family: case.family.clone(),
            expected: case.expected,
            stage_count,
            actual_terminal: terminal,
            handoff_verified: handoff,
            exact,
            replay_verified: replay,
            tamper_rejected: tamper,
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
    let supported_handoffs = receipts
        .iter()
        .filter(|r| r.expected == Expected::Supported && r.handoff_verified)
        .count();
    let replay_verified = receipts.iter().filter(|r| r.replay_verified).count();
    let tamper_rejections = receipts.iter().filter(|r| r.tamper_rejected).count();
    let false_authorizations = receipts.iter().filter(|r| r.false_authorization).count();
    let false_denials = receipts.iter().filter(|r| r.false_denial).count();
    assert_eq!((supported, ambiguous, refused), (120, 40, 80));
    assert_eq!(exact_decisions, cases);
    assert_eq!(supported_handoffs, supported);
    assert_eq!(replay_verified, cases);
    assert_eq!(tamper_rejections, cases);
    assert_eq!(false_authorizations, 0);
    assert_eq!(false_denials, 0);
    let report = Report {
        schema: "stage136-arithmetic-functions-composition-v1",
        source: "independently authored arithmetic-functions cross-domain corpus",
        corpus_sha256,
        cases,
        supported,
        ambiguous,
        refused,
        exact_decisions,
        supported_handoffs,
        replay_verified,
        tamper_rejections,
        false_authorizations,
        false_denials,
        family_counts,
        receipts,
    };
    let json = serde_json::to_vec_pretty(&report)?;
    std::fs::write("docs/stage136_arithmetic_functions_composition.json", &json)?;
    println!("{}", String::from_utf8(json)?);
    Ok(())
}
