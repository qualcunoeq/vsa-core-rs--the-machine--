//! Source-derived finite mean-update execution through the generic formula runtime.

use crate::source_formula_pack::{evaluate_formula_records, FormulaRequest, FormulaResult};
use crate::source_mean_update_frontend::{records, DOMAIN};

pub fn evaluate(request: &FormulaRequest) -> FormulaResult {
    evaluate_formula_records(request, DOMAIN, &records())
}
