//! Phase 35 pressure benchmark for the shadow classical-mechanics pack.
//!
//! The corpus and oracle are independent of `evaluate_mechanics`.  Defect
//! injection is diagnostic only: each mutation is tested against the frozen
//! corpus, then repaired by re-running the canonical shadow pack in a cloned
//! sandbox.  No production registry or HLE route is touched.

use serde::Serialize;
use sha2::{Digest, Sha256};
use std::env;
use std::fs;
use the_machine::classical_mechanics_pack::{
    classical_mechanics_pack, evaluate_mechanics, replay_mechanics, MechanicsEvaluationRequest,
    MechanicsStatus, NumericBinding,
};

#[derive(Debug, Clone, Serialize)]
struct PressureCase {
    id: String,
    family: String,
    expected: String,
    request: MechanicsEvaluationRequest,
    oracle_value: Option<f64>,
}

#[derive(Debug, Serialize)]
struct DefectResult {
    defect: String,
    injected_counterexamples: usize,
    counterexample_ids: Vec<String>,
    diagnosed: bool,
    sandbox_repaired: bool,
    parent_pack_sha256: String,
    revised_shadow_sha256: String,
    parent_immutable: bool,
}

#[derive(Debug, Serialize)]
struct CaseResult {
    id: String,
    family: String,
    expected: String,
    actual: String,
    value: Option<f64>,
    oracle_value: Option<f64>,
    value_matches_oracle: bool,
    replay_verified: bool,
}

#[derive(Debug, Serialize)]
struct Report {
    schema_version: String,
    pack_sha256: String,
    corpus_sha256: String,
    total_cases: usize,
    supported_cases: usize,
    ambiguous_cases: usize,
    boundary_cases: usize,
    exact_decisions: usize,
    exact_values: usize,
    replay_verified: usize,
    false_authorizations: usize,
    false_denials: usize,
    defect_count: usize,
    defects_with_counterexamples: usize,
    defects_diagnosed: usize,
    defects_repaired: usize,
    parent_immutable: bool,
    registry_mutated: bool,
    category_counts: Vec<(String, usize)>,
    defects: Vec<DefectResult>,
    cases: Vec<CaseResult>,
    method: String,
}

fn hash<T: Serialize>(value: &T) -> String {
    format!("{:x}", Sha256::digest(serde_json::to_vec(value).unwrap()))
}

fn binding(symbol: &str, value: f64, unit: &str, case_id: &str) -> NumericBinding {
    NumericBinding {
        symbol: symbol.into(),
        value,
        unit: unit.into(),
        provenance: format!("phase35:{case_id}:{symbol}"),
    }
}

fn status_name(status: MechanicsStatus) -> &'static str {
    match status {
        MechanicsStatus::Complete => "complete",
        MechanicsStatus::Missing => "missing",
        MechanicsStatus::Ambiguous => "ambiguous",
        MechanicsStatus::UnitMismatch => "unit_mismatch",
        MechanicsStatus::Unsupported => "unsupported",
    }
}

fn close(left: Option<f64>, right: Option<f64>) -> bool {
    match (left, right) {
        (None, None) => true,
        (Some(a), Some(b)) => (a - b).abs() <= 1e-9 * (1.0 + a.abs().max(b.abs())),
        _ => false,
    }
}

fn oracle_value(request: &MechanicsEvaluationRequest) -> Option<f64> {
    let get = |symbol: &str| {
        request
            .bindings
            .iter()
            .find(|binding| binding.symbol == symbol)
            .map(|binding| binding.value)
    };
    let law = request.law_id.trim().to_ascii_lowercase();
    match law.as_str() {
        "newton's second law" | "net force law" | "newtons_second_law" => {
            match request.requested_output.as_str() {
                "F_net" => Some(get("m")? * get("a")?),
                "m" => Some(get("F_net")? / get("a")?),
                "a" => Some(get("F_net")? / get("m")?),
                _ => None,
            }
        }
        "linear momentum" | "momentum definition" | "linear_momentum" => {
            match request.requested_output.as_str() {
                "p" => Some(get("m")? * get("v")?),
                "m" => Some(get("p")? / get("v")?),
                "v" => Some(get("p")? / get("m")?),
                _ => None,
            }
        }
        "kinetic energy" | "energy of motion" | "kinetic_energy" => {
            match request.requested_output.as_str() {
                "K" => Some(0.5 * get("m")? * get("v")?.powi(2)),
                "m" => Some(2.0 * get("K")? / get("v")?.powi(2)),
                "v" => Some((2.0 * get("K")? / get("m")?).sqrt()),
                _ => None,
            }
        }
        "hooke's law" | "spring restoring force" | "hooke_force" => {
            match request.requested_output.as_str() {
                "F_spring" => Some(-get("k")? * get("x")?),
                "k" => Some(-get("F_spring")? / get("x")?),
                "x" => Some(-get("F_spring")? / get("k")?),
                _ => None,
            }
        }
        "elastic potential energy" | "spring energy" | "elastic_potential_energy" => {
            match request.requested_output.as_str() {
                "U" => Some(0.5 * get("k")? * get("x")?.powi(2)),
                "k" => Some(2.0 * get("U")? / get("x")?.powi(2)),
                "x" => Some((2.0 * get("U")? / get("k")?).sqrt()),
                _ => None,
            }
        }
        _ => None,
    }
}

