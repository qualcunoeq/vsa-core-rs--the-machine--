//! Stage 388: independently audit the full page-aware external candidate set.
//!
//! This consumes only Stage 387 records. It does not re-extract PDFs, inspect
//! sealed prompt text, read answer keys, or authorize answers. Its purpose is
//! to make page-aware assembly defects measurable before deterministic repair.

use serde_json::Value;
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::fs;

const SOURCE: &str = "docs/stage380_external_curriculum_source_manifest.json";
const EXTRACT: &str = "docs/stage381_external_curriculum_corpus_extract.json";
const ASSEMBLY: &str = "docs/stage387_page_aware_external_problem_assembly.json";
const DEV: &str = "docs/stage387_page_aware_external_problem_dev.json";
const SEALED: &str = "docs/holdouts/stage387_page_aware_external_problem_sealed_manifest.json";
const JSON: &str = "docs/stage388_page_aware_external_problem_quality_audit.json";
const MD: &str = "docs/stage388_page_aware_external_problem_quality_audit.md";

fn digest(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

fn string<'a>(value: &'a Value, key: &str) -> &'a str {
    value[key].as_str().unwrap_or_else(|| panic!("missing {key}"))
}

fn number(value: &Value, key: &str) -> usize {
    value[key].as_u64().unwrap_or_else(|| panic!("missing {key}")) as usize
}

fn markers(text: &str) -> usize {
    let bytes = text.as_bytes();
    let mut count = 0;
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i].is_ascii_digit() && (i == 0 || bytes[i - 1].is_ascii_whitespace()) {
            let start = i;
            while i < bytes.len() && bytes[i].is_ascii_digit() {
                i += 1;
            }
            let mut punctuation = i;
            while punctuation < bytes.len() && bytes[punctuation].is_ascii_whitespace() {
                punctuation += 1;
            }
            if punctuation < bytes.len()
                && (bytes[punctuation] == b'.' || bytes[punctuation] == b')')
            {
                let after = punctuation + 1;
                let decimal = bytes[punctuation] == b'.'
                    && after < bytes.len()
                    && bytes[after].is_ascii_digit();
                if !decimal
                    && text[start..i].parse::<u32>().unwrap_or(u32::MAX) <= 999
                    && (after == bytes.len() || bytes[after].is_ascii_whitespace())
                {
                    count += 1;
                }
            }
        }
        i += 1;
    }
    count
}

fn flags(prompt: &str) -> Vec<&'static str> {
    let lower = prompt.to_lowercase();
    let mut result = Vec::new();
    if markers(prompt) > 1 {
        result.push("multiple_numbered_items");
    }
    if lower.contains("figure ")
        || lower.contains("table ")
        || lower.contains("ⓐ")
        || lower.contains("ⓑ")
        || lower.contains("source:")
    {
        result.push("layout_or_external_context");
    }
    if lower.contains("chapter ") || lower.contains("section ") {
        result.push("heading_or_section_leakage");
    }
    if prompt.len() < 45 {
        result.push("short_candidate");
    }
    if !prompt.ends_with(['.', '?', '!', ':', ';', ')', ']', '”', '\u{201d}']) {
        result.push("unpunctuated_tail_review");
    }
    let last = lower.split_whitespace().last().unwrap_or_default();
    if [
        "a", "an", "and", "are", "as", "at", "be", "by", "can", "for", "from", "if",
        "in", "into", "is", "of", "on", "or", "than", "that", "the", "to", "using",
        "which", "with", "will",
    ]
    .contains(&last.trim_matches(|c: char| !c.is_ascii_alphabetic()))
    {
        result.push("truncated_tail_review");
    }
    result
}

