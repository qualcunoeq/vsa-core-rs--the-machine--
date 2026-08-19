//! Bounded persistent world-model substrate.
//!
//! This module deliberately separates observed claims, derived transition
//! results, and unresolved hypotheses.  It is a deterministic synthetic
//! proving ground for entity identity, typed state, timestamped evidence,
//! source reliability, guarded events, contradictions, and replay.

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct EntityId(pub String);

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ClaimValue {
    State(String),
    Boolean(bool),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ClaimKind {
    Observed,
    Derived,
    Hypothesis,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TransitionRule {
    pub from: String,
    pub event: String,
    pub guard: Option<String>,
    pub to: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorldModelSpec {
    pub states: BTreeSet<String>,
    pub events: BTreeSet<String>,
    pub transitions: Vec<TransitionRule>,
    pub required_variables: BTreeSet<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Observation {
    pub id: String,
    pub entity: EntityId,
    pub variable: String,
    pub value: ClaimValue,
    pub timestamp: u64,
    /// Optional inclusive start of the interval in which this claim applies.
    pub valid_from: Option<u64>,
    /// Optional inclusive end of the interval in which this claim applies.
    pub valid_until: Option<u64>,
    pub source: String,
    pub reliability: u8,
    pub confidence: u8,
}

impl Observation {
    pub fn valid_at(&self, timestamp: u64) -> bool {
        self.valid_from.is_none_or(|start| timestamp >= start)
            && self.valid_until.is_none_or(|end| timestamp <= end)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorldEvent {
    pub id: String,
    pub entity: EntityId,
    pub event: String,
    pub timestamp: u64,
    pub source: String,
    pub reliability: u8,
    pub confidence: u8,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Investigation {
    pub id: String,
    pub entities: BTreeSet<EntityId>,
    pub spec: WorldModelSpec,
    pub observations: Vec<Observation>,
    pub events: Vec<WorldEvent>,
    pub expected: InvestigationExpectation,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InvestigationExpectation {
    pub applied_events: usize,
    pub contradictions: usize,
    pub impossible_events: usize,
    pub missing_evidence: usize,
    pub final_state: Option<String>,
    pub competing_hypotheses: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BeliefClaim {
    pub value: ClaimValue,
    pub kind: ClaimKind,
    pub source: String,
    pub timestamp: u64,
    pub valid_from: Option<u64>,
    pub valid_until: Option<u64>,
    pub score: u16,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BeliefState {
    pub entity: EntityId,
    pub state: Option<String>,
    pub competing: Vec<BeliefClaim>,
    pub claims: Vec<BeliefClaim>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum WorldUpdate {
    ObservationAccepted {
        id: String,
        entity: EntityId,
    },
    Contradiction {
        entity: EntityId,
        timestamp: u64,
    },
    EventApplied {
        id: String,
        entity: EntityId,
        from: String,
        to: String,
    },
    ImpossibleEvent {
        id: String,
        entity: EntityId,
    },
    MissingEvidence {
        id: String,
        entity: EntityId,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorldReplayReceipt {
    pub investigation_id: String,
    pub updates: Vec<WorldUpdate>,
    pub beliefs: BTreeMap<EntityId, BeliefState>,
    pub contradictions: usize,
    pub impossible_events: usize,
    pub missing_evidence: usize,
    pub replay_hash: String,
}

impl WorldReplayReceipt {
    pub fn replay_verified(&self) -> bool {
        self.replay_hash
            == receipt_hash(
                &self.updates,
                &self.beliefs,
                self.contradictions,
                self.impossible_events,
                self.missing_evidence,
            )
            && self.updates.iter().all(|update| match update {
                WorldUpdate::ObservationAccepted { id, .. }
                | WorldUpdate::EventApplied { id, .. }
                | WorldUpdate::ImpossibleEvent { id, .. }
                | WorldUpdate::MissingEvidence { id, .. } => !id.is_empty(),
                WorldUpdate::Contradiction { .. } => true,
            })
    }
}

fn receipt_hash(
    updates: &[WorldUpdate],
    beliefs: &BTreeMap<EntityId, BeliefState>,
    contradictions: usize,
    impossible_events: usize,
    missing_evidence: usize,
) -> String {
    let mut hasher = Sha256::new();
    hasher.update(
        serde_json::to_vec(&(
            updates,
            beliefs,
            contradictions,
            impossible_events,
            missing_evidence,
        ))
        .expect("world receipt serializes"),
    );
    format!("{:x}", hasher.finalize())
}

fn score(reliability: u8, confidence: u8) -> u16 {
    u16::from(reliability) * u16::from(confidence)
}

fn value_state(value: &ClaimValue) -> Option<&str> {
    match value {
        ClaimValue::State(state) => Some(state),
        ClaimValue::Boolean(_) => None,
    }
}

pub fn replay_investigation(investigation: &Investigation) -> WorldReplayReceipt {
    let mut beliefs: BTreeMap<EntityId, BeliefState> = investigation
        .entities
        .iter()
        .cloned()
        .map(|entity| {
            (
                entity.clone(),
                BeliefState {
                    entity,
                    state: None,
                    competing: Vec::new(),
                    claims: Vec::new(),
                },
            )
        })
        .collect();
    let mut guard_values: BTreeMap<(EntityId, String), bool> = BTreeMap::new();
    let mut updates = Vec::new();
    let mut contradictions = 0;
    let mut impossible_events = 0;
    let mut missing_evidence = 0;
    let mut observations = investigation.observations.clone();
    observations.sort_by(|a, b| (a.timestamp, &a.id).cmp(&(b.timestamp, &b.id)));
    let mut events = investigation.events.clone();
    events.sort_by(|a, b| (a.timestamp, &a.id).cmp(&(b.timestamp, &b.id)));
    for observation in observations {
        let Some(belief) = beliefs.get_mut(&observation.entity) else {
            updates.push(WorldUpdate::ImpossibleEvent {
                id: observation.id,
                entity: observation.entity,
            });
            impossible_events += 1;
            continue;
        };
        let claim = BeliefClaim {
            value: observation.value.clone(),
            kind: ClaimKind::Observed,
            source: observation.source.clone(),
            timestamp: observation.timestamp,
            valid_from: observation.valid_from,
            valid_until: observation.valid_until,
            score: score(observation.reliability, observation.confidence),
        };
        if let ClaimValue::Boolean(value) = observation.value {
            guard_values.insert(
                (observation.entity.clone(), observation.variable.clone()),
                value,
            );
        }
        if observation.variable == "status" {
            let conflicting: Vec<&BeliefClaim> = belief
                .claims
                .iter()
                .filter(|existing| {
                    existing.timestamp == observation.timestamp
                        && value_state(&existing.value) != value_state(&claim.value)
                })
                .collect();
            if !conflicting.is_empty() {
                contradictions += 1;
                updates.push(WorldUpdate::Contradiction {
                    entity: observation.entity.clone(),
                    timestamp: observation.timestamp,
                });
                if conflicting
                    .iter()
                    .all(|existing| existing.score == claim.score)
                {
                    belief.competing.extend(conflicting.into_iter().cloned());
                    belief.competing.push(claim.clone());
                    belief.state = None;
                }
            }
            if let Some(state) = value_state(&claim.value) {
                if belief.competing.is_empty()
                    && (belief.state.is_none()
                        || claim.score
                            >= belief
                                .claims
                                .iter()
                                .filter_map(|item| value_state(&item.value).map(|_| item.score))
                                .max()
                                .unwrap_or(0))
                {
                    belief.state = Some(state.to_string());
                }
            }
        }
        belief.claims.push(claim);
        updates.push(WorldUpdate::ObservationAccepted {
            id: observation.id,
            entity: observation.entity,
        });
    }
    for event in events {
        let Some(belief) = beliefs.get_mut(&event.entity) else {
            impossible_events += 1;
            updates.push(WorldUpdate::ImpossibleEvent {
                id: event.id,
                entity: event.entity,
            });
            continue;
        };
        let Some(current) = belief.state.clone() else {
            missing_evidence += 1;
            updates.push(WorldUpdate::MissingEvidence {
                id: event.id,
                entity: event.entity,
            });
            continue;
        };
        let candidates: Vec<&TransitionRule> = investigation
            .spec
            .transitions
            .iter()
            .filter(|rule| rule.from == current && rule.event == event.event)
            .collect();
        let viable: Vec<&TransitionRule> = candidates
            .iter()
            .copied()
            .filter(|rule| {
                rule.guard.as_ref().is_none_or(|guard| {
                    guard_values.get(&(event.entity.clone(), format!("guard:{guard}")))
                        == Some(&true)
                })
            })
            .collect();
        if viable.len() != 1 {
            if candidates.iter().any(|rule| {
                rule.guard.as_ref().is_some_and(|guard| {
                    guard_values
                        .get(&(event.entity.clone(), format!("guard:{guard}")))
                        .is_none()
                })
            }) {
                missing_evidence += 1;
                updates.push(WorldUpdate::MissingEvidence {
                    id: event.id,
                    entity: event.entity,
                });
            } else {
                impossible_events += 1;
                updates.push(WorldUpdate::ImpossibleEvent {
                    id: event.id,
                    entity: event.entity,
                });
            }
            continue;
        }
        let rule = viable[0];
        let from = current.clone();
        belief.state = Some(rule.to.clone());
        belief.claims.push(BeliefClaim {
            value: ClaimValue::State(rule.to.clone()),
            kind: ClaimKind::Derived,
            source: event.source.clone(),
            timestamp: event.timestamp,
            valid_from: None,
            valid_until: None,
            score: score(event.reliability, event.confidence),
        });
        updates.push(WorldUpdate::EventApplied {
            id: event.id,
            entity: event.entity,
            from,
            to: rule.to.clone(),
        });
    }
    let replay_hash = receipt_hash(
        &updates,
        &beliefs,
        contradictions,
        impossible_events,
        missing_evidence,
    );
    WorldReplayReceipt {
        investigation_id: investigation.id.clone(),
        updates,
        beliefs,
        contradictions,
        impossible_events,
        missing_evidence,
        replay_hash,
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct InvestigationBenchmarkReport {
    pub cases: usize,
    pub exact_expectations: usize,
    pub replay_verified: usize,
    pub contradictions: usize,
    pub impossible_events: usize,
    pub missing_evidence: usize,
    pub competing_hypotheses: usize,
}

pub fn evaluate_corpus(cases: &[Investigation]) -> InvestigationBenchmarkReport {
    let mut report = InvestigationBenchmarkReport {
        cases: cases.len(),
        ..Default::default()
    };
    for case in cases {
        let receipt = replay_investigation(case);
        let final_state = receipt
            .beliefs
            .values()
            .next()
            .and_then(|belief| belief.state.clone());
        let competing = receipt
            .beliefs
            .values()
            .map(|belief| belief.competing.len())
            .sum::<usize>();
        report.exact_expectations += usize::from(
            receipt
                .updates
                .iter()
                .filter(|update| matches!(update, WorldUpdate::EventApplied { .. }))
                .count()
                == case.expected.applied_events
                && receipt.contradictions == case.expected.contradictions
                && receipt.impossible_events == case.expected.impossible_events
                && receipt.missing_evidence == case.expected.missing_evidence
                && final_state == case.expected.final_state
                && competing == case.expected.competing_hypotheses,
        );
        report.replay_verified += usize::from(receipt.replay_verified());
        report.contradictions += receipt.contradictions;
        report.impossible_events += receipt.impossible_events;
        report.missing_evidence += receipt.missing_evidence;
        report.competing_hypotheses += competing;
    }
    report
}

fn base_spec() -> WorldModelSpec {
    WorldModelSpec {
        states: ["idle", "active", "blocked"]
            .into_iter()
            .map(String::from)
            .collect(),
        events: ["start", "stop", "fail", "reset"]
            .into_iter()
            .map(String::from)
            .collect(),
        transitions: vec![
            TransitionRule {
                from: "idle".into(),
                event: "start".into(),
                guard: None,
                to: "active".into(),
            },
            TransitionRule {
                from: "active".into(),
                event: "stop".into(),
                guard: None,
                to: "idle".into(),
            },
            TransitionRule {
                from: "active".into(),
                event: "fail".into(),
                guard: None,
                to: "blocked".into(),
            },
            TransitionRule {
                from: "blocked".into(),
                event: "reset".into(),
                guard: None,
                to: "idle".into(),
            },
            TransitionRule {
                from: "idle".into(),
                event: "start".into(),
                guard: Some("authorized".into()),
                to: "active".into(),
            },
        ],
        required_variables: ["status".into(), "guard:authorized".into()]
            .into_iter()
            .collect(),
    }
}

fn observation(id: &str, value: &str, timestamp: u64, confidence: u8) -> Observation {
    Observation {
        id: id.into(),
        entity: EntityId("device-0".into()),
        variable: "status".into(),
        value: ClaimValue::State(value.into()),
        timestamp,
        valid_from: None,
        valid_until: None,
        source: if id == "obs-b" {
            "sensor-b".into()
        } else {
            "sensor-a".into()
        },
        reliability: 90,
        confidence,
    }
}
fn event(id: &str, name: &str, timestamp: u64) -> WorldEvent {
    WorldEvent {
        id: id.into(),
        entity: EntityId("device-0".into()),
        event: name.into(),
        timestamp,
        source: "controller".into(),
        reliability: 90,
        confidence: 90,
    }
}
fn investigation(
    id: String,
    observations: Vec<Observation>,
    events: Vec<WorldEvent>,
    expected: InvestigationExpectation,
) -> Investigation {
    Investigation {
        id,
        entities: [EntityId("device-0".into())].into_iter().collect(),
        spec: base_spec(),
        observations,
        events,
        expected,
    }
}

pub fn synthetic_corpus() -> Vec<Investigation> {
    let mut cases = Vec::with_capacity(240);
    for index in 0..100 {
        cases.push(investigation(
            format!("world-valid-{index:03}"),
            vec![observation("obs-0", "idle", 0, 95)],
            vec![event("evt-start", "start", 1), event("evt-stop", "stop", 2)],
            InvestigationExpectation {
                applied_events: 2,
                contradictions: 0,
                impossible_events: 0,
                missing_evidence: 0,
                final_state: Some("idle".into()),
                competing_hypotheses: 0,
            },
        ));
    }
    for index in 0..40 {
        cases.push(investigation(
            format!("world-contradiction-{index:03}"),
            vec![
                observation("obs-a", "idle", 0, 90),
                observation("obs-b", "active", 0, 90),
            ],
            vec![event("evt-stop", "stop", 1)],
            InvestigationExpectation {
                applied_events: 0,
                contradictions: 1,
                impossible_events: 0,
                missing_evidence: 1,
                final_state: None,
                competing_hypotheses: 2,
            },
        ));
    }
    for index in 0..40 {
        cases.push(investigation(
            format!("world-impossible-{index:03}"),
            vec![observation("obs-0", "idle", 0, 95)],
            vec![event("evt-stop", "stop", 1)],
            InvestigationExpectation {
                applied_events: 0,
                contradictions: 0,
                impossible_events: 1,
                missing_evidence: 0,
                final_state: Some("idle".into()),
                competing_hypotheses: 0,
            },
        ));
    }
    for index in 0..30 {
        cases.push(investigation(
            format!("world-missing-{index:03}"),
            Vec::new(),
            vec![event("evt-start", "start", 1)],
            InvestigationExpectation {
                applied_events: 0,
                contradictions: 0,
                impossible_events: 0,
                missing_evidence: 1,
                final_state: None,
                competing_hypotheses: 0,
            },
        ));
    }
    for index in 0..30 {
        cases.push(investigation(
            format!("world-hypothesis-{index:03}"),
            vec![
                observation("obs-a", "idle", 0, 90),
                observation("obs-b", "active", 0, 80),
            ],
            Vec::new(),
            InvestigationExpectation {
                applied_events: 0,
                contradictions: 1,
                impossible_events: 0,
                missing_evidence: 0,
                final_state: Some("idle".into()),
                competing_hypotheses: 0,
            },
        ));
    }
    cases
}

pub fn synthetic_corpus_hash() -> String {
    let mut hasher = Sha256::new();
    hasher.update(serde_json::to_vec(&synthetic_corpus()).expect("world corpus serializes"));
    format!("{:x}", hasher.finalize())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn synthetic_world_model_replays_and_separates_evidence_layers() {
        let cases = synthetic_corpus();
        assert_eq!(cases.len(), 240);
        assert!(!synthetic_corpus_hash().is_empty());
        let report = evaluate_corpus(&cases);
        eprintln!("phase5 world model: hash={} cases={} exact={} replay={} contradictions={} impossible={} missing={} competing={}", synthetic_corpus_hash(), report.cases, report.exact_expectations, report.replay_verified, report.contradictions, report.impossible_events, report.missing_evidence, report.competing_hypotheses);
        assert_eq!(report.exact_expectations, 240);
        assert_eq!(report.replay_verified, 240);
        assert!(report.contradictions > 0);
        assert!(report.impossible_events > 0);
        assert!(report.missing_evidence > 0);
        let receipt = replay_investigation(&cases[0]);
        assert!(receipt.replay_verified());
        assert!(receipt
            .beliefs
            .values()
            .flat_map(|belief| belief.claims.iter())
            .any(|claim| claim.kind == ClaimKind::Observed));
        assert!(receipt
            .beliefs
            .values()
            .flat_map(|belief| belief.claims.iter())
            .any(|claim| claim.kind == ClaimKind::Derived));
        let mut tampered = receipt.clone();
        tampered.updates.pop();
        assert!(!tampered.replay_verified());
        let mut counter_tampered = receipt.clone();
        counter_tampered.missing_evidence += 1;
        assert!(!counter_tampered.replay_verified());
        assert_eq!(cases[0].observations[0].source, "sensor-a");
        assert_eq!(cases[100].observations[1].source, "sensor-b");
        let interval = Observation {
            valid_until: Some(5),
            ..cases[0].observations[0].clone()
        };
        assert!(interval.valid_at(5));
        assert!(!interval.valid_at(6));
    }
}
