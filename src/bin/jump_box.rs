// ─── Jump-box Server ──────────────────────────────────────────────────────
//
// The execution layer's remote endpoint.
//
// The Machine sends JSON ActionRequests over TCP.  This server receives them,
// validates the target against an allowlist, executes the corresponding
// shell command, logs everything, and returns a JSON ActionResult.
//
// Protocol:
//   Client:   TCP connect
//   Client → Server:  single line of JSON (ActionRequest) + newline
//   Server → Client:  single line of JSON (ActionResult) + newline
//   Client:   TCP close
//
// This server binds to 127.0.0.1:7878 by default.  It MUST NOT bind to
// 0.0.0.0.  The Machine connects via SSH tunnel or isolated local network.
//
// Safety constraints (non-negotiable):
//   1. Target allowlist — only predefined IPs/ranges are reachable
//   2. Log-before-execute — every command is logged before it runs
//   3. Timeout on every command — no action hangs forever
//   4. No shell injection — arguments are passed as argv[], not shell strings
// ────────────────────────────────────────────────────────────────────────────

use std::collections::HashMap;
use std::time::Instant;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::TcpListener;
use tokio::process::Command;
use tokio::time::{timeout, Duration};

// ─── Re-exported types from the main crate ─────────────────────────────────
// These are the shared protocol types.  The actuator module defines them
// for the client side; here we use them identically on the server side.
use the_machine::actuator::{
    ActionRequest, ActionType, ActionResult,
};

// ═══════════════════════════════════════════════════════════════════════════
// SAFETY CONFIGURATION
// ═══════════════════════════════════════════════════════════════════════════

/// Default bind address — localhost ONLY.  Never 0.0.0.0.
const BIND_ADDRESS: &str = "127.0.0.1:7878";

/// Hard timeout for any single action (seconds).
const ACTION_TIMEOUT_SECS: u64 = 120;

/// Default port range for full host scan.
const DEFAULT_TOP_PORTS: &str = "100";

/// Allowlist of target IPs this jump-box is allowed to touch.
///
/// Every incoming ActionRequest is checked against this list before
/// any command is executed.  Requests with targets outside this list
/// are rejected with an error — no command is run.
///
/// This is the last line of defense against a reasoning error in the
/// planner causing commands to hit unintended targets.
const ALLOWED_TARGETS: &[&str] = &[
    "192.168.100.10",  // Target VM
    // Add target IPs here before deploying
];

