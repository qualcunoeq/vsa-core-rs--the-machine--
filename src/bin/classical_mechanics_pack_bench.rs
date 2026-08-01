//! Phase 34 shadow benchmark for an externally grounded mechanics pack.

use serde::Serialize;
use sha2::{Digest, Sha256};
use std::env;
use std::fs;
use the_machine::classical_mechanics_pack::{
    classical_mechanics_pack, evaluate_mechanics, lookup_mechanics, replay_mechanics,
    MechanicsEvaluationRequest, MechanicsStatus, NumericBinding,
};

#[derive(Debug, Clone, Serialize)]
struct CaseSpec {
    id: String,
    expected: String,
    request: MechanicsEvaluationRequest,
}

#[derive(Debug, Serialize)]
struct Report {
    schema_version: String,
    pack_sha256: String,
    corpus_sha256: String,
    source_urls: Vec<String>,
    total_cases: usize,
    supported_cases: usize,
    rejected_cases: usize,
    complete_results: usize,
    false_authorizations: usize,
    false_denials: usize,
    replay_verified: usize,
    lookup_unique: usize,
    lookup_ambiguous: usize,
    lookup_unsupported_or_missing: usize,
    registry_mutated: bool,
    cases: Vec<CaseResult>,
    method: String,
}

#[derive(Debug, Serialize)]
struct CaseResult {
    id: String,
    expected: String,
    actual: String,
    value: Option<f64>,
    replay_verified: bool,
    source_id: Option<String>,
    reasons: Vec<String>,
}

fn hash<T: Serialize>(value: &T) -> String {
    format!("{:x}", Sha256::digest(serde_json::to_vec(value).unwrap()))
}

fn status_name(status: MechanicsStatus) -> String {
    match status {
        MechanicsStatus::Complete => "complete",
        MechanicsStatus::Missing => "missing",
        MechanicsStatus::Ambiguous => "ambiguous",
        MechanicsStatus::UnitMismatch => "unit_mismatch",
        MechanicsStatus::Unsupported => "unsupported",
    }
    .into()
}

fn binding(symbol: &str, value: f64, unit: &str) -> NumericBinding {
    NumericBinding {
        symbol: symbol.into(),
        value,
        unit: unit.into(),
        provenance: format!("independent-exercise:{symbol}"),
    }
}

fn supported_cases() -> Vec<CaseSpec> {
    vec![
        CaseSpec {
            id: "newton-a".into(),
            expected: "complete".into(),
            request: MechanicsEvaluationRequest {
                law_id: "Newton's second law".into(),
                bindings: vec![binding("F_net", 12.0, "N"), binding("m", 3.0, "kg")],
                requested_output: "a".into(),
            },
        },
        CaseSpec {
            id: "newton-b".into(),
            expected: "complete".into(),
            request: MechanicsEvaluationRequest {
                law_id: "net force law".into(),
                bindings: vec![binding("m", 2.0, "kg"), binding("a", 4.0, "m/s^2")],
                requested_output: "F_net".into(),
            },
        },
        CaseSpec {
            id: "momentum-a".into(),
            expected: "complete".into(),
            request: MechanicsEvaluationRequest {
                law_id: "linear momentum".into(),
                bindings: vec![binding("m", 2.0, "kg"), binding("v", -3.0, "m/s")],
                requested_output: "p".into(),
            },
        },
        CaseSpec {
            id: "momentum-b".into(),
            expected: "complete".into(),
            request: MechanicsEvaluationRequest {
                law_id: "momentum definition".into(),
                bindings: vec![binding("p", 20.0, "kg*m/s"), binding("m", 4.0, "kg")],
                requested_output: "v".into(),
            },
        },
        CaseSpec {
            id: "kinetic-a".into(),
            expected: "complete".into(),
            request: MechanicsEvaluationRequest {
                law_id: "kinetic energy".into(),
                bindings: vec![binding("m", 4.0, "kg"), binding("v", 5.0, "m/s")],
                requested_output: "K".into(),
            },
        },
        CaseSpec {
            id: "kinetic-b".into(),
            expected: "complete".into(),
            request: MechanicsEvaluationRequest {
                law_id: "energy of motion".into(),
                bindings: vec![binding("K", 50.0, "J"), binding("m", 4.0, "kg")],
                requested_output: "v".into(),
            },
        },
        CaseSpec {
            id: "hooke-a".into(),
            expected: "complete".into(),
            request: MechanicsEvaluationRequest {
                law_id: "Hooke's law".into(),
                bindings: vec![binding("k", 10.0, "N/m"), binding("x", -0.2, "m")],
                requested_output: "F_spring".into(),
            },
        },
        CaseSpec {
            id: "hooke-b".into(),
            expected: "complete".into(),
            request: MechanicsEvaluationRequest {
                law_id: "spring restoring force".into(),
                bindings: vec![binding("F_spring", 2.0, "N"), binding("x", -0.2, "m")],
                requested_output: "k".into(),
            },
        },
        CaseSpec {
            id: "elastic-a".into(),
            expected: "complete".into(),
            request: MechanicsEvaluationRequest {
                law_id: "elastic potential energy".into(),
                bindings: vec![binding("k", 8.0, "N/m"), binding("x", 0.5, "m")],
                requested_output: "U".into(),
            },
        },
        CaseSpec {
            id: "elastic-b".into(),
            expected: "complete".into(),
            request: MechanicsEvaluationRequest {
                law_id: "spring energy".into(),
                bindings: vec![binding("U", 1.0, "J"), binding("k", 8.0, "N/m")],
                requested_output: "x".into(),
            },
        },
    ]
}

