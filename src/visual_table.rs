//! Conservative visual-to-table frontend.
//!
//! This module lowers coordinate-bearing OCR observations into a typed table
//! artifact. It does not infer chart meaning, units, formulas, or relationships
//! from appearance. Ragged rows, misaligned columns, and empty OCR input are
//! preserved as ambiguity or unsupported input.

use crate::vision::{parse_tesseract_tsv, OcrWord};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

#[path = "visual_probability_bridge.rs"]
pub mod visual_probability_bridge;

#[path = "visual_biology_bridge.rs"]
pub mod visual_biology_bridge;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum TableStatus {
    Complete,
    Ambiguous,
    Unsupported,
    Missing,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TableCell {
    pub text: String,
    pub row: usize,
    pub column: usize,
    pub left: u32,
    pub top: u32,
    pub width: u32,
    pub height: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TableArtifact {
    pub rows: Vec<Vec<String>>,
    pub cells: Vec<TableCell>,
    pub row_count: usize,
    pub column_count: usize,
    pub provenance_spans: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TableFrontendResult {
    pub status: TableStatus,
    pub artifact: Option<TableArtifact>,
    pub alternatives: Vec<String>,
    pub reasons: Vec<String>,
    pub replay_hash: String,
}

fn digest<T: Serialize>(value: &T) -> String {
    format!(
        "{:x}",
        Sha256::digest(serde_json::to_vec(value).expect("table result serializes"))
    )
}

fn payload(result: &TableFrontendResult) -> impl Serialize + '_ {
    (
        result.status,
        &result.artifact,
        &result.alternatives,
        &result.reasons,
    )
}

fn result(
    status: TableStatus,
    artifact: Option<TableArtifact>,
    alternatives: Vec<String>,
    reasons: Vec<String>,
) -> TableFrontendResult {
    let mut output = TableFrontendResult {
        status,
        artifact,
        alternatives,
        reasons,
        replay_hash: String::new(),
    };
    let replay_hash = digest(&payload(&output));
    output.replay_hash = replay_hash;
    output
}

fn grouped_rows(words: &[OcrWord]) -> Vec<Vec<OcrWord>> {
    let mut ordered = words.to_vec();
    ordered.sort_by_key(|word| (word.top, word.left));
    let mut rows: Vec<(u32, Vec<OcrWord>)> = Vec::new();
    for word in ordered {
        let tolerance = word.height.max(8);
        if let Some((top, row)) = rows
            .iter_mut()
            .find(|(top, _)| word.top.abs_diff(*top) <= tolerance)
        {
            *top = (*top).min(word.top);
            row.push(word);
        } else {
            rows.push((word.top, vec![word]));
        }
    }
    rows.sort_by_key(|(top, _)| *top);
    rows.into_iter()
        .map(|(_, mut row)| {
            row.sort_by_key(|word| word.left);
            row
        })
        .collect()
}

fn overlaps(left: &OcrWord, right: &OcrWord) -> bool {
    let left_end = left.left.saturating_add(left.width);
    let right_end = right.left.saturating_add(right.width);
    left.left < right_end && right.left < left_end
}

/// Lower a Tesseract TSV snapshot into a table only when its grid is explicit
/// and stable across rows. Coordinate spans remain part of the artifact.
pub fn formalize_table_tsv(tsv: &str) -> TableFrontendResult {
    let words = parse_tesseract_tsv(tsv);
    if words.is_empty() {
        return result(
            TableStatus::Missing,
            None,
            Vec::new(),
            vec!["no coordinate-bearing OCR words were available".into()],
        );
    }
    let rows = grouped_rows(&words);
    if rows.len() < 2 {
        return result(
            TableStatus::Unsupported,
            None,
            Vec::new(),
            vec!["a bounded table needs at least two rows and two cells per row".into()],
        );
    }
    let column_count = rows[0].len();
    if rows.iter().any(|row| row.len() != column_count) {
        return result(
            TableStatus::Ambiguous,
            Some(TableArtifact {
                rows: rows
                    .iter()
                    .map(|row| row.iter().map(|word| word.text.clone()).collect())
                    .collect(),
                cells: Vec::new(),
                row_count: rows.len(),
                column_count,
                provenance_spans: words
                    .iter()
                    .map(|word| format!("{}@{},{}", word.text, word.left, word.top))
                    .collect(),
            }),
            vec!["row lengths do not define one unique table grid".into()],
            vec!["column count is inconsistent across rows".into()],
        );
    }
    if column_count < 2 {
        return result(
            TableStatus::Unsupported,
            None,
            Vec::new(),
            vec!["a bounded table needs at least two cells per row".into()],
        );
    }
    for row in &rows {
        if row.windows(2).any(|pair| overlaps(&pair[0], &pair[1])) {
            return result(
                TableStatus::Ambiguous,
                None,
                vec!["overlapping OCR boxes admit multiple cell assignments".into()],
                Vec::new(),
            );
        }
    }
    let anchors: Vec<u32> = rows[0].iter().map(|word| word.left).collect();
    let aligned = rows.iter().all(|row| {
        row.iter()
            .enumerate()
            .all(|(column, word)| word.left.abs_diff(anchors[column]) <= word.width.max(8))
    });
    if !aligned {
        return result(
            TableStatus::Ambiguous,
            None,
            vec!["row columns are not aligned to one coordinate grid".into()],
            Vec::new(),
        );
    }
    let cells: Vec<TableCell> = rows
        .iter()
        .enumerate()
        .flat_map(|(row, words)| {
            words
                .iter()
                .enumerate()
                .map(move |(column, word)| TableCell {
                    text: word.text.clone(),
                    row,
                    column,
                    left: word.left,
                    top: word.top,
                    width: word.width,
                    height: word.height,
                })
        })
        .collect();
    let artifact = TableArtifact {
        rows: rows
            .iter()
            .map(|row| row.iter().map(|word| word.text.clone()).collect())
            .collect(),
        cells,
        row_count: rows.len(),
        column_count,
        provenance_spans: words
            .iter()
            .map(|word| format!("{}@{},{}", word.text, word.left, word.top))
            .collect(),
    };
    result(
        TableStatus::Complete,
        Some(artifact),
        Vec::new(),
        Vec::new(),
    )
}

impl TableFrontendResult {
    pub fn replay_verified(&self) -> bool {
        self.replay_hash == digest(&payload(self))
            && match self.status {
                TableStatus::Complete => self.artifact.as_ref().is_some_and(|artifact| {
                    artifact.row_count > 1
                        && artifact.column_count > 1
                        && artifact.cells.len() == artifact.row_count * artifact.column_count
                        && !artifact.provenance_spans.is_empty()
                }),
                _ => true,
            }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn row(_level: usize, left: usize, top: usize, text: &str) -> String {
        format!("5\t1\t1\t1\t1\t90\t{left}\t{top}\t20\t10\t90\t{text}")
    }

    #[test]
    fn aligned_grid_becomes_replayable_table() {
        let tsv = [
            "level\tpage\tblock\tpar\tline\tword\tleft\ttop\twidth\theight\tconf\ttext",
            &row(5, 10, 10, "x"),
            &row(5, 50, 10, "y"),
            &row(5, 10, 40, "1"),
            &row(5, 50, 40, "2"),
        ]
        .join("\n");
        let result = formalize_table_tsv(&tsv);
        assert_eq!(result.status, TableStatus::Complete);
        assert!(result.replay_verified());
    }

    #[test]
    fn ragged_and_empty_inputs_fail_closed() {
        let ragged = [
            "level\tpage\tblock\tpar\tline\tword\tleft\ttop\twidth\theight\tconf\ttext",
            &row(5, 10, 10, "x"),
            &row(5, 50, 10, "y"),
            &row(5, 10, 40, "1"),
        ]
        .join("\n");
        assert_eq!(formalize_table_tsv(&ragged).status, TableStatus::Ambiguous);
        assert_eq!(formalize_table_tsv("").status, TableStatus::Missing);
    }
}
