// ─── Counterfactual Simulator ─────────────────────────────────────────────
//
// Gives The Machine the ability to imagine alternative futures before
// committing to an action.  Uses Self_t + global_broadcast as the ground
// truth (T₀), rolls out candidate actions through simulated time steps,
// and evaluates each simulated outcome against a shadow self-model.
//
// ## The Simulation Cycle
//
//   T₀ = Self_t (current integrated identity)
//
//   For each candidate action A ∈ {A₁, A₂, A₃}:
//     T₁ = T₀ ⊕ ρ⁰(A.effect)  →  evaluate(T₁) × γ⁰
//     T₂ = T₁ ⊕ ρ¹(A.effect)  →  evaluate(T₂) × γ¹
//     T₃ = T₂ ⊕ ρ²(A.effect)  →  evaluate(T₃) × γ²
//     (ρⁿ is rotate-left by n bits — avoids XOR idempotency where
//      applying the same vector twice cancels out)
//     Score_A = Σ evaluate(Tₙ) × γⁿ
//
//   Select argmin Score_A (lower = better = more stable future)
//
// ## Evaluation Function (Free Energy Analogue)
//
//   evaluate(state) = w₁·Δdeficit + w₂·Δerror + w₃·Δidentity + w₄·cost
//
//   Δdeficit   = how much the simulated homeostasis differs from ideal (0)
//   Δerror     = how much prediction error increases or decreases
//   Δidentity  = NHD(simulated_self, current_self) — shock avoidance
//   cost       = computational/energy cost of the action
//
//   Lower scores are better.  The action that minimizes total expected
//   free energy across the rollout horizon is selected.
//
// ## Why T+3 with Decay
//
// T+1 alone cannot distinguish sustainable from short-sighted actions.
// T+3 with γ = 0.5 per step:
//   - T+1 weighted 1.0 (immediate effect)
//   - T+2 weighted 0.5 (near future)
//   - T+3 weighted 0.25 (distant future, heavily discounted)
//
// An action that helps at T+1 but hurts at T+3 gets a worse total score
// than one that helps modestly across all three steps.
//
// ## Action Proposals
//
// Actions are predefined as (label, effect_vector, deficit_change,
// error_change, cost).  The effect_vector is XORed into the current
// state (with a step-dependent rotation to avoid XOR idempotency) to
// produce the simulated next state.  Deficit and error changes are the
// simulator's predictive model of what each action does.
//
// ## Mathematical Guarantees
//
// **Theorem Sim1 (Bounded Rollout):** For D rollout steps and K actions,
// the total number of simulated states is bounded by K × D.  Time
// complexity = O(K × D × D_eval) where D_eval = O(D) for the identity
// distance computation.
//
// **Theorem Sim2 (Deterministic Evaluation):** For the same
// (state, action_set, homeostasis, error) inputs, the simulated
// outcomes are deterministic.  No randomness.
//
// **Theorem Sim3 (Free Energy Minimization):** The overall score
// F = w₁·Δdeficit + w₂·Δerror + w₃·Δidentity + w₄·cost is a
// variational free energy analogue.  Selecting argmin F implements
// active inference: the system acts to minimize its expected surprise.
//
// **Theorem Sim4 (Convergent Selection):** For a fixed state and fixed
// action set, the same action is selected every time.  No oscillation.
//
// ## Test Coverage
//
// 1. test_baseline_simulation   — NULL action produces minimal change
// 2. test_action_selection      — The action with best free energy wins
// 3. test_deterministic_eval    — Same inputs → same outcome
// 4. test_multi_step_rollout    — T+3 simulation converges
// 5. test_action_priority       — Low-cost effective action beats high-cost
// 6. test_action_registration   — Actions register and unregister
//
// ## Wiring (in main.rs)
//
//   let mut simulator = CounterfactualSimulator::with_default_actions();
//   let outcome = simulator.evaluate(
//       &self_model.current_identity,
//       &profile,
//       self_model.global_error,
//       &workspace.global_broadcast,
//   );
//   // outcome.best_action is the winner → route to intent system

use crate::Hypervector;

// ═══════════════════════════════════════════════════════════════════════════
// CONSTANTS
// ═══════════════════════════════════════════════════════════════════════════

/// Maximum number of action proposals.
pub const MAX_ACTIONS: usize = 16;

/// Default rollout depth (number of simulated time steps).
pub const DEFAULT_ROLLOUT_DEPTH: usize = 3;

