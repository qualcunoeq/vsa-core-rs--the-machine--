use std::sync::Arc;
use tokio::sync::RwLock;

#[derive(Clone)]
pub struct DefenseSystem {
    pub threat_level: Arc<RwLock<f64>>,
    pub active_port: Arc<RwLock<u16>>,
    pub stealth_mode: Arc<RwLock<bool>>,
    pub anxiety: Arc<RwLock<f64>>,
    /// ██ UPGRADE v2.0: Anxiety Inhibition (refractory period) ██
    /// After a pivot/DissonanceAlert, this counter counts down from
    /// INHIBITION_TICKS to 0.  During inhibition:
    ///   - Anxiety increases are dampened by 50%
    ///   - The vigilance threshold is fixed at 0.60 (midpoint)
    /// This prevents runaway feedback loops where anxiety → hypervigilance
    /// → more threat detection → more anxiety.
    pub inhibition_counter: Arc<RwLock<usize>>,
}

/// Number of ticks the inhibition period lasts after a pivot event.
pub const INHIBITION_TICKS: usize = 10;

impl DefenseSystem {
    pub fn new(initial_port: u16) -> Self {
        DefenseSystem {
            threat_level: Arc::new(RwLock::new(0.0)),
            active_port: Arc::new(RwLock::new(initial_port)),
            stealth_mode: Arc::new(RwLock::new(false)),
            anxiety: Arc::new(RwLock::new(0.0)),
            inhibition_counter: Arc::new(RwLock::new(0)),
        }
    }

    /// ██ UPGRADE v2.0: Inhibition-aware threat increment ██
    /// During inhibition, threat increases are dampened by 50%
    /// and the anxiety amplification factor is halved.
    pub async fn increment_threat(&self, amount: f64) {
        let anxiety_val = *self.anxiety.read().await;
        let inhibition = {
            let ic = self.inhibition_counter.read().await;
            *ic > 0
        };

        let dampener = if inhibition { 0.5 } else { 1.0 };
        let anxiety_amp = 1.0 + anxiety_val * dampener; // halved during inhibition
        let scaled_amount = amount * anxiety_amp * dampener;

        let mut level = self.threat_level.write().await;
        *level = (*level + scaled_amount).clamp(0.0, 1.0);
    }

    pub async fn decrement_threat(&self, amount: f64) {
        let mut level = self.threat_level.write().await;
        *level = (*level - amount).clamp(0.0, 1.0);
    }

    /// ██ UPGRADE v2.0: Inhibition-aware threat evaluation ██
    ///
    /// During inhibition:
    ///   - Vigilance threshold is fixed at 0.60 (preventing both
    ///     hypervigilance and complacency)
    ///   - Stealth mode deactivation threshold is also clamped
    ///
    /// After evaluating, decrements the inhibition counter.
    pub async fn evaluate_threat_response(&self) -> bool {
        let level = *self.threat_level.read().await;
        let anxiety_val = *self.anxiety.read().await;
        let mut inhibition = self.inhibition_counter.write().await;

        let (threshold, deact_threshold) = if *inhibition > 0 {
            // During inhibition: fixed mid-range thresholds
            *inhibition -= 1;
            (0.60, 0.25)
        } else {
            // Normal dynamic thresholds
            (0.8 - 0.4 * anxiety_val, 0.3 - 0.15 * anxiety_val)
        };

        if level >= threshold {
            let mut stealth = self.stealth_mode.write().await;
            if !*stealth {
                *stealth = true;
                let mut port = self.active_port.write().await;
                use rand::Rng;
                let new_port = rand::thread_rng().gen_range(9001..=9999);
                *port = new_port;

                // ██ Activate inhibition on pivot ██
                *inhibition = INHIBITION_TICKS;

                return true;
            }
        } else if level < deact_threshold {
            let mut stealth = self.stealth_mode.write().await;
            *stealth = false;
        }
        false
    }

    /// Trigger inhibition manually (e.g., from a DissonanceAlert).
    pub async fn trigger_inhibition(&self) {
        let mut inhibition = self.inhibition_counter.write().await;
        *inhibition = INHIBITION_TICKS;
    }

    /// Check whether the system is currently in the inhibition (refractory) period.
    pub async fn is_inhibited(&self) -> bool {
        let ic = self.inhibition_counter.read().await;
        *ic > 0
    }

