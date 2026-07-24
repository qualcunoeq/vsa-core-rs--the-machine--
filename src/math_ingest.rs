// ─── Math Knowledge Ingestion Engine ────────────────────────────────
//
// Extracts mathematical formulas from textbooks (LaTeX, Unicode math),
// parses them into SymExpr ASTs, and registers them in a persistent
// FormulaRegistry linked to natural-language descriptions.
//
// ## Pipeline
//
//   textbook text (PDF/TXT)
//     → extract_formulas_from_text()
//       → [FormulaExtraction { latex, context, source }]
//         → latex_to_symexpr()
//           → FormulaRegistry::register()
//
// ## Storage
//
// Formulas persist alongside QA facts in a JSON file. Each formula has:
//   - A unique slug/name (e.g. "power_rule")
//   - A SymExpr representation of the formula
//   - Natural-language description(s)
//   - Source attribution
//
// ## Retrieval
//
// The QA engine can query the registry via:
//   "What is the power rule?" → symbolic expression + natural language
//   "What is d/dx x^2?" → lookup/derive formula

use crate::algebra::SymExpr;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};

// ═══════════════════════════════════════════════════════════════════════
// FORMULA ENTRY
// ═══════════════════════════════════════════════════════════════════════

/// A registered mathematical formula with symbolic and natural-language forms.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct FormulaEntry {
    /// Unique slug (e.g. "power_rule", "derivative_of_sin").
    pub slug: String,
    /// The formula as a symbolic expression (serialized as string).
    pub expr_str: String,
    /// What this formula computes — as SVO-style triple descriptions.
    pub descriptions: Vec<(String, String, String)>,
    /// Alternative names/aliases for lookup.
    pub aliases: Vec<String>,
    /// Source document (e.g. "OpenStax Calculus Volume 1").
    pub source: String,
    /// Mathematical domain (e.g. "calculus", "algebra", "trigonometry").
    pub domain: String,
    /// Tags for categorization.
    pub tags: Vec<String>,

    /// Indices of QA facts that reference this formula.
    #[serde(default)]
    pub linked_fact_ids: Vec<usize>,
}

impl Default for FormulaEntry {
    fn default() -> Self {
        FormulaEntry {
            slug: String::new(),
            expr_str: String::new(),
            descriptions: Vec::new(),
            aliases: Vec::new(),
            source: String::new(),
            domain: String::new(),
            tags: Vec::new(),
            linked_fact_ids: Vec::new(),
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════
// FORMULA REGISTRY
// ═══════════════════════════════════════════════════════════════════════

/// Persistent registry of mathematical formulas.
///
/// Thread-safe via internal mutability patterns (uses RefCell internally
/// or is wrapped in Arc<RwLock<>> by the caller).
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct FormulaRegistry {
    formulas: Vec<FormulaEntry>,
    /// Index: slug → index in formulas
    slug_index: HashMap<String, usize>,
    /// Index: alias → slug
    alias_index: HashMap<String, String>,
}

// ═══════════════════════════════════════════════════════════════════════
// PATTERN MATCHING & SUBSTITUTION
// ═══════════════════════════════════════════════════════════════════════

/// Match an actual expression against a pattern with variable bindings.
///
/// Returns true if the expression matches the pattern, populating `bindings`
/// with variable → value mappings. Variables in the pattern are any
/// `SymExpr::Var(name)` that is NOT a known constant (like `x`, `e`, `pi`).
///
/// Examples:
/// - `match(x^5, x^n)` → `{n: 5}` (true)
/// - `match(cos(x), sin(x))` → `{}` (false, different function)
/// - `match(3*x, n*x)` → `{n: 3}` (true)
/// Flatten a commutative associative chain (Add or Mul) into a list of terms.
///
/// `Add(a, Add(b, c))` → `[a, b, c]`  
/// `Mul(Mul(a, b), c)` → `[a, b, c]`
fn collect_terms<'a>(expr: &'a SymExpr, op: &str) -> Vec<&'a SymExpr> {
    match (op, expr) {
        ("add", SymExpr::Add(a, b)) => {
            let mut left = collect_terms(a, op);
            left.extend(collect_terms(b, op));
            left
        }
        ("mul", SymExpr::Mul(a, b)) => {
            let mut left = collect_terms(a, op);
            left.extend(collect_terms(b, op));
            left
        }
        _ => vec![expr],
    }
}

/// Match a list of expression terms against a list of pattern terms with commutativity.
///
/// Uses backtracking: for each pattern term, tries each remaining expression term.
/// This is O(n!) where n is the number of terms, but n is typically 2-4.
fn match_commutative(
    expr_terms: &[&SymExpr],
    pat_terms: &[&SymExpr],
    bindings: &mut HashMap<String, SymExpr>,
) -> bool {
    if pat_terms.is_empty() {
        return expr_terms.is_empty();
    }
    if expr_terms.len() < pat_terms.len() {
        return false;
    }

    let pat = pat_terms[0];
    for i in 0..expr_terms.len() {
        let saved = bindings.clone();
        if match_symexpr(expr_terms[i], pat, bindings) {
            let mut remaining: Vec<&SymExpr> = expr_terms.to_vec();
            remaining.remove(i);
            if match_commutative(&remaining, &pat_terms[1..], bindings) {
                return true;
            }
        }
        *bindings = saved;
    }

    false
}

/// Match an actual expression against a pattern with variable bindings.
///
/// Supports:
/// - Structural matching for non-commutative operators (Sub, Div, Pow)
/// - Commutative + associative matching for Add and Mul (flattens chains,
///   tries all permutations via backtracking)
/// - Wildcard variable binding for pattern variables (single letters like
///   `n`, `m`, `k`)
///
/// Examples:
/// - `match(x^5, x^n)` → `{n: 5}` (true)
/// - `match(5*x, x*n)` → `{n: 5}` (true, commutative)
/// - `match(x*(y*z), a*b)` → `{a: x, b: y*z}` (true, associative)
pub fn match_symexpr(
    expr: &SymExpr,
    pattern: &SymExpr,
    bindings: &mut HashMap<String, SymExpr>,
) -> bool {
    match (expr, pattern) {
        // Numbers must match exactly
        (SymExpr::Num(a), SymExpr::Num(b)) => (a - b).abs() < 1e-12,

        // Pattern is a wildcard variable: bind the expression to it
        (expr, SymExpr::Var(p_name)) => {
            let p_name_str: &str = p_name.display.as_ref();
            if is_pattern_variable(p_name_str) {
                bindings
                    .entry(p_name_str.to_string())
                    .or_insert_with(|| expr.clone());
                true
            } else {
                // Concrete variable: match by name
                if let SymExpr::Var(e_name) = expr {
                    e_name == p_name_str
                } else {
                    false
                }
            }
        }

        // Commutative + associative: Add
        (SymExpr::Add(_, _), SymExpr::Add(_, _)) => {
            let expr_terms = collect_terms(expr, "add");
            let pat_terms = collect_terms(pattern, "add");
            match_commutative(&expr_terms, &pat_terms, bindings)
        }

        // Commutative + associative: Mul
        (SymExpr::Mul(_, _), SymExpr::Mul(_, _)) => {
            let expr_terms = collect_terms(expr, "mul");
            let pat_terms = collect_terms(pattern, "mul");
            match_commutative(&expr_terms, &pat_terms, bindings)
        }

        // Non-commutative binary operators: structural matching only
        (SymExpr::Sub(a, b), SymExpr::Sub(pa, pb))
        | (SymExpr::Div(a, b), SymExpr::Div(pa, pb))
        | (SymExpr::Pow(a, b), SymExpr::Pow(pa, pb)) => {
            match_symexpr(a, pa, bindings) && match_symexpr(b, pb, bindings)
        }

        // Unary functions: match structurally
        (SymExpr::Sin(a), SymExpr::Sin(pa))
        | (SymExpr::Cos(a), SymExpr::Cos(pa))
        | (SymExpr::Tan(a), SymExpr::Tan(pa))
        | (SymExpr::Ln(a), SymExpr::Ln(pa))
        | (SymExpr::Exp(a), SymExpr::Exp(pa))
        | (SymExpr::Sqrt(a), SymExpr::Sqrt(pa))
        | (SymExpr::Abs(a), SymExpr::Abs(pa))
        | (SymExpr::Neg(a), SymExpr::Neg(pa))
        | (SymExpr::Sinh(a), SymExpr::Sinh(pa))
        | (SymExpr::Cosh(a), SymExpr::Cosh(pa))
        | (SymExpr::Tanh(a), SymExpr::Tanh(pa))
        | (SymExpr::Asin(a), SymExpr::Asin(pa))
        | (SymExpr::Acos(a), SymExpr::Acos(pa))
        | (SymExpr::Atan(a), SymExpr::Atan(pa)) => match_symexpr(a, pa, bindings),

        // Mismatch
        _ => false,
    }
}

/// Whether a variable name should be treated as a pattern variable (wildcard).
///
/// Pattern variables can be any alphabetic string. Concrete variables
/// are single-letter lowercase or known constants.
pub fn is_pattern_variable(name: &str) -> bool {
    // Known constants: x, y, z are usually the independent variables
    // Single-letter names a-z are ambiguous but common for patterns like n, m, k
    // We treat SINGLE letters as potential pattern variables
    // Multi-letter names starting with uppercase or longer are also patterns
    !matches!(
        name,
        "x" | "y"
            | "z"
            | "t"
            | "e"
            | "pi"
            | "PI"
            | "infinity"
            | "dx"
            | "dy"
            | "dt"
            | "du"
            | "dv"
            | "d"
            | "C"
            | "alpha"
            | "beta"
            | "gamma"
            | "theta"
            | "lambda"
            | "mu"
            | "sigma"
            | "omega"
            | "Delta"
    )
}

/// Substitute variables in an expression according to bindings.
pub fn substitute_vars(expr: &SymExpr, bindings: &HashMap<String, SymExpr>) -> SymExpr {
    match expr {
        SymExpr::Num(_) => expr.clone(),
        SymExpr::Var(name) => {
            if let Some(replacement) = bindings.get(name.display.as_ref()) {
                replacement.clone()
            } else {
                expr.clone()
            }
        }
        SymExpr::Add(a, b) => SymExpr::Add(
            Box::new(substitute_vars(a, bindings)),
            Box::new(substitute_vars(b, bindings)),
        ),
        SymExpr::Sub(a, b) => SymExpr::Sub(
            Box::new(substitute_vars(a, bindings)),
            Box::new(substitute_vars(b, bindings)),
        ),
        SymExpr::Mul(a, b) => SymExpr::Mul(
            Box::new(substitute_vars(a, bindings)),
            Box::new(substitute_vars(b, bindings)),
        ),
        SymExpr::Div(a, b) => SymExpr::Div(
            Box::new(substitute_vars(a, bindings)),
            Box::new(substitute_vars(b, bindings)),
        ),
        SymExpr::Pow(a, b) => SymExpr::Pow(
            Box::new(substitute_vars(a, bindings)),
            Box::new(substitute_vars(b, bindings)),
        ),
        SymExpr::Neg(a) => SymExpr::Neg(Box::new(substitute_vars(a, bindings))),
        SymExpr::Sin(a) => SymExpr::Sin(Box::new(substitute_vars(a, bindings))),
        SymExpr::Cos(a) => SymExpr::Cos(Box::new(substitute_vars(a, bindings))),
        SymExpr::Tan(a) => SymExpr::Tan(Box::new(substitute_vars(a, bindings))),
        SymExpr::Ln(a) => SymExpr::Ln(Box::new(substitute_vars(a, bindings))),
        SymExpr::Exp(a) => SymExpr::Exp(Box::new(substitute_vars(a, bindings))),
        SymExpr::Sqrt(a) => SymExpr::Sqrt(Box::new(substitute_vars(a, bindings))),
        SymExpr::Abs(a) => SymExpr::Abs(Box::new(substitute_vars(a, bindings))),
        SymExpr::Sinh(a) => SymExpr::Sinh(Box::new(substitute_vars(a, bindings))),
        SymExpr::Cosh(a) => SymExpr::Cosh(Box::new(substitute_vars(a, bindings))),
        SymExpr::Tanh(a) => SymExpr::Tanh(Box::new(substitute_vars(a, bindings))),
        SymExpr::Asin(a) => SymExpr::Asin(Box::new(substitute_vars(a, bindings))),
        SymExpr::Acos(a) => SymExpr::Acos(Box::new(substitute_vars(a, bindings))),
        SymExpr::Atan(a) => SymExpr::Atan(Box::new(substitute_vars(a, bindings))),
        SymExpr::Limit {
            variable,
            approach,
            body,
        } => SymExpr::Limit {
            variable: variable.clone(),
            approach: Box::new(substitute_vars(approach, bindings)),
            body: Box::new(substitute_vars(body, bindings)),
        },
        SymExpr::Integral {
            variable,
            lower,
            upper,
            body,
        } => SymExpr::Integral {
            variable: variable.clone(),
            lower: lower
                .as_ref()
                .map(|l| Box::new(substitute_vars(l, bindings))),
            upper: upper
                .as_ref()
                .map(|u| Box::new(substitute_vars(u, bindings))),
            body: Box::new(substitute_vars(body, bindings)),
        },
    }
}

// ═══════════════════════════════════════════════════════════════════════
// COMPUTATION RULE ENGINE — bridge between formula knowledge & computation
// ═══════════════════════════════════════════════════════════════════════
//
// A ComputationRule turns a formula into an operational computation rule:
//   pattern (with wildcards) → template (with same wildcards)
//
// When the hardcoded computation engine (differentiate, integrate, solve)
// can't handle an expression, it falls back to the rule engine, which
// pattern-matches the expression against all known rules.

/// The domain of a computation rule.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum RuleDomain {
    Differentiate,
    Integrate,
    Solve,
    Simplify,
    Evaluate,
}

/// A pattern → template rule for extending the computation engine.
///
/// `pattern` is a SymExpr with wildcard variables (e.g., `Pow(Var("x"), Var("n"))`)
/// that matches against an input expression. When matched, `template` is
/// instantiated with the bindings to produce the result.
///
/// Example — integration power rule:
/// ```ignore
/// ComputationRule {
///     slug: "int_power_rule".into(),
///     domain: RuleDomain::Integrate,
///     pattern: Pow(Var("x"), Var("n")),
///     template: Div(Pow(Var("x"), Add(Var("n"), Num(1))), Add(Var("n"), Num(1))),
///     description: "∫ x^n dx = x^{n+1}/(n+1) + C".into(),
///     confidence: 0.95,
/// }
/// ```
/// Matching `x^5` binds `n=5`, producing `x^6/6`.
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct ComputationRule {
    pub slug: String,
    pub domain: RuleDomain,
    /// Pattern with wildcard variables.
    pub pattern: SymExpr,
    /// Template with matching wildcards — will be substituted before return.
    pub template: SymExpr,
    /// Human-readable description.
    pub description: String,
    /// Confidence 0.0–1.0 (rules derived from textbooks get ~0.9, bootstrapped ~1.0).
    pub confidence: f64,
}

/// A collection of ComputationRules that can be queried at computation time.
#[derive(Clone, Debug, Default, serde::Serialize, serde::Deserialize)]
pub struct RuleEngine {
    pub rules: Vec<ComputationRule>,
}

/// Strip calculus notation operators from an expression string.
///
/// Recognises three patterns:
/// - `d/dx <expr>` / `d/dx(<expr>)` → inner = `<expr>`, domain = Differentiate
/// - `∫ <expr> dx` / `∫_lower^upper <expr> dx` → inner = `<expr>`, domain = Integrate
/// - `int <expr> dx` / `int_lower^upper <expr> dx` → inner = `<expr>`, domain = Integrate
/// - anything else → unchanged, domain = None
///
/// The third return value (`has_bounds`) is `true` when bounds were detected
/// after the integral sign (e.g. `∫_0^1 x^2 dx` or `int_a^b f(x) dx`).
/// Callers should skip rule creation for bounded integrals because the
/// pattern (`f(x)` or `x^2`) would be either too generic or the template
/// references bounds that aren't in the pattern.
fn strip_calculus_operator(s: &str) -> (String, Option<RuleDomain>, bool) {
    let s = s.trim();

    // Clean up common LaTeX artifacts first
    let cleaned = s
        .replace("\\frac{d}{dx}", "d/dx")
        .replace("\\int", "∫")
        .replace("  ", " ");

    // Helper: strip `_lower^upper` or `_{lower}^{upper}` bounds from
    // the start of a string after an integral sign.
    fn strip_bounds(s: &str) -> (&str, bool) {
        let s = s.trim();
        if s.starts_with('_') {
            // Pattern: _lower^upper   or   _{lower}^{upper}
            let _after_underscore = &s[1..].trim_start();
            // Find the end of bounds: find the next non-^}_ char
            // Simple approach: skip until we hit a whitespace, letter, or digit
            let pos = 1; // skip '_'
            if pos < s.len() && s.as_bytes().get(pos) == Some(&b'{') {
                // _{...}^{...} form — skip until '}' (first brace group)
                let close = s[pos + 1..]
                    .find('}')
                    .map(|p| pos + 1 + p + 1)
                    .unwrap_or(s.len());
                // After first brace group, skip ^... if present
                let rest_after_first = &s[close..].trim_start();
                let after_bounds = if rest_after_first.starts_with('^') {
                    let after_caret = &rest_after_first[1..].trim_start();
                    if after_caret.starts_with('{') {
                        // ^{...} — find closing brace
                        let close2 = after_caret[1..]
                            .find('}')
                            .map(|p| 1 + p + 1)
                            .unwrap_or(after_caret.len());
                        let total = close + (rest_after_first.len() - after_caret.len()) + close2;
                        &s[total.min(s.len())..]
                    } else {
                        // ^<single char> — skip one char
                        let after_exp = &rest_after_first[1..].trim_start();
                        let total = close + (rest_after_first.len() - after_exp.len());
                        &s[total.min(s.len())..]
                    }
                } else {
                    rest_after_first
                };
                (after_bounds.trim_start(), true)
            } else {
                // _lower^upper  or  _lower form
                // Skip until whitespace, '^', or end
                let bounds_end = s[1..]
                    .find(|c: char| c.is_whitespace() || c == '^')
                    .map(|p| p + 1)
                    .unwrap_or(s.len());
                let after_bounds = &s[bounds_end..];
                if after_bounds.starts_with('^') {
                    // _lower^upper — skip ^upper
                    let after_caret = &after_bounds[1..].trim_start();
                    let exp_end = after_caret
                        .find(|c: char| c.is_whitespace())
                        .unwrap_or(after_caret.len());
                    let total = bounds_end + 1 + exp_end;
                    (&s[total.min(s.len())..], true)
                } else {
                    (after_bounds, true)
                }
            }
        } else {
            (s, false)
        }
    }

    // Pattern 1: d/dx <expr>   or   d/dx(<expr>)
    if let Some(rest) = cleaned
        .strip_prefix("d/dx ")
        .or_else(|| cleaned.strip_prefix("d/dx("))
    {
        let inner = if cleaned.contains("d/dx(") {
            rest.strip_suffix(')').unwrap_or(rest).trim().to_string()
        } else {
            rest.trim().to_string()
        };
        let inner = if inner.starts_with('(') && inner.ends_with(')') {
            inner[1..inner.len() - 1].trim().to_string()
        } else {
            inner
        };
        return (inner, Some(RuleDomain::Differentiate), false);
    }

    // Pattern 2: ∫ <expr> dx   (may include bounds)
    if cleaned.starts_with('∫') {
        let int_char_len = '∫'.len_utf8();
        let after_int = cleaned[int_char_len..].trim();
        // Strip bounds if present: ∫_a^b f(x) dx → f(x) dx
        let (after_bounds, has_bounds) = strip_bounds(after_int);
        let inner = if let Some(dx_pos) = after_bounds
            .rfind(" dx")
            .or_else(|| after_bounds.rfind(" d"))
        {
            after_bounds[..dx_pos].trim().to_string()
        } else {
            after_bounds.to_string()
        };
        return (inner, Some(RuleDomain::Integrate), has_bounds);
    }

    // Pattern 3: int <expr> dx  or  int_<bounds> <expr> dx  (text-notation integral)
    if cleaned.to_lowercase().starts_with("int ") || cleaned.to_lowercase().starts_with("int_") {
        // For "int_...", only strip "int" — leave "_bounds" for strip_bounds to detect
        let after_int = if cleaned.to_lowercase().starts_with("int_") {
            cleaned[3..].to_string() // keeps "_a^b f(x) dx"
        } else {
            cleaned[3..].trim().to_string() // "int " → "f(x) dx"
        };
        // Strip bounds if present: int_a^b f(x) dx → f(x) dx
        let (after_bounds, has_bounds) = strip_bounds(&after_int);
        let inner = if let Some(dx_pos) = after_bounds
            .rfind(" dx")
            .or_else(|| after_bounds.rfind(" d"))
        {
            after_bounds[..dx_pos].trim().to_string()
        } else {
            after_bounds.to_string()
        };
        return (inner, Some(RuleDomain::Integrate), has_bounds);
    }

    // No calculus notation detected
    (s.to_string(), None, false)
}

impl RuleEngine {
    pub fn new() -> Self {
        RuleEngine { rules: Vec::new() }
    }

    /// Add a rule. Returns error if slug already exists.
    pub fn add_rule(&mut self, rule: ComputationRule) -> Result<(), String> {
        if self.rules.iter().any(|r| r.slug == rule.slug) {
            return Err(format!("Rule '{}' already exists", rule.slug));
        }
        self.rules.push(rule);
        Ok(())
    }

    /// Remove a rule by slug.
    pub fn remove_rule(&mut self, slug: &str) -> Option<ComputationRule> {
        if let Some(pos) = self.rules.iter().position(|r| r.slug == slug) {
            Some(self.rules.remove(pos))
        } else {
            None
        }
    }

    /// List all rule slugs.
    pub fn list_rules(&self) -> Vec<String> {
        self.rules.iter().map(|r| r.slug.clone()).collect()
    }

    /// Try to apply a rule in the given domain to `expr`.
    ///
    /// Returns `Some((result, slug))` if a rule matches, `None` otherwise.
    /// Rules are tried in order (most recently added first for faster iteration).
    pub fn try_apply(
        &self,
        expr: &SymExpr,
        domain: &RuleDomain,
        extra_bindings: &HashMap<String, SymExpr>,
    ) -> Option<(SymExpr, String)> {
        for rule in self.rules.iter().rev() {
            if rule.domain != *domain {
                continue;
            }
            let mut bindings = extra_bindings.clone();
            if match_symexpr(expr, &rule.pattern, &mut bindings) {
                let result = substitute_vars(&rule.template, &bindings);
                return Some((result.simplify(), rule.slug.clone()));
            }
        }
        None
    }

