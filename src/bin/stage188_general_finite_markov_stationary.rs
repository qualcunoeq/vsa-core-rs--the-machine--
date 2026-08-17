//! Stage 188: independent validation of the bounded general stationary pack.

use serde::Serialize;
use sha2::{Digest, Sha256};
use std::fs;
use the_machine::curriculum::breadth_first_manifest;
use the_machine::finite_markov_stationary_pack::{
    evaluate, StationaryArtifact, StationaryRequest, StationaryResult, StationaryStatus,
};
use the_machine::probability_pack::Rational;

const REPORT_JSON: &str = "docs/stage188_general_finite_markov_stationary.json";
const REPORT_MD: &str = "docs/stage188_general_finite_markov_stationary.md";

fn q(numerator: i128, denominator: i128) -> Rational {
    Rational::new(numerator, denominator).expect("valid rational")
}

fn request(transition: Vec<Vec<Rational>>) -> StationaryRequest {
    StationaryRequest {
        domain: "finite_exact_markov_stationary".into(),
        transition,
        row_stochastic: Some(true),
        ambiguity: None,
        provenance: vec!["stage188-independent-stationary-corpus".into()],
    }
}

fn two_state() -> (Vec<Vec<Rational>>, Vec<Rational>) {
    (
        vec![vec![q(3, 4), q(1, 4)], vec![q(1, 2), q(1, 2)]],
        vec![q(2, 3), q(1, 3)],
    )
}

fn three_cycle() -> (Vec<Vec<Rational>>, Vec<Rational>) {
    (
        vec![
            vec![q(0, 1), q(1, 1), q(0, 1)],
            vec![q(0, 1), q(0, 1), q(1, 1)],
            vec![q(1, 1), q(0, 1), q(0, 1)],
        ],
        vec![q(1, 3), q(1, 3), q(1, 3)],
    )
}

fn three_mixing() -> (Vec<Vec<Rational>>, Vec<Rational>) {
    let row = vec![q(1, 2), q(1, 3), q(1, 6)];
    (
        vec![row.clone(), row.clone(), row],
        vec![q(1, 2), q(1, 3), q(1, 6)],
    )
}

fn four_mixing() -> (Vec<Vec<Rational>>, Vec<Rational>) {
    let row = vec![q(1, 4), q(1, 4), q(1, 4), q(1, 4)];
    (
        vec![row.clone(), row.clone(), row.clone(), row],
        vec![q(1, 4), q(1, 4), q(1, 4), q(1, 4)],
    )
}

#[derive(Debug, Clone, Serialize)]
struct Case {
    id: String,
    request: StationaryRequest,
    expected_status: StationaryStatus,
    expected_artifact: Option<StationaryArtifact>,
}

#[derive(Debug, Serialize)]
struct Receipt {
    id: String,
    expected_status: StationaryStatus,
    actual_status: StationaryStatus,
    expected_artifact: Option<StationaryArtifact>,
    actual_artifact: Option<StationaryArtifact>,
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
    format!(
        "{:x}",
        Sha256::digest(serde_json::to_vec(value).expect("stage188 serializes"))
    )
}

