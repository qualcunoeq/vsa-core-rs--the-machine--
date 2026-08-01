//! Phase 41 global HLE typed-target census.
//!
//! This pass combines already frozen diagnostic artifacts and clusters by
//! output artifact plus transformation signature.  It never synthesizes or
//! promotes a capability; apparent repetition remains a candidate for external
//! validation only.

use serde::Serialize;
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::env;
use std::fs;

const HLE_DATASET: &str = "data/hle.jsonl";
const METHOD_REPORT: &str = "docs/phase29_hle_reasoning_method_audit.json";
const LAW_REPORT: &str = "docs/phase30_hle_law_audit.json";
const MECHANICS_REPORT: &str = "docs/phase40_hle_mechanics_target_audit.json";

#[derive(Debug, Clone, Serialize)]
struct TargetRecord {
    id: String,
    sources: BTreeSet<String>,
    output_artifact: String,
    input_artifacts: BTreeSet<String>,
    transformation_signature: String,
    prerequisite_class: String,
    audit_classes: BTreeSet<String>,
}

#[derive(Debug, Serialize)]
struct CandidateFamily {
    key: String,
    cases: usize,
    source_pools: BTreeSet<String>,
    output_artifact: String,
    transformation_signature: String,
    status: String,
    reason: String,
}

#[derive(Debug, Serialize)]
struct Report {
    schema_version: String,
    hle_dataset_sha256: String,
    input_report_hashes: BTreeMap<String, String>,
    requested_pool_counts: BTreeMap<String, usize>,
    unmaterialized_pool_notes: BTreeMap<String, String>,
    materialized_unique_cases: usize,
    duplicate_case_ids_collapsed: usize,
    target_output_counts: BTreeMap<String, usize>,
    transformation_counts: BTreeMap<String, usize>,
    prerequisite_counts: BTreeMap<String, usize>,
    candidate_families: Vec<CandidateFamily>,
    specialist_singletons: usize,
    unresolved_residuals: usize,
    records: Vec<TargetRecord>,
    method: String,
}

