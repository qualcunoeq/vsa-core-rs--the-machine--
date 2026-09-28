//! Typed capability adapter: reach the library's bounded, replay-verified
//! execution capabilities through the chat interface.
//!
//! Phase 5 wires a small set of mature capabilities — expression evaluation,
//! single linear and quadratic equations, small linear systems, and an
//! explicit unit conversion — into the same interface users see. Each one is
//! reached through the same six steps:
//!
//! 1. interpret the question,
//! 2. construct typed inputs,
//! 3. identify missing information or ambiguity,
//! 4. execute,
//! 5. verify with the capability's own replay checker,
//! 6. render the result with assumptions and evidence.
//!
//! The adapter is deliberately conservative about *which* capability a prompt
//! belongs to. It never guesses a binding: when a capability is recognized but
//! its required information is absent, the turn asks for it; when the wording
//! is not interpretable, the caller falls back to the library router.

use crate::capabilities::{CapabilityRegistry, CapabilitySelection};
use crate::formalization::SubjectObjectType;
use crate::linear_system::LinearSystemClassification;

use super::types::{Capability, EvidenceKind, EvidenceRef, TurnOutcome, VerificationStatus};

/// How the adapter's own attempt ended.
///
/// These are the four distinctions the phase asks for, kept separate so an
/// interface (and a developer) can tell an unsupported request from a
/// missing-information one from a verification failure:
#[derive(Clone, Debug, PartialEq)]
pub enum CapabilityDisposition {
    /// The capability ran and its replay checker passed.
    Verified,
    /// We understood the requested operation, but do not support it here.
    OperationUnsupported { reason: String },
    /// We understood the operation; required information is missing.
    MissingInformation { question: String },
    /// The operation ran but its result failed its own verifier.
    VerificationFailed { reason: String },
    /// The wording could not be interpreted as any supported capability.
    NotInterpreted,
}

/// A fully rendered capability attempt, ready to become an `AdapterOutput`.
#[derive(Clone, Debug, PartialEq)]
pub struct CapabilityAttempt {
    pub disposition: CapabilityDisposition,
    pub capability: Capability,
    pub answer: Option<String>,
    pub evidence: Vec<EvidenceRef>,
    pub verification: VerificationStatus,
    pub notes: Vec<String>,
}

impl CapabilityAttempt {
    fn not_interpreted() -> Self {
        Self {
            disposition: CapabilityDisposition::NotInterpreted,
            capability: Capability::None,
            answer: None,
            evidence: Vec::new(),
            verification: VerificationStatus::NotAttempted,
            notes: Vec::new(),
        }
    }

    fn unsupported(id: &str, reason: impl Into<String>) -> Self {
        let reason = reason.into();
        Self {
            disposition: CapabilityDisposition::OperationUnsupported {
                reason: reason.clone(),
            },
            capability: Capability::StructuredSolver {
                domain: id.to_string(),
            },
            answer: Some(reason.clone()),
            evidence: Vec::new(),
            verification: VerificationStatus::NotAttempted,
            notes: vec![format!("capability {id} recognized but unsupported: {reason}")],
        }
    }

    fn missing(id: &str, question: impl Into<String>) -> Self {
        let question = question.into();
        Self {
            disposition: CapabilityDisposition::MissingInformation {
                question: question.clone(),
            },
            capability: Capability::StructuredSolver {
                domain: id.to_string(),
            },
            answer: Some(question.clone()),
            evidence: Vec::new(),
            verification: VerificationStatus::NotAttempted,
            notes: vec![format!("capability {id} recognized but requires more information")],
        }
    }

    fn verification_failed(id: &str, reason: impl Into<String>) -> Self {
        let reason = reason.into();
        Self {
            disposition: CapabilityDisposition::VerificationFailed {
                reason: reason.clone(),
            },
            capability: Capability::StructuredSolver {
                domain: id.to_string(),
            },
            answer: Some(format!("I could not verify the result for that request: {reason}")),
            evidence: Vec::new(),
            verification: VerificationStatus::Unverified {
                reason: reason.clone(),
            },
            notes: vec![format!("capability {id} produced a result that failed verification")],
        }
    }