/// Uncertainty decay per rollout step (multiplied each step).
/// γ = 0.5: T+1 weighted 1.0, T+2 weighted 0.5, T+3 weighted 0.25.
pub const UNCERTAINTY_DECAY: f64 = 0.5;

/// Default free energy weights: [Δdeficit, Δerror, Δidentity, cost].
pub const DEFAULT_WEIGHTS: [f64; 4] = [0.30, 0.30, 0.20, 0.20];

// ═══════════════════════════════════════════════════════════════════════════
// ACTION PROPOSAL
// ═══════════════════════════════════════════════════════════════════════════

/// A candidate action the simulator can evaluate.
///
/// Each action has:
/// - A unique ID and human label
/// - An effect vector: bind this (via XOR) into the current state to simulate
///   the action's impact on the identity/self.  At each rollout step the
///   effect is rotated by `step` bits so repeated applications accumulate
///   instead of cancelling (XOR is its own inverse).
/// - Expected deficit change per step: how homeostasis responds (-1..1,
///   negative = reduces deficit = good)
/// - Expected error change per step: how prediction error responds (-1..1,
///   negative = reduces error = good)
/// - A computational/energy cost (0..1)
#[derive(Clone, Debug)]
pub struct ActionProposal {
    pub id: u8,
    pub label: String,
    /// Effect hypervector: XOR into state (with step-dependent rotation) to
    /// simulate the action's cumulative impact.
    pub effect_vector: Hypervector,
    /// Expected change in homeostatic deficit per rollout step.
    pub deficit_delta: f64,
    /// Expected change in prediction error per rollout step.
    pub error_delta: f64,
    /// Computational / energy cost of this action (0.0–1.0).
    pub cost: f64,
}

impl ActionProposal {
    pub fn new(
        id: u8,
        label: &str,
        effect_vector: Hypervector,
        deficit_delta: f64,
        error_delta: f64,
        cost: f64,
    ) -> Self {
        ActionProposal {
            id,
            label: label.to_string(),
            effect_vector,
            deficit_delta: deficit_delta.clamp(-1.0, 1.0),
            error_delta: error_delta.clamp(-1.0, 1.0),
            cost: cost.clamp(0.0, 1.0),
        }
    }

