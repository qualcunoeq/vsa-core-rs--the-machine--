//! Stage 427: answer-key-blind external reachability for the uniform-die route.
//!
//! This is a shadow-only transfer probe.  It measures whether naturally
//! authored source language reaches the newly validated frontend; it never
//! reads answers and never authorizes production routing.

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::fs;
use the_machine::curriculum::breadth_first_manifest;
use the_machine::uniform_die_frontend::{formalize, replay_verified, FrontendStatus};

const CORPUS_PATH: &str = "docs/stage389_page_aware_external_problem_dev.json";
const CORPUS_SHA256: &str = "92304d95521ac2ef4d49a08272f0b0cf79ac7225c9715d5ace9a667c822f6e03";

#[derive(Debug, Deserialize)]
struct Record {
    record_id: String,
    prompt: String,
    prompt_sha256: String,
    split: String,
    #[serde(default)]
    quality_flags: Vec<String>,
    answer_key_status: String,
}

#[derive(Debug, Serialize)]
struct Candidate {
    record_id: String,
    split: String,
    prompt_sha256: String,
    status: FrontendStatus,
    replay_verified: bool,
    request_replay_hash: Option<String>,
}

#[derive(Debug, Serialize)]
struct Report {
    schema: &'static str,
    corpus_path: &'static str,
    corpus_sha256: String,
    declared_corpus_sha256: &'static str,
    signal_records: usize,
    complete: usize,
    ambiguous: usize,
    missing: usize,
    unsupported: usize,
    frontend_replay_verified: usize,
    candidate_records: Vec<Candidate>,
    answer_keys_read: usize,
    plaintext_answers_read: usize,
    production_authorizations: usize,
    false_authorizations: usize,
    manifest_unchanged: bool,
    report_sha256: String,
}

fn digest_bytes(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

fn digest<T: Serialize>(value: &T) -> String {
    digest_bytes(&serde_json::to_vec(value).unwrap())
}

fn signal(text: &str) -> bool {
    let lower = text.to_ascii_lowercase();
    ["probability", "probabilities", "coin is tossed", "dice"]
        .iter()
        .any(|marker| lower.contains(marker))
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let bytes = fs::read(CORPUS_PATH)?;
    let corpus_sha256 = digest_bytes(&bytes);
    assert_eq!(corpus_sha256, CORPUS_SHA256);
    let records: Vec<Record> = serde_json::from_slice(&bytes)?;
    let before = breadth_first_manifest().replay_hash();
    let mut complete = 0;
    let mut ambiguous = 0;
    let mut missing = 0;
    let mut unsupported = 0;
    let mut replay_count = 0;
    let mut candidate_records = Vec::new();
    let mut signal_records = 0;
    for (index, record) in records
        .iter()
        .filter(|r| r.quality_flags.is_empty())
        .enumerate()
    {
        assert_eq!(record.answer_key_status, "not_read");
        if !signal(&record.prompt) {
            continue;
        }
        signal_records += 1;
        let result = formalize(&record.prompt, &format!("stage427-{index:04}"));
        let replay = replay_verified(&result);
        replay_count += usize::from(replay);
        match result.status {
            FrontendStatus::Complete => complete += 1,
            FrontendStatus::Ambiguous => ambiguous += 1,
            FrontendStatus::Missing => missing += 1,
            FrontendStatus::Unsupported => unsupported += 1,
        }
        if result.status == FrontendStatus::Complete {
            candidate_records.push(Candidate {
                record_id: record.record_id.clone(),
                split: record.split.clone(),
                prompt_sha256: record.prompt_sha256.clone(),
                status: result.status,
                replay_verified: replay,
                request_replay_hash: result.request.as_ref().map(|request| digest(request)),
            });
        }
    }
    let after = breadth_first_manifest().replay_hash();
    assert_eq!(before, after);
    let mut report = Report {
        schema: "stage427-uniform-die-external-reachability-v1",
        corpus_path: CORPUS_PATH,
        corpus_sha256,
        declared_corpus_sha256: CORPUS_SHA256,
        signal_records,
        complete,
        ambiguous,
        missing,
        unsupported,
        frontend_replay_verified: replay_count,
        candidate_records,
        answer_keys_read: 0,
        plaintext_answers_read: 0,
        production_authorizations: 0,
        false_authorizations: 0,
        manifest_unchanged: before == after,
        report_sha256: String::new(),
    };
    report.report_sha256 = digest(&report);
    fs::write(
        "docs/stage427_uniform_die_external_reachability.json",
        serde_json::to_vec_pretty(&report)?,
    )?;
    fs::write(
        "docs/stage427_uniform_die_external_reachability.md",
        format!(
            "# Stage 427 — uniform-die external reachability\n\n- signal records: {}\n- complete / ambiguous / missing / unsupported: {} / {} / {} / {}\n- frontend replay: {}/{}\n- answer keys / plaintext answers / production authorizations / false authorizations: 0 / 0 / 0 / 0\n- manifest unchanged: {}\n- corpus SHA-256: `{}`\n\nThis is a reachability probe only; no candidate answer was authorized and no answer alignment was read.\n",
            report.signal_records,
            report.complete,
            report.ambiguous,
            report.missing,
            report.unsupported,
            report.frontend_replay_verified,
            report.signal_records,
            report.manifest_unchanged,
            report.corpus_sha256,
        ),
    )?;
    println!(
        "Stage 427 — signals={} complete={} ambiguous={} missing={} unsupported={} replay={}",
        report.signal_records,
        report.complete,
        report.ambiguous,
        report.missing,
        report.unsupported,
        report.frontend_replay_verified,
    );
    Ok(())
}
