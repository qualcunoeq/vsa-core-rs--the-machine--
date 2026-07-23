//! Release-mode integration run for the isolated and composed corpora.

use std::env;
use std::fs;
use the_machine::cross_vertical_benchmark::{evaluate as evaluate_composition, CompositionCorpus};
use the_machine::mixed_ood_benchmark::{evaluate as evaluate_mixed, MixedOodCorpus};

fn main() {
    let mixed_path = env::args().nth(1).unwrap_or_else(|| "data/mixed_ood_v1.json".into());
    let composition_path = env::args().nth(2).unwrap_or_else(|| "data/cross_vertical_ood_v1.json".into());
    let mixed: MixedOodCorpus = serde_json::from_str(&fs::read_to_string(&mixed_path).expect("mixed corpus"))
        .expect("mixed JSON");
    let composition: CompositionCorpus = serde_json::from_str(&fs::read_to_string(&composition_path).expect("composition corpus"))
        .expect("composition JSON");
    assert!(mixed.validation_errors().is_empty());
    assert!(composition.validation_errors().is_empty());
    let mixed_report = evaluate_mixed(&mixed);
    let composition_report = evaluate_composition(&composition);
    assert_eq!(mixed_report.metrics.false_authorizations, 0);
    assert_eq!(mixed_report.metrics.false_denials, 0);
    assert_eq!(mixed_report.rewrites.regressions, 0);
    assert_eq!(composition_report.metrics.false_authorizations, 0);
    assert_eq!(composition_report.metrics.false_denials, 0);
    assert_eq!(composition_report.metrics.final_replay_verified, composition_report.metrics.authorized);
    assert_eq!(composition_report.rewrites.regressions, 0);
    println!("integrated: cases={} (mixed={} composition={}) route={:.3} mixed_auth={} composition_auth={} false_auth=0 false_denials=0", mixed_report.corpus_cases + composition_report.corpus_cases, mixed_report.corpus_cases, composition_report.corpus_cases, mixed_report.metrics.route_correct as f64 / mixed_report.corpus_cases as f64, mixed_report.metrics.authorized, composition_report.metrics.authorized);
    println!("composition_replay: intermediate={} final={} forged_rejected={} incompatible_rejected={}", composition_report.metrics.intermediate_replay_verified, composition_report.metrics.final_replay_verified, composition_report.metrics.forged_intermediates_rejected, composition_report.metrics.incompatible_handoffs_rejected);
}
