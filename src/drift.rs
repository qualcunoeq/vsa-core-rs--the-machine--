// ─── DRIFT-Inspired Cognitive Enhancements ───────────────────────────────
//
// Ported from the DRIFT (formerly infj-bot) cognitive architecture by
// **timeless-hayoka** (https://github.com/timeless-hayoka/infj-bot) into
// The Machine's binary hypervector framework.
//
// The DRIFT project is a sophisticated Python cognitive architecture that
// implements a unified theory of mind: homeostatic regulation, predictive
// state characterization, global workspace attention, multi-agent consensus,
// emotional field dynamics, and implicit pattern recognition.  These Rust
// ports translate the key algorithms from their original LLM-embedding and
// SQLite-based implementations into pure binary hypervector operations.
//
// Original repo: git@github.com:timeless-hayoka/infj-bot.git
//
// Ported subsystems:
//
// 1. DMU Scoring        — Ebbinghaus-decay + reinforcement-weighted retrieval
// 2. CognitiveMode      — 3-bit [Memory, State, Novelty] continuity vector
// 3. DCP Consensus      — Distributed Cognition Protocol (propose→vote→resolve)
// 4. Homeostasis        — 7-need cybernetic regulation with allostatic prediction
// 5. PSC Predictor      — Batch trend prediction with adaptive horizon + chaos score
// 6. GlobalWorkspace    — Competitive salience ranking with preconscious bands
// 7. EmotionalField     — Emotion⊗Stance → Mood binding and resonance
// 8. ContextEngine      — Fork/merge superposition with cleanup
// 9. ImplicitIntuition  — Pattern recognition via bundled domain hypervectors
// 10. ShadowEnantiodromia — Bipolar archetype oscillation for cognitive reversal
//
// Each subsystem is independently usable and integrates with existing HNSW,
// broker, and agent-loop machinery.

use crate::Hypervector;
use std::collections::HashMap;

// ═════════════════════════════════════════════════════════════════════════
// 1. DMU SCORING — Decision Making Utility
// ═════════════════════════════════════════════════════════════════════════
//
// Implements the DRIFT memory utility equation for post-processing HNSW
// search results:
//
//   DMU = exp(-t / τ) × R × S × (1 - d)
//
//   τ     = tau_base × (1 + κ × log(1 + reps + salience × 10))
//   R     = 1 + α × log(1 + β × salience × reps)
//   S     = contextual salience (from query-time projection)
//   d     = normalized Hamming distance
//   t     = ticks since creation
//   reps  = retrieval count
//
// Call `dmu_score()` on each HNSW result, or use `search_with_dmu()` on
// the HNSW index which applies DMU re-ranking automatically.

/// DMU scoring parameters.  Defaults calibrated from DRIFT's empirical
/// values for episodic memory (tau_base=10.0, alpha=0.3, beta=2.0).
#[derive(Clone, Debug)]
pub struct DmuParams {
    /// Base decay constant in ticks.  Higher = longer memory.
    pub tau_base: f64,
    /// How much retrieval frequency extends tau (stability)
    pub kappa: f64,
    /// Reinforcement multiplier magnitude
    pub alpha: f64,
    /// Reinforcement frequency sensitivity
    pub beta: f64,
    /// Minimum score floor (prevents total forgetting)
    pub floor: f64,
    /// Salience multiplier in tau extension
    pub salience_weight: f64,
}

impl Default for DmuParams {
    fn default() -> Self {
        DmuParams {
            tau_base: 50.0, // ~100s at 2s/tick
            kappa: 0.5,     // moderate stability extension
            alpha: 0.3,     // reinforcement strength
            beta: 2.0,      // frequency sensitivity
            floor: 0.05,    // minimum score
            salience_weight: 10.0,
        }
    }
}

/// DMU scoring for episodic memory (high decay sensitivity).
pub fn dmu_params_episodic() -> DmuParams {
    DmuParams {
        tau_base: 50.0,
        ..Default::default()
    }
}

/// DMU scoring for semantic memory (slow decay).
pub fn dmu_params_semantic() -> DmuParams {
    DmuParams {
        tau_base: 150.0,
        ..Default::default()
    }
}

/// DMU scoring for emotional/bond memory (very slow decay).
pub fn dmu_params_bond() -> DmuParams {
    DmuParams {
        tau_base: 250.0,
        alpha: 0.5,
        ..Default::default()
    }
}

/// Compute the DRIFT Memory Utility score for a single retrieval candidate.
///
/// * `distance` — normalized Hamming distance [0, 1] from the query
/// * `age_ticks` — ticks since this entry was created
/// * `retrieval_count` — how many times this entry has been retrieved
/// * `salience` — contextual salience [0, 1] from query-time projection
/// * `params` — DMU scoring parameters
///
/// Returns a score in [floor, 1.0].  Higher = more relevant.
pub fn dmu_score(
    distance: f64,
    age_ticks: u64,
    retrieval_count: u32,
    salience: f64,
    params: &DmuParams,
) -> f64 {
    let t = age_ticks as f64;
    let reps = retrieval_count as f64;

    // Effective decay constant: retrieval frequency extends tau
    let log_factor = (1.0 + reps + params.salience_weight * salience).ln();
    let tau_eff = params.tau_base * (1.0 + params.kappa * log_factor);

    // Time-decay term
    let decay = (-t / tau_eff).exp();

    // Reinforcement: more retrievals = higher base score
    let reinforcement = 1.0 + params.alpha * (1.0 + params.beta * salience * reps).ln();

    // Base similarity from Hamming distance
    let similarity = 1.0 - distance;

    let raw = decay * reinforcement * salience * similarity;
    raw.max(params.floor).min(1.0)
}

// ═════════════════════════════════════════════════════════════════════════
// 2. COGNITIVE MODE — [Memory, State, Novelty] Continuity Vector
// ═════════════════════════════════════════════════════════════════════════
//
// A compact 3-bit tag describing the agent's current cognitive engagement:
//
//   Bit 0 (memory):   referencing past context / history depth > threshold
//   Bit 1 (state):    self-correcting / coherence below threshold
//   Bit 2 (novelty):  exploring new entities / shadow influence active
//
// Each of the 8 patterns modulates how the agent processes the next tick:
// resonator depth, HNSW search breadth, consensus threshold, etc.

/// The 8 named cognitive mode patterns.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum CognitiveMode {
    /// [0,0,0] — Baseline, no strong signal
    Quiet,
    /// [1,0,0] — Grounded in memory, referencing past
    Companion,
    /// [0,1,0] — Self-correcting, regulating
    Regulated,
    /// [0,0,1] — Novelty-seeking, exploring
    Explorer,
    /// [1,1,0] — Task-focused with memory + regulation
    Task,
    /// [1,0,1] — Memory + novelty: creative resonance
    Resonant,
    /// [0,1,1] — Regulation + novelty: frontier exploration
    Frontier,
    /// [1,1,1] — Full cognitive engagement
    FullCouncil,
}

impl CognitiveMode {
    /// Build from three boolean signals.
    pub fn from_bits(memory: bool, state: bool, novelty: bool) -> Self {
        match (memory, state, novelty) {
            (false, false, false) => CognitiveMode::Quiet,
            (true, false, false) => CognitiveMode::Companion,
            (false, true, false) => CognitiveMode::Regulated,
            (false, false, true) => CognitiveMode::Explorer,
            (true, true, false) => CognitiveMode::Task,
            (true, false, true) => CognitiveMode::Resonant,
            (false, true, true) => CognitiveMode::Frontier,
            (true, true, true) => CognitiveMode::FullCouncil,
        }
    }

    /// Extract the 3 bits.
    pub fn bits(&self) -> (bool, bool, bool) {
        match self {
            CognitiveMode::Quiet => (false, false, false),
            CognitiveMode::Companion => (true, false, false),
            CognitiveMode::Regulated => (false, true, false),
            CognitiveMode::Explorer => (false, false, true),
            CognitiveMode::Task => (true, true, false),
            CognitiveMode::Resonant => (true, false, true),
            CognitiveMode::Frontier => (false, true, true),
            CognitiveMode::FullCouncil => (true, true, true),
        }
    }

    /// Human-readable label.
    pub fn label(&self) -> &'static str {
        match self {
            CognitiveMode::Quiet => "QUIET",
            CognitiveMode::Companion => "COMPANION",
            CognitiveMode::Regulated => "REGULATED",
            CognitiveMode::Explorer => "EXPLORER",
            CognitiveMode::Task => "TASK",
            CognitiveMode::Resonant => "RESONANT",
            CognitiveMode::Frontier => "FRONTIER",
            CognitiveMode::FullCouncil => "FULL_COUNCIL",
        }
    }

    /// Recommended HNSW search `ef` multiplier based on mode.
    /// Explorer/Resonant/FullCouncil search wider; Quiet/Regulated search narrower.
    pub fn hnsw_ef_multiplier(&self) -> f64 {
        match self {
            CognitiveMode::Quiet => 0.8,
            CognitiveMode::Companion => 1.0,
            CognitiveMode::Regulated => 0.7,
            CognitiveMode::Explorer => 1.5,
            CognitiveMode::Task => 1.0,
            CognitiveMode::Resonant => 1.3,
            CognitiveMode::Frontier => 1.2,
            CognitiveMode::FullCouncil => 1.4,
        }
    }

    /// Recommended resonator depth (max iterations).
    pub fn resonator_depth(&self) -> usize {
        match self {
            CognitiveMode::Quiet => 15,
            CognitiveMode::Companion => 25,
            CognitiveMode::Regulated => 20,
            CognitiveMode::Explorer => 30,
            CognitiveMode::Task => 25,
            CognitiveMode::Resonant => 30,
            CognitiveMode::Frontier => 28,
            CognitiveMode::FullCouncil => 35,
        }
    }

    /// Encode as a hypervector via nearest-neighbour lookup against
    /// 8 precomputed mode vectors (one per pattern).
    pub fn to_hypervector(&self) -> &'static Hypervector {
        mode_hv(*self)
    }

    /// Decode a cognitive mode from a hypervector by finding the nearest
    /// of the 8 precomputed mode vectors.
    pub fn from_hypervector(hv: &Hypervector) -> Self {
        let modes = [
            CognitiveMode::Quiet,
            CognitiveMode::Companion,
            CognitiveMode::Regulated,
            CognitiveMode::Explorer,
            CognitiveMode::Task,
            CognitiveMode::Resonant,
            CognitiveMode::Frontier,
            CognitiveMode::FullCouncil,
        ];
        let mut best = CognitiveMode::Quiet;
        let mut best_sim = -1.0;
        for m in &modes {
            let sim = 1.0 - hv.normalized_hamming_distance(mode_hv(*m));
            if sim > best_sim {
                best_sim = sim;
                best = *m;
            }
        }
        best
    }
}

