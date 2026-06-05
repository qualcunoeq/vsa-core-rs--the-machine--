use crate::action::{ActionProfile, ActionRegistry};
use crate::resonator::ResonatorVocabulary;
use crate::Hypervector;

// ─── Constants ────────────────────────────────────────────────────────────

/// Exponential scaling factor for the crisis-risk penalty.
/// Higher values penalise crisis-proximate actions more aggressively.
pub const LAMBDA: f64 = 5.0;

/// Similarity threshold below which no crisis-risk penalty is applied.
/// If `Sim(S_{t+1}, C_crisis) ≤ θ_safe` the penalty term is zero.
pub const THETA_SAFE: f64 = 0.50;

// ─── Data types ───────────────────────────────────────────────────────────

#[derive(Clone, Debug)]
pub struct PlanningStep {
    pub action: String,
    pub parameter: String,
    pub step_vector: Hypervector,
    /// The *actual* cost recorded for this step (may differ from base cost
    /// due to dynamic crisis-risk adjustment).
    pub cost: f64,
}

#[derive(Clone, Debug)]
pub struct PlanningTrajectory {
    pub steps: Vec<PlanningStep>,
    pub final_state: Hypervector,
    pub cumulative_cost: f64,
    pub score: f64,
}

// ─── Regime-Adaptive Drift Forecasting ────────────────────────────────────

/// A single drift regime representing one possible environmental trajectory.
#[derive(Clone, Debug)]
pub struct DriftRegime {
    /// Human-readable label (e.g. "stable", "volatile", "crisis")
    pub label: String,
    /// Bayesian confidence weight (will be normalized to sum to 1.0)
    pub weight: f64,
    /// One drift vector per simulation step; length determines forecast horizon
    pub drift_sequence: Vec<Hypervector>,
}

/// A probabilistic collection of drift regimes forming a Bayesian Model
/// Averaging (BMA) forecast of environmental dynamics.
#[derive(Clone, Debug)]
pub struct DriftForecast {
    pub regimes: Vec<DriftRegime>,
}

impl DriftForecast {
    pub fn new() -> Self {
        DriftForecast {
            regimes: Vec::new(),
        }
    }

    /// Add a regime and auto-normalise all weights so they sum to 1.0
    pub fn add_regime(
        &mut self,
        label: &str,
        weight: f64,
        drift_sequence: Vec<Hypervector>,
    ) {
        self.regimes.push(DriftRegime {
            label: label.to_string(),
            weight,
            drift_sequence,
        });
        self.normalize_weights();
    }

    pub fn normalize_weights(&mut self) {
        let total: f64 = self.regimes.iter().map(|r| r.weight).sum();
        if total > 0.0 {
            for regime in &mut self.regimes {
                regime.weight /= total;
            }
        }
    }

    pub fn is_empty(&self) -> bool {
        self.regimes.is_empty()
    }

    /// The longest horizon across all regimes
    pub fn max_horizon(&self) -> usize {
        self.regimes
            .iter()
            .map(|r| r.drift_sequence.len())
            .max()
            .unwrap_or(0)
    }
}

// ─── Dynamic action-cost calculation ──────────────────────────────────────

/// Calculate the VSA-native dynamic cost for an action given the projected
/// next-state and the crisis concepts.
///
/// $$Cost_{dyn} = C_{base} \times (1 + \beta \times e^{\lambda \cdot \max(0,\; \text{Sim}(S_{t+1}, C_{crisis}) - \theta_{safe})})$$
///
/// The `regime_volatility` multiplier (0.0 → 1.0) amplifies `β`, so high-beta
/// actions become dramatically more expensive when the environment is unstable.
pub fn calculate_dynamic_cost(
    profile: &ActionProfile,
    next_state: &Hypervector,
    crisis_concepts: &[Hypervector],
    lambda: f64,
    theta_safe: f64,
    regime_volatility: f64,
) -> f64 {
    if crisis_concepts.is_empty() {
        return profile.base_cost;
    }

    // Highest similarity between the projected next-state and any crisis concept
    let max_crisis_sim = crisis_concepts
        .iter()
        .map(|c| 1.0 - next_state.normalized_hamming_distance(c))
        .fold(0.0, f64::max);

    // Risk-exposure above the safety threshold
    let risk_exposure = (max_crisis_sim - theta_safe).max(0.0);

    // Regime-volatility amplifies the effective beta
    let adjusted_beta = profile.risk_beta * (1.0 + regime_volatility * 2.0);

    // (e^(λ·x) - 1) ensures zero penalty when risk_exposure = 0
    let penalty = if risk_exposure > 0.0 {
        (lambda * risk_exposure).exp() - 1.0
    } else {
        0.0
    };

    profile.base_cost * (1.0 + adjusted_beta * penalty)
}

