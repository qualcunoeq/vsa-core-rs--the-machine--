//! Stage 383: assemble complete exercise blocks from the external source seed.
//!
//! Stage 381 stores line-level candidates. This stage re-reads only the
//! hash-pinned source files and joins instruction context, numbered prompts,
//! and continuation lines. It remains answer-key blind. Sealed records retain
//! hashes and provenance only; the stage is not baseline-ready until answer
//! alignment is supplied from a separately governed source.

use serde_json::Value;
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::fs;

const SOURCE_MANIFEST: &str = "docs/stage380_external_curriculum_source_manifest.json";
const EXTRACT: &str = "docs/stage381_external_curriculum_corpus_extract.json";
const JSON: &str = "docs/stage383_external_problem_assembly.json";
const DEV_JSON: &str = "docs/stage383_external_problem_dev.json";
const SEALED_JSON: &str = "docs/holdouts/stage383_external_problem_sealed_manifest.json";
const MD: &str = "docs/stage383_external_problem_assembly.md";

#[derive(Clone)]
struct SourceInfo {
    family: String,
    sha256: String,
}

#[derive(serde::Serialize)]
struct ProblemRecord {
    record_id: String,
    source_path: String,
    source_family: String,
    source_sha256: String,
    source_line: usize,
    prompt: String,
    prompt_sha256: String,
    split: &'static str,
    assembly_status: &'static str,
    answer_key_status: &'static str,
}

#[derive(serde::Serialize)]
struct SealedRecord {
    record_id: String,
    source_path: String,
    source_family: String,
    source_sha256: String,
    source_line: usize,
    prompt_sha256: String,
    split: &'static str,
    assembly_status: &'static str,
    answer_key_status: &'static str,
}

fn digest_bytes(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

fn clean(line: &str) -> String {
    line.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn solution_line(lower: &str) -> bool {
    let line = lower.trim();
    line.contains("answer")
        || line.contains("solution")
        || line.starts_with("if you missed this problem")
}

fn solution_heading(lower: &str) -> bool {
    matches!(
        lower.trim(),
        "answers" | "solutions" | "answer key" | "solutions manual"
    )
}

fn instruction(lower: &str) -> bool {
    [
        "in the following exercises",
        "for the following exercises",
        "in each of the following exercises",
        "in the following problems",
        "practice makes perfect",
        "writing exercises",
        "section exercises",
        "review exercises",
        "practice test",
    ]
    .iter()
    .any(|prefix| lower.trim_start().starts_with(prefix))
}

fn exercise_heading(lower: &str) -> bool {
    let line = lower.trim();
    line == "exercises"
        || line == "practice makes perfect"
        || (line.starts_with("section ") && line.contains("exercises"))
        || line.starts_with("chapter review")
        || line.starts_with("review exercises")
        || line.starts_with("practice test")
}

fn section_boundary(lower: &str) -> bool {
    let line = lower.trim();
    (line.starts_with("section ") && !line.contains("exercises"))
        || (line.starts_with("chapter ") && !line.contains("review"))
        || line.starts_with("appendix ")
        || line.starts_with("table of contents")
        || line.starts_with("front matter")
        || line.starts_with("back matter")
}

fn probable_heading(line: &str) -> bool {
    let line = line.trim();
    line.len() >= 3
        && line.len() <= 100
        && !line.contains(['.', ',', '?', '!', ':', ';'])
        && line.chars().next().is_some_and(char::is_uppercase)
        && line.split_whitespace().count() <= 12
}

fn numbered(line: &str) -> bool {
    let trimmed = line.trim_start();
    let digits = trimmed.chars().take_while(|c| c.is_ascii_digit()).count();
    if digits == 0 {
        return false;
    }
    let rest = &trimmed[digits..];
    if !(rest.starts_with('.') || rest.starts_with(')')) {
        return false;
    }
    if rest.starts_with('.') && rest.as_bytes().get(1).is_some_and(u8::is_ascii_digit) {
        return false;
    }
    line.len() >= 20 && !line.contains("Exercises") && !line.contains('•')
}

fn inline_numbered_parts(line: &str) -> Option<(String, Vec<String>)> {
    let bytes = line.as_bytes();
    let mut starts = Vec::new();
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
                if marker == start + 1
                    && bytes[marker] == b'.'
                    && index < bytes.len()
                    && bytes[index].is_ascii_digit()
                {
                    continue;
                }
                if index < bytes.len() && bytes[index].is_ascii_whitespace() {
                    starts.push(start);
                }
            }
        }
        index += 1;
    }
    if starts.len() < 2 {
        return None;
    }
    let prefix = line[..starts[0]].trim().to_string();
    let parts = starts
        .iter()
        .enumerate()
        .map(|(position, start)| {
            let end = starts.get(position + 1).copied().unwrap_or(line.len());
            line[*start..end].trim().to_string()
        })
        .filter(|part| part.len() >= 12)
        .collect::<Vec<_>>();
    if parts.len() < 2 {
        None
    } else {
        Some((prefix, parts))
    }
}

