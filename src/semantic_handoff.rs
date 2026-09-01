//! Deterministic lowering from a validated semantic proposal to an existing
//! typed problem interface.
//!
//! This is deliberately a handoff, not a solver route.  It accepts only
//! equation-compatible targets for `EquationProblemBindingV1`; other target
//! kinds remain explicitly unsupported until their own typed consumers exist.

use crate::equation_problem_binding::{
    BindingStatus, ConstraintArtifact, DeclaredStatus, EquationProblemBinding,
    FunctionDomainBinding, IndexedObjectBinding, ParenthesizedCandidate, RequestedUnknown,
    SourceSpan, SymbolBinding, SymbolScope,
};
use crate::semantic_ir::{
    validate_candidate, CandidateSemanticParse, TargetKind, ValidationDecision,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::BTreeSet;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HandoffStatus {
    Complete,
    Ambiguous,
    Unsupported,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SemanticProblemHandoff {
    pub status: HandoffStatus,
    pub binding: Option<EquationProblemBinding>,
    pub candidate_hash: String,
    pub diagnostics: Vec<String>,
    pub replay_hash: String,
    pub downstream_authorized: bool,
}

fn digest<T: Serialize>(value: &T) -> String {
    format!(
        "{:x}",
        Sha256::digest(serde_json::to_vec(value).expect("semantic handoff serializes"))
    )
}

fn handoff_hash(handoff: &SemanticProblemHandoff) -> String {
    digest(&(
        handoff.status,
        &handoff.binding,
        &handoff.candidate_hash,
        &handoff.diagnostics,
        handoff.downstream_authorized,
    ))
}

impl SemanticProblemHandoff {
    pub fn replay_verified(&self) -> bool {
        self.replay_hash == handoff_hash(self)
            && !self.downstream_authorized
            && self.candidate_hash.len() == 64
            && self
                .binding
                .as_ref()
                .map(|binding| binding.replay_verified())
                .unwrap_or(true)
    }
}

fn source_span(span: &crate::semantic_ir::EvidenceSpan) -> SourceSpan {
    SourceSpan {
        start: span.start,
        end: span.end,
        text: span.text.clone(),
    }
}

fn make_handoff(
    status: HandoffStatus,
    binding: Option<EquationProblemBinding>,
    candidate_hash: String,
    diagnostics: Vec<String>,
) -> SemanticProblemHandoff {
    let mut result = SemanticProblemHandoff {
        status,
        binding,
        candidate_hash,
        diagnostics,
        replay_hash: String::new(),
        downstream_authorized: false,
    };
    result.replay_hash = handoff_hash(&result);
    result
}

/// Lower an accepted semantic proposal into the existing equation-problem
/// interface.  Validation is repeated here so callers cannot bypass the IR
/// gate by manufacturing a handoff directly.
pub fn lower_candidate_to_equation(
    input: &str,
    candidate: &CandidateSemanticParse,
) -> SemanticProblemHandoff {
    let candidate_hash = candidate.candidate_hash();
    let validation = validate_candidate(input, candidate);
    if !candidate.replay_verified() {
        return make_handoff(
            HandoffStatus::Unsupported,
            None,
            candidate_hash,
            vec!["candidate_replay_failed".into()],
        );
    }
    match validation.decision {
        ValidationDecision::PreserveAmbiguity => {
            return make_handoff(
                HandoffStatus::Ambiguous,
                None,
                candidate_hash,
                validation.diagnostics,
            )
        }
        ValidationDecision::RejectCandidate => {
            return make_handoff(
                HandoffStatus::Unsupported,
                None,
                candidate_hash,
                validation.diagnostics,
            )
        }
        ValidationDecision::AcceptCandidate => {}
    }

    if !matches!(
        candidate.target_kind,
        TargetKind::Scalar | TargetKind::Expression | TargetKind::Set
    ) {
        return make_handoff(
            HandoffStatus::Unsupported,
            None,
            candidate_hash,
            vec!["target_kind_has_no_equation_problem_consumer".into()],
        );
    }

    let symbol_names: BTreeSet<&str> = candidate
        .symbols
        .iter()
        .map(|symbol| symbol.name.as_str())
        .collect();
    let mut diagnostics = Vec::new();
    let mut symbols = Vec::new();
    for symbol in &candidate.symbols {
        symbols.push(SymbolBinding {
            symbol: symbol.name.clone(),
            expression: None,
            type_name: symbol.type_name.clone(),
            domain: symbol.domain.clone(),
            declared_status: if symbol.declared {
                DeclaredStatus::Declared
            } else {
                DeclaredStatus::Inferred
            },
            scope: SymbolScope {
                id: symbol.scope.clone(),
                parent: None,
            },
            source_spans: symbol.evidence_spans.iter().map(source_span).collect(),
            unresolved_alternatives: Vec::new(),
            assumptions: candidate.assumptions.clone(),
            dependencies: Vec::new(),
        });
    }

    let mut constraints = Vec::new();
    for relation in &candidate.equations {
        let unknown_symbols: Vec<String> = relation
            .symbols
            .iter()
            .filter(|name| !symbol_names.contains(name.as_str()) && **name != candidate.target)
            .cloned()
            .collect();
        if !unknown_symbols.is_empty() {
            diagnostics.push(format!(
                "relation_has_unbound_symbols: {}",
                unknown_symbols.join(",")
            ));
        }
        constraints.push(ConstraintArtifact {
            expression: relation.expression.clone(),
            symbols: relation.symbols.clone(),
            kind: relation.kind.clone(),
            source_spans: relation.evidence_spans.iter().map(source_span).collect(),
            assumptions: candidate.assumptions.clone(),
            dependencies: relation.symbols.clone(),
        });
    }
    if !diagnostics.is_empty() {
        return make_handoff(
            HandoffStatus::Unsupported,
            None,
            candidate_hash,
            diagnostics,
        );
    }

    let binding = EquationProblemBinding {
        status: BindingStatus::Complete,
        input: input.to_string(),
        symbols,
        requested_unknown: RequestedUnknown {
            candidates: vec![candidate.target.clone()],
            selected: Some(candidate.target.clone()),
            source_spans: candidate.evidence_spans.iter().map(source_span).collect(),
            unresolved_alternatives: Vec::new(),
        },
        indexed_objects: Vec::<IndexedObjectBinding>::new(),
        function_domains: Vec::<FunctionDomainBinding>::new(),
        parenthesized_candidates: Vec::<ParenthesizedCandidate>::new(),
        constraints,
        assumptions: candidate.assumptions.clone(),
        unresolved_alternatives: candidate.unresolved_ambiguities.clone(),
        dependencies: candidate
            .symbols
            .iter()
            .map(|symbol| symbol.name.clone())
            .collect(),
        reason: "lowered from validated semantic proposal; solver selection deferred".into(),
        replay_hash: String::new(),
        downstream_authorized: false,
    }
    .with_replay_hash();
    let handoff = make_handoff(
        HandoffStatus::Complete,
        Some(binding),
        candidate_hash,
        Vec::new(),
    );
    if handoff
        .binding
        .as_ref()
        .is_some_and(|binding| !binding.replay_verified())
    {
        make_handoff(
            HandoffStatus::Unsupported,
            None,
            handoff.candidate_hash,
            vec!["lowered_binding_replay_failed".into()],
        )
    } else {
        handoff
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::semantic_ir::{EvidenceSpan, RelationIR, SymbolIR, SEMANTIC_IR_SCHEMA};
    use std::collections::BTreeMap;

    fn candidate(input: &str, target_kind: TargetKind) -> CandidateSemanticParse {
        let span = EvidenceSpan {
            start: 6,
            end: 7,
            text: "x".into(),
            role: "target".into(),
        };
        CandidateSemanticParse {
            schema: SEMANTIC_IR_SCHEMA.into(),
            input_hash: digest(&input),
            model_id: "test-model".into(),
            model_config_hash: "config".into(),
            prompt_hash: "prompt".into(),
            grammar_version: "grammar".into(),
            target: "x".into(),
            target_kind,
            operation: "solve".into(),
            symbols: vec![SymbolIR {
                name: "x".into(),
                scope: "root".into(),
                type_name: Some("scalar".into()),
                domain: Some("real".into()),
                declared: true,
                evidence_spans: vec![span.clone()],
            }],
            symbol_scopes: BTreeMap::from([("x".into(), "root".into())]),
            equations: vec![RelationIR {
                kind: "constraint".into(),
                expression: "x = 1".into(),
                symbols: vec!["x".into()],
                evidence_spans: vec![span.clone()],
            }],
            assumptions: Vec::new(),
            domains: vec!["real".into()],
            candidate_pack: None,
            unresolved_ambiguities: Vec::new(),
            evidence_spans: vec![span],
            confidence: 0.0,
            raw_output_hash: "raw".into(),
            replay_hash: String::new(),
        }
        .with_replay_hash()
    }

    #[test]
    fn lowers_equation_candidate_without_authorizing_solver() {
        let input = "solve x";
        let handoff = lower_candidate_to_equation(input, &candidate(input, TargetKind::Scalar));
        assert_eq!(handoff.status, HandoffStatus::Complete);
        assert!(handoff.binding.is_some());
        assert!(!handoff.downstream_authorized);
        assert!(handoff.replay_verified());
    }

    #[test]
    fn property_target_is_not_forced_into_equation_binding() {
        let input = "solve x";
        let handoff = lower_candidate_to_equation(input, &candidate(input, TargetKind::Property));
        assert_eq!(handoff.status, HandoffStatus::Unsupported);
        assert!(handoff.binding.is_none());
        assert!(handoff.replay_verified());
    }

    #[test]
    fn unbound_relation_symbol_is_rejected() {
        let input = "solve x";
        let mut proposal = candidate(input, TargetKind::Scalar);
        proposal.equations[0].symbols.push("y".into());
        proposal = proposal.with_replay_hash();
        let handoff = lower_candidate_to_equation(input, &proposal);
        assert_eq!(handoff.status, HandoffStatus::Unsupported);
        assert!(handoff
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.starts_with("relation_has_unbound_symbols")));
    }
}