/// Convenience wrapper for use outside the pathfinder (e.g. logging).
pub fn get_action_cost_static(action: &str) -> f64 {
    match action {
        "sys_read" => 0.05,
        "sys_write" => 0.10,
        "execute_bash" => 0.25,
        _ => 0.50,
    }
}

// ─── Trajectory optimiser (dynamic drift + dynamic cost) ──────────────────

/// Breadth-first search for an optimal action trajectory that moves
/// `start_state` toward `goal_state` under a **dynamic** drift sequence.
///
/// Action costs are dynamically adjusted via `calculate_dynamic_cost`,
/// penalising actions that push the projected next-state toward any of the
/// `crisis_concepts`.  The `regime_volatility` parameter (0.0 → 1.0) further
/// amplifies risk-beta during turbulent periods.
pub fn find_optimal_trajectory(
    start_state: &Hypervector,
    goal_state: &Hypervector,
    drift_sequence: &[Hypervector],
    registry: &ActionRegistry,
    vocab: &ResonatorVocabulary,
    max_depth: usize,
    crisis_concepts: &[Hypervector],
    regime_volatility: f64,
    experiences: &[Hypervector],
) -> Option<PlanningTrajectory> {
    if vocab.terms.is_empty() {
        return None;
    }
    let mut best_trajectory: Option<PlanningTrajectory> = None;

    // Generate possible single step candidates from actions x vocab terms.
    // Candidate cost is set to 0.0 — the *actual* cost is computed
    // dynamically during the search.
    let mut candidates = Vec::new();
    for (act_name, profile) in &registry.actions {
        for (param_name, param_hv) in &vocab.terms {
            let step_vector = profile.vector.bitwise_xor(param_hv);
            candidates.push(PlanningStep {
                action: act_name.clone(),
                parameter: param_name.clone(),
                step_vector,
                cost: 0.0, // filled in during search
            });
        }
    }

    // Prepare experience bundle if available
    let exp_bundle = if !experiences.is_empty() {
        let refs: Vec<&Hypervector> = experiences.iter().collect();
        Some(Hypervector::bundle(&refs))
    } else {
        None
    };

    // Queue for search tree traversal: (current_state, steps_taken, cumulative_cost)
    let mut queue: Vec<(Hypervector, Vec<PlanningStep>, f64)> =
        vec![(*start_state, Vec::new(), 0.0)];

    for depth in 1..=max_depth {
        // Pick the drift vector for this depth (cycle if sequence is exhausted)
        let e_step = drift_sequence
            .get(depth - 1)
            .or_else(|| drift_sequence.last())
            .copied()
            .unwrap_or_else(Hypervector::new_zero);

        let mut next_queue = Vec::new();
        for (curr_state, steps, cum_cost) in queue {
            for step in &candidates {
                // Avoid repeating identical steps consecutively
                if let Some(last) = steps.last() {
                    if last.action == step.action && last.parameter == step.parameter {
                        continue;
                    }
                }

                // S_{t+1} = ρ(S_t) ⊕ A_t ⊕ drift[t]
                let next_state = curr_state
                    .rotate_left(13)
                    .bitwise_xor(&step.step_vector)
                    .bitwise_xor(&e_step);

                // ── Dynamic cost ──────────────────────────────────────
                let profile = match registry.get_profile(&step.action) {
                    Some(p) => p,
                    None => continue, // unknown action — skip
                };
                let mut step_cost = calculate_dynamic_cost(
                    profile,
                    &next_state,
                    crisis_concepts,
                    LAMBDA,
                    THETA_SAFE,
                    regime_volatility,
                );

                // ── VSA Outcome-Vector Learning Penalty ────────────────
                if let Some(ref exp_b) = exp_bundle {
                    let a_vec = profile.vector;
                    let p_vec = step.step_vector.bitwise_xor(&a_vec);
                    let o_est = exp_b.bitwise_xor(&a_vec).bitwise_xor(&p_vec).bitwise_xor(&curr_state);

                    let v_success = Hypervector::encode_text_ngram("SUCCESS", 3);
                    let v_failure = Hypervector::encode_text_ngram("FAILURE", 3);
                    let sim_success = 1.0 - o_est.normalized_hamming_distance(&v_success);
                    let sim_failure = 1.0 - o_est.normalized_hamming_distance(&v_failure);

                    if sim_failure > sim_success && sim_failure > 0.55 {
                        let penalty = (sim_failure - sim_success) * 0.5;
                        step_cost += penalty;
                    }
                }

                let next_cost = cum_cost + step_cost;
                let mut next_steps = steps.clone();
                let mut step_with_cost = step.clone();
                step_with_cost.cost = step_cost;
                next_steps.push(step_with_cost);

                // Score = Similarity to Goal - Cumulative Action Costs
                let similarity = 1.0 - next_state.normalized_hamming_distance(goal_state);
                let score = similarity - next_cost;

                let traj = PlanningTrajectory {
                    steps: next_steps.clone(),
                    final_state: next_state,
                    cumulative_cost: next_cost,
                    score,
                };

                if best_trajectory.is_none() || score > best_trajectory.as_ref().unwrap().score {
                    best_trajectory = Some(traj);
                }

                if depth < max_depth {
                    next_queue.push((next_state, next_steps, next_cost));
                }
            }
        }
        queue = next_queue;
    }

    best_trajectory
}

