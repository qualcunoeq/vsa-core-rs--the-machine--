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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum StateBehaviorDefect {
    IgnoreGuards,
    FirstMatchingTransition,
    SkipInvalidIntermediate,
    ReorderEvents,
    ContinueAfterTerminal,
    OmitTraceReplay,
    AcceptUnknownStates,
    BypassSequenceBudget,
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
            || self.required_bindings != ["initial_state", "transition_table", "event_sequence"]
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
        let cases: Vec<&StateCase> = self
            .cases
            .iter()
            .filter(|case| case.split == split)
            .collect();
        sha256(&serde_json::to_vec(&cases).expect("state split serializes"))
    }
}

fn sha256(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    format!("{:x}", hasher.finalize())
}

fn parse_bindings(
    text: &str,
) -> Option<(
    String,
    Vec<StateTransition>,
    Vec<String>,
    String,
    BTreeMap<String, bool>,
)> {
    let initial = Regex::new(r"initial state\s*:\s*([a-z0-9_]+)")
        .ok()?
        .captures(text)?
        .get(1)?
        .as_str()
        .to_string();
    let expected = Regex::new(r"expected state\s*:\s*([a-z0-9_]+)")
        .ok()?
        .captures(text)?
        .get(1)?
        .as_str()
        .to_string();
    let event_caps = Regex::new(r"event sequence\s*:\s*([a-z0-9_, ]+)")
        .ok()?
        .captures(text)?;
    let events: Vec<String> = event_caps
        .get(1)?
        .as_str()
        .split(',')
        .map(str::trim)
        .filter(|item| !item.is_empty())
        .map(String::from)
        .collect();
    if events.is_empty() {
        return None;
    }
    let transition_regex = Regex::new(
        r"([a-z0-9_]+)\s*--\s*([a-z0-9_]+)(?:\s*\[([a-z0-9_]+)\])?\s*-->\s*([a-z0-9_]+)",
    )
    .ok()?;
    let transitions: Vec<StateTransition> = transition_regex
        .captures_iter(text)
        .map(|caps| StateTransition {
            from: caps[1].into(),
            event: caps[2].into(),
            guard: caps.get(3).map(|value| value.as_str().into()),
            to: caps[4].into(),
        })
        .collect();
    if transitions.is_empty() {
        return None;
    }
    let mut guards = BTreeMap::new();
    if let Some(caps) = Regex::new(r"guards\s*:\s*([a-z0-9_=, ]+)")
        .ok()?
        .captures(text)
    {
        for binding in caps[1].split(',') {
            let mut parts = binding.trim().split('=');
            let name = parts.next()?.trim();
            let value = parts.next()?.trim();
            if name.is_empty() || !matches!(value, "true" | "false") {
                return None;
            }
            guards.insert(name.into(), value == "true");
        }
    }
    Some((initial, transitions, events, expected, guards))
}

pub fn formalize(prompt: &str) -> (StateDecision, Option<StateTraceArtifact>) {
    let text = prompt.to_ascii_lowercase().replace(['\n', '\r'], " ");
    if [
        "nondeterministic",
        "random transition",
        "probabilistic",
        "stochastic",
        "probability",
        "timezone",
        "calendar",
        "unbounded",
        "continuous",
    ]
    .iter()
    .any(|marker| text.contains(marker))
    {
        return (StateDecision::Unsupported, None);
    }
    let Some((initial, transitions, events, expected, guards)) = parse_bindings(text.trim()) else {
        return (StateDecision::Ambiguous, None);
    };
    if events.len() > 8 {
        return (StateDecision::Unsupported, None);
    }
    for pair in transitions.iter().enumerate() {
        for other in transitions.iter().skip(pair.0 + 1) {
            if pair.1.from == other.from
                && pair.1.event == other.event
                && pair.1.guard == other.guard
                && pair.1.to != other.to
            {
                return (StateDecision::Unsupported, None);
            }
        }
    }
    let mut states = vec![initial.clone()];
    let mut current = initial.clone();
    for event in &events {
        let candidates: Vec<&StateTransition> = transitions
            .iter()
            .filter(|transition| transition.from == current && transition.event == *event)
            .collect();
        if candidates.is_empty() {
            return (StateDecision::Unsupported, None);
        }
        let viable: Vec<&StateTransition> = candidates
            .iter()
            .copied()
            .filter(|transition| {
                transition
                    .guard
                    .as_ref()
                    .is_none_or(|guard| guards.get(guard) == Some(&true))
            })
            .collect();
        if viable.is_empty() {
            if candidates.iter().any(|transition| {
                transition
                    .guard
                    .as_ref()
                    .is_some_and(|guard| !guards.contains_key(guard))
            }) {
                return (StateDecision::Ambiguous, None);
            }
            return (StateDecision::Unsupported, None);
        }
        if viable.len() != 1 {
            return (StateDecision::Unsupported, None);
        }
        current = viable[0].to.clone();
        states.push(current.clone());
    }
    if current != expected {
        return (StateDecision::Unsupported, None);
    }
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
    if artifact.replay_verified() {
        (StateDecision::Supported, Some(artifact))
    } else {
        (StateDecision::Unsupported, None)
    }
}

