//! Run the unified tiered governed-reasoning benchmark.

use std::fs::{create_dir_all, OpenOptions};
use std::io::Write;
use std::path::Path;
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
    let report = evaluate(seed, generated, strategic);
    if let Some(parent) = Path::new(&out).parent() {
        if !parent.as_os_str().is_empty() {
            create_dir_all(parent)?;
        }
    }
    let mut file = OpenOptions::new().create(true).append(true).open(&out)?;
    for result in experiment_results(&report, commit) {
        writeln!(file, "{}", serde_json::to_string(&result)?)?;
    }
    for tier in report.tiers.values() {
        eprintln!(
            "{}: cases={} success={:.3} replay={:.3} false_auth={} false_denials={}",
            tier.tier,
            tier.cases,
            tier.success_rate,
            tier.replay_rate,
            tier.false_authorizations,
            tier.false_denials,
        );
    }
    for ablation in &report.ablations {
        eprintln!("ablation {}: {}", ablation.name, ablation.status);
    }
    eprintln!("wrote governed benchmark results to {out}");
    Ok(())
}
