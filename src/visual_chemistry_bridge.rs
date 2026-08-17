//! Explicit visual-table to source-derived chemistry/linear bridge.
//!
//! Numeric tables are not molecular formulas by appearance alone.  This
//! bridge requires an exact `element,count` header, explicit element symbols,
//! positive bounded integer counts, and no charge/phase notation before a
//! canonical formula is passed to the source chemistry pack.

use crate::source_formula_pack::chemistry_pack::{
    chemistry_linear_bridge::{bridge_chemistry_to_linear, ChemistryLinearBridgeResult},
    evaluate_chemistry, ChemistryArtifact, ChemistryOperation, ChemistryRequest, ChemistryResult,
    ChemistryStatus,
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
pub struct VisualChemistryResult {
    pub status: BridgeStatus,
    pub table: Option<TableArtifact>,
    pub chemistry: Option<ChemistryResult>,
    pub linear: Option<ChemistryLinearBridgeResult>,
    pub reasons: Vec<String>,
    pub provenance: Vec<String>,
    pub replay_hash: String,
}

fn digest<T: Serialize>(value: &T) -> String {
    format!(
        "{:x}",
        Sha256::digest(serde_json::to_vec(value).expect("visual chemistry serializes"))
    )
}

fn payload(result: &VisualChemistryResult) -> impl Serialize + '_ {
    (
        result.status,
        &result.table,
        &result.chemistry,
        &result.linear,
        &result.reasons,
        &result.provenance,
    )
}

fn output(
    status: BridgeStatus,
    table: Option<TableArtifact>,
    chemistry: Option<ChemistryResult>,
    linear: Option<ChemistryLinearBridgeResult>,
    reasons: Vec<String>,
) -> VisualChemistryResult {
    let provenance = table
        .as_ref()
        .map(|value| value.provenance_spans.clone())
        .unwrap_or_default();
    let mut result = VisualChemistryResult {
        status,
        table,
        chemistry,
        linear,
        reasons,
        provenance,
        replay_hash: String::new(),
    };
    let replay_hash = digest(&payload(&result));
    result.replay_hash = replay_hash;
    result
}

fn count(text: &str) -> Option<u32> {
    let text = text.trim();
    if text.is_empty() || text.starts_with('-') {
        return None;
    }
    text.parse().ok()
}

fn chemistry_status(status: ChemistryStatus) -> BridgeStatus {
    match status {
        ChemistryStatus::Complete => BridgeStatus::Complete,
        ChemistryStatus::Ambiguous => BridgeStatus::Ambiguous,
        ChemistryStatus::Missing => BridgeStatus::Missing,
        ChemistryStatus::Unsupported | ChemistryStatus::InvalidDomain => BridgeStatus::Unsupported,
        ChemistryStatus::Inconsistent => BridgeStatus::Inconsistent,
    }
}

