//! Validate a manually reviewed formalization curriculum corpus.
//!
//! This tool only checks annotation integrity.  It never runs a solver and
//! never turns a gold case into an answer route.

use std::{collections::BTreeMap, env, fs};
use the_machine::formalization::FormalizationCorpus;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let path = env::args()
        .nth(1)
        .ok_or("usage: formalization_corpus_check <corpus.json>")?;
    let text = fs::read_to_string(&path)?;
    let corpus: FormalizationCorpus = serde_json::from_str(&text)?;
    let errors = corpus.validation_errors();
    if errors.is_empty() {
        println!(
            "valid formalization corpus: schema_version={}, cases={}",
            corpus.schema_version,
            corpus.cases.len()
        );
        let mut tiers = BTreeMap::<&str, usize>::new();
        let mut transformations = BTreeMap::<&str, usize>::new();
        let mut authorized = 0usize;
        for case in &corpus.cases {
            *tiers.entry(case.tier.label()).or_default() += 1;
            *transformations
                .entry(case.transformation.label())
                .or_default() += 1;
            authorized += usize::from(case.authorization_expected);
        }
        println!("authorization_expected={authorized}");
        println!("tiers:");
        for (tier, count) in tiers {
            println!("  {tier}: {count}");
        }
        println!("transformations:");
        for (transformation, count) in transformations {
            println!("  {transformation}: {count}");
        }
        Ok(())
    } else {
        eprintln!("invalid formalization corpus ({} errors):", errors.len());
        for error in errors {
            eprintln!("- {error}");
        }
        Err("formalization corpus validation failed".into())
    }
}
