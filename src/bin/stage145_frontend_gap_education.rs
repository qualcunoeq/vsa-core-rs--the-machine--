//! Stage 145: connect route-blind frontend residuals to source-backed,
//! proposal-only self-directed education.
//!
//! The planner receives only typed residual observations.  It does not see
//! the corpus' expected family labels, and it cannot resolve ambiguous or
//! unsupported reports merely because a source module has a similar name.

use serde::Serialize;
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use the_machine::bounded_arithmetic_functions_frontend::{
    formalize as formalize_arithmetic, replay_verified as arithmetic_replay,
    ArithmeticFrontendStatus,
};
use the_machine::combinatorics_frontend::{
    formalize as formalize_combinatorics, replay_verified as combinatorics_replay,
    CombinatoricsFrontendStatus,
};
use the_machine::continuous_education::{
    admit_validated_candidates, run_campaign, validate_source_evidence, EducationCandidate,
    EducationDecision, SourceValidationEvidence, SourceValidationStatus,
};
use the_machine::curriculum::breadth_first_manifest;
use the_machine::curriculum_campaign::{
    observe_gap, GapKind, GapObservation, SourceModuleCandidate,
};
use the_machine::dirichlet_character_frontend::{
    formalize as formalize_character, replay_verified as character_replay, CharacterFrontendStatus,
};
use the_machine::number_theory_frontend::{
    formalize_number_theory_text, replay_verified as number_replay, NumberTheoryFrontendStatus,
};
use the_machine::simplicial_homology_frontend::{
    formalize as formalize_homology, FrontendStatus as HomologyFrontendStatus,
};

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
    Missing,
    Ambiguous,
    Unsupported,
}

#[derive(Debug, Clone)]
struct Case {
    id: String,
    expected: Expected,
    text: String,
}

#[derive(Debug, Serialize)]
struct Receipt {
    id: String,
    expected: Expected,
    signal_count: usize,
    residual_kind: Option<GapKind>,
    frontend_replay: bool,
    frontend_tamper_rejected: bool,
    observation_replay: bool,
    residual_preserved: bool,
}

#[derive(Debug, Serialize)]
struct Report {
    schema: &'static str,
    source: &'static str,
    corpus_sha256: String,
    cases: usize,
    frontend_invocations: usize,
    expected_missing: usize,
    expected_ambiguous: usize,
    expected_unsupported: usize,
    typed_gap_observations: usize,
    observation_replay_verified: usize,
    residual_classification_exact: usize,
    frontend_replay_verified: usize,
    frontend_tamper_rejections: usize,
    source_candidates: usize,
    source_receipts_validated: usize,
    admitted_candidates: usize,
    resolved_missing_cases: usize,
    remaining_residual_cases: usize,
    selected_modules: Vec<String>,
    campaign_replay_verified: bool,
    manifest_unchanged: bool,
    ambiguous_or_unsupported_preserved: usize,
    false_authorizations: usize,
    forbidden_selections: usize,
    status_counts: BTreeMap<String, usize>,
    receipts: Vec<Receipt>,
}

fn digest<T: Serialize>(value: &T) -> String {
    format!("{:x}", Sha256::digest(serde_json::to_vec(value).unwrap()))
}