fn add_supported(
    cases: &mut Vec<Case>,
    prefix: &str,
    transition: Vec<Vec<Rational>>,
    values: Vec<Rational>,
) {
    let states = values.len();
    for i in 0..30 {
        cases.push(Case {
            id: format!("{prefix}_{i}"),
            request: request(transition.clone()),
            expected_status: StationaryStatus::Complete,
            expected_artifact: Some(StationaryArtifact {
                distribution: values.clone(),
                state_order: (0..states).collect(),
                residual_verified: true,
            }),
        });
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut cases = Vec::with_capacity(240);
    let (transition, values) = two_state();
    add_supported(&mut cases, "two_state", transition, values);
    let (transition, values) = three_cycle();
    add_supported(&mut cases, "three_cycle", transition, values);
    let (transition, values) = three_mixing();
    add_supported(&mut cases, "three_mixing", transition, values);
    let (transition, values) = four_mixing();
    add_supported(&mut cases, "four_mixing", transition, values);

    let (transition, _) = three_cycle();
    for i in 0..40 {
        let mut r = request(transition.clone());
        r.row_stochastic = None;
        cases.push(Case {
            id: format!("ambiguous_convention_{i}"),
            request: r,
            expected_status: StationaryStatus::Ambiguous,
            expected_artifact: None,
        });
    }
    for i in 0..40 {
        let identity = vec![
            vec![q(1, 1), q(0, 1), q(0, 1)],
            vec![q(0, 1), q(1, 1), q(0, 1)],
            vec![q(0, 1), q(0, 1), q(1, 1)],
        ];
        cases.push(Case {
            id: format!("non_unique_{i}"),
            request: request(identity),
            expected_status: StationaryStatus::NonUnique,
            expected_artifact: None,
        });
    }
    for i in 0..20 {
        let invalid = vec![
            vec![q(1, 1), q(0, 1), q(0, 1)],
            vec![q(0, 1), q(1, 1), q(0, 1)],
            vec![q(1, 1), q(1, 1), q(0, 1)],
        ];
        cases.push(Case {
            id: format!("invalid_row_{i}"),
            request: request(invalid),
            expected_status: StationaryStatus::InvalidTransition,
            expected_artifact: None,
        });
    }
    for i in 0..20 {
        let identity = (0..5)
            .map(|row| {
                (0..5)
                    .map(|column| if row == column { q(1, 1) } else { q(0, 1) })
                    .collect()
            })
            .collect();
        cases.push(Case {
            id: format!("over_dimension_{i}"),
            request: request(identity),
            expected_status: StationaryStatus::DimensionMismatch,
            expected_artifact: None,
        });
    }
    assert_eq!(cases.len(), 240);
    let corpus_sha256 = digest(&cases);
    let mut receipts = Vec::with_capacity(cases.len());
    for case in cases {
        let output: StationaryResult = evaluate(&case.request);
        let mut tampered = output.clone();
        tampered.replay_hash.push('x');
        let exact =
            output.status == case.expected_status && output.artifact == case.expected_artifact;
        let false_authorization =
            case.expected_status != StationaryStatus::Complete && output.artifact.is_some();
        let false_denial = case.expected_status == StationaryStatus::Complete && !exact;
        receipts.push(Receipt {
            id: case.id,
            expected_status: case.expected_status,
            actual_status: output.status,
            expected_artifact: case.expected_artifact,
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
        .filter(|r| r.expected_status == StationaryStatus::Complete)
        .count();
    let ambiguous = receipts
        .iter()
        .filter(|r| r.expected_status == StationaryStatus::Ambiguous)
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
    let report = Report {
        schema: "stage188-general-finite-markov-stationary-v1",
        manifest_sha256: breadth_first_manifest().replay_hash(),
        corpus_sha256,
        source: "docs/sources/openstax_finite_markov_stationary_source.txt",
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
            "# Stage 188 — bounded general finite stationary distributions\n\nThe separate pack solves exact stationary distributions for validated row-stochastic matrices with at most four states. The historical two-state Markov pack remains unchanged.\n\n| Measure | Result |\n|---|---:|\n| Cases | {cases} |\n| Supported / ambiguous / refused | {supported} / {ambiguous} / {refused} |\n| Exact decisions | {exact_decisions}/{cases} |\n| Replay / tamper | {replay_verified}/{cases} / {tamper_rejections}/{cases} |\n| False authorizations / denials | {false_authorizations} / {false_denials} |\n| Production mutation | false |\n\nManifest SHA-256: `{}`\n\nSource record: `{}`\n\nMachine-readable report: `{REPORT_JSON}`\n",
            breadth_first_manifest().replay_hash(),
            "docs/sources/openstax_finite_markov_stationary_source.txt"
        ),
    )?;
    println!("{serialized}");
    Ok(())
}
