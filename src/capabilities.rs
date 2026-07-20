//! Registry of bounded, independently verified execution capabilities.
//!
//! A capability is an auditable contract, not a routing hint.  A registry
//! entry describes the object/operation/result boundary; the executor still
//! performs its own detailed authorization and verification.

use crate::formalization::{AnswerForm, OperationKind, SubjectObjectType};
use serde::Serialize;
use std::collections::BTreeMap;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct CapabilitySpec {
    pub id: String,
    pub version: u32,
    pub supported_object_types: Vec<SubjectObjectType>,
    pub supported_operations: Vec<OperationKind>,
    pub supported_answer_forms: Vec<AnswerForm>,
    pub executor: String,
    pub verifier: String,
    pub regression_cases: Vec<String>,
}

impl CapabilitySpec {
    pub fn function_application_v1() -> Self {
        Self {
            id: "function_application".into(),
            version: 1,
            supported_object_types: vec![SubjectObjectType::Function],
            supported_operations: vec![OperationKind::Evaluate],
            supported_answer_forms: vec![AnswerForm::ExactValue, AnswerForm::SimplifiedExpression],
            executor: "function_application::execute_function_application".into(),
            verifier: "function_application::replay_substitution".into(),
            regression_cases: vec![
                "function_application::explicit_function_application_executes_and_replays".into(),
                "function_application::undefined_function_is_denied".into(),
                "function_application::piecewise_like_definition_is_denied".into(),
            ],
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
    }
}
