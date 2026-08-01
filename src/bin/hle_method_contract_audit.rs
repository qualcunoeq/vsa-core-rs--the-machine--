//! Phase 28 shadow gate for turning HLE method families into contracts.
//!
//! A family is eligible only when its grounded cases share a semantic method
//! signature. This prevents two unrelated specialist questions from being
//! hidden behind one broad capability name.

use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::env;
use std::fs;

const DATASET: &str = "data/hle.jsonl";

#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
enum MethodFamily {
    GeometryInequality,
    NumberTheory,
    FractalDimension,
    GraphTheory,
    DifferentialEquation,
    LinearAlgebra,
    ProbabilityStatistics,
    AppliedPde,
    SetTheory,
    RealAnalysis,
    Other,
}

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
enum MethodSignature {
    QuadraticImageShape,
    EuclideanCircleRatio,
    RandomSeriesExpectation,
    ModularPowerCardinality,
    SectionDimensionBound,
    SelfSimilarComponentCount,
    Generic,
}

#[derive(Debug, Deserialize)]
struct Phase27Report {
    grounding_report_sha256: String,
    dataset_sha256: String,
    requirements: Vec<Requirement>,
}

#[derive(Debug, Deserialize)]
struct Requirement {
    id: String,
    family: MethodFamily,
    input_artifact_type: String,
    requested_output_artifact: String,
    operation: String,
    prerequisites: Vec<String>,
    nearest_existing_capability: String,
    interface_mismatch: bool,
}

#[derive(Debug, Serialize)]
struct BridgeContract {
    source_artifact_type: String,
    target_solver_artifact: String,
    required_bindings: Vec<String>,
    invariants: Vec<String>,
    ambiguity_boundaries: Vec<String>,
    rejection_boundaries: Vec<String>,
}

#[derive(Debug, Serialize)]
struct MethodContract {
    accepted_problem_form: String,
    transformation_or_theorem: String,
    assumptions: Vec<String>,
    output_type: String,
    replay_obligations: Vec<String>,
}

#[derive(Debug, Serialize)]
struct FamilyDecision {
    family: MethodFamily,
    case_ids: Vec<String>,
    signatures: BTreeMap<MethodSignature, Vec<String>>,
    coherent: bool,
    decision: String,
    bridge_contract: Option<BridgeContract>,
    method_contract: Option<MethodContract>,
}

#[derive(Debug, Serialize)]
struct Report {
    phase27_report_sha256: String,
    phase27_grounding_report_sha256: String,
    dataset_sha256: String,
    families_considered: usize,
    coherent_families: usize,
    selected_family: Option<MethodFamily>,
    decisions: Vec<FamilyDecision>,
    method: String,
}

