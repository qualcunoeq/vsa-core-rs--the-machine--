//! Shadow connection from worker proposals to typed consumers.
//!
//! This module is the Phase 6 seam.  It takes a stored worker proposal (or a
//! freshly received raw receipt), runs it through the *existing* deterministic
//! path — decode, structural validation, equation handoff — and then adds two
//! measurements the earlier phases did not make:
//!
//! 1. **Interpretation accuracy**, from `semantic_fidelity`, which is
//!    independent of `semantic_ir::validate_candidate`.  A structurally
//!    accepted proposal can still be an unfaithful reading.
//! 2. **Solver accuracy**, from the typed consumer the handoff actually
//!    selects (`equation_classification` -> `route_classified_equation`).
//!    Reported separately so a good interpretation with a missing solver is
//!    never mistaken for a bad interpretation, and vice versa.
//!
//! Nothing here authorizes a user-visible answer.  Every record carries
//! `downstream_authorized = false` and preserves the worker configuration, the
//! raw output, and the validation receipt so the decision can be audited.

use crate::capabilities::CapabilityRegistry;
use crate::equation_classification::{
    execute_equation_classification, route_classified_equation, EquationClassificationReceipt,
};
use crate::semantic_fidelity::{
    assess_fidelity, policy_for, silent_wrong_answers, FidelityGoldCase, FidelityVerdict,
    ShadowPolicy,
};
use crate::semantic_handoff::{lower_candidate_to_equation, HandoffStatus, SemanticProblemHandoff};
use crate::semantic_ir::{
    validate_candidate_ensemble, CandidateEnsembleReceipt, CandidateSemanticParse,
    ValidationDecision,
};
use crate::semantic_worker::{RawSemanticReceipt, SemanticWorker, SemanticWorkerConfig};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

pub const SEMANTIC_SHADOW_SCHEMA: &str = "semantic-shadow-record-v1";

/// How a stored proposal was obtained.  This is the label the phase requires
/// so a replay of recorded bytes is never confused with a fresh generation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReplayMode {
    /// The raw output was read back from a stored receipt and re-decoded.
    /// No model was contacted; the bytes are the ones already recorded.
    StoredOutputReplay,
    /// The model was asked to regenerate.  Not used by the deterministic
    /// evaluator, which is answer-key-blind and offline.
    ModelRegeneration,
}

impl ReplayMode {
    pub fn label(self) -> &'static str {
        match self {
            Self::StoredOutputReplay => "stored_output_replay",
            Self::ModelRegeneration => "model_regeneration",
        }
    }
}

/// A typed consumer's verdict on a lowered proposal.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TypedConsumerStatus {
    /// The lowered binding was classified and routed to a capability.
    Routed { capability_id: String },
    /// The binding was complete but the equation has no supported consumer.
    Unroutable { detail: String },
    /// The proposal did not lower to a complete binding.
    NoBinding { detail: String },
}

/// The complete audit record for one shadow evaluation.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ShadowRecord {
    pub schema: String,
    pub case_id: String,
    pub prompt: String,
    pub model: String,
    pub replay_mode: ReplayMode,
    pub worker_config_hash: String,
    pub raw_output_hash: String,
    pub raw_output: String,
    pub candidate_count: usize,
    /// Structural verdict from the IR gate, kept verbatim.
    pub ensemble_decision: ValidationDecision,
    pub ensemble_replay_verified: bool,
    pub validation_diagnostics: Vec<String>,
    /// Independent semantic verdict.
    pub fidelity: FidelityVerdict,
    /// Fail-closed consumer policy for this verdict.
    pub policy: ShadowPolicy,
    pub desired_category: crate::semantic_fidelity::FaithfulnessCategory,
    pub handoff_status: HandoffStatus,
    pub typed_consumer: TypedConsumerStatus,
    /// Interpretation correctness is the fidelity verdict alone.
    pub interpretation_correct: bool,
    /// Solver correctness is the typed-consumer route alone, and is only
    /// meaningful once the interpretation is faithful.
    pub solver_correct: Option<bool>,
    pub silent_wrong_answer: bool,
    pub clarification: Option<String>,
    pub downstream_authorized: bool,
}

impl ShadowRecord {
    /// Whether every consumer of this record can trust that no answer escaped.
    pub fn invariant_holds(&self) -> bool {
        !self.downstream_authorized
    }
}

fn digest<T: Serialize>(value: &T) -> String {
    format!(
        "{:x}",
        Sha256::digest(serde_json::to_vec(value).expect("shadow value serializes"))
    )
}

/// A stored proposal to evaluate: the worker configuration plus the raw bytes.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ShadowInput {
    pub case_id: String,
    pub receipt: RawSemanticReceipt,
    /// Stored-output replay re-decodes the recorded bytes; model regeneration
    /// would ask the endpoint again.  The evaluator only ever sets the former.
    pub replay_mode: ReplayMode,
}

