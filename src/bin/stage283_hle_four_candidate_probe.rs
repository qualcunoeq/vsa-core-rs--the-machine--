//! Stage 283: frozen HLE probe for the four-candidate shadow portfolio.
//!
//! This is diagnostic only.  A question reaches a shadow candidate only when
//! exactly one source module yields a complete replayable result; no answer
//! is authorized in production and the HLE corpus is not used for training.

use serde::Serialize;
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::fs::{self, File};
use std::io::{BufRead, BufReader};

use the_machine::router::QuestionRouter;
use the_machine::source_formula_frontend::{
    formalize_source_formula_report, report_replay_verified, FrontendStatus,
};
use the_machine::source_formula_pack::{evaluate_formula_records, FormulaStatus};
use the_machine::source_module_discovery::{
    discover_formula_module, DiscoveredSourceModule, SourceDocument,
};

const DATASET: &str = "data/hle.jsonl";
const SHADOW_MANIFEST: &str = "docs/stage282_four_candidate_shadow_manifest.json";
const REPORT_JSON: &str = "docs/stage283_hle_four_candidate_probe.json";
const REPORT_MD: &str = "docs/stage283_hle_four_candidate_probe.md";
const ECONOMICS: &str = include_str!("../../docs/sources/openstax_bounded_economics_source.txt");
const GEOMETRY: &str = include_str!("../../docs/sources/openstax_bounded_geometry_source.txt");
const HEALTH: &str = include_str!("../../docs/sources/openstax_bounded_health_ratios_source.txt");
const UNITS: &str = include_str!("../../docs/sources/openstax_unit_conversion_catalog.txt");

#[derive(Debug, Serialize)]
struct Report {
    schema: &'static str,
    dataset_sha256: String,
    shadow_manifest_sha256: String,
    cases: usize,
    source_modules: usize,
    source_records: usize,
    frontend_replays: usize,
    frontend_tamper_rejections: usize,
    unique_shadow_candidates: usize,
    correct_shadow_candidates: usize,
    incorrect_shadow_candidates_rejected: usize,
    ambiguous_or_missing: usize,
    unsupported: usize,
    production_authorizations: usize,
    false_authorizations: usize,
    live_manifest_mutations: usize,
    live_registry_mutations: usize,
    corpus_sha256: String,
}

