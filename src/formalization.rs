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

/// Gate-by-gate explanation for a denied direct-instantiation attempt.  This
/// is diagnostic only; it does not relax `authorization_safe()`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct AuthorizationDenialTrace {
    pub case_id: String,
    pub gold_should_authorize: bool,
    pub target_complete: bool,
    pub representation_complete: bool,
    pub bindings_complete: bool,
    pub constraints_complete: bool,
    pub operation_supported: bool,
    pub verification_available: bool,
    pub first_blocker: String,
    pub all_blockers: Vec<String>,
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

    pub fn denial_trace(&self, gold_should_authorize: bool) -> AuthorizationDenialTrace {
        let target_complete = self.target_identified
            && !self.authorization_blockers.iter().any(|b| {
                b == "target_not_explicit_instantiation" || b == "concrete_argument_binding_missing"
            });
        let representation_complete = self.missing_representation.is_empty();
        let bindings_complete = self.binders_identified
            && !self
                .authorization_blockers
                .iter()
                .any(|b| b == "concrete_argument_binding_missing");
        let constraints_complete = self.side_conditions_identified;
        let operation_supported = self.one_step_representable;
        let verification_available = self.verifier_available;
        let mut all_blockers = self.authorization_blockers.clone();
        if !target_complete {
            all_blockers.push("target_incomplete".into());
        }
        if !representation_complete {
            all_blockers.push("representation_incomplete".into());
        }
        if !bindings_complete {
            all_blockers.push("bindings_incomplete".into());
        }
        if !constraints_complete {
            all_blockers.push("constraints_incomplete".into());
        }
        if !operation_supported {
            all_blockers.push("operation_unsupported".into());
        }
        if !verification_available {
            all_blockers.push("verification_unavailable".into());
        }
        all_blockers.sort();
        all_blockers.dedup();
        let first_blocker = if !target_complete {
            "target_incomplete"
        } else if !representation_complete {
            "representation_incomplete"
        } else if !bindings_complete {
            "bindings_incomplete"
        } else if !constraints_complete {
            "constraints_incomplete"
        } else if !operation_supported {
            "operation_unsupported"
        } else if !verification_available {
            "verification_unavailable"
        } else {
            "authorization_contract_or_lower_bound"
        };
        AuthorizationDenialTrace {
            case_id: self.question_id.clone(),
            gold_should_authorize,
            target_complete,
            representation_complete,
            bindings_complete,
            constraints_complete,
            operation_supported,
            verification_available,
            first_blocker: first_blocker.into(),
            all_blockers,
        }
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
    pub target_comparison: TargetComparison,
    /// Structural target agreement is deliberately weaker than textual
    /// equality: a trace may include the full prompt sentence while the gold
    /// target stores only the requested operation.
    pub target_structural: bool,
    pub authorization_correct: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TargetComparison {
    pub kind_matches: bool,
    pub subject_overlap: bool,
    pub semantically_equivalent: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OperationKind {
    Evaluate,
    Substitute,
    Solve,
    Simplify,
    Compare,
    InstantiateDefinition,
    Verify,
    Prove,
    Count,
    Unknown,
}

impl OperationKind {
    pub fn label(self) -> &'static str {
        match self {
            Self::Evaluate => "evaluate",
            Self::Substitute => "substitute",
            Self::Solve => "solve",
            Self::Simplify => "simplify",
            Self::Compare => "compare",
            Self::InstantiateDefinition => "instantiate_definition",
            Self::Verify => "verify",
            Self::Prove => "prove",
            Self::Count => "count",
            Self::Unknown => "unknown",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OperationStatus {
    Recognized(OperationKind),
    Ambiguous(Vec<OperationKind>),
    Unsupported(String),
    NotIdentified,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TextSpan {
    pub source_fragment: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TargetProvenance {
    pub operation_span: Option<TextSpan>,
    pub subject_span: Option<TextSpan>,
    pub variable_spans: Vec<TextSpan>,
    pub argument_spans: Vec<TextSpan>,
    pub domain_span: Option<TextSpan>,
    pub answer_form_span: Option<TextSpan>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OperationFrame {
    Evaluate {
        expression: String,
        bindings: Vec<TargetArgumentBinding>,
    },
    Simplify {
        expression: String,
    },
    Solve {
        relation: String,
        variables: Vec<String>,
        domain: Option<String>,
    },
    Compare {
        subject: String,
    },
    Substitute {
        subject: String,
        bindings: Vec<TargetArgumentBinding>,
    },
    InstantiateDefinition {
        definition: String,
        arguments: Vec<TargetArgumentBinding>,
        requested_property: Option<String>,
    },
    Unsupported {
        requested: String,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SubjectObjectType {
    Expression,
    Relation,
    Function,
    Definition,
    Comparison,
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SubjectCandidate {
    pub object_id: String,
    pub object: String,
    pub object_type: SubjectObjectType,
    pub source_spans: Vec<TextSpan>,
    pub referenced_by_target: bool,
    pub definition_available: bool,
    pub evidence: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SubjectGap {
    NoCandidateObject,
    MultipleCandidateObjects,
    ObjectMentionedButUndefined,
    DefinitionNotLinked,
    WrongObjectType,
    ObjectExistsButTargetDoesNotReferenceIt,
    CompoundObjectUnsupported,
    ScopeResolutionFailure,
}

impl SubjectGap {
    pub fn label(self) -> &'static str {
        match self {
            Self::NoCandidateObject => "no_candidate_object",
            Self::MultipleCandidateObjects => "multiple_candidate_objects",
            Self::ObjectMentionedButUndefined => "object_mentioned_but_undefined",
            Self::DefinitionNotLinked => "definition_not_linked",
            Self::WrongObjectType => "wrong_object_type",
            Self::ObjectExistsButTargetDoesNotReferenceIt => {
                "object_exists_but_target_does_not_reference_it"
            }
            Self::CompoundObjectUnsupported => "compound_object_unsupported",
            Self::ScopeResolutionFailure => "scope_resolution_failure",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SubjectResolution {
    pub selected: Option<SubjectCandidate>,
    pub alternatives: Vec<SubjectCandidate>,
    pub blockers: Vec<SubjectGap>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TargetFieldStatus {
    Complete,
    Missing,
    Ambiguous,
    NotRequired,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum BindingStatus {
    Complete,
    Missing(Vec<String>),
    Ambiguous(Vec<String>),
    Conflicting(Vec<String>),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TargetCompleteness {
    pub operation_kind: TargetFieldStatus,
    pub subject: TargetFieldStatus,
    pub target_variable: TargetFieldStatus,
    pub arguments: TargetFieldStatus,
    pub domain: TargetFieldStatus,
    pub requested_form: TargetFieldStatus,
    pub provenance: TargetFieldStatus,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AnswerForm {
    ExactValue,
    Approximation,
    SolutionSet,
    SingleSelectedSolution,
    Proof,
    Counterexample,
    SimplifiedExpression,
    ComparisonResult,
    Classification,
    Explanation,
}

impl AnswerForm {
    pub fn label(self) -> &'static str {
        match self {
            Self::ExactValue => "exact_value",
            Self::Approximation => "approximation",
            Self::SolutionSet => "solution_set",
            Self::SingleSelectedSolution => "single_selected_solution",
            Self::Proof => "proof",
            Self::Counterexample => "counterexample",
            Self::SimplifiedExpression => "simplified_expression",
            Self::ComparisonResult => "comparison_result",
            Self::Classification => "classification",
            Self::Explanation => "explanation",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TargetBuildTrace {
    pub operation: OperationStatus,
    pub subject: TargetFieldStatus,
    pub binding_status: BindingStatus,
    pub bindings: TargetFieldStatus,
    pub requested_form: TargetFieldStatus,
    pub provenance: TargetFieldStatus,
    pub final_status: TargetStatus,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum TargetStatus {
    Complete,
    Incomplete(Vec<String>),
    Ambiguous(Vec<String>),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TargetArgumentBinding {
    pub parameter: String,
    pub value: String,
    pub provenance: String,
    pub status: TargetFieldStatus,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FormalizedTarget {
    pub operation: OperationKind,
    pub operation_status: OperationStatus,
    pub frame: Option<OperationFrame>,
    pub subject: Option<String>,
    pub subject_resolution: SubjectResolution,
    pub target_variable: Option<String>,
    pub arguments: Vec<TargetArgumentBinding>,
    pub domain: Option<String>,
    pub requested_form: Option<String>,
    pub answer_form: Option<AnswerForm>,
    pub provenance: Option<TargetProvenance>,
    pub completeness: TargetCompleteness,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OperationCapability {
    pub operation: OperationKind,
    pub executor_available: bool,
    pub verifier_available: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TargetCompletion {
    pub target: FormalizedTarget,
    pub reasons: Vec<String>,
    pub operation_supported: bool,
    pub verifier_available: bool,
    pub complete: bool,
    pub build_trace: TargetBuildTrace,
}

pub fn operation_capability(operation: OperationKind) -> OperationCapability {
    let supported = matches!(
        operation,
        OperationKind::Evaluate
            | OperationKind::Substitute
            | OperationKind::Solve
            | OperationKind::Simplify
            | OperationKind::Compare
    );
    OperationCapability {
        operation,
        executor_available: supported,
        verifier_available: supported,
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub enum FormalizedFact {
    Equation {
        lhs: String,
        relation: String,
        rhs: String,
        source_fragment: String,
    },
    Expression {
        expression: String,
        source_fragment: String,
    },
    LogicalPremise {
        statement: String,
        source_fragment: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub enum FormalizedConstraint {
    DomainOrSideCondition {
        statement: String,
        source_fragment: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub enum CompletenessStatus {
    Complete,
    Incomplete(Vec<ModelingObligation>),
    Ambiguous(Vec<String>),
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

fn target_kind(value: &str) -> Option<&'static str> {
    let text = normalized_text(value);
    [
        ("solve", "solve"),
        ("evaluate", "evaluate"),
        ("compute", "compute"),
        ("calculate", "calculate"),
        ("find", "find"),
        ("determine", "determine"),
        ("simplify", "simplify"),
        ("compare", "compare"),
        ("prove", "prove"),
        ("show", "prove"),
        ("check", "verify"),
        ("verify", "verify"),
    ]
    .iter()
    .find_map(|(needle, kind)| text.contains(needle).then_some(*kind))
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
    let kind_matches = target_kind(&gold.target.statement) == target_kind(predicted_target);
    let subject_overlap = gold_tokens
        .intersection(&predicted_tokens)
        .any(|token| !["the", "for", "at", "of", "and", "is"].contains(&token.as_str()));
    let target_comparison = TargetComparison {
        kind_matches,
        subject_overlap,
        semantically_equivalent: kind_matches && subject_overlap,
    };
    let target_structural = target_comparison.semantically_equivalent;
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
        target_comparison,
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
    pub formalized_facts: Vec<FormalizedFact>,
    pub facts_completeness: CompletenessStatus,
    pub target: Option<TargetAnnotation>,
    pub formalized_target: FormalizedTarget,
    pub target_completion: TargetCompletion,
    pub assumptions: Vec<AssumptionAnnotation>,
    pub constraints: Vec<ConstraintAnnotation>,
    pub formalized_constraints: Vec<FormalizedConstraint>,
    pub constraints_completeness: CompletenessStatus,
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

fn extract_explicit_relation(question: &str) -> Option<(String, String, String)> {
    let relation = Regex::new(
        r"(?i)([A-Za-z_][A-Za-z0-9_()^*/+\-. ]*?)\s*(<=|>=|=|<|>)\s*([A-Za-z0-9_()^*/+\-. ]+)",
    )
    .expect("static relation regex");
    let captures = relation.captures(question)?;
    let mut lhs = captures.get(1)?.as_str().trim().to_string();
    let operator = captures.get(2)?.as_str().to_string();
    let rhs = captures
        .get(3)?
        .as_str()
        .trim()
        .split([',', '?', ';'])
        .next()
        .unwrap_or_default()
        .trim()
        .trim_end_matches(" for real x")
        .trim_end_matches(" for x")
        .trim_end_matches('.')
        .to_string();
    for prefix in [
        "given ",
        "let ",
        "if ",
        "where ",
        "solve ",
        "the equation ",
        "for real x, ",
    ] {
        if lhs.to_ascii_lowercase().starts_with(prefix) {
            lhs = lhs[prefix.len()..].trim().to_string();
        }
    }
    if lhs.is_empty() || rhs.is_empty() {
        None
    } else {
        Some((lhs, operator, rhs))
    }
}

fn extract_expression_payload(question: &str) -> Option<String> {
    let lower = question.to_ascii_lowercase();
    for verb in [
        "evaluate ",
        "compute ",
        "calculate ",
        "simplify ",
        "compare ",
    ] {
        if let Some(start) = lower.find(verb) {
            let payload = question[start + verb.len()..]
                .split(['.', '?', '\n'])
                .next()
                .unwrap_or_default()
                .trim()
                .trim_end_matches(" exactly")
                .trim();
            if !payload.is_empty() {
                return Some(payload.to_string());
            }
        }
    }
    None
}

fn formalized_facts(facts: &[FactAnnotation], question: &str) -> Vec<FormalizedFact> {
    if let Some((lhs, relation, rhs)) = extract_explicit_relation(question) {
        return vec![FormalizedFact::Equation {
            lhs,
            relation,
            rhs,
            source_fragment: question.into(),
        }];
    }
    if let Some(expression) = extract_expression_payload(question) {
        return vec![FormalizedFact::Expression {
            expression,
            source_fragment: question.into(),
        }];
    }
    facts
        .iter()
        .map(|fact| {
            if fact.statement.contains("quantified") {
                FormalizedFact::LogicalPremise {
                    statement: fact.statement.clone(),
                    source_fragment: fact.source_fragment.clone(),
                }
            } else {
                FormalizedFact::Expression {
                    expression: fact.statement.clone(),
                    source_fragment: fact.source_fragment.clone(),
                }
            }
        })
        .collect()
}

fn completeness_for(
    obligations: &[ModelingObligation],
    relevant: &[ModelingObligation],
) -> CompletenessStatus {
    let unresolved: Vec<_> = relevant
        .iter()
        .copied()
        .filter(|obligation| obligations.contains(obligation))
        .collect();
    if unresolved.is_empty() {
        CompletenessStatus::Complete
    } else {
        CompletenessStatus::Incomplete(unresolved)
    }
}

fn operation_from_text(text: &str) -> OperationKind {
    let lower = text.to_ascii_lowercase();
    if lower.contains("prove") || lower.contains("show that") {
        OperationKind::Prove
    } else if lower.contains("how many") || lower.contains("count") {
        OperationKind::Count
    } else if lower.contains("simplify") {
        OperationKind::Simplify
    } else if lower.contains("substitute") || lower.contains("plug in") {
        OperationKind::Substitute
    } else if lower.contains("using the definition")
        || lower.contains("according to the definition")
        || lower.contains("what does ") && lower.contains(" return")
    {
        OperationKind::InstantiateDefinition
    } else if lower.contains("solve")
        || lower.contains("find the solution")
        || lower.contains("which values")
        || lower.contains("for which")
        || lower.contains("find all roots")
        || lower.contains("find the roots")
    {
        OperationKind::Solve
    } else if lower.contains("evaluate") || lower.contains("compute") || lower.contains("calculate")
    {
        OperationKind::Evaluate
    } else if lower.contains("compare") || lower.contains("equivalent") || lower.contains("same as")
    {
        OperationKind::Compare
    } else if lower.contains("verify")
        || lower.contains("check whether")
        || lower.contains("check if")
    {
        OperationKind::Verify
    } else {
        OperationKind::Unknown
    }
}

pub fn infer_answer_form(text: &str, operation: OperationKind) -> Option<AnswerForm> {
    let lower = text.to_ascii_lowercase();
    if lower.contains("prove") || lower.contains("show that") {
        Some(AnswerForm::Proof)
    } else if lower.contains("counterexample") {
        Some(AnswerForm::Counterexample)
    } else if lower.contains("positive solution")
        || lower.contains("negative solution")
        || lower.contains("single solution")
    {
        Some(AnswerForm::SingleSelectedSolution)
    } else if lower.contains("all solutions")
        || lower.contains("all roots")
        || lower.contains("solution set")
    {
        Some(AnswerForm::SolutionSet)
    } else if lower.contains("approx") || lower.contains("decimal") {
        Some(AnswerForm::Approximation)
    } else if lower.contains("factor") || lower.contains("simplif") {
        Some(AnswerForm::SimplifiedExpression)
    } else if matches!(operation, OperationKind::Compare) || lower.contains("equivalent") {
        Some(AnswerForm::ComparisonResult)
    } else if lower.contains("classif") || lower.contains("whether") {
        Some(AnswerForm::Classification)
    } else if matches!(operation, OperationKind::Prove) {
        Some(AnswerForm::Proof)
    } else if matches!(operation, OperationKind::Evaluate | OperationKind::Solve) {
        Some(AnswerForm::ExactValue)
    } else {
        None
    }
}

fn resolve_subject(
    question: &str,
    target_text: &str,
    formalized_facts: &[FormalizedFact],
) -> SubjectResolution {
    let mut candidates = Vec::new();
    let function_definition =
        Regex::new(r"(?i)\b([A-Za-z_][A-Za-z0-9_]*)\s*\([^)]*\)\s*=\s*([^.;?]+)")
            .expect("static function-definition regex");
    for capture in function_definition.captures_iter(question) {
        let Some(name) = capture.get(1) else { continue };
        let Some(body) = capture.get(0) else { continue };
        candidates.push(SubjectCandidate {
            object_id: name.as_str().to_string(),
            object: body.as_str().trim().to_string(),
            object_type: SubjectObjectType::Function,
            source_spans: vec![TextSpan {
                source_fragment: body.as_str().to_string(),
            }],
            referenced_by_target: target_text
                .to_ascii_lowercase()
                .contains(&format!("{}(", name.as_str().to_ascii_lowercase())),
            definition_available: true,
            evidence: "explicit_function_definition".into(),
        });
    }
    for (index, fact) in formalized_facts.iter().enumerate() {
        let (object, object_type) = match fact {
            FormalizedFact::Equation {
                lhs, relation, rhs, ..
            } => (
                format!("{lhs} {relation} {rhs}"),
                SubjectObjectType::Relation,
            ),
            FormalizedFact::Expression { expression, .. } => {
                (expression.clone(), SubjectObjectType::Expression)
            }
            FormalizedFact::LogicalPremise { statement, .. } => {
                (statement.clone(), SubjectObjectType::Definition)
            }
        };
        if candidates
            .iter()
            .any(|candidate| candidate.object == object)
        {
            continue;
        }
        let referenced = target_text
            .split_whitespace()
            .any(|token| object.contains(token.trim_matches(|c: char| !c.is_alphanumeric())));
        candidates.push(SubjectCandidate {
            object_id: format!("fact_{index}"),
            object: object.clone(),
            object_type,
            source_spans: vec![TextSpan {
                source_fragment: object,
            }],
            referenced_by_target: referenced,
            definition_available: true,
            evidence: "typed_fact".into(),
        });
    }

    let target_lower = target_text.to_ascii_lowercase();
    let function_reference_without_definition = Regex::new(r"\b([A-Za-z_][A-Za-z0-9_]*)\s*\(")
        .expect("static function-reference regex")
        .captures_iter(target_text)
        .map(|capture| capture.get(1).unwrap().as_str().to_ascii_lowercase())
        .find(|name| {
            !candidates
                .iter()
                .any(|candidate| candidate.object_id.to_ascii_lowercase() == *name)
        });
    if function_reference_without_definition.is_some() {
        return SubjectResolution {
            selected: None,
            alternatives: candidates,
            blockers: vec![SubjectGap::ObjectMentionedButUndefined],
        };
    }

    let referenced_functions: Vec<_> = candidates
        .iter()
        .filter(|candidate| {
            candidate.object_type == SubjectObjectType::Function && candidate.referenced_by_target
        })
        .cloned()
        .collect();
    if referenced_functions.len() == 1 {
        return SubjectResolution {
            selected: referenced_functions.into_iter().next(),
            alternatives: candidates,
            blockers: Vec::new(),
        };
    }

    let referenced: Vec<_> = candidates
        .iter()
        .filter(|candidate| candidate.referenced_by_target)
        .cloned()
        .collect();
    if referenced.len() == 1 {
        return SubjectResolution {
            selected: referenced.into_iter().next(),
            alternatives: candidates,
            blockers: Vec::new(),
        };
    }
    if referenced.len() > 1 {
        return SubjectResolution {
            selected: None,
            alternatives: referenced,
            blockers: vec![SubjectGap::MultipleCandidateObjects],
        };
    }
    if candidates.len() == 1 {
        let candidate = candidates.into_iter().next().unwrap();
        if target_lower.contains("solve")
            || target_lower.contains("evaluate")
            || target_lower.contains("simplify")
            || target_lower.contains("substitute")
        {
            return SubjectResolution {
                selected: Some(candidate),
                alternatives: Vec::new(),
                blockers: Vec::new(),
            };
        }
        return SubjectResolution {
            selected: None,
            alternatives: vec![candidate],
            blockers: vec![SubjectGap::ObjectExistsButTargetDoesNotReferenceIt],
        };
    }
    if candidates.is_empty() {
        SubjectResolution {
            selected: None,
            alternatives: Vec::new(),
            blockers: vec![SubjectGap::NoCandidateObject],
        }
    } else {
        SubjectResolution {
            selected: None,
            alternatives: candidates,
            blockers: vec![SubjectGap::MultipleCandidateObjects],
        }
    }
}

fn build_target_completion(
    question: &str,
    target: Option<&TargetAnnotation>,
    formalized_facts: &[FormalizedFact],
) -> TargetCompletion {
    let target_text = target
        .map(|value| value.statement.as_str())
        .unwrap_or_default();
    let subject_resolution = resolve_subject(question, target_text, formalized_facts);
    let subject = subject_resolution
        .selected
        .as_ref()
        .map(|candidate| candidate.object.clone());
    let mut operation = operation_from_text(target_text);
    if operation == OperationKind::Unknown
        && (target_text.to_ascii_lowercase().contains("find")
            || target_text.to_ascii_lowercase().contains("what is"))
    {
        operation = if subject
            .as_ref()
            .map(|value| value.contains('='))
            .unwrap_or(false)
        {
            OperationKind::Solve
        } else {
            OperationKind::Evaluate
        };
    }
    let explicit_target_variable = Regex::new(r"(?i)\bsolve\s+for\s+([A-Za-z_][A-Za-z0-9_]*)")
        .expect("static target variable regex")
        .captures(target_text)
        .and_then(|captures| captures.get(1).map(|value| value.as_str().to_string()));
    let target_variable = explicit_target_variable.or_else(|| {
        if operation != OperationKind::Solve {
            return None;
        }
        let symbols = Regex::new(r"\b[A-Za-z_][A-Za-z0-9_]*\b")
            .expect("static symbol regex")
            .find_iter(subject.as_deref().unwrap_or_default())
            .map(|value| value.as_str().to_string())
            .filter(|symbol| {
                !matches!(
                    symbol.as_str(),
                    "e" | "and"
                        | "or"
                        | "give"
                        | "the"
                        | "positive"
                        | "negative"
                        | "solution"
                        | "solutions"
                        | "find"
                        | "for"
                        | "which"
                )
            })
            .collect::<BTreeSet<_>>();
        (symbols.len() == 1).then(|| symbols.into_iter().next().unwrap())
    });
    let mut arguments = Vec::new();
    if let Some(captures) = Regex::new(r"\b([A-Za-z_][A-Za-z0-9_]*)\(([^()]*)\)")
        .expect("static function argument regex")
        .captures(target_text)
    {
        let value = captures
            .get(2)
            .map(|v| v.as_str().trim())
            .unwrap_or_default();
        arguments.push(TargetArgumentBinding {
            parameter: "argument".into(),
            value: value.into(),
            provenance: target
                .map(|v| v.source_fragment.clone())
                .unwrap_or_default(),
            status: if value.is_empty() {
                TargetFieldStatus::Missing
            } else {
                TargetFieldStatus::Complete
            },
        });
    }
    if let Some(captures) = Regex::new(r"\bat\s+([A-Za-z_][A-Za-z0-9_]*)\s*=\s*([^,?.]+)")
        .expect("static at-argument regex")
        .captures(target_text)
    {
        arguments.push(TargetArgumentBinding {
            parameter: captures.get(1).unwrap().as_str().into(),
            value: captures.get(2).unwrap().as_str().trim().into(),
            provenance: target
                .map(|v| v.source_fragment.clone())
                .unwrap_or_default(),
            status: TargetFieldStatus::Complete,
        });
    }
    let lower = target_text.to_ascii_lowercase();
    let answer_form = infer_answer_form(target_text, operation);
    let domain = ["real", "integer", "natural", "positive", "complex"]
        .iter()
        .find(|word| lower.contains(**word))
        .map(|word| (*word).to_string());
    let requested_form = ["exact", "positive", "all", "approximate", "inequality"]
        .iter()
        .find(|word| lower.contains(**word))
        .map(|word| (*word).to_string());
    let subject_status = if subject.is_some() {
        TargetFieldStatus::Complete
    } else if subject_resolution
        .blockers
        .contains(&SubjectGap::MultipleCandidateObjects)
    {
        TargetFieldStatus::Ambiguous
    } else {
        TargetFieldStatus::Missing
    };
    let requires_arguments = match operation {
        OperationKind::Substitute | OperationKind::InstantiateDefinition => true,
        OperationKind::Evaluate => {
            target_text.contains('(')
                || subject
                    .as_deref()
                    .map(|value| {
                        Regex::new(r"\b[A-Za-z_][A-Za-z0-9_]*\b")
                            .expect("static free-variable regex")
                            .is_match(value)
                    })
                    .unwrap_or(false)
        }
        _ => false,
    };
    let arguments_status = if !requires_arguments {
        TargetFieldStatus::NotRequired
    } else if arguments.is_empty() {
        TargetFieldStatus::Missing
    } else if arguments
        .iter()
        .any(|argument| argument.status == TargetFieldStatus::Ambiguous)
    {
        TargetFieldStatus::Ambiguous
    } else if arguments
        .iter()
        .all(|argument| argument.status == TargetFieldStatus::Complete)
    {
        TargetFieldStatus::Complete
    } else {
        TargetFieldStatus::Missing
    };
    let completeness = TargetCompleteness {
        operation_kind: if operation == OperationKind::Unknown {
            TargetFieldStatus::Missing
        } else {
            TargetFieldStatus::Complete
        },
        subject: subject_status,
        target_variable: if operation == OperationKind::Solve {
            if target_variable.is_some() {
                TargetFieldStatus::Complete
            } else {
                TargetFieldStatus::Missing
            }
        } else {
            TargetFieldStatus::NotRequired
        },
        arguments: arguments_status,
        domain: if operation == OperationKind::Solve && domain.is_some() {
            TargetFieldStatus::Complete
        } else {
            TargetFieldStatus::NotRequired
        },
        requested_form: if requested_form.is_some() || answer_form.is_some() {
            TargetFieldStatus::Complete
        } else {
            TargetFieldStatus::NotRequired
        },
        provenance: if target.is_some() {
            TargetFieldStatus::Complete
        } else {
            TargetFieldStatus::Missing
        },
    };
    let operation_status = match operation {
        OperationKind::Unknown => OperationStatus::NotIdentified,
        OperationKind::Prove | OperationKind::Count => {
            OperationStatus::Unsupported(operation.label().to_string())
        }
        recognized => OperationStatus::Recognized(recognized),
    };
    let frame = match operation {
        OperationKind::Evaluate => subject.clone().map(|expression| OperationFrame::Evaluate {
            expression,
            bindings: arguments.clone(),
        }),
        OperationKind::Simplify => subject
            .clone()
            .map(|expression| OperationFrame::Simplify { expression }),
        OperationKind::Solve => subject.clone().map(|relation| OperationFrame::Solve {
            relation,
            variables: target_variable.clone().into_iter().collect(),
            domain: domain.clone(),
        }),
        OperationKind::Compare => subject
            .clone()
            .map(|subject| OperationFrame::Compare { subject }),
        OperationKind::Substitute => subject.clone().map(|subject| OperationFrame::Substitute {
            subject,
            bindings: arguments.clone(),
        }),
        OperationKind::InstantiateDefinition => {
            subject
                .clone()
                .map(|definition| OperationFrame::InstantiateDefinition {
                    definition,
                    arguments: arguments.clone(),
                    requested_property: answer_form.map(|form| form.label().to_string()),
                })
        }
        OperationKind::Prove | OperationKind::Count => Some(OperationFrame::Unsupported {
            requested: target_text.into(),
        }),
        OperationKind::Verify | OperationKind::Unknown => None,
    };
    let provenance = target.map(|value| TargetProvenance {
        operation_span: Some(TextSpan {
            source_fragment: value.statement.clone(),
        }),
        subject_span: subject.as_ref().map(|subject| TextSpan {
            source_fragment: subject.clone(),
        }),
        variable_spans: target_variable
            .as_ref()
            .map(|variable| {
                vec![TextSpan {
                    source_fragment: variable.clone(),
                }]
            })
            .unwrap_or_default(),
        argument_spans: arguments
            .iter()
            .map(|argument| TextSpan {
                source_fragment: argument.provenance.clone(),
            })
            .collect(),
        domain_span: domain.as_ref().map(|value| TextSpan {
            source_fragment: value.clone(),
        }),
        answer_form_span: requested_form.as_ref().map(|value| TextSpan {
            source_fragment: value.clone(),
        }),
    });
    let capability = operation_capability(operation);
    let mut reasons = Vec::new();
    for (name, status) in [
        ("operation_kind", completeness.operation_kind),
        ("subject", completeness.subject),
        ("target_variable", completeness.target_variable),
        ("arguments", completeness.arguments),
        ("domain", completeness.domain),
        ("requested_form", completeness.requested_form),
        ("provenance", completeness.provenance),
    ] {
        if matches!(
            status,
            TargetFieldStatus::Missing | TargetFieldStatus::Ambiguous
        ) {
            let suffix = if status == TargetFieldStatus::Ambiguous {
                "ambiguous"
            } else {
                "incomplete"
            };
            reasons.push(format!("{name}_{suffix}"));
        }
    }
    for gap in &subject_resolution.blockers {
        reasons.push(format!("subject_gap_{}", gap.label()));
    }
    let complete =
        reasons.is_empty() && capability.executor_available && capability.verifier_available;
    let final_status = if reasons.is_empty() {
        TargetStatus::Complete
    } else if reasons.iter().any(|reason| reason.contains("ambiguous")) {
        TargetStatus::Ambiguous(reasons.clone())
    } else {
        TargetStatus::Incomplete(reasons.clone())
    };
    let binding_status = match arguments_status {
        TargetFieldStatus::Complete | TargetFieldStatus::NotRequired => BindingStatus::Complete,
        TargetFieldStatus::Ambiguous => {
            BindingStatus::Ambiguous(vec!["argument_binding_ambiguous".into()])
        }
        TargetFieldStatus::Missing => {
            BindingStatus::Missing(vec!["required_argument_binding_missing".into()])
        }
    };
    let build_trace = TargetBuildTrace {
        operation: operation_status.clone(),
        subject: completeness.subject,
        binding_status,
        bindings: completeness.arguments,
        requested_form: completeness.requested_form,
        provenance: completeness.provenance,
        final_status,
    };
    TargetCompletion {
        target: FormalizedTarget {
            operation,
            operation_status,
            frame,
            subject,
            subject_resolution,
            target_variable,
            arguments,
            domain,
            requested_form,
            answer_form,
            provenance,
            completeness,
        },
        reasons,
        operation_supported: capability.executor_available,
        verifier_available: capability.verifier_available,
        complete,
        build_trace,
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
        let statement = extract_explicit_relation(question)
            .map(|(lhs, relation, rhs)| format!("{lhs} {relation} {rhs}"))
            .unwrap_or_else(|| "explicit relation or equation signal".into());
        facts.push(FactAnnotation {
            statement,
            source_fragment: question.into(),
        });
    } else if let Some(expression) = extract_expression_payload(question) {
        facts.push(FactAnnotation {
            statement: expression,
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
            "simplify",
            "substitute",
            "plug in",
            "compare",
            "equivalent",
            "verify",
            "check",
            "prove",
            "show that",
            "how many",
            "count",
            "what does",
        ],
    ) {
        let request_clause = question
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
        let statement = request_clause
            .rsplit_once(". ")
            .map(|(_, clause)| clause.trim())
            .filter(|clause| {
                has_any(
                    &clause.to_ascii_lowercase(),
                    &[
                        "find",
                        "compute",
                        "calculate",
                        "determine",
                        "evaluate",
                        "solve",
                        "what is",
                        "which",
                        "simplify",
                        "compare",
                        "check",
                        "prove",
                        "show",
                        "simplify",
                        "substitute",
                        "plug in",
                        "compare",
                        "equivalent",
                        "verify",
                        "check",
                        "how many",
                        "count",
                        "what does",
                    ],
                )
            })
            .unwrap_or(&request_clause)
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

    let typed_facts = formalized_facts(&facts, question);
    let target_completion = build_target_completion(question, target.as_ref(), &typed_facts);
    let formalized_target = target_completion.target.clone();
    let typed_constraints = constraints
        .iter()
        .map(|constraint| FormalizedConstraint::DomainOrSideCondition {
            statement: constraint.statement.clone(),
            source_fragment: constraint.source_fragment.clone(),
        })
        .collect();
    let facts_completeness = completeness_for(
        &obligations,
        &[
            ModelingObligation::ConstructEquation,
            ModelingObligation::ExtractQuantifiers,
            ModelingObligation::DefineObject,
        ],
    );
    let constraints_completeness = completeness_for(
        &obligations,
        &[
            ModelingObligation::IdentifyDomain,
            ModelingObligation::ResolveEntityReference,
        ],
    );

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
        formalized_facts: typed_facts,
        facts_completeness,
        target,
        formalized_target,
        target_completion,
        assumptions,
        constraints,
        formalized_constraints: typed_constraints,
        constraints_completeness,
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
    fn operation_frame_tracks_simplification_without_bindings() {
        let trace = assess_prompt("q", "Simplify x + x.", "Math", false);
        let target = &trace.target_completion;
        assert_eq!(
            target.target.operation_status,
            OperationStatus::Recognized(OperationKind::Simplify)
        );
        assert!(matches!(
            target.target.frame,
            Some(OperationFrame::Simplify { .. })
        ));
        assert_eq!(
            target.target.completeness.arguments,
            TargetFieldStatus::NotRequired
        );
        assert!(matches!(
            target.build_trace.final_status,
            TargetStatus::Complete
        ));
        assert_eq!(target.build_trace.binding_status, BindingStatus::Complete);
    }

    #[test]
    fn operation_frame_distinguishes_unsupported_proof_from_unknown_request() {
        let trace = assess_prompt("q", "Prove that x = x.", "Math", false);
        let target = &trace.target_completion;
        assert_eq!(
            target.target.operation_status,
            OperationStatus::Unsupported("prove".into())
        );
        assert!(!target.operation_supported);
        assert!(matches!(
            target.build_trace.final_status,
            TargetStatus::Complete
        ));
    }

    #[test]
    fn answer_form_preserves_positive_root_selection() {
        let trace = assess_prompt(
            "q",
            "Solve x^2 - 4 = 0 and give the positive solution.",
            "Math",
            false,
        );
        assert_eq!(
            trace.target_completion.target.answer_form,
            Some(AnswerForm::SingleSelectedSolution)
        );
        assert_eq!(
            trace.target_completion.target.completeness.target_variable,
            TargetFieldStatus::Complete
        );
    }

    #[test]
    fn subject_resolution_links_function_application_to_definition() {
        let trace = assess_prompt(
            "q",
            "Let f(x)=x^2+1. Let g(x)=2x. What is f(3)?",
            "Math",
            false,
        );
        let resolution = &trace.target_completion.target.subject_resolution;
        assert_eq!(
            resolution
                .selected
                .as_ref()
                .map(|candidate| candidate.object_id.as_str()),
            Some("f")
        );
        assert_eq!(
            resolution
                .selected
                .as_ref()
                .map(|candidate| candidate.object_type),
            Some(SubjectObjectType::Function)
        );
        assert!(resolution.selected.as_ref().unwrap().definition_available);
        assert!(resolution.blockers.is_empty());
    }

    #[test]
    fn subject_resolution_rejects_undefined_function_reference() {
        let trace = assess_prompt("q", "What is h(3)?", "Math", false);
        assert!(trace
            .target_completion
            .target
            .subject_resolution
            .blockers
            .contains(&SubjectGap::ObjectMentionedButUndefined));
        assert!(trace
            .target_completion
            .reasons
            .iter()
            .any(|reason| reason == "subject_gap_object_mentioned_but_undefined"));
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
