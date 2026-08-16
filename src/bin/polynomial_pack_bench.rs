//! Independent validation campaign for bounded prime-field polynomial algebra.

use serde::Serialize;
use sha2::{Digest, Sha256};
use the_machine::polynomial_pack::{
    evaluate_polynomial, Polynomial, PolynomialOperation, PolynomialRequest, PolynomialStatus,
};

fn digest<T: Serialize>(value: &T) -> String {
    format!("{:x}", Sha256::digest(serde_json::to_vec(value).unwrap()))
}

fn polynomial(coefficients: Vec<u64>) -> Polynomial {
    Polynomial {
        coefficients,
        modulus: 7,
    }
}

fn request(operation: PolynomialOperation) -> PolynomialRequest {
    PolynomialRequest {
        operation,
        left: Some(polynomial(vec![6, 0, 1])), // x² - 1 over F₇
        right: Some(polynomial(vec![6, 1])),   // x - 1 over F₇
        point: Some(3),
        domain: "bounded_exact_prime_field_polynomial".into(),
        ambiguity: None,
        provenance: vec!["independent-polynomial-corpus".into()],
    }
}

fn main() {
    let operations = [
        PolynomialOperation::Add,
        PolynomialOperation::Multiply,
        PolynomialOperation::Divide,
        PolynomialOperation::Gcd,
        PolynomialOperation::Evaluate,
        PolynomialOperation::Roots,
        PolynomialOperation::FactorQuadratic,
    ];
    let mut exact = 0usize;
    let mut supported = 0usize;
    let mut ambiguous = 0usize;
    let mut refused = 0usize;
    let mut replay = 0usize;
    let mut tamper = 0usize;
    let mut false_authorizations = 0usize;
    let mut records = Vec::new();

    for index in 0..120 {
        let result = evaluate_polynomial(&request(operations[index % operations.len()]));
        let ok = result.status == PolynomialStatus::Complete
            && result.artifact.is_some()
            && result.replay_verified();
        exact += usize::from(ok);
        supported += usize::from(ok);
        replay += usize::from(result.replay_verified());
        let mut altered = result.clone();
        altered.replay_hash.push('x');
        tamper += usize::from(!altered.replay_verified());
        false_authorizations += usize::from(!ok);
        records.push((index, "supported", ok));
    }

    for index in 0..40 {
        let mut req = request(PolynomialOperation::Add);
        req.ambiguity = Some("coefficient domain or polynomial orientation is unresolved".into());
        let result = evaluate_polynomial(&req);
        let ok = result.status == PolynomialStatus::Ambiguous
            && result.artifact.is_none()
            && result.replay_verified();
        exact += usize::from(ok);
        ambiguous += usize::from(ok);
        replay += usize::from(result.replay_verified());
        let mut altered = result.clone();
        altered.replay_hash.push('x');
        tamper += usize::from(!altered.replay_verified());
        false_authorizations += usize::from(!ok);
        records.push((index, "ambiguous", ok));
    }

    for index in 0..80 {
        let mut req = request(if index % 2 == 0 {
            PolynomialOperation::MinimalPolynomial
        } else {
            PolynomialOperation::Add
        });
        if index % 2 == 0 {
            req.domain = "unsupported_operator_domain".into();
        } else {
            req.left = Some(Polynomial {
                coefficients: vec![1, 2, 3, 4, 5, 6, 1, 2, 3, 4],
                modulus: 7,
            });
        }
        let result = evaluate_polynomial(&req);
        let ok = result.status != PolynomialStatus::Complete
            && result.artifact.is_none()
            && result.replay_verified();
        exact += usize::from(ok);
        refused += usize::from(ok);
        replay += usize::from(result.replay_verified());
        let mut altered = result.clone();
        altered.replay_hash.push('x');
        tamper += usize::from(!altered.replay_verified());
        false_authorizations += usize::from(!ok);
        records.push((index, "refused", ok));
    }

    assert_eq!(exact, 240);
    assert_eq!(supported, 120);
    assert_eq!(ambiguous, 40);
    assert_eq!(refused, 80);
    assert_eq!(replay, 240);
    assert_eq!(tamper, 240);
    assert_eq!(false_authorizations, 0);
    let report = serde_json::json!({
        "schema": "stage-a-polynomial-algebra-v1",
        "cases": 240,
        "supported": supported,
        "ambiguous": ambiguous,
        "refused": refused,
        "exact_decisions": exact,
        "replay_verified": replay,
        "tamper_rejected": tamper,
        "false_authorizations": false_authorizations,
        "record_hash": digest(&records),
    });
    let serialized = serde_json::to_string_pretty(&report).unwrap();
    std::fs::write(
        "docs/stage_a_polynomial_algebra.json",
        format!("{serialized}\n"),
    )
    .unwrap();
    println!("{serialized}");
}
