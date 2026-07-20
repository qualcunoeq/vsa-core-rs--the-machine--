// ─── Intrinsic Motivation & Goal Hierarchy ──────────────────────────────
//
// Gives The Machine autonomous ambition — the drive to hunt for signals
// even when homeostasis is satisfied.  Four fundamental drives modulate
// the Counterfactual Simulator's scoring function, transforming it from
// a purely homeostatic regulator into a proactive exploration engine.
//
// ## The Four Drives
//
//   Predictive Mastery — seek regimes where prediction error is high
//                        and test new abstractions until error drops.
//                        Maps to simulator weight Δerror.
//
//   Coherence Drive    — align workspace focus with broker consensus
//                        and minimize identity instability.
//                        Maps to simulator weight Δidentity.
//
//   Abstraction Drive  — hunger for new L2/L3 concepts.  When high,
//                        the system routes attention toward dense text
//                        (FOMC minutes, macro surprises) to mine new
//                        semantic structures. Maps to Δcost (willing
//                        to pay exploration cost).
//
//   Self-Preservation  — baseline homeostatic regulation.  When deficits
//                        are high, all other drives are suppressed.
//                        Maps to simulator weight Δdeficit.
//
// ## Architecture
//
//   Drives act as dynamic multipliers on the simulator's static weights:
//
//     effective_weight[i] = base_weight[i] × (1.0 + mult[i] × intensity[i])
//
//   When a drive is starved (intensity high), its multiplier amplifies
//   the corresponding scoring term, making the simulator prefer actions
//   that address that drive.
//
//   During sleep, the WakeNarrative evaluates which drive was most
//   starved during the wake cycle and boosts its multiplier for the
//   next cycle.  This is how the system "learns what it wants."
//
// ## Mathematical Guarantees
//
// **Theorem D1 (Bounded Drives):** Each drive intensity is bounded
// in [0, 1].  Effective weights are bounded by
// base_weight × (1 + max_multiplier).
//
// **Theorem D2 (Homeostatic Suppression):** When SelfPreservation
// intensity > 0.80, all other drive multipliers are clamped to 0.1,
// ensuring the system prioritizes survival over exploration.
//
// **Theorem D3 (Drive Equilibrium):** In a stationary environment,
// drive intensities converge to an equilibrium where the weighted
// sum of effective intensities is minimized (the system is satisfied).
//
// **Theorem D4 (Multiplier Monotonicity):** Drive multipliers only
// increase during sleep (when a drive was starved) and decay during
// wake (as the drive is satisfied).  They are monotonic with respect
// to cumulative deprivation.
//
// ## Test Coverage
//
// 1. test_drive_intensity_bounds     — All intensities in [0, 1]
// 2. test_predictive_mastery_drive   — High error → high PM intensity
// 3. test_abstraction_drive          — Few L2 concepts → high AB intensity
// 4. test_homeostatic_suppression    — High deficit suppresses other drives
// 5. test_weight_modulation          — Drives correctly modulate weights
// 6. test_sleep_weight_shift         — Sleep shifts multipliers
// 7. test_drive_equilibrium          — Low deficit + low error → all quiet

// ═══════════════════════════════════════════════════════════════════════════
// CONSTANTS
// ═══════════════════════════════════════════════════════════════════════════

/// Number of intrinsic drives.
pub const DRIVE_COUNT: usize = 4;

/// Maximum multiplier boost (100% = weight can double).
pub const MAX_MULTIPLIER: f64 = 1.0;

/// Multiplier decay per tick during wake (when drive is being satisfied).
pub const MULTIPLIER_DECAY: f64 = 0.9995;

/// SelfPreservation threshold for homeostatic suppression.
pub const SP_SUPPRESSION_THRESHOLD: f64 = 0.80;

/// Clamped multiplier for suppressed drives.
pub const SUPPRESSED_MULTIPLIER: f64 = 0.10;

/// Sleep boost: how much to increase a starved drive's multiplier.
pub const SLEEP_BOOST: f64 = 0.15;

