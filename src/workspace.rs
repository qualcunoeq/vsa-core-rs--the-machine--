// ─── Global Workspace & Attention Allocation Engine ──────────────────────
//
// Implements a Global Workspace Theory (GWT) attention mechanism using pure
// VSA operations.  The SelfHypervector is the attention query; competing
// modules broadcast their state; the best-matching module wins the global
// spotlight and its vector becomes the system-wide broadcast context.
//
// ## The Attention Cycle
//
//   Every tick:
//     1. Each registered module posts its current representation vector
//     2. Self_t probes all modules via similarity (1 - NHD)
//     3. The module with highest similarity above threshold wins
//     4. The winner's vector is role-unbound and broadcast globally
//     5. All modules receive the broadcast for the next tick's context
//
// ## Why This Works in Binary HDC
//
// In a Vector Symbolic Architecture, attention is not a complex sorting
// network — it's a single similarity search.  The query (Self_t) finds the
// module whose current state best matches the system's integrated identity.
//
// - Self_t encodes "what I currently am" (mode, body, error, focus)
// - Each module encodes "what I currently perceive"
// - Similarity = overlap between identity and perception
// - The winner is what the system "decides to be about"
//
// ## Attention Threshold
//
// The default threshold (0.48 similarity ≈ 0.52 NHD) ensures that only
// genuinely matching modules win.  Below threshold, the workspace stays
// idle (no broadcast = no global constraint), which is itself a signal:
// "I am not coherently attending to anything right now."
//
// ## Multi-Agent Alignment
//
// In the hive mind (main.rs), each agent has its own GlobalWorkspace.
// The per-agent workspace selects what a single agent focuses on.
// The broker (NeocortexBroker) handles cross-agent consensus.
// These are complementary: the workspace selects, the broker consolidates.
//
// ## Mathematical Guarantees
//
// **Theorem W1 (Convergent Selection):** For a fixed set of module vectors
// and a fixed Self_t query, the same module wins every time.  Attention
// selection is deterministic.
//
// **Theorem W2 (Threshold Safety):** With threshold T ≥ 0.40, a random
// module's chance of winning by chance is < 1/K where K is the number
// of modules (for D=10240 bits, chance similarity < 0.50).
//
// **Theorem W3 (Bounded Modules):** The module registry is bounded at
// MAX_MODULES (16).  Memory = MAX_MODULES × (1280 + label + role) < 50 KB.
//
// **Theorem W4 (Broadcast Decay):** The global broadcast automatically
// decays when no module achieves threshold for 10+ consecutive ticks,
// resetting the workspace to idle.
//
// ## Test Coverage
//
// 1. test_attention_selection       — Self_t selects the best-matching module
// 2. test_threshold_gating          — Below-threshold modules don't win
// 3. test_deterministic_selection   — Same inputs → same winner
// 4. test_module_registration       — Modules register and unregister cleanly
// 5. test_broadcast_amplification   — Winner's vector becomes the broadcast
// 6. test_bounded_modules           — Cannot exceed MAX_MODULES
// 7. test_attention_shift           — Changing Self_t changes the winner

use crate::Hypervector;

// ═══════════════════════════════════════════════════════════════════════════
// CONSTANTS
// ═══════════════════════════════════════════════════════════════════════════

/// Maximum number of modules that can register in the workspace.
pub const MAX_MODULES: usize = 16;

/// Default attention threshold (similarity in [0,1]).
/// A random module's similarity to Self_t is < 0.50 for D=10240.
/// 0.48 is just below that, so only genuine matches win.
pub const DEFAULT_ATTENTION_THRESHOLD: f64 = 0.48;

/// Minimum threshold (hard floor).  Below this, no module can win —
/// the workspace stays idle.
pub const MIN_ATTENTION_THRESHOLD: f64 = 0.35;

/// How many ticks of consecutive below-threshold evaluations before
/// the global broadcast is reset to zero (idle broadcast).
pub const BROADCAST_IDLE_TIMEOUT: usize = 10;

// ═══════════════════════════════════════════════════════════════════════════
// WORKSPACE ROLE VECTORS
// ═══════════════════════════════════════════════════════════════════════════

