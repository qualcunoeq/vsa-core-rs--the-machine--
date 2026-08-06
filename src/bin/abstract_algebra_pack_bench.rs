//! Phase 67 independent validation corpus for bounded abstract algebra.

use serde::Serialize;
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::fs;
use the_machine::abstract_algebra_pack::{
    evaluate_abstract_algebra, AbstractAlgebraArtifact, AbstractAlgebraOperation,
    AbstractAlgebraRequest, AbstractAlgebraStatus,
};

#[derive(Clone, Serialize)]
struct Case {
    id: String,
    family: String,
    request: AbstractAlgebraRequest,
    expected_status: AbstractAlgebraStatus,
    expected_artifact: Option<AbstractAlgebraArtifact>,
}

#[derive(Serialize)]
struct Receipt {
    id: String,
    family: String,
    expected_status: AbstractAlgebraStatus,
    actual_status: AbstractAlgebraStatus,
    expected_artifact: Option<AbstractAlgebraArtifact>,
    actual_artifact: Option<AbstractAlgebraArtifact>,
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
    unsupported: usize,
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

fn request(operation: AbstractAlgebraOperation) -> AbstractAlgebraRequest {
    AbstractAlgebraRequest {
        operation,
        modulus: None,
        source_modulus: None,
        target_modulus: None,
        element: None,
        multiplier: None,
        domain: "finite_exact_abstract_algebra".into(),
        assumptions: vec!["finite exact structure stated".into()],
        ambiguity: None,
        provenance: vec!["phase67-independent-abstract-algebra".into()],
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut corpus = Vec::new();
    for index in 0..30 {
        let modulus = 2 + (index as u32 % 30);
        let mut req = request(AbstractAlgebraOperation::ConstructCyclicGroup);
        req.modulus = Some(modulus);
        corpus.push(Case {
            id: format!("cyclic_group_{index}"),
            family: "cyclic_group".into(),
            request: req,
            expected_status: AbstractAlgebraStatus::Complete,
            expected_artifact: Some(AbstractAlgebraArtifact::CyclicGroup { order: modulus }),
        });
    }
    for index in 0..20 {
        let modulus = 2 + (index as u32 % 20);
        let mut req = request(AbstractAlgebraOperation::ConstructModularRing);
        req.modulus = Some(modulus);
        corpus.push(Case {
            id: format!("modular_ring_{index}"),
            family: "modular_ring".into(),
            request: req,
            expected_status: AbstractAlgebraStatus::Complete,
            expected_artifact: Some(AbstractAlgebraArtifact::ModularRing { modulus }),
        });
    }
    for index in 0..30 {
        let source = 2 + (index as u32 % 8);
        let target = 2 + (index as u32 % 7);
        let multiplier = index as u32 + 1;
        let mut req = request(AbstractAlgebraOperation::CheckCyclicHomomorphism);
        req.source_modulus = Some(source);
        req.target_modulus = Some(target);
        req.multiplier = Some(multiplier);
        let valid = (u64::from(multiplier) * u64::from(source)) % u64::from(target) == 0;
        corpus.push(Case {
            id: format!("cyclic_homomorphism_{index}"),
            family: "cyclic_homomorphism".into(),
            request: req,
            expected_status: AbstractAlgebraStatus::Complete,
            expected_artifact: Some(AbstractAlgebraArtifact::Boolean(valid)),
        });
    }
    for index in 0..20 {
        let modulus = 3 + (index as u32 % 25);
        let element = index as u32 % modulus;
        let mut req = request(AbstractAlgebraOperation::AdditiveOrder);
        req.modulus = Some(modulus);
        req.element = Some(element);
        let mut order = modulus;
        let mut divisor = element;
        while divisor != 0 {
            let remainder = order % divisor;
            order = divisor;
            divisor = remainder;
        }
        let gcd = order;
        corpus.push(Case {
            id: format!("additive_order_{index}"),
            family: "additive_order".into(),
            request: req,
            expected_status: AbstractAlgebraStatus::Complete,
            expected_artifact: Some(AbstractAlgebraArtifact::Scalar(modulus / gcd.max(1))),
        });
    }
    for index in 0..20 {
        let modulus = 5 + (index as u32 % 20);
        let element = (2 * index as u32 + 1) % modulus;
        let mut req = request(AbstractAlgebraOperation::CheckUnit);
        req.modulus = Some(modulus);
        req.element = Some(element);
        let mut left = modulus;
        let mut right = element;
        while right != 0 {
            let remainder = left % right;
            left = right;
            right = remainder;
        }
        corpus.push(Case {
            id: format!("unit_{index}"),
            family: "unit".into(),
            request: req,
            expected_status: AbstractAlgebraStatus::Complete,
            expected_artifact: Some(AbstractAlgebraArtifact::Boolean(left == 1)),
        });
    }
    for index in 0..20 {
        let mut req = request(AbstractAlgebraOperation::ConstructCyclicGroup);
        req.modulus = Some(2 + (index as u32 % 10));
        req.ambiguity = Some("group operation convention is unresolved".into());
        corpus.push(Case {
            id: format!("ambiguous_operation_{index}"),
            family: "ambiguous_operation".into(),
            request: req,
            expected_status: AbstractAlgebraStatus::Ambiguous,
            expected_artifact: None,
        });
    }
    for index in 0..10 {
        corpus.push(Case {
            id: format!("missing_modulus_{index}"),
            family: "missing_modulus".into(),
            request: request(AbstractAlgebraOperation::ConstructCyclicGroup),
            expected_status: AbstractAlgebraStatus::Missing,
            expected_artifact: None,
        });
    }
    for index in 0..10 {
        let mut req = request(AbstractAlgebraOperation::CheckCyclicHomomorphism);
        req.source_modulus = Some(4);
        req.target_modulus = Some(6);
        corpus.push(Case {
            id: format!("missing_homomorphism_parameter_{index}"),
            family: "missing_homomorphism_parameter".into(),
            request: req,
            expected_status: AbstractAlgebraStatus::Missing,
            expected_artifact: None,
        });
    }
    for index in 0..20 {
        let mut req = request(AbstractAlgebraOperation::ConstructCyclicGroup);
        req.modulus = Some(65);
        corpus.push(Case {
            id: format!("oversized_modulus_{index}"),
            family: "oversized_modulus".into(),
            request: req,
            expected_status: AbstractAlgebraStatus::Unsupported,
            expected_artifact: None,
        });
    }
    for index in 0..20 {
        let mut req = request(AbstractAlgebraOperation::ConstructCyclicGroup);
        req.modulus = Some(8);
        req.domain = "infinite_group".into();
        corpus.push(Case {
            id: format!("unsupported_domain_{index}"),
            family: "unsupported_domain".into(),
            request: req,
            expected_status: AbstractAlgebraStatus::InvalidDomain,
            expected_artifact: None,
        });
    }
    for index in 0..20 {
        let mut req = request(AbstractAlgebraOperation::AdditiveOrder);
        req.modulus = Some(12);
        req.element = Some(12 + index as u32);
        corpus.push(Case {
            id: format!("noncanonical_residue_{index}"),
            family: "noncanonical_residue".into(),
            request: req,
            expected_status: AbstractAlgebraStatus::Inconsistent,
            expected_artifact: None,
        });
    }
    for index in 0..20 {
        let mut req = request(AbstractAlgebraOperation::ConstructModularRing);
        req.modulus = Some(1);
        corpus.push(Case {
            id: format!("unsupported_ring_boundary_{index}"),
            family: "unsupported_ring_boundary".into(),
            request: req,
            expected_status: AbstractAlgebraStatus::Unsupported,
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
        let result = evaluate_abstract_algebra(&case.request);
        *status_counts
            .entry(format!("{:?}", result.status))
            .or_insert(0) += 1;
        let mut tampered = result.clone();
        tampered.replay_hash.push('x');
        let exact =
            result.status == case.expected_status && result.artifact == case.expected_artifact;
        let false_authorization =
            case.expected_status != AbstractAlgebraStatus::Complete && result.artifact.is_some();
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
            false_authorization,
        });
    }
    let cases = receipts.len();
    let supported = receipts
        .iter()
        .filter(|row| row.expected_status == AbstractAlgebraStatus::Complete)
        .count();
    let ambiguous = receipts
        .iter()
        .filter(|row| row.expected_status == AbstractAlgebraStatus::Ambiguous)
        .count()
        + receipts
            .iter()
            .filter(|row| row.expected_status == AbstractAlgebraStatus::Missing)
            .count();
    let unsupported = cases - supported - ambiguous;
    let exact_decisions = receipts.iter().filter(|row| row.exact).count();
    let supported_artifacts = receipts
        .iter()
        .filter(|row| {
            row.expected_status == AbstractAlgebraStatus::Complete && row.actual_artifact.is_some()
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
        .filter(|row| row.expected_status == AbstractAlgebraStatus::Complete && !row.exact)
        .count();
    assert_eq!(exact_decisions, cases);
    assert_eq!(supported_artifacts, 120);
    assert_eq!(replay_verified, cases);
    assert_eq!(tamper_rejections, cases);
    assert_eq!(false_authorizations, 0);
    assert_eq!(false_denials, 0);
    let report = Report {
        schema: "phase67-bounded-abstract-algebra-v1",
        source: "independently authored finite cyclic/ring corpus",
        corpus_sha256,
        cases,
        supported,
        ambiguous,
        unsupported,
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
        "docs/phase67_abstract_algebra_pack.json",
        serde_json::to_vec_pretty(&report)?,
    )?;
    println!("{}", serde_json::to_string_pretty(&report)?);
    Ok(())
}
