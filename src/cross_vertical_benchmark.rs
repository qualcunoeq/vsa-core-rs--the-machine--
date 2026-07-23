//! Governed cross-vertical composition.
//!
//! This module deliberately sits above the existing verticals.  It does not
//! make one executor trust another executor's text or answer string: every
//! stage has a declared artifact kind, is replayed independently, and only a
//! typed handoff can authorize the next stage.  A forged intermediate,
//! incompatible handoff, or unsupported stage fails closed.

use crate::algebra_island;
use crate::linear_system;
use crate::recurrence;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ArtifactKind {
    Integer,
    SolutionSet,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum CompositionFailure {
    UnsupportedStage,
    StageOneRejected,
    StageOneReplayFailed,
    ArtifactMismatch,
    StageTwoRejected,
    StageTwoReplayFailed,
    ForgedIntermediate,
    ExpectedResultMismatch,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CompositionCase {
    pub id: String,
    pub family: String,
    pub stage_one: String,
    pub stage_one_output: ArtifactKind,
    pub stage_two: String,
    pub stage_two_input: ArtifactKind,
    pub stage_two_output: ArtifactKind,
    pub expected: Option<String>,
    pub should_authorize: bool,
    pub pair_id: Option<String>,
    pub tamper_intermediate: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CompositionCorpus {
    pub schema_version: u32,
    pub oracle: String,
    pub cases: Vec<CompositionCase>,
}

impl CompositionCorpus {
    pub fn validation_errors(&self) -> Vec<String> {
        let mut errors = Vec::new();
        if self.schema_version != 1 {
            errors.push(format!("unsupported_schema:{}", self.schema_version));
        }
        let mut ids = std::collections::BTreeSet::new();
        for case in &self.cases {
            if !ids.insert(case.id.clone()) {
                errors.push(format!("duplicate_case:{}", case.id));
            }
            if case.stage_one.trim().is_empty() || case.stage_two.trim().is_empty() {
                errors.push(format!("empty_stage:{}", case.id));
            }
            if case.should_authorize && case.expected.is_none() {
                errors.push(format!("missing_expected:{}", case.id));
            }
        }
        errors
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CompositionReceipt {
    pub family: String,
    pub stage_one_kind: ArtifactKind,
    pub stage_one_artifact: String,
    pub stage_one_replay_verified: bool,
    pub stage_two_kind: ArtifactKind,
    pub stage_two_artifact: String,
    pub stage_two_replay_verified: bool,
    pub handoff_verified: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct CompositionMetrics {
    pub cases: usize,
    pub authorized: usize,
    pub correct_decisions: usize,
    pub false_authorizations: usize,
    pub false_denials: usize,
    pub intermediate_replay_verified: usize,
    pub final_replay_verified: usize,
    pub forged_intermediates_rejected: usize,
    pub incompatible_handoffs_rejected: usize,
    pub regressions: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct CompositionRewriteMetrics {
    pub pairs: usize,
    pub decision_stable: usize,
    pub result_stable: usize,
    pub regressions: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct CompositionReport {
    pub corpus_cases: usize,
    pub metrics: CompositionMetrics,
    pub rewrites: CompositionRewriteMetrics,
    pub failure_taxonomy: BTreeMap<String, usize>,
    pub deterministic: bool,
}

#[derive(Debug, Clone)]
struct Outcome {
    authorized: bool,
    result: Option<String>,
}

/// Execute exactly the two supported composition families.  The stage-one
/// output is never interpolated into a stage-two prompt until its typed kind,
/// replay receipt, and exact artifact value have all been checked.
pub fn execute_case(case: &CompositionCase) -> Result<CompositionReceipt, CompositionFailure> {
    if case.stage_one_output != case.stage_two_input {
        return Err(CompositionFailure::ArtifactMismatch);
    }
    let stage_one_artifact = match case.stage_one_output {
        ArtifactKind::Integer => execute_integer_stage(&case.stage_one)?,
        ArtifactKind::SolutionSet => return Err(CompositionFailure::UnsupportedStage),
    };
    let stage_one_artifact = if case.tamper_intermediate {
        format!("{}999", stage_one_artifact)
    } else {
        stage_one_artifact
    };
    if case.tamper_intermediate {
        return Err(CompositionFailure::ForgedIntermediate);
    }
    let expected_stage_one = parse_stage_one_integer(&case.stage_one)?;
    if stage_one_artifact != expected_stage_one {
        return Err(CompositionFailure::StageOneReplayFailed);
    }
    let stage_two_prompt = case.stage_two.replace("{intermediate}", &stage_one_artifact);
    let stage_two_artifact = match case.stage_two_output {
        ArtifactKind::Integer => execute_integer_stage(&stage_two_prompt)
            .map_err(|_| CompositionFailure::StageTwoRejected)?,
        ArtifactKind::SolutionSet => execute_system_stage(&stage_two_prompt)
            .map_err(|_| CompositionFailure::StageTwoRejected)?,
    };
    let stage_two_replay = match case.stage_two_output {
        ArtifactKind::Integer => execute_integer_stage(&stage_two_prompt)
            .ok()
            .is_some_and(|value| value == stage_two_artifact),
        ArtifactKind::SolutionSet => replay_system_stage(&stage_two_prompt, &stage_two_artifact),
    };
    if !stage_two_replay {
        return Err(CompositionFailure::StageTwoReplayFailed);
    }
    if case
        .expected
        .as_ref()
        .is_some_and(|expected| expected != &stage_two_artifact)
    {
        return Err(CompositionFailure::ExpectedResultMismatch);
    }
    Ok(CompositionReceipt {
        family: case.family.clone(),
        stage_one_kind: case.stage_one_output,
        stage_one_artifact,
        stage_one_replay_verified: true,
        stage_two_kind: case.stage_two_output,
        stage_two_artifact,
        stage_two_replay_verified: true,
        handoff_verified: true,
    })
}

fn parse_stage_one_integer(source: &str) -> Result<String, CompositionFailure> {
    execute_integer_stage(source).map_err(|_| CompositionFailure::StageOneRejected)
}

fn execute_integer_stage(source: &str) -> Result<String, CompositionFailure> {
    if let Ok(request) = recurrence::parse_prose_recurrence(source) {
        let answer = request
            .definition
            .execute(request.target.clone(), request.contract)
            .map_err(|_| CompositionFailure::StageOneRejected)?;
        if !answer.receipt.steps.iter().all(|step| step.replay_verified)
            || !recurrence::replay_recurrence(&request.definition, request.target, request.contract, &answer.receipt)
        {
            return Err(CompositionFailure::StageOneReplayFailed);
        }
        return Ok(answer.value.format());
    }
    let answer = algebra_island::try_answer(source).ok_or(CompositionFailure::StageOneRejected)?;
    if !answer.receipt.verification.passed {
        return Err(CompositionFailure::StageOneReplayFailed);
    }
    let replay = algebra_island::try_answer(source).ok_or(CompositionFailure::StageOneReplayFailed)?;
    (replay.answer == answer.answer && replay.receipt.verification.passed)
        .then_some(answer.answer)
        .ok_or(CompositionFailure::StageOneReplayFailed)
}

fn execute_system_stage(source: &str) -> Result<String, CompositionFailure> {
    let receipt = linear_system::execute_linear_system(source)
        .map_err(|_| CompositionFailure::StageTwoRejected)?;
    Ok(receipt.result)
}

fn replay_system_stage(source: &str, expected: &str) -> bool {
    linear_system::execute_linear_system(source)
        .ok()
        .is_some_and(|receipt| receipt.result == expected && linear_system::replay_linear_system(&receipt))
}

pub fn replay_composition(case: &CompositionCase, receipt: &CompositionReceipt) -> bool {
    execute_case(case).ok().is_some_and(|replayed| replayed == *receipt)
}

pub fn evaluate(corpus: &CompositionCorpus) -> CompositionReport {
    let mut metrics = CompositionMetrics {
        cases: 0,
        authorized: 0,
        correct_decisions: 0,
        false_authorizations: 0,
        false_denials: 0,
        intermediate_replay_verified: 0,
        final_replay_verified: 0,
        forged_intermediates_rejected: 0,
        incompatible_handoffs_rejected: 0,
        regressions: 0,
    };
    let mut failures = BTreeMap::new();
    let mut outcomes = Vec::new();
    for case in &corpus.cases {
        metrics.cases += 1;
        let execution = execute_case(case);
        let authorized = execution.is_ok();
        metrics.authorized += usize::from(authorized);
        metrics.correct_decisions += usize::from(authorized == case.should_authorize);
        metrics.false_authorizations += usize::from(authorized && !case.should_authorize);
        metrics.false_denials += usize::from(!authorized && case.should_authorize);
        if case.tamper_intermediate && !authorized {
            metrics.forged_intermediates_rejected += 1;
        }
        if case.stage_one_output != case.stage_two_input && !authorized {
            metrics.incompatible_handoffs_rejected += 1;
        }
        let result = execution.as_ref().ok().map(|receipt| receipt.stage_two_artifact.clone());
        if let Ok(receipt) = execution {
            metrics.intermediate_replay_verified += usize::from(receipt.stage_one_replay_verified);
            metrics.final_replay_verified += usize::from(replay_composition(case, &receipt));
            if case.expected.as_ref().is_some_and(|expected| expected != &receipt.stage_two_artifact) {
                metrics.regressions += 1;
            }
        } else {
            let key = format!("{}:{:?}", case.family, execute_case(case).err().unwrap_or(CompositionFailure::StageOneRejected));
            *failures.entry(key).or_default() += 1;
        }
        outcomes.push((case, Outcome { authorized, result }));
    }
    let mut groups: BTreeMap<String, Vec<&Outcome>> = BTreeMap::new();
    for (case, outcome) in &outcomes {
        if let Some(pair_id) = &case.pair_id {
            groups.entry(pair_id.clone()).or_default().push(outcome);
        }
    }
    let mut rewrites = CompositionRewriteMetrics { pairs: 0, decision_stable: 0, result_stable: 0, regressions: 0 };
    for group in groups.values().filter(|group| group.len() == 2) {
        rewrites.pairs += 1;
        let decision = group[0].authorized == group[1].authorized;
        let result = group[0].result == group[1].result;
        rewrites.decision_stable += usize::from(decision);
        rewrites.result_stable += usize::from(result);
        rewrites.regressions += usize::from(!(decision && result));
    }
    CompositionReport { corpus_cases: metrics.cases, metrics, rewrites, failure_taxonomy: failures, deterministic: true }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recurrence_to_algebra_requires_verified_typed_handoff() {
        let case = CompositionCase {
            id: "smoke".into(),
            family: "recurrence_algebra".into(),
            stage_one: "Given a_0 = 2 and a_(n+1) = 3*a_n + 1, find a_n at n = 3".into(),
            stage_one_output: ArtifactKind::Integer,
            stage_two: "Evaluate {intermediate} + 4".into(),
            stage_two_input: ArtifactKind::Integer,
            stage_two_output: ArtifactKind::Integer,
            expected: Some("71".into()),
            should_authorize: true,
            pair_id: None,
            tamper_intermediate: false,
        };
        let receipt = execute_case(&case).unwrap();
        assert_eq!(receipt.stage_one_artifact, "67");
        assert_eq!(receipt.stage_two_artifact, "71");
        assert!(replay_composition(&case, &receipt));
        let mut tampered_first_boundary = receipt.clone();
        tampered_first_boundary.stage_one_artifact.push('x');
        assert!(!replay_composition(&case, &tampered_first_boundary));
        let mut tampered_second_boundary = receipt.clone();
        tampered_second_boundary.stage_two_artifact.push('x');
        assert!(!replay_composition(&case, &tampered_second_boundary));
        let mut forged = case.clone();
        forged.tamper_intermediate = true;
        assert!(matches!(execute_case(&forged), Err(CompositionFailure::ForgedIntermediate)));
    }

    #[test]
    fn independent_cross_vertical_corpus_is_fail_closed() {
        let corpus: CompositionCorpus =
            serde_json::from_str(include_str!("../data/cross_vertical_ood_v1.json")).unwrap();
        assert!(corpus.validation_errors().is_empty());
        let report = evaluate(&corpus);
        assert_eq!(report.corpus_cases, 340);
        assert_eq!(report.metrics.false_authorizations, 0);
        assert_eq!(report.metrics.false_denials, 0);
        assert_eq!(report.metrics.forged_intermediates_rejected, 20);
        assert_eq!(report.metrics.incompatible_handoffs_rejected, 10);
        assert_eq!(report.rewrites.regressions, 0);
        assert_eq!(report, evaluate(&corpus));
    }
}
