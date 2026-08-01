//! Phase 40 audit of mechanics-signal HLE questions whose requested target
//! could not be grounded by MechanicsSituationV1.
//!
//! This is a diagnostic taxonomy pass.  It does not infer an answer, invoke a
//! production capability, or promote a new ontology/method.

use serde::Serialize;
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::env;
use std::fs;

const DATASET: &str = "data/hle.jsonl";
const PHASE39_REPORT: &str = "docs/phase39_hle_mechanics_situation_shadow.json";

#[derive(Debug, Serialize)]
struct TargetRecord {
    id: Option<String>,
    category: String,
    raw_subject: String,
    question_sha256: String,
    artifact_family_candidates: Vec<String>,
    artifact_family: String,
    subdomain_candidates: Vec<String>,
    subdomain: String,
    indicators: Vec<String>,
    first_failing_gate: String,
}

#[derive(Debug, Serialize)]
struct Report {
    schema_version: String,
    dataset: String,
    dataset_sha256: String,
    phase39_report_sha256: String,
    audited_cases: usize,
    artifact_family_counts: BTreeMap<String, usize>,
    subdomain_counts: BTreeMap<String, usize>,
    cross_family_subdomain_counts: BTreeMap<String, usize>,
    unclassified_target_cases: usize,
    ambiguous_artifact_cases: usize,
    ambiguous_subdomain_cases: usize,
    records: Vec<TargetRecord>,
    method: String,
}

