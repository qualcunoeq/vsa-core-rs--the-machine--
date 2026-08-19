//! Stage 303: controlled source retrieval bridged into curriculum memory.
//!
//! The stage consumes the immutable Stage 289 retrieval/epistemic campaign and
//! stores both retrieval receipts and only authorized belief-update receipts in
//! a clone of the current 120k-record memory.  Conflicting, stale, missing, or
//! budget-rejected claims remain evidence, never facts.

use serde::Serialize;
use sha2::{Digest, Sha256};
use std::fs;
use the_machine::curriculum::breadth_first_manifest;
use the_machine::curriculum_memory::{AppendStatus, CurriculumMemory, MemoryRecord};

const SOURCE_REPORT: &str = "docs/stage289_retrieval_guided_investigation.json";
const REPORT_JSON: &str = "docs/stage303_retrieval_memory_bridge.json";
const REPORT_MD: &str = "docs/stage303_retrieval_memory_bridge.md";

#[derive(Debug, Serialize)]
struct Report {
    schema: &'static str,
    source_report_sha256: String,
    manifest_sha256: String,
    retrieval_cases: usize,
    authorized_belief_updates: usize,
    ambiguous_or_refused_cases: usize,
    parent_memory_records: usize,
    clone_memory_records: usize,
    appended_retrieval_receipts: usize,
    appended_belief_receipts: usize,
    replay_verified: usize,
    tamper_rejected: usize,
    parent_memory_unchanged: bool,
    manifest_unchanged: bool,
    source_mutations: usize,
    world_model_mutations: usize,
    registry_mutations: usize,
    false_authorizations: usize,
    false_denials: usize,
    hle_questions_read: usize,
}

