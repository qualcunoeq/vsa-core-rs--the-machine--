//! Independent deterministic benchmark for the hybrid semantic boundary.
//!
//! This is intentionally not a language-model benchmark: candidates are
//! generated independently of the validator to measure the typed boundary,
//! ambiguity policy, replay, and tamper resistance before GPU access exists.

use serde::Serialize;
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::fs;
use the_machine::semantic_ir::{
    validate_candidate_ensemble, CandidateSemanticParse, EvidenceSpan, RelationIR, SymbolIR,
    TargetKind, ValidationDecision, SEMANTIC_IR_SCHEMA,
};

#[derive(Debug, Serialize)]
struct BenchReport {
    schema: &'static str,
    corpus_sha256: String,
    cases: usize,
    expected_complete: usize,
    expected_ambiguous: usize,
    expected_unsupported: usize,
    exact_decisions: usize,
    ensemble_replays: usize,
    candidate_replay_eligible: usize,
    candidate_replays: usize,
    validation_receipt_replays: usize,
    tamper_rejections: usize,
    false_authorizations: usize,
    false_denials: usize,
    report_sha256: String,
}

fn digest<T: Serialize>(value: &T) -> String {
    format!(
        "{:x}",
        Sha256::digest(serde_json::to_vec(value).expect("benchmark value serializes"))
    )
}

