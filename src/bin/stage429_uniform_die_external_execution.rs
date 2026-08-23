//! Stage 429: external uniform-die execution with a separate semantic oracle.
//!
//! The six reachable records are development-only source problems.  Their
//! source answer keys remain unread.  Expected fractions are independently
//! recomputed from the explicit finite experiment and kept in this evaluator
//! only to test the execution handoff, not to score a sealed benchmark.

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::fs;
use the_machine::curriculum::breadth_first_manifest;
use the_machine::probability_pack::Rational;
use the_machine::uniform_die_frontend::{
    execute, execution_replay_verified, formalize, replay_verified, FrontendStatus,
};

const CORPUS_PATH: &str = "docs/stage389_page_aware_external_problem_dev.json";
const CORPUS_SHA256: &str = "92304d95521ac2ef4d49a08272f0b0cf79ac7225c9715d5ace9a667c822f6e03";
const REACHABILITY_PATH: &str = "docs/stage427_uniform_die_external_reachability.json";

#[derive(Debug, Deserialize)]
struct Record {
    record_id: String,
    prompt: String,
    prompt_sha256: String,
    split: String,
    answer_key_status: String,
}

#[derive(Debug, Deserialize)]
struct Reachability {
    candidate_records: Vec<Candidate>,
}

#[derive(Debug, Deserialize)]
struct Candidate {
    record_id: String,
    prompt_sha256: String,
}

#[derive(Debug, Serialize)]
struct ResultRecord {
    record_id: String,
    prompt_sha256: String,
    status: FrontendStatus,
    value: Option<String>,
    expected_value: String,
    oracle_match: bool,
    frontend_replay: bool,
    frontend_tamper_rejected: bool,
    execution_replay: bool,
    execution_tamper_rejected: bool,
}

#[derive(Debug, Serialize)]
struct Report {
    schema: &'static str,
    corpus_path: &'static str,
    corpus_sha256: String,
    declared_corpus_sha256: &'static str,
    candidate_count: usize,
    complete_executions: usize,
    oracle_matches: usize,
    incorrect_oracle_matches: usize,
    frontend_replay_verified: usize,
    frontend_tamper_rejected: usize,
    execution_replay_verified: usize,
    execution_tamper_rejected: usize,
    answer_keys_read: usize,
    plaintext_answers_read: usize,
    production_authorizations: usize,
    false_authorizations: usize,
    manifest_unchanged: bool,
    results: Vec<ResultRecord>,
    report_sha256: String,
}

fn digest_bytes(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

fn digest<T: Serialize>(value: &T) -> String {
    digest_bytes(&serde_json::to_vec(value).unwrap())
}

fn oracle() -> BTreeMap<&'static str, Rational> {
    BTreeMap::from([
        ("7598f728add9eeccddde9a0978b35e2e514b3050c184e95d39901630012d2cfa", Rational::new(0, 1).unwrap()),
        ("6c904919856c95cb7ea48d5f799f18655d9f1f7ab681b38fa6937b5c022acd53", Rational::new(1, 1).unwrap()),
        ("1806fc0ffd2cede77d9b3e06ccb84fff0b14ae3f4ac70648cbf0160d97a2fa77", Rational::new(1, 12).unwrap()),
        ("5f20c1e112d3c5e6f73892630893ab8aaeb620aa17f47bc2bc8fc26e24a28241", Rational::new(1, 2).unwrap()),
        ("a859d189af54278c2076a6dd5c1d77e983bfff6a5244625223f683694161feef", Rational::new(11, 12).unwrap()),
        ("f0306a82bae994aeb099895e1d843362e8f054d4679e60233159a51b1922d5f4", Rational::new(1, 4).unwrap()),
    ])
}

