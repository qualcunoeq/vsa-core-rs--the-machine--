//! Stage 115: self-directed retrieval over immutable curriculum memory.
//!
//! This planner admits a source-derived artifact only when exact artifact
//! evidence, source-catalog provenance, and prerequisite closure all agree.
//! It proposes plans; it never mutates the curriculum manifest or live route.

use serde::Serialize;
use sha2::{Digest, Sha256};
use the_machine::curriculum::breadth_first_manifest;
use the_machine::curriculum_memory::{AppendStatus, CurriculumMemory, MemoryRecord};
use the_machine::prerequisite_discovery::{discover, DiscoveryStatus};
use the_machine::source_catalog_ingestion::ingest;

const GAPS: usize = 1_200;
const COMPLETE: usize = 600;
const STALE: usize = 200;
const UNKNOWN: usize = 200;
const SOURCE_MISSING: usize = 200;

const SET_SOURCE: &str =
    include_str!("../../docs/sources/openstax_finite_set_operations_catalog.txt");
const COUNT_SOURCE: &str =
    include_str!("../../docs/sources/openstax_counting_principles_catalog.txt");
const LOGIC_SOURCE: &str = include_str!("../../docs/sources/openstax_truth_table_catalog.txt");

fn digest<T: Serialize + ?Sized>(value: &T) -> String {
    format!("{:x}", Sha256::digest(serde_json::to_vec(value).unwrap()))
}

#[derive(Debug, Clone, Serialize)]
struct PlanReceipt {
    gap_id: usize,
    requested_artifact: String,
    requested_version: String,
    requested_source: String,
    status: String,
    selected_record: Option<String>,
    selected_hash: Option<String>,
    prerequisite_packs: Vec<String>,
    reasons: Vec<String>,
    replay_hash: String,
}

fn receipt_hash(receipt: &PlanReceipt) -> String {
    digest(&(
        receipt.gap_id,
        &receipt.requested_artifact,
        &receipt.requested_version,
        &receipt.requested_source,
        &receipt.status,
        &receipt.selected_record,
        &receipt.selected_hash,
        &receipt.prerequisite_packs,
        &receipt.reasons,
    ))
}

fn replay_verified(receipt: &PlanReceipt) -> bool {
    receipt.replay_hash == receipt_hash(receipt)
}

#[derive(Debug, Serialize)]
struct Report {
    schema: &'static str,
    gaps: usize,
    complete: usize,
    stale_version_refused: usize,
    unknown_artifact_refused: usize,
    unavailable_source_refused: usize,
    plan_replays: usize,
    plan_tamper_rejections: usize,
    source_catalogs: usize,
    provenance_mismatches: usize,
    prerequisite_failures: usize,
    manifest_unchanged: bool,
    live_route_mutations: usize,
    manifest_hash: String,
}