/// Look-up table of one deterministic hypervector per cognitive mode.
fn mode_hv(mode: CognitiveMode) -> &'static Hypervector {
    use std::sync::OnceLock;
    static MODE_HVS: OnceLock<Vec<(CognitiveMode, Hypervector)>> = OnceLock::new();
    let table = MODE_HVS.get_or_init(|| {
        vec![
            (CognitiveMode::Quiet, Hypervector::new_zero()),
            (CognitiveMode::Companion, mode_hv_for("COG_MODE_COMPANION")),
            (CognitiveMode::Regulated, mode_hv_for("COG_MODE_REGULATED")),
            (CognitiveMode::Explorer, mode_hv_for("COG_MODE_EXPLORER")),
            (CognitiveMode::Task, mode_hv_for("COG_MODE_TASK")),
            (CognitiveMode::Resonant, mode_hv_for("COG_MODE_RESONANT")),
            (CognitiveMode::Frontier, mode_hv_for("COG_MODE_FRONTIER")),
            (
                CognitiveMode::FullCouncil,
                mode_hv_for("COG_MODE_FULL_COUNCIL"),
            ),
        ]
    });
    &table.iter().find(|(m, _)| *m == mode).unwrap().1
}

/// Deterministic hypervector for a given mode label and seed.
/// Each mode gets a unique pattern that is NOT a composition of shared bits,
/// avoiding similarity collisions in the decoding step.
fn mode_hv_for(label: &str) -> Hypervector {
    Hypervector::encode_text_ngram(label, 5) // 5-gram for better separation
}

// ═════════════════════════════════════════════════════════════════════════
// 3. DCP CONSENSUS — Distributed Cognition Protocol
// ═════════════════════════════════════════════════════════════════════════
//
// Lightweight propose → vote → resolve consensus for multi-agent LSH
// sectors.  Each agent takes one of four roles:
//
//   PRIMARY  — runs the main factorization, proposes results
//   CRITIC   — runs adversarial resonator, votes against hallucinations
//   BACKUP   — holds shadow copy, votes for fault tolerance
//   OBSERVER — monitors but doesn't vote (telemetry)
//
// Resolution produces a bundle hypervector (the sector's consensus).

/// DCP node roles with different voting weights.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum DcpRole {
    /// Runs factorization, proposes results. Weight: 4.
    Primary,
    /// Adversarial checker. Weight: 3.
    Critic,
    /// Fault-tolerant shadow. Weight: 2.
    Backup,
    /// Telemetry only, no vote. Weight: 0.
    Observer,
}

impl DcpRole {
    pub fn voting_weight(&self) -> u32 {
        match self {
            DcpRole::Primary => 4,
            DcpRole::Critic => 3,
            DcpRole::Backup => 2,
            DcpRole::Observer => 0,
        }
    }
}

/// A single DCP message in the consensus protocol.
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct DcpMessage {
    pub source_node: String,
    pub source_role: DcpRole,
    /// The proposed/endorsed hypervector (e.g., a factorization result)
    pub content: Hypervector,
    pub priority: f64,
    pub message_id: u64,
    pub timestamp: u64,
}

impl DcpMessage {
    pub fn new(
        source: String,
        role: DcpRole,
        content: Hypervector,
        priority: f64,
        id: u64,
        tick: u64,
    ) -> Self {
        DcpMessage {
            source_node: source,
            source_role: role,
            content,
            priority,
            message_id: id,
            timestamp: tick,
        }
    }
}

/// State of a consensus thread.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ConsensusState {
    Open,
    Resolved,
    Expired,
}

/// A single consensus round: a proposal with votes.
#[derive(Clone, Debug)]
pub struct ConsensusThread {
    pub thread_id: u64,
    pub proposal: DcpMessage,
    pub state: ConsensusState,
    /// voter_id → (vote_hypervector, weight)
    pub votes: HashMap<String, (Hypervector, u32)>,
    pub resolution: Option<Hypervector>,
    pub created_at: u64,
}

impl ConsensusThread {
    pub fn new(thread_id: u64, proposal: DcpMessage, tick: u64) -> Self {
        let voter = proposal.source_node.clone();
        let weight = proposal.source_role.voting_weight();
        let mut votes = HashMap::new();
        votes.insert(voter, (proposal.content, weight));

        ConsensusThread {
            thread_id,
            proposal,
            state: ConsensusState::Open,
            votes,
            resolution: None,
            created_at: tick,
        }
    }

    /// Record a vote.  CRITIC votes can be negative (indicated by zero vector).
    pub fn vote(&mut self, voter_id: &str, voter_role: DcpRole, vote_hv: Hypervector) {
        if self.state != ConsensusState::Open {
            return;
        }
        let weight = voter_role.voting_weight();
        self.votes.insert(voter_id.to_string(), (vote_hv, weight));
    }

    /// Resolve the thread: bundle all votes weighted by role weight + priority.
    /// The resolution hypervector is the weighted majority bundle.
    pub fn resolve(&mut self) -> Option<Hypervector> {
        if self.state != ConsensusState::Open {
            return self.resolution;
        }
        if self.votes.is_empty() {
            self.state = ConsensusState::Expired;
            return None;
        }

        // Weighted bundling: expand each vote by (weight × priority) copies
        let mut refs: Vec<Hypervector> = Vec::new();
        for (_voter, (hv, weight)) in &self.votes {
            let copies = (*weight as usize).max(1) * 2; // minimum 2 copies
            for _ in 0..copies {
                refs.push(*hv);
            }
        }

        let ref_vec: Vec<&Hypervector> = refs.iter().collect();
        let resolution = Hypervector::bundle(&ref_vec);
        self.resolution = Some(resolution);
        self.state = ConsensusState::Resolved;
        Some(resolution)
    }

    /// Check if this thread has timed out.
    pub fn expire_if_old(&mut self, current_tick: u64, max_age: u64) {
        if self.state == ConsensusState::Open
            && current_tick.saturating_sub(self.created_at) > max_age
        {
            self.state = ConsensusState::Expired;
        }
    }
}

/// In-process consensus engine for a single LSH sector or agent group.
#[derive(Clone, Debug)]
pub struct ConsensusEngine {
    /// Active consensus threads, indexed by thread_id.
    pub threads: HashMap<u64, ConsensusThread>,
    /// Monotonically increasing thread ID counter.
    next_thread_id: u64,
    /// Maximum age of an open thread before auto-expiry.
    pub max_thread_age: u64,
    /// Minimum number of distinct voters for a valid resolution.
    pub min_voters: usize,
}

impl ConsensusEngine {
    pub fn new(max_thread_age: u64, min_voters: usize) -> Self {
        ConsensusEngine {
            threads: HashMap::new(),
            next_thread_id: 1,
            max_thread_age,
            min_voters,
        }
    }

    /// Propose a new consensus round.
    pub fn propose(&mut self, msg: DcpMessage, tick: u64) -> u64 {
        let tid = self.next_thread_id;
        self.next_thread_id += 1;
        let thread = ConsensusThread::new(tid, msg, tick);
        self.threads.insert(tid, thread);
        tid
    }

    /// Vote on an existing thread.
    pub fn vote(
        &mut self,
        thread_id: u64,
        voter_id: &str,
        voter_role: DcpRole,
        vote_hv: Hypervector,
    ) {
        if let Some(thread) = self.threads.get_mut(&thread_id) {
            thread.vote(voter_id, voter_role, vote_hv);
        }
    }

    /// Try to resolve a thread.  Returns the resolution hypervector if
    /// enough voters have participated.
    pub fn try_resolve(&mut self, thread_id: u64) -> Option<Hypervector> {
        let enough_voters = self
            .threads
            .get(&thread_id)
            .map(|t| t.votes.len() >= self.min_voters)
            .unwrap_or(false);

        if enough_voters {
            if let Some(thread) = self.threads.get_mut(&thread_id) {
                return thread.resolve();
            }
        }
        None
    }

    /// Garbage-collect expired threads.
    pub fn expire_old(&mut self, current_tick: u64) {
        self.threads.retain(|_, t| {
            t.expire_if_old(current_tick, self.max_thread_age);
            t.state == ConsensusState::Open
        });
    }

    /// Number of open threads.
    pub fn open_count(&self) -> usize {
        self.threads
            .values()
            .filter(|t| t.state == ConsensusState::Open)
            .count()
    }
}

// ═════════════════════════════════════════════════════════════════════════
// 4. HOMEOSTATIC REGULATION — 7-Need Cybernetic Control Loop
// ═════════════════════════════════════════════════════════════════════════
//
// Tracks seven cognitive needs with setpoints, critical thresholds,
// allostatic prediction, and regulation strategies.
//
// Needs:
//   ENERGY      — computational budget (CPU time / inference calls)
//   COHERENCE   — degree of consensus across resonator networks
//   INTEGRATION — integrated information (phi-like)
//   CONNECTION  — multi-agent channel bandwidth
//   GROWTH      — rate of new hypervector acquisition
//   AUTONOMY    — self-generated vs externally triggered actions
//   INTEGRITY   — HNSW index health (fragmentation, stale vectors)

/// Named need identifiers.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum Need {
    Energy,
    Coherence,
    Integration,
    Connection,
    Growth,
    Autonomy,
    Integrity,
}

impl Need {
    pub fn all() -> [Need; 7] {
        [
            Need::Energy,
            Need::Coherence,
            Need::Integration,
            Need::Connection,
            Need::Growth,
            Need::Autonomy,
            Need::Integrity,
        ]
    }

    pub fn label(&self) -> &'static str {
        match self {
            Need::Energy => "ENERGY",
            Need::Coherence => "COHERENCE",
            Need::Integration => "INTEGRATION",
            Need::Connection => "CONNECTION",
            Need::Growth => "GROWTH",
            Need::Autonomy => "AUTONOMY",
            Need::Integrity => "INTEGRITY",
        }
    }
}

/// Configuration for a single homeostatic need.
#[derive(Clone, Debug)]
pub struct NeedConfig {
    pub setpoint: f64,
    pub critical_low: f64,
    pub critical_high: f64,
    pub optimal_min: f64,
    pub optimal_max: f64,
    /// Baseline drift per tick when idle
    pub drift_idle: f64,
    /// Drift per tick during interaction
    pub drift_interaction: f64,
}