fn sha256(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

fn contains_any(text: &str, markers: &[&str]) -> bool {
    markers.iter().any(|marker| text.contains(marker))
}

fn contains_term(text: &str, marker: &str) -> bool {
    let mut offset = 0;
    while let Some(found) = text[offset..].find(marker) {
        let start = offset + found;
        let end = start + marker.len();
        let left_ok = start == 0 || !text.as_bytes()[start - 1].is_ascii_alphanumeric();
        let right_ok = end == text.len() || !text.as_bytes()[end].is_ascii_alphanumeric();
        if left_ok && right_ok {
            return true;
        }
        offset = end;
    }
    false
}

fn mechanics_signal(text: &str) -> bool {
    contains_any(
        text,
        &[
            "force",
            "mass",
            "velocity",
            "speed",
            "acceleration",
            "momentum",
            "kinetic",
            "spring",
            "displacement",
            "elastic",
            "mechanical energy",
            "projectile",
            "inertial",
        ],
    )
}

fn classify_artifact_family(text: &str) -> (Vec<String>, Vec<String>) {
    let mut candidates = Vec::new();
    let mut indicators = Vec::new();
    let families: &[(&str, &[&str])] = &[
        (
            "prove_or_derive_relation",
            &["prove", "derive", "show that", "establish", "demonstrate"],
        ),
        (
            "asymptotic_or_scaling_law",
            &["asymptotic", "scaling", "order of", "growth rate", "limit"],
        ),
        (
            "spectrum_eigenvalue_or_mode",
            &["eigenvalue", "eigenvector", "spectrum", "spectral", "mode"],
        ),
        (
            "optimization_or_bound",
            &[
                "maximize",
                "maximise",
                "minimize",
                "minimise",
                "optimize",
                "optimise",
                "bound",
                "upper bound",
                "lower bound",
            ],
        ),
        (
            "dimensionless_parameter",
            &[
                "dimensionless",
                "dimensionless number",
                "reynolds",
                "mach number",
                "froude",
            ],
        ),
        (
            "integral_or_differential_equation",
            &[
                "integral",
                "integrate",
                "derivative",
                "differential equation",
                "ordinary differential",
                "partial differential",
                "pde",
                "ode",
            ],
        ),
        (
            "physical_phenomenon_or_model",
            &[
                "phenomenon",
                "physical model",
                "model",
                "mechanism",
                "process",
            ],
        ),
        (
            "conceptual_consequence",
            &[
                "explain",
                "why does",
                "why is",
                "consequence",
                "interpret",
                "meaning",
                "intuition",
            ],
        ),
        (
            "named_object_theorem_or_convention",
            &[
                "theorem",
                "principle",
                "law",
                "definition",
                "convention",
                "named",
            ],
        ),
        (
            "specialist_factual_target",
            &["who", "which scientist", "identify", "name the"],
        ),
    ];
    for (family, markers) in families {
        let matched: Vec<&str> = markers
            .iter()
            .copied()
            .filter(|marker| contains_term(text, marker))
            .collect();
        if !matched.is_empty() {
            candidates.push((*family).to_string());
            indicators.extend(matched.into_iter().map(str::to_string));
        }
    }
    (candidates, indicators)
}

fn classify_subdomain(text: &str) -> Vec<String> {
    let domains: &[(&str, &[&str])] = &[
        (
            "analytical_mechanics",
            &[
                "lagrangian",
                "lagrange",
                "hamiltonian",
                "canonical",
                "action principle",
                "phase space",
            ],
        ),
        (
            "continuum_mechanics",
            &[
                "continuum",
                "fluid",
                "fluid mechanics",
                "stress",
                "strain",
                "viscosity",
                "navier",
            ],
        ),
        (
            "statistical_mechanics",
            &[
                "statistical mechanics",
                "entropy",
                "partition function",
                "ensemble",
                "boltzmann",
            ],
        ),
        (
            "quantum_mechanics",
            &[
                "quantum",
                "wavefunction",
                "schrodinger",
                "schrödinger",
                "commutator",
                "spin",
            ],
        ),
        (
            "field_theory",
            &[
                "field theory",
                "gauge",
                "lagrangian density",
                "quantum field",
                "electromagnetic field",
            ],
        ),
        (
            "relativity",
            &[
                "relativistic",
                "relativity",
                "lorentz",
                "spacetime",
                "minkowski",
                "geodesic",
            ],
        ),
        (
            "dynamical_systems",
            &[
                "dynamical system",
                "stability",
                "stable",
                "fixed point",
                "bifurcation",
                "lyapunov",
                "chaos",
                "phase portrait",
            ],
        ),
        (
            "mathematical_physics",
            &[
                "mathematical physics",
                "green's function",
                "boundary value",
                "operator",
                "pde",
                "partial differential",
            ],
        ),
    ];
    domains
        .iter()
        .filter(|(_, markers)| markers.iter().any(|marker| contains_term(text, marker)))
        .map(|(domain, _)| (*domain).to_string())
        .collect()
}

fn named_classification(candidates: &[String], fallback: &str) -> String {
    match candidates {
        [] => fallback.into(),
        [one] => one.clone(),
        _ => "ambiguous".into(),
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let dataset_bytes = fs::read(DATASET)?;
    let phase39_bytes = fs::read(PHASE39_REPORT)?;
    let phase39: Value = serde_json::from_slice(&phase39_bytes)?;
    let target_ids: std::collections::BTreeSet<String> = phase39
        .get("records")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter(|record| {
            record.get("first_failing_gate").and_then(Value::as_str)
                == Some("target_not_groundable")
        })
        .filter_map(|record| record.get("id").and_then(Value::as_str).map(str::to_string))
        .collect();
    let mut records = Vec::new();
    let mut artifact_family_counts = BTreeMap::new();
    let mut subdomain_counts = BTreeMap::new();
    let mut cross_family_subdomain_counts = BTreeMap::new();
    let mut unclassified_target_cases = 0;
    let mut ambiguous_artifact_cases = 0;
    let mut ambiguous_subdomain_cases = 0;
    for line in String::from_utf8(dataset_bytes.clone())?
        .lines()
        .filter(|line| !line.trim().is_empty())
    {
        let entry: Value = serde_json::from_str(line)?;
        let id = entry.get("id").and_then(Value::as_str).map(str::to_string);
        if id.as_ref().is_none_or(|value| !target_ids.contains(value)) {
            continue;
        }
        let question = entry.get("question").and_then(Value::as_str).unwrap_or("");
        let lower = question.to_ascii_lowercase();
        if !mechanics_signal(&lower) {
            continue;
        }
        let (artifact_candidates, mut indicators) = classify_artifact_family(&lower);
        let subdomain_candidates = classify_subdomain(&lower);
        if artifact_candidates.is_empty() {
            unclassified_target_cases += 1;
        }
        if artifact_candidates.len() > 1 {
            ambiguous_artifact_cases += 1;
        }
        if subdomain_candidates.len() > 1 {
            ambiguous_subdomain_cases += 1;
        }
        let artifact_family = named_classification(&artifact_candidates, "unclassified");
        let subdomain = named_classification(&subdomain_candidates, "unclassified");
        indicators.sort();
        indicators.dedup();
        *artifact_family_counts
            .entry(artifact_family.clone())
            .or_insert(0) += 1;
        *subdomain_counts.entry(subdomain.clone()).or_insert(0) += 1;
        let cross_key = format!("{artifact_family}::{subdomain}");
        *cross_family_subdomain_counts.entry(cross_key).or_insert(0) += 1;
        records.push(TargetRecord {
            id,
            category: entry
                .get("category")
                .and_then(Value::as_str)
                .unwrap_or("uncategorized")
                .into(),
            raw_subject: entry
                .get("raw_subject")
                .and_then(Value::as_str)
                .unwrap_or("uncategorized")
                .into(),
            question_sha256: sha256(question.as_bytes()),
            artifact_family_candidates: artifact_candidates,
            artifact_family,
            subdomain_candidates,
            subdomain,
            indicators,
            first_failing_gate: "target_not_groundable".into(),
        });
    }
    let report = Report {
        schema_version: "phase40.hle.mechanics.target.audit.v1".into(),
        dataset: DATASET.into(),
        dataset_sha256: sha256(&dataset_bytes),
        phase39_report_sha256: sha256(&phase39_bytes),
        audited_cases: records.len(),
        artifact_family_counts,
        subdomain_counts,
        cross_family_subdomain_counts,
        unclassified_target_cases,
        ambiguous_artifact_cases,
        ambiguous_subdomain_cases,
        records,
        method: "diagnostic audit of Phase 39 mechanics-signal target-not-groundable cases; lexical indicators are preserved as candidates and never authorize execution".into(),
    };
    let output = serde_json::to_string_pretty(&report)?;
    let path = env::args()
        .nth(1)
        .unwrap_or_else(|| "docs/phase40_hle_mechanics_target_audit.json".into());
    fs::write(&path, output)?;
    println!("phase40 report written to {path}");
    Ok(())
}
