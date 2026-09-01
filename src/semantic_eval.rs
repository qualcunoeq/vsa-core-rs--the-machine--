//! Stable record schema for answer-key-blind semantic-worker evaluation.
//!
//! A stored record is a worker's complete raw receipt plus an external case ID.
//! Keeping this type in the library prevents probes and evaluators from
//! silently drifting apart in their JSON interpretation.

use crate::semantic_worker::RawSemanticReceipt;
use serde::{Deserialize, Serialize};

pub const STORED_SEMANTIC_OUTPUT_SCHEMA: &str = "stored-semantic-output-v1";

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StoredSemanticOutput {
    pub id: String,
    #[serde(flatten)]
    pub receipt: RawSemanticReceipt,
}

impl StoredSemanticOutput {
    pub fn replay_verified(&self) -> bool {
        !self.id.trim().is_empty() && self.receipt.replay_verified()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::semantic_worker::{SemanticWorker, SemanticWorkerConfig, WorkerTier};

    #[test]
    fn stored_record_preserves_full_worker_lineage() {
        let worker = SemanticWorker::new(SemanticWorkerConfig {
            tier: WorkerTier::Fast5070,
            endpoint: "stored://worker".into(),
            model: "test-model".into(),
            prompt_version: "prompt-v1".into(),
            grammar_version: "grammar-v1".into(),
            grammar: None,
            max_candidates: 3,
            max_output_tokens: 128,
            temperature: 0.0,
            timeout_ms: 1,
        })
        .expect("worker config");
        let receipt = worker.raw_receipt(
            "find x",
            "[]".into(),
            "stored://worker".into(),
            "prompt".into(),
        );
        let record = StoredSemanticOutput {
            id: "case-1".into(),
            receipt,
        };
        assert_eq!(STORED_SEMANTIC_OUTPUT_SCHEMA, "stored-semantic-output-v1");
        assert!(record.replay_verified());
        let encoded = serde_json::to_string(&record).expect("record JSON");
        let decoded: StoredSemanticOutput =
            serde_json::from_str(&encoded).expect("record roundtrip");
        assert_eq!(record, decoded);
    }
}