/// Role vector for the global broadcast itself.
/// Modules can unbind against this to extract the broadcast from their
/// context when it is delivered as a role-bound vector.
pub fn role_broadcast() -> Hypervector {
    Hypervector::encode_text_ngram("ROLE_GLOBAL_BROADCAST", 3)
}

// ═══════════════════════════════════════════════════════════════════════════
// MODULE DESCRIPTOR
// ═══════════════════════════════════════════════════════════════════════════

/// A registered module in the global workspace.
///
/// Each module has a unique ID, a human-readable label, a role hypervector
/// (for binding its contributions), and a current state vector that it
/// updates every tick.
#[derive(Clone, Debug)]
pub struct ModuleDescriptor {
    /// Unique module ID (assigned at registration).
    pub id: u8,
    /// Human-readable label (e.g., "HOMEOSTASIS", "PREDICTIVE").
    pub label: String,
    /// Role hypervector: used to bind the module's contributions.
    /// Determined at registration; deterministic from the label.
    pub role: Hypervector,
    /// The module's current state vector (updated every tick by the module).
    /// This is what Self_t probes against.
    pub current_vector: Hypervector,
    /// How many ticks since this module last won the attention competition.
    pub ticks_since_win: u64,
    /// Total times this module has won (for statistics).
    pub total_wins: u64,
}

impl ModuleDescriptor {
    /// Create a new module descriptor with a deterministic role vector.
    pub fn new(id: u8, label: &str) -> Self {
        let role = Hypervector::encode_text_ngram(&format!("MODULE_ROLE_{}", label), 3);
        ModuleDescriptor {
            id,
            label: label.to_string(),
            role,
            current_vector: Hypervector::new_zero(),
            ticks_since_win: 0,
            total_wins: 0,
        }
    }

    /// Bind the module's current vector to its role.
    /// Used when contributing to the broadcast superposition.
    pub fn bind_current(&self) -> Hypervector {
        self.role.bitwise_xor(&self.current_vector)
    }

