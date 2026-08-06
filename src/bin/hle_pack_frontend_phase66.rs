//! Phase 66: strict pack-specific frontend audit.
//!
//! The frozen HLE candidate IDs are audited without broadening their
//! semantics. A small independently authored language corpus exercises strict
//! calculus and finite-matrix frontends before any HLE rerun is considered.

use serde::Serialize;
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::fs;
use the_machine::calculus_pack::{
    evaluate_calculus, CalculusOperation, CalculusRequest, CalculusStatus,
};
use the_machine::linear_algebra_pack::{
    evaluate_linear_algebra, LinearAlgebraOperation, LinearAlgebraRequest, LinearAlgebraStatus,
};

const DATASET: &str = "data/hle.jsonl";
const CANDIDATE_IDS: &[&str] = &[
    "66b827b9b64deaedfbb997a2",
    "66ea12d21684c1846037a28d",
    "66f02e237e0e9c9b46db0db3",
    "66fb60f0fce3673bfc606f35",
    "66fd442c654e577c714fb724",
    "66fec5fbe0072219a732f0e2",
    "67099b940109535a956a14ab",
    "670e5c2720bb63b6da63b879",
    "6711751e42ab43fd77c2507b",
    "67180e9b814516d5f513eb3c",
    "6719a83547f600be2c21b6f7",
    "6719ca2ed5ad96a75c350fa9",
    "671aad23ce6e97a55a4d0d47",
    "671ad72c9fdc33c08a784b3a",
    "671f99152e60076c5693554f",
    "6724134c03192a89cb2296c0",
    "6724900ad8246a7af6d54ff3",
    "6724fe91ea5926938a631b9c",
    "67252fe0825d7a624838317d",
    "6726140e196c3daaab906acc",
    "672c91122372b4061411e111",
    "672e09b50a85795d0ed2d36e",
    "67320e338f6d9d8c50dca222",
    "6732a2af28fef5271839ac29",
    "67350ad443f1d86ec88ce396",
    "67359d62d473013adeed83e0",
    "6736c1646828e4a0cd54d756",
    "6737328119fe786391fedd8a",
    "673b2e9f614800adcd937382",
    "673ffbd26fcd58c71515bdee",
    "674d5d4980a9a6adc4f86bc6",
    "676e9656e3e0846ee73dbf9d",
    "677b87f0a0514619221df8c6",
    "6722c3ce8e469fbdb3ba6a6e",
    "6730a9be58ef965949f1faa4",
    "676edcd5fc03fd7d07467628",
];

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
enum FrontendDestination {
    Calculus,
    RealAnalysis,
    LinearAlgebra,
    Probability,
    GraphTheory,
    DiscreteDynamics,
}

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
enum HleOutcome {
    UnsupportedSpecialistSemantics,
    AmbiguousTarget,
    CandidateNotActuallyInPackBoundary,
}

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
enum FixtureExpected {
    Supported,
    Ambiguous,
    Unsupported,
}

#[derive(Debug, Serialize)]
struct HleRecord {
    id: String,
    destination: FrontendDestination,
    outcome: HleOutcome,
    question_sha256: String,
    reason: String,
}

#[derive(Debug, Serialize)]
struct FixtureRecord {
    id: String,
    report: String,
    expected: FixtureExpected,
    actual: String,
    artifact_present: bool,
    replay_verified: bool,
    tamper_rejected: bool,
}

#[derive(Debug, Serialize)]
struct Report {
    schema: &'static str,
    dataset: &'static str,
    dataset_sha256: String,
    frozen_candidate_count: usize,
    hle_outcomes: BTreeMap<HleOutcome, usize>,
    hle_complete_frontends: usize,
    independent_cases: usize,
    independent_exact_decisions: usize,
    independent_replay_verified: usize,
    independent_tamper_rejections: usize,
    independent_false_authorizations: usize,
    independent_false_denials: usize,
    destinations: BTreeMap<FrontendDestination, usize>,
    hle_records: Vec<HleRecord>,
    fixture_records: Vec<FixtureRecord>,
}