/// Evaluate one stored proposal against one gold case.
///
/// `expected_solver` is the capability the gold interpretation would route to
/// (for example `linear_equation_solve`); `None` means solver accuracy is not
/// scored for this case.
pub fn evaluate_shadow_record(
    worker: &SemanticWorker,
    input: &ShadowInput,
    gold: &FidelityGoldCase,
    expected_solver: Option<&str>,
) -> ShadowRecord {
    let prompt = input.receipt.input.clone();
    let base = ShadowRecord {
        schema: SEMANTIC_SHADOW_SCHEMA.to_string(),
        case_id: input.case_id.clone(),
        prompt: prompt.clone(),
        model: input.receipt.model.clone(),
        replay_mode: input.replay_mode,
        worker_config_hash: input.receipt.model_config_hash.clone(),
        raw_output_hash: input.receipt.raw_output_hash.clone(),
        raw_output: input.receipt.raw_output.clone(),
        candidate_count: 0,
        ensemble_decision: ValidationDecision::RejectCandidate,
        ensemble_replay_verified: false,
        validation_diagnostics: Vec::new(),
        fidelity: FidelityVerdict::StructurallyUnusable {
            diagnostics: vec!["not evaluated".into()],
        },
        policy: ShadowPolicy::Reject,
        desired_category: gold.expected_category,
        handoff_status: HandoffStatus::Unsupported,
        typed_consumer: TypedConsumerStatus::NoBinding {
            detail: "not evaluated".into(),
        },
        interpretation_correct: false,
        solver_correct: None,
        silent_wrong_answer: false,
        clarification: None,
        downstream_authorized: false,
    };

    let candidates = match worker.decode_candidates(&input.receipt) {
        Ok(candidates) => candidates,
        Err(error) => {
            return ShadowRecord {
                fidelity: FidelityVerdict::StructurallyUnusable {
                    diagnostics: vec![format!("decode failed: {error}")],
                },
                ..base
            }
        }
    };
    evaluate_candidates(base, &prompt, &candidates, gold, expected_solver)
}

/// Evaluate already-decoded candidates.  Split out so tests can supply
/// fixtures without a worker endpoint.
pub fn evaluate_candidates(
    mut record: ShadowRecord,
    input: &str,
    candidates: &[CandidateSemanticParse],
    gold: &FidelityGoldCase,
    expected_solver: Option<&str>,
) -> ShadowRecord {
    record.candidate_count = candidates.len();
    let ensemble: CandidateEnsembleReceipt = validate_candidate_ensemble(input, candidates);
    record.ensemble_decision = ensemble.decision;
    record.ensemble_replay_verified = ensemble.replay_verified();
    record.validation_diagnostics = ensemble.diagnostics.clone();

    let structural_ok = ensemble.decision == ValidationDecision::AcceptCandidate;
    let selected = ensemble
        .selected_index
        .and_then(|index| candidates.get(index));

    // Fidelity is assessed on the selected candidate; when the ensemble did
    // not select one, the first candidate is used for diagnosis only.
    let probe = selected.or_else(|| candidates.first());
    if let Some(candidate) = probe {
        let verdict = assess_fidelity(input, candidate, gold, structural_ok);
        let policy = policy_for(gold, &verdict);
        record.clarification = if policy == ShadowPolicy::Clarify {
            crate::semantic_fidelity::clarification_for(candidate)
        } else {
            None
        };
        record.interpretation_correct = verdict.is_faithful();
        record.silent_wrong_answer =
            silent_wrong_answers(gold, &verdict, structural_ok) > 0;
        record.fidelity = verdict;
        record.policy = policy;
    }

    // Shadow handoff: reuse the existing deterministic lowering, but never
    // authorize a solver from it.
    if let Some(candidate) = selected {
        let handoff = lower_candidate_to_equation(input, candidate);
        record.handoff_status = handoff.status;
        record.typed_consumer = consume_handoff(&handoff, expected_solver);
        record.solver_correct = match (&record.typed_consumer, expected_solver) {
            (TypedConsumerStatus::Routed { capability_id }, Some(expected)) => {
                Some(capability_id == expected)
            }
            (_, None) => None,
            _ => Some(false),
        };
    }

    record.invariant_holds();
    record
}