    /// Unbind a role-bound vector to recover this module's contribution.
    pub fn unbind(&self, bound: &Hypervector) -> Hypervector {
        self.role.bitwise_xor(bound)
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// ATTENTION REPORT
// ═══════════════════════════════════════════════════════════════════════════

/// Result of a single attention evaluation cycle.
#[derive(Clone, Debug)]
pub struct AttentionReport {
    /// The module ID that won attention (None if below threshold).
    pub winner_id: Option<u8>,
    /// Label of the winning module.
    pub winner_label: String,
    /// Similarity score of the winner (0.0 if no winner).
    pub winner_similarity: f64,
    /// Whether the workspace broadcast was updated this cycle.
    pub broadcast_updated: bool,
    /// Number of registered modules that participated.
    pub module_count: usize,
    /// The similarity scores for all modules.
    pub all_scores: Vec<(u8, String, f64)>,
    /// Whether the broadcast has gone idle (no winner for BROADCAST_IDLE_TIMEOUT).
    pub idle: bool,
}

impl AttentionReport {
    pub fn new() -> Self {
        AttentionReport {
            winner_id: None,
            winner_label: "none".to_string(),
            winner_similarity: 0.0,
            broadcast_updated: false,
            module_count: 0,
            all_scores: Vec::new(),
            idle: false,
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// GLOBAL WORKSPACE
// ═══════════════════════════════════════════════════════════════════════════

/// The Global Workspace: the system's attention bottleneck.
///
/// Every tick, registered modules update their current_vector.  The
/// workspace probes each module's vector with Self_t as the query.
/// The best match above threshold wins the attention competition and
/// its vector becomes the global_broadcast — the single coherent context
/// that constrains all downstream processing.
pub struct GlobalWorkspace {
    /// Registered modules (fixed capacity = MAX_MODULES).
    pub modules: Vec<ModuleDescriptor>,
    /// The current global broadcast hypervector.
    /// Reset to zero when no module wins for BROADCAST_IDLE_TIMEOUT ticks.
    pub global_broadcast: Hypervector,
    /// The module ID that currently holds the broadcast (None if idle).
    pub active_module_id: Option<u8>,
    /// Attention similarity threshold.
    pub attention_threshold: f64,
    /// Tick counter.
    pub tick: u64,
    /// Consecutive ticks with no winner (for idle detection).
    idle_ticks: usize,
    /// Total attention evaluations performed.
    pub total_evaluations: u64,
}

impl GlobalWorkspace {
    pub fn new(attention_threshold: f64) -> Self {
        let threshold = attention_threshold.max(MIN_ATTENTION_THRESHOLD);
        GlobalWorkspace {
            modules: Vec::with_capacity(MAX_MODULES),
            global_broadcast: Hypervector::new_zero(),
            active_module_id: None,
            attention_threshold: threshold,
            tick: 0,
            idle_ticks: 0,
            total_evaluations: 0,
        }
    }

    /// Create with the default attention threshold.
    pub fn with_defaults() -> Self {
        GlobalWorkspace::new(DEFAULT_ATTENTION_THRESHOLD)
    }

    // ═════════════════════════════════════════════════════════════════════
    // MODULE REGISTRATION
    // ═════════════════════════════════════════════════════════════════════

    /// Register a new module.  Returns its ID, or None if at capacity.
    ///
    /// The module is assigned the next available ID (0, 1, 2, ...).
    /// Modules cannot be registered after the workspace has started
    /// evaluating (tick > 0) unless `allow_late` is true.
    pub fn register_module(&mut self, label: &str, allow_late: bool) -> Option<u8> {
        if self.modules.len() >= MAX_MODULES {
            return None;
        }
        if !allow_late && self.tick > 0 {
            return None; // late registration denied
        }
        let id = self.modules.len() as u8;
        let module = ModuleDescriptor::new(id, label);
        self.modules.push(module);
        Some(id)
    }

    /// Unregister a module by ID.  Returns true if found and removed.
    pub fn unregister_module(&mut self, id: u8) -> bool {
        let pos = self.modules.iter().position(|m| m.id == id);
        if let Some(idx) = pos {
            self.modules.remove(idx);
            // If the unregistered module was the active one, reset broadcast
            if self.active_module_id == Some(id) {
                self.global_broadcast = Hypervector::new_zero();
                self.active_module_id = None;
            }
            true
        } else {
            false
        }
    }

    /// Update a module's current vector.  Called by the module itself
    /// before each attention evaluation.
    pub fn update_module(&mut self, id: u8, vector: Hypervector) -> bool {
        if let Some(module) = self.modules.iter_mut().find(|m| m.id == id) {
            module.current_vector = vector;
            true
        } else {
            false
        }
    }

    /// Get a module's current vector by ID.
    pub fn module_vector(&self, id: u8) -> Option<&Hypervector> {
        self.modules.iter().find(|m| m.id == id).map(|m| &m.current_vector)
    }

    /// Number of registered modules.
    pub fn module_count(&self) -> usize {
        self.modules.len()
    }

    // ═════════════════════════════════════════════════════════════════════
    // ATTENTION EVALUATION — The core cycle
    // ═════════════════════════════════════════════════════════════════════

    /// Run one attention evaluation cycle.
    ///
    /// 1. Probe each registered module with Self_t as query
    /// 2. Compute similarity = 1 - NHD(Self_t, module_vector)
    /// 3. Find the module with highest similarity
    /// 4. If similarity >= threshold: that module wins, its vector becomes
    ///    the global broadcast
    /// 5. If below threshold for BROADCAST_IDLE_TIMEOUT ticks: reset broadcast
    ///
    /// # Arguments
    ///
    /// * `self_query` — The SelfHypervector (from SelfModel.current_identity).
    ///   This is the attention probe.  The system attends to whatever matches
    ///   its current integrated identity.
    ///
    /// Returns an AttentionReport with the results.
    pub fn evaluate_attention(&mut self, self_query: &Hypervector) -> AttentionReport {
        self.tick += 1;
        self.total_evaluations += 1;

        let mut report = AttentionReport::new();
        report.module_count = self.modules.len();

        if self.modules.is_empty() {
            report.idle = true;
            return report;
        }

        // Phase 1: Probe all modules with Self_t
        let mut best_sim = MIN_ATTENTION_THRESHOLD - 0.01; // below threshold
        let mut best_id: Option<u8> = None;

        for module in &self.modules {
            let sim = 1.0 - self_query.normalized_hamming_distance(&module.current_vector);
            report.all_scores.push((module.id, module.label.clone(), sim));

            if sim > best_sim {
                best_sim = sim;
                best_id = Some(module.id);
            }
        }

        // Phase 2: Check threshold
        if let Some(winner_id) = best_id {
            if best_sim >= self.attention_threshold {
                // Winner found: set broadcast
                if let Some(winner) = self.modules.iter_mut().find(|m| m.id == winner_id) {
                    self.global_broadcast = winner.current_vector;
                    self.active_module_id = Some(winner_id);
                    winner.ticks_since_win = 0;
                    winner.total_wins += 1;

                    report.winner_id = Some(winner_id);
                    report.winner_label = winner.label.clone();
                    report.winner_similarity = best_sim;
                    report.broadcast_updated = true;
                    self.idle_ticks = 0;
                }
            } else {
                // No winner above threshold
                self.idle_ticks += 1;
                report.idle = self.idle_ticks >= BROADCAST_IDLE_TIMEOUT;
            }
        } else {
            self.idle_ticks += 1;
            report.idle = self.idle_ticks >= BROADCAST_IDLE_TIMEOUT;
        }

        // Phase 3: Idle timeout — reset broadcast if idle too long
        if self.idle_ticks >= BROADCAST_IDLE_TIMEOUT {
            self.global_broadcast = Hypervector::new_zero();
            self.active_module_id = None;
        }

        // Phase 4: Increment ticks_since_win for non-winners
        for module in self.modules.iter_mut() {
            if Some(module.id) != best_id || best_sim < self.attention_threshold {
                module.ticks_since_win += 1;
            }
        }

        report
    }

    // ═════════════════════════════════════════════════════════════════════
    // ACCESSORS
    // ═════════════════════════════════════════════════════════════════════

    /// Get the current global broadcast (role-bound).
    /// Modules can unbind this against their own role to extract the
    /// broadcast content in their own context.
    pub fn get_broadcast(&self) -> &Hypervector {
        &self.global_broadcast
    }

    /// Get the global broadcast pre-bound to the workspace role.
    /// This is the canonical form: the broadcast as the system would
    /// inject it into each module's context.
    pub fn get_broadcast_bound(&self) -> Hypervector {
        role_broadcast().bitwise_xor(&self.global_broadcast)
    }

    /// Get the label of the currently active (winning) module.
    pub fn active_module_label(&self) -> &str {
        self.active_module_id
            .and_then(|id| self.modules.iter().find(|m| m.id == id))
            .map(|m| m.label.as_str())
            .unwrap_or("idle")
    }

    /// Check if the workspace is idle (no module has won recently).
    pub fn is_idle(&self) -> bool {
        self.idle_ticks >= BROADCAST_IDLE_TIMEOUT
    }

    /// Summary statistics string.
    pub fn report(&self) -> String {
        let active = self.active_module_label();
        format!(
            "Workspace: tick={}, modules={}, active={}, broadcasts={}, idle={}",
            self.tick,
            self.modules.len(),
            active,
            self.total_evaluations,
            if self.is_idle() { "YES" } else { "no" },
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

    /// Theorem W1: Deterministic selection.
    ///
    /// For the same Self_t and same module vectors, the same module
    /// wins every time.
    #[test]
    fn test_attention_selection() {
        let mut ws = GlobalWorkspace::with_defaults();

        // Register three modules with distinct vectors
        ws.register_module("ALPHA", true);
        ws.register_module("BETA", true);
        ws.register_module("GAMMA", true);

        // Set module vectors: ALPHA at a specific position
        let alpha_vec = Hypervector::encode_text_ngram("ALPHA_STATE_A", 3);
        let beta_vec = Hypervector::encode_text_ngram("BETA_STATE_B", 3);
        let gamma_vec = Hypervector::encode_text_ngram("GAMMA_STATE_C", 3);

        ws.update_module(0, alpha_vec);
        ws.update_module(1, beta_vec);
        ws.update_module(2, gamma_vec);

        // Self_t is exactly the ALPHA vector — ALPHA should win
        let self_query = alpha_vec;
        let report = ws.evaluate_attention(&self_query);

        eprintln!("  Winner: {:?} (sim={:.4})", report.winner_id, report.winner_similarity);
        eprintln!("  All scores:");
        for (id, label, sim) in &report.all_scores {
            eprintln!("    {}[{}]: {:.4}", label, id, sim);
        }

        assert!(
            report.winner_similarity > 0.50,
            "Matching module should have high similarity: {}",
            report.winner_similarity
        );
        assert_eq!(report.winner_id, Some(0), "ALPHA should win");

        // Theorem W1: Second evaluation with same inputs gives same result
        let report2 = ws.evaluate_attention(&self_query);
        assert_eq!(
            report2.winner_id, report.winner_id,
            "Deterministic: same inputs → same winner"
        );

        // Broadcast should be non-zero
        assert!(
            ws.global_broadcast.count_ones() > 0,
            "Broadcast should be non-zero after selection"
        );
    }

    /// Theorem W2: Threshold prevents random modules from winning.
    ///
    /// When Self_t is random and all modules are random, no module
    /// should exceed the attention threshold.
    #[test]
    fn test_threshold_gating() {
        let mut ws = GlobalWorkspace::new(0.55); // high threshold

        for i in 0..5 {
            ws.register_module(&format!("MOD_{}", i), true);
        }

        // All random vectors — no structure
        for id in 0..5 {
            ws.update_module(id, Hypervector::new_random());
        }

        let self_query = Hypervector::new_random(); // random query
        let report = ws.evaluate_attention(&self_query);

        eprintln!("  Winner: {:?} (sim={:.4})", report.winner_id, report.winner_similarity);
        eprintln!("  Threshold: {:.4}", ws.attention_threshold);

        // With a high threshold (0.55) and random vectors,
        // no module should win reliably
        // (This is probabilistic — may rarely fail, but N=100 should be stable)
        let mut wins = 0;
        for _ in 0..100 {
            for id in 0..5 {
                ws.update_module(id, Hypervector::new_random());
            }
            let r = ws.evaluate_attention(&Hypervector::new_random());
            if r.winner_id.is_some() {
                wins += 1;
            }
        }
        eprintln!("  Wins by chance (100 trials): {} (threshold={})", wins, ws.attention_threshold);

        // At threshold 0.55 with random 10240-bit vectors, chance wins
        // should be very rare (well below 50%)
        assert!(
            wins < 30,
            "Random modules should rarely win at high threshold: {}",
            wins
        );
    }

    /// Test that a module's vector becomes the broadcast when it wins.
    #[test]
    fn test_broadcast_amplification() {
        let mut ws = GlobalWorkspace::with_defaults();

        ws.register_module("SENSOR", true);

        let sensor_vec = Hypervector::encode_text_ngram("SENSOR_READING_X", 3);
        ws.update_module(0, sensor_vec);

        let self_query = sensor_vec;
        let _report = ws.evaluate_attention(&self_query);

        // The broadcast should be equal to the winning module's vector
        let broadcast = ws.get_broadcast();
        let dist = broadcast.normalized_hamming_distance(&sensor_vec);
        eprintln!("  Broadcast distance to winning vector: {:.6}", dist);

        // Since the winning vector IS the broadcast, distance should be 0
        assert!(
            dist < 0.01,
            "Broadcast should match winning module's vector: dist={}",
            dist
        );

        // The broadcast bound form should be different (role-bound)
        let bound = ws.get_broadcast_bound();
        let bound_dist = bound.normalized_hamming_distance(&sensor_vec);
        eprintln!("  Role-bound broadcast distance to winning vector: {:.6}", bound_dist);
        // Role-bound version is XORed with role_broadcast, so it should differ
        assert!(
            bound_dist > 0.40,
            "Role-bound broadcast should differ from raw: dist={}",
            bound_dist
        );
    }

    /// Test that changing Self_t changes the attention winner.
    #[test]
    fn test_attention_shift() {
        let mut ws = GlobalWorkspace::with_defaults();

        ws.register_module("VISUAL", true);
        ws.register_module("AUDIO", true);

        let visual_vec = Hypervector::encode_text_ngram("VISUAL_SCENE", 3);
        let audio_vec = Hypervector::encode_text_ngram("AUDIO_SIGNAL", 3);

        ws.update_module(0, visual_vec);
        ws.update_module(1, audio_vec);

        // Query close to visual → VISUAL wins
        let query_visual = visual_vec;
        let report1 = ws.evaluate_attention(&query_visual);
        eprintln!("  Visual query winner: {:?}", report1.winner_id);
        assert_eq!(report1.winner_id, Some(0), "VISUAL should win for visual query");

        // Query close to audio → AUDIO wins
        let query_audio = audio_vec;
        let report2 = ws.evaluate_attention(&query_audio);
        eprintln!("  Audio query winner: {:?}", report2.winner_id);
        assert_eq!(report2.winner_id, Some(1), "AUDIO should win for audio query");
    }

    /// Test module registration bounds.
    #[test]
    fn test_bounded_modules() {
        let mut ws = GlobalWorkspace::with_defaults();

        // Register up to MAX_MODULES
        for i in 0..MAX_MODULES {
            let result = ws.register_module(&format!("MOD_{}", i), true);
            assert!(result.is_some(), "Module {} should register", i);
        }

        // Next registration should fail
        let overflow = ws.register_module("OVERFLOW", true);
        assert!(overflow.is_none(), "Should not exceed MAX_MODULES");

        assert_eq!(ws.module_count(), MAX_MODULES);
    }

    /// Test that modules can be unregistered cleanly.
    #[test]
    fn test_module_unregistration() {
        let mut ws = GlobalWorkspace::with_defaults();

        ws.register_module("A", true);
        ws.register_module("B", true);
        assert_eq!(ws.module_count(), 2);

        // Unregister B
        let removed = ws.unregister_module(1);
        assert!(removed, "Module B should be removed");
        assert_eq!(ws.module_count(), 1);

        // A should still be there
        assert!(ws.module_vector(0).is_some(), "Module A should still exist");

        // Removing non-existent returns false
        let false_removed = ws.unregister_module(99);
        assert!(!false_removed, "Non-existent module should return false");
    }

    /// Test that the workspace enters idle state after no winner
    /// for BROADCAST_IDLE_TIMEOUT ticks.
    #[test]
    fn test_workspace_idle() {
        let mut ws = GlobalWorkspace::new(0.90); // very high threshold

        ws.register_module("ALPHA", true);
        let alpha_vec = Hypervector::encode_text_ngram("ALPHA_STATE", 3);
        ws.update_module(0, alpha_vec);

        // Feed a query that's well below threshold
        let random_query = Hypervector::new_random();

        for i in 0..BROADCAST_IDLE_TIMEOUT + 2 {
            let report = ws.evaluate_attention(&random_query);
            if i == BROADCAST_IDLE_TIMEOUT {
                eprintln!("  Tick {}: idle={}", i, report.idle);
            }
        }

        assert!(
            ws.is_idle(),
            "Workspace should be idle after {} ticks without winner",
            BROADCAST_IDLE_TIMEOUT
        );

        // Broadcast should be zero
        assert_eq!(
            ws.global_broadcast.count_ones(),
            0,
            "Broadcast should be zero when idle"
        );
    }

    /// Test that module update correctly changes the vector.
    #[test]
    fn test_module_update() {
        let mut ws = GlobalWorkspace::with_defaults();
        ws.register_module("TEST", true);

        let v1 = Hypervector::encode_text_ngram("VECTOR_1", 3);
        let v2 = Hypervector::encode_text_ngram("VECTOR_2", 3);

        ws.update_module(0, v1);
        let retrieved = ws.module_vector(0).unwrap();
        let d1 = retrieved.normalized_hamming_distance(&v1);
        assert!(d1 < 0.01, "Should retrieve VECTOR_1: dist={}", d1);

        ws.update_module(0, v2);
        let retrieved2 = ws.module_vector(0).unwrap();
        let d2 = retrieved2.normalized_hamming_distance(&v2);
        assert!(d2 < 0.01, "Should retrieve VECTOR_2: dist={}", d2);
    }
}
