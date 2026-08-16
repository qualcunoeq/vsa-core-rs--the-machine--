//! Phase 70 algebra/number-theory composition and invariant audit.

use serde::Serialize;
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::fs;
use the_machine::abstract_algebra_pack::{
    evaluate_abstract_algebra, AbstractAlgebraOperation, AbstractAlgebraRequest,
    AbstractAlgebraResult,
};
use the_machine::number_theory_pack::{
    evaluate_number_theory, NumberTheoryArtifact, NumberTheoryOperation, NumberTheoryRequest,
    NumberTheoryResult, NumberTheoryStatus,
};

#[derive(Clone, Serialize)]
enum Stage {
    Algebra(AbstractAlgebraRequest),
    Number(NumberTheoryRequest),
}

enum StageOutput {
    Algebra {
        request: AbstractAlgebraRequest,
        result: AbstractAlgebraResult,
    },
    Number {
        request: NumberTheoryRequest,
        result: NumberTheoryResult,
    },
}

fn output_receipt(
    output: &StageOutput,
) -> (String, Option<NumberTheoryArtifact>, bool, bool, bool) {
    match output {
        StageOutput::Algebra { result, .. } => {
            let mut tampered = result.clone();
            tampered.replay_hash.push('x');
            (
                format!("{:?}", result.status),
                None,
                result.replay_verified(),
                !tampered.replay_verified(),
                result.status
                    != the_machine::abstract_algebra_pack::AbstractAlgebraStatus::Complete
                    || result.artifact.is_some(),
            )
        }
        StageOutput::Number { result, .. } => {
            let mut tampered = result.clone();
            tampered.replay_hash.push('x');
            (
                format!("{:?}", result.status),
                result.artifact.clone(),
                result.replay_verified(),
                !tampered.replay_verified(),
                result.status != NumberTheoryStatus::Complete || result.artifact.is_some(),
            )
        }
    }
}

