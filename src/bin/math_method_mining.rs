//! Run the diagnostic method-shape miner over the complete checked-in HLE
//! corpus.  This binary is intentionally shadow-only: it produces a report
//! and never loads a MathematicalMethodRegistry or answers a question.

use serde::Deserialize;
use std::fs;
use std::path::Path;
use the_machine::development::{assess_math_funnel, MathTaskKind};
use the_machine::math_method_mining::{
    ClusterCoherence, MethodClusterAnnotation, MethodClusterReport, MethodShape,
    RepresentationCost, VerificationAvailability,
};
use the_machine::math_methods::{MathDomain, TaskShape};

#[derive(Debug, Deserialize)]
struct HleRow {
    id: String,
    question: String,
    category: String,
}

fn domain(task: MathTaskKind) -> MathDomain {
    match task {
        MathTaskKind::ExplicitExpressionEvaluation
        | MathTaskKind::EquationSolving
        | MathTaskKind::PolynomialAlgebra => MathDomain::Algebra,
        MathTaskKind::FiniteCombinatorics => MathDomain::Combinatorics,
        MathTaskKind::ElementaryCalculus => MathDomain::Calculus,
        MathTaskKind::NumberTheory => MathDomain::NumberTheory,
        MathTaskKind::LinearAlgebra => MathDomain::LinearAlgebra,
        MathTaskKind::Geometry => MathDomain::Geometry,
        MathTaskKind::ProofOrTheorem | MathTaskKind::AdvancedSpecialized => MathDomain::General,
        MathTaskKind::DiagramDependent
        | MathTaskKind::UnparsedMathematicalProse
        | MathTaskKind::NotMath => MathDomain::General,
    }
}

fn task_shape(task: MathTaskKind, lower: &str) -> TaskShape {
    match task {
        MathTaskKind::ProofOrTheorem => {
            if lower.contains("prove") || lower.contains("show that") {
                TaskShape::ProveIdentity
            } else {
                TaskShape::ClassifyStructure
            }
        }
        MathTaskKind::FiniteCombinatorics => TaskShape::CountObjects,
        MathTaskKind::NumberTheory => {
            if lower.contains("exist") || lower.contains("solution") {
                TaskShape::FindAllObjects
            } else {
                TaskShape::ComputeExplicitValue
            }
        }
        MathTaskKind::AdvancedSpecialized => TaskShape::ClassifyStructure,
        _ if lower.contains("bound") || lower.contains("at most") || lower.contains("at least") => {
            TaskShape::BoundQuantity
        }
        _ if lower.contains("prove") || lower.contains("show that") => TaskShape::ProveIdentity,
        _ => TaskShape::ComputeExplicitValue,
    }
}

fn image_dependent(question: &str) -> bool {
    let lower = question.to_ascii_lowercase();
    [
        "diagram",
        "figure",
        "table",
        "graph shown",
        "pictured",
        "shown below",
        "image",
        "plot",
        "see the",
    ]
    .iter()
    .any(|token| lower.contains(token))
}

fn proof_target(question: &str) -> bool {
    let lower = question.to_ascii_lowercase();
    [
        "prove",
        "proof",
        "show that",
        "establish that",
        "demonstrate",
    ]
    .iter()
    .any(|token| lower.contains(token))
}

fn method_shape(question: &str, task: MathTaskKind) -> (MethodShape, String, ClusterCoherence) {
    let lower = question.to_ascii_lowercase();
    let named = [
        ("vieta", "vieta"),
        ("chinese remainder", "chinese_remainder"),
        ("pigeonhole", "pigeonhole"),
        ("bayes", "bayes"),
        ("rank-nullity", "rank_nullity"),
        ("cauchy-schwarz", "cauchy_schwarz"),
        ("stokes", "stokes"),
        ("lagrange", "lagrange_multipliers"),
        ("comparison test", "comparison_test"),
    ];
    if let Some((_, name)) = named.iter().find(|(token, _)| lower.contains(token)) {
        return (
            MethodShape::DirectTheoremInstantiation,
            (*name).to_string(),
            ClusterCoherence::ExactMethod,
        );
    }
    if lower.contains("recurrence")
        || lower.contains("recursive")
        || lower.contains("sequence")
        || lower.contains("a_n")
    {
        return (
            MethodShape::RecurrenceUnrolling,
            "recurrence_instantiation".to_string(),
            ClusterCoherence::ParameterizedMethodFamily,
        );
    }
    if lower.contains("defined as")
        || lower.contains("definition of")
        || lower.contains("let ")
        || lower.contains("denote")
        || lower.contains("suppose")
    {
        return (
            MethodShape::DefinitionInstantiation,
            "prompt_supplied_definition".to_string(),
            ClusterCoherence::ParameterizedMethodFamily,
        );
    }
    if lower.contains("invariant") || lower.contains("is unchanged") {
        return (
            MethodShape::InvariantApplication,
            "invariant_application".to_string(),
            ClusterCoherence::SharedShapeOnly,
        );
    }
    if lower.contains("identity") || lower.contains("simplify") {
        return (
            MethodShape::AlgebraicIdentityApplication,
            "identity_application".to_string(),
            ClusterCoherence::SharedShapeOnly,
        );
    }
    if lower.contains("bound") || lower.contains("at most") || lower.contains("at least") {
        return (
            MethodShape::BoundApplication,
            "bound_application".to_string(),
            ClusterCoherence::SharedShapeOnly,
        );
    }
    if lower.contains("case") || lower.contains("classify") || lower.contains("how many") {
        return (
            MethodShape::FiniteCaseReduction,
            "finite_case_reduction".to_string(),
            ClusterCoherence::SharedShapeOnly,
        );
    }
    if matches!(
        task,
        MathTaskKind::ProofOrTheorem | MathTaskKind::AdvancedSpecialized
    ) {
        return (
            MethodShape::ClassificationLookup,
            "specialized_method_unknown".to_string(),
            ClusterCoherence::SuperficialVocabulary,
        );
    }
    (
        MethodShape::TransformAndEvaluate,
        "generic_transform".to_string(),
        ClusterCoherence::SuperficialVocabulary,
    )
}

