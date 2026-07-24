// ─── Tactics Layer ──────────────────────────────────────────────────
//
// A recursive tactic engine that automatically proves goals by applying
// tactics: assumption, intro, apply, exact.
//
// ## Architecture
//
// ```
// Goal (proposition + context)
//     │
//     ▼
// prove() — tries tactics in order, recurses on subgoals
//     │
//     ▼
// Proof (checked by kernel)
// ```
//
// ## Tactic order (tried in sequence)
//
// 1. `assumption` — goal matches a hypothesis in context
// 2. `intro` — goal is P → Q (move P to context, prove Q)
// 3. `apply` — goal unifies with a theorem conclusion (generate subgoals from premises)
// 4. `exact` — goal is a trivial truth (Refl, constant evaluation)
//
// ## Depth limit
//
// A max_depth parameter prevents infinite recursion. Each tactic call
// that produces subgoals consumes one depth level.

use crate::kernel::{check_constant_evaluation, Certificate, Proof, ProofChecker};
use crate::proposition::*;
use std::fmt;

// ═══════════════════════════════════════════════════════════════════
// GOAL
// ═══════════════════════════════════════════════════════════════════

/// A proof goal: prove `proposition` in the given `context`.
#[derive(Clone, Debug, PartialEq)]
pub struct Goal {
    pub context: LocalContext,
    pub proposition: Proposition,
}

impl Goal {
    pub fn new(context: LocalContext, proposition: Proposition) -> Self {
        Goal {
            context,
            proposition,
        }
    }
}

impl fmt::Display for Goal {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "⊢ {}  [{:?}]", self.proposition, self.context)
    }
}

// ═══════════════════════════════════════════════════════════════════
// TACTIC ERROR
// ═══════════════════════════════════════════════════════════════════

/// Errors produced by the tactic engine.
#[derive(Clone, Debug, PartialEq)]
pub enum TacticError {
    /// No tactic could make progress on the goal.
    NoTacticApplies(Goal),
    /// Max recursion depth reached.
    MaxDepthReached(usize),
    /// Theorem application failed (name not found, unification failed).
    ApplyFailed(String),
    /// The generated proof was rejected by the kernel.
    KernelRejected(String),
}

impl fmt::Display for TacticError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TacticError::NoTacticApplies(goal) => {
                write!(f, "no tactic applies to {}", goal.proposition)
            }
            TacticError::MaxDepthReached(d) => {
                write!(f, "max recursion depth {} reached", d)
            }
            TacticError::ApplyFailed(msg) => {
                write!(f, "apply failed: {}", msg)
            }
            TacticError::KernelRejected(msg) => {
                write!(f, "kernel rejected proof: {}", msg)
            }
        }
    }
}

// ═══════════════════════════════════════════════════════════════════
// TACTICS
// ═══════════════════════════════════════════════════════════════════

/// Try to close the goal by finding a matching hypothesis in the context.
///
/// Tries both exact match and unifying match. Returns a `Proof::Hypothesis`
/// if found.
fn try_assumption(goal: &Goal) -> Option<Proof> {
    // Exact match first
    if let Some(id) = goal.context.find_exact(&goal.proposition) {
        return Some(Proof::Hypothesis(id));
    }
    None
}

/// Try to prove the goal by introducing a binder.
///
/// If the goal is `P → Q`:
///   - Creates a new hypothesis `P` in the context
///   - Sets the new goal to `Q`
///   - Returns `Proof::Intro { Assumption { P }, body: ? }`
///
/// If the goal is `∀x, P(x)`:
///   - Checks `x` is not free in context hypotheses
///   - Sets the new goal to `P(x)`
///   - Returns `Proof::Intro { ForAll { x }, body: ? }`
fn try_intro(goal: &Goal) -> Option<(Binder, Goal)> {
    match &goal.proposition {
        Proposition::Implies(premise, conclusion) => {
            // Create the binder
            let binder = Binder::Assumption {
                hypothesis_id: HypothesisId(0), // placeholder — kernel assigns its own
                proposition: premise.clone(),
            };

            // Extend context with the premise
            let mut new_context = goal.context.clone();
            new_context.add_hypothesis((**premise).clone());

            let sub_goal = Goal::new(new_context, (**conclusion).clone());
            Some((binder, sub_goal))
        }

        Proposition::ForAll(Binder::ForAll { variable }, body) => {
            // Check variable doesn't appear free in any hypothesis
            for (_, h) in &goal.context.hypotheses {
                if free_vars_proposition(h).contains(variable.display.as_ref()) {
                    return None; // can't intro — variable clashes
                }
            }

            let binder = Binder::ForAll {
                variable: variable.clone(),
            };
            let sub_goal = Goal::new(goal.context.clone(), (**body).clone());
            Some((binder, sub_goal))
        }

        _ => None,
    }
}

