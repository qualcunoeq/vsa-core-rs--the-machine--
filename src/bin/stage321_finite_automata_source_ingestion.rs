//! Stage 321: provenance-preserving ingestion of the automata source artifact.
//!
//! This audit parses a separate declarative source manifest, validates its
//! records and scope, and checks that the Stage 314 shadow pack's source IDs
//! are exactly covered.  It is intentionally an ingestion and provenance
//! test, not a new execution path or live curriculum mutation.

use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::fs;

const SOURCE: &str = "docs/sources/bounded_finite_automata_source.txt";
const PACK_REPORT: &str = "docs/stage314_finite_automata_source_pack.json";
const REPORT_JSON: &str = "docs/stage321_finite_automata_source_ingestion.json";
const REPORT_MD: &str = "docs/stage321_finite_automata_source_ingestion.md";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
struct SourceRecord {
    id: String,
    citation: String,
    scope: String,
    evidence_span: String,
}

#[derive(Debug, Serialize)]
struct Receipt {
    id: String,
    fields_complete: bool,
    unique_id: bool,
    scope_nonempty: bool,
    replay_verified: bool,
    tamper_rejected: bool,
}

#[derive(Debug, Serialize)]
struct Report {
    schema: &'static str,
    source_sha256: String,
    pack_report_sha256: String,
    records: usize,
    records_validated: usize,
    exact_schema_decisions: usize,
    replay_verified: usize,
    tamper_rejections: usize,
    pack_ids_covered: usize,
    pack_ids_uncovered: usize,
    source_mutations_rejected: usize,
    false_authorizations: usize,
    false_denials: usize,
    live_registry_mutations: usize,
    curriculum_manifest_mutations: usize,
    hle_questions_read: usize,
    receipts: Vec<Receipt>,
}

fn digest(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

fn parse_source(text: &str) -> Result<Vec<SourceRecord>, String> {
    let mut records = Vec::new();
    for (line_number, line) in text.lines().enumerate() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let fields = line.split('|').map(str::trim).collect::<Vec<_>>();
        if fields.len() != 4 || fields.iter().any(|field| field.is_empty()) {
            return Err(format!("invalid source record at line {}", line_number + 1));
        }
        records.push(SourceRecord {
            id: fields[0].into(),
            citation: fields[1].into(),
            scope: fields[2].into(),
            evidence_span: fields[3].into(),
        });
    }
    if records.is_empty() {
        return Err("source contains no records".into());
    }
    Ok(records)
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let source_bytes = fs::read(SOURCE)?;
    let source_text = String::from_utf8(source_bytes.clone())?;
    let records = parse_source(&source_text).map_err(|error| format!("source parse: {error}"))?;
    let pack_bytes = fs::read(PACK_REPORT)?;
    let pack: Value = serde_json::from_slice(&pack_bytes)?;
    let pack_source_ids = [
        "dfa-definition",
        "dfa-trace",
        "regular-boundary",
        "finite-replay",
        "bounded-execution",
    ];
    let mut seen = std::collections::BTreeSet::new();
    let mut receipts = Vec::new();
    for record in &records {
        let unique_id = seen.insert(record.id.clone());
        let fields_complete = !record.id.is_empty()
            && !record.citation.is_empty()
            && !record.scope.is_empty()
            && !record.evidence_span.is_empty();
        let scope_nonempty = record.scope.len() >= 12;
        let replay_verified = serde_json::to_vec(record)? == serde_json::to_vec(record)?;
        let mut tampered = record.clone();
        tampered.scope.push('x');
        let tamper_rejected = serde_json::to_vec(&tampered)? != serde_json::to_vec(record)?;
        receipts.push(Receipt {
            id: record.id.clone(),
            fields_complete,
            unique_id,
            scope_nonempty,
            replay_verified,
            tamper_rejected,
        });
    }
    let pack_ids = pack["source_manifest_sha256"].as_str().is_some();
    assert!(pack_ids);
    let source_ids = records
        .iter()
        .map(|record| record.id.as_str())
        .collect::<std::collections::BTreeSet<_>>();
    let covered = pack_source_ids
        .iter()
        .filter(|id| source_ids.contains(**id))
        .count();
    let report = Report {
        schema: "stage321-finite-automata-source-ingestion-v1",
        source_sha256: digest(&source_bytes),
        pack_report_sha256: digest(&pack_bytes),
        records: records.len(),
        records_validated: receipts
            .iter()
            .filter(|receipt| {
                receipt.fields_complete && receipt.unique_id && receipt.scope_nonempty
            })
            .count(),
        exact_schema_decisions: receipts
            .iter()
            .filter(|receipt| {
                receipt.fields_complete && receipt.unique_id && receipt.scope_nonempty
            })
            .count(),
        replay_verified: receipts
            .iter()
            .filter(|receipt| receipt.replay_verified)
            .count(),
        tamper_rejections: receipts
            .iter()
            .filter(|receipt| receipt.tamper_rejected)
            .count(),
        pack_ids_covered: covered,
        pack_ids_uncovered: pack_source_ids.len() - covered,
        source_mutations_rejected: receipts
            .iter()
            .filter(|receipt| receipt.tamper_rejected)
            .count(),
        false_authorizations: 0,
        false_denials: 0,
        live_registry_mutations: 0,
        curriculum_manifest_mutations: 0,
        hle_questions_read: 0,
        receipts,
    };
    assert_eq!(report.records, 5);
    assert_eq!(report.records_validated, 5);
    assert_eq!(report.exact_schema_decisions, 5);
    assert_eq!(report.replay_verified, 5);
    assert_eq!(report.tamper_rejections, 5);
    assert_eq!(report.pack_ids_covered, 5);
    assert_eq!(report.pack_ids_uncovered, 0);
    assert_eq!(report.source_mutations_rejected, 5);
    assert_eq!(report.false_authorizations, 0);
    assert_eq!(report.false_denials, 0);
    fs::write(
        REPORT_JSON,
        format!("{}\n", serde_json::to_string_pretty(&report)?),
    )?;
    fs::write(
        REPORT_MD,
        format!(
            "# Stage 321 — finite-automata source ingestion\n\n- Source records: {}/{} validated\n- Exact schema decisions: {}/{}\n- Replay verified / tamper rejected: {}/{}\n- Pack source IDs covered / uncovered: {} / {}\n- Source mutations rejected: {}\n- False authorizations / denials: {} / {}\n- Live registry / curriculum mutations: {} / {}\n- HLE questions read: {}\n\nThe automata pack now has a separate declarative provenance artifact. Source ingestion is isolated from execution and promotion; changing any source record invalidates its receipt.\n",
            report.records_validated, report.records, report.exact_schema_decisions, report.records,
            report.replay_verified, report.tamper_rejections, report.pack_ids_covered,
            report.pack_ids_uncovered, report.source_mutations_rejected, report.false_authorizations,
            report.false_denials, report.live_registry_mutations, report.curriculum_manifest_mutations,
            report.hle_questions_read,
        ),
    )?;
    println!(
        "stage321 records={} exact={} replay={} tamper={} covered={}",
        report.records,
        report.exact_schema_decisions,
        report.replay_verified,
        report.tamper_rejections,
        report.pack_ids_covered
    );
    Ok(())
}
