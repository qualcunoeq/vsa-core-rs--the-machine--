//! Stage 414: answer-key-blind structural family census.
//!
//! This is a selection audit, not a solver.  It searches only the quality-clean
//! Stage 389 candidates for repeated request/input signatures, preserving source
//! lineage and development/validation separation.  No family is promoted and
//! no answer alignment is consumed.

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
struct PageRecord {
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
enum Family {
    UnitConversion,
    FractionPercent,
    MeanAverage,
    LinearEquations,
    Quadratic,
    Probability,
    Combinatorics,
    CalculusDerivative,
    CalculusIntegral,
    Matrix,
    RecurrenceSequence,
    GeometryMeasurement,
    LogarithmExponential,
    Statistics,
}

impl Family {
    fn all() -> &'static [Family] {
        &[
            Family::UnitConversion,
            Family::FractionPercent,
            Family::MeanAverage,
            Family::LinearEquations,
            Family::Quadratic,
            Family::Probability,
            Family::Combinatorics,
            Family::CalculusDerivative,
            Family::CalculusIntegral,
            Family::Matrix,
            Family::RecurrenceSequence,
            Family::GeometryMeasurement,
            Family::LogarithmExponential,
            Family::Statistics,
        ]
    }

    fn name(self) -> &'static str {
        match self {
            Family::UnitConversion => "unit_conversion",
            Family::FractionPercent => "fraction_percent",
            Family::MeanAverage => "mean_average",
            Family::LinearEquations => "linear_equations",
            Family::Quadratic => "quadratic",
            Family::Probability => "probability",
            Family::Combinatorics => "combinatorics",
            Family::CalculusDerivative => "calculus_derivative",
            Family::CalculusIntegral => "calculus_integral",
            Family::Matrix => "matrix",
            Family::RecurrenceSequence => "recurrence_sequence",
            Family::GeometryMeasurement => "geometry_measurement",
            Family::LogarithmExponential => "logarithm_exponential",
            Family::Statistics => "statistics",
        }
    }
}

#[derive(Debug, Serialize)]
struct FamilySummary {
    family: Family,
    candidate_records: usize,
    development_records: usize,
    validation_records: usize,
    distinct_sources: usize,
    records_with_two_or_more_numeric_tokens: usize,
    records_with_target_verb: usize,
    source_hashes: Vec<String>,
    source_paths: Vec<String>,
    samples: Vec<String>,
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
    family_summaries: Vec<FamilySummary>,
    overlapping_family_records: usize,
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
    digest_bytes(&serde_json::to_vec(value).expect("census serializes"))
}

fn has_any(text: &str, terms: &[&str]) -> bool {
    terms.iter().any(|term| text.contains(term))
}

fn family_match(family: Family, text: &str) -> bool {
    match family {
        Family::UnitConversion => {
            has_any(
                text,
                &[
                    "convert the units",
                    "convert between",
                    "unit conversion",
                    "convert ",
                ],
            ) && has_any(
                text,
                &[
                    "inch", "foot", "feet", "meter", "mile", "ounce", "gram", "pound",
                ],
            )
        }
        Family::FractionPercent => has_any(
            text,
            &[
                "fraction to a percent",
                "percent to a fraction",
                "fraction as a percent",
            ],
        ),
        Family::MeanAverage => has_any(text, &[" arithmetic mean", " average ", "mean "]),
        Family::LinearEquations => has_any(
            text,
            &["system of equations", "linear equation", "linear equations"],
        ),
        Family::Quadratic => has_any(text, &["quadratic", "quadratic formula"]),
        Family::Probability => has_any(
            text,
            &["probability", "probabilities", "coin is tossed", "dice"],
        ),
        Family::Combinatorics => has_any(
            text,
            &["how many ways", "combination", "permutation", "choose "],
        ),
        Family::CalculusDerivative => {
            has_any(text, &["derivative", "differentiate", "tangent line"])
        }
        Family::CalculusIntegral => has_any(text, &["integral", "integrate", "antiderivative"]),
        Family::Matrix => has_any(text, &["matrix", "matrices", "eigenvalue", "eigenvector"]),
        Family::RecurrenceSequence => has_any(
            text,
            &[
                "recursive formula",
                "recurrence",
                "arithmetic sequence",
                "geometric sequence",
            ],
        ),
        Family::GeometryMeasurement => has_any(
            text,
            &[
                "area",
                "volume",
                "perimeter",
                "circumference",
                "surface area",
            ],
        ),
        Family::LogarithmExponential => has_any(
            text,
            &["logarithmic", "logarithm", "exponential", "natural log"],
        ),
        Family::Statistics => has_any(
            text,
            &[
                "standard deviation",
                "sample",
                "hypothesis",
                "confidence interval",
                "distribution",
            ],
        ),
    }
}

