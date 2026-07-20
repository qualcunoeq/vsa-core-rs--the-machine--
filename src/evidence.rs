//! Shared evidence provenance and acceptance policy primitives.

use serde::Serialize;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum EvidenceOrigin {
    Prompt,
    Clarification,
    Derived,
    Retrieved,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum EvidenceStatus {
    Explicit,
    Confirmed,
    Inferred,
    Rejected,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct EvidenceItem {
    pub content: String,
    pub origin: EvidenceOrigin,
    pub status: EvidenceStatus,
    pub provenance: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct EvidencePolicy {
    pub allowed_origins: Vec<EvidenceOrigin>,
    pub allowed_statuses: Vec<EvidenceStatus>,
    pub require_provenance: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum EvidencePolicyRejection {
    OriginNotAllowed,
    StatusNotAllowed,
    ProvenanceMissing,
}

impl EvidencePolicy {
    pub fn strict_prompt_confirmed() -> Self {
        Self {
            allowed_origins: vec![EvidenceOrigin::Prompt, EvidenceOrigin::Clarification],
            allowed_statuses: vec![EvidenceStatus::Explicit, EvidenceStatus::Confirmed],
            require_provenance: true,
        }
    }

    pub fn evaluate(&self, evidence: &EvidenceItem) -> Result<(), EvidencePolicyRejection> {
        if !self.allowed_origins.contains(&evidence.origin) {
            return Err(EvidencePolicyRejection::OriginNotAllowed);
        }
        if !self.allowed_statuses.contains(&evidence.status) {
            return Err(EvidencePolicyRejection::StatusNotAllowed);
        }
        if self.require_provenance && evidence.provenance.trim().is_empty() {
            return Err(EvidencePolicyRejection::ProvenanceMissing);
        }
        Ok(())
    }

    pub fn valid(&self) -> bool {
        !self.allowed_origins.is_empty() && !self.allowed_statuses.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strict_policy_accepts_prompt_and_rejects_inferred_derived_evidence() {
        let policy = EvidencePolicy::strict_prompt_confirmed();
        let prompt = EvidenceItem {
            content: "constant rate".into(),
            origin: EvidenceOrigin::Prompt,
            status: EvidenceStatus::Explicit,
            provenance: "prompt span".into(),
        };
        let derived = EvidenceItem {
            content: "zero intercept".into(),
            origin: EvidenceOrigin::Derived,
            status: EvidenceStatus::Inferred,
            provenance: "model output".into(),
        };
        assert_eq!(policy.evaluate(&prompt), Ok(()));
        assert_eq!(
            policy.evaluate(&derived),
            Err(EvidencePolicyRejection::OriginNotAllowed)
        );
    }

    #[test]
    fn policy_requires_provenance_when_configured() {
        let policy = EvidencePolicy {
            allowed_origins: vec![EvidenceOrigin::Prompt],
            allowed_statuses: vec![EvidenceStatus::Explicit],
            require_provenance: true,
        };
        let item = EvidenceItem {
            content: "fact".into(),
            origin: EvidenceOrigin::Prompt,
            status: EvidenceStatus::Explicit,
            provenance: String::new(),
        };
        assert_eq!(
            policy.evaluate(&item),
            Err(EvidencePolicyRejection::ProvenanceMissing)
        );
    }
}
