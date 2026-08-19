use std::env;
use std::fs;

use the_machine::cross_vertical_benchmark::{evaluate, CompositionCorpus};

fn main() {
    let path = env::args()
        .nth(1)
        .unwrap_or_else(|| "data/cross_vertical_ood_v1.json".to_string());
    let corpus: CompositionCorpus = serde_json::from_str(
        &fs::read_to_string(&path).unwrap_or_else(|error| panic!("read {path}: {error}")),
    )
    .unwrap_or_else(|error| panic!("parse {path}: {error}"));
    assert!(corpus.validation_errors().is_empty());
    let report = evaluate(&corpus);
    println!("cross-vertical: cases={} authorized={} correct_decisions={} false_auth={} false_denials={} intermediate_replay={} final_replay={} forged_rejected={} incompatible_rejected={} regressions={}", report.corpus_cases, report.metrics.authorized, report.metrics.correct_decisions, report.metrics.false_authorizations, report.metrics.false_denials, report.metrics.intermediate_replay_verified, report.metrics.final_replay_verified, report.metrics.forged_intermediates_rejected, report.metrics.incompatible_handoffs_rejected, report.metrics.regressions);
    println!(
        "rewrites: pairs={} decision_stable={} result_stable={} regressions={}",
        report.rewrites.pairs,
        report.rewrites.decision_stable,
        report.rewrites.result_stable,
        report.rewrites.regressions
    );
    println!("failures: {:?}", report.failure_taxonomy);
}
