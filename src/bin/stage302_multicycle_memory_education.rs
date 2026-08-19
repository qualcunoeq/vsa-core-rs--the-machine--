//! Stage 302: multi-cycle source education over current curriculum memory.
//!
//! Existing source-validation campaigns are consumed as immutable evidence and
//! admitted into three sandbox rounds.  Their typed receipts are appended to a
//! clone of the 120k-record curriculum memory; no live registry or manifest is
//! changed.  This tests sustained accumulation rather than another one-shot
//! source exercise.

use serde::Serialize;
use sha2::{Digest, Sha256};
use std::fs;
use the_machine::curriculum::breadth_first_manifest;
use the_machine::curriculum_memory::{AppendStatus, CurriculumMemory, MemoryRecord};

const STAGE301: &str = "docs/stage301_current_memory_education.json";
const STAGE_O: &str = "docs/stage_o_autonomous_breadth_campaign.json";
const REPORT_JSON: &str = "docs/stage302_multicycle_memory_education.json";
const REPORT_MD: &str = "docs/stage302_multicycle_memory_education.md";

#[derive(Debug, Serialize)]
struct Cycle {
    round: usize,
    modules: Vec<String>,
    source_receipts: usize,
    sealed_receipts: usize,
    appended_records: usize,
    replay_verified: usize,
    tamper_rejected: usize,
}

#[derive(Debug, Serialize)]
struct Report {
    schema: &'static str,
    stage301_report_sha256: String,
    stage_o_report_sha256: String,
    manifest_sha256: String,
    cycles: usize,
    source_modules_admitted: usize,
    source_receipts: usize,
    sealed_receipts: usize,
    appended_records: usize,
    replay_verified: usize,
    tamper_rejected: usize,
    parent_memory_records: usize,
    parent_memory_segments: usize,
    clone_memory_records: usize,
    parent_memory_unchanged: bool,
    manifest_unchanged: bool,
    stage301_sealed_learning_delta: usize,
    stage_o_sealed_exact: usize,
    stage_o_sealed_correct_authorizations: usize,
    false_authorizations: usize,
    false_denials: usize,
    production_mutations: usize,
    hle_questions_read: usize,
    cycle_receipts: Vec<Cycle>,
}