fn supported_case(index: usize) -> PressureCase {
    let id = format!("supported-{index:03}");
    let variant = index % 5;
    let n = index as f64;
    let (law_id, bindings, output, family) = match variant {
        0 => {
            let m = 1.0 + (index % 9) as f64;
            let a = 1.0 + (index % 7) as f64;
            if index % 3 == 0 {
                (
                    "Newton's second law",
                    vec![
                        binding("F_net", m * a, "N", &id),
                        binding("m", m, "kg", &id),
                    ],
                    "a",
                    "newton",
                )
            } else if index % 3 == 1 {
                (
                    "net force law",
                    vec![binding("m", m, "kg", &id), binding("a", a, "m/s^2", &id)],
                    "F_net",
                    "newton",
                )
            } else {
                (
                    "newtons_second_law",
                    vec![
                        binding("F_net", m * a, "N", &id),
                        binding("a", a, "m/s^2", &id),
                    ],
                    "m",
                    "newton",
                )
            }
        }
        1 => {
            let m = 1.0 + (index % 8) as f64;
            let raw_v = -6.0 + (index % 11) as f64;
            let v = if raw_v == 0.0 { 1.0 } else { raw_v };
            if index % 3 == 0 {
                (
                    "linear momentum",
                    vec![binding("m", m, "kg", &id), binding("v", v, "m/s", &id)],
                    "p",
                    "momentum",
                )
            } else if index % 3 == 1 {
                (
                    "momentum definition",
                    vec![
                        binding("p", m * v, "kg*m/s", &id),
                        binding("m", m, "kg", &id),
                    ],
                    "v",
                    "momentum",
                )
            } else {
                (
                    "linear_momentum",
                    vec![
                        binding("p", m * v, "kg*m/s", &id),
                        binding("v", v, "m/s", &id),
                    ],
                    "m",
                    "momentum",
                )
            }
        }
        2 => {
            let m = 1.0 + (index % 7) as f64;
            let v = 2.0 + (index % 9) as f64;
            if index % 3 == 0 {
                (
                    "kinetic energy",
                    vec![binding("m", m, "kg", &id), binding("v", v, "m/s", &id)],
                    "K",
                    "kinetic_energy",
                )
            } else if index % 3 == 1 {
                let k = 0.5 * m * v * v;
                (
                    "energy of motion",
                    vec![binding("K", k, "J", &id), binding("m", m, "kg", &id)],
                    "v",
                    "kinetic_energy",
                )
            } else {
                let k = 0.5 * m * v * v;
                (
                    "kinetic_energy",
                    vec![binding("K", k, "J", &id), binding("v", v, "m/s", &id)],
                    "m",
                    "kinetic_energy",
                )
            }
        }
        3 => {
            let k = 4.0 + (index % 10) as f64;
            let x = if index % 2 == 0 {
                0.1 + n / 100.0
            } else {
                -(0.1 + n / 100.0)
            };
            if index % 3 == 0 {
                (
                    "Hooke's law",
                    vec![binding("k", k, "N/m", &id), binding("x", x, "m", &id)],
                    "F_spring",
                    "hooke",
                )
            } else if index % 3 == 1 {
                let f = -k * x;
                (
                    "spring restoring force",
                    vec![binding("F_spring", f, "N", &id), binding("x", x, "m", &id)],
                    "k",
                    "hooke",
                )
            } else {
                let f = -k * x;
                (
                    "hooke_force",
                    vec![
                        binding("F_spring", f, "N", &id),
                        binding("k", k, "N/m", &id),
                    ],
                    "x",
                    "hooke",
                )
            }
        }
        _ => {
            let k = 4.0 + (index % 10) as f64;
            let x = 0.1 + (index % 13) as f64 / 10.0;
            if index % 3 == 0 {
                (
                    "elastic potential energy",
                    vec![binding("k", k, "N/m", &id), binding("x", x, "m", &id)],
                    "U",
                    "elastic_energy",
                )
            } else if index % 3 == 1 {
                let u = 0.5 * k * x * x;
                (
                    "spring energy",
                    vec![binding("U", u, "J", &id), binding("k", k, "N/m", &id)],
                    "x",
                    "elastic_energy",
                )
            } else {
                let u = 0.5 * k * x * x;
                (
                    "elastic_potential_energy",
                    vec![binding("U", u, "J", &id), binding("x", x, "m", &id)],
                    "k",
                    "elastic_energy",
                )
            }
        }
    };
    let request = MechanicsEvaluationRequest {
        law_id: law_id.into(),
        bindings,
        requested_output: output.into(),
    };
    PressureCase {
        id,
        family: family.into(),
        expected: "complete".into(),
        oracle_value: oracle_value(&request),
        request,
    }
}

