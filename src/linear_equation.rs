//! Verified execution for one explicit, single-variable linear equation.
//!
//! This is intentionally a capability boundary, not a general equation
//! router.  The subject must already be grounded as an `Equation`, the target
//! variable must be explicit, and the bounded algebra island must classify the
//! relation as degree one before execution is authorized.

use crate::algebra_island::{self, AlgebraAnswer, AlgebraOperation, AlgebraResult};
use crate::capabilities::{CapabilityRegistry, CapabilitySelection};
use crate::formalization::{FormalizedTarget, SubjectObjectType, TargetFieldStatus};
use serde::Serialize;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub enum LinearEquationFailure {
    OperationNotSolve,
    SubjectMissing,
    SubjectNotEquation,
    TargetVariableMissing,
    TargetVariableAmbiguous,
    CapabilityContractRejected,
    UnsupportedRelation,
    SolverFailed,
    NonUniqueResult,
    ReplayVerificationFailed,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct LinearEquationExecutionReceipt {
    pub equation_source: String,
    pub target_variable: String,
    pub result: String,
    pub solution_set: Vec<String>,
    pub completeness_proven: bool,
    pub replay_verified: bool,
}

fn grounded_equation(target: &FormalizedTarget) -> Result<(String, String), LinearEquationFailure> {
    let subject = target
        .subject_resolution
        .selected
        .as_ref()
        .ok_or(LinearEquationFailure::SubjectMissing)?;
    if subject.object_type != SubjectObjectType::Equation {
        return Err(LinearEquationFailure::SubjectNotEquation);
    }
    let variable = target
        .target_variable
        .as_deref()
        .ok_or(LinearEquationFailure::TargetVariableMissing)?;
    if variable.len() != 1
        || !variable
            .chars()
            .all(|character| character.is_ascii_alphabetic())
    {
        return Err(LinearEquationFailure::TargetVariableAmbiguous);
    }
    Ok((subject.object.clone(), variable.to_string()))
}

fn problem_for(
    target: &FormalizedTarget,
) -> Result<(String, String, crate::algebra_island::AlgebraProblem), LinearEquationFailure> {
    let (equation, variable) = grounded_equation(target)?;
    let problem = algebra_island::parse_problem(&format!("Solve for {variable}: {equation}"))
        .ok_or(LinearEquationFailure::UnsupportedRelation)?;
    if problem.operation != AlgebraOperation::SolveLinearEquation {
        return Err(LinearEquationFailure::UnsupportedRelation);
    }
    Ok((equation, variable, problem))
}

pub fn authorize_linear_equation(
    target: &FormalizedTarget,
) -> Result<(String, String, crate::algebra_island::AlgebraProblem), LinearEquationFailure> {
    if target.operation != crate::formalization::OperationKind::Solve {
        return Err(LinearEquationFailure::OperationNotSolve);
    }
    let (equation, variable, problem) = problem_for(target)?;
    let subject = target
        .subject_resolution
        .selected
        .as_ref()
        .ok_or(LinearEquationFailure::SubjectMissing)?;
    let registry = CapabilityRegistry::production();
    if !registry.accepts(
        "linear_equation_solve",
        subject.object_type,
        target.operation,
        target.answer_form,
    ) || registry.discover(target).selection
        != CapabilitySelection::Unique("linear_equation_solve".into())
    {
        return Err(LinearEquationFailure::CapabilityContractRejected);
    }
    if target.completeness.target_variable != TargetFieldStatus::Complete {
        return Err(LinearEquationFailure::TargetVariableMissing);
    }
    Ok((equation, variable, problem))
}

pub fn execute_linear_equation(
    target: &FormalizedTarget,
) -> Result<LinearEquationExecutionReceipt, LinearEquationFailure> {
    let (equation, variable, _problem) = authorize_linear_equation(target)?;
    let answer: AlgebraAnswer =
        algebra_island::try_answer(&format!("Solve for {variable}: {equation}"))
            .ok_or(LinearEquationFailure::SolverFailed)?;
    if !answer.receipt.verification.passed
        || answer.receipt.operation != AlgebraOperation::SolveLinearEquation
    {
        return Err(LinearEquationFailure::ReplayVerificationFailed);
    }
    let solution_set = match &answer.result {
        AlgebraResult::FiniteSolutionSet(values) => values.clone(),
        AlgebraResult::ExactValue(value) => vec![value.clone()],
        _ => return Err(LinearEquationFailure::NonUniqueResult),
    };
    if solution_set.len() != 1 {
        return Err(LinearEquationFailure::NonUniqueResult);
    }
    Ok(LinearEquationExecutionReceipt {
        equation_source: equation,
        target_variable: variable,
        result: answer.answer,
        solution_set,
        completeness_proven: matches!(
            answer.receipt.verification.completeness,
            algebra_island::CompletenessVerification::LinearDegreeOne
        ),
        replay_verified: answer.receipt.verification.passed,
    })
}

pub fn replay_linear_equation(receipt: &LinearEquationExecutionReceipt) -> bool {
    let Some(answer) = algebra_island::try_answer(&format!(
        "Solve for {}: {}",
        receipt.target_variable, receipt.equation_source
    )) else {
        return false;
    };
    answer.answer == receipt.result
        && answer.receipt.verification.passed
        && receipt.completeness_proven
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::formalization::assess_prompt;

    #[test]
    fn unique_solution_executes_and_replays() {
        let trace = assess_prompt("linear-1", "Solve for x: 3*x+2=11.", "Math", false);
        let receipt = execute_linear_equation(&trace.target_completion.target).unwrap();
        assert_eq!(receipt.result, "3");
        assert!(receipt.replay_verified);
        assert!(replay_linear_equation(&receipt));
    }

    #[test]
    fn fractional_solution_executes_and_replays() {
        let trace = assess_prompt("linear-2", "Solve for x: 2*x=1.", "Math", false);
        let receipt = execute_linear_equation(&trace.target_completion.target).unwrap();
        assert_eq!(receipt.result, "0.5");
        assert!(receipt.replay_verified);
    }

    #[test]
    fn quadratic_is_denied() {
        let trace = assess_prompt("linear-3", "Solve for x: x^2=4.", "Math", false);
        assert!(authorize_linear_equation(&trace.target_completion.target).is_err());
    }

    #[test]
    fn multiple_variables_are_denied() {
        let trace = assess_prompt("linear-4", "Solve for x: x+y=4.", "Math", false);
        assert!(authorize_linear_equation(&trace.target_completion.target).is_err());
    }
}
