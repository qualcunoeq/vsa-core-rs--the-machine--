//! Typed mathematical-method metadata and strict one-step instantiation.
//!
//! This module is deliberately an authorization layer, not a theorem search
//! engine or a CAS.  Retrieval only uses typed task/fact shapes.  A candidate
//! becomes usable only after its premise patterns bind to grounded facts and
//! all schema side conditions are discharged.  No method in this module
//! executes an expression.

use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct MathMethodId(pub String);

impl MathMethodId {
    pub fn new(id: impl Into<String>) -> Self {
        Self(id.into())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MathDomain {
    Algebra,
    NumberTheory,
    Calculus,
    Combinatorics,
    Probability,
    Geometry,
    LinearAlgebra,
    General,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MathMethodFamily {
    DefinitionApplication,
    Vieta,
    Congruence,
    Divisibility,
    Counting,
    Probability,
    Convergence,
    LinearAlgebra,
    Inequality,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TaskShape {
    ComputeExplicitValue,
    DetermineExistence,
    DetermineUniqueness,
    CountObjects,
    FindAllObjects,
    ProveIdentity,
    ProveImplication,
    BoundQuantity,
    DetermineConvergence,
    DetermineAsymptoticBehavior,
    OptimizeUnderConstraints,
    ClassifyStructure,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum FactKind {
    Equation,
    Inequality,
    Membership,
    Divides,
    Congruence,
    Definition,
    Proposition,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum FactStatus {
    Explicit,
    Normalized,
    Derived,
    Inferred,
    Ambiguous,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum FactConfidence {
    High,
    Medium,
    Low,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct FactProvenance {
    pub source_fragments: Vec<String>,
    pub source_label: String,
    pub statement_hash: String,
}

impl FactProvenance {
    pub fn explicit(fragment: impl Into<String>) -> Self {
        let fragment = fragment.into();
        Self {
            statement_hash: stable_hash(&fragment),
            source_fragments: vec![fragment],
            source_label: "prompt".to_string(),
        }
    }
}

/// The fact language is intentionally structural.  Expressions remain text
/// until an executor with a typed contract is selected; they are never
/// executed merely because they resemble a theorem statement.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub enum MathematicalFact {
    Equation {
        lhs: String,
        rhs: String,
    },
    Inequality {
        lhs: String,
        relation: String,
        rhs: String,
    },
    Membership {
        element: String,
        set: String,
    },
    Divides {
        divisor: String,
        dividend: String,
    },
    Congruence {
        lhs: String,
        rhs: String,
        modulus: String,
    },
    Definition {
        symbol: String,
        expression: String,
    },
    Proposition {
        statement: String,
    },
}

impl MathematicalFact {
    pub fn kind(&self) -> FactKind {
        match self {
            Self::Equation { .. } => FactKind::Equation,
            Self::Inequality { .. } => FactKind::Inequality,
            Self::Membership { .. } => FactKind::Membership,
            Self::Divides { .. } => FactKind::Divides,
            Self::Congruence { .. } => FactKind::Congruence,
            Self::Definition { .. } => FactKind::Definition,
            Self::Proposition { .. } => FactKind::Proposition,
        }
    }

    fn text(&self) -> String {
        match self {
            Self::Equation { lhs, rhs } => format!("{lhs}={rhs}"),
            Self::Inequality { lhs, relation, rhs } => format!("{lhs}{relation}{rhs}"),
            Self::Membership { element, set } => format!("{element}∈{set}"),
            Self::Divides { divisor, dividend } => format!("{divisor}|{dividend}"),
            Self::Congruence { lhs, rhs, modulus } => format!("{lhs}≡{rhs} mod {modulus}"),
            Self::Definition { symbol, expression } => format!("{symbol}:={expression}"),
            Self::Proposition { statement } => statement.clone(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct GroundedFact {
    pub id: String,
    pub fact: MathematicalFact,
    /// Parser-provided semantic bindings.  A method may bind only names
    /// present here; it never invents a symbol from a text substring.
    pub bindings: BTreeMap<String, String>,
    pub provenance: FactProvenance,
    pub confidence: FactConfidence,
    pub status: FactStatus,
}

impl GroundedFact {
    pub fn explicit(
        id: impl Into<String>,
        fact: MathematicalFact,
        bindings: BTreeMap<String, String>,
    ) -> Self {
        Self {
            id: id.into(),
            provenance: FactProvenance::explicit("prompt"),
            fact,
            bindings,
            confidence: FactConfidence::High,
            status: FactStatus::Explicit,
        }
    }

    pub fn is_authoritative(&self) -> bool {
        matches!(
            self.status,
            FactStatus::Explicit | FactStatus::Normalized | FactStatus::Derived
        ) && !matches!(self.confidence, FactConfidence::Low)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub enum MathAssumption {
    RealDomain,
    IntegerDomain,
    Nonzero(String),
    Positive(String),
    Coprime(String, String),
    UniformFiniteSpace,
    Explicit(String),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub enum SideCondition {
    Bound(String),
    Nonzero(String),
    Positive(String),
    Domain(String),
    Coprime(String, String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum MathCapability {
    NormalizeFact,
    BindVariables,
    SymbolicAlgebra,
    IntegerArithmetic,
    ProofReplay,
    CheckDomain,
    CheckSideConditions,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum VerificationStrategy {
    SubstituteAndReplay,
    IdentityCheck,
    ModularReplay,
    IndependentCountingIdentity,
    DomainCheckOnly,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct MethodProvenance {
    pub source_id: String,
    pub source_location: String,
    pub statement_hash: String,
    pub curator: String,
    pub audit_status: String,
    pub canonical_statement: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct PremisePattern {
    pub kind: FactKind,
    /// Names introduced by this premise, written as `$x` in the textual
    /// pattern.  Binding is exact and is later required to be consistent.
    pub binders: BTreeSet<String>,
    /// Optional exact tokens that must occur in the normalized fact text.
    /// This is a structural guard, not semantic similarity search.
    pub required_tokens: BTreeSet<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ConclusionPattern {
    pub kind: FactKind,
    pub template: String,
    pub binders: BTreeSet<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct MathematicalMethodSpec {
    pub id: MathMethodId,
    pub domain: MathDomain,
    pub family: MathMethodFamily,
    pub task_shapes: BTreeSet<TaskShape>,
    pub premise_patterns: Vec<PremisePattern>,
    pub conclusion_patterns: Vec<ConclusionPattern>,
    pub required_assumptions: Vec<MathAssumption>,
    pub side_conditions: Vec<SideCondition>,
    pub produced_fact_kinds: BTreeSet<FactKind>,
    pub required_capabilities: BTreeSet<MathCapability>,
    pub verification_strategy: VerificationStrategy,
    pub provenance: MethodProvenance,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MethodSchemaError {
    DuplicateMethodId,
    MissingProvenance,
    NoPremises,
    NoConclusions,
    DuplicatePremiseBinder(String),
    UnboundConclusionBinder(String),
    SideConditionUnboundVariable(String),
    ProducedKindMismatch,
    MalformedConclusionTemplate,
}

impl MathematicalMethodSpec {
    pub fn validate(&self) -> Result<(), Vec<MethodSchemaError>> {
        let mut errors = Vec::new();
        if self.provenance.source_id.trim().is_empty()
            || self.provenance.statement_hash.trim().is_empty()
            || self.provenance.canonical_statement.trim().is_empty()
        {
            errors.push(MethodSchemaError::MissingProvenance);
        }
        if self.premise_patterns.is_empty() {
            errors.push(MethodSchemaError::NoPremises);
        }
        if self.conclusion_patterns.is_empty() {
            errors.push(MethodSchemaError::NoConclusions);
        }
        let mut bound = BTreeSet::new();
        for premise in &self.premise_patterns {
            for binder in &premise.binders {
                // A repeated binder across premises is intentional: it is a
                // shared metavariable whose value must agree during
                // instantiation.  It is therefore not a schema error.
                bound.insert(binder.clone());
            }
        }
        for conclusion in &self.conclusion_patterns {
            if !self.produced_fact_kinds.contains(&conclusion.kind) {
                errors.push(MethodSchemaError::ProducedKindMismatch);
            }
            let schema_template = substitute_template(
                &conclusion.template,
                &conclusion
                    .binders
                    .iter()
                    .map(|name| (name.clone(), "v".to_string()))
                    .collect(),
            );
            if fact_from_template(conclusion.kind, &schema_template).is_none() {
                errors.push(MethodSchemaError::MalformedConclusionTemplate);
            }
            for binder in &conclusion.binders {
                if !bound.contains(binder) {
                    errors.push(MethodSchemaError::UnboundConclusionBinder(binder.clone()));
                }
            }
        }
        for condition in &self.side_conditions {
            for variable in side_condition_variables(condition) {
                if !bound.contains(&variable) {
                    errors.push(MethodSchemaError::SideConditionUnboundVariable(variable));
                }
            }
        }
        if errors.is_empty() {
            Ok(())
        } else {
            Err(errors)
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct MethodQuery {
    pub domain: MathDomain,
    pub task_shape: TaskShape,
    pub premise_kinds: BTreeSet<FactKind>,
    pub target_kind: FactKind,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct RetrievedMethodCandidate {
    pub method_id: MathMethodId,
    pub structural_score: u32,
    pub retrieval_evidence: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum PlanSelectionKind {
    Unique,
    Consensus,
    Ambiguous,
    None,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub enum MethodRejection {
    TaskShapeMismatch,
    TargetKindMismatch,
    MissingPremise,
    AmbiguousPremiseBinding,
    ConflictingPremiseBinding,
    MissingDefinition,
    MissingAssumption,
    ContradictedAssumption,
    SideConditionUnresolved,
    DomainMismatch,
    UnsupportedInstantiation,
    UnsupportedExecutionCapability,
    VerificationUnavailable,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct PremiseBinding {
    pub pattern_index: usize,
    pub fact_id: String,
    pub substitutions: BTreeMap<String, String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct EstablishedCondition {
    pub condition: String,
    pub source_fact_ids: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct MathematicalDerivationStep {
    pub method_id: MathMethodId,
    pub instantiated_premises: Vec<String>,
    pub discharged_conditions: Vec<EstablishedCondition>,
    pub produced_facts: Vec<GroundedFact>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct MathematicalPlanCost {
    pub method_steps: usize,
    pub unresolved_obligations: usize,
    pub inferred_premises: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct MathematicalDerivationPlan {
    pub initial_facts: Vec<String>,
    pub steps: Vec<MathematicalDerivationStep>,
    pub target_fact: Option<String>,
    pub unresolved: Vec<MethodRejection>,
    pub cost: MathematicalPlanCost,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct MethodInstantiation {
    pub method_id: MathMethodId,
    pub premise_bindings: Vec<PremiseBinding>,
    pub substitutions: BTreeMap<String, String>,
    pub established_assumptions: Vec<MathAssumption>,
    pub discharged_side_conditions: Vec<EstablishedCondition>,
    pub produced_facts: Vec<MathematicalFact>,
    pub unresolved_side_conditions: Vec<SideCondition>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct PlannerRejection {
    pub method_id: MathMethodId,
    pub reason: MethodRejection,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct OneStepPlan {
    pub selection: PlanSelectionKind,
    pub instantiations: Vec<MethodInstantiation>,
    pub rejected_candidates: Vec<PlannerRejection>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MathematicalPlannerLimits {
    pub max_candidates: usize,
    pub allow_inferred_premises: bool,
    pub allow_unverified_intermediates: bool,
}

impl Default for MathematicalPlannerLimits {
    fn default() -> Self {
        Self {
            max_candidates: 8,
            allow_inferred_premises: false,
            allow_unverified_intermediates: false,
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct MathematicalMethodRegistry {
    methods: Vec<MathematicalMethodSpec>,
}

impl MathematicalMethodRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn register(
        &mut self,
        method: MathematicalMethodSpec,
    ) -> Result<(), Vec<MethodSchemaError>> {
        method.validate()?;
        if self.methods.iter().any(|existing| existing.id == method.id) {
            return Err(vec![MethodSchemaError::DuplicateMethodId]);
        }
        self.methods.push(method);
        Ok(())
    }

    pub fn methods(&self) -> &[MathematicalMethodSpec] {
        &self.methods
    }

    /// Structural retrieval only.  The score is intentionally explainable and
    /// uses no embedding or text similarity.  A candidate is never executable
    /// until [`Self::instantiate`] succeeds.
    pub fn retrieve(&self, query: &MethodQuery) -> Vec<RetrievedMethodCandidate> {
        let mut out = Vec::new();
        for method in &self.methods {
            if method.domain != query.domain || !method.task_shapes.contains(&query.task_shape) {
                continue;
            }
            if !method.produced_fact_kinds.contains(&query.target_kind) {
                continue;
            }
            let kinds: BTreeSet<_> = method.premise_patterns.iter().map(|p| p.kind).collect();
            if !query.premise_kinds.is_subset(&kinds) {
                continue;
            }
            let score =
                10 + query.premise_kinds.len() as u32 + method.premise_patterns.len() as u32;
            out.push(RetrievedMethodCandidate {
                method_id: method.id.clone(),
                structural_score: score,
                retrieval_evidence: vec!["domain/task/fact-shape match".to_string()],
            });
        }
        out.sort_by(|a, b| {
            b.structural_score
                .cmp(&a.structural_score)
                .then_with(|| a.method_id.cmp(&b.method_id))
        });
        out
    }

    /// Plan exactly one method application.  Retrieval remains a proposal
    /// stage; every candidate must still instantiate against authoritative
    /// facts and explicit obligations.  Distinct successful methods are never
    /// resolved by registry order: equivalent produced facts yield consensus,
    /// while disagreement is an ambiguity.
    pub fn plan_one_step(
        &self,
        query: &MethodQuery,
        facts: &[GroundedFact],
        assumptions: &[MathAssumption],
        conditions: &[EstablishedCondition],
        limits: MathematicalPlannerLimits,
    ) -> OneStepPlan {
        let candidates = self.retrieve(query);
        let mut instantiations = Vec::new();
        let mut rejected_candidates = Vec::new();
        for candidate in candidates.into_iter().take(limits.max_candidates) {
            match self.instantiate_with_context(
                &candidate.method_id,
                facts,
                assumptions,
                conditions,
            ) {
                Ok(instantiation) => instantiations.push(instantiation),
                Err(reason) => rejected_candidates.push(PlannerRejection {
                    method_id: candidate.method_id,
                    reason,
                }),
            }
        }
        let selection = match instantiations.len() {
            0 => PlanSelectionKind::None,
            1 => PlanSelectionKind::Unique,
            _ => {
                let first = &instantiations[0].produced_facts;
                if instantiations
                    .iter()
                    .all(|candidate| candidate.produced_facts == *first)
                {
                    PlanSelectionKind::Consensus
                } else {
                    PlanSelectionKind::Ambiguous
                }
            }
        };
        OneStepPlan {
            selection,
            instantiations,
            rejected_candidates,
        }
    }

    /// Strict single-step instantiation.  Facts must be authoritative and
    /// each premise must have exactly one matching grounded fact.
    pub fn instantiate(
        &self,
        method_id: &MathMethodId,
        facts: &[GroundedFact],
    ) -> Result<MethodInstantiation, MethodRejection> {
        self.instantiate_with_context(method_id, facts, &[], &[])
    }

    /// Instantiate with explicit evidence for assumptions and side
    /// conditions.  A method never gets authority merely because its schema
    /// lists a condition; the caller must establish it in the context.
    pub fn instantiate_with_context(
        &self,
        method_id: &MathMethodId,
        facts: &[GroundedFact],
        assumptions: &[MathAssumption],
        conditions: &[EstablishedCondition],
    ) -> Result<MethodInstantiation, MethodRejection> {
        let method = self
            .methods
            .iter()
            .find(|m| &m.id == method_id)
            .ok_or(MethodRejection::UnsupportedInstantiation)?;
        if method
            .required_assumptions
            .iter()
            .any(|required| !assumptions.contains(required))
        {
            return Err(MethodRejection::MissingAssumption);
        }
        let mut bindings = Vec::new();
        let mut substitutions = BTreeMap::new();
        for (index, pattern) in method.premise_patterns.iter().enumerate() {
            let matches: Vec<_> = facts
                .iter()
                .filter(|fact| {
                    fact.is_authoritative()
                        && fact.fact.kind() == pattern.kind
                        && pattern.required_tokens.iter().all(|token| {
                            normalize(token)
                                .split_whitespace()
                                .all(|part| normalize(&fact.fact.text()).contains(part))
                        })
                })
                .collect();
            if matches.is_empty() {
                return Err(MethodRejection::MissingPremise);
            }
            if matches.len() > 1 {
                return Err(MethodRejection::AmbiguousPremiseBinding);
            }
            let fact = matches[0];
            let mut local = BTreeMap::new();
            for binder in &pattern.binders {
                let value = fact
                    .bindings
                    .get(binder)
                    .ok_or(MethodRejection::UnsupportedInstantiation)?;
                if let Some(previous) = substitutions.get(binder) {
                    if previous != value {
                        return Err(MethodRejection::ConflictingPremiseBinding);
                    }
                }
                substitutions.insert(binder.clone(), value.clone());
                local.insert(binder.clone(), value.clone());
            }
            bindings.push(PremiseBinding {
                pattern_index: index,
                fact_id: fact.id.clone(),
                substitutions: local,
            });
        }
        let produced_facts = method
            .conclusion_patterns
            .iter()
            .map(|pattern| {
                let template = substitute_template(&pattern.template, &substitutions);
                fact_from_template(pattern.kind, &template)
                    .ok_or(MethodRejection::UnsupportedInstantiation)
            })
            .collect::<Result<Vec<_>, _>>()?;
        if method.side_conditions.iter().any(|required| {
            !conditions
                .iter()
                .any(|given| given.condition == format_condition(required))
        }) {
            return Err(MethodRejection::SideConditionUnresolved);
        }
        Ok(MethodInstantiation {
            method_id: method.id.clone(),
            premise_bindings: bindings,
            substitutions,
            established_assumptions: assumptions.to_vec(),
            discharged_side_conditions: conditions.to_vec(),
            produced_facts,
            unresolved_side_conditions: Vec::new(),
        })
    }

    /// Convert an authorized instantiation into an auditable derivation step.
    /// This deliberately leaves the produced facts unverified: a separate
    /// executor/verifier must attach independent replay evidence before a
    /// plan can answer a question.
    pub fn derivation_step(
        &self,
        instantiation: &MethodInstantiation,
        source_facts: &[GroundedFact],
    ) -> MathematicalDerivationStep {
        let produced_facts = instantiation
            .produced_facts
            .iter()
            .enumerate()
            .map(|(index, fact)| GroundedFact {
                id: format!("{}:derived:{index}", instantiation.method_id.0),
                fact: fact.clone(),
                bindings: instantiation.substitutions.clone(),
                provenance: FactProvenance {
                    source_fragments: source_facts
                        .iter()
                        .flat_map(|source| source.provenance.source_fragments.clone())
                        .collect(),
                    source_label: format!("method:{}", instantiation.method_id.0),
                    statement_hash: stable_hash(&fact.text()),
                },
                confidence: FactConfidence::High,
                status: FactStatus::Derived,
            })
            .collect();
        MathematicalDerivationStep {
            method_id: instantiation.method_id.clone(),
            instantiated_premises: instantiation
                .premise_bindings
                .iter()
                .map(|binding| binding.fact_id.clone())
                .collect(),
            discharged_conditions: instantiation.discharged_side_conditions.clone(),
            produced_facts,
        }
    }
}

fn side_condition_variables(condition: &SideCondition) -> Vec<String> {
    match condition {
        SideCondition::Bound(name)
        | SideCondition::Nonzero(name)
        | SideCondition::Positive(name)
        | SideCondition::Domain(name) => vec![name.clone()],
        SideCondition::Coprime(a, b) => vec![a.clone(), b.clone()],
    }
}

fn normalize(value: &str) -> String {
    value
        .split_whitespace()
        .collect::<String>()
        .to_ascii_lowercase()
}

fn substitute_template(template: &str, substitutions: &BTreeMap<String, String>) -> String {
    substitutions
        .iter()
        .fold(template.to_string(), |text, (name, value)| {
            text.replace(&format!("${name}"), value)
        })
}

fn format_condition(condition: &SideCondition) -> String {
    match condition {
        SideCondition::Bound(v) => format!("bound:{v}"),
        SideCondition::Nonzero(v) => format!("nonzero:{v}"),
        SideCondition::Positive(v) => format!("positive:{v}"),
        SideCondition::Domain(v) => format!("domain:{v}"),
        SideCondition::Coprime(a, b) => format!("coprime:{a}:{b}"),
    }
}

fn fact_from_template(kind: FactKind, template: &str) -> Option<MathematicalFact> {
    match kind {
        FactKind::Equation => {
            let (lhs, rhs) = template.split_once('=')?;
            Some(MathematicalFact::Equation {
                lhs: lhs.trim().into(),
                rhs: rhs.trim().into(),
            })
        }
        FactKind::Inequality => {
            for relation in ["<=", ">=", "<", ">"] {
                if let Some((lhs, rhs)) = template.split_once(relation) {
                    return Some(MathematicalFact::Inequality {
                        lhs: lhs.trim().into(),
                        relation: relation.into(),
                        rhs: rhs.trim().into(),
                    });
                }
            }
            None
        }
        FactKind::Membership => {
            let (element, set) = template.split_once('∈')?;
            Some(MathematicalFact::Membership {
                element: element.trim().into(),
                set: set.trim().into(),
            })
        }
        FactKind::Divides => {
            let (divisor, dividend) = template.split_once('|')?;
            Some(MathematicalFact::Divides {
                divisor: divisor.trim().into(),
                dividend: dividend.trim().into(),
            })
        }
        FactKind::Congruence => {
            let (lhs, rest) = template.split_once('≡')?;
            let (rhs, modulus) = rest.split_once(" mod ")?;
            Some(MathematicalFact::Congruence {
                lhs: lhs.trim().into(),
                rhs: rhs.trim().into(),
                modulus: modulus.trim().into(),
            })
        }
        FactKind::Definition => {
            let (symbol, expression) = template.split_once(":=")?;
            Some(MathematicalFact::Definition {
                symbol: symbol.trim().into(),
                expression: expression.trim().into(),
            })
        }
        FactKind::Proposition => Some(MathematicalFact::Proposition {
            statement: template.into(),
        }),
    }
}

fn stable_hash(value: &str) -> String {
    let mut hash: u64 = 1469598103934665603;
    for byte in value.as_bytes() {
        hash ^= *byte as u64;
        hash = hash.wrapping_mul(1099511628211);
    }
    format!("{hash:016x}")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn definition_method() -> MathematicalMethodSpec {
        MathematicalMethodSpec {
            id: MathMethodId::new("algebra.definition.substitute"),
            domain: MathDomain::Algebra,
            family: MathMethodFamily::DefinitionApplication,
            task_shapes: BTreeSet::from([TaskShape::ComputeExplicitValue]),
            premise_patterns: vec![PremisePattern {
                kind: FactKind::Definition,
                binders: BTreeSet::from(["f".to_string(), "x".to_string(), "expr".to_string()]),
                required_tokens: BTreeSet::new(),
            }],
            conclusion_patterns: vec![ConclusionPattern {
                kind: FactKind::Proposition,
                template: "$f($x) = $expr".to_string(),
                binders: BTreeSet::from(["f".to_string(), "x".to_string(), "expr".to_string()]),
            }],
            required_assumptions: vec![],
            side_conditions: vec![],
            produced_fact_kinds: BTreeSet::from([FactKind::Proposition]),
            required_capabilities: BTreeSet::from([MathCapability::BindVariables]),
            verification_strategy: VerificationStrategy::IdentityCheck,
            provenance: MethodProvenance {
                source_id: "curated.test".into(),
                source_location: "test:1".into(),
                statement_hash: "abc".into(),
                curator: "test".into(),
                audit_status: "audited".into(),
                canonical_statement: "definition application".into(),
            },
        }
    }

    #[test]
    fn schema_rejects_unbound_conclusion_variable() {
        let mut method = definition_method();
        method.conclusion_patterns[0].binders.insert("y".into());
        assert!(
            matches!(method.validate(), Err(errors) if errors.iter().any(|e| matches!(e, MethodSchemaError::UnboundConclusionBinder(name) if name == "y")))
        );
    }

    #[test]
    fn retrieval_is_structural_and_does_not_execute() {
        let mut registry = MathematicalMethodRegistry::new();
        registry.register(definition_method()).unwrap();
        let candidates = registry.retrieve(&MethodQuery {
            domain: MathDomain::Algebra,
            task_shape: TaskShape::ComputeExplicitValue,
            premise_kinds: BTreeSet::from([FactKind::Definition]),
            target_kind: FactKind::Proposition,
        });
        assert_eq!(candidates.len(), 1);
        assert_eq!(
            candidates[0].retrieval_evidence,
            vec!["domain/task/fact-shape match"]
        );
    }

    #[test]
    fn instantiation_requires_authoritative_exact_fact_and_preserves_bindings() {
        let mut registry = MathematicalMethodRegistry::new();
        let method = definition_method();
        let id = method.id.clone();
        registry.register(method).unwrap();
        let fact = GroundedFact::explicit(
            "f1",
            MathematicalFact::Definition {
                symbol: "f".into(),
                expression: "x^2+1".into(),
            },
            BTreeMap::from([
                ("f".into(), "f".into()),
                ("x".into(), "x".into()),
                ("expr".into(), "x^2+1".into()),
            ]),
        );
        let result = registry.instantiate(&id, &[fact]).unwrap();
        assert_eq!(result.substitutions["expr"], "x^2+1");
        assert!(matches!(
            result.produced_facts[0],
            MathematicalFact::Proposition { .. }
        ));
    }

    #[test]
    fn ambiguous_premises_abstain() {
        let mut registry = MathematicalMethodRegistry::new();
        let method = definition_method();
        let id = method.id.clone();
        registry.register(method).unwrap();
        let mk = |id: &str| {
            GroundedFact::explicit(
                id,
                MathematicalFact::Definition {
                    symbol: "f".into(),
                    expression: "x".into(),
                },
                BTreeMap::from([
                    ("f".into(), "f".into()),
                    ("x".into(), "x".into()),
                    ("expr".into(), "x".into()),
                ]),
            )
        };
        assert_eq!(
            registry.instantiate(&id, &[mk("a"), mk("b")]),
            Err(MethodRejection::AmbiguousPremiseBinding)
        );
    }

    #[test]
    fn assumptions_and_side_conditions_are_not_invented() {
        let mut method = definition_method();
        method.required_assumptions = vec![MathAssumption::RealDomain];
        method.side_conditions = vec![SideCondition::Nonzero("x".into())];
        let id = method.id.clone();
        let mut registry = MathematicalMethodRegistry::new();
        registry.register(method).unwrap();
        let fact = GroundedFact::explicit(
            "f1",
            MathematicalFact::Definition {
                symbol: "f".into(),
                expression: "x".into(),
            },
            BTreeMap::from([
                ("f".into(), "f".into()),
                ("x".into(), "x".into()),
                ("expr".into(), "x".into()),
            ]),
        );
        assert_eq!(
            registry.instantiate(&id, &[fact.clone()]),
            Err(MethodRejection::MissingAssumption)
        );
        let condition = EstablishedCondition {
            condition: "nonzero:x".into(),
            source_fact_ids: vec!["f1".into()],
        };
        let result = registry
            .instantiate_with_context(&id, &[fact], &[MathAssumption::RealDomain], &[condition])
            .unwrap();
        assert_eq!(result.discharged_side_conditions.len(), 1);
        assert!(matches!(
            result.produced_facts[0],
            MathematicalFact::Proposition { .. }
        ));
    }

    #[test]
    fn conclusion_kind_is_preserved() {
        let mut method = definition_method();
        method.conclusion_patterns[0] = ConclusionPattern {
            kind: FactKind::Equation,
            template: "$x=$expr".into(),
            binders: BTreeSet::from(["x".into(), "expr".into()]),
        };
        method.produced_fact_kinds = BTreeSet::from([FactKind::Equation]);
        let id = method.id.clone();
        let mut registry = MathematicalMethodRegistry::new();
        registry.register(method).unwrap();
        let fact = GroundedFact::explicit(
            "f1",
            MathematicalFact::Definition {
                symbol: "f".into(),
                expression: "x".into(),
            },
            BTreeMap::from([
                ("f".into(), "f".into()),
                ("x".into(), "x".into()),
                ("expr".into(), "x".into()),
            ]),
        );
        let result = registry.instantiate(&id, &[fact]).unwrap();
        assert!(matches!(
            result.produced_facts[0],
            MathematicalFact::Equation { .. }
        ));
    }

    #[test]
    fn derivation_step_retains_source_provenance() {
        let mut registry = MathematicalMethodRegistry::new();
        let method = definition_method();
        let id = method.id.clone();
        registry.register(method).unwrap();
        let fact = GroundedFact::explicit(
            "f1",
            MathematicalFact::Definition {
                symbol: "f".into(),
                expression: "x".into(),
            },
            BTreeMap::from([
                ("f".into(), "f".into()),
                ("x".into(), "x".into()),
                ("expr".into(), "x".into()),
            ]),
        );
        let instantiation = registry
            .instantiate(&id, std::slice::from_ref(&fact))
            .unwrap();
        let step = registry.derivation_step(&instantiation, std::slice::from_ref(&fact));
        assert_eq!(step.instantiated_premises, vec!["f1"]);
        assert_eq!(step.produced_facts[0].status, FactStatus::Derived);
        assert_eq!(
            step.produced_facts[0].provenance.source_fragments,
            vec!["prompt"]
        );
    }

    fn definition_fact() -> GroundedFact {
        GroundedFact::explicit(
            "f1",
            MathematicalFact::Definition {
                symbol: "f".into(),
                expression: "x".into(),
            },
            BTreeMap::from([
                ("f".into(), "f".into()),
                ("x".into(), "x".into()),
                ("expr".into(), "x".into()),
            ]),
        )
    }

    fn definition_query() -> MethodQuery {
        MethodQuery {
            domain: MathDomain::Algebra,
            task_shape: TaskShape::ComputeExplicitValue,
            premise_kinds: BTreeSet::from([FactKind::Definition]),
            target_kind: FactKind::Proposition,
        }
    }

    #[test]
    fn one_step_planner_returns_unique_only_after_instantiation() {
        let mut registry = MathematicalMethodRegistry::new();
        registry.register(definition_method()).unwrap();
        let plan = registry.plan_one_step(
            &definition_query(),
            &[definition_fact()],
            &[],
            &[],
            MathematicalPlannerLimits::default(),
        );
        assert_eq!(plan.selection, PlanSelectionKind::Unique);
        assert_eq!(plan.instantiations.len(), 1);
        assert!(plan.rejected_candidates.is_empty());
    }

    #[test]
    fn one_step_planner_retains_missing_premise_rejection() {
        let mut registry = MathematicalMethodRegistry::new();
        let method = definition_method();
        let id = method.id.clone();
        registry.register(method).unwrap();
        let plan = registry.plan_one_step(
            &definition_query(),
            &[],
            &[],
            &[],
            MathematicalPlannerLimits::default(),
        );
        assert_eq!(plan.selection, PlanSelectionKind::None);
        assert_eq!(
            plan.rejected_candidates,
            vec![PlannerRejection {
                method_id: id,
                reason: MethodRejection::MissingPremise
            }]
        );
    }

    #[test]
    fn equivalent_methods_produce_consensus_not_registry_order() {
        let mut registry = MathematicalMethodRegistry::new();
        registry.register(definition_method()).unwrap();
        let mut equivalent = definition_method();
        equivalent.id = MathMethodId::new("algebra.definition.substitute.alt");
        registry.register(equivalent).unwrap();
        let plan = registry.plan_one_step(
            &definition_query(),
            &[definition_fact()],
            &[],
            &[],
            MathematicalPlannerLimits::default(),
        );
        assert_eq!(plan.selection, PlanSelectionKind::Consensus);
        assert_eq!(plan.instantiations.len(), 2);
    }

    #[test]
    fn conflicting_methods_abstain_as_ambiguous() {
        let mut registry = MathematicalMethodRegistry::new();
        registry.register(definition_method()).unwrap();
        let mut conflicting = definition_method();
        conflicting.id = MathMethodId::new("algebra.definition.conflict");
        conflicting.conclusion_patterns[0].template = "$f($x) = 0".into();
        registry.register(conflicting).unwrap();
        let plan = registry.plan_one_step(
            &definition_query(),
            &[definition_fact()],
            &[],
            &[],
            MathematicalPlannerLimits::default(),
        );
        assert_eq!(plan.selection, PlanSelectionKind::Ambiguous);
        assert_eq!(plan.instantiations.len(), 2);
    }
}
