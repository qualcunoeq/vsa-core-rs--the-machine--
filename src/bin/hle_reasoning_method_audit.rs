//! Shadow coherence audit for the full HLE missing-reasoning-method pool.
//!
//! This is deliberately stricter than subject clustering. A reusable family
//! requires the same typed transformation signature and output artifact; no
//! capability or bridge is proposed by this binary.

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::env;
use std::fs;

const DATASET: &str = "data/hle.jsonl";

#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
enum MethodSignature {
    RecurrenceEvaluation,
    RecurrenceClosedForm,
    MatrixSpectralInvariant,
    MatrixDecomposition,
    MatrixManifoldExponential,
    MatrixExponentialProjection,
    MatrixLogGaussianSampling,
    ExponentialOperatorDeterminant,
    MatrixManifoldConstraint,
    RiemannianMetricStructure,
    PoincareDiskGeometry,
    LorentzSampling,
    FunctionSampling,
    LieAlgebraMatrix,
    MatrixLogSampling,
    MatrixRankOrDeterminant,
    GroupTheoryStructure,
    CombinatorialEnumeration,
    NumberTheoryCongruence,
    NumberTheoryPrimeDensity,
    ProbabilityExpectation,
    ProbabilityDistribution,
    DifferentialEquationSolve,
    CalculusTransformation,
    GraphInvariant,
    GeometricConstruction,
    AsymptoticRuntime,
    InequalityBound,
    TheoremApplication,
    ScientificLawApplication,
    GenericSpecialist,
}

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
enum AuditClass {
    TransformationCandidate,
    RepresentationBridge,
    KnowledgeDependent,
    CompositionGap,
    SpecialistSingleton,
    AmbiguousOrContaminated,
}

#[derive(Debug, Deserialize)]
struct TraceRow {
    id: String,
    question: String,
    terminal_classification: String,
    route_trace: Vec<String>,
    required_capabilities: Vec<String>,
}

#[derive(Debug, Serialize)]
struct MethodCase {
    id: String,
    signature: MethodSignature,
    output_artifact: String,
    audit_class: AuditClass,
    prerequisite_cues: Vec<String>,
    representation_cues: Vec<String>,
    route_trace_len: usize,
}

#[derive(Debug, Serialize)]
struct FamilySummary {
    signature: MethodSignature,
    cases: usize,
    output_artifacts: BTreeMap<String, usize>,
    case_ids: Vec<String>,
    coherent: bool,
    contract_readiness: String,
}

#[derive(Debug, Serialize)]
struct Report {
    trace_sha256: String,
    dataset_sha256: String,
    missing_method_cases: usize,
    class_counts: BTreeMap<AuditClass, usize>,
    signature_counts: BTreeMap<MethodSignature, usize>,
    coherent_families: usize,
    cases: Vec<MethodCase>,
    families: Vec<FamilySummary>,
    method: String,
}

