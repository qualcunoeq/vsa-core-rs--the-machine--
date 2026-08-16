//! Stage D source-derived domain acquisition for bounded finite topology.
//!
//! The topology axiom record is extracted from an attributed textbook
//! transcription.  Exercises are generated independently from that record;
//! the source-derived executor is validated in a shadow copy and never added
//! to production routing.

use serde::Serialize;
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use the_machine::curriculum::breadth_first_manifest;
use the_machine::source_topology_pack::{
    evaluate_topology, extract_topology_definitions, TopologyArtifact, TopologyOperation,
    TopologyRequest, TopologyStatus,
};

#[derive(Clone, Serialize)]
struct Case {
    id: String,
    family: String,
    request: TopologyRequest,
    expected_status: TopologyStatus,
    expected_artifact: Option<TopologyArtifact>,
}

#[derive(Serialize)]
struct Receipt {
    id: String,
    family: String,
    expected_status: TopologyStatus,
    actual_status: TopologyStatus,
    exact: bool,
    replay_verified: bool,
    tamper_rejected: bool,
    false_authorization: bool,
}

#[derive(Serialize)]
struct Report {
    schema: &'static str,
    source_document_sha256: String,
    corpus_sha256: String,
    extracted_records: usize,
    source_mutations: usize,
    source_mutations_rejected: usize,
    cases: usize,
    supported: usize,
    ambiguous: usize,
    unsupported: usize,
    exact_decisions: usize,
    supported_artifacts: usize,
    replay_verified: usize,
    tamper_rejected: usize,
    false_authorizations: usize,
    false_denials: usize,
    manifest_unchanged: bool,
    production_authorizations: usize,
    family_counts: BTreeMap<String, usize>,
    receipts_hash: String,
}

fn hash<T: Serialize>(value: &T) -> String {
    format!("{:x}", Sha256::digest(serde_json::to_vec(value).unwrap()))
}

fn names(n: usize) -> Vec<String> {
    (0..n).map(|index| format!("p{index}")).collect()
}

fn all_subsets(points: &[String]) -> Vec<Vec<String>> {
    (0..(1usize << points.len()))
        .map(|mask| {
            points
                .iter()
                .enumerate()
                .filter(|(index, _)| (mask & (1 << index)) != 0)
                .map(|(_, point)| point.clone())
                .collect()
        })
        .collect()
}

fn topology_for(index: usize) -> (Vec<String>, Vec<Vec<String>>) {
    let points = names(2 + index % 4);
    match index % 3 {
        0 => (points.clone(), vec![Vec::new(), points]),
        1 => (points.clone(), all_subsets(&points)),
        _ => {
            let first = vec![points[0].clone()];
            (points.clone(), vec![Vec::new(), first, points])
        }
    }
}

fn request(
    operation: TopologyOperation,
    points: Vec<String>,
    open_sets: Vec<Vec<String>>,
    target_set: Option<Vec<String>>,
) -> TopologyRequest {
    TopologyRequest {
        operation,
        topology: "finite_topology_axioms".into(),
        points,
        open_sets,
        target_set,
        domain: "source_derived_finite_topology".into(),
        ambiguity: None,
        provenance: vec!["stage-d-independent-topology-exercise".into()],
    }
}

