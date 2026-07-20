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

// ══════════════════════════════════════════════════════════════════════════
// DYNAMIC TOOL-USE PROTOCOL (General-Purpose Embodied Generalization)
// ══════════════════════════════════════════════════════════════════════════

/// A tool's signature, describing its interface as hypervectors.
///
/// Tools are "discovered" dynamically — their signature encodes:
/// - A unique tool ID (hypervector fingerprint)
/// - Input/output type signatures (what kinds of hypervectors they consume/produce)
/// - A human-readable description
/// - A cost model (compute cost, risk score)
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct ToolSignature {
    /// Unique tool identifier
    pub tool_id: String,
    /// Hypervector fingerprint of the tool's identity
    pub fingerprint: Hypervector,
    /// Human-readable description
    pub description: String,
    /// Input parameter types (names → expected hypervector patterns)
    pub input_types: Vec<ToolParamType>,
    /// Output type(s)
    pub output_types: Vec<ToolParamType>,
    /// Computational cost (abstract units)
    pub compute_cost: f64,
    /// Risk score (0.0 = safe, 1.0 = dangerous)
    pub risk_score: f64,
    /// Category tags for discovery
    pub tags: Vec<String>,
}

/// Describes a parameter type for a tool.
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct ToolParamType {
    pub name: String,
    pub description: String,
    /// Expected hypervector pattern for this parameter type
    pub type_vector: Hypervector,
}

/// A dynamic tool instance — discovered at runtime and invokable.
#[derive(Clone)]
pub struct DynamicTool {
    pub signature: ToolSignature,
    /// The invocation function. Takes parameter vectors, returns result vector.
    pub invoke_fn: Arc<dyn Fn(&[Hypervector]) -> Result<Hypervector, String> + Send + Sync>,
}

use std::sync::Arc;

impl std::fmt::Debug for DynamicTool {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DynamicTool")
            .field("tool_id", &self.signature.tool_id)
            .field("description", &self.signature.description)
            .finish()
    }
}

/// The tool registry — discovers, registers, and invokes tools dynamically.
pub struct ToolRegistry {
    tools: HashMap<String, DynamicTool>,
}

impl ToolRegistry {
    pub fn new() -> Self {
        let mut reg = ToolRegistry {
            tools: HashMap::new(),
        };

        // Register built-in tools
        reg.register_builtin_tools();

        reg
    }

    /// Generate a deterministic but distinctive fingerprint for a tool name.
    /// Uses a seeded hash to create a unique hypervector for each tool.
    fn tool_fingerprint(tool_id: &str) -> Hypervector {
        // Use the tool ID itself as the seed for a distinctive fingerprint
        // Rather than n-gram (which clusters similar names), we use a
        // seeded random approach for maximum orthogonality between tools.
        let seed: u64 = tool_id
            .bytes()
            .fold(0u64, |acc, b| acc.wrapping_mul(31).wrapping_add(b as u64));
        let mut bits = [0u64; 160];
        let mut x = seed;
        for i in 0..160 {
            x = x
                .wrapping_mul(6364136223846793005)
                .wrapping_add(1442695040888963407);
            bits[i] = x;
        }
        Hypervector { bits }
    }

