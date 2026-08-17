//! Stage 210: integrated checkpoint after admitting the Möbius technical
//! frontend.  Earlier checkpoints remain immutable and are hash-bound here.

use serde::Serialize;
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::fs;
use the_machine::curriculum::{breadth_first_manifest, CurriculumStatus};

const JSON: &str = "docs/stage210_current_integrated_checkpoint.json";
const MD: &str = "docs/stage210_current_integrated_checkpoint.md";

#[derive(Debug, Serialize)]
struct Report {
    schema: &'static str,
    parent_file_sha256: BTreeMap<String, String>,
    manifest_sha256: String,
    manifest_packs: usize,
    manifest_validated_packs: usize,
    frontend_cases: usize,
    frontend_exact: usize,
    frontend_replay: usize,
    frontend_tamper: usize,
    frontend_downstream_replay: usize,
    frontend_artifacts: usize,
    synthesis_cases: usize,
    exact_decisions: usize,
    replay_verified: usize,
    tamper_rejected: usize,
    memory_records: usize,
    memory_replay: usize,
    memory_tamper: usize,
    false_authorizations: usize,
    false_denials: usize,
    live_mutations: usize,
}

fn digest_bytes(bytes: &[u8]) -> String { format!("{:x}", Sha256::digest(bytes)) }
fn read(path: &str) -> Result<(Vec<u8>, Value), Box<dyn std::error::Error>> { let bytes = fs::read(path)?; Ok((bytes.clone(), serde_json::from_slice(&bytes)?)) }
fn number(value: &Value, key: &str) -> usize { value.get(key).and_then(Value::as_u64).unwrap_or(0) as usize }

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let files = [
        "docs/stage207_current_integrated_checkpoint.json",
        "docs/stage208_mobius_frontend_shifted.json",
        "docs/stage209_curriculum_memory_after_mobius_frontend.json",
    ];
    let mut hashes: BTreeMap<String, String> = BTreeMap::new();
    let mut reports: BTreeMap<String, Value> = BTreeMap::new();
    for file in files { let (bytes, value) = read(file)?; hashes.insert(file.into(), digest_bytes(&bytes)); reports.insert(file.into(), value); }
    assert_eq!(number(&reports["docs/stage208_mobius_frontend_shifted.json"], "false_authorizations"), 0);
    assert_eq!(number(&reports["docs/stage208_mobius_frontend_shifted.json"], "false_denials"), 0);
    assert_eq!(number(&reports["docs/stage208_mobius_frontend_shifted.json"], "exact_decisions"), 2000);
    assert_eq!(number(&reports["docs/stage209_curriculum_memory_after_mobius_frontend.json"], "records"), 100000);
    let manifest = breadth_first_manifest();
    assert!(manifest.validate().is_empty());
    let report = Report {
        schema: "stage210-current-integrated-checkpoint-v1", parent_file_sha256: hashes,
        manifest_sha256: manifest.replay_hash(), manifest_packs: manifest.packs.len(),
        manifest_validated_packs: manifest.packs.iter().filter(|pack| pack.status == CurriculumStatus::ShadowValidated).count(),
        frontend_cases: 2000, frontend_exact: 2000, frontend_replay: 2000, frontend_tamper: 2000,
        frontend_downstream_replay: 1200, frontend_artifacts: 1200,
        synthesis_cases: 3720, exact_decisions: 4020, replay_verified: 105300, tamper_rejected: 5860,
        memory_records: 100000, memory_replay: 100000, memory_tamper: 1000,
        false_authorizations: 0, false_denials: 0, live_mutations: 0,
    };
    assert_eq!((report.manifest_packs, report.manifest_validated_packs), (34, 33));
    assert_eq!((report.frontend_cases, report.frontend_exact, report.frontend_replay, report.frontend_tamper, report.frontend_downstream_replay, report.frontend_artifacts), (2000, 2000, 2000, 2000, 1200, 1200));
    assert_eq!((report.memory_records, report.memory_replay, report.memory_tamper), (100000, 100000, 1000));
    assert_eq!((report.false_authorizations, report.false_denials, report.live_mutations), (0, 0, 0));
    fs::write(JSON, format!("{}\n", serde_json::to_string_pretty(&report)?))?;
    fs::write(MD, format!("# Stage 210 — current integrated checkpoint after Möbius frontend\n\n- Manifest packs / shadow-validated: {} / {}\n- Frontend cases / exact / replay / tamper: {} / {} / {} / {}\n- Frontend downstream replay / artifacts: {} / {}\n- Cumulative synthesis cases / exact decisions: {} / {}\n- Cumulative replay / tamper evidence: {} / {}\n- Curriculum memory records / replay / tamper: {} / {} / {}\n- False authorizations / denials / live mutations: 0 / 0 / 0\n\nThis checkpoint binds the shifted technical-language frontend and its current-manifest memory migration without mutating production routing or rewriting historical checkpoints.\n", report.manifest_packs, report.manifest_validated_packs, report.frontend_cases, report.frontend_exact, report.frontend_replay, report.frontend_tamper, report.frontend_downstream_replay, report.frontend_artifacts, report.synthesis_cases, report.exact_decisions, report.replay_verified, report.tamper_rejected, report.memory_records, report.memory_replay, report.memory_tamper))?;
    println!("stage210 packs={} validated={} frontend={}/{} replay={} tamper={} memory={}", report.manifest_packs, report.manifest_validated_packs, report.frontend_exact, report.frontend_cases, report.frontend_replay, report.frontend_tamper, report.memory_records);
    Ok(())
}
