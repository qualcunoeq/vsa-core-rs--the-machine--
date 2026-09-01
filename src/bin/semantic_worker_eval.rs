//! Answer-key-blind evaluator for stored semantic-worker outputs.
//!
//! Input JSONL records have `{id,input,raw_output}`. The evaluator performs
//! decoding, deterministic candidate-ensemble validation, and replay checks;
//! it never reads answer keys or invokes a downstream solver.

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::env;
use std::fs;
use the_machine::semantic_ir::{validate_candidate_ensemble, ValidationDecision};
use the_machine::semantic_worker::{SemanticWorker, SemanticWorkerConfig, WorkerTier};

#[derive(Debug, Deserialize)]
struct InputRecord {
    id: String,
    input: String,
    raw_output: String,
}

#[derive(Debug, Serialize)]
struct OutputRecord {
    id: String,
    candidate_count: usize,
    receipt_replay_verified: bool,
    candidate_replays: usize,
    ensemble_decision: ValidationDecision,
    selected_index: Option<usize>,
    ensemble_replay_verified: bool,
    downstream_authorized: bool,
    error: Option<String>,
}

fn digest<T: Serialize>(value: &T) -> String {
    format!(
        "{:x}",
        Sha256::digest(serde_json::to_vec(value).expect("eval value serializes"))
    )
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let input_path = env::var("SEMANTIC_EVAL_INPUT")
        .map_err(|_| "SEMANTIC_EVAL_INPUT must point to worker-output JSONL")?;
    let output_path = env::var("SEMANTIC_EVAL_OUTPUT")
        .unwrap_or_else(|_| "/tmp/semantic_worker_eval.jsonl".into());
    let model = env::var("SEMANTIC_WORKER_MODEL").unwrap_or_else(|_| "stored-worker".into());
    let worker = SemanticWorker::new(SemanticWorkerConfig {
        tier: WorkerTier::Fast5070,
        endpoint: env::var("SEMANTIC_WORKER_ENDPOINT").unwrap_or_else(|_| "stored://worker".into()),
        model,
        prompt_version: "semantic-prompt-v1".into(),
        grammar_version: "candidate-json-v1".into(),
        grammar: env::var("SEMANTIC_WORKER_GRAMMAR").ok(),
        max_candidates: 3,
        temperature: 0.0,
        timeout_ms: 1,
    })?;

    let mut output = String::new();
    let mut records = 0usize;
    let mut decode_errors = 0usize;
    let mut accepted = 0usize;
    let mut ambiguous = 0usize;
    let mut rejected = 0usize;
    let mut replay_verified = 0usize;
    for line in fs::read_to_string(&input_path)?
        .lines()
        .filter(|line| !line.trim().is_empty())
    {
        records += 1;
        let record = match serde_json::from_str::<InputRecord>(line) {
            Ok(record) => record,
            Err(error) => {
                decode_errors += 1;
                output.push_str(&serde_json::to_string(&OutputRecord {
                    id: format!("invalid-{records}"),
                    candidate_count: 0,
                    receipt_replay_verified: false,
                    candidate_replays: 0,
                    ensemble_decision: ValidationDecision::RejectCandidate,
                    selected_index: None,
                    ensemble_replay_verified: false,
                    downstream_authorized: false,
                    error: Some(format!("input record: {error}")),
                })?);
                output.push('\n');
                continue;
            }
        };
        match worker.decode_raw_output(&record.input, record.raw_output) {
            Ok((receipt, candidates)) => {
                let ensemble = validate_candidate_ensemble(&record.input, &candidates);
                match ensemble.decision {
                    ValidationDecision::AcceptCandidate => accepted += 1,
                    ValidationDecision::PreserveAmbiguity => ambiguous += 1,
                    ValidationDecision::RejectCandidate => rejected += 1,
                }
                if receipt.replay_verified() && ensemble.replay_verified() {
                    replay_verified += 1;
                }
                output.push_str(&serde_json::to_string(&OutputRecord {
                    id: record.id,
                    candidate_count: candidates.len(),
                    receipt_replay_verified: receipt.replay_verified(),
                    candidate_replays: candidates
                        .iter()
                        .filter(|candidate| candidate.replay_verified())
                        .count(),
                    ensemble_decision: ensemble.decision,
                    selected_index: ensemble.selected_index,
                    ensemble_replay_verified: ensemble.replay_verified(),
                    downstream_authorized: ensemble.downstream_authorized,
                    error: None,
                })?);
                output.push('\n');
            }
            Err(error) => {
                decode_errors += 1;
                output.push_str(&serde_json::to_string(&OutputRecord {
                    id: record.id,
                    candidate_count: 0,
                    receipt_replay_verified: false,
                    candidate_replays: 0,
                    ensemble_decision: ValidationDecision::RejectCandidate,
                    selected_index: None,
                    ensemble_replay_verified: false,
                    downstream_authorized: false,
                    error: Some(error),
                })?);
                output.push('\n');
            }
        }
    }
    fs::write(&output_path, output)?;
    eprintln!(
        "records={records} accepted={accepted} ambiguous={ambiguous} rejected={rejected} decode_errors={decode_errors} replay_verified={replay_verified} output_sha256={}",
        digest(&fs::read(&output_path)?)
    );
    Ok(())
}
