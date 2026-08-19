use serde::Deserialize;
use std::{
    collections::{BTreeMap, BTreeSet},
    env, fs,
};

#[derive(Debug, Deserialize)]
struct Corpus {
    schema_version: u32,
    cases: Vec<Case>,
}

#[derive(Debug, Deserialize)]
struct Case {
    id: String,
    prompt: String,
    outcome: String,
    family: String,
    signature: Option<String>,
    target: Option<String>,
    pair_id: Option<String>,
    reason: Option<String>,
}

fn main() {
    let path = env::args()
        .nth(1)
        .unwrap_or_else(|| "data/quantity_relation_v1_pilot.json".into());
    let corpus: Corpus =
        serde_json::from_str(&fs::read_to_string(&path).expect("quantity relation corpus"))
            .expect("quantity relation JSON");
    assert_eq!(corpus.schema_version, 1, "unsupported schema");
    let mut ids = BTreeSet::new();
    let mut pairs: BTreeMap<String, Vec<&Case>> = BTreeMap::new();
    for case in &corpus.cases {
        assert!(ids.insert(&case.id), "duplicate case id: {}", case.id);
        assert!(!case.prompt.trim().is_empty(), "empty prompt: {}", case.id);
        assert!(!case.family.trim().is_empty(), "empty family: {}", case.id);
        match case.outcome.as_str() {
            "supported" => {
                assert!(
                    case.signature.is_some(),
                    "supported case missing signature: {}",
                    case.id
                );
                assert!(
                    case.target.is_some(),
                    "supported case missing target: {}",
                    case.id
                );
                assert!(
                    case.reason.is_none(),
                    "supported case has rejection reason: {}",
                    case.id
                );
            }
            "ambiguous" | "unsupported" => {
                assert!(
                    case.signature.is_none(),
                    "negative case has signature: {}",
                    case.id
                );
                assert!(
                    case.reason.is_some(),
                    "negative case missing reason: {}",
                    case.id
                );
            }
            other => panic!("unknown outcome {other:?} for {}", case.id),
        }
        if let Some(pair_id) = &case.pair_id {
            pairs.entry(pair_id.clone()).or_default().push(case);
        }
    }
    let rewrite_pairs = pairs
        .values()
        .filter(|group| group.len() > 1)
        .inspect(|group| {
            let supported: Vec<_> = group
                .iter()
                .filter(|case| case.outcome == "supported")
                .filter_map(|case| case.signature.as_deref())
                .collect();
            if supported.len() > 1 {
                assert!(
                    supported.iter().all(|signature| *signature == supported[0]),
                    "rewrite signature mismatch"
                );
            }
        })
        .count();
    let supported = corpus
        .cases
        .iter()
        .filter(|case| case.outcome == "supported")
        .count();
    let ambiguous = corpus
        .cases
        .iter()
        .filter(|case| case.outcome == "ambiguous")
        .count();
    let unsupported = corpus
        .cases
        .iter()
        .filter(|case| case.outcome == "unsupported")
        .count();
    println!(
        "quantity-relation-corpus: cases={} supported={} ambiguous={} unsupported={} rewrite_pairs={} deterministic=true",
        corpus.cases.len(), supported, ambiguous, unsupported, rewrite_pairs
    );
}
