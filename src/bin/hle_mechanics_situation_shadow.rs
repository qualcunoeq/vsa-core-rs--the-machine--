//! Phase 39 diagnostic HLE funnel for MechanicsSituationV1.
//!
//! This binary is deliberately shadow-only.  It runs the frozen HLE questions
//! through the situation formalizer and the unchanged classical-mechanics pack,
//! but never changes production routing or authorizes an HLE answer.

use serde::Serialize;
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::env;
use std::fs;
use the_machine::classical_mechanics_pack::{classical_mechanics_pack, MechanicsStatus};
use the_machine::mechanics_situation::{
    execute_mechanics_situation, formalize_mechanics_situation, replay_execution, replay_situation,
    SituationStatus,
};

const DATASET: &str = "data/hle.jsonl";
const BASELINE_CORRECT_AUTHORIZED: usize = 2;

#[derive(Debug, Serialize)]
struct QuestionResult {
    id: Option<String>,
    category: String,
    question_sha256: String,
    answer_sha256: String,
    situation_status: String,
    candidate_laws: Vec<String>,
    requested_output: Option<String>,
    pack_invoked: bool,
    pack_status: Option<String>,
    candidate_value: Option<f64>,
    reference_numeric_value: Option<f64>,
    reference_match: Option<bool>,
    first_failing_gate: String,
    reasons: Vec<String>,
    situation_replay: bool,
    execution_replay: bool,
    provenance_spans: usize,
}

#[derive(Debug, Serialize)]
struct Report {
    schema_version: String,
    dataset: String,
    dataset_sha256: String,
    pack_sha256: String,
    cases: usize,
    mechanics_situations_detected: usize,
    supported_subdomain: usize,
    complete_typed_situations: usize,
    unique_applicable_laws: usize,
    complete_bindings: usize,
    pack_invocations: usize,
    pack_complete_results: usize,
    candidate_answers: usize,
    reference_matches: usize,
    false_authorizations: usize,
    situation_replay_verified: usize,
    execution_replay_verified: usize,
    first_failure_counts: BTreeMap<String, usize>,
    mechanics_signal_first_failure_counts: BTreeMap<String, usize>,
    situation_status_counts: BTreeMap<String, usize>,
    registry_mutated: bool,
    production_router_mutated: bool,
    production_hle_score_changed: bool,
    baseline_correct_authorized: usize,
    shadow_score_note: String,
    records: Vec<QuestionResult>,
    method: String,
}

fn sha256(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

fn pack_sha() -> String {
    sha256(&serde_json::to_vec(&classical_mechanics_pack()).expect("pack serializes"))
}

fn status_name(status: &SituationStatus) -> &'static str {
    match status {
        SituationStatus::Unique => "unique",
        SituationStatus::Ambiguous => "ambiguous",
        SituationStatus::Missing => "missing",
        SituationStatus::Unsupported => "unsupported",
    }
}

fn mechanics_signal(text: &str) -> bool {
    let lower = text.to_ascii_lowercase();
    [
        "force",
        "mass",
        "velocity",
        "speed",
        "acceleration",
        "momentum",
        "kinetic",
        "spring",
        "displacement",
        "elastic",
        "mechanical energy",
        "projectile",
        "inertial",
    ]
    .iter()
    .any(|marker| lower.contains(marker))
}

fn unsupported_signal(text: &str) -> bool {
    let lower = text.to_ascii_lowercase();
    [
        "relativistic",
        "quantum",
        "rotation",
        "rotational",
        "fluid",
        "thermodynamic",
        "lagrangian",
        "tensor",
        "field theory",
        "bernoulli",
        "orbital",
    ]
    .iter()
    .any(|marker| lower.contains(marker))
}

fn numeric_tokens(text: &str) -> Vec<f64> {
    let mut values = Vec::new();
    let mut token = String::new();
    for character in text.chars() {
        if character.is_ascii_digit() || (character == '.' && !token.contains('.')) {
            token.push(character);
        } else if !token.is_empty() {
            if let Ok(value) = token.parse() {
                values.push(value);
            }
            token.clear();
        }
    }
    if let Ok(value) = token.parse() {
        values.push(value);
    }
    values
}

fn reference_numeric_value(answer: &str) -> Option<f64> {
    numeric_tokens(answer).into_iter().next()
}

fn close(left: f64, right: f64) -> bool {
    (left - right).abs() <= 1e-7 * (1.0 + left.abs().max(right.abs()))
}