    /// Energy gate: verify that executing an action is safe given the
    /// current cognitive state.
    ///
    /// Returns `Ok(())` if the action passes all gates:
    /// 1. Threat level is below critical threshold (or action is sys_read)
    /// 2. The parameter is not empty
    /// 3. Basic sandbox safety (delegates to `check_sandbox_safety`)
    ///
    /// Returns `Err(reason)` if any gate rejects the action.
    pub async fn check_action_safety(
        &self,
        action_name: &str,
        param_str: &str,
    ) -> Result<(), String> {
        let threat = *self.threat_level.read().await;
        let anxiety = *self.anxiety.read().await;
        let inhibited = self.is_inhibited().await;

        // Gate 1: sys_read is always safe (read-only)
        if action_name == "sys_read" {
            return Ok(());
        }

        // Gate 2: Empty parameters are rejected
        if param_str.is_empty() {
            return Err("Energy gate: empty parameter".to_string());
        }

        // Gate 3: Dynamic danger threshold (clamped to 0.60 during inhibition)
        let danger_threshold = if inhibited {
            0.60
        } else {
            0.8 - 0.4 * anxiety
        };
        if threat >= danger_threshold && action_name == "execute_bash" {
            return Err(format!(
                "Energy gate: threat={:.2} exceeds threshold={:.2}. Blocking shell execution.",
                threat, danger_threshold
            ));
        }

        // Gate 4: Sandbox safety
        if action_name == "execute_bash" && !crate::action::check_sandbox_safety(param_str) {
            return Err("Energy gate: blocked by sandbox guard".to_string());
        }

        Ok(())
    }

    /// Overwrites system telemetry files or temporary traces with random noise
    pub async fn scrub_traces(&self) {
        let temp_file_path = "data/ledger_temp.tmp";
        if std::path::Path::new(temp_file_path).exists() {
            use rand::Rng;
            let mut rng = rand::thread_rng();
            let mut noise = vec![0u8; 1024];
            rng.fill(&mut noise[..]);
            let _ = std::fs::write(temp_file_path, noise);
            let _ = std::fs::remove_file(temp_file_path);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_threat_evaluation_and_rotation() {
        let defense = DefenseSystem::new(9000);

        assert_eq!(*defense.threat_level.read().await, 0.0);
        assert_eq!(*defense.active_port.read().await, 9000);
        assert!(!*defense.stealth_mode.read().await);

        defense.increment_threat(0.5).await;
        assert_eq!(*defense.threat_level.read().await, 0.5);
        let rotation = defense.evaluate_threat_response().await;
        assert!(!rotation);
        assert!(!*defense.stealth_mode.read().await);

        defense.increment_threat(0.35).await;
        assert_eq!(*defense.threat_level.read().await, 0.85);
        let rotation = defense.evaluate_threat_response().await;
        assert!(rotation);
        assert!(*defense.stealth_mode.read().await);
        let new_port = *defense.active_port.read().await;
        assert!(new_port >= 9001 && new_port <= 9999);

        let rotation2 = defense.evaluate_threat_response().await;
        assert!(!rotation2);
    }

    #[tokio::test]
    async fn test_anxiety_threat_scaling() {
        let defense = DefenseSystem::new(9000);

        {
            let mut anxiety_guard = defense.anxiety.write().await;
            *anxiety_guard = 1.0;
        }

        defense.increment_threat(0.25).await;
        assert_eq!(*defense.threat_level.read().await, 0.50);

        let rotated = defense.evaluate_threat_response().await;
        assert!(rotated);
        assert!(*defense.stealth_mode.read().await);
    }

    #[tokio::test]
    async fn test_inhibition_dampens_threat() {
        let defense = DefenseSystem::new(9000);

        // Activate inhibition
        {
            let mut ic = defense.inhibition_counter.write().await;
            *ic = 5;
        }

        // During inhibition, threat increment should be dampened
        let initial = *defense.threat_level.read().await;
        defense.increment_threat(0.5).await;
        let after = *defense.threat_level.read().await;

        // Without inhibition: 0.5 * (1 + 0) = 0.5
        // With inhibition: 0.5 * (1 + 0*0.5) * 0.5 = 0.25
        assert!(
            (after - initial - 0.25).abs() < 0.001,
            "Inhibition should dampen threat increment: got {}",
            after - initial
        );
    }

    #[tokio::test]
    async fn test_inhibition_fixed_threshold() {
        let defense = DefenseSystem::new(9000);

        // Set anxiety very high and activate inhibition
        {
            let mut anxiety_guard = defense.anxiety.write().await;
            *anxiety_guard = 1.0;
        }
        {
            let mut ic = defense.inhibition_counter.write().await;
            *ic = 3;
        }

        // During inhibition, threshold should be 0.60 regardless of anxiety
        defense.increment_threat(0.55).await;
        // Threat should now be 0.55 (dampened: 0.55 * 0.5 * 0.5 = 0.1375)
        // That's well below 0.60, so no rotation yet
        let rotated = defense.evaluate_threat_response().await;
        assert!(!rotated, "Should not rotate at 0.1375 threat during inhibition");

        // Manually set threat to 0.65
        {
            let mut t = defense.threat_level.write().await;
            *t = 0.65;
        }
        let rotated2 = defense.evaluate_threat_response().await;
        assert!(rotated2, "Should rotate at 0.65 threat during inhibition (threshold=0.60)");
    }
}
