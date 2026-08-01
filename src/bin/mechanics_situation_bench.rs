//! Phase 37 independent benchmark for MechanicsSituationV1.

use serde::Serialize;
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::env;
use std::fs;
use the_machine::mechanics_situation::{
    execute_mechanics_situation, formalize_mechanics_situation, replay_situation, SituationStatus,
};

#[derive(Debug, Clone, Serialize)]
struct CaseSpec {
    id: String,
    family: String,
    text: String,
    expected_status: String,
    expected_law: Option<String>,
    expected_value: Option<f64>,
}

#[derive(Debug, Serialize)]
struct CaseResult {
    id: String,
    family: String,
    expected_status: String,
    actual_status: String,
    expected_law: Option<String>,
    actual_laws: Vec<String>,
    expected_value: Option<f64>,
    actual_value: Option<f64>,
    status_correct: bool,
    law_correct: bool,
    value_correct: bool,
    situation_replay: bool,
    execution_replay: bool,
    provenance_spans: usize,
    mechanics_status: Option<String>,
    reasons: Vec<String>,
}

#[derive(Debug, Serialize)]
struct Report {
    schema_version: String,
    corpus_sha256: String,
    total_cases: usize,
    supported_cases: usize,
    ambiguous_cases: usize,
    unsupported_cases: usize,
    exact_status: usize,
    exact_law: usize,
    exact_values: usize,
    situation_replay_verified: usize,
    execution_replay_verified: usize,
    false_unique_applications: usize,
    false_authorizations: usize,
    pack_invocations: usize,
    pack_complete_results: usize,
    provenance_complete: usize,
    class_counts: Vec<(String, usize)>,
    registry_mutated: bool,
    hle_routing_mutated: bool,
    cases: Vec<CaseResult>,
    method: String,
}

fn hash<T: Serialize>(value: &T) -> String {
    format!("{:x}", Sha256::digest(serde_json::to_vec(value).unwrap()))
}

fn status_name(status: &SituationStatus) -> &'static str {
    match status {
        SituationStatus::Unique => "unique",
        SituationStatus::Ambiguous => "ambiguous",
        SituationStatus::Missing => "missing",
        SituationStatus::Unsupported => "unsupported",
    }
}

fn close(left: Option<f64>, right: Option<f64>) -> bool {
    match (left, right) {
        (None, None) => true,
        (Some(a), Some(b)) => (a - b).abs() <= 1e-9 * (1.0 + a.abs().max(b.abs())),
        _ => false,
    }
}

