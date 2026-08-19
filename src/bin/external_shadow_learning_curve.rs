//! Cumulative, development-only shadow learning curve for the external
//! routes. This is an accounting checkpoint, not a production score: no
//! sealed oracle is read and no route is authorized.

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::fs;

const BASELINE: &str = "docs/goal1_external_math_exam_baseline_development.json";
const MEAN: &str = "docs/goal6_external_list_mean_shadow_score.json";
const SEQUENCE: &str = "docs/goal6_external_sequence_shadow_score.json";
const COUNTING: &str = "docs/goal6_external_counting_frontend.json";
const REPORT_JSON: &str = "docs/goal6_external_shadow_learning_curve.json";
const REPORT_MD: &str = "docs/goal6_external_shadow_learning_curve.md";

#[derive(Debug, Deserialize)]
struct Baseline {
    cases: usize,
    correct_authorized: usize,
    false_authorizations: usize,
    registry_mutated: bool,
}

#[derive(Debug, Deserialize)]
struct Candidate {
    id: String,
    reference_match: bool,
    replay_verified: bool,
}

#[derive(Debug, Deserialize)]
struct RouteReport {
    questions_read: usize,
    plaintext_answers_read: usize,
    candidate_cases: usize,
    correct_shadow_candidates: usize,
    incorrect_shadow_candidates_rejected: usize,
    candidate_replays: usize,
    production_authorizations: usize,
    false_authorizations: usize,
    manifest_unchanged: bool,
    candidates: Vec<Candidate>,
}

#[derive(Debug, Deserialize)]
struct CountingReport {
    external_questions_read: usize,
    external_answer_keys_read: usize,
    external_executable_candidates: usize,
    external_candidate_replays: usize,
    production_authorizations: usize,
    false_authorizations: usize,
    manifest_unchanged: bool,
    external_counting_signals: usize,
    external_candidates: Vec<CountingCandidate>,
}

#[derive(Debug, Deserialize)]
struct CountingCandidate {
    id: String,
}

#[derive(Debug, Serialize)]
struct Checkpoint {
    label: String,
    evaluated_cases: usize,
    correct_shadow: usize,
    false_authorizations: usize,
    replay_verified: usize,
    production_authorizations: usize,
}

#[derive(Debug, Serialize)]
struct Report {
    schema: &'static str,
    development_cases: usize,
    answer_keys_read: usize,
    plaintext_answers_read: usize,
    checkpoints: Vec<Checkpoint>,
    cumulative_candidates: usize,
    cumulative_correct_shadow: usize,
    cumulative_replays: usize,
    cumulative_false_authorizations: usize,
    candidate_ids_disjoint: bool,
    route_reports_unchanged: bool,
    production_authorizations: usize,
    report_sha256: String,
}