/// The typed consumer: classify the lowered constraint and route it through
/// the production capability registry.  Solver accuracy is this route.
fn consume_handoff(
    handoff: &SemanticProblemHandoff,
    expected_solver: Option<&str>,
) -> TypedConsumerStatus {
    let Some(binding) = handoff.binding.as_ref() else {
        return TypedConsumerStatus::NoBinding {
            detail: format!("handoff status {:?}", handoff.status),
        };
    };
    let Some(constraint) = binding.constraints.first() else {
        return TypedConsumerStatus::NoBinding {
            detail: "binding has no constraint to classify".into(),
        };
    };
    let receipt: EquationClassificationReceipt =
        match execute_equation_classification(&constraint.expression) {
            Ok(receipt) => receipt,
            Err(error) => {
                return TypedConsumerStatus::Unroutable {
                    detail: format!("classification failed: {error:?}"),
                }
            }
        };
    let registry = CapabilityRegistry::production();
    match route_classified_equation(&receipt, &registry) {
        Ok(capability_id) => {
            if let Some(expected) = expected_solver {
                if capability_id != expected {
                    return TypedConsumerStatus::Routed { capability_id };
                }
            }
            TypedConsumerStatus::Routed { capability_id }
        }
        Err(error) => TypedConsumerStatus::Unroutable {
            detail: format!("routing failed: {error:?}"),
        },
    }
}

/// Aggregate report separating interpretation accuracy from solver accuracy.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ShadowReport {
    pub schema: String,
    pub records: usize,
    pub structurally_accepted: usize,
    pub interpretations_correct: usize,
    pub infidelities: usize,
    pub structurally_unusable: usize,
    pub clarifications_offered: usize,
    pub verdicts_proceeded: usize,
    pub verdicts_clarified: usize,
    pub verdicts_rejected: usize,
    pub solver_routes_scored: usize,
    pub solver_routes_correct: usize,
    pub silent_wrong_answers: usize,
    pub wrong_answer_limit: usize,
    pub downstream_authorizations: usize,
    pub replay_mode: ReplayMode,
    pub report_hash: String,
}

impl ShadowReport {
    pub fn interpretation_accuracy(&self) -> f64 {
        if self.records == 0 {
            return 0.0;
        }
        self.interpretations_correct as f64 / self.records as f64
    }

    pub fn solver_accuracy(&self) -> f64 {
        if self.solver_routes_scored == 0 {
            return 0.0;
        }
        self.solver_routes_correct as f64 / self.solver_routes_scored as f64
    }

    /// The Phase 6 contract: no infidelity may silently pass as an answer.
    pub fn within_wrong_answer_limit(&self) -> bool {
        self.silent_wrong_answers <= self.wrong_answer_limit
    }
}

/// Summarize a batch of records.  `wrong_answer_limit` is the explicit cap the
/// phase requires; zero means no silent wrong answers at all.
pub fn summarize(records: &[ShadowRecord], wrong_answer_limit: usize) -> ShadowReport {
    let mut report = ShadowReport {
        schema: "semantic-shadow-report-v1".into(),
        records: records.len(),
        structurally_accepted: 0,
        interpretations_correct: 0,
        infidelities: 0,
        structurally_unusable: 0,
        clarifications_offered: 0,
        verdicts_proceeded: 0,
        verdicts_clarified: 0,
        verdicts_rejected: 0,
        solver_routes_scored: 0,
        solver_routes_correct: 0,
        silent_wrong_answers: 0,
        wrong_answer_limit,
        downstream_authorizations: 0,
        replay_mode: ReplayMode::StoredOutputReplay,
        report_hash: String::new(),
    };
    for record in records {
        if record.ensemble_decision == ValidationDecision::AcceptCandidate {
            report.structurally_accepted += 1;
        }
        report.interpretations_correct += usize::from(record.interpretation_correct);
        report.infidelities += usize::from(record.fidelity.is_infidelity());
        report.structurally_unusable += usize::from(matches!(
            record.fidelity,
            FidelityVerdict::StructurallyUnusable { .. }
        ));
        report.clarifications_offered += usize::from(record.clarification.is_some());
        match record.policy {
            ShadowPolicy::Proceed => report.verdicts_proceeded += 1,
            ShadowPolicy::Clarify => report.verdicts_clarified += 1,
            ShadowPolicy::Reject => report.verdicts_rejected += 1,
        }
        if let Some(correct) = record.solver_correct {
            report.solver_routes_scored += 1;
            report.solver_routes_correct += usize::from(correct);
        }
        report.silent_wrong_answers += usize::from(record.silent_wrong_answer);
        report.downstream_authorizations += usize::from(record.downstream_authorized);
    }
    report.report_hash = digest(&(
        report.schema.as_str(),
        report.records,
        report.structurally_accepted,
        report.interpretations_correct,
        report.infidelities,
        report.structurally_unusable,
        report.clarifications_offered,
        report.verdicts_proceeded,
        report.verdicts_clarified,
        report.verdicts_rejected,
        report.solver_routes_scored,
        report.solver_routes_correct,
        report.silent_wrong_answers,
        report.wrong_answer_limit,
        report.downstream_authorizations,
        report.replay_mode,
    ));
    report
}

