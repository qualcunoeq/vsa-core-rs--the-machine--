//! Phase 68 pressure corpus for bounded abstract-algebra composition.

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
    route: Vec<AbstractAlgebraRequest>,
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
    route_replay_verified: bool,
    exact: bool,
    tamper_rejected: bool,
    false_authorization: bool,
}

#[derive(Serialize)]
struct Report {
    schema: &'static str,
    source: &'static str,
    corpus_sha256: String,
    cases: usize,
    supported_compositions: usize,
    ambiguous_routes: usize,
    refused_routes: usize,
    exact_route_decisions: usize,
    route_replay_verified: usize,
    tamper_rejections: usize,
    false_authorizations: usize,
    false_denials: usize,
    family_counts: BTreeMap<String, usize>,
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
        second_multiplier: None,
        domain: "finite_exact_abstract_algebra".into(),
        assumptions: vec!["finite cyclic structures explicitly declared".into()],
        ambiguity: None,
        provenance: vec!["phase68-independent-composition".into()],
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut corpus = Vec::new();
    for index in 0..30 {
        let modulus = 3 + index as u32 % 20;
        let mut construct = request(AbstractAlgebraOperation::ConstructCyclicGroup);
        construct.modulus = Some(modulus);
        let mut element = request(AbstractAlgebraOperation::AdditiveOrder);
        element.modulus = Some(modulus);
        element.element = Some(index as u32 % modulus);
        corpus.push(Case {
            id: format!("representative_to_additive_order_{index}"),
            family: "representative_to_additive_structure".into(),
            route: vec![construct, element],
            expected_status: AbstractAlgebraStatus::Complete,
            expected_artifact: Some(AbstractAlgebraArtifact::Scalar(
                modulus / gcd(modulus, index as u32 % modulus),
            )),
        });
    }
    for index in 0..40 {
        let source = 2 + index as u32 % 8;
        let middle = 2 + index as u32 % 7;
        let target = 2 + index as u32 % 6;
        let first = 1 + index as u32;
        let second = if index % 2 == 0 { target } else { target * 2 };
        let mut compose = request(AbstractAlgebraOperation::ComposeCyclicHomomorphisms);
        compose.source_modulus = Some(source);
        compose.modulus = Some(middle);
        compose.target_modulus = Some(target);
        compose.multiplier = Some(first);
        compose.second_multiplier = Some(second);
        // Choose a valid first map by adjusting the multiplier to a multiple
        // of middle/gcd(source,middle), while retaining varied cases.
        let required = middle / gcd(source, middle);
        compose.multiplier = Some(required * (index as u32 + 1));
        let composed = (compose.multiplier.unwrap() * second) % target;
        corpus.push(Case {
            id: format!("homomorphism_composition_{index}"),
            family: "homomorphism_composition".into(),
            route: vec![compose],
            expected_status: AbstractAlgebraStatus::Complete,
            expected_artifact: Some(AbstractAlgebraArtifact::CyclicHomomorphism {
                source_order: source,
                target_order: target,
                multiplier: composed,
            }),
        });
    }
    for index in 0..30 {
        let source = 3 + index as u32 % 18;
        let target = 2 + index as u32 % 12;
        let multiplier = target * (index as u32 + 1);
        let mut req = request(AbstractAlgebraOperation::KernelImage);
        req.source_modulus = Some(source);
        req.target_modulus = Some(target);
        req.multiplier = Some(multiplier);
        let common = gcd(multiplier, target);
        corpus.push(Case {
            id: format!("kernel_image_{index}"),
            family: "kernel_image".into(),
            route: vec![req],
            expected_status: AbstractAlgebraStatus::Complete,
            expected_artifact: Some(AbstractAlgebraArtifact::KernelImage {
                kernel_size: source * common / target,
                image_size: target / common,
            }),
        });
    }
    for index in 0..20 {
        let mut ring = request(AbstractAlgebraOperation::ConstructModularRing);
        ring.modulus = Some(5 + index as u32 % 20);
        let mut unit = request(AbstractAlgebraOperation::CheckUnit);
        unit.modulus = ring.modulus;
        unit.element = Some((2 * index as u32 + 1) % unit.modulus.unwrap());
        let unit_modulus = unit.modulus.unwrap();
        let unit_element = unit.element.unwrap();
        corpus.push(Case {
            id: format!("ring_to_unit_{index}"),
            family: "ring_to_unit".into(),
            route: vec![ring, unit],
            expected_status: AbstractAlgebraStatus::Complete,
            expected_artifact: Some(AbstractAlgebraArtifact::Boolean(
                gcd(unit_modulus, unit_element) == 1,
            )),
        });
    }
    for index in 0..20 {
        let mut req = request(AbstractAlgebraOperation::ComposeCyclicHomomorphisms);
        req.source_modulus = Some(4);
        req.modulus = Some(6);
        req.target_modulus = Some(8);
        req.multiplier = Some(3);
        if index % 2 == 0 {
            req.second_multiplier = None;
        } else {
            req.second_multiplier = Some(4);
            req.ambiguity = Some("middle cyclic convention is unresolved".into());
        }
        corpus.push(Case {
            id: format!("ambiguous_composition_{index}"),
            family: "ambiguous_composition".into(),
            route: vec![req],
            expected_status: if index % 2 == 0 {
                AbstractAlgebraStatus::Missing
            } else {
                AbstractAlgebraStatus::Ambiguous
            },
            expected_artifact: None,
        });
    }
    for index in 0..20 {
        let mut req = request(AbstractAlgebraOperation::KernelImage);
        req.source_modulus = Some(8);
        req.target_modulus = Some(6);
        req.multiplier = Some(1);
        corpus.push(Case {
            id: format!("invalid_kernel_map_{index}"),
            family: "invalid_kernel_map".into(),
            route: vec![req],
            expected_status: AbstractAlgebraStatus::Inconsistent,
            expected_artifact: None,
        });
    }
    for index in 0..20 {
        let mut req = request(AbstractAlgebraOperation::ConstructModularRing);
        req.modulus = Some(7);
        req.domain = "finite_field_semantics".into();
        corpus.push(Case {
            id: format!("field_semantics_refusal_{index}"),
            family: "field_semantics_refusal".into(),
            route: vec![req],
            expected_status: AbstractAlgebraStatus::InvalidDomain,
            expected_artifact: None,
        });
    }
    for index in 0..20 {
        let mut req = request(AbstractAlgebraOperation::ConstructCyclicGroup);
        req.modulus = Some(8);
        req.domain = "non_cyclic_group_presentation".into();
        corpus.push(Case {
            id: format!("noncyclic_refusal_{index}"),
            family: "noncyclic_refusal".into(),
            route: vec![req],
            expected_status: AbstractAlgebraStatus::InvalidDomain,
            expected_artifact: None,
        });
    }
    for index in 0..20 {
        let mut req = request(AbstractAlgebraOperation::ComposeCyclicHomomorphisms);
        req.source_modulus = Some(4);
        req.modulus = Some(6);
        req.target_modulus = Some(8);
        req.multiplier = Some(1);
        req.second_multiplier = Some(1);
        corpus.push(Case {
            id: format!("invalid_component_composition_{index}"),
            family: "invalid_component_composition".into(),
            route: vec![req],
            expected_status: AbstractAlgebraStatus::Inconsistent,
            expected_artifact: None,
        });
    }
    for index in 0..20 {
        let mut req = request(AbstractAlgebraOperation::CheckUnit);
        req.modulus = Some(1);
        req.element = Some(0);
        corpus.push(Case {
            id: format!("additive_multiplicative_confusion_{index}"),
            family: "additive_multiplicative_confusion".into(),
            route: vec![req],
            expected_status: AbstractAlgebraStatus::Unsupported,
            expected_artifact: None,
        });
    }
    assert_eq!(corpus.len(), 240);

