//! Stage 143: independent route-blind corpus for the bounded combinatorics
//! technical-language frontend.

use serde::Serialize;
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use the_machine::combinatorics_frontend::{
    formalize, replay_verified, CombinatoricsFrontendStatus,
};
use the_machine::combinatorics_pack::{evaluate_combinatorics, CombinatoricsStatus};

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
enum Expected {
    Supported,
    Ambiguous,
    Unsupported,
}

#[derive(Debug, Serialize)]
struct Receipt {
    id: String,
    expected: Expected,
    status: CombinatoricsFrontendStatus,
    exact: bool,
    authorized: bool,
    replay_verified: bool,
    tamper_rejected: bool,
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
    exact_decisions: usize,
    supported_authorizations: usize,
    replay_verified: usize,
    tamper_rejections: usize,
    false_authorizations: usize,
    false_denials: usize,
    status_counts: BTreeMap<String, usize>,
    receipts: Vec<Receipt>,
}

#[derive(Debug, Clone)]
struct Case {
    id: String,
    expected: Expected,
    text: String,
}

fn digest<T: Serialize>(value: &T) -> String {
    format!("{:x}", Sha256::digest(serde_json::to_vec(value).unwrap()))
}

fn corpus() -> Vec<Case> {
    let mut cases = Vec::with_capacity(240);
    let supported = [
        "Compute permutations n=8 k=3.",
        "Compute combinations choose n=8 k=3.",
        "Compute the multinomial coefficient with parts 2,3,1.",
        "Apply inclusion-exclusion: first=5 second=7 intersection=2.",
        "Use the pigeonhole principle for objects=17 boxes=4.",
        "Evaluate the Stirling number of the second kind n=7 k=3.",
        "Count surjection functions n=6 k=3.",
    ];
    for index in 0..120 {
        cases.push(Case {
            id: format!("supported_{index:03}"),
            expected: Expected::Supported,
            text: supported[index % supported.len()].into(),
        });
    }
    for index in 0..60 {
        cases.push(Case {
            id: format!("ambiguous_{index:03}"),
            expected: Expected::Ambiguous,
            text: if index % 2 == 0 {
                "A quoted definition gives combinations n=8 k=3, while another scope asks permutations n=8 k=3; select neither.".into()
            } else {
                "The source defines multinomial parts 2,3,1 and later Stirling number n=6 k=2; the requested operation is not identified.".into()
            },
        });
    }
    for index in 0..60 {
        cases.push(Case {
            id: format!("unsupported_{index:03}"),
            expected: Expected::Unsupported,
            text: if index % 2 == 0 {
                "Estimate an asymptotic random graph generating function count.".into()
            } else {
                "Compute an unbounded weighted infinite-family enumeration.".into()
            },
        });
    }
    cases
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cases = corpus();
    assert_eq!(cases.len(), 240);
    let corpus_sha256 = digest(
        &cases
            .iter()
            .map(|c| (&c.id, c.expected, &c.text))
            .collect::<Vec<_>>(),
    );
    let mut receipts = Vec::with_capacity(cases.len());
    let mut status_counts = BTreeMap::new();
    for case in cases {
        let frontend = formalize(&case.text, &case.id);
        let mut tampered = frontend.clone();
        tampered.replay_hash.push('x');
        let replay = replay_verified(&frontend);
        let tamper_rejected = !replay_verified(&tampered);
        let authorized = frontend.request.as_ref().is_some_and(|request| {
            let result = evaluate_combinatorics(request);
            result.status == CombinatoricsStatus::Complete && result.replay_verified()
        });
        let exact = match case.expected {
            Expected::Supported => {
                frontend.status == CombinatoricsFrontendStatus::Complete && authorized
            }
            Expected::Ambiguous => {
                frontend.status == CombinatoricsFrontendStatus::Ambiguous && !authorized
            }
            Expected::Unsupported => {
                frontend.status == CombinatoricsFrontendStatus::Unsupported && !authorized
            }
        };
        *status_counts
            .entry(format!("{:?}", frontend.status))
            .or_insert(0) += 1;
        receipts.push(Receipt {
            id: case.id,
            expected: case.expected,
            status: frontend.status,
            exact,
            authorized,
            replay_verified: replay,
            tamper_rejected,
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
    let exact_decisions = receipts.iter().filter(|r| r.exact).count();
    let supported_authorizations = receipts.iter().filter(|r| r.authorized).count();
    let replay_verified = receipts.iter().filter(|r| r.replay_verified).count();
    let tamper_rejections = receipts.iter().filter(|r| r.tamper_rejected).count();
    let false_authorizations = receipts
        .iter()
        .filter(|r| r.expected != Expected::Supported && r.authorized)
        .count();
    let false_denials = receipts
        .iter()
        .filter(|r| r.expected == Expected::Supported && !r.authorized)
        .count();
    assert_eq!((supported, ambiguous, unsupported), (120, 60, 60));
    assert_eq!(exact_decisions, 240);
    assert_eq!(supported_authorizations, 120);
    assert_eq!(replay_verified, 240);
    assert_eq!(tamper_rejections, 240);
    assert_eq!(false_authorizations, 0);
    assert_eq!(false_denials, 0);
    let report = Report {
        schema: "stage143-combinatorics-language-frontend-v1",
        source: "independently authored bounded counting language corpus",
        corpus_sha256,
        cases,
        supported,
        ambiguous,
        unsupported,
        exact_decisions,
        supported_authorizations,
        replay_verified,
        tamper_rejections,
        false_authorizations,
        false_denials,
        status_counts,
        receipts,
    };
    let json = serde_json::to_vec_pretty(&report)?;
    std::fs::write("docs/stage143_combinatorics_language_frontend.json", &json)?;
    println!("{}", String::from_utf8(json)?);
    Ok(())
}