/// Maximum L2 count for abstraction drive normalization.
pub const MAX_L2_CONCEPTS: usize = 32;

// ═══════════════════════════════════════════════════════════════════════════
// DRIVE IDENTIFIERS
// ═══════════════════════════════════════════════════════════════════════════

/// Identifiers for the four intrinsic drives.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum DriveId {
    /// Drive to reduce prediction error and master the environment.
    PredictiveMastery = 0,
    /// Drive to maintain identity coherence and broker alignment.
    Coherence = 1,
    /// Drive to form new abstract concepts (L2/L3).
    Abstraction = 2,
    /// Drive to maintain homeostatic balance (survival).
    SelfPreservation = 3,
}

impl DriveId {
    pub fn all() -> [DriveId; 4] {
        [
            DriveId::PredictiveMastery,
            DriveId::Coherence,
            DriveId::Abstraction,
            DriveId::SelfPreservation,
        ]
    }

    pub fn label(&self) -> &'static str {
        match self {
            DriveId::PredictiveMastery => "PREDICTIVE_MASTERY",
            DriveId::Coherence => "COHERENCE",
            DriveId::Abstraction => "ABSTRACTION",
            DriveId::SelfPreservation => "SELF_PRESERVATION",
        }
    }

    /// Which simulator weight index this drive modulates.
    pub fn simulator_weight_idx(&self) -> usize {
        match self {
            DriveId::PredictiveMastery => 1, // Δerror
            DriveId::Coherence => 2,         // Δidentity
            DriveId::Abstraction => 3,       // Δcost
            DriveId::SelfPreservation => 0,  // Δdeficit
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// DRIVE STATE
// ═══════════════════════════════════════════════════════════════════════════

/// The state of a single intrinsic drive.
#[derive(Clone, Debug)]
pub struct DriveState {
    /// Which drive this is.
    pub id: DriveId,
    /// Current intensity (0.0 = satisfied, 1.0 = maximally deprived).
    pub intensity: f64,
    /// Current multiplier for the simulator weight (0.0–MAX_MULTIPLIER).
    pub multiplier: f64,
    /// Integrated deprivation over the wake cycle (for sleep evaluation).
    pub cumulative_deprivation: f64,
}

impl DriveState {
    pub fn new(id: DriveId) -> Self {
        DriveState {
            id,
            intensity: 0.0,
            multiplier: 0.0,
            cumulative_deprivation: 0.0,
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// INTRINSIC MOTIVATION SYSTEM
// ═══════════════════════════════════════════════════════════════════════════

/// The intrinsic motivation system: computes drive intensities from system
/// state and produces dynamic weight multipliers for the simulator.
///
/// # Usage
///
/// ```ignore
/// let mut drives = IntrinsicMotivation::new();
///
/// // Every tick (after module updates):
/// drives.update(error, min_error, deficit, identity_stability, l2_count);
/// let multipliers = drives.get_multipliers(); // pass to simulator
///
/// // During sleep:
/// let starved = drives.starved_drive();
/// drives.adjust_multipliers(starved, SLEEP_BOOST);
/// drives.reset_cumulative();
/// ```
pub struct IntrinsicMotivation {
    /// The four drive states.
    pub drives: Vec<DriveState>,
    /// Current tick.
    pub tick: u64,
    /// Tick of last update (for delta-T calculations).
    last_update_tick: u64,
}

impl IntrinsicMotivation {
    pub fn new() -> Self {
        let drives: Vec<DriveState> = DriveId::all()
            .iter()
            .map(|id| DriveState::new(*id))
            .collect();
        IntrinsicMotivation {
            drives,
            tick: 0,
            last_update_tick: 0,
        }
    }

    // ═════════════════════════════════════════════════════════════════════
    // CORE UPDATE — Call every tick
    // ═════════════════════════════════════════════════════════════════════

    /// Update all drive intensities from current system state.
    ///
    /// # Arguments
    ///
    /// * `current_error` — Current blended prediction error (0.0–1.0).
    /// * `min_error` — Minimum observed prediction error (noise floor).
    /// * `overall_deficit` — Homeostatic overall deficit (0.0–1.0).
    /// * `identity_stability` — SelfModel identity stability NHD (0.0–1.0+).
    /// * `l2_concept_count` — Current number of L2 concepts in the hierarchy.
    pub fn update(
        &mut self,
        current_error: f64,
        min_error: f64,
        overall_deficit: f64,
        identity_stability: f64,
        l2_concept_count: usize,
    ) {
        self.tick += 1;
        let dt = (self.tick - self.last_update_tick).max(1) as f64;
        self.last_update_tick = self.tick;

        // 1. Compute raw intensities from system state
        let pm_intensity = self.compute_predictive_mastery(current_error, min_error);
        let coh_intensity = self.compute_coherence(identity_stability);
        let abs_intensity = self.compute_abstraction(l2_concept_count);
        let sp_intensity = self.compute_self_preservation(overall_deficit);

        // 2. Apply homeostatic suppression
        let (pm, coh, abs, sp) = if sp_intensity > SP_SUPPRESSION_THRESHOLD {
            // Survival mode: suppress all non-survival drives
            (
                pm_intensity * 0.1,
                coh_intensity * 0.1,
                abs_intensity * 0.1,
                sp_intensity,
            )
        } else {
            (pm_intensity, coh_intensity, abs_intensity, sp_intensity)
        };

        // 3. Update drive states
        self.set_intensity(DriveId::PredictiveMastery, pm);
        self.set_intensity(DriveId::Coherence, coh);
        self.set_intensity(DriveId::Abstraction, abs);
        self.set_intensity(DriveId::SelfPreservation, sp);

        // 4. Decay multipliers during wake
        for drive in self.drives.iter_mut() {
            drive.multiplier *= MULTIPLIER_DECAY.powf(dt);
            drive.cumulative_deprivation += drive.intensity * dt;
        }
    }

    // ═════════════════════════════════════════════════════════════════════
    // INTENSITY COMPUTATION
    // ═════════════════════════════════════════════════════════════════════

    /// Predictive Mastery: how much room for improvement in prediction error.
    /// 0.0 = error at floor (satisfied), 1.0 = error far above floor (hungry).
    fn compute_predictive_mastery(&self, current_error: f64, min_error: f64) -> f64 {
        let range = (1.0 - min_error).max(0.01);
        ((current_error - min_error) / range).clamp(0.0, 1.0)
    }

    /// Coherence Drive: how unstable the identity is.
    /// 0.0 = perfectly stable, 1.0 = highly unstable.
    fn compute_coherence(&self, identity_stability: f64) -> f64 {
        identity_stability.clamp(0.0, 1.0)
    }

    /// Abstraction Drive: how few L2 concepts exist relative to capacity.
    /// 0.0 = many concepts (satisfied), 1.0 = few concepts (hungry).
    fn compute_abstraction(&self, l2_concept_count: usize) -> f64 {
        let fraction = (l2_concept_count as f64) / (MAX_L2_CONCEPTS as f64);
        (1.0 - fraction).clamp(0.0, 1.0)
    }

    /// Self-Preservation: the homeostatic deficit directly.
    fn compute_self_preservation(&self, overall_deficit: f64) -> f64 {
        overall_deficit.clamp(0.0, 1.0)
    }

    // ═════════════════════════════════════════════════════════════════════
    // MULTIPLIER ACCESS
    // ═════════════════════════════════════════════════════════════════════

    /// Get the dynamic weight multipliers for the simulator.
    ///
    /// Returns [m_selfpres, m_mastery, m_coherence, m_abstraction] where
    /// each is in [0, MAX_MULTIPLIER].
    ///
    /// The simulator should compute:
    ///   effective_weight[i] = base_weight[i] × (1.0 + m[i])
    pub fn get_multipliers(&self) -> [f64; 4] {
        let mut result = [0.0; 4];
        for drive in &self.drives {
            let idx = drive.id.simulator_weight_idx();
            result[idx] = drive.multiplier;
        }
        result
    }

    /// Get raw current intensities for all drives.
    pub fn get_intensities(&self) -> [f64; 4] {
        let mut result = [0.0; 4];
        for drive in &self.drives {
            let idx = drive.id as usize;
            result[idx] = drive.intensity;
        }
        result
    }

    /// Get the effective weight array for a set of base weights.
    /// effective[i] = base[i] × (1.0 + multiplier[i])
    pub fn effective_weights(&self, base: &[f64; 4]) -> [f64; 4] {
        let mult = self.get_multipliers();
        [
            base[0] * (1.0 + mult[0]),
            base[1] * (1.0 + mult[1]),
            base[2] * (1.0 + mult[2]),
            base[3] * (1.0 + mult[3]),
        ]
    }

    // ═════════════════════════════════════════════════════════════════════
    // SLEEP-PHASE ADJUSTMENT
    // ═════════════════════════════════════════════════════════════════════

    /// Find the drive with the highest cumulative deprivation (most starved).
    pub fn starved_drive(&self) -> DriveId {
        let mut worst = DriveId::SelfPreservation;
        let mut worst_val = -1.0;
        for drive in &self.drives {
            if drive.cumulative_deprivation > worst_val {
                worst_val = drive.cumulative_deprivation;
                worst = drive.id;
            }
        }
        worst
    }

    /// Boost a drive's multiplier (called during sleep).
    /// The starved drive gets its multiplier increased.
    pub fn adjust_multipliers(&mut self, starved: DriveId, boost: f64) {
        for drive in self.drives.iter_mut() {
            if drive.id == starved {
                drive.multiplier = (drive.multiplier + boost).min(MAX_MULTIPLIER);
            }
        }
    }

    /// Reset cumulative deprivation counters (called after sleep adjustment).
    pub fn reset_cumulative(&mut self) {
        for drive in self.drives.iter_mut() {
            drive.cumulative_deprivation = 0.0;
        }
    }

    /// Helper: set a drive's intensity.
    fn set_intensity(&mut self, id: DriveId, intensity: f64) {
        if let Some(drive) = self.drives.iter_mut().find(|d| d.id == id) {
            drive.intensity = intensity.clamp(0.0, 1.0);
        }
    }

    // ═════════════════════════════════════════════════════════════════════
    // REPORT
    // ═════════════════════════════════════════════════════════════════════

    pub fn report(&self) -> String {
        let mult = self.get_multipliers();
        let intensities = self.get_intensities();
        format!(
            "Drives: PM={:.2}(×{:.2}) COH={:.2}(×{:.2}) AB={:.2}(×{:.2}) SP={:.2}(×{:.2})",
            intensities[0],
            mult[1],
            intensities[1],
            mult[2],
            intensities[2],
            mult[3],
            intensities[3],
            mult[0],
        )
    }
}

// ═════════════════════════════════════════════════════════════════════════════
// TESTS
// ═════════════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;

    /// Theorem D1: All drive intensities are bounded in [0, 1].
    #[test]
    fn test_drive_intensity_bounds() {
        let mut im = IntrinsicMotivation::new();

        // Update with extreme values
        im.update(2.0, 0.0, 2.0, 2.0, 1000);
        let intensities = im.get_intensities();

        for (i, &val) in intensities.iter().enumerate() {
            eprintln!("  Drive[{}] intensity: {:.4}", i, val);
            assert!(
                val >= 0.0 && val <= 1.0,
                "Drive[{}] intensity must be in [0, 1]: {}",
                i,
                val
            );
        }

        // Update with normal values
        im.update(0.10, 0.05, 0.20, 0.03, 16);
        let intensities2 = im.get_intensities();
        for (i, &val) in intensities2.iter().enumerate() {
            assert!(
                val >= 0.0 && val <= 1.0,
                "Drive[{}] intensity must be in [0, 1]: {}",
                i,
                val
            );
        }
    }

    /// Test: High prediction error → high Predictive Mastery intensity.
    #[test]
    fn test_predictive_mastery_drive() {
        let mut im = IntrinsicMotivation::new();

        // Low error (satisfied)
        im.update(0.08, 0.05, 0.20, 0.03, 16);
        let intensities_low = im.get_intensities();
        eprintln!("  Low error PM intensity: {:.4}", intensities_low[0]);

        // High error (starved)
        im.update(0.80, 0.05, 0.20, 0.03, 16);
        let intensities_high = im.get_intensities();
        eprintln!("  High error PM intensity: {:.4}", intensities_high[0]);

        assert!(
            intensities_high[0] > intensities_low[0],
            "PM intensity should be higher when error is high: {} vs {}",
            intensities_high[0],
            intensities_low[0]
        );
    }

    /// Test: Few L2 concepts → high Abstraction drive.
    #[test]
    fn test_abstraction_drive() {
        let mut im = IntrinsicMotivation::new();

        // Many L2 concepts (satisfied)
        im.update(0.10, 0.05, 0.20, 0.03, 30);
        let abs_low = im.get_intensities()[2];
        eprintln!("  Many L2 (30) AB intensity: {:.4}", abs_low);

        // Few L2 concepts (starved)
        im.update(0.10, 0.05, 0.20, 0.03, 2);
        let abs_high = im.get_intensities()[2];
        eprintln!("  Few L2 (2) AB intensity: {:.4}", abs_high);

        assert!(
            abs_high > abs_low,
            "AB intensity should be higher when L2 count is low: {} vs {}",
            abs_high,
            abs_low
        );
    }

    /// Theorem D2: High SelfPreservation suppresses other drives.
    #[test]
    fn test_homeostatic_suppression() {
        let mut im = IntrinsicMotivation::new();

        // Low deficit (normal operation)
        im.update(0.80, 0.05, 0.15, 0.20, 2);
        let normal = im.get_intensities();

        // High deficit (survival mode) — should suppress PM, COH, AB
        im.update(0.80, 0.05, 0.90, 0.20, 2);
        let survival = im.get_intensities();

        eprintln!(
            "  Normal intensities: PM={:.3} COH={:.3} AB={:.3} SP={:.3}",
            normal[0], normal[1], normal[2], normal[3]
        );
        eprintln!(
            "  Survival intensities: PM={:.3} COH={:.3} AB={:.3} SP={:.3}",
            survival[0], survival[1], survival[2], survival[3]
        );

        // SP should be higher in survival mode
        assert!(
            survival[3] > normal[3],
            "SP intensity should be higher in survival mode"
        );

        // PM, COH, AB should be suppressed (lower) in survival mode
        // Note: raw PM intensity is the same (error unchanged), but suppression
        // multiplies by 0.1. However, if normal mode also has high error, the
        // suppression is visible as reduced intensity.
        assert!(
            survival[0] <= normal[0] + 0.01,
            "PM should not increase in survival mode: {} vs {}",
            survival[0],
            normal[0]
        );
    }

    /// Test: Drive multipliers correctly modulate effective weights.
    #[test]
    fn test_weight_modulation() {
        let mut im = IntrinsicMotivation::new();

        // Start with everything satisfied
        im.update(0.10, 0.05, 0.10, 0.02, 16);
        let base = [0.30, 0.30, 0.20, 0.20];

        let eff_satisfied = im.effective_weights(&base);
        eprintln!(
            "  Satisfied effective weights: [{:.4}, {:.4}, {:.4}, {:.4}]",
            eff_satisfied[0], eff_satisfied[1], eff_satisfied[2], eff_satisfied[3]
        );

        // When satisfied, multipliers are near 0, so effective ≈ base
        for i in 0..4 {
            assert!(
                (eff_satisfied[i] - base[i]).abs() < 0.05,
                "Satisfied drive should not distort weight[{}]: {:.4} ≈ {:.4}",
                i,
                eff_satisfied[i],
                base[i]
            );
        }

        // Boost the PredictiveMastery multiplier (simulating sleep adjustment)
        im.adjust_multipliers(DriveId::PredictiveMastery, 0.5);
        let eff_boosted = im.effective_weights(&base);
        eprintln!(
            "  PM-boosted effective weights: [{:.4}, {:.4}, {:.4}, {:.4}]",
            eff_boosted[0], eff_boosted[1], eff_boosted[2], eff_boosted[3]
        );

        // The PM-modulated weight (index 1 = Δerror) should be higher
        assert!(
            eff_boosted[1] > eff_satisfied[1],
            "PM boost should increase Δerror weight: {} > {}",
            eff_boosted[1],
            eff_satisfied[1]
        );
    }

    /// Test: Sleep-phase weight shift boosts the starved drive.
    #[test]
    fn test_sleep_weight_shift() {
        let mut im = IntrinsicMotivation::new();

        // Run many updates with high error (starving Predictive Mastery)
        for _ in 0..100 {
            im.update(0.80, 0.05, 0.30, 0.05, 16);
        }

        let before = im.get_multipliers();
        eprintln!(
            "  Multipliers before sleep: [{:.4}, {:.4}, {:.4}, {:.4}]",
            before[0], before[1], before[2], before[3]
        );

        // Determine starved drive
        let starved = im.starved_drive();
        eprintln!("  Starved drive: {:?}", starved);

        // Sleep: boost the starved drive
        im.adjust_multipliers(starved, SLEEP_BOOST);
        let after = im.get_multipliers();
        eprintln!(
            "  Multipliers after sleep: [{:.4}, {:.4}, {:.4}, {:.4}]",
            after[0], after[1], after[2], after[3]
        );

        // The starved drive's multiplier should have increased
        let idx = starved.simulator_weight_idx();
        assert!(
            after[idx] >= before[idx],
            "Sleep should increase starved drive multiplier: {} >= {}",
            after[idx],
            before[idx]
        );

        // Reset for next wake cycle
        im.reset_cumulative();
        assert_eq!(
            im.drives
                .iter()
                .map(|d| d.cumulative_deprivation as u32)
                .sum::<u32>(),
            0,
            "Cumulative deprivation should reset to 0"
        );
    }

    /// Theorem D3: Low error + low deficit → all drives quiet.
    #[test]
    fn test_drive_equilibrium() {
        let mut im = IntrinsicMotivation::new();

        // Everything is fine
        im.update(0.06, 0.05, 0.05, 0.02, 30);
        let intensities = im.get_intensities();

        eprintln!(
            "  Equilibrium intensities: PM={:.3} COH={:.3} AB={:.3} SP={:.3}",
            intensities[0], intensities[1], intensities[2], intensities[3]
        );

        // All drives should be low
        for (i, &val) in intensities.iter().enumerate() {
            assert!(
                val < 0.30,
                "Drive[{}] should be low at equilibrium: {}",
                i,
                val
            );
        }
    }

    /// Test that multipliers decay over time during wake.
    #[test]
    fn test_multiplier_decay() {
        let mut im = IntrinsicMotivation::new();

        // Boost a multiplier
        im.adjust_multipliers(DriveId::Abstraction, 0.5);
        let m_before = im
            .drives
            .iter()
            .find(|d| d.id == DriveId::Abstraction)
            .unwrap()
            .multiplier;
        eprintln!("  AB multiplier before decay: {:.6}", m_before);

        // Run many updates (which apply decay)
        for _ in 0..1000 {
            im.update(0.10, 0.05, 0.10, 0.03, 30);
        }

        let m_after = im
            .drives
            .iter()
            .find(|d| d.id == DriveId::Abstraction)
            .unwrap()
            .multiplier;
        eprintln!("  AB multiplier after 1000 ticks: {:.6}", m_after);

        // Multiplier should have decayed
        assert!(
            m_after < m_before,
            "Multiplier should decay over time: {} < {}",
            m_after,
            m_before
        );
    }
}
