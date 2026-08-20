//! Stage 381: extract a naturally authored, answer-key-excluded corpus seed.
//!
//! Stage 380 freezes all source files. This stage deliberately consumes a
//! small, explicit OpenStax seed from that manifest so extraction remains
//! reproducible while the PDF reader is hardened. It stores original
//! question-like lines, never answer keys or solutions. This is a corpus
//! boundary, not a solved benchmark.

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::BTreeSet;
use std::fs;

const SOURCE_MANIFEST: &str = "docs/stage380_external_curriculum_source_manifest.json";
const JSON: &str = "docs/stage381_external_curriculum_corpus_extract.json";
const MD: &str = "docs/stage381_external_curriculum_corpus_extract.md";
const DEV_JSON: &str = "docs/stage381_external_curriculum_dev.json";
const SEALED_JSON: &str = "docs/holdouts/stage381_external_curriculum_sealed_manifest.json";
const SEED_SOURCES: &[&str] = &[
    "data/openstax_pdfs/prealgebra-2e_-_WEB.pdf",
    "data/openstax_pdfs/elementary-algebra-2e_-_WEB.pdf",
    "data/openstax_pdfs/college-algebra-2e_-_WEB.pdf",
    "data/openstax_pdfs/algebra-and-trigonometry-2e_-_WEB.pdf",
    "data/openstax_pdfs/calculus-volume-1_-_WEB.pdf",
    "data/openstax_pdfs/calculus-volume-2_-_WEB.pdf",
    "data/openstax_pdfs/calculus-volume-3_-_WEB.pdf",
    "data/openstax_pdfs/contemporary-mathematics_-_WEB.pdf",
    "data/openstax_pdfs/precalculus-2e_-_WEB.pdf",
    "data/openstax_pdfs/intermediate-algebra-2e_-_WEB.pdf",
    "data/openstax_pdfs/introductory-statistics-2e_-_WEB.pdf",
    "data/openstax_pdfs/introductory-business-statistics-2e_-_WEB.pdf",
];

#[derive(Debug, Deserialize)]
struct SourceManifest {
    schema: String,
    manifest_sha256: String,
    files: Vec<SourceFile>,
}

#[derive(Debug, Deserialize)]
struct SourceFile {
    path: String,
    family: String,
    sha256: String,
}

#[derive(Debug, Serialize)]
struct CorpusRecord {
    record_id: String,
    source_path: String,
    source_family: String,
    source_sha256: String,
    source_line: usize,
    candidate_kind: &'static str,
    prompt: String,
    prompt_sha256: String,
    split: &'static str,
    answer_key_status: &'static str,
}

#[derive(Debug, Serialize)]
struct SealedRecord {
    record_id: String,
    source_path: String,
    source_family: String,
    source_sha256: String,
    source_line: usize,
    candidate_kind: &'static str,
    prompt_sha256: String,
    split: &'static str,
    answer_key_status: &'static str,
}

#[derive(Debug, Serialize)]
struct Report {
    schema: &'static str,
    source_manifest_schema: String,
    source_manifest_sha256: String,
    seed_source_paths: Vec<String>,
    extracted_files: usize,
    failed_files: usize,
    answer_like_rejected: usize,
    structural_nonproblem_rejected: usize,
    development_records: Vec<CorpusRecord>,
    development_count: usize,
    validation_count: usize,
    sealed_holdout_count: usize,
    answer_key_policy: &'static str,
    sealed_holdout_manifest_sha256: String,
    corpus_sha256: String,
}

