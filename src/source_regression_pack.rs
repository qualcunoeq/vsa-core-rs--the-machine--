//! Source-derived bounded finite linear-regression diagnostics.
//!
//! This module contains no regression-specific evaluator.  Definitions and
//! assumptions are extracted from an attributed source transcription and are
//! executed by the generic rational formula catalog runtime.

use crate::source_formula_pack::{
    evaluate_formula_records, extract_formula_records, FormulaRecord, FormulaRequest, FormulaResult,
};

#[path = "source_regression_frontend.rs"]
pub mod source_regression_frontend;

pub const DOMAIN: &str = "source_derived_finite_regression";

/// Load the immutable source-derived regression catalog.
pub fn records() -> Vec<FormulaRecord> {
    extract_formula_records(include_str!(
        "../docs/sources/openstax_finite_regression_source.txt"
    ))
    .expect("source-derived regression source document extracts and validates")
}

/// Execute a regression relation through the generic source catalog runtime.
pub fn evaluate_regression(request: &FormulaRequest) -> FormulaResult {
    evaluate_formula_records(request, DOMAIN, &records())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::probability_pack::Rational;
    use std::collections::BTreeMap;

    fn q(numerator: i128, denominator: i128) -> Rational {
        Rational::new(numerator, denominator).expect("test rational")
    }

    #[test]
    fn source_catalog_is_generic_and_replayable() {
        let request = FormulaRequest {
            formula: "regression_slope".into(),
            inputs: BTreeMap::from([
                ("covariance_sum".into(), q(12, 1)),
                ("x_variance_sum".into(), q(4, 1)),
            ]),
            domain: DOMAIN.into(),
            ambiguity: None,
            provenance: vec!["source-regression-test".into()],
        };
        let result = evaluate_regression(&request);
        assert_eq!(result.value, Some(q(3, 1)));
        assert!(result.replay_verified());
        assert!(result.source.is_some());
    }

    #[test]
    fn source_constraints_refuse_degenerate_variation() {
        let request = FormulaRequest {
            formula: "regression_slope".into(),
            inputs: BTreeMap::from([
                ("covariance_sum".into(), q(12, 1)),
                ("x_variance_sum".into(), q(0, 1)),
            ]),
            domain: DOMAIN.into(),
            ambiguity: None,
            provenance: vec!["source-regression-test".into()],
        };
        let result = evaluate_regression(&request);
        assert_eq!(
            result.status,
            crate::source_formula_pack::FormulaStatus::Inconsistent
        );
        assert!(result.replay_verified());
    }
}