    /// Convert to an outcome, preserving the four-way distinction above.
    pub fn outcome(&self) -> TurnOutcome {
        match &self.disposition {
            CapabilityDisposition::Verified => TurnOutcome::Answered,
            CapabilityDisposition::OperationUnsupported { reason } => TurnOutcome::Unsupported {
                reason: reason.clone(),
            },
            CapabilityDisposition::MissingInformation { question } => {
                TurnOutcome::ClarificationNeeded {
                    question: question.clone(),
                }
            }
            CapabilityDisposition::VerificationFailed { reason } => TurnOutcome::Failed {
                error: reason.clone(),
            },
            CapabilityDisposition::NotInterpreted => TurnOutcome::Unsupported {
                reason: "the wording was not recognized as a supported capability".to_string(),
            },
        }
    }

    pub fn is_verified(&self) -> bool {
        matches!(self.disposition, CapabilityDisposition::Verified)
    }
}

/// One capability the adapter knows how to reach.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SupportedCapability {
    ExpressionEvaluation,
    LinearEquation,
    QuadraticEquation,
    LinearSystem,
    UnitConversion,
}

impl SupportedCapability {
    pub fn id(self) -> &'static str {
        match self {
            Self::ExpressionEvaluation => "expression_evaluation",
            Self::LinearEquation => "linear_equation_solve",
            Self::QuadraticEquation => "quadratic_equation_solve",
            Self::LinearSystem => "linear_system_solve",
            Self::UnitConversion => "unit_conversion",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::ExpressionEvaluation => "expression evaluation",
            Self::LinearEquation => "linear equation solving",
            Self::QuadraticEquation => "quadratic equation solving",
            Self::LinearSystem => "small linear systems",
            Self::UnitConversion => "explicit unit conversion",
        }
    }
}

/// Interpret a question and attempt the matching typed capability.
///
/// Returns [`CapabilityAttempt::not_interpreted`] when the wording does not
/// match a supported capability, so the caller can fall back to the broader
/// library router.
pub fn attempt(question: &str) -> CapabilityAttempt {
    // Unit conversion has its own explicit surface and no `FormalizedTarget`.
    if let Some(attempt) = unit_conversion::attempt(question) {
        return attempt;
    }

    let Some(interpretation) = interpret(question) else {
        return CapabilityAttempt::not_interpreted();
    };
    execute(interpretation)
}

/// A capability the adapter selected, plus the typed target built from the
/// question. Keeping the interpretation as data makes step 1–3 testable
/// without running the solver.
#[derive(Clone, Debug)]
pub struct Interpretation {
    pub capability: SupportedCapability,
    pub target: crate::formalization::FormalizedTarget,
}

/// Step 1–2: recognize the requested operation and build a typed target.
///
/// This intentionally reuses the project's existing formalization and target
/// builder (`assess_prompt`) rather than re-deriving subjects and variables
/// with new regexes.
pub fn interpret(question: &str) -> Option<Interpretation> {
    let trace = crate::formalization::assess_prompt("chat", question, "Math", false);
    let target = trace.target_completion.target;

    // A linear system is only executable when the bounded classifier agrees it
    // has a unique 2x2 solution; otherwise it is either unsupported input or a
    // degenerate system we report honestly.
    if target
        .subject_resolution
        .selected
        .as_ref()
        .map(|subject| subject.object_type == SubjectObjectType::EquationSystem)
        .unwrap_or(false)
    {
        return Some(Interpretation {
            capability: SupportedCapability::LinearSystem,
            target,
        });
    }

    let registry = CapabilityRegistry::production();
    let selection = registry.discover(&target).selection;
    let capability = match selection {
        CapabilitySelection::Unique(id) => match id.as_str() {
            "expression_evaluation" => SupportedCapability::ExpressionEvaluation,
            "linear_equation_solve" => SupportedCapability::LinearEquation,
            "quadratic_equation_solve" => SupportedCapability::QuadraticEquation,
            "linear_system_solve" => SupportedCapability::LinearSystem,
            _ => return None,
        },
        CapabilitySelection::Ambiguous(ids) => {
            // The registry found more than one eligible capability. Because a
            // single grounded target should select exactly one, this means the
            // wording did not commit to an operation; do not guess.
            let _ = ids;
            return None;
        }
        CapabilitySelection::None => return None,
    };
    Some(Interpretation { capability, target })
}

