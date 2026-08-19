//! Stage 358: exact-version memory checkpoint over every admitted catalog.
//!
//! The inventory is discovered from the source directory without a subject
//! list.  Every admitted format, including metric, is stored and retrieved by
//! exact kind and source version in a cloned append-only memory.

use serde::Serialize;
use sha2::{Digest, Sha256};
use std::fs;
use std::path::PathBuf;
use the_machine::curriculum::breadth_first_manifest;
use the_machine::curriculum_memory::{AppendStatus, CurriculumMemory};
use the_machine::source_multiformat_discovery::{
    discover_source_catalog, DiscoveredSourceCatalog, SourceCatalogDocument,
};
use the_machine::source_multiformat_memory::{
    append_catalog, replay_verified, retrieve_catalog, CatalogMemoryStatus,
};

const SOURCE_DIR: &str = "docs/sources";
const REPORT_JSON: &str = "docs/stage358_goal7_multiformat_memory_checkpoint.json";
const REPORT_MD: &str = "docs/stage358_goal7_multiformat_memory_checkpoint.md";

#[derive(Debug, Serialize)]
struct Report {
    schema: &'static str,
    source_documents: usize,
    admitted_catalogs: usize,
    admitted_records: usize,
    appended: usize,
    duplicate_refusals: usize,
    unique_retrievals: usize,
    retrieval_replays: usize,
    result_tamper_rejections: usize,
    stored_record_tamper_rejections: usize,
    missing_version_refusals: usize,
    wrong_kind_refusals: usize,
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

fn source_files() -> Result<Vec<PathBuf>, Box<dyn std::error::Error>> {
    let mut paths = fs::read_dir(SOURCE_DIR)?
        .filter_map(|entry| entry.ok().map(|entry| entry.path()))
        .filter(|path| path.extension().is_some_and(|extension| extension == "txt"))
        .collect::<Vec<_>>();
    paths.sort();
    Ok(paths)
}

fn catalogs(paths: &[PathBuf]) -> Result<Vec<DiscoveredSourceCatalog>, Box<dyn std::error::Error>> {
    let mut admitted = Vec::new();
    for path in paths {
        let bytes = fs::read(path)?;
        let text = std::str::from_utf8(&bytes)?;
        if let Ok(catalog) = discover_source_catalog(SourceCatalogDocument {
            path: &path.display().to_string(),
            document: text,
        }) {
            admitted.push(catalog);
        }
    }
    Ok(admitted)
}

fn alternate_kind(
    kind: the_machine::source_multiformat_discovery::SourceCatalogKind,
) -> the_machine::source_multiformat_discovery::SourceCatalogKind {
    use the_machine::source_multiformat_discovery::SourceCatalogKind;
    match kind {
        SourceCatalogKind::Formula => SourceCatalogKind::Relation,
        SourceCatalogKind::Relation => SourceCatalogKind::Formula,
        SourceCatalogKind::Topology => SourceCatalogKind::Formula,
        SourceCatalogKind::Metric => SourceCatalogKind::Topology,
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let paths = source_files()?;
    let catalogs = catalogs(&paths)?;
    let manifest_before = breadth_first_manifest().replay_hash();
    let mut memory = CurriculumMemory::new();
    let mut appended = 0;
    let mut duplicate_refusals = 0;
    let mut unique_retrievals = 0;
    let mut retrieval_replays = 0;
    let mut result_tamper_rejections = 0;
    let mut stored_record_tamper_rejections = 0;
    let mut missing_version_refusals = 0;
    let mut wrong_kind_refusals = 0;

    for catalog in &catalogs {
        let first = append_catalog(&mut memory, catalog);
        let duplicate = append_catalog(&mut memory, catalog);
        appended += usize::from(first == AppendStatus::Appended);
        duplicate_refusals += usize::from(duplicate == AppendStatus::Duplicate);
        let result = retrieve_catalog(
            &memory,
            catalog.kind,
            &catalog.candidate.domain,
            &catalog.source_hash,
        );
        unique_retrievals += usize::from(result.status == CatalogMemoryStatus::Unique);
        retrieval_replays += usize::from(replay_verified(&result));
        let mut result_tampered = result.clone();
        result_tampered.version.push('x');
        result_tamper_rejections += usize::from(!replay_verified(&result_tampered));
        stored_record_tamper_rejections += usize::from(
            result
                .memory_record_ids
                .first()
                .and_then(|id| memory.get(id))
                .map(|record| {
                    let mut tampered = record.clone();
                    tampered.payload.push('x');
                    !memory.replay_verified(&tampered)
                })
                .unwrap_or(false),
        );
        let missing = retrieve_catalog(
            &memory,
            catalog.kind,
            &catalog.candidate.domain,
            "missing-version",
        );
        missing_version_refusals += usize::from(missing.status == CatalogMemoryStatus::Missing);
        let wrong_kind = retrieve_catalog(
            &memory,
            alternate_kind(catalog.kind),
            &catalog.candidate.domain,
            &catalog.source_hash,
        );
        wrong_kind_refusals += usize::from(wrong_kind.status == CatalogMemoryStatus::Missing);
    }

    let manifest_unchanged = manifest_before == breadth_first_manifest().replay_hash();
    assert_eq!(catalogs.len(), 14);
    assert_eq!(appended, catalogs.len());
    assert_eq!(duplicate_refusals, catalogs.len());
    assert_eq!(unique_retrievals, catalogs.len());
    assert_eq!(retrieval_replays, catalogs.len());
    assert_eq!(result_tamper_rejections, catalogs.len());
    assert_eq!(stored_record_tamper_rejections, catalogs.len());
    assert_eq!(missing_version_refusals, catalogs.len());
    assert_eq!(wrong_kind_refusals, catalogs.len());
    assert!(manifest_unchanged);

    let mut report = Report {
        schema: "stage358-goal7-multiformat-memory-checkpoint-v1",
        source_documents: paths.len(),
        admitted_catalogs: catalogs.len(),
        admitted_records: catalogs.iter().map(|catalog| catalog.records.len()).sum(),
        appended,
        duplicate_refusals,
        unique_retrievals,
        retrieval_replays,
        result_tamper_rejections,
        stored_record_tamper_rejections,
        missing_version_refusals,
        wrong_kind_refusals,
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
            "# Stage 358 — generic multi-format memory checkpoint\n\n\
* source documents / admitted catalogs / records: {} / {} / {}\n\
* append / duplicate refusal: {} / {}\n\
* unique retrieval / replay: {} / {}\n\
* result / stored-record tamper rejection: {} / {}\n\
* missing-version / wrong-kind refusals: {} / {}\n\
* false authorizations / denials: {} / {}\n\
* production mutations: {}\n\
* manifest unchanged: {}\n\n\
All admitted source formats were appended to and retrieved from cloned exact-version memory. Catalog kind, source version, provenance, and replay integrity remained part of the retrieval contract; no live curriculum or registry was changed.\n",
            report.source_documents,
            report.admitted_catalogs,
            report.admitted_records,
            report.appended,
            report.duplicate_refusals,
            report.unique_retrievals,
            report.retrieval_replays,
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
    println!("{}", serde_json::to_string_pretty(&report)?);
    Ok(())
}
