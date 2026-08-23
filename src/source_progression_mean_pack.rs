//! Source-derived bounded arithmetic-progression mean pack.

use crate::source_formula_pack::{evaluate_formula_records, FormulaRequest, FormulaResult};
use crate::source_progression_mean_frontend::{records, DOMAIN};

pub fn evaluate(request: &FormulaRequest) -> FormulaResult {
    evaluate_formula_records(request, DOMAIN, &records())
}
