use std::{env, fs};
use the_machine::third_party_corpus_benchmark::{evaluate, ThirdPartyCorpus};

fn main() {
    let path = env::args()
        .nth(1)
        .unwrap_or_else(|| "data/third_party_corpus_fixture_v1.json".into());
    let corpus: ThirdPartyCorpus =
        serde_json::from_str(&fs::read_to_string(&path).expect("third-party corpus"))
            .expect("third-party JSON");
    let errors = corpus.validation_errors();
    assert!(errors.is_empty(), "invalid third-party release: {errors:?}");
    let report = evaluate(&corpus);
    let metrics = &report.evaluation.metrics;
    println!(
        "third-party-corpus: release={} kind={:?} hash={} cases={} structural={}/{} realized={} replayed_stages={} false_auth={} false_denials={}",
        report.release_id,
        report.release_kind,
        report.release_hash,
        report.evaluation.corpus_cases,
        metrics.structural_correct,
        metrics.cases,
        metrics.realized_plans,
        metrics.replayed_stages,
        metrics.false_authorizations,
        metrics.false_denials,
    );
    println!(
        "development: structural={}/{} holdout: structural={}/{} failures={:?}",
        report.evaluation.development.structural_correct,
        report.evaluation.development.cases,
        report.evaluation.holdout.structural_correct,
        report.evaluation.holdout.cases,
        report.evaluation.failure_taxonomy,
    );
}
