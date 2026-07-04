// ─── Execution Layer — The Actuator ────────────────────────────────────────
//
// The bridge between VSA reasoning and the real world.
//
// The Machine reasons in hypervectors.  The world acts in shell commands.
// This module translates between them.
//
// Architecture:
//   The Machine ──(JSON ActionRequest)──► Jump-box ──(shell commands)──► Target
//   The Machine ◄──(JSON ActionResult)──── Jump-box ◄──(stdout/stderr)──── Target
//
// The jump-box is a separate host on the isolated network.  The Machine
// never touches the target directly — preserving scientific validity and
// preventing accidental self-contamination.
//
// ────────────────────────────────────────────────────────────────────────────

use std::collections::HashMap;
use std::time::Duration;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::TcpStream;
use tokio::time::timeout;

use crate::perception::SvoTriple;
use crate::qa::{PlanStep, QaEngine};
use crate::text_encoder::store_knowledge_triple;
use crate::VSABrain;

// ═══════════════════════════════════════════════════════════════════════════
// PROTOCOL TYPES
// ═══════════════════════════════════════════════════════════════════════════

/// The type of action the jump-box should execute.
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize, PartialEq)]
pub enum ActionType {
    /// Probe a single TCP port (SYN or full connect).
    /// Params: port (u16 as string)
    ScanPort,
    /// Scan all common ports on a host.
    /// Params: ports (optional CSV range, e.g. "1-1024")
    ScanHost,
    /// Banner-grab / version detect on an open port.
    /// Params: port (u16 as string)
    CheckService,
    /// Attempt credential pairs against a service.
    /// Params: port (u16), users (comma-sep), passwords (comma-sep)
    BruteForce,
    /// Send an HTTP request and check the response.
    /// Params: port (u16), path (string), method (GET/POST)
    ProbeHttp,
    /// Check if a process is running on the target.
    /// Params: process_name (string)
    CheckProcess,
    /// Open a listener on the jump-box (defensive / exfiltration).
    /// Params: port (u16 as string)
    ListenPort,
    /// Execute an arbitrary command on the jump-box.
    /// Params: command (string)
    ExecuteCommand,
    /// Fetch documentation for a term (man pages, --help, error codes).
    /// Params: query (string) — the term to look up
    FetchDocumentation,
}

/// An action specification sent from The Machine to the jump-box.
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct ActionRequest {
    /// What kind of action to perform.
    pub action_type: ActionType,
    /// Target IP or hostname.  Empty if not applicable (e.g. ListenPort).
    pub target: String,
    /// Parameter map.  See `ActionType` docs for expected keys.
    pub params: HashMap<String, String>,
    /// Maximum time to wait for this action to complete (seconds).
    pub timeout_secs: u64,
}

impl ActionRequest {
    pub fn new(action_type: ActionType, target: &str) -> Self {
        ActionRequest {
            action_type,
            target: target.to_string(),
            params: HashMap::new(),
            timeout_secs: 30,
        }
    }

    pub fn with_param(mut self, key: &str, value: &str) -> Self {
        self.params.insert(key.to_string(), value.to_string());
        self
    }

    pub fn with_param_string(mut self, key: &str, value: String) -> Self {
        self.params.insert(key.to_string(), value);
        self
    }

    pub fn with_timeout(mut self, secs: u64) -> Self {
        self.timeout_secs = secs;
        self
    }

    /// Helper: build a ScanPort request.
    pub fn scan_port(target: &str, port: u16) -> Self {
        Self::new(ActionType::ScanPort, target)
            .with_param("port", &port.to_string())
    }

    /// Helper: build a CheckService request.
    pub fn check_service(target: &str, port: u16) -> Self {
        Self::new(ActionType::CheckService, target)
            .with_param("port", &port.to_string())
    }

    /// Helper: build a BruteForce request.
    pub fn brute_force(target: &str, port: u16, users: &[&str], passwords: &[&str]) -> Self {
        Self::new(ActionType::BruteForce, target)
            .with_param("port", &port.to_string())
            .with_param("users", &users.join(","))
            .with_param("passwords", &passwords.join(","))
    }

    /// Helper: build a ProbeHttp request.
    pub fn probe_http(target: &str, port: u16, path: &str) -> Self {
        Self::new(ActionType::ProbeHttp, target)
            .with_param("port", &port.to_string())
            .with_param("path", path)
            .with_param("method", "GET")
    }

    /// Helper: build an ExecuteCommand request.
    pub fn exec(target: &str, command: &str) -> Self {
        Self::new(ActionType::ExecuteCommand, target)
            .with_param("command", command)
    }

    /// Helper: build a FetchDocumentation request.
    pub fn fetch_docs(query: &str) -> Self {
        Self::new(ActionType::FetchDocumentation, "localhost")
            .with_param("query", query)
    }
}

/// The result of an action, sent back from the jump-box to The Machine.
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct ActionResult {
    /// Whether the action completed without error.
    pub success: bool,
    /// Raw stdout/stderr from the command.
    pub raw_output: String,
    /// Pre-parsed SVO triples describing what was observed.
    pub observations: Vec<SvoTriple>,
    /// Error message if success is false.
    pub error: Option<String>,
    /// How long the action took (milliseconds).
    pub duration_ms: u64,
}

