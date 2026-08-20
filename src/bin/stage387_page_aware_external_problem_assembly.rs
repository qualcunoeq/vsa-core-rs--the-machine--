//! Stage 387: assemble external candidates with page-bounded provenance.
//!
//! The flattened PDF extractor used by Stage 383 can interleave columns and
//! page transitions. This stage consumes the same hash-pinned seed sources
//! through page-bounded extraction. The split is derived from source/page/line
//! location, not prompt content, so sealed candidates are never classified by
//! development heuristics and are emitted as hashes only.

use serde_json::Value;
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::env;
use std::fs;
use std::path::{Path, PathBuf};

const SOURCE_MANIFEST: &str = "docs/stage380_external_curriculum_source_manifest.json";
const EXTRACT: &str = "docs/stage381_external_curriculum_corpus_extract.json";
const JSON: &str = "docs/stage387_page_aware_external_problem_assembly.json";
const DEV_JSON: &str = "docs/stage387_page_aware_external_problem_dev.json";
const SEALED_JSON: &str = "docs/holdouts/stage387_page_aware_external_problem_sealed_manifest.json";
const MD: &str = "docs/stage387_page_aware_external_problem_assembly.md";

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
    source_page: usize,
    source_line: usize,
    prompt: String,
    prompt_sha256: String,
    split: &'static str,
    assembly_status: &'static str,
    answer_key_status: &'static str,
    quality_flags: Vec<&'static str>,
}

#[derive(serde::Serialize)]
struct SealedRecord {
    record_id: String,
    source_path: String,
    source_family: String,
    source_sha256: String,
    source_page: usize,
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

fn marker_spans(line: &str) -> Vec<(usize, usize)> {
    let bytes = line.as_bytes();
    let mut result = Vec::new();
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index].is_ascii_digit() && (index == 0 || bytes[index - 1].is_ascii_whitespace()) {
            let start = index;
            while index < bytes.len() && bytes[index].is_ascii_digit() {
                index += 1;
            }
            let number = line[start..index].parse::<u32>().unwrap_or(u32::MAX);
            let mut punctuation = index;
            while punctuation < bytes.len() && bytes[punctuation].is_ascii_whitespace() {
                punctuation += 1;
            }
            if number <= 999
                && punctuation < bytes.len()
                && (bytes[punctuation] == b'.' || bytes[punctuation] == b')')
            {
                let after = punctuation + 1;
                if !(bytes[punctuation] == b'.'
                    && after < bytes.len()
                    && bytes[after].is_ascii_digit())
                    && (after == bytes.len() || bytes[after].is_ascii_whitespace())
                {
                    result.push((start, after));
                }
            }
        }
        index += 1;
    }
    result
}

fn numbered(line: &str) -> bool {
    let trimmed = line.trim_start();
    let markers = marker_spans(trimmed);
    !markers.is_empty()
        && trimmed.len() >= 20
        && !trimmed.contains("Exercises")
        && !trimmed.contains('•')
}

fn inline_numbered_parts(line: &str) -> Option<(String, Vec<String>)> {
    let markers = marker_spans(line);
    if markers.len() < 2 {
        return None;
    }
    let prefix = line[..markers[0].0].trim().to_string();
    let parts = markers
        .iter()
        .enumerate()
        .map(|(position, (start, _))| {
            let end = markers
                .get(position + 1)
                .map(|(next, _)| *next)
                .unwrap_or(line.len());
            line[*start..end].trim().to_string()
        })
        .filter(|part| part.len() >= 12)
        .collect::<Vec<_>>();
    (parts.len() >= 2).then_some((prefix, parts))
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
            && !lower.starts_with("yes,")
            && !lower.starts_with("no,")
            && !lower.starts_with("the answer")
            && !lower.contains("because it contains")
}

fn split_for_location(source_sha: &str, page: usize, line: usize) -> &'static str {
    let hash = digest_bytes(format!("{source_sha}:{page}:{line}").as_bytes());
    match u8::from_str_radix(&hash[..2], 16).expect("location hash") % 10 {
        0..=6 => "development",
        7..=8 => "validation",
        _ => "sealed_holdout",
    }
}