fn execute(interpretation: Interpretation) -> CapabilityAttempt {
    match interpretation.capability {
        SupportedCapability::ExpressionEvaluation => {
            expression_evaluation(&interpretation.target)
        }
        SupportedCapability::LinearEquation => linear_equation(&interpretation.target),
        SupportedCapability::QuadraticEquation => quadratic_equation(&interpretation.target),
        SupportedCapability::LinearSystem => linear_system(&interpretation.target),
        SupportedCapability::UnitConversion => unreachable!("handled before interpretation"),
    }
}

fn expression_evaluation(
    target: &crate::formalization::FormalizedTarget,
) -> CapabilityAttempt {
    use crate::expression_evaluation::{
        execute_expression_evaluation, replay_expression_evaluation, ExpressionEvaluationFailure,
    };
    const ID: &str = "expression_evaluation";
    match execute_expression_evaluation(target) {
        Ok(receipt) => {
            if !replay_expression_evaluation(&receipt) {
                return CapabilityAttempt::verification_failed(
                    ID,
                    "expression replay did not reproduce the numeric result",
                );
            }
            let value = render_number(receipt.numeric_result);
            let rendered = if receipt.argument_bindings.is_empty() {
                format!("{} = {value}", receipt.expression_source)
            } else {
                let bindings = receipt
                    .argument_bindings
                    .iter()
                    .map(|(name, binding)| format!("{name} = {binding}"))
                    .collect::<Vec<_>>()
                    .join(", ");
                format!("{} with {bindings} = {value}", receipt.expression_source)
            };
            CapabilityAttempt {
                disposition: CapabilityDisposition::Verified,
                capability: Capability::StructuredSolver {
                    domain: ID.to_string(),
                },
                answer: Some(rendered.clone()),
                evidence: vec![EvidenceRef {
                    kind: EvidenceKind::ComputedAnswer,
                    content: rendered,
                    provenance: "expression_evaluation_replay".to_string(),
                    confidence: 1.0,
                    replay_verified: Some(true),
                    assertion_id: None,
                }],
                verification: VerificationStatus::Verified {
                    method: "expression_replay".to_string(),
                    score: 1.0,
                },
                notes: vec!["evaluated a grounded expression and replayed it from source".to_string()],
            }
        }
        Err(ExpressionEvaluationFailure::BindingMissing(missing)) => CapabilityAttempt::missing(
            ID,
            format!(
                "I can evaluate that expression, but I need a value for {}.",
                missing.join(", ")
            ),
        ),
        Err(ExpressionEvaluationFailure::BindingAmbiguous) => CapabilityAttempt::missing(
            ID,
            "I can evaluate that expression, but one of the argument values is ambiguous.",
        ),
        Err(ExpressionEvaluationFailure::SubjectMissing)
        | Err(ExpressionEvaluationFailure::SubjectNotExpression) => {
            CapabilityAttempt::not_interpreted()
        }
        Err(error) => CapabilityAttempt::unsupported(ID, format!("{error:?}")),
    }
}

