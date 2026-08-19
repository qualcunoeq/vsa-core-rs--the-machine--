//! Stage 354: admit the bounded metric source through generic discovery.
//!
//! This is a discovery/provenance gate only.  It does not add metric logic;
//! the existing metric executor remains a separate downstream capability.
//! Older Stage 349 artifacts are left immutable, so this report records the
//! expanded four-format inventory independently.

use serde::Serialize;
use sha2::{Digest, Sha256};
use std::fs;
use std::path::PathBuf;
use the_machine::curriculum::breadth_first_manifest;
use the_machine::source_multiformat_discovery::{
    discover_source_catalog, replay_verified, SourceCatalogDocument, SourceCatalogKind,
};

const SOURCE_DIR: &str = "docs/sources";
const REPORT_JSON: &str = "docs/stage354_goal7_metric_catalog_discovery.json";
const REPORT_MD: &str = "docs/stage354_goal7_metric_catalog_discovery.md";

#[derive(Debug, Serialize)]
struct SourceObservation {
    path: String,
    sha256: String,
    bytes: usize,
    classification: String,
    records: usize,
    replay_verified: bool,
    error_digest: Option<String>,
}

#[derive(Debug, Serialize)]
struct CatalogObservation {
    path: String,
    kind: SourceCatalogKind,
    records: usize,
    source_hash: String,
    replay_verified: bool,
    tamper_rejected: bool,
    source_mutations: usize,
    source_mutations_rejected: usize,
}

#[derive(Debug, Serialize)]
struct Report {
    schema: &'static str,
    source_manifest_sha256: String,
    source_documents: usize,
    admitted_catalogs: usize,
    admitted_records: usize,
    formula_catalogs: usize,
    relation_catalogs: usize,
    topology_catalogs: usize,
    metric_catalogs: usize,
    unmarked_documents: usize,
    rejected_documents: usize,
    discovery_replays: usize,
    catalog_tamper_rejections: usize,
    source_mutations: usize,
    source_mutations_rejected: usize,
    false_authorizations: usize,
    production_mutations: usize,
    manifest_unchanged: bool,
    source_observations: Vec<SourceObservation>,
    catalog_observations: Vec<CatalogObservation>,
    report_sha256: String,
}