fn sha256(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

fn string_set(value: Option<&Value>) -> BTreeSet<String> {
    value
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .map(str::to_string)
        .collect()
}

fn insert_record(
    records: &mut BTreeMap<String, TargetRecord>,
    id: &str,
    source: &str,
    output_artifact: String,
    input_artifacts: BTreeSet<String>,
    transformation_signature: String,
    prerequisite_class: String,
    audit_class: String,
) {
    let entry = records
        .entry(id.to_string())
        .or_insert_with(|| TargetRecord {
            id: id.to_string(),
            sources: BTreeSet::new(),
            output_artifact: output_artifact.clone(),
            input_artifacts: input_artifacts.clone(),
            transformation_signature: transformation_signature.clone(),
            prerequisite_class: prerequisite_class.clone(),
            audit_classes: BTreeSet::new(),
        });
    entry.sources.insert(source.to_string());
    entry.input_artifacts.extend(input_artifacts);
    entry.audit_classes.insert(audit_class);
    if entry.output_artifact == "unknown" && output_artifact != "unknown" {
        entry.output_artifact = output_artifact;
    }
    if entry.transformation_signature == "unknown" && transformation_signature != "unknown" {
        entry.transformation_signature = transformation_signature;
    }
    if entry.prerequisite_class == "unknown" && prerequisite_class != "unknown" {
        entry.prerequisite_class = prerequisite_class;
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let hle_bytes = fs::read(HLE_DATASET)?;
    let method_bytes = fs::read(METHOD_REPORT)?;
    let law_bytes = fs::read(LAW_REPORT)?;
    let mechanics_bytes = fs::read(MECHANICS_REPORT)?;
    let method: Value = serde_json::from_slice(&method_bytes)?;
    let law: Value = serde_json::from_slice(&law_bytes)?;
    let mechanics: Value = serde_json::from_slice(&mechanics_bytes)?;
    let mut records = BTreeMap::new();
    let mut requested_pool_counts = BTreeMap::new();
    requested_pool_counts.insert("missing_method_cases".into(), 222);
    requested_pool_counts.insert("in_question_equations".into(), 34);
    requested_pool_counts.insert("representation_bridges".into(), 29);
    requested_pool_counts.insert("derivation_after_retrieval".into(), 189);
    requested_pool_counts.insert("mechanics_target_residuals".into(), 152);

    for row in method
        .get("cases")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
    {
        let id = row.get("id").and_then(Value::as_str).unwrap_or("unknown");
        let output = row
            .get("output_artifact")
            .and_then(Value::as_str)
            .unwrap_or("unknown")
            .to_string();
        let input = string_set(row.get("representation_cues"));
        let signature = row
            .get("signature")
            .and_then(Value::as_str)
            .unwrap_or("unknown")
            .to_string();
        let prerequisite = if row
            .get("prerequisite_cues")
            .and_then(Value::as_array)
            .is_some_and(|cues| !cues.is_empty())
        {
            "knowledge_or_assumptions"
        } else if input.is_empty() {
            "unknown"
        } else {
            "representation"
        };
        insert_record(
            &mut records,
            id,
            "phase29_method_audit",
            output,
            input,
            signature,
            prerequisite.into(),
            row.get("audit_class")
                .and_then(Value::as_str)
                .unwrap_or("missing_method")
                .into(),
        );
    }

    for row in law
        .get("cases")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
    {
        if row.get("outcome").and_then(Value::as_str) != Some("in_question_equation") {
            continue;
        }
        let id = row.get("id").and_then(Value::as_str).unwrap_or("unknown");
        let mut input = BTreeSet::new();
        input.insert("typed_equation_or_law_context".into());
        let signature = row
            .get("bridge_primitives")
            .and_then(Value::as_array)
            .map(|values| {
                values
                    .iter()
                    .filter_map(Value::as_str)
                    .collect::<Vec<_>>()
                    .join("+")
            })
            .filter(|value| !value.is_empty())
            .unwrap_or_else(|| "equation_binding".into());
        insert_record(
            &mut records,
            id,
            "phase30_in_question_equations",
            row.get("requested_output")
                .and_then(Value::as_str)
                .unwrap_or("equation_solution")
                .into(),
            input,
            signature,
            "knowledge_or_assumptions".into(),
            "in_question_equation".into(),
        );
    }

    for row in mechanics
        .get("records")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
    {
        let id = row.get("id").and_then(Value::as_str).unwrap_or("unknown");
        let mut input = BTreeSet::new();
        input.insert("mechanics_signal".into());
        insert_record(
            &mut records,
            id,
            "phase40_mechanics_target_audit",
            row.get("artifact_family")
                .and_then(Value::as_str)
                .unwrap_or("unknown")
                .into(),
            input,
            "target_grounding".into(),
            row.get("subdomain")
                .and_then(Value::as_str)
                .unwrap_or("unknown")
                .into(),
            "mechanics_target_residual".into(),
        );
    }

    let mut target_output_counts = BTreeMap::new();
    let mut transformation_counts = BTreeMap::new();
    let mut prerequisite_counts = BTreeMap::new();
    let mut family_buckets: BTreeMap<String, Vec<&TargetRecord>> = BTreeMap::new();
    for record in records.values() {
        *target_output_counts
            .entry(record.output_artifact.clone())
            .or_insert(0) += 1;
        *transformation_counts
            .entry(record.transformation_signature.clone())
            .or_insert(0) += 1;
        *prerequisite_counts
            .entry(record.prerequisite_class.clone())
            .or_insert(0) += 1;
        let key = format!(
            "{}::{}",
            record.output_artifact, record.transformation_signature
        );
        family_buckets.entry(key).or_default().push(record);
    }
    let mut candidate_families = Vec::new();
    let mut specialist_singletons = 0;
    for (key, members) in family_buckets {
        let source_pools: BTreeSet<String> = members
            .iter()
            .flat_map(|member| member.sources.iter().cloned())
            .collect();
        let generic = members[0].transformation_signature == "generic_specialist"
            || members[0].transformation_signature == "target_grounding"
            || members[0].output_artifact == "unknown"
            || members[0].output_artifact == "unclassified"
            || members[0].output_artifact == "ambiguous";
        let status = if members.len() >= 8 && !generic {
            "candidate_for_external_validation"
        } else if members.len() == 1 {
            specialist_singletons += 1;
            "specialist_singleton"
        } else {
            "insufficient_coherence_evidence"
        };
        candidate_families.push(CandidateFamily {
            key,
            cases: members.len(),
            source_pools,
            output_artifact: members[0].output_artifact.clone(),
            transformation_signature: members[0].transformation_signature.clone(),
            status: status.into(),
            reason: if status == "candidate_for_external_validation" {
                "repeated output and exact transformation signature; external corpus and semantic review still required".into()
            } else {
                "no stable repeated typed transformation at this census level".into()
            },
        });
    }
    candidate_families
        .sort_by(|left, right| right.cases.cmp(&left.cases).then(left.key.cmp(&right.key)));
    let duplicate_case_ids_collapsed = 222 + 34 + 152 - records.len();
    let report = Report {
        schema_version: "phase41.hle.global.typed.target.census.v1".into(),
        hle_dataset_sha256: sha256(&hle_bytes),
        input_report_hashes: [
            ("phase29_method_audit".into(), sha256(&method_bytes)),
            ("phase30_law_audit".into(), sha256(&law_bytes)),
            ("phase40_mechanics_target_audit".into(), sha256(&mechanics_bytes)),
        ]
        .into_iter()
        .collect(),
        requested_pool_counts,
        unmaterialized_pool_notes: [
            (
                "derivation_after_retrieval".into(),
                "Phase 21 preserves only an aggregate count (189); no immutable per-question machine-readable artifact was available, so no synthetic records were created.".into(),
            ),
        ]
        .into_iter()
        .collect(),
        materialized_unique_cases: records.len(),
        duplicate_case_ids_collapsed,
        target_output_counts,
        transformation_counts,
        prerequisite_counts,
        candidate_families,
        specialist_singletons,
        unresolved_residuals: records
            .values()
            .filter(|record| record.output_artifact == "unknown" || record.output_artifact == "unclassified")
            .count(),
        records: records.into_values().collect(),
        method: "diagnostic census of frozen HLE audit artifacts; exact typed output/transformation signatures only; no capability synthesis, execution, or promotion".into(),
    };
    let output = serde_json::to_string_pretty(&report)?;
    let path = env::args()
        .nth(1)
        .unwrap_or_else(|| "docs/phase41_hle_global_target_census.json".into());
    fs::write(&path, output)?;
    println!("phase41 report written to {path}");
    Ok(())
}
