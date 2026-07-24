//! Pre-implementation contract corpus for PercentageQuantityV1.
//!
//! This module defines and validates a capability boundary only. It does not
//! parse production prose, execute percentage arithmetic, or alter routing.
//! The generated corpus is deterministic and deliberately separates shared
//! linear percentage semantics from finance and compound-growth cases.

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PercentageScope {
    Supported,
    Ambiguous,
    Unsupported,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PercentageCase {
    pub id: String,
    pub prompt: String,
    pub scope: PercentageScope,
    pub family: Option<String>,
    pub input_contract: Option<String>,
    pub output_contract: Option<String>,
    pub relation_schema: Option<String>,
    pub pair_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PercentageContractCorpus {
    pub schema_version: u32,
    pub release_id: String,
    pub oracle: String,
    pub cases: Vec<PercentageCase>,
}

impl PercentageContractCorpus {
    pub fn validation_errors(&self) -> Vec<String> {
        let mut errors = Vec::new();
        if self.schema_version != 1 {
            errors.push(format!("unsupported_schema:{}", self.schema_version));
        }
        let mut ids = BTreeSet::new();
        let mut pair_groups: BTreeMap<String, Vec<&PercentageCase>> = BTreeMap::new();
        let unsupported_markers = [
            "compound",
            "interest",
            "probability",
            "percentage points",
            "twice",
            "overlapping",
        ];
        for case in &self.cases {
            if !ids.insert(case.id.clone()) {
                errors.push(format!("duplicate_case:{}", case.id));
            }
            if case.prompt.trim().is_empty() {
                errors.push(format!("empty_prompt:{}", case.id));
            }
            match case.scope {
                PercentageScope::Supported => {
                    if case.family.is_none()
                        || case.input_contract.is_none()
                        || case.output_contract.is_none()
                        || case.relation_schema.is_none()
                    {
                        errors.push(format!("incomplete_supported_contract:{}", case.id));
                    }
                    let text = case.prompt.to_ascii_lowercase();
                    if unsupported_markers
                        .iter()
                        .any(|marker| text.contains(marker))
                    {
                        errors.push(format!("scope_leak_supported:{}", case.id));
                    }
                }
                PercentageScope::Ambiguous => {
                    if case.family.is_some() || case.relation_schema.is_some() {
                        errors.push(format!("ambiguous_case_has_contract:{}", case.id));
                    }
                }
                PercentageScope::Unsupported => {
                    if case.family.is_some() || case.relation_schema.is_some() {
                        errors.push(format!("unsupported_case_has_contract:{}", case.id));
                    }
                }
            }
            if let Some(pair_id) = &case.pair_id {
                pair_groups.entry(pair_id.clone()).or_default().push(case);
            }
        }
        for (pair_id, pair) in pair_groups {
            if pair.len() != 2 {
                errors.push(format!("rewrite_pair_size:{}:{}", pair_id, pair.len()));
                continue;
            }
            if pair
                .iter()
                .any(|case| case.scope != PercentageScope::Supported)
            {
                errors.push(format!("rewrite_pair_not_supported:{}", pair_id));
            }
            if pair[0].relation_schema != pair[1].relation_schema {
                errors.push(format!("rewrite_pair_relation_mismatch:{}", pair_id));
            }
        }
        errors
    }

    pub fn counts(&self) -> (usize, usize, usize) {
        let supported = self
            .cases
            .iter()
            .filter(|case| case.scope == PercentageScope::Supported)
            .count();
        let ambiguous = self
            .cases
            .iter()
            .filter(|case| case.scope == PercentageScope::Ambiguous)
            .count();
        let unsupported = self
            .cases
            .iter()
            .filter(|case| case.scope == PercentageScope::Unsupported)
            .count();
        (supported, ambiguous, unsupported)
    }

    pub fn rewrite_pairs(&self) -> usize {
        let mut pairs = BTreeSet::new();
        for case in &self.cases {
            if let Some(pair_id) = &case.pair_id {
                pairs.insert(pair_id.clone());
            }
        }
        pairs.len()
    }

    pub fn release_hash(&self) -> String {
        let bytes = serde_json::to_vec(self).expect("percentage corpus serializes");
        format!("{:x}", Sha256::digest(bytes))
    }
}

fn supported_case(
    id: String,
    prompt: String,
    family: &str,
    relation: &str,
    pair_id: Option<String>,
) -> PercentageCase {
    PercentageCase {
        id,
        prompt,
        scope: PercentageScope::Supported,
        family: Some(family.into()),
        input_contract: Some("explicit numeric whole/base and percentage rate".into()),
        output_contract: Some("typed linear quantity relation".into()),
        relation_schema: Some(relation.into()),
        pair_id,
    }
}

fn negative_case(id: String, prompt: String, scope: PercentageScope) -> PercentageCase {
    PercentageCase {
        id,
        prompt,
        scope,
        family: None,
        input_contract: None,
        output_contract: None,
        relation_schema: None,
        pair_id: None,
    }
}

pub fn corpus() -> PercentageContractCorpus {
    let mut cases = Vec::with_capacity(350);

    // 50 rewrite families, two surface forms each: 25 percentage-of and 25
    // single-step discount cases.
    for i in 0..25u32 {
        let rate = 10 + (i % 9) * 5;
        let whole = 40 + (i % 10) * 10;
        let pair = format!("percentage-of-rewrite-{i:02}");
        cases.push(supported_case(
            format!("pct-pair-{i:02}a"),
            format!("What is {rate}% of {whole}?"),
            "percentage_of",
            "part = (rate / 100) * whole",
            Some(pair.clone()),
        ));
        cases.push(supported_case(
            format!("pct-pair-{i:02}b"),
            format!("Calculate {rate} percent of the whole quantity {whole}."),
            "percentage_of",
            "part = (rate / 100) * whole",
            Some(pair),
        ));
    }
    for i in 0..25u32 {
        let rate = 5 + (i % 8) * 5;
        let base = 60 + (i % 10) * 10;
        let pair = format!("discount-rewrite-{i:02}");
        cases.push(supported_case(
            format!("pct-discount-{i:02}a"),
            format!(
                "An item priced at ${base} receives a {rate}% discount. What is the final price?"
            ),
            "single_step_change",
            "final = base * (1 - rate / 100)",
            Some(pair.clone()),
        ));
        cases.push(supported_case(
            format!("pct-discount-{i:02}b"),
            format!("Apply a {rate} percent reduction to a base price of {base} dollars; find the final price."),
            "single_step_change",
            "final = base * (1 - rate / 100)",
            Some(pair),
        ));
    }

    // 100 additional supported cases: explicit percentage-of and one-step
    // increase/markup with an unambiguous reference base.
    for i in 0..50u32 {
        let rate = 10 + (i % 9) * 5;
        let whole = 30 + (i % 20) * 5;
        cases.push(supported_case(
            format!("pct-of-{i:03}"),
            format!("Find {rate}% of {whole}."),
            "percentage_of",
            "part = (rate / 100) * whole",
            None,
        ));
    }
    for i in 0..50u32 {
        let rate = 5 + (i % 8) * 5;
        let base = 50 + (i % 20) * 5;
        cases.push(supported_case(
            format!("pct-increase-{i:03}"),
            format!("A quantity with base value {base} increases by {rate}%. What is the final value after this one change?"),
            "single_step_change",
            "final = base * (1 + rate / 100)",
            None,
        ));
    }

    // 50 ambiguous cases: the surface mentions a percentage but omits the
    // reference base, direction, or target interpretation.
    for i in 0..50u32 {
        let prompt = match i % 5 {
            0 => format!("What is {}% more than the amount?", 20 + i % 30),
            1 => "The price decreased by 20%. What is it now?".into(),
            2 => "What is 30% of the total? The total is not specified.".into(),
            3 => "The value changed to 20%. Determine the result.".into(),
            _ => "A percentage change is mentioned, but the original value and direction are unknown.".into(),
        };
        cases.push(negative_case(
            format!("pct-ambiguous-{i:03}"),
            prompt,
            PercentageScope::Ambiguous,
        ));
    }

    // 100 unsupported/adversarial cases. These deliberately remain outside
    // V1 even when their surface resembles a linear percentage relation.
    for i in 0..100u32 {
        let (family, prompt) = match i % 5 {
            0 => (
                "compound_growth",
                format!(
                    "A balance grows by 5% each year for {} years. What is the final balance?",
                    2 + i % 10
                ),
            ),
            1 => (
                "finance_interest",
                format!(
                    "A loan charges {}% simple interest over time; calculate the finance cost.",
                    3 + i % 8
                ),
            ),
            2 => (
                "percentage_points",
                format!(
                    "A rate rises by {} percentage points. What is the new rate?",
                    2 + i % 7
                ),
            ),
            3 => (
                "overlapping_adjustments",
                "Apply a 20% discount followed by 10% tax; determine the final price.".into(),
            ),
            _ => (
                "probability_or_symbolic",
                "There is a 25% probability that an unknown variable succeeds.".into(),
            ),
        };
        let _ = family;
        cases.push(negative_case(
            format!("pct-unsupported-{i:03}"),
            prompt,
            PercentageScope::Unsupported,
        ));
    }

    PercentageContractCorpus {
        schema_version: 1,
        release_id: "percentage-quantity-v1-contract-corpus".into(),
        oracle: "deterministic pre-implementation contract oracle; no execution".into(),
        cases,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn contract_corpus_has_requested_shape() {
        let corpus = corpus();
        assert_eq!(corpus.counts(), (200, 50, 100));
        assert_eq!(corpus.rewrite_pairs(), 50);
        assert!(corpus.validation_errors().is_empty());
    }

    #[test]
    fn contract_generation_is_deterministic() {
        assert_eq!(corpus(), corpus());
        assert_eq!(corpus().release_hash(), corpus().release_hash());
    }
}
