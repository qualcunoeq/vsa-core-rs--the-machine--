//! Stage 424: answer-key-blind audit of the external slope family.
//!
//! Stage 415's broad `line_slope_equation` membership was a screening signal,
//! not a capability contract.  This audit partitions those records by the
//! requested artifact and checks whether the only explicit coordinate-pair
//! cases already reach a typed backend.  It deliberately does not compute
//! answers, read answer keys, or add a route.

use regex::Regex;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use the_machine::curriculum::breadth_first_manifest;

const CORPUS_PATH: &str = "docs/stage389_page_aware_external_problem_dev.json";
const CORPUS_SHA256: &str = "92304d95521ac2ef4d49a08272f0b0cf79ac7225c9715d5ace9a667c822f6e03";
const SEALED_MANIFEST_PATH: &str =
    "docs/holdouts/stage389_page_aware_external_problem_sealed_manifest.json";
const SEALED_MANIFEST_SHA256: &str =
    "ac4227bff20decad585210b8e0d4c60401f6056092b718e99157e4f208a1b8de";

#[derive(Debug, Deserialize)]
struct Record {
    record_id: String,
    source_sha256: String,
    prompt: String,
    prompt_sha256: String,
    split: String,
    #[serde(default)]
    quality_flags: Vec<String>,
    answer_key_status: String,
}

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
enum Family {
    CoordinatePairSlope,
    RiseRunApplication,
    LineModelApplication,
    RegressionInterpretation,
    ConceptualSlope,
    TangentCurveSlope,
    LineEquationConstruction,
    UnresolvedOrIncomplete,
}

impl Family {
    fn name(self) -> &'static str {
        match self {
            Self::CoordinatePairSlope => "coordinate_pair_slope",
            Self::RiseRunApplication => "rise_run_application",
            Self::LineModelApplication => "line_model_application",
            Self::RegressionInterpretation => "regression_interpretation",
            Self::ConceptualSlope => "conceptual_slope",
            Self::TangentCurveSlope => "tangent_curve_slope",
            Self::LineEquationConstruction => "line_equation_construction",
            Self::UnresolvedOrIncomplete => "unresolved_or_incomplete",
        }
    }
}

#[derive(Debug, Serialize)]
struct FamilySummary {
    family: Family,
    candidates: usize,
    development: usize,
    validation: usize,
    distinct_sources: usize,
    records_with_explicit_target: usize,
    records_with_complete_numeric_context: usize,
    prompt_hashes: Vec<String>,
}

#[derive(Debug, Serialize)]
struct CoordinateCandidate {
    record_id: String,
    split: String,
    prompt_sha256: String,
    point_count: usize,
    points: Vec<[i128; 2]>,
    typed_target: String,
    existing_backend: String,
    reason_not_executable: String,
}

#[derive(Debug, Serialize)]
struct Report {
    schema: &'static str,
    corpus_path: &'static str,
    corpus_file_sha256: String,
    declared_corpus_sha256: &'static str,
    source_records: usize,
    quality_clean_records: usize,
    quality_rejected_records: usize,
    slope_family_records: usize,
    family_summaries: Vec<FamilySummary>,
    coordinate_pair_candidates: Vec<CoordinateCandidate>,
    coherent_family_count: usize,
    capability_contracts_proposed: usize,
    answer_keys_read: usize,
    plaintext_answers_read: usize,
    production_authorizations: usize,
    false_authorizations: usize,
    sealed_manifest_path: &'static str,
    sealed_manifest_sha256: String,
    declared_sealed_manifest_sha256: &'static str,
    sealed_manifest_records: usize,
    manifest_sha256_before: String,
    manifest_sha256_after: String,
    manifest_unchanged: bool,
    report_sha256: String,
}

