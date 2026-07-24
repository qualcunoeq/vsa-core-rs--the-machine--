//! Post-QuantityPlanner diagnostic labels for the frozen GSM8K release.
//!
//! These labels describe where the remaining external cases sit after the
//! quantity planner has been applied. They are evidence only: no label changes
//! authorization, routing, or capability registration.

use crate::external_decomposition_benchmark::ExpectedOutcome;
use crate::third_party_corpus_benchmark::rejection_cluster;

pub fn ambiguity_reason(prompt: &str, expected: ExpectedOutcome) -> &'static str {
    let text = prompt.to_ascii_lowercase();
    if text.contains("times more") || text.contains("twice as") {
        "comparative_multiplier_scope"
    } else if text.contains("half")
        || text.contains("third")
        || text.contains("quarter")
        || text.contains("1/6")
        || text.contains("2/3")
        || text.contains("3/4")
        || text.contains("2/5")
    {
        "fractional_scope_or_anchor"
    } else if text.contains("remaining") || text.contains("after ") {
        "temporal_reference_scope"
    } else if expected == ExpectedOutcome::Ambiguous {
        "source_oracle_ambiguity"
    } else {
        "unresolved_quantity_reference"
    }
}

pub fn residual_cluster(prompt: &str) -> &'static str {
    rejection_cluster(prompt)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ambiguity_labels_are_stable_and_conservative() {
        assert_eq!(
            ambiguity_reason("How many times more is this?", ExpectedOutcome::Ambiguous),
            "comparative_multiplier_scope"
        );
        assert_eq!(
            residual_cluster("What percentage is 20% of the total?"),
            "percentage_discount_finance"
        );
    }
}
