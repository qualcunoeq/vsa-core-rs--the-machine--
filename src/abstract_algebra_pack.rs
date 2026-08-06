//! Bounded exact abstract-algebra foundations for the governed curriculum.
//!
//! This pack deliberately starts with finite cyclic groups and modular rings.
//! It validates structure and homomorphism conditions, but does not infer an
//! abstract operation table from labels or silently promote arbitrary finite
//! sets to groups, rings, fields, or quotients.

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

const MAX_MODULUS: u32 = 64;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum AbstractAlgebraOperation {
    ConstructCyclicGroup,
    ConstructModularRing,
    CheckCyclicHomomorphism,
    AdditiveOrder,
    CheckUnit,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum AbstractAlgebraStatus {
    Complete,
    Missing,
    Ambiguous,
    Unsupported,
    InvalidDomain,
    Inconsistent,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum AbstractAlgebraArtifact {
    CyclicGroup { order: u32 },
    ModularRing { modulus: u32 },
    Boolean(bool),
    Scalar(u32),
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AbstractAlgebraRequest {
    pub operation: AbstractAlgebraOperation,
    pub modulus: Option<u32>,
    pub source_modulus: Option<u32>,
    pub target_modulus: Option<u32>,
    pub element: Option<u32>,
    pub multiplier: Option<u32>,
    pub domain: String,
    pub assumptions: Vec<String>,
    pub ambiguity: Option<String>,
    pub provenance: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AbstractAlgebraResult {
    pub status: AbstractAlgebraStatus,
    pub artifact: Option<AbstractAlgebraArtifact>,
    pub operation: AbstractAlgebraOperation,
    pub assumptions: Vec<String>,
    pub reasons: Vec<String>,
    pub provenance: Vec<String>,
    pub replay_hash: String,
}

fn digest<T: Serialize>(value: &T) -> String {
    format!(
        "{:x}",
        Sha256::digest(serde_json::to_vec(value).expect("algebra serializes"))
    )
}

fn payload(result: &AbstractAlgebraResult) -> impl Serialize + '_ {
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
    request: &AbstractAlgebraRequest,
    status: AbstractAlgebraStatus,
    artifact: Option<AbstractAlgebraArtifact>,
    assumptions: Vec<String>,
    reasons: Vec<String>,
) -> AbstractAlgebraResult {
    let mut output = AbstractAlgebraResult {
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

fn modulus(request: &AbstractAlgebraRequest) -> Result<u32, AbstractAlgebraStatus> {
    match request.modulus {
        None => Err(AbstractAlgebraStatus::Missing),
        Some(value) if !(1..=MAX_MODULUS).contains(&value) => {
            Err(AbstractAlgebraStatus::Unsupported)
        }
        Some(value) => Ok(value),
    }
}

fn gcd(mut left: u32, mut right: u32) -> u32 {
    while right != 0 {
        let remainder = left % right;
        left = right;
        right = remainder;
    }
    left
}

/// Evaluate one bounded abstract-algebra request without mutating a registry.
pub fn evaluate_abstract_algebra(request: &AbstractAlgebraRequest) -> AbstractAlgebraResult {
    if request.domain != "finite_exact_abstract_algebra" {
        return result(
            request,
            AbstractAlgebraStatus::InvalidDomain,
            None,
            Vec::new(),
            vec!["domain is outside the bounded finite algebra pack".into()],
        );
    }
    if let Some(ambiguity) = &request.ambiguity {
        return result(
            request,
            AbstractAlgebraStatus::Ambiguous,
            None,
            Vec::new(),
            vec![ambiguity.clone()],
        );
    }
    let assumptions = vec![
        "finite exact structure".into(),
        "canonical residue representatives".into(),
        "modulus bounded by 64".into(),
    ];
    match request.operation {
        AbstractAlgebraOperation::ConstructCyclicGroup => match modulus(request) {
            Ok(order) => result(
                request,
                AbstractAlgebraStatus::Complete,
                Some(AbstractAlgebraArtifact::CyclicGroup { order }),
                assumptions,
                Vec::new(),
            ),
            Err(status) => result(
                request,
                status,
                None,
                assumptions,
                vec!["group order is missing or outside the bounded range".into()],
            ),
        },
        AbstractAlgebraOperation::ConstructModularRing => match modulus(request) {
            Ok(modulus) if modulus >= 2 => result(
                request,
                AbstractAlgebraStatus::Complete,
                Some(AbstractAlgebraArtifact::ModularRing { modulus }),
                assumptions,
                Vec::new(),
            ),
            Ok(_) => result(
                request,
                AbstractAlgebraStatus::Unsupported,
                None,
                assumptions,
                vec!["the modular ring boundary starts at Z/2Z".into()],
            ),
            Err(status) => result(
                request,
                status,
                None,
                assumptions,
                vec!["ring modulus is missing or outside the bounded range".into()],
            ),
        },
        AbstractAlgebraOperation::CheckCyclicHomomorphism => {
            let (Some(source), Some(target), Some(multiplier)) = (
                request.source_modulus,
                request.target_modulus,
                request.multiplier,
            ) else {
                return result(
                    request,
                    AbstractAlgebraStatus::Missing,
                    None,
                    assumptions,
                    vec!["source modulus, target modulus, and multiplier are required".into()],
                );
            };
            if !(1..=MAX_MODULUS).contains(&source) || !(1..=MAX_MODULUS).contains(&target) {
                return result(
                    request,
                    AbstractAlgebraStatus::Unsupported,
                    None,
                    assumptions,
                    vec!["cyclic homomorphism moduli exceed the bounded range".into()],
                );
            }
            let well_defined = (u64::from(multiplier) * u64::from(source)) % u64::from(target) == 0;
            result(
                request,
                AbstractAlgebraStatus::Complete,
                Some(AbstractAlgebraArtifact::Boolean(well_defined)),
                assumptions,
                vec!["f([x]) = [multiplier · x] is well-defined exactly when target modulus divides multiplier · source modulus".into()],
            )
        }
        AbstractAlgebraOperation::AdditiveOrder => {
            let (Some(modulus), Some(element)) = (request.modulus, request.element) else {
                return result(
                    request,
                    AbstractAlgebraStatus::Missing,
                    None,
                    assumptions,
                    vec!["modulus and residue element are required".into()],
                );
            };
            if !(1..=MAX_MODULUS).contains(&modulus) {
                return result(
                    request,
                    AbstractAlgebraStatus::Unsupported,
                    None,
                    assumptions,
                    vec!["modulus exceeds the bounded range".into()],
                );
            }
            if element >= modulus {
                return result(
                    request,
                    AbstractAlgebraStatus::Inconsistent,
                    None,
                    assumptions,
                    vec!["element must be a canonical residue representative".into()],
                );
            }
            result(
                request,
                AbstractAlgebraStatus::Complete,
                Some(AbstractAlgebraArtifact::Scalar(
                    modulus / gcd(modulus, element),
                )),
                assumptions,
                Vec::new(),
            )
        }
        AbstractAlgebraOperation::CheckUnit => {
            let (Some(modulus), Some(element)) = (request.modulus, request.element) else {
                return result(
                    request,
                    AbstractAlgebraStatus::Missing,
                    None,
                    assumptions,
                    vec!["modulus and residue element are required".into()],
                );
            };
            if !(2..=MAX_MODULUS).contains(&modulus) {
                return result(
                    request,
                    AbstractAlgebraStatus::Unsupported,
                    None,
                    assumptions,
                    vec!["unit checking requires a bounded modular ring".into()],
                );
            }
            if element >= modulus {
                return result(
                    request,
                    AbstractAlgebraStatus::Inconsistent,
                    None,
                    assumptions,
                    vec!["element must be a canonical residue representative".into()],
                );
            }
            result(
                request,
                AbstractAlgebraStatus::Complete,
                Some(AbstractAlgebraArtifact::Boolean(gcd(modulus, element) == 1)),
                assumptions,
                Vec::new(),
            )
        }
    }
}

impl AbstractAlgebraResult {
    pub fn replay_verified(&self) -> bool {
        self.replay_hash == digest(&payload(self))
            && !self.provenance.is_empty()
            && (self.status != AbstractAlgebraStatus::Complete || self.artifact.is_some())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn request(operation: AbstractAlgebraOperation) -> AbstractAlgebraRequest {
        AbstractAlgebraRequest {
            operation,
            modulus: Some(12),
            source_modulus: None,
            target_modulus: None,
            element: Some(5),
            multiplier: None,
            domain: "finite_exact_abstract_algebra".into(),
            assumptions: Vec::new(),
            ambiguity: None,
            provenance: vec!["abstract-algebra-test".into()],
        }
    }

    #[test]
    fn cyclic_group_and_unit_are_replayable() {
        let group =
            evaluate_abstract_algebra(&request(AbstractAlgebraOperation::ConstructCyclicGroup));
        assert_eq!(group.status, AbstractAlgebraStatus::Complete);
        assert!(group.replay_verified());
        let unit = evaluate_abstract_algebra(&request(AbstractAlgebraOperation::CheckUnit));
        assert_eq!(unit.artifact, Some(AbstractAlgebraArtifact::Boolean(true)));
        assert!(unit.replay_verified());
    }

    #[test]
    fn homomorphism_boundary_is_exact() {
        let mut request = request(AbstractAlgebraOperation::CheckCyclicHomomorphism);
        request.modulus = None;
        request.source_modulus = Some(4);
        request.target_modulus = Some(6);
        request.multiplier = Some(3);
        let result = evaluate_abstract_algebra(&request);
        assert_eq!(
            result.artifact,
            Some(AbstractAlgebraArtifact::Boolean(true))
        );
        assert!(result.replay_verified());
    }

    #[test]
    fn missing_and_tampered_results_fail_closed() {
        let mut request = request(AbstractAlgebraOperation::AdditiveOrder);
        request.modulus = None;
        let result = evaluate_abstract_algebra(&request);
        assert_eq!(result.status, AbstractAlgebraStatus::Missing);
        let mut tampered = result.clone();
        tampered.replay_hash.push('x');
        assert!(!tampered.replay_verified());
    }
}
