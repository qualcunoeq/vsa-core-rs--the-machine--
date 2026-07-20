//! Registry of bounded, independently verified execution capabilities.
//!
//! A capability is an auditable contract, not a routing hint.  A registry
//! entry describes the object/operation/result boundary; the executor still
//! performs its own detailed authorization and verification.

use crate::formalization::{AnswerForm, OperationKind, SubjectObjectType};
use serde::Serialize;
use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum InputRequirement {
    ExplicitFunctionDefinition,
    ExactlyOneArgumentBinding,
    ExplicitExpressionBody,
    ParseableExpressionSubject,
    AllExpressionVariablesBound,
    SingleEquationSubject,
    SingleTargetVariable,
    LinearRelation,
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
    /// Other enabled capabilities required by this capability's executor.
    /// Dependencies are capability IDs, not registry insertion positions.
    pub dependencies: Vec<String>,
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
    DependencyUnavailable(String),
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
            dependencies: vec!["expression_evaluation".into()],
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
            dependencies: Vec::new(),
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

    pub fn linear_equation_solve_v1() -> Self {
        Self {
            id: "linear_equation_solve".into(),
            version: 1,
            dependencies: Vec::new(),
            supported_object_types: vec![SubjectObjectType::Equation],
            supported_operations: vec![OperationKind::Solve],
            supported_answer_forms: vec![
                AnswerForm::ExactValue,
                AnswerForm::SolutionSet,
                AnswerForm::SingleSelectedSolution,
            ],
            input_requirements: vec![
                InputRequirement::SingleEquationSubject,
                InputRequirement::SingleTargetVariable,
                InputRequirement::LinearRelation,
                InputRequirement::ReplayVerifier,
            ],
            executor: "linear_equation::execute_linear_equation".into(),
            verifier: "linear_equation::replay_linear_equation".into(),
            regression_cases: vec![
                "linear_equation::unique_solution_executes_and_replays".into(),
                "linear_equation::fractional_solution_executes_and_replays".into(),
                "linear_equation::quadratic_is_denied".into(),
                "linear_equation::multiple_variables_are_denied".into(),
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
        registry.register(CapabilitySpec::linear_equation_solve_v1());
        registry
    }

    pub fn register(&mut self, capability: CapabilitySpec) {
        self.capabilities.insert(capability.id.clone(), capability);
    }

    pub fn get(&self, id: &str) -> Option<&CapabilitySpec> {
        self.capabilities.get(id)
    }

    /// Return a deterministic dependency-first order, rejecting missing
    /// dependencies and cycles before a planner can use the registry.
    pub fn dependency_order(&self) -> Result<Vec<String>, Vec<String>> {
        fn visit(
            id: &str,
            registry: &CapabilityRegistry,
            visiting: &mut BTreeSet<String>,
            visited: &mut BTreeSet<String>,
            order: &mut Vec<String>,
            errors: &mut Vec<String>,
        ) {
            if visited.contains(id) {
                return;
            }
            if !visiting.insert(id.to_string()) {
                errors.push(format!("dependency_cycle:{id}"));
                return;
            }
            let Some(capability) = registry.get(id) else {
                errors.push(format!("missing_capability:{id}"));
                visiting.remove(id);
                return;
            };
            for dependency in &capability.dependencies {
                visit(dependency, registry, visiting, visited, order, errors);
            }
            visiting.remove(id);
            visited.insert(id.to_string());
            order.push(id.to_string());
        }

        let mut visiting = BTreeSet::new();
        let mut visited = BTreeSet::new();
        let mut order = Vec::new();
        let mut errors = Vec::new();
        for id in self.capabilities.keys() {
            visit(id, self, &mut visiting, &mut visited, &mut order, &mut errors);
        }
        if errors.is_empty() {
            Ok(order)
        } else {
            errors.sort();
            errors.dedup();
            Err(errors)
        }
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
            for dependency in &capability.dependencies {
                match self.get(dependency) {
                    Some(spec) if spec.quality_gate.enabled() => {}
                    _ => rejections.push(CapabilityRejection::DependencyUnavailable(
                        dependency.clone(),
                    )),
                }
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
                    InputRequirement::SingleEquationSubject => {
                        subject.object_type == SubjectObjectType::Equation
                            && !subject.object.trim().is_empty()
                    }
                    InputRequirement::SingleTargetVariable => {
                        target.target_variable.is_some()
                            && target.target_variable.as_deref().unwrap_or_default().len() == 1
                    }
                    InputRequirement::LinearRelation => {
                        if subject.object_type != SubjectObjectType::Equation {
                            true
                        } else if let Some(variable) = target.target_variable.as_deref() {
                            crate::algebra_island::parse_problem(&format!(
                                "Solve for {variable}: {}",
                                subject.object
                            ))
                            .map(|problem| {
                                problem.operation
                                    == crate::algebra_island::AlgebraOperation::SolveLinearEquation
                            })
                            .unwrap_or(false)
                        } else {
                            false
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
        assert_eq!(function.dependencies, vec!["expression_evaluation"]);
        assert_eq!(
            registry.dependency_order().unwrap(),
            vec![
                "expression_evaluation".to_string(),
                "function_application".to_string(),
                "linear_equation_solve".to_string(),
            ]
        );
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

    #[test]
    fn linear_solver_is_selected_only_for_linear_equations() {
        let registry = CapabilityRegistry::production();
        let trace = crate::formalization::assess_prompt(
            "cap-linear-1",
            "Solve for x: 3*x+2=11.",
            "Math",
            false,
        );
        assert_eq!(
            registry.discover(&trace.target_completion.target).selection,
            CapabilitySelection::Unique("linear_equation_solve".into())
        );
        let quadratic = crate::formalization::assess_prompt(
            "cap-linear-2",
            "Solve for x: x^2=4.",
            "Math",
            false,
        );
        assert_eq!(
            registry.discover(&quadratic.target_completion.target).selection,
            CapabilitySelection::None
        );
    }

    #[test]
    fn dependency_order_rejects_cycles_and_missing_nodes() {
        let mut cyclic = CapabilityRegistry::default();
        let mut first = CapabilitySpec::expression_evaluation_v1();
        let mut second = CapabilitySpec::linear_equation_solve_v1();
        first.dependencies = vec![second.id.clone()];
        second.dependencies = vec![first.id.clone()];
        cyclic.register(first);
        cyclic.register(second);
        let errors = cyclic.dependency_order().unwrap_err();
        assert!(errors.iter().any(|error| error.contains("dependency_cycle")));

        let mut missing = CapabilityRegistry::default();
        let mut function = CapabilitySpec::function_application_v1();
        function.dependencies = vec!["missing".into()];
        missing.register(function);
        let errors = missing.dependency_order().unwrap_err();
        assert!(errors.iter().any(|error| error == "missing_capability:missing"));
    }
}
