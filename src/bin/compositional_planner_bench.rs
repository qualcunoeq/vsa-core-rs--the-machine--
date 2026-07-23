use std::env;
use std::fs;
use the_machine::compositional_planner_benchmark::{evaluate, PlannerCorpus};

fn main() {
    let path = env::args().nth(1).unwrap_or_else(|| "data/compositional_planner_ood_v1.json".into());
    let corpus: PlannerCorpus = serde_json::from_str(&fs::read_to_string(&path).expect("planner corpus")).expect("planner JSON");
    assert!(corpus.validation_errors().is_empty());
    let report = evaluate(&corpus);
    println!("planner: cases={} authorized={} correct_decisions={} false_auth={} false_denials={} replayed_stages={} ambiguous={} invalid_handoffs={} route_failures={}", report.corpus_cases, report.metrics.authorized, report.metrics.correct_decisions, report.metrics.false_authorizations, report.metrics.false_denials, report.metrics.accepted_replayed_stages, report.metrics.ambiguous, report.metrics.invalid_handoffs_rejected, report.metrics.route_failures);
    println!("failures: {:?}", report.failure_taxonomy);
}
