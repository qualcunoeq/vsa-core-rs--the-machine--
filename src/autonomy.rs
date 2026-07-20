use crate::action::ActionRegistry;
use crate::planning::find_optimal_trajectory;
use crate::resonator::{factorize_svo, ResonatorVocabulary};
use crate::{Hypervector, HD_DIMENSION};

// ─── Default SVO candidate lists ──────────────────────────────────────────

pub const DEFAULT_SUBJECTS: &[&str] = &[
    "System",
    "Agent",
    "Observer",
    "Process",
    "Component",
    "Module",
    "Interface",
    "Environment",
];

pub const DEFAULT_VERBS: &[&str] = &[
    "observe", "process", "respond", "adapt", "learn", "connect", "analyze", "signal",
];

pub const DEFAULT_OBJECTS: &[&str] = &[
    "state",
    "pattern",
    "signal",
    "context",
    "relation",
    "structure",
    "data",
    "event",
    "resource",
    "boundary",
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
        let normalized_dist = set_bits as f64 / HD_DIMENSION as f64;
        normalized_dist > self.dissonance_threshold && normalized_dist < 0.55
    }

    // ── Semantic intent formulation via planning layer ──────────────────

    /// Parse a dissonance vector and use the **planning layer** to find the
    /// optimal corrective action, rather than a hardcoded dispatch table.
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

// ══════════════════════════════════════════════════════════════════════════
// OPEN-ENDED DRIVES (Intrinsic Motivation & Goal Formulation)
// ══════════════════════════════════════════════════════════════════════════

/// A curiosity drive that seeks out novel information when dissonance is low.
///
/// Implements the "Intrinsic Motivation" loop: when the environment is stable
/// and no threats exist, the machine actively explores novel hypervector
/// regions to build a richer cognitive map.
#[derive(Clone, Debug)]
pub struct CuriosityDrive {
    /// Threshold below which exploration is triggered (low novelty)
    pub saturation_threshold: f64,
    /// How much the curiosity state has been satiated
    pub satiation: f64,
    /// Total number of novel concepts discovered
    pub discoveries: usize,
    /// Vector representing the current curiosity state
    pub curiosity_vector: Hypervector,
    /// Exploration mode: "focused" (targeted) or "diffuse" (random)
    pub exploration_mode: ExplorationMode,
    /// Tracks recently visited state regions to avoid re-exploration
    visited_regions: Vec<Hypervector>,
    max_visited: usize,
}

#[derive(Clone, Debug, PartialEq)]
pub enum ExplorationMode {
    /// Focused exploration around a specific curiosity target
    Focused(Hypervector),
    /// Diffuse exploration — randomly sample novel regions
    Diffuse,
    /// Directed by a user-injected goal state
    GoalDirected(Hypervector),
}

impl CuriosityDrive {
    pub fn new(saturation_threshold: f64) -> Self {
        CuriosityDrive {
            saturation_threshold,
            satiation: 0.0,
            discoveries: 0,
            curiosity_vector: Hypervector::new_random(),
            exploration_mode: ExplorationMode::Diffuse,
            visited_regions: Vec::new(),
            max_visited: 50,
        }
    }

    /// Compute novelty of the current world state.
    /// Returns a score 0.0 (completely familiar) to 1.0 (completely novel).
    pub fn compute_novelty(&self, world_state: &Hypervector, memory: &[Hypervector]) -> f64 {
        if memory.is_empty() {
            return 1.0; // Everything is novel
        }

        // Find the closest memory to the current state
        let mut max_sim = 0.0;
        for mem in memory {
            let sim = 1.0 - world_state.normalized_hamming_distance(mem);
            if sim > max_sim {
                max_sim = sim;
            }
        }

        // Novelty = 1 - familiarity
        1.0 - max_sim
    }

    /// Compute information gain: how much does this state add to our knowledge?
    pub fn information_gain(&self, world_state: &Hypervector, _memory: &[Hypervector]) -> f64 {
        let novelty = self.compute_novelty(world_state, _memory);

        // Information gain is high when:
        // 1. The state is novel (we learn something new)
        // 2. The state is not too chaotic (we can model it)
        // The curiosity drive seeks states with moderate-to-high novelty
        if novelty > 0.85 {
            // Too novel — might be noise
            novelty * 0.5
        } else if novelty > self.saturation_threshold {
            // Sweet spot: novel but comprehensible
            novelty * 2.0
        } else {
            // Too familiar — boring
            novelty * 0.1
        }
    }