fn boundary_cases() -> Vec<CaseSpec> {
    vec![
        CaseSpec {
            id: "missing-mass".into(),
            expected: "missing".into(),
            request: MechanicsEvaluationRequest {
                law_id: "Newton's second law".into(),
                bindings: vec![binding("F_net", 12.0, "N")],
                requested_output: "a".into(),
            },
        },
        CaseSpec {
            id: "wrong-unit".into(),
            expected: "unit_mismatch".into(),
            request: MechanicsEvaluationRequest {
                law_id: "kinetic energy".into(),
                bindings: vec![binding("m", 4.0, "N"), binding("v", 5.0, "m/s")],
                requested_output: "K".into(),
            },
        },
        CaseSpec {
            id: "relativistic".into(),
            expected: "unsupported".into(),
            request: MechanicsEvaluationRequest {
                law_id: "relativistic kinetic energy".into(),
                bindings: vec![binding("m", 1.0, "kg"), binding("v", 299_000_000.0, "m/s")],
                requested_output: "K".into(),
            },
        },
        CaseSpec {
            id: "ambiguous-energy".into(),
            expected: "ambiguous".into(),
            request: MechanicsEvaluationRequest {
                law_id: "energy".into(),
                bindings: vec![binding("m", 1.0, "kg"), binding("v", 2.0, "m/s")],
                requested_output: "K".into(),
            },
        },
        CaseSpec {
            id: "missing-speed".into(),
            expected: "missing".into(),
            request: MechanicsEvaluationRequest {
                law_id: "momentum definition".into(),
                bindings: vec![binding("m", 2.0, "kg")],
                requested_output: "p".into(),
            },
        },
        CaseSpec {
            id: "wrong-regime".into(),
            expected: "unsupported".into(),
            request: MechanicsEvaluationRequest {
                law_id: "Hooke's law outside linear regime".into(),
                bindings: vec![binding("k", 10.0, "N/m"), binding("x", 2.0, "m")],
                requested_output: "F_spring".into(),
            },
        },
    ]
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let pack = classical_mechanics_pack();
    let mut cases = supported_cases();
    cases.extend(boundary_cases());
    let mut lookup_unique = 0;
    let mut lookup_ambiguous = 0;
    let mut lookup_unsupported_or_missing = 0;
    for alias in [
        "Newton's second law",
        "linear momentum",
        "kinetic energy",
        "Hooke's law",
        "elastic potential energy",
        "energy",
        "relativistic kinetic energy",
        "",
    ] {
        let (status, _) = lookup_mechanics(alias, &pack);
        match status {
            MechanicsStatus::Complete => lookup_unique += 1,
            MechanicsStatus::Ambiguous => lookup_ambiguous += 1,
            MechanicsStatus::Unsupported | MechanicsStatus::Missing => {
                lookup_unsupported_or_missing += 1
            }
            _ => {}
        }
    }
    let mut false_authorizations = 0;
    let mut false_denials = 0;
    let mut complete_results = 0;
    let mut replay_verified = 0;
    let mut results = Vec::new();
    for case in &cases {
        let result = evaluate_mechanics(&case.request, &pack);
        let actual = status_name(result.status);
        let expected_complete = case.expected == "complete";
        let actual_complete = result.status == MechanicsStatus::Complete;
        complete_results += usize::from(actual_complete);
        false_authorizations += usize::from(!expected_complete && actual_complete);
        false_denials += usize::from(expected_complete && !actual_complete);
        replay_verified += usize::from(replay_mechanics(&result));
        results.push(CaseResult {
            id: case.id.clone(),
            expected: case.expected.clone(),
            actual,
            value: result.value,
            replay_verified: replay_mechanics(&result),
            source_id: result.source.map(|source| source.source_id),
            reasons: result.reasons,
        });
    }
    let source_urls = pack
        .iter()
        .map(|law| law.source.url.clone())
        .collect::<Vec<_>>();
    let report = Report {
        schema_version: "phase34.classical.mechanics.pack.v1".into(),
        pack_sha256: hash(&pack),
        corpus_sha256: hash(&cases),
        source_urls,
        total_cases: cases.len(),
        supported_cases: supported_cases().len(),
        rejected_cases: boundary_cases().len(),
        complete_results,
        false_authorizations,
        false_denials,
        replay_verified,
        lookup_unique,
        lookup_ambiguous,
        lookup_unsupported_or_missing,
        registry_mutated: false,
        cases: results,
        method:
            "shadow-only externally sourced classical-mechanics pack; no HLE or production routing"
                .into(),
    };
    let output = serde_json::to_string_pretty(&report)?;
    let path = env::args()
        .nth(1)
        .unwrap_or_else(|| "/tmp/classical_mechanics_pack_bench.json".into());
    fs::write(&path, &output)?;
    println!("{}", output);
    Ok(())
}
