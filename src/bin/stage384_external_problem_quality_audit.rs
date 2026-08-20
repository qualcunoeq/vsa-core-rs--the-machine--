//! Stage 384: independently audit the quality of assembled external problems.
//!
//! Stage 383 produces question candidates, not yet benchmark problems. This
//! audit deliberately consumes only the committed development records and the
//! hash-only sealed manifest. It does not re-run extraction, inspect answer
//! keys, or attempt to repair candidates. Its purpose is to make the remaining
//! assembly defects measurable before answer alignment or baseline scoring.

use serde_json::Value;
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::fs;

const SOURCE_MANIFEST: &str = "docs/stage380_external_curriculum_source_manifest.json";
const REPORT: &str = "docs/stage383_external_problem_assembly.json";
const DEV: &str = "docs/stage383_external_problem_dev.json";
const SEALED: &str = "docs/holdouts/stage383_external_problem_sealed_manifest.json";
const JSON: &str = "docs/stage384_external_problem_quality_audit.json";
const MD: &str = "docs/stage384_external_problem_quality_audit.md";

fn digest_bytes(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

fn string_field<'a>(value: &'a Value, key: &str) -> &'a str {
    value
        .get(key)
        .and_then(Value::as_str)
        .unwrap_or_else(|| panic!("missing string field {key}"))
}

fn usize_field(value: &Value, key: &str) -> usize {
    value
        .get(key)
        .and_then(Value::as_u64)
        .unwrap_or_else(|| panic!("missing integer field {key}")) as usize
}

fn bool_count(checks: &[bool]) -> usize {
    checks.iter().filter(|check| **check).count()
}

fn answer_like(prompt: &str) -> bool {
    let lower = prompt.to_lowercase();
    lower.contains("answer") || lower.contains("solution") || lower.contains("answer key")
}

fn question_like(prompt: &str) -> bool {
    let lower = prompt.to_lowercase();
    lower.contains('?')
        || [
            "find ",
            "determine ",
            "solve ",
            "calculate ",
            "compute ",
            "evaluate ",
            "identify ",
            "describe ",
            "convert ",
            "simplify ",
            "write ",
            "draw ",
            "classify ",
            "list ",
            "state ",
            "give ",
            "show ",
            "prove ",
            "indicate ",
            "use ",
            "decide ",
            "compare ",
            "round ",
            "construct ",
        ]
        .iter()
        .any(|verb| lower.contains(verb))
}

fn numbered_markers(prompt: &str) -> usize {
    let bytes = prompt.as_bytes();
    let mut count = 0;
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index].is_ascii_digit() && (index == 0 || bytes[index - 1].is_ascii_whitespace()) {
            let start = index;
            while index < bytes.len() && bytes[index].is_ascii_digit() {
                index += 1;
            }
            if index < bytes.len() && (bytes[index] == b'.' || bytes[index] == b')') {
                let marker = index;
                index += 1;
                // Decimal numbers such as 2.5 are not exercise markers.
                if !(bytes[marker] == b'.' && index < bytes.len() && bytes[index].is_ascii_digit())
                    && start < prompt.len()
                {
                    count += 1;
                }
            }
        }
        index += 1;
    }
    count
}

fn quality_flags(prompt: &str) -> Vec<&'static str> {
    let lower = prompt.to_lowercase();
    let mut flags = Vec::new();
    if numbered_markers(prompt) > 1 {
        flags.push("multiple_numbered_items");
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
        "a", "an", "and", "are", "as", "at", "be", "by", "can", "for", "from", "if", "in", "into",
        "is", "of", "on", "or", "than", "that", "the", "to", "using", "which", "with", "will",
    ]
    .contains(&last.trim_matches(|c: char| !c.is_ascii_alphabetic()))
    {
        flags.push("truncated_tail_review");
    }
    flags
}