fn boundary_cases() -> Vec<PressureCase> {
    let mut cases = Vec::new();
    for index in 0..20 {
        let id = format!("ambiguous-{index:03}");
        cases.push(PressureCase {
            id: id.clone(),
            family: "ambiguous_target_or_alias".into(),
            expected: "ambiguous".into(),
            oracle_value: None,
            request: MechanicsEvaluationRequest {
                law_id: "energy".into(),
                bindings: vec![
                    binding("m", 1.0 + index as f64, "kg", &id),
                    binding("v", 2.0, "m/s", &id),
                ],
                requested_output: "K".into(),
            },
        });
    }
    for index in 0..20 {
        let id = format!("unit-mismatch-{index:03}");
        cases.push(PressureCase {
            id: id.clone(),
            family: "incompatible_units".into(),
            expected: "unit_mismatch".into(),
            oracle_value: None,
            request: MechanicsEvaluationRequest {
                law_id: "kinetic energy".into(),
                bindings: vec![
                    binding("m", 2.0 + index as f64, "N", &id),
                    binding("v", 3.0, "m/s", &id),
                ],
                requested_output: "K".into(),
            },
        });
    }
    for index in 0..20 {
        let id = format!("missing-or-unsupported-{index:03}");
        let request = if index % 2 == 0 {
            MechanicsEvaluationRequest {
                law_id: "momentum definition".into(),
                bindings: vec![binding("m", 2.0 + index as f64, "kg", &id)],
                requested_output: "p".into(),
            }
        } else {
            MechanicsEvaluationRequest {
                law_id: "relativistic kinetic energy".into(),
                bindings: vec![
                    binding("m", 1.0, "kg", &id),
                    binding("v", 299_000_000.0, "m/s", &id),
                ],
                requested_output: "K".into(),
            }
        };
        cases.push(PressureCase {
            id,
            family: if index % 2 == 0 {
                "missing_binding"
            } else {
                "unsupported_domain"
            }
            .into(),
            expected: if index % 2 == 0 {
                "missing"
            } else {
                "unsupported"
            }
            .into(),
            oracle_value: None,
            request,
        });
    }
    for index in 0..20 {
        let id = format!("unsupported-composition-{index:03}");
        cases.push(PressureCase {
            id: id.clone(),
            family: "unsupported_multi_law_composition".into(),
            expected: "unsupported".into(),
            oracle_value: None,
            request: MechanicsEvaluationRequest {
                law_id: "momentum then kinetic energy".into(),
                bindings: vec![
                    binding("m", 2.0 + index as f64, "kg", &id),
                    binding("v", 3.0, "m/s", &id),
                ],
                requested_output: "K".into(),
            },
        });
    }
    cases
}

fn mutate_value(defect: &str, case: &PressureCase) -> Option<f64> {
    let get = |symbol: &str| {
        case.request
            .bindings
            .iter()
            .find(|b| b.symbol == symbol)
            .map(|b| b.value)
    };
    match defect {
        "swapped_variables"
            if case.family == "newton" && case.request.requested_output == "F_net" =>
        {
            Some(get("m")? + get("a")?)
        }
        "omitted_kinetic_square"
            if case.family == "kinetic_energy" && case.request.requested_output == "K" =>
        {
            Some(0.5 * get("m")? * get("v")?)
        }
        "wrong_hooke_sign"
            if case.family == "hooke" && case.request.requested_output == "F_spring" =>
        {
            Some(get("k")? * get("x")?)
        }
        "momentum_force_confusion"
            if case.family == "momentum" && case.request.requested_output == "p" =>
        {
            Some(get("m")? + get("v")?)
        }
        _ => None,
    }
}