fn value_text(value: &Option<Rational>) -> Option<String> {
    value
        .as_ref()
        .map(|value| format!("{}/{}", value.numerator, value.denominator))
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let bytes = fs::read(CORPUS_PATH)?;
    let corpus_sha256 = digest_bytes(&bytes);
    assert_eq!(corpus_sha256, CORPUS_SHA256);
    let records: Vec<Record> = serde_json::from_slice(&bytes)?;
    let reachability: Reachability = serde_json::from_slice(&fs::read(REACHABILITY_PATH)?)?;
    let selected: BTreeMap<_, _> = reachability
        .candidate_records
        .iter()
        .map(|candidate| (candidate.record_id.as_str(), candidate.prompt_sha256.as_str()))
        .collect();
    let expected = oracle();
    let before = breadth_first_manifest().replay_hash();
    let mut results = Vec::new();
    for record in records.iter().filter(|record| selected.contains_key(record.record_id.as_str())) {
        assert_eq!(record.split, "development");
        assert_eq!(record.answer_key_status, "not_read");
        assert_eq!(selected[record.record_id.as_str()], record.prompt_sha256);
        let expected_value = expected
            .get(record.prompt_sha256.as_str())
            .expect("every reachable candidate has an independently derived oracle");
        let frontend = formalize(&record.prompt, &format!("stage429-{}", record.record_id));
        let mut frontend_tampered = frontend.clone();
        frontend_tampered.replay_hash.push('x');
        let (status, value, execution_replay, execution_tamper_rejected) =
            if let Some(request) = frontend.request.as_ref() {
                let execution = execute(request);
                let mut tampered = execution.clone();
                tampered.replay_hash.push('x');
                let value = execution.value.clone();
                (
                    execution.status,
                    value,
                    execution_replay_verified(&execution),
                    !execution_replay_verified(&tampered),
                )
            } else {
                (frontend.status, None, false, true)
            };
        let oracle_match = status == FrontendStatus::Complete && value.as_ref() == Some(expected_value);
        results.push(ResultRecord {
            record_id: record.record_id.clone(),
            prompt_sha256: record.prompt_sha256.clone(),
            status,
            value: value_text(&value),
            expected_value: format!("{}/{}", expected_value.numerator, expected_value.denominator),
            oracle_match,
            frontend_replay: replay_verified(&frontend),
            frontend_tamper_rejected: !replay_verified(&frontend_tampered),
            execution_replay,
            execution_tamper_rejected,
        });
    }
    assert_eq!(results.len(), 6);
    let after = breadth_first_manifest().replay_hash();
    assert_eq!(before, after);
    let report_without_hash = Report {
        schema: "stage429-uniform-die-external-execution-v1",
        corpus_path: CORPUS_PATH,
        corpus_sha256,
        declared_corpus_sha256: CORPUS_SHA256,
        candidate_count: results.len(),
        complete_executions: results.iter().filter(|result| result.status == FrontendStatus::Complete).count(),
        oracle_matches: results.iter().filter(|result| result.oracle_match).count(),
        incorrect_oracle_matches: results.iter().filter(|result| !result.oracle_match).count(),
        frontend_replay_verified: results.iter().filter(|result| result.frontend_replay).count(),
        frontend_tamper_rejected: results.iter().filter(|result| result.frontend_tamper_rejected).count(),
        execution_replay_verified: results.iter().filter(|result| result.execution_replay).count(),
        execution_tamper_rejected: results.iter().filter(|result| result.execution_tamper_rejected).count(),
        answer_keys_read: 0,
        plaintext_answers_read: 0,
        production_authorizations: 0,
        false_authorizations: 0,
        manifest_unchanged: before == after,
        results,
        report_sha256: String::new(),
    };
    let mut report = report_without_hash;
    report.report_sha256 = digest(&report);
    fs::write("docs/stage429_uniform_die_external_execution.json", serde_json::to_vec_pretty(&report)?)?;
    fs::write(
        "docs/stage429_uniform_die_external_execution.md",
        format!(
            "# Stage 429 — external uniform-die execution audit\n\n- development candidates: {}\n- complete executions: {}/{}\n- independent semantic-oracle matches: {}/{}\n- incorrect oracle matches: {}\n- frontend replay / tamper: {}/{} / {}/{}\n- execution replay / tamper: {}/{} / {}/{}\n- answer keys / plaintext answers / production authorizations / false authorizations: 0 / 0 / 0 / 0\n- manifest unchanged: {}\n- corpus SHA-256: `{}`\n\nThe oracle values were independently recomputed from the explicit finite experiments. This is not a sealed benchmark score and does not read source answer keys.\n",
            report.candidate_count,
            report.complete_executions,
            report.candidate_count,
            report.oracle_matches,
            report.candidate_count,
            report.incorrect_oracle_matches,
            report.frontend_replay_verified,
            report.candidate_count,
            report.frontend_tamper_rejected,
            report.candidate_count,
            report.execution_replay_verified,
            report.candidate_count,
            report.execution_tamper_rejected,
            report.candidate_count,
            report.manifest_unchanged,
            report.corpus_sha256,
        ),
    )?;
    println!(
        "Stage 429 — candidates={} complete={} oracle_matches={} replay={}/{}",
        report.candidate_count,
        report.complete_executions,
        report.oracle_matches,
        report.execution_replay_verified,
        report.candidate_count,
    );
    Ok(())
}
