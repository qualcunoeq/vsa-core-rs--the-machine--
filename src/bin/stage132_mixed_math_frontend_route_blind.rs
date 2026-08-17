//! Stage 132: mixed mathematical technical-language routing.
//!
//! Three independently built frontends receive every report.  A supported
//! report must have exactly one complete frontend and a replayable downstream
//! artifact; ambiguous and unsupported reports must have no complete route.

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
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
    Homology,
    NumberTheory,
    Character,
}

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
    family: Family,
    expected: Expected,
    text: String,
}

#[derive(Debug, Serialize)]
struct Receipt {
    id: String,
    family: Family,
    expected: Expected,
    selected_route: Option<Family>,
    route_exact: bool,
    downstream_status: Option<String>,
    downstream_emitted: bool,
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
    status_counts: BTreeMap<String, usize>,
    receipts: Vec<Receipt>,
}

fn digest<T: Serialize>(value: &T) -> String {
    format!("{:x}", Sha256::digest(serde_json::to_vec(value).unwrap()))
}

fn homology_supported(index: usize) -> String {
    let operation = match index % 4 {
        0 => "Compute Betti numbers",
        1 => "Find the Euler characteristic",
        2 => "Construct the boundary matrices",
        _ => "Validate the complex",
    };
    format!(
        "{operation} for the finite simplicial complex. Vertices: [a,b,c]. Simplices: [[a],[b],[c],[a,b],[a,c],[b,c],[a,b,c]]. Coefficients: F_2."
    )
}

fn homology_ambiguous(index: usize) -> String {
    if index % 2 == 0 {
        "Compute Betti numbers for the finite simplicial complex. Vertices: [a,b,c]. Simplices: [[a],[b],[c],[a,b],[a,c],[b,c],[a,b,c]].".into()
    } else {
        "Study the finite simplicial complex. Vertices: [a,b,c]. Simplices: [[a],[b],[c],[a,b],[a,c],[b,c],[a,b,c]]. Coefficients: F_2.".into()
    }
}

fn homology_unsupported(index: usize) -> String {
    match index % 3 {
        0 => "Compute persistent homology for an infinite complex on vertices [a,b].".into(),
        1 => "Compute Betti numbers over the integers for the finite complex on vertices [a,b]."
            .into(),
        _ => "Use numerical approximation to analyze a continuous complex with vertices [a,b]."
            .into(),
    }
}

fn number_supported(index: usize) -> String {
    match index % 4 {
        0 => {
            "Compute the greatest common divisor and Bezout coefficients for a=84 and b=30.".into()
        }
        1 => "Find the least nonnegative modular inverse of a=7 modulo m=20.".into(),
        2 => "Solve the linear congruence a=6 x congruent to b=9 modulo m=15.".into(),
        _ => "Compute Euler's totient phi(n=36).".into(),
    }
}

fn number_ambiguous(index: usize) -> String {
    match index % 2 {
        0 => "Compute the gcd or modular inverse for a=7 and b=20.".into(),
        _ => "The problem may ask for a totient or an inverse; n=36 and m=20 are supplied.".into(),
    }
}

fn number_unsupported(index: usize) -> String {
    match index % 3 {
        0 => "Infer the cryptographic security of modulus m=20 from this inverse.".into(),
        1 => "Prove the asymptotic number-theory behavior of a=7 as the modulus grows.".into(),
        _ => "Give an unbounded prime factorization and analytic number theory conclusion.".into(),
    }
}

fn character_supported(index: usize) -> String {
    match index % 4 {
        0 => format!(
            "Validate the finite character modulo p={} with exponent k=1.",
            [5, 7, 11, 13][index % 4]
        ),
        1 => format!(
            "Evaluate the character value at x=2 modulo p={} with exponent k=1.",
            [5, 7, 11, 13][index % 4]
        ),
        2 => format!(
            "Compute the partial sum through limit=8 modulo p={} with exponent k=1.",
            [5, 7, 11, 13][index % 4]
        ),
        _ => format!(
            "Check orthogonality of the finite character modulo p={} with exponent k=1.",
            [5, 7, 11, 13][index % 4]
        ),
    }
}

fn character_ambiguous(index: usize) -> String {
    match index % 2 {
        0 => "Evaluate or compute the partial sum of a character modulo p=5 with exponent k=1."
            .into(),
        _ => "Evaluate the character value at x=2 modulo p=11; the exponent is omitted.".into(),
    }
}

fn character_unsupported(index: usize) -> String {
    match index % 3 {
        0 => "Estimate the asymptotic Dirichlet series for modulus p=5 and exponent k=1.".into(),
        1 => "Compute analytic continuation of the L-function for p=7 and k=1.".into(),
        _ => "Evaluate the character for a composite modulus p=9 with exponent k=1.".into(),
    }
}

fn corpus() -> Vec<Case> {
    let mut cases = Vec::with_capacity(1200);
    for family in [Family::Homology, Family::NumberTheory, Family::Character] {
        for index in 0..240 {
            let (expected, text) = match (family, index) {
                (Family::Homology, 0..120) => (Expected::Supported, homology_supported(index)),
                (Family::Homology, 120..160) => (Expected::Ambiguous, homology_ambiguous(index)),
                (Family::Homology, _) => (Expected::Unsupported, homology_unsupported(index)),
                (Family::NumberTheory, 0..120) => (Expected::Supported, number_supported(index)),
                (Family::NumberTheory, 120..160) => (Expected::Ambiguous, number_ambiguous(index)),
                (Family::NumberTheory, _) => (Expected::Unsupported, number_unsupported(index)),
                (Family::Character, 0..120) => (Expected::Supported, character_supported(index)),
                (Family::Character, 120..160) => (Expected::Ambiguous, character_ambiguous(index)),
                (Family::Character, _) => (Expected::Unsupported, character_unsupported(index)),
            };
            cases.push(Case {
                id: format!("{:?}_{index:03}", family),
                family,
                expected,
                text,
            });
        }
    }
    cases
}