/// Normalize a small, explicit set of technical-language field aliases before
/// invoking the frozen state-transition formalizer. This is deliberately not
/// a semantic paraphrase engine: it rewrites only labels whose meaning is
/// unambiguous in a finite-state problem and leaves transitions, guards, and
/// targets to the existing fail-closed parser.
pub fn formalize_technical(prompt: &str) -> (StateDecision, Option<StateTraceArtifact>) {
    let mut text = prompt.to_ascii_lowercase().replace(['\n', '\r'], " ");
    let label_rewrites = [
        (r"\bstart(?:ing)? state\s*:\s*", "initial state: "),
        (r"\bbegin(?:ning)? state\s*:\s*", "initial state: "),
        (r"\bstart in state\s+", "initial state: "),
        (r"\bbegin in state\s+", "initial state: "),
        (r"\binput events?\s*:\s*", "event sequence: "),
        (r"\bprocess(?:ed)? events?\s*:\s*", "event sequence: "),
        (r"\bevents? to process\s*:\s*", "event sequence: "),
        (r"\bfinal state\s*:\s*", "expected state: "),
        (r"\bend in state\s+([a-z0-9_]+)", "expected state: $1"),
        (r"\bfinish in state\s+([a-z0-9_]+)", "expected state: $1"),
    ];
    for (pattern, replacement) in label_rewrites {
        text = Regex::new(pattern)
            .expect("technical state label regex")
            .replace_all(&text, replacement)
            .into_owned();
    }
    formalize(&text)
}