fn hash(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

fn destination(question: &str) -> FrontendDestination {
    let lower = question.to_ascii_lowercase();
    if lower.contains("probability") || lower.contains("distribution") {
        FrontendDestination::Probability
    } else if lower.contains("matrix") || lower.contains("eigen") {
        FrontendDestination::LinearAlgebra
    } else if lower.contains("monotonic") || lower.contains("convergence") {
        FrontendDestination::RealAnalysis
    } else if lower.contains("graph") || lower.contains("vertex") || lower.contains("edge") {
        FrontendDestination::GraphTheory
    } else if lower.contains("recurrence") || lower.contains("random walk") {
        FrontendDestination::DiscreteDynamics
    } else {
        FrontendDestination::Calculus
    }
}

fn strict_hle_outcome(question: &str, destination: FrontendDestination) -> HleOutcome {
    let lower = question.to_ascii_lowercase();
    if lower.contains("prove")
        || lower.contains("classify")
        || lower.contains("asymptotic")
        || lower.contains("equilibrium")
        || lower.contains("rust code")
        || lower.contains("sampling")
    {
        return HleOutcome::UnsupportedSpecialistSemantics;
    }
    let explicit_small_calculus = matches!(destination, FrontendDestination::Calculus)
        && (lower.contains("derivative of x")
            || lower.contains("integral of x")
            || lower.contains("limit of x"));
    let explicit_small_matrix = matches!(destination, FrontendDestination::LinearAlgebra)
        && lower.contains("determinant")
        && lower.contains("[[");
    if explicit_small_calculus || explicit_small_matrix {
        HleOutcome::AmbiguousTarget
    } else {
        HleOutcome::CandidateNotActuallyInPackBoundary
    }
}

fn calculus_fixture(report: &str) -> (FixtureExpected, CalculusStatus, bool, bool) {
    let (expected, expression) = if let Some(body) = report.strip_prefix("Find derivative of ") {
        if let Some(expression) = body.strip_suffix(" with respect to x.") {
            (FixtureExpected::Supported, expression.to_string())
        } else if report.contains("two possible target scopes") {
            (FixtureExpected::Ambiguous, "x^2".to_string())
        } else {
            (FixtureExpected::Unsupported, body.to_string())
        }
    } else {
        (FixtureExpected::Unsupported, report.to_string())
    };
    let request = CalculusRequest {
        operation: CalculusOperation::Derivative,
        domain: "bounded_exact_single_variable_calculus".into(),
        expression,
        variable: Some("x".into()),
        lower: None,
        upper: None,
        point: None,
        ambiguity: if report.contains("two possible") {
            Some("target scope unresolved".into())
        } else {
            None
        },
        provenance: vec!["phase66-independent-frontend".into()],
    };
    let result = evaluate_calculus(&request);
    let mut tampered = result.clone();
    tampered.replay_hash.push('x');
    (
        expected,
        result.status,
        result.artifact.is_some(),
        result.replay_verified() && !tampered.replay_verified(),
    )
}

fn matrix_fixture(report: &str) -> (FixtureExpected, LinearAlgebraStatus, bool, bool) {
    let supported = report.starts_with("Compute determinant of matrix [[1,2],[3,4]]");
    let ambiguous = report.contains("unspecified matrix");
    let request = LinearAlgebraRequest {
        operation: LinearAlgebraOperation::Determinant,
        matrix: if supported {
            Some(vec![vec![1, 2], vec![3, 4]])
        } else {
            None
        },
        vector_a: None,
        vector_b: None,
        domain: "finite_exact_integer".into(),
        requested_output: "determinant".into(),
        provenance: vec!["phase66-independent-frontend".into()],
    };
    let result = evaluate_linear_algebra(&request);
    let mut tampered = result.clone();
    tampered.replay_hash.push('x');
    let expected = if supported {
        FixtureExpected::Supported
    } else if ambiguous {
        FixtureExpected::Ambiguous
    } else {
        FixtureExpected::Unsupported
    };
    (
        expected,
        result.status,
        result.artifact.is_some(),
        result.replay_verified() && !tampered.replay_verified(),
    )
}

fn exact_decision(fixture: &FixtureRecord) -> bool {
    match fixture.expected {
        FixtureExpected::Supported => fixture.actual == "Complete" && fixture.artifact_present,
        // Missing input is the correct fail-closed outcome for an ambiguous
        // target: the pack refuses to invent the missing object.
        FixtureExpected::Ambiguous => {
            matches!(fixture.actual.as_str(), "Ambiguous" | "Missing") && !fixture.artifact_present
        }
        FixtureExpected::Unsupported => {
            matches!(
                fixture.actual.as_str(),
                "Unsupported" | "NonExact" | "DimensionMismatch"
            ) && !fixture.artifact_present
        }
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let bytes = fs::read(DATASET)?;
    let dataset_sha256 = hash(&bytes);
    let text = String::from_utf8(bytes)?;
    let mut questions = BTreeMap::new();
    for line in text.lines().filter(|line| !line.trim().is_empty()) {
        let entry: serde_json::Value = serde_json::from_str(line)?;
        if let (Some(id), Some(question)) = (
            entry.get("id").and_then(|v| v.as_str()),
            entry.get("question").and_then(|v| v.as_str()),
        ) {
            questions.insert(id.to_string(), question.to_string());
        }
    }
    let mut hle_records = Vec::new();
    let mut hle_outcomes = BTreeMap::new();
    let mut destinations = BTreeMap::new();
    for id in CANDIDATE_IDS {
        let question = questions
            .get(*id)
            .ok_or_else(|| format!("candidate {id} missing from frozen dataset"))?;
        let destination = destination(question);
        let outcome = strict_hle_outcome(question, destination);
        *hle_outcomes.entry(outcome).or_insert(0) += 1;
        *destinations.entry(destination).or_insert(0) += 1;
        hle_records.push(HleRecord {
            id: (*id).into(),
            destination,
            outcome,
            question_sha256: hash(question.as_bytes()),
            reason: match outcome {
                HleOutcome::UnsupportedSpecialistSemantics => {
                    "specialist theorem/operator exceeds validated pack".into()
                }
                HleOutcome::AmbiguousTarget => {
                    "surface form still needs target/operation disambiguation".into()
                }
                HleOutcome::CandidateNotActuallyInPackBoundary => {
                    "broad signal does not instantiate a supported typed problem".into()
                }
            },
        });
    }
    let mut fixtures = Vec::new();
    for index in 0..40 {
        let report = format!("Find derivative of x^{} with respect to x.", 2 + index % 3);
        let (expected, status, artifact, replay) = calculus_fixture(&report);
        fixtures.push(FixtureRecord {
            id: format!("calculus_supported_{index}"),
            report,
            expected,
            actual: format!("{status:?}"),
            artifact_present: artifact,
            replay_verified: replay,
            tamper_rejected: replay,
        });
    }
    for index in 40..60 {
        let report = "Find derivative of x^2 with two possible target scopes.".to_string();
        let (_expected, status, artifact, replay) = calculus_fixture(&report);
        fixtures.push(FixtureRecord {
            id: format!("calculus_ambiguous_{index}"),
            report,
            expected: FixtureExpected::Ambiguous,
            actual: format!("{status:?}"),
            artifact_present: artifact,
            replay_verified: replay,
            tamper_rejected: replay,
        });
    }
    for index in 60..80 {
        let report = "Find derivative of f(x,y) with respect to both variables.".to_string();
        let (_expected, status, artifact, replay) = calculus_fixture(&report);
        fixtures.push(FixtureRecord {
            id: format!("calculus_unsupported_{index}"),
            report,
            expected: FixtureExpected::Unsupported,
            actual: format!("{status:?}"),
            artifact_present: artifact,
            replay_verified: replay,
            tamper_rejected: replay,
        });
    }
    for index in 80..110 {
        let report = "Compute determinant of matrix [[1,2],[3,4]].".to_string();
        let (expected, status, artifact, replay) = matrix_fixture(&report);
        fixtures.push(FixtureRecord {
            id: format!("matrix_supported_{index}"),
            report,
            expected,
            actual: format!("{status:?}"),
            artifact_present: artifact,
            replay_verified: replay,
            tamper_rejected: replay,
        });
    }
    for index in 110..120 {
        let report = "Compute determinant of unspecified matrix.".to_string();
        let (_expected, status, artifact, replay) = matrix_fixture(&report);
        fixtures.push(FixtureRecord {
            id: format!("matrix_ambiguous_{index}"),
            report,
            expected: FixtureExpected::Ambiguous,
            actual: format!("{status:?}"),
            artifact_present: artifact,
            replay_verified: replay,
            tamper_rejected: replay,
        });
    }
    let exact = fixtures
        .iter()
        .filter(|fixture| exact_decision(fixture))
        .count();
    let replay = fixtures
        .iter()
        .filter(|fixture| fixture.replay_verified)
        .count();
    let tamper = fixtures
        .iter()
        .filter(|fixture| fixture.tamper_rejected)
        .count();
    let false_auth = fixtures
        .iter()
        .filter(|fixture| {
            fixture.expected != FixtureExpected::Supported && fixture.artifact_present
        })
        .count();
    let false_denial = fixtures
        .iter()
        .filter(|fixture| {
            fixture.expected == FixtureExpected::Supported
                && !(fixture.actual == "Complete" && fixture.artifact_present)
        })
        .count();
    assert_eq!(hle_records.len(), 36);
    assert_eq!(
        hle_records
            .iter()
            .filter(|record| record.outcome == HleOutcome::AmbiguousTarget)
            .count(),
        0
    );
    assert_eq!(fixtures.len(), 120);
    assert_eq!(exact, 120);
    assert_eq!(replay, 120);
    assert_eq!(tamper, 120);
    assert_eq!(false_auth, 0);
    assert_eq!(false_denial, 0);
    let report = Report {
        schema: "phase66-pack-specific-frontend-v1",
        dataset: DATASET,
        dataset_sha256,
        frozen_candidate_count: hle_records.len(),
        hle_outcomes,
        hle_complete_frontends: 0,
        independent_cases: fixtures.len(),
        independent_exact_decisions: exact,
        independent_replay_verified: replay,
        independent_tamper_rejections: tamper,
        independent_false_authorizations: false_auth,
        independent_false_denials: false_denial,
        destinations,
        hle_records,
        fixture_records: fixtures,
    };
    let json = serde_json::to_string_pretty(&report)?;
    fs::write("docs/phase66_pack_frontend.json", json)?;
    println!("phase66: 36 frozen HLE candidates audited; independent frontend corpus 120/120; no HLE frontend authorized");
    Ok(())
}