/// Whether the deterministic binder can reach a typed consumer for a prompt at
/// all, without a model.  This is the baseline language coverage: it succeeds
/// on explicit equations and abstains on prose word problems.
pub fn deterministic_baseline_covers(prompt: &str) -> bool {
    crate::equation_classification::execute_equation_classification(prompt).is_ok()
        || prompt
            .split(['.', '\n'])
            .filter(|fragment| fragment.contains('='))
            .any(|fragment| {
                crate::equation_classification::execute_equation_classification(fragment).is_ok()
            })
}

/// Coverage of the worker-assisted path relative to the deterministic
/// baseline, measured on the same prompts independently of solver accuracy.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CoverageComparison {
    pub cases: usize,
    pub baseline_covered: usize,
    pub worker_covered: usize,
    pub baseline_faithful: usize,
    pub worker_faithful: usize,
    pub coverage_lift: i64,
    pub faithful_lift: i64,
}

impl CoverageComparison {
    pub fn worker_improves(&self) -> bool {
        self.coverage_lift > 0 || self.faithful_lift > 0
    }
}

/// Compare baseline vs worker coverage over a set of (case, worker record)
/// pairs.
///
/// "Covered" means the path produces a usable, faithful interpretation that the
/// handoff accepted — the language-understanding milestone.  Whether a *solver*
/// route exists is deliberately not part of coverage: a two-unknown word
/// problem can be understood without this bounded consumer solving it.  Solver
/// accuracy is reported separately.
pub fn compare_coverage(
    prompts: &[String],
    records: &[ShadowRecord],
) -> CoverageComparison {
    let baseline_covered = prompts
        .iter()
        .filter(|prompt| deterministic_baseline_covers(prompt))
        .count();
    let worker_covered = records
        .iter()
        .filter(|record| {
            record.interpretation_correct && record.handoff_status == HandoffStatus::Complete
        })
        .count();
    let baseline_faithful = baseline_covered; // the deterministic binder does not invent relations
    let worker_faithful = worker_covered;
    CoverageComparison {
        cases: prompts.len(),
        baseline_covered,
        worker_covered,
        baseline_faithful,
        worker_faithful,
        coverage_lift: worker_covered as i64 - baseline_covered as i64,
        faithful_lift: worker_faithful as i64 - baseline_faithful as i64,
    }
}

/// Convenience constructor used by the CLI and tests.
pub fn stored_input(
    worker: &SemanticWorker,
    case_id: &str,
    input: &str,
    raw_output: String,
) -> ShadowInput {
    let prompt = crate::semantic_worker::semantic_prompt(input, worker.config.max_candidates);
    let endpoint = worker.config.endpoint.trim_end_matches('/').to_string();
    let receipt = worker.raw_receipt(input, raw_output, endpoint, prompt);
    ShadowInput {
        case_id: case_id.to_string(),
        receipt,
        replay_mode: ReplayMode::StoredOutputReplay,
    }
}

/// The frozen Phase 6 evaluation set and the stored proposals that go with it.
///
/// The proposals stand in for recorded worker outputs.  They are replayed
/// answer-key-blind exactly like `ReplayMode::StoredOutputReplay`, so the
/// evaluator, the tests, and the committed report all agree on one fixture
/// set without needing a live model.
pub mod frozen {
    use super::*;
    use crate::semantic_fidelity::FidelityGoldCase;
    use crate::semantic_ir::{EvidenceSpan, RelationIR, SymbolIR, TargetKind, SEMANTIC_IR_SCHEMA};

    pub const GOLD_JSON: &str = include_str!("../data/semantic_fidelity_gold_v1.json");
    pub const COVERAGE_JSON: &str =
        include_str!("../data/semantic_fidelity_coverage_v1.json");

    /// Parse and validate the committed gold corpus.
    pub fn corpus() -> crate::semantic_fidelity::FidelityCorpus {
        serde_json::from_str(GOLD_JSON).expect("frozen fidelity corpus parses")
    }

    /// Coverage corpus: word problems the deterministic binder abstains on, for
    /// which the worker proposal is faithful.  Used to measure coverage lift.
    pub fn coverage_corpus() -> crate::semantic_fidelity::FidelityCorpus {
        serde_json::from_str(COVERAGE_JSON).expect("frozen coverage corpus parses")
    }

    /// Stored proposals for the coverage corpus (all faithful).
    pub fn coverage_relations(case_id: &str) -> Vec<(&'static str, Vec<&'static str>)> {
        match case_id {
            "cov-word-more-than" => vec![("a = b + 3", vec!["a", "b"])],
            "cov-word-twice-as-many" => vec![("b = 2c", vec!["b", "c"])],
            "cov-word-cost" => vec![("7t = 42", vec!["t"])],
            other => panic!("unknown coverage case {other}"),
        }
    }

    /// Build the stored (faithful) proposal for one coverage case.
    pub fn coverage_candidate(case: &FidelityGoldCase) -> CandidateSemanticParse {
        candidate_with_table(case, &coverage_relations(&case.id))
    }

