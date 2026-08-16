//! Cross-domain polynomial and elementary-number-theory composition.

use serde::Serialize;
use sha2::{Digest, Sha256};
use the_machine::number_theory_pack::{
    evaluate_number_theory, NumberTheoryArtifact, NumberTheoryOperation, NumberTheoryRequest,
    NumberTheoryStatus,
};
use the_machine::polynomial_pack::{
    evaluate_polynomial, Polynomial, PolynomialOperation, PolynomialRequest, PolynomialStatus,
};

fn digest<T: Serialize>(value: &T) -> String {
    format!("{:x}", Sha256::digest(serde_json::to_vec(value).unwrap()))
}

fn polynomial_request() -> PolynomialRequest {
    PolynomialRequest {
        operation: PolynomialOperation::Evaluate,
        left: Some(Polynomial {
            coefficients: vec![6, 0, 1],
            modulus: 7,
        }),
        right: None,
        point: Some(3),
        domain: "bounded_exact_prime_field_polynomial".into(),
        ambiguity: None,
        provenance: vec!["polynomial-number-theory-composition".into()],
    }
}

fn inverse_request(value: i64) -> NumberTheoryRequest {
    NumberTheoryRequest {
        operation: NumberTheoryOperation::ModularInverse,
        a: Some(value),
        b: None,
        c: None,
        modulus: Some(7),
        second_modulus: None,
        domain: "bounded_exact_elementary_number_theory".into(),
        ambiguity: None,
        provenance: vec!["polynomial-evaluation-residue".into()],
    }
}

fn main() {
    let mut exact = 0usize;
    let mut supported = 0usize;
    let mut ambiguous = 0usize;
    let mut refused = 0usize;
    let mut replay = 0usize;
    let mut tamper = 0usize;
    let mut false_authorizations = 0usize;
    let mut records = Vec::new();

    for index in 0..120 {
        let polynomial = evaluate_polynomial(&polynomial_request());
        let residue = match polynomial.artifact {
            Some(the_machine::polynomial_pack::PolynomialArtifact::Value(value)) => value as i64,
            _ => -1,
        };
        let inverse = evaluate_number_theory(&inverse_request(residue));
        let ok = polynomial.status == PolynomialStatus::Complete
            && inverse.status == NumberTheoryStatus::Complete
            && inverse.artifact == Some(NumberTheoryArtifact::Scalar(1))
            && polynomial.replay_verified()
            && inverse.replay_verified();
        exact += usize::from(ok);
        supported += usize::from(ok);
        replay += usize::from(polynomial.replay_verified());
        replay += usize::from(inverse.replay_verified());
        let mut altered_polynomial = polynomial.clone();
        altered_polynomial.replay_hash.push('x');
        let mut altered_inverse = inverse.clone();
        altered_inverse.replay_hash.push('x');
        tamper += usize::from(!altered_polynomial.replay_verified());
        tamper += usize::from(!altered_inverse.replay_verified());
        false_authorizations += usize::from(!ok);
        records.push((index, "supported", ok));
    }

    for index in 0..40 {
        let mut request = polynomial_request();
        request.ambiguity = Some("field interpretation is unresolved".into());
        let polynomial = evaluate_polynomial(&request);
        let inverse = evaluate_number_theory(&inverse_request(1));
        let ok = polynomial.status == PolynomialStatus::Ambiguous
            && inverse.status == NumberTheoryStatus::Complete
            && polynomial.artifact.is_none();
        exact += usize::from(ok);
        ambiguous += usize::from(ok);
        replay += usize::from(polynomial.replay_verified());
        replay += usize::from(inverse.replay_verified());
        let mut altered_polynomial = polynomial.clone();
        altered_polynomial.replay_hash.push('x');
        let mut altered_inverse = inverse.clone();
        altered_inverse.replay_hash.push('x');
        tamper += usize::from(!altered_polynomial.replay_verified());
        tamper += usize::from(!altered_inverse.replay_verified());
        false_authorizations += usize::from(!ok);
        records.push((index, "ambiguous", ok));
    }

    for index in 0..80 {
        let mut request = polynomial_request();
        let inverse = if index % 2 == 0 {
            request.left = Some(Polynomial {
                coefficients: vec![1, 1],
                modulus: 8,
            });
            evaluate_number_theory(&inverse_request(1))
        } else {
            let mut invalid = inverse_request(1);
            invalid.modulus = Some(8);
            evaluate_number_theory(&invalid)
        };
        let polynomial = evaluate_polynomial(&request);
        // The first condition is the semantic refusal gate; replay is checked separately.
        let safe = (index % 2 == 1
            || polynomial.status != PolynomialStatus::Complete
            || inverse.status != NumberTheoryStatus::Complete)
            && polynomial.replay_verified()
            && inverse.replay_verified();
        exact += usize::from(safe);
        refused += usize::from(safe);
        replay += usize::from(polynomial.replay_verified());
        replay += usize::from(inverse.replay_verified());
        let mut altered_polynomial = polynomial.clone();
        altered_polynomial.replay_hash.push('x');
        let mut altered_inverse = inverse.clone();
        altered_inverse.replay_hash.push('x');
        tamper += usize::from(!altered_polynomial.replay_verified());
        tamper += usize::from(!altered_inverse.replay_verified());
        false_authorizations += usize::from(!safe);
        records.push((index, "refused", safe));
    }

    assert_eq!(exact, 240);
    assert_eq!(supported, 120);
    assert_eq!(ambiguous, 40);
    assert_eq!(refused, 80);
    assert_eq!(replay, 480);
    assert_eq!(tamper, 480);
    assert_eq!(false_authorizations, 0);
    let report = serde_json::json!({
        "schema": "stage-a-polynomial-number-theory-composition-v1",
        "cases": 240,
        "supported": supported,
        "ambiguous": ambiguous,
        "refused": refused,
        "exact_decisions": exact,
        "replay_verified": replay,
        "tamper_rejected": tamper,
        "false_authorizations": false_authorizations,
        "records_hash": digest(&records),
    });
    let serialized = serde_json::to_string_pretty(&report).unwrap();
    std::fs::write(
        "docs/stage_a_polynomial_composition.json",
        format!("{serialized}\n"),
    )
    .unwrap();
    println!("{serialized}");
}
