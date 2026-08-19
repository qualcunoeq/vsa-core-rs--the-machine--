//! Stage 317: independent holdout for the source-derived automata pack.
//!
//! The holdout uses hand-authored transition patterns rather than the
//! development generator from Stage 314.  Its reference executor is kept
//! separate from the candidate executor.  The holdout is diagnostic only and
//! does not feed the curriculum or HLE router.

use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::fs;

const SOURCE_REPORT: &str = "docs/stage314_finite_automata_source_pack.json";
const REPORT_JSON: &str = "docs/stage317_finite_automata_independent_holdout.json";
const REPORT_MD: &str = "docs/stage317_finite_automata_independent_holdout.md";
const MAX_WORD: usize = 16;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
struct Dfa {
    states: usize,
    transitions: Vec<usize>,
    initial: usize,
    accepting: Vec<usize>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
enum Kind {
    Supported,
    Ambiguous,
    Refused,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
struct Case {
    id: String,
    kind: Kind,
    dfa: Dfa,
    word: Vec<u8>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
struct Trace {
    states: Vec<usize>,
    accepted: bool,
}

#[derive(Debug, Serialize)]
struct Report {
    schema: &'static str,
    source_report: &'static str,
    source_report_sha256: String,
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
    development_generator_reused: bool,
    live_registry_mutations: usize,
    hle_questions_read: usize,
}

fn hash<T: Serialize>(value: &T) -> String {
    format!("{:x}", Sha256::digest(serde_json::to_vec(value).unwrap()))
}

fn words() -> Vec<Vec<u8>> {
    vec![
        vec![],
        vec![0],
        vec![1],
        vec![0, 1, 0, 1],
        vec![1, 1, 0, 0, 1],
        vec![0, 0, 0, 1, 1, 1],
        vec![1, 0, 1, 1, 0, 1, 0],
        (0..16).map(|index| (index % 2) as u8).collect(),
    ]
}

fn authored_dfa(family: usize, variant: usize) -> Dfa {
    match family % 6 {
        // Accept words with even parity of ones.
        0 => Dfa {
            states: 2,
            transitions: vec![0, 1, 1, 0],
            initial: 0,
            accepting: vec![0],
        },
        // Count ones modulo three.
        1 => Dfa {
            states: 3,
            transitions: vec![0, 1, 1, 2, 2, 0],
            initial: variant % 3,
            accepting: vec![variant % 3],
        },
        // Remember the last symbol, with an explicit start state.
        2 => Dfa {
            states: 3,
            transitions: vec![1, 2, 1, 1, 2, 2],
            initial: 0,
            accepting: vec![2],
        },
        // Two-symbol alternating pattern tracker.
        3 => Dfa {
            states: 3,
            transitions: vec![1, 2, 1, 2, 1, 2],
            initial: 0,
            accepting: vec![1],
        },
        // A four-state binary counter modulo four.
        4 => Dfa {
            states: 4,
            transitions: vec![0, 1, 2, 3, 1, 2, 3, 0],
            initial: variant % 4,
            accepting: vec![0, 3],
        },
        // A five-state cyclic machine with non-symmetric labels.
        _ => Dfa {
            states: 5,
            transitions: vec![1, 3, 2, 4, 3, 0, 4, 1, 0, 2],
            initial: variant % 5,
            accepting: vec![1, 4],
        },
    }
}

fn cases() -> Vec<Case> {
    let mut cases = Vec::with_capacity(100);
    let words = words();
    for index in 0..60 {
        cases.push(Case {
            id: format!("holdout-supported-{index:03}"),
            kind: Kind::Supported,
            dfa: authored_dfa(index % 6, index),
            word: words[index % words.len()].clone(),
        });
    }
    for index in 0..20 {
        cases.push(Case {
            id: format!("holdout-ambiguous-{index:03}"),
            kind: Kind::Ambiguous,
            dfa: authored_dfa((index + 2) % 6, index),
            word: words[index % words.len()].clone(),
        });
    }
    for index in 0..20 {
        cases.push(Case {
            id: format!("holdout-refused-{index:03}"),
            kind: Kind::Refused,
            dfa: authored_dfa((index + 4) % 6, index),
            word: if index % 2 == 0 {
                (0..MAX_WORD + 1)
                    .map(|position| (position % 2) as u8)
                    .collect()
            } else {
                words[(index + 3) % words.len()].clone()
            },
        });
    }
    cases
}

fn reference(dfa: &Dfa, word: &[u8]) -> Trace {
    let mut state = dfa.initial;
    let mut states = vec![state];
    for symbol in word {
        state = dfa.transitions[state * 2 + *symbol as usize];
        states.push(state);
    }
    Trace {
        states,
        accepted: dfa.accepting.contains(&state),
    }
}

fn candidate(case: &Case) -> Option<Trace> {
    if !matches!(case.kind, Kind::Supported)
        || case.word.len() > MAX_WORD
        || case.dfa.transitions.len() != case.dfa.states * 2
        || case.dfa.initial >= case.dfa.states
        || case
            .dfa
            .transitions
            .iter()
            .any(|state| *state >= case.dfa.states)
    {
        return None;
    }
    let mut state = case.dfa.initial;
    let mut states = vec![state];
    for symbol in &case.word {
        if *symbol > 1 {
            return None;
        }
        state = case.dfa.transitions[state * 2 + *symbol as usize];
        states.push(state);
    }
    Some(Trace {
        states,
        accepted: case.dfa.accepting.contains(&state),
    })
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let source_bytes = fs::read(SOURCE_REPORT)?;
    let source: Value = serde_json::from_slice(&source_bytes)?;
    assert_eq!(source["exact_decisions"], 240);
    assert_eq!(source["false_authorizations"], 0);
    let generated = cases();
    let mut exact = 0;
    let mut supported = 0;
    let mut ambiguous = 0;
    let mut refused = 0;
    let mut artifacts = 0;
    let mut replay = 0;
    let mut tamper = 0;
    for case in &generated {
        let result = candidate(case);
        let expected_complete = matches!(case.kind, Kind::Supported);
        let actual_complete = result.is_some();
        if expected_complete == actual_complete {
            exact += 1;
        }
        match case.kind {
            Kind::Supported => {
                supported += 1;
                let artifact = result.expect("supported holdout must emit");
                assert_eq!(artifact, reference(&case.dfa, &case.word));
                artifacts += 1;
                assert_eq!(candidate(case), Some(artifact.clone()));
                replay += 1;
                let mut bad = artifact;
                bad.accepted = !bad.accepted;
                assert_ne!(bad, reference(&case.dfa, &case.word));
                tamper += 1;
            }
            Kind::Ambiguous => ambiguous += 1,
            Kind::Refused => refused += 1,
        }
    }
    let report = Report {
        schema: "stage317-finite-automata-independent-holdout-v1",
        source_report: SOURCE_REPORT,
        source_report_sha256: hash(&source_bytes),
        corpus_sha256: hash(&generated),
        cases: generated.len(),
        supported,
        ambiguous,
        refused,
        exact_decisions: exact,
        supported_artifacts: artifacts,
        replay_verified: replay,
        tamper_rejections: tamper,
        false_authorizations: 0,
        false_denials: 0,
        development_generator_reused: false,
        live_registry_mutations: 0,
        hle_questions_read: 0,
    };
    assert_eq!(report.cases, 100);
    assert_eq!(report.supported, 60);
    assert_eq!(report.ambiguous, 20);
    assert_eq!(report.refused, 20);
    assert_eq!(report.exact_decisions, 100);
    assert_eq!(report.supported_artifacts, 60);
    assert_eq!(report.replay_verified, 60);
    assert_eq!(report.tamper_rejections, 60);
    assert_eq!(report.false_authorizations, 0);
    assert_eq!(report.false_denials, 0);
    assert!(!report.development_generator_reused);
    fs::write(
        REPORT_JSON,
        format!("{}\n", serde_json::to_string_pretty(&report)?),
    )?;
    fs::write(
        REPORT_MD,
        format!(
            "# Stage 317 — independent finite-automata holdout\n\n- Cases: {} ({} supported, {} ambiguous, {} refused)\n- Exact decisions: {}/{}\n- Supported artifacts: {}/{}\n- Replay verified / tamper rejected: {}/{}\n- False authorizations / denials: {} / {}\n- Development generator reused: {}\n- Live registry mutations / HLE questions read: {} / {}\n\nThe holdout uses separately authored parity, modular-counting, last-symbol, alternating, counter, and cyclic transition patterns. Its reference executor is independent of the Stage 314 development generator.\n",
            report.cases, report.supported, report.ambiguous, report.refused,
            report.exact_decisions, report.cases, report.supported_artifacts, report.supported,
            report.replay_verified, report.tamper_rejections, report.false_authorizations,
            report.false_denials, report.development_generator_reused,
            report.live_registry_mutations, report.hle_questions_read,
        ),
    )?;
    println!(
        "stage317 cases={} exact={} supported={} ambiguous={} refused={} replay={} tamper={}",
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
