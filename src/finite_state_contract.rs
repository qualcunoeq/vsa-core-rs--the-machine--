//! Frozen, narrow finite-state transition contract for the second unseen
//! implementation-synthesis experiment.
//!
//! The trusted substrate accepts explicit deterministic transition tables,
//! bounded event sequences, and optional Boolean guards.  It rejects missing
//! transitions, unresolved guards, nondeterminism, and stochastic or
//! calendar-like state descriptions.

use regex::Regex;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum StateDecision {
    Supported,
    Ambiguous,
    Unsupported,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StateTransition {
    pub from: String,
    pub event: String,
    pub guard: Option<String>,
    pub to: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StateTraceArtifact {
    pub initial_state: String,
    pub events: Vec<String>,
    pub states: Vec<String>,
    pub final_state: String,
    pub expected_state: String,
    pub transitions: Vec<StateTransition>,
    pub guards: BTreeMap<String, bool>,
    pub signature: String,
}

impl StateTraceArtifact {
    pub fn replay_verified(&self) -> bool {
        if self.initial_state.is_empty()
            || self.states.len() != self.events.len() + 1
            || self.states.first() != Some(&self.initial_state)
            || self.states.last() != Some(&self.final_state)
            || self.final_state != self.expected_state
            || self.signature.is_empty()
        {
            return false;
        }
        let mut current = self.initial_state.clone();
        for (index, event) in self.events.iter().enumerate() {
            let candidates: Vec<&StateTransition> = self
                .transitions
                .iter()
                .filter(|transition| transition.from == current && transition.event == *event)
                .filter(|transition| {
                    transition
                        .guard
                        .as_ref()
                        .is_none_or(|guard| self.guards.get(guard) == Some(&true))
                })
                .collect();
            if candidates.len() != 1 || self.states[index] != current {
                return false;
            }
            current = candidates[0].to.clone();
            if self.states[index + 1] != current {
                return false;
            }
        }
        current == self.final_state
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StateCase {
    pub id: String,
    pub prompt: String,
    pub expected: StateDecision,
    pub expected_state: Option<String>,
    pub split: StateSplit,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum StateSplit {
    Development,
    Holdout,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StateContract {
    pub contract_id: String,
    pub input_artifact: String,
    pub output_artifact: String,
    pub supported_forms: Vec<String>,
    pub required_bindings: Vec<String>,
    pub predicates: Vec<String>,
    pub cases: Vec<StateCase>,
}

impl StateContract {
    pub fn validation_errors(&self) -> Vec<String> {
        let mut errors = Vec::new();
        if self.contract_id != "FiniteStateTransitionV1" {
            errors.push("unexpected_contract_id".into());
        }
        if self.input_artifact != "RawPrompt" || self.output_artifact != "StateTransitionTrace" {
            errors.push("incorrect_artifact_contract".into());
        }
        if self.supported_forms.len() != 2
            || self.required_bindings
                != ["initial_state", "transition_table", "event_sequence"]
        {
            errors.push("incomplete_supported_contract".into());
        }
        let mut ids = BTreeSet::new();
        for case in &self.cases {
            if !ids.insert(case.id.clone()) {
                errors.push(format!("duplicate_case:{}", case.id));
            }
            if case.prompt.trim().is_empty() {
                errors.push(format!("empty_prompt:{}", case.id));
            }
            if case.expected == StateDecision::Supported && case.expected_state.is_none() {
                errors.push(format!("supported_missing_state:{}", case.id));
            }
            if case.expected != StateDecision::Supported && case.expected_state.is_some() {
                errors.push(format!("negative_has_state:{}", case.id));
            }
        }
        errors
    }

    pub fn release_hash(&self) -> String {
        sha256(&serde_json::to_vec(self).expect("state contract serializes"))
    }

    pub fn split_hash(&self, split: StateSplit) -> String {
        let cases: Vec<&StateCase> = self.cases.iter().filter(|case| case.split == split).collect();
        sha256(&serde_json::to_vec(&cases).expect("state split serializes"))
    }
}

fn sha256(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    format!("{:x}", hasher.finalize())
}

fn parse_bindings(text: &str) -> Option<(String, Vec<StateTransition>, Vec<String>, String, BTreeMap<String, bool>)> {
    let initial = Regex::new(r"initial state\s*:\s*([a-z0-9_]+)").ok()?.captures(text)?.get(1)?.as_str().to_string();
    let expected = Regex::new(r"expected state\s*:\s*([a-z0-9_]+)").ok()?.captures(text)?.get(1)?.as_str().to_string();
    let event_caps = Regex::new(r"event sequence\s*:\s*([a-z0-9_, ]+)").ok()?.captures(text)?;
    let events: Vec<String> = event_caps.get(1)?.as_str().split(',').map(str::trim).filter(|item| !item.is_empty()).map(String::from).collect();
    if events.is_empty() { return None; }
    let transition_regex = Regex::new(r"([a-z0-9_]+)\s*--\s*([a-z0-9_]+)(?:\s*\[([a-z0-9_]+)\])?\s*-->\s*([a-z0-9_]+)").ok()?;
    let transitions: Vec<StateTransition> = transition_regex.captures_iter(text).map(|caps| StateTransition {
        from: caps[1].into(), event: caps[2].into(), guard: caps.get(3).map(|value| value.as_str().into()), to: caps[4].into(),
    }).collect();
    if transitions.is_empty() { return None; }
    let mut guards = BTreeMap::new();
    if let Some(caps) = Regex::new(r"guards\s*:\s*([a-z0-9_=, ]+)").ok()?.captures(text) {
        for binding in caps[1].split(',') {
            let mut parts = binding.trim().split('=');
            let name = parts.next()?.trim();
            let value = parts.next()?.trim();
            if name.is_empty() || !matches!(value, "true" | "false") { return None; }
            guards.insert(name.into(), value == "true");
        }
    }
    Some((initial, transitions, events, expected, guards))
}

pub fn formalize(prompt: &str) -> (StateDecision, Option<StateTraceArtifact>) {
    let text = prompt.to_ascii_lowercase().replace(['\n', '\r'], " ");
    if ["nondeterministic", "random transition", "probability", "timezone", "calendar", "unbounded", "continuous"].iter().any(|marker| text.contains(marker)) {
        return (StateDecision::Unsupported, None);
    }
    let Some((initial, transitions, events, expected, guards)) = parse_bindings(text.trim()) else {
        return (StateDecision::Ambiguous, None);
    };
    for pair in transitions.iter().enumerate() {
        for other in transitions.iter().skip(pair.0 + 1) {
            if pair.1.from == other.from && pair.1.event == other.event && pair.1.guard == other.guard && pair.1.to != other.to {
                return (StateDecision::Unsupported, None);
            }
        }
    }
    let mut states = vec![initial.clone()];
    let mut current = initial.clone();
    for event in &events {
        let candidates: Vec<&StateTransition> = transitions.iter().filter(|transition| transition.from == current && transition.event == *event).collect();
        if candidates.is_empty() { return (StateDecision::Unsupported, None); }
        let viable: Vec<&StateTransition> = candidates.iter().copied().filter(|transition| transition.guard.as_ref().is_none_or(|guard| guards.get(guard) == Some(&true))).collect();
        if viable.is_empty() {
            if candidates.iter().any(|transition| transition.guard.as_ref().is_some_and(|guard| !guards.contains_key(guard))) {
                return (StateDecision::Ambiguous, None);
            }
            return (StateDecision::Unsupported, None);
        }
        if viable.len() != 1 { return (StateDecision::Unsupported, None); }
        current = viable[0].to.clone();
        states.push(current.clone());
    }
    if current != expected { return (StateDecision::Unsupported, None); }
    let artifact = StateTraceArtifact {
        initial_state: initial,
        events,
        states,
        final_state: current,
        expected_state: expected,
        transitions,
        guards,
        signature: "state-trace-v1".into(),
    };
    if artifact.replay_verified() { (StateDecision::Supported, Some(artifact)) } else { (StateDecision::Unsupported, None) }
}

pub fn contract() -> StateContract {
    let supported = [
        ("Initial state: locked. Transitions: locked --insert--> unlocked; unlocked --remove--> locked. Event sequence: insert, remove. Expected state: locked.", "locked"),
        ("Initial state: red. Transitions: red --timer--> green; green --timer--> yellow; yellow --timer--> red. Event sequence: timer, timer, timer. Expected state: red.", "red"),
        ("Initial state: idle. Transitions: idle --start--> running; running --stop--> idle. Event sequence: start, stop, start. Expected state: running.", "running"),
        ("Initial state: locked. Transitions: locked --open [key_ok]--> open; open --close--> locked. Guards: key_ok=true. Event sequence: open, close. Expected state: locked.", "locked"),
        ("Initial state: armed. Transitions: armed --trigger--> alarm; alarm --reset--> armed. Event sequence: trigger, reset, trigger. Expected state: alarm.", "alarm"),
        ("Initial state: a. Transitions: a --x--> b; b --y--> c; c --z--> a. Event sequence: x, y, z, x. Expected state: b.", "b"),
    ];
    let holdout = [
        ("Initial state: closed. Transitions: closed --open--> open; open --close--> closed. Event sequence: open. Expected state: open.", "open"),
        ("Initial state: cold. Transitions: cold --heat--> warm; warm --heat--> hot; hot --cool--> warm. Event sequence: heat, heat, cool. Expected state: warm.", "warm"),
        ("Initial state: idle. Transitions: idle --start [authorized]--> active; active --stop--> idle. Guards: authorized=true. Event sequence: start, stop. Expected state: idle.", "idle"),
        ("Initial state: q0. Transitions: q0 --a--> q1; q1 --b--> q2; q2 --c--> q0. Event sequence: a, b, c, a, b. Expected state: q2.", "q2"),
    ];
    let mut cases = Vec::new();
    for (index, (prompt, state)) in supported.into_iter().enumerate() { cases.push(StateCase { id: format!("state-dev-{index:02}"), prompt: prompt.into(), expected: StateDecision::Supported, expected_state: Some(state.into()), split: StateSplit::Development }); }
    for (index, (prompt, state)) in holdout.into_iter().enumerate() { cases.push(StateCase { id: format!("state-holdout-{index:02}"), prompt: prompt.into(), expected: StateDecision::Supported, expected_state: Some(state.into()), split: StateSplit::Holdout }); }
    let ambiguous = [
        "Initial state: locked. Transitions: locked --open [key_ok]--> open. Event sequence: open. Expected state: open.",
        "Initial state: idle. Transitions: idle --start--> running. Event sequence: start.",
        "The machine has states and events, but the transition table is omitted.",
        "Initial state: q0. Transitions: q0 --a--> q1. Event sequence: a. Expected state: q1. Guards: key_ok=maybe.",
    ];
    for (index, prompt) in ambiguous.into_iter().enumerate() { cases.push(StateCase { id: format!("state-amb-{index:02}"), prompt: prompt.into(), expected: StateDecision::Ambiguous, expected_state: None, split: if index % 2 == 0 { StateSplit::Development } else { StateSplit::Holdout } }); }
    let unsupported = [
        "Initial state: locked. Transitions: locked --open--> open. Event sequence: close. Expected state: locked.",
        "Initial state: q0. Transitions: q0 --a--> q1; q0 --a--> q2. Event sequence: a. Expected state: q1.",
        "This is a nondeterministic state machine with a random transition.",
        "Initial state: q0. Transitions: q0 --a--> q1. Event sequence: a. Expected state: q2.",
    ];
    for (index, prompt) in unsupported.into_iter().enumerate() { cases.push(StateCase { id: format!("state-unsup-{index:02}"), prompt: prompt.into(), expected: StateDecision::Unsupported, expected_state: None, split: if index % 2 == 0 { StateSplit::Development } else { StateSplit::Holdout } }); }
    StateContract { contract_id: "FiniteStateTransitionV1".into(), input_artifact: "RawPrompt".into(), output_artifact: "StateTransitionTrace".into(), supported_forms: vec!["deterministic_transition_table".into(), "bounded_guarded_sequence".into()], required_bindings: vec!["initial_state".into(), "transition_table".into(), "event_sequence".into()], predicates: vec!["deterministic_transitions".into(), "guard_resolution".into(), "bounded_trace".into()], cases }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn state_contract_is_frozen_and_replay_verified() {
        let contract = contract();
        assert!(contract.validation_errors().is_empty());
        assert_eq!(contract.cases.len(), 18);
        let correct = contract.cases.iter().filter(|case| formalize(&case.prompt).0 == case.expected).count();
        assert_eq!(correct, contract.cases.len());
        assert!(formalize(&contract.cases[0].prompt).1.is_some_and(|artifact| artifact.replay_verified()));
        assert!(!contract.release_hash().is_empty());
    }
}