fn linear_equation(target: &crate::formalization::FormalizedTarget) -> CapabilityAttempt {
    use crate::linear_equation::{
        execute_linear_equation, replay_linear_equation, LinearEquationFailure,
    };
    const ID: &str = "linear_equation_solve";
    match execute_linear_equation(target) {
        Ok(receipt) => {
            if !replay_linear_equation(&receipt) {
                return CapabilityAttempt::verification_failed(
                    ID,
                    "linear replay did not reproduce the solution",
                );
            }
            let solution = receipt
                .solution_set
                .first()
                .cloned()
                .unwrap_or_else(|| receipt.result.clone());
            CapabilityAttempt {
                disposition: CapabilityDisposition::Verified,
                capability: Capability::StructuredSolver {
                    domain: ID.to_string(),
                },
                answer: Some(format!(
                    "Solving {} for {} gives {} = {}.",
                    receipt.equation_source, receipt.target_variable, receipt.target_variable,
                    solution
                )),
                evidence: vec![EvidenceRef {
                    kind: EvidenceKind::ComputedAnswer,
                    content: format!(
                        "{} = {} from {}",
                        receipt.target_variable, solution, receipt.equation_source
                    ),
                    provenance: "linear_equation_replay".to_string(),
                    confidence: 1.0,
                    replay_verified: Some(true),
                    assertion_id: None,
                }],
                verification: VerificationStatus::Verified {
                    method: "linear_equation_replay".to_string(),
                    score: 1.0,
                },
                notes: vec!["degree-one equation solved and replayed".to_string()],
            }
        }
        Err(LinearEquationFailure::TargetVariableMissing) => CapabilityAttempt::missing(
            ID,
            "I can solve that equation, but I need to know which variable to solve for.",
        ),
        Err(LinearEquationFailure::TargetVariableAmbiguous) => CapabilityAttempt::missing(
            ID,
            "I can solve that equation, but the target variable is ambiguous.",
        ),
        Err(LinearEquationFailure::SubjectMissing)
        | Err(LinearEquationFailure::SubjectNotEquation) => CapabilityAttempt::not_interpreted(),
        Err(LinearEquationFailure::OperationNotSolve) => {
            CapabilityAttempt::not_interpreted()
        }
        Err(error) => CapabilityAttempt::unsupported(ID, format!("{error:?}")),
    }
}

fn quadratic_equation(target: &crate::formalization::FormalizedTarget) -> CapabilityAttempt {
    use crate::quadratic_equation::{
        execute_quadratic_equation, replay_quadratic_equation, QuadraticEquationFailure,
    };
    const ID: &str = "quadratic_equation_solve";
    match execute_quadratic_equation(target) {
        Ok(receipt) => {
            if !replay_quadratic_equation(&receipt) {
                return CapabilityAttempt::verification_failed(
                    ID,
                    "quadratic replay did not reproduce the solution set",
                );
            }
            let roots = receipt
                .solution_set
                .iter()
                .map(|root| format!("{} = {root}", receipt.target_variable))
                .collect::<Vec<_>>()
                .join(" or ");
            CapabilityAttempt {
                disposition: CapabilityDisposition::Verified,
                capability: Capability::StructuredSolver {
                    domain: ID.to_string(),
                },
                answer: Some(format!(
                    "Solving {} for {} gives {roots}.",
                    receipt.equation_source, receipt.target_variable
                )),
                evidence: vec![EvidenceRef {
                    kind: EvidenceKind::ComputedAnswer,
                    content: format!("{roots} from {}", receipt.equation_source),
                    provenance: "quadratic_equation_replay".to_string(),
                    confidence: 1.0,
                    replay_verified: Some(true),
                    assertion_id: None,
                }],
                verification: VerificationStatus::Verified {
                    method: "quadratic_equation_replay".to_string(),
                    score: 1.0,
                },
                notes: vec!["degree-two equation solved and replayed".to_string()],
            }
        }
        Err(QuadraticEquationFailure::NoRealSolution) => CapabilityAttempt::unsupported(
            ID,
            "that quadratic has no real solution, which this bounded capability does not report",
        ),
        Err(QuadraticEquationFailure::TargetVariableMissing) => CapabilityAttempt::missing(
            ID,
            "I can solve that quadratic, but I need to know which variable to solve for.",
        ),
        Err(QuadraticEquationFailure::TargetVariableAmbiguous) => CapabilityAttempt::missing(
            ID,
            "I can solve that quadratic, but the target variable is ambiguous.",
        ),
        Err(QuadraticEquationFailure::SubjectMissing)
        | Err(QuadraticEquationFailure::SubjectNotEquation) => {
            CapabilityAttempt::not_interpreted()
        }
        Err(QuadraticEquationFailure::OperationNotSolve) => {
            CapabilityAttempt::not_interpreted()
        }
        Err(error) => CapabilityAttempt::unsupported(ID, format!("{error:?}")),
    }
}