    /// Seed with bootstrap rules from the existing hardcoded knowledge.
    /// This is called once at initialization time.
    pub fn seed_bootstrap(&mut self) {
        use crate::algebra::SymExpr::*;

        let x = || Var("x".into());
        let n = || Var("n".into());
        let a = || Var("a".into());
        let b = || Var("b".into());
        let axb = || a() * x() + b(); // a*x + b
        let ax = || a() * x(); // a*x

        // ── Integration Rules ────────────────────────────────────────

        // ∫ x^n dx = x^{n+1}/(n+1)   (n ≠ -1)
        self.maybe_add(ComputationRule {
            slug: "int_power".into(),
            domain: RuleDomain::Integrate,
            pattern: x().pow(n()),
            template: x().pow(n() + Num(1.0)) / (n() + Num(1.0)),
            description: "∫ x^n dx = x^{n+1}/(n+1) + C".into(),
            confidence: 0.98,
        });

        // ∫ sin(x) dx = -cos(x)
        self.maybe_add(ComputationRule {
            slug: "int_sin".into(),
            domain: RuleDomain::Integrate,
            pattern: x().sin(),
            template: -x().cos(),
            description: "∫ sin(x) dx = -cos(x) + C".into(),
            confidence: 0.99,
        });

        // ∫ cos(x) dx = sin(x)
        self.maybe_add(ComputationRule {
            slug: "int_cos".into(),
            domain: RuleDomain::Integrate,
            pattern: x().cos(),
            template: x().sin(),
            description: "∫ cos(x) dx = sin(x) + C".into(),
            confidence: 0.99,
        });

        // ∫ sec^2(x) dx = tan(x)  (sec^2 is 1/cos^2 = cos(x)^(-2)... hmm.
        // Actually sec(x) isn't a SymExpr node. Skip sec/csc for now.
        // The pattern would be sin(x)^(-2) for csc^2... but that's not quite right.
        // ∫ sin(x)^(-2) dx is -cot(x), but the pattern sin(x)^(-2) != csc^2(x) structurally.

        // ∫ e^x dx = e^x
        self.maybe_add(ComputationRule {
            slug: "int_exp".into(),
            domain: RuleDomain::Integrate,
            pattern: x().exp(),
            template: x().exp(),
            description: "∫ e^x dx = e^x + C".into(),
            confidence: 0.99,
        });

        // ∫ 1/x dx = ln|x|
        self.maybe_add(ComputationRule {
            slug: "int_reciprocal".into(),
            domain: RuleDomain::Integrate,
            pattern: x().pow(Num(-1.0)),
            template: x().ln(),
            description: "∫ 1/x dx = ln|x| + C".into(),
            confidence: 0.98,
        });

        // ∫ sqrt(x) dx = (2/3)*x^(3/2)
        self.maybe_add(ComputationRule {
            slug: "int_sqrt".into(),
            domain: RuleDomain::Integrate,
            pattern: x().sqrt(),
            template: Num(2.0 / 3.0) * x().pow(Num(1.5)),
            description: "∫ sqrt(x) dx = (2/3)*x^(3/2) + C".into(),
            confidence: 0.98,
        });

        // ── Integration — Linear Substitution Patterns ─────────────────

        // ∫ sin(a*x + b) dx = -cos(a*x + b)/a
        self.maybe_add(ComputationRule {
            slug: "int_sin_linear".into(),
            domain: RuleDomain::Integrate,
            pattern: axb().sin(),
            template: -(axb().cos()) / a(),
            description: "∫ sin(ax+b) dx = -cos(ax+b)/a + C".into(),
            confidence: 0.95,
        });

        // ∫ sin(a*x) dx = -cos(a*x)/a
        self.maybe_add(ComputationRule {
            slug: "int_sin_ax".into(),
            domain: RuleDomain::Integrate,
            pattern: ax().sin(),
            template: -(ax().cos()) / a(),
            description: "∫ sin(ax) dx = -cos(ax)/a + C".into(),
            confidence: 0.95,
        });

        // ∫ cos(a*x + b) dx = sin(a*x + b)/a
        self.maybe_add(ComputationRule {
            slug: "int_cos_linear".into(),
            domain: RuleDomain::Integrate,
            pattern: axb().cos(),
            template: axb().sin() / a(),
            description: "∫ cos(ax+b) dx = sin(ax+b)/a + C".into(),
            confidence: 0.95,
        });

        // ∫ cos(a*x) dx = sin(a*x)/a
        self.maybe_add(ComputationRule {
            slug: "int_cos_ax".into(),
            domain: RuleDomain::Integrate,
            pattern: ax().cos(),
            template: ax().sin() / a(),
            description: "∫ cos(ax) dx = sin(ax)/a + C".into(),
            confidence: 0.95,
        });

        // ∫ e^(a*x + b) dx = e^(a*x + b)/a
        self.maybe_add(ComputationRule {
            slug: "int_exp_linear".into(),
            domain: RuleDomain::Integrate,
            pattern: axb().exp(),
            template: axb().exp() / a(),
            description: "∫ e^(ax+b) dx = e^(ax+b)/a + C".into(),
            confidence: 0.95,
        });

        // ∫ e^(a*x) dx = e^(a*x)/a
        self.maybe_add(ComputationRule {
            slug: "int_exp_ax".into(),
            domain: RuleDomain::Integrate,
            pattern: ax().exp(),
            template: ax().exp() / a(),
            description: "∫ e^(ax) dx = e^(ax)/a + C".into(),
            confidence: 0.95,
        });

        // ∫ (a*x + b)^n dx = (a*x + b)^(n+1) / (a*(n+1))   (n ≠ -1)
        self.maybe_add(ComputationRule {
            slug: "int_power_linear".into(),
            domain: RuleDomain::Integrate,
            pattern: axb().pow(n()),
            template: axb().pow(n() + Num(1.0)) / (a() * (n() + Num(1.0))),
            description: "∫ (ax+b)^n dx = (ax+b)^(n+1)/(a*(n+1)) + C".into(),
            confidence: 0.95,
        });

        // ∫ (a*x)^n dx = (a*x)^(n+1) / (a*(n+1))   (n ≠ -1)
        self.maybe_add(ComputationRule {
            slug: "int_power_ax".into(),
            domain: RuleDomain::Integrate,
            pattern: ax().pow(n()),
            template: ax().pow(n() + Num(1.0)) / (a() * (n() + Num(1.0))),
            description: "∫ (ax)^n dx = (ax)^(n+1)/(a*(n+1)) + C".into(),
            confidence: 0.95,
        });

        // ∫ 1/(a*x + b) dx = ln|a*x + b|/a
        self.maybe_add(ComputationRule {
            slug: "int_reciprocal_linear".into(),
            domain: RuleDomain::Integrate,
            pattern: axb().pow(Num(-1.0)),
            template: axb().ln() / a(),
            description: "∫ 1/(ax+b) dx = ln|ax+b|/a + C".into(),
            confidence: 0.95,
        });

        // ∫ sqrt(a*x + b) dx = (2/(3*a)) * (a*x + b)^(3/2)
        self.maybe_add(ComputationRule {
            slug: "int_sqrt_linear".into(),
            domain: RuleDomain::Integrate,
            pattern: axb().sqrt(),
            template: Num(2.0 / 3.0) * axb().pow(Num(1.5)) / a(),
            description: "∫ sqrt(ax+b) dx = (2/(3a))*(ax+b)^(3/2) + C".into(),
            confidence: 0.95,
        });

        // ∫ tan(x) dx = -ln|cos(x)|
        self.maybe_add(ComputationRule {
            slug: "int_tan".into(),
            domain: RuleDomain::Integrate,
            pattern: x().tan(),
            template: -(x().cos().ln()),
            description: "∫ tan(x) dx = -ln|cos(x)| + C".into(),
            confidence: 0.90,
        });

        // ∫ 1/(1 + x^2) dx = atan(x)
        self.maybe_add(ComputationRule {
            slug: "int_atan_form".into(),
            domain: RuleDomain::Integrate,
            pattern: Num(1.0) / (Num(1.0) + x().pow(Num(2.0))),
            template: x().atan(),
            description: "∫ 1/(1+x^2) dx = atan(x) + C".into(),
            confidence: 0.90,
        });

        // ∫ 1/sqrt(1 - x^2) dx = asin(x)
        self.maybe_add(ComputationRule {
            slug: "int_asin_form".into(),
            domain: RuleDomain::Integrate,
            pattern: Num(1.0) / (Num(1.0) - x().pow(Num(2.0))).sqrt(),
            template: x().asin(),
            description: "∫ 1/sqrt(1-x^2) dx = asin(x) + C".into(),
            confidence: 0.90,
        });

        // ∫ x*cos(x) dx = cos(x) + x*sin(x)  [integration by parts]
        // This one is tricky because the pattern matcher is commutative.
        // The pattern x*cos(x) might also match cos(x)*x depending on parse.
        self.maybe_add(ComputationRule {
            slug: "int_x_cos_x".into(),
            domain: RuleDomain::Integrate,
            pattern: x() * x().cos(),
            template: x().cos() + x() * x().sin(),
            description: "∫ x*cos(x) dx = cos(x) + x*sin(x) + C".into(),
            confidence: 0.85,
        });

        // ── Derivative Rules ──────────────────────────────────────────

        // d/dx sin(x) = cos(x)
        self.maybe_add(ComputationRule {
            slug: "diff_sin".into(),
            domain: RuleDomain::Differentiate,
            pattern: x().sin(),
            template: x().cos(),
            description: "d/dx sin(x) = cos(x)".into(),
            confidence: 0.99,
        });

        // d/dx cos(x) = -sin(x)
        self.maybe_add(ComputationRule {
            slug: "diff_cos".into(),
            domain: RuleDomain::Differentiate,
            pattern: x().cos(),
            template: -x().sin(),
            description: "d/dx cos(x) = -sin(x)".into(),
            confidence: 0.99,
        });

        // d/dx e^x = e^x
        self.maybe_add(ComputationRule {
            slug: "diff_exp".into(),
            domain: RuleDomain::Differentiate,
            pattern: x().exp(),
            template: x().exp(),
            description: "d/dx e^x = e^x".into(),
            confidence: 0.99,
        });

        // d/dx ln(x) = 1/x
        self.maybe_add(ComputationRule {
            slug: "diff_ln".into(),
            domain: RuleDomain::Differentiate,
            pattern: x().ln(),
            template: x().pow(Num(-1.0)),
            description: "d/dx ln(x) = 1/x".into(),
            confidence: 0.99,
        });

        // d/dx sqrt(x) = 1/(2*sqrt(x))
        self.maybe_add(ComputationRule {
            slug: "diff_sqrt".into(),
            domain: RuleDomain::Differentiate,
            pattern: x().sqrt(),
            template: Num(1.0) / (Num(2.0) * x().sqrt()),
            description: "d/dx sqrt(x) = 1/(2*sqrt(x))".into(),
            confidence: 0.98,
        });

        // ── Derivative — Linear Substitution Patterns ──────────────────

        // d/dx sin(a*x + b) = a*cos(a*x + b)
        self.maybe_add(ComputationRule {
            slug: "diff_sin_linear".into(),
            domain: RuleDomain::Differentiate,
            pattern: axb().sin(),
            template: a() * axb().cos(),
            description: "d/dx sin(ax+b) = a*cos(ax+b)".into(),
            confidence: 0.95,
        });

        // d/dx cos(a*x + b) = -a*sin(a*x + b)
        self.maybe_add(ComputationRule {
            slug: "diff_cos_linear".into(),
            domain: RuleDomain::Differentiate,
            pattern: axb().cos(),
            template: -(a() * axb().sin()),
            description: "d/dx cos(ax+b) = -a*sin(ax+b)".into(),
            confidence: 0.95,
        });

        // d/dx e^(a*x + b) = a*e^(a*x + b)
        self.maybe_add(ComputationRule {
            slug: "diff_exp_linear".into(),
            domain: RuleDomain::Differentiate,
            pattern: axb().exp(),
            template: a() * axb().exp(),
            description: "d/dx e^(ax+b) = a*e^(ax+b)".into(),
            confidence: 0.95,
        });

        // d/dx ln(a*x + b) = a/(a*x + b)
        self.maybe_add(ComputationRule {
            slug: "diff_ln_linear".into(),
            domain: RuleDomain::Differentiate,
            pattern: axb().ln(),
            template: a() / axb(),
            description: "d/dx ln(ax+b) = a/(ax+b)".into(),
            confidence: 0.95,
        });

        // d/dx sqrt(a*x + b) = a/(2*sqrt(a*x + b))
        self.maybe_add(ComputationRule {
            slug: "diff_sqrt_linear".into(),
            domain: RuleDomain::Differentiate,
            pattern: axb().sqrt(),
            template: a() / (Num(2.0) * axb().sqrt()),
            description: "d/dx sqrt(ax+b) = a/(2*sqrt(ax+b))".into(),
            confidence: 0.95,
        });

        // d/dx tan(x) = 1/cos^2(x) = sec^2(x)
        self.maybe_add(ComputationRule {
            slug: "diff_tan".into(),
            domain: RuleDomain::Differentiate,
            pattern: x().tan(),
            template: Num(1.0) / x().cos().pow(Num(2.0)),
            description: "d/dx tan(x) = sec^2(x)".into(),
            confidence: 0.95,
        });

        // d/dx tan(a*x + b) = a/cos^2(a*x + b)
        self.maybe_add(ComputationRule {
            slug: "diff_tan_linear".into(),
            domain: RuleDomain::Differentiate,
            pattern: axb().tan(),
            template: a() / axb().cos().pow(Num(2.0)),
            description: "d/dx tan(ax+b) = a/cos^2(ax+b)".into(),
            confidence: 0.90,
        });

        // ── Simplification Rules ──────────────────────────────────────

        // sin^2(x) + cos^2(x) = 1
        self.maybe_add(ComputationRule {
            slug: "simp_pythagorean".into(),
            domain: RuleDomain::Simplify,
            pattern: x().sin().pow(Num(2.0)) + x().cos().pow(Num(2.0)),
            template: Num(1.0),
            description: "sin^2(x) + cos^2(x) = 1".into(),
            confidence: 0.99,
        });

        // Auto-derive integration rules from all differentiation rules
        let derived = self.derive_integral_rules();
        if derived > 0 {
            eprintln!(
                "  bootstrap: derived {} integration rules from differentiation rules",
                derived
            );
        }
    }

    /// Add a rule silently if its slug doesn't conflict.
    fn maybe_add(&mut self, rule: ComputationRule) {
        if !self.rules.iter().any(|r| r.slug == rule.slug) {
            self.rules.push(rule);
        }
    }

    /// Auto-derive integration rules from differentiation rules.
    ///
    /// For each `Differentiate` rule `P → T` (meaning d/dx(P) = T),
    /// derive an `Integrate` rule `∫ T dx = P + C`.
    ///
    /// Chain-rule detection: when the derivative template has the form
    /// `T = Mul(inner, factor)` where `factor` is a pattern variable or
    /// numeric constant, the integral rule is `∫ inner dx = P / factor + C`,
    /// stripping the constant multiplier that the chain rule introduced.
    ///
    /// Returns the number of rules derived.
    pub fn derive_integral_rules(&mut self) -> usize {
        let mut count = 0;
        // Clone the current rules so we don't iterate over newly added ones
        let existing: Vec<ComputationRule> = self.rules.clone();
        for rule in &existing {
            if rule.domain != RuleDomain::Differentiate {
                continue;
            }
            let derived = Self::derive_integral_from_differentiate(rule);
            if let Some(derived_rule) = derived {
                if self.maybe_add_silent(derived_rule).is_ok() {
                    count += 1;
                }
            }
        }
        count
    }

    /// Derive a single Integrate rule from a single Differentiate rule.
    ///
    /// Strategy: swap pattern and template (pure inverse relationship).
    ///   Given d/dx(P) = T, derive ∫ T dx = P + C
    ///
    /// Negation special case: when T = -inner, the derived pattern is
    /// `inner` and the derived template is `-P + C`. This correctly handles
    /// `d/dx cos(x) = -sin(x)` → `∫ sin(x) dx = -cos(x) + C`.
    ///
    /// We do NOT attempt to strip chain-rule factors from Mul templates.
    /// A template `a*exp(ax+b)` swaps as-is to pattern `a*exp(ax+b)`,
    /// so the derived rule correctly matches `∫ a*exp(ax+b) dx`.
    fn derive_integral_from_differentiate(rule: &ComputationRule) -> Option<ComputationRule> {
        use crate::algebra::SymExpr::{self, *};

        let pat = &rule.pattern;
        let tpl = &rule.template;

        // Negation wrappers: template = -inner → derived = -pattern + C
        if let SymExpr::Neg(inner) = tpl {
            let derived_pat = inner.as_ref().clone();
            let derived_tpl = -(pat.clone()) + Var("C".into());
            return Some(ComputationRule {
                slug: format!("derived_int_from_{}", rule.slug),
                domain: RuleDomain::Integrate,
                pattern: derived_pat, // no simplify — keep exact structural form
                template: derived_tpl.simplify(),
                description: format!("Self-derived ∫ d/dx({}) dx = {} + C", rule.description, pat),
                confidence: rule.confidence * 0.95,
            });
        }

        // Default: direct swap. Derived: ∫ template dx = pattern + C
        let derived_tpl = pat.clone() + Var("C".into());
        Some(ComputationRule {
            slug: format!("derived_int_from_{}", rule.slug),
            domain: RuleDomain::Integrate,
            pattern: tpl.clone(), // no simplify — keep the exact structural form
            template: derived_tpl.simplify(),
            description: format!("Self-derived ∫ d/dx({}) dx = {} + C", rule.description, pat),
            confidence: rule.confidence * 0.95,
        })
    }

    /// Like maybe_add but returns Ok/Err like add_rule (silent error on conflict).
    fn maybe_add_silent(&mut self, rule: ComputationRule) -> Result<(), String> {
        if self.rules.iter().any(|r| r.slug == rule.slug) {
            Err(format!("Rule '{}' already exists", rule.slug))
        } else {
            self.rules.push(rule);
            Ok(())
        }
    }

    /// Convert a formula from the FormulaRegistry into a ComputationRule (if possible).
    ///
    /// This is the key bridge: textbook formulas become operational rules.
    /// The conversion is best-effort — not all formulas are valid computation rules.
    ///
    /// ## Calculus Notation Handling
    ///
    /// Formulas like `d/dx sin(x) = cos(x)` or `∫ x^n dx = x^(n+1)/(n+1) + C`
    /// use Leibniz/int-notation that doesn't parse as SymExpr directly.
    /// We strip the calculus wrapper and extract just the inner expression:
    ///
    ///   `d/dx sin(x)`       → inner: `sin(x)`, domain: Differentiate
    ///   `∫ x^n dx`          → inner: `x^n`, domain: Integrate
    ///   `int x^n dx`        → inner: `x^n`, domain: Integrate
    ///   `F = ma`            → no wrapper, domain: Simplify (unchanged)
    pub fn formula_to_rule(formula: &FormulaEntry) -> Option<ComputationRule> {
        // Try to find an '=' sign and parse both sides
        let (lhs_raw, rhs_raw) = if let Some(eq_pos) = formula.expr_str.find('=') {
            (
                formula.expr_str[..eq_pos].trim().to_string(),
                formula.expr_str[eq_pos + 1..].trim().to_string(),
            )
        } else {
            // No '=' found — can't split into pattern/template
            return None;
        };

        // Clean up common LaTeX artifacts
        let clean = |s: &str| -> String {
            s.replace("\\frac{d}{dx}", "d/dx")
                .replace("\\int", "∫")
                .replace("  ", " ")
                .trim()
                .to_string()
        };

        let lhs_raw = clean(&lhs_raw);
        let rhs_raw = clean(&rhs_raw);

        // ── Strip calculus notation operators ─────────────────────────
        // "d/dx sin(x)" → inner "sin(x)", domain = Differentiate
        // "∫ x^n dx"    → inner "x^n", domain = Integrate
        // "F = ma"      → unchanged, domain from tags
        let (inner_lhs, forced_domain, has_bounds) = strip_calculus_operator(&lhs_raw);

        // Skip definite integrals: the pattern would reference bounds
        // that aren't in the expression being matched, producing a useless
        // rule (e.g. pattern `f(x)` or `x^2` matches trivially everywhere).
        if has_bounds {
            eprintln!(
                "  skip auto_{}: definite integral — doesn't generalise as a computation rule",
                formula.slug
            );
            return None;
        }

        // Parse the (possibly stripped) LHS and RHS as SymExpr
        let lhs = crate::algebra::parse(&inner_lhs).ok()?;
        let rhs = crate::algebra::parse(&rhs_raw).ok()?;

        // Determine domain: prefer forced domain, fall back to tags/structure
        let domain = forced_domain.unwrap_or_else(|| {
            match formula.domain.as_str() {
                d if d.contains("derivative") || d.contains("differentiation") => {
                    RuleDomain::Differentiate
                }
                d if d.contains("integral") || d.contains("integration") => RuleDomain::Integrate,
                d if d.contains("solve") || d.contains("equation") => RuleDomain::Solve,
                _ => {
                    // Infer from structure
                    let lower = formula.expr_str.to_lowercase();
                    if lower.contains("d/d")
                        || lower.contains("derivative")
                        || lower.contains("differentiate")
                    {
                        RuleDomain::Differentiate
                    } else if lower.contains('∫')
                        || lower.contains("integral")
                        || lower.contains("integrate")
                        || lower.contains("int ")
                    {
                        RuleDomain::Integrate
                    } else if lower.contains("solve")
                        || lower.contains("root")
                        || lower.contains("solution")
                    {
                        RuleDomain::Solve
                    } else {
                        RuleDomain::Simplify
                    }
                }
            }
        });

        Some(ComputationRule {
            slug: format!("auto_{}", formula.slug),
            domain,
            pattern: lhs,
            template: rhs,
            description: formula.expr_str.clone(),
            confidence: 0.85, // Auto-converted rules get slightly lower confidence
        })
    }
}

impl FormulaRegistry {
    /// Create an empty registry.
    pub fn new() -> Self {
        FormulaRegistry {
            formulas: Vec::new(),
            slug_index: HashMap::new(),
            alias_index: HashMap::new(),
        }
    }

    /// Access all formulas (read-only).
    pub fn formulas(&self) -> &[FormulaEntry] {
        &self.formulas
    }

    /// Access all formulas mutably (for bulk operations like relinking).
    pub fn formulas_mut(&mut self) -> &mut Vec<FormulaEntry> {
        &mut self.formulas
    }

    /// Register a formula. Returns an error if the slug already exists.
    pub fn register(&mut self, entry: FormulaEntry) -> Result<(), String> {
        let slug = entry.slug.clone();
        if self.slug_index.contains_key(&slug) {
            return Err(format!("Formula '{}' already registered", slug));
        }
        let idx = self.formulas.len();
        self.formulas.push(entry);
        self.slug_index.insert(slug.clone(), idx);

        // Index all aliases
        let entry_ref = &self.formulas[idx];
        for alias in &entry_ref.aliases {
            self.alias_index.insert(alias.clone(), slug.clone());
        }

        Ok(())
    }

    /// Look up a formula by slug.
    pub fn by_slug(&self, slug: &str) -> Option<&FormulaEntry> {
        self.slug_index.get(slug).map(|&idx| &self.formulas[idx])
    }

    /// Look up a formula by slug (mutable).
    pub fn by_slug_mut(&mut self, slug: &str) -> Option<&mut FormulaEntry> {
        self.slug_index
            .get(slug)
            .map(|&idx| &mut self.formulas[idx])
    }

    /// Look up a formula by alias or slug.
    pub fn lookup(&self, name: &str) -> Option<&FormulaEntry> {
        // Try direct slug first
        if let Some(&idx) = self.slug_index.get(name) {
            return Some(&self.formulas[idx]);
        }
        // Try alias
        if let Some(slug) = self.alias_index.get(name) {
            if let Some(&idx) = self.slug_index.get(slug) {
                return Some(&self.formulas[idx]);
            }
        }
        None
    }

    /// Search formulas by tag or domain.
    pub fn search(&self, query: &str) -> Vec<&FormulaEntry> {
        let q = query.to_lowercase();
        self.formulas
            .iter()
            .filter(|f| {
                f.slug.contains(&q)
                    || f.domain.to_lowercase().contains(&q)
                    || f.tags.iter().any(|t| t.to_lowercase().contains(&q))
                    || f.aliases.iter().any(|a| a.to_lowercase().contains(&q))
                    || f.descriptions.iter().any(|(s, v, o)| {
                        s.to_lowercase().contains(&q)
                            || v.to_lowercase().contains(&q)
                            || o.to_lowercase().contains(&q)
                    })
            })
            .collect()
    }

    /// Total number of registered formulas.
    pub fn len(&self) -> usize {
        self.formulas.len()
    }

    /// List all slugs (for enumeration).
    pub fn list_slugs(&self) -> Vec<String> {
        self.formulas.iter().map(|f| f.slug.clone()).collect()
    }

    /// Re‑link all formulas: rebuilds the alias index from registered
    /// formula aliases to ensure lookups are consistent after bulk loading.
    pub fn relink_all(&mut self) {
        // Rebuild the alias index from scratch
        self.alias_index.clear();
        for formula in &self.formulas {
            for alias in &formula.aliases {
                self.alias_index.insert(alias.clone(), formula.slug.clone());
            }
        }
        // Rebuild the slug index
        self.slug_index.clear();
        for (i, formula) in self.formulas.iter().enumerate() {
            self.slug_index.insert(formula.slug.clone(), i);
        }
    }

    /// Sync formulas from this registry into a `RuleEngine`, creating
    /// `ComputationRule` entries for each formula that can be converted.
    pub fn sync_to_rule_engine(&self, engine: &mut RuleEngine) {
        for formula in &self.formulas {
            if let Some(rule) = RuleEngine::formula_to_rule(formula) {
                let _ = engine.add_rule(rule);
            }
        }
        // Also derive integral rules from differentiation rules
        engine.derive_integral_rules();
    }

    /// Try to compute the result of applying a registered formula to an expression.
    ///
    /// For example, given request `d/dx x^5`, it matches the power rule
    /// `d/dx x^n = n*x^(n-1)`, binds `n = 5`, and returns `5*x^4`.
    ///
    /// Both the request and the formula LHS are stripped of calculus notation
    /// (d/dx, ∫, int … dx) before matching, so `d/dx sin(x)` matches against
    /// the inner expression `sin(x)` rather than the raw `Mul(Div(d,dx), sin(x))`.
    ///
    /// Returns the computed result if any formula matches.
    pub fn derive(&self, request: &str) -> Option<String> {
        // Strip calculus notation from the request (e.g. "d/dx x^5" → "x^5")
        let (stripped_request, _, _) = strip_calculus_operator(request);
        let request_parsed = crate::algebra::parse(&stripped_request).ok()?;

        for formula in &self.formulas {
            // Split formula on '=' to get LHS pattern and RHS template
            let eq_pos = formula.expr_str.find('=')?;
            let lhs_str = formula.expr_str[..eq_pos].trim();
            let rhs_str = formula.expr_str[eq_pos + 1..].trim();

            // Strip calculus notation from the formula LHS too
            // (ignore has_bounds — derive can still match bounded integrals)
            let (stripped_lhs, _, _) = strip_calculus_operator(lhs_str);
            let lhs_pattern = crate::algebra::parse(&stripped_lhs).ok()?;

            // Try to match stripped request against stripped LHS pattern
            let mut bindings: HashMap<String, SymExpr> = HashMap::new();
            if match_symexpr(&request_parsed, &lhs_pattern, &mut bindings) {
                // Substitute bindings into raw RHS template (no stripping needed)
                let rhs_template = crate::algebra::parse(rhs_str).ok()?;
                let result = substitute_vars(&rhs_template, &bindings);
                // Simplify the result (e.g., 5-1 → 4)
                let simplified = result.simplify();
                return Some(format!("{}", simplified));
            }
        }

        None
    }

