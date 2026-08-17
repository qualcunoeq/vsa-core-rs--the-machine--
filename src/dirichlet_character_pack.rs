//! Bounded finite Dirichlet characters.
//!
//! This is a narrow algebraic foundation, not analytic number theory.  It
//! evaluates characters of the multiplicative group modulo a small prime and
//! represents values as exact roots of unity.  Partial sums are retained as
//! exact exponent histograms; no floating-point complex approximation,
//! asymptotic estimate, or unbounded character theory is authorized.

use crate::source_formula_pack::SourceCitation;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;

const MAX_PRIME: u32 = 31;
const MAX_SUM_LIMIT: u32 = 256;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum CharacterOperation {
    ValidateCharacter,
    Evaluate,
    PartialSum,
    Orthogonality,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum CharacterStatus {
    Complete,
    Missing,
    Ambiguous,
    Unsupported,
    InvalidDomain,
    Inconsistent,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum CharacterValue {
    Zero,
    RootOfUnity { order: u32, exponent: u32 },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum CharacterArtifact {
    Character {
        modulus: u32,
        exponent: u32,
        generator: u32,
    },
    Value {
        input: i64,
        value: CharacterValue,
    },
    PartialSum {
        limit: u32,
        zero_terms: u32,
        root_exponents: BTreeMap<u32, u32>,
    },
    Orthogonality {
        nontrivial_sum_is_zero: bool,
        root_exponents: BTreeMap<u32, u32>,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DirichletCharacterRequest {
    pub operation: CharacterOperation,
    pub modulus: Option<u32>,
    /// Character exponent in the cyclic dual group; `0 <= exponent < p - 1`.
    pub exponent: Option<u32>,
    pub value: Option<i64>,
    pub sum_limit: Option<u32>,
    pub domain: String,
    pub ambiguity: Option<String>,
    pub provenance: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CharacterResult {
    pub status: CharacterStatus,
    pub operation: CharacterOperation,
    pub artifact: Option<CharacterArtifact>,
    pub source: SourceCitation,
    pub assumptions: Vec<String>,
    pub reasons: Vec<String>,
    pub provenance: Vec<String>,
    pub replay_hash: String,
}

fn source() -> SourceCitation {
    SourceCitation {
        source_id: "mit-ocw-18-785:finite-dirichlet-characters".into(),
        title: "Analytic Number Theory, MIT OpenCourseWare 18.785".into(),
        section: "characters of the finite multiplicative group modulo a prime".into(),
        url: "https://ocw.mit.edu/courses/18-785-number-theory-i-fall-2012/".into(),
        license: "MIT OpenCourseWare attribution required".into(),
        retrieved_utc: "2026-08-17".into(),
        evidence_span: "finite multiplicative characters and orthogonality".into(),
    }
}

fn digest<T: Serialize>(value: &T) -> String {
    format!("{:x}", Sha256::digest(serde_json::to_vec(value).unwrap()))
}

fn payload(result: &CharacterResult) -> impl Serialize + '_ {
    (
        result.status,
        result.operation,
        &result.artifact,
        &result.source,
        &result.assumptions,
        &result.reasons,
        &result.provenance,
    )
}

fn output(
    request: &DirichletCharacterRequest,
    status: CharacterStatus,
    artifact: Option<CharacterArtifact>,
    assumptions: Vec<String>,
    reasons: Vec<String>,
) -> CharacterResult {
    let mut result = CharacterResult {
        status,
        operation: request.operation,
        artifact,
        source: source(),
        assumptions,
        reasons,
        provenance: request.provenance.clone(),
        replay_hash: String::new(),
    };
    let replay_hash = digest(&payload(&result));
    result.replay_hash = replay_hash;
    result
}

fn is_prime(value: u32) -> bool {
    if value < 2 {
        return false;
    }
    let mut divisor = 2;
    while divisor * divisor <= value {
        if value % divisor == 0 {
            return false;
        }
        divisor += 1;
    }
    true
}

fn prime_factors(mut value: u32) -> Vec<u32> {
    let mut factors = Vec::new();
    let mut divisor = 2;
    while divisor * divisor <= value {
        if value % divisor == 0 {
            factors.push(divisor);
            while value % divisor == 0 {
                value /= divisor;
            }
        }
        divisor += 1;
    }
    if value > 1 {
        factors.push(value);
    }
    factors
}

fn pow_mod(mut base: u32, mut exponent: u32, modulus: u32) -> u32 {
    let mut result = 1u32;
    while exponent > 0 {
        if exponent & 1 == 1 {
            result = result * base % modulus;
        }
        base = base * base % modulus;
        exponent >>= 1;
    }
    result
}

fn primitive_root(prime: u32) -> Option<u32> {
    if prime == 2 {
        return Some(1);
    }
    let order = prime - 1;
    let factors = prime_factors(order);
    (2..prime).find(|candidate| {
        factors
            .iter()
            .all(|factor| pow_mod(*candidate, order / factor, prime) != 1)
    })
}

fn discrete_log(value: u32, generator: u32, prime: u32) -> Option<u32> {
    let mut current = 1;
    for exponent in 0..prime - 1 {
        if current == value {
            return Some(exponent);
        }
        current = current * generator % prime;
    }
    None
}

fn validate_parameters(
    request: &DirichletCharacterRequest,
) -> Result<(u32, u32, u32), CharacterStatus> {
    let Some(prime) = request.modulus else {
        return Err(CharacterStatus::Missing);
    };
    let Some(exponent) = request.exponent else {
        return Err(CharacterStatus::Missing);
    };
    if prime > MAX_PRIME && is_prime(prime) {
        return Err(CharacterStatus::Unsupported);
    }
    if !is_prime(prime) || prime < 2 {
        return Err(CharacterStatus::Unsupported);
    }
    if exponent >= prime - 1 {
        return Err(CharacterStatus::Inconsistent);
    }
    let generator = primitive_root(prime).ok_or(CharacterStatus::Unsupported)?;
    Ok((prime, exponent, generator))
}

fn character_value(input: i64, prime: u32, exponent: u32, generator: u32) -> CharacterValue {
    let residue = input.rem_euclid(prime as i64) as u32;
    if residue == 0 {
        CharacterValue::Zero
    } else {
        let logarithm =
            discrete_log(residue, generator, prime).expect("primitive root spans group");
        CharacterValue::RootOfUnity {
            order: prime - 1,
            exponent: exponent * logarithm % (prime - 1),
        }
    }
}

fn partial_sum(limit: u32, prime: u32, exponent: u32, generator: u32) -> (u32, BTreeMap<u32, u32>) {
    let mut zero_terms = 0;
    let mut root_exponents = BTreeMap::new();
    for input in 1..=limit {
        match character_value(input as i64, prime, exponent, generator) {
            CharacterValue::Zero => zero_terms += 1,
            CharacterValue::RootOfUnity { exponent, .. } => {
                *root_exponents.entry(exponent).or_insert(0) += 1;
            }
        }
    }
    (zero_terms, root_exponents)
}

/// Evaluate a bounded exact Dirichlet-character request.
pub fn evaluate(request: &DirichletCharacterRequest) -> CharacterResult {
    if request.domain != "bounded_dirichlet_character" {
        return output(
            request,
            CharacterStatus::InvalidDomain,
            None,
            Vec::new(),
            vec!["domain is outside the bounded finite-character contract".into()],
        );
    }
    if let Some(ambiguity) = &request.ambiguity {
        return output(
            request,
            CharacterStatus::Ambiguous,
            None,
            Vec::new(),
            vec![ambiguity.clone()],
        );
    }
    let (prime, exponent, generator) = match validate_parameters(request) {
        Ok(parameters) => parameters,
        Err(status) => {
            return output(
                request,
                status,
                None,
                Vec::new(),
                vec![
                    "modulus and character exponent do not satisfy the finite prime contract"
                        .into(),
                ],
            )
        }
    };
    let assumptions = vec![
        "prime modulus at most 31".into(),
        "character values are exact roots of unity, not floating-point complex numbers".into(),
        "analytic continuation and asymptotic estimates are outside scope".into(),
    ];
    let artifact = match request.operation {
        CharacterOperation::ValidateCharacter => CharacterArtifact::Character {
            modulus: prime,
            exponent,
            generator,
        },
        CharacterOperation::Evaluate => {
            let Some(value) = request.value else {
                return output(
                    request,
                    CharacterStatus::Missing,
                    None,
                    assumptions,
                    vec!["an input value is required".into()],
                );
            };
            CharacterArtifact::Value {
                input: value,
                value: character_value(value, prime, exponent, generator),
            }
        }
        CharacterOperation::PartialSum => {
            let Some(limit) = request.sum_limit else {
                return output(
                    request,
                    CharacterStatus::Missing,
                    None,
                    assumptions,
                    vec!["a finite summation limit is required".into()],
                );
            };
            if limit == 0 || limit > MAX_SUM_LIMIT {
                return output(
                    request,
                    CharacterStatus::Unsupported,
                    None,
                    assumptions,
                    vec!["summation limit is outside the bounded range".into()],
                );
            }
            let (zero_terms, root_exponents) = partial_sum(limit, prime, exponent, generator);
            CharacterArtifact::PartialSum {
                limit,
                zero_terms,
                root_exponents,
            }
        }
        CharacterOperation::Orthogonality => {
            let (zero_terms, root_exponents) = partial_sum(prime - 1, prime, exponent, generator);
            let nontrivial_sum_is_zero = if exponent == 0 {
                root_exponents == BTreeMap::from([(0, prime - 1)])
            } else {
                root_exponents.values().all(|count| count == &1)
                    && root_exponents.len() as u32 == prime - 1
            };
            let _ = zero_terms;
            CharacterArtifact::Orthogonality {
                nontrivial_sum_is_zero,
                root_exponents,
            }
        }
    };
    output(
        request,
        CharacterStatus::Complete,
        Some(artifact),
        assumptions,
        Vec::new(),
    )
}

impl CharacterResult {
    pub fn replay_verified(&self) -> bool {
        self.replay_hash == digest(&payload(self))
            && !self.provenance.is_empty()
            && (self.status != CharacterStatus::Complete || self.artifact.is_some())
    }

    pub fn authorized(&self) -> bool {
        self.status == CharacterStatus::Complete && self.replay_verified()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn request(operation: CharacterOperation) -> DirichletCharacterRequest {
        DirichletCharacterRequest {
            operation,
            modulus: Some(5),
            exponent: Some(1),
            value: Some(2),
            sum_limit: Some(8),
            domain: "bounded_dirichlet_character".into(),
            ambiguity: None,
            provenance: vec!["test".into()],
        }
    }

    #[test]
    fn evaluates_exact_root_of_unity_and_zero() {
        let result = evaluate(&request(CharacterOperation::Evaluate));
        assert!(result.authorized());
        assert_eq!(
            result.artifact,
            Some(CharacterArtifact::Value {
                input: 2,
                value: CharacterValue::RootOfUnity {
                    order: 4,
                    exponent: 1,
                },
            })
        );
        let mut zero = request(CharacterOperation::Evaluate);
        zero.value = Some(5);
        assert_eq!(
            evaluate(&zero).artifact,
            Some(CharacterArtifact::Value {
                input: 5,
                value: CharacterValue::Zero,
            })
        );
    }

    #[test]
    fn nontrivial_orthogonality_is_exact() {
        let result = evaluate(&request(CharacterOperation::Orthogonality));
        assert!(result.authorized());
        assert_eq!(
            result.artifact,
            Some(CharacterArtifact::Orthogonality {
                nontrivial_sum_is_zero: true,
                root_exponents: BTreeMap::from([(0, 1), (1, 1), (2, 1), (3, 1)]),
            })
        );
    }

    #[test]
    fn composite_modulus_is_refused() {
        let mut request = request(CharacterOperation::ValidateCharacter);
        request.modulus = Some(9);
        assert_eq!(evaluate(&request).status, CharacterStatus::Unsupported);
    }

    #[test]
    fn tampering_breaks_replay() {
        let mut result = evaluate(&request(CharacterOperation::ValidateCharacter));
        assert!(result.replay_verified());
        result.replay_hash = "tampered".into();
        assert!(!result.replay_verified());
    }
}
