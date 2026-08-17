//! Source-attributed bounded arithmetic functions.
//!
//! This is a finite exact prerequisite layer for number theory.  It evaluates
//! divisor counts/sums, the Möbius function, and the prime-counting function
//! under a strict trial-factorization budget.  It does not authorize
//! asymptotics, analytic continuation, Dirichlet-series estimates, or
//! unbounded factorization.

use crate::source_formula_pack::SourceCitation;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

const MAX_INPUT: u64 = 100_000;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ArithmeticFunctionOperation {
    DivisorCount,
    DivisorSum,
    Mobius,
    PrimeCounting,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ArithmeticFunctionStatus {
    Complete,
    Missing,
    Ambiguous,
    Unsupported,
    InvalidDomain,
    Inconsistent,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum ArithmeticFunctionArtifact {
    DivisorCertificate {
        value: u64,
        prime_factors: Vec<(u64, u32)>,
        divisor_count: u64,
        divisor_sum: u64,
    },
    Mobius {
        value: u64,
        result: i8,
    },
    PrimeCounting {
        value: u64,
        count: u64,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ArithmeticFunctionRequest {
    pub operation: ArithmeticFunctionOperation,
    pub value: Option<u64>,
    pub domain: String,
    pub ambiguity: Option<String>,
    pub provenance: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ArithmeticFunctionResult {
    pub status: ArithmeticFunctionStatus,
    pub operation: ArithmeticFunctionOperation,
    pub artifact: Option<ArithmeticFunctionArtifact>,
    pub source: SourceCitation,
    pub assumptions: Vec<String>,
    pub reasons: Vec<String>,
    pub provenance: Vec<String>,
    pub replay_hash: String,
}

fn source() -> SourceCitation {
    SourceCitation {
        source_id: "mit-ocw-18-781:bounded-arithmetic-functions".into(),
        title: "Theory of Numbers, MIT OpenCourseWare 18.781".into(),
        section: "multiplicative arithmetic functions, divisor functions, and Möbius function"
            .into(),
        url: "https://ocw.mit.edu/courses/18-781-theory-of-numbers-spring-2012/".into(),
        license: "MIT OpenCourseWare attribution required".into(),
        retrieved_utc: "2026-08-17".into(),
        evidence_span:
            "divisor functions, Möbius inversion prerequisites, and prime counting notation".into(),
    }
}

fn digest<T: Serialize>(value: &T) -> String {
    format!("{:x}", Sha256::digest(serde_json::to_vec(value).unwrap()))
}

fn payload(result: &ArithmeticFunctionResult) -> impl Serialize {
    (
        result.status,
        result.operation,
        result.artifact.clone(),
        result.source.clone(),
        result.assumptions.clone(),
        result.reasons.clone(),
        result.provenance.clone(),
    )
}

fn output(
    request: &ArithmeticFunctionRequest,
    status: ArithmeticFunctionStatus,
    artifact: Option<ArithmeticFunctionArtifact>,
    assumptions: Vec<String>,
    reasons: Vec<String>,
) -> ArithmeticFunctionResult {
    let mut result = ArithmeticFunctionResult {
        status,
        operation: request.operation,
        artifact,
        source: source(),
        assumptions,
        reasons,
        provenance: request.provenance.clone(),
        replay_hash: String::new(),
    };
    result.replay_hash = digest(&payload(&result));
    result
}

fn factors(mut value: u64) -> Vec<(u64, u32)> {
    let mut result = Vec::new();
    let mut divisor = 2;
    while divisor * divisor <= value {
        if value % divisor == 0 {
            let mut exponent = 0;
            while value % divisor == 0 {
                value /= divisor;
                exponent += 1;
            }
            result.push((divisor, exponent));
        }
        divisor += if divisor == 2 { 1 } else { 2 };
    }
    if value > 1 {
        result.push((value, 1));
    }
    result
}

fn divisor_data(value: u64) -> (Vec<(u64, u32)>, u64, u64) {
    let prime_factors = factors(value);
    let divisor_count = prime_factors
        .iter()
        .map(|(_, exponent)| u64::from(*exponent) + 1)
        .product();
    let divisor_sum = prime_factors
        .iter()
        .map(|(prime, exponent)| (0..=*exponent).map(|power| prime.pow(power)).sum::<u64>())
        .product();
    (prime_factors, divisor_count, divisor_sum)
}

fn is_prime(value: u64) -> bool {
    value >= 2 && factors(value).len() == 1 && factors(value)[0].1 == 1
}

/// Evaluate a finite exact arithmetic-function request.
pub fn evaluate(request: &ArithmeticFunctionRequest) -> ArithmeticFunctionResult {
    if request.domain != "bounded_arithmetic_functions" {
        return output(
            request,
            ArithmeticFunctionStatus::InvalidDomain,
            None,
            Vec::new(),
            vec!["domain is outside the bounded arithmetic-functions contract".into()],
        );
    }
    if let Some(ambiguity) = &request.ambiguity {
        return output(
            request,
            ArithmeticFunctionStatus::Ambiguous,
            None,
            Vec::new(),
            vec![ambiguity.clone()],
        );
    }
    let Some(value) = request.value else {
        return output(
            request,
            ArithmeticFunctionStatus::Missing,
            None,
            Vec::new(),
            vec!["a positive integer input is required".into()],
        );
    };
    if value == 0 {
        return output(
            request,
            ArithmeticFunctionStatus::Inconsistent,
            None,
            Vec::new(),
            vec!["arithmetic functions in this contract are defined on positive integers".into()],
        );
    }
    if value > MAX_INPUT {
        return output(
            request,
            ArithmeticFunctionStatus::Unsupported,
            None,
            Vec::new(),
            vec!["trial-factorization and prime-counting budget is exceeded".into()],
        );
    }
    let assumptions = vec![
        "positive integer input".into(),
        format!("bounded exact input at most {MAX_INPUT}"),
        "trial factorization is explicit and finite".into(),
        "asymptotic and analytic claims are outside scope".into(),
    ];
    match request.operation {
        ArithmeticFunctionOperation::DivisorCount | ArithmeticFunctionOperation::DivisorSum => {
            let (prime_factors, divisor_count, divisor_sum) = divisor_data(value);
            output(
                request,
                ArithmeticFunctionStatus::Complete,
                Some(ArithmeticFunctionArtifact::DivisorCertificate {
                    value,
                    prime_factors,
                    divisor_count,
                    divisor_sum,
                }),
                assumptions,
                Vec::new(),
            )
        }
        ArithmeticFunctionOperation::Mobius => {
            let prime_factors = factors(value);
            let result = if prime_factors.iter().any(|(_, exponent)| *exponent > 1) {
                0
            } else if prime_factors.len() % 2 == 0 {
                1
            } else {
                -1
            };
            output(
                request,
                ArithmeticFunctionStatus::Complete,
                Some(ArithmeticFunctionArtifact::Mobius { value, result }),
                assumptions,
                Vec::new(),
            )
        }
        ArithmeticFunctionOperation::PrimeCounting => {
            let count = (2..=value).filter(|candidate| is_prime(*candidate)).count() as u64;
            output(
                request,
                ArithmeticFunctionStatus::Complete,
                Some(ArithmeticFunctionArtifact::PrimeCounting { value, count }),
                assumptions,
                Vec::new(),
            )
        }
    }
}

impl ArithmeticFunctionResult {
    pub fn replay_verified(&self) -> bool {
        self.replay_hash == digest(&payload(self))
            && !self.provenance.is_empty()
            && (self.status != ArithmeticFunctionStatus::Complete || self.artifact.is_some())
    }

    pub fn authorized(&self) -> bool {
        self.status == ArithmeticFunctionStatus::Complete && self.replay_verified()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn request(
        operation: ArithmeticFunctionOperation,
        value: Option<u64>,
    ) -> ArithmeticFunctionRequest {
        ArithmeticFunctionRequest {
            operation,
            value,
            domain: "bounded_arithmetic_functions".into(),
            ambiguity: None,
            provenance: vec!["test".into()],
        }
    }

    #[test]
    fn divisor_certificate_and_mobius_are_exact() {
        let divisor = evaluate(&request(
            ArithmeticFunctionOperation::DivisorCount,
            Some(12),
        ));
        assert!(divisor.authorized());
        assert!(matches!(
            divisor.artifact,
            Some(ArithmeticFunctionArtifact::DivisorCertificate {
                divisor_count: 6,
                divisor_sum: 28,
                ..
            })
        ));
        let mobius = evaluate(&request(ArithmeticFunctionOperation::Mobius, Some(30)));
        assert!(matches!(
            mobius.artifact,
            Some(ArithmeticFunctionArtifact::Mobius { result: -1, .. })
        ));
    }

    #[test]
    fn boundaries_fail_closed() {
        let missing = evaluate(&request(ArithmeticFunctionOperation::PrimeCounting, None));
        assert_eq!(missing.status, ArithmeticFunctionStatus::Missing);
        let oversized = evaluate(&request(ArithmeticFunctionOperation::Mobius, Some(100_001)));
        assert_eq!(oversized.status, ArithmeticFunctionStatus::Unsupported);
        let mut analytic = request(ArithmeticFunctionOperation::PrimeCounting, Some(10));
        analytic.domain = "analytic_number_theory".into();
        assert_eq!(
            evaluate(&analytic).status,
            ArithmeticFunctionStatus::InvalidDomain
        );
    }
}
