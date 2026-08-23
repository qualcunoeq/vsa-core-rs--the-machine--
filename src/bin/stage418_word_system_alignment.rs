//! Privileged hash-only answer alignment for the frozen Stage 417 frontend.
//! Plaintext answers are never stored and production routing is untouched.

use serde::Deserialize;
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::env;
use std::fs;
use the_machine::curriculum::breadth_first_manifest;
use the_machine::source_word_system_frontend::{
    execute_word_system, execution_replay_verified, formalize_two_number_system, replay_verified,
    WordSystemStatus,
};

const CORPUS: &str = "docs/stage389_page_aware_external_problem_dev.json";
const CORPUS_SHA: &str = "92304d95521ac2ef4d49a08272f0b0cf79ac7225c9715d5ace9a667c822f6e03";
const ALIGNMENT: &str = "docs/stage418_word_system_alignment.json";
const REPORT_JSON: &str = "docs/stage418_word_system_alignment_result.json";
const REPORT_MD: &str = "docs/stage418_word_system_alignment_result.md";

#[derive(Debug, Deserialize)]
struct ExternalRecord {
    record_id: String,
    prompt: String,
    source_path: String,
    source_sha256: String,
    #[serde(default)]
    quality_flags: Vec<String>,
    answer_key_status: String,
}

#[derive(Debug, Deserialize)]
struct AlignmentRecord {
    record_id: String,
    expected_status: String,
    answer_sha256: Option<String>,
    source: String,
    source_sha256: String,
}

#[derive(Debug, Deserialize)]
struct AlignmentManifest {
    schema: String,
    corpus_sha256: String,
    records: Vec<AlignmentRecord>,
    plaintext_answers_stored: bool,
}

