//! Stage H source-derived bounded DNA biology benchmark.

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use the_machine::source_formula_pack::biology_pack::{
    evaluate_biology, BiologyOperation, BiologyRequest, BiologyStatus,
};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
enum Expected {
    Supported,
    Ambiguous,
    Refused,
}

#[derive(Debug, Clone, Serialize)]
struct Receipt {
    id: String,
    expected: Expected,
    actual_status: BiologyStatus,
    exact: bool,
    authorized: bool,
    replay_verified: bool,
    tamper_rejected: bool,
    source_preserved: bool,
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
    supported_artifacts: usize,
    replay_verified: usize,
    tamper_rejections: usize,
    source_preserved: usize,
    false_authorizations: usize,
    false_denials: usize,
    status_counts: BTreeMap<String, usize>,
    receipts: Vec<Receipt>,
}

fn digest<T: Serialize>(value: &T) -> String {
    format!(
        "{:x}",
        Sha256::digest(serde_json::to_vec(value).expect("biology corpus serializes"))
    )
}

fn request(
    operation: BiologyOperation,
    sequence: Option<String>,
    orientation: Option<&str>,
    domain: &str,
    ambiguity: Option<&str>,
) -> BiologyRequest {
    BiologyRequest {
        operation,
        sequence,
        orientation: orientation.map(str::to_string),
        domain: domain.into(),
        ambiguity: ambiguity.map(str::to_string),
        provenance: vec!["stage-h-source-biology-pack".into()],
    }
}

fn run(id: String, request: BiologyRequest, expected: Expected) -> Receipt {
    let result = evaluate_biology(&request);
    let authorized = result.authorized();
    let replay_verified = result.replay_verified();
    let mut tampered = result.clone();
    tampered.replay_hash.push('x');
    let tamper_rejected = !tampered.replay_verified();
    let exact = match expected {
        Expected::Supported => result.status == BiologyStatus::Complete && authorized,
        Expected::Ambiguous => result.status == BiologyStatus::Ambiguous && !authorized,
        Expected::Refused => {
            matches!(
                result.status,
                BiologyStatus::Unsupported
                    | BiologyStatus::Inconsistent
                    | BiologyStatus::InvalidDomain
            ) && !authorized
        }
    };
    Receipt {
        id,
        expected,
        actual_status: result.status,
        exact,
        authorized,
        replay_verified,
        tamper_rejected,
        source_preserved: result.source.is_some() || !result.provenance.is_empty(),
        false_authorization: expected != Expected::Supported && authorized,
        false_denial: expected == Expected::Supported && !authorized,
    }
}

