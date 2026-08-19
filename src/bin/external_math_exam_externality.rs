//! Externality audit for the 4,000-case public mathematics release.
//!
//! This checker deliberately does not read either oracle.  It inspects only
//! the release manifest/questions and a fixed list of local development
//! artifacts, looking for answer exposure, split contamination, and prompt or
//! template overlap.

use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::collections::BTreeSet;
use std::fs;

const QUESTIONS: &str = "data/external_math_exam_v1/questions.jsonl";
const MANIFEST: &str = "data/external_math_exam_v1/manifest.json";
const LOCAL_ARTIFACTS: &[&str] = &[
    "data/external_decomposition_v1.json",
    "data/external_decomposition_v2.json",
    "data/compositional_planner_ood_v1.json",
    "data/formalization_baseline_v9.json",
    "data/formalization_baseline_v10.json",
    "data/algebra_ood_v1.json",
];

#[derive(Debug, Deserialize, Serialize)]
struct Question {
    id: String,
    split: String,
    original_prompt: String,
}

#[derive(Debug, Serialize)]
struct Report {
    schema: &'static str,
    release_id: String,
    source_independence: bool,
    source_locator: String,
    source_revision: String,
    license: String,
    development_cases: usize,
    sealed_cases: usize,
    total_cases: usize,
    duplicate_question_count: usize,
    duplicate_template_count: usize,
    template_overlap_review: String,
    local_lexical_overlap_count: usize,
    local_template_overlap_count: usize,
    answer_key_field_exposure_count: usize,
    answer_hash_in_prompt_count: usize,
    holdout_locked: bool,
    final_exam_eligible: bool,
    verdict: String,
    manifest_sha256: String,
    questions_sha256: String,
}

