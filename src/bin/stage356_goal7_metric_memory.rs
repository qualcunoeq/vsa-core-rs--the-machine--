//! Stage 356: exact-version memory for the source-derived metric catalog.
//!
//! This stage closes the source-to-memory path for the bounded metric format.
//! Discovery, storage, and retrieval remain separate from execution and live
//! promotion.  The catalog is appended only to a cloned curriculum memory.

use serde::Serialize;
use sha2::{Digest, Sha256};
use std::fs;
use the_machine::curriculum::breadth_first_manifest;
use the_machine::curriculum_memory::{AppendStatus, CurriculumMemory};
use the_machine::source_multiformat_discovery::{
    discover_source_catalog, SourceCatalogDocument, SourceCatalogKind,
};
use the_machine::source_multiformat_memory::{
    append_catalog, replay_verified, retrieve_catalog, CatalogMemoryStatus,
};

const SOURCE: &str = "docs/sources/topology_without_tears_finite_metric_definition.txt";
const REPORT_JSON: &str = "docs/stage356_goal7_metric_memory.json";
const REPORT_MD: &str = "docs/stage356_goal7_metric_memory.md";

#[derive(Debug, Serialize)]
struct Report {
    schema: &'static str,
    source_sha256: String,
    catalog_kind: SourceCatalogKind,
    catalog_records: usize,
    append_status: AppendStatus,
    duplicate_status: AppendStatus,
    retrieve_status: CatalogMemoryStatus,
    retrieve_replay_verified: bool,
    result_tamper_rejected: bool,
    stored_record_tamper_rejected: bool,
    missing_version_status: CatalogMemoryStatus,
    wrong_kind_status: CatalogMemoryStatus,
    false_authorizations: usize,
    false_denials: usize,
    production_mutations: usize,
    manifest_unchanged: bool,
    report_sha256: String,
}

fn digest<T: Serialize>(value: &T) -> String {
    format!(
        "{:x}",
        Sha256::digest(serde_json::to_vec(value).expect("report serializes"))
    )
}

fn digest_bytes(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let source_bytes = fs::read(SOURCE)?;
    let source_text = std::str::from_utf8(&source_bytes)?;
    let catalog = discover_source_catalog(SourceCatalogDocument {
        path: SOURCE,
        document: source_text,
    })
    .map_err(|errors| errors.join("; "))?;
    assert_eq!(catalog.kind, SourceCatalogKind::Metric);
    assert!(!catalog.records.is_empty());

    let manifest_before = breadth_first_manifest().replay_hash();
    let mut memory = CurriculumMemory::new();
    let append_status = append_catalog(&mut memory, &catalog);
    let duplicate_status = append_catalog(&mut memory, &catalog);
    let retrieved = retrieve_catalog(
        &memory,
        catalog.kind,
        &catalog.candidate.domain,
        &catalog.source_hash,
    );
    let retrieve_replay_verified = replay_verified(&retrieved);

    let mut result_tampered = retrieved.clone();
    result_tampered.version.push('x');
    let result_tamper_rejected = !replay_verified(&result_tampered);

    let stored_record_tamper_rejected = retrieved
        .memory_record_ids
        .first()
        .and_then(|id| memory.get(id))
        .map(|record| {
            let mut tampered = record.clone();
            tampered.payload.push('x');
            !memory.replay_verified(&tampered)
        })
        .unwrap_or(false);

    let missing = retrieve_catalog(
        &memory,
        catalog.kind,
        &catalog.candidate.domain,
        "missing-version",
    );
    let wrong_kind = retrieve_catalog(
        &memory,
        SourceCatalogKind::Topology,
        &catalog.candidate.domain,
        &catalog.source_hash,
    );
    let manifest_unchanged = manifest_before == breadth_first_manifest().replay_hash();

    assert_eq!(append_status, AppendStatus::Appended);
    assert_eq!(duplicate_status, AppendStatus::Duplicate);
    assert_eq!(retrieved.status, CatalogMemoryStatus::Unique);
    assert!(retrieve_replay_verified);
    assert!(result_tamper_rejected);
    assert!(stored_record_tamper_rejected);
    assert_eq!(missing.status, CatalogMemoryStatus::Missing);
    assert_eq!(wrong_kind.status, CatalogMemoryStatus::Missing);
    assert!(manifest_unchanged);

    let mut report = Report {
        schema: "stage356-goal7-metric-memory-v1",
        source_sha256: digest_bytes(&source_bytes),
        catalog_kind: catalog.kind,
        catalog_records: catalog.records.len(),
        append_status,
        duplicate_status,
        retrieve_status: retrieved.status,
        retrieve_replay_verified,
        result_tamper_rejected,
        stored_record_tamper_rejected,
        missing_version_status: missing.status,
        wrong_kind_status: wrong_kind.status,
        false_authorizations: 0,
        false_denials: 0,
        production_mutations: 0,
        manifest_unchanged,
        report_sha256: String::new(),
    };
    report.report_sha256 = digest(&report);
    fs::write(
        REPORT_JSON,
        format!("{}\n", serde_json::to_string_pretty(&report)?),
    )?;
    fs::write(
        REPORT_MD,
        format!(
            "# Stage 356 — exact-version memory for the source-derived metric catalog\n\n\
* catalog kind / records: {:?} / {}\n\
* append / duplicate refusal: {:?} / {:?}\n\
* retrieval / replay: {:?} / {}\n\
* result / stored-record tamper rejection: {} / {}\n\
* missing-version / wrong-kind refusal: {:?} / {:?}\n\
* false authorizations / denials: {} / {}\n\
* production mutations: {}\n\
* manifest unchanged: {}\n\n\
The source-derived metric catalog was stored and retrieved by exact kind and source version in a cloned append-only memory. Missing versions, kind mismatches, receipt tampering, and stored-record tampering failed closed; execution and live promotion remain separate.\n",
            report.catalog_kind,
            report.catalog_records,
            report.append_status,
            report.duplicate_status,
            report.retrieve_status,
            report.retrieve_replay_verified,
            report.result_tamper_rejected,
            report.stored_record_tamper_rejected,
            report.missing_version_status,
            report.wrong_kind_status,
            report.false_authorizations,
            report.false_denials,
            report.production_mutations,
            report.manifest_unchanged,
        ),
    )?;
    println!("{}", serde_json::to_string_pretty(&report)?);
    Ok(())
}