fn linear_system(target: &crate::formalization::FormalizedTarget) -> CapabilityAttempt {
    const ID: &str = "linear_system_solve";
    let source = match system_source(target) {
        Some(source) => source,
        None => return CapabilityAttempt::not_interpreted(),
    };
    match crate::linear_system::classify_linear_system(&source) {
        LinearSystemClassification::Unique(solution) => {
            match crate::linear_system::execute_linear_system(&source) {
                Ok(receipt) => {
                    if !crate::linear_system::replay_linear_system(&receipt) {
                        return CapabilityAttempt::verification_failed(
                            ID,
                            "linear-system replay did not reproduce the solution",
                        );
                    }
                    let pairs = solution
                        .iter()
                        .map(|(variable, value)| format!("{variable} = {value}"))
                        .collect::<Vec<_>>()
                        .join(", ");
                    CapabilityAttempt {
                        disposition: CapabilityDisposition::Verified,
                        capability: Capability::StructuredSolver {
                            domain: ID.to_string(),
                        },
                        answer: Some(format!("The system has the unique solution {pairs}.")),
                        evidence: vec![EvidenceRef {
                            kind: EvidenceKind::ComputedAnswer,
                            content: format!("{pairs} from {source}"),
                            provenance: "linear_system_replay".to_string(),
                            confidence: 1.0,
                            replay_verified: Some(true),
                            assertion_id: None,
                        }],
                        verification: VerificationStatus::Verified {
                            method: "linear_system_replay".to_string(),
                            score: 1.0,
                        },
                        notes: vec!["2x2 system classified unique, solved, and replayed".to_string()],
                    }
                }
                Err(error) => CapabilityAttempt::unsupported(ID, format!("{error:?}")),
            }
        }
        LinearSystemClassification::NoSolution => CapabilityAttempt::unsupported(
            ID,
            "that system has no solution; this capability only reports a unique 2x2 solution",
        ),
        LinearSystemClassification::InfiniteSolutions(_) => CapabilityAttempt::unsupported(
            ID,
            "that system has infinitely many solutions; this capability only reports a unique 2x2 solution",
        ),
        LinearSystemClassification::Unsupported => {
            // The classifier could not read the system. That is a wording
            // problem, not a verified unsupported operation.
            CapabilityAttempt::not_interpreted()
        }
    }
}

/// Reconstruct the exact system source the classifier and executor expect from
/// the grounded equation-system subject.
fn system_source(target: &crate::formalization::FormalizedTarget) -> Option<String> {
    let subject = target.subject_resolution.selected.as_ref()?;
    if subject.object_type != SubjectObjectType::EquationSystem {
        return None;
    }
    let variables = target.target_variable.as_deref().unwrap_or("x,y");
    let object = subject.object.trim();
    let equations = if let Some(rest) = object.strip_prefix(':') {
        rest.trim()
    } else {
        object
    };
    Some(format!("Solve system: {equations} for {variables}"))
}

fn render_number(value: f64) -> String {
    if value.fract() == 0.0 && value.abs() < 1e15 {
        format!("{}", value as i64)
    } else {
        format!("{value}")
    }
}

/// Explicit unit conversion and compatible unit addition/subtraction.
mod unit_conversion {
    use super::*;
    use crate::unit_aware_quantity::{
        formalize, UnitQuantityArtifact, UnitQuantityDecision,
    };

    const ID: &str = "unit_conversion";

    /// Returns `None` when the prompt is not unit-shaped at all, so the caller
    /// can fall back. A unit-shaped but unsupported/ambiguous prompt returns a
    /// reasoned attempt.
    pub fn attempt(question: &str) -> Option<CapabilityAttempt> {
        if !looks_unit_shaped(question) {
            return None;
        }
        match formalize(question) {
            UnitQuantityDecision::Accepted(artifact) => Some(execute(&artifact)),
            UnitQuantityDecision::Ambiguous => Some(CapabilityAttempt::missing(
                ID,
                "I can convert that, but the conversion factor or target unit is not specified. \
                 Please state it, for example \"convert 3 meters to centimeters using 100 \
                 centimeters per meter\".",
            )),
            UnitQuantityDecision::Unsupported => {
                // The frontend cannot distinguish "known family, factor not
                // stated" from "family I do not support". This adapter should:
                // a stated-factor-less conversion inside a supported family is
                // missing information (ask), everything else is unsupported.
                if needs_stated_factor(question) {
                    Some(CapabilityAttempt::missing(
                        ID,
                        "I can convert that, but I need the conversion factor. Please state it, \
                         for example \"convert 3 meters to centimeters using 100 centimeters per \
                         meter\".",
                    ))
                } else {
                    Some(CapabilityAttempt::unsupported(
                        ID,
                        "that unit request is outside the explicit conversions I support (length, \
                         volume, mass, and time, with a stated factor)",
                    ))
                }
            }
        }
    }

