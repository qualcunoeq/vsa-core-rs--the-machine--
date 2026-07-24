//! A deliberately narrow, typed algebra execution island.
//!
//! This module is intentionally separate from the legacy CAS recognizer.  A
//! CAS result is admissible here only after an anchored prompt parser has
//! represented the operation, domain, constraints, and result semantics.
//! Unsupported prose returns `None`; it is never converted into a partial
//! expression or a guessed answer.

use crate::algebra::{self, SymExpr};
use std::collections::{BTreeMap, BTreeSet, HashMap};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AlgebraOperation {
    EvaluateExpression,
    SubstituteValues,
    SimplifyExpression,
    SolveLinearEquation,
    SolveQuadraticEquation,
    SolveSmallLinearSystem,
    CompareExpressions,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CoefficientDomain {
    ExactRational,
    RealNumeric,
}

/// Precise failure classes for the exact backend.  The public `try_answer`
/// API remains an intentional `Option` for router compatibility, while
/// receipts/tests can use these categories when a diagnostic API is added.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AlgebraFailure {
    IntegerOverflow,
    ZeroDenominator,
    UnsupportedComplexity,
    UnsupportedExpression,
    VerificationFailed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AlgebraContract {
    pub operation: AlgebraOperation,
    pub max_variables: usize,
    pub max_degree: Option<u32>,
    pub coefficient_domain: CoefficientDomain,
    pub supports_parameters: bool,
    pub supports_inequalities: bool,
}

impl AlgebraContract {
    pub fn for_operation(operation: AlgebraOperation) -> Self {
        Self {
            operation,
            max_variables: 1,
            max_degree: Some(2),
            coefficient_domain: CoefficientDomain::ExactRational,
            supports_parameters: false,
            supports_inequalities: false,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum AlgebraTarget {
    Evaluate(SymExpr),
    Simplify(SymExpr),
    SolveFor(String),
    Compare(SymExpr, SymExpr),
    SolveSystem(Vec<String>),
}

#[derive(Debug, Clone, PartialEq)]
pub struct AlgebraProblem {
    pub operation: AlgebraOperation,
    pub contract: AlgebraContract,
    pub equations: Vec<(SymExpr, SymExpr)>,
    pub expression: Option<SymExpr>,
    pub substitutions: HashMap<String, f64>,
    pub domain: String,
    pub constraints: Vec<String>,
    pub target: AlgebraTarget,
    pub source: String,
}

#[derive(Debug, Clone, PartialEq)]
pub enum AlgebraResult {
    ExactValue(String),
    ApproximateValue { value: f64, tolerance: f64 },
    UniqueSolution(BTreeMap<String, String>),
    FiniteSolutionSet(Vec<String>),
    NoSolution,
    InfiniteSolutions(String),
    EquivalentExpressions(bool),
}

#[derive(Debug, Clone, PartialEq)]
pub enum AlgebraStep {
    Parse,
    NormalizeEquation,
    CollectPolynomialCoefficients,
    ApplyLinearFormula,
    ApplyQuadraticFormula,
    Substitute,
    ReplayOriginalEquation,
    Canonicalize,
    GaussianElimination,
    ProveCompleteness,
}

/// Small exact number used by the bounded algebra island.  The parser still
/// accepts the existing expression AST, but constant rational subexpressions
/// are evaluated here without passing through binary floating point.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExactNumber {
    Integer(i128),
    Rational { numerator: i128, denominator: i128 },
}

impl ExactNumber {
    pub fn checked_rational(numerator: i128, denominator: i128) -> Result<Self, AlgebraFailure> {
        if denominator == 0 {
            return Err(AlgebraFailure::ZeroDenominator);
        }
        Self::rational(numerator, denominator).ok_or(AlgebraFailure::IntegerOverflow)
    }

    pub fn checked_add(self, rhs: Self) -> Result<Self, AlgebraFailure> {
        self.add(rhs).ok_or(AlgebraFailure::IntegerOverflow)
    }
    pub fn checked_sub(self, rhs: Self) -> Result<Self, AlgebraFailure> {
        self.sub(rhs).ok_or(AlgebraFailure::IntegerOverflow)
    }
    pub fn checked_mul(self, rhs: Self) -> Result<Self, AlgebraFailure> {
        self.mul(rhs).ok_or(AlgebraFailure::IntegerOverflow)
    }
    pub fn checked_neg(self) -> Result<Self, AlgebraFailure> {
        self.neg().ok_or(AlgebraFailure::IntegerOverflow)
    }
    pub fn checked_div(self, rhs: Self) -> Result<Self, AlgebraFailure> {
        if rhs.as_pair().0 == 0 {
            return Err(AlgebraFailure::ZeroDenominator);
        }
        self.div(rhs).ok_or(AlgebraFailure::IntegerOverflow)
    }

    fn rational(numerator: i128, denominator: i128) -> Option<Self> {
        if denominator == 0 {
            return None;
        }
        let mut n = numerator;
        let mut d = denominator;
        if d < 0 {
            n = n.checked_neg()?;
            d = d.checked_neg()?;
        }
        let g = gcd_i128(n, d);
        n /= g;
        d /= g;
        if d == 1 {
            Some(Self::Integer(n))
        } else {
            Some(Self::Rational {
                numerator: n,
                denominator: d,
            })
        }
    }

    fn as_pair(self) -> (i128, i128) {
        match self {
            Self::Integer(n) => (n, 1),
            Self::Rational {
                numerator,
                denominator,
            } => (numerator, denominator),
        }
    }

    fn add(self, rhs: Self) -> Option<Self> {
        let (a, b) = self.as_pair();
        let (c, d) = rhs.as_pair();
        let g = gcd_i128(b, d);
        let b1 = b / g;
        let d1 = d / g;
        let left = a.checked_mul(d1)?;
        let right = c.checked_mul(b1)?;
        Self::rational(left.checked_add(right)?, b1.checked_mul(d)?)
    }
    fn sub(self, rhs: Self) -> Option<Self> {
        self.add(rhs.neg()?)
    }
    fn mul(self, rhs: Self) -> Option<Self> {
        let (mut a, mut b) = self.as_pair();
        let (mut c, mut d) = rhs.as_pair();
        let g1 = gcd_i128(a, d);
        let g2 = gcd_i128(c, b);
        a /= g1;
        d /= g1;
        c /= g2;
        b /= g2;
        Self::rational(a.checked_mul(c)?, b.checked_mul(d)?)
    }
    fn div(self, rhs: Self) -> Option<Self> {
        let (mut a, mut b) = self.as_pair();
        let (mut c, mut d) = rhs.as_pair();
        if c == 0 {
            return None;
        }
        let g1 = gcd_i128(a, c);
        let g2 = gcd_i128(d, b);
        a /= g1;
        c /= g1;
        d /= g2;
        b /= g2;
        Self::rational(a.checked_mul(d)?, b.checked_mul(c)?)
    }
    fn neg(self) -> Option<Self> {
        let (a, b) = self.as_pair();
        Self::rational(a.checked_neg()?, b)
    }
    fn to_f64(self) -> f64 {
        let (n, d) = self.as_pair();
        n as f64 / d as f64
    }
    pub fn format(self) -> String {
        match self {
            Self::Integer(n) => n.to_string(),
            Self::Rational {
                numerator,
                denominator,
            } => format!("{numerator}/{denominator}"),
        }
    }
}

fn gcd_i128(a: i128, b: i128) -> i128 {
    let mut x = a.unsigned_abs();
    let mut y = b.unsigned_abs();
    while y != 0 {
        let r = x % y;
        x = y;
        y = r;
    }
    i128::try_from(x.max(1)).unwrap_or(i128::MAX)
}

#[derive(Debug, Clone, PartialEq)]
pub struct AlgebraVerificationReceipt {
    pub passed: bool,
    pub checks: Vec<String>,
    pub completeness: CompletenessVerification,
}

#[derive(Debug, Clone, PartialEq)]
pub enum CompletenessVerification {
    NotApplicable,
    LinearDegreeOne,
    QuadraticDiscriminant,
    TwoByTwoRankClassification,
}

#[derive(Debug, Clone, PartialEq)]
pub struct AlgebraExecutionReceipt {
    pub operation: AlgebraOperation,
    pub parsed_statements: Vec<String>,
    pub normalized_statements: Vec<String>,
    pub generated_constraints: Vec<String>,
    pub transformation_steps: Vec<AlgebraStep>,
    pub candidate_solutions: Vec<String>,
    pub rejected_solutions: Vec<String>,
    pub final_result: AlgebraResult,
    pub verification: AlgebraVerificationReceipt,
}

#[derive(Debug, Clone, PartialEq)]
pub struct AlgebraAnswer {
    pub answer: String,
    pub result: AlgebraResult,
    pub receipt: AlgebraExecutionReceipt,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AlgebraDevelopmentCase {
    pub id: &'static str,
    pub question: &'static str,
    pub expected: Option<&'static str>,
}

/// Frozen regression island.  These are deliberately authored independently
/// of benchmark traces: they test the contract, not a list of HLE strings.
pub fn development_cases() -> &'static [AlgebraDevelopmentCase] {
    &[
        AlgebraDevelopmentCase {
            id: "eval.01",
            question: "Compute (2 + 3) * 4",
            expected: Some("20"),
        },
        AlgebraDevelopmentCase {
            id: "eval.02",
            question: "Evaluate 3^2 + 4^2",
            expected: Some("25"),
        },
        AlgebraDevelopmentCase {
            id: "eval.03",
            question: "Compute 12 / 3 + 5",
            expected: Some("9"),
        },
        AlgebraDevelopmentCase {
            id: "eval.04",
            question: "Simplify 2 * (3 + 7)",
            expected: Some("20"),
        },
        AlgebraDevelopmentCase {
            id: "eval.05",
            question: "Evaluate -3 + 10",
            expected: Some("7"),
        },
        AlgebraDevelopmentCase {
            id: "sub.01",
            question: "Evaluate 2*x + 1 at x=3",
            expected: Some("7"),
        },
        AlgebraDevelopmentCase {
            id: "sub.02",
            question: "Substitute y=-2 into y^2 + 3",
            expected: Some("7"),
        },
        AlgebraDevelopmentCase {
            id: "sub.03",
            question: "Evaluate (x + 1) * (x - 1) at x=5",
            expected: Some("24"),
        },
        AlgebraDevelopmentCase {
            id: "sub.04",
            question: "Substitute z=4 into z/2 + 6",
            expected: Some("8"),
        },
        AlgebraDevelopmentCase {
            id: "sub.05",
            question: "Evaluate a^2 - 2*a at a=3",
            expected: Some("3"),
        },
        AlgebraDevelopmentCase {
            id: "linear.01",
            question: "Solve x + 3 = 7 for x",
            expected: Some("4"),
        },
        AlgebraDevelopmentCase {
            id: "linear.02",
            question: "Solve 2*x - 6 = 0 for x",
            expected: Some("3"),
        },
        AlgebraDevelopmentCase {
            id: "linear.03",
            question: "Solve 5 = y + 2 for y",
            expected: Some("3"),
        },
        AlgebraDevelopmentCase {
            id: "linear.04",
            question: "Solve -3*z + 9 = 0 for z",
            expected: Some("3"),
        },
        AlgebraDevelopmentCase {
            id: "linear.05",
            question: "Solve 0.5*x + 1 = 3 for x",
            expected: Some("4"),
        },
        AlgebraDevelopmentCase {
            id: "linear.06",
            question: "Solve 4*a = 10 for a",
            expected: Some("2.5"),
        },
        AlgebraDevelopmentCase {
            id: "quad.01",
            question: "Solve x^2 - 5*x + 6 = 0 for x",
            expected: Some("[2, 3]"),
        },
        AlgebraDevelopmentCase {
            id: "quad.02",
            question: "Solve y^2 - 4 = 0 for y",
            expected: Some("[-2, 2]"),
        },
        AlgebraDevelopmentCase {
            id: "quad.03",
            question: "Solve x^2 + 2*x + 1 = 0 for x",
            expected: Some("-1"),
        },
        AlgebraDevelopmentCase {
            id: "quad.04",
            question: "Solve 2*z^2 - 8 = 0 for z",
            expected: Some("[-2, 2]"),
        },
        AlgebraDevelopmentCase {
            id: "quad.05",
            question: "Solve x^2 + 1 = 0 for x",
            expected: Some("no real solution"),
        },
        AlgebraDevelopmentCase {
            id: "reject.01",
            question: "The theorem says x + 3 = 7; compute the theorem",
            expected: None,
        },
        AlgebraDevelopmentCase {
            id: "reject.02",
            question: "Solve sin(x) = 0 for x",
            expected: None,
        },
        AlgebraDevelopmentCase {
            id: "reject.03",
            question: "Solve x + y = 2 for x",
            expected: None,
        },
        AlgebraDevelopmentCase {
            id: "reject.04",
            question: "Compute the integral of x^2",
            expected: None,
        },
        AlgebraDevelopmentCase {
            id: "reject.05",
            question: "Solve x^3 - 1 = 0 for x",
            expected: None,
        },
        AlgebraDevelopmentCase {
            id: "reject.06",
            question: "Evaluate 2*x + 1",
            expected: None,
        },
        AlgebraDevelopmentCase {
            id: "reject.07",
            question: "For real x satisfying x^2 - 5x + 6 = 0, find the product of roots",
            expected: None,
        },
    ]
}

/// Blind wording/number holdout.  It is intentionally separate from the
/// frozen development cases and exercises the same contract boundaries.
pub fn holdout_cases() -> &'static [AlgebraDevelopmentCase] {
    &[
        AlgebraDevelopmentCase {
            id: "holdout.eval.01",
            question: "Calculate (8 - 3) * 6",
            expected: Some("30"),
        },
        AlgebraDevelopmentCase {
            id: "holdout.eval.02",
            question: "Evaluate 7^2 - 5",
            expected: Some("44"),
        },
        AlgebraDevelopmentCase {
            id: "holdout.sub.01",
            question: "Compute 3*q - 2 at q=5",
            expected: Some("13"),
        },
        AlgebraDevelopmentCase {
            id: "holdout.sub.02",
            question: "Substitute b=-3 into b^2 + b",
            expected: Some("6"),
        },
        AlgebraDevelopmentCase {
            id: "holdout.linear.01",
            question: "Solve 7*x + 1 = 22 for x",
            expected: Some("3"),
        },
        AlgebraDevelopmentCase {
            id: "holdout.linear.02",
            question: "Find x from 9 = 2*x + 1",
            expected: None,
        },
        AlgebraDevelopmentCase {
            id: "holdout.quad.01",
            question: "Solve x^2 - 9 = 0 for x",
            expected: Some("[-3, 3]"),
        },
        AlgebraDevelopmentCase {
            id: "holdout.quad.02",
            question: "Solve t^2 + 6*t + 9 = 0 for t",
            expected: Some("-3"),
        },
        AlgebraDevelopmentCase {
            id: "holdout.reject.01",
            question: "Prove that x^2 - 5x + 6 = 0 has roots",
            expected: None,
        },
        AlgebraDevelopmentCase {
            id: "holdout.reject.02",
            question: "Solve x^2 + y^2 = 1 for x",
            expected: None,
        },
    ]
}

/// Parse and execute a complete, bounded algebra request.  The grammar is
/// anchored so an equation embedded in a theorem or word problem is ignored.
pub fn try_answer(question: &str) -> Option<AlgebraAnswer> {
    let problem = parse_problem(question)?;
    execute(&problem)
}

pub fn parse_problem(question: &str) -> Option<AlgebraProblem> {
    let text = question.trim().trim_end_matches(['?', '.']).trim();
    if text.is_empty() || text.len() > 512 || text.contains('\n') {
        return None;
    }
    let lower = text.to_ascii_lowercase();

    // OOD system prompts often describe the same typed two-equation object
    // with prose rather than the development grammar.  Normalize those
    // forms into the exact same bounded 2x2 contract before considering any
    // one-equation fallback.  This remains deliberately narrow: exactly two
    // equations, two variables, and an explicit solve/determine intent.
    if let Some(problem) = parse_prose_linear_system(text, &lower) {
        return supported_problem(problem);
    }

    // Explicit comparison has a deterministic boolean result and no CAS.
    if let Some(body) = lower.strip_prefix("compare ") {
        let (left, right) = body.split_once('=')?;
        let lhs = algebra::parse(left.trim()).ok()?;
        let rhs = algebra::parse(right.trim()).ok()?;
        let mut problem = base(AlgebraOperation::CompareExpressions, text);
        problem.expression = Some(lhs.clone());
        problem.target = AlgebraTarget::Compare(lhs, rhs);
        return supported_problem(problem);
    }

    // Root-finding prose is normalized into the guarded single-variable
    // solver after checking that the relation names exactly one variable.
    for prefix in ["find all roots of ", "find the roots of "] {
        if let Some(body) = lower.strip_prefix(prefix) {
            let (lhs, rhs) = parse_equation(body)?;
            let mut variables = BTreeSet::new();
            collect_system_variables(&lhs, &mut variables);
            collect_system_variables(&rhs, &mut variables);
            let variable = variables.into_iter().collect::<Vec<_>>();
            if variable.len() != 1 {
                return None;
            }
            return parse_problem(&format!("Solve for {}: {body}", variable[0]));
        }
    }

    // A deliberately explicit 2×2 system grammar.  We accept either
    // `Solve system: e1; e2 for x,y` or `Solve e1 and e2 for x,y`; neither
    // form is allowed to fall through to the one-equation solver.
    if let Some(rest) = lower.strip_prefix("solve system") {
        let (equations_text, variables_text) = rest
            .trim()
            .trim_start_matches(':')
            .trim()
            .rsplit_once(" for ")?;
        let equations_text = equations_text.trim();
        let equation_parts: Vec<&str> = if equations_text.contains(';') {
            equations_text.split(';').map(str::trim).collect()
        } else {
            equations_text.split(" and ").map(str::trim).collect()
        };
        if equation_parts.len() != 2 {
            return None;
        }
        let variables: Vec<String> = variables_text
            .split(',')
            .map(str::trim)
            .filter(|v| v.len() == 1 && v.chars().all(|c| c.is_ascii_alphabetic()))
            .map(str::to_string)
            .collect();
        if variables.len() != 2 || variables[0] == variables[1] {
            return None;
        }
        let equations: Vec<(SymExpr, SymExpr)> = equation_parts
            .iter()
            .map(|part| parse_equation(part))
            .collect::<Option<_>>()?;
        let mut problem = base(AlgebraOperation::SolveSmallLinearSystem, text);
        problem.contract = AlgebraContract {
            operation: AlgebraOperation::SolveSmallLinearSystem,
            max_variables: 2,
            max_degree: Some(1),
            coefficient_domain: CoefficientDomain::ExactRational,
            supports_parameters: false,
            supports_inequalities: false,
        };
        problem.equations = equations;
        problem.target = AlgebraTarget::SolveSystem(variables);
        return supported_problem(problem);
    }

    // Same system grammar without the optional `system:` label.
    if let Some(rest) = lower.strip_prefix("solve ") {
        let system_rest = rest
            .strip_prefix("the system ")
            .or_else(|| rest.strip_prefix("the consistent system "))
            .or_else(|| rest.strip_prefix("the inconsistent system "))
            .unwrap_or(rest);
        if system_rest.contains(" and ") && system_rest.matches("=").count() == 2 {
            let (equations_text, variables_text) = system_rest
                .rsplit_once(" for ")
                .map(|(equations, variables)| (equations, Some(variables)))
                .unwrap_or((system_rest, None));
            let equation_parts: Vec<&str> = equations_text.split(" and ").map(str::trim).collect();
            if equation_parts.len() == 2 {
                let equations: Vec<(SymExpr, SymExpr)> = equation_parts
                    .iter()
                    .map(|part| parse_equation(part))
                    .collect::<Option<_>>()?;
                let variables: Vec<String> = if let Some(variables_text) = variables_text {
                    variables_text
                        .split(',')
                        .map(str::trim)
                        .filter(|v| v.len() == 1 && v.chars().all(|c| c.is_ascii_alphabetic()))
                        .map(str::to_string)
                        .collect()
                } else {
                    let mut inferred = BTreeSet::new();
                    for (lhs, rhs) in &equations {
                        collect_system_variables(lhs, &mut inferred);
                        collect_system_variables(rhs, &mut inferred);
                    }
                    inferred.into_iter().collect()
                };
                if variables.len() == 2 && variables[0] != variables[1] {
                    let mut problem = base(AlgebraOperation::SolveSmallLinearSystem, text);
                    problem.contract = AlgebraContract {
                        operation: AlgebraOperation::SolveSmallLinearSystem,
                        max_variables: 2,
                        max_degree: Some(1),
                        coefficient_domain: CoefficientDomain::ExactRational,
                        supports_parameters: false,
                        supports_inequalities: false,
                    };
                    problem.equations = equations;
                    problem.target = AlgebraTarget::SolveSystem(variables);
                    return supported_problem(problem);
                }
            }
        }
    }

    let (operation, body, variable, substitution) =
        if let Some(rest) = lower.strip_prefix("solve for ") {
            let (var, expr) = rest.split_once(':')?;
            (
                Some(AlgebraOperation::SolveLinearEquation),
                expr.trim(),
                Some(var.trim()),
                None,
            )
        } else if let Some(rest) = lower.strip_prefix("solve ") {
            let (expr, var) = rest.rsplit_once(" for ")?;
            (
                Some(AlgebraOperation::SolveLinearEquation),
                expr.trim(),
                Some(var.trim()),
                None,
            )
        } else if let Some(rest) = lower.strip_prefix("substitute ") {
            let (binding, expr) = rest.split_once(" into ")?;
            let (var, value) = binding.split_once('=')?;
            let value = value.trim().parse::<f64>().ok()?;
            (
                Some(AlgebraOperation::SubstituteValues),
                expr.trim(),
                Some(var.trim()),
                Some((var.trim().to_string(), value)),
            )
        } else if lower.starts_with("evaluate ")
            || lower.starts_with("compute ")
            || lower.starts_with("calculate ")
        {
            let expr = lower.split_once(' ')?.1.trim();
            if let Some((expr, binding)) = expr.rsplit_once(" at ") {
                let (var, value) = binding.split_once('=')?;
                let value = value.trim().parse::<f64>().ok()?;
                (
                    Some(AlgebraOperation::SubstituteValues),
                    expr.trim(),
                    Some(var.trim()),
                    Some((var.trim().to_string(), value)),
                )
            } else {
                (Some(AlgebraOperation::EvaluateExpression), expr, None, None)
            }
        } else if lower.starts_with("simplify ") {
            let expr = lower.split_once(' ')?.1.trim();
            (Some(AlgebraOperation::SimplifyExpression), expr, None, None)
        } else {
            (None, "", None, None)
        };
    let operation = operation?;
    let variable = variable
        .map(str::trim)
        .filter(|v| v.len() == 1 && v.chars().all(|c| c.is_ascii_alphabetic()));
    if matches!(operation, AlgebraOperation::SolveLinearEquation) && variable.is_none() {
        return None;
    }
    if body.matches('=').count() > 1 {
        return None;
    }

    let mut problem = base(operation, text);
    if let Some((name, value)) = substitution {
        problem.substitutions.insert(name, value);
    }
    match operation {
        AlgebraOperation::EvaluateExpression
        | AlgebraOperation::SimplifyExpression
        | AlgebraOperation::SubstituteValues => {
            let expr = parse_expression(body)?;
            problem.expression = Some(expr.clone());
            problem.target = if operation == AlgebraOperation::EvaluateExpression
                || operation == AlgebraOperation::SubstituteValues
            {
                AlgebraTarget::Evaluate(expr)
            } else {
                AlgebraTarget::Simplify(expr)
            };
        }
        AlgebraOperation::SolveLinearEquation => {
            let (lhs, rhs) = parse_equation(body)?;
            let var = variable?.to_string();
            let degree = polynomial_degree(&lhs, &var).max(polynomial_degree(&rhs, &var));
            if degree > 2 {
                return None;
            }
            problem.operation = if degree == 2 {
                AlgebraOperation::SolveQuadraticEquation
            } else {
                AlgebraOperation::SolveLinearEquation
            };
            problem.contract = AlgebraContract::for_operation(problem.operation);
            problem.equations.push((lhs, rhs));
            problem.target = AlgebraTarget::SolveFor(var);
        }
        _ => return None,
    }
    supported_problem(problem)
}

fn parse_prose_linear_system(source: &str, lower: &str) -> Option<AlgebraProblem> {
    if lower.matches('=').count() != 2
        || !(lower.contains(" and ") || lower.contains(" together with ") || lower.contains(';'))
        || !(lower.contains("solve")
            || lower.contains("determine")
            || lower.contains("find ")
            || lower.starts_with("use "))
        || lower.contains("whether")
        || lower.starts_with("can ")
    {
        return None;
    }

    let mut equations_text = lower.trim().trim_end_matches(['?', '.']).trim();
    let mut requested_variables: Option<String> = None;

    // Pull the variable list out of the common OOD request forms.  The
    // equation-bearing region is intentionally kept separate from the prose.
    if let Some((equations, vars)) = equations_text.split_once(" to determine ") {
        equations_text = equations.trim();
        requested_variables = Some(vars.trim().to_string());
    } else if let Some((equations, vars)) = equations_text.rsplit_once(" solve for ") {
        equations_text = equations.trim();
        requested_variables = Some(vars.trim().to_string());
    } else if let Some((equations, vars)) = equations_text.rsplit_once(" for ") {
        equations_text = equations.trim().trim_end_matches(',').trim();
        requested_variables = Some(vars.trim().to_string());
    } else if let Some((vars, equations)) = equations_text.split_once(" from ") {
        let vars = vars
            .trim()
            .trim_start_matches("find ")
            .trim_start_matches("determine ")
            .trim();
        equations_text = equations.trim();
        requested_variables = Some(vars.to_string());
    }

    // Remove the leading request prose left by forms such as
    // `Use ...`, `Solve simultaneously: ...`, and `The pair obeys ...`.
    for marker in [
        "for x,y, solve the simultaneous equations ",
        "solve simultaneously:",
        "solve simultaneously ",
        "solve the simultaneous equations ",
        "the pair obeys ",
        "solve the pair ",
        "the pair ",
        "the equations are ",
        "given ",
        "use ",
    ] {
        if let Some(rest) = equations_text.strip_prefix(marker) {
            equations_text = rest.trim();
            break;
        }
    }

    if let Some((equations, vars)) = equations_text
        .rsplit_once(" find the ordered pair ")
        .or_else(|| equations_text.rsplit_once(", find the ordered pair "))
    {
        equations_text = equations.trim();
        requested_variables = Some(vars.trim().to_string());
    }

    let delimiter = if equations_text.contains(" together with ") {
        " together with "
    } else if equations_text.contains(" and ") {
        " and "
    } else {
        ";"
    };
    let parts = equations_text
        .split(delimiter)
        .map(str::trim)
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>();
    if parts.len() != 2 {
        return None;
    }

    let equations = parts
        .iter()
        .map(|part| parse_equation_fragment(part))
        .collect::<Option<Vec<_>>>()?;

    let variables = requested_variables
        .as_deref()
        .map(parse_variable_list)
        .filter(|vars| vars.len() == 2 && vars[0] != vars[1])
        .or_else(|| {
            let mut inferred = BTreeSet::new();
            for (lhs, rhs) in &equations {
                collect_system_variables(lhs, &mut inferred);
                collect_system_variables(rhs, &mut inferred);
            }
            let inferred = inferred.into_iter().collect::<Vec<_>>();
            (inferred.len() == 2).then_some(inferred)
        })?;

    let mut problem = base(AlgebraOperation::SolveSmallLinearSystem, source);
    problem.contract = AlgebraContract {
        operation: AlgebraOperation::SolveSmallLinearSystem,
        max_variables: 2,
        max_degree: Some(1),
        coefficient_domain: CoefficientDomain::ExactRational,
        supports_parameters: false,
        supports_inequalities: false,
    };
    problem.equations = equations;
    problem.target = AlgebraTarget::SolveSystem(variables);
    Some(problem)
}

fn parse_variable_list(text: &str) -> Vec<String> {
    text.trim()
        .trim_end_matches(['?', '.'])
        .replace(" and ", ",")
        .split(',')
        .map(str::trim)
        .filter(|v| v.len() == 1 && v.chars().all(|c| c.is_ascii_alphabetic()))
        .map(str::to_string)
        .collect()
}

fn parse_equation_fragment(fragment: &str) -> Option<(SymExpr, SymExpr)> {
    let fragment = fragment.trim().trim_end_matches([',', '.', '?']).trim();
    // Find the first parseable equation boundary so leading words such as
    // `Use` or `Given` cannot become part of the expression AST.
    for (offset, ch) in fragment.char_indices() {
        if !(ch.is_ascii_alphanumeric() || ch == '(' || ch == '-') {
            continue;
        }
        if let Some(equation) = parse_equation(&fragment[offset..]) {
            return Some(equation);
        }
    }
    None
}

fn collect_system_variables(expr: &SymExpr, variables: &mut BTreeSet<String>) {
    match expr {
        SymExpr::Var(variable) => {
            variables.insert(variable.display.to_string());
        }
        SymExpr::Add(a, b)
        | SymExpr::Sub(a, b)
        | SymExpr::Mul(a, b)
        | SymExpr::Div(a, b)
        | SymExpr::Pow(a, b) => {
            collect_system_variables(a, variables);
            collect_system_variables(b, variables);
        }
        SymExpr::Neg(a)
        | SymExpr::Sin(a)
        | SymExpr::Cos(a)
        | SymExpr::Tan(a)
        | SymExpr::Sqrt(a)
        | SymExpr::Exp(a)
        | SymExpr::Ln(a)
        | SymExpr::Abs(a)
        | SymExpr::Sinh(a)
        | SymExpr::Cosh(a)
        | SymExpr::Tanh(a)
        | SymExpr::Asin(a)
        | SymExpr::Acos(a)
        | SymExpr::Atan(a) => collect_system_variables(a, variables),
        SymExpr::Num(_) => {}
        SymExpr::Limit {
            variable,
            approach,
            body,
        } => {
            variables.insert(variable.display.to_string());
            collect_system_variables(approach, variables);
            collect_system_variables(body, variables);
        }
        SymExpr::Integral {
            variable,
            lower,
            upper,
            body,
        } => {
            variables.insert(variable.display.to_string());
            if let Some(lower) = lower {
                collect_system_variables(lower, variables);
            }
            if let Some(upper) = upper {
                collect_system_variables(upper, variables);
            }
            collect_system_variables(body, variables);
        }
    }
}

fn base(operation: AlgebraOperation, source: &str) -> AlgebraProblem {
    AlgebraProblem {
        operation,
        contract: AlgebraContract::for_operation(operation),
        equations: Vec::new(),
        expression: None,
        substitutions: HashMap::new(),
        domain: "real".to_string(),
        constraints: Vec::new(),
        target: AlgebraTarget::Evaluate(SymExpr::Num(0.0)),
        source: source.to_string(),
    }
}

fn supported_problem(mut problem: AlgebraProblem) -> Option<AlgebraProblem> {
    let mut vars = BTreeSet::new();
    if let Some(expr) = &problem.expression {
        collect_vars(expr, &mut vars);
        if !supported_nodes(expr) {
            return None;
        }
    }
    for (lhs, rhs) in &problem.equations {
        collect_vars(lhs, &mut vars);
        collect_vars(rhs, &mut vars);
        if !supported_nodes(lhs) || !supported_nodes(rhs) {
            return None;
        }
    }
    if problem.operation == AlgebraOperation::SolveSmallLinearSystem {
        let AlgebraTarget::SolveSystem(names) = &problem.target else {
            return None;
        };
        if names.len() != 2 || problem.equations.len() != 2 {
            return None;
        }
        if vars
            .iter()
            .any(|name| !names.iter().any(|target| target == name))
        {
            return None;
        }
        if problem.equations.iter().any(|(lhs, rhs)| {
            names
                .iter()
                .any(|name| polynomial_degree(lhs, name) > 1 || polynomial_degree(rhs, name) > 1)
        }) {
            return None;
        }
    }
    if vars.len() > problem.contract.max_variables {
        return None;
    }
    if let AlgebraTarget::Evaluate(expr) = &problem.target {
        if problem.operation != AlgebraOperation::SubstituteValues && !vars.is_empty() {
            // Evaluation without explicit bindings is under-specified.  The
            // substitution operation will be added when its prose grammar is
            // available; never silently treat symbols as zero.
            return None;
        }
        if problem.operation == AlgebraOperation::SubstituteValues
            && vars
                .iter()
                .any(|var| !problem.substitutions.contains_key(var))
        {
            return None;
        }
        let _ = expr;
    }
    if problem.operation == AlgebraOperation::SolveQuadraticEquation {
        problem
            .constraints
            .push("real-domain roots only".to_string());
    }
    Some(problem)
}

fn parse_expression(text: &str) -> Option<SymExpr> {
    let normalized = text
        .replace('×', "*")
        .replace('÷', "/")
        .replace('−', "-")
        .replace(" to the power of ", "^")
        .replace(" multiplied by ", "*")
        .replace(" divided by ", "/")
        .replace(" times ", "*")
        .replace(" plus ", "+")
        .replace(" minus ", "-")
        .replace(" squared", "^2")
        .replace(" cubed", "^3");
    if normalized.is_empty()
        || !normalized
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || " +-*/^().,".contains(c))
        || normalized.contains(',')
    {
        return None;
    }
    algebra::parse(normalized.trim()).ok()
}

fn parse_equation(text: &str) -> Option<(SymExpr, SymExpr)> {
    let (left, right) = text.split_once('=')?;
    let lhs = parse_expression(left.trim())?;
    let rhs = parse_expression(right.trim())?;
    Some((lhs, rhs))
}

fn collect_vars(expr: &SymExpr, out: &mut BTreeSet<String>) {
    match expr {
        SymExpr::Var(v) => {
            out.insert(v.display.to_string());
        }
        SymExpr::Add(a, b)
        | SymExpr::Sub(a, b)
        | SymExpr::Mul(a, b)
        | SymExpr::Div(a, b)
        | SymExpr::Pow(a, b) => {
            collect_vars(a, out);
            collect_vars(b, out);
        }
        SymExpr::Neg(a)
        | SymExpr::Sin(a)
        | SymExpr::Cos(a)
        | SymExpr::Tan(a)
        | SymExpr::Sqrt(a)
        | SymExpr::Exp(a)
        | SymExpr::Ln(a)
        | SymExpr::Abs(a)
        | SymExpr::Sinh(a)
        | SymExpr::Cosh(a)
        | SymExpr::Tanh(a)
        | SymExpr::Asin(a)
        | SymExpr::Acos(a)
        | SymExpr::Atan(a) => collect_vars(a, out),
        SymExpr::Num(_) => {}
        SymExpr::Limit { body, approach, .. } => {
            collect_vars(body, out);
            collect_vars(approach, out);
        }
        SymExpr::Integral {
            body, lower, upper, ..
        } => {
            collect_vars(body, out);
            if let Some(x) = lower {
                collect_vars(x, out);
            }
            if let Some(x) = upper {
                collect_vars(x, out);
            }
        }
    }
}

fn supported_nodes(expr: &SymExpr) -> bool {
    match expr {
        SymExpr::Num(_) | SymExpr::Var(_) => true,
        SymExpr::Add(a, b) | SymExpr::Sub(a, b) | SymExpr::Mul(a, b) => {
            supported_nodes(a) && supported_nodes(b)
        }
        SymExpr::Neg(a) => supported_nodes(a),
        SymExpr::Div(a, b) => {
            matches!(b.as_ref(), SymExpr::Num(n) if *n != 0.0) && supported_nodes(a)
        }
        SymExpr::Pow(base, exp) => {
            matches!(exp.as_ref(), SymExpr::Num(n) if *n >= 0.0 && n.fract() == 0.0 && *n <= 2.0)
                && supported_nodes(base)
        }
        _ => false,
    }
}

fn polynomial_degree(expr: &SymExpr, var: &str) -> u32 {
    match expr {
        SymExpr::Num(_) => 0,
        SymExpr::Var(v) => u32::from(v.display.as_ref() == var),
        SymExpr::Add(a, b) | SymExpr::Sub(a, b) => {
            polynomial_degree(a, var).max(polynomial_degree(b, var))
        }
        SymExpr::Mul(a, b) => polynomial_degree(a, var).saturating_add(polynomial_degree(b, var)),
        SymExpr::Neg(a) => polynomial_degree(a, var),
        SymExpr::Div(a, _) => polynomial_degree(a, var),
        SymExpr::Pow(base, exp) => match exp.as_ref() {
            SymExpr::Num(n) if *n >= 0.0 && n.fract() == 0.0 => {
                polynomial_degree(base, var).saturating_mul(*n as u32)
            }
            _ => 99,
        },
        _ => 99,
    }
}

fn exact_from_f64(value: f64) -> Option<ExactNumber> {
    if !value.is_finite() {
        return None;
    }
    let text = format!("{value:.15}")
        .trim_end_matches('0')
        .trim_end_matches('.')
        .to_string();
    if let Some((whole, frac)) = text.split_once('.') {
        let sign = if whole.starts_with('-') {
            -1_i128
        } else {
            1_i128
        };
        let whole_abs = whole.trim_start_matches('-').parse::<i128>().ok()?;
        let scale = 10_i128.checked_pow(frac.len() as u32)?;
        let digits = frac.parse::<i128>().ok()?;
        ExactNumber::rational(
            sign.checked_mul(whole_abs.checked_mul(scale)?.checked_add(digits)?)?,
            scale,
        )
    } else {
        ExactNumber::rational(text.parse::<i128>().ok()?, 1)
    }
}

fn exact_constant(expr: &SymExpr) -> Option<ExactNumber> {
    match expr {
        SymExpr::Num(n) => exact_from_f64(*n),
        SymExpr::Neg(a) => exact_constant(a)?.neg(),
        SymExpr::Add(a, b) => exact_constant(a)?.add(exact_constant(b)?),
        SymExpr::Sub(a, b) => exact_constant(a)?.sub(exact_constant(b)?),
        SymExpr::Mul(a, b) => exact_constant(a)?.mul(exact_constant(b)?),
        SymExpr::Div(a, b) => exact_constant(a)?.div(exact_constant(b)?),
        SymExpr::Pow(a, b) => {
            let base = exact_constant(a)?;
            let exponent = exact_constant(b)?.as_pair();
            if exponent.1 != 1 || !(0..=2).contains(&exponent.0) {
                return None;
            }
            (0..exponent.0).try_fold(ExactNumber::Integer(1), |acc, _| acc.mul(base))
        }
        _ => None,
    }
}

fn exact_poly(expr: &SymExpr, var: &str) -> Option<(ExactNumber, ExactNumber, ExactNumber)> {
    let z = || ExactNumber::Integer(0);
    let o = || ExactNumber::Integer(1);
    match expr {
        SymExpr::Num(_) => Some((z(), z(), exact_constant(expr)?)),
        SymExpr::Var(v) if v.display.as_ref() == var => Some((z(), o(), z())),
        SymExpr::Var(_) => None,
        SymExpr::Add(a, b) => exact_poly_add(exact_poly(a, var)?, exact_poly(b, var)?, false),
        SymExpr::Sub(a, b) => exact_poly_add(exact_poly(a, var)?, exact_poly(b, var)?, true),
        SymExpr::Neg(a) => {
            let (q, l, c) = exact_poly(a, var)?;
            Some((q.neg()?, l.neg()?, c.neg()?))
        }
        SymExpr::Mul(a, b) => {
            let x = exact_poly(a, var)?;
            let y = exact_poly(b, var)?;
            Some((
                x.0.mul(y.2)?.add(x.1.mul(y.1)?)?.add(x.2.mul(y.0)?)?,
                x.1.mul(y.2)?.add(x.2.mul(y.1)?)?,
                x.2.mul(y.2)?,
            ))
        }
        SymExpr::Div(a, b) => {
            let den = exact_poly(b, var)?;
            if den.0 == z() && den.1 == z() {
                let (q, l, c) = exact_poly(a, var)?;
                Some((q.div(den.2)?, l.div(den.2)?, c.div(den.2)?))
            } else {
                None
            }
        }
        SymExpr::Pow(base, exp) => {
            let n = exact_constant(exp)?.as_pair();
            if n.1 != 1 || !(0..=2).contains(&n.0) {
                return None;
            }
            let p = exact_poly(base, var)?;
            match n.0 {
                0 => Some((z(), z(), o())),
                1 => Some(p),
                2 => Some((
                    p.1.mul(p.1)?
                        .add(p.0.mul(p.2)?.mul(ExactNumber::Integer(2))?)?,
                    p.1.mul(p.2)?.mul(ExactNumber::Integer(2))?,
                    p.2.mul(p.2)?,
                )),
                _ => None,
            }
        }
        _ => None,
    }
}

fn exact_poly_add(
    a: (ExactNumber, ExactNumber, ExactNumber),
    b: (ExactNumber, ExactNumber, ExactNumber),
    subtract: bool,
) -> Option<(ExactNumber, ExactNumber, ExactNumber)> {
    if subtract {
        Some((a.0.sub(b.0)?, a.1.sub(b.1)?, a.2.sub(b.2)?))
    } else {
        Some((a.0.add(b.0)?, a.1.add(b.1)?, a.2.add(b.2)?))
    }
}

fn terminating_decimal(n: i128, d: i128) -> Option<String> {
    let original_d = d.checked_abs()?;
    let mut d = original_d;
    let mut twos = 0;
    let mut fives = 0;
    while d % 2 == 0 {
        d /= 2;
        twos += 1;
    }
    while d % 5 == 0 {
        d /= 5;
        fives += 1;
    }
    if d != 1 {
        return None;
    }
    let places = twos.max(fives);
    let scale = 10_i128.checked_pow(places)?;
    let scaled = n.checked_mul(scale)?.checked_div(original_d.max(1))?;
    let sign = if scaled < 0 { "-" } else { "" };
    let abs = scaled.checked_abs()?;
    if places == 0 {
        Some(format!("{sign}{abs}"))
    } else {
        let raw = format!("{abs:0width$}", width = (places as usize) + 1);
        let split = raw.len() - places as usize;
        Some(
            format!("{sign}{}.{}", &raw[..split], &raw[split..])
                .trim_end_matches('0')
                .trim_end_matches('.')
                .to_string(),
        )
    }
}

fn format_exact(value: ExactNumber) -> String {
    let (n, d) = value.as_pair();
    terminating_decimal(n, d).unwrap_or_else(|| value.format())
}

fn exact_linear_coefficients(
    expr: &SymExpr,
    first: &str,
    second: &str,
) -> Option<(ExactNumber, ExactNumber, ExactNumber)> {
    let z = ExactNumber::Integer(0);
    let o = ExactNumber::Integer(1);
    match expr {
        SymExpr::Num(_) => Some((z, z, exact_constant(expr)?)),
        SymExpr::Var(v) if v.display.as_ref() == first => Some((o, z, z)),
        SymExpr::Var(v) if v.display.as_ref() == second => Some((z, o, z)),
        SymExpr::Var(_) => None,
        SymExpr::Add(a, b) => {
            let x = exact_linear_coefficients(a, first, second)?;
            let y = exact_linear_coefficients(b, first, second)?;
            Some((x.0.add(y.0)?, x.1.add(y.1)?, x.2.add(y.2)?))
        }
        SymExpr::Sub(a, b) => {
            let x = exact_linear_coefficients(a, first, second)?;
            let y = exact_linear_coefficients(b, first, second)?;
            Some((x.0.sub(y.0)?, x.1.sub(y.1)?, x.2.sub(y.2)?))
        }
        SymExpr::Neg(a) => {
            let x = exact_linear_coefficients(a, first, second)?;
            Some((x.0.neg()?, x.1.neg()?, x.2.neg()?))
        }
        SymExpr::Mul(a, b) => {
            let x = exact_linear_coefficients(a, first, second)?;
            let y = exact_linear_coefficients(b, first, second)?;
            if x.0 != z && (y.0 != z || y.1 != z) || x.1 != z && (y.0 != z || y.1 != z) {
                return None;
            }
            if x.0 != z || x.1 != z {
                Some((x.0.mul(y.2)?, x.1.mul(y.2)?, x.2.mul(y.2)?))
            } else {
                Some((y.0.mul(x.2)?, y.1.mul(x.2)?, y.2.mul(x.2)?))
            }
        }
        SymExpr::Div(a, b) => {
            let den = exact_constant(b)?;
            if den == z {
                None
            } else {
                let x = exact_linear_coefficients(a, first, second)?;
                Some((x.0.div(den)?, x.1.div(den)?, x.2.div(den)?))
            }
        }
        SymExpr::Pow(base, exp) => {
            let n = exact_constant(exp)?.as_pair();
            if n == (0, 1) {
                Some((z, z, o))
            } else if n == (1, 1) {
                exact_linear_coefficients(base, first, second)
            } else {
                None
            }
        }
        _ => None,
    }
}

fn exact_linear_row(
    equation: &(SymExpr, SymExpr),
    first: &str,
    second: &str,
) -> Option<(ExactNumber, ExactNumber, ExactNumber)> {
    let diff = equation.0.clone() - equation.1.clone();
    let (a, b, c) = exact_linear_coefficients(&diff, first, second)?;
    Some((a, b, c.neg()?))
}

fn exact_expression(expr: &SymExpr, bindings: &HashMap<String, f64>) -> Option<ExactNumber> {
    match expr {
        SymExpr::Num(_) => exact_constant(expr),
        SymExpr::Var(v) => exact_from_f64(*bindings.get(v.display.as_ref())?),
        SymExpr::Neg(a) => exact_expression(a, bindings)?.neg(),
        SymExpr::Add(a, b) => exact_expression(a, bindings)?.add(exact_expression(b, bindings)?),
        SymExpr::Sub(a, b) => exact_expression(a, bindings)?.sub(exact_expression(b, bindings)?),
        SymExpr::Mul(a, b) => exact_expression(a, bindings)?.mul(exact_expression(b, bindings)?),
        SymExpr::Div(a, b) => exact_expression(a, bindings)?.div(exact_expression(b, bindings)?),
        SymExpr::Pow(a, b) => {
            let base = exact_expression(a, bindings)?;
            let exponent = exact_expression(b, bindings)?.as_pair();
            if exponent.1 != 1 || !(0..=2).contains(&exponent.0) {
                return None;
            }
            (0..exponent.0).try_fold(ExactNumber::Integer(1), |acc, _| acc.mul(base))
        }
        _ => None,
    }
}

fn exact_sqrt(value: ExactNumber) -> Option<ExactNumber> {
    let (n, d) = value.as_pair();
    if n < 0 {
        return None;
    }
    let sn = integer_sqrt_u128(n as u128);
    let sd = integer_sqrt_u128(d as u128);
    if sn.checked_mul(sn)? == n && sd.checked_mul(sd)? == d {
        ExactNumber::rational(sn, sd)
    } else {
        None
    }
}

fn integer_sqrt_u128(value: u128) -> i128 {
    let mut low = 0_u128;
    let mut high = value.min(1_u128 << 64).saturating_add(1);
    while low + 1 < high {
        let mid = low + (high - low) / 2;
        if mid <= value / mid.max(1) {
            low = mid;
        } else {
            high = mid;
        }
    }
    i128::try_from(low).unwrap_or(i128::MAX)
}

fn execute(problem: &AlgebraProblem) -> Option<AlgebraAnswer> {
    let mut steps = vec![AlgebraStep::Parse];
    let mut parsed = Vec::new();
    let mut normalized = Vec::new();
    let mut constraints = problem.constraints.clone();
    let (result, candidates, rejected) = match (&problem.operation, &problem.target) {
        (AlgebraOperation::EvaluateExpression, AlgebraTarget::Evaluate(expr)) => {
            let exact = exact_expression(expr, &HashMap::new())?;
            steps.push(AlgebraStep::Canonicalize);
            let formatted = format_exact(exact);
            (
                AlgebraResult::ExactValue(formatted.clone()),
                vec![formatted],
                Vec::new(),
            )
        }
        (AlgebraOperation::SubstituteValues, AlgebraTarget::Evaluate(expr)) => {
            let exact = exact_expression(expr, &problem.substitutions)?;
            steps.push(AlgebraStep::Substitute);
            steps.push(AlgebraStep::Canonicalize);
            let formatted = format_exact(exact);
            (
                AlgebraResult::ExactValue(formatted.clone()),
                vec![formatted],
                Vec::new(),
            )
        }
        (AlgebraOperation::CompareExpressions, AlgebraTarget::Compare(lhs, rhs)) => {
            let difference = (lhs.clone() - rhs.clone()).simplify();
            let equivalent = matches!(difference, SymExpr::Num(value) if value.abs() <= 1e-12);
            steps.push(AlgebraStep::Canonicalize);
            (
                AlgebraResult::EquivalentExpressions(equivalent),
                Vec::new(),
                Vec::new(),
            )
        }
        (AlgebraOperation::SimplifyExpression, AlgebraTarget::Simplify(expr)) => {
            let simplified = expr.clone().simplify().to_string();
            steps.push(AlgebraStep::Canonicalize);
            (
                AlgebraResult::ExactValue(simplified.clone()),
                vec![simplified],
                Vec::new(),
            )
        }
        (
            AlgebraOperation::SolveLinearEquation | AlgebraOperation::SolveQuadraticEquation,
            AlgebraTarget::SolveFor(var),
        ) => {
            let (lhs, rhs) = problem.equations.first()?;
            parsed.push(format!("{} = {}", lhs, rhs));
            let diff = lhs.clone() - rhs.clone();
            normalized.push(format!("{} = 0", diff.clone().simplify()));
            if let Some((ea, eb, ec)) = exact_poly(&diff, var) {
                let zero = ExactNumber::Integer(0);
                if ea == zero && eb == zero {
                    steps.push(AlgebraStep::ProveCompleteness);
                    if ec == zero {
                        (
                            AlgebraResult::InfiniteSolutions(format!("{} is unconstrained", var)),
                            vec![],
                            vec![],
                        )
                    } else {
                        (AlgebraResult::NoSolution, vec![], vec![])
                    }
                } else if ea == zero {
                    let root = ec.neg()?.div(eb)?;
                    let formatted = format_exact(root);
                    constraints.push(format!("coefficient of {} is nonzero", var));
                    steps.push(AlgebraStep::CollectPolynomialCoefficients);
                    steps.push(AlgebraStep::ApplyLinearFormula);
                    steps.push(AlgebraStep::ProveCompleteness);
                    (
                        AlgebraResult::FiniteSolutionSet(vec![formatted.clone()]),
                        vec![formatted],
                        vec![],
                    )
                } else {
                    let disc = eb.mul(eb)?.sub(ea.mul(ec)?.mul(ExactNumber::Integer(4))?)?;
                    if let Some(root_disc) = exact_sqrt(disc) {
                        let denom = ea.mul(ExactNumber::Integer(2))?;
                        let r1 = eb.neg()?.sub(root_disc)?.div(denom)?;
                        let r2 = eb.neg()?.add(root_disc)?.div(denom)?;
                        let mut roots = vec![format_exact(r1), format_exact(r2)];
                        roots.sort();
                        roots.dedup();
                        steps.push(AlgebraStep::ApplyQuadraticFormula);
                        steps.push(AlgebraStep::ProveCompleteness);
                        (
                            AlgebraResult::FiniteSolutionSet(roots.clone()),
                            roots,
                            vec![],
                        )
                    } else {
                        let disc_f = disc.to_f64();
                        if disc_f < -1e-12 {
                            steps.push(AlgebraStep::ProveCompleteness);
                            (AlgebraResult::NoSolution, vec![], vec![])
                        } else {
                            let af = ea.to_f64();
                            let bf = eb.to_f64();
                            let _cf = ec.to_f64();
                            let s = disc_f.max(0.0).sqrt();
                            let mut roots = vec![(-bf - s) / (2.0 * af), (-bf + s) / (2.0 * af)];
                            roots.sort_by(|x, y| x.partial_cmp(y).unwrap());
                            let out = roots.iter().map(|r| format_float(*r)).collect::<Vec<_>>();
                            steps.push(AlgebraStep::ApplyQuadraticFormula);
                            steps.push(AlgebraStep::ProveCompleteness);
                            (AlgebraResult::FiniteSolutionSet(out.clone()), out, vec![])
                        }
                    }
                }
            } else {
                // Exact coefficient construction failed (including checked
                // overflow).  Do not silently downgrade the same equation to
                // floating point; that would defeat the exactness contract.
                return None;
            }
        }
        (AlgebraOperation::SolveSmallLinearSystem, AlgebraTarget::SolveSystem(names)) => {
            for (lhs, rhs) in &problem.equations {
                parsed.push(format!("{} = {}", lhs, rhs));
                normalized.push(format!("{} = 0", (lhs.clone() - rhs.clone()).simplify()));
            }
            let ((a11, a12, b1), (a21, a22, b2)) = (
                exact_linear_row(&problem.equations[0], &names[0], &names[1])?,
                exact_linear_row(&problem.equations[1], &names[0], &names[1])?,
            );
            steps.push(AlgebraStep::CollectPolynomialCoefficients);
            let det = a11.mul(a22)?.sub(a12.mul(a21)?)?;
            if det != ExactNumber::Integer(0) {
                let x = (b1.mul(a22)?.sub(a12.mul(b2)?)?).div(det)?;
                let y = (a11.mul(b2)?.sub(b1.mul(a21)?)?).div(det)?;
                let mut values = BTreeMap::new();
                values.insert(names[0].clone(), format_exact(x));
                values.insert(names[1].clone(), format_exact(y));
                steps.push(AlgebraStep::GaussianElimination);
                steps.push(AlgebraStep::ProveCompleteness);
                (AlgebraResult::UniqueSolution(values), vec![], Vec::new())
            } else {
                let consistent = a11.mul(b2)?.sub(a21.mul(b1)?)? == ExactNumber::Integer(0)
                    && a12.mul(b2)?.sub(a22.mul(b1)?)? == ExactNumber::Integer(0);
                steps.push(AlgebraStep::GaussianElimination);
                steps.push(AlgebraStep::ProveCompleteness);
                if consistent {
                    (
                        AlgebraResult::InfiniteSolutions(
                            "two-variable system has free variables".to_string(),
                        ),
                        vec![],
                        Vec::new(),
                    )
                } else {
                    (AlgebraResult::NoSolution, vec![], Vec::new())
                }
            }
        }
        _ => return None,
    };
    let mut checks = Vec::new();
    let mut passed = true;
    if let (AlgebraTarget::SolveFor(var), Some((lhs, rhs))) =
        (&problem.target, problem.equations.first())
    {
        steps.push(AlgebraStep::ReplayOriginalEquation);
        for candidate in &candidates {
            let value = parse_numeric(candidate)?;
            let l = lhs.evaluate(&[(var.as_str(), value)])?;
            let r = rhs.evaluate(&[(var.as_str(), value)])?;
            let ok = (l - r).abs() <= 1e-9_f64.max(l.abs().max(r.abs()) * 1e-9);
            checks.push(format!("{}={} replay {}", candidate, l, r));
            passed &= ok;
        }
    } else if let AlgebraTarget::SolveSystem(names) = &problem.target {
        if let AlgebraResult::UniqueSolution(values) = &result {
            let mut bindings = Vec::new();
            for name in names {
                bindings.push((name.as_str(), parse_numeric(values.get(name)?)?));
            }
            for (lhs, rhs) in &problem.equations {
                let l = lhs.evaluate(&bindings)?;
                let r = rhs.evaluate(&bindings)?;
                let ok = (l - r).abs() <= 1e-9_f64.max(l.abs().max(r.abs()) * 1e-9);
                checks.push(format!("system replay {}={} {}", l, r, ok));
                passed &= ok;
            }
            steps.push(AlgebraStep::ReplayOriginalEquation);
        } else {
            checks.push("rank/consistency classification for degenerate system".to_string());
        }
    } else {
        checks.push("deterministic AST evaluation".to_string());
    }
    let completeness = match problem.operation {
        AlgebraOperation::SolveLinearEquation => CompletenessVerification::LinearDegreeOne,
        AlgebraOperation::SolveQuadraticEquation => CompletenessVerification::QuadraticDiscriminant,
        AlgebraOperation::SolveSmallLinearSystem => {
            CompletenessVerification::TwoByTwoRankClassification
        }
        _ => CompletenessVerification::NotApplicable,
    };
    let verification = AlgebraVerificationReceipt {
        passed,
        checks,
        completeness,
    };
    if !verification.passed {
        return None;
    }
    let answer = match &result {
        AlgebraResult::ApproximateValue { value, .. } => format_float(*value),
        AlgebraResult::ExactValue(s) => s.clone(),
        AlgebraResult::FiniteSolutionSet(v) if v.len() == 1 => v[0].clone(),
        AlgebraResult::FiniteSolutionSet(v) => format!("[{}]", v.join(", ")),
        AlgebraResult::NoSolution => "no real solution".to_string(),
        AlgebraResult::InfiniteSolutions(s) => s.clone(),
        AlgebraResult::EquivalentExpressions(v) => v.to_string(),
        AlgebraResult::UniqueSolution(m) => format!("{:?}", m),
    };
    Some(AlgebraAnswer {
        answer,
        result: result.clone(),
        receipt: AlgebraExecutionReceipt {
            operation: problem.operation,
            parsed_statements: parsed,
            normalized_statements: normalized,
            generated_constraints: constraints,
            transformation_steps: steps,
            candidate_solutions: candidates,
            rejected_solutions: rejected,
            final_result: result,
            verification,
        },
    })
}

fn format_float(value: f64) -> String {
    if (value - value.round()).abs() < 1e-10 {
        (value.round() as i64).to_string()
    } else {
        format!("{value:.12}")
            .trim_end_matches('0')
            .trim_end_matches('.')
            .to_string()
    }
}

fn parse_numeric(text: &str) -> Option<f64> {
    if let Some((n, d)) = text.split_once('/') {
        let n = n.trim().parse::<f64>().ok()?;
        let d = d.trim().parse::<f64>().ok()?;
        if d.abs() < 1e-15 {
            None
        } else {
            Some(n / d)
        }
    } else {
        text.parse::<f64>().ok()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn check_case(case_: &AlgebraDevelopmentCase) {
        let actual = try_answer(case_.question).map(|answer| answer.answer);
        assert_eq!(actual.as_deref(), case_.expected, "case {}", case_.id);
        if let Some(answer) = try_answer(case_.question) {
            assert!(
                answer.receipt.verification.passed,
                "unverified {}",
                case_.id
            );
        }
    }
    #[test]
    fn evaluates_only_complete_expression() {
        assert_eq!(try_answer("Compute (2 + 3) * 4").unwrap().answer, "20");
        assert!(algebra::parse("-3 + 10").is_ok(), "raw parser");
        assert!(
            try_answer("Evaluate -3 + 10").is_some(),
            "negative expression parse: {:?}",
            parse_problem("Evaluate -3 + 10")
        );
        assert!(try_answer("The theorem says x + 3 = 7; compute the theorem").is_none());
    }
    #[test]
    fn substitutes_only_declared_values() {
        assert_eq!(try_answer("Evaluate 2*x + 1 at x=3").unwrap().answer, "7");
        assert_eq!(
            try_answer("Substitute x=4 into x^2 - 1").unwrap().answer,
            "15"
        );
        assert!(try_answer("Evaluate 2*x + 1").is_none());
    }
    #[test]
    fn compares_only_symbolically_equal_expressions() {
        assert_eq!(try_answer("Compare 2 + 2 = 4").unwrap().answer, "true");
        assert_eq!(try_answer("Compare x + 1 = x + 2").unwrap().answer, "false");
    }
    #[test]
    fn solves_linear_and_replays() {
        let a = try_answer("Solve x + 3 = 7 for x").unwrap();
        assert_eq!(a.answer, "4");
        assert!(a.receipt.verification.passed);
    }
    #[test]
    fn solves_quadratic_real_roots() {
        let a = try_answer("Solve x^2 - 5*x + 6 = 0 for x").unwrap();
        assert_eq!(a.answer, "[2, 3]");
        assert_eq!(
            try_answer("Find all roots of x^2 - 1 = 0").unwrap().answer,
            "[-1, 1]"
        );
    }
    #[test]
    fn solves_prose_two_by_two_systems_with_inferred_variables() {
        let answer = try_answer("Solve the consistent system x + y = 5 and x - y = 1").unwrap();
        assert_eq!(answer.answer, r#"{"x": "3", "y": "2"}"#);
    }
    #[test]
    fn system_parser_requires_explicit_solve_intent() {
        assert!(parse_problem("x=1 and y=2").is_none());
    }
    #[test]
    fn rejects_complex_and_unsupported_nodes() {
        assert_eq!(
            try_answer("Solve x^2 + 1 = 0 for x").unwrap().answer,
            "no real solution"
        );
        assert!(try_answer("Solve sin(x) = 0 for x").is_none());
    }
    #[test]
    fn preserves_degenerate_semantics() {
        assert_eq!(
            try_answer("Solve x = x for x").unwrap().answer,
            "x is unconstrained"
        );
        assert_eq!(
            try_answer("Solve x = x + 1 for x").unwrap().answer,
            "no real solution"
        );
    }
    #[test]
    fn frozen_development_corpus_is_contract_consistent() {
        for case_ in development_cases() {
            check_case(case_);
        }
    }
    #[test]
    fn blind_holdout_has_no_unsafe_execution() {
        for case_ in holdout_cases() {
            check_case(case_);
        }
    }
    #[test]
    fn exact_arithmetic_preserves_rationals() {
        assert_eq!(try_answer("Compute 1/3 + 1/6").unwrap().answer, "0.5");
        assert_eq!(try_answer("Solve x/3 = 1/2 for x").unwrap().answer, "1.5");
        assert_eq!(
            try_answer("Solve 2*x^2 - 8 = 0 for x").unwrap().answer,
            "[-2, 2]"
        );
    }
    #[test]
    fn exact_arithmetic_abstains_on_checked_overflow_and_reduces_first() {
        let max = ExactNumber::Integer(i128::MAX);
        assert!(max.add(ExactNumber::Integer(1)).is_none());
        assert!(max.mul(ExactNumber::Integer(2)).is_none());
        assert!(ExactNumber::rational(i128::MIN, -1).is_none());
        let reducible = ExactNumber::Rational {
            numerator: i128::MAX,
            denominator: 2,
        }
        .mul(ExactNumber::Integer(2))
        .unwrap();
        assert_eq!(reducible, max);
        assert!(ExactNumber::Integer(1)
            .div(ExactNumber::Integer(0))
            .is_none());
        assert_eq!(
            max.checked_add(ExactNumber::Integer(1)),
            Err(AlgebraFailure::IntegerOverflow)
        );
        assert_eq!(
            ExactNumber::Integer(1).checked_div(ExactNumber::Integer(0)),
            Err(AlgebraFailure::ZeroDenominator)
        );
        assert_eq!(
            ExactNumber::checked_rational(1, 0),
            Err(AlgebraFailure::ZeroDenominator)
        );
        assert_eq!(
            ExactNumber::checked_rational(i128::MIN, -1),
            Err(AlgebraFailure::IntegerOverflow)
        );
    }
    #[test]
    fn bounded_two_by_two_systems_classify_all_cases() {
        let unique = try_answer("Solve system: x + y = 5; x - y = 1 for x,y").unwrap();
        assert_eq!(unique.answer, "{\"x\": \"3\", \"y\": \"2\"}");
        assert!(unique.receipt.verification.passed);
        assert_eq!(
            try_answer("Solve system: x + y = 2; 2*x + 2*y = 4 for x,y")
                .unwrap()
                .answer,
            "two-variable system has free variables"
        );
        assert_eq!(
            try_answer("Solve system: x + y = 2; 2*x + 2*y = 5 for x,y")
                .unwrap()
                .answer,
            "no real solution"
        );
        assert!(try_answer("Solve system: x*y = 2; x + y = 3 for x,y").is_none());
    }
    #[test]
    fn router_does_not_fall_back_to_unbounded_cas() {
        let result = crate::router::QuestionRouter::orchestrate("Solve x + y = 2 for x");
        assert!(
            result.answer.is_none(),
            "unsupported multi-variable solve leaked through: {result:?}"
        );
    }
    #[test]
    fn router_trace_carries_typed_algebra_receipt_evidence() {
        let result = crate::router::QuestionRouter::orchestrate("Solve x + 3 = 7 for x");
        assert_eq!(result.answer.as_deref(), Some("[4]"));
        assert!(result
            .attempts
            .iter()
            .any(|attempt| attempt.contains("Algebra receipt")));
        assert!(result.evidence.iter().any(|evidence| matches!(
            evidence,
            crate::router::VerificationEvidence::ExecutableCheck { .. }
        )));
    }
}
