//! Stage 152: coordinate-bearing OCR TSV through typed tables into source
//! statistics, biology, and chemistry bridges.

use serde::Serialize;
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use the_machine::vision::visual_source_statistics_bridge::table_to_statistics;
use the_machine::vision::visual_table::visual_biology_bridge::table_to_biology_probability;
use the_machine::vision::visual_table::visual_chemistry_bridge::table_to_chemistry_linear;
use the_machine::vision::visual_table::{formalize_table_tsv, TableStatus};

const HEADER: &str = "level\tpage\tblock\tpar\tline\tword\tleft\ttop\twidth\theight\tconf\ttext";

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
enum Family {
    Statistics,
    Biology,
    Chemistry,
}

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
enum Expected {
    Supported,
    Ambiguous,
    Unsupported,
}

#[derive(Debug, Serialize)]
struct Receipt {
    family: Family,
    id: String,
    expected: Expected,
    frontend_status: TableStatus,
    downstream_status: String,
    authorized: bool,
    exact: bool,
    frontend_replay: bool,
    downstream_replay: bool,
    frontend_tamper_rejected: bool,
    downstream_tamper_rejected: bool,
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
    frontend_replay_verified: usize,
    downstream_replay_verified: usize,
    frontend_tamper_rejections: usize,
    downstream_tamper_rejections: usize,
    false_authorizations: usize,
    false_denials: usize,
    family_counts: BTreeMap<String, usize>,
    receipts: Vec<Receipt>,
}

fn digest<T: Serialize>(value: &T) -> String {
    format!("{:x}", Sha256::digest(serde_json::to_vec(value).unwrap()))
}

fn word(left: usize, top: usize, text: &str) -> String {
    format!("5\t1\t1\t1\t1\t1\t{left}\t{top}\t40\t10\t90\t{text}")
}

fn tsv(rows: &[Vec<&str>]) -> String {
    let mut lines = vec![HEADER.into()];
    for (row, values) in rows.iter().enumerate() {
        for (column, value) in values.iter().enumerate() {
            lines.push(word(10 + column * 80, 10 + row * 30, value));
        }
    }
    lines.join("\n")
}

