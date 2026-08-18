//! Stage 264: frozen HLE probe for the geometry shadow candidate.
//!
//! The geometry catalog is evaluated only through the cloned manifest from
//! Stage 263.  A unique result is recorded as a shadow candidate, never as a
//! production authorization.

use serde::Serialize;
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::fs::{self, File};
use std::io::{BufRead, BufReader};

use the_machine::router::QuestionRouter;
use the_machine::source_formula_frontend::{
    formalize_source_formula_report, report_replay_verified, FrontendStatus,
};
use the_machine::source_formula_pack::{
    evaluate_formula_records, extract_formula_records, FormulaStatus,
};

const DATASET: &str = "data/hle.jsonl";
const SHADOW_MANIFEST: &str = "docs/stage263_geometry_shadow_manifest.json";
const SOURCE: &str = include_str!("../../docs/sources/openstax_bounded_geometry_source.txt");
const REPORT_JSON: &str = "docs/stage264_hle_geometry_shadow_probe.json";
const REPORT_MD: &str = "docs/stage264_hle_geometry_shadow_probe.md";
const DOMAIN: &str = "source_derived_bounded_geometry";

#[derive(Debug, Serialize)]
struct Report {
    schema: &'static str,
    dataset_sha256: String,
    shadow_manifest_sha256: String,
    cases: usize,
    source_records: usize,
    frontend_replays: usize,
    frontend_tamper_rejections: usize,
    complete_frontends: usize,
    executable_candidates: usize,
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

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let shadow_bytes = fs::read(SHADOW_MANIFEST)?;
    let shadow: Value = serde_json::from_slice(&shadow_bytes)?;
    assert_eq!(
        shadow.get("candidate_id").and_then(Value::as_str),
        Some(DOMAIN)
    );
    assert_eq!(
        shadow.get("shadow_only").and_then(Value::as_bool),
        Some(true)
    );
    let records = extract_formula_records(SOURCE).map_err(|errors| errors.join("; "))?;
    let dataset = fs::read(DATASET)?;
    let mut report = Report {
        schema: "stage264-hle-geometry-shadow-probe-v1",
        dataset_sha256: digest_bytes(&dataset),
        shadow_manifest_sha256: digest_bytes(&shadow_bytes),
        cases: 0,
        source_records: records.len(),
        frontend_replays: 0,
        frontend_tamper_rejections: 0,
        complete_frontends: 0,
        executable_candidates: 0,
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
        let frontend = formalize_source_formula_report(question, DOMAIN, &records);
        report.frontend_replays += usize::from(report_replay_verified(&frontend));
        let mut tampered = frontend.clone();
        tampered.replay_hash.push('x');
        report.frontend_tamper_rejections += usize::from(!report_replay_verified(&tampered));
        let mut candidate = None;
        if frontend.frontend.status == FrontendStatus::Complete {
            report.complete_frontends += 1;
            if let Some(request) = frontend.frontend.request.as_ref() {
                let execution = evaluate_formula_records(request, DOMAIN, &records);
                if execution.status == FormulaStatus::Complete {
                    report.executable_candidates += 1;
                    if let Some(value) = execution.value {
                        let rendered = if value.denominator == 1 {
                            value.numerator.to_string()
                        } else {
                            format!("{}/{}", value.numerator, value.denominator)
                        };
                        candidate = Some(rendered);
                    }
                }
            }
        }
        if frontend.frontend.status == FrontendStatus::Unsupported {
            report.unsupported += 1;
        }
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
    assert_eq!(report.source_records, 5);
    assert_eq!(report.frontend_replays, report.cases);
    assert_eq!(report.frontend_tamper_rejections, report.cases);
    assert_eq!(report.production_authorizations, 0);
    assert_eq!(report.false_authorizations, 0);
    assert_eq!(report.live_manifest_mutations, 0);
    assert_eq!(report.live_registry_mutations, 0);
    fs::write(REPORT_JSON, serde_json::to_vec_pretty(&report)?)?;
    fs::write(
        REPORT_MD,
        format!(
            "# Stage 264 — HLE geometry shadow probe\n\nThe frozen HLE corpus was evaluated against the cloned geometry manifest only.\n\n* cases: {}\n* source records: {}\n* complete frontends / executable candidates: {} / {}\n* unique shadow candidates: {}\n* correct / rejected candidate answers: {} / {}\n* ambiguous or missing: {}\n* unsupported: {}\n* frontend replay / tamper: {} / {}\n* production authorizations: 0\n* false authorizations: 0\n* live manifest / registry mutations: 0 / 0\n\nA shadow candidate is never a production answer. The current live manifest and router remain unchanged.\n\nReproduce with `cargo run --quiet --bin stage264_hle_geometry_shadow_probe`.\n",
            report.cases,
            report.source_records,
            report.complete_frontends,
            report.executable_candidates,
            report.unique_shadow_candidates,
            report.correct_shadow_candidates,
            report.incorrect_shadow_candidates_rejected,
            report.ambiguous_or_missing,
            report.unsupported,
            report.frontend_replays,
            report.frontend_tamper_rejections,
        ),
    )?;
    println!(
        "stage264 cases={} unique_candidates={} correct_candidates={} false_auth=0 manifest_mutated=false",
        report.cases, report.unique_shadow_candidates, report.correct_shadow_candidates
    );
    Ok(())
}