fn hash(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

fn has_any(text: &str, terms: &[&str]) -> bool {
    terms.iter().any(|term| text.contains(term))
}

fn signature(question: &str) -> MethodSignature {
    let text = question.to_ascii_lowercase();
    if has_any(&text, &["recurrence", "a_{n+1}", "a_{n+1}", "sequence of"]) {
        if has_any(
            &text,
            &["closed form", "explicit formula", "formula for a_n"],
        ) {
            MethodSignature::RecurrenceClosedForm
        } else {
            MethodSignature::RecurrenceEvaluation
        }
    } else if text.contains("riemannian exponential map") {
        MethodSignature::MatrixManifoldExponential
    } else if has_any(&text, &["matrix logarithm", "log (m)", "log denoted"])
        && has_any(&text, &["matrix", "sample", "density"])
    {
        MethodSignature::MatrixLogGaussianSampling
    } else if has_any(&text, &["expm", "matrix exponential"])
        && has_any(&text, &["operator defined", "proj_", "det"])
    {
        MethodSignature::ExponentialOperatorDeterminant
    } else if has_any(&text, &["expm", "matrix exponential"]) {
        MethodSignature::MatrixExponentialProjection
    } else if text.contains("mdiag") {
        MethodSignature::MatrixManifoldConstraint
    } else if has_any(&text, &["riemannian matrix manifold", "matrix manifold"]) {
        MethodSignature::RiemannianMetricStructure
    } else if has_any(&text, &["poincaré disk", "poincare disk", "omega points"]) {
        MethodSignature::PoincareDiskGeometry
    } else if text.contains("lorentz model") {
        MethodSignature::LorentzSampling
    } else if has_any(&text, &["function sampling", "gamma(", "qr-decomposition"]) {
        MethodSignature::FunctionSampling
    } else if has_any(&text, &["lie algebra", "commutator", "skew-symmetric"]) {
        MethodSignature::LieAlgebraMatrix
    } else if has_any(&text, &["log(m)", "matrix logarithm", "matrix exponential"]) {
        MethodSignature::MatrixLogSampling
    } else if has_any(
        &text,
        &["graph of groups", "fundamental group", "mapping torus"],
    ) {
        MethodSignature::GroupTheoryStructure
    } else if has_any(
        &text,
        &["eigenvalue", "eigenvalues", "spectrum", "spectral"],
    ) {
        MethodSignature::MatrixSpectralInvariant
    } else if has_any(
        &text,
        &["cholesky", "parlett-reid", "popov normal", "decomposition"],
    ) {
        MethodSignature::MatrixDecomposition
    } else if has_any(&text, &["matrix", "determinant", "rank", "trace"]) {
        MethodSignature::MatrixRankOrDeterminant
    } else if has_any(
        &text,
        &[
            "prime density",
            "irreducible mod",
            "density of the set of prime",
        ],
    ) {
        MethodSignature::NumberTheoryPrimeDensity
    } else if has_any(&text, &["modulo", " mod ", "congruent", "divisib"]) {
        MethodSignature::NumberTheoryCongruence
    } else if has_any(&text, &["expected", "expectation", "expected value"]) {
        MethodSignature::ProbabilityExpectation
    } else if has_any(
        &text,
        &["probability", "distribution", "random variable", "randomly"],
    ) {
        MethodSignature::ProbabilityDistribution
    } else if has_any(
        &text,
        &["differential equation", "initial value problem", "pde"],
    ) {
        MethodSignature::DifferentialEquationSolve
    } else if has_any(&text, &["derivative", "integral", "integrate"]) {
        MethodSignature::CalculusTransformation
    } else if has_any(
        &text,
        &["graph", "treewidth", "chromatic", "vertex", "edge"],
    ) {
        MethodSignature::GraphInvariant
    } else if has_any(&text, &["triangle", "circumcircle", "angle", "geometry"]) {
        MethodSignature::GeometricConstruction
    } else if has_any(
        &text,
        &[
            "complexity",
            "runtime",
            "algorithm",
            "o(n",
            "polynomial time",
        ],
    ) {
        MethodSignature::AsymptoticRuntime
    } else if has_any(
        &text,
        &[
            "inequality",
            "lower and upper bounds",
            "supremum",
            "infimum",
        ],
    ) {
        MethodSignature::InequalityBound
    } else if has_any(
        &text,
        &["theorem", "prove", "lemma", "corollary", "show that"],
    ) {
        MethodSignature::TheoremApplication
    } else if has_any(
        &text,
        &["law of", "scientific law", "formula", "model", "equation"],
    ) {
        MethodSignature::ScientificLawApplication
    } else if has_any(
        &text,
        &[
            "number of ways",
            "cardinality",
            "partition",
            "permutation",
            "count the",
        ],
    ) {
        MethodSignature::CombinatorialEnumeration
    } else {
        MethodSignature::GenericSpecialist
    }
}

fn output_artifact(question: &str) -> String {
    let text = question.to_ascii_lowercase();
    if has_any(
        &text,
        &["cardinality", "how many", "number of ways", "count"],
    ) {
        "cardinality_answer".into()
    } else if has_any(
        &text,
        &["yes or no", "true or false", "which of the following"],
    ) {
        "choice_or_boolean_answer".into()
    } else if has_any(&text, &["formula", "expression", "derive"]) {
        "expression_answer".into()
    } else {
        "scalar_or_structured_answer".into()
    }
}

fn cues(question: &str) -> (Vec<String>, Vec<String>) {
    let text = question.to_ascii_lowercase();
    let prerequisites = [
        ("theorem", "named theorem or theorem assumptions"),
        ("law", "scientific law and validity domain"),
        ("definition", "domain definition"),
        ("standard", "specialist convention"),
        ("known", "external factual premise"),
    ]
    .iter()
    .filter(|(term, _)| text.contains(term))
    .map(|(_, label)| (*label).to_string())
    .collect();
    let representation = [
        ("\\sum", "summation notation"),
        ("\\frac", "fraction notation"),
        ("matrix", "matrix representation"),
        ("\\operator", "named operator"),
        ("image", "visual or diagram input"),
    ]
    .iter()
    .filter(|(term, _)| text.contains(term))
    .map(|(_, label)| (*label).to_string())
    .collect();
    (prerequisites, representation)
}

fn classify(
    signature: MethodSignature,
    output: &str,
    question: &str,
    route_len: usize,
    required_capabilities: usize,
) -> AuditClass {
    let (prerequisites, representation) = cues(question);
    if !representation.is_empty() && signature == MethodSignature::GenericSpecialist {
        AuditClass::RepresentationBridge
    } else if !prerequisites.is_empty() && signature == MethodSignature::ScientificLawApplication {
        AuditClass::KnowledgeDependent
    } else if route_len > 3 && required_capabilities >= 2 {
        AuditClass::CompositionGap
    } else if signature == MethodSignature::GenericSpecialist {
        AuditClass::SpecialistSingleton
    } else if output.is_empty() {
        AuditClass::AmbiguousOrContaminated
    } else {
        AuditClass::TransformationCandidate
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let trace_path = env::args()
        .nth(1)
        .unwrap_or_else(|| "/tmp/hle_phase26_combined.traces.jsonl".into());
    let output = env::args()
        .nth(2)
        .unwrap_or_else(|| "/tmp/hle_reasoning_method_audit_2147e9e.json".into());
    let trace_bytes = fs::read(&trace_path)?;
    let dataset_bytes = fs::read(DATASET)?;
    let mut cases = Vec::new();
    for line in String::from_utf8(trace_bytes.clone())?.lines() {
        let row: TraceRow = serde_json::from_str(line)?;
        if row.terminal_classification != "missing_reasoning_method" {
            continue;
        }
        let method_signature = signature(&row.question);
        let output_artifact = output_artifact(&row.question);
        let (prerequisites, representation) = cues(&row.question);
        let audit_class = classify(
            method_signature,
            &output_artifact,
            &row.question,
            row.route_trace.len(),
            row.required_capabilities.len(),
        );
        cases.push(MethodCase {
            id: row.id,
            signature: method_signature,
            output_artifact,
            audit_class,
            prerequisite_cues: prerequisites,
            representation_cues: representation,
            route_trace_len: row.route_trace.len(),
        });
    }
    let mut grouped: BTreeMap<MethodSignature, Vec<&MethodCase>> = BTreeMap::new();
    for case in &cases {
        grouped.entry(case.signature).or_default().push(case);
    }
    let mut families = Vec::new();
    for (method_signature, members) in grouped {
        let mut outputs = BTreeMap::new();
        for member in &members {
            *outputs.entry(member.output_artifact.clone()).or_insert(0) += 1;
        }
        let coherent = members.len() >= 2 && outputs.len() == 1;
        families.push(FamilySummary {
            signature: method_signature,
            cases: members.len(),
            output_artifacts: outputs,
            case_ids: members.iter().map(|m| m.id.clone()).collect(),
            coherent,
            contract_readiness: if coherent {
                "candidate; requires independent boundary corpus".into()
            } else if members.len() >= 2 {
                "defer; output artifacts diverge".into()
            } else {
                "singleton; insufficient evidence".into()
            },
        });
    }
    families.sort_by(|a, b| {
        b.cases
            .cmp(&a.cases)
            .then_with(|| a.signature.cmp(&b.signature))
    });
    let mut class_counts = BTreeMap::new();
    let mut signature_counts = BTreeMap::new();
    for case in &cases {
        *class_counts.entry(case.audit_class).or_insert(0) += 1;
        *signature_counts.entry(case.signature).or_insert(0) += 1;
    }
    let report = Report {
        trace_sha256: hash(&trace_bytes),
        dataset_sha256: hash(&dataset_bytes),
        missing_method_cases: cases.len(),
        class_counts,
        signature_counts,
        coherent_families: families.iter().filter(|f| f.coherent).count(),
        cases,
        families,
        method: "shadow-only exact transformation coherence audit; no capability or bridge authorization".into(),
    };
    fs::write(&output, serde_json::to_vec_pretty(&report)?)?;
    println!("{}", serde_json::to_string_pretty(&report)?);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recurrence_signatures_split_evaluation_and_closed_form() {
        assert_eq!(
            signature("Given a recurrence, find a_n."),
            MethodSignature::RecurrenceEvaluation
        );
        assert_eq!(
            signature("Derive a closed form for the recurrence."),
            MethodSignature::RecurrenceClosedForm
        );
    }

    #[test]
    fn output_divergence_blocks_family_coherence() {
        assert!(!matches!(
            classify(
                MethodSignature::GenericSpecialist,
                "scalar_or_structured_answer",
                "A specialist question.",
                1,
                0
            ),
            AuditClass::TransformationCandidate
        ));
    }

    #[test]
    fn decomposition_keyword_does_not_hide_distinct_matrix_methods() {
        assert_ne!(
            signature("Riemannian matrix manifold exponential map"),
            signature("QR-decomposition sampling procedure")
        );
        assert_ne!(
            signature("Graph of groups fundamental group"),
            signature("Cholesky decomposition sampling")
        );
    }
}