    let corpus_sha256 = hash(&corpus);
    let mut receipts = Vec::with_capacity(corpus.len());
    let mut family_counts = BTreeMap::new();
    for case in corpus {
        *family_counts.entry(case.family.clone()).or_insert(0) += 1;
        let results: Vec<_> = case.route.iter().map(evaluate_abstract_algebra).collect();
        let final_result = results.last().expect("non-empty route");
        let route_replay_verified = results.iter().all(|result| result.replay_verified());
        let mut tampered = final_result.clone();
        tampered.replay_hash.push('x');
        let exact = final_result.status == case.expected_status
            && final_result.artifact == case.expected_artifact;
        let false_authorization = case.expected_status != AbstractAlgebraStatus::Complete
            && final_result.artifact.is_some();
        receipts.push(Receipt {
            id: case.id,
            family: case.family,
            expected_status: case.expected_status,
            actual_status: final_result.status,
            expected_artifact: case.expected_artifact,
            actual_artifact: final_result.artifact.clone(),
            route_replay_verified,
            exact,
            tamper_rejected: !tampered.replay_verified(),
            false_authorization,
        });
    }
    let cases = receipts.len();
    let supported_compositions = receipts
        .iter()
        .filter(|row| row.expected_status == AbstractAlgebraStatus::Complete)
        .count();
    let ambiguous_routes = receipts
        .iter()
        .filter(|row| {
            matches!(
                row.expected_status,
                AbstractAlgebraStatus::Ambiguous | AbstractAlgebraStatus::Missing
            )
        })
        .count();
    let refused_routes = cases - supported_compositions - ambiguous_routes;
    let exact_route_decisions = receipts.iter().filter(|row| row.exact).count();
    let route_replay_verified = receipts
        .iter()
        .filter(|row| row.route_replay_verified)
        .count();
    let tamper_rejections = receipts.iter().filter(|row| row.tamper_rejected).count();
    let false_authorizations = receipts
        .iter()
        .filter(|row| row.false_authorization)
        .count();
    let false_denials = receipts
        .iter()
        .filter(|row| row.expected_status == AbstractAlgebraStatus::Complete && !row.exact)
        .count();
    assert_eq!(exact_route_decisions, cases);
    assert_eq!(route_replay_verified, cases);
    assert_eq!(tamper_rejections, cases);
    assert_eq!(false_authorizations, 0);
    assert_eq!(false_denials, 0);
    let report = Report {
        schema: "phase68-abstract-algebra-composition-v1",
        source: "independently authored finite cyclic composition corpus",
        corpus_sha256,
        cases,
        supported_compositions,
        ambiguous_routes,
        refused_routes,
        exact_route_decisions,
        route_replay_verified,
        tamper_rejections,
        false_authorizations,
        false_denials,
        family_counts,
        receipts,
    };
    fs::write(
        "docs/phase68_abstract_algebra_composition.json",
        serde_json::to_vec_pretty(&report)?,
    )?;
    println!("{}", serde_json::to_string_pretty(&report)?);
    Ok(())
}

fn gcd(mut left: u32, mut right: u32) -> u32 {
    while right != 0 {
        let remainder = left % right;
        left = right;
        right = remainder;
    }
    left.max(1)
}
