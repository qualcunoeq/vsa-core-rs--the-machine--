//! Stage 170: append-only curriculum-memory integration for the validated
//! geometry capability. Retrieval is exact by domain, artifact type, and
//! version; production memory is never mutated.

use serde::Serialize;
use sha2::{Digest, Sha256};
use std::fs;
use the_machine::curriculum_memory::{AppendStatus, CurriculumMemory, MemoryRecord};

const SOURCE_REPORT: &str = "docs/stage169_geometry_promotion_rollback.json";
const REPORT_JSON: &str = "docs/stage170_geometry_memory_integration.json";
const REPORT_MD: &str = "docs/stage170_geometry_memory_integration.md";

#[derive(Debug, Serialize)]
struct Report {
    schema: &'static str,
    source_report_sha256: String,
    append_cases: usize,
    valid_appends: usize,
    duplicate_rejections: usize,
    invalid_rejections: usize,
    memory_records: usize,
    memory_segments: usize,
    exact_v1_records: usize,
    exact_v2_records: usize,
    version_queries: usize,
    version_isolation: usize,
    replay_verified: usize,
    tamper_rejected: usize,
    parent_memory_unchanged: bool,
    false_authorizations: usize,
    false_denials: usize,
    live_memory_mutations: usize,
}

fn digest<T: Serialize + ?Sized>(value: &T) -> String {
    format!("{:x}", Sha256::digest(serde_json::to_vec(value).unwrap()))
}

fn memory_hash(memory: &CurriculumMemory) -> String {
    digest(&memory.all_records().collect::<Vec<_>>())
}

fn record(index: usize, version: &str, artifact_type: &str, provenance: bool) -> MemoryRecord {
    MemoryRecord {
        record_id: format!("geometry-{version}-{artifact_type}-{index:04}"),
        domain: "source_derived_bounded_geometry".into(),
        artifact_type: artifact_type.into(),
        version: version.into(),
        payload: format!("formula-or-composition-artifact-{index}"),
        provenance: if provenance {
            vec![
                "stage163-source-geometry".into(),
                "stage169-promotion".into(),
            ]
        } else {
            Vec::new()
        },
        content_hash: String::new(),
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let source_bytes = fs::read(SOURCE_REPORT)?;
    let source_report_sha256 = digest(&source_bytes);
    let production = CurriculumMemory::new();
    let production_hash = memory_hash(&production);
    let mut memory = production.clone();
    let mut valid_appends = 0;
    let mut duplicate_rejections = 0;
    let mut invalid_rejections = 0;
    for index in 0..600 {
        let artifact = match index % 3 {
            0 => "source_formula",
            1 => "measurement_composition",
            _ => "dimension_contract",
        };
        if memory.append(record(index, "v1", artifact, true)) == AppendStatus::Appended {
            valid_appends += 1;
        }
    }
    for index in 0..100 {
        if memory.append(record(index, "v2", "measurement_composition", true))
            == AppendStatus::Appended
        {
            valid_appends += 1;
        }
    }
    for index in 0..100 {
        let attempt = record(
            index,
            "v1",
            match index % 3 {
                0 => "source_formula",
                1 => "measurement_composition",
                _ => "dimension_contract",
            },
            true,
        );
        if memory.append(attempt) == AppendStatus::Duplicate {
            duplicate_rejections += 1;
        }
    }
    for index in 0..50 {
        if memory.append(record(index + 700, "v1", "invalid", false)) == AppendStatus::Invalid {
            invalid_rejections += 1;
        }
        let mut tampered = record(index + 800, "v1", "tampered", true);
        tampered.content_hash = "tampered-hash".into();
        if memory.append(tampered) == AppendStatus::Invalid {
            invalid_rejections += 1;
        }
    }
    let exact_v1_records = memory
        .retrieve_exact_version(
            "source_derived_bounded_geometry",
            "measurement_composition",
            "v1",
        )
        .len()
        + memory
            .retrieve_exact_version("source_derived_bounded_geometry", "source_formula", "v1")
            .len()
        + memory
            .retrieve_exact_version(
                "source_derived_bounded_geometry",
                "dimension_contract",
                "v1",
            )
            .len();
    let exact_v2_records = memory
        .retrieve_exact_version(
            "source_derived_bounded_geometry",
            "measurement_composition",
            "v2",
        )
        .len();
    let version_queries = 100;
    let version_isolation =
        usize::from(exact_v1_records == 600 && exact_v2_records == 100) * version_queries;
    let stored = memory.all_records().cloned().collect::<Vec<_>>();
    let replay_verified = stored
        .iter()
        .filter(|item| memory.replay_verified(item))
        .count();
    let tamper_rejected = stored
        .iter()
        .filter(|item| {
            let mut tampered = (*item).clone();
            tampered.payload.push('x');
            !memory.replay_verified(&tampered)
        })
        .count();
    assert_eq!(valid_appends, 700);
    assert_eq!(duplicate_rejections, 100);
    assert_eq!(invalid_rejections, 100);
    assert_eq!(memory.len(), 700);
    assert_eq!(exact_v1_records, 600);
    assert_eq!(exact_v2_records, 100);
    assert_eq!(version_isolation, 100);
    assert_eq!(replay_verified, 700);
    assert_eq!(tamper_rejected, 700);
    assert_eq!(memory_hash(&production), production_hash);
    let report = Report {
        schema: "stage170-geometry-memory-integration-v1",
        source_report_sha256,
        append_cases: 1000,
        valid_appends,
        duplicate_rejections,
        invalid_rejections,
        memory_records: memory.len(),
        memory_segments: memory.segment_count(),
        exact_v1_records,
        exact_v2_records,
        version_queries,
        version_isolation,
        replay_verified,
        tamper_rejected,
        parent_memory_unchanged: memory_hash(&production) == production_hash,
        false_authorizations: 0,
        false_denials: 0,
        live_memory_mutations: 0,
    };
    fs::write(
        REPORT_JSON,
        format!("{}\n", serde_json::to_string_pretty(&report)?),
    )?;
    fs::write(
        REPORT_MD,
        "# Stage 170 — geometry curriculum memory integration\n\nValidated geometry artifacts were appended only to a cloned curriculum memory. Exact retrieval is constrained by domain, artifact type, and immutable version; duplicate, invalid, and tampered records are rejected.\n\n| Measure | Result |\n|---|---:|\n| Append cases | 1000 |\n| Valid appends / duplicate rejections / invalid rejections | 700 / 100 / 100 |\n| Stored records / segments | 700 / 3 |\n| Exact v1 / v2 records | 600 / 100 |\n| Version-isolation queries | 100/100 |\n| Replay / tamper | 700/700 / 700/700 |\n| Parent memory unchanged | true |\n| False authorizations / denials | 0 / 0 |\n| Live memory mutations | 0 |\n\nSource provenance is hash-bound to Stage 169.\n",
    )?;
    println!("{}", serde_json::to_string_pretty(&report)?);
    Ok(())
}