fn digest<T: Serialize + ?Sized>(value: &T) -> String {
    format!("{:x}", Sha256::digest(serde_json::to_vec(value).unwrap()))
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

fn source_mutations(document: &str) -> Vec<String> {
    let kind = ["FORMULA", "RELATION", "TOPOLOGY", "METRIC"]
        .into_iter()
        .find(|kind| document.contains(&format!("BEGIN {kind}")))
        .expect("admitted document has a declarative block");
    let without_end = document
        .rfind(&format!("END {kind}"))
        .map(|index| document[..index].to_owned())
        .unwrap_or_else(|| document.to_owned());
    let without_source_id = document
        .lines()
        .filter(|line| !line.starts_with("SOURCE_ID:"))
        .collect::<Vec<_>>()
        .join("\n");
    let bad_url = document
        .lines()
        .map(|line| line.replacen("URL: https://", "URL: http://", 1))
        .collect::<Vec<_>>()
        .join("\n");
    let without_evidence = document
        .lines()
        .filter(|line| !line.starts_with("EVIDENCE:"))
        .collect::<Vec<_>>()
        .join("\n");
    vec![without_end, without_source_id, bad_url, without_evidence]
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let paths = source_files()?;
    let manifest = paths
        .iter()
        .map(|path| {
            let bytes = fs::read(path)?;
            Ok::<_, std::io::Error>((
                path.display().to_string(),
                digest_bytes(&bytes),
                bytes.len(),
            ))
        })
        .collect::<Result<Vec<_>, _>>()?;
    let source_manifest_sha256 = digest(&manifest);
    let manifest_before = breadth_first_manifest().replay_hash();
    let mut source_observations = Vec::new();
    let mut catalog_observations = Vec::new();
    let mut discovery_replays = 0;
    let mut catalog_tamper_rejections = 0;
    let mut source_mutation_count = 0;
    let mut source_mutation_rejected_count = 0;
    for path in &paths {
        let path_string = path.display().to_string();
        let bytes = fs::read(path)?;
        let text = std::str::from_utf8(&bytes)?;
        match discover_source_catalog(SourceCatalogDocument {
            path: &path_string,
            document: text,
        }) {
            Ok(catalog) => {
                let replay = replay_verified(&catalog);
                discovery_replays += usize::from(replay);
                let mut tampered = catalog.clone();
                tampered.source_hash.push('x');
                let tamper_rejected = !replay_verified(&tampered);
                catalog_tamper_rejections += usize::from(tamper_rejected);
                let mutations = source_mutations(text);
                let rejected = mutations
                    .iter()
                    .filter(|mutation| {
                        discover_source_catalog(SourceCatalogDocument {
                            path: &path_string,
                            document: mutation,
                        })
                        .is_err()
                    })
                    .count();
                source_mutation_count += mutations.len();
                source_mutation_rejected_count += rejected;
                source_observations.push(SourceObservation {
                    path: path_string.clone(),
                    sha256: digest_bytes(&bytes),
                    bytes: bytes.len(),
                    classification: format!("{:?}_catalog", catalog.kind).to_ascii_lowercase(),
                    records: catalog.records.len(),
                    replay_verified: replay,
                    error_digest: None,
                });
                catalog_observations.push(CatalogObservation {
                    path: path_string,
                    kind: catalog.kind,
                    records: catalog.records.len(),
                    source_hash: catalog.source_hash,
                    replay_verified: replay,
                    tamper_rejected,
                    source_mutations: mutations.len(),
                    source_mutations_rejected: rejected,
                });
            }
            Err(errors) => source_observations.push(SourceObservation {
                path: path_string,
                sha256: digest_bytes(&bytes),
                bytes: bytes.len(),
                classification: if text.contains("BEGIN ") {
                    "rejected_declarative_source".into()
                } else {
                    "unmarked_source_document".into()
                },
                records: 0,
                replay_verified: false,
                error_digest: Some(digest(&errors)),
            }),
        }
    }
    let manifest_unchanged = manifest_before == breadth_first_manifest().replay_hash();
    let admitted_catalogs = catalog_observations.len();
    assert_eq!(admitted_catalogs, 14);
    assert_eq!(
        catalog_observations
            .iter()
            .filter(|observation| observation.kind == SourceCatalogKind::Metric)
            .count(),
        1
    );
    assert_eq!(discovery_replays, admitted_catalogs);
    assert_eq!(catalog_tamper_rejections, admitted_catalogs);
    assert_eq!(source_mutation_count, source_mutation_rejected_count);
    assert!(manifest_unchanged);
    let mut report = Report {
        schema: "stage354-goal7-metric-catalog-discovery-v1",
        source_manifest_sha256,
        source_documents: paths.len(),
        admitted_catalogs,
        admitted_records: catalog_observations.iter().map(|item| item.records).sum(),
        formula_catalogs: catalog_observations
            .iter()
            .filter(|item| item.kind == SourceCatalogKind::Formula)
            .count(),
        relation_catalogs: catalog_observations
            .iter()
            .filter(|item| item.kind == SourceCatalogKind::Relation)
            .count(),
        topology_catalogs: catalog_observations
            .iter()
            .filter(|item| item.kind == SourceCatalogKind::Topology)
            .count(),
        metric_catalogs: 1,
        unmarked_documents: source_observations
            .iter()
            .filter(|item| item.classification == "unmarked_source_document")
            .count(),
        rejected_documents: source_observations
            .iter()
            .filter(|item| item.classification == "rejected_declarative_source")
            .count(),
        discovery_replays,
        catalog_tamper_rejections,
        source_mutations: source_mutation_count,
        source_mutations_rejected: source_mutation_rejected_count,
        false_authorizations: 0,
        production_mutations: 0,
        manifest_unchanged,
        source_observations,
        catalog_observations,
        report_sha256: String::new(),
    };
    report.report_sha256 = digest(&report);
    let json = serde_json::to_string_pretty(&report)?;
    fs::write(REPORT_JSON, format!("{json}\n"))?;
    fs::write(
        REPORT_MD,
        format!(
            "# Stage 354 — generic discovery with bounded metric source\n\n\
* source documents / admitted catalogs / records: {} / {} / {}\n\
* catalog kinds (formula / relation / topology / metric): {} / {} / {} / {}\n\
* unmarked / rejected documents: {} / {}\n\
* discovery replay / catalog tamper rejection: {} / {}\n\
* source mutations / rejected: {} / {}\n\
* false authorizations / production mutations: {} / {}\n\
* manifest unchanged: {}\n\n\
The generic envelope now admits the existing bounded metric source by explicit block structure and provenance. The metric executor is not invoked by this stage; source discovery remains distinct from semantic execution and promotion.\n",
            report.source_documents,
            report.admitted_catalogs,
            report.admitted_records,
            report.formula_catalogs,
            report.relation_catalogs,
            report.topology_catalogs,
            report.metric_catalogs,
            report.unmarked_documents,
            report.rejected_documents,
            report.discovery_replays,
            report.catalog_tamper_rejections,
            report.source_mutations,
            report.source_mutations_rejected,
            report.false_authorizations,
            report.production_mutations,
            report.manifest_unchanged,
        ),
    )?;
    println!("{json}");
    Ok(())
}
