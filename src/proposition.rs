// ─── Proposition AST & Theorem Schemas ──────────────────────────────
//
// A layer above `SymExpr`: represents claims about terms, not just terms.
//
//     SymExpr  →  x² + 1       (the term language)
//     Proposition → ∀x, x ≥ 0 → sqrt(x²) = x   (the claim language)
//
// ## Key types
//
// | Type | Purpose |
// |------|---------|
// | `Proposition` | Logical claim about SymExpr terms |
// | `Binder` | Introduces a ∀-bound variable or →-assumption |
// | `TheoremSchema` | A stored theorem with binders/premises/conclusion |
// | `Substitution` | Maps variable names → SymExpr for instantiation |
//
// ## Design decisions
//
// - Variables are `String`-named, matching `SymExpr::Var(String)`.
//   No separate `VarId` numeric type — avoids impedance mismatch with
//   the deeply-embedded `SymExpr::Var("x")` pattern.
// - Capture-safe substitution freshens bound variables by appending
//   a counter suffix (e.g., `x` → `x_1`) when a substitution would
//   otherwise capture a free variable.
// - All 12 initial theorem schemas are hand-curated. No auto-conversion
//   from the 18k legacy formula database. Those remain `KnowledgeStatus::Legacy`
//   and are invisible to the proof kernel.

use crate::algebra::{SymExpr, VarId, Variable, VariableKind};
use std::collections::{HashMap, HashSet};
use std::fmt;
use std::sync::LazyLock;
use std::sync::Mutex;

// ── Shared variable cache for production and test code ─────────────
// Ensures variables with the same display name share the same VarId
// across `initial_theorems()`, `forall_v()`, and test helpers.
static SHARED_VARS: LazyLock<Mutex<HashMap<String, Variable>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

/// Get or create a Variable with the given display name from the shared cache.
/// Multiple calls with the same name return the same Variable (same VarId).
pub fn shared_var(name: &str) -> Variable {
    let mut map = SHARED_VARS.lock().unwrap();
    map.entry(name.to_string())
        .or_insert_with(|| Variable::interned(name))
        .clone()
}

// ═══════════════════════════════════════════════════════════════════
// IDENTIFIERS
// ═══════════════════════════════════════════════════════════════════

/// Identifies a hypothesis in a LocalContext.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct HypothesisId(pub u64);

/// Identifies a theorem in the trusted theorem environment.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct TheoremId(pub u64);

// ═══════════════════════════════════════════════════════════════════
// BINDER
// ═══════════════════════════════════════════════════════════════════

/// A binder introduces either a ∀-quantified variable or an →-assumption.
#[derive(Clone, Debug, PartialEq)]
pub enum Binder {
    /// ForAll { variable: x } in `∀x, P(x)`.
    /// The variable's display name is used in `SymExpr::Var(variable)`.
    ForAll { variable: Variable },
    /// Assumption in `P → Q`. `hypothesis_id` tracks it in the context.
    Assumption {
        hypothesis_id: HypothesisId,
        proposition: Box<Proposition>,
    },
}

impl Binder {
    pub fn var_name(&self) -> Option<&str> {
        match self {
            Binder::ForAll { variable } => Some(variable.display.as_ref()),
            Binder::Assumption { .. } => None,
        }
    }
}

impl fmt::Display for Binder {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Binder::ForAll { variable } => write!(f, "∀{}", variable.display),
            Binder::Assumption { proposition, .. } => {
                write!(f, "({}) → ?", proposition)
            }
        }
    }
}

// ═══════════════════════════════════════════════════════════════════
// PROPOSITION
// ═══════════════════════════════════════════════════════════════════

/// A logical claim about symbolic expressions.
#[derive(Clone, Debug, PartialEq)]
pub enum Proposition {
    /// ⊤ — always true.
    True,
    /// ⊥ — always false.
    False,

    // ── Relations (term-level) ────────────────────────────────
    /// a = b
    Eq(SymExpr, SymExpr),
    /// a ≠ b
    NotEq(SymExpr, SymExpr),
    /// a < b
    Lt(SymExpr, SymExpr),
    /// a ≤ b
    Le(SymExpr, SymExpr),
    /// a > b
    Gt(SymExpr, SymExpr),
    /// a ≥ b
    Ge(SymExpr, SymExpr),

    // ── Predicates ────────────────────────────────────────────
    /// A generic predicate: e.g. Real(x), DefinedAt(f, x)
    Predicate { symbol: String, args: Vec<SymExpr> },

    // ── Logical connectives ───────────────────────────────────
    /// ¬P
    Not(Box<Proposition>),
    /// P ∧ Q (binary for simplicity; nested for N-ary)
    And(Box<Proposition>, Box<Proposition>),
    /// P ∨ Q
    Or(Box<Proposition>, Box<Proposition>),
    /// P → Q
    Implies(Box<Proposition>, Box<Proposition>),
    /// P ↔ Q
    Iff(Box<Proposition>, Box<Proposition>),

    // ── Quantifiers ───────────────────────────────────────────
    /// ∀x, P(x)
    ForAll(Binder, Box<Proposition>),
    /// ∃x, P(x)
    Exists(Binder, Box<Proposition>),
}

// ── Convenience constructors ─────────────────────────────────────

impl Proposition {
    pub fn eq(lhs: SymExpr, rhs: SymExpr) -> Self {
        Proposition::Eq(lhs, rhs)
    }

    pub fn ne(lhs: SymExpr, rhs: SymExpr) -> Self {
        Proposition::NotEq(lhs, rhs)
    }

    pub fn ge(lhs: SymExpr, rhs: SymExpr) -> Self {
        Proposition::Ge(lhs, rhs)
    }

    pub fn gt(lhs: SymExpr, rhs: SymExpr) -> Self {
        Proposition::Gt(lhs, rhs)
    }

    pub fn le(lhs: SymExpr, rhs: SymExpr) -> Self {
        Proposition::Le(lhs, rhs)
    }

    pub fn lt(lhs: SymExpr, rhs: SymExpr) -> Self {
        Proposition::Lt(lhs, rhs)
    }

    pub fn not(p: Proposition) -> Self {
        Proposition::Not(Box::new(p))
    }

    pub fn and(a: Proposition, b: Proposition) -> Self {
        Proposition::And(Box::new(a), Box::new(b))
    }

    pub fn or(a: Proposition, b: Proposition) -> Self {
        Proposition::Or(Box::new(a), Box::new(b))
    }

    pub fn implies(premise: Proposition, conclusion: Proposition) -> Self {
        Proposition::Implies(Box::new(premise), Box::new(conclusion))
    }

    pub fn iff(a: Proposition, b: Proposition) -> Self {
        Proposition::Iff(Box::new(a), Box::new(b))
    }

    pub fn forall(variable: &Variable, body: Proposition) -> Self {
        Proposition::ForAll(
            Binder::ForAll {
                variable: variable.clone(),
            },
            Box::new(body),
        )
    }

    pub fn exists(variable: &Variable, body: Proposition) -> Self {
        Proposition::Exists(
            Binder::ForAll {
                variable: variable.clone(),
            },
            Box::new(body),
        )
    }

    pub fn predicate(symbol: &str, args: Vec<SymExpr>) -> Self {
        Proposition::Predicate {
            symbol: symbol.to_string(),
            args,
        }
    }

    /// Collect all free (unbound) variable names in this proposition.
    pub fn free_vars(&self) -> HashSet<String> {
        free_vars_proposition(self)
    }
}

// ═══════════════════════════════════════════════════════════════════
// DISPLAY
// ═══════════════════════════════════════════════════════════════════

impl fmt::Display for Proposition {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Proposition::True => write!(f, "⊤"),
            Proposition::False => write!(f, "⊥"),
            Proposition::Eq(a, b) => write!(f, "({} = {})", a, b),
            Proposition::NotEq(a, b) => write!(f, "({} ≠ {})", a, b),
            Proposition::Lt(a, b) => write!(f, "({} < {})", a, b),
            Proposition::Le(a, b) => write!(f, "({} ≤ {})", a, b),
            Proposition::Gt(a, b) => write!(f, "({} > {})", a, b),
            Proposition::Ge(a, b) => write!(f, "({} ≥ {})", a, b),
            Proposition::Predicate { symbol, args } => {
                let args_str: Vec<String> = args.iter().map(|a| format!("{}", a)).collect();
                write!(f, "{}({})", symbol, args_str.join(", "))
            }
            Proposition::Not(p) => write!(f, "¬({})", p),
            Proposition::And(a, b) => write!(f, "({} ∧ {})", a, b),
            Proposition::Or(a, b) => write!(f, "({} ∨ {})", a, b),
            Proposition::Implies(a, b) => write!(f, "({} → {})", a, b),
            Proposition::Iff(a, b) => write!(f, "({} ↔ {})", a, b),
            Proposition::ForAll(binder, body) => {
                write!(f, "∀{}, ({})", binder.var_name().unwrap_or("?"), body)
            }
            Proposition::Exists(binder, body) => {
                write!(f, "∃{}, ({})", binder.var_name().unwrap_or("?"), body)
            }
        }
    }
}

// ═══════════════════════════════════════════════════════════════════
// THEOREM TRUST
// ═══════════════════════════════════════════════════════════════════

/// How a theorem entered the trusted environment.
#[derive(Clone, Debug, PartialEq)]
pub enum TheoremTrust {
    /// Logical axiom (equality, etc.) — cannot be wrong.
    LogicalPrimitive,
    /// Curated by hand from a known-correct source.
    CuratedAxiom,
}

// ═══════════════════════════════════════════════════════════════════
// THEOREM SCHEMA
// ═══════════════════════════════════════════════════════════════════

/// A stored theorem with typed binders, premises, and conclusion.
///
/// For example, the theorem `∀x, x ≥ 0 → sqrt(x²) = x` has:
/// - binders: [ForAll("x")]
/// - premises: [Ge(Var("x"), Num(0))]
/// - conclusion: Eq(Sqrt(Pow(Var("x"), Num(2.0))), Var("x"))
#[derive(Clone, Debug)]
pub struct TheoremSchema {
    pub id: TheoremId,
    pub name: String,
    /// Universally quantified variables (in order).
    pub binders: Vec<String>,
    /// Premises (assumptions) of the implication chain.
    pub premises: Vec<Proposition>,
    /// The conclusion (what the theorem asserts).
    pub conclusion: Proposition,
    /// How this theorem was added.
    pub trust: TheoremTrust,
}

impl TheoremSchema {
    /// Create a new theorem schema.
    pub fn new(
        id: TheoremId,
        name: &str,
        binders: Vec<String>,
        premises: Vec<Proposition>,
        conclusion: Proposition,
        trust: TheoremTrust,
    ) -> Self {
        TheoremSchema {
            id,
            name: name.to_string(),
            binders,
            premises,
            conclusion,
            trust,
        }
    }

    /// Return the full proposition: ∀binders, (premises → conclusion).
    pub fn as_proposition(&self) -> Proposition {
        let mut p = self.conclusion.clone();
        // Wrap in premises: P1 → P2 → ... → conclusion
        for premise in self.premises.iter().rev() {
            p = Proposition::implies(premise.clone(), p);
        }
        // Wrap in quantifiers: ∀x1, ∀x2, ... → body
        for binder in self.binders.iter().rev() {
            let var = shared_var(binder);
            p = Proposition::forall(&var, p);
        }
        p
    }