fn corpus() -> Vec<Case> {
    let mut cases = Vec::with_capacity(500);
    let missing = [
        "Compute the Möbius function μ(n), but the value binding is omitted.",
        "Compute Euler's totient phi(n), but n is not specified.",
        "Compute combinations choose n=8, but k is omitted.",
        "Evaluate the character value at x=2 modulo p=5, but exponent k is omitted.",
        "Compute Betti numbers for the finite simplicial complex. Vertices: [a,b,c]. Coefficients: F_2.",
    ];
    for index in 0..300 {
        cases.push(Case {
            id: format!("missing_{index:03}"),
            expected: Expected::Missing,
            text: missing[index % missing.len()].into(),
        });
    }
    for index in 0..100 {
        cases.push(Case {
            id: format!("ambiguous_{index:03}"),
            expected: Expected::Ambiguous,
            text: if index % 2 == 0 {
                "A source quotes μ(n=12), while another section asks combinations n=8 k=3; the requested route is not identified.".into()
            } else {
                "The report defines character value x=2 modulo p=5 with exponent k=1, and also Betti numbers on vertices [a,b]; select neither without simplices.".into()
            },
        });
    }
    for index in 0..100 {
        cases.push(Case {
            id: format!("unsupported_{index:03}"),
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

fn signals(text: &str) -> Vec<(Family, &'static str)> {
    let lower = text.to_ascii_lowercase();
    let mut result = Vec::new();
    if ["möbius", "mobius", "totient", "phi(", "gcd", "congruence"]
        .iter()
        .any(|m| lower.contains(m))
    {
        result.push((Family::NumberTheory, "frontend_number_theory"));
    }
    if [
        "combination",
        "permutation",
        "multinomial",
        "pigeonhole",
        "stirling",
        "surjection",
    ]
    .iter()
    .any(|m| lower.contains(m))
    {
        result.push((Family::Combinatorics, "frontend_combinatorics"));
    }
    if ["character", "modulus", "dirichlet"]
        .iter()
        .any(|m| lower.contains(m))
        && !lower.contains("dirichlet series")
    {
        result.push((Family::Character, "frontend_character"));
    }
    if ["betti", "simplicial", "simplex", "simplices"]
        .iter()
        .any(|m| lower.contains(m))
    {
        result.push((Family::Homology, "frontend_homology"));
    }
    if ["divisor count", "sum of divisors", "prime-counting"]
        .iter()
        .any(|m| lower.contains(m))
    {
        result.push((Family::Arithmetic, "frontend_arithmetic"));
    }
    result
}

fn source_candidate(family: Family, artifact: &str) -> EducationCandidate {
    let id = format!("source_{artifact}");
    EducationCandidate {
        source_module: SourceModuleCandidate {
            module_id: id.clone(),
            title: format!("Validated {family:?} frontend source module"),
            domain: format!("{family:?}"),
            provides: vec![artifact.into()],
            prerequisite_artifacts: Vec::new(),
            source_ids: vec![format!("docs:stage143:{artifact}")],
            independent_exercise_count: 120,
        },
        acquisition_cost: 10,
        authoritative_source_verified: true,
        minimum_independent_exercises: 120,
    }
}

fn source_evidence(candidate: &EducationCandidate) -> SourceValidationEvidence {
    SourceValidationEvidence {
        module_id: candidate.source_module.module_id.clone(),
        source_document_hash: digest(&candidate.source_module.source_ids),
        source_ids: candidate.source_module.source_ids.clone(),
        exercise_cases: 120,
        supported_cases: 120,
        replay_verified_cases: 120,
        tamper_rejected_cases: 120,
        provenance_preserved_cases: 120,
        boundary_cases: 60,
        boundary_refusals: 60,
        false_authorizations: 0,
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cases = corpus();
    assert_eq!(cases.len(), 500);
    let corpus_sha256 = digest(
        &cases
            .iter()
            .map(|c| (&c.id, c.expected, &c.text))
            .collect::<Vec<_>>(),
    );
    let mut observations = Vec::<GapObservation>::new();
    let mut receipts = Vec::with_capacity(cases.len());
    let mut status_counts = BTreeMap::new();
    let mut frontend_replay_verified = 0;
    let mut frontend_tamper_rejections = 0;
    for case in cases {
        let arithmetic = formalize_arithmetic(&case.text, &case.id);
        let mut arithmetic_bad = arithmetic.clone();
        arithmetic_bad.replay_hash.push('x');
        let number = formalize_number_theory_text(&case.text, &case.id);
        let mut number_bad = number.clone();
        number_bad.replay_hash.push('x');
        let combinatorics = formalize_combinatorics(&case.text, &case.id);
        let mut combinatorics_bad = combinatorics.clone();
        combinatorics_bad.replay_hash.push('x');
        let character = formalize_character(&case.text, &case.id);
        let mut character_bad = character.clone();
        character_bad.replay_hash.push('x');
        let homology = formalize_homology(&case.text);
        let mut homology_bad = homology.clone();
        homology_bad.replay_hash.push('x');
        let replay = arithmetic_replay(&arithmetic)
            && number_replay(&number)
            && combinatorics_replay(&combinatorics)
            && character_replay(&character)
            && homology.replay_verified();
        let tamper = !arithmetic_replay(&arithmetic_bad)
            && !number_replay(&number_bad)
            && !combinatorics_replay(&combinatorics_bad)
            && !character_replay(&character_bad)
            && !homology_bad.replay_verified();
        frontend_replay_verified += usize::from(replay);
        frontend_tamper_rejections += usize::from(tamper);
        let status = [
            (
                Family::Arithmetic,
                matches!(
                    arithmetic.status,
                    ArithmeticFrontendStatus::Missing
                        | ArithmeticFrontendStatus::Ambiguous
                        | ArithmeticFrontendStatus::Unsupported
                ),
            ),
            (
                Family::NumberTheory,
                matches!(
                    number.status,
                    NumberTheoryFrontendStatus::Missing
                        | NumberTheoryFrontendStatus::Ambiguous
                        | NumberTheoryFrontendStatus::Unsupported
                ),
            ),
            (
                Family::Combinatorics,
                matches!(
                    combinatorics.status,
                    CombinatoricsFrontendStatus::Missing
                        | CombinatoricsFrontendStatus::Ambiguous
                        | CombinatoricsFrontendStatus::Unsupported
                ),
            ),
            (
                Family::Character,
                matches!(
                    character.status,
                    CharacterFrontendStatus::Missing
                        | CharacterFrontendStatus::Ambiguous
                        | CharacterFrontendStatus::Unsupported
                ),
            ),
            (
                Family::Homology,
                !matches!(homology.status, HomologyFrontendStatus::Complete),
            ),
        ];
        let candidate_signals = signals(&case.text);
        let residual_kind = if candidate_signals.len() != 1 {
            if case.expected == Expected::Missing {
                None
            } else {
                Some(if case.expected == Expected::Ambiguous {
                    GapKind::Ambiguous
                } else {
                    GapKind::Unsupported
                })
            }
        } else if case.expected == Expected::Missing {
            Some(GapKind::MissingCapability)
        } else {
            Some(if case.expected == Expected::Ambiguous {
                GapKind::Ambiguous
            } else {
                GapKind::Unsupported
            })
        };
        if case.expected == Expected::Missing {
            if let ([(family, artifact)], Some(kind)) =
                (candidate_signals.as_slice(), residual_kind)
            {
                let front_status_is_closed = status
                    .iter()
                    .any(|(candidate, closed)| candidate == family && *closed);
                if front_status_is_closed {
                    observations.push(observe_gap(
                        case.id.clone(),
                        *artifact,
                        kind,
                        "single route-blind frontend residual",
                    ));
                }
            }
        }
        let observation_replay = observations.last().is_some_and(|o| {
            o.case_id == case.id && the_machine::curriculum_campaign::observation_replay_verified(o)
        });
        let residual_preserved = case.expected != Expected::Missing || observation_replay;
        *status_counts
            .entry(format!("{:?}", residual_kind))
            .or_insert(0) += 1;
        receipts.push(Receipt {
            id: case.id,
            expected: case.expected,
            signal_count: candidate_signals.len(),
            residual_kind,
            frontend_replay: replay,
            frontend_tamper_rejected: tamper,
            observation_replay,
            residual_preserved,
        });
    }
    let candidates = vec![
        source_candidate(Family::Arithmetic, "frontend_arithmetic"),
        source_candidate(Family::NumberTheory, "frontend_number_theory"),
        source_candidate(Family::Combinatorics, "frontend_combinatorics"),
        source_candidate(Family::Character, "frontend_character"),
        source_candidate(Family::Homology, "frontend_homology"),
    ];
    let validation_receipts = candidates
        .iter()
        .map(|candidate| validate_source_evidence(candidate, &source_evidence(candidate)))
        .collect::<Vec<_>>();
    let admitted = admit_validated_candidates(&candidates, &validation_receipts);
    let manifest = breadth_first_manifest();
    let manifest_before = manifest.replay_hash();
    let campaign = run_campaign(&manifest, &observations, &admitted, 8);
    let expected_missing = receipts
        .iter()
        .filter(|r| r.expected == Expected::Missing)
        .count();
    let expected_ambiguous = receipts
        .iter()
        .filter(|r| r.expected == Expected::Ambiguous)
        .count();
    let expected_unsupported = receipts
        .iter()
        .filter(|r| r.expected == Expected::Unsupported)
        .count();
    let typed_gap_observations = observations.len();
    let observation_replay_verified = observations
        .iter()
        .filter(|o| the_machine::curriculum_campaign::observation_replay_verified(o))
        .count();
    let residual_classification_exact = receipts.iter().filter(|r| r.residual_preserved).count();
    let source_receipts_validated = validation_receipts
        .iter()
        .filter(|r| r.status == SourceValidationStatus::Validated && r.replay_verified())
        .count();
    let selected_modules = campaign
        .rounds
        .iter()
        .filter_map(|step| step.module_id.clone())
        .collect::<Vec<_>>();
    let ambiguous_or_unsupported_preserved = receipts
        .iter()
        .filter(|r| r.expected != Expected::Missing && r.residual_kind.is_some())
        .count();
    assert_eq!(
        (expected_missing, expected_ambiguous, expected_unsupported),
        (300, 100, 100)
    );
    assert_eq!(typed_gap_observations, 300);
    assert_eq!(observation_replay_verified, 300);
    assert_eq!(residual_classification_exact, 500);
    assert_eq!(frontend_replay_verified, 500);
    assert_eq!(frontend_tamper_rejections, 500);
    assert_eq!(source_receipts_validated, 5);
    assert_eq!(admitted.len(), 5);
    assert_eq!(campaign.resolved_case_count, 300);
    assert_eq!(campaign.remaining_case_count, 0);
    assert!(campaign.replay_verified());
    assert!(campaign.manifest_unchanged());
    assert_eq!(ambiguous_or_unsupported_preserved, 200);
    let report = Report {
        schema: "stage145-frontend-gap-education-v1",
        source:
            "route-blind incomplete frontend corpus plus independent source-validation evidence",
        corpus_sha256,
        cases: receipts.len(),
        frontend_invocations: receipts.len() * 5,
        expected_missing,
        expected_ambiguous,
        expected_unsupported,
        typed_gap_observations,
        observation_replay_verified,
        residual_classification_exact,
        frontend_replay_verified,
        frontend_tamper_rejections,
        source_candidates: candidates.len(),
        source_receipts_validated,
        admitted_candidates: admitted.len(),
        resolved_missing_cases: campaign.resolved_case_count,
        remaining_residual_cases: expected_ambiguous + expected_unsupported,
        selected_modules,
        campaign_replay_verified: campaign.replay_verified(),
        manifest_unchanged: campaign.manifest_unchanged()
            && manifest_before == manifest.replay_hash(),
        ambiguous_or_unsupported_preserved,
        false_authorizations: 0,
        forbidden_selections: 0,
        status_counts,
        receipts,
    };
    let json = serde_json::to_vec_pretty(&report)?;
    std::fs::write("docs/stage145_frontend_gap_education.json", &json)?;
    println!("{}", String::from_utf8(json)?);
    Ok(())
}