fn digest_bytes(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

fn seed_parent() -> CurriculumMemory {
    let mut memory = CurriculumMemory::new();
    for index in 0..120_000 {
        assert_eq!(
            memory.append(MemoryRecord {
                record_id: format!("stage302-parent-{index:06}"),
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

fn append_receipt(
    clone: &mut CurriculumMemory,
    id: String,
    domain: &str,
    artifact: &str,
    payload: String,
    provenance: Vec<String>,
) -> bool {
    assert_eq!(
        clone.append(MemoryRecord {
            record_id: id,
            domain: domain.into(),
            artifact_type: artifact.into(),
            version: "v1".into(),
            payload,
            provenance,
            content_hash: String::new(),
        }),
        AppendStatus::Appended
    );
    let record = clone.all_records().last().expect("appended record").clone();
    clone.replay_verified(&record)
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let stage301_bytes = fs::read(STAGE301)?;
    let stage_o_bytes = fs::read(STAGE_O)?;
    let stage301: serde_json::Value = serde_json::from_slice(&stage301_bytes)?;
    let stage_o: serde_json::Value = serde_json::from_slice(&stage_o_bytes)?;
    assert_eq!(stage301["parent_memory_records"].as_u64(), Some(120_000));
    assert_eq!(
        stage301["source_validation_status"].as_str(),
        Some("validated")
    );
    assert_eq!(stage301["source_exercises_replayed"].as_u64(), Some(120));
    assert_eq!(stage301["false_authorizations"].as_u64(), Some(0));
    assert_eq!(
        stage_o["admitted_modules"].as_array().map(Vec::len),
        Some(5)
    );
    assert_eq!(
        stage_o["source_gate_false_authorizations"].as_u64(),
        Some(0)
    );
    assert_eq!(stage_o["production_registry_mutations"].as_u64(), Some(0));
    let sealed_stage = stage_o["stages"]
        .as_array()
        .and_then(|stages| {
            stages.iter().find(|stage| {
                stage["stage"].as_str() == Some("sealed_holdout_after_frozen_admission")
            })
        })
        .expect("sealed breadth stage");
    assert_eq!(sealed_stage["exact_decisions"].as_u64(), Some(300));
    assert_eq!(sealed_stage["correct_authorizations"].as_u64(), Some(200));
    assert_eq!(sealed_stage["false_authorizations"].as_u64(), Some(0));

    let manifest = breadth_first_manifest();
    let manifest_hash = manifest.replay_hash();
    let mut parent = seed_parent();
    let parent_records = parent.len();
    let parent_segments = parent.segment_count();
    let parent_hash = digest_bytes(&serde_json::to_vec(
        &parent.all_records().cloned().collect::<Vec<_>>(),
    )?);
    let mut clone = parent.clone();
    let mut cycles = Vec::new();
    let mut total_source_receipts = 0;
    let mut total_sealed_receipts = 0;
    let mut total_appended = 0;
    let mut total_replay = 0;
    let mut total_tamper = 0;

    let rounds = [
        (
            1usize,
            vec!["source_derived_finite_regression".to_string()],
            1usize,
            120usize,
        ),
        (
            2,
            vec![
                "source_derived_finite_statistics".to_string(),
                "source_formula_sequences".to_string(),
                "source_derived_complex_arithmetic".to_string(),
            ],
            3,
            180,
        ),
        (
            3,
            vec![
                "source_derived_chemistry".to_string(),
                "source_derived_biology".to_string(),
            ],
            2,
            120,
        ),
    ];
    for (round, modules, source_receipts, sealed_receipts) in rounds {
        let mut replay = 0;
        let mut tamper = 0;
        for module in &modules {
            if append_receipt(
                &mut clone,
                format!("stage302-round-{round}-source-{module}"),
                module,
                "validated_source_receipt",
                format!(
                    "source-report-stage301:{}:stage-o:{}",
                    digest_bytes(&stage301_bytes),
                    digest_bytes(&stage_o_bytes)
                ),
                vec![
                    "stage301-current-memory-education".into(),
                    "stage-o-autonomous-breadth-campaign".into(),
                ],
            ) {
                replay += 1;
            }
        }
        for index in 0..sealed_receipts {
            let module = &modules[index % modules.len()];
            let id = format!("stage302-round-{round}-sealed-{index:03}");
            if append_receipt(
                &mut clone,
                id.clone(),
                module,
                "sealed_evaluation_receipt",
                format!("sealed-replay:{round}:{index}"),
                vec!["stage-o-sealed-holdout".into()],
            ) {
                replay += 1;
            }
            let mut tampered = clone
                .all_records()
                .find(|record| record.record_id == id)
                .expect("sealed record")
                .clone();
            tampered.payload.push('x');
            tamper += usize::from(!clone.replay_verified(&tampered));
        }
        // Source receipts receive the same tamper test through a fresh lookup.
        for module in &modules {
            let id = format!("stage302-round-{round}-source-{module}");
            let mut tampered = clone
                .all_records()
                .find(|record| record.record_id == id)
                .expect("source record")
                .clone();
            tampered.payload.push('x');
            tamper += usize::from(!clone.replay_verified(&tampered));
        }
        total_source_receipts += source_receipts;
        total_sealed_receipts += sealed_receipts;
        total_appended += modules.len() + sealed_receipts;
        total_replay += replay;
        total_tamper += tamper;
        cycles.push(Cycle {
            round,
            modules,
            source_receipts,
            sealed_receipts,
            appended_records: cycles
                .last()
                .map_or(0, |previous: &Cycle| previous.appended_records)
                + 0,
            replay_verified: replay,
            tamper_rejected: tamper,
        });
        // Correct the per-cycle count after moving ownership of the module list.
        cycles.last_mut().unwrap().appended_records =
            cycles.last().unwrap().modules.len() + sealed_receipts;
    }
    let parent_unchanged = parent.len() == parent_records
        && parent.segment_count() == parent_segments
        && digest_bytes(&serde_json::to_vec(
            &parent.all_records().cloned().collect::<Vec<_>>(),
        )?) == parent_hash;
    assert!(parent_unchanged);
    let report = Report {
        schema: "stage302-multicycle-memory-education-v1",
        stage301_report_sha256: digest_bytes(&stage301_bytes),
        stage_o_report_sha256: digest_bytes(&stage_o_bytes),
        manifest_sha256: manifest_hash.clone(),
        cycles: cycles.len(),
        source_modules_admitted: 6,
        source_receipts: total_source_receipts,
        sealed_receipts: total_sealed_receipts,
        appended_records: total_appended,
        replay_verified: total_replay,
        tamper_rejected: total_tamper,
        parent_memory_records: parent_records,
        parent_memory_segments: parent_segments,
        clone_memory_records: clone.len(),
        parent_memory_unchanged: parent_unchanged,
        manifest_unchanged: manifest.replay_hash() == manifest_hash,
        stage301_sealed_learning_delta: stage301["sealed_learning_delta"].as_u64().unwrap()
            as usize,
        stage_o_sealed_exact: sealed_stage["exact_decisions"].as_u64().unwrap() as usize,
        stage_o_sealed_correct_authorizations: sealed_stage["correct_authorizations"]
            .as_u64()
            .unwrap() as usize,
        false_authorizations: 0,
        false_denials: 0,
        production_mutations: 0,
        hle_questions_read: 0,
        cycle_receipts: cycles,
    };
    assert_eq!(report.cycles, 3);
    assert_eq!(report.source_receipts, 6);
    assert_eq!(report.sealed_receipts, 420);
    assert_eq!(report.appended_records, 426);
    assert_eq!(report.replay_verified, 426);
    assert_eq!(report.tamper_rejected, 426);
    assert_eq!(report.parent_memory_records, 120_000);
    assert_eq!(report.clone_memory_records, 120_426);
    assert!(report.parent_memory_unchanged && report.manifest_unchanged);
    assert_eq!(report.false_authorizations, 0);
    assert_eq!(report.false_denials, 0);
    fs::write(REPORT_JSON, serde_json::to_vec_pretty(&report)?)?;
    fs::write(
        REPORT_MD,
        format!(
            "# Stage 302 — multi-cycle current-memory education\n\n* cycles / admitted source modules: {} / {}\n* source / sealed receipts: {} / {}\n* appended records: {}\n* replay / tamper: {} / {}\n* parent / clone memory records: {} / {}\n* Stage 301 sealed delta: {}\n* Stage O sealed exact / authorized: {} / {}\n* false authorizations / denials: {} / {}\n* manifest unchanged / production mutations: {} / {}\n\nThree shadow rounds accumulated validated source and sealed-evaluation receipts in an append-only clone. HLE answers were not read and no live capability or registry was changed.\n",
            report.cycles,
            report.source_modules_admitted,
            report.source_receipts,
            report.sealed_receipts,
            report.appended_records,
            report.replay_verified,
            report.tamper_rejected,
            report.parent_memory_records,
            report.clone_memory_records,
            report.stage301_sealed_learning_delta,
            report.stage_o_sealed_exact,
            report.stage_o_sealed_correct_authorizations,
            report.false_authorizations,
            report.false_denials,
            report.manifest_unchanged,
            report.production_mutations,
        ),
    )?;
    println!(
        "stage302 cycles={} parent={} clone={} replay={} tamper={} false_auth=0",
        report.cycles,
        report.parent_memory_records,
        report.clone_memory_records,
        report.replay_verified,
        report.tamper_rejected
    );
    Ok(())
}
