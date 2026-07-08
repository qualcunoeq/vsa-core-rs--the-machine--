// ─── Language Decoder: SVO triples → deterministic English ────────────────
//
// This is the first "mouth" for The Machine: it does not hallucinate prose
// from a latent model.  It verbalizes explicit SVO facts and traces already
// present in memory, preserving the bitwise/retrieval architecture.
// ────────────────────────────────────────────────────────────────────────────

use crate::perception::SvoTriple;

/// Deterministic decoder for facts, answers, and reasoning traces.
pub struct NlpDecoder;

impl NlpDecoder {
    pub fn new() -> Self {
        NlpDecoder
    }

    /// Convert a machine token into readable text without losing identity.
    pub fn verbalize_token(token: &str) -> String {
        let token = token.trim();
        if token.is_empty() {
            return "unknown".to_string();
        }

        token
            .replace('_', " ")
            .split_whitespace()
            .collect::<Vec<_>>()
            .join(" ")
    }

    /// Convert a relation token into a readable verb phrase.
    pub fn verbalize_relation(relation: &str) -> String {
        match relation.trim() {
            "is_child_of" => "is a child of".to_string(),
            "is_running" => "is running".to_string(),
            "run_by_user" => "is run by user".to_string(),
            "has_open" => "has open".to_string(),
            "connected_to" => "is connected to".to_string(),
            "state" => "has state".to_string(),
            "local" => "has local endpoint".to_string(),
            "protocol" => "uses protocol".to_string(),
            "executing" => "is executing".to_string(),
            other => Self::verbalize_token(other),
        }
    }

    /// Render one SVO triple as one sentence.
    pub fn decode_triple(&self, triple: &SvoTriple) -> String {
        let subject = Self::verbalize_token(&triple.0);
        let relation = Self::verbalize_relation(&triple.1);
        let object = Self::verbalize_token(&triple.2);
        format!("{} {} {}.", subject, relation, object)
    }

    /// Render a compact paragraph from triples, preserving input order.
    pub fn decode_triples(&self, triples: &[SvoTriple], limit: usize) -> String {
        if triples.is_empty() || limit == 0 {
            return "I do not have an explicit fact to report.".to_string();
        }

        triples
            .iter()
            .take(limit)
            .map(|triple| self.decode_triple(triple))
            .collect::<Vec<_>>()
            .join(" ")
    }

    /// Render an answer with a concise evidence trail.
    pub fn answer_with_evidence(&self, answer: &str, evidence: &[SvoTriple]) -> String {
        let answer = Self::verbalize_token(answer);
        if evidence.is_empty() {
            return format!("Answer: {}. I have no explicit evidence trace.", answer);
        }
        format!(
            "Answer: {}. Evidence: {}",
            answer,
            self.decode_triples(evidence, evidence.len())
        )
    }

    /// Render a reasoning trace as ordered steps.
    pub fn decode_trace(&self, trace: &[SvoTriple]) -> String {
        if trace.is_empty() {
            return "No reasoning trace is available.".to_string();
        }

        trace
            .iter()
            .enumerate()
            .map(|(idx, triple)| format!("Step {}: {}", idx + 1, self.decode_triple(triple)))
            .collect::<Vec<_>>()
            .join(" ")
    }
}

impl Default for NlpDecoder {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_decode_triple_verbalizes_machine_tokens() {
        let decoder = NlpDecoder::new();
        let triple = (
            "process_42".to_string(),
            "connected_to".to_string(),
            "127.0.0.1:443".to_string(),
        );

        assert_eq!(
            decoder.decode_triple(&triple),
            "process 42 is connected to 127.0.0.1:443."
        );
    }

    #[test]
    fn test_answer_with_evidence_is_grounded() {
        let decoder = NlpDecoder::new();
        let evidence = vec![(
            "the_fed".to_string(),
            "raise".to_string(),
            "rates".to_string(),
        )];

        let response = decoder.answer_with_evidence("rates", &evidence);
        assert!(response.contains("Answer: rates."));
        assert!(response.contains("the fed raise rates."));
    }

    #[test]
    fn test_empty_decode_refuses_to_invent_fact() {
        let decoder = NlpDecoder::new();
        assert_eq!(
            decoder.decode_triples(&[], 3),
            "I do not have an explicit fact to report."
        );
    }
}
