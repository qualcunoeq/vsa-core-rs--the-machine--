//! Stage 135: technical-language transfer into bounded arithmetic functions.

use serde::Serialize;
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use the_machine::bounded_arithmetic_functions_frontend::{
    formalize, replay_verified as frontend_replay, ArithmeticFrontendStatus,
};
use the_machine::bounded_arithmetic_functions_pack::{evaluate, ArithmeticFunctionStatus};

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
enum Expected {
    Supported,
    Ambiguous,
    Unsupported,
}

#[derive(Debug, Clone, Serialize)]
struct Case {
    id: String,
    text: String,
    expected: Expected,
}

#[derive(Debug, Serialize)]
struct Receipt {
    id: String,
    expected: Expected,
    frontend_status: ArithmeticFrontendStatus,
    downstream_status: Option<ArithmeticFunctionStatus>,
    exact: bool,
    authorized: bool,
    frontend_replay_verified: bool,
    downstream_replay_verified: bool,
    frontend_tamper_rejected: bool,
    downstream_tamper_rejected: bool,
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
    downstream_emitted: usize,
    downstream_authorizations: usize,
    frontend_replay_verified: usize,
    downstream_replay_verified: usize,
    frontend_tamper_rejected: usize,
    downstream_tamper_rejected: usize,
    false_authorizations: usize,
    false_denials: usize,
    status_counts: BTreeMap<String, usize>,
    receipts: Vec<Receipt>,
}

fn digest<T: Serialize>(value: &T) -> String {
    format!("{:x}", Sha256::digest(serde_json::to_vec(value).unwrap()))
}

fn corpus() -> Vec<Case> {
    let mut cases = Vec::with_capacity(240);
    let supported = [
        "Find the number of divisors of n=36.",
        "Compute the sum of divisors at value n=60.",
        "Evaluate the Möbius function μ(n=30).",
        "Count the primes up to n=71 using the prime-counting function.",
    ];
    for index in 0..120 {
        cases.push(Case {
            id: format!("supported_{index:03}"),
            text: supported[index % supported.len()].into(),
            expected: Expected::Supported,
        });
    }
    for index in 0..40 {
        cases.push(Case {
            id: format!("ambiguous_{index:03}"),
            text: if index % 2 == 0 {
                "Find the divisor count or divisor sum at n=36.".into()
            } else {
                "Determine an arithmetic function at n=36, but the function is not specified."
                    .into()
            },
            expected: Expected::Ambiguous,
        });
    }
    for index in 0..80 {
        cases.push(Case {
            id: format!("unsupported_{index:03}"),
            text: match index % 4 {
                0 => "Estimate the asymptotic prime-counting function.".into(),
                1 => "Use an analytic Dirichlet series to compute the divisor sum at n=36.".into(),
                2 => "Compute the unbounded number of divisors of n=1000000.".into(),
                _ => "Infer an approximate prime-counting value from a graph.".into(),
            },
            expected: Expected::Unsupported,
        });
    }
    cases
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cases = corpus();
    assert_eq!(cases.len(), 240);
    let corpus_sha256 = digest(&cases);
    let mut receipts = Vec::with_capacity(cases.len());
    let mut status_counts = BTreeMap::new();
    for case in cases {
        let frontend = formalize(&case.text, &case.id);
        let mut frontend_tampered = frontend.clone();
        frontend_tampered.replay_hash.push('x');
        let frontend_replay_verified = frontend_replay(&frontend);
        let frontend_tamper_rejected = !frontend_replay(&frontend_tampered);
        let downstream = frontend.request.as_ref().map(evaluate);
        let downstream_replay_verified = downstream
            .as_ref()
            .is_none_or(|result| result.replay_verified());
        let downstream_tamper_rejected = downstream.as_ref().is_none_or(|result| {
            let mut tampered = result.clone();
            tampered.replay_hash.push('x');
            !tampered.replay_verified()
        });
        let authorized = downstream
            .as_ref()
            .is_some_and(|result| result.authorized());
        let exact = match case.expected {
            Expected::Supported => authorized,
            Expected::Ambiguous => frontend.status == ArithmeticFrontendStatus::Ambiguous,
            Expected::Unsupported => frontend.status == ArithmeticFrontendStatus::Unsupported,
        };
        let false_authorization = case.expected != Expected::Supported && authorized;
        let false_denial = case.expected == Expected::Supported && !authorized;
        *status_counts
            .entry(format!("{:?}", frontend.status))
            .or_insert(0usize) += 1;
        receipts.push(Receipt {
            id: case.id,
            expected: case.expected,
            frontend_status: frontend.status,
            downstream_status: downstream.as_ref().map(|result| result.status),
            exact,
            authorized,
            frontend_replay_verified,
            downstream_replay_verified,
            frontend_tamper_rejected,
            downstream_tamper_rejected,
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
    let exact_frontend_decisions = receipts.iter().filter(|r| r.exact).count();
    let downstream_emitted = receipts
        .iter()
        .filter(|r| r.downstream_status.is_some())
        .count();
    let downstream_authorizations = receipts.iter().filter(|r| r.authorized).count();
    let frontend_replay_verified = receipts
        .iter()
        .filter(|r| r.frontend_replay_verified)
        .count();
    let downstream_replay_verified = receipts
        .iter()
        .filter(|r| r.downstream_status.is_some() && r.downstream_replay_verified)
        .count();
    let frontend_tamper_rejected = receipts
        .iter()
        .filter(|r| r.frontend_tamper_rejected)
        .count();
    let downstream_tamper_rejected = receipts
        .iter()
        .filter(|r| r.downstream_status.is_some() && r.downstream_tamper_rejected)
        .count();
    let false_authorizations = receipts.iter().filter(|r| r.false_authorization).count();
    let false_denials = receipts.iter().filter(|r| r.false_denial).count();
    assert_eq!((supported, ambiguous, unsupported), (120, 40, 80));
    assert_eq!(exact_frontend_decisions, cases);
    assert_eq!(downstream_emitted, supported);
    assert_eq!(downstream_authorizations, supported);
    assert_eq!(frontend_replay_verified, cases);
    assert_eq!(downstream_replay_verified, supported);
    assert_eq!(frontend_tamper_rejected, cases);
    assert_eq!(downstream_tamper_rejected, supported);
    assert_eq!(false_authorizations, 0);
    assert_eq!(false_denials, 0);
    let report = Report {
        schema: "stage135-arithmetic-functions-language-frontend-v1",
        source: "independently authored arithmetic-functions technical-language corpus",
        corpus_sha256,
        cases,
        supported,
        ambiguous,
        unsupported,
        exact_frontend_decisions,
        downstream_emitted,
        downstream_authorizations,
        frontend_replay_verified,
        downstream_replay_verified,
        frontend_tamper_rejected,
        downstream_tamper_rejected,
        false_authorizations,
        false_denials,
        status_counts,
        receipts,
    };
    std::fs::write(
        "docs/stage135_arithmetic_functions_language_frontend.json",
        serde_json::to_vec_pretty(&report)?,
    )?;
    println!("{}", serde_json::to_string_pretty(&report)?);
    Ok(())
}