/// Try to apply a theorem whose conclusion unifies with the goal.
///
/// Uses the theorem environment to find candidates by:
/// 1. Head-symbol matching (robust for structural heads like `sqrt`, `+`, `*`)
/// 2. Fallback: try all theorems
///
/// Returns the theorem info, substitution, and subgoals (instantiated premises).
fn try_apply(
    goal: &Goal,
    theorems: &TheoremEnvironment,
) -> Vec<(TheoremSchema, Substitution, Vec<Proposition>)> {
    let goal_head = head_from_goal(&goal.proposition);
    let mut candidates: Vec<&TheoremSchema> = Vec::new();

    // Step 1: Try head-symbol matching
    if !goal_head.is_empty() {
        let head_matches = theorems.find_by_head(&goal_head);
        candidates.extend(head_matches);
    }

    // Step 2: Fallback — try all remaining theorems
    for t in theorems.all() {
        if !candidates.iter().any(|c| c.id == t.id) {
            candidates.push(t);
        }
    }

    // Score and sort candidates (best first — lowest score = best)
    let mut scored: Vec<(i32, &TheoremSchema)> = candidates
        .into_iter()
        .map(|t| {
            let score = score_candidate(t, &goal_head, &goal.proposition);
            (score, t)
        })
        .collect();
    scored.sort_by_key(|(score, _)| *score);

    // Try each candidate in order, return all successful applications
    let mut results = Vec::new();
    for (_score, theorem) in scored {
        if let Some(result) = apply_theorem(theorem, &goal.proposition) {
            results.push((theorem.clone(), result.substitution, result.subgoals));
        }
    }

    results
}

/// Score a theorem candidate for ranking in `try_apply`.
/// Lower score = better (tried first).
fn score_candidate(theorem: &TheoremSchema, goal_head: &str, _goal: &Proposition) -> i32 {
    let mut score: i32 = 0;

    // Primary: prefer fewer premises
    score += (theorem.premises.len() as i32) * 100;

    // Prefer theorems whose conclusion head matches the goal head exactly
    let conc_heads = conclusion_heads(&theorem.conclusion);
    if conc_heads.iter().any(|h| h == goal_head) {
        score -= 50; // strong bonus for head match
    }
    if conc_heads.iter().any(|h| h == "=") && goal_head != "=" && !goal_head.is_empty() {
        score += 10; // slight penalty for equality theorems on non-equality goals
    }

    // Prefer curated axioms over logical primitives for non-structural goals
    match theorem.trust {
        TheoremTrust::LogicalPrimitive => score += 5,
        TheoremTrust::CuratedAxiom => score += 0,
    }

    // Bonus for theorems whose name suggests relevance to the goal
    if theorem.name.contains(goal_head) {
        score -= 10;
    }

    score
}

/// Try exact/trivial proofs for the goal.
///
/// Handles:
/// - `t = t` via reflexivity
/// - Ground arithmetic truths via constant evaluation certificate
fn try_exact(goal: &Goal) -> Option<Proof> {
    let p = &goal.proposition;

    // Reflexivity: t = t
    if let Proposition::Eq(a, b) = p {
        if a == b {
            return Some(Proof::Refl(a.clone()));
        }
    }

    // Constant evaluation: closed arithmetic
    if check_constant_evaluation(p) {
        return Some(Proof::Certificate {
            proposition: p.clone(),
            certificate: Certificate::ConstantEvaluation,
        });
    }

    None
}

// ═══════════════════════════════════════════════════════════════════
// HEAD SYMBOL FROM GOAL
// ═══════════════════════════════════════════════════════════════════

