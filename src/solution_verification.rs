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

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub enum SolutionSetVerificationFailure {
    EquationParseRejected,
    EmptySolutionSet,
    CandidateNotNumeric,
    SolverDidNotProduceFiniteSet,
    IncompleteSolutionSet,
    ReplayVerificationFailed,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct SolutionSetVerificationReceipt {
    pub equation: String,
    pub target_variable: String,
    pub candidates: Vec<String>,
    pub verified_solution_set: Vec<String>,
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

/// Verify that a submitted finite root set exactly matches the bounded
/// algebra solver's finite solution set. This is intentionally limited to
/// one-variable real solution sets.
pub fn execute_solution_set_verification(
    equation: &str,
    target_variable: &str,
    candidates: &[&str],
) -> Result<SolutionSetVerificationReceipt, SolutionSetVerificationFailure> {
    if algebra::parse_equation(equation).is_err() {
        return Err(SolutionSetVerificationFailure::EquationParseRejected);
    }
    if candidates.is_empty() {
        return Err(SolutionSetVerificationFailure::EmptySolutionSet);
    }
    let mut candidate_values = candidates
        .iter()
        .map(|candidate| {
            candidate
                .trim()
                .parse::<f64>()
                .map_err(|_| SolutionSetVerificationFailure::CandidateNotNumeric)
        })
        .collect::<Result<Vec<_>, _>>()?;
    candidate_values.sort_by(f64::total_cmp);
    candidate_values.dedup_by(|a, b| (*a - *b).abs() <= 1e-12);

    let answer = crate::algebra_island::try_answer(&format!(
        "Solve for {target_variable}: {equation}"
    ))
    .ok_or(SolutionSetVerificationFailure::SolverDidNotProduceFiniteSet)?;
    let expected = match answer.result {
        crate::algebra_island::AlgebraResult::FiniteSolutionSet(values) => values,
        _ => return Err(SolutionSetVerificationFailure::SolverDidNotProduceFiniteSet),
    };
    let mut expected_values = expected
        .iter()
        .map(|value| {
            value
                .parse::<f64>()
                .map_err(|_| SolutionSetVerificationFailure::SolverDidNotProduceFiniteSet)
        })
        .collect::<Result<Vec<_>, _>>()?;
    expected_values.sort_by(f64::total_cmp);
    expected_values.dedup_by(|a, b| (*a - *b).abs() <= 1e-12);
    if candidate_values.len() != expected_values.len()
        || candidate_values
            .iter()
            .zip(&expected_values)
            .any(|(candidate, expected)| (*candidate - *expected).abs() > 1e-9)
    {
        return Err(SolutionSetVerificationFailure::IncompleteSolutionSet);
    }
    let verified_solution_set = expected_values.iter().map(|value| value.to_string()).collect();
    let receipt = SolutionSetVerificationReceipt {
        equation: equation.trim().into(),
        target_variable: target_variable.trim().into(),
        candidates: candidates.iter().map(|candidate| candidate.trim().into()).collect(),
        verified_solution_set,
        replay_verified: false,
    };
    if !replay_solution_set_verification(&receipt) {
        return Err(SolutionSetVerificationFailure::ReplayVerificationFailed);
    }
    Ok(SolutionSetVerificationReceipt {
        replay_verified: true,
        ..receipt
    })
}

pub fn replay_solution_set_verification(receipt: &SolutionSetVerificationReceipt) -> bool {
    let Ok((_, _)) = algebra::parse_equation(&receipt.equation) else {
        return false;
    };
    let Some(answer) = crate::algebra_island::try_answer(&format!(
        "Solve for {}: {}",
        receipt.target_variable, receipt.equation
    )) else {
        return false;
    };
    let crate::algebra_island::AlgebraResult::FiniteSolutionSet(values) = answer.result else {
        return false;
    };
    let mut expected = values
        .iter()
        .filter_map(|value| value.parse::<f64>().ok())
        .collect::<Vec<_>>();
    expected.sort_by(f64::total_cmp);
    expected.dedup_by(|a, b| (*a - *b).abs() <= 1e-12);
    let verified = expected.iter().map(|value| value.to_string()).collect::<Vec<_>>();
    verified == receipt.verified_solution_set
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

    #[test]
    fn verifies_complete_quadratic_solution_set() {
        let receipt = execute_solution_set_verification("x^2 - 4 = 0", "x", &["-2", "2"])
            .unwrap();
        assert!(receipt.replay_verified);
        assert_eq!(receipt.verified_solution_set.len(), 2);
    }

    #[test]
    fn rejects_incomplete_solution_set() {
        assert_eq!(
            execute_solution_set_verification("x^2 - 4 = 0", "x", &["2"]),
            Err(SolutionSetVerificationFailure::IncompleteSolutionSet)
        );
    }
}
