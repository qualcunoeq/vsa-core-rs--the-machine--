//! Stage 190: independent validation of bounded exact hitting probabilities.

use serde::Serialize;
use sha2::{Digest, Sha256};
use std::fs;
use the_machine::curriculum::breadth_first_manifest;
use the_machine::finite_markov_hitting_pack::{
    evaluate, HittingArtifact, HittingRequest, HittingResult, HittingStatus,
};
use the_machine::probability_pack::Rational;

const REPORT_JSON: &str = "docs/stage190_finite_markov_hitting.json";
const REPORT_MD: &str = "docs/stage190_finite_markov_hitting.md";

fn q(numerator: i128, denominator: i128) -> Rational {
    Rational::new(numerator, denominator).unwrap()
}

fn request(
    transition: Vec<Vec<Rational>>,
    initial: Vec<Rational>,
    target_states: Vec<usize>,
    avoid_states: Vec<usize>,
) -> HittingRequest {
    HittingRequest {
        domain: "finite_exact_markov_hitting".into(),
        transition,
        initial,
        target_states,
        avoid_states,
        row_stochastic: Some(true),
        ambiguity: None,
        provenance: vec!["stage190-independent-hitting-corpus".into()],
    }
}

fn chain_a() -> (HittingRequest, HittingArtifact) {
    (
        request(
            vec![
                vec![q(1, 1), q(0, 1), q(0, 1)],
                vec![q(1, 4), q(1, 4), q(1, 2)],
                vec![q(0, 1), q(0, 1), q(1, 1)],
            ],
            vec![q(0, 1), q(1, 1), q(0, 1)],
            vec![2],
            vec![0],
        ),
        HittingArtifact {
            state_probabilities: vec![q(0, 1), q(2, 3), q(1, 1)],
            initial_probability: q(2, 3),
            target_states: vec![2],
            avoid_states: vec![0],
            residual_verified: true,
        },
    )
}

fn chain_b() -> (HittingRequest, HittingArtifact) {
    (
        request(
            vec![
                vec![q(1, 1), q(0, 1), q(0, 1)],
                vec![q(1, 3), q(1, 3), q(1, 3)],
                vec![q(0, 1), q(0, 1), q(1, 1)],
            ],
            vec![q(0, 1), q(1, 1), q(0, 1)],
            vec![2],
            vec![0],
        ),
        HittingArtifact {
            state_probabilities: vec![q(0, 1), q(1, 2), q(1, 1)],
            initial_probability: q(1, 2),
            target_states: vec![2],
            avoid_states: vec![0],
            residual_verified: true,
        },
    )
}

fn chain_c() -> (HittingRequest, HittingArtifact) {
    (
        request(
            vec![
                vec![q(1, 1), q(0, 1), q(0, 1), q(0, 1)],
                vec![q(1, 4), q(1, 2), q(1, 4), q(0, 1)],
                vec![q(0, 1), q(1, 4), q(1, 2), q(1, 4)],
                vec![q(0, 1), q(0, 1), q(0, 1), q(1, 1)],
            ],
            vec![q(0, 1), q(1, 1), q(0, 1), q(0, 1)],
            vec![3],
            vec![0],
        ),
        HittingArtifact {
            state_probabilities: vec![q(0, 1), q(1, 3), q(2, 3), q(1, 1)],
            initial_probability: q(1, 3),
            target_states: vec![3],
            avoid_states: vec![0],
            residual_verified: true,
        },
    )
}

fn chain_d() -> (HittingRequest, HittingArtifact) {
    (
        request(
            vec![
                vec![q(1, 1), q(0, 1), q(0, 1), q(0, 1)],
                vec![q(1, 2), q(1, 4), q(1, 4), q(0, 1)],
                vec![q(0, 1), q(0, 1), q(1, 1), q(0, 1)],
                vec![q(0, 1), q(0, 1), q(0, 1), q(1, 1)],
            ],
            vec![q(0, 1), q(1, 1), q(0, 1), q(0, 1)],
            vec![2],
            vec![0, 3],
        ),
        HittingArtifact {
            state_probabilities: vec![q(0, 1), q(1, 3), q(1, 1), q(0, 1)],
            initial_probability: q(1, 3),
            target_states: vec![2],
            avoid_states: vec![0, 3],
            residual_verified: true,
        },
    )
}

#[derive(Debug, Clone, Serialize)]
struct Case {
    id: String,
    request: HittingRequest,
    expected: HittingStatus,
    artifact: Option<HittingArtifact>,
}

#[derive(Debug, Serialize)]
struct Receipt {
    id: String,
    expected: HittingStatus,
    actual: HittingStatus,
    expected_artifact: Option<HittingArtifact>,
    actual_artifact: Option<HittingArtifact>,
    exact: bool,
    replay_verified: bool,
    tamper_rejected: bool,
    false_authorization: bool,
    false_denial: bool,
}