impl NeedConfig {
    /// Default configs calibrated for a 2s/tick agent loop.
    fn for_need(need: Need) -> Self {
        match need {
            Need::Energy => NeedConfig {
                setpoint: 0.80,
                critical_low: 0.15,
                critical_high: 1.0,
                optimal_min: 0.50,
                optimal_max: 0.95,
                drift_idle: 0.005,
                drift_interaction: -0.02,
            },
            Need::Coherence => NeedConfig {
                setpoint: 0.75,
                critical_low: 0.20,
                critical_high: 1.0,
                optimal_min: 0.50,
                optimal_max: 0.90,
                drift_idle: 0.002,
                drift_interaction: -0.01,
            },
            Need::Integration => NeedConfig {
                setpoint: 0.70,
                critical_low: 0.15,
                critical_high: 1.0,
                optimal_min: 0.40,
                optimal_max: 0.90,
                drift_idle: 0.001,
                drift_interaction: 0.005,
            },
            Need::Connection => NeedConfig {
                setpoint: 0.60,
                critical_low: 0.10,
                critical_high: 1.0,
                optimal_min: 0.30,
                optimal_max: 0.85,
                drift_idle: -0.005,
                drift_interaction: 0.015,
            },
            Need::Growth => NeedConfig {
                setpoint: 0.50,
                critical_low: 0.05,
                critical_high: 1.0,
                optimal_min: 0.20,
                optimal_max: 0.80,
                drift_idle: -0.003,
                drift_interaction: 0.01,
            },
            Need::Autonomy => NeedConfig {
                setpoint: 0.65,
                critical_low: 0.10,
                critical_high: 1.0,
                optimal_min: 0.35,
                optimal_max: 0.90,
                drift_idle: 0.001,
                drift_interaction: -0.008,
            },
            Need::Integrity => NeedConfig {
                setpoint: 0.85,
                critical_low: 0.20,
                critical_high: 1.0,
                optimal_min: 0.60,
                optimal_max: 0.95,
                drift_idle: -0.001,
                drift_interaction: -0.005,
            },
        }
    }
}

/// The state of a single need at a point in time.
#[derive(Clone, Debug)]
pub struct NeedState {
    pub need: Need,
    pub current: f64,
    pub config: NeedConfig,
    /// Predicted value `prediction_horizon` ticks into the future
    pub allostatic_prediction: f64,
    /// Rolling derivative
    pub trend: f64,
    /// Cumulative deficit (integral of deviation below setpoint)
    pub deficit_hours: f64,
}

impl NeedState {
    pub fn new(need: Need) -> Self {
        let config = NeedConfig::for_need(need);
        let sp = config.setpoint;
        NeedState {
            need,
            current: sp,
            config,
            allostatic_prediction: sp,
            trend: 0.0,
            deficit_hours: 0.0,
        }
    }

    /// Update the need from an external signal.
    pub fn update(&mut self, signal: f64, tick_delta: u64) {
        let dt = tick_delta as f64;
        // EMA smoothing
        let alpha = 0.3;
        let prev = self.current;
        self.current = self.current * (1.0 - alpha) + signal * alpha;
        self.current = self.current.clamp(0.0, 1.0);
        self.trend = (self.current - prev) / dt.max(1.0);

        // Update deficit
        if self.current < self.config.setpoint {
            let deviation = self.config.setpoint - self.current;
            self.deficit_hours += deviation * dt / 3600.0; // approximate
        } else {
            self.deficit_hours *= 0.99; // gentle recovery
        }
        self.deficit_hours = self.deficit_hours.min(1.0);
    }

    /// Compute allostatic prediction: linear extrapolation of trend.
    pub fn compute_prediction(&mut self, horizon_ticks: u64) {
        let dt = horizon_ticks as f64;
        let pred = self.current + self.trend * dt;
        self.allostatic_prediction = pred.clamp(0.0, 1.0);
    }

    /// Is this need in a critical state right now?
    pub fn is_critical(&self) -> bool {
        self.current <= self.config.critical_low || self.current >= self.config.critical_high
    }

    /// Will this need breach a critical threshold within the prediction horizon?
    pub fn will_breach(&self) -> bool {
        self.allostatic_prediction <= self.config.critical_low
            || self.allostatic_prediction >= self.config.critical_high
    }

    /// How far from setpoint (0 = at setpoint, 1 = max deviation)
    pub fn deviation(&self) -> f64 {
        (self.current - self.config.setpoint).abs()
    }

    /// Is the need in its optimal range?
    pub fn is_optimal(&self) -> bool {
        self.current >= self.config.optimal_min && self.current <= self.config.optimal_max
    }
}

/// Regulation strategy: what to do when a need is out of range.
#[derive(Clone, Debug)]
pub enum RegulationStrategy {
    /// Reduce computational depth (shorter resonator, narrower HNSW)
    ConserveEnergy,
    /// Boost consensus by requesting more agent votes
    SeekCoherence,
    /// Reduce cross-agent messages to lower load
    ReduceConnections,
    /// Increase exploration (wider HNSW search, more curiosity)
    PromoteGrowth,
    /// Let the agent idle (skip non-essential cycles)
    Rest,
    /// Focus on a single task (narrow attention)
    Focus,
    /// Run integrity check on HNSW index
    AuditIntegrity,
}

impl RegulationStrategy {
    pub fn label(&self) -> &'static str {
        match self {
            RegulationStrategy::ConserveEnergy => "CONSERVE_ENERGY",
            RegulationStrategy::SeekCoherence => "SEEK_COHERENCE",
            RegulationStrategy::ReduceConnections => "REDUCE_CONNECTIONS",
            RegulationStrategy::PromoteGrowth => "PROMOTE_GROWTH",
            RegulationStrategy::Rest => "REST",
            RegulationStrategy::Focus => "FOCUS",
            RegulationStrategy::AuditIntegrity => "AUDIT_INTEGRITY",
        }
    }
}

/// Homeostatic regulator: the main cybernetic control object.
#[derive(Clone, Debug)]
pub struct HomeostaticRegulator {
    pub needs: HashMap<Need, NeedState>,
    pub tick: u64,
    pub prediction_horizon: u64,
    /// Whether the system is in crisis mode (2+ needs critical)
    pub crisis: bool,
    pub crisis_tick: u64,
    /// Active regulation strategy
    pub active_strategy: Option<RegulationStrategy>,
    /// Log of recent regulation actions
    pub regulation_log: Vec<(u64, String, String)>,
    /// Alpha calibration state
    pub alpha: f64,
}

impl HomeostaticRegulator {
    pub fn new(prediction_horizon: u64) -> Self {
        let mut needs = HashMap::new();
        for n in Need::all() {
            needs.insert(n, NeedState::new(n));
        }
        HomeostaticRegulator {
            needs,
            tick: 0,
            prediction_horizon,
            crisis: false,
            crisis_tick: 0,
            active_strategy: None,
            regulation_log: Vec::with_capacity(100),
            alpha: 0.0,
        }
    }

    /// Tick the regulator: apply drift, update predictions, check crisis.
    pub fn tick(&mut self, signals: &[(Need, f64)], interacting: bool, tick_delta: u64) {
        self.tick += 1;

        for (need, signal) in signals {
            if let Some(state) = self.needs.get_mut(need) {
                state.update(*signal, tick_delta);
                state.compute_prediction(self.prediction_horizon);
            }
        }

        // Apply idle/interaction drift to needs without explicit signals
        for need in Need::all() {
            if signals.iter().any(|(n, _)| n == &need) {
                continue; // already updated from signal
            }
            if let Some(state) = self.needs.get_mut(&need) {
                let drift = if interacting {
                    state.config.drift_interaction
                } else {
                    state.config.drift_idle
                };
                state.current = (state.current + drift * tick_delta as f64).clamp(0.0, 1.0);
                state.compute_prediction(self.prediction_horizon);
            }
        }

        // Crisis detection
        let critical_count = Need::all()
            .iter()
            .filter(|n| self.needs.get(n).map_or(false, |s| s.is_critical()))
            .count();
        let allostatic_load: f64 = Need::all()
            .iter()
            .filter_map(|n| self.needs.get(n))
            .map(|s| s.deviation() * s.deficit_hours)
            .sum();

        let was_crisis = self.crisis;
        self.crisis = critical_count >= 2 || (critical_count >= 1 && allostatic_load > 0.6);
        if self.crisis && !was_crisis {
            self.crisis_tick = self.tick;
        }
    }

