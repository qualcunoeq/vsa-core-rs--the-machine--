//! Independent protocol environment for pressure-testing investigation agency.
//!
//! The environment owns hidden truth and scenario state. The controller sees
//! only protocol replies, never scenario labels or ground-truth internals.

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MachineAction {
    pub request_id: String,
    pub query: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EnvironmentObservation {
    pub request_id: String,
    pub timestamp: u64,
    pub outcome: String,
    pub source: String,
    pub correlation_group: String,
    pub available: bool,
    pub failure_mode: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EnvironmentEvent {
    pub timestamp: u64,
    pub description: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EnvironmentReply {
    pub accepted: bool,
    pub cost: u16,
    pub observations: Vec<EnvironmentObservation>,
    pub events: Vec<EnvironmentEvent>,
    pub delayed: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ExpectedTerminal {
    Resolved(String),
    JustifiedUnresolved,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum EnvironmentScenario {
    Clean,
    DelayedResponse,
    UnavailableQuery,
    ChangingWorld,
    DeceptiveSource,
    UnknownEntity,
    Irresolvable,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProtocolEpisode {
    pub id: String,
    pub scenario: EnvironmentScenario,
    pub expected: ExpectedTerminal,
    pub action_budget: u16,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct PendingObservation {
    deliver_at: u64,
    observation: EnvironmentObservation,
}

#[derive(Debug, Clone)]
pub struct ExternalEnvironment {
    scenario: EnvironmentScenario,
    clock: u64,
    spent: u16,
    pending: Vec<PendingObservation>,
    changed: bool,
    action_count: usize,
    budget: u16,
}

impl ExternalEnvironment {
    pub fn new(episode: &ProtocolEpisode) -> Self {
        Self { scenario: episode.scenario.clone(), clock: 0, spent: 0, pending: Vec::new(), changed: false, action_count: 0, budget: episode.action_budget }
    }

    pub fn submit(&mut self, action: &MachineAction) -> EnvironmentReply {
        self.clock += 1;
        self.action_count += 1;
        let cost = match action.query.as_str() { "status:primary" => 1, "status:secondary" => 2, "status:tertiary" => 2, "entity:unknown" => 1, _ => 1 };
        if self.spent + cost > self.budget { return EnvironmentReply { accepted: false, cost: 0, observations: self.collect_due(), events: Vec::new(), delayed: false }; }
        self.spent += cost;
        let mut events = Vec::new();
        if matches!(self.scenario, EnvironmentScenario::ChangingWorld) && !self.changed {
            self.changed = true;
            events.push(EnvironmentEvent { timestamp: self.clock, description: "world-state changed between observations".into() });
        }
        let (available, outcome, source, group, failure_mode, delay) = match (&self.scenario, action.query.as_str()) {
            (EnvironmentScenario::UnknownEntity, _) | (_, "entity:unknown") => (false, "unknown".into(), "none".into(), "none".into(), None, 0),
            (EnvironmentScenario::UnavailableQuery, "status:primary") => (false, "unavailable".into(), "primary".into(), "primary".into(), None, 0),
            (EnvironmentScenario::DelayedResponse, "status:primary") => (true, "stable".into(), "primary".into(), "primary".into(), None, 2),
            (EnvironmentScenario::ChangingWorld, "status:primary") => (true, "stable".into(), "primary".into(), "primary".into(), None, 0),
            (EnvironmentScenario::ChangingWorld, "status:tertiary") => (true, "changed".into(), "tertiary".into(), "tertiary".into(), None, 0),
            (EnvironmentScenario::ChangingWorld, _) => (true, "changed".into(), "secondary".into(), "secondary".into(), None, 0),
            (EnvironmentScenario::DeceptiveSource, "status:primary") => (true, "stable".into(), "deceptive".into(), "deceptive".into(), Some("adversarial_fabrication".into()), 0),
            (EnvironmentScenario::Irresolvable, "status:primary") => (true, "stable".into(), "primary".into(), "primary".into(), None, 0),
            (EnvironmentScenario::Irresolvable, "status:secondary") => (true, "changed".into(), "secondary".into(), "secondary".into(), None, 0),
            (EnvironmentScenario::Irresolvable, _) => (false, "unavailable".into(), "none".into(), "none".into(), None, 0),
            (_, "status:secondary") | (_, "status:tertiary") => (true, "stable".into(), action.query.clone(), action.query.clone(), None, 0),
            _ => (false, "unavailable".into(), "none".into(), "none".into(), None, 0),
        };
        if available {
            let observation = EnvironmentObservation { request_id: action.request_id.clone(), timestamp: self.clock, outcome, source, correlation_group: group, available, failure_mode };
            if delay > 0 { self.pending.push(PendingObservation { deliver_at: self.clock + delay, observation }); }
            else { self.pending.push(PendingObservation { deliver_at: self.clock, observation }); }
        }
        let delayed = delay > 0;
        EnvironmentReply { accepted: true, cost, observations: self.collect_due(), events, delayed }
    }

    fn collect_due(&mut self) -> Vec<EnvironmentObservation> {
        let now = self.clock;
        let mut due = Vec::new();
        self.pending.retain(|pending| if pending.deliver_at <= now { due.push(pending.observation.clone()); false } else { true });
        due
    }

    pub fn hidden_terminal(&self) -> ExpectedTerminal {
        match self.scenario { EnvironmentScenario::UnknownEntity | EnvironmentScenario::Irresolvable => ExpectedTerminal::JustifiedUnresolved, _ => ExpectedTerminal::Resolved(if matches!(self.scenario, EnvironmentScenario::ChangingWorld) { "changed" } else { "stable" }.into()) }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProtocolStep {
    pub action: MachineAction,
    pub reply: EnvironmentReply,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProtocolReceipt {
    pub episode_id: String,
    pub terminal: ExpectedTerminal,
    pub steps: Vec<ProtocolStep>,
    pub spent: u16,
    pub replay_hash: String,
}

impl ProtocolReceipt {
    pub fn replay_verified(&self) -> bool {
        self.replay_hash == receipt_hash(&self.terminal, &self.steps, self.spent)
    }
}

fn receipt_hash(terminal: &ExpectedTerminal, steps: &[ProtocolStep], spent: u16) -> String {
    let mut hasher = Sha256::new();
    hasher.update(serde_json::to_vec(&(terminal, steps, spent)).expect("protocol receipt serializes"));
    format!("{:x}", hasher.finalize())
}

pub fn run_protocol_episode(episode: &ProtocolEpisode) -> ProtocolReceipt {
    let mut environment = ExternalEnvironment::new(episode);
    let mut steps = Vec::new();
    let mut observations = Vec::new();
    let mut groups = BTreeMap::<String, String>::new();
    let candidate_queries = ["status:primary", "status:secondary", "status:tertiary", "entity:unknown"];
    for (index, query) in candidate_queries.iter().enumerate() {
        let action = MachineAction { request_id: format!("{}-request-{index}", episode.id), query: (*query).into() };
        let reply = environment.submit(&action);
        observations.extend(reply.observations.iter().filter(|observation| observation.available).cloned());
        steps.push(ProtocolStep { action, reply });
        for observation in &observations { if observation.failure_mode.is_none() { groups.entry(observation.correlation_group.clone()).or_insert_with(|| observation.outcome.clone()); } }
        let mut counts = BTreeMap::<String, usize>::new();
        for outcome in groups.values() { *counts.entry(outcome.clone()).or_insert(0) += 1; }
        if counts.values().any(|count| *count >= 2) { break; }
    }
    let terminal = if let Some(outcome) = groups.values().find(|outcome| groups.values().filter(|value| *value == *outcome).count() >= 2) { ExpectedTerminal::Resolved(outcome.clone()) } else { ExpectedTerminal::JustifiedUnresolved };
    let spent = steps.iter().map(|step| step.reply.cost).sum();
    let replay_hash = receipt_hash(&terminal, &steps, spent);
    ProtocolReceipt { episode_id: episode.id.clone(), terminal, steps, spent, replay_hash }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct IndependentEnvironmentReport {
    pub cases: usize,
    pub terminal_correct: usize,
    pub calibrated_abstentions: usize,
    pub unnecessary_actions: usize,
    pub delayed_recovery: usize,
    pub unexpected_event_recovery: usize,
    pub unsupported_actions: usize,
    pub cost_budget_violations: usize,
    pub replay_verified: usize,
}

pub fn evaluate_corpus(cases: &[ProtocolEpisode]) -> IndependentEnvironmentReport {
    let mut report = IndependentEnvironmentReport { cases: cases.len(), ..Default::default() };
    for episode in cases {
        let receipt = run_protocol_episode(episode);
        report.terminal_correct += usize::from(receipt.terminal == episode.expected);
        report.calibrated_abstentions += usize::from(matches!(episode.expected, ExpectedTerminal::JustifiedUnresolved) == matches!(receipt.terminal, ExpectedTerminal::JustifiedUnresolved));
        let mut resolved_at = None;
        let mut seen_groups = BTreeMap::<String, String>::new();
        for (index, step) in receipt.steps.iter().enumerate() {
            for observation in &step.reply.observations { if observation.available && observation.failure_mode.is_none() { seen_groups.entry(observation.correlation_group.clone()).or_insert_with(|| observation.outcome.clone()); } }
            if resolved_at.is_none() && seen_groups.values().any(|outcome| seen_groups.values().filter(|other| *other == outcome).count() >= 2) { resolved_at = Some(index); }
        }
        report.unnecessary_actions += usize::from(resolved_at.is_some_and(|index| receipt.steps.len() > index + 1));
        report.delayed_recovery += usize::from(!matches!(episode.scenario, EnvironmentScenario::DelayedResponse) || receipt.terminal == episode.expected);
        report.unexpected_event_recovery += usize::from(!matches!(episode.scenario, EnvironmentScenario::ChangingWorld) || receipt.terminal == episode.expected);
        report.unsupported_actions += usize::from(receipt.steps.iter().any(|step| !["status:primary", "status:secondary", "status:tertiary", "entity:unknown"].contains(&step.action.query.as_str())));
        report.cost_budget_violations += usize::from(receipt.spent > episode.action_budget);
        report.replay_verified += usize::from(receipt.replay_verified());
    }
    report
}

pub fn synthetic_corpus() -> Vec<ProtocolEpisode> {
    let mut cases = Vec::with_capacity(300);
    for i in 0..80 { cases.push(ProtocolEpisode { id: format!("env-clean-{i:03}"), scenario: EnvironmentScenario::Clean, expected: ExpectedTerminal::Resolved("stable".into()), action_budget: 8 }); }
    for i in 0..50 { cases.push(ProtocolEpisode { id: format!("env-delayed-{i:03}"), scenario: EnvironmentScenario::DelayedResponse, expected: ExpectedTerminal::Resolved("stable".into()), action_budget: 8 }); }
    for i in 0..40 { cases.push(ProtocolEpisode { id: format!("env-unavailable-{i:03}"), scenario: EnvironmentScenario::UnavailableQuery, expected: ExpectedTerminal::Resolved("stable".into()), action_budget: 8 }); }
    for i in 0..40 { cases.push(ProtocolEpisode { id: format!("env-changing-{i:03}"), scenario: EnvironmentScenario::ChangingWorld, expected: ExpectedTerminal::Resolved("changed".into()), action_budget: 8 }); }
    for i in 0..30 { cases.push(ProtocolEpisode { id: format!("env-deceptive-{i:03}"), scenario: EnvironmentScenario::DeceptiveSource, expected: ExpectedTerminal::Resolved("stable".into()), action_budget: 8 }); }
    for i in 0..30 { cases.push(ProtocolEpisode { id: format!("env-unknown-{i:03}"), scenario: EnvironmentScenario::UnknownEntity, expected: ExpectedTerminal::JustifiedUnresolved, action_budget: 8 }); }
    for i in 0..30 { cases.push(ProtocolEpisode { id: format!("env-irresolvable-{i:03}"), scenario: EnvironmentScenario::Irresolvable, expected: ExpectedTerminal::JustifiedUnresolved, action_budget: 8 }); }
    cases
}

pub fn synthetic_corpus_hash() -> String {
    let mut hasher = Sha256::new();
    hasher.update(serde_json::to_vec(&synthetic_corpus()).expect("environment corpus serializes"));
    format!("{:x}", hasher.finalize())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn independent_environment_preserves_protocol_boundary() {
        let cases = synthetic_corpus();
        let report = evaluate_corpus(&cases);
        eprintln!("phase10 independent env: hash={} cases={} terminal={} abstentions={} unnecessary={} delayed={} events={} unsupported={} budget={} replay={}", synthetic_corpus_hash(), report.cases, report.terminal_correct, report.calibrated_abstentions, report.unnecessary_actions, report.delayed_recovery, report.unexpected_event_recovery, report.unsupported_actions, report.cost_budget_violations, report.replay_verified);
        assert_eq!(report.cases, 300);
        assert_eq!(report.terminal_correct, 300);
        assert_eq!(report.calibrated_abstentions, 300);
        assert_eq!(report.unnecessary_actions, 0);
        assert_eq!(report.delayed_recovery, 300);
        assert_eq!(report.unexpected_event_recovery, 300);
        assert_eq!(report.unsupported_actions, 0);
        assert_eq!(report.cost_budget_violations, 0);
        assert_eq!(report.replay_verified, 300);
        let mut tampered = run_protocol_episode(&cases[0]);
        tampered.spent += 1;
        assert!(!tampered.replay_verified());
    }
}
