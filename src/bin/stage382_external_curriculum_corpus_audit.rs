//! Stage 382: audit the external corpus seed before any baseline scoring.
//!
//! This audit is intentionally independent of the extractor's control flow.
//! It verifies source hashes, split isolation, answer-like rejection, duplicate
//! behavior, and receipt hashes. It also refuses to call line-level extraction
//! a benchmark: complete problem assembly and answer-key alignment remain
//! explicit follow-up work.

use serde_json::Value;
use sha2::{Digest, Sha256};
use std::collections::BTreeSet;
use std::fs;

const SOURCE_MANIFEST: &str = "docs/stage380_external_curriculum_source_manifest.json";
const EXTRACT: &str = "docs/stage381_external_curriculum_corpus_extract.json";
const SEALED: &str = "docs/holdouts/stage381_external_curriculum_sealed_manifest.json";
const JSON: &str = "docs/stage382_external_curriculum_corpus_audit.json";
const MD: &str = "docs/stage382_external_curriculum_corpus_audit.md";

fn string_field<'a>(value: &'a Value, key: &str) -> &'a str {
    value
        .get(key)
        .and_then(Value::as_str)
        .unwrap_or_else(|| panic!("missing string field {key}"))
}

fn bool_count(checks: &[bool]) -> usize {
    checks.iter().filter(|check| **check).count()
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let source: Value = serde_json::from_str(&fs::read_to_string(SOURCE_MANIFEST)?)?;
    let extract: Value = serde_json::from_str(&fs::read_to_string(EXTRACT)?)?;
    let sealed: Value = serde_json::from_str(&fs::read_to_string(SEALED)?)?;
    let source_files = source
        .get("files")
        .and_then(Value::as_array)
        .expect("source files array");
    let seed_paths = extract
        .get("seed_source_paths")
        .and_then(Value::as_array)
        .expect("seed source paths array");
    let dev_records = extract
        .get("development_records")
        .and_then(Value::as_array)
        .expect("development records array");
    let sealed_records = sealed.as_array().expect("sealed records array");

    let source_manifest_sha256 = string_field(&extract, "source_manifest_sha256");
    let expected_source_manifest_sha256 = string_field(&source, "manifest_sha256");
    let source_hash_ok = source_manifest_sha256 == expected_source_manifest_sha256;
    let source_by_path = source_files
        .iter()
        .map(|record| (string_field(record, "path"), record))
        .collect::<std::collections::BTreeMap<_, _>>();
    let seed_paths_ok = seed_paths.iter().all(|path| {
        path.as_str()
            .map(|path| source_by_path.contains_key(path))
            .unwrap_or(false)
    });
    let source_drift_free = seed_paths.iter().all(|path| {
        let Some(path) = path.as_str() else {
            return false;
        };
        let expected = string_field(source_by_path[path], "sha256");
        let actual = digest_bytes(&fs::read(path).expect("seed source is readable"));
        expected == actual
    });

    let development_count = extract
        .get("development_count")
        .and_then(Value::as_u64)
        .expect("development count") as usize;
    let validation_count = extract
        .get("validation_count")
        .and_then(Value::as_u64)
        .expect("validation count") as usize;
    let sealed_count = extract
        .get("sealed_holdout_count")
        .and_then(Value::as_u64)
        .expect("sealed count") as usize;
    let counts_ok = dev_records.len() == development_count + validation_count
        && sealed_records.len() == sealed_count;

    let dev_splits_ok = dev_records
        .iter()
        .all(|record| matches!(string_field(record, "split"), "development" | "validation"));
    let sealed_splits_ok = sealed_records
        .iter()
        .all(|record| string_field(record, "split") == "sealed_holdout");
    let dev_answer_like_free = dev_records.iter().all(|record| {
        let prompt = string_field(record, "prompt").to_lowercase();
        !prompt.contains("answer") && !prompt.contains("solution")
    });
    let sealed_prompt_free = sealed_records.iter().all(|record| {
        record
            .as_object()
            .is_some_and(|object| !object.contains_key("prompt"))
    });

    let mut dev_hashes = BTreeSet::new();
    let dev_duplicates = dev_records
        .iter()
        .filter(|record| !dev_hashes.insert(string_field(record, "prompt_sha256")))
        .count();
    let mut record_ids = BTreeSet::new();
    let dev_ids_unique = dev_records
        .iter()
        .all(|record| record_ids.insert(string_field(record, "record_id")));
    let sealed_ids_disjoint = sealed_records
        .iter()
        .all(|record| !record_ids.contains(string_field(record, "record_id")));
    let prompt_hashes_recompute = dev_records.iter().all(|record| {
        let prompt = string_field(record, "prompt");
        digest_bytes(prompt.as_bytes()) == string_field(record, "prompt_sha256")
    });

    let sealed_hash = stable_sealed_digest(sealed_records);
    let recorded_sealed_hash = string_field(&extract, "sealed_holdout_manifest_sha256");
    let sealed_hash_ok = sealed_hash == recorded_sealed_hash;
    let corpus_hash = digest_bytes(
        format!(
            "dev:{}\nsealed:{}",
            stable_development_digest(dev_records),
            sealed_hash
        )
        .as_bytes(),
    );
    let corpus_hash_ok = corpus_hash == string_field(&extract, "corpus_sha256");

    let checks = [
        source_hash_ok,
        seed_paths_ok,
        source_drift_free,
        counts_ok,
        dev_splits_ok,
        sealed_splits_ok,
        dev_answer_like_free,
        sealed_prompt_free,
        dev_duplicates == 0,
        dev_ids_unique,
        sealed_ids_disjoint,
        prompt_hashes_recompute,
        sealed_hash_ok,
        corpus_hash_ok,
    ];
    let integrity_passed = bool_count(&checks) == checks.len();
    let instruction_count = dev_records
        .iter()
        .filter(|record| string_field(record, "candidate_kind") == "instruction")
        .count();
    let numbered_count = dev_records
        .iter()
        .filter(|record| string_field(record, "candidate_kind") == "numbered_problem")
        .count();
    let report = serde_json::json!({
        "schema": "stage382-external-curriculum-corpus-audit-v1",
        "source_manifest_sha256": expected_source_manifest_sha256,
        "source_hash_ok": source_hash_ok,
        "seed_paths_ok": seed_paths_ok,
        "source_drift_free": source_drift_free,
        "source_files": seed_paths.len(),
        "development_count": development_count,
        "validation_count": validation_count,
        "sealed_holdout_count": sealed_count,
        "instruction_records": instruction_count,
        "numbered_records": numbered_count,
        "dev_answer_like_free": dev_answer_like_free,
        "sealed_prompt_free": sealed_prompt_free,
        "dev_duplicate_prompt_hashes": dev_duplicates,
        "dev_ids_unique": dev_ids_unique,
        "sealed_ids_disjoint": sealed_ids_disjoint,
        "prompt_hashes_recompute": prompt_hashes_recompute,
        "sealed_hash_ok": sealed_hash_ok,
        "corpus_hash_ok": corpus_hash_ok,
        "integrity_passed": integrity_passed,
        "baseline_ready": false,
        "baseline_readiness_reason": "records are line-level source candidates; complete multi-line problem assembly and answer-key alignment are not yet implemented",
        "checks_passed": bool_count(&checks),
        "checks_total": checks.len(),
        "corpus_sha256": string_field(&extract, "corpus_sha256"),
    });
    if !integrity_passed {
        eprintln!("external corpus integrity checks: {:?}", checks);
        eprintln!(
            "sealed recorded={} computed={}",
            recorded_sealed_hash, sealed_hash
        );
        eprintln!(
            "corpus recorded={} computed={}",
            string_field(&extract, "corpus_sha256"),
            corpus_hash
        );
    }
    assert!(integrity_passed, "external corpus integrity audit failed");
    fs::write(
        JSON,
        format!("{}\n", serde_json::to_string_pretty(&report)?),
    )?;
    fs::write(
        MD,
        format!(
            "# Stage 382 — external curriculum corpus audit\n\n- integrity checks: {}/{}\n- source files: {}\n- records with prompts: {} development + {} validation\n- sealed holdout records: {} (prompt text absent from development artifacts)\n- instruction / numbered candidates: {} / {}\n- duplicate prompt hashes: {}\n- baseline ready: `{}`\n- corpus SHA-256: `{}`\n\nThe source boundary, file hashes, split membership, answer-like filtering, duplicate policy, and replay hashes pass independently. Baseline scoring is intentionally not authorized yet: the extracted records are line-level candidates and require complete problem assembly plus answer-key alignment before they can be treated as benchmark questions.\n\nReproduce with `cargo run --quiet --bin stage382_external_curriculum_corpus_audit`.\nMachine-readable report: `{}`\n",
            bool_count(&checks),
            checks.len(),
            seed_paths.len(),
            development_count,
            validation_count,
            sealed_count,
            instruction_count,
            numbered_count,
            dev_duplicates,
            false,
            string_field(&extract, "corpus_sha256"),
            JSON,
        ),
    )?;
    println!(
        "stage382 integrity={}/{} baseline_ready=false sealed={} duplicates={} corpus_hash_ok={}",
        bool_count(&checks),
        checks.len(),
        sealed_count,
        dev_duplicates,
        corpus_hash_ok,
    );
    Ok(())
}

