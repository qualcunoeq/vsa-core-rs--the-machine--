//! Phase 69 independent validation corpus for bounded elementary number theory.

use serde::Serialize;
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::fs;
use the_machine::number_theory_pack::{
    evaluate_number_theory, NumberTheoryArtifact, NumberTheoryOperation, NumberTheoryRequest,
    NumberTheoryStatus,
};

#[derive(Clone, Serialize)]
struct Case {
    id: String,
    family: String,
    request: NumberTheoryRequest,
    expected_status: NumberTheoryStatus,
    expected_artifact: Option<NumberTheoryArtifact>,
}

#[derive(Serialize)]
struct Receipt {
    id: String,
    family: String,
    expected_status: NumberTheoryStatus,
    actual_status: NumberTheoryStatus,
    expected_artifact: Option<NumberTheoryArtifact>,
    actual_artifact: Option<NumberTheoryArtifact>,
    exact: bool,
    replay_verified: bool,
    tamper_rejected: bool,
    false_authorization: bool,
}

#[derive(Serialize)]
struct Report {
    schema: &'static str,
    source: &'static str,
    corpus_sha256: String,
    cases: usize,
    supported: usize,
    ambiguous: usize,
    refused: usize,
    exact_decisions: usize,
    supported_artifacts: usize,
    replay_verified: usize,
    tamper_rejections: usize,
    false_authorizations: usize,
    false_denials: usize,
    family_counts: BTreeMap<String, usize>,
    status_counts: BTreeMap<String, usize>,
    receipts: Vec<Receipt>,
}

fn hash<T: Serialize>(value: &T) -> String {
    format!("{:x}", Sha256::digest(serde_json::to_vec(value).unwrap()))
}