    /// Check if a variable name is one of the binders.
    pub fn is_bound(&self, name: &str) -> bool {
        self.binders.iter().any(|b| b == name)
    }
}

impl fmt::Display for TheoremSchema {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {}", self.name, self.as_proposition())
    }
}

// ═══════════════════════════════════════════════════════════════════
// SUBSTITUTION
// ═══════════════════════════════════════════════════════════════════

/// A mapping from logical variable identities → symbolic expressions.
///
/// Used by `apply_theorem` to instantiate a theorem's bound variables.
///
/// # Capture safety
///
/// During substitution into a proposition, bound variables are freshened
/// (renamed to `name_N`) when a substitution would otherwise cause capture.
/// For example, substituting `x → f(y)` into `∀y, P(x, y)` yields
/// `∀y_1, P(f(y), y_1)` — the inner `y` is freshened to avoid capture.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Substitution {
    map: std::collections::HashMap<VarId, SymExpr>,
    /// Retained solely for diagnostics. Identity, never the display name, is
    /// the lookup key.
    names: std::collections::HashMap<VarId, String>,
}

/// Instantiation of a theorem schema.  Keys are the schema binders' `VarId`s.
pub type TheoremInstantiation = Substitution;
/// Bindings accumulated while unifying fresh theorem meta-variables.
pub type MetaSubstitution = Substitution;

/// Values accepted by the legacy-friendly substitution API.  Production
/// proof code passes `Variable`; string support resolves through the shared
/// theorem-variable cache and exists only for older tests and callers.
pub trait VariableKey {
    fn variable(&self) -> Variable;
}

impl VariableKey for Variable {
    fn variable(&self) -> Variable {
        self.clone()
    }
}
impl VariableKey for &Variable {
    fn variable(&self) -> Variable {
        (*self).clone()
    }
}
impl VariableKey for &str {
    fn variable(&self) -> Variable {
        shared_var(self)
    }
}
impl VariableKey for String {
    fn variable(&self) -> Variable {
        shared_var(self)
    }
}
impl VariableKey for VarId {
    fn variable(&self) -> Variable {
        Variable::new(*self, VariableKind::Rigid, format!("v{}", self.0))
    }
}

impl Substitution {
    pub fn new() -> Self {
        Substitution {
            map: std::collections::HashMap::new(),
            names: std::collections::HashMap::new(),
        }
    }

    /// Insert a binding: `var` → `expr`.
    pub fn insert<K: VariableKey>(&mut self, var: K, expr: SymExpr) {
        let var = var.variable();
        self.names.insert(var.id, var.display.to_string());
        self.map.insert(var.id, expr);
    }

    /// Look up a binding.
    pub fn get<K: VariableKey>(&self, var: K) -> Option<&SymExpr> {
        self.map.get(&var.variable().id)
    }

    /// Return all bound variable names.
    pub fn domain(&self) -> Vec<VarId> {
        self.map.keys().copied().collect()
    }

    /// Return true if the substitution is empty.
    pub fn is_empty(&self) -> bool {
        self.map.is_empty()
    }

    /// Number of bindings.
    pub fn len(&self) -> usize {
        self.map.len()
    }

    /// Iterate over (var, expr) pairs.
    pub fn iter(&self) -> impl Iterator<Item = (&VarId, &SymExpr)> {
        self.map.iter()
    }
}

impl fmt::Display for Substitution {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let pairs: Vec<String> = self
            .map
            .iter()
            .map(|(k, v)| {
                format!(
                    "{} → {}",
                    self.names.get(k).map(String::as_str).unwrap_or("?"),
                    v
                )
            })
            .collect();
        write!(f, "{{ {} }}", pairs.join(", "))
    }
}

// ═══════════════════════════════════════════════════════════════════
// FREE VARIABLE EXTRACTION
// ═══════════════════════════════════════════════════════════════════

/// Collect all variable names appearing in a `SymExpr`.
pub fn free_vars_term(expr: &SymExpr) -> HashSet<String> {
    use SymExpr::*;
    let mut vars = HashSet::new();
    match expr {
        Num(_) => {}
        Var(name) => {
            vars.insert(name.to_string());
        }
        Add(a, b) | Sub(a, b) | Mul(a, b) | Div(a, b) | Pow(a, b) => {
            vars.extend(free_vars_term(a));
            vars.extend(free_vars_term(b));
        }
        Neg(a) | Sin(a) | Cos(a) | Tan(a) | Sqrt(a) | Exp(a) | Ln(a) | Abs(a) | Sinh(a)
        | Cosh(a) | Tanh(a) | Asin(a) | Acos(a) | Atan(a) => {
            vars.extend(free_vars_term(a));
        }
        Limit { body, .. } | Integral { body, .. } => {
            vars.extend(free_vars_term(body));
        }
    }
    vars
}

/// Collect all free variable names in a `Proposition`.
///
/// Bound variables (in `ForAll`/`Exists`) are excluded from the result.
pub fn free_vars_proposition(p: &Proposition) -> HashSet<String> {
    use Proposition::*;
    let mut vars = HashSet::new();
    match p {
        True | False => {}
        Eq(a, b) | NotEq(a, b) | Lt(a, b) | Le(a, b) | Gt(a, b) | Ge(a, b) => {
            vars.extend(free_vars_term(a));
            vars.extend(free_vars_term(b));
        }
        Predicate { args, .. } => {
            for arg in args {
                vars.extend(free_vars_term(arg));
            }
        }
        Not(body) => vars.extend(free_vars_proposition(body)),
        And(a, b) | Or(a, b) | Implies(a, b) | Iff(a, b) => {
            vars.extend(free_vars_proposition(a));
            vars.extend(free_vars_proposition(b));
        }
        ForAll(Binder::ForAll { variable }, body) | Exists(Binder::ForAll { variable }, body) => {
            vars.extend(free_vars_proposition(body));
            vars.remove(variable.display.as_ref());
        }
        ForAll(Binder::Assumption { proposition, .. }, body)
        | Exists(Binder::Assumption { proposition, .. }, body) => {
            vars.extend(free_vars_proposition(proposition));
            vars.extend(free_vars_proposition(body));
        }
    }
    vars
}

// ═══════════════════════════════════════════════════════════════════
// CAPTURE-SAFE SUBSTITUTION
// ═══════════════════════════════════════════════════════════════════

/// Counter for generating fresh variable names during capture-safe substitution.
/// Thread-local (not atomic) — safe because substitution is single-threaded.
use std::cell::Cell;
thread_local! {
    static FRESH_COUNTER: Cell<u64> = Cell::new(0);
    // Public legacy unification remains permissive for its historical callers.
    // Theorem application enables this guard so only freshly allocated metas
    // (never rigid variables from the goal or local context) are assignable.
    static META_ONLY_UNIFICATION: Cell<bool> = Cell::new(false);
}

fn next_fresh_suffix() -> u64 {
    FRESH_COUNTER.with(|c| {
        let val = c.get() + 1;
        c.set(val);
        val
    })
}

/// Reset the fresh counter (for determinism in tests).
pub fn reset_fresh_counter() {
    FRESH_COUNTER.with(|c| c.set(0));
}

/// Generate a fresh variable name that doesn't conflict with a set of names.
fn fresh_var_name(base: &str, reserved: &HashSet<String>) -> String {
    let mut name = format!("{}_{}", base, next_fresh_suffix());
    while reserved.contains(&name) {
        name = format!("{}_{}", base, next_fresh_suffix());
    }
    name
}

/// Apply a substitution to a `SymExpr`, replacing variables.
///
/// This is always safe — variables are just replaced (no binding structure
/// in SymExpr to cause capture).
pub fn substitute_term(expr: &SymExpr, subst: &Substitution) -> SymExpr {
    use SymExpr::*;
    match expr {
        Num(_) => expr.clone(),
        Var(variable) => subst.get(variable).cloned().unwrap_or_else(|| expr.clone()),
        Add(a, b) => Add(
            Box::new(substitute_term(a, subst)),
            Box::new(substitute_term(b, subst)),
        ),
        Sub(a, b) => Sub(
            Box::new(substitute_term(a, subst)),
            Box::new(substitute_term(b, subst)),
        ),
        Mul(a, b) => Mul(
            Box::new(substitute_term(a, subst)),
            Box::new(substitute_term(b, subst)),
        ),
        Div(a, b) => Div(
            Box::new(substitute_term(a, subst)),
            Box::new(substitute_term(b, subst)),
        ),
        Pow(a, b) => Pow(
            Box::new(substitute_term(a, subst)),
            Box::new(substitute_term(b, subst)),
        ),
        Neg(a) => Neg(Box::new(substitute_term(a, subst))),
        Sin(a) => Sin(Box::new(substitute_term(a, subst))),
        Cos(a) => Cos(Box::new(substitute_term(a, subst))),
        Tan(a) => Tan(Box::new(substitute_term(a, subst))),
        Sqrt(a) => Sqrt(Box::new(substitute_term(a, subst))),
        Exp(a) => Exp(Box::new(substitute_term(a, subst))),
        Ln(a) => Ln(Box::new(substitute_term(a, subst))),
        Abs(a) => Abs(Box::new(substitute_term(a, subst))),
        Sinh(a) => Sinh(Box::new(substitute_term(a, subst))),
        Cosh(a) => Cosh(Box::new(substitute_term(a, subst))),
        Tanh(a) => Tanh(Box::new(substitute_term(a, subst))),
        Asin(a) => Asin(Box::new(substitute_term(a, subst))),
        Acos(a) => Acos(Box::new(substitute_term(a, subst))),
        Atan(a) => Atan(Box::new(substitute_term(a, subst))),
        Limit {
            variable,
            approach,
            body,
        } => {
            // Limits bind `variable` — don't substitute it if it's in the subst domain
            let approach = substitute_term(approach, subst);
            let body = if subst.get(variable).is_some() {
                // The limit variable is bound; don't substitute it
                body.as_ref().clone()
            } else {
                substitute_term(body, subst)
            };
            Limit {
                variable: variable.clone(),
                approach: Box::new(approach),
                body: Box::new(body),
            }
        }
        Integral {
            variable,
            lower,
            upper,
            body,
        } => {
            let lower = lower.as_ref().map(|l| substitute_term(l, subst));
            let upper = upper.as_ref().map(|u| substitute_term(u, subst));
            let body = if subst.get(variable).is_some() {
                body.as_ref().clone()
            } else {
                substitute_term(body, subst)
            };
            Integral {
                variable: variable.clone(),
                lower: lower.map(Box::new),
                upper: upper.map(Box::new),
                body: Box::new(body),
            }
        }
    }
}

