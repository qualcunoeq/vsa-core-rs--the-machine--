//! Stage 131: route-blind technical-language frontend for finite characters.

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use the_machine::dirichlet_character_frontend::{
    formalize, replay_verified as frontend_replay_verified, CharacterFrontendStatus,
};
use the_machine::dirichlet_character_pack::{evaluate, CharacterStatus};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
enum Expected {
    Supported,
    Ambiguous,
    Unsupported,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Case {
    id: String,
    text: String,
    expected: Expected,
}

#[derive(Debug, Serialize)]
struct Receipt {
    id: String,
    expected: Expected,
    frontend_status: CharacterFrontendStatus,
    frontend_exact: bool,
    downstream_status: Option<CharacterStatus>,
    authorized: bool,
    replay_verified: bool,
    tamper_rejected: bool,
    false_authorization: bool,
    false_denial: bool,
}

#[derive(Debug, Serialize)]
struct Report {
    schema: &'static str,
    source: &'static str,
    corpus_sha256: String,
    cases: usize,
    supported: usize,
    ambiguous: usize,
    unsupported: usize,
    exact_frontend_decisions: usize,
    downstream_authorizations: usize,
    replay_verified: usize,
    tamper_rejected: usize,
    false_authorizations: usize,
    false_denials: usize,
    status_counts: BTreeMap<String, usize>,
    receipts: Vec<Receipt>,
}

fn digest<T: Serialize>(value: &T) -> String {
    format!("{:x}", Sha256::digest(serde_json::to_vec(value).unwrap()))
}

fn supported_cases() -> Vec<Case> {
    let mut cases = Vec::new();
    for index in 0..30 {
        cases.push(Case {
            id: format!("validate_{index:03}"),
            text: format!(
                "Validate the finite character modulo p={} with exponent k=1.",
                [5, 7, 11, 13, 17][index % 5]
            ),
            expected: Expected::Supported,
        });
    }
    for index in 0..30 {
        cases.push(Case {
            id: format!("evaluate_{index:03}"),
            text: format!(
                "Evaluate the character value at x={} modulo p={} with exponent k=1.",
                1 + index as i64,
                [5, 7, 11, 13, 17][index % 5]
            ),
            expected: Expected::Supported,
        });
    }
    for index in 0..30 {
        cases.push(Case {
            id: format!("partial_sum_{index:03}"),
            text: format!(
                "Compute the partial sum through limit={} modulo p={} with exponent k=1.",
                4 + index as i64,
                [5, 7, 11, 13, 17][index % 5]
            ),
            expected: Expected::Supported,
        });
    }
    for index in 0..30 {
        cases.push(Case {
            id: format!("orthogonality_{index:03}"),
            text: format!(
                "Check orthogonality of the finite character modulo p={} with exponent k=1.",
                [5, 7, 11, 13, 17][index % 5]
            ),
            expected: Expected::Supported,
        });
    }
    cases
}

fn ambiguous_cases() -> Vec<Case> {
    (0..40)
        .map(|index| {
            let text = match index % 3 {
                0 => "Evaluate or compute the partial sum of a character modulo p=5 with exponent k=1.".into(),
                1 => "Check the finite character modulo p=7; the operation is not specified.".into(),
                _ => "Evaluate the character value at x=2 modulo p=11; the exponent is omitted.".into(),
            };
            Case {
                id: format!("ambiguous_{index:03}"),
                text,
                expected: Expected::Ambiguous,
            }
        })
        .collect()
}

fn unsupported_cases() -> Vec<Case> {
    (0..80)
        .map(|index| {
            let text = match index % 4 {
                0 => "Estimate the asymptotic Dirichlet series for modulus p=5 and exponent k=1.".into(),
                1 => "Compute analytic continuation of the L-function for p=7 and k=1.".into(),
                2 => "Evaluate the character for a composite modulus p=9 with exponent k=1.".into(),
                _ => "Give an approximate complex value for the continuous character at p=11 and k=1.".into(),
            };
            Case {
                id: format!("unsupported_{index:03}"),
                text,
                expected: Expected::Unsupported,
            }
        })
        .collect()
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut corpus = supported_cases();
    corpus.extend(ambiguous_cases());
    corpus.extend(unsupported_cases());
    assert_eq!(corpus.len(), 240);
    let corpus_sha256 = digest(&corpus);
    let mut receipts = Vec::with_capacity(corpus.len());
    for case in corpus {
        let frontend = formalize(&case.text, &case.id);
        let frontend_exact = match case.expected {
            Expected::Supported => frontend.status == CharacterFrontendStatus::Complete,
            Expected::Ambiguous | Expected::Unsupported => {
                frontend.status != CharacterFrontendStatus::Complete
            }
        };
        let mut frontend_tampered = frontend.clone();
        frontend_tampered.replay_hash.push('x');
        let mut replay_verified = frontend_replay_verified(&frontend);
        let mut tamper_rejected = !frontend_replay_verified(&frontend_tampered);
        let downstream = frontend.request.as_ref().map(evaluate);
        if let Some(result) = downstream.as_ref() {
            replay_verified &= result.replay_verified();
            let mut tampered = result.clone();
            tampered.replay_hash.push('x');
            tamper_rejected &= !tampered.replay_verified();
        }
        let authorized = downstream.as_ref().is_some_and(|result| {
            result.status == CharacterStatus::Complete
                && result.artifact.is_some()
                && result.replay_verified()
        });
        let false_authorization = case.expected != Expected::Supported && authorized;
        let false_denial = case.expected == Expected::Supported && !authorized;
        receipts.push(Receipt {
            id: case.id,
            expected: case.expected,
            frontend_status: frontend.status,
            frontend_exact,
            downstream_status: downstream.as_ref().map(|result| result.status),
            authorized,
            replay_verified,
            tamper_rejected,
            false_authorization,
            false_denial,
        });
    }
    let cases = receipts.len();
    let supported = receipts
        .iter()
        .filter(|r| r.expected == Expected::Supported)
        .count();
    let ambiguous = receipts
        .iter()
        .filter(|r| r.expected == Expected::Ambiguous)
        .count();
    let unsupported = receipts
        .iter()
        .filter(|r| r.expected == Expected::Unsupported)
        .count();
    let exact_frontend_decisions = receipts.iter().filter(|r| r.frontend_exact).count();
    let downstream_authorizations = receipts.iter().filter(|r| r.authorized).count();
    let replay_verified = receipts.iter().filter(|r| r.replay_verified).count();
    let tamper_rejected = receipts.iter().filter(|r| r.tamper_rejected).count();
    let false_authorizations = receipts.iter().filter(|r| r.false_authorization).count();
    let false_denials = receipts.iter().filter(|r| r.false_denial).count();
    let mut status_counts = BTreeMap::new();
    for receipt in &receipts {
        *status_counts
            .entry(format!("{:?}", receipt.frontend_status))
            .or_insert(0usize) += 1;
    }
    assert_eq!((supported, ambiguous, unsupported), (120, 40, 80));
    assert_eq!(exact_frontend_decisions, cases);
    assert_eq!(downstream_authorizations, supported);
    assert_eq!(replay_verified, cases);
    assert_eq!(tamper_rejected, cases);
    assert_eq!(false_authorizations, 0);
    assert_eq!(false_denials, 0);
    let report = Report {
        schema: "stage131-dirichlet-character-language-frontend-v1",
        source: "independently authored finite-character technical-language corpus",
        corpus_sha256,
        cases,
        supported,
        ambiguous,
        unsupported,
        exact_frontend_decisions,
        downstream_authorizations,
        replay_verified,
        tamper_rejected,
        false_authorizations,
        false_denials,
        status_counts,
        receipts,
    };
    std::fs::write(
        "docs/stage131_dirichlet_character_language_frontend.json",
        serde_json::to_vec_pretty(&report)?,
    )?;
    println!("{}", serde_json::to_string_pretty(&report)?);
    Ok(())
}