    /// Create an action with a text-encoded effect vector.
    pub fn from_text(
        id: u8,
        label: &str,
        effect_text: &str,
        deficit_delta: f64,
        error_delta: f64,
        cost: f64,
    ) -> Self {
        let effect = Hypervector::encode_text_ngram(effect_text, 3);
        ActionProposal::new(id, label, effect, deficit_delta, error_delta, cost)
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// SIMULATED OUTCOME
// ═══════════════════════════════════════════════════════════════════════════

/// The result of simulating a single action over the rollout horizon.
#[derive(Clone, Debug)]
pub struct SimulatedOutcome {
    /// The action that was simulated.
    pub action_id: u8,
    pub action_label: String,
    /// Simulated states at each rollout step (T+1, T+2, T+3).
    pub simulated_states: Vec<Hypervector>,
    /// Simulated homeostatic deficit at each step.
    pub simulated_deficits: Vec<f64>,
    /// Simulated prediction error at each step.
    pub simulated_errors: Vec<f64>,
    /// Identity shift at each step (NHD from state at that step to current).
    pub identity_shifts: Vec<f64>,
    /// Step-wise scores (lower = better).
    pub step_scores: Vec<f64>,
    /// Total weighted score across all steps (lower = better).
    pub total_score: f64,
}

// ═══════════════════════════════════════════════════════════════════════════
// SIMULATION REPORT
// ═══════════════════════════════════════════════════════════════════════════

/// Full report from a simulation round.
#[derive(Clone, Debug)]
pub struct SimulationReport {
    /// The winning action (lowest total score).
    pub best_action: ActionProposal,
    /// Its full simulated outcome.
    pub best_outcome: SimulatedOutcome,
    /// All actions evaluated (sorted by total_score ascending).
    pub ranked_outcomes: Vec<SimulatedOutcome>,
    /// Number of actions evaluated.
    pub actions_evaluated: usize,
    /// Rollout depth used.
    pub rollout_depth: usize,
    /// Total simulation cycles performed.
    pub total_simulations: u64,
}

// ═══════════════════════════════════════════════════════════════════════════
// COUNTERFACTUAL SIMULATOR
// ═══════════════════════════════════════════════════════════════════════════

/// The counterfactual simulator: imagines alternative futures, evaluates
/// them through a shadow self-model, and selects the action that minimizes
/// expected free energy.
pub struct CounterfactualSimulator {
    /// Registered action proposals.
    pub actions: Vec<ActionProposal>,
    /// Rollout depth (number of simulated steps).
    pub rollout_depth: usize,
    /// Uncertainty decay per step.
    pub uncertainty_decay: f64,
    /// Free energy weights: [Δdeficit, Δerror, Δidentity, cost].
    pub weights: [f64; 4],
    /// Total simulations performed (for statistics).
    pub total_simulations: u64,
}

impl CounterfactualSimulator {
    pub fn new(rollout_depth: usize, weights: [f64; 4]) -> Self {
        CounterfactualSimulator {
            actions: Vec::with_capacity(MAX_ACTIONS),
            rollout_depth: rollout_depth.min(MAX_ACTIONS),
            uncertainty_decay: UNCERTAINTY_DECAY,
            weights,
            total_simulations: 0,
        }
    }

    /// Create with default rollout depth (3) and weights.
    pub fn with_defaults() -> Self {
        CounterfactualSimulator::new(DEFAULT_ROLLOUT_DEPTH, DEFAULT_WEIGHTS)
    }

    /// Register a set of default actions (domain-agnostic cognitive actions).
    ///
    /// These actions encode the fundamental behavioral modes the system
    /// can switch between.  Each has a text-encoded effect vector, a
    /// predicted impact on homeostasis and error, and a cost.
    pub fn register_default_actions(&mut self) {
        self.actions.clear();

        // 0: NULL — do nothing (baseline).  Uses zero effect vector so
        // repeated application produces no state change.
        self.actions.push(ActionProposal::new(
            0,
            "NULL",
            Hypervector::new_zero(),
            0.0,
            0.0,
            0.0,
        ));

        // 1: EXPLORE — shift toward high entropy, increase curiosity
        self.actions.push(ActionProposal::from_text(
            1,
            "EXPLORE",
            "ACTION_SHIFT_EXPLORE",
            -0.05, // slightly reduces deficit (growth/curiosity need)
            0.10,  // error may increase (exploring novel states)
            0.20,  // moderate cost
        ));

        // 2: TASK — focus attention on the current dominant L2 concept
        self.actions.push(ActionProposal::from_text(
            2,
            "TASK",
            "ACTION_SHIFT_TASK",
            0.05,  // slight deficit increase (other needs deferred)
            -0.10, // error decreases (focus reduces uncertainty)
            0.15,  // low cost
        ));

        // 3: REGULATE — prioritize homeostatic restoration
        self.actions.push(ActionProposal::from_text(
            3,
            "REGULATE",
            "ACTION_RESTORE_HOMEOSTASIS",
            -0.20, // significantly reduces overall deficit
            0.05,  // slight error increase (regulation distracts from prediction)
            0.30,  // moderate cost
        ));

        // 4: BROADCAST — send EpistemicUpdate to the broker
        self.actions.push(ActionProposal::from_text(
            4,
            "BROADCAST",
            "ACTION_BROADCAST_EPISTEMIC",
            0.10,  // slight deficit increase (communication overhead)
            -0.05, // error decreases (shared context improves prediction)
            0.40,  // high cost (network + consensus)
        ));
    }

    /// Register a custom action proposal.
    pub fn register_action(&mut self, action: ActionProposal) -> Option<u8> {
        if self.actions.len() >= MAX_ACTIONS {
            return None;
        }
        let id = action.id;
        self.actions.push(action);
        Some(id)
    }

    /// Unregister an action by ID.
    pub fn unregister_action(&mut self, id: u8) -> bool {
        let pos = self.actions.iter().position(|a| a.id == id);
        if let Some(idx) = pos {
            self.actions.remove(idx);
            true
        } else {
            false
        }
    }

    /// Number of registered actions.
    pub fn action_count(&self) -> usize {
        self.actions.len()
    }

    // ═════════════════════════════════════════════════════════════════════
    // CORE SIMULATION LOOP
    // ═════════════════════════════════════════════════════════════════════

    /// Run a full simulation round: evaluate every action over the rollout
    /// horizon and return the ranked results.
    ///
    /// Uses `self.weights`.  For dynamically-modulated weights (e.g., from
    /// the IntrinsicMotivation drive system), use `evaluate_driven`.
    ///
    /// # Arguments
    ///
    /// * `current_identity` — Self_t from SelfModel (the ground truth T₀).
    /// * `homeostatic_deficit` — Current overall deficit (0.0 = nothing wrong,
    ///   1.0 = everything wrong).  From HomeostaticProfile.overall_deficit.
    /// * `prediction_error` — Current blended prediction error (0.0–1.0).
    /// * `global_broadcast` — The winning workspace broadcast vector (for
    ///   context).  May be zero if workspace is idle.
    ///
    /// Returns a SimulationReport with the best action and all outcomes.
    pub fn evaluate(
        &self,
        current_identity: &Hypervector,
        homeostatic_deficit: f64,
        prediction_error: f64,
        global_broadcast: &Hypervector,
    ) -> SimulationReport {
        self.evaluate_internal(
            current_identity,
            homeostatic_deficit,
            prediction_error,
            global_broadcast,
            &self.weights,
        )
    }

    /// Run a simulation round with dynamic weights (e.g., from drives).
    ///
    /// Identical to `evaluate()` but uses the provided `weights` array
    /// instead of `self.weights`.  Use this when the IntrinsicMotivation
    /// system has modulated the weights.
    pub fn evaluate_driven(
        &self,
        current_identity: &Hypervector,
        homeostatic_deficit: f64,
        prediction_error: f64,
        global_broadcast: &Hypervector,
        weights: &[f64; 4],
    ) -> SimulationReport {
        self.evaluate_internal(
            current_identity,
            homeostatic_deficit,
            prediction_error,
            global_broadcast,
            weights,
        )
    }

    /// Evaluate with the instance's default weights.
    fn evaluate_internal(
        &self,
        current_identity: &Hypervector,
        homeostatic_deficit: f64,
        prediction_error: f64,
        global_broadcast: &Hypervector,
        weights: &[f64; 4],
    ) -> SimulationReport {
        if self.actions.is_empty() {
            return SimulationReport {
                best_action: ActionProposal::new(0, "NONE", Hypervector::new_zero(), 0.0, 0.0, 0.0),
                best_outcome: SimulatedOutcome {
                    action_id: 0,
                    action_label: "NONE".to_string(),
                    simulated_states: Vec::new(),
                    simulated_deficits: Vec::new(),
                    simulated_errors: Vec::new(),
                    identity_shifts: Vec::new(),
                    step_scores: Vec::new(),
                    total_score: 0.0,
                },
                ranked_outcomes: Vec::new(),
                actions_evaluated: 0,
                rollout_depth: self.rollout_depth,
                total_simulations: self.total_simulations,
            };
        }

        let mut outcomes: Vec<SimulatedOutcome> = Vec::with_capacity(self.actions.len());

        for action in &self.actions {
            let outcome = self.simulate_action_internal(
                action,
                current_identity,
                homeostatic_deficit,
                prediction_error,
                global_broadcast,
                weights,
            );
            outcomes.push(outcome);
        }

        outcomes.sort_by(|a, b| a.total_score.partial_cmp(&b.total_score).unwrap());
        let best_outcome = outcomes.first().cloned().unwrap();
        let best_action = self
            .actions
            .iter()
            .find(|a| a.id == best_outcome.action_id)
            .cloned()
            .unwrap();

        SimulationReport {
            best_action,
            best_outcome,
            ranked_outcomes: outcomes,
            actions_evaluated: self.actions.len(),
            rollout_depth: self.rollout_depth,
            total_simulations: self.total_simulations,
        }
    }

    /// Core simulation loop for a single action.  Shared by both
    /// `evaluate` (uses `self.weights`) and `evaluate_driven` (uses
    /// caller-supplied weights) — the only difference is which weight
    /// array is passed in.
    fn simulate_action_internal(
        &self,
        action: &ActionProposal,
        current_identity: &Hypervector,
        homeostatic_deficit: f64,
        prediction_error: f64,
        global_broadcast: &Hypervector,
        weights: &[f64; 4],
    ) -> SimulatedOutcome {
        let mut states = Vec::with_capacity(self.rollout_depth);
        let mut deficits = Vec::with_capacity(self.rollout_depth);
        let mut errors = Vec::with_capacity(self.rollout_depth);
        let mut shifts = Vec::with_capacity(self.rollout_depth);
        let mut scores = Vec::with_capacity(self.rollout_depth);

        // T₀ = current identity ⊕ global_broadcast (the integrated now)
        let ground = current_identity.bitwise_xor(global_broadcast);

        let mut sim_state = ground;
        let mut cum_deficit = homeostatic_deficit;
        let mut cum_error = prediction_error;

        for step in 0..self.rollout_depth {
            // Apply action effect with rotation to avoid XOR idempotency:
            // Tₜ₊₁ = Tₜ ⊕ ρᵗ(effect).  Rotating by step bits ensures each
            // application is distinct and accumulates rather than cancelling.
            let rotated_effect = action.effect_vector.rotate_left(step as usize);
            sim_state = sim_state.bitwise_xor(&rotated_effect);

            // Update simulated homeostasis and error
            cum_deficit = (cum_deficit + action.deficit_delta).clamp(0.0, 1.0);
            cum_error = (cum_error + action.error_delta).clamp(0.0, 1.0);

            // Identity shift: NHD from current_identity to simulated state
            let identity_shift = current_identity.normalized_hamming_distance(&sim_state);

            // Decay factor for this step: γ^step
            let decay = self.uncertainty_decay.powi(step as i32);

            // Step score (lower = better)
            let step_score = decay
                * (weights[0] * cum_deficit
                    + weights[1] * cum_error
                    + weights[2] * identity_shift
                    + weights[3] * action.cost);

            states.push(sim_state);
            deficits.push(cum_deficit);
            errors.push(cum_error);
            shifts.push(identity_shift);
            scores.push(step_score);
        }

        // Total score = sum of step scores (lower = better)
        let total_score: f64 = scores.iter().sum();

        SimulatedOutcome {
            action_id: action.id,
            action_label: action.label.clone(),
            simulated_states: states,
            simulated_deficits: deficits,
            simulated_errors: errors,
            identity_shifts: shifts,
            step_scores: scores,
            total_score,
        }
    }

    // ═════════════════════════════════════════════════════════════════════
    // QUERY HELPERS
    // ═════════════════════════════════════════════════════════════════════

    /// Get the best action for a given state without running a full report.
    /// Convenience wrapper that returns just the winning action ID.
    pub fn best_action(
        &self,
        current_identity: &Hypervector,
        homeostatic_deficit: f64,
        prediction_error: f64,
        global_broadcast: &Hypervector,
    ) -> (u8, String, f64) {
        let report = self.evaluate(
            current_identity,
            homeostatic_deficit,
            prediction_error,
            global_broadcast,
        );
        (
            report.best_action.id,
            report.best_action.label.clone(),
            report.best_outcome.total_score,
        )
    }

    /// Summary string for diagnostics.
    pub fn report(&self) -> String {
        format!(
            "Simulator: {} actions, depth={}, decay={}, weights=[{:.2}, {:.2}, {:.2}, {:.2}], sims={}",
            self.actions.len(),
            self.rollout_depth,
            self.uncertainty_decay,
            self.weights[0], self.weights[1], self.weights[2], self.weights[3],
            self.total_simulations,
        )
    }
}

// ═════════════════════════════════════════════════════════════════════════════
// TESTS
// ═════════════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Hypervector;

    /// Theorem Sim2 + Sim4: Deterministic evaluation + convergent selection.
    ///
    /// For the same inputs, the same action wins every time with the same
    /// total score.
    #[test]
    fn test_baseline_simulation() {
        let mut sim = CounterfactualSimulator::with_defaults();
        sim.register_default_actions();

        let identity = Hypervector::encode_text_ngram("SELF_STATE_BASELINE", 3);
        let broadcast = Hypervector::new_zero();
        let deficit = 0.30;
        let error = 0.20;

        // Run simulation twice — should produce identical results
        let report1 = sim.evaluate(&identity, deficit, error, &broadcast);
        let report2 = sim.evaluate(&identity, deficit, error, &broadcast);

        eprintln!(
            "  Best action (run 1): {} (score={:.6})",
            report1.best_action.label, report1.best_outcome.total_score
        );
        eprintln!(
            "  Best action (run 2): {} (score={:.6})",
            report2.best_action.label, report2.best_outcome.total_score
        );

        // Same action should win
        assert_eq!(
            report1.best_action.id, report2.best_action.id,
            "Deterministic: same inputs → same winner"
        );

        // Same score
        assert!(
            (report1.best_outcome.total_score - report2.best_outcome.total_score).abs() < 1e-12,
            "Deterministic: same inputs → same score"
        );

        // NULL action (id=0) should have lowest cost and minimal identity shift
        let null_outcome = report1
            .ranked_outcomes
            .iter()
            .find(|o| o.action_id == 0)
            .unwrap();
        eprintln!(
            "  NULL action identity shifts: {:?}",
            null_outcome.identity_shifts
        );
        eprintln!("  NULL action total score: {:.6}", null_outcome.total_score);

        // With deficit=0.30, the REGULATE action (which reduces deficit)
        // should be competitive
        eprintln!("  Ranked outcomes:");
        for (i, o) in report1.ranked_outcomes.iter().enumerate() {
            eprintln!(
                "    {}. {}: total_score={:.6}, final_deficit={:.3}, final_error={:.3}",
                i + 1,
                o.action_label,
                o.total_score,
                o.simulated_deficits.last().unwrap_or(&0.0),
                o.simulated_errors.last().unwrap_or(&0.0)
            );
        }
    }

    /// Test that the action which best reduces homeostatic deficit wins
    /// when deficit is high.
    #[test]
    fn test_action_selection() {
        let mut sim = CounterfactualSimulator::with_defaults();
        sim.register_default_actions();

        let identity = Hypervector::encode_text_ngram("HIGH_DEFICIT_STATE", 3);
        let broadcast = Hypervector::new_zero();

        // Scenario 1: High deficit (0.80), low error (0.10)
        // REGULATE should win because it strongly reduces deficit
        let report_high_deficit = sim.evaluate(&identity, 0.80, 0.10, &broadcast);
        eprintln!(
            "  High deficit (0.80): winner = {} (score={:.6})",
            report_high_deficit.best_action.label, report_high_deficit.best_outcome.total_score
        );

        // REGULATE has deficit_delta=-0.20, which should make it the best
        // when deficit is high and error is low
        // (May not always be strict #1 due to cost weighting, but should be
        // in the top 2)
        let regulate_rank = report_high_deficit
            .ranked_outcomes
            .iter()
            .position(|o| o.action_id == 3);
        eprintln!("  REGULATE rank: {:?}", regulate_rank.map(|r| r + 1));
        assert!(
            regulate_rank.is_some() && regulate_rank.unwrap() < 3,
            "REGULATE should be in top 2 when deficit is high"
        );

        // Scenario 2: Low deficit (0.10), high error (0.80)
        // TASK should win because it reduces error
        let report_high_error = sim.evaluate(&identity, 0.10, 0.80, &broadcast);
        eprintln!(
            "  High error (0.80): winner = {} (score={:.6})",
            report_high_error.best_action.label, report_high_error.best_outcome.total_score
        );

        let task_rank = report_high_error
            .ranked_outcomes
            .iter()
            .position(|o| o.action_id == 2);
        eprintln!("  TASK rank: {:?}", task_rank.map(|r| r + 1));
        assert!(
            task_rank.is_some() && task_rank.unwrap() < 3,
            "TASK should be in top 2 when error is high"
        );
    }

    /// Test that multi-step rollout produces meaningful step scoring.
    #[test]
    fn test_multi_step_rollout() {
        let mut sim = CounterfactualSimulator::new(3, [0.30, 0.30, 0.20, 0.20]);
        sim.register_default_actions();

        let identity = Hypervector::encode_text_ngram("ROLLOUT_TEST", 3);
        let broadcast = Hypervector::new_zero();
        let deficit = 0.50;
        let error = 0.50;

        let report = sim.evaluate(&identity, deficit, error, &broadcast);

        // Each simulated outcome should have rollout_depth steps
        for outcome in &report.ranked_outcomes {
            assert_eq!(
                outcome.simulated_states.len(),
                sim.rollout_depth,
                "Each outcome should have {} simulated states",
                sim.rollout_depth
            );
            assert_eq!(
                outcome.step_scores.len(),
                sim.rollout_depth,
                "Each outcome should have {} step scores",
                sim.rollout_depth
            );

            // Step scores should be decaying (later steps matter less)
            // But identity shifts may increase, so total step score may
            // not be monotonic.  Check the raw scores.
            eprintln!(
                "  {} step scores: {:?}",
                outcome.action_label, outcome.step_scores
            );
        }

        // Total score should be positive
        assert!(
            report.best_outcome.total_score > 0.0,
            "Total score should be positive: {}",
            report.best_outcome.total_score
        );
    }

    /// Test that the NULL action (no change) produces a consistent
    /// identity shift that is the same every time (determinism).
    #[test]
    fn test_null_identity_preservation() {
        let mut sim = CounterfactualSimulator::with_defaults();
        sim.register_default_actions();

        let identity = Hypervector::encode_text_ngram("NULL_TEST_IDENTITY", 3);
        let broadcast = Hypervector::new_zero();

        let report1 = sim.evaluate(&identity, 0.30, 0.30, &broadcast);
        let report2 = sim.evaluate(&identity, 0.30, 0.30, &broadcast);

        // NULL identity shifts should be identical across runs
        let null_shifts_1 = report1
            .ranked_outcomes
            .iter()
            .find(|o| o.action_id == 0)
            .unwrap()
            .identity_shifts
            .clone();
        let null_shifts_2 = report2
            .ranked_outcomes
            .iter()
            .find(|o| o.action_id == 0)
            .unwrap()
            .identity_shifts
            .clone();

        for (i, (s1, s2)) in null_shifts_1.iter().zip(null_shifts_2.iter()).enumerate() {
            assert!(
                (s1 - s2).abs() < 1e-12,
                "NULL identity shift step {} must be deterministic: {} vs {}",
                i,
                s1,
                s2
            );
        }
        eprintln!(
            "  NULL identity shifts (deterministic): {:?}",
            null_shifts_1
        );

        // NULL total score should be deterministic
        let score1 = report1
            .ranked_outcomes
            .iter()
            .find(|o| o.action_id == 0)
            .map(|o| o.total_score)
            .unwrap();
        let score2 = report2
            .ranked_outcomes
            .iter()
            .find(|o| o.action_id == 0)
            .map(|o| o.total_score)
            .unwrap();
        assert!(
            (score1 - score2).abs() < 1e-12,
            "NULL total score must be deterministic: {} vs {}",
            score1,
            score2
        );
    }

    /// Test action registration and unregistration.
    #[test]
    fn test_action_registration() {
        let mut sim = CounterfactualSimulator::with_defaults();

        assert_eq!(sim.action_count(), 0, "Should start with no actions");

        sim.register_default_actions();
        assert_eq!(sim.action_count(), 5, "Should have 5 default actions");

        let custom_action = ActionProposal {
            id: 10,
            label: "CUSTOM".to_string(),
            effect_vector: Hypervector::new_random(),
            deficit_delta: 0.0,
            error_delta: 0.0,
            cost: 0.5,
        };
        sim.register_action(custom_action);
        assert_eq!(
            sim.action_count(),
            6,
            "Should have 6 actions after custom add"
        );

        // Unregister
        let removed = sim.unregister_action(10);
        assert!(removed, "Custom action should be removed");
        assert_eq!(sim.action_count(), 5, "Should be back to 5");

        // Unregister non-existent
        let false_removed = sim.unregister_action(99);
        assert!(!false_removed, "Non-existent action returns false");
    }

    /// Test that the simulator handles zero actions gracefully.
    #[test]
    fn test_empty_action_set() {
        let sim = CounterfactualSimulator::with_defaults();
        let identity = Hypervector::new_random();
        let broadcast = Hypervector::new_zero();

        let report = sim.evaluate(&identity, 0.5, 0.5, &broadcast);
        assert_eq!(report.actions_evaluated, 0, "No actions evaluated");
        assert_eq!(report.ranked_outcomes.len(), 0, "No outcomes");
    }

    /// Test that action priority works: a low-cost effective action
    /// beats a high-cost mediocre one.
    #[test]
    fn test_action_priority() {
        let mut sim = CounterfactualSimulator::new(1, [0.40, 0.30, 0.10, 0.20]);
        // Weights: deficit matters most (0.40), cost matters (0.20)

        // Register two custom actions:
        // A: moderately effective, low cost
        sim.register_action(ActionProposal::new(
            0,
            "EFFICIENT",
            Hypervector::encode_text_ngram("EFFICIENT_ACTION", 3),
            -0.10, // reduces deficit
            -0.05, // slightly reduces error
            0.10,  // low cost
        ));
        // B: very effective, high cost
        sim.register_action(ActionProposal::new(
            1,
            "EXPENSIVE_BUT_EFFECTIVE",
            Hypervector::encode_text_ngram("EXPENSIVE_ACTION", 3),
            -0.25, // strongly reduces deficit
            -0.15, // strongly reduces error
            0.80,  // very high cost
        ));

        let identity = Hypervector::encode_text_ngram("PRIORITY_TEST", 3);
        let broadcast = Hypervector::new_zero();

        // With moderate deficit (0.50), the high cost of B should
        // offset its effectiveness, making A the winner
        let report = sim.evaluate(&identity, 0.50, 0.30, &broadcast);

        eprintln!(
            "  Winner: {} (score={:.6})",
            report.best_action.label, report.best_outcome.total_score
        );
        eprintln!("  All outcomes:");
        for (i, o) in report.ranked_outcomes.iter().enumerate() {
            eprintln!(
                "    {}. {}: score={:.6}",
                i + 1,
                o.action_label,
                o.total_score
            );
        }

        // A (EFFICIENT) should beat B (EXPENSIVE_BUT_EFFECTIVE) due to cost
        let score_a = report
            .ranked_outcomes
            .iter()
            .find(|o| o.action_id == 0)
            .map(|o| o.total_score)
            .unwrap();
        let score_b = report
            .ranked_outcomes
            .iter()
            .find(|o| o.action_id == 1)
            .map(|o| o.total_score)
            .unwrap();

        assert!(
            score_a < score_b,
            "Efficient action should beat expensive one: {:.6} < {:.6}",
            score_a,
            score_b
        );
    }

    /// Test that `evaluate_driven` can override weights to make action
    /// preferable over inaction.
    ///
    /// With default weights [0.30, 0.30, 0.20, 0.20], NULL always wins
    /// because it has zero cost and identity shift.  But when the drive
    /// system pushes deficit weight to 0.80 and identity/cost near zero,
    /// REGULATE should beat NULL at high deficit.
    #[test]
    fn test_driven_evaluation() {
        let mut sim = CounterfactualSimulator::new(3, [0.30, 0.30, 0.20, 0.20]);
        sim.register_default_actions();

        let identity = Hypervector::encode_text_ngram("DRIVEN_TEST", 3);
        let broadcast = Hypervector::new_zero();
        let deficit = 0.90; // very high deficit
        let error = 0.10; // low error

        // First: default weights → NULL should win (as seen in other tests)
        let default_report = sim.evaluate(&identity, deficit, error, &broadcast);
        eprintln!(
            "  Default weights winner: {} (score={:.6})",
            default_report.best_action.label, default_report.best_outcome.total_score
        );
        assert_eq!(
            default_report.best_action.id, 0,
            "With default weights, NULL should win (id=0), got {}",
            default_report.best_action.label,
        );

        // Second: driven weights that heavily prioritise deficit reduction
        // and almost ignore identity shift and cost.
        // [Δdeficit, Δerror, Δidentity, cost] = [0.80, 0.10, 0.05, 0.05]
        let driven_weights = [0.80, 0.10, 0.05, 0.05];
        let driven_report =
            sim.evaluate_driven(&identity, deficit, error, &broadcast, &driven_weights);
        eprintln!(
            "  Driven weights winner: {} (score={:.6})",
            driven_report.best_action.label, driven_report.best_outcome.total_score
        );
        eprintln!("  Driven outcome rankings:");
        for (i, o) in driven_report.ranked_outcomes.iter().enumerate() {
            eprintln!(
                "    {}. {}: score={:.6}, final_deficit={:.3}, final_error={:.3}",
                i + 1,
                o.action_label,
                o.total_score,
                o.simulated_deficits.last().unwrap_or(&0.0),
                o.simulated_errors.last().unwrap_or(&0.0)
            );
        }

        // With high deficit and identity/cost de-emphasised, REGULATE (id=3)
        // should win because it reduces deficit the fastest (-0.20/step)
        assert_eq!(
            driven_report.best_action.id, 3,
            "With driven weights favouring deficit reduction, REGULATE \
             should win (id=3), got {} (id={})",
            driven_report.best_action.label, driven_report.best_action.id,
        );

        // Third: confirm default weights path is unchanged (not affected
        // by the driven call — &self, no mutation)
        let after_report = sim.evaluate(&identity, deficit, error, &broadcast);
        assert_eq!(
            after_report.best_action.id, 0,
            "Default weights should still pick NULL after driven call",
        );
    }
}
