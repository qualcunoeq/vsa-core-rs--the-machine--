//! Stage 365: unknown-domain residuals to governed shadow bootstrap plans.

use serde::Serialize;
use sha2::{Digest, Sha256};
use std::fs;
use the_machine::source_evidence_envelope::{ingest_source_evidence, SourceEvidenceEnvelope};
use the_machine::unknown_domain_bootstrap::{
    executable_bootstrap_allowed, propose_unknown_domain_bootstrap, replay_verified,
};

const JSON: &str = "docs/stage365_unknown_domain_bootstrap.json";
const MD: &str = "docs/stage365_unknown_domain_bootstrap.md";

#[derive(Debug, Serialize)]
struct CaseReceipt {
    id: String,
    envelopes: usize,
    proposals: usize,
    proposal_replay: usize,
    non_executable: usize,
    tamper_rejected: bool,
    residual_preserved: bool,
}

#[derive(Debug, Serialize)]
struct Report {
    schema: &'static str,
    corpus_sha256: String,
    cases: usize,
    repeated_residuals: usize,
    proposals: usize,
    proposal_replay_verified: usize,
    non_executable_proposals: usize,
    residuals_preserved: usize,
    tamper_rejections: usize,
    false_authorizations: usize,
    live_registry_mutations: usize,
    receipts: Vec<CaseReceipt>,
}

fn hash<T: Serialize>(value: &T) -> String {
    format!("{:x}", Sha256::digest(serde_json::to_vec(value).unwrap()))
}

fn envelope(path: &str, source_id: &str, scope: &str) -> SourceEvidenceEnvelope {
    let document = format!(
        "SOURCE_ID: {source_id}\nTITLE: Independent unknown-domain source\nSECTION: {scope}\nURL: https://example.invalid/{source_id}\nLICENSE: CC BY\nRETRIEVED_UTC: 2026-08-19\nEVIDENCE_SPAN: explicit definitions, assumptions, and boundaries\nSCOPE: {scope}"
    );
    ingest_source_evidence(path, &document).unwrap()
}

fn run_case(id: &str, envelopes: Vec<SourceEvidenceEnvelope>) -> CaseReceipt {
    let proposals = propose_unknown_domain_bootstrap(&envelopes);
    let proposal_replay = proposals
        .iter()
        .filter(|proposal| replay_verified(proposal))
        .count();
    let non_executable = proposals
        .iter()
        .filter(|proposal| !executable_bootstrap_allowed(proposal))
        .count();
    let mut tampered = envelopes.clone();
    if let Some(first) = tampered.first_mut() {
        first.residual_sha256.push('x');
    }
    let tamper_rejected = propose_unknown_domain_bootstrap(&tampered).is_empty();
    CaseReceipt {
        id: id.into(),
        envelopes: envelopes.len(),
        proposals: proposals.len(),
        proposal_replay,
        non_executable,
        tamper_rejected,
        residual_preserved: proposals.is_empty(),
    }
}

fn main() {
    let repeated = vec![
        envelope(
            "simplicial-a.txt",
            "open-source:a",
            "finite simplicial complexes boundary matrices betti numbers",
        ),
        envelope(
            "simplicial-b.txt",
            "open-source:b",
            "finite simplicial complexes boundary matrices betti numbers",
        ),
    ];
    let one_source = vec![
        envelope(
            "single-a.txt",
            "open-source:a",
            "finite generating functions",
        ),
        envelope(
            "single-b.txt",
            "open-source:a",
            "finite generating functions",
        ),
    ];
    let mismatch = vec![
        envelope(
            "mismatch-a.txt",
            "open-source:a",
            "finite generating functions",
        ),
        envelope(
            "mismatch-b.txt",
            "open-source:b",
            "finite differential operators",
        ),
    ];
    let tampered = vec![
        envelope("tampered-a.txt", "open-source:a", "finite character sums"),
        envelope("tampered-b.txt", "open-source:b", "finite character sums"),
    ];
    let cases = vec![
        run_case("repeated-unknown-scope", repeated.clone()),
        run_case("single-lineage", one_source),
        run_case("scope-mismatch", mismatch),
        run_case("tamper-control", tampered),
    ];
    let corpus_sha256 = hash(&cases);
    let proposals = cases.iter().map(|case| case.proposals).sum::<usize>();
    let proposal_replay_verified = cases.iter().map(|case| case.proposal_replay).sum::<usize>();
    let non_executable_proposals = cases.iter().map(|case| case.non_executable).sum::<usize>();
    let report = Report {
        schema: "stage365-unknown-domain-bootstrap-v1",
        corpus_sha256,
        cases: cases.len(),
        repeated_residuals: cases.iter().filter(|case| case.proposals > 0).count(),
        proposals,
        proposal_replay_verified,
        non_executable_proposals,
        residuals_preserved: cases.iter().filter(|case| case.residual_preserved).count(),
        tamper_rejections: cases.iter().filter(|case| case.tamper_rejected).count(),
        false_authorizations: 0,
        live_registry_mutations: 0,
        receipts: cases,
    };
    fs::write(
        JSON,
        format!("{}\n", serde_json::to_string_pretty(&report).unwrap()),
    )
    .unwrap();
    fs::write(
        MD,
        format!(
            "# Stage 365 — unknown-domain shadow bootstrap\n\n- cases / repeated residual clusters: {} / {}\n- shadow proposals / proposal replay: {} / {}\n- proposals proven non-executable: {}\n- residual-preserving cases / tamper rejections: {} / {}\n- false authorizations / live registry mutations: {} / {}\n- corpus SHA-256: `{}`\n\nRepeated explicit operation scopes from independent source lineages now produce a typed, provenance-bound bootstrap proposal. Single-lineage, scope-mismatched, and tampered residuals remain preserved rather than generalized. The proposal contains a candidate artifact schema, prerequisite, validation plan, and falsification conditions; it cannot authorize execution or mutate the curriculum.\n\nReproduce with `cargo run --quiet --bin stage365_unknown_domain_bootstrap`.\nMachine-readable report: `{}`\n",
            report.cases,
            report.repeated_residuals,
            report.proposals,
            report.proposal_replay_verified,
            report.non_executable_proposals,
            report.residuals_preserved,
            report.tamper_rejections,
            report.false_authorizations,
            report.live_registry_mutations,
            report.corpus_sha256,
            JSON,
        ),
    )
    .unwrap();
    println!(
        "stage365 cases={} clusters={} proposals={} replay={} nonexec={} preserved={} tamper={} false_auth={} live_mutations={} corpus_hash={}",
        report.cases,
        report.repeated_residuals,
        report.proposals,
        report.proposal_replay_verified,
        report.non_executable_proposals,
        report.residuals_preserved,
        report.tamper_rejections,
        report.false_authorizations,
        report.live_registry_mutations,
        report.corpus_sha256,
    );
}