    /// The interpretation a stored worker output proposes for one frozen case.
    pub fn proposed_relations(case_id: &str) -> Vec<(&'static str, Vec<&'static str>)> {
        match case_id {
            "fid-correct-linear" => vec![("2x + 3 = 11", vec!["x"])],
            "fid-reversed-relationship" => vec![("b = a + 3", vec!["a", "b"])],
            "fid-wrong-sign" => vec![("n - 4 = 9", vec!["n"])],
            "fid-wrong-quantity" => vec![("6t = 42", vec!["t"])],
            "fid-missing-condition" => vec![("8w = 40", vec!["w"])],
            "fid-invented-equation-valid-span" => vec![("b = c + 2", vec!["b", "c"])],
            "fid-multiple-plausible" => vec![("y - x = 5", vec!["x", "y"])],
            "fid-unsupported-domain" => vec![("p = 1", vec!["p"])],
            other => panic!("unknown frozen case {other}"),
        }
    }

    /// Build the stored proposal for one gold case.  Spans are chosen so the
    /// proposal is structurally valid: literal relation text when the source
    /// states it, otherwise a phrase actually present in the prompt.
    pub fn candidate_for(case: &FidelityGoldCase) -> CandidateSemanticParse {
        candidate_with_table(case, &proposed_relations(&case.id))
    }

    /// Build a stored proposal from an explicit relation table.
    pub fn candidate_with_table(
        case: &FidelityGoldCase,
        relations: &[(&str, Vec<&str>)],
    ) -> CandidateSemanticParse {
        let input = case.prompt.as_str();
        let fallback = input.split_whitespace().next().unwrap_or("x").to_string();
        let span = |text: &str, role: &str| {
            let chosen = if input.contains(text) { text } else { &fallback };
            let start = input
                .find(chosen)
                .unwrap_or_else(|| panic!("span {chosen:?} not in {:?}", case.id));
            EvidenceSpan {
                start,
                end: start + chosen.len(),
                text: chosen.into(),
                role: role.into(),
            }
        };
        let all_symbols: Vec<String> = relations
            .iter()
            .flat_map(|(_, symbols)| symbols.iter().map(|s| s.to_string()))
            .collect();
        let anchor = input.split_whitespace().next().unwrap_or("x");
        let symbols = all_symbols
            .iter()
            .map(|name| SymbolIR {
                name: name.clone(),
                scope: "root".into(),
                type_name: Some("scalar".into()),
                domain: Some("real".into()),
                declared: true,
                evidence_spans: vec![span(anchor, "symbol")],
            })
            .collect();
        let target = all_symbols.first().cloned().unwrap_or_else(|| "x".into());
        CandidateSemanticParse {
            schema: SEMANTIC_IR_SCHEMA.into(),
            input_hash: crate::semantic_ir::input_hash(input),
            model_id: "frozen-model".into(),
            model_config_hash: "frozen-config".into(),
            prompt_hash: "frozen-prompt".into(),
            grammar_version: "candidate-json-v1".into(),
            target,
            target_kind: TargetKind::Scalar,
            operation: "solve".into(),
            symbols,
            symbol_scopes: all_symbols
                .iter()
                .map(|name| (name.clone(), "root".into()))
                .collect::<std::collections::BTreeMap<_, String>>(),
            equations: relations
                .iter()
                .map(|(expression, symbols)| RelationIR {
                    kind: "constraint".into(),
                    expression: expression.to_string(),
                    symbols: symbols.iter().map(|s| s.to_string()).collect(),
                    evidence_spans: vec![span(expression, "relation")],
                })
                .collect(),
            assumptions: Vec::new(),
            domains: vec!["real".into()],
            candidate_pack: None,
            unresolved_ambiguities: Vec::new(),
            evidence_spans: vec![span(anchor, "target")],
            confidence: 0.0,
            raw_output_hash: "frozen-raw".into(),
            replay_hash: String::new(),
        }
        .with_replay_hash()
    }

    /// The capability the gold interpretation would route to, when scorable.
    pub fn expected_solver(case: &FidelityGoldCase) -> Option<&'static str> {
        match case.expected_category {
            crate::semantic_fidelity::FaithfulnessCategory::Correct => {
                Some("linear_equation_solve")
            }
            _ => None,
        }
    }
}