fn first_failure(
    text: &str,
    situation: &the_machine::mechanics_situation::MechanicsSituation,
) -> String {
    let lower = text.to_ascii_lowercase();
    if situation.status == SituationStatus::Unsupported || unsupported_signal(text) {
        return "unsupported_mechanics_subdomain".into();
    }
    if situation
        .unresolved_assumptions
        .iter()
        .any(|reason| reason.contains("multi-body"))
    {
        return "multi_body_ambiguity".into();
    }
    if situation
        .unresolved_assumptions
        .iter()
        .any(|reason| reason.contains("multiple requested"))
    {
        return "requires_multi_law_composition".into();
    }
    if situation
        .unresolved_assumptions
        .iter()
        .any(|reason| reason.contains("vector direction"))
    {
        return "vector_or_frame_ambiguity".into();
    }
    if situation.unresolved_assumptions.iter().any(|reason| {
        reason.contains("frame") || reason.contains("spring model") || reason.contains("regime")
    }) {
        return "missing_assumption".into();
    }
    if situation.status == SituationStatus::Unique {
        return "none_complete_situation".into();
    }
    if situation.candidate_laws.is_empty() {
        if situation.requested_output.is_none()
            || !lower.contains("find")
                && !lower.contains("calculate")
                && !lower.contains("compute")
                && !lower.contains("determine")
                && !lower.contains("what")
        {
            return "target_not_groundable".into();
        }
        if mechanics_signal(text) {
            return "missing_required_quantities".into();
        }
        return "no_mechanics_candidate".into();
    }
    if situation.status == SituationStatus::Ambiguous {
        return "target_or_law_ambiguity".into();
    }
    "unsupported_or_unresolved".into()
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let bytes = fs::read(DATASET)?;
    let dataset_sha256 = sha256(&bytes);
    let pack_sha256 = pack_sha();
    let mut records = Vec::new();
    let mut first_failure_counts = BTreeMap::new();
    let mut mechanics_signal_first_failure_counts = BTreeMap::new();
    let mut situation_status_counts = BTreeMap::new();
    let mut mechanics_situations_detected = 0;
    let mut supported_subdomain = 0;
    let mut complete_typed_situations = 0;
    let mut unique_applicable_laws = 0;
    let mut complete_bindings = 0;
    let mut pack_invocations = 0;
    let mut pack_complete_results = 0;
    let mut candidate_answers = 0;
    let mut reference_matches = 0;
    let mut false_authorizations = 0;
    let mut situation_replay_verified = 0;
    let mut execution_replay_verified = 0;

    for line in String::from_utf8(bytes.clone())?
        .lines()
        .filter(|line| !line.trim().is_empty())
    {
        let entry: Value = serde_json::from_str(line)?;
        let question = entry.get("question").and_then(Value::as_str).unwrap_or("");
        let answer = entry.get("answer").and_then(Value::as_str).unwrap_or("");
        let situation = formalize_mechanics_situation(question);
        let execution = execute_mechanics_situation(&situation);
        let has_signal = mechanics_signal(question);
        mechanics_situations_detected += usize::from(has_signal);
        if has_signal && situation.status != SituationStatus::Unsupported {
            supported_subdomain += 1;
        }
        complete_typed_situations += usize::from(situation.status == SituationStatus::Unique);
        unique_applicable_laws += usize::from(situation.candidate_laws.len() == 1);
        let invoked = execution.mechanics_status.is_some();
        let complete = execution.mechanics_status == Some(MechanicsStatus::Complete);
        pack_invocations += usize::from(invoked);
        pack_complete_results += usize::from(complete);
        complete_bindings += usize::from(complete);
        let reference = reference_numeric_value(answer);
        let reference_match = match (execution.value, reference, complete) {
            (Some(candidate), Some(expected), true) => Some(close(candidate, expected)),
            _ => None,
        };
        candidate_answers += usize::from(execution.value.is_some() && complete);
        reference_matches += usize::from(reference_match == Some(true));
        false_authorizations += usize::from(complete && reference_match != Some(true));
        let situation_replay = replay_situation(&situation);
        let execution_replay = replay_execution(&execution);
        situation_replay_verified += usize::from(situation_replay);
        execution_replay_verified += usize::from(execution_replay);
        let failure = first_failure(question, &situation);
        *first_failure_counts.entry(failure.clone()).or_insert(0) += 1;
        if has_signal {
            *mechanics_signal_first_failure_counts
                .entry(failure.clone())
                .or_insert(0) += 1;
        }
        *situation_status_counts
            .entry(status_name(&situation.status).to_string())
            .or_insert(0) += 1;
        records.push(QuestionResult {
            id: entry.get("id").and_then(Value::as_str).map(str::to_string),
            category: entry
                .get("category")
                .and_then(Value::as_str)
                .unwrap_or("uncategorized")
                .into(),
            question_sha256: sha256(question.as_bytes()),
            answer_sha256: sha256(answer.as_bytes()),
            situation_status: status_name(&situation.status).into(),
            candidate_laws: situation.candidate_laws.clone(),
            requested_output: situation.requested_output.clone(),
            pack_invoked: invoked,
            pack_status: execution
                .mechanics_status
                .map(|status| format!("{status:?}")),
            candidate_value: execution.value,
            reference_numeric_value: reference,
            reference_match,
            first_failing_gate: failure,
            reasons: execution.reasons,
            situation_replay,
            execution_replay,
            provenance_spans: situation.provenance.len(),
        });
    }
    let report = Report {
        schema_version: "phase39.hle.mechanics.situation.shadow.v1".into(),
        dataset: DATASET.into(),
        dataset_sha256,
        pack_sha256,
        cases: records.len(),
        mechanics_situations_detected,
        supported_subdomain,
        complete_typed_situations,
        unique_applicable_laws,
        complete_bindings,
        pack_invocations,
        pack_complete_results,
        candidate_answers,
        reference_matches,
        false_authorizations,
        situation_replay_verified,
        execution_replay_verified,
        first_failure_counts,
        mechanics_signal_first_failure_counts,
        situation_status_counts,
        registry_mutated: false,
        production_router_mutated: false,
        production_hle_score_changed: false,
        baseline_correct_authorized: BASELINE_CORRECT_AUTHORIZED,
        shadow_score_note: "Diagnostic only: no answer was authorized and production HLE routing was unchanged.".into(),
        records,
        method: "frozen HLE scan through MechanicsSituationV1 and unchanged classical-mechanics pack; shadow-only, non-authorizing".into(),
    };
    let output = serde_json::to_string_pretty(&report)?;
    let path = env::args()
        .nth(1)
        .unwrap_or_else(|| "docs/phase39_hle_mechanics_situation_shadow.json".into());
    fs::write(&path, output)?;
    println!("phase39 report written to {path}");
    Ok(())
}