/// Apply a substitution to a `Proposition`, with capture-safe handling
/// of bound variables.
///
/// When a substitution would cause a free variable to be captured by a
/// binder (e.g., substituting `x → f(y)` into `∀y, P(x,y)`), the bound
/// variable is freshened: the inner `y` becomes `y_1`.
pub fn substitute_proposition(p: &Proposition, subst: &Substitution) -> Proposition {
    use Proposition::*;

    // Collect all variables that would be introduced by the substitution.
    // These are the free vars of all substitution RHS expressions.
    let _introduced_vars: HashSet<String> = subst
        .iter()
        .flat_map(|(_, expr)| free_vars_term(expr))
        .collect();

    match p {
        True | False => p.clone(),

        Eq(a, b) => Eq(substitute_term(a, subst), substitute_term(b, subst)),
        NotEq(a, b) => NotEq(substitute_term(a, subst), substitute_term(b, subst)),
        Lt(a, b) => Lt(substitute_term(a, subst), substitute_term(b, subst)),
        Le(a, b) => Le(substitute_term(a, subst), substitute_term(b, subst)),
        Gt(a, b) => Gt(substitute_term(a, subst), substitute_term(b, subst)),
        Ge(a, b) => Ge(substitute_term(a, subst), substitute_term(b, subst)),

        Predicate { symbol, args } => Predicate {
            symbol: symbol.clone(),
            args: args.iter().map(|a| substitute_term(a, subst)).collect(),
        },

        Not(body) => Not(Box::new(substitute_proposition(body, subst))),

        And(a, b) => And(
            Box::new(substitute_proposition(a, subst)),
            Box::new(substitute_proposition(b, subst)),
        ),
        Or(a, b) => Or(
            Box::new(substitute_proposition(a, subst)),
            Box::new(substitute_proposition(b, subst)),
        ),
        Implies(a, b) => Implies(
            Box::new(substitute_proposition(a, subst)),
            Box::new(substitute_proposition(b, subst)),
        ),
        Iff(a, b) => Iff(
            Box::new(substitute_proposition(a, subst)),
            Box::new(substitute_proposition(b, subst)),
        ),

        // ── Quantifiers: capture-safe handling ────────────────
        ForAll(Binder::ForAll { variable }, body) | Exists(Binder::ForAll { variable }, body) => {
            // CRITICAL: Remove the bound variable from the substitution domain.
            // If `variable` is in the substitution (e.g. x → z), we must NOT
            // substitute x inside ∀x, P(x) — x is bound here.
            let mut restricted_subst = Substitution::new();
            for (k, v) in subst.iter() {
                if *k != variable.id {
                    restricted_subst.insert(*k, v.clone());
                }
            }

            // Now check if we need to freshen the bound variable to avoid
            // capture: this happens when `var_name` appears free in any
            // RHS of the (restricted) substitution.
            let remaining_introduced: HashSet<String> = restricted_subst
                .iter()
                .flat_map(|(_, expr)| free_vars_term(expr))
                .collect();

            if remaining_introduced.contains(variable.display.as_ref()) {
                // Freshen: rename `var_name` to something that doesn't collide
                let body_free = free_vars_proposition(body);
                let mut reserved: HashSet<String> = restricted_subst
                    .domain()
                    .into_iter()
                    .filter_map(|id| restricted_subst.names.get(&id).cloned())
                    .collect();
                reserved.extend(remaining_introduced);
                reserved.extend(body_free);

                let new_name = fresh_var_name(variable.display.as_ref(), &reserved);

                // Rename the bound variable in the body
                let mut rename_subst = Substitution::new();
                rename_subst.insert(
                    variable.clone(),
                    SymExpr::Var(Variable::fresh_named(&new_name)),
                );
                let renamed_body = substitute_proposition(body, &rename_subst);

                // Apply the restricted substitution to the renamed body
                let subst_body = substitute_proposition(&renamed_body, &restricted_subst);

                let new_binder = Binder::ForAll {
                    variable: Variable::fresh_named(&new_name),
                };
                match p {
                    ForAll(_, _) => ForAll(new_binder, Box::new(subst_body)),
                    _ => Exists(new_binder, Box::new(subst_body)),
                }
            } else {
                // No capture risk — apply the restricted substitution to body
                let subst_body = substitute_proposition(body, &restricted_subst);
                match p {
                    ForAll(_, _) => ForAll(
                        Binder::ForAll {
                            variable: variable.clone(),
                        },
                        Box::new(subst_body),
                    ),
                    _ => Exists(
                        Binder::ForAll {
                            variable: variable.clone(),
                        },
                        Box::new(subst_body),
                    ),
                }
            }
        }

        ForAll(Binder::Assumption { .. }, _) | Exists(Binder::Assumption { .. }, _) => {
            // We don't use Assumption binders in ForAll/Exists currently.
            // If we did, they'd need similar capture handling.
            p.clone()
        }
    }
}

// ═══════════════════════════════════════════════════════════════════
// INSTANTIATE THEOREM
// ═══════════════════════════════════════════════════════════════════

/// Given a theorem schema and a substitution for its binders, produce
/// the instantiated premises and conclusion.
///
/// Returns `None` if the substitution doesn't cover all binders.
///
/// # Capture safety
///
/// This applies the substitution to the theorem's premises and conclusion,
/// using capture-safe substitution throughout.
pub fn instantiate_theorem(
    theorem: &TheoremSchema,
    subst: &Substitution,
) -> Option<(Vec<Proposition>, Proposition)> {
    // Check all binders are bound
    for binder in &theorem.binders {
        if subst.get(shared_var(binder)).is_none() {
            return None;
        }
    }

    // Apply substitution to premises
    let premises: Vec<Proposition> = theorem
        .premises
        .iter()
        .map(|p| substitute_proposition(p, subst))
        .collect();

    // Apply substitution to conclusion
    let conclusion = substitute_proposition(&theorem.conclusion, subst);

    Some((premises, conclusion))
}

// ═══════════════════════════════════════════════════════════════════
// INITIAL TRUSTED THEOREMS
// ═══════════════════════════════════════════════════════════════════

/// Build the initial set of 12 hand-curated theorem schemas.
///
/// These are all ground truths — no auto-conversion from the 18k
/// formula database. They form a minimal trusted environment for
/// the proof kernel.
pub fn initial_theorems() -> Vec<TheoremSchema> {
    let s = |name: &str| SymExpr::Var(shared_var(name));
    let n = |v: f64| SymExpr::Num(v);

    vec![
        // 1. eq_reflexive: ∀x, x = x
        TheoremSchema::new(
            TheoremId(1),
            "eq_reflexive",
            vec!["x".to_string()],
            vec![],
            Proposition::eq(s("x"), s("x")),
            TheoremTrust::LogicalPrimitive,
        ),
        // 2. eq_symmetric: ∀a ∀b, a = b → b = a
        TheoremSchema::new(
            TheoremId(2),
            "eq_symmetric",
            vec!["a".to_string(), "b".to_string()],
            vec![Proposition::eq(s("a"), s("b"))],
            Proposition::eq(s("b"), s("a")),
            TheoremTrust::LogicalPrimitive,
        ),
        // 3. eq_transitive: ∀a ∀b ∀c, a = b ∧ b = c → a = c
        TheoremSchema::new(
            TheoremId(3),
            "eq_transitive",
            vec!["a".to_string(), "b".to_string(), "c".to_string()],
            vec![Proposition::and(
                Proposition::eq(s("a"), s("b")),
                Proposition::eq(s("b"), s("c")),
            )],
            Proposition::eq(s("a"), s("c")),
            TheoremTrust::LogicalPrimitive,
        ),
        // 4. sqrt_square_nonnegative: ∀x, x ≥ 0 → sqrt(x²) = x
        TheoremSchema::new(
            TheoremId(4),
            "sqrt_square_nonnegative",
            vec!["x".to_string()],
            vec![Proposition::ge(s("x"), n(0.0))],
            Proposition::eq(
                SymExpr::Sqrt(Box::new(SymExpr::Pow(Box::new(s("x")), Box::new(n(2.0))))),
                s("x"),
            ),
            TheoremTrust::CuratedAxiom,
        ),
        // 5. abs_of_nonnegative: ∀x, x ≥ 0 → |x| = x
        TheoremSchema::new(
            TheoremId(5),
            "abs_of_nonnegative",
            vec!["x".to_string()],
            vec![Proposition::ge(s("x"), n(0.0))],
            Proposition::eq(SymExpr::Abs(Box::new(s("x"))), s("x")),
            TheoremTrust::CuratedAxiom,
        ),
        // 6. abs_of_negative: ∀x, x < 0 → |x| = -x
        TheoremSchema::new(
            TheoremId(6),
            "abs_of_negative",
            vec!["x".to_string()],
            vec![Proposition::lt(s("x"), n(0.0))],
            Proposition::eq(
                SymExpr::Abs(Box::new(s("x"))),
                SymExpr::Neg(Box::new(s("x"))),
            ),
            TheoremTrust::CuratedAxiom,
        ),
        // 7. add_zero: ∀x, x + 0 = x
        TheoremSchema::new(
            TheoremId(7),
            "add_zero",
            vec!["x".to_string()],
            vec![],
            Proposition::eq(s("x") + n(0.0), s("x")),
            TheoremTrust::CuratedAxiom,
        ),
        // 8. mul_one: ∀x, x · 1 = x
        TheoremSchema::new(
            TheoremId(8),
            "mul_one",
            vec!["x".to_string()],
            vec![],
            Proposition::eq(s("x") * n(1.0), s("x")),
            TheoremTrust::CuratedAxiom,
        ),
        // 9. mul_zero: ∀x, x · 0 = 0
        TheoremSchema::new(
            TheoremId(9),
            "mul_zero",
            vec!["x".to_string()],
            vec![],
            Proposition::eq(s("x") * n(0.0), n(0.0)),
            TheoremTrust::CuratedAxiom,
        ),
        // 10. add_comm: ∀a ∀b, a + b = b + a
        TheoremSchema::new(
            TheoremId(10),
            "add_comm",
            vec!["a".to_string(), "b".to_string()],
            vec![],
            Proposition::eq(s("a") + s("b"), s("b") + s("a")),
            TheoremTrust::CuratedAxiom,
        ),
        // 11. mul_comm: ∀a ∀b, a · b = b · a
        TheoremSchema::new(
            TheoremId(11),
            "mul_comm",
            vec!["a".to_string(), "b".to_string()],
            vec![],
            Proposition::eq(s("a") * s("b"), s("b") * s("a")),
            TheoremTrust::CuratedAxiom,
        ),
        // 12. distribute: ∀a ∀b ∀c, a·(b + c) = a·b + a·c
        TheoremSchema::new(
            TheoremId(12),
            "distribute",
            vec!["a".to_string(), "b".to_string(), "c".to_string()],
            vec![],
            Proposition::eq(
                s("a") * (s("b") + s("c")),
                (s("a") * s("b")) + (s("a") * s("c")),
            ),
            TheoremTrust::CuratedAxiom,
        ),
    ]
}

// ═══════════════════════════════════════════════════════════════════
// THEOREM ENVIRONMENT
// ═══════════════════════════════════════════════════════════════════

/// The trusted theorem environment — only contains hand-curated or
/// kernel-proved theorems. Legacy formulas (18k from Wikipedia) are
/// NOT added here; they remain in `KnowledgeStatus::Legacy`.
#[derive(Clone, Debug)]
pub struct TheoremEnvironment {
    theorems: Vec<TheoremSchema>,
    by_id: std::collections::HashMap<TheoremId, usize>,
    by_name: std::collections::HashMap<String, usize>,
    /// Index by head symbol of conclusion LHS for fast lookup
    by_head: std::collections::HashMap<String, Vec<usize>>,
}

impl TheoremEnvironment {
    pub fn new() -> Self {
        TheoremEnvironment {
            theorems: Vec::new(),
            by_id: std::collections::HashMap::new(),
            by_name: std::collections::HashMap::new(),
            by_head: std::collections::HashMap::new(),
        }
    }

    /// Build from the initial theorem set.
    pub fn with_initial_theorems() -> Self {
        let mut env = Self::new();
        for theorem in initial_theorems() {
            env.add(theorem);
        }
        env
    }

