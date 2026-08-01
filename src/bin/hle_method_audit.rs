//! Phase 27 shadow audit of downstream method requirements.
//!
//! This tool classifies already-grounded HLE targets into reusable method
//! families. It proposes no capability, changes no registry, and never turns
//! a family label into authorization.

use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::env;
use std::fs;

const DATASET: &str = "data/hle.jsonl";

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
enum MethodFamily {
    GeometryInequality,
    AbstractAlgebra,
    FractalDimension,
    GraphTheory,
    DifferentialEquation,
    CategoryTheory,
    AlgorithmComplexity,
    NumberTheory,
    Topology,
    LinearAlgebra,
    ProbabilityStatistics,
    AppliedPde,
    Combinatorics,
    Optimization,
    SetTheory,
    RealAnalysis,
    Unclassified,
}

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
enum ArtifactType {
    GroundedMathTarget,
    ScalarAnswer,
    ExpressionAnswer,
    BooleanAnswer,
    SetOrRangeAnswer,
    ComplexityBound,
    CardinalityAnswer,
}

#[derive(Debug, Serialize)]
struct MethodRequirement {
    id: Option<String>,
    family: MethodFamily,
    input_artifact_type: ArtifactType,
    requested_output_artifact: ArtifactType,
    operation: String,
    prerequisites: Vec<String>,
    reusable: bool,
    nearest_existing_capability: String,
    interface_mismatch: bool,
    unsupported_operator_or_representation: Vec<String>,
    evidence: Vec<String>,
}

#[derive(Debug, Serialize)]
struct MethodFamilySummary {
    family: MethodFamily,
    cases: usize,
    reusable_cases: usize,
    input_artifact_types: Vec<ArtifactType>,
    requested_output_artifacts: Vec<ArtifactType>,
    shared_operation: String,
    prerequisite_families: Vec<String>,
    nearest_existing_capabilities: Vec<String>,
    interface_mismatch_cases: usize,
    contract_status: String,
}

#[derive(Debug, Deserialize)]
struct GroundingReport {
    hle_records: Vec<GroundingRecord>,
}

#[derive(Debug, Deserialize)]
struct GroundingRecord {
    id: Option<String>,
    grounding_status: String,
}

#[derive(Debug, Serialize)]
struct Report {
    grounding_report_sha256: String,
    dataset_sha256: String,
    grounded_cases: usize,
    family_counts: BTreeMap<MethodFamily, usize>,
    reusable_cases: usize,
    interface_mismatch_cases: usize,
    prerequisite_knowledge_cases: usize,
    largest_family: Option<MethodFamily>,
    largest_family_cases: usize,
    requirements: Vec<MethodRequirement>,
    family_summaries: Vec<MethodFamilySummary>,
    method: String,
}

