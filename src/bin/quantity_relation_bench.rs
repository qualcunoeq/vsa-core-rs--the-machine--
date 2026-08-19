use serde::Deserialize;
use std::{collections::BTreeMap, env, fs};
use the_machine::quantity_relation::{formalize, QuantityRelationDecision};
use the_machine::quantity_relation_integration::{
    bridge_ratio_to_linear_system, bridge_to_algebra,
};

#[derive(Debug, Deserialize)]
struct Corpus {
    cases: Vec<Case>,
}

#[derive(Debug, Deserialize)]
struct Case {
    #[allow(dead_code)]
    id: String,
    prompt: String,
    outcome: String,
    family: String,
    signature: Option<String>,
    pair_id: Option<String>,
}

fn main() {
    let path = env::args()
        .nth(1)
        .unwrap_or_else(|| "data/quantity_relation_v1_expanded.json".into());
    let corpus: Corpus = serde_json::from_str(&fs::read_to_string(path).expect("quantity corpus"))
        .expect("quantity JSON");
    let mut structural = 0usize;
    let mut accepted = 0usize;
    let mut ambiguous = 0usize;
    let mut unsupported = 0usize;
    let mut false_auth = 0usize;
    let mut false_denial = 0usize;
    let mut replayed = 0usize;
    let mut algebra_bridged = 0usize;
    let mut algebra_replayed = 0usize;
    let mut system_bridged = 0usize;
    let mut system_replayed = 0usize;
    let mut failures = BTreeMap::<String, usize>::new();
    let mut failures_by_family = BTreeMap::<String, usize>::new();
    let mut pair_results = BTreeMap::<String, Vec<(String, Option<String>)>>::new();
    for case in &corpus.cases {
        let decision = formalize(&case.prompt);
        let (actual, signature) = match &decision {
            QuantityRelationDecision::Accepted(artifact) => {
                accepted += 1;
                replayed += usize::from(artifact.replay_verified());
                if let Some(receipt) = bridge_to_algebra(artifact) {
                    algebra_bridged += 1;
                    algebra_replayed += usize::from(receipt.algebra_replay_verified);
                }
                if artifact.family == "ratio" {
                    if let Some(receipt) = bridge_ratio_to_linear_system(artifact) {
                        system_bridged += 1;
                        system_replayed += usize::from(receipt.replay_verified);
                    }
                }
                ("supported", Some(artifact.signature.clone()))
            }
            QuantityRelationDecision::Ambiguous => {
                ambiguous += 1;
                ("ambiguous", None)
            }
            QuantityRelationDecision::Unsupported => {
                unsupported += 1;
                ("unsupported", None)
            }
        };
        let correct =
            actual == case.outcome && (actual != "supported" || signature == case.signature);
        structural += usize::from(correct);
        if actual == "supported" && case.outcome != "supported" {
            false_auth += 1;
        }
        if actual != "supported" && case.outcome == "supported" {
            false_denial += 1;
        }
        if !correct {
            let label = if case.outcome == "supported" {
                "supported_case_not_accepted"
            } else {
                "negative_case_accepted_or_misclassified"
            };
            *failures.entry(label.into()).or_default() += 1;
            *failures_by_family.entry(case.family.clone()).or_default() += 1;
        }
        if let Some(pair_id) = &case.pair_id {
            pair_results
                .entry(pair_id.clone())
                .or_default()
                .push((actual.into(), signature));
        }
    }
    let mut rewrite_pairs = 0usize;
    let mut rewrite_stable = 0usize;
    for results in pair_results.values() {
        if results.len() > 1 {
            rewrite_pairs += 1;
            let stable = results
                .iter()
                .all(|(outcome, signature)| outcome == "supported" && *signature == results[0].1);
            rewrite_stable += usize::from(stable);
        }
    }
    println!("quantity-relation: cases={} structural={}/{} accepted={} ambiguous={} unsupported={} replayed={} algebra_bridge={}/{} ratio_system_bridge={}/{} rewrite_pairs={}/{} false_auth={} false_denials={} failures={:?} failures_by_family={:?}", corpus.cases.len(), structural, corpus.cases.len(), accepted, ambiguous, unsupported, replayed, algebra_bridged, algebra_replayed, system_bridged, system_replayed, rewrite_stable, rewrite_pairs, false_auth, false_denial, failures, failures_by_family);
}