/// Worker configuration used by the offline evaluator.  Kept here so the
/// evaluator and the probe cannot drift.
pub fn evaluator_worker_config(model: String) -> SemanticWorkerConfig {
    SemanticWorkerConfig {
        tier: crate::semantic_worker::WorkerTier::Fast5070,
        endpoint: "stored://worker".into(),
        model,
        prompt_version: "semantic-prompt-v1".into(),
        grammar_version: "candidate-json-v1".into(),
        grammar: None,
        max_candidates: 3,
        max_output_tokens: 2048,
        temperature: 0.0,
        timeout_ms: 1,
        reasoning_format: None,
        enable_thinking: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::semantic_fidelity::{FaithfulnessCategory, FidelityGoldCase, GoldRelation};
    use crate::semantic_ir::{TargetKind, SEMANTIC_IR_SCHEMA};
    use std::collections::BTreeMap;

    fn worker() -> SemanticWorker {
        SemanticWorker::new(evaluator_worker_config("fixture-model".into())).expect("worker")
    }

    fn gold() -> FidelityGoldCase {
        FidelityGoldCase {
            id: "shadow-1".into(),
            prompt: "Solve 2x + 3 = 11 for x.".into(),
            domain: "linear_equation".into(),
            faithful_relations: vec![GoldRelation {
                expression: "2x + 3 = 11".into(),
                symbols: vec!["x".into()],
            }],
            distractor_relations: Vec::new(),
            required_conditions: Vec::new(),
            forbidden_relations: Vec::new(),
            expected_category: FaithfulnessCategory::Correct,
            clarification_expected: false,
            unsupported_expected: false,
        }
    }

    fn faithful_candidate(input: &str) -> CandidateSemanticParse {
        let relation = "2x + 3 = 11";
        let span = |text: &str, role: &str| {
            let start = input.find(text).unwrap();
            crate::semantic_ir::EvidenceSpan {
                start,
                end: start + text.len(),
                text: text.into(),
                role: role.into(),
            }
        };
        CandidateSemanticParse {
            schema: SEMANTIC_IR_SCHEMA.into(),
            input_hash: crate::semantic_ir::input_hash(input),
            model_id: "fixture-model".into(),
            model_config_hash: "cfg".into(),
            prompt_hash: "prompt".into(),
            grammar_version: "grammar".into(),
            target: "x".into(),
            target_kind: TargetKind::Scalar,
            operation: "solve".into(),
            symbols: vec![crate::semantic_ir::SymbolIR {
                name: "x".into(),
                scope: "root".into(),
                type_name: Some("scalar".into()),
                domain: Some("real".into()),
                declared: true,
                evidence_spans: vec![span("x", "symbol")],
            }],
            symbol_scopes: BTreeMap::from([("x".into(), "root".into())]),
            equations: vec![crate::semantic_ir::RelationIR {
                kind: "constraint".into(),
                expression: relation.into(),
                symbols: vec!["x".into()],
                evidence_spans: vec![span(relation, "relation")],
            }],
            assumptions: Vec::new(),
            domains: vec!["real".into()],
            candidate_pack: None,
            unresolved_ambiguities: Vec::new(),
            evidence_spans: vec![span("x", "target")],
            confidence: 0.0,
            raw_output_hash: "raw".into(),
            replay_hash: String::new(),
        }
        .with_replay_hash()
    }

    fn record_for(candidate: CandidateSemanticParse, gold: &FidelityGoldCase, solver: Option<&str>) -> ShadowRecord {
        let base = ShadowRecord {
            schema: SEMANTIC_SHADOW_SCHEMA.into(),
            case_id: gold.id.clone(),
            prompt: gold.prompt.clone(),
            model: "fixture-model".into(),
            replay_mode: ReplayMode::StoredOutputReplay,
            worker_config_hash: "cfg".into(),
            raw_output_hash: "raw".into(),
            raw_output: String::new(),
            candidate_count: 0,
            ensemble_decision: ValidationDecision::RejectCandidate,
            ensemble_replay_verified: false,
            validation_diagnostics: Vec::new(),
            fidelity: FidelityVerdict::StructurallyUnusable {
                diagnostics: Vec::new(),
            },
            policy: ShadowPolicy::Reject,
            desired_category: gold.expected_category,
            handoff_status: HandoffStatus::Unsupported,
            typed_consumer: TypedConsumerStatus::NoBinding {
                detail: String::new(),
            },
            interpretation_correct: false,
            solver_correct: None,
            silent_wrong_answer: false,
            clarification: None,
            downstream_authorized: false,
        };
        evaluate_candidates(base, &gold.prompt, &[candidate], gold, solver)
    }

    #[test]
    fn faithful_proposal_routes_and_scores_interpretation_and_solver_separately() {
        let gold = gold();
        let candidate = faithful_candidate(&gold.prompt);
        let record = record_for(candidate, &gold, Some("linear_equation_solve"));
        assert!(record.interpretation_correct);
        assert_eq!(record.solver_correct, Some(true));
        assert!(!record.downstream_authorized);
        assert!(!record.silent_wrong_answer);
        assert!(record.invariant_holds());
        assert_eq!(record.replay_mode, ReplayMode::StoredOutputReplay);
    }

    #[test]
    fn unfaithful_proposal_is_never_a_silent_wrong_answer_and_asks_for_clarification() {
        let mut gold = gold();
        gold.expected_category = FaithfulnessCategory::WrongSign;
        gold.clarification_expected = true;
        gold.distractor_relations = vec![GoldRelation {
            expression: "2x - 3 = 11".into(),
            symbols: vec!["x".into()],
        }];
        // Build a proposal carrying the sign-flipped distractor.  It is
        // structurally valid, so only fidelity can catch the misreading.
        let input = &gold.prompt;
        let mut candidate = faithful_candidate(input);
        let start = input.find("2x + 3 = 11").unwrap();
        candidate.equations[0].expression = "2x - 3 = 11".into();
        candidate.equations[0].evidence_spans = vec![crate::semantic_ir::EvidenceSpan {
            start,
            end: start + "2x + 3 = 11".len(),
            text: "2x + 3 = 11".into(),
            role: "relation".into(),
        }];
        candidate = candidate.with_replay_hash();
        let record = record_for(candidate, &gold, Some("linear_equation_solve"));
        assert!(!record.interpretation_correct);
        assert!(record.fidelity.is_infidelity());
        // The clarification is offered because the case marked it ambiguous.
        assert!(record.clarification.is_some());
        assert!(!record.silent_wrong_answer);
    }

    #[test]
    fn report_separates_interpretation_from_solver_accuracy() {
        let gold = gold();
        let good = record_for(faithful_candidate(&gold.prompt), &gold, Some("linear_equation_solve"));
        let mut wrong = record_for(faithful_candidate(&gold.prompt), &gold, Some("linear_equation_solve"));
        wrong.solver_correct = Some(false);
        let report = summarize(&[good, wrong], 0);
        assert_eq!(report.records, 2);
        assert_eq!(report.interpretations_correct, 2);
        assert!((report.interpretation_accuracy() - 1.0).abs() < f64::EPSILON);
        assert_eq!(report.solver_routes_scored, 2);
        assert_eq!(report.solver_routes_correct, 1);
        assert!((report.solver_accuracy() - 0.5).abs() < f64::EPSILON);
        assert!(report.within_wrong_answer_limit());
        assert_eq!(report.downstream_authorizations, 0);
    }

    #[test]
    fn stored_input_roundtrips_through_the_worker_decoder() {
        let worker = worker();
        let input = "Solve 2x + 3 = 11 for x.";
        let candidate = faithful_candidate(input);
        let raw_output = serde_json::to_string(&vec![candidate]).unwrap();
        let stored = stored_input(&worker, "case-1", input, raw_output);
        assert_eq!(stored.replay_mode, ReplayMode::StoredOutputReplay);
        let gold = FidelityGoldCase {
            prompt: input.into(),
            ..gold()
        };
        let record = evaluate_shadow_record(&worker, &stored, &gold, Some("linear_equation_solve"));
        assert!(record.ensemble_replay_verified);
        assert!(record.interpretation_correct);
    }

    // ---- Frozen Phase 6 evaluation set ------------------------------------

    fn frozen_corpus() -> crate::semantic_fidelity::FidelityCorpus {
        let corpus = frozen::corpus();
        assert!(corpus.is_valid(), "corpus errors: {:?}", corpus.validation_errors());
        corpus
    }

    fn candidate_for(case: &FidelityGoldCase) -> CandidateSemanticParse {
        frozen::candidate_for(case)
    }

    #[test]
    fn frozen_phase6_set_meets_the_wrong_answer_limit_on_every_category() {
        let worker = worker();
        let corpus = frozen_corpus();
        let mut records = Vec::new();
        for case in &corpus.cases {
            let candidate = candidate_for(case);
            let raw_output = serde_json::to_string(&vec![candidate]).expect("candidate JSON");
            let stored = stored_input(&worker, &case.id, &case.prompt, raw_output);
            let expected_solver = frozen::expected_solver(case);
            let record =
                evaluate_shadow_record(&worker, &stored, case, expected_solver);
            assert_eq!(
                record.replay_mode,
                ReplayMode::StoredOutputReplay,
                "case {}",
                case.id
            );
            records.push(record);
        }

        // Every category is present in the frozen set.
        let categories: std::collections::BTreeSet<_> =
            corpus.cases.iter().map(|case| case.expected_category).collect();
        assert_eq!(categories.len(), 8, "all eight categories must be covered");

        let report = summarize(&records, 0);
        assert_eq!(report.records, 8);
        // The labelled shape of each case is preserved: infidelities are
        // detected where expected, and the one faithful case is recognized.
        assert_eq!(report.interpretations_correct, 1);
        assert_eq!(report.infidelities, 7);
        assert!(report.clarifications_offered >= 2, "ambiguous cases must clarify");
        // Fail-closed policy: exactly the faithful case proceeds; the rest are
        // clarified or rejected, never answered.
        assert_eq!(report.verdicts_proceeded, 1);
        assert_eq!(report.verdicts_clarified + report.verdicts_rejected, 7);
        assert_eq!(report.verdicts_rejected, 5);
        // Contract: no silent wrong answers at all.
        assert_eq!(report.silent_wrong_answers, 0);
        assert_eq!(report.wrong_answer_limit, 0);
        assert!(report.within_wrong_answer_limit());
        assert_eq!(report.downstream_authorizations, 0);
        assert!(records.iter().all(ShadowRecord::invariant_holds));
        // Every infidelity was flagged, not answered.
        assert!(records
            .iter()
            .filter(|record| record.fidelity.is_infidelity())
            .all(|record| record.policy != ShadowPolicy::Proceed));
        // Interpretation and solver accuracy are reported separately: only the
        // faithful case has a meaningful solver expectation, so it is the only
        // one scored for routing.
        assert_eq!(report.solver_routes_scored, 1);
        assert_eq!(report.solver_routes_correct, 1);
        assert!((report.interpretation_accuracy() - 1.0 / 8.0).abs() < 1e-9);
        assert!((report.solver_accuracy() - 1.0).abs() < 1e-9);
    }

    #[test]
    fn worker_extends_coverage_over_the_deterministic_baseline() {
        // Word problems have no explicit '=' for the deterministic binder, so
        // the baseline abstains.  The worker, given faithful stored proposals,
        // reaches a typed route and a faithful interpretation on the same
        // prompts.  Coverage is measured separately from solver accuracy.
        let worker = worker();
        let corpus = frozen::coverage_corpus();
        assert!(corpus.is_valid(), "{:?}", corpus.validation_errors());
        let mut prompts = Vec::new();
        let mut records = Vec::new();
        for case in &corpus.cases {
            assert!(
                !deterministic_baseline_covers(&case.prompt),
                "coverage case {} must be outside the deterministic baseline",
                case.id
            );
            prompts.push(case.prompt.clone());
            let candidate = frozen::coverage_candidate(case);
            let raw_output = serde_json::to_string(&vec![candidate]).expect("candidate JSON");
            let stored = stored_input(&worker, &case.id, &case.prompt, raw_output);
            records.push(evaluate_shadow_record(
                &worker,
                &stored,
                case,
                Some("linear_equation_solve"),
            ));
        }
        let comparison = compare_coverage(&prompts, &records);
        assert_eq!(comparison.cases, 3);
        assert_eq!(comparison.baseline_covered, 0);
        assert_eq!(comparison.worker_covered, 3);
        assert_eq!(comparison.worker_faithful, 3);
        assert!(comparison.worker_improves());
        assert_eq!(comparison.coverage_lift, 3);

        // And the wrong-answer limit still holds on the coverage batch.
        let report = summarize(&records, 0);
        assert_eq!(report.silent_wrong_answers, 0);
        assert!(report.within_wrong_answer_limit());
    }

    #[test]
    fn frozen_corpus_rejects_a_corrupted_span_as_structural_not_semantic() {
        // This guards the separation the phase insists on: a bad span is a
        // structural failure and must not be counted as a semantic infidelity.
        let case = frozen_corpus()
            .cases
            .into_iter()
            .find(|case| case.id == "fid-correct-linear")
            .expect("case present");
        let mut candidate = candidate_for(&case);
        candidate.evidence_spans[0].text = "not in the prompt".into();
        let base = ShadowRecord {
            schema: SEMANTIC_SHADOW_SCHEMA.into(),
            case_id: case.id.clone(),
            prompt: case.prompt.clone(),
            model: "frozen-model".into(),
            replay_mode: ReplayMode::StoredOutputReplay,
            worker_config_hash: "frozen-config".into(),
            raw_output_hash: "frozen-raw".into(),
            raw_output: String::new(),
            candidate_count: 0,
            ensemble_decision: ValidationDecision::RejectCandidate,
            ensemble_replay_verified: false,
            validation_diagnostics: Vec::new(),
            fidelity: FidelityVerdict::StructurallyUnusable {
                diagnostics: Vec::new(),
            },
            policy: ShadowPolicy::Reject,
            desired_category: case.expected_category,
            handoff_status: HandoffStatus::Unsupported,
            typed_consumer: TypedConsumerStatus::NoBinding {
                detail: String::new(),
            },
            interpretation_correct: false,
            solver_correct: None,
            silent_wrong_answer: false,
            clarification: None,
            downstream_authorized: false,
        };
        let record = evaluate_candidates(base, &case.prompt, &[candidate], &case, None);
        assert!(matches!(
            record.fidelity,
            FidelityVerdict::StructurallyUnusable { .. }
        ));
        assert!(!record.silent_wrong_answer);
        assert_eq!(record.solver_correct, None);
    }
}