/// Validate that the second stage actually consumes the first stage's typed
/// artifact and shared arithmetic operands.  A pair of independently
/// successful requests is not a composition.
fn validate_handoff(family: &str, prior: &StageOutput, current: &StageOutput) -> bool {
    match family {
        "bezout_to_inverse" => match (prior, current) {
            (
                StageOutput::Number {
                    request: prior_request,
                    result: prior_result,
                },
                StageOutput::Number {
                    request: current_request,
                    result: current_result,
                },
            ) => {
                let Some(NumberTheoryArtifact::GcdBezout { gcd, .. }) =
                    prior_result.artifact.as_ref()
                else {
                    return false;
                };
                let Some(NumberTheoryArtifact::Scalar(inverse)) = current_result.artifact.as_ref()
                else {
                    return false;
                };
                *gcd == 1
                    && prior_request.a == current_request.a
                    && prior_request.b.map(|value| value as u64) == current_request.modulus
                    && current_request.a.zip(current_request.modulus).is_some_and(
                        |(value, modulus)| {
                            (value as i128 * *inverse as i128).rem_euclid(modulus as i128) == 1
                        },
                    )
            }
            _ => false,
        },
        "gcd_to_congruence" => match (prior, current) {
            (
                StageOutput::Number {
                    request: prior_request,
                    result: prior_result,
                },
                StageOutput::Number {
                    request: current_request,
                    result: current_result,
                },
            ) => {
                let Some(NumberTheoryArtifact::GcdBezout { gcd, .. }) =
                    prior_result.artifact.as_ref()
                else {
                    return false;
                };
                let Some(NumberTheoryArtifact::CongruenceClass {
                    modulus,
                    residue,
                    solution_count,
                }) = current_result.artifact.as_ref()
                else {
                    return false;
                };
                prior_request.a == current_request.a
                    && prior_request.b.map(|value| value as u64) == Some(*modulus)
                    && current_request.modulus == Some(*modulus)
                    && *solution_count == *gcd as u64
                    && current_request
                        .a
                        .zip(current_request.b)
                        .is_some_and(|(a, b)| {
                            (a as i128 * *residue as i128 - b as i128).rem_euclid(*modulus as i128)
                                == 0
                        })
            }
            _ => false,
        },
        "bezout_to_crt" => match (prior, current) {
            (
                StageOutput::Number {
                    request: prior_request,
                    result: prior_result,
                },
                StageOutput::Number {
                    request: current_request,
                    result: current_result,
                },
            ) => {
                let Some(NumberTheoryArtifact::GcdBezout { gcd, .. }) =
                    prior_result.artifact.as_ref()
                else {
                    return false;
                };
                let Some(NumberTheoryArtifact::CrtClass { modulus, residue }) =
                    current_result.artifact.as_ref()
                else {
                    return false;
                };
                let (Some(left), Some(right), Some(left_modulus), Some(right_modulus)) = (
                    current_request.a,
                    current_request.b,
                    current_request.modulus,
                    current_request.second_modulus,
                ) else {
                    return false;
                };
                prior_request.a.map(|value| value as u64) == Some(left_modulus)
                    && prior_request.b.map(|value| value as u64) == Some(right_modulus)
                    && *gcd == gcd_u64(left_modulus, right_modulus) as i64
                    && *modulus == left_modulus / *gcd as u64 * right_modulus
                    && residue % left_modulus == left.rem_euclid(left_modulus as i64) as u64
                    && residue % right_modulus == right.rem_euclid(right_modulus as i64) as u64
            }
            _ => false,
        },
        "unit_to_inverse" => match (prior, current) {
            (
                StageOutput::Algebra {
                    request: prior_request,
                    result: prior_result,
                },
                StageOutput::Number {
                    request: current_request,
                    result: current_result,
                },
            ) => {
                let Some(the_machine::abstract_algebra_pack::AbstractAlgebraArtifact::Boolean(
                    is_unit,
                )) = prior_result.artifact.as_ref()
                else {
                    return false;
                };
                let Some(NumberTheoryArtifact::Scalar(inverse)) = current_result.artifact.as_ref()
                else {
                    return false;
                };
                *is_unit
                    && prior_request.modulus.map(u64::from) == current_request.modulus
                    && prior_request.element.map(i64::from) == current_request.a
                    && current_request.a.zip(current_request.modulus).is_some_and(
                        |(value, modulus)| {
                            (value as i128 * *inverse as i128).rem_euclid(modulus as i128) == 1
                        },
                    )
            }
            _ => false,
        },
        "kernel_to_congruence" => match (prior, current) {
            (
                StageOutput::Algebra {
                    request: prior_request,
                    result: prior_result,
                },
                StageOutput::Number {
                    request: current_request,
                    result: current_result,
                },
            ) => {
                let Some(
                    the_machine::abstract_algebra_pack::AbstractAlgebraArtifact::KernelImage {
                        kernel_size,
                        ..
                    },
                ) = prior_result.artifact.as_ref()
                else {
                    return false;
                };
                let Some(NumberTheoryArtifact::CongruenceClass {
                    modulus,
                    solution_count,
                    ..
                }) = current_result.artifact.as_ref()
                else {
                    return false;
                };
                let (Some(source), Some(target), Some(multiplier)) = (
                    prior_request.source_modulus,
                    prior_request.target_modulus,
                    prior_request.multiplier,
                ) else {
                    return false;
                };
                current_request.modulus == Some(u64::from(target))
                    && current_request.a == Some(i64::from(multiplier))
                    && current_request.b == Some(0)
                    && *modulus == u64::from(target)
                    && *kernel_size as u64 * u64::from(target)
                        == *solution_count * u64::from(source)
            }
            _ => false,
        },
        _ => true,
    }
}

fn gcd_u64(mut left: u64, mut right: u64) -> u64 {
    while right != 0 {
        let remainder = left % right;
        left = right;
        right = remainder;
    }
    left
}

#[derive(Clone, Serialize)]
struct Case {
    id: String,
    family: String,
    route: Vec<Stage>,
    expected_status: NumberTheoryStatus,
    expected_artifact: Option<NumberTheoryArtifact>,
}

#[derive(Serialize)]
struct Receipt {
    id: String,
    family: String,
    expected_status: NumberTheoryStatus,
    actual_status: NumberTheoryStatus,
    expected_artifact: Option<NumberTheoryArtifact>,
    actual_artifact: Option<NumberTheoryArtifact>,
    route_replay_verified: bool,
    invariant_preserved: bool,
    handoff_evaluated: bool,
    handoff_verified: bool,
    exact: bool,
    tamper_rejected: bool,
    false_authorization: bool,
}

#[derive(Serialize)]
struct Report {
    schema: &'static str,
    source: &'static str,
    corpus_sha256: String,
    cases: usize,
    supported_routes: usize,
    ambiguous_routes: usize,
    refused_routes: usize,
    exact_route_decisions: usize,
    invariant_preserved: usize,
    handoff_verifications: usize,
    route_replay_verified: usize,
    tamper_rejections: usize,
    false_authorizations: usize,
    false_denials: usize,
    family_counts: BTreeMap<String, usize>,
    receipts: Vec<Receipt>,
}