fn hash(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

fn signature_for(question: &str, family: MethodFamily) -> MethodSignature {
    let text = question.to_ascii_lowercase();
    match family {
        MethodFamily::GeometryInequality
            if text.contains("shape of s") || text.contains("shape of $") =>
        {
            MethodSignature::QuadraticImageShape
        }
        MethodFamily::GeometryInequality if text.contains("circumcircle") => {
            MethodSignature::EuclideanCircleRatio
        }
        MethodFamily::NumberTheory if text.contains("expected") && text.contains("sum") => {
            MethodSignature::RandomSeriesExpectation
        }
        MethodFamily::NumberTheory if text.contains("mod") && text.contains("cardinality") => {
            MethodSignature::ModularPowerCardinality
        }
        MethodFamily::FractalDimension
            if text.contains("intersection")
                || text.contains("dimension of l")
                || text.contains("dimension of $") =>
        {
            MethodSignature::SectionDimensionBound
        }
        MethodFamily::FractalDimension if text.contains("components") || text.contains("union") => {
            MethodSignature::SelfSimilarComponentCount
        }
        _ => MethodSignature::Generic,
    }
}

fn dataset_questions() -> Result<(String, BTreeMap<String, String>), Box<dyn std::error::Error>> {
    let bytes = fs::read(DATASET)?;
    let mut questions = BTreeMap::new();
    for line in String::from_utf8(bytes.clone())?.lines() {
        let row: Value = serde_json::from_str(line)?;
        if let (Some(id), Some(question)) = (
            row.get("id").and_then(Value::as_str),
            row.get("question").and_then(Value::as_str),
        ) {
            questions.insert(id.to_string(), question.to_string());
        }
    }
    Ok((hash(&bytes), questions))
}

fn decision(
    family: MethodFamily,
    requirements: &[&Requirement],
    questions: &BTreeMap<String, String>,
) -> FamilyDecision {
    let mut signatures: BTreeMap<MethodSignature, Vec<String>> = BTreeMap::new();
    for requirement in requirements {
        let signature = questions
            .get(&requirement.id)
            .map(|question| signature_for(question, family))
            .unwrap_or(MethodSignature::Generic);
        signatures
            .entry(signature)
            .or_default()
            .push(requirement.id.clone());
    }
    let case_ids = requirements
        .iter()
        .map(|r| r.id.clone())
        .collect::<Vec<_>>();
    let coherent = signatures.len() == 1
        && !signatures.contains_key(&MethodSignature::Generic)
        && requirements
            .iter()
            .all(|r| r.interface_mismatch && !r.nearest_existing_capability.is_empty());
    let (bridge_contract, method_contract) = if coherent {
        let output = requirements[0].requested_output_artifact.clone();
        (
            Some(BridgeContract {
                source_artifact_type: requirements[0].input_artifact_type.clone(),
                target_solver_artifact: output.clone(),
                required_bindings: vec![
                    "target-linked math region".into(),
                    "all locally scoped symbols".into(),
                    "provenance spans for definitions and assumptions".into(),
                ],
                invariants: vec![
                    "selected region is uniquely linked to the question target".into(),
                    "symbol bindings are deterministic and replayable".into(),
                    "no external convention is silently inserted".into(),
                ],
                ambiguity_boundaries: vec![
                    "multiple target regions".into(),
                    "overloaded or unresolved symbols".into(),
                ],
                rejection_boundaries: vec![
                    "unsupported operator or representation".into(),
                    "missing target-linked definition".into(),
                ],
            }),
            Some(MethodContract {
                accepted_problem_form: format!(
                    "grounded target with {:?} semantics",
                    signatures.keys().next().unwrap()
                ),
                transformation_or_theorem: requirements[0].operation.clone(),
                assumptions: requirements
                    .iter()
                    .flat_map(|r| r.prerequisites.iter().cloned())
                    .collect(),
                output_type: output,
                replay_obligations: vec![
                    "reconstruct the typed input artifact".into(),
                    "replay every transformation".into(),
                    "verify the final output against the independent oracle".into(),
                ],
            }),
        )
    } else {
        (None, None)
    };
    let decision = if coherent {
        "eligible for independent contract corpus".into()
    } else {
        "defer; family contains distinct method signatures or unresolved interface evidence".into()
    };
    FamilyDecision {
        family,
        case_ids,
        signatures,
        coherent,
        decision,
        bridge_contract,
        method_contract,
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let input = env::args()
        .nth(1)
        .unwrap_or_else(|| "docs/phase27_hle_method_audit.json".into());
    let output = env::args()
        .nth(2)
        .unwrap_or_else(|| "/tmp/hle_method_contract_audit_2147e9e.json".into());
    let bytes = fs::read(&input)?;
    let report: Phase27Report = serde_json::from_slice(&bytes)?;
    let (dataset_sha256, questions) = dataset_questions()?;
    if dataset_sha256 != report.dataset_sha256 {
        return Err("Phase 27 report and HLE dataset hashes differ".into());
    }
    let mut grouped: BTreeMap<MethodFamily, Vec<&Requirement>> = BTreeMap::new();
    for requirement in &report.requirements {
        grouped
            .entry(requirement.family)
            .or_default()
            .push(requirement);
    }
    let mut decisions = grouped
        .iter()
        .filter(|(_, requirements)| requirements.len() >= 2)
        .map(|(family, requirements)| decision(*family, requirements, &questions))
        .collect::<Vec<_>>();
    decisions.sort_by(|left, right| left.family.cmp(&right.family));
    let selected_family = decisions
        .iter()
        .find(|decision| decision.coherent)
        .map(|decision| decision.family);
    let output_report = Report {
        phase27_report_sha256: hash(&bytes),
        phase27_grounding_report_sha256: report.grounding_report_sha256,
        dataset_sha256,
        families_considered: decisions.len(),
        coherent_families: decisions.iter().filter(|d| d.coherent).count(),
        selected_family,
        decisions,
        method: "shadow-only method-family coherence gate; no capability or bridge authorization"
            .into(),
    };
    fs::write(&output, serde_json::to_vec_pretty(&output_report)?)?;
    println!("{}", serde_json::to_string_pretty(&output_report)?);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn geometry_cases_do_not_collapse_into_one_method() {
        let a = signature_for(
            "What is the shape of S from inner products?",
            MethodFamily::GeometryInequality,
        );
        let b = signature_for(
            "A point lies on the circumcircle of a triangle.",
            MethodFamily::GeometryInequality,
        );
        assert_ne!(a, b);
    }

    #[test]
    fn number_theory_cases_do_not_collapse_into_one_method() {
        let a = signature_for(
            "Find the expected value of a random sum.",
            MethodFamily::NumberTheory,
        );
        let b = signature_for(
            "Find the cardinality of powers modulo 22.",
            MethodFamily::NumberTheory,
        );
        assert_ne!(a, b);
    }
}
