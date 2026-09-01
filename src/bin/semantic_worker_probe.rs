//! Probe a local OpenAI-compatible semantic worker without granting it any
//! solver, registry, or answer authority.

use std::env;
use the_machine::semantic_ir::validate_candidate_ensemble;
use the_machine::semantic_worker::{SemanticWorker, SemanticWorkerConfig, WorkerTier};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let endpoint =
        env::var("SEMANTIC_WORKER_ENDPOINT").unwrap_or_else(|_| "http://127.0.0.1:8081".into());
    let model = env::var("SEMANTIC_WORKER_MODEL").unwrap_or_else(|_| "semantic-parser".into());
    let input = env::args().skip(1).collect::<Vec<_>>().join(" ");
    if input.trim().is_empty() {
        return Err("usage: semantic_worker_probe <technical problem>".into());
    }
    let tier = match env::var("SEMANTIC_WORKER_TIER")
        .unwrap_or_else(|_| "fast5070".into())
        .as_str()
    {
        "fast5070" => WorkerTier::Fast5070,
        "deepp40" => WorkerTier::DeepP40,
        other => return Err(format!("unknown SEMANTIC_WORKER_TIER: {other}").into()),
    };
    let grammar = env::var("SEMANTIC_WORKER_GRAMMAR").ok();
    let worker = SemanticWorker::new(SemanticWorkerConfig {
        tier,
        endpoint,
        model,
        prompt_version: "semantic-prompt-v1".into(),
        grammar_version: "candidate-json-v1".into(),
        grammar,
        max_candidates: 3,
        max_output_tokens: 2048,
        temperature: 0.0,
        timeout_ms: 30_000,
    })?;
    let raw = worker.propose_raw(&input).await?;
    let candidates = worker.decode_candidates(&raw)?;
    let ensemble = validate_candidate_ensemble(&input, &candidates);
    println!(
        "{}",
        serde_json::to_string_pretty(&serde_json::json!({
            "raw_receipt": raw,
            "candidate_count": candidates.len(),
            "candidate_replays": candidates.iter().filter(|candidate| candidate.replay_verified()).count(),
            "ensemble": ensemble,
            "semantic_only": true,
            "downstream_authorized": false,
        }))?
    );
    Ok(())
}
