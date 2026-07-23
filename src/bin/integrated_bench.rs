//! Release-mode integration run for the isolated and composed corpora.

use std::env;
use std::fs;
use the_machine::cross_vertical_benchmark::{evaluate as evaluate_composition, CompositionCorpus};
use the_machine::compositional_planner_benchmark::{evaluate as evaluate_planner, PlannerCorpus};
use the_machine::raw_decomposition_benchmark::{evaluate as evaluate_raw, RawCorpus};
use the_machine::mixed_ood_benchmark::{evaluate as evaluate_mixed, MixedOodCorpus};

fn main() {
    let mixed_path = env::args().nth(1).unwrap_or_else(|| "data/mixed_ood_v1.json".into());
    let composition_path = env::args().nth(2).unwrap_or_else(|| "data/cross_vertical_ood_v1.json".into());
    let planner_path = env::args().nth(3).unwrap_or_else(|| "data/compositional_planner_ood_v1.json".into());
    let raw_path = env::args().nth(4).unwrap_or_else(|| "data/raw_decomposition_ood_v1.json".into());
    let mixed: MixedOodCorpus = serde_json::from_str(&fs::read_to_string(&mixed_path).expect("mixed corpus"))
        .expect("mixed JSON");
    let composition: CompositionCorpus = serde_json::from_str(&fs::read_to_string(&composition_path).expect("composition corpus"))
        .expect("composition JSON");
    let planner: PlannerCorpus = serde_json::from_str(&fs::read_to_string(&planner_path).expect("planner corpus"))
        .expect("planner JSON");
    let raw: RawCorpus = serde_json::from_str(&fs::read_to_string(&raw_path).expect("raw decomposition corpus"))
        .expect("raw decomposition JSON");
    assert!(mixed.validation_errors().is_empty());
    assert!(composition.validation_errors().is_empty());
    assert!(planner.validation_errors().is_empty());
    let mixed_report = evaluate_mixed(&mixed);
    let composition_report = evaluate_composition(&composition);
    let planner_report = evaluate_planner(&planner);
    let raw_report = evaluate_raw(&raw);
    assert_eq!(mixed_report.metrics.false_authorizations, 0);
    assert_eq!(mixed_report.metrics.false_denials, 0);
    assert_eq!(mixed_report.rewrites.regressions, 0);
    assert_eq!(composition_report.metrics.false_authorizations, 0);
    assert_eq!(composition_report.metrics.false_denials, 0);
    assert_eq!(composition_report.metrics.final_replay_verified, composition_report.metrics.authorized);
    assert_eq!(composition_report.rewrites.regressions, 0);
    assert_eq!(planner_report.metrics.false_authorizations, 0);
    assert_eq!(planner_report.metrics.false_denials, 0);
    assert_eq!(planner_report.metrics.route_failures, 0);
    assert_eq!(raw_report.metrics.false_authorizations, 0);
    assert_eq!(raw_report.metrics.false_denials, 0);
    assert_eq!(raw_report.metrics.structural_correct, raw_report.corpus_cases);
    println!("integrated: cases={} (mixed={} composition={} planner={} raw={}) route={:.3} mixed_auth={} composition_auth={} planner_auth={} raw_realized={} false_auth=0 false_denials=0", mixed_report.corpus_cases + composition_report.corpus_cases + planner_report.corpus_cases + raw_report.corpus_cases, mixed_report.corpus_cases, composition_report.corpus_cases, planner_report.corpus_cases, raw_report.corpus_cases, mixed_report.metrics.route_correct as f64 / mixed_report.corpus_cases as f64, mixed_report.metrics.authorized, composition_report.metrics.authorized, planner_report.metrics.authorized, raw_report.metrics.realized_plans);
    println!("composition_replay: intermediate={} final={} forged_rejected={} incompatible_rejected={}", composition_report.metrics.intermediate_replay_verified, composition_report.metrics.final_replay_verified, composition_report.metrics.forged_intermediates_rejected, composition_report.metrics.incompatible_handoffs_rejected);
    println!("planner: replayed_stages={} ambiguous={} invalid_handoffs={}", planner_report.metrics.accepted_replayed_stages, planner_report.metrics.ambiguous, planner_report.metrics.invalid_handoffs_rejected);
    println!("raw_decomposition: structural={} ambiguous={} replayed_stages={}", raw_report.metrics.structural_correct, raw_report.metrics.ambiguous_preserved, raw_report.metrics.replayed_stages);
}
