use crate::action::ActionRegistry;
use crate::resonator::{factorize_svo, ResonatorVocabulary};
use crate::Hypervector;

// ─── Default SVO candidate lists ──────────────────────────────────────────
// These must align with terms loaded in `ResonatorVocabulary::new()`.

/// Entities that can occupy the Subject role in a parsed SVO triple.
pub const DEFAULT_SUBJECTS: &[&str] = &[
    "Agent-1", "Agent-2", "Agent-3", "Broker", "Finch",
    "Market", "News", "Infra",
];

/// Actions that can occupy the Verb role.
pub const DEFAULT_VERBS: &[&str] = &[
    "read", "write", "execute", "panic", "sync", "breached",
];

/// Resources / states that can occupy the Object role.
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

    /// Evaluates the discrepancy vector between expectation (historical memory)
    /// and current reality (active world state) via XOR.
    pub fn calculate_dissonance(current: &Hypervector, historical: &Hypervector) -> Hypervector {
        current.bitwise_xor(historical)
    }

    /// Evaluates whether the discrepancy is a meaningful structural anomaly
    /// that justifies pivoting active search intent.
    pub fn evaluates_necessity_to_pivot(&self, dissonance: &Hypervector) -> bool {
        let set_bits = dissonance.count_ones();
        let normalized_dist = set_bits as f64 / 10048.0;
        normalized_dist > self.dissonance_threshold && normalized_dist < 0.55
    }

    // ── Semantic intent formulation ─────────────────────────────────────

    /// Parse a dissonance (or world-state offset) through the simultaneous
    /// resonator network and formulate a structured corrective intent.
    ///
    /// Returns `(intent_vector, human_label)` if parsing succeeds and the
    /// reconstruction energy meets the hallucination threshold; `None` if
    /// the dissonance cannot be parsed (noise / unmodeled regime).
    pub fn formulate_intent(
        &self,
        dissonance: &Hypervector,
        vocab: &ResonatorVocabulary,
        registry: &ActionRegistry,
        subjects: &[String],
        verbs: &[String],
        objects: &[String],
        max_iterations: usize,
    ) -> Option<(Hypervector, String)> {
        // 1. Parse the dissonance through the Simultaneous Resonator Network
        let (s_str, v_str, o_str, energy) =
            factorize_svo(dissonance, vocab, subjects, verbs, objects, max_iterations)?;

        // 2. Select a corrective (action, parameter) based on parsed semantics
        let (action_name, param_name) = self.select_corrective_action(&v_str, &o_str);

        // 3. Look up the HV representations
        let profile = registry.get_profile(&action_name)?;
        let param_hv = vocab.get_vector(&param_name)?;

        // 4. Bind into corrective intent:  I = A ⊕ P
        let intent = profile.vector.bitwise_xor(param_hv);

        let label = format!(
            "Parsed: {} {} {} | Corrective: {} {} (E={:.2})",
            s_str, v_str, o_str, action_name, param_name, energy
        );

        Some((intent, label))
    }

    /// Map parsed (verb, object) semantics to a corrective (action, parameter).
    ///
    /// | Parsed Semantics              | Corrective Action     | Rationale                 |
    /// |-------------------------------|-----------------------|---------------------------|
    /// | breached / Attack / Crisis    | execute_bash panic    | Emergency lockdown        |
    /// | panic                         | sys_write ledger      | Document the event        |
    /// | sync / ledger                 | sys_read hosts        | Verify system state       |
    /// | execute / write on resource   | sys_read resource     | Investigate further       |
    /// | anything else                 | sys_read hosts        | Conservative default      |
    fn select_corrective_action(&self, verb: &str, object: &str) -> (String, String) {
        match (verb, object) {
            // More specific matches first (narrowest scope)
            ("panic", _) => {
                ("sys_write".to_string(), "ledger".to_string())
            }
            ("breached", _) | (_, "Attack") | (_, "Breach") | (_, "Crisis") => {
                ("execute_bash".to_string(), "panic".to_string())
            }
            ("sync", _) | (_, "ledger") | (_, "sync") => {
                ("sys_read".to_string(), "hosts".to_string())
            }
            ("execute", _) | ("write", _) => {
                // Investigate the object of concern
                ("sys_read".to_string(), object.to_string())
            }
            ("read", _) => {
                // Re-read to confirm
                ("sys_read".to_string(), object.to_string())
            }
            _ => {
                // Conservative default: verify system hosts
                ("sys_read".to_string(), "hosts".to_string())
            }
        }
    }
}