// ─── Probabilistic threat trajectory simulation ───────────────────────────

/// Simulates future state trajectories under **multiple probabilistic drift
/// regimes** and returns the Bayesian-Model-Averaged step count until a
/// crisis concept is matched above `threshold`.
pub fn simulate_threat_trajectory(
    start_state: &Hypervector,
    forecast: &DriftForecast,
    crisis_concepts: &[Hypervector],
    threshold: f64,
) -> Option<f64> {
    if forecast.regimes.is_empty() || crisis_concepts.is_empty() {
        return None;
    }

    let max_steps = forecast.max_horizon();
    if max_steps == 0 {
        return None;
    }

    let mut weighted_sum = 0.0;
    let mut total_weight_reaching_crisis = 0.0;

    for regime in &forecast.regimes {
        let mut curr_state = *start_state;
        let mut steps_to_crisis: Option<usize> = None;

        for step_idx in 0..regime.drift_sequence.len().min(max_steps) {
            // S_{t+1} = ρ(S_t) ⊕ drift[step_idx]
            curr_state = curr_state
                .rotate_left(13)
                .bitwise_xor(&regime.drift_sequence[step_idx]);

            let hit = crisis_concepts.iter().any(|concept| {
                1.0 - curr_state.normalized_hamming_distance(concept) >= threshold
            });

            if hit {
                steps_to_crisis = Some(step_idx + 1);
                break;
            }
        }

        if let Some(steps) = steps_to_crisis {
            weighted_sum += regime.weight * steps as f64;
            total_weight_reaching_crisis += regime.weight;
        }
    }

    if total_weight_reaching_crisis > 0.0 {
        Some(weighted_sum / total_weight_reaching_crisis)
    } else {
        None
    }
}

/// Legacy single-regime wrapper for backward compatibility.
pub fn simulate_threat_trajectory_static(
    start_state: &Hypervector,
    e_world: &Hypervector,
    steps: usize,
    crisis_concepts: &[Hypervector],
    threshold: f64,
) -> Option<usize> {
    let mut forecast = DriftForecast::new();
    let drift_sequence = vec![*e_world; steps];
    forecast.add_regime("static", 1.0, drift_sequence);

    simulate_threat_trajectory(start_state, &forecast, crisis_concepts, threshold)
        .map(|v| v.round() as usize)
}

// ─── Utility: EWMA-weighted bundling of drift deltas ──────────────────────