    /// Add a theorem to the environment.
    pub fn add(&mut self, theorem: TheoremSchema) {
        let idx = self.theorems.len();
        self.by_id.insert(theorem.id, idx);
        self.by_name.insert(theorem.name.clone(), idx);

        // Index by conclusion type and content
        let heads = conclusion_heads(&theorem.conclusion);
        for head in heads {
            self.by_head.entry(head).or_default().push(idx);
        }
        // Also index by premise predicates (so "≥" goals can find theorems
        // with Ge premises that become subgoals)
        for premise in &theorem.premises {
            match premise {
                Proposition::Ge(_, _) => {
                    self.by_head.entry("≥".to_string()).or_default().push(idx);
                }
                Proposition::Lt(_, _) => {
                    self.by_head.entry("<".to_string()).or_default().push(idx);
                }
                Proposition::Le(_, _) => {
                    self.by_head.entry("≤".to_string()).or_default().push(idx);
                }
                Proposition::Gt(_, _) => {
                    self.by_head.entry(">".to_string()).or_default().push(idx);
                }
                Proposition::NotEq(_, _) => {
                    self.by_head.entry("≠".to_string()).or_default().push(idx);
                }
                Proposition::And(_, _) => {
                    self.by_head.entry("∧".to_string()).or_default().push(idx);
                }
                _ => {}
            }
        }

        self.theorems.push(theorem);
    }

    /// Look up a theorem by ID.
    pub fn get_by_id(&self, id: TheoremId) -> Option<&TheoremSchema> {
        self.by_id.get(&id).map(|&idx| &self.theorems[idx])
    }

    /// Look up a theorem by name.
    pub fn get_by_name(&self, name: &str) -> Option<&TheoremSchema> {
        self.by_name.get(name).map(|&idx| &self.theorems[idx])
    }

    /// Find theorems whose conclusion's LHS head symbol matches.
    pub fn find_by_head(&self, head: &str) -> Vec<&TheoremSchema> {
        self.by_head
            .get(head)
            .map(|indices| indices.iter().map(|&i| &self.theorems[i]).collect())
            .unwrap_or_default()
    }

    /// Return all theorems.
    pub fn all(&self) -> &[TheoremSchema] {
        &self.theorems
    }

    /// Number of theorems.
    pub fn len(&self) -> usize {
        self.theorems.len()
    }

    /// Return true if empty.
    pub fn is_empty(&self) -> bool {
        self.theorems.is_empty()
    }
}

/// Extract the "head symbol" of a SymExpr for indexing purposes.
/// E.g., `sqrt(x²)` → "sqrt", `x + y` → "+", `x` → "x".
pub fn head_symbol(expr: &SymExpr) -> String {
    use SymExpr::*;
    match expr {
        Num(_) => "Num".to_string(),
        Var(name) => name.to_string(),
        Add(_, _) => "+".to_string(),
        Sub(_, _) => "-".to_string(),
        Mul(_, _) => "*".to_string(),
        Div(_, _) => "/".to_string(),
        Pow(_, _) => "^".to_string(),
        Neg(_) => "neg".to_string(),
        Sin(_) => "sin".to_string(),
        Cos(_) => "cos".to_string(),
        Tan(_) => "tan".to_string(),
        Sqrt(_) => "sqrt".to_string(),
        Exp(_) => "exp".to_string(),
        Ln(_) => "ln".to_string(),
        Abs(_) => "abs".to_string(),
        Sinh(_) => "sinh".to_string(),
        Cosh(_) => "cosh".to_string(),
        Tanh(_) => "tanh".to_string(),
        Asin(_) => "asin".to_string(),
        Acos(_) => "acos".to_string(),
        Atan(_) => "atan".to_string(),
        Limit { .. } => "limit".to_string(),
        Integral { .. } => "integral".to_string(),
    }
}

/// Extract indexing heads from a conclusion proposition.
/// Returns multiple keys to allow flexible matching:
/// - For Eq(lhs, rhs): the head symbol of lhs (e.g., "sqrt", "+")
/// - For Ge/Le/Gt/Lt: the relation symbol (e.g., "≥", "≤")
/// - For Not/And/Or/Implies: the connective symbol
/// - For Predicate: the predicate name
/// - Also the LHS head for Eq (most common case)
pub fn conclusion_heads(p: &Proposition) -> Vec<String> {
    use Proposition::*;
    match p {
        Eq(lhs, _) => {
            let mut heads = vec![head_symbol(lhs)];
            // Also add "=" as a generic equality key
            heads.push("=".to_string());
            heads
        }
        Ge(_, _) => vec!["≥".to_string()],
        Le(_, _) => vec!["≤".to_string()],
        Gt(_, _) => vec![">".to_string()],
        Lt(_, _) => vec!["<".to_string()],
        NotEq(_, _) => vec!["≠".to_string()],
        Not(_) => vec!["¬".to_string()],
        And(_, _) => vec!["∧".to_string()],
        Or(_, _) => vec!["∨".to_string()],
        Implies(_, _) => vec!["→".to_string()],
        Iff(_, _) => vec!["↔".to_string()],
        ForAll(_, _) => vec!["∀".to_string()],
        Exists(_, _) => vec!["∃".to_string()],
        Predicate { symbol, .. } => vec![symbol.clone()],
        True => vec!["⊤".to_string()],
        False => vec!["⊥".to_string()],
    }
}

// ═══════════════════════════════════════════════════════════════════
// LOCAL CONTEXT (Proof State)
// ═══════════════════════════════════════════════════════════════════

/// A local context for proof checking: tracks hypotheses available
/// to close goals.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct LocalContext {
    pub hypotheses: Vec<(HypothesisId, Proposition)>,
    next_hypothesis_id: u64,
}

impl LocalContext {
    pub fn new() -> Self {
        LocalContext {
            hypotheses: Vec::new(),
            next_hypothesis_id: 1,
        }
    }

    /// Add a hypothesis and return its ID.
    pub fn add_hypothesis(&mut self, p: Proposition) -> HypothesisId {
        let id = HypothesisId(self.next_hypothesis_id);
        self.next_hypothesis_id += 1;
        self.hypotheses.push((id, p));
        id
    }

    /// Find a hypothesis that propositionally matches the given proposition
    /// (structural equality, not unification). Returns the ID if found.
    pub fn find_exact(&self, p: &Proposition) -> Option<HypothesisId> {
        self.hypotheses
            .iter()
            .find(|(_, h)| h == p)
            .map(|(id, _)| *id)
    }

    /// Find a hypothesis that unifies with the given proposition.
    /// Returns (hypothesis_id, substitution) if found.
    pub fn find_unifying(&self, p: &Proposition) -> Option<(HypothesisId, Substitution)> {
        for (id, h) in &self.hypotheses {
            if let Some(subst) = unify_propositions(h, p) {
                return Some((*id, subst));
            }
        }
        None
    }

    /// Number of hypotheses.
    pub fn len(&self) -> usize {
        self.hypotheses.len()
    }

    /// True if no hypotheses.
    pub fn is_empty(&self) -> bool {
        self.hypotheses.is_empty()
    }
}

// ═══════════════════════════════════════════════════════════════════
// UNIFICATION — TERM UNIFICATION WITH OCCURS CHECK
// ═══════════════════════════════════════════════════════════════════

/// Fully dereference a term through the substitution chain.
/// After deref, no variable in the result appears in the substitution's domain.
fn deref_term(expr: &SymExpr, subst: &Substitution) -> SymExpr {
    use SymExpr::*;
    match expr {
        Var(name) => {
            if let Some(val) = subst.get(name) {
                deref_term(val, subst) // follow chain
            } else {
                Var(name.clone())
            }
        }
        Num(_) => expr.clone(),
        Add(a, b) => Add(
            Box::new(deref_term(a, subst)),
            Box::new(deref_term(b, subst)),
        ),
        Sub(a, b) => Sub(
            Box::new(deref_term(a, subst)),
            Box::new(deref_term(b, subst)),
        ),
        Mul(a, b) => Mul(
            Box::new(deref_term(a, subst)),
            Box::new(deref_term(b, subst)),
        ),
        Div(a, b) => Div(
            Box::new(deref_term(a, subst)),
            Box::new(deref_term(b, subst)),
        ),
        Pow(a, b) => Pow(
            Box::new(deref_term(a, subst)),
            Box::new(deref_term(b, subst)),
        ),
        Neg(a) => Neg(Box::new(deref_term(a, subst))),
        Sin(a) => Sin(Box::new(deref_term(a, subst))),
        Cos(a) => Cos(Box::new(deref_term(a, subst))),
        Tan(a) => Tan(Box::new(deref_term(a, subst))),
        Sqrt(a) => Sqrt(Box::new(deref_term(a, subst))),
        Exp(a) => Exp(Box::new(deref_term(a, subst))),
        Ln(a) => Ln(Box::new(deref_term(a, subst))),
        Abs(a) => Abs(Box::new(deref_term(a, subst))),
        Sinh(a) => Sinh(Box::new(deref_term(a, subst))),
        Cosh(a) => Cosh(Box::new(deref_term(a, subst))),
        Tanh(a) => Tanh(Box::new(deref_term(a, subst))),
        Asin(a) => Asin(Box::new(deref_term(a, subst))),
        Acos(a) => Acos(Box::new(deref_term(a, subst))),
        Atan(a) => Atan(Box::new(deref_term(a, subst))),
        Limit {
            variable,
            approach,
            body,
        } => Limit {
            variable: variable.clone(),
            approach: Box::new(deref_term(approach, subst)),
            body: Box::new(deref_term(body, subst)),
        },
        Integral {
            variable,
            lower,
            upper,
            body,
        } => Integral {
            variable: variable.clone(),
            lower: lower.as_ref().map(|l| Box::new(deref_term(l, subst))),
            upper: upper.as_ref().map(|u| Box::new(deref_term(u, subst))),
            body: Box::new(deref_term(body, subst)),
        },
    }
}

/// Check if a variable occurs free inside an expression (occurs check).
fn occurs_check(var: &str, expr: &SymExpr) -> bool {
    use SymExpr::*;
    match expr {
        Var(name) => name == var,
        Num(_) => false,
        Add(a, b) | Sub(a, b) | Mul(a, b) | Div(a, b) | Pow(a, b) => {
            occurs_check(var, a) || occurs_check(var, b)
        }
        Neg(a) | Sin(a) | Cos(a) | Tan(a) | Sqrt(a) | Exp(a) | Ln(a) | Abs(a) | Sinh(a)
        | Cosh(a) | Tanh(a) | Asin(a) | Acos(a) | Atan(a) => occurs_check(var, a),
        Limit { body, .. } | Integral { body, .. } => occurs_check(var, body),
    }
}