/// Lower an explicitly labelled element-count table to chemistry and a
/// semantically labelled element-count vector.  The table does not assert a
/// reaction, charge, phase, molar mass, or molecular identity beyond its
/// explicit element counts.
pub fn table_to_chemistry_linear(table: &TableArtifact) -> VisualChemistryResult {
    if table.row_count < 2 || table.column_count != 2 || table.rows.len() != table.row_count {
        return output(
            BridgeStatus::Unsupported,
            Some(table.clone()),
            None,
            None,
            vec!["chemistry requires a two-column element-count table".into()],
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
    if header != ["element", "count"] {
        let joined = header.join(" ");
        let status = if joined.contains("charge") || joined.contains("phase") {
            BridgeStatus::Unsupported
        } else {
            BridgeStatus::Ambiguous
        };
        return output(
            status,
            Some(table.clone()),
            None,
            None,
            vec!["headers do not uniquely establish element-count semantics".into()],
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
                vec!["each row needs one element symbol and one count".into()],
            );
        }
        let element = row[0].trim().to_string();
        if element.contains('+') || element.contains('-') || element.contains('(') {
            return output(
                BridgeStatus::Unsupported,
                Some(table.clone()),
                None,
                None,
                vec!["charges, phases, and grouped species are outside this bridge".into()],
            );
        }
        let Some(value) = count(&row[1]) else {
            return output(
                BridgeStatus::Inconsistent,
                Some(table.clone()),
                None,
                None,
                vec!["element counts must be positive integers".into()],
            );
        };
        if value == 0 || value > 100 {
            return output(
                BridgeStatus::Unsupported,
                Some(table.clone()),
                None,
                None,
                vec!["element counts exceed the bounded formula contract".into()],
            );
        }
        if counts.insert(element, value).is_some() {
            return output(
                BridgeStatus::Ambiguous,
                Some(table.clone()),
                None,
                None,
                vec!["duplicate element rows create multiple formula bindings".into()],
            );
        }
    }
    if counts.is_empty() {
        return output(
            BridgeStatus::Missing,
            Some(table.clone()),
            None,
            None,
            vec!["at least one element count is required".into()],
        );
    }
    let formula = counts
        .iter()
        .map(|(element, value)| {
            if *value == 1 {
                element.clone()
            } else {
                format!("{element}{value}")
            }
        })
        .collect::<String>();
    let chemistry = evaluate_chemistry(&ChemistryRequest {
        operation: ChemistryOperation::ParseFormula,
        formula: Some(formula),
        reaction: None,
        from_species: None,
        to_species: None,
        domain: "source_derived_bounded_chemistry".into(),
        ambiguity: None,
        provenance: table.provenance_spans.clone(),
    });
    if chemistry.status != ChemistryStatus::Complete || !chemistry.replay_verified() {
        return output(
            chemistry_status(chemistry.status),
            Some(table.clone()),
            Some(chemistry),
            None,
            vec!["element table did not produce a replayable chemistry artifact".into()],
        );
    }
    if !matches!(
        chemistry.artifact,
        Some(ChemistryArtifact::MolecularFormula { .. })
    ) {
        return output(
            BridgeStatus::Unsupported,
            Some(table.clone()),
            Some(chemistry),
            None,
            vec!["only molecular-formula artifacts may cross this bridge".into()],
        );
    }
    let linear = bridge_chemistry_to_linear(&chemistry);
    let status = if linear.authorized() {
        BridgeStatus::Complete
    } else {
        BridgeStatus::Unsupported
    };
    output(
        status,
        Some(table.clone()),
        Some(chemistry),
        Some(linear),
        Vec::new(),
    )
}

impl VisualChemistryResult {
    pub fn replay_verified(&self) -> bool {
        self.replay_hash == digest(&payload(self))
            && !self.provenance.is_empty()
            && self
                .chemistry
                .as_ref()
                .is_none_or(ChemistryResult::replay_verified)
            && self
                .linear
                .as_ref()
                .is_none_or(ChemistryLinearBridgeResult::replay_verified)
    }

    pub fn authorized(&self) -> bool {
        self.status == BridgeStatus::Complete
            && self.replay_verified()
            && self
                .chemistry
                .as_ref()
                .is_some_and(|value| value.status == ChemistryStatus::Complete)
            && self.linear.as_ref().is_some_and(|value| value.authorized())
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
    fn explicit_element_counts_become_linear_artifacts() {
        let result = table_to_chemistry_linear(&table(vec![
            vec!["element", "count"],
            vec!["H", "2"],
            vec!["O", "1"],
        ]));
        assert!(result.authorized());
        assert_eq!(
            result.linear.unwrap().artifact.unwrap().basis,
            vec!["H", "O"]
        );
    }

    #[test]
    fn duplicate_and_charge_rows_fail_closed() {
        let duplicate = table(vec![
            vec!["element", "count"],
            vec!["H", "2"],
            vec!["H", "1"],
        ]);
        assert_eq!(
            table_to_chemistry_linear(&duplicate).status,
            BridgeStatus::Ambiguous
        );
        let charge = table(vec![vec!["element", "count"], vec!["Na+", "1"]]);
        assert_eq!(
            table_to_chemistry_linear(&charge).status,
            BridgeStatus::Unsupported
        );
    }
}
