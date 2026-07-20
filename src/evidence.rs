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

/// A transformation output is deliberately not an `EvidenceItem`.  It may
/// be consumed by later verified transformations, but it cannot silently
/// become prompt evidence for model selection.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct DerivedFact {
    pub id: String,
    pub content: String,
    pub parent_lineage: Vec<String>,
    pub provenance: String,
}

impl DerivedFact {
    pub fn as_inferred_evidence(&self) -> EvidenceItem {
        EvidenceItem {
            content: self.content.clone(),
            origin: EvidenceOrigin::Derived,
            status: EvidenceStatus::Inferred,
            provenance: self.provenance.clone(),
        }
    }
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

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub enum FactPolicyRejection {
    LineageMissing,
    StatusNotAllowed,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct FactPolicy {
    pub allowed_statuses: Vec<EvidenceStatus>,
    pub require_lineage: bool,
}

impl FactPolicy {
    pub fn verified_transformation() -> Self {
        Self {
            allowed_statuses: vec![
                EvidenceStatus::Explicit,
                EvidenceStatus::Confirmed,
                EvidenceStatus::Inferred,
            ],
            require_lineage: true,
        }
    }

    pub fn evaluate(
        &self,
        fact: &DerivedFact,
        status: EvidenceStatus,
    ) -> Result<(), FactPolicyRejection> {
        if !self.allowed_statuses.contains(&status) {
            return Err(FactPolicyRejection::StatusNotAllowed);
        }
        if self.require_lineage && fact.parent_lineage.is_empty() {
            return Err(FactPolicyRejection::LineageMissing);
        }
        Ok(())
    }
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

    #[test]
    fn derived_fact_is_transformable_but_not_promotable_to_model_evidence() {
        let fact = DerivedFact {
            id: "fact-1".into(),
            content: "x squared equals 25".into(),
            parent_lineage: vec!["prompt-span-x-equals-5".into()],
            provenance: "verified substitution".into(),
        };
        assert_eq!(
            FactPolicy::verified_transformation().evaluate(&fact, EvidenceStatus::Inferred),
            Ok(())
        );
        let inferred = fact.as_inferred_evidence();
        assert_eq!(inferred.origin, EvidenceOrigin::Derived);
        assert_eq!(inferred.status, EvidenceStatus::Inferred);
        assert_eq!(
            EvidencePolicy::strict_prompt_confirmed().evaluate(&inferred),
            Err(EvidencePolicyRejection::OriginNotAllowed)
        );
    }

    #[test]
    fn derived_fact_without_lineage_is_rejected_by_transformation_policy() {
        let fact = DerivedFact {
            id: "fact-2".into(),
            content: "unjustified result".into(),
            parent_lineage: Vec::new(),
            provenance: "unknown".into(),
        };
        assert_eq!(
            FactPolicy::verified_transformation().evaluate(&fact, EvidenceStatus::Inferred),
            Err(FactPolicyRejection::LineageMissing)
        );
    }
}