/// Internal mutable unification: unify two terms, accumulating bindings in `subst`.
/// Returns true on success, false on failure.
fn unify_terms_mut(a: &SymExpr, b: &SymExpr, subst: &mut Substitution) -> bool {
    use SymExpr::*;

    // Dereference both sides through the current substitution
    let a = deref_term(a, subst);
    let b = deref_term(b, subst);

    match (&a, &b) {
        // Same value or variable
        (Num(x), Num(y)) if (x - y).abs() < f64::EPSILON => true,
        (Var(x), Var(y)) if x == y => true,
        (Num(_), Num(_)) => false,

        // Bind variable to expression
        (Var(x), _) => bind_var(x, &b, subst),
        (_, Var(y)) => bind_var(y, &a, subst),

        // Structural matching
        (Add(a1, a2), Add(b1, b2))
        | (Sub(a1, a2), Sub(b1, b2))
        | (Mul(a1, a2), Mul(b1, b2))
        | (Div(a1, a2), Div(b1, b2))
        | (Pow(a1, a2), Pow(b1, b2)) => {
            unify_terms_mut(a1, b1, subst) && unify_terms_mut(a2, b2, subst)
        }

        (Neg(a1), Neg(b1))
        | (Sin(a1), Sin(b1))
        | (Cos(a1), Cos(b1))
        | (Tan(a1), Tan(b1))
        | (Sqrt(a1), Sqrt(b1))
        | (Exp(a1), Exp(b1))
        | (Ln(a1), Ln(b1))
        | (Abs(a1), Abs(b1))
        | (Sinh(a1), Sinh(b1))
        | (Cosh(a1), Cosh(b1))
        | (Tanh(a1), Tanh(b1))
        | (Asin(a1), Asin(b1))
        | (Acos(a1), Acos(b1))
        | (Atan(a1), Atan(b1)) => unify_terms_mut(a1, b1, subst),

        (
            Limit {
                variable: v1,
                approach: ap1,
                body: bd1,
            },
            Limit {
                variable: v2,
                approach: ap2,
                body: bd2,
            },
        ) => {
            // Limit variables are binding positions — rename one side
            if v1 != v2 {
                // Rename v1 → v2 in the body
                let mut rename_subst = Substitution::new();
                rename_subst.insert(v1.clone(), Var(v2.clone()));
                let renamed_body1 = substitute_term(bd1, &rename_subst);
                unify_terms_mut(ap1, ap2, subst) && unify_terms_mut(&renamed_body1, bd2, subst)
            } else {
                unify_terms_mut(ap1, ap2, subst) && unify_terms_mut(bd1, bd2, subst)
            }
        }

        (
            Integral {
                variable: v1,
                lower: l1,
                upper: u1,
                body: bd1,
            },
            Integral {
                variable: v2,
                lower: l2,
                upper: u2,
                body: bd2,
            },
        ) => {
            // Rename bound variable if needed
            let (bd1_renamed, l1_renamed, u1_renamed) = if v1 != v2 {
                let mut rename_subst = Substitution::new();
                rename_subst.insert(v1.clone(), Var(v2.clone()));
                (
                    substitute_term(bd1, &rename_subst),
                    l1.as_ref().map(|l| substitute_term(l, &rename_subst)),
                    u1.as_ref().map(|u| substitute_term(u, &rename_subst)),
                )
            } else {
                (
                    bd1.as_ref().clone(),
                    l1.as_ref().map(|l| l.as_ref().clone()),
                    u1.as_ref().map(|u| u.as_ref().clone()),
                )
            };
            match (&l1_renamed, l2) {
                (Some(la), Some(lb)) => {
                    if !unify_terms_mut(la, lb, subst) {
                        return false;
                    }
                }
                (None, None) => {}
                _ => return false,
            }
            match (&u1_renamed, u2) {
                (Some(ua), Some(ub)) => {
                    if !unify_terms_mut(ua, ub, subst) {
                        return false;
                    }
                }
                (None, None) => {}
                _ => return false,
            }
            unify_terms_mut(&bd1_renamed, bd2, subst)
        }

        // Anything else: no match
        _ => false,
    }
}

/// Try to bind a variable to an expression (with occurs check).
fn bind_var(var: &Variable, expr: &SymExpr, subst: &mut Substitution) -> bool {
    if META_ONLY_UNIFICATION.with(|mode| mode.get()) && var.kind != VariableKind::Meta {
        return matches!(expr, SymExpr::Var(other) if other == var);
    }
    // If already bound, unify the existing binding with the new expression
    if let Some(existing) = subst.get(var).cloned() {
        return unify_terms_mut(&existing, expr, subst);
    }
    // Occurs check: prevent x → f(x) circularity
    if occurs_check(var.display.as_ref(), expr) {
        return false;
    }
    subst.insert(var.clone(), expr.clone());
    true
}

/// Unify two symbolic terms and return the resulting substitution.
/// Returns `None` if unification fails.
pub fn unify_terms(a: &SymExpr, b: &SymExpr) -> Option<Substitution> {
    let mut subst = Substitution::new();
    if unify_terms_mut(a, b, &mut subst) {
        Some(subst)
    } else {
        None
    }
}

// ═══════════════════════════════════════════════════════════════════
// UNIFICATION — PROPOSITION UNIFICATION
// ═══════════════════════════════════════════════════════════════════

/// Internal mutable unification for propositions.
fn unify_propositions_mut(a: &Proposition, b: &Proposition, subst: &mut Substitution) -> bool {
    use Proposition::*;

    match (a, b) {
        (True, True) | (False, False) => true,

        // Eq is symmetric: unify either orientation
        (Eq(a1, a2), Eq(b1, b2)) => {
            // Try a1↔b1, a2↔b2 first
            let mut s1 = subst.clone();
            if unify_terms_mut(a1, b1, &mut s1) && unify_terms_mut(a2, b2, &mut s1) {
                *subst = s1;
                return true;
            }
            // Then try a1↔b2, a2↔b1 (flipped)
            let mut s2 = subst.clone();
            if unify_terms_mut(a1, b2, &mut s2) && unify_terms_mut(a2, b1, &mut s2) {
                *subst = s2;
                return true;
            }
            false
        }

        // Non-symmetric relations: straightforward pairwise
        (NotEq(a1, a2), NotEq(b1, b2))
        | (Lt(a1, a2), Lt(b1, b2))
        | (Le(a1, a2), Le(b1, b2))
        | (Gt(a1, a2), Gt(b1, b2))
        | (Ge(a1, a2), Ge(b1, b2)) => {
            unify_terms_mut(a1, b1, subst) && unify_terms_mut(a2, b2, subst)
        }

        // Predicates: match by symbol name, then unify args pairwise
        (
            Predicate {
                symbol: s1,
                args: args1,
            },
            Predicate {
                symbol: s2,
                args: args2,
            },
        ) => {
            if s1 != s2 || args1.len() != args2.len() {
                return false;
            }
            for (arg1, arg2) in args1.iter().zip(args2.iter()) {
                if !unify_terms_mut(arg1, arg2, subst) {
                    return false;
                }
            }
            true
        }

        // Logical connectives
        (Not(a1), Not(b1)) => unify_propositions_mut(a1, b1, subst),
        (And(a1, a2), And(b1, b2))
        | (Or(a1, a2), Or(b1, b2))
        | (Implies(a1, a2), Implies(b1, b2))
        | (Iff(a1, a2), Iff(b1, b2)) => {
            unify_propositions_mut(a1, b1, subst) && unify_propositions_mut(a2, b2, subst)
        }

        // Quantifiers: alpha-equivalence
        (
            ForAll(Binder::ForAll { variable: v1 }, b1),
            ForAll(Binder::ForAll { variable: v2 }, b2),
        )
        | (
            Exists(Binder::ForAll { variable: v1 }, b1),
            Exists(Binder::ForAll { variable: v2 }, b2),
        ) => {
            if v1 == v2 {
                unify_propositions_mut(b1, b2, subst)
            } else {
                // Alpha-rename: freshen v1 → v2 in body of b1
                let mut rename_subst = Substitution::new();
                rename_subst.insert(v1.clone(), SymExpr::Var(v2.clone()));
                let renamed_b1 = substitute_proposition(b1, &rename_subst);
                unify_propositions_mut(&renamed_b1, b2, subst)
            }
        }

        // Mismatch
        _ => false,
    }
}

/// Unify two propositions and return the resulting substitution.
/// Returns `None` if unification fails.
pub fn unify_propositions(a: &Proposition, b: &Proposition) -> Option<Substitution> {
    let mut subst = Substitution::new();
    if unify_propositions_mut(a, b, &mut subst) {
        Some(subst)
    } else {
        None
    }
}

/// Unify a freshened theorem against a goal.  Unlike the legacy public
/// unifier, rigid variables are constants: only `VariableKind::Meta` terms
/// may be bound.
fn unify_theorem_proposition(a: &Proposition, b: &Proposition) -> Option<MetaSubstitution> {
    META_ONLY_UNIFICATION.with(|mode| {
        let was_enabled = mode.replace(true);
        let result = unify_propositions(a, b);
        mode.set(was_enabled);
        result
    })
}

// ═══════════════════════════════════════════════════════════════════
// APPLY THEOREM — Backward chaining by unification
// ═══════════════════════════════════════════════════════════════════

/// The result of applying a theorem to a goal.
#[derive(Clone, Debug)]
pub struct ApplyResult {
    /// The theorem that was applied.
    pub theorem_id: TheoremId,
    /// The theorem's name.
    pub theorem_name: String,
    /// The substitution produced by unifying the theorem's conclusion with the goal.
    pub substitution: Substitution,
    /// Subgoals (the theorem's premises, instantiated through the substitution).
    pub subgoals: Vec<Proposition>,
}

/// Freshen a theorem's universally quantified binders, returning the
/// renamed binders, premises, and conclusion.
///
/// Each binder variable is renamed to a fresh name (e.g., `x` → `x_1`)
/// to avoid name collisions with the goal's variables.
fn freshen_binders(
    theorem: &TheoremSchema,
) -> (Vec<(String, Variable)>, Vec<Proposition>, Proposition) {
    let mut subst = Substitution::new();
    let mut new_binders = Vec::new();

    for binder in &theorem.binders {
        let fresh_name = format!("{}_{}", binder, next_fresh_suffix());
        let mut fresh = Variable::fresh_named(&fresh_name);
        fresh.kind = VariableKind::Meta;
        subst.insert(shared_var(binder), SymExpr::Var(fresh.clone()));
        new_binders.push((binder.clone(), fresh));
    }

    let premises: Vec<Proposition> = theorem
        .premises
        .iter()
        .map(|p| substitute_proposition(p, &subst))
        .collect();
    let conclusion = substitute_proposition(&theorem.conclusion, &subst);

    (new_binders, premises, conclusion)
}

/// Apply a theorem to a goal by unifying the theorem's conclusion
/// with the goal, then returning the instantiated premises as subgoals.
///
/// Returns `None` if the theorem's conclusion cannot be unified with the goal.
///
/// # Algorithm
///
/// The theorem binders are first replaced by fresh meta-variables.  Rigid
/// variables in the goal therefore cannot be accidentally assigned simply
/// because they share a display name with a schema binder.  The resulting
/// bindings are translated back to the schema binders before being stored in
/// the proof object, so kernel checking is deterministic and replayable.
///
/// # Example
///
/// Given theorem `∀x, x ≥ 0 → sqrt(x²) = x` and goal `sqrt(y²) = y`:
///
/// 1. Unify: `sqrt(x²) = x` with `sqrt(y²) = y` → `{x → y}`
/// 2. Apply to premises: `x ≥ 0` → `y ≥ 0`
/// 3. Subgoal: `y ≥ 0`
pub fn apply_theorem(theorem: &TheoremSchema, goal: &Proposition) -> Option<ApplyResult> {
    let (fresh_binders, _fresh_premises, fresh_conclusion) = freshen_binders(theorem);
    let meta_subst = unify_theorem_proposition(&fresh_conclusion, goal)?;

    // Every universal binder must resolve.  A theorem whose conclusion does
    // not constrain one of its binders cannot be instantiated safely here.
    let mut unification_subst = Substitution::new();
    for (original_name, fresh_meta) in fresh_binders {
        let value = meta_subst.get(&fresh_meta)?.clone();
        unification_subst.insert(shared_var(&original_name), deref_term(&value, &meta_subst));
    }

    // Step 3: Apply the substitution to the premises to get subgoals
    let subgoals: Vec<Proposition> = theorem
        .premises
        .iter()
        .map(|p| substitute_proposition(p, &unification_subst))
        .collect();

    Some(ApplyResult {
        theorem_id: theorem.id,
        theorem_name: theorem.name.clone(),
        substitution: unification_subst,
        subgoals,
    })
}
#[cfg(test)]
mod tests {
    use super::*;
    
    
    