    /// Decide whether to explore (return true) or exploit (return false).
    pub fn should_explore(&self, dissonance: f64, threat: f64) -> bool {
        // Only explore when safe and dissonance is low
        if threat > 0.3 {
            return false; // Too dangerous to explore
        }
        if dissonance > self.dissonance_threshold() {
            return false; // Too much dissonance — need to resolve first
        }

        // Check if curiosity is unsatiated
        self.satiation < self.saturation_threshold
    }

    fn dissonance_threshold(&self) -> f64 {
        0.43 // Matches the default dissonance threshold
    }

    /// Generate an exploration intent — a hypervector pointing toward
    /// a novel region of the state space.
    pub fn generate_exploration_intent(
        &mut self,
        current_state: &Hypervector,
        _memory: &[Hypervector],
    ) -> Hypervector {
        match &self.exploration_mode {
            ExplorationMode::Focused(target) => {
                // Move toward the curiosity target
                current_state.bitwise_xor(target)
            }
            ExplorationMode::GoalDirected(target) => {
                // Move toward the injected goal
                current_state.bitwise_xor(target)
            }
            ExplorationMode::Diffuse => {
                // Generate a random exploration vector, biased away from
                // recently visited regions
                let mut intent = Hypervector::new_random();

                // Repel from visited regions to encourage exploration
                for visited in &self.visited_regions {
                    let sim = 1.0 - current_state.normalized_hamming_distance(visited);
                    if sim > 0.60 {
                        // XOR with visited to get a "difference" vector
                        intent = intent.bitwise_xor(&current_state.bitwise_xor(visited));
                    }
                }

                intent
            }
        }
    }

    /// Record that we've visited a state region (for novelty tracking).
    pub fn record_visit(&mut self, state: &Hypervector) {
        self.visited_regions.push(*state);
        if self.visited_regions.len() > self.max_visited {
            self.visited_regions.remove(0);
        }

        // Satiate curiosity slightly with each visit
        self.satiation = (self.satiation + 0.05).min(1.0);
    }

    /// Set a focused exploration target.
    pub fn set_focus(&mut self, target: Hypervector) {
        self.exploration_mode = ExplorationMode::Focused(target);
    }

    /// Set a goal-directed exploration target.
    pub fn set_goal(&mut self, target: Hypervector) {
        self.exploration_mode = ExplorationMode::GoalDirected(target);
    }

    /// Reset to diffuse exploration.
    pub fn set_diffuse(&mut self) {
        self.exploration_mode = ExplorationMode::Diffuse;
    }

    /// Discover something new — increases discovery count and satiates curiosity.
    pub fn discover(&mut self) {
        self.discoveries += 1;
        self.satiation = (self.satiation + 0.15).min(1.0);
    }

    /// Decay satiation over time (curiosity builds again).
    pub fn decay(&mut self, amount: f64) {
        self.satiation = (self.satiation - amount).max(0.0);
    }
}

// ─── Goal Formulation Engine ──────────────────────────────────────────────

/// A goal formulation engine that allows the machine to accept arbitrary
/// target state vectors and plan a path to reach them.
///
/// This turns the machine into a universal problem solver:
/// 1. Accept a "Target State Vector" (from user injection, self-formulation, etc.)
/// 2. Use the Pathfinder's BFS trajectory optimizer to chart a path
/// 3. Execute the plan
/// 4. Iterate based on feedback
#[derive(Clone, Debug)]
pub struct GoalFormulationEngine {
    /// Currently active goal
    pub active_goal: Option<Goal>,
    /// History of past goals and their outcomes
    pub goal_history: Vec<GoalRecord>,
    /// Maximum number of goals to keep in history
    max_history: usize,
}

/// A structured goal with state vectors.
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct Goal {
    /// Unique goal identifier
    pub id: String,
    /// Human-readable description
    pub description: String,
    /// The target state hypervector (desired end state)
    pub target_state: Hypervector,
    /// The current state (start state)
    pub current_state: Hypervector,
    /// Priority (0.0 = low, 1.0 = critical)
    pub priority: f64,
    /// Whether the goal has been achieved
    pub achieved: bool,
    /// Sub-goals for hierarchical decomposition
    pub subgoals: Vec<Goal>,
}

/// Record of a completed goal attempt.
#[derive(Clone, Debug)]
pub struct GoalRecord {
    pub goal: Goal,
    pub success: bool,
    pub steps_taken: usize,
    pub final_cost: f64,
}

