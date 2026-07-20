//! Report-only formalization diagnostics.
//!
//! This module measures how far a prompt is from an executable typed problem.
//! It deliberately does not authorize a solver or infer missing mathematics.
//! A future parser can populate the same structures with stronger evidence;
//! the current assessor is conservative and intended for corpus analysis.

use crate::math_methods::{MathDomain, TaskShape};
use regex::Regex;
use serde::Serialize;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum DefinitionKind {
    Function,
    Sequence,
    Set,
    Operation,
    Relation,
    Graph,
    ProbabilityModel,
    PhysicalQuantity,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ModelingObligation {
    DefineObject,
    IdentifyStateVariables,
    IdentifyDomain,
    ExtractQuantifiers,
    DetermineTargetSemantics,
    ConstructEquation,
    EstablishBoundaryConditions,
    EstablishInitialConditions,
    SelectApproximationRegime,
    ResolveEntityReference,
    SelectSpecializedMethod,
    ParseAttachment,
}

impl ModelingObligation {
    pub fn label(self) -> &'static str {
        match self {
            Self::DefineObject => "define_object",
            Self::IdentifyStateVariables => "identify_state_variables",
            Self::IdentifyDomain => "identify_domain",
            Self::ExtractQuantifiers => "extract_quantifiers",
            Self::DetermineTargetSemantics => "determine_target_semantics",
            Self::ConstructEquation => "construct_equation",
            Self::EstablishBoundaryConditions => "establish_boundary_conditions",
            Self::EstablishInitialConditions => "establish_initial_conditions",
            Self::SelectApproximationRegime => "select_approximation_regime",
            Self::ResolveEntityReference => "resolve_entity_reference",
            Self::SelectSpecializedMethod => "select_specialized_method",
            Self::ParseAttachment => "parse_attachment",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ModelingDistance {
    ExecutableObject = 0,
    DirectInstantiation = 1,
    OneModelingStep = 2,
    MethodSelection = 3,
    SpecialistReasoning = 4,
}

impl ModelingDistance {
    pub fn label(self) -> &'static str {
        match self {
            Self::ExecutableObject => "executable_object",
            Self::DirectInstantiation => "direct_instantiation",
            Self::OneModelingStep => "one_modeling_step",
            Self::MethodSelection => "method_selection",
            Self::SpecialistReasoning => "specialist_reasoning",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum FormalizationStatus {
    Structured,
    PartiallyStructured,
    Unresolved,
    AttachmentRequired,
}

impl FormalizationStatus {
    pub fn label(self) -> &'static str {
        match self {
            Self::Structured => "structured",
            Self::PartiallyStructured => "partially_structured",
            Self::Unresolved => "unresolved",
            Self::AttachmentRequired => "attachment_required",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct EntityAnnotation {
    pub label: String,
    pub source_fragment: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct FactAnnotation {
    pub statement: String,
    pub source_fragment: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct TargetAnnotation {
    pub statement: String,
    pub source_fragment: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct AssumptionAnnotation {
    pub statement: String,
    pub source_fragment: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ConstraintAnnotation {
    pub statement: String,
    pub source_fragment: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct FormalizationCase {
    pub prompt: String,
    pub expected_domain: MathDomain,
    pub expected_task_shape: TaskShape,
    pub expected_entities: Vec<EntityAnnotation>,
    pub expected_facts: Vec<FactAnnotation>,
    pub expected_target: Option<TargetAnnotation>,
    pub expected_assumptions: Vec<AssumptionAnnotation>,
    pub expected_constraints: Vec<ConstraintAnnotation>,
    pub allowed_methods: Vec<String>,
    pub expected_answer: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct FormalizationTrace {
    pub question_id: String,
    pub category: String,
    pub has_image: bool,
    /// True only when the question text itself refers to visual material.  A
    /// benchmark `has_image` flag alone does not establish that the prompt's
    /// reasoning depends on the attachment.
    pub textual_attachment_reference: bool,
    pub domain: MathDomain,
    pub task_shape: TaskShape,
    pub status: FormalizationStatus,
    pub definitions: Vec<DefinitionKind>,
    pub entities: Vec<EntityAnnotation>,
    pub facts: Vec<FactAnnotation>,
    pub target: Option<TargetAnnotation>,
    pub assumptions: Vec<AssumptionAnnotation>,
    pub constraints: Vec<ConstraintAnnotation>,
    pub obligations: Vec<ModelingObligation>,
    pub modeling_distance: ModelingDistance,
}

fn has_any(text: &str, terms: &[&str]) -> bool {
    terms.iter().any(|term| text.contains(term))
}

fn classify_domain(question: &str, category: &str) -> MathDomain {
    let text = format!(
        "{} {}",
        question.to_ascii_lowercase(),
        category.to_ascii_lowercase()
    );
    if has_any(
        &text,
        &["derivative", "integral", "limit", "converges", "series"],
    ) {
        MathDomain::Calculus
    } else if has_any(
        &text,
        &["mod ", "divisible", "prime", "gcd", "congruen", "integer"],
    ) {
        MathDomain::NumberTheory
    } else if has_any(
        &text,
        &[
            "probability",
            "binomial",
            "choose",
            "permutation",
            "coin",
            "card",
        ],
    ) {
        MathDomain::Probability
    } else if has_any(
        &text,
        &["matrix", "eigenvalue", "vector", "rank", "linear algebra"],
    ) {
        MathDomain::LinearAlgebra
    } else if has_any(&text, &["triangle", "circle", "angle", "area", "geometry"]) {
        MathDomain::Geometry
    } else {
        MathDomain::General
    }
}

fn classify_task(question: &str) -> TaskShape {
    let text = question.to_ascii_lowercase();
    if has_any(&text, &["prove", "show that", "establish", "demonstrate"]) {
        TaskShape::ProveIdentity
    } else if has_any(&text, &["how many", "count", "number of ways"]) {
        TaskShape::CountObjects
    } else if has_any(&text, &["exists", "does there exist", "is it possible"]) {
        TaskShape::DetermineExistence
    } else if has_any(&text, &["unique", "only solution", "exactly one"]) {
        TaskShape::DetermineUniqueness
    } else if has_any(
        &text,
        &["bound", "at most", "at least", "upper bound", "lower bound"],
    ) {
        TaskShape::BoundQuantity
    } else if has_any(&text, &["find all", "all values", "classify"]) {
        TaskShape::FindAllObjects
    } else {
        TaskShape::ComputeExplicitValue
    }
}

/// Conservative, non-executing assessment used by the formalization report.
/// It records missing modeling work instead of guessing a formal object.
pub fn assess_prompt(
    question_id: &str,
    question: &str,
    category: &str,
    has_image: bool,
) -> FormalizationTrace {
    let lower = question.to_ascii_lowercase();
    let domain = classify_domain(question, category);
    let task_shape = classify_task(question);
    let mut obligations = Vec::new();
    let mut definitions = Vec::new();
    let mut entities = Vec::new();
    let mut facts = Vec::new();
    let mut assumptions = Vec::new();
    let mut constraints = Vec::new();

    let textual_attachment_reference = has_any(
        &lower,
        &[
            "diagram",
            "figure",
            "pictured",
            "shown below",
            "table",
            "graph shown",
            "plot",
        ],
    );
    if has_image || textual_attachment_reference {
        obligations.push(ModelingObligation::ParseAttachment);
    }
    if has_any(
        &lower,
        &[
            "define ",
            "defined ",
            "let ",
            "where ",
            "is defined as",
            "denote ",
            "recurrence",
            "recursive",
        ],
    ) {
        definitions.push(
            if has_any(&lower, &["sequence", "recursive", "recurrence", "a_n"]) {
                DefinitionKind::Sequence
            } else if has_any(&lower, &["probability", "random variable", "sample space"]) {
                DefinitionKind::ProbabilityModel
            } else if has_any(&lower, &["set of", "subset", "elements"]) {
                DefinitionKind::Set
            } else if has_any(&lower, &["function", "f(", "g("]) {
                DefinitionKind::Function
            } else {
                DefinitionKind::Relation
            },
        );
    }
    if has_any(
        &lower,
        &[
            "for all",
            "for every",
            "there exists",
            "for some",
            "such that",
            "∀",
            "∃",
        ],
    ) {
        facts.push(FactAnnotation {
            statement: "quantified statement present".into(),
            source_fragment: question.into(),
        });
    } else if task_shape == TaskShape::ProveIdentity
        || has_any(&lower, &["theorem", "lemma", "criterion"])
    {
        obligations.push(ModelingObligation::ExtractQuantifiers);
    }
    if lower.contains('=') || has_any(&lower, &["equation", "given that", "satisfies", "where"]) {
        facts.push(FactAnnotation {
            statement: "explicit relation or equation signal".into(),
            source_fragment: question.into(),
        });
    } else if matches!(
        domain,
        MathDomain::Algebra
            | MathDomain::Calculus
            | MathDomain::NumberTheory
            | MathDomain::Probability
    ) {
        obligations.push(ModelingObligation::ConstructEquation);
    }
    if definitions.contains(&DefinitionKind::Sequence) && facts.is_empty() {
        obligations.push(ModelingObligation::ConstructEquation);
    }
    let target = if has_any(
        &lower,
        &[
            "find",
            "compute",
            "calculate",
            "determine",
            "evaluate",
            "solve",
            "what is",
            "which",
        ],
    ) {
        let statement = question
            .split(['?', '\n'])
            .rev()
            .find(|part| {
                has_any(
                    &part.to_ascii_lowercase(),
                    &[
                        "find",
                        "compute",
                        "calculate",
                        "determine",
                        "evaluate",
                        "solve",
                        "what is",
                        "which",
                    ],
                )
            })
            .unwrap_or(question)
            .trim()
            .to_string();
        Some(TargetAnnotation {
            statement,
            source_fragment: question.into(),
        })
    } else {
        obligations.push(ModelingObligation::DetermineTargetSemantics);
        None
    };
    if !has_any(
        &lower,
        &[
            "real", "integer", "natural", "positive", "nonzero", "non-zero", "complex", "mod ",
        ],
    ) {
        if matches!(
            domain,
            MathDomain::Algebra | MathDomain::NumberTheory | MathDomain::Calculus
        ) {
            obligations.push(ModelingObligation::IdentifyDomain);
        }
    } else {
        constraints.push(ConstraintAnnotation {
            statement: "explicit domain/side-condition signal".into(),
            source_fragment: question.into(),
        });
    }
    if definitions.is_empty()
        && facts.is_empty()
        && matches!(domain, MathDomain::General | MathDomain::Probability)
    {
        obligations.push(ModelingObligation::DefineObject);
    }
    if task_shape == TaskShape::ProveIdentity
        || has_any(&lower, &["theorem", "lemma", "using", "apply"])
    {
        obligations.push(ModelingObligation::SelectSpecializedMethod);
    }
    if lower.contains("initial") || lower.contains("boundary") {
        obligations.push(if lower.contains("initial") {
            ModelingObligation::EstablishInitialConditions
        } else {
            ModelingObligation::EstablishBoundaryConditions
        });
    }
    if lower.contains("object") || lower.contains("particle") || lower.contains("person") {
        entities.push(EntityAnnotation {
            label: "explicit entity mention".into(),
            source_fragment: question.into(),
        });
    }
    if lower.contains("approx") || lower.contains("small parameter") || lower.contains("asymptotic")
    {
        obligations.push(ModelingObligation::SelectApproximationRegime);
    }
    if lower.contains("after") || lower.contains("before") || lower.contains("respectively") {
        obligations.push(ModelingObligation::ResolveEntityReference);
    }
    obligations.sort_by_key(|obligation| obligation.label());
    obligations.dedup();

    let explicit_object = !facts.is_empty() || !definitions.is_empty();
    let distance = if textual_attachment_reference
        || task_shape == TaskShape::ProveIdentity
            && obligations.contains(&ModelingObligation::SelectSpecializedMethod)
    {
        ModelingDistance::SpecialistReasoning
    } else if obligations.contains(&ModelingObligation::SelectSpecializedMethod) {
        ModelingDistance::MethodSelection
    } else if obligations.iter().any(|obligation| {
        matches!(
            obligation,
            ModelingObligation::ConstructEquation
                | ModelingObligation::DefineObject
                | ModelingObligation::ExtractQuantifiers
                | ModelingObligation::ResolveEntityReference
        )
    }) {
        ModelingDistance::OneModelingStep
    } else if target.is_some() && explicit_object {
        ModelingDistance::DirectInstantiation
    } else {
        ModelingDistance::ExecutableObject
    };
    let status = if has_image {
        FormalizationStatus::AttachmentRequired
    } else if target.is_none() || !obligations.is_empty() {
        FormalizationStatus::PartiallyStructured
    } else {
        FormalizationStatus::Structured
    };
    FormalizationTrace {
        question_id: question_id.into(),
        category: category.into(),
        has_image,
        textual_attachment_reference,
        domain,
        task_shape,
        status,
        definitions,
        entities,
        facts,
        target,
        assumptions,
        constraints,
        obligations,
        modeling_distance: distance,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn explicit_equation_is_direct_instantiation_diagnostic() {
        let trace = assess_prompt("q", "Solve for x: 3x + 2 = 11, for real x.", "Math", false);
        assert_eq!(
            trace.modeling_distance,
            ModelingDistance::DirectInstantiation
        );
        assert!(trace.target.is_some());
        assert!(trace.obligations.is_empty());
    }

    #[test]
    fn recurrence_surface_does_not_authorize_recurrence_execution() {
        let trace = assess_prompt(
            "q",
            "The sequence is defined recursively. Find its closed form.",
            "Math",
            false,
        );
        assert!(trace
            .obligations
            .contains(&ModelingObligation::ConstructEquation));
        assert!(trace.modeling_distance >= ModelingDistance::OneModelingStep);
    }

    #[test]
    fn image_questions_remain_attachment_required() {
        let trace = assess_prompt("q", "Find the angle shown in the figure.", "Math", true);
        assert_eq!(trace.status, FormalizationStatus::AttachmentRequired);
        assert!(trace
            .obligations
            .contains(&ModelingObligation::ParseAttachment));
        assert_eq!(
            trace.modeling_distance,
            ModelingDistance::SpecialistReasoning
        );
        assert!(trace.textual_attachment_reference);
    }
}