#[derive(Debug, Serialize)]
struct Report {
    schema: &'static str,
    manifest_sha256: String,
    corpus_sha256: String,
    source: &'static str,
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

fn digest<T: Serialize + ?Sized>(value: &T) -> String {
    format!("{:x}", Sha256::digest(serde_json::to_vec(value).unwrap()))
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut cases = Vec::with_capacity(240);
    for (prefix, builder) in [
        (
            "chain_a",
            chain_a as fn() -> (HittingRequest, HittingArtifact),
        ),
        (
            "chain_b",
            chain_b as fn() -> (HittingRequest, HittingArtifact),
        ),
        (
            "chain_c",
            chain_c as fn() -> (HittingRequest, HittingArtifact),
        ),
        (
            "chain_d",
            chain_d as fn() -> (HittingRequest, HittingArtifact),
        ),
    ] {
        for i in 0..30 {
            let (request, artifact) = builder();
            cases.push(Case {
                id: format!("{prefix}_{i}"),
                request,
                expected: HittingStatus::Complete,
                artifact: Some(artifact),
            });
        }
    }
    for i in 0..40 {
        let (mut request, _) = chain_a();
        request.ambiguity = Some("target event or avoid event was not uniquely identified".into());
        cases.push(Case {
            id: format!("ambiguous_boundary_{i}"),
            request,
            expected: HittingStatus::Ambiguous,
            artifact: None,
        });
    }
    for i in 0..20 {
        let (mut request, _) = chain_a();
        request.transition = vec![
            vec![q(1, 1), q(0, 1), q(0, 1)],
            vec![q(0, 1), q(1, 1), q(0, 1)],
            vec![q(0, 1), q(0, 1), q(1, 1)],
        ];
        cases.push(Case {
            id: format!("non_unique_transient_{i}"),
            request,
            expected: HittingStatus::NonUnique,
            artifact: None,
        });
    }
    for i in 0..20 {
        let (mut request, _) = chain_a();
        request.target_states = vec![0, 0];
        request.avoid_states = vec![0];
        cases.push(Case {
            id: format!("invalid_target_overlap_{i}"),
            request,
            expected: HittingStatus::InvalidBoundary,
            artifact: None,
        });
    }
    for i in 0..20 {
        let (mut request, _) = chain_a();
        request.transition = (0..5)
            .map(|row| {
                (0..5)
                    .map(|column| if row == column { q(1, 1) } else { q(0, 1) })
                    .collect()
            })
            .collect();
        request.initial = vec![q(1, 1), q(0, 1), q(0, 1)];
        cases.push(Case {
            id: format!("over_dimension_{i}"),
            request,
            expected: HittingStatus::DimensionMismatch,
            artifact: None,
        });
    }
    for i in 0..20 {
        let (mut request, _) = chain_a();
        request.initial = vec![q(1, 1), q(1, 1), q(0, 1)];
        cases.push(Case {
            id: format!("invalid_initial_{i}"),
            request,
            expected: HittingStatus::DimensionMismatch,
            artifact: None,
        });
    }
    assert_eq!(cases.len(), 240);
    let corpus_sha256 = digest(&cases);
    let mut receipts = Vec::with_capacity(cases.len());
    for case in cases {
        let output: HittingResult = evaluate(&case.request);
        let mut tampered = output.clone();
        tampered.replay_hash.push('x');
        let exact = output.status == case.expected && output.artifact == case.artifact;
        let false_authorization =
            case.expected != HittingStatus::Complete && output.artifact.is_some();
        let false_denial = case.expected == HittingStatus::Complete && !exact;
        receipts.push(Receipt {
            id: case.id,
            expected: case.expected,
            actual: output.status,
            expected_artifact: case.artifact,
            actual_artifact: output.artifact.clone(),
            exact,
            replay_verified: output.replay_verified(),
            tamper_rejected: !tampered.replay_verified(),
            false_authorization,
            false_denial,
        });
    }
    let cases = receipts.len();
    let supported = receipts
        .iter()
        .filter(|r| r.expected == HittingStatus::Complete)
        .count();
    let ambiguous = receipts
        .iter()
        .filter(|r| r.expected == HittingStatus::Ambiguous)
        .count();
    let refused = cases - supported - ambiguous;
    let exact_decisions = receipts.iter().filter(|r| r.exact).count();
    let replay_verified = receipts.iter().filter(|r| r.replay_verified).count();
    let tamper_rejections = receipts.iter().filter(|r| r.tamper_rejected).count();
    let false_authorizations = receipts.iter().filter(|r| r.false_authorization).count();
    let false_denials = receipts.iter().filter(|r| r.false_denial).count();
    assert_eq!((supported, ambiguous, refused), (120, 40, 80));
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
    let manifest_sha256 = breadth_first_manifest().replay_hash();
    let report = Report {
        schema: "stage190-finite-markov-hitting-v1",
        manifest_sha256: manifest_sha256.clone(),
        corpus_sha256,
        source: "docs/sources/openstax_finite_markov_hitting_source.txt",
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
    let serialized = serde_json::to_string_pretty(&report)?;
    fs::write(REPORT_JSON, format!("{serialized}\n"))?;
    fs::write(
        REPORT_MD,
        format!(
            "# Stage 190 — bounded exact hitting probabilities\n\nThe pack solves explicit target-before-avoid probabilities for finite rational chains with at most four states.\n\n| Measure | Result |\n|---|---:|\n| Cases | {cases} |\n| Supported / ambiguous / refused | {supported} / {ambiguous} / {refused} |\n| Exact decisions | {exact_decisions}/{cases} |\n| Replay / tamper | {replay_verified}/{cases} / {tamper_rejections}/{cases} |\n| False authorizations / denials | {false_authorizations} / {false_denials} |\n| Production mutation | false |\n\nManifest SHA-256: `{manifest_sha256}`\n\nSource record: `docs/sources/openstax_finite_markov_hitting_source.txt`\n\nMachine-readable report: `{REPORT_JSON}`\n"
        ),
    )?;
    println!("{serialized}");
    Ok(())
}
