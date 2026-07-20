//! Typed, provenance-bearing method metadata for the verified solver path.
//!
//! Formula text is deliberately not executable knowledge.  A [`MethodSpec`]
//! says which semantic quantities a relation accepts, which assumptions it
//! needs, and which directed derivations are legal before a CAS is involved.

use crate::router::{Capability, ProblemQuantity, StructuredProblem, Tool};
use serde::Serialize;
use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize)]
pub struct MethodId(pub String);

impl MethodId {
    pub fn new(id: impl Into<String>) -> Self {
        Self(id.into())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum Domain {
    Algebra,
    Physics,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum MethodFamily {
    LinearAlgebra,
    QuadraticAlgebra,
    Kinematics,
    Mechanics,
    Energy,
}

/// SI base-dimension exponents.  This is intentionally a value type so
/// dimensional compatibility is structural, never a unit-name string match.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
pub struct Dimensions {
    pub length: i8,
    pub mass: i8,
    pub time: i8,
}

impl Dimensions {
    pub const DIMENSIONLESS: Self = Self {
        length: 0,
        mass: 0,
        time: 0,
    };
    pub const LENGTH: Self = Self {
        length: 1,
        mass: 0,
        time: 0,
    };
    pub const TIME: Self = Self {
        length: 0,
        mass: 0,
        time: 1,
    };
    pub const MASS: Self = Self {
        length: 0,
        mass: 1,
        time: 0,
    };
    pub const VELOCITY: Self = Self {
        length: 1,
        mass: 0,
        time: -1,
    };
    pub const ACCELERATION: Self = Self {
        length: 1,
        mass: 0,
        time: -2,
    };
    pub const FORCE: Self = Self {
        length: 1,
        mass: 1,
        time: -2,
    };
    pub const ENERGY: Self = Self {
        length: 2,
        mass: 1,
        time: -2,
    };
    pub const POWER: Self = Self {
        length: 2,
        mass: 1,
        time: -3,
    };
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
pub enum QuantityConcept {
    Distance,
    Time,
    Mass,
    Force,
    Velocity,
    Acceleration,
    Energy,
    Work,
    Power,
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize)]
pub enum Qualifier {
    Initial,
    Final,
    Average,
    Constant,
    RelativeTo(String),
}

/// Distinguishes values that share dimensions but cannot be substituted for
/// one another without an explicitly authorized method.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
pub enum ValueKind {
    Scalar,
    SignedScalar,
    Magnitude,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize)]
pub struct SemanticVariable {
    pub concept: QuantityConcept,
    pub qualifiers: BTreeSet<Qualifier>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct VariableSpec {
    pub local_symbol: String,
    pub semantic: SemanticVariable,
    pub dimensions: Dimensions,
    pub value_kind: ValueKind,
}

impl VariableSpec {
    pub fn quantity(symbol: &str, concept: QuantityConcept, dimensions: Dimensions) -> Self {
        Self {
            local_symbol: symbol.to_string(),
            semantic: SemanticVariable {
                concept,
                qualifiers: BTreeSet::new(),
            },
            dimensions,
            value_kind: ValueKind::Scalar,
        }
    }

    pub fn magnitude(symbol: &str, concept: QuantityConcept, dimensions: Dimensions) -> Self {
        let mut value = Self::quantity(symbol, concept, dimensions);
        value.value_kind = ValueKind::Magnitude;
        value
    }

    pub fn signed_scalar(symbol: &str, concept: QuantityConcept, dimensions: Dimensions) -> Self {
        let mut value = Self::quantity(symbol, concept, dimensions);
        value.value_kind = ValueKind::SignedScalar;
        value
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub enum MethodAssumption {
    ConstantVelocity,
    ConstantAcceleration,
    ConstantForce,
    CollinearForceDisplacement,
    /// Required when an energy value is reinterpreted as energy transferred
    /// over the interval used by an average-power relation.  A duration by
    /// itself never establishes this bridge.
    EnergyTransferredOverInterval,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub enum MethodConstraint {
    NonNegative(String),
    NonZero(String),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Relation {
    /// Kept as a human-readable audit artifact.  Solvers receive a parsed AST
    /// only after an edge has passed typed applicability checks.
    pub expression: String,
    pub variables: Vec<String>,
    /// Additional conditions for solving this relation *for* a symbol.
    pub inversion_constraints: BTreeMap<String, Vec<MethodConstraint>>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Provenance {
    pub source: String,
    pub quality: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct MethodSpec {
    pub id: MethodId,
    pub domain: Domain,
    pub family: MethodFamily,
    pub relations: Vec<Relation>,
    pub variables: Vec<VariableSpec>,
    pub assumptions: Vec<MethodAssumption>,
    /// Assumptions required only when this method consumes a derived
    /// intermediate.  Direct measured inputs (for example, "20 J over 4 s")
    /// remain valid without a bridge assumption.
    pub handoff_assumptions: Vec<MethodAssumption>,
    pub constraints: Vec<MethodConstraint>,
    pub capabilities: Vec<Capability>,
    pub provenance: Provenance,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct DerivationEdge {
    pub id: DerivationEdgeId,
    pub method_id: MethodId,
    pub relation: String,
    pub requires: Vec<VariableSpec>,
    pub produces: VariableSpec,
    pub preconditions: Vec<MethodConstraint>,
    pub handoff_assumptions: Vec<MethodAssumption>,
    pub operation: Capability,
}

/// Stable identity for one authorized direction of one relation.  A method is
/// not enough for audit: `F = ma` has three materially different derivations.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize)]
pub struct DerivationEdgeId(pub String);

impl MethodSpec {
    /// Compile each relation into a bounded set of directed derivations.  This
    /// does not guess an algebraic rearrangement: all possible outputs must be
    /// declared variables, and inversion preconditions travel with the edge.
    pub fn derivation_edges(&self) -> Vec<DerivationEdge> {
        self.relations
            .iter()
            .flat_map(|relation| {
                relation.variables.iter().filter_map(|output| {
                    let produces = self
                        .variables
                        .iter()
                        .find(|variable| variable.local_symbol == *output)?
                        .clone();
                    let requires: Vec<VariableSpec> = relation
                        .variables
                        .iter()
                        .filter(|symbol| *symbol != output)
                        .filter_map(|symbol| {
                            self.variables
                                .iter()
                                .find(|variable| variable.local_symbol == *symbol)
                                .cloned()
                        })
                        .collect();
                    (requires.len() + 1 == relation.variables.len()).then(|| {
                        let mut preconditions = self.constraints.clone();
                        preconditions.extend(
                            relation
                                .inversion_constraints
                                .get(output)
                                .cloned()
                                .unwrap_or_default(),
                        );
                        DerivationEdge {
                            id: DerivationEdgeId(format!("{}::solve_{}", self.id.0, output)),
                            method_id: self.id.clone(),
                            relation: relation.expression.clone(),
                            requires,
                            produces,
                            preconditions,
                            handoff_assumptions: self.handoff_assumptions.clone(),
                            operation: Capability::SolveEquation,
                        }
                    })
                })
            })
            .collect()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct VariableBinding {
    pub method_symbol: String,
    pub problem_variable: String,
    pub value: String,
    pub unit: Option<String>,
    pub source: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct EstablishedAssumption {
    pub assumption: MethodAssumption,
    pub source: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SatisfiedConstraint {
    pub constraint: MethodConstraint,
    pub evidence: String,
}

/// Lexicographic rather than scalar cost.  Safety-relevant uncertainty always
/// outranks the convenience of a shorter algebraic path.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Serialize)]
pub struct PlanCost {
    pub unresolved_ambiguities: usize,
    pub inferred_assumptions: usize,
    pub weak_bindings: usize,
    pub method_steps: usize,
    pub inversions: usize,
    pub unit_conversions: usize,
    pub numeric_approximations: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub enum CandidateRejection {
    MissingSemanticInput,
    TargetMismatch,
    DimensionMismatch,
    MissingAssumption,
    ContradictedAssumption,
    UnsatisfiedConstraint,
    AmbiguousBinding,
    UnsupportedExecution,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct RejectedCandidateTrace {
    pub method_id: MethodId,
    pub edge_id: DerivationEdgeId,
    pub reason: CandidateRejection,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct PlannedDerivationTrace {
    pub method_id: MethodId,
    pub edge_id: DerivationEdgeId,
    pub target_binding: String,
    pub input_bindings: Vec<VariableBinding>,
    pub established_assumptions: Vec<EstablishedAssumption>,
    pub satisfied_constraints: Vec<SatisfiedConstraint>,
    pub cost: PlanCost,
    pub rejected_alternatives: Vec<RejectedCandidateTrace>,
}

/// Receipt emitted by execution, separate from authorization by the planner.
/// Strings are deliberate audit artifacts for now; a future typed CAS AST can
/// replace them without changing the planner/executor boundary.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ExecutionReceipt {
    pub plan_id: DerivationEdgeId,
    pub operation: Capability,
    pub symbolic_input: String,
    pub symbolic_output: String,
    pub substituted_values: Vec<VariableBinding>,
    pub numeric_output: Option<String>,
    pub generated_constraints: Vec<MethodConstraint>,
    pub discarded_solutions: Vec<String>,
}

/// A semantic hand-off between two authorized edges.  The value is produced
/// by execution later; planning records its identity and provenance now.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct IntermediateBinding {
    pub quantity: SemanticVariable,
    pub value_kind: ValueKind,
    pub dimensions: Dimensions,
    pub produced_by: DerivationEdgeId,
    pub consumed_by: DerivationEdgeId,
    pub source_dependencies: Vec<String>,
    pub assumptions: Vec<EstablishedAssumption>,
    pub consumed_as: SemanticVariable,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct DerivationPlan {
    pub steps: Vec<SingleStepPlan>,
    pub intermediate_bindings: Vec<IntermediateBinding>,
    pub combined_constraints: Vec<MethodConstraint>,
    pub cost: PlanCost,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub enum PlanSelection {
    Unique(DerivationPlan),
    Consensus(Vec<DerivationPlan>),
    Ambiguous(Vec<DerivationPlan>),
    None(Vec<RejectedCandidateTrace>),
}

/// Hard limits for the first composition implementation.  In particular,
/// only one unresolved input may be supplied by the inner step.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct PlannerLimits {
    pub max_depth: usize,
    pub max_candidates_per_target: usize,
    pub max_total_expansions: usize,
    pub allow_inferred_assumptions: bool,
    pub allow_ambiguous_bindings: bool,
}

impl Default for PlannerLimits {
    fn default() -> Self {
        Self {
            max_depth: 2,
            max_candidates_per_target: 8,
            max_total_expansions: 64,
            allow_inferred_assumptions: false,
            allow_ambiguous_bindings: false,
        }
    }
}

/// A plan-level receipt keeps local execution receipts and the intermediate
/// hand-off separate.  It is intentionally not an answer by itself; callers
/// must still populate `final_verification`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct DerivedIntermediate {
    pub binding: IntermediateBinding,
    pub value: String,
    pub source_receipt: DerivationEdgeId,
    pub source_dependencies: Vec<String>,
    pub assumptions: Vec<EstablishedAssumption>,
    pub consumed_as: SemanticVariable,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct VerificationReceipt {
    pub checks: Vec<String>,
    pub passed: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct PlanExecutionReceipt {
    pub plan_id: String,
    pub step_receipts: Vec<ExecutionReceipt>,
    pub intermediate_values: Vec<DerivedIntermediate>,
    pub final_verification: VerificationReceipt,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SingleStepPlan {
    pub edge: DerivationEdge,
    pub bindings: Vec<VariableBinding>,
    pub assumptions: Vec<EstablishedAssumption>,
    pub satisfied_constraints: Vec<SatisfiedConstraint>,
    pub cost: PlanCost,
    pub rejected_alternatives: Vec<RejectedCandidateTrace>,
}

impl SingleStepPlan {
    pub fn trace(&self, target: &str) -> PlannedDerivationTrace {
        PlannedDerivationTrace {
            method_id: self.edge.method_id.clone(),
            edge_id: self.edge.id.clone(),
            target_binding: target.to_string(),
            input_bindings: self.bindings.clone(),
            established_assumptions: self.assumptions.clone(),
            satisfied_constraints: self.satisfied_constraints.clone(),
            cost: self.cost,
            rejected_alternatives: self.rejected_alternatives.clone(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub enum SingleStepPlanResult {
    Planned(SingleStepPlan),
    NoApplicableMethod(Vec<RejectedCandidateTrace>),
    MultipleUnresolvedMethods(Vec<MethodId>, Vec<RejectedCandidateTrace>),
}

#[derive(Debug, Clone, Default)]
pub struct MethodRegistry {
    methods: Vec<MethodSpec>,
}

impl MethodRegistry {
    pub fn mechanics_island() -> Self {
        let newton = MethodSpec {
            id: MethodId::new("mechanics.newton_second_law"),
            domain: Domain::Physics,
            family: MethodFamily::Mechanics,
            relations: vec![Relation {
                expression: "F = m * a".to_string(),
                variables: vec!["F".to_string(), "m".to_string(), "a".to_string()],
                inversion_constraints: BTreeMap::from([
                    (
                        "a".to_string(),
                        vec![MethodConstraint::NonZero("m".to_string())],
                    ),
                    (
                        "m".to_string(),
                        vec![MethodConstraint::NonZero("a".to_string())],
                    ),
                ]),
            }],
            variables: vec![
                VariableSpec::quantity("F", QuantityConcept::Force, Dimensions::FORCE),
                VariableSpec::quantity("m", QuantityConcept::Mass, Dimensions::MASS),
                VariableSpec::quantity(
                    "a",
                    QuantityConcept::Acceleration,
                    Dimensions::ACCELERATION,
                ),
            ],
            assumptions: Vec::new(),
            handoff_assumptions: Vec::new(),
            constraints: Vec::new(),
            capabilities: vec![
                Capability::BindVariables,
                Capability::SolveEquation,
                Capability::CheckDimensions,
                Capability::VerifySubstitution,
            ],
            provenance: Provenance {
                source: "mechanics kernel".to_string(),
                quality: "curated".to_string(),
            },
        };
        let constant_velocity = MethodSpec {
            id: MethodId::new("mechanics.constant_velocity.distance"),
            domain: Domain::Physics,
            family: MethodFamily::Kinematics,
            relations: vec![Relation {
                expression: "d = v * t".to_string(),
                variables: vec!["d".to_string(), "v".to_string(), "t".to_string()],
                inversion_constraints: BTreeMap::from([
                    (
                        "v".to_string(),
                        vec![MethodConstraint::NonZero("t".to_string())],
                    ),
                    (
                        "t".to_string(),
                        vec![MethodConstraint::NonZero("v".to_string())],
                    ),
                ]),
            }],
            variables: vec![
                VariableSpec::quantity("d", QuantityConcept::Distance, Dimensions::LENGTH),
                VariableSpec::quantity("v", QuantityConcept::Velocity, Dimensions::VELOCITY),
                VariableSpec::quantity("t", QuantityConcept::Time, Dimensions::TIME),
            ],
            assumptions: vec![MethodAssumption::ConstantVelocity],
            handoff_assumptions: Vec::new(),
            constraints: vec![MethodConstraint::NonNegative("t".to_string())],
            capabilities: vec![
                Capability::BindVariables,
                Capability::SolveEquation,
                Capability::CheckDimensions,
                Capability::VerifySubstitution,
            ],
            provenance: Provenance {
                source: "mechanics kernel".to_string(),
                quality: "curated".to_string(),
            },
        };
        let kinetic_energy = MethodSpec {
            id: MethodId::new("mechanics.kinetic_energy"),
            domain: Domain::Physics,
            family: MethodFamily::Energy,
            relations: vec![Relation {
                expression: "E = 1/2 * m * v^2".to_string(),
                variables: vec!["E".to_string(), "m".to_string(), "v".to_string()],
                inversion_constraints: BTreeMap::from([
                    (
                        "v".to_string(),
                        vec![MethodConstraint::NonZero("m".to_string())],
                    ),
                    (
                        "m".to_string(),
                        vec![MethodConstraint::NonZero("v".to_string())],
                    ),
                ]),
            }],
            variables: vec![
                VariableSpec::magnitude("E", QuantityConcept::Energy, Dimensions::ENERGY),
                VariableSpec::quantity("m", QuantityConcept::Mass, Dimensions::MASS),
                VariableSpec::magnitude("v", QuantityConcept::Velocity, Dimensions::VELOCITY),
            ],
            assumptions: Vec::new(),
            handoff_assumptions: Vec::new(),
            constraints: Vec::new(),
            capabilities: vec![
                Capability::BindVariables,
                Capability::SolveEquation,
                Capability::CheckDimensions,
                Capability::VerifySubstitution,
            ],
            provenance: Provenance {
                source: "mechanics kernel".to_string(),
                quality: "curated".to_string(),
            },
        };
        let power = MethodSpec {
            id: MethodId::new("mechanics.power"),
            domain: Domain::Physics,
            family: MethodFamily::Energy,
            relations: vec![Relation {
                expression: "P = E / t".to_string(),
                variables: vec!["P".to_string(), "E".to_string(), "t".to_string()],
                inversion_constraints: BTreeMap::from([
                    (
                        "P".to_string(),
                        vec![MethodConstraint::NonZero("t".to_string())],
                    ),
                    (
                        "t".to_string(),
                        vec![MethodConstraint::NonZero("P".to_string())],
                    ),
                ]),
            }],
            variables: vec![
                VariableSpec::magnitude("P", QuantityConcept::Power, Dimensions::POWER),
                VariableSpec::magnitude("E", QuantityConcept::Energy, Dimensions::ENERGY),
                VariableSpec::quantity("t", QuantityConcept::Time, Dimensions::TIME),
            ],
            assumptions: Vec::new(),
            handoff_assumptions: vec![MethodAssumption::EnergyTransferredOverInterval],
            constraints: vec![MethodConstraint::NonNegative("t".to_string())],
            capabilities: vec![
                Capability::BindVariables,
                Capability::SolveEquation,
                Capability::CheckDimensions,
                Capability::VerifySubstitution,
            ],
            provenance: Provenance {
                source: "mechanics kernel".to_string(),
                quality: "curated".to_string(),
            },
        };
        let work = MethodSpec {
            id: MethodId::new("mechanics.work_constant_force"),
            domain: Domain::Physics,
            family: MethodFamily::Mechanics,
            relations: vec![Relation {
                expression: "W = F * d".to_string(),
                variables: vec!["W".to_string(), "F".to_string(), "d".to_string()],
                inversion_constraints: BTreeMap::from([
                    (
                        "F".to_string(),
                        vec![MethodConstraint::NonZero("d".to_string())],
                    ),
                    (
                        "d".to_string(),
                        vec![MethodConstraint::NonZero("F".to_string())],
                    ),
                ]),
            }],
            variables: vec![
                VariableSpec::quantity("W", QuantityConcept::Work, Dimensions::ENERGY),
                VariableSpec::quantity("F", QuantityConcept::Force, Dimensions::FORCE),
                VariableSpec::magnitude("d", QuantityConcept::Distance, Dimensions::LENGTH),
            ],
            assumptions: vec![
                MethodAssumption::ConstantForce,
                MethodAssumption::CollinearForceDisplacement,
            ],
            handoff_assumptions: Vec::new(),
            constraints: Vec::new(),
            capabilities: vec![
                Capability::BindVariables,
                Capability::SolveEquation,
                Capability::CheckDimensions,
                Capability::VerifySubstitution,
            ],
            provenance: Provenance {
                source: "mechanics kernel".to_string(),
                quality: "curated".to_string(),
            },
        };
        Self {
            methods: vec![newton, constant_velocity, kinetic_energy, power, work],
        }
    }

    pub fn methods(&self) -> &[MethodSpec] {
        &self.methods
    }

    /// Safe depth-one planning.  A plan exists only where exactly one method
    /// can produce the target from explicit semantic inputs and assumptions.
    pub fn plan_single_step(&self, problem: &StructuredProblem) -> SingleStepPlanResult {
        if problem.domain != Tool::Physics {
            return SingleStepPlanResult::NoApplicableMethod(Vec::new());
        }
        let Some(target) = problem
            .requested
            .as_deref()
            .and_then(semantic_variable_for_name)
        else {
            return SingleStepPlanResult::NoApplicableMethod(Vec::new());
        };
        let known: Vec<(&ProblemQuantity, SemanticVariable)> = problem
            .givens
            .iter()
            .filter_map(|given| {
                semantic_variable_for_name(&given.variable).map(|semantic| (given, semantic))
            })
            .collect();
        let mut rejected = Vec::new();
        let mut candidates = Vec::new();
        for edge in self.methods.iter().flat_map(MethodSpec::derivation_edges) {
            let reject = |reason, rejected: &mut Vec<RejectedCandidateTrace>| {
                rejected.push(RejectedCandidateTrace {
                    method_id: edge.method_id.clone(),
                    edge_id: edge.id.clone(),
                    reason,
                });
            };
            if edge.produces.semantic != target {
                reject(CandidateRejection::TargetMismatch, &mut rejected);
                continue;
            }
            let Some(method) = self
                .methods
                .iter()
                .find(|method| method.id == edge.method_id)
            else {
                reject(CandidateRejection::UnsupportedExecution, &mut rejected);
                continue;
            };
            if method.id.0 == "mechanics.work_constant_force"
                && entity_or_interval_identity_ambiguous(problem)
            {
                reject(CandidateRejection::AmbiguousBinding, &mut rejected);
                continue;
            }
            if method
                .assumptions
                .iter()
                .any(|assumption| assumption_is_contradicted(assumption, problem))
            {
                reject(CandidateRejection::ContradictedAssumption, &mut rejected);
                continue;
            }
            let mut established = Vec::new();
            let mut missing_assumption = false;
            for assumption in &method.assumptions {
                match problem.assumptions.iter().find(|source| {
                    assumption_is_explicit(assumption, std::slice::from_ref(*source))
                }) {
                    Some(source) => established.push(EstablishedAssumption {
                        assumption: assumption.clone(),
                        source: source.clone(),
                    }),
                    None => missing_assumption = true,
                }
            }
            if missing_assumption {
                reject(CandidateRejection::MissingAssumption, &mut rejected);
                continue;
            }
            let mut bindings = Vec::new();
            let mut input_failure = None;
            for required in &edge.requires {
                let semantic_matches: Vec<_> = known
                    .iter()
                    .filter(|(_, semantic)| *semantic == required.semantic)
                    .collect();
                if semantic_matches.is_empty() {
                    input_failure = Some(CandidateRejection::MissingSemanticInput);
                    break;
                }
                let dimensional_matches: Vec<_> = semantic_matches
                    .into_iter()
                    .filter(|(given, _)| {
                        unit_dimensions(given.unit.as_deref()) == Some(required.dimensions)
                    })
                    .collect();
                if dimensional_matches.is_empty() {
                    input_failure = Some(CandidateRejection::DimensionMismatch);
                    break;
                }
                if dimensional_matches.len() != 1 {
                    input_failure = Some(CandidateRejection::AmbiguousBinding);
                    break;
                }
                let (given, _) = dimensional_matches[0];
                bindings.push(VariableBinding {
                    method_symbol: required.local_symbol.clone(),
                    problem_variable: given.variable.clone(),
                    value: given.value.clone(),
                    unit: given.unit.clone(),
                    source: given.source.clone(),
                });
            }
            if let Some(reason) = input_failure {
                reject(reason, &mut rejected);
                continue;
            }
            let mut satisfied_constraints = Vec::new();
            let mut failed_constraint = false;
            for constraint in &edge.preconditions {
                // Preconditions on the produced variable are checked by the
                // executor after solving (for example, non-negative time
                // when solving `d = v*t` for `t`).  They cannot be evaluated
                // from input bindings during authorization.
                if constraint_symbol(constraint) == edge.produces.local_symbol {
                    continue;
                }
                let Some(binding) = bindings.iter().find(|binding| match constraint {
                    MethodConstraint::NonNegative(symbol) | MethodConstraint::NonZero(symbol) => {
                        &binding.method_symbol == symbol
                    }
                }) else {
                    failed_constraint = true;
                    break;
                };
                let Some(value) = binding
                    .value
                    .parse::<f64>()
                    .ok()
                    .filter(|value| value.is_finite())
                else {
                    failed_constraint = true;
                    break;
                };
                let holds = match constraint {
                    MethodConstraint::NonNegative(_) => value >= 0.0,
                    MethodConstraint::NonZero(_) => value != 0.0,
                };
                if !holds {
                    failed_constraint = true;
                    break;
                }
                satisfied_constraints.push(SatisfiedConstraint {
                    constraint: constraint.clone(),
                    evidence: format!(
                        "{} = {} from {}",
                        binding.method_symbol, binding.value, binding.source
                    ),
                });
            }
            if failed_constraint {
                reject(CandidateRejection::UnsatisfiedConstraint, &mut rejected);
                continue;
            }
            candidates.push(SingleStepPlan {
                edge,
                bindings,
                assumptions: established,
                satisfied_constraints,
                cost: PlanCost {
                    method_steps: 1,
                    inversions: usize::from(problem.requested.as_deref() != Some("F")),
                    ..PlanCost::default()
                },
                rejected_alternatives: Vec::new(),
            });
        }
        match candidates.len() {
            0 => SingleStepPlanResult::NoApplicableMethod(rejected),
            1 => {
                let mut plan = candidates.into_iter().next().expect("one candidate");
                plan.rejected_alternatives = rejected;
                SingleStepPlanResult::Planned(plan)
            }
            _ => SingleStepPlanResult::MultipleUnresolvedMethods(
                candidates
                    .into_iter()
                    .map(|plan| plan.edge.method_id)
                    .collect(),
                rejected,
            ),
        }
    }

    /// Restrictive backward composition.  The outer edge must produce the
    /// requested target and may have exactly one unresolved input.  That
    /// input is supplied by one inner edge whose remaining inputs are all
    /// explicit givens.  No assumptions are inferred and no edge may repeat.
    pub fn plan_depth_two(
        &self,
        problem: &StructuredProblem,
        limits: PlannerLimits,
    ) -> PlanSelection {
        if problem.domain != Tool::Physics || limits.max_depth < 2 {
            return PlanSelection::None(Vec::new());
        }
        let Some(target) = problem
            .requested
            .as_deref()
            .and_then(semantic_variable_for_name)
        else {
            return PlanSelection::None(Vec::new());
        };
        let known: Vec<(&ProblemQuantity, SemanticVariable)> = problem
            .givens
            .iter()
            .filter_map(|given| {
                semantic_variable_for_name(&given.variable).map(|semantic| (given, semantic))
            })
            .collect();
        let edges: Vec<DerivationEdge> = self
            .methods
            .iter()
            .flat_map(MethodSpec::derivation_edges)
            .collect();
        let mut rejected = Vec::new();
        let mut plans = Vec::new();
        let mut expansions = 0usize;

        for outer in &edges {
            if expansions >= limits.max_total_expansions {
                break;
            }
            expansions += 1;
            if outer.produces.semantic != target {
                continue;
            }
            let Some(outer_method) = self
                .methods
                .iter()
                .find(|method| method.id == outer.method_id)
            else {
                rejected.push(rejected_candidate(
                    outer,
                    CandidateRejection::UnsupportedExecution,
                ));
                continue;
            };
            if outer_method.id.0 == "mechanics.work_constant_force"
                && entity_or_interval_identity_ambiguous(problem)
            {
                rejected.push(rejected_candidate(
                    outer,
                    CandidateRejection::AmbiguousBinding,
                ));
                continue;
            }
            let Ok(outer_assumptions) = explicit_assumptions(problem, outer_method) else {
                rejected.push(rejected_candidate(
                    outer,
                    CandidateRejection::MissingAssumption,
                ));
                continue;
            };
            let (outer_bindings, missing) = match bind_known_inputs(&outer.requires, &known) {
                Ok(bindings) => (bindings, Vec::new()),
                Err((bindings, missing, reason)) => {
                    if reason == CandidateRejection::MissingSemanticInput && !missing.is_empty() {
                        (bindings, missing)
                    } else {
                        rejected.push(rejected_candidate(outer, reason));
                        continue;
                    }
                }
            };
            let handoff_assumptions = match explicit_handoff_assumptions(problem, outer) {
                Ok(value) => value,
                Err(reason) => {
                    rejected.push(rejected_candidate(outer, reason.clone()));
                    // Keep the legacy "missing applicability" diagnostic as
                    // well as the more precise contradiction stage.  Existing
                    // benchmark traces consume the former, while new
                    // development reports can distinguish the latter.
                    if reason == CandidateRejection::ContradictedAssumption {
                        rejected.push(rejected_candidate(
                            outer,
                            CandidateRejection::MissingAssumption,
                        ));
                    }
                    continue;
                }
            };
            if missing.len() != 1 {
                // Depth two deliberately does not solve two simultaneous
                // subgoals, and it does not duplicate a one-step plan.
                if missing.len() > 1 {
                    rejected.push(rejected_candidate(
                        outer,
                        CandidateRejection::MissingSemanticInput,
                    ));
                }
                continue;
            }
            let intermediate = &missing[0];

            for inner in &edges {
                if plans.len() >= limits.max_candidates_per_target
                    || expansions >= limits.max_total_expansions
                {
                    break;
                }
                expansions += 1;
                if inner.produces.semantic != intermediate.semantic
                    || inner.produces.dimensions != intermediate.dimensions
                    || inner.produces.value_kind != intermediate.value_kind
                    || inner.id == outer.id
                {
                    if inner.produces.semantic == intermediate.semantic
                        && (inner.produces.dimensions != intermediate.dimensions
                            || inner.produces.value_kind != intermediate.value_kind)
                    {
                        rejected.push(rejected_candidate(
                            inner,
                            CandidateRejection::DimensionMismatch,
                        ));
                    }
                    continue;
                }
                let Some(inner_method) = self
                    .methods
                    .iter()
                    .find(|method| method.id == inner.method_id)
                else {
                    rejected.push(rejected_candidate(
                        inner,
                        CandidateRejection::UnsupportedExecution,
                    ));
                    continue;
                };
                let Ok(inner_assumptions) = explicit_assumptions(problem, inner_method) else {
                    rejected.push(rejected_candidate(
                        inner,
                        CandidateRejection::MissingAssumption,
                    ));
                    continue;
                };
                let inner_bindings = match bind_known_inputs(&inner.requires, &known) {
                    Ok(bindings) => bindings,
                    Err((_, missing_inputs, reason)) => {
                        let reason = if missing_inputs.is_empty() {
                            reason
                        } else {
                            CandidateRejection::MissingSemanticInput
                        };
                        rejected.push(rejected_candidate(inner, reason));
                        continue;
                    }
                };
                let inner_satisfied =
                    match satisfied_constraints(&inner.preconditions, &inner_bindings) {
                        Ok(value) => value,
                        Err(reason) => {
                            rejected.push(rejected_candidate(inner, reason));
                            continue;
                        }
                    };
                // Constraints on the intermediate cannot be numerically
                // checked until execution; retain them for plan verification.
                let mut outer_satisfied = Vec::new();
                let mut outer_constraints_ok = true;
                for constraint in &outer.preconditions {
                    let symbol = constraint_symbol(constraint);
                    if symbol == intermediate.local_symbol {
                        continue;
                    }
                    match satisfied_constraints(std::slice::from_ref(constraint), &outer_bindings) {
                        Ok(mut value) => outer_satisfied.append(&mut value),
                        Err(reason) => {
                            rejected.push(rejected_candidate(outer, reason));
                            outer_constraints_ok = false;
                            break;
                        }
                    }
                }
                if !outer_constraints_ok {
                    continue;
                }
                let intermediate_binding = IntermediateBinding {
                    quantity: inner.produces.semantic.clone(),
                    value_kind: inner.produces.value_kind,
                    dimensions: inner.produces.dimensions,
                    produced_by: inner.id.clone(),
                    consumed_by: outer.id.clone(),
                    source_dependencies: inner_bindings
                        .iter()
                        .map(|binding| binding.source.clone())
                        .collect(),
                    assumptions: handoff_assumptions.clone(),
                    consumed_as: intermediate.semantic.clone(),
                };
                let synthetic = VariableBinding {
                    method_symbol: intermediate.local_symbol.clone(),
                    problem_variable: "<derived intermediate>".to_string(),
                    value: "<derived at execution>".to_string(),
                    unit: None,
                    source: format!("produced by {}", inner.id.0),
                };
                let mut outer_all_bindings = outer_bindings.clone();
                outer_all_bindings.push(synthetic);
                let inner_plan = SingleStepPlan {
                    edge: inner.clone(),
                    bindings: inner_bindings,
                    assumptions: inner_assumptions,
                    satisfied_constraints: inner_satisfied,
                    cost: step_cost(inner, &inner.produces.local_symbol),
                    rejected_alternatives: Vec::new(),
                };
                let outer_plan = SingleStepPlan {
                    edge: outer.clone(),
                    bindings: outer_all_bindings,
                    assumptions: outer_assumptions
                        .iter()
                        .cloned()
                        .chain(handoff_assumptions.iter().cloned())
                        .collect(),
                    satisfied_constraints: outer_satisfied,
                    cost: step_cost(outer, &outer.produces.local_symbol),
                    rejected_alternatives: Vec::new(),
                };
                let cost = add_cost(inner_plan.cost, outer_plan.cost);
                plans.push(DerivationPlan {
                    steps: vec![inner_plan, outer_plan],
                    intermediate_bindings: vec![intermediate_binding],
                    combined_constraints: inner
                        .preconditions
                        .iter()
                        .chain(outer.preconditions.iter())
                        .cloned()
                        .collect(),
                    cost,
                });
            }
        }
        if plans.is_empty() {
            return PlanSelection::None(rejected);
        }
        plans.sort_by_key(|plan| plan.cost);
        let best_cost = plans[0].cost;
        plans.retain(|plan| plan.cost == best_cost);
        match plans.len() {
            1 => PlanSelection::Unique(plans.remove(0)),
            _ => PlanSelection::Ambiguous(plans),
        }
    }
}

fn rejected_candidate(edge: &DerivationEdge, reason: CandidateRejection) -> RejectedCandidateTrace {
    RejectedCandidateTrace {
        method_id: edge.method_id.clone(),
        edge_id: edge.id.clone(),
        reason,
    }
}

fn constraint_symbol(constraint: &MethodConstraint) -> &str {
    match constraint {
        MethodConstraint::NonNegative(symbol) | MethodConstraint::NonZero(symbol) => symbol,
    }
}

fn explicit_assumptions(
    problem: &StructuredProblem,
    method: &MethodSpec,
) -> Result<Vec<EstablishedAssumption>, CandidateRejection> {
    if method
        .assumptions
        .iter()
        .any(|assumption| assumption_is_contradicted(assumption, problem))
    {
        return Err(CandidateRejection::ContradictedAssumption);
    }
    method
        .assumptions
        .iter()
        .map(|assumption| {
            problem
                .assumptions
                .iter()
                .find(|source| assumption_is_explicit(assumption, std::slice::from_ref(*source)))
                .map(|source| EstablishedAssumption {
                    assumption: assumption.clone(),
                    source: source.clone(),
                })
                .ok_or(CandidateRejection::MissingAssumption)
        })
        .collect()
}

fn explicit_handoff_assumptions(
    problem: &StructuredProblem,
    edge: &DerivationEdge,
) -> Result<Vec<EstablishedAssumption>, CandidateRejection> {
    if edge
        .handoff_assumptions
        .iter()
        .any(|assumption| assumption_is_contradicted(assumption, problem))
    {
        return Err(CandidateRejection::ContradictedAssumption);
    }
    edge.handoff_assumptions
        .iter()
        .map(|assumption| {
            if assumption_is_explicit_in_problem(assumption, problem) {
                Ok(EstablishedAssumption {
                    assumption: assumption.clone(),
                    source: problem.stem.clone(),
                })
            } else {
                Err(CandidateRejection::MissingAssumption)
            }
        })
        .collect()
}

/// Bind all available explicit quantities, preserving a partial binding and
/// the missing semantic inputs so the depth-two caller can choose one
/// intermediate deliberately.
fn bind_known_inputs<'a>(
    required: &[VariableSpec],
    known: &[(&'a ProblemQuantity, SemanticVariable)],
) -> Result<Vec<VariableBinding>, (Vec<VariableBinding>, Vec<VariableSpec>, CandidateRejection)> {
    let mut bindings = Vec::new();
    let mut missing = Vec::new();
    for required in required {
        let semantic_matches: Vec<_> = known
            .iter()
            .filter(|(_, semantic)| *semantic == required.semantic)
            .collect();
        if semantic_matches.is_empty() {
            missing.push(required.clone());
            continue;
        }
        let dimensional_matches: Vec<_> = semantic_matches
            .into_iter()
            .filter(|(given, _)| {
                unit_dimensions(given.unit.as_deref()) == Some(required.dimensions)
            })
            .collect();
        if dimensional_matches.is_empty() {
            return Err((bindings, missing, CandidateRejection::DimensionMismatch));
        }
        if dimensional_matches.len() != 1 {
            return Err((bindings, missing, CandidateRejection::AmbiguousBinding));
        }
        let (given, _) = dimensional_matches[0];
        bindings.push(VariableBinding {
            method_symbol: required.local_symbol.clone(),
            problem_variable: given.variable.clone(),
            value: given.value.clone(),
            unit: given.unit.clone(),
            source: given.source.clone(),
        });
    }
    if missing.is_empty() {
        Ok(bindings)
    } else {
        Err((bindings, missing, CandidateRejection::MissingSemanticInput))
    }
}

fn satisfied_constraints(
    constraints: &[MethodConstraint],
    bindings: &[VariableBinding],
) -> Result<Vec<SatisfiedConstraint>, CandidateRejection> {
    let mut satisfied = Vec::new();
    for constraint in constraints {
        let Some(binding) = bindings
            .iter()
            .find(|binding| binding.method_symbol == constraint_symbol(constraint))
        else {
            return Err(CandidateRejection::UnsatisfiedConstraint);
        };
        let Some(value) = binding
            .value
            .parse::<f64>()
            .ok()
            .filter(|value| value.is_finite())
        else {
            return Err(CandidateRejection::UnsatisfiedConstraint);
        };
        let holds = match constraint {
            MethodConstraint::NonNegative(_) => value >= 0.0,
            MethodConstraint::NonZero(_) => value != 0.0,
        };
        if !holds {
            return Err(CandidateRejection::UnsatisfiedConstraint);
        }
        satisfied.push(SatisfiedConstraint {
            constraint: constraint.clone(),
            evidence: format!(
                "{} = {} from {}",
                binding.method_symbol, binding.value, binding.source
            ),
        });
    }
    Ok(satisfied)
}

fn step_cost(edge: &DerivationEdge, output: &str) -> PlanCost {
    PlanCost {
        method_steps: 1,
        inversions: usize::from(edge.relation.split('=').next().map(str::trim) != Some(output)),
        ..PlanCost::default()
    }
}

fn add_cost(left: PlanCost, right: PlanCost) -> PlanCost {
    PlanCost {
        unresolved_ambiguities: left.unresolved_ambiguities + right.unresolved_ambiguities,
        inferred_assumptions: left.inferred_assumptions + right.inferred_assumptions,
        weak_bindings: left.weak_bindings + right.weak_bindings,
        method_steps: left.method_steps + right.method_steps,
        inversions: left.inversions + right.inversions,
        unit_conversions: left.unit_conversions + right.unit_conversions,
        numeric_approximations: left.numeric_approximations + right.numeric_approximations,
    }
}

fn unit_dimensions(unit: Option<&str>) -> Option<Dimensions> {
    match unit?.trim().to_ascii_lowercase().as_str() {
        "m" | "km" | "cm" | "mm" => Some(Dimensions::LENGTH),
        "s" | "ms" | "min" | "h" | "hr" | "hour" | "hours" => Some(Dimensions::TIME),
        "kg" | "g" => Some(Dimensions::MASS),
        "n" => Some(Dimensions::FORCE),
        "m/s" | "km/h" | "kmh" => Some(Dimensions::VELOCITY),
        "m/s2" | "m/s^2" | "m/s²" => Some(Dimensions::ACCELERATION),
        "j" => Some(Dimensions::ENERGY),
        "w" | "kw" | "mw" | "gw" => Some(Dimensions::POWER),
        _ => None,
    }
}

pub fn semantic_variable_for_name(name: &str) -> Option<SemanticVariable> {
    let concept = match name.trim() {
        "d" => QuantityConcept::Distance,
        "t" => QuantityConcept::Time,
        "m" => QuantityConcept::Mass,
        "F" | "f" => QuantityConcept::Force,
        "v" => QuantityConcept::Velocity,
        "a" => QuantityConcept::Acceleration,
        "E" | "KE" => QuantityConcept::Energy,
        "W" => QuantityConcept::Work,
        "P" | "P_mirror" => QuantityConcept::Power,
        _ => return None,
    };
    Some(SemanticVariable {
        concept,
        qualifiers: BTreeSet::new(),
    })
}

fn assumption_is_explicit(assumption: &MethodAssumption, stated: &[String]) -> bool {
    let needles: &[&str] = match assumption {
        MethodAssumption::ConstantVelocity => &["constant velocity"],
        MethodAssumption::ConstantAcceleration => &["constant acceleration"],
        MethodAssumption::ConstantForce => &["constant force"],
        MethodAssumption::CollinearForceDisplacement => &[
            "force is parallel to displacement",
            "force parallel to displacement",
        ],
        MethodAssumption::EnergyTransferredOverInterval => &["energy transferred"],
    };
    stated.iter().any(|item| {
        let item = item.to_ascii_lowercase();
        needles.iter().any(|needle| item.contains(needle))
    })
}

fn assumption_is_explicit_in_problem(
    assumption: &MethodAssumption,
    problem: &StructuredProblem,
) -> bool {
    if matches!(assumption, MethodAssumption::EnergyTransferredOverInterval) {
        let stem = problem.stem.to_ascii_lowercase();
        // Require an explicit semantic bridge.  "moves for 2 s" and
        // "loses some energy" do not establish that the full derived kinetic
        // energy was transferred during the interval.
        return [
            "kinetic energy is transferred",
            "kinetic energy was transferred",
            "kinetic energy is expended",
            "kinetic energy was expended",
            "entire kinetic energy",
            "all of its kinetic energy",
            "all its kinetic energy",
            "full kinetic energy",
        ]
        .iter()
        .any(|phrase| stem.contains(phrase));
    }
    assumption_is_explicit(assumption, &problem.assumptions)
}

/// An explicit contradiction is stronger than an absent assumption.  This is
/// checked before applicability so a positive phrase elsewhere in a prompt
/// cannot silently override a statement that rules out the method.
fn assumption_is_contradicted(assumption: &MethodAssumption, problem: &StructuredProblem) -> bool {
    let text = format!(
        "{} {}",
        problem.stem.to_ascii_lowercase(),
        problem.contradictions.join(" ").to_ascii_lowercase()
    );
    match assumption {
        MethodAssumption::ConstantVelocity => [
            "velocity changes",
            "speed changes",
            "accelerates",
            "decelerates",
            "variable velocity",
        ]
        .iter()
        .any(|phrase| text.contains(phrase)),
        MethodAssumption::ConstantForce => [
            "force varies",
            "force is variable",
            "variable force",
            "changing force",
        ]
        .iter()
        .any(|phrase| text.contains(phrase)),
        MethodAssumption::CollinearForceDisplacement => [
            "force is perpendicular to displacement",
            "force perpendicular to displacement",
            "perpendicular force",
        ]
        .iter()
        .any(|phrase| text.contains(phrase)),
        MethodAssumption::EnergyTransferredOverInterval => [
            "some kinetic energy",
            "half the kinetic energy",
            "half of the kinetic energy",
            "only part of the kinetic energy",
            "unknown fraction of the kinetic energy",
        ]
        .iter()
        .any(|phrase| text.contains(phrase)),
        MethodAssumption::ConstantAcceleration => ["acceleration changes", "variable acceleration"]
            .iter()
            .any(|phrase| text.contains(phrase)),
    }
}

/// The current extractor intentionally does not yet resolve entity and time
/// interval IDs.  Refuse a composition when prose explicitly names multiple
/// possible owners instead of silently binding A's force to B's displacement.
fn entity_or_interval_identity_ambiguous(problem: &StructuredProblem) -> bool {
    let text = problem.stem.to_ascii_lowercase();
    (text.contains("object a") && text.contains("object b"))
        || text.contains("different object")
        || text.contains("different interval")
        || (text.contains("at t0") && text.contains("at t1"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::router::QuestionRouter;

    #[test]
    fn newton_relation_compiles_only_declared_safe_directions() {
        let registry = MethodRegistry::mechanics_island();
        let method = registry
            .methods()
            .iter()
            .find(|method| method.id.0 == "mechanics.newton_second_law")
            .unwrap();
        let edges = method.derivation_edges();
        assert_eq!(edges.len(), 3);
        let acceleration = edges
            .iter()
            .find(|edge| edge.produces.local_symbol == "a")
            .unwrap();
        assert!(acceleration
            .preconditions
            .contains(&MethodConstraint::NonZero("m".to_string())));
    }

    #[test]
    fn single_step_plan_binds_semantics_not_formula_letters() {
        let problem = QuestionRouter::extract_problem(
            "A 2 kg object is acted on by a 10 N force. What is its acceleration?",
            Tool::Physics,
            Vec::new(),
        );
        let result = MethodRegistry::mechanics_island().plan_single_step(&problem);
        let SingleStepPlanResult::Planned(plan) = result else {
            panic!("Newton plan should be applicable");
        };
        assert_eq!(plan.edge.method_id.0, "mechanics.newton_second_law");
        assert_eq!(plan.bindings.len(), 2);
    }

    #[test]
    fn method_with_unstated_assumption_is_not_applicable() {
        let mut problem = QuestionRouter::extract_problem(
            "A car travels at 3 m/s for 4 s. What is the distance?",
            Tool::Physics,
            Vec::new(),
        );
        problem.requested = Some("d".to_string());
        let result = MethodRegistry::mechanics_island().plan_single_step(&problem);
        assert!(
            matches!(result, SingleStepPlanResult::NoApplicableMethod(ref rejected)
            if rejected.iter().any(|candidate| candidate.reason == CandidateRejection::MissingAssumption)),
            "{result:?}"
        );
    }

    #[test]
    fn semantic_match_with_wrong_dimensions_is_rejected() {
        let mut problem = QuestionRouter::extract_problem(
            "A 2 kg object is acted on by a 10 N force. What is its acceleration?",
            Tool::Physics,
            Vec::new(),
        );
        problem
            .givens
            .iter_mut()
            .find(|given| given.variable == "F")
            .expect("force given")
            .unit = Some("J".to_string());
        let result = MethodRegistry::mechanics_island().plan_single_step(&problem);
        assert!(
            matches!(result, SingleStepPlanResult::NoApplicableMethod(ref rejected)
            if rejected.iter().any(|candidate| candidate.reason == CandidateRejection::DimensionMismatch)),
            "{result:?}"
        );
    }

    #[test]
    fn planned_edge_carries_source_bindings_and_rejected_alternatives() {
        let problem = QuestionRouter::extract_problem(
            "A 2 kg object is acted on by a 10 N force. What is its acceleration?",
            Tool::Physics,
            Vec::new(),
        );
        let SingleStepPlanResult::Planned(plan) =
            MethodRegistry::mechanics_island().plan_single_step(&problem)
        else {
            panic!("expected a Newton plan");
        };
        assert!(plan.edge.id.0.ends_with("solve_a"));
        assert!(plan
            .bindings
            .iter()
            .all(|binding| !binding.source.is_empty()));
        assert!(plan
            .rejected_alternatives
            .iter()
            .any(|candidate| { candidate.reason == CandidateRejection::TargetMismatch }));
    }

    #[test]
    fn depth_two_planner_binds_one_intermediate_and_preserves_identity() {
        let problem = QuestionRouter::extract_problem(
            "A 2 kg object moves at 3 m/s for 4 s. Its entire kinetic energy is transferred over the interval. What is the power?",
            Tool::Physics,
            Vec::new(),
        );
        let result =
            MethodRegistry::mechanics_island().plan_depth_two(&problem, PlannerLimits::default());
        let PlanSelection::Unique(plan) = result else {
            panic!("expected one kinetic-energy -> power plan, got {result:?}");
        };
        assert_eq!(plan.steps.len(), 2);
        assert_eq!(plan.cost.method_steps, 2);
        assert_eq!(plan.intermediate_bindings.len(), 1);
        let intermediate = &plan.intermediate_bindings[0];
        assert_eq!(intermediate.quantity.concept, QuantityConcept::Energy);
        assert_eq!(
            intermediate.produced_by.0,
            "mechanics.kinetic_energy::solve_E"
        );
        assert_eq!(intermediate.consumed_by.0, "mechanics.power::solve_P");
        assert!(plan.steps[1]
            .bindings
            .iter()
            .any(|binding| binding.problem_variable == "<derived intermediate>"));
    }

    #[test]
    fn depth_two_rejects_multiple_unresolved_subgoals() {
        let problem = QuestionRouter::extract_problem(
            "What is the power of an object?",
            Tool::Physics,
            Vec::new(),
        );
        assert!(matches!(
            MethodRegistry::mechanics_island().plan_depth_two(&problem, PlannerLimits::default()),
            PlanSelection::None(_)
        ));
    }

    #[test]
    fn constant_velocity_requires_the_assumption_not_a_velocity_value() {
        for stem in [
            "A car moves at 5 m/s for 4 s. What distance does it travel?",
            "A car has a velocity of 5 m/s at t = 2 s. What distance does it travel?",
            "A car travels at a constant 5 m/s for 4 s. What distance does it travel?",
        ] {
            let problem = QuestionRouter::extract_problem(stem, Tool::Physics, Vec::new());
            let result = MethodRegistry::mechanics_island().plan_single_step(&problem);
            assert!(
                matches!(result, SingleStepPlanResult::NoApplicableMethod(ref rejected)
                if rejected.iter().any(|candidate| candidate.reason == CandidateRejection::MissingAssumption)),
                "unsafe constant-velocity inference for {stem:?}: {result:?}"
            );
        }
        let problem = QuestionRouter::extract_problem(
            "At constant velocity, a car moves at 5 m/s for 4 s. What distance does it travel?",
            Tool::Physics,
            Vec::new(),
        );
        assert!(matches!(
            MethodRegistry::mechanics_island().plan_single_step(&problem),
            SingleStepPlanResult::Planned(_)
        ));
    }
}