fn question_like(prompt: &str) -> bool {
    let lower = prompt.to_lowercase();
    if lower.contains('?') {
        return true;
    }
    [
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
        && !lower.starts_with("yes,")
        && !lower.starts_with("no,")
        && !lower.starts_with("the answer")
        && !lower.contains(" yes, ")
        && !lower.contains(" no, ")
        && !lower.contains("because it contains")
}

fn split_for_hash(hash: &str) -> &'static str {
    let prefix = u8::from_str_radix(&hash[..2], 16).expect("hash prefix is hex");
    match prefix % 10 {
        0..=6 => "development",
        7..=8 => "validation",
        _ => "sealed_holdout",
    }
}

fn source_map() -> Result<BTreeMap<String, SourceInfo>, Box<dyn std::error::Error>> {
    let manifest: Value = serde_json::from_str(&fs::read_to_string(SOURCE_MANIFEST)?)?;
    let files = manifest["files"].as_array().expect("source files");
    let mut result = BTreeMap::new();
    for file in files {
        result.insert(
            file["path"].as_str().unwrap().to_string(),
            SourceInfo {
                family: file["family"].as_str().unwrap().to_string(),
                sha256: file["sha256"].as_str().unwrap().to_string(),
            },
        );
    }
    Ok(result)
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let extract: Value = serde_json::from_str(&fs::read_to_string(EXTRACT)?)?;
    let paths = extract["seed_source_paths"].as_array().expect("seed paths");
    let sources = source_map()?;
    let mut development = Vec::new();
    let mut sealed = Vec::new();
    let mut extracted_files = 0;
    let mut answer_like_rejected = 0;
    let mut nonquestion_rejected = 0;
    for path_value in paths {
        let path = path_value.as_str().unwrap();
        let source = sources.get(path).expect("seed source in manifest");
        let bytes = fs::read(path)?;
        assert_eq!(digest_bytes(&bytes), source.sha256, "source drift detected");
        let text = match the_machine::pdf_reader::extract_text(path) {
            Ok(text) => text,
            Err(_) => continue,
        };
        extracted_files += 1;
        let mut solution_mode = false;
        let mut exercise_mode = false;
        let mut context = String::new();
        let mut pending: Option<(usize, String, Vec<String>)> = None;
        let mut flush = |pending: &mut Option<(usize, String, Vec<String>)>, context: &str| {
            let Some((line, first, continuation)) = pending.take() else {
                return;
            };
            let mut pieces = Vec::new();
            if !context.is_empty() {
                pieces.push(context.to_string());
            }
            pieces.push(first);
            pieces.extend(continuation);
            let prompt = pieces.join(" ");
            if prompt.len() < 30
                || prompt.to_lowercase().contains("answer")
                || prompt.to_lowercase().contains("solution")
            {
                return;
            }
            let prompt_sha256 = digest_bytes(prompt.as_bytes());
            let split = split_for_hash(&prompt_sha256);
            let status = if question_like(&prompt) {
                "question_candidate_needs_alignment"
            } else {
                nonquestion_rejected += 1;
                "nonquestion_fragment"
            };
            if status == "nonquestion_fragment" {
                return;
            }
            let record_id = format!("{}:{}:{}", source.sha256, line, &prompt_sha256[..16]);
            let record = ProblemRecord {
                record_id,
                source_path: path.to_string(),
                source_family: source.family.clone(),
                source_sha256: source.sha256.clone(),
                source_line: line,
                prompt,
                prompt_sha256,
                split,
                assembly_status: status,
                answer_key_status: "not_read",
            };
            if split == "sealed_holdout" {
                sealed.push(SealedRecord {
                    record_id: record.record_id,
                    source_path: record.source_path,
                    source_family: record.source_family,
                    source_sha256: record.source_sha256,
                    source_line: record.source_line,
                    prompt_sha256: record.prompt_sha256,
                    split,
                    assembly_status: status,
                    answer_key_status: record.answer_key_status,
                });
            } else {
                development.push(record);
            }
        };
        for (line_number, raw_line) in text.lines().enumerate() {
            let line = clean(raw_line);
            if line.is_empty() || line.len() > 1400 {
                continue;
            }
            let lower = line.to_lowercase();
            if solution_heading(&lower) {
                flush(&mut pending, &context);
                solution_mode = true;
                continue;
            }
            if solution_line(&lower) {
                answer_like_rejected += 1;
                continue;
            }
            if solution_mode {
                continue;
            }
            if exercise_heading(&lower) || instruction(&lower) {
                flush(&mut pending, &context);
                exercise_mode = true;
                if let Some((prefix, parts)) = inline_numbered_parts(&line) {
                    context = prefix;
                    for part in parts {
                        flush(&mut pending, &context);
                        pending = Some((line_number + 1, part, Vec::new()));
                    }
                } else {
                    context = line;
                }
                continue;
            }
            if section_boundary(&lower) || probable_heading(&line) {
                flush(&mut pending, &context);
                exercise_mode = false;
                context.clear();
                continue;
            }
            if exercise_mode {
                if let Some((prefix, parts)) = inline_numbered_parts(&line) {
                    if !prefix.is_empty() && !prefix.contains("•") {
                        context = prefix;
                    }
                    for part in parts {
                        flush(&mut pending, &context);
                        pending = Some((line_number + 1, part, Vec::new()));
                    }
                    continue;
                }
            }
            if exercise_mode && numbered(&line) {
                flush(&mut pending, &context);
                pending = Some((line_number + 1, line, Vec::new()));
            } else if pending.is_some() && !line.contains('•') {
                pending.as_mut().unwrap().2.push(line);
            }
        }
        flush(&mut pending, &context);
    }
    development.sort_by(|a, b| a.record_id.cmp(&b.record_id));
    sealed.sort_by(|a, b| a.record_id.cmp(&b.record_id));
    let complete = development
        .iter()
        .filter(|record| record.assembly_status == "question_candidate_needs_alignment")
        .count();
    let fragmented = development.len() - complete;
    let sealed_complete = sealed
        .iter()
        .filter(|record| record.assembly_status == "question_candidate_needs_alignment")
        .count();
    let sealed_manifest_hash = digest_bytes(&serde_json::to_vec(&sealed)?);
    let corpus_hash = digest_bytes(serde_json::to_vec(&(&development, &sealed))?.as_slice());
    fs::create_dir_all("docs/holdouts")?;
    fs::write(
        DEV_JSON,
        format!("{}\n", serde_json::to_string_pretty(&development)?),
    )?;
    fs::write(
        SEALED_JSON,
        format!("{}\n", serde_json::to_string_pretty(&sealed)?),
    )?;
    let report = serde_json::json!({
        "schema": "stage383-external-problem-assembly-v1",
        "source_manifest_sha256": extract["source_manifest_sha256"],
        "source_files": paths.len(),
        "extracted_files": extracted_files,
        "answer_like_rejected": answer_like_rejected,
        "nonquestion_rejected": nonquestion_rejected,
        "development_count": development.len(),
        "sealed_holdout_count": sealed.len(),
        "question_candidates_development": complete,
        "nonquestion_fragments_development": fragmented,
        "question_candidates_sealed": sealed_complete,
        "sealed_prompt_text_stored": false,
        "answer_key_policy": "answer keys are never read, parsed, inferred, or stored",
        "baseline_ready": false,
        "baseline_readiness_reason": "candidates still require exact multi-line quality review and answer-key alignment",
        "sealed_manifest_sha256": sealed_manifest_hash,
        "corpus_sha256": corpus_hash,
    });
    fs::write(
        JSON,
        format!("{}\n", serde_json::to_string_pretty(&report)?),
    )?;
    fs::write(
        MD,
        format!(
            "# Stage 383 — external problem assembly\n\n- source files / extracted: {} / {}\n- question candidates with context: {} (development {}, non-question fragments {})\n- sealed holdout candidates: {} (question candidates {})\n- answer-like lines rejected: {}\n- non-question fragments rejected: {}\n- sealed prompt text stored: `false`\n- baseline ready: `false`\n- reason: candidates still require exact multi-line quality review and answer-key alignment\n- corpus SHA-256: `{}`\n\nThe assembler joins source exercise context and continuations without reading solutions. It rejects declarative/non-question fragments and preserves a hash-only sealed manifest. These are question candidates, not authorized benchmark problems; baseline scoring remains blocked until exact problem assembly and answer-key alignment are supplied through independent governed inputs.\n\nReproduce with `cargo run --quiet --bin stage383_external_problem_assembly`.\nMachine-readable report: `{}`\n",
            paths.len(), extracted_files, development.len(), complete, fragmented, sealed.len(), sealed_complete, answer_like_rejected, nonquestion_rejected, corpus_hash, JSON
        ),
    )?;
    println!(
        "stage383 development={} complete={} fragments={} sealed={} baseline_ready=false corpus_sha256={}",
        development.len(), complete, fragmented, sealed.len(), corpus_hash
    );
    Ok(())
}