fn make_candidate(input: &str, target: &str, scope: &str) -> CandidateSemanticParse {
    let start = input.find(target).expect("target fixture span");
    let evidence = EvidenceSpan {
        start,
        end: start + target.len(),
        text: target.into(),
        role: "target".into(),
    };
    CandidateSemanticParse {
        schema: SEMANTIC_IR_SCHEMA.into(),
        input_hash: digest(&input),
        model_id: "independent-fixture".into(),
        model_config_hash: "fixture-config-v1".into(),
        prompt_hash: "fixture-prompt-v1".into(),
        grammar_version: "candidate-json-v1".into(),
        target: target.into(),
        target_kind: TargetKind::Scalar,
        operation: "solve".into(),
        symbols: vec![SymbolIR {
            name: target.into(),
            scope: scope.into(),
            type_name: Some("scalar".into()),
            domain: Some("real".into()),
            declared: true,
            evidence_spans: vec![evidence.clone()],
        }],
        symbol_scopes: BTreeMap::from([(target.into(), scope.into())]),
        equations: vec![RelationIR {
            kind: "constraint".into(),
            expression: format!("{target} = 2"),
            symbols: vec![target.into()],
            evidence_spans: vec![evidence.clone()],
        }],
        assumptions: Vec::new(),
        domains: vec!["real".into()],
        candidate_pack: Some("bounded_scalar".into()),
        unresolved_ambiguities: Vec::new(),
        evidence_spans: vec![evidence],
        confidence: 0.5,
        raw_output_hash: "fixture-output-v1".into(),
        replay_hash: String::new(),
    }
    .with_replay_hash()
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut corpus = Vec::new();
    for index in 0..80 {
        let input = format!("solve x{index}");
        corpus.push((
            input.clone(),
            vec![make_candidate(&input, &format!("x{index}"), "root")],
            ValidationDecision::AcceptCandidate,
        ));
    }
    for index in 80..120 {
        let input = format!("solve x{index}");
        let target = format!("x{index}");
        let mut first = make_candidate(&input, &target, "root");
        let second = make_candidate(&input, &target, "nested");
        first.symbols.push(second.symbols[0].clone());
        first.replay_hash = first.candidate_hash();
        corpus.push((input, vec![first], ValidationDecision::PreserveAmbiguity));
    }
    for index in 120..160 {
        let input = format!("solve x{index}");
        let target = format!("x{index}");
        let mut candidate = make_candidate(&input, &target, "root");
        candidate.unresolved_ambiguities = vec!["target scope is unresolved".into()];
        candidate.replay_hash = candidate.candidate_hash();
        corpus.push((
            input,
            vec![candidate],
            ValidationDecision::PreserveAmbiguity,
        ));
    }
    for index in 160..200 {
        let input = format!("solve x{index}");
        let target = format!("x{index}");
        let mut candidate = make_candidate(&input, &target, "root");
        candidate.evidence_spans[0].text = "missing".into();
        candidate.replay_hash = candidate.candidate_hash();
        corpus.push((input, vec![candidate], ValidationDecision::RejectCandidate));
    }
    for index in 200..240 {
        let input = format!("solve x{index}");
        let target = format!("x{index}");
        let mut candidate = make_candidate(&input, &target, "root");
        candidate.schema = "unknown-schema".into();
        candidate.replay_hash = candidate.candidate_hash();
        corpus.push((input, vec![candidate], ValidationDecision::RejectCandidate));
    }

    let corpus_sha256 = digest(&corpus);
    let mut report = BenchReport {
        schema: "stage486-semantic-boundary-bench-v1",
        corpus_sha256,
        cases: corpus.len(),
        expected_complete: 80,
        expected_ambiguous: 80,
        expected_unsupported: 80,
        exact_decisions: 0,
        ensemble_replays: 0,
        candidate_replay_eligible: 0,
        candidate_replays: 0,
        validation_receipt_replays: 0,
        tamper_rejections: 0,
        false_authorizations: 0,
        false_denials: 0,
        report_sha256: String::new(),
    };
    for (input, candidates, expected) in &corpus {
        let ensemble = validate_candidate_ensemble(input, candidates);
        report.exact_decisions += usize::from(ensemble.decision == *expected);
        report.ensemble_replays += usize::from(ensemble.replay_verified());
        report.candidate_replay_eligible += candidates
            .iter()
            .filter(|candidate| candidate.schema == SEMANTIC_IR_SCHEMA)
            .count();
        report.candidate_replays += candidates
            .iter()
            .filter(|candidate| candidate.replay_verified())
            .count();
        report.validation_receipt_replays += ensemble
            .member_receipts
            .iter()
            .filter(|receipt| receipt.replay_verified())
            .count();
        let mut tampered = candidates[0].clone();
        tampered.target.push('x');
        report.tamper_rejections += usize::from(!tampered.replay_verified());
        report.false_authorizations += usize::from(ensemble.downstream_authorized);
        report.false_denials += usize::from(
            (*expected == ValidationDecision::AcceptCandidate)
                && (ensemble.decision != ValidationDecision::AcceptCandidate),
        );
    }
    report.report_sha256 = digest(&report);
    let json = serde_json::to_string_pretty(&report)?;
    fs::write(
        "docs/stage486_semantic_boundary_bench.json",
        format!("{json}\n"),
    )?;
    fs::write(
        "docs/stage486_semantic_boundary_bench.md",
        format!(
            "# Stage 486 — semantic boundary benchmark\n\nDeterministic candidate fixtures independently exercise the hybrid semantic IR; this is not a neural-quality result.\n\n* cases: {cases}\n* expected complete / ambiguous / unsupported: 80 / 80 / 80\n* exact decisions: {exact}/240\n* ensemble replay: {ensemble}/240\n* candidate replay: {candidate}/{eligible} schema-valid candidates\n* validation-receipt replay: {receipts}/240\n* tamper rejections: {tamper}/240\n* false authorizations / denials: {auth} / {denial}\n* corpus SHA-256: `{corpus}`\n* report SHA-256: `{report}`\n",
            cases = report.cases,
            exact = report.exact_decisions,
            ensemble = report.ensemble_replays,
            candidate = report.candidate_replays,
            eligible = report.candidate_replay_eligible,
            receipts = report.validation_receipt_replays,
            tamper = report.tamper_rejections,
            auth = report.false_authorizations,
            denial = report.false_denials,
            corpus = report.corpus_sha256,
            report = report.report_sha256,
        ),
    )?;
    println!("{json}");
    Ok(())
}
