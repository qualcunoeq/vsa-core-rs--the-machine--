use std::{env, fs};
use the_machine::quantity_cross_domain_benchmark::{evaluate, CrossDomainCorpus};

fn main() {
    let path = env::args()
        .nth(1)
        .unwrap_or_else(|| "data/quantity_cross_domain_v1.json".into());
    let corpus: CrossDomainCorpus =
        serde_json::from_str(&fs::read_to_string(&path).expect("cross-domain corpus"))
            .expect("cross-domain JSON");
    let errors = corpus.validation_errors();
    assert!(errors.is_empty(), "invalid corpus: {errors:?}");
    let report = evaluate(&corpus);
    println!(
        "quantity-cross-domain: cases={} authorized={} correct_decisions={} false_auth={} false_denials={} intermediate_replays={} final_replays={} invalid_handoffs_rejected={} route_failures={} ambiguous={} rewrite_decisions={}/{} rewrite_results={}/{} regressions={} failures={:?} deterministic={}",
        report.corpus_cases,
        report.metrics.authorized,
        report.metrics.correct_decisions,
        report.metrics.false_authorizations,
        report.metrics.false_denials,
        report.metrics.intermediate_replays,
        report.metrics.final_replays,
        report.metrics.invalid_handoffs_rejected,
        report.metrics.route_failures,
        report.metrics.ambiguous,
        report.rewrites.decision_stable,
        report.rewrites.pairs,
        report.rewrites.result_stable,
        report.rewrites.pairs,
        report.rewrites.regressions,
        report.failure_taxonomy,
        report.deterministic,
    );
}
