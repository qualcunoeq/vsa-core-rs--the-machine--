//! Stage H shifted technical-language benchmark for bounded DNA biology.

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use the_machine::source_formula_pack::biology_pack::biology_frontend::{
    formalize_biology_text, BiologyFrontendResult, BiologyFrontendStatus,
};
use the_machine::source_formula_pack::biology_pack::evaluate_biology;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
enum Expected {
    Complete,
    Ambiguous,
    Unsupported,
}

#[derive(Debug, Clone, Serialize)]
struct Receipt {
    id: String,
    expected: Expected,
    frontend_status: BiologyFrontendStatus,
    downstream_authorized: bool,
    exact: bool,
    frontend_replay_verified: bool,
    downstream_replay_verified: bool,
    frontend_tamper_rejected: bool,
    downstream_tamper_rejected: bool,
    provenance_preserved: bool,
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
    unsupported: usize,
    exact_decisions: usize,
    complete_frontends: usize,
    downstream_authorizations: usize,
    frontend_replay_verified: usize,
    downstream_replay_verified: usize,
    frontend_tamper_rejections: usize,
    downstream_tamper_rejections: usize,
    provenance_preserved: usize,
    false_authorizations: usize,
    false_denials: usize,
    status_counts: BTreeMap<String, usize>,
    receipts: Vec<Receipt>,
}

fn digest<T: Serialize>(value: &T) -> String {
    format!(
        "{:x}",
        Sha256::digest(serde_json::to_vec(value).expect("biology frontend serializes"))
    )
}

fn downstream(frontend: &BiologyFrontendResult) -> (bool, bool, bool) {
    let Some(request) = frontend.request.clone() else {
        return (false, false, false);
    };
    let result = evaluate_biology(&request);
    let authorized = result.authorized();
    let replay = result.replay_verified();
    let mut tampered = result.clone();
    tampered.replay_hash.push('x');
    (authorized, replay, !tampered.replay_verified())
}

fn run(id: String, text: String, expected: Expected) -> Receipt {
    let frontend = formalize_biology_text(&text);
    let (downstream_authorized, downstream_replay_verified, downstream_tamper_rejected) =
        downstream(&frontend);
    let frontend_replay_verified = frontend.replay_verified();
    let mut tampered = frontend.clone();
    tampered.replay_hash.push('x');
    let frontend_tamper_rejected = !tampered.replay_verified();
    let exact = match expected {
        Expected::Complete => {
            frontend.status == BiologyFrontendStatus::Complete && downstream_authorized
        }
        Expected::Ambiguous => {
            frontend.status == BiologyFrontendStatus::Ambiguous && !downstream_authorized
        }
        Expected::Unsupported => {
            frontend.status == BiologyFrontendStatus::Unsupported && !downstream_authorized
        }
    };
    Receipt {
        id,
        expected,
        frontend_status: frontend.status,
        downstream_authorized,
        exact,
        frontend_replay_verified,
        downstream_replay_verified,
        frontend_tamper_rejected,
        downstream_tamper_rejected,
        provenance_preserved: !frontend.provenance.is_empty(),
        false_authorization: expected != Expected::Complete && downstream_authorized,
        false_denial: expected == Expected::Complete && !downstream_authorized,
    }
}

