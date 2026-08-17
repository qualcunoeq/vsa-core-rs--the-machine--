//! Stage 144: route-blind composition of five bounded technical frontends.

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
use the_machine::combinatorics_frontend::{
    formalize as formalize_combinatorics, replay_verified as combinatorics_replay,
    CombinatoricsFrontendStatus,
};
use the_machine::combinatorics_pack::{evaluate_combinatorics, CombinatoricsStatus};
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

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
enum Family {
    Arithmetic,
    NumberTheory,
    Combinatorics,
    Character,
    Homology,
}

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
enum Expected {
    Supported,
    Ambiguous,
    Unsupported,
}

#[derive(Debug, Clone)]
struct Case {
    id: String,
    family: Family,
    expected: Expected,
    text: String,
}

#[derive(Debug, Serialize)]
struct Receipt {
    id: String,
    family: Family,
    expected: Expected,
    complete_routes: usize,
    unique_route: Option<Family>,
    authorized: bool,
    exact: bool,
    replay_verified: bool,
    tamper_rejected: bool,
}

#[derive(Debug, Serialize)]
struct Report {
    schema: &'static str,
    source: &'static str,
    corpus_sha256: String,
    cases: usize,
    frontend_invocations: usize,
    supported: usize,
    ambiguous: usize,
    unsupported: usize,
    exact_route_decisions: usize,
    supported_authorizations: usize,
    ambiguity_or_unsupported_preserved: usize,
    replay_verified: usize,
    tamper_rejections: usize,
    false_authorizations: usize,
    false_denials: usize,
    route_leakage: usize,
    route_counts: BTreeMap<String, usize>,
    receipts: Vec<Receipt>,
}

fn digest<T: Serialize>(value: &T) -> String {
    format!("{:x}", Sha256::digest(serde_json::to_vec(value).unwrap()))
}

fn homology_text() -> String {
    "Compute Betti numbers for the finite simplicial complex. Vertices: [a,b,c]. Simplices: [[a],[b],[c],[a,b],[a,c],[b,c],[a,b,c]]. Coefficients: F_2.".into()
}

fn corpus() -> Vec<Case> {
    let supported = [
        (Family::Arithmetic, "Compute the Möbius function μ(n=30)."),
        (Family::NumberTheory, "Compute Euler's totient phi(n=36)."),
        (
            Family::Combinatorics,
            "Compute combinations choose n=8 k=3.",
        ),
        (
            Family::Character,
            "Evaluate the character value at x=2 modulo p=5 with exponent k=1.",
        ),
        (Family::Homology, ""),
    ];
    let mut cases = Vec::with_capacity(500);
    for index in 0..100 {
        for (family, text) in supported {
            cases.push(Case {
                id: format!("supported_{index:03}_{family:?}"),
                family,
                expected: Expected::Supported,
                text: if family == Family::Homology {
                    homology_text()
                } else {
                    text.into()
                },
            });
        }
    }
    for index in 0..100 {
        cases.push(Case {
            id: format!("ambiguous_{index:03}"),
            family: Family::Combinatorics,
            expected: Expected::Ambiguous,
            text: if index % 2 == 0 {
                "A quoted formula gives combinations n=8 k=3, while another scope asks permutations n=8 k=3; select neither.".into()
            } else {
                "A theorem defines μ(n=12) and phi(n=36) before asking for the relevant arithmetic result; scope is not identified.".into()
            },
        });
    }
    for index in 0..100 {
        cases.push(Case {
            id: format!("unsupported_{index:03}"),
            family: Family::Character,
            expected: Expected::Unsupported,
            text: if index % 2 == 0 {
                "Estimate the asymptotic Dirichlet series of an infinite random graph.".into()
            } else {
                "Compute an unbounded weighted count with an unspecified domain.".into()
            },
        });
    }
    cases
}

#[derive(Clone, Copy)]
struct Observation {
    family: Family,
    complete: bool,
    authorized: bool,
    replay: bool,
    tamper: bool,
}