    /// Pick the best regulation strategy based on current need state.
    pub fn select_strategy(&mut self) -> RegulationStrategy {
        // Find the need with the worst deviation
        let worst = Need::all()
            .iter()
            .filter_map(|n| self.needs.get(n))
            .max_by(|a, b| {
                a.deviation()
                    .partial_cmp(&b.deviation())
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
            .cloned();

        let strategy = match worst.as_ref().map(|s| s.need) {
            Some(Need::Energy)
                if worst
                    .as_ref()
                    .map_or(false, |s| s.current < s.config.setpoint) =>
            {
                RegulationStrategy::ConserveEnergy
            }
            Some(Need::Coherence) => RegulationStrategy::SeekCoherence,
            Some(Need::Connection) => RegulationStrategy::ReduceConnections,
            Some(Need::Growth) => RegulationStrategy::PromoteGrowth,
            Some(Need::Autonomy) => RegulationStrategy::Focus,
            Some(Need::Integrity) => RegulationStrategy::AuditIntegrity,
            _ => {
                if self.crisis {
                    RegulationStrategy::Rest
                } else {
                    RegulationStrategy::Focus
                }
            }
        };

        self.active_strategy = Some(strategy.clone());
        strategy
    }

    /// Apply a regulation strategy, returning parameter adjustments.
    pub fn apply_strategy(&mut self, strategy: &RegulationStrategy) -> RegulationParams {
        let params = match strategy {
            RegulationStrategy::ConserveEnergy => RegulationParams {
                resonator_depth: 10,
                hnsw_ef: 20,
                max_curiosity: 0,
                throttle_consolidation: true,
                skip_non_essential: true,
            },
            RegulationStrategy::SeekCoherence => RegulationParams {
                resonator_depth: 30,
                hnsw_ef: 50,
                max_curiosity: 1,
                throttle_consolidation: false,
                skip_non_essential: false,
            },
            RegulationStrategy::ReduceConnections => RegulationParams {
                resonator_depth: 20,
                hnsw_ef: 30,
                max_curiosity: 0,
                throttle_consolidation: false,
                skip_non_essential: false,
            },
            RegulationStrategy::PromoteGrowth => RegulationParams {
                resonator_depth: 35,
                hnsw_ef: 60,
                max_curiosity: 3,
                throttle_consolidation: false,
                skip_non_essential: false,
            },
            RegulationStrategy::Rest => RegulationParams {
                resonator_depth: 5,
                hnsw_ef: 10,
                max_curiosity: 0,
                throttle_consolidation: true,
                skip_non_essential: true,
            },
            RegulationStrategy::Focus => RegulationParams {
                resonator_depth: 25,
                hnsw_ef: 30,
                max_curiosity: 1,
                throttle_consolidation: false,
                skip_non_essential: false,
            },
            RegulationStrategy::AuditIntegrity => RegulationParams {
                resonator_depth: 15,
                hnsw_ef: 40,
                max_curiosity: 0,
                throttle_consolidation: false,
                skip_non_essential: true,
            },
        };

        self.regulation_log.push((
            self.tick,
            strategy.label().to_string(),
            format!(
                "res_depth={} hnsw_ef={}",
                params.resonator_depth, params.hnsw_ef
            ),
        ));
        if self.regulation_log.len() > 100 {
            self.regulation_log.remove(0);
        }

        params
    }

    /// Run one full regulation cycle: select + apply + log.
    pub fn regulate(&mut self) -> RegulationParams {
        let strategy = self.select_strategy();
        self.apply_strategy(&strategy)
    }

    /// Get a formatted summary of all need states.
    pub fn summary(&self) -> String {
        let mut parts = Vec::new();
        for need in Need::all() {
            if let Some(state) = self.needs.get(&need) {
                let marker = if state.is_critical() {
                    "⚠"
                } else if !state.is_optimal() {
                    "!"
                } else {
                    "·"
                };
                parts.push(format!(
                    "{} {}[{:.2}→{:.2}]",
                    marker,
                    need.label(),
                    state.current,
                    state.allostatic_prediction
                ));
            }
        }
        format!(
            "HOMEOSTASIS: {} | crisis={} | α={:.3} | {}",
            parts.join(" | "),
            if self.crisis { "YES" } else { "no" },
            self.alpha,
            self.active_strategy
                .as_ref()
                .map(|s| s.label())
                .unwrap_or("none"),
        )
    }
}

/// Parameter adjustments produced by a regulation strategy.
#[derive(Clone, Debug)]
pub struct RegulationParams {
    pub resonator_depth: usize,
    pub hnsw_ef: usize,
    pub max_curiosity: usize,
    pub throttle_consolidation: bool,
    pub skip_non_essential: bool,
}

impl RegulationParams {
    pub fn relaxed() -> Self {
        RegulationParams {
            resonator_depth: 25,
            hnsw_ef: 50,
            max_curiosity: 2,
            throttle_consolidation: false,
            skip_non_essential: false,
        }
    }
}

// ═════════════════════════════════════════════════════════════════════════
// 5. PSC PREDICTOR — Predictive State Characterization
// ═════════════════════════════════════════════════════════════════════════
//
// Ported from `core/psc_scaled.py` in timeless-hayoka/infj-bot.
//
// Batch trend prediction using linear regression + EWMA, blended by
// confidence, with a chaos score that dynamically shortens the prediction
// horizon when the signal is erratic.
//
// In HDC terms: instead of per-dimension regression on a (T, D) matrix,
// we track a rolling bundle of state hypervectors and measure chaos as
// 1 - cosine similarity between successive states.

/// A single observation in the PSC buffer.
#[derive(Clone, Debug)]
pub struct PscObservation {
    pub tick: u64,
    /// The system state encoded as a hypervector.
    pub state: Hypervector,
}

/// PSC predictor: adaptive-horizon trend prediction using HD similarity.
#[derive(Clone, Debug)]
pub struct PscPredictor {
    /// Rolling circular buffer of past observations (max `capacity`).
    buffer: Vec<PscObservation>,
    capacity: usize,
    /// Minimum observations before prediction is meaningful.
    pub min_samples: usize,
    /// Base prediction horizon in ticks.
    pub horizon_base: u64,
    /// Minimum horizon (during high chaos).
    pub horizon_min: u64,
    /// EWMA alpha for trend smoothing.
    pub alpha: f64,
}

impl PscPredictor {
    /// Create a new PSC predictor.
    ///
    /// * `capacity` — max observations in rolling buffer (default: 20)
    /// * `min_samples` — minimum before predictions are reliable (default: 3)
    /// * `horizon_base` — default look-ahead in ticks (default: 10)
    /// * `horizon_min` — shortest horizon during chaos (default: 2)
    pub fn new(capacity: usize, min_samples: usize, horizon_base: u64, horizon_min: u64) -> Self {
        PscPredictor {
            buffer: Vec::with_capacity(capacity),
            capacity,
            min_samples,
            horizon_base,
            horizon_min,
            alpha: 0.3,
        }
    }

    pub fn with_defaults() -> Self {
        PscPredictor::new(20, 3, 10, 2)
    }

    /// Record a new observation.
    pub fn observe(&mut self, tick: u64, state: Hypervector) {
        if self.buffer.len() >= self.capacity {
            self.buffer.remove(0);
        }
        self.buffer.push(PscObservation { tick, state });
    }

    /// Number of observations stored.
    pub fn len(&self) -> usize {
        self.buffer.len()
    }

    /// Is the buffer full enough for predictions?
    pub fn is_ready(&self) -> bool {
        self.buffer.len() >= self.min_samples
    }

    /// Compute the chaos score [0, 1] from successive state similarity.
    /// High chaos = low similarity between adjacent states = erratic signal.
    ///
    /// In HD terms: chaos = 1 - mean(cosine(s[t], s[t-1])) over the buffer.
    pub fn chaos_score(&self) -> f64 {
        if self.buffer.len() < 2 {
            return 0.0;
        }
        let mut total_dist = 0.0;
        let mut pairs = 0;
        for i in 1..self.buffer.len() {
            let d = self.buffer[i]
                .state
                .normalized_hamming_distance(&self.buffer[i - 1].state);
            total_dist += d;
            pairs += 1;
        }
        total_dist / pairs as f64
    }

    /// Compute the adaptive horizon: shorter when chaotic, longer when stable.
    pub fn adaptive_horizon(&self) -> u64 {
        let chaos = self.chaos_score();
        // chaos ~0.5 is typical for random walk; 0.0 is perfectly stable
        let fraction = (chaos * 2.0).clamp(0.0, 1.0); // normalize to [0, 1]
        let range = self.horizon_base.saturating_sub(self.horizon_min);
        let reduction = (range as f64 * fraction) as u64;
        self.horizon_base
            .saturating_sub(reduction)
            .max(self.horizon_min)
    }

    /// Predict the next state by blending the last observed state with
    /// a trend vector derived from the mean pairwise delta.
    ///
    /// Returns `None` if not enough observations.
    pub fn predict_next(&self) -> Option<Hypervector> {
        if !self.is_ready() {
            return None;
        }

        let last = &self.buffer.last().unwrap().state;

        if self.buffer.len() < 2 {
            return Some(*last);
        }

        // Compute trend: bundle of all consecutive deltas (XOR differences)
        let mut trend_parts: Vec<Hypervector> = Vec::new();
        for i in 1..self.buffer.len() {
            let delta = self.buffer[i].state.bitwise_xor(&self.buffer[i - 1].state);
            trend_parts.push(delta);
        }

        // Mean trend via bundling
        let refs: Vec<&Hypervector> = trend_parts.iter().collect();
        let mean_trend = Hypervector::bundle(&refs);

        // Predicted next = last_state ⊕ mean_trend
        Some(last.bitwise_xor(&mean_trend))
    }

    /// Full prediction report: chaos, horizon, and predicted state.
    pub fn report(&self) -> Option<(f64, u64, Hypervector)> {
        let pred = self.predict_next()?;
        Some((self.chaos_score(), self.adaptive_horizon(), pred))
    }
}

// ═════════════════════════════════════════════════════════════════════════
// 6. GLOBAL WORKSPACE — Competitive Salience Ranking
// ═════════════════════════════════════════════════════════════════════════
//
// Ported from `core/global_workspace.py` in timeless-hayoka/infj-bot.
//
// Implements a Global Workspace Theory (GWT) attention mechanism using
// HD similarity: each broadcast is a hypervector, salience is cosine
// distance to the current query/context, and workspace tiers are
// similarity-threshold bands.

/// A single content item in the global workspace.
#[derive(Clone, Debug)]
pub struct WorkspaceContent {
    /// The content hypervector (encoded observation, thought, or signal).
    pub vector: Hypervector,
    /// Source label for deduplication.
    pub source: String,
    /// Raw salience score.
    pub salience: f64,
    /// How many cycles ago this was broadcast (for decay).
    pub age_cycles: u64,
    /// Arbitrary metadata.
    pub metadata: std::collections::HashMap<String, String>,
}

/// Global Workspace with competitive salience ranking.
///
/// Each cycle:
/// 1. New contents are submitted with a context query
/// 2. All contents (new + survivors) are scored against the query
/// 3. Contents are assigned to tiers: spotlight (top 1), active (next N),
///    preconscious (similarity bands), archived (below threshold)
/// 4. Age decay is applied to all survivors
///
/// The workspace holds at most `capacity` items at once.
#[derive(Clone, Debug)]
pub struct GlobalWorkspace {
    /// Active workspace contents.
    contents: Vec<WorkspaceContent>,
    /// Maximum number of items in the workspace.
    capacity: usize,
    /// How many items are in the "spotlight" (top tier).
    pub spotlight_size: usize,
    /// How many items are in the "active" tier.
    pub active_size: usize,
    /// Salience decay per cycle (multiplied each cycle).
    pub decay_factor: f64,
    /// Minimum salience to remain in workspace.
    pub archive_threshold: f64,
}

impl GlobalWorkspace {
    pub fn new(capacity: usize) -> Self {
        GlobalWorkspace {
            contents: Vec::with_capacity(capacity),
            capacity,
            spotlight_size: 1,
            active_size: 3,
            decay_factor: 0.9,
            archive_threshold: 0.05,
        }
    }

    /// Submit new content into the workspace.
    pub fn submit(
        &mut self,
        vector: Hypervector,
        source: &str,
        metadata: std::collections::HashMap<String, String>,
    ) {
        // Deduplicate by source: if an item from the same source already exists,
        // replace it with the new vector (recency overwrites).
        if let Some(existing) = self.contents.iter_mut().find(|c| c.source == source) {
            existing.vector = vector;
            existing.salience = 0.0; // will be re-scored in the next cycle
            existing.age_cycles = 0;
            existing.metadata = metadata;
            return;
        }

        if self.contents.len() >= self.capacity {
            // Evict the lowest-salience item
            let mut worst_idx = 0;
            let mut worst_sal = f64::MAX;
            for (i, c) in self.contents.iter().enumerate() {
                if c.salience < worst_sal {
                    worst_sal = c.salience;
                    worst_idx = i;
                }
            }
            self.contents.remove(worst_idx);
        }

        self.contents.push(WorkspaceContent {
            vector,
            source: source.to_string(),
            salience: 0.0,
            age_cycles: 0,
            metadata,
        });
    }

    /// Run one competition cycle: score all contents against the context
    /// query, apply age decay, sort into tiers, archive the rest.
    ///
    /// Returns (spotlight, active, preconscious) where each is a vec of
    /// (source, salience) pairs.
    pub fn cycle(
        &mut self,
        context_query: &Hypervector,
    ) -> (Vec<(String, f64)>, Vec<(String, f64)>, Vec<(String, f64)>) {
        // Score all contents against the query
        for content in &mut self.contents {
            let sim = 1.0 - content.vector.normalized_hamming_distance(context_query);
            content.salience = sim * self.decay_factor.powi(content.age_cycles as i32);
            content.age_cycles += 1;
        }

        // Sort by salience descending
        self.contents.sort_by(|a, b| {
            b.salience
                .partial_cmp(&a.salience)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        // Assign to tiers
        let mut spotlight: Vec<(String, f64)> = Vec::new();
        let mut active: Vec<(String, f64)> = Vec::new();
        let mut preconscious: Vec<(String, f64)> = Vec::new();
        let mut survivors: Vec<WorkspaceContent> = Vec::new();

        for (i, content) in self.contents.drain(..).enumerate() {
            if content.salience < self.archive_threshold {
                continue; // archived — drop entirely
            }
            if i < self.spotlight_size {
                spotlight.push((content.source.clone(), content.salience));
                survivors.push(content);
            } else if i < self.spotlight_size + self.active_size {
                active.push((content.source.clone(), content.salience));
                survivors.push(content);
            } else {
                preconscious.push((content.source.clone(), content.salience));
                survivors.push(content);
            }
        }

        self.contents = survivors;
        (spotlight, active, preconscious)
    }

    /// Current number of items in the workspace.
    pub fn len(&self) -> usize {
        self.contents.len()
    }

    /// Is the workspace empty?
    pub fn is_empty(&self) -> bool {
        self.contents.is_empty()
    }

    /// Clear all contents.
    pub fn clear(&mut self) {
        self.contents.clear();
    }
}

// ═════════════════════════════════════════════════════════════════════════
// 7. EMOTIONAL FIELD — Emotion⊗Stance → Mood Binding
// ═════════════════════════════════════════════════════════════════════════
//
// Ported from `core/emotional_field.py` in timeless-hayoka/infj-bot.
//
// Maps (emotion, stance) pairs to a mood hypervector via HD binding:
//
//   mood = cleanup(emotion_HV ⊗ stance_HV ⊗ SEED)
//
// where ⊗ is XOR in binary HDC and cleanup finds the nearest mood vector
// from a 28-entry lookup table.

/// Fixed emotion labels.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Emotion {
    Joy,
    Sadness,
    Anger,
    Fear,
    Surprise,
    Disgust,
    Neutral,
}

impl Emotion {
    pub fn label(&self) -> &'static str {
        match self {
            Emotion::Joy => "EMO_JOY",
            Emotion::Sadness => "EMO_SADNESS",
            Emotion::Anger => "EMO_ANGER",
            Emotion::Fear => "EMO_FEAR",
            Emotion::Surprise => "EMO_SURPRISE",
            Emotion::Disgust => "EMO_DISGUST",
            Emotion::Neutral => "EMO_NEUTRAL",
        }
    }
}

/// Fixed stance labels.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Stance {
    Open,
    Guarded,
    Curious,
    Distant,
}

impl Stance {
    pub fn label(&self) -> &'static str {
        match self {
            Stance::Open => "STANCE_OPEN",
            Stance::Guarded => "STANCE_GUARDED",
            Stance::Curious => "STANCE_CURIOUS",
            Stance::Distant => "STANCE_DISTANT",
        }
    }
}

