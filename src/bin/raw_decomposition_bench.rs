use std::{env, fs};
use the_machine::raw_decomposition_benchmark::{evaluate, RawCorpus};

fn main() {
    let path = env::args()
        .nth(1)
        .unwrap_or_else(|| "data/raw_decomposition_ood_v1.json".into());
    let corpus: RawCorpus =
        serde_json::from_str(&fs::read_to_string(&path).expect("raw corpus")).expect("raw JSON");
    let report = evaluate(&corpus);
    println!("raw-decomposition: cases={} structural={} decisions={} correct={} false_auth={} false_denials={} realized={} replayed_stages={} ambiguous={} unnecessary={} missed_direct={}", report.corpus_cases, report.metrics.structural_correct, report.metrics.decomposition_decisions, report.metrics.correct_decisions, report.metrics.false_authorizations, report.metrics.false_denials, report.metrics.realized_plans, report.metrics.replayed_stages, report.metrics.ambiguous_preserved, report.metrics.unnecessary_decompositions, report.metrics.missed_direct_routes);
    println!("failures: {:?}", report.failure_taxonomy);
}
