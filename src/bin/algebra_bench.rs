//! Run the bounded algebra execution benchmark.

use std::fs::{create_dir_all, OpenOptions};
use std::io::Write;
use std::path::Path;
use the_machine::algebra_benchmark::{evaluate, experiment_results, AlgebraCorpus};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = std::env::args().skip(1).collect::<Vec<_>>();
    let corpus_path = args
        .first()
        .cloned()
        .ok_or("usage: algebra_bench <corpus.json> [out.jsonl] [commit]")?;
    let out = args
        .get(1)
        .cloned()
        .unwrap_or_else(|| "/tmp/algebra_bench.jsonl".into());
    let commit = args.get(2).cloned().unwrap_or_else(|| "unknown".into());
    let corpus: AlgebraCorpus = serde_json::from_str(&std::fs::read_to_string(&corpus_path)?)?;
    let errors = corpus.validation_errors();
    if !errors.is_empty() {
        return Err(format!("algebra corpus validation failed: {errors:?}").into());
    }
    let report = evaluate(&corpus);
    if let Some(parent) = Path::new(&out).parent() {
        if !parent.as_os_str().is_empty() {
            create_dir_all(parent)?;
        }
    }
    let mut file = OpenOptions::new().create(true).append(true).open(&out)?;
    for result in experiment_results(&report, corpus_path, commit) {
        writeln!(file, "{}", serde_json::to_string(&result)?)?;
    }
    for group in report.groups.values() {
        eprintln!(
            "{}: cases={} accuracy={:.3} formalization={:.3} execution={:.3} replay={:.3} false_auth={} false_denials={}",
            group.group,
            group.cases,
            group.solution_accuracy,
            group.formalization_success_rate,
            group.execution_success_rate,
            group.replay_success_rate,
            group.false_authorizations,
            group.false_denials,
        );
    }
    eprintln!("wrote {} groups to {}", report.groups.len(), out);
    Ok(())
}
