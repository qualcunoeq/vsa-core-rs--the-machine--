//! Verified normalization for one explicit symbolic equation.
//!
//! Normalization moves both sides to a canonical zero-form. It is deliberately
//! separate from expression simplification because the equality relation is
//! part of the artifact contract.

use crate::algebra;
use serde::Serialize;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub enum EquationNormalizationFailure {
    MissingEquality,
    ParseRejected,
    ReplayVerificationFailed,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct EquationNormalizationReceipt {
    pub source: String,
    pub normalized_equation: String,
    pub replay_verified: bool,
}

pub fn execute_equation_normalization(
    source: &str,
) -> Result<EquationNormalizationReceipt, EquationNormalizationFailure> {
    if !source.contains('=') {
        return Err(EquationNormalizationFailure::MissingEquality);
    }
    let (lhs, rhs) =
        algebra::parse_equation(source).map_err(|_| EquationNormalizationFailure::ParseRejected)?;
    let difference = (lhs - rhs).canonicalize();
    let normalized_equation = format!("{} = 0", difference);
    let receipt = EquationNormalizationReceipt {
        source: source.trim().to_string(),
        normalized_equation,
        replay_verified: false,
    };
    if !replay_equation_normalization(&receipt) {
        return Err(EquationNormalizationFailure::ReplayVerificationFailed);
    }
    Ok(EquationNormalizationReceipt {
        replay_verified: true,
        ..receipt
    })
}

pub fn replay_equation_normalization(receipt: &EquationNormalizationReceipt) -> bool {
    let Ok((source_lhs, source_rhs)) = algebra::parse_equation(&receipt.source) else {
        return false;
    };
    let Ok((normalized_lhs, normalized_rhs)) =
        algebra::parse_equation(&receipt.normalized_equation)
    else {
        return false;
    };
    algebra::equivalent(
        &(source_lhs - source_rhs),
        &(normalized_lhs - normalized_rhs),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn equation_normalizes_to_zero_form_and_replays() {
        let receipt = execute_equation_normalization("2*x + 3 = 7").unwrap();
        assert!(receipt.normalized_equation.contains("= 0"));
        assert!(receipt.replay_verified);
        assert!(replay_equation_normalization(&receipt));
    }

    #[test]
    fn expression_without_equality_is_rejected() {
        assert_eq!(
            execute_equation_normalization("2*x + 3"),
            Err(EquationNormalizationFailure::MissingEquality)
        );
    }

    #[test]
    fn malformed_equation_is_rejected() {
        assert!(matches!(
            execute_equation_normalization("2*x + = 7"),
            Err(EquationNormalizationFailure::ParseRejected)
        ));
    }
}
