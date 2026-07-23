//! Immutable-release governance for genuinely third-party evaluation corpora.
//!
//! This module deliberately separates source provenance and scope annotation
//! from the hand-authored external-style corpora.  A release can be evaluated
//! only after its source manifest, split, and independently supplied oracle
//! labels validate.  No acquisition or annotation is inferred by the Machine.

use crate::external_decomposition_benchmark::{
    evaluate as evaluate_external, CorpusSplit, ExpectedOutcome, ExternalCase, ExternalCorpus,
    ExternalReport,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReleaseKind {
    Fixture,
    ThirdParty,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ScopeLabel {
    InScope,
    UnderstandableUnsupported,
    Ambiguous,
    OutsideScope,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SourceRecord {
    pub source_id: String,
    pub citation: String,
    pub locator: String,
    pub license: String,
    pub retrieved_at: String,
    pub hash_basis: String,
    pub content_sha256: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ThirdPartyCase {
    pub id: String,
    pub source_id: String,
    pub source_item_id: String,
    pub split: CorpusSplit,
    pub original_prompt: String,
    pub scope: ScopeLabel,
    pub expected_outcome: ExpectedOutcome,
    pub expected_signature: Option<String>,
    #[serde(default)]
    pub expected_result: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ThirdPartyCorpus {
    pub schema_version: u32,
    pub release_id: String,
    pub release_kind: ReleaseKind,
    pub oracle: String,
    pub holdout_locked: bool,
    pub sources: Vec<SourceRecord>,
    pub cases: Vec<ThirdPartyCase>,
}

impl ThirdPartyCorpus {
    pub fn validation_errors(&self) -> Vec<String> {
        let mut errors = Vec::new();
        if self.schema_version != 1 {
            errors.push(format!("unsupported_schema:{}", self.schema_version));
        }
        if self.release_id.trim().is_empty() {
            errors.push("empty_release_id".into());
        }
        if self.oracle.trim().is_empty() {
            errors.push("empty_oracle".into());
        }
        if !self.holdout_locked {
            errors.push("holdout_not_locked".into());
        }
        if self.release_kind == ReleaseKind::ThirdParty && self.sources.is_empty() {
            errors.push("third_party_release_without_sources".into());
        }
        let mut source_ids = BTreeSet::new();
        for source in &self.sources {
            if !source_ids.insert(source.source_id.clone()) {
                errors.push(format!("duplicate_source:{}", source.source_id));
            }
            for (field, value) in [
                ("citation", &source.citation),
                ("locator", &source.locator),
                ("license", &source.license),
                ("retrieved_at", &source.retrieved_at),
                ("hash_basis", &source.hash_basis),
                ("content_sha256", &source.content_sha256),
            ] {
                if value.trim().is_empty() {
                    errors.push(format!("empty_source_{}_{}", field, source.source_id));
                }
            }
            if source.content_sha256.len() != 64
                || !source
                    .content_sha256
                    .chars()
                    .all(|ch| ch.is_ascii_hexdigit())
            {
                errors.push(format!("invalid_source_hash:{}", source.source_id));
            }
        }
        let mut case_ids = BTreeSet::new();
        let mut splits = BTreeSet::new();
        for case in &self.cases {
            if !case_ids.insert(case.id.clone()) {
                errors.push(format!("duplicate_case:{}", case.id));
            }
            if !source_ids.contains(&case.source_id) {
                errors.push(format!("unknown_source:{}", case.id));
            }
            if case.source_item_id.trim().is_empty() {
                errors.push(format!("empty_source_item:{}", case.id));
            }
            if case.original_prompt.trim().is_empty() {
                errors.push(format!("empty_original_prompt:{}", case.id));
            }
            splits.insert(case.split);
            let expected_scope = match case.scope {
                ScopeLabel::InScope => ExpectedOutcome::Supported,
                ScopeLabel::Ambiguous => ExpectedOutcome::Ambiguous,
                ScopeLabel::UnderstandableUnsupported | ScopeLabel::OutsideScope => {
                    ExpectedOutcome::Unsupported
                }
            };
            if case.expected_outcome != expected_scope {
                errors.push(format!("scope_outcome_mismatch:{}", case.id));
            }
            match case.expected_outcome {
                ExpectedOutcome::Supported if case.expected_signature.is_none() => {
                    errors.push(format!("supported_case_missing_signature:{}", case.id));
                }
                ExpectedOutcome::Ambiguous | ExpectedOutcome::Unsupported
                    if case.expected_signature.is_some() =>
                {
                    errors.push(format!("negative_case_has_signature:{}", case.id));
                }
                _ => {}
            }
        }
        if !splits.contains(&CorpusSplit::Development) {
            errors.push("missing_development_split".into());
        }
        if !splits.contains(&CorpusSplit::Holdout) {
            errors.push("missing_holdout_split".into());
        }
        errors
    }

    /// Stable SHA-256 fingerprint of the complete release manifest and cases.
    pub fn release_hash(&self) -> String {
        let bytes = serde_json::to_vec(self).expect("third-party corpus serializes");
        let digest = Sha256::digest(bytes);
        format!("{digest:x}")
    }

    fn as_external(&self) -> ExternalCorpus {
        ExternalCorpus {
            schema_version: 1,
            oracle: self.oracle.clone(),
            holdout_locked: self.holdout_locked,
            cases: self
                .cases
                .iter()
                .map(|case| ExternalCase {
                    id: case.id.clone(),
                    source: case.source_id.clone(),
                    split: case.split,
                    prompt: case.original_prompt.clone(),
                    expected_outcome: case.expected_outcome,
                    expected_signature: case.expected_signature.clone(),
                    expected_result: case.expected_result.clone(),
                })
                .collect(),
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct ThirdPartyReport {
    pub release_id: String,
    pub release_hash: String,
    pub release_kind: ReleaseKind,
    pub evaluation: ExternalReport,
    /// Diagnostic clusters for cases expected to be unsupported.  These are
    /// evidence-collection labels, not authorization decisions or inferred
    /// capabilities.
    pub rejection_clusters: BTreeMap<String, usize>,
    /// Case-level diagnostic labels, keyed by immutable corpus case id.
    pub rejection_reasons: BTreeMap<String, String>,
}

/// Assign a stable, conservative research cluster to an unsupported prose
/// problem.  This deliberately describes the missing capability family; it
/// does not attempt to solve the prompt or broaden `decompose`.
pub fn rejection_cluster(prompt: &str) -> &'static str {
    let text = prompt.to_ascii_lowercase();
    let has_word = |word: &str| {
        text.split(|ch: char| !ch.is_ascii_alphanumeric() && ch != '%')
            .any(|token| token == word)
    };
    if text.contains('%')
        || text.contains("percent")
        || text.contains("percentage")
        || text.contains("discount")
        || text.contains("interest")
        || has_word("fee")
        || has_word("fees")
    {
        return "percentage_discount_finance";
    }
    if text.contains("ratio")
        || text.contains("times as")
        || text.contains("twice")
        || text.contains("thrice")
        || text.contains("three times")
        || text.contains("four times")
        || text.contains("rate")
        || text.contains("speed")
        || text.contains("mph")
        || text.contains("per hour")
    {
        return "ratio_rate_proportion";
    }
    if text.contains("half")
        || text.contains("third")
        || text.contains("quarter")
        || text.contains("fraction")
        || text.contains("1/6")
        || text.contains("2/3")
        || text.contains("3/4")
        || text.contains("2/5")
    {
        return "fractional_quantity";
    }
    if text.contains("liter")
        || text.contains("gallon")
        || text.contains("pound")
        || text.contains("ounce")
        || text.contains("gram")
        || text.contains("meter")
        || text.contains("mile")
        || text.contains("feet")
        || text.contains("inch")
        || text.contains("dozen")
        || text.contains("cups")
        || text.contains("gb")
    {
        return "unit_measurement_conversion";
    }
    if text.contains("every day")
        || text.contains("each year")
        || text.contains("per day")
        || text.contains("per week")
        || text.contains("per month")
        || text.contains("over ")
        || text.contains("after ")
        || text.contains("remaining")
    {
        return "temporal_or_sequential_reasoning";
    }
    "multi_step_quantity_arithmetic"
}

pub fn evaluate(corpus: &ThirdPartyCorpus) -> ThirdPartyReport {
    let evaluation = evaluate_external(&corpus.as_external());
    let mut rejection_clusters = BTreeMap::new();
    let mut rejection_reasons = BTreeMap::new();
    for case in &corpus.cases {
        if case.expected_outcome == ExpectedOutcome::Unsupported {
            let cluster = rejection_cluster(&case.original_prompt).to_string();
            *rejection_clusters.entry(cluster.clone()).or_default() += 1;
            rejection_reasons.insert(case.id.clone(), cluster);
        }
    }
    ThirdPartyReport {
        release_id: corpus.release_id.clone(),
        release_hash: corpus.release_hash(),
        release_kind: corpus.release_kind,
        evaluation,
        rejection_clusters,
        rejection_reasons,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn source() -> SourceRecord {
        SourceRecord {
            source_id: "fixture".into(),
            citation: "Fixture only".into(),
            locator: "fixture://third-party-schema".into(),
            license: "not-evidence".into(),
            retrieved_at: "2026-07-23".into(),
            hash_basis: "fixture manifest".into(),
            content_sha256: "0".repeat(64),
        }
    }

    #[test]
    fn scope_labels_and_release_hash_are_validated() {
        let corpus = ThirdPartyCorpus {
            schema_version: 1,
            release_id: "fixture-v1".into(),
            release_kind: ReleaseKind::Fixture,
            oracle: "independent fixture oracle".into(),
            holdout_locked: true,
            sources: vec![source()],
            cases: vec![
                ThirdPartyCase {
                    id: "in-scope".into(),
                    source_id: "fixture".into(),
                    source_item_id: "item-1".into(),
                    split: CorpusSplit::Development,
                    original_prompt: "Compute 2 + 3".into(),
                    scope: ScopeLabel::InScope,
                    expected_outcome: ExpectedOutcome::Supported,
                    expected_signature: Some("None>Integer".into()),
                    expected_result: None,
                },
                ThirdPartyCase {
                    id: "ambiguous".into(),
                    source_id: "fixture".into(),
                    source_item_id: "item-2".into(),
                    split: CorpusSplit::Holdout,
                    original_prompt: "Either compute 2 + 3 or use another route".into(),
                    scope: ScopeLabel::Ambiguous,
                    expected_outcome: ExpectedOutcome::Ambiguous,
                    expected_signature: None,
                    expected_result: None,
                },
            ],
        };
        assert!(corpus.validation_errors().is_empty());
        assert_eq!(corpus.release_hash().len(), 64);
        let report = evaluate(&corpus);
        assert_eq!(report.evaluation.metrics.structural_correct, 2);
        assert_eq!(report.rejection_clusters, BTreeMap::new());
    }

    #[test]
    fn unsupported_clusters_are_diagnostic_and_deterministic() {
        assert_eq!(
            rejection_cluster("What percentage is 20% of the total?"),
            "percentage_discount_finance"
        );
        assert_eq!(
            rejection_cluster("A train travels at 40 mph for two hours."),
            "ratio_rate_proportion"
        );
        assert_eq!(
            rejection_cluster("What is half of 24 liters?"),
            "fractional_quantity"
        );
        assert_eq!(
            rejection_cluster("How many gallons remain?"),
            "unit_measurement_conversion"
        );
        assert_eq!(
            rejection_cluster("She works at the coffee shop every day."),
            "temporal_or_sequential_reasoning"
        );
    }
}