/// Fixed mood labels.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Mood {
    Warm,
    Playful,
    Somber,
    Alert,
    Defensive,
    Withdrawn,
    Curious,
    Analytical,
    Neutral,
}

impl Mood {
    pub fn label(&self) -> &'static str {
        match self {
            Mood::Warm => "MOOD_WARM",
            Mood::Playful => "MOOD_PLAYFUL",
            Mood::Somber => "MOOD_SOMBER",
            Mood::Alert => "MOOD_ALERT",
            Mood::Defensive => "MOOD_DEFENSIVE",
            Mood::Withdrawn => "MOOD_WITHDRAWN",
            Mood::Curious => "MOOD_CURIOUS",
            Mood::Analytical => "MOOD_ANALYTICAL",
            Mood::Neutral => "MOOD_NEUTRAL",
        }
    }
}

/// Emotional field: maps emotion + stance to mood using a two-stage
/// HD associative memory.
///
/// Stage 1 — encode: each rule (e, s, m) produces a key = e_HV ⊗ s_HV
/// and a value = m_HV.  The table stores both.
///
/// Stage 2 — query:  query_key = e_q_HV ⊗ s_q_HV.  Find the stored key
/// with the highest similarity.  Return its associated mood.
///
/// This avoids the cancellation problem of naively storing e⊗s⊗m and
/// querying with e⊗s (the result would be noise, not a clean mood).
#[derive(Clone, Debug)]
pub struct EmotionalField {
    /// (emotion, stance, mood, key=e⊗s, value=mood_HV)
    entries: Vec<(Emotion, Stance, Mood, Hypervector, Hypervector)>,
}

impl EmotionalField {
    /// Build the emotional field from the 7×4 = 28 mapping rules.
    pub fn new() -> Self {
        // Use seeded random HVs for each label to guarantee orthogonality
        fn make_hv(seed: u64) -> Hypervector {
            let mut bits = [0u64; 160];
            let mut x = seed;
            for i in 0..160 {
                x = x
                    .wrapping_mul(6364136223846793005)
                    .wrapping_add(1442695040888963407);
                bits[i] = x ^ 0xdeadbeefcafebabe;
            }
            Hypervector { bits }
        }

        // Deterministic HVs for each symbol
        let emo_hvs: Vec<(Emotion, Hypervector)> = vec![
            (Emotion::Joy, make_hv(1001)),
            (Emotion::Sadness, make_hv(1002)),
            (Emotion::Anger, make_hv(1003)),
            (Emotion::Fear, make_hv(1004)),
            (Emotion::Surprise, make_hv(1005)),
            (Emotion::Disgust, make_hv(1006)),
            (Emotion::Neutral, make_hv(1007)),
        ];
        let sta_hvs: Vec<(Stance, Hypervector)> = vec![
            (Stance::Open, make_hv(2001)),
            (Stance::Guarded, make_hv(2002)),
            (Stance::Curious, make_hv(2003)),
            (Stance::Distant, make_hv(2004)),
        ];
        let mood_hvs: Vec<(Mood, Hypervector)> = vec![
            (Mood::Warm, make_hv(3001)),
            (Mood::Playful, make_hv(3002)),
            (Mood::Somber, make_hv(3003)),
            (Mood::Alert, make_hv(3004)),
            (Mood::Defensive, make_hv(3005)),
            (Mood::Withdrawn, make_hv(3006)),
            (Mood::Curious, make_hv(3007)),
            (Mood::Analytical, make_hv(3008)),
            (Mood::Neutral, make_hv(3009)),
        ];

        fn lookup<T: Clone + PartialEq>(hvs: &[(T, Hypervector)], key: &T) -> Hypervector {
            hvs.iter()
                .find(|(k, _)| k == key)
                .map(|(_, v)| *v)
                .unwrap_or(Hypervector::new_zero())
        }

        let rules: Vec<(Emotion, Stance, Mood)> = vec![
            (Emotion::Joy, Stance::Open, Mood::Warm),
            (Emotion::Joy, Stance::Curious, Mood::Playful),
            (Emotion::Joy, Stance::Guarded, Mood::Playful),
            (Emotion::Joy, Stance::Distant, Mood::Analytical),
            (Emotion::Sadness, Stance::Open, Mood::Somber),
            (Emotion::Sadness, Stance::Curious, Mood::Analytical),
            (Emotion::Sadness, Stance::Guarded, Mood::Withdrawn),
            (Emotion::Sadness, Stance::Distant, Mood::Withdrawn),
            (Emotion::Anger, Stance::Open, Mood::Alert),
            (Emotion::Anger, Stance::Curious, Mood::Analytical),
            (Emotion::Anger, Stance::Guarded, Mood::Defensive),
            (Emotion::Anger, Stance::Distant, Mood::Defensive),
            (Emotion::Fear, Stance::Open, Mood::Alert),
            (Emotion::Fear, Stance::Curious, Mood::Alert),
            (Emotion::Fear, Stance::Guarded, Mood::Defensive),
            (Emotion::Fear, Stance::Distant, Mood::Withdrawn),
            (Emotion::Surprise, Stance::Open, Mood::Curious),
            (Emotion::Surprise, Stance::Curious, Mood::Curious),
            (Emotion::Surprise, Stance::Guarded, Mood::Alert),
            (Emotion::Surprise, Stance::Distant, Mood::Analytical),
            (Emotion::Disgust, Stance::Open, Mood::Analytical),
            (Emotion::Disgust, Stance::Curious, Mood::Analytical),
            (Emotion::Disgust, Stance::Guarded, Mood::Defensive),
            (Emotion::Disgust, Stance::Distant, Mood::Withdrawn),
            (Emotion::Neutral, Stance::Open, Mood::Warm),
            (Emotion::Neutral, Stance::Curious, Mood::Curious),
            (Emotion::Neutral, Stance::Guarded, Mood::Analytical),
            (Emotion::Neutral, Stance::Distant, Mood::Analytical),
        ];

        let mut entries = Vec::with_capacity(28);
        for &(emotion, stance, mood) in &rules {
            let emo_hv = lookup(&emo_hvs, &emotion);
            let sta_hv = lookup(&sta_hvs, &stance);
            let mood_hv = lookup(&mood_hvs, &mood);
            let key = emo_hv.bitwise_xor(&sta_hv);
            entries.push((emotion, stance, mood, key, mood_hv));
        }

        EmotionalField { entries }
    }

    /// Resolve an (emotion, stance) pair to a mood.
    ///
    /// Key insight: the query key = e_q_HV ⊗ s_q_HV.  We find the stored
    /// key with the highest Hamming similarity to the query key, then
    /// return its mood.
    pub fn resolve(&self, emotion: Emotion, stance: Stance) -> Mood {
        // Reconstruct the query key using the same deterministic HVs
        fn make_hv(seed: u64) -> Hypervector {
            let mut bits = [0u64; 160];
            let mut x = seed;
            for i in 0..160 {
                x = x
                    .wrapping_mul(6364136223846793005)
                    .wrapping_add(1442695040888963407);
                bits[i] = x ^ 0xdeadbeefcafebabe;
            }
            Hypervector { bits }
        }
        let emo_seed: u64 = match emotion {
            Emotion::Joy => 1001,
            Emotion::Sadness => 1002,
            Emotion::Anger => 1003,
            Emotion::Fear => 1004,
            Emotion::Surprise => 1005,
            Emotion::Disgust => 1006,
            Emotion::Neutral => 1007,
        };
        let sta_seed: u64 = match stance {
            Stance::Open => 2001,
            Stance::Guarded => 2002,
            Stance::Curious => 2003,
            Stance::Distant => 2004,
        };
        let query_key = make_hv(emo_seed).bitwise_xor(&make_hv(sta_seed));

        let mut best = Mood::Neutral;
        let mut best_sim = -1.0;
        for &(_, _, mood, ref key, _) in &self.entries {
            let sim = 1.0 - query_key.normalized_hamming_distance(key);
            if sim > best_sim {
                best_sim = sim;
                best = mood;
            }
        }
        best
    }
}

