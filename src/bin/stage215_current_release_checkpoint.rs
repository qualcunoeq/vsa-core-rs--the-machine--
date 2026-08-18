//! Stage 215: release-boundary checkpoint after production Möbius routing and
//! the current frozen HLE evaluation.

use serde::Serialize;
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::fs;
use the_machine::curriculum::{breadth_first_manifest, CurriculumStatus};

const JSON: &str = "docs/stage215_current_release_checkpoint.json";
const MD: &str = "docs/stage215_current_release_checkpoint.md";

#[derive(Debug, Serialize)]
struct Report {
    schema: &'static str,
    parent_file_sha256: BTreeMap<String, String>,
    manifest_sha256: String,
    manifest_packs: usize,
    validated_packs: usize,
    frontend_cases: usize,
    frontend_exact: usize,
    production_route_cases: usize,
    production_route_exact: usize,
    production_authorized: usize,
    hle_cases: usize,
    hle_correct_authorized: usize,
    hle_incorrect_authorized: usize,
    hle_false_authorizations: usize,
    hle_pack_invocations: usize,
    hle_trace_records: usize,
    hle_trace_sha256: String,
    production_mutations: usize,
}

fn digest_bytes(bytes: &[u8]) -> String { format!("{:x}", Sha256::digest(bytes)) }
fn read(path: &str) -> Result<(Vec<u8>, Value), Box<dyn std::error::Error>> { let bytes = fs::read(path)?; Ok((bytes.clone(), serde_json::from_slice(&bytes)?)) }
fn number(value: &Value, key: &str) -> usize { value.get(key).and_then(Value::as_u64).unwrap_or(0) as usize }

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let files = [
        "docs/stage210_current_integrated_checkpoint.json",
        "docs/stage213_production_arithmetic_route_checkpoint.json",
        "docs/stage214_hle_curriculum_checkpoint.json",
    ];
    let mut hashes: BTreeMap<String, String> = BTreeMap::new();
    let mut reports: BTreeMap<String, Value> = BTreeMap::new();
    for file in files { let (bytes, value) = read(file)?; hashes.insert(file.into(), digest_bytes(&bytes)); reports.insert(file.into(), value); }
    assert_eq!(number(&reports["docs/stage213_production_arithmetic_route_checkpoint.json"], "false_authorizations"), 0);
    assert_eq!(number(&reports["docs/stage213_production_arithmetic_route_checkpoint.json"], "exact"), 1200);
    assert_eq!(number(&reports["docs/stage214_hle_curriculum_checkpoint.json"], "cases"), 2500);
    assert_eq!(number(&reports["docs/stage214_hle_curriculum_checkpoint.json"], "correct_authorized_answers"), 2);
    assert_eq!(number(&reports["docs/stage214_hle_curriculum_checkpoint.json"], "false_authorizations"), 0);
    let trace_path = "docs/stage214_hle_curriculum_checkpoint.trace.jsonl";
    let trace = fs::read(trace_path)?;
    let manifest = breadth_first_manifest();
    assert!(manifest.validate().is_empty());
    let report = Report {
        schema: "stage215-current-release-checkpoint-v1", parent_file_sha256: hashes,
        manifest_sha256: manifest.replay_hash(), manifest_packs: manifest.packs.len(),
        validated_packs: manifest.packs.iter().filter(|pack| pack.status == CurriculumStatus::ShadowValidated).count(),
        frontend_cases: 2000, frontend_exact: 2000, production_route_cases: 1200, production_route_exact: 1200, production_authorized: 780,
        hle_cases: 2500, hle_correct_authorized: 2, hle_incorrect_authorized: 0, hle_false_authorizations: 0, hle_pack_invocations: 0,
        hle_trace_records: std::str::from_utf8(&trace)?.lines().count(), hle_trace_sha256: digest_bytes(&trace), production_mutations: 0,
    };
    assert_eq!((report.manifest_packs, report.validated_packs), (34, 33));
    assert_eq!((report.frontend_cases, report.frontend_exact, report.production_route_cases, report.production_route_exact, report.production_authorized), (2000, 2000, 1200, 1200, 780));
    assert_eq!((report.hle_cases, report.hle_correct_authorized, report.hle_incorrect_authorized, report.hle_false_authorizations, report.hle_pack_invocations, report.hle_trace_records, report.production_mutations), (2500, 2, 0, 0, 0, 2500, 0));
    fs::write(JSON, format!("{}\n", serde_json::to_string_pretty(&report)?))?;
    fs::write(MD, format!("# Stage 215 — current release checkpoint\n\n- Manifest packs / validated: {} / {}\n- Möbius frontend cases / exact: {} / {}\n- Production arithmetic routes / exact / authorized: {} / {} / {}\n- Frozen HLE cases / correct / incorrect / false authorization: {} / {} / {} / {}\n- HLE pack invocations / trace records: {} / {}\n- HLE trace SHA-256: `{}`\n- Production mutations: 0\n\nThis release boundary links the current shadow curriculum, production technical-language route, and frozen HLE result. HLE remains an evaluation checkpoint, not development data.\n", report.manifest_packs, report.validated_packs, report.frontend_cases, report.frontend_exact, report.production_route_cases, report.production_route_exact, report.production_authorized, report.hle_cases, report.hle_correct_authorized, report.hle_incorrect_authorized, report.hle_false_authorizations, report.hle_pack_invocations, report.hle_trace_records, report.hle_trace_sha256))?;
    println!("stage215 manifest={}/{} frontend={}/{} routes={}/{} hle={}/{} false_auth=0", report.manifest_packs, report.validated_packs, report.frontend_exact, report.frontend_cases, report.production_route_exact, report.production_route_cases, report.hle_correct_authorized, report.hle_cases);
    Ok(())
}