    use SymExpr::*;

    // ── Shared variable cache for tests ────────────────────────
    // Uses the same cache as initial_theorems() so VarIds match.
    use crate::proposition::shared_var;

    /// Assert two propositions are structurally equal (comparing by display name,
    /// ignoring VarId differences from different Variable constructors).
    fn assert_prop_eq(a: &Proposition, b: &Proposition, msg: &str) {
        assert_eq!(format!("{}", a), format!("{}", b), "{}", msg);
    }

    // ── Helper: make variables using shared cache ──────────────
    fn v(name: &str) -> SymExpr {
        Var(shared_var(name))
    }

    /// Create a ForAll binder using the shared variable cache.
    fn forall_v(name: &str, body: Proposition) -> Proposition {
        let var = shared_var(name);
        Proposition::forall(&var, body)
    }

    /// Create an Exists binder using the shared variable cache.
    fn exists_v(name: &str, body: Proposition) -> Proposition {
        let var = shared_var(name);
        Proposition::exists(&var, body)
    }

    fn num(v: f64) -> SymExpr {
        Num(v)
    }

    #[test]
    fn test_free_vars_term_simple() {
        let expr = v("x") + v("y");
        let mut vars = free_vars_term(&expr);
        let mut expected: HashSet<String> = ["x".to_string(), "y".to_string()].into();
        assert_eq!(vars, expected);

        // Constant has no vars
        assert!(free_vars_term(&num(42.0)).is_empty());
    }

    #[test]
    fn test_free_vars_term_nested() {
        let expr = Sqrt(Box::new(Pow(Box::new(v("x")), Box::new(num(2.0)))));
        let vars = free_vars_term(&expr);
        let expected: HashSet<String> = ["x".to_string()].into();
        assert_eq!(vars, expected);
    }

    #[test]
    fn test_free_vars_proposition_no_quantifiers() {
        let p = Proposition::eq(v("x"), v("y"));
        let vars = p.free_vars();
        let expected: HashSet<String> = ["x".to_string(), "y".to_string()].into();
        assert_eq!(vars, expected);
    }

    #[test]
    fn test_free_vars_proposition_with_quantifier() {
        // ∀x, x ≥ 0 → sqrt(x²) = x
        let p = forall_v(
            "x",
            Proposition::implies(
                Proposition::ge(v("x"), num(0.0)),
                Proposition::eq(
                    Sqrt(Box::new(Pow(Box::new(v("x")), Box::new(num(2.0))))),
                    v("x"),
                ),
            ),
        );
        let vars = p.free_vars();
        assert!(vars.is_empty(), "∀x quantifier should bind x: {:?}", vars);
    }

    #[test]
    fn test_free_vars_proposition_free_in_quantifier() {
        // ∀x, x ≥ y → sqrt(x²) = x  (y is free!)
        let p = forall_v(
            "x",
            Proposition::implies(
                Proposition::ge(v("x"), v("y")),
                Proposition::eq(
                    Sqrt(Box::new(Pow(Box::new(v("x")), Box::new(num(2.0))))),
                    v("x"),
                ),
            ),
        );
        let vars = p.free_vars();
        let expected: HashSet<String> = ["y".to_string()].into();
        assert_eq!(vars, expected);
    }

    // ── Substitution tests ────────────────────────────────────

    #[test]
    fn test_substitute_term_simple() {
        let mut subst = Substitution::new();
        subst.insert("x".to_string(), num(5.0));

        let expr = v("x") + v("y");
        let result = substitute_term(&expr, &subst);
        assert_eq!(result, num(5.0) + v("y"));
    }

    #[test]
    fn test_substitute_term_nested() {
        let mut subst = Substitution::new();
        subst.insert("x".to_string(), num(3.0));

        let expr = Pow(Box::new(v("x")), Box::new(num(2.0)));
        let result = substitute_term(&expr, &subst);
        assert_eq!(result, Pow(Box::new(num(3.0)), Box::new(num(2.0))));
    }

    #[test]
    fn test_substitute_proposition_eq() {
        let mut subst = Substitution::new();
        subst.insert("x".to_string(), num(5.0));

        let p = Proposition::eq(v("x"), v("y"));
        let result = substitute_proposition(&p, &subst);
        assert_eq!(result, Proposition::eq(num(5.0), v("y")));
    }

    #[test]
    fn test_substitute_proposition_ge() {
        let mut subst = Substitution::new();
        subst.insert("x".to_string(), num(3.0));

        let p = Proposition::ge(v("x"), num(0.0));
        let result = substitute_proposition(&p, &subst);
        assert_eq!(result, Proposition::ge(num(3.0), num(0.0)));
    }

    #[test]
    fn test_substitute_proposition_implies() {
        let mut subst = Substitution::new();
        subst.insert("x".to_string(), num(7.0));

        let p = Proposition::implies(
            Proposition::ge(v("x"), num(0.0)),
            Proposition::eq(
                Sqrt(Box::new(Pow(Box::new(v("x")), Box::new(num(2.0))))),
                v("x"),
            ),
        );
        let result = substitute_proposition(&p, &subst);
        let expected = Proposition::implies(
            Proposition::ge(num(7.0), num(0.0)),
            Proposition::eq(
                Sqrt(Box::new(Pow(Box::new(num(7.0)), Box::new(num(2.0))))),
                num(7.0),
            ),
        );
        // Compare by display (structural equality ignoring VarIds)
        assert_eq!(
            format!("{}", result),
            format!("{}", expected),
            "Capture-avoiding substitution should produce expected structure"
        );
        let mut subst = Substitution::new();
        subst.insert("x".to_string(), num(5.0));

        // ∀y, x + y = y + x  (x free, y bound)
        let p = forall_v("y", Proposition::eq(v("x") + v("y"), v("y") + v("x")));
        let result = substitute_proposition(&p, &subst);

        // ∀y, 5 + y = y + 5  (no capture needed: y not introduced by subst)
        let expected = forall_v("y", Proposition::eq(num(5.0) + v("y"), v("y") + num(5.0)));
        assert_eq!(result, expected);
    }

    #[test]
    fn test_substitute_proposition_forall_capture_detected_and_avoided() {
        reset_fresh_counter();

        let mut subst = Substitution::new();
        // Substituting x → f(y): f(y) introduces the variable "y"
        subst.insert("x".to_string(), Sin(Box::new(v("y"))));

        // ∀y, P(x, y) — the bound variable "y" conflicts with introduced "y"
        let p = forall_v("y", Proposition::ge(v("x"), v("y")));
        let result = substitute_proposition(&p, &subst);

        // Should produce: ∀y_1, sin(y) ≥ y_1  (bound y freshened to y_1)
        let expected = forall_v("y_1", Proposition::ge(Sin(Box::new(v("y"))), v("y_1")));
        assert_prop_eq(&result, &expected, "Capture-avoiding substitution");
    }

    #[test]
    fn test_substitute_proposition_no_capture_when_no_conflict() {
        let mut subst = Substitution::new();
        subst.insert("x".to_string(), num(10.0));

        // ∀y, x ≥ y → (x free, y bound — no conflict since subst doesn't introduce y)
        let p = forall_v(
            "y",
            Proposition::implies(
                Proposition::ge(v("x"), v("y")),
                Proposition::eq(
                    Sqrt(Box::new(Pow(Box::new(v("x")), Box::new(num(2.0))))),
                    v("x"),
                ),
            ),
        );
        let result = substitute_proposition(&p, &subst);

        let expected = forall_v(
            "y",
            Proposition::implies(
                Proposition::ge(num(10.0), v("y")),
                Proposition::eq(
                    Sqrt(Box::new(Pow(Box::new(num(10.0)), Box::new(num(2.0))))),
                    num(10.0),
                ),
            ),
        );
        assert_eq!(result, expected);
    }

    #[test]
    fn test_instantiate_theorem_sqrt() {
        let theorems = initial_theorems();
        let sqrt_theorem = theorems
            .iter()
            .find(|t| t.name == "sqrt_square_nonnegative")
            .unwrap();

        let mut subst = Substitution::new();
        subst.insert("x".to_string(), v("y"));

        let (premises, conclusion) = instantiate_theorem(sqrt_theorem, &subst).unwrap();

        // Premise: y ≥ 0
        assert_eq!(premises.len(), 1);
        assert_eq!(premises[0], Proposition::ge(v("y"), num(0.0)));

        // Conclusion: sqrt(y²) = y
        let expected_conclusion = Proposition::eq(
            Sqrt(Box::new(Pow(Box::new(v("y")), Box::new(num(2.0))))),
            v("y"),
        );
        assert_eq!(conclusion, expected_conclusion);
    }

    #[test]
    fn test_instantiate_theorem_missing_binder() {
        let theorems = initial_theorems();
        let sqrt_theorem = theorems
            .iter()
            .find(|t| t.name == "sqrt_square_nonnegative")
            .unwrap();

        let subst = Substitution::new(); // empty — missing binder x
        assert!(instantiate_theorem(sqrt_theorem, &subst).is_none());
    }

    #[test]
    fn test_theorem_as_proposition() {
        let theorems = initial_theorems();
        let sqrt_theorem = theorems
            .iter()
            .find(|t| t.name == "sqrt_square_nonnegative")
            .unwrap();

        let prop = sqrt_theorem.as_proposition();
        let expected = forall_v(
            "x",
            Proposition::implies(
                Proposition::ge(v("x"), num(0.0)),
                Proposition::eq(
                    Sqrt(Box::new(Pow(Box::new(v("x")), Box::new(num(2.0))))),
                    v("x"),
                ),
            ),
        );
        // Compare structurally by display name since VarIds may differ
        assert_eq!(
            format!("{}", prop),
            format!("{}", expected),
            "Proposition structure should match"
        );
    }

    #[test]
    fn test_initial_theorems_count() {
        let theorems = initial_theorems();
        assert_eq!(theorems.len(), 12);
    }

    #[test]
    fn test_theorem_environment_lookup() {
        let env = TheoremEnvironment::with_initial_theorems();
        assert_eq!(env.len(), 12);

        let t = env.get_by_name("sqrt_square_nonnegative").unwrap();
        assert_eq!(t.name, "sqrt_square_nonnegative");
        assert_eq!(t.id, TheoremId(4));

        let t2 = env.get_by_id(TheoremId(7)).unwrap();
        assert_eq!(t2.name, "add_zero");
    }

