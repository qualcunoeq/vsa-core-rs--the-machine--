//! Explicit visual-table to finite-probability bridge.
//!
//! Only a table whose first row is exactly `outcome` and `probability` is
//! eligible. The bridge never treats a numeric-looking second column as a
//! probability without that semantic header.

use crate::probability_pack::{
    evaluate_probability, ProbabilityArtifact, ProbabilityOperation, ProbabilityRequest,
    ProbabilityResult, ProbabilityStatus, Rational,
};
use crate::vision::visual_table::TableArtifact;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

pub const DOMAIN: &str = "visual_table_to_finite_probability";

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum BridgeStatus {
    Complete,
    Missing,
    Ambiguous,
    Unsupported,
    InvalidProbability,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TableProbabilityResult {
    pub status: BridgeStatus,
    pub table: Option<TableArtifact>,
    pub probability: Option<ProbabilityResult>,
    pub reasons: Vec<String>,
    pub provenance: Vec<String>,
    pub replay_hash: String,
}

fn digest<T: Serialize>(value: &T) -> String {
    format!(
        "{:x}",
        Sha256::digest(serde_json::to_vec(value).expect("visual probability serializes"))
    )
}

fn payload(result: &TableProbabilityResult) -> impl Serialize + '_ {
    (
        result.status,
        &result.table,
        &result.probability,
        &result.reasons,
        &result.provenance,
    )
}

fn result(
    status: BridgeStatus,
    table: Option<TableArtifact>,
    probability: Option<ProbabilityResult>,
    reasons: Vec<String>,
) -> TableProbabilityResult {
    let provenance = table
        .as_ref()
        .map(|value| value.provenance_spans.clone())
        .unwrap_or_default();
    let mut output = TableProbabilityResult {
        status,
        table,
        probability,
        reasons,
        provenance,
        replay_hash: String::new(),
    };
    let replay_hash = digest(&payload(&output));
    output.replay_hash = replay_hash;
    output
}

fn parse_rational(text: &str) -> Option<Rational> {
    let text = text.trim();
    if let Some((numerator, denominator)) = text.split_once('/') {
        return Rational::new(numerator.parse().ok()?, denominator.parse().ok()?);
    }
    Rational::new(text.parse().ok()?, 1)
}

/// Convert an explicitly labelled visual table to a finite distribution.
pub fn table_to_probability(table: &TableArtifact) -> TableProbabilityResult {
    if table.row_count < 2 || table.column_count != 2 || table.rows.len() != table.row_count {
        return result(
            BridgeStatus::Unsupported,
            Some(table.clone()),
            None,
            vec!["finite probability requires a two-column table with data rows".into()],
        );
    }
    if table.cells.len() != table.row_count * table.column_count
        || table.provenance_spans.is_empty()
    {
        return result(
            BridgeStatus::Missing,
            Some(table.clone()),
            None,
            vec!["complete coordinate and provenance cells are required".into()],
        );
    }
    let header = table.rows[0]
        .iter()
        .map(|value| value.trim().to_ascii_lowercase())
        .collect::<Vec<_>>();
    if header != ["outcome", "probability"] {
        let lower = header.join(" ");
        if lower.contains("density") || lower.contains("continuous") {
            return result(
                BridgeStatus::Unsupported,
                Some(table.clone()),
                None,
                vec!["continuous density semantics are outside finite probability".into()],
            );
        }
        return result(
            BridgeStatus::Ambiguous,
            Some(table.clone()),
            None,
            vec!["table headers do not uniquely establish outcome/probability semantics".into()],
        );
    }
    let mut outcomes = Vec::new();
    let mut probabilities = Vec::new();
    for row in table.rows.iter().skip(1) {
        if row.len() != 2 || row[0].trim().is_empty() {
            return result(
                BridgeStatus::Missing,
                Some(table.clone()),
                None,
                vec!["each probability row needs one named outcome and one exact value".into()],
            );
        }
        let Some(probability) = parse_rational(&row[1]) else {
            return result(
                BridgeStatus::InvalidProbability,
                Some(table.clone()),
                None,
                vec!["probability cells must be exact integers or rationals".into()],
            );
        };
        outcomes.push(row[0].clone());
        probabilities.push(probability);
    }
    let probability = evaluate_probability(&ProbabilityRequest {
        operation: ProbabilityOperation::DistributionConstruction,
        domain: "finite_exact_probability".into(),
        outcomes,
        probabilities,
        values: Vec::new(),
        event_a: None,
        event_b: None,
        partition: Vec::new(),
        conditional_values: Vec::new(),
        prior_probability: None,
        likelihood: None,
        evidence: None,
        ambiguity: None,
        provenance: table.provenance_spans.clone(),
    });
    let status = match probability.status {
        ProbabilityStatus::Complete => BridgeStatus::Complete,
        ProbabilityStatus::InvalidProbability => BridgeStatus::InvalidProbability,
        ProbabilityStatus::Missing => BridgeStatus::Missing,
        ProbabilityStatus::Ambiguous => BridgeStatus::Ambiguous,
        _ => BridgeStatus::Unsupported,
    };
    result(status, Some(table.clone()), Some(probability), Vec::new())
}

impl TableProbabilityResult {
    pub fn replay_verified(&self) -> bool {
        self.replay_hash == digest(&payload(self))
            && !self.provenance.is_empty()
            && self.table.as_ref().is_some_and(|table| table.row_count > 1)
            && self
                .probability
                .as_ref()
                .is_none_or(ProbabilityResult::replay_verified)
    }

    pub fn authorized(&self) -> bool {
        self.status == BridgeStatus::Complete
            && self.replay_verified()
            && self.probability.as_ref().is_some_and(|probability| {
                probability.status == ProbabilityStatus::Complete
                    && probability.artifact.as_ref().is_some_and(|artifact| {
                        matches!(artifact, ProbabilityArtifact::Distribution(_))
                    })
            })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn table(headers: [&str; 2], values: [(&str, &str); 2]) -> TableArtifact {
        let rows: Vec<Vec<String>> = vec![
            headers.iter().map(|value| (*value).into()).collect(),
            vec![values[0].0.into(), values[0].1.into()],
            vec![values[1].0.into(), values[1].1.into()],
        ];
        let cells = rows
            .iter()
            .enumerate()
            .flat_map(|(row, values)| {
                values.iter().enumerate().map(move |(column, text)| {
                    crate::vision::visual_table::TableCell {
                        text: text.clone(),
                        row,
                        column,
                        left: (column * 30) as u32,
                        top: (row * 20) as u32,
                        width: 20,
                        height: 10,
                    }
                })
            })
            .collect();
        TableArtifact {
            rows,
            cells,
            row_count: 3,
            column_count: 2,
            provenance_spans: vec!["table-test".into()],
        }
    }

    #[test]
    fn explicitly_labelled_table_becomes_distribution() {
        let output = table_to_probability(&table(
            ["outcome", "probability"],
            [("a", "1/2"), ("b", "1/2")],
        ));
        assert!(output.authorized());
        assert!(output.replay_verified());
    }

    #[test]
    fn unknown_header_and_bad_sum_fail_closed() {
        let output =
            table_to_probability(&table(["value", "weight"], [("a", "1/2"), ("b", "1/2")]));
        assert_eq!(output.status, BridgeStatus::Ambiguous);
        let output = table_to_probability(&table(
            ["outcome", "probability"],
            [("a", "1/3"), ("b", "1/3")],
        ));
        assert_eq!(output.status, BridgeStatus::InvalidProbability);
    }
}