/// Sandbox-only semantic faults for the finite-state pressure campaign.  The
/// normal parser is unchanged; each mutation is applied to a cloned prompt or
/// replay result and can never enter the production capability graph.
pub fn formalize_with_defect(
    prompt: &str,
    defect: StateBehaviorDefect,
) -> (StateDecision, Option<StateTraceArtifact>, bool) {
    let mut text = prompt.to_ascii_lowercase();
    match defect {
        StateBehaviorDefect::IgnoreGuards => {
            let guard_re = Regex::new(r"\s*\[[a-z0-9_]+\]").unwrap();
            text = guard_re.replace_all(&text, "").into_owned();
            text = Regex::new(r"guards\s*:\s*[a-z0-9_]+=(?:true|false)")
                .unwrap()
                .replace_all(&text, "")
                .into_owned();
        }
        StateBehaviorDefect::SkipInvalidIntermediate
        | StateBehaviorDefect::ContinueAfterTerminal => {
            if let Some(caps) = Regex::new(r"event sequence\s*:\s*([a-z0-9_, ]+)")
                .unwrap()
                .captures(&text)
            {
                let events: Vec<&str> = caps[1]
                    .split(',')
                    .map(str::trim)
                    .filter(|event| !event.is_empty())
                    .collect();
                let valid: Vec<&str> = events
                    .into_iter()
                    .filter(|event| {
                        *event != "close_unknown"
                            && *event != "after_terminal"
                            && *event != "unknown"
                    })
                    .collect();
                let replacement = format!("event sequence: {}", valid.join(", "));
                text = Regex::new(r"event sequence\s*:\s*[a-z0-9_, ]+")
                    .unwrap()
                    .replace(&text, replacement)
                    .into_owned();
            }
        }
        StateBehaviorDefect::ReorderEvents => {
            if let Some(caps) = Regex::new(r"event sequence\s*:\s*([a-z0-9_, ]+)")
                .unwrap()
                .captures(&text)
            {
                let mut events: Vec<&str> = caps[1]
                    .split(',')
                    .map(str::trim)
                    .filter(|event| !event.is_empty())
                    .collect();
                events.reverse();
                let replacement = format!("event sequence: {}", events.join(", "));
                text = Regex::new(r"event sequence\s*:\s*[a-z0-9_, ]+")
                    .unwrap()
                    .replace(&text, replacement)
                    .into_owned();
            }
        }
        StateBehaviorDefect::FirstMatchingTransition => {
            if let Some(duplicate) =
                Regex::new(r";\s*([a-z0-9_]+\s*--\s*[a-z0-9_]+\s*-->\s*[a-z0-9_]+)")
                    .unwrap()
                    .captures(&text)
            {
                let suffix = duplicate
                    .get(1)
                    .map(|value| value.as_str())
                    .unwrap_or_default();
                text = text.replacen(&format!("; {suffix}"), "", 1);
            }
        }
        StateBehaviorDefect::AcceptUnknownStates => {
            if text.contains("initial state: ghost") {
                text = text.replace("initial state: ghost", "initial state: locked");
            }
        }
        StateBehaviorDefect::BypassSequenceBudget => {
            if let Some(caps) = Regex::new(r"event sequence\s*:\s*([a-z0-9_, ]+)")
                .unwrap()
                .captures(&text)
            {
                let events: Vec<&str> = caps[1]
                    .split(',')
                    .map(str::trim)
                    .filter(|event| !event.is_empty())
                    .take(8)
                    .collect();
                let replacement = format!("event sequence: {}", events.join(", "));
                text = Regex::new(r"event sequence\s*:\s*[a-z0-9_, ]+")
                    .unwrap()
                    .replace(&text, replacement)
                    .into_owned();
            }
        }
        StateBehaviorDefect::OmitTraceReplay => {}
    }
    let (decision, artifact) = formalize(&text);
    if defect == StateBehaviorDefect::OmitTraceReplay {
        return (decision, artifact, false);
    }
    let replay = artifact
        .as_ref()
        .is_some_and(StateTraceArtifact::replay_verified);
    (decision, artifact, replay)
}

