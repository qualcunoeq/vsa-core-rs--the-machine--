//! Run the structured formalization benchmark over a versioned gold corpus.

use std::fs::{create_dir_all, OpenOptions};
use std::io::Write;
use std::path::Path;
use the_machine::formalization::FormalizationCorpus;
use the_machine::formalization_benchmark::{evaluate, experiment_results};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = std::env::args().skip(1).collect::<Vec<_>>();
    let corpus_path = args
        .first()
        .cloned()
        .ok_or("usage: formalization_bench <corpus.json> [out.jsonl] [commit]")?;
    let out = args
        .get(1)
        .cloned()
        .unwrap_or_else(|| "/tmp/formalization_bench.jsonl".into());
    let commit = args.get(2).cloned().unwrap_or_else(|| "unknown".into());
    let corpus: FormalizationCorpus = serde_json::from_str(&std::fs::read_to_string(&corpus_path)?)?;
    let errors = corpus.validation_errors();
    if !errors.is_empty() {
        return Err(format!("formalization corpus validation failed: {errors:?}").into());
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
            "{}: cases={} structural={:.3} complete={:.3} auth={:.3} false_denials={} coverage={:.3}",
            group.group,
            group.cases,
            group.structural_target_accuracy,
            group.target_complete_rate,
            group.authorization_accuracy,
            group.false_denials,
            group.failure_taxonomy.classification_coverage
        );
    }
    eprintln!("wrote {} groups to {}", report.groups.len(), out);
    Ok(())
}