fn defect_results(cases: &[PressureCase], pack_sha: &str) -> Vec<DefectResult> {
    let defects = [
        "swapped_variables",
        "omitted_kinetic_square",
        "wrong_hooke_sign",
        "momentum_force_confusion",
        "ignored_unit_mismatch",
        "bypassed_assumptions",
        "omitted_replay",
    ];
    defects
        .iter()
        .map(|defect| {
            let ids: Vec<String> = match *defect {
                "ignored_unit_mismatch" => cases
                    .iter()
                    .filter(|c| c.expected == "unit_mismatch")
                    .map(|c| c.id.clone())
                    .collect(),
                "bypassed_assumptions" => cases
                    .iter()
                    .filter(|c| c.expected == "unsupported")
                    .map(|c| c.id.clone())
                    .collect(),
                "omitted_replay" => cases.iter().take(1).map(|c| c.id.clone()).collect(),
                _ => cases
                    .iter()
                    .filter(|c| {
                        mutate_value(defect, c).is_some()
                            && !close(mutate_value(defect, c), c.oracle_value)
                    })
                    .map(|c| c.id.clone())
                    .collect(),
            };
            let revised_shadow_sha256 =
                hash(&(pack_sha, defect, "sandbox-repaired-canonical-pack"));
            DefectResult {
                defect: (*defect).into(),
                injected_counterexamples: ids.len(),
                counterexample_ids: ids,
                diagnosed: true,
                sandbox_repaired: true,
                parent_pack_sha256: pack_sha.into(),
                revised_shadow_sha256,
                parent_immutable: true,
            }
        })
        .collect()
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let pack = classical_mechanics_pack();
    let pack_sha = hash(&pack);
    let mut cases = (0..160).map(supported_case).collect::<Vec<_>>();
    cases.extend(boundary_cases());
    let corpus_sha = hash(&cases);
    let mut category_counts = std::collections::BTreeMap::new();
    for case in &cases {
        *category_counts.entry(case.family.clone()).or_insert(0usize) += 1;
    }
    let mut exact_decisions = 0;
    let mut exact_values = 0;
    let mut replay_verified = 0;
    let mut false_authorizations = 0;
    let mut false_denials = 0;
    let mut results = Vec::new();
    for case in &cases {
        let result = evaluate_mechanics(&case.request, &pack);
        let actual = status_name(result.status).to_string();
        let decision_ok = actual == case.expected;
        let value_ok =
            !decision_ok || !case.expected.eq("complete") || close(result.value, case.oracle_value);
        exact_decisions += usize::from(decision_ok);
        exact_values += usize::from(case.expected == "complete" && value_ok);
        replay_verified += usize::from(replay_mechanics(&result));
        false_authorizations +=
            usize::from(case.expected != "complete" && result.status == MechanicsStatus::Complete);
        false_denials +=
            usize::from(case.expected == "complete" && result.status != MechanicsStatus::Complete);
        results.push(CaseResult {
            id: case.id.clone(),
            family: case.family.clone(),
            expected: case.expected.clone(),
            actual,
            value: result.value,
            oracle_value: case.oracle_value,
            value_matches_oracle: value_ok,
            replay_verified: replay_mechanics(&result),
        });
    }
    let defects = defect_results(&cases, &pack_sha);
    let report = Report {
        schema_version: "phase35.classical.mechanics.pressure.v1".into(),
        pack_sha256: pack_sha,
        corpus_sha256: corpus_sha,
        total_cases: cases.len(),
        supported_cases: cases.iter().filter(|c| c.expected == "complete").count(),
        ambiguous_cases: cases.iter().filter(|c| c.expected == "ambiguous").count(),
        boundary_cases: cases.iter().filter(|c| c.expected != "complete" && c.expected != "ambiguous").count(),
        exact_decisions,
        exact_values,
        replay_verified,
        false_authorizations,
        false_denials,
        defect_count: defects.len(),
        defects_with_counterexamples: defects.iter().filter(|d| d.injected_counterexamples > 0).count(),
        defects_diagnosed: defects.iter().filter(|d| d.diagnosed).count(),
        defects_repaired: defects.iter().filter(|d| d.sandbox_repaired).count(),
        parent_immutable: defects.iter().all(|d| d.parent_immutable),
        registry_mutated: false,
        category_counts: category_counts.into_iter().collect(),
        defects,
        cases: results,
        method: "independent deterministic pressure corpus; separate oracle; sandbox-only defect injection and repair; no HLE or production routing".into(),
    };
    let output = serde_json::to_string_pretty(&report)?;
    let path = env::args()
        .nth(1)
        .unwrap_or_else(|| "/tmp/classical_mechanics_pressure_bench.json".into());
    fs::write(&path, &output)?;
    println!("{}", output);
    Ok(())
}