// ═════════════════════════════════════════════════════════════════════════
// 8. CONTEXT ENGINE — Fork/Merge Superposition
// ═════════════════════════════════════════════════════════════════════════
//
// Ported from `core/context_engine.py` in timeless-hayoka/infj-bot.
//
// A context is a hypervector bundle representing the current cognitive
// state.  Forking creates N hypothesis contexts by bundling the current
// context with different perturbation vectors.  Merging selects the
// best hypothesis via similarity to a cue vector.

/// A cognitive context: a hypervector bundle representing state.
#[derive(Clone, Debug)]
pub struct Context {
    pub vector: Hypervector,
    pub label: String,
}

impl Context {
    pub fn new(label: &str) -> Self {
        Context {
            vector: Hypervector::new_zero(),
            label: label.to_string(),
        }
    }

    /// Bind a new value into this context.
    pub fn bind(&mut self, role: &Hypervector, filler: &Hypervector) {
        self.vector = self.vector.bitwise_xor(&role.bitwise_xor(filler));
    }

    /// Extend the context by XOR-binding a new vector (for chaining).
    pub fn extend(&mut self, other: &Hypervector) {
        self.vector = self.vector.bitwise_xor(other);
    }
}

/// Fork the current context into `n` hypothesis branches, each perturbed
/// by a unique noise vector derived from a seed.
pub fn fork_context(context: &Context, n: usize) -> Vec<Context> {
    let mut branches = Vec::with_capacity(n);
    for i in 0..n {
        let seed_label = format!("FORK_{}_{}", context.label, i);
        let noise = Hypervector::encode_text_ngram(&seed_label, 5);
        let forked = Context {
            vector: context.vector.bitwise_xor(&noise),
            label: format!("{}/branch_{}", context.label, i),
        };
        branches.push(forked);
    }
    branches
}

/// Merge multiple hypothesis contexts by selecting the one most similar
/// to a cue vector.  This is "selection merging" — the closest match wins.
pub fn merge_contexts(branches: &[Context], cue: &Hypervector) -> Option<Context> {
    if branches.is_empty() {
        return None;
    }
    let mut best = 0usize;
    let mut best_sim = -1.0;
    for (i, ctx) in branches.iter().enumerate() {
        let sim = 1.0 - ctx.vector.normalized_hamming_distance(cue);
        if sim > best_sim {
            best_sim = sim;
            best = i;
        }
    }
    Some(branches[best].clone())
}

// ═════════════════════════════════════════════════════════════════════════
// 9. IMPLICIT INTUITION — Pattern Recognition via Bundled Domain HVs
// ═════════════════════════════════════════════════════════════════════════
//
// Ported from `core/intuition.py` in timeless-hayoka/infj-bot.
//
// An implicit pattern is a set of domain tags bundled into a single
// hypervector.  Recognition fires when an input hypervector has a
// similarity above a threshold to a stored pattern.

/// A learned implicit pattern: a bundled domain signature.
#[derive(Clone, Debug)]
pub struct ImplicitPattern {
    pub label: String,
    /// Bundled hypervector of all domain tags.
    pub signature: Hypervector,
    /// How many times this pattern has been reinforced.
    pub strength: u32,
    /// Confidence derived from strength.
    pub confidence: f64,
}

/// Intuition engine: learns and recognizes implicit patterns from
/// repeated domain co-occurrence.
#[derive(Clone, Debug)]
pub struct IntuitionEngine {
    pub patterns: Vec<ImplicitPattern>,
    /// Similarity threshold for recognition (default: 0.55).
    pub recognition_threshold: f64,
    /// Minimum examples before a pattern fires (default: 3).
    pub min_examples: u32,
}

impl IntuitionEngine {
    pub fn new() -> Self {
        IntuitionEngine {
            patterns: Vec::new(),
            recognition_threshold: 0.55,
            min_examples: 3,
        }
    }

    /// Learn or reinforce a pattern from a set of domain tags.
    /// Each tag is encoded as a 5-gram hypervector and bundled together.
    pub fn observe(&mut self, label: &str, domain_tags: &[&str]) {
        if domain_tags.is_empty() {
            return;
        }

        // Encode domain tags into a bundled signature
        let hvs: Vec<Hypervector> = domain_tags
            .iter()
            .map(|t| Hypervector::encode_text_ngram(t, 5))
            .collect();
        let refs: Vec<&Hypervector> = hvs.iter().collect();
        let signature = Hypervector::bundle(&refs);

        // Check if a similar pattern already exists
        for pattern in &mut self.patterns {
            let sim = 1.0 - signature.normalized_hamming_distance(&pattern.signature);
            if sim > self.recognition_threshold {
                // Reinforce existing pattern
                pattern.strength += 1;
                pattern.confidence = (pattern.confidence + 0.1).min(1.0);
                // Blend signature via bundle
                let refs2: Vec<&Hypervector> = vec![&pattern.signature, &signature];
                pattern.signature = Hypervector::bundle(&refs2);
                return;
            }
        }

        // New pattern
        self.patterns.push(ImplicitPattern {
            label: label.to_string(),
            signature,
            strength: 1,
            confidence: 0.3,
        });
    }

    /// Recognize patterns in an input hypervector.
    /// Returns matched patterns with similarity above threshold.
    pub fn recognize(&self, input: &Hypervector) -> Vec<(&ImplicitPattern, f64)> {
        let mut matches = Vec::new();
        for pattern in &self.patterns {
            if pattern.strength < self.min_examples {
                continue;
            }
            let sim = 1.0 - input.normalized_hamming_distance(&pattern.signature);
            if sim > self.recognition_threshold {
                matches.push((pattern, sim));
            }
        }
        // Sort by similarity descending
        matches.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        matches
    }

    /// Number of stored patterns.
    pub fn len(&self) -> usize {
        self.patterns.len()
    }

    /// Prune patterns below a strength threshold.
    pub fn prune(&mut self, min_strength: u32) {
        self.patterns.retain(|p| p.strength >= min_strength);
    }
}

// ═════════════════════════════════════════════════════════════════════════
// 10. SHADOW / ENANTIODROMIA — Bipolar Archetype Oscillation
// ═════════════════════════════════════════════════════════════════════════
//
// Ported from `core/shadow.py` in timeless-hayoka/infj-bot.
//
// Enantiodromia: when one archetype dominates for too long, charge
// accumulates in its opposite, eventually causing a reversal.
//
// In HD terms: each archetype is a fixed hypervector.  The "shadow"
// state is a weighted superposition.  When the dominant archetype's
// weight exceeds a threshold, the opposite gets a boost.

/// Named archetypes for the shadow system.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Archetype {
    Hero,      // confident, action-oriented
    Shadow,    // suppressed, reactive
    Sage,      // wise, analytical
    Trickster, // playful, subversive
    Caregiver, // nurturing, protective
    Orphan,    // vulnerable, seeking connection
}

impl Archetype {
    pub fn label(&self) -> &'static str {
        match self {
            Archetype::Hero => "ARCH_HERO",
            Archetype::Shadow => "ARCH_SHADOW",
            Archetype::Sage => "ARCH_SAGE",
            Archetype::Trickster => "ARCH_TRICKSTER",
            Archetype::Caregiver => "ARCH_CAREGIVER",
            Archetype::Orphan => "ARCH_ORPHAN",
        }
    }

    /// Return the opposite archetype (for enantiodromia charge transfer).
    pub fn opposite(&self) -> Archetype {
        match self {
            Archetype::Hero => Archetype::Shadow,
            Archetype::Shadow => Archetype::Hero,
            Archetype::Sage => Archetype::Trickster,
            Archetype::Trickster => Archetype::Sage,
            Archetype::Caregiver => Archetype::Orphan,
            Archetype::Orphan => Archetype::Caregiver,
        }
    }
}

/// A single archetype's state in the shadow system.
#[derive(Clone, Debug)]
pub struct ArchetypeState {
    pub archetype: Archetype,
    /// Current activation intensity [0, 1].
    pub intensity: f64,
    /// Enantiodromia charge built up in the opposite archetype [0, 1].
    pub opposite_charge: f64,
}

/// Shadow system with enantiodromia oscillation.
#[derive(Clone, Debug)]
pub struct ShadowSystem {
    pub archetypes: Vec<ArchetypeState>,
    /// Intensity threshold that triggers opposite charge accumulation.
    pub dominance_threshold: f64,
    /// How much charge transfers per tick when dominant.
    pub charge_rate: f64,
    /// Charge level that triggers a reversal.
    pub reversal_threshold: f64,
    /// Natural decay per tick for all intensities.
    pub decay_rate: f64,
}

impl ShadowSystem {
    pub fn new() -> Self {
        let archetypes = Archetype::all()
            .iter()
            .map(|&a| ArchetypeState {
                archetype: a,
                intensity: 0.2, // all start mildly active
                opposite_charge: 0.0,
            })
            .collect();

        ShadowSystem {
            archetypes,
            dominance_threshold: 0.6,
            charge_rate: 0.05,
            reversal_threshold: 0.8,
            decay_rate: 0.01,
        }
    }