    #[test]
    fn test_theorem_environment_head_index() {
        let env = TheoremEnvironment::with_initial_theorems();

        // sqrt_square_nonnegative has head "sqrt"
        let sqrt_theorems = env.find_by_head("sqrt");
        assert_eq!(sqrt_theorems.len(), 1);
        assert_eq!(sqrt_theorems[0].name, "sqrt_square_nonnegative");

        // add_comm has head "+"
        let add_theorems = env.find_by_head("+");
        assert!(!add_theorems.is_empty());
    }

    #[test]
    fn test_all_free_vars() {
        // Test free_vars across multiple Proposition variants
        let p = Proposition::and(
            Proposition::eq(v("x"), v("y")),
            Proposition::ge(v("z"), num(0.0)),
        );
        let vars = free_vars_proposition(&p);
        let expected: HashSet<String> = ["x".to_string(), "y".to_string(), "z".to_string()].into();
        assert_eq!(vars, expected);
    }

    #[test]
    fn test_head_symbol() {
        assert_eq!(head_symbol(&Sqrt(Box::new(v("x")))), "sqrt");
        assert_eq!(head_symbol(&(v("a") + v("b"))), "+");
        assert_eq!(head_symbol(&v("x")), "x");
        assert_eq!(head_symbol(&num(42.0)), "Num");
    }

    /// Test: capture safety is maintained with nested quantifiers
    #[test]
    fn test_nested_quantifier_capture_safety() {
        reset_fresh_counter();

        let mut subst = Substitution::new();
        subst.insert("x".to_string(), v("z"));

        // ∀y, ∀x, x ≥ y → (inner x shadows outer, should still work)
        // Substituting x → z should NOT affect the inner x (it's bound)
        let p = forall_v("y", forall_v("x", Proposition::ge(v("x"), v("y"))));

        let result = substitute_proposition(&p, &subst);

        // Before substitution: ∀y, ∀x, x ≥ y
        // After: ∀y₁, ∀x, x ≥ y₁
        //   — outer y is fine (no conflict with "z")
        //   — inner x stays as-is (bound, not captured)
        let expected = forall_v("y", forall_v("x", Proposition::ge(v("x"), v("y"))));
        assert_eq!(result, expected);
    }

    /// Test: substituting into predicate propositions
    #[test]
    fn test_substitute_predicate() {
        let mut subst = Substitution::new();
        subst.insert("x".to_string(), num(1.0));

        let p = Proposition::predicate("Real", vec![v("x")]);
        let result = substitute_proposition(&p, &subst);
        assert_eq!(result, Proposition::predicate("Real", vec![num(1.0)]));
    }

    /// Test: substitution into Not/And/Or/Iff
    #[test]
    fn test_substitute_logical_connectives() {
        let mut subst = Substitution::new();
        subst.insert("x".to_string(), num(1.0));
        subst.insert("y".to_string(), num(2.0));

        // ¬(x = y) ∧ (x < y ∨ x > y)
        let p = Proposition::and(
            Proposition::not(Proposition::eq(v("x"), v("y"))),
            Proposition::or(
                Proposition::lt(v("x"), v("y")),
                Proposition::gt(v("x"), v("y")),
            ),
        );
        let result = substitute_proposition(&p, &subst);
        let expected = Proposition::and(
            Proposition::not(Proposition::eq(num(1.0), num(2.0))),
            Proposition::or(
                Proposition::lt(num(1.0), num(2.0)),
                Proposition::gt(num(1.0), num(2.0)),
            ),
        );
        assert_eq!(result, expected);
    }

    /// Test: capture safety with exists
    #[test]
    fn test_exists_capture_safety() {
        reset_fresh_counter();

        let mut subst = Substitution::new();
        subst.insert("x".to_string(), v("y"));

        // ∃y, x > y → should freshen inner y
        let p = exists_v("y", Proposition::gt(v("x"), v("y")));
        let result = substitute_proposition(&p, &subst);

        let expected = exists_v("y_1", Proposition::gt(v("y"), v("y_1")));
        assert_prop_eq(&result, &expected, "Exists capture safety");
    }

    #[test]
    fn test_instantiate_no_premises() {
        let theorems = initial_theorems();
        let t = theorems.iter().find(|t| t.name == "add_zero").unwrap();
        assert!(t.premises.is_empty());

        let mut subst = Substitution::new();
        subst.insert("x".to_string(), v("z"));
        let (premises, conclusion) = instantiate_theorem(t, &subst).unwrap();
        assert!(premises.is_empty());
        assert_eq!(conclusion, Proposition::eq(v("z") + num(0.0), v("z")));
    }

    /// Test: substitution preserves equality structure (transitive rule)
    #[test]
    fn test_substitute_transitive() {
        let theorems = initial_theorems();
        let t = theorems.iter().find(|t| t.name == "eq_transitive").unwrap();

        let mut subst = Substitution::new();
        subst.insert("a".to_string(), v("x"));
        subst.insert("b".to_string(), v("y"));
        subst.insert("c".to_string(), v("z"));

        let (premises, conclusion) = instantiate_theorem(t, &subst).unwrap();
        assert_eq!(premises.len(), 1);
        assert_eq!(
            premises[0],
            Proposition::and(
                Proposition::eq(v("x"), v("y")),
                Proposition::eq(v("y"), v("z")),
            )
        );
        assert_eq!(conclusion, Proposition::eq(v("x"), v("z")));
    }

    /// Test: Display output is readable
    #[test]
    fn test_display_proposition() {
        let p = forall_v(
            "x",
            Proposition::implies(
                Proposition::ge(v("x"), num(0.0)),
                Proposition::eq(
                    Sqrt(Box::new(Pow(Box::new(v("x")), Box::new(num(2.0))))),
                    v("x"),
                ),
            ),
        );
        let display_str = format!("{}", p);
        assert!(display_str.contains("∀x"));
        assert!(display_str.contains("≥"));
        assert!(display_str.contains("sqrt"));
    }

    #[test]
    fn test_display_substitution() {
        let mut subst = Substitution::new();
        subst.insert("x".to_string(), num(5.0));
        subst.insert("y".to_string(), Sin(Box::new(v("z"))));
        let display_str = format!("{}", subst);
        assert!(display_str.contains("x →"));
        assert!(display_str.contains("y →"));
    }

    #[test]
    fn test_display_theorem_schema() {
        let theorems = initial_theorems();
        let t = &theorems[0];
        let display_str = format!("{}", t);
        assert!(display_str.contains("eq_reflexive"));
        assert!(display_str.contains("∀x"));
    }

    #[test]
    fn test_initial_theorems_have_unique_ids() {
        let theorems = initial_theorems();
        let ids: std::collections::HashSet<TheoremId> = theorems.iter().map(|t| t.id).collect();
        assert_eq!(ids.len(), theorems.len());
    }

    // ═══════════════════════════════════════════════════════════
    // UNIFICATION TESTS
    // ═══════════════════════════════════════════════════════════

    #[test]
    fn test_unify_terms_identical_vars() {
        let subst = unify_terms(&v("x"), &v("x")).unwrap();
        assert!(subst.is_empty());
    }

    #[test]
    fn test_unify_terms_constants_equal() {
        let subst = unify_terms(&num(42.0), &num(42.0)).unwrap();
        assert!(subst.is_empty());
    }

    #[test]
    fn test_unify_terms_constants_not_equal() {
        assert!(unify_terms(&num(1.0), &num(2.0)).is_none());
    }

    #[test]
    fn test_unify_terms_bind_var_to_const() {
        let subst = unify_terms(&v("x"), &num(5.0)).unwrap();
        assert_eq!(subst.get("x"), Some(&num(5.0)));
    }

    #[test]
    fn test_unify_terms_bind_both_same() {
        let subst = unify_terms(&v("x"), &v("y")).unwrap();
        assert_eq!(subst.get("x"), Some(&v("y")));
    }

    #[test]
    fn test_unify_terms_occurs_check_fails() {
        // x and f(x) should NOT unify (circular)
        assert!(unify_terms(&v("x"), &Sin(Box::new(v("x")))).is_none());
    }

    #[test]
    fn test_unify_terms_nested_structure() {
        // sqrt(x²) unifies with sqrt(y²) → {x → y}
        let a = Sqrt(Box::new(Pow(Box::new(v("x")), Box::new(num(2.0)))));
        let b = Sqrt(Box::new(Pow(Box::new(v("y")), Box::new(num(2.0)))));
        let subst = unify_terms(&a, &b).unwrap();
        assert_eq!(subst.get("x"), Some(&v("y")));
    }

    /// THE KEY TEST: unify sqrt(y²) with sqrt(x²), get x ↦ y
    #[test]
    fn test_unify_sqrt_square() {
        // sqrt(x²) with sqrt(y²) should give x ↦ y
        let a = Sqrt(Box::new(Pow(Box::new(v("x")), Box::new(num(2.0)))));
        let b = Sqrt(Box::new(Pow(Box::new(v("y")), Box::new(num(2.0)))));
        let subst = unify_terms(&a, &b).unwrap();
        assert_eq!(subst.get("x"), Some(&v("y")));

        // Also the other direction: y ↦ x
        let subst2 = unify_terms(&b, &a).unwrap();
        assert_eq!(subst2.get("y"), Some(&v("x")));
    }

    #[test]
    fn test_unify_terms_add() {
        // (x + 5) with (3 + y) → {x → 3, y → 5}
        let a = v("x") + num(5.0);
        let b = num(3.0) + v("y");
        let subst = unify_terms(&a, &b).unwrap();
        assert_eq!(subst.get("x"), Some(&num(3.0)));
        assert_eq!(subst.get("y"), Some(&num(5.0)));
    }

    #[test]
    fn test_unify_terms_mul() {
        // (x * 2) with (y * z) → {x → y, z → 2} etc
        let a = v("x") * num(2.0);
        let b = v("y") * v("z");
        let subst = unify_terms(&a, &b).unwrap();
        assert_eq!(subst.get("x"), Some(&v("y")));
        assert_eq!(subst.get("z"), Some(&num(2.0)));
    }

    #[test]
    fn test_unify_terms_diff_constructors() {
        // sin(x) and cos(x) should not unify
        assert!(unify_terms(&Sin(Box::new(v("x"))), &Cos(Box::new(v("x")))).is_none());
    }

    #[test]
    fn test_unify_terms_transitive_binding() {
        // Unify a with b, then something that implies b = c
        // First: a = b → {a ↦ b}
        // Then: f(a) with f(c) → {a ↦ c} → merge gives {a ↦ c, b ↦ c} via deref
        let subst = unify_terms(&Sin(Box::new(v("a"))), &Sin(Box::new(v("c")))).unwrap();
        assert_eq!(subst.get("a"), Some(&v("c")));
    }

    #[test]
    fn test_unify_terms_chain() {
        // x with y, then later y with 5 → {x → 5, y → 5}
        let mut subst = Substitution::new();
        assert!(unify_terms_mut(&v("x"), &v("y"), &mut subst));
        assert!(unify_terms_mut(&v("y"), &num(5.0), &mut subst));
        // After dereferencing, x should be 5
        let x_val = deref_term(&v("x"), &subst);
        let y_val = deref_term(&v("y"), &subst);
        assert_eq!(x_val, num(5.0));
        assert_eq!(y_val, num(5.0));
    }

    // ── Proposition unification ───────────────────────────────

    #[test]
    fn test_unify_propositions_eq() {
        // (x = 5) with (y = z) → {x → y, z → 5}
        let p1 = Proposition::eq(v("x"), num(5.0));
        let p2 = Proposition::eq(v("y"), v("z"));
        let subst = unify_propositions(&p1, &p2).unwrap();
        assert_eq!(subst.get("x"), Some(&v("y")));
        assert_eq!(subst.get("z"), Some(&num(5.0)));
    }

