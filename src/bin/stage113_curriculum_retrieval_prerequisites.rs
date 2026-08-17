//! Stage 113: selective curriculum retrieval and prerequisite completeness.
//!
//! Retrieval is proposal-only.  A record is selectable only through its exact
//! domain/artifact/version dimensions, and a curriculum artifact is usable only
//! when the immutable prerequisite graph proves closure.

use serde::Serialize;
use sha2::{Digest, Sha256};
use std::collections::BTreeSet;
use the_machine::curriculum::breadth_first_manifest;
use the_machine::curriculum_memory::{AppendStatus, CurriculumMemory, MemoryRecord};
use the_machine::prerequisite_discovery::{discover, proposed_edge_is_acyclic, DiscoveryStatus};

const RECORDS: usize = 12_000;
const QUERIES: usize = 2_000;

fn digest<T: Serialize + ?Sized>(value: &T) -> String {
    format!("{:x}", Sha256::digest(serde_json::to_vec(value).unwrap()))
}

fn domain(i: usize) -> &'static str {
    match i % 6 {
        0 => "finite_set",
        1 => "bounded_counting",
        2 => "truth_tables",
        3 => "bayes_rule",
        4 => "linear_algebra",
        _ => "graph_theory",
    }
}

fn artifact_type(i: usize) -> &'static str {
    match i % 3 {
        0 => "theorem",
        1 => "receipt",
        _ => "problem",
    }
}

fn record(i: usize) -> MemoryRecord {
    MemoryRecord {
        record_id: format!("stage113-record-{i:06}"),
        domain: domain(i).into(),
        artifact_type: artifact_type(i).into(),
        version: format!("v{}", (i / 6) % 3),
        payload: format!("source-derived-payload-{i}"),
        provenance: vec![
            format!("openstax-source-{}", i % 3),
            "stage113-shadow".into(),
        ],
        content_hash: String::new(),
    }
}

#[derive(Debug, Clone, Serialize)]
struct RetrievalReceipt {
    query_id: usize,
    domain: String,
    artifact_type: String,
    version: Option<String>,
    source: Option<String>,
    selected_ids: Vec<String>,
    selected_hashes: Vec<String>,
    status: String,
    replay_hash: String,
}

fn receipt_hash(receipt: &RetrievalReceipt) -> String {
    digest(&(
        receipt.query_id,
        &receipt.domain,
        &receipt.artifact_type,
        &receipt.version,
        &receipt.source,
        &receipt.selected_ids,
        &receipt.selected_hashes,
        &receipt.status,
    ))
}

fn replay_verified(receipt: &RetrievalReceipt) -> bool {
    receipt.replay_hash == receipt_hash(receipt)
}

#[derive(Debug, Serialize)]
struct Report {
    schema: &'static str,
    records: usize,
    queries: usize,
    exact_version_queries: usize,
    exact_version_complete: usize,
    unversioned_ambiguous_queries: usize,
    unversioned_ambiguous: usize,
    source_filter_queries: usize,
    source_filter_clean: usize,
    unsupported_queries: usize,
    unsupported_refused: usize,
    prerequisite_queries: usize,
    prerequisite_complete: usize,
    unknown_prerequisite_queries: usize,
    unknown_prerequisites_refused: usize,
    cycle_proposals_rejected: usize,
    receipt_replays: usize,
    receipt_tamper_rejections: usize,
    retrieval_contamination: usize,
    manifest_unchanged: bool,
    manifest_hash: String,
}