fn digest<T: Serialize>(value: &T) -> String {
    format!("{:x}", Sha256::digest(serde_json::to_vec(value).unwrap()))
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let baseline: Baseline = serde_json::from_slice(&fs::read(BASELINE)?)?;
    let mean: RouteReport = serde_json::from_slice(&fs::read(MEAN)?)?;
    let sequence: RouteReport = serde_json::from_slice(&fs::read(SEQUENCE)?)?;
    let counting: CountingReport = serde_json::from_slice(&fs::read(COUNTING)?)?;
    assert_eq!(baseline.cases, 3000);
    assert_eq!(baseline.correct_authorized, 0);
    assert_eq!(baseline.false_authorizations, 0);
    assert!(!baseline.registry_mutated);
    assert_eq!(mean.questions_read, 4000);
    assert_eq!(mean.plaintext_answers_read, 0);
    assert_eq!(sequence.questions_read, 3000);
    assert_eq!(sequence.plaintext_answers_read, 0);
    assert_eq!(counting.external_questions_read, 3000);
    assert_eq!(counting.external_answer_keys_read, 0);
    assert_eq!(counting.external_executable_candidates, 0);
    assert_eq!(counting.external_candidate_replays, 0);
    assert_eq!(counting.production_authorizations, 0);
    assert_eq!(counting.false_authorizations, 0);
    assert!(mean.manifest_unchanged && sequence.manifest_unchanged && counting.manifest_unchanged);
    let mean_ids = mean
        .candidates
        .iter()
        .map(|candidate| candidate.id.clone())
        .collect::<Vec<_>>();
    let sequence_ids = sequence
        .candidates
        .iter()
        .map(|candidate| candidate.id.clone())
        .collect::<Vec<_>>();
    let counting_ids = counting
        .external_candidates
        .iter()
        .map(|candidate| candidate.id.clone())
        .collect::<Vec<_>>();
    let candidate_ids_disjoint = mean_ids
        .iter()
        .all(|id| !sequence_ids.contains(id) && !counting_ids.contains(id))
        && sequence_ids.iter().all(|id| !counting_ids.contains(id));
    let cumulative_replays =
        mean.candidate_replays + sequence.candidate_replays + counting.external_candidate_replays;
    let cumulative_correct = mean.correct_shadow_candidates
        + sequence.correct_shadow_candidates
        + counting.external_executable_candidates;
    let mut report = Report {
        schema: "goal6-external-shadow-learning-curve-v2",
        development_cases: baseline.cases,
        answer_keys_read: 0,
        plaintext_answers_read: mean.plaintext_answers_read + sequence.plaintext_answers_read,
        checkpoints: vec![
            Checkpoint {
                label: "baseline".into(),
                evaluated_cases: baseline.cases,
                correct_shadow: baseline.correct_authorized,
                false_authorizations: baseline.false_authorizations,
                replay_verified: 0,
                production_authorizations: 0,
            },
            Checkpoint {
                label: "finite_list_mean_shadow".into(),
                evaluated_cases: mean.questions_read,
                correct_shadow: mean.correct_shadow_candidates,
                false_authorizations: mean.false_authorizations,
                replay_verified: mean.candidate_replays,
                production_authorizations: mean.production_authorizations,
            },
            Checkpoint {
                label: "arithmetic_sequence_shadow".into(),
                evaluated_cases: sequence.questions_read,
                correct_shadow: sequence.correct_shadow_candidates,
                false_authorizations: sequence.false_authorizations,
                replay_verified: sequence.candidate_replays,
                production_authorizations: sequence.production_authorizations,
            },
            Checkpoint {
                label: "bounded_counting_shadow".into(),
                evaluated_cases: counting.external_questions_read,
                correct_shadow: counting.external_executable_candidates,
                false_authorizations: counting.false_authorizations,
                replay_verified: counting.external_candidate_replays,
                production_authorizations: counting.production_authorizations,
            },
        ],
        cumulative_candidates: mean.candidate_cases
            + sequence.candidate_cases
            + counting.external_executable_candidates,
        cumulative_correct_shadow: cumulative_correct,
        cumulative_replays,
        cumulative_false_authorizations: mean.false_authorizations
            + sequence.false_authorizations
            + counting.false_authorizations,
        candidate_ids_disjoint,
        route_reports_unchanged: mean.manifest_unchanged
            && sequence.manifest_unchanged
            && counting.manifest_unchanged,
        production_authorizations: mean.production_authorizations
            + sequence.production_authorizations
            + counting.production_authorizations,
        report_sha256: String::new(),
    };
    assert_eq!(mean.candidate_cases, 2);
    assert_eq!(mean.correct_shadow_candidates, 2);
    assert_eq!(mean.incorrect_shadow_candidates_rejected, 0);
    assert_eq!(sequence.candidate_cases, 3);
    assert_eq!(sequence.correct_shadow_candidates, 3);
    assert_eq!(sequence.incorrect_shadow_candidates_rejected, 0);
    assert!(counting.external_candidates.is_empty());
    assert_eq!(cumulative_correct, 5);
    assert_eq!(cumulative_replays, 5);
    assert_eq!(report.answer_keys_read, 0);
    assert_eq!(report.plaintext_answers_read, 0);
    assert!(report.candidate_ids_disjoint);
    assert!(report.route_reports_unchanged);
    assert_eq!(report.production_authorizations, 0);
    let mut unsigned = serde_json::to_value(&report)?;
    unsigned["report_sha256"] = serde_json::Value::String(String::new());
    report.report_sha256 = digest(&unsigned);
    let serialized = serde_json::to_string_pretty(&report)?;
    fs::write(REPORT_JSON, format!("{serialized}\n"))?;
    fs::write(
        REPORT_MD,
        format!(
            "# Goal 6 — external shadow learning curve\n\n- Baseline: 0/{} authorized, 0 false authorizations\n- Finite-list-mean shadow: {}/{} correct candidates, {}/{} replay\n- Arithmetic-sequence shadow: {}/{} correct candidates, {}/{} replay\n- Bounded-counting shadow: {}/{} complete candidates, {}/{} replay (signals: {})\n- Cumulative correct shadow candidates: {}\n- Cumulative candidate replay: {}\n- Candidate IDs disjoint: {}\n- Answer keys / plaintext answers read by this aggregator: {} / {}\n- Production authorizations / false authorizations: {} / {}\n\nThis is a development-only shadow curve; it does not claim autonomous curriculum selection and does not modify production routing.\n",
            baseline.cases,
            mean.correct_shadow_candidates,
            mean.candidate_cases,
            mean.candidate_replays,
            mean.candidate_cases,
            sequence.correct_shadow_candidates,
            sequence.candidate_cases,
            sequence.candidate_replays,
            sequence.candidate_cases,
            counting.external_executable_candidates,
            counting.external_questions_read,
            counting.external_candidate_replays,
            counting.external_executable_candidates,
            counting.external_counting_signals,
            report.cumulative_correct_shadow,
            report.cumulative_replays,
            report.candidate_ids_disjoint,
            report.answer_keys_read,
            report.plaintext_answers_read,
            report.production_authorizations,
            report.cumulative_false_authorizations,
        ),
    )?;
    println!("{serialized}");
    Ok(())
}