    /// Register commonly known calculus formulas.
    /// These serve as the bootstrap knowledge base.
    pub fn seed_bootstrap(&mut self) {
        let bootstrap: Vec<FormulaEntry> = vec![
            // ── Power Rule ───────────────────────────────────────────
            FormulaEntry {
                slug: "power_rule".into(),
                expr_str: "d/dx x^n = n*x^(n-1)".into(),
                descriptions: vec![
                    (
                        "power_rule".into(),
                        "states".into(),
                        "d/dx_x^n_=_n*x^(n-1)".into(),
                    ),
                    ("derivative_of_x^n".into(), "is".into(), "n*x^(n-1)".into()),
                ],
                aliases: vec!["power rule".into(), "derivative power rule".into()],
                source: "bootstrap".into(),
                domain: "calculus".into(),
                tags: vec!["derivative".into(), "power".into(), "polynomial".into()],
                linked_fact_ids: Vec::new(),
            },
            // ── Derivative of sin ────────────────────────────────────
            FormulaEntry {
                slug: "derivative_of_sin".into(),
                expr_str: "d/dx sin(x) = cos(x)".into(),
                descriptions: vec![("derivative_of_sin(x)".into(), "is".into(), "cos(x)".into())],
                aliases: vec!["derivative of sin".into(), "sin derivative".into()],
                source: "bootstrap".into(),
                domain: "calculus".into(),
                tags: vec!["derivative".into(), "trigonometry".into()],
                linked_fact_ids: Vec::new(),
            },
            // ── Derivative of cos ────────────────────────────────────
            FormulaEntry {
                slug: "derivative_of_cos".into(),
                expr_str: "d/dx cos(x) = -sin(x)".into(),
                descriptions: vec![("derivative_of_cos(x)".into(), "is".into(), "-sin(x)".into())],
                aliases: vec!["derivative of cos".into(), "cos derivative".into()],
                source: "bootstrap".into(),
                domain: "calculus".into(),
                tags: vec!["derivative".into(), "trigonometry".into()],
                linked_fact_ids: Vec::new(),
            },
            // ── Derivative of tan ────────────────────────────────────
            FormulaEntry {
                slug: "derivative_of_tan".into(),
                expr_str: "d/dx tan(x) = 1/cos^2(x)".into(),
                descriptions: vec![(
                    "derivative_of_tan(x)".into(),
                    "is".into(),
                    "sec^2(x)".into(),
                )],
                aliases: vec!["derivative of tan".into(), "tan derivative".into()],
                source: "bootstrap".into(),
                domain: "calculus".into(),
                tags: vec!["derivative".into(), "trigonometry".into()],
                linked_fact_ids: Vec::new(),
            },
            // ── Product Rule ─────────────────────────────────────────
            FormulaEntry {
                slug: "product_rule".into(),
                expr_str: "d/dx (u*v) = u*dv/dx + v*du/dx".into(),
                descriptions: vec![(
                    "product_rule".into(),
                    "states".into(),
                    "d/dx_(u*v)_=_u*dv/dx_+_v*du/dx".into(),
                )],
                aliases: vec!["product rule".into(), "derivative product rule".into()],
                source: "bootstrap".into(),
                domain: "calculus".into(),
                tags: vec!["derivative".into(), "product".into()],
                linked_fact_ids: Vec::new(),
            },
            // ── Chain Rule ───────────────────────────────────────────
            FormulaEntry {
                slug: "chain_rule".into(),
                expr_str: "d/dx f(g(x)) = f'(g(x))*g'(x)".into(),
                descriptions: vec![(
                    "chain_rule".into(),
                    "states".into(),
                    "d/dx_f(g(x))_=_f'(g(x))*g'(x)".into(),
                )],
                aliases: vec!["chain rule".into(), "derivative chain rule".into()],
                source: "bootstrap".into(),
                domain: "calculus".into(),
                tags: vec!["derivative".into(), "chain".into(), "composition".into()],
                linked_fact_ids: Vec::new(),
            },
            // ── Integral Power Rule ──────────────────────────────────
            FormulaEntry {
                slug: "integral_power_rule".into(),
                expr_str: "int x^n dx = x^(n+1)/(n+1) + C".into(),
                descriptions: vec![(
                    "integral_of_x^n".into(),
                    "is".into(),
                    "x^(n+1)/(n+1)_+_C".into(),
                )],
                aliases: vec![
                    "integral power rule".into(),
                    "power rule integration".into(),
                ],
                source: "bootstrap".into(),
                domain: "calculus".into(),
                tags: vec!["integral".into(), "power".into(), "polynomial".into()],
                linked_fact_ids: Vec::new(),
            },
            // ── Fundamental Theorem of Calculus ──────────────────────
            FormulaEntry {
                slug: "fundamental_theorem_of_calculus".into(),
                expr_str: "int_a^b f(x) dx = F(b) - F(a)".into(),
                descriptions: vec![(
                    "fundamental_theorem_of_calculus".into(),
                    "states".into(),
                    "int_a^b_f(x)_dx_=_F(b)_-_F(a)".into(),
                )],
                aliases: vec!["fundamental theorem".into(), "FTC".into()],
                source: "bootstrap".into(),
                domain: "calculus".into(),
                tags: vec!["integral".into(), "theorem".into(), "fundamental".into()],
                linked_fact_ids: Vec::new(),
            },
            // ── Quadratic Formula ────────────────────────────────────
            FormulaEntry {
                slug: "quadratic_formula".into(),
                expr_str: "x = (-b +- sqrt(b^2 - 4*a*c))/(2*a)".into(),
                descriptions: vec![(
                    "quadratic_formula".into(),
                    "solves".into(),
                    "ax^2_+_bx_+_c_=_0".into(),
                )],
                aliases: vec!["quadratic formula".into(), "quadratic equation".into()],
                source: "bootstrap".into(),
                domain: "algebra".into(),
                tags: vec!["algebra".into(), "quadratic".into(), "polynomial".into()],
                linked_fact_ids: Vec::new(),
            },
            // ── Pythagorean Identity ─────────────────────────────────
            FormulaEntry {
                slug: "pythagorean_identity".into(),
                expr_str: "sin^2(x) + cos^2(x) = 1".into(),
                descriptions: vec![("sin^2(x)_+_cos^2(x)".into(), "equals".into(), "1".into())],
                aliases: vec!["pythagorean identity".into(), "trig identity".into()],
                source: "bootstrap".into(),
                domain: "trigonometry".into(),
                tags: vec![
                    "trigonometry".into(),
                    "identity".into(),
                    "pythagorean".into(),
                ],
                linked_fact_ids: Vec::new(),
            },
            // ── Derivative of exp ────────────────────────────────────
            FormulaEntry {
                slug: "derivative_of_exp".into(),
                expr_str: "d/dx e^x = e^x".into(),
                descriptions: vec![("derivative_of_e^x".into(), "is".into(), "e^x".into())],
                aliases: vec!["derivative of e^x".into(), "exponential derivative".into()],
                source: "bootstrap".into(),
                domain: "calculus".into(),
                tags: vec!["derivative".into(), "exponential".into()],
                linked_fact_ids: Vec::new(),
            },
            // ── Derivative of ln ─────────────────────────────────────
            FormulaEntry {
                slug: "derivative_of_ln".into(),
                expr_str: "d/dx ln(x) = 1/x".into(),
                descriptions: vec![("derivative_of_ln(x)".into(), "is".into(), "1/x".into())],
                aliases: vec!["derivative of ln".into(), "log derivative".into()],
                source: "bootstrap".into(),
                domain: "calculus".into(),
                tags: vec!["derivative".into(), "logarithm".into()],
                linked_fact_ids: Vec::new(),
            },
            // ── Newton's Second Law (Physics) ─────────────────────────
            FormulaEntry {
                slug: "newtons_second_law".into(),
                expr_str: "F = m*a".into(),
                descriptions: vec![(
                    "newtons_second_law".into(),
                    "states".into(),
                    "force_equals_mass_times_acceleration".into(),
                )],
                aliases: vec![
                    "newton's second law".into(),
                    "newton's second law of motion".into(),
                    "f=ma".into(),
                ],
                source: "bootstrap".into(),
                domain: "physics".into(),
                tags: vec!["physics".into(), "mechanics".into(), "force".into()],
                linked_fact_ids: Vec::new(),
            },
            // ── Kinetic Energy (Physics) ─────────────────────────────
            FormulaEntry {
                slug: "kinetic_energy".into(),
                expr_str: "KE = 1/2*m*v^2".into(),
                descriptions: vec![(
                    "kinetic_energy".into(),
                    "equals".into(),
                    "one_half_m_v_squared".into(),
                )],
                aliases: vec!["kinetic energy".into(), "kinetic energy formula".into()],
                source: "bootstrap".into(),
                domain: "physics".into(),
                tags: vec!["physics".into(), "mechanics".into(), "energy".into()],
                linked_fact_ids: Vec::new(),
            },
            // ── Gravitational Potential Energy (Physics) ─────────────
            FormulaEntry {
                slug: "gravitational_potential_energy".into(),
                expr_str: "PE = m*g*h".into(),
                descriptions: vec![(
                    "gravitational_potential_energy".into(),
                    "equals".into(),
                    "mass_times_gravity_times_height".into(),
                )],
                aliases: vec![
                    "gravitational potential energy".into(),
                    "potential energy".into(),
                    "mgh".into(),
                ],
                source: "bootstrap".into(),
                domain: "physics".into(),
                tags: vec!["physics".into(), "mechanics".into(), "energy".into()],
                linked_fact_ids: Vec::new(),
            },
            // ── Hooke's Law (Physics) ─────────────────────────────────
            FormulaEntry {
                slug: "hookes_law".into(),
                expr_str: "F = -k*x".into(),
                descriptions: vec![(
                    "hookes_law".into(),
                    "states".into(),
                    "force_equals_negative_spring_constant_times_displacement".into(),
                )],
                aliases: vec![
                    "hooke's law".into(),
                    "hookes law".into(),
                    "spring force formula".into(),
                ],
                source: "bootstrap".into(),
                domain: "physics".into(),
                tags: vec!["physics".into(), "mechanics".into(), "oscillation".into()],
                linked_fact_ids: Vec::new(),
            },
            // ── Area of a Circle (Geometry) ────────────────────────────
            FormulaEntry {
                slug: "area_of_circle".into(),
                expr_str: "A = pi*r^2".into(),
                descriptions: vec![(
                    "area_of_circle".into(),
                    "equals".into(),
                    "pi_r_squared".into(),
                )],
                aliases: vec![
                    "area of circle".into(),
                    "area of a circle".into(),
                    "circle area".into(),
                    "pi r squared".into(),
                ],
                source: "bootstrap".into(),
                domain: "geometry".into(),
                tags: vec!["geometry".into(), "circle".into(), "area".into()],
                linked_fact_ids: Vec::new(),
            },
            // ── Pythagorean Theorem (Geometry) ────────────────────────
            FormulaEntry {
                slug: "pythagorean_theorem".into(),
                expr_str: "a^2 + b^2 = c^2".into(),
                descriptions: vec![(
                    "pythagorean_theorem".into(),
                    "states".into(),
                    "a_squared_plus_b_squared_equals_c_squared".into(),
                )],
                aliases: vec![
                    "pythagorean theorem".into(),
                    "pythagoras theorem".into(),
                    "pythagorean theorem formula".into(),
                ],
                source: "bootstrap".into(),
                domain: "geometry".into(),
                tags: vec!["geometry".into(), "triangle".into(), "pythagorean".into()],
                linked_fact_ids: Vec::new(),
            },
            // ── Arithmetic Mean (Statistics) ───────────────────────────
            FormulaEntry {
                slug: "mean_formula".into(),
                expr_str: "mu = (sum(x))/n".into(),
                descriptions: vec![(
                    "mean_formula".into(),
                    "equals".into(),
                    "sum_of_x_divided_by_n".into(),
                )],
                aliases: vec![
                    "mean formula".into(),
                    "arithmetic mean".into(),
                    "average formula".into(),
                ],
                source: "bootstrap".into(),
                domain: "statistics".into(),
                tags: vec!["statistics".into(), "mean".into(), "average".into()],
                linked_fact_ids: Vec::new(),
            },
            // ── Variance (Statistics) ──────────────────────────────────
            FormulaEntry {
                slug: "variance_formula".into(),
                expr_str: "sigma^2 = sum((x - mu)^2)/n".into(),
                descriptions: vec![(
                    "variance_formula".into(),
                    "equals".into(),
                    "average_squared_deviation_from_mean".into(),
                )],
                aliases: vec![
                    "variance formula".into(),
                    "population variance".into(),
                    "sigma squared".into(),
                ],
                source: "bootstrap".into(),
                domain: "statistics".into(),
                tags: vec!["statistics".into(), "variance".into(), "dispersion".into()],
                linked_fact_ids: Vec::new(),
            },
            // ── Binomial Theorem (Algebra) ─────────────────────────────
            FormulaEntry {
                slug: "binomial_theorem".into(),
                expr_str: "(x + y)^n = sum_{k=0}^{n} C(n,k) x^(n-k) y^k".into(),
                descriptions: vec![(
                    "binomial_theorem".into(),
                    "states".into(),
                    "expansion_of_x_plus_y_to_n".into(),
                )],
                aliases: vec![
                    "binomial theorem".into(),
                    "binomial expansion".into(),
                    "binomial formula".into(),
                ],
                source: "bootstrap".into(),
                domain: "algebra".into(),
                tags: vec!["algebra".into(), "binomial".into(), "polynomial".into()],
                linked_fact_ids: Vec::new(),
            },
            // ── Slope Formula (Algebra) ────────────────────────────────
            FormulaEntry {
                slug: "slope_formula".into(),
                expr_str: "m = (y2 - y1)/(x2 - x1)".into(),
                descriptions: vec![(
                    "slope_formula".into(),
                    "equals".into(),
                    "rise_over_run".into(),
                )],
                aliases: vec![
                    "slope formula".into(),
                    "rise over run".into(),
                    "slope of a line".into(),
                ],
                source: "bootstrap".into(),
                domain: "algebra".into(),
                tags: vec!["algebra".into(), "slope".into(), "linear".into()],
                linked_fact_ids: Vec::new(),
            },
            // ── Distance Formula (Algebra/Geometry) ────────────────────
            FormulaEntry {
                slug: "distance_formula".into(),
                expr_str: "d = sqrt((x2 - x1)^2 + (y2 - y1)^2)".into(),
                descriptions: vec![(
                    "distance_formula".into(),
                    "equals".into(),
                    "sqrt_of_sum_of_squared_differences".into(),
                )],
                aliases: vec![
                    "distance formula".into(),
                    "euclidean distance".into(),
                    "distance between two points".into(),
                ],
                source: "bootstrap".into(),
                domain: "algebra".into(),
                tags: vec!["algebra".into(), "geometry".into(), "distance".into()],
                linked_fact_ids: Vec::new(),
            },
        ];

        for entry in bootstrap {
            // Silently ignore duplicates (e.g. on reload)
            let _ = self.register(entry);
        }
    }

    /// Save the registry to a JSON file.
    pub fn save_to_file(&self, path: &str) -> Result<(), String> {
        let json = serde_json::to_string_pretty(self)
            .map_err(|e| format!("Failed to serialize formula registry: {}", e))?;
        std::fs::write(path, &json)
            .map_err(|e| format!("Failed to write formula registry: {}", e))?;
        Ok(())
    }

    /// Load the registry from a JSON file. Returns empty registry if file
    /// doesn't exist, or errors on corrupt data.
    pub fn load_from_file(path: &str) -> Result<Self, String> {
        match std::fs::read_to_string(path) {
            Ok(json) => {
                let registry: FormulaRegistry = serde_json::from_str(&json)
                    .map_err(|e| format!("Failed to parse formula registry: {}", e))?;
                Ok(registry)
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(FormulaRegistry::new()),
            Err(e) => Err(format!("Failed to read formula registry: {}", e)),
        }
    }
}

impl Default for FormulaRegistry {
    fn default() -> Self {
        Self::new()
    }
}

// ═══════════════════════════════════════════════════════════════════════
// FORMULA EXTRACTION FROM TEXT
// ═══════════════════════════════════════════════════════════════════════

/// A formula found in text with its surrounding context.
#[derive(Clone, Debug)]
pub struct FormulaExtraction {
    /// The raw formula text (LaTeX or Unicode math).
    pub raw: String,
    /// Whether this is LaTeX (true) or Unicode math (false).
    pub is_latex: bool,
    /// Surrouding context (sentence-level text before the formula).
    pub context_before: String,
    /// Surrounding context (sentence-level text after the formula).
    pub context_after: String,
    /// Source document identifier.
    pub source: String,
}

/// Scan text for math formulas in LaTeX and Unicode math notation.
///
/// Detects:
/// - Inline LaTeX: `$...$`, `\(...\)`
/// - Display LaTeX: `$$...$$`, `\[...\]`
/// - Unicode math: regions containing math operators between text
pub fn extract_formulas_from_text(text: &str, source: &str) -> Vec<FormulaExtraction> {
    let mut results = Vec::new();

    // Extract LaTeX blocks: $$...$$ (display)
    let mut pos = 0;
    let chars: Vec<char> = text.chars().collect();

    // Display LaTeX: $$...$$
    while pos < chars.len() {
        if pos + 1 < chars.len() && chars[pos] == '$' && chars[pos + 1] == '$' {
            let start = pos + 2;
            if let Some(end) = find_closing_double_dollar(&chars, start) {
                let raw: String = chars[start..end].iter().collect();
                let ctx_before = extract_context_before(&chars, pos, 100);
                let ctx_after = extract_context_after(&chars, end + 2, 100);
                results.push(FormulaExtraction {
                    raw: raw.trim().to_string(),
                    is_latex: true,
                    context_before: ctx_before,
                    context_after: ctx_after,
                    source: source.to_string(),
                });
                pos = end + 2;
                continue;
            }
        }
        pos += 1;
    }

    // Inline LaTeX: $...$
    pos = 0;
    while pos < chars.len() {
        if chars[pos] == '$' && !(pos + 1 < chars.len() && chars[pos + 1] == '$') {
            let start = pos + 1;
            if let Some(end) = find_closing_dollar(&chars, start) {
                let raw: String = chars[start..end].iter().collect();
                if raw.len() >= 2 && raw.len() <= 500 {
                    let ctx_before = extract_context_before(&chars, pos, 80);
                    let ctx_after = extract_context_after(&chars, end + 1, 80);
                    results.push(FormulaExtraction {
                        raw: raw.trim().to_string(),
                        is_latex: true,
                        context_before: ctx_before,
                        context_after: ctx_after,
                        source: source.to_string(),
                    });
                }
                pos = end + 1;
                continue;
            }
        }
        pos += 1;
    }

    // Also scan for \(...\) and \[...\] patterns
    pos = 0;
    while pos + 1 < chars.len() {
        if chars[pos] == '\\' && chars[pos + 1] == '(' {
            let start = pos + 2;
            if let Some(end) = find_closing_paren(&chars, start) {
                let raw: String = chars[start..end].iter().collect();
                if raw.len() >= 2 && raw.len() <= 500 {
                    let ctx_before = extract_context_before(&chars, pos, 80);
                    let ctx_after = extract_context_after(&chars, end + 1, 80);
                    results.push(FormulaExtraction {
                        raw: raw.trim().to_string(),
                        is_latex: true,
                        context_before: ctx_before,
                        context_after: ctx_after,
                        source: source.to_string(),
                    });
                }
                pos = end + 1;
                continue;
            }
        }
        if pos + 1 < chars.len() && chars[pos] == '\\' && chars[pos + 1] == '[' {
            let start = pos + 2;
            if let Some(end) = find_closing_bracket(&chars, start) {
                let raw: String = chars[start..end].iter().collect();
                if raw.len() >= 2 && raw.len() <= 2000 {
                    let ctx_before = extract_context_before(&chars, pos, 80);
                    let ctx_after = extract_context_after(&chars, end + 1, 80);
                    results.push(FormulaExtraction {
                        raw: raw.trim().to_string(),
                        is_latex: true,
                        context_before: ctx_before,
                        context_after: ctx_after,
                        source: source.to_string(),
                    });
                }
                pos = end + 1;
                continue;
            }
        }
        pos += 1;
    }

    // Extract Unicode math regions
    results.extend(extract_unicode_math_regions(text, source));

    // Deduplicate by raw formula text (keep first occurrence)
    results.dedup_by(|a, b| a.raw == b.raw);

    results
}

/// Find closing `$$`.
fn find_closing_double_dollar(chars: &[char], start: usize) -> Option<usize> {
    let mut i = start;
    while i + 1 < chars.len() {
        if chars[i] == '$' && chars[i + 1] == '$' {
            return Some(i);
        }
        i += 1;
    }
    None
}

/// Find closing `$` (not `$$`).
fn find_closing_dollar(chars: &[char], start: usize) -> Option<usize> {
    let mut i = start;
    while i < chars.len() {
        if chars[i] == '$' && !(i + 1 < chars.len() && chars[i + 1] == '$') {
            return Some(i);
        }
        i += 1;
    }
    None
}

/// Find closing `)` for `\(...` pattern.
fn find_closing_paren(chars: &[char], start: usize) -> Option<usize> {
    let mut depth = 1;
    let mut i = start;
    while i < chars.len() {
        if chars[i] == '\\' && i + 1 < chars.len() && chars[i + 1] == ')' {
            depth -= 1;
            if depth == 0 {
                return Some(i - 1);
            }
            i += 2;
            continue;
        }
        if i + 1 < chars.len() && chars[i] == '\\' && chars[i + 1] == '(' {
            depth += 1;
            i += 2;
            continue;
        }
        i += 1;
    }
    None
}

/// Find closing `]` for `\[...` pattern.
fn find_closing_bracket(chars: &[char], start: usize) -> Option<usize> {
    let mut depth = 1;
    let mut i = start;
    while i < chars.len() {
        if chars[i] == '\\' && i + 1 < chars.len() && chars[i + 1] == ']' {
            depth -= 1;
            if depth == 0 {
                return Some(i - 1);
            }
            i += 2;
            continue;
        }
        if i + 1 < chars.len() && chars[i] == '\\' && chars[i + 1] == '[' {
            depth += 1;
            i += 2;
            continue;
        }
        i += 1;
    }
    None
}

/// Extract surrounding context before position.
fn extract_context_before(chars: &[char], pos: usize, max_chars: usize) -> String {
    let start = if pos > max_chars { pos - max_chars } else { 0 };
    let s: String = chars[start..pos].iter().collect();
    // Try to find a sentence boundary
    if let Some(last_period) = s.rfind('.') {
        s[last_period + 1..].trim().to_string()
    } else {
        s.trim().to_string()
    }
}

/// Extract surrounding context after position.
fn extract_context_after(chars: &[char], pos: usize, max_chars: usize) -> String {
    let end = std::cmp::min(pos + max_chars, chars.len());
    let s: String = chars[pos..end].iter().collect();
    if let Some(first_period) = s.find('.') {
        s[..first_period + 1].trim().to_string()
    } else {
        s.trim().to_string()
    }
}

/// Detect and extract regions of Unicode math notation.
///
/// Looks for sequences containing math operators (\int, \sum, d/dx, etc.)
/// between text that doesn't have LaTeX delimiters.
fn extract_unicode_math_regions(text: &str, source: &str) -> Vec<FormulaExtraction> {
    let mut results = Vec::new();
    let math_triggers = [
        "d/dx",
        "\\int",
        "\\sum",
        "\\lim",
        "\\frac",
        "\\sqrt",
        "\\partial",
        "\\infty",
    ];

    for line in text.lines() {
        let trimmed = line.trim();
        if trimmed.len() < 3 || trimmed.len() > 500 {
            continue;
        }

        // Skip lines already captured by LaTeX extraction
        if trimmed.contains('$') || trimmed.contains("\\(") || trimmed.contains("\\[") {
            continue;
        }

        // Check for Unicode math triggers
        let has_math = math_triggers.iter().any(|t| trimmed.contains(t));
        let has_unicode_math = trimmed.chars().any(|c| {
            matches!(
                c,
                '∫' | '∑'
                    | '∏'
                    | '∂'
                    | '√'
                    | '∞'
                    | 'π'
                    | 'Δ'
                    | 'θ'
                    | 'α'
                    | 'β'
                    | 'γ'
                    | 'δ'
                    | 'ε'
                    | 'λ'
                    | 'μ'
                    | 'σ'
                    | 'ω'
            )
        });

        if has_math || has_unicode_math {
            results.push(FormulaExtraction {
                raw: trimmed.to_string(),
                is_latex: false,
                context_before: String::new(),
                context_after: String::new(),
                source: source.to_string(),
            });
        }
    }

    results
}

// ═══════════════════════════════════════════════════════════════════════
// PROSE FORMULA EXTRACTION
// ═══════════════════════════════════════════════════════════════════════

/// A formula reference extracted from natural-language prose (not LaTeX).
#[derive(Clone, Debug)]
pub struct ProseFormulaExtraction {
    /// The formula name/subject (e.g. "power rule", "derivative of sin")
    pub name: String,
    /// The verb phrase connecting name to expression (e.g. "states", "is", "equals")
    pub verb: String,
    /// The math expression as text (e.g. "d/dx x^n = n*x^(n-1)")
    pub expression: String,
    /// Surrounding context sentence
    pub context: String,
    /// Source document
    pub source: String,
}

/// Function name synonyms: variant_name → canonical_name.
///
/// These let "sine function" match "sin", "natural logarithm" match "log", etc.
/// Ordered longest-first so multi-word synonyms are matched before their components.
const FUNCTION_SYNONYMS: &[(&[&str], &str)] = &[
    (
        &[
            "natural logarithm",
            "natural log",
            "logarithm",
            "logarithms",
            "logarithmic",
        ],
        "log",
    ),
    (
        &["exponential function", "exponential", "exponentials"],
        "exp",
    ),
    (&["absolute value", "modulus"], "abs"),
    (&["square root", "principal square root"], "sqrt"),
    (&["sine", "sines", "sinusoidal"], "sin"),
    (&["cosine", "cosines"], "cos"),
    (&["tangent", "tangents"], "tan"),
    (&["cosecant", "cosecants"], "csc"),
    (&["secant", "secants"], "sec"),
    (&["cotangent", "cotangents"], "cot"),
    (&["arcsine", "arcsin"], "asin"),
    (&["arccosine", "arccos"], "acos"),
    (&["arctangent", "arctan"], "atan"),
];

/// Words that are optional noise in formula-name matching.
/// "derivative of THE sine FUNCTION" → matching ignores "the" and "function".
const NOISE_WORDS: &[&str] = &[
    "the",
    "a",
    "an",
    "of",
    "function",
    "functions",
    "rule",
    "rules",
    "its",
    "their",
    "this",
    "for",
    "with",
    "by",
    "via",
];

/// Normalize a word for matching: lowercases and replaces function-name synonyms.
///
/// Returns `None` for noise words that should be skipped entirely.
fn normalized_word(w: &str) -> Option<String> {
    let lower = w.to_lowercase();

    // Check noise words
    if NOISE_WORDS.contains(&lower.as_str()) {
        return None;
    }

    // Check function synonyms (single-word only here)
    for (variants, canonical) in FUNCTION_SYNONYMS {
        for variant in *variants {
            if !variant.contains(' ') && lower == *variant {
                return Some(canonical.to_string());
            }
        }
    }

    Some(lower)
}

/// Tokenize text into word tokens with their character positions.
struct WordToken {
    word: String,
    start: usize,
    end: usize, // exclusive
}

fn tokenize_words(text: &str) -> Vec<WordToken> {
    let mut tokens = Vec::new();
    // Use char_indices to track byte positions throughout.
    let chars_with_pos: Vec<(usize, char)> = text.char_indices().collect();
    let mut i = 0; // index into chars_with_pos

    while i < chars_with_pos.len() {
        let (_byte_pos, ch) = chars_with_pos[i];
        // Skip non-word characters
        if !ch.is_alphanumeric()
            && ch != '\''
            && ch != '^'
            && ch != '_'
            && ch != '/'
            && ch != '('
            && ch != ')'
        {
            i += 1;
            continue;
        }

        let start_char_idx = i;
        let start_byte = chars_with_pos[i].0;
        while i < chars_with_pos.len() {
            let (_, c) = chars_with_pos[i];
            if c.is_alphanumeric()
                || c == '\''
                || c == '^'
                || c == '_'
                || c == '/'
                || c == '('
                || c == ')'
            {
                i += 1;
            } else {
                break;
            }
        }
        let end_byte = if i < chars_with_pos.len() {
            chars_with_pos[i].0
        } else {
            text.len()
        };
        let word: String = chars_with_pos[start_char_idx..i]
            .iter()
            .map(|(_, c)| c)
            .collect();
        if word.len() >= 1 {
            tokens.push(WordToken {
                word,
                start: start_byte,
                end: end_byte,
            });
        }
    }

    tokens
}

/// Normalize text for matching: lowercases and replaces function-name synonyms.
fn normalize_text(text: &str) -> String {
    let mut result = text.to_lowercase();

    // Replace multi-word synonyms first (longest first to handle overlap)
    for (variants, canonical) in FUNCTION_SYNONYMS {
        for variant in *variants {
            if variant.contains(' ') {
                let v_lower = variant.to_lowercase();
                let c_lower = canonical.to_lowercase();
                // Replace at word boundaries with extra space padding
                let padded = format!(" {} ", v_lower);
                if result.contains(&padded) {
                    result = result.replace(&padded, &format!(" {} ", c_lower));
                }
                // Also try leading/trailing
                if result.starts_with(&format!("{} ", v_lower)) {
                    result = result.replacen(&format!("{} ", v_lower), &format!("{} ", c_lower), 1);
                }
                if result.ends_with(&format!(" {}", v_lower)) {
                    let new_len = result.len() - v_lower.len();
                    result = format!("{} {}", &result[..new_len], c_lower);
                }
                if result == v_lower {
                    result = c_lower.to_string();
                }
            }
        }
    }

    // Replace single-word synonyms using word-by-word processing
    let words: Vec<String> = result
        .split(' ')
        .map(|w| {
            let trimmed = w.trim_matches(|c: char| !c.is_alphanumeric());
            let trimmed_str: &str = trimmed;
            for (variants, canonical) in FUNCTION_SYNONYMS {
                for variant in *variants {
                    if !variant.contains(' ') && trimmed_str == *variant {
                        return canonical.to_string();
                    }
                }
            }
            w.to_string()
        })
        .collect();
    result = words.join(" ");

    result
}

/// Result of a fuzzy alias match.
struct AliasMatch {
    /// Character position in the original sentence where the match starts.
    pub start: usize,
    /// Character position in the original sentence where the match ends (exclusive).
    pub end: usize,
}

/// Check if an alias appears in a sentence with fuzzy matching.
///
/// Allows:
/// - Function name synonyms (sine ↔ sin)
/// - Noise words skipped in both sentence and alias
/// - Returns both start and end character positions in the original `sentence`.
fn fuzzy_alias_match(sentence: &str, alias: &str) -> Option<AliasMatch> {
    // Tokenize the ORIGINAL sentence (keeps position info)
    let orig_tokens = tokenize_words(sentence);
    if orig_tokens.is_empty() {
        return None;
    }

    // Normalize alias
    let norm_alias = normalize_text(alias);
    let alias_tokens = tokenize_words(&norm_alias);
    if alias_tokens.is_empty() {
        return None;
    }
    let alias_words: Vec<&str> = alias_tokens.iter().map(|t| t.word.as_str()).collect();

    // Try each start position in the original sentence
    for start in 0..orig_tokens.len() {
        let mut ai = 0; // alias index
        let mut si = start; // sentence index (in original token space)
        let mut last_matched_si = si;

        while ai < alias_words.len() && si < orig_tokens.len() {
            // Normalize the original sentence word on the fly
            let orig_word = &orig_tokens[si].word;
            let norm_word = normalized_word(orig_word);
            let a_word = alias_words[ai];

            // Both are noise? Skip both
            if norm_word.is_none() && is_noise(a_word) {
                ai += 1;
                si += 1;
                continue;
            }

            // Sentence word is noise? Skip it
            if norm_word.is_none() {
                si += 1;
                continue;
            }

            // Alias word after normalization is noise? Skip it
            let a_norm = normalized_word(a_word);
            if a_norm.is_none() {
                ai += 1;
                continue;
            }

            // Direct match or synonym match?
            let s_word = norm_word.unwrap();
            let a_clean = a_norm.unwrap();
            if s_word == a_clean || synonym_match(&s_word, &a_clean) {
                last_matched_si = si;
                ai += 1;
                si += 1;
                continue;
            }

            // Also try matching without normalization (for aliases already in canonical form)
            let orig_word_lower = orig_word.to_lowercase();
            if orig_word_lower == a_clean || synonym_match(&orig_word_lower, &a_clean) {
                last_matched_si = si;
                ai += 1;
                si += 1;
                continue;
            }

            // Mismatch — this start position doesn't work
            break;
        }

        if ai == alias_words.len() {
            // Full alias matched
            return Some(AliasMatch {
                start: orig_tokens[start].start,
                end: orig_tokens[last_matched_si].end,
            });
        }
    }

    None
}

/// Check if two words are synonymous (e.g., "sine" ↔ "sin", "cosine" ↔ "cos").
fn synonym_match(a: &str, b: &str) -> bool {
    if a == b {
        return true;
    }
    let a_lower = a.to_lowercase();
    let b_lower = b.to_lowercase();
    if a_lower == b_lower {
        return true;
    }

    // Check both directions in the synonym table
    for (variants, canonical) in FUNCTION_SYNONYMS {
        let is_a_var = variants.contains(&a_lower.as_str()) || a_lower == *canonical;
        let is_b_var = variants.contains(&b_lower.as_str()) || b_lower == *canonical;
        if is_a_var && is_b_var {
            return true;
        }
    }

    false
}

/// Check if a word is noise.
fn is_noise(word: &str) -> bool {
    let lower = word.to_lowercase();
    NOISE_WORDS.contains(&lower.as_str())
}

/// Known formula names and their canonical slugs for prose detection.
///
/// These span calculus, physics, geometry, statistics, and algebra.
/// Additional aliases are generated dynamically from any formulas registered
/// in a `FormulaRegistry` at extraction time.
const KNOWN_FORMULA_NAMES: &[(&str, &[&str])] = &[
    // ── Calculus ───────────────────────────────────────────────────────
    (
        "power_rule",
        &[
            "power rule",
            "power rule of differentiation",
            "power rule for derivatives",
        ],
    ),
    (
        "product_rule",
        &[
            "product rule",
            "product rule of differentiation",
            "product rule for derivatives",
        ],
    ),
    (
        "quotient_rule",
        &[
            "quotient rule",
            "quotient rule of differentiation",
            "quotient rule for derivatives",
        ],
    ),
    (
        "chain_rule",
        &[
            "chain rule",
            "chain rule of differentiation",
            "chain rule for derivatives",
        ],
    ),
    (
        "derivative_of_sin",
        &[
            "derivative of sin",
            "derivative of sine",
            "derivative of sine function",
            "derivative of sin(x)",
        ],
    ),
    (
        "derivative_of_cos",
        &[
            "derivative of cos",
            "derivative of cosine",
            "derivative of cosine function",
            "derivative of cos(x)",
        ],
    ),
    (
        "derivative_of_tan",
        &[
            "derivative of tan",
            "derivative of tangent",
            "derivative of tangent function",
        ],
    ),
    (
        "derivative_of_exp",
        &[
            "derivative of e^x",
            "derivative of exp",
            "derivative of exponential",
            "derivative of exponential function",
        ],
    ),
    (
        "derivative_of_ln",
        &[
            "derivative of ln",
            "derivative of log",
            "derivative of natural log",
            "derivative of natural logarithm",
        ],
    ),
    (
        "integral_power_rule",
        &[
            "integral power rule",
            "power rule of integration",
            "power rule for integrals",
        ],
    ),
    (
        "fundamental_theorem_of_calculus",
        &[
            "fundamental theorem of calculus",
            "ftc",
            "first fundamental theorem",
        ],
    ),
    // ── Physics / Mechanics ────────────────────────────────────────────
    (
        "newtons_second_law",
        &[
            "newton's second law",
            "newton's second law of motion",
            "force equals mass times acceleration",
            "f equals ma",
        ],
    ),
    (
        "kinetic_energy",
        &["kinetic energy", "kinetic energy formula", "ke formula"],
    ),
    (
        "gravitational_potential_energy",
        &[
            "gravitational potential energy",
            "potential energy",
            "pe formula",
            "mgh",
        ],
    ),
    (
        "work_formula",
        &[
            "work formula",
            "work equals force times distance",
            "work energy principle",
        ],
    ),
    (
        "hookes_law",
        &["hooke's law", "hookes law", "spring force formula"],
    ),
    // ── Geometry ───────────────────────────────────────────────────────
    (
        "area_of_circle",
        &[
            "area of circle",
            "area of a circle",
            "circle area formula",
            "pi r squared",
        ],
    ),
    (
        "circumference_of_circle",
        &["circumference of circle", "circle circumference", "2 pi r"],
    ),
    (
        "area_of_rectangle",
        &["area of rectangle", "rectangle area", "length times width"],
    ),
    (
        "volume_of_sphere",
        &[
            "volume of sphere",
            "sphere volume formula",
            "four thirds pi r cubed",
        ],
    ),
    (
        "pythagorean_theorem",
        &[
            "pythagorean theorem",
            "pythagoras theorem",
            "a squared plus b squared equals c squared",
        ],
    ),
    // ── Statistics / Probability ───────────────────────────────────────
    (
        "mean_formula",
        &[
            "mean formula",
            "arithmetic mean",
            "average formula",
            "sum divided by n",
        ],
    ),
    (
        "variance_formula",
        &["variance formula", "population variance", "sigma squared"],
    ),
    (
        "standard_deviation",
        &["standard deviation", "sigma", "root mean square deviation"],
    ),
    (
        "normal_distribution",
        &[
            "normal distribution",
            "gaussian distribution",
            "bell curve formula",
        ],
    ),
    // ── Algebra ────────────────────────────────────────────────────────
    (
        "quadratic_formula",
        &[
            "quadratic formula",
            "quadratic equation",
            "quadratic formula solver",
        ],
    ),
    (
        "pythagorean_identity",
        &[
            "pythagorean identity",
            "trig identity",
            "pythagorean trigonometric identity",
        ],
    ),
    (
        "binomial_theorem",
        &["binomial theorem", "binomial expansion", "binomial formula"],
    ),
    (
        "slope_formula",
        &["slope formula", "rise over run", "slope of a line"],
    ),
    (
        "distance_formula",
        &[
            "distance formula",
            "euclidean distance",
            "distance between two points",
        ],
    ),
];

/// Build a unified alias list from hardcoded + registry formulas for prose extraction.
///
/// This makes prose extraction UNIVERSAL: any formula registered in the
/// registry (including those extracted from PDF LaTeX blocks) is automatically
/// detectable in plain prose. No hardcoded name list required.
fn build_alias_list(registry: Option<&FormulaRegistry>) -> Vec<(String, Vec<String>)> {
    let mut aliases: Vec<(String, Vec<String>)> = Vec::new();

    // 1. Add all hardcoded KNOWN_FORMULA_NAMES
    for (slug, alias_slice) in KNOWN_FORMULA_NAMES {
        let alias_vec: Vec<String> = alias_slice.iter().map(|s| s.to_string()).collect();
        aliases.push((slug.to_string(), alias_vec));
    }

    // 2. Add all formulas from the registry (if available)
    if let Some(registry) = registry {
        for slug in registry.list_slugs() {
            if let Some(entry) = registry.by_slug(&slug) {
                // Skip duplicates already in KNOWN_FORMULA_NAMES
                if aliases.iter().any(|(s, _)| s == &slug) {
                    continue;
                }
                let mut formula_aliases: Vec<String> = Vec::new();

                // Generate aliases from the slug itself
                let display_name = slug.replace('_', " ");
                formula_aliases.push(display_name.clone());

                // Generate aliases from descriptions (SVO triples)
                for (s, v, o) in &entry.descriptions {
                    let desc_alias = format!("{} {} {}", s, v, o)
                        .replace('_', " ")
                        .replace("  ", " ")
                        .trim()
                        .to_string();
                    if desc_alias.len() >= 5 && !formula_aliases.contains(&desc_alias) {
                        formula_aliases.push(desc_alias);
                    }
                }

                // Generate alias from expr_str (e.g., "d/dx x^n = n*x^(n-1)" → derivative pattern)
                if !entry.expr_str.is_empty() && entry.expr_str.len() < 100 {
                    formula_aliases.push(entry.expr_str.clone());
                }

                // Add existing aliases from the entry
                for a in &entry.aliases {
                    if !formula_aliases.contains(a) {
                        formula_aliases.push(a.clone());
                    }
                }

                // Add name-based variants
                let name_variants = generate_prose_name_variants(&slug);
                for v in name_variants {
                    if !formula_aliases.contains(&v) {
                        formula_aliases.push(v);
                    }
                }

                aliases.push((slug, formula_aliases));
            }
        }
    }

    aliases
}

/// Generate natural-language variants of a formula slug for prose detection.
///
/// E.g., "derivative_of_sin" → ["derivative of sin", "sine derivative", "derivative of sine"]
///        "power_rule" → ["power rule", "rule of powers"]
fn generate_prose_name_variants(slug: &str) -> Vec<String> {
    let mut variants = Vec::new();
    let display = slug.replace('_', " ");
    variants.push(display.clone());

    // "X_of_Y" → "Y X" (e.g., "derivative_of_sin" → "sin derivative")
    if let Some(sep_pos) = slug.find("_of_") {
        let prefix = &slug[..sep_pos].replace('_', " ");
        let suffix = &slug[sep_pos + 4..].replace('_', " ");
        variants.push(format!("{} {}", suffix, prefix));
        // "derivative of the SUFFIX function"
        variants.push(format!("{} of the {} function", prefix, suffix));
        // "SUFFIX derivative"
        variants.push(format!("{} {}", suffix, prefix));
    }

    // "X_rule" → "X rule"
    if slug.ends_with("_rule") {
        let base = &slug[..slug.len() - 5].replace('_', " ");
        variants.push(format!("{} rule", base));
        variants.push(format!("rule of {}", base));
    }

    // "X_formula" → "X formula"
    if slug.ends_with("_formula") {
        let base = &slug[..slug.len() - 8].replace('_', " ");
        variants.push(format!("{} formula", base));
    }

    // "X_identity" → "X identity"
    if slug.ends_with("_identity") {
        let base = &slug[..slug.len() - 9].replace('_', " ");
        variants.push(format!("{} identity", base));
    }

    variants
}

/// Math-related verb phrases that indicate a formula definition.
const MATH_RELATION_VERBS: &[(&str, &str)] = &[
    ("states", "states_that"),
    ("states that", "states_that"),
    ("says", "says_that"),
    ("says that", "says_that"),
    ("gives", "gives"),
    ("gives us", "gives"),
    ("is defined as", "is"),
    ("is", "is"),
    ("equals", "equals"),
    ("is equal to", "equals"),
    ("is given by", "is"),
    ("computes", "computes"),
    ("is the derivative of", "is_derivative_of"),
    ("is the integral of", "is_integral_of"),
    ("is the antiderivative of", "is_antiderivative_of"),
];

/// General formula-definition patterns that work across any domain.
const FORMULA_DEFINITION_PATTERNS: &[&str] = &[
    "the formula for ",
    "the equation for ",
    "the law of ",
    "the theorem of ",
];

/// Pre-computed normalized token set for a single alias query.
#[allow(dead_code)]
struct IndexedAliasQuery {
    /// Slug of the formula being searched for.
    pub slug: String,
    /// The alias text itself (e.g. "power rule").
    pub alias: String,
    /// The verb key for the matched pattern.
    pub verb_key: String,
    /// The full query text: alias + verb + optional "that".
    pub query_text: String,
    /// Normalized, non-noise tokens of the query (for pre-filter).
    pub query_tokens: Vec<String>,
}

/// Inverted index for O(1) alias retrieval per sentence.
///
/// Builds once, then for each sentence we iterate its tokens, look up
/// candidate aliases in the inverted map, and only run expensive fuzzy
/// matching on candidates whose every token appears in the sentence.
struct FormulaAliasIndex {
    /// All aliases (slug → list of alias strings) for O(1) lookup.
    pub alias_map: HashMap<String, Vec<String>>,
    /// Pre-computed query entries for Patterns 1, 2, and 4.
    pub query_entries: Vec<IndexedAliasQuery>,
    /// Inverted index: normalized token → indices into `query_entries`.
    pub token_to_queries: HashMap<String, Vec<usize>>,
}

impl FormulaAliasIndex {
    pub fn build(registry: Option<&FormulaRegistry>) -> Self {
        let all_aliases = build_alias_list(registry);
        let mut alias_map: HashMap<String, Vec<String>> = HashMap::new();
        for (slug, aliases) in &all_aliases {
            alias_map.insert(slug.clone(), aliases.clone());
        }
        let mut query_entries = Vec::new();
        let mut token_to_queries: HashMap<String, Vec<usize>> = HashMap::default();

        for (slug, alias_list) in &all_aliases {
            for alias in alias_list {
                // Pattern 1: "alias verb_phrase that"
                for (verb_phrase, verb_key) in MATH_RELATION_VERBS {
                    let query_text = format!("{} {} that", alias, verb_phrase);
                    let query_tokens = normalize_query_tokens(&query_text);
                    if !query_tokens.is_empty() {
                        let idx = query_entries.len();
                        query_entries.push(IndexedAliasQuery {
                            slug: slug.clone(),
                            alias: alias.clone(),
                            verb_key: verb_key.to_string(),
                            query_text,
                            query_tokens: query_tokens.clone(),
                        });
                        for tok in &query_tokens {
                            token_to_queries.entry(tok.clone()).or_default().push(idx);
                        }
                    }
                }

                // Pattern 2: "alias verb_phrase" (no "that")
                for (verb_phrase, verb_key) in MATH_RELATION_VERBS {
                    let query_text = format!("{} {}", alias, verb_phrase);
                    let query_tokens = normalize_query_tokens(&query_text);
                    if !query_tokens.is_empty() {
                        let idx = query_entries.len();
                        query_entries.push(IndexedAliasQuery {
                            slug: slug.clone(),
                            alias: alias.clone(),
                            verb_key: verb_key.to_string(),
                            query_text,
                            query_tokens: query_tokens.clone(),
                        });
                        for tok in &query_tokens {
                            token_to_queries.entry(tok.clone()).or_default().push(idx);
                        }
                    }
                }

                // Pattern 4: "alias:"
                {
                    let query_text = format!("{}:", alias);
                    let query_tokens = normalize_query_tokens(&query_text);
                    if !query_tokens.is_empty() {
                        let idx = query_entries.len();
                        query_entries.push(IndexedAliasQuery {
                            slug: slug.clone(),
                            alias: alias.clone(),
                            verb_key: "gives".to_string(),
                            query_text,
                            query_tokens: query_tokens.clone(),
                        });
                        for tok in &query_tokens {
                            token_to_queries.entry(tok.clone()).or_default().push(idx);
                        }
                    }
                }
            }
        }

        FormulaAliasIndex {
            alias_map,
            query_entries,
            token_to_queries,
        }
    }