pub fn pressure_corpus() -> Vec<StateCase> {
    let mut cases = Vec::with_capacity(240);
    let mut id = 0usize;
    for index in 0..40 {
        let prompt = format!("Initial state: s{index}. Transitions: s{index} --go--> t{index}; t{index} --back--> s{index}. Event sequence: go, back, go. Expected state: t{index}.");
        cases.push(StateCase {
            id: format!("state-pressure-supported-{id:03}"),
            prompt,
            expected: StateDecision::Supported,
            expected_state: Some(format!("t{index}")),
            split: if id < 120 {
                StateSplit::Development
            } else {
                StateSplit::Holdout
            },
        });
        id += 1;
    }
    for index in 0..20 {
        let prompt = format!("Initial state: idle{index}. Transitions: idle{index} --tick--> idle{index}. Event sequence: tick, tick, tick, tick. Expected state: idle{index}.");
        cases.push(StateCase {
            id: format!("state-pressure-self-{index:03}"),
            prompt,
            expected: StateDecision::Supported,
            expected_state: Some(format!("idle{index}")),
            split: StateSplit::Development,
        });
    }
    for index in 0..20 {
        let prompt = format!("Initial state: locked{index}. Transitions: locked{index} --open [key{index}]--> open{index}; open{index} --close--> locked{index}. Guards: key{index}=true. Event sequence: open, close. Expected state: locked{index}.");
        cases.push(StateCase {
            id: format!("state-pressure-guarded-{index:03}"),
            prompt,
            expected: StateDecision::Supported,
            expected_state: Some(format!("locked{index}")),
            split: StateSplit::Development,
        });
    }
    for index in 0..20 {
        let prompt = format!("Initial state: q{index}0. Transitions: q{index}0 --a--> q{index}1; q{index}1 --b--> q{index}2; q{index}2 --c--> q{index}0. Event sequence: a, b, c, a, b, c. Expected state: q{index}0.");
        cases.push(StateCase {
            id: format!("state-pressure-cycle-{index:03}"),
            prompt,
            expected: StateDecision::Supported,
            expected_state: Some(format!("q{index}0")),
            split: StateSplit::Holdout,
        });
    }
    let ambiguous = [
        "Initial state: locked. Transitions: locked --open [key_ok]--> open. Event sequence: open. Expected state: open.",
        "Initial state: idle. Transitions: idle --start--> running. Event sequence: start.",
        "The transition table is omitted but an event sequence is supplied.",
        "Initial state: q0. Transitions: q0 --a--> q1. Event sequence: a. Expected state: q1. Guards: key_ok=maybe.",
        "Initial state: q0. Transitions: q0 --a--> q1. Event sequence: a, unknown.",
    ];
    for index in 0..40 {
        cases.push(StateCase {
            id: format!("state-pressure-ambiguous-{index:03}"),
            prompt: ambiguous[index % ambiguous.len()].into(),
            expected: StateDecision::Ambiguous,
            expected_state: None,
            split: if index < 20 {
                StateSplit::Development
            } else {
                StateSplit::Holdout
            },
        });
    }
    let unsupported = [
        "Initial state: locked. Transitions: locked --open--> open. Event sequence: unknown. Expected state: locked.",
        "Initial state: q0. Transitions: q0 --a--> q1; q0 --a--> q2. Event sequence: a. Expected state: q1.",
        "This is a nondeterministic state machine with a random transition.",
        "Initial state: q0. Transitions: q0 --a--> q1. Event sequence: a. Expected state: q2.",
        "Initial state: ghost. Transitions: locked --open--> open. Event sequence: open. Expected state: open.",
        "Initial state: terminal. Transitions: terminal --close_unknown--> terminal. Event sequence: close_unknown, after_terminal. Expected state: terminal.",
        "Initial state: idle. Transitions: idle --tick--> idle. Event sequence: tick, tick, tick, tick, tick, tick, tick, tick, tick. Expected state: idle.",
        "Initial state: q0. Transitions: q0 --a--> q1; q0 --a--> q1. Event sequence: a. Expected state: q1.",
        "Initial state: locked. Transitions: locked --open [key_ok]--> open. Guards: key_ok=false. Event sequence: open. Expected state: open.",
    ];
    for index in 0..100 {
        cases.push(StateCase {
            id: format!("state-pressure-unsupported-{index:03}"),
            prompt: unsupported[index % unsupported.len()].into(),
            expected: StateDecision::Unsupported,
            expected_state: None,
            split: if index < 50 {
                StateSplit::Development
            } else {
                StateSplit::Holdout
            },
        });
    }
    cases
}