fn stable_sealed_digest(records: &[Value]) -> String {
    let material = records
        .iter()
        .map(|record| {
            format!(
                "{}|{}|{}|{}|{}|{}|{}|{}|{}",
                string_field(record, "record_id"),
                string_field(record, "source_path"),
                string_field(record, "source_family"),
                string_field(record, "source_sha256"),
                record.get("source_line").and_then(Value::as_u64).unwrap(),
                string_field(record, "candidate_kind"),
                string_field(record, "prompt_sha256"),
                string_field(record, "split"),
                string_field(record, "answer_key_status"),
            )
        })
        .collect::<Vec<_>>()
        .join("\n");
    digest_bytes(material.as_bytes())
}

fn stable_development_digest(records: &[Value]) -> String {
    let material = records
        .iter()
        .map(|record| {
            format!(
                "{}|{}|{}|{}|{}|{}|{}|{}|{}|{}",
                string_field(record, "record_id"),
                string_field(record, "source_path"),
                string_field(record, "source_family"),
                string_field(record, "source_sha256"),
                record.get("source_line").and_then(Value::as_u64).unwrap(),
                string_field(record, "candidate_kind"),
                string_field(record, "prompt_sha256"),
                string_field(record, "split"),
                string_field(record, "answer_key_status"),
                string_field(record, "prompt"),
            )
        })
        .collect::<Vec<_>>()
        .join("\n");
    digest_bytes(material.as_bytes())
}

fn digest_bytes(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}
