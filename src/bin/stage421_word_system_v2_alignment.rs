//! Privileged hash-only alignment for the V2 shadow frontend.
//! The Stage 418 alignment is reused for the original eight cases; one
//! separately governed delta adds the newly supported five-times case.

use serde::Deserialize;
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::env;
use std::fs;
use the_machine::curriculum::breadth_first_manifest;
use the_machine::source_word_system_frontend::{
    execute_word_system, execution_replay_verified, formalize_two_number_system_v2,
    replay_verified, WordSystemStatus,
};

const CORPUS: &str = "docs/stage389_page_aware_external_problem_dev.json";
const CORPUS_SHA: &str = "92304d95521ac2ef4d49a08272f0b0cf79ac7225c9715d5ace9a667c822f6e03";
const BASE: &str = "docs/stage418_word_system_alignment.json";
const DELTA: &str = "docs/stage421_word_system_v2_alignment_delta.json";

#[derive(Debug, Deserialize)]
struct ExternalRecord {
    record_id: String,
    prompt: String,
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
    source_sha256: String,
}
#[derive(Debug, Deserialize)]
struct AlignmentFile {
    records: Vec<AlignmentRecord>,
    corpus_sha256: String,
    plaintext_answers_stored: bool,
}
fn sha(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}
fn status(s: WordSystemStatus) -> &'static str {
    match s {
        WordSystemStatus::Complete => "complete",
        WordSystemStatus::Ambiguous => "ambiguous",
        WordSystemStatus::Missing => "missing",
        WordSystemStatus::Unsupported => "unsupported",
    }
}
fn candidate(prompt: &str) -> bool {
    let p = prompt.to_ascii_lowercase();
    p.contains("sum of two number") && p.contains("one number is") && p.contains("find")
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    assert_eq!(env::var("STAGE421_PRIVILEGED_EVAL").as_deref(), Ok("true"));
    let corpus_bytes = fs::read(CORPUS)?;
    assert_eq!(sha(&corpus_bytes), CORPUS_SHA);
    let all: Vec<ExternalRecord> = serde_json::from_slice(&corpus_bytes)?;
    assert!(all.iter().all(|r| r.answer_key_status == "not_read"));
    let candidates = all
        .into_iter()
        .filter(|r| r.quality_flags.is_empty() && candidate(&r.prompt))
        .collect::<Vec<_>>();
    let base: AlignmentFile = serde_json::from_str(&fs::read_to_string(BASE)?)?;
    let delta: AlignmentFile = serde_json::from_str(&fs::read_to_string(DELTA)?)?;
    assert_eq!(base.corpus_sha256, CORPUS_SHA);
    assert_eq!(delta.corpus_sha256, CORPUS_SHA);
    assert!(!base.plaintext_answers_stored && !delta.plaintext_answers_stored);
    let mut aligned = base
        .records
        .into_iter()
        .map(|r| (r.record_id.clone(), r))
        .collect::<BTreeMap<_, _>>();
    for record in delta.records {
        aligned.insert(record.record_id.clone(), record);
    }
    assert_eq!(aligned.len(), candidates.len());
    let before = breadth_first_manifest().replay_hash();
    let mut exact = 0;
    let mut complete = 0;
    let mut correct = 0;
    let mut incorrect = 0;
    let mut false_denials = 0;
    let mut false_auth = 0;
    let mut front_replay = 0;
    let mut front_tamper = 0;
    let mut exec_replay = 0;
    let mut exec_tamper = 0;
    let mut hashes = 0;
    for record in &candidates {
        let expected = aligned.get(&record.record_id).unwrap();
        assert_eq!(expected.source_sha256, record.source_sha256);
        let frontend = formalize_two_number_system_v2(&record.prompt, &record.record_id);
        let observed = status(frontend.status);
        exact += usize::from(observed == expected.expected_status);
        front_replay += usize::from(replay_verified(&frontend));
        let mut fbad = frontend.clone();
        fbad.replay_hash.push('x');
        front_tamper += usize::from(!replay_verified(&fbad));
        let execution = execute_word_system(&frontend);
        complete += usize::from(execution.is_some());
        if let Some(receipt) = execution {
            if expected.expected_status != "complete" {
                false_auth += 1;
            }
            let expected_hash = expected.answer_sha256.as_ref().expect("hash");
            hashes += 1;
            let matched = sha(receipt.result.as_bytes()) == *expected_hash;
            if matched {
                correct += 1;
            } else {
                incorrect += 1;
            }
            exec_replay += usize::from(execution_replay_verified(&receipt));
            let mut ebad = receipt.clone();
            ebad.result.push('x');
            exec_tamper += usize::from(!execution_replay_verified(&ebad));
        } else if expected.expected_status == "complete" {
            false_denials += 1;
        }
    }
    let after = breadth_first_manifest().replay_hash();
    let report = serde_json::json!({"schema":"stage421-word-system-v2-alignment-result-v1","stage420_commit":"2897c65","corpus_path":CORPUS,"corpus_sha256":CORPUS_SHA,"base_alignment":BASE,"delta_alignment":DELTA,"candidates":candidates.len(),"expected_complete":9,"expected_ambiguous":2,"expected_unsupported":0,"exact_statuses":exact,"complete_executions":complete,"correct_shadow_answers":correct,"incorrect_shadow_answers":incorrect,"false_denials":false_denials,"false_authorizations":false_auth,"frontend_replays":front_replay,"frontend_tamper_rejections":front_tamper,"execution_replays":exec_replay,"execution_tamper_rejections":exec_tamper,"answer_hashes_read":hashes,"plaintext_answers_read":0,"production_authorizations":0,"manifest_unchanged":before==after});
    assert_eq!(report["candidates"], 11);
    assert_eq!(report["exact_statuses"], 11);
    assert_eq!(report["complete_executions"], 9);
    assert_eq!(report["correct_shadow_answers"], 9);
    assert_eq!(report["incorrect_shadow_answers"], 0);
    assert_eq!(report["false_denials"], 0);
    assert_eq!(report["false_authorizations"], 0);
    assert_eq!(report["frontend_replays"], 11);
    assert_eq!(report["frontend_tamper_rejections"], 11);
    assert_eq!(report["execution_replays"], 9);
    assert_eq!(report["execution_tamper_rejections"], 9);
    assert_eq!(report["answer_hashes_read"], 9);
    assert_eq!(report["plaintext_answers_read"], 0);
    assert!(report["manifest_unchanged"].as_bool().unwrap());
    let mut report = report;
    let hash = sha(&serde_json::to_vec(&report)?);
    report["report_sha256"] = serde_json::Value::String(hash);
    fs::write(
        "docs/stage421_word_system_v2_alignment.json",
        format!("{}\n", serde_json::to_string_pretty(&report)?),
    )?;
    fs::write("docs/stage421_word_system_v2_alignment.md",format!("# Stage 421 — V2 external answer alignment\n\n- candidates / complete / ambiguous / unsupported: 11 / 9 / 2 / 0\n- exact statuses / executions: {exact} / {complete}\n- correct / incorrect shadow answers: {correct} / {incorrect}\n- false denials / false authorizations: {false_denials} / {false_auth}\n- frontend replay / tamper: {front_replay} / {front_tamper}\n- execution replay / tamper: {exec_replay} / {exec_tamper}\n- answer hashes / plaintext answers: {hashes} / 0\n- manifest unchanged: {}\n",before==after))?;
    println!("Stage 421 — candidates=11 correct={correct} false_auth={false_auth} false_denials={false_denials}");
    Ok(())
}