fn digest_bytes(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

fn stable_record_digest(records: &[SealedRecord]) -> String {
    let material = records
        .iter()
        .map(|record| {
            format!(
                "{}|{}|{}|{}|{}|{}|{}|{}|{}",
                record.record_id,
                record.source_path,
                record.source_family,
                record.source_sha256,
                record.source_line,
                record.candidate_kind,
                record.prompt_sha256,
                record.split,
                record.answer_key_status,
            )
        })
        .collect::<Vec<_>>()
        .join("\n");
    digest_bytes(material.as_bytes())
}

fn clean_line(line: &str) -> String {
    line.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn is_solution_line(lower: &str) -> bool {
    let line = lower.trim();
    line.contains("answer")
        || line.contains("solution")
        || line.starts_with("if you missed this problem")
}

fn is_solution_heading(lower: &str) -> bool {
    let line = lower.trim();
    line == "answers" || line == "solutions" || line == "answer key" || line == "solutions manual"
}

fn is_instruction(lower: &str) -> bool {
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

fn is_exercise_heading(lower: &str) -> bool {
    let line = lower.trim();
    line == "exercises"
        || line == "practice makes perfect"
        || line.starts_with("section ") && line.contains("exercises")
        || line.starts_with("chapter review")
        || line.starts_with("review exercises")
        || line.starts_with("practice test")
}

fn is_section_boundary(lower: &str) -> bool {
    let line = lower.trim();
    (line.starts_with("section ") && !line.contains("exercises"))
        || (line.starts_with("chapter ") && !line.contains("review"))
        || (line.starts_with("appendix "))
        || (line.starts_with("table of contents"))
        || (line.starts_with("front matter"))
        || (line.starts_with("back matter"))
}

fn is_probable_heading(prompt: &str) -> bool {
    let line = prompt.trim();
    if line.len() < 3 || line.len() > 100 || line.contains(['.', ',', '?', '!', ':', ';']) {
        return false;
    }
    line.chars().next().is_some_and(char::is_uppercase) && line.split_whitespace().count() <= 12
}

fn is_numbered_problem(line: &str) -> bool {
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

fn split_for_hash(hash: &str) -> &'static str {
    let prefix = u8::from_str_radix(&hash[..2], 16).expect("SHA prefix is hexadecimal");
    match prefix % 10 {
        0..=6 => "development",
        7..=8 => "validation",
        _ => "sealed_holdout",
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let manifest: SourceManifest = serde_json::from_str(&fs::read_to_string(SOURCE_MANIFEST)?)?;
    assert_eq!(
        manifest.schema,
        "stage380-external-curriculum-source-manifest-v1"
    );
    let by_path = manifest
        .files
        .iter()
        .map(|file| (file.path.as_str(), file))
        .collect::<std::collections::BTreeMap<_, _>>();
    for path in SEED_SOURCES {
        assert!(
            by_path.contains_key(path),
            "seed source is absent from Stage 380"
        );
        let actual_sha256 = digest_bytes(&fs::read(path)?);
        assert_eq!(
            actual_sha256, by_path[path].sha256,
            "source drift detected against the Stage 380 manifest"
        );
    }

    let mut records = Vec::new();
    let mut extracted_files = 0;
    let mut failed_files = 0;
    let mut answer_like_rejected = 0;
    let mut structural_nonproblem_rejected = 0;
    for path in SEED_SOURCES {
        let source = by_path[path];
        let text = match the_machine::pdf_reader::extract_text(path) {
            Ok(text) => text,
            Err(_) => {
                failed_files += 1;
                continue;
            }
        };
        extracted_files += 1;
        let mut solution_section = false;
        let mut exercise_section = false;
        for (line_number, raw_line) in text.lines().enumerate() {
            let prompt = clean_line(raw_line);
            if prompt.is_empty() || prompt.len() < 12 || prompt.len() > 1200 {
                continue;
            }
            let lower = prompt.to_lowercase();
            if is_solution_heading(&lower) {
                solution_section = true;
                continue;
            }
            if is_solution_line(&lower) {
                answer_like_rejected += 1;
                continue;
            }
            // Source exercise sections precede the answer material in this
            // seed. Once a dedicated solution heading appears, never re-enter
            // the answer corpus; this prevents answer leakage across pages.
            if solution_section {
                continue;
            }
            if is_exercise_heading(&lower) || is_instruction(&lower) {
                exercise_section = true;
            } else if is_section_boundary(&lower) {
                exercise_section = false;
            } else if is_probable_heading(&prompt) {
                exercise_section = false;
            }
            let candidate_kind = if is_instruction(&lower) {
                "instruction"
            } else if exercise_section && is_numbered_problem(&prompt) {
                "numbered_problem"
            } else {
                structural_nonproblem_rejected += 1;
                continue;
            };
            let prompt_sha256 = digest_bytes(prompt.as_bytes());
            let split = split_for_hash(&prompt_sha256);
            records.push(CorpusRecord {
                record_id: format!(
                    "{}:{}:{}",
                    source.sha256,
                    line_number + 1,
                    &prompt_sha256[..16]
                ),
                source_path: source.path.clone(),
                source_family: source.family.clone(),
                source_sha256: source.sha256.clone(),
                source_line: line_number + 1,
                candidate_kind,
                prompt,
                prompt_sha256,
                split,
                answer_key_status: "excluded_not_read",
            });
        }
    }
    records.sort_by(|left, right| left.record_id.cmp(&right.record_id));
    let mut seen_prompt_hashes = BTreeSet::new();
    records.retain(|record| seen_prompt_hashes.insert(record.prompt_sha256.clone()));
    let development_count = records.iter().filter(|r| r.split == "development").count();
    let validation_count = records.iter().filter(|r| r.split == "validation").count();
    let sealed_holdout_count = records
        .iter()
        .filter(|r| r.split == "sealed_holdout")
        .count();
    assert!(
        !records.is_empty(),
        "no source exercise candidates extracted"
    );
    assert!(development_count > 0, "no development records extracted");
    assert!(validation_count > 0, "no validation records extracted");
    assert!(
        sealed_holdout_count > 0,
        "no sealed holdout records extracted"
    );
    let sealed_records = records
        .iter()
        .filter(|record| record.split == "sealed_holdout")
        .map(|record| SealedRecord {
            record_id: record.record_id.clone(),
            source_path: record.source_path.clone(),
            source_family: record.source_family.clone(),
            source_sha256: record.source_sha256.clone(),
            source_line: record.source_line,
            candidate_kind: record.candidate_kind,
            prompt_sha256: record.prompt_sha256.clone(),
            split: record.split,
            answer_key_status: record.answer_key_status,
        })
        .collect::<Vec<_>>();
    let development_records = records
        .into_iter()
        .filter(|record| record.split != "sealed_holdout")
        .collect::<Vec<_>>();
    let sealed_holdout_manifest_sha256 = stable_record_digest(&sealed_records);
    let development_digest_material = development_records
        .iter()
        .map(|record| {
            format!(
                "{}|{}|{}|{}|{}|{}|{}|{}|{}|{}",
                record.record_id,
                record.source_path,
                record.source_family,
                record.source_sha256,
                record.source_line,
                record.candidate_kind,
                record.prompt_sha256,
                record.split,
                record.answer_key_status,
                record.prompt,
            )
        })
        .collect::<Vec<_>>()
        .join("\n");
    let corpus_sha256 = digest_bytes(
        format!(
            "dev:{}\nsealed:{}",
            digest_bytes(development_digest_material.as_bytes()),
            sealed_holdout_manifest_sha256
        )
        .as_bytes(),
    );
    let report = Report {
        schema: "stage381-external-curriculum-corpus-extract-v1",
        source_manifest_schema: manifest.schema,
        source_manifest_sha256: manifest.manifest_sha256,
        seed_source_paths: SEED_SOURCES.iter().map(|path| (*path).into()).collect(),
        extracted_files,
        failed_files,
        answer_like_rejected,
        structural_nonproblem_rejected,
        development_records,
        development_count,
        validation_count,
        sealed_holdout_count,
        answer_key_policy: "answer keys are never read, parsed, inferred, or stored",
        sealed_holdout_manifest_sha256,
        corpus_sha256,
    };
    fs::write(
        DEV_JSON,
        format!(
            "{}\n",
            serde_json::to_string_pretty(&report.development_records)?
        ),
    )?;
    fs::create_dir_all("docs/holdouts")?;
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
            "# Stage 381 — external curriculum corpus seed\n\n- seed sources: {}\n- extracted files: {}\n- failed files: {}\n- answer-like lines rejected: {}\n- structural non-problems rejected: {}\n- records with prompts: {} (development {}, validation {}, sealed holdout {})\n- answer-key policy: {}\n- source manifest SHA-256: `{}`\n- sealed holdout manifest SHA-256: `{}`\n- corpus SHA-256: `{}`\n\nThis corpus preserves naturally authored OpenStax exercise instructions and numbered problems from hash-pinned sources. Any line containing answer/solution language and all obvious decimal, table-of-contents, and page-heading artifacts are rejected. Answer keys and solutions are excluded and never read or inferred. Sealed holdout prompts are not present in the development report; only their provenance and prompt hashes are retained in the holdout manifest. This stage establishes a real external corpus boundary; it does not claim that the records are solved or that the source set is broad enough for the final exam.\n\nReproduce with `cargo run --quiet --bin stage381_external_curriculum_corpus_extract`.\nDevelopment records: `{}`\nSealed manifest: `{}`\nMachine-readable report: `{}`\n",
            report.seed_source_paths.len(),
            report.extracted_files,
            report.failed_files,
            report.answer_like_rejected,
            report.structural_nonproblem_rejected,
            report.development_records.len(),
            report.development_count,
            report.validation_count,
            report.sealed_holdout_count,
            report.answer_key_policy,
            report.source_manifest_sha256,
            report.sealed_holdout_manifest_sha256,
            report.corpus_sha256,
            DEV_JSON,
            SEALED_JSON,
            JSON,
        ),
    )?;
    println!(
        "stage381 records={} development={} validation={} sealed={} extracted={} failed={} corpus_sha256={}",
        report.development_records.len() + report.sealed_holdout_count,
        report.development_count,
        report.validation_count,
        report.sealed_holdout_count,
        report.extracted_files,
        report.failed_files,
        report.corpus_sha256,
    );
    Ok(())
}
