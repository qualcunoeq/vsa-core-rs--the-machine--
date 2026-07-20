//! Registry of bounded, independently verified execution capabilities.
//!
//! A capability is an auditable contract, not a routing hint.  A registry
//! entry describes the object/operation/result boundary; the executor still
//! performs its own detailed authorization and verification.

use crate::formalization::{AnswerForm, OperationKind, SubjectObjectType};
use serde::Serialize;
use std::collections::BTreeMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum InputRequirement {
    ExplicitFunctionDefinition,
    ExactlyOneArgumentBinding,
    ExplicitExpressionBody,
    ParseableExpressionSubject,
    AllExpressionVariablesBound,
    NoFreeVariables,
    ReplayVerifier,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct CapabilityQualityGate {
    pub positive_cases: usize,
    pub negative_cases: usize,
    pub adversarial_cases: usize,
    pub false_authorizations: usize,
    pub replay_failures: usize,
}

impl CapabilityQualityGate {
    pub fn enabled(&self) -> bool {
        self.positive_cases > 0
            && self.negative_cases > 0
            && self.adversarial_cases > 0
            && self.false_authorizations == 0
            && self.replay_failures == 0
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct CapabilitySpec {
    pub id: String,
    pub version: u32,
    pub supported_object_types: Vec<SubjectObjectType>,
    pub supported_operations: Vec<OperationKind>,
    pub supported_answer_forms: Vec<AnswerForm>,
    pub input_requirements: Vec<InputRequirement>,
    pub executor: String,
    pub verifier: String,
    pub regression_cases: Vec<String>,
    pub quality_gate: CapabilityQualityGate,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub enum CapabilityRejection {
    QualityGateFailed,
    MissingSubject,
    ObjectTypeMismatch,
    OperationMismatch,
    AnswerFormMismatch,
    InputRequirementMissing(InputRequirement),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct CapabilityCandidate {
    pub id: String,
    pub eligible: bool,
    pub rejections: Vec<CapabilityRejection>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub enum CapabilitySelection {
    Unique(String),
    Ambiguous(Vec<String>),
    None,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct CapabilityDiscoveryTrace {
    pub candidates: Vec<CapabilityCandidate>,
    pub selection: CapabilitySelection,
}

impl CapabilitySpec {
    pub fn function_application_v1() -> Self {
        Self {
            id: "function_application".into(),
            version: 1,
            supported_object_types: vec![SubjectObjectType::Function],
            supported_operations: vec![OperationKind::Evaluate],
            supported_answer_forms: vec![AnswerForm::ExactValue, AnswerForm::SimplifiedExpression],
            input_requirements: vec![
                InputRequirement::ExplicitFunctionDefinition,
                InputRequirement::ExactlyOneArgumentBinding,
                InputRequirement::ExplicitExpressionBody,
                InputRequirement::ReplayVerifier,
            ],
            executor: "function_application::execute_function_application".into(),
            verifier: "function_application::replay_substitution".into(),
            regression_cases: vec![
                "function_application::explicit_function_application_executes_and_replays".into(),
                "function_application::undefined_function_is_denied".into(),
                "function_application::piecewise_like_definition_is_denied".into(),
            ],
            quality_gate: CapabilityQualityGate {
                positive_cases: 1,
                negative_cases: 2,
                adversarial_cases: 1,
                false_authorizations: 0,
                replay_failures: 0,
            },
        }
    }

    pub fn expression_evaluation_v1() -> Self {
        Self {
            id: "expression_evaluation".into(),
            version: 1,
            supported_object_types: vec![SubjectObjectType::Expression],
            supported_operations: vec![OperationKind::Evaluate],
            supported_answer_forms: vec![AnswerForm::ExactValue],
            input_requirements: vec![
                InputRequirement::ParseableExpressionSubject,
                InputRequirement::AllExpressionVariablesBound,
                InputRequirement::ReplayVerifier,
            ],
            executor: "expression_evaluation::execute_expression_evaluation".into(),
            verifier: "expression_evaluation::replay_expression_evaluation".into(),
            regression_cases: vec![
                "expression_evaluation::numeric_expression_executes_and_replays".into(),
                "expression_evaluation::bound_expression_executes_and_replays".into(),
                "expression_evaluation::unbound_expression_is_denied".into(),
                "expression_evaluation::unsupported_expression_is_denied".into(),
            ],
            quality_gate: CapabilityQualityGate {
                positive_cases: 2,
                negative_cases: 1,
                adversarial_cases: 1,
                false_authorizations: 0,
                replay_failures: 0,
            },
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Default)]
pub struct CapabilityRegistry {
    pub capabilities: BTreeMap<String, CapabilitySpec>,
}

impl CapabilityRegistry {
    pub fn production() -> Self {
        let mut registry = Self::default();
        registry.register(CapabilitySpec::function_application_v1());
        registry.register(CapabilitySpec::expression_evaluation_v1());
        registry
    }

    pub fn register(&mut self, capability: CapabilitySpec) {
        self.capabilities.insert(capability.id.clone(), capability);
    }

    pub fn get(&self, id: &str) -> Option<&CapabilitySpec> {
        self.capabilities.get(id)
    }

    pub fn accepts(
        &self,
        id: &str,
        object_type: SubjectObjectType,
        operation: OperationKind,
        answer_form: Option<AnswerForm>,
    ) -> bool {
        let Some(capability) = self.get(id) else {
            return false;
        };
        capability.supported_object_types.contains(&object_type)
            && capability.supported_operations.contains(&operation)
            && answer_form
                .map(|form| capability.supported_answer_forms.contains(&form))
                .unwrap_or(false)
    }

    pub fn discover(
        &self,
        target: &crate::formalization::FormalizedTarget,
    ) -> CapabilityDiscoveryTrace {
        let mut candidates = Vec::new();
        for capability in self.capabilities.values() {
            let mut rejections = Vec::new();
            if !capability.quality_gate.enabled() {
                rejections.push(CapabilityRejection::QualityGateFailed);
            }
            let Some(subject) = target.subject_resolution.selected.as_ref() else {
                candidates.push(CapabilityCandidate {
                    id: capability.id.clone(),
                    eligible: false,
                    rejections: vec![CapabilityRejection::MissingSubject],
                });
                continue;
            };
            if !capability
                .supported_object_types
                .contains(&subject.object_type)
            {
                rejections.push(CapabilityRejection::ObjectTypeMismatch);
            }
            if !capability.supported_operations.contains(&target.operation) {
                rejections.push(CapabilityRejection::OperationMismatch);
            }
            if !target
                .answer_form
                .map(|form| capability.supported_answer_forms.contains(&form))
                .unwrap_or(false)
            {
                rejections.push(CapabilityRejection::AnswerFormMismatch);
            }
            for requirement in &capability.input_requirements {
                let satisfied = match requirement {
                    InputRequirement::ExplicitFunctionDefinition => {
                        subject.object_type == SubjectObjectType::Function
                            && subject.definition_available
                    }
                    InputRequirement::ExactlyOneArgumentBinding => {
                        target.arguments.len() == 1
                            && target.arguments[0].status
                                == crate::formalization::TargetFieldStatus::Complete
                    }
                    InputRequirement::ExplicitExpressionBody => subject.object.contains('='),
                    InputRequirement::ParseableExpressionSubject => {
                        subject.object.parse::<f64>().is_ok()
                            || crate::algebra::parse(subject.object.trim()).is_ok()
                    }
                    InputRequirement::AllExpressionVariablesBound => {
                        if subject.object_type != SubjectObjectType::Expression {
                            true
                        } else {
                            target.arguments.iter().all(|binding| {
                                binding.status
                                    == crate::formalization::TargetFieldStatus::Complete
                            }) || !subject.object.chars().any(|c| c.is_ascii_alphabetic())
                        }
                    }
                    InputRequirement::NoFreeVariables | InputRequirement::ReplayVerifier => true,
                };
                if !satisfied {
                    rejections.push(CapabilityRejection::InputRequirementMissing(*requirement));
                }
            }
            candidates.push(CapabilityCandidate {
                id: capability.id.clone(),
                eligible: rejections.is_empty(),
                rejections,
            });
        }
        let eligible: Vec<_> = candidates
            .iter()
            .filter(|candidate| candidate.eligible)
            .map(|candidate| candidate.id.clone())
            .collect();
        let selection = match eligible.as_slice() {
            [single] => CapabilitySelection::Unique(single.clone()),
            [] => CapabilitySelection::None,
            many => CapabilitySelection::Ambiguous(many.to_vec()),
        };
        CapabilityDiscoveryTrace {
            candidates,
            selection,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn production_registry_contains_only_verified_function_v1() {
        let registry = CapabilityRegistry::production();
        let function = registry.get("function_application").unwrap();
        assert_eq!(function.version, 1);
        assert!(registry.accepts(
            "function_application",
            SubjectObjectType::Function,
            OperationKind::Evaluate,
            Some(AnswerForm::ExactValue)
        ));
        assert!(!registry.accepts(
            "function_application",
            SubjectObjectType::Function,
            OperationKind::Solve,
            Some(AnswerForm::ExactValue)
        ));
        assert!(!registry.accepts(
            "function_application",
            SubjectObjectType::Function,
            OperationKind::Evaluate,
            Some(AnswerForm::Proof)
        ));
        let trace = registry.discover(
            &crate::formalization::assess_prompt(
                "cap-1",
                "Let f(x)=x+1. What is f(2)?",
                "Math",
                false,
            )
            .target_completion
            .target,
        );
        assert_eq!(
            trace.selection,
            CapabilitySelection::Unique("function_application".into())
        );
    }

    #[test]
    fn expression_evaluation_is_selected_for_expression_targets() {
        let registry = CapabilityRegistry::production();
        let trace = crate::formalization::assess_prompt(
            "cap-expression-1",
            "Evaluate 2*x+3 at x=4.",
            "Math",
            false,
        );
        let discovery = registry.discover(&trace.target_completion.target);
        assert_eq!(
            discovery.selection,
            CapabilitySelection::Unique("expression_evaluation".into())
        );
        let expression = discovery
            .candidates
            .iter()
            .find(|candidate| candidate.id == "expression_evaluation")
            .unwrap();
        assert!(expression.eligible);
    }
}