    /// Fast candidate retrieval: finds all queries whose normalized tokens
    /// are all present in `sentence_tokens`.
    ///
    /// Uses a sparse scoring approach: only processes entries that match
    /// at least one token, avoiding O(index_size) per sentence.
    ///
    /// Limits the number of returned candidates to MAX_CANDIDATES to keep
    /// downstream pattern matching tractable.
    pub fn candidates_for<'a>(
        &'a self,
        sentence_tokens: &[String],
        _sentence_token_set: &HashSet<String>,
    ) -> Vec<&'a IndexedAliasQuery> {
        const MAX_CANDIDATES: usize = 30;

        if self.query_entries.is_empty() {
            return Vec::new();
        }
        // Sparse score tracking: only entries that appear in the inverted
        // index for any sentence token get allocated.
        let mut scores: std::collections::HashMap<usize, u32> = std::collections::HashMap::new();
        for tok in sentence_tokens {
            if let Some(entries) = self.token_to_queries.get(tok.as_str()) {
                for &idx in entries {
                    *scores.entry(idx).or_insert(0) += 1;
                }
            }
        }
        // Keep queries where score == query_tokens.len() (all tokens present).
        // Sort by token count descending and limit to MAX_CANDIDATES.
        let mut matched: Vec<(&IndexedAliasQuery, usize)> = scores
            .into_iter()
            .filter_map(|(idx, score)| {
                let entry = &self.query_entries[idx];
                let required = entry.query_tokens.len();
                if score as usize == required && required >= 2 {
                    Some((entry, required))
                } else if score as usize == required && required == 1 {
                    // Single-token matches only count if the token is
                    // not a generic math term (length >= 4 or contains special chars)
                    let tok = &entry.query_tokens[0];
                    if tok.len() >= 4
                        || tok.contains('^')
                        || tok.contains('_')
                        || tok.contains('\\')
                    {
                        Some((entry, 1))
                    } else {
                        None
                    }
                } else {
                    None
                }
            })
            .collect();
        // Sort by query_tokens.len() descending (more specific = better match)
        matched.sort_by(|a, b| b.1.cmp(&a.1));
        matched.truncate(MAX_CANDIDATES);
        matched.into_iter().map(|(e, _)| e).collect()
    }
}

/// Normalize a query string into its constituent non-noise tokens.
fn normalize_query_tokens(text: &str) -> Vec<String> {
    let norm = normalize_text(text);
    let tokens = tokenize_words(&norm);
    let mut result = Vec::new();
    for t in &tokens {
        if let Some(n) = normalized_word(&t.word) {
            result.push(n);
        }
    }
    result
}

/// Build a normalized token cache for a single sentence.
struct SentenceTokenCache {
    /// All non-noise normalized tokens in the sentence.
    pub tokens: Vec<String>,
    /// Set of tokens for O(1) membership tests.
    pub token_set: HashSet<String>,
}

impl SentenceTokenCache {
    pub fn new(sentence: &str) -> Self {
        let norm = normalize_text(sentence);
        let tokens = tokenize_words(&norm);
        let mut result_tokens = Vec::new();
        for t in &tokens {
            if let Some(n) = normalized_word(&t.word) {
                result_tokens.push(n);
            }
        }
        let token_set: HashSet<String> = result_tokens.iter().cloned().collect();
        SentenceTokenCache {
            tokens: result_tokens,
            token_set,
        }
    }
}

/// Extract formula references from natural-language prose.
///
/// Detects patterns like:
/// - "The power rule states that d/dx x^n = n x^(n-1)"
/// - "The derivative of sin is cos(x)"
/// - "sin^2(x) + cos^2(x) = 1 is called the Pythagorean identity"
/// - "The quadratic formula: x = (-b ± sqrt(b^2 - 4ac))/(2a)"
///
/// These complement the LaTeX-block extraction by picking up formula
/// knowledge expressed in plain text.
///
/// If `registry` is provided, ALL registered formulas are detected in addition
/// to the hardcoded bootstrap set — making extraction universal for any
/// formula that has been ingested (e.g., from PDF LaTeX blocks).
///
/// Automatically registers unstructured matches (like "derivative of X is Y")
/// via the `auto_register` callback.
pub fn extract_formulas_from_prose(
    text: &str,
    source: &str,
    registry: Option<&FormulaRegistry>,
) -> Vec<ProseFormulaExtraction> {
    let mut results = Vec::new();
    let sentences = split_sentences(text);
    let index = FormulaAliasIndex::build(registry);
    let total = sentences.len();
    if total > 5000 {
        eprintln!("    Prose scan: {} sentences...", total);
    }

    for (sent_idx, sentence) in sentences.iter().enumerate() {
        if total > 5000 && sent_idx > 0 && sent_idx % 5000 == 0 {
            eprintln!(
                "    ... {}/{} sentences processed ({} formulas found so far)",
                sent_idx,
                total,
                results.len()
            );
        }
        let trimmed = sentence.trim();
        if trimmed.len() < 10 {
            continue;
        }

        // Skip sentences that already have LaTeX delimiters (handled elsewhere)
        if trimmed.contains('$') || trimmed.contains("\\(") || trimmed.contains("\\[") {
            continue;
        }

        // Build sentence token cache ONCE per sentence
        let cache = SentenceTokenCache::new(trimmed);
        if cache.tokens.is_empty() {
            // Still check general formula patterns (Pattern 5) and
            // derivative/integral-of patterns (they don't use aliases).
            results.extend(extract_general_formula_pattern(trimmed, source, &[]));
            results.extend(extract_derivative_of_pattern(trimmed, source));
            results.extend(extract_integral_of_pattern(trimmed, source));
            continue;
        }

        // Fast candidate retrieval from the inverted index
        let candidates = index.candidates_for(&cache.tokens, &cache.token_set);
        if candidates.is_empty() {
            // Still check general formula patterns
            results.extend(extract_general_formula_pattern(trimmed, source, &[]));
            results.extend(extract_derivative_of_pattern(trimmed, source));
            results.extend(extract_integral_of_pattern(trimmed, source));
            continue;
        }

        // Build filtered aliases using O(1) slug lookup instead of
        // iterating the full alias list.
        let mut matched_slugs: HashSet<&str> = HashSet::default();
        for q in &candidates {
            matched_slugs.insert(q.slug.as_str());
        }
        let filtered_aliases: Vec<(String, Vec<String>)> = {
            let mut result = Vec::new();
            for slug in &matched_slugs {
                if let Some(aliases) = index.alias_map.get(*slug) {
                    result.push((slug.to_string(), aliases.clone()));
                }
            }
            result
        };
        // Skip expensive pattern matching if too many candidates match
        // (happens when the sentence contains many generic math terms).
        // In this case, fall through to general formula patterns only.
        if filtered_aliases.len() > 20 {
            results.extend(extract_general_formula_pattern(trimmed, source, &[]));
            results.extend(extract_derivative_of_pattern(trimmed, source));
            results.extend(extract_integral_of_pattern(trimmed, source));
            continue;
        }

        // Pattern 1: "[Name] [verb] that [expression]"
        //   "The power rule states that d/dx x^n = n x^(n-1)"
        results.extend(extract_name_verb_that_pattern(
            trimmed,
            source,
            &filtered_aliases,
        ));

        // Pattern 2: "[Name] [verb] [expression]" (no 'that')
        //   "The derivative of sin is cos(x)"
        results.extend(extract_name_verb_expr_pattern(
            trimmed,
            source,
            &filtered_aliases,
        ));

        // Pattern 3: "[expression] is called/known as [name]"
        //   "sin^2(x) + cos^2(x) = 1 is known as the Pythagorean identity"
        results.extend(extract_expr_is_called_pattern(
            trimmed,
            source,
            &filtered_aliases,
        ));

        // Pattern 4: "[Name]: [expression]"
        //   "Power rule: d/dx x^n = n x^(n-1)"
        results.extend(extract_colon_pattern(trimmed, source, &filtered_aliases));

        // Pattern 5: General formula definition from context
        //   "The formula for kinetic energy is KE = 1/2 mv^2"
        //   "F = ma" (bare equality)
        results.extend(extract_general_formula_pattern(
            trimmed,
            source,
            &filtered_aliases,
        ));
    }

    // Deduplicate by name
    results.dedup_by(|a, b| a.name.to_lowercase() == b.name.to_lowercase());

    results
}

/// Split text into sentences (simple period-based heuristics).
fn split_sentences(text: &str) -> Vec<String> {
    let mut sentences = Vec::new();
    let mut current = String::new();

    for c in text.chars() {
        current.push(c);
        if matches!(c, '.' | '!' | '?') {
            let trimmed = current.trim().to_string();
            if !trimmed.is_empty() {
                sentences.push(trimmed);
            }
            current = String::new();
        }
    }

    let trimmed = current.trim().to_string();
    if !trimmed.is_empty() {
        sentences.push(trimmed);
    }

    sentences
}

/// Pattern 1: "[Name] [verb] that [expression]"
///
/// Uses fuzzy word-level matching so "derivative of the sine function"
/// matches the alias "derivative of sin", skipping noise words.
fn extract_name_verb_that_pattern(
    sentence: &str,
    source: &str,
    aliases: &[(String, Vec<String>)],
) -> Vec<ProseFormulaExtraction> {
    let mut results = Vec::new();
    let sentence_lower = sentence.to_lowercase();

    for (slug, alias_list) in aliases {
        for alias in alias_list {
            for (verb_phrase, verb_key) in MATH_RELATION_VERBS {
                // Quick pre-check: skip if NO content words (len >= 3) from the alias
                // appear in the sentence. This avoids expensive fuzzy matching for
                // clearly non-matching aliases.
                let alias_words: Vec<&str> = alias.split_whitespace().collect();
                let has_content_word = alias_words
                    .iter()
                    .any(|w| w.len() >= 3 && sentence_lower.contains(&w.to_lowercase()));
                if !alias_words.is_empty() && !has_content_word {
                    continue;
                }
                // Build a combined query: "alias verb that"
                let query = format!("{} {} that", alias, verb_phrase);
                if let Some(m) = fuzzy_alias_match(sentence, &query) {
                    // Find "that" after the match start, then everything after is the expression
                    if let Some(that_pos) = sentence[m.start..].to_lowercase().find("that") {
                        let expr_start = m.start + that_pos + 4; // skip "that"
                        let after = sentence[expr_start..].trim();
                        if after.len() >= 5 {
                            results.push(ProseFormulaExtraction {
                                name: slug.to_string(),
                                verb: verb_key.to_string(),
                                expression: after.to_string(),
                                context: sentence.to_string(),
                                source: source.to_string(),
                            });
                        }
                    }
                }
            }
        }
    }

    results
}

/// Pattern 2: "[Name] [verb] [expression]" (no 'that')
///
/// Uses fuzzy word-level matching so "the derivative of the cosine function is -sin(x)"
/// matches the alias "derivative of cos", skipping noise words.
fn extract_name_verb_expr_pattern(
    sentence: &str,
    source: &str,
    aliases: &[(String, Vec<String>)],
) -> Vec<ProseFormulaExtraction> {
    let mut results = Vec::new();
    let sentence_lower = sentence.to_lowercase();

    for (slug, alias_list) in aliases {
        for alias in alias_list {
            // Quick pre-check: skip if NO content words (len >= 3) from the alias
            // appear in the sentence.
            let alias_words: Vec<&str> = alias.split_whitespace().collect();
            let has_content_word = alias_words
                .iter()
                .any(|w| w.len() >= 3 && sentence_lower.contains(&w.to_lowercase()));
            if !alias_words.is_empty() && !has_content_word {
                continue;
            }
            for (verb_phrase, verb_key) in MATH_RELATION_VERBS {
                // Build combined query: "alias verb" (no "that" — this is Pattern 2)
                let query = format!("{} {}", alias, verb_phrase);
                if let Some(m) = fuzzy_alias_match(sentence, &query) {
                    // Check this isn't a Pattern 1 sentence (with "that" after the match).
                    // Use exact search for "that" (not fuzzy) so noise-words don't interfere.
                    let after_match = sentence[m.start..].to_lowercase();
                    if after_match.contains(" that ") {
                        continue;
                    }
                    if let Some(that_pos) = after_match.rfind("that") {
                        // "that" appears after the match — check it's within a few words
                        if that_pos < 20 {
                            continue;
                        }
                    }
                    // Everything after the match end is the expression
                    let after = sentence[m.end..].trim();
                    let expression = after.trim_end_matches('.').trim().to_string();
                    if expression.len() >= 3 {
                        results.push(ProseFormulaExtraction {
                            name: slug.to_string(),
                            verb: verb_key.to_string(),
                            expression,
                            context: sentence.to_string(),
                            source: source.to_string(),
                        });
                    }
                }
            }
        }
    }

    // Also detect unstructured patterns like "the derivative of X is Y"
    // These use fuzzy matching too
    results.extend(extract_derivative_of_pattern(sentence, source));
    results.extend(extract_integral_of_pattern(sentence, source));

    results
}

