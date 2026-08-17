//! Stage 111: generic source catalog ingestion and tamper audit.
use serde::Serialize;
use sha2::{Digest, Sha256};
use the_machine::source_catalog_ingestion::{ingest, replay_verified, SourceCatalog};
const SET: &str = include_str!("../../docs/sources/openstax_finite_set_operations_catalog.txt");
const COUNT: &str = include_str!("../../docs/sources/openstax_counting_principles_catalog.txt");
const LOGIC: &str = include_str!("../../docs/sources/openstax_truth_table_catalog.txt");
#[derive(Debug, Serialize)]
struct Receipt {
    source_id: String,
    operations: usize,
    expected_operations: usize,
    replay_verified: bool,
    tamper_rejected: bool,
    provenance_preserved: bool,
}
#[derive(Debug, Serialize)]
struct Report {
    schema: &'static str,
    catalogs: usize,
    valid_catalogs: usize,
    operations_declared: usize,
    replay_verified: usize,
    tamper_rejections: usize,
    provenance_preserved: usize,
    invalid_mutations_rejected: usize,
    source_hashes: Vec<String>,
    receipts: Vec<Receipt>,
}
fn digest(v: &str) -> String {
    format!("{:x}", Sha256::digest(v.as_bytes()))
}
fn check(document: &str, expected: usize) -> Receipt {
    let c = ingest(document).expect("catalog metadata parses");
    let mut tampered = c.clone();
    tampered.replay_hash.push('x');
    Receipt {
        source_id: c.citation.source_id.clone(),
        operations: c.operations.len(),
        expected_operations: expected,
        replay_verified: replay_verified(&c),
        tamper_rejected: !replay_verified(&tampered),
        provenance_preserved: !c.citation.evidence_span.is_empty(),
    }
}
fn main() {
    let docs = [(SET, 5), (COUNT, 4), (LOGIC, 4)];
    let receipts: Vec<_> = docs.iter().map(|(d, n)| check(d, *n)).collect();
    assert_eq!(receipts.len(), 3);
    assert!(receipts
        .iter()
        .all(|r| r.operations == r.expected_operations
            && r.replay_verified
            && r.tamper_rejected
            && r.provenance_preserved));
    let invalid = (0..8)
        .filter(|n| {
            ingest(
                &docs[*n % 3]
                    .0
                    .lines()
                    .take(*n)
                    .collect::<Vec<_>>()
                    .join("\n"),
            )
            .is_err()
        })
        .count();
    assert_eq!(invalid, 8);
    let report = Report {
        schema: "stage111-source-catalog-ingestion-v1",
        catalogs: 3,
        valid_catalogs: 3,
        operations_declared: receipts.iter().map(|r| r.operations).sum(),
        replay_verified: 3,
        tamper_rejections: 3,
        provenance_preserved: 3,
        invalid_mutations_rejected: invalid,
        source_hashes: docs.iter().map(|(d, _)| digest(d)).collect(),
        receipts,
    };
    println!("{}", serde_json::to_string_pretty(&report).unwrap());
}