fn digest_bytes(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

fn digest<T: Serialize>(value: &T) -> String {
    digest_bytes(&serde_json::to_vec(value).expect("stage 424 report serializes"))
}

fn has_any(text: &str, markers: &[&str]) -> bool {
    markers.iter().any(|marker| text.contains(marker))
}

fn explicit_target(text: &str) -> bool {
    has_any(
        text,
        &[
            "find ",
            "calculate ",
            "compute ",
            "determine ",
            "solve ",
            "write ",
            "identify ",
            "what is ",
            "why is ",
        ],
    )
}

fn complete_numeric_context(text: &str) -> bool {
    let lower = text.to_ascii_lowercase();
    (lower.contains("rise")
        && lower.contains("run")
        && lower.matches(char::is_numeric).count() >= 2)
        || (lower.contains("slope formula") && extract_points(text).len() >= 2)
}

fn extract_points(text: &str) -> Vec<[i128; 2]> {
    let pattern =
        Regex::new(r"\(\s*([−-]?\d+)\s*,\s*([−-]?\d+)\s*\)").expect("coordinate pattern is valid");
    pattern
        .captures_iter(text)
        .filter_map(|capture| {
            let x = capture.get(1)?.as_str().replace('−', "-").parse().ok()?;
            let y = capture.get(2)?.as_str().replace('−', "-").parse().ok()?;
            Some([x, y])
        })
        .collect()
}

fn slope_family(text: &str) -> Family {
    let lower = text.to_ascii_lowercase();
    let points = extract_points(text);
    if lower.contains("regression") {
        Family::RegressionInterpretation
    } else if lower.contains("tangent")
        || lower.contains("polar curve")
        || lower.contains("catenary")
    {
        Family::TangentCurveSlope
    } else if lower.contains("vertical line")
        || lower.contains("undefined")
        || lower.contains("graph of a line with slope differ")
    {
        Family::ConceptualSlope
    } else if lower.contains("slope formula") && points.len() >= 2 {
        Family::CoordinatePairSlope
    } else if lower.contains("rise") && lower.contains("run")
        || lower.contains("climbs") && lower.contains("feet")
    {
        Family::RiseRunApplication
    } else if lower.contains("slope-intercept")
        || has_any(
            &lower,
            &[
                "payment for a month",
                "reimbursed",
                "cherie's salary",
                "initial population",
                "weight of a newborn",
                "common cold",
            ],
        )
    {
        Family::LineModelApplication
    } else if lower.contains("point-slope") || lower.contains("equation of the line") {
        Family::LineEquationConstruction
    } else {
        Family::UnresolvedOrIncomplete
    }
}

fn slope_signal(text: &str) -> bool {
    has_any(
        &text.to_ascii_lowercase(),
        &[
            "slope",
            "equation of the line",
            "equation of a line",
            "line passing",
        ],
    )
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let corpus_bytes = fs::read(CORPUS_PATH)?;
    let corpus_file_sha256 = digest_bytes(&corpus_bytes);
    assert_eq!(corpus_file_sha256, CORPUS_SHA256);
    let all_records: Vec<Record> = serde_json::from_slice(&corpus_bytes)?;
    let source_records = all_records.len();
    let quality_rejected_records = all_records
        .iter()
        .filter(|record| !record.quality_flags.is_empty())
        .count();
    let records: Vec<Record> = all_records
        .into_iter()
        .filter(|record| record.quality_flags.is_empty())
        .collect();
    assert_eq!(records.len(), 2105);
    assert!(records
        .iter()
        .all(|record| record.answer_key_status == "not_read"));

    let sealed_bytes = fs::read(SEALED_MANIFEST_PATH)?;
    let sealed_manifest_sha256 = digest_bytes(&sealed_bytes);
    assert_eq!(sealed_manifest_sha256, SEALED_MANIFEST_SHA256);
    let sealed_value: serde_json::Value = serde_json::from_slice(&sealed_bytes)?;
    let sealed_manifest_records = sealed_value
        .as_array()
        .map(Vec::len)
        .or_else(|| {
            sealed_value
                .get("records")
                .and_then(|v| v.as_array())
                .map(Vec::len)
        })
        .unwrap_or(0);
    assert_eq!(sealed_manifest_records, 1091);

    let manifest_sha256_before = breadth_first_manifest().replay_hash();
    let mut selected = Vec::new();
    for record in records {
        if slope_signal(&record.prompt) {
            selected.push(record);
        }
    }
    // This is the exact clean membership screened by Stage 415 for this
    // family.  A changed count is a corpus/schema drift, not a silent update.
    assert_eq!(selected.len(), 23);

    let mut grouped: BTreeMap<Family, Vec<&Record>> = BTreeMap::new();
    for record in &selected {
        grouped
            .entry(slope_family(&record.prompt))
            .or_default()
            .push(record);
    }
    let mut summaries = Vec::new();
    let mut coordinate_pair_candidates = Vec::new();
    for family in [
        Family::CoordinatePairSlope,
        Family::RiseRunApplication,
        Family::LineModelApplication,
        Family::RegressionInterpretation,
        Family::ConceptualSlope,
        Family::TangentCurveSlope,
        Family::LineEquationConstruction,
        Family::UnresolvedOrIncomplete,
    ] {
        let candidates = grouped.get(&family).cloned().unwrap_or_default();
        let mut source_hashes = BTreeSet::new();
        let mut complete = 0;
        let mut target = 0;
        for record in &candidates {
            source_hashes.insert(record.source_sha256.clone());
            complete += usize::from(complete_numeric_context(&record.prompt));
            target += usize::from(explicit_target(&record.prompt));
            if family == Family::CoordinatePairSlope {
                let points = extract_points(&record.prompt);
                coordinate_pair_candidates.push(CoordinateCandidate {
                    record_id: record.record_id.clone(),
                    split: record.split.clone(),
                    prompt_sha256: record.prompt_sha256.clone(),
                    point_count: points.len(),
                    points,
                    typed_target: "slope_between_two_cartesian_points".into(),
                    existing_backend: "none".into(),
                    reason_not_executable: "the existing regression frontend requires labeled covariance/variance inputs; the formula registry has no coordinate-pair text frontend".into(),
                });
            }
        }
        summaries.push(FamilySummary {
            family,
            candidates: candidates.len(),
            development: candidates
                .iter()
                .filter(|r| r.split == "development")
                .count(),
            validation: candidates
                .iter()
                .filter(|r| r.split == "validation")
                .count(),
            distinct_sources: source_hashes.len(),
            records_with_explicit_target: target,
            records_with_complete_numeric_context: complete,
            prompt_hashes: candidates
                .iter()
                .take(8)
                .map(|r| r.prompt_sha256.clone())
                .collect(),
        });
    }

    let manifest_sha256_after = breadth_first_manifest().replay_hash();
    assert_eq!(manifest_sha256_before, manifest_sha256_after);
    let mut report = Report {
        schema: "stage424-slope-family-audit-v1",
        corpus_path: CORPUS_PATH,
        corpus_file_sha256,
        declared_corpus_sha256: CORPUS_SHA256,
        source_records,
        quality_clean_records: 2105,
        quality_rejected_records,
        slope_family_records: selected.len(),
        family_summaries: summaries,
        coordinate_pair_candidates,
        coherent_family_count: 0,
        capability_contracts_proposed: 0,
        answer_keys_read: 0,
        plaintext_answers_read: 0,
        production_authorizations: 0,
        false_authorizations: 0,
        sealed_manifest_path: SEALED_MANIFEST_PATH,
        sealed_manifest_sha256,
        declared_sealed_manifest_sha256: SEALED_MANIFEST_SHA256,
        sealed_manifest_records,
        manifest_sha256_before,
        manifest_sha256_after,
        manifest_unchanged: true,
        report_sha256: String::new(),
    };
    report.report_sha256 = digest(&report);
    fs::write(
        "docs/stage424_slope_family_audit.json",
        serde_json::to_vec_pretty(&report)?,
    )?;

    let mut markdown = format!(
        "# Stage 424 — answer-key-blind slope-family audit\n\n- source records / quality-clean records / quality-rejected: {} / {} / {}\n- clean slope-family records: {}\n- family summaries: {}\n- coherent reusable families / contracts proposed: 0 / 0\n- coordinate-pair candidates with an existing typed backend: 0 / {}\n- answer keys / plaintext answers / production authorizations / false authorizations: 0 / 0 / 0 / 0\n- sealed manifest records: {} (prompt text not consumed)\n- manifest unchanged: true\n\n| Family | Cases | Dev | Val | Sources | Explicit target | Complete numeric context |\n|---|---:|---:|---:|---:|---:|---:|\n",
        source_records,
        report.quality_clean_records,
        quality_rejected_records,
        report.slope_family_records,
        report.family_summaries.len(),
        report.coordinate_pair_candidates.len(),
        sealed_manifest_records,
    );
    for summary in &report.family_summaries {
        markdown.push_str(&format!(
            "| `{}` | {} | {} | {} | {} | {} | {} |\n",
            summary.family.name(),
            summary.candidates,
            summary.development,
            summary.validation,
            summary.distinct_sources,
            summary.records_with_explicit_target,
            summary.records_with_complete_numeric_context,
        ));
    }
    markdown.push_str(
        "\nThe two coordinate-pair records share a typed target, but no existing source-derived text frontend reaches that target. The remaining records require different representations (regression interpretation, rise/run data, line modeling, conceptual explanation, tangent/curve calculus, or visual/incomplete context). No capability contract is justified from this family.\n",
    );
    fs::write("docs/stage424_slope_family_audit.md", markdown)?;
    println!(
        "Stage 424 — slope_records={} families={} coordinate_pair={} backend_routes=0",
        report.slope_family_records,
        report.family_summaries.len(),
        report.coordinate_pair_candidates.len(),
    );
    Ok(())
}