fn numeric_token_count(text: &str) -> usize {
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
            "convert ",
            "identify ",
            "write ",
            "show ",
            "prove ",
            "classify ",
        ],
    )
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let corpus_bytes = fs::read(CORPUS_PATH)?;
    let corpus_file_sha256 = digest_bytes(&corpus_bytes);
    let all_records: Vec<PageRecord> = serde_json::from_slice(&corpus_bytes)?;
    let source_records = all_records.len();
    let quality_rejected_records = all_records
        .iter()
        .filter(|record| !record.quality_flags.is_empty())
        .count();
    let records: Vec<PageRecord> = all_records
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
    let mut summaries = Vec::new();
    let mut matched_record_counts = BTreeMap::<String, usize>::new();
    let mut all_matched_records = BTreeSet::new();
    for family in Family::all() {
        let mut records_for_family = Vec::new();
        let mut sources = BTreeSet::new();
        let mut source_paths = BTreeSet::new();
        let mut numeric = 0;
        let mut targets = 0;
        for record in &records {
            let text = record.prompt.to_ascii_lowercase();
            if family_match(*family, &text) {
                records_for_family.push(record);
                sources.insert(record.source_sha256.clone());
                source_paths.insert(record.source_path.clone());
                all_matched_records.insert(record.record_id.clone());
                numeric += usize::from(numeric_token_count(&text) >= 2);
                targets += usize::from(target_verb(&text));
            }
        }
        let name = family.name().to_owned();
        matched_record_counts.insert(name, records_for_family.len());
        summaries.push(FamilySummary {
            family: *family,
            candidate_records: records_for_family.len(),
            development_records: records_for_family
                .iter()
                .filter(|record| record.split == "development")
                .count(),
            validation_records: records_for_family
                .iter()
                .filter(|record| record.split == "validation")
                .count(),
            distinct_sources: sources.len(),
            records_with_two_or_more_numeric_tokens: numeric,
            records_with_target_verb: targets,
            source_hashes: sources.into_iter().collect(),
            source_paths: source_paths.into_iter().collect(),
            samples: records_for_family
                .iter()
                .take(5)
                .map(|record| record.prompt_sha256.clone())
                .collect(),
        });
    }
    let manifest_sha256_after = breadth_first_manifest().replay_hash();
    assert_eq!(manifest_sha256_before, manifest_sha256_after);
    let total_memberships: usize = matched_record_counts.values().sum();
    let mut report = Report {
        schema: "stage414-external-family-census-v1",
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
        family_summaries: summaries,
        overlapping_family_records: total_memberships.saturating_sub(all_matched_records.len()),
        unclassified_records: records.len().saturating_sub(all_matched_records.len()),
        manifest_sha256_before,
        manifest_sha256_after,
        manifest_unchanged: true,
        report_sha256: String::new(),
    };
    report.report_sha256 = digest(&report);
    fs::write(
        "docs/stage414_external_family_census.json",
        serde_json::to_vec_pretty(&report)?,
    )?;
    let mut markdown = format!(
        "# Stage 414 — answer-key-blind external family census\n\n- source records / clean / rejected: {} / {} / {}\n- family memberships / overlapping records / unclassified: {} / {} / {}\n- answer keys read / production authorizations / false authorizations: 0 / 0 / 0\n- sealed manifest records: {} (prompt text not consumed)\n- manifest unchanged: true\n\n",
        report.source_records, report.quality_clean_records, report.quality_rejected_records,
        total_memberships, report.overlapping_family_records, report.unclassified_records,
        report.sealed_manifest_records,
    );
    markdown.push_str("| Family | Candidates | Development | Validation | Sources | ≥2 numeric tokens | Target verb |\n|---|---:|---:|---:|---:|---:|---:|\n");
    for summary in &report.family_summaries {
        markdown.push_str(&format!(
            "| `{}` | {} | {} | {} | {} | {} | {} |\n",
            summary.family.name(),
            summary.candidate_records,
            summary.development_records,
            summary.validation_records,
            summary.distinct_sources,
            summary.records_with_two_or_more_numeric_tokens,
            summary.records_with_target_verb,
        ));
    }
    markdown.push_str("\nThis census proposes no capability and reads no answer alignment. Candidate families require a separate semantic-coherence and independent-exercise gate.\n");
    fs::write("docs/stage414_external_family_census.md", markdown)?;
    println!(
        "Stage 414 — clean={} memberships={} unclassified={}",
        report.quality_clean_records, total_memberships, report.unclassified_records
    );
    Ok(())
}