fn annotate(row: HleRow) -> Option<MethodClusterAnnotation> {
    let assessment = assess_math_funnel(&row.question, &row.category);
    if !assessment.math_signal {
        return None;
    }
    let lower = row.question.to_ascii_lowercase();
    let (shape, variant, coherence) = method_shape(&row.question, assessment.task_kind);
    let image_dependency = image_dependent(&row.question);
    let proof = proof_target(&row.question);
    let premises = !assessment.structured_statements.is_empty()
        || ["given", "let ", "assume", "suppose", "where ", "satisfying"]
            .iter()
            .any(|token| lower.contains(token));
    let definitions = [
        "defined as",
        "definition",
        "let ",
        "denote",
        "recurrence",
        "recursive",
        "suppose",
    ]
    .iter()
    .any(|token| lower.contains(token));
    let useful_conclusion = assessment.target.is_some()
        && !matches!(
            assessment.task_kind,
            MathTaskKind::UnparsedMathematicalProse
        );
    let likely_one_step = matches!(
        shape,
        MethodShape::DefinitionInstantiation
            | MethodShape::DirectTheoremInstantiation
            | MethodShape::RecurrenceUnrolling
    ) && premises
        && useful_conclusion
        && !image_dependency
        && !proof;
    let verifier = if assessment.executor_candidate && !image_dependency && !proof {
        VerificationAvailability::Replay
    } else {
        VerificationAvailability::None
    };
    let structurally_compatible = (assessment.executor_candidate || likely_one_step)
        && premises
        && useful_conclusion
        && !image_dependency
        && !proof;
    Some(MethodClusterAnnotation {
        question_id: row.id,
        domain: domain(assessment.task_kind),
        task_shape: task_shape(assessment.task_kind, &lower),
        required_method_shape: shape,
        named_methods: if coherence == ClusterCoherence::ExactMethod {
            vec![variant.clone()]
        } else {
            Vec::new()
        },
        premises_explicit: premises,
        definitions_explicit: definitions,
        side_conditions_extractable: !proof
            && !matches!(assessment.task_kind, MathTaskKind::AdvancedSpecialized),
        verifier_available: verifier,
        estimated_steps: if assessment.executor_candidate || likely_one_step {
            1
        } else {
            2
        },
        representation_cost: if assessment.executor_candidate || likely_one_step {
            RepresentationCost::Medium
        } else {
            RepresentationCost::High
        },
        coherence,
        method_variant: variant,
        // `has_image` is true for every HLE row in this export and means an
        // attachment slot exists, not that the question depends on it.  Use
        // textual diagram/figure dependency for this gate until attachment
        // paths are preserved by the benchmark loader.
        image_independent: !image_dependency,
        non_proof_target: !proof,
        useful_conclusion,
        reviewed: false,
        structurally_compatible,
    })
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let input = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "data/hle.jsonl".to_string());
    let json = fs::read_to_string(&input)?;
    let mut scanned = 0usize;
    let mut annotations = Vec::new();
    for line in json.lines().filter(|line| !line.trim().is_empty()) {
        let row: HleRow = serde_json::from_str(line)?;
        scanned += 1;
        if let Some(annotation) = annotate(row) {
            annotations.push(annotation);
        }
    }
    let report = MethodClusterReport::from_annotations(&annotations);
    let mut markdown = format!(
        "# Complete HLE method-shape reconnaissance\n\nInput: `{}`\nScanned rows: {}\nMath annotations: {}\n\nThis is a heuristic, shadow-only scan. All generated rows have `reviewed = false`; no row can authorize a method pack. A cluster is pack-eligible only after manual review, coherence validation, one-step typed premises, an independent verifier, and image/proof gates.\n\n",
        input, scanned, report.annotations
    );
    markdown.push_str(&report.to_markdown());
    let json_path = Path::new("docs/math_method_cluster_mining_20260720.json");
    let md_path = Path::new("docs/math_method_cluster_mining_20260720.md");
    fs::write(json_path, serde_json::to_string_pretty(&report)?)?;
    fs::write(md_path, markdown)?;
    println!(
        "scanned={} math_annotations={} clusters={} eligible_pack_candidates={}",
        scanned,
        report.annotations,
        report.clusters.len(),
        report
            .pack_candidates(3, VerificationAvailability::Replay)
            .len()
    );
    Ok(())
}