fn number_authorized(result: &the_machine::number_theory_pack::NumberTheoryResult) -> bool {
    result.status == NumberTheoryStatus::Complete
        && result.artifact.is_some()
        && result.replay_verified()
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cases = corpus();
    assert_eq!(cases.len(), 720);
    let corpus_sha256 = digest(&cases);
    let mut receipts = Vec::with_capacity(cases.len());
    let mut status_counts = BTreeMap::new();
    for case in cases {
        let homology = formalize_homology(&case.text);
        let number = formalize_number_theory_text(&case.text, &case.id);
        let character = formalize_character(&case.text, &case.id);
        let frontend_replay_verified =
            homology.replay_verified() && number_replay(&number) && character_replay(&character);
        let mut homology_tampered = homology.clone();
        homology_tampered.replay_hash.push('x');
        let mut number_tampered = number.clone();
        number_tampered.replay_hash.push('x');
        let mut character_tampered = character.clone();
        character_tampered.replay_hash.push('x');
        let frontend_tamper_rejected = !homology_tampered.replay_verified()
            && !number_replay(&number_tampered)
            && !character_replay(&character_tampered);
        let complete_routes = [
            (
                Family::Homology,
                homology.status == HomologyFrontendStatus::Complete,
            ),
            (
                Family::NumberTheory,
                number.status == NumberTheoryFrontendStatus::Complete,
            ),
            (
                Family::Character,
                character.status == CharacterFrontendStatus::Complete,
            ),
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
        let downstream = match selected {
            Some(Family::Homology) => homology.request.as_ref().map(|request| {
                (
                    evaluate_homology(request).status == HomologyStatus::Complete,
                    evaluate_homology(request).replay_verified(),
                    evaluate_homology(request).authorized(),
                    {
                        let mut result = evaluate_homology(request);
                        result.replay_hash.push('x');
                        !result.replay_verified()
                    },
                )
            }),
            Some(Family::NumberTheory) => number.request.as_ref().map(|request| {
                (
                    evaluate_number_theory(request).status == NumberTheoryStatus::Complete,
                    evaluate_number_theory(request).replay_verified(),
                    number_authorized(&evaluate_number_theory(request)),
                    {
                        let mut result = evaluate_number_theory(request);
                        result.replay_hash.push('x');
                        !result.replay_verified()
                    },
                )
            }),
            Some(Family::Character) => character.request.as_ref().map(|request| {
                (
                    evaluate_character(request).status == CharacterStatus::Complete,
                    evaluate_character(request).replay_verified(),
                    evaluate_character(request).authorized(),
                    {
                        let mut result = evaluate_character(request);
                        result.replay_hash.push('x');
                        !result.replay_verified()
                    },
                )
            }),
            None => None,
        };
        let authorized =
            downstream.is_some_and(|(complete, _, authorized, _)| complete && authorized);
        let downstream_replay_verified = downstream.is_none_or(|(_, replay, _, _)| replay);
        let downstream_tamper_rejected = downstream.is_none_or(|(_, _, _, tamper)| tamper);
        let false_authorization = case.expected != Expected::Supported && authorized;
        let false_denial = case.expected == Expected::Supported && !authorized;
        let status_key = format!("{:?}:{:?}", case.family, case.expected);
        *status_counts.entry(status_key).or_insert(0usize) += 1;
        receipts.push(Receipt {
            id: case.id,
            family: case.family,
            expected: case.expected,
            selected_route: selected,
            route_exact,
            downstream_status: downstream.map(|(complete, _, _, _)| complete.to_string()),
            downstream_emitted: selected.is_some(),
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
    let frontend_invocations = cases * 3;
    let exact_route_decisions = receipts.iter().filter(|r| r.route_exact).count();
    let supported_authorizations = receipts.iter().filter(|r| r.authorized).count();
    let downstream_emitted = receipts.iter().filter(|r| r.downstream_emitted).count();
    let frontend_replay_verified = receipts
        .iter()
        .filter(|r| r.frontend_replay_verified)
        .count();
    let downstream_replay_verified = receipts
        .iter()
        .filter(|r| r.downstream_emitted && r.downstream_replay_verified)
        .count();
    let frontend_tamper_rejected = receipts
        .iter()
        .filter(|r| r.frontend_tamper_rejected)
        .count();
    let downstream_tamper_rejected = receipts
        .iter()
        .filter(|r| r.downstream_emitted && r.downstream_tamper_rejected)
        .count();
    let ambiguity_or_unsupported_preserved = receipts
        .iter()
        .filter(|r| r.expected != Expected::Supported && r.selected_route.is_none())
        .count();
    let false_authorizations = receipts.iter().filter(|r| r.false_authorization).count();
    let false_denials = receipts.iter().filter(|r| r.false_denial).count();
    let route_leakage = receipts
        .iter()
        .filter(|r| (r.expected == Expected::Supported) != r.authorized)
        .count();
    assert_eq!((supported, ambiguous, unsupported), (360, 120, 240));
    assert_eq!(frontend_invocations, 2160);
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
        schema: "stage132-mixed-math-frontend-route-blind-v1",
        source: "independently authored mixed homology, elementary-number-theory, and finite-character corpus",
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
        status_counts,
        receipts,
    };
    std::fs::write(
        "docs/stage132_mixed_math_frontend_route_blind.json",
        serde_json::to_vec_pretty(&report)?,
    )?;
    println!("{}", serde_json::to_string_pretty(&report)?);
    Ok(())
}
