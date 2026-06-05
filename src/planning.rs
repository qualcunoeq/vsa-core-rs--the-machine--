use crate::action::ActionRegistry;
use crate::resonator::ResonatorVocabulary;
use crate::Hypervector;

#[derive(Clone, Debug)]
pub struct PlanningStep {
    pub action: String,
    pub parameter: String,
    pub step_vector: Hypervector,
    pub cost: f64,
}

#[derive(Clone, Debug)]
pub struct PlanningTrajectory {
    pub steps: Vec<PlanningStep>,
    pub final_state: Hypervector,
    pub cumulative_cost: f64,
    pub score: f64,
}

pub fn get_action_cost(action: &str) -> f64 {
    match action {
        "sys_read" => 0.05,
        "sys_write" => 0.10,
        "execute_bash" => 0.25,
        _ => 0.50,
    }
}

pub fn find_optimal_trajectory(
    start_state: &Hypervector,
    goal_state: &Hypervector,
    e_world: &Hypervector,
    registry: &ActionRegistry,
    vocab: &ResonatorVocabulary,
    max_depth: usize,
) -> Option<PlanningTrajectory> {
    if vocab.terms.is_empty() {
        return None;
    }
    let mut best_trajectory: Option<PlanningTrajectory> = None;

    // Generate possible single step candidates from actions x vocab terms
    let mut candidates = Vec::new();
    for (act_name, act_hv) in &registry.actions {
        for (param_name, param_hv) in &vocab.terms {
            let step_vector = act_hv.bitwise_xor(param_hv);
            let cost = get_action_cost(act_name);
            candidates.push(PlanningStep {
                action: act_name.clone(),
                parameter: param_name.clone(),
                step_vector,
                cost,
            });
        }
    }

    // Queue for search tree traversal: (current_state, steps_taken, cumulative_cost)
    let mut queue: Vec<(Hypervector, Vec<PlanningStep>, f64)> =
        vec![(*start_state, Vec::new(), 0.0)];

    for depth in 1..=max_depth {
        let mut next_queue = Vec::new();
        for (curr_state, steps, cum_cost) in queue {
            for step in &candidates {
                // Avoid repeating identical steps consecutively
                if let Some(last) = steps.last() {
                    if last.action == step.action && last.parameter == step.parameter {
                        continue;
                    }
                }

                // S_{t+1} = \rho(S_t) \oplus A_t \oplus E_{world}
                let next_state = curr_state
                    .rotate_left(13)
                    .bitwise_xor(&step.step_vector)
                    .bitwise_xor(e_world);
                let next_cost = cum_cost + step.cost;
                let mut next_steps = steps.clone();
                next_steps.push(step.clone());

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

/// Simulates future state trajectory under passive environmental drift (no machine actions).
/// If any future state exceeds similarity threshold to any crisis concept, returns the step horizon.
pub fn simulate_threat_trajectory(
    start_state: &Hypervector,
    e_world: &Hypervector,
    steps: usize,
    crisis_concepts: &[Hypervector],
    threshold: f64,
) -> Option<usize> {
    let mut curr_state = *start_state;
    for step_idx in 1..=steps {
        // S_{t+1} = \rho(S_t) \oplus E_{world}
        curr_state = curr_state.rotate_left(13).bitwise_xor(e_world);
        for concept in crisis_concepts {
            let similarity = 1.0 - curr_state.normalized_hamming_distance(concept);
            if similarity >= threshold {
                return Some(step_idx);
            }
        }
    }
    None
}
