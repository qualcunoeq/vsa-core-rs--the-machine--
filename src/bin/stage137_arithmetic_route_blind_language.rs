//! Stage 137: route-blind language transfer with arithmetic functions.
//!
//! Every report is offered to the arithmetic-functions, elementary
//! number-theory, finite-character, and simplicial-homology frontends.  No
//! expected route is supplied to the dispatcher.  A supported report must
//! produce exactly one complete typed request; ambiguity and unsupported
//! reports remain closed.

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use the_machine::bounded_arithmetic_functions_frontend::{
    formalize as formalize_arithmetic, replay_verified as arithmetic_replay,
    ArithmeticFrontendStatus,
};
use the_machine::bounded_arithmetic_functions_pack::{
    evaluate as evaluate_arithmetic, ArithmeticFunctionStatus,
};
use the_machine::dirichlet_character_frontend::{
    formalize as formalize_character, replay_verified as character_replay, CharacterFrontendStatus,
};
use the_machine::dirichlet_character_pack::{evaluate as evaluate_character, CharacterStatus};
use the_machine::number_theory_frontend::{
    formalize_number_theory_text, replay_verified as number_replay, NumberTheoryFrontendStatus,
};
use the_machine::number_theory_pack::{evaluate_number_theory, NumberTheoryStatus};
use the_machine::simplicial_homology_frontend::{
    formalize as formalize_homology, FrontendStatus as HomologyFrontendStatus,
};
use the_machine::simplicial_homology_pack::{evaluate as evaluate_homology, HomologyStatus};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
enum Family {
    Arithmetic,
    NumberTheory,
    Character,
    Homology,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
enum Expected {
    Supported,
    Ambiguous,
    Unsupported,
}

#[derive(Debug, Clone, Serialize)]
struct Case {
    id: String,
    family: Family,
    expected: Expected,
    text: String,
}

#[derive(Debug, Clone, Copy)]
struct RouteObservation {
    complete: bool,
    authorized: bool,
    replay_verified: bool,
    tamper_rejected: bool,
}

#[derive(Debug, Serialize)]
struct Receipt {
    id: String,
    family: Family,
    expected: Expected,
    selected_route: Option<Family>,
    route_exact: bool,
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
    frontend_invocations: usize,
    exact_route_decisions: usize,
    supported_authorizations: usize,
    downstream_emitted: usize,
    frontend_replay_verified: usize,
    downstream_replay_verified: usize,
    frontend_tamper_rejected: usize,
    downstream_tamper_rejected: usize,
    ambiguity_or_unsupported_preserved: usize,
    false_authorizations: usize,
    false_denials: usize,
    route_leakage: usize,
    family_counts: BTreeMap<String, usize>,
    receipts: Vec<Receipt>,
}

fn digest<T: Serialize>(value: &T) -> String {
    format!("{:x}", Sha256::digest(serde_json::to_vec(value).unwrap()))
}

fn arithmetic_text(index: usize, expected: Expected) -> String {
    match expected {
        Expected::Supported => match index % 4 {
            0 => "Find the number of divisors of n=36.".into(),
            1 => "Compute the sum of divisors at value n=60.".into(),
            2 => "Evaluate the Möbius function μ(n=30).".into(),
            _ => "Count the primes up to n=71 using the prime-counting function.".into(),
        },
        Expected::Ambiguous => match index % 2 {
            0 => "Find the divisor count or divisor sum at n=36.".into(),
            _ => "Determine an arithmetic function at n=36, but the function is not specified."
                .into(),
        },
        Expected::Unsupported => match index % 4 {
            0 => "Estimate the asymptotic prime-counting function.".into(),
            1 => "Use an analytic Dirichlet series to compute the divisor sum at n=36.".into(),
            2 => "Compute the unbounded number of divisors of n=1000000.".into(),
            _ => "Infer an approximate prime-counting value from a graph.".into(),
        },
    }
}

fn number_text(index: usize, expected: Expected) -> String {
    match expected {
        Expected::Supported => match index % 4 {
            0 => "Compute the greatest common divisor and Bezout coefficients for a=84 and b=30."
                .into(),
            1 => "Find the least nonnegative modular inverse of a=7 modulo m=20.".into(),
            2 => "Solve the linear congruence a=6 x congruent to b=9 modulo m=15.".into(),
            _ => "Compute Euler's totient phi(n=36).".into(),
        },
        Expected::Ambiguous => "Compute the gcd or modular inverse for a=7 and b=20.".into(),
        Expected::Unsupported => match index % 3 {
            0 => "Infer the cryptographic security of modulus m=20 from this inverse.".into(),
            1 => "Prove the asymptotic number-theory behavior as the modulus grows.".into(),
            _ => "Give an unbounded prime factorization and analytic number theory conclusion."
                .into(),
        },
    }
}

fn character_text(index: usize, expected: Expected) -> String {
    let prime = [5, 7, 11, 13][index % 4];
    match expected {
        Expected::Supported => match index % 4 {
            0 => format!("Validate the finite character modulo p={prime} with exponent k=1."),
            1 => format!("Evaluate the character value at x=2 modulo p={prime} with exponent k=1."),
            2 => format!(
                "Compute the partial sum through limit=8 modulo p={prime} with exponent k=1."
            ),
            _ => format!(
                "Check orthogonality of the finite character modulo p={prime} with exponent k=1."
            ),
        },
        Expected::Ambiguous => {
            "Evaluate or compute the partial sum of a character modulo p=5 with exponent k=1."
                .into()
        }
        Expected::Unsupported => match index % 3 {
            0 => {
                "Estimate the asymptotic Dirichlet series for modulus p=5 and exponent k=1.".into()
            }
            1 => "Compute analytic continuation of the L-function for p=7 and k=1.".into(),
            _ => "Evaluate the character for a composite modulus p=9 with exponent k=1.".into(),
        },
    }
}

fn homology_text(index: usize, expected: Expected) -> String {
    let supported = match index % 4 {
        0 => "Compute Betti numbers",
        1 => "Find the Euler characteristic",
        2 => "Construct the boundary matrices",
        _ => "Validate the complex",
    };
    match expected {
        Expected::Supported => format!(
            "{supported} for the finite simplicial complex. Vertices: [a,b,c]. Simplices: [[a],[b],[c],[a,b],[a,c],[b,c],[a,b,c]]. Coefficients: F_2."
        ),
        Expected::Ambiguous => "Compute Betti numbers for the finite simplicial complex. Vertices: [a,b,c]. Simplices: [[a],[b],[c],[a,b],[a,c],[b,c],[a,b,c]].".into(),
        Expected::Unsupported => match index % 3 {
            0 => "Compute persistent homology for an infinite complex on vertices [a,b].".into(),
            1 => "Compute Betti numbers over the integers for the finite complex on vertices [a,b].".into(),
            _ => "Use numerical approximation to analyze a continuous complex with vertices [a,b].".into(),
        },
    }
}

fn corpus() -> Vec<Case> {
    let families = [
        Family::Arithmetic,
        Family::NumberTheory,
        Family::Character,
        Family::Homology,
    ];
    let mut cases = Vec::with_capacity(1600);
    for family in families {
        for index in 0..400 {
            let expected = match index {
                0..240 => Expected::Supported,
                240..320 => Expected::Ambiguous,
                _ => Expected::Unsupported,
            };
            let text = match family {
                Family::Arithmetic => arithmetic_text(index, expected),
                Family::NumberTheory => number_text(index, expected),
                Family::Character => character_text(index, expected),
                Family::Homology => homology_text(index, expected),
            };
            cases.push(Case {
                id: format!("{family:?}_{index:03}"),
                family,
                expected,
                text,
            });
        }
    }
    cases
}

fn observe_arithmetic(text: &str, id: &str) -> (bool, RouteObservation) {
    let frontend = formalize_arithmetic(text, id);
    let mut tampered = frontend.clone();
    tampered.replay_hash.push('x');
    let frontend_replay = arithmetic_replay(&frontend);
    let frontend_tamper = !arithmetic_replay(&tampered);
    let Some(request) = frontend.request.as_ref() else {
        return (
            frontend.status == ArithmeticFrontendStatus::Complete,
            RouteObservation {
                complete: false,
                authorized: false,
                replay_verified: frontend_replay,
                tamper_rejected: frontend_tamper,
            },
        );
    };
    let result = evaluate_arithmetic(request);
    let mut result_tampered = result.clone();
    result_tampered.replay_hash.push('x');
    (
        frontend.status == ArithmeticFrontendStatus::Complete,
        RouteObservation {
            complete: frontend.status == ArithmeticFrontendStatus::Complete,
            authorized: result.status == ArithmeticFunctionStatus::Complete
                && result.replay_verified(),
            replay_verified: frontend_replay && result.replay_verified(),
            tamper_rejected: frontend_tamper && !result_tampered.replay_verified(),
        },
    )
}

fn observe_number(text: &str, id: &str) -> (bool, RouteObservation) {
    let frontend = formalize_number_theory_text(text, id);
    let mut tampered = frontend.clone();
    tampered.replay_hash.push('x');
    let frontend_replay = number_replay(&frontend);
    let frontend_tamper = !number_replay(&tampered);
    let Some(request) = frontend.request.as_ref() else {
        return (
            frontend.status == NumberTheoryFrontendStatus::Complete,
            RouteObservation {
                complete: false,
                authorized: false,
                replay_verified: frontend_replay,
                tamper_rejected: frontend_tamper,
            },
        );
    };
    let result = evaluate_number_theory(request);
    let mut result_tampered = result.clone();
    result_tampered.replay_hash.push('x');
    (
        frontend.status == NumberTheoryFrontendStatus::Complete,
        RouteObservation {
            complete: frontend.status == NumberTheoryFrontendStatus::Complete,
            authorized: result.status == NumberTheoryStatus::Complete
                && result.artifact.is_some()
                && result.replay_verified(),
            replay_verified: frontend_replay && result.replay_verified(),
            tamper_rejected: frontend_tamper && !result_tampered.replay_verified(),
        },
    )
}

fn observe_character(text: &str, id: &str) -> (bool, RouteObservation) {
    let frontend = formalize_character(text, id);
    let mut tampered = frontend.clone();
    tampered.replay_hash.push('x');
    let frontend_replay = character_replay(&frontend);
    let frontend_tamper = !character_replay(&tampered);
    let Some(request) = frontend.request.as_ref() else {
        return (
            frontend.status == CharacterFrontendStatus::Complete,
            RouteObservation {
                complete: false,
                authorized: false,
                replay_verified: frontend_replay,
                tamper_rejected: frontend_tamper,
            },
        );
    };
    let result = evaluate_character(request);
    let mut result_tampered = result.clone();
    result_tampered.replay_hash.push('x');
    (
        frontend.status == CharacterFrontendStatus::Complete,
        RouteObservation {
            complete: frontend.status == CharacterFrontendStatus::Complete,
            authorized: result.status == CharacterStatus::Complete && result.authorized(),
            replay_verified: frontend_replay && result.replay_verified(),
            tamper_rejected: frontend_tamper && !result_tampered.replay_verified(),
        },
    )
}

fn observe_homology(text: &str) -> (bool, RouteObservation) {
    let frontend = formalize_homology(text);
    let mut tampered = frontend.clone();
    tampered.replay_hash.push('x');
    let frontend_replay = frontend.replay_verified();
    let frontend_tamper = !tampered.replay_verified();
    let Some(request) = frontend.request.as_ref() else {
        return (
            frontend.status == HomologyFrontendStatus::Complete,
            RouteObservation {
                complete: false,
                authorized: false,
                replay_verified: frontend_replay,
                tamper_rejected: frontend_tamper,
            },
        );
    };
    let result = evaluate_homology(request);
    let mut result_tampered = result.clone();
    result_tampered.replay_hash.push('x');
    (
        frontend.status == HomologyFrontendStatus::Complete,
        RouteObservation {
            complete: frontend.status == HomologyFrontendStatus::Complete,
            authorized: result.status == HomologyStatus::Complete && result.authorized(),
            replay_verified: frontend_replay && result.replay_verified(),
            tamper_rejected: frontend_tamper && !result_tampered.replay_verified(),
        },
    )
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cases = corpus();
    assert_eq!(cases.len(), 1600);
    let corpus_sha256 = digest(&cases);
    let mut receipts = Vec::with_capacity(cases.len());
    let mut family_counts = BTreeMap::new();
    for case in cases {
        *family_counts
            .entry(format!("{:?}", case.family))
            .or_insert(0usize) += 1;
        let (arithmetic_complete, arithmetic) = observe_arithmetic(&case.text, &case.id);
        let (number_complete, number) = observe_number(&case.text, &case.id);
        let (character_complete, character) = observe_character(&case.text, &case.id);
        let (homology_complete, homology) = observe_homology(&case.text);
        let complete_routes = [
            (Family::Arithmetic, arithmetic_complete),
            (Family::NumberTheory, number_complete),
            (Family::Character, character_complete),
            (Family::Homology, homology_complete),
        ];
        let selected = (complete_routes
            .iter()
            .filter(|(_, complete)| *complete)
            .count()
            == 1)
            .then(|| {
                complete_routes
                    .iter()
                    .find(|(_, complete)| *complete)
                    .unwrap()
                    .0
            });
        let route_exact = match case.expected {
            Expected::Supported => selected == Some(case.family),
            Expected::Ambiguous | Expected::Unsupported => selected.is_none(),
        };
        let selected_observation = match selected {
            Some(Family::Arithmetic) => arithmetic,
            Some(Family::NumberTheory) => number,
            Some(Family::Character) => character,
            Some(Family::Homology) => homology,
            None => RouteObservation {
                complete: false,
                authorized: false,
                replay_verified: true,
                tamper_rejected: true,
            },
        };
        let authorized = selected.is_some_and(|_| selected_observation.authorized);
        let frontend_replay_verified = arithmetic.replay_verified
            && number.replay_verified
            && character.replay_verified
            && homology.replay_verified;
        let frontend_tamper_rejected = arithmetic.tamper_rejected
            && number.tamper_rejected
            && character.tamper_rejected
            && homology.tamper_rejected;
        let downstream_emitted = selected.is_some();
        let false_authorization = case.expected != Expected::Supported && authorized;
        let false_denial = case.expected == Expected::Supported && !authorized;
        receipts.push(Receipt {
            id: case.id,
            family: case.family,
            expected: case.expected,
            selected_route: selected,
            route_exact,
            authorized,
            frontend_replay_verified,
            downstream_replay_verified: !downstream_emitted || selected_observation.replay_verified,
            frontend_tamper_rejected,
            downstream_tamper_rejected: !downstream_emitted || selected_observation.tamper_rejected,
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
    let frontend_invocations = cases * 4;
    let exact_route_decisions = receipts.iter().filter(|r| r.route_exact).count();
    let supported_authorizations = receipts.iter().filter(|r| r.authorized).count();
    let downstream_emitted = receipts
        .iter()
        .filter(|r| r.selected_route.is_some())
        .count();
    let frontend_replay_verified = receipts
        .iter()
        .filter(|r| r.frontend_replay_verified)
        .count();
    let downstream_replay_verified = receipts
        .iter()
        .filter(|r| r.selected_route.is_some() && r.downstream_replay_verified)
        .count();
    let frontend_tamper_rejected = receipts
        .iter()
        .filter(|r| r.frontend_tamper_rejected)
        .count();
    let downstream_tamper_rejected = receipts
        .iter()
        .filter(|r| r.selected_route.is_some() && r.downstream_tamper_rejected)
        .count();
    let ambiguity_or_unsupported_preserved = receipts
        .iter()
        .filter(|r| {
            matches!(r.expected, Expected::Ambiguous | Expected::Unsupported)
                && r.selected_route.is_none()
        })
        .count();
    let false_authorizations = receipts.iter().filter(|r| r.false_authorization).count();
    let false_denials = receipts.iter().filter(|r| r.false_denial).count();
    let route_leakage = receipts
        .iter()
        .filter(|r| r.expected == Expected::Supported && r.selected_route != Some(r.family))
        .count();
    assert_eq!((supported, ambiguous, unsupported), (960, 320, 320));
    assert_eq!(frontend_invocations, 6400);
    assert_eq!(exact_route_decisions, cases);
    assert_eq!(supported_authorizations, supported);
    assert_eq!(downstream_emitted, supported);
    assert_eq!(frontend_replay_verified, cases);
    assert_eq!(downstream_replay_verified, supported);
    assert_eq!(frontend_tamper_rejected, cases);
    assert_eq!(downstream_tamper_rejected, supported);
    assert_eq!(ambiguity_or_unsupported_preserved, ambiguous + unsupported);
    assert_eq!(false_authorizations, 0);
    assert_eq!(false_denials, 0);
    assert_eq!(route_leakage, 0);
    let report = Report {
        schema: "stage137-arithmetic-route-blind-language-v1",
        source: "independently authored arithmetic and mixed technical-language corpus",
        corpus_sha256,
        cases,
        supported,
        ambiguous,
        unsupported,
        frontend_invocations,
        exact_route_decisions,
        supported_authorizations,
        downstream_emitted,
        frontend_replay_verified,
        downstream_replay_verified,
        frontend_tamper_rejected,
        downstream_tamper_rejected,
        ambiguity_or_unsupported_preserved,
        false_authorizations,
        false_denials,
        route_leakage,
        family_counts,
        receipts,
    };
    let json = serde_json::to_vec_pretty(&report)?;
    std::fs::write("docs/stage137_arithmetic_route_blind_language.json", &json)?;
    println!("{}", String::from_utf8(json)?);
    Ok(())
}
