//! Source-derived finite Bayes catalog.  The formula remains declarative;
//! execution is delegated to the generic source-formula interpreter.

use crate::source_formula_pack::{
    evaluate_formula_records, extract_formula_records, FormulaRecord, FormulaRequest, FormulaResult,
};

pub const DOMAIN: &str = "source_catalog_bayes_rule";

pub fn records() -> Vec<FormulaRecord> {
    extract_formula_records(include_str!(
        "../docs/sources/openstax_bayes_rule_catalog.txt"
    ))
    .expect("source-derived Bayes catalog extracts and validates")
}

pub fn evaluate(request: &FormulaRequest) -> FormulaResult {
    evaluate_formula_records(request, DOMAIN, &records())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::probability_pack::Rational;
    use std::collections::BTreeMap;

    #[test]
    fn evaluates_cited_bayes_formula() {
        let request = FormulaRequest {
            formula: "bayes_posterior".into(),
            inputs: BTreeMap::from([
                ("prior".into(), Rational::new(3, 100).unwrap()),
                ("likelihood".into(), Rational::new(3, 4).unwrap()),
                ("evidence".into(), Rational::new(1, 5).unwrap()),
            ]),
            domain: DOMAIN.into(),
            ambiguity: None,
            provenance: vec!["test".into()],
        };
        let result = evaluate(&request);
        assert_eq!(
            result.status,
            crate::source_formula_pack::FormulaStatus::Complete
        );
        assert_eq!(result.value, Some(Rational::new(9, 80).unwrap()));
        assert!(result.replay_verified());
    }
}
