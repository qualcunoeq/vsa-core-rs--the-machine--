//! Phase 71 bounded finite Markov curriculum validation.

use serde::Serialize;
use sha2::{Digest, Sha256};
use std::fs;
use the_machine::finite_markov_pack::{
    evaluate_markov, MarkovArtifact, MarkovOperation, MarkovRequest, MarkovResult, MarkovStatus,
};
use the_machine::probability_pack::Rational;

fn q(n: i128, d: i128) -> Rational {
    Rational::new(n, d).expect("valid rational")
}

fn request(operation: MarkovOperation) -> MarkovRequest {
    MarkovRequest {
        operation,
        domain: "finite_exact_markov_chain".into(),
        initial: vec![q(1, 1), q(0, 1)],
        transition: vec![vec![q(3, 4), q(1, 4)], vec![q(1, 2), q(1, 2)]],
        steps: 1,
        row_stochastic: Some(true),
        ambiguity: None,
        provenance: vec!["phase71-independent-finite-markov-corpus".into()],
    }
}

#[derive(Serialize)]
struct Receipt {
    id: String,
    expected: MarkovStatus,
    actual: MarkovStatus,
    expected_artifact: Option<MarkovArtifact>,
    actual_artifact: Option<MarkovArtifact>,
    exact: bool,
    replay: bool,
    tamper_rejected: bool,
    false_authorization: bool,
}

#[derive(Serialize)]
struct Report {
    schema: &'static str,
    corpus_sha256: String,
    cases: usize,
    supported: usize,
    ambiguous: usize,
    refused: usize,
    exact_decisions: usize,
    replay_verified: usize,
    tamper_rejections: usize,
    false_authorizations: usize,
    false_denials: usize,
    receipts: Vec<Receipt>,
}

fn hash<T: Serialize>(value: &T) -> String {
    format!("{:x}", Sha256::digest(serde_json::to_vec(value).unwrap()))
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut cases: Vec<(String, MarkovRequest, MarkovStatus, Option<MarkovArtifact>)> = Vec::new();
    let one_step = Some(MarkovArtifact::Distribution(vec![q(3, 4), q(1, 4)]));
    for i in 0..40 {
        cases.push((
            format!("one_step_{i}"),
            request(MarkovOperation::OneStep),
            MarkovStatus::Complete,
            one_step.clone(),
        ));
    }
    let mut horizon = request(MarkovOperation::FiniteHorizon);
    horizon.steps = 3;
    let horizon_artifact = Some(MarkovArtifact::Trace(vec![
        vec![q(3, 4), q(1, 4)],
        vec![q(11, 16), q(5, 16)],
        vec![q(43, 64), q(21, 64)],
    ]));
    for i in 0..40 {
        cases.push((
            format!("finite_horizon_{i}"),
            horizon.clone(),
            MarkovStatus::Complete,
            horizon_artifact.clone(),
        ));
    }
    let stationary_artifact = Some(MarkovArtifact::Distribution(vec![q(2, 3), q(1, 3)]));
    for i in 0..40 {
        cases.push((
            format!("stationary_{i}"),
            request(MarkovOperation::StationaryDistribution),
            MarkovStatus::Complete,
            stationary_artifact.clone(),
        ));
    }
    for i in 0..20 {
        let mut r = request(MarkovOperation::OneStep);
        r.row_stochastic = None;
        cases.push((
            format!("missing_convention_{i}"),
            r,
            MarkovStatus::Ambiguous,
            None,
        ));
    }
    for i in 0..20 {
        let mut r = request(MarkovOperation::StationaryDistribution);
        r.transition = vec![vec![q(1, 1), q(0, 1)], vec![q(0, 1), q(1, 1)]];
        cases.push((
            format!("nonunique_stationary_{i}"),
            r,
            MarkovStatus::NonUniqueStationary,
            None,
        ));
    }
    for i in 0..20 {
        let mut r = request(MarkovOperation::FiniteHorizon);
        r.steps = 9;
        cases.push((
            format!("over_budget_{i}"),
            r,
            MarkovStatus::BudgetExceeded,
            None,
        ));
    }
    for i in 0..20 {
        let mut r = request(MarkovOperation::StationaryDistribution);
        r.transition = vec![
            vec![q(1, 2), q(1, 4), q(1, 4)],
            vec![q(1, 3), q(1, 3), q(1, 3)],
            vec![q(1, 4), q(1, 4), q(1, 2)],
        ];
        r.initial = vec![q(1, 3), q(1, 3), q(1, 3)];
        cases.push((
            format!("larger_stationary_{i}"),
            r,
            MarkovStatus::Unsupported,
            None,
        ));
    }
    for i in 0..40 {
        let mut r = request(MarkovOperation::OneStep);
        r.transition[0][0] = q(3, 2);
        cases.push((
            format!("invalid_transition_{i}"),
            r,
            MarkovStatus::InvalidTransition,
            None,
        ));
    }
    assert_eq!(cases.len(), 240);
    let corpus_sha256 = hash(&cases);
    let mut receipts = Vec::with_capacity(cases.len());
    for (id, request, expected, expected_artifact) in cases {
        let output: MarkovResult = evaluate_markov(&request);
        let mut tampered = output.clone();
        tampered.replay_hash.push('x');
        let exact = output.status == expected && output.artifact == expected_artifact;
        let false_authorization = expected != MarkovStatus::Complete && output.artifact.is_some();
        let replay = output.replay_verified();
        let actual_artifact = output.artifact.clone();
        receipts.push(Receipt {
            id,
            expected,
            actual: output.status,
            expected_artifact,
            actual_artifact,
            exact,
            replay,
            tamper_rejected: !tampered.replay_verified(),
            false_authorization,
        });
    }
    let cases = receipts.len();
    let supported = receipts
        .iter()
        .filter(|r| r.expected == MarkovStatus::Complete)
        .count();
    let ambiguous = receipts
        .iter()
        .filter(|r| r.expected == MarkovStatus::Ambiguous)
        .count();
    let refused = cases - supported - ambiguous;
    let exact_decisions = receipts.iter().filter(|r| r.exact).count();
    let replay_verified = receipts.iter().filter(|r| r.replay).count();
    let tamper_rejections = receipts.iter().filter(|r| r.tamper_rejected).count();
    let false_authorizations = receipts.iter().filter(|r| r.false_authorization).count();
    let false_denials = receipts
        .iter()
        .filter(|r| r.expected == MarkovStatus::Complete && !r.exact)
        .count();
    assert_eq!(
        (
            exact_decisions,
            replay_verified,
            tamper_rejections,
            false_authorizations,
            false_denials
        ),
        (240, 240, 240, 0, 0)
    );
    let report = Report {
        schema: "phase71-finite-markov-pack-v1",
        corpus_sha256,
        cases,
        supported,
        ambiguous,
        refused,
        exact_decisions,
        replay_verified,
        tamper_rejections,
        false_authorizations,
        false_denials,
        receipts,
    };
    fs::write(
        "docs/stage_a_finite_markov_pack.json",
        serde_json::to_vec_pretty(&report)?,
    )?;
    println!("{}", serde_json::to_string_pretty(&report)?);
    Ok(())
}