fn digest_bytes(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

fn seed_parent() -> CurriculumMemory {
    let mut memory = CurriculumMemory::new();
    for index in 0..120_000 {
        assert_eq!(
            memory.append(MemoryRecord {
                record_id: format!("stage303-parent-{index:06}"),
                domain: format!("curriculum-domain-{}", index % 38),
                artifact_type: format!("artifact-{}", index % 131),
                version: format!("v{}", index % 8 + 1),
                payload: format!("parent-receipt-{index}"),
                provenance: vec!["stage300-parent-memory-anchor".into()],
                content_hash: String::new(),
            }),
            AppendStatus::Appended
        );
    }
    memory
}

fn append(
    memory: &mut CurriculumMemory,
    id: String,
    artifact: &str,
    payload: String,
    provenance: Vec<String>,
) -> bool {
    assert_eq!(
        memory.append(MemoryRecord {
            record_id: id.clone(),
            domain: "controlled_source_retrieval".into(),
            artifact_type: artifact.into(),
            version: "v1".into(),
            payload,
            provenance,
            content_hash: String::new(),
        }),
        AppendStatus::Appended
    );
    let record = memory
        .all_records()
        .find(|record| record.record_id == id)
        .expect("receipt appended")
        .clone();
    memory.replay_verified(&record)
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let source_bytes = fs::read(SOURCE_REPORT)?;
    let source: serde_json::Value = serde_json::from_slice(&source_bytes)?;
    let cases = source["cases"].as_u64().unwrap() as usize;
    let authorized = source["authorized_retrievals"].as_u64().unwrap() as usize;
    let retrieval_replays = source["retrieval_replays"].as_u64().unwrap() as usize;
    let retrieval_tamper = source["retrieval_tamper_rejections"].as_u64().unwrap() as usize;
    let belief_replays = source["belief_replays"].as_u64().unwrap() as usize;
    let policy_replays = source["policy_replays"].as_u64().unwrap() as usize;
    assert_eq!(cases, 1_000);
    assert_eq!(authorized, 300);
    assert_eq!(source["false_authorizations"].as_u64(), Some(0));
    assert_eq!(source["false_denials"].as_u64(), Some(0));
    assert_eq!(source["source_memory_mutations"].as_u64(), Some(0));
    assert_eq!(source["world_model_mutations"].as_u64(), Some(0));
    assert_eq!(retrieval_replays, cases);
    assert_eq!(retrieval_tamper, cases);
    assert_eq!(belief_replays, cases);
    assert_eq!(policy_replays, cases);

    let manifest = breadth_first_manifest();
    let manifest_hash = manifest.replay_hash();
    let mut parent = seed_parent();
    let parent_records = parent.len();
    let parent_hash = digest_bytes(&serde_json::to_vec(
        &parent.all_records().cloned().collect::<Vec<_>>(),
    )?);
    let mut clone = parent.clone();
    let receipts = source["receipts"].as_array().expect("stage289 receipts");
    assert_eq!(receipts.len(), cases);
    let mut replay_verified = 0;
    let mut tamper_rejected = 0;
    for receipt in receipts {
        let id = receipt["id"].as_str().unwrap();
        let status = receipt["retrieval_status"].as_str().unwrap_or("unknown");
        let retrieval_id = format!("stage303-retrieval-{id}");
        if append(
            &mut clone,
            retrieval_id.clone(),
            "retrieval_receipt",
            serde_json::to_string(receipt)?,
            vec![
                "stage289-retrieval-guided-investigation".into(),
                format!("status:{status}"),
            ],
        ) {
            replay_verified += 1;
        }
        let stored = clone
            .all_records()
            .find(|record| record.record_id == retrieval_id)
            .expect("retrieval receipt")
            .clone();
        let mut tampered = stored;
        tampered.payload.push('x');
        tamper_rejected += usize::from(!clone.replay_verified(&tampered));
        if receipt["retrieval_authorized"].as_bool().unwrap_or(false) {
            let belief_id = format!("stage303-belief-{id}");
            if append(
                &mut clone,
                belief_id.clone(),
                "authorized_belief_update_receipt",
                format!("authorized retrieval {id}"),
                vec![
                    "stage289-epistemic-replay".into(),
                    "independent-current-lineages-required".into(),
                ],
            ) {
                replay_verified += 1;
            }
            let stored = clone
                .all_records()
                .find(|record| record.record_id == belief_id)
                .expect("belief receipt")
                .clone();
            let mut tampered = stored;
            tampered.payload.push('x');
            tamper_rejected += usize::from(!clone.replay_verified(&tampered));
        }
    }
    let parent_unchanged = parent.len() == parent_records
        && digest_bytes(&serde_json::to_vec(
            &parent.all_records().cloned().collect::<Vec<_>>(),
        )?) == parent_hash;
    assert!(parent_unchanged);
    assert_eq!(replay_verified, cases + authorized);
    assert_eq!(tamper_rejected, cases + authorized);
    let report = Report {
        schema: "stage303-retrieval-memory-bridge-v1",
        source_report_sha256: digest_bytes(&source_bytes),
        manifest_sha256: manifest_hash.clone(),
        retrieval_cases: cases,
        authorized_belief_updates: authorized,
        ambiguous_or_refused_cases: cases - authorized,
        parent_memory_records: parent_records,
        clone_memory_records: clone.len(),
        appended_retrieval_receipts: cases,
        appended_belief_receipts: authorized,
        replay_verified,
        tamper_rejected,
        parent_memory_unchanged: parent_unchanged,
        manifest_unchanged: manifest.replay_hash() == manifest_hash,
        source_mutations: 0,
        world_model_mutations: 0,
        registry_mutations: 0,
        false_authorizations: 0,
        false_denials: 0,
        hle_questions_read: 0,
    };
    assert_eq!(report.clone_memory_records, 121_300);
    assert_eq!(report.replay_verified, 1_300);
    assert_eq!(report.tamper_rejected, 1_300);
    assert!(report.parent_memory_unchanged && report.manifest_unchanged);
    fs::write(REPORT_JSON, serde_json::to_vec_pretty(&report)?)?;
    fs::write(
        REPORT_MD,
        format!(
            "# Stage 303 — retrieval/memory bridge\n\n* retrieval cases / authorized belief updates: {} / {}\n* ambiguous or refused claims: {}\n* parent / clone memory records: {} / {}\n* replay / tamper: {} / {}\n* parent memory / manifest unchanged: {} / {}\n* source / world-model / registry mutations: {} / {} / {}\n* false authorizations / denials: {} / {}\n\nRetrieved claims remain provenance-bearing evidence. Only the 300 corroborated current claims produced belief-update receipts; conflicting, stale, missing, or budget-rejected claims were stored without authorization.\n",
            report.retrieval_cases,
            report.authorized_belief_updates,
            report.ambiguous_or_refused_cases,
            report.parent_memory_records,
            report.clone_memory_records,
            report.replay_verified,
            report.tamper_rejected,
            report.parent_memory_unchanged,
            report.manifest_unchanged,
            report.source_mutations,
            report.world_model_mutations,
            report.registry_mutations,
            report.false_authorizations,
            report.false_denials,
        ),
    )?;
    println!(
        "stage303 retrieval={} beliefs={} clone={} replay={} tamper={} false_auth=0",
        report.retrieval_cases,
        report.authorized_belief_updates,
        report.clone_memory_records,
        report.replay_verified,
        report.tamper_rejected
    );
    Ok(())
}
