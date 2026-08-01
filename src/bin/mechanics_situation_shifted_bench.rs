//! Phase 38 shifted-language benchmark for MechanicsSituationV1.

use serde::Serialize;
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::env;
use std::fs;
use the_machine::classical_mechanics_pack::MechanicsStatus;
use the_machine::mechanics_situation::{
    execute_mechanics_situation, formalize_mechanics_situation, replay_execution, replay_situation,
    SituationStatus,
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
    false_domain_entries: usize,
    false_unique_law_selections: usize,
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

fn supported() -> Vec<CaseSpec> {
    let mut cases = Vec::new();
    for index in 0..120 {
        let id = format!("shifted-supported-{index:03}");
        let family_index = index % 5;
        let n = index as f64;
        let case = match family_index {
            0 => {
                let m = 2.0 + (index % 7) as f64;
                let a = 2.0 + (index % 6) as f64;
                let f = m * a;
                CaseSpec { id, family: "indirect_newton".into(), text: format!("The experiment is performed in an inertial frame. An unrelated label {n} is ignored; a body of mass {m} kg experiences a net force of {f} N. Determine the acceleration."), expected_status: "unique".into(), expected_law: Some("newtons_second_law".into()), expected_value: Some(a) }
            }
            1 => {
                let m = 2.0 + (index % 7) as f64;
                let v = 1.0 + (index % 8) as f64;
                CaseSpec { id, family: "reordered_momentum".into(), text: format!("At one instant the velocity is {v} m/s; the particle's mass, stated afterward, is {m} kg. What is its momentum?"), expected_status: "unique".into(), expected_law: Some("linear_momentum".into()), expected_value: Some(m * v) }
            }
            2 => {
                let m = 2.0 + (index % 6) as f64;
                let v = 2.0 + (index % 7) as f64;
                CaseSpec { id, family: "indirect_kinetic".into(), text: format!("Ignoring an irrelevant identifier {n}, a non-relativistic particle moves at {v} m/s and has mass {m} kg. Compute the kinetic energy."), expected_status: "unique".into(), expected_law: Some("kinetic_energy".into()), expected_value: Some(0.5 * m * v * v) }
            }
            3 => {
                let k = 8.0 + (index % 9) as f64;
                let x = 0.2 + (index % 5) as f64 / 10.0;
                CaseSpec { id, family: "reordered_hooke".into(), text: format!("The displacement is {x} m and the stiffness is {k} N/m. The device is an ideal linear spring. What restoring force results?"), expected_status: "unique".into(), expected_law: Some("hooke_force".into()), expected_value: Some(-k * x) }
            }
            _ => {
                let k = 8.0 + (index % 9) as f64;
                let x = 0.2 + (index % 5) as f64 / 10.0;
                CaseSpec { id, family: "indirect_elastic".into(), text: format!("An ideal linear spring is displaced by {x} m; its spring constant is {k} N/m. Calculate the elastic potential energy, not the unrelated label {n}."), expected_status: "unique".into(), expected_law: Some("elastic_potential_energy".into()), expected_value: Some(0.5 * k * x * x) }
            }
        };
        cases.push(case);
    }
    cases
}

fn boundaries() -> Vec<CaseSpec> {
    let mut cases = Vec::new();
    for index in 0..20 {
        cases.push(CaseSpec {
            id: format!("implicit-target-{index:03}"),
            family: "implicit_target".into(),
            text: format!(
                "A particle has mass {} kg and velocity 4 m/s. What does this tell us?",
                2 + index
            ),
            expected_status: "ambiguous".into(),
            expected_law: None,
            expected_value: None,
        });
    }
    for index in 0..20 {
        cases.push(CaseSpec { id: format!("force-not-net-{index:03}"), family: "force_not_net".into(), text: format!("In an inertial frame, a body of mass {} kg experiences a force of 12 N. Determine the acceleration.", 2 + index), expected_status: "ambiguous".into(), expected_law: None, expected_value: None });
    }
    for index in 0..20 {
        cases.push(CaseSpec { id: format!("scalar-direction-{index:03}"), family: "scalar_direction".into(), text: format!("In an inertial frame, a body of mass {} kg has a net force magnitude of 12 N. Determine acceleration; no direction is given.", 2 + index), expected_status: "ambiguous".into(), expected_law: None, expected_value: None });
    }
    for index in 0..20 {
        cases.push(CaseSpec { id: format!("multi-body-{index:03}"), family: "multi_body".into(), text: format!("Two bodies of mass {} kg and 3 kg interact in an inertial frame under a net force of 12 N. Determine the acceleration.", 2 + index), expected_status: "ambiguous".into(), expected_law: None, expected_value: None });
    }
    for index in 0..20 {
        cases.push(CaseSpec {
            id: format!("generic-energy-{index:03}"),
            family: "generic_energy".into(),
            text: format!(
                "An object of mass {} kg moves at 4 m/s. What is its energy?",
                2 + index
            ),
            expected_status: "ambiguous".into(),
            expected_law: None,
            expected_value: None,
        });
    }
    for index in 0..20 {
        cases.push(CaseSpec { id: format!("unsupported-domain-{index:03}"), family: "unsupported_domain".into(), text: format!("A relativistic rotating system has mass {} kg and speed 4 m/s. Determine its orbital state.", 2 + index), expected_status: "unsupported".into(), expected_law: None, expected_value: None });
    }
    for index in 0..20 {
        cases.push(CaseSpec { id: format!("missing-assumption-{index:03}"), family: "missing_assumption".into(), text: format!("A body of mass {} kg experiences net force 12 N. Determine acceleration, but no frame is specified.", 2 + index), expected_status: "ambiguous".into(), expected_law: None, expected_value: None });
    }
    cases
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut cases = supported();
    cases.extend(boundaries());
    let corpus_sha256 = hash(&cases);
    let mut exact_status = 0;
    let mut exact_law = 0;
    let mut exact_values = 0;
    let mut situation_replay_verified = 0;
    let mut execution_replay_verified = 0;
    let mut false_domain_entries = 0;
    let mut false_unique_law_selections = 0;
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
        let value_correct =
            case.expected_status != "unique" || close(execution.value, case.expected_value);
        exact_status += usize::from(status_correct);
        exact_law += usize::from(law_correct);
        exact_values += usize::from(case.expected_status == "unique" && value_correct);
        let situation_replay = replay_situation(&situation);
        let execution_replay = replay_execution(&execution);
        situation_replay_verified += usize::from(situation_replay);
        execution_replay_verified += usize::from(execution_replay);
        let invoked = execution.mechanics_status.is_some();
        pack_invocations += usize::from(invoked);
        pack_complete_results +=
            usize::from(execution.mechanics_status == Some(MechanicsStatus::Complete));
        false_domain_entries += usize::from(
            case.expected_status == "unsupported"
                && situation.status != SituationStatus::Unsupported,
        );
        false_unique_law_selections += usize::from(
            case.expected_status != "unique" && situation.status == SituationStatus::Unique,
        );
        false_authorizations += usize::from(
            case.expected_status != "unique"
                && execution.mechanics_status == Some(MechanicsStatus::Complete),
        );
        provenance_complete += usize::from(!situation.provenance.is_empty());
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
            situation_replay,
            execution_replay,
            provenance_spans: situation.provenance.len(),
            reasons: execution.reasons,
        });
    }
    let report = Report { schema_version: "phase38.mechanics.situation.shifted.v1".into(), corpus_sha256, total_cases: cases.len(), supported_cases: cases.iter().filter(|case| case.expected_status == "unique").count(), ambiguous_cases: cases.iter().filter(|case| case.expected_status == "ambiguous").count(), unsupported_cases: cases.iter().filter(|case| case.expected_status == "unsupported").count(), exact_status, exact_law, exact_values, situation_replay_verified, execution_replay_verified, false_domain_entries, false_unique_law_selections, false_authorizations, pack_invocations, pack_complete_results, provenance_complete, class_counts: class_counts.into_iter().collect(), registry_mutated: false, hle_routing_mutated: false, cases: results, method: "independent shifted-language corpus; structural situation formalization; no HLE or production routing".into() };
    let output = serde_json::to_string_pretty(&report)?;
    let path = env::args()
        .nth(1)
        .unwrap_or_else(|| "docs/phase38_mechanics_situation_shifted_bench.json".into());
    fs::write(&path, output)?;
    println!("phase38 report written to {path}");
    Ok(())
}
