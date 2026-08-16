//! Bounded exact polynomial algebra over explicitly declared prime fields.
//!
//! The pack provides canonical coefficient vectors, arithmetic, Euclidean
//! division, gcd, evaluation, and exhaustive roots for small degrees. It does
//! not infer coefficient domains, factor arbitrary integer polynomials, or
//! authorize minimal-polynomial and analytic claims without a separate pack.

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

const MAX_DEGREE: usize = 8;
const MAX_MODULUS: u64 = 10_000;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Polynomial {
    pub coefficients: Vec<u64>,
    pub modulus: u64,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum PolynomialOperation {
    Add,
    Multiply,
    Divide,
    Gcd,
    Evaluate,
    Roots,
    FactorQuadratic,
    MinimalPolynomial,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum PolynomialStatus {
    Complete,
    Missing,
    Ambiguous,
    Unsupported,
    InvalidDomain,
    Inconsistent,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum PolynomialArtifact {
    Polynomial(Polynomial),
    Division {
        quotient: Polynomial,
        remainder: Polynomial,
    },
    Value(u64),
    Roots(Vec<u64>),
    Factors(Vec<Polynomial>),
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PolynomialRequest {
    pub operation: PolynomialOperation,
    pub left: Option<Polynomial>,
    pub right: Option<Polynomial>,
    pub point: Option<u64>,
    pub domain: String,
    pub ambiguity: Option<String>,
    pub provenance: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PolynomialResult {
    pub status: PolynomialStatus,
    pub artifact: Option<PolynomialArtifact>,
    pub operation: PolynomialOperation,
    pub assumptions: Vec<String>,
    pub reasons: Vec<String>,
    pub provenance: Vec<String>,
    pub replay_hash: String,
}

fn digest<T: Serialize>(value: &T) -> String {
    format!("{:x}", Sha256::digest(serde_json::to_vec(value).unwrap()))
}

fn payload(result: &PolynomialResult) -> impl Serialize {
    (
        result.status,
        result.artifact.clone(),
        result.operation,
        result.assumptions.clone(),
        result.reasons.clone(),
        result.provenance.clone(),
    )
}

fn prime(modulus: u64) -> bool {
    if modulus < 2 || modulus > MAX_MODULUS {
        return false;
    }
    let mut divisor = 2;
    while divisor * divisor <= modulus {
        if modulus % divisor == 0 {
            return false;
        }
        divisor += 1;
    }
    true
}

fn normalize(mut coefficients: Vec<u64>, modulus: u64) -> Vec<u64> {
    for coefficient in &mut coefficients {
        *coefficient %= modulus;
    }
    while coefficients.len() > 1 && coefficients.last() == Some(&0) {
        coefficients.pop();
    }
    if coefficients.is_empty() {
        coefficients.push(0);
    }
    coefficients
}

fn valid(poly: &Polynomial) -> bool {
    prime(poly.modulus)
        && !poly.coefficients.is_empty()
        && poly.coefficients.len() <= MAX_DEGREE + 1
        && poly
            .coefficients
            .iter()
            .all(|coefficient| *coefficient < poly.modulus)
        && (poly.coefficients.len() == 1 || poly.coefficients.last() != Some(&0))
}

fn make(coefficients: Vec<u64>, modulus: u64) -> Polynomial {
    Polynomial {
        coefficients: normalize(coefficients, modulus),
        modulus,
    }
}

fn add(left: &Polynomial, right: &Polynomial) -> Polynomial {
    let len = left.coefficients.len().max(right.coefficients.len());
    let coefficients = (0..len)
        .map(|index| {
            (left.coefficients.get(index).copied().unwrap_or(0)
                + right.coefficients.get(index).copied().unwrap_or(0))
                % left.modulus
        })
        .collect();
    make(coefficients, left.modulus)
}

fn multiply(left: &Polynomial, right: &Polynomial) -> Polynomial {
    let mut coefficients = vec![0; left.coefficients.len() + right.coefficients.len() - 1];
    for (left_index, left_value) in left.coefficients.iter().enumerate() {
        for (right_index, right_value) in right.coefficients.iter().enumerate() {
            coefficients[left_index + right_index] =
                (coefficients[left_index + right_index] + left_value * right_value) % left.modulus;
        }
    }
    make(coefficients, left.modulus)
}

fn mod_pow(mut base: u64, mut exponent: u64, modulus: u64) -> u64 {
    let mut result = 1;
    base %= modulus;
    while exponent > 0 {
        if exponent % 2 == 1 {
            result = result * base % modulus;
        }
        base = base * base % modulus;
        exponent /= 2;
    }
    result
}

fn inverse(value: u64, modulus: u64) -> Option<u64> {
    (value != 0).then(|| mod_pow(value, modulus - 2, modulus))
}

fn divide(dividend: &Polynomial, divisor: &Polynomial) -> Option<(Polynomial, Polynomial)> {
    if divisor.coefficients.len() == 1 && divisor.coefficients[0] == 0 {
        return None;
    }
    let modulus = dividend.modulus;
    let divisor_lead = *divisor.coefficients.last()?;
    let divisor_inverse = inverse(divisor_lead, modulus)?;
    let mut remainder = dividend.coefficients.clone();
    let mut quotient = vec![
        0;
        dividend
            .coefficients
            .len()
            .saturating_sub(divisor.coefficients.len())
            + 1
    ];
    while !(remainder.len() == 1 && remainder[0] == 0)
        && remainder.len() >= divisor.coefficients.len()
    {
        let shift = remainder.len() - divisor.coefficients.len();
        let factor = remainder.last().copied().unwrap_or(0) * divisor_inverse % modulus;
        quotient[shift] = factor;
        for (index, coefficient) in divisor.coefficients.iter().enumerate() {
            let target = index + shift;
            let subtraction = factor * coefficient % modulus;
            remainder[target] = (remainder[target] + modulus - subtraction) % modulus;
        }
        remainder = normalize(remainder, modulus);
    }
    Some((make(quotient, modulus), make(remainder, modulus)))
}

fn gcd(mut left: Polynomial, mut right: Polynomial) -> Polynomial {
    while !(right.coefficients.len() == 1 && right.coefficients[0] == 0) {
        let Some((_, remainder)) = divide(&left, &right) else {
            return make(vec![0], left.modulus);
        };
        left = right;
        right = remainder;
    }
    let lead = *left.coefficients.last().unwrap_or(&0);
    if let Some(inverse_lead) = inverse(lead, left.modulus) {
        make(
            left.coefficients
                .iter()
                .map(|coefficient| coefficient * inverse_lead % left.modulus)
                .collect(),
            left.modulus,
        )
    } else {
        left
    }
}

fn evaluate(poly: &Polynomial, point: u64) -> u64 {
    poly.coefficients
        .iter()
        .rev()
        .fold(0, |accumulator, coefficient| {
            (accumulator * (point % poly.modulus) + coefficient) % poly.modulus
        })
}

fn result(
    request: &PolynomialRequest,
    status: PolynomialStatus,
    artifact: Option<PolynomialArtifact>,
    reasons: Vec<String>,
) -> PolynomialResult {
    let mut output = PolynomialResult {
        status,
        artifact,
        operation: request.operation,
        assumptions: vec![
            "explicit prime field modulus".into(),
            format!("degree at most {MAX_DEGREE}"),
            "canonical low-to-high coefficient vectors".into(),
        ],
        reasons,
        provenance: request.provenance.clone(),
        replay_hash: String::new(),
    };
    output.replay_hash = digest(&payload(&output));
    output
}

/// Evaluate a bounded polynomial request without mutation or implicit domain inference.
pub fn evaluate_polynomial(request: &PolynomialRequest) -> PolynomialResult {
    if request.domain != "bounded_exact_prime_field_polynomial" {
        return result(
            request,
            PolynomialStatus::InvalidDomain,
            None,
            vec!["domain is outside bounded prime-field polynomial algebra".into()],
        );
    }
    if let Some(ambiguity) = &request.ambiguity {
        return result(
            request,
            PolynomialStatus::Ambiguous,
            None,
            vec![ambiguity.clone()],
        );
    }
    let Some(left) = request.left.as_ref() else {
        return result(
            request,
            PolynomialStatus::Missing,
            None,
            vec!["left polynomial is required".into()],
        );
    };
    if !valid(left) {
        return result(
            request,
            PolynomialStatus::InvalidDomain,
            None,
            vec!["left polynomial is not canonical, bounded, or over a prime field".into()],
        );
    }
    if matches!(
        request.operation,
        PolynomialOperation::Add
            | PolynomialOperation::Multiply
            | PolynomialOperation::Divide
            | PolynomialOperation::Gcd
    ) && request
        .right
        .as_ref()
        .is_none_or(|right| !valid(right) || right.modulus != left.modulus)
    {
        return result(
            request,
            PolynomialStatus::Inconsistent,
            None,
            vec!["a compatible right polynomial is required".into()],
        );
    }
    let artifact = match request.operation {
        PolynomialOperation::Add => {
            PolynomialArtifact::Polynomial(add(left, request.right.as_ref().unwrap()))
        }
        PolynomialOperation::Multiply => {
            PolynomialArtifact::Polynomial(multiply(left, request.right.as_ref().unwrap()))
        }
        PolynomialOperation::Divide => {
            let Some((quotient, remainder)) = divide(left, request.right.as_ref().unwrap()) else {
                return result(
                    request,
                    PolynomialStatus::Inconsistent,
                    None,
                    vec!["division by zero polynomial".into()],
                );
            };
            PolynomialArtifact::Division {
                quotient,
                remainder,
            }
        }
        PolynomialOperation::Gcd => PolynomialArtifact::Polynomial(gcd(
            left.clone(),
            request.right.as_ref().unwrap().clone(),
        )),
        PolynomialOperation::Evaluate => {
            let Some(point) = request.point else {
                return result(
                    request,
                    PolynomialStatus::Missing,
                    None,
                    vec!["evaluation point is required".into()],
                );
            };
            PolynomialArtifact::Value(evaluate(left, point))
        }
        PolynomialOperation::Roots => {
            let roots = (0..left.modulus)
                .filter(|point| evaluate(left, *point) == 0)
                .collect();
            PolynomialArtifact::Roots(roots)
        }
        PolynomialOperation::FactorQuadratic => {
            if left.coefficients.len() != 3 {
                return result(
                    request,
                    PolynomialStatus::Unsupported,
                    None,
                    vec!["only exact quadratic factorization is supported".into()],
                );
            }
            let roots: Vec<u64> = (0..left.modulus)
                .filter(|point| evaluate(left, *point) == 0)
                .collect();
            if roots.len() != 2 {
                return result(
                    request,
                    PolynomialStatus::Unsupported,
                    None,
                    vec!["quadratic must have two distinct field roots".into()],
                );
            }
            PolynomialArtifact::Factors(vec![
                make(
                    vec![(left.modulus - roots[0]) % left.modulus, 1],
                    left.modulus,
                ),
                make(
                    vec![(left.modulus - roots[1]) % left.modulus, 1],
                    left.modulus,
                ),
            ])
        }
        PolynomialOperation::MinimalPolynomial => {
            return result(
                request,
                PolynomialStatus::Unsupported,
                None,
                vec!["minimal-polynomial reasoning requires a validated linear-map witness".into()],
            );
        }
    };
    result(
        request,
        PolynomialStatus::Complete,
        Some(artifact),
        Vec::new(),
    )
}

impl PolynomialResult {
    pub fn replay_verified(&self) -> bool {
        self.replay_hash == digest(&payload(self)) && !self.provenance.is_empty()
    }
}
