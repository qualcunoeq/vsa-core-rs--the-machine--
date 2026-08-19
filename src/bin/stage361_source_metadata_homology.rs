//! Stage 361: generic source metadata ingestion for finite homology.
//!
//! The simplicial-homology source was previously retained only as residual
//! evidence because its document had no declarative execution block. The
//! generic metadata parser now verifies citation and scope, after which the
//! already validated homology pack is exercised with source-linked
//! provenance. No new homology method is introduced here.

use serde::Serialize;
use sha2::{Digest, Sha256};
use std::fs;
use the_machine::simplicial_homology_pack::{
    evaluate, HomologyOperation, HomologyStatus, SimplicialComplexRequest,
};
use the_machine::source_metadata::{extract_source_metadata, replay_verified};

const SOURCE: &str = "docs/sources/topology_without_tears_simplicial_homology_definition.txt";
const REPORT_JSON: &str = "docs/stage361_source_metadata_homology.json";
const REPORT_MD: &str = "docs/stage361_source_metadata_homology.md";

#[derive(Debug, Serialize)]
struct Report {
    schema: &'static str,
    source_sha256: String,
    metadata_replay_verified: bool,
    metadata_tamper_rejected: bool,
    source_mutations: usize,
    source_mutations_rejected: usize,
    execution_cases: usize,
    exact_decisions: usize,
    execution_replay_verified: usize,
    execution_tamper_rejected: usize,
    complete_cases: usize,
    ambiguous_cases: usize,
    unsupported_or_invalid_cases: usize,
    false_authorizations: usize,
    false_denials: usize,
    source_match_verified: bool,
    production_mutations: usize,
    report_sha256: String,
}

fn digest<T: Serialize>(value: &T) -> String {
    format!(
        "{:x}",
        Sha256::digest(serde_json::to_vec(value).expect("report serializes"))
    )
}

