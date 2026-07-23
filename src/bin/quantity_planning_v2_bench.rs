use the_machine::quantity_cross_domain_benchmark::evaluate;
use the_machine::quantity_planning_v2_benchmark::corpus;

fn main() {
    let corpus = corpus();
    assert!(corpus.validation_errors().is_empty());
    let report = evaluate(&corpus);
    println!(
        "quantity-planning-v2: cases={} authorized={} correct_decisions={} false_auth={} false_denials={} intermediate_replays={} final_replays={} invalid_handoffs_rejected={} route_failures={} ambiguous={} rewrite_decisions={}/{} rewrite_results={}/{} regressions={} failures={:?} deterministic={}",
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