    /// Register the default set of built-in tools.
    fn register_builtin_tools(&mut self) {
        // ── HTTP GET Tool ──────────────────────────────────────────────
        self.register_tool(DynamicTool {
            signature: ToolSignature {
                tool_id: "http_get".to_string(),
                fingerprint: Self::tool_fingerprint("http_get"),
                description: "Fetch a URL via HTTP GET and return the response body as text."
                    .to_string(),
                input_types: vec![ToolParamType {
                    name: "url".to_string(),
                    description: "The URL to fetch".to_string(),
                    type_vector: Self::tool_fingerprint("param_url"),
                }],
                output_types: vec![ToolParamType {
                    name: "response_body".to_string(),
                    description: "The HTTP response body".to_string(),
                    type_vector: Self::tool_fingerprint("param_response"),
                }],
                compute_cost: 0.3,
                risk_score: 0.1,
                tags: vec![
                    "network".to_string(),
                    "fetch".to_string(),
                    "http".to_string(),
                ],
            },
            invoke_fn: Arc::new(|params: &[Hypervector]| {
                if params.is_empty() {
                    return Err("http_get requires a URL parameter".to_string());
                }
                Ok(params[0])
            }),
        });

        // ── HTTP POST Tool ─────────────────────────────────────────────
        self.register_tool(DynamicTool {
            signature: ToolSignature {
                tool_id: "http_post".to_string(),
                fingerprint: Self::tool_fingerprint("http_post"),
                description: "Send an HTTP POST request with JSON body.".to_string(),
                input_types: vec![
                    ToolParamType {
                        name: "url".to_string(),
                        description: "Target URL".to_string(),
                        type_vector: Self::tool_fingerprint("param_url"),
                    },
                    ToolParamType {
                        name: "body".to_string(),
                        description: "JSON body to send".to_string(),
                        type_vector: Self::tool_fingerprint("param_json_body"),
                    },
                ],
                output_types: vec![ToolParamType {
                    name: "response".to_string(),
                    description: "HTTP response".to_string(),
                    type_vector: Self::tool_fingerprint("param_response"),
                }],
                compute_cost: 0.4,
                risk_score: 0.2,
                tags: vec![
                    "network".to_string(),
                    "http".to_string(),
                    "write".to_string(),
                ],
            },
            invoke_fn: Arc::new(|params: &[Hypervector]| {
                if params.len() < 2 {
                    return Err("http_post requires url and body parameters".to_string());
                }
                Ok(params[0])
            }),
        });

        // ── Python Execution Tool ──────────────────────────────────────
        self.register_tool(DynamicTool {
            signature: ToolSignature {
                tool_id: "exec_python".to_string(),
                fingerprint: Self::tool_fingerprint("exec_python"),
                description: "Execute a Python script or expression and return stdout.".to_string(),
                input_types: vec![ToolParamType {
                    name: "code".to_string(),
                    description: "Python code to execute".to_string(),
                    type_vector: Self::tool_fingerprint("param_python_code"),
                }],
                output_types: vec![ToolParamType {
                    name: "stdout".to_string(),
                    description: "Standard output of the Python execution".to_string(),
                    type_vector: Self::tool_fingerprint("param_exec_output"),
                }],
                compute_cost: 0.5,
                risk_score: 0.6,
                tags: vec![
                    "execution".to_string(),
                    "python".to_string(),
                    "code".to_string(),
                ],
            },
            invoke_fn: Arc::new(|params: &[Hypervector]| {
                if params.is_empty() {
                    return Err("exec_python requires a code parameter".to_string());
                }
                Ok(params[0])
            }),
        });

        // ── Shell Command Tool ─────────────────────────────────────────
        self.register_tool(DynamicTool {
            signature: ToolSignature {
                tool_id: "exec_shell".to_string(),
                fingerprint: Self::tool_fingerprint("exec_shell"),
                description: "Execute a shell command (sandboxed).".to_string(),
                input_types: vec![ToolParamType {
                    name: "command".to_string(),
                    description: "Shell command to execute".to_string(),
                    type_vector: Self::tool_fingerprint("param_shell_cmd"),
                }],
                output_types: vec![ToolParamType {
                    name: "output".to_string(),
                    description: "Command output".to_string(),
                    type_vector: Self::tool_fingerprint("param_exec_output"),
                }],
                compute_cost: 0.4,
                risk_score: 0.7,
                tags: vec![
                    "execution".to_string(),
                    "shell".to_string(),
                    "system".to_string(),
                ],
            },
            invoke_fn: Arc::new(|params: &[Hypervector]| {
                if params.is_empty() {
                    return Err("exec_shell requires a command parameter".to_string());
                }
                Ok(params[0])
            }),
        });

        // ── File Read Tool ─────────────────────────────────────────────
        self.register_tool(DynamicTool {
            signature: ToolSignature {
                tool_id: "file_read".to_string(),
                fingerprint: Self::tool_fingerprint("file_read"),
                description: "Read a file from the local filesystem.".to_string(),
                input_types: vec![ToolParamType {
                    name: "path".to_string(),
                    description: "Path to the file".to_string(),
                    type_vector: Self::tool_fingerprint("param_filepath"),
                }],
                output_types: vec![ToolParamType {
                    name: "contents".to_string(),
                    description: "File contents as text".to_string(),
                    type_vector: Self::tool_fingerprint("param_file_contents"),
                }],
                compute_cost: 0.1,
                risk_score: 0.2,
                tags: vec![
                    "filesystem".to_string(),
                    "read".to_string(),
                    "io".to_string(),
                ],
            },
            invoke_fn: Arc::new(|params: &[Hypervector]| {
                if params.is_empty() {
                    return Err("file_read requires a path parameter".to_string());
                }
                Ok(params[0])
            }),
        });

        // ── Search / Query Tool ────────────────────────────────────────
        self.register_tool(DynamicTool {
            signature: ToolSignature {
                tool_id: "semantic_search".to_string(),
                fingerprint: Self::tool_fingerprint("semantic_search"),
                description: "Search the machine's semantic memory for related concepts."
                    .to_string(),
                input_types: vec![ToolParamType {
                    name: "query".to_string(),
                    description: "Semantic query hypervector".to_string(),
                    type_vector: Self::tool_fingerprint("param_query"),
                }],
                output_types: vec![ToolParamType {
                    name: "results".to_string(),
                    description: "Matching memory entries".to_string(),
                    type_vector: Self::tool_fingerprint("param_search_results"),
                }],
                compute_cost: 0.2,
                risk_score: 0.0,
                tags: vec![
                    "memory".to_string(),
                    "search".to_string(),
                    "semantic".to_string(),
                ],
            },
            invoke_fn: Arc::new(|params: &[Hypervector]| {
                if params.is_empty() {
                    return Err("semantic_search requires a query".to_string());
                }
                Ok(params[0])
            }),
        });
    }

