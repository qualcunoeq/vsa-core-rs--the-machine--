//! Stage 385: split merged development candidates without touching the sealed holdout.
//!
//! Stage 384 showed that the PDF assembler still joins multiple numbered
//! exercises into one candidate. This stage performs a deterministic repair on
//! the already extracted development records only. It preserves the source
//! line and parent record for every child, deduplicates exact child prompts,
//! and leaves the hash-only sealed manifest byte-for-byte represented as
//! metadata. It does not read source solutions or create answer alignment.

use serde_json::Value;
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::fs;

const SOURCE_MANIFEST: &str = "docs/stage380_external_curriculum_source_manifest.json";
const DEV_IN: &str = "docs/stage383_external_problem_dev.json";
const SEALED_IN: &str = "docs/holdouts/stage383_external_problem_sealed_manifest.json";
const QUALITY_IN: &str = "docs/stage384_external_problem_quality_audit.json";
const JSON: &str = "docs/stage385_external_problem_candidate_repair.json";
const DEV_JSON: &str = "docs/stage385_external_problem_dev.json";
const SEALED_JSON: &str = "docs/holdouts/stage385_external_problem_sealed_manifest.json";
const MD: &str = "docs/stage385_external_problem_candidate_repair.md";

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

fn answer_like(prompt: &str) -> bool {
    let lower = prompt.to_lowercase();
    lower.contains("answer") || lower.contains("solution") || lower.contains("answer key")
}

fn marker_spans(prompt: &str) -> Vec<(usize, usize)> {
    let bytes = prompt.as_bytes();
    let mut markers = Vec::new();
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index].is_ascii_digit() && (index == 0 || bytes[index - 1].is_ascii_whitespace()) {
            let start = index;
            while index < bytes.len() && bytes[index].is_ascii_digit() {
                index += 1;
            }
            let number = prompt[start..index].parse::<u32>().unwrap_or(u32::MAX);
            let mut punctuation = index;
            while punctuation < bytes.len() && bytes[punctuation].is_ascii_whitespace() {
                punctuation += 1;
            }
            if number <= 999
                && punctuation < bytes.len()
                && (bytes[punctuation] == b'.' || bytes[punctuation] == b')')
            {
                let after = punctuation + 1;
                // Decimal numbers such as 2.5 are not exercise markers.
                if !(bytes[punctuation] == b'.'
                    && after < bytes.len()
                    && bytes[after].is_ascii_digit())
                    && (after == bytes.len() || bytes[after].is_ascii_whitespace())
                {
                    markers.push((start, after));
                }
            }
        }
        index += 1;
    }
    markers
}

fn split_prompt(prompt: &str) -> Vec<String> {
    let markers = marker_spans(prompt);
    if markers.len() < 2 {
        return vec![prompt.trim().to_string()];
    }
    let prefix = prompt[..markers[0].0].trim();
    let mut parts = Vec::new();
    for (position, (start, _)) in markers.iter().enumerate() {
        let end = markers
            .get(position + 1)
            .map(|(next, _)| *next)
            .unwrap_or(prompt.len());
        let body = prompt[*start..end].trim();
        if body.len() < 24 {
            continue;
        }
        let child = if prefix.is_empty() {
            body.to_string()
        } else {
            format!("{prefix} {body}")
        };
        parts.push(child);
    }
    if parts.len() >= 2 {
        parts
    } else {
        vec![prompt.trim().to_string()]
    }
}

fn quality_flags(prompt: &str, split_parent: bool) -> Vec<&'static str> {
    let lower = prompt.to_lowercase();
    let mut flags = Vec::new();
    if split_parent {
        // A PDF line can interleave columns, instructions, and numbered
        // subparts. Splitting is useful for review, but does not prove that
        // each child retained all of its context.
        flags.push("split_parent_context_review");
    }
    if marker_spans(prompt).len() > 1 {
        flags.push("multiple_numbered_items_remaining");
    }
    if lower.contains("figure ")
        || lower.contains("table ")
        || lower.contains("ⓐ")
        || lower.contains("ⓑ")
        || lower.contains("source:")
    {
        flags.push("layout_or_external_context");
    }
    if prompt.len() < 45 {
        flags.push("short_candidate");
    }
    let last = lower.split_whitespace().last().unwrap_or_default();
    if [
        "a", "an", "and", "are", "as", "at", "be", "by", "can", "for", "from", "if", "in", "into",
        "is", "of", "on", "or", "than", "that", "the", "to", "using", "which", "with", "will",
    ]
    .contains(&last.trim_matches(|c: char| !c.is_ascii_alphabetic()))
    {
        flags.push("truncated_tail_review");
    }
    flags
}

