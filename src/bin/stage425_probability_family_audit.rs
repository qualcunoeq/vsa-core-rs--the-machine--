//! Stage 425: answer-key-blind audit of external finite-probability signals.
//!
//! The existing probability frontend consumes an explicit finite distribution.
//! This audit checks whether naturally authored source records contain a
//! repeated experiment description that could be lowered to that artifact.
//! It does not enumerate outcomes, compute answers, read answer keys, or add
//! a production route.

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
    UniformDie,
    LabeledDie,
    CardOrTileDraw,
    RepeatedGame,
    EmpiricalOrSubjective,
    CoinExperiment,
    OtherFiniteExperiment,
}

impl Family {
    fn name(self) -> &'static str {
        match self {
            Self::UniformDie => "uniform_die",
            Self::LabeledDie => "labeled_die",
            Self::CardOrTileDraw => "card_or_tile_draw",
            Self::RepeatedGame => "repeated_game",
            Self::EmpiricalOrSubjective => "empirical_or_subjective",
            Self::CoinExperiment => "coin_experiment",
            Self::OtherFiniteExperiment => "other_finite_experiment",
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
    explicit_experiment_definition: usize,
    explicit_event_target: usize,
    literal_distribution_frontend_inputs: usize,
    prompt_hashes: Vec<String>,
}

#[derive(Debug, Serialize)]
struct Candidate {
    record_id: String,
    split: String,
    prompt_sha256: String,
    family: Family,
    typed_candidate: String,
    existing_frontend: String,
    missing_handoff: String,
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
    probability_signal_records: usize,
    family_summaries: Vec<FamilySummary>,
    uniform_or_labeled_candidates: Vec<Candidate>,
    promising_family_count: usize,
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
    digest_bytes(&serde_json::to_vec(value).expect("stage 425 report serializes"))
}

fn has_any(text: &str, markers: &[&str]) -> bool {
    markers.iter().any(|marker| text.contains(marker))
}

fn probability_signal(text: &str) -> bool {
    has_any(
        text,
        &["probability", "probabilities", "coin is tossed", "dice"],
    )
}

fn target_present(text: &str) -> bool {
    has_any(
        text,
        &[
            "what is",
            "find ",
            "calculate ",
            "compute ",
            "determine ",
            "decide ",
            "what are the odds",
        ],
    )
}

fn explicit_experiment(text: &str) -> bool {
    has_any(
        text,
        &[
            "standard 6-sided",
            "standard 12-sided",
            "6-sided die with",
            "special 6-sided die",
            "standard 52-card",
            "deck containing",
            "bag contains",
            "coin is tossed",
        ],
    )
}

fn family(text: &str) -> Family {
    let lower = text.to_ascii_lowercase();
    if lower.contains("empirical probability")
        || lower.contains("chance that")
        || lower.contains("likely determined")
        || lower.contains("estimates there")
    {
        Family::EmpiricalOrSubjective
    } else if lower.contains("card") || lower.contains("deck") || lower.contains("scrabble") {
        Family::CardOrTileDraw
    } else if lower.contains("special")
        || lower.contains("labeled")
        || has_any(&lower, &["orange faces", "green faces", "blue faces"])
    {
        Family::LabeledDie
    } else if lower.contains("die") || lower.contains("dice") {
        Family::UniformDie
    } else if lower.contains("coin") {
        Family::CoinExperiment
    } else if lower.contains("series") || lower.contains("team a") {
        Family::RepeatedGame
    } else {
        Family::OtherFiniteExperiment
    }
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
    let selected: Vec<Record> = records
        .into_iter()
        .filter(|record| probability_signal(&record.prompt))
        .collect();
    assert_eq!(selected.len(), 108);

    let mut grouped: BTreeMap<Family, Vec<&Record>> = BTreeMap::new();
    for record in &selected {
        grouped
            .entry(family(&record.prompt))
            .or_default()
            .push(record);
    }
    let mut summaries = Vec::new();
    let mut uniform_or_labeled_candidates = Vec::new();
    for current_family in [
        Family::UniformDie,
        Family::LabeledDie,
        Family::CardOrTileDraw,
        Family::RepeatedGame,
        Family::EmpiricalOrSubjective,
        Family::CoinExperiment,
        Family::OtherFiniteExperiment,
    ] {
        let candidates = grouped.get(&current_family).cloned().unwrap_or_default();
        let mut source_hashes = BTreeSet::new();
        let mut definitions = 0;
        let mut targets = 0;
        let mut literal_inputs = 0;
        for record in &candidates {
            let lower = record.prompt.to_ascii_lowercase();
            source_hashes.insert(record.source_sha256.clone());
            definitions += usize::from(explicit_experiment(&lower));
            targets += usize::from(target_present(&lower));
            literal_inputs +=
                usize::from(lower.contains("outcomes=[") && lower.contains("probabilities=["));
            if matches!(current_family, Family::UniformDie | Family::LabeledDie)
                && explicit_experiment(&lower)
                && target_present(&lower)
            {
                uniform_or_labeled_candidates.push(Candidate {
                    record_id: record.record_id.clone(),
                    split: record.split.clone(),
                    prompt_sha256: record.prompt_sha256.clone(),
                    family: current_family,
                    typed_candidate: "finite_uniform_experiment_event".into(),
                    existing_frontend: "none".into(),
                    missing_handoff: "the existing frontend requires explicit outcomes and probabilities; natural-language sample-space and event predicates are not lowered".into(),
                });
            }
        }
        summaries.push(FamilySummary {
            family: current_family,
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
            explicit_experiment_definition: definitions,
            explicit_event_target: targets,
            literal_distribution_frontend_inputs: literal_inputs,
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
        schema: "stage425-probability-family-audit-v1",
        corpus_path: CORPUS_PATH,
        corpus_file_sha256,
        declared_corpus_sha256: CORPUS_SHA256,
        source_records,
        quality_clean_records: 2105,
        quality_rejected_records,
        probability_signal_records: selected.len(),
        family_summaries: summaries,
        uniform_or_labeled_candidates,
        promising_family_count: 1,
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
        "docs/stage425_probability_family_audit.json",
        serde_json::to_vec_pretty(&report)?,
    )?;

    let mut markdown = format!(
        "# Stage 425 — answer-key-blind probability-family audit\n\n- source records / quality-clean / rejected: {} / {} / {}\n- probability-signal records: {}\n- promising repeated family: finite uniform/labeled experiment events\n- candidate records / existing typed frontends: {} / 0\n- contracts proposed: 0\n- answer keys / plaintext answers / production authorizations / false authorizations: 0 / 0 / 0 / 0\n- sealed manifest records: {} (prompt text not consumed)\n- manifest unchanged: true\n\n| Family | Cases | Dev | Val | Sources | Explicit experiment | Event target | Literal distribution inputs |\n|---|---:|---:|---:|---:|---:|---:|---:|\n",
        source_records,
        report.quality_clean_records,
        quality_rejected_records,
        report.probability_signal_records,
        report.uniform_or_labeled_candidates.len(),
        sealed_manifest_records,
    );
    for summary in &report.family_summaries {
        markdown.push_str(&format!(
            "| `{}` | {} | {} | {} | {} | {} | {} | {} |\n",
            summary.family.name(),
            summary.candidates,
            summary.development,
            summary.validation,
            summary.distinct_sources,
            summary.explicit_experiment_definition,
            summary.explicit_event_target,
            summary.literal_distribution_frontend_inputs,
        ));
    }
    markdown.push_str(
        "\nThe repeated uniform/labeled experiment family is a source-selection candidate, not yet a capability contract. The next phase must build an independent finite-experiment corpus and validate sample-space/event lowering before any external alignment or promotion.\n",
    );
    fs::write("docs/stage425_probability_family_audit.md", markdown)?;
    println!(
        "Stage 425 — probability_records={} families={} uniform_or_labeled_candidates={} frontend_routes=0",
        report.probability_signal_records,
        report.family_summaries.len(),
        report.uniform_or_labeled_candidates.len(),
    );
    Ok(())
}
