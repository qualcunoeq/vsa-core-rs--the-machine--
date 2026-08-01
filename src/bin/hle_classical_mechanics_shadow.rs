//! Phase 36 diagnostic HLE run for the shadow classical-mechanics pack.
//!
//! This scanner never calls the production router and never authorizes an HLE
//! answer.  It only tests whether frozen HLE questions contain a uniquely
//! grounded, in-scope mechanics request that could safely reach the Phase 34
//! pack.

use serde::Serialize;
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::env;
use std::fs;
use the_machine::classical_mechanics_pack::classical_mechanics_pack;

const DATASET: &str = "data/hle.jsonl";
const BASELINE_CORRECT_AUTHORIZED: usize = 2;

#[derive(Debug, Serialize)]
struct QuestionResult {
    id: Option<String>,
    category: String,
    question_sha256: String,
    exact_pack_aliases: Vec<String>,
    ambiguous_pack_aliases: Vec<String>,
    broad_mechanics_signal: bool,
    terminal_classification: String,
    reason: String,
    pack_route: String,
    replay_result: String,
}

#[derive(Debug, Serialize)]
struct Report {
    schema_version: String,
    dataset: String,
    dataset_sha256: String,
    pack_sha256: String,
    cases: usize,
    exact_law_mentions: usize,
    ambiguous_pack_alias_mentions: usize,
    broad_mechanics_signals: usize,
    uniquely_grounded_candidates: usize,
    grounding_failures: usize,
    outside_pack_cases: usize,
    pack_reached_cases: usize,
    shadow_correct_answers: usize,
    shadow_incorrect_answers: usize,
    false_authorizations: usize,
    replay_verified: usize,
    replay_failed: usize,
    production_router_mutated: bool,
    production_hle_score_changed: bool,
    baseline_correct_authorized: usize,
    shadow_score_note: String,
    class_counts: BTreeMap<String, usize>,
    records: Vec<QuestionResult>,
    method: String,
}

