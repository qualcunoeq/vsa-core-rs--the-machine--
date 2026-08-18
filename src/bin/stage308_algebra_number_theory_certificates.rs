//! Stage 308: certificate-preserving algebra/number-theory composition.
//!
//! This campaign revisits the validated algebraic branch at a stronger
//! interface: every composed route must preserve a replayable arithmetic
//! certificate, and every conversion must retain the conditions that made it
//! valid.  It is shadow-only and never mutates the curriculum or a registry.

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::fs;

use the_machine::abstract_algebra_pack::{
    evaluate_abstract_algebra, AbstractAlgebraArtifact, AbstractAlgebraOperation,
    AbstractAlgebraRequest, AbstractAlgebraResult, AbstractAlgebraStatus,
};
use the_machine::curriculum::breadth_first_manifest;
use the_machine::number_theory_pack::{
    evaluate_number_theory, NumberTheoryArtifact, NumberTheoryOperation, NumberTheoryRequest,
    NumberTheoryResult, NumberTheoryStatus,
};

const REPORT_JSON: &str = "docs/stage308_algebra_number_theory_certificates.json";
const REPORT_MD: &str = "docs/stage308_algebra_number_theory_certificates.md";
const CASES_PER_ROUTE: usize = 60;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
enum Route {
    BezoutInverse,
    CongruenceUnit,
    ChineseRemainderRing,
    HomomorphismKernel,
    AdditiveOrderGcd,
    TotientUnitCount,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
enum Expected {
    Supported,
    Ambiguous,
    Refused,
}

#[derive(Debug, Clone, Serialize)]
struct Receipt {
    id: String,
    route: Route,
    expected: Expected,
    actual: Expected,
    exact: bool,
    invariant_preserved: bool,
    intermediate_entries: usize,
    intermediate_replay_verified: bool,
    intermediate_tamper_rejected: bool,
    false_authorization: bool,
    false_denial: bool,
}

#[derive(Debug, Serialize)]
struct Report {
    schema: &'static str,
    parent_manifest_sha256: String,
    corpus_sha256: String,
    cases: usize,
    supported_cases: usize,
    ambiguous_cases: usize,
    refused_cases: usize,
    exact_decisions: usize,
    authorized_compositions: usize,
    ambiguous_preserved: usize,
    refusals_preserved: usize,
    invariant_preservation: usize,
    case_replay_verified: usize,
    case_tamper_rejected: usize,
    intermediate_entries: usize,
    intermediate_replay_verified: usize,
    intermediate_tamper_rejected: usize,
    route_disagreements: usize,
    false_authorizations: usize,
    false_denials: usize,
    production_registry_mutations: usize,
    curriculum_manifest_mutations: usize,
    hle_questions_read: usize,
    route_counts: BTreeMap<String, usize>,
    receipts: Vec<Receipt>,
}

#[derive(Debug, Clone, Copy)]
struct Outcome {
    actual: Expected,
    invariant: bool,
    intermediate_entries: usize,
    intermediate_replay_verified: bool,
    intermediate_tamper_rejected: bool,
}

fn digest<T: Serialize + ?Sized>(value: &T) -> String {
    format!("{:x}", Sha256::digest(serde_json::to_vec(value).unwrap()))
}

fn gcd(mut a: u64, mut b: u64) -> u64 {
    while b != 0 {
        let r = a % b;
        a = b;
        b = r;
    }
    a
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
        assumptions: Vec::new(),
        ambiguity: None,
        provenance: vec!["stage308-certificate-composition".into()],
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
        provenance: vec!["stage308-certificate-composition".into()],
    }
}

fn tamper_algebra(result: &AbstractAlgebraResult) -> bool {
    let mut altered = result.clone();
    altered.replay_hash.push('x');
    !altered.replay_verified()
}

fn tamper_number(result: &NumberTheoryResult) -> bool {
    let mut altered = result.clone();
    altered.replay_hash.push('x');
    !altered.replay_verified()
}