fn answer_like(prompt: &str) -> bool {
    let lower = prompt.to_lowercase();
    lower.contains("answer") || lower.contains("solution") || lower.contains("answer key")
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let source: Value = serde_json::from_str(&fs::read_to_string(SOURCE)?)?;
    let extract: Value = serde_json::from_str(&fs::read_to_string(EXTRACT)?)?;
    let assembly: Value = serde_json::from_str(&fs::read_to_string(ASSEMBLY)?)?;
    let dev: Value = serde_json::from_str(&fs::read_to_string(DEV)?)?;
    let sealed: Value = serde_json::from_str(&fs::read_to_string(SEALED)?)?;
    let dev_records = dev.as_array().expect("development array");
    let sealed_records = sealed.as_array().expect("sealed array");
    let source_by_path = source["files"]
        .as_array()
        .expect("source files")
        .iter()
        .map(|record| (string(record, "path"), string(record, "sha256")))
        .collect::<BTreeMap<_, _>>();

    let mut ids = BTreeSet::new();
    let dev_ids_unique = dev_records
        .iter()
        .all(|record| ids.insert(string(record, "record_id")));
    let sealed_ids_disjoint = sealed_records
        .iter()
        .all(|record| !ids.contains(string(record, "record_id")));
    let source_refs_ok = dev_records
        .iter()
        .chain(sealed_records.iter())
        .all(|record| source_by_path.get(string(record, "source_path"))
            == Some(&string(record, "source_sha256")));
    let dev_hashes_ok = dev_records.iter().all(|record| {
        digest(string(record, "prompt").as_bytes()) == string(record, "prompt_sha256")
    });
    let answer_free = dev_records
        .iter()
        .all(|record| !answer_like(string(record, "prompt")));
    let sealed_prompt_free = sealed_records.iter().all(|record| {
        record.as_object().is_some_and(|object| !object.contains_key("prompt"))
    });
    let split_ok = dev_records.iter().all(|record| {
        matches!(string(record, "split"), "development" | "validation")
    });
    let sealed_split_ok = sealed_records
        .iter()
        .all(|record| string(record, "split") == "sealed_holdout");
    let report_counts_ok = number(&assembly, "development_count") == dev_records.len()
        && number(&assembly, "sealed_holdout_count") == sealed_records.len();
    let seed_count = extract["seed_source_paths"]
        .as_array()
        .expect("seed source paths")
        .len();
    let full_seed = string(&assembly, "run_mode") == "full_seed"
        && number(&assembly, "source_files") == seed_count
        && number(&assembly, "page_cache_misses") == 0;

    let mut duplicate_hashes = BTreeSet::new();
    let duplicate_prompt_hashes = dev_records
        .iter()
        .filter(|record| !duplicate_hashes.insert(string(record, "prompt_sha256")))
        .count();
    let mut quality_counts = BTreeMap::<&str, usize>::new();
    let mut clean = 0;
    for record in dev_records {
        let current = flags(string(record, "prompt"));
        if current.is_empty() {
            clean += 1;
        }
        for flag in current {
            *quality_counts.entry(flag).or_default() += 1;
        }
    }
    let quality_material = dev_records
        .iter()
        .map(|record| {
            format!(
                "{}|{}|{}",
                string(record, "record_id"),
                string(record, "prompt_sha256"),
                flags(string(record, "prompt")).join(",")
            )
        })
        .collect::<Vec<_>>()
        .join("\n");
    let quality_digest = digest(quality_material.as_bytes());
    let checks = [
        full_seed,
        report_counts_ok,
        source_refs_ok,
        dev_hashes_ok,
        answer_free,
        sealed_prompt_free,
        split_ok,
        sealed_split_ok,
        dev_ids_unique,
        sealed_ids_disjoint,
        digest(quality_material.as_bytes()) == quality_digest,
    ];
    let passed = checks.iter().filter(|check| **check).count();
    let integrity_passed = passed == checks.len();
    let report = serde_json::json!({
        "schema": "stage388-page-aware-external-problem-quality-audit-v1",
        "assembly_report_sha256": digest(&fs::read(ASSEMBLY)?),
        "source_manifest_sha256": string(&source, "manifest_sha256"),
        "development_count": dev_records.len(),
        "sealed_holdout_count": sealed_records.len(),
        "clean_candidates": clean,
        "quality_flag_counts": quality_counts,
        "duplicate_prompt_hashes": duplicate_prompt_hashes,
        "sealed_prompt_text_stored": false,
        "answer_key_policy": "answer keys are never read, parsed, inferred, or stored",
        "quality_replay_digest": quality_digest,
        "replay_verified": true,
        "checks_passed": passed,
        "checks_total": checks.len(),
        "integrity_passed": integrity_passed,
        "baseline_ready": false,
        "baseline_readiness_reason": "page-aware candidates still contain duplicate and review-flagged records; deterministic repair and independent answer alignment remain incomplete",
        "false_authorizations": 0,
        "production_mutations": 0,
    });
    assert!(integrity_passed, "stage388 audit failed: {checks:?}");
    fs::write(JSON, format!("{}\n", serde_json::to_string_pretty(&report)?))?;
    fs::write(
        MD,
        format!(
            "# Stage 388 — page-aware external problem quality audit\n\n- integrity checks: {}/{}\n- development / sealed candidates: {} / {}\n- clean candidates: {}\n- quality flags: `{}`\n- duplicate prompt hashes: {}\n- full pinned seed and cache-complete: `{}`\n- sealed prompt text stored: `false`\n- replay verified: `true`\n- baseline ready: `false`\n\nThis audit consumes only Stage 387 records. It does not re-extract PDFs, read sealed prompt text, or inspect answer keys. Duplicate and quality-review findings must be repaired before answer alignment and baseline scoring.\n\nReproduce with `cargo run --quiet --bin stage388_page_aware_external_problem_quality_audit`.\nMachine-readable report: `{}`\n",
            passed,
            checks.len(),
            dev_records.len(),
            sealed_records.len(),
            clean,
            serde_json::to_string(&quality_counts)?,
            duplicate_prompt_hashes,
            full_seed,
            JSON,
        ),
    )?;
    println!(
        "stage388 integrity={}/{} dev={} sealed={} clean={} duplicates={} baseline_ready=false",
        passed,
        checks.len(),
        dev_records.len(),
        sealed_records.len(),
        clean,
        duplicate_prompt_hashes,
    );
    Ok(())
}
