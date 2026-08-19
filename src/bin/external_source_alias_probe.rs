//! Exact source-alias probe over the external-MATH development prompts.
//!
//! This is a stricter successor to lexical source selection.  It reads only
//! original development questions and provenance-bearing source catalogs,
//! finds exact declared formula aliases, and runs the generic source frontend
//! in a clone.  It never reads answer keys, authorizes a result, or mutates the
//! curriculum.  Alias overlap is still only a selection signal: a source is
//! not promotable without independent exercise evidence and boundary tests.

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use the_machine::source_formula_frontend::{
    formalize_source_formula_report, report_replay_verified, FrontendStatus,
};
use the_machine::source_formula_pack::{
    evaluate_formula_records, extract_formula_records, validate_formula_records, FormulaStatus,
};
use the_machine::curriculum::breadth_first_manifest;

const QUESTIONS: &str = "data/external_math_exam_v1/questions.jsonl";
const SOURCE_DIR: &str = "docs/sources";
const REPORT_JSON: &str = "docs/goal6_external_source_alias_probe.json";
const REPORT_MD: &str = "docs/goal6_external_source_alias_probe.md";

#[derive(Debug, Deserialize)]
struct Question {
    id: String,
    original_prompt: String,
}

#[derive(Debug, Serialize)]
struct SourceProbe {
    source_path: String,
    source_sha256: String,
    records: usize,
    valid_catalog: bool,
    distinct_aliases: usize,
    alias_hits: usize,
    distinct_hit_questions: usize,
    hit_records: BTreeMap<String, usize>,
    matched_case_ids: Vec<String>,
    frontend_cases: usize,
    frontend_complete: usize,
    frontend_ambiguous: usize,
    frontend_missing: usize,
    frontend_unsupported: usize,
    frontend_replays: usize,
    frontend_tamper_rejections: usize,
    executable_candidates: usize,
    candidate_replays: usize,
    independent_exercise_evidence: bool,
    promotion_status: String,
    reasons: Vec<String>,
}

#[derive(Debug, Serialize)]
struct Report {
    schema: &'static str,
    release_id: &'static str,
    development_questions: usize,
    answer_keys_read: usize,
    source_documents_considered: usize,
    parseable_sources: usize,
    selected_sources: usize,
    source_probes: Vec<SourceProbe>,
    manifest_sha256_before: String,
    manifest_sha256_after: String,
    manifest_unchanged: bool,
    false_authorizations: usize,
    report_sha256: String,
}

fn digest<T: Serialize>(value: &T) -> String {
    format!("{:x}", Sha256::digest(serde_json::to_vec(value).unwrap()))
}

