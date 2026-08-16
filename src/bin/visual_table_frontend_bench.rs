//! Stage J benchmark for coordinate-preserving visual table formalization.

use serde::Serialize;
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use the_machine::vision::visual_table::{formalize_table_tsv, TableStatus};

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
enum Expected {
    Supported,
    Ambiguous,
    Unsupported,
}

#[derive(Debug, Serialize)]
struct Receipt {
    id: String,
    expected: Expected,
    actual: TableStatus,
    row_count: Option<usize>,
    column_count: Option<usize>,
    cell_count: usize,
    exact: bool,
    provenance_preserved: bool,
    replay_verified: bool,
    tamper_rejected: bool,
    false_authorization: bool,
}

#[derive(Debug, Serialize)]
struct Report {
    schema: &'static str,
    corpus_sha256: String,
    cases: usize,
    supported: usize,
    ambiguous: usize,
    unsupported: usize,
    exact_decisions: usize,
    supported_artifacts: usize,
    provenance_preserved: usize,
    replay_verified: usize,
    tamper_rejections: usize,
    false_authorizations: usize,
    false_denials: usize,
    status_counts: BTreeMap<String, usize>,
    receipts: Vec<Receipt>,
}

const HEADER: &str = "level\tpage\tblock\tpar\tline\tword\tleft\ttop\twidth\theight\tconf\ttext";

fn word(left: usize, top: usize, text: &str, width: usize) -> String {
    format!("5\t1\t1\t1\t1\t1\t{left}\t{top}\t{width}\t10\t90\t{text}")
}

fn grid(rows: usize, columns: usize, shift: usize) -> String {
    let mut lines = vec![HEADER.into()];
    for row in 0..rows {
        for column in 0..columns {
            lines.push(word(
                10 + column * 50 + if row > 0 { shift } else { 0 },
                10 + row * 30,
                &format!("r{row}c{column}"),
                20,
            ));
        }
    }
    lines.join("\n")
}

fn run(id: String, text: String, expected: Expected) -> Receipt {
    let output = formalize_table_tsv(&text);
    let mut tampered = output.clone();
    tampered.replay_hash.push('x');
    let exact = match expected {
        Expected::Supported => output.status == TableStatus::Complete,
        Expected::Ambiguous => output.status == TableStatus::Ambiguous,
        Expected::Unsupported => {
            matches!(
                output.status,
                TableStatus::Unsupported | TableStatus::Missing
            )
        }
    };
    let artifact = output.artifact.as_ref();
    Receipt {
        id,
        expected,
        actual: output.status,
        row_count: artifact.map(|value| value.row_count),
        column_count: artifact.map(|value| value.column_count),
        cell_count: artifact.map_or(0, |value| value.cells.len()),
        exact,
        provenance_preserved: artifact.map_or(false, |value| !value.provenance_spans.is_empty()),
        replay_verified: output.replay_verified(),
        tamper_rejected: !tampered.replay_verified(),
        false_authorization: expected != Expected::Supported
            && output.status == TableStatus::Complete,
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut receipts = Vec::with_capacity(240);
    for index in 0..120 {
        let (rows, columns) = if index % 2 == 0 { (2, 2) } else { (3, 3) };
        receipts.push(run(
            format!("supported_{index:03}"),
            grid(rows, columns, 0),
            Expected::Supported,
        ));
    }
    for index in 0..40 {
        let text = if index % 2 == 0 {
            [
                HEADER,
                &word(10, 10, "x", 20),
                &word(50, 10, "y", 20),
                &word(10, 40, "1", 20),
            ]
            .join("\n")
        } else {
            grid(2, 2, 45)
        };
        receipts.push(run(
            format!("ambiguous_{index:03}"),
            text,
            Expected::Ambiguous,
        ));
    }
    let unsupported = [
        "",
        HEADER,
        &word(10, 10, "single", 20),
        &[HEADER, &word(10, 10, "x", 20), &word(50, 10, "y", 20)].join("\n"),
    ];
    for index in 0..80 {
        receipts.push(run(
            format!("unsupported_{index:03}"),
            unsupported[index % unsupported.len()].into(),
            Expected::Unsupported,
        ));
    }
    assert_eq!(receipts.len(), 240);
    let cases = receipts.len();
    let supported = receipts
        .iter()
        .filter(|r| r.expected == Expected::Supported)
        .count();
    let ambiguous = receipts
        .iter()
        .filter(|r| r.expected == Expected::Ambiguous)
        .count();
    let unsupported = receipts
        .iter()
        .filter(|r| r.expected == Expected::Unsupported)
        .count();
    let exact_decisions = receipts.iter().filter(|r| r.exact).count();
    let supported_artifacts = receipts
        .iter()
        .filter(|r| r.expected == Expected::Supported && r.actual == TableStatus::Complete)
        .count();
    let provenance_preserved = receipts
        .iter()
        .filter(|r| r.expected == Expected::Supported && r.provenance_preserved)
        .count();
    let replay_verified = receipts.iter().filter(|r| r.replay_verified).count();
    let tamper_rejections = receipts.iter().filter(|r| r.tamper_rejected).count();
    let false_authorizations = receipts.iter().filter(|r| r.false_authorization).count();
    let false_denials = receipts
        .iter()
        .filter(|r| r.expected == Expected::Supported && !r.exact)
        .count();
    let mut status_counts = BTreeMap::new();
    for receipt in &receipts {
        *status_counts
            .entry(format!("{:?}", receipt.actual))
            .or_insert(0usize) += 1;
    }
    assert_eq!((supported, ambiguous, unsupported), (120, 40, 80));
    assert_eq!(exact_decisions, cases);
    assert_eq!(supported_artifacts, supported);
    assert_eq!(provenance_preserved, supported);
    assert_eq!(replay_verified, cases);
    assert_eq!(tamper_rejections, cases);
    assert_eq!(false_authorizations, 0);
    assert_eq!(false_denials, 0);
    let report = Report {
        schema: "stage-j-visual-table-frontend-v1",
        corpus_sha256: format!("{:x}", Sha256::digest(serde_json::to_vec(&receipts)?)),
        cases,
        supported,
        ambiguous,
        unsupported,
        exact_decisions,
        supported_artifacts,
        provenance_preserved,
        replay_verified,
        tamper_rejections,
        false_authorizations,
        false_denials,
        status_counts,
        receipts,
    };
    let serialized = serde_json::to_string_pretty(&report)?;
    std::fs::write(
        "docs/stage_j_visual_table_frontend.json",
        format!("{serialized}\n"),
    )?;
    println!("{serialized}");
    Ok(())
}