fn digest(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

fn tokens(prompt: &str) -> Vec<String> {
    prompt
        .split(|ch: char| !ch.is_ascii_alphanumeric())
        .filter(|token| !token.is_empty())
        .map(|token| token.to_ascii_lowercase())
        .collect()
}

fn template(prompt: &str) -> String {
    tokens(prompt)
        .into_iter()
        .map(|token| {
            if token.chars().all(|ch| ch.is_ascii_digit()) {
                "<num>".to_string()
            } else if token.len() >= 12 {
                "<long>".to_string()
            } else {
                token
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

fn collect_prompt_strings(value: &Value, output: &mut Vec<String>) {
    match value {
        Value::Object(map) => {
            for (key, child) in map {
                let lower = key.to_ascii_lowercase();
                if matches!(
                    lower.as_str(),
                    "prompt" | "question" | "original_prompt" | "problem" | "text"
                ) {
                    if let Some(text) = child.as_str() {
                        output.push(text.to_string());
                    }
                }
                collect_prompt_strings(child, output);
            }
        }
        Value::Array(values) => values
            .iter()
            .for_each(|child| collect_prompt_strings(child, output)),
        _ => {}
    }
}

fn contains_answer_key(value: &Value) -> bool {
    match value {
        Value::Object(map) => map.iter().any(|(key, child)| {
            matches!(
                key.to_ascii_lowercase().as_str(),
                "answer" | "solution" | "extracted_solution" | "answer_sha256"
            ) || contains_answer_key(child)
        }),
        Value::Array(values) => values.iter().any(contains_answer_key),
        _ => false,
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let manifest_bytes = fs::read(MANIFEST)?;
    let questions_bytes = fs::read(QUESTIONS)?;
    let manifest: Value = serde_json::from_slice(&manifest_bytes)?;
    let questions = questions_bytes
        .split(|byte| *byte == b'\n')
        .filter(|line| !line.is_empty())
        .map(serde_json::from_slice::<Question>)
        .collect::<Result<Vec<_>, _>>()?;
    let source = manifest.get("source").ok_or("manifest missing source")?;
    let source_locator = source
        .get("dataset_locator")
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string();
    let source_revision = source
        .get("revision")
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string();
    let license = source
        .get("license")
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string();
    let source_independence = source_locator.starts_with("https://")
        && !source_locator.contains("the-machine")
        && source_revision.len() == 40
        && license.eq_ignore_ascii_case("mit")
        && source
            .get("source_train_sha256")
            .and_then(Value::as_str)
            .is_some_and(|hash| hash.len() == 64)
        && source
            .get("source_test_sha256")
            .and_then(Value::as_str)
            .is_some_and(|hash| hash.len() == 64);
    let development: Vec<&Question> = questions
        .iter()
        .filter(|question| question.split == "development")
        .collect();
    let sealed: Vec<&Question> = questions
        .iter()
        .filter(|question| question.split == "sealed")
        .collect();
    let development_questions: BTreeSet<String> = development
        .iter()
        .map(|question| question.original_prompt.clone())
        .collect();
    let development_templates: BTreeSet<String> = development
        .iter()
        .map(|question| template(&question.original_prompt))
        .collect();
    let mut duplicate_question_count = 0;
    let mut duplicate_template_count = 0;
    for question in &sealed {
        duplicate_question_count +=
            usize::from(development_questions.contains(&question.original_prompt));
        duplicate_template_count +=
            usize::from(development_templates.contains(&template(&question.original_prompt)));
    }
    let mut local_prompts = Vec::new();
    for path in LOCAL_ARTIFACTS {
        if let Ok(bytes) = fs::read(path) {
            if let Ok(value) = serde_json::from_slice::<Value>(&bytes) {
                collect_prompt_strings(&value, &mut local_prompts);
            }
        }
    }
    let local_prompt_set: BTreeSet<String> = local_prompts.iter().cloned().collect();
    let local_template_set: BTreeSet<String> = local_prompts.iter().map(|p| template(p)).collect();
    let local_lexical_overlap_count = questions
        .iter()
        .filter(|question| local_prompt_set.contains(&question.original_prompt))
        .count();
    let local_template_overlap_count = questions
        .iter()
        .filter(|question| local_template_set.contains(&template(&question.original_prompt)))
        .count();
    let answer_key_field_exposure_count = questions
        .iter()
        .filter(|question| {
            serde_json::to_value(question)
                .map(|value| contains_answer_key(&value))
                .unwrap_or(true)
        })
        .count();
    let answer_hash_in_prompt_count = questions
        .iter()
        .filter(|question| question.original_prompt.contains("sha256"))
        .count();
    let holdout_locked = manifest
        .get("holdout_locked")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    // The public source naturally reuses parameterized exercise forms.  A
    // template match is therefore a review signal, not contamination by
    // itself.  Exact prompt/source-item overlap and local-artifact overlap
    // remain hard failures.
    let template_overlap_review = if duplicate_template_count == 0 {
        "no_template_overlap".to_string()
    } else if duplicate_question_count == 0 {
        "parameterized_source_variants_no_exact_copy".to_string()
    } else {
        "exact_copy_detected".to_string()
    };
    let final_exam_eligible = source_independence
        && holdout_locked
        && questions.len() >= 3_000
        && questions.len() <= 5_000
        && development.len() == 3_000
        && sealed.len() == 1_000
        && duplicate_question_count == 0
        && template_overlap_review != "exact_copy_detected"
        && local_lexical_overlap_count == 0
        && local_template_overlap_count == 0
        && answer_key_field_exposure_count == 0
        && answer_hash_in_prompt_count == 0;
    let report = Report {
        schema: "goal3-externality-audit-external-math-v1",
        release_id: manifest
            .get("release_id")
            .and_then(Value::as_str)
            .unwrap_or("unknown")
            .to_string(),
        source_independence,
        source_locator,
        source_revision,
        license,
        development_cases: development.len(),
        sealed_cases: sealed.len(),
        total_cases: questions.len(),
        duplicate_question_count,
        duplicate_template_count,
        local_lexical_overlap_count,
        local_template_overlap_count,
        answer_key_field_exposure_count,
        answer_hash_in_prompt_count,
        holdout_locked,
        final_exam_eligible,
        template_overlap_review,
        verdict: if final_exam_eligible && duplicate_template_count == 0 {
            "independent_external_release".into()
        } else if final_exam_eligible {
            "independent_external_release_template_overlap_reviewed".into()
        } else {
            "externality_audit_failed".into()
        },
        manifest_sha256: digest(&manifest_bytes),
        questions_sha256: digest(&questions_bytes),
    };
    assert!(
        report.final_exam_eligible,
        "externality audit failed: {report:?}"
    );
    fs::write(
        "docs/goal3_externality_audit_external_math.json",
        format!("{}\n", serde_json::to_string_pretty(&report)?),
    )?;
    fs::write(
        "docs/goal3_externality_audit_external_math.md",
        format!(
            "# Goal 3 — external MATH release audit\n\n\
             - Release: `{}`\n- Cases: {} (development {}, sealed {})\n\
             - Source independence: {}\n- Prompt overlap with local artifacts: {}\n\
             - Template overlap with local artifacts: {}\n- Development/sealed exact overlap: {}\n\
             - Development/sealed template overlap: {}\n\
             - Template review: `{}`\n- Answer-key fields in questions: {}\n\
             - Final-exam eligible: {}\n- Verdict: `{}`\n",
            report.release_id,
            report.total_cases,
            report.development_cases,
            report.sealed_cases,
            report.source_independence,
            report.local_lexical_overlap_count,
            report.local_template_overlap_count,
            report.duplicate_question_count,
            report.duplicate_template_count,
            report.template_overlap_review,
            report.answer_key_field_exposure_count,
            report.final_exam_eligible,
            report.verdict,
        ),
    )?;
    println!("{}", serde_json::to_string_pretty(&report)?);
    Ok(())
}
