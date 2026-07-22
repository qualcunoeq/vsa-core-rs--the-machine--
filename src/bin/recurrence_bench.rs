//! Run the deterministic bounded recurrence benchmark.

use std::fs::{create_dir_all, OpenOptions};
use std::io::Write;
use std::path::Path;
use the_machine::recurrence_benchmark::{evaluate, experiment_results};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = std::env::args().skip(1).collect::<Vec<_>>();
    let count = args
        .first()
        .map(|value| value.parse::<usize>())
        .transpose()?
        .unwrap_or(500);
    let seed = args
        .get(1)
        .map(|value| value.parse::<u64>())
        .transpose()?
        .unwrap_or(42);
    let out = args
        .get(2)
        .cloned()
        .unwrap_or_else(|| "/tmp/recurrence_bench.jsonl".into());
    let commit = args.get(3).cloned().unwrap_or_else(|| "unknown".into());
    let report = evaluate(count, seed);
    if let Some(parent) = Path::new(&out).parent() {
        if !parent.as_os_str().is_empty() {
            create_dir_all(parent)?;
        }
    }
    let mut file = OpenOptions::new().create(true).append(true).open(&out)?;
    for result in experiment_results(&report, commit) {
        writeln!(file, "{}", serde_json::to_string(&result)?)?;
    }
    for (name, metrics) in [
        ("total", &report.total),
        ("development", &report.development),
        ("holdout", &report.holdout),
    ] {
        eprintln!(
            "{name}: cases={} expected_authorized={} authorized={} replay={} false_auth={} false_denials={} failures={:?}",
            metrics.cases,
            metrics.expected_authorized,
            metrics.authorized,
            metrics.replay_verified,
            metrics.false_authorizations,
            metrics.false_denials,
            metrics.failure_taxonomy,
        );
    }
    eprintln!("wrote recurrence benchmark results to {out}");
    Ok(())
}
