//! Explicit visual-table to bounded DNA composition and probability bridge.
//!
//! A table of base counts is not a DNA observation merely because its labels
//! look biological.  This bridge requires an exact `base,count` header, all
//! four DNA bases exactly once, bounded non-negative integer counts, and an
//! explicit sampling policy before it can become a probability artifact.

use crate::source_formula_pack::biology_pack::{
    biology_probability_bridge::{
        bridge_base_composition, BiologyProbabilityBridgeResult, BiologyProbabilityBridgeStatus,
    },
    evaluate_biology, BiologyOperation, BiologyRequest, BiologyResult, BiologyStatus,
};
use crate::vision::visual_table::TableArtifact;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum BridgeStatus {
    Complete,
    Ambiguous,
    Unsupported,
    Missing,
    Inconsistent,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct VisualBiologyResult {
    pub status: BridgeStatus,
    pub table: Option<TableArtifact>,
    pub biology: Option<BiologyResult>,
    pub probability: Option<BiologyProbabilityBridgeResult>,
    pub reasons: Vec<String>,
    pub provenance: Vec<String>,
    pub replay_hash: String,
}

fn digest<T: Serialize>(value: &T) -> String {
    format!(
        "{:x}",
        Sha256::digest(serde_json::to_vec(value).expect("visual biology serializes"))
    )
}

fn payload(result: &VisualBiologyResult) -> impl Serialize + '_ {
    (
        result.status,
        &result.table,
        &result.biology,
        &result.probability,
        &result.reasons,
        &result.provenance,
    )
}

fn output(
    status: BridgeStatus,
    table: Option<TableArtifact>,
    biology: Option<BiologyResult>,
    probability: Option<BiologyProbabilityBridgeResult>,
    reasons: Vec<String>,
) -> VisualBiologyResult {
    let provenance = table
        .as_ref()
        .map(|value| value.provenance_spans.clone())
        .unwrap_or_default();
    let mut result = VisualBiologyResult {
        status,
        table,
        biology,
        probability,
        reasons,
        provenance,
        replay_hash: String::new(),
    };
    let replay_hash = digest(&payload(&result));
    result.replay_hash = replay_hash;
    result
}

fn parse_count(text: &str) -> Option<u32> {
    let text = text.trim();
    if text.is_empty() || text.starts_with('-') {
        return None;
    }
    text.parse().ok()
}

fn classify_biology(status: BiologyStatus) -> BridgeStatus {
    match status {
        BiologyStatus::Complete => BridgeStatus::Complete,
        BiologyStatus::Ambiguous => BridgeStatus::Ambiguous,
        BiologyStatus::Missing => BridgeStatus::Missing,
        BiologyStatus::Unsupported | BiologyStatus::InvalidDomain => BridgeStatus::Unsupported,
        BiologyStatus::Inconsistent => BridgeStatus::Inconsistent,
    }
}

