//! Narrow, verified evaluation of a grounded explicit expression.
//!
//! This capability deliberately does not choose an expression from prose and
//! does not solve for missing values.  The formalization layer must already
//! have selected one `Expression` subject; this module only checks bindings,
//! evaluates it, and replays the computation from the original source.

use crate::algebra::{self, SymExpr};
use crate::capabilities::{CapabilityRegistry, CapabilitySelection};
use crate::formalization::{FormalizedTarget, SubjectObjectType, TargetFieldStatus};
use crate::math_ingest::substitute_vars;
use serde::Serialize;
use std::collections::{BTreeSet, HashMap};

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub enum ExpressionEvaluationFailure {
    OperationNotEvaluate,
    SubjectNotExpression,
    SubjectMissing,
    CapabilityContractRejected,
    ExpressionParseFailed(String),
    BindingMissing(Vec<String>),
    BindingAmbiguous,
    UnsupportedArgument(String),
    UnsupportedExpression,
    EvaluationFailed,
    ReplayVerificationFailed,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ExpressionExecutionReceipt {
    pub expression_source: String,
    pub argument_bindings: Vec<(String, String)>,
    pub instantiated_expression: SymExpr,
    pub numeric_result: f64,
    pub replay_verified: bool,
}

fn expression_source(target: &FormalizedTarget) -> Result<String, ExpressionEvaluationFailure> {
    let subject = target
        .subject_resolution
        .selected
        .as_ref()
        .ok_or(ExpressionEvaluationFailure::SubjectMissing)?;
    if subject.object_type != SubjectObjectType::Expression {
        return Err(ExpressionEvaluationFailure::SubjectNotExpression);
    }
    // The target extractor may retain the concrete `at x=...` clause in the
    // expression payload.  It is a binding, not part of the subject.
    let source = regex::Regex::new(r"(?i)\s+(?:at|when)\s+[A-Za-z_][A-Za-z0-9_]*\s*=")
        .expect("static expression binding suffix regex")
        .split(&subject.object)
        .next()
        .unwrap_or(subject.object.as_str())
        .trim()
        .trim_end_matches([',', ';', '.'])
        .trim()
        .to_string();
    if source.is_empty() {
        return Err(ExpressionEvaluationFailure::SubjectMissing);
    }
    Ok(source)
}

fn collect_variables(expr: &SymExpr, out: &mut BTreeSet<String>) {
    match expr {
        SymExpr::Num(_) => {}
        SymExpr::Var(variable) => {
            out.insert(variable.display.to_string());
        }
        SymExpr::Add(a, b)
        | SymExpr::Sub(a, b)
        | SymExpr::Mul(a, b)
        | SymExpr::Div(a, b)
        | SymExpr::Pow(a, b) => {
            collect_variables(a, out);
            collect_variables(b, out);
        }
        SymExpr::Neg(a)
        | SymExpr::Sin(a)
        | SymExpr::Cos(a)
        | SymExpr::Tan(a)
        | SymExpr::Sqrt(a)
        | SymExpr::Exp(a)
        | SymExpr::Ln(a)
        | SymExpr::Abs(a)
        | SymExpr::Sinh(a)
        | SymExpr::Cosh(a)
        | SymExpr::Tanh(a)
        | SymExpr::Asin(a)
        | SymExpr::Acos(a)
        | SymExpr::Atan(a) => collect_variables(a, out),
        SymExpr::Limit { approach, body, .. } => {
            collect_variables(approach, out);
            collect_variables(body, out);
        }
        SymExpr::Integral {
            lower, upper, body, ..
        } => {
            if let Some(value) = lower {
                collect_variables(value, out);
            }
            if let Some(value) = upper {
                collect_variables(value, out);
            }
            collect_variables(body, out);
        }
    }
}

fn checked_bindings(
    target: &FormalizedTarget,
    variables: &BTreeSet<String>,
) -> Result<(HashMap<String, SymExpr>, Vec<(String, String)>), ExpressionEvaluationFailure> {
    let mut bindings = HashMap::new();
    let mut receipt_bindings = Vec::new();
    for binding in &target.arguments {
        if binding.status == TargetFieldStatus::Ambiguous {
            return Err(ExpressionEvaluationFailure::BindingAmbiguous);
        }
        if binding.status != TargetFieldStatus::Complete {
            continue;
        }
        let value = algebra::parse(binding.value.trim())
            .map_err(|error| ExpressionEvaluationFailure::UnsupportedArgument(error.to_string()))?;
        if value.evaluate(&[]).is_none() {
            return Err(ExpressionEvaluationFailure::UnsupportedArgument(
                binding.value.clone(),
            ));
        }
        if variables.contains(&binding.parameter) {
            bindings.insert(binding.parameter.clone(), value);
            receipt_bindings.push((binding.parameter.clone(), binding.value.clone()));
        }
    }
    let missing = variables
        .iter()
        .filter(|variable| !bindings.contains_key(*variable))
        .cloned()
        .collect::<Vec<_>>();
    if !missing.is_empty() {
        return Err(ExpressionEvaluationFailure::BindingMissing(missing));
    }
    Ok((bindings, receipt_bindings))
}

pub fn authorize_expression_evaluation(
    target: &FormalizedTarget,
) -> Result<
    (
        String,
        SymExpr,
        HashMap<String, SymExpr>,
        Vec<(String, String)>,
    ),
    ExpressionEvaluationFailure,
> {
    if target.operation != crate::formalization::OperationKind::Evaluate {
        return Err(ExpressionEvaluationFailure::OperationNotEvaluate);
    }
    let source = expression_source(target)?;
    let subject = target
        .subject_resolution
        .selected
        .as_ref()
        .ok_or(ExpressionEvaluationFailure::SubjectMissing)?;
    let registry = CapabilityRegistry::production();
    if !registry.accepts(
        "expression_evaluation",
        subject.object_type,
        target.operation,
        target.answer_form,
    ) || registry.discover(target).selection
        != CapabilitySelection::Unique("expression_evaluation".into())
    {
        return Err(ExpressionEvaluationFailure::CapabilityContractRejected);
    }
    let expression =
        algebra::parse(&source).map_err(ExpressionEvaluationFailure::ExpressionParseFailed)?;
    let mut variables = BTreeSet::new();
    collect_variables(&expression, &mut variables);
    let (bindings, receipt_bindings) = checked_bindings(target, &variables)?;
    Ok((source, expression, bindings, receipt_bindings))
}

pub fn execute_expression_evaluation(
    target: &FormalizedTarget,
) -> Result<ExpressionExecutionReceipt, ExpressionEvaluationFailure> {
    let (source, expression, bindings, receipt_bindings) = authorize_expression_evaluation(target)?;
    let instantiated_expression = substitute_vars(&expression, &bindings);
    let numeric_result = instantiated_expression
        .evaluate(&[])
        .ok_or(ExpressionEvaluationFailure::EvaluationFailed)?;

    // Replay from the original source and bindings instead of trusting the
    // cached AST or instantiated expression.
    let replay_expression =
        algebra::parse(&source).map_err(ExpressionEvaluationFailure::ExpressionParseFailed)?;
    let replay_bindings = receipt_bindings
        .iter()
        .map(|(name, value)| {
            algebra::parse(value)
                .map(|parsed| (name.clone(), parsed))
                .map_err(|error| {
                    ExpressionEvaluationFailure::UnsupportedArgument(error.to_string())
                })
        })
        .collect::<Result<HashMap<_, _>, _>>()?;
    let replay_result = substitute_vars(&replay_expression, &replay_bindings)
        .evaluate(&[])
        .ok_or(ExpressionEvaluationFailure::EvaluationFailed)?;
    if (numeric_result - replay_result).abs() > 1e-12 {
        return Err(ExpressionEvaluationFailure::ReplayVerificationFailed);
    }
    Ok(ExpressionExecutionReceipt {
        expression_source: source,
        argument_bindings: receipt_bindings,
        instantiated_expression,
        numeric_result,
        replay_verified: true,
    })
}

pub fn replay_expression_evaluation(receipt: &ExpressionExecutionReceipt) -> bool {
    let Ok(expression) = algebra::parse(&receipt.expression_source) else {
        return false;
    };
    let Ok(bindings) = receipt
        .argument_bindings
        .iter()
        .map(|(name, value)| algebra::parse(value).map(|parsed| (name.clone(), parsed)))
        .collect::<Result<HashMap<_, _>, _>>()
    else {
        return false;
    };
    substitute_vars(&expression, &bindings)
        .evaluate(&[])
        .map(|value| (value - receipt.numeric_result).abs() <= 1e-12)
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::formalization::assess_prompt;

    #[test]
    fn numeric_expression_executes_and_replays() {
        let trace = assess_prompt("expr-1", "Evaluate 2+3.", "Math", false);
        let receipt = execute_expression_evaluation(&trace.target_completion.target).unwrap();
        assert_eq!(receipt.numeric_result, 5.0);
        assert!(receipt.replay_verified);
        assert!(replay_expression_evaluation(&receipt));
    }

    #[test]
    fn bound_expression_executes_and_replays() {
        let trace = assess_prompt("expr-2", "Evaluate 2*x+3 at x=4.", "Math", false);
        let receipt = execute_expression_evaluation(&trace.target_completion.target).unwrap();
        assert_eq!(receipt.numeric_result, 11.0);
        assert!(receipt.replay_verified);
    }

    #[test]
    fn unbound_expression_is_denied() {
        let trace = assess_prompt("expr-3", "Evaluate 2*x+3.", "Math", false);
        assert!(matches!(
            authorize_expression_evaluation(&trace.target_completion.target),
            Err(ExpressionEvaluationFailure::BindingMissing(_))
                | Err(ExpressionEvaluationFailure::CapabilityContractRejected)
        ));
    }

    #[test]
    fn unsupported_expression_is_denied() {
        let trace = assess_prompt("expr-4", "Evaluate integral(x).", "Math", false);
        assert!(authorize_expression_evaluation(&trace.target_completion.target).is_err());
    }
}
