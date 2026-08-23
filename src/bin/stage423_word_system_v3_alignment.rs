//! Privileged hash-only alignment for the V3 multiplier extension.

use serde::Deserialize;
use sha2::{Digest, Sha256};
use std::{collections::BTreeMap, env, fs};
use the_machine::curriculum::breadth_first_manifest;
use the_machine::source_word_system_frontend::{
    execute_word_system, execution_replay_verified, formalize_two_number_system_v3, replay_verified,
};

const CORPUS: &str = "docs/stage389_page_aware_external_problem_dev.json";
const CORPUS_SHA: &str = "92304d95521ac2ef4d49a08272f0b0cf79ac7225c9715d5ace9a667c822f6e03";
const BASE: &str = "docs/stage418_word_system_alignment.json";
const DELTA_V2: &str = "docs/stage421_word_system_v2_alignment_delta.json";
const DELTA_V3: &str = "docs/stage423_word_system_v3_alignment_delta.json";
#[derive(Deserialize)]
struct External {
    record_id: String,
    prompt: String,
    source_sha256: String,
    #[serde(default)]
    quality_flags: Vec<String>,
    answer_key_status: String,
}
#[derive(Deserialize)]
struct Alignment {
    record_id: String,
    expected_status: String,
    answer_sha256: Option<String>,
    source_sha256: String,
}
#[derive(Deserialize)]
struct FileData {
    records: Vec<Alignment>,
    corpus_sha256: String,
    plaintext_answers_stored: bool,
}
fn sha(b: &[u8]) -> String {
    format!("{:x}", Sha256::digest(b))
}
fn cand(p: &str) -> bool {
    let p = p.to_ascii_lowercase();
    p.contains("sum of two number") && p.contains("one number is") && p.contains("find")
}
fn status(s: the_machine::source_word_system_frontend::WordSystemStatus) -> &'static str {
    match s {
        the_machine::source_word_system_frontend::WordSystemStatus::Complete => "complete",
        the_machine::source_word_system_frontend::WordSystemStatus::Ambiguous => "ambiguous",
        the_machine::source_word_system_frontend::WordSystemStatus::Missing => "missing",
        the_machine::source_word_system_frontend::WordSystemStatus::Unsupported => "unsupported",
    }
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    assert_eq!(env::var("STAGE423_PRIVILEGED_EVAL").as_deref(), Ok("true"));
    let bytes = fs::read(CORPUS)?;
    assert_eq!(sha(&bytes), CORPUS_SHA);
    let all: Vec<External> = serde_json::from_slice(&bytes)?;
    assert!(all.iter().all(|r| r.answer_key_status == "not_read"));
    let cs = all
        .into_iter()
        .filter(|r| r.quality_flags.is_empty() && cand(&r.prompt))
        .collect::<Vec<_>>();
    let base: FileData = serde_json::from_str(&fs::read_to_string(BASE)?)?;
    let d2: FileData = serde_json::from_str(&fs::read_to_string(DELTA_V2)?)?;
    let d3: FileData = serde_json::from_str(&fs::read_to_string(DELTA_V3)?)?;
    assert_eq!(base.corpus_sha256, CORPUS_SHA);
    assert_eq!(d2.corpus_sha256, CORPUS_SHA);
    assert_eq!(d3.corpus_sha256, CORPUS_SHA);
    assert!(
        !base.plaintext_answers_stored
            && !d2.plaintext_answers_stored
            && !d3.plaintext_answers_stored
    );
    let mut a = base
        .records
        .into_iter()
        .map(|r| (r.record_id.clone(), r))
        .collect::<BTreeMap<_, _>>();
    for r in d2.records.into_iter().chain(d3.records) {
        a.insert(r.record_id.clone(), r);
    }
    assert_eq!(a.len(), cs.len());
    let before = breadth_first_manifest().replay_hash();
    let mut exact = 0;
    let mut complete = 0;
    let mut correct = 0;
    let mut incorrect = 0;
    let mut fd = 0;
    let mut fa = 0;
    let mut fr = 0;
    let mut ft = 0;
    let mut er = 0;
    let mut et = 0;
    let mut hashes = 0;
    for r in &cs {
        let e = a.get(&r.record_id).unwrap();
        assert_eq!(e.source_sha256, r.source_sha256);
        let f = formalize_two_number_system_v3(&r.prompt, &r.record_id);
        exact += usize::from(status(f.status) == e.expected_status);
        fr += usize::from(replay_verified(&f));
        let mut fb = f.clone();
        fb.replay_hash.push('x');
        ft += usize::from(!replay_verified(&fb));
        if let Some(x) = execute_word_system(&f) {
            complete += 1;
            if e.expected_status != "complete" {
                fa += 1;
            }
            let h = e.answer_sha256.as_ref().unwrap();
            hashes += 1;
            if sha(x.result.as_bytes()) == *h {
                correct += 1;
            } else {
                incorrect += 1;
            }
            er += usize::from(execution_replay_verified(&x));
            let mut xb = x.clone();
            xb.result.push('x');
            et += usize::from(!execution_replay_verified(&xb));
        } else if e.expected_status == "complete" {
            fd += 1;
        }
    }
    let after = breadth_first_manifest().replay_hash();
    let report = serde_json::json!({"schema":"stage423-word-system-v3-alignment-result-v1","corpus_sha256":CORPUS_SHA,"base_alignment":BASE,"delta_v2":DELTA_V2,"delta_v3":DELTA_V3,"candidates":cs.len(),"expected_complete":10,"expected_ambiguous":1,"exact_statuses":exact,"complete_executions":complete,"correct_shadow_answers":correct,"incorrect_shadow_answers":incorrect,"false_denials":fd,"false_authorizations":fa,"frontend_replays":fr,"frontend_tamper_rejections":ft,"execution_replays":er,"execution_tamper_rejections":et,"answer_hashes_read":hashes,"plaintext_answers_read":0,"production_authorizations":0,"manifest_unchanged":before==after});
    assert_eq!(report["candidates"], 11);
    assert_eq!(report["exact_statuses"], 11);
    assert_eq!(report["complete_executions"], 10);
    assert_eq!(report["correct_shadow_answers"], 10);
    assert_eq!(report["incorrect_shadow_answers"], 0);
    assert_eq!(report["false_denials"], 0);
    assert_eq!(report["false_authorizations"], 0);
    assert_eq!(report["frontend_replays"], 11);
    assert_eq!(report["frontend_tamper_rejections"], 11);
    assert_eq!(report["execution_replays"], 10);
    assert_eq!(report["execution_tamper_rejections"], 10);
    assert_eq!(report["answer_hashes_read"], 10);
    assert!(report["manifest_unchanged"].as_bool().unwrap());
    let mut report = report;
    report["report_sha256"] = serde_json::Value::String(sha(&serde_json::to_vec(&report)?));
    fs::write(
        "docs/stage423_word_system_v3_alignment.json",
        format!("{}\n", serde_json::to_string_pretty(&report)?),
    )?;
    fs::write("docs/stage423_word_system_v3_alignment.md",format!("# Stage 423 — V3 external answer alignment\n\n- candidates / complete / ambiguous: 11 / 10 / 1\n- exact statuses / executions: {exact} / {complete}\n- correct / incorrect shadow answers: {correct} / {incorrect}\n- false denials / false authorizations: {fd} / {fa}\n- frontend replay / tamper: {fr} / {ft}\n- execution replay / tamper: {er} / {et}\n- answer hashes / plaintext answers: {hashes} / 0\n- manifest unchanged: {}\n",before==after))?;
    println!("Stage 423 — candidates=11 correct={correct} false_auth={fa} false_denials={fd}");
    Ok(())
}
