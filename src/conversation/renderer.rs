//! Turns structured [`TurnResult`] records into readable answers.

use super::types::{TurnOutcome, TurnResult, VerificationStatus};

/// Deterministic, rule-based renderer. No model inference is involved.
#[derive(Clone, Debug)]
pub struct ResponseRenderer {
    /// When true, evidence, memory changes, and verification are appended.
    pub include_details: bool,
}

impl Default for ResponseRenderer {
    fn default() -> Self {
        Self::new()
    }
}

impl ResponseRenderer {
    pub fn new() -> Self {
        ResponseRenderer {
            include_details: true,
        }
    }

    pub fn without_details() -> Self {
        ResponseRenderer {
            include_details: false,
        }
    }

    pub fn render(&self, result: &TurnResult) -> String {
        let raw = result.answer.clone().unwrap_or_default();
        let raw = raw.trim();
        let mut out = String::new();

        match &result.outcome {
            TurnOutcome::Answered => {
                if raw.is_empty() {
                    out.push_str("Done.");
                } else {
                    out.push_str(raw);
                }
            }
            TurnOutcome::ClarificationNeeded { .. } => {
                if raw.is_empty() {
                    out.push_str("I need more information to answer that.");
                } else {
                    out.push_str(raw);
                }
                out.push_str("\nCould you provide the missing detail?");
            }
            TurnOutcome::Unsupported { reason } => {
                if raw.is_empty() {
                    out.push_str("I do not know the answer to that question.");
                } else {
                    out.push_str(raw);
                }
                if self.include_details {
                    out.push_str(&format!("\nReason: {reason}"));
                }
            }
            TurnOutcome::Failed { error } => {
                out.push_str(&format!("I could not complete that request: {error}"));
            }
            TurnOutcome::Cancelled => {
                if raw.is_empty() {
                    out.push_str("Request cancelled.");
                } else {
                    out.push_str(raw);
                }
            }
        }

        if !self.include_details {
            return out;
        }

        if !result.evidence.is_empty() {
            out.push_str("\nEvidence:");
            for item in &result.evidence {
                let replay = match item.replay_verified {
                    Some(true) => ", replay=verified",
                    Some(false) => ", replay=failed",
                    None => "",
                };
                out.push_str(&format!(
                    "\n- [{}] {} (source: {}, conf={:.2}{})",
                    item.kind.label(),
                    item.content,
                    item.provenance,
                    item.confidence,
                    replay
                ));
            }
        }

        match &result.verification {
            VerificationStatus::Verified { method, score } => {
                out.push_str(&format!("\nVerification: verified via {method} ({score:.2})"));
            }
            VerificationStatus::Unverified { reason } => {
                out.push_str(&format!("\nVerification: unverified ({reason})"));
            }
            VerificationStatus::NotAttempted => {}
        }

        if !result.memory_changes.is_empty() {
            out.push_str("\nMemory:");
            for change in &result.memory_changes {
                out.push_str(&format!(
                    "\n- {} {} ({})",
                    change.operation, change.detail, change.provenance
                ));
            }
        }

        out
    }
}
