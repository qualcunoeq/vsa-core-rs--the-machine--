//! Stage 386: govern the answer-alignment boundary for the external corpus.
//!
//! Stage 385 deliberately contains prompts and provenance only. This gate
//! refuses baseline readiness unless a separately supplied development/
//! validation alignment manifest exists and covers only non-sealed records.
//! The default run therefore records the missing-input state without reading
//! any PDF answer key or sealed answer.

use serde_json::Value;
use sha2::{Digest, Sha256};
use std::collections::BTreeSet;
use std::fs;
use std::path::Path;

const DEV: &str = "docs/stage385_external_problem_dev.json";
const SEALED: &str = "docs/holdouts/stage385_external_problem_sealed_manifest.json";
const ALIGNMENT: &str = "docs/stage386_external_answer_alignment.json";
const JSON: &str = "docs/stage386_external_answer_alignment_gate.json";
const MD: &str = "docs/stage386_external_answer_alignment_gate.md";

fn digest_bytes(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

fn string_field<'a>(value: &'a Value, key: &str) -> &'a str {
    value
        .get(key)
        .and_then(Value::as_str)
        .unwrap_or_else(|| panic!("missing string field {key}"))
}

fn bool_count(checks: &[bool]) -> usize {
    checks.iter().filter(|check| **check).count()
}

fn strict_candidate(record: &Value) -> bool {
    record
        .get("quality_flags")
        .and_then(Value::as_array)
        .is_some_and(|flags| flags.is_empty())
        && string_field(record, "assembly_status") == "split_candidate_needs_alignment"
}