fn stable_records_digest(records: &[Value], sealed: bool) -> String {
    let material = records
        .iter()
        .map(|record| {
            let mut fields = vec![
                string_field(record, "record_id").to_string(),
                string_field(record, "source_path").to_string(),
                string_field(record, "source_sha256").to_string(),
                string_field(record, "prompt_sha256").to_string(),
                string_field(record, "split").to_string(),
                string_field(record, "assembly_status").to_string(),
                string_field(record, "answer_key_status").to_string(),
            ];
            if !sealed {
                fields.push(string_field(record, "prompt").to_string());
                fields.push(string_field(record, "parent_record_id").to_string());
            }
            fields.join("|")
        })
        .collect::<Vec<_>>()
        .join("\n");
    digest_bytes(material.as_bytes())
}

fn child_record(parent: &Value, prompt: String, index: usize, split_parent: bool) -> Value {
    let mut object = parent.as_object().expect("candidate object").clone();
    let parent_id = string_field(parent, "record_id").to_string();
    let prompt_sha256 = digest_bytes(prompt.as_bytes());
    object.insert(
        "record_id".into(),
        Value::String(format!("{parent_id}:part{index}")),
    );
    object.insert("parent_record_id".into(), Value::String(parent_id));
    object.insert("split_index".into(), Value::from(index as u64));
    object.insert("prompt".into(), Value::String(prompt.clone()));
    object.insert("prompt_sha256".into(), Value::String(prompt_sha256));
    object.insert(
        "assembly_status".into(),
        Value::String("split_candidate_needs_alignment".into()),
    );
    object.insert(
        "quality_flags".into(),
        Value::Array(
            quality_flags(&prompt, split_parent)
                .into_iter()
                .map(|flag| Value::String(flag.into()))
                .collect(),
        ),
    );
    Value::Object(object)
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let source: Value = serde_json::from_str(&fs::read_to_string(SOURCE_MANIFEST)?)?;
    let dev: Value = serde_json::from_str(&fs::read_to_string(DEV_IN)?)?;
    let sealed: Value = serde_json::from_str(&fs::read_to_string(SEALED_IN)?)?;
    let quality: Value = serde_json::from_str(&fs::read_to_string(QUALITY_IN)?)?;
    let parents = dev.as_array().expect("development array");
    let sealed_records = sealed.as_array().expect("sealed array").clone();
    let source_by_path = source["files"]
        .as_array()
        .expect("source files")
        .iter()
        .map(|record| (string_field(record, "path"), record))
        .collect::<BTreeMap<_, _>>();

    let mut repaired = Vec::new();
    let mut source_parent_count = 0usize;
    let mut split_parent_count = 0usize;
    let mut child_count_before_dedup = 0usize;
    let mut parent_hashes_ok = true;
    let mut source_refs_ok = true;
    for parent in parents {
        let prompt = string_field(parent, "prompt");
        parent_hashes_ok &=
            digest_bytes(prompt.as_bytes()) == string_field(parent, "prompt_sha256");
        source_refs_ok &= source_by_path
            .get(string_field(parent, "source_path"))
            .is_some_and(|source_record| {
                string_field(source_record, "sha256") == string_field(parent, "source_sha256")
            });
        source_parent_count += 1;
        let parts = split_prompt(prompt);
        let split_parent = parts.len() > 1;
        if parts.len() > 1 {
            split_parent_count += 1;
        }
        child_count_before_dedup += parts.len();
        for (index, part) in parts.into_iter().enumerate() {
            repaired.push(child_record(parent, part, index + 1, split_parent));
        }
    }
    repaired.sort_by(|left, right| {
        string_field(left, "record_id").cmp(string_field(right, "record_id"))
    });

    let mut prompt_hashes = BTreeSet::new();
    let duplicate_children = repaired
        .iter()
        .filter(|record| !prompt_hashes.insert(string_field(record, "prompt_sha256")))
        .count();
    let mut deduplicated = Vec::new();
    let mut seen = BTreeSet::new();
    for record in repaired {
        if seen.insert(string_field(&record, "prompt_sha256").to_string()) {
            deduplicated.push(record);
        }
    }
    let dedup_count = deduplicated.len();
    let answer_free = deduplicated
        .iter()
        .all(|record| !answer_like(string_field(record, "prompt")));
    let split_ok = deduplicated
        .iter()
        .all(|record| matches!(string_field(record, "split"), "development" | "validation"));
    let sealed_prompt_free = sealed_records.iter().all(|record| {
        record
            .as_object()
            .is_some_and(|object| !object.contains_key("prompt"))
    });
    let mut ids = BTreeSet::new();
    let ids_unique = deduplicated
        .iter()
        .all(|record| ids.insert(string_field(record, "record_id")));
    let sealed_disjoint = sealed_records
        .iter()
        .all(|record| !ids.contains(string_field(record, "record_id")));
    let source_manifest_ok = string_field(&quality, "source_manifest_sha256")
        == string_field(&source, "manifest_sha256");
    let repaired_digest = stable_records_digest(&deduplicated, false);
    let sealed_digest = stable_records_digest(&sealed_records, true);
    let replay_material = format!("{repaired_digest}|{sealed_digest}");
    let replay_digest = digest_bytes(replay_material.as_bytes());
    let replay_verified = digest_bytes(replay_material.as_bytes()) == replay_digest;
    let checks = [
        source_parent_count == parents.len(),
        parent_hashes_ok,
        source_refs_ok,
        source_manifest_ok,
        answer_free,
        split_ok,
        sealed_prompt_free,
        ids_unique,
        sealed_disjoint,
        replay_verified,
    ];
    let integrity_passed = bool_count(&checks) == checks.len();
    let clean_count = deduplicated
        .iter()
        .filter(|record| {
            record["quality_flags"]
                .as_array()
                .is_some_and(|flags| flags.is_empty())
        })
        .count();
    let mut quality_flag_counts: BTreeMap<String, usize> = BTreeMap::new();
    for record in &deduplicated {
        for flag in record["quality_flags"].as_array().expect("quality flags") {
            *quality_flag_counts
                .entry(flag.as_str().expect("quality flag").to_string())
                .or_default() += 1;
        }
    }

    let report = serde_json::json!({
        "schema": "stage385-external-problem-candidate-repair-v1",
        "quality_audit_sha256": digest_bytes(fs::read(QUALITY_IN)?.as_slice()),
        "source_manifest_sha256": string_field(&source, "manifest_sha256"),
        "parent_candidates": parents.len(),
        "split_parent_candidates": split_parent_count,
        "child_candidates_before_dedup": child_count_before_dedup,
        "child_candidates_after_dedup": dedup_count,
        "duplicate_child_prompts_removed": duplicate_children,
        "clean_candidates_after_repair": clean_count,
        "quality_flag_counts": quality_flag_counts,
        "sealed_holdout_count": sealed_records.len(),
        "sealed_prompt_text_stored": false,
        "answer_key_policy": "answer keys are never read, parsed, inferred, or stored",
        "repaired_digest": repaired_digest,
        "sealed_metadata_digest": sealed_digest,
        "replay_digest": replay_digest,
        "replay_verified": replay_verified,
        "checks_passed": bool_count(&checks),
        "checks_total": checks.len(),
        "integrity_passed": integrity_passed,
        "baseline_ready": false,
        "baseline_readiness_reason": "candidate splitting is improved, but answer alignment and exact problem quality review remain incomplete",
        "false_authorizations": 0,
        "production_mutations": 0,
    });
    assert!(
        integrity_passed,
        "stage385 candidate repair failed: {checks:?}"
    );
    fs::write(
        DEV_JSON,
        format!("{}\n", serde_json::to_string_pretty(&deduplicated)?),
    )?;
    fs::write(
        SEALED_JSON,
        format!("{}\n", serde_json::to_string_pretty(&sealed_records)?),
    )?;
    fs::write(
        JSON,
        format!("{}\n", serde_json::to_string_pretty(&report)?),
    )?;
    fs::write(
        MD,
        format!(
            "# Stage 385 — external problem candidate repair\n\n- parent candidates / split parents: {} / {}\n- child candidates before / after deduplication: {} / {}\n- duplicate child prompts removed: {}\n- clean candidates after repair: {}\n- quality flags: `{}`\n- sealed holdout records: {}\n- sealed prompt text stored: `false`\n- integrity checks: {}/{}\n- replay verified: `{}`\n- baseline ready: `false`\n\nThis stage splits only already-extracted development candidates, preserving each parent record, source line, and prompt hash lineage. Exact duplicate child prompts are retained once and counted. The sealed holdout is copied as metadata only; no sealed prompt text or answer key is read. The result remains a candidate corpus, not a scored benchmark, until exact problem review and independently governed answer alignment are complete.\n\nReproduce with `cargo run --quiet --bin stage385_external_problem_candidate_repair`.\nMachine-readable report: `{}`\n",
            parents.len(),
            split_parent_count,
            child_count_before_dedup,
            dedup_count,
            duplicate_children,
            clean_count,
            serde_json::to_string(&quality_flag_counts)?,
            sealed_records.len(),
            bool_count(&checks),
            checks.len(),
            replay_verified,
            JSON,
        ),
    )?;
    println!(
        "stage385 parents={} split_parents={} children={} dedup={} clean={} sealed={} integrity={}/{} baseline_ready=false",
        parents.len(),
        split_parent_count,
        child_count_before_dedup,
        dedup_count,
        clean_count,
        sealed_records.len(),
        bool_count(&checks),
        checks.len(),
    );
    Ok(())
}