fn digest_bytes(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

fn triangle(operation: HomologyOperation, provenance: &[String]) -> SimplicialComplexRequest {
    SimplicialComplexRequest {
        operation,
        domain: "finite_simplicial_complex".into(),
        vertices: vec!["a".into(), "b".into(), "c".into()],
        simplices: vec![
            vec![0],
            vec![1],
            vec![2],
            vec![0, 1],
            vec![1, 2],
            vec![0, 2],
            vec![0, 1, 2],
        ],
        coefficient_field: Some(2),
        provenance: provenance.to_vec(),
        ambiguity: None,
    }
}

fn cases(provenance: &[String]) -> Vec<(SimplicialComplexRequest, HomologyStatus)> {
    let mut cases = Vec::new();
    for operation in [
        HomologyOperation::ValidateComplex,
        HomologyOperation::EulerCharacteristic,
        HomologyOperation::BettiNumbers,
        HomologyOperation::BoundaryMatrices,
    ] {
        cases.push((triangle(operation, provenance), HomologyStatus::Complete));
    }
    let mut ambiguous = triangle(HomologyOperation::BettiNumbers, provenance);
    ambiguous.coefficient_field = None;
    cases.push((ambiguous, HomologyStatus::Ambiguous));
    let mut unsupported_field = triangle(HomologyOperation::BettiNumbers, provenance);
    unsupported_field.coefficient_field = Some(3);
    cases.push((unsupported_field, HomologyStatus::Unsupported));
    let mut ambiguous_text = triangle(HomologyOperation::EulerCharacteristic, provenance);
    ambiguous_text.ambiguity = Some("orientation or coefficient interpretation unresolved".into());
    cases.push((ambiguous_text, HomologyStatus::Ambiguous));
    let mut missing_face = triangle(HomologyOperation::ValidateComplex, provenance);
    missing_face
        .simplices
        .retain(|simplex| simplex != &vec![0, 2]);
    cases.push((missing_face, HomologyStatus::InvalidComplex));
    let mut duplicate_vertex = triangle(HomologyOperation::ValidateComplex, provenance);
    duplicate_vertex.vertices[2] = "b".into();
    cases.push((duplicate_vertex, HomologyStatus::InvalidComplex));
    let mut unknown_vertex = triangle(HomologyOperation::ValidateComplex, provenance);
    unknown_vertex.simplices.push(vec![0, 3]);
    cases.push((unknown_vertex, HomologyStatus::InvalidComplex));
    let mut high_dimension = triangle(HomologyOperation::ValidateComplex, provenance);
    high_dimension.vertices = vec!["a", "b", "c", "d", "e"]
        .into_iter()
        .map(String::from)
        .collect();
    high_dimension.simplices = vec![
        vec![0],
        vec![1],
        vec![2],
        vec![3],
        vec![4],
        vec![0, 1],
        vec![0, 1, 2],
        vec![0, 1, 2, 3],
        vec![0, 1, 2, 3, 4],
    ];
    cases.push((high_dimension, HomologyStatus::InvalidComplex));
    let mut wrong_domain = triangle(HomologyOperation::ValidateComplex, provenance);
    wrong_domain.domain = "infinite_homology".into();
    cases.push((wrong_domain, HomologyStatus::InvalidComplex));
    cases
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let source_bytes = fs::read(SOURCE)?;
    let source_text = std::str::from_utf8(&source_bytes)?;
    let metadata = extract_source_metadata(source_text).map_err(|errors| errors.join("; "))?;
    assert_eq!(
        metadata.citation.source_id,
        "topology-without-tears:finite-simplicial-homology"
    );
    assert!(metadata
        .scope
        .as_deref()
        .unwrap()
        .contains("coefficients F_2"));
    let metadata_replay_verified = replay_verified(&metadata);
    let mut metadata_tampered = metadata.clone();
    metadata_tampered.citation.title.push('x');
    let metadata_tamper_rejected = !replay_verified(&metadata_tampered);

    let mutations = vec![
        source_text.replace("SOURCE_ID:", "SOURCE_ID_REMOVED:"),
        source_text.replace("URL:", "URL_REMOVED:"),
        source_text.replace("EVIDENCE:", "EVIDENCE_REMOVED:"),
        source_text.replace("RETRIEVED_UTC:", "RETRIEVED_REMOVED:"),
    ];
    let source_mutations_rejected = mutations
        .iter()
        .filter(|mutation| extract_source_metadata(mutation).is_err())
        .count();
    let provenance = vec![
        format!("source-id:{}", metadata.citation.source_id),
        format!("source-document-sha256:{}", metadata.document_sha256),
        "stage361-source-metadata-homology".into(),
    ];
    let mut exact = 0;
    let mut replay = 0;
    let mut tamper = 0;
    let mut false_auth = 0;
    let mut false_denial = 0;
    let mut complete = 0;
    let mut ambiguous = 0;
    let mut unsupported_or_invalid = 0;
    let mut source_match_verified = true;
    for (request, expected) in cases(&provenance) {
        let result = evaluate(&request);
        exact += usize::from(result.status == expected);
        replay += usize::from(result.replay_verified());
        let mut tampered = result.clone();
        tampered.replay_hash.push('x');
        tamper += usize::from(!tampered.replay_verified());
        false_auth += usize::from(
            expected != HomologyStatus::Complete && result.status == HomologyStatus::Complete,
        );
        false_denial += usize::from(
            expected == HomologyStatus::Complete && result.status != HomologyStatus::Complete,
        );
        complete += usize::from(result.status == HomologyStatus::Complete);
        ambiguous += usize::from(result.status == HomologyStatus::Ambiguous);
        unsupported_or_invalid += usize::from(matches!(
            result.status,
            HomologyStatus::Unsupported
                | HomologyStatus::InvalidComplex
                | HomologyStatus::Inconsistent
        ));
        source_match_verified &= result.source == metadata.citation;
    }
    let execution_cases = cases(&provenance).len();
    assert!(metadata_replay_verified);
    assert!(metadata_tamper_rejected);
    assert_eq!(source_mutations_rejected, mutations.len());
    assert_eq!(exact, execution_cases);
    assert_eq!(replay, execution_cases);
    assert_eq!(tamper, execution_cases);
    assert_eq!(false_auth, 0);
    assert_eq!(false_denial, 0);
    assert!(source_match_verified);

    let mut report = Report {
        schema: "stage361-source-metadata-homology-v1",
        source_sha256: digest_bytes(&source_bytes),
        metadata_replay_verified,
        metadata_tamper_rejected,
        source_mutations: mutations.len(),
        source_mutations_rejected,
        execution_cases,
        exact_decisions: exact,
        execution_replay_verified: replay,
        execution_tamper_rejected: tamper,
        complete_cases: complete,
        ambiguous_cases: ambiguous,
        unsupported_or_invalid_cases: unsupported_or_invalid,
        false_authorizations: false_auth,
        false_denials: false_denial,
        source_match_verified,
        production_mutations: 0,
        report_sha256: String::new(),
    };
    report.report_sha256 = digest(&report);
    fs::write(
        REPORT_JSON,
        format!("{}\n", serde_json::to_string_pretty(&report)?),
    )?;
    fs::write(
        REPORT_MD,
        format!(
            "# Stage 361 — generic source metadata ingestion for finite homology\n\n\
* metadata replay / tamper rejection: {} / {}\n\
* source mutations / rejected: {} / {}\n\
* execution cases / exact decisions: {} / {}\n\
* execution replay / tamper rejection: {} / {}\n\
* complete / ambiguous / unsupported-or-invalid: {} / {} / {}\n\
* source citation match: {}\n\
* false authorizations / denials: {} / {}\n\
* production mutations: {}\n\n\
The previously residual simplicial-homology source now passes a generic citation/scope gate. The existing bounded homology pack consumed source-linked provenance and retained its F_2, finite-complex, and dimension boundaries; no new domain-specific solver or live promotion was introduced.\n",
            report.metadata_replay_verified,
            report.metadata_tamper_rejected,
            report.source_mutations,
            report.source_mutations_rejected,
            report.execution_cases,
            report.exact_decisions,
            report.execution_replay_verified,
            report.execution_tamper_rejected,
            report.complete_cases,
            report.ambiguous_cases,
            report.unsupported_or_invalid_cases,
            report.source_match_verified,
            report.false_authorizations,
            report.false_denials,
            report.production_mutations,
        ),
    )?;
    println!("{}", serde_json::to_string_pretty(&report)?);
    Ok(())
}