impl GoalFormulationEngine {
    pub fn new() -> Self {
        GoalFormulationEngine {
            active_goal: None,
            goal_history: Vec::new(),
            max_history: 100,
        }
    }

    /// Inject a new goal from an external source (user, sensor, etc.).
    pub fn inject_goal(
        &mut self,
        description: &str,
        target_state: Hypervector,
        current_state: Hypervector,
        priority: f64,
    ) -> String {
        let id = format!("goal_{}", chrono::Utc::now().format("%Y%m%d%H%M%S%3f"));
        let goal = Goal {
            id: id.clone(),
            description: description.to_string(),
            target_state,
            current_state,
            priority: priority.clamp(0.0, 1.0),
            achieved: false,
            subgoals: Vec::new(),
        };
        self.active_goal = Some(goal);
        id
    }

    /// Decompose a complex goal into sub-goals.
    /// Uses analogy with known past goals to find a decomposition strategy.
    pub fn decompose_goal(&mut self, _vocab: &ResonatorVocabulary) -> Vec<Goal> {
        let goal = match &self.active_goal {
            Some(g) => g.clone(),
            None => return Vec::new(),
        };

        // If no subgoals exist, try to create a decomposition
        if !goal.subgoals.is_empty() {
            return goal.subgoals;
        }

        // In a full implementation, this would use the planning layer to
        // find intermediate states. For now, we create a simple two-step
        // decomposition as a placeholder.
        let mid_state = Hypervector::bundle(&[&goal.current_state, &goal.target_state]);

        let subgoal1 = Goal {
            id: format!("{}_sg1", goal.id),
            description: format!("{} (step 1/2)", goal.description),
            target_state: mid_state,
            current_state: goal.current_state,
            priority: goal.priority,
            achieved: false,
            subgoals: Vec::new(),
        };

        let subgoal2 = Goal {
            id: format!("{}_sg2", goal.id),
            description: format!("{} (step 2/2)", goal.description),
            target_state: goal.target_state,
            current_state: mid_state,
            priority: goal.priority,
            achieved: false,
            subgoals: Vec::new(),
        };

        vec![subgoal1, subgoal2]
    }

    /// Check if the current goal has been achieved.
    pub fn check_achievement(&self, current_state: &Hypervector, threshold: f64) -> bool {
        match &self.active_goal {
            Some(goal) => {
                let sim = 1.0 - current_state.normalized_hamming_distance(&goal.target_state);
                sim >= threshold
            }
            None => false,
        }
    }

    /// Mark the current goal as achieved and record it.
    pub fn achieve_goal(&mut self, steps: usize, cost: f64) {
        if let Some(goal) = self.active_goal.take() {
            let mut goal = goal;
            goal.achieved = true;
            self.goal_history.push(GoalRecord {
                goal,
                success: true,
                steps_taken: steps,
                final_cost: cost,
            });
            self.prune_history();
        }
    }

    /// Mark the current goal as failed.
    pub fn fail_goal(&mut self, steps: usize, cost: f64) {
        if let Some(goal) = self.active_goal.take() {
            self.goal_history.push(GoalRecord {
                goal,
                success: false,
                steps_taken: steps,
                final_cost: cost,
            });
            self.prune_history();
        }
    }

    /// Get the target state vector from the active goal, if any.
    pub fn get_target_state(&self) -> Option<Hypervector> {
        self.active_goal.as_ref().map(|g| g.target_state)
    }

    /// Get the current goal's description.
    pub fn get_goal_description(&self) -> Option<String> {
        self.active_goal.as_ref().map(|g| g.description.clone())
    }

    fn prune_history(&mut self) {
        while self.goal_history.len() > self.max_history {
            self.goal_history.remove(0);
        }
    }

    /// Learn from past goal outcomes to improve future planning.
    /// Finds similar past goals and returns their success patterns.
    pub fn get_similar_past_outcomes(
        &self,
        target: &Hypervector,
        threshold: f64,
    ) -> Vec<&GoalRecord> {
        self.goal_history
            .iter()
            .filter(|record| {
                let sim = 1.0 - target.normalized_hamming_distance(&record.goal.target_state);
                sim >= threshold
            })
            .collect()
    }
}

// ─── Combined Drive System ────────────────────────────────────────────────

/// The complete drive system combining:
/// - Reactive: Dissonance-based (threat/crisis response)
/// - Proactive: Curiosity-based (exploration/information gain)
/// - Directed: Goal-based (user-injected or self-formulated targets)
#[derive(Clone, Debug)]
pub struct DriveSystem {
    pub autonomy: AutonomyDrive,
    pub curiosity: CuriosityDrive,
    pub goal_formulation: GoalFormulationEngine,
    /// Current dominant drive label
    pub active_drive: DriveLabel,
}

