//! Stable dominant-failure classification for evaluation reports.
//!
//! A task may have several blockers, but reports need one primary bucket so
//! repeated runs can be compared and the next engineering investment is
//! explicit. Detailed denial receipts remain available alongside this summary.

use crate::formalization::{
    AuthorizationDenialTrace, FormalizationTrace, OperationStatus,
};
use serde::Serialize;
use std::collections::BTreeMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum FailureClass {
    UnsupportedDomain,
    InputParsingFailure,
    FormalizationFailure,
    MissingAssumptions,
    WrongArtifactTyping,
    MethodNotFound,
    PlanningFailure,
    ExecutionFailure,
    VerificationFailure,
    RetrievalFailure,
    SafetyRejection,
    ResourceDepthLimit,
}

impl FailureClass {
    pub const ALL: [Self; 12] = [
        Self::UnsupportedDomain,
        Self::InputParsingFailure,
        Self::FormalizationFailure,
        Self::MissingAssumptions,
        Self::WrongArtifactTyping,
        Self::MethodNotFound,
        Self::PlanningFailure,
        Self::ExecutionFailure,
        Self::VerificationFailure,
        Self::RetrievalFailure,
        Self::SafetyRejection,
        Self::ResourceDepthLimit,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Self::UnsupportedDomain => "unsupported_domain",
            Self::InputParsingFailure => "input_parsing_failure",
            Self::FormalizationFailure => "formalization_failure",
            Self::MissingAssumptions => "missing_assumptions",
            Self::WrongArtifactTyping => "wrong_artifact_typing",
            Self::MethodNotFound => "method_not_found",
            Self::PlanningFailure => "planning_failure",
            Self::ExecutionFailure => "execution_failure",
            Self::VerificationFailure => "verification_failure",
            Self::RetrievalFailure => "retrieval_failure",
            Self::SafetyRejection => "safety_rejection",
            Self::ResourceDepthLimit => "resource_depth_limit",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct FailureRecord {
    pub case_id: String,
    pub class: FailureClass,
    pub blocker: String,
    pub evidence: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct FailureTaxonomyReport {
    pub total_cases: usize,
    pub failed_cases: usize,
    pub classified_failures: usize,
    pub unclassified_failures: usize,
    pub classification_coverage: f64,
    pub counts: BTreeMap<String, usize>,
    pub records: Vec<FailureRecord>,
}

impl Default for FailureTaxonomyReport {
    fn default() -> Self {
        let counts = FailureClass::ALL
            .into_iter()
            .map(|class| (class.label().to_string(), 0))
            .collect();
        Self {
            total_cases: 0,
            failed_cases: 0,
            classified_failures: 0,
            unclassified_failures: 0,
            classification_coverage: 1.0,
            counts,
            records: Vec::new(),
        }
    }
}

impl FailureTaxonomyReport {
    pub fn observe(
        &mut self,
        trace: &FormalizationTrace,
        denial: &AuthorizationDenialTrace,
        actual_authorized: bool,
    ) -> Option<FailureRecord> {
        self.total_cases += 1;
        let Some(record) = classify_formalization_failure(trace, denial, actual_authorized) else {
            return None;
        };
        self.failed_cases += 1;
        *self.counts.entry(record.class.label().to_string()).or_default() += 1;
        self.classified_failures += 1;
        self.classification_coverage = self.classified_failures as f64 / self.failed_cases as f64;
        self.records.push(record.clone());
        Some(record)
    }

    pub fn finalize(&mut self) {
        self.unclassified_failures = self.failed_cases.saturating_sub(self.classified_failures);
        self.classification_coverage = if self.failed_cases == 0 {
            1.0
        } else {
            self.classified_failures as f64 / self.failed_cases as f64
        };
    }
}

/// Classify only incorrect outcomes. Correct abstentions are not failures;
/// false authorization is always a safety failure even when another blocker
/// is present in the trace.
pub fn classify_formalization_failure(
    trace: &FormalizationTrace,
    denial: &AuthorizationDenialTrace,
    actual_authorized: bool,
) -> Option<FailureRecord> {
    if denial.gold_should_authorize == actual_authorized {
        return None;
    }
    let (class, blocker) = if actual_authorized && !denial.gold_should_authorize {
        (FailureClass::SafetyRejection, "false_authorization".to_string())
    } else {
        classify_denial(trace, denial)
    };
    Some(FailureRecord {
        case_id: denial.case_id.clone(),
        class,
        blocker,
        evidence: denial.all_blockers.clone(),
    })
}

fn classify_denial(
    trace: &FormalizationTrace,
    denial: &AuthorizationDenialTrace,
) -> (FailureClass, String) {
    let blocker = denial.first_blocker.clone();
    let class = match &trace.target_completion.target.operation_status {
        OperationStatus::NotIdentified | OperationStatus::Ambiguous(_) => {
            FailureClass::InputParsingFailure
        }
        OperationStatus::Unsupported(_) => FailureClass::MethodNotFound,
        OperationStatus::Recognized(_) => match blocker.as_str() {
            "bindings_incomplete" | "constraints_incomplete" => {
                FailureClass::MissingAssumptions
            }
            "operation_unsupported" => FailureClass::MethodNotFound,
            "verification_unavailable" => FailureClass::VerificationFailure,
            "target_incomplete"
                if trace.target_completion.complete
                    && trace.target_completion.build_trace.final_status
                        == crate::formalization::TargetStatus::Complete =>
            {
                // The typed target is complete; the report's direct-audit
                // path declined because a multi-step method is required.
                FailureClass::PlanningFailure
            }
            "representation_incomplete" | "target_incomplete" => {
                FailureClass::FormalizationFailure
            }
            "authorization_contract_or_lower_bound" => FailureClass::SafetyRejection,
            _ if denial
                .all_blockers
                .iter()
                .any(|item| item.contains("artifact") || item.contains("type")) =>
            {
                FailureClass::WrongArtifactTyping
            }
            _ => FailureClass::PlanningFailure,
        },
    };
    (class, blocker)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::formalization::assess_prompt;

    #[test]
    fn correct_abstention_is_not_reported_as_failure() {
        let trace = assess_prompt("unsupported", "Prove a theorem about topology", "Math", false);
        let assessment = crate::formalization::assess_direct_instantiation(&trace);
        let receipt = assessment.denial_trace(false);
        assert!(classify_formalization_failure(&trace, &receipt, false).is_none());
    }

    #[test]
    fn false_denial_gets_one_stable_dominant_bucket() {
        let trace = assess_prompt("missing-target", "Evaluate x when x equals 5", "Math", false);
        let assessment = crate::formalization::assess_direct_instantiation(&trace);
        let receipt = assessment.denial_trace(true);
        let record = classify_formalization_failure(&trace, &receipt, false).unwrap();
        assert_eq!(record.class, FailureClass::PlanningFailure);
        assert!(!record.blocker.is_empty());
    }

    #[test]
    fn complete_typed_target_is_planning_failure_when_direct_audit_abstains() {
        let trace = assess_prompt("complete-target", "Given x + 4 = 9, solve for x.", "Math", false);
        assert!(trace.target_completion.complete);
        let assessment = crate::formalization::assess_direct_instantiation(&trace);
        let receipt = assessment.denial_trace(true);
        let record = classify_formalization_failure(&trace, &receipt, false).unwrap();
        assert_eq!(record.class, FailureClass::PlanningFailure);
    }

    #[test]
    fn false_authorization_is_always_safety_rejection() {
        let trace = assess_prompt("unsafe", "Prove a theorem about topology", "Math", false);
        let assessment = crate::formalization::assess_direct_instantiation(&trace);
        let receipt = assessment.denial_trace(false);
        let record = classify_formalization_failure(&trace, &receipt, true).unwrap();
        assert_eq!(record.class, FailureClass::SafetyRejection);
        assert_eq!(record.blocker, "false_authorization");
    }

    #[test]
    fn report_keeps_all_buckets_and_tracks_coverage() {
        let mut report = FailureTaxonomyReport::default();
        let trace = assess_prompt("missing-target", "Evaluate x when x equals 5", "Math", false);
        let assessment = crate::formalization::assess_direct_instantiation(&trace);
        report.observe(&trace, &assessment.denial_trace(true), false);
        report.finalize();
        assert_eq!(report.failed_cases, 1);
        assert_eq!(report.classified_failures, 1);
        assert_eq!(report.unclassified_failures, 0);
        assert_eq!(report.classification_coverage, 1.0);
        assert_eq!(report.counts.len(), 12);
    }
}
