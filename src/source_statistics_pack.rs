//! Source-derived finite-statistics catalog.
//!
//! The subject methods are declarative records from an attributed source;
//! execution is delegated to the generic source-formula runtime. This module
//! contains no formula-specific evaluator branch.

use crate::source_formula_pack::{
    evaluate_formula_records, extract_formula_records, FormulaRecord, FormulaRequest, FormulaResult,
};

#[path = "source_statistics_frontend.rs"]
pub mod source_statistics_frontend;

pub const DOMAIN: &str = "source_derived_finite_statistics";

/// Return the immutable source records for the finite statistics catalog.
pub fn records() -> Vec<FormulaRecord> {
    extract_formula_records(include_str!(
        "../docs/sources/openstax_finite_statistics_source.txt"
    ))
    .expect("source-derived statistics source document extracts and validates")
}

/// Execute a source-derived finite-statistics request through the generic
/// catalog interpreter.
pub fn evaluate_statistics(request: &FormulaRequest) -> FormulaResult {
    evaluate_formula_records(request, DOMAIN, &records())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::probability_pack::Rational;
    use std::collections::BTreeMap;

    fn q(numerator: i128, denominator: i128) -> Rational {
        Rational::new(numerator, denominator).unwrap()
    }

    fn request(formula: &str) -> FormulaRequest {
        FormulaRequest {
            formula: formula.into(),
            inputs: BTreeMap::from([
                ("sum".into(), q(30, 1)),
                ("count".into(), q(5, 1)),
                ("p".into(), q(1, 4)),
                ("n".into(), q(8, 1)),
            ]),
            domain: DOMAIN.into(),
            ambiguity: None,
            provenance: vec!["unit-test".into()],
        }
    }

    #[test]
    fn source_constraints_are_enforced_and_replayable() {
        let valid = evaluate_statistics(&request("arithmetic_mean"));
        assert_eq!(
            valid.status,
            crate::source_formula_pack::FormulaStatus::Complete
        );
        assert_eq!(valid.value, Some(q(6, 1)));
        assert!(valid.replay_verified());
        let mut invalid = request("bernoulli_variance");
        invalid.inputs.insert("p".into(), q(5, 4));
        let invalid = evaluate_statistics(&invalid);
        assert_eq!(
            invalid.status,
            crate::source_formula_pack::FormulaStatus::Inconsistent
        );
        assert!(invalid.replay_verified());
    }
}
