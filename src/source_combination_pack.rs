//! Source-derived bounded combination execution wrapper.

use crate::source_counting_pack::{evaluate, CountingRequest, CountingResult};

pub const DOMAIN: &str = "source_derived_bounded_combination";

pub fn evaluate_combination(request: &CountingRequest) -> CountingResult {
    evaluate(request)
}
