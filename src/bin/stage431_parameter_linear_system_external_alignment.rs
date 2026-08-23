//! Stage 431: answer-key-blind external alignment for the parameter-system
//! frontend.
//!
//! The development prompts are read from the independently sourced MATH
//! release.  Only answer hashes from the separate development oracle are
//! consumed; plaintext answers are never loaded.  The route remains shadow
//! only and does not authorize or mutate production routing.

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::env;
use std::fs::{self, File};
use std::io::{BufRead, BufReader};
use the_machine::curriculum::breadth_first_manifest;
use the_machine::parameter_linear_system_frontend::{
    execute, execution_replay_verified, formalize, replay_verified, FrontendStatus,
};

const QUESTIONS_PATH: &str = "data/external_math_exam_v1/questions.jsonl";
const ORACLE_PATH: &str = "data/external_math_exam_v1/oracle_development.jsonl";
const QUESTIONS_SHA256: &str =
    "3cf924116a0f8f6a0c84d0ce7949b0c1e16221e0d4b5fcb0c4322110e30714f2";
const ORACLE_SHA256: &str =
    "5bc0e75c5af49d0500cf14437c566e1eed398dd9aeef32c91d05062b83b22ec0";

#[derive(Debug, Deserialize)]
struct QuestionRecord {
    id: String,
    split: String,
    original_prompt: String,
}

#[derive(Debug, Deserialize)]
struct OracleRecord {
    id: String,
    expected_outcome: String,
    answer_sha256: String,
}

#[derive(Debug, Serialize)]
struct CandidateResult {
    id: String,
    prompt_sha256: String,
    frontend_status: FrontendStatus,
    answer_sha256: Option<String>,
    expected_answer_sha256: Option<String>,
    oracle_match: bool,
    frontend_replay: bool,
    frontend_tamper_rejected: bool,
    execution_replay: bool,
    execution_tamper_rejected: bool,
}

#[derive(Debug, Serialize)]
struct Report {
    schema: &'static str,
    questions_path: &'static str,
    oracle_path: &'static str,
    questions_sha256: String,
    declared_questions_sha256: &'static str,
    oracle_sha256: String,
    declared_oracle_sha256: &'static str,
    development_questions_read: usize,
    sealed_questions_read: usize,
    complete_frontends: usize,
    complete_executions: usize,
    oracle_matches: usize,
    incorrect_shadow_answers: usize,
    frontend_replay_verified: usize,
    frontend_tamper_rejected: usize,
    execution_replay_verified: usize,
    execution_tamper_rejected: usize,
    answer_hashes_read: usize,
    plaintext_answers_read: usize,
    production_authorizations: usize,
    false_authorizations: usize,
    manifest_unchanged: bool,
    results: Vec<CandidateResult>,
    report_sha256: String,
}

