//! Bounded exact combinatorics foundations for the curriculum.
//!
//! The pack covers finite counting identities only. Inputs are explicit and
//! bounded; weighted, infinite, asymptotic, and representation-dependent
//! counting requests are refused rather than guessed.

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

const MAX_N: u64 = 30;
const MAX_OCCUPANCY: u64 = 100;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum CombinatoricsOperation {
    Permutations,
    Combinations,
    Multinomial,
    InclusionExclusionTwo,
    PigeonholeMinimum,
    StirlingSecond,
    SurjectionCount,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum CombinatoricsStatus {
    Complete,
    Missing,
    Ambiguous,
    Unsupported,
    InvalidDomain,
    Inconsistent,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum CombinatoricsArtifact {
    Scalar(u128),
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CombinatoricsRequest {
    pub operation: CombinatoricsOperation,
    pub n: Option<u64>,
    pub k: Option<u64>,
    pub parts: Vec<u64>,
    pub first_count: Option<u64>,
    pub second_count: Option<u64>,
    pub intersection_count: Option<u64>,
    pub objects: Option<u64>,
    pub boxes: Option<u64>,
    pub domain: String,
    pub ambiguity: Option<String>,
    pub provenance: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CombinatoricsResult {
    pub status: CombinatoricsStatus,
    pub artifact: Option<CombinatoricsArtifact>,
    pub operation: CombinatoricsOperation,
    pub assumptions: Vec<String>,
    pub reasons: Vec<String>,
    pub provenance: Vec<String>,
    pub replay_hash: String,
}

fn digest<T: Serialize>(value: &T) -> String {
    format!(
        "{:x}",
        Sha256::digest(serde_json::to_vec(value).expect("combinatorics serializes"))
    )
}

fn payload(result: &CombinatoricsResult) -> impl Serialize + '_ {
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
    request: &CombinatoricsRequest,
    status: CombinatoricsStatus,
    artifact: Option<CombinatoricsArtifact>,
    assumptions: Vec<String>,
    reasons: Vec<String>,
) -> CombinatoricsResult {
    let mut output = CombinatoricsResult {
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

fn factorial(n: u64) -> u128 {
    (1..=n).fold(1u128, |acc, value| acc * u128::from(value))
}

fn choose(n: u64, k: u64) -> u128 {
    if k > n {
        return 0;
    }
    let k = k.min(n - k);
    (1..=k).fold(1u128, |acc, index| {
        acc * u128::from(n - k + index) / u128::from(index)
    })
}

fn require_nk(request: &CombinatoricsRequest) -> Result<(u64, u64), CombinatoricsStatus> {
    let (Some(n), Some(k)) = (request.n, request.k) else {
        return Err(CombinatoricsStatus::Missing);
    };
    if n > MAX_N {
        return Err(CombinatoricsStatus::Unsupported);
    }
    Ok((n, k))
}

fn stirling_second(n: u64, k: u64) -> u128 {
    let mut table = vec![vec![0u128; (k + 1) as usize]; (n + 1) as usize];
    table[0][0] = 1;
    for row in 1..=n as usize {
        for column in 1..=k.min(n) as usize {
            table[row][column] =
                table[row - 1][column - 1] + u128::from(column as u64) * table[row - 1][column];
        }
    }
    table[n as usize][k as usize]
}

fn surjection_count(n: u64, k: u64) -> u128 {
    (0..=k).fold(0i128, |sum, excluded| {
        let term = choose(k, excluded);
        let signed = if excluded % 2 == 0 {
            term as i128
        } else {
            -(term as i128)
        };
        // Number of functions from n labeled objects to the remaining boxes.
        sum + signed * (u128::from(k - excluded).pow(n as u32) as i128)
    }) as u128
}

/// Evaluate one bounded exact combinatorics request.
pub fn evaluate_combinatorics(request: &CombinatoricsRequest) -> CombinatoricsResult {
    if request.domain != "bounded_exact_combinatorics" {
        return result(
            request,
            CombinatoricsStatus::InvalidDomain,
            None,
            Vec::new(),
            vec!["domain is outside bounded exact combinatorics".into()],
        );
    }
    if let Some(ambiguity) = &request.ambiguity {
        return result(
            request,
            CombinatoricsStatus::Ambiguous,
            None,
            Vec::new(),
            vec![ambiguity.clone()],
        );
    }
    let assumptions = vec![
        "finite labeled objects where stated".into(),
        "exact integer counting".into(),
        "explicit bounded parameters".into(),
    ];
    match request.operation {
        CombinatoricsOperation::Permutations => {
            let (n, k) = match require_nk(request) {
                Ok(values) => values,
                Err(status) => {
                    return result(
                        request,
                        status,
                        None,
                        assumptions,
                        vec!["n and k must be explicit and bounded".into()],
                    )
                }
            };
            if k > n {
                return result(
                    request,
                    CombinatoricsStatus::Inconsistent,
                    None,
                    assumptions,
                    vec!["k cannot exceed n".into()],
                );
            }
            result(
                request,
                CombinatoricsStatus::Complete,
                Some(CombinatoricsArtifact::Scalar(
                    factorial(n) / factorial(n - k),
                )),
                assumptions,
                Vec::new(),
            )
        }
        CombinatoricsOperation::Combinations => {
            let (n, k) = match require_nk(request) {
                Ok(values) => values,
                Err(status) => {
                    return result(
                        request,
                        status,
                        None,
                        assumptions,
                        vec!["n and k must be explicit and bounded".into()],
                    )
                }
            };
            if k > n {
                return result(
                    request,
                    CombinatoricsStatus::Inconsistent,
                    None,
                    assumptions,
                    vec!["k cannot exceed n".into()],
                );
            }
            result(
                request,
                CombinatoricsStatus::Complete,
                Some(CombinatoricsArtifact::Scalar(choose(n, k))),
                assumptions,
                Vec::new(),
            )
        }
        CombinatoricsOperation::Multinomial => {
            if request.parts.is_empty() {
                return result(
                    request,
                    CombinatoricsStatus::Missing,
                    None,
                    assumptions,
                    vec!["a nonempty partition is required".into()],
                );
            }
            let total: u64 = request.parts.iter().sum();
            if total > MAX_N {
                return result(
                    request,
                    CombinatoricsStatus::Unsupported,
                    None,
                    assumptions,
                    vec!["partition total exceeds the bounded factorial range".into()],
                );
            }
            let denominator = request
                .parts
                .iter()
                .map(|part| factorial(*part))
                .product::<u128>();
            result(
                request,
                CombinatoricsStatus::Complete,
                Some(CombinatoricsArtifact::Scalar(
                    factorial(total) / denominator,
                )),
                assumptions,
                Vec::new(),
            )
        }
        CombinatoricsOperation::InclusionExclusionTwo => {
            let (Some(first), Some(second), Some(intersection)) = (
                request.first_count,
                request.second_count,
                request.intersection_count,
            ) else {
                return result(
                    request,
                    CombinatoricsStatus::Missing,
                    None,
                    assumptions,
                    vec!["both set counts and their intersection are required".into()],
                );
            };
            if intersection > first || intersection > second {
                return result(
                    request,
                    CombinatoricsStatus::Inconsistent,
                    None,
                    assumptions,
                    vec!["intersection cannot exceed either set".into()],
                );
            }
            result(
                request,
                CombinatoricsStatus::Complete,
                Some(CombinatoricsArtifact::Scalar(u128::from(
                    first + second - intersection,
                ))),
                assumptions,
                Vec::new(),
            )
        }
        CombinatoricsOperation::PigeonholeMinimum => {
            let (Some(objects), Some(boxes)) = (request.objects, request.boxes) else {
                return result(
                    request,
                    CombinatoricsStatus::Missing,
                    None,
                    assumptions,
                    vec!["objects and boxes are required".into()],
                );
            };
            if objects > MAX_OCCUPANCY || boxes == 0 || boxes > MAX_OCCUPANCY {
                return result(
                    request,
                    CombinatoricsStatus::Unsupported,
                    None,
                    assumptions,
                    vec!["occupancy parameters are outside the bounded range".into()],
                );
            }
            if objects == 0 {
                return result(
                    request,
                    CombinatoricsStatus::Inconsistent,
                    None,
                    assumptions,
                    vec!["at least one object is required".into()],
                );
            }
            result(
                request,
                CombinatoricsStatus::Complete,
                Some(CombinatoricsArtifact::Scalar(u128::from(
                    (objects - 1) / boxes + 1,
                ))),
                assumptions,
                Vec::new(),
            )
        }
        CombinatoricsOperation::StirlingSecond => {
            let (n, k) = match require_nk(request) {
                Ok(values) => values,
                Err(status) => {
                    return result(
                        request,
                        status,
                        None,
                        assumptions,
                        vec!["n and k must be explicit and bounded".into()],
                    )
                }
            };
            if k > n {
                return result(
                    request,
                    CombinatoricsStatus::Inconsistent,
                    None,
                    assumptions,
                    vec!["k cannot exceed n".into()],
                );
            }
            result(
                request,
                CombinatoricsStatus::Complete,
                Some(CombinatoricsArtifact::Scalar(stirling_second(n, k))),
                assumptions,
                Vec::new(),
            )
        }
        CombinatoricsOperation::SurjectionCount => {
            let (n, k) = match require_nk(request) {
                Ok(values) => values,
                Err(status) => {
                    return result(
                        request,
                        status,
                        None,
                        assumptions,
                        vec!["n and k must be explicit and bounded".into()],
                    )
                }
            };
            if k > n || n > 12 {
                return result(
                    request,
                    CombinatoricsStatus::Unsupported,
                    None,
                    assumptions,
                    vec!["surjection count exceeds the bounded inclusion-exclusion range".into()],
                );
            }
            result(
                request,
                CombinatoricsStatus::Complete,
                Some(CombinatoricsArtifact::Scalar(surjection_count(n, k))),
                assumptions,
                Vec::new(),
            )
        }
    }
}

impl CombinatoricsResult {
    pub fn replay_verified(&self) -> bool {
        self.replay_hash == digest(&payload(self))
            && !self.provenance.is_empty()
            && (self.status != CombinatoricsStatus::Complete || self.artifact.is_some())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn request(operation: CombinatoricsOperation) -> CombinatoricsRequest {
        CombinatoricsRequest {
            operation,
            n: Some(5),
            k: Some(2),
            parts: Vec::new(),
            first_count: None,
            second_count: None,
            intersection_count: None,
            objects: None,
            boxes: None,
            domain: "bounded_exact_combinatorics".into(),
            ambiguity: None,
            provenance: vec!["combinatorics-test".into()],
        }
    }

    #[test]
    fn exact_counting_and_replay() {
        let result = evaluate_combinatorics(&request(CombinatoricsOperation::Combinations));
        assert_eq!(result.artifact, Some(CombinatoricsArtifact::Scalar(10)));
        assert!(result.replay_verified());
    }

    #[test]
    fn invalid_and_tampered_cases_fail_closed() {
        let mut request = request(CombinatoricsOperation::PigeonholeMinimum);
        request.objects = Some(0);
        request.boxes = Some(3);
        let result = evaluate_combinatorics(&request);
        assert_eq!(result.status, CombinatoricsStatus::Inconsistent);
        let mut tampered = result.clone();
        tampered.replay_hash.push('x');
        assert!(!tampered.replay_verified());
    }
}
