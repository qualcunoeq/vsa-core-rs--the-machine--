//! Verified execution for one explicit, single-variable real quadratic.
//!
//! This capability is deliberately narrower than the algebra backend: it
//! accepts only a grounded equation whose parsed degree is exactly two and
//! only publishes finite real solution sets with a quadratic completeness
//! receipt.

use crate::algebra_island::{self, AlgebraAnswer, AlgebraOperation, AlgebraResult};
use crate::capabilities::{CapabilityRegistry, CapabilitySelection};
use crate::formalization::{FormalizedTarget, OperationKind, SubjectObjectType, TargetFieldStatus};
use serde::Serialize;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub enum QuadraticEquationFailure {
    OperationNotSolve,
    SubjectMissing,
    SubjectNotEquation,
    TargetVariableMissing,
    TargetVariableAmbiguous,
    CapabilityContractRejected,
    UnsupportedRelation,
    SolverFailed,
    NoRealSolution,
    ReplayVerificationFailed,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct QuadraticEquationExecutionReceipt {
    pub equation_source: String,
    pub target_variable: String,
    pub result: String,
    pub solution_set: Vec<String>,
    pub completeness_proven: bool,
    pub replay_verified: bool,
}

fn grounded_equation(
    target: &FormalizedTarget,
) -> Result<(String, String), QuadraticEquationFailure> {
    let subject = target
        .subject_resolution
        .selected
        .as_ref()
        .ok_or(QuadraticEquationFailure::SubjectMissing)?;
    if subject.object_type != SubjectObjectType::Equation {
        return Err(QuadraticEquationFailure::SubjectNotEquation);
    }
    let variable = target
        .target_variable
        .as_deref()
        .ok_or(QuadraticEquationFailure::TargetVariableMissing)?;
    if variable.len() != 1
        || !variable
            .chars()
            .all(|character| character.is_ascii_alphabetic())
    {
        return Err(QuadraticEquationFailure::TargetVariableAmbiguous);
    }
    Ok((subject.object.clone(), variable.to_string()))
}

pub fn authorize_quadratic_equation(
    target: &FormalizedTarget,
) -> Result<(String, String, algebra_island::AlgebraProblem), QuadraticEquationFailure> {
    if target.operation != OperationKind::Solve {
        return Err(QuadraticEquationFailure::OperationNotSolve);
    }
    let (equation, variable) = grounded_equation(target)?;
    let problem = algebra_island::parse_problem(&format!("Solve for {variable}: {equation}"))
        .ok_or(QuadraticEquationFailure::UnsupportedRelation)?;
    if problem.operation != AlgebraOperation::SolveQuadraticEquation {
        return Err(QuadraticEquationFailure::UnsupportedRelation);
    }
    let subject = target
        .subject_resolution
        .selected
        .as_ref()
        .ok_or(QuadraticEquationFailure::SubjectMissing)?;
    let registry = CapabilityRegistry::production();
    if !registry.accepts(
        "quadratic_equation_solve",
        subject.object_type,
        target.operation,
        target.answer_form,
    ) || registry.discover(target).selection
        != CapabilitySelection::Unique("quadratic_equation_solve".into())
    {
        return Err(QuadraticEquationFailure::CapabilityContractRejected);
    }
    if target.completeness.target_variable != TargetFieldStatus::Complete {
        return Err(QuadraticEquationFailure::TargetVariableMissing);
    }
    Ok((equation, variable, problem))
}

pub fn execute_quadratic_equation(
    target: &FormalizedTarget,
) -> Result<QuadraticEquationExecutionReceipt, QuadraticEquationFailure> {
    let (equation, variable, _problem) = authorize_quadratic_equation(target)?;
    let answer: AlgebraAnswer =
        algebra_island::try_answer(&format!("Solve for {variable}: {equation}"))
            .ok_or(QuadraticEquationFailure::SolverFailed)?;
    if !answer.receipt.verification.passed
        || answer.receipt.operation != AlgebraOperation::SolveQuadraticEquation
        || !matches!(
            answer.receipt.verification.completeness,
            algebra_island::CompletenessVerification::QuadraticDiscriminant
        )
    {
        return Err(QuadraticEquationFailure::ReplayVerificationFailed);
    }
    let solution_set = match &answer.result {
        AlgebraResult::FiniteSolutionSet(values) if !values.is_empty() => values.clone(),
        AlgebraResult::ExactValue(value) => vec![value.clone()],
        AlgebraResult::NoSolution => return Err(QuadraticEquationFailure::NoRealSolution),
        _ => return Err(QuadraticEquationFailure::SolverFailed),
    };
    Ok(QuadraticEquationExecutionReceipt {
        equation_source: equation,
        target_variable: variable,
        result: answer.answer,
        solution_set,
        completeness_proven: true,
        replay_verified: answer.receipt.verification.passed,
    })
}

pub fn replay_quadratic_equation(receipt: &QuadraticEquationExecutionReceipt) -> bool {
    let Some(answer) = algebra_island::try_answer(&format!(
        "Solve for {}: {}",
        receipt.target_variable, receipt.equation_source
    )) else {
        return false;
    };
    answer.answer == receipt.result
        && answer.receipt.verification.passed
        && answer.receipt.operation == AlgebraOperation::SolveQuadraticEquation
        && matches!(
            answer.receipt.verification.completeness,
            algebra_island::CompletenessVerification::QuadraticDiscriminant
        )
        && receipt.completeness_proven
        && receipt.replay_verified
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::formalization::assess_prompt;

    #[test]
    fn two_real_roots_execute_and_replay() {
        let trace = assess_prompt(
            "quadratic-1",
            "Solve for x: x^2 - 5*x + 6 = 0.",
            "Math",
            false,
        );
        let receipt = execute_quadratic_equation(&trace.target_completion.target).unwrap();
        assert_eq!(receipt.result, "[2, 3]");
        assert!(receipt.replay_verified);
        assert!(replay_quadratic_equation(&receipt));
    }

    #[test]
    fn double_root_executes_and_replays() {
        let trace = assess_prompt(
            "quadratic-2",
            "Solve for x: x^2 - 4*x + 4 = 0.",
            "Math",
            false,
        );
        let receipt = execute_quadratic_equation(&trace.target_completion.target).unwrap();
        assert_eq!(receipt.solution_set, vec!["2"]);
        assert!(replay_quadratic_equation(&receipt));
    }

    #[test]
    fn linear_relation_is_denied() {
        let trace = assess_prompt("quadratic-3", "Solve for x: 2*x + 4 = 0.", "Math", false);
        assert!(authorize_quadratic_equation(&trace.target_completion.target).is_err());
    }

    #[test]
    fn complex_roots_are_denied() {
        let trace = assess_prompt("quadratic-4", "Solve for x: x^2 + 1 = 0.", "Math", false);
        assert_eq!(
            execute_quadratic_equation(&trace.target_completion.target),
            Err(QuadraticEquationFailure::NoRealSolution)
        );
    }
}