pub fn pressure_hash() -> String {
    sha256(&serde_json::to_vec(&pressure_corpus()).expect("state pressure serializes"))
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
    for (index, (prompt, state)) in supported.into_iter().enumerate() {
        cases.push(StateCase {
            id: format!("state-dev-{index:02}"),
            prompt: prompt.into(),
            expected: StateDecision::Supported,
            expected_state: Some(state.into()),
            split: StateSplit::Development,
        });
    }
    for (index, (prompt, state)) in holdout.into_iter().enumerate() {
        cases.push(StateCase {
            id: format!("state-holdout-{index:02}"),
            prompt: prompt.into(),
            expected: StateDecision::Supported,
            expected_state: Some(state.into()),
            split: StateSplit::Holdout,
        });
    }
    let ambiguous = [
        "Initial state: locked. Transitions: locked --open [key_ok]--> open. Event sequence: open. Expected state: open.",
        "Initial state: idle. Transitions: idle --start--> running. Event sequence: start.",
        "The machine has states and events, but the transition table is omitted.",
        "Initial state: q0. Transitions: q0 --a--> q1. Event sequence: a. Expected state: q1. Guards: key_ok=maybe.",
    ];
    for (index, prompt) in ambiguous.into_iter().enumerate() {
        cases.push(StateCase {
            id: format!("state-amb-{index:02}"),
            prompt: prompt.into(),
            expected: StateDecision::Ambiguous,
            expected_state: None,
            split: if index % 2 == 0 {
                StateSplit::Development
            } else {
                StateSplit::Holdout
            },
        });
    }
    let unsupported = [
        "Initial state: locked. Transitions: locked --open--> open. Event sequence: close. Expected state: locked.",
        "Initial state: q0. Transitions: q0 --a--> q1; q0 --a--> q2. Event sequence: a. Expected state: q1.",
        "This is a nondeterministic state machine with a random transition.",
        "Initial state: q0. Transitions: q0 --a--> q1. Event sequence: a. Expected state: q2.",
    ];
    for (index, prompt) in unsupported.into_iter().enumerate() {
        cases.push(StateCase {
            id: format!("state-unsup-{index:02}"),
            prompt: prompt.into(),
            expected: StateDecision::Unsupported,
            expected_state: None,
            split: if index % 2 == 0 {
                StateSplit::Development
            } else {
                StateSplit::Holdout
            },
        });
    }
    StateContract {
        contract_id: "FiniteStateTransitionV1".into(),
        input_artifact: "RawPrompt".into(),
        output_artifact: "StateTransitionTrace".into(),
        supported_forms: vec![
            "deterministic_transition_table".into(),
            "bounded_guarded_sequence".into(),
        ],
        required_bindings: vec![
            "initial_state".into(),
            "transition_table".into(),
            "event_sequence".into(),
        ],
        predicates: vec![
            "deterministic_transitions".into(),
            "guard_resolution".into(),
            "bounded_trace".into(),
        ],
        cases,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn state_contract_is_frozen_and_replay_verified() {
        let contract = contract();
        assert!(contract.validation_errors().is_empty());
        assert_eq!(contract.cases.len(), 18);
        let correct = contract
            .cases
            .iter()
            .filter(|case| formalize(&case.prompt).0 == case.expected)
            .count();
        assert_eq!(correct, contract.cases.len());
        assert!(formalize(&contract.cases[0].prompt)
            .1
            .is_some_and(|artifact| artifact.replay_verified()));
        assert!(!contract.release_hash().is_empty());
    }

    #[test]
    fn pressure_corpus_covers_state_boundaries() {
        let cases = pressure_corpus();
        assert_eq!(cases.len(), 240);
        assert_eq!(
            cases
                .iter()
                .filter(|case| case.expected == StateDecision::Supported)
                .count(),
            100
        );
        assert_eq!(
            cases
                .iter()
                .filter(|case| case.expected == StateDecision::Ambiguous)
                .count(),
            40
        );
        assert_eq!(
            cases
                .iter()
                .filter(|case| case.expected == StateDecision::Unsupported)
                .count(),
            100
        );
        let mut ids = BTreeSet::new();
        assert!(cases.iter().all(|case| ids.insert(case.id.clone())));
        let correct = cases
            .iter()
            .filter(|case| formalize(&case.prompt).0 == case.expected)
            .count();
        assert_eq!(correct, cases.len());
        assert!(!pressure_hash().is_empty());
    }

    #[test]
    fn technical_aliases_normalize_only_explicit_state_fields() {
        let prompt = "Begin in state locked. Transitions: locked --open--> open; open --close--> locked. Input events: open, close. Finish in state locked.";
        let (decision, artifact) = formalize_technical(prompt);
        assert_eq!(decision, StateDecision::Supported);
        assert!(artifact.is_some_and(|trace| trace.replay_verified()));

        let missing =
            "Begin in state locked. Transitions: locked --open--> open. Input events: open.";
        let (decision, artifact) = formalize_technical(missing);
        assert_eq!(decision, StateDecision::Ambiguous);
        assert!(artifact.is_none());
    }
}