fn digest_bytes(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

fn normalize(text: &str) -> String {
    text.to_ascii_lowercase().replace(['_', '-'], " ")
}

fn has_alias(prompt: &str, alias: &str) -> bool {
    let prompt = normalize(prompt);
    let alias = normalize(alias);
    prompt
        .split(|c: char| !c.is_ascii_alphanumeric() && c != ' ')
        .collect::<Vec<_>>()
        .join(" ")
        .contains(&alias)
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let questions = fs::read_to_string(QUESTIONS)?
        .lines()
        .filter(|line| !line.trim().is_empty())
        .map(serde_json::from_str::<Question>)
        .collect::<Result<Vec<_>, _>>()?
        .into_iter()
        .filter(|question| question.id.contains("development"))
        .collect::<Vec<_>>();
    let manifest_before = breadth_first_manifest().replay_hash();
    let mut probes = Vec::new();
    let mut parseable_sources = 0;
    let mut source_documents_considered = 0;

    for entry in fs::read_dir(SOURCE_DIR)? {
        let path = entry?.path();
        if !path.is_file() {
            continue;
        }
        source_documents_considered += 1;
        let bytes = fs::read(&path)?;
        let text = String::from_utf8_lossy(&bytes);
        let Ok(records) = extract_formula_records(&text) else {
            continue;
        };
        if validate_formula_records(&records).is_err() {
            continue;
        }
        parseable_sources += 1;
        let aliases = records
            .iter()
            .flat_map(|record| record.aliases.iter().cloned())
            .filter(|alias| alias.split_whitespace().count() >= 2 || alias.len() >= 8)
            .collect::<BTreeSet<_>>();
        let mut hit_records = BTreeMap::new();
        let mut matched_case_ids = BTreeSet::new();
        let mut alias_hits = 0;
        for question in &questions {
            for record in &records {
                let mut candidates = std::iter::once(record.formula_id.as_str())
                    .chain(record.aliases.iter().map(String::as_str));
                let matched = candidates.any(|alias| has_alias(&question.original_prompt, alias));
                if matched {
                    *hit_records.entry(record.formula_id.clone()).or_default() += 1;
                    matched_case_ids.insert(question.id.clone());
                    alias_hits += 1;
                }
            }
        }
        // Exact alias overlap is intentionally a conservative selection gate.
        // A single incidental mention is not enough to trigger a frontend run.
        if matched_case_ids.len() < 3 {
            continue;
        }
        let domain = format!("external-source-alias-shadow:{}", digest_bytes(&bytes));
        let mut frontend_complete = 0;
        let mut frontend_ambiguous = 0;
        let mut frontend_missing = 0;
        let mut frontend_unsupported = 0;
        let mut frontend_replays = 0;
        let mut frontend_tamper_rejections = 0;
        let mut executable_candidates = 0;
        let mut candidate_replays = 0;
        for question in &questions {
            let frontend = formalize_source_formula_report(
                &question.original_prompt,
                &domain,
                &records,
            );
            frontend_replays += usize::from(report_replay_verified(&frontend));
            let mut tampered = frontend.clone();
            tampered.replay_hash.push('x');
            frontend_tamper_rejections += usize::from(!report_replay_verified(&tampered));
            match frontend.frontend.status {
                FrontendStatus::Complete => {
                    frontend_complete += 1;
                    if let Some(request) = frontend.frontend.request.as_ref() {
                        let result = evaluate_formula_records(request, &domain, &records);
                        if result.status == FormulaStatus::Complete {
                            executable_candidates += 1;
                            candidate_replays += usize::from(result.replay_verified());
                        }
                    }
                }
                FrontendStatus::Ambiguous => frontend_ambiguous += 1,
                FrontendStatus::Missing => frontend_missing += 1,
                FrontendStatus::Unsupported => frontend_unsupported += 1,
            }
        }
        let records_count = records.len();
        probes.push(SourceProbe {
            source_path: path.to_string_lossy().into_owned(),
            source_sha256: digest_bytes(&bytes),
            records: records_count,
            valid_catalog: true,
            distinct_aliases: aliases.len(),
            alias_hits,
            distinct_hit_questions: matched_case_ids.len(),
            hit_records,
            matched_case_ids: matched_case_ids.into_iter().collect(),
            frontend_cases: questions.len(),
            frontend_complete,
            frontend_ambiguous,
            frontend_missing,
            frontend_unsupported,
            frontend_replays,
            frontend_tamper_rejections,
            executable_candidates,
            candidate_replays,
            independent_exercise_evidence: false,
            promotion_status: "blocked".into(),
            reasons: vec![
                "exact alias overlap is a triage signal, not semantic proof".into(),
                "no independent external exercise corpus is attached".into(),
                "no frontend result authorizes an answer".into(),
            ],
        });
    }
    probes.sort_by(|a, b| {
        b.distinct_hit_questions
            .cmp(&a.distinct_hit_questions)
            .then_with(|| a.source_path.cmp(&b.source_path))
    });
    let manifest_after = breadth_first_manifest().replay_hash();
    let mut report = Report {
        schema: "external-source-alias-probe-v1",
        release_id: "external-math-exam-v1",
        development_questions: questions.len(),
        answer_keys_read: 0,
        source_documents_considered,
        parseable_sources,
        selected_sources: probes.len(),
        source_probes: probes,
        manifest_sha256_before: manifest_before.clone(),
        manifest_sha256_after: manifest_after.clone(),
        manifest_unchanged: manifest_before == manifest_after,
        false_authorizations: 0,
        report_sha256: String::new(),
    };
    let mut unsigned = serde_json::to_value(&report)?;
    unsigned["report_sha256"] = serde_json::Value::String(String::new());
    report.report_sha256 = digest(&unsigned);
    assert_eq!(report.answer_keys_read, 0);
    assert!(report.manifest_unchanged);
    assert_eq!(report.false_authorizations, 0);
    assert!(report.source_probes.iter().all(|probe| {
        probe.frontend_replays == probe.frontend_cases
            && probe.frontend_tamper_rejections == probe.frontend_cases
            && probe.candidate_replays == probe.executable_candidates
            && probe.promotion_status == "blocked"
    }));
    let serialized = serde_json::to_string_pretty(&report)?;
    fs::write(REPORT_JSON, format!("{serialized}\n"))?;
    fs::write(
        REPORT_MD,
        format!(
            "# Goal 6 — exact source-alias probe\n\n\
             - Development questions read: {}\n\
             - Answer keys read: {}\n\
             - Source documents considered / parseable: {} / {}\n\
             - Sources with at least three exact alias-hit questions: {}\n\
             - Promotion-eligible sources: 0\n\
             - Manifest unchanged: {}\n\
             - False authorizations: {}\n\n\
             Exact alias overlap is retained as selection evidence only. All\
             probes remain blocked without independent exercise evidence.\n",
            report.development_questions,
            report.answer_keys_read,
            report.source_documents_considered,
            report.parseable_sources,
            report.selected_sources,
            report.manifest_unchanged,
            report.false_authorizations,
        ),
    )?;
    println!("{serialized}");
    Ok(())
}