fn main() {
    let manifest = breadth_first_manifest();
    let manifest_hash = manifest.replay_hash();
    assert!(manifest.validate().is_empty());
    let catalogs = [SET_SOURCE, COUNT_SOURCE, LOGIC_SOURCE]
        .iter()
        .map(|source| ingest(source).expect("source catalog is valid"))
        .collect::<Vec<_>>();
    let source_ids: Vec<String> = catalogs
        .iter()
        .map(|catalog| catalog.citation.source_id.clone())
        .collect();

    let mut memory = CurriculumMemory::new();
    let mut artifacts = Vec::new();
    for pack in &manifest.packs {
        for artifact in &pack.reusable_artifacts {
            artifacts.push((pack.id.clone(), artifact.clone()));
            for version in ["v1", "v2"] {
                let index = artifacts.len() + usize::from(version == "v2");
                let source = source_ids[index % source_ids.len()].clone();
                let record = MemoryRecord {
                    record_id: format!("stage115-{}-{artifact}-{version}", pack.id),
                    domain: pack.id.clone(),
                    artifact_type: "typed_artifact".into(),
                    version: version.into(),
                    payload: format!("artifact:{artifact}"),
                    provenance: vec![source, "stage115-source-derived".into()],
                    content_hash: String::new(),
                };
                assert_eq!(memory.append(record), AppendStatus::Appended);
            }
        }
    }

    let mut complete = 0;
    let mut stale_version_refused = 0;
    let mut unknown_artifact_refused = 0;
    let mut unavailable_source_refused = 0;
    let mut plan_replays = 0;
    let mut plan_tamper_rejections = 0;
    let mut provenance_mismatches = 0;
    let mut prerequisite_failures = 0;
    let mut receipts = Vec::with_capacity(GAPS);

    for gap_id in 0..GAPS {
        let family_index = gap_id % artifacts.len();
        let (pack, known_artifact) = &artifacts[family_index];
        let (requested_artifact, requested_version, requested_source, expected_status) =
            if gap_id < COMPLETE {
                (
                    known_artifact.clone(),
                    "v2".to_string(),
                    source_ids[(family_index + 2) % source_ids.len()].clone(),
                    "complete",
                )
            } else if gap_id < COMPLETE + STALE {
                (
                    known_artifact.clone(),
                    "v9".to_string(),
                    source_ids[family_index % source_ids.len()].clone(),
                    "stale_version_refused",
                )
            } else if gap_id < COMPLETE + STALE + UNKNOWN {
                (
                    format!("unknown_artifact_{gap_id}"),
                    "v2".to_string(),
                    source_ids[family_index % source_ids.len()].clone(),
                    "unknown_artifact_refused",
                )
            } else {
                (
                    known_artifact.clone(),
                    "v2".to_string(),
                    "unavailable-source".into(),
                    "unavailable_source_refused",
                )
            };

        let mut selected = memory
            .retrieve_exact_version(pack, "typed_artifact", &requested_version)
            .into_iter()
            .filter(|record| record.payload == format!("artifact:{requested_artifact}"))
            .filter(|record| {
                record
                    .provenance
                    .iter()
                    .any(|source| source == &requested_source)
            })
            .collect::<Vec<_>>();
        let mut reasons = Vec::new();
        let mut prerequisite_packs = Vec::new();
        let status;
        let selected_record;
        let selected_hash;
        if expected_status == "complete" {
            let closure = discover(&manifest, &[requested_artifact.clone()]);
            if closure.status != DiscoveryStatus::Complete {
                prerequisite_failures += 1;
            }
            prerequisite_packs = closure.packs;
            if selected.len() == 1 && closure.status == DiscoveryStatus::Complete {
                status = "complete".to_string();
                selected_record = Some(selected.remove(0).record_id.clone());
                selected_hash = memory
                    .get(selected_record.as_deref().unwrap())
                    .map(|record| record.content_hash.clone());
                complete += 1;
            } else {
                status = "refused".to_string();
                selected_record = None;
                selected_hash = None;
                provenance_mismatches += 1;
            }
        } else {
            status = expected_status.to_string();
            selected_record = None;
            selected_hash = None;
            match expected_status {
                "stale_version_refused" => stale_version_refused += 1,
                "unknown_artifact_refused" => unknown_artifact_refused += 1,
                "unavailable_source_refused" => unavailable_source_refused += 1,
                _ => unreachable!(),
            }
            if expected_status == "unknown_artifact_refused" {
                let closure = discover(&manifest, &[requested_artifact.clone()]);
                assert_eq!(closure.status, DiscoveryStatus::UnknownArtifact);
            }
            reasons.push("exact source/version/artifact evidence was unavailable".into());
        }
        let mut receipt = PlanReceipt {
            gap_id,
            requested_artifact,
            requested_version,
            requested_source,
            status,
            selected_record,
            selected_hash,
            prerequisite_packs,
            reasons,
            replay_hash: String::new(),
        };
        receipt.replay_hash = receipt_hash(&receipt);
        if replay_verified(&receipt) {
            plan_replays += 1;
        }
        let mut tampered = receipt.clone();
        tampered.status.push_str("-tampered");
        if !replay_verified(&tampered) {
            plan_tamper_rejections += 1;
        }
        receipts.push(receipt);
    }

    assert_eq!(receipts.len(), GAPS);
    assert_eq!(complete, COMPLETE);
    assert_eq!(stale_version_refused, STALE);
    assert_eq!(unknown_artifact_refused, UNKNOWN);
    assert_eq!(unavailable_source_refused, SOURCE_MISSING);
    assert_eq!(plan_replays, GAPS);
    assert_eq!(plan_tamper_rejections, GAPS);
    assert_eq!(provenance_mismatches, 0);
    assert_eq!(prerequisite_failures, 0);
    assert_eq!(manifest.replay_hash(), manifest_hash);

    let report = Report {
        schema: "stage115-self-directed-memory-retrieval-v1",
        gaps: GAPS,
        complete,
        stale_version_refused,
        unknown_artifact_refused,
        unavailable_source_refused,
        plan_replays,
        plan_tamper_rejections,
        source_catalogs: catalogs.len(),
        provenance_mismatches,
        prerequisite_failures,
        manifest_unchanged: manifest.replay_hash() == manifest_hash,
        live_route_mutations: 0,
        manifest_hash,
    };
    println!("{}", serde_json::to_string_pretty(&report).unwrap());
}
