//! Bounded exact elementary number theory for the governed curriculum.
//!
//! The pack uses explicit finite integer bounds and canonical residues. It
//! refuses unbounded factorization, asymptotics, specialist theorems, and
//! cryptographic conclusions.

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

const MAX_INPUT: u64 = 100_000;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum NumberTheoryOperation {
    GcdBezout,
    ModularInverse,
    LinearCongruence,
    ChineseRemainder,
    EulerTotient,
    LinearDiophantine,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum NumberTheoryStatus {
    Complete,
    Missing,
    Ambiguous,
    Unsupported,
    InvalidDomain,
    Inconsistent,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum NumberTheoryArtifact {
    GcdBezout {
        gcd: i64,
        x: i64,
        y: i64,
    },
    CongruenceClass {
        modulus: u64,
        residue: u64,
        solution_count: u64,
    },
    CrtClass {
        modulus: u64,
        residue: u64,
    },
    Diophantine {
        gcd: i64,
        x: i64,
        y: i64,
    },
    Scalar(u64),
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct NumberTheoryRequest {
    pub operation: NumberTheoryOperation,
    pub a: Option<i64>,
    pub b: Option<i64>,
    pub c: Option<i64>,
    pub modulus: Option<u64>,
    pub second_modulus: Option<u64>,
    pub domain: String,
    pub ambiguity: Option<String>,
    pub provenance: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct NumberTheoryResult {
    pub status: NumberTheoryStatus,
    pub artifact: Option<NumberTheoryArtifact>,
    pub operation: NumberTheoryOperation,
    pub assumptions: Vec<String>,
    pub reasons: Vec<String>,
    pub provenance: Vec<String>,
    pub replay_hash: String,
}

fn digest<T: Serialize>(value: &T) -> String {
    format!(
        "{:x}",
        Sha256::digest(serde_json::to_vec(value).expect("number theory serializes"))
    )
}

fn payload(result: &NumberTheoryResult) -> impl Serialize + '_ {
    (
        result.status,
        result.artifact.as_ref(),
        result.operation,
        &result.assumptions,
        &result.reasons,
        &result.provenance,
    )
}

fn result(
    request: &NumberTheoryRequest,
    status: NumberTheoryStatus,
    artifact: Option<NumberTheoryArtifact>,
    assumptions: Vec<String>,
    reasons: Vec<String>,
) -> NumberTheoryResult {
    let mut output = NumberTheoryResult {
        status,
        artifact,
        operation: request.operation,
        assumptions,
        reasons,
        provenance: request.provenance.clone(),
        replay_hash: String::new(),
    };
    let replay_hash = digest(&payload(&output));
    output.replay_hash = replay_hash;
    output
}

fn egcd(a: i64, b: i64) -> (i64, i64, i64) {
    if b == 0 {
        (a.abs(), a.signum(), 0)
    } else {
        let (g, x, y) = egcd(b, a % b);
        (g, y, x - (a / b) * y)
    }
}

fn gcd_u64(mut a: u64, mut b: u64) -> u64 {
    while b != 0 {
        let remainder = a % b;
        a = b;
        b = remainder;
    }
    a
}

fn mod_inverse(value: i64, modulus: u64) -> Option<u64> {
    let modulus_i = i64::try_from(modulus).ok()?;
    let (gcd, x, _) = egcd(value, modulus_i);
    if gcd != 1 {
        return None;
    }
    Some(x.rem_euclid(modulus_i) as u64)
}

fn factor_totient(mut value: u64) -> u64 {
    let mut result = value;
    let mut factor = 2;
    while factor * factor <= value {
        if value % factor == 0 {
            result = result / factor * (factor - 1);
            while value % factor == 0 {
                value /= factor;
            }
        }
        factor += if factor == 2 { 1 } else { 2 };
    }
    if value > 1 {
        result = result / value * (value - 1);
    }
    result
}

/// Evaluate a bounded exact number-theory request without live mutation.
pub fn evaluate_number_theory(request: &NumberTheoryRequest) -> NumberTheoryResult {
    if request.domain != "bounded_exact_elementary_number_theory" {
        return result(
            request,
            NumberTheoryStatus::InvalidDomain,
            None,
            Vec::new(),
            vec!["domain is outside bounded elementary number theory".into()],
        );
    }
    if let Some(ambiguity) = &request.ambiguity {
        return result(
            request,
            NumberTheoryStatus::Ambiguous,
            None,
            Vec::new(),
            vec![ambiguity.clone()],
        );
    }
    let assumptions = vec![
        "exact integer arithmetic".into(),
        "canonical nonnegative residues".into(),
        "inputs bounded by 100000 where applicable".into(),
    ];
    match request.operation {
        NumberTheoryOperation::GcdBezout => {
            let (Some(a), Some(b)) = (request.a, request.b) else {
                return result(
                    request,
                    NumberTheoryStatus::Missing,
                    None,
                    assumptions,
                    vec!["two integer operands are required".into()],
                );
            };
            let (gcd, x, y) = egcd(a, b);
            result(
                request,
                NumberTheoryStatus::Complete,
                Some(NumberTheoryArtifact::GcdBezout { gcd, x, y }),
                assumptions,
                Vec::new(),
            )
        }
        NumberTheoryOperation::ModularInverse => {
            let (Some(value), Some(modulus)) = (request.a, request.modulus) else {
                return result(
                    request,
                    NumberTheoryStatus::Missing,
                    None,
                    assumptions,
                    vec!["value and positive modulus are required".into()],
                );
            };
            if modulus < 2 || modulus > MAX_INPUT {
                return result(
                    request,
                    NumberTheoryStatus::Unsupported,
                    None,
                    assumptions,
                    vec!["modulus is outside the bounded inverse range".into()],
                );
            }
            match mod_inverse(value, modulus) {
                Some(inverse) => result(
                    request,
                    NumberTheoryStatus::Complete,
                    Some(NumberTheoryArtifact::Scalar(inverse)),
                    assumptions,
                    Vec::new(),
                ),
                None => result(
                    request,
                    NumberTheoryStatus::Inconsistent,
                    None,
                    assumptions,
                    vec!["value is not coprime to the modulus".into()],
                ),
            }
        }
        NumberTheoryOperation::LinearCongruence => {
            let (Some(a), Some(b), Some(modulus)) = (request.a, request.b, request.modulus) else {
                return result(
                    request,
                    NumberTheoryStatus::Missing,
                    None,
                    assumptions,
                    vec!["coefficient, right side, and modulus are required".into()],
                );
            };
            if modulus < 2 || modulus > MAX_INPUT {
                return result(
                    request,
                    NumberTheoryStatus::Unsupported,
                    None,
                    assumptions,
                    vec!["modulus is outside the bounded congruence range".into()],
                );
            }
            let modulus_i = modulus as i64;
            let d = gcd_u64(a.unsigned_abs(), modulus);
            if b.rem_euclid(d as i64) != 0 {
                return result(
                    request,
                    NumberTheoryStatus::Inconsistent,
                    None,
                    assumptions,
                    vec!["gcd(coefficient, modulus) does not divide the right side".into()],
                );
            }
            let reduced_a = a / d as i64;
            let reduced_b = b / d as i64;
            let reduced_modulus = modulus / d;
            let inverse =
                mod_inverse(reduced_a, reduced_modulus).expect("reduced coefficient is invertible");
            let residue = (inverse as i64 * reduced_b).rem_euclid(reduced_modulus as i64) as u64;
            let _ = modulus_i;
            result(
                request,
                NumberTheoryStatus::Complete,
                Some(NumberTheoryArtifact::CongruenceClass {
                    modulus,
                    residue,
                    solution_count: d,
                }),
                assumptions,
                Vec::new(),
            )
        }
        NumberTheoryOperation::ChineseRemainder => {
            let (Some(left), Some(right), Some(left_modulus), Some(right_modulus)) = (
                request.a,
                request.b,
                request.modulus,
                request.second_modulus,
            ) else {
                return result(
                    request,
                    NumberTheoryStatus::Missing,
                    None,
                    assumptions,
                    vec!["two residues and two positive moduli are required".into()],
                );
            };
            if left_modulus < 2
                || right_modulus < 2
                || left_modulus > MAX_INPUT
                || right_modulus > MAX_INPUT
            {
                return result(
                    request,
                    NumberTheoryStatus::Unsupported,
                    None,
                    assumptions,
                    vec!["CRT moduli are outside the bounded range".into()],
                );
            }
            let common = gcd_u64(left_modulus, right_modulus);
            if (left - right).rem_euclid(common as i64) != 0 {
                return result(
                    request,
                    NumberTheoryStatus::Inconsistent,
                    None,
                    assumptions,
                    vec!["residues disagree modulo the common divisor".into()],
                );
            }
            let reduced = right_modulus / common;
            let left_factor = left_modulus / common;
            let inverse = mod_inverse(left_factor as i64, reduced)
                .expect("reduced CRT modulus is invertible");
            let step = (right - left)
                .div_euclid(common as i64)
                .rem_euclid(reduced as i64) as u64;
            let k = (step * inverse) % reduced;
            let lcm = left_modulus / common * right_modulus;
            let residue =
                (left as i128 + left_modulus as i128 * k as i128).rem_euclid(lcm as i128) as u64;
            result(
                request,
                NumberTheoryStatus::Complete,
                Some(NumberTheoryArtifact::CrtClass {
                    modulus: lcm,
                    residue,
                }),
                assumptions,
                Vec::new(),
            )
        }
        NumberTheoryOperation::EulerTotient => {
            let Some(value) = request.modulus else {
                return result(
                    request,
                    NumberTheoryStatus::Missing,
                    None,
                    assumptions,
                    vec!["a positive integer is required".into()],
                );
            };
            if !(1..=MAX_INPUT).contains(&value) {
                return result(
                    request,
                    NumberTheoryStatus::Unsupported,
                    None,
                    assumptions,
                    vec!["factorization exceeds the bounded totient budget".into()],
                );
            }
            result(
                request,
                NumberTheoryStatus::Complete,
                Some(NumberTheoryArtifact::Scalar(factor_totient(value))),
                assumptions,
                vec!["bounded trial factorization only".into()],
            )
        }
        NumberTheoryOperation::LinearDiophantine => {
            let (Some(a), Some(b), Some(c)) = (request.a, request.b, request.c) else {
                return result(
                    request,
                    NumberTheoryStatus::Missing,
                    None,
                    assumptions,
                    vec!["three integer coefficients are required".into()],
                );
            };
            let (gcd, x, y) = egcd(a, b);
            if gcd == 0 {
                return result(
                    request,
                    NumberTheoryStatus::Inconsistent,
                    None,
                    assumptions,
                    vec!["both leading coefficients cannot be zero".into()],
                );
            }
            if c % gcd != 0 {
                return result(
                    request,
                    NumberTheoryStatus::Inconsistent,
                    None,
                    assumptions,
                    vec!["gcd(a,b) does not divide c".into()],
                );
            }
            let scale = c / gcd;
            result(
                request,
                NumberTheoryStatus::Complete,
                Some(NumberTheoryArtifact::Diophantine {
                    gcd,
                    x: x * scale,
                    y: y * scale,
                }),
                assumptions,
                Vec::new(),
            )
        }
    }
}

impl NumberTheoryResult {
    pub fn replay_verified(&self) -> bool {
        self.replay_hash == digest(&payload(self))
            && !self.provenance.is_empty()
            && (self.status != NumberTheoryStatus::Complete || self.artifact.is_some())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn request(operation: NumberTheoryOperation) -> NumberTheoryRequest {
        NumberTheoryRequest {
            operation,
            a: Some(3),
            b: Some(6),
            c: Some(3),
            modulus: Some(10),
            second_modulus: Some(3),
            domain: "bounded_exact_elementary_number_theory".into(),
            ambiguity: None,
            provenance: vec!["number-theory-test".into()],
        }
    }

    #[test]
    fn congruence_and_crt_replay() {
        let congruence = evaluate_number_theory(&request(NumberTheoryOperation::LinearCongruence));
        assert!(congruence.replay_verified());
        let mut crt = request(NumberTheoryOperation::ChineseRemainder);
        crt.a = Some(2);
        crt.b = Some(3);
        crt.modulus = Some(3);
        crt.second_modulus = Some(5);
        let result = evaluate_number_theory(&crt);
        assert_eq!(
            result.artifact,
            Some(NumberTheoryArtifact::CrtClass {
                modulus: 15,
                residue: 8
            })
        );
        assert!(result.replay_verified());
    }

    #[test]
    fn non_coprime_inverse_refuses() {
        let mut request = request(NumberTheoryOperation::ModularInverse);
        request.a = Some(2);
        let result = evaluate_number_theory(&request);
        assert_eq!(result.status, NumberTheoryStatus::Inconsistent);
        let mut tampered = result.clone();
        tampered.replay_hash.push('x');
        assert!(!tampered.replay_verified());
    }
}
