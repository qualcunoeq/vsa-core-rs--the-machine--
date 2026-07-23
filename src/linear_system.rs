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

/// Classification produced before the execution gate. Degenerate systems
/// remain valid mathematical objects but are not materialized as unique
/// solution receipts.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub enum LinearSystemClassification {
    Unique(BTreeMap<String, String>),
    NoSolution,
    InfiniteSolutions(String),
    Unsupported,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct LinearSystemExecutionReceipt {
    pub source: String,
    pub variables: Vec<String>,
    pub solution: BTreeMap<String, String>,
    pub result: String,
    pub replay_verified: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct LinearSystemAblationReport {
    pub cases: usize,
    pub unique_cases: usize,
    pub classifier_bypass_false_accepts: usize,
    pub replay_valid_accepts: usize,
    pub replay_tampered_rejections: usize,
    pub replay_bypass_tampered_accepts: usize,
}

/// Measure the two safety boundaries without changing the production path:
/// accepting any parsed result instead of requiring a unique classification,
/// and accepting a receipt without replaying its source. The bypass counts
/// are diagnostic evidence, not alternate execution behavior.
pub fn evaluate_system_ablations(sources: &[&str]) -> LinearSystemAblationReport {
    let mut report = LinearSystemAblationReport {
        cases: sources.len(),
        unique_cases: 0,
        classifier_bypass_false_accepts: 0,
        replay_valid_accepts: 0,
        replay_tampered_rejections: 0,
        replay_bypass_tampered_accepts: 0,
    };
    for source in sources {
        let classification = classify_linear_system(source);
        if matches!(classification, LinearSystemClassification::Unique(_)) {
            report.unique_cases += 1;
            let Ok(receipt) = execute_linear_system(source) else {
                continue;
            };
            report.replay_valid_accepts += usize::from(replay_linear_system(&receipt));
            let mut tampered = receipt.clone();
            tampered.result = "tampered-result".into();
            tampered.solution.clear();
            report.replay_tampered_rejections += usize::from(!replay_linear_system(&tampered));
            // A receipt-only consumer would accept this mutated object; this
            // is the deliberately unsafe counterfactual being measured.
            report.replay_bypass_tampered_accepts += 1;
        } else if algebra_island::try_answer(source).is_some() {
            // The parser understood a degenerate system, but an ablated
            // classifier that ignored rank/uniqueness would falsely accept it.
            report.classifier_bypass_false_accepts += 1;
        }
    }
    report
}

pub fn classify_linear_system(source: &str) -> LinearSystemClassification {
    let Some(answer) = algebra_island::try_answer(source) else {
        return LinearSystemClassification::Unsupported;
    };
    if answer.receipt.operation != AlgebraOperation::SolveSmallLinearSystem
        || !answer.receipt.verification.passed
    {
        return LinearSystemClassification::Unsupported;
    }
    match answer.result {
        AlgebraResult::UniqueSolution(solution) => LinearSystemClassification::Unique(solution),
        AlgebraResult::NoSolution => LinearSystemClassification::NoSolution,
        AlgebraResult::InfiniteSolutions(reason) => {
            LinearSystemClassification::InfiniteSolutions(reason)
        }
        _ => LinearSystemClassification::Unsupported,
    }
}

pub fn execute_linear_system(
    source: &str,
) -> Result<LinearSystemExecutionReceipt, LinearSystemFailure> {
    let solution = match classify_linear_system(source) {
        LinearSystemClassification::Unique(solution) => solution,
        LinearSystemClassification::NoSolution
        | LinearSystemClassification::InfiniteSolutions(_) => {
            return Err(LinearSystemFailure::NotAUniqueSolution)
        }
        LinearSystemClassification::Unsupported => {
            return Err(LinearSystemFailure::UnsupportedInput)
        }
    };
    let answer = algebra_island::try_answer(source).ok_or(LinearSystemFailure::UnsupportedInput)?;
    let variables = solution.keys().cloned().collect::<Vec<_>>();
    let replay =
        algebra_island::try_answer(source).ok_or(LinearSystemFailure::ReplayVerificationFailed)?;
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

    #[test]
    fn classification_preserves_degenerate_and_unsupported_outcomes() {
        assert_eq!(
            classify_linear_system("Solve system: x+y=2; 2*x+2*y=5 for x,y"),
            LinearSystemClassification::NoSolution
        );
        assert!(matches!(
            classify_linear_system("Solve system: x+y=2; 2*x+2*y=4 for x,y"),
            LinearSystemClassification::InfiniteSolutions(_)
        ));
        assert_eq!(
            classify_linear_system("Solve system: x*y=2; x+y=3 for x,y"),
            LinearSystemClassification::Unsupported
        );
    }

    #[test]
    fn prose_system_with_ordered_pair_request_executes() {
        let receipt =
            execute_linear_system("The pair obeys x+3*y=11 and 2*x-y=3. Solve for x,y.").unwrap();
        assert_eq!(receipt.result, r#"{"x": "20/7", "y": "19/7"}"#);
        assert!(replay_linear_system(&receipt));
        let variant =
            execute_linear_system("Given 2*x-y=3 and x+3*y=11, find the ordered pair x,y.")
                .unwrap();
        assert_eq!(variant.result, r#"{"x": "20/7", "y": "19/7"}"#);
    }
}
