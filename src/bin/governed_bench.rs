//! Run the unified tiered governed-reasoning benchmark.

use std::fs::{create_dir_all, OpenOptions};
use std::io::Write;
use std::path::Path;
use std::time::Instant;
use the_machine::cognition::ExperimentResult;
use the_machine::governed_benchmark::{evaluate, experiment_results};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = std::env::args().skip(1).collect::<Vec<_>>();
    let generated = args
        .first()
        .map(|value| value.parse::<usize>())
        .transpose()?
        .unwrap_or(500);
    let strategic = args
        .get(1)
        .map(|value| value.parse::<usize>())
        .transpose()?
        .unwrap_or(500);
    let seed = args
        .get(2)
        .map(|value| value.parse::<u64>())
        .transpose()?
        .unwrap_or(42);
    let out = args
        .get(3)
        .cloned()
        .unwrap_or_else(|| "/tmp/governed_bench.jsonl".into());
    let commit = args.get(4).cloned().unwrap_or_else(|| "unknown".into());
    let started = Instant::now();
    let report = evaluate(seed, generated, strategic);
    let elapsed_ms = started.elapsed().as_secs_f64() * 1_000.0;
    if let Some(parent) = Path::new(&out).parent() {
        if !parent.as_os_str().is_empty() {
            create_dir_all(parent)?;
        }
    }
    let mut file = OpenOptions::new().create(true).append(true).open(&out)?;
    for result in experiment_results(&report, commit.clone()) {
        writeln!(file, "{}", serde_json::to_string(&result)?)?;
    }
    let mut runtime_metrics = std::collections::HashMap::new();
    runtime_metrics.insert("elapsed_ms".into(), elapsed_ms);
    runtime_metrics.insert("tier_count".into(), report.tiers.len() as f64);
    writeln!(
        file,
        "{}",
        serde_json::to_string(&ExperimentResult {
            experiment: "governed_suite_runtime".into(),
            claim: "record benchmark runtime for measured operational hardening".into(),
            commit: commit.clone(),
            seed,
            dataset: Some("unified_governed_suite".into()),
            baseline: "release benchmark runner".into(),
            metrics: runtime_metrics,
            passed: true,
            notes: "measurement only; no runtime SLO is asserted".into(),
        })?
    )?;
    for tier in report.tiers.values() {
        eprintln!(
            "{}: cases={} success={:.3} positive_success={:.3} replay={:.3} positive_replay={:.3} false_auth={} false_denials={}",
            tier.tier,
            tier.cases,
            tier.success_rate,
            tier.positive_success_rate,
            tier.replay_rate,
            tier.positive_replay_rate,
            tier.false_authorizations,
            tier.false_denials,
        );
    }
    for ablation in &report.ablations {
        eprintln!("ablation {}: {}", ablation.name, ablation.status);
    }
    eprintln!("governed_suite_runtime: elapsed_ms={elapsed_ms:.3}");
    eprintln!("wrote governed benchmark results to {out}");
    Ok(())
}
