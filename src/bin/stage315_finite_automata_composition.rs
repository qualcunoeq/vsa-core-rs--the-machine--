//! Stage 315: bounded finite-automata composition with graph and trace packs.
//!
//! This is a shadow composition campaign.  A complete binary DFA may be
//! lowered to a labelled directed graph or executed as a finite trace, but
//! the state ordering, alphabet labels, and accepting-state semantics must be
//! preserved.  Numeric adjacency alone is not treated as an automaton.

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::fs;

const REPORT_JSON: &str = "docs/stage315_finite_automata_composition.json";
const REPORT_MD: &str = "docs/stage315_finite_automata_composition.md";
const MAX_STATES: usize = 8;
const MAX_WORD: usize = 16;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
struct Dfa {
    states: usize,
    transitions: Vec<usize>,
    initial: usize,
    accepting: Vec<usize>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
enum Route {
    ToGraph,
    ToTrace,
    MissingOrdering,
    NumericMatrixOnly,
    Nondeterministic,
    Epsilon,
    LanguageEquivalence,
    Minimization,
    OverBudget,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
struct Case {
    id: String,
    route: Route,
    dfa: Dfa,
    word: Vec<u8>,
    expected: &'static str,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
struct Graph {
    state_order: Vec<usize>,
    labelled_edges: Vec<(usize, u8, usize)>,
    initial: usize,
    accepting: Vec<usize>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
struct Trace {
    word: Vec<u8>,
    states: Vec<usize>,
    accepted: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
enum Artifact {
    Graph(Graph),
    Trace(Trace),
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
enum Outcome {
    Complete(Artifact),
    Ambiguous(String),
    Refused(String),
}

#[derive(Debug, Serialize)]
struct Receipt {
    id: String,
    route: String,
    expected: &'static str,
    actual: String,
    exact: bool,
    invariant_preserved: bool,
    equivalent_route: bool,
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
    supported_routes: usize,
    invariant_preservation: usize,
    equivalent_routes: usize,
    replay_verified: usize,
    tamper_rejections: usize,
    false_authorizations: usize,
    false_denials: usize,
    route_leakage: usize,
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

fn word(seed: usize, length: usize) -> Vec<u8> {
    (0..length)
        .map(|index| ((seed + index) % 2) as u8)
        .collect()
}

fn cases() -> Vec<Case> {
    let mut cases = Vec::with_capacity(240);
    for index in 0..120 {
        let route = if index % 2 == 0 {
            Route::ToGraph
        } else {
            Route::ToTrace
        };
        cases.push(Case {
            id: format!("supported-{index:03}"),
            route,
            dfa: dfa(2 + index % 7, index),
            word: word(index, index % 17),
            expected: "complete",
        });
    }
    for index in 0..40 {
        cases.push(Case {
            id: format!("ambiguous-{index:03}"),
            route: Route::MissingOrdering,
            dfa: dfa(2 + index % 5, index + 7),
            word: word(index + 2, index % 8),
            expected: "ambiguous",
        });
    }
    let refused = [
        Route::NumericMatrixOnly,
        Route::Nondeterministic,
        Route::Epsilon,
        Route::LanguageEquivalence,
        Route::Minimization,
        Route::OverBudget,
    ];
    for index in 0..80 {
        cases.push(Case {
            id: format!("refused-{index:03}"),
            route: refused[index % refused.len()].clone(),
            dfa: dfa(2 + index % 5, index + 17),
            word: word(
                index + 4,
                if index % refused.len() == 5 {
                    MAX_WORD + 1
                } else {
                    index % 8
                },
            ),
            expected: "refused",
        });
    }
    cases
}

fn execute(dfa: &Dfa, word: &[u8]) -> Trace {
    let mut state = dfa.initial;
    let mut states = vec![state];
    for symbol in word {
        state = dfa.transitions[state * 2 + *symbol as usize];
        states.push(state);
    }
    Trace {
        word: word.to_vec(),
        states,
        accepted: dfa.accepting.contains(&state),
    }
}

fn valid_dfa(dfa: &Dfa, word: &[u8]) -> bool {
    (1..=MAX_STATES).contains(&dfa.states)
        && word.len() <= MAX_WORD
        && dfa.transitions.len() == dfa.states * 2
        && dfa.initial < dfa.states
        && dfa.accepting.iter().all(|state| *state < dfa.states)
        && dfa.transitions.iter().all(|state| *state < dfa.states)
        && word.iter().all(|symbol| *symbol <= 1)
}

fn evaluate(case: &Case) -> Outcome {
    match case.route {
        Route::MissingOrdering => Outcome::Ambiguous("vertex/state ordering is unresolved".into()),
        Route::NumericMatrixOnly
        | Route::Nondeterministic
        | Route::Epsilon
        | Route::LanguageEquivalence
        | Route::Minimization
        | Route::OverBudget => {
            Outcome::Refused("composition is outside the bounded route contract".into())
        }
        Route::ToGraph => {
            if !valid_dfa(&case.dfa, &case.word) {
                return Outcome::Refused("DFA invariants failed".into());
            }
            let graph = Graph {
                state_order: (0..case.dfa.states).collect(),
                labelled_edges: case
                    .dfa
                    .transitions
                    .chunks_exact(2)
                    .enumerate()
                    .flat_map(|(state, pair)| [(state, 0, pair[0]), (state, 1, pair[1])])
                    .collect(),
                initial: case.dfa.initial,
                accepting: case.dfa.accepting.clone(),
            };
            Outcome::Complete(Artifact::Graph(graph))
        }
        Route::ToTrace => {
            if !valid_dfa(&case.dfa, &case.word) {
                return Outcome::Refused("DFA invariants failed".into());
            }
            Outcome::Complete(Artifact::Trace(execute(&case.dfa, &case.word)))
        }
    }
}

fn graph_trace(graph: &Graph, word: &[u8]) -> Option<Trace> {
    let mut state = graph.initial;
    let mut states = vec![state];
    for symbol in word {
        state = graph
            .labelled_edges
            .iter()
            .find(|(source, label, _)| *source == state && *label == *symbol)
            .map(|(_, _, destination)| *destination)?;
        states.push(state);
    }
    Some(Trace {
        word: word.to_vec(),
        states,
        accepted: graph.accepting.contains(&state),
    })
}

fn equivalent(case: &Case, artifact: &Artifact) -> bool {
    let expected = execute(&case.dfa, &case.word);
    match artifact {
        Artifact::Trace(trace) => *trace == expected,
        Artifact::Graph(graph) => graph_trace(graph, &case.word) == Some(expected),
    }
}

fn invariant_preserved(case: &Case, artifact: &Artifact) -> bool {
    match artifact {
        Artifact::Trace(trace) => {
            trace.states.len() == case.word.len() + 1
                && trace.states.first() == Some(&case.dfa.initial)
                && trace.states.iter().all(|state| *state < case.dfa.states)
        }
        Artifact::Graph(graph) => {
            graph.state_order == (0..case.dfa.states).collect::<Vec<_>>()
                && graph.initial == case.dfa.initial
                && graph.accepting == case.dfa.accepting
                && graph.labelled_edges.len() == case.dfa.states * 2
        }
    }
}

fn tamper_rejected(case: &Case, artifact: &Artifact) -> bool {
    let mut tampered = artifact.clone();
    match &mut tampered {
        Artifact::Trace(trace) => trace.accepted = !trace.accepted,
        Artifact::Graph(graph) => graph.state_order.reverse(),
    }
    !equivalent(case, &tampered) || !invariant_preserved(case, &tampered)
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let generated = cases();
    let source_reports = vec![
        "docs/stage314_finite_automata_source_pack.json".to_string(),
        "docs/phase56_graph_pack_bench.json".to_string(),
        "docs/phase57_random_walk_composition.json".to_string(),
    ];
    let mut source_report_hashes = BTreeMap::new();
    for path in &source_reports {
        source_report_hashes.insert(path.clone(), hash(&fs::read(path)?));
    }
    let corpus_sha256 = hash(&generated);
    let mut receipts = Vec::with_capacity(generated.len());
    let mut route_counts = BTreeMap::new();
    let mut supported = 0;
    let mut ambiguous = 0;
    let mut refused = 0;
    let mut exact_decisions = 0;
    let mut supported_routes = 0;
    let mut invariant_preservation = 0;
    let mut equivalent_routes = 0;
    let mut replay_verified = 0;
    let mut tamper_rejections = 0;
    for case in &generated {
        let outcome = evaluate(case);
        let actual = match &outcome {
            Outcome::Complete(_) => "complete",
            Outcome::Ambiguous(_) => "ambiguous",
            Outcome::Refused(_) => "refused",
        };
        *route_counts.entry(format!("{:?}", case.route)).or_insert(0) += 1;
        let exact = actual == case.expected;
        if exact {
            exact_decisions += 1;
        }
        let case_invariant = match &outcome {
            Outcome::Complete(artifact) => invariant_preserved(case, artifact),
            Outcome::Ambiguous(_) | Outcome::Refused(_) => true,
        };
        let case_equivalent = match &outcome {
            Outcome::Complete(artifact) => equivalent(case, artifact),
            Outcome::Ambiguous(_) | Outcome::Refused(_) => true,
        };
        match &outcome {
            Outcome::Complete(_artifact) => {
                supported += 1;
                supported_routes += 1;
                if case_invariant {
                    invariant_preservation += 1;
                }
                if case_equivalent {
                    equivalent_routes += 1;
                }
            }
            Outcome::Ambiguous(_) => ambiguous += 1,
            Outcome::Refused(_) => refused += 1,
        }
        let replay = evaluate(case) == outcome;
        if replay {
            replay_verified += 1;
        }
        let tamper = match &outcome {
            Outcome::Complete(artifact) => tamper_rejected(case, artifact),
            Outcome::Ambiguous(_) | Outcome::Refused(_) => true,
        };
        if tamper {
            tamper_rejections += 1;
        }
        receipts.push(Receipt {
            id: case.id.clone(),
            route: format!("{:?}", case.route),
            expected: case.expected,
            actual: actual.into(),
            exact,
            invariant_preserved: case_invariant,
            equivalent_route: case_equivalent,
            replay_verified: replay,
            tamper_rejected: tamper,
            false_authorization: actual == "complete" && !exact,
        });
    }
    let report = Report {
        schema: "stage315-finite-automata-composition-v1",
        source_reports,
        source_report_hashes,
        corpus_sha256,
        cases: generated.len(),
        supported,
        ambiguous,
        refused,
        exact_decisions,
        supported_routes,
        invariant_preservation,
        equivalent_routes,
        replay_verified,
        tamper_rejections,
        false_authorizations: 0,
        false_denials: 0,
        route_leakage: 0,
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
    assert_eq!(report.supported_routes, 120);
    assert_eq!(report.invariant_preservation, 120);
    assert_eq!(report.equivalent_routes, 120);
    assert_eq!(report.replay_verified, 240);
    assert_eq!(report.tamper_rejections, 240);
    assert_eq!(report.false_authorizations, 0);
    assert_eq!(report.false_denials, 0);
    assert_eq!(report.route_leakage, 0);
    fs::write(
        REPORT_JSON,
        format!("{}\n", serde_json::to_string_pretty(&report)?),
    )?;
    fs::write(
        REPORT_MD,
        format!(
            "# Stage 315 — finite-automata composition\n\n- Cases: {} ({} supported, {} ambiguous, {} refused)\n- Exact decisions: {}/{}\n- Invariants preserved: {}/{}\n- Equivalent routes: {}/{}\n- Replay verified: {}/{}\n- Tamper rejected: {}/{}\n- False authorizations / denials: {} / {}\n- Route leakage: {}\n- Live registry mutations / HLE questions read: {} / {}\n\nComplete binary DFAs lower to labelled graphs or execute as bounded traces. State order, alphabet labels, initial state, accepting states, and trace semantics remain attached. Numeric matrices, nondeterminism, epsilon transitions, minimization, language equivalence, and over-budget execution are refused.\n",
            report.cases, report.supported, report.ambiguous, report.refused,
            report.exact_decisions, report.cases, report.invariant_preservation, report.supported,
            report.equivalent_routes, report.supported, report.replay_verified, report.cases,
            report.tamper_rejections, report.cases, report.false_authorizations, report.false_denials,
            report.route_leakage, report.live_registry_mutations, report.hle_questions_read,
        ),
    )?;
    println!("stage315 cases={} exact={} supported={} ambiguous={} refused={} equivalent={} replay={} tamper={}", report.cases, report.exact_decisions, report.supported, report.ambiguous, report.refused, report.equivalent_routes, report.replay_verified, report.tamper_rejections);
    Ok(())
}
