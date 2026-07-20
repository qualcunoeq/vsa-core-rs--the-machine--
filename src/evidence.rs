//! Shared evidence provenance and acceptance policy primitives.

use serde::Serialize;
use std::collections::{BTreeMap, BTreeSet};

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

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum DerivedProofKind {
    ExactTransformation,
    ApproximateTransformation,
    Measurement,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum FactPrecision {
    Exact,
    Approximate,
    Measured,
}

impl DerivedProofKind {
    pub fn compose(self, other: Self) -> Self {
        match (self, other) {
            (Self::Measurement, _) | (_, Self::Measurement) => Self::Measurement,
            (Self::ApproximateTransformation, _)
            | (_, Self::ApproximateTransformation) => Self::ApproximateTransformation,
            (Self::ExactTransformation, Self::ExactTransformation) => Self::ExactTransformation,
        }
    }
}

impl FactPrecision {
    pub fn compose(self, other: Self) -> Self {
        match (self, other) {
            (Self::Measured, _) | (_, Self::Measured) => Self::Measured,
            (Self::Approximate, _) | (_, Self::Approximate) => Self::Approximate,
            (Self::Exact, Self::Exact) => Self::Exact,
        }
    }
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
    pub proof_kind: DerivedProofKind,
    pub precision: FactPrecision,
    pub assumptions: Vec<String>,
    pub domain: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum FactDerivationRejection {
    NoParents,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct FactConflict {
    pub key: String,
    pub fact_ids: Vec<String>,
    pub contents: Vec<String>,
    pub lineages: Vec<Vec<String>>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub enum FactIndexRejection {
    Policy(FactPolicyRejection),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub enum FactIndexInsert {
    Added,
    Conflict(FactConflict),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub enum FactIndexQueryFailure {
    Conflict(FactConflict),
    Unavailable {
        key: String,
        facts: Vec<(String, FactStatus)>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub enum FactSelectionRejection {
    LineageMissing,
    ProofKindNotAllowed,
    PrecisionNotAllowed,
    DomainNotAllowed,
    AssumptionsNotAllowed,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub enum FactSelectionFailure {
    Conflict(FactConflict),
    Unavailable {
        key: String,
        facts: Vec<(String, FactStatus)>,
    },
    NoAcceptableFacts {
        key: String,
        rejections: Vec<(String, FactSelectionRejection)>,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum FactStatus {
    Active,
    Superseded,
    Invalidated,
    Conflicted,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct FactLifecycle {
    pub status: FactStatus,
    pub cause: Option<String>,
    pub replacement: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum FactStateTransitionRejection {
    UnknownFact,
}

/// Consumer-side policy for selecting facts from the ledger.  This is
/// intentionally separate from `FactPolicy`: the latter governs whether a
/// transformation may create/consume a lineage-bearing fact at all, while
/// this policy describes which semantic quality a particular goal accepts.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct FactSelectionPolicy {
    pub allowed_proof_kinds: Vec<DerivedProofKind>,
    pub allowed_precision: Vec<FactPrecision>,
    pub allowed_domains: Vec<String>,
    pub require_lineage: bool,
    pub allow_assumptions: bool,
}

impl FactSelectionPolicy {
    pub fn exact_algebra() -> Self {
        Self {
            allowed_proof_kinds: vec![DerivedProofKind::ExactTransformation],
            allowed_precision: vec![FactPrecision::Exact],
            allowed_domains: Vec::new(),
            require_lineage: true,
            allow_assumptions: false,
        }
    }

    pub fn measured_numerical() -> Self {
        Self {
            allowed_proof_kinds: vec![
                DerivedProofKind::ExactTransformation,
                DerivedProofKind::ApproximateTransformation,
                DerivedProofKind::Measurement,
            ],
            allowed_precision: vec![FactPrecision::Exact, FactPrecision::Approximate, FactPrecision::Measured],
            allowed_domains: Vec::new(),
            require_lineage: true,
            allow_assumptions: true,
        }
    }

    pub fn evaluate(&self, fact: &DerivedFact) -> Result<(), FactSelectionRejection> {
        if self.require_lineage && fact.parent_lineage.is_empty() {
            return Err(FactSelectionRejection::LineageMissing);
        }
        if !self.allowed_proof_kinds.contains(&fact.proof_kind) {
            return Err(FactSelectionRejection::ProofKindNotAllowed);
        }
        if !self.allowed_precision.contains(&fact.precision) {
            return Err(FactSelectionRejection::PrecisionNotAllowed);
        }
        if !self.allowed_domains.is_empty()
            && fact
                .domain
                .as_ref()
                .map(|domain| self.allowed_domains.contains(domain))
                != Some(true)
        {
            return Err(FactSelectionRejection::DomainNotAllowed);
        }
        if !self.allow_assumptions && !fact.assumptions.is_empty() {
            return Err(FactSelectionRejection::AssumptionsNotAllowed);
        }
        Ok(())
    }
}

/// A small relevance index for derived facts.  Keys are supplied by the
/// producer (for example `distance` or `equation:lhs`) rather than guessed
/// from prose.  Facts are accepted only after lineage validation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Default)]
pub struct DerivedFactIndex {
    entries: BTreeMap<String, Vec<DerivedFact>>,
    lifecycle: BTreeMap<String, FactLifecycle>,
}

impl DerivedFactIndex {
    pub fn insert(
        &mut self,
        key: impl Into<String>,
        fact: DerivedFact,
        policy: &FactPolicy,
    ) -> Result<FactIndexInsert, FactIndexRejection> {
        policy
            .evaluate(&fact, EvidenceStatus::Inferred)
            .map_err(FactIndexRejection::Policy)?;
        let key = key.into();
        let fact_id = fact.id.clone();
        let conflict = {
            let entries = self.entries.entry(key.clone()).or_default();
            entries.push(fact);
            let mut contents = entries
                .iter()
                .map(|entry| entry.content.clone())
                .collect::<Vec<_>>();
            contents.sort();
            contents.dedup();
            contents.len() > 1
        };
        if conflict {
            let receipt = self.conflict_for(&key);
            for fact_id in &receipt.fact_ids {
                self.lifecycle.insert(
                    fact_id.clone(),
                    FactLifecycle {
                        status: FactStatus::Conflicted,
                        cause: Some(format!("conflict:{key}")),
                        replacement: None,
                    },
                );
            }
            Ok(FactIndexInsert::Conflict(receipt))
        } else {
            self.lifecycle.insert(
                fact_id,
                FactLifecycle {
                    status: FactStatus::Active,
                    cause: None,
                    replacement: None,
                },
            );
            Ok(FactIndexInsert::Added)
        }
    }

    pub fn keys(&self) -> impl Iterator<Item = &String> {
        self.entries.keys()
    }

    pub fn fact(&self, id: &str) -> Option<&DerivedFact> {
        self.entries
            .values()
            .flat_map(|facts| facts.iter())
            .find(|fact| fact.id == id)
    }

    pub fn lifecycle(&self, id: &str) -> Option<&FactLifecycle> {
        self.lifecycle.get(id)
    }

    pub fn ancestors_of(&self, id: &str) -> Vec<String> {
        self.fact(id)
            .map(|fact| fact.parent_lineage.clone())
            .unwrap_or_default()
    }

    /// Return all facts whose lineage depends on `id`.  Because lineage is
    /// transitive, this also finds indirect dependents, not only immediate
    /// children.
    pub fn dependents_of(&self, id: &str) -> Vec<String> {
        let mut dependents = BTreeSet::new();
        for facts in self.entries.values() {
            for fact in facts {
                if fact.parent_lineage.iter().any(|parent| parent == id) {
                    dependents.insert(fact.id.clone());
                }
            }
        }
        dependents.into_iter().collect()
    }

    /// Compute the transitive invalidation closure for a changed parent.
    /// This is diagnostic/lifecycle information only; it never silently
    /// deletes or rewrites facts.
    pub fn invalidation_closure(&self, id: &str) -> Vec<String> {
        let mut closure = BTreeSet::new();
        let mut frontier = vec![id.to_string()];
        while let Some(current) = frontier.pop() {
            for dependent in self.dependents_of(&current) {
                if closure.insert(dependent.clone()) {
                    frontier.push(dependent);
                }
            }
        }
        closure.into_iter().collect()
    }

    pub fn candidates(&self, key: &str) -> &[DerivedFact] {
        self.entries.get(key).map(Vec::as_slice).unwrap_or(&[])
    }

    /// Return facts only when the key is internally consistent.  A caller
    /// must resolve the conflict before allowing any downstream action.
    pub fn usable(&self, key: &str) -> Result<&[DerivedFact], FactIndexQueryFailure> {
        let candidates = self.candidates(key);
        let mut contents = candidates
            .iter()
            .map(|entry| entry.content.clone())
            .collect::<Vec<_>>();
        contents.sort();
        contents.dedup();
        if contents.len() > 1 {
            Err(FactIndexQueryFailure::Conflict(self.conflict_for(key)))
        } else {
            let unavailable = candidates
                .iter()
                .filter_map(|fact| {
                    self.lifecycle(fact.id.as_str())
                        .filter(|state| state.status != FactStatus::Active)
                        .map(|state| (fact.id.clone(), state.status))
                })
                .collect::<Vec<_>>();
            if !unavailable.is_empty() {
                Err(FactIndexQueryFailure::Unavailable {
                    key: key.to_string(),
                    facts: unavailable,
                })
            } else {
                Ok(candidates)
            }
        }
    }

    pub fn select(
        &self,
        key: &str,
        policy: &FactSelectionPolicy,
    ) -> Result<Vec<&DerivedFact>, FactSelectionFailure> {
        let candidates = self
            .usable(key)
            .map_err(|failure| match failure {
                FactIndexQueryFailure::Conflict(conflict) => FactSelectionFailure::Conflict(conflict),
                FactIndexQueryFailure::Unavailable { key, facts } => {
                    FactSelectionFailure::Unavailable { key, facts }
                }
            })?;
        let mut accepted = Vec::new();
        let mut rejected = Vec::new();
        for fact in candidates {
            match policy.evaluate(fact) {
                Ok(()) => accepted.push(fact),
                Err(reason) => rejected.push((fact.id.clone(), reason)),
            }
        }
        if accepted.is_empty() {
            Err(FactSelectionFailure::NoAcceptableFacts {
                key: key.to_string(),
                rejections: rejected,
            })
        } else {
            Ok(accepted)
        }
    }

    pub fn invalidate(
        &mut self,
        id: &str,
        cause: impl Into<String>,
        replacement: Option<String>,
    ) -> Result<Vec<String>, FactStateTransitionRejection> {
        if self.fact(id).is_none() {
            return Err(FactStateTransitionRejection::UnknownFact);
        }
        let cause = cause.into();
        let mut affected = self.invalidation_closure(id);
        affected.push(id.to_string());
        affected.sort();
        affected.dedup();
        for affected_id in &affected {
            self.lifecycle.insert(
                affected_id.clone(),
                FactLifecycle {
                    status: FactStatus::Invalidated,
                    cause: Some(cause.clone()),
                    replacement: replacement.clone(),
                },
            );
        }
        Ok(affected)
    }

    pub fn supersede(
        &mut self,
        id: &str,
        replacement: impl Into<String>,
        cause: impl Into<String>,
    ) -> Result<Vec<String>, FactStateTransitionRejection> {
        if self.fact(id).is_none() {
            return Err(FactStateTransitionRejection::UnknownFact);
        }
        let replacement = replacement.into();
        let cause = cause.into();
        let affected = self.invalidation_closure(id);
        self.lifecycle.insert(
            id.to_string(),
            FactLifecycle {
                status: FactStatus::Superseded,
                cause: Some(cause.clone()),
                replacement: Some(replacement.clone()),
            },
        );
        for affected_id in &affected {
            self.lifecycle.insert(
                affected_id.clone(),
                FactLifecycle {
                    status: FactStatus::Invalidated,
                    cause: Some(cause.clone()),
                    replacement: Some(replacement.clone()),
                },
            );
        }
        Ok(affected)
    }

    pub fn conflicts(&self) -> Vec<FactConflict> {
        self.entries
            .iter()
            .filter_map(|(key, entries)| {
                let mut contents = entries
                    .iter()
                    .map(|entry| entry.content.clone())
                    .collect::<Vec<_>>();
                contents.sort();
                contents.dedup();
                (contents.len() > 1).then(|| self.conflict_for(key))
            })
            .collect()
    }

    fn conflict_for(&self, key: &str) -> FactConflict {
        let entries = self.candidates(key);
        FactConflict {
            key: key.to_string(),
            fact_ids: entries.iter().map(|entry| entry.id.clone()).collect(),
            contents: entries.iter().map(|entry| entry.content.clone()).collect(),
            lineages: entries
                .iter()
                .map(|entry| entry.parent_lineage.clone())
                .collect(),
        }
    }
}

impl DerivedFact {
    /// Compose verified parent facts without discarding their quality
    /// metadata.  This is deliberately coarse: it propagates semantic
    /// classes, not numeric error bars.
    pub fn derive_from(
        id: impl Into<String>,
        content: impl Into<String>,
        parents: &[&DerivedFact],
        provenance: impl Into<String>,
        assumptions: &[String],
        domain: Option<String>,
    ) -> Result<Self, FactDerivationRejection> {
        let Some(first) = parents.first() else {
            return Err(FactDerivationRejection::NoParents);
        };
        let mut lineage = BTreeSet::new();
        let mut inherited_assumptions = BTreeSet::new();
        let mut proof_kind = first.proof_kind;
        let mut precision = first.precision;
        let mut common_domain = first.domain.clone();
        for parent in parents {
            lineage.insert(parent.id.clone());
            lineage.extend(parent.parent_lineage.iter().cloned());
            inherited_assumptions.extend(parent.assumptions.iter().cloned());
            proof_kind = proof_kind.compose(parent.proof_kind);
            precision = precision.compose(parent.precision);
            if common_domain != parent.domain {
                common_domain = None;
            }
        }
        inherited_assumptions.extend(assumptions.iter().cloned());
        Ok(Self {
            id: id.into(),
            content: content.into(),
            parent_lineage: lineage.into_iter().collect(),
            provenance: provenance.into(),
            proof_kind,
            precision,
            assumptions: inherited_assumptions.into_iter().collect(),
            domain: domain.or(common_domain),
        })
    }

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
    ProofKindNotAllowed,
    PrecisionNotAllowed,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct FactPolicy {
    pub allowed_statuses: Vec<EvidenceStatus>,
    pub require_lineage: bool,
    pub allowed_proof_kinds: Vec<DerivedProofKind>,
    pub allowed_precision: Vec<FactPrecision>,
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
            allowed_proof_kinds: vec![
                DerivedProofKind::ExactTransformation,
                DerivedProofKind::ApproximateTransformation,
                DerivedProofKind::Measurement,
            ],
            allowed_precision: vec![
                FactPrecision::Exact,
                FactPrecision::Approximate,
                FactPrecision::Measured,
            ],
        }
    }

    pub fn exact_transformation() -> Self {
        Self {
            allowed_statuses: vec![EvidenceStatus::Explicit, EvidenceStatus::Confirmed, EvidenceStatus::Inferred],
            require_lineage: true,
            allowed_proof_kinds: vec![DerivedProofKind::ExactTransformation],
            allowed_precision: vec![FactPrecision::Exact],
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
        if !self.allowed_proof_kinds.contains(&fact.proof_kind) {
            return Err(FactPolicyRejection::ProofKindNotAllowed);
        }
        if !self.allowed_precision.contains(&fact.precision) {
            return Err(FactPolicyRejection::PrecisionNotAllowed);
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
            proof_kind: DerivedProofKind::ExactTransformation,
            precision: FactPrecision::Exact,
            assumptions: Vec::new(),
            domain: None,
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
            proof_kind: DerivedProofKind::ExactTransformation,
            precision: FactPrecision::Exact,
            assumptions: Vec::new(),
            domain: None,
        };
        assert_eq!(
            FactPolicy::verified_transformation().evaluate(&fact, EvidenceStatus::Inferred),
            Err(FactPolicyRejection::LineageMissing)
        );
    }

    #[test]
    fn exact_fact_policy_rejects_approximate_and_measured_results() {
        let mut approximate = DerivedFact {
            id: "approx".into(),
            content: "pi = 3.14".into(),
            parent_lineage: vec!["numeric-evaluation".into()],
            provenance: "bounded approximation".into(),
            proof_kind: DerivedProofKind::ApproximateTransformation,
            precision: FactPrecision::Approximate,
            assumptions: Vec::new(),
            domain: None,
        };
        let policy = FactPolicy::exact_transformation();
        assert_eq!(
            policy.evaluate(&approximate, EvidenceStatus::Inferred),
            Err(FactPolicyRejection::ProofKindNotAllowed)
        );
        approximate.proof_kind = DerivedProofKind::Measurement;
        approximate.precision = FactPrecision::Measured;
        assert_eq!(
            policy.evaluate(&approximate, EvidenceStatus::Inferred),
            Err(FactPolicyRejection::ProofKindNotAllowed)
        );
    }

    #[test]
    fn fact_composition_propagates_quality_and_lineage() {
        let mut exact = derived_fact("exact", "mass = 2kg", "prompt-mass");
        exact.domain = Some("mechanics".into());
        let mut measured = derived_fact("measured", "velocity = 3m/s", "sensor-velocity");
        measured.proof_kind = DerivedProofKind::Measurement;
        measured.precision = FactPrecision::Measured;
        measured.assumptions = vec!["calibrated sensor".into()];
        measured.domain = Some("mechanics".into());
        let result = DerivedFact::derive_from(
            "momentum",
            "momentum = 6kg*m/s",
            &[&exact, &measured],
            "verified momentum transformation",
            &["parallel vectors".into()],
            None,
        )
        .unwrap();
        assert_eq!(result.proof_kind, DerivedProofKind::Measurement);
        assert_eq!(result.precision, FactPrecision::Measured);
        assert_eq!(result.domain.as_deref(), Some("mechanics"));
        assert_eq!(
            result.parent_lineage,
            vec!["exact", "measured", "prompt-mass", "sensor-velocity"]
        );
        assert_eq!(
            result.assumptions,
            vec!["calibrated sensor", "parallel vectors"]
        );
    }

    #[test]
    fn fact_composition_rejects_parentless_conclusions() {
        assert_eq!(
            DerivedFact::derive_from(
                "guess",
                "x = 42",
                &[],
                "unverified",
                &[],
                None,
            ),
            Err(FactDerivationRejection::NoParents)
        );
    }

    #[test]
    fn fact_selection_policy_filters_quality_for_exact_consumers() {
        let mut index = DerivedFactIndex::default();
        let broad = FactPolicy::verified_transformation();
        let exact = derived_fact("exact", "x = 5", "proof-exact");
        let mut approximate = derived_fact("approx", "x = 5.01", "proof-approx");
        approximate.proof_kind = DerivedProofKind::ApproximateTransformation;
        approximate.precision = FactPrecision::Approximate;
        index.insert("x", exact, &broad).unwrap();
        index.insert("x-approx", approximate, &broad).unwrap();

        let selected = index.select("x", &FactSelectionPolicy::exact_algebra()).unwrap();
        assert_eq!(selected.iter().map(|fact| fact.id.as_str()).collect::<Vec<_>>(), vec!["exact"]);
        assert!(matches!(
            index.select("x-approx", &FactSelectionPolicy::exact_algebra()),
            Err(FactSelectionFailure::NoAcceptableFacts { key, .. }) if key == "x-approx"
        ));
    }

    #[test]
    fn fact_selection_policy_never_selects_from_a_conflicted_key() {
        let mut index = DerivedFactIndex::default();
        let policy = FactPolicy::verified_transformation();
        index
            .insert("x", derived_fact("x1", "x = 5", "proof-a"), &policy)
            .unwrap();
        index
            .insert("x", derived_fact("x2", "x = 7", "proof-b"), &policy)
            .unwrap();
        assert!(matches!(
            index.select("x", &FactSelectionPolicy::exact_algebra()),
            Err(FactSelectionFailure::Conflict(conflict)) if conflict.key == "x"
        ));
    }

    #[test]
    fn fact_index_exposes_dependency_and_invalidation_queries() {
        let policy = FactPolicy::verified_transformation();
        let base = derived_fact("base", "time = 5s", "prompt-time");
        let intermediate = DerivedFact::derive_from(
            "distance",
            "distance = 50m",
            &[&base],
            "rate transformation",
            &[],
            Some("mechanics".into()),
        )
        .unwrap();
        let conclusion = DerivedFact::derive_from(
            "arrival",
            "arrival = true",
            &[&intermediate],
            "threshold check",
            &[],
            Some("mechanics".into()),
        )
        .unwrap();
        let mut index = DerivedFactIndex::default();
        index.insert("time", base, &policy).unwrap();
        index.insert("distance", intermediate, &policy).unwrap();
        index.insert("arrival", conclusion, &policy).unwrap();

        assert_eq!(
            index.ancestors_of("arrival"),
            vec!["base", "distance", "prompt-time"]
        );
        assert_eq!(index.dependents_of("base"), vec!["arrival", "distance"]);
        assert_eq!(index.invalidation_closure("base"), vec!["arrival", "distance"]);
        assert!(index.fact("missing").is_none());
    }

    #[test]
    fn fact_invalidation_is_non_destructive_and_blocks_selection() {
        let policy = FactPolicy::verified_transformation();
        let base = derived_fact("base", "time = 5s", "prompt-time");
        let derived = DerivedFact::derive_from(
            "distance",
            "distance = 50m",
            &[&base],
            "rate transformation",
            &[],
            Some("mechanics".into()),
        )
        .unwrap();
        let mut index = DerivedFactIndex::default();
        index.insert("time", base, &policy).unwrap();
        index.insert("distance", derived, &policy).unwrap();

        let affected = index
            .invalidate("base", "time corrected by user", Some("time-new".into()))
            .unwrap();
        assert_eq!(affected, vec!["base", "distance"]);
        assert_eq!(
            index.lifecycle("base").map(|state| state.status),
            Some(FactStatus::Invalidated)
        );
        assert_eq!(
            index.lifecycle("distance").map(|state| state.status),
            Some(FactStatus::Invalidated)
        );
        assert!(matches!(
            index.select("distance", &FactSelectionPolicy::exact_algebra()),
            Err(FactSelectionFailure::Unavailable { key, .. }) if key == "distance"
        ));
        assert!(index.fact("distance").is_some());
    }

    #[test]
    fn superseding_a_fact_preserves_history_and_invalidates_dependents() {
        let policy = FactPolicy::verified_transformation();
        let base = derived_fact("base", "x = 5", "prompt-x");
        let derived = DerivedFact::derive_from(
            "double",
            "2x = 10",
            &[&base],
            "doubling",
            &[],
            None,
        )
        .unwrap();
        let mut index = DerivedFactIndex::default();
        index.insert("x", base, &policy).unwrap();
        index.insert("double", derived, &policy).unwrap();
        let affected = index.supersede("base", "base-new", "clarification").unwrap();
        assert_eq!(affected, vec!["double"]);
        assert_eq!(index.lifecycle("base").unwrap().status, FactStatus::Superseded);
        assert_eq!(index.lifecycle("double").unwrap().status, FactStatus::Invalidated);
        assert_eq!(index.fact("base").unwrap().content, "x = 5");
    }

    fn derived_fact(id: &str, content: &str, parent: &str) -> DerivedFact {
        DerivedFact {
            id: id.into(),
            content: content.into(),
            parent_lineage: vec![parent.into()],
            provenance: "verified transformation".into(),
            proof_kind: DerivedProofKind::ExactTransformation,
            precision: FactPrecision::Exact,
            assumptions: Vec::new(),
            domain: None,
        }
    }

    #[test]
    fn derived_fact_index_validates_lineage_and_supports_relevance_queries() {
        let mut index = DerivedFactIndex::default();
        let policy = FactPolicy::verified_transformation();
        assert_eq!(
            index.insert("distance", derived_fact("d1", "distance = 50m", "rate-time"), &policy),
            Ok(FactIndexInsert::Added)
        );
        assert_eq!(index.candidates("mass"), &[]);
        let usable = index.usable("distance").unwrap();
        assert_eq!(usable.len(), 1);
        assert_eq!(usable[0].id, "d1");
    }

    #[test]
    fn derived_fact_index_rejects_unlineaged_facts() {
        let mut index = DerivedFactIndex::default();
        let policy = FactPolicy::verified_transformation();
        let fact = DerivedFact {
            id: "guess".into(),
            content: "answer = 42".into(),
            parent_lineage: Vec::new(),
            provenance: "guess".into(),
            proof_kind: DerivedProofKind::ExactTransformation,
            precision: FactPrecision::Exact,
            assumptions: Vec::new(),
            domain: None,
        };
        assert_eq!(
            index.insert("answer", fact, &policy),
            Err(FactIndexRejection::Policy(FactPolicyRejection::LineageMissing))
        );
    }

    #[test]
    fn derived_fact_index_surfaces_conflicting_valid_lineages() {
        let mut index = DerivedFactIndex::default();
        let policy = FactPolicy::verified_transformation();
        assert_eq!(
            index.insert("distance", derived_fact("d1", "distance = 50m", "proof-a"), &policy),
            Ok(FactIndexInsert::Added)
        );
        let result = index.insert(
            "distance",
            derived_fact("d2", "distance = 60m", "proof-b"),
            &policy,
        );
        assert!(matches!(result, Ok(FactIndexInsert::Conflict(_))));
        assert!(matches!(
            index.usable("distance"),
            Err(FactIndexQueryFailure::Conflict(conflict))
                if conflict.key == "distance" && conflict.fact_ids == vec!["d1", "d2"]
        ));
        assert_eq!(index.conflicts().len(), 1);
    }
}
