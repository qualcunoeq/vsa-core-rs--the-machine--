//! Diagnostic mining for selecting a *real* mathematical method pack.
//!
//! This module deliberately does not retrieve, instantiate, or execute a
//! theorem.  It aggregates human/trace annotations by method *shape* so that
//! a pack is selected from benchmark evidence rather than from a textbook
//! checklist.  The resulting report is safe to run over the whole benchmark:
//! it can rank candidates, but it cannot authorize an answer.

use crate::math_methods::{MathDomain, TaskShape};
use serde::Serialize;
use std::collections::BTreeMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum MethodShape {
    DefinitionInstantiation,
    DirectTheoremInstantiation,
    AlgebraicIdentityApplication,
    FiniteCaseReduction,
    RecurrenceUnrolling,
    InvariantApplication,
    BoundApplication,
    TransformAndEvaluate,
    ClassificationLookup,
    ConstructiveSearch,
    ProofByContradiction,
}

impl MethodShape {
    pub fn label(self) -> &'static str {
        match self {
            Self::DefinitionInstantiation => "definition_instantiation",
            Self::DirectTheoremInstantiation => "direct_theorem_instantiation",
            Self::AlgebraicIdentityApplication => "algebraic_identity_application",
            Self::FiniteCaseReduction => "finite_case_reduction",
            Self::RecurrenceUnrolling => "recurrence_unrolling",
            Self::InvariantApplication => "invariant_application",
            Self::BoundApplication => "bound_application",
            Self::TransformAndEvaluate => "transform_and_evaluate",
            Self::ClassificationLookup => "classification_lookup",
            Self::ConstructiveSearch => "constructive_search",
            Self::ProofByContradiction => "proof_by_contradiction",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum VerificationAvailability {
    None,
    Replay,
    IndependentIdentity,
    ExecutableTest,
    AuthoritativeSource,
}

impl VerificationAvailability {
    fn rank(self) -> u8 {
        match self {
            Self::None => 0,
            Self::AuthoritativeSource => 1,
            Self::Replay => 2,
            Self::IndependentIdentity => 3,
            Self::ExecutableTest => 4,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::None => "none",
            Self::Replay => "replay",
            Self::IndependentIdentity => "independent_identity",
            Self::ExecutableTest => "executable_test",
            Self::AuthoritativeSource => "authoritative_source",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RepresentationCost {
    Low,
    Medium,
    High,
    VeryHigh,
}

impl RepresentationCost {
    fn rank(self) -> u8 {
        match self {
            Self::Low => 0,
            Self::Medium => 1,
            Self::High => 2,
            Self::VeryHigh => 3,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Low => "low",
            Self::Medium => "medium",
            Self::High => "high",
            Self::VeryHigh => "very_high",
        }
    }
}

/// One manually reviewed or trace-derived annotation.  The fields are kept
/// local to the question; a global confidence score would hide which premise
/// or side condition is actually missing.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct MethodClusterAnnotation {
    pub question_id: String,
    pub domain: MathDomain,
    pub task_shape: TaskShape,
    pub required_method_shape: MethodShape,
    pub named_methods: Vec<String>,
    pub premises_explicit: bool,
    pub definitions_explicit: bool,
    pub side_conditions_extractable: bool,
    pub verifier_available: VerificationAvailability,
    pub estimated_steps: usize,
    pub representation_cost: RepresentationCost,
    /// True only when the question's formal object and target are sufficiently
    /// specified for a typed method schema to be authored.  This is diagnostic
    /// evidence, never an execution permission.
    pub structurally_compatible: bool,
}

impl MethodClusterAnnotation {
    pub fn eligible(&self, minimum_verification: VerificationAvailability) -> bool {
        self.structurally_compatible
            && self.premises_explicit
            && self.definitions_explicit
            && self.side_conditions_extractable
            && self.verifier_available.rank() >= minimum_verification.rank()
            && self.estimated_steps == 1
            && self.representation_cost <= RepresentationCost::Medium
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize)]
pub struct MethodClusterKey {
    pub domain: MathDomain,
    pub task_shape: TaskShape,
    pub method_shape: MethodShape,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct MethodClusterSummary {
    pub key: MethodClusterKey,
    pub question_ids: Vec<String>,
    pub total_questions: usize,
    pub structurally_compatible: usize,
    pub premises_explicit: usize,
    pub definitions_explicit: usize,
    pub side_conditions_extractable: usize,
    pub verifier_available: usize,
    pub one_step: usize,
    pub eligible: usize,
    pub named_methods: Vec<String>,
    /// Sum is retained instead of a floating-point mean so reports remain
    /// exactly comparable and serializable.  Callers can divide by
    /// `total_questions` for presentation.
    pub estimated_steps_total: usize,
    pub maximum_representation_cost: RepresentationCost,
    pub best_verification: VerificationAvailability,
}

impl MethodClusterSummary {
    pub fn eligible_for_pack(
        &self,
        minimum_questions: usize,
        minimum_verification: VerificationAvailability,
    ) -> bool {
        self.eligible >= minimum_questions
            && self.best_verification.rank() >= minimum_verification.rank()
    }

    pub fn mean_estimated_steps(&self) -> f64 {
        if self.total_questions == 0 {
            0.0
        } else {
            self.estimated_steps_total as f64 / self.total_questions as f64
        }
    }

    fn sort_key(&self) -> (usize, usize, usize, usize, usize, usize, u8, u8) {
        // Higher evidence dominates.  Fewer steps/cost are preferred only
        // after evidence and verification, so an easy but unsupported family
        // cannot outrank a slightly harder, well-grounded one.
        (
            self.eligible,
            self.structurally_compatible,
            self.premises_explicit,
            self.definitions_explicit,
            self.side_conditions_extractable,
            self.verifier_available,
            self.best_verification.rank(),
            u8::MAX - self.maximum_representation_cost.rank(),
        )
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct MethodClusterReport {
    pub annotations: usize,
    pub clusters: Vec<MethodClusterSummary>,
}

impl MethodClusterReport {
    pub fn from_annotations(annotations: &[MethodClusterAnnotation]) -> Self {
        #[derive(Default)]
        struct Accumulator {
            ids: Vec<String>,
            compatible: usize,
            premises: usize,
            definitions: usize,
            side_conditions: usize,
            verifiers: usize,
            one_step: usize,
            eligible: usize,
            named_methods: Vec<String>,
            step_total: usize,
            maximum_cost: Option<RepresentationCost>,
            best_verification: Option<VerificationAvailability>,
        }

        let mut grouped: BTreeMap<MethodClusterKey, Accumulator> = BTreeMap::new();
        for annotation in annotations {
            let key = MethodClusterKey {
                domain: annotation.domain,
                task_shape: annotation.task_shape,
                method_shape: annotation.required_method_shape,
            };
            let entry = grouped.entry(key).or_default();
            entry.ids.push(annotation.question_id.clone());
            entry.compatible += usize::from(annotation.structurally_compatible);
            entry.premises += usize::from(annotation.premises_explicit);
            entry.definitions += usize::from(annotation.definitions_explicit);
            entry.side_conditions += usize::from(annotation.side_conditions_extractable);
            entry.verifiers +=
                usize::from(annotation.verifier_available != VerificationAvailability::None);
            entry.one_step += usize::from(annotation.estimated_steps == 1);
            entry.eligible += usize::from(annotation.eligible(VerificationAvailability::Replay));
            entry.step_total += annotation.estimated_steps;
            entry.maximum_cost = Some(
                entry
                    .maximum_cost
                    .map_or(annotation.representation_cost, |current| {
                        current.max(annotation.representation_cost)
                    }),
            );
            entry.best_verification = Some(
                entry
                    .best_verification
                    .map_or(annotation.verifier_available, |current| {
                        current.max(annotation.verifier_available)
                    }),
            );
            for method in &annotation.named_methods {
                if !entry.named_methods.contains(method) {
                    entry.named_methods.push(method.clone());
                }
            }
        }

        let mut clusters: Vec<_> = grouped
            .into_iter()
            .map(|(key, mut acc)| {
                acc.ids.sort();
                acc.named_methods.sort();
                let total = acc.ids.len();
                MethodClusterSummary {
                    key,
                    question_ids: acc.ids,
                    total_questions: total,
                    structurally_compatible: acc.compatible,
                    premises_explicit: acc.premises,
                    definitions_explicit: acc.definitions,
                    side_conditions_extractable: acc.side_conditions,
                    verifier_available: acc.verifiers,
                    one_step: acc.one_step,
                    eligible: acc.eligible,
                    named_methods: acc.named_methods,
                    estimated_steps_total: acc.step_total,
                    maximum_representation_cost: acc
                        .maximum_cost
                        .unwrap_or(RepresentationCost::VeryHigh),
                    best_verification: acc
                        .best_verification
                        .unwrap_or(VerificationAvailability::None),
                }
            })
            .collect();
        clusters.sort_by(|a, b| {
            b.sort_key()
                .cmp(&a.sort_key())
                .then_with(|| a.key.cmp(&b.key))
        });
        Self {
            annotations: annotations.len(),
            clusters,
        }
    }

    pub fn pack_candidates(
        &self,
        minimum_questions: usize,
        minimum_verification: VerificationAvailability,
    ) -> Vec<&MethodClusterSummary> {
        self.clusters
            .iter()
            .filter(|cluster| cluster.eligible_for_pack(minimum_questions, minimum_verification))
            .collect()
    }

    /// Stable, reviewable output suitable for committing alongside a pack
    /// proposal.  This is intentionally a report, not a registry loader.
    pub fn to_markdown(&self) -> String {
        let mut out = format!(
            "# Mathematical method-shape mining report\n\nAnnotations: {}\n\n",
            self.annotations
        );
        out.push_str("| Rank | Domain | Task shape | Method shape | Questions | Eligible | Best verification | Cost | Methods |\n|---:|---|---|---|---:|---:|---|---|---|\n");
        for (index, cluster) in self.clusters.iter().enumerate() {
            let methods = if cluster.named_methods.is_empty() {
                "—".to_string()
            } else {
                cluster.named_methods.join(", ")
            };
            out.push_str(&format!(
                "| {} | {:?} | {:?} | {} | {} | {} | {} | {} | {} |\n",
                index + 1,
                cluster.key.domain,
                cluster.key.task_shape,
                cluster.key.method_shape.label(),
                cluster.total_questions,
                cluster.eligible,
                cluster.best_verification.label(),
                cluster.maximum_representation_cost.label(),
                methods
            ));
        }
        out
    }
}

/// A deliberately empty default.  Until annotations identify a qualifying
/// cluster, no theorem/method pack is enabled by the runtime.
pub fn empty_method_cluster_report() -> MethodClusterReport {
    MethodClusterReport::from_annotations(&[])
}

#[cfg(test)]
mod tests {
    use super::*;

    fn annotation(
        id: &str,
        shape: MethodShape,
        compatible: bool,
        verifier: VerificationAvailability,
    ) -> MethodClusterAnnotation {
        MethodClusterAnnotation {
            question_id: id.to_string(),
            domain: MathDomain::Algebra,
            task_shape: TaskShape::ComputeExplicitValue,
            required_method_shape: shape,
            named_methods: vec!["demo.method".to_string()],
            premises_explicit: compatible,
            definitions_explicit: compatible,
            side_conditions_extractable: compatible,
            verifier_available: verifier,
            estimated_steps: 1,
            representation_cost: RepresentationCost::Low,
            structurally_compatible: compatible,
        }
    }

    #[test]
    fn groups_and_counts_annotations_without_authorizing_execution() {
        let annotations = vec![
            annotation(
                "q2",
                MethodShape::DefinitionInstantiation,
                true,
                VerificationAvailability::Replay,
            ),
            annotation(
                "q1",
                MethodShape::DefinitionInstantiation,
                true,
                VerificationAvailability::Replay,
            ),
            annotation(
                "q3",
                MethodShape::DirectTheoremInstantiation,
                false,
                VerificationAvailability::None,
            ),
        ];
        let report = MethodClusterReport::from_annotations(&annotations);
        assert_eq!(report.annotations, 3);
        assert_eq!(report.clusters.len(), 2);
        let first = &report.clusters[0];
        assert_eq!(first.key.method_shape, MethodShape::DefinitionInstantiation);
        assert_eq!(first.question_ids, vec!["q1", "q2"]);
        assert_eq!(first.eligible, 2);
        assert_eq!(
            report
                .pack_candidates(3, VerificationAvailability::Replay)
                .len(),
            0
        );
    }

    #[test]
    fn ranking_prefers_evidence_then_verification_and_is_stable() {
        let annotations = vec![
            annotation(
                "weak",
                MethodShape::DirectTheoremInstantiation,
                true,
                VerificationAvailability::Replay,
            ),
            annotation(
                "strong",
                MethodShape::DefinitionInstantiation,
                true,
                VerificationAvailability::ExecutableTest,
            ),
            annotation(
                "strong2",
                MethodShape::DefinitionInstantiation,
                true,
                VerificationAvailability::ExecutableTest,
            ),
        ];
        let report = MethodClusterReport::from_annotations(&annotations);
        assert_eq!(
            report.clusters[0].key.method_shape,
            MethodShape::DefinitionInstantiation
        );
        assert!(report.clusters[0].eligible_for_pack(2, VerificationAvailability::Replay));
        assert!(!report.clusters[1].eligible_for_pack(2, VerificationAvailability::Replay));
        let markdown = report.to_markdown();
        assert!(markdown.contains("definition_instantiation"));
        assert!(markdown.contains("executable_test"));
    }

    #[test]
    fn eligibility_rejects_multi_step_or_high_cost_questions() {
        let mut item = annotation(
            "q",
            MethodShape::InvariantApplication,
            true,
            VerificationAvailability::Replay,
        );
        item.estimated_steps = 2;
        assert!(!item.eligible(VerificationAvailability::Replay));
        item.estimated_steps = 1;
        item.representation_cost = RepresentationCost::High;
        assert!(!item.eligible(VerificationAvailability::Replay));
        item.representation_cost = RepresentationCost::Low;
        item.verifier_available = VerificationAvailability::None;
        assert!(!item.eligible(VerificationAvailability::Replay));
    }
}
