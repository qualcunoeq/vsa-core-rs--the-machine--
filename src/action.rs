use crate::resonator::ResonatorVocabulary;
use crate::Hypervector;
use std::collections::HashMap;
use std::fs;
use std::process::Command;

/// Risk profile for an action, analogous to a financial beta.
///
/// * `base_cost` — nominal execution cost in a neutral environment
/// * `risk_beta` — sensitivity to environmental volatility / crisis proximity
///
/// | Action        | Base Cost | β   | Rationale                         |
/// |---------------|-----------|-----|-----------------------------------|
/// | `sys_read`    | 0.05      | 0.1 | Safe in any regime                |
/// | `sys_write`   | 0.10      | 0.5 | Moderate risk if system unstable  |
/// | `execute_bash`| 0.25      | 1.5 | High risk; penalised near crisis  |
#[derive(Clone, Debug)]
pub struct ActionProfile {
    pub vector: Hypervector,
    pub base_cost: f64,
    pub risk_beta: f64,
}

pub struct ActionRegistry {
    pub actions: HashMap<String, ActionProfile>,
}

impl ActionRegistry {
    pub fn new() -> Self {
        let mut reg = ActionRegistry {
            actions: HashMap::new(),
        };
        reg.actions.insert(
            "sys_read".to_string(),
            ActionProfile {
                vector: Hypervector::encode_text_ngram("sys_read", 3),
                base_cost: 0.05,
                risk_beta: 0.1,
            },
        );
        reg.actions.insert(
            "sys_write".to_string(),
            ActionProfile {
                vector: Hypervector::encode_text_ngram("sys_write", 3),
                base_cost: 0.10,
                risk_beta: 0.5,
            },
        );
        reg.actions.insert(
            "execute_bash".to_string(),
            ActionProfile {
                vector: Hypervector::encode_text_ngram("execute_bash", 3),
                base_cost: 0.25,
                risk_beta: 1.5,
            },
        );
        reg
    }

    pub fn get_action_vector(&self, name: &str) -> Option<&Hypervector> {
        self.actions.get(name).map(|p| &p.vector)
    }

    pub fn get_profile(&self, name: &str) -> Option<&ActionProfile> {
        self.actions.get(name)
    }

    /// Evaluates which action is encoded in the intent vector,
    /// returning the action name and the unbound parameter vector.
    pub fn decode_intent(
        &self,
        intent: &Hypervector,
        parameter_vocab: &ResonatorVocabulary,
    ) -> Option<(String, Hypervector)> {
        let mut best_action = None;
        let mut best_sim = -1.0;

        for (name, profile) in &self.actions {
            // Unbind the action from intent to estimate parameter: Param = Intent ^ H_action
            let param_estimate = intent.bitwise_xor(&profile.vector);
            // Cleanup check: verify if the parameter exists with high similarity in the vocabulary
            let (_, sim) = parameter_vocab.cleanup(&param_estimate);
            if sim > best_sim {
                best_sim = sim;
                best_action = Some(name.clone());
            }
        }

        if let Some(ref name) = best_action {
            let profile = self.actions.get(name).unwrap();
            let param = intent.bitwise_xor(&profile.vector);
            return Some((name.clone(), param));
        }
        None
    }
}

pub fn check_sandbox_safety(command: &str) -> bool {
    let cmd_lower = command.to_lowercase();
    // Safety guard to block hazardous shell patterns
    let blocked = vec![
        "rm ",
        "del ",
        "format ",
        "shred ",
        "mkfs ",
        "shutdown ",
        "reboot ",
        "dd ",
    ];
    for pattern in blocked {
        if cmd_lower.contains(pattern) {
            return false;
        }
    }
    true
}

pub fn execute_action(
    action_name: &str,
    param_vector: &Hypervector,
    vocab: &ResonatorVocabulary,
) -> Result<String, String> {
    // 1. Cleanup the parameter vector to extract the parameter string
    let (param_str, sim) = vocab.cleanup(param_vector);
    if sim < 0.40 {
        return Err(format!(
            "Parameter decoding failed. Similarity too low: {:.4}",
            sim
        ));
    }

    match action_name {
        "sys_read" => {
            // Safe read limited to workspace
            let path = std::path::Path::new(&param_str);
            if path.to_str().map(|s| s.contains("..")).unwrap_or(true) {
                return Err("Path traversal disallowed".to_string());
            }
            if !path.exists() {
                return Err(format!("File not found: {}", param_str));
            }
            fs::read_to_string(path).map_err(|e| format!("Read failed: {}", e))
        }
        "sys_write" => {
            // Write parameter string directly to a designated dynamic output file
            let target_path = "data/dynamic_output.txt";
            fs::write(target_path, &param_str).map_err(|e| format!("Write failed: {}", e))?;
            Ok(format!("Successfully wrote payload to {}", target_path))
        }
        "execute_bash" => {
            if !check_sandbox_safety(&param_str) {
                return Err("Action rejected by Sandbox Guard".to_string());
            }
            // Run a safe bash check command
            let output = if cfg!(target_os = "windows") {
                Command::new("cmd")
                    .args(&["/C", &param_str])
                    .output()
                    .map_err(|e| format!("Cmd run failed: {}", e))?
            } else {
                Command::new("sh")
                    .args(&["-c", &param_str])
                    .output()
                    .map_err(|e| format!("Shell run failed: {}", e))?
            };
            if output.status.success() {
                Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
            } else {
                Err(String::from_utf8_lossy(&output.stderr).trim().to_string())
            }
        }
        _ => Err(format!("Unknown action: {}", action_name)),
    }
}
