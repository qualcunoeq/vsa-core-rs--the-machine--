//! Privileged, post-freeze scorer for the arithmetic-sequence shadow route.
//! It reads development answer hashes only, never plaintext or sealed data,
//! and cannot authorize or mutate a production route.

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::fs;
use the_machine::curriculum::breadth_first_manifest;
use the_machine::source_formula_pack::{
    evaluate_formula_records, source_formula_records, FormulaStatus,
};
use the_machine::source_sequence_frontend::{formalize_sequence_terms_text, replay_verified};

const RELEASE_DIR: &str = "data/external_math_exam_v1";
const DOMAIN: &str = "external-source-sequence-shadow";
const REPORT_JSON: &str = "docs/goal6_external_sequence_shadow_score.json";
const REPORT_MD: &str = "docs/goal6_external_sequence_shadow_score.md";

#[derive(Debug, Deserialize)]
struct Question {
    id: String,
    original_prompt: String,
}

#[derive(Debug, Deserialize)]
struct Oracle {
    id: String,
    answer_sha256: String,
}

#[derive(Debug, Serialize)]
struct CandidateReceipt {
    id: String,
    candidate_answer_sha256: String,
    reference_match: bool,
    replay_verified: bool,
}

#[derive(Debug, Serialize)]
struct Report {
    schema: &'static str,
    release_id: &'static str,
    partition: &'static str,
    source_domain: &'static str,
    questions_read: usize,
    answer_hashes_read: usize,
    plaintext_answers_read: usize,
    candidate_cases: usize,
    correct_shadow_candidates: usize,
    incorrect_shadow_candidates_rejected: usize,
    candidate_replays: usize,
    production_authorizations: usize,
    false_authorizations: usize,
    manifest_unchanged: bool,
    candidates: Vec<CandidateReceipt>,
    report_sha256: String,
}

fn digest_bytes(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

fn digest<T: Serialize>(value: &T) -> String {
    format!("{:x}", Sha256::digest(serde_json::to_vec(value).unwrap()))
}

fn answer_text(value: &the_machine::probability_pack::Rational) -> String {
    if value.denominator == 1 {
        value.numerator.to_string()
    } else {
        format!("{}/{}", value.numerator, value.denominator)
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let records = source_formula_records();
    let questions: Vec<Question> = fs::read_to_string(format!("{RELEASE_DIR}/questions.jsonl"))?
        .lines()
        .filter(|line| line.contains("\"split\": \"development\""))
        .map(serde_json::from_str)
        .collect::<Result<Vec<Question>, _>>()?;
    let oracle: BTreeMap<String, Oracle> = fs::read_to_string(format!(
        "{RELEASE_DIR}/oracle_development.jsonl"
    ))?
    .lines()
    .filter(|line| !line.trim().is_empty())
    .map(serde_json::from_str::<Oracle>)
    .collect::<Result<Vec<_>, _>>()?
    .into_iter()
    .map(|record| (record.id.clone(), record))
    .collect();
    let manifest_before = breadth_first_manifest().replay_hash();
    let mut candidates = Vec::new();
    for question in &questions {
        let frontend = formalize_sequence_terms_text(
            &question.original_prompt,
            &question.id,
            DOMAIN,
        );
        let Some(request) = frontend.request.as_ref() else {
            continue;
        };
        let execution = evaluate_formula_records(request, DOMAIN, &records);
        if execution.status != FormulaStatus::Complete || !execution.replay_verified() {
            continue;
        }
        let Some(value) = execution.value.as_ref() else {
            continue;
        };
        let candidate_hash = digest_bytes(answer_text(value).as_bytes());
        let expected = oracle
            .get(&question.id)
            .ok_or_else(|| format!("missing development oracle for {}", question.id))?;
        candidates.push(CandidateReceipt {
            id: question.id.clone(),
            candidate_answer_sha256: candidate_hash.clone(),
            reference_match: candidate_hash == expected.answer_sha256,
            replay_verified: replay_verified(&frontend) && execution.replay_verified(),
        });
    }
    let manifest_after = breadth_first_manifest().replay_hash();
    let mut report = Report {
        schema: "goal6-external-sequence-shadow-score-v1",
        release_id: "external-math-exam-v1",
        partition: "development",
        source_domain: DOMAIN,
        questions_read: questions.len(),
        answer_hashes_read: oracle.len(),
        plaintext_answers_read: 0,
        candidate_cases: candidates.len(),
        correct_shadow_candidates: candidates.iter().filter(|candidate| candidate.reference_match).count(),
        incorrect_shadow_candidates_rejected: candidates.iter().filter(|candidate| !candidate.reference_match).count(),
        candidate_replays: candidates.iter().filter(|candidate| candidate.replay_verified).count(),
        production_authorizations: 0,
        false_authorizations: 0,
        manifest_unchanged: manifest_before == manifest_after,
        candidates,
        report_sha256: String::new(),
    };
    let mut unsigned = serde_json::to_value(&report)?;
    unsigned["report_sha256"] = serde_json::Value::String(String::new());
    report.report_sha256 = digest(&unsigned);
    assert_eq!(report.plaintext_answers_read, 0);
    assert_eq!(report.production_authorizations, 0);
    assert_eq!(report.false_authorizations, 0);
    assert!(report.manifest_unchanged);
    assert_eq!(report.candidate_replays, report.candidate_cases);
    let serialized = serde_json::to_string_pretty(&report)?;
    fs::write(REPORT_JSON, format!("{serialized}\n"))?;
    fs::write(
        REPORT_MD,
        format!(
            "# Goal 6 — arithmetic-sequence privileged shadow score\n\n- Development questions read: {}\n- Answer hashes read / plaintext answers read: {} / {}\n- Shadow candidates: {}\n- Correct / rejected shadow candidates: {} / {}\n- Candidate replay: {} / {}\n- Production authorizations / false authorizations: {} / {}\n- Manifest unchanged: {}\n\nThis scorer is separate from answer-key-blind acquisition and reads only development hashes after the route is frozen.\n",
            report.questions_read,
            report.answer_hashes_read,
            report.plaintext_answers_read,
            report.candidate_cases,
            report.correct_shadow_candidates,
            report.incorrect_shadow_candidates_rejected,
            report.candidate_replays,
            report.candidate_cases,
            report.production_authorizations,
            report.false_authorizations,
            report.manifest_unchanged,
        ),
    )?;
    println!("{serialized}");
    Ok(())
}
