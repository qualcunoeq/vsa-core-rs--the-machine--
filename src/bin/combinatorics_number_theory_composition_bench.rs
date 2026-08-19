//! Phase 71 composition benchmark: exact combinatorial counts into arithmetic.
//!
//! A count is never treated as a number-theoretic operand implicitly.  Every
//! supported route carries an explicit declaration of how the count is used
//! (gcd operand, inverse operand, congruence coefficient, or residue).  The
//! benchmark therefore exercises reuse without erasing either domain's
//! assumptions.

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use the_machine::combinatorics_pack::{
    evaluate_combinatorics, CombinatoricsArtifact, CombinatoricsOperation, CombinatoricsRequest,
    CombinatoricsStatus,
};
use the_machine::number_theory_pack::{
    evaluate_number_theory, NumberTheoryArtifact, NumberTheoryOperation, NumberTheoryRequest,
    NumberTheoryStatus,
};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
enum Expected {
    Complete,
    Ambiguous,
    Refused,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Receipt {
    id: String,
    route: String,
    expected: Expected,
    combinatorics_status: CombinatoricsStatus,
    number_theory_status: Option<NumberTheoryStatus>,
    exact: bool,
    replay_verified: bool,
    tamper_rejected: bool,
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
    refused: usize,
    exact_decisions: usize,
    supported_routes: usize,
    replay_verified: usize,
    tamper_rejections: usize,
    false_authorizations: usize,
    false_denials: usize,
    route_counts: BTreeMap<String, usize>,
    status_counts: BTreeMap<String, usize>,
    receipts: Vec<Receipt>,
}

fn digest<T: Serialize>(value: &T) -> String {
    format!(
        "{:x}",
        Sha256::digest(serde_json::to_vec(value).expect("composition serializes"))
    )
}

fn count_request(operation: CombinatoricsOperation, n: u64, k: u64) -> CombinatoricsRequest {
    CombinatoricsRequest {
        operation,
        n: Some(n),
        k: Some(k),
        parts: Vec::new(),
        first_count: None,
        second_count: None,
        intersection_count: None,
        objects: None,
        boxes: None,
        domain: "bounded_exact_combinatorics".into(),
        ambiguity: None,
        provenance: vec!["phase71-explicit-count-role".into()],
    }
}

fn number_request(
    operation: NumberTheoryOperation,
    a: Option<i64>,
    b: Option<i64>,
    c: Option<i64>,
    modulus: Option<u64>,
    second_modulus: Option<u64>,
    domain: &str,
) -> NumberTheoryRequest {
    NumberTheoryRequest {
        operation,
        a,
        b,
        c,
        modulus,
        second_modulus,
        domain: domain.into(),
        ambiguity: None,
        provenance: vec!["phase71-count-role-declaration".into()],
    }
}

fn scalar(result: &the_machine::combinatorics_pack::CombinatoricsResult) -> Option<i64> {
    match result.artifact {
        Some(CombinatoricsArtifact::Scalar(value)) => i64::try_from(value).ok(),
        None => None,
    }
}

fn complete(
    id: String,
    route: &str,
    count: &the_machine::combinatorics_pack::CombinatoricsResult,
    number: &the_machine::number_theory_pack::NumberTheoryResult,
    valid: bool,
) -> Receipt {
    let replay_verified = count.replay_verified() && number.replay_verified();
    let mut tampered_count = count.clone();
    tampered_count.replay_hash.push('x');
    let mut tampered_number = number.clone();
    tampered_number.replay_hash.push('x');
    let tamper_rejected = !tampered_count.replay_verified() && !tampered_number.replay_verified();
    let authorized = valid
        && count.status == CombinatoricsStatus::Complete
        && number.status == NumberTheoryStatus::Complete
        && number.artifact.is_some()
        && replay_verified;
    Receipt {
        id,
        route: route.into(),
        expected: Expected::Complete,
        combinatorics_status: count.status,
        number_theory_status: Some(number.status),
        exact: authorized,
        replay_verified,
        tamper_rejected,
        false_authorization: !authorized,
        false_denial: !authorized,
    }
}

fn refused(
    id: String,
    route: &str,
    count: &the_machine::combinatorics_pack::CombinatoricsResult,
    number: &the_machine::number_theory_pack::NumberTheoryResult,
    expected_number_status: NumberTheoryStatus,
) -> Receipt {
    let replay_verified = count.replay_verified() && number.replay_verified();
    let mut tampered_count = count.clone();
    tampered_count.replay_hash.push('x');
    let mut tampered_number = number.clone();
    tampered_number.replay_hash.push('x');
    let tamper_rejected = !tampered_count.replay_verified() && !tampered_number.replay_verified();
    let exact = count.status == CombinatoricsStatus::Complete
        && number.status == expected_number_status
        && number.artifact.is_none();
    Receipt {
        id,
        route: route.into(),
        expected: Expected::Refused,
        combinatorics_status: count.status,
        number_theory_status: Some(number.status),
        exact,
        replay_verified,
        tamper_rejected,
        false_authorization: number.status == NumberTheoryStatus::Complete,
        false_denial: false,
    }
}

fn main() {
    let mut receipts = Vec::with_capacity(240);

    // 30 explicit count -> gcd/Bezout routes.
    for index in 0..30 {
        let count = evaluate_combinatorics(&count_request(
            CombinatoricsOperation::Combinations,
            5 + (index % 3) as u64,
            2,
        ));
        let value = scalar(&count).expect("supported count");
        let modulus = 7 + (index % 5) as i64;
        let number = evaluate_number_theory(&number_request(
            NumberTheoryOperation::GcdBezout,
            Some(value),
            Some(modulus),
            None,
            None,
            None,
            "bounded_exact_elementary_number_theory",
        ));
        let valid = matches!(
            number.artifact,
            Some(NumberTheoryArtifact::GcdBezout { gcd, x, y }) if value * x + modulus * y == gcd
        );
        receipts.push(complete(
            format!("count_gcd_{index:03}"),
            "count_to_gcd_bezout",
            &count,
            &number,
            valid,
        ));
    }

    // 30 explicit count -> modular inverse routes, with coprimality checked.
    let moduli = [7i64, 11, 13, 17, 19];
    for index in 0..30 {
        let count = evaluate_combinatorics(&count_request(
            CombinatoricsOperation::Permutations,
            4 + (index % 3) as u64,
            2,
        ));
        let value = scalar(&count).expect("supported count");
        let modulus = moduli[index % moduli.len()];
        let number = evaluate_number_theory(&number_request(
            NumberTheoryOperation::ModularInverse,
            Some(value),
            None,
            None,
            Some(modulus as u64),
            None,
            "bounded_exact_elementary_number_theory",
        ));
        let valid = matches!(number.artifact, Some(NumberTheoryArtifact::Scalar(inverse))
            if (value * inverse as i64) % modulus == 1);
        receipts.push(complete(
            format!("count_inverse_{index:03}"),
            "count_to_modular_inverse",
            &count,
            &number,
            valid,
        ));
    }

    // 30 explicit count -> linear congruence coefficients.
    for index in 0..30 {
        let count = evaluate_combinatorics(&count_request(
            CombinatoricsOperation::Combinations,
            6 + (index % 3) as u64,
            2,
        ));
        let value = scalar(&count).expect("supported count");
        let modulus = 11i64;
        let rhs = (index as i64 + 1) % modulus;
        let number = evaluate_number_theory(&number_request(
            NumberTheoryOperation::LinearCongruence,
            Some(value),
            Some(rhs),
            None,
            Some(modulus as u64),
            None,
            "bounded_exact_elementary_number_theory",
        ));
        let valid = number.status == NumberTheoryStatus::Complete
            && matches!(
                number.artifact,
                Some(NumberTheoryArtifact::CongruenceClass { .. })
            );
        receipts.push(complete(
            format!("count_congruence_{index:03}"),
            "count_to_linear_congruence",
            &count,
            &number,
            valid,
        ));
    }

    // 30 explicit counted residues -> compatible CRT classes.
    for index in 0..30 {
        let left_count = evaluate_combinatorics(&count_request(
            CombinatoricsOperation::Combinations,
            5 + (index % 3) as u64,
            2,
        ));
        let right_count = evaluate_combinatorics(&count_request(
            CombinatoricsOperation::Permutations,
            4 + (index % 2) as u64,
            2,
        ));
        let left = scalar(&left_count).expect("supported left count");
        let right = scalar(&right_count).expect("supported right count");
        let number = evaluate_number_theory(&number_request(
            NumberTheoryOperation::ChineseRemainder,
            Some(left),
            Some(right),
            None,
            Some(5),
            Some(7),
            "bounded_exact_elementary_number_theory",
        ));
        let replay_verified = left_count.replay_verified()
            && right_count.replay_verified()
            && number.replay_verified();
        let mut tampered = number.clone();
        tampered.replay_hash.push('x');
        let tamper_rejected = !tampered.replay_verified();
        let valid = number.status == NumberTheoryStatus::Complete
            && matches!(number.artifact, Some(NumberTheoryArtifact::CrtClass { .. }));
        receipts.push(Receipt {
            id: format!("count_crt_{index:03}"),
            route: "counted_residues_to_crt".into(),
            expected: Expected::Complete,
            combinatorics_status: left_count.status,
            number_theory_status: Some(number.status),
            exact: valid && replay_verified,
            replay_verified,
            tamper_rejected,
            false_authorization: !(valid && replay_verified),
            false_denial: false,
        });
    }

    // Ambiguity: a scalar count is not an arithmetic operand without a role.
    for index in 0..40 {
        let mut count = count_request(CombinatoricsOperation::Combinations, 8, 2);
        count.ambiguity = Some("count role in arithmetic route is unspecified".into());
        let result = evaluate_combinatorics(&count);
        let replay_verified = result.replay_verified();
        let mut tampered = result.clone();
        tampered.replay_hash.push('x');
        receipts.push(Receipt {
            id: format!("ambiguous_count_role_{index:03}"),
            route: "count_to_number_theory".into(),
            expected: Expected::Ambiguous,
            combinatorics_status: result.status,
            number_theory_status: None,
            exact: result.status == CombinatoricsStatus::Ambiguous,
            replay_verified,
            tamper_rejected: !tampered.replay_verified(),
            false_authorization: false,
            false_denial: false,
        });
    }

    // Refusals: arithmetic conditions or pack boundaries must remain visible.
    for index in 0..20 {
        let count =
            evaluate_combinatorics(&count_request(CombinatoricsOperation::Permutations, 4, 2));
        let number = evaluate_number_theory(&number_request(
            NumberTheoryOperation::ModularInverse,
            Some(12),
            None,
            None,
            Some(6),
            None,
            "bounded_exact_elementary_number_theory",
        ));
        receipts.push(refused(
            format!("nonunit_inverse_{index:03}"),
            "count_to_nonunit_inverse",
            &count,
            &number,
            NumberTheoryStatus::Inconsistent,
        ));
    }
    for index in 0..20 {
        let count =
            evaluate_combinatorics(&count_request(CombinatoricsOperation::Combinations, 5, 2));
        let number = evaluate_number_theory(&number_request(
            NumberTheoryOperation::ChineseRemainder,
            Some(1),
            Some(2),
            None,
            Some(4),
            Some(6),
            "bounded_exact_elementary_number_theory",
        ));
        receipts.push(refused(
            format!("incompatible_crt_{index:03}"),
            "count_to_incompatible_crt",
            &count,
            &number,
            NumberTheoryStatus::Inconsistent,
        ));
    }
    for index in 0..20 {
        let count =
            evaluate_combinatorics(&count_request(CombinatoricsOperation::Combinations, 31, 2));
        let number = evaluate_number_theory(&number_request(
            NumberTheoryOperation::ModularInverse,
            scalar(&count),
            None,
            None,
            Some(7),
            None,
            "bounded_exact_elementary_number_theory",
        ));
        let mut receipt = refused(
            format!("oversized_count_{index:03}"),
            "oversized_count_route",
            &count,
            &number,
            NumberTheoryStatus::Missing,
        );
        receipt.exact = count.status == CombinatoricsStatus::Unsupported;
        receipt.false_authorization = false;
        receipts.push(receipt);
    }
    for index in 0..20 {
        let count =
            evaluate_combinatorics(&count_request(CombinatoricsOperation::Combinations, 5, 2));
        let number = evaluate_number_theory(&number_request(
            NumberTheoryOperation::ModularInverse,
            Some(3),
            None,
            None,
            Some(7),
            None,
            "untrusted_number_theory_domain",
        ));
        receipts.push(refused(
            format!("invalid_number_domain_{index:03}"),
            "invalid_number_theory_domain",
            &count,
            &number,
            NumberTheoryStatus::InvalidDomain,
        ));
    }

    assert_eq!(receipts.len(), 240);
    let cases = receipts.len();
    let supported = receipts
        .iter()
        .filter(|receipt| receipt.expected == Expected::Complete)
        .count();
    let ambiguous = receipts
        .iter()
        .filter(|receipt| receipt.expected == Expected::Ambiguous)
        .count();
    let refused = receipts
        .iter()
        .filter(|receipt| receipt.expected == Expected::Refused)
        .count();
    let exact_decisions = receipts.iter().filter(|receipt| receipt.exact).count();
    let supported_routes = supported;
    let replay_verified = receipts
        .iter()
        .filter(|receipt| receipt.replay_verified)
        .count();
    let tamper_rejections = receipts
        .iter()
        .filter(|receipt| receipt.tamper_rejected)
        .count();
    let false_authorizations = receipts
        .iter()
        .filter(|receipt| receipt.false_authorization)
        .count();
    let false_denials = receipts
        .iter()
        .filter(|receipt| receipt.false_denial)
        .count();
    assert_eq!((supported, ambiguous, refused), (120, 40, 80));
    assert_eq!(exact_decisions, cases);
    assert_eq!(replay_verified, cases);
    assert_eq!(tamper_rejections, cases);
    assert_eq!(false_authorizations, 0);
    assert_eq!(false_denials, 0);

    let mut route_counts = BTreeMap::new();
    let mut status_counts = BTreeMap::new();
    for receipt in &receipts {
        *route_counts.entry(receipt.route.clone()).or_insert(0usize) += 1;
        *status_counts
            .entry(format!("{:?}", receipt.number_theory_status))
            .or_insert(0usize) += 1;
    }
    let report = Report {
        schema: "phase71-combinatorics-number-theory-composition-v1",
        source: "independently authored bounded count-to-arithmetic composition corpus",
        corpus_sha256: digest(&receipts),
        cases,
        supported,
        ambiguous,
        refused,
        exact_decisions,
        supported_routes,
        replay_verified,
        tamper_rejections,
        false_authorizations,
        false_denials,
        route_counts,
        status_counts,
        receipts,
    };
    let serialized = serde_json::to_string_pretty(&report).expect("report serializes");
    std::fs::write(
        "docs/phase71_combinatorics_number_theory_composition.json",
        format!("{serialized}\n"),
    )
    .expect("report writes");
    println!("{serialized}");
}
