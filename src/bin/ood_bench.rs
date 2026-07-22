//! Run the independent out-of-distribution governed algebra audit.

use std::fs;
use the_machine::ood_benchmark::{evaluate, OodCorpus};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = std::env::args().skip(1).collect::<Vec<_>>();
    let input = args
        .first()
        .cloned()
        .unwrap_or_else(|| "data/algebra_ood_v1.json".into());
    let output = args
        .get(1)
        .cloned()
        .unwrap_or_else(|| "/tmp/algebra_ood_v1_report.json".into());
    let corpus: OodCorpus = serde_json::from_str(&fs::read_to_string(&input)?)?;
    let errors = corpus.validation_errors();
    if !errors.is_empty() {
        return Err(format!("invalid OOD corpus: {errors:?}").into());
    }
    let report = evaluate(&corpus);
    fs::write(&output, serde_json::to_vec_pretty(&report)?)?;
    let metrics = &report.metrics;
    eprintln!(
        "ood: cases={} independent={} variants={} decision={:.3} result={:.3} formalized={:.3} replay={:.3} false_auth={} false_denials={}",
        report.corpus_cases,
        report.independent_cases,
        report.variant_cases,
        metrics.correct_decisions as f64 / metrics.cases.max(1) as f64,
        metrics.correct_results as f64 / metrics.cases.max(1) as f64,
        metrics.formalized as f64 / metrics.cases.max(1) as f64,
        metrics.replay_successes as f64 / metrics.execution_successes.max(1) as f64,
        metrics.false_authorizations,
        metrics.false_denials,
    );
    eprintln!(
        "invariance: pairs={} decision_stable={} canonical_stable={} result_stable={} regressions={}",
        report.invariance.pairs,
        report.invariance.decision_stable,
        report.invariance.canonical_stable,
        report.invariance.result_stable,
        report.invariance.rewrite_regressions,
    );
    eprintln!("refusal_taxonomy={:?}", metrics.refusal_taxonomy);
    eprintln!("divergence_stages={:?}", report.divergence_stages);
    eprintln!("wrote {output}");
    Ok(())
}