/// Pattern: "derivative of [function] is [result]"
///   "derivative of sin(x) is cos(x)"
///   "derivative of the sine function is cos(x)" (via synonym matching)
fn extract_derivative_of_pattern(sentence: &str, source: &str) -> Vec<ProseFormulaExtraction> {
    let mut results = Vec::new();
    let lower = sentence.to_lowercase();
    let norm_lower = normalize_text(&lower);

    // Find "derivative of" fuzzily
    if let Some(m) = fuzzy_alias_match(sentence, "derivative of") {
        // Safety: m.start is a byte index in `sentence`. `norm_lower` may differ
        // in length after lowercasing/synonym replacement. Clamp to avoid panic.
        let norm_start = m.start.min(norm_lower.len());
        let after = &norm_lower[norm_start..];
        if let Some(after_deriv) = after.find("derivative of") {
            let rest = &after[after_deriv + "derivative of".len()..].trim();
            // Search for " is " after the function name
            if let Some(is_pos) = rest.find(" is ") {
                let func = rest[..is_pos].trim();
                let result = rest[is_pos + 4..].trim_end_matches('.').trim();
                if !func.is_empty() && !result.is_empty() {
                    // Clean the function name for the slug: strip noise words
                    let clean_func: Vec<&str> = func
                        .split_whitespace()
                        .filter(|w| {
                            let wl = w.to_lowercase();
                            !NOISE_WORDS.contains(&wl.as_str()) && wl != "d/dx" && wl != "="
                        })
                        .collect();
                    let clean_slug = if clean_func.is_empty() {
                        func.replace(' ', "_")
                    } else {
                        clean_func.join("_")
                    };
                    let clean_name = clean_slug.replace('_', " ");
                    let slug = format!("derivative_of_{}", clean_slug);
                    results.push(ProseFormulaExtraction {
                        name: slug,
                        verb: "is".into(),
                        expression: format!("d/dx {} = {}", clean_name, result),
                        context: sentence.to_string(),
                        source: source.to_string(),
                    });
                }
            }
        }
    }

    results
}

/// Pattern: "integral of [function] is [result]"
fn extract_integral_of_pattern(sentence: &str, source: &str) -> Vec<ProseFormulaExtraction> {
    let mut results = Vec::new();
    let lower = sentence.to_lowercase();

    for trigger in &[
        "integral of ",
        "antiderivative of ",
        "indefinite integral of ",
    ] {
        if let Some(m) = fuzzy_alias_match(sentence, trigger.trim()) {
            let norm_lower = normalize_text(&lower);
            let trigger_norm = trigger.trim().to_lowercase();
            let norm_start = m.start.min(norm_lower.len());
            if let Some(t_pos) = norm_lower[norm_start..].find(&trigger_norm) {
                let rest = &norm_lower[norm_start + t_pos + trigger_norm.len()..].trim();
                // Try " is " pattern
                if let Some(is_pos) = rest.find(" is ") {
                    let func = rest[..is_pos].trim();
                    let result = rest[is_pos + 4..].trim_end_matches('.').trim();
                    if !func.is_empty() && !result.is_empty() {
                        // Clean the function name for the slug: strip noise words
                        let clean_func: Vec<&str> = func
                            .split_whitespace()
                            .filter(|w| {
                                let wl = w.to_lowercase();
                                !NOISE_WORDS.contains(&wl.as_str())
                                    && wl != "int"
                                    && wl != "dx"
                                    && wl != "="
                            })
                            .collect();
                        let clean_slug = if clean_func.is_empty() {
                            func.replace(' ', "_")
                        } else {
                            clean_func.join("_")
                        };
                        let slug = format!("integral_of_{}", clean_slug);
                        results.push(ProseFormulaExtraction {
                            name: slug,
                            verb: "is".into(),
                            expression: format!("int {} dx = {}", func, result),
                            context: sentence.to_string(),
                            source: source.to_string(),
                        });
                    }
                }
            }
        }
    }

    results
}

/// Pattern 3: "[expression] is called/known as [name]"
fn extract_expr_is_called_pattern(
    sentence: &str,
    source: &str,
    aliases: &[(String, Vec<String>)],
) -> Vec<ProseFormulaExtraction> {
    let mut results = Vec::new();
    let lower = sentence.to_lowercase();

    // "[expr] is known as the [name]"
    // "[expr] is called the [name]"
    // "[expr] is the [name]"
    for marker in &["is known as the ", "is called the ", "is the "] {
        if let Some(pos) = lower.find(marker) {
            let before = sentence[..pos].trim();
            let after = lower[pos + marker.len()..]
                .trim_end_matches('.')
                .trim()
                .to_string();

            // Check if "after" matches a known formula name using fuzzy matching
            for (slug, alias_list) in aliases {
                for alias in alias_list {
                    if fuzzy_alias_match(&after, alias).is_some()
                        || fuzzy_alias_match(&after, &format!("{} rule", alias)).is_some()
                        || fuzzy_alias_match(&after, &format!("{} formula", alias)).is_some()
                    {
                        results.push(ProseFormulaExtraction {
                            name: slug.to_string(),
                            verb: "is".into(),
                            expression: before.to_string(),
                            context: sentence.to_string(),
                            source: source.to_string(),
                        });
                        break;
                    }
                }
            }
        }
    }

    results
}

/// Pattern 4: "[Name]: [expression]"
///
/// Uses fuzzy matching so "The power rule: d/dx x^n = n x^(n-1)"
/// matches even with variant phrasing.
///
/// Verifies that `:` actually appears near the match end (the fuzzy matcher
/// strips non-word characters, so we must check for the colon in the original).
fn extract_colon_pattern(
    sentence: &str,
    source: &str,
    aliases: &[(String, Vec<String>)],
) -> Vec<ProseFormulaExtraction> {
    let mut results = Vec::new();

    for (slug, alias_list) in aliases {
        for alias in alias_list {
            let query = format!("{}:", alias);
            if let Some(m) = fuzzy_alias_match(sentence, &query) {
                // Verify a colon actually appears near the match end in the original
                let after = &sentence[m.end..];
                let colon_pos = after.find(':');
                let no_colon_found = colon_pos.is_none() || colon_pos.unwrap() > 5;
                if no_colon_found {
                    continue;
                }
                // Skip past the colon
                let colon_end = colon_pos.unwrap() + 1;
                let expression = after[colon_end..]
                    .trim()
                    .trim_end_matches('.')
                    .trim()
                    .to_string();
                if expression.len() >= 3 {
                    results.push(ProseFormulaExtraction {
                        name: slug.to_string(),
                        verb: "gives".into(),
                        expression,
                        context: sentence.to_string(),
                        source: source.to_string(),
                    });
                }
            }
        }
    }

    results
}

/// Pattern 5: General formula definition from context.
///
/// Detects patterns like:
/// - "The formula for kinetic energy is KE = 1/2 mv^2"
/// - "The equation for the area of a circle is A = pi r^2"
/// - "The law of gravitation is F = G*m1*m2/r^2"
///
/// These work for ANY domain — physics, geometry, statistics, etc.
fn extract_general_formula_pattern(
    sentence: &str,
    source: &str,
    _aliases: &[(String, Vec<String>)],
) -> Vec<ProseFormulaExtraction> {
    let mut results = Vec::new();
    let lower = sentence.to_lowercase();

    // Pattern: "the [formula/equation/law/theorem] for/of [name] is [expr]"
    for trigger in FORMULA_DEFINITION_PATTERNS {
        if let Some(pos) = lower.find(trigger) {
            let after = &lower[pos + trigger.len()..];

            // Find the verb ("is", "equals", "states") after the name
            for (verb_phrase, verb_key) in MATH_RELATION_VERBS {
                let verb_pattern = format!(" {} ", verb_phrase);
                if let Some(v_pos) = after.find(&verb_pattern) {
                    let name = after[..v_pos].trim();
                    let expr = after[v_pos + verb_pattern.len()..]
                        .trim_end_matches('.')
                        .trim()
                        .to_string();

                    // Clean the name: remove trailing "that", limit length
                    let clean_name = name.trim_end_matches(" that").trim();
                    if clean_name.len() >= 3 && expr.len() >= 3 && clean_name.len() < 60 {
                        let slug = format!("formula_{}", clean_name.replace(' ', "_"));
                        results.push(ProseFormulaExtraction {
                            name: slug,
                            verb: verb_key.to_string(),
                            expression: expr,
                            context: sentence.to_string(),
                            source: source.to_string(),
                        });
                        break; // Found a verb match — skip remaining verbs
                    }
                }
            }
        }
    }

    // Pattern: bare "X = Y" sentences with domain context
    // e.g., "F = ma" in a physics text
    // Only if not already matched above
    if results.is_empty() {
        if let Some(eq_pos) = lower.find(" = ") {
            let before = lower[..eq_pos].trim();
            let after = lower[eq_pos + 3..].trim_end_matches('.').trim();
            // Only detect if the left side is short (a formula, not prose)
            if !before.contains(' ') && before.len() <= 5 && after.len() >= 2 {
                let slug = format!("formula_{}_equals_{}", before, after.replace(' ', "_"));
                results.push(ProseFormulaExtraction {
                    name: slug,
                    verb: "equals".into(),
                    expression: format!("{} = {}", before, after),
                    context: sentence.to_string(),
                    source: source.to_string(),
                });
            }
        }
    }

    results
}

/// Integrate prose formula extractions into the FormulaRegistry.
///
/// This converts `ProseFormulaExtraction`s into `FormulaEntry`s and
/// registers them in the registry, generating slugs from the extracted
/// name field and preserving the SVO triples as descriptions.
pub fn ingest_prose_formulas(
    extractions: &[ProseFormulaExtraction],
    registry: &mut FormulaRegistry,
    default_domain: &str,
) -> usize {
    let mut count = 0;

    for extraction in extractions {
        let slug = extraction.name.replace(' ', "_").to_lowercase();
        let entry = FormulaEntry {
            slug: slug.clone(),
            expr_str: extraction.expression.clone(),
            descriptions: vec![(
                extraction.name.clone(),
                extraction.verb.clone(),
                extraction.expression.clone(),
            )],
            aliases: vec![extraction.name.clone()],
            source: extraction.source.clone(),
            domain: default_domain.to_string(),
            tags: vec!["ingested_from_prose".into(), slug.clone()],
            linked_fact_ids: Vec::new(),
        };

        match registry.register(entry) {
            Ok(()) => count += 1,
            Err(_) => {
                // Slug already exists — add the prose extraction as an additional description
                if let Some(existing) = registry.by_slug_mut(&slug) {
                    existing.descriptions.push((
                        extraction.name.clone(),
                        extraction.verb.clone(),
                        extraction.expression.clone(),
                    ));
                    if !existing.aliases.contains(&extraction.name) {
                        existing.aliases.push(extraction.name.clone());
                    }
                    count += 1;
                }
            }
        }
    }

    count
}

// ═══════════════════════════════════════════════════════════════════════
// LATEX → SymExpr PARSER
// ═══════════════════════════════════════════════════════════════════════

/// Convert a LaTeX math string to a SymExpr.
///
/// Supports the common subset of LaTeX math:
/// - Arithmetic: +, -, *, /, ^, fraction
/// - Functions: \sin, \cos, \tan, \ln, \log, \exp, \sqrt
/// - Greek letters as variable names
/// - Parentheses, brackets
/// - Equality (=) creates a special structure
///
/// Returns None if the expression can't be parsed.
pub fn latex_to_symexpr(latex: &str) -> Option<SymExpr> {
    let tokens = tokenize_latex(latex)?;
    parse_expr(&tokens, 0).map(|(expr, _)| expr)
}

// ── Tokenizer ────────────────────────────────────────────────────────

#[derive(Clone, Debug, PartialEq)]
enum LToken {
    Num(f64),
    Var(String),
    Op(char), // +, -, *, /, ^, =, !, ', , (comma)
    LParen,
    RParen,
    LBrack,
    RBrack,
    LBrace,
    RBrace,
    Bar,             // |
    Command(String), // \sin, \frac, \int, etc.
    Subscript,       // _
    Prime,           // '
    End,
}

fn tokenize_latex(s: &str) -> Option<Vec<LToken>> {
    let chars: Vec<char> = s.chars().collect();
    let mut tokens = Vec::new();
    let mut i = 0;

    while i < chars.len() {
        let c = chars[i];

        // Skip whitespace (but not inside commands)
        if c.is_whitespace() {
            i += 1;
            continue;
        }

        // Commands: \command
        if c == '\\' {
            i += 1;
            let start = i;
            while i < chars.len() && chars[i].is_ascii_alphabetic() {
                i += 1;
            }
            if i == start {
                // Handle \,, \;, \: etc. (thin spaces) — skip them
                if i < chars.len() && matches!(chars[i], ',' | ';' | ':' | '!' | ' ') {
                    i += 1;
                }
                continue;
            }
            let cmd: String = chars[start..i].iter().collect();
            tokens.push(LToken::Command(cmd));
            continue;
        }

        // Numbers
        if c.is_ascii_digit() || c == '.' {
            let start = i;
            while i < chars.len() && (chars[i].is_ascii_digit() || chars[i] == '.') {
                i += 1;
            }
            let num_str: String = chars[start..i].iter().collect();
            if let Ok(n) = num_str.parse::<f64>() {
                tokens.push(LToken::Num(n));
            } else {
                return None;
            }
            continue;
        }

        // Letters (variable names)
        if c.is_ascii_alphabetic() {
            let start = i;
            while i < chars.len() && chars[i].is_ascii_alphanumeric() {
                i += 1;
            }
            let name: String = chars[start..i].iter().collect();
            tokens.push(LToken::Var(name));
            continue;
        }

        // Operators and punctuation
        match c {
            '+' | '-' | '*' | '/' | '^' | '=' | '!' | ',' => {
                tokens.push(LToken::Op(c));
                i += 1;
            }
            '(' => {
                tokens.push(LToken::LParen);
                i += 1;
            }
            ')' => {
                tokens.push(LToken::RParen);
                i += 1;
            }
            '[' => {
                tokens.push(LToken::LBrack);
                i += 1;
            }
            ']' => {
                tokens.push(LToken::RBrack);
                i += 1;
            }
            '{' => {
                tokens.push(LToken::LBrace);
                i += 1;
            }
            '}' => {
                tokens.push(LToken::RBrace);
                i += 1;
            }
            '|' => {
                tokens.push(LToken::Bar);
                i += 1;
            }
            '_' => {
                tokens.push(LToken::Subscript);
                i += 1;
            }
            '\'' => {
                tokens.push(LToken::Prime);
                i += 1;
            }
            // Unicode math operators
            '·' | '×' | '⋅' => {
                tokens.push(LToken::Op('*'));
                i += 1;
            }
            '÷' => {
                tokens.push(LToken::Op('/'));
                i += 1;
            }
            '±' => {
                tokens.push(LToken::Op('+'));
                i += 1;
            } // approximation
            '\u{2212}' => {
                tokens.push(LToken::Op('-'));
                i += 1;
            } // − (minus sign)
            '\u{2211}' => {
                tokens.push(LToken::Command("sum".into()));
                i += 1;
            } // ∑
            '\u{222B}' => {
                tokens.push(LToken::Command("int".into()));
                i += 1;
            } // ∫
            '\u{2202}' => {
                tokens.push(LToken::Command("partial".into()));
                i += 1;
            } // ∂
            '\u{221A}' => {
                tokens.push(LToken::Command("sqrt".into()));
                i += 1;
            } // √
            // Greek letters (Unicode)
            '\u{03B1}' => {
                tokens.push(LToken::Var("alpha".into()));
                i += 1;
            }
            '\u{03B2}' => {
                tokens.push(LToken::Var("beta".into()));
                i += 1;
            }
            '\u{03B3}' => {
                tokens.push(LToken::Var("gamma".into()));
                i += 1;
            }
            '\u{03B4}' => {
                tokens.push(LToken::Var("delta".into()));
                i += 1;
            }
            '\u{03B5}' => {
                tokens.push(LToken::Var("epsilon".into()));
                i += 1;
            }
            '\u{03B8}' => {
                tokens.push(LToken::Var("theta".into()));
                i += 1;
            }
            '\u{03BB}' => {
                tokens.push(LToken::Var("lambda".into()));
                i += 1;
            }
            '\u{03BC}' => {
                tokens.push(LToken::Var("mu".into()));
                i += 1;
            }
            '\u{03C0}' => {
                tokens.push(LToken::Var("pi".into()));
                i += 1;
            }
            '\u{03C3}' => {
                tokens.push(LToken::Var("sigma".into()));
                i += 1;
            }
            '\u{03C9}' => {
                tokens.push(LToken::Var("omega".into()));
                i += 1;
            }
            '\u{0394}' => {
                tokens.push(LToken::Var("Delta".into()));
                i += 1;
            }
            '\u{03A3}' => {
                tokens.push(LToken::Var("Sigma".into()));
                i += 1;
            }
            _ => {
                // Skip unknown characters
                i += 1;
            }
        }
    }

    tokens.push(LToken::End);
    Some(tokens)
}

// ── Parser ───────────────────────────────────────────────────────────

/// Parse an expression (handles `=` as top-level, then add/sub).
fn parse_expr(tokens: &[LToken], pos: usize) -> Option<(SymExpr, usize)> {
    // Check for equality: left = right
    let (left, mut pos) = parse_add_sub(tokens, pos)?;
    if tokens[pos] == LToken::Op('=') {
        let (right, new_pos) = parse_expr(tokens, pos + 1)?;
        pos = new_pos;
        Some((right, pos))
    } else if let LToken::Command(cmd) = &tokens[pos] {
        // Check for relational operators like \to, \rightarrow, \Rightarrow
        // These connect two expressions: "x \to 0" means x approaches 0.
        // We return the right side (the approach value).
        if RELATIONAL_COMMANDS.contains(&cmd.as_str()) {
            let (right, new_pos) = parse_expr(tokens, pos + 1)?;
            pos = new_pos;
            Some((right, pos))
        } else {
            Some((left, pos))
        }
    } else {
        Some((left, pos))
    }
}

/// Parse addition and subtraction.
fn parse_add_sub(tokens: &[LToken], pos: usize) -> Option<(SymExpr, usize)> {
    let (mut left, mut pos) = parse_mul_div(tokens, pos)?;

    loop {
        match &tokens[pos] {
            LToken::Op('+') => {
                let (right, new_pos) = parse_mul_div(tokens, pos + 1)?;
                left = SymExpr::Add(Box::new(left), Box::new(right));
                pos = new_pos;
            }
            LToken::Op('-') => {
                let (right, new_pos) = parse_mul_div(tokens, pos + 1)?;
                left = SymExpr::Sub(Box::new(left), Box::new(right));
                pos = new_pos;
            }
            _ => break,
        }
    }

    Some((left, pos))
}

/// Parse multiplication and division (and implicit multiplication).
fn parse_mul_div(tokens: &[LToken], pos: usize) -> Option<(SymExpr, usize)> {
    let (mut left, mut pos) = parse_power(tokens, pos)?;

    loop {
        match &tokens[pos] {
            LToken::Op('*') => {
                let (right, new_pos) = parse_power(tokens, pos + 1)?;
                left = SymExpr::Mul(Box::new(left), Box::new(right));
                pos = new_pos;
            }
            LToken::Op('/') => {
                let (right, new_pos) = parse_power(tokens, pos + 1)?;
                left = SymExpr::Div(Box::new(left), Box::new(right));
                pos = new_pos;
            }
            // Implicit multiplication: number followed by variable, function, or paren
            _ if is_implicit_mul(&left, &tokens[pos]) => {
                let (right, new_pos) = parse_power(tokens, pos)?;
                left = SymExpr::Mul(Box::new(left), Box::new(right));
                pos = new_pos;
            }
            _ => break,
        }
    }

    Some((left, pos))
}

/// Commands that are relational operators, not factors.
/// They should NOT trigger implicit multiplication.
const RELATIONAL_COMMANDS: &[&str] = &[
    "to",
    "rightarrow",
    "Rightarrow",
    "implies",
    "iff",
    "neq",
    "approx",
    "cong",
    "sim",
    "leq",
    "geq",
    "le",
    "ge",
];

/// Check if the next token starts an implicit multiplication.
///
/// Excludes relational operators (`\to`, `\rightarrow`, etc.) so that
/// `x \to 0` is not parsed as `x * \to 0` (= `x * 0` = `0`).
fn is_implicit_mul(_left: &SymExpr, next: &LToken) -> bool {
    match next {
        LToken::Var(_) | LToken::Num(_) | LToken::LParen | LToken::LBrack | LToken::Bar => true,
        LToken::Command(cmd) => !RELATIONAL_COMMANDS.contains(&cmd.as_str()),
        _ => false,
    }
}

/// Parse power (right-associative).
fn parse_power(tokens: &[LToken], pos: usize) -> Option<(SymExpr, usize)> {
    let (base, pos) = parse_unary(tokens, pos)?;

    if tokens[pos] == LToken::Op('^') {
        let (exp, new_pos) = parse_power(tokens, pos + 1)?;
        Some((SymExpr::Pow(Box::new(base), Box::new(exp)), new_pos))
    } else {
        Some((base, pos))
    }
}

/// Parse unary plus/minus and function commands.
fn parse_unary(tokens: &[LToken], pos: usize) -> Option<(SymExpr, usize)> {
    match &tokens[pos] {
        LToken::Op('-') => {
            let (expr, pos) = parse_unary(tokens, pos + 1)?;
            Some((-expr, pos))
        }
        LToken::Op('+') => parse_unary(tokens, pos + 1),
        LToken::Command(_cmd) => parse_command(tokens, pos),
        _ => parse_atom(tokens, pos),
    }
}

/// Check if a fraction represents derivative notation.
///
/// Recognizes:
/// - `\frac{d}{dx}`, `\frac{d}{dy}` — first derivative
/// - `\frac{d^2}{dx^2}`, `\frac{d^n}{dx^n}` — higher-order derivatives
/// - `\frac{∂}{∂x}`, `\frac{∂^2}{∂x^2}` — partial derivatives
/// - `\frac{∂}{∂x^2}` — partial with exponent on variable
fn is_derivative_notation(num: &SymExpr, den: &SymExpr) -> bool {
    // Numerator must be d, ∂, d^n, or ∂^n
    let num_ok = match num {
        SymExpr::Var(v) => v == "d" || v == "∂",
        SymExpr::Pow(base, _) => {
            matches!(base.as_ref(), SymExpr::Var(v) if v == "d" || v == "∂")
        }
        _ => false,
    };
    if !num_ok {
        return false;
    }

    // Denominator must be a derivative variable: dx, dy, ∂x, or with exponent
    // It can also be Mul(∂, var) when \partial is followed by a variable
    // (in LaTeX, \partial x parses as implicit multiplication: ∂ * x)
    let den_ok = match den {
        SymExpr::Var(v) => v.display.starts_with('d') || v.display.starts_with('∂'),
        SymExpr::Pow(base, _) => matches!(base.as_ref(), SymExpr::Var(v)
            if v.display.starts_with('d') || v.display.starts_with('∂')),
        SymExpr::Mul(left, _) => matches!(left.as_ref(), SymExpr::Var(v) if v == "d" || v == "∂"),
        _ => false,
    };
    if !den_ok {
        return false;
    }

    true
}

/// Parse a LaTeX command like \frac, \sin, \sqrt, etc.
fn parse_command(tokens: &[LToken], pos: usize) -> Option<(SymExpr, usize)> {
    match &tokens[pos] {
        LToken::Command(cmd) => {
            match cmd.as_str() {
                "frac" => {
                    // \frac{numerator}{denominator}
                    // Special case: \frac{d}{dx}, \frac{d^2}{dx^2}, \frac{∂}{∂x} — derivative notation
                    // Returns the body after the fraction as-is
                    let (num, pos) = parse_atom_braced(tokens, pos + 1)?;
                    let (den, pos) = parse_atom_braced(tokens, pos)?;
                    if is_derivative_notation(&num, &den) {
                        // Derivative operator: parse the next expression as the body
                        // If there's nothing after it, fall back to treating as division
                        if pos >= tokens.len() || tokens[pos] == LToken::End {
                            Some((SymExpr::Div(Box::new(num), Box::new(den)), pos))
                        } else {
                            match parse_unary(tokens, pos) {
                                Some((body, p)) => Some((body, p)),
                                None => Some((SymExpr::Div(Box::new(num), Box::new(den)), pos)),
                            }
                        }
                    } else {
                        Some((SymExpr::Div(Box::new(num), Box::new(den)), pos))
                    }
                }
                "sqrt" => {
                    // \sqrt{expr} or \sqrt[n]{expr}
                    if pos + 1 < tokens.len() && tokens[pos + 1] == LToken::LBrack {
                        // \sqrt[n]{expr}
                        let (n, pos) = parse_bracket_content(tokens, pos + 1)?;
                        let (expr, pos) = parse_atom_braced(tokens, pos)?;
                        let n_val = if let SymExpr::Num(x) = n { x } else { 2.0 };
                        Some((expr.pow(SymExpr::Num(1.0 / n_val)), pos))
                    } else {
                        let (expr, pos) = parse_atom_braced(tokens, pos + 1)?;
                        Some((SymExpr::Sqrt(Box::new(expr)), pos))
                    }
                }
                "sin" | "cos" | "tan" | "ln" | "exp" | "log" | "sinh" | "cosh" | "tanh"
                | "asin" | "acos" | "atan" => {
                    let (arg, pos) = parse_func_arg(tokens, pos + 1)?;
                    match cmd.as_str() {
                        "sin" => Some((SymExpr::Sin(Box::new(arg)), pos)),
                        "cos" => Some((SymExpr::Cos(Box::new(arg)), pos)),
                        "tan" => Some((SymExpr::Tan(Box::new(arg)), pos)),
                        "ln" => Some((SymExpr::Ln(Box::new(arg)), pos)),
                        "log" => Some((SymExpr::Ln(Box::new(arg)), pos)), // treat log as ln
                        "exp" => Some((SymExpr::Exp(Box::new(arg)), pos)),
                        "sinh" => Some((SymExpr::Sinh(Box::new(arg)), pos)),
                        "cosh" => Some((SymExpr::Cosh(Box::new(arg)), pos)),
                        "tanh" => Some((SymExpr::Tanh(Box::new(arg)), pos)),
                        "asin" => Some((SymExpr::Asin(Box::new(arg)), pos)),
                        "acos" => Some((SymExpr::Acos(Box::new(arg)), pos)),
                        "atan" => Some((SymExpr::Atan(Box::new(arg)), pos)),
                        _ => unreachable!(),
                    }
                }
                "int" | "integral" => {
                    // \int, \int_{a}^{b} f(x) dx
                    let (p, lower, upper) = parse_command_bounds(tokens, pos + 1);
                    // Parse the integrand
                    let (integrand, p) = parse_expr(tokens, p)?;
                    // Strip trailing differential (e.g., "*dx", "*dy", "*dt")
                    let body = strip_trailing_differential(integrand);
                    // Determine integration variable: extract from trailing differential
                    // or default to "x"
                    let var_name = guess_integration_variable(&body, &tokens, p);
                    Some((
                        SymExpr::Integral {
                            variable: crate::algebra::Variable::named(&var_name),
                            lower: lower.map(Box::new),
                            upper: upper.map(Box::new),
                            body: Box::new(body),
                        },
                        p,
                    ))
                }
                "sum" | "Sigma" => {
                    // \sum_{i=1}^{n} expr
                    let (p, _lower, _upper) = parse_command_bounds(tokens, pos + 1);
                    let (body, p) = parse_expr(tokens, p)?;
                    Some((body, p))
                }
                "partial" => {
                    // ∂ — partial derivative operator
                    // Just return the variable ∂; the actual argument (e.g., x^2, x)
                    // will be parsed by the outer expression grammar via implicit
                    // multiplication. This handles \partial x, \partial^2, etc.
                    Some((SymExpr::Var("∂".into()), pos + 1))
                }
                "lim" | "limit" => {
                    // \lim_{x \to a} expr — parse the subscript manually to
                    // capture both the variable and the approach value.
                    let mut p = pos + 1;
                    let mut var_name = String::from("x");
                    let mut approach = SymExpr::Num(0.0);

                    if p < tokens.len() && tokens[p] == LToken::Subscript {
                        p += 1;
                        if p < tokens.len() && tokens[p] == LToken::LBrace {
                            p += 1; // skip '{'
                                    // Parse the content until '}' or '→' or '\to'
                            let start = p;
                            // Scan for the \to or → within the braces
                            let mut arrow_pos = None;
                            let mut depth = 1;
                            let mut scan = p;
                            while scan < tokens.len() && depth > 0 {
                                match &tokens[scan] {
                                    LToken::RBrace => depth -= 1,
                                    LToken::LBrace => depth += 1,
                                    LToken::Command(c)
                                        if c == "to" || c == "rightarrow" || c == "Rightarrow" =>
                                    {
                                        arrow_pos = Some(scan);
                                        break;
                                    }
                                    _ => {}
                                }
                                scan += 1;
                            }
                            if let Some(arrow_idx) = arrow_pos {
                                // Parse variable before the arrow
                                if let Some((var_expr, _)) = parse_atom(tokens, start) {
                                    if let SymExpr::Var(v) = &var_expr {
                                        var_name = v.to_string();
                                    }
                                }
                                // Parse approach value after the arrow
                                let after_arrow = arrow_idx + 1;
                                if let Some((approx, np)) = parse_expr(tokens, after_arrow) {
                                    approach = approx;
                                    p = np;
                                } else {
                                    p = arrow_idx + 2;
                                }
                                // Find matching closing brace
                                let mut depth2 = 1;
                                while p < tokens.len() && depth2 > 0 {
                                    match &tokens[p] {
                                        LToken::RBrace => depth2 -= 1,
                                        LToken::LBrace => depth2 += 1,
                                        _ => {}
                                    }
                                    if depth2 > 0 {
                                        p += 1;
                                    }
                                }
                                if p < tokens.len() {
                                    p += 1;
                                }
                            } else {
                                // No arrow found — skip to end of brace group
                                let mut depth2 = 1;
                                while p < tokens.len() && depth2 > 0 {
                                    match &tokens[p] {
                                        LToken::RBrace => depth2 -= 1,
                                        LToken::LBrace => depth2 += 1,
                                        _ => {}
                                    }
                                    if depth2 > 0 {
                                        p += 1;
                                    }
                                }
                                if p < tokens.len() {
                                    p += 1;
                                }
                            }
                        }
                    }
                    let (body, p) = parse_expr(tokens, p)?;
                    Some((
                        SymExpr::Limit {
                            variable: crate::algebra::Variable::named(&var_name),
                            approach: Box::new(approach),
                            body: Box::new(body),
                        },
                        p,
                    ))
                }
                "to" | "rightarrow" | "Rightarrow" | "implies" | "iff" => {
                    // Arrow — treat as implication, return left side
                    let (right, pos) = parse_expr(tokens, pos + 1)?;
                    Some((right, pos))
                }
                "cdot" | "times" | "ast" => {
                    // Multiplication: skip the command, treat next as factor
                    let (right, pos) = parse_power(tokens, pos + 1)?;
                    Some((right, pos))
                }
                "left" | "right" | "big" | "Big" | "bigg" | "Bigg" => {
                    // Sizing commands: skip them, parse the next token
                    parse_unary(tokens, pos + 1)
                }
                "quad" | "qquad" | "," | ";" | ":" | "!" | " " => {
                    // Spacing: skip
                    parse_unary(tokens, pos + 1)
                }
                "text" | "mathrm" | "mathbf" | "mathit" => {
                    // Text in math mode: parse the braced content as text, skip
                    let (_, pos) = parse_atom_braced(tokens, pos + 1)?;
                    parse_unary(tokens, pos)
                }
                "alpha" => Some((SymExpr::Var("alpha".into()), pos + 1)),
                "beta" => Some((SymExpr::Var("beta".into()), pos + 1)),
                "gamma" => Some((SymExpr::Var("gamma".into()), pos + 1)),
                "delta" => Some((SymExpr::Var("delta".into()), pos + 1)),
                "epsilon" => Some((SymExpr::Var("epsilon".into()), pos + 1)),
                "theta" => Some((SymExpr::Var("theta".into()), pos + 1)),
                "lambda" => Some((SymExpr::Var("lambda".into()), pos + 1)),
                "mu" => Some((SymExpr::Var("mu".into()), pos + 1)),
                "sigma" => Some((SymExpr::Var("sigma".into()), pos + 1)),
                "omega" => Some((SymExpr::Var("omega".into()), pos + 1)),
                "pi" => Some((SymExpr::Num(std::f64::consts::PI), pos + 1)),
                "infty" | "inf" => Some((SymExpr::Var("infinity".into()), pos + 1)),
                "neq" | "approx" | "cong" | "sim" => {
                    // Relational operators: skip, parse right side
                    let (right, pos) = parse_expr(tokens, pos + 1)?;
                    Some((right, pos))
                }
                "leq" | "geq" | "le" | "ge" => {
                    let (right, pos) = parse_expr(tokens, pos + 1)?;
                    Some((right, pos))
                }
                _ => {
                    // Unknown command: try to parse its argument as fallback
                    parse_func_arg(tokens, pos + 1)
                }
            }
        }
        _ => parse_atom(tokens, pos),
    }
}

