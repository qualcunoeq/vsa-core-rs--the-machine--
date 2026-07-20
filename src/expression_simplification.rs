//! Verified execution for explicit expression simplification.
//!
//! Simplification may preserve free variables, but it still requires a
//! parseable expression and an independent replay of the same algebra-island
//! operation.

use crate::algebra_island::{self, AlgebraAnswer, AlgebraOperation};
use crate::capabilities::{CapabilityRegistry, CapabilitySelection};
use crate::formalization::{FormalizedTarget, OperationKind, SubjectObjectType};
use serde::Serialize;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub enum ExpressionSimplificationFailure {
    OperationNotSimplify,
    SubjectMissing,
    SubjectNotExpression,
    CapabilityContractRejected,
    UnsupportedExpression,
    SolverFailed,
    ReplayVerificationFailed,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ExpressionSimplificationReceipt {
    pub expression_source: String,
    pub simplified_expression: String,
    pub replay_verified: bool,
}

fn grounded_expression(target: &FormalizedTarget) -> Result<String, ExpressionSimplificationFailure> {
    if target.operation != OperationKind::Simplify {
        return Err(ExpressionSimplificationFailure::OperationNotSimplify);
    }
    let subject = target
        .subject_resolution
        .selected
        .as_ref()
        .ok_or(ExpressionSimplificationFailure::SubjectMissing)?;
    if subject.object_type != SubjectObjectType::Expression {
        return Err(ExpressionSimplificationFailure::SubjectNotExpression);
    }
    Ok(subject.object.clone())
}

pub fn authorize_expression_simplification(
    target: &FormalizedTarget,
) -> Result<String, ExpressionSimplificationFailure> {
    let expression = grounded_expression(target)?;
    let registry = CapabilityRegistry::production();
    if !registry.accepts(
        "expression_simplification",
        SubjectObjectType::Expression,
        target.operation,
        target.answer_form,
    ) || registry.discover(target).selection
        != CapabilitySelection::Unique("expression_simplification".into())
    {
        return Err(ExpressionSimplificationFailure::CapabilityContractRejected);
    }
    if crate::algebra::parse(expression.trim()).is_err() {
        return Err(ExpressionSimplificationFailure::UnsupportedExpression);
    }
    Ok(expression)
}

pub fn execute_expression_simplification(
    target: &FormalizedTarget,
) -> Result<ExpressionSimplificationReceipt, ExpressionSimplificationFailure> {
    let expression = authorize_expression_simplification(target)?;
    let answer: AlgebraAnswer = algebra_island::try_answer(&format!("Simplify {expression}"))
        .ok_or(ExpressionSimplificationFailure::SolverFailed)?;
    if !answer.receipt.verification.passed
        || answer.receipt.operation != AlgebraOperation::SimplifyExpression
    {
        return Err(ExpressionSimplificationFailure::ReplayVerificationFailed);
    }
    Ok(ExpressionSimplificationReceipt {
        expression_source: expression,
        simplified_expression: answer.answer,
        replay_verified: true,
    })
}

pub fn replay_expression_simplification(receipt: &ExpressionSimplificationReceipt) -> bool {
    let Some(answer) = algebra_island::try_answer(&format!(
        "Simplify {}",
        receipt.expression_source
    )) else {
        return false;
    };
    answer.answer == receipt.simplified_expression
        && answer.receipt.operation == AlgebraOperation::SimplifyExpression
        && answer.receipt.verification.passed
        && receipt.replay_verified
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::formalization::assess_prompt;

    #[test]
    fn symbolic_expression_simplifies_and_replays() {
        let trace = assess_prompt("simplify-1", "Simplify x + 0.", "Math", false);
        let receipt = execute_expression_simplification(&trace.target_completion.target).unwrap();
        assert_eq!(receipt.simplified_expression, "x");
        assert!(replay_expression_simplification(&receipt));
    }

    #[test]
    fn numeric_expression_simplifies_and_replays() {
        let trace = assess_prompt("simplify-2", "Simplify 2 + 3.", "Math", false);
        let receipt = execute_expression_simplification(&trace.target_completion.target).unwrap();
        assert_eq!(receipt.simplified_expression, "5");
        assert!(replay_expression_simplification(&receipt));
    }

    #[test]
    fn equation_is_not_accepted_as_expression() {
        let trace = assess_prompt("simplify-3", "Simplify x = 2.", "Math", false);
        assert!(authorize_expression_simplification(&trace.target_completion.target).is_err());
    }
}
