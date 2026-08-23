//! Stage 415: semantic-coherence audit for the largest external families.
//!
//! Stage 414 used broad lexical signals.  This audit narrows geometry and
//! linear-equation candidates into explicit object/operation signatures and
//! reports recoverable inputs, target evidence, source diversity, and split
//! stability.  It does not claim that any signature is a capability family.

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use the_machine::curriculum::breadth_first_manifest;

const CORPUS_PATH: &str = "docs/stage389_page_aware_external_problem_dev.json";
const SEALED_MANIFEST_PATH: &str =
    "docs/holdouts/stage389_page_aware_external_problem_sealed_manifest.json";
const CORPUS_SHA256: &str = "92304d95521ac2ef4d49a08272f0b0cf79ac7225c9715d5ace9a667c822f6e03";
const SEALED_MANIFEST_SHA256: &str =
    "ac4227bff20decad585210b8e0d4c60401f6056092b718e99157e4f208a1b8de";

#[derive(Debug, Deserialize)]
struct Record {
    record_id: String,
    source_path: String,
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
enum Signature {
    LinearSystemSolve,
    LinearProgramming,
    InequalityConstraints,
    LineSlopeEquation,
    LinearWordProblem,
    TriangleArea,
    TriangleSideOrPerimeter,
    CircleMeasurement,
    RectangleMeasurement,
    TrapezoidMeasurement,
    VolumeMeasurement,
    SurfaceArea,
    CoordinateGeometry,
    AngleTrigonometry,
}

impl Signature {
    fn all() -> &'static [Signature] {
        &[
            Signature::LinearSystemSolve,
            Signature::LinearProgramming,
            Signature::InequalityConstraints,
            Signature::LineSlopeEquation,
            Signature::LinearWordProblem,
            Signature::TriangleArea,
            Signature::TriangleSideOrPerimeter,
            Signature::CircleMeasurement,
            Signature::RectangleMeasurement,
            Signature::TrapezoidMeasurement,
            Signature::VolumeMeasurement,
            Signature::SurfaceArea,
            Signature::CoordinateGeometry,
            Signature::AngleTrigonometry,
        ]
    }

    fn name(self) -> &'static str {
        match self {
            Signature::LinearSystemSolve => "linear_system_solve",
            Signature::LinearProgramming => "linear_programming",
            Signature::InequalityConstraints => "inequality_constraints",
            Signature::LineSlopeEquation => "line_slope_equation",
            Signature::LinearWordProblem => "linear_word_problem",
            Signature::TriangleArea => "triangle_area",
            Signature::TriangleSideOrPerimeter => "triangle_side_or_perimeter",
            Signature::CircleMeasurement => "circle_measurement",
            Signature::RectangleMeasurement => "rectangle_measurement",
            Signature::TrapezoidMeasurement => "trapezoid_measurement",
            Signature::VolumeMeasurement => "volume_measurement",
            Signature::SurfaceArea => "surface_area",
            Signature::CoordinateGeometry => "coordinate_geometry",
            Signature::AngleTrigonometry => "angle_trigonometry",
        }
    }
}

#[derive(Debug, Serialize)]
struct SignatureSummary {
    signature: Signature,
    candidates: usize,
    development: usize,
    validation: usize,
    distinct_sources: usize,
    with_two_or_more_numeric_tokens: usize,
    with_target_verb: usize,
    recoverable_candidate_count: usize,
    source_paths: Vec<String>,
    sample_prompt_hashes: Vec<String>,
    structurally_repeated: bool,
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
    answer_keys_read: usize,
    plaintext_answers_read: usize,
    production_authorizations: usize,
    false_authorizations: usize,
    sealed_manifest_path: &'static str,
    sealed_manifest_sha256: String,
    declared_sealed_manifest_sha256: &'static str,
    sealed_manifest_records: usize,
    signature_summaries: Vec<SignatureSummary>,
    overlapping_signature_memberships: usize,
    unclassified_records: usize,
    manifest_sha256_before: String,
    manifest_sha256_after: String,
    manifest_unchanged: bool,
    report_sha256: String,
}