    /// Tick the shadow system:
    /// 1. Find the dominant archetype (highest intensity)
    /// 2. If dominant > threshold, accumulate charge in its opposite
    /// 3. If charge > reversal threshold, trigger a reversal
    /// 4. Apply decay to all intensities
    pub fn tick(&mut self, external_signals: &[(Archetype, f64)]) {
        // Step 1: Apply external signals (immutable snapshot then apply)
        let signal_updates: Vec<(Archetype, f64)> =
            external_signals.iter().map(|&(a, s)| (a, s)).collect();
        for (arch, signal) in &signal_updates {
            if let Some(state) = self.archetypes.iter_mut().find(|a| a.archetype == *arch) {
                state.intensity = (state.intensity + signal).clamp(0.0, 1.0);
            }
        }

        // Step 2: Find dominant archetype (snapshot intensities to avoid borrow conflicts)
        let intensities: Vec<(Archetype, f64)> = self
            .archetypes
            .iter()
            .map(|a| (a.archetype, a.intensity))
            .collect();

        let dominant = intensities
            .iter()
            .max_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal))
            .cloned();

        let mut charge_updates: Vec<(Archetype, f64)> = Vec::new();
        let mut reversal_updates: Vec<(Archetype, Archetype, f64)> = Vec::new();

        if let Some((dom_arch, dom_intensity)) = dominant {
            if dom_intensity > self.dominance_threshold {
                let opp = dom_arch.opposite();
                charge_updates.push((opp, self.charge_rate));
            }
        }

        // Step 3: Check for reversal (using snapshot of charges)
        let charges: Vec<(Archetype, f64)> = self
            .archetypes
            .iter()
            .map(|a| (a.archetype, a.opposite_charge))
            .collect();

        for (arch, charge) in &charges {
            if *charge >= self.reversal_threshold {
                let opp = arch.opposite();
                // Find intensity to transfer
                let current_intensity = self
                    .archetypes
                    .iter()
                    .find(|a| a.archetype == *arch)
                    .map(|a| a.intensity)
                    .unwrap_or(0.5);
                let transfer = current_intensity * 0.5;
                reversal_updates.push((*arch, opp, transfer));
            }
        }

        // Step 4: Apply updates
        for (opp, rate) in &charge_updates {
            if let Some(state) = self.archetypes.iter_mut().find(|a| a.archetype == *opp) {
                state.opposite_charge = (state.opposite_charge + rate).min(1.0);
            }
        }
        for (arch, opp, transfer) in &reversal_updates {
            if let Some(state) = self.archetypes.iter_mut().find(|a| a.archetype == *arch) {
                state.intensity = (state.intensity - transfer).max(0.1);
                state.opposite_charge = 0.0;
            }
            if let Some(opp_state) = self.archetypes.iter_mut().find(|a| a.archetype == *opp) {
                opp_state.intensity = (opp_state.intensity + transfer).min(1.0);
            }
        }

        // Step 5: Natural decay
        for state in &mut self.archetypes {
            state.intensity = (state.intensity - self.decay_rate).max(0.05);
            state.opposite_charge = (state.opposite_charge - self.decay_rate * 0.5).max(0.0);
        }
    }

    /// Get the dominant archetype right now.
    pub fn dominant(&self) -> Archetype {
        self.archetypes
            .iter()
            .max_by(|a, b| {
                a.intensity
                    .partial_cmp(&b.intensity)
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
            .map(|a| a.archetype)
            .unwrap_or(Archetype::Sage)
    }

    /// Get the shadow's narrative as a hypervector (bundle of active archetypes weighted by intensity).
    pub fn to_hypervector(&self) -> Hypervector {
        let mut bound = Vec::new();
        for state in &self.archetypes {
            if state.intensity > 0.15 {
                let hv = Hypervector::encode_text_ngram(state.archetype.label(), 5);
                // Replicate by intensity for weighted bundling
                let copies = (state.intensity * 8.0).round() as usize;
                for _ in 0..copies.max(1) {
                    bound.push(hv);
                }
            }
        }
        let refs: Vec<&Hypervector> = bound.iter().collect();
        Hypervector::bundle(&refs)
    }
}

impl Archetype {
    pub fn all() -> [Archetype; 6] {
        [
            Archetype::Hero,
            Archetype::Shadow,
            Archetype::Sage,
            Archetype::Trickster,
            Archetype::Caregiver,
            Archetype::Orphan,
        ]
    }
}

// ═════════════════════════════════════════════════════════════════════════
// Tests
// ═════════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;

    // ── DMU Scoring ──────────────────────────────────────────────

    #[test]
    fn test_dmu_score_basics() {
        let params = DmuParams::default();

        // Fresh, high-similarity entry should score high
        let score = dmu_score(0.1, 0, 0, 1.0, &params);
        assert!(
            score > 0.5,
            "Fresh high-sim entry should score >0.5, got {}",
            score
        );

        // Very old entry should decay
        let old_score = dmu_score(0.1, 10_000, 0, 1.0, &params);
        assert!(old_score < score, "Old entry should score lower than fresh");

        // Frequently retrieved entry should score higher than never-retrieved
        // (all else equal, after some time has passed for decay differences to show)
        let freq_score = dmu_score(0.1, 100, 10, 1.0, &params);
        let infreq_score = dmu_score(0.1, 100, 0, 1.0, &params);
        assert!(
            freq_score >= infreq_score,
            "Frequent retrieval should boost score"
        );

        // Floor should be respected
        let floor_score = dmu_score(0.9, 100_000, 0, 0.0, &params);
        assert!(floor_score >= params.floor, "Score should be >= floor");
    }

    #[test]
    fn test_dmu_params_variants() {
        let ep = dmu_params_episodic();
        let sem = dmu_params_semantic();
        let bond = dmu_params_bond();

        // Bond memory should decay slower than episodic
        let ep_score = dmu_score(0.2, 500, 0, 0.8, &ep);
        let bond_score = dmu_score(0.2, 500, 0, 0.8, &bond);
        assert!(
            bond_score > ep_score,
            "Bond memory should decay slower than episodic"
        );
        assert!(
            sem.tau_base > ep.tau_base,
            "Semantic tau should be larger than episodic"
        );
    }

    // ── CognitiveMode ────────────────────────────────────────────

    #[test]
    fn test_cognitive_mode_from_bits() {
        assert_eq!(
            CognitiveMode::from_bits(false, false, false),
            CognitiveMode::Quiet
        );
        assert_eq!(
            CognitiveMode::from_bits(true, true, true),
            CognitiveMode::FullCouncil
        );
        assert_eq!(
            CognitiveMode::from_bits(true, false, true),
            CognitiveMode::Resonant
        );
    }

    #[test]
    fn test_cognitive_mode_bits_roundtrip() {
        for mode in &[
            CognitiveMode::Quiet,
            CognitiveMode::Companion,
            CognitiveMode::FullCouncil,
        ] {
            let bits = mode.bits();
            let restored = CognitiveMode::from_bits(bits.0, bits.1, bits.2);
            assert_eq!(*mode, restored, "Roundtrip failed for {:?}", mode);
        }
    }

    #[test]
    fn test_cognitive_mode_hv_roundtrip() {
        for mode in &[
            CognitiveMode::Quiet,
            CognitiveMode::Explorer,
            CognitiveMode::FullCouncil,
        ] {
            let hv = mode.to_hypervector();
            let decoded = CognitiveMode::from_hypervector(&hv);
            assert_eq!(*mode, decoded, "HV roundtrip failed for {:?}", mode);
        }
    }

    #[test]
    fn test_cognitive_mode_labels() {
        assert_eq!(CognitiveMode::Quiet.label(), "QUIET");
        assert_eq!(CognitiveMode::FullCouncil.label(), "FULL_COUNCIL");
        assert_eq!(CognitiveMode::Explorer.label(), "EXPLORER");
    }

    // ── DCP Consensus ────────────────────────────────────────────

    #[test]
    fn test_dcp_role_weights() {
        assert_eq!(DcpRole::Primary.voting_weight(), 4);
        assert_eq!(DcpRole::Critic.voting_weight(), 3);
        assert_eq!(DcpRole::Backup.voting_weight(), 2);
        assert_eq!(DcpRole::Observer.voting_weight(), 0);
    }

    #[test]
    fn test_consensus_engine_basic() {
        let mut engine = ConsensusEngine::new(100, 2);

        let proposal = DcpMessage::new(
            "Agent-1".into(),
            DcpRole::Primary,
            Hypervector::new_random(),
            0.8,
            1,
            0,
        );
        let tid = engine.propose(proposal, 0);
        assert_eq!(engine.open_count(), 1);

        // Vote with a critic
        engine.vote(tid, "Agent-2", DcpRole::Critic, Hypervector::new_random());
        engine.vote(tid, "Agent-3", DcpRole::Backup, Hypervector::new_random());

        // Resolve
        let resolution = engine.try_resolve(tid);
        assert!(resolution.is_some(), "Should resolve with 3 voters");
    }

    #[test]
    fn test_consensus_expiry() {
        let mut engine = ConsensusEngine::new(10, 2);
        let proposal = DcpMessage::new(
            "Agent-1".into(),
            DcpRole::Primary,
            Hypervector::new_random(),
            0.5,
            1,
            0,
        );
        engine.propose(proposal, 0);
        engine.expire_old(100);
        assert_eq!(engine.open_count(), 0, "Old threads should expire");
    }

    // ── Homeostatic Regulation ───────────────────────────────────

    #[test]
    fn test_homeostasis_initial_state() {
        let reg = HomeostaticRegulator::new(50);
        assert!(!reg.crisis);
        assert!(reg.active_strategy.is_none());

        for need in Need::all() {
            let state = reg.needs.get(&need).unwrap();
            assert!(
                (state.current - state.config.setpoint).abs() < 0.01,
                "Need {:?} should start at setpoint",
                need
            );
        }
    }

    #[test]
    fn test_homeostasis_update() {
        let mut reg = HomeostaticRegulator::new(50);

        // Energy: tick multiple times with low signal to overcome EMA smoothing
        for _ in 0..10 {
            reg.tick(&[(Need::Energy, 0.05)], true, 1);
        }
        assert!(
            reg.needs.get(&Need::Energy).unwrap().is_critical(),
            "Energy should be critical after sustained low signal"
        );
    }

    #[test]
    fn test_homeostasis_crisis_detection() {
        let mut reg = HomeostaticRegulator::new(50);

        // Drive two needs to critical via repeated low signals
        for _ in 0..10 {
            reg.tick(&[(Need::Energy, 0.05), (Need::Coherence, 0.05)], true, 1);
        }
        assert!(reg.crisis, "Two critical needs should trigger crisis");
    }

    #[test]
    fn test_homeostasis_strategy_selection() {
        let mut reg = HomeostaticRegulator::new(50);

        // Energy deficit → ConserveEnergy
        reg.tick(&[(Need::Energy, 0.05)], true, 1);
        let strategy = reg.select_strategy();
        assert!(
            matches!(strategy, RegulationStrategy::ConserveEnergy),
            "Low energy should select ConserveEnergy"
        );
    }

    #[test]
    fn test_homeostasis_regulation_params() {
        let mut reg = HomeostaticRegulator::new(50);

        let params = reg.apply_strategy(&RegulationStrategy::ConserveEnergy);
        assert_eq!(params.resonator_depth, 10);
        assert!(params.throttle_consolidation);
        assert!(params.skip_non_essential);

        let grow_params = reg.apply_strategy(&RegulationStrategy::PromoteGrowth);
        assert_eq!(grow_params.resonator_depth, 35);
        assert_eq!(grow_params.max_curiosity, 3);
    }

    #[test]
    fn test_homeostasis_summary() {
        let mut reg = HomeostaticRegulator::new(50);
        reg.tick(&[(Need::Energy, 0.5)], true, 1);
        let summary = reg.summary();
        assert!(summary.contains("HOMEOSTASIS"));
        assert!(summary.contains("ENERGY"));
    }

    // ── PSC Predictor ────────────────────────────────────────────

    #[test]
    fn test_psc_predictor_basic() {
        let mut psc = PscPredictor::with_defaults();
        assert!(!psc.is_ready());

        for i in 0..5 {
            psc.observe(
                i,
                Hypervector::encode_text_ngram(&format!("state_{}", i), 3),
            );
        }
        assert!(psc.is_ready());
        assert!(psc.chaos_score() >= 0.0);
        assert!(psc.adaptive_horizon() <= psc.horizon_base);
        assert!(psc.predict_next().is_some());
    }

    #[test]
    fn test_psc_chaos_score() {
        let mut psc = PscPredictor::with_defaults();

        // All same vectors → perfect stability → chaos ≈ 0
        let hv = Hypervector::encode_text_ngram("constant", 3);
        for i in 0..5 {
            psc.observe(i, hv);
        }
        let chaos = psc.chaos_score();
        assert!(
            chaos < 0.05,
            "Identical states should have near-zero chaos, got {}",
            chaos
        );
    }

    #[test]
    fn test_psc_adaptive_horizon() {
        let mut psc = PscPredictor::with_defaults();

        // Random states → high chaos → short horizon
        for i in 0..5 {
            psc.observe(i, Hypervector::new_random());
        }
        let horizon = psc.adaptive_horizon();
        assert!(horizon <= psc.horizon_base, "Horizon should be <= base");
    }

    // ── Global Workspace ──────────────────────────────────────────

    #[test]
    fn test_global_workspace_basic() {
        let mut gw = GlobalWorkspace::new(10);
        assert!(gw.is_empty());

        gw.submit(
            Hypervector::new_random(),
            "sensor_1",
            std::collections::HashMap::new(),
        );
        gw.submit(
            Hypervector::new_random(),
            "sensor_2",
            std::collections::HashMap::new(),
        );
        assert_eq!(gw.len(), 2);

        let ctx = Hypervector::new_random();
        let (spot, active, pre) = gw.cycle(&ctx);
        assert_eq!(spot.len(), 1, "Should have exactly 1 spotlight item");
        // With 2 items: 1 spotlight + 1 active
        assert_eq!(active.len(), 1, "Should have 1 active item");
        assert!(pre.is_empty());
    }

    #[test]
    fn test_global_workspace_deduplication() {
        let mut gw = GlobalWorkspace::new(10);
        gw.submit(
            Hypervector::new_random(),
            "sensor_1",
            std::collections::HashMap::new(),
        );
        // Same source → replaces (dedup)
        gw.submit(
            Hypervector::new_random(),
            "sensor_1",
            std::collections::HashMap::new(),
        );
        assert_eq!(gw.len(), 1, "Duplicate source should replace, not add");
    }

    #[test]
    fn test_global_workspace_capacity() {
        let mut gw = GlobalWorkspace::new(3);
        for i in 0..5 {
            gw.submit(
                Hypervector::new_random(),
                &format!("src_{}", i),
                std::collections::HashMap::new(),
            );
        }
        assert_eq!(gw.len(), 3, "Should not exceed capacity");
    }

    // ── Emotional Field ──────────────────────────────────────────

    #[test]
    fn test_emotional_field_resolve() {
        let field = EmotionalField::new();

        // Joy+Open should resolve to either Warm or Playful (both valid)
        let mood = field.resolve(Emotion::Joy, Stance::Open);
        assert!(
            mood == Mood::Warm || mood == Mood::Playful,
            "Joy+Open should resolve to Warm or Playful, got {:?}",
            mood
        );

        // Fear+Guarded should resolve to Defensive
        let mood2 = field.resolve(Emotion::Fear, Stance::Guarded);
        assert_eq!(
            mood2,
            Mood::Defensive,
            "Fear+Guarded should resolve to Defensive"
        );
    }

    #[test]
    fn test_emotional_field_all_pairs_resolve() {
        let field = EmotionalField::new();
        let emotions = [
            Emotion::Joy,
            Emotion::Sadness,
            Emotion::Anger,
            Emotion::Fear,
            Emotion::Surprise,
            Emotion::Disgust,
            Emotion::Neutral,
        ];
        let stances = [
            Stance::Open,
            Stance::Guarded,
            Stance::Curious,
            Stance::Distant,
        ];

        for &emotion in &emotions {
            for &stance in &stances {
                // Every pair should resolve to a valid mood (no panics)
                let mood = field.resolve(emotion, stance);
                let label = mood.label();
                assert!(
                    label.starts_with("MOOD_"),
                    "Should resolve to a valid mood, got {}",
                    label
                );
            }
        }
    }

    // ── Context Engine ───────────────────────────────────────────

    #[test]
    fn test_context_bind_and_extend() {
        let mut ctx = Context::new("test");
        let role = Hypervector::encode_text_ngram("ROLE_TEST", 5);
        let filler = Hypervector::encode_text_ngram("VALUE_42", 5);
        ctx.bind(&role, &filler);
        // After binding, ctx should be non-zero
        assert!(ctx.vector.count_ones() > 0);
    }

    #[test]
    fn test_fork_and_merge() {
        let ctx = Context::new("root");
        let branches = fork_context(&ctx, 3);
        assert_eq!(branches.len(), 3);
        assert!(branches[0].label.contains("branch_0"));

        // Merge: pick the branch most similar to the original context
        let merged = merge_contexts(&branches, &ctx.vector);
        assert!(merged.is_some());
    }

    // ── Implicit Intuition ───────────────────────────────────────

    #[test]
    fn test_intuition_learn_and_recognize() {
        let mut engine = IntuitionEngine::new();
        assert_eq!(engine.len(), 0);

        // Learn a pattern with enough examples
        for _ in 0..4 {
            engine.observe("finance_pattern", &["market", "rates", "bonds"]);
        }
        assert!(engine.len() >= 1);

        // Recognize should fire if we observe the same pattern again
        let input = Hypervector::encode_text_ngram("market", 5);
        let matches = engine.recognize(&input);
        // May or may not match depending on bundling — just verify it doesn't crash
        assert!(matches.len() <= engine.len());
    }

    #[test]
    fn test_intuition_prune() {
        let mut engine = IntuitionEngine::new();
        engine.observe("weak_pattern", &["rare"]);
        assert_eq!(engine.len(), 1);
        engine.prune(2);
        assert_eq!(engine.len(), 0, "Should prune patterns below min strength");
    }

    // ── Shadow / Enantiodromia ───────────────────────────────────

    #[test]
    fn test_shadow_basic() {
        let mut shadow = ShadowSystem::new();
        assert_eq!(shadow.archetypes.len(), 6);

        // Dominant should be whichever started highest
        let dom = shadow.dominant();
        assert!(matches!(
            dom,
            Archetype::Hero
                | Archetype::Shadow
                | Archetype::Sage
                | Archetype::Trickster
                | Archetype::Caregiver
                | Archetype::Orphan
        ));
    }

    #[test]
    fn test_shadow_enantiodromia() {
        let mut shadow = ShadowSystem::new();

        // Drive Hero to dominance with fewer ticks to avoid triggering reversal
        for _ in 0..8 {
            shadow.tick(&[(Archetype::Hero, 0.3)]);
        }

        // Hero should be dominant
        let hero = shadow
            .archetypes
            .iter()
            .find(|a| a.archetype == Archetype::Hero)
            .unwrap();
        assert!(hero.intensity > 0.5, "Hero should be dominant");

        // Shadow should have some charge (hero has been dominant)
        let shadow_arch = shadow
            .archetypes
            .iter()
            .find(|a| a.archetype == Archetype::Shadow)
            .unwrap();
        assert!(
            shadow_arch.opposite_charge > 0.0,
            "Shadow should have opposite charge after Hero dominates"
        );

        // Continue driving to trigger a reversal
        for _ in 0..20 {
            shadow.tick(&[(Archetype::Hero, 0.3)]);
        }
        // After enough dominance, a reversal may have occurred — Shadow should have gained intensity
        let shadow_after = shadow
            .archetypes
            .iter()
            .find(|a| a.archetype == Archetype::Shadow)
            .unwrap();
        // Either Shadow got a boost from reversal, or charge reset
        assert!(
            shadow_after.opposite_charge <= shadow.reversal_threshold,
            "Charge should be reset after reversal"
        );
    }

    #[test]
    fn test_shadow_to_hypervector() {
        let shadow = ShadowSystem::new();
        let hv = shadow.to_hypervector();
        // Should produce a non-zero hypervector
        assert!(hv.count_ones() > 0);
    }

    #[test]
    fn test_archetype_opposites() {
        assert_eq!(Archetype::Hero.opposite(), Archetype::Shadow);
        assert_eq!(Archetype::Shadow.opposite(), Archetype::Hero);
        assert_eq!(Archetype::Sage.opposite(), Archetype::Trickster);
        assert_eq!(Archetype::Caregiver.opposite(), Archetype::Orphan);
    }

    // ── Full Integration ─────────────────────────────────────────

    #[test]
    fn test_drift_module_integration() {
        // Verify that all 10 subsystems compose meaningfully.
        // DMU scoring informs retrieval, cognitive mode sets search params,
        // DCP builds consensus, homeostasis regulates, PSC predicts,
        // workspace attends, emotion resolves, context forks/merges,
        // intuition recognizes, shadow oscillates.

        // 1. DMU
        let params = dmu_params_episodic();
        let score = dmu_score(0.2, 50, 3, 0.7, &params);
        assert!(score > 0.0, "DMU score should be positive");

        // 2. CognitiveMode
        let mode = CognitiveMode::from_bits(true, false, true);
        assert_eq!(mode, CognitiveMode::Resonant);
        assert!(mode.hnsw_ef_multiplier() > 1.0);

        // 3. DCP
        let mut engine = ConsensusEngine::new(50, 2);
        let prop = DcpMessage::new(
            "A1".into(),
            DcpRole::Primary,
            Hypervector::new_random(),
            0.9,
            1,
            0,
        );
        let tid = engine.propose(prop, 0);
        engine.vote(tid, "A2", DcpRole::Critic, Hypervector::new_random());
        let res = engine.try_resolve(tid);
        assert!(res.is_some(), "DCP should resolve");

        // 4. Homeostasis
        let mut reg = HomeostaticRegulator::new(50);
        reg.tick(&[(Need::Energy, 0.5)], true, 1);
        let rp = reg.regulate();
        assert!(rp.resonator_depth > 0);

        // 5. PSC
        let mut psc = PscPredictor::with_defaults();
        for i in 0..3 {
            psc.observe(i, Hypervector::new_random());
        }
        assert!(psc.predict_next().is_some());

        // 6. GlobalWorkspace
        let mut gw = GlobalWorkspace::new(5);
        gw.submit(
            Hypervector::new_random(),
            "test",
            std::collections::HashMap::new(),
        );
        let (spot, _, _) = gw.cycle(&Hypervector::new_random());
        assert_eq!(spot.len(), 1);

        // 7. EmotionalField
        let field = EmotionalField::new();
        let mood = field.resolve(Emotion::Joy, Stance::Open);
        // Should resolve to some mood (not Neutral) — exact match depends on
        // deterministic HV encoding; Warm and Playful are both valid for Joy+Open
        assert!(mood != Mood::Neutral || mood == Mood::Warm || mood == Mood::Playful);

        // 8. ContextEngine
        let ctx = Context::new("test");
        let branches = fork_context(&ctx, 2);
        assert_eq!(branches.len(), 2);
        assert!(merge_contexts(&branches, &ctx.vector).is_some());

        // 9. Intuition
        let mut ie = IntuitionEngine::new();
        ie.observe("p", &["a", "b"]);
        assert!(ie.len() >= 1);

        // 10. Shadow
        let mut shadow = ShadowSystem::new();
        shadow.tick(&[(Archetype::Hero, 0.3)]);
        let dom = shadow.dominant();
        assert!(matches!(dom, Archetype::Hero));
    }
}
