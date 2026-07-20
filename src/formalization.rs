//! Report-only formalization diagnostics.
//!
//! This module measures how far a prompt is from an executable typed problem.
//! It deliberately does not authorize a solver or infer missing mathematics.
//! A future parser can populate the same structures with stronger evidence;
//! the current assessor is conservative and intended for corpus analysis.

use crate::math_methods::{MathDomain, TaskShape};
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
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

impl DefinitionKind {
    pub fn label(self) -> &'static str {
        match self {
            Self::Function => "function",
            Self::Sequence => "sequence",
            Self::Set => "set",
            Self::Operation => "operation",
            Self::Relation => "relation",
            Self::Graph => "graph",
            Self::ProbabilityModel => "probability_model",
            Self::PhysicalQuantity => "physical_quantity",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ModelingObligation {
    DefineObject,
    IdentifyStateVariables,
    IdentifyDomain,
    ExtractQuantifiers,
    DetermineTargetSemantics,
    ConstructEquation,
    ConstructSmallSystem,
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
            Self::ConstructSmallSystem => "construct_small_system",
            Self::EstablishBoundaryConditions => "establish_boundary_conditions",
            Self::EstablishInitialConditions => "establish_initial_conditions",
            Self::SelectApproximationRegime => "select_approximation_regime",
            Self::ResolveEntityReference => "resolve_entity_reference",
            Self::SelectSpecializedMethod => "select_specialized_method",
            Self::ParseAttachment => "parse_attachment",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FormalizationStatus {
    Structured,
    PartiallyStructured,
    Unresolved,
    AttachmentRequired,
}

/// Input dependencies are tracked independently from formalization distance.
/// A prompt may therefore be both direct-instantiation-shaped and diagram
/// dependent; the distance must not hide that separate integration blocker.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum InputDependency {
    TextOnly,
    Image,
    Diagram,
    Table,
    Graph,
    MissingAttachment,
}

impl InputDependency {
    pub fn label(self) -> &'static str {
        match self {
            Self::TextOnly => "text_only",
            Self::Image => "image",
            Self::Diagram => "diagram",
            Self::Table => "table",
            Self::Graph => "graph",
            Self::MissingAttachment => "missing_attachment",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SuppliedObjectKind {
    ExplicitDefinition,
    ExplicitEquation,
    ExplicitRecurrence,
    ExplicitAlgorithm,
    ExplicitTransformationRule,
    ExplicitTheoremStatement,
    ExplicitDataTable,
    ExplicitLogicalPremises,
    ExplicitPhysicalLaw,
    Unclear,
}

impl SuppliedObjectKind {
    pub fn label(self) -> &'static str {
        match self {
            Self::ExplicitDefinition => "explicit_definition",
            Self::ExplicitEquation => "explicit_equation",
            Self::ExplicitRecurrence => "explicit_recurrence",
            Self::ExplicitAlgorithm => "explicit_algorithm",
            Self::ExplicitTransformationRule => "explicit_transformation_rule",
            Self::ExplicitTheoremStatement => "explicit_theorem_statement",
            Self::ExplicitDataTable => "explicit_data_table",
            Self::ExplicitLogicalPremises => "explicit_logical_premises",
            Self::ExplicitPhysicalLaw => "explicit_physical_law",
            Self::Unclear => "unclear",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum InstantiationTargetKind {
    EvaluateAtArguments,
    SubstituteValues,
    ApplyOnce,
    VerifyInstance,
    ClassifyObject,
    ComputeDerivedProperty,
    DetermineWhetherConditionHolds,
    ProduceCounterexample,
    ProveConsequence,
    Other,
}

impl InstantiationTargetKind {
    pub fn label(self) -> &'static str {
        match self {
            Self::EvaluateAtArguments => "evaluate_at_arguments",
            Self::SubstituteValues => "substitute_values",
            Self::ApplyOnce => "apply_once",
            Self::VerifyInstance => "verify_instance",
            Self::ClassifyObject => "classify_object",
            Self::ComputeDerivedProperty => "compute_derived_property",
            Self::DetermineWhetherConditionHolds => "determine_condition",
            Self::ProduceCounterexample => "produce_counterexample",
            Self::ProveConsequence => "prove_consequence",
            Self::Other => "other",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RepresentationReadiness {
    RepresentationReady,
    NearReady,
    FalseDirectInstantiation,
}

/// Why a low-distance surface match is unsafe to treat as direct execution.
/// This is deliberately a coarse, mutually-exclusive diagnostic label: the
/// detailed blockers remain available on `DirectInstantiationAssessment`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum FalseLowDistanceReason {
    DefinitionMentionedButNotOperational,
    SuppliedObjectIncomplete,
    TargetRequiresModelConstruction,
    TargetRequiresTheoremSelection,
    TargetRequiresProof,
    SpecializedDefinitionUnrepresented,
    QuantifiedStructureUnresolved,
    ImplicitObjectConstruction,
    CrossDomainInterpretationRequired,
    VisualEvidenceRequired,
    SurfaceEquationNotCentral,
    SurfaceRuleNotApplicable,
}

impl FalseLowDistanceReason {
    pub fn label(self) -> &'static str {
        match self {
            Self::DefinitionMentionedButNotOperational => {
                "definition_mentioned_but_not_operational"
            }
            Self::SuppliedObjectIncomplete => "supplied_object_incomplete",
            Self::TargetRequiresModelConstruction => "target_requires_model_construction",
            Self::TargetRequiresTheoremSelection => "target_requires_theorem_selection",
            Self::TargetRequiresProof => "target_requires_proof",
            Self::SpecializedDefinitionUnrepresented => "specialized_definition_unrepresented",
            Self::QuantifiedStructureUnresolved => "quantified_structure_unresolved",
            Self::ImplicitObjectConstruction => "implicit_object_construction",
            Self::CrossDomainInterpretationRequired => "cross_domain_interpretation_required",
            Self::VisualEvidenceRequired => "visual_evidence_required",
            Self::SurfaceEquationNotCentral => "surface_equation_not_central",
            Self::SurfaceRuleNotApplicable => "surface_rule_not_applicable",
        }
    }
}

impl RepresentationReadiness {
    pub fn label(self) -> &'static str {
        match self {
            Self::RepresentationReady => "representation_ready",
            Self::NearReady => "near_ready",
            Self::FalseDirectInstantiation => "false_direct_instantiation",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct DirectInstantiationAssessment {
    pub question_id: String,
    pub supplied_object: SuppliedObjectKind,
    pub target: InstantiationTargetKind,
    pub readiness: RepresentationReadiness,
    /// Conservative lower bound used by future authorization.  A heuristic
    /// direct-instantiation prediction must never lower this bound merely
    /// because an equation/definition-shaped phrase was detected.
    pub conservative_lower_bound: ModelingDistance,
    pub definition_isolated: bool,
    pub binders_identified: bool,
    pub domain_identified: bool,
    pub target_identified: bool,
    pub quantifiers_preserved: bool,
    pub side_conditions_identified: bool,
    pub one_step_representable: bool,
    pub verifier_available: bool,
    pub missing_representation: Vec<String>,
    pub authorization_blockers: Vec<String>,
    pub false_low_distance_reason: Option<FalseLowDistanceReason>,
}

impl DirectInstantiationAssessment {
    /// The only predicate a future executor may use to authorize a
    /// prompt-supplied direct application.  Surface distance alone is never
    /// sufficient.
    pub fn authorization_safe(&self) -> bool {
        self.readiness == RepresentationReadiness::RepresentationReady
            && self.conservative_lower_bound == ModelingDistance::DirectInstantiation
            && self.definition_isolated
            && self.binders_identified
            && self.domain_identified
            && self.target_identified
            && self.quantifiers_preserved
            && self.side_conditions_identified
            && self.one_step_representable
            && self.verifier_available
            && self.missing_representation.is_empty()
            && self.authorization_blockers.is_empty()
            && self.false_low_distance_reason.is_none()
    }
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

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EntityAnnotation {
    pub label: String,
    pub source_fragment: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FactAnnotation {
    pub statement: String,
    pub source_fragment: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TargetAnnotation {
    pub statement: String,
    pub source_fragment: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AssumptionAnnotation {
    pub statement: String,
    pub source_fragment: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConstraintAnnotation {
    pub statement: String,
    pub source_fragment: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
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

/// The primary language-to-form transformation represented by a gold case.
/// Labels are intentionally about the transformation, not the subject domain,
/// so failures can be grouped across algebra, mechanics, and discrete math.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FormalizationTransformation {
    ExtractExplicitEquation,
    InstantiateDefinition,
    TranslateComparisonToInequality,
    TranslateRateStatement,
    ConstructSingleEquation,
    ConstructSmallSystem,
    BindEntitiesAcrossSentences,
    ExtractDomainRestriction,
    ExtractQuantifierStructure,
    IdentifyRequestedTarget,
}

impl FormalizationTransformation {
    pub fn label(self) -> &'static str {
        match self {
            Self::ExtractExplicitEquation => "extract_explicit_equation",
            Self::InstantiateDefinition => "instantiate_definition",
            Self::TranslateComparisonToInequality => "translate_comparison_to_inequality",
            Self::TranslateRateStatement => "translate_rate_statement",
            Self::ConstructSingleEquation => "construct_single_equation",
            Self::ConstructSmallSystem => "construct_small_system",
            Self::BindEntitiesAcrossSentences => "bind_entities_across_sentences",
            Self::ExtractDomainRestriction => "extract_domain_restriction",
            Self::ExtractQuantifierStructure => "extract_quantifier_structure",
            Self::IdentifyRequestedTarget => "identify_requested_target",
        }
    }
}

/// A provenance span supporting a gold annotation.  `source_fragment` is
/// kept as text (rather than byte offsets) so corpus files remain stable when
/// prompt storage changes line endings or Unicode normalization.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProvenanceSpan {
    pub role: String,
    pub source_fragment: String,
}

/// Curriculum tier used by the formalization benchmark.  Tiers describe the
/// distance from language to a typed problem, not the difficulty of the final
/// arithmetic executor.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FormalizationTier {
    ExplicitObject,
    DirectInstantiation,
    ProseModeling,
    MethodSelection,
    SpecialistReasoning,
}

impl FormalizationTier {
    pub fn label(self) -> &'static str {
        match self {
            Self::ExplicitObject => "explicit_object",
            Self::DirectInstantiation => "direct_instantiation",
            Self::ProseModeling => "prose_modeling",
            Self::MethodSelection => "method_selection",
            Self::SpecialistReasoning => "specialist_reasoning",
        }
    }
}

/// A manually reviewed gold item for the formalization curriculum.  This is
/// intentionally separate from `FormalizationTrace`: traces are heuristic
/// observations, while gold cases are human-authorized expectations used to
/// measure field-level extraction and false authorization.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FormalizationGoldCase {
    pub id: String,
    pub tier: FormalizationTier,
    pub transformation: FormalizationTransformation,
    pub prompt: String,
    pub definitions: Vec<DefinitionKind>,
    pub entities: Vec<EntityAnnotation>,
    pub facts: Vec<FactAnnotation>,
    pub target: TargetAnnotation,
    pub assumptions: Vec<AssumptionAnnotation>,
    pub constraints: Vec<ConstraintAnnotation>,
    pub obligations: Vec<ModelingObligation>,
    pub provenance_spans: Vec<ProvenanceSpan>,
    pub authorization_expected: bool,
    pub allowed_methods: Vec<String>,
    pub expected_answer: Option<String>,
}

impl FormalizationGoldCase {
    /// Validate the annotation contract without executing a solver.  An empty
    /// result means the case is internally well-formed; it does not claim
    /// that the machine can solve the case.
    pub fn validation_errors(&self) -> Vec<String> {
        let mut errors = Vec::new();
        if self.id.trim().is_empty() {
            errors.push("id is empty".to_string());
        }
        if self.prompt.trim().is_empty() {
            errors.push("prompt is empty".to_string());
        }
        if self.target.statement.trim().is_empty() {
            errors.push("target statement is empty".to_string());
        }
        if self.target.source_fragment.trim().is_empty() {
            errors.push("target source_fragment is empty".to_string());
        }
        if self.provenance_spans.is_empty() {
            errors.push("provenance_spans must contain at least one supporting span".to_string());
        }
        for (idx, span) in self.provenance_spans.iter().enumerate() {
            if span.role.trim().is_empty() {
                errors.push(format!("provenance_spans[{idx}] role is empty"));
            }
            if span.source_fragment.trim().is_empty() {
                errors.push(format!("provenance_spans[{idx}] source_fragment is empty"));
            }
        }
        if self.authorization_expected {
            if self.allowed_methods.is_empty() {
                errors.push("authorization_expected requires allowed_methods".to_string());
            }
            if self.expected_answer.is_none() {
                errors.push("authorization_expected requires expected_answer".to_string());
            }
            if self.tier > FormalizationTier::DirectInstantiation {
                errors.push(
                    "authorization_expected is only valid for explicit/direct tiers".to_string(),
                );
            }
        }
        for (idx, fact) in self.facts.iter().enumerate() {
            if fact.statement.trim().is_empty() {
                errors.push(format!("fact[{idx}] statement is empty"));
            }
            if fact.source_fragment.trim().is_empty() {
                errors.push(format!("fact[{idx}] source_fragment is empty"));
            }
        }
        errors
    }

    pub fn is_valid(&self) -> bool {
        self.validation_errors().is_empty()
    }
}

/// Versioned corpus envelope.  Versioning prevents a future change to the
/// annotation contract from silently reinterpreting old gold labels.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FormalizationCorpus {
    pub schema_version: u32,
    pub cases: Vec<FormalizationGoldCase>,
}

impl FormalizationCorpus {
    pub const CURRENT_SCHEMA_VERSION: u32 = 1;

    pub fn validation_errors(&self) -> Vec<String> {
        let mut errors = Vec::new();
        if self.schema_version != Self::CURRENT_SCHEMA_VERSION {
            errors.push(format!(
                "unsupported schema_version {}; expected {}",
                self.schema_version,
                Self::CURRENT_SCHEMA_VERSION
            ));
        }
        let mut ids = BTreeSet::new();
        for (idx, case) in self.cases.iter().enumerate() {
            for error in case.validation_errors() {
                errors.push(format!("case[{idx}] {}: {error}", case.id));
            }
            if !ids.insert(case.id.clone()) {
                errors.push(format!("duplicate case id: {}", case.id));
            }
        }
        errors
    }

    pub fn is_valid(&self) -> bool {
        self.validation_errors().is_empty()
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FieldScore {
    pub matched: usize,
    pub expected: usize,
    pub predicted: usize,
    pub precision: f64,
    pub recall: f64,
}

impl FieldScore {
    fn from_counts(matched: usize, expected: usize, predicted: usize) -> Self {
        Self {
            matched,
            expected,
            predicted,
            precision: if predicted == 0 {
                if expected == 0 {
                    1.0
                } else {
                    0.0
                }
            } else {
                matched as f64 / predicted as f64
            },
            recall: if expected == 0 {
                if predicted == 0 {
                    1.0
                } else {
                    0.0
                }
            } else {
                matched as f64 / expected as f64
            },
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct FormalizationScore {
    pub definitions: FieldScore,
    pub facts: FieldScore,
    pub entities: FieldScore,
    pub assumptions: FieldScore,
    pub constraints: FieldScore,
    pub obligations: FieldScore,
    pub target_exact: bool,
    /// Structural target agreement is deliberately weaker than textual
    /// equality: a trace may include the full prompt sentence while the gold
    /// target stores only the requested operation.
    pub target_structural: bool,
    pub authorization_correct: bool,
}

fn normalized_text(value: &str) -> String {
    value
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_ascii_lowercase()
}

fn target_tokens(value: &str) -> BTreeSet<String> {
    normalized_text(value)
        .split(|c: char| !c.is_ascii_alphanumeric() && c != '_')
        .filter(|token| token.len() > 1)
        .map(str::to_string)
        .collect()
}

fn set_score(
    expected: impl IntoIterator<Item = String>,
    predicted: impl IntoIterator<Item = String>,
) -> FieldScore {
    let expected: BTreeSet<_> = expected.into_iter().map(|v| normalized_text(&v)).collect();
    let predicted: BTreeSet<_> = predicted.into_iter().map(|v| normalized_text(&v)).collect();
    let matched = expected.intersection(&predicted).count();
    FieldScore::from_counts(matched, expected.len(), predicted.len())
}

/// Compare a model trace with a manually reviewed gold case.  This is a
/// diagnostic scorer only: it does not execute a method or grant authority.
pub fn score_formalization(
    gold: &FormalizationGoldCase,
    trace: &FormalizationTrace,
    actual_authorization: bool,
) -> FormalizationScore {
    let predicted_target = trace
        .target
        .as_ref()
        .map(|v| v.statement.as_str())
        .unwrap_or_default();
    let gold_tokens = target_tokens(&gold.target.statement);
    let predicted_tokens = target_tokens(predicted_target);
    let target_structural = !gold_tokens.is_empty()
        && gold_tokens.intersection(&predicted_tokens).count() * 2 >= gold_tokens.len();
    FormalizationScore {
        definitions: set_score(
            gold.definitions.iter().map(|v| v.label().to_string()),
            trace.definitions.iter().map(|v| v.label().to_string()),
        ),
        facts: set_score(
            gold.facts.iter().map(|v| v.statement.clone()),
            trace.facts.iter().map(|v| v.statement.clone()),
        ),
        entities: set_score(
            gold.entities.iter().map(|v| v.label.clone()),
            trace.entities.iter().map(|v| v.label.clone()),
        ),
        assumptions: set_score(
            gold.assumptions.iter().map(|v| v.statement.clone()),
            trace.assumptions.iter().map(|v| v.statement.clone()),
        ),
        constraints: set_score(
            gold.constraints.iter().map(|v| v.statement.clone()),
            trace.constraints.iter().map(|v| v.statement.clone()),
        ),
        obligations: set_score(
            gold.obligations.iter().map(|v| v.label().to_string()),
            trace.obligations.iter().map(|v| v.label().to_string()),
        ),
        target_exact: gold.target.statement == predicted_target,
        target_structural,
        authorization_correct: gold.authorization_expected == actual_authorization,
    }
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
    pub input_dependencies: BTreeSet<InputDependency>,
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

fn classify_supplied_object(question: &str, trace: &FormalizationTrace) -> SuppliedObjectKind {
    let text = question.to_ascii_lowercase();
    if has_any(
        &text,
        &["pseudocode", "algorithm", "procedure", "step 1", "step 2"],
    ) {
        SuppliedObjectKind::ExplicitAlgorithm
    } else if has_any(&text, &["recurrence", "recursive", "a_n+1", "a_{n+1}"]) {
        SuppliedObjectKind::ExplicitRecurrence
    } else if has_any(&text, &["data table", "following table", "table below"]) {
        SuppliedObjectKind::ExplicitDataTable
    } else if has_any(
        &text,
        &[
            "theorem states",
            "the theorem says",
            "lemma states",
            "criterion states",
        ],
    ) {
        SuppliedObjectKind::ExplicitTheoremStatement
    } else if has_any(&text, &["physical law", "law of", "is given by", "obeys"]) {
        SuppliedObjectKind::ExplicitPhysicalLaw
    } else if has_any(
        &text,
        &[
            "if and only if",
            "assume that",
            "suppose that",
            "given that",
        ],
    ) && trace.facts.len() >= 1
    {
        SuppliedObjectKind::ExplicitLogicalPremises
    } else if has_any(
        &text,
        &["transform", "mapping", "operator", "send ", "maps "],
    ) {
        SuppliedObjectKind::ExplicitTransformationRule
    } else if !trace.definitions.is_empty() {
        SuppliedObjectKind::ExplicitDefinition
    } else if text.contains('=') || !trace.facts.is_empty() {
        SuppliedObjectKind::ExplicitEquation
    } else {
        SuppliedObjectKind::Unclear
    }
}

fn classify_instantiation_target(question: &str, task: TaskShape) -> InstantiationTargetKind {
    let text = question.to_ascii_lowercase();
    if task == TaskShape::ProveIdentity
        || has_any(&text, &["prove that", "show that", "establish that"])
    {
        InstantiationTargetKind::ProveConsequence
    } else if has_any(&text, &["counterexample", "disprove"]) {
        InstantiationTargetKind::ProduceCounterexample
    } else if has_any(
        &text,
        &["verify", "check whether", "check if", "test whether"],
    ) {
        InstantiationTargetKind::VerifyInstance
    } else if has_any(&text, &["substitute", "plug in", "substitution of"]) {
        InstantiationTargetKind::SubstituteValues
    } else if has_any(
        &text,
        &[
            "at x =",
            "at n =",
            "evaluate at",
            "value of f(",
            "compute f(",
        ],
    ) {
        InstantiationTargetKind::EvaluateAtArguments
    } else if has_any(
        &text,
        &[
            "apply the",
            "apply once",
            "next step",
            "next term",
            "trace the",
        ],
    ) {
        InstantiationTargetKind::ApplyOnce
    } else if has_any(
        &text,
        &[
            "check whether",
            "check if",
            "test whether",
            "is it true that",
        ],
    ) {
        InstantiationTargetKind::DetermineWhetherConditionHolds
    } else if task == TaskShape::CountObjects
        || has_any(&text, &["classify", "which class", "what type"])
    {
        InstantiationTargetKind::ClassifyObject
    } else if has_any(
        &text,
        &[
            "derive",
            "resulting",
            "consequent",
            "what is the",
            "compute",
        ],
    ) {
        InstantiationTargetKind::ComputeDerivedProperty
    } else {
        InstantiationTargetKind::Other
    }
}

/// Second-level audit of low-distance predictions.  This remains diagnostic:
/// no method is authorized by this heuristic and no executor is called.
pub fn assess_direct_instantiation(trace: &FormalizationTrace) -> DirectInstantiationAssessment {
    let source = trace.target_source();
    let source_lower = source.to_ascii_lowercase();
    let supplied_object = classify_supplied_object(&source, trace);
    let mut target = classify_instantiation_target(&source, trace.task_shape);
    if target == InstantiationTargetKind::ApplyOnce
        && !matches!(
            classify_supplied_object(&source, trace),
            SuppliedObjectKind::ExplicitAlgorithm
                | SuppliedObjectKind::ExplicitRecurrence
                | SuppliedObjectKind::ExplicitTransformationRule
        )
    {
        // “What is the next step?” in a clinical or procedural narrative is
        // not application of a supplied mathematical rule.
        target = InstantiationTargetKind::Other;
    }
    let definition_isolated = !matches!(supplied_object, SuppliedObjectKind::Unclear);
    let binders_identified = !trace.definitions.is_empty() || !trace.facts.is_empty();
    let domain_identified = !trace.constraints.is_empty();
    let target_identified = trace.target.is_some();
    let quantifiers_preserved = !trace
        .obligations
        .contains(&ModelingObligation::ExtractQuantifiers);
    let side_conditions_identified = !trace
        .obligations
        .contains(&ModelingObligation::IdentifyDomain)
        && !trace
            .obligations
            .contains(&ModelingObligation::ResolveEntityReference);
    let direct_target = matches!(
        target,
        InstantiationTargetKind::EvaluateAtArguments
            | InstantiationTargetKind::SubstituteValues
            | InstantiationTargetKind::ApplyOnce
            | InstantiationTargetKind::VerifyInstance
            | InstantiationTargetKind::DetermineWhetherConditionHolds
    );
    // These phrases commonly co-occur with a supplied equation/definition,
    // but their answers require specialist mathematics rather than one-step
    // binding.  Keeping them out of the ready bucket prevents false low-
    // distance classifications from becoming future execution routes.
    let specialist_surface = has_any(
        &source_lower,
        &[
            "asymptotic",
            "density",
            "homotopy",
            "cohomology",
            "moduli",
            "quiver",
            "eigenvalue",
            "poincare polynomial",
            "spectral norm",
            "lie algebra",
            "tropical",
            "automorphism",
            "quantum",
            "conormal",
            "natural number",
            "smallest possible",
            "minimum number",
            "rank of",
            "infimum",
            "growth rate",
        ],
    );
    let concrete_binding = has_any(
        &source_lower,
        &[
            "at x =",
            "at n =",
            "at t =",
            "evaluate at",
            "substitute",
            "plug in",
            "next term",
            "next step",
            "given x =",
            "given n =",
        ],
    );
    let one_step_representable = definition_isolated
        && target_identified
        && direct_target
        && concrete_binding
        && !specialist_surface
        && !matches!(
            supplied_object,
            SuppliedObjectKind::ExplicitTheoremStatement
                | SuppliedObjectKind::ExplicitPhysicalLaw
                | SuppliedObjectKind::ExplicitLogicalPremises
        )
        && trace.textual_attachment_reference == false;
    let verifier_available = one_step_representable
        && matches!(
            target,
            InstantiationTargetKind::EvaluateAtArguments
                | InstantiationTargetKind::SubstituteValues
                | InstantiationTargetKind::ApplyOnce
                | InstantiationTargetKind::VerifyInstance
                | InstantiationTargetKind::DetermineWhetherConditionHolds
        );
    let mut missing_representation = Vec::new();
    if !definition_isolated {
        missing_representation.push("supplied_object".into());
    }
    if !target_identified {
        missing_representation.push("target".into());
    }
    if !domain_identified {
        missing_representation.push("domain_or_side_conditions".into());
    }
    if !quantifiers_preserved {
        missing_representation.push("quantifiers".into());
    }
    if !side_conditions_identified {
        missing_representation.push("entity_or_domain_constraints".into());
    }
    if trace.textual_attachment_reference {
        missing_representation.push("visual_input".into());
    }
    let mut authorization_blockers = Vec::new();
    if !direct_target {
        authorization_blockers.push("target_not_explicit_instantiation".into());
    }
    if !concrete_binding {
        authorization_blockers.push("concrete_argument_binding_missing".into());
    }
    if specialist_surface {
        authorization_blockers.push("specialist_surface_requires_modeling".into());
    }
    if matches!(
        supplied_object,
        SuppliedObjectKind::ExplicitTheoremStatement
            | SuppliedObjectKind::ExplicitPhysicalLaw
            | SuppliedObjectKind::ExplicitLogicalPremises
    ) {
        authorization_blockers.push("method_or_premise_reasoning_required".into());
    }
    if trace.textual_attachment_reference {
        authorization_blockers.push("visual_input_unresolved".into());
    }
    if !verifier_available {
        authorization_blockers.push("independent_verifier_unavailable".into());
    }
    authorization_blockers.sort();
    authorization_blockers.dedup();
    let readiness = if matches!(trace.task_shape, TaskShape::ProveIdentity)
        || matches!(
            supplied_object,
            SuppliedObjectKind::ExplicitTheoremStatement | SuppliedObjectKind::ExplicitPhysicalLaw
        )
        || !direct_target
        || specialist_surface
        || trace.textual_attachment_reference
    {
        RepresentationReadiness::FalseDirectInstantiation
    } else if one_step_representable && verifier_available && missing_representation.is_empty() {
        RepresentationReadiness::RepresentationReady
    } else {
        RepresentationReadiness::NearReady
    };
    let false_low_distance_reason =
        if readiness == RepresentationReadiness::FalseDirectInstantiation {
            Some(if trace.textual_attachment_reference {
                FalseLowDistanceReason::VisualEvidenceRequired
            } else if matches!(trace.task_shape, TaskShape::ProveIdentity)
                || target == InstantiationTargetKind::ProveConsequence
            {
                FalseLowDistanceReason::TargetRequiresProof
            } else if specialist_surface {
                FalseLowDistanceReason::SpecializedDefinitionUnrepresented
            } else if matches!(
                supplied_object,
                SuppliedObjectKind::ExplicitTheoremStatement
                    | SuppliedObjectKind::ExplicitPhysicalLaw
                    | SuppliedObjectKind::ExplicitLogicalPremises
            ) {
                FalseLowDistanceReason::TargetRequiresTheoremSelection
            } else if !direct_target {
                FalseLowDistanceReason::TargetRequiresModelConstruction
            } else if !concrete_binding {
                FalseLowDistanceReason::DefinitionMentionedButNotOperational
            } else if !definition_isolated {
                FalseLowDistanceReason::SuppliedObjectIncomplete
            } else if !quantifiers_preserved {
                FalseLowDistanceReason::QuantifiedStructureUnresolved
            } else {
                FalseLowDistanceReason::SurfaceRuleNotApplicable
            })
        } else {
            None
        };
    // This is intentionally a lower bound, not a best guess.  A direct
    // prediction with unresolved target/modeling evidence is at least one
    // modeling step away; specialist/proof cases are higher still.  Runtime
    // authorization must use this field rather than the surface classifier's
    // original distance.
    let conservative_lower_bound = if readiness == RepresentationReadiness::RepresentationReady {
        ModelingDistance::DirectInstantiation
    } else if matches!(trace.task_shape, TaskShape::ProveIdentity)
        || target == InstantiationTargetKind::ProveConsequence
    {
        ModelingDistance::SpecialistReasoning
    } else if specialist_surface
        || matches!(
            supplied_object,
            SuppliedObjectKind::ExplicitTheoremStatement
                | SuppliedObjectKind::ExplicitPhysicalLaw
                | SuppliedObjectKind::ExplicitLogicalPremises
        )
    {
        ModelingDistance::MethodSelection
    } else {
        ModelingDistance::OneModelingStep
    };
    DirectInstantiationAssessment {
        question_id: trace.question_id.clone(),
        supplied_object,
        target,
        readiness,
        conservative_lower_bound,
        definition_isolated,
        binders_identified,
        domain_identified,
        target_identified,
        quantifiers_preserved,
        side_conditions_identified,
        one_step_representable,
        verifier_available,
        missing_representation,
        authorization_blockers,
        false_low_distance_reason,
    }
}

trait TraceQuestionSource {
    fn target_source(&self) -> String;
}

impl TraceQuestionSource for FormalizationTrace {
    fn target_source(&self) -> String {
        self.target
            .as_ref()
            .map(|target| target.source_fragment.clone())
            .or_else(|| self.facts.first().map(|fact| fact.source_fragment.clone()))
            .unwrap_or_default()
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
    let mut input_dependencies = BTreeSet::from([InputDependency::TextOnly]);
    if textual_attachment_reference {
        input_dependencies.remove(&InputDependency::TextOnly);
        if has_image {
            input_dependencies.insert(InputDependency::Image);
        } else {
            input_dependencies.insert(InputDependency::MissingAttachment);
        }
        if has_any(&lower, &["diagram", "figure", "pictured", "shown below"]) {
            input_dependencies.insert(InputDependency::Diagram);
        }
        if has_any(&lower, &["table", "tabulated"]) {
            input_dependencies.insert(InputDependency::Table);
        }
        if has_any(&lower, &["graph", "plot", "axis", "axes"]) {
            input_dependencies.insert(InputDependency::Graph);
        }
    }
    if textual_attachment_reference {
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
    // The benchmark's `has_image` flag is metadata, not proof that the
    // question's reasoning depends on an image.  Keep attachment need on the
    // dependency axis; only textual visual references affect this status.
    let status = if textual_attachment_reference {
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
        input_dependencies,
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
        assert!(trace.input_dependencies.contains(&InputDependency::Image));
        assert!(trace.input_dependencies.contains(&InputDependency::Diagram));
    }

    #[test]
    fn image_metadata_without_visual_reference_does_not_change_distance_dependency() {
        let trace = assess_prompt("q", "Solve for x: 3x + 2 = 11, for real x.", "Math", true);
        assert_eq!(
            trace.modeling_distance,
            ModelingDistance::DirectInstantiation
        );
        assert_eq!(trace.status, FormalizationStatus::Structured);
        assert_eq!(
            trace.input_dependencies,
            BTreeSet::from([InputDependency::TextOnly])
        );
    }

    #[test]
    fn direct_audit_rejects_clinical_next_step_as_rule_application() {
        let trace = assess_prompt(
            "q",
            "The patient has a rash. What is the next step in management?",
            "Biology/Medicine",
            false,
        );
        let audit = assess_direct_instantiation(&trace);
        assert_eq!(audit.target, InstantiationTargetKind::Other);
        assert_eq!(
            audit.readiness,
            RepresentationReadiness::FalseDirectInstantiation
        );
    }

    #[test]
    fn direct_audit_requires_concrete_binding_and_verifier() {
        let trace = assess_prompt(
            "q",
            "Let f(x) = x + 1 for real x. Evaluate at x = 2.",
            "Math",
            false,
        );
        let audit = assess_direct_instantiation(&trace);
        assert_eq!(
            audit.supplied_object,
            SuppliedObjectKind::ExplicitDefinition
        );
        assert_eq!(audit.target, InstantiationTargetKind::EvaluateAtArguments);
        assert_eq!(
            audit.readiness,
            RepresentationReadiness::RepresentationReady
        );
        assert!(audit.verifier_available);
        assert!(audit.authorization_safe());
        assert_eq!(
            audit.conservative_lower_bound,
            ModelingDistance::DirectInstantiation
        );
        assert_eq!(audit.false_low_distance_reason, None);
    }

    #[test]
    fn direct_audit_exposes_conservative_lower_bound_for_specialist_target() {
        let trace = assess_prompt(
            "q",
            "Given f(x)=x^2, determine the asymptotic growth rate of the sequence.",
            "Math",
            false,
        );
        let audit = assess_direct_instantiation(&trace);
        assert_eq!(
            audit.readiness,
            RepresentationReadiness::FalseDirectInstantiation
        );
        assert!(audit.conservative_lower_bound >= ModelingDistance::MethodSelection);
        assert!(audit.false_low_distance_reason.is_some());
        assert!(!audit.authorization_safe());
    }

    #[test]
    fn gold_case_validation_requires_evidence_for_authorization() {
        let case = FormalizationGoldCase {
            id: "tier1-f".into(),
            tier: FormalizationTier::DirectInstantiation,
            transformation: FormalizationTransformation::InstantiateDefinition,
            prompt: "Let f(x)=x+1. Evaluate f(2).".into(),
            definitions: vec![DefinitionKind::Function],
            entities: vec![],
            facts: vec![],
            target: TargetAnnotation {
                statement: "evaluate f at 2".into(),
                source_fragment: "Evaluate f(2)".into(),
            },
            assumptions: vec![],
            constraints: vec![],
            obligations: vec![],
            provenance_spans: vec![ProvenanceSpan {
                role: "definition_and_target".into(),
                source_fragment: "Let f(x)=x+1. Evaluate f(2).".into(),
            }],
            authorization_expected: true,
            allowed_methods: vec!["definition_application".into()],
            expected_answer: Some("3".into()),
        };
        assert!(case.is_valid());
    }

    #[test]
    fn corpus_rejects_duplicate_ids_and_unsupported_schema() {
        let target = TargetAnnotation {
            statement: "compute".into(),
            source_fragment: "compute".into(),
        };
        let make_case = || FormalizationGoldCase {
            id: "duplicate".into(),
            tier: FormalizationTier::ExplicitObject,
            transformation: FormalizationTransformation::ExtractExplicitEquation,
            prompt: "x=1".into(),
            definitions: vec![],
            entities: vec![],
            facts: vec![],
            target: target.clone(),
            assumptions: vec![],
            constraints: vec![],
            obligations: vec![],
            provenance_spans: vec![ProvenanceSpan {
                role: "equation".into(),
                source_fragment: "x=1".into(),
            }],
            authorization_expected: false,
            allowed_methods: vec![],
            expected_answer: None,
        };
        let corpus = FormalizationCorpus {
            schema_version: 99,
            cases: vec![make_case(), make_case()],
        };
        let errors = corpus.validation_errors();
        assert!(errors
            .iter()
            .any(|e| e.contains("unsupported schema_version")));
        assert!(errors.iter().any(|e| e.contains("duplicate case id")));
    }

    #[test]
    fn field_score_keeps_authorization_separate_from_extraction() {
        let gold = FormalizationGoldCase {
            id: "score-1".into(),
            tier: FormalizationTier::DirectInstantiation,
            transformation: FormalizationTransformation::InstantiateDefinition,
            prompt: "Let f(x)=x+1. Evaluate f(2).".into(),
            definitions: vec![DefinitionKind::Function],
            entities: vec![],
            facts: vec![],
            target: TargetAnnotation {
                statement: "evaluate f at 2".into(),
                source_fragment: "Evaluate f(2)".into(),
            },
            assumptions: vec![],
            constraints: vec![],
            obligations: vec![],
            provenance_spans: vec![ProvenanceSpan {
                role: "definition_and_target".into(),
                source_fragment: "Let f(x)=x+1. Evaluate f(2).".into(),
            }],
            authorization_expected: true,
            allowed_methods: vec!["definition_application".into()],
            expected_answer: Some("3".into()),
        };
        let trace = assess_prompt("score-1", &gold.prompt, "Math", false);
        let score = score_formalization(&gold, &trace, false);
        assert_eq!(score.definitions.recall, 1.0);
        assert!(!score.authorization_correct);
    }
}