fn hash(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

fn contains_any(text: &str, terms: &[&str]) -> bool {
    terms.iter().any(|term| text.contains(term))
}

fn family_for(question: &str) -> (MethodFamily, Vec<String>) {
    let text = question.to_ascii_lowercase();
    let mut evidence = Vec::new();
    let patterns = [
        (
            MethodFamily::GeometryInequality,
            &["triangle", "angle", "side lengths", "acute"] as &[&str],
        ),
        (
            MethodFamily::AbstractAlgebra,
            &["magma", "cancellable", "medial", "monoid"],
        ),
        (
            MethodFamily::FractalDimension,
            &[
                "dimension of",
                "compact set",
                "dimension of c",
                "components of f",
                "unit square",
                "self-similar",
            ],
        ),
        (
            MethodFamily::GraphTheory,
            &[
                "graph",
                "treewidth",
                "planar",
                "crossing",
                "chess board",
                "colony",
                "alontarsi",
            ],
        ),
        (
            MethodFamily::DifferentialEquation,
            &["initial value problem", "x'(t)", "differential equation"],
        ),
        (
            MethodFamily::CategoryTheory,
            &[
                "day convolution",
                "delooping",
                "functor",
                "hom_",
                "higher central charge",
                "orbifold euler",
                "quotient stack",
            ],
        ),
        (
            MethodFamily::AlgorithmComplexity,
            &["algorithm", "computational time", "complexity", "base c"],
        ),
        (
            MethodFamily::NumberTheory,
            &["floor", "modulo", " mod ", "\\pmod", "congruent", "integer"],
        ),
        (
            MethodFamily::Topology,
            &["compact", "open sub-basis", "topology"],
        ),
        (
            MethodFamily::LinearAlgebra,
            &["inequality holds", "sum_{", "eigen", "quadratic form"],
        ),
        (
            MethodFamily::ProbabilityStatistics,
            &[
                "minimax risk",
                "bin(",
                "loss",
                "i.i.d.",
                "expected time",
                "expected number",
                "probability",
                "poker",
            ],
        ),
        (
            MethodFamily::AppliedPde,
            &["electro-osmotic", "potential distribution", "debye"],
        ),
        (
            MethodFamily::Combinatorics,
            &[
                "partition of",
                "partition an",
                "number of ways",
                "generating function",
                "inclusion-exclusion",
                "weak order",
            ],
        ),
        (
            MethodFamily::Optimization,
            &[
                "pareto",
                "scalarization",
                "minimize",
                "maximize",
                "evolutionary algorithm",
            ],
        ),
        (
            MethodFamily::SetTheory,
            &["ultrafilter", "antichain", "finite-to-one", "cardinality"],
        ),
        (
            MethodFamily::RealAnalysis,
            &[
                "lower and upper bounds",
                "real number",
                "there always exist",
                "interval",
            ],
        ),
    ];
    for (family, terms) in patterns {
        if contains_any(&text, terms) {
            evidence.push(format!("matched {:?} lexical semantic anchors", family));
            return (family, evidence);
        }
    }
    evidence.push("no reusable method-family anchor".into());
    (MethodFamily::Unclassified, evidence)
}

fn output_for(question: &str) -> ArtifactType {
    let text = question.to_ascii_lowercase();
    if contains_any(
        &text,
        &["yes or no", "true or false", "which of the following"],
    ) {
        ArtifactType::BooleanAnswer
    } else if contains_any(&text, &["range of values", "interval", "set of"]) {
        ArtifactType::SetOrRangeAnswer
    } else if contains_any(&text, &["complexity", "big-o", "o(log", "time complexity"]) {
        ArtifactType::ComplexityBound
    } else if contains_any(&text, &["cardinality", "count the number", "how many"]) {
        ArtifactType::CardinalityAnswer
    } else if contains_any(&text, &["expression", "formula", "derive"]) {
        ArtifactType::ExpressionAnswer
    } else {
        ArtifactType::ScalarAnswer
    }
}

fn requirement(id: Option<String>, question: &str) -> MethodRequirement {
    let (family, mut evidence) = family_for(question);
    let text = question.to_ascii_lowercase();
    let output = output_for(question);
    let prerequisites = if contains_any(
        &text,
        &[
            "theorem",
            "law",
            "standard",
            "definition",
            "nccn",
            "convention",
        ],
    ) {
        vec!["external theorem, law, or domain convention with validity conditions".into()]
    } else {
        Vec::new()
    };
    let unsupported = [
        ("\\frac", "fractional expression"),
        ("\\sum", "summation notation"),
        ("\\log", "logarithm notation"),
        ("\\lfloor", "floor notation"),
        ("\\operatorname", "named operator"),
        ("matrix", "matrix/layout representation"),
        ("x'(t)", "differential operator"),
        ("\\circledast", "custom convolution operator"),
    ];
    let unsupported_operator_or_representation: Vec<String> = unsupported
        .iter()
        .filter(|(marker, _)| text.contains(marker))
        .map(|(_, label)| (*label).to_string())
        .collect();
    let (nearest, mismatch) = match family {
        MethodFamily::DifferentialEquation | MethodFamily::AppliedPde => {
            ("algebra/math parser", true)
        }
        MethodFamily::LinearAlgebra => ("linear-system executor", true),
        MethodFamily::GeometryInequality => ("algebra and proposition verifier", true),
        MethodFamily::NumberTheory | MethodFamily::AlgorithmComplexity => ("algebra island", true),
        MethodFamily::ProbabilityStatistics => ("algebra island", true),
        MethodFamily::Combinatorics
        | MethodFamily::Optimization
        | MethodFamily::SetTheory
        | MethodFamily::RealAnalysis => ("algebra and proposition verifier", true),
        MethodFamily::GraphTheory
        | MethodFamily::Topology
        | MethodFamily::AbstractAlgebra
        | MethodFamily::FractalDimension
        | MethodFamily::CategoryTheory => ("proposition verifier", true),
        MethodFamily::Unclassified => ("none", false),
    };
    evidence.push(format!("requested output classified as {:?}", output));
    evidence.push(if mismatch {
        "existing capability is adjacent but lacks a typed bridge for this family".into()
    } else {
        "no adjacent capability identified".into()
    });
    MethodRequirement {
        id,
        family,
        input_artifact_type: ArtifactType::GroundedMathTarget,
        requested_output_artifact: output,
        operation: format!(
            "derive {:?} from a grounded target using {:?} semantics",
            output, family
        ),
        prerequisites,
        reusable: family != MethodFamily::Unclassified,
        nearest_existing_capability: nearest.into(),
        interface_mismatch: mismatch,
        unsupported_operator_or_representation,
        evidence,
    }
}

fn dataset_questions() -> Result<(String, BTreeMap<String, String>), Box<dyn std::error::Error>> {
    let bytes = fs::read(DATASET)?;
    let mut questions = BTreeMap::new();
    for line in String::from_utf8(bytes.clone())?.lines() {
        let entry: Value = serde_json::from_str(line)?;
        if let (Some(id), Some(question)) = (
            entry.get("id").and_then(Value::as_str),
            entry.get("question").and_then(Value::as_str),
        ) {
            questions.insert(id.to_string(), question.to_string());
        }
    }
    Ok((hash(&bytes), questions))
}

fn summarize_family(
    family: MethodFamily,
    requirements: &[MethodRequirement],
) -> MethodFamilySummary {
    let cases = requirements.iter().filter(|r| r.family == family).count();
    let reusable_cases = requirements
        .iter()
        .filter(|r| r.family == family && r.reusable)
        .count();
    let mut inputs = requirements
        .iter()
        .filter(|r| r.family == family)
        .map(|r| r.input_artifact_type)
        .collect::<Vec<_>>();
    inputs.sort();
    inputs.dedup();
    let mut outputs = requirements
        .iter()
        .filter(|r| r.family == family)
        .map(|r| r.requested_output_artifact)
        .collect::<Vec<_>>();
    outputs.sort();
    outputs.dedup();
    let mut capabilities = requirements
        .iter()
        .filter(|r| r.family == family)
        .map(|r| r.nearest_existing_capability.clone())
        .collect::<Vec<_>>();
    capabilities.sort();
    capabilities.dedup();
    let mut prerequisite_families = requirements
        .iter()
        .filter(|r| r.family == family)
        .flat_map(|r| r.prerequisites.iter().cloned())
        .collect::<Vec<_>>();
    prerequisite_families.sort();
    prerequisite_families.dedup();
    let operation = requirements
        .iter()
        .find(|r| r.family == family)
        .map(|r| r.operation.clone())
        .unwrap_or_else(|| "no operation identified".into());
    let interface_mismatch_cases = requirements
        .iter()
        .filter(|r| r.family == family && r.interface_mismatch)
        .count();
    let contract_status = if family == MethodFamily::Unclassified {
        "unclassified; no contract proposal".into()
    } else if interface_mismatch_cases == cases {
        "candidate reusable method; typed bridge required".into()
    } else {
        "candidate reusable method; boundary requires independent validation".into()
    };
    MethodFamilySummary {
        family,
        cases,
        reusable_cases,
        input_artifact_types: inputs,
        requested_output_artifacts: outputs,
        shared_operation: operation,
        prerequisite_families,
        nearest_existing_capabilities: capabilities,
        interface_mismatch_cases,
        contract_status,
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let grounding_path = env::args()
        .nth(1)
        .unwrap_or_else(|| "/tmp/hle_notation_grounding_2147e9e.json".into());
    let output = env::args()
        .nth(2)
        .unwrap_or_else(|| "/tmp/hle_method_audit_2147e9e.json".into());
    let grounding_bytes = fs::read(&grounding_path)?;
    let grounding: GroundingReport = serde_json::from_slice(&grounding_bytes)?;
    let (dataset_sha256, questions) = dataset_questions()?;
    let mut requirements = Vec::new();
    for row in grounding
        .hle_records
        .into_iter()
        .filter(|row| row.grounding_status == "accepted")
    {
        let Some(id) = row.id.clone() else { continue };
        let Some(question) = questions.get(&id) else {
            continue;
        };
        requirements.push(requirement(Some(id), question));
    }
    let mut family_counts = BTreeMap::new();
    for requirement in &requirements {
        *family_counts.entry(requirement.family).or_insert(0) += 1;
    }
    let mut family_summaries = family_counts
        .keys()
        .copied()
        .map(|family| summarize_family(family, &requirements))
        .collect::<Vec<_>>();
    family_summaries.sort_by(|left, right| {
        right
            .cases
            .cmp(&left.cases)
            .then_with(|| left.family.cmp(&right.family))
    });
    let (largest_family, largest_family_cases) = family_summaries
        .first()
        .map(|summary| (Some(summary.family), summary.cases))
        .unwrap_or((None, 0));
    let report = Report {
        grounding_report_sha256: hash(&grounding_bytes),
        dataset_sha256,
        grounded_cases: requirements.len(),
        reusable_cases: requirements.iter().filter(|r| r.reusable).count(),
        interface_mismatch_cases: requirements.iter().filter(|r| r.interface_mismatch).count(),
        prerequisite_knowledge_cases: requirements
            .iter()
            .filter(|r| !r.prerequisites.is_empty())
            .count(),
        largest_family,
        largest_family_cases,
        family_counts,
        requirements,
        family_summaries,
        method: "shadow-only deterministic downstream method audit; family labels are diagnostic and non-authorizing".into(),
    };
    fs::write(&output, serde_json::to_vec_pretty(&report)?)?;
    println!("{}", serde_json::to_string_pretty(&report)?);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn method_families_are_reusable_and_deterministic() {
        let cases = [
            (
                "Find the acute triangle angle range.",
                MethodFamily::GeometryInequality,
            ),
            (
                "For a magma, determine whether it is medial.",
                MethodFamily::AbstractAlgebra,
            ),
            (
                "Find the computational time complexity in base c.",
                MethodFamily::AlgorithmComplexity,
            ),
            (
                "Solve the initial value problem x'(t)=...",
                MethodFamily::DifferentialEquation,
            ),
            (
                "Find minimax risk under square error loss.",
                MethodFamily::ProbabilityStatistics,
            ),
        ];
        for (question, expected) in cases {
            let (actual, _) = family_for(question);
            assert_eq!(actual, expected);
            assert!(requirement(None, question).reusable);
        }
    }

    #[test]
    fn interface_gaps_are_reported_without_authorization() {
        let audit = requirement(
            None,
            "Find the expression for an electro-osmotic potential distribution.",
        );
        assert!(audit.interface_mismatch);
        assert!(audit.reusable);
        assert_eq!(audit.input_artifact_type, ArtifactType::GroundedMathTarget);
    }

    #[test]
    fn family_summary_exposes_reuse_and_bridge_boundary() {
        let requirements = vec![
            requirement(
                None,
                "Find the computational time complexity of the algorithm.",
            ),
            requirement(None, "Find the computational time complexity in base c."),
        ];
        let summary = summarize_family(MethodFamily::AlgorithmComplexity, &requirements);
        assert_eq!(summary.cases, 2);
        assert_eq!(summary.reusable_cases, 2);
        assert_eq!(summary.interface_mismatch_cases, 2);
        assert!(summary.contract_status.contains("typed bridge"));
    }
}