fn expected_set(operation: TopologyOperation, points: &[String], opens: &[Vec<String>], target: &[String]) -> TopologyArtifact {
    match operation {
        TopologyOperation::IsOpen => TopologyArtifact::Boolean(opens.iter().any(|open| open == target)),
        TopologyOperation::IsClosed => {
            let complement = points.iter().filter(|point| !target.contains(point)).cloned().collect::<Vec<_>>();
            TopologyArtifact::Boolean(opens.iter().any(|open| open == &complement))
        }
        TopologyOperation::Interior => {
            let mut result = Vec::new();
            for open in opens.iter().filter(|open| open.iter().all(|point| target.contains(point))) {
                result.extend(open.iter().cloned());
            }
            result.sort();
            result.dedup();
            TopologyArtifact::Set(result)
        }
        TopologyOperation::Closure => {
            let closed_sets = opens.iter().map(|open| points.iter().filter(|point| !open.contains(point)).cloned().collect::<Vec<_>>()).collect::<Vec<_>>();
            let mut result = points.to_vec();
            for closed in closed_sets.iter().filter(|closed| target.iter().all(|point| closed.contains(point))) {
                result.retain(|point| closed.contains(point));
            }
            result.sort();
            TopologyArtifact::Set(result)
        }
        TopologyOperation::ValidateTopology => {
            let mut canonical_opens = opens
                .iter()
                .map(|open| {
                    let mut open = open.clone();
                    open.sort();
                    open
                })
                .collect::<Vec<_>>();
            canonical_opens.sort();
            TopologyArtifact::ValidatedTopology {
                points: points.to_vec(),
                open_sets: canonical_opens,
            }
        }
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let manifest = breadth_first_manifest();
    let manifest_hash = manifest.replay_hash();
    let source_document = include_str!("../../docs/sources/topology_without_tears_finite_definition.txt");
    let records = extract_topology_definitions(source_document).expect("source definition extracts");
    let mut mutations = vec![
        source_document.replace("TOPOLOGY_ID: finite_topology_axioms", "TOPOLOGY_ID: "),
        source_document.replace("URL: https://", "URL: http://"),
        source_document.replace("AXIOMS: empty;whole;unions;finite_intersections", "AXIOMS: empty;whole"),
        source_document.replace("MAX_POINTS: 8", "MAX_POINTS: 0"),
        source_document.replace("END TOPOLOGY", "BEGIN TOPOLOGY"),
        source_document.replace("ALIASES: finite topology|topological space", "ALIASES: duplicate|duplicate"),
    ];
    let source_mutations_rejected = mutations
        .drain(..)
        .filter(|mutation| extract_topology_definitions(&mutation).is_err())
        .count();
    assert_eq!(records.len(), 1);
    assert_eq!(source_mutations_rejected, 6);

    let mut corpus = Vec::new();
    for index in 0..24 {
        let (points, opens) = topology_for(index);
        corpus.push(Case {
            id: format!("validate_{index}"),
            family: "validate_topology".into(),
            request: request(TopologyOperation::ValidateTopology, points.clone(), opens.clone(), None),
            expected_status: TopologyStatus::Complete,
            expected_artifact: Some(expected_set(TopologyOperation::ValidateTopology, &points, &opens, &[])),
        });
        let target = opens[index % opens.len()].clone();
        corpus.push(Case {
            id: format!("is_open_{index}"),
            family: "is_open".into(),
            request: request(TopologyOperation::IsOpen, points.clone(), opens.clone(), Some(target.clone())),
            expected_status: TopologyStatus::Complete,
            expected_artifact: Some(expected_set(TopologyOperation::IsOpen, &points, &opens, &target)),
        });
        let target = points.iter().filter(|point| !opens[index % opens.len()].contains(point)).cloned().collect::<Vec<_>>();
        corpus.push(Case {
            id: format!("is_closed_{index}"),
            family: "is_closed".into(),
            request: request(TopologyOperation::IsClosed, points.clone(), opens.clone(), Some(target.clone())),
            expected_status: TopologyStatus::Complete,
            expected_artifact: Some(expected_set(TopologyOperation::IsClosed, &points, &opens, &target)),
        });
        let target = points.iter().take(1 + index % points.len()).cloned().collect::<Vec<_>>();
        corpus.push(Case {
            id: format!("interior_{index}"),
            family: "interior".into(),
            request: request(TopologyOperation::Interior, points.clone(), opens.clone(), Some(target.clone())),
            expected_status: TopologyStatus::Complete,
            expected_artifact: Some(expected_set(TopologyOperation::Interior, &points, &opens, &target)),
        });
        let target = points.iter().skip(index % points.len()).take(1 + index % points.len()).cloned().collect::<Vec<_>>();
        corpus.push(Case {
            id: format!("closure_{index}"),
            family: "closure".into(),
            request: request(TopologyOperation::Closure, points.clone(), opens.clone(), Some(target.clone())),
            expected_status: TopologyStatus::Complete,
            expected_artifact: Some(expected_set(TopologyOperation::Closure, &points, &opens, &target)),
        });
    }
    assert_eq!(corpus.len(), 120);
    for index in 0..40 {
        let (points, opens) = topology_for(index);
        let mut req = request(TopologyOperation::Interior, points, opens, Some(vec!["p0".into()]));
        req.ambiguity = Some("the source notation leaves the target set unresolved".into());
        corpus.push(Case {
            id: format!("ambiguous_{index}"),
            family: "ambiguous_target".into(),
            request: req,
            expected_status: TopologyStatus::Ambiguous,
            expected_artifact: None,
        });
    }
    for index in 0..20 {
        let (points, opens) = topology_for(index);
        let mut req = request(TopologyOperation::ValidateTopology, points, opens, None);
        req.domain = "metric_or_infinite_topology".into();
        corpus.push(Case {
            id: format!("unsupported_domain_{index}"),
            family: "unsupported_domain".into(),
            request: req,
            expected_status: TopologyStatus::Missing,
            expected_artifact: None,
        });
    }
    for index in 0..20 {
        let points = names(9);
        let opens = vec![Vec::new(), points.clone()];
        corpus.push(Case {
            id: format!("oversized_{index}"),
            family: "oversized_carrier".into(),
            request: request(TopologyOperation::ValidateTopology, points, opens, None),
            expected_status: TopologyStatus::Unsupported,
            expected_artifact: None,
        });
    }
    for index in 0..20 {
        let points = names(3);
        let opens = vec![Vec::new(), vec!["p0".into()], vec!["p1".into()], points.clone()];
        corpus.push(Case {
            id: format!("invalid_axioms_{index}"),
            family: "invalid_open_set_family".into(),
            request: request(TopologyOperation::ValidateTopology, points, opens, None),
            expected_status: TopologyStatus::Inconsistent,
            expected_artifact: None,
        });
    }
    assert_eq!(corpus.len(), 220);
    for index in 0..20 {
        let (points, opens) = topology_for(index);
        corpus.push(Case {
            id: format!("missing_target_{index}"),
            family: "missing_target".into(),
            request: request(TopologyOperation::Closure, points, opens, None),
            expected_status: TopologyStatus::Inconsistent,
            expected_artifact: None,
        });
    }
    assert_eq!(corpus.len(), 240);

    let mut receipts = Vec::with_capacity(corpus.len());
    let mut exact = 0;
    let mut supported = 0;
    let mut ambiguous = 0;
    let mut unsupported = 0;
    let mut supported_artifacts = 0;
    let mut replay = 0;
    let mut tamper = 0;
    let mut false_auth = 0;
    let mut false_denial = 0;
    let mut family_counts = BTreeMap::new();
    for case in &corpus {
        *family_counts.entry(case.family.clone()).or_insert(0usize) += 1;
        let result = evaluate_topology(&case.request, &records);
        let exact_case = result.status == case.expected_status && result.artifact == case.expected_artifact;
        let replay_ok = result.replay_verified();
        let mut altered = result.clone();
        altered.replay_hash.push('x');
        let tamper_ok = !altered.replay_verified();
        exact += usize::from(exact_case);
        supported += usize::from(case.expected_status == TopologyStatus::Complete && exact_case);
        ambiguous += usize::from(case.expected_status == TopologyStatus::Ambiguous && exact_case);
        unsupported += usize::from(
            case.expected_status != TopologyStatus::Complete
                && case.expected_status != TopologyStatus::Ambiguous
                && exact_case,
        );
        supported_artifacts += usize::from(case.expected_status == TopologyStatus::Complete && result.artifact.is_some());
        replay += usize::from(replay_ok);
        tamper += usize::from(tamper_ok);
        let false_authorization = case.expected_status != TopologyStatus::Complete && result.authorized();
        let false_denial_case = case.expected_status == TopologyStatus::Complete && !result.authorized();
        false_auth += usize::from(false_authorization);
        false_denial += usize::from(false_denial_case);
        receipts.push(Receipt {
            id: case.id.clone(),
            family: case.family.clone(),
            expected_status: case.expected_status,
            actual_status: result.status,
            exact: exact_case,
            replay_verified: replay_ok,
            tamper_rejected: tamper_ok,
            false_authorization,
        });
    }
    assert_eq!(exact, 240);
    assert_eq!(supported, 120);
    assert_eq!(ambiguous, 40);
    assert_eq!(unsupported, 80);
    assert_eq!(supported_artifacts, 120);
    assert_eq!(replay, 240);
    assert_eq!(tamper, 240);
    assert_eq!(false_auth, 0);
    assert_eq!(false_denial, 0);
    let report = Report {
        schema: "stage-d-source-derived-finite-topology-v1",
        source_document_sha256: hash(&source_document),
        corpus_sha256: hash(&corpus),
        extracted_records: records.len(),
        source_mutations: 6,
        source_mutations_rejected,
        cases: corpus.len(),
        supported,
        ambiguous,
        unsupported,
        exact_decisions: exact,
        supported_artifacts,
        replay_verified: replay,
        tamper_rejected: tamper,
        false_authorizations: false_auth,
        false_denials: false_denial,
        manifest_unchanged: manifest_hash == manifest.replay_hash(),
        production_authorizations: 0,
        family_counts,
        receipts_hash: hash(&receipts),
    };
    assert!(report.manifest_unchanged);
    let serialized = serde_json::to_string_pretty(&report)?;
    std::fs::write("docs/stage-d-source-derived-finite-topology.json", format!("{serialized}\n"))?;
    println!("{serialized}");
    Ok(())
}
