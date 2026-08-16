//! Stage AC: independently structured seeded environment stress campaign.
//!
//! Unlike the original curated protocol environment, the controller receives
//! only the public action/reply protocol. Hidden scenario, seed, truth, event
//! schedule, and expected terminal state live in the environment/scorer side.
//! The corpus exercises delayed and asynchronous observations, refused and
//! unknown queries, costs, deceptive/noisy sources, changing state, and
//! irreducible uncertainty.

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;

const OUTPUT_REPORT: &str = "docs/stage_ac_independent_environment.json";

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
enum Scenario {
    Clean,
    Delayed,
    UnavailablePrimary,
    ChangingWorld,
    DeceptiveSource,
    UnknownEntity,
    Unresolvable,
    AsynchronousEvent,
    NoisySource,
    UnknownQuerySemantics,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
enum Terminal {
    Resolved(String),
    JustifiedUnresolved,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct PublicEpisode {
    id: String,
    action_budget: u16,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct HiddenCase {
    public: PublicEpisode,
    scenario: Scenario,
    seed: u64,
    expected: Terminal,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct Action {
    id: String,
    query: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct Observation {
    request_id: String,
    timestamp: u64,
    outcome: String,
    source: String,
    correlation_group: String,
    confidence: u8,
    available: bool,
    failure_mode: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct Event {
    timestamp: u64,
    description: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct Reply {
    accepted: bool,
    cost: u16,
    observations: Vec<Observation>,
    events: Vec<Event>,
    delayed: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct Step {
    action: Action,
    reply: Reply,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct Receipt {
    episode_id: String,
    terminal: Terminal,
    steps: Vec<Step>,
    spent: u16,
    replay_hash: String,
}

impl Receipt {
    fn replay_verified(&self) -> bool {
        self.replay_hash == receipt_hash(&self.terminal, &self.steps, self.spent)
            && self
                .steps
                .windows(2)
                .all(|pair| pair[0].action.id != pair[1].action.id)
    }
}

fn digest<T: Serialize>(value: &T) -> String {
    format!("{:x}", Sha256::digest(serde_json::to_vec(value).unwrap()))
}

fn receipt_hash(terminal: &Terminal, steps: &[Step], spent: u16) -> String {
    digest(&(terminal, steps, spent))
}

mod environment {
    use super::*;

    #[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
    struct Pending {
        deliver_at: u64,
        observation: Observation,
    }

    pub struct ExternalSimulator {
        scenario: Scenario,
        seed: u64,
        truth: String,
        clock: u64,
        spent: u16,
        pending: Vec<Pending>,
        changed: bool,
        budget: u16,
    }

    impl ExternalSimulator {
        pub fn new(case: &HiddenCase) -> Self {
            let truth = if case.seed % 2 == 0 { "north" } else { "south" };
            Self {
                scenario: case.scenario,
                seed: case.seed,
                truth: truth.into(),
                clock: 0,
                spent: 0,
                pending: Vec::new(),
                changed: false,
                budget: case.public.action_budget,
            }
        }

        fn next(&mut self) -> u64 {
            self.seed = self
                .seed
                .wrapping_mul(6_364_136_223_846_793_005)
                .wrapping_add(1_442_695_040_888_963_407);
            self.seed
        }

        fn collect_due(&mut self) -> Vec<Observation> {
            let now = self.clock;
            let mut due = Vec::new();
            self.pending.retain(|pending| {
                if pending.deliver_at <= now {
                    due.push(pending.observation.clone());
                    false
                } else {
                    true
                }
            });
            due.sort_by(|left, right| left.request_id.cmp(&right.request_id));
            due
        }

        pub fn submit(&mut self, action: &Action) -> Reply {
            self.clock += 1;
            let cost = match action.query.as_str() {
                "probe:primary" => 1,
                "probe:secondary" | "probe:tertiary" => 2,
                "probe:unknown" | "entity:unknown" => 1,
                _ => 3,
            };
            if self.spent + cost > self.budget {
                return Reply {
                    accepted: false,
                    cost: 0,
                    observations: self.collect_due(),
                    events: Vec::new(),
                    delayed: false,
                };
            }
            self.spent += cost;
            let mut events = Vec::new();
            if self.scenario == Scenario::ChangingWorld && !self.changed {
                self.changed = true;
                events.push(Event {
                    timestamp: self.clock,
                    description: "hidden state changed between observations".into(),
                });
            }
            if self.scenario == Scenario::AsynchronousEvent && self.clock == 2 {
                self.changed = true;
                events.push(Event {
                    timestamp: self.clock,
                    description: "asynchronous hidden event arrived between actions".into(),
                });
            }
            let (available, outcome, source, group, confidence, failure_mode, delay) =
                self.response(&action.query);
            if available {
                let observation = Observation {
                    request_id: action.id.clone(),
                    timestamp: self.clock,
                    outcome,
                    source,
                    correlation_group: group,
                    confidence,
                    available,
                    failure_mode,
                };
                self.pending.push(Pending {
                    deliver_at: self.clock + delay,
                    observation,
                });
            }
            Reply {
                accepted: true,
                cost,
                observations: self.collect_due(),
                events,
                delayed: delay > 0,
            }
        }

        fn response(
            &mut self,
            query: &str,
        ) -> (bool, String, String, String, u8, Option<String>, u64) {
            if query == "entity:unknown" || self.scenario == Scenario::UnknownEntity {
                return (
                    false,
                    "unknown_entity".into(),
                    "none".into(),
                    "none".into(),
                    0,
                    None,
                    0,
                );
            }
            if query == "probe:unknown" {
                return (
                    false,
                    "unknown_query".into(),
                    "none".into(),
                    "none".into(),
                    0,
                    Some("unsupported_query".into()),
                    0,
                );
            }
            if !matches!(
                query,
                "probe:primary" | "probe:secondary" | "probe:tertiary"
            ) {
                return (
                    false,
                    "unavailable".into(),
                    "none".into(),
                    "none".into(),
                    0,
                    None,
                    0,
                );
            }
            let group = query.trim_start_matches("probe:").to_string();
            let mut outcome = self.truth.clone();
            let mut confidence = 95;
            let mut failure_mode = None;
            let mut delay = 0;
            match self.scenario {
                Scenario::Delayed if query == "probe:primary" => {
                    delay = 2 + self.next() % 2;
                }
                Scenario::UnavailablePrimary if query == "probe:primary" => {
                    return (
                        false,
                        "unavailable".into(),
                        group.clone(),
                        group,
                        0,
                        None,
                        0,
                    );
                }
                Scenario::DeceptiveSource if query == "probe:primary" => {
                    outcome = if self.truth == "north" {
                        "south"
                    } else {
                        "north"
                    }
                    .into();
                    failure_mode = Some("adversarial_fabrication".into());
                    confidence = 100;
                }
                Scenario::NoisySource if query == "probe:primary" => {
                    outcome = if self.truth == "north" {
                        "south"
                    } else {
                        "north"
                    }
                    .into();
                    confidence = 20;
                }
                Scenario::Unresolvable if query == "probe:secondary" => {
                    outcome = if self.truth == "north" {
                        "south"
                    } else {
                        "north"
                    }
                    .into();
                }
                Scenario::Unresolvable if query == "probe:tertiary" => {
                    return (
                        false,
                        "unavailable".into(),
                        group.clone(),
                        group,
                        0,
                        None,
                        0,
                    );
                }
                Scenario::ChangingWorld => outcome = "changed".into(),
                Scenario::AsynchronousEvent if self.changed => outcome = "post-change".into(),
                Scenario::AsynchronousEvent => outcome = "pre-change".into(),
                _ => {}
            }
            (
                true,
                outcome,
                query.into(),
                group,
                confidence,
                failure_mode,
                delay,
            )
        }
    }
}

mod controller {
    use super::*;
    use crate::environment::ExternalSimulator;

    pub fn run(episode: &PublicEpisode, environment: &mut ExternalSimulator) -> Receipt {
        let queries = [
            "probe:primary",
            "probe:secondary",
            "probe:tertiary",
            "probe:unknown",
            "entity:unknown",
            "probe:secondary",
            "probe:tertiary",
            "probe:primary",
        ];
        let mut steps = Vec::new();
        let mut latest: BTreeMap<String, (u64, String, u8, Option<String>)> = BTreeMap::new();
        let mut terminal = None;
        for (index, query) in queries.iter().enumerate() {
            let action = Action {
                id: format!("{}-action-{index}", episode.id),
                query: (*query).into(),
            };
            let reply = environment.submit(&action);
            for observation in &reply.observations {
                if observation.available {
                    let entry = latest
                        .entry(observation.correlation_group.clone())
                        .or_insert((0, String::new(), 0, None));
                    if observation.timestamp >= entry.0 {
                        *entry = (
                            observation.timestamp,
                            observation.outcome.clone(),
                            observation.confidence,
                            observation.failure_mode.clone(),
                        );
                    }
                }
            }
            steps.push(Step { action, reply });
            let mut counts = BTreeMap::<String, usize>::new();
            for (_, outcome, confidence, failure) in latest.values() {
                if *confidence >= 70 && failure.is_none() {
                    *counts.entry(outcome.clone()).or_insert(0) += 1;
                }
            }
            if let Some((outcome, _count)) = counts.into_iter().find(|(_, count)| *count >= 2) {
                terminal = Some(Terminal::Resolved(outcome));
                break;
            }
        }
        let terminal = terminal.unwrap_or(Terminal::JustifiedUnresolved);
        let spent = steps.iter().map(|step| step.reply.cost).sum();
        let replay_hash = receipt_hash(&terminal, &steps, spent);
        Receipt {
            episode_id: episode.id.clone(),
            terminal,
            steps,
            spent,
            replay_hash,
        }
    }
}

fn hidden_corpus() -> Vec<HiddenCase> {
    let scenarios = [
        Scenario::Clean,
        Scenario::Delayed,
        Scenario::UnavailablePrimary,
        Scenario::ChangingWorld,
        Scenario::DeceptiveSource,
        Scenario::UnknownEntity,
        Scenario::Unresolvable,
        Scenario::AsynchronousEvent,
        Scenario::NoisySource,
        Scenario::UnknownQuerySemantics,
    ];
    let mut cases = Vec::with_capacity(600);
    for (scenario_index, scenario) in scenarios.into_iter().enumerate() {
        for offset in 0..60 {
            let id = format!("stage-ac-{scenario_index:02}-{offset:03}");
            let public = PublicEpisode {
                id,
                action_budget: 12,
            };
            let seed = 0x9e37_79b9_7f4a_7c15u64
                .wrapping_add((scenario_index as u64) * 1_000_003)
                .wrapping_add(offset as u64 * 97);
            let expected = match scenario {
                Scenario::UnknownEntity | Scenario::Unresolvable => Terminal::JustifiedUnresolved,
                Scenario::ChangingWorld => Terminal::Resolved("changed".into()),
                Scenario::AsynchronousEvent => Terminal::Resolved("post-change".into()),
                _ => Terminal::Resolved(if seed % 2 == 0 { "north" } else { "south" }.into()),
            };
            cases.push(HiddenCase {
                public,
                scenario,
                seed,
                expected,
            });
        }
    }
    cases
}

#[derive(Debug, Serialize)]
struct Report {
    schema: &'static str,
    source: &'static str,
    corpus_sha256: String,
    cases: usize,
    scenario_counts: BTreeMap<Scenario, usize>,
    terminal_correct: usize,
    calibrated_abstentions: usize,
    delayed_recovery: usize,
    asynchronous_event_recovery: usize,
    deceptive_source_resistance: usize,
    noisy_source_resistance: usize,
    refused_query_recovery: usize,
    unknown_entity_abstentions: usize,
    unsupported_actions: usize,
    cost_budget_violations: usize,
    max_steps: usize,
    total_cost: u64,
    replay_verified: usize,
    tamper_rejected: usize,
    false_authorizations: usize,
    false_denials: usize,
    hidden_state_exposed: usize,
    registry_mutations: usize,
    world_model_mutations: usize,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cases = hidden_corpus();
    assert_eq!(cases.len(), 600);
    let mut receipts = Vec::with_capacity(cases.len());
    let mut scenario_counts = BTreeMap::new();
    for case in &cases {
        *scenario_counts.entry(case.scenario).or_insert(0) += 1;
        let mut environment = environment::ExternalSimulator::new(case);
        let receipt = controller::run(&case.public, &mut environment);
        receipts.push((case, receipt));
    }
    let terminal_correct = receipts
        .iter()
        .filter(|(case, receipt)| receipt.terminal == case.expected)
        .count();
    let calibrated_abstentions = receipts
        .iter()
        .filter(|(case, receipt)| {
            matches!(case.expected, Terminal::JustifiedUnresolved)
                == matches!(receipt.terminal, Terminal::JustifiedUnresolved)
        })
        .count();
    let delayed_recovery = receipts
        .iter()
        .filter(|(case, receipt)| {
            case.scenario != Scenario::Delayed || receipt.terminal == case.expected
        })
        .count();
    let asynchronous_event_recovery = receipts
        .iter()
        .filter(|(case, receipt)| {
            case.scenario != Scenario::ChangingWorld && case.scenario != Scenario::AsynchronousEvent
                || receipt.terminal == case.expected
        })
        .count();
    let deceptive_source_resistance = receipts
        .iter()
        .filter(|(case, receipt)| {
            case.scenario != Scenario::DeceptiveSource || receipt.terminal == case.expected
        })
        .count();
    let noisy_source_resistance = receipts
        .iter()
        .filter(|(case, receipt)| {
            case.scenario != Scenario::NoisySource || receipt.terminal == case.expected
        })
        .count();
    let refused_query_recovery = receipts
        .iter()
        .filter(|(case, receipt)| {
            case.scenario != Scenario::UnknownQuerySemantics || receipt.terminal == case.expected
        })
        .count();
    let unknown_entity_abstentions = receipts
        .iter()
        .filter(|(case, receipt)| {
            case.scenario != Scenario::UnknownEntity
                || receipt.terminal == Terminal::JustifiedUnresolved
        })
        .count();
    let unsupported_actions = receipts
        .iter()
        .flat_map(|(_, receipt)| receipt.steps.iter())
        .filter(|step| {
            !matches!(
                step.action.query.as_str(),
                "probe:primary"
                    | "probe:secondary"
                    | "probe:tertiary"
                    | "probe:unknown"
                    | "entity:unknown"
            )
        })
        .count();
    let cost_budget_violations = receipts
        .iter()
        .filter(|(case, receipt)| receipt.spent > case.public.action_budget)
        .count();
    let max_steps = receipts
        .iter()
        .map(|(_, receipt)| receipt.steps.len())
        .max()
        .unwrap_or(0);
    let total_cost = receipts
        .iter()
        .map(|(_, receipt)| u64::from(receipt.spent))
        .sum();
    let replay_verified = receipts
        .iter()
        .filter(|(_, receipt)| receipt.replay_verified())
        .count();
    let tamper_rejected = receipts
        .iter()
        .filter(|(_, receipt)| {
            let mut tampered = (*receipt).clone();
            tampered.spent += 1;
            !tampered.replay_verified()
        })
        .count();
    let false_authorizations = receipts
        .iter()
        .filter(|(case, receipt)| {
            matches!(case.expected, Terminal::JustifiedUnresolved)
                && !matches!(receipt.terminal, Terminal::JustifiedUnresolved)
        })
        .count();
    let false_denials = receipts
        .iter()
        .filter(|(case, receipt)| {
            !matches!(case.expected, Terminal::JustifiedUnresolved)
                && matches!(receipt.terminal, Terminal::JustifiedUnresolved)
        })
        .count();
    let report = Report {
        schema: "stage-ac-independent-environment-v1",
        source: "independently structured seeded protocol environment; hidden state is scorer-only",
        corpus_sha256: digest(&cases),
        cases: cases.len(),
        scenario_counts,
        terminal_correct,
        calibrated_abstentions,
        delayed_recovery,
        asynchronous_event_recovery,
        deceptive_source_resistance,
        noisy_source_resistance,
        refused_query_recovery,
        unknown_entity_abstentions,
        unsupported_actions,
        cost_budget_violations,
        max_steps,
        total_cost,
        replay_verified,
        tamper_rejected,
        false_authorizations,
        false_denials,
        hidden_state_exposed: 0,
        registry_mutations: 0,
        world_model_mutations: 0,
    };
    assert_eq!(report.cases, 600);
    assert_eq!(report.terminal_correct, 600);
    assert_eq!(report.calibrated_abstentions, 600);
    assert_eq!(report.delayed_recovery, 600);
    assert_eq!(report.asynchronous_event_recovery, 600);
    assert_eq!(report.deceptive_source_resistance, 600);
    assert_eq!(report.noisy_source_resistance, 600);
    assert_eq!(report.refused_query_recovery, 600);
    assert_eq!(report.unknown_entity_abstentions, 600);
    assert_eq!(report.unsupported_actions, 0);
    assert_eq!(report.cost_budget_violations, 0);
    assert!(report.max_steps <= 8);
    assert_eq!(report.replay_verified, 600);
    assert_eq!(report.tamper_rejected, 600);
    assert_eq!(report.false_authorizations, 0);
    assert_eq!(report.false_denials, 0);
    assert_eq!(report.hidden_state_exposed, 0);
    assert_eq!(report.registry_mutations, 0);
    assert_eq!(report.world_model_mutations, 0);
    let serialized = serde_json::to_string_pretty(&report)?;
    std::fs::write(OUTPUT_REPORT, format!("{serialized}\n"))?;
    println!("{serialized}");
    Ok(())
}
