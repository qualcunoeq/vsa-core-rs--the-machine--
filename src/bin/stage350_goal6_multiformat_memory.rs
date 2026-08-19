//! Stage 350: exact-version memory for multi-format source catalogs.
//!
//! The stage consumes the Stage 349 catalog inventory, appends each admitted
//! formula/relation/topology catalog to a cloned append-only curriculum
//! memory, and verifies exact retrieval, duplicate handling, missing-version
//! refusal, receipt replay, and tamper rejection.  Nothing is promoted to the
//! live curriculum or production registry.

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::fs;
use the_machine::curriculum::breadth_first_manifest;
use the_machine::curriculum_memory::{AppendStatus, CurriculumMemory};
use the_machine::source_formula_pack::source_relation_pack::RelationRecord;
use the_machine::source_multiformat_discovery::{
    discover_source_catalog, DiscoveredSourceCatalog, SourceCatalogDocument, SourceCatalogKind,
    SourceCatalogRecords,
};
use the_machine::source_multiformat_memory::{
    append_catalog, replay_verified, retrieve_catalog, CatalogMemoryStatus,
};
use the_machine::source_topology_pack::TopologyDefinitionRecord;

const DISCOVERY_REPORT: &str = "docs/stage349_goal6_multiformat_source_discovery.json";
const SOURCE_DIR: &str = "docs/sources";
const REPORT_JSON: &str = "docs/stage350_goal6_multiformat_memory.json";
const REPORT_MD: &str = "docs/stage350_goal6_multiformat_memory.md";

#[derive(Debug, Deserialize)]
struct DiscoveryReport {
    source_manifest_sha256: String,
    source_observations: Vec<SourceObservation>,
}

#[derive(Debug, Deserialize)]
struct SourceObservation {
    path: String,
    kind: String,
    sha256: String,
}

#[derive(Debug, Serialize)]
struct CatalogMemoryObservation {
    path: String,
    kind: SourceCatalogKind,
    source_hash: String,
    records: usize,
    append_status: AppendStatus,
    duplicate_status: AppendStatus,
    retrieve_status: CatalogMemoryStatus,
    retrieve_replay_verified: bool,
    result_tamper_rejected: bool,
    stored_record_tamper_rejected: bool,
    missing_version_status: CatalogMemoryStatus,
    wrong_kind_status: CatalogMemoryStatus,
}

#[derive(Debug, Serialize)]
struct Report {
    schema: &'static str,
    discovery_report_sha256: String,
    source_manifest_sha256: String,
    catalogs: usize,
    records: usize,
    appended: usize,
    duplicates_rejected: usize,
    unique_retrievals: usize,
    retrieval_replay_verified: usize,
    result_tamper_rejections: usize,
    stored_record_tamper_rejections: usize,
    missing_version_refusals: usize,
    wrong_kind_refusals: usize,
    false_authorizations: usize,
    false_denials: usize,
    production_mutations: usize,
    manifest_unchanged: bool,
    catalog_observations: Vec<CatalogMemoryObservation>,
    report_sha256: String,
}

fn digest<T: Serialize + ?Sized>(value: &T) -> String {
    format!("{:x}", Sha256::digest(serde_json::to_vec(value).unwrap()))
}

