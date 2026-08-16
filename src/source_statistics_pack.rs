//! Source-derived finite-statistics catalog.
//!
//! The subject methods are declarative records from an attributed source;
//! execution is delegated to the generic source-formula runtime. This module
//! contains no formula-specific evaluator branch.

use crate::source_formula_pack::{
    evaluate_formula_records, Expr, FormulaRecord, FormulaRequest, FormulaResult, InputConstraint,
    SourceCitation,
};

pub const DOMAIN: &str = "source_derived_finite_statistics";

fn citation() -> SourceCitation {
    SourceCitation {
        source_id: "openstax-introductory-statistics-2e:descriptive-statistics".into(),
        title: "Introductory Statistics 2e".into(),
        section: "Measures of Central Tendency and Variability".into(),
        url: "https://openstax.org/details/books/introductory-statistics-2e".into(),
        license: "CC BY 4.0; OpenStax attribution required".into(),
        retrieved_utc: "2026-08-16".into(),
    }
}

fn input(name: &str) -> Expr {
    Expr::Input(name.into())
}

/// Return the immutable source records for the finite statistics catalog.
pub fn records() -> Vec<FormulaRecord> {
    let source = citation();
    vec![
        FormulaRecord {
            formula_id: "arithmetic_mean".into(),
            aliases: vec!["sample mean".into(), "mean from sum and count".into()],
            expression: Expr::Div(Box::new(input("sum")), Box::new(input("count"))),
            required_inputs: vec!["sum".into(), "count".into()],
            assumptions: vec!["count is positive".into(), "values share one scale".into()],
            constraints: vec![InputConstraint::Positive("count".into())],
            source: source.clone(),
        },
        FormulaRecord {
            formula_id: "weighted_mean".into(),
            aliases: vec!["weighted average".into()],
            expression: Expr::Div(
                Box::new(input("weighted_sum")),
                Box::new(input("total_weight")),
            ),
            required_inputs: vec!["weighted_sum".into(), "total_weight".into()],
            assumptions: vec!["total weight is positive".into(), "weights are declared".into()],
            constraints: vec![InputConstraint::Positive("total_weight".into())],
            source: source.clone(),
        },
        FormulaRecord {
            formula_id: "bernoulli_variance".into(),
            aliases: vec!["binary-outcome variance".into()],
            expression: Expr::Mul(
                Box::new(input("p")),
                Box::new(Expr::Sub(Box::new(Expr::Constant(1)), Box::new(input("p")))),
            ),
            required_inputs: vec!["p".into()],
            assumptions: vec!["p is a probability in [0,1]".into()],
            constraints: vec![InputConstraint::Probability("p".into())],
            source: source.clone(),
        },
        FormulaRecord {
            formula_id: "binomial_expected_value".into(),
            aliases: vec!["binomial mean".into()],
            expression: Expr::Mul(Box::new(input("n")), Box::new(input("p"))),
            required_inputs: vec!["n".into(), "p".into()],
            assumptions: vec!["n is a nonnegative integer; p is a probability".into()],
            constraints: vec![
                InputConstraint::NonnegativeInteger("n".into()),
                InputConstraint::Probability("p".into()),
            ],
            source: source.clone(),
        },
        FormulaRecord {
            formula_id: "binomial_variance".into(),
            aliases: vec!["binomial variance".into()],
            expression: Expr::Mul(
                Box::new(Expr::Mul(Box::new(input("n")), Box::new(input("p")))),
                Box::new(Expr::Sub(Box::new(Expr::Constant(1)), Box::new(input("p")))),
            ),
            required_inputs: vec!["n".into(), "p".into()],
            assumptions: vec!["n is a nonnegative integer; p is a probability".into()],
            constraints: vec![
                InputConstraint::NonnegativeInteger("n".into()),
                InputConstraint::Probability("p".into()),
            ],
            source,
        },
    ]
}

/// Execute a source-derived finite-statistics request through the generic
/// catalog interpreter.
pub fn evaluate_statistics(request: &FormulaRequest) -> FormulaResult {
    evaluate_formula_records(request, DOMAIN, &records())
}