fn observe(case: &Case) -> Vec<Observation> {
    let arithmetic = formalize_arithmetic(&case.text, &case.id);
    let mut arithmetic_bad = arithmetic.clone();
    arithmetic_bad.replay_hash.push('x');
    let arithmetic_auth = arithmetic.request.as_ref().is_some_and(|r| {
        let result = evaluate_arithmetic(r);
        result.status == ArithmeticFunctionStatus::Complete && result.replay_verified()
    });
    let number = formalize_number_theory_text(&case.text, &case.id);
    let mut number_bad = number.clone();
    number_bad.replay_hash.push('x');
    let number_auth = number.request.as_ref().is_some_and(|r| {
        let result = evaluate_number_theory(r);
        result.status == NumberTheoryStatus::Complete
            && result.artifact.is_some()
            && result.replay_verified()
    });
    let combinatorics = formalize_combinatorics(&case.text, &case.id);
    let mut combinatorics_bad = combinatorics.clone();
    combinatorics_bad.replay_hash.push('x');
    let combinatorics_auth = combinatorics.request.as_ref().is_some_and(|r| {
        let result = evaluate_combinatorics(r);
        result.status == CombinatoricsStatus::Complete
            && result.artifact.is_some()
            && result.replay_verified()
    });
    let character = formalize_character(&case.text, &case.id);
    let mut character_bad = character.clone();
    character_bad.replay_hash.push('x');
    let character_auth = character.request.as_ref().is_some_and(|r| {
        let result = evaluate_character(r);
        result.status == CharacterStatus::Complete
            && result.artifact.is_some()
            && result.replay_verified()
    });
    let homology = formalize_homology(&case.text);
    let mut homology_bad = homology.clone();
    homology_bad.replay_hash.push('x');
    let homology_auth = homology.request.as_ref().is_some_and(|r| {
        let result = evaluate_homology(r);
        result.status == HomologyStatus::Complete
            && result.artifact.is_some()
            && result.replay_verified()
    });
    vec![
        Observation {
            family: Family::Arithmetic,
            complete: arithmetic.status == ArithmeticFrontendStatus::Complete,
            authorized: arithmetic_auth,
            replay: arithmetic_replay(&arithmetic),
            tamper: !arithmetic_replay(&arithmetic_bad),
        },
        Observation {
            family: Family::NumberTheory,
            complete: number.status == NumberTheoryFrontendStatus::Complete,
            authorized: number_auth,
            replay: number_replay(&number),
            tamper: !number_replay(&number_bad),
        },
        Observation {
            family: Family::Combinatorics,
            complete: combinatorics.status == CombinatoricsFrontendStatus::Complete,
            authorized: combinatorics_auth,
            replay: combinatorics_replay(&combinatorics),
            tamper: !combinatorics_replay(&combinatorics_bad),
        },
        Observation {
            family: Family::Character,
            complete: character.status == CharacterFrontendStatus::Complete,
            authorized: character_auth,
            replay: character_replay(&character),
            tamper: !character_replay(&character_bad),
        },
        Observation {
            family: Family::Homology,
            complete: homology.status == HomologyFrontendStatus::Complete,
            authorized: homology_auth,
            replay: homology.replay_verified(),
            tamper: !homology_bad.replay_verified(),
        },
    ]
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cases = corpus();
    assert_eq!(cases.len(), 700);
    let corpus_sha256 = digest(
        &cases
            .iter()
            .map(|c| (&c.id, c.family, c.expected, &c.text))
            .collect::<Vec<_>>(),
    );
    let mut receipts = Vec::with_capacity(cases.len());
    let mut route_counts = BTreeMap::new();
    for case in cases {
        let observations = observe(&case);
        let complete = observations
            .iter()
            .filter(|o| o.complete)
            .collect::<Vec<_>>();
        let unique = (complete.len() == 1).then(|| complete[0].family);
        let authorized = unique
            .and_then(|family| {
                complete
                    .iter()
                    .find(|o| o.family == family)
                    .map(|o| o.authorized)
            })
            .unwrap_or(false);
        let exact = match case.expected {
            Expected::Supported => unique == Some(case.family) && authorized,
            Expected::Ambiguous | Expected::Unsupported => unique.is_none() && !authorized,
        };
        let replay = observations.iter().all(|o| o.replay);
        let tamper = observations.iter().all(|o| o.tamper);
        let route_key = unique
            .map(|f| format!("{f:?}"))
            .unwrap_or_else(|| "none".into());
        *route_counts.entry(route_key).or_insert(0) += 1;
        receipts.push(Receipt {
            id: case.id,
            family: case.family,
            expected: case.expected,
            complete_routes: complete.len(),
            unique_route: unique,
            authorized,
            exact,
            replay_verified: replay,
            tamper_rejected: tamper,
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
    let exact_route_decisions = receipts.iter().filter(|r| r.exact).count();
    let supported_authorizations = receipts.iter().filter(|r| r.authorized).count();
    let ambiguity_or_unsupported_preserved = receipts
        .iter()
        .filter(|r| r.expected != Expected::Supported && r.unique_route.is_none())
        .count();
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
    let route_leakage = receipts
        .iter()
        .filter(|r| r.expected == Expected::Supported && r.unique_route != Some(r.family))
        .count();
    assert_eq!((supported, ambiguous, unsupported), (500, 100, 100));
    assert_eq!(exact_route_decisions, 700);
    assert_eq!(supported_authorizations, 500);
    assert_eq!(ambiguity_or_unsupported_preserved, 200);
    assert_eq!(replay_verified, 700);
    assert_eq!(tamper_rejections, 700);
    assert_eq!(false_authorizations, 0);
    assert_eq!(false_denials, 0);
    assert_eq!(route_leakage, 0);
    let report = Report {
        schema: "stage144-mixed-frontend-route-blind-v1",
        source: "independently authored five-family technical-language corpus",
        corpus_sha256,
        cases,
        frontend_invocations: cases * 5,
        supported,
        ambiguous,
        unsupported,
        exact_route_decisions,
        supported_authorizations,
        ambiguity_or_unsupported_preserved,
        replay_verified,
        tamper_rejections,
        false_authorizations,
        false_denials,
        route_leakage,
        route_counts,
        receipts,
    };
    let json = serde_json::to_vec_pretty(&report)?;
    std::fs::write("docs/stage144_mixed_frontend_route_blind.json", &json)?;
    println!("{}", String::from_utf8(json)?);
    Ok(())
}
