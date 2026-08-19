//! Stage J composition benchmark: visual table observations into probability.

use serde::Serialize;
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use the_machine::vision::visual_table::visual_probability_bridge::{
    table_to_probability, BridgeStatus,
};
use the_machine::vision::visual_table::{formalize_table_tsv, TableStatus};

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
enum Expected {
    Supported,
    Ambiguous,
    Refused,
}

#[derive(Debug, Serialize)]
struct Receipt {
    id: String,
    expected: Expected,
    visual_status: TableStatus,
    bridge_status: Option<BridgeStatus>,
    authorized: bool,
    exact: bool,
    visual_replay_verified: bool,
    bridge_replay_verified: bool,
    visual_tamper_rejected: bool,
    bridge_tamper_rejected: bool,
    false_authorization: bool,
}

#[derive(Debug, Serialize)]
struct Report {
    schema: &'static str,
    corpus_sha256: String,
    cases: usize,
    supported: usize,
    ambiguous: usize,
    refused: usize,
    exact_decisions: usize,
    supported_authorizations: usize,
    visual_replay_verified: usize,
    bridge_emitted: usize,
    bridge_replay_verified: usize,
    visual_tamper_rejections: usize,
    bridge_tamper_rejections: usize,
    false_authorizations: usize,
    false_denials: usize,
    visual_status_counts: BTreeMap<String, usize>,
    bridge_status_counts: BTreeMap<String, usize>,
    receipts: Vec<Receipt>,
}

const HEADER: &str = "level\tpage\tblock\tpar\tline\tword\tleft\ttop\twidth\theight\tconf\ttext";

fn word(left: usize, top: usize, text: &str, width: usize) -> String {
    format!("5\t1\t1\t1\t1\t1\t{left}\t{top}\t{width}\t10\t90\t{text}")
}

fn table_text(headers: &[&str], rows: &[Vec<&str>]) -> String {
    let mut lines = vec![HEADER.into()];
    for (column, text) in headers.iter().enumerate() {
        lines.push(word(10 + column * 70, 10, text, 50));
    }
    for (row, values) in rows.iter().enumerate() {
        for (column, text) in values.iter().enumerate() {
            lines.push(word(10 + column * 70, 30 + row * 25, text, 50));
        }
    }
    lines.join("\n")
}

