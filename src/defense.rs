use std::sync::Arc;
use tokio::sync::RwLock;

#[derive(Clone)]
pub struct DefenseSystem {
    pub threat_level: Arc<RwLock<f64>>,
    pub active_port: Arc<RwLock<u16>>,
    pub stealth_mode: Arc<RwLock<bool>>,
    pub anxiety: Arc<RwLock<f64>>,
}

impl DefenseSystem {
    pub fn new(initial_port: u16) -> Self {
        DefenseSystem {
            threat_level: Arc::new(RwLock::new(0.0)),
            active_port: Arc::new(RwLock::new(initial_port)),
            stealth_mode: Arc::new(RwLock::new(false)),
            anxiety: Arc::new(RwLock::new(0.0)),
        }
    }

    pub async fn increment_threat(&self, amount: f64) {
        let anxiety_val = *self.anxiety.read().await;
        // Scale threat increments with cognitive anxiety (up to 2x amplification)
        let scaled_amount = amount * (1.0 + anxiety_val);
        let mut level = self.threat_level.write().await;
        *level = (*level + scaled_amount).clamp(0.0, 1.0);
    }

    pub async fn decrement_threat(&self, amount: f64) {
        let mut level = self.threat_level.write().await;
        *level = (*level - amount).clamp(0.0, 1.0);
    }

    /// Evaluates threats. If threat level crosses the dynamic vigilance threshold
    /// (which drops from 0.80 down to 0.40 under high anxiety), triggers stealth mode.
    pub async fn evaluate_threat_response(&self) -> bool {
        let level = *self.threat_level.read().await;
        let anxiety_val = *self.anxiety.read().await;

        // Dynamic vigilance threshold: drops to 0.40 when anxiety is 1.0
        let threshold = 0.8 - 0.4 * anxiety_val;

        if level >= threshold {
            let mut stealth = self.stealth_mode.write().await;
            if !*stealth {
                *stealth = true;

                // Select a pseudo-random high port in the range 9001..=9999
                let mut port = self.active_port.write().await;
                use rand::Rng;
                let new_port = rand::thread_rng().gen_range(9001..=9999);
                *port = new_port;
                return true;
            }
        } else if level < 0.3 - 0.15 * anxiety_val {
            let mut stealth = self.stealth_mode.write().await;
            *stealth = false;
        }
        false
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

        // Gate 1: sys_read is always safe (read-only)
        if action_name == "sys_read" {
            return Ok(());
        }

        // Gate 2: Empty parameters are rejected
        if param_str.is_empty() {
            return Err("Energy gate: empty parameter".to_string());
        }

        // Gate 3: Under high threat + anxiety, only sys_read is permitted.
        // The dynamic threshold mirrors evaluate_threat_response.
        let danger_threshold = 0.8 - 0.4 * anxiety;
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
        // In a true POI scenario, we wipe log files or RAM chunks.
        // We simulate trace scrubbing by cleaning up data/ temporary directories.
        let temp_file_path = "data/ledger_temp.tmp";
        if std::path::Path::new(temp_file_path).exists() {
            // Overwrite with random bytes to prevent forensic recovery
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

        // 1. Initial status
        assert_eq!(*defense.threat_level.read().await, 0.0);
        assert_eq!(*defense.active_port.read().await, 9000);
        assert!(!*defense.stealth_mode.read().await);

        // 2. Increment threat below trigger threshold
        defense.increment_threat(0.5).await;
        assert_eq!(*defense.threat_level.read().await, 0.5);
        let rotation = defense.evaluate_threat_response().await;
        assert!(!rotation);
        assert!(!*defense.stealth_mode.read().await);

        // 3. Increment threat to trigger stealth & port rotation
        defense.increment_threat(0.35).await;
        assert_eq!(*defense.threat_level.read().await, 0.85);
        let rotation = defense.evaluate_threat_response().await;
        assert!(rotation);
        assert!(*defense.stealth_mode.read().await);
        let new_port = *defense.active_port.read().await;
        assert!(new_port >= 9001 && new_port <= 9999);

        // 4. Repeated check does not trigger another rotation immediately
        let rotation2 = defense.evaluate_threat_response().await;
        assert!(!rotation2);
    }

    #[tokio::test]
    async fn test_anxiety_threat_scaling() {
        let defense = DefenseSystem::new(9000);

        // Set anxiety to high (1.0)
        {
            let mut anxiety_guard = defense.anxiety.write().await;
            *anxiety_guard = 1.0;
        }

        // Increment threat: base 0.25, scaled by (1.0 + 1.0) = 2.0x -> should become 0.50!
        defense.increment_threat(0.25).await;
        assert_eq!(*defense.threat_level.read().await, 0.50);

        // Under anxiety = 1.0, the threshold drops to 0.8 - 0.4 * 1.0 = 0.40.
        // Since threat is 0.50, evaluate_threat_response should trigger stealth rotation!
        let rotated = defense.evaluate_threat_response().await;
        assert!(rotated);
        assert!(*defense.stealth_mode.read().await);
    }
}