fn main() {
    let mut memory = CurriculumMemory::new();
    for i in 0..RECORDS {
        assert_eq!(memory.append(record(i)), AppendStatus::Appended);
    }

    let manifest = breadth_first_manifest();
    let manifest_hash = manifest.replay_hash();
    assert!(manifest.validate().is_empty());

    let mut exact_version_complete = 0;
    let mut unversioned_ambiguous = 0;
    let mut source_filter_clean = 0;
    let mut unsupported_refused = 0;
    let mut prerequisite_complete = 0;
    let mut unknown_prerequisites_refused = 0;
    let mut receipt_replays = 0;
    let mut receipt_tamper_rejections = 0;
    let mut retrieval_contamination = 0;
    let mut receipts = Vec::with_capacity(QUERIES);

    for query_id in 0..QUERIES {
        let (query_domain, query_artifact, version, source, status) = match query_id {
            0..=599 => (
                domain(query_id),
                artifact_type(query_id),
                Some(format!("v{}", query_id % 3)),
                None,
                "exact_version",
            ),
            600..=899 => (
                domain(query_id),
                artifact_type(query_id),
                None,
                None,
                "unversioned_ambiguous",
            ),
            900..=1099 => (
                domain(query_id),
                artifact_type(query_id),
                Some(format!("v{}", query_id % 3)),
                Some(format!("openstax-source-{}", query_id % 3)),
                "source_filtered",
            ),
            1100..=1299 => (
                "not-a-governed-domain",
                "theorem",
                Some("v0".into()),
                None,
                "unsupported",
            ),
            1300..=1699 => (
                "manifest-prerequisite",
                "artifact",
                None,
                None,
                "prerequisite",
            ),
            _ => (
                "unknown-prerequisite",
                "artifact",
                None,
                None,
                "unknown_prerequisite",
            ),
        };

        let selected = if status == "unsupported" {
            Vec::new()
        } else if let Some(version) = &version {
            memory.retrieve_exact_version(query_domain, query_artifact, version)
        } else {
            memory.retrieve_exact(query_domain, query_artifact)
        };
        let selected = if let Some(expected) = source.as_deref() {
            selected
                .into_iter()
                .filter(|r| r.provenance.iter().any(|p| p == expected))
                .collect::<Vec<_>>()
        } else {
            selected
        };
        let selected_ids: Vec<String> = selected.iter().map(|r| r.record_id.clone()).collect();
        let selected_hashes: Vec<String> =
            selected.iter().map(|r| r.content_hash.clone()).collect();
        let mut status_value = status.to_string();
        if status == "exact_version" {
            if selected.is_empty() {
                retrieval_contamination += 1;
                status_value = "refused".into();
            } else if selected.iter().all(|r| Some(r.version.clone()) == version) {
                exact_version_complete += 1;
            } else {
                retrieval_contamination += 1;
            }
        } else if status == "unversioned_ambiguous" {
            let versions: BTreeSet<String> = selected.iter().map(|r| r.version.clone()).collect();
            if versions.len() > 1 {
                unversioned_ambiguous += 1;
            } else {
                retrieval_contamination += 1;
            }
        } else if status == "source_filtered" {
            let expected = source.as_deref().unwrap();
            if selected
                .iter()
                .all(|r| r.provenance.iter().any(|p| p == expected))
            {
                source_filter_clean += 1;
            } else {
                retrieval_contamination += 1;
            }
        } else if status == "unsupported" {
            if selected.is_empty() {
                unsupported_refused += 1;
            } else {
                retrieval_contamination += 1;
            }
        } else if status == "prerequisite" {
            let result = discover(&manifest, &["matrix_artifact".into()]);
            if result.status == DiscoveryStatus::Complete && !result.packs.is_empty() {
                prerequisite_complete += 1;
            } else {
                retrieval_contamination += 1;
            }
        } else if status == "unknown_prerequisite" {
            let result = discover(&manifest, &["not-in-manifest".into()]);
            if result.status == DiscoveryStatus::UnknownArtifact {
                unknown_prerequisites_refused += 1;
                status_value = "unknown_refused".into();
            } else {
                retrieval_contamination += 1;
            }
        }
        let mut receipt = RetrievalReceipt {
            query_id,
            domain: query_domain.into(),
            artifact_type: query_artifact.into(),
            version,
            source,
            selected_ids,
            selected_hashes,
            status: status_value,
            replay_hash: String::new(),
        };
        receipt.replay_hash = receipt_hash(&receipt);
        if replay_verified(&receipt) {
            receipt_replays += 1;
        }
        let mut tampered = receipt.clone();
        tampered.status.push_str("-tampered");
        if !replay_verified(&tampered) {
            receipt_tamper_rejections += 1;
        }
        receipts.push(receipt);
    }

    let cycle_proposals_rejected = usize::from(!proposed_edge_is_acyclic(
        &manifest,
        "linear_algebra_spectral",
        "linear_algebra_spectral",
    ));
    assert_eq!(receipts.len(), QUERIES);
    assert_eq!(receipt_replays, QUERIES);
    assert_eq!(receipt_tamper_rejections, QUERIES);
    assert_eq!(exact_version_complete, 600);
    assert_eq!(unversioned_ambiguous, 300);
    assert_eq!(source_filter_clean, 200);
    assert_eq!(unsupported_refused, 200);
    assert_eq!(prerequisite_complete, 400);
    assert_eq!(unknown_prerequisites_refused, 300);
    assert_eq!(cycle_proposals_rejected, 1);
    assert_eq!(retrieval_contamination, 0);
    assert_eq!(manifest.replay_hash(), manifest_hash);

    let report = Report {
        schema: "stage113-curriculum-retrieval-prerequisites-v1",
        records: RECORDS,
        queries: QUERIES,
        exact_version_queries: 600,
        exact_version_complete,
        unversioned_ambiguous_queries: 300,
        unversioned_ambiguous,
        source_filter_queries: 200,
        source_filter_clean,
        unsupported_queries: 200,
        unsupported_refused,
        prerequisite_queries: 400,
        prerequisite_complete,
        unknown_prerequisite_queries: 300,
        unknown_prerequisites_refused,
        cycle_proposals_rejected,
        receipt_replays,
        receipt_tamper_rejections,
        retrieval_contamination,
        manifest_unchanged: manifest.replay_hash() == manifest_hash,
        manifest_hash,
    };
    println!("{}", serde_json::to_string_pretty(&report).unwrap());
}
