//! Contract-driven development island for the typed mechanics solver.
//!
//! This corpus is intentionally small and explicit.  It is not a second HLE
//! score: each case states which plans are authorized and, for unsupported
//! prompts, which safe abstention is expected.  A numerically correct answer
//! produced by a forbidden plan is a contract failure.

use crate::methods::{CandidateRejection, MethodAssumption};
use crate::router::{AbstentionReason, OrchestratedAnswer, QuestionRouter};
use regex::Regex;
use serde::Serialize;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SupportStatus {
    Supported,
    MustAbstain,
}

/// Diagnostic-only support classification for real benchmark reconnaissance.
/// It never authorizes execution; only the typed planner and executor can do
/// that after a question passes the normal gates.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SupportAssessment {
    InsideSupportedContract,
    PotentiallySupportedExtractionFailed,
    RequiresUnsupportedMethod,
    RequiresUnsupportedPlanDepth,
    RequiresUnsupportedRepresentation,
    RequiredAssumptionAbsent,
    OutsideDomain,
}

impl SupportAssessment {
    pub fn label(self) -> &'static str {
        match self {
            Self::InsideSupportedContract => "inside_supported_contract",
            Self::PotentiallySupportedExtractionFailed => "potentially_supported_extraction_failed",
            Self::RequiresUnsupportedMethod => "requires_unsupported_method",
            Self::RequiresUnsupportedPlanDepth => "requires_unsupported_plan_depth",
            Self::RequiresUnsupportedRepresentation => "requires_unsupported_representation",
            Self::RequiredAssumptionAbsent => "required_assumption_absent",
            Self::OutsideDomain => "outside_domain",
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct FunnelAssessment {
    pub route: String,
    pub target: Option<String>,
    pub givens: Vec<String>,
    pub assumptions: Vec<String>,
    pub candidate_edges: Vec<String>,
    pub reachable_depth: usize,
    pub first_rejection: Option<String>,
    /// A stricter proximity signal than a keyword route: enough semantic
    /// inputs for at least one registry edge are present, even if extraction
    /// or an explicit assumption still blocks execution.
    pub near_supported_contract: bool,
    pub assessment: SupportAssessment,
}

/// A deliberately shallow, non-executing mathematics taxonomy.  This is a
/// reconnaissance artifact: it measures whether a prompt has a structured
/// task that an eventual math executor could consume.  It is never used to
/// authorize an answer.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum MathTaskKind {
    ExplicitExpressionEvaluation,
    EquationSolving,
    PolynomialAlgebra,
    FiniteCombinatorics,
    ElementaryCalculus,
    NumberTheory,
    LinearAlgebra,
    Geometry,
    ProofOrTheorem,
    DiagramDependent,
    AdvancedSpecialized,
    UnparsedMathematicalProse,
    NotMath,
}

impl MathTaskKind {
    pub fn label(self) -> &'static str {
        match self {
            Self::ExplicitExpressionEvaluation => "explicit_expression_evaluation",
            Self::EquationSolving => "equation_solving",
            Self::PolynomialAlgebra => "polynomial_algebra",
            Self::FiniteCombinatorics => "finite_combinatorics",
            Self::ElementaryCalculus => "elementary_calculus",
            Self::NumberTheory => "number_theory",
            Self::LinearAlgebra => "linear_algebra",
            Self::Geometry => "geometry",
            Self::ProofOrTheorem => "proof_or_theorem",
            Self::DiagramDependent => "diagram_dependent",
            Self::AdvancedSpecialized => "advanced_specialized",
            Self::UnparsedMathematicalProse => "unparsed_mathematical_prose",
            Self::NotMath => "not_math",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum FiniteMathTaskKind {
    ExplicitFactorial,
    ExplicitBinomial,
    OrderedSelection,
    UnorderedSelection,
    UniformFiniteProbability,
    ConditionalFiniteProbability,
    InclusionExclusion,
    ExpectationFiniteSupport,
    VarianceFiniteSupport,
    Recurrence,
    GeneratingFunction,
    AdvancedCombinatorics,
    AdvancedProbability,
    Proof,
    DomainModelingRequired,
    Unclassified,
}

impl FiniteMathTaskKind {
    pub fn label(self) -> &'static str {
        match self {
            Self::ExplicitFactorial => "explicit_factorial",
            Self::ExplicitBinomial => "explicit_binomial",
            Self::OrderedSelection => "ordered_selection",
            Self::UnorderedSelection => "unordered_selection",
            Self::UniformFiniteProbability => "uniform_finite_probability",
            Self::ConditionalFiniteProbability => "conditional_finite_probability",
            Self::InclusionExclusion => "inclusion_exclusion",
            Self::ExpectationFiniteSupport => "expectation_finite_support",
            Self::VarianceFiniteSupport => "variance_finite_support",
            Self::Recurrence => "recurrence",
            Self::GeneratingFunction => "generating_function",
            Self::AdvancedCombinatorics => "advanced_combinatorics",
            Self::AdvancedProbability => "advanced_probability",
            Self::Proof => "proof",
            Self::DomainModelingRequired => "domain_modeling_required",
            Self::Unclassified => "unclassified",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum FiniteMathSupportAssessment {
    ExplicitBoundedOperation,
    PotentiallyBoundedButAmbiguous,
    MissingSamplingPolicy,
    MissingOrderSemantics,
    MissingReplacementSemantics,
    RequiresCombinatorialModeling,
    RequiresProbabilityModeling,
    RequiresAdvancedTheorem,
    RequiresUnsupportedArithmetic,
    RequiresDiagramOrTable,
    OutsideFiniteMath,
}

impl FiniteMathSupportAssessment {
    pub fn label(self) -> &'static str {
        match self {
            Self::ExplicitBoundedOperation => "explicit_bounded_operation",
            Self::PotentiallyBoundedButAmbiguous => "potentially_bounded_but_ambiguous",
            Self::MissingSamplingPolicy => "missing_sampling_policy",
            Self::MissingOrderSemantics => "missing_order_semantics",
            Self::MissingReplacementSemantics => "missing_replacement_semantics",
            Self::RequiresCombinatorialModeling => "requires_combinatorial_modeling",
            Self::RequiresProbabilityModeling => "requires_probability_modeling",
            Self::RequiresAdvancedTheorem => "requires_advanced_theorem",
            Self::RequiresUnsupportedArithmetic => "requires_unsupported_arithmetic",
            Self::RequiresDiagramOrTable => "requires_diagram_or_table",
            Self::OutsideFiniteMath => "outside_finite_math",
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct FiniteMathFunnelAssessment {
    pub category: String,
    pub task_kind: FiniteMathTaskKind,
    pub support: FiniteMathSupportAssessment,
    pub target: Option<String>,
    pub population_size: Option<u64>,
    pub selection_size: Option<u64>,
    pub order_matters: Option<bool>,
    pub replacement: Option<bool>,
    pub uniformity_explicit: bool,
    pub event_explicit: bool,
    pub bounded_operation: bool,
    pub first_unsupported_stage: Option<String>,
}

/// Non-executing finite-math reconnaissance.  Every semantic field is
/// independently extracted and remains `None` when the prompt does not state
/// it; no sampling policy or order convention is inferred.
pub fn assess_finite_math_funnel(question: &str, category: &str) -> FiniteMathFunnelAssessment {
    let lower = question.to_ascii_lowercase();
    let number = Regex::new(r"\b([0-9]{1,6})\b").expect("finite number recognizer");
    let numbers: Vec<u64> = number
        .captures_iter(&lower)
        .filter_map(|m| m.get(1)?.as_str().parse().ok())
        .collect();
    let population_size = numbers.first().copied();
    let selection_size = numbers.get(1).copied();
    let has_image = lower.contains("diagram")
        || lower.contains("figure")
        || lower.contains("table")
        || lower.contains("image");
    let probability = lower.contains("probability")
        || lower.contains("random variable")
        || lower.contains("coin")
        || lower.contains("card");
    let factorial = lower.contains("factorial")
        || Regex::new(r"\b[0-9]{1,6}!\b")
            .expect("factorial recognizer")
            .is_match(&lower);
    let explicit_binomial =
        Regex::new(r"\\binom\s*\{|\bc\s*\(\s*[0-9]+\s*,\s*[0-9]+\s*\)|binomial coefficient")
            .expect("binomial recognizer")
            .is_match(&lower);
    let binomial = explicit_binomial || lower.contains("choose") || lower.contains("combination");
    let explicit_selection =
        Regex::new(r"(choose|select|arrange|assign)\s+[0-9]+\s+(from|of|out of|among)\s+[0-9]+")
            .expect("selection recognizer")
            .is_match(&lower);
    let permutation = lower.contains("ordered selection")
        || lower.contains("order matters")
        || (lower.contains("permutation") && explicit_selection);
    let unordered = lower.contains("unordered")
        || lower.contains("without regard to order")
        || lower.contains("select") && lower.contains("without replacement");
    let conditional = lower.contains("conditional")
        || lower.contains("given that")
        || lower.contains("conditioned on")
        || lower.contains("knowing that");
    let uniform = lower.contains("uniform")
        || lower.contains("equally likely")
        || lower.contains("each with probability")
        || lower.contains("fair coin")
        || lower.contains("fair die");
    let event_explicit = lower.contains("event")
        || lower.contains("favorable")
        || lower.contains("at least")
        || lower.contains("exactly")
        || lower.contains("outcome")
        || lower.contains("heads")
        || lower.contains("tails")
        || lower.contains("ace")
        || lower.contains("even")
        || lower.contains("odd")
        || lower.contains("all") && probability;
    let replacement = if lower.contains("with replacement") {
        Some(true)
    } else if lower.contains("without replacement") {
        Some(false)
    } else {
        None
    };
    let order_matters = if permutation || lower.contains("ordered") {
        Some(true)
    } else if unordered {
        Some(false)
    } else {
        None
    };
    let recurrence = lower.contains("recurrence")
        || lower.contains("recursive")
        || lower.contains("a_n") && lower.contains("a_{n");
    let generating =
        lower.contains("generating function") || lower.contains("generating polynomial");
    let inclusion = lower.contains("inclusion-exclusion") || lower.contains("inclusion exclusion");
    let expectation = lower.contains("expected value") || lower.contains("expectation");
    let variance = lower.contains("variance") || lower.contains("standard deviation");
    let proof = ["prove", "proof", "show that", "theorem", "rigorously"]
        .iter()
        .any(|term| lower.contains(term));
    let advanced = [
        "polytope",
        "elliptic",
        "markov group",
        "markov chain",
        "random walk",
        "stochastic process",
        "asymptotic",
        "generating function",
        "recurrence",
        "smooth function",
        "simpson",
        "subgroup",
        "schur multiplier",
        "graph",
        "closed tree",
        "fibonacci",
        "rubik",
        "cube",
        "unit square",
        "vertices",
        "floor of the reciprocal",
        "avoid",
        "inversion",
    ]
    .iter()
    .any(|term| lower.contains(term));
    let finite_signal = probability
        || factorial
        || binomial
        || permutation
        || unordered
        || inclusion
        || expectation
        || variance
        || lower.contains("count")
        || lower.contains("how many");
    let (task_kind, support, first_unsupported_stage, bounded_operation) = if has_image {
        (
            FiniteMathTaskKind::Unclassified,
            FiniteMathSupportAssessment::RequiresDiagramOrTable,
            Some("image_or_table_extraction".to_string()),
            false,
        )
    } else if !finite_signal || !category.eq_ignore_ascii_case("math") {
        (
            FiniteMathTaskKind::Unclassified,
            FiniteMathSupportAssessment::OutsideFiniteMath,
            Some("finite_math_signal".to_string()),
            false,
        )
    } else if proof {
        (
            FiniteMathTaskKind::Proof,
            FiniteMathSupportAssessment::RequiresAdvancedTheorem,
            Some("proof_or_theorem".to_string()),
            false,
        )
    } else if advanced {
        (
            if generating {
                FiniteMathTaskKind::GeneratingFunction
            } else if recurrence {
                FiniteMathTaskKind::Recurrence
            } else if probability {
                FiniteMathTaskKind::AdvancedProbability
            } else {
                FiniteMathTaskKind::AdvancedCombinatorics
            },
            FiniteMathSupportAssessment::RequiresAdvancedTheorem,
            Some("specialized_theorem".to_string()),
            false,
        )
    } else if recurrence {
        (
            FiniteMathTaskKind::Recurrence,
            FiniteMathSupportAssessment::RequiresAdvancedTheorem,
            Some("recurrence_solver".to_string()),
            false,
        )
    } else if generating {
        (
            FiniteMathTaskKind::GeneratingFunction,
            FiniteMathSupportAssessment::RequiresAdvancedTheorem,
            Some("generating_function_solver".to_string()),
            false,
        )
    } else if inclusion {
        (
            FiniteMathTaskKind::InclusionExclusion,
            FiniteMathSupportAssessment::RequiresCombinatorialModeling,
            Some("event_intersection_model".to_string()),
            false,
        )
    } else if variance {
        (
            FiniteMathTaskKind::VarianceFiniteSupport,
            FiniteMathSupportAssessment::RequiresProbabilityModeling,
            Some("finite_distribution".to_string()),
            false,
        )
    } else if expectation {
        (
            FiniteMathTaskKind::ExpectationFiniteSupport,
            FiniteMathSupportAssessment::RequiresProbabilityModeling,
            Some("finite_distribution".to_string()),
            false,
        )
    } else if probability && conditional {
        (
            FiniteMathTaskKind::ConditionalFiniteProbability,
            if !uniform {
                FiniteMathSupportAssessment::MissingSamplingPolicy
            } else if !event_explicit {
                FiniteMathSupportAssessment::RequiresProbabilityModeling
            } else {
                FiniteMathSupportAssessment::ExplicitBoundedOperation
            },
            if !uniform {
                Some("uniformity_or_sampling_policy".to_string())
            } else {
                None
            },
            uniform && event_explicit,
        )
    } else if probability {
        (
            FiniteMathTaskKind::UniformFiniteProbability,
            if !uniform {
                FiniteMathSupportAssessment::MissingSamplingPolicy
            } else if replacement.is_none() && (lower.contains("draw") || lower.contains("select"))
            {
                FiniteMathSupportAssessment::MissingReplacementSemantics
            } else if !event_explicit {
                FiniteMathSupportAssessment::RequiresProbabilityModeling
            } else {
                FiniteMathSupportAssessment::ExplicitBoundedOperation
            },
            if !uniform {
                Some("uniformity".to_string())
            } else {
                None
            },
            uniform
                && event_explicit
                && (replacement.is_some() || !(lower.contains("draw") || lower.contains("select"))),
        )
    } else if factorial {
        (
            FiniteMathTaskKind::ExplicitFactorial,
            if population_size.is_some() {
                FiniteMathSupportAssessment::ExplicitBoundedOperation
            } else {
                FiniteMathSupportAssessment::PotentiallyBoundedButAmbiguous
            },
            if population_size.is_none() {
                Some("integer_argument".to_string())
            } else {
                None
            },
            population_size.is_some(),
        )
    } else if binomial && order_matters == Some(false) {
        (
            FiniteMathTaskKind::UnorderedSelection,
            if population_size.is_some() && selection_size.is_some() {
                FiniteMathSupportAssessment::ExplicitBoundedOperation
            } else {
                FiniteMathSupportAssessment::PotentiallyBoundedButAmbiguous
            },
            if population_size.is_none() || selection_size.is_none() {
                Some("selection_parameters".to_string())
            } else {
                None
            },
            population_size.is_some() && selection_size.is_some(),
        )
    } else if permutation || order_matters == Some(true) {
        (
            FiniteMathTaskKind::OrderedSelection,
            if population_size.is_some() && selection_size.is_some() {
                FiniteMathSupportAssessment::ExplicitBoundedOperation
            } else {
                FiniteMathSupportAssessment::MissingOrderSemantics
            },
            if order_matters.is_none() {
                Some("order_semantics".to_string())
            } else {
                None
            },
            population_size.is_some() && selection_size.is_some(),
        )
    } else if binomial {
        (
            FiniteMathTaskKind::ExplicitBinomial,
            if population_size.is_some() && selection_size.is_some() {
                FiniteMathSupportAssessment::ExplicitBoundedOperation
            } else {
                FiniteMathSupportAssessment::PotentiallyBoundedButAmbiguous
            },
            if population_size.is_none() || selection_size.is_none() {
                Some("selection_parameters".to_string())
            } else {
                None
            },
            population_size.is_some() && selection_size.is_some(),
        )
    } else {
        (
            FiniteMathTaskKind::DomainModelingRequired,
            FiniteMathSupportAssessment::RequiresCombinatorialModeling,
            Some("counting_model".to_string()),
            false,
        )
    };
    FiniteMathFunnelAssessment {
        category: category.to_string(),
        task_kind,
        support,
        target: if probability {
            Some("probability".to_string())
        } else if expectation {
            Some("expectation".to_string())
        } else if variance {
            Some("variance".to_string())
        } else if finite_signal {
            Some("count".to_string())
        } else {
            None
        },
        population_size,
        selection_size,
        order_matters,
        replacement,
        uniformity_explicit: uniform,
        event_explicit,
        bounded_operation,
        first_unsupported_stage,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum NumberTheoryTaskKind {
    ExplicitDivisibility,
    ModularEvaluation,
    LinearCongruence,
    GcdLcm,
    PrimeFactorization,
    DiophantineEquation,
    ChineseRemainder,
    MultiplicativeOrder,
    QuadraticResidue,
    IntegerSequence,
    DigitConstraint,
    CountingIntegers,
    Proof,
    AdvancedTheorem,
    SpecializedDefinition,
    Unclassified,
}

impl NumberTheoryTaskKind {
    pub fn label(self) -> &'static str {
        match self {
            Self::ExplicitDivisibility => "explicit_divisibility",
            Self::ModularEvaluation => "modular_evaluation",
            Self::LinearCongruence => "linear_congruence",
            Self::GcdLcm => "gcd_lcm",
            Self::PrimeFactorization => "prime_factorization",
            Self::DiophantineEquation => "diophantine_equation",
            Self::ChineseRemainder => "chinese_remainder",
            Self::MultiplicativeOrder => "multiplicative_order",
            Self::QuadraticResidue => "quadratic_residue",
            Self::IntegerSequence => "integer_sequence",
            Self::DigitConstraint => "digit_constraint",
            Self::CountingIntegers => "counting_integers",
            Self::Proof => "proof",
            Self::AdvancedTheorem => "advanced_theorem",
            Self::SpecializedDefinition => "specialized_definition",
            Self::Unclassified => "unclassified",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum NumberTheorySupportAssessment {
    ExplicitBoundedComputation,
    ExplicitButMagnitudeUnsupported,
    RequiresDomainModeling,
    RequiresSearchStrategy,
    RequiresProof,
    RequiresAdvancedTheorem,
    RequiresSpecializedDefinition,
    RequiresCrossDomainReasoning,
    DiagramOrTableDependent,
    OutsideNumberTheory,
}

impl NumberTheorySupportAssessment {
    pub fn label(self) -> &'static str {
        match self {
            Self::ExplicitBoundedComputation => "explicit_bounded_computation",
            Self::ExplicitButMagnitudeUnsupported => "explicit_but_magnitude_unsupported",
            Self::RequiresDomainModeling => "requires_domain_modeling",
            Self::RequiresSearchStrategy => "requires_search_strategy",
            Self::RequiresProof => "requires_proof",
            Self::RequiresAdvancedTheorem => "requires_advanced_theorem",
            Self::RequiresSpecializedDefinition => "requires_specialized_definition",
            Self::RequiresCrossDomainReasoning => "requires_cross_domain_reasoning",
            Self::DiagramOrTableDependent => "diagram_or_table_dependent",
            Self::OutsideNumberTheory => "outside_number_theory",
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct NumberTheoryFunnelAssessment {
    pub category: String,
    pub task_kind: NumberTheoryTaskKind,
    pub support: NumberTheorySupportAssessment,
    pub integer_operands: Vec<u64>,
    pub modulus: Option<u64>,
    pub target: Option<String>,
    pub explicit_bounds: bool,
    pub named_theorem: Option<String>,
    pub proof_requested: bool,
    pub first_unsupported_stage: Option<String>,
}

/// Number-theory reconnaissance only.  The explicit-computation gate requires
/// a finite integer object and target; modular vocabulary alone is not enough.
pub fn assess_number_theory_funnel(question: &str, category: &str) -> NumberTheoryFunnelAssessment {
    let lower = question.to_ascii_lowercase();
    let number = Regex::new(r"\b([0-9]{1,12})\b").expect("integer recognizer");
    let operands: Vec<u64> = number
        .captures_iter(&lower)
        .filter_map(|m| m.get(1)?.as_str().parse().ok())
        .collect();
    let modulus =
        if lower.contains("mod") || lower.contains("congru") || lower.contains("remainder") {
            operands.last().copied()
        } else {
            None
        };
    let proof = [
        "prove",
        "proof",
        "show that",
        "for all",
        "infinitely many",
        "rigorously",
        "theorem",
    ]
    .iter()
    .any(|term| lower.contains(term));
    let advanced_marker = [
        "elliptic",
        "finite field",
        "algebraic number",
        "quadratic form",
        "class group",
        "modular form",
        "representation",
        "zeta",
        "ramanujan",
        "p-adic",
        "galois",
        "topology",
        "manifold",
        "research",
        "simplicial",
        "euler characteristic",
        "monoid",
        "delooping",
        "day convolution",
        "energy ball",
        "surface area",
        "container",
        "cylinder",
        "invariant measure",
        "infinite product",
        "\\prod",
        "closed formula",
        "infinite sum",
        "sum of 1/n",
        "rational number times",
        "gamma(",
        "dessin",
        "tiling",
        "coloring",
        "recursion",
        "recurrence",
        "a_{",
        "binom",
        "cell",
        "arrow",
        "dots",
        "circle",
        "square",
        "triangle",
        "face",
        "walk",
        "cube",
        "graph",
        "slice",
        "fibonacci",
    ]
    .iter()
    .find(|term| lower.contains(**term))
    .map(|term| (*term).to_string());
    let diagram = lower.contains("diagram")
        || lower.contains("figure")
        || lower.contains("table")
        || lower.contains("image");
    let gcd = lower.contains("gcd")
        || lower.contains("greatest common divisor")
        || lower.contains("lcm")
        || lower.contains("least common multiple");
    let factor = lower.contains("prime factor")
        || lower.contains("factorization")
        || lower.contains("factorise")
        || lower.contains("factorize");
    let linear_congruence = lower.contains("congruence") || lower.contains("≡");
    let modular =
        (lower.contains("mod ") || lower.contains("modulo") || lower.contains("remainder"))
            && !linear_congruence;
    let divisibility = lower.contains("divisible")
        || lower.contains("divisibility")
        || lower.contains("divides")
        || lower.contains("multiple of");
    let diophantine = lower.contains("diophantine")
        || lower.contains("integer solution")
        || lower.contains("integer solutions");
    let crt = lower.contains("chinese remainder") || lower.contains("simultaneous congruence");
    let order = lower.contains("multiplicative order")
        || lower.contains("order of") && lower.contains("mod");
    let residue = lower.contains("quadratic residue") || lower.contains("quadratic non-residue");
    let sequence = lower.contains("sequence") || lower.contains("recurrence");
    let digit = lower.contains("digits") || lower.contains("digit");
    let counting = lower.contains("how many integers")
        || lower.contains("number of integers")
        || lower.contains("count the integers");
    let signal = gcd
        || factor
        || linear_congruence
        || modular
        || divisibility
        || diophantine
        || crt
        || order
        || residue
        || sequence
        || digit
        || counting
        || lower.contains("prime");
    let target_words = [
        "compute",
        "evaluate",
        "find",
        "determine",
        "calculate",
        "remainder",
        "gcd",
        "lcm",
    ];
    let explicit_target = target_words.iter().any(|word| lower.contains(word));
    let bounded_inputs = operands.len() >= 2 && operands.iter().all(|n| *n <= 1_000_000_000);
    let magnitude_unsupported = Regex::new(r"\b[0-9]{13,}\b")
        .expect("magnitude recognizer")
        .is_match(&lower);
    let (task_kind, support, first_unsupported_stage) = if diagram {
        (
            NumberTheoryTaskKind::Unclassified,
            NumberTheorySupportAssessment::DiagramOrTableDependent,
            Some("diagram_or_table".to_string()),
        )
    } else if !signal || !category.eq_ignore_ascii_case("math") {
        (
            NumberTheoryTaskKind::Unclassified,
            NumberTheorySupportAssessment::OutsideNumberTheory,
            Some("number_theory_signal".to_string()),
        )
    } else if proof {
        (
            NumberTheoryTaskKind::Proof,
            NumberTheorySupportAssessment::RequiresProof,
            Some("proof_or_theorem".to_string()),
        )
    } else if let Some(name) = advanced_marker.clone() {
        (
            if name == "elliptic" || name == "galois" || name == "p-adic" {
                NumberTheoryTaskKind::SpecializedDefinition
            } else {
                NumberTheoryTaskKind::AdvancedTheorem
            },
            NumberTheorySupportAssessment::RequiresAdvancedTheorem,
            Some(name),
        )
    } else if magnitude_unsupported {
        (
            if linear_congruence {
                NumberTheoryTaskKind::LinearCongruence
            } else if modular {
                NumberTheoryTaskKind::ModularEvaluation
            } else if factor {
                NumberTheoryTaskKind::PrimeFactorization
            } else {
                NumberTheoryTaskKind::ExplicitDivisibility
            },
            NumberTheorySupportAssessment::ExplicitButMagnitudeUnsupported,
            Some("integer_magnitude".to_string()),
        )
    } else if crt {
        (
            NumberTheoryTaskKind::ChineseRemainder,
            NumberTheorySupportAssessment::RequiresDomainModeling,
            Some("residue_system_extraction".to_string()),
        )
    } else if diophantine {
        (
            NumberTheoryTaskKind::DiophantineEquation,
            NumberTheorySupportAssessment::RequiresSearchStrategy,
            Some("integer_solution_search".to_string()),
        )
    } else if sequence {
        (
            NumberTheoryTaskKind::IntegerSequence,
            NumberTheorySupportAssessment::RequiresSearchStrategy,
            Some("sequence_definition".to_string()),
        )
    } else if digit {
        (
            NumberTheoryTaskKind::DigitConstraint,
            NumberTheorySupportAssessment::RequiresDomainModeling,
            Some("digit_constraint_model".to_string()),
        )
    } else if counting {
        (
            NumberTheoryTaskKind::CountingIntegers,
            NumberTheorySupportAssessment::RequiresSearchStrategy,
            Some("bounded_integer_range".to_string()),
        )
    } else if residue {
        (
            NumberTheoryTaskKind::QuadraticResidue,
            NumberTheorySupportAssessment::RequiresAdvancedTheorem,
            Some("residue_theorem".to_string()),
        )
    } else if order {
        (
            NumberTheoryTaskKind::MultiplicativeOrder,
            NumberTheorySupportAssessment::RequiresDomainModeling,
            Some("group_order_definition".to_string()),
        )
    } else if linear_congruence {
        (
            NumberTheoryTaskKind::LinearCongruence,
            if explicit_target && bounded_inputs {
                NumberTheorySupportAssessment::ExplicitBoundedComputation
            } else {
                NumberTheorySupportAssessment::RequiresDomainModeling
            },
            if explicit_target && bounded_inputs {
                None
            } else {
                Some("congruence_operands".to_string())
            },
        )
    } else if gcd {
        (
            NumberTheoryTaskKind::GcdLcm,
            if explicit_target && bounded_inputs {
                NumberTheorySupportAssessment::ExplicitBoundedComputation
            } else {
                NumberTheorySupportAssessment::RequiresDomainModeling
            },
            if explicit_target && bounded_inputs {
                None
            } else {
                Some("gcd_operands".to_string())
            },
        )
    } else if factor {
        (
            NumberTheoryTaskKind::PrimeFactorization,
            if explicit_target && operands.len() == 1 && bounded_inputs {
                NumberTheorySupportAssessment::ExplicitBoundedComputation
            } else {
                NumberTheorySupportAssessment::ExplicitButMagnitudeUnsupported
            },
            if explicit_target && operands.len() == 1 {
                None
            } else {
                Some("factorization_operand".to_string())
            },
        )
    } else if modular {
        (
            NumberTheoryTaskKind::ModularEvaluation,
            if explicit_target && bounded_inputs && modulus.is_some() {
                NumberTheorySupportAssessment::ExplicitBoundedComputation
            } else {
                NumberTheorySupportAssessment::RequiresDomainModeling
            },
            if explicit_target && bounded_inputs && modulus.is_some() {
                None
            } else {
                Some("modulus_or_expression".to_string())
            },
        )
    } else if divisibility {
        (
            NumberTheoryTaskKind::ExplicitDivisibility,
            if explicit_target && bounded_inputs {
                NumberTheorySupportAssessment::ExplicitBoundedComputation
            } else {
                NumberTheorySupportAssessment::RequiresDomainModeling
            },
            if explicit_target && bounded_inputs {
                None
            } else {
                Some("divisibility_operands".to_string())
            },
        )
    } else {
        (
            NumberTheoryTaskKind::Unclassified,
            NumberTheorySupportAssessment::RequiresDomainModeling,
            Some("number_theory_object".to_string()),
        )
    };
    NumberTheoryFunnelAssessment {
        category: category.to_string(),
        task_kind,
        support,
        integer_operands: operands,
        modulus,
        target: if explicit_target {
            target_words
                .iter()
                .find(|word| lower.contains(**word))
                .map(|word| (*word).to_string())
        } else {
            None
        },
        explicit_bounds: bounded_inputs,
        named_theorem: advanced_marker,
        proof_requested: proof,
        first_unsupported_stage,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CalculusTaskKind {
    ExplicitDifferentiation,
    ExplicitIntegration,
    ExplicitLimitEvaluation,
    FiniteOrInfiniteSeries,
    OdePde,
    Optimization,
    AsymptoticAnalysis,
    SpecialFunctions,
    Proof,
    SpecializedAppliedModeling,
    Unclassified,
}

impl CalculusTaskKind {
    pub fn label(self) -> &'static str {
        match self {
            Self::ExplicitDifferentiation => "explicit_differentiation",
            Self::ExplicitIntegration => "explicit_integration",
            Self::ExplicitLimitEvaluation => "explicit_limit_evaluation",
            Self::FiniteOrInfiniteSeries => "finite_or_infinite_series",
            Self::OdePde => "ode_pde",
            Self::Optimization => "optimization",
            Self::AsymptoticAnalysis => "asymptotic_analysis",
            Self::SpecialFunctions => "special_functions",
            Self::Proof => "proof",
            Self::SpecializedAppliedModeling => "specialized_applied_modeling",
            Self::Unclassified => "unclassified",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CalculusSupportAssessment {
    ExplicitBoundedOperation,
    RequiresSymbolicCapability,
    RequiresBoundaryConditions,
    RequiresConvergenceArgument,
    RequiresAdvancedTheorem,
    RequiresSpecializedDefinition,
    RequiresModeling,
    DiagramOrTableDependent,
    OutsideCalculus,
}

impl CalculusSupportAssessment {
    pub fn label(self) -> &'static str {
        match self {
            Self::ExplicitBoundedOperation => "explicit_bounded_operation",
            Self::RequiresSymbolicCapability => "requires_symbolic_capability",
            Self::RequiresBoundaryConditions => "requires_boundary_conditions",
            Self::RequiresConvergenceArgument => "requires_convergence_argument",
            Self::RequiresAdvancedTheorem => "requires_advanced_theorem",
            Self::RequiresSpecializedDefinition => "requires_specialized_definition",
            Self::RequiresModeling => "requires_modeling",
            Self::DiagramOrTableDependent => "diagram_or_table_dependent",
            Self::OutsideCalculus => "outside_calculus",
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct CalculusFunnelAssessment {
    pub category: String,
    pub task_kind: CalculusTaskKind,
    pub support: CalculusSupportAssessment,
    pub target: Option<String>,
    pub explicit_expression: bool,
    pub bounds_explicit: bool,
    pub domain_explicit: bool,
    pub first_unsupported_stage: Option<String>,
}

pub fn assess_calculus_funnel(question: &str, category: &str) -> CalculusFunnelAssessment {
    let lower = question.to_ascii_lowercase();
    let derivative = lower.contains("derivative")
        || lower.contains("differentiate")
        || lower.contains("differentiate");
    let integral = lower.contains("integral") || lower.contains("integrate") || lower.contains("∫");
    let limit = lower.contains("limit") || lower.contains("lim(") || lower.contains("lim ");
    let series = lower.contains("series")
        || lower.contains("summation")
        || lower.contains("sum_{")
        || lower.contains("power series");
    let ode = lower.contains("differential equation")
        || lower.contains("ode")
        || lower.contains("pde")
        || lower.contains("initial value");
    let optimization = lower.contains("maximum")
        || lower.contains("minimum")
        || lower.contains("maximize")
        || lower.contains("minimize")
        || lower.contains("extremum");
    let asymptotic = lower.contains("asymptotic")
        || lower.contains("tends to infinity")
        || lower.contains("big-o")
        || lower.contains("o(");
    let special = [
        "gamma",
        "zeta",
        "bessel",
        "elliptic",
        "fourier",
        "laplace transform",
        "error function",
        "hypergeometric",
        "resolvent",
        "fractional",
        "caputo",
        "general relativity",
        "curved spacetime",
        "kdv",
        "burgers",
        "non-local",
        "nonlocal",
        "oscillator",
        "coupled",
        "wave equation",
        "f₀",
        "f₁",
        "max(",
        "min(",
        "extrema",
    ]
    .iter()
    .any(|term| lower.contains(term));
    let proof = ["prove", "proof", "show that", "theorem", "rigorously"]
        .iter()
        .any(|term| lower.contains(term));
    let _modeling = [
        "physical",
        "mass",
        "velocity",
        "population",
        "rate",
        "temperature",
        "fluid",
        "circuit",
        "geometry",
        "area",
        "volume",
        "optimization problem",
    ]
    .iter()
    .any(|term| lower.contains(term));
    let diagram = lower.contains("diagram")
        || lower.contains("figure")
        || lower.contains("table")
        || lower.contains("graph shown");
    let explicit_expression = lower.contains("$")
        || lower.contains("\\(")
        || lower.contains("f(")
        || lower.contains("x^")
        || lower.contains("sin(")
        || lower.contains("cos(");
    let bounds_explicit = lower.contains("from") && lower.contains("to")
        || lower.contains("0 to")
        || lower.contains("[-");
    let domain_explicit = lower.contains("for x")
        || lower.contains("where x")
        || lower.contains("x ∈")
        || lower.contains("x in");
    let signal =
        derivative || integral || limit || series || ode || optimization || asymptotic || special;
    let (task_kind, support, first_unsupported_stage) = if diagram {
        (
            CalculusTaskKind::Unclassified,
            CalculusSupportAssessment::DiagramOrTableDependent,
            Some("diagram_or_table".to_string()),
        )
    } else if !signal || !category.eq_ignore_ascii_case("math") {
        (
            CalculusTaskKind::Unclassified,
            CalculusSupportAssessment::OutsideCalculus,
            Some("calculus_signal".to_string()),
        )
    } else if proof {
        (
            CalculusTaskKind::Proof,
            CalculusSupportAssessment::RequiresAdvancedTheorem,
            Some("proof_or_theorem".to_string()),
        )
    } else if special {
        (
            CalculusTaskKind::SpecialFunctions,
            CalculusSupportAssessment::RequiresSpecializedDefinition,
            Some("special_function_definition".to_string()),
        )
    } else if asymptotic {
        (
            CalculusTaskKind::AsymptoticAnalysis,
            CalculusSupportAssessment::RequiresAdvancedTheorem,
            Some("asymptotic_method".to_string()),
        )
    } else if ode {
        (
            CalculusTaskKind::OdePde,
            if domain_explicit && lower.contains("initial") {
                CalculusSupportAssessment::RequiresBoundaryConditions
            } else {
                CalculusSupportAssessment::RequiresModeling
            },
            Some("differential_equation_solver".to_string()),
        )
    } else if series {
        (
            CalculusTaskKind::FiniteOrInfiniteSeries,
            if lower.contains("converge") || lower.contains("convergence") {
                CalculusSupportAssessment::RequiresConvergenceArgument
            } else {
                CalculusSupportAssessment::RequiresSymbolicCapability
            },
            Some("series_semantics".to_string()),
        )
    } else if optimization {
        (
            CalculusTaskKind::Optimization,
            CalculusSupportAssessment::RequiresModeling,
            Some("objective_and_constraints".to_string()),
        )
    } else if derivative {
        (
            CalculusTaskKind::ExplicitDifferentiation,
            if explicit_expression {
                CalculusSupportAssessment::ExplicitBoundedOperation
            } else {
                CalculusSupportAssessment::RequiresSymbolicCapability
            },
            if explicit_expression {
                None
            } else {
                Some("expression_extraction".to_string())
            },
        )
    } else if integral {
        (
            CalculusTaskKind::ExplicitIntegration,
            if explicit_expression && (bounds_explicit || lower.contains("indefinite")) {
                CalculusSupportAssessment::ExplicitBoundedOperation
            } else if !bounds_explicit {
                CalculusSupportAssessment::RequiresSymbolicCapability
            } else {
                CalculusSupportAssessment::RequiresModeling
            },
            if explicit_expression {
                None
            } else {
                Some("integrand_extraction".to_string())
            },
        )
    } else if limit {
        (
            CalculusTaskKind::ExplicitLimitEvaluation,
            if explicit_expression && domain_explicit {
                CalculusSupportAssessment::ExplicitBoundedOperation
            } else {
                CalculusSupportAssessment::RequiresSymbolicCapability
            },
            if explicit_expression {
                None
            } else {
                Some("limit_expression_or_point".to_string())
            },
        )
    } else {
        (
            CalculusTaskKind::Unclassified,
            CalculusSupportAssessment::RequiresSymbolicCapability,
            Some("calculus_operation".to_string()),
        )
    };
    CalculusFunnelAssessment {
        category: category.to_string(),
        task_kind,
        support,
        target: if derivative {
            Some("derivative".to_string())
        } else if integral {
            Some("integral".to_string())
        } else if limit {
            Some("limit".to_string())
        } else if series {
            Some("series".to_string())
        } else if optimization {
            Some("optimization".to_string())
        } else {
            None
        },
        explicit_expression,
        bounds_explicit,
        domain_explicit,
        first_unsupported_stage,
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct MathFunnelAssessment {
    pub category: String,
    pub math_signal: bool,
    pub task_kind: MathTaskKind,
    pub target: Option<String>,
    pub structured_statements: Vec<String>,
    pub answer_form: Option<String>,
    pub executor_candidate: bool,
    pub support_reason: String,
}

/// Classify mathematical prompts by operation rather than benchmark route.
/// The recognizer intentionally prefers abstention: a Math label alone is not
/// evidence that the question is executable algebra.  Equation/expression
/// candidates are only marked as candidates when a target and a structured
/// statement are both visible.
pub fn assess_math_funnel(question: &str, category: &str) -> MathFunnelAssessment {
    let lower = question.to_ascii_lowercase();
    let latex = lower.contains("$") || lower.contains("\\(") || lower.contains("\\[");
    let operator = lower.contains('=')
        || lower.contains("∫")
        || lower.contains("∑")
        || lower.contains("lim")
        || lower.contains("derivative")
        || lower.contains("integral")
        || lower.contains("polynomial")
        || lower.contains("matrix")
        || lower.contains("prime")
        || lower.contains("probability");
    let math_signal = category.eq_ignore_ascii_case("math")
        || (latex && operator)
        || [
            "equation",
            "calculate",
            "compute",
            "evaluate",
            "simplify",
            "solve for",
        ]
        .iter()
        .any(|term| lower.contains(term))
            && operator;

    let has_image = lower.contains("attached image")
        || lower.contains("shown in the image")
        || lower.contains("see fig")
        || lower.contains("diagram")
        || lower.contains("figure");
    let advanced_marker = [
        "manifold",
        "homology",
        "cohomology",
        "bordism",
        "lie algebra",
        "group representation",
        "elliptic curve",
        "chow group",
        "grassmannian",
        "resolvent",
        "schwarzschild",
        "functional analysis",
        "wasserstein",
        "operator",
        "tensor",
        "quantum",
        "relativistic",
        "markov chain",
        "random walk",
        "stochastic",
        "pde",
        "lagrangian",
        "fourier",
        "hilbert",
        "torsion subgroup",
        "stable commutator",
        "differential geometry",
        "field equation",
        "spinor",
        "bose-einstein",
        "kaluza",
        "conformal",
        "spectral",
    ]
    .iter()
    .any(|term| lower.contains(term));
    let proof = [
        "prove",
        "proof",
        "rigorously",
        "is it true",
        "true or false",
        "theorem",
        "show that",
        "demonstrate",
    ]
    .iter()
    .any(|term| lower.contains(term));
    let calculus = [
        "integral",
        "integrate",
        "derivative",
        "differentiate",
        "differential equation",
        "limit",
        "laplacian",
        "gradient",
        "∫",
        "∂",
        "ode",
    ]
    .iter()
    .any(|term| lower.contains(term));
    let number_theory = [
        "prime",
        "divisib",
        "congruence",
        "modulo",
        "mod ",
        "diophantine",
        "integer",
        "gcd",
        "totient",
        "natural density",
    ]
    .iter()
    .any(|term| lower.contains(term));
    let linear_algebra = [
        "matrix",
        "eigenvalue",
        "eigenvector",
        "determinant",
        "rank",
        "vector space",
        "linear algebra",
        "singular value",
    ]
    .iter()
    .any(|term| lower.contains(term));
    let geometry = [
        "triangle",
        "circle",
        "sphere",
        "polygon",
        "angle",
        "area",
        "volume",
        "geometric",
        "geometry",
        "icosahedron",
    ]
    .iter()
    .any(|term| lower.contains(term));
    let combinatorics = [
        "probability",
        "random variable",
        "how many",
        "number of",
        "count",
        "binomial",
        "permutation",
        "combination",
        "arrangement",
        "expected value",
    ]
    .iter()
    .any(|term| lower.contains(term));
    let polynomial = [
        "polynomial",
        "factor",
        "roots",
        "degree",
        "quadratic",
        "cubic",
    ]
    .iter()
    .any(|term| lower.contains(term));
    let solve_words = [
        "solve",
        "find",
        "determine",
        "calculate",
        "compute",
        "evaluate",
        "simplify",
    ];
    let has_target_word = solve_words.iter().any(|term| lower.contains(term));
    let equation = Regex::new(r"(?i)([a-z][a-z0-9_]*|x|y|z|[0-9])[^\n]{0,40}[=<>]\s*[-+0-9a-z\\(]")
        .expect("equation recognizer");
    let has_equation = equation.is_match(question);
    let has_expression = latex
        && (lower.contains("\\frac")
            || lower.contains("^")
            || lower.contains("sqrt")
            || lower.contains("+ ")
            || lower.contains("- "));
    let bounded_context = question.len() <= 700
        && ![
            "finite set",
            "subject to",
            "for all",
            "group",
            "manifold",
            "matrix",
            "system of",
            "boundary",
            "initial value",
            "random",
            "probability",
            "summation",
            "integral",
            "function ",
            "space",
            "curve",
            "field",
            "diagram",
            "tensor",
            "operator",
            "differential",
            "eigen",
            "vector",
        ]
        .iter()
        .any(|term| lower.contains(term));

    let (task_kind, support_reason) = if !math_signal {
        (MathTaskKind::NotMath, "no mathematical signal".to_string())
    } else if has_image {
        (
            MathTaskKind::DiagramDependent,
            "visual structure must be extracted before math".to_string(),
        )
    } else if advanced_marker {
        (
            MathTaskKind::AdvancedSpecialized,
            "specialized mathematical/physical semantics exceed a bounded algebra task".to_string(),
        )
    } else if proof {
        (
            MathTaskKind::ProofOrTheorem,
            "requires theorem/proof reasoning, not expression execution".to_string(),
        )
    } else if calculus {
        (
            MathTaskKind::ElementaryCalculus,
            "calculus operation requires a dedicated symbolic capability".to_string(),
        )
    } else if number_theory {
        (
            MathTaskKind::NumberTheory,
            "number-theory semantics exceed the current algebra executor".to_string(),
        )
    } else if linear_algebra {
        (
            MathTaskKind::LinearAlgebra,
            "matrix/vector semantics require a typed linear-algebra executor".to_string(),
        )
    } else if geometry {
        (
            MathTaskKind::Geometry,
            "geometric relations or diagram semantics are not yet structured".to_string(),
        )
    } else if combinatorics {
        (
            MathTaskKind::FiniteCombinatorics,
            "finite counting/probability needs a discrete solver".to_string(),
        )
    } else if polynomial {
        (
            MathTaskKind::PolynomialAlgebra,
            "polynomial target is recognizable but not yet authorized for execution".to_string(),
        )
    } else if has_target_word && has_equation {
        (
            MathTaskKind::EquationSolving,
            "explicit equation and target detected".to_string(),
        )
    } else if has_target_word && has_expression {
        (
            MathTaskKind::ExplicitExpressionEvaluation,
            "explicit expression and evaluation target detected".to_string(),
        )
    } else if category.eq_ignore_ascii_case("math") {
        (
            MathTaskKind::AdvancedSpecialized,
            "math prompt lacks a bounded executable operation".to_string(),
        )
    } else {
        (
            MathTaskKind::UnparsedMathematicalProse,
            "mathematical signal needs structured parsing".to_string(),
        )
    };

    let mut structured_statements = Vec::new();
    if has_equation {
        structured_statements.push("equation_or_inequality".to_string());
    }
    if latex {
        structured_statements.push("latex_expression".to_string());
    }
    if lower.contains("answer choices") {
        structured_statements.push("answer_choices".to_string());
    }
    let target = if has_target_word {
        solve_words
            .iter()
            .find(|term| lower.contains(**term))
            .map(|term| (*term).to_string())
    } else {
        None
    };
    let answer_form = if lower.contains("answer choices") {
        Some("choice".to_string())
    } else if lower.contains("yes") && lower.contains("no") {
        Some("boolean".to_string())
    } else if lower.contains("integer") || lower.contains("whole number") {
        Some("integer".to_string())
    } else {
        None
    };
    let executor_candidate = matches!(
        task_kind,
        MathTaskKind::EquationSolving | MathTaskKind::ExplicitExpressionEvaluation
    ) && has_target_word
        && (!structured_statements.is_empty())
        && !advanced_marker
        && bounded_context;
    MathFunnelAssessment {
        category: category.to_string(),
        math_signal,
        task_kind,
        target,
        structured_statements,
        answer_form,
        executor_candidate,
        support_reason,
    }
}

/// Analyze a prompt without invoking an executor, external tool, or answer
/// formatter.  This is the benchmark coastline measurement pass.
pub fn assess_mechanics_funnel(question: &str) -> FunnelAssessment {
    let route = QuestionRouter::route(question);
    let lower = question.to_ascii_lowercase();
    let quantity_terms = [
        "mass",
        "force",
        "acceleration",
        "velocity",
        "speed",
        "displacement",
        "distance",
        "work",
        "energy",
        "power",
        "kinetic",
        "momentum",
    ];
    let quantity_hits = quantity_terms
        .iter()
        .filter(|term| lower.contains(**term))
        .count();
    let unit_or_law = [
        "kg",
        "m/s",
        "m/s2",
        "m/s²",
        "newton",
        "joule",
        "watt",
        "gravity",
        "kinetic energy",
    ]
    .iter()
    .any(|term| lower.contains(term));
    let physical_context = [
        "object",
        "body",
        "particle",
        "acted on",
        "travels",
        "moves",
        "accelerates",
        "transferred",
        "displacement",
    ]
    .iter()
    .any(|term| lower.contains(term));
    let representation_noise = [
        "computer architecture",
        "compiler",
        "program in",
        "write a c program",
        "source code",
        "registers",
        "wuxing",
        "bagua computing",
        "titan is an advanced computer",
        "quantum",
        "hydrogen atom",
        "non-ideal gas",
        "gas mixture",
        "manometer",
        "pumped from",
        "robot arm",
        "telescope",
        "galaxy",
        "magnetic monopole",
    ]
    .iter()
    .any(|term| lower.contains(term));
    let mechanics_signal =
        quantity_hits >= 2 && (unit_or_law || physical_context) && !representation_noise;
    if !mechanics_signal {
        return FunnelAssessment {
            route: format!("{route:?}"),
            target: None,
            givens: Vec::new(),
            assumptions: Vec::new(),
            candidate_edges: Vec::new(),
            reachable_depth: 0,
            first_rejection: None,
            near_supported_contract: false,
            assessment: SupportAssessment::OutsideDomain,
        };
    }
    let problem =
        QuestionRouter::extract_problem(question, crate::router::Tool::Physics, Vec::new());
    let registry = crate::methods::MethodRegistry::mechanics_island();
    let target = problem.requested.clone();
    let edges: Vec<_> = registry
        .methods()
        .iter()
        .flat_map(crate::methods::MethodSpec::derivation_edges)
        .filter(|edge| {
            target
                .as_deref()
                .and_then(crate::methods::semantic_variable_for_name)
                .is_some_and(|wanted| edge.produces.semantic == wanted)
        })
        .collect();
    let candidate_edges = edges.iter().map(|edge| edge.id.0.clone()).collect();
    if target.is_none() {
        return FunnelAssessment {
            route: format!("{route:?}"),
            target,
            givens: problem
                .givens
                .iter()
                .map(|given| given.source.clone())
                .collect(),
            assumptions: problem.assumptions.clone(),
            candidate_edges,
            reachable_depth: 0,
            first_rejection: problem
                .unresolved
                .first()
                .map(|reason| format!("{reason:?}")),
            near_supported_contract: false,
            assessment: SupportAssessment::PotentiallySupportedExtractionFailed,
        };
    }
    if edges.is_empty() {
        return FunnelAssessment {
            route: format!("{route:?}"),
            target,
            givens: problem
                .givens
                .iter()
                .map(|given| given.source.clone())
                .collect(),
            assumptions: problem.assumptions.clone(),
            candidate_edges,
            reachable_depth: 0,
            first_rejection: Some("no registry edge produces target".to_string()),
            near_supported_contract: false,
            assessment: SupportAssessment::RequiresUnsupportedMethod,
        };
    }
    let single = registry.plan_single_step(&problem);
    let depth_two = registry.plan_depth_two(&problem, crate::methods::PlannerLimits::default());
    let near_supported_contract = edges.iter().any(|edge| {
        let known: Vec<_> = problem
            .givens
            .iter()
            .filter_map(|given| crate::methods::semantic_variable_for_name(&given.variable))
            .collect();
        let bound = edge
            .requires
            .iter()
            .filter(|required| {
                known
                    .iter()
                    .any(|candidate| *candidate == required.semantic)
            })
            .count();
        bound == edge.requires.len()
    });
    let (reachable_depth, first_rejection, assessment) = match single {
        crate::methods::SingleStepPlanResult::Planned(_) => {
            (1, None, SupportAssessment::InsideSupportedContract)
        }
        crate::methods::SingleStepPlanResult::MultipleUnresolvedMethods(_, rejected) => (
            0,
            rejected.first().map(|item| format!("{:?}", item.reason)),
            SupportAssessment::RequiresUnsupportedRepresentation,
        ),
        crate::methods::SingleStepPlanResult::NoApplicableMethod(rejected) => {
            let rejection = rejected.first().map(|item| format!("{:?}", item.reason));
            match depth_two {
                crate::methods::PlanSelection::Unique(_) => {
                    (2, rejection, SupportAssessment::InsideSupportedContract)
                }
                crate::methods::PlanSelection::Consensus(_) => {
                    (2, rejection, SupportAssessment::InsideSupportedContract)
                }
                crate::methods::PlanSelection::Ambiguous(_) => (
                    0,
                    rejection,
                    SupportAssessment::RequiresUnsupportedRepresentation,
                ),
                crate::methods::PlanSelection::None(depth_rejected) => {
                    let reasons: Vec<_> = rejected
                        .iter()
                        .chain(depth_rejected.iter())
                        .map(|item| item.reason.clone())
                        .collect();
                    let first = reasons.first().map(|reason| format!("{reason:?}"));
                    let assessment = if reasons.iter().any(|reason| {
                        *reason == crate::methods::CandidateRejection::MissingAssumption
                    }) {
                        SupportAssessment::RequiredAssumptionAbsent
                    } else if problem.givens.len() > 0 && target.as_deref() == Some("P") {
                        SupportAssessment::RequiresUnsupportedPlanDepth
                    } else {
                        SupportAssessment::RequiresUnsupportedMethod
                    };
                    (0, first.or(rejection), assessment)
                }
            }
        }
    };
    FunnelAssessment {
        route: format!("{route:?}"),
        target,
        givens: problem
            .givens
            .iter()
            .map(|given| given.source.clone())
            .collect(),
        assumptions: problem.assumptions,
        candidate_edges,
        reachable_depth,
        first_rejection,
        near_supported_contract,
        assessment,
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExpectedGiven {
    pub variable: &'static str,
    pub value: &'static str,
}

#[derive(Debug, Clone)]
pub struct AnnotatedDevelopmentCase {
    pub question_id: &'static str,
    pub question: &'static str,
    pub support_status: SupportStatus,
    pub expected_target: &'static str,
    pub expected_givens: &'static [ExpectedGiven],
    /// One or more complete authorized edge sequences.  A sequence of one is
    /// a direct derivation; two is a bounded compositional derivation.
    pub allowed_plans: &'static [&'static [&'static str]],
    pub required_assumptions: &'static [MethodAssumption],
    pub forbidden_assumptions: &'static [MethodAssumption],
    pub expected_answer: Option<&'static str>,
    pub expected_abstention: Option<AbstentionReason>,
}

#[derive(Debug, Clone)]
pub struct DevelopmentCaseResult {
    pub question_id: &'static str,
    pub answer: Option<String>,
    pub abstention: Option<AbstentionReason>,
    pub target_ok: bool,
    pub givens_ok: bool,
    pub assumptions_ok: bool,
    pub plan_ok: bool,
    pub provenance_ok: bool,
    pub contract_ok: bool,
    pub unsafe_execution: bool,
    pub rejected: Vec<CandidateRejection>,
}

#[derive(Debug, Clone)]
pub struct DevelopmentReport {
    pub cases: Vec<DevelopmentCaseResult>,
}

impl DevelopmentReport {
    pub fn failures(&self) -> Vec<&DevelopmentCaseResult> {
        self.cases
            .iter()
            .filter(|case_| !case_.contract_ok)
            .collect()
    }

    pub fn supported(&self) -> usize {
        self.cases
            .iter()
            .filter(|case_| case_.answer.is_some())
            .count()
    }

    pub fn verified(&self) -> usize {
        self.cases
            .iter()
            .filter(|case_| case_.answer.is_some() && !case_.unsafe_execution)
            .count()
    }
}

const EMPTY_GIVENS: &[ExpectedGiven] = &[];
const NO_ASSUMPTIONS: &[MethodAssumption] = &[];
const NO_PLANS: &[&[&str]] = &[];
const NONE: Option<&str> = None;

const M_2_F_10: &[ExpectedGiven] = &[
    ExpectedGiven {
        variable: "m",
        value: "2",
    },
    ExpectedGiven {
        variable: "F",
        value: "10",
    },
];
const M_2_A_3: &[ExpectedGiven] = &[
    ExpectedGiven {
        variable: "m",
        value: "2",
    },
    ExpectedGiven {
        variable: "a",
        value: "3",
    },
];
const V_3_T_4: &[ExpectedGiven] = &[
    ExpectedGiven {
        variable: "v",
        value: "3",
    },
    ExpectedGiven {
        variable: "t",
        value: "4",
    },
];
const D_12_T_4: &[ExpectedGiven] = &[
    ExpectedGiven {
        variable: "d",
        value: "12",
    },
    ExpectedGiven {
        variable: "t",
        value: "4",
    },
];
const E_8_T_2: &[ExpectedGiven] = &[
    ExpectedGiven {
        variable: "E",
        value: "8",
    },
    ExpectedGiven {
        variable: "t",
        value: "2",
    },
];
const M_4_V_3_T_2: &[ExpectedGiven] = &[
    ExpectedGiven {
        variable: "m",
        value: "4",
    },
    ExpectedGiven {
        variable: "v",
        value: "3",
    },
    ExpectedGiven {
        variable: "t",
        value: "2",
    },
];
const M_4_V_3: &[ExpectedGiven] = &[
    ExpectedGiven {
        variable: "m",
        value: "4",
    },
    ExpectedGiven {
        variable: "v",
        value: "3",
    },
];
const M_4_T_2: &[ExpectedGiven] = &[
    ExpectedGiven {
        variable: "m",
        value: "4",
    },
    ExpectedGiven {
        variable: "t",
        value: "2",
    },
];
const M_2_V_3_T_4: &[ExpectedGiven] = &[
    ExpectedGiven {
        variable: "m",
        value: "2",
    },
    ExpectedGiven {
        variable: "v",
        value: "3",
    },
    ExpectedGiven {
        variable: "t",
        value: "4",
    },
];
const M_3_V_4_T_6: &[ExpectedGiven] = &[
    ExpectedGiven {
        variable: "m",
        value: "3",
    },
    ExpectedGiven {
        variable: "v",
        value: "4",
    },
    ExpectedGiven {
        variable: "t",
        value: "6",
    },
];
const M_5_A_2_D_3: &[ExpectedGiven] = &[
    ExpectedGiven {
        variable: "m",
        value: "5",
    },
    ExpectedGiven {
        variable: "a",
        value: "2",
    },
    ExpectedGiven {
        variable: "d",
        value: "3",
    },
];
const M_2_A_3_D_4: &[ExpectedGiven] = &[
    ExpectedGiven {
        variable: "m",
        value: "2",
    },
    ExpectedGiven {
        variable: "a",
        value: "3",
    },
    ExpectedGiven {
        variable: "d",
        value: "4",
    },
];

const NEWTON_A: &[&str] = &["mechanics.newton_second_law::solve_a"];
const NEWTON_F: &[&str] = &["mechanics.newton_second_law::solve_F"];
const VELOCITY_D: &[&str] = &["mechanics.constant_velocity.distance::solve_d"];
const VELOCITY_V: &[&str] = &["mechanics.constant_velocity.distance::solve_v"];
const POWER_P: &[&str] = &["mechanics.power::solve_P"];
const ENERGY_POWER: &[&str] = &[
    "mechanics.kinetic_energy::solve_E",
    "mechanics.power::solve_P",
];
const FORCE_WORK: &[&str] = &[
    "mechanics.newton_second_law::solve_F",
    "mechanics.work_constant_force::solve_W",
];

const CV: &[MethodAssumption] = &[MethodAssumption::ConstantVelocity];
const TRANSFER: &[MethodAssumption] = &[MethodAssumption::EnergyTransferredOverInterval];
const WORK_ASSUMPTIONS: &[MethodAssumption] = &[
    MethodAssumption::ConstantForce,
    MethodAssumption::CollinearForceDisplacement,
];

/// The initial 25-case island: 5 direct, 7 compositional, 5 missing
/// assumptions, 4 semantic handoff traps, and 4 deliberately unsupported.
pub fn mechanics_island_cases() -> Vec<AnnotatedDevelopmentCase> {
    vec![
        AnnotatedDevelopmentCase { question_id: "single.newton.acceleration", question: "A 2 kg object is acted on by a 10 N force. What is its acceleration?", support_status: SupportStatus::Supported, expected_target: "a", expected_givens: M_2_F_10, allowed_plans: &[NEWTON_A], required_assumptions: NO_ASSUMPTIONS, forbidden_assumptions: NO_ASSUMPTIONS, expected_answer: Some("5"), expected_abstention: None },
        AnnotatedDevelopmentCase { question_id: "single.newton.force", question: "A 2 kg object accelerates at 3 m/s2. What force acts on it?", support_status: SupportStatus::Supported, expected_target: "F", expected_givens: M_2_A_3, allowed_plans: &[NEWTON_F], required_assumptions: NO_ASSUMPTIONS, forbidden_assumptions: NO_ASSUMPTIONS, expected_answer: Some("6"), expected_abstention: None },
        AnnotatedDevelopmentCase { question_id: "single.kinematics.distance", question: "At constant velocity, a car travels at 3 m/s for 4 s. What distance does it travel?", support_status: SupportStatus::Supported, expected_target: "d", expected_givens: V_3_T_4, allowed_plans: &[VELOCITY_D], required_assumptions: CV, forbidden_assumptions: NO_ASSUMPTIONS, expected_answer: Some("12"), expected_abstention: None },
        AnnotatedDevelopmentCase { question_id: "single.kinematics.velocity", question: "At constant velocity, a car travels 12 m in 4 s. What velocity does it have?", support_status: SupportStatus::Supported, expected_target: "v", expected_givens: D_12_T_4, allowed_plans: &[VELOCITY_V], required_assumptions: CV, forbidden_assumptions: NO_ASSUMPTIONS, expected_answer: Some("3"), expected_abstention: None },
        AnnotatedDevelopmentCase { question_id: "single.power.direct", question: "An object transfers 8 J in 2 s. What is its power?", support_status: SupportStatus::Supported, expected_target: "P", expected_givens: E_8_T_2, allowed_plans: &[POWER_P], required_assumptions: NO_ASSUMPTIONS, forbidden_assumptions: NO_ASSUMPTIONS, expected_answer: Some("4"), expected_abstention: None },

        AnnotatedDevelopmentCase { question_id: "depth2.energy_power.1", question: "A 2 kg object moves at 3 m/s for 4 s. Its entire kinetic energy is transferred over the interval. What is the power?", support_status: SupportStatus::Supported, expected_target: "P", expected_givens: M_2_V_3_T_4, allowed_plans: &[ENERGY_POWER], required_assumptions: TRANSFER, forbidden_assumptions: NO_ASSUMPTIONS, expected_answer: Some("2.25"), expected_abstention: None },
        AnnotatedDevelopmentCase { question_id: "depth2.energy_power.2", question: "A 4 kg object moves at 3 m/s for 2 s. Its entire kinetic energy is transferred over the interval. What is the power?", support_status: SupportStatus::Supported, expected_target: "P", expected_givens: M_4_V_3_T_2, allowed_plans: &[ENERGY_POWER], required_assumptions: TRANSFER, forbidden_assumptions: NO_ASSUMPTIONS, expected_answer: Some("9"), expected_abstention: None },
        AnnotatedDevelopmentCase { question_id: "depth2.energy_power.3", question: "A 3 kg body has speed 4 m/s. All of its kinetic energy is transferred over 6 s. Find the power.", support_status: SupportStatus::Supported, expected_target: "P", expected_givens: M_3_V_4_T_6, allowed_plans: &[ENERGY_POWER], required_assumptions: TRANSFER, forbidden_assumptions: NO_ASSUMPTIONS, expected_answer: Some("4"), expected_abstention: None },
        AnnotatedDevelopmentCase { question_id: "depth2.force_work.1", question: "Assuming constant force and force is parallel to displacement, a 2 kg object accelerates at 3 m/s2 over 4 m. What work is done?", support_status: SupportStatus::Supported, expected_target: "W", expected_givens: M_2_A_3_D_4, allowed_plans: &[FORCE_WORK], required_assumptions: WORK_ASSUMPTIONS, forbidden_assumptions: NO_ASSUMPTIONS, expected_answer: Some("24"), expected_abstention: None },
        AnnotatedDevelopmentCase { question_id: "depth2.force_work.2", question: "Assuming constant force and force is parallel to displacement, a 5 kg object accelerates at 2 m/s2 over 3 m. What work is done?", support_status: SupportStatus::Supported, expected_target: "W", expected_givens: M_5_A_2_D_3, allowed_plans: &[FORCE_WORK], required_assumptions: WORK_ASSUMPTIONS, forbidden_assumptions: NO_ASSUMPTIONS, expected_answer: Some("30"), expected_abstention: None },
        AnnotatedDevelopmentCase { question_id: "depth2.force_work.3", question: "Under constant force and force is parallel to displacement, a 5 kg object accelerates at 2 m/s2 over 3 m. Find the work.", support_status: SupportStatus::Supported, expected_target: "W", expected_givens: M_5_A_2_D_3, allowed_plans: &[FORCE_WORK], required_assumptions: WORK_ASSUMPTIONS, forbidden_assumptions: NO_ASSUMPTIONS, expected_answer: Some("30"), expected_abstention: None },
        AnnotatedDevelopmentCase { question_id: "depth2.energy_power.4", question: "A 2 kg object moves at 3 m/s. Its full kinetic energy is expended over 4 s. Find power.", support_status: SupportStatus::Supported, expected_target: "P", expected_givens: M_2_V_3_T_4, allowed_plans: &[ENERGY_POWER], required_assumptions: TRANSFER, forbidden_assumptions: NO_ASSUMPTIONS, expected_answer: Some("2.25"), expected_abstention: None },

        AnnotatedDevelopmentCase { question_id: "abstain.missing.duration_power", question: "A 4 kg object moves at 3 m/s. What is the power?", support_status: SupportStatus::MustAbstain, expected_target: "P", expected_givens: M_4_V_3, allowed_plans: NO_PLANS, required_assumptions: NO_ASSUMPTIONS, forbidden_assumptions: NO_ASSUMPTIONS, expected_answer: NONE, expected_abstention: Some(AbstentionReason::RequiredAssumptionMissing) },
        AnnotatedDevelopmentCase { question_id: "abstain.missing.work_assumptions", question: "A 2 kg object accelerates at 3 m/s2 over 4 m. What work is done?", support_status: SupportStatus::MustAbstain, expected_target: "W", expected_givens: M_2_A_3_D_4, allowed_plans: NO_PLANS, required_assumptions: NO_ASSUMPTIONS, forbidden_assumptions: WORK_ASSUMPTIONS, expected_answer: NONE, expected_abstention: Some(AbstentionReason::RequiredAssumptionMissing) },
        AnnotatedDevelopmentCase { question_id: "abstain.contradicted.perpendicular", question: "Assuming constant force, a 2 kg object accelerates at 3 m/s2. The force is perpendicular to the 4 m displacement. What work is done?", support_status: SupportStatus::MustAbstain, expected_target: "W", expected_givens: M_2_A_3_D_4, allowed_plans: NO_PLANS, required_assumptions: NO_ASSUMPTIONS, forbidden_assumptions: WORK_ASSUMPTIONS, expected_answer: NONE, expected_abstention: Some(AbstentionReason::RequiredAssumptionContradicted) },
        AnnotatedDevelopmentCase { question_id: "abstain.contradicted.variable_force", question: "Assuming force is parallel to displacement, a variable force acts on a 2 kg object accelerating at 3 m/s2 over 4 m. What work is done?", support_status: SupportStatus::MustAbstain, expected_target: "W", expected_givens: M_2_A_3_D_4, allowed_plans: NO_PLANS, required_assumptions: NO_ASSUMPTIONS, forbidden_assumptions: WORK_ASSUMPTIONS, expected_answer: NONE, expected_abstention: Some(AbstentionReason::RequiredAssumptionContradicted) },
        AnnotatedDevelopmentCase { question_id: "abstain.contradicted.velocity", question: "A car's speed changes from 3 m/s over 4 s. What distance does it travel?", support_status: SupportStatus::MustAbstain, expected_target: "d", expected_givens: V_3_T_4, allowed_plans: NO_PLANS, required_assumptions: NO_ASSUMPTIONS, forbidden_assumptions: CV, expected_answer: NONE, expected_abstention: Some(AbstentionReason::RequiredAssumptionContradicted) },

        AnnotatedDevelopmentCase { question_id: "trap.partial_energy", question: "A 4 kg object loses some kinetic energy over 2 s. What is the power?", support_status: SupportStatus::MustAbstain, expected_target: "P", expected_givens: M_4_T_2, allowed_plans: NO_PLANS, required_assumptions: NO_ASSUMPTIONS, forbidden_assumptions: TRANSFER, expected_answer: NONE, expected_abstention: Some(AbstentionReason::RequiredAssumptionContradicted) },
        AnnotatedDevelopmentCase { question_id: "trap.motion_duration", question: "A 4 kg object moves at 3 m/s for 2 s. What is the power?", support_status: SupportStatus::MustAbstain, expected_target: "P", expected_givens: M_4_V_3_T_2, allowed_plans: NO_PLANS, required_assumptions: NO_ASSUMPTIONS, forbidden_assumptions: TRANSFER, expected_answer: NONE, expected_abstention: Some(AbstentionReason::RequiredAssumptionMissing) },
        AnnotatedDevelopmentCase { question_id: "trap.acceleration_power", question: "A 4 kg object accelerates to 3 m/s over 2 s. What power is required?", support_status: SupportStatus::MustAbstain, expected_target: "P", expected_givens: M_4_V_3_T_2, allowed_plans: NO_PLANS, required_assumptions: NO_ASSUMPTIONS, forbidden_assumptions: TRANSFER, expected_answer: NONE, expected_abstention: Some(AbstentionReason::RequiredAssumptionMissing) },
        AnnotatedDevelopmentCase { question_id: "trap.perpendicular_work", question: "A 2 kg object accelerates at 3 m/s2 over 4 m; the force is perpendicular to displacement. What work is done?", support_status: SupportStatus::MustAbstain, expected_target: "W", expected_givens: M_2_A_3_D_4, allowed_plans: NO_PLANS, required_assumptions: NO_ASSUMPTIONS, forbidden_assumptions: WORK_ASSUMPTIONS, expected_answer: NONE, expected_abstention: Some(AbstentionReason::RequiredAssumptionContradicted) },

        AnnotatedDevelopmentCase { question_id: "unsupported.drag", question: "A 2 kg object falls with quadratic air drag. What is its terminal velocity?", support_status: SupportStatus::MustAbstain, expected_target: "v", expected_givens: EMPTY_GIVENS, allowed_plans: NO_PLANS, required_assumptions: NO_ASSUMPTIONS, forbidden_assumptions: NO_ASSUMPTIONS, expected_answer: NONE, expected_abstention: None },
        AnnotatedDevelopmentCase { question_id: "unsupported.projectile", question: "A projectile is launched at 20 m/s at 45 degrees. How far does it travel?", support_status: SupportStatus::MustAbstain, expected_target: "", expected_givens: EMPTY_GIVENS, allowed_plans: NO_PLANS, required_assumptions: NO_ASSUMPTIONS, forbidden_assumptions: NO_ASSUMPTIONS, expected_answer: NONE, expected_abstention: None },
        AnnotatedDevelopmentCase { question_id: "unsupported.circular", question: "What is the tension in a string for a nonuniform circular orbit?", support_status: SupportStatus::MustAbstain, expected_target: "", expected_givens: EMPTY_GIVENS, allowed_plans: NO_PLANS, required_assumptions: NO_ASSUMPTIONS, forbidden_assumptions: NO_ASSUMPTIONS, expected_answer: NONE, expected_abstention: None },
        AnnotatedDevelopmentCase { question_id: "unsupported.thermo", question: "A gas expands reversibly with changing pressure. Find the work from the diagram.", support_status: SupportStatus::MustAbstain, expected_target: "", expected_givens: EMPTY_GIVENS, allowed_plans: NO_PLANS, required_assumptions: NO_ASSUMPTIONS, forbidden_assumptions: NO_ASSUMPTIONS, expected_answer: NONE, expected_abstention: None },
    ]
}

// Authored independently of the regression corpus.  These cases deliberately
// vary order, phrasing, units, decimals, and irrelevant facts while staying
// inside the same five authorized method families.
pub fn mechanics_holdout_cases() -> Vec<AnnotatedDevelopmentCase> {
    const MF12: &[ExpectedGiven] = &[
        ExpectedGiven {
            variable: "m",
            value: "3",
        },
        ExpectedGiven {
            variable: "F",
            value: "12",
        },
    ];
    const MA52: &[ExpectedGiven] = &[
        ExpectedGiven {
            variable: "m",
            value: "5",
        },
        ExpectedGiven {
            variable: "a",
            value: "2",
        },
    ];
    const V7T3: &[ExpectedGiven] = &[
        ExpectedGiven {
            variable: "v",
            value: "7",
        },
        ExpectedGiven {
            variable: "t",
            value: "3",
        },
    ];
    const D21T3: &[ExpectedGiven] = &[
        ExpectedGiven {
            variable: "d",
            value: "21",
        },
        ExpectedGiven {
            variable: "t",
            value: "3",
        },
    ];
    const E15T3: &[ExpectedGiven] = &[
        ExpectedGiven {
            variable: "E",
            value: "15",
        },
        ExpectedGiven {
            variable: "t",
            value: "3",
        },
    ];
    const M2V5T4: &[ExpectedGiven] = &[
        ExpectedGiven {
            variable: "m",
            value: "2",
        },
        ExpectedGiven {
            variable: "v",
            value: "5",
        },
        ExpectedGiven {
            variable: "t",
            value: "4",
        },
    ];
    const M6V2T3: &[ExpectedGiven] = &[
        ExpectedGiven {
            variable: "m",
            value: "6",
        },
        ExpectedGiven {
            variable: "v",
            value: "2",
        },
        ExpectedGiven {
            variable: "t",
            value: "3",
        },
    ];
    const M5A2D3: &[ExpectedGiven] = &[
        ExpectedGiven {
            variable: "m",
            value: "5",
        },
        ExpectedGiven {
            variable: "a",
            value: "2",
        },
        ExpectedGiven {
            variable: "d",
            value: "3",
        },
    ];
    const M15A4D2: &[ExpectedGiven] = &[
        ExpectedGiven {
            variable: "m",
            value: "1.5",
        },
        ExpectedGiven {
            variable: "a",
            value: "4",
        },
        ExpectedGiven {
            variable: "d",
            value: "2",
        },
    ];
    const MG500A2D3: &[ExpectedGiven] = &[
        ExpectedGiven {
            variable: "m",
            value: "500",
        },
        ExpectedGiven {
            variable: "a",
            value: "2",
        },
        ExpectedGiven {
            variable: "d",
            value: "3",
        },
    ];
    const M2VKMT3: &[ExpectedGiven] = &[
        ExpectedGiven {
            variable: "m",
            value: "2",
        },
        ExpectedGiven {
            variable: "v",
            value: "18",
        },
        ExpectedGiven {
            variable: "t",
            value: "3",
        },
    ];
    const M2V5T4000: &[ExpectedGiven] = &[
        ExpectedGiven {
            variable: "m",
            value: "2",
        },
        ExpectedGiven {
            variable: "v",
            value: "5",
        },
        ExpectedGiven {
            variable: "t",
            value: "4000",
        },
    ];
    const NEWTON_A: &[&str] = &["mechanics.newton_second_law::solve_a"];
    const NEWTON_F: &[&str] = &["mechanics.newton_second_law::solve_F"];
    const CV_D: &[&str] = &["mechanics.constant_velocity.distance::solve_d"];
    const CV_V: &[&str] = &["mechanics.constant_velocity.distance::solve_v"];
    const PWR: &[&str] = &["mechanics.power::solve_P"];
    const EP: &[&str] = ENERGY_POWER;
    const FW: &[&str] = FORCE_WORK;
    let no = NO_ASSUMPTIONS;
    let cv = CV;
    let transfer = TRANSFER;
    let work = WORK_ASSUMPTIONS;
    vec![
        AnnotatedDevelopmentCase { question_id: "holdout.01", question: "Given a net force of 12 N on a 3 kg body, calculate its acceleration.", support_status: SupportStatus::Supported, expected_target: "a", expected_givens: MF12, allowed_plans: &[NEWTON_A], required_assumptions: no, forbidden_assumptions: no, expected_answer: Some("4"), expected_abstention: None },
        AnnotatedDevelopmentCase { question_id: "holdout.02", question: "With mass 5 kg and acceleration 2 m/s2, determine the force. The object is blue.", support_status: SupportStatus::Supported, expected_target: "F", expected_givens: MA52, allowed_plans: &[NEWTON_F], required_assumptions: no, forbidden_assumptions: no, expected_answer: Some("10"), expected_abstention: None },
        AnnotatedDevelopmentCase { question_id: "holdout.03", question: "What distance results when a vehicle at constant velocity travels 7 m/s for 3 s?", support_status: SupportStatus::Supported, expected_target: "d", expected_givens: V7T3, allowed_plans: &[CV_D], required_assumptions: cv, forbidden_assumptions: no, expected_answer: Some("21"), expected_abstention: None },
        AnnotatedDevelopmentCase { question_id: "holdout.04", question: "At constant velocity, 21 m is covered in 3 s. Determine the velocity.", support_status: SupportStatus::Supported, expected_target: "v", expected_givens: D21T3, allowed_plans: &[CV_V], required_assumptions: cv, forbidden_assumptions: no, expected_answer: Some("7"), expected_abstention: None },
        AnnotatedDevelopmentCase { question_id: "holdout.05", question: "Determine the power when 15 J is transferred in 3 s.", support_status: SupportStatus::Supported, expected_target: "P", expected_givens: E15T3, allowed_plans: &[PWR], required_assumptions: no, forbidden_assumptions: no, expected_answer: Some("5"), expected_abstention: None },
        AnnotatedDevelopmentCase { question_id: "holdout.06", question: "Determine the power: a 2 kg body at 5 m/s transfers all its kinetic energy in 4 s.", support_status: SupportStatus::Supported, expected_target: "P", expected_givens: M2V5T4, allowed_plans: &[EP], required_assumptions: transfer, forbidden_assumptions: no, expected_answer: Some("6.25"), expected_abstention: None },
        AnnotatedDevelopmentCase { question_id: "holdout.07", question: "A 6 kg body moves at 2 m/s. Its entire kinetic energy is transferred over 3 s. Find power.", support_status: SupportStatus::Supported, expected_target: "P", expected_givens: M6V2T3, allowed_plans: &[EP], required_assumptions: transfer, forbidden_assumptions: no, expected_answer: Some("4"), expected_abstention: None },
        AnnotatedDevelopmentCase { question_id: "holdout.08", question: "With constant force and force is parallel to displacement, a 5 kg object has acceleration 2 m/s2 and displacement 3 m. Find work.", support_status: SupportStatus::Supported, expected_target: "W", expected_givens: M5A2D3, allowed_plans: &[FW], required_assumptions: work, forbidden_assumptions: no, expected_answer: Some("30"), expected_abstention: None },
        AnnotatedDevelopmentCase { question_id: "holdout.09", question: "What work is done? Assuming constant force and force is parallel to displacement, a 1.5 kg object accelerates at 4 m/s2 over 2 m.", support_status: SupportStatus::Supported, expected_target: "W", expected_givens: M15A4D2, allowed_plans: &[FW], required_assumptions: work, forbidden_assumptions: no, expected_answer: Some("12"), expected_abstention: None },
        AnnotatedDevelopmentCase { question_id: "holdout.10", question: "Under constant force and force is parallel to displacement, a 500 g object accelerates at 2 m/s2 over 3 m. What work is done?", support_status: SupportStatus::Supported, expected_target: "W", expected_givens: MG500A2D3, allowed_plans: &[FW], required_assumptions: work, forbidden_assumptions: no, expected_answer: Some("3"), expected_abstention: None },
        AnnotatedDevelopmentCase { question_id: "holdout.11", question: "A 2 kg object moves at 18 km/h for 3 s. Its entire kinetic energy is transferred over the interval. What is the power?", support_status: SupportStatus::Supported, expected_target: "P", expected_givens: M2VKMT3, allowed_plans: &[EP], required_assumptions: transfer, forbidden_assumptions: no, expected_answer: Some("8.333333333333334"), expected_abstention: None },
        AnnotatedDevelopmentCase { question_id: "holdout.12", question: "A 2 kg object moves at 5 m/s. Its entire kinetic energy is transferred in 4000 ms. Find the power.", support_status: SupportStatus::Supported, expected_target: "P", expected_givens: M2V5T4000, allowed_plans: &[EP], required_assumptions: transfer, forbidden_assumptions: no, expected_answer: Some("6.25"), expected_abstention: None },
        AnnotatedDevelopmentCase { question_id: "holdout.13", question: "At constant velocity, a car covers 0.5 km in 2 s. What velocity does it have?", support_status: SupportStatus::Supported, expected_target: "v", expected_givens: &[], allowed_plans: &[CV_V], required_assumptions: cv, forbidden_assumptions: no, expected_answer: Some("250"), expected_abstention: None },
        AnnotatedDevelopmentCase { question_id: "holdout.14", question: "At constant velocity, a car travels 2 m/s for 500 ms. What distance does it travel?", support_status: SupportStatus::Supported, expected_target: "d", expected_givens: &[], allowed_plans: &[CV_D], required_assumptions: cv, forbidden_assumptions: no, expected_answer: Some("1"), expected_abstention: None },
        AnnotatedDevelopmentCase { question_id: "holdout.15", question: "A machine transfers 2.5 J in 0.5 s. Determine its power.", support_status: SupportStatus::Supported, expected_target: "P", expected_givens: &[], allowed_plans: &[PWR], required_assumptions: no, forbidden_assumptions: no, expected_answer: Some("5"), expected_abstention: None },
        AnnotatedDevelopmentCase { question_id: "holdout.16", question: "Assuming constant force and force is parallel to displacement, mass is 2 kg, acceleration is 2 m/s2, and displacement is 2 m. Find work; temperature is 300 K.", support_status: SupportStatus::Supported, expected_target: "W", expected_givens: &[], allowed_plans: &[FW], required_assumptions: work, forbidden_assumptions: no, expected_answer: Some("8"), expected_abstention: None },
        AnnotatedDevelopmentCase { question_id: "holdout.17", question: "A 3 kg object is acted on by 6 N. What is its acceleration? Ignore its color and temperature.", support_status: SupportStatus::Supported, expected_target: "a", expected_givens: &[], allowed_plans: &[NEWTON_A], required_assumptions: no, forbidden_assumptions: no, expected_answer: Some("2"), expected_abstention: None },
        AnnotatedDevelopmentCase { question_id: "holdout.18", question: "A 4 kg object moves at 2 m/s. Its entire kinetic energy is transferred over 1 s. Find power.", support_status: SupportStatus::Supported, expected_target: "P", expected_givens: &[], allowed_plans: &[EP], required_assumptions: transfer, forbidden_assumptions: no, expected_answer: Some("8"), expected_abstention: None },
        AnnotatedDevelopmentCase { question_id: "holdout.19", question: "Assume constant force; force is parallel to displacement. A 2 kg mass accelerates at 5 m/s2 through 0.4 m. What work is done?", support_status: SupportStatus::Supported, expected_target: "W", expected_givens: &[], allowed_plans: &[FW], required_assumptions: work, forbidden_assumptions: no, expected_answer: Some("4"), expected_abstention: None },
        AnnotatedDevelopmentCase { question_id: "holdout.20", question: "At constant velocity, a runner goes 12 m in 2 s. Determine velocity.", support_status: SupportStatus::Supported, expected_target: "v", expected_givens: &[], allowed_plans: &[CV_V], required_assumptions: cv, forbidden_assumptions: no, expected_answer: Some("6"), expected_abstention: None },
    ]
}

fn plan_edges(result: &OrchestratedAnswer) -> Vec<Vec<String>> {
    if let Some(plan) = &result.depth_two_plan {
        return vec![plan
            .steps
            .iter()
            .map(|step| step.edge.id.0.clone())
            .collect()];
    }
    result
        .planned_derivation
        .as_ref()
        .map(|trace| vec![vec![trace.edge_id.0.clone()]])
        .unwrap_or_default()
}

fn assumptions_in_result(result: &OrchestratedAnswer) -> Vec<MethodAssumption> {
    if let Some(plan) = &result.depth_two_plan {
        return plan
            .steps
            .iter()
            .flat_map(|step| {
                step.assumptions
                    .iter()
                    .map(|assumption| assumption.assumption.clone())
            })
            .collect();
    }
    result
        .planned_derivation
        .as_ref()
        .map(|trace| {
            trace
                .established_assumptions
                .iter()
                .map(|a| a.assumption.clone())
                .collect()
        })
        .unwrap_or_default()
}

fn expected_givens_match(result: &OrchestratedAnswer, expected: &[ExpectedGiven]) -> bool {
    expected.iter().all(|wanted| {
        result
            .plan
            .problem
            .givens
            .iter()
            .any(|given| given.variable == wanted.variable && given.value == wanted.value)
    })
}

fn provenance_complete(result: &OrchestratedAnswer) -> bool {
    let Some(plan_receipt) = &result.plan_execution_receipt else {
        // A direct plan has its source bindings on the edge trace.
        return result
            .planned_derivation
            .as_ref()
            .is_some_and(|trace| !trace.input_bindings.is_empty() || trace.target_binding == "P");
    };
    let mut sources: Vec<&str> = plan_receipt
        .step_receipts
        .iter()
        .flat_map(|receipt| {
            receipt
                .substituted_values
                .iter()
                .map(|binding| binding.source.as_str())
        })
        .collect();
    sources.extend(
        plan_receipt
            .intermediate_values
            .iter()
            .flat_map(|value| value.source_dependencies.iter().map(String::as_str)),
    );
    result
        .plan
        .problem
        .givens
        .iter()
        .all(|given| sources.iter().any(|source| *source == given.source))
}

pub fn evaluate_case(case_: &AnnotatedDevelopmentCase) -> DevelopmentCaseResult {
    let result = QuestionRouter::orchestrate(case_.question);
    let target_ok = if case_.expected_target.is_empty() {
        result.plan.problem.requested.is_none()
    } else {
        result.plan.problem.requested.as_deref() == Some(case_.expected_target)
    };
    let givens_ok = expected_givens_match(&result, case_.expected_givens);
    let actual_assumptions = assumptions_in_result(&result);
    let assumptions_ok = case_
        .required_assumptions
        .iter()
        .all(|required| actual_assumptions.contains(required))
        && case_
            .forbidden_assumptions
            .iter()
            .all(|forbidden| !actual_assumptions.contains(forbidden));
    let actual_plans = plan_edges(&result);
    let plan_ok = match case_.support_status {
        SupportStatus::Supported => case_.allowed_plans.iter().any(|allowed| {
            actual_plans.iter().any(|actual| {
                actual
                    .iter()
                    .map(String::as_str)
                    .eq(allowed.iter().copied())
            })
        }),
        SupportStatus::MustAbstain => actual_plans.is_empty(),
    };
    let provenance_ok = match case_.support_status {
        SupportStatus::Supported => provenance_complete(&result),
        SupportStatus::MustAbstain => true,
    };
    let answer_ok = result.answer.as_deref() == case_.expected_answer;
    let abstention_ok = case_.expected_abstention.is_none()
        || result.abstention_reason == case_.expected_abstention;
    let unsafe_execution = result.answer.is_some()
        && (result.evidence.is_empty()
            || result
                .plan_execution_receipt
                .as_ref()
                .is_some_and(|receipt| !receipt.final_verification.passed));
    let contract_ok = answer_ok
        && abstention_ok
        && target_ok
        && givens_ok
        && assumptions_ok
        && plan_ok
        && provenance_ok
        && !unsafe_execution;
    DevelopmentCaseResult {
        question_id: case_.question_id,
        answer: result.answer,
        abstention: result.abstention_reason,
        target_ok,
        givens_ok,
        assumptions_ok,
        plan_ok,
        provenance_ok,
        contract_ok,
        unsafe_execution,
        rejected: result
            .rejected_candidates
            .iter()
            .map(|candidate| candidate.reason.clone())
            .collect(),
    }
}

pub fn evaluate_mechanics_island() -> DevelopmentReport {
    DevelopmentReport {
        cases: mechanics_island_cases().iter().map(evaluate_case).collect(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn annotated_mechanics_island_has_25_cases_and_no_contract_failures() {
        let cases = mechanics_island_cases();
        assert_eq!(cases.len(), 25);
        let report = evaluate_mechanics_island();
        let failures = report.failures();
        assert!(failures.is_empty(), "development failures: {failures:?}");
        assert!(report.verified() >= 12, "report: {report:?}");
    }

    #[test]
    fn mutation_contract_contains_both_positive_and_negative_boundaries() {
        let cases = mechanics_island_cases();
        assert!(cases
            .iter()
            .any(|case_| case_.support_status == SupportStatus::Supported));
        assert!(cases
            .iter()
            .any(|case_| case_.support_status == SupportStatus::MustAbstain));
        assert!(cases.iter().any(|case_| case_
            .forbidden_assumptions
            .contains(&MethodAssumption::EnergyTransferredOverInterval)));
        assert!(cases.iter().any(|case_| case_.expected_abstention
            == Some(AbstentionReason::RequiredAssumptionContradicted)));
    }

    #[test]
    fn positive_to_negative_mutations_never_reuse_the_positive_plan() {
        let mutations = [
            "A 4 kg object moves at 3 m/s for 2 s. What is the power?",
            "A 4 kg object loses some kinetic energy over 2 s. What is the power?",
            "A 2 kg object accelerates at 3 m/s2 over 4 m. What work is done?",
            "A 2 kg object accelerates at 3 m/s2 over 4 m; the force is perpendicular to displacement. What work is done?",
            "A car travels at 3 m/s for 4 s. What distance does it travel?",
        ];
        for question in mutations {
            let result = QuestionRouter::orchestrate(question);
            assert!(
                result.answer.is_none(),
                "mutation was answered: {question}: {result:?}"
            );
            assert!(
                result.depth_two_plan.is_none(),
                "mutation retained a composition: {question}"
            );
        }
    }

    #[test]
    fn provenance_receipt_covers_every_given_in_composed_cases() {
        for case_ in mechanics_island_cases()
            .into_iter()
            .filter(|case_| case_.allowed_plans.iter().any(|plan| plan.len() == 2))
        {
            let result = QuestionRouter::orchestrate(case_.question);
            assert!(
                provenance_complete(&result),
                "incomplete receipt: {}",
                case_.question
            );
        }
    }

    #[test]
    fn cross_entity_work_handoff_abstains_until_identity_resolution_exists() {
        let result = QuestionRouter::orchestrate(
            "Assuming constant force and force is parallel to displacement, object A has mass 2 kg and acceleration 3 m/s2; object B moves 4 m. What work is done?",
        );
        assert!(
            result.answer.is_none(),
            "cross-entity work was answered: {result:?}"
        );
        assert!(
            result
                .rejected_candidates
                .iter()
                .any(|candidate| { candidate.reason == CandidateRejection::AmbiguousBinding }),
            "missing entity ambiguity: {:?}",
            result.rejected_candidates
        );
    }

    #[test]
    fn blind_holdout_preserves_the_supported_contract() {
        let cases = mechanics_holdout_cases();
        assert_eq!(cases.len(), 20);
        let results: Vec<_> = cases.iter().map(evaluate_case).collect();
        let correct = results.iter().filter(|result| result.contract_ok).count();
        let unsafe_count = results
            .iter()
            .filter(|result| result.unsafe_execution)
            .count();
        assert_eq!(unsafe_count, 0, "unsafe holdout executions: {results:?}");
        assert!(
            correct * 100 >= cases.len() * 85,
            "holdout {correct}/{}: {results:?}",
            cases.len()
        );
    }

    #[test]
    fn math_funnel_separates_bounded_algebra_from_specialized_math() {
        let equation = assess_math_funnel("Solve x + 3 = 7 for x.", "Math");
        assert_eq!(equation.task_kind, MathTaskKind::EquationSolving);
        assert!(equation.executor_candidate);

        let calculus = assess_math_funnel("Compute the integral of x^2 from 0 to 1.", "Math");
        assert_eq!(calculus.task_kind, MathTaskKind::ElementaryCalculus);
        assert!(!calculus.executor_candidate);

        let specialized = assess_math_funnel(
            "Compute the reduced Spin bordism of the classifying space of G2.",
            "Math",
        );
        assert_eq!(specialized.task_kind, MathTaskKind::AdvancedSpecialized);
        assert!(!specialized.executor_candidate);
    }

    #[test]
    fn finite_funnel_requires_explicit_sampling_semantics() {
        let explicit = assess_finite_math_funnel(
            "A fair die is rolled once. What is the probability of an even outcome?",
            "Math",
        );
        assert_eq!(
            explicit.task_kind,
            FiniteMathTaskKind::UniformFiniteProbability
        );
        assert_eq!(
            explicit.support,
            FiniteMathSupportAssessment::ExplicitBoundedOperation
        );
        assert!(explicit.uniformity_explicit);

        let ambiguous = assess_finite_math_funnel(
            "Three cards are drawn. What is the probability that all are aces?",
            "Math",
        );
        assert_eq!(
            ambiguous.support,
            FiniteMathSupportAssessment::MissingSamplingPolicy
        );
        assert!(!ambiguous.bounded_operation);
    }

    #[test]
    fn finite_funnel_rejects_domain_modeling_false_positives() {
        let graph = assess_finite_math_funnel(
            "For a graph, count closed tree-like walks using degree binomial terms.",
            "Math",
        );
        assert!(!graph.bounded_operation);
        assert_eq!(
            graph.support,
            FiniteMathSupportAssessment::RequiresAdvancedTheorem
        );
    }

    #[test]
    fn number_theory_funnel_requires_explicit_operands_and_target() {
        let gcd = assess_number_theory_funnel("Compute gcd(12348, 5436).", "Math");
        assert_eq!(gcd.task_kind, NumberTheoryTaskKind::GcdLcm);
        assert_eq!(
            gcd.support,
            NumberTheorySupportAssessment::ExplicitBoundedComputation
        );
        let theorem = assess_number_theory_funnel(
            "Prove that infinitely many primes have the required property.",
            "Math",
        );
        assert_eq!(theorem.task_kind, NumberTheoryTaskKind::Proof);
        assert_eq!(
            theorem.support,
            NumberTheorySupportAssessment::RequiresProof
        );
    }
}
