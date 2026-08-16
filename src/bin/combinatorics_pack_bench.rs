//! Stage A independent validation corpus for bounded combinatorics.

use serde::Serialize;
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::fs;
use the_machine::combinatorics_pack::{
    evaluate_combinatorics, CombinatoricsArtifact, CombinatoricsOperation, CombinatoricsRequest,
    CombinatoricsStatus,
};

#[derive(Clone, Serialize)]
struct Case {
    id: String,
    family: String,
    request: CombinatoricsRequest,
    expected_status: CombinatoricsStatus,
    expected_artifact: Option<CombinatoricsArtifact>,
}

#[derive(Serialize)]
struct Receipt {
    id: String,
    family: String,
    expected_status: CombinatoricsStatus,
    actual_status: CombinatoricsStatus,
    expected_artifact: Option<CombinatoricsArtifact>,
    actual_artifact: Option<CombinatoricsArtifact>,
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
    receipts: Vec<Receipt>,
}

fn hash<T: Serialize>(value: &T) -> String {
    format!("{:x}", Sha256::digest(serde_json::to_vec(value).unwrap()))
}

fn request(operation: CombinatoricsOperation) -> CombinatoricsRequest {
    CombinatoricsRequest {
        operation,
        n: None,
        k: None,
        parts: Vec::new(),
        first_count: None,
        second_count: None,
        intersection_count: None,
        objects: None,
        boxes: None,
        domain: "bounded_exact_combinatorics".into(),
        ambiguity: None,
        provenance: vec!["stage-a-independent-combinatorics".into()],
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut corpus = Vec::new();
    for index in 0..25 {
        let mut req = request(CombinatoricsOperation::Permutations);
        req.n = Some(10);
        req.k = Some(3 + index as u64 % 4);
        let expected = (1..=req.k.unwrap()).fold(1u128, |acc, j| acc * u128::from(10 - j + 1));
        corpus.push(Case {
            id: format!("permutations_{index}"),
            family: "permutations".into(),
            request: req,
            expected_status: CombinatoricsStatus::Complete,
            expected_artifact: Some(CombinatoricsArtifact::Scalar(expected)),
        });
    }
    for index in 0..25 {
        let mut req = request(CombinatoricsOperation::Combinations);
        req.n = Some(12);
        req.k = Some(2 + index as u64 % 5);
        let k = req.k.unwrap();
        let expected = (1..=k).fold(1u128, |acc, j| acc * u128::from(12 - k + j) / u128::from(j));
        corpus.push(Case {
            id: format!("combinations_{index}"),
            family: "combinations".into(),
            request: req,
            expected_status: CombinatoricsStatus::Complete,
            expected_artifact: Some(CombinatoricsArtifact::Scalar(expected)),
        });
    }
    for index in 0..20 {
        let mut req = request(CombinatoricsOperation::Multinomial);
        req.parts = vec![2, 3, 1];
        corpus.push(Case {
            id: format!("multinomial_{index}"),
            family: "multinomial".into(),
            request: req,
            expected_status: CombinatoricsStatus::Complete,
            expected_artifact: Some(CombinatoricsArtifact::Scalar(60)),
        });
    }
    for index in 0..15 {
        let mut req = request(CombinatoricsOperation::InclusionExclusionTwo);
        req.first_count = Some(8);
        req.second_count = Some(7);
        req.intersection_count = Some(3);
        corpus.push(Case {
            id: format!("inclusion_exclusion_{index}"),
            family: "inclusion_exclusion".into(),
            request: req,
            expected_status: CombinatoricsStatus::Complete,
            expected_artifact: Some(CombinatoricsArtifact::Scalar(12)),
        });
    }
    for index in 0..15 {
        let mut req = request(CombinatoricsOperation::PigeonholeMinimum);
        req.objects = Some(17);
        req.boxes = Some(4);
        corpus.push(Case {
            id: format!("pigeonhole_{index}"),
            family: "pigeonhole".into(),
            request: req,
            expected_status: CombinatoricsStatus::Complete,
            expected_artifact: Some(CombinatoricsArtifact::Scalar(5)),
        });
    }
    for index in 0..10 {
        let mut req = request(CombinatoricsOperation::StirlingSecond);
        req.n = Some(6);
        req.k = Some(3);
        corpus.push(Case {
            id: format!("stirling_second_{index}"),
            family: "stirling_second".into(),
            request: req,
            expected_status: CombinatoricsStatus::Complete,
            expected_artifact: Some(CombinatoricsArtifact::Scalar(90)),
        });
    }
    for index in 0..10 {
        let mut req = request(CombinatoricsOperation::SurjectionCount);
        req.n = Some(5);
        req.k = Some(3);
        corpus.push(Case {
            id: format!("surjection_{index}"),
            family: "surjection".into(),
            request: req,
            expected_status: CombinatoricsStatus::Complete,
            expected_artifact: Some(CombinatoricsArtifact::Scalar(150)),
        });
    }
    for index in 0..20 {
        let mut req = request(CombinatoricsOperation::Combinations);
        req.n = Some(10);
        req.k = Some(3);
        req.ambiguity = Some("labeled versus unlabeled selection is unresolved".into());
        corpus.push(Case {
            id: format!("ambiguous_selection_{index}"),
            family: "ambiguous_selection".into(),
            request: req,
            expected_status: CombinatoricsStatus::Ambiguous,
            expected_artifact: None,
        });
    }
    for index in 0..10 {
        corpus.push(Case {
            id: format!("missing_parameters_{index}"),
            family: "missing_parameters".into(),
            request: request(CombinatoricsOperation::Permutations),
            expected_status: CombinatoricsStatus::Missing,
            expected_artifact: None,
        });
    }
    for index in 0..10 {
        let mut req = request(CombinatoricsOperation::Multinomial);
        req.parts = vec![2, 3];
        req.ambiguity = Some("partition total scope is unresolved".into());
        corpus.push(Case {
            id: format!("missing_partition_scope_{index}"),
            family: "missing_partition_scope".into(),
            request: req,
            expected_status: CombinatoricsStatus::Ambiguous,
            expected_artifact: None,
        });
    }
    for index in 0..20 {
        let mut req = request(CombinatoricsOperation::Combinations);
        req.n = Some(31);
        req.k = Some(2);
        corpus.push(Case {
            id: format!("oversized_count_{index}"),
            family: "oversized_count".into(),
            request: req,
            expected_status: CombinatoricsStatus::Unsupported,
            expected_artifact: None,
        });
    }
    for index in 0..20 {
        let mut req = request(CombinatoricsOperation::PigeonholeMinimum);
        req.objects = Some(17);
        req.boxes = Some(0);
        corpus.push(Case {
            id: format!("invalid_boxes_{index}"),
            family: "invalid_boxes".into(),
            request: req,
            expected_status: CombinatoricsStatus::Unsupported,
            expected_artifact: None,
        });
    }
    for index in 0..20 {
        let mut req = request(CombinatoricsOperation::SurjectionCount);
        req.n = Some(13);
        req.k = Some(3);
        corpus.push(Case {
            id: format!("long_surjection_{index}"),
            family: "long_surjection".into(),
            request: req,
            expected_status: CombinatoricsStatus::Unsupported,
            expected_artifact: None,
        });
    }
    for index in 0..20 {
        let mut req = request(CombinatoricsOperation::InclusionExclusionTwo);
        req.first_count = Some(2);
        req.second_count = Some(3);
        req.intersection_count = Some(4);
        corpus.push(Case {
            id: format!("invalid_intersection_{index}"),
            family: "invalid_intersection".into(),
            request: req,
            expected_status: CombinatoricsStatus::Inconsistent,
            expected_artifact: None,
        });
    }
    assert_eq!(corpus.len(), 240);
    let corpus_sha256 = hash(&corpus);
    let mut receipts = Vec::with_capacity(corpus.len());
    let mut family_counts = BTreeMap::new();
    for case in corpus {
        *family_counts.entry(case.family.clone()).or_insert(0) += 1;
        let output = evaluate_combinatorics(&case.request);
        let mut tampered = output.clone();
        tampered.replay_hash.push('x');
        let exact =
            output.status == case.expected_status && output.artifact == case.expected_artifact;
        receipts.push(Receipt {
            id: case.id,
            family: case.family,
            expected_status: case.expected_status,
            actual_status: output.status,
            expected_artifact: case.expected_artifact,
            actual_artifact: output.artifact.clone(),
            exact,
            replay_verified: output.replay_verified(),
            tamper_rejected: !tampered.replay_verified(),
            false_authorization: case.expected_status != CombinatoricsStatus::Complete
                && output.artifact.is_some(),
        });
    }
    let cases = receipts.len();
    let supported = receipts
        .iter()
        .filter(|row| row.expected_status == CombinatoricsStatus::Complete)
        .count();
    let ambiguous = receipts
        .iter()
        .filter(|row| {
            matches!(
                row.expected_status,
                CombinatoricsStatus::Ambiguous | CombinatoricsStatus::Missing
            )
        })
        .count();
    let refused = cases - supported - ambiguous;
    let exact_decisions = receipts.iter().filter(|row| row.exact).count();
    let supported_artifacts = receipts
        .iter()
        .filter(|row| {
            row.expected_status == CombinatoricsStatus::Complete && row.actual_artifact.is_some()
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
        .filter(|row| row.expected_status == CombinatoricsStatus::Complete && !row.exact)
        .count();
    assert_eq!(exact_decisions, cases);
    assert_eq!(supported_artifacts, supported);
    assert_eq!(replay_verified, cases);
    assert_eq!(tamper_rejections, cases);
    assert_eq!(false_authorizations, 0);
    assert_eq!(false_denials, 0);
    let report = Report {
        schema: "stage-a-bounded-combinatorics-v1",
        source: "independently authored finite counting corpus",
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
        receipts,
    };
    fs::write(
        "docs/stage_a_combinatorics_pack.json",
        serde_json::to_vec_pretty(&report)?,
    )?;
    println!("{}", serde_json::to_string_pretty(&report)?);
    Ok(())
}
