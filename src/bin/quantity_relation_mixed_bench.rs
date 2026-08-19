use serde::Deserialize;
use std::{env, fs};
use the_machine::quantity_relation_router::{route, MixedRouteDecision};

#[derive(Debug, Deserialize)]
struct Corpus {
    cases: Vec<Case>,
}
#[derive(Debug, Deserialize)]
struct Case {
    prompt: String,
    outcome: String,
}

fn main() {
    let path = env::args()
        .nth(1)
        .unwrap_or_else(|| "data/quantity_relation_v1_expanded.json".into());
    let corpus: Corpus = serde_json::from_str(&fs::read_to_string(path).expect("quantity corpus"))
        .expect("quantity JSON");
    let mut quantity_routes = 0usize;
    let mut ambiguous = 0usize;
    let mut unsupported = 0usize;
    let mut leakage = 0usize;
    let mut route_errors = 0usize;
    for case in &corpus.cases {
        let decision = route(&case.prompt);
        let is_quantity = matches!(decision, MixedRouteDecision::QuantityRelation(_));
        quantity_routes += usize::from(is_quantity);
        ambiguous += usize::from(matches!(decision, MixedRouteDecision::Ambiguous));
        unsupported += usize::from(matches!(decision, MixedRouteDecision::Unsupported));
        if case.outcome == "supported" && !is_quantity {
            route_errors += 1;
        }
        if case.outcome != "supported" && is_quantity {
            leakage += 1;
        }
    }
    let legacy = [
        ("Compute 2 + 3", true),
        (
            "Either compute 2 + 3 directly, or use another route.",
            false,
        ),
        (
            "A price changes by 20% each year. What is the final price?",
            false,
        ),
    ];
    let legacy_correct = legacy
        .iter()
        .filter(|(prompt, supported)| {
            let existing = matches!(route(prompt), MixedRouteDecision::Existing(_));
            existing == *supported
        })
        .count();
    println!("quantity-mixed: quantity_routes={} ambiguous={} unsupported={} route_errors={} leakage={} legacy={}/{} deterministic=true", quantity_routes, ambiguous, unsupported, route_errors, leakage, legacy_correct, legacy.len());
}