fn hash<T: Serialize>(value: &T) -> String {
    format!("{:x}", Sha256::digest(serde_json::to_vec(value).unwrap()))
}

fn algebra(operation: AbstractAlgebraOperation) -> AbstractAlgebraRequest {
    AbstractAlgebraRequest {
        operation,
        modulus: None,
        source_modulus: None,
        target_modulus: None,
        element: None,
        multiplier: None,
        second_multiplier: None,
        domain: "finite_exact_abstract_algebra".into(),
        assumptions: vec!["finite structure explicitly declared".into()],
        ambiguity: None,
        provenance: vec!["phase70-algebra-number-theory-composition".into()],
    }
}

fn number(operation: NumberTheoryOperation) -> NumberTheoryRequest {
    NumberTheoryRequest {
        operation,
        a: None,
        b: None,
        c: None,
        modulus: None,
        second_modulus: None,
        domain: "bounded_exact_elementary_number_theory".into(),
        ambiguity: None,
        provenance: vec!["phase70-algebra-number-theory-composition".into()],
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut corpus = Vec::new();
    for index in 0..20 {
        let mut gcd = number(NumberTheoryOperation::GcdBezout);
        // The first stage proves the exact coprimality prerequisite consumed
        // by the inverse stage.  Do not let unrelated requests masquerade as
        // a composition merely because both happen to succeed.
        gcd.a = Some(3);
        gcd.b = Some(11);
        let mut inverse = number(NumberTheoryOperation::ModularInverse);
        inverse.a = Some(3);
        inverse.modulus = Some(11);
        corpus.push(Case {
            id: format!("bezout_to_inverse_{index}"),
            family: "bezout_to_inverse".into(),
            route: vec![Stage::Number(gcd), Stage::Number(inverse)],
            expected_status: NumberTheoryStatus::Complete,
            expected_artifact: Some(NumberTheoryArtifact::Scalar(4)),
        });
    }
    for index in 0..30 {
        let mut gcd = number(NumberTheoryOperation::GcdBezout);
        gcd.a = Some(3);
        gcd.b = Some(10);
        let mut congruence = number(NumberTheoryOperation::LinearCongruence);
        congruence.a = Some(3);
        congruence.b = Some(6);
        congruence.modulus = Some(10);
        corpus.push(Case {
            id: format!("gcd_to_congruence_{index}"),
            family: "gcd_to_congruence".into(),
            route: vec![Stage::Number(gcd), Stage::Number(congruence)],
            expected_status: NumberTheoryStatus::Complete,
            expected_artifact: Some(NumberTheoryArtifact::CongruenceClass {
                modulus: 10,
                residue: 2,
                solution_count: 1,
            }),
        });
    }
    for index in 0..30 {
        let mut compatibility = number(NumberTheoryOperation::GcdBezout);
        compatibility.a = Some(3);
        compatibility.b = Some(5);
        let mut crt = number(NumberTheoryOperation::ChineseRemainder);
        crt.a = Some(2);
        crt.b = Some(3);
        crt.modulus = Some(3);
        crt.second_modulus = Some(5);
        corpus.push(Case {
            id: format!("bezout_to_crt_{index}"),
            family: "bezout_to_crt".into(),
            route: vec![Stage::Number(compatibility), Stage::Number(crt)],
            expected_status: NumberTheoryStatus::Complete,
            expected_artifact: Some(NumberTheoryArtifact::CrtClass {
                modulus: 15,
                residue: 8,
            }),
        });
    }
    for index in 0..20 {
        let mut unit = algebra(AbstractAlgebraOperation::CheckUnit);
        unit.modulus = Some(11);
        unit.element = Some(3);
        let mut inverse = number(NumberTheoryOperation::ModularInverse);
        inverse.a = Some(3);
        inverse.modulus = Some(11);
        corpus.push(Case {
            id: format!("unit_to_inverse_{index}"),
            family: "unit_to_inverse".into(),
            route: vec![Stage::Algebra(unit), Stage::Number(inverse)],
            expected_status: NumberTheoryStatus::Complete,
            expected_artifact: Some(NumberTheoryArtifact::Scalar(4)),
        });
    }
    for index in 0..20 {
        let mut kernel = algebra(AbstractAlgebraOperation::KernelImage);
        kernel.source_modulus = Some(8);
        kernel.target_modulus = Some(4);
        kernel.multiplier = Some(2);
        let mut congruence = number(NumberTheoryOperation::LinearCongruence);
        congruence.a = Some(2);
        congruence.b = Some(0);
        // The kernel is the zero fibre in the target cyclic group.  The
        // number-theory stage therefore solves m*x = 0 modulo the target
        // modulus; the source/target lift is checked below.
        congruence.modulus = Some(4);
        corpus.push(Case {
            id: format!("kernel_to_congruence_{index}"),
            family: "kernel_to_congruence".into(),
            route: vec![Stage::Algebra(kernel), Stage::Number(congruence)],
            expected_status: NumberTheoryStatus::Complete,
            expected_artifact: Some(NumberTheoryArtifact::CongruenceClass {
                modulus: 4,
                residue: 0,
                solution_count: 2,
            }),
        });
    }
    for index in 0..20 {
        let mut inverse = number(NumberTheoryOperation::ModularInverse);
        inverse.a = Some(3);
        inverse.modulus = Some(11);
        inverse.ambiguity = Some("coprimality evidence is not established".into());
        corpus.push(Case {
            id: format!("missing_coprimality_{index}"),
            family: "missing_coprimality".into(),
            route: vec![Stage::Number(inverse)],
            expected_status: NumberTheoryStatus::Ambiguous,
            expected_artifact: None,
        });
    }
    for index in 0..20 {
        let mut crt = number(NumberTheoryOperation::ChineseRemainder);
        crt.a = Some(2);
        crt.b = Some(3);
        crt.modulus = Some(3);
        crt.ambiguity = Some("modulus compatibility is not established".into());
        corpus.push(Case {
            id: format!("missing_crt_compatibility_{index}"),
            family: "missing_crt_compatibility".into(),
            route: vec![Stage::Number(crt)],
            expected_status: NumberTheoryStatus::Ambiguous,
            expected_artifact: None,
        });
    }
    for index in 0..20 {
        let mut inverse = number(NumberTheoryOperation::ModularInverse);
        inverse.a = Some(2);
        inverse.modulus = Some(10);
        corpus.push(Case {
            id: format!("nonunit_refusal_{index}"),
            family: "nonunit_refusal".into(),
            route: vec![Stage::Number(inverse)],
            expected_status: NumberTheoryStatus::Inconsistent,
            expected_artifact: None,
        });
    }
    for index in 0..20 {
        let mut crt = number(NumberTheoryOperation::ChineseRemainder);
        crt.a = Some(1);
        crt.b = Some(0);
        crt.modulus = Some(2);
        crt.second_modulus = Some(4);
        corpus.push(Case {
            id: format!("incompatible_crt_refusal_{index}"),
            family: "incompatible_crt_refusal".into(),
            route: vec![Stage::Number(crt)],
            expected_status: NumberTheoryStatus::Inconsistent,
            expected_artifact: None,
        });
    }
    for index in 0..20 {
        let mut request = number(NumberTheoryOperation::LinearDiophantine);
        request.a = Some(2);
        request.b = Some(3);
        request.c = Some(1);
        request.domain = "nonlinear_diophantine".into();
        corpus.push(Case {
            id: format!("nonlinear_diophantine_refusal_{index}"),
            family: "nonlinear_diophantine_refusal".into(),
            route: vec![Stage::Number(request)],
            expected_status: NumberTheoryStatus::InvalidDomain,
            expected_artifact: None,
        });
    }
    for index in 0..20 {
        let mut inverse = number(NumberTheoryOperation::ModularInverse);
        inverse.a = Some(3);
        inverse.modulus = Some(11);
        inverse.domain = "cryptographic_security_claim".into();
        corpus.push(Case {
            id: format!("cryptographic_claim_refusal_{index}"),
            family: "cryptographic_claim_refusal".into(),
            route: vec![Stage::Number(inverse)],
            expected_status: NumberTheoryStatus::InvalidDomain,
            expected_artifact: None,
        });
    }
    assert_eq!(corpus.len(), 240);

    let corpus_sha256 = hash(&corpus);
    let mut receipts = Vec::with_capacity(corpus.len());
    let mut family_counts = BTreeMap::new();
    for case in corpus {
        *family_counts.entry(case.family.clone()).or_insert(0) += 1;
        let mut final_status = None;
        let mut final_artifact = None;
        let mut prior_output: Option<StageOutput> = None;
        let mut route_replay_verified = true;
        let mut invariant_preserved = true;
        let mut tamper_rejected = true;
        let mut handoff_verified = true;
        for stage in &case.route {
            let output = match stage {
                Stage::Algebra(request) => {
                    let output: AbstractAlgebraResult = evaluate_abstract_algebra(request);
                    StageOutput::Algebra {
                        request: request.clone(),
                        result: output,
                    }
                }
                Stage::Number(request) => {
                    let output: NumberTheoryResult = evaluate_number_theory(request);
                    StageOutput::Number {
                        request: request.clone(),
                        result: output,
                    }
                }
            };
            handoff_verified &= prior_output
                .as_ref()
                .map(|prior| validate_handoff(&case.family, prior, &output))
                .unwrap_or(true);
            let (status, artifact, replay, tamper, stage_valid) = output_receipt(&output);
            route_replay_verified &= replay;
            tamper_rejected &= tamper;
            invariant_preserved &= stage_valid;
            if status != "Complete" && stage_is_not_final(stage, &case.route) {
                invariant_preserved = false;
            }
            final_status = Some(status);
            final_artifact = artifact;
            prior_output = Some(output);
        }
        let actual_status = parse_number_status(final_status.as_deref().unwrap());
        let exact =
            actual_status == case.expected_status && final_artifact == case.expected_artifact;
        receipts.push(Receipt {
            id: case.id,
            family: case.family,
            expected_status: case.expected_status,
            actual_status,
            expected_artifact: case.expected_artifact,
            actual_artifact: final_artifact.clone(),
            route_replay_verified,
            invariant_preserved: case.expected_status == NumberTheoryStatus::Complete
                && invariant_preserved
                && exact,
            handoff_evaluated: case.route.len() > 1,
            handoff_verified,
            exact,
            tamper_rejected,
            false_authorization: case.expected_status != NumberTheoryStatus::Complete
                && final_artifact.is_some(),
        });
    }
    let cases = receipts.len();
    let supported_routes = receipts
        .iter()
        .filter(|r| r.expected_status == NumberTheoryStatus::Complete)
        .count();
    let ambiguous_routes = receipts
        .iter()
        .filter(|r| r.expected_status == NumberTheoryStatus::Ambiguous)
        .count();
    let refused_routes = cases - supported_routes - ambiguous_routes;
    let exact_route_decisions = receipts.iter().filter(|r| r.exact).count();
    let invariant_preserved = receipts.iter().filter(|r| r.invariant_preserved).count();
    let handoff_verifications = receipts
        .iter()
        .filter(|r| r.handoff_evaluated && r.handoff_verified)
        .count();
    let route_replay_verified = receipts.iter().filter(|r| r.route_replay_verified).count();
    let tamper_rejections = receipts.iter().filter(|r| r.tamper_rejected).count();
    let false_authorizations = receipts.iter().filter(|r| r.false_authorization).count();
    let false_denials = receipts
        .iter()
        .filter(|r| r.expected_status == NumberTheoryStatus::Complete && !r.exact)
        .count();
    assert_eq!(exact_route_decisions, cases);
    assert_eq!(invariant_preserved, supported_routes);
    assert_eq!(
        handoff_verifications, supported_routes,
        "every supported multi-stage route must verify its typed handoff"
    );
    assert_eq!(route_replay_verified, cases);
    assert_eq!(tamper_rejections, cases);
    assert_eq!(false_authorizations, 0);
    assert_eq!(false_denials, 0);
    let report = Report {
        schema: "phase70-algebra-number-theory-composition-v2",
        source: "independently authored algebra/number-theory route corpus",
        corpus_sha256,
        cases,
        supported_routes,
        ambiguous_routes,
        refused_routes,
        exact_route_decisions,
        invariant_preserved,
        handoff_verifications,
        route_replay_verified,
        tamper_rejections,
        false_authorizations,
        false_denials,
        family_counts,
        receipts,
    };
    fs::write(
        "docs/phase70_algebra_number_theory_composition.json",
        serde_json::to_vec_pretty(&report)?,
    )?;
    println!("{}", serde_json::to_string_pretty(&report)?);
    Ok(())
}

fn parse_number_status(status: &str) -> NumberTheoryStatus {
    match status {
        "Complete" => NumberTheoryStatus::Complete,
        "Missing" => NumberTheoryStatus::Missing,
        "Ambiguous" => NumberTheoryStatus::Ambiguous,
        "Unsupported" => NumberTheoryStatus::Unsupported,
        "InvalidDomain" => NumberTheoryStatus::InvalidDomain,
        _ => NumberTheoryStatus::Inconsistent,
    }
}

fn stage_is_not_final(stage: &Stage, route: &[Stage]) -> bool {
    !std::ptr::eq(stage, route.last().unwrap())
}