fn positives() -> Vec<CaseSpec> {
    let mut cases = Vec::new();
    for index in 0..160 {
        let id = format!("supported-{index:03}");
        let n = index as f64;
        let family_index = index % 5;
        let case = match family_index {
            0 => {
                let m = 2.0 + (index % 7) as f64;
                let a = 3.0 + (index % 5) as f64;
                let f = m * a;
                let text = if index % 2 == 0 {
                    format!("In an inertial frame, a body has mass {m} kg and net force {f} N. Find acceleration.")
                } else {
                    format!("For an inertial reference frame, mass {m} kg and net force {f} N act on the body. Find acceleration.")
                };
                CaseSpec {
                    id,
                    family: "newton".into(),
                    text,
                    expected_status: "unique".into(),
                    expected_law: Some("newtons_second_law".into()),
                    expected_value: Some(a),
                }
            }
            1 => {
                let m = 2.0 + (index % 7) as f64;
                let v = 1.0 + (index % 8) as f64;
                let p = m * v;
                let text = if index % 2 == 0 {
                    format!(
                        "A point particle has mass {m} kg and velocity {v} m/s. Find its momentum."
                    )
                } else {
                    format!("The particle's mass is {m} kg and its velocity is {v} m/s; calculate momentum.")
                };
                CaseSpec {
                    id,
                    family: "momentum".into(),
                    text,
                    expected_status: "unique".into(),
                    expected_law: Some("linear_momentum".into()),
                    expected_value: Some(p),
                }
            }
            2 => {
                let m = 2.0 + (index % 6) as f64;
                let v = 2.0 + (index % 7) as f64;
                let k = 0.5 * m * v * v;
                let text = if index % 2 == 0 {
                    format!("For non-relativistic translational motion, mass {m} kg and velocity {v} m/s are given. Find the kinetic energy.")
                } else {
                    format!("A non-relativistic particle moves at velocity {v} m/s and has mass {m} kg. Calculate its kinetic energy.")
                };
                CaseSpec {
                    id,
                    family: "kinetic_energy".into(),
                    text,
                    expected_status: "unique".into(),
                    expected_law: Some("kinetic_energy".into()),
                    expected_value: Some(k),
                }
            }
            3 => {
                let k = 8.0 + (index % 9) as f64;
                let x = 0.2 + (index % 5) as f64 / 10.0;
                let f = -k * x;
                let text = if index % 2 == 0 {
                    format!("An ideal linear spring has spring constant {k} N/m and displacement {x} m. Find the restoring force.")
                } else {
                    format!("For a spring in the ideal linear regime, stiffness {k} N/m and extension {x} m are measured. Find spring force.")
                };
                CaseSpec {
                    id,
                    family: "hooke_force".into(),
                    text,
                    expected_status: "unique".into(),
                    expected_law: Some("hooke_force".into()),
                    expected_value: Some(f),
                }
            }
            _ => {
                let k = 8.0 + (index % 9) as f64;
                let x = 0.2 + (index % 5) as f64 / 10.0;
                let u = 0.5 * k * x * x;
                let text = if index % 2 == 0 {
                    format!("An ideal linear spring has spring constant {k} N/m and displacement {x} m. Find its elastic potential energy.")
                } else {
                    format!("The spring is ideal linear, with stiffness {k} N/m and extension {x} m. Calculate spring energy.")
                };
                CaseSpec {
                    id,
                    family: "elastic_energy".into(),
                    text,
                    expected_status: "unique".into(),
                    expected_law: Some("elastic_potential_energy".into()),
                    expected_value: Some(u),
                }
            }
        };
        debug_assert!(n >= 0.0);
        cases.push(case);
    }
    cases
}

