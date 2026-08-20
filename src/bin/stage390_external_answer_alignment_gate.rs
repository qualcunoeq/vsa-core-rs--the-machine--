//! Stage 390: govern answer alignment after page-aware candidate repair.

use serde_json::Value;
use sha2::{Digest, Sha256};
use std::collections::BTreeSet;
use std::fs;
use std::path::Path;

const DEV: &str = "docs/stage389_page_aware_external_problem_dev.json";
const SEALED: &str =
    "docs/holdouts/stage389_page_aware_external_problem_sealed_manifest.json";
const ALIGNMENT: &str = "docs/stage390_external_answer_alignment.json";
const JSON: &str = "docs/stage390_external_answer_alignment_gate.json";
const MD: &str = "docs/stage390_external_answer_alignment_gate.md";

fn digest(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

fn string<'a>(value: &'a Value, key: &str) -> &'a str {
    value[key].as_str().unwrap_or_else(|| panic!("missing {key}"))
}

fn hash_shape(value: &str) -> bool {
    value.len() == 64 && value.as_bytes().iter().all(u8::is_ascii_hexdigit)
}

fn strict(record: &Value) -> bool {
    record["quality_flags"]
        .as_array()
        .is_some_and(|flags| flags.is_empty())
        && string(record, "assembly_status") == "page_split_candidate_needs_alignment"
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let dev: Value = serde_json::from_str(&fs::read_to_string(DEV)?)?;
    let sealed: Value = serde_json::from_str(&fs::read_to_string(SEALED)?)?;
    let dev_records = dev.as_array().expect("development array");
    let sealed_records = sealed.as_array().expect("sealed array");
    let mut dev_ids = BTreeSet::new();
    let dev_ids_unique = dev_records
        .iter()
        .all(|record| dev_ids.insert(string(record, "record_id")));
    let sealed_ids_disjoint = sealed_records
        .iter()
        .all(|record| !dev_ids.contains(string(record, "record_id")));
    let sealed_prompt_free = sealed_records.iter().all(|record| {
        record.as_object().is_some_and(|object| !object.contains_key("prompt"))
    });
    let prompt_hashes_valid = dev_records.iter().all(|record| {
        digest(string(record, "prompt").as_bytes()) == string(record, "prompt_sha256")
    });
    let answer_free = dev_records.iter().all(|record| {
        let lower = string(record, "prompt").to_lowercase();
        string(record, "answer_key_status") == "not_read"
            && !lower.contains("answer")
            && !lower.contains("solution")
    });
    let input_material = dev_records
        .iter()
        .map(|record| {
            format!(
                "{}|{}|{}",
                string(record, "record_id"),
                string(record, "prompt_sha256"),
                string(record, "split")
            )
        })
        .collect::<Vec<_>>()
        .join("\n");
    let input_digest = digest(input_material.as_bytes());
    let strict_candidates = dev_records.iter().filter(|record| strict(record)).count();

    let alignment_present = Path::new(ALIGNMENT).exists();
    let mut alignment_records = Vec::new();
    let mut alignment_ids_unique = true;
    let mut alignment_nonsealed = true;
    let mut alignment_hashes_valid = true;
    let mut alignment_provenance_complete = true;
    if alignment_present {
        let alignment: Value = serde_json::from_str(&fs::read_to_string(ALIGNMENT)?)?;
        alignment_records = alignment["records"].as_array().cloned().unwrap_or_default();
        let mut ids = BTreeSet::new();
        for record in &alignment_records {
            let id = record["record_id"].as_str().unwrap_or("");
            alignment_ids_unique &= ids.insert(id);
            alignment_nonsealed &= dev_ids.contains(id)
                && sealed_records.iter().all(|sealed_record| {
                    string(sealed_record, "record_id") != id
                });
            alignment_hashes_valid &= record["answer_sha256"]
                .as_str()
                .is_some_and(hash_shape);
            alignment_provenance_complete &= record["source"]
                .as_str()
                .is_some_and(|source| !source.trim().is_empty())
                && record["source_sha256"]
                    .as_str()
                    .is_some_and(hash_shape);
        }
    }
    let alignment_valid = alignment_present
        && !alignment_records.is_empty()
        && alignment_ids_unique
        && alignment_nonsealed
        && alignment_hashes_valid
        && alignment_provenance_complete;
    let checks = [
        dev_ids_unique,
        sealed_ids_disjoint,
        sealed_prompt_free,
        prompt_hashes_valid,
        answer_free,
        !alignment_present || alignment_valid,
    ];
    let passed = checks.iter().filter(|check| **check).count();
    let integrity_passed = passed == checks.len();
    let report = serde_json::json!({
        "schema": "stage390-external-answer-alignment-gate-v1",
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
        "checks_passed": passed,
        "checks_total": checks.len(),
        "integrity_passed": integrity_passed,
        "baseline_ready": false,
        "baseline_readiness_reason": if alignment_present {
            "alignment exists but still requires a separately authorized evaluator and complete coverage"
        } else {
            "no separately governed development/validation answer alignment manifest is present"
        },
        "false_authorizations": 0,
        "production_mutations": 0,
    });
    assert!(integrity_passed, "stage390 gate failed: {checks:?}");
    fs::write(JSON, format!("{}\n", serde_json::to_string_pretty(&report)?))?;
    fs::write(
        MD,
        format!(
            "# Stage 390 — external answer-alignment gate\n\n- development / sealed records: {} / {}\n- strict candidates: {}\n- alignment manifest present: {}\n- alignment records: {}\n- answer plaintext read / sealed answer read: false / false\n- integrity checks: {}/{}\n- baseline ready: false\n\nThis gate consumes the repaired page-aware corpus. It never reads PDF answer keys, sealed prompt text, or sealed answers. A separately governed development/validation alignment manifest and authorized scorer are required before a baseline can run.\n\nReproduce with cargo run --quiet --bin stage390_external_answer_alignment_gate.\nMachine-readable report: {}\n",
            dev_records.len(),
            sealed_records.len(),
            strict_candidates,
            alignment_present,
            alignment_records.len(),
            passed,
            checks.len(),
            JSON,
        ),
    )?;
    println!(
        "stage390 records={} sealed={} strict={} alignment_present={} integrity={}/{} baseline_ready=false",
        dev_records.len(),
        sealed_records.len(),
        strict_candidates,
        alignment_present,
        passed,
        checks.len(),
    );
    Ok(())
}