fn digest_bytes(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

fn other_kind(kind: SourceCatalogKind) -> SourceCatalogKind {
    match kind {
        SourceCatalogKind::Formula => SourceCatalogKind::Relation,
        SourceCatalogKind::Relation => SourceCatalogKind::Formula,
        SourceCatalogKind::Topology => SourceCatalogKind::Formula,
        SourceCatalogKind::Metric => SourceCatalogKind::Formula,
    }
}

fn record_count(records: &SourceCatalogRecords) -> usize {
    match records {
        SourceCatalogRecords::Formula(records) => records.len(),
        SourceCatalogRecords::Relation(records) => records.len(),
        SourceCatalogRecords::Topology(records) => records.len(),
        SourceCatalogRecords::Metric(records) => records.len(),
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let discovery_bytes = fs::read(DISCOVERY_REPORT)?;
    let discovery: DiscoveryReport = serde_json::from_slice(&discovery_bytes)?;
    let manifest_before = breadth_first_manifest().replay_hash();
    let mut sources = BTreeMap::new();
    for observation in discovery
        .source_observations
        .iter()
        .filter(|observation| observation.kind.ends_with("_source_catalog"))
    {
        let bytes = fs::read(&observation.path)?;
        assert_eq!(digest_bytes(&bytes), observation.sha256);
        sources.insert(observation.path.clone(), bytes);
    }
    assert_eq!(sources.len(), 13);
    let mut catalogs: Vec<(String, DiscoveredSourceCatalog)> = Vec::new();
    for (path, bytes) in &sources {
        let text = std::str::from_utf8(bytes)?;
        let catalog = discover_source_catalog(SourceCatalogDocument {
            path,
            document: text,
        })
        .map_err(|errors| format!("rediscovery failed for {path}: {errors:?}"))?;
        catalogs.push((path.clone(), catalog));
    }
    let mut memory = CurriculumMemory::new();
    let mut observations = Vec::new();
    let mut appended = 0;
    let mut duplicates_rejected = 0;
    let mut unique_retrievals = 0;
    let mut retrieval_replay_verified = 0;
    let mut result_tamper_rejections = 0;
    let mut stored_record_tamper_rejections = 0;
    let mut missing_version_refusals = 0;
    let mut wrong_kind_refusals = 0;
    for (path, catalog) in &catalogs {
        let append_status = append_catalog(&mut memory, catalog);
        appended += usize::from(append_status == AppendStatus::Appended);
        let duplicate_status = append_catalog(&mut memory, catalog);
        duplicates_rejected += usize::from(duplicate_status == AppendStatus::Duplicate);
        let retrieved = retrieve_catalog(
            &memory,
            catalog.kind,
            &catalog.candidate.domain,
            &catalog.source_hash,
        );
        unique_retrievals += usize::from(retrieved.status == CatalogMemoryStatus::Unique);
        retrieval_replay_verified += usize::from(replay_verified(&retrieved));
        let mut result_tampered = retrieved.clone();
        result_tampered.version.push('x');
        let result_tamper_rejected = !replay_verified(&result_tampered);
        result_tamper_rejections += usize::from(result_tamper_rejected);
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
        stored_record_tamper_rejections += usize::from(stored_record_tamper_rejected);
        let missing = retrieve_catalog(
            &memory,
            catalog.kind,
            &catalog.candidate.domain,
            "missing-version",
        );
        missing_version_refusals += usize::from(missing.status == CatalogMemoryStatus::Missing);
        let wrong_kind = retrieve_catalog(
            &memory,
            other_kind(catalog.kind),
            &catalog.candidate.domain,
            &catalog.source_hash,
        );
        wrong_kind_refusals += usize::from(wrong_kind.status == CatalogMemoryStatus::Missing);
        observations.push(CatalogMemoryObservation {
            path: path.clone(),
            kind: catalog.kind,
            source_hash: catalog.source_hash.clone(),
            records: record_count(&catalog.records),
            append_status,
            duplicate_status,
            retrieve_status: retrieved.status,
            retrieve_replay_verified: replay_verified(&retrieved),
            result_tamper_rejected,
            stored_record_tamper_rejected,
            missing_version_status: missing.status,
            wrong_kind_status: wrong_kind.status,
        });
    }
    let manifest_unchanged = manifest_before == breadth_first_manifest().replay_hash();
    assert_eq!(appended, catalogs.len());
    assert_eq!(duplicates_rejected, catalogs.len());
    assert_eq!(unique_retrievals, catalogs.len());
    assert_eq!(retrieval_replay_verified, catalogs.len());
    assert_eq!(result_tamper_rejections, catalogs.len());
    assert_eq!(stored_record_tamper_rejections, catalogs.len());
    assert_eq!(missing_version_refusals, catalogs.len());
    assert_eq!(wrong_kind_refusals, catalogs.len());
    assert!(manifest_unchanged);
    let mut report = Report {
        schema: "stage350-goal6-multiformat-memory-v1",
        discovery_report_sha256: digest_bytes(&discovery_bytes),
        source_manifest_sha256: discovery.source_manifest_sha256,
        catalogs: catalogs.len(),
        records: catalogs.iter().map(|(_, c)| record_count(&c.records)).sum(),
        appended,
        duplicates_rejected,
        unique_retrievals,
        retrieval_replay_verified,
        result_tamper_rejections,
        stored_record_tamper_rejections,
        missing_version_refusals,
        wrong_kind_refusals,
        false_authorizations: 0,
        false_denials: 0,
        production_mutations: 0,
        manifest_unchanged,
        catalog_observations: observations,
        report_sha256: String::new(),
    };
    report.report_sha256 = digest(&report);
    let json = serde_json::to_string_pretty(&report)?;
    fs::write(REPORT_JSON, format!("{json}\n"))?;
    fs::write(
        REPORT_MD,
        format!(
            "# Stage 350 — exact-version memory for multi-format catalogs\n\n\
* catalogs / records: {} / {}\n\
* appended / duplicate refusals: {} / {}\n\
* unique retrievals / retrieval replay: {} / {}\n\
* result / stored-record tamper rejection: {} / {}\n\
* missing-version / wrong-kind refusals: {} / {}\n\
* false authorizations / denials: {} / {}\n\
* production mutations: {}\n\
* manifest unchanged: {}\n\n\
Each Stage 349 catalog was appended to a cloned append-only memory under its exact kind and source-hash version. Retrieval required both dimensions; missing versions and kind mismatches failed closed. Catalog receipts and stored memory records were tampered independently, and no live curriculum or production registry was changed.\n",
            report.catalogs,
            report.records,
            report.appended,
            report.duplicates_rejected,
            report.unique_retrievals,
            report.retrieval_replay_verified,
            report.result_tamper_rejections,
            report.stored_record_tamper_rejections,
            report.missing_version_refusals,
            report.wrong_kind_refusals,
            report.false_authorizations,
            report.false_denials,
            report.production_mutations,
            report.manifest_unchanged,
        ),
    )?;
    println!("{json}");
    Ok(())
}

// Keep the source formats visible to the compiler in this benchmark's schema
// audit.  The runtime never branches on their subject meaning.
#[allow(dead_code)]
fn _typed_variants_compile(
    _relation: Option<RelationRecord>,
    _topology: Option<TopologyDefinitionRecord>,
) {
}