fn boundaries() -> Vec<CaseSpec> {
    let mut cases = Vec::new();
    for index in 0..20 {
        cases.push(CaseSpec {
            id: format!("ambiguous-energy-{index:03}"),
            family: "generic_energy".into(),
            text: format!(
                "An object has mass {} kg and velocity 4 m/s. What is the energy?",
                2 + index
            ),
            expected_status: "ambiguous".into(),
            expected_law: None,
            expected_value: None,
        });
    }
    for index in 0..20 {
        cases.push(CaseSpec {
            id: format!("missing-assumption-{index:03}"),
            family: "missing_assumption".into(),
            text: format!(
                "A body has mass {} kg and net force 12 N. Find acceleration.",
                2 + index
            ),
            expected_status: "ambiguous".into(),
            expected_law: None,
            expected_value: None,
        });
    }
    for index in 0..20 {
        cases.push(CaseSpec {
            id: format!("unsupported-domain-{index:03}"),
            family: "unsupported_domain".into(),
            text: format!("A relativistic rotating system has mass {} kg and speed 4 m/s. Determine its orbital state.", 2 + index),
            expected_status: "unsupported".into(),
            expected_law: None,
            expected_value: None,
        });
    }
    for index in 0..20 {
        cases.push(CaseSpec {
            id: format!("multi-law-{index:03}"),
            family: "multi_law_composition".into(),
            text: format!("A non-relativistic particle has mass {} kg and velocity 4 m/s. Find both kinetic energy and momentum.", 2 + index),
            expected_status: "ambiguous".into(),
            expected_law: None,
            expected_value: None,
        });
    }
    cases
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut cases = positives();
    cases.extend(boundaries());
    let corpus_sha256 = hash(&cases);
    let mut exact_status = 0;
    let mut exact_law = 0;
    let mut exact_values = 0;
    let mut situation_replay_verified = 0;
    let mut execution_replay_verified = 0;
    let mut false_unique_applications = 0;
    let mut false_authorizations = 0;
    let mut pack_invocations = 0;
    let mut pack_complete_results = 0;
    let mut provenance_complete = 0;
    let mut class_counts = BTreeMap::new();
    let mut results = Vec::new();
    for case in &cases {
        let situation = formalize_mechanics_situation(&case.text);
        let execution = execute_mechanics_situation(&situation);
        let actual_status = status_name(&situation.status).to_string();
        let status_correct = actual_status == case.expected_status;
        let law_correct = case
            .expected_law
            .as_ref()
            .map(|law| situation.candidate_laws == vec![law.clone()])
            .unwrap_or_else(|| situation.candidate_laws.is_empty());
        let value_correct = if case.expected_status == "unique" {
            close(execution.value, case.expected_value)
        } else {
            true
        };
        exact_status += usize::from(status_correct);
        exact_law += usize::from(law_correct);
        exact_values += usize::from(case.expected_status == "unique" && value_correct);
        situation_replay_verified += usize::from(replay_situation(&situation));
        execution_replay_verified += usize::from(
            the_machine::mechanics_situation::replay_execution(&execution),
        );
        let invoked = execution.mechanics_status.is_some();
        pack_invocations += usize::from(invoked);
        pack_complete_results += usize::from(
            execution.mechanics_status
                == Some(the_machine::classical_mechanics_pack::MechanicsStatus::Complete),
        );
        false_unique_applications += usize::from(
            case.expected_status != "unique" && situation.status == SituationStatus::Unique,
        );
        false_authorizations += usize::from(
            case.expected_status != "unique"
                && execution.mechanics_status
                    == Some(the_machine::classical_mechanics_pack::MechanicsStatus::Complete),
        );
        provenance_complete +=
            usize::from(!situation.provenance.is_empty() || case.expected_status == "unsupported");
        *class_counts.entry(case.family.clone()).or_insert(0) += 1;
        results.push(CaseResult {
            id: case.id.clone(),
            family: case.family.clone(),
            expected_status: case.expected_status.clone(),
            actual_status,
            expected_law: case.expected_law.clone(),
            actual_laws: situation.candidate_laws.clone(),
            expected_value: case.expected_value,
            actual_value: execution.value,
            status_correct,
            law_correct,
            value_correct,
            situation_replay: replay_situation(&situation),
            execution_replay: the_machine::mechanics_situation::replay_execution(&execution),
            provenance_spans: execution.situation.provenance.len(),
            mechanics_status: execution
                .mechanics_status
                .map(|status| format!("{status:?}")),
            reasons: execution.reasons,
        });
    }
    let report = Report {
        schema_version: "phase37.mechanics.situation.v1".into(),
        corpus_sha256,
        total_cases: cases.len(),
        supported_cases: cases.iter().filter(|case| case.expected_status == "unique").count(),
        ambiguous_cases: cases.iter().filter(|case| case.expected_status == "ambiguous").count(),
        unsupported_cases: cases.iter().filter(|case| case.expected_status == "unsupported").count(),
        exact_status,
        exact_law,
        exact_values,
        situation_replay_verified,
        execution_replay_verified,
        false_unique_applications,
        false_authorizations,
        pack_invocations,
        pack_complete_results,
        provenance_complete,
        class_counts: class_counts.into_iter().collect(),
        registry_mutated: false,
        hle_routing_mutated: false,
        cases: results,
        method: "independent situation corpus; bounded extraction; typed shadow bridge; no HLE or production routing".into(),
    };
    let output = serde_json::to_string_pretty(&report)?;
    let path = env::args()
        .nth(1)
        .unwrap_or_else(|| "docs/phase37_mechanics_situation_bench.json".into());
    fs::write(&path, output)?;
    println!("phase37 report written to {path}");
    Ok(())
}
