//! Verified execution for one explicit 2×2 linear system.
//!
//! The grammar and backend are intentionally narrow: two equations, two
//! distinct variables, exact coefficients, and a unique solution only.

use crate::algebra_island::{self, AlgebraOperation, AlgebraResult};
use serde::Serialize;
use std::collections::BTreeMap;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub enum LinearSystemFailure {
    UnsupportedInput,
    NotAUniqueSolution,
    ReplayVerificationFailed,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct LinearSystemExecutionReceipt {
    pub source: String,
    pub variables: Vec<String>,
    pub solution: BTreeMap<String, String>,
    pub result: String,
    pub replay_verified: bool,
}

pub fn execute_linear_system(
    source: &str,
) -> Result<LinearSystemExecutionReceipt, LinearSystemFailure> {
    let answer = algebra_island::try_answer(source).ok_or(LinearSystemFailure::UnsupportedInput)?;
    if answer.receipt.operation != AlgebraOperation::SolveSmallLinearSystem
        || !answer.receipt.verification.passed
    {
        return Err(LinearSystemFailure::UnsupportedInput);
    }
    let solution = match answer.result {
        AlgebraResult::UniqueSolution(solution) => solution,
        _ => return Err(LinearSystemFailure::NotAUniqueSolution),
    };
    let variables = solution.keys().cloned().collect::<Vec<_>>();
    let replay = algebra_island::try_answer(source).ok_or(LinearSystemFailure::ReplayVerificationFailed)?;
    if replay.answer != answer.answer
        || replay.receipt.operation != AlgebraOperation::SolveSmallLinearSystem
        || !replay.receipt.verification.passed
    {
        return Err(LinearSystemFailure::ReplayVerificationFailed);
    }
    Ok(LinearSystemExecutionReceipt {
        source: source.trim().to_string(),
        variables,
        solution,
        result: answer.answer,
        replay_verified: true,
    })
}

pub fn replay_linear_system(receipt: &LinearSystemExecutionReceipt) -> bool {
    execute_linear_system(&receipt.source)
        .map(|replayed| {
            replayed.result == receipt.result
                && replayed.solution == receipt.solution
                && replayed.replay_verified
        })
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unique_two_by_two_system_executes_and_replays() {
        let receipt = execute_linear_system("Solve system: x + y = 5; x - y = 1 for x,y").unwrap();
        assert_eq!(receipt.result, "{\"x\": \"3\", \"y\": \"2\"}");
        assert!(receipt.replay_verified);
        assert!(replay_linear_system(&receipt));
    }

    #[test]
    fn degenerate_system_is_rejected() {
        assert!(matches!(
            execute_linear_system("Solve system: x + y = 2; 2*x + 2*y = 4 for x,y"),
            Err(LinearSystemFailure::NotAUniqueSolution)
        ));
    }

    #[test]
    fn nonlinear_system_is_rejected() {
        assert!(execute_linear_system("Solve system: x*y = 2; x + y = 3 for x,y").is_err());
    }
}