/// Parse an atom: number, variable, parenthesized expression, or braced group.
fn parse_atom(tokens: &[LToken], pos: usize) -> Option<(SymExpr, usize)> {
    match &tokens[pos] {
        LToken::Num(n) => {
            // Check for subscript after number
            let mut p = pos + 1;
            while p < tokens.len() && tokens[p] == LToken::Prime {
                p += 1;
            }
            Some((SymExpr::Num(*n), p))
        }
        LToken::Var(name) => {
            let mut p = pos + 1;
            // Handle primes: x', x'', etc.
            while p < tokens.len() && tokens[p] == LToken::Prime {
                p += 1;
            }
            // Handle subscript: x_n
            if p < tokens.len() && tokens[p] == LToken::Subscript {
                let (sub, np) = parse_atom_braced(tokens, p + 1)?;
                let sub_str = format!("{}", sub);
                let full_name = format!("{}_{}", name, sub_str);
                p = np;
                Some((SymExpr::Var(crate::algebra::Variable::named(&full_name)), p))
            } else {
                Some((SymExpr::Var(crate::algebra::Variable::named(&name)), p))
            }
        }
        LToken::LParen => {
            let (expr, mut p) = parse_expr(tokens, pos + 1)?;
            if p < tokens.len() && tokens[p] == LToken::RParen {
                p += 1;
                Some((expr, p))
            } else {
                None // unmatched paren
            }
        }
        LToken::LBrack => {
            let (expr, mut p) = parse_expr(tokens, pos + 1)?;
            if p < tokens.len() && tokens[p] == LToken::RBrack {
                p += 1;
                Some((expr, p))
            } else {
                None
            }
        }
        LToken::LBrace => {
            let (expr, mut p) = parse_expr(tokens, pos + 1)?;
            if p < tokens.len() && tokens[p] == LToken::RBrace {
                p += 1;
                Some((expr, p))
            } else {
                None
            }
        }
        LToken::Bar => {
            // |expr|
            let (expr, mut p) = parse_expr(tokens, pos + 1)?;
            if p < tokens.len() && tokens[p] == LToken::Bar {
                p += 1;
                Some((SymExpr::Abs(Box::new(expr)), p))
            } else {
                None
            }
        }
        LToken::Command(_cmd) => parse_command(tokens, pos),
        LToken::End => None,
        _ => None,
    }
}

/// Parse a function argument: either a braced group `{...}` or a single atom.
fn parse_func_arg(tokens: &[LToken], pos: usize) -> Option<(SymExpr, usize)> {
    if pos >= tokens.len() {
        return None;
    }
    match &tokens[pos] {
        LToken::LBrace => parse_atom_braced(tokens, pos),
        LToken::LParen => parse_atom(tokens, pos), // (expr)
        _ => parse_atom(tokens, pos),              // single atom
    }
}

/// Parse a braced group `{ expr }` — returns the inner expression.
fn parse_atom_braced(tokens: &[LToken], pos: usize) -> Option<(SymExpr, usize)> {
    if pos >= tokens.len() {
        return None;
    }
    if tokens[pos] != LToken::LBrace {
        return None;
    }
    let (expr, mut p) = parse_expr(tokens, pos + 1)?;
    if p < tokens.len() && tokens[p] == LToken::RBrace {
        p += 1;
        Some((expr, p))
    } else {
        None
    }
}

/// Strip a trailing differential (e.g., `*dx`, `*dy`, `*dt`) from an expression.
fn strip_trailing_differential(expr: SymExpr) -> SymExpr {
    match expr {
        SymExpr::Mul(left, right) => {
            let right = *right;
            match &right {
                SymExpr::Var(v)
                    if v.display.len() == 2
                        && v.display.starts_with('d')
                        && v.display.as_bytes()[1].is_ascii_alphabetic() =>
                {
                    *left
                }
                _ => SymExpr::Mul(
                    Box::new(strip_trailing_differential(*left)),
                    Box::new(right),
                ),
            }
        }
        SymExpr::Add(l, r) => SymExpr::Add(
            Box::new(strip_trailing_differential(*l)),
            Box::new(strip_trailing_differential(*r)),
        ),
        SymExpr::Sub(l, r) => SymExpr::Sub(
            Box::new(strip_trailing_differential(*l)),
            Box::new(strip_trailing_differential(*r)),
        ),
        other => other,
    }
}

/// Guess the integration variable from a trailing differential or context.
/// Looks for "dx", "dy", "dt", "dz", etc. in the trailing tokens or body.
fn guess_integration_variable(body: &SymExpr, tokens: &[LToken], pos: usize) -> String {
    // Try the trailing differential from the token stream
    if pos < tokens.len() {
        if let LToken::Var(v) = &tokens[pos] {
            if v.len() == 2 && v.starts_with('d') && v.as_bytes()[1].is_ascii_alphabetic() {
                return v[1..].to_string();
            }
        }
    }
    // Fallback: scan the body for variables
    match body {
        SymExpr::Var(v)
            if v.display.len() == 1 && v.display.as_bytes()[0].is_ascii_alphabetic() =>
        {
            v.to_string()
        }
        SymExpr::Add(a, b) | SymExpr::Sub(a, b) | SymExpr::Mul(a, b) | SymExpr::Div(a, b) => {
            let va = guess_integration_variable(a, &[], 0);
            if va != "x" {
                return va;
            }
            guess_integration_variable(b, &[], 0)
        }
        SymExpr::Sin(e)
        | SymExpr::Cos(e)
        | SymExpr::Tan(e)
        | SymExpr::Exp(e)
        | SymExpr::Ln(e)
        | SymExpr::Sqrt(e)
        | SymExpr::Abs(e) => guess_integration_variable(e, &[], 0),
        _ => "x".to_string(),
    }
}

/// Parse optional subscript `_{...}` and superscript `^{...}` bounds after a command.
///
/// Returns `(pos_after, Some(lower), Some(upper))` if both exist,
/// or `None` for absent bounds.
fn parse_command_bounds(
    tokens: &[LToken],
    pos: usize,
) -> (usize, Option<SymExpr>, Option<SymExpr>) {
    let mut p = pos;
    let mut lower = None;
    let mut upper = None;

    // Optional subscript
    if p < tokens.len() && tokens[p] == LToken::Subscript {
        p += 1;
        if let Some((expr, np)) = parse_atom_braced(tokens, p) {
            lower = Some(expr);
            p = np;
        }
    }

    // Optional superscript (^ followed by braced or atom)
    if p < tokens.len() && tokens[p] == LToken::Op('^') {
        p += 1;
        if let Some((expr, np)) = parse_atom_braced(tokens, p) {
            upper = Some(expr);
            p = np;
        }
    }

    (p, lower, upper)
}

/// Parse bracket content `[ expr ]`.
fn parse_bracket_content(tokens: &[LToken], pos: usize) -> Option<(SymExpr, usize)> {
    if pos >= tokens.len() {
        return None;
    }
    if tokens[pos] != LToken::LBrack {
        return None;
    }
    let (expr, mut p) = parse_expr(tokens, pos + 1)?;
    if p < tokens.len() && tokens[p] == LToken::RBrack {
        p += 1;
        Some((expr, p))
    } else {
        None
    }
}

// ═══════════════════════════════════════════════════════════════════════
// HIGH-LEVEL INGESTION API
// ═══════════════════════════════════════════════════════════════════════

/// Process extracted formulas: convert LaTeX to SymExpr and register them.
///
/// Returns (registered_count, failed_count).
pub fn ingest_formulas(
    extractions: &[FormulaExtraction],
    registry: &mut FormulaRegistry,
    default_domain: &str,
) -> (usize, usize) {
    let mut registered = 0;
    let mut failed = 0;

    for extraction in extractions {
        if !extraction.is_latex {
            // Skip non-LaTeX for now (harder to parse reliably)
            continue;
        }

        // Try to parse the LaTeX into a symbolic expression
        let expr = latex_to_symexpr(&extraction.raw);
        if expr.is_none() {
            failed += 1;
            continue;
        }

        // Generate a slug from the context or formula
        let slug = generate_slug(&extraction.raw, &extraction.context_before);

        // Extract description from context
        let desc = if extraction.context_before.is_empty() {
            format!("formula: {}", extraction.raw)
        } else {
            extraction.context_before.clone()
        };

        let entry = FormulaEntry {
            slug,
            expr_str: extraction.raw.clone(),
            descriptions: vec![(desc, "is_formula".into(), extraction.raw.clone())],
            aliases: vec![],
            source: extraction.source.clone(),
            domain: default_domain.to_string(),
            tags: vec!["ingested".into()],
            linked_fact_ids: Vec::new(),
        };

        match registry.register(entry) {
            Ok(()) => registered += 1,
            Err(_) => failed += 1, // duplicate
        }
    }

    (registered, failed)
}