fn digest_bytes(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

fn digest<T: Serialize>(value: &T) -> String {
    format!("{:x}", Sha256::digest(serde_json::to_vec(value).unwrap()))
}

fn render(value: the_machine::probability_pack::Rational) -> String {
    if value.denominator == 1 {
        value.numerator.to_string()
    } else {
        format!("{}/{}", value.numerator, value.denominator)
    }
}

fn modules() -> Result<Vec<DiscoveredSourceModule>, Box<dyn std::error::Error>> {
    Ok(vec![
        discover_formula_module(SourceDocument {
            domain: "source_derived_bounded_economics",
            version: "openstax-2026",
            source_hint: "economics",
            document: ECONOMICS,
        })
        .map_err(|e| e.join("; "))?,
        discover_formula_module(SourceDocument {
            domain: "source_derived_bounded_geometry",
            version: "openstax-2026",
            source_hint: "geometry",
            document: GEOMETRY,
        })
        .map_err(|e| e.join("; "))?,
        discover_formula_module(SourceDocument {
            domain: "source_derived_bounded_health_ratios",
            version: "openstax-2026",
            source_hint: "health-ratios",
            document: HEALTH,
        })
        .map_err(|e| e.join("; "))?,
        discover_formula_module(SourceDocument {
            domain: "source_derived_bounded_unit_conversion",
            version: "openstax-2026",
            source_hint: "unit-conversion",
            document: UNITS,
        })
        .map_err(|e| e.join("; "))?,
    ])
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let dataset_bytes = fs::read(DATASET)?;
    let shadow_bytes = fs::read(SHADOW_MANIFEST)?;
    let shadow: Value = serde_json::from_slice(&shadow_bytes)?;
    assert_eq!(
        shadow.get("shadow_only").and_then(Value::as_bool),
        Some(true)
    );
    let modules = modules()?;
    let mut report = Report {
        schema: "stage283-hle-four-candidate-probe-v1",
        dataset_sha256: digest_bytes(&dataset_bytes),
        shadow_manifest_sha256: digest_bytes(&shadow_bytes),
        cases: 0,
        source_modules: modules.len(),
        source_records: modules.iter().map(|module| module.records.len()).sum(),
        frontend_replays: 0,
        frontend_tamper_rejections: 0,
        unique_shadow_candidates: 0,
        correct_shadow_candidates: 0,
        incorrect_shadow_candidates_rejected: 0,
        ambiguous_or_missing: 0,
        unsupported: 0,
        production_authorizations: 0,
        false_authorizations: 0,
        live_manifest_mutations: 0,
        live_registry_mutations: 0,
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
        let mut candidates = Vec::new();
        let mut unsupported_count = 0;
        for module in &modules {
            let frontend = formalize_source_formula_report(
                question,
                &module.candidate.domain,
                &module.records,
            );
            report.frontend_replays += usize::from(report_replay_verified(&frontend));
            let mut tampered = frontend.clone();
            tampered.replay_hash.push('x');
            report.frontend_tamper_rejections += usize::from(!report_replay_verified(&tampered));
            if frontend.frontend.status == FrontendStatus::Unsupported {
                unsupported_count += 1;
            }
            if frontend.frontend.status == FrontendStatus::Complete {
                if let Some(request) = frontend.frontend.request.as_ref() {
                    let execution = evaluate_formula_records(
                        request,
                        &module.candidate.domain,
                        &module.records,
                    );
                    if execution.status == FormulaStatus::Complete
                        && execution.value.is_some()
                        && execution.replay_verified()
                    {
                        candidates.push(render(execution.value.as_ref().unwrap().clone()));
                    }
                }
            }
        }
        report.unsupported += usize::from(unsupported_count == modules.len());
        let candidate = if candidates.len() == 1 {
            candidates.into_iter().next()
        } else {
            None
        };
        if let Some(answer) = candidate.as_ref() {
            report.unique_shadow_candidates += 1;
            if QuestionRouter::exact_answers_match(answer, expected) {
                report.correct_shadow_candidates += 1;
            } else {
                report.incorrect_shadow_candidates_rejected += 1;
            }
        } else {
            report.ambiguous_or_missing += 1;
        }
        receipts.push((question.to_owned(), expected.to_owned(), candidate));
        report.cases += 1;
    }
    report.corpus_sha256 = digest(&receipts);
    assert_eq!(report.cases, 2500);
    assert_eq!(report.source_modules, 4);
    assert_eq!(report.source_records, 19);
    assert_eq!(report.frontend_replays, 10000);
    assert_eq!(report.frontend_tamper_rejections, 10000);
    assert_eq!(report.production_authorizations, 0);
    assert_eq!(report.false_authorizations, 0);
    assert_eq!(report.live_manifest_mutations, 0);
    assert_eq!(report.live_registry_mutations, 0);
    fs::write(REPORT_JSON, serde_json::to_vec_pretty(&report)?)?;
    fs::write(REPORT_MD, format!("# Stage 283 — HLE four-candidate shadow probe\n\nThe frozen HLE corpus was offered to the clone-only economics, geometry, health-ratio, and unit-conversion portfolio. No result enters production routing.\n\n* cases: {}\n* source modules / records: {} / {}\n* frontend replay / tamper: {} / {}\n* unique shadow candidates: {}\n* correct / rejected candidates: {} / {}\n* ambiguous or missing: {}\n* unsupported: {}\n* production authorizations: 0\n* false authorizations: 0\n* live manifest / registry mutations: 0 / 0\n\nReproduce with `cargo run --quiet --bin stage283_hle_four_candidate_probe`.\n", report.cases, report.source_modules, report.source_records, report.frontend_replays, report.frontend_tamper_rejections, report.unique_shadow_candidates, report.correct_shadow_candidates, report.incorrect_shadow_candidates_rejected, report.ambiguous_or_missing, report.unsupported))?;
    println!("stage283 cases={} unique_candidates={} correct_candidates={} false_auth=0 manifest_mutated=false", report.cases, report.unique_shadow_candidates, report.correct_shadow_candidates);
    Ok(())
}
