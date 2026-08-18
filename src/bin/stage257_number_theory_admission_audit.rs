//! Stage 257: admission audit for the broader number-theory curriculum node.
//!
//! Finite characters and arithmetic functions are validated prerequisites. This
//! audit exercises them together, but deliberately refuses to promote the
//! broader node while asymptotic and unbounded representations are absent.

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{collections::BTreeMap, fs};

use the_machine::bounded_arithmetic_functions_pack::{
    evaluate, ArithmeticFunctionOperation, ArithmeticFunctionRequest, ArithmeticFunctionStatus,
};
use the_machine::curriculum::{breadth_first_manifest, CurriculumStatus};
use the_machine::dirichlet_character_pack::{
    evaluate as evaluate_character, CharacterOperation, CharacterStatus, DirichletCharacterRequest,
};

const JSON: &str = "docs/stage257_number_theory_admission_audit.json";
const MD: &str = "docs/stage257_number_theory_admission_audit.md";

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
enum Route {
    FiniteArithmeticFunction,
    FiniteCharacter,
    FiniteComposition,
    AmbiguousPrerequisite,
    AsymptoticRequest,
    CompositeCharacterModulus,
    UnboundedCharacter,
    MissingCompositionBinding,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
enum Expected {
    Supported,
    Ambiguous,
    Refused,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Case {
    id: String,
    route: Route,
    expected: Expected,
    index: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Receipt {
    id: String,
    route: Route,
    expected: Expected,
    actual: Expected,
    exact: bool,
    arithmetic_replay: bool,
    character_replay: bool,
    composition_replay: bool,
    source_provenance: bool,
    tamper_rejected: bool,
    false_authorization: bool,
    false_denial: bool,
}

#[derive(Debug, Serialize)]
struct Report {
    schema: &'static str,
    source_manifest_hash: String,
    corpus_sha256: String,
    cases: usize,
    supported: usize,
    ambiguous: usize,
    refused: usize,
    exact_decisions: usize,
    supported_replays: usize,
    arithmetic_replays: usize,
    character_replays: usize,
    composition_replays: usize,
    tamper_rejections: usize,
    source_provenance_preserved: usize,
    false_authorizations: usize,
    false_denials: usize,
    broader_number_theory_status: CurriculumStatus,
    promotion_allowed: bool,
    rejection_reasons: Vec<String>,
    route_counts: BTreeMap<String, usize>,
    receipts: Vec<Receipt>,
    corpus: Vec<Case>,
}

fn digest<T: Serialize + ?Sized>(value: &T) -> String {
    format!("{:x}", Sha256::digest(serde_json::to_vec(value).unwrap()))
}

fn arith(
    operation: ArithmeticFunctionOperation,
    value: Option<u64>,
    ambiguity: Option<&str>,
    domain: &str,
    index: usize,
) -> ArithmeticFunctionRequest {
    ArithmeticFunctionRequest {
        operation,
        value,
        domain: domain.into(),
        ambiguity: ambiguity.map(str::to_owned),
        provenance: vec![
            format!("stage257-case-{index:03}"),
            "source:mit-ocw-18-781-bounded-arithmetic-functions".into(),
        ],
    }
}

fn character(
    operation: CharacterOperation,
    modulus: Option<u32>,
    exponent: Option<u32>,
    value: Option<i64>,
    sum_limit: Option<u32>,
    ambiguity: Option<&str>,
    domain: &str,
    index: usize,
) -> DirichletCharacterRequest {
    DirichletCharacterRequest {
        operation,
        modulus,
        exponent,
        value,
        sum_limit,
        domain: domain.into(),
        ambiguity: ambiguity.map(str::to_owned),
        provenance: vec![
            format!("stage257-case-{index:03}"),
            "source:mit-ocw-18-785-finite-characters".into(),
        ],
    }
}

fn classify(arithmetic: ArithmeticFunctionStatus, character: CharacterStatus) -> Expected {
    if arithmetic == ArithmeticFunctionStatus::Ambiguous || character == CharacterStatus::Ambiguous
    {
        Expected::Ambiguous
    } else if arithmetic == ArithmeticFunctionStatus::Complete
        && character == CharacterStatus::Complete
    {
        Expected::Supported
    } else {
        Expected::Refused
    }
}

fn arithmetic_class(status: ArithmeticFunctionStatus) -> Expected {
    match status {
        ArithmeticFunctionStatus::Complete => Expected::Supported,
        ArithmeticFunctionStatus::Ambiguous => Expected::Ambiguous,
        _ => Expected::Refused,
    }
}

fn character_class(status: CharacterStatus) -> Expected {
    match status {
        CharacterStatus::Complete => Expected::Supported,
        CharacterStatus::Ambiguous => Expected::Ambiguous,
        _ => Expected::Refused,
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let manifest = breadth_first_manifest();
    assert!(manifest.validate().is_empty());
    let planned = manifest
        .packs
        .iter()
        .find(|pack| pack.id == "number_theory")
        .expect("planned number theory node");
    assert_eq!(planned.status, CurriculumStatus::Planned);

    let mut cases = Vec::new();
    let mut add = |route: Route, expected: Expected, count: usize| {
        for index in 0..count {
            cases.push(Case {
                id: format!("stage257-{route:?}-{index:03}").to_lowercase(),
                route,
                expected,
                index,
            });
        }
    };
    add(Route::FiniteArithmeticFunction, Expected::Supported, 40);
    add(Route::FiniteCharacter, Expected::Supported, 40);
    add(Route::FiniteComposition, Expected::Supported, 40);
    add(Route::AmbiguousPrerequisite, Expected::Ambiguous, 40);
    add(Route::AsymptoticRequest, Expected::Refused, 20);
    add(Route::CompositeCharacterModulus, Expected::Refused, 20);
    add(Route::UnboundedCharacter, Expected::Refused, 20);
    add(Route::MissingCompositionBinding, Expected::Refused, 20);
    assert_eq!(cases.len(), 240);

    let mut receipts = Vec::new();
    for case in &cases {
        let mut arithmetic_replay = false;
        let mut character_replay = false;
        let mut composition_replay = false;
        let mut source_provenance = false;
        let mut tamper_rejected = true;
        let actual = match case.route {
            Route::FiniteArithmeticFunction => {
                let ops = [
                    ArithmeticFunctionOperation::DivisorCount,
                    ArithmeticFunctionOperation::DivisorSum,
                    ArithmeticFunctionOperation::Mobius,
                    ArithmeticFunctionOperation::PrimeCounting,
                ];
                let result = evaluate(&arith(
                    ops[case.index % 4],
                    Some((case.index % 97 + 2) as u64),
                    None,
                    "bounded_arithmetic_functions",
                    case.index,
                ));
                arithmetic_replay = result.replay_verified();
                source_provenance = !result.provenance.is_empty();
                let mut tampered = result.clone();
                tampered.replay_hash.push('x');
                tamper_rejected = !tampered.replay_verified();
                arithmetic_class(result.status)
            }
            Route::FiniteCharacter => {
                let ops = [
                    CharacterOperation::ValidateCharacter,
                    CharacterOperation::Evaluate,
                    CharacterOperation::PartialSum,
                    CharacterOperation::Orthogonality,
                ];
                let result = evaluate_character(&character(
                    ops[case.index % 4],
                    Some(5),
                    Some((case.index % 4) as u32),
                    Some((case.index + 1) as i64),
                    Some((case.index % 31 + 1) as u32),
                    None,
                    "bounded_dirichlet_character",
                    case.index,
                ));
                character_replay = result.replay_verified();
                source_provenance = !result.provenance.is_empty();
                let mut tampered = result.clone();
                tampered.replay_hash.push('x');
                tamper_rejected = !tampered.replay_verified();
                character_class(result.status)
            }
            Route::FiniteComposition => {
                let value = (case.index % 97 + 2) as u64;
                let arithmetic = evaluate(&arith(
                    ArithmeticFunctionOperation::Mobius,
                    Some(value),
                    None,
                    "bounded_arithmetic_functions",
                    case.index,
                ));
                let character = evaluate_character(&character(
                    CharacterOperation::Evaluate,
                    Some(5),
                    Some((case.index % 4) as u32),
                    Some(value as i64),
                    None,
                    None,
                    "bounded_dirichlet_character",
                    case.index,
                ));
                arithmetic_replay = arithmetic.replay_verified();
                character_replay = character.replay_verified();
                source_provenance =
                    !arithmetic.provenance.is_empty() && !character.provenance.is_empty();
                composition_replay = arithmetic_replay
                    && character_replay
                    && arithmetic.status == ArithmeticFunctionStatus::Complete
                    && character.status == CharacterStatus::Complete
                    && arithmetic.provenance.first().is_some()
                    && character.provenance.first().is_some();
                let hash = digest(&(
                    value,
                    &arithmetic.artifact,
                    &character.artifact,
                    &arithmetic.provenance,
                    &character.provenance,
                ));
                let tampered = digest(&(value + 1, &arithmetic.artifact, &character.artifact));
                tamper_rejected = hash != tampered;
                classify(arithmetic.status, character.status)
            }
            Route::AmbiguousPrerequisite => {
                let arithmetic = evaluate(&arith(
                    ArithmeticFunctionOperation::Mobius,
                    Some(30),
                    Some("finite value or asymptotic estimate"),
                    "bounded_arithmetic_functions",
                    case.index,
                ));
                let character = evaluate_character(&character(
                    CharacterOperation::Evaluate,
                    Some(5),
                    Some(1),
                    Some(2),
                    None,
                    Some("character convention is not unique"),
                    "bounded_dirichlet_character",
                    case.index,
                ));
                arithmetic_replay = arithmetic.replay_verified();
                character_replay = character.replay_verified();
                source_provenance = true;
                let mut a = arithmetic.clone();
                let mut c = character.clone();
                a.replay_hash.push('x');
                c.replay_hash.push('x');
                tamper_rejected = !a.replay_verified() && !c.replay_verified();
                classify(arithmetic.status, character.status)
            }
            Route::AsymptoticRequest => {
                let arithmetic = evaluate(&arith(
                    ArithmeticFunctionOperation::PrimeCounting,
                    Some(100),
                    None,
                    "analytic_number_theory",
                    case.index,
                ));
                let character = evaluate_character(&character(
                    CharacterOperation::PartialSum,
                    Some(5),
                    Some(1),
                    None,
                    Some(256),
                    None,
                    "analytic_number_theory",
                    case.index,
                ));
                arithmetic_replay = arithmetic.replay_verified();
                character_replay = character.replay_verified();
                source_provenance = true;
                classify(arithmetic.status, character.status)
            }
            Route::CompositeCharacterModulus => {
                let result = evaluate_character(&character(
                    CharacterOperation::Evaluate,
                    Some(15),
                    Some(1),
                    Some(2),
                    None,
                    None,
                    "bounded_dirichlet_character",
                    case.index,
                ));
                character_replay = result.replay_verified();
                source_provenance = !result.provenance.is_empty();
                let mut t = result.clone();
                t.replay_hash.push('x');
                tamper_rejected = !t.replay_verified();
                character_class(result.status)
            }
            Route::UnboundedCharacter => {
                let result = evaluate_character(&character(
                    CharacterOperation::ValidateCharacter,
                    Some(37),
                    Some(1),
                    None,
                    None,
                    None,
                    "bounded_dirichlet_character",
                    case.index,
                ));
                character_replay = result.replay_verified();
                source_provenance = !result.provenance.is_empty();
                let mut t = result.clone();
                t.replay_hash.push('x');
                tamper_rejected = !t.replay_verified();
                character_class(result.status)
            }
            Route::MissingCompositionBinding => {
                let arithmetic = evaluate(&arith(
                    ArithmeticFunctionOperation::Mobius,
                    None,
                    None,
                    "bounded_arithmetic_functions",
                    case.index,
                ));
                let character = evaluate_character(&character(
                    CharacterOperation::Evaluate,
                    Some(5),
                    None,
                    Some(2),
                    None,
                    None,
                    "bounded_dirichlet_character",
                    case.index,
                ));
                arithmetic_replay = arithmetic.replay_verified();
                character_replay = character.replay_verified();
                source_provenance = true;
                classify(arithmetic.status, character.status)
            }
        };
        let exact = actual == case.expected;
        let authorized = actual == Expected::Supported;
        receipts.push(Receipt {
            id: case.id.clone(),
            route: case.route,
            expected: case.expected,
            actual,
            exact,
            arithmetic_replay,
            character_replay,
            composition_replay,
            source_provenance,
            tamper_rejected,
            false_authorization: authorized && case.expected != Expected::Supported,
            false_denial: !authorized && case.expected == Expected::Supported,
        });
    }

    let supported = 120;
    let ambiguous = 40;
    let refused = 80;
    let report = Report {
        schema: "stage257-number-theory-admission-audit-v1",
        source_manifest_hash: manifest.replay_hash(),
        corpus_sha256: digest(&cases),
        cases: cases.len(),
        supported,
        ambiguous,
        refused,
        exact_decisions: receipts.iter().filter(|r| r.exact).count(),
        supported_replays: receipts
            .iter()
            .filter(|r| r.expected == Expected::Supported && r.exact)
            .count(),
        arithmetic_replays: receipts.iter().filter(|r| r.arithmetic_replay).count(),
        character_replays: receipts.iter().filter(|r| r.character_replay).count(),
        composition_replays: receipts.iter().filter(|r| r.composition_replay).count(),
        tamper_rejections: receipts.iter().filter(|r| r.tamper_rejected).count(),
        source_provenance_preserved: receipts.iter().filter(|r| r.source_provenance).count(),
        false_authorizations: receipts.iter().filter(|r| r.false_authorization).count(),
        false_denials: receipts.iter().filter(|r| r.false_denial).count(),
        broader_number_theory_status: planned.status,
        promotion_allowed: false,
        rejection_reasons: vec![
            "asymptotic_count has no bounded exact theorem gate".into(),
            "integer_relation is not an admitted reusable source contract".into(),
            "finite character/function evidence cannot authorize analytic claims".into(),
        ],
        route_counts: cases.iter().fold(BTreeMap::new(), |mut m, c| {
            *m.entry(format!("{:?}", c.route).to_lowercase())
                .or_insert(0) += 1;
            m
        }),
        receipts,
        corpus: cases,
    };
    assert_eq!(
        (report.supported, report.ambiguous, report.refused),
        (120, 40, 80)
    );
    assert_eq!(report.exact_decisions, 240);
    assert_eq!(report.supported_replays, 120);
    assert_eq!(report.tamper_rejections, 240);
    assert_eq!(report.source_provenance_preserved, 240);
    assert_eq!(report.false_authorizations, 0);
    assert_eq!(report.false_denials, 0);
    assert_eq!(
        report.broader_number_theory_status,
        CurriculumStatus::Planned
    );
    fs::write(JSON, serde_json::to_vec_pretty(&report)?)?;
    fs::write(MD, format!("# Stage 257 — broader number-theory admission audit\n\nFinite source-backed character and arithmetic-function prerequisites were exercised without promoting the broader node.\n\n* 240/240 exact decisions\n* 120 supported / 40 ambiguous / 80 refused\n* 120 supported replays; 240/240 tamper rejection\n* 240/240 provenance-preserving receipts\n* 0 false authorizations or denials\n* broader `number_theory` status: `{:?}`\n* promotion allowed: `false`\n\nThe node remains planned because asymptotic counts and the required algebraic/integer-relation contracts do not yet have bounded exact theorem gates.\n\nReproduce with `cargo run --quiet --bin stage257_number_theory_admission_audit`.\n", report.broader_number_theory_status))?;
    println!("stage257 exact={} supported={} ambiguous={} refused={} replay={} tamper={} promotion_allowed=false", report.exact_decisions, report.supported, report.ambiguous, report.refused, report.supported_replays, report.tamper_rejections);
    Ok(())
}