/// Build an exponentially-weighted bundle from a time-ordered slice of
/// hypervectors (most recent = last element).
pub fn bundle_weighted_ewma(deltas: &[Hypervector], half_life: usize) -> Hypervector {
    if deltas.is_empty() {
        return Hypervector::new_zero();
    }
    if deltas.len() == 1 {
        return deltas[0];
    }

    let n = deltas.len();
    let mut weighted_refs: Vec<&Hypervector> = Vec::with_capacity(n * 4);

    for (i, hv) in deltas.iter().enumerate() {
        let age = (n - 1 - i) as f64; // 0 = newest
        let raw_weight = (-age * std::f64::consts::LN_2 / half_life as f64).exp();
        let copies = (raw_weight * 8.0).round().max(1.0) as usize;
        for _ in 0..copies {
            weighted_refs.push(hv);
        }
    }

    Hypervector::bundle(&weighted_refs)
}

/// Compute the average pairwise normalised Hamming distance across a set of
/// deltas.  Values near 0.0 indicate stable drift; values approaching 0.5
/// indicate a regime shift.
pub fn drift_variance(deltas: &[Hypervector]) -> f64 {
    if deltas.len() < 2 {
        return 0.0;
    }
    let mut total_dist = 0.0;
    let mut pairs = 0;
    for i in 0..deltas.len() {
        for j in (i + 1)..deltas.len() {
            total_dist += deltas[i].normalized_hamming_distance(&deltas[j]);
            pairs += 1;
        }
    }
    total_dist / pairs as f64
}

/// Build a probabilistic `DriftForecast` from the recent-delta history,
/// adjusting Bayesian Model Averaging weights based on historical regime errors.
pub fn build_drift_forecast(
    deltas: &[Hypervector],
    variance: f64,
    horizon: usize,
    half_life: usize,
    stable_err: f64,
    nominal_err: f64,
    volatile_err: f64,
) -> DriftForecast {
    let mut forecast = DriftForecast::new();

    if deltas.is_empty() {
        let zero_seq = vec![Hypervector::new_zero(); horizon];
        forecast.add_regime("null", 1.0, zero_seq);
        return forecast;
    }

    let nominal = bundle_weighted_ewma(deltas, half_life);

    if variance <= 0.38 {
        let seq = vec![nominal; horizon];
        forecast.add_regime("nominal", 1.0, seq);
        return forecast;
    }

    // ── High variance (regime shift detected): multi-regime BMA ──

    // Stable regime: bundle with old-delta bias (reverse EWMA)
    let mut reversed: Vec<Hypervector> = deltas.to_vec();
    reversed.reverse();
    let stable_drift = bundle_weighted_ewma(&reversed, half_life);

    // Volatile regime: amplify the most recent delta
    let newest = deltas.last().copied().unwrap_or(nominal);
    let amp_refs: Vec<&Hypervector> =
        std::iter::repeat(&newest).take(5).chain(std::iter::once(&nominal)).collect();
    let volatile_drift = Hypervector::bundle(&amp_refs);

    // Weights spread proportional to uncertainty (prior)
    let uncertainty = (variance - 0.38) / 0.12;
    let prior_stable = 0.15 + uncertainty * 0.25;
    let prior_volatile = 0.15 + uncertainty * 0.25;
    let prior_nominal = (1.0 - prior_stable - prior_volatile).max(0.0);

    // Performance-based scaling factor (posterior scaling)
    let perf_stable = 1.0 / (stable_err + 0.05);
    let perf_nominal = 1.0 / (nominal_err + 0.05);
    let perf_volatile = 1.0 / (volatile_err + 0.05);

    let mut stable_weight = prior_stable * perf_stable;
    let mut nominal_weight = prior_nominal * perf_nominal;
    let mut volatile_weight = prior_volatile * perf_volatile;

    // Normalise
    let total_w = stable_weight + nominal_weight + volatile_weight;
    if total_w > 0.0 {
        stable_weight /= total_w;
        nominal_weight /= total_w;
        volatile_weight /= total_w;
    }

    let stable_seq = vec![stable_drift; horizon];
    let nominal_seq = vec![nominal; horizon];
    let volatile_seq = vec![volatile_drift; horizon];

    forecast.add_regime("stable", stable_weight, stable_seq);
    forecast.add_regime("nominal", nominal_weight, nominal_seq);
    forecast.add_regime("volatile", volatile_weight, volatile_seq);

    forecast
}

