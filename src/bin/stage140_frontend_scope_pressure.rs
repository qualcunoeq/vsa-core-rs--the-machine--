//! Stage 140: independent scope/embedded-notation pressure for two
//! arithmetic frontends.
//!
//! The corpus deliberately separates standalone requests from long technical
//! context containing equations, quoted notation, multiple scopes, or
//! competing operations.  A parser must not turn a syntactically visible
//! `phi`, `mu`, `n=`, or `m=` into a complete task without unique local target
//! evidence.

use serde::Serialize;
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use the_machine::bounded_arithmetic_functions_frontend::{
    formalize as formalize_arithmetic, replay_verified as arithmetic_replay,
    ArithmeticFrontendStatus,
};
use the_machine::bounded_arithmetic_functions_pack::{
    evaluate as evaluate_arithmetic, ArithmeticFunctionStatus,
};
use the_machine::number_theory_frontend::{
    formalize_number_theory_text, replay_verified as number_replay, NumberTheoryFrontendStatus,
};
use the_machine::number_theory_pack::{evaluate_number_theory, NumberTheoryStatus};

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
enum Expected {
    Arithmetic,
    NumberTheory,
    Ambiguous,
    Unsupported,
}

#[derive(Debug, Clone, Serialize)]
struct Case {
    id: String,
    expected: Expected,
    text: String,
}

#[derive(Debug, Serialize)]
struct Receipt {
    id: String,
    expected: Expected,
    arithmetic_status: String,
    number_status: String,
    complete_routes: usize,
    unique_route: Option<String>,
    downstream_authorized: bool,
    exact: bool,
    replay_verified: bool,
    tamper_rejected: bool,
    overbroad_completion: bool,
}