    #[test]
    fn test_unify_propositions_eq_flipped() {
        // Eq is symmetric: (x = 5) with (z = y) — should match flipped
        let p1 = Proposition::eq(v("x"), num(5.0));
        let p2 = Proposition::eq(v("z"), v("y"));
        let subst = unify_propositions(&p1, &p2);
        assert!(subst.is_some(), "Eq should be symmetric");
    }

    #[test]
    fn test_unify_propositions_ge() {
        let p1 = Proposition::ge(v("x"), num(0.0));
        let p2 = Proposition::ge(v("y"), num(0.0));
        let subst = unify_propositions(&p1, &p2).unwrap();
        assert_eq!(subst.get("x"), Some(&v("y")));
    }

    #[test]
    fn test_unify_propositions_ge_fail() {
        // x ≥ 0 does not unify with x < 0
        let p1 = Proposition::ge(v("x"), num(0.0));
        let p2 = Proposition::lt(v("x"), num(0.0));
        assert!(unify_propositions(&p1, &p2).is_none());
    }

    #[test]
    fn test_unify_propositions_predicate() {
        let p1 = Proposition::predicate("Real", vec![v("x")]);
        let p2 = Proposition::predicate("Real", vec![v("y")]);
        let subst = unify_propositions(&p1, &p2).unwrap();
        assert_eq!(subst.get("x"), Some(&v("y")));
    }

    #[test]
    fn test_unify_propositions_predicate_wrong_name() {
        let p1 = Proposition::predicate("Real", vec![v("x")]);
        let p2 = Proposition::predicate("Complex", vec![v("x")]);
        assert!(unify_propositions(&p1, &p2).is_none());
    }

    #[test]
    fn test_unify_propositions_forall_alpha() {
        // ∀x, x ≥ 0 should unify (alpha-equivalent) with ∀y, y ≥ 0
        let p1 = forall_v("x", Proposition::ge(v("x"), num(0.0)));
        let p2 = forall_v("y", Proposition::ge(v("y"), num(0.0)));
        let subst = unify_propositions(&p1, &p2);
        assert!(subst.is_some(), "Alpha-equivalent ForAll should unify");
    }

    #[test]
    fn test_unify_propositions_forall_alpha_binds() {
        // ∀x, x ≥ x should unify with ∀y, y ≥ y
        let p1 = forall_v("x", Proposition::ge(v("x"), v("x")));
        let p2 = forall_v("y", Proposition::ge(v("y"), v("y")));
        assert!(unify_propositions(&p1, &p2).is_some());
    }

    #[test]
    fn test_unify_propositions_not() {
        // ¬(x = 0) with ¬(y = 0) → {x → y}
        let p1 = Proposition::not(Proposition::eq(v("x"), num(0.0)));
        let p2 = Proposition::not(Proposition::eq(v("y"), num(0.0)));
        let subst = unify_propositions(&p1, &p2).unwrap();
        assert_eq!(subst.get("x"), Some(&v("y")));
    }

    #[test]
    fn test_unify_propositions_and() {
        // (x = 1 ∧ y = 2) with (a = 1 ∧ b = 2) → {x → a, y → b}
        let p1 = Proposition::and(
            Proposition::eq(v("x"), num(1.0)),
            Proposition::eq(v("y"), num(2.0)),
        );
        let p2 = Proposition::and(
            Proposition::eq(v("a"), num(1.0)),
            Proposition::eq(v("b"), num(2.0)),
        );
        let subst = unify_propositions(&p1, &p2).unwrap();
        assert_eq!(subst.get("x"), Some(&v("a")));
        assert_eq!(subst.get("y"), Some(&v("b")));
    }

    #[test]
    fn test_unify_propositions_implies() {
        // (x ≥ 0 → sqrt(x²) = x) with (y ≥ 0 → sqrt(y²) = y)
        let p1 = Proposition::implies(
            Proposition::ge(v("x"), num(0.0)),
            Proposition::eq(
                Sqrt(Box::new(Pow(Box::new(v("x")), Box::new(num(2.0))))),
                v("x"),
            ),
        );
        let p2 = Proposition::implies(
            Proposition::ge(v("y"), num(0.0)),
            Proposition::eq(
                Sqrt(Box::new(Pow(Box::new(v("y")), Box::new(num(2.0))))),
                v("y"),
            ),
        );
        let subst = unify_propositions(&p1, &p2).unwrap();
        assert_eq!(subst.get("x"), Some(&v("y")));
    }

    // ── Apply theorem tests ───────────────────────────────────

    #[test]
    fn test_apply_theorem_sqrt_symbolic() {
        reset_fresh_counter();
        let theorems = initial_theorems();
        let sqrt_theorem = theorems
            .iter()
            .find(|t| t.name == "sqrt_square_nonnegative")
            .unwrap();

        // Goal: sqrt(y²) = y  (to be proven in context where y ≥ 0)
        let goal = Proposition::eq(
            Sqrt(Box::new(Pow(Box::new(v("y")), Box::new(num(2.0))))),
            v("y"),
        );

        let result = apply_theorem(sqrt_theorem, &goal).unwrap();

        // Substitution: x → y
        assert_eq!(result.theorem_name, "sqrt_square_nonnegative");

        // Check that dereferencing x gives y
        let x_val = deref_term(&v("x"), &result.substitution);
        assert_eq!(x_val, v("y"));

        // Subgoal: y ≥ 0
        assert_eq!(result.subgoals.len(), 1);
        assert_eq!(result.subgoals[0], Proposition::ge(v("y"), num(0.0)));
    }

    #[test]
    fn test_apply_theorem_add_zero_to_numeric() {
        reset_fresh_counter();
        let theorems = initial_theorems();
        let add_zero = theorems.iter().find(|t| t.name == "add_zero").unwrap();

        // Goal: (z + 0) = z
        let goal = Proposition::eq(v("z") + num(0.0), v("z"));
        let result = apply_theorem(add_zero, &goal).unwrap();

        assert_eq!(result.theorem_name, "add_zero");
        assert!(result.subgoals.is_empty()); // no premises
    }

    #[test]
    fn test_apply_theorem_no_match() {
        let theorems = initial_theorems();
        let sqrt_theorem = theorems
            .iter()
            .find(|t| t.name == "sqrt_square_nonnegative")
            .unwrap();

        // Goal: x + 5 = 10 — should NOT match sqrt theorem
        let goal = Proposition::eq(v("x") + num(5.0), num(10.0));
        assert!(apply_theorem(sqrt_theorem, &goal).is_none());
    }

    #[test]
    fn test_apply_theorem_eq_symmetric() {
        reset_fresh_counter();
        let theorems = initial_theorems();
        let sym = theorems.iter().find(|t| t.name == "eq_symmetric").unwrap();

        // Goal: b = a (where we know a = b)
        let goal = Proposition::eq(v("b"), v("a"));
        let result = apply_theorem(sym, &goal).unwrap();

        // Subgoals: a = b
        assert_eq!(result.subgoals.len(), 1);
        assert_prop_eq(
            &result.subgoals[0],
            &Proposition::eq(v("a"), v("b")),
            "eq_symmetric subgoal",
        );
    }

    #[test]
    fn test_apply_theorem_add_comm() {
        reset_fresh_counter();
        let theorems = initial_theorems();
        let add_comm = theorems.iter().find(|t| t.name == "add_comm").unwrap();

        // Goal: y + x = x + y
        let goal = Proposition::eq(v("y") + v("x"), v("x") + v("y"));
        let result = apply_theorem(add_comm, &goal).unwrap();
        assert!(result.subgoals.is_empty());
    }

    #[test]
    fn test_apply_theorem_mul_comm_numeric() {
        reset_fresh_counter();
        let theorems = initial_theorems();
        let mul_comm = theorems.iter().find(|t| t.name == "mul_comm").unwrap();

        // Goal: 3 * 7 = 7 * 3
        let goal = Proposition::eq(num(3.0) * num(7.0), num(7.0) * num(3.0));
        let result = apply_theorem(mul_comm, &goal).unwrap();
        assert!(result.subgoals.is_empty());
    }

    #[test]
    fn test_apply_theorem_distribute() {
        reset_fresh_counter();
        let theorems = initial_theorems();
        let dist = theorems.iter().find(|t| t.name == "distribute").unwrap();

        // Goal: p*(q + r) = p*q + p*r
        let goal = Proposition::eq(
            v("p") * (v("q") + v("r")),
            (v("p") * v("q")) + (v("p") * v("r")),
        );
        let result = apply_theorem(dist, &goal).unwrap();
        assert!(result.subgoals.is_empty());
    }

    // ── LocalContext tests ────────────────────────────────────

    #[test]
    fn test_local_context_add_and_find() {
        let mut ctx = LocalContext::new();
        let id = ctx.add_hypothesis(Proposition::ge(v("y"), num(0.0)));
        assert_eq!(id, HypothesisId(1));

        let found = ctx.find_exact(&Proposition::ge(v("y"), num(0.0)));
        assert_eq!(found, Some(HypothesisId(1)));

        // Should NOT find a different proposition
        let not_found = ctx.find_exact(&Proposition::ge(v("z"), num(0.0)));
        assert_eq!(not_found, None);
    }

    #[test]
    fn test_local_context_find_unifying() {
        let mut ctx = LocalContext::new();
        ctx.add_hypothesis(Proposition::ge(v("y"), num(0.0)));

        // Find unifying: z ≥ 0 should unify with y ≥ 0
        let found = ctx.find_unifying(&Proposition::ge(v("z"), num(0.0)));
        assert!(found.is_some());
        let (_, subst) = found.unwrap();
        // Either y → z or z → y — check via dereference
        let y_val = deref_term(&v("y"), &subst);
        let z_val = deref_term(&v("z"), &subst);
        assert_eq!(y_val, z_val, "y and z should be unified");
    }

    #[test]
    fn test_local_context_empty() {
        let ctx = LocalContext::new();
        assert!(ctx.find_exact(&Proposition::ge(v("x"), num(0.0))).is_none());
        assert!(ctx
            .find_unifying(&Proposition::ge(v("x"), num(0.0)))
            .is_none());
    }

    #[test]
    fn test_substitution_uses_variable_identity_not_display_name() {
        let left_x = Variable::new(VarId(10_001), VariableKind::Rigid, "x");
        let right_x = Variable::new(VarId(10_002), VariableKind::Rigid, "x");
        let mut subst = TheoremInstantiation::new();
        subst.insert(left_x.clone(), num(7.0));

        assert_eq!(substitute_term(&Var(left_x), &subst), num(7.0));
        assert_eq!(substitute_term(&Var(right_x.clone()), &subst), Var(right_x));
    }

    #[test]
    fn test_alpha_equivalence_ignores_binder_identity() {
        let x = Variable::new(VarId(10_011), VariableKind::Rigid, "x");
        let y = Variable::new(VarId(10_012), VariableKind::Rigid, "y");
        let lhs = Proposition::forall(&x, Proposition::eq(Var(x.clone()), num(1.0)));
        let rhs = Proposition::forall(&y, Proposition::eq(Var(y.clone()), num(1.0)));
        assert!(unify_propositions(&lhs, &rhs).is_some());
    }
}
