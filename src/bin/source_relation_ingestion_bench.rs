//! Stage D generic source-relation extraction and ingestion benchmark.

use serde::Serialize;
use sha2::{Digest, Sha256};
use the_machine::source_formula_pack::biology_pack::{
    evaluate_biology, BiologyArtifact, BiologyOperation, BiologyRequest,
};
use the_machine::source_formula_pack::source_relation_pack::{
    evaluate_relation, extract_relation_records, validate_relation_records, RelationRequest,
    RelationStatus,
};

#[derive(Serialize)]
struct Receipt {
    id: String,
    expected: &'static str,
    actual_status: RelationStatus,
    exact: bool,
    biology_agrees: bool,
    replay_verified: bool,
    tamper_rejected: bool,
    source_preserved: bool,
    false_authorization: bool,
}

#[derive(Serialize)]
struct Report {
    schema: &'static str,
    source_document_sha256: String,
    corpus_sha256: String,
    record_count: usize,
    valid_catalogs: usize,
    mutated_catalogs: usize,
    mutated_catalogs_rejected: usize,
    independent_exercises: usize,
    exercise_matches: usize,
    ambiguous_cases: usize,
    refused_cases: usize,
    exact_decisions: usize,
    replay_verified: usize,
    tamper_rejected: usize,
    source_preserved: usize,
    false_authorizations: usize,
    receipts: Vec<Receipt>,
}

fn digest<T: Serialize>(value: &T) -> String {
    format!(
        "{:x}",
        Sha256::digest(serde_json::to_vec(value).expect("relation report serializes"))
    )
}

fn relation_request(relation: &str, input: &str) -> RelationRequest {
    RelationRequest {
        relation: relation.into(),
        input: input.into(),
        domain: "molecular_biology.dna".into(),
        ambiguity: None,
        provenance: vec!["stage-d-source-relation-exercise".into()],
    }
}

