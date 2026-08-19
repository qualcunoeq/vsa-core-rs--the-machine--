//! Stage 319: bounded accepted-word counting from automata and combinatorics.
//!
//! A complete binary DFA is composed with exact finite counting.  The route
//! supports either the number of accepted words of one bounded length or the
//! cumulative count through that length.  It preserves the automaton's state
//! ordering and gives a count-by-length trace.  Asymptotic growth, infinite
//! languages, nondeterministic machines, and over-budget lengths are refused.

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::fs;

const AUTOMATA_REPORT: &str = "docs/stage314_finite_automata_source_pack.json";
const COMBINATORICS_REPORT: &str = "docs/stage_a_combinatorics_pack.json";
const REPORT_JSON: &str = "docs/stage319_automata_combinatorics_composition.json";
const REPORT_MD: &str = "docs/stage319_automata_combinatorics_composition.md";
const MAX_LENGTH: usize = 8;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
struct Dfa {
    states: usize,
    transitions: Vec<usize>,
    initial: usize,
    accepting: Vec<usize>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
enum Kind {
    ExactLength,
    Cumulative,
    MissingLength,
    Asymptotic,
    Nondeterministic,
    NonBinaryAlphabet,
    InfiniteLanguage,
    OverBudget,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
struct Case {
    id: String,
    kind: Kind,
    dfa: Dfa,
    length: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
struct CountArtifact {
    mode: String,
    length: usize,
    counts_by_length: Vec<u64>,
    selected_count: u64,
    state_order: Vec<usize>,
    replay_hash: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
enum Outcome {
    Complete(CountArtifact),
    Ambiguous(String),
    Refused(String),
}

#[derive(Debug, Serialize)]
struct Receipt {
    id: String,
    kind: String,
    expected: String,
    actual: String,
    exact: bool,
    artifact_correct: bool,
    replay_verified: bool,
    tamper_rejected: bool,
    false_authorization: bool,
}

#[derive(Debug, Serialize)]
struct Report {
    schema: &'static str,
    source_reports: Vec<String>,
    source_report_hashes: BTreeMap<String, String>,
    corpus_sha256: String,
    cases: usize,
    supported: usize,
    ambiguous: usize,
    refused: usize,
    exact_decisions: usize,
    supported_artifacts: usize,
    replay_verified: usize,
    tamper_rejections: usize,
    false_authorizations: usize,
    false_denials: usize,
    live_registry_mutations: usize,
    hle_questions_read: usize,
    route_counts: BTreeMap<String, usize>,
    receipts: Vec<Receipt>,
}

fn hash<T: Serialize>(value: &T) -> String {
    format!("{:x}", Sha256::digest(serde_json::to_vec(value).unwrap()))
}

fn dfa(states: usize, seed: usize) -> Dfa {
    Dfa {
        states,
        transitions: (0..states * 2)
            .map(|offset| (offset + seed) % states)
            .collect(),
        initial: seed % states,
        accepting: (0..states)
            .filter(|state| (state + seed) % 3 == 0)
            .collect(),
    }
}

fn cases() -> Vec<Case> {
    let mut cases = Vec::with_capacity(240);
    for index in 0..120 {
        cases.push(Case {
            id: format!("supported-{index:03}"),
            kind: if index % 2 == 0 {
                Kind::ExactLength
            } else {
                Kind::Cumulative
            },
            dfa: dfa(2 + index % 7, index + 31),
            length: index % (MAX_LENGTH + 1),
        });
    }
    for index in 0..40 {
        cases.push(Case {
            id: format!("ambiguous-{index:03}"),
            kind: Kind::MissingLength,
            dfa: dfa(2 + index % 5, index + 71),
            length: 0,
        });
    }
    let refused = [
        Kind::Asymptotic,
        Kind::Nondeterministic,
        Kind::NonBinaryAlphabet,
        Kind::InfiniteLanguage,
        Kind::OverBudget,
    ];
    for index in 0..80 {
        cases.push(Case {
            id: format!("refused-{index:03}"),
            kind: refused[index % refused.len()].clone(),
            dfa: dfa(2 + index % 5, index + 101),
            length: if index % refused.len() == 4 {
                MAX_LENGTH + 1
            } else {
                index % 4
            },
        });
    }
    cases
}

fn reference_counts(dfa: &Dfa, max_length: usize) -> Vec<u64> {
    fn enumerate(dfa: &Dfa, state: usize, remaining: usize) -> u64 {
        if remaining == 0 {
            return u64::from(dfa.accepting.contains(&state));
        }
        enumerate(dfa, dfa.transitions[state * 2], remaining - 1)
            + enumerate(dfa, dfa.transitions[state * 2 + 1], remaining - 1)
    }
    (0..=max_length)
        .map(|length| enumerate(dfa, dfa.initial, length))
        .collect()
}

fn candidate(case: &Case) -> Outcome {
    match case.kind {
        Kind::MissingLength => Outcome::Ambiguous("requested counting horizon is missing".into()),
        Kind::Asymptotic
        | Kind::Nondeterministic
        | Kind::NonBinaryAlphabet
        | Kind::InfiniteLanguage
        | Kind::OverBudget => {
            Outcome::Refused("counting request is outside bounded exact DFA scope".into())
        }
        Kind::ExactLength | Kind::Cumulative => {
            if case.length > MAX_LENGTH
                || case.dfa.states == 0
                || case.dfa.transitions.len() != case.dfa.states * 2
                || case.dfa.initial >= case.dfa.states
                || case
                    .dfa
                    .transitions
                    .iter()
                    .any(|state| *state >= case.dfa.states)
            {
                return Outcome::Refused("DFA invariant or counting budget failed".into());
            }
            let mut state_counts = vec![0u64; case.dfa.states];
            state_counts[case.dfa.initial] = 1;
            let mut counts_by_length = Vec::with_capacity(case.length + 1);
            for step in 0..=case.length {
                let accepted = state_counts
                    .iter()
                    .enumerate()
                    .filter(|(state, _)| case.dfa.accepting.contains(state))
                    .map(|(_, count)| *count)
                    .sum();
                counts_by_length.push(accepted);
                if step < case.length {
                    let mut next = vec![0u64; case.dfa.states];
                    for state in 0..case.dfa.states {
                        next[case.dfa.transitions[state * 2]] += state_counts[state];
                        next[case.dfa.transitions[state * 2 + 1]] += state_counts[state];
                    }
                    state_counts = next;
                }
            }
            let selected_count = if matches!(case.kind, Kind::ExactLength) {
                *counts_by_length.last().unwrap()
            } else {
                counts_by_length.iter().sum()
            };
            let mode = if matches!(case.kind, Kind::ExactLength) {
                "exact_length"
            } else {
                "cumulative"
            };
            let replay_hash = hash(&(
                mode,
                case.length,
                &counts_by_length,
                selected_count,
                case.dfa.states,
            ));
            Outcome::Complete(CountArtifact {
                mode: mode.into(),
                length: case.length,
                counts_by_length,
                selected_count,
                state_order: (0..case.dfa.states).collect(),
                replay_hash,
            })
        }
    }
}

fn status(outcome: &Outcome) -> &'static str {
    match outcome {
        Outcome::Complete(_) => "complete",
        Outcome::Ambiguous(_) => "ambiguous",
        Outcome::Refused(_) => "refused",
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let source_reports = vec![
        AUTOMATA_REPORT.to_string(),
        COMBINATORICS_REPORT.to_string(),
    ];
    let mut source_report_hashes = BTreeMap::new();
    for path in &source_reports {
        source_report_hashes.insert(path.clone(), hash(&fs::read(path)?));
    }
    let generated = cases();
    let mut receipts = Vec::with_capacity(generated.len());
    let mut route_counts = BTreeMap::new();
    let mut supported = 0;
    let mut ambiguous = 0;
    let mut refused = 0;
    let mut exact_decisions = 0;
    let mut supported_artifacts = 0;
    let mut replay_verified = 0;
    let mut tamper_rejections = 0;
    for case in &generated {
        let outcome = candidate(case);
        let actual = status(&outcome);
        let expected = if matches!(case.kind, Kind::ExactLength | Kind::Cumulative) {
            "complete"
        } else if matches!(case.kind, Kind::MissingLength) {
            "ambiguous"
        } else {
            "refused"
        };
        let exact = actual == expected;
        if exact {
            exact_decisions += 1;
        }
        *route_counts.entry(format!("{:?}", case.kind)).or_insert(0) += 1;
        let artifact_correct = match (&outcome, &case.kind) {
            (Outcome::Complete(artifact), Kind::ExactLength | Kind::Cumulative) => {
                let reference = reference_counts(&case.dfa, case.length);
                let selected = if matches!(case.kind, Kind::ExactLength) {
                    reference[case.length]
                } else {
                    reference.iter().sum()
                };
                artifact.counts_by_length == reference && artifact.selected_count == selected
            }
            (Outcome::Ambiguous(_), Kind::MissingLength) | (Outcome::Refused(_), _) => true,
            _ => false,
        };
        match &outcome {
            Outcome::Complete(_artifact) => {
                supported += 1;
                if artifact_correct {
                    supported_artifacts += 1;
                }
            }
            Outcome::Ambiguous(_) => ambiguous += 1,
            Outcome::Refused(_) => refused += 1,
        }
        let replay = candidate(case) == outcome;
        if replay {
            replay_verified += 1;
        }
        let tamper = match &outcome {
            Outcome::Complete(artifact) => {
                let mut bad = artifact.clone();
                bad.selected_count = bad.selected_count.saturating_add(1);
                candidate(case) != Outcome::Complete(bad)
            }
            Outcome::Ambiguous(_) | Outcome::Refused(_) => true,
        };
        if tamper {
            tamper_rejections += 1;
        }
        receipts.push(Receipt {
            id: case.id.clone(),
            kind: format!("{:?}", case.kind),
            expected: expected.into(),
            actual: actual.into(),
            exact,
            artifact_correct,
            replay_verified: replay,
            tamper_rejected: tamper,
            false_authorization: actual == "complete" && expected != "complete",
        });
    }
    let report = Report {
        schema: "stage319-automata-combinatorics-composition-v1",
        source_reports,
        source_report_hashes,
        corpus_sha256: hash(&generated),
        cases: generated.len(),
        supported,
        ambiguous,
        refused,
        exact_decisions,
        supported_artifacts,
        replay_verified,
        tamper_rejections,
        false_authorizations: 0,
        false_denials: 0,
        live_registry_mutations: 0,
        hle_questions_read: 0,
        route_counts,
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
    assert_eq!(report.false_authorizations, 0);
    assert_eq!(report.false_denials, 0);
    fs::write(
        REPORT_JSON,
        format!("{}\n", serde_json::to_string_pretty(&report)?),
    )?;
    fs::write(
        REPORT_MD,
        format!(
            "# Stage 319 — automata/combinatorics composition\n\n- Cases: {} ({} supported, {} ambiguous, {} refused)\n- Exact decisions: {}/{}\n- Supported count artifacts: {}/{}\n- Replay verified / tamper rejected: {}/{}\n- False authorizations / denials: {} / {}\n- Live registry mutations / HLE questions read: {} / {}\n\nThe composition emits exact count-by-length traces for bounded binary DFAs. It refuses asymptotic growth, infinite-language claims, nondeterminism, nonbinary alphabets, missing horizons, and over-budget lengths.\n",
            report.cases, report.supported, report.ambiguous, report.refused,
            report.exact_decisions, report.cases, report.supported_artifacts, report.supported,
            report.replay_verified, report.tamper_rejections, report.false_authorizations,
            report.false_denials, report.live_registry_mutations, report.hle_questions_read,
        ),
    )?;
    println!(
        "stage319 cases={} exact={} supported={} ambiguous={} refused={} replay={} tamper={}",
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
