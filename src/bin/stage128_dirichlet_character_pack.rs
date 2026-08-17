//! Stage 128: bounded finite Dirichlet-character curriculum pack.

use serde::Serialize;
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use the_machine::dirichlet_character_pack::{
    evaluate, CharacterOperation, CharacterStatus, DirichletCharacterRequest,
};

const SOURCE: &str =
    include_str!("../../docs/sources/mit_analytic_number_theory_character_definition.txt");

#[derive(Debug, Clone, Serialize)]
struct Case {
    id: String,
    expected: CharacterStatus,
    request: DirichletCharacterRequest,
}

#[derive(Debug, Serialize)]
struct Report {
    schema: &'static str,
    source_sha256: String,
    corpus_sha256: String,
    cases: usize,
    supported: usize,
    ambiguous: usize,
    refused: usize,
    exact_decisions: usize,
    supported_artifacts: usize,
    replay_verified: usize,
    tamper_rejected: usize,
    false_authorizations: usize,
    false_denials: usize,
    statuses: BTreeMap<String, usize>,
}

fn digest<T: serde::Serialize>(value: &T) -> String {
    format!("{:x}", Sha256::digest(serde_json::to_vec(value).unwrap()))
}

fn request(
    operation: CharacterOperation,
    modulus: Option<u32>,
    exponent: Option<u32>,
    value: Option<i64>,
    sum_limit: Option<u32>,
    ambiguity: Option<&str>,
) -> DirichletCharacterRequest {
    DirichletCharacterRequest {
        operation,
        modulus,
        exponent,
        value,
        sum_limit,
        domain: "bounded_dirichlet_character".into(),
        ambiguity: ambiguity.map(str::to_owned),
        provenance: vec!["stage128-independent-character-corpus".into()],
    }
}

fn corpus() -> Vec<Case> {
    let mut cases = Vec::new();
    for index in 0..30 {
        cases.push(Case {
            id: format!("validate-{index:03}"),
            expected: CharacterStatus::Complete,
            request: request(
                CharacterOperation::ValidateCharacter,
                Some(if index % 2 == 0 { 5 } else { 7 }),
                Some(index % if index % 2 == 0 { 4 } else { 6 }),
                None,
                None,
                None,
            ),
        });
    }
    for index in 0..30 {
        cases.push(Case {
            id: format!("evaluate-{index:03}"),
            expected: CharacterStatus::Complete,
            request: request(
                CharacterOperation::Evaluate,
                Some(5),
                Some(index % 4),
                Some(index as i64 - 12),
                None,
                None,
            ),
        });
    }
    for index in 0..30 {
        cases.push(Case {
            id: format!("partial-sum-{index:03}"),
            expected: CharacterStatus::Complete,
            request: request(
                CharacterOperation::PartialSum,
                Some(7),
                Some(index % 6),
                None,
                Some(1 + index as u32),
                None,
            ),
        });
    }
    for index in 0..30 {
        cases.push(Case {
            id: format!("orthogonality-{index:03}"),
            expected: CharacterStatus::Complete,
            request: request(
                CharacterOperation::Orthogonality,
                Some(if index % 2 == 0 { 5 } else { 7 }),
                Some(if index % 2 == 0 { index % 4 } else { index % 6 }),
                None,
                None,
                None,
            ),
        });
    }
    for index in 0..40 {
        cases.push(Case {
            id: format!("ambiguous-{index:03}"),
            expected: CharacterStatus::Ambiguous,
            request: request(
                CharacterOperation::Evaluate,
                Some(5),
                Some(1),
                Some(2),
                None,
                Some("the character convention is not uniquely specified"),
            ),
        });
    }
    for index in 0..80 {
        let (modulus, exponent, sum_limit, expected) = match index % 4 {
            0 => (Some(9), Some(1), None, CharacterStatus::Unsupported),
            1 => (Some(37), Some(1), None, CharacterStatus::Unsupported),
            2 => (Some(5), Some(4), None, CharacterStatus::Inconsistent),
            _ => (Some(5), Some(1), Some(300), CharacterStatus::Unsupported),
        };
        cases.push(Case {
            id: format!("refused-{index:03}"),
            expected,
            request: request(
                if index % 4 == 3 {
                    CharacterOperation::PartialSum
                } else {
                    CharacterOperation::ValidateCharacter
                },
                modulus,
                exponent,
                None,
                sum_limit,
                None,
            ),
        });
    }
    cases
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cases = corpus();
    let mut exact_decisions = 0;
    let mut supported_artifacts = 0;
    let mut replay_verified = 0;
    let mut tamper_rejected = 0;
    let mut false_authorizations = 0;
    let mut false_denials = 0;
    let mut statuses = BTreeMap::new();
    for case in &cases {
        let output = evaluate(&case.request);
        *statuses
            .entry(format!("{:?}", output.status).to_ascii_lowercase())
            .or_insert(0usize) += 1;
        if output.status == case.expected {
            exact_decisions += 1;
        }
        if case.expected == CharacterStatus::Complete && output.authorized() {
            supported_artifacts += 1;
        }
        if output.replay_verified() {
            replay_verified += 1;
        }
        let mut tampered = output.clone();
        tampered.replay_hash = "tampered".into();
        if !tampered.replay_verified() {
            tamper_rejected += 1;
        }
        if case.expected != CharacterStatus::Complete && output.authorized() {
            false_authorizations += 1;
        }
        if case.expected == CharacterStatus::Complete && !output.authorized() {
            false_denials += 1;
        }
    }
    let report = Report {
        schema: "stage128-dirichlet-character-pack-v1",
        source_sha256: digest(&SOURCE),
        corpus_sha256: digest(&cases),
        cases: cases.len(),
        supported: 120,
        ambiguous: 40,
        refused: 80,
        exact_decisions,
        supported_artifacts,
        replay_verified,
        tamper_rejected,
        false_authorizations,
        false_denials,
        statuses,
    };
    assert_eq!(report.cases, 240);
    assert_eq!(report.exact_decisions, 240);
    assert_eq!(report.supported_artifacts, 120);
    assert_eq!(report.replay_verified, 240);
    assert_eq!(report.tamper_rejected, 240);
    assert_eq!(report.false_authorizations, 0);
    assert_eq!(report.false_denials, 0);
    println!("{}", serde_json::to_string_pretty(&report)?);
    Ok(())
}
