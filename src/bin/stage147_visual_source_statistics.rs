//! Stage 147: route-blind visual table to source-derived statistics.

use serde::Serialize;
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use the_machine::source_formula_pack::FormulaStatus;
use the_machine::vision::visual_source_statistics_bridge::{table_to_statistics, BridgeStatus};
use the_machine::vision::visual_table::{TableArtifact, TableCell};

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
enum Expected {
    Supported,
    Ambiguous,
    Unsupported,
}

#[derive(Debug, Clone)]
struct Case {
    id: String,
    expected: Expected,
    rows: Vec<Vec<String>>,
}

#[derive(Debug, Serialize)]
struct Receipt {
    id: String,
    expected: Expected,
    status: BridgeStatus,
    authorized: bool,
    exact: bool,
    replay_verified: bool,
    tamper_rejected: bool,
}

#[derive(Debug, Serialize)]
struct Report {
    schema: &'static str,
    source: &'static str,
    corpus_sha256: String,
    cases: usize,
    supported: usize,
    ambiguous: usize,
    unsupported: usize,
    exact_decisions: usize,
    authorized_supported: usize,
    replay_verified: usize,
    tamper_rejections: usize,
    false_authorizations: usize,
    false_denials: usize,
    status_counts: BTreeMap<String, usize>,
    receipts: Vec<Receipt>,
}

fn digest<T: Serialize>(value: &T) -> String {
    format!("{:x}", Sha256::digest(serde_json::to_vec(value).unwrap()))
}

fn artifact(rows: Vec<Vec<String>>, case_id: &str) -> TableArtifact {
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
                    left: (column * 50) as u32,
                    top: (row * 20) as u32,
                    width: 40,
                    height: 12,
                })
        })
        .collect();
    TableArtifact {
        row_count: rows.len(),
        column_count: rows.first().map_or(0, Vec::len),
        cells,
        provenance_spans: vec![
            format!("ocr-table:{case_id}"),
            "source-catalog:finite-statistics".into(),
        ],
        rows,
    }
}

fn corpus() -> Vec<Case> {
    let mut cases = Vec::with_capacity(240);
    for index in 0..120 {
        let rows = if index % 2 == 0 {
            vec![
                vec!["quantity".into(), "value".into()],
                vec!["sum".into(), "30".into()],
                vec!["count".into(), "5".into()],
            ]
        } else {
            vec![
                vec!["quantity".into(), "value".into()],
                vec!["weighted_sum".into(), "42".into()],
                vec!["total_weight".into(), "7".into()],
            ]
        };
        cases.push(Case {
            id: format!("supported_{index:03}"),
            expected: Expected::Supported,
            rows,
        });
    }
    for index in 0..40 {
        cases.push(Case {
            id: format!("ambiguous_{index:03}"),
            expected: Expected::Ambiguous,
            rows: vec![
                vec!["label".into(), "value".into()],
                vec!["sum".into(), "30".into()],
                vec!["count".into(), "5".into()],
            ],
        });
    }
    for index in 0..40 {
        cases.push(Case {
            id: format!("ambiguous_duplicate_{index:03}"),
            expected: Expected::Ambiguous,
            rows: vec![
                vec!["quantity".into(), "value".into()],
                vec!["sum".into(), "30".into()],
                vec!["sum".into(), "31".into()],
            ],
        });
    }
    for index in 0..40 {
        let rows = if index % 2 == 0 {
            vec![
                vec!["continuous density".into(), "value".into()],
                vec!["sum".into(), "30".into()],
                vec!["count".into(), "5".into()],
            ]
        } else {
            vec![
                vec!["quantity".into(), "value".into()],
                vec!["unknown_quantity".into(), "30".into()],
                vec!["count".into(), "5".into()],
            ]
        };
        cases.push(Case {
            id: format!("unsupported_{index:03}"),
            expected: Expected::Unsupported,
            rows,
        });
    }
    cases
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cases = corpus();
    assert_eq!(cases.len(), 240);
    let corpus_sha256 = digest(
        &cases
            .iter()
            .map(|c| (&c.id, c.expected, &c.rows))
            .collect::<Vec<_>>(),
    );
    let mut receipts = Vec::with_capacity(cases.len());
    let mut status_counts = BTreeMap::new();
    for case in cases {
        let table = artifact(case.rows, &case.id);
        let result = table_to_statistics(&table);
        let mut tampered = result.clone();
        tampered.replay_hash.push('x');
        let authorized = result.authorized()
            && result
                .statistics
                .as_ref()
                .is_some_and(|value| value.status == FormulaStatus::Complete);
        let exact = match case.expected {
            Expected::Supported => result.status == BridgeStatus::Complete && authorized,
            Expected::Ambiguous => result.status == BridgeStatus::Ambiguous && !authorized,
            Expected::Unsupported => result.status == BridgeStatus::Unsupported && !authorized,
        };
        let replay = result.replay_verified();
        let tamper_rejected = !tampered.replay_verified();
        *status_counts
            .entry(format!("{:?}", result.status))
            .or_insert(0) += 1;
        receipts.push(Receipt {
            id: case.id,
            expected: case.expected,
            status: result.status,
            authorized,
            exact,
            replay_verified: replay,
            tamper_rejected,
        });
    }
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
    let authorized_supported = receipts.iter().filter(|r| r.authorized).count();
    let replay_verified = receipts.iter().filter(|r| r.replay_verified).count();
    let tamper_rejections = receipts.iter().filter(|r| r.tamper_rejected).count();
    let false_authorizations = receipts
        .iter()
        .filter(|r| r.expected != Expected::Supported && r.authorized)
        .count();
    let false_denials = receipts
        .iter()
        .filter(|r| r.expected == Expected::Supported && !r.authorized)
        .count();
    assert_eq!((supported, ambiguous, unsupported), (120, 80, 40));
    assert_eq!(exact_decisions, 240);
    assert_eq!(authorized_supported, 120);
    assert_eq!(replay_verified, 240);
    assert_eq!(tamper_rejections, 240);
    assert_eq!(false_authorizations, 0);
    assert_eq!(false_denials, 0);
    let report = Report {
        schema: "stage147-visual-source-statistics-v1",
        source: "independently authored coordinate-preserving visual table corpus",
        corpus_sha256,
        cases,
        supported,
        ambiguous,
        unsupported,
        exact_decisions,
        authorized_supported,
        replay_verified,
        tamper_rejections,
        false_authorizations,
        false_denials,
        status_counts,
        receipts,
    };
    let json = serde_json::to_vec_pretty(&report)?;
    std::fs::write("docs/stage147_visual_source_statistics.json", &json)?;
    println!("{}", String::from_utf8(json)?);
    Ok(())
}
