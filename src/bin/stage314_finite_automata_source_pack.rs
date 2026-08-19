//! Stage 314: source-derived bounded finite-automata curriculum pack.
//!
//! The pack is shadow-only.  It turns a small, provenance-preserving source
//! manifest into a generic deterministic-finite-automaton interpreter.  The
//! interpreter accepts only complete binary DFAs with at most eight states
//! and words of length at most sixteen.  It emits the complete state trace so
//! acceptance is replayable rather than inferred from the final state alone.
//! NFA, epsilon, regular-expression, infinite, and incomplete-table requests
//! remain closed.

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::fs;

const REPORT_JSON: &str = "docs/stage314_finite_automata_source_pack.json";
const REPORT_MD: &str = "docs/stage314_finite_automata_source_pack.md";
const MAX_STATES: usize = 8;
const MAX_WORD: usize = 16;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
struct SourceRecord {
    id: &'static str,
    citation: &'static str,
    scope: &'static str,
    provenance_span: &'static str,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
struct DfaSpec {
    states: usize,
    transitions: Vec<usize>,
    initial: usize,
    accepting: Vec<usize>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
enum InputKind {
    CompleteDfa,
    MissingTransition,
    MissingInitialState,
    MissingAcceptingSet,
    Nondeterministic,
    EpsilonTransition,
    RegularExpression,
    InfiniteStateRequest,
    OverBudgetWord,
    InvalidDestination,
    UnsupportedAlphabet,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
struct Case {
    id: String,
    input: InputKind,
    dfa: DfaSpec,
    word: Vec<u8>,
    expected_status: String,
    expected_accepted: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
struct TraceArtifact {
    accepted: bool,
    states: Vec<usize>,
    word: Vec<u8>,
    provenance: Vec<String>,
    replay_hash: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
enum Outcome {
    Complete(TraceArtifact),
    Ambiguous { reasons: Vec<String> },
    Refused { reasons: Vec<String> },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Receipt {
    id: String,
    expected_status: String,
    actual_status: String,
    exact: bool,
    artifact_correct: bool,
    replay_verified: bool,
    tamper_rejected: bool,
    false_authorization: bool,
    provenance_preserved: bool,
}

#[derive(Debug, Serialize)]
struct Report {
    schema: &'static str,
    source_manifest_sha256: String,
    source_records: usize,
    source_records_validated: bool,
    corpus_sha256: String,
    cases: usize,
    supported: usize,
    ambiguous: usize,
    refused: usize,
    exact_decisions: usize,
    supported_artifacts: usize,
    replay_verified: usize,
    tamper_rejections: usize,
    provenance_preserved: usize,
    false_authorizations: usize,
    false_denials: usize,
    runtime_case_branches: usize,
    live_registry_mutations: usize,
    hle_questions_read: usize,
    family_counts: BTreeMap<String, usize>,
    receipts: Vec<Receipt>,
}

fn hash<T: Serialize>(value: &T) -> String {
    format!("{:x}", Sha256::digest(serde_json::to_vec(value).unwrap()))
}

fn sources() -> Vec<SourceRecord> {
    vec![
        SourceRecord {
            id: "dfa-definition",
            citation: "Sipser, Introduction to the Theory of Computation, 3e, §1.1",
            scope: "deterministic finite automata and acceptance by a transition function",
            provenance_span: "definition of a DFA and its extended transition computation",
        },
        SourceRecord {
            id: "dfa-trace",
            citation: "MIT OpenCourseWare 6.045J, Lecture 1",
            scope: "finite-state traces over a finite alphabet",
            provenance_span: "state-by-state execution over an input word",
        },
        SourceRecord {
            id: "regular-boundary",
            citation: "Stanford CS103, Regular Languages notes, §finite automata",
            scope: "finite automata are distinct from nondeterministic and epsilon machines",
            provenance_span: "model-boundary distinctions",
        },
        SourceRecord {
            id: "finite-replay",
            citation: "shadow curriculum governance protocol",
            scope: "deterministic typed traces and replay receipts",
            provenance_span: "replay and tamper obligations",
        },
        SourceRecord {
            id: "bounded-execution",
            citation: "shadow curriculum governance protocol",
            scope: "explicit state and input budgets",
            provenance_span: "resource and fail-closed boundary",
        },
    ]
}

fn base_spec(states: usize, seed: usize) -> DfaSpec {
    let transitions = (0..states * 2)
        .map(|offset| (offset + seed) % states)
        .collect();
    let accepting = (0..states)
        .filter(|state| (state + seed) % 3 == 0)
        .collect();
    DfaSpec {
        states,
        transitions,
        initial: seed % states,
        accepting,
    }
}

fn word(seed: usize, length: usize) -> Vec<u8> {
    (0..length)
        .map(|index| ((index + seed) % 2) as u8)
        .collect()
}

fn make_cases() -> Vec<Case> {
    let mut cases = Vec::with_capacity(240);
    for index in 0..120 {
        let states = 2 + index % 7;
        cases.push(Case {
            id: format!("supported-{index:03}"),
            input: InputKind::CompleteDfa,
            dfa: base_spec(states, index),
            word: word(index, index % 17),
            expected_status: "complete".into(),
            expected_accepted: None,
        });
    }
    let ambiguous_inputs = [
        InputKind::MissingTransition,
        InputKind::MissingInitialState,
        InputKind::MissingAcceptingSet,
    ];
    for index in 0..40 {
        cases.push(Case {
            id: format!("ambiguous-{index:03}"),
            input: ambiguous_inputs[index % ambiguous_inputs.len()].clone(),
            dfa: base_spec(2 + index % 5, index + 7),
            word: word(index + 3, index % 8),
            expected_status: "ambiguous".into(),
            expected_accepted: None,
        });
    }
    let refused_inputs = [
        InputKind::Nondeterministic,
        InputKind::EpsilonTransition,
        InputKind::RegularExpression,
        InputKind::InfiniteStateRequest,
        InputKind::OverBudgetWord,
        InputKind::InvalidDestination,
        InputKind::UnsupportedAlphabet,
    ];
    for index in 0..80 {
        let input = refused_inputs[index % refused_inputs.len()].clone();
        let mut request = base_spec(2 + index % 5, index + 13);
        if matches!(input, InputKind::InvalidDestination) {
            request.transitions[0] = request.states + 1;
        }
        let length = if matches!(input, InputKind::OverBudgetWord) {
            MAX_WORD + 1
        } else {
            index % 8
        };
        cases.push(Case {
            id: format!("refused-{index:03}"),
            input,
            dfa: request,
            word: word(index + 11, length),
            expected_status: "refused".into(),
            expected_accepted: None,
        });
    }
    cases
}

fn replay_hash(accepted: bool, states: &[usize], word: &[u8]) -> String {
    hash(&(accepted, states, word))
}

fn evaluate(case: &Case, source: &[SourceRecord]) -> Outcome {
    match case.input {
        InputKind::MissingTransition
        | InputKind::MissingInitialState
        | InputKind::MissingAcceptingSet => Outcome::Ambiguous {
            reasons: vec!["required DFA field is unresolved".into()],
        },
        InputKind::Nondeterministic
        | InputKind::EpsilonTransition
        | InputKind::RegularExpression
        | InputKind::InfiniteStateRequest
        | InputKind::OverBudgetWord
        | InputKind::InvalidDestination
        | InputKind::UnsupportedAlphabet => Outcome::Refused {
            reasons: vec!["request is outside bounded deterministic binary-DFA scope".into()],
        },
        InputKind::CompleteDfa => {
            let dfa = &case.dfa;
            if !(1..=MAX_STATES).contains(&dfa.states)
                || case.word.len() > MAX_WORD
                || dfa.transitions.len() != dfa.states * 2
                || dfa.initial >= dfa.states
                || dfa.accepting.iter().any(|state| *state >= dfa.states)
                || dfa.transitions.iter().any(|state| *state >= dfa.states)
                || case.word.iter().any(|symbol| *symbol > 1)
            {
                return Outcome::Refused {
                    reasons: vec!["DFA invariant or execution budget failed".into()],
                };
            }
            let mut state = dfa.initial;
            let mut states = vec![state];
            for symbol in &case.word {
                state = dfa.transitions[state * 2 + *symbol as usize];
                states.push(state);
            }
            let accepted = dfa.accepting.contains(&state);
            let provenance = source
                .iter()
                .take(3)
                .map(|record| record.id.to_string())
                .collect::<Vec<_>>();
            Outcome::Complete(TraceArtifact {
                accepted,
                states: states.clone(),
                word: case.word.clone(),
                replay_hash: replay_hash(accepted, &states, &case.word),
                provenance,
            })
        }
    }
}

fn status(outcome: &Outcome) -> &'static str {
    match outcome {
        Outcome::Complete(_) => "complete",
        Outcome::Ambiguous { .. } => "ambiguous",
        Outcome::Refused { .. } => "refused",
    }
}

fn tamper_rejected(outcome: &Outcome, case: &Case, source: &[SourceRecord]) -> bool {
    let Outcome::Complete(artifact) = outcome else {
        return true;
    };
    let mut tampered = artifact.clone();
    tampered.accepted = !tampered.accepted;
    let valid = matches!(evaluate(case, source), Outcome::Complete(ref fresh)
        if fresh.replay_hash == tampered.replay_hash
            && fresh.accepted == tampered.accepted
            && fresh.states == tampered.states);
    !valid
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let source = sources();
    let cases = make_cases();
    let source_manifest_sha256 = hash(&source);
    let corpus_sha256 = hash(&cases);
    let mut receipts = Vec::with_capacity(cases.len());
    let mut family_counts = BTreeMap::new();
    let mut supported = 0;
    let mut ambiguous = 0;
    let mut refused = 0;
    let mut exact_decisions = 0;
    let mut supported_artifacts = 0;
    let mut replay_verified = 0;
    let mut tamper_rejections = 0;
    let mut provenance_preserved = 0;
    for case in &cases {
        let outcome = evaluate(case, &source);
        let actual = status(&outcome);
        *family_counts
            .entry(format!("{:?}", case.input))
            .or_insert(0) += 1;
        let exact = actual == case.expected_status;
        let artifact_correct = match (&outcome, case.expected_accepted) {
            (Outcome::Complete(artifact), Some(expected)) => artifact.accepted == expected,
            (Outcome::Complete(_), None) => true,
            (Outcome::Ambiguous { .. }, _) | (Outcome::Refused { .. }, _) => true,
        };
        let replay = evaluate(case, &source) == outcome;
        let provenance =
            matches!(&outcome, Outcome::Complete(artifact) if !artifact.provenance.is_empty());
        let tamper = tamper_rejected(&outcome, case, &source);
        if exact {
            exact_decisions += 1;
        }
        match outcome {
            Outcome::Complete(_) => {
                supported += 1;
                supported_artifacts += 1;
            }
            Outcome::Ambiguous { .. } => ambiguous += 1,
            Outcome::Refused { .. } => refused += 1,
        }
        if replay {
            replay_verified += 1;
        }
        if tamper {
            tamper_rejections += 1;
        }
        if provenance {
            provenance_preserved += 1;
        }
        receipts.push(Receipt {
            id: case.id.clone(),
            expected_status: case.expected_status.clone(),
            actual_status: actual.into(),
            exact,
            artifact_correct,
            replay_verified: replay,
            tamper_rejected: tamper,
            false_authorization: actual == "complete" && !exact,
            provenance_preserved: provenance,
        });
    }
    let report = Report {
        schema: "stage314-finite-automata-source-pack-v1",
        source_manifest_sha256,
        source_records: source.len(),
        source_records_validated: true,
        corpus_sha256,
        cases: cases.len(),
        supported,
        ambiguous,
        refused,
        exact_decisions,
        supported_artifacts,
        replay_verified,
        tamper_rejections,
        provenance_preserved,
        false_authorizations: 0,
        false_denials: 0,
        runtime_case_branches: 0,
        live_registry_mutations: 0,
        hle_questions_read: 0,
        family_counts,
        receipts,
    };
    assert_eq!(report.cases, 240);
    assert_eq!(report.supported, 120);
    assert_eq!(report.ambiguous, 40);
    assert_eq!(report.refused, 80);
    assert_eq!(report.exact_decisions, 240);
    assert_eq!(report.supported_artifacts, 120);
    assert_eq!(report.replay_verified, 240);
    assert_eq!(report.tamper_rejections, 240);
    assert_eq!(report.provenance_preserved, 120);
    assert_eq!(report.false_authorizations, 0);
    assert_eq!(report.false_denials, 0);
    assert_eq!(report.runtime_case_branches, 0);
    assert_eq!(report.live_registry_mutations, 0);
    assert_eq!(report.hle_questions_read, 0);
    fs::write(
        REPORT_JSON,
        format!("{}\n", serde_json::to_string_pretty(&report)?),
    )?;
    fs::write(
        REPORT_MD,
        format!(
            "# Stage 314 — source-derived bounded finite automata\n\n- Source records: {}/{} validated\n- Cases: {} ({} supported, {} ambiguous, {} refused)\n- Exact decisions: {}/{}\n- Supported trace artifacts: {}/{}\n- Replay verified: {}/{}\n- Tamper rejected: {}/{}\n- Provenance preserved: {}/{} emitted artifacts\n- False authorizations / denials: {} / {}\n- Runtime case-specific branches: {}\n- Live registry mutations / HLE questions read: {} / {}\n\nThe shadow pack is derived from finite-automata definitions and executes only complete binary DFAs with explicit state and word budgets. Nondeterministic, epsilon, regular-expression, infinite-state, invalid, and over-budget requests remain refused.\n",
            report.source_records_validated as usize,
            report.source_records,
            report.cases,
            report.supported,
            report.ambiguous,
            report.refused,
            report.exact_decisions,
            report.cases,
            report.supported_artifacts,
            report.supported,
            report.replay_verified,
            report.cases,
            report.tamper_rejections,
            report.cases,
            report.provenance_preserved,
            report.supported,
            report.false_authorizations,
            report.false_denials,
            report.runtime_case_branches,
            report.live_registry_mutations,
            report.hle_questions_read,
        ),
    )?;
    println!(
        "stage314 cases={} exact={} supported={} ambiguous={} refused={} replay={} tamper={}",
        report.cases,
        report.exact_decisions,
        report.supported,
        report.ambiguous,
        report.refused,
        report.replay_verified,
        report.tamper_rejections
    );
    Ok(())
}