/// Extract a search key from a goal proposition for theorem indexing.
///
/// For `Eq(lhs, rhs)`, returns the head symbol of the LHS.
/// For `Ge/Le/Gt/Lt`, returns `"≥"`, `"≤"`, etc.
/// For `Not(p)`, returns `"¬"`.
/// For `Predicate { symbol, .. }`, returns the symbol.
fn head_from_goal(p: &Proposition) -> String {
    match p {
        Proposition::Eq(lhs, _) => head_symbol(lhs),
        Proposition::Ge(_, _) => "≥".to_string(),
        Proposition::Le(_, _) => "≤".to_string(),
        Proposition::Gt(_, _) => ">".to_string(),
        Proposition::Lt(_, _) => "<".to_string(),
        Proposition::NotEq(_, _) => "≠".to_string(),
        Proposition::Not(_) => "¬".to_string(),
        Proposition::And(_, _) => "∧".to_string(),
        Proposition::Or(_, _) => "∨".to_string(),
        Proposition::Implies(_, _) => "→".to_string(),
        Proposition::Iff(_, _) => "↔".to_string(),
        Proposition::ForAll(_, _) => "∀".to_string(),
        Proposition::Exists(_, _) => "∃".to_string(),
        Proposition::Predicate { symbol, .. } => symbol.clone(),
        Proposition::True => "⊤".to_string(),
        Proposition::False => "⊥".to_string(),
    }
}

// ═══════════════════════════════════════════════════════════════════
// PROVE — Main recursive engine
// ═══════════════════════════════════════════════════════════════════

/// Prove a goal using the available theorems, with a depth limit.
///
/// Tries tactics in order: assumption → intro → apply → exact.
/// When a tactic creates subgoals, `prove` recurses on each.
///
/// # Arguments
///
/// * `goal` — What to prove
/// * `theorems` — Trusted theorem environment
/// * `max_depth` — Maximum recursion depth (prevents infinite loops)
///
/// # Returns
///
/// * `Ok(Proof)` — A proof object that the kernel can verify
/// * `Err(TacticError)` — What went wrong
pub fn prove(
    goal: &Goal,
    theorems: &TheoremEnvironment,
    max_depth: usize,
) -> Result<Proof, TacticError> {
    prove_inner(goal, theorems, max_depth, 0)
}

/// Internal recursive prover with current depth tracking.
fn prove_inner(
    goal: &Goal,
    theorems: &TheoremEnvironment,
    max_depth: usize,
    depth: usize,
) -> Result<Proof, TacticError> {
    if depth > max_depth {
        return Err(TacticError::MaxDepthReached(depth));
    }

    // ── Tactic 1: assumption ─────────────────────────────────
    if let Some(proof) = try_assumption(goal) {
        return Ok(proof);
    }

    // ── Tactic 2: intro ──────────────────────────────────────
    if let Some((binder, sub_goal)) = try_intro(goal) {
        let body_proof = prove_inner(&sub_goal, theorems, max_depth, depth + 1)?;
        return Ok(Proof::Intro {
            binder,
            body: Box::new(body_proof),
        });
    }

    // ── Tactic 3: exact ───────────────────────────────────────
    if let Some(proof) = try_exact(goal) {
        return Ok(proof);
    }

    // ── Tactic 4: apply ──────────────────────────────────────
    let apply_results = try_apply(goal, theorems);
    for (theorem, subst, subgoals) in apply_results {
        let mut premise_proofs = Vec::new();
        let mut success = true;

        for subgoal_prop in subgoals {
            let sub_goal = Goal::new(goal.context.clone(), subgoal_prop);
            match prove_inner(&sub_goal, theorems, max_depth, depth + 1) {
                Ok(p) => premise_proofs.push(p),
                Err(_) => {
                    success = false;
                    break;
                }
            }
        }

        if success {
            return Ok(Proof::Theorem {
                id: theorem.id,
                subst,
                premise_proofs,
            });
        }
    }

    // ── Tactic 4: exact ──────────────────────────────────────
    if let Some(proof) = try_exact(goal) {
        return Ok(proof);
    }

    Err(TacticError::NoTacticApplies(goal.clone()))
}