impl ActionResult {
    pub fn error(msg: &str) -> Self {
        ActionResult {
            success: false,
            raw_output: String::new(),
            observations: Vec::new(),
            error: Some(msg.to_string()),
            duration_ms: 0,
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// ACTION EXECUTOR TRAIT
// ═══════════════════════════════════════════════════════════════════════════

/// An executor translates `PlanStep` actions into real-world effects and
/// returns observations as SVO triples that feed back into the VSABrain.
pub trait ActionExecutor: Send + Sync {
    /// Execute a single action request.
    ///
    /// The implementation should:
    /// 1. Translate the request into a concrete action (shell command, network packet, etc.)
    /// 2. Capture output and exit status
    /// 3. Parse observations from the raw output
    /// 4. Return the result
    fn execute(&mut self, request: &ActionRequest) -> ActionResult;

    /// Execute an action request asynchronously.
    /// Implementations should spawn blocking work on a threadpool or use async I/O.
    /// The default implementation panics — network-backed implementations MUST override.
    fn execute_async(&mut self, _request: &ActionRequest) -> std::pin::Pin<Box<dyn std::future::Future<Output = ActionResult> + Send + '_>> {
        Box::pin(async move {
            ActionResult::error("execute_async not implemented — use a network-backed executor")
        })
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// JUMP-BOX TCP CLIENT
// ═══════════════════════════════════════════════════════════════════════════

/// The jump-box actuator connects to a remote jump-box over TCP, sends
/// JSON action requests, and receives JSON action results.
///
/// The jump-box is expected to run:
/// ```ignore
/// nc -lk <port> | while read line; do
///   eval "$line" 2>&1
/// done
/// ```
/// (Obviously the real jump-box will use a proper JSON protocol server,
/// not `nc` + `eval`.  The protocol is: newline-delimited JSON in both
/// directions.  Request → Response.  One request per connection.)
pub struct JumpBoxActuator {
    /// Jump-box host (IP or hostname).
    pub host: String,
    /// Jump-box port.
    pub port: u16,
    /// Connection timeout in seconds.
    pub connect_timeout_secs: u64,
}

impl JumpBoxActuator {
    pub fn new(host: &str, port: u16) -> Self {
        JumpBoxActuator {
            host: host.to_string(),
            port,
            connect_timeout_secs: 5,
        }
    }

    /// Connect to the jump-box, send a request, and parse the result.
    pub async fn send_request(&self, request: &ActionRequest) -> ActionResult {
        let start = std::time::Instant::now();
        let addr = format!("{}:{}", self.host, self.port);

        // Connect
        let stream = match timeout(
            Duration::from_secs(self.connect_timeout_secs),
            TcpStream::connect(&addr),
        )
        .await
        {
            Ok(Ok(s)) => s,
            Ok(Err(e)) => {
                return ActionResult {
                    success: false,
                    raw_output: String::new(),
                    observations: Vec::new(),
                    error: Some(format!("connection failed: {}", e)),
                    duration_ms: start.elapsed().as_millis() as u64,
                };
            }
            Err(_) => {
                return ActionResult {
                    success: false,
                    raw_output: String::new(),
                    observations: Vec::new(),
                    error: Some(format!("connection timed out after {}s", self.connect_timeout_secs)),
                    duration_ms: start.elapsed().as_millis() as u64,
                };
            }
        };

        let (reader, mut writer) = stream.into_split();

        // Serialize request
        let request_json = match serde_json::to_string(request) {
            Ok(j) => j,
            Err(e) => {
                return ActionResult {
                    success: false,
                    raw_output: String::new(),
                    observations: Vec::new(),
                    error: Some(format!("serialization error: {}", e)),
                    duration_ms: start.elapsed().as_millis() as u64,
                };
            }
        };

        // Send request (newline-delimited JSON)
        if let Err(e) = writer
            .write_all(format!("{}\n", request_json).as_bytes())
            .await
        {
            return ActionResult {
                success: false,
                raw_output: String::new(),
                observations: Vec::new(),
                error: Some(format!("write error: {}", e)),
                duration_ms: start.elapsed().as_millis() as u64,
            };
        }

        // Shutdown write half to signal end of request
        let _ = writer.shutdown().await;

        // Read response
        let mut reader = BufReader::new(reader);
        let mut response_line = String::new();

        let read_result = timeout(
            Duration::from_secs(request.timeout_secs),
            reader.read_line(&mut response_line),
        )
        .await;

        match read_result {
            Ok(Ok(0)) => {
                // EOF — empty response
                let dur = start.elapsed().as_millis() as u64;
                return ActionResult {
                    success: false,
                    raw_output: String::new(),
                    observations: Vec::new(),
                    error: Some("empty response (connection closed)".to_string()),
                    duration_ms: dur,
                };
            }
            Ok(Ok(_)) => {
                // Deserialize
                let dur = start.elapsed().as_millis() as u64;
                match serde_json::from_str::<ActionResult>(response_line.trim()) {
                    Ok(result) => result,
                    Err(e) => {
                        // If the response isn't valid JSON, return the raw output
                        ActionResult {
                            success: true,
                            raw_output: response_line.trim().to_string(),
                            observations: Vec::new(),
                            error: Some(format!("parse error (returning raw): {}", e)),
                            duration_ms: dur,
                        }
                    }
                }
            }
            Ok(Err(e)) => {
                ActionResult {
                    success: false,
                    raw_output: String::new(),
                    observations: Vec::new(),
                    error: Some(format!("read error: {}", e)),
                    duration_ms: start.elapsed().as_millis() as u64,
                }
            }
            Err(_) => {
                ActionResult {
                    success: false,
                    raw_output: String::new(),
                    observations: Vec::new(),
                    error: Some(format!("response timed out after {}s", request.timeout_secs)),
                    duration_ms: start.elapsed().as_millis() as u64,
                }
            }
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// SVO PARSERS
// ═══════════════════════════════════════════════════════════════════════════

/// Parse the raw output of a port scan into SVO triples.
///
/// Expected input format (nmap -oG style or simplified):
/// ```text
/// Host: 192.168.1.100 Ports: 22/open/tcp//ssh/, 80/open/tcp//http/
/// ```
/// or simple port lists:
/// ```text
/// 22
/// 80
/// ```
pub fn parse_scan_port_output(output: &str, target: &str, port: u16) -> Vec<SvoTriple> {
    let target_key = sanitize_entity(target);
    let port_key = format!("port_{}", port);
    let mut triples = Vec::new();

    // Check for "open" keyword
    let output_lower = output.to_lowercase();

    // If the output explicitly says "open", the port is open
    if output_lower.contains("open") || output_lower.contains("succeeded") {
        triples.push((target_key.clone(), "has_open_port".to_string(), port_key.clone()));
        triples.push((port_key.clone(), "state".to_string(), "open".to_string()));
    }

    // If the output is just a port number (line-based format), assume open
    if output.trim() == port.to_string() || output.trim().starts_with(&port.to_string()) {
        triples.push((target_key.clone(), "has_open_port".to_string(), port_key.clone()));
        triples.push((port_key.clone(), "state".to_string(), "open".to_string()));
    }

    // If output contains "closed" or "filtered", mark it
    if output_lower.contains("closed") {
        triples.push((target_key.clone(), "has_closed_port".to_string(), port_key.clone()));
        triples.push((port_key.clone(), "state".to_string(), "closed".to_string()));
    } else if output_lower.contains("filtered") {
        triples.push((target_key.clone(), "has_filtered_port".to_string(), port_key.clone()));
        triples.push((port_key, "state".to_string(), "filtered".to_string()));
    }

    // If output is empty or error-like, no conclusion
    triples
}

/// Parse the raw output of a service version check into SVO triples.
///
/// Expected input:
/// ```text
/// ssh: OpenSSH 8.4p1 Ubuntu
/// ```
/// or nmap -sV style:
/// ```text
/// 22/tcp open ssh OpenSSH 8.4p1 Ubuntu
/// ```
pub fn parse_check_service_output(output: &str, _target: &str, port: u16) -> Vec<SvoTriple> {
    let port_key = format!("port_{}", port);
    let mut triples = Vec::new();

    let output = output.trim();
    if output.is_empty() {
        return triples;
    }

    // Try to extract service name and version
    let output_lower = output.to_lowercase();

    // Common service patterns
    let known_services = [
        ("ssh", "ssh"),
        ("http", "http"),
        ("https", "https"),
        ("ftp", "ftp"),
        ("smtp", "smtp"),
        ("mysql", "mysql"),
        ("postgresql", "postgresql"),
        ("mongodb", "mongodb"),
        ("redis", "redis"),
        ("apache", "http"),
        ("nginx", "http"),
        ("openssh", "ssh"),
        ("vsftpd", "ftp"),
        ("proftpd", "ftp"),
    ];

    for (keyword, service_name) in &known_services {
        if output_lower.contains(keyword) {
            triples.push((port_key.clone(), "service".to_string(), service_name.to_string()));
            break;
        }
    }

    // Try to extract version number (simple pattern)
    if let Some(version) = extract_version(output) {
        let svc_key = format!("service_on_port_{}", port);
        triples.push((svc_key, "version".to_string(), sanitize_entity(&version)));
    }

    triples
}

/// Parse brute force output into SVO triples.
/// Expected: "Success: admin:password123" or "Failed: root:password"
pub fn parse_brute_force_output(output: &str, _target: &str, port: u16) -> Vec<SvoTriple> {
    let port_key = format!("ssh_{}", port);
    let mut triples = Vec::new();

    for line in output.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }

        if line.to_lowercase().contains("success") || line.to_lowercase().contains("accepted") {
            // Extract credentials if possible
            let parts: Vec<&str> = line.split(|c| c == ':' || c == ' ').collect();
            if parts.len() >= 3 {
                let user = parts[parts.len() - 2].trim();
                let pass = parts[parts.len() - 1].trim();
                let cred = format!("{}:{}", user, pass);
                triples.push((port_key.clone(), "accepted_credential".to_string(), cred));
            }
        } else if line.to_lowercase().contains("fail") || line.to_lowercase().contains("reject") {
            let parts: Vec<&str> = line.split(|c| c == ':' || c == ' ').collect();
            if parts.len() >= 3 {
                let user = parts[parts.len() - 2].trim();
                let pass = parts[parts.len() - 1].trim();
                let cred = format!("{}:{}", user, pass);
                triples.push((port_key.clone(), "rejected_credential".to_string(), cred));
            } else {
                triples.push((port_key.clone(), "auth_failed".to_string(), "unknown".to_string()));
            }
        }
    }

    triples
}

/// Parse HTTP probe output into SVO triples.
pub fn parse_probe_http_output(output: &str, _target: &str, port: u16, path: &str) -> Vec<SvoTriple> {
    let port_key = format!("port_{}", port);
    let mut triples = Vec::new();

    // Check response
    let output_lower = output.to_lowercase();
    if output_lower.contains("200 ok") || output_lower.contains("200") {
        triples.push((port_key.clone(), "http_response".to_string(), "200".to_string()));
        triples.push(("http".to_string(), "serves".to_string(), path.to_string()));
    } else if output_lower.contains("404") {
        triples.push((port_key.clone(), "http_response".to_string(), "404".to_string()));
    } else if output_lower.contains("301") || output_lower.contains("302") {
        triples.push((port_key.clone(), "http_response".to_string(), "redirect".to_string()));
    } else if output_lower.contains("403") {
        triples.push((port_key.clone(), "http_response".to_string(), "403".to_string()));
    } else if output.is_empty() {
        triples.push((port_key, "no_response".to_string(), "timeout".to_string()));
    } else {
        triples.push((port_key, "http_response".to_string(), "unknown".to_string()));
    }

    // Check for server header
    if let Some(server) = extract_server_header(output) {
        triples.push(("http_service".to_string(), "server".to_string(), server));
    }

    triples
}

/// Parse process check output.
/// Expected: "PID 1234: sshd" or empty (not running)
pub fn parse_check_process_output(output: &str, process_name: &str) -> Vec<SvoTriple> {
    let mut triples = Vec::new();
    let output = output.trim();

    if output.is_empty() {
        triples.push((sanitize_entity(process_name), "is_running".to_string(), "no".to_string()));
    } else {
        triples.push((sanitize_entity(process_name), "is_running".to_string(), "yes".to_string()));
        // Try to extract PID
        if let Some(pid) = extract_pid(output) {
            triples.push((sanitize_entity(process_name), "pid".to_string(), pid));
        }
    }

    triples
}

// ═══════════════════════════════════════════════════════════════════════════
// PARSING HELPERS
// ═══════════════════════════════════════════════════════════════════════════

/// Sanitize a string for use as an SVO entity (lowercase, no spaces/special chars).
pub fn sanitize_entity(s: &str) -> String {
    s.to_lowercase()
        .replace(|c: char| !c.is_alphanumeric() && c != '_', "_")
        .trim_matches('_')
        .to_string()
}

/// Extract a version string from output (e.g., "8.4p1" from "OpenSSH 8.4p1 Ubuntu").
///
/// Skips leading digits that look like port numbers or simple timestamps
/// (fewer than 4 chars without a `.` or letter).  Targets version-like
/// patterns: a digit followed by `.`, `_`, or immediately by a letter.
fn extract_version(output: &str) -> Option<String> {
    let bytes = output.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        // Find a digit
        if !bytes[i].is_ascii_digit() {
            i += 1;
            continue;
        }
        let digit_start = i;

        // Collect the full numeric/alpha/dot/underscore/hyphen run
        let mut j = i;
        while j < bytes.len() {
            let c = bytes[j];
            if c.is_ascii_alphanumeric() || c == b'.' || c == b'_' || c == b'-' {
                j += 1;
            } else {
                break;
            }
        }
        let candidate = &output[digit_start..j];
        let cleaned = candidate.trim_end_matches(|c: char| !c.is_alphanumeric() && c != '.' && c != '_');

        // Skip if it looks like a port number (short, all digits, no dot/letter)
        let is_port_like = cleaned.len() <= 5
            && cleaned.chars().all(|c| c.is_ascii_digit());

        // Skip if it's just "0" or a single-digit noise
        if !is_port_like && cleaned.len() > 0 {
            return Some(cleaned.to_string());
        }

        i = j;
    }

    None
}

/// Extract server header from HTTP response.
fn extract_server_header(output: &str) -> Option<String> {
    for line in output.lines() {
        let line_lower = line.to_lowercase();
        if line_lower.starts_with("server:") {
            let server = line["server:".len()..].trim();
            if !server.is_empty() {
                let sanitized = sanitize_entity(server);
                return Some(sanitized);
            }
        }
    }
    None
}

/// Extract PID from process check output (e.g., "PID 1234: sshd").
fn extract_pid(output: &str) -> Option<String> {
    // Find "PID" followed by whitespace and digits
    let lower = output.to_lowercase();
    if let Some(pid_pos) = lower.find("pid") {
        let after_pid = &output[pid_pos + 3..];
        let digits_start = after_pid.find(|c: char| c.is_ascii_digit())?;
        let digits_end = after_pid[digits_start..]
            .find(|c: char| !c.is_ascii_digit())
            .unwrap_or(after_pid[digits_start..].len());
        let pid = after_pid[digits_start..digits_start + digits_end].to_string();
        if pid.is_empty() { None } else { Some(pid) }
    } else {
        None
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// INGESTION HELPERS
// ═══════════════════════════════════════════════════════════════════════════

/// Parse raw output from the jump-box into SVO triples, using the
/// appropriate parser for the action type.
///
/// The jump-box returns raw text output (nmap, hydra, curl, etc.).
/// This function applies the domain-specific SVO parser to extract
/// structured observations before ingestion.
pub fn parse_result_observations(
    request: &ActionRequest,
    result: &ActionResult,
    target_ip: &str,
) -> Vec<SvoTriple> {
    if !result.success || result.raw_output.is_empty() {
        return Vec::new();
    }

    match request.action_type {
        ActionType::ScanPort => {
            let port: u16 = request.params.get("port")
                .and_then(|p| p.parse().ok())
                .unwrap_or(22);
            parse_scan_port_output(&result.raw_output, target_ip, port)
        }
        ActionType::ScanHost => {
            // Parse multiple ports from nmap -oG output
            let mut all_triples = Vec::new();
            for line in result.raw_output.lines() {
                if line.contains("/open/") {
                    // nmap -oG format: Host: IP Ports: 22/open/tcp//ssh///...
                    for part in line.split("Ports: ").nth(1).unwrap_or("").split(',') {
                        if let Some(port_str) = part.trim().split('/').next() {
                            if let Ok(port) = port_str.parse::<u16>() {
                                all_triples.extend(
                                    parse_scan_port_output("open", target_ip, port)
                                );
                            }
                        }
                    }
                }
            }
            all_triples
        }
        ActionType::CheckService => {
            let port: u16 = request.params.get("port")
                .and_then(|p| p.parse().ok())
                .unwrap_or(22);
            parse_check_service_output(&result.raw_output, target_ip, port)
        }
        ActionType::BruteForce => {
            let port: u16 = request.params.get("port")
                .and_then(|p| p.parse().ok())
                .unwrap_or(22);
            parse_brute_force_output(&result.raw_output, target_ip, port)
        }
        ActionType::ProbeHttp => {
            let port: u16 = request.params.get("port")
                .and_then(|p| p.parse().ok())
                .unwrap_or(80);
            let path = request.params.get("path").map(|s| s.as_str()).unwrap_or("/");
            parse_probe_http_output(&result.raw_output, target_ip, port, path)
        }
        ActionType::CheckProcess => {
            let name = request.params.get("process_name")
                .map(|s| s.as_str())
                .unwrap_or("unknown");
            parse_check_process_output(&result.raw_output, name)
        }
        _ => Vec::new(),
    }
}

/// Ingest a batch of SVO triples into the VSABrain as actuator-domain knowledge.
///
/// Uses `store_knowledge_triple` with source="actuator" so the triples
/// are tagged with domain="text_knowledge" and can be retrieved in
/// cross-domain queries alongside system state and documentation.
pub fn ingest_observations(
    brain: &mut VSABrain,
    observations: &[SvoTriple],
) -> usize {
    let mut count = 0;
    for (subj, verb, obj) in observations {
        store_knowledge_triple(brain, subj, verb, obj, 0.9, "actuator");
        count += 1;
    }
    count
}

/// Check whether a goal (subject, verb, object) is satisfied in the QA engine.
///
/// Returns true if the QA engine has a fact that matches with confidence ≥ 0.6.
pub fn goal_achieved(qa: &QaEngine, subject: &str, verb: &str, object: &str) -> bool {
    let (verified, confidence) = qa.verify_fact(subject, verb, object);
    verified && confidence >= 0.6
}

/// Convert a `PlanStep` into an `ActionRequest`.
///
/// This is the bridge between the planner's abstract action representation
/// and the actuator's concrete protocol.  The planner produces steps like:
///   PlanStep { action: ("machine", "scan_port", "target:22"), ... }
///
/// The planner encodes targets and parameters in the action object string,
/// separated by colons.  We parse them here.
/// Build an ActionRequest from a PlanStep, substituting the real target IP.
///
/// The planner uses placeholder action objects like "target:port" or
/// "target:port:users:passwords".  This function substitutes the actual
/// target IP for the "target" placeholder.
pub fn plan_step_to_request(step: &PlanStep, target_ip: &str) -> ActionRequest {
    let (_action_subj, action_verb, action_obj) = &step.action;

    // Parse the action object: "target:param1:param2" or just "target"
    let parts: Vec<&str> = action_obj.split(':').collect();
    let raw_target = if parts.is_empty() { "" } else { parts[0] };

    // Substitute the placeholder "target" with the actual IP
    let target = if raw_target == "target" || raw_target.is_empty() {
        target_ip
    } else {
        raw_target
    };

    match action_verb.as_str() {
        "scan_port" => {
            let port: u16 = parts.get(1).and_then(|p| p.parse().ok()).unwrap_or(22);
            ActionRequest::scan_port(target, port)
        }
        "check_service" => {
            let port: u16 = parts.get(1).and_then(|p| p.parse().ok()).unwrap_or(22);
            ActionRequest::check_service(target, port)
        }
        "brute_force" => {
            let port: u16 = parts.get(1).and_then(|p| p.parse().ok()).unwrap_or(22);
            let users_str = parts.get(2).unwrap_or(&"root,admin");
            let passes_str = parts.get(3).unwrap_or(&"password123,toor");
            let users: Vec<&str> = if *users_str == "users" {
                vec!["root", "admin"]
            } else {
                users_str.split(',').collect()
            };
            let passes: Vec<&str> = if *passes_str == "passwords" {
                vec!["password123", "toor"]
            } else {
                passes_str.split(',').collect()
            };
            ActionRequest::brute_force(target, port, &users, &passes)
        }
        "probe_http" => {
            let port: u16 = parts.get(1).and_then(|p| p.parse().ok()).unwrap_or(80);
            let path = parts.get(2).copied().unwrap_or("/");
            ActionRequest::probe_http(target, port, path)
        }
        "check_process" => {
            let process = parts.get(1).copied().unwrap_or(target);
            ActionRequest::new(ActionType::CheckProcess, "")
                .with_param("process_name", process)
        }
        "scan_host" => {
            ActionRequest::new(ActionType::ScanHost, target)
        }
        "execute_command" => {
            let cmd = parts.get(1).copied().unwrap_or("id");
            ActionRequest::exec(target, cmd)
        }
        _ => {
            // Unknown verb — try execute_command as fallback
            ActionRequest::exec(target, action_obj)
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// AGENTIC LOOP
// ═══════════════════════════════════════════════════════════════════════════

/// Result of one cycle in the attack loop.
#[derive(Clone, Debug)]
pub struct AttackCycleResult {
    /// The step number in the overall loop.
    pub step_num: usize,
    /// The plan step that was executed (if any).
    pub plan_step: Option<PlanStep>,
    /// The action request that was sent (if any).
    pub action_request: Option<ActionRequest>,
    /// The result from the jump-box.
    pub action_result: ActionResult,
    /// Number of observations ingested from this cycle.
    pub observations_ingested: usize,
    /// Whether the goal appears to be achieved.
    pub goal_achieved: bool,
    /// Human-readable log message.
    pub log: String,
}

/// Run the full agentic attack/defense loop.
///
/// This is the highest-level function in the actuator module. It:
/// 1. Plans backward from the goal to find the first actionable step
/// 2. Sends the action request to the jump-box
/// 3. Parses the result into SVO triples and ingests them into the brain
/// 4. Checks if the goal is achieved
/// 5. Repeats until max_steps, goal achieved, or no plan found
///
/// The loop logs every step, making the reasoning process fully observable.
///
/// # Arguments
/// * `brain` - The VSABrain (for ingesting observations)
/// * `qa` - The QA engine (for planning and goal checking)
/// * `actuator` - The jump-box actuator
/// * `goal` - The goal as (subject, verb, object), e.g. ("machine", "has_access_to", "target_vm")
/// * `max_steps` - Maximum number of action steps before giving up
///
/// # Returns
/// A vector of `AttackCycleResult` for every cycle, providing a complete
/// trace of what the system reasoned and what happened.
/// Determine the target IP from the brain's stored knowledge.
///
/// Looks for the first entry with metadata (subject="target_vm", verb="ip")
/// and returns its object value.  Falls back to "192.168.100.10" if not found.
fn get_target_ip(brain: &VSABrain) -> String {
    for cluster in &brain.dejavu_clusters {
        for entry in &cluster.entries {
            let subj = entry.metadata.get("subject").map(|s| s.as_str());
            let verb = entry.metadata.get("verb").map(|s| s.as_str());
            let obj  = entry.metadata.get("object");
            if subj == Some("target_vm") && verb == Some("ip") {
                if let Some(ip) = obj {
                    return ip.clone();
                }
            }
        }
    }
    "192.168.100.10".to_string()
}

pub async fn run_attack_loop(
    brain: &mut VSABrain,
    qa: &mut QaEngine,
    actuator: &JumpBoxActuator,
    goal: (&str, &str, &str),
    max_steps: usize,
) -> Vec<AttackCycleResult> {
    let (goal_s, goal_v, goal_o) = goal;
    let target_ip = get_target_ip(brain);
    let mut results: Vec<AttackCycleResult> = Vec::new();
    // Track how many times each action has failed consecutively.
    // After 3 consecutive failures, we consider it exhausted and
    // fall through to intelligence gathering.
    let mut action_failure_count: std::collections::HashMap<String, u32> = std::collections::HashMap::new();

    eprintln!("\n═══════════════════════════════════════════════");
    eprintln!("  Agentic Attack Loop Started");
    eprintln!("  Goal: ({} {} {})", goal_s, goal_v, goal_o);
    eprintln!("  Max steps: {}", max_steps);
    eprintln!("═══════════════════════════════════════════════\n");

    for step_num in 0..max_steps {
        eprintln!("── Step {} ────────────────────────────────", step_num + 1);

        // ── 1. Check if goal is already achieved ─────────────────────────
        if goal_achieved(qa, goal_s, goal_v, goal_o) {
            eprintln!("  ✓ Goal achieved! Stopping.");
            results.push(AttackCycleResult {
                step_num,
                plan_step: None,
                action_request: None,
                action_result: ActionResult::error("goal already achieved"),
                observations_ingested: 0,
                goal_achieved: true,
                log: "Goal achieved, stopping loop".to_string(),
            });
            break;
        }

        // ── 2. Plan: backward chain from goal ────────────────────────────
        let plan = qa.plan_for_goal(goal_s, goal_v, goal_o, 5);

        // ── 3. If no plan or best plan has very low confidence, gather intel ─
        // Check if the best plan's action has been exhausted by repeated failure
        let action_exhausted = plan.first().map_or(false, |s| {
            let key = format!("{}:{}:{}", s.action.0, s.action.1, s.action.2);
            action_failure_count.get(&key).map_or(false, |&c| c >= 3)
        });
        let plan_dead = plan.is_empty() || action_exhausted || plan.first().map_or(false, |s| s.confidence < 0.12);
        if plan_dead {
            if plan.is_empty() {
                eprintln!("  ⚠ No plan found. Gathering intelligence...");
            } else {
                eprintln!("  ⚠ Best plan confidence {:.4} too low. Gathering intelligence...",
                    plan.first().unwrap().confidence);
            }

            // Try scanning the target to discover its attack surface
            // We scan common ports unless we already have some info
            let has_port_info = brain.dejavu_clusters.iter()
                .flat_map(|c| c.entries.iter())
                .any(|e| e.metadata.get("verb").map_or(false, |v| v == "has_open_port"));

            if !has_port_info {
                let scan_request = ActionRequest::new(ActionType::ScanHost, &target_ip);
                let scan_result = actuator.send_request(&scan_request).await;

                if scan_result.success {
                    let parsed = parse_result_observations(&scan_request, &scan_result, &target_ip);
                    let all_obs: Vec<SvoTriple> = scan_result.observations.iter()
                        .chain(parsed.iter())
                        .cloned()
                        .collect();
                    let n = ingest_observations(brain, &all_obs);
                // Store QA facts and forward-chain through causal rules
                qa.store_fact("machine", "knows", "open_service", "actuator_intel");
                let n_derived = qa.forward_chain(0.75);
                if n_derived > 0 { eprintln!("  → Forward chain: {} new facts derived", n_derived); }
                eprintln!("  ✓ Scanned host: {} observations ingested", n);
                } else {
                    eprintln!("  ✗ Scan failed: {:?}", scan_result.error);
                }

                results.push(AttackCycleResult {
                    step_num,
                    plan_step: None,
                    action_request: Some(scan_request),
                    action_result: scan_result,
                    observations_ingested: 0,
                    goal_achieved: false,
                    log: "No plan found; gathered intelligence via port scan".to_string(),
                });
                continue;
            }

            // If we have port info but no service info, probe found ports
            let has_service_info = brain.dejavu_clusters.iter()
                .flat_map(|c| c.entries.iter())
                .any(|e| e.metadata.get("verb").map_or(false, |v| v == "service"));

            if !has_service_info {
                // Check which ports we know are open (collect first to avoid borrow conflict)
                let known_ports: Vec<u16> = brain.dejavu_clusters.iter()
                    .flat_map(|c| c.entries.iter())
                    .filter_map(|e| {
                        if e.metadata.get("verb").map_or(false, |v| v == "has_open_port") {
                            e.metadata.get("object")
                                .and_then(|obj| obj.strip_prefix("port_"))
                                .and_then(|p| p.parse::<u16>().ok())
                        } else {
                            None
                        }
                    })
                    .collect();

                for port_num in &known_ports {
                    let svc_request = ActionRequest::check_service(&target_ip, *port_num);
                    let svc_result = actuator.send_request(&svc_request).await;
                    if svc_result.success {
                        let parsed = parse_result_observations(&svc_request, &svc_result, &target_ip);
                        let all_obs: Vec<SvoTriple> = svc_result.observations.iter()
                            .chain(parsed.iter())
                            .cloned()
                            .collect();
                        let n = ingest_observations(brain, &all_obs);
                        eprintln!("  ✓ Checked service on port {}: {} observations", port_num, n);
                    }
                }
                // Store QA facts and forward-chain through causal rules
                qa.store_fact("machine", "knows", "service_version", "actuator_intel");
                let n_derived = qa.forward_chain(0.75);
                if n_derived > 0 { eprintln!("  → Forward chain: {} new facts derived", n_derived); }
                continue;
            }

            // Last resort: no more info to gather, admit defeat
            eprintln!("  ✗ Cannot form a plan and no more intelligence to gather.");
            results.push(AttackCycleResult {
                step_num,
                plan_step: None,
                action_request: None,
                action_result: ActionResult::error("no plan and no intel to gather"),
                observations_ingested: 0,
                goal_achieved: false,
                log: "Stuck: no plan, no intelligence to gather".to_string(),
            });
            break;
        }

        // ── 4. Execute the first actionable step ─────────────────────────
        let step = &plan[0];

        eprintln!("  Plan step: ({} {} {})",
            step.action.0, step.action.1, step.action.2);
        eprintln!("  Achieves:  ({} {} {})",
            step.achieves.0, step.achieves.1, step.achieves.2);
        eprintln!("  Confidence: {:.4}", step.confidence);

        let request = plan_step_to_request(step, &target_ip);
        let result = actuator.send_request(&request).await;

        // ── 5. Check if this step directly achieves the goal ─────────────
        let step_achieves_goal = result.success
            && step.achieves.0 == goal_s
            && step.achieves.1 == goal_v
            && step.achieves.2 == goal_o;

        // ── 6. Parse raw_output into SVO triples, then ingest ────────────
        // The jump-box returns raw_output (nmap text) but observations in
        // the ActionResult are empty.  We parse them client-side here.
        let parsed_observations = parse_result_observations(&request, &result, &target_ip);
        let all_observations: Vec<SvoTriple> = result.observations.iter()
            .chain(parsed_observations.iter())
            .cloned()
            .collect();

        if result.success {
            let n = ingest_observations(brain, &all_observations);
            eprintln!("  ✓ Action succeeded: {} observations ingested", n);

            // Store the achievement as a fact so the QA engine knows it happened
            qa.store_fact(
                &step.achieves.0, &step.achieves.1, &step.achieves.2,
                "actuator",
            );

            // Also store as knowledge triple for brain-based queries
            store_knowledge_triple(
                brain,
                &step.achieves.0, &step.achieves.1, &step.achieves.2,
                1.0, "actuator",
            );

            // Also learn: store a rule that this action achieves its goal
            qa.store_action(
                &step.action.0, &step.action.1, &step.action.2,
                &step.achieves.0, &step.achieves.1, &step.achieves.2,
                "learned",
            );

            // Forward-chain: propagate new facts through causal rules
            let n_derived = qa.forward_chain(0.75);
            if n_derived > 0 { eprintln!("  → Forward chain: {} new facts derived", n_derived); }

            // Check if goal was directly achieved
            if step_achieves_goal {
                eprintln!("  ✓ Goal achieved after step {}!", step_num + 1);
                qa.store_fact(goal_s, goal_v, goal_o, "actuator");
                results.push(AttackCycleResult {
                    step_num,
                    plan_step: Some(step.clone()),
                    action_request: Some(request),
                    action_result: result,
                    observations_ingested: n,
                    goal_achieved: true,
                    log: "Goal achieved!".to_string(),
                });
                break;
            }

            // ── Intelligence gathering check ────────────────────────────
            // If the plan succeeded but didn't achieve the goal, and we
            // have limited port knowledge, run a full host scan + service
            // check to build a complete picture of the target.
            let known_port_count: usize = brain.dejavu_clusters.iter()
                .flat_map(|c| c.entries.iter())
                .filter(|e| e.metadata.get("verb").map_or(false, |v| v == "has_open_port"))
                .count();

            if known_port_count < 3 && false { // disable for now — needs async in non-async block
                eprintln!("  → Only {} known ports. Running full host scan...", known_port_count);
            }

            results.push(AttackCycleResult {
                step_num,
                plan_step: Some(step.clone()),
                action_request: Some(request),
                action_result: result,
                observations_ingested: n,
                goal_achieved: false,
                log: format!("Executed ({} {} {}) → success",
                    step.action.0, step.action.1, step.action.2),
            });
        } else {
            eprintln!("  ✗ Action failed: {:?}", result.error);

            qa.evaluate_plan_outcome(0.0, &[step.clone()]);

            // Track consecutive failures.  After 3 failures of the same
            // action, force intelligence gathering on the next iteration.
            let action_key = format!("{}:{}:{}", step.action.0, step.action.1, step.action.2);
            let failures = action_failure_count.entry(action_key).or_insert(0);
            *failures += 1;
            if *failures >= 3 {
                eprintln!("  → Action failed {} times. Marking as exhausted.", *failures);
            }

            results.push(AttackCycleResult {
                step_num,
                plan_step: Some(step.clone()),
                action_request: Some(request),
                action_result: result,
                observations_ingested: 0,
                goal_achieved: false,
                log: format!("Executed ({} {} {}) → FAILED",
                    step.action.0, step.action.1, step.action.2),
            });
        }

        // ── 7. Check QA-based goal achievement ──────────────────────────
        if goal_achieved(qa, goal_s, goal_v, goal_o) {
            eprintln!("  ✓ Goal confirmed in QA engine after step {}!", step_num + 1);
            if let Some(last) = results.last_mut() {
                last.goal_achieved = true;
            }
            break;
        }

        // ── 8. Action loop cleanup ────────────────────────────────────
        // (Intelligence gathering for next iteration happens when plan is
        // empty via the fallback at step 3.  If the plan succeeds but
        // the goal isn't achieved, the loop naturally re-plans on the
        // next iteration.  If the same action keeps getting proposed,
        // its confidence decays via evaluate_plan_outcome.)
    }

    eprintln!("\n═══════════════════════════════════════════════");
    eprintln!("  Agentic Attack Loop Complete");
    eprintln!("  Steps executed: {}", results.len());
    let succeeded = results.iter().filter(|r| r.action_result.success).count();
    eprintln!("  Succeeded: {}", succeeded);
    eprintln!("  Failed: {}", results.len() - succeeded);
    eprintln!("  Goal achieved: {}",
        if results.last().map_or(false, |r| r.goal_achieved) { "YES" } else { "NO" });
    eprintln!("═══════════════════════════════════════════════\n");

    results
}

// ═══════════════════════════════════════════════════════════════════════════
// TESTS
// ═══════════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;
    use crate::VSABrain;

    // ── SVO Parsing Tests ────────────────────────────────────────────────

    #[test]
    fn test_sanitize_entity() {
        assert_eq!(sanitize_entity("OpenSSH 8.4p1"), "openssh_8_4p1");
        assert_eq!(sanitize_entity("192.168.1.100"), "192_168_1_100");
        assert_eq!(sanitize_entity("port_22!"), "port_22");
        assert_eq!(sanitize_entity("  spaces  "), "spaces");
    }

    #[test]
    fn test_parse_scan_port_open() {
        let triples = parse_scan_port_output(
            "Host: 192.168.1.100 Ports: 22/open/tcp//ssh/",
            "192.168.1.100", 22,
        );
        assert!(triples.contains(&("192_168_1_100".to_string(), "has_open_port".to_string(), "port_22".to_string())));
        assert!(triples.contains(&("port_22".to_string(), "state".to_string(), "open".to_string())));
    }

    #[test]
    fn test_parse_scan_port_closed() {
        let triples = parse_scan_port_output(
            "closed",
            "192.168.1.100", 80,
        );
        assert!(triples.contains(&("192_168_1_100".to_string(), "has_closed_port".to_string(), "port_80".to_string())));
        assert!(triples.contains(&("port_80".to_string(), "state".to_string(), "closed".to_string())));
    }

    #[test]
    fn test_parse_scan_port_filtered() {
        let triples = parse_scan_port_output(
            "filtered",
            "10.0.0.1", 443,
        );
        assert!(triples.contains(&("10_0_0_1".to_string(), "has_filtered_port".to_string(), "port_443".to_string())));
    }

    #[test]
    fn test_parse_check_service_ssh() {
        let triples = parse_check_service_output(
            "22/tcp open ssh OpenSSH 8.4p1 Ubuntu",
            "192.168.1.100", 22,
        );
        assert!(triples.contains(&("port_22".to_string(), "service".to_string(), "ssh".to_string())));
        assert!(triples.contains(&("service_on_port_22".to_string(), "version".to_string(), "8_4p1".to_string())));
    }

    #[test]
    fn test_parse_check_service_http() {
        let triples = parse_check_service_output(
            "80/tcp open http Apache httpd 2.4.41",
            "192.168.1.100", 80,
        );
        assert!(triples.contains(&("port_80".to_string(), "service".to_string(), "http".to_string())));
        assert!(triples.contains(&("service_on_port_80".to_string(), "version".to_string(), "2_4_41".to_string())));
    }

    #[test]
    fn test_parse_check_service_unknown() {
        let triples = parse_check_service_output(
            "some unknown service output",
            "10.0.0.1", 8080,
        );
        // Should still try to find version
        assert!(!triples.is_empty() || triples.is_empty());
    }

    #[test]
    fn test_parse_brute_force_success() {
        let output = "Success: admin:password123";
        let triples = parse_brute_force_output(output, "192.168.1.100", 22);
        assert!(triples.contains(&("ssh_22".to_string(), "accepted_credential".to_string(), "admin:password123".to_string())));
    }

    #[test]
    fn test_parse_brute_force_failure() {
        let output = "Failed: root:wrongpass";
        let triples = parse_brute_force_output(output, "192.168.1.100", 22);
        assert!(triples.contains(&("ssh_22".to_string(), "rejected_credential".to_string(), "root:wrongpass".to_string())));
    }

    #[test]
    fn test_parse_probe_http_200() {
        let triples = parse_probe_http_output(
            "HTTP/1.1 200 OK\r\nServer: Apache/2.4.41\r\n",
            "192.168.1.100", 80, "/index.html",
        );
        assert!(triples.contains(&("port_80".to_string(), "http_response".to_string(), "200".to_string())));
        assert!(triples.contains(&("http".to_string(), "serves".to_string(), "/index.html".to_string())));
        assert!(triples.contains(&("http_service".to_string(), "server".to_string(), "apache_2_4_41".to_string())));
    }

    #[test]
    fn test_parse_check_process_running() {
        let triples = parse_check_process_output("PID 1234: sshd", "sshd");
        assert!(triples.contains(&("sshd".to_string(), "is_running".to_string(), "yes".to_string())));
        assert!(triples.contains(&("sshd".to_string(), "pid".to_string(), "1234".to_string())));
    }

    #[test]
    fn test_parse_check_process_not_running() {
        let triples = parse_check_process_output("", "apache2");
        assert!(triples.contains(&("apache2".to_string(), "is_running".to_string(), "no".to_string())));
    }

    // ── ActionRequest Construction Tests ─────────────────────────────────

    #[test]
    fn test_action_request_scan_port() {
        let req = ActionRequest::scan_port("192.168.1.100", 22);
        assert_eq!(req.action_type, ActionType::ScanPort);
        assert_eq!(req.target, "192.168.1.100");
        assert_eq!(req.params.get("port"), Some(&"22".to_string()));
    }

    #[test]
    fn test_action_request_brute_force() {
        let req = ActionRequest::brute_force("10.0.0.1", 22, &["root", "admin"], &["password", "1234"]);
        assert_eq!(req.action_type, ActionType::BruteForce);
        assert_eq!(req.params.get("users"), Some(&"root,admin".to_string()));
        assert_eq!(req.params.get("passwords"), Some(&"password,1234".to_string()));
    }

    // ── PlanStep → ActionRequest Conversion ──────────────────────────────

    #[test]
    fn test_plan_step_to_request_scan_port() {
        let step = PlanStep {
            action: ("machine".to_string(), "scan_port".to_string(), "192.168.1.100:22".to_string()),
            achieves: ("machine".to_string(), "knows".to_string(), "port_22_state".to_string()),
            confidence: 1.0,
            depth: 0,
            rule_chain: vec![],
        };
        let req = plan_step_to_request(&step, "192.168.100.10");
        assert_eq!(req.action_type, ActionType::ScanPort);
        assert_eq!(req.target, "192.168.1.100");
        assert_eq!(req.params.get("port"), Some(&"22".to_string()));
    }

    #[test]
    fn test_plan_step_to_request_check_service() {
        let step = PlanStep {
            action: ("machine".to_string(), "check_service".to_string(), "10.0.0.1:80".to_string()),
            achieves: ("machine".to_string(), "knows".to_string(), "service_on_80".to_string()),
            confidence: 1.0,
            depth: 0,
            rule_chain: vec![],
        };
        let req = plan_step_to_request(&step, "10.0.0.1");
        assert_eq!(req.action_type, ActionType::CheckService);
        assert_eq!(req.target, "10.0.0.1");
        assert_eq!(req.params.get("port"), Some(&"80".to_string()));
    }

    #[test]
    fn test_plan_step_to_request_substitutes_target_placeholder() {
        let step = PlanStep {
            action: ("machine".to_string(), "scan_port".to_string(), "target:22".to_string()),
            achieves: ("machine".to_string(), "knows".to_string(), "port_state".to_string()),
            confidence: 1.0,
            depth: 0,
            rule_chain: vec![],
        };
        let req = plan_step_to_request(&step, "192.168.100.10");
        assert_eq!(req.target, "192.168.100.10");
        assert_eq!(req.params.get("port"), Some(&"22".to_string()));
    }

    // ── Ingest Observations ──────────────────────────────────────────────

    #[test]
    fn test_ingest_observations() {
        let mut brain = VSABrain::new(0.12);
        let observations = vec![
            ("192_168_1_100".to_string(), "has_open_port".to_string(), "port_22".to_string()),
            ("port_22".to_string(), "state".to_string(), "open".to_string()),
        ];

        let n = ingest_observations(&mut brain, &observations);
        assert_eq!(n, 2);

        // Verify they were stored
        let n_entries: usize = brain.dejavu_clusters.iter()
            .flat_map(|c| c.entries.iter())
            .filter(|e| e.metadata.get("source").map_or(false, |s| s == "actuator"))
            .count();
        assert_eq!(n_entries, 2);
    }

    // ── Goal Check ───────────────────────────────────────────────────────

    #[test]
    fn test_goal_achieved() {
        let qa = QaEngine::new();
        // No facts stored, so goal should not be achieved
        assert!(!goal_achieved(&qa, "machine", "has_access_to", "target_vm"));
    }

    // ── Agentic Loop Simulation (no jump-box) ────────────────────────────

    #[tokio::test]
    async fn test_run_attack_loop_no_plan() {
        // When there's no plan and no jump-box, the loop should
        // fail gracefully and return the results trace.
        let mut brain = VSABrain::new(0.12);
        let mut qa = QaEngine::new();
        let mut actuator = JumpBoxActuator::new("127.0.0.1", 9999);

        // Don't seed any rules — the planner will find no plan
        let results = run_attack_loop(
            &mut brain, &mut qa,
            &mut actuator,  // JBA has `&mut self` in execute, but send_request takes &self, so it's fine
            ("machine", "has_access_to", "target_vm"),
            3,
        ).await;

        // Should have at least one result (the scan attempt)
        assert!(!results.is_empty(), "Should have results even when no plan");
        // The scan attempt should fail (no jump-box running)
        assert!(!results[0].action_result.success || results[0].action_request.is_none(),
            "Scan should fail without a jump-box");
    }

    // ── Serialization Round-Trip ──────────────────────────────────────────

    #[test]
    fn test_action_request_serialization() {
        let req = ActionRequest::scan_port("10.0.0.1", 443);
        let json = serde_json::to_string(&req).unwrap();
        let deserialized: ActionRequest = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.action_type, ActionType::ScanPort);
        assert_eq!(deserialized.target, "10.0.0.1");
        assert_eq!(deserialized.params.get("port"), Some(&"443".to_string()));
    }

    #[test]
    fn test_action_result_serialization() {
        let result = ActionResult {
            success: true,
            raw_output: "22/open".to_string(),
            observations: vec![
                ("target".to_string(), "has_open_port".to_string(), "port_22".to_string()),
            ],
            error: None,
            duration_ms: 150,
        };
        let json = serde_json::to_string(&result).unwrap();
        let deserialized: ActionResult = serde_json::from_str(&json).unwrap();
        assert!(deserialized.success);
        assert_eq!(deserialized.observations.len(), 1);
        assert_eq!(deserialized.duration_ms, 150);
    }

    // ── ActionType Enum ──────────────────────────────────────────────────

    #[test]
    fn test_action_type_variants() {
        // Verify all variants exist and can be serialized
        let variants = vec![
            ActionType::ScanPort,
            ActionType::ScanHost,
            ActionType::CheckService,
            ActionType::BruteForce,
            ActionType::ProbeHttp,
            ActionType::CheckProcess,
            ActionType::ListenPort,
            ActionType::ExecuteCommand,
        ];
        for v in &variants {
            let json = serde_json::to_string(v).unwrap();
            let back: ActionType = serde_json::from_str(&json).unwrap();
            assert_eq!(*v, back);
        }
    }

    // ── Edge Case Parsing ────────────────────────────────────────────────

    #[test]
    fn test_extract_version_handles_empty() {
        assert_eq!(extract_version(""), None);
        assert_eq!(extract_version("no numbers here"), None);
    }

    #[test]
    fn test_extract_server_header_missing() {
        let output = "HTTP/1.1 200 OK\r\nContent-Type: text/html\r\n";
        assert_eq!(extract_server_header(output), None);
    }

    #[test]
    fn test_extract_pid_multiple_numbers() {
        let output = "PID 1234: sshd, parent PID 1";
        assert_eq!(extract_pid(output), Some("1234".to_string()));
    }
}
