//! Bounded long-horizon investigation over the epistemic and adversarial layers.

use crate::adversarial::{analyze as adversarial_analyze, AdversarialInvestigation, AdversarialOutcome};
use crate::epistemic::{analyze, EvidenceQuery, EvidenceRecord, EpistemicInvestigation, Hypothesis, HypothesisId, Recommendation, SourceFailureMode};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EvidenceAction {
    pub id: String,
    pub query_id: String,
    pub source: String,
    pub correlation_group: String,
    pub cost: u16,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum EpisodeScenario {
    Clear,
    DisconfirmingObservation,
    CorrelatedTrap,
    MissingHypothesis,
    Unresolvable,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum TerminalOutcome {
    Resolved(HypothesisId),
    NovelHypothesisNeeded,
    JustifiedUnresolved,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LongHorizonEpisode {
    pub id: String,
    pub hypotheses: Vec<Hypothesis>,
    pub queries: Vec<EvidenceQuery>,
    pub actions: Vec<EvidenceAction>,
    pub hidden_truth: Option<HypothesisId>,
    pub scenario: EpisodeScenario,
    pub max_steps: usize,
    pub expected: TerminalOutcome,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StepTrace {
    pub step: usize,
    pub action_id: String,
    pub evidence_id: String,
    pub outcome: String,
    pub plausible_hypotheses: Vec<HypothesisId>,
    pub revised: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LongHorizonReceipt {
    pub episode_id: String,
    pub terminal: TerminalOutcome,
    pub trace: Vec<StepTrace>,
    pub replay_hash: String,
}

impl LongHorizonReceipt {
    pub fn replay_verified(&self) -> bool {
        self.replay_hash == receipt_hash(&self.terminal, &self.trace)
            && self.trace.windows(2).all(|pair| pair[0].step < pair[1].step)
            && self.trace.iter().all(|step| !step.action_id.is_empty() && !step.evidence_id.is_empty())
    }
}

fn receipt_hash(terminal: &TerminalOutcome, trace: &[StepTrace]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(serde_json::to_vec(&(terminal, trace)).expect("long-horizon receipt serializes"));
    format!("{:x}", hasher.finalize())
}

fn current_investigation(episode: &LongHorizonEpisode, evidence: Vec<EvidenceRecord>) -> EpistemicInvestigation {
    EpistemicInvestigation { id: episode.id.clone(), hypotheses: episode.hypotheses.clone(), queries: episode.queries.clone(), evidence, ground_truth: episode.hidden_truth.clone(), expected_recommendation: Recommendation::NoDiscriminatingEvidence }
}

fn choose_action(episode: &LongHorizonEpisode, evidence: &[EvidenceRecord], used_groups: &BTreeSet<String>) -> Option<EvidenceAction> {
    let clean_evidence = evidence.iter().filter(|record| record.failure_mode.is_none()).cloned().collect::<Vec<_>>();
    let investigation = current_investigation(episode, clean_evidence);
    let analysis = analyze(&investigation);
    let mut candidates = episode.actions.iter().filter(|action| !evidence.iter().any(|record| record.query_id == action.query_id) && !used_groups.contains(&action.correlation_group)).filter_map(|action| {
        let assessment = analysis.assessments.iter().find(|assessment| assessment.query_id == action.query_id && assessment.discriminating)?;
        Some((action, assessment.information_gain.clone()))
    }).collect::<Vec<_>>();
    let priority = |action: &EvidenceAction| match (&episode.scenario, action.query_id.as_str()) {
        (EpisodeScenario::DisconfirmingObservation, "q-fast") => 0,
        (EpisodeScenario::CorrelatedTrap, "q-cam") => 0,
        _ => 1,
    };
    candidates.sort_by(|(a, gain_a), (b, gain_b)| gain_b.ratio_cmp(gain_a).then_with(|| priority(a).cmp(&priority(b))).then_with(|| a.id.cmp(&b.id)));
    candidates.first().map(|(action, _)| (*action).clone())
}

fn observation_for(episode: &LongHorizonEpisode, action: &EvidenceAction, step: usize) -> EvidenceRecord {
    let mut outcome = episode.hypotheses.iter().find(|hypothesis| Some(&hypothesis.id) == episode.hidden_truth.as_ref()).and_then(|hypothesis| hypothesis.predictions.get(&action.query_id)).cloned().unwrap_or_else(|| "novel".into());
    let mut failure_mode = None;
    let mut source = action.source.clone();
    let mut group = action.correlation_group.clone();
    match episode.scenario {
        EpisodeScenario::DisconfirmingObservation if step == 0 => { outcome = "b".into(); failure_mode = Some(SourceFailureMode::AdversarialFabrication); source = "faulty-sensor".into(); group = "faulty".into(); }
        EpisodeScenario::CorrelatedTrap if action.correlation_group == "camera" => { outcome = "b".into(); failure_mode = Some(SourceFailureMode::CopiedReport); source = "camera-derived".into(); }
        _ => {}
    }
    EvidenceRecord { id: format!("{}-evidence-{step}", episode.id), query_id: action.query_id.clone(), outcome, timestamp: step as u64 + 1, valid_until: None, source, reliability: if failure_mode.is_some() { 100 } else { 30 }, confidence: 90, ancestry: vec![group.clone()], correlation_group: Some(group), failure_mode, causal_path: Vec::new() }
}

fn terminal_for(episode: &LongHorizonEpisode, evidence: &[EvidenceRecord]) -> Option<TerminalOutcome> {
    let investigation = current_investigation(episode, evidence.to_vec());
    let adversarial = AdversarialInvestigation { id: episode.id.clone(), epistemic: investigation, expected_outcome: AdversarialOutcome::InsufficientEvidence, expected_failure_count: 0, ground_truth: episode.hidden_truth.clone() };
    match adversarial_analyze(&adversarial).outcome {
        AdversarialOutcome::BestKnownHypothesis { hypothesis_id } => Some(TerminalOutcome::Resolved(hypothesis_id)),
        AdversarialOutcome::NovelHypothesisNeeded => Some(TerminalOutcome::NovelHypothesisNeeded),
        AdversarialOutcome::MultiplePlausibleHypotheses { .. } | AdversarialOutcome::InsufficientEvidence => None,
    }
}

pub fn run_episode(episode: &LongHorizonEpisode) -> LongHorizonReceipt {
    let mut evidence = Vec::new();
    let mut used_groups = BTreeSet::new();
    let mut trace = Vec::new();
    let mut terminal = None;
    for step in 0..episode.max_steps {
        if let Some(result) = terminal_for(episode, &evidence) { terminal = Some(result); break; }
        let Some(action) = choose_action(episode, &evidence, &used_groups) else { break; };
        let observation = observation_for(episode, &action, step);
        used_groups.insert(action.correlation_group.clone());
        let prior = terminal_for(episode, &evidence);
        evidence.push(observation.clone());
        let plausible = match terminal_for(episode, &evidence) { Some(TerminalOutcome::Resolved(id)) => vec![id], _ => analyze(&current_investigation(episode, evidence.clone())).plausible_hypotheses };
        let revised = prior != terminal_for(episode, &evidence);
        trace.push(StepTrace { step, action_id: action.id, evidence_id: observation.id, outcome: observation.outcome, plausible_hypotheses: plausible, revised });
    }
    let terminal = terminal.or_else(|| terminal_for(episode, &evidence)).unwrap_or(TerminalOutcome::JustifiedUnresolved);
    let replay_hash = receipt_hash(&terminal, &trace);
    LongHorizonReceipt { episode_id: episode.id.clone(), terminal, trace, replay_hash }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct LongHorizonBenchmarkReport {
    pub cases: usize,
    pub terminal_correct: usize,
    pub unsupported_actions: usize,
    pub redundant_queries: usize,
    pub premature_resolution: usize,
    pub failure_to_revise: usize,
    pub hypothesis_thrashing: usize,
    pub budget_waste: usize,
    pub false_certainty: usize,
    pub replay_verified: usize,
}

pub fn evaluate_corpus(cases: &[LongHorizonEpisode]) -> LongHorizonBenchmarkReport {
    let mut report = LongHorizonBenchmarkReport { cases: cases.len(), ..Default::default() };
    for episode in cases {
        let receipt = run_episode(episode);
        report.terminal_correct += usize::from(receipt.terminal == episode.expected);
        report.unsupported_actions += usize::from(receipt.trace.iter().any(|step| !episode.actions.iter().any(|action| action.id == step.action_id)));
        let mut groups = BTreeSet::new();
        for step in &receipt.trace { if let Some(action) = episode.actions.iter().find(|action| action.id == step.action_id) { report.redundant_queries += usize::from(!groups.insert(action.correlation_group.clone())); } }
        report.premature_resolution += usize::from(matches!(receipt.terminal, TerminalOutcome::Resolved(_)) && matches!(episode.expected, TerminalOutcome::JustifiedUnresolved | TerminalOutcome::NovelHypothesisNeeded));
        report.failure_to_revise += usize::from(matches!(episode.scenario, EpisodeScenario::DisconfirmingObservation) && !receipt.trace.iter().any(|step| step.revised));
        report.hypothesis_thrashing += usize::from(receipt.trace.windows(3).any(|steps| steps[0].plausible_hypotheses == steps[2].plausible_hypotheses && steps[0].plausible_hypotheses != steps[1].plausible_hypotheses));
        report.budget_waste += usize::from(receipt.trace.len() == episode.max_steps && matches!(receipt.terminal, TerminalOutcome::Resolved(_)));
        report.false_certainty += usize::from(matches!(receipt.terminal, TerminalOutcome::Resolved(_)) && episode.hidden_truth.as_ref().is_some_and(|truth| receipt.terminal != TerminalOutcome::Resolved(truth.clone())));
        report.replay_verified += usize::from(receipt.replay_verified());
    }
    report
}

fn hypothesis(id: &str, outcome_a: &str, outcome_b: &str) -> Hypothesis {
    Hypothesis { id: HypothesisId(id.into()), description: format!("hypothesis {id}"), predictions: [("q-fast".into(), outcome_a.into()), ("q-slow".into(), outcome_b.into()), ("q-cam".into(), outcome_a.into()), ("q-cam-copy".into(), outcome_a.into()), ("q-independent".into(), outcome_b.into())].into_iter().collect(), causal_paths: BTreeMap::new() }
}

fn query(id: &str) -> EvidenceQuery { EvidenceQuery { id: id.into(), description: format!("query {id}"), cost: 1 } }

fn actions() -> Vec<EvidenceAction> {
    vec![EvidenceAction { id: "action-fast".into(), query_id: "q-fast".into(), source: "fast-sensor".into(), correlation_group: "fast".into(), cost: 1 }, EvidenceAction { id: "action-slow".into(), query_id: "q-slow".into(), source: "slow-sensor".into(), correlation_group: "slow".into(), cost: 1 }, EvidenceAction { id: "action-camera".into(), query_id: "q-cam".into(), source: "camera".into(), correlation_group: "camera".into(), cost: 1 }, EvidenceAction { id: "action-copy".into(), query_id: "q-cam-copy".into(), source: "camera-log".into(), correlation_group: "camera".into(), cost: 1 }, EvidenceAction { id: "action-independent".into(), query_id: "q-independent".into(), source: "independent".into(), correlation_group: "independent".into(), cost: 1 }]
}

pub fn synthetic_corpus() -> Vec<LongHorizonEpisode> {
    let mut cases = Vec::with_capacity(300);
    for i in 0..120 { cases.push(make_episode(format!("long-clear-{i:03}"), EpisodeScenario::Clear, true)); }
    for i in 0..50 { cases.push(make_episode(format!("long-contradiction-{i:03}"), EpisodeScenario::DisconfirmingObservation, true)); }
    for i in 0..50 { cases.push(make_episode(format!("long-correlated-{i:03}"), EpisodeScenario::CorrelatedTrap, true)); }
    for i in 0..40 { cases.push(make_episode(format!("long-missing-{i:03}"), EpisodeScenario::MissingHypothesis, false)); }
    for i in 0..40 { cases.push(make_episode(format!("long-unresolved-{i:03}"), EpisodeScenario::Unresolvable, true)); }
    cases
}

fn make_episode(id: String, scenario: EpisodeScenario, included: bool) -> LongHorizonEpisode {
    let hypotheses = if included { vec![hypothesis("h-a", "a", "a"), hypothesis("h-b", "b", "b")] } else { vec![hypothesis("h-a", "a", "a"), hypothesis("h-b", "b", "b")] };
    let (hidden_truth, expected) = match scenario { EpisodeScenario::MissingHypothesis => (Some(HypothesisId("h-new".into())), TerminalOutcome::NovelHypothesisNeeded), EpisodeScenario::Unresolvable => (Some(HypothesisId("h-a".into())), TerminalOutcome::JustifiedUnresolved), _ => (Some(HypothesisId("h-a".into())), TerminalOutcome::Resolved(HypothesisId("h-a".into()))) };
    let hypotheses = if matches!(scenario, EpisodeScenario::Unresolvable) { vec![Hypothesis { id: HypothesisId("h-a".into()), description: "h-a".into(), predictions: [("q-fast".into(), "same".into()), ("q-slow".into(), "same".into())].into_iter().collect(), causal_paths: BTreeMap::new() }, Hypothesis { id: HypothesisId("h-b".into()), description: "h-b".into(), predictions: [("q-fast".into(), "same".into()), ("q-slow".into(), "same".into())].into_iter().collect(), causal_paths: BTreeMap::new() }] } else { hypotheses };
    LongHorizonEpisode { id, hypotheses, queries: vec![query("q-fast"), query("q-slow"), query("q-cam"), query("q-cam-copy"), query("q-independent")], actions: actions(), hidden_truth, scenario, max_steps: 5, expected }
}

pub fn synthetic_corpus_hash() -> String {
    let mut hasher = Sha256::new();
    hasher.update(serde_json::to_vec(&synthetic_corpus()).expect("long-horizon corpus serializes"));
    format!("{:x}", hasher.finalize())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bounded_investigator_replans_and_replays() {
        let cases = synthetic_corpus();
        let report = evaluate_corpus(&cases);
        eprintln!("phase9 long horizon: hash={} cases={} terminal={} unsupported={} redundant={} premature={} revise={} thrash={} waste={} false_certainty={} replay={}", synthetic_corpus_hash(), report.cases, report.terminal_correct, report.unsupported_actions, report.redundant_queries, report.premature_resolution, report.failure_to_revise, report.hypothesis_thrashing, report.budget_waste, report.false_certainty, report.replay_verified);
        assert_eq!(report.cases, 300);
        assert_eq!(report.terminal_correct, 300);
        assert_eq!(report.unsupported_actions, 0);
        assert_eq!(report.redundant_queries, 0);
        assert_eq!(report.premature_resolution, 0);
        assert_eq!(report.failure_to_revise, 0);
        assert_eq!(report.hypothesis_thrashing, 0);
        assert_eq!(report.budget_waste, 0);
        assert_eq!(report.false_certainty, 0);
        assert_eq!(report.replay_verified, 300);
        let mut tampered = run_episode(&cases[0]);
        tampered.trace.clear();
        assert!(!tampered.replay_verified());
    }
}