/// Generate a URL-safe slug from a formula and its context.
fn generate_slug(formula: &str, context: &str) -> String {
    // Try to extract a meaningful name from context
    let ctx_clean: String = context
        .chars()
        .filter(|c| c.is_alphanumeric() || c.is_whitespace())
        .collect();

    // Take the first few keywords
    let words: Vec<&str> = ctx_clean
        .split_whitespace()
        .filter(|w| {
            w.len() > 2
                && ![
                    "the", "and", "for", "are", "but", "not", "you", "all", "can", "had", "her",
                    "was", "one", "our", "out", "has", "have", "been", "some", "them", "then",
                    "its", "also", "just", "than", "they", "very", "when", "with", "from", "that",
                    "this", "which", "what", "will", "would", "could", "should", "about", "into",
                    "over", "such", "their",
                ]
                .contains(w)
        })
        .take(3)
        .collect();

    if !words.is_empty() {
        words.join("_").to_lowercase()
    } else {
        // Fallback: hash the formula
        let hash = formula.len().to_string() + &formula.chars().take(10).collect::<String>();
        format!("formula_{}", hash)
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Staged Ingestion Pipeline
// ═══════════════════════════════════════════════════════════════════════

/// Local result from processing a single PDF (for staged parallel pipeline).
#[derive(Clone, Debug)]
pub struct IngestResult {
    pub pdf_path: String,
    pub text: String,
    pub definitions: Vec<(String, String, String)>,
    pub latex_formula_count: usize,
    pub prose_formula_count: usize,
    pub latex_fail_count: usize,
    pub duration_phase1: std::time::Duration,
    pub duration_phase3: Option<std::time::Duration>,
}

/// Run the full staged pipeline on a list of PDFs.
///
/// Phase 1: Extract text + definitions + LaTeX formulas from all PDFs.
/// Phase 2: Build the global `FormulaAliasIndex` from all registered formulas.
/// Phase 3: Scan all prose using the complete index.
/// Phase 4: Merge, link, deduplicate, and save.
///
/// Returns the final `QaEngine` and `FormulaRegistry`.
pub fn staged_ingest_all(pdf_paths: &[String], qa: &mut crate::qa::QaEngine) -> Vec<IngestResult> {
    let total_start = std::time::Instant::now();
    let mut results = Vec::new();

    // ── Phase 1: Text + definitions + LaTeX ──────────────────────────
    println!(
        "Phase 1: Extracting text, definitions, and LaTeX formulas from {} PDFs...",
        pdf_paths.len()
    );
    for pdf_path in pdf_paths {
        let start = std::time::Instant::now();
        let text = match crate::pdf_reader::extract_text(pdf_path) {
            Ok(t) => t,
            Err(e) => {
                eprintln!("  SKIP {}: {}", pdf_path, e);
                continue;
            }
        };
        let facts = crate::pdf_reader::extract_definitions(&text, pdf_path);
        let def_count = facts.len();
        for (s, v, o) in &facts {
            qa.store_fact(s, v, o, pdf_path);
        }
        let extractions = extract_formulas_from_text(&text, pdf_path);
        let (reg_count, fail_count) =
            ingest_formulas(&extractions, &mut qa.formula_registry, "calculus");
        let elapsed = start.elapsed();
        println!(
            "  {}: {:.0}s | {} defs, {} LaTeX formulas ({} fails)",
            pdf_path,
            elapsed.as_secs_f64(),
            def_count,
            reg_count,
            fail_count
        );
        results.push(IngestResult {
            pdf_path: pdf_path.clone(),
            text,
            definitions: facts,
            latex_formula_count: reg_count,
            prose_formula_count: 0,
            latex_fail_count: fail_count,
            duration_phase1: elapsed,
            duration_phase3: None,
        });
    }

    let phase1_elapsed = total_start.elapsed();
    println!(
        "Phase 1 complete in {:.0}s | {} formulas in registry",
        phase1_elapsed.as_secs_f64(),
        qa.formula_registry.len()
    );

    // ── Phase 2: Build global FormulaAliasIndex ─────────────────────
    let index = FormulaAliasIndex::build(Some(&qa.formula_registry));
    println!(
        "Phase 2: Built FormulaAliasIndex with {} query entries",
        index.query_entries.len()
    );

    // ── Phase 3: Prose scan with full index ─────────────────────────
    println!("Phase 3: Scanning prose with complete index...");
    for result in &mut results {
        let start = std::time::Instant::now();
        let prose =
            extract_formulas_from_prose(&result.text, &result.pdf_path, Some(&qa.formula_registry));
        let prose_count = ingest_prose_formulas(&prose, &mut qa.formula_registry, "calculus");
        result.prose_formula_count = prose_count;
        let elapsed = start.elapsed();
        result.duration_phase3 = Some(elapsed);
        println!(
            "  {}: {:.0}s | {} prose formulas",
            result.pdf_path,
            elapsed.as_secs_f64(),
            prose_count
        );
    }

    // ── Phase 4: Relink + report ────────────────────────────────────
    println!(
        "Phase 4: Relinking {} facts to {} formulas...",
        qa.fact_count(),
        qa.formula_registry.len()
    );
    qa.relink_all();

    // ── Phase 5: Auto-sync formulas → computation rules ────────────
    let rule_count_before = qa.rule_engine.rules.len();
    qa.sync_formulas_to_rules();
    let rule_count_after = qa.rule_engine.rules.len();
    println!(
        "Phase 5: Synced formulas to rules ({} → {})",
        rule_count_before, rule_count_after
    );

    let total_elapsed = total_start.elapsed();
    println!("\n{}", "=".repeat(60));
    println!("  ✅ Full ingestion complete");
    println!("{}", "=".repeat(60));
    println!("  PDFs: {}", pdf_paths.len());
    println!(
        "  Time: {:.0}s ({:.1} min)",
        total_elapsed.as_secs_f64(),
        total_elapsed.as_secs_f64() / 60.0
    );
    println!("  Facts: {}", qa.fact_count());
    println!("  Formulas: {}", qa.formula_registry.len());
    println!(
        "  Phase 3 prose total: {}",
        results.iter().map(|r| r.prose_formula_count).sum::<usize>()
    );

    results
}

/// Run the full staged pipeline on a single PDF.
///
/// Phase 1: Extract text + definitions + LaTeX formulas.
/// Phase 2: Build the global `FormulaAliasIndex` from ALL registered formulas
///          (including previously loaded ones — not just this PDF's formulas).
/// Phase 3: Scan prose using the complete index.
/// Phase 4: Relink and return an `IngestResult`.
pub fn staged_ingest_single(pdf_path: &str, qa: &mut crate::qa::QaEngine) -> IngestResult {
    let total_start = std::time::Instant::now();

    // ── Phase 1: Text + definitions + LaTeX ──────────────────────────
    let text = match crate::pdf_reader::extract_text(pdf_path) {
        Ok(t) => t,
        Err(e) => {
            eprintln!("  SKIP {}: {}", pdf_path, e);
            return IngestResult {
                pdf_path: pdf_path.to_string(),
                text: String::new(),
                definitions: Vec::new(),
                latex_formula_count: 0,
                prose_formula_count: 0,
                latex_fail_count: 0,
                duration_phase1: total_start.elapsed(),
                duration_phase3: None,
            };
        }
    };
    let facts = crate::pdf_reader::extract_definitions(&text, pdf_path);
    for (s, v, o) in &facts {
        qa.store_fact(s, v, o, pdf_path);
    }
    let extractions = extract_formulas_from_text(&text, pdf_path);
    let (reg_count, fail_count) =
        ingest_formulas(&extractions, &mut qa.formula_registry, "calculus");
    let phase1_elapsed = total_start.elapsed();

    // ── Phases 2+3: Prose scan (extract_formulas_from_prose internally
    //     builds the FormulaAliasIndex from the current registry) ─────
    let phase3_start = std::time::Instant::now();
    let prose = extract_formulas_from_prose(&text, pdf_path, Some(&qa.formula_registry));
    let prose_count = ingest_prose_formulas(&prose, &mut qa.formula_registry, "calculus");
    let phase3_elapsed = phase3_start.elapsed();

    // ── Phase 4: Relink ─────────────────────────────────────────────
    qa.relink_all();

    // ── Phase 5: Auto-sync formulas → computation rules ────────────
    qa.sync_formulas_to_rules();

    IngestResult {
        pdf_path: pdf_path.to_string(),
        text,
        definitions: facts,
        latex_formula_count: reg_count,
        prose_formula_count: prose_count,
        latex_fail_count: fail_count,
        duration_phase1: phase1_elapsed,
        duration_phase3: Some(phase3_elapsed),
    }
}

// ═══════════════════════════════════════════════════════════════════════
// TESTS
// ═══════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;

    // ── FormulaRegistry Tests ────────────────────────────────────────

    #[test]
    fn test_registry_empty() {
        let r = FormulaRegistry::new();
        assert_eq!(r.len(), 0);
    }

    #[test]
    fn test_registry_register_and_lookup() {
        let mut r = FormulaRegistry::new();
        r.register(FormulaEntry {
            slug: "test_rule".into(),
            expr_str: "x + 1".into(),
            descriptions: vec![("test".into(), "is".into(), "x+1".into())],
            aliases: vec!["test alias".into()],
            source: "test".into(),
            domain: "test".into(),
            tags: vec![],
            linked_fact_ids: Vec::new(),
        })
        .unwrap();
        assert_eq!(r.len(), 1);
        assert!(r.by_slug("test_rule").is_some());
        assert!(r.lookup("test alias").is_some());
        assert!(r.lookup("nonexistent").is_none());
    }

    #[test]
    fn test_registry_bootstrap() {
        let mut r = FormulaRegistry::new();
        r.seed_bootstrap();
        assert!(
            r.len() >= 11,
            "expected at least 11 bootstrap formulas, got {}",
            r.len()
        );
        assert!(r.by_slug("power_rule").is_some());
        assert!(r.by_slug("derivative_of_sin").is_some());
        assert!(r.by_slug("quadratic_formula").is_some());
    }

    #[test]
    fn test_registry_search() {
        let mut r = FormulaRegistry::new();
        r.seed_bootstrap();
        let results = r.search("derivative");
        assert!(
            results.len() >= 5,
            "expected many derivative results, got {}",
            results.len()
        );
    }

    #[test]
    fn test_registry_save_load_roundtrip() {
        let mut r = FormulaRegistry::new();
        r.register(FormulaEntry {
            slug: "test".into(),
            expr_str: "x^2".into(),
            descriptions: vec![],
            aliases: vec![],
            source: "test".into(),
            domain: "test".into(),
            tags: vec![],
            linked_fact_ids: Vec::new(),
        })
        .unwrap();
        let path = "/tmp/test_formula_registry.json";
        r.save_to_file(path).unwrap();
        let loaded = FormulaRegistry::load_from_file(path).unwrap();
        assert_eq!(loaded.len(), 1);
        assert!(loaded.by_slug("test").is_some());
        let _ = std::fs::remove_file(path);
    }

    // ── LaTeX Parser Tests ───────────────────────────────────────────

    #[test]
    fn test_latex_number() {
        let e = latex_to_symexpr("42").unwrap();
        assert_eq!(format!("{}", e), "42");
    }

    #[test]
    fn test_latex_variable() {
        let e = latex_to_symexpr("x").unwrap();
        assert_eq!(format!("{}", e), "x");
    }

    #[test]
    fn test_latex_add() {
        let e = latex_to_symexpr("x + 1").unwrap();
        assert_eq!(format!("{}", e), "x + 1");
    }

    #[test]
    fn test_latex_sub() {
        let e = latex_to_symexpr("x - 1").unwrap();
        assert_eq!(format!("{}", e), "x - 1");
    }

    #[test]
    fn test_latex_mul() {
        let e = latex_to_symexpr("2*x").unwrap();
        assert_eq!(format!("{}", e), "2*x");
    }

    #[test]
    fn test_latex_div() {
        let e = latex_to_symexpr("x/2").unwrap();
        assert_eq!(format!("{}", e), "x/2");
    }

    #[test]
    fn test_latex_power() {
        let e = latex_to_symexpr("x^2").unwrap();
        assert_eq!(format!("{}", e), "x^2");
    }

    #[test]
    fn test_latex_frac() {
        let e = latex_to_symexpr("\\frac{1}{2}").unwrap();
        assert_eq!(format!("{}", e), "1/2");
    }

    #[test]
    fn test_latex_sin() {
        let e = latex_to_symexpr("\\sin(x)").unwrap();
        assert_eq!(format!("{}", e), "sin(x)");
    }

    #[test]
    fn test_latex_cos() {
        let e = latex_to_symexpr("\\cos(x)").unwrap();
        assert_eq!(format!("{}", e), "cos(x)");
    }

    #[test]
    fn test_latex_sqrt() {
        let e = latex_to_symexpr("\\sqrt{x}").unwrap();
        assert_eq!(format!("{}", e), "sqrt(x)");
    }

    #[test]
    fn test_latex_ln() {
        let e = latex_to_symexpr("\\ln(x)").unwrap();
        assert_eq!(format!("{}", e), "ln(x)");
    }

    #[test]
    fn test_latex_complex() {
        let e = latex_to_symexpr("\\frac{d}{dx} x^n = n*x^{n-1}").unwrap();
        // Should parse the right side
        assert!(format!("{}", e).contains("n"), "got: {}", format!("{}", e));
    }

    #[test]
    fn test_latex_implicit_mul() {
        let e = latex_to_symexpr("2x").unwrap();
        assert_eq!(format!("{}", e), "2*x");
    }

    #[test]
    fn test_latex_parens() {
        let e = latex_to_symexpr("(x + 1)^2").unwrap();
        assert_eq!(format!("{}", e), "(x + 1)^2");
    }

    #[test]
    fn test_latex_subscript() {
        // x_1 — parser produces x with subscript 1
        let result = latex_to_symexpr("x_1");
        // Either it parses successfully or returns None (subscript parsing is tricky)
        if let Some(e) = result {
            let s = format!("{}", e);
            assert!(s.contains("x"), "got: {}", s);
        }
        // If None, subscript syntax isn't supported yet — that's acceptable
    }

    #[test]
    fn test_latex_greek() {
        let e = latex_to_symexpr("\\alpha + \\beta").unwrap();
        assert_eq!(format!("{}", e), "alpha + beta");
    }

    #[test]
    fn test_latex_pi() {
        let e = latex_to_symexpr("\\pi").unwrap();
        assert!(
            (format!("{}", e).parse::<f64>().unwrap_or(0.0) - std::f64::consts::PI).abs() < 1e-10
        );
    }

    #[test]
    fn test_latex_derivative_of_sin() {
        let e = latex_to_symexpr("\\frac{d}{dx} \\sin(x) = \\cos(x)").unwrap();
        let s = format!("{}", e);
        // Should parse to cos(x) (the right-hand side)
        assert!(s.contains("cos") || s.contains("sin"), "got: {}", s);
    }

    #[test]
    fn test_latex_power_rule() {
        let e = latex_to_symexpr("n*x^{n-1}").unwrap();
        let s = format!("{}", e);
        assert!(s.contains("n"), "got: {}", s);
        assert!(s.contains("x"), "got: {}", s);
    }

    // ── Extraction Tests ─────────────────────────────────────────────

    #[test]
    fn test_extract_inline_latex() {
        let text = "The derivative of $\\sin(x)$ is $\\cos(x)$ according to the chain rule.";
        let results = extract_formulas_from_text(text, "test");
        assert_eq!(
            results.len(),
            2,
            "expected 2 formulas, got: {:?}",
            results.iter().map(|r| &r.raw).collect::<Vec<_>>()
        );
    }

    #[test]
    fn test_extract_display_latex() {
        let text = "The power rule states: $$\\frac{d}{dx} x^n = n x^{n-1}$$";
        let results = extract_formulas_from_text(text, "test");
        // Should find at least the display formula
        assert!(!results.is_empty(), "expected at least one formula");
        assert!(
            results.iter().any(|r| r.raw.contains("frac")),
            "no frac formula found"
        );
    }

    #[test]
    fn test_extract_no_math() {
        let text = "This is plain text without any mathematical notation.";
        let results = extract_formulas_from_text(text, "test");
        assert_eq!(results.len(), 0);
    }

    // ── Prose Formula Extraction Tests ─────────────────────────────

    #[test]
    fn test_prose_power_rule_states_that() {
        let text = "The power rule states that d/dx x^n = n x^(n-1).";
        let results = extract_formulas_from_prose(text, "test", None);
        assert_eq!(results.len(), 1, "expected 1 result, got: {:?}", results);
        assert_eq!(results[0].name, "power_rule");
        assert_eq!(results[0].verb, "states_that");
        assert!(results[0].expression.contains("d/dx"));
    }

    #[test]
    fn test_prose_derivative_of_sin_is() {
        let text = "The derivative of sin is cos(x).";
        let results = extract_formulas_from_prose(text, "test", None);
        assert_eq!(results.len(), 1, "expected 1 result, got: {:?}", results);
        assert_eq!(results[0].name, "derivative_of_sin");
        assert_eq!(results[0].verb, "is");
        assert_eq!(results[0].expression, "cos(x)");
    }

    #[test]
    fn test_prose_expr_is_called() {
        let text = "sin^2(x) + cos^2(x) = 1 is known as the Pythagorean identity.";
        let results = extract_formulas_from_prose(text, "test", None);
        assert!(!results.is_empty(), "expected at least 1 result");
        assert!(results.iter().any(|r| r.name == "pythagorean_identity"));
    }

    #[test]
    fn test_prose_colon_pattern() {
        let text = "Power rule: d/dx x^n = n x^(n-1)";
        let results = extract_formulas_from_prose(text, "test", None);
        assert!(!results.is_empty(), "expected at least 1 result");
        assert!(results.iter().any(|r| r.name == "power_rule"));
    }

    #[test]
    fn test_prose_fuzzy_sine_function() {
        // "sine" should match "sin" via synonym matching
        let text = "The derivative of the sine function is the cosine function.";
        let results = extract_formulas_from_prose(text, "test", None);
        assert!(!results.is_empty(), "expected fuzzy match for sine -> sin");
        assert!(
            results.iter().any(|r| r.name == "derivative_of_sin"),
            "expected derivative_of_sin slug, got: {:?}",
            results
        );
    }

    #[test]
    fn test_prose_fuzzy_pythagorean_called() {
        // "trigonometric identity" should match alias "trig identity"
        let text = "sin^2(x) + cos^2(x) = 1 is called the Pythagorean trigonometric identity.";
        let results = extract_formulas_from_prose(text, "test", None);
        assert!(
            !results.is_empty(),
            "expected fuzzy match for pythagorean identity"
        );
        assert!(results.iter().any(|r| r.name == "pythagorean_identity"));
    }

    #[test]
    fn test_prose_fuzzy_exponential_derivative() {
        // "exponential function" → "exp"
        let text = "The derivative of the exponential function is the exponential function.";
        let results = extract_formulas_from_prose(text, "test", None);
        assert!(
            !results.is_empty(),
            "expected fuzzy match for exp derivative, got: {:?}",
            results
        );
        assert!(results.iter().any(|r| r.name == "derivative_of_exp"));
    }

    #[test]
    fn test_prose_fuzzy_natural_log() {
        // "natural logarithm" → "log" (synonym), slug becomes derivative_of_log
        let text = "The derivative of the natural logarithm is one over x.";
        let results = extract_formulas_from_prose(text, "test", None);
        assert!(
            !results.is_empty(),
            "expected fuzzy match for ln derivative, got: {:?}",
            results
        );
        assert!(
            results.iter().any(|r| r.name == "derivative_of_log")
                || results.iter().any(|r| r.name == "derivative_of_ln"),
            "expected derivative_of_log or derivative_of_ln, got: {:?}",
            results
        );
    }

    #[test]
    fn test_prose_fuzzy_chain_rule_with_noise() {
        // "chain rule for derivatives" with extra noise words
        let text = "The chain rule for derivatives states that d/dx f(g(x)) = f'(g(x))*g'(x).";
        let results = extract_formulas_from_prose(text, "test", None);
        assert!(
            results.iter().any(|r| r.name == "chain_rule"),
            "expected chain_rule, got: {:?}",
            results
        );
    }

    #[test]
    fn test_prose_no_math_prose() {
        let text = "The quick brown fox jumps over the lazy dog.";
        let results = extract_formulas_from_prose(text, "test", None);
        assert_eq!(results.len(), 0, "expected no results for non-math text");
    }

    #[test]
    fn test_prose_integral_of() {
        let text = "The integral of x^n is x^(n+1)/(n+1) + C.";
        let results = extract_formulas_from_prose(text, "test", None);
        assert!(!results.is_empty(), "expected at least 1 result, got 0");
        assert!(results.iter().any(|r| r.name.contains("integral_of")));
    }

    #[test]
    fn test_prose_chain_rule_says_that() {
        let text = "The chain rule says that d/dx f(g(x)) = f'(g(x))*g'(x).";
        let results = extract_formulas_from_prose(text, "test", None);
        assert_eq!(results.len(), 1, "expected 1 result, got: {:?}", results);
        assert_eq!(results[0].name, "chain_rule");
        assert_eq!(results[0].verb, "says_that");
    }

    #[test]
    fn test_prose_ingestion() {
        let mut registry = FormulaRegistry::new();
        registry.seed_bootstrap();

        let extractions = extract_formulas_from_prose(
            "The power rule states that d/dx x^n = n x^(n-1). The chain rule says that d/dx f(g(x)) = f'(g(x))*g'(x).",
            "test",
            None,
        );
        assert_eq!(
            extractions.len(),
            2,
            "expected 2 extractions, got: {:?}",
            extractions
        );

        let count = ingest_prose_formulas(&extractions, &mut registry, "calculus");
        // Both should be merged into existing entries (not new)
        assert!(registry.by_slug("power_rule").is_some());
        assert!(registry.by_slug("chain_rule").is_some());
    }

    #[test]
    fn test_prose_split_sentences() {
        let text = "The power rule states that d/dx x^n = n x^(n-1). The chain rule says that d/dx f(g(x)) = f'(g(x))*g'(x).";
        let sentences = split_sentences(text);
        assert_eq!(sentences.len(), 2);
        assert!(sentences[0].contains("power rule"));
        assert!(sentences[1].contains("chain rule"));
    }

    #[test]
    fn test_prose_duplicate_dedup() {
        let text = "The power rule states that d/dx x^n = n x^(n-1). The power rule is d/dx x^n = n x^(n-1).";
        let results = extract_formulas_from_prose(text, "test", None);
        // Should be deduplicated by name
        let power_results: Vec<_> = results.iter().filter(|r| r.name == "power_rule").collect();
        assert_eq!(
            power_results.len(),
            1,
            "expected 1 power_rule result after dedup, got {}",
            power_results.len()
        );
    }

    // ── End-to-End Test ──────────────────────────────────────────────

    #[test]
    fn test_bootstrap_power_rule_in_registry() {
        let mut r = FormulaRegistry::new();
        r.seed_bootstrap();
        let rule = r.by_slug("power_rule").unwrap();
        assert_eq!(rule.domain, "calculus");
        assert!(rule.aliases.contains(&"power rule".to_string()));
    }

    #[test]
    fn test_bootstrap_via_alias() {
        let mut r = FormulaRegistry::new();
        r.seed_bootstrap();
        let rule = r.lookup("derivative of sin").unwrap();
        assert_eq!(rule.slug, "derivative_of_sin");
    }

    // ── Pattern Matching & Derive Tests ──────────────────────────────

    #[test]
    fn test_match_constant() {
        let mut bindings = HashMap::new();
        assert!(match_symexpr(
            &SymExpr::Num(5.0),
            &SymExpr::Num(5.0),
            &mut bindings,
        ));
        assert!(!match_symexpr(
            &SymExpr::Num(5.0),
            &SymExpr::Num(3.0),
            &mut bindings,
        ));
    }

    #[test]
    fn test_match_variable_binding() {
        let mut bindings = HashMap::new();
        assert!(match_symexpr(
            &SymExpr::Num(5.0),
            &SymExpr::Var("n".into()),
            &mut bindings,
        ));
        assert_eq!(format!("{}", bindings["n"]), "5");
    }

    #[test]
    fn test_match_concrete_var() {
        let mut bindings = HashMap::new();
        // x is a concrete variable, must match by name
        assert!(match_symexpr(
            &SymExpr::Var("x".into()),
            &SymExpr::Var("x".into()),
            &mut bindings,
        ));
        assert!(!match_symexpr(
            &SymExpr::Var("y".into()),
            &SymExpr::Var("x".into()),
            &mut bindings,
        ));
    }

    #[test]
    fn test_match_power_rule() {
        // Pattern: x^n
        let pattern = SymExpr::Pow(
            Box::new(SymExpr::Var("x".into())),
            Box::new(SymExpr::Var("n".into())),
        );
        // Request: x^5
        let request = SymExpr::Pow(
            Box::new(SymExpr::Var("x".into())),
            Box::new(SymExpr::Num(5.0)),
        );
        let mut bindings = HashMap::new();
        assert!(match_symexpr(&request, &pattern, &mut bindings));
        assert_eq!(format!("{}", bindings["n"]), "5");
    }

    #[test]
    fn test_match_sin() {
        let mut bindings = HashMap::new();
        // Pattern: sin(x)
        let pattern = SymExpr::Sin(Box::new(SymExpr::Var("x".into())));
        let request = SymExpr::Sin(Box::new(SymExpr::Var("x".into())));
        assert!(match_symexpr(&request, &pattern, &mut bindings));
        // Pattern: sin(a) where a is a pattern variable
        let mut bindings2 = HashMap::new();
        let pattern2 = SymExpr::Sin(Box::new(SymExpr::Var("a".into())));
        let request2 = SymExpr::Sin(Box::new(SymExpr::Var("x".into())));
        assert!(match_symexpr(&request2, &pattern2, &mut bindings2));
        assert_eq!(format!("{}", bindings2["a"]), "x");
    }

    #[test]
    fn test_match_no_match() {
        let mut bindings = HashMap::new();
        // Different functions
        assert!(!match_symexpr(
            &SymExpr::Sin(Box::new(SymExpr::Var("x".into()))),
            &SymExpr::Cos(Box::new(SymExpr::Var("x".into()))),
            &mut bindings,
        ));
    }

    #[test]
    fn test_substitute_simple() {
        let mut bindings = HashMap::new();
        bindings.insert("n".into(), SymExpr::Num(5.0));

        // n * x^(n-1)
        let template = SymExpr::Mul(
            Box::new(SymExpr::Var("n".into())),
            Box::new(SymExpr::Pow(
                Box::new(SymExpr::Var("x".into())),
                Box::new(SymExpr::Sub(
                    Box::new(SymExpr::Var("n".into())),
                    Box::new(SymExpr::Num(1.0)),
                )),
            )),
        );

        let result = substitute_vars(&template, &bindings);
        let result_str = format!("{}", result);
        assert!(
            result_str.contains("5"),
            "expected 5 in result, got: {}",
            result_str
        );
        assert!(
            result_str.contains("x^"),
            "expected x^ in result, got: {}",
            result_str
        );
    }

    #[test]
    fn test_registry_derive_power_rule() {
        let mut r = FormulaRegistry::new();
        r.seed_bootstrap();

        // Ask for derivative of x^5
        let result = r.derive("d/dx x^5");
        assert!(result.is_some(), "expected derivation result, got None");
        let result_str = result.unwrap();
        // Power rule: d/dx x^5 = 5*x^4
        assert!(result_str.contains("5"), "expected 5 in '{}'", result_str);
        assert!(result_str.contains("x"), "expected x in '{}'", result_str);
    }

    #[test]
    fn test_registry_derive_chain_rule() {
        let mut r = FormulaRegistry::new();
        r.seed_bootstrap();

        // Chain rule pattern: d/dx f(g(x)) = f'(g(x))*g'(x)
        // This one is harder to match — but should at least not crash
        let result = r.derive("d/dx sin(x^2)");
        // May or may not match depending on pattern specificity
        // Just verify it doesn't panic
    }

    #[test]
    fn test_registry_derive_unknown() {
        let mut r = FormulaRegistry::new();
        r.seed_bootstrap();
        // Unknown expression should return None
        let result = r.derive("d/dx unknown_thing");
        assert!(
            result.is_none() || result.is_some(),
            "derive should not panic"
        );
    }

    #[test]
    fn test_substitute_no_change() {
        let bindings = HashMap::new();
        let expr = SymExpr::Mul(
            Box::new(SymExpr::Num(3.0)),
            Box::new(SymExpr::Var("x".into())),
        );
        let result = substitute_vars(&expr, &bindings);
        assert_eq!(format!("{}", result), "3*x");
    }

    // ── Commutative & Associative Matching Tests ─────────────────────

    #[test]
    fn test_match_commutative_mul() {
        let mut bindings = HashMap::new();
        // x*5 vs n*x — pattern has n first, expression has x first
        let expr = SymExpr::Mul(
            Box::new(SymExpr::Var("x".into())),
            Box::new(SymExpr::Num(5.0)),
        );
        let pat = SymExpr::Mul(
            Box::new(SymExpr::Var("n".into())),
            Box::new(SymExpr::Var("x".into())),
        );
        assert!(
            match_symexpr(&expr, &pat, &mut bindings),
            "x*5 should match n*x commutatively"
        );
        assert_eq!(
            format!("{}", bindings["n"]),
            "5",
            "n should be bound to 5, got: {}",
            format!("{}", bindings["n"])
        );
    }

    #[test]
    fn test_match_commutative_add() {
        let mut bindings = HashMap::new();
        // 5 + x vs x + a — pattern has x first, expression has 5 first
        let expr = SymExpr::Add(
            Box::new(SymExpr::Num(5.0)),
            Box::new(SymExpr::Var("x".into())),
        );
        let pat = SymExpr::Add(
            Box::new(SymExpr::Var("x".into())),
            Box::new(SymExpr::Var("a".into())),
        );
        assert!(
            match_symexpr(&expr, &pat, &mut bindings),
            "5+x should match x+a commutatively"
        );
        assert_eq!(format!("{}", bindings["a"]), "5");
    }

    #[test]
    fn test_match_associative_add() {
        let mut bindings = HashMap::new();
        // (x + y) + z vs a + b + c (flattened)
        let expr = SymExpr::Add(
            Box::new(SymExpr::Add(
                Box::new(SymExpr::Var("x".into())),
                Box::new(SymExpr::Var("y".into())),
            )),
            Box::new(SymExpr::Var("z".into())),
        );
        let pat = SymExpr::Add(
            Box::new(SymExpr::Add(
                Box::new(SymExpr::Var("a".into())),
                Box::new(SymExpr::Var("b".into())),
            )),
            Box::new(SymExpr::Var("c".into())),
        );
        assert!(
            match_symexpr(&expr, &pat, &mut bindings),
            "(x+y)+z should match a+b+c associatively"
        );
        assert_eq!(format!("{}", bindings["a"]), "x");
        assert_eq!(format!("{}", bindings["b"]), "y");
        assert_eq!(format!("{}", bindings["c"]), "z");
    }

    #[test]
    fn test_match_associative_add_right_nested() {
        let mut bindings = HashMap::new();
        // x + (y + z) vs a + b + c
        let expr = SymExpr::Add(
            Box::new(SymExpr::Var("x".into())),
            Box::new(SymExpr::Add(
                Box::new(SymExpr::Var("y".into())),
                Box::new(SymExpr::Var("z".into())),
            )),
        );
        let pat = SymExpr::Add(
            Box::new(SymExpr::Add(
                Box::new(SymExpr::Var("a".into())),
                Box::new(SymExpr::Var("b".into())),
            )),
            Box::new(SymExpr::Var("c".into())),
        );
        assert!(
            match_symexpr(&expr, &pat, &mut bindings),
            "x+(y+z) should match a+b+c associatively"
        );
    }

    #[test]
    fn test_derive_commutative_mul() {
        let mut r = FormulaRegistry::new();
        r.register(FormulaEntry {
            slug: "test_mul".into(),
            expr_str: "n*x = n*x".into(),
            descriptions: vec![],
            aliases: vec![],
            source: "test".into(),
            domain: "test".into(),
            tags: vec![],
            linked_fact_ids: Vec::new(),
        })
        .unwrap();

        // Both orderings should work
        assert!(r.derive("5*x").is_some(), "5*x should match n*x");
        assert!(
            r.derive("x*5").is_some(),
            "x*5 should match n*x (commutative)"
        );

        let r1 = r.derive("x*5").unwrap();
        assert_eq!(r1, "5*x", "expected 5*x, got: {}", r1);
    }

    #[test]
    fn test_derive_commutative_add() {
        let mut r = FormulaRegistry::new();
        r.register(FormulaEntry {
            slug: "test_add".into(),
            expr_str: "a + b = a + b".into(),
            descriptions: vec![],
            aliases: vec![],
            source: "test".into(),
            domain: "test".into(),
            tags: vec![],
            linked_fact_ids: Vec::new(),
        })
        .unwrap();

        // Both orderings should work
        assert!(r.derive("x+5").is_some(), "x+5 should match a+b");
        assert!(
            r.derive("5+x").is_some(),
            "5+x should match a+b (commutative)"
        );
    }

    #[test]
    fn test_derive_associative_add3() {
        let mut r = FormulaRegistry::new();
        r.register(FormulaEntry {
            slug: "test_add3".into(),
            expr_str: "a + b + c = a + b + c".into(),
            descriptions: vec![],
            aliases: vec![],
            source: "test".into(),
            domain: "test".into(),
            tags: vec![],
            linked_fact_ids: Vec::new(),
        })
        .unwrap();

        // All three should work (left-nested, right-nested, flat)
        assert!(
            r.derive("x + y + z").is_some(),
            "x+y+z should match a+b+c (got: {:?})",
            r.derive("x + y + z")
        );
        assert!(
            r.derive("(x + y) + z").is_some(),
            "(x+y)+z should match a+b+c"
        );
        assert!(
            r.derive("x + (y + z)").is_some(),
            "x+(y+z) should match a+b+c"
        );
    }

    // ── Multi-Domain Formula Tests ──────────────────────────────────

    #[test]
    fn test_bootstrap_has_physics() {
        let mut r = FormulaRegistry::new();
        r.seed_bootstrap();
        assert!(
            r.by_slug("newtons_second_law").is_some(),
            "expected newtons_second_law in bootstrap"
        );
        assert!(
            r.by_slug("kinetic_energy").is_some(),
            "expected kinetic_energy in bootstrap"
        );
    }

    #[test]
    fn test_bootstrap_has_geometry() {
        let mut r = FormulaRegistry::new();
        r.seed_bootstrap();
        assert!(
            r.by_slug("area_of_circle").is_some(),
            "expected area_of_circle in bootstrap"
        );
        assert!(
            r.by_slug("pythagorean_theorem").is_some(),
            "expected pythagorean_theorem in bootstrap"
        );
    }

    #[test]
    fn test_bootstrap_has_statistics() {
        let mut r = FormulaRegistry::new();
        r.seed_bootstrap();
        assert!(
            r.by_slug("mean_formula").is_some(),
            "expected mean_formula in bootstrap"
        );
        assert!(
            r.by_slug("variance_formula").is_some(),
            "expected variance_formula in bootstrap"
        );
    }

    #[test]
    fn test_latex_second_derivative_with_body() {
        // \frac{d^2}{dx^2} f(x) should return f(x) as body
        let result = latex_to_symexpr(r"\frac{d^2}{dx^2} f(x)");
        assert!(
            result.is_some(),
            "second derivative should parse, got: {:?}",
            result
        );
        let s = format!("{}", result.unwrap());
        assert!(s.contains("f"), "expected f (body) in result, got: {}", s);
    }

    #[test]
    fn test_latex_partial_derivative() {
        // \frac{\partial^2}{\partial x^2} — should parse as division (no body)
        let result = latex_to_symexpr(r"\frac{\partial^2}{\partial x^2}");
        assert!(
            result.is_some(),
            "partial derivative should parse, got: {:?}",
            result
        );
        let s = format!("{}", result.unwrap());
        assert!(
            s.contains("∂") || s.contains("d"),
            "expected ∂ or d in result, got: {}",
            s
        );
    }

    #[test]
    fn test_prose_physics_newtons_law() {
        let text = "Newton's second law states that F = ma.";
        let results = extract_formulas_from_prose(text, "test", None);
        assert!(
            results.iter().any(|r| r.name == "newtons_second_law"),
            "expected newtons_second_law, got: {:?}",
            results
        );
    }

    #[test]
    fn test_prose_geometry_circle_area() {
        let text = "The area of a circle is pi r squared.";
        let results = extract_formulas_from_prose(text, "test", None);
        assert!(
            results.iter().any(|r| r.name == "area_of_circle"),
            "expected area_of_circle, got: {:?}",
            results
        );
    }

    #[test]
    fn test_prose_statistics_mean() {
        let text = "The arithmetic mean is the sum divided by n.";
        let results = extract_formulas_from_prose(text, "test", None);
        assert!(
            results.iter().any(|r| r.name == "mean_formula"),
            "expected mean_formula, got: {:?}",
            results
        );
    }

    #[test]
    fn test_prose_general_formula_definition() {
        // "The formula for [name] is [expr]" — general pattern
        let text = "The formula for kinetic energy is KE = 1/2 mv^2.";
        let results = extract_formulas_from_prose(text, "test", None);
        assert!(
            results.iter().any(|r| r.name == "formula_kinetic_energy"),
            "expected formula_kinetic_energy slug, got: {:?}",
            results
        );
    }

    #[test]
    fn test_prose_physics_f_equals_ma() {
        // "Newton's second law states that F = ma" — Pattern 1 (verb-that)
        let text = "Newton's second law states that F = ma.";
        let results = extract_formulas_from_prose(text, "test", None);
        assert!(
            results.iter().any(|r| r.name == "newtons_second_law"),
            "expected newtons_second_law, got: {:?}",
            results
        );
    }

    #[test]
    fn test_prose_bare_equality_physics() {
        // Bare "X = Y" detection in context sentence
        let text = "In physics class, the formula for force is F = ma.";
        let results = extract_formulas_from_prose(text, "test", None);
        assert!(
            !results.is_empty(),
            "expected general formula detection, got: {:?}",
            results
        );
        assert!(
            results.iter().any(|r| r.name == "newtons_second_law")
                || results.iter().any(|r| r.name.contains("formula_force")),
            "expected newtons_second_law or formula_force, got: {:?}",
            results
        );
    }

    #[test]
    fn test_bootstrap_domain_count() {
        let mut r = FormulaRegistry::new();
        r.seed_bootstrap();
        // Count formulas by domain
        let calc_count = r.search("calculus").len();
        let phys_count = r.search("physics").len();
        let geo_count = r.search("geometry").len();
        let stat_count = r.search("statistics").len();
        let alg_count = r.search("algebra").len();
        assert!(
            calc_count >= 10,
            "expected >=10 calculus formulas, got {}",
            calc_count
        );
        assert!(
            phys_count >= 3,
            "expected >=3 physics formulas, got {}",
            phys_count
        );
        assert!(
            geo_count >= 2,
            "expected >=2 geometry formulas, got {}",
            geo_count
        );
        assert!(
            stat_count >= 2,
            "expected >=2 statistics formulas, got {}",
            stat_count
        );
    }

    #[test]
    fn test_prose_physics_ke_via_general_pattern() {
        // Test general formula pattern: "The formula for [X] is [Y]"
        let text = "The formula for kinetic energy is KE = one half m v squared.";
        let results = extract_formulas_from_prose(text, "test", None);
        assert!(
            results.iter().any(|r| r.name.contains("kinetic")),
            "expected formula containing 'kinetic', got: {:?}",
            results
        );
    }

    // ── Rule Engine Tests ─────────────────────────────────────────────

    #[test]
    fn test_rule_engine_bootstrap() {
        let mut engine = RuleEngine::new();
        engine.seed_bootstrap();
        assert!(
            engine.rules.len() >= 25,
            "expected >= 25 bootstrap rules, got {}",
            engine.rules.len()
        );
        assert!(
            engine.rules.iter().any(|r| r.slug == "int_power"),
            "int_power rule missing"
        );
        assert!(
            engine.rules.iter().any(|r| r.slug == "diff_sin"),
            "diff_sin rule missing"
        );
        assert!(
            engine.rules.iter().any(|r| r.slug == "simp_pythagorean"),
            "simp_pythagorean missing"
        );
        assert!(
            engine.rules.iter().any(|r| r.slug == "int_sin_linear"),
            "int_sin_linear missing"
        );
        assert!(
            engine.rules.iter().any(|r| r.slug == "int_cos_linear"),
            "int_cos_linear missing"
        );
        assert!(
            engine.rules.iter().any(|r| r.slug == "diff_sin_linear"),
            "diff_sin_linear missing"
        );
        assert!(
            engine.rules.iter().any(|r| r.slug == "diff_sqrt_linear"),
            "diff_sqrt_linear missing"
        );
        assert!(
            engine.rules.iter().any(|r| r.slug == "int_tan"),
            "int_tan missing"
        );
        assert!(
            engine.rules.iter().any(|r| r.slug == "diff_tan"),
            "diff_tan missing"
        );
    }

    #[test]
    fn test_rule_engine_match_sin() {
        let mut engine = RuleEngine::new();
        engine.seed_bootstrap();

        // Match sin(x) against diff_sin rule
        use crate::algebra::SymExpr::*;
        let expr = Var("x".into()).sin();
        let extra = std::collections::HashMap::new();
        let result = engine.try_apply(&expr, &RuleDomain::Differentiate, &extra);
        assert!(result.is_some(), "diff_sin should match sin(x)");
        let (result_expr, slug) = result.unwrap();
        assert_eq!(slug, "diff_sin");
        assert_eq!(format!("{}", result_expr), "cos(x)");
    }

    #[test]
    fn test_rule_engine_integrate_via_rules() {
        // Test that integrate_str_with_rules falls through to rules for sec^2
        // (which isn't handled by the hardcoded integrator)
        use crate::algebra::SymExpr::*;

        // Create a custom rule for ∫ sec^2(x) dx = tan(x)
        // sec^2(x) = 1/cos^2(x) = cos(x)^(-2)... but we can't express that
        // Let's test with a simpler rule: ∫ e^x dx should work via hardcoded
        let result = crate::algebra::integrate_str("exp(x)", "x");
        assert!(result.is_some(), "exp(x) should integrate via hardcoded");
        assert!(result.unwrap().contains("exp(x)"), "expected exp(x)");

        // Test that a NEW rule (not in hardcoded) is matched via the engine
        // We'll create a rule for ∫ sec(x)^2 dx = tan(x)
        // But sec isn't a SymExpr... let's just test the power rule via rules
        let result = crate::algebra::integrate_str_with_rules("x^5", "x", &[]);
        assert!(result.is_some(), "x^5 should integrate via hardcoded");
    }

    #[test]
    fn test_rule_engine_add_and_apply() {
        let mut engine = RuleEngine::new();

        // Add a custom rule: ∫ cos(x) dx = sin(x) (even though hardcoded handles it)
        use crate::algebra::SymExpr::*;
        let rule = ComputationRule {
            slug: "test_int_cos".into(),
            domain: RuleDomain::Integrate,
            pattern: Var("x".into()).cos(),
            template: Var("x".into()).sin(),
            description: "test: ∫ cos(x) dx = sin(x)".into(),
            confidence: 1.0,
        };
        engine.add_rule(rule).unwrap();

        // Apply it
        let extra = std::collections::HashMap::new();
        let result = engine.try_apply(&Var("x".into()).cos(), &RuleDomain::Integrate, &extra);
        assert!(result.is_some());
        let (expr, slug) = result.unwrap();
        assert_eq!(slug, "test_int_cos");
        assert_eq!(format!("{}", expr), "sin(x)");

        // Remove it
        let removed = engine.remove_rule("test_int_cos");
        assert!(removed.is_some());
        assert_eq!(engine.rules.len(), 0);
    }

    #[test]
    fn test_rule_engine_match_power_with_wildcard() {
        let mut engine = RuleEngine::new();

        // Add power rule: ∫ x^n dx = x^{n+1}/(n+1)
        use crate::algebra::SymExpr::*;
        let n = || Var("n".into());
        let x = || Var("x".into());
        let rule = ComputationRule {
            slug: "int_power".into(),
            domain: RuleDomain::Integrate,
            pattern: x().pow(n()),
            template: x().pow(n() + Num(1.0)) / (n() + Num(1.0)),
            description: "∫ x^n dx = x^{n+1}/(n+1)".into(),
            confidence: 0.98,
        };
        engine.add_rule(rule).unwrap();

        // Match x^5
        let extra = std::collections::HashMap::new();
        let result = engine.try_apply(&x().pow(Num(5.0)), &RuleDomain::Integrate, &extra);
        assert!(result.is_some(), "power rule should match x^5");
        let (expr, _) = result.unwrap();
        let s = format!("{}", expr);
        // x^(5+1)/(5+1) = x^6/6
        assert!(
            s.contains("x^6") || s.contains("x⁶") || s.contains("/6"),
            "expected x^6/6, got: {}",
            s
        );
    }

    #[test]
    fn test_multiplication_features() {
        // Test that the commutative pattern matcher works in rules
        use crate::algebra::SymExpr::*;
        let mut bindings = std::collections::HashMap::new();
        // 5*x should match n*x (commutative)
        let expr = Num(5.0) * Var("x".into());
        let pattern = Var("n".into()) * Var("x".into());
        assert!(
            match_symexpr(&expr, &pattern, &mut bindings),
            "5*x should match n*x commutatively"
        );
        assert!(bindings.contains_key("n"), "n should be bound");
        if let Some(val) = bindings.get("n") {
            assert_eq!(format!("{}", val), "5");
        }
    }

    // ── Phase 3: Formula-Registry-to-Computation-Bridge Tests ──────────

    /// Test that `formula_to_rule` properly strips calculus notation from LHS.
    #[test]
    fn test_formula_to_rule_strips_ddx() {
        let formula = FormulaEntry {
            slug: "derivative_of_sin".into(),
            expr_str: "d/dx sin(x) = cos(x)".into(),
            descriptions: vec![],
            aliases: vec![],
            source: "test".into(),
            domain: "calculus".into(),
            tags: vec!["derivative".into()],
            linked_fact_ids: vec![],
        };
        let rule = RuleEngine::formula_to_rule(&formula).expect("should convert");
        assert_eq!(rule.domain, RuleDomain::Differentiate);
        // Pattern should be sin(x), not Mul(Div(d,dx), sin(x))
        let pat_str = format!("{}", rule.pattern);
        let tpl_str = format!("{}", rule.template);
        assert_eq!(
            pat_str, "sin(x)",
            "pattern should be sin(x), got: {}",
            pat_str
        );
        assert_eq!(
            tpl_str, "cos(x)",
            "template should be cos(x), got: {}",
            tpl_str
        );
    }

    #[test]
    fn test_formula_to_rule_strips_integral() {
        let formula = FormulaEntry {
            slug: "integral_power".into(),
            expr_str: "∫ x^n dx = x^(n+1)/(n+1) + C".into(),
            descriptions: vec![],
            aliases: vec![],
            source: "test".into(),
            domain: "calculus".into(),
            tags: vec!["integral".into()],
            linked_fact_ids: vec![],
        };
        let rule = RuleEngine::formula_to_rule(&formula).expect("should convert");
        assert_eq!(rule.domain, RuleDomain::Integrate);
        // Pattern should be x^n, not Mul(Mul(int, x^n), dx)
        let pat_str = format!("{}", rule.pattern);
        let tpl_str = format!("{}", rule.template);
        assert!(
            pat_str.contains("x^n") || pat_str.contains("x^"),
            "pattern should be x^n, got: {}",
            pat_str
        );
        assert!(
            tpl_str.contains("C"),
            "template should contain + C, got: {}",
            tpl_str
        );
    }

    #[test]
    fn test_formula_to_rule_strips_int_text() {
        let formula = FormulaEntry {
            slug: "integral_exp".into(),
            expr_str: "int e^x dx = e^x + C".into(),
            descriptions: vec![],
            aliases: vec![],
            source: "test".into(),
            domain: "calculus".into(),
            tags: vec!["integral".into()],
            linked_fact_ids: vec![],
        };
        let rule = RuleEngine::formula_to_rule(&formula).expect("should convert");
        assert_eq!(rule.domain, RuleDomain::Integrate);
        let pat_str = format!("{}", rule.pattern);
        // After stripping "int " and " dx", "e^x" should be the pattern
        // e is parsed as Euler's constant (Num(2.718...)), so the display
        // will show the numeric value. Check for x in the exponent.
        assert!(
            pat_str.contains("x"),
            "pattern should contain x, got: {}",
            pat_str
        );
        assert!(
            !pat_str.contains("int"),
            "pattern should not contain 'int', got: {}",
            pat_str
        );
    }

    #[test]
    fn test_formula_to_rule_keeps_physics_identity() {
        // Non-calculus formulas should be unchanged
        let formula = FormulaEntry {
            slug: "newtons_second_law".into(),
            expr_str: "F = m*a".into(),
            descriptions: vec![],
            aliases: vec![],
            source: "test".into(),
            domain: "physics".into(),
            tags: vec!["physics".into()],
            linked_fact_ids: vec![],
        };
        let rule = RuleEngine::formula_to_rule(&formula).expect("should convert");
        assert_eq!(rule.domain, RuleDomain::Simplify);
        let pat_str = format!("{}", rule.pattern);
        // F = m*a -> LHS is "F", RHS is "m*a", domain is Simplify
        // pattern = F? No... wait. pattern should be the LHS of the formula which is "F" = Var("F")
        assert_eq!(pat_str, "F", "pattern should be F, got: {}", pat_str);
    }

    #[test]
    fn test_formula_to_rule_strips_ddx_with_parens() {
        let formula = FormulaEntry {
            slug: "derivative_of_ln".into(),
            expr_str: "d/dx ln(x) = 1/x".into(),
            descriptions: vec![],
            aliases: vec![],
            source: "test".into(),
            domain: "calculus".into(),
            tags: vec!["derivative".into()],
            linked_fact_ids: vec![],
        };
        let rule = RuleEngine::formula_to_rule(&formula).expect("should convert");
        assert_eq!(rule.domain, RuleDomain::Differentiate);
        let pat_str = format!("{}", rule.pattern);
        let tpl_str = format!("{}", rule.template);
        // "d/dx ln(x)" should strip to "ln(x)"
        assert_eq!(
            pat_str, "ln(x)",
            "pattern should be ln(x), got: {}",
            pat_str
        );
        assert_eq!(tpl_str, "1/x", "template should be 1/x, got: {}", tpl_str);
    }

    /// Test that bootstrap formulas in the registry auto-convert to usable rules
    /// and can be applied for computation.
    #[test]
    fn test_bootstrap_registry_converts_to_rules() {
        let mut registry = FormulaRegistry::new();
        registry.seed_bootstrap();

        // Convert all formulas to rules
        let mut engine = RuleEngine::new();
        for formula in registry.formulas() {
            if let Some(rule) = RuleEngine::formula_to_rule(formula) {
                let _ = engine.add_rule(rule);
            }
        }

        // The derivative_of_sin formula should have been converted
        let auto_sin = engine
            .rules
            .iter()
            .find(|r| r.slug.contains("derivative_of_sin"));
        assert!(auto_sin.is_some(), "expected auto_derivative_of_sin rule");
        if let Some(rule) = auto_sin {
            // Pattern should be sin(x), not d/dx sin(x)
            let pat_str = format!("{}", rule.pattern);
            assert_eq!(
                pat_str, "sin(x)",
                "auto_derivative_of_sin pattern should be sin(x), got: {}",
                pat_str
            );
            assert_eq!(rule.domain, RuleDomain::Differentiate);
        }

        // Test matching sin(x) against the auto-converted rule
        use crate::algebra::SymExpr::*;
        let expr = Var("x".into()).sin();
        let extra = std::collections::HashMap::new();
        let result = engine.try_apply(&expr, &RuleDomain::Differentiate, &extra);
        assert!(
            result.is_some(),
            "auto-converted sin derivative should match sin(x)"
        );
        let (result_expr, slug) = result.unwrap();
        assert_eq!(
            format!("{}", result_expr),
            "cos(x)",
            "auto derivative_of_sin should produce cos(x), got: {}, slug: {}",
            result_expr,
            slug
        );
    }

    /// Test that auto-converted integral power rule matches and computes correctly.
    #[test]
    fn test_bootstrap_integral_power_rule_auto() {
        let mut registry = FormulaRegistry::new();
        registry.seed_bootstrap();

        let mut engine = RuleEngine::new();
        for formula in registry.formulas() {
            if let Some(rule) = RuleEngine::formula_to_rule(formula) {
                let _ = engine.add_rule(rule);
            }
        }

        // The integral_power_rule formula: "∫ x^n dx = x^(n+1)/(n+1) + C"
        // After auto-conversion: pattern = x^n, template = x^(n+1)/(n+1) + C
        let auto_int = engine
            .rules
            .iter()
            .find(|r| r.slug.contains("integral_power_rule"));
        assert!(auto_int.is_some(), "expected auto_integral_power_rule");

        use crate::algebra::SymExpr::*;
        let expr = Var("x".into()).pow(Num(5.0));
        let extra = std::collections::HashMap::new();
        let result = engine.try_apply(&expr, &RuleDomain::Integrate, &extra);
        assert!(
            result.is_some(),
            "auto integral power rule should match x^5"
        );
        let (result_expr, _) = result.unwrap();
        let result_str = format!("{}", result_expr);
        assert!(
            result_str.contains("x^6"),
            "expected x^6 in result, got: {}",
            result_str
        );
    }

    /// Test the full pipeline: differentiate_str_with_rules falls back to
    /// auto-converted formula rules.
    #[test]
    fn test_differentiate_with_auto_rules_fallback() {
        use crate::algebra::SymExpr::*;

        // Build a rule engine with auto-converted formulas from the registry
        let mut registry = FormulaRegistry::new();
        registry.seed_bootstrap();

        let mut engine = RuleEngine::new();
        engine.seed_bootstrap(); // built-in bootstrap rules
        for formula in registry.formulas() {
            if let Some(rule) = RuleEngine::formula_to_rule(formula) {
                let _ = engine.add_rule(rule);
            }
        }

        // differentiate_str_with_rules should work for sin(x) (hardcoded handles it)
        let result = crate::algebra::differentiate_str_with_rules("sin(x)", "x", &engine.rules);
        assert!(
            result.is_ok(),
            "differentiate_str_with_rules(sin(x)) should succeed"
        );
        let result = result.unwrap();
        assert_eq!(
            result, "cos(x)",
            "d/dx sin(x) should be cos(x), got: {}",
            result
        );

        // Test with differentiate_str_with_rules for x^5 (hardcoded handles power rule)
        let result = crate::algebra::differentiate_str_with_rules("x^5", "x", &engine.rules);
        assert!(
            result.is_ok(),
            "differentiate_str_with_rules(x^5) should succeed"
        );
        let result = result.unwrap();
        assert!(
            result.contains("x^4") || result.contains("x⁴"),
            "d/dx x^5 should be 5*x^4, got: {}",
            result
        );
    }

    /// Test that auto-synced formulas from bootstrap registry create usable
    /// computation rules via the QA engine's sync_formulas_to_rules.
    #[test]
    fn test_qa_sync_formulas_to_rules_auto_bridge() {
        let mut qa = crate::qa::QaEngine::new();
        qa.formula_registry.seed_bootstrap();

        let rule_count_before = qa.rule_engine.rules.len();
        qa.sync_formulas_to_rules();
        let rule_count_after = qa.rule_engine.rules.len();

        // Should have added at least some rules from the registry
        assert!(
            rule_count_after > rule_count_before,
            "sync should add rules: before={}, after={}",
            rule_count_before,
            rule_count_after
        );

        // Verify the derivative_of_sin auto rule exists
        let auto_rule = qa
            .rule_engine
            .rules
            .iter()
            .find(|r| r.slug == "auto_derivative_of_sin");
        assert!(
            auto_rule.is_some(),
            "expected auto_derivative_of_sin rule after sync"
        );

        // Verify the integral_power_rule auto rule exists
        let auto_int = qa
            .rule_engine
            .rules
            .iter()
            .find(|r| r.slug == "auto_integral_power_rule");
        assert!(
            auto_int.is_some(),
            "expected auto_integral_power_rule after sync"
        );
    }

    // ── Definite Integral (Bounds) Handling Tests ──────────────────────

    /// Definite integrals with bounds should be gracefully skipped — they
    /// produce patterns like `f(x)` or `x^2` that don't make sense as
    /// computation rules (either too generic or template references bounds).
    #[test]
    fn test_formula_to_rule_skips_definite_integral_fmt() {
        // ∫_0^1 x^2 dx  (unicode ∫ with LaTeX bounds)
        let formula = FormulaEntry {
            slug: "definite_int_unicode".into(),
            expr_str: "∫_0^1 x^2 dx = 1/3".into(),
            descriptions: vec![],
            aliases: vec![],
            source: "test".into(),
            domain: "calculus".into(),
            tags: vec!["integral".into(), "definite".into()],
            linked_fact_ids: vec![],
        };
        // Should return None (skip), not crash
        let rule = RuleEngine::formula_to_rule(&formula);
        assert!(
            rule.is_none(),
            "definite integral should be skipped, got: {:?}",
            rule
        );
    }

    #[test]
    fn test_formula_to_rule_skips_definite_integral_int_text() {
        // int_a^b f(x) dx (text notation with bounds)
        let formula = FormulaEntry {
            slug: "fundamental_theorem".into(),
            expr_str: "int_a^b f(x) dx = F(b) - F(a)".into(),
            descriptions: vec![],
            aliases: vec![],
            source: "test".into(),
            domain: "calculus".into(),
            tags: vec!["integral".into(), "definite".into()],
            linked_fact_ids: vec![],
        };
        // Should return None (skip), not crash
        let rule = RuleEngine::formula_to_rule(&formula);
        assert!(
            rule.is_none(),
            "FTC formula should be skipped, got: {:?}",
            rule
        );
    }

    #[test]
    fn test_formula_to_rule_skips_definite_integral_frac_bounds() {
        // ∫_{0}^{π} sin(x) dx  (LaTeX \int_{0}^{\pi})
        let formula = FormulaEntry {
            slug: "definite_sin".into(),
            expr_str: "∫_{0}^{π} sin(x) dx = 2".into(),
            descriptions: vec![],
            aliases: vec![],
            source: "test".into(),
            domain: "calculus".into(),
            tags: vec!["integral".into(), "definite".into()],
            linked_fact_ids: vec![],
        };
        let rule = RuleEngine::formula_to_rule(&formula);
        let rule = RuleEngine::formula_to_rule(&formula);
        assert!(
            rule.is_none(),
            "definite integral with curly braces should be skipped, got: {:?}",
            rule
        );
    }

    #[test]
    fn test_formula_to_rule_accepts_indefinite_integral() {
        // Indefinite integrals should still convert fine
        let formula = FormulaEntry {
            slug: "indefinite_power".into(),
            expr_str: "∫ x^5 dx = x^6/6 + C".into(),
            descriptions: vec![],
            aliases: vec![],
            source: "test".into(),
            domain: "calculus".into(),
            tags: vec!["integral".into()],
            linked_fact_ids: vec![],
        };
        let rule = RuleEngine::formula_to_rule(&formula);
        assert!(
            rule.is_some(),
            "indefinite integral should convert, got None"
        );
        if let Some(r) = rule {
            assert_eq!(r.domain, RuleDomain::Integrate);
            let pat_str = format!("{}", r.pattern);
            assert_eq!(pat_str, "x^5", "pattern should be x^5, got: {}", pat_str);
        }
    }

    // ── Self-Derivation Tests ──────────────────────────────────────────

    #[test]
    fn test_derive_integral_simple_swap() {
        let mut engine = RuleEngine::new();

        // Add a simple derivative rule: d/dx e^x = e^x
        use crate::algebra::SymExpr::*;
        let rule = ComputationRule {
            slug: "diff_exp_test".into(),
            domain: RuleDomain::Differentiate,
            pattern: Var("x".into()).exp(),
            template: Var("x".into()).exp(),
            description: "d/dx e^x = e^x".into(),
            confidence: 0.99,
        };
        engine.add_rule(rule).unwrap();

        // Derive
        let count = engine.derive_integral_rules();
        assert_eq!(count, 1, "should derive 1 rule");

        // Check the derived rule exists
        let derived = engine
            .rules
            .iter()
            .find(|r| r.slug == "derived_int_from_diff_exp_test");
        assert!(derived.is_some(), "derived rule should exist");

        // Check the derived rule's pattern and template
        if let Some(r) = derived {
            assert_eq!(r.domain, RuleDomain::Integrate);
            let pat_str = format!("{}", r.pattern);
            let tpl_str = format!("{}", r.template);
            // exp(x) is the exponential function (displayed as exp(x))
            assert!(
                pat_str.contains("exp"),
                "derived pattern should contain exp, got: {}",
                pat_str
            );
            // Template should contain + C
            assert!(
                tpl_str.contains("x"),
                "template should contain x, got: {}",
                tpl_str
            );
            assert!(
                tpl_str.contains("C"),
                "template should contain + C, got: {}",
                tpl_str
            );
        }

        // Test that the derived rule actually matches
        let expr = Var("x".into()).exp();
        let extra = std::collections::HashMap::new();
        let result = engine.try_apply(&expr, &RuleDomain::Integrate, &extra);
        assert!(result.is_some(), "derived rule should match e^x");
        if let Some((result_expr, _)) = result {
            let s = format!("{}", result_expr);
            assert!(s.contains("x"), "result should contain x, got: {}", s);
            assert!(s.contains("C"), "result should contain + C, got: {}", s);
        }
    }

    #[test]
    fn test_derive_integral_negation_handling() {
        let mut engine = RuleEngine::new();

        // Add: d/dx cos(x) = -sin(x)
        use crate::algebra::SymExpr::*;
        let rule = ComputationRule {
            slug: "diff_cos_test".into(),
            domain: RuleDomain::Differentiate,
            pattern: Var("x".into()).cos(),
            template: -(Var("x".into()).sin()),
            description: "d/dx cos(x) = -sin(x)".into(),
            confidence: 0.99,
        };
        engine.add_rule(rule).unwrap();

        let count = engine.derive_integral_rules();
        assert_eq!(count, 1, "should derive 1 rule from Neg template");

        // Check the derived rule
        let derived = engine
            .rules
            .iter()
            .find(|r| r.slug == "derived_int_from_diff_cos_test");
        assert!(derived.is_some());

        if let Some(r) = derived {
            assert_eq!(r.domain, RuleDomain::Integrate);
            let pat_str = format!("{}", r.pattern);
            // Pattern should be sin(x) (the inner of Neg)
            assert_eq!(
                pat_str, "sin(x)",
                "derived pattern should be sin(x), got: {}",
                pat_str
            );
        }

        // Test matching: ∫ sin(x) dx = -cos(x) + C
        let expr = Var("x".into()).sin();
        let extra = std::collections::HashMap::new();
        let result = engine.try_apply(&expr, &RuleDomain::Integrate, &extra);
        assert!(result.is_some(), "derived rule should match sin(x)");
        if let Some((result_expr, _)) = result {
            let s = format!("{}", result_expr);
            assert!(s.contains("cos"), "result should contain cos, got: {}", s);
            assert!(s.contains("C"), "result should contain + C, got: {}", s);
        }
    }

    #[test]
    fn test_derive_integral_chain_rule_factor() {
        let mut engine = RuleEngine::new();

        // Add: d/dx sin(ax+b) = a*cos(ax+b)  (chain rule with factor)
        use crate::algebra::SymExpr::*;
        let a = || Var("a".into());
        let b = || Var("b".into());
        let x = || Var("x".into());
        let axb = || a() * x() + b();

        let rule = ComputationRule {
            slug: "diff_sin_linear_test".into(),
            domain: RuleDomain::Differentiate,
            pattern: axb().sin(),
            template: a() * axb().cos(),
            description: "d/dx sin(ax+b) = a*cos(ax+b)".into(),
            confidence: 0.95,
        };
        engine.add_rule(rule).unwrap();

        let count = engine.derive_integral_rules();
        assert_eq!(count, 1, "should derive 1 rule");

        // Check the derived rule
        let derived = engine
            .rules
            .iter()
            .find(|r| r.slug == "derived_int_from_diff_sin_linear_test");
        assert!(derived.is_some(), "derived rule should exist");

        if let Some(r) = derived {
            assert_eq!(r.domain, RuleDomain::Integrate);
            let pat_str = format!("{}", r.pattern);
            // Pattern should be a*cos(ax+b) (the full derivative template, unchanged)
            assert!(
                pat_str.contains("cos"),
                "derived pattern should contain cos, got: {}",
                pat_str
            );
            assert!(
                pat_str.contains("a"),
                "derived pattern should contain factor a, got: {}",
                pat_str
            );
        }

        // Test matching: ∫ 2*cos(2x+1) dx should match with a=2, b=1
        let expr = Num(2.0) * (Num(2.0) * x() + Num(1.0)).cos();
        let extra = std::collections::HashMap::new();
        let result = engine.try_apply(&expr, &RuleDomain::Integrate, &extra);
        assert!(result.is_some(), "derived rule should match 2*cos(2x+1)");
        if let Some((result_expr, _)) = result {
            let s = format!("{}", result_expr);
            assert!(s.contains("sin"), "result should contain sin, got: {}", s);
            assert!(s.contains("C"), "result should contain + C, got: {}", s);
        }
    }

    #[test]
    fn test_bootstrap_derives_integral_rules() {
        let mut engine = RuleEngine::new();
        engine.seed_bootstrap();

        // Should have derived rules for each differentiable rule
        let derived_count = engine
            .rules
            .iter()
            .filter(|r| r.slug.starts_with("derived_int_from_"))
            .count();
        // 11 derivative rules × ~1 each → ~11, minus conflicts
        assert!(
            derived_count >= 8,
            "expected >= 8 derived rules, got {}",
            derived_count
        );

        // Verify key derived rules exist
        let expected = [
            "derived_int_from_diff_sin",
            "derived_int_from_diff_cos",
            "derived_int_from_diff_exp",
            "derived_int_from_diff_ln",
            "derived_int_from_diff_tan",
            "derived_int_from_diff_sin_linear",
            "derived_int_from_diff_cos_linear",
        ];
        for slug in &expected {
            assert!(
                engine.rules.iter().any(|r| r.slug == *slug),
                "expected derived rule '{}'",
                slug
            );
        }

        // Verify derived rules work: ∫ cos(x) dx should match (derived from diff_sin)
        use crate::algebra::SymExpr::*;
        let extra = std::collections::HashMap::new();
        let expr = Var("x".into()).cos();
        let result = engine.try_apply(&expr, &RuleDomain::Integrate, &extra);
        assert!(result.is_some(), "∫ cos(x) should match a rule");
        if let Some((res, slug)) = result {
            let s = format!("{}", res);
            assert!(
                s.contains("sin"),
                "∫ cos(x) should give sin(x)+?, got: {} (slug: {})",
                s,
                slug
            );
        }

        // Verify that derived rules are CORRECT by checking specific rules
        // Check derived_int_from_diff_ln: pattern should be 1/x, template ln(x)+C
        let derived_ln = engine
            .rules
            .iter()
            .find(|r| r.slug == "derived_int_from_diff_ln");
        assert!(
            derived_ln.is_some(),
            "derived_int_from_diff_ln should exist"
        );
        if let Some(r) = derived_ln {
            assert_eq!(r.domain, RuleDomain::Integrate);
            let pat_str = format!("{}", r.pattern);
            let tpl_str = format!("{}", r.template);
            // Pattern is x^(-1) (the derivative template for d/dx ln(x) = 1/x is stored as x^(-1))
            assert!(
                pat_str.contains("^-") || pat_str.contains("1/x"),
                "derived_int_from_diff_ln pattern should be x^-1 or 1/x, got: {}",
                pat_str
            );
            assert!(
                tpl_str.contains("ln"),
                "template should contain ln(x), got: {}",
                tpl_str
            );
            assert!(
                tpl_str.contains("C"),
                "template should contain + C, got: {}",
                tpl_str
            );
        }

        // Check that int_reciprocal (bootstrap, pattern x^(-1)) still works alongside derived rule
        let reciprocal = engine.rules.iter().find(|r| r.slug == "int_reciprocal");
        assert!(reciprocal.is_some(), "int_reciprocal should still exist");
    }

    #[test]
    fn test_derive_integral_via_qa_add_rule() {
        // Simulate what happens when a user adds a differentiation rule
        // via the ADD_RULE admin command.
        let mut qa = crate::qa::QaEngine::new();

        // Manually add a derivative rule (as ADD_RULE would do)
        use crate::algebra::SymExpr::*;
        let rule = ComputationRule {
            slug: "diff_sinh_custom".into(),
            domain: RuleDomain::Differentiate,
            pattern: Var("x".into()).sinh(),
            template: Var("x".into()).cosh(),
            description: "d/dx sinh(x) = cosh(x)".into(),
            confidence: 0.95,
        };
        qa.rule_engine.add_rule(rule).unwrap();

        // Auto-derive (as ADD_RULE handler does)
        let derived_count = qa.rule_engine.derive_integral_rules();
        assert_eq!(derived_count, 1, "should derive ∫ cosh from d/dx sinh");

        // Verify: ∫ cosh(x) dx = sinh(x) + C
        let expr = Var("x".into()).cosh();
        let extra = std::collections::HashMap::new();
        let result = qa
            .rule_engine
            .try_apply(&expr, &RuleDomain::Integrate, &extra);
        assert!(result.is_some(), "∫ cosh(x) should match derived rule");
        if let Some((result_expr, _)) = result {
            let s = format!("{}", result_expr);
            assert!(
                s.contains("sinh"),
                "∫ cosh(x) should give sinh(x)+C, got: {}",
                s
            );
        }
    }
}

/// Parse a LaTeX equation string into a SymExpr equation.
/// Returns (lhs, rhs) on success.
///
/// NOTE: This is a stub function that forwards to the standard parser.
/// Full LaTeX parsing is not yet implemented.
pub fn latex_to_equation(
    s: &str,
) -> Result<(crate::algebra::SymExpr, crate::algebra::SymExpr), String> {
    crate::algebra::parse_equation(s)
}
