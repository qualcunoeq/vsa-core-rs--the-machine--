use crate::action::ActionRegistry;
use crate::planning::find_optimal_trajectory;
use crate::resonator::{factorize_svo, ResonatorVocabulary};
use crate::Hypervector;

// ─── Default SVO candidate lists ──────────────────────────────────────────

pub const DEFAULT_SUBJECTS: &[&str] = &[
    "Agent-1", "Agent-2", "Agent-3", "Broker", "Finch",
    "Market", "News", "Infra",
];

pub const DEFAULT_VERBS: &[&str] = &[
    "read", "write", "execute", "panic", "sync", "breached",
];

pub const DEFAULT_OBJECTS: &[&str] = &[
    "hosts", "ledger", "crisis", "Stable",
    "Attack", "Breach", "Stealth", "Lehman",
    "admin", "server",
];

// ─── AutonomyDrive ────────────────────────────────────────────────────────

#[derive(Clone, Debug)]
pub struct AutonomyDrive {
    pub dissonance_threshold: f64,
}

impl AutonomyDrive {
    pub fn new(dissonance_threshold: f64) -> Self {
        AutonomyDrive {
            dissonance_threshold,
        }
    }

    pub fn calculate_dissonance(current: &Hypervector, historical: &Hypervector) -> Hypervector {
        current.bitwise_xor(historical)
    }

    pub fn evaluates_necessity_to_pivot(&self, dissonance: &Hypervector) -> bool {
        let set_bits = dissonance.count_ones();
        let normalized_dist = set_bits as f64 / 10048.0;
        normalized_dist > self.dissonance_threshold && normalized_dist < 0.55
    }

    // ── Semantic intent formulation via planning layer ──────────────────

    /// Parse a dissonance vector and use the **planning layer** to find the
    /// optimal corrective action, rather than a hardcoded dispatch table.
    ///
    /// 1. Parse the dissonance through the Simultaneous Resonator Network
    ///    to extract structured (S, V, O) understanding.
    /// 2. Call `find_optimal_trajectory(depth=1)` with the current state,
    ///    goal state, drift, crisis concepts, and past experiences.
    ///    The planner's dynamic cost function naturally penalises high-beta
    ///    actions when the environment is volatile or crisis-proximate.
    /// 3. Return the first step of the optimal trajectory as the corrective
    ///    intent, with a human-readable label.
    ///
    /// Returns `None` when parsing fails (hallucination filter) or the
    /// planner cannot find a viable corrective step.
    pub fn formulate_intent(
        &self,
        dissonance: &Hypervector,
        vocab: &ResonatorVocabulary,
        registry: &ActionRegistry,
        subjects: &[String],
        verbs: &[String],
        objects: &[String],
        max_iterations: usize,
        // Planning-layer parameters
        current_state: &Hypervector,
        goal_state: &Hypervector,
        drift_sequence: &[Hypervector],
        crisis_concepts: &[Hypervector],
        regime_volatility: f64,
        experiences: &[Hypervector],
    ) -> Option<(Hypervector, String)> {
        // 1. Parse dissonance through resonator (energy gate rejects hallucinations)
        let (_s_str, _v_str, _o_str, energy) =
            factorize_svo(dissonance, vocab, subjects, verbs, objects, max_iterations)?;

        // 2. Use the planning layer to find the optimal single-step correction.
        //    The dynamic cost function (calculate_dynamic_cost) naturally
        //    prices high-beta actions out of the market when the system is
        //    near crisis — no hardcoded dispatch table needed.
        let trajectory = find_optimal_trajectory(
            current_state,
            goal_state,
            drift_sequence,
            registry,
            vocab,
            1, // depth=1 — single corrective step
            crisis_concepts,
            regime_volatility,
            experiences,
        )?;

        let first_step = trajectory.steps.first()?;
        let intent = first_step.step_vector; // Already A ⊕ P from the planner

        let label = format!(
            "SVO:({:.2})→Plan: {} {} (cost={:.3})",
            energy, first_step.action, first_step.parameter, trajectory.cumulative_cost,
        );

        Some((intent, label))
    }
}

// ─── Tests ────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::action::ActionRegistry;
    use crate::resonator::{encode_svo, ResonatorVocabulary};

    fn setup_env() -> (ResonatorVocabulary, ActionRegistry, Vec<String>, Vec<String>, Vec<String>) {
        let vocab = ResonatorVocabulary::new();
        let registry = ActionRegistry::new();
        let subjects: Vec<String> = DEFAULT_SUBJECTS.iter().map(|s| s.to_string()).collect();
        let verbs: Vec<String> = DEFAULT_VERBS.iter().map(|v| v.to_string()).collect();
        let objects: Vec<String> = DEFAULT_OBJECTS.iter().map(|o| o.to_string()).collect();
        (vocab, registry, subjects, verbs, objects)
    }

    #[test]
    fn test_calculate_dissonance() {
        let v1 = Hypervector::new_random();
        let v2 = Hypervector::new_random();
        let dissonance = AutonomyDrive::calculate_dissonance(&v1, &v2);
        let reversed = dissonance.bitwise_xor(&v1);
        assert_eq!(reversed, v2);
    }

    #[test]
    fn test_necessity_to_pivot() {
        let drive = AutonomyDrive::new(0.43);
        let v1 = Hypervector::new_random();
        let diss_zero = AutonomyDrive::calculate_dissonance(&v1, &v1);
        assert!(!drive.evaluates_necessity_to_pivot(&diss_zero));

        let v2 = Hypervector::new_random();
        let diss_random = AutonomyDrive::calculate_dissonance(&v1, &v2);
        let dist = diss_random.normalized_hamming_distance(&Hypervector::new_zero());
        if dist > 0.43 && dist < 0.55 {
            assert!(drive.evaluates_necessity_to_pivot(&diss_random));
        }
    }

    #[test]
    fn test_formulate_intent_planning_routed() {
        let (vocab, registry, subjects, verbs, objects) = setup_env();
        let drive = AutonomyDrive::new(0.43);

        let s_hv = vocab.get_vector("Finch").unwrap();
        let v_hv = vocab.get_vector("write").unwrap();
        let o_hv = vocab.get_vector("ledger").unwrap();
        let dissonance = encode_svo(s_hv, v_hv, o_hv);

        let current_state = Hypervector::new_random();
        let goal_state = Hypervector::new_random();
        let drift_seq = vec![Hypervector::new_zero(); 1];

        let result = drive.formulate_intent(
            &dissonance, &vocab, &registry,
            &subjects, &verbs, &objects, 30,
            &current_state, &goal_state, &drift_seq,
            &[], 0.0, &[],
        );

        assert!(
            result.is_some(),
            "formulate_intent should resolve via planning layer"
        );
        let (_intent, label) = result.unwrap();
        assert!(
            label.contains("Plan:"),
            "Label should reflect planning dispatch: {}",
            label
        );
    }
}
