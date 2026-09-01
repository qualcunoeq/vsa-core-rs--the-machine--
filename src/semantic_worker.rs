//! Sandboxed semantic-proposal worker boundary.
//!
//! This module speaks the small subset of the OpenAI-compatible chat protocol
//! exposed by local llama.cpp workers.  It records the raw model output and
//! reproducibility metadata, but deliberately does not validate, route, or
//! authorize a candidate.  `semantic_ir::validate_candidate` remains the only
//! semantic gate.

use crate::semantic_ir::{CandidateSemanticParse, SEMANTIC_IR_SCHEMA};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::time::Duration;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WorkerTier {
    Fast5070,
    DeepP40,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SemanticJob {
    pub token_estimate: usize,
    pub context_size: usize,
    pub ambiguity_score: u8,
    pub candidate_count: u8,
}

impl SemanticJob {
    /// Route only from explicit workload properties.  No domain or lexical
    /// signal is consulted, so the scheduler cannot silently become a router.
    pub fn preferred_tier(&self) -> WorkerTier {
        if self.context_size > 8_000
            || self.token_estimate > 6_000
            || self.ambiguity_score >= 2
            || self.candidate_count > 3
        {
            WorkerTier::DeepP40
        } else {
            WorkerTier::Fast5070
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SemanticWorkerConfig {
    pub tier: WorkerTier,
    pub endpoint: String,
    pub model: String,
    pub prompt_version: String,
    pub grammar_version: String,
    /// Optional llama.cpp grammar text. `None` keeps the request compatible
    /// with workers that enforce structure through their own server policy.
    pub grammar: Option<String>,
    pub max_candidates: u8,
    pub temperature: f32,
    pub timeout_ms: u64,
}

impl SemanticWorkerConfig {
    pub fn config_hash(&self) -> String {
        digest(&(
            self.tier,
            &self.endpoint,
            &self.model,
            &self.prompt_version,
            &self.grammar_version,
            &self.grammar,
            self.max_candidates,
            self.temperature.to_bits(),
            self.timeout_ms,
        ))
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RawSemanticReceipt {
    pub input: String,
    pub input_hash: String,
    pub tier: WorkerTier,
    pub endpoint: String,
    pub model: String,
    pub model_config_hash: String,
    pub prompt: String,
    pub prompt_hash: String,
    pub grammar_version: String,
    pub raw_output: String,
    pub raw_output_hash: String,
    pub replay_hash: String,
}

impl RawSemanticReceipt {
    pub fn replay_verified(&self) -> bool {
        self.input_hash == digest(&self.input)
            && self.prompt_hash == digest(&self.prompt)
            && self.raw_output_hash == digest(&self.raw_output)
            && self.replay_hash == receipt_hash(self)
            && !self.raw_output.is_empty()
    }
}

fn digest<T: Serialize>(value: &T) -> String {
    format!(
        "{:x}",
        Sha256::digest(serde_json::to_vec(value).expect("worker value serializes"))
    )
}

fn receipt_hash(receipt: &RawSemanticReceipt) -> String {
    digest(&(
        &receipt.input_hash,
        receipt.tier,
        &receipt.endpoint,
        &receipt.model,
        &receipt.model_config_hash,
        &receipt.prompt_hash,
        &receipt.grammar_version,
        &receipt.raw_output_hash,
    ))
}

pub fn semantic_prompt(input: &str, max_candidates: u8) -> String {
    format!(
        "Return at most {max_candidates} JSON candidate semantic parses using schema {SEMANTIC_IR_SCHEMA}. Extract only evidence present in the problem; preserve alternatives instead of guessing. Do not solve, authorize, mutate state, or add unstated assumptions.\n\nPROBLEM:\n{input}"
    )
}

#[derive(Debug, Clone)]
pub struct SemanticWorker {
    client: Client,
    pub config: SemanticWorkerConfig,
}

impl SemanticWorker {
    pub fn new(config: SemanticWorkerConfig) -> Result<Self, String> {
        let client = Client::builder()
            .timeout(Duration::from_millis(config.timeout_ms))
            .build()
            .map_err(|error| format!("worker client: {error}"))?;
        Ok(Self { client, config })
    }

    /// Request proposals and return an immutable raw receipt.  Parsing the
    /// response into candidates is a separate step so malformed model output
    /// remains auditable rather than being silently repaired here.
    pub async fn propose_raw(&self, input: &str) -> Result<RawSemanticReceipt, String> {
        let prompt = semantic_prompt(input, self.config.max_candidates);
        let payload = serde_json::json!({
            "model": self.config.model,
            "temperature": self.config.temperature,
            "n": 1,
            "grammar": &self.config.grammar,
            "messages": [
                {"role": "system", "content": "You are a semantic proposal engine. Output JSON only."},
                {"role": "user", "content": prompt}
            ]
        });
        let endpoint = self.config.endpoint.trim_end_matches('/').to_string();
        let response = self
            .client
            .post(format!("{endpoint}/v1/chat/completions"))
            .json(&payload)
            .send()
            .await
            .map_err(|error| format!("semantic worker request: {error}"))?;
        let status = response.status();
        let body = response
            .text()
            .await
            .map_err(|error| format!("semantic worker response: {error}"))?;
        if !status.is_success() {
            return Err(format!("semantic worker HTTP {status}: {body}"));
        }
        let value: serde_json::Value = serde_json::from_str(&body)
            .map_err(|error| format!("worker JSON envelope: {error}"))?;
        let raw_output = value
            .get("choices")
            .and_then(|choices| choices.get(0))
            .and_then(|choice| choice.get("message"))
            .and_then(|message| message.get("content"))
            .and_then(serde_json::Value::as_str)
            .ok_or_else(|| "worker response missing choices[0].message.content".to_string())?
            .to_string();
        let mut receipt = RawSemanticReceipt {
            input: input.to_string(),
            input_hash: digest(&input),
            tier: self.config.tier,
            endpoint,
            model: self.config.model.clone(),
            model_config_hash: self.config.config_hash(),
            prompt: prompt.clone(),
            prompt_hash: digest(&prompt),
            grammar_version: self.config.grammar_version.clone(),
            raw_output: raw_output.clone(),
            raw_output_hash: digest(&raw_output),
            replay_hash: String::new(),
        };
        receipt.replay_hash = receipt_hash(&receipt);
        Ok(receipt)
    }

    /// Decode only structurally valid JSON candidate arrays.  Worker metadata
    /// is stamped by this trusted boundary; semantic fields are not filled in.
    pub fn decode_candidates(
        &self,
        receipt: &RawSemanticReceipt,
    ) -> Result<Vec<CandidateSemanticParse>, String> {
        if !receipt.replay_verified() {
            return Err("raw worker receipt failed replay verification".into());
        }
        let json_text = receipt
            .raw_output
            .trim()
            .trim_start_matches("```json")
            .trim_start_matches("```")
            .trim_end_matches("```")
            .trim();
        let value: serde_json::Value = serde_json::from_str(json_text)
            .map_err(|error| format!("candidate JSON rejected: {error}"))?;
        let candidates = value.get("candidates").cloned().unwrap_or(value);
        let mut parsed: Vec<CandidateSemanticParse> = serde_json::from_value(candidates)
            .map_err(|error| format!("candidate schema rejected: {error}"))?;
        for candidate in &mut parsed {
            if candidate.schema != SEMANTIC_IR_SCHEMA {
                return Err("candidate schema version mismatch".into());
            }
            candidate.input_hash = receipt.input_hash.clone();
            candidate.model_id = receipt.model.clone();
            candidate.model_config_hash = receipt.model_config_hash.clone();
            candidate.prompt_hash = receipt.prompt_hash.clone();
            candidate.grammar_version = receipt.grammar_version.clone();
            candidate.raw_output_hash = receipt.raw_output_hash.clone();
            *candidate = candidate.clone().with_replay_hash();
        }
        if parsed.len() > self.config.max_candidates as usize {
            return Err("worker exceeded candidate budget".into());
        }
        Ok(parsed)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::semantic_ir::{validate_candidate, EvidenceSpan, TargetKind};
    use std::collections::BTreeMap;

    fn config() -> SemanticWorkerConfig {
        SemanticWorkerConfig {
            tier: WorkerTier::Fast5070,
            endpoint: "http://127.0.0.1:8081".into(),
            model: "test-model".into(),
            prompt_version: "semantic-prompt-v1".into(),
            grammar_version: "candidate-json-v1".into(),
            grammar: Some("root ::= \"[]\"".into()),
            max_candidates: 3,
            temperature: 0.0,
            timeout_ms: 100,
        }
    }

    #[test]
    fn scheduler_escalates_only_explicitly_complex_jobs() {
        assert_eq!(
            SemanticJob {
                token_estimate: 100,
                context_size: 100,
                ambiguity_score: 0,
                candidate_count: 1,
            }
            .preferred_tier(),
            WorkerTier::Fast5070
        );
        assert_eq!(
            SemanticJob {
                token_estimate: 100,
                context_size: 9_000,
                ambiguity_score: 0,
                candidate_count: 1,
            }
            .preferred_tier(),
            WorkerTier::DeepP40
        );
    }

    #[test]
    fn prompt_is_version_independent_and_does_not_request_answers() {
        let prompt = semantic_prompt("find x", 3);
        assert!(prompt.contains(SEMANTIC_IR_SCHEMA));
        assert!(prompt.contains("Do not solve"));
        assert!(!prompt.contains("answer:"));
    }

    #[test]
    fn config_and_raw_receipt_hashes_are_stable() {
        let cfg = config();
        assert_eq!(cfg.config_hash().len(), 64);
        let prompt = semantic_prompt("find x", cfg.max_candidates);
        let mut receipt = RawSemanticReceipt {
            input: "find x".into(),
            input_hash: digest(&"find x"),
            tier: cfg.tier,
            endpoint: cfg.endpoint.clone(),
            model: cfg.model.clone(),
            model_config_hash: cfg.config_hash(),
            prompt: prompt.clone(),
            prompt_hash: digest(&prompt),
            grammar_version: cfg.grammar_version.clone(),
            raw_output: "[]".into(),
            raw_output_hash: digest(&"[]"),
            replay_hash: String::new(),
        };
        receipt.replay_hash = receipt_hash(&receipt);
        assert!(receipt.replay_verified());
        receipt.raw_output.push('x');
        assert!(!receipt.replay_verified());
    }

    #[test]
    fn decoded_candidate_crosses_only_the_deterministic_validator() {
        let cfg = config();
        let worker = SemanticWorker::new(cfg.clone()).expect("worker config");
        let input = "solve x";
        let mut proposal = CandidateSemanticParse {
            schema: SEMANTIC_IR_SCHEMA.into(),
            input_hash: String::new(),
            model_id: "model-supplied-value".into(),
            model_config_hash: String::new(),
            prompt_hash: String::new(),
            grammar_version: String::new(),
            target: "x".into(),
            target_kind: TargetKind::Scalar,
            operation: "solve".into(),
            symbols: Vec::new(),
            symbol_scopes: BTreeMap::new(),
            equations: Vec::new(),
            assumptions: Vec::new(),
            domains: Vec::new(),
            candidate_pack: None,
            unresolved_ambiguities: Vec::new(),
            evidence_spans: vec![EvidenceSpan {
                start: 6,
                end: 7,
                text: "x".into(),
                role: "target".into(),
            }],
            confidence: 0.99,
            raw_output_hash: String::new(),
            replay_hash: String::new(),
        };
        proposal = proposal.with_replay_hash();
        let raw_output = serde_json::to_string(&vec![proposal]).expect("candidate JSON");
        let prompt = semantic_prompt(input, cfg.max_candidates);
        let mut receipt = RawSemanticReceipt {
            input: input.into(),
            input_hash: digest(&input),
            tier: cfg.tier,
            endpoint: cfg.endpoint.clone(),
            model: cfg.model.clone(),
            model_config_hash: cfg.config_hash(),
            prompt: prompt.clone(),
            prompt_hash: digest(&prompt),
            grammar_version: cfg.grammar_version,
            raw_output: raw_output.clone(),
            raw_output_hash: digest(&raw_output),
            replay_hash: String::new(),
        };
        receipt.replay_hash = receipt_hash(&receipt);
        let decoded = worker
            .decode_candidates(&receipt)
            .expect("decode candidate");
        assert_eq!(decoded.len(), 1);
        assert!(decoded[0].replay_verified());
        let validation = validate_candidate(input, &decoded[0]);
        assert_eq!(
            validation.decision,
            crate::semantic_ir::ValidationDecision::AcceptCandidate,
            "diagnostics: {:?}",
            validation.diagnostics
        );
        assert!(!validation.downstream_authorized);
    }

    #[test]
    fn malformed_or_over_budget_model_output_is_rejected() {
        let cfg = config();
        let worker = SemanticWorker::new(cfg.clone()).expect("worker config");
        let input = "solve x";
        let prompt = semantic_prompt(input, cfg.max_candidates);
        let make_receipt = |raw_output: String| {
            let mut receipt = RawSemanticReceipt {
                input: input.into(),
                input_hash: digest(&input),
                tier: cfg.tier,
                endpoint: cfg.endpoint.clone(),
                model: cfg.model.clone(),
                model_config_hash: cfg.config_hash(),
                prompt: prompt.clone(),
                prompt_hash: digest(&prompt),
                grammar_version: cfg.grammar_version.clone(),
                raw_output_hash: digest(&raw_output),
                raw_output,
                replay_hash: String::new(),
            };
            receipt.replay_hash = receipt_hash(&receipt);
            receipt
        };
        assert!(worker
            .decode_candidates(&make_receipt("not json".into()))
            .is_err());
        let empty_candidates = serde_json::to_string(&vec![
            serde_json::json!({}),
            serde_json::json!({}),
            serde_json::json!({}),
            serde_json::json!({}),
        ])
        .expect("over-budget JSON");
        assert!(worker
            .decode_candidates(&make_receipt(empty_candidates))
            .is_err());
    }

    #[tokio::test]
    #[ignore = "sandbox forbids binding a local TCP socket; run in an integration environment"]
    async fn openai_compatible_response_becomes_a_replayable_raw_receipt() {
        use tokio::io::{AsyncReadExt, AsyncWriteExt};
        use tokio::net::TcpListener;

        let listener = TcpListener::bind(("127.0.0.1", 0))
            .await
            .expect("bind mock worker");
        let address = listener.local_addr().expect("mock address");
        let server = tokio::spawn(async move {
            let (mut socket, _) = listener.accept().await.expect("accept worker");
            let mut request = vec![0_u8; 16 * 1024];
            let size = socket
                .read(&mut request)
                .await
                .expect("read worker request");
            let request = String::from_utf8_lossy(&request[..size]);
            assert!(request.contains("POST /v1/chat/completions"));
            assert!(request.contains("test-model"));
            assert!(request.contains("candidate-json-v1"));
            let body = r#"{"choices":[{"message":{"content":"[]"}}]}"#;
            let response = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                body.len(),
                body
            );
            socket
                .write_all(response.as_bytes())
                .await
                .expect("write worker response");
        });

        let mut cfg = config();
        cfg.endpoint = format!("http://{address}");
        let worker = SemanticWorker::new(cfg).expect("worker config");
        let receipt = worker
            .propose_raw("extract the target x")
            .await
            .expect("mock worker response");
        assert!(receipt.replay_verified());
        assert_eq!(receipt.raw_output, "[]");
        server.await.expect("mock worker task");
    }
}