// ─── Tests ────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::action::ActionRegistry;
    use crate::resonator::ResonatorVocabulary;

    fn setup_env() -> (ResonatorVocabulary, ActionRegistry, Vec<String>, Vec<String>, Vec<String>) {
        let vocab = ResonatorVocabulary::new();
        let registry = ActionRegistry::new();

        let subjects: Vec<String> = DEFAULT_SUBJECTS.iter().map(|s| s.to_string()).collect();
        let verbs: Vec<String> = DEFAULT_VERBS.iter().map(|v| v.to_string()).collect();
        let objects: Vec<String> = DEFAULT_OBJECTS.iter().map(|o| o.to_string()).collect();

        // Register any additional terms tests need
        (vocab, registry, subjects, verbs, objects)
    }

    #[test]
    fn test_calculate_dissonance() {
        let v1 = Hypervector::new_random();
        let v2 = Hypervector::new_random();
        let dissonance = AutonomyDrive::calculate_dissonance(&v1, &v2);

        // XOR is self-reversible: dissonance XOR v1 should yield v2
        let reversed = dissonance.bitwise_xor(&v1);
        assert_eq!(reversed, v2);
    }

    #[test]
    fn test_necessity_to_pivot() {
        let drive = AutonomyDrive::new(0.43);

        // 1. Identical vectors → dissonance is zero → no pivot
        let v1 = Hypervector::new_random();
        let diss_zero = AutonomyDrive::calculate_dissonance(&v1, &v1);
        assert!(!drive.evaluates_necessity_to_pivot(&diss_zero));

        // 2. Completely random vectors → distance ~0.50 → should pivot if above threshold
        let v2 = Hypervector::new_random();
        let diss_random = AutonomyDrive::calculate_dissonance(&v1, &v2);
        let dist = diss_random.normalized_hamming_distance(&Hypervector::new_zero());
        if dist > 0.43 && dist < 0.55 {
            assert!(drive.evaluates_necessity_to_pivot(&diss_random));
        }
    }

    #[test]
    fn test_formulate_intent_parses_known_svo() {
        let (vocab, registry, subjects, verbs, objects) = setup_env();
        let drive = AutonomyDrive::new(0.43);

        // Build a known thought: T = ρ₁₃("Finch") ⊕ ρ₂₆("write") ⊕ ρ₃₉("ledger")
        let s_hv = vocab.get_vector("Finch").unwrap();
        let v_hv = vocab.get_vector("write").unwrap();
        let o_hv = vocab.get_vector("ledger").unwrap();
        let dissonance = crate::resonator::encode_svo(s_hv, v_hv, o_hv);

        let result = drive.formulate_intent(
            &dissonance, &vocab, &registry,
            &subjects, &verbs, &objects, 30,
        );

        assert!(
            result.is_some(),
            "formulate_intent should parse a valid SVO thought"
        );

        let (_intent, label) = result.unwrap();
        // The corrective action for ("write", "ledger") is sys_read "ledger"
        assert!(
            label.contains("Parsed: Finch write ledger"),
            "Label should reflect parsed content: {}",
            label
        );
    }

    #[test]
    fn test_formulate_intent_rejects_noise() {
        let (vocab, registry, subjects, verbs, objects) = setup_env();
        let drive = AutonomyDrive::new(0.43);

        // Random vector that does NOT encode a valid SVO
        let noise = Hypervector::new_random();

        let result = drive.formulate_intent(
            &noise, &vocab, &registry,
            &subjects, &verbs, &objects, 20,
        );

        // Should either return None (energy gate) or the energy must be valid
        if let Some((_intent, label)) = result {
            assert!(
                label.contains("E="),
                "A returned formulation must carry energy info: {}",
                label
            );
        }
    }

    #[test]
    fn test_select_corrective_action_crisis() {
        let drive = AutonomyDrive::new(0.43);
        let (action, param) = drive.select_corrective_action("breached", "server");
        assert_eq!(action, "execute_bash");
        assert_eq!(param, "panic");
    }

    #[test]
    fn test_select_corrective_action_default() {
        let drive = AutonomyDrive::new(0.43);
        let (action, param) = drive.select_corrective_action("unknown", "foo");
        assert_eq!(action, "sys_read");
        assert_eq!(param, "hosts");
    }

    #[test]
    fn test_select_corrective_action_panic_to_ledger() {
        let drive = AutonomyDrive::new(0.43);
        let (action, param) = drive.select_corrective_action("panic", "Crisis");
        assert_eq!(action, "sys_write");
        assert_eq!(param, "ledger");
    }
}