fn sha(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}
fn status_name(status: WordSystemStatus) -> &'static str {
    match status {
        WordSystemStatus::Complete => "complete",
        WordSystemStatus::Ambiguous => "ambiguous",
        WordSystemStatus::Missing => "missing",
        WordSystemStatus::Unsupported => "unsupported",
    }
}
fn candidate(prompt: &str) -> bool {
    let lower = prompt.to_ascii_lowercase();
    lower.contains("sum of two number") && lower.contains("one number is") && lower.contains("find")
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    assert_eq!(env::var("STAGE418_PRIVILEGED_EVAL").as_deref(), Ok("true"));
    let corpus_bytes = fs::read(CORPUS)?;
    let corpus_sha = sha(&corpus_bytes);
    assert_eq!(corpus_sha, CORPUS_SHA);
    let all: Vec<ExternalRecord> = serde_json::from_slice(&corpus_bytes)?;
    assert!(all.iter().all(|r| r.answer_key_status == "not_read"));
    let candidates = all
        .into_iter()
        .filter(|r| r.quality_flags.is_empty() && candidate(&r.prompt))
        .collect::<Vec<_>>();

    let alignment_bytes = fs::read(ALIGNMENT)?;
    let alignment_sha = sha(&alignment_bytes);
    let alignment: AlignmentManifest = serde_json::from_slice(&alignment_bytes)?;
    assert_eq!(alignment.schema, "stage418-word-system-alignment-v1");
    assert_eq!(alignment.corpus_sha256, CORPUS_SHA);
    assert!(!alignment.plaintext_answers_stored);
    let aligned = alignment
        .records
        .into_iter()
        .map(|r| (r.record_id.clone(), r))
        .collect::<BTreeMap<_, _>>();
    assert_eq!(aligned.len(), candidates.len());
    assert!(candidates
        .iter()
        .all(|r| aligned.contains_key(&r.record_id)));

    let before = breadth_first_manifest().replay_hash();
    let mut exact_statuses = 0;
    let mut complete_executions = 0;
    let mut correct_answers = 0;
    let mut incorrect_answers = 0;
    let mut false_denials = 0;
    let mut false_authorizations = 0;
    let mut frontend_replays = 0;
    let mut frontend_tamper = 0;
    let mut execution_replays = 0;
    let mut execution_tamper = 0;
    let mut answer_hashes = 0;
    let mut receipts = Vec::new();

    for record in &candidates {
        let expected = aligned.get(&record.record_id).unwrap();
        assert!(!expected.source.trim().is_empty());
        assert_eq!(expected.source_sha256, record.source_sha256);
        let result = formalize_two_number_system(&record.prompt, &record.record_id);
        let status = status_name(result.status);
        let status_match = status == expected.expected_status;
        exact_statuses += usize::from(status_match);
        let frontend_ok = replay_verified(&result);
        frontend_replays += usize::from(frontend_ok);
        let mut frontend_bad = result.clone();
        frontend_bad.replay_hash.push('x');
        frontend_tamper += usize::from(!replay_verified(&frontend_bad));

        let execution = execute_word_system(&result);
        let emitted = execution.is_some();
        complete_executions += usize::from(emitted);
        let mut reference_match = None;
        let mut execution_ok = false;
        let mut execution_bad = false;
        if let Some(receipt) = execution {
            if expected.expected_status != "complete" {
                false_authorizations += 1;
            }
            let expected_hash = expected.answer_sha256.as_ref().expect("complete hash");
            answer_hashes += 1;
            let matched = sha(receipt.result.as_bytes()) == *expected_hash;
            reference_match = Some(matched);
            if matched {
                correct_answers += 1;
            } else {
                incorrect_answers += 1;
            }
            execution_ok = execution_replay_verified(&receipt);
            let mut bad = receipt.clone();
            bad.result.push('x');
            execution_bad = !execution_replay_verified(&bad);
            execution_replays += usize::from(execution_ok);
            execution_tamper += usize::from(execution_bad);
        } else if expected.expected_status == "complete" {
            false_denials += 1;
        }
        receipts.push(serde_json::json!({
            "record_id": record.record_id,
            "source_path": record.source_path,
            "source_sha256": record.source_sha256,
            "expected_status": expected.expected_status,
            "observed_status": status,
            "status_match": status_match,
            "frontend_replay_verified": frontend_ok,
            "frontend_tamper_rejected": !frontend_bad.replay_hash.is_empty() && !replay_verified(&frontend_bad),
            "execution_emitted": emitted,
            "execution_replay_verified": execution_ok,
            "execution_tamper_rejected": execution_bad,
            "reference_match": reference_match
        }));
    }

    let after = breadth_first_manifest().replay_hash();
    let mut report = serde_json::json!({
        "schema": "stage418-word-system-alignment-result-v1",
        "stage417_commit": "eac8bfa",
        "corpus_path": CORPUS,
        "corpus_sha256": corpus_sha,
        "declared_corpus_sha256": CORPUS_SHA,
        "alignment_path": ALIGNMENT,
        "alignment_sha256": alignment_sha,
        "candidates": candidates.len(),
        "expected_complete": 8,
        "expected_ambiguous": 1,
        "expected_unsupported": 2,
        "exact_statuses": exact_statuses,
        "complete_executions": complete_executions,
        "correct_shadow_answers": correct_answers,
        "incorrect_shadow_answers": incorrect_answers,
        "false_denials": false_denials,
        "false_authorizations": false_authorizations,
        "frontend_replays": frontend_replays,
        "frontend_tamper_rejections": frontend_tamper,
        "execution_replays": execution_replays,
        "execution_tamper_rejections": execution_tamper,
        "answer_hashes_read": answer_hashes,
        "plaintext_answers_read": 0,
        "production_authorizations": 0,
        "manifest_sha256_before": before,
        "manifest_sha256_after": after,
        "manifest_unchanged": before == after,
        "receipts": receipts
    });
    let report_hash = sha(&serde_json::to_vec(&report)?);
    report["report_sha256"] = serde_json::Value::String(report_hash);
    assert_eq!(report["candidates"], 11);
    assert_eq!(report["exact_statuses"], 11);
    assert_eq!(report["complete_executions"], 8);
    assert_eq!(report["correct_shadow_answers"], 8);
    assert_eq!(report["incorrect_shadow_answers"], 0);
    assert_eq!(report["false_denials"], 0);
    assert_eq!(report["false_authorizations"], 0);
    assert_eq!(report["frontend_replays"], 11);
    assert_eq!(report["frontend_tamper_rejections"], 11);
    assert_eq!(report["execution_replays"], 8);
    assert_eq!(report["execution_tamper_rejections"], 8);
    assert_eq!(report["answer_hashes_read"], 8);
    assert_eq!(report["plaintext_answers_read"], 0);
    assert_eq!(report["production_authorizations"], 0);
    assert!(report["manifest_unchanged"].as_bool().unwrap());
    fs::write(
        REPORT_JSON,
        format!("{}\n", serde_json::to_string_pretty(&report)?),
    )?;
    fs::write(REPORT_MD, format!("# Stage 418 — bounded word-system answer alignment\n\n- candidates / complete / ambiguous / unsupported: 11 / 8 / 1 / 2\n- exact statuses / executions: {exact_statuses} / {complete_executions}\n- correct / incorrect shadow answers: {correct_answers} / {incorrect_answers}\n- false denials / false authorizations: {false_denials} / {false_authorizations}\n- frontend replay / tamper: {frontend_replays} / {frontend_tamper}\n- execution replay / tamper: {execution_replays} / {execution_tamper}\n- answer hashes / plaintext answers: {answer_hashes} / 0\n- production authorizations: 0\n- manifest unchanged: {}\n\nThis privileged scorer compares the frozen Stage 417 frontend with a separately governed hash-only alignment manifest.\n", before == after))?;
    println!("Stage 418 — candidates=11 correct={correct_answers} false_auth={false_authorizations} false_denials={false_denials}");
    Ok(())
}