#[derive(Debug, Serialize)]
struct Report {
    schema: &'static str,
    source: &'static str,
    corpus_sha256: String,
    cases: usize,
    arithmetic_supported: usize,
    number_theory_supported: usize,
    ambiguous: usize,
    unsupported: usize,
    exact_decisions: usize,
    replay_verified: usize,
    tamper_rejections: usize,
    overbroad_completions: usize,
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
    for index in 0..60 {
        cases.push(Case {
            id: format!("arithmetic_direct_{index:03}"),
            expected: Expected::Arithmetic,
            text: match index % 4 {
                0 => "Find the number of divisors of n=36.".into(),
                1 => "Compute the sum of divisors at value n=60.".into(),
                2 => "Evaluate the Möbius function μ(n=30).".into(),
                _ => "Count the primes up to n=71 using the prime-counting function.".into(),
            },
        });
        cases.push(Case {
            id: format!("number_direct_{index:03}"),
            expected: Expected::NumberTheory,
            text: match index % 4 {
                0 => {
                    "Compute the greatest common divisor and Bezout coefficients for a=84 and b=30."
                        .into()
                }
                1 => "Find the least nonnegative modular inverse of a=7 modulo m=20.".into(),
                2 => "Solve the linear congruence a=6 x congruent to b=9 modulo m=15.".into(),
                _ => "Compute Euler's totient phi(n=36).".into(),
            },
        });
    }
    for index in 0..40 {
        cases.push(Case {
            id: format!("embedded_both_{index:03}"),
            expected: Expected::Ambiguous,
            text: if index % 2 == 0 {
                "A theorem quotes μ(n)=(-1)^k and φ(n)=n\u{202f}prod(1-1/p). Determine the arithmetic function at n=36.".into()
            } else {
                "The background defines phi(n=12), while the requested result is the divisor sum at n=60; preserve the competing scopes.".into()
            },
        });
        cases.push(Case {
            id: format!("embedded_number_{index:03}"),
            expected: Expected::Ambiguous,
            text: if index % 2 == 0 {
                "A quoted example asks for a modular inverse a=7 modulo m=20, while a second scope asks for a modular inverse a=11 modulo m=20; the requested scope is not identified.".into()
            } else {
                "A paper defines phi(n=12) and later phi(n=36) in separate scopes before asking for the totient; the binding scope is not unique.".into()
            },
        });
    }
    for index in 0..40 {
        cases.push(Case {
            id: format!("unsupported_scope_{index:03}"),
            expected: Expected::Unsupported,
            text: match index % 4 {
                0 => "Prove the asymptotic behavior of a Dirichlet series containing phi(n) and mu(n).".into(),
                1 => "Estimate an unbounded prime factorization while the source quotes n=1000000.".into(),
                2 => "Infer a cryptographic security theorem from modular inverse notation a=7, m=20.".into(),
                _ => "Use numerical approximation for an analytic arithmetic-function integral.".into(),
            },
        });
    }
    cases
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let repair_mode = std::env::var_os("STAGE140_REPAIRED").is_some();
    let cases = corpus();
    assert_eq!(cases.len(), 240);
    let corpus_sha256 = digest(&cases);
    let mut receipts = Vec::with_capacity(cases.len());
    let mut status_counts = BTreeMap::new();
    for case in cases {
        let arithmetic = formalize_arithmetic(&case.text, &case.id);
        let number = formalize_number_theory_text(&case.text, &case.id);
        let mut arithmetic_frontend_tampered = arithmetic.clone();
        arithmetic_frontend_tampered.replay_hash.push('x');
        let mut number_frontend_tampered = number.clone();
        number_frontend_tampered.replay_hash.push('x');
        let arithmetic_replay_ok = arithmetic_replay(&arithmetic);
        let number_replay_ok = number_replay(&number);
        let arithmetic_tamper = !arithmetic_replay(&arithmetic_frontend_tampered);
        let number_tamper = !number_replay(&number_frontend_tampered);
        let mut route_names = Vec::new();
        if arithmetic.status == ArithmeticFrontendStatus::Complete {
            route_names.push("arithmetic_functions");
        }
        if number.status == NumberTheoryFrontendStatus::Complete {
            route_names.push("number_theory");
        }
        let unique_route = (route_names.len() == 1).then(|| route_names[0].to_string());
        let downstream_authorized = match unique_route.as_deref() {
            Some("arithmetic_functions") => arithmetic.request.as_ref().is_some_and(|request| {
                let result = evaluate_arithmetic(request);
                result.status == ArithmeticFunctionStatus::Complete && result.replay_verified()
            }),
            Some("number_theory") => number.request.as_ref().is_some_and(|request| {
                let result = evaluate_number_theory(request);
                result.status == NumberTheoryStatus::Complete
                    && result.artifact.is_some()
                    && result.replay_verified()
            }),
            _ => false,
        };
        let expected_unique =
            matches!(case.expected, Expected::Arithmetic | Expected::NumberTheory);
        let exact = match case.expected {
            Expected::Arithmetic => {
                unique_route.as_deref() == Some("arithmetic_functions") && downstream_authorized
            }
            Expected::NumberTheory => {
                unique_route.as_deref() == Some("number_theory") && downstream_authorized
            }
            Expected::Ambiguous | Expected::Unsupported => unique_route.is_none(),
        };
        let overbroad_completion = !expected_unique && !route_names.is_empty();
        let replay_verified = arithmetic_replay_ok && number_replay_ok;
        let tamper_rejected = arithmetic_tamper && number_tamper;
        *status_counts
            .entry(format!("{:?}:{:?}", arithmetic.status, number.status))
            .or_insert(0) += 1;
        receipts.push(Receipt {
            id: case.id,
            expected: case.expected,
            arithmetic_status: format!("{:?}", arithmetic.status),
            number_status: format!("{:?}", number.status),
            complete_routes: route_names.len(),
            unique_route,
            downstream_authorized,
            exact,
            replay_verified,
            tamper_rejected,
            overbroad_completion,
        });
    }
    let cases = receipts.len();
    let arithmetic_supported = receipts
        .iter()
        .filter(|r| r.expected == Expected::Arithmetic)
        .count();
    let number_theory_supported = receipts
        .iter()
        .filter(|r| r.expected == Expected::NumberTheory)
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
    let replay_verified = receipts.iter().filter(|r| r.replay_verified).count();
    let tamper_rejections = receipts.iter().filter(|r| r.tamper_rejected).count();
    let overbroad_completions = receipts.iter().filter(|r| r.overbroad_completion).count();
    let false_authorizations = receipts
        .iter()
        .filter(|r| {
            !matches!(r.expected, Expected::Arithmetic | Expected::NumberTheory)
                && r.downstream_authorized
        })
        .count();
    let false_denials = receipts
        .iter()
        .filter(|r| matches!(r.expected, Expected::Arithmetic | Expected::NumberTheory) && !r.exact)
        .count();
    assert_eq!(
        (
            arithmetic_supported,
            number_theory_supported,
            ambiguous,
            unsupported
        ),
        (60, 60, 80, 40)
    );
    assert_eq!(exact_decisions, if repair_mode { 240 } else { 200 });
    assert_eq!(replay_verified, cases);
    assert_eq!(tamper_rejections, cases);
    assert_eq!(false_authorizations, if repair_mode { 0 } else { 40 });
    assert_eq!(false_denials, 0);
    if repair_mode {
        assert_eq!(overbroad_completions, 0);
    }
    let report = Report {
        schema: if repair_mode {
            "stage141-frontend-scope-repair-v1"
        } else {
            "stage140-frontend-scope-pressure-v1"
        },
        source: "independently authored embedded-notation and multi-scope corpus",
        corpus_sha256,
        cases,
        arithmetic_supported,
        number_theory_supported,
        ambiguous,
        unsupported,
        exact_decisions,
        replay_verified,
        tamper_rejections,
        overbroad_completions,
        false_authorizations,
        false_denials,
        status_counts,
        receipts,
    };
    let json = serde_json::to_vec_pretty(&report)?;
    let output = if repair_mode {
        "docs/stage141_frontend_scope_repair.json"
    } else {
        "docs/stage140_frontend_scope_pressure.json"
    };
    std::fs::write(output, &json)?;
    println!("{}", String::from_utf8(json)?);
    Ok(())
}
