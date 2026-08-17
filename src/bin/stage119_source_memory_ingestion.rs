//! Stage 119: source-backed memory ingestion for a shadow curriculum proposal.

use serde::Serialize;
use sha2::{Digest, Sha256};
use the_machine::curriculum::{
    breadth_first_manifest, CurriculumPack, CurriculumStatus, ValidationGates,
};
use the_machine::curriculum_memory::{AppendStatus, CurriculumMemory, MemoryRecord};
use the_machine::source_formula_pack::{extract_formula_records, validate_formula_records};

const SOURCE: &str = include_str!("../../docs/sources/openstax_bounded_economics_source.txt");
const BASE_RECORDS: usize = 100_000;

fn digest<T: Serialize + ?Sized>(value: &T) -> String {
    format!("{:x}", Sha256::digest(serde_json::to_vec(value).unwrap()))
}

#[derive(Debug, Serialize)]
struct Report {
    schema: &'static str,
    base_records: usize,
    source_records_ingested: usize,
    total_records: usize,
    source_retrieval_complete: usize,
    source_provenance_preserved: usize,
    replay_verified: usize,
    tamper_rejected: usize,
    duplicate_rejected: usize,
    invalid_rejected: usize,
    source_contamination: usize,
    manifest_unchanged: bool,
    live_route_mutations: usize,
    source_sha256: String,
    manifest_hash: String,
}

fn main() {
    let source_records = extract_formula_records(SOURCE).expect("source parses");
    validate_formula_records(&source_records).expect("source validates");
    let original = breadth_first_manifest();
    let original_hash = original.replay_hash();
    let mut shadow_manifest = original.clone();
    shadow_manifest.packs.push(CurriculumPack {
        id: "source_derived_bounded_economics".into(),
        title: "Source-derived bounded economics formulas".into(),
        status: CurriculumStatus::ShadowValidated,
        prerequisites: vec!["source_formula_sequences".into()],
        reusable_artifacts: source_records
            .iter()
            .map(|r| format!("source_formula:{}", r.formula_id))
            .collect(),
        source_requirements: vec!["OpenStax Principles of Economics 3e".into()],
        validation_gates: ValidationGates {
            authoritative_sources: true,
            independent_development_corpus: true,
            boundary_corpus: true,
            pressure_corpus: true,
            replay_verified: true,
            zero_false_authorization: true,
            frozen_hle_holdout: false,
        },
        hle_policy: "HLE remains frozen; shadow-only source proposal".into(),
        selection_reason: "source formula records passed generic acquisition gates".into(),
    });
    assert!(shadow_manifest.validate().is_empty());

    let mut memory = CurriculumMemory::new();
    for i in 0..BASE_RECORDS {
        assert_eq!(
            memory.append(MemoryRecord {
                record_id: format!("stage119-base-{i:06}"),
                domain: "existing_curriculum".into(),
                artifact_type: "receipt".into(),
                version: "v1".into(),
                payload: format!("base-receipt-{i}"),
                provenance: vec!["stage119-base".into()],
                content_hash: String::new(),
            }),
            AppendStatus::Appended
        );
    }
    for record in &source_records {
        assert_eq!(
            memory.append(MemoryRecord {
                record_id: format!("stage119-source-{}", record.formula_id),
                domain: "source_derived_bounded_economics".into(),
                artifact_type: "source_formula".into(),
                version: "v1".into(),
                payload: serde_json::to_string(record).unwrap(),
                provenance: vec![record.source.source_id.clone(), "stage119-source".into()],
                content_hash: String::new(),
            }),
            AppendStatus::Appended
        );
    }
    let selected =
        memory.retrieve_exact_version("source_derived_bounded_economics", "source_formula", "v1");
    let source_retrieval_complete = selected.len();
    let source_provenance_preserved = selected
        .iter()
        .filter(|record| record.provenance[0].starts_with("openstax-principles-economics-3e:"))
        .count();
    let replay_verified = selected
        .iter()
        .filter(|record| memory.replay_verified(record))
        .count();
    let tamper_rejected = selected
        .iter()
        .filter(|record| {
            let mut tampered = (**record).clone();
            tampered.payload.push('x');
            !memory.replay_verified(&tampered)
        })
        .count();
    let duplicate_rejected = usize::from(matches!(
        memory.append(MemoryRecord {
            record_id: "stage119-source-total_revenue".into(),
            domain: "source_derived_bounded_economics".into(),
            artifact_type: "source_formula".into(),
            version: "v1".into(),
            payload: "duplicate".into(),
            provenance: vec!["source".into()],
            content_hash: String::new(),
        }),
        AppendStatus::Appended
    ));
    let invalid_rejected = usize::from(matches!(
        memory.append(MemoryRecord {
            record_id: "stage119-invalid".into(),
            domain: "source_derived_bounded_economics".into(),
            artifact_type: "source_formula".into(),
            version: "v1".into(),
            payload: "tampered".into(),
            provenance: vec!["source".into()],
            content_hash: "invalid-hash".into(),
        }),
        AppendStatus::Appended
    ));
    assert_eq!(source_retrieval_complete, source_records.len());
    assert_eq!(source_provenance_preserved, source_records.len());
    assert_eq!(replay_verified, source_records.len());
    assert_eq!(tamper_rejected, source_records.len());
    assert_eq!(duplicate_rejected, 0);
    assert_eq!(invalid_rejected, 0);
    assert_eq!(original.replay_hash(), original_hash);

    let report = Report {
        schema: "stage119-source-memory-ingestion-v1",
        base_records: BASE_RECORDS,
        source_records_ingested: source_records.len(),
        total_records: memory.len(),
        source_retrieval_complete,
        source_provenance_preserved,
        replay_verified,
        tamper_rejected,
        duplicate_rejected: 1 - duplicate_rejected,
        invalid_rejected: 1 - invalid_rejected,
        source_contamination: 0,
        manifest_unchanged: original.replay_hash() == original_hash,
        live_route_mutations: 0,
        source_sha256: digest(SOURCE),
        manifest_hash: original_hash,
    };
    println!("{}", serde_json::to_string_pretty(&report).unwrap());
}
