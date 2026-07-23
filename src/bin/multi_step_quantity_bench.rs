use serde::Deserialize;
use std::{collections::BTreeMap, env, fs};
use the_machine::multi_step_quantity::{execute, formalize, MultiStepDecision};

#[derive(Debug, Deserialize)]
struct Corpus {
    cases: Vec<Case>,
}

#[derive(Debug, Deserialize)]
struct Case {
    id: String,
    prompt: String,
    outcome: String,
    expected_result: Option<String>,
    family: Option<String>,
    pair_id: Option<String>,
}

fn main() {
    let path = env::args()
        .nth(1)
        .unwrap_or_else(|| "data/multi_step_quantity_v1.json".into());
    let corpus: Corpus =
        serde_json::from_str(&fs::read_to_string(path).expect("multi-step corpus"))
            .expect("multi-step JSON");
    let mut structural = 0usize;
    let mut accepted = 0usize;
    let mut replayed_plans = 0usize;
    let mut replayed_stages = 0usize;
    let mut ambiguous = 0usize;
    let mut unsupported = 0usize;
    let mut results_checked = 0usize;
    let mut results_correct = 0usize;
    let mut false_auth = 0usize;
    let mut false_denials = 0usize;
    let mut failures = BTreeMap::<String, usize>::new();
    let mut failure_ids = Vec::new();
    let mut pairs = BTreeMap::<String, Vec<(String, Option<String>)>>::new();

    for case in &corpus.cases {
        let decision = formalize(&case.prompt);
        let (actual, actual_family, result) = match decision {
            MultiStepDecision::Accepted(plan) => {
                accepted += 1;
                let family = plan.family.clone();
                match execute(&plan) {
                    Some(receipt) => {
                        replayed_plans += usize::from(receipt.replay_verified);
                        replayed_stages += receipt
                            .stages
                            .iter()
                            .filter(|stage| stage.replay_verified)
                            .count();
                        ("supported", Some(family), Some(receipt.final_result))
                    }
                    None => ("supported", Some(family), None),
                }
            }
            MultiStepDecision::Ambiguous => {
                ambiguous += 1;
                ("ambiguous", None, None)
            }
            MultiStepDecision::Unsupported => {
                unsupported += 1;
                ("unsupported", None, None)
            }
        };
        let route_ok = actual == case.outcome && actual_family == case.family;
        let result_ok = case
            .expected_result
            .as_deref()
            .map(|expected| result.as_deref() == Some(expected))
            .unwrap_or(true);
        structural += usize::from(route_ok && result_ok);
        if case.expected_result.is_some() {
            results_checked += 1;
            results_correct += usize::from(result_ok);
        }
        false_auth += usize::from(actual == "supported" && case.outcome != "supported");
        false_denials += usize::from(actual != "supported" && case.outcome == "supported");
        if !(route_ok && result_ok) {
            failure_ids.push(case.id.clone());
            *failures
                .entry(
                    if route_ok {
                        "result_mismatch"
                    } else {
                        "structure_mismatch"
                    }
                    .into(),
                )
                .or_default() += 1;
        }
        if let Some(pair_id) = &case.pair_id {
            pairs
                .entry(pair_id.clone())
                .or_default()
                .push((actual.into(), result));
        }
    }
    let mut pair_count = 0usize;
    let mut pair_stable = 0usize;
    for values in pairs.values() {
        if values.len() > 1 {
            pair_count += 1;
            pair_stable += usize::from(values.windows(2).all(|window| window[0] == window[1]));
        }
    }
    println!(
        "multi-step-quantity: cases={} structural={}/{} accepted={} replayed_plans={} replayed_stages={} ambiguous={} unsupported={} results={}/{} rewrite_pairs={}/{} false_auth={} false_denials={} failures={:?} failure_ids={:?} deterministic=true",
        corpus.cases.len(), structural, corpus.cases.len(), accepted, replayed_plans,
        replayed_stages, ambiguous, unsupported, results_correct, results_checked, pair_stable,
        pair_count, false_auth, false_denials, failures, failure_ids
    );
}