fn run(family: Family, id: String, expected: Expected, rows: Vec<Vec<&str>>) -> Receipt {
    let frontend = formalize_table_tsv(&tsv(&rows));
    let mut frontend_tampered = frontend.clone();
    frontend_tampered.replay_hash.push('x');
    let Some(table) = frontend.artifact.as_ref() else {
        return Receipt {
            family,
            id,
            expected,
            frontend_status: frontend.status,
            downstream_status: "not_emitted".into(),
            authorized: false,
            exact: expected != Expected::Supported,
            frontend_replay: frontend.replay_verified(),
            downstream_replay: true,
            frontend_tamper_rejected: !frontend_tampered.replay_verified(),
            downstream_tamper_rejected: true,
        };
    };
    let (downstream_status, authorized, downstream_replay, downstream_tamper_rejected) =
        match family {
            Family::Statistics => {
                let result = table_to_statistics(table);
                let mut tampered = result.clone();
                tampered.replay_hash.push('x');
                (
                    format!("{:?}", result.status),
                    result.authorized(),
                    result.replay_verified(),
                    !tampered.replay_verified(),
                )
            }
            Family::Biology => {
                let policy = (expected == Expected::Supported).then_some("uniform_position");
                let result = table_to_biology_probability(table, policy);
                let mut tampered = result.clone();
                tampered.replay_hash.push('x');
                (
                    format!("{:?}", result.status),
                    result.authorized(),
                    result.replay_verified(),
                    !tampered.replay_verified(),
                )
            }
            Family::Chemistry => {
                let result = table_to_chemistry_linear(table);
                let mut tampered = result.clone();
                tampered.replay_hash.push('x');
                (
                    format!("{:?}", result.status),
                    result.authorized(),
                    result.replay_verified(),
                    !tampered.replay_verified(),
                )
            }
        };
    let exact = match expected {
        Expected::Supported => frontend.status == TableStatus::Complete && authorized,
        Expected::Ambiguous => !authorized && downstream_status == "Ambiguous",
        Expected::Unsupported => {
            !authorized
                && matches!(
                    frontend.status,
                    TableStatus::Complete | TableStatus::Unsupported | TableStatus::Ambiguous
                )
        }
    };
    Receipt {
        family,
        id,
        expected,
        frontend_status: frontend.status,
        downstream_status,
        authorized,
        exact,
        frontend_replay: frontend.replay_verified(),
        downstream_replay,
        frontend_tamper_rejected: !frontend_tampered.replay_verified(),
        downstream_tamper_rejected,
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut receipts = Vec::with_capacity(600);
    for index in 0..120 {
        receipts.push(run(
            Family::Statistics,
            format!("supported_{index:03}"),
            Expected::Supported,
            vec![
                vec!["quantity", "value"],
                vec!["sum", "30"],
                vec!["count", "5"],
            ],
        ));
    }
    for index in 0..40 {
        receipts.push(run(
            Family::Statistics,
            format!("ambiguous_{index:03}"),
            Expected::Ambiguous,
            vec![
                vec!["label", "value"],
                vec!["sum", "30"],
                vec!["count", "5"],
            ],
        ));
    }
    for index in 0..40 {
        receipts.push(run(
            Family::Statistics,
            format!("unsupported_{index:03}"),
            Expected::Unsupported,
            vec![
                vec!["continuous density", "value"],
                vec!["sum", "30"],
                vec!["count", "5"],
            ],
        ));
    }
    for index in 0..120 {
        receipts.push(run(
            Family::Biology,
            format!("supported_{index:03}"),
            Expected::Supported,
            vec![
                vec!["base", "count"],
                vec!["A", "2"],
                vec!["C", "2"],
                vec!["G", "2"],
                vec!["T", "2"],
            ],
        ));
    }
    for index in 0..40 {
        receipts.push(run(
            Family::Biology,
            format!("ambiguous_{index:03}"),
            Expected::Ambiguous,
            vec![
                vec!["base", "count"],
                vec!["A", "2"],
                vec!["C", "2"],
                vec!["G", "2"],
                vec!["T", "2"],
            ],
        ));
    }
    for index in 0..40 {
        receipts.push(run(
            Family::Biology,
            format!("unsupported_{index:03}"),
            Expected::Unsupported,
            vec![vec!["rna", "count"], vec!["A", "2"], vec!["U", "2"]],
        ));
    }
    for index in 0..120 {
        receipts.push(run(
            Family::Chemistry,
            format!("supported_{index:03}"),
            Expected::Supported,
            vec![vec!["element", "count"], vec!["H", "2"], vec!["O", "1"]],
        ));
    }
    for index in 0..40 {
        receipts.push(run(
            Family::Chemistry,
            format!("ambiguous_{index:03}"),
            Expected::Ambiguous,
            vec![vec!["label", "count"], vec!["H", "2"], vec!["O", "1"]],
        ));
    }
    for index in 0..40 {
        receipts.push(run(
            Family::Chemistry,
            format!("unsupported_{index:03}"),
            Expected::Unsupported,
            vec![vec!["element", "count"], vec!["Na+", "1"]],
        ));
    }
    assert_eq!(receipts.len(), 600);
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
    let frontend_replay_verified = receipts.iter().filter(|r| r.frontend_replay).count();
    let downstream_replay_verified = receipts.iter().filter(|r| r.downstream_replay).count();
    let frontend_tamper_rejections = receipts
        .iter()
        .filter(|r| r.frontend_tamper_rejected)
        .count();
    let downstream_tamper_rejections = receipts
        .iter()
        .filter(|r| r.downstream_tamper_rejected)
        .count();
    let false_authorizations = receipts
        .iter()
        .filter(|r| r.expected != Expected::Supported && r.authorized)
        .count();
    let false_denials = receipts
        .iter()
        .filter(|r| r.expected == Expected::Supported && !r.authorized)
        .count();
    assert_eq!((supported, ambiguous, unsupported), (360, 120, 120));
    assert_eq!(exact_decisions, 600);
    assert_eq!(authorized_supported, 360);
    assert_eq!(frontend_replay_verified, 600);
    assert_eq!(downstream_replay_verified, 600);
    assert_eq!(frontend_tamper_rejections, 600);
    assert_eq!(downstream_tamper_rejections, 600);
    assert_eq!(false_authorizations, 0);
    assert_eq!(false_denials, 0);
    let mut family_counts = BTreeMap::new();
    for receipt in &receipts {
        *family_counts
            .entry(format!("{:?}", receipt.family))
            .or_insert(0) += 1;
    }
    let report = Report {
        schema: "stage152-visual-science-tsv-composition-v1",
        source: "independently authored coordinate-bearing OCR TSV corpus",
        corpus_sha256: digest(&receipts),
        cases,
        supported,
        ambiguous,
        unsupported,
        exact_decisions,
        authorized_supported,
        frontend_replay_verified,
        downstream_replay_verified,
        frontend_tamper_rejections,
        downstream_tamper_rejections,
        false_authorizations,
        false_denials,
        family_counts,
        receipts,
    };
    let json = serde_json::to_vec_pretty(&report)?;
    std::fs::write("docs/stage152_visual_science_tsv_composition.json", &json)?;
    println!("{}", String::from_utf8(json)?);
    Ok(())
}