    /// A conversion between two units of a supported family that does not
    /// state a factor is missing information, not an unsupported operation.
    fn needs_stated_factor(question: &str) -> bool {
        let lower = question.to_ascii_lowercase();
        let conversion = lower.starts_with("convert ") || lower.starts_with("express ");
        let families = [
            "meter", "centimeter", "foot", "feet", "inch", "inches", "liter", "milliliter",
            "kilogram", "gram", "hour", "minute",
        ];
        conversion
            && !lower.contains(" per ")
            && families.iter().filter(|unit| lower.contains(**unit)).count() >= 2
    }

    fn looks_unit_shaped(question: &str) -> bool {
        let lower = question.to_ascii_lowercase();
        lower.starts_with("convert ")
            || lower.starts_with("express ")
            || lower.starts_with("add ")
            || lower.starts_with("subtract ")
            || (lower.contains(" per ") && lower.contains(" to "))
    }

    fn execute(artifact: &UnitQuantityArtifact) -> CapabilityAttempt {
        if !artifact.replay_verified() {
            return CapabilityAttempt::verification_failed(
                ID,
                "the unit artifact did not satisfy its own replay check",
            );
        }
        // Evaluate the exact expression through the replayed algebra bridge,
        // which is the same independent check used by the composition tests.
        let value = match crate::unit_quantity_composition::compose_to_algebra(artifact) {
            Some(receipt) if receipt.unit_replay_verified && receipt.relation_replay_verified => {
                receipt.algebra.result
            }
            _ => {
                return CapabilityAttempt::verification_failed(
                    ID,
                    "the unit expression could not be independently replayed through algebra",
                )
            }
        };
        let answer = match artifact.operation.as_str() {
            "conversion" => format!(
                "{} gives {value} {}.",
                relation_phrase(artifact),
                artifact.target_unit
            ),
            "addition_subtraction" => format!(
                "{} gives {value} {}.",
                relation_phrase(artifact),
                artifact.target_unit
            ),
            _ => format!("The result is {value} {}.", artifact.target_unit),
        };
        CapabilityAttempt {
            disposition: CapabilityDisposition::Verified,
            capability: Capability::StructuredSolver {
                domain: ID.to_string(),
            },
            answer: Some(answer),
            evidence: vec![EvidenceRef {
                kind: EvidenceKind::ComputedAnswer,
                content: format!(
                    "{} -> {} {} (expression {})",
                    artifact.signature, value, artifact.target_unit, artifact.expression
                ),
                provenance: "unit_quantity_algebra_replay".to_string(),
                confidence: 1.0,
                replay_verified: Some(true),
                assertion_id: None,
            }],
            verification: VerificationStatus::Verified {
                method: "unit_quantity_algebra_replay".to_string(),
                score: 1.0,
            },
            notes: vec!["explicit unit conversion replayed through the algebra bridge".to_string()],
        }
    }

