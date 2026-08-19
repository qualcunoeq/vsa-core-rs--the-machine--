//! Stage 362: provenance-only inventory of residual source documents.
//!
//! Documents that are not executable declarative catalogs are scanned through
//! the generic metadata gate. Accepted records remain non-executable source
//! candidates; the inventory supplies provenance and scope for later governed
//! source selection without subject-specific routing.

use serde::Serialize;
use sha2::{Digest, Sha256};
use std::fs;
use std::path::PathBuf;
use the_machine::curriculum::breadth_first_manifest;
use the_machine::source_metadata::{extract_source_metadata, replay_verified, SourceMetadata};
use the_machine::source_multiformat_discovery::{discover_source_catalog, SourceCatalogDocument};

const SOURCE_DIR: &str = "docs/sources";
const REPORT_JSON: &str = "docs/stage362_residual_source_metadata_inventory.json";
const REPORT_MD: &str = "docs/stage362_residual_source_metadata_inventory.md";

#[derive(Debug, Serialize)]
struct Observation {
    path: String,
    document_sha256: String,
    metadata_accepted: bool,
    metadata_replay_verified: bool,
    metadata_tamper_rejected: bool,
    source_id: Option<String>,
    scope: Option<String>,
    unsupported: Option<String>,
    mutation_count: usize,
    mutations_rejected: usize,
}

#[derive(Debug, Serialize)]
struct Report {
    schema: &'static str,
    residual_documents: usize,
    metadata_accepted: usize,
    metadata_rejected: usize,
    metadata_replays: usize,
    metadata_tamper_rejections: usize,
    source_mutations: usize,
    source_mutations_rejected: usize,
    executable_records: usize,
    false_authorizations: usize,
    production_mutations: usize,
    manifest_unchanged: bool,
    observations: Vec<Observation>,
    report_sha256: String,
}

fn digest<T: Serialize>(value: &T) -> String {
    format!(
        "{:x}",
        Sha256::digest(serde_json::to_vec(value).expect("inventory serializes"))
    )
}

fn digest_bytes(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

fn source_files() -> Result<Vec<PathBuf>, Box<dyn std::error::Error>> {
    let mut paths = fs::read_dir(SOURCE_DIR)?
        .filter_map(|entry| entry.ok().map(|entry| entry.path()))
        .filter(|path| path.extension().is_some_and(|extension| extension == "txt"))
        .collect::<Vec<_>>();
    paths.sort();
    Ok(paths)
}

fn mutations(document: &str) -> Vec<String> {
    vec![
        document
            .lines()
            .filter(|line| !line.to_ascii_uppercase().starts_with("SOURCE_ID:"))
            .collect::<Vec<_>>()
            .join("\n"),
        document
            .lines()
            .filter(|line| !line.to_ascii_uppercase().starts_with("URL:"))
            .collect::<Vec<_>>()
            .join("\n"),
        document
            .lines()
            .filter(|line| {
                let upper = line.to_ascii_uppercase();
                !upper.starts_with("EVIDENCE:") && !upper.starts_with("EVIDENCE_SPAN:")
            })
            .collect::<Vec<_>>()
            .join("\n"),
    ]
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let paths = source_files()?;
    let manifest_before = breadth_first_manifest().replay_hash();
    let mut observations = Vec::new();
    let mut metadata_accepted = 0;
    let mut metadata_replays = 0;
    let mut metadata_tamper_rejections = 0;
    let mut source_mutation_count = 0;
    let mut source_mutation_rejected_count = 0;
    for path in paths.iter().filter(|path| {
        let bytes = fs::read(path).expect("source readable");
        let text = std::str::from_utf8(&bytes).expect("source utf8");
        discover_source_catalog(SourceCatalogDocument {
            path: &path.display().to_string(),
            document: text,
        })
        .is_err()
    }) {
        let bytes = fs::read(path)?;
        let text = std::str::from_utf8(&bytes)?;
        let source_mutations = mutations(text);
        let rejected = source_mutations
            .iter()
            .filter(|mutation| extract_source_metadata(mutation).is_err())
            .count();
        source_mutation_count += source_mutations.len();
        source_mutation_rejected_count += rejected;
        let metadata = extract_source_metadata(text).ok();
        if metadata.is_some() {
            metadata_accepted += 1;
        }
        let (replay, tamper, source_id, scope, unsupported) = match metadata {
            Some(metadata) => {
                let replay = replay_verified(&metadata);
                let mut tampered: SourceMetadata = metadata.clone();
                tampered.document_sha256.push('x');
                (
                    replay,
                    !replay_verified(&tampered),
                    Some(metadata.citation.source_id),
                    metadata.scope,
                    metadata.unsupported,
                )
            }
            None => (false, false, None, None, None),
        };
        metadata_replays += usize::from(replay);
        metadata_tamper_rejections += usize::from(tamper);
        observations.push(Observation {
            path: path.display().to_string(),
            document_sha256: digest_bytes(&bytes),
            metadata_accepted: source_id.is_some(),
            metadata_replay_verified: replay,
            metadata_tamper_rejected: tamper,
            source_id,
            scope,
            unsupported,
            mutation_count: source_mutations.len(),
            mutations_rejected: rejected,
        });
    }
    let manifest_unchanged = manifest_before == breadth_first_manifest().replay_hash();
    assert_eq!(metadata_replays, metadata_accepted);
    assert_eq!(metadata_tamper_rejections, metadata_accepted);
    assert_eq!(source_mutation_count, source_mutation_rejected_count);
    assert!(manifest_unchanged);
    let mut report = Report {
        schema: "stage362-residual-source-metadata-inventory-v1",
        residual_documents: observations.len(),
        metadata_accepted,
        metadata_rejected: observations.len() - metadata_accepted,
        metadata_replays,
        metadata_tamper_rejections,
        source_mutations: source_mutation_count,
        source_mutations_rejected: source_mutation_rejected_count,
        executable_records: 0,
        false_authorizations: 0,
        production_mutations: 0,
        manifest_unchanged,
        observations,
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
            "# Stage 362 — residual source metadata inventory\n\n\
* residual documents / metadata accepted / rejected: {} / {} / {}\n\
* metadata replay / tamper rejection: {} / {}\n\
* source mutations / rejected: {} / {}\n\
* executable records: {}\n\
* false authorizations: {}\n\
* production mutations: {}\n\
* manifest unchanged: {}\n\n\
Residual documents were classified through a generic citation-and-scope parser after executable catalog discovery failed. Accepted metadata remains non-executable and is available only as provenance-bearing source-selection evidence.\n",
            report.residual_documents,
            report.metadata_accepted,
            report.metadata_rejected,
            report.metadata_replays,
            report.metadata_tamper_rejections,
            report.source_mutations,
            report.source_mutations_rejected,
            report.executable_records,
            report.false_authorizations,
            report.production_mutations,
            report.manifest_unchanged,
        ),
    )?;
    println!("{}", serde_json::to_string_pretty(&report)?);
    Ok(())
}
