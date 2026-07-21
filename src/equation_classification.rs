//! Verified classification of normalized equations for solver routing.
//!
//! Classification is deliberately narrower than solving: it reports the
//! single-variable polynomial degree supported by the algebra island, while
//! refusing unsupported syntax or multiple variables.

use crate::algebra::{self, SymExpr};
use serde::Serialize;
use std::collections::BTreeSet;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum EquationClass {
    Linear,
    Quadratic,
    Unsupported,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub enum EquationClassificationFailure {
    MissingEquality,
    ParseRejected,
    MultipleVariables,
    UnsupportedSyntax,
    ReplayVerificationFailed,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct EquationClassificationReceipt {
    pub source: String,
    pub variable: Option<String>,
    pub class: EquationClass,
    pub replay_verified: bool,
}

pub fn execute_equation_classification(
    source: &str,
) -> Result<EquationClassificationReceipt, EquationClassificationFailure> {
    if !source.contains('=') {
        return Err(EquationClassificationFailure::MissingEquality);
    }
    let (lhs, rhs) = algebra::parse_equation(source)
        .map_err(|_| EquationClassificationFailure::ParseRejected)?;
    let mut variables = BTreeSet::new();
    collect_variables(&lhs, &mut variables);
    collect_variables(&rhs, &mut variables);
    if variables.len() > 1 {
        return Err(EquationClassificationFailure::MultipleVariables);
    }
    let variable = variables.into_iter().next();
    let class = classify_pair(&lhs, &rhs, variable.as_deref())?;
    let receipt = EquationClassificationReceipt {
        source: source.trim().to_string(),
        variable,
        class,
        replay_verified: false,
    };
    if !replay_equation_classification(&receipt) {
        return Err(EquationClassificationFailure::ReplayVerificationFailed);
    }
    Ok(EquationClassificationReceipt {
        replay_verified: true,
        ..receipt
    })
}

pub fn replay_equation_classification(receipt: &EquationClassificationReceipt) -> bool {
    let Ok((lhs, rhs)) = algebra::parse_equation(&receipt.source) else {
        return false;
    };
    let mut variables = BTreeSet::new();
    collect_variables(&lhs, &mut variables);
    collect_variables(&rhs, &mut variables);
    if variables.len() > 1 || variables.into_iter().next() != receipt.variable {
        return false;
    }
    classify_pair(&lhs, &rhs, receipt.variable.as_deref())
        .map(|class| class == receipt.class)
        .unwrap_or(false)
}

fn classify_pair(
    lhs: &SymExpr,
    rhs: &SymExpr,
    variable: Option<&str>,
) -> Result<EquationClass, EquationClassificationFailure> {
    let degree = match variable {
        Some(variable) => polynomial_degree(&((lhs.clone() - rhs.clone()).canonicalize()), variable),
        None => {
            if supported_nodes(lhs) && supported_nodes(rhs) {
                0
            } else {
                return Err(EquationClassificationFailure::UnsupportedSyntax);
            }
        }
    };
    Ok(match degree {
        0 | 1 => EquationClass::Linear,
        2 => EquationClass::Quadratic,
        _ => EquationClass::Unsupported,
    })
}

fn collect_variables(expr: &SymExpr, variables: &mut BTreeSet<String>) {
    match expr {
        SymExpr::Var(variable) => {
            variables.insert(variable.display.to_string());
        }
        SymExpr::Add(a, b)
        | SymExpr::Sub(a, b)
        | SymExpr::Mul(a, b)
        | SymExpr::Div(a, b)
        | SymExpr::Pow(a, b) => {
            collect_variables(a, variables);
            collect_variables(b, variables);
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
        | SymExpr::Atan(a) => collect_variables(a, variables),
        SymExpr::Num(_) => {}
        SymExpr::Limit { approach, body, .. } => {
            collect_variables(approach, variables);
            collect_variables(body, variables);
        }
        SymExpr::Integral {
            lower,
            upper,
            body,
            ..
        } => {
            if let Some(lower) = lower {
                collect_variables(lower, variables);
            }
            if let Some(upper) = upper {
                collect_variables(upper, variables);
            }
            collect_variables(body, variables);
        }
    }
}

fn supported_nodes(expr: &SymExpr) -> bool {
    match expr {
        SymExpr::Num(_) | SymExpr::Var(_) => true,
        SymExpr::Add(a, b) | SymExpr::Sub(a, b) | SymExpr::Mul(a, b) => {
            supported_nodes(a) && supported_nodes(b)
        }
        SymExpr::Neg(a) => supported_nodes(a),
        SymExpr::Div(a, b) => {
            matches!(b.as_ref(), SymExpr::Num(n) if *n != 0.0) && supported_nodes(a)
        }
        SymExpr::Pow(base, exp) => {
            matches!(exp.as_ref(), SymExpr::Num(n) if *n >= 0.0 && n.fract() == 0.0)
                && supported_nodes(base)
        }
        _ => false,
    }
}

fn polynomial_degree(expr: &SymExpr, variable: &str) -> u32 {
    match expr {
        SymExpr::Num(_) => 0,
        SymExpr::Var(v) => u32::from(v.display.as_ref() == variable),
        SymExpr::Add(a, b) | SymExpr::Sub(a, b) => {
            polynomial_degree(a, variable).max(polynomial_degree(b, variable))
        }
        SymExpr::Mul(a, b) => polynomial_degree(a, variable).saturating_add(polynomial_degree(b, variable)),
        SymExpr::Neg(a) => polynomial_degree(a, variable),
        SymExpr::Div(a, _) => polynomial_degree(a, variable),
        SymExpr::Pow(base, exp) => match exp.as_ref() {
            SymExpr::Num(n) if *n >= 0.0 && n.fract() == 0.0 => {
                polynomial_degree(base, variable).saturating_mul(*n as u32)
            }
            _ => u32::MAX,
        },
        _ => u32::MAX,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classifies_linear_and_replays() {
        let receipt = execute_equation_classification("2*x + 3 = 7").unwrap();
        assert_eq!(receipt.class, EquationClass::Linear);
        assert_eq!(receipt.variable.as_deref(), Some("x"));
        assert!(receipt.replay_verified);
    }

    #[test]
    fn classifies_quadratic_and_replays() {
        let receipt = execute_equation_classification("x^2 - 4 = 0").unwrap();
        assert_eq!(receipt.class, EquationClass::Quadratic);
        assert!(replay_equation_classification(&receipt));
    }

    #[test]
    fn rejects_multiple_variables() {
        assert_eq!(
            execute_equation_classification("x + y = 2"),
            Err(EquationClassificationFailure::MultipleVariables)
        );
    }

    #[test]
    fn classifies_higher_degree_as_unsupported() {
        let receipt = execute_equation_classification("x^3 = 1").unwrap();
        assert_eq!(receipt.class, EquationClass::Unsupported);
    }
}