fn hash_shape(value: &str) -> bool {
    value.len() == 64 && value.as_bytes().iter().all(u8::is_ascii_hexdigit)
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let dev: Value = serde_json::from_str(&fs::read_to_string(DEV)?)?;
    let sealed: Value = serde_json::from_str(&fs::read_to_string(SEALED)?)?;
    let dev_records = dev.as_array().expect("development records array");
    let sealed_records = sealed.as_array().expect("sealed records array");

    let mut dev_ids = BTreeSet::new();
    let dev_ids_unique = dev_records
        .iter()
        .all(|record| dev_ids.insert(string_field(record, "record_id")));
    let sealed_ids_disjoint = sealed_records
        .iter()
        .all(|record| !dev_ids.contains(string_field(record, "record_id")));
    let sealed_prompt_free = sealed_records.iter().all(|record| {
        record
            .as_object()
            .is_some_and(|object| !object.contains_key("prompt"))
    });
    let dev_answer_free = dev_records.iter().all(|record| {
        let prompt = string_field(record, "prompt").to_lowercase();
        string_field(record, "answer_key_status") == "not_read"
            && !prompt.contains("answer")
            && !prompt.contains("solution")
    });
    let prompt_hashes_valid = dev_records.iter().all(|record| {
        let prompt = string_field(record, "prompt");
        digest_bytes(prompt.as_bytes()) == string_field(record, "prompt_sha256")
    });
    let input_material = dev_records
        .iter()
        .map(|record| {
            format!(
                "{}|{}|{}",
                string_field(record, "record_id"),
                string_field(record, "prompt_sha256"),
                string_field(record, "split")
            )
        })
        .collect::<Vec<_>>()
        .join("\n");
    let input_digest = digest_bytes(input_material.as_bytes());

    let strict_candidates = dev_records
        .iter()
        .filter(|record| strict_candidate(record))
        .count();
    let alignment_present = Path::new(ALIGNMENT).exists();
    let mut alignment_records = Vec::new();
    let mut alignment_valid = false;
    let mut alignment_ids_unique = true;
    let mut alignment_nonsealed = true;
    let mut alignment_hashes_valid = true;
    let mut alignment_provenance_complete = true;
    if alignment_present {
        let alignment: Value = serde_json::from_str(&fs::read_to_string(ALIGNMENT)?)?;
        alignment_records = alignment
            .get("records")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default();
        let mut ids = BTreeSet::new();
        for record in &alignment_records {
            let id = record
                .get("record_id")
                .and_then(Value::as_str)
                .unwrap_or("");
            alignment_ids_unique &= ids.insert(id);
            alignment_nonsealed &= dev_ids.contains(id)
                && !sealed_records
                    .iter()
                    .any(|sealed_record| string_field(sealed_record, "record_id") == id);
            alignment_hashes_valid &= record
                .get("answer_sha256")
                .and_then(Value::as_str)
                .is_some_and(hash_shape);
            alignment_provenance_complete &= record
                .get("source")
                .and_then(Value::as_str)
                .is_some_and(|source| !source.trim().is_empty())
                && record
                    .get("source_sha256")
                    .and_then(Value::as_str)
                    .is_some_and(hash_shape);
        }
        alignment_valid = !alignment_records.is_empty()
            && alignment_ids_unique
            && alignment_nonsealed
            && alignment_hashes_valid
            && alignment_provenance_complete;
    }

    let checks = [
        dev_ids_unique,
        sealed_ids_disjoint,
        sealed_prompt_free,
        dev_answer_free,
        prompt_hashes_valid,
        !alignment_present || alignment_valid,
    ];
    let integrity_passed = bool_count(&checks) == checks.len();
    // Hash-only answer alignment is not enough to run a scorer; an evaluator
    // must consume a separately authorized oracle implementation. This gate
    // therefore never claims readiness merely because a file exists.
    let baseline_ready = false;
    let report = serde_json::json!({
        "schema": "stage386-external-answer-alignment-gate-v1",
        "development_records": dev_records.len(),
        "sealed_records": sealed_records.len(),
        "strict_candidates": strict_candidates,
        "input_digest": input_digest,
        "alignment_path": ALIGNMENT,
        "alignment_present": alignment_present,
        "alignment_records": alignment_records.len(),
        "alignment_ids_unique": alignment_ids_unique,
        "alignment_nonsealed": alignment_nonsealed,
        "alignment_hashes_valid": alignment_hashes_valid,
        "alignment_provenance_complete": alignment_provenance_complete,
        "alignment_valid": alignment_valid,
        "answer_key_plaintext_read": false,
        "sealed_answer_read": false,
        "checks_passed": bool_count(&checks),
        "checks_total": checks.len(),
        "integrity_passed": integrity_passed,
        "baseline_ready": baseline_ready,
        "baseline_readiness_reason": if alignment_present {
            "alignment requires a separately authorized evaluator and complete independent coverage"
        } else {
            "no separately governed development/validation answer alignment manifest is present"
        },
        "false_authorizations": 0,
        "production_mutations": 0,
    });
    assert!(
        integrity_passed,
        "stage386 answer-alignment gate failed: {checks:?}"
    );
    fs::write(
        JSON,
        format!("{}\n", serde_json::to_string_pretty(&report)?),
    )?;
    fs::write(
        MD,
        format!(
            "# Stage 386 — external answer-alignment gate\n\n- development / sealed records: {} / {}\n- strict candidate count: {}\n- alignment manifest present: `{}`\n- alignment records: {}\n- answer plaintext read / sealed answer read: `false` / `false`\n- integrity checks: {}/{}\n- baseline ready: `false`\n\nThe Stage 385 corpus contains prompts and provenance only. This gate requires a separately governed development/validation answer-alignment manifest and still keeps baseline scoring disabled until an authorized evaluator consumes that oracle. It never reads PDF answer keys, sealed prompt text, or sealed answers.\n\nReproduce with `cargo run --quiet --bin stage386_external_answer_alignment_gate`.\nMachine-readable report: `{}`\n",
            dev_records.len(),
            sealed_records.len(),
            strict_candidates,
            alignment_present,
            alignment_records.len(),
            bool_count(&checks),
            checks.len(),
            JSON,
        ),
    )?;
    println!(
        "stage386 records={} sealed={} strict={} alignment_present={} integrity={}/{} baseline_ready=false",
        dev_records.len(),
        sealed_records.len(),
        strict_candidates,
        alignment_present,
        bool_count(&checks),
        checks.len(),
    );
    Ok(())
}