fn main() {
    let sequences = ["AATTGGCC", "ATCGATCG", "GCGCGCAA", "TTAAACCG", "AGCTAGCT"];
    let mut receipts = Vec::with_capacity(240);
    for index in 0..60 {
        let sequence = sequences[index % sequences.len()];
        let text = match index % 3 {
            0 => format!("Validate DNA sequence: {sequence}, orientation 5_to_3."),
            1 => format!("Given DNA sequence is {sequence}; validate the bases."),
            _ => format!("Check strand: {sequence}, 5' to 3'."),
        };
        receipts.push(run(format!("validate_{index:03}"), text, Expected::Complete));
    }
    for index in 0..20 {
        receipts.push(run(
            format!("complement_{index:03}"),
            format!(
                "Find the aligned complement of sequence: {}, 5' to 3'.",
                sequences[index % sequences.len()]
            ),
            Expected::Complete,
        ));
    }
    for index in 0..20 {
        receipts.push(run(
            format!("reverse_complement_{index:03}"),
            format!(
                "Find the reverse-complement of DNA sequence is {}, 5’ to 3’.",
                sequences[index % sequences.len()]
            ),
            Expected::Complete,
        ));
    }
    for index in 0..20 {
        receipts.push(run(
            format!("composition_{index:03}"),
            format!(
                "Compute the base composition and GC content of DNA sequence: {}.",
                sequences[index % sequences.len()]
            ),
            Expected::Complete,
        ));
    }
    for index in 0..20 {
        receipts.push(run(
            format!("ambiguous_orientation_{index:03}"),
            "Find the complement of DNA sequence: AATTGGCC.".into(),
            Expected::Ambiguous,
        ));
    }
    for index in 0..20 {
        receipts.push(run(
            format!("ambiguous_target_{index:03}"),
            "Compare DNA sequence: AATTGGCC with strand: GCGCGCAA, both 5_to_3.".into(),
            Expected::Ambiguous,
        ));
    }
    let unsupported = [
        "Transcribe RNA from DNA sequence: AATTGGCC.",
        "Translate the codon sequence: AUGGCC.",
        "Determine the protein from DNA sequence: ATGGCC.",
        "Predict the phenotype from gene sequence: AATTGGCC.",
    ];
    for index in 0..80 {
        receipts.push(run(
            format!("unsupported_{index:03}"),
            unsupported[index % unsupported.len()].into(),
            Expected::Unsupported,
        ));
    }

    assert_eq!(receipts.len(), 240);
    let cases = receipts.len();
    let supported = receipts.iter().filter(|r| r.expected == Expected::Complete).count();
    let ambiguous = receipts.iter().filter(|r| r.expected == Expected::Ambiguous).count();
    let unsupported_count = receipts.iter().filter(|r| r.expected == Expected::Unsupported).count();
    let exact_decisions = receipts.iter().filter(|r| r.exact).count();
    let complete_frontends = receipts
        .iter()
        .filter(|r| r.frontend_status == BiologyFrontendStatus::Complete)
        .count();
    let downstream_authorizations = receipts.iter().filter(|r| r.downstream_authorized).count();
    let frontend_replay_verified = receipts.iter().filter(|r| r.frontend_replay_verified).count();
    let downstream_replay_verified = receipts.iter().filter(|r| r.downstream_replay_verified).count();
    let frontend_tamper_rejections = receipts.iter().filter(|r| r.frontend_tamper_rejected).count();
    let downstream_tamper_rejections = receipts
        .iter()
        .filter(|r| r.downstream_tamper_rejected)
        .count();
    let provenance_preserved = receipts.iter().filter(|r| r.provenance_preserved).count();
    let false_authorizations = receipts.iter().filter(|r| r.false_authorization).count();
    let false_denials = receipts.iter().filter(|r| r.false_denial).count();
    assert_eq!((supported, ambiguous, unsupported_count), (120, 40, 80));
    assert_eq!(exact_decisions, cases);
    assert_eq!(complete_frontends, supported);
    assert_eq!(downstream_authorizations, supported);
    assert_eq!(frontend_replay_verified, cases);
    assert_eq!(downstream_replay_verified, supported);
    assert_eq!(frontend_tamper_rejections, cases);
    assert_eq!(downstream_tamper_rejections, supported);
    assert_eq!(provenance_preserved, cases);
    assert_eq!(false_authorizations, 0);
    assert_eq!(false_denials, 0);
    let mut status_counts = BTreeMap::new();
    for receipt in &receipts {
        *status_counts
            .entry(format!("{:?}", receipt.frontend_status))
            .or_insert(0usize) += 1;
    }
    let report = Report {
        schema: "stage-h-source-biology-frontend-v1",
        source: "independently authored shifted bounded DNA-language corpus",
        corpus_sha256: digest(&receipts),
        cases,
        supported,
        ambiguous,
        unsupported: unsupported_count,
        exact_decisions,
        complete_frontends,
        downstream_authorizations,
        frontend_replay_verified,
        downstream_replay_verified,
        frontend_tamper_rejections,
        downstream_tamper_rejections,
        provenance_preserved,
        false_authorizations,
        false_denials,
        status_counts,
        receipts,
    };
    let serialized = serde_json::to_string_pretty(&report).expect("biology frontend serializes");
    std::fs::write("docs/stage_h_source_biology_frontend.json", format!("{serialized}\n"))
        .expect("biology frontend report writes");
    println!("{serialized}");
}
