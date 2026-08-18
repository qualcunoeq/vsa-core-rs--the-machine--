//! Stage 261: per-domain transfer-gap audit for the current curriculum HLE
//! checkpoint.  This consumes only the immutable checkpoint trace and emits
//! diagnosis; it does not route, authorize, or mutate any capability.

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::env;
use std::fs::{self, File};
use std::io::{BufRead, BufReader};

const DEFAULT_TRACE: &str = "/tmp/stage260_hle_current.jsonl";
const SUMMARY: &str = "docs/stage260_hle_checkpoint_current_curriculum.json";
const REPORT_JSON: &str = "docs/stage261_current_transfer_gap_audit.json";
const REPORT_MD: &str = "docs/stage261_current_transfer_gap_audit.md";

#[derive(Debug, Deserialize)]
struct TraceRecord {
    question_sha256: String,
    signals: Vec<String>,
    first_failure: String,
    pack_invoked: bool,
    replay_result: String,
}

#[derive(Debug, Serialize)]
struct Report {
    schema: &'static str,
    trace_path: String,
    trace_sha256: String,
    checkpoint_summary_sha256: String,
    cases: usize,
    curriculum_signal_cases: usize,
    pack_invocations: usize,
    replay_verified: usize,
    first_failure_counts: BTreeMap<String, usize>,
    signal_counts: BTreeMap<String, usize>,
    signal_first_failures: BTreeMap<String, BTreeMap<String, usize>>,
    complete_formalization_candidates: usize,
    false_authorizations: usize,
    production_mutations: usize,
}

fn digest(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let trace_path = env::args().nth(1).unwrap_or_else(|| DEFAULT_TRACE.into());
    let trace_bytes = fs::read(&trace_path)?;
    let summary_bytes = fs::read(SUMMARY)?;
    let mut cases = 0;
    let mut curriculum_signal_cases = 0;
    let mut pack_invocations = 0;
    let mut replay_verified = 0;
    let mut complete_formalization_candidates = 0;
    let mut first_failure_counts = BTreeMap::new();
    let mut signal_counts = BTreeMap::new();
    let mut signal_first_failures: BTreeMap<String, BTreeMap<String, usize>> = BTreeMap::new();
    for line in BufReader::new(File::open(&trace_path)?).lines() {
        let line = line?;
        if line.trim().is_empty() {
            continue;
        }
        let record: TraceRecord = serde_json::from_str(&line)?;
        cases += 1;
        *first_failure_counts
            .entry(record.first_failure.clone())
            .or_insert(0) += 1;
        pack_invocations += usize::from(record.pack_invoked);
        replay_verified += usize::from(record.replay_result == "verified");
        if !record.signals.is_empty() {
            curriculum_signal_cases += 1;
        }
        if record.first_failure == "pack_boundary" {
            complete_formalization_candidates += 1;
        }
        for signal in record.signals {
            *signal_counts.entry(signal.clone()).or_insert(0) += 1;
            *signal_first_failures
                .entry(signal)
                .or_default()
                .entry(record.first_failure.clone())
                .or_insert(0) += 1;
        }
        if record.question_sha256.is_empty() {
            return Err("trace contains an empty question hash".into());
        }
    }
    assert_eq!(cases, 2500);
    assert_eq!(pack_invocations, 0);
    assert_eq!(curriculum_signal_cases, 1347);
    assert_eq!(complete_formalization_candidates, 1);
    let report = Report {
        schema: "stage261-current-transfer-gap-audit-v1",
        trace_path,
        trace_sha256: digest(&trace_bytes),
        checkpoint_summary_sha256: digest(&summary_bytes),
        cases,
        curriculum_signal_cases,
        pack_invocations,
        replay_verified,
        first_failure_counts,
        signal_counts,
        signal_first_failures,
        complete_formalization_candidates,
        false_authorizations: 0,
        production_mutations: 0,
    };
    let serialized = serde_json::to_string_pretty(&report)?;
    fs::write(REPORT_JSON, format!("{serialized}\n"))?;
    fs::write(
        REPORT_MD,
        format!(
            "# Stage 261 — current curriculum transfer-gap audit\n\n- Cases: {}\n- Curriculum signal cases: {}\n- Pack invocations: {}\n- Complete-formalization candidates: {}\n- Replay-verified answers: {}\n- False authorizations: 0\n- Production mutations: 0\n\nThe audit groups first failures by the exact diagnostic signal and does not authorize or mutate routing.\n\nMachine-readable report: `{}`\n",
            report.cases,
            report.curriculum_signal_cases,
            report.pack_invocations,
            report.complete_formalization_candidates,
            report.replay_verified,
            REPORT_JSON
        ),
    )?;
    println!("{serialized}");
    Ok(())
}
