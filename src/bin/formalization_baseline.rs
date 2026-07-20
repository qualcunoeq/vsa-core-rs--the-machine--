//! Run the current report-only formalizer against the seed curriculum.
//!
//! This is intentionally a baseline evaluator, not an answer route.  It
//! calls `assess_prompt` and `assess_direct_instantiation`, records field-level
//! extraction scores, and writes a deterministic JSON report.

use serde::Serialize;
use sha2::{Digest, Sha256};
use std::{collections::BTreeMap, env, fs};
use the_machine::formalization::{
    assess_direct_instantiation, assess_prompt, score_formalization, FieldScore,
    FormalizationCorpus, FormalizationGoldCase, FormalizationScore,
};

#[derive(Debug, Serialize)]
struct Aggregate {
    cases: usize,
    exact_target: usize,
    structural_target: usize,
    authorization_correct: usize,
    definitions: Counts,
    facts: Counts,
    entities: Counts,
    assumptions: Counts,
    constraints: Counts,
    obligations: Counts,
    invented_definitions: usize,
    invented_facts: usize,
    invented_entities: usize,
    invented_assumptions: usize,
    invented_constraints: usize,
    invented_obligations: usize,
    false_authorizations: usize,
    false_denials: usize,
    failures: BTreeMap<String, Vec<String>>,
}

#[derive(Debug, Default, Serialize)]
struct Counts {
    matched: usize,
    expected: usize,
    predicted: usize,
    precision: f64,
    recall: f64,
}

impl Counts {
    fn add(&mut self, score: FieldScore) {
        self.matched += score.matched;
        self.expected += score.expected;
        self.predicted += score.predicted;
        self.precision = if self.predicted == 0 {
            if self.expected == 0 {
                1.0
            } else {
                0.0
            }
        } else {
            self.matched as f64 / self.predicted as f64
        };
        self.recall = if self.expected == 0 {
            if self.predicted == 0 {
                1.0
            } else {
                0.0
            }
        } else {
            self.matched as f64 / self.expected as f64
        };
    }
}

impl Aggregate {
    fn new() -> Self {
        Self {
            cases: 0,
            exact_target: 0,
            structural_target: 0,
            authorization_correct: 0,
            definitions: Counts::default(),
            facts: Counts::default(),
            entities: Counts::default(),
            assumptions: Counts::default(),
            constraints: Counts::default(),
            obligations: Counts::default(),
            invented_definitions: 0,
            invented_facts: 0,
            invented_entities: 0,
            invented_assumptions: 0,
            invented_constraints: 0,
            invented_obligations: 0,
            false_authorizations: 0,
            false_denials: 0,
            failures: BTreeMap::new(),
        }
    }

    fn add(
        &mut self,
        id: &str,
        score: &FormalizationScore,
        should_authorize: bool,
        authorized: bool,
    ) {
        self.cases += 1;
        self.exact_target += usize::from(score.target_exact);
        self.structural_target += usize::from(score.target_structural);
        self.authorization_correct += usize::from(score.authorization_correct);
        self.definitions.add(score.definitions);
        self.facts.add(score.facts);
        self.entities.add(score.entities);
        self.assumptions.add(score.assumptions);
        self.constraints.add(score.constraints);
        self.obligations.add(score.obligations);
        self.invented_definitions += score
            .definitions
            .predicted
            .saturating_sub(score.definitions.matched);
        self.invented_facts += score.facts.predicted.saturating_sub(score.facts.matched);
        self.invented_entities += score
            .entities
            .predicted
            .saturating_sub(score.entities.matched);
        self.invented_assumptions += score
            .assumptions
            .predicted
            .saturating_sub(score.assumptions.matched);
        self.invented_constraints += score
            .constraints
            .predicted
            .saturating_sub(score.constraints.matched);
        self.invented_obligations += score
            .obligations
            .predicted
            .saturating_sub(score.obligations.matched);
        if authorized && !should_authorize {
            self.false_authorizations += 1;
            self.failures
                .entry("false_authorization".into())
                .or_default()
                .push(id.into());
        }
        if !authorized && should_authorize {
            self.false_denials += 1;
            self.failures
                .entry("false_denial".into())
                .or_default()
                .push(id.into());
        }
        if !score.target_structural {
            self.failures
                .entry("target".into())
                .or_default()
                .push(id.into());
        }
        for (name, field) in [
            ("definitions", score.definitions),
            ("facts", score.facts),
            ("entities", score.entities),
            ("assumptions", score.assumptions),
            ("constraints", score.constraints),
            ("obligations", score.obligations),
        ] {
            if field.recall < 1.0 {
                self.failures
                    .entry(format!("{name}_recall"))
                    .or_default()
                    .push(id.into());
            }
        }
    }
}

#[derive(Debug, Serialize)]
struct BaselineReport {
    corpus_schema_version: u32,
    corpus_sha256: String,
    split_rule: String,
    total: Aggregate,
    development: Aggregate,
    holdout: Aggregate,
    by_tier: BTreeMap<String, Aggregate>,
    by_transformation: BTreeMap<String, Aggregate>,
}

fn holdout(id: &str) -> bool {
    id.rsplit('-')
        .next()
        .and_then(|suffix| suffix.parse::<u32>().ok())
        .map(|number| number >= 15)
        .unwrap_or(false)
}

fn evaluate_case(case: &FormalizationGoldCase) -> (FormalizationScore, bool) {
    let trace = assess_prompt(&case.id, &case.prompt, "Math", false);
    let authorized = assess_direct_instantiation(&trace).authorization_safe();
    (score_formalization(case, &trace, authorized), authorized)
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let path = env::args()
        .nth(1)
        .ok_or("usage: formalization_baseline <corpus.json> [report.json]")?;
    let text = fs::read_to_string(&path)?;
    let corpus: FormalizationCorpus = serde_json::from_str(&text)?;
    if !corpus.is_valid() {
        return Err("formalization corpus validation failed".into());
    }
    let digest = format!("{:x}", Sha256::digest(text.as_bytes()));
    let mut report = BaselineReport {
        corpus_schema_version: corpus.schema_version,
        corpus_sha256: digest,
        split_rule: "case suffix 01-14=development, 15-20=holdout within each tier".into(),
        total: Aggregate::new(),
        development: Aggregate::new(),
        holdout: Aggregate::new(),
        by_tier: BTreeMap::new(),
        by_transformation: BTreeMap::new(),
    };
    for case in &corpus.cases {
        let (score, authorized) = evaluate_case(case);
        report
            .total
            .add(&case.id, &score, case.authorization_expected, authorized);
        let split = if holdout(&case.id) {
            &mut report.holdout
        } else {
            &mut report.development
        };
        split.add(&case.id, &score, case.authorization_expected, authorized);
        report
            .by_tier
            .entry(case.tier.label().into())
            .or_insert_with(Aggregate::new)
            .add(&case.id, &score, case.authorization_expected, authorized);
        report
            .by_transformation
            .entry(case.transformation.label().into())
            .or_insert_with(Aggregate::new)
            .add(&case.id, &score, case.authorization_expected, authorized);
    }
    let output = serde_json::to_string_pretty(&report)?;
    println!("{output}");
    if let Some(report_path) = env::args().nth(2) {
        fs::write(report_path, output)?;
    }
    Ok(())
}