#[derive(Clone, Debug, PartialEq)]
pub enum DriveLabel {
    /// Resolving dissonance (threat/crisis response)
    Reactive(String),
    /// Exploring out of curiosity
    Curious(String),
    /// Pursuing a formulated goal
    GoalDirected(String),
    /// Idle / subconscious processing
    Idle,
}

impl DriveSystem {
    pub fn new(dissonance_threshold: f64) -> Self {
        DriveSystem {
            autonomy: AutonomyDrive::new(dissonance_threshold),
            curiosity: CuriosityDrive::new(dissonance_threshold),
            goal_formulation: GoalFormulationEngine::new(),
            active_drive: DriveLabel::Idle,
        }
    }

    /// Evaluate which drive should be active at this moment.
    /// Priority: Reactive > Goal-Directed > Curious > Idle
    pub fn evaluate_drive(
        &mut self,
        current_state: &Hypervector,
        dissonance_vector: &Hypervector,
        threat: f64,
        memory: &[Hypervector],
    ) -> DriveLabel {
        // 1. Check reactive drive (highest priority)
        if threat > 0.3
            || self
                .autonomy
                .evaluates_necessity_to_pivot(dissonance_vector)
        {
            let dissonance_dist = dissonance_vector.count_ones() as f64 / 10048.0;
            let label = DriveLabel::Reactive(format!(
                "Threat={:.2}, Dissonance={:.3}",
                threat, dissonance_dist
            ));
            self.active_drive = label.clone();
            return label;
        }

        // 2. Check goal-directed drive
        if self.goal_formulation.active_goal.is_some() {
            let desc = self
                .goal_formulation
                .get_goal_description()
                .unwrap_or_else(|| "Unknown goal".to_string());
            let label = DriveLabel::GoalDirected(desc);
            self.active_drive = label.clone();
            return label;
        }

        // 3. Check curiosity drive
        if self.curiosity.should_explore(0.0, threat) {
            let novelty = self.curiosity.compute_novelty(current_state, memory);
            let info_gain = self.curiosity.information_gain(current_state, memory);
            let label = DriveLabel::Curious(format!(
                "Novelty={:.3}, InfoGain={:.3}, Discoveries={}",
                novelty, info_gain, self.curiosity.discoveries
            ));
            self.active_drive = label.clone();
            return label;
        }

        // 4. Idle
        self.active_drive = DriveLabel::Idle;
        DriveLabel::Idle
    }
}