fn quality_flags(prompt: &str) -> Vec<&'static str> {
    let lower = prompt.to_lowercase();
    let mut flags = Vec::new();
    if marker_spans(prompt).len() > 1 {
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

fn source_map() -> Result<BTreeMap<String, SourceInfo>, Box<dyn std::error::Error>> {
    let manifest: Value = serde_json::from_str(&fs::read_to_string(SOURCE_MANIFEST)?)?;
    let files = manifest["files"].as_array().expect("source files");
    Ok(files
        .iter()
        .map(|file| {
            (
                file["path"].as_str().unwrap().to_string(),
                SourceInfo {
                    family: file["family"].as_str().unwrap().to_string(),
                    sha256: file["sha256"].as_str().unwrap().to_string(),
                },
            )
        })
        .collect())
}

fn page_cache_path(cache_dir: &Path, source_sha: &str) -> PathBuf {
    cache_dir.join(format!("{source_sha}.json"))
}

fn extract_pages_cached(
    path: &str,
    source_sha: &str,
    cache_dir: &Path,
) -> Result<(Vec<String>, bool), Box<dyn std::error::Error>> {
    let cache_path = page_cache_path(cache_dir, source_sha);
    if let Ok(bytes) = fs::read(&cache_path) {
        if let Ok(cached) = serde_json::from_slice::<Vec<String>>(&bytes) {
            if !cached.is_empty() && cached.iter().any(|page| !page.trim().is_empty()) {
                return Ok((cached, true));
            }
        }
    }
    let pages = the_machine::pdf_reader::extract_pages(path)
        .map_err(|error| format!("{path}: {error}"))?;
    fs::create_dir_all(cache_dir)?;
    let temporary = cache_path.with_extension("json.tmp");
    fs::write(&temporary, serde_json::to_vec(&pages)?)?;
    fs::rename(temporary, cache_path)?;
    Ok((pages, false))
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let extract: Value = serde_json::from_str(&fs::read_to_string(EXTRACT)?)?;
    let paths = extract["seed_source_paths"].as_array().expect("seed paths");
    let source_offset = env::var("STAGE387_SOURCE_OFFSET")
        .ok()
        .and_then(|value| value.parse::<usize>().ok())
        .unwrap_or(0)
        .min(paths.len());
    let source_limit = env::var("STAGE387_SOURCE_LIMIT")
        .ok()
        .and_then(|value| value.parse::<usize>().ok())
        .unwrap_or(paths.len())
        .min(paths.len().saturating_sub(source_offset));
    let cache_dir = env::var("STAGE387_PAGE_CACHE_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("/tmp/the-machine-stage387-page-cache"));
    let sources = source_map()?;
    let mut development = Vec::new();
    let mut sealed = Vec::new();
    let mut extracted_files = 0usize;
    let mut page_count = 0usize;
    let mut answer_like_rejected = 0usize;
    let mut nonquestion_rejected = 0usize;
    let mut split_candidates = 0usize;
    let mut cache_hits = 0usize;
    let mut cache_misses = 0usize;

    for path_value in paths.iter().skip(source_offset).take(source_limit) {
        let path = path_value.as_str().unwrap();
        let source = sources.get(path).expect("seed source in manifest");
        let bytes = fs::read(path)?;
        assert_eq!(digest_bytes(&bytes), source.sha256, "source drift detected");
        let (pages, cache_hit) = match extract_pages_cached(path, &source.sha256, &cache_dir) {
            Ok(result) => result,
            Err(_) => continue,
        };
        if cache_hit {
            cache_hits += 1;
        } else {
            cache_misses += 1;
        }
        extracted_files += 1;
        page_count += pages.len();
        for (page_index, page_text) in pages.iter().enumerate() {
            let page = page_index + 1;
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
                if prompt.len() < 30 {
                    return;
                }
                let prompt_sha256 = digest_bytes(prompt.as_bytes());
                let split = split_for_location(&source.sha256, page, line);
                let record_id = format!(
                    "{}:{}:{}:{}",
                    source.sha256,
                    page,
                    line,
                    &prompt_sha256[..16]
                );
                if split == "sealed_holdout" {
                    sealed.push(SealedRecord {
                        record_id,
                        source_path: path.to_string(),
                        source_family: source.family.clone(),
                        source_sha256: source.sha256.clone(),
                        source_page: page,
                        source_line: line,
                        prompt_sha256,
                        split,
                        assembly_status: "page_candidate_needs_alignment",
                        answer_key_status: "not_read",
                    });
                    return;
                }
                if prompt.to_lowercase().contains("answer")
                    || prompt.to_lowercase().contains("solution")
                {
                    answer_like_rejected += 1;
                    return;
                }
                if !question_like(&prompt) {
                    nonquestion_rejected += 1;
                    return;
                }
                let flags = quality_flags(&prompt);
                if flags.contains(&"multiple_numbered_items") {
                    split_candidates += 1;
                }
                development.push(ProblemRecord {
                    record_id,
                    source_path: path.to_string(),
                    source_family: source.family.clone(),
                    source_sha256: source.sha256.clone(),
                    source_page: page,
                    source_line: line,
                    prompt,
                    prompt_sha256,
                    split,
                    assembly_status: "page_candidate_needs_alignment",
                    answer_key_status: "not_read",
                    quality_flags: flags,
                });
            };

            for (line_number, raw_line) in page_text.lines().enumerate() {
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
                        if !prefix.is_empty() && !prefix.contains('•') {
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
    }

    development.sort_by(|left, right| left.record_id.cmp(&right.record_id));
    sealed.sort_by(|left, right| left.record_id.cmp(&right.record_id));
    let mut seen = BTreeSet::new();
    let duplicate_prompt_hashes = development
        .iter()
        .filter(|record| !seen.insert(record.prompt_sha256.clone()))
        .count();
    let development_digest = digest_bytes(serde_json::to_vec(&development)?.as_slice());
    let sealed_digest = digest_bytes(serde_json::to_vec(&sealed)?.as_slice());
    let corpus_sha256 = digest_bytes(format!("{development_digest}|{sealed_digest}").as_bytes());
    let report = serde_json::json!({
        "schema": "stage387-page-aware-external-problem-assembly-v1",
        "source_manifest_sha256": extract["source_manifest_sha256"],
        "source_files": source_limit,
        "source_offset": source_offset,
        "source_limit": source_limit,
        "run_mode": if source_offset == 0 && source_limit == paths.len() {
            "full_seed"
        } else {
            "controlled_subset"
        },
        "page_cache_dir": cache_dir,
        "page_cache_hits": cache_hits,
        "page_cache_misses": cache_misses,
        "extracted_files": extracted_files,
        "pages": page_count,
        "answer_like_rejected": answer_like_rejected,
        "nonquestion_rejected": nonquestion_rejected,
        "development_count": development.len(),
        "sealed_holdout_count": sealed.len(),
        "split_candidates": split_candidates,
        "duplicate_prompt_hashes": duplicate_prompt_hashes,
        "sealed_prompt_text_stored": false,
        "answer_key_policy": "answer keys are never read, parsed, inferred, or stored",
        "baseline_ready": false,
        "baseline_readiness_reason": "page-aware candidates still require exact quality review and independent answer-key alignment",
        "development_digest": development_digest,
        "sealed_metadata_digest": sealed_digest,
        "corpus_sha256": corpus_sha256,
    });
    fs::create_dir_all("docs/holdouts")?;
    fs::write(
        DEV_JSON,
        format!("{}\n", serde_json::to_string_pretty(&development)?),
    )?;
    fs::write(
        SEALED_JSON,
        format!("{}\n", serde_json::to_string_pretty(&sealed)?),
    )?;
    fs::write(
        JSON,
        format!("{}\n", serde_json::to_string_pretty(&report)?),
    )?;
    fs::write(
        MD,
        format!(
            "# Stage 387 — page-aware external problem assembly\n\n- run mode: `{}`\n- source offset / limit / extracted / pages: {} / {} / {} / {}\n- page-cache hits / misses: {} / {}\n- development / sealed candidates: {} / {}\n- answer-like / nonquestion rejects: {} / {}\n- candidates retaining multiple markers: {}\n- duplicate prompt hashes: {}\n- sealed prompt text stored: `false`\n- baseline ready: `false`\n- corpus SHA-256: `{}`\n\nThis stage consumes the same hash-pinned seed sources through page-bounded extraction. Partitioning is derived from source/page/line provenance rather than prompt content. Sealed candidates are recorded as metadata and hashes only; development heuristics never classify sealed prompts. The corpus remains a candidate set until exact quality review and independently governed answer alignment are complete.\n\nReproduce with `cargo run --quiet --bin stage387_page_aware_external_problem_assembly`. For resumable extraction, set `STAGE387_SOURCE_OFFSET`, `STAGE387_SOURCE_LIMIT`, and `STAGE387_PAGE_CACHE_DIR`.\nMachine-readable report: `{}`\n",
            if source_offset == 0 && source_limit == paths.len() { "full_seed" } else { "controlled_subset" },
            source_offset,
            source_limit,
            extracted_files,
            page_count,
            cache_hits,
            cache_misses,
            development.len(),
            sealed.len(),
            answer_like_rejected,
            nonquestion_rejected,
            split_candidates,
            duplicate_prompt_hashes,
            corpus_sha256,
            JSON,
        ),
    )?;
    println!(
        "stage387 mode={} offset={} sources={} extracted={} pages={} cache_hits={} cache_misses={} development={} sealed={} duplicates={} baseline_ready=false corpus_sha256={}",
        if source_offset == 0 && source_limit == paths.len() { "full_seed" } else { "controlled_subset" },
        source_offset,
        source_limit,
        extracted_files,
        page_count,
        cache_hits,
        cache_misses,
        development.len(),
        sealed.len(),
        duplicate_prompt_hashes,
        corpus_sha256,
    );
    Ok(())
}
