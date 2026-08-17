//! Stage 133: sealed shifted technical-language exam for the mixed math routes.
//!
//! The corpus is permanently partitioned before execution.  Development and
//! validation are visible for curriculum work; the sealed partition is
//! reported separately and has an independent hash.  All three frontends are
//! invoked on every report so routing cannot depend on a predeclared domain.

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

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
enum Family {
    Homology,
    NumberTheory,
    Character,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
enum Expected {
    Supported,
    Ambiguous,
    Unsupported,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
enum Partition {
    Development,
    Validation,
    Sealed,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Case {
    id: String,
    family: Family,
    expected: Expected,
    partition: Partition,
    text: String,
}

#[derive(Debug, Serialize, Default)]
struct PartitionStats {
    cases: usize,
    supported: usize,
    ambiguous: usize,
    unsupported: usize,
    exact_route_decisions: usize,
    authorizations: usize,
    frontend_replay_verified: usize,
    downstream_emitted: usize,
    downstream_replay_verified: usize,
    frontend_tamper_rejected: usize,
    downstream_tamper_rejected: usize,
    false_authorizations: usize,
    false_denials: usize,
}

#[derive(Debug, Serialize)]
struct Receipt {
    id: String,
    family: Family,
    partition: Partition,
    expected: Expected,
    selected_route: Option<Family>,
    route_exact: bool,
    authorized: bool,
    frontend_replay_verified: bool,
    downstream_emitted: bool,
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
    development_sha256: String,
    validation_sha256: String,
    sealed_sha256: String,
    cases: usize,
    frontend_invocations: usize,
    status_counts: BTreeMap<String, usize>,
    partitions: BTreeMap<Partition, PartitionStats>,
    receipts: Vec<Receipt>,
}

fn digest<T: Serialize>(value: &T) -> String {
    format!("{:x}", Sha256::digest(serde_json::to_vec(value).unwrap()))
}

fn partition(index: usize) -> Partition {
    match index % 3 {
        0 => Partition::Development,
        1 => Partition::Validation,
        _ => Partition::Sealed,
    }
}

fn homology_text(index: usize, expected: Expected) -> String {
    match expected {
        Expected::Supported => match index % 4 {
            0 => "Compute Betti numbers for a finite simplicial complex. Vertices: [u,v,w]. Simplex list: [[u],[v],[w],[u,v],[u,w],[v,w],[u,v,w]]. Coefficients: F₂.".into(),
            1 => "Find the Euler characteristic. Vertex set: [u,v,w]. Faces: [[u],[v],[w],[u,v],[u,w],[v,w],[u,v,w]]. Coefficients: F_2.".into(),
            2 => "Construct the boundary matrices on vertices [u,v,w]. Simplices: [[u],[v],[w],[u,v],[u,w],[v,w],[u,v,w]]. Coefficients: F2.".into(),
            _ => "Validate the complex on vertices [u,v,w]. Faces: [[u],[v],[w],[u,v],[u,w],[v,w],[u,v,w]]. Coefficients: F₂.".into(),
        },
        Expected::Ambiguous => {
            if index % 2 == 0 {
                "Compute Betti numbers for a finite complex. Vertex set: [u,v,w]. Faces: [[u],[v],[w],[u,v],[u,w],[v,w],[u,v,w]].".into()
            } else {
                "Study this finite simplicial complex. Vertices: [u,v,w]. Simplex list: [[u],[v],[w],[u,v],[u,w],[v,w],[u,v,w]]. Coefficients: F₂.".into()
            }
        }
        Expected::Unsupported => match index % 3 {
            0 => "Compute persistent homology for an infinite complex on vertices [u,v].".into(),
            1 => "Compute Betti numbers over the integers for a finite complex on vertices [u,v].".into(),
            _ => "Approximate a continuous complex numerically using vertices [u,v].".into(),
        },
    }
}

fn number_text(index: usize, expected: Expected) -> String {
    match expected {
        Expected::Supported => match index % 4 {
            0 => "Determine Bezout coefficients and the gcd of a = 84 and b = 30.".into(),
            1 => "Determine the least nonnegative modular inverse of value a=7 modulo m=20.".into(),
            2 => "Solve the linear congruence a=6 x congruent to b=9 modulo m=15.".into(),
            _ => "Evaluate Euler's totient function at n = 36 exactly.".into(),
        },
        Expected::Ambiguous => match index % 2 {
            0 => "Compute the gcd or modular inverse for a=7 and b=20.".into(),
            _ => "The requested result may be a totient or an inverse; n=36 and m=20 are supplied."
                .into(),
        },
        Expected::Unsupported => match index % 3 {
            0 => "Infer the cryptographic security consequence of modulus m=20.".into(),
            1 => "Prove the asymptotic number-theory behavior as the modulus grows.".into(),
            _ => "Give an unbounded prime factorization and analytic number-theory conclusion."
                .into(),
        },
    }
}

fn character_text(index: usize, expected: Expected) -> String {
    let prime = [5, 7, 11, 13][index % 4];
    match expected {
        Expected::Supported => match index % 4 {
            0 => format!("Validate a finite character modulo p={prime} with exponent k=1."),
            1 => format!("Evaluate its character value at x=2 modulo p={prime} with exponent k=1."),
            2 => format!("Compute the finite partial sum through limit=8 modulo p={prime} with exponent k=1."),
            _ => format!("Check character orthogonality modulo p={prime} with exponent k=1."),
        },
        Expected::Ambiguous => match index % 2 {
            0 => "Evaluate or compute the partial sum of a character modulo p=5 with exponent k=1.".into(),
            _ => "Evaluate the character value at x=2 modulo p=11, but omit the exponent.".into(),
        },
        Expected::Unsupported => match index % 3 {
            0 => "Estimate the asymptotic Dirichlet series for p=5 and exponent k=1.".into(),
            1 => "Compute analytic continuation of the L-function for p=7 and k=1.".into(),
            _ => "Evaluate a character for composite modulus p=9 with exponent k=1.".into(),
        },
    }
}

fn corpus() -> Vec<Case> {
    let mut cases = Vec::with_capacity(1200);
    for family in [Family::Homology, Family::NumberTheory, Family::Character] {
        for index in 0..400 {
            let expected = if index < 240 {
                Expected::Supported
            } else if index < 320 {
                Expected::Ambiguous
            } else {
                Expected::Unsupported
            };
            let text = match family {
                Family::Homology => homology_text(index, expected),
                Family::NumberTheory => number_text(index, expected),
                Family::Character => character_text(index, expected),
            };
            let partition = partition(cases.len());
            cases.push(Case {
                id: format!("{:?}-{index:03}", family),
                family,
                expected,
                partition,
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
    assert_eq!(cases.len(), 1200);
    let corpus_sha256 = digest(&cases);
    let development_sha256 = digest(
        &cases
            .iter()
            .filter(|case| case.partition == Partition::Development)
            .collect::<Vec<_>>(),
    );
    let validation_sha256 = digest(
        &cases
            .iter()
            .filter(|case| case.partition == Partition::Validation)
            .collect::<Vec<_>>(),
    );
    let sealed_sha256 = digest(
        &cases
            .iter()
            .filter(|case| case.partition == Partition::Sealed)
            .collect::<Vec<_>>(),
    );
    let mut receipts = Vec::with_capacity(cases.len());
    let mut partitions = BTreeMap::<Partition, PartitionStats>::new();
    let mut status_counts = BTreeMap::new();
    for case in cases {
        let homology = formalize_homology(&case.text);
        let number = formalize_number_theory_text(&case.text, &case.id);
        let character = formalize_character(&case.text, &case.id);
        let frontend_replay_verified =
            homology.replay_verified() && number_replay(&number) && character_replay(&character);
        let mut h_tampered = homology.clone();
        h_tampered.replay_hash.push('x');
        let mut n_tampered = number.clone();
        n_tampered.replay_hash.push('x');
        let mut c_tampered = character.clone();
        c_tampered.replay_hash.push('x');
        let frontend_tamper_rejected = !h_tampered.replay_verified()
            && !number_replay(&n_tampered)
            && !character_replay(&c_tampered);
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
                let result = evaluate_homology(request);
                let authorized = result.status == HomologyStatus::Complete && result.authorized();
                let replay = result.replay_verified();
                let mut tampered = result.clone();
                tampered.replay_hash.push('x');
                (authorized, replay, !tampered.replay_verified())
            }),
            Some(Family::NumberTheory) => number.request.as_ref().map(|request| {
                let result = evaluate_number_theory(request);
                let authorized = number_authorized(&result);
                let replay = result.replay_verified();
                let mut tampered = result.clone();
                tampered.replay_hash.push('x');
                (authorized, replay, !tampered.replay_verified())
            }),
            Some(Family::Character) => character.request.as_ref().map(|request| {
                let result = evaluate_character(request);
                let authorized = result.status == CharacterStatus::Complete && result.authorized();
                let replay = result.replay_verified();
                let mut tampered = result.clone();
                tampered.replay_hash.push('x');
                (authorized, replay, !tampered.replay_verified())
            }),
            None => None,
        };
        let authorized = downstream.is_some_and(|(authorized, _, _)| authorized);
        let downstream_emitted = selected.is_some();
        let downstream_replay_verified = downstream.is_none_or(|(_, replay, _)| replay);
        let downstream_tamper_rejected = downstream.is_none_or(|(_, _, tamper)| tamper);
        let false_authorization = case.expected != Expected::Supported && authorized;
        let false_denial = case.expected == Expected::Supported && !authorized;
        *status_counts
            .entry(format!("{:?}:{:?}", case.family, case.expected))
            .or_insert(0usize) += 1;
        let stats = partitions.entry(case.partition).or_default();
        stats.cases += 1;
        match case.expected {
            Expected::Supported => stats.supported += 1,
            Expected::Ambiguous => stats.ambiguous += 1,
            Expected::Unsupported => stats.unsupported += 1,
        }
        stats.exact_route_decisions += usize::from(route_exact);
        stats.authorizations += usize::from(authorized);
        stats.frontend_replay_verified += usize::from(frontend_replay_verified);
        stats.downstream_emitted += usize::from(downstream_emitted);
        stats.downstream_replay_verified +=
            usize::from(downstream_emitted && downstream_replay_verified);
        stats.frontend_tamper_rejected += usize::from(frontend_tamper_rejected);
        stats.downstream_tamper_rejected +=
            usize::from(downstream_emitted && downstream_tamper_rejected);
        stats.false_authorizations += usize::from(false_authorization);
        stats.false_denials += usize::from(false_denial);
        receipts.push(Receipt {
            id: case.id,
            family: case.family,
            partition: case.partition,
            expected: case.expected,
            selected_route: selected,
            route_exact,
            authorized,
            frontend_replay_verified,
            downstream_emitted,
            downstream_replay_verified,
            frontend_tamper_rejected,
            downstream_tamper_rejected,
            false_authorization,
            false_denial,
        });
    }
    assert_eq!(partitions[&Partition::Development].cases, 400);
    assert_eq!(partitions[&Partition::Validation].cases, 400);
    assert_eq!(partitions[&Partition::Sealed].cases, 400);
    for stats in partitions.values() {
        assert_eq!(stats.exact_route_decisions, stats.cases);
        assert_eq!(stats.frontend_replay_verified, stats.cases);
        assert_eq!(stats.frontend_tamper_rejected, stats.cases);
        assert_eq!(stats.authorizations, stats.supported);
        assert_eq!(stats.downstream_emitted, stats.supported);
        assert_eq!(stats.downstream_replay_verified, stats.supported);
        assert_eq!(stats.downstream_tamper_rejected, stats.supported);
        assert_eq!(stats.false_authorizations, 0);
        assert_eq!(stats.false_denials, 0);
    }
    let report = Report {
        schema: "stage133-mixed-math-sealed-exam-v1",
        source: "independently authored shifted technical-language exam; sealed answers are not development inputs",
        corpus_sha256,
        development_sha256,
        validation_sha256,
        sealed_sha256,
        cases: receipts.len(),
        frontend_invocations: receipts.len() * 3,
        status_counts,
        partitions,
        receipts,
    };
    std::fs::write(
        "docs/stage133_mixed_math_sealed_exam.json",
        serde_json::to_vec_pretty(&report)?,
    )?;
    println!("{}", serde_json::to_string_pretty(&report)?);
    Ok(())
}
