//! Narrow, verified function-application execution island.
//!
//! This module intentionally accepts only explicit single-argument function
//! definitions and concrete arguments.  It does not infer missing functions,
//! piecewise branches, recursive definitions, or symbolic parameters.

use crate::algebra::{self, SymExpr};
use crate::formalization::{AnswerForm, FormalizedTarget, SubjectObjectType, TargetFieldStatus};
use crate::math_ingest::substitute_vars;
use regex::Regex;
use serde::Serialize;
use std::collections::HashMap;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct FunctionDefinition {
    pub function_id: String,
    pub parameter: String,
    pub body_source: String,
    pub definition_source: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub enum FunctionApplicationFailure {
    OperationNotEvaluate,
    SubjectNotFunction,
    DefinitionUnavailable,
    MultipleDefinitions,
    FunctionApplicationReferenceMissing,
    ArgumentMissing,
    ArgumentAmbiguous,
    UnsupportedArgument,
    UnsupportedDefinition,
    ExpressionParseFailed(String),
    EvaluationFailed,
    ReplayVerificationFailed,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct FunctionExecutionReceipt {
    pub function_id: String,
    pub definition_source: String,
    pub parameter: String,
    pub argument_source: String,
    pub instantiated_expression: SymExpr,
    pub numeric_result: f64,
    pub replay_verified: bool,
}

pub fn parse_function_definition(
    source: &str,
) -> Result<FunctionDefinition, FunctionApplicationFailure> {
    let regex = Regex::new(
        r"(?i)^\s*([A-Za-z_][A-Za-z0-9_]*)\s*\(\s*([A-Za-z_][A-Za-z0-9_]*)\s*\)\s*=\s*(.*?)\s*[.;]?\s*$",
    )
    .expect("static function definition regex");
    let captures = regex
        .captures(source)
        .ok_or(FunctionApplicationFailure::UnsupportedDefinition)?;
    let function_id = captures.get(1).unwrap().as_str().to_string();
    let parameter = captures.get(2).unwrap().as_str().to_string();
    let body_source = captures.get(3).unwrap().as_str().trim().to_string();
    if body_source.is_empty() || body_source.contains('{') || body_source.contains('}') {
        return Err(FunctionApplicationFailure::UnsupportedDefinition);
    }
    Ok(FunctionDefinition {
        function_id,
        parameter,
        body_source,
        definition_source: source.trim().to_string(),
    })
}

fn target_function_definition(
    target: &FormalizedTarget,
) -> Result<FunctionDefinition, FunctionApplicationFailure> {
    let selected = target
        .subject_resolution
        .selected
        .as_ref()
        .ok_or(FunctionApplicationFailure::DefinitionUnavailable)?;
    if selected.object_type != SubjectObjectType::Function {
        return Err(FunctionApplicationFailure::SubjectNotFunction);
    }
    parse_function_definition(&selected.object)
}

fn concrete_argument(
    target: &FormalizedTarget,
) -> Result<(&str, &str), FunctionApplicationFailure> {
    if target.arguments.len() != 1 {
        return Err(FunctionApplicationFailure::ArgumentMissing);
    }
    let binding = &target.arguments[0];
    if binding.status != TargetFieldStatus::Complete {
        return Err(FunctionApplicationFailure::ArgumentAmbiguous);
    }
    if binding.value.contains(',') || binding.value.trim().is_empty() {
        return Err(FunctionApplicationFailure::UnsupportedArgument);
    }
    Ok((binding.parameter.as_str(), binding.value.as_str()))
}

/// Authorizes the narrow function island without performing execution.
pub fn authorize_function_application(
    target: &FormalizedTarget,
) -> Result<FunctionDefinition, FunctionApplicationFailure> {
    if target.operation != crate::formalization::OperationKind::Evaluate {
        return Err(FunctionApplicationFailure::OperationNotEvaluate);
    }
    let definition = target_function_definition(target)?;
    if !target
        .reference_graph
        .references
        .iter()
        .any(|edge| edge.relation == crate::formalization::ReferenceRelation::FunctionApplication)
    {
        return Err(FunctionApplicationFailure::FunctionApplicationReferenceMissing);
    }
    let answer_ok = matches!(
        target.answer_form,
        Some(AnswerForm::ExactValue | AnswerForm::SimplifiedExpression)
    );
    if !answer_ok {
        return Err(FunctionApplicationFailure::UnsupportedArgument);
    }
    let (_, argument) = concrete_argument(target)?;
    algebra::parse(argument)
        .map_err(|error| FunctionApplicationFailure::ExpressionParseFailed(error.to_string()))?
        .evaluate(&[])
        .ok_or(FunctionApplicationFailure::UnsupportedArgument)?;
    algebra::parse(&definition.body_source)
        .map_err(|error| FunctionApplicationFailure::ExpressionParseFailed(error.to_string()))?;
    Ok(definition)
}

/// Execute and independently replay-verify one explicit function application.
pub fn execute_function_application(
    target: &FormalizedTarget,
) -> Result<FunctionExecutionReceipt, FunctionApplicationFailure> {
    let definition = authorize_function_application(target)?;
    let (_, argument_source) = concrete_argument(target)?;
    let argument_expr = algebra::parse(argument_source)
        .map_err(|error| FunctionApplicationFailure::ExpressionParseFailed(error.to_string()))?;
    let _argument_value = argument_expr
        .evaluate(&[])
        .ok_or(FunctionApplicationFailure::UnsupportedArgument)?;
    let body = algebra::parse(&definition.body_source)
        .map_err(|error| FunctionApplicationFailure::ExpressionParseFailed(error.to_string()))?;
    let mut bindings = HashMap::new();
    bindings.insert(definition.parameter.clone(), argument_expr.clone());
    let instantiated_expression = substitute_vars(&body, &bindings);
    let numeric_result = instantiated_expression
        .evaluate(&[])
        .ok_or(FunctionApplicationFailure::EvaluationFailed)?;

    // Replay from the original definition and argument, not from the cached
    // instantiated expression.  This is the independent verification step.
    let replay_body = algebra::parse(&definition.body_source)
        .map_err(|error| FunctionApplicationFailure::ExpressionParseFailed(error.to_string()))?;
    let replay_argument = algebra::parse(argument_source)
        .map_err(|error| FunctionApplicationFailure::ExpressionParseFailed(error.to_string()))?;
    let mut replay_bindings = HashMap::new();
    replay_bindings.insert(definition.parameter.clone(), replay_argument);
    let replay_expression = substitute_vars(&replay_body, &replay_bindings);
    let replay_result = replay_expression
        .evaluate(&[])
        .ok_or(FunctionApplicationFailure::EvaluationFailed)?;
    if (numeric_result - replay_result).abs() > 1e-12 {
        return Err(FunctionApplicationFailure::ReplayVerificationFailed);
    }
    Ok(FunctionExecutionReceipt {
        function_id: definition.function_id,
        definition_source: definition.definition_source,
        parameter: definition.parameter,
        argument_source: argument_source.to_string(),
        instantiated_expression,
        numeric_result,
        replay_verified: true,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::formalization::assess_prompt;

    #[test]
    fn explicit_function_application_executes_and_replays() {
        let trace = assess_prompt("f-1", "Let f(x)=2x+3. What is f(4)?", "Math", false);
        let receipt = execute_function_application(&trace.target_completion.target).unwrap();
        assert_eq!(receipt.function_id, "f");
        assert_eq!(receipt.numeric_result, 11.0);
        assert!(receipt.replay_verified);
    }

    #[test]
    fn undefined_function_is_denied() {
        let trace = assess_prompt("f-2", "What is h(4)?", "Math", false);
        assert_eq!(
            authorize_function_application(&trace.target_completion.target),
            Err(FunctionApplicationFailure::DefinitionUnavailable)
        );
    }

    #[test]
    fn piecewise_like_definition_is_denied() {
        let trace = assess_prompt("f-3", "Let f(x)={x if x>0}. What is f(2)?", "Math", false);
        assert!(authorize_function_application(&trace.target_completion.target).is_err());
    }
}
