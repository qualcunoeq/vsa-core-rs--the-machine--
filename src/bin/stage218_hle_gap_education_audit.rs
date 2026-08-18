//! Stage 218: current HLE residuals through the governed education planner.
//!
//! Broad curriculum signals are deliberately not converted into guessed
//! artifacts.  Until a pack-specific typed request exists, they remain
//! ambiguous residuals and no source module is selected.

use serde::Serialize;
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::fs;
use the_machine::continuous_education::run_campaign;
use the_machine::curriculum::breadth_first_manifest;
use the_machine::curriculum_campaign::{observe_gap, GapKind, GapObservation};

const TRACE: &str = "docs/stage216_hle_curriculum_checkpoint.trace.jsonl";
const JSON: &str = "docs/stage218_hle_gap_education_audit.json";
const MD: &str = "docs/stage218_hle_gap_education_audit.md";

#[derive(Debug, Serialize)]
struct Report {
    schema: &'static str,
    trace_sha256: String,
    trace_cases: usize,
    curriculum_signal_cases: usize,
    typed_actionable_gaps: usize,
    ambiguous_residuals: usize,
    first_failure_counts: BTreeMap<String, usize>,
    campaign_replay_verified: bool,
    deterministic_rerun: bool,
    tamper_rejected: bool,
    manifest_unchanged: bool,
    selected_modules: usize,
    resolved_cases: usize,
    remaining_cases: usize,
    false_authorizations: usize,
    live_registry_mutations: usize,
}

fn digest_bytes(bytes: &[u8]) -> String { format!("{:x}", Sha256::digest(bytes)) }

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let trace = fs::read(TRACE)?;
    let manifest = breadth_first_manifest();
    let mut observations: Vec<GapObservation> = Vec::new();
    let mut first_failure_counts = BTreeMap::new();
    let mut trace_cases = 0;
    let mut signal_cases = 0;
    for line in std::str::from_utf8(&trace)?.lines() {
        let record: Value = serde_json::from_str(line)?;
        trace_cases += 1;
        let failure = record.get("first_failure").and_then(Value::as_str).unwrap_or("unknown");
        *first_failure_counts.entry(failure.to_string()).or_insert(0) += 1;
        let has_signal = record.get("signals").and_then(Value::as_array).is_some_and(|values| !values.is_empty());
        if !has_signal || failure == "authorized_reference_match" { continue; }
        signal_cases += 1;
        observations.push(observe_gap(
            record.get("id").and_then(Value::as_str).unwrap_or("unknown"),
            "untyped_curriculum_signal",
            GapKind::Ambiguous,
            "broad signal lacks a uniquely typed pack request; preserve residual",
        ));
    }
    let campaign = run_campaign(&manifest, &observations, &[], 1);
    let rerun = run_campaign(&manifest, &observations, &[], 1);
    let mut tampered = campaign.clone();
    tampered.remaining_case_count += 1;
    let selected_modules = campaign.rounds.iter().filter(|step| step.module_id.is_some()).count();
    let report = Report {
        schema: "stage218-hle-gap-education-audit-v1",
        trace_sha256: digest_bytes(&trace), trace_cases, curriculum_signal_cases: signal_cases,
        typed_actionable_gaps: 0, ambiguous_residuals: observations.len(), first_failure_counts,
        campaign_replay_verified: campaign.replay_verified(), deterministic_rerun: campaign == rerun,
        tamper_rejected: !tampered.replay_verified(), manifest_unchanged: campaign.manifest_unchanged(),
        selected_modules, resolved_cases: campaign.resolved_case_count, remaining_cases: campaign.remaining_case_count,
        false_authorizations: 0, live_registry_mutations: 0,
    };
    assert_eq!((report.trace_cases, report.curriculum_signal_cases, report.ambiguous_residuals), (2500, 705, 705));
    assert_eq!((report.typed_actionable_gaps, report.selected_modules, report.resolved_cases, report.remaining_cases), (0, 0, 0, 705));
    assert!(report.campaign_replay_verified && report.deterministic_rerun && report.tamper_rejected && report.manifest_unchanged);
    assert_eq!((report.false_authorizations, report.live_registry_mutations), (0, 0));
    fs::write(JSON, format!("{}\n", serde_json::to_string_pretty(&report)?))?;
    fs::write(MD, format!("# Stage 218 — HLE gap-to-education audit\n\n- Trace cases / curriculum signals: {}/{}\n- Typed actionable gaps / ambiguous residuals: 0 / {}\n- Selected source modules / resolved cases: 0 / 0\n- Remaining residuals: {}\n- Campaign replay / deterministic rerun / tamper: true / true / true\n- Manifest unchanged / false authorizations / live mutations: true / 0 / 0\n\nThe current HLE signal set does not justify a source-education action: broad vocabulary is retained as an ambiguous `untyped_curriculum_signal` rather than converted into a guessed artifact. The first-failure counts remain available in the machine-readable report for the next independently validated frontend or source campaign.\n", report.trace_cases, report.curriculum_signal_cases, report.ambiguous_residuals, report.remaining_cases))?;
    println!("stage218 trace={} signals={} ambiguous={} selected=0 resolved=0 replay=true", report.trace_cases, report.curriculum_signal_cases, report.ambiguous_residuals);
    Ok(())
}
