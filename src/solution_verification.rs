//! Independent verification for a single candidate equation solution.

use crate::algebra;
use serde::Serialize;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub enum SolutionVerificationFailure {
    EquationParseRejected,
    CandidateParseRejected,
    CandidateMustBindOneVariable,
    CandidateMustBeNumeric,
    EquationEvaluationFailed,
    CandidateDoesNotSatisfyEquation,
    ReplayVerificationFailed,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct SolutionVerificationReceipt {
    pub equation: String,
    pub candidate: String,
    pub variable: String,
    pub value: f64,
    pub residual_ratio: f64,
    pub verified_solution: String,
    pub replay_verified: bool,
}

pub fn execute_solution_verification(
    equation: &str,
    candidate: &str,
) -> Result<SolutionVerificationReceipt, SolutionVerificationFailure> {
    let parsed_equation = algebra::parse_equation(equation)
        .map_err(|_| SolutionVerificationFailure::EquationParseRejected)?;
    let (candidate_lhs, candidate_rhs) = algebra::parse_equation(candidate)
        .map_err(|_| SolutionVerificationFailure::CandidateParseRejected)?;
    let variable = match candidate_lhs {
        algebra::SymExpr::Var(variable) => variable.display.to_string(),
        _ => return Err(SolutionVerificationFailure::CandidateMustBindOneVariable),
    };
    let value = candidate_rhs
        .evaluate(&[])
        .ok_or(SolutionVerificationFailure::CandidateMustBeNumeric)?;
    let residual_ratio = algebra::evaluate_equation(&parsed_equation, &[(&variable, value)])
        .ok_or(SolutionVerificationFailure::EquationEvaluationFailed)?;
    if (residual_ratio - 1.0).abs() > 1e-9 {
        return Err(SolutionVerificationFailure::CandidateDoesNotSatisfyEquation);
    }
    let receipt = SolutionVerificationReceipt {
        equation: equation.trim().into(),
        candidate: candidate.trim().into(),
        variable: variable.clone(),
        value,
        residual_ratio,
        verified_solution: format!("{} = {}", variable, value),
        replay_verified: false,
    };
    if !replay_solution_verification(&receipt) {
        return Err(SolutionVerificationFailure::ReplayVerificationFailed);
    }
    Ok(SolutionVerificationReceipt {
        replay_verified: true,
        ..receipt
    })
}

pub fn replay_solution_verification(receipt: &SolutionVerificationReceipt) -> bool {
    let Ok(parsed_equation) = algebra::parse_equation(&receipt.equation) else {
        return false;
    };
    let Ok((candidate_lhs, candidate_rhs)) = algebra::parse_equation(&receipt.candidate) else {
        return false;
    };
    let algebra::SymExpr::Var(variable) = candidate_lhs else {
        return false;
    };
    let variable = variable.display.to_string();
    let Some(value) = candidate_rhs.evaluate(&[]) else {
        return false;
    };
    let Some(residual_ratio) = algebra::evaluate_equation(&parsed_equation, &[(&variable, value)])
    else {
        return false;
    };
    variable == receipt.variable
        && (value - receipt.value).abs() <= 1e-12
        && (residual_ratio - 1.0).abs() <= 1e-9
        && receipt.verified_solution == format!("{} = {}", variable, value)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn verifies_linear_candidate_and_replays() {
        let receipt = execute_solution_verification("2*x + 1 = 7", "x = 3").unwrap();
        assert!(receipt.replay_verified);
        assert_eq!(receipt.verified_solution, "x = 3");
    }

    #[test]
    fn rejects_wrong_candidate() {
        assert_eq!(
            execute_solution_verification("2*x + 1 = 7", "x = 4"),
            Err(SolutionVerificationFailure::CandidateDoesNotSatisfyEquation)
        );
    }

    #[test]
    fn rejects_non_numeric_candidate() {
        assert_eq!(
            execute_solution_verification("2*x + 1 = 7", "x = y"),
            Err(SolutionVerificationFailure::CandidateMustBeNumeric)
        );
    }
}