fn sha256(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

fn pack_sha(pack: &[the_machine::classical_mechanics_pack::MechanicsLaw]) -> String {
    sha256(&serde_json::to_vec(pack).expect("pack serializes"))
}

fn contains_any(text: &str, needles: &[&str]) -> bool {
    needles.iter().any(|needle| text.contains(needle))
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let bytes = fs::read(DATASET)?;
    let dataset_sha256 = sha256(&bytes);
    let pack = classical_mechanics_pack();
    let pack_sha256 = pack_sha(&pack);
    let aliases: Vec<(String, String)> = pack
        .iter()
        .flat_map(|law| {
            std::iter::once((law.law_id.clone(), law.law_id.to_ascii_lowercase())).chain(
                law.aliases
                    .iter()
                    .map(|alias| (alias.clone(), alias.to_ascii_lowercase())),
            )
        })
        .collect();
    let ambiguous_aliases = ["energy"];
    let broad_markers = [
        "force",
        "mass",
        "velocity",
        "acceleration",
        "momentum",
        "kinetic",
        "spring",
        "projectile",
        "mechanical energy",
    ];
    let advanced_markers = [
        "relativistic",
        "quantum",
        "perturbation",
        "integral",
        "tensor",
        "lagrangian",
        "angular",
        "fluid",
        "electron",
        "field theory",
        "thermodynamic",
        "water",
        "pump",
        "pipe",
        "pressure",
        "flow",
        "density",
        "tank",
        "mold",
        "manometer",
        "bernoulli",
        "velocity profile",
        "figure",
    ];
    let target_markers = [
        "calculate",
        "compute",
        "determine",
        "find",
        "what is",
        "evaluate",
    ];
    let mut records = Vec::new();
    let mut class_counts = BTreeMap::new();
    let mut exact_law_mentions = 0;
    let mut broad_mechanics_signals = 0;
    let mut uniquely_grounded_candidates = 0;
    let mut grounding_failures = 0;
    let mut outside_pack_cases = 0;
    let mut pack_reached_cases = 0;
    for line in String::from_utf8(bytes.clone())?.lines() {
        if line.trim().is_empty() {
            continue;
        }
        let entry: Value = serde_json::from_str(line)?;
        let question = entry.get("question").and_then(Value::as_str).unwrap_or("");
        let lower = question.to_ascii_lowercase();
        let ambiguous_pack_aliases: Vec<String> = aliases
            .iter()
            .filter(|(_, normalized)| {
                ambiguous_aliases
                    .iter()
                    .any(|ambiguous| normalized == ambiguous)
                    && lower.contains(normalized)
            })
            .map(|(original, _)| original.clone())
            .collect();
        let exact_pack_aliases: Vec<String> = aliases
            .iter()
            .filter(|(_, normalized)| {
                lower.contains(normalized)
                    && !ambiguous_aliases
                        .iter()
                        .any(|ambiguous| normalized == ambiguous)
            })
            .map(|(original, _)| original.clone())
            .collect();
        let mut ambiguous_pack_aliases = ambiguous_pack_aliases;
        ambiguous_pack_aliases.sort();
        ambiguous_pack_aliases.dedup();
        let mut exact_pack_aliases = exact_pack_aliases;
        exact_pack_aliases.sort();
        exact_pack_aliases.dedup();
        let broad_signal = contains_any(&lower, &broad_markers);
        let has_target = contains_any(&lower, &target_markers);
        let numeric_count = lower
            .chars()
            .filter(|character| character.is_ascii_digit())
            .count();
        let advanced = contains_any(&lower, &advanced_markers);
        let (classification, reason, route) = if exact_pack_aliases.len() == 1
            && has_target
            && numeric_count >= 2
            && !advanced
        {
            uniquely_grounded_candidates += 1;
            pack_reached_cases += 1;
            (
                "grounded_but_binding_unavailable".to_string(),
                "unique pack alias found, but this diagnostic run does not infer numeric bindings"
                    .to_string(),
                "classical_mechanics_shadow".to_string(),
            )
        } else if !exact_pack_aliases.is_empty() {
            exact_law_mentions += 1;
            grounding_failures += 1;
            (
                "language_or_domain_grounding_failure".to_string(),
                "pack alias is present, but local wording does not establish a unique in-scope numerical law request".to_string(),
                "not_reached".to_string(),
            )
        } else if !ambiguous_pack_aliases.is_empty() {
            grounding_failures += 1;
            (
                "ambiguous_pack_alias".to_string(),
                "generic pack alias is overloaded; no law record selected".to_string(),
                "not_reached".to_string(),
            )
        } else if broad_signal {
            broad_mechanics_signals += 1;
            outside_pack_cases += 1;
            (
                "mechanics_signal_outside_pack".to_string(),
                "broad mechanics vocabulary without an exact pack law; no inference permitted"
                    .to_string(),
                "not_reached".to_string(),
            )
        } else {
            (
                "no_mechanics_candidate".to_string(),
                "no pack signal".to_string(),
                "not_reached".to_string(),
            )
        };
        *class_counts.entry(classification.clone()).or_insert(0) += 1;
        records.push(QuestionResult {
            id: entry.get("id").and_then(Value::as_str).map(str::to_string),
            category: entry
                .get("category")
                .and_then(Value::as_str)
                .unwrap_or("uncategorized")
                .to_string(),
            question_sha256: sha256(question.as_bytes()),
            exact_pack_aliases,
            ambiguous_pack_aliases,
            broad_mechanics_signal: broad_signal,
            terminal_classification: classification,
            reason,
            pack_route: route,
            replay_result: "not_applicable_shadow_only".to_string(),
        });
    }
    // Exact aliases are counted once per question in the aggregate, not once
    // per alias, so an overloaded phrase cannot inflate coverage.
    exact_law_mentions = records
        .iter()
        .filter(|record| !record.exact_pack_aliases.is_empty())
        .count();
    let ambiguous_pack_alias_mentions = records
        .iter()
        .filter(|record| !record.ambiguous_pack_aliases.is_empty())
        .count();
    broad_mechanics_signals = records
        .iter()
        .filter(|record| record.broad_mechanics_signal)
        .count();
    let report = Report {
        schema_version: "phase36.hle.classical.mechanics.shadow.v1".into(),
        dataset: DATASET.into(),
        dataset_sha256,
        pack_sha256,
        cases: records.len(),
        exact_law_mentions,
        ambiguous_pack_alias_mentions,
        broad_mechanics_signals,
        uniquely_grounded_candidates,
        grounding_failures,
        outside_pack_cases,
        pack_reached_cases,
        shadow_correct_answers: 0,
        shadow_incorrect_answers: 0,
        false_authorizations: 0,
        replay_verified: 0,
        replay_failed: 0,
        production_router_mutated: false,
        production_hle_score_changed: false,
        baseline_correct_authorized: BASELINE_CORRECT_AUTHORIZED,
        shadow_score_note: "No HLE answer was authorized: the pack was reached only by diagnostic candidate classification, never by production routing or answer emission.".into(),
        class_counts,
        records,
        method: "frozen HLE scan; exact pack-law grounding only; shadow-only, non-authorizing, no router or registry mutation".into(),
    };
    let output = serde_json::to_string_pretty(&report)?;
    let path = env::args()
        .nth(1)
        .unwrap_or_else(|| "docs/phase36_hle_classical_mechanics_shadow.json".into());
    fs::write(&path, output)?;
    println!("phase36 report written to {path}");
    Ok(())
}