fn digest_bytes(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

fn digest<T: Serialize>(value: &T) -> String {
    digest_bytes(&serde_json::to_vec(value).expect("audit serializes"))
}

fn has_any(text: &str, terms: &[&str]) -> bool {
    terms.iter().any(|term| text.contains(term))
}

fn numeric_tokens(text: &str) -> usize {
    text.split_whitespace()
        .filter(|token| token.chars().any(|ch| ch.is_ascii_digit()))
        .count()
}

fn target_verb(text: &str) -> bool {
    has_any(
        text,
        &[
            "find ",
            "calculate ",
            "compute ",
            "determine ",
            "solve ",
            "evaluate ",
            "write ",
            "identify ",
            "show ",
            "prove ",
            "classify ",
        ],
    )
}

fn matches(signature: Signature, text: &str) -> bool {
    match signature {
        Signature::LinearSystemSolve => {
            has_any(text, &["system of equations", "simultaneous equations"])
                && has_any(text, &["solve", "solution"])
        }
        Signature::LinearProgramming => {
            has_any(text, &["linear programming", "constraint inequalities"])
        }
        Signature::InequalityConstraints => {
            has_any(text, &["inequality", "inequalities", "constraints"])
        }
        Signature::LineSlopeEquation => has_any(
            text,
            &[
                "slope",
                "equation of the line",
                "equation of a line",
                "line passing",
            ],
        ),
        Signature::LinearWordProblem => {
            has_any(text, &["solve", "equation"])
                && !has_any(
                    text,
                    &["system of equations", "quadratic", "linear programming"],
                )
        }
        Signature::TriangleArea => has_any(text, &["triangle"]) && has_any(text, &["area"]),
        Signature::TriangleSideOrPerimeter => {
            has_any(text, &["triangle"]) && has_any(text, &["side", "perimeter", "pythagorean"])
        }
        Signature::CircleMeasurement => {
            has_any(text, &["circle", "circumference"])
                && has_any(text, &["area", "radius", "diameter", "circumference"])
        }
        Signature::RectangleMeasurement => {
            has_any(text, &["rectangle"])
                && has_any(text, &["area", "perimeter", "length", "width"])
        }
        Signature::TrapezoidMeasurement => {
            has_any(text, &["trapezoid"]) && has_any(text, &["area", "height", "base"])
        }
        Signature::VolumeMeasurement => {
            has_any(text, &["volume"])
                && has_any(text, &["cylinder", "prism", "sphere", "cone", "solid"])
        }
        Signature::SurfaceArea => has_any(text, &["surface area"]),
        Signature::CoordinateGeometry => {
            has_any(
                text,
                &["coordinate", "point", "distance", "ellipse", "parabola"],
            ) && has_any(text, &["graph", "equation", "distance", "coordinate"])
        }
        Signature::AngleTrigonometry => has_any(
            text,
            &["angle", "sine", "cosine", "tangent", "trigonometric"],
        ),
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let corpus_bytes = fs::read(CORPUS_PATH)?;
    let corpus_file_sha256 = digest_bytes(&corpus_bytes);
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
    let sealed_value: serde_json::Value = serde_json::from_slice(&sealed_bytes)?;
    let sealed_manifest_records = sealed_value
        .as_array()
        .map(Vec::len)
        .or_else(|| {
            sealed_value
                .get("records")
                .and_then(|value| value.as_array())
                .map(Vec::len)
        })
        .unwrap_or(0);
    assert_eq!(sealed_manifest_records, 1091);

    let manifest_sha256_before = breadth_first_manifest().replay_hash();
    let mut membership_count = 0;
    let mut matched_records = BTreeSet::new();
    let mut summaries = Vec::new();
    for signature in Signature::all() {
        let mut candidates = Vec::new();
        let mut sources = BTreeSet::new();
        let mut source_paths = BTreeSet::new();
        let mut numeric = 0;
        let mut target = 0;
        let mut recoverable = 0;
        for record in &records {
            let text = record.prompt.to_ascii_lowercase();
            if matches(*signature, &text) {
                candidates.push(record);
                sources.insert(record.source_sha256.clone());
                source_paths.insert(record.source_path.clone());
                matched_records.insert(record.record_id.clone());
                let enough_numeric = numeric_tokens(&text) >= 2;
                numeric += usize::from(enough_numeric);
                let has_target = target_verb(&text);
                target += usize::from(has_target);
                recoverable += usize::from(enough_numeric && has_target);
            }
        }
        membership_count += candidates.len();
        let development = candidates
            .iter()
            .filter(|record| record.split == "development")
            .count();
        let validation = candidates
            .iter()
            .filter(|record| record.split == "validation")
            .count();
        let structurally_repeated = candidates.len() >= 8
            && development > 0
            && validation > 0
            && sources.len() >= 2
            && recoverable * 2 >= candidates.len();
        summaries.push(SignatureSummary {
            signature: *signature,
            candidates: candidates.len(),
            development,
            validation,
            distinct_sources: sources.len(),
            with_two_or_more_numeric_tokens: numeric,
            with_target_verb: target,
            recoverable_candidate_count: recoverable,
            source_paths: source_paths.into_iter().collect(),
            sample_prompt_hashes: candidates
                .iter()
                .take(8)
                .map(|record| record.prompt_sha256.clone())
                .collect(),
            structurally_repeated,
        });
    }
    let manifest_sha256_after = breadth_first_manifest().replay_hash();
    assert_eq!(manifest_sha256_before, manifest_sha256_after);
    let mut report = Report {
        schema: "stage415-semantic-coherence-audit-v1",
        corpus_path: CORPUS_PATH,
        corpus_file_sha256,
        declared_corpus_sha256: CORPUS_SHA256,
        source_records,
        quality_clean_records: records.len(),
        quality_rejected_records,
        answer_keys_read: 0,
        plaintext_answers_read: 0,
        production_authorizations: 0,
        false_authorizations: 0,
        sealed_manifest_path: SEALED_MANIFEST_PATH,
        sealed_manifest_sha256,
        declared_sealed_manifest_sha256: SEALED_MANIFEST_SHA256,
        sealed_manifest_records,
        signature_summaries: summaries,
        overlapping_signature_memberships: membership_count.saturating_sub(matched_records.len()),
        unclassified_records: records.len().saturating_sub(matched_records.len()),
        manifest_sha256_before,
        manifest_sha256_after,
        manifest_unchanged: true,
        report_sha256: String::new(),
    };
    report.report_sha256 = digest(&report);
    fs::write(
        "docs/stage415_semantic_coherence_audit.json",
        serde_json::to_vec_pretty(&report)?,
    )?;
    let mut markdown = format!(
        "# Stage 415 — semantic-coherence audit\n\n- source records / clean / rejected: {} / {} / {}\n- signature memberships / overlap / unclassified: {} / {} / {}\n- answer keys read / production authorizations / false authorizations: 0 / 0 / 0\n- sealed manifest records: {} (prompt text not consumed)\n- manifest unchanged: true\n\n| Signature | Candidates | Dev | Val | Sources | Recoverable | Structurally repeated |\n|---|---:|---:|---:|---:|---:|:---:|\n",
        report.source_records, report.quality_clean_records, report.quality_rejected_records,
        membership_count, report.overlapping_signature_memberships, report.unclassified_records,
        report.sealed_manifest_records,
    );
    for summary in &report.signature_summaries {
        markdown.push_str(&format!(
            "| `{}` | {} | {} | {} | {} | {} | {} |\n",
            summary.signature.name(),
            summary.candidates,
            summary.development,
            summary.validation,
            summary.distinct_sources,
            summary.recoverable_candidate_count,
            summary.structurally_repeated,
        ));
    }
    markdown.push_str("\n`structurally_repeated` is a screening signal only; it is not a capability contract or semantic proof. Any selected family requires independent exercises, boundary cases, and downstream validation.\n");
    fs::write("docs/stage415_semantic_coherence_audit.md", markdown)?;
    println!(
        "Stage 415 — clean={} signatures={} overlap={} unclassified={}",
        report.quality_clean_records,
        membership_count,
        report.overlapping_signature_memberships,
        report.unclassified_records
    );
    Ok(())
}