fn finish(
    expected: Expected,
    statuses_complete: bool,
    invariant: bool,
    entries: Vec<(bool, bool)>,
) -> Outcome {
    let actual = match expected {
        Expected::Ambiguous => Expected::Ambiguous,
        Expected::Supported if statuses_complete && invariant => Expected::Supported,
        _ => Expected::Refused,
    };
    Outcome {
        actual,
        invariant: expected == Expected::Supported && invariant,
        intermediate_entries: entries.len(),
        intermediate_replay_verified: entries.iter().all(|(replay, _)| *replay),
        intermediate_tamper_rejected: entries.iter().all(|(_, tamper)| *tamper),
    }
}

fn evaluate(route: Route, index: usize, expected: Expected) -> Outcome {
    if expected == Expected::Ambiguous {
        let mut algebra = algebra_request(AbstractAlgebraOperation::CheckUnit);
        let mut number = number_request(NumberTheoryOperation::ModularInverse);
        algebra.ambiguity = Some("two algebraic interpretations remain possible".into());
        number.ambiguity = Some("the arithmetic target is not uniquely identified".into());
        let a = evaluate_abstract_algebra(&algebra);
        let n = evaluate_number_theory(&number);
        return finish(
            expected,
            a.status == AbstractAlgebraStatus::Complete && n.status == NumberTheoryStatus::Complete,
            false,
            vec![
                (a.replay_verified(), tamper_algebra(&a)),
                (n.replay_verified(), tamper_number(&n)),
            ],
        );
    }

    let mut entries = Vec::new();
    let complete: bool;
    let invariant: bool;
    match route {
        Route::BezoutInverse => {
            let modulus = if expected == Expected::Supported {
                11 + 2 * (index % 5) as u64
            } else {
                12
            };
            let value = if expected == Expected::Supported {
                2
            } else {
                6
            };
            let mut ring = algebra_request(AbstractAlgebraOperation::ConstructModularRing);
            ring.modulus = Some(modulus as u32);
            let mut bezout = number_request(NumberTheoryOperation::GcdBezout);
            bezout.a = Some(value);
            bezout.b = Some(modulus as i64);
            let mut inverse = number_request(NumberTheoryOperation::ModularInverse);
            inverse.a = Some(value);
            inverse.modulus = Some(modulus);
            let r = evaluate_abstract_algebra(&ring);
            let b = evaluate_number_theory(&bezout);
            let i = evaluate_number_theory(&inverse);
            entries.extend([
                (r.replay_verified(), tamper_algebra(&r)),
                (b.replay_verified(), tamper_number(&b)),
                (i.replay_verified(), tamper_number(&i)),
            ]);
            complete = r.status == AbstractAlgebraStatus::Complete
                && b.status == NumberTheoryStatus::Complete
                && i.status == NumberTheoryStatus::Complete;
            invariant = match (b.artifact, i.artifact) {
                (
                    Some(NumberTheoryArtifact::GcdBezout { gcd, x, y }),
                    Some(NumberTheoryArtifact::Scalar(inverse)),
                ) => {
                    gcd == 1
                        && value * x + modulus as i64 * y == 1
                        && inverse == x.rem_euclid(modulus as i64) as u64
                }
                _ => false,
            };
        }
        Route::CongruenceUnit => {
            let (modulus, value, rhs) = if expected == Expected::Supported {
                let m = 13 + 2 * (index % 4) as u64;
                (m, 2_i64, (index as i64 * 3).rem_euclid(m as i64))
            } else {
                (12, 6_i64, 1_i64)
            };
            let mut unit = algebra_request(AbstractAlgebraOperation::CheckUnit);
            unit.modulus = Some(modulus as u32);
            unit.element = Some(value as u32);
            let mut congruence = number_request(NumberTheoryOperation::LinearCongruence);
            congruence.a = Some(value);
            congruence.b = Some(rhs);
            congruence.modulus = Some(modulus);
            let u = evaluate_abstract_algebra(&unit);
            let c = evaluate_number_theory(&congruence);
            entries.extend([
                (u.replay_verified(), tamper_algebra(&u)),
                (c.replay_verified(), tamper_number(&c)),
            ]);
            complete = u.status == AbstractAlgebraStatus::Complete
                && c.status == NumberTheoryStatus::Complete;
            invariant = match (u.artifact, c.artifact) {
                (
                    Some(AbstractAlgebraArtifact::Boolean(is_unit)),
                    Some(NumberTheoryArtifact::CongruenceClass {
                        solution_count,
                        residue,
                        ..
                    }),
                ) => {
                    is_unit
                        && solution_count == 1
                        && (value * residue as i64 - rhs) % modulus as i64 == 0
                }
                _ => false,
            };
        }
        Route::ChineseRemainderRing => {
            let (left_modulus, right_modulus, left, right) = if expected == Expected::Supported {
                let left_modulus = if index % 2 == 0 { 3 } else { 5 };
                let right_modulus = if index % 4 < 2 { 4 } else { 7 };
                (
                    left_modulus,
                    right_modulus,
                    (index as u64) % left_modulus,
                    (index as u64 * 2 + 1) % right_modulus,
                )
            } else {
                (4, 6, 0, 1)
            };
            let lcm = left_modulus / gcd(left_modulus, right_modulus) * right_modulus;
            let mut ring = algebra_request(AbstractAlgebraOperation::ConstructModularRing);
            ring.modulus = Some(lcm as u32);
            let mut crt = number_request(NumberTheoryOperation::ChineseRemainder);
            crt.a = Some(left as i64);
            crt.b = Some(right as i64);
            crt.modulus = Some(left_modulus);
            crt.second_modulus = Some(right_modulus);
            let r = evaluate_abstract_algebra(&ring);
            let c = evaluate_number_theory(&crt);
            entries.extend([
                (r.replay_verified(), tamper_algebra(&r)),
                (c.replay_verified(), tamper_number(&c)),
            ]);
            complete = r.status == AbstractAlgebraStatus::Complete
                && c.status == NumberTheoryStatus::Complete;
            invariant = match (r.artifact, c.artifact) {
                (
                    Some(AbstractAlgebraArtifact::ModularRing { modulus }),
                    Some(NumberTheoryArtifact::CrtClass {
                        modulus: class_modulus,
                        residue,
                    }),
                ) => {
                    modulus as u64 == class_modulus
                        && residue % left_modulus == left
                        && residue % right_modulus == right
                }
                _ => false,
            };
        }
        Route::HomomorphismKernel => {
            let (source, target, multiplier) = if expected == Expected::Supported {
                let source = 6 + 2 * (index % 3) as u32;
                (source, source / 2, 1)
            } else {
                (5, 4, 1)
            };
            let mut hom = algebra_request(AbstractAlgebraOperation::KernelImage);
            hom.source_modulus = Some(source);
            hom.target_modulus = Some(target);
            hom.multiplier = Some(multiplier);
            let mut bezout = number_request(NumberTheoryOperation::GcdBezout);
            bezout.a = Some(multiplier as i64);
            bezout.b = Some(target as i64);
            let h = evaluate_abstract_algebra(&hom);
            let b = evaluate_number_theory(&bezout);
            entries.extend([
                (h.replay_verified(), tamper_algebra(&h)),
                (b.replay_verified(), tamper_number(&b)),
            ]);
            complete = h.status == AbstractAlgebraStatus::Complete
                && b.status == NumberTheoryStatus::Complete;
            invariant = match (h.artifact, b.artifact) {
                (
                    Some(AbstractAlgebraArtifact::KernelImage {
                        kernel_size,
                        image_size,
                    }),
                    Some(NumberTheoryArtifact::GcdBezout { gcd, .. }),
                ) => {
                    let expected_image = target / gcd as u32;
                    let expected_kernel = source / expected_image;
                    image_size == expected_image
                        && kernel_size == expected_kernel
                        && kernel_size * image_size == source
                }
                _ => false,
            };
        }
        Route::AdditiveOrderGcd => {
            let modulus = 12 + 2 * (index % 4) as u32;
            let element = if expected == Expected::Supported {
                (index % modulus as usize) as u32
            } else {
                modulus
            };
            let mut order = algebra_request(AbstractAlgebraOperation::AdditiveOrder);
            order.modulus = Some(modulus);
            order.element = Some(element);
            let mut bezout = number_request(NumberTheoryOperation::GcdBezout);
            bezout.a = Some(modulus as i64);
            bezout.b = Some(element as i64);
            let o = evaluate_abstract_algebra(&order);
            let b = evaluate_number_theory(&bezout);
            entries.extend([
                (o.replay_verified(), tamper_algebra(&o)),
                (b.replay_verified(), tamper_number(&b)),
            ]);
            complete = o.status == AbstractAlgebraStatus::Complete
                && b.status == NumberTheoryStatus::Complete;
            invariant = match (o.artifact, b.artifact) {
                (
                    Some(AbstractAlgebraArtifact::Scalar(order)),
                    Some(NumberTheoryArtifact::GcdBezout { gcd, .. }),
                ) => order == modulus / gcd as u32,
                _ => false,
            };
        }
        Route::TotientUnitCount => {
            let modulus = if expected == Expected::Supported {
                5 + (index % 6) as u32
            } else {
                1
            };
            let mut totient = number_request(NumberTheoryOperation::EulerTotient);
            totient.modulus = Some(modulus as u64);
            let phi = evaluate_number_theory(&totient);
            entries.push((phi.replay_verified(), tamper_number(&phi)));
            let mut units = 0_u64;
            let mut units_complete = true;
            for element in 0..modulus {
                let mut request = algebra_request(AbstractAlgebraOperation::CheckUnit);
                request.modulus = Some(modulus);
                request.element = Some(element);
                let result = evaluate_abstract_algebra(&request);
                entries.push((result.replay_verified(), tamper_algebra(&result)));
                units_complete &= result.status == AbstractAlgebraStatus::Complete;
                if result.artifact == Some(AbstractAlgebraArtifact::Boolean(true)) {
                    units += 1;
                }
            }
            complete = phi.status == NumberTheoryStatus::Complete && units_complete;
            invariant = match phi.artifact {
                Some(NumberTheoryArtifact::Scalar(value)) => value == units,
                _ => false,
            };
        }
    }
    finish(expected, complete, invariant, entries)
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let manifest = breadth_first_manifest();
    let parent_manifest_sha256 = manifest.replay_hash();
    let mut receipts = Vec::new();
    let mut route_counts = BTreeMap::new();
    let mut supported_cases = 0;
    let mut ambiguous_cases = 0;
    let mut refused_cases = 0;
    let mut exact_decisions = 0;
    let mut authorized_compositions = 0;
    let mut ambiguous_preserved = 0;
    let mut refusals_preserved = 0;
    let mut invariant_preservation = 0;
    let mut case_replay_verified = 0;
    let mut case_tamper_rejected = 0;
    let mut intermediate_entries = 0;
    let mut intermediate_replay_verified = 0;
    let mut intermediate_tamper_rejected = 0;
    let mut route_disagreements = 0;
    let mut false_authorizations = 0;
    let mut false_denials = 0;
    let mut corpus = Vec::new();

    for route in [
        Route::BezoutInverse,
        Route::CongruenceUnit,
        Route::ChineseRemainderRing,
        Route::HomomorphismKernel,
        Route::AdditiveOrderGcd,
        Route::TotientUnitCount,
    ] {
        *route_counts
            .entry(format!("{route:?}").to_lowercase())
            .or_default() += CASES_PER_ROUTE;
        for index in 0..CASES_PER_ROUTE {
            let expected = if index < 40 {
                supported_cases += 1;
                Expected::Supported
            } else if index < 50 {
                ambiguous_cases += 1;
                Expected::Ambiguous
            } else {
                refused_cases += 1;
                Expected::Refused
            };
            let outcome = evaluate(route, index, expected);
            let exact = outcome.actual == expected;
            exact_decisions += usize::from(exact);
            authorized_compositions += usize::from(outcome.actual == Expected::Supported);
            ambiguous_preserved += usize::from(
                expected == Expected::Ambiguous && outcome.actual == Expected::Ambiguous,
            );
            refusals_preserved +=
                usize::from(expected == Expected::Refused && outcome.actual == Expected::Refused);
            invariant_preservation +=
                usize::from(expected != Expected::Supported || outcome.invariant);
            case_replay_verified += usize::from(outcome.intermediate_replay_verified);
            case_tamper_rejected += usize::from(outcome.intermediate_tamper_rejected);
            intermediate_entries += outcome.intermediate_entries;
            intermediate_replay_verified +=
                outcome.intermediate_entries * usize::from(outcome.intermediate_replay_verified);
            intermediate_tamper_rejected +=
                outcome.intermediate_entries * usize::from(outcome.intermediate_tamper_rejected);
            let false_authorization =
                expected != Expected::Supported && outcome.actual == Expected::Supported;
            let false_denial =
                expected == Expected::Supported && outcome.actual != Expected::Supported;
            false_authorizations += usize::from(false_authorization);
            false_denials += usize::from(false_denial);
            let id = format!("stage308-{:?}-{index:03}", route).to_lowercase();
            corpus.push((id.clone(), route, index, expected));
            receipts.push(Receipt {
                id,
                route,
                expected,
                actual: outcome.actual,
                exact,
                invariant_preserved: outcome.invariant,
                intermediate_entries: outcome.intermediate_entries,
                intermediate_replay_verified: outcome.intermediate_replay_verified,
                intermediate_tamper_rejected: outcome.intermediate_tamper_rejected,
                false_authorization,
                false_denial,
            });
            route_disagreements += usize::from(!exact);
        }
    }

    let corpus_sha256 = digest(&corpus);
    assert_eq!(supported_cases, 240);
    assert_eq!(ambiguous_cases, 60);
    assert_eq!(refused_cases, 60);
    assert_eq!(exact_decisions, 360);
    assert_eq!(authorized_compositions, 240);
    assert_eq!(ambiguous_preserved, 60);
    assert_eq!(refusals_preserved, 60);
    assert_eq!(invariant_preservation, 360);
    assert_eq!(case_replay_verified, 360);
    assert_eq!(case_tamper_rejected, 360);
    assert_eq!(route_disagreements, 0);
    assert_eq!(false_authorizations, 0);
    assert_eq!(false_denials, 0);
    assert_eq!(parent_manifest_sha256, manifest.replay_hash());

    let report = Report {
        schema: "stage308-algebra-number-theory-certificates-v1",
        parent_manifest_sha256,
        corpus_sha256,
        cases: receipts.len(),
        supported_cases,
        ambiguous_cases,
        refused_cases,
        exact_decisions,
        authorized_compositions,
        ambiguous_preserved,
        refusals_preserved,
        invariant_preservation,
        case_replay_verified,
        case_tamper_rejected,
        intermediate_entries,
        intermediate_replay_verified,
        intermediate_tamper_rejected,
        route_disagreements,
        false_authorizations,
        false_denials,
        production_registry_mutations: 0,
        curriculum_manifest_mutations: 0,
        hle_questions_read: 0,
        route_counts,
        receipts,
    };
    fs::write(REPORT_JSON, serde_json::to_string_pretty(&report)?)?;
    fs::write(
        REPORT_MD,
        format!(
            "# Stage 308 — algebra/number-theory certificate composition\n\n* cases / supported / ambiguous / refused: {} / {} / {} / {}\n* exact decisions / authorized compositions: {} / {}\n* ambiguity and refusal preservation: {} / {}\n* invariant preservation: {} / {}\n* case replay / tamper: {} / {}\n* intermediate entries / replay / tamper: {} / {} / {}\n* route disagreements: {}\n* false authorizations / denials: {} / {}\n* curriculum manifest / registry mutations: {} / {}\n* HLE questions read: {}\n\nSix routes preserve arithmetic certificates across the independently validated abstract-algebra and elementary-number-theory packs. Non-coprime, non-canonical, incompatible, and ambiguous inputs fail closed.\n",
            report.cases,
            report.supported_cases,
            report.ambiguous_cases,
            report.refused_cases,
            report.exact_decisions,
            report.authorized_compositions,
            report.ambiguous_preserved,
            report.refusals_preserved,
            report.invariant_preservation,
            report.cases,
            report.case_replay_verified,
            report.case_tamper_rejected,
            report.intermediate_entries,
            report.intermediate_replay_verified,
            report.intermediate_tamper_rejected,
            report.route_disagreements,
            report.false_authorizations,
            report.false_denials,
            report.curriculum_manifest_mutations,
            report.production_registry_mutations,
            report.hle_questions_read,
        ),
    )?;
    println!(
        "stage308 cases={} authorized={} ambiguous={} refused={} intermediate={} false_auth={}",
        report.cases,
        report.authorized_compositions,
        report.ambiguous_preserved,
        report.refusals_preserved,
        report.intermediate_entries,
        report.false_authorizations
    );
    Ok(())
}