// ─── Tests ────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::action::ActionRegistry;
    use crate::resonator::ResonatorVocabulary;

    // ── Dynamic cost tests ───────────────────────────────────────────────

    #[test]
    fn test_dynamic_cost_no_crisis() {
        let profile = ActionProfile {
            vector: Hypervector::new_random(),
            base_cost: 0.25,
            risk_beta: 1.5,
        };
        let state = Hypervector::new_random();
        let crisis = Hypervector::new_random(); // orthogonal to state
        let cost = calculate_dynamic_cost(&profile, &state, &[crisis], 5.0, 0.50, 0.0);
        // No similarity to crisis → cost should equal base_cost
        // With low crisis-similarity the cost should be near base_cost;
        // relax tolerance because random 10k-bit vectors can occasionally
        // land slightly above θ_safe (0.50) by chance.
        assert!(cost < 0.35, "cost should be near base_cost (0.25): {}", cost);
    }

    #[test]
    fn test_dynamic_cost_at_crisis() {
        let crisis = Hypervector::new_random();
        let profile = ActionProfile {
            vector: Hypervector::new_random(),
            base_cost: 0.25,
            risk_beta: 1.5,
        };
        // next_state IS the crisis → risk_exposure ≈ 1.0 - 0.50 = 0.50
        // penalty = e^(5.0 * 0.50) = e^2.5 ≈ 12.18
        // adjusted_beta = 1.5 * (1.0 + 0.0) = 1.5
        // cost = 0.25 * (1.0 + 1.5 * 12.18) ≈ 0.25 * 19.27 ≈ 4.82
        let cost = calculate_dynamic_cost(&profile, &crisis, &[crisis], 5.0, 0.50, 0.0);
        assert!(cost > 4.0, "cost should be heavily penalised: {}", cost);
    }

    #[test]
    fn test_dynamic_cost_regime_volatility_amplifies() {
        let crisis = Hypervector::new_random();
        let profile = ActionProfile {
            vector: Hypervector::new_random(),
            base_cost: 0.25,
            risk_beta: 1.5,
        };

        let cost_stable = calculate_dynamic_cost(&profile, &crisis, &[crisis], 5.0, 0.50, 0.0);
        let cost_volatile =
            calculate_dynamic_cost(&profile, &crisis, &[crisis], 5.0, 0.50, 1.0);

        assert!(
            cost_volatile > cost_stable,
            "volatile regime should amplify cost: stable={}, volatile={}",
            cost_stable,
            cost_volatile
        );
    }

    #[test]
    fn test_dynamic_cost_low_beta_immune() {
        let crisis = Hypervector::new_random();
        let profile = ActionProfile {
            vector: Hypervector::new_random(),
            base_cost: 0.05,
            risk_beta: 0.1, // sys_read-like
        };
        let high_beta = ActionProfile {
            vector: Hypervector::new_random(),
            base_cost: 0.25,
            risk_beta: 1.5, // execute_bash-like
        };

        let cost_low = calculate_dynamic_cost(&profile, &crisis, &[crisis], 5.0, 0.50, 1.0);
        let cost_high =
            calculate_dynamic_cost(&high_beta, &crisis, &[crisis], 5.0, 0.50, 1.0);

        assert!(
            cost_high > cost_low * 5.0,
            "high-beta action should be far more penalised: low={}, high={}",
            cost_low,
            cost_high
        );
    }

    #[test]
    fn test_dynamic_cost_no_crisis_concepts() {
        let profile = ActionProfile {
            vector: Hypervector::new_random(),
            base_cost: 0.10,
            risk_beta: 0.5,
        };
        let state = Hypervector::new_random();
        let cost = calculate_dynamic_cost(&profile, &state, &[], 5.0, 0.50, 0.0);
        assert!((cost - 0.10).abs() < 0.01);
    }

    // ── EWMA / variance / forecast tests ─────────────────────────────────

    #[test]
    fn test_bundle_weighted_ewma_identical() {
        let v = Hypervector::new_random();
        let deltas = vec![v, v, v, v, v];
        let bundled = bundle_weighted_ewma(&deltas, 2);
        assert_eq!(bundled, v);
    }

    #[test]
    fn test_bundle_weighted_ewma_single() {
        let v = Hypervector::new_random();
        let bundled = bundle_weighted_ewma(&[v], 2);
        assert_eq!(bundled, v);
    }

    #[test]
    fn test_bundle_weighted_ewma_empty() {
        let bundled = bundle_weighted_ewma(&[], 2);
        assert_eq!(bundled, Hypervector::new_zero());
    }

    #[test]
    fn test_drift_variance_identical() {
        let v = Hypervector::new_random();
        let deltas = vec![v, v, v];
        assert!(drift_variance(&deltas) < 0.01);
    }

    #[test]
    fn test_drift_variance_random() {
        let d1 = Hypervector::new_random();
        let d2 = Hypervector::new_random();
        let d3 = Hypervector::new_random();
        let var = drift_variance(&[d1, d2, d3]);
        assert!(var > 0.40 && var < 0.60);
    }

    #[test]
    fn test_build_drift_forecast_low_variance() {
        let v = Hypervector::new_random();
        let deltas = vec![v, v, v, v, v];
        let var = drift_variance(&deltas);
        let forecast = build_drift_forecast(&deltas, var, 10, 3, 0.5, 0.5, 0.5);
        assert_eq!(forecast.regimes.len(), 1);
        assert_eq!(forecast.regimes[0].label, "nominal");
    }

    #[test]
    fn test_build_drift_forecast_high_variance() {
        let d1 = Hypervector::new_random();
        let d2 = Hypervector::new_random();
        let d3 = Hypervector::new_random();
        let d4 = Hypervector::new_random();
        let d5 = Hypervector::new_random();
        let deltas = vec![d1, d2, d3, d4, d5];
        let var = drift_variance(&deltas);
        let forecast = build_drift_forecast(&deltas, var, 10, 3, 0.5, 0.5, 0.5);
        assert_eq!(forecast.regimes.len(), 3);
        let total: f64 = forecast.regimes.iter().map(|r| r.weight).sum();
        assert!((total - 1.0).abs() < 0.001);
    }

    // ── Threat trajectory simulation tests ───────────────────────────────

    #[test]
    fn test_simulate_threat_trajectory_no_crisis() {
        let s0 = Hypervector::new_random();
        let crisis = Hypervector::new_random();
        let mut forecast = DriftForecast::new();
        let zero_seq = vec![Hypervector::new_zero(); 10];
        forecast.add_regime("null", 1.0, zero_seq);

        let result = simulate_threat_trajectory(&s0, &forecast, &[crisis], 0.80);
        assert!(result.is_none());
    }

    #[test]
    fn test_simulate_threat_trajectory_immediate_crisis() {
        let crisis = Hypervector::new_random();
        let drift = crisis.bitwise_xor(&crisis.rotate_left(13));
        let mut forecast = DriftForecast::new();
        let drift_seq = vec![drift; 5];
        forecast.add_regime("persistent", 1.0, drift_seq);

        let result = simulate_threat_trajectory(&crisis, &forecast, &[crisis], 0.99);
        assert!(result.is_some());
        assert!((result.unwrap() - 1.0).abs() < 0.001);
    }

    #[test]
    fn test_multiple_regime_weighting() {
        let crisis = Hypervector::new_zero();
        let start = Hypervector::new_zero();

        let hit_seq = vec![Hypervector::new_zero(); 5];
        let miss_seq = vec![Hypervector::new_random(); 10];

        let mut forecast = DriftForecast::new();
        forecast.add_regime("hit", 0.5, hit_seq);
        forecast.add_regime("miss", 0.5, miss_seq);

        let result = simulate_threat_trajectory(&start, &forecast, &[crisis], 0.99);
        assert!(result.is_some());
        assert!((result.unwrap() - 1.0).abs() < 0.001);
    }

    #[test]
    fn test_simulate_threat_trajectory_static_backward_compat() {
        let s0 = Hypervector::new_random();
        let crisis = Hypervector::new_random();
        let e_world = Hypervector::new_zero();
        let result = simulate_threat_trajectory_static(&s0, &e_world, 5, &[crisis], 0.80);
        assert!(result.is_none() || result.unwrap() <= 5);
    }

    // ── Planning solver tests ────────────────────────────────────────────

    #[test]
    fn test_temporal_planning() {
        let reg = ActionRegistry::new();
        let mut vocab = ResonatorVocabulary::new();
        vocab.register_term("hosts");
        vocab.register_term("cargo check");

        let s0 = Hypervector::new_random();
        let e_world = Hypervector::new_zero();

        let act1_hv = reg.get_action_vector("sys_read").unwrap();
        let param1_hv = vocab.get_vector("hosts").unwrap();
        let step1 = act1_hv.bitwise_xor(param1_hv);

        let act2_hv = reg.get_action_vector("execute_bash").unwrap();
        let param2_hv = vocab.get_vector("cargo check").unwrap();
        let step2 = act2_hv.bitwise_xor(param2_hv);

        // S2 = ρ(ρ(S0) ⊕ step1 ⊕ drift) ⊕ step2 ⊕ drift
        let s1 = s0.rotate_left(13).bitwise_xor(&step1).bitwise_xor(&e_world);
        let goal_state = s1.rotate_left(13).bitwise_xor(&step2).bitwise_xor(&e_world);

        let drift_seq = vec![e_world; 2];
        let traj_opt = find_optimal_trajectory(
            &s0, &goal_state, &drift_seq, &reg, &vocab, 2,
            &[], 0.0, &[], // no crisis concepts → static costs
        );
        assert!(traj_opt.is_some(), "Should find a valid trajectory");

        let traj = traj_opt.unwrap();
        assert_eq!(traj.steps.len(), 2);
        assert_eq!(traj.steps[0].action, "sys_read");
        assert_eq!(traj.steps[1].action, "execute_bash");
    }

    #[test]
    fn test_planning_cost_optimization() {
        let reg = ActionRegistry::new();
        let mut vocab = ResonatorVocabulary::new();
        vocab.register_term("hosts");
        vocab.register_term("cargo check");

        let s0 = Hypervector::new_random();
        let e_world = Hypervector::new_zero();

        let act_hv = reg.get_action_vector("execute_bash").unwrap();
        let param_hv = vocab.get_vector("cargo check").unwrap();
        let step = act_hv.bitwise_xor(param_hv);
        let goal = s0.rotate_left(13).bitwise_xor(&step).bitwise_xor(&e_world);

        let drift_seq = vec![e_world; 2];
        let traj_opt = find_optimal_trajectory(
            &s0, &goal, &drift_seq, &reg, &vocab, 2,
            &[], 0.0, &[],
        );
        assert!(traj_opt.is_some());

        let traj = traj_opt.unwrap();
        assert_eq!(traj.steps.len(), 1);
        assert_eq!(traj.steps[0].action, "execute_bash");
        assert_eq!(traj.steps[0].parameter, "cargo check");
    }

    #[test]
    fn test_outcome_learning_penalises_failures() {
        let reg = ActionRegistry::new();
        let mut vocab = ResonatorVocabulary::new();
        vocab.register_term("hosts");

        let s0 = Hypervector::new_random();
        let e_world = Hypervector::new_zero();

        let act_read = reg.get_action_vector("sys_read").unwrap();
        let param_hosts = vocab.get_vector("hosts").unwrap();
        let step = act_read.bitwise_xor(param_hosts);
        let goal = s0.rotate_left(13).bitwise_xor(&step).bitwise_xor(&e_world);

        let drift_seq = vec![e_world; 1];
        let traj_neutral = find_optimal_trajectory(
            &s0, &goal, &drift_seq, &reg, &vocab, 1,
            &[], 0.0, &[],
        ).unwrap();

        let v_failure = Hypervector::encode_text_ngram("FAILURE", 3);
        let exp = act_read.bitwise_xor(param_hosts).bitwise_xor(&s0).bitwise_xor(&v_failure);
        
        let traj_penalised = find_optimal_trajectory(
            &s0, &goal, &drift_seq, &reg, &vocab, 1,
            &[], 0.0, &[exp],
        ).unwrap();

        assert!(
            traj_penalised.cumulative_cost > traj_neutral.cumulative_cost,
            "Cost should increase due to failure penalty: neutral={}, penalised={}",
            traj_neutral.cumulative_cost,
            traj_penalised.cumulative_cost
        );
    }
}