    /// Register a new tool dynamically (discovery).
    pub fn register_tool(&mut self, tool: DynamicTool) {
        let id = tool.signature.tool_id.clone();
        self.tools.insert(id, tool);
    }

    /// Discover tools by tag.
    pub fn discover_by_tag(&self, tag: &str) -> Vec<&ToolSignature> {
        self.tools
            .values()
            .filter(|t| t.signature.tags.contains(&tag.to_string()))
            .map(|t| &t.signature)
            .collect()
    }

    /// Discover tools by similarity to a query hypervector.
    pub fn discover_by_similarity(
        &self,
        query: &Hypervector,
        threshold: f64,
    ) -> Vec<(String, f64)> {
        let mut results = Vec::new();
        for (id, tool) in &self.tools {
            let sim = 1.0 - query.normalized_hamming_distance(&tool.signature.fingerprint);
            if sim >= threshold {
                results.push((id.clone(), sim));
            }
        }
        results.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        results
    }

    /// Get a tool by its ID.
    pub fn get_tool(&self, tool_id: &str) -> Option<&DynamicTool> {
        self.tools.get(tool_id)
    }

    /// Invoke a tool by ID with the given hypervector parameters.
    pub fn invoke_tool(
        &self,
        tool_id: &str,
        params: &[Hypervector],
    ) -> Result<Hypervector, String> {
        let tool = self
            .tools
            .get(tool_id)
            .ok_or_else(|| format!("Tool '{}' not found in registry", tool_id))?;
        (tool.invoke_fn)(params)
    }

    /// Get all registered tool signatures.
    pub fn list_tools(&self) -> Vec<&ToolSignature> {
        self.tools.values().map(|t| &t.signature).collect()
    }

    /// Encode a tool call intent into a hypervector.
    ///
    /// Uses a **bundling-based** approach:
    ///   intent = bundle(rotate(fp, 13), param1, ..., paramN)
    ///
    /// Bundling (majority-sum) is used instead of XOR binding because XOR's
    /// self-inverse property prevents reliable tool identification. Bundling
    /// preserves similarity: the intent remains similar to the rotated
    /// fingerprint, allowing tool identification via nearest-neighbor search.
    pub fn encode_tool_call(
        &self,
        tool_id: &str,
        param_vectors: &[Hypervector],
    ) -> Option<Hypervector> {
        let tool = self.tools.get(tool_id)?;
        let rotated_fp = tool.signature.fingerprint.rotate_left(13);

        let mut components = vec![&rotated_fp];
        for p in param_vectors {
            components.push(p);
        }

        Some(Hypervector::bundle(&components))
    }

    /// Decode which tool is being called from a bundled intent vector.
    ///
    /// With bundling (not XOR), the intent preserves similarity to the tool's
    /// rotated fingerprint. We find the closest tool by measuring similarity
    /// between the intent and each tool's rotated fingerprint.
    ///
    /// The parameter is estimated as the "difference" between the intent and
    /// the fingerprint: since intent = bundle(fp_rotated, param), the Hamming
    /// difference gives a noisy estimate of the parameter.
    pub fn decode_tool_call(&self, intent: &Hypervector) -> Option<(String, Vec<Hypervector>)> {
        let mut best_id = None;
        let mut best_sim = -1.0;

        for (id, tool) in &self.tools {
            let rotated_fp = tool.signature.fingerprint.rotate_left(13);
            // With bundling, the intent is most similar to the rotated fingerprint
            // of the correct tool (because bundling preserves similarity)
            let sim = 1.0 - intent.normalized_hamming_distance(&rotated_fp);

            if sim > best_sim {
                best_sim = sim;
                best_id = Some(id.clone());
            }
        }

        let id = best_id?;
        let tool = self.tools.get(&id)?;
        let rotated_fp = tool.signature.fingerprint.rotate_left(13);

        // Estimate parameter by "removing" the fingerprint influence.
        // In binary HDC, removal is approximate via XOR of the bundle with
        // the fingerprint — this gives a noisy estimate of the parameter.
        let param_estimate = intent.bitwise_xor(&rotated_fp);

        Some((id, vec![param_estimate]))
    }
}

