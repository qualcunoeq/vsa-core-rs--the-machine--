//! Run the blind mixed-domain integration benchmark.

use std::fs;
use the_machine::mixed_ood_benchmark::{evaluate, MixedOodCorpus};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = std::env::args().skip(1).collect::<Vec<_>>();
    let input = args
        .first()
        .cloned()
        .unwrap_or_else(|| "data/mixed_ood_v1.json".into());
    let output = args
        .get(1)
        .cloned()
        .unwrap_or_else(|| "/tmp/mixed_ood_v1_report.json".into());
    let corpus: MixedOodCorpus = serde_json::from_str(&fs::read_to_string(&input)?)?;
    let errors = corpus.validation_errors();
    if !errors.is_empty() {
        return Err(format!("invalid mixed OOD corpus: {errors:?}").into());
    }
    let report = evaluate(&corpus);
    fs::write(&output, serde_json::to_vec_pretty(&report)?)?;
    let metrics = &report.metrics;
    eprintln!(
        "mixed-ood: cases={} route={:.3} formalized={:.3} decisions={:.3} authorized={} replay={} false_auth={} false_denials={}",
        report.corpus_cases,
        metrics.route_correct as f64 / metrics.cases.max(1) as f64,
        metrics.formalized as f64 / metrics.cases.max(1) as f64,
        metrics.correct_decisions as f64 / metrics.cases.max(1) as f64,
        metrics.authorized,
        metrics.replay_successes,
        metrics.false_authorizations,
        metrics.false_denials,
    );
    eprintln!(
        "rewrites: pairs={} route_stable={} decision_stable={} answer_stable={} regressions={}",
        report.rewrites.pairs,
        report.rewrites.route_stable,
        report.rewrites.decision_stable,
        report.rewrites.answer_stable,
        report.rewrites.regressions,
    );
    eprintln!("route_confusion={:?}", report.route_confusion);
    eprintln!("failure_taxonomy={:?}", metrics.failure_taxonomy);
    eprintln!("wrote {output}");
    Ok(())
}
