//! Safety-boundary ablations for the expanded independent systems corpus.

use std::{env, fs};
use the_machine::linear_system::evaluate_system_ablations;
use the_machine::ood_benchmark::OodCorpus;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let path = env::args()
        .nth(1)
        .unwrap_or_else(|| "data/algebra_systems_ood_v2.json".into());
    let corpus: OodCorpus = serde_json::from_str(&fs::read_to_string(path)?)?;
    let sources = corpus
        .all_cases()
        .into_iter()
        .map(|case| case.prompt.as_str())
        .collect::<Vec<_>>();
    let report = evaluate_system_ablations(&sources);
    println!("{report:?}");
    println!(
        "classifier_bypass_false_accepts={} replay_tampered_rejections={}/{} replay_bypass_tampered_accepts={}",
        report.classifier_bypass_false_accepts,
        report.replay_tampered_rejections,
        report.unique_cases,
        report.replay_bypass_tampered_accepts,
    );
    Ok(())
}