/// Lower an explicitly labelled base-count table to bounded DNA artifacts.
///
/// The canonical sequence used internally is only a deterministic carrier for
/// the validated composition operation; no ordering claim is made about the
/// original sample.  A probability handoff additionally requires the explicit
/// `uniform_position` sampling policy.
pub fn table_to_biology_probability(
    table: &TableArtifact,
    sampling_policy: Option<&str>,
) -> VisualBiologyResult {
    if table.row_count < 2 || table.column_count != 2 || table.rows.len() != table.row_count {
        return output(
            BridgeStatus::Unsupported,
            Some(table.clone()),
            None,
            None,
            vec!["DNA composition requires a two-column table with data rows".into()],
        );
    }
    if table.cells.len() != table.row_count * table.column_count
        || table.provenance_spans.is_empty()
    {
        return output(
            BridgeStatus::Missing,
            Some(table.clone()),
            None,
            None,
            vec!["coordinate cells and provenance are required".into()],
        );
    }
    let header = table.rows[0]
        .iter()
        .map(|value| value.trim().to_ascii_lowercase())
        .collect::<Vec<_>>();
    if header != ["base", "count"] {
        let joined = header.join(" ");
        let status = if joined.contains("rna") || joined.contains("amino") {
            BridgeStatus::Unsupported
        } else {
            BridgeStatus::Ambiguous
        };
        return output(
            status,
            Some(table.clone()),
            None,
            None,
            vec!["headers do not uniquely establish DNA base-count semantics".into()],
        );
    }

    let mut counts = BTreeMap::new();
    for row in table.rows.iter().skip(1) {
        if row.len() != 2 || row[0].trim().is_empty() {
            return output(
                BridgeStatus::Missing,
                Some(table.clone()),
                None,
                None,
                vec!["every DNA row needs one base label and one count".into()],
            );
        }
        let base = row[0].trim().to_ascii_uppercase();
        if !matches!(base.as_str(), "A" | "C" | "G" | "T") {
            return output(
                BridgeStatus::Unsupported,
                Some(table.clone()),
                None,
                None,
                vec!["only the bounded DNA alphabet A, C, G, T is supported".into()],
            );
        }
        let Some(count) = parse_count(&row[1]) else {
            return output(
                BridgeStatus::Inconsistent,
                Some(table.clone()),
                None,
                None,
                vec!["base counts must be non-negative integers".into()],
            );
        };
        if counts.insert(base, count).is_some() {
            return output(
                BridgeStatus::Ambiguous,
                Some(table.clone()),
                None,
                None,
                vec!["duplicate base labels create multiple count bindings".into()],
            );
        }
    }
    if counts.len() != 4
        || ["A", "C", "G", "T"]
            .iter()
            .any(|base| !counts.contains_key(*base))
    {
        return output(
            BridgeStatus::Missing,
            Some(table.clone()),
            None,
            None,
            vec!["all four DNA bases must be present exactly once".into()],
        );
    }
    let total: u32 = counts.values().copied().sum();
    if total == 0 || total > 256 {
        return output(
            BridgeStatus::Unsupported,
            Some(table.clone()),
            None,
            None,
            vec!["DNA composition length must be between one and 256".into()],
        );
    }
    let mut sequence = String::with_capacity(total as usize);
    for base in ["A", "C", "G", "T"] {
        sequence.push_str(&base.repeat(counts[base] as usize));
    }
    let biology = evaluate_biology(&BiologyRequest {
        operation: BiologyOperation::BaseComposition,
        sequence: Some(sequence),
        orientation: None,
        domain: "source_derived_bounded_dna".into(),
        ambiguity: None,
        provenance: table.provenance_spans.clone(),
    });
    if biology.status != BiologyStatus::Complete || !biology.replay_verified() {
        return output(
            classify_biology(biology.status),
            Some(table.clone()),
            Some(biology),
            None,
            vec!["base-count table did not produce a replayable biology artifact".into()],
        );
    }
    let probability = bridge_base_composition(&biology, sampling_policy);
    let status = match probability.status {
        BiologyProbabilityBridgeStatus::Complete => BridgeStatus::Complete,
        BiologyProbabilityBridgeStatus::Ambiguous => BridgeStatus::Ambiguous,
        BiologyProbabilityBridgeStatus::Missing => BridgeStatus::Missing,
        BiologyProbabilityBridgeStatus::Unsupported => BridgeStatus::Unsupported,
    };
    output(
        status,
        Some(table.clone()),
        Some(biology),
        Some(probability),
        Vec::new(),
    )
}

impl VisualBiologyResult {
    pub fn replay_verified(&self) -> bool {
        self.replay_hash == digest(&payload(self))
            && !self.provenance.is_empty()
            && self
                .biology
                .as_ref()
                .is_none_or(BiologyResult::replay_verified)
            && self
                .probability
                .as_ref()
                .is_none_or(BiologyProbabilityBridgeResult::replay_verified)
    }

    pub fn authorized(&self) -> bool {
        self.status == BridgeStatus::Complete
            && self.replay_verified()
            && self
                .biology
                .as_ref()
                .is_some_and(|biology| biology.status == BiologyStatus::Complete)
            && self
                .probability
                .as_ref()
                .is_some_and(BiologyProbabilityBridgeResult::authorized)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::vision::visual_table::TableCell;

    fn table(rows: Vec<Vec<&str>>) -> TableArtifact {
        let rows = rows
            .into_iter()
            .map(|row| row.into_iter().map(str::to_owned).collect::<Vec<_>>())
            .collect::<Vec<_>>();
        let cells = rows
            .iter()
            .enumerate()
            .flat_map(|(row, values)| {
                values
                    .iter()
                    .enumerate()
                    .map(move |(column, text)| TableCell {
                        text: text.clone(),
                        row,
                        column,
                        left: (column * 40) as u32,
                        top: (row * 20) as u32,
                        width: 30,
                        height: 10,
                    })
            })
            .collect();
        TableArtifact {
            row_count: rows.len(),
            column_count: 2,
            cells,
            provenance_spans: vec!["test".into()],
            rows,
        }
    }

    #[test]
    fn explicit_counts_need_uniform_sampling_policy() {
        let table = table(vec![
            vec!["base", "count"],
            vec!["A", "2"],
            vec!["C", "2"],
            vec!["G", "2"],
            vec!["T", "2"],
        ]);
        let complete = table_to_biology_probability(&table, Some("uniform_position"));
        assert!(complete.authorized());
        let ambiguous = table_to_biology_probability(&table, None);
        assert_eq!(ambiguous.status, BridgeStatus::Ambiguous);
        assert!(!ambiguous.authorized());
    }

    #[test]
    fn duplicate_or_rna_labels_fail_closed() {
        let duplicate = table(vec![vec!["base", "count"], vec!["A", "2"], vec!["A", "2"]]);
        assert_eq!(
            table_to_biology_probability(&duplicate, Some("uniform_position")).status,
            BridgeStatus::Ambiguous
        );
        let rna = table(vec![vec!["rna", "count"], vec!["A", "2"], vec!["U", "2"]]);
        assert_eq!(
            table_to_biology_probability(&rna, Some("uniform_position")).status,
            BridgeStatus::Unsupported
        );
    }
}
