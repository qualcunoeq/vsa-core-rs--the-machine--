//! Stage 235: shadow-only HLE probe against provenance-derived catalogs.
//!
//! This is not a production route. It asks whether any frozen HLE question
//! would form a unique, replayable source-formula request against the newly
//! acquired catalogs, while rejecting incorrect candidates before any answer
//! authorization or live mutation.

use serde::Serialize;
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::fs::{self, File};
use std::io::{BufRead, BufReader};
use the_machine::probability_pack::Rational;
use the_machine::router::QuestionRouter;
use the_machine::source_formula_frontend::{
    formalize_source_formula_report, report_replay_verified, FrontendStatus,
};
use the_machine::source_formula_pack::{evaluate_formula_records, FormulaStatus};
use the_machine::source_module_discovery::{discover_formula_corpus, DiscoveredSourceModule};

const DATASET: &str = "data/hle.jsonl";
const SOURCE_REPORT: &str = "docs/stage233_provenance_learning_curve.json";
const ECONOMICS: &str = include_str!("../../docs/sources/openstax_bounded_economics_source.txt");
const STATISTICS: &str = include_str!("../../docs/sources/openstax_finite_statistics_source.txt");
const COMPLEX: &str = include_str!("../../docs/sources/openstax_complex_arithmetic_source.txt");

#[derive(Debug, Serialize)]
struct Report {
    schema: &'static str,
    source_learning_report_sha256: String,
    dataset_sha256: String,
    cases: usize,
    source_modules: usize,
    frontend_replays: usize,
    frontend_tamper_rejections: usize,
    unique_shadow_candidates: usize,
    correct_shadow_candidates: usize,
    incorrect_shadow_candidates_rejected: usize,
    ambiguous_or_missing: usize,
    unsupported: usize,
    production_authorizations: usize,
    false_authorizations: usize,
    source_memory_mutations: usize,
    registry_mutations: usize,
    corpus_sha256: String,
}

fn digest_bytes(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

fn digest<T: Serialize>(value: &T) -> String {
    format!("{:x}", Sha256::digest(serde_json::to_vec(value).unwrap()))
}

fn render(value: Rational) -> String {
    if value.denominator == 1 {
        value.numerator.to_string()
    } else {
        format!("{}/{}", value.numerator, value.denominator)
    }
}

fn shadow_candidate(
    question: &str,
    modules: &[DiscoveredSourceModule],
) -> (Option<String>, usize, usize, usize) {
    let mut candidates = Vec::new();
    let mut replay = 0;
    let mut tamper = 0;
    let mut unsupported = 0;
    for module in modules {
        let report =
            formalize_source_formula_report(question, &module.candidate.domain, &module.records);
        replay += usize::from(report_replay_verified(&report));
        let mut altered = report.clone();
        altered.replay_hash.push('x');
        tamper += usize::from(!report_replay_verified(&altered));
        if report.frontend.status == FrontendStatus::Unsupported {
            unsupported += 1;
        }
        if report.frontend.status != FrontendStatus::Complete {
            continue;
        }
        if let Some(request) = report.frontend.request.as_ref() {
            let result =
                evaluate_formula_records(request, &module.candidate.domain, &module.records);
            if result.status == FormulaStatus::Complete {
                if let Some(value) = result.value {
                    candidates.push(render(value));
                }
            }
        }
    }
    let unique = candidates.len() == 1;
    (
        if unique {
            candidates.into_iter().next()
        } else {
            None
        },
        replay,
        tamper,
        unsupported,
    )
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let source_report = fs::read(SOURCE_REPORT)?;
    let source: Value = serde_json::from_slice(&source_report)?;
    assert_eq!(
        source.get("sealed_authorizations").and_then(Value::as_u64),
        Some(120)
    );
    assert_eq!(
        source.get("false_authorizations").and_then(Value::as_u64),
        Some(0)
    );
    let dataset = fs::read(DATASET)?;
    let modules = discover_formula_corpus(&[ECONOMICS, STATISTICS, COMPLEX], "unused-hint")
        .map_err(|errors| errors.join("; "))?;
    assert_eq!(modules.len(), 6);
    let mut report = Report {
        schema: "stage235-hle-shadow-source-probe-v1",
        source_learning_report_sha256: digest_bytes(&source_report),
        dataset_sha256: digest_bytes(&dataset),
        cases: 0,
        source_modules: modules.len(),
        frontend_replays: 0,
        frontend_tamper_rejections: 0,
        unique_shadow_candidates: 0,
        correct_shadow_candidates: 0,
        incorrect_shadow_candidates_rejected: 0,
        ambiguous_or_missing: 0,
        unsupported: 0,
        production_authorizations: 0,
        false_authorizations: 0,
        source_memory_mutations: 0,
        registry_mutations: 0,
        corpus_sha256: String::new(),
    };
    let mut receipts = Vec::new();
    for line in BufReader::new(File::open(DATASET)?).lines() {
        let line = line?;
        if line.trim().is_empty() {
            continue;
        }
        let entry: Value = serde_json::from_str(&line)?;
        let question = entry.get("question").and_then(Value::as_str).unwrap_or("");
        let expected = entry.get("answer").and_then(Value::as_str).unwrap_or("");
        let (candidate, replay, tamper, unsupported) = shadow_candidate(question, &modules);
        report.frontend_replays += replay;
        report.frontend_tamper_rejections += tamper;
        report.unsupported += usize::from(unsupported == modules.len());
        match candidate {
            Some(ref answer) => {
                report.unique_shadow_candidates += 1;
                if QuestionRouter::exact_answers_match(&answer, expected) {
                    report.correct_shadow_candidates += 1;
                } else {
                    report.incorrect_shadow_candidates_rejected += 1;
                }
            }
            None => report.ambiguous_or_missing += 1,
        }
        receipts.push((question.to_owned(), expected.to_owned(), candidate));
        report.cases += 1;
    }
    report.corpus_sha256 = digest(&receipts);
    assert_eq!(report.cases, 2500);
    assert_eq!(report.source_modules, 6);
    assert_eq!(report.production_authorizations, 0);
    assert_eq!(report.false_authorizations, 0);
    assert_eq!(report.source_memory_mutations, 0);
    assert_eq!(report.registry_mutations, 0);
    println!("{}", serde_json::to_string_pretty(&report)?);
    Ok(())
}