fn run(id: String, text: String, expected: Expected) -> Receipt {
    let visual = formalize_table_tsv(&text);
    let mut tampered_visual = visual.clone();
    tampered_visual.replay_hash.push('x');
    let (bridge_status, authorized, bridge_replay_verified, bridge_tamper_rejected) =
        if let Some(table) = visual.artifact.clone() {
            let bridge = table_to_probability(&table);
            let mut tampered_bridge = bridge.clone();
            tampered_bridge.replay_hash.push('x');
            (
                Some(bridge.status),
                bridge.authorized(),
                bridge.replay_verified(),
                !tampered_bridge.replay_verified(),
            )
        } else {
            (None, false, true, true)
        };
    let exact = match expected {
        Expected::Supported => authorized,
        Expected::Ambiguous => {
            !authorized && matches!(bridge_status, Some(BridgeStatus::Ambiguous))
        }
        Expected::Refused => !authorized,
    };
    Receipt {
        id,
        expected,
        visual_status: visual.status,
        bridge_status,
        authorized,
        exact,
        visual_replay_verified: visual.replay_verified(),
        bridge_replay_verified,
        visual_tamper_rejected: !tampered_visual.replay_verified(),
        bridge_tamper_rejected,
        false_authorization: expected != Expected::Supported && authorized,
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut receipts = Vec::with_capacity(240);
    for index in 0..120 {
        let text = table_text(
            &["outcome", "probability"],
            &[vec!["a", "1/2"], vec!["b", "1/3"], vec!["c", "1/6"]],
        );
        receipts.push(run(
            format!("supported_{index:03}"),
            text,
            Expected::Supported,
        ));
    }
    for index in 0..40 {
        let text = table_text(&["value", "weight"], &[vec!["a", "1/2"], vec!["b", "1/2"]]);
        receipts.push(run(
            format!("ambiguous_{index:03}"),
            text,
            Expected::Ambiguous,
        ));
    }
    let refused = [
        table_text(
            &["outcome", "probability"],
            &[vec!["a", "1/3"], vec!["b", "1/3"]],
        ),
        table_text(
            &["outcome", "density"],
            &[vec!["a", "1/2"], vec!["b", "1/2"]],
        ),
        table_text(
            &["outcome", "probability", "time"],
            &[vec!["a", "1/2", "0"], vec!["b", "1/2", "1"]],
        ),
        table_text(
            &["outcome", "probability"],
            &[vec!["a", "0.5"], vec!["b", "0.5"]],
        ),
    ];
    for index in 0..80 {
        receipts.push(run(
            format!("refused_{index:03}"),
            refused[index % refused.len()].clone(),
            Expected::Refused,
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
    let refused = receipts
        .iter()
        .filter(|r| r.expected == Expected::Refused)
        .count();
    let exact_decisions = receipts.iter().filter(|r| r.exact).count();
    let supported_authorizations = receipts
        .iter()
        .filter(|r| r.expected == Expected::Supported && r.authorized)
        .count();
    let visual_replay_verified = receipts.iter().filter(|r| r.visual_replay_verified).count();
    let bridge_emitted = receipts
        .iter()
        .filter(|r| r.bridge_status.is_some())
        .count();
    let bridge_replay_verified = receipts
        .iter()
        .filter(|r| r.bridge_status.is_some() && r.bridge_replay_verified)
        .count();
    let visual_tamper_rejections = receipts.iter().filter(|r| r.visual_tamper_rejected).count();
    let bridge_tamper_rejections = receipts
        .iter()
        .filter(|r| r.bridge_status.is_some() && r.bridge_tamper_rejected)
        .count();
    let false_authorizations = receipts.iter().filter(|r| r.false_authorization).count();
    let false_denials = receipts
        .iter()
        .filter(|r| r.expected == Expected::Supported && !r.exact)
        .count();
    let mut visual_status_counts = BTreeMap::new();
    let mut bridge_status_counts = BTreeMap::new();
    for receipt in &receipts {
        *visual_status_counts
            .entry(format!("{:?}", receipt.visual_status))
            .or_insert(0usize) += 1;
        if let Some(status) = receipt.bridge_status {
            *bridge_status_counts
                .entry(format!("{:?}", status))
                .or_insert(0usize) += 1;
        }
    }
    assert_eq!((supported, ambiguous, refused), (120, 40, 80));
    assert_eq!(exact_decisions, cases);
    assert_eq!(supported_authorizations, supported);
    assert_eq!(visual_replay_verified, cases);
    assert_eq!(bridge_replay_verified, bridge_emitted);
    assert_eq!(visual_tamper_rejections, cases);
    assert_eq!(bridge_tamper_rejections, bridge_emitted);
    assert_eq!(false_authorizations, 0);
    assert_eq!(false_denials, 0);
    let report = Report {
        schema: "stage-j-visual-probability-bridge-v1",
        corpus_sha256: format!("{:x}", Sha256::digest(serde_json::to_vec(&receipts)?)),
        cases,
        supported,
        ambiguous,
        refused,
        exact_decisions,
        supported_authorizations,
        visual_replay_verified,
        bridge_emitted,
        bridge_replay_verified,
        visual_tamper_rejections,
        bridge_tamper_rejections,
        false_authorizations,
        false_denials,
        visual_status_counts,
        bridge_status_counts,
        receipts,
    };
    let serialized = serde_json::to_string_pretty(&report)?;
    std::fs::write(
        "docs/stage_j_visual_probability_bridge.json",
        format!("{serialized}\n"),
    )?;
    println!("{serialized}");
    Ok(())
}
