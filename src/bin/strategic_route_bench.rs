//! Run the contextual concept/strategy planning evaluation.

use std::fs::{create_dir_all, OpenOptions};
use std::io::Write;
use std::path::Path;
use the_machine::strategic_route_benchmark::{evaluate, experiment_results, task_count_for_scale};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = std::env::args().skip(1).collect::<Vec<_>>();
    let mut scale = "small".to_string();
    let mut seed = 42_u64;
    let mut out = "/tmp/strategic_route_bench.jsonl".to_string();
    let mut commit = "unknown".to_string();
    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--scale" => {
                i += 1;
                scale = args.get(i).ok_or("--scale needs a value")?.clone();
            }
            "--seed" => {
                i += 1;
                seed = args.get(i).ok_or("--seed needs a value")?.parse::<u64>()?;
            }
            "--out" => {
                i += 1;
                out = args.get(i).ok_or("--out needs a value")?.clone();
            }
            "--commit" => {
                i += 1;
                commit = args.get(i).ok_or("--commit needs a value")?.clone();
            }
            "--help" | "-h" => {
                println!("usage: strategic_route_bench [--scale small|medium|large] [--seed N] [--out PATH] [--commit SHA]");
                return Ok(());
            }
            other => return Err(format!("unknown argument '{other}'").into()),
        }
        i += 1;
    }
    let count = task_count_for_scale(&scale)?;
    let report = evaluate(seed, count);
    if let Some(parent) = Path::new(&out).parent() {
        if !parent.as_os_str().is_empty() {
            create_dir_all(parent)?;
        }
    }
    let mut file = OpenOptions::new().create(true).append(true).open(&out)?;
    let results = experiment_results(&report, commit);
    for result in &results {
        writeln!(file, "{}", serde_json::to_string(&result)?)?;
    }
    for mode in report.modes.values() {
        eprintln!(
            "{}: accuracy={:.3} abstentions={} mean_steps={:.2}",
            mode.mode.label(),
            mode.accuracy,
            mode.abstentions,
            mode.mean_route_steps
        );
    }
    eprintln!(
        "wrote {} results ({} modes + receipt shadow) for {} tasks to {}",
        results.len(),
        report.modes.len(),
        report.task_count,
        out
    );
    Ok(())
}
