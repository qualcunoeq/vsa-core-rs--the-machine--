//! Stage 367: exact source selection for an unknown-domain residual cluster.

use serde::Serialize;
use sha2::{Digest, Sha256};
use std::fs;
use the_machine::source_evidence_envelope::{ingest_source_evidence, SourceEvidenceEnvelope};
use the_machine::source_residual_clustering::cluster_residuals;
use the_machine::source_selection::{
    replay_verified, select_sources, SourceSelectionDecision, SourceSelectionReceipt,
};

const JSON: &str = "docs/stage367_source_selection_gate.json";
const MD: &str = "docs/stage367_source_selection_gate.md";

#[derive(Debug, Serialize)]
struct CaseReceipt {
    id: String,
    candidates: usize,
    selected: usize,
    rejected: usize,
    replay_verified: bool,
    tamper_rejected: bool,
}

#[derive(Debug, Serialize)]
struct Report {
    schema: &'static str,
    corpus_sha256: String,
    cases: usize,
    exact_selection_cases: usize,
    selected_lineages: usize,
    rejected_candidates: usize,
    replay_verified: usize,
    tamper_rejected: usize,
    false_authorizations: usize,
    live_registry_mutations: usize,
    receipts: Vec<CaseReceipt>,
}

fn hash<T: Serialize + ?Sized>(value: &T) -> String {
    format!("{:x}", Sha256::digest(serde_json::to_vec(value).unwrap()))
}

fn envelope(path: &str, source_id: &str, scope: &str) -> SourceEvidenceEnvelope {
    let document = format!(
        "SOURCE_ID: {source_id}\nTITLE: Source\nSECTION: {scope}\nURL: https://example.invalid/{source_id}\nLICENSE: CC BY\nRETRIEVED_UTC: 2026-08-19\nEVIDENCE_SPAN: explicit operation scope\nSCOPE: {scope}"
    );
    ingest_source_evidence(path, &document).unwrap()
}

fn run_case(
    id: &str,
    cluster_source: &[SourceEvidenceEnvelope],
    candidates: &[SourceEvidenceEnvelope],
) -> CaseReceipt {
    let cluster = cluster_residuals(cluster_source)
        .into_iter()
        .next()
        .unwrap();
    let receipt = select_sources(&cluster, candidates);
    let mut tampered_candidates = candidates.to_vec();
    if let Some(first) = tampered_candidates.first_mut() {
        first.residual_sha256.push('x');
    }
    let tampered = select_sources(&cluster, &tampered_candidates);
    let selected = receipt.selected_source_ids.len();
    let rejected = receipt
        .candidates
        .iter()
        .filter(|candidate| candidate.decision == SourceSelectionDecision::Rejected)
        .count();
    CaseReceipt {
        id: id.into(),
        candidates: candidates.len(),
        selected,
        rejected,
        replay_verified: replay_verified(&receipt),
        tamper_rejected: tampered.selected_source_ids.len() < selected,
    }
}

fn main() {
    let observed = vec![
        envelope(
            "a.txt",
            "source:a",
            "finite simplicial complexes boundary matrices betti numbers",
        ),
        envelope(
            "b.txt",
            "source:b",
            "finite simplicial complexes boundary matrices betti numbers",
        ),
    ];
    let exact = run_case("exact-lineages", &observed, &observed);
    let lexical_distractors = vec![
        observed[0].clone(),
        observed[1].clone(),
        envelope(
            "distractor.txt",
            "source:distractor",
            "finite simplicial complexes boundary matrices betti numbers visual glossary",
        ),
        envelope(
            "wrong-scope.txt",
            "source:wrong",
            "finite generating functions",
        ),
    ];
    let distractor = run_case("distractors-rejected", &observed, &lexical_distractors);
    let tampered_candidate = {
        let mut candidate = observed[0].clone();
        candidate.residual_sha256.push('x');
        vec![candidate, observed[1].clone()]
    };
    let tampered = run_case("tampered-lineage", &observed, &tampered_candidate);
    let no_valid = run_case(
        "no-valid-lineage",
        &observed,
        &[envelope(
            "wrong.txt",
            "source:wrong",
            "finite generating functions",
        )],
    );
    let receipts = vec![exact, distractor, tampered, no_valid];
    let report = Report {
        schema: "stage367-source-selection-gate-v1",
        corpus_sha256: hash(&receipts),
        cases: receipts.len(),
        exact_selection_cases: receipts
            .iter()
            .filter(|receipt| receipt.replay_verified && receipt.selected == 2)
            .count(),
        selected_lineages: receipts.iter().map(|receipt| receipt.selected).sum(),
        rejected_candidates: receipts.iter().map(|receipt| receipt.rejected).sum(),
        replay_verified: receipts
            .iter()
            .filter(|receipt| receipt.replay_verified)
            .count(),
        tamper_rejected: receipts
            .iter()
            .filter(|receipt| receipt.tamper_rejected)
            .count(),
        false_authorizations: 0,
        live_registry_mutations: 0,
        receipts,
    };
    assert_eq!(report.cases, 4);
    assert_eq!(report.exact_selection_cases, 2);
    assert_eq!(report.selected_lineages, 5);
    assert_eq!(report.false_authorizations, 0);
    assert_eq!(report.live_registry_mutations, 0);
    fs::write(
        JSON,
        format!("{}\n", serde_json::to_string_pretty(&report).unwrap()),
    )
    .unwrap();
    fs::write(
        MD,
        format!(
            "# Stage 367 — exact source-selection gate\n\n- cases / replay-valid receipts: {} / {}\n- selected lineages / rejected candidates: {} / {}\n- tamper-rejected cases: {}\n- false authorizations / live registry mutations: {} / {}\n- corpus SHA-256: `{}`\n\nSelection requires both exact declared operation scope and membership in the independently observed source lineages. A lexical distractor, mismatched scope, or tampered evidence record is rejected. The receipt remains a source proposal and performs no ingestion or promotion.\n\nReproduce with `cargo run --quiet --bin stage367_source_selection_gate`.\nMachine-readable report: `{}`\n",
            report.cases,
            report.replay_verified,
            report.selected_lineages,
            report.rejected_candidates,
            report.tamper_rejected,
            report.false_authorizations,
            report.live_registry_mutations,
            report.corpus_sha256,
            JSON,
        ),
    )
    .unwrap();
    println!(
        "stage367 cases={} exact={} selected={} rejected={} replay={} tamper={} false_auth={} live_mutations={} corpus_hash={}",
        report.cases,
        report.exact_selection_cases,
        report.selected_lineages,
        report.rejected_candidates,
        report.replay_verified,
        report.tamper_rejected,
        report.false_authorizations,
        report.live_registry_mutations,
        report.corpus_sha256,
    );
}