fn biology_pair(input: &str) -> Option<String> {
    let result = evaluate_biology(&BiologyRequest {
        operation: BiologyOperation::Complement,
        sequence: Some(input.into()),
        orientation: Some("5_to_3".into()),
        domain: "source_derived_bounded_dna".into(),
        ambiguity: None,
        provenance: vec!["source-relation-comparison".into()],
    });
    match result.artifact {
        Some(BiologyArtifact::PairedComplement { complement, .. }) => Some(complement),
        _ => None,
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let document = include_str!("../../docs/sources/openstax_biology_relation_document.txt");
    let records = extract_relation_records(document).expect("source relation document extracts");
    assert_eq!(records.len(), 1);
    assert!(validate_relation_records(&records).is_ok());
    let source_document_sha256 = digest(&document);
    let mut receipts = Vec::with_capacity(200);
    let bases = ["A", "T", "C", "G"];
    for index in 0..120 {
        let input = bases[index % bases.len()];
        let relation = if index % 2 == 0 {
            "dna_complementary_base"
        } else {
            "DNA complementary base pairing"
        };
        let result = evaluate_relation(&relation_request(relation, input), &records);
        let expected_output = biology_pair(input).expect("biology pair");
        let biology_agrees = result
            .artifact
            .as_ref()
            .is_some_and(|artifact| artifact.output == expected_output);
        let mut tampered = result.clone();
        tampered.replay_hash.push('x');
        let authorized = result.authorized() && biology_agrees;
        receipts.push(Receipt {
            id: format!("supported_{index:03}"),
            expected: "supported",
            actual_status: result.status,
            exact: authorized,
            biology_agrees,
            replay_verified: result.replay_verified(),
            tamper_rejected: !tampered.replay_verified(),
            source_preserved: result.source.is_some(),
            false_authorization: false,
        });
    }
    for index in 0..40 {
        let mut request = relation_request("dna_complementary_base", "A");
        request.ambiguity = Some("multiple relation formulations remain possible".into());
        let result = evaluate_relation(&request, &records);
        let mut tampered = result.clone();
        tampered.replay_hash.push('x');
        receipts.push(Receipt {
            id: format!("ambiguous_{index:03}"),
            expected: "ambiguous",
            actual_status: result.status,
            exact: result.status == RelationStatus::Ambiguous,
            biology_agrees: false,
            replay_verified: result.replay_verified(),
            tamper_rejected: !tampered.replay_verified(),
            source_preserved: !result.provenance.is_empty(),
            false_authorization: result.authorized(),
        });
    }
    for index in 0..40 {
        let input = if index % 2 == 0 { "U" } else { "X" };
        let result = evaluate_relation(&relation_request("dna_complementary_base", input), &records);
        let mut tampered = result.clone();
        tampered.replay_hash.push('x');
        receipts.push(Receipt {
            id: format!("refused_symbol_{index:03}"),
            expected: "refused",
            actual_status: result.status,
            exact: result.status == RelationStatus::Unsupported,
            biology_agrees: false,
            replay_verified: result.replay_verified(),
            tamper_rejected: !tampered.replay_verified(),
            source_preserved: result.source.is_some(),
            false_authorization: result.authorized(),
        });
    }
    for index in 0..20 {
        let mut request = relation_request("dna_complementary_base", "A");
        request.domain = "unvalidated.domain".into();
        let result = evaluate_relation(&request, &records);
        let mut tampered = result.clone();
        tampered.replay_hash.push('x');
        receipts.push(Receipt {
            id: format!("refused_domain_{index:03}"),
            expected: "refused",
            actual_status: result.status,
            exact: result.status == RelationStatus::Missing,
            biology_agrees: false,
            replay_verified: result.replay_verified(),
            tamper_rejected: !tampered.replay_verified(),
            source_preserved: !result.provenance.is_empty(),
            false_authorization: result.authorized(),
        });
    }
    for index in 0..20 {
        let result = evaluate_relation(&relation_request("unknown_relation", "A"), &records);
        let mut tampered = result.clone();
        tampered.replay_hash.push('x');
        receipts.push(Receipt {
            id: format!("refused_relation_{index:03}"),
            expected: "refused",
            actual_status: result.status,
            exact: result.status == RelationStatus::Missing,
            biology_agrees: false,
            replay_verified: result.replay_verified(),
            tamper_rejected: !tampered.replay_verified(),
            source_preserved: !result.provenance.is_empty(),
            false_authorization: result.authorized(),
        });
    }
    assert_eq!(receipts.len(), 240);
    let exact_decisions = receipts.iter().filter(|receipt| receipt.exact).count();
    let replay_verified = receipts.iter().filter(|receipt| receipt.replay_verified).count();
    let tamper_rejected = receipts.iter().filter(|receipt| receipt.tamper_rejected).count();
    let source_preserved = receipts.iter().filter(|receipt| receipt.source_preserved).count();
    let false_authorizations = receipts.iter().filter(|receipt| receipt.false_authorization).count();
    assert_eq!(exact_decisions, 240);
    assert_eq!(replay_verified, 240);
    assert_eq!(tamper_rejected, 240);
    assert_eq!(source_preserved, 240);
    assert_eq!(false_authorizations, 0);

    let mutations = vec![
        document.replace("RELATION_ID: dna_complementary_base", "RELATION_ID: "),
        document.replace("PAIRS: A=T|T=A|C=G|G=C", "PAIRS: A=T|A=G"),
        document.replace("URL: https://", "URL: http://"),
        document.replace(
            "EVIDENCE: A pairs with T and G pairs with C in complementary DNA strands",
            "EVIDENCE: ",
        ),
        document.replace("END RELATION", "BEGIN RELATION"),
        document.replace("ALIASES: DNA complementary base pairing|complementary DNA base", "ALIASES: duplicate|duplicate"),
    ];
    let mutated_catalogs_rejected = mutations
        .iter()
        .filter(|mutation| extract_relation_records(mutation).is_err())
        .count();
    assert_eq!(mutated_catalogs_rejected, mutations.len());
    let exercise_matches = receipts
        .iter()
        .filter(|receipt| receipt.expected == "supported" && receipt.biology_agrees)
        .count();
    assert_eq!(exercise_matches, 120);
    let report = Report {
        schema: "stage-d-source-relation-ingestion-v1",
        source_document_sha256,
        corpus_sha256: digest(&receipts),
        record_count: records.len(),
        valid_catalogs: 1,
        mutated_catalogs: mutations.len(),
        mutated_catalogs_rejected,
        independent_exercises: 120,
        exercise_matches,
        ambiguous_cases: 40,
        refused_cases: 80,
        exact_decisions,
        replay_verified,
        tamper_rejected,
        source_preserved,
        false_authorizations,
        receipts,
    };
    let serialized = serde_json::to_string_pretty(&report)?;
    std::fs::write(
        "docs/stage_d_source_relation_ingestion.json",
        format!("{serialized}\n"),
    )?;
    println!("{serialized}");
    Ok(())
}