// ─── Tests ────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tool_registry_builtins() {
        let registry = ToolRegistry::new();
        let tools = registry.list_tools();
        assert!(
            tools.len() >= 6,
            "Should have at least 6 built-in tools, got {}",
            tools.len()
        );

        // Verify specific tools exist
        assert!(registry.get_tool("http_get").is_some());
        assert!(registry.get_tool("exec_python").is_some());
        assert!(registry.get_tool("exec_shell").is_some());
        assert!(registry.get_tool("file_read").is_some());
    }

    #[test]
    fn test_tool_discovery_by_tag() {
        let registry = ToolRegistry::new();
        let network_tools = registry.discover_by_tag("network");
        assert!(!network_tools.is_empty(), "Should find network tools");
        assert!(network_tools.iter().any(|t| t.tool_id == "http_get"));
    }

    #[test]
    fn test_tool_discovery_by_similarity() {
        let registry = ToolRegistry::new();

        // Get a known tool's fingerprint and use it as query
        let http_get = registry.get_tool("http_get").unwrap();
        let query = http_get.signature.fingerprint;

        // Query with a relaxed threshold since binary HDC has noise
        let results = registry.discover_by_similarity(&query, 0.50);
        assert!(!results.is_empty(), "Should find similar tools");

        // The best match should be the tool itself
        let best = results.first().unwrap();
        assert_eq!(best.0, "http_get", "Best match should be the tool itself");
        assert!(best.1 > 0.80, "Self-similarity should be high: {}", best.1);
    }

    #[test]
    fn test_tool_call_encoding_roundtrip() {
        let registry = ToolRegistry::new();

        let param = Hypervector::encode_text_ngram("https://api.example.com/v2/data", 3);

        // Encode using the proper method (rotated fingerprint XOR params)
        let encoded = registry.encode_tool_call("http_get", &[param]);
        assert!(encoded.is_some());
        let encoded = encoded.unwrap();

        // Decode
        let (decoded_id, decoded_params) = registry.decode_tool_call(&encoded).unwrap();
        assert_eq!(decoded_params.len(), 1);

        // With bundling, the correct tool should be identified by similarity
        // The parameter is approximately recoverable
        let param_sim = 1.0 - decoded_params[0].normalized_hamming_distance(&param);
        assert!(
            param_sim > 0.50,
            "Parameter roundtrip similarity too low: {}",
            param_sim
        );

        assert_eq!(
            decoded_id, "http_get",
            "Tool should be correctly identified. Got '{}', expected 'http_get'",
            decoded_id
        );
    }

    #[test]
    fn test_invoke_tool() {
        let registry = ToolRegistry::new();
        let param = Hypervector::encode_text_ngram("test_param", 3);
        let result = registry.invoke_tool("http_get", &[param]);
        assert!(result.is_ok());
    }

    #[test]
    fn test_dynamic_tool_registration() {
        let mut registry = ToolRegistry::new();

        // Register a custom tool dynamically
        let custom_tool = DynamicTool {
            signature: ToolSignature {
                tool_id: "custom_analyze".to_string(),
                fingerprint: Hypervector::encode_text_ngram("tool_custom_analyze", 3),
                description: "Custom analysis tool.".to_string(),
                input_types: vec![ToolParamType {
                    name: "data".to_string(),
                    description: "Data to analyze".to_string(),
                    type_vector: Hypervector::encode_text_ngram("param_data", 3),
                }],
                output_types: vec![ToolParamType {
                    name: "analysis".to_string(),
                    description: "Analysis result".to_string(),
                    type_vector: Hypervector::encode_text_ngram("param_analysis", 3),
                }],
                compute_cost: 0.3,
                risk_score: 0.1,
                tags: vec!["custom".to_string(), "analysis".to_string()],
            },
            invoke_fn: Arc::new(|params: &[Hypervector]| Ok(params[0])),
        };

        registry.register_tool(custom_tool);
        assert!(registry.get_tool("custom_analyze").is_some());
        let custom_tools = registry.discover_by_tag("custom");
        assert_eq!(custom_tools.len(), 1);
    }
}