/// Convenience: prove a goal and verify the proof with the kernel.
///
/// Combines `prove()` + `ProofChecker::check()` into a single call.
///
/// Returns the proof on success, or the first error encountered.
pub fn prove_and_verify(
    goal: &Goal,
    theorems: &TheoremEnvironment,
    checker: &ProofChecker,
    max_depth: usize,
) -> Result<Proof, String> {
    let proof = prove(goal, theorems, max_depth).map_err(|e| format!("{}", e))?;

    checker
        .check(&goal.context, &proof, &goal.proposition)
        .map_err(|e| format!("kernel rejected proof: {}", e))?;

    Ok(proof)
}

// ═══════════════════════════════════════════════════════════════════
// TESTS
// ═══════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;
    use crate::kernel::ProofChecker;
    use SymExpr::*;

    fn v(name: &str) -> SymExpr {
        Var(crate::proposition::shared_var(name))
    }

    fn num(v: f64) -> SymExpr {
        Num(v)
    }

    fn make_theorems() -> TheoremEnvironment {
        TheoremEnvironment::with_initial_theorems()
    }

    // ── Assumption tactic tests ──────────────────────────────

    #[test]
    fn test_tactic_assumption_exact_match() {
        let mut ctx = LocalContext::new();
        ctx.add_hypothesis(Proposition::ge(v("y"), num(0.0)));

        let goal = Goal::new(ctx, Proposition::ge(v("y"), num(0.0)));
        let proof = try_assumption(&goal);
        assert!(proof.is_some());
    }

    #[test]
    fn test_tactic_assumption_no_match() {
        let mut ctx = LocalContext::new();
        ctx.add_hypothesis(Proposition::ge(v("y"), num(0.0)));

        let goal = Goal::new(ctx, Proposition::ge(v("z"), num(0.0)));
        let proof = try_assumption(&goal);
        assert!(proof.is_none(), "should not match: different variable name");
    }

    // ── Intro tactic tests ───────────────────────────────────

    #[test]
    fn test_tactic_intro_implies() {
        let ctx = LocalContext::new();
        let goal = Goal::new(
            ctx,
            Proposition::implies(
                Proposition::ge(v("x"), num(0.0)),
                Proposition::eq(
                    Sqrt(Box::new(Pow(Box::new(v("x")), Box::new(num(2.0))))),
                    v("x"),
                ),
            ),
        );

        let result = try_intro(&goal);
        assert!(result.is_some());
        let (binder, sub_goal) = result.unwrap();

        // Binder should be an assumption
        match binder {
            Binder::Assumption { proposition, .. } => {
                assert_eq!((*proposition), Proposition::ge(v("x"), num(0.0)));
            }
            _ => panic!("expected Assumption binder"),
        }

        // Subgoal should be sqrt(x²) = x
        assert_eq!(
            sub_goal.proposition,
            Proposition::eq(
                Sqrt(Box::new(Pow(Box::new(v("x")), Box::new(num(2.0))))),
                v("x"),
            )
        );

        // Context should have the hypothesis
        assert_eq!(sub_goal.context.len(), 1);
    }

    #[test]
    fn test_tactic_intro_no_implies() {
        let ctx = LocalContext::new();
        let goal = Goal::new(ctx, Proposition::eq(v("x"), v("y")));
        assert!(try_intro(&goal).is_none());
    }

    // ── Exact tactic tests ───────────────────────────────────

    #[test]
    fn test_tactic_exact_refl() {
        let ctx = LocalContext::new();
        let goal = Goal::new(ctx, Proposition::eq(v("x"), v("x")));
        let proof = try_exact(&goal);
        assert!(proof.is_some());
        assert!(matches!(proof.unwrap(), Proof::Refl(_)));
    }

    #[test]
    fn test_tactic_exact_constant_eval() {
        let ctx = LocalContext::new();
        let goal = Goal::new(ctx, Proposition::ge(num(4.0), num(0.0)));
        let proof = try_exact(&goal);
        assert!(proof.is_some());
    }

    #[test]
    fn test_tactic_exact_no_match() {
        let ctx = LocalContext::new();
        let goal = Goal::new(ctx, Proposition::eq(v("x"), v("y")));
        assert!(try_exact(&goal).is_none());
    }

    // ── Apply tactic tests ───────────────────────────────────

    #[test]
    fn test_tactic_apply_sqrt() {
        let theorems = make_theorems();
        let ctx = LocalContext::new();

        let goal = Goal::new(
            ctx,
            Proposition::eq(
                Sqrt(Box::new(Pow(Box::new(v("y")), Box::new(num(2.0))))),
                v("y"),
            ),
        );

        let results = try_apply(&goal, &theorems);
        assert!(!results.is_empty(), "should find sqrt_square_nonnegative");

        let (theorem, subst, subgoals) = &results[0];
        assert_eq!(theorem.name, "sqrt_square_nonnegative");
        assert_eq!(subgoals.len(), 1);
        assert_eq!(subgoals[0], Proposition::ge(v("y"), num(0.0)));
    }

    #[test]
    fn test_tactic_apply_add_zero() {
        let theorems = make_theorems();
        let ctx = LocalContext::new();

        let goal = Goal::new(ctx, Proposition::eq(v("z") + num(0.0), v("z")));
        let results = try_apply(&goal, &theorems);
        assert!(!results.is_empty(), "should find add_zero");
        assert_eq!(results[0].0.name, "add_zero");
        assert!(results[0].2.is_empty());
    }

    // ── Full prove tests ─────────────────────────────────────

    /// THE KEY TEST: prove sqrt(y²) = y from hypothesis y ≥ 0
    #[test]
    fn test_prove_sqrt_symbolic() {
        let theorems = make_theorems();
        let checker = ProofChecker::new(theorems.clone());

        // Context: y ≥ 0
        let mut ctx = LocalContext::new();
        ctx.add_hypothesis(Proposition::ge(v("y"), num(0.0)));

        // Goal: sqrt(y²) = y
        let goal = Goal::new(
            ctx,
            Proposition::eq(
                Sqrt(Box::new(Pow(Box::new(v("y")), Box::new(num(2.0))))),
                v("y"),
            ),
        );

        let proof = prove(&goal, &theorems, 5).expect("should prove sqrt goal");

        // Verify with kernel
        let check_result = checker.check(&goal.context, &proof, &goal.proposition);
        assert!(
            check_result.is_ok(),
            "kernel rejected valid proof: {:?}",
            check_result
        );
    }

    /// Test: prove and verify in one call
    #[test]
    fn test_prove_and_verify_sqrt_symbolic() {
        let theorems = make_theorems();
        let checker = ProofChecker::new(theorems.clone());

        let mut ctx = LocalContext::new();
        ctx.add_hypothesis(Proposition::ge(v("y"), num(0.0)));

        let goal = Goal::new(
            ctx,
            Proposition::eq(
                Sqrt(Box::new(Pow(Box::new(v("y")), Box::new(num(2.0))))),
                v("y"),
            ),
        );

        let result = prove_and_verify(&goal, &theorems, &checker, 5);
        assert!(result.is_ok(), "prove_and_verify failed: {:?}", result);
    }

    /// Test: prove sqrt(4²) = 4 (numeric, no hypotheses)
    #[test]
    fn test_prove_sqrt_numeric() {
        let theorems = make_theorems();
        let checker = ProofChecker::new(theorems.clone());

        let ctx = LocalContext::new();

        let goal = Goal::new(
            ctx,
            Proposition::eq(
                Sqrt(Box::new(Pow(Box::new(num(4.0)), Box::new(num(2.0))))),
                num(4.0),
            ),
        );

        let result = prove_and_verify(&goal, &theorems, &checker, 5);
        assert!(result.is_ok(), "should prove sqrt(4²) = 4: {:?}", result);
    }

    /// Test: prove add_zero: y + 0 = y
    #[test]
    fn test_prove_add_zero() {
        let theorems = make_theorems();
        let checker = ProofChecker::new(theorems.clone());

        let ctx = LocalContext::new();
        let goal = Goal::new(ctx, Proposition::eq(v("y") + num(0.0), v("y")));

        let result = prove_and_verify(&goal, &theorems, &checker, 5);
        assert!(result.is_ok(), "should prove add_zero: {:?}", result);
    }

    /// Test: prove mul_one: y * 1 = y
    #[test]
    fn test_prove_mul_one() {
        let theorems = make_theorems();
        let checker = ProofChecker::new(theorems.clone());

        let ctx = LocalContext::new();
        let goal = Goal::new(ctx, Proposition::eq(v("y") * num(1.0), v("y")));

        let result = prove_and_verify(&goal, &theorems, &checker, 5);
        assert!(result.is_ok(), "should prove mul_one: {:?}", result);
    }

    /// Test: prove add_comm: a + b = b + a
    #[test]
    fn test_prove_add_comm() {
        let theorems = make_theorems();
        let checker = ProofChecker::new(theorems.clone());

        let ctx = LocalContext::new();
        let goal = Goal::new(ctx, Proposition::eq(v("a") + v("b"), v("b") + v("a")));

        let result = prove_and_verify(&goal, &theorems, &checker, 5);
        assert!(result.is_ok(), "should prove add_comm: {:?}", result);
    }

    /// Test: prove mul_comm: a * b = b * a
    #[test]
    fn test_prove_mul_comm() {
        let theorems = make_theorems();
        let checker = ProofChecker::new(theorems.clone());

        let ctx = LocalContext::new();
        let goal = Goal::new(ctx, Proposition::eq(v("a") * v("b"), v("b") * v("a")));

        let result = prove_and_verify(&goal, &theorems, &checker, 5);
        assert!(result.is_ok(), "should prove mul_comm: {:?}", result);
    }

    /// Test: prove distributive: a(b + c) = ab + ac
    #[test]
    fn test_prove_distribute() {
        let theorems = make_theorems();
        let checker = ProofChecker::new(theorems.clone());

        let ctx = LocalContext::new();
        let goal = Goal::new(
            ctx,
            Proposition::eq(
                v("a") * (v("b") + v("c")),
                (v("a") * v("b")) + (v("a") * v("c")),
            ),
        );

        let result = prove_and_verify(&goal, &theorems, &checker, 5);
        assert!(result.is_ok(), "should prove distribute: {:?}", result);
    }

    /// Test: prove implication via intro
    ///
    /// Prove: (y ≥ 0) → sqrt(y²) = y
    /// Using: intro (moves y ≥ 0 to context), then apply sqrt theorem
    #[test]
    fn test_prove_implication() {
        let theorems = make_theorems();
        let checker = ProofChecker::new(theorems.clone());

        let ctx = LocalContext::new();
        let goal = Goal::new(
            ctx,
            Proposition::implies(
                Proposition::ge(v("y"), num(0.0)),
                Proposition::eq(
                    Sqrt(Box::new(Pow(Box::new(v("y")), Box::new(num(2.0))))),
                    v("y"),
                ),
            ),
        );

        let result = prove_and_verify(&goal, &theorems, &checker, 10);
        assert!(
            result.is_ok(),
            "should prove implication via intro+apply: {:?}",
            result
        );
    }

    /// Test: impossible goal (no theorems match, no hypotheses)
    #[test]
    fn test_prove_impossible_goal() {
        let theorems = make_theorems();

        // A goal that can't be matched by any theorem or tactic
        // Use a Predicate that no theorem covers
        let ctx = LocalContext::new();
        let goal = Goal::new(ctx, Proposition::predicate("Magic", vec![v("x")]));

        let result = prove(&goal, &theorems, 3);
        assert!(result.is_err());
        match result.unwrap_err() {
            TacticError::NoTacticApplies(_) => {} // expected
            other => panic!("expected NoTacticApplies, got: {:?}", other),
        }
    }

    /// Test: intro + apply chain — implication then theorem use
    ///
    /// This tests the full pipeline:
    ///   Goal: y ≥ 0 → sqrt(y²) = y
    ///   → intro: move y ≥ 0 to context
    ///   → apply sqrt_square_nonnegative with {x → y}
    ///   → assumption: y ≥ 0 matches hypothesis
    #[test]
    fn test_prove_intro_apply_assumption_chain() {
        let theorems = make_theorems();
        let checker = ProofChecker::new(theorems.clone());

        let ctx = LocalContext::new();
        let goal = Goal::new(
            ctx,
            Proposition::implies(
                Proposition::ge(v("y"), num(0.0)),
                Proposition::eq(
                    Sqrt(Box::new(Pow(Box::new(v("y")), Box::new(num(2.0))))),
                    v("y"),
                ),
            ),
        );

        let proof = prove(&goal, &theorems, 10).expect("full chain should work");
        let check_result = checker.check(&goal.context, &proof, &goal.proposition);
        assert!(check_result.is_ok(), "kernel rejected: {:?}", check_result);
    }

    /// Test: Refl (reflexivity) via exact
    #[test]
    fn test_prove_refl() {
        let theorems = make_theorems();
        let checker = ProofChecker::new(theorems.clone());

        let ctx = LocalContext::new();
        let goal = Goal::new(ctx, Proposition::eq(v("x"), v("x")));

        let result = prove_and_verify(&goal, &theorems, &checker, 5);
        assert!(result.is_ok(), "Refl proof failed: {:?}", result);
    }

    /// Test: constant evaluation via exact
    #[test]
    fn test_prove_constant_eval() {
        let theorems = make_theorems();
        let checker = ProofChecker::new(theorems.clone());

        let ctx = LocalContext::new();
        let goal = Goal::new(ctx, Proposition::ge(num(100.0), num(0.0)));

        let result = prove_and_verify(&goal, &theorems, &checker, 5);
        assert!(result.is_ok(), "constant eval proof failed: {:?}", result);
    }

    /// Test: Deep recursion with multiple theorem applications
    ///
    /// While our current theorem set doesn't chain theorems together
    /// (each theorem is an axiom with no premises that need theorem-based proofs),
    /// this tests that the recursion works correctly for the existing case.
    #[test]
    fn test_prove_deep_depth_ok() {
        let theorems = make_theorems();
        let checker = ProofChecker::new(theorems.clone());

        let ctx = LocalContext::new();
        let goal = Goal::new(ctx, Proposition::eq(v("z") * num(1.0), v("z")));

        let result = prove_and_verify(&goal, &theorems, &checker, 20);
        assert!(result.is_ok(), "deep proof failed: {:?}", result);
    }

    #[test]
    fn test_prove_abs_of_nonnegative() {
        let theorems = make_theorems();
        let checker = ProofChecker::new(theorems.clone());

        let mut ctx = LocalContext::new();
        ctx.add_hypothesis(Proposition::ge(v("x"), num(0.0)));

        let goal = Goal::new(ctx, Proposition::eq(Abs(Box::new(v("x"))), v("x")));

        let result = prove_and_verify(&goal, &theorems, &checker, 5);
        assert!(
            result.is_ok(),
            "abs_of_nonnegative proof failed: {:?}",
            result
        );
    }

    #[test]
    fn test_prove_abs_of_negative() {
        let theorems = make_theorems();
        let checker = ProofChecker::new(theorems.clone());

        let mut ctx = LocalContext::new();
        ctx.add_hypothesis(Proposition::lt(v("x"), num(0.0)));

        let goal = Goal::new(
            ctx,
            Proposition::eq(Abs(Box::new(v("x"))), Neg(Box::new(v("x")))),
        );

        let result = prove_and_verify(&goal, &theorems, &checker, 5);
        assert!(result.is_ok(), "abs_of_negative proof failed: {:?}", result);
    }

    #[test]
    fn test_prove_mul_zero() {
        let theorems = make_theorems();
        let checker = ProofChecker::new(theorems.clone());

        let ctx = LocalContext::new();
        let goal = Goal::new(ctx, Proposition::eq(v("x") * num(0.0), num(0.0)));

        let result = prove_and_verify(&goal, &theorems, &checker, 5);
        assert!(result.is_ok(), "mul_zero proof failed: {:?}", result);
    }

    /// Test: prove mul_comm with different variable names
    #[test]
    fn test_prove_mul_comm_different_names() {
        let theorems = make_theorems();
        let checker = ProofChecker::new(theorems.clone());

        let ctx = LocalContext::new();
        // different variable names from the theorem's "a" and "b"
        let goal = Goal::new(ctx, Proposition::eq(v("x") * v("y"), v("y") * v("x")));

        let result = prove_and_verify(&goal, &theorems, &checker, 5);
        assert!(
            result.is_ok(),
            "mul_comm with diff names failed: {:?}",
            result
        );
    }

    #[test]
    fn test_prove_eq_symmetric_with_context() {
        let theorems = make_theorems();
        let checker = ProofChecker::new(theorems.clone());

        let mut ctx = LocalContext::new();
        ctx.add_hypothesis(Proposition::eq(v("a"), v("b")));

        // Goal: b = a (symmetric of hypothesis)
        let goal = Goal::new(ctx, Proposition::eq(v("b"), v("a")));

        let result = prove_and_verify(&goal, &theorems, &checker, 10);
        assert!(result.is_ok(), "eq_symmetric proof failed: {:?}", result);
    }
}
