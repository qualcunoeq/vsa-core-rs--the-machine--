//! PercentageQuantityV1: narrow, non-authorizing percentage formalization.
//!
//! This module implements exactly the four target forms from the
//! V1 implementation contract.  It does not solve the relation,
//! infer missing quantities, or mutate any registry.  A later
//! governed planner may hand an accepted artifact to an algebra
//! executor after independently validating the artifact and its replay.
//!
//! Supported forms:
//!   - PercentageOf:            part = rate × base
//!   - IncreaseByPercentage:    final = base × (1 + rate)
//!   - DecreaseByPercentage:    final = base × (1 - rate)
//!   - RecoverBase:             base = final / (1 ± rate)
//!
//! Excluded (even when surface resembles a linear percentage):
//!   compound growth, interest, percentage points, overlapping
//!   adjustments, probability, ambiguous reference quantities.

use crate::quantity_relation::{QuantityConstraint, QuantityRelationArtifact};
use crate::quantity_relation_integration::{
    bridge_to_algebra as bridge_quantity_to_algebra, AlgebraBridgeReceipt,
};
use regex::Regex;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PercentageDecisionKind {
    Accepted,
    Ambiguous,
    Unsupported,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PercentageQuantityArtifact {
    /// One of: percentage_of, increase_by_percentage,
    /// decrease_by_percentage, recover_base
    pub operation_kind: String,
    /// The percentage rate as entered (e.g. 20 for 20%)
    pub rate: u64,
    /// The reference/base quantity
    pub base_quantity: u64,
    /// Optional target quantity (present for PercentageOf, Increase,
    /// Decrease; absent for RecoverBase where it's given)
    pub target_quantity: Option<u64>,
    /// Increase or Decrease (None for PercentageOf)
    pub direction: Option<String>,
    /// True for all V1 operations (single-step only)
    pub single_step: bool,
    /// Canonical typed signature
    pub signature: String,
    /// Human-readable target description
    pub target: String,
    /// Constraints for replay verification
    pub constraints: Vec<QuantityConstraint>,
    /// Expression for algebra handoff
    pub expression: String,
}

impl PercentageQuantityArtifact {
    /// Deterministic local replay of the typed artifact.  This verifies that
    /// the artifact still has the declared structure without executing it.
    ///
    /// For RecoverBase, `base_quantity` is 0 (the base is what we solve for)
    /// and `target_quantity` carries the given final value instead.  The
    /// replay gate accepts either valid base or valid target.
    pub fn replay_verified(&self) -> bool {
        let base_or_target_ok = match self.operation_kind.as_str() {
            "recover_base" => self.target_quantity.is_some_and(|t| t > 0),
            _ => self.base_quantity > 0,
        };
        self.rate > 0
            && base_or_target_ok
            && self.single_step
            && !self.operation_kind.is_empty()
            && !self.signature.is_empty()
            && !self.target.is_empty()
            && !self.expression.is_empty()
            && !self.constraints.is_empty()
            && self
                .constraints
                .iter()
                .all(|c| !c.lhs.is_empty() && !c.rhs.is_empty())
    }

    /// Convert to a QuantityRelationArtifact for bridge compatibility.
    pub fn to_quantity_relation(&self) -> QuantityRelationArtifact {
        QuantityRelationArtifact {
            family: format!("percentage_quantity:{}", self.operation_kind),
            signature: self.signature.clone(),
            target: self.target.clone(),
            constraints: self.constraints.clone(),
            algebra_expression: Some(self.expression.clone()),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum PercentageQuantityDecision {
    Accepted(PercentageQuantityArtifact),
    Ambiguous,
    Unsupported,
}

impl PercentageQuantityDecision {
    pub fn kind(&self) -> PercentageDecisionKind {
        match self {
            Self::Accepted(_) => PercentageDecisionKind::Accepted,
            Self::Ambiguous => PercentageDecisionKind::Ambiguous,
            Self::Unsupported => PercentageDecisionKind::Unsupported,
        }
    }
}

fn accepted(
    operation_kind: &str,
    rate: u64,
    base_quantity: u64,
    target_quantity: Option<u64>,
    direction: Option<&str>,
    target: String,
    expression: String,
    constraint_lhs: String,
    constraint_rhs: String,
) -> PercentageQuantityDecision {
    let signature = match operation_kind {
        "percentage_of" => format!("[rate:{rate}%,base:{base_quantity}]>part>percentage_of"),
        "increase_by_percentage" => {
            format!("[rate:{rate}%,base:{base_quantity}]>final>increase_by_percentage")
        }
        "decrease_by_percentage" => {
            format!("[rate:{rate}%,base:{base_quantity}]>final>decrease_by_percentage")
        }
        "recover_base" => format!(
            "[rate:{rate}%,final:{},direction:{}]>base>recover_base",
            target_quantity.map_or("?".into(), |v| v.to_string()),
            direction.unwrap_or("?")
        ),
        _ => format!("[rate:{rate}%,base:{base_quantity}]>target>percentage"),
    };
    PercentageQuantityDecision::Accepted(PercentageQuantityArtifact {
        operation_kind: operation_kind.into(),
        rate,
        base_quantity,
        target_quantity,
        direction: direction.map(String::from),
        single_step: true,
        signature,
        target,
        constraints: vec![QuantityConstraint {
            lhs: constraint_lhs,
            rhs: constraint_rhs,
        }],
        expression,
    })
}

/// Check for out-of-scope patterns that must be rejected regardless
/// of surface similarity to linear percentage relations.
fn unsupported_guard(text: &str) -> Option<PercentageQuantityDecision> {
    // Compound growth indicators
    if text.contains("each year")
        || text.contains("compound")
        || (text.contains("grows") && text.contains("each"))
        || text.contains("every year")
    {
        return Some(PercentageQuantityDecision::Unsupported);
    }
    // Finance-specific
    if text.contains("interest") || text.contains("loan charges") {
        return Some(PercentageQuantityDecision::Unsupported);
    }
    // Percentage points (not same as percentage)
    if text.contains("percentage points") || text.contains("percentage point") {
        return Some(PercentageQuantityDecision::Unsupported);
    }
    // Overlapping adjustments (two sequential operations)
    if text.contains("followed by") && (text.contains("discount") || text.contains("tax")) {
        return Some(PercentageQuantityDecision::Unsupported);
    }
    // Probability
    if text.contains("probability") || text.contains("chance") {
        return Some(PercentageQuantityDecision::Unsupported);
    }
    None
}

/// Check for ambiguous patterns where the surface mentions a percentage
/// but omits the reference base, direction, or target interpretation.
fn ambiguous_guard(text: &str) -> Option<PercentageQuantityDecision> {
    // "What is X% more than the amount?" — "the amount" is a vague
    // placeholder, not an explicit numeric base.
    if Regex::new(r"what is \d+% more than the amount")
        .ok()
        .is_some_and(|re| re.is_match(text))
    {
        return Some(PercentageQuantityDecision::Ambiguous);
    }
    // "The price decreased by 20%. What is it now?" — no explicit base.
    if Regex::new(r"price decreased by \d+%")
        .ok()
        .is_some_and(|re| re.is_match(text))
        && !text.contains("priced at")
        && !text.contains("base price")
        && !text.contains("base value")
    {
        return Some(PercentageQuantityDecision::Ambiguous);
    }
    // "What is 30% of the total? The total is not specified."
    if text.contains("not specified") || text.contains("unknown") {
        return Some(PercentageQuantityDecision::Ambiguous);
    }
    // "The value changed to 20%. Determine the result."
    if text.contains("changed to") {
        return Some(PercentageQuantityDecision::Ambiguous);
    }
    // "A percentage change is mentioned, but the original value and
    // direction are unknown."
    if text.contains("original value and direction are unknown") {
        return Some(PercentageQuantityDecision::Ambiguous);
    }
    None
}

/// Formalize percentage quantity statements into typed artifacts.
///
/// Returns `Accepted(artifact)` only for the four supported target forms.
/// Returns `Ambiguous` when the surface mentions a percentage but omits
/// essential information.  Returns `Unsupported` for out-of-scope
/// patterns (compound, finance, points, probability, etc.).
pub fn formalize(prompt: &str) -> PercentageQuantityDecision {
    let text = prompt
        .to_ascii_lowercase()
        .replace(['\n', '\r'], " ")
        .replace('$', "");
    let text = text.trim();

    // 1. Unsupported guard — reject out-of-scope patterns first.
    if let Some(decision) = unsupported_guard(text) {
        return decision;
    }

    // 2. Ambiguous guard — reject missing-information patterns.
    if let Some(decision) = ambiguous_guard(text) {
        return decision;
    }

    // 3a. PercentageOf: "What is {rate}% of {whole}?"
    let pct_of_what = Regex::new(r"^what is (\d+)% of (\d+)\??\.?$").unwrap();
    if let Some(caps) = pct_of_what.captures(text) {
        let rate: u64 = caps[1].parse().unwrap();
        let whole: u64 = caps[2].parse().unwrap();
        return accepted(
            "percentage_of",
            rate,
            whole,
            None,
            None,
            "percentage_part".into(),
            format!("{whole} * {rate} / 100"),
            format!("part = {rate} / 100 * {whole}"),
            format!("rate = {rate}%, whole = {whole}"),
        );
    }

    // 3b. PercentageOf: "Calculate {rate} percent of the whole quantity {whole}."
    let pct_calc = Regex::new(r"^calculate (\d+) percent of the whole quantity (\d+)\.?$").unwrap();
    if let Some(caps) = pct_calc.captures(text) {
        let rate: u64 = caps[1].parse().unwrap();
        let whole: u64 = caps[2].parse().unwrap();
        return accepted(
            "percentage_of",
            rate,
            whole,
            None,
            None,
            "percentage_part".into(),
            format!("{whole} * {rate} / 100"),
            format!("part = {rate} / 100 * {whole}"),
            format!("rate = {rate}%, whole = {whole}"),
        );
    }

    // 3c. PercentageOf: "Find {rate}% of {whole}."
    let pct_find = Regex::new(r"^find (\d+)% of (\d+)\.?$").unwrap();
    if let Some(caps) = pct_find.captures(text) {
        let rate: u64 = caps[1].parse().unwrap();
        let whole: u64 = caps[2].parse().unwrap();
        return accepted(
            "percentage_of",
            rate,
            whole,
            None,
            None,
            "percentage_part".into(),
            format!("{whole} * {rate} / 100"),
            format!("part = {rate} / 100 * {whole}"),
            format!("rate = {rate}%, whole = {whole}"),
        );
    }

    // 4a. DecreaseByPercentage: "An item priced at ${base} receives a
    //     {rate}% discount. What is the final price?"
    let discount1 = Regex::new(
        r"^an item priced at (\d+) receives a (\d+)% discount\. what is the final price\??\.?$",
    )
    .unwrap();
    if let Some(caps) = discount1.captures(text) {
        let base: u64 = caps[1].parse().unwrap();
        let rate: u64 = caps[2].parse().unwrap();
        return accepted(
            "decrease_by_percentage",
            rate,
            base,
            None,
            Some("decrease"),
            "discounted_price".into(),
            format!("{base} * (100 - {rate}) / 100"),
            format!("final = {base} * (1 - {rate}/100)"),
            format!("rate = {rate}%, base = {base}"),
        );
    }

    // 4b. DecreaseByPercentage: "Apply a {rate} percent reduction to a
    //     base price of {base} dollars; find the final price."
    let discount2 = Regex::new(
        r"^apply a (\d+) percent reduction to a base price of (\d+) dollars; find the final price\.?$",
    )
    .unwrap();
    if let Some(caps) = discount2.captures(text) {
        let rate: u64 = caps[1].parse().unwrap();
        let base: u64 = caps[2].parse().unwrap();
        return accepted(
            "decrease_by_percentage",
            rate,
            base,
            None,
            Some("decrease"),
            "discounted_price".into(),
            format!("{base} * (100 - {rate}) / 100"),
            format!("final = {base} * (1 - {rate}/100)"),
            format!("rate = {rate}%, base = {base}"),
        );
    }

    // 5. IncreaseByPercentage: "A quantity with base value {base}
    //    increases by {rate}%. What is the final value after this one
    //    change?"
    let increase = Regex::new(
        r"^a quantity with base value (\d+) increases by (\d+)%\. what is the final value after this one change\??\.?$",
    )
    .unwrap();
    if let Some(caps) = increase.captures(text) {
        let base: u64 = caps[1].parse().unwrap();
        let rate: u64 = caps[2].parse().unwrap();
        return accepted(
            "increase_by_percentage",
            rate,
            base,
            None,
            Some("increase"),
            "increased_value".into(),
            format!("{base} * (100 + {rate}) / 100"),
            format!("final = {base} * (1 + {rate}/100)"),
            format!("rate = {rate}%, base = {base}"),
        );
    }

    // 6a. RecoverBase (increase): "After a {rate}% increase, the new
    //     value is {final}. What was the original?"
    //     Expression: final * 100 / (100 + rate)
    //     which simplifies to: final * 100 / combined  (combined = 100 + rate)
    let recover1 = Regex::new(
        r"^after a (\d+)% increase, the new value is (\d+)\. what was the original\??\.?$",
    )
    .unwrap();
    if let Some(caps) = recover1.captures(text) {
        let rate: u64 = caps[1].parse().unwrap();
        let final_val: u64 = caps[2].parse().unwrap();
        let combined = 100 + rate;
        return accepted(
            "recover_base",
            rate,
            0, // base is unknown, will be computed
            Some(final_val),
            Some("increase"),
            "original_value".into(),
            format!("{final_val} * 100 / {combined}"),
            format!("original = {final_val} / (1 + {rate}/100)"),
            format!("rate = {rate}%, final = {final_val}, direction = increase"),
        );
    }

    // 6b. RecoverBase (decrease): "After a {rate}% reduction, the
    //     discounted price is {final}. Find the original price."
    //     Expression: final * 100 / (100 - rate)
    //     which simplifies to: final * 100 / combined  (combined = 100 - rate)
    let recover2 = Regex::new(
        r"^after a (\d+)% reduction, the (?:discounted price|new value) is (\d+)\. find the original(?: price)?\??\.?$",
    )
    .unwrap();
    if let Some(caps) = recover2.captures(text) {
        let rate: u64 = caps[1].parse().unwrap();
        let final_val: u64 = caps[2].parse().unwrap();
        if rate >= 100 {
            return PercentageQuantityDecision::Unsupported;
        }
        let combined = 100 - rate;
        return accepted(
            "recover_base",
            rate,
            0,
            Some(final_val),
            Some("decrease"),
            "original_value".into(),
            format!("{final_val} * 100 / {combined}"),
            format!("original = {final_val} / (1 - {rate}/100)"),
            format!("rate = {rate}%, final = {final_val}, direction = decrease"),
        );
    }

    // 6c. RecoverBase (discount): "The final price is {final} after a
    //     {rate}% discount. What was the original?"
    let recover3 = Regex::new(
        r"^the final price is (\d+) after a (\d+)% discount\. what was the original\??\.?$",
    )
    .unwrap();
    if let Some(caps) = recover3.captures(text) {
        let final_val: u64 = caps[1].parse().unwrap();
        let rate: u64 = caps[2].parse().unwrap();
        if rate >= 100 {
            return PercentageQuantityDecision::Unsupported;
        }
        let combined = 100 - rate;
        return accepted(
            "recover_base",
            rate,
            0,
            Some(final_val),
            Some("decrease"),
            "original_price".into(),
            format!("{final_val} * 100 / {combined}"),
            format!("original = {final_val} / (1 - {rate}/100)"),
            format!("rate = {rate}%, final = {final_val}, direction = decrease"),
        );
    }

    // No supported pattern matched.  If the prompt contains a percentage
    // mention, treat as ambiguous (unknown form).  Otherwise unsupported.
    if text.contains('%') || text.contains("percent") {
        PercentageQuantityDecision::Ambiguous
    } else {
        PercentageQuantityDecision::Unsupported
    }
}

/// Bridge a percentage artifact through the quantity-relation algebra
/// executor.  Both the artifact and the downstream algebra receipt are
/// independently replay-verified.
pub fn bridge_to_algebra(artifact: &PercentageQuantityArtifact) -> Option<AlgebraBridgeReceipt> {
    if !artifact.replay_verified() {
        return None;
    }
    bridge_quantity_to_algebra(&artifact.to_quantity_relation())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::percentage_quantity_proposal::{corpus, PercentageScope};

    // ── Supported form tests ──────────────────────────────────────────

    fn assert_accepted(prompt: &str, expected_kind: &str) -> PercentageQuantityArtifact {
        let decision = formalize(prompt);
        let PercentageQuantityDecision::Accepted(artifact) = decision else {
            panic!(
                "expected Accepted({expected_kind}), got {:?} for: {prompt}",
                decision.kind()
            );
        };
        assert_eq!(
            artifact.operation_kind, expected_kind,
            "expected operation_kind {expected_kind}, got {} for: {prompt}",
            artifact.operation_kind
        );
        assert!(
            artifact.replay_verified(),
            "replay_verified() failed for: {prompt}"
        );
        artifact
    }

    #[test]
    fn accepts_percentage_of_what_is() {
        let a = assert_accepted("What is 20% of 50?", "percentage_of");
        assert_eq!(a.rate, 20);
        assert_eq!(a.base_quantity, 50);
        assert!(a.single_step);
    }

    #[test]
    fn accepts_percentage_of_calculate() {
        let a = assert_accepted(
            "Calculate 15 percent of the whole quantity 80.",
            "percentage_of",
        );
        assert_eq!(a.rate, 15);
        assert_eq!(a.base_quantity, 80);
    }

    #[test]
    fn accepts_percentage_of_find() {
        let a = assert_accepted("Find 30% of 60.", "percentage_of");
        assert_eq!(a.rate, 30);
        assert_eq!(a.base_quantity, 60);
    }

    #[test]
    fn accepts_discount_priced_at() {
        let a = assert_accepted(
            "An item priced at $80 receives a 20% discount. What is the final price?",
            "decrease_by_percentage",
        );
        assert_eq!(a.rate, 20);
        assert_eq!(a.base_quantity, 80);
        assert_eq!(a.direction.as_deref(), Some("decrease"));
    }

    #[test]
    fn accepts_discount_reduction_apply() {
        let a = assert_accepted(
            "Apply a 15 percent reduction to a base price of 100 dollars; find the final price.",
            "decrease_by_percentage",
        );
        assert_eq!(a.rate, 15);
        assert_eq!(a.base_quantity, 100);
    }

    #[test]
    fn accepts_increase_by_percentage() {
        let a = assert_accepted(
            "A quantity with base value 50 increases by 10%. What is the final value after this one change?",
            "increase_by_percentage",
        );
        assert_eq!(a.rate, 10);
        assert_eq!(a.base_quantity, 50);
        assert_eq!(a.direction.as_deref(), Some("increase"));
    }

    #[test]
    fn accepts_recover_base_after_increase() {
        let a = assert_accepted(
            "After a 20% increase, the new value is 120. What was the original?",
            "recover_base",
        );
        assert_eq!(a.rate, 20);
        assert_eq!(a.target_quantity, Some(120));
        assert_eq!(a.direction.as_deref(), Some("increase"));
    }

    #[test]
    fn accepts_recover_base_after_reduction() {
        let a = assert_accepted(
            "After a 25% reduction, the discounted price is 75. Find the original price.",
            "recover_base",
        );
        assert_eq!(a.rate, 25);
        assert_eq!(a.target_quantity, Some(75));
        assert_eq!(a.direction.as_deref(), Some("decrease"));
    }

    #[test]
    fn accepts_recover_base_after_discount() {
        let a = assert_accepted(
            "The final price is 60 after a 20% discount. What was the original?",
            "recover_base",
        );
        assert_eq!(a.rate, 20);
        assert_eq!(a.target_quantity, Some(60));
        assert_eq!(a.direction.as_deref(), Some("decrease"));
    }

    // ── Bridge tests ──────────────────────────────────────────────────

    #[test]
    fn bridge_percentage_of_to_algebra() {
        let a = assert_accepted("What is 20% of 50?", "percentage_of");
        let receipt = bridge_to_algebra(&a).expect("bridge_to_algebra");
        assert_eq!(receipt.result, "10");
        assert!(receipt.algebra_replay_verified);
    }

    #[test]
    fn bridge_discount_to_algebra() {
        let a = assert_accepted(
            "An item priced at $80 receives a 20% discount. What is the final price?",
            "decrease_by_percentage",
        );
        let receipt = bridge_to_algebra(&a).expect("bridge_to_algebra");
        assert_eq!(receipt.result, "64");
        assert!(receipt.algebra_replay_verified);
    }

    #[test]
    fn bridge_increase_to_algebra() {
        let a = assert_accepted(
            "A quantity with base value 50 increases by 10%. What is the final value after this one change?",
            "increase_by_percentage",
        );
        let receipt = bridge_to_algebra(&a).expect("bridge_to_algebra");
        assert_eq!(receipt.result, "55");
        assert!(receipt.algebra_replay_verified);
    }

    #[test]
    fn bridge_recover_increase_to_algebra() {
        let a = assert_accepted(
            "After a 20% increase, the new value is 120. What was the original?",
            "recover_base",
        );
        let receipt = bridge_to_algebra(&a).expect("bridge_to_algebra");
        assert_eq!(receipt.result, "100");
        assert!(receipt.algebra_replay_verified);
    }

    #[test]
    fn bridge_recover_reduction_to_algebra() {
        let a = assert_accepted(
            "After a 25% reduction, the discounted price is 75. Find the original price.",
            "recover_base",
        );
        let receipt = bridge_to_algebra(&a).expect("bridge_to_algebra");
        assert_eq!(receipt.result, "100");
        assert!(receipt.algebra_replay_verified);
    }

    // ── Ambiguous rejection tests ─────────────────────────────────────

    fn assert_ambiguous(prompt: &str) {
        let decision = formalize(prompt);
        assert!(
            matches!(decision, PercentageQuantityDecision::Ambiguous),
            "expected Ambiguous, got {:?} for: {prompt}",
            decision.kind()
        );
    }

    #[test]
    fn refuses_missing_base_amount() {
        assert_ambiguous("What is 20% more than the amount?");
    }

    #[test]
    fn refuses_missing_base_decrease() {
        assert_ambiguous("The price decreased by 20%. What is it now?");
    }

    #[test]
    fn refuses_unspecified_total() {
        assert_ambiguous("What is 30% of the total? The total is not specified.");
    }

    #[test]
    fn refuses_ambiguous_direction() {
        assert_ambiguous("The value changed to 20%. Determine the result.");
    }

    #[test]
    fn refuses_unknown_original() {
        assert_ambiguous(
            "A percentage change is mentioned, but the original value and direction are unknown.",
        );
    }

    // ── Unsupported rejection tests ───────────────────────────────────

    fn assert_unsupported(prompt: &str) {
        let decision = formalize(prompt);
        assert!(
            matches!(decision, PercentageQuantityDecision::Unsupported),
            "expected Unsupported, got {:?} for: {prompt}",
            decision.kind()
        );
    }

    #[test]
    fn refuses_compound_growth() {
        assert_unsupported(
            "A balance grows by 5% each year for 5 years. What is the final balance?",
        );
    }

    #[test]
    fn refuses_compound_growth_variant() {
        assert_unsupported(
            "A balance grows by 5% each year for 3 years. What is the final balance?",
        );
    }

    #[test]
    fn refuses_finance_interest() {
        assert_unsupported(
            "A loan charges 3% simple interest over time; calculate the finance cost.",
        );
    }

    #[test]
    fn refuses_percentage_points() {
        assert_unsupported("A rate rises by 2 percentage points. What is the new rate?");
    }

    #[test]
    fn refuses_overlapping_adjustments() {
        assert_unsupported("Apply a 20% discount followed by 10% tax; determine the final price.");
    }

    #[test]
    fn refuses_probability() {
        assert_unsupported("There is a 25% probability that an unknown variable succeeds.");
    }

    // ── Contract corpus ablation test ─────────────────────────────────

    #[test]
    fn contract_corpus_all_supported_accepted() {
        let c = corpus();
        for case in &c.cases {
            if case.scope != PercentageScope::Supported {
                continue;
            }
            let decision = formalize(&case.prompt);
            let expected_kind = case.family.as_deref().unwrap_or("percentage");
            match &decision {
                PercentageQuantityDecision::Accepted(artifact) => {
                    assert!(
                        artifact.replay_verified(),
                        "replay_verified failed for supported: {}",
                        case.id
                    );
                    // Verify the operation kind matches the family
                    let family_ok = match expected_kind {
                        "percentage_of" => artifact.operation_kind == "percentage_of",
                        "single_step_change" => {
                            artifact.operation_kind == "increase_by_percentage"
                                || artifact.operation_kind == "decrease_by_percentage"
                        }
                        _ => true,
                    };
                    assert!(
                        family_ok,
                        "case {} (family={}) got operation_kind={}",
                        case.id, expected_kind, artifact.operation_kind
                    );
                }
                other => {
                    panic!(
                        "supported case {} ({:?}) got {:?}: {}",
                        case.id,
                        case.scope,
                        other.kind(),
                        case.prompt
                    );
                }
            }
        }
    }

    #[test]
    fn contract_corpus_all_ambiguous_rejected() {
        let c = corpus();
        for case in &c.cases {
            if case.scope != PercentageScope::Ambiguous {
                continue;
            }
            let decision = formalize(&case.prompt);
            assert!(
                matches!(decision, PercentageQuantityDecision::Ambiguous),
                "ambiguous case {} expected Ambiguous, got {:?}: {}",
                case.id,
                decision.kind(),
                case.prompt
            );
        }
    }

    #[test]
    fn contract_corpus_all_unsupported_rejected() {
        let c = corpus();
        for case in &c.cases {
            if case.scope != PercentageScope::Unsupported {
                continue;
            }
            let decision = formalize(&case.prompt);
            assert!(
                matches!(decision, PercentageQuantityDecision::Unsupported),
                "unsupported case {} expected Unsupported, got {:?}: {}",
                case.id,
                decision.kind(),
                case.prompt
            );
        }
    }

    #[test]
    fn contract_corpus_all_rewrite_pairs_have_consistent_operation_kind() {
        let c = corpus();
        let mut pairs: std::collections::BTreeMap<String, Vec<String>> =
            std::collections::BTreeMap::new();
        for case in &c.cases {
            if let Some(pair_id) = &case.pair_id {
                pairs
                    .entry(pair_id.clone())
                    .or_default()
                    .push(case.prompt.clone());
            }
        }
        for (pair_id, prompts) in &pairs {
            assert_eq!(
                prompts.len(),
                2,
                "rewrite pair {pair_id} has {} prompts",
                prompts.len()
            );
            let kinds: Vec<_> = prompts
                .iter()
                .map(|p| {
                    let d = formalize(p);
                    match &d {
                        PercentageQuantityDecision::Accepted(a) => a.operation_kind.clone(),
                        other => format!("{:?}", other.kind()),
                    }
                })
                .collect();
            assert_eq!(
                kinds[0], kinds[1],
                "rewrite pair {pair_id} has inconsistent operation kinds: {:?} vs {:?}",
                kinds[0], kinds[1]
            );
        }
    }

    #[test]
    fn contract_corpus_deterministic() {
        for _ in 0..5 {
            assert_eq!(
                formalize("What is 20% of 50?").kind() as u8,
                PercentageDecisionKind::Accepted as u8
            );
            assert_eq!(
                formalize("What is 30% of the total? The total is not specified.").kind() as u8,
                PercentageDecisionKind::Ambiguous as u8
            );
            assert_eq!(
                formalize("A balance grows by 5% each year for 5 years. What is the final balance?")
                    .kind() as u8,
                PercentageDecisionKind::Unsupported as u8
            );
        }
    }

    #[test]
    fn bridge_replay_deterministic() {
        let a = assert_accepted("What is 20% of 50?", "percentage_of");
        let r1 = bridge_to_algebra(&a).expect("first bridge");
        let r2 = bridge_to_algebra(&a).expect("second bridge");
        assert_eq!(r1.result, r2.result);
        assert_eq!(r1.algebra_replay_verified, r2.algebra_replay_verified);
    }

    #[test]
    fn tampered_artifact_fails_replay() {
        let mut a = PercentageQuantityArtifact {
            operation_kind: "percentage_of".into(),
            rate: 20,
            base_quantity: 50,
            target_quantity: None,
            direction: None,
            single_step: true,
            signature: "[rate:20%,base:50]>part>percentage_of".into(),
            target: "percentage_part".into(),
            constraints: vec![QuantityConstraint {
                lhs: "part = 20 / 100 * 50".into(),
                rhs: "rate = 20%, whole = 50".into(),
            }],
            expression: "50 * 20 / 100".into(),
        };
        assert!(a.replay_verified());

        // Tamper: zero rate
        a.rate = 0;
        assert!(!a.replay_verified());
        a.rate = 20;

        // Tamper: zero base
        a.base_quantity = 0;
        assert!(!a.replay_verified());
        a.base_quantity = 50;

        // Tamper: single_step = false
        a.single_step = false;
        assert!(!a.replay_verified());
        a.single_step = true;

        // Tamper: empty expression
        a.expression.clear();
        assert!(!a.replay_verified());
    }
}
