//! Explicit visual-table to source-derived finite-statistics bridge.
//!
//! A table is not a statistic merely because it contains numbers.  This bridge
//! requires an exact `quantity,value` header and explicit source-catalog input
//! labels before delegating to the generic source-formula evaluator.

use crate::probability_pack::Rational;
use crate::source_formula_pack::{FormulaRequest, FormulaResult, FormulaStatus};
use crate::source_statistics_pack::evaluate_statistics;
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
pub struct VisualStatisticsResult {
    pub status: BridgeStatus,
    pub table: Option<TableArtifact>,
    pub request: Option<FormulaRequest>,
    pub statistics: Option<FormulaResult>,
    pub reasons: Vec<String>,
    pub provenance: Vec<String>,
    pub replay_hash: String,
}

fn digest<T: Serialize>(value: &T) -> String {
    format!(
        "{:x}",
        Sha256::digest(serde_json::to_vec(value).expect("visual statistics serializes"))
    )
}

fn payload(result: &VisualStatisticsResult) -> impl Serialize + '_ {
    (
        result.status,
        &result.table,
        &result.request,
        &result.statistics,
        &result.reasons,
        &result.provenance,
    )
}

fn result(
    status: BridgeStatus,
    table: Option<TableArtifact>,
    request: Option<FormulaRequest>,
    statistics: Option<FormulaResult>,
    reasons: Vec<String>,
) -> VisualStatisticsResult {
    let provenance = table
        .as_ref()
        .map(|value| value.provenance_spans.clone())
        .unwrap_or_default();
    let mut output = VisualStatisticsResult {
        status,
        table,
        request,
        statistics,
        reasons,
        provenance,
        replay_hash: String::new(),
    };
    let replay_hash = digest(&payload(&output));
    output.replay_hash = replay_hash;
    output
}

fn rational(text: &str) -> Option<Rational> {
    let text = text.trim();
    if let Some((numerator, denominator)) = text.split_once('/') {
        Rational::new(numerator.parse().ok()?, denominator.parse().ok()?)
    } else {
        Rational::new(text.parse().ok()?, 1)
    }
}

/// Lower one explicitly labelled table into the finite statistics catalog.
pub fn table_to_statistics(table: &TableArtifact) -> VisualStatisticsResult {
    if table.row_count < 2 || table.column_count != 2 || table.rows.len() != table.row_count {
        return result(
            BridgeStatus::Unsupported,
            Some(table.clone()),
            None,
            None,
            vec!["source statistics requires a two-column table with data rows".into()],
        );
    }
    if table.cells.len() != table.row_count * table.column_count
        || table.provenance_spans.is_empty()
    {
        return result(
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
    if header != ["quantity", "value"] {
        let joined = header.join(" ");
        let status = if joined.contains("density") || joined.contains("continuous") {
            BridgeStatus::Unsupported
        } else {
            BridgeStatus::Ambiguous
        };
        return result(
            status,
            Some(table.clone()),
            None,
            None,
            vec!["headers do not uniquely establish source-statistics quantities".into()],
        );
    }
    let mut inputs = BTreeMap::new();
    for row in table.rows.iter().skip(1) {
        if row.len() != 2 || row[0].trim().is_empty() {
            return result(
                BridgeStatus::Missing,
                Some(table.clone()),
                None,
                None,
                vec!["every row needs a labeled quantity and exact value".into()],
            );
        }
        let key = row[0].trim().to_ascii_lowercase();
        if !matches!(
            key.as_str(),
            "sum" | "count" | "weighted_sum" | "total_weight"
        ) {
            return result(
                BridgeStatus::Unsupported,
                Some(table.clone()),
                None,
                None,
                vec!["quantity is outside the validated finite-statistics catalog".into()],
            );
        }
        let Some(value) = rational(&row[1]) else {
            return result(
                BridgeStatus::Inconsistent,
                Some(table.clone()),
                None,
                None,
                vec!["statistical values must be exact integers or rationals".into()],
            );
        };
        if inputs.insert(key, value).is_some() {
            return result(
                BridgeStatus::Ambiguous,
                Some(table.clone()),
                None,
                None,
                vec!["duplicate quantity labels create multiple bindings".into()],
            );
        }
    }
    let formula = if inputs.contains_key("sum") && inputs.contains_key("count") && inputs.len() == 2
    {
        "arithmetic_mean"
    } else if inputs.contains_key("weighted_sum")
        && inputs.contains_key("total_weight")
        && inputs.len() == 2
    {
        "weighted_mean"
    } else {
        return result(
            BridgeStatus::Ambiguous,
            Some(table.clone()),
            None,
            None,
            vec!["quantity set does not select one validated source formula".into()],
        );
    };
    let request = FormulaRequest {
        formula: formula.into(),
        inputs,
        domain: crate::source_statistics_pack::DOMAIN.into(),
        ambiguity: None,
        provenance: table.provenance_spans.clone(),
    };
    let statistics = evaluate_statistics(&request);
    let status = match statistics.status {
        FormulaStatus::Complete => BridgeStatus::Complete,
        FormulaStatus::Inconsistent => BridgeStatus::Inconsistent,
        FormulaStatus::Ambiguous => BridgeStatus::Ambiguous,
        FormulaStatus::Missing => BridgeStatus::Missing,
        FormulaStatus::Unsupported | FormulaStatus::InvalidDomain => BridgeStatus::Unsupported,
    };
    result(
        status,
        Some(table.clone()),
        Some(request),
        Some(statistics),
        Vec::new(),
    )
}

impl VisualStatisticsResult {
    pub fn replay_verified(&self) -> bool {
        self.replay_hash == digest(&payload(self))
            && !self.provenance.is_empty()
            && self
                .statistics
                .as_ref()
                .is_none_or(FormulaResult::replay_verified)
    }

    pub fn authorized(&self) -> bool {
        self.status == BridgeStatus::Complete
            && self.replay_verified()
            && self
                .statistics
                .as_ref()
                .is_some_and(|result| result.status == FormulaStatus::Complete)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn table(rows: Vec<Vec<&str>>) -> TableArtifact {
        let rows = rows
            .into_iter()
            .map(|row| row.into_iter().map(str::to_owned).collect::<Vec<_>>())
            .collect::<Vec<_>>();
        let cells = rows
            .iter()
            .enumerate()
            .flat_map(|(row, values)| {
                values.iter().enumerate().map(move |(column, text)| {
                    crate::vision::visual_table::TableCell {
                        text: text.clone(),
                        row,
                        column,
                        left: (column * 40) as u32,
                        top: (row * 20) as u32,
                        width: 30,
                        height: 10,
                    }
                })
            })
            .collect();
        TableArtifact {
            row_count: rows.len(),
            column_count: rows[0].len(),
            cells,
            provenance_spans: vec!["ocr:table".into()],
            rows,
        }
    }

    #[test]
    fn labeled_table_reaches_source_statistics() {
        let result = table_to_statistics(&table(vec![
            vec!["quantity", "value"],
            vec!["sum", "30"],
            vec!["count", "5"],
        ]));
        assert!(result.authorized());
        assert!(result.replay_verified());
    }

    #[test]
    fn unlabeled_table_stays_closed() {
        let result = table_to_statistics(&table(vec![
            vec!["name", "value"],
            vec!["sum", "30"],
            vec!["count", "5"],
        ]));
        assert_eq!(result.status, BridgeStatus::Ambiguous);
        assert!(!result.authorized());
    }
}
