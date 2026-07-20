//! Verified explicit substitution over one grounded expression.

use crate::algebra::{self, SymExpr};
use crate::capabilities::{CapabilityRegistry, CapabilitySelection};
use crate::formalization::{FormalizedTarget, SubjectObjectType, TargetFieldStatus};
use crate::math_ingest::substitute_vars;
use serde::Serialize;
use std::collections::{BTreeSet, HashMap};

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub enum SubstitutionFailure {
    OperationNotSubstitute,
    SubjectMissing,
    SubjectNotExpression,
    CapabilityContractRejected,
    ExpressionParseFailed(String),
    BindingMissing(Vec<String>),
    BindingAmbiguous,
    UnsupportedBinding(String),
    ReplayVerificationFailed,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct SubstitutionExecutionReceipt {
    pub expression_source: String,
    pub bindings: Vec<(String, String)>,
    pub instantiated_expression: SymExpr,
    pub numeric_result: Option<f64>,
    pub replay_verified: bool,
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

fn grounded_input(
    target: &FormalizedTarget,
) -> Result<(String, SymExpr, HashMap<String, SymExpr>, Vec<(String, String)>), SubstitutionFailure>
{
    let subject = target
        .subject_resolution
        .selected
        .as_ref()
        .ok_or(SubstitutionFailure::SubjectMissing)?;
    if subject.object_type != SubjectObjectType::Expression {
        return Err(SubstitutionFailure::SubjectNotExpression);
    }
    let source = subject.object.trim().to_string();
    let expression = algebra::parse(&source)
        .map_err(SubstitutionFailure::ExpressionParseFailed)?;
    let mut variables = BTreeSet::new();
    collect_variables(&expression, &mut variables);
    let mut bindings = HashMap::new();
    let mut receipt_bindings = Vec::new();
    for binding in &target.arguments {
        if binding.status == TargetFieldStatus::Ambiguous {
            return Err(SubstitutionFailure::BindingAmbiguous);
        }
        if binding.status != TargetFieldStatus::Complete {
            continue;
        }
        let value = algebra::parse(binding.value.trim()).map_err(|error| {
            SubstitutionFailure::UnsupportedBinding(error.to_string())
        })?;
        if value.evaluate(&[]).is_none() || !variables.contains(&binding.parameter) {
            return Err(SubstitutionFailure::UnsupportedBinding(binding.parameter.clone()));
        }
        bindings.insert(binding.parameter.clone(), value);
        receipt_bindings.push((binding.parameter.clone(), binding.value.clone()));
    }
    let missing = variables
        .iter()
        .filter(|variable| !bindings.contains_key(*variable))
        .cloned()
        .collect::<Vec<_>>();
    if !missing.is_empty() {
        return Err(SubstitutionFailure::BindingMissing(missing));
    }
    Ok((source, expression, bindings, receipt_bindings))
}

pub fn authorize_substitution(
    target: &FormalizedTarget,
) -> Result<(String, SymExpr, HashMap<String, SymExpr>, Vec<(String, String)>), SubstitutionFailure>
{
    if target.operation != crate::formalization::OperationKind::Substitute {
        return Err(SubstitutionFailure::OperationNotSubstitute);
    }
    let input = grounded_input(target)?;
    let subject = target
        .subject_resolution
        .selected
        .as_ref()
        .ok_or(SubstitutionFailure::SubjectMissing)?;
    let registry = CapabilityRegistry::production();
    if !registry.accepts(
        "substitution",
        subject.object_type,
        target.operation,
        target.answer_form,
    ) || registry.discover(target).selection
        != CapabilitySelection::Unique("substitution".into())
    {
        return Err(SubstitutionFailure::CapabilityContractRejected);
    }
    Ok(input)
}

pub fn execute_substitution(
    target: &FormalizedTarget,
) -> Result<SubstitutionExecutionReceipt, SubstitutionFailure> {
    let (source, expression, bindings, receipt_bindings) = authorize_substitution(target)?;
    let instantiated_expression = substitute_vars(&expression, &bindings);
    let numeric_result = instantiated_expression.evaluate(&[]);

    let replay_expression = algebra::parse(&source)
        .map_err(SubstitutionFailure::ExpressionParseFailed)?;
    let replay_bindings = receipt_bindings
        .iter()
        .map(|(name, value)| {
            algebra::parse(value)
                .map(|parsed| (name.clone(), parsed))
                .map_err(|error| SubstitutionFailure::UnsupportedBinding(error.to_string()))
        })
        .collect::<Result<HashMap<_, _>, _>>()?;
    let replay_result = substitute_vars(&replay_expression, &replay_bindings).evaluate(&[]);
    if numeric_result != replay_result {
        return Err(SubstitutionFailure::ReplayVerificationFailed);
    }
    Ok(SubstitutionExecutionReceipt {
        expression_source: source,
        bindings: receipt_bindings,
        instantiated_expression,
        numeric_result,
        replay_verified: true,
    })
}

pub fn replay_substitution(receipt: &SubstitutionExecutionReceipt) -> bool {
    let Ok(expression) = algebra::parse(&receipt.expression_source) else {
        return false;
    };
    let Ok(bindings) = receipt
        .bindings
        .iter()
        .map(|(name, value)| algebra::parse(value).map(|parsed| (name.clone(), parsed)))
        .collect::<Result<HashMap<_, _>, _>>()
    else {
        return false;
    };
    substitute_vars(&expression, &bindings).evaluate(&[]) == receipt.numeric_result
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::formalization::assess_prompt;

    #[test]
    fn numeric_binding_executes_and_replays() {
        let trace = assess_prompt(
            "substitution-1",
            "Substitute x=4 into x^2-1.",
            "Math",
            false,
        );
        let receipt = execute_substitution(&trace.target_completion.target).unwrap();
        assert_eq!(receipt.numeric_result, Some(15.0));
        assert!(receipt.replay_verified);
        assert!(replay_substitution(&receipt));
    }

    #[test]
    fn missing_binding_is_denied() {
        let trace = assess_prompt("substitution-2", "Substitute x=4 into x+y.", "Math", false);
        assert!(matches!(
            authorize_substitution(&trace.target_completion.target),
            Err(SubstitutionFailure::BindingMissing(_))
                | Err(SubstitutionFailure::CapabilityContractRejected)
        ));
    }

    #[test]
    fn ambiguous_binding_is_denied() {
        let trace = assess_prompt(
            "substitution-3",
            "Substitute x=4 and x=5 into x+1.",
            "Math",
            false,
        );
        assert!(authorize_substitution(&trace.target_completion.target).is_err());
    }
}