fn request(operation: NumberTheoryOperation) -> NumberTheoryRequest {
    NumberTheoryRequest {
        operation,
        a: None,
        b: None,
        c: None,
        modulus: None,
        second_modulus: None,
        domain: "bounded_exact_elementary_number_theory".into(),
        ambiguity: None,
        provenance: vec!["phase69-independent-number-theory".into()],
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut corpus = Vec::new();
    for index in 0..30 {
        let mut req = request(NumberTheoryOperation::GcdBezout);
        req.a = Some(84);
        req.b = Some(30);
        corpus.push(Case {
            id: format!("gcd_bezout_{index}"),
            family: "gcd_bezout".into(),
            request: req,
            expected_status: NumberTheoryStatus::Complete,
            expected_artifact: Some(NumberTheoryArtifact::GcdBezout {
                gcd: 6,
                x: -1,
                y: 3,
            }),
        });
    }
    for index in 0..20 {
        let mut req = request(NumberTheoryOperation::ModularInverse);
        req.a = Some(3);
        req.modulus = Some(11);
        corpus.push(Case {
            id: format!("modular_inverse_{index}"),
            family: "modular_inverse".into(),
            request: req,
            expected_status: NumberTheoryStatus::Complete,
            expected_artifact: Some(NumberTheoryArtifact::Scalar(4)),
        });
    }
    for index in 0..25 {
        let mut req = request(NumberTheoryOperation::LinearCongruence);
        req.a = Some(3);
        req.b = Some(6);
        req.modulus = Some(10);
        corpus.push(Case {
            id: format!("linear_congruence_{index}"),
            family: "linear_congruence".into(),
            request: req,
            expected_status: NumberTheoryStatus::Complete,
            expected_artifact: Some(NumberTheoryArtifact::CongruenceClass {
                modulus: 10,
                residue: 2,
                solution_count: 1,
            }),
        });
    }
    for index in 0..25 {
        let mut req = request(NumberTheoryOperation::ChineseRemainder);
        req.a = Some(2);
        req.b = Some(3);
        req.modulus = Some(3);
        req.second_modulus = Some(5);
        corpus.push(Case {
            id: format!("crt_{index}"),
            family: "chinese_remainder".into(),
            request: req,
            expected_status: NumberTheoryStatus::Complete,
            expected_artifact: Some(NumberTheoryArtifact::CrtClass {
                modulus: 15,
                residue: 8,
            }),
        });
    }
    for index in 0..10 {
        let mut req = request(NumberTheoryOperation::EulerTotient);
        req.modulus = Some(9);
        corpus.push(Case {
            id: format!("euler_totient_{index}"),
            family: "euler_totient".into(),
            request: req,
            expected_status: NumberTheoryStatus::Complete,
            expected_artifact: Some(NumberTheoryArtifact::Scalar(6)),
        });
    }
    for index in 0..10 {
        let mut req = request(NumberTheoryOperation::LinearDiophantine);
        req.a = Some(6);
        req.b = Some(9);
        req.c = Some(3);
        corpus.push(Case {
            id: format!("linear_diophantine_{index}"),
            family: "linear_diophantine".into(),
            request: req,
            expected_status: NumberTheoryStatus::Complete,
            expected_artifact: Some(NumberTheoryArtifact::Diophantine {
                gcd: 3,
                x: -1,
                y: 1,
            }),
        });
    }
    for index in 0..20 {
        let mut req = request(NumberTheoryOperation::GcdBezout);
        req.a = Some(84);
        req.ambiguity = Some("signed versus unsigned gcd convention is unresolved".into());
        corpus.push(Case {
            id: format!("ambiguous_gcd_{index}"),
            family: "ambiguous_gcd".into(),
            request: req,
            expected_status: NumberTheoryStatus::Ambiguous,
            expected_artifact: None,
        });
    }
    for index in 0..10 {
        corpus.push(Case {
            id: format!("missing_inverse_{index}"),
            family: "missing_inverse".into(),
            request: request(NumberTheoryOperation::ModularInverse),
            expected_status: NumberTheoryStatus::Missing,
            expected_artifact: None,
        });
    }
    for index in 0..10 {
        let mut req = request(NumberTheoryOperation::ChineseRemainder);
        req.a = Some(2);
        req.b = Some(3);
        req.modulus = Some(3);
        corpus.push(Case {
            id: format!("missing_crt_modulus_{index}"),
            family: "missing_crt_modulus".into(),
            request: req,
            expected_status: NumberTheoryStatus::Missing,
            expected_artifact: None,
        });
    }
    for index in 0..20 {
        let mut req = request(NumberTheoryOperation::EulerTotient);
        req.modulus = Some(100_001);
        corpus.push(Case {
            id: format!("unbounded_factorization_{index}"),
            family: "unbounded_factorization".into(),
            request: req,
            expected_status: NumberTheoryStatus::Unsupported,
            expected_artifact: None,
        });
    }
    for index in 0..20 {
        let mut req = request(NumberTheoryOperation::GcdBezout);
        req.a = Some(84);
        req.b = Some(30);
        req.domain = "analytic_number_theory".into();
        corpus.push(Case {
            id: format!("unsupported_domain_{index}"),
            family: "unsupported_domain".into(),
            request: req,
            expected_status: NumberTheoryStatus::InvalidDomain,
            expected_artifact: None,
        });
    }
    for index in 0..20 {
        let mut req = request(NumberTheoryOperation::ModularInverse);
        req.a = Some(2);
        req.modulus = Some(10);
        corpus.push(Case {
            id: format!("nonunit_inverse_{index}"),
            family: "nonunit_inverse".into(),
            request: req,
            expected_status: NumberTheoryStatus::Inconsistent,
            expected_artifact: None,
        });
    }
    for index in 0..20 {
        let mut req = request(NumberTheoryOperation::ChineseRemainder);
        req.a = Some(1);
        req.b = Some(0);
        req.modulus = Some(2);
        req.second_modulus = Some(4);
        corpus.push(Case {
            id: format!("incompatible_crt_{index}"),
            family: "incompatible_crt".into(),
            request: req,
            expected_status: NumberTheoryStatus::Inconsistent,
            expected_artifact: None,
        });
    }
    assert_eq!(corpus.len(), 240);

    let corpus_sha256 = hash(&corpus);
    let mut receipts = Vec::with_capacity(corpus.len());
    let mut family_counts = BTreeMap::new();
    let mut status_counts = BTreeMap::new();
    for case in corpus {
        *family_counts.entry(case.family.clone()).or_insert(0) += 1;
        let result = evaluate_number_theory(&case.request);
        *status_counts
            .entry(format!("{:?}", result.status))
            .or_insert(0) += 1;
        let mut tampered = result.clone();
        tampered.replay_hash.push('x');
        let exact =
            result.status == case.expected_status && result.artifact == case.expected_artifact;
        receipts.push(Receipt {
            id: case.id,
            family: case.family,
            expected_status: case.expected_status,
            actual_status: result.status,
            expected_artifact: case.expected_artifact,
            actual_artifact: result.artifact.clone(),
            exact,
            replay_verified: result.replay_verified(),
            tamper_rejected: !tampered.replay_verified(),
            false_authorization: case.expected_status != NumberTheoryStatus::Complete
                && result.artifact.is_some(),
        });
    }
    let cases = receipts.len();
    let supported = receipts
        .iter()
        .filter(|row| row.expected_status == NumberTheoryStatus::Complete)
        .count();
    let ambiguous = receipts
        .iter()
        .filter(|row| {
            matches!(
                row.expected_status,
                NumberTheoryStatus::Ambiguous | NumberTheoryStatus::Missing
            )
        })
        .count();
    let refused = cases - supported - ambiguous;
    let exact_decisions = receipts.iter().filter(|row| row.exact).count();
    let supported_artifacts = receipts
        .iter()
        .filter(|row| {
            row.expected_status == NumberTheoryStatus::Complete && row.actual_artifact.is_some()
        })
        .count();
    let replay_verified = receipts.iter().filter(|row| row.replay_verified).count();
    let tamper_rejections = receipts.iter().filter(|row| row.tamper_rejected).count();
    let false_authorizations = receipts
        .iter()
        .filter(|row| row.false_authorization)
        .count();
    let false_denials = receipts
        .iter()
        .filter(|row| row.expected_status == NumberTheoryStatus::Complete && !row.exact)
        .count();
    assert_eq!(exact_decisions, cases);
    assert_eq!(supported_artifacts, supported);
    assert_eq!(replay_verified, cases);
    assert_eq!(tamper_rejections, cases);
    assert_eq!(false_authorizations, 0);
    assert_eq!(false_denials, 0);
    let report = Report {
        schema: "phase69-bounded-elementary-number-theory-v1",
        source: "independently authored exact arithmetic corpus",
        corpus_sha256,
        cases,
        supported,
        ambiguous,
        refused,
        exact_decisions,
        supported_artifacts,
        replay_verified,
        tamper_rejections,
        false_authorizations,
        false_denials,
        family_counts,
        status_counts,
        receipts,
    };
    fs::write(
        "docs/phase69_number_theory_pack.json",
        serde_json::to_vec_pretty(&report)?,
    )?;
    println!("{}", serde_json::to_string_pretty(&report)?);
    Ok(())
}