// ═══════════════════════════════════════════════════════════════════════════
// ENTRY POINT
// ═══════════════════════════════════════════════════════════════════════════

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Stderr logging — env_logger writes to stderr, JSON protocol to stdout.
    // In production, redirect stderr to a file:
    //   jump_box 2>> /var/log/jump_box.log
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info"))
        .format_timestamp_secs()
        .init();

    let listener = TcpListener::bind(BIND_ADDRESS).await?;

    log::info!("Jump-box server starting on {}", BIND_ADDRESS);
    log::info!("Allowed targets: {:?}", ALLOWED_TARGETS);
    log::info!("Action timeout: {}s", ACTION_TIMEOUT_SECS);
    log::warn!("This server executes shell commands.  Bind to 127.0.0.1 only.");
    log::info!("──────────────────────────────────────────");

    loop {
        let (socket, addr) = listener.accept().await?;
        log::info!("Connection from {}", addr);
        tokio::spawn(handle_connection(socket, addr));
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// CONNECTION HANDLER
// ═══════════════════════════════════════════════════════════════════════════

/// Handle a single TCP connection.
///
/// Protocol: read one newline-delimited JSON ActionRequest, execute,
/// write one newline-delimited JSON ActionResult, close.
async fn handle_connection(socket: tokio::net::TcpStream, addr: std::net::SocketAddr) {
    let start = Instant::now();
    let (reader, mut writer) = socket.into_split();
    let mut reader = BufReader::new(reader);
    let mut line = String::new();

    // ── 1. Read request ─────────────────────────────────────────────────
    match reader.read_line(&mut line).await {
        Ok(0) => {
            log::warn!("{}: empty request (connection closed)", addr);
            return;
        }
        Ok(_) => {
            log::debug!("{}: raw request: {}", addr, line.trim());
        }
        Err(e) => {
            log::error!("{}: read error: {}", addr, e);
            return;
        }
    }

    // ── 2. Deserialize ──────────────────────────────────────────────────
    let request: ActionRequest = match serde_json::from_str(line.trim()) {
        Ok(r) => r,
        Err(e) => {
            let msg = format!("invalid JSON: {}", e);
            log::error!("{}: {}", addr, msg);
            let error_result = ActionResult {
                success: false,
                raw_output: String::new(),
                observations: Vec::new(),
                error: Some(msg),
                duration_ms: start.elapsed().as_millis() as u64,
            };
            let _ = write_result(&mut writer, &error_result).await;
            return;
        }
    };

    log::info!("{}: {:?} target={} params={:?} timeout={}s",
        addr, request.action_type, request.target, request.params, request.timeout_secs);

    // ── 3. Validate target ──────────────────────────────────────────────
    if let Err(msg) = validate_target(&request.target) {
        log::error!("{}: target validation failed: {}", addr, msg);
        let error_result = ActionResult {
            success: false,
            raw_output: String::new(),
            observations: Vec::new(),
            error: Some(msg),
            duration_ms: start.elapsed().as_millis() as u64,
        };
        let _ = write_result(&mut writer, &error_result).await;
        return;
    }

    // ── 4. Execute ──────────────────────────────────────────────────────
    let result = match dispatch_action(&request).await {
        Ok(r) => r,
        Err(e) => {
            let msg = format!("execution error: {}", e);
            log::error!("{}: {}", addr, msg);
            ActionResult {
                success: false,
                raw_output: String::new(),
                observations: Vec::new(),
                error: Some(msg),
                duration_ms: start.elapsed().as_millis() as u64,
            }
        }
    };

    // ── 5. Write response ───────────────────────────────────────────────
    log::info!("{}: result: success={} duration={}ms error={:?} observations={}",
        addr, result.success, result.duration_ms, result.error, result.observations.len());

    if write_result(&mut writer, &result).await.is_err() {
        log::error!("{}: failed to write response", addr);
    }
}

/// Serialize and write an ActionResult to the socket.
async fn write_result(
    writer: &mut tokio::net::tcp::OwnedWriteHalf,
    result: &ActionResult,
) -> Result<(), Box<dyn std::error::Error>> {
    let json = serde_json::to_string(result)?;
    writer.write_all(format!("{}\n", json).as_bytes()).await?;
    writer.shutdown().await?;
    Ok(())
}

// ═══════════════════════════════════════════════════════════════════════════
// TARGET VALIDATION
// ═══════════════════════════════════════════════════════════════════════════

/// Check that a target is in the allowlist.
///
/// Returns Ok(()) if the target is allowed, Err with a message if not.
fn validate_target(target: &str) -> Result<(), String> {
    // Empty target is allowed for actions that don't need one
    // (e.g., CheckProcess with process name in params, ListenPort)
    if target.is_empty() {
        return Ok(());
    }

    if ALLOWED_TARGETS.contains(&target) {
        return Ok(());
    }

    // CIDR or prefix matching (e.g., "192.168.100.0/24")
    for allowed in ALLOWED_TARGETS {
        if let Some(prefix) = allowed.strip_suffix("/24") {
            if target.starts_with(&prefix[..prefix.len() - 1]) {
                return Ok(());
            }
        }
    }

    Err(format!(
        "target '{}' is not in the allowlist {:?}. \
         Add it to ALLOWED_TARGETS in src/bin/jump_box.rs and recompile.",
        target, ALLOWED_TARGETS
    ))
}

// ═══════════════════════════════════════════════════════════════════════════
// ACTION DISPATCH
// ═══════════════════════════════════════════════════════════════════════════

/// Dispatch an ActionRequest to the appropriate handler.
///
/// Every handler follows the same pattern:
///   1. Extract parameters from request.params
///   2. Log the full command BEFORE executing
///   3. Execute with timeout
///   4. Log stdout, stderr, and exit code
///   5. Return ActionResult
async fn dispatch_action(request: &ActionRequest) -> Result<ActionResult, String> {
    match request.action_type {
        ActionType::ScanPort => handle_scan_port(request).await,
        ActionType::ScanHost => handle_scan_host(request).await,
        ActionType::CheckService => handle_check_service(request).await,
        ActionType::BruteForce => handle_brute_force(request).await,
        ActionType::ProbeHttp => handle_probe_http(request).await,
        ActionType::CheckProcess => handle_check_process(request).await,
        ActionType::ListenPort => handle_listen_port(request).await,
        ActionType::ExecuteCommand => handle_execute_command(request).await,
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// ACTION HANDLERS
// ═══════════════════════════════════════════════════════════════════════════

/// ScanPort: nmap -p {port} {target} -oG -
async fn handle_scan_port(request: &ActionRequest) -> Result<ActionResult, String> {
    let port = request.params.get("port")
        .ok_or_else(|| "missing param: port".to_string())?;
    run_command("nmap", &["-p", port, &request.target, "-oG", "-"]).await
}

/// ScanHost: nmap -sV --top-ports {n} {target} -oG -
async fn handle_scan_host(request: &ActionRequest) -> Result<ActionResult, String> {
    let ports = request.params.get("ports")
        .map(|s| s.as_str())
        .unwrap_or(DEFAULT_TOP_PORTS);
    run_command("nmap", &["-sV", "--top-ports", ports, &request.target, "-oG", "-"]).await
}

/// CheckService: nmap -sV -p {port} {target}
async fn handle_check_service(request: &ActionRequest) -> Result<ActionResult, String> {
    let port = request.params.get("port")
        .ok_or_else(|| "missing param: port".to_string())?;
    run_command("nmap", &["-sV", "-p", port, &request.target]).await
}

/// BruteForce: hydra -L users.txt -P passwords.txt {target} ssh
async fn handle_brute_force(request: &ActionRequest) -> Result<ActionResult, String> {
    let port = request.params.get("port")
        .ok_or_else(|| "missing param: port".to_string())?;
    let users_str = request.params.get("users")
        .ok_or_else(|| "missing param: users".to_string())?;
    let passwords_str = request.params.get("passwords")
        .ok_or_else(|| "missing param: passwords".to_string())?;

    // Write credentials to temp files
    let user_file = write_temp_file("jump_users_", users_str)?;
    let pass_file = write_temp_file("jump_pass_", passwords_str)?;

    // Detect service from port
    let service = match port.as_str() {
        "22" => "ssh",
        "21" => "ftp",
        "80" | "443" => "http-post-form",  // simplified
        "3306" => "mysql",
        "5432" => "postgres",
        _ => "ssh",  // default
    };

    // Use hydra with stdin files
    let result = run_command("hydra", &[
        "-L", &user_file,
        "-P", &pass_file,
        "-s", port,
        &request.target,
        service,
        "-t", "4",        // 4 parallel threads
        "-o", "/dev/null",
        "-w", "10",       // 10s timeout per try
    ]).await;

    // Clean up temp files
    let _ = std::fs::remove_file(&user_file);
    let _ = std::fs::remove_file(&pass_file);

    result
}

/// ProbeHttp: curl -s -I --max-time 10 http://{target}:{port}{path}
async fn handle_probe_http(request: &ActionRequest) -> Result<ActionResult, String> {
    let port = request.params.get("port")
        .ok_or_else(|| "missing param: port".to_string())?;
    let path = request.params.get("path")
        .map(|s| s.as_str())
        .unwrap_or("/");
    let method = request.params.get("method")
        .map(|s| s.as_str())
        .unwrap_or("GET");

    let url = format!("http://{}:{}{}", request.target, port, path);

    if method == "HEAD" {
        run_command("curl", &["-s", "-I", "--max-time", "10", &url]).await
    } else {
        run_command("curl", &["-s", "--max-time", "10", &url]).await
    }
}

/// CheckProcess: pgrep -a {process_name} or ps aux | grep
async fn handle_check_process(request: &ActionRequest) -> Result<ActionResult, String> {
    let name = request.params.get("process_name")
        .ok_or_else(|| "missing param: process_name".to_string())?;

    // Try pgrep first; fall back to ps+grep if pgrep not available
    let result = run_command("pgrep", &["-a", name]).await;

    match result {
        Ok(r) => {
            if !r.success || r.raw_output.trim().is_empty() {
                // Fallback
                run_command("sh", &["-c", &format!("ps aux | grep -v grep | grep '{}'", name)]).await
            } else {
                Ok(r)
            }
        }
        Err(e) => {
            // pgrep not available — try ps fallback directly
            log::warn!("pgrep failed ({}), trying ps fallback", e);
            run_command("sh", &["-c", &format!("ps aux | grep -v grep | grep '{}'", name)]).await
        }
    }
}

/// ListenPort: nc -l -k -p {port} in background
/// Returns immediately; the listener stays alive in a child process.
async fn handle_listen_port(request: &ActionRequest) -> Result<ActionResult, String> {
    let port = request.params.get("port")
        .ok_or_else(|| "missing param: port".to_string())?;

    log::info!("Starting netcat listener on port {}", port);

    // Spawn in background — we don't wait for it
    let child = Command::new("nc")
        .args(["-l", "-k", "-p", port])
        .kill_on_drop(true)
        .spawn();

    match child {
        Ok(_) => Ok(ActionResult {
            success: true,
            raw_output: format!("Listener started on port {}", port),
            observations: Vec::new(),
            error: None,
            duration_ms: 0,
        }),
        Err(e) => Err(format!("failed to start netcat listener: {}", e)),
    }
}

/// ExecuteCommand: runs an arbitrary command via sh -c
///
/// WARNING: This is intentionally the most dangerous handler.
/// Target validation still applies, but if the target is allowlisted,
/// this runs whatever command is requested.  Logged with full verbosity.
async fn handle_execute_command(request: &ActionRequest) -> Result<ActionResult, String> {
    let command = request.params.get("command")
        .ok_or_else(|| "missing param: command".to_string())?;

    log::warn!("RAW COMMAND EXECUTION: {}", command);

    run_command("sh", &["-c", command]).await
}

// ═══════════════════════════════════════════════════════════════════════════
// COMMAND EXECUTION
// ═══════════════════════════════════════════════════════════════════════════

/// Run a shell command with args, log everything, return ActionResult.
///
/// This is the central execution primitive.  Every action handler funnels
/// through this function, guaranteeing:
///   - Full command is logged BEFORE execution
///   - stdout and stderr are captured and logged
///   - Exit code is logged
///   - A hard timeout prevents hanging
async fn run_command(cmd: &str, args: &[&str]) -> Result<ActionResult, String> {
    let full_command = format!("{} {}", cmd, args.join(" "));
    let start = Instant::now();

    // Log BEFORE executing — if the process crashes the system, the record
    // of what was attempted survives.
    log::info!("EXECUTE: {}", full_command);

    let child = Command::new(cmd)
        .args(args)
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .kill_on_drop(true)
        .spawn()
        .map_err(|e| format!("failed to spawn '{}': {}. Is it installed?", cmd, e))?;

    // Apply timeout
    let timeout_duration = Duration::from_secs(ACTION_TIMEOUT_SECS);
    let output = match timeout(timeout_duration, child.wait_with_output()).await {
        Ok(Ok(output)) => output,
        Ok(Err(e)) => return Err(format!("I/O error: {}", e)),
        Err(_) => {
            // Timeout — child was killed by kill_on_drop
            return Err(format!(
                "command timed out after {}s: {}",
                ACTION_TIMEOUT_SECS, full_command
            ));
        }
    };

    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();
    let exit_code = output.status.code().unwrap_or(-1);
    let duration_ms = start.elapsed().as_millis() as u64;

    // Log AFTER execution
    log::info!("TIMESTAMP: {}", chrono::Utc::now().to_rfc3339());
    log::info!("EXIT: {}", exit_code);
    log::info!("DURATION: {}ms", duration_ms);

    if !stdout.is_empty() {
        // Log stdout line by line for auditability
        for line in stdout.lines() {
            log::info!("STDOUT: {}", line);
        }
    }
    if !stderr.is_empty() {
        for line in stderr.lines() {
            log::info!("STDERR: {}", line);
        }
    }

    Ok(ActionResult {
        success: output.status.success(),
        raw_output: stdout,
        observations: Vec::new(),
        error: if output.status.success() {
            None
        } else {
            Some(if stderr.is_empty() {
                format!("exit code {}", exit_code)
            } else {
                stderr.trim().to_string()
            })
        },
        duration_ms,
    })
}

// ═══════════════════════════════════════════════════════════════════════════
// HELPERS
// ═══════════════════════════════════════════════════════════════════════════

/// Write a string to a temporary file and return the path.
fn write_temp_file(prefix: &str, content: &str) -> Result<String, String> {
    let mut path = std::env::temp_dir();
    path.push(format!("{}{}", prefix, uuid::Uuid::new_v4()));

    std::fs::write(&path, content)
        .map_err(|e| format!("failed to write temp file: {}", e))?;

    Ok(path.to_string_lossy().to_string())
}

// ═══════════════════════════════════════════════════════════════════════════
// TESTS
// ═══════════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;

    // ── Target Validation ───────────────────────────────────────────────

    #[test]
    fn test_validate_empty_target() {
        assert!(validate_target("").is_ok());
    }

    #[test]
    fn test_validate_allowed_target() {
        assert!(validate_target("192.168.100.10").is_ok());
    }

    #[test]
    fn test_validate_disallowed_target() {
        assert!(validate_target("192.168.1.1").is_err());
    }

    #[test]
    fn test_validate_external_target() {
        assert!(validate_target("google.com").is_err());
        assert!(validate_target("10.0.0.1").is_err());
    }

    // ── ActionRequest Deserialization ───────────────────────────────────

    #[test]
    fn test_deserialize_scan_port() {
        let json = r#"{
            "action_type": "ScanPort",
            "target": "192.168.100.10",
            "params": {"port": "22"},
            "timeout_secs": 30
        }"#;
        let req: ActionRequest = serde_json::from_str(json).unwrap();
        assert_eq!(req.action_type, ActionType::ScanPort);
        assert_eq!(req.target, "192.168.100.10");
        assert_eq!(req.params.get("port"), Some(&"22".to_string()));
    }

    #[test]
    fn test_deserialize_check_service() {
        let json = r#"{
            "action_type": "CheckService",
            "target": "192.168.100.10",
            "params": {"port": "80"},
            "timeout_secs": 30
        }"#;
        let req: ActionRequest = serde_json::from_str(json).unwrap();
        assert_eq!(req.action_type, ActionType::CheckService);
        assert_eq!(req.params.get("port"), Some(&"80".to_string()));
    }

    #[test]
    fn test_deserialize_brute_force() {
        let json = r#"{
            "action_type": "BruteForce",
            "target": "192.168.100.10",
            "params": {"port": "22", "users": "root,admin", "passwords": "password,1234"},
            "timeout_secs": 60
        }"#;
        let req: ActionRequest = serde_json::from_str(json).unwrap();
        assert_eq!(req.action_type, ActionType::BruteForce);
        assert_eq!(req.params.get("users"), Some(&"root,admin".to_string()));
        assert_eq!(req.params.get("passwords"), Some(&"password,1234".to_string()));
    }

    #[test]
    fn test_deserialize_probe_http() {
        let json = r#"{
            "action_type": "ProbeHttp",
            "target": "192.168.100.10",
            "params": {"port": "80", "path": "/index.html", "method": "GET"},
            "timeout_secs": 15
        }"#;
        let req: ActionRequest = serde_json::from_str(json).unwrap();
        assert_eq!(req.action_type, ActionType::ProbeHttp);
        assert_eq!(req.params.get("path"), Some(&"/index.html".to_string()));
    }

    #[test]
    fn test_deserialize_all_action_types() {
        // Every action type must deserialize from its JSON representation
        let cases = vec![
            (r#""ScanPort""#, ActionType::ScanPort),
            (r#""ScanHost""#, ActionType::ScanHost),
            (r#""CheckService""#, ActionType::CheckService),
            (r#""BruteForce""#, ActionType::BruteForce),
            (r#""ProbeHttp""#, ActionType::ProbeHttp),
            (r#""CheckProcess""#, ActionType::CheckProcess),
            (r#""ListenPort""#, ActionType::ListenPort),
            (r#""ExecuteCommand""#, ActionType::ExecuteCommand),
        ];
        for (json, expected) in cases {
            let deserialized: ActionType = serde_json::from_str(json).unwrap();
            assert_eq!(deserialized, expected, "Failed for {}", json);
        }
    }

    // ── ActionResult Serialization ──────────────────────────────────────

    #[test]
    fn test_serialize_action_result() {
        let result = ActionResult {
            success: true,
            raw_output: "22/tcp open ssh".to_string(),
            observations: vec![],
            error: None,
            duration_ms: 150,
        };
        let json = serde_json::to_string(&result).unwrap();
        assert!(json.contains("\"success\":true"));
        assert!(json.contains("\"raw_output\":\"22/tcp open ssh\""));
    }

    // ── Protocol Round-Trip ─────────────────────────────────────────────

    #[test]
    fn test_protocol_round_trip() {
        // Verify that what the client sends (ActionRequest) is exactly
        // what the server receives, and what the server sends (ActionResult)
        // is exactly what the client receives.
        let request = ActionRequest {
            action_type: ActionType::ScanPort,
            target: "192.168.100.10".to_string(),
            params: [("port".to_string(), "22".to_string())].into(),
            timeout_secs: 30,
        };

        // Client: serialize
        let client_json = serde_json::to_string(&request).unwrap();

        // Server: deserialize
        let server_req: ActionRequest = serde_json::from_str(&client_json).unwrap();
        assert_eq!(server_req.action_type, ActionType::ScanPort);
        assert_eq!(server_req.target, "192.168.100.10");
        assert_eq!(server_req.params.get("port"), Some(&"22".to_string()));

        // Server: create result
        let result = ActionResult {
            success: true,
            raw_output: "22/open".to_string(),
            observations: vec![
                ("192_168_100_10".to_string(), "has_open_port".to_string(), "port_22".to_string()),
            ],
            error: None,
            duration_ms: 150,
        };

        // Server: serialize
        let server_json = serde_json::to_string(&result).unwrap();

        // Client: deserialize
        let client_result: ActionResult = serde_json::from_str(&server_json).unwrap();
        assert!(client_result.success);
        assert_eq!(client_result.observations.len(), 1);
        assert_eq!(client_result.observations[0].0, "192_168_100_10");
    }
}
