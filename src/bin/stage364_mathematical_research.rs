//! Stage 364: bounded long-horizon mathematical research over real packs.

use serde::Serialize;
use sha2::{Digest, Sha256};
use std::fs;
use the_machine::mathematical_research::{
    evaluate_corpus, independent_corpus, replay_case, run_case, unsupported_corpus, ResearchCase,
    ResearchConclusion, ResearchReceipt,
};

const JSON: &str = "docs/stage364_mathematical_research.json";
const MD: &str = "docs/stage364_mathematical_research.md";

#[derive(Debug, Serialize)]
struct ReceiptSummary {
    id: String,
    claim: String,
    conclusion: ResearchConclusion,
    expected: ResearchConclusion,
    operations: usize,
    counterexamples: usize,
    replans: usize,
    pack_replay: usize,
    receipt_replay: bool,
    tamper_rejected: bool,
}

#[derive(Debug, Serialize)]
struct Report {
    schema: &'static str,
    corpus_sha256: String,
    cases: usize,
    supported: usize,
    refuted: usize,
    unresolved: usize,
    terminal_correct: usize,
    counterexamples_found: usize,
    replans: usize,
    operations: usize,
    pack_replay_verified: usize,
    receipt_replay_verified: usize,
    tamper_rejected: usize,
    false_authorizations: usize,
    false_denials: usize,
    unsupported_cases: usize,
    unsupported_unresolved: usize,
    unsupported_false_authorizations: usize,
    receipts: Vec<ReceiptSummary>,
}

fn hash<T: Serialize>(value: &T) -> String {
    format!("{:x}", Sha256::digest(serde_json::to_vec(value).unwrap()))
}

fn summary(case: &ResearchCase, receipt: &ResearchReceipt) -> ReceiptSummary {
    let mut tampered = receipt.clone();
    if let Some(step) = tampered.steps.first_mut() {
        step.observation.push_str(" tampered");
    }
    ReceiptSummary {
        id: case.id.clone(),
        claim: format!("{:?}", case.claim),
        conclusion: receipt.conclusion,
        expected: case.expected,
        operations: receipt.steps.len(),
        counterexamples: receipt.counterexamples.len(),
        replans: receipt.replans,
        pack_replay: receipt
            .steps
            .iter()
            .filter(|step| step.pack_replay_verified)
            .count(),
        receipt_replay: replay_case(case, receipt),
        tamper_rejected: !replay_case(case, &tampered),
    }
}

fn main() {
    let cases = independent_corpus();
    let unsupported = unsupported_corpus();
    let report = evaluate_corpus(&cases);
    let unsupported_report = evaluate_corpus(&unsupported);
    let receipts = cases
        .iter()
        .map(|case| summary(case, &run_case(case)))
        .collect::<Vec<_>>();
    let output = Report {
        schema: "stage364-mathematical-research-v1",
        corpus_sha256: hash(&cases),
        cases: report.cases,
        supported: report.supported,
        refuted: report.refuted,
        unresolved: report.unresolved,
        terminal_correct: report.terminal_correct,
        counterexamples_found: report.counterexamples_found,
        replans: report.replans,
        operations: report.operations,
        pack_replay_verified: report.pack_replay_verified,
        receipt_replay_verified: report.receipt_replay_verified,
        tamper_rejected: report.tamper_rejected,
        false_authorizations: report.false_authorizations,
        false_denials: report.false_denials,
        unsupported_cases: unsupported_report.cases,
        unsupported_unresolved: unsupported_report.unresolved,
        unsupported_false_authorizations: unsupported_report.false_authorizations,
        receipts,
    };
    let json = serde_json::to_string_pretty(&output).unwrap();
    fs::write(JSON, format!("{json}\n")).unwrap();
    let markdown = format!(
        "# Stage 364 — bounded long-horizon mathematical research\n\n- independent cases / exact terminal decisions: {} / {}\n- supported / refuted / unresolved: {} / {} / {}\n- counterexample-bearing cases / replans: {} / {}\n- pack-backed operations / pack replay: {} / {}\n- receipt replay / tamper rejection: {} / {}\n- false authorizations / denials: {} / {}\n- unsupported control cases / unresolved / false authorization: {} / {} / {}\n- corpus SHA-256: `{}`\n\nThe controller decomposes finite conjectures, invokes actual graph, linear-algebra, and discrete-dynamics packs, inspects typed artifacts, searches the supplied finite family for counterexamples, replans after violations, and stops with a bounded conclusion. It does not infer an unvalidated theorem or mutate any registry.\n\nReproduce with `cargo run --quiet --bin stage364_mathematical_research`.\nMachine-readable report: `{}`\n",
        output.cases,
        output.terminal_correct,
        output.supported,
        output.refuted,
        output.unresolved,
        output.counterexamples_found,
        output.replans,
        output.operations,
        output.pack_replay_verified,
        output.receipt_replay_verified,
        output.tamper_rejected,
        output.false_authorizations,
        output.false_denials,
        output.unsupported_cases,
        output.unsupported_unresolved,
        output.unsupported_false_authorizations,
        output.corpus_sha256,
        JSON,
    );
    fs::write(MD, markdown).unwrap();
    println!(
        "stage364 cases={} correct={} supported={} refuted={} unresolved={} operations={} counterexamples={} replans={} pack_replay={} receipt_replay={} tamper={} false_auth={} false_denials={} unsupported_unresolved={} corpus_hash={}",
        output.cases,
        output.terminal_correct,
        output.supported,
        output.refuted,
        output.unresolved,
        output.operations,
        output.counterexamples_found,
        output.replans,
        output.pack_replay_verified,
        output.receipt_replay_verified,
        output.tamper_rejected,
        output.false_authorizations,
        output.false_denials,
        output.unsupported_unresolved,
        output.corpus_sha256,
    );
}