fn stable_dev_digest(records: &[Value]) -> String {
    let material = records
        .iter()
        .map(|record| {
            format!(
                "{}|{}|{}|{}|{}|{}|{}|{}|{}|{}",
                string_field(record, "record_id"),
                string_field(record, "source_path"),
                string_field(record, "source_family"),
                string_field(record, "source_sha256"),
                usize_field(record, "source_line"),
                string_field(record, "prompt_sha256"),
                string_field(record, "split"),
                string_field(record, "assembly_status"),
                string_field(record, "answer_key_status"),
                string_field(record, "prompt"),
            )
        })
        .collect::<Vec<_>>()
        .join("\n");
    digest_bytes(material.as_bytes())
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
                usize_field(record, "source_line"),
                string_field(record, "prompt_sha256"),
                string_field(record, "split"),
                string_field(record, "assembly_status"),
                string_field(record, "answer_key_status"),
            )
        })
        .collect::<Vec<_>>()
        .join("\n");
    digest_bytes(material.as_bytes())
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let source: Value = serde_json::from_str(&fs::read_to_string(SOURCE_MANIFEST)?)?;
    let report: Value = serde_json::from_str(&fs::read_to_string(REPORT)?)?;
    let dev: Value = serde_json::from_str(&fs::read_to_string(DEV)?)?;
    let sealed: Value = serde_json::from_str(&fs::read_to_string(SEALED)?)?;
    let dev_records = dev.as_array().expect("development records array");
    let sealed_records = sealed.as_array().expect("sealed records array");
    let source_files = source["files"].as_array().expect("source files array");
    let source_by_path = source_files
        .iter()
        .map(|record| (string_field(record, "path"), record))
        .collect::<BTreeMap<_, _>>();

    let report_counts_ok = dev_records.len() == usize_field(&report, "development_count")
        && sealed_records.len() == usize_field(&report, "sealed_holdout_count");
    let source_manifest_ok =
        string_field(&report, "source_manifest_sha256") == string_field(&source, "manifest_sha256");
    let dev_splits_ok = dev_records
        .iter()
        .all(|record| matches!(string_field(record, "split"), "development" | "validation"));
    let sealed_splits_ok = sealed_records
        .iter()
        .all(|record| string_field(record, "split") == "sealed_holdout");
    let sealed_prompt_free = sealed_records.iter().all(|record| {
        record
            .as_object()
            .is_some_and(|object| !object.contains_key("prompt"))
    });
    let dev_prompt_hashes_ok = dev_records.iter().all(|record| {
        digest_bytes(string_field(record, "prompt").as_bytes())
            == string_field(record, "prompt_sha256")
    });
    let dev_answer_free = dev_records
        .iter()
        .all(|record| !answer_like(string_field(record, "prompt")));
    let source_refs_ok = dev_records
        .iter()
        .chain(sealed_records.iter())
        .all(|record| {
            let path = string_field(record, "source_path");
            source_by_path.get(path).is_some_and(|source_record| {
                string_field(source_record, "sha256") == string_field(record, "source_sha256")
            })
        });

    let mut ids = BTreeSet::new();
    let dev_ids_unique = dev_records
        .iter()
        .all(|record| ids.insert(string_field(record, "record_id")));
    let sealed_ids_disjoint = sealed_records
        .iter()
        .all(|record| !ids.contains(string_field(record, "record_id")));
    let mut dev_prompt_hashes = BTreeSet::new();
    let duplicate_prompt_hashes = dev_records
        .iter()
        .filter(|record| !dev_prompt_hashes.insert(string_field(record, "prompt_sha256")))
        .count();

    let mut quality_counts: BTreeMap<&str, usize> = BTreeMap::new();
    let mut clean_candidates = 0usize;
    let mut question_candidates = 0usize;
    let mut replay_material = Vec::new();
    for record in dev_records {
        let prompt = string_field(record, "prompt");
        if question_like(prompt) {
            question_candidates += 1;
        }
        let flags = quality_flags(prompt);
        if flags.is_empty() {
            clean_candidates += 1;
        }
        for flag in flags {
            *quality_counts.entry(flag).or_default() += 1;
        }
        replay_material.push(format!(
            "{}|{}|{}",
            string_field(record, "record_id"),
            string_field(record, "prompt_sha256"),
            quality_flags(prompt).join(",")
        ));
    }
    let quality_digest = digest_bytes(replay_material.join("\n").as_bytes());
    let dev_digest = stable_dev_digest(dev_records);
    let sealed_digest = stable_sealed_digest(sealed_records);
    let replay_verified = digest_bytes(replay_material.join("\n").as_bytes()) == quality_digest;
    let checks = [
        report_counts_ok,
        source_manifest_ok,
        dev_splits_ok,
        sealed_splits_ok,
        sealed_prompt_free,
        dev_prompt_hashes_ok,
        dev_answer_free,
        source_refs_ok,
        dev_ids_unique,
        sealed_ids_disjoint,
        replay_verified,
    ];
    let integrity_passed = bool_count(&checks) == checks.len();
    let baseline_ready = false;
    let report_out = serde_json::json!({
        "schema": "stage384-external-problem-quality-audit-v1",
        "assembly_report_sha256": digest_bytes(fs::read(REPORT)?.as_slice()),
        "source_manifest_sha256": string_field(&source, "manifest_sha256"),
        "development_count": dev_records.len(),
        "sealed_holdout_count": sealed_records.len(),
        "question_candidates": question_candidates,
        "clean_candidates": clean_candidates,
        "quality_flag_counts": quality_counts,
        "duplicate_prompt_hashes": duplicate_prompt_hashes,
        "sealed_prompt_text_stored": false,
        "answer_key_policy": "answer keys are never read, parsed, inferred, or stored",
        "development_digest": dev_digest,
        "sealed_metadata_digest": sealed_digest,
        "quality_replay_digest": quality_digest,
        "replay_verified": replay_verified,
        "checks_passed": bool_count(&checks),
        "checks_total": checks.len(),
        "integrity_passed": integrity_passed,
        "baseline_ready": baseline_ready,
        "baseline_readiness_reason": "candidate quality is measured, but exact problem assembly and independent answer-key alignment remain incomplete",
        "false_authorizations": 0,
        "production_mutations": 0,
    });
    assert!(
        integrity_passed,
        "stage384 quality audit failed: {checks:?}"
    );
    fs::write(
        JSON,
        format!("{}\n", serde_json::to_string_pretty(&report_out)?),
    )?;
    fs::write(
        MD,
        format!(
            "# Stage 384 — external problem quality audit\n\n- integrity checks: {}/{}\n- development / sealed candidates: {} / {}\n- question-like / clean development candidates: {} / {}\n- quality flags: `{}`\n- duplicate prompt hashes: {}\n- sealed prompt text stored: `false`\n- replay verified: `{}`\n- baseline ready: `false`\n\nThis independent audit consumes only Stage 383 records. It measures likely multi-problem merges, layout/context leakage, truncation indicators, and short candidates without re-extracting PDFs or reading answer keys. Sealed prompts remain hash-only. The benchmark is not baseline-ready until exact problem assembly and separately governed answer-key alignment are complete.\n\nReproduce with `cargo run --quiet --bin stage384_external_problem_quality_audit`.\nMachine-readable report: `{}`\n",
            bool_count(&checks),
            checks.len(),
            dev_records.len(),
            sealed_records.len(),
            question_candidates,
            clean_candidates,
            serde_json::to_string(&quality_counts)?,
            duplicate_prompt_hashes,
            replay_verified,
            JSON,
        ),
    )?;
    println!(
        "stage384 integrity={}/{} question_like={} clean={} sealed={} baseline_ready=false",
        bool_count(&checks),
        checks.len(),
        question_candidates,
        clean_candidates,
        sealed_records.len(),
    );
    Ok(())
}