fn digest_bytes(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

fn digest<T: Serialize>(value: &T) -> String {
    digest_bytes(&serde_json::to_vec(value).expect("serialize report"))
}

fn read_jsonl<T: for<'de> Deserialize<'de>>(
    path: &str,
) -> Result<Vec<T>, Box<dyn std::error::Error>> {
    let mut records = Vec::new();
    for line in BufReader::new(File::open(path)?).lines() {
        let line = line?;
        if !line.trim().is_empty() {
            records.push(serde_json::from_str(&line)?);
        }
    }
    Ok(records)
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    if env::var("STAGE431_PRIVILEGED_EVAL").as_deref() != Ok("true") {
        return Err("set STAGE431_PRIVILEGED_EVAL=true for hash-only alignment".into());
    }
    let question_bytes = fs::read(QUESTIONS_PATH)?;
    let oracle_bytes = fs::read(ORACLE_PATH)?;
    let questions_sha256 = digest_bytes(&question_bytes);
    let oracle_sha256 = digest_bytes(&oracle_bytes);
    assert_eq!(questions_sha256, QUESTIONS_SHA256);
    assert_eq!(oracle_sha256, ORACLE_SHA256);
    let questions: Vec<QuestionRecord> = read_jsonl(QUESTIONS_PATH)?;
    let oracle_records: Vec<OracleRecord> = read_jsonl(ORACLE_PATH)?;
    let oracle: BTreeMap<_, _> = oracle_records
        .iter()
        .map(|record| {
            assert_eq!(record.expected_outcome, "supported");
            (record.id.as_str(), record.answer_sha256.as_str())
        })
        .collect();
    let before = breadth_first_manifest().replay_hash();
    let mut development_questions_read = 0;
    let mut sealed_questions_read = 0;
    let mut complete_frontends = 0;
    let mut complete_executions = 0;
    let mut oracle_matches = 0;
    let mut incorrect_shadow_answers = 0;
    let mut frontend_replay_verified_count = 0;
    let mut frontend_tamper_rejected_count = 0;
    let mut execution_replay_verified_count = 0;
    let mut execution_tamper_rejected_count = 0;
    let mut results = Vec::new();

    for question in &questions {
        if question.split == "sealed" {
            // The sealed partition is intentionally not consumed by this
            // development alignment run, including its prompt text.
            sealed_questions_read += 1;
            continue;
        }
        assert_eq!(question.split, "development");
        development_questions_read += 1;
        let frontend = formalize(&question.original_prompt, &format!("stage431-{}", question.id));
        let mut frontend_tampered = frontend.clone();
        frontend_tampered.replay_hash.push('x');
        let frontend_replay = replay_verified(&frontend);
        let frontend_tamper_rejected = !replay_verified(&frontend_tampered);
        frontend_replay_verified_count += usize::from(frontend_replay);
        frontend_tamper_rejected_count += usize::from(frontend_tamper_rejected);
        if frontend.status != FrontendStatus::Complete {
            continue;
        }
        complete_frontends += 1;
        let request = frontend.request.as_ref().expect("complete frontend request");
        let execution = execute(request);
        let mut execution_tampered = execution.clone();
        execution_tampered.replay_hash.push('x');
        let execution_replay = execution_replay_verified(&execution);
        let execution_tamper_rejected = !execution_replay_verified(&execution_tampered);
        execution_replay_verified_count += usize::from(execution_replay);
        execution_tamper_rejected_count += usize::from(execution_tamper_rejected);
        if execution.status != FrontendStatus::Complete {
            continue;
        }
        complete_executions += 1;
        let answer_sha256 = execution
            .answer
            .as_deref()
            .map(|answer| digest_bytes(answer.as_bytes()));
        let expected_answer_sha256 = oracle.get(question.id.as_str()).copied();
        let oracle_match = expected_answer_sha256.is_some_and(|expected| {
            answer_sha256.as_deref() == Some(expected)
        });
        if oracle_match {
            oracle_matches += 1;
        } else {
            incorrect_shadow_answers += 1;
        }
        results.push(CandidateResult {
            id: question.id.clone(),
            prompt_sha256: digest_bytes(question.original_prompt.as_bytes()),
            frontend_status: frontend.status,
            answer_sha256,
            expected_answer_sha256: expected_answer_sha256.map(str::to_owned),
            oracle_match,
            frontend_replay,
            frontend_tamper_rejected,
            execution_replay,
            execution_tamper_rejected,
        });
    }
    let after = breadth_first_manifest().replay_hash();
    assert_eq!(before, after);
    let report_without_hash = Report {
        schema: "stage431-parameter-linear-system-external-alignment-v1",
        questions_path: QUESTIONS_PATH,
        oracle_path: ORACLE_PATH,
        questions_sha256,
        declared_questions_sha256: QUESTIONS_SHA256,
        oracle_sha256,
        declared_oracle_sha256: ORACLE_SHA256,
        development_questions_read,
        sealed_questions_read,
        complete_frontends,
        complete_executions,
        oracle_matches,
        incorrect_shadow_answers,
        frontend_replay_verified: frontend_replay_verified_count,
        frontend_tamper_rejected: frontend_tamper_rejected_count,
        execution_replay_verified: execution_replay_verified_count,
        execution_tamper_rejected: execution_tamper_rejected_count,
        answer_hashes_read: oracle_records.len(),
        plaintext_answers_read: 0,
        production_authorizations: 0,
        false_authorizations: 0,
        manifest_unchanged: before == after,
        results,
        report_sha256: String::new(),
    };
    let mut report = report_without_hash;
    report.report_sha256 = digest(&report);
    fs::write(
        "docs/stage431_parameter_linear_system_external_alignment.json",
        serde_json::to_vec_pretty(&report)?,
    )?;
    fs::write(
        "docs/stage431_parameter_linear_system_external_alignment.md",
        format!(
            "# Stage 431 — parameterized linear-system external alignment\n\n- development / sealed questions: {} / {}\n- complete frontends / executions: {} / {}\n- oracle matches / incorrect shadow answers: {} / {}\n- frontend replay / tamper: {} / {}\n- execution replay / tamper: {} / {}\n- answer hashes / plaintext answers: {} / {}\n- production authorizations / false authorizations: {} / {}\n- manifest unchanged: {}\n- questions SHA-256: `{}`\n- oracle SHA-256: `{}`\n\nThis is a shadow-only development alignment. It reads answer hashes but never plaintext answers, consumes no sealed prompts, and does not mutate production routing.\n",
            report.development_questions_read,
            report.sealed_questions_read,
            report.complete_frontends,
            report.complete_executions,
            report.oracle_matches,
            report.incorrect_shadow_answers,
            report.frontend_replay_verified,
            report.frontend_tamper_rejected,
            report.execution_replay_verified,
            report.execution_tamper_rejected,
            report.answer_hashes_read,
            report.plaintext_answers_read,
            report.production_authorizations,
            report.false_authorizations,
            report.manifest_unchanged,
            report.questions_sha256,
            report.oracle_sha256,
        ),
    )?;
    println!(
        "Stage 431 — development={} complete={} matches={} incorrect_shadow={}",
        report.development_questions_read,
        report.complete_executions,
        report.oracle_matches,
        report.incorrect_shadow_answers,
    );
    Ok(())
}