// ─── Tests ────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::action::ActionRegistry;
    use crate::resonator::{encode_svo, ResonatorVocabulary};

    fn setup_env() -> (
        ResonatorVocabulary,
        ActionRegistry,
        Vec<String>,
        Vec<String>,
        Vec<String>,
    ) {
        let mut vocab = ResonatorVocabulary::new();
        let registry = ActionRegistry::new();
        let subjects: Vec<String> = DEFAULT_SUBJECTS.iter().map(|s| s.to_string()).collect();
        let verbs: Vec<String> = DEFAULT_VERBS.iter().map(|v| v.to_string()).collect();
        let objects: Vec<String> = DEFAULT_OBJECTS.iter().map(|o| o.to_string()).collect();
        // Ensure default terms are registered in vocab for factorization
        for s in &subjects {
            vocab.register_term(s);
        }
        for v in &verbs {
            vocab.register_term(v);
        }
        for o in &objects {
            vocab.register_term(o);
        }
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

        let s_hv = vocab.get_vector("Agent").unwrap();
        let v_hv = vocab.get_vector("process").unwrap();
        let o_hv = vocab.get_vector("data").unwrap();
        let dissonance = encode_svo(s_hv, v_hv, o_hv);

        let current_state = Hypervector::new_random();
        let goal_state = Hypervector::new_random();
        let drift_seq = vec![Hypervector::new_zero(); 1];

        let result = drive.formulate_intent(
            &dissonance,
            &vocab,
            &registry,
            &subjects,
            &verbs,
            &objects,
            30,
            &current_state,
            &goal_state,
            &drift_seq,
            &[],
            0.0,
            &[],
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

    // ── Curiosity Drive Tests ─────────────────────────────────────────

    #[test]
    fn test_curiosity_novelty() {
        let curiosity = CuriosityDrive::new(0.43);
        let state = Hypervector::new_random();

        // No memory → everything is novel
        let novelty = curiosity.compute_novelty(&state, &[]);
        assert!((novelty - 1.0).abs() < 0.01);

        // State in memory → low novelty
        let memory = vec![state];
        let novelty = curiosity.compute_novelty(&state, &memory);
        assert!(novelty < 0.1);
    }

    #[test]
    fn test_curiosity_exploration_decision() {
        let curiosity = CuriosityDrive::new(0.43);

        // High threat → don't explore
        assert!(!curiosity.should_explore(0.3, 0.5));

        // Low threat, low dissonance → explore (if curiosity unsatiated)
        assert!(curiosity.should_explore(0.2, 0.1));
    }

    #[test]
    fn test_curiosity_discovery() {
        let mut curiosity = CuriosityDrive::new(0.43);
        assert_eq!(curiosity.discoveries, 0);
        curiosity.discover();
        assert_eq!(curiosity.discoveries, 1);
        assert!(curiosity.satiation > 0.0);
    }

    #[test]
    fn test_curiosity_satiation_decay() {
        let mut curiosity = CuriosityDrive::new(0.43);
        curiosity.discover();
        let satiation_before = curiosity.satiation;
        curiosity.decay(0.1);
        assert!(curiosity.satiation < satiation_before);
    }

    // ── Goal Formulation Tests ────────────────────────────────────────

    #[test]
    fn test_goal_injection() {
        let mut gfe = GoalFormulationEngine::new();
        let target = Hypervector::encode_text_ngram("desired_state", 3);
        let current = Hypervector::new_zero();

        let id = gfe.inject_goal("Test goal", target, current, 0.8);
        assert!(gfe.active_goal.is_some());
        assert_eq!(gfe.active_goal.as_ref().unwrap().id, id);
        assert_eq!(gfe.active_goal.as_ref().unwrap().priority, 0.8);
    }

    #[test]
    fn test_goal_achievement_check() {
        let mut gfe = GoalFormulationEngine::new();
        let target = Hypervector::encode_text_ngram("target_state", 3);
        let current = Hypervector::new_zero();

        gfe.inject_goal("Test", target, current, 0.5);

        // Same as target → achieved
        assert!(gfe.check_achievement(&target, 0.75));

        // Different → not achieved
        let different = Hypervector::new_random();
        assert!(!gfe.check_achievement(&different, 0.75));
    }

    #[test]
    fn test_goal_completion_recording() {
        let mut gfe = GoalFormulationEngine::new();
        let target = Hypervector::encode_text_ngram("target", 3);
        let current = Hypervector::new_zero();

        gfe.inject_goal("Test goal", target, current, 0.5);
        gfe.achieve_goal(5, 1.2);

        assert!(gfe.active_goal.is_none());
        assert_eq!(gfe.goal_history.len(), 1);
        assert!(gfe.goal_history[0].success);
    }

    #[test]
    fn test_drive_system_prioritization() {
        let mut ds = DriveSystem::new(0.43);
        let state = Hypervector::new_random();
        let dissonance = Hypervector::new_zero(); // No dissonance
        let memory = vec![];

        // No threats, no goals, fresh curiosity → should be Curious or Idle
        let drive = ds.evaluate_drive(&state, &dissonance, 0.0, &memory);
        assert!(
            matches!(drive, DriveLabel::Curious(_)) || drive == DriveLabel::Idle,
            "Should be curious or idle, got {:?}",
            drive
        );
    }

    #[test]
    fn test_drive_system_reactive_priority() {
        let mut ds = DriveSystem::new(0.43);
        let state = Hypervector::new_random();
        let dissonance = Hypervector::new_random(); // High dissonance
        let memory = vec![];

        // High threat → reactive
        let drive = ds.evaluate_drive(&state, &dissonance, 0.8, &memory);
        assert!(
            matches!(drive, DriveLabel::Reactive(..)),
            "High threat should be reactive, got {:?}",
            drive
        );
    }

    #[test]
    fn test_goal_decomposition() {
        let mut gfe = GoalFormulationEngine::new();
        let vocab = ResonatorVocabulary::new();
        let target = Hypervector::encode_text_ngram("complex_target", 3);
        let current = Hypervector::new_zero();

        gfe.inject_goal("Complex goal", target, current, 0.9);
        let subgoals = gfe.decompose_goal(&vocab);

        // Should produce subgoals
        assert_eq!(subgoals.len(), 2);
        assert!(subgoals[0].id.contains("sg1"));
        assert!(subgoals[1].id.contains("sg2"));
    }
}
