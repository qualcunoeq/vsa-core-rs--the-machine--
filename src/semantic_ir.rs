//! Versioned semantic intermediate representation for neural proposal frontends.
//!
//! A model may propose a parse, but it cannot authorize a problem or mutate a
//! registry.  This module is deliberately independent of any model runtime so
//! that llama.cpp workers, a future fine-tuned parser, and deterministic test
//! fixtures all cross the same typed boundary.

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};

pub const SEMANTIC_IR_SCHEMA: &str = "candidate-semantic-parse-v1";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CandidateStatus {
    Complete,
    Ambiguous,
    Unsupported,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TargetKind {
    Scalar,
    Expression,
    Set,
    Classification,
    Proof,
    Property,
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EvidenceSpan {
    pub start: usize,
    pub end: usize,
    pub text: String,
    pub role: String,
}

impl EvidenceSpan {
    pub fn valid_for(&self, input: &str) -> bool {
        self.start < self.end
            && self.end <= input.len()
            && input.get(self.start..self.end) == Some(self.text.as_str())
            && input.is_char_boundary(self.start)
            && input.is_char_boundary(self.end)
            && !self.role.trim().is_empty()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SymbolIR {
    pub name: String,
    pub scope: String,
    pub type_name: Option<String>,
    pub domain: Option<String>,
    pub declared: bool,
    pub evidence_spans: Vec<EvidenceSpan>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RelationIR {
    pub kind: String,
    pub expression: String,
    pub symbols: Vec<String>,
    pub evidence_spans: Vec<EvidenceSpan>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct IntentIR {
    pub target: String,
    pub target_kind: TargetKind,
    pub operation: String,
    pub candidate_pack: Option<String>,
}

/// Proposal emitted by a semantic model.  `confidence` is diagnostic only;
/// it is never consulted by the authorization boundary.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CandidateSemanticParse {
    pub schema: String,
    pub input_hash: String,
    pub model_id: String,
    pub model_config_hash: String,
    pub prompt_hash: String,
    pub grammar_version: String,
    pub target: String,
    pub target_kind: TargetKind,
    pub operation: String,
    pub symbols: Vec<SymbolIR>,
    pub symbol_scopes: BTreeMap<String, String>,
    pub equations: Vec<RelationIR>,
    pub assumptions: Vec<String>,
    pub domains: Vec<String>,
    pub candidate_pack: Option<String>,
    pub unresolved_ambiguities: Vec<String>,
    pub evidence_spans: Vec<EvidenceSpan>,
    pub confidence: f32,
    pub raw_output_hash: String,
    pub replay_hash: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ValidationDecision {
    AcceptCandidate,
    PreserveAmbiguity,
    RejectCandidate,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SemanticValidationReceipt {
    pub decision: ValidationDecision,
    pub diagnostics: Vec<String>,
    pub candidate_hash: String,
    pub replay_hash: String,
    /// Always false: this receipt authorizes only a semantic parse, never an
    /// answer, capability, registry mutation, or world-model update.
    pub downstream_authorized: bool,
}

fn digest<T: Serialize>(value: &T) -> String {
    format!("{:x}", Sha256::digest(serde_json::to_vec(value).expect("semantic IR serializes")))
}

fn candidate_payload(candidate: &CandidateSemanticParse) -> impl Serialize + '_ {
    #[derive(Serialize)]
    struct Payload<'a> {
        schema: &'a str,
        input_hash: &'a str,
        model_id: &'a str,
        model_config_hash: &'a str,
        prompt_hash: &'a str,
        grammar_version: &'a str,
        target: &'a str,
        target_kind: TargetKind,
        operation: &'a str,
        symbols: &'a [SymbolIR],
        symbol_scopes: &'a BTreeMap<String, String>,
        equations: &'a [RelationIR],
        assumptions: &'a [String],
        domains: &'a [String],
        candidate_pack: &'a Option<String>,
        unresolved_ambiguities: &'a [String],
        evidence_spans: &'a [EvidenceSpan],
        confidence_bits: u32,
        raw_output_hash: &'a str,
    }
    Payload {
        schema: &candidate.schema,
        input_hash: &candidate.input_hash,
        model_id: &candidate.model_id,
        model_config_hash: &candidate.model_config_hash,
        prompt_hash: &candidate.prompt_hash,
        grammar_version: &candidate.grammar_version,
        target: &candidate.target,
        target_kind: candidate.target_kind,
        operation: &candidate.operation,
        symbols: &candidate.symbols,
        symbol_scopes: &candidate.symbol_scopes,
        equations: &candidate.equations,
        assumptions: &candidate.assumptions,
        domains: &candidate.domains,
        candidate_pack: &candidate.candidate_pack,
        unresolved_ambiguities: &candidate.unresolved_ambiguities,
        evidence_spans: &candidate.evidence_spans,
        confidence_bits: candidate.confidence.to_bits(),
        raw_output_hash: &candidate.raw_output_hash,
    }
}

fn receipt_payload(receipt: &SemanticValidationReceipt) -> impl Serialize + '_ {
    (
        receipt.decision,
        &receipt.diagnostics,
        &receipt.candidate_hash,
        receipt.downstream_authorized,
    )
}

impl CandidateSemanticParse {
    pub fn candidate_hash(&self) -> String {
        digest(&candidate_payload(self))
    }

    pub fn with_replay_hash(mut self) -> Self {
        self.replay_hash = self.candidate_hash();
        self
    }

    pub fn replay_verified(&self) -> bool {
        self.schema == SEMANTIC_IR_SCHEMA
            && !self.input_hash.is_empty()
            && !self.model_id.is_empty()
            && !self.raw_output_hash.is_empty()
            && self.replay_hash == self.candidate_hash()
    }
}

impl SemanticValidationReceipt {
    pub fn replay_verified(&self) -> bool {
        !self.downstream_authorized
            && self.replay_hash == digest(&receipt_payload(self))
            && self.candidate_hash.len() == 64
    }
}

fn receipt(
    decision: ValidationDecision,
    diagnostics: Vec<String>,
    candidate_hash: String,
) -> SemanticValidationReceipt {
    let mut result = SemanticValidationReceipt {
        decision,
        diagnostics,
        candidate_hash,
        replay_hash: String::new(),
        downstream_authorized: false,
    };
    let replay_hash = digest(&receipt_payload(&result));
    result.replay_hash = replay_hash;
    result
}

/// Validate a model proposal without selecting a solver or authorizing an
/// answer.  Confidence is intentionally ignored.
pub fn validate_candidate(
    input: &str,
    candidate: &CandidateSemanticParse,
) -> SemanticValidationReceipt {
    let mut diagnostics = Vec::new();
    if candidate.schema != SEMANTIC_IR_SCHEMA {
        diagnostics.push("schema_version_mismatch".into());
    }
    if candidate.input_hash.is_empty() || candidate.model_id.trim().is_empty() {
        diagnostics.push("missing_reproducibility_metadata".into());
    } else if candidate.input_hash != digest(&input) {
        diagnostics.push("input_hash_mismatch".into());
    }
    if candidate.target.trim().is_empty() {
        diagnostics.push("missing_target".into());
    }
    if candidate.operation.trim().is_empty() {
        diagnostics.push("missing_operation".into());
    }
    if candidate
        .evidence_spans
        .iter()
        .chain(candidate.symbols.iter().flat_map(|s| s.evidence_spans.iter()))
        .chain(candidate.equations.iter().flat_map(|r| r.evidence_spans.iter()))
        .any(|span| !span.valid_for(input))
    {
        diagnostics.push("invalid_evidence_span".into());
    }

    let mut scopes_by_symbol: BTreeMap<&str, BTreeSet<&str>> = BTreeMap::new();
    for symbol in &candidate.symbols {
        if symbol.name.trim().is_empty() || symbol.scope.trim().is_empty() {
            diagnostics.push("empty_symbol_or_scope".into());
        }
        scopes_by_symbol
            .entry(symbol.name.as_str())
            .or_default()
            .insert(symbol.scope.as_str());
        match candidate.symbol_scopes.get(&symbol.name) {
            Some(scope) if scope == &symbol.scope => {}
            Some(_) => diagnostics.push(format!("symbol_scope_mismatch: {}", symbol.name)),
            None => diagnostics.push(format!("missing_symbol_scope: {}", symbol.name)),
        }
    }
    for (symbol, scopes) in scopes_by_symbol {
        if scopes.len() > 1 {
            diagnostics.push(format!("multiple_symbol_scopes: {symbol}"));
        }
    }

    if !candidate.unresolved_ambiguities.is_empty()
        || diagnostics.iter().any(|d| d.starts_with("multiple_symbol_scopes"))
    {
        return receipt(
            ValidationDecision::PreserveAmbiguity,
            diagnostics,
            candidate.candidate_hash(),
        );
    }
    if diagnostics.is_empty() {
        receipt(
            ValidationDecision::AcceptCandidate,
            diagnostics,
            candidate.candidate_hash(),
        )
    } else {
        receipt(
            ValidationDecision::RejectCandidate,
            diagnostics,
            candidate.candidate_hash(),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn span(input: &str, text: &str, role: &str) -> EvidenceSpan {
        let start = input.find(text).expect("test span");
        EvidenceSpan {
            start,
            end: start + text.len(),
            text: text.into(),
            role: role.into(),
        }
    }

    fn candidate(input: &str) -> CandidateSemanticParse {
        let target_span = span(input, "x", "target");
        let symbol = SymbolIR {
            name: "x".into(),
            scope: "root".into(),
            type_name: Some("scalar".into()),
            domain: Some("real".into()),
            declared: true,
            evidence_spans: vec![target_span.clone()],
        };
        CandidateSemanticParse {
            schema: SEMANTIC_IR_SCHEMA.into(),
            input_hash: digest(&input),
            model_id: "test-model".into(),
            model_config_hash: "config-hash".into(),
            prompt_hash: "prompt-hash".into(),
            grammar_version: "grammar-v1".into(),
            target: "x".into(),
            target_kind: TargetKind::Scalar,
            operation: "solve".into(),
            symbols: vec![symbol],
            symbol_scopes: BTreeMap::from([("x".into(), "root".into())]),
            equations: Vec::new(),
            assumptions: Vec::new(),
            domains: vec!["real".into()],
            candidate_pack: Some("linear_algebra".into()),
            unresolved_ambiguities: Vec::new(),
            evidence_spans: vec![target_span],
            confidence: 0.01,
            raw_output_hash: "raw-output-hash".into(),
            replay_hash: String::new(),
        }
        .with_replay_hash()
    }

    #[test]
    fn low_confidence_does_not_block_structurally_valid_proposal() {
        let input = "solve x";
        let proposal = candidate(input);
        let receipt = validate_candidate(input, &proposal);
        assert_eq!(receipt.decision, ValidationDecision::AcceptCandidate);
        assert!(!receipt.downstream_authorized);
        assert!(receipt.replay_verified());
    }

    #[test]
    fn duplicate_scopes_preserve_ambiguity() {
        let input = "solve x";
        let mut proposal = candidate(input);
        proposal.symbols.push(SymbolIR {
            name: "x".into(),
            scope: "nested".into(),
            type_name: Some("scalar".into()),
            domain: Some("real".into()),
            declared: true,
            evidence_spans: vec![span(input, "x", "nested")],
        });
        let receipt = validate_candidate(input, &proposal);
        assert_eq!(receipt.decision, ValidationDecision::PreserveAmbiguity);
        assert!(receipt
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic == "multiple_symbol_scopes: x"));
    }

    #[test]
    fn bad_span_is_rejected_and_not_authorized() {
        let input = "solve x";
        let mut proposal = candidate(input);
        proposal.evidence_spans[0].text = "not present".into();
        let receipt = validate_candidate(input, &proposal);
        assert_eq!(receipt.decision, ValidationDecision::RejectCandidate);
        assert!(!receipt.downstream_authorized);
        assert!(receipt.replay_verified());
    }

    #[test]
    fn candidate_replay_detects_tampering() {
        let input = "solve x";
        let mut proposal = candidate(input);
        assert!(proposal.replay_verified());
        proposal.target = "y".into();
        assert!(!proposal.replay_verified());
    }

    #[test]
    fn unresolved_model_alternatives_are_preserved() {
        let input = "solve x";
        let mut proposal = candidate(input);
        proposal.unresolved_ambiguities = vec!["x may be x_1 or x_2".into()];
        let receipt = validate_candidate(input, &proposal);
        assert_eq!(receipt.decision, ValidationDecision::PreserveAmbiguity);
        assert!(receipt.replay_verified());
    }
}