    /// A readable phrase for the unit relation, derived from the artifact's
    /// own constraint (never invented). The stored form is either
    /// `amount unit = factor unit/unit` (conversion) or
    /// `left unit op right unit -> target` (add/subtract).
    fn relation_phrase(artifact: &UnitQuantityArtifact) -> String {
        let raw = artifact
            .constraints
            .first()
            .map(|constraint| constraint.lhs.clone())
            .unwrap_or_else(|| artifact.expression.clone());
        match raw.split_once("->") {
            Some((left, target)) => format!(
                "{} (in {})",
                left.trim().replace(" + ", " plus ").replace(" - ", " minus "),
                target.trim()
            ),
            None => raw,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn attempt_outcome(question: &str) -> (CapabilityDisposition, CapabilityAttempt) {
        let attempt = attempt(question);
        (attempt.disposition.clone(), attempt)
    }

    #[test]
    fn evaluates_a_grounded_expression() {
        let (disposition, attempt) = attempt_outcome("Evaluate 2+3.");
        assert!(attempt.is_verified(), "{attempt:?}");
        assert!(matches!(disposition, CapabilityDisposition::Verified));
        assert_eq!(attempt.capability.id(), "structured_solver:expression_evaluation");
        assert!(attempt.answer.as_deref().unwrap().contains('5'), "{attempt:?}");
        assert_eq!(attempt.evidence[0].replay_verified, Some(true));
    }

    #[test]
    fn evaluates_a_bound_expression_paraphrase() {
        let attempt = attempt("Evaluate 2*x+3 at x=4.");
        assert!(attempt.is_verified(), "{attempt:?}");
        assert!(attempt.answer.as_deref().unwrap().contains("11"), "{attempt:?}");
    }

    #[test]
    fn solves_a_linear_equation_and_reports_the_root() {
        let attempt = attempt("Solve 2*x + 3 = 11 for x.");
        assert!(attempt.is_verified(), "{attempt:?}");
        assert_eq!(attempt.capability.id(), "structured_solver:linear_equation_solve");
        assert!(attempt.answer.as_deref().unwrap().contains('4'), "{attempt:?}");
    }

    #[test]
    fn solves_a_quadratic_and_reports_both_roots() {
        let attempt = attempt("Solve x^2 - 5*x + 6 = 0 for x.");
        assert!(attempt.is_verified(), "{attempt:?}");
        assert_eq!(attempt.capability.id(), "structured_solver:quadratic_equation_solve");
        let answer = attempt.answer.as_deref().unwrap();
        assert!(answer.contains('2') && answer.contains('3'), "{attempt:?}");
    }

    #[test]
    fn solves_a_small_linear_system() {
        let attempt = attempt("Solve system: x + y = 5; x - y = 1 for x,y");
        assert!(attempt.is_verified(), "{attempt:?}");
        assert_eq!(attempt.capability.id(), "structured_solver:linear_system_solve");
        let answer = attempt.answer.as_deref().unwrap();
        assert!(answer.contains("x = 3") && answer.contains("y = 2"), "{attempt:?}");
    }

    #[test]
    fn converts_units_through_the_algebra_bridge() {
        let attempt = attempt("Convert 3 meters to centimeters using 100 centimeters per meter.");
        assert!(attempt.is_verified(), "{attempt:?}");
        assert_eq!(attempt.capability.id(), "structured_solver:unit_conversion");
        assert!(attempt.answer.as_deref().unwrap().contains("300"), "{attempt:?}");
    }

    #[test]
    fn missing_binding_asks_instead_of_guessing() {
        let attempt = attempt("Evaluate 2*x+3.");
        assert!(
            matches!(attempt.disposition, CapabilityDisposition::MissingInformation { .. }),
            "{attempt:?}"
        );
        assert!(matches!(attempt.outcome(), TurnOutcome::ClarificationNeeded { .. }));
    }

    #[test]
    fn missing_unit_factor_asks_instead_of_guessing() {
        let attempt = attempt("Convert 3 meters to centimeters.");
        assert!(
            matches!(attempt.disposition, CapabilityDisposition::MissingInformation { .. }),
            "{attempt:?}"
        );
    }

    #[test]
    fn unsupported_unit_family_is_reported_as_unsupported() {
        let attempt = attempt("Convert 3 miles to kilometers using 1.6 kilometers per mile.");
        assert!(
            matches!(attempt.disposition, CapabilityDisposition::OperationUnsupported { .. }),
            "{attempt:?}"
        );
        assert!(matches!(attempt.outcome(), TurnOutcome::Unsupported { .. }));
    }

    #[test]
    fn uninterpretable_wording_is_not_claimed_by_the_adapter() {
        let attempt = attempt("Who manages the observatory?");
        assert!(
            matches!(attempt.disposition, CapabilityDisposition::NotInterpreted),
            "{attempt:?}"
        );
    }

    #[test]
    fn malformed_math_is_not_interpreted_rather_than_guessed() {
        let attempt = attempt("Solve for x: 2*x + = 11");
        assert!(
            matches!(attempt.disposition, CapabilityDisposition::NotInterpreted),
            "{attempt:?}"
        );
    }
}
