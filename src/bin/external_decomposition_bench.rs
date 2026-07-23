use std::{env, fs};
use the_machine::external_decomposition_benchmark::{evaluate, ExternalCorpus};

fn main() {
    let path = env::args()
        .nth(1)
        .unwrap_or_else(|| "data/external_decomposition_v1.json".into());
    let corpus: ExternalCorpus =
        serde_json::from_str(&fs::read_to_string(&path).expect("external corpus"))
            .expect("external JSON");
    let errors = corpus.validation_errors();
    assert!(errors.is_empty(), "invalid external corpus: {errors:?}");
    let report = evaluate(&corpus);
    println!(
        "external-decomposition: cases={} dev={} holdout={} structural={}/{} realized={} replayed_stages={} ambiguous={} false_auth={} false_denials={}",
        report.corpus_cases,
        report.development.cases,
        report.holdout.cases,
        report.metrics.structural_correct,
        report.corpus_cases,
        report.metrics.realized_plans,
        report.metrics.replayed_stages,
        report.metrics.ambiguous_preserved,
        report.metrics.false_authorizations,
        report.metrics.false_denials,
    );
    println!(
        "development: structural={}/{} realized={} false_auth={} false_denials={}",
        report.development.structural_correct,
        report.development.cases,
        report.development.realized_plans,
        report.development.false_authorizations,
        report.development.false_denials,
    );
    println!(
        "holdout: structural={}/{} realized={} false_auth={} false_denials={}",
        report.holdout.structural_correct,
        report.holdout.cases,
        report.holdout.realized_plans,
        report.holdout.false_authorizations,
        report.holdout.false_denials,
    );
    println!("failure_taxonomy: {:?}", report.failure_taxonomy);
    println!("failures_by_source: {:?}", report.failures_by_source);
}