fn main() {
    let sequences = ["AATTGGCC", "ATCGATCG", "GCGCGCAA", "TTAAACCG", "AGCTAGCT"];
    let mut receipts = Vec::with_capacity(240);
    for index in 0..60 {
        receipts.push(run(
            format!("validate_{index:03}"),
            request(
                BiologyOperation::ValidateDna,
                Some(sequences[index % sequences.len()].into()),
                Some("5_to_3"),
                "source_derived_bounded_dna",
                None,
            ),
            Expected::Supported,
        ));
    }
    for index in 0..30 {
        receipts.push(run(
            format!("complement_{index:03}"),
            request(
                BiologyOperation::Complement,
                Some(sequences[index % sequences.len()].into()),
                Some("5_to_3"),
                "source_derived_bounded_dna",
                None,
            ),
            Expected::Supported,
        ));
    }
    for index in 0..30 {
        receipts.push(run(
            format!("reverse_complement_{index:03}"),
            request(
                BiologyOperation::ReverseComplement,
                Some(sequences[index % sequences.len()].into()),
                Some("5_to_3"),
                "source_derived_bounded_dna",
                None,
            ),
            Expected::Supported,
        ));
    }
    for index in 0..20 {
        receipts.push(run(
            format!("ambiguous_complement_{index:03}"),
            request(
                BiologyOperation::Complement,
                Some("AATTGGCC".into()),
                None,
                "source_derived_bounded_dna",
                None,
            ),
            Expected::Ambiguous,
        ));
    }
    for index in 0..20 {
        receipts.push(run(
            format!("ambiguous_report_{index:03}"),
            request(
                BiologyOperation::ReverseComplement,
                Some("AATTGGCC".into()),
                Some("unknown_orientation"),
                "source_derived_bounded_dna",
                Some("orientation convention is unresolved"),
            ),
            Expected::Ambiguous,
        ));
    }
    for index in 0..20 {
        receipts.push(run(
            format!("refused_rna_{index:03}"),
            request(
                BiologyOperation::ValidateDna,
                Some("AUGC".into()),
                Some("5_to_3"),
                "source_derived_bounded_dna",
                None,
            ),
            Expected::Refused,
        ));
    }
    for index in 0..20 {
        receipts.push(run(
            format!("refused_malformed_{index:03}"),
            request(
                BiologyOperation::ValidateDna,
                Some("ATXG".into()),
                Some("5_to_3"),
                "source_derived_bounded_dna",
                None,
            ),
            Expected::Refused,
        ));
    }
    for index in 0..20 {
        receipts.push(run(
            format!("refused_oversized_{index:03}"),
            request(
                BiologyOperation::BaseComposition,
                Some("A".repeat(257)),
                Some("5_to_3"),
                "source_derived_bounded_dna",
                None,
            ),
            Expected::Refused,
        ));
    }
    for index in 0..20 {
        receipts.push(run(
            format!("refused_domain_{index:03}"),
            request(
                BiologyOperation::ValidateDna,
                Some("ATCG".into()),
                Some("5_to_3"),
                "unvalidated_rna_or_translation_domain",
                None,
            ),
            Expected::Refused,
        ));
    }

    assert_eq!(receipts.len(), 240);
    let cases = receipts.len();
    let supported = receipts.iter().filter(|r| r.expected == Expected::Supported).count();
    let ambiguous = receipts.iter().filter(|r| r.expected == Expected::Ambiguous).count();
    let refused = receipts.iter().filter(|r| r.expected == Expected::Refused).count();
    let exact_decisions = receipts.iter().filter(|r| r.exact).count();
    let supported_artifacts = receipts.iter().filter(|r| r.authorized).count();
    let replay_verified = receipts.iter().filter(|r| r.replay_verified).count();
    let tamper_rejections = receipts.iter().filter(|r| r.tamper_rejected).count();
    let source_preserved = receipts.iter().filter(|r| r.source_preserved).count();
    let false_authorizations = receipts.iter().filter(|r| r.false_authorization).count();
    let false_denials = receipts.iter().filter(|r| r.false_denial).count();
    assert_eq!((supported, ambiguous, refused), (120, 40, 80));
    assert_eq!(exact_decisions, cases);
    assert_eq!(supported_artifacts, supported);
    assert_eq!(replay_verified, cases);
    assert_eq!(tamper_rejections, cases);
    assert_eq!(source_preserved, cases);
    assert_eq!(false_authorizations, 0);
    assert_eq!(false_denials, 0);
    let mut status_counts = BTreeMap::new();
    for receipt in &receipts {
        *status_counts
            .entry(format!("{:?}", receipt.actual_status))
            .or_insert(0usize) += 1;
    }
    let report = Report {
        schema: "stage-h-source-biology-pack-v1",
        source: "OpenStax Biology 2e sections 3.5 and 14.2",
        corpus_sha256: digest(&receipts),
        cases,
        supported,
        ambiguous,
        refused,
        exact_decisions,
        supported_artifacts,
        replay_verified,
        tamper_rejections,
        source_preserved,
        false_authorizations,
        false_denials,
        status_counts,
        receipts,
    };
    let serialized = serde_json::to_string_pretty(&report).expect("biology report serializes");
    std::fs::write("docs/stage_h_source_biology_pack.json", format!("{serialized}\n"))
        .expect("biology report writes");
    println!("{serialized}");
}
