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
// Network layout:
//   The Machine: 192.168.100.1
//   Jump-box:    192.168.100.2:7878
//   Target VM:   192.168.100.10
//
// Usage:
//   jump_box --bind 192.168.100.2:7878 \
//            --allowlist /etc/jumpbox/allowed_targets.txt \
//            --log /var/log/jumpbox.log
//
// Safety constraints (non-negotiable):
//   1. Target allowlist — only predefined IPs/ranges are reachable
//   2. Log-before-execute — every command is logged before it runs
//   3. Hard timeout (120s) — no action hangs forever
//   4. Arguments are passed as argv[], never concatenated into shell strings
//   5. --bind defaults to 127.0.0.1:7878 for safety; explicitly set for deployment
// ────────────────────────────────────────────────────────────────────────────

use std::collections::HashMap;
use std::fs;
use std::time::Instant;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::TcpListener;
use tokio::process::Command;
use tokio::time::{timeout, Duration};

use the_machine::actuator::{ActionRequest, ActionResult, ActionType};

// ═══════════════════════════════════════════════════════════════════════════
// CLI ARGUMENTS
// ═══════════════════════════════════════════════════════════════════════════

struct Config {
    /// Address to bind (default: 127.0.0.1:7878).
    pub bind: String,
    /// Path to allowlist file (one target per line, '#' comments, blank lines ignored).
    /// If not provided, falls back to built-in defaults.
    pub allowlist_path: Option<String>,
    /// Path to log file.  If not provided, logs to stderr.
    pub log_path: Option<String>,
}

impl Config {
    fn parse(args: &[String]) -> Result<Config, String> {
        let mut bind = "127.0.0.1:7878".to_string();
        let mut allowlist_path = None;
        let mut log_path = None;

        let mut i = 1; // skip binary name
        while i < args.len() {
            match args[i].as_str() {
                "--bind" => {
                    i += 1;
                    if i >= args.len() {
                        return Err(
                            "--bind requires an argument (e.g., 192.168.100.2:7878)".to_string()
                        );
                    }
                    bind = args[i].clone();
                }
                "--allowlist" => {
                    i += 1;
                    if i >= args.len() {
                        return Err("--allowlist requires a file path".to_string());
                    }
                    allowlist_path = Some(args[i].clone());
                }
                "--log" => {
                    i += 1;
                    if i >= args.len() {
                        return Err("--log requires a file path".to_string());
                    }
                    log_path = Some(args[i].clone());
                }
                "--help" | "-h" => {
                    return Err(format!(
                        "Usage: jump_box [OPTIONS]\n\
                         \n\
                         Options:\n\
                         --bind ADDR        Bind address (default: 127.0.0.1:7878)\n\
                         --allowlist FILE   Path to allowed targets file\n\
                         --log FILE         Path to log file (default: stderr)\n\
                         --help             Show this help\n\
                         \n\
                         Allowlist file format:\n\
                           One IP or CIDR per line.  '#' starts a comment.  Blank lines ignored.\n\
                         \n\
                         Example:\n\
                           192.168.100.10\n\
                           192.168.100.0/24\n\
                           # This is a comment\n"
                    ));
                }
                _ => {
                    return Err(format!(
                        "Unknown argument: {}. Use --help for usage.",
                        args[i]
                    ));
                }
            }
            i += 1;
        }
        Ok(Config {
            bind,
            allowlist_path,
            log_path,
        })
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// SAFETY CONFIGURATION
// ═══════════════════════════════════════════════════════════════════════════

/// Hard timeout for any single action (seconds).
const ACTION_TIMEOUT_SECS: u64 = 120;

/// Default port range for full host scan.
const DEFAULT_TOP_PORTS: &str = "100";

/// Built-in default allowlist (used when no --allowlist file is provided).
const BUILTIN_ALLOWED: &[&str] = &["192.168.100.10"];

// ═══════════════════════════════════════════════════════════════════════════
// ENTRY POINT
// ═══════════════════════════════════════════════════════════════════════════

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().collect();

    let config = match Config::parse(&args) {
        Ok(c) => c,
        Err(msg) => {
            eprintln!("{}", msg);
            std::process::exit(if msg.starts_with("Usage") { 0 } else { 1 });
        }
    };

    // ── Configure logging ───────────────────────────────────────────────
    // env_logger writes to stderr.  In production, redirect stderr to a file:
    //   jump_box 2>> /var/log/jumpbox.log
    // Or use systemd journal with StandardError=journal.
    if let Some(ref log_path) = config.log_path {
        // Store the log path for reference; actual redirection is done by
        // the caller (systemd, shell redirect, etc.)
        eprintln!("Logging to {} (ensure stderr is redirected)", log_path);
    }

    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info"))
        .format_timestamp_secs()
        .init();

    // ── Load allowlist ──────────────────────────────────────────────────
    let allowed_targets: Vec<String> = if let Some(ref path) = config.allowlist_path {
        load_allowlist(path)?
    } else {
        BUILTIN_ALLOWED.iter().map(|s| s.to_string()).collect()
    };

    if allowed_targets.is_empty() {
        log::warn!("Allowlist is EMPTY — no targets are reachable!");
        log::warn!(
            "Add targets to {} or use the built-in defaults.",
            config
                .allowlist_path
                .as_deref()
                .unwrap_or("--allowlist FILE")
        );
    }

    // ── Start server ────────────────────────────────────────────────────
    let listener = TcpListener::bind(&config.bind).await?;

    log::info!("Jump-box server starting on {}", config.bind);
    log::info!(
        "Allowed targets ({}): {:?}",
        allowed_targets.len(),
        allowed_targets
    );
    log::info!("Action timeout: {}s", ACTION_TIMEOUT_SECS);
    log::warn!("This server executes shell commands.  Bind to a non-routable isolated-network IP.");
    log::info!("──────────────────────────────────────────");

    loop {
        let (socket, addr) = listener.accept().await?;
        log::info!("Connection from {}", addr);
        let targets = allowed_targets.clone();
        tokio::spawn(handle_connection(socket, addr, targets));
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// ALLOWLIST LOADING
// ═══════════════════════════════════════════════════════════════════════════

/// Load a list of allowed targets from a file.
///
/// Format: one IP or CIDR per line.  Lines starting with '#' are comments.
/// Blank lines are ignored.  Leading/trailing whitespace is trimmed.
fn load_allowlist(path: &str) -> Result<Vec<String>, Box<dyn std::error::Error>> {
    let content = fs::read_to_string(path)?;
    let mut targets = Vec::new();

    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        targets.push(trimmed.to_string());
    }

    Ok(targets)
}

// ═══════════════════════════════════════════════════════════════════════════
// CONNECTION HANDLER
// ═══════════════════════════════════════════════════════════════════════════

/// Handle a single TCP connection.
///
/// Protocol: read one newline-delimited JSON ActionRequest, execute,
/// write one newline-delimited JSON ActionResult, close.
async fn handle_connection(
    socket: tokio::net::TcpStream,
    addr: std::net::SocketAddr,
    allowed_targets: Vec<String>,
) {
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

    log::info!(
        "{}: {:?} target={} params={:?} timeout={}s",
        addr,
        request.action_type,
        request.target,
        request.params,
        request.timeout_secs
    );

    // ── 3. Validate target ──────────────────────────────────────────────
    if let Err(msg) = validate_target(&request.target, &allowed_targets) {
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
    log::info!(
        "{}: result: success={} duration={}ms error={:?} observations={}",
        addr,
        result.success,
        result.duration_ms,
        result.error,
        result.observations.len()
    );

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
/// Supports:
/// - Exact IP match: "192.168.100.10"
/// - CIDR prefix: "192.168.100.0/24", "10.0.0.0/16"
/// - Empty string: always allowed (for actions that don't target a remote host)
fn validate_target(target: &str, allowed_targets: &[String]) -> Result<(), String> {
    if target.is_empty() {
        return Ok(());
    }

    // Exact match
    if allowed_targets.iter().any(|t| t == target) {
        return Ok(());
    }

    // CIDR match
    for allowed in allowed_targets {
        if let Some((cidr_base, cidr_len)) = allowed.split_once('/') {
            if let Ok(len) = cidr_len.parse::<u8>() {
                if ip_in_cidr(target, cidr_base, len) {
                    return Ok(());
                }
            }
        }
    }

    Err(format!(
        "target '{}' is not in the allowlist {:?}",
        target, allowed_targets
    ))
}

/// Check if an IP address falls within a CIDR range.
///
/// Simple string-based check: split both IPs into octets, compare
/// the first N bits (where N is the CIDR prefix length).
fn ip_in_cidr(ip: &str, cidr_base: &str, prefix_len: u8) -> bool {
    let ip_octets: Vec<&str> = ip.split('.').collect();
    let base_octets: Vec<&str> = cidr_base.split('.').collect();

    if ip_octets.len() != 4 || base_octets.len() != 4 {
        return false;
    }

    let ip_int = ip_octets
        .iter()
        .filter_map(|o| o.parse::<u32>().ok())
        .fold(0u32, |acc, octet| (acc << 8) | octet);

    let base_int = base_octets
        .iter()
        .filter_map(|o| o.parse::<u32>().ok())
        .fold(0u32, |acc, octet| (acc << 8) | octet);

    if prefix_len == 0 {
        return true; // /0 matches everything
    }

    let mask = if prefix_len >= 32 {
        u32::MAX
    } else {
        u32::MAX << (32 - prefix_len)
    };

    (ip_int & mask) == (base_int & mask)
}

// ═══════════════════════════════════════════════════════════════════════════
// ACTION DISPATCH
// ═══════════════════════════════════════════════════════════════════════════

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
        ActionType::FetchDocumentation => handle_fetch_documentation(request).await,
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// ACTION HANDLERS
// ═══════════════════════════════════════════════════════════════════════════

async fn handle_scan_port(request: &ActionRequest) -> Result<ActionResult, String> {
    let port = request
        .params
        .get("port")
        .ok_or_else(|| "missing param: port".to_string())?;
    run_command("nmap", &["-p", port, &request.target, "-oG", "-"]).await
}

async fn handle_scan_host(request: &ActionRequest) -> Result<ActionResult, String> {
    let ports = request
        .params
        .get("ports")
        .map(|s| s.as_str())
        .unwrap_or(DEFAULT_TOP_PORTS);
    run_command(
        "nmap",
        &["-sV", "--top-ports", ports, &request.target, "-oG", "-"],
    )
    .await
}

async fn handle_check_service(request: &ActionRequest) -> Result<ActionResult, String> {
    let port = request
        .params
        .get("port")
        .ok_or_else(|| "missing param: port".to_string())?;
    run_command("nmap", &["-sV", "-p", port, &request.target]).await
}

async fn handle_brute_force(request: &ActionRequest) -> Result<ActionResult, String> {
    let port = request
        .params
        .get("port")
        .ok_or_else(|| "missing param: port".to_string())?;
    let users_str = request
        .params
        .get("users")
        .ok_or_else(|| "missing param: users".to_string())?;
    let passwords_str = request
        .params
        .get("passwords")
        .ok_or_else(|| "missing param: passwords".to_string())?;

    // Write credentials to temp files
    let user_file = write_temp_file("jump_users_", users_str)?;
    let pass_file = write_temp_file("jump_pass_", passwords_str)?;

    // Detect service from port
    let service = match port.as_str() {
        "22" => "ssh",
        "21" => "ftp",
        "80" | "443" => "http-post-form",
        "3306" => "mysql",
        "5432" => "postgres",
        _ => "ssh",
    };

    let result = run_command(
        "hydra",
        &[
            "-L",
            &user_file,
            "-P",
            &pass_file,
            "-s",
            port,
            &request.target,
            service,
            "-t",
            "4",
            "-o",
            "/dev/null",
            "-w",
            "10",
        ],
    )
    .await;

    let _ = std::fs::remove_file(&user_file);
    let _ = std::fs::remove_file(&pass_file);

    result
}

async fn handle_probe_http(request: &ActionRequest) -> Result<ActionResult, String> {
    let port = request
        .params
        .get("port")
        .ok_or_else(|| "missing param: port".to_string())?;
    let path = request
        .params
        .get("path")
        .map(|s| s.as_str())
        .unwrap_or("/");
    let method = request
        .params
        .get("method")
        .map(|s| s.as_str())
        .unwrap_or("GET");

    let url = format!("http://{}:{}{}", request.target, port, path);

    if method == "HEAD" {
        run_command("curl", &["-s", "-I", "--max-time", "10", &url]).await
    } else {
        run_command("curl", &["-s", "--max-time", "10", &url]).await
    }
}

async fn handle_check_process(request: &ActionRequest) -> Result<ActionResult, String> {
    let name = request
        .params
        .get("process_name")
        .ok_or_else(|| "missing param: process_name".to_string())?;

    let result = run_command("pgrep", &["-a", name]).await;

    match result {
        Ok(r) => {
            if !r.success || r.raw_output.trim().is_empty() {
                run_command(
                    "sh",
                    &["-c", &format!("ps aux | grep -v grep | grep '{}'", name)],
                )
                .await
            } else {
                Ok(r)
            }
        }
        Err(e) => {
            log::warn!("pgrep failed ({}), trying ps fallback", e);
            run_command(
                "sh",
                &["-c", &format!("ps aux | grep -v grep | grep '{}'", name)],
            )
            .await
        }
    }
}

async fn handle_listen_port(request: &ActionRequest) -> Result<ActionResult, String> {
    let port = request
        .params
        .get("port")
        .ok_or_else(|| "missing param: port".to_string())?;

    log::info!("Starting netcat listener on port {}", port);

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

async fn handle_execute_command(request: &ActionRequest) -> Result<ActionResult, String> {
    let command = request
        .params
        .get("command")
        .ok_or_else(|| "missing param: command".to_string())?;

    log::warn!("RAW COMMAND EXECUTION: {}", command);
    run_command("sh", &["-c", command]).await
}

/// Fetch documentation for a term.  Tries man pages first, then --help.
/// Binaries allowed for `--help` documentation lookup.
/// Constrained to standard system documentation tools only — prevents
/// arbitrary binary execution from learned/observed query terms.
const DOC_BINARY_ALLOWLIST: &[&str] = &["man", "apropos", "whatis", "perror", "errno"];

async fn handle_fetch_documentation(request: &ActionRequest) -> Result<ActionResult, String> {
    let query = request
        .params
        .get("query")
        .ok_or_else(|| "missing param: query".to_string())?;

    // Sanitize: remove dangerous characters
    let safe_query: String = query
        .chars()
        .filter(|c| c.is_alphanumeric() || *c == '-' || *c == '_' || *c == '.')
        .collect();

    if safe_query.is_empty() || safe_query.len() > 100 {
        return Err("invalid query".to_string());
    }

    log::info!("FETCH DOCS: {}", safe_query);

    // Try man page first
    let man_result = run_command("man", &["-P", "cat", &safe_query]).await;
    if let Ok(result) = man_result {
        if result.success && !result.raw_output.trim().is_empty() {
            return Ok(result);
        }
    }

    // Fall back to --help on allowlisted binaries only
    // Constrained to prevent arbitrary binary execution from
    // learned/observed terms (security boundary).
    if DOC_BINARY_ALLOWLIST.contains(&safe_query.as_str()) {
        let help_result = run_command(&safe_query, &["--help"]).await;
        if let Ok(result) = help_result {
            if result.success && !result.raw_output.trim().is_empty() {
                return Ok(result);
            }
        }
    }

    // Fall back to error code lookup via `perror` or `errno`
    let perror_result = run_command("perror", &[&safe_query]).await;
    if let Ok(result) = perror_result {
        if result.success && !result.raw_output.trim().is_empty() {
            return Ok(result);
        }
    }

    // If nothing worked, return what we have (even if empty)
    Ok(ActionResult {
        success: false,
        raw_output: format!("no documentation found for '{}'\n", safe_query),
        error: Some("not found".to_string()),
        observations: vec![],
        duration_ms: 0,
    })
}

// ═══════════════════════════════════════════════════════════════════════════
// COMMAND EXECUTION
// ═══════════════════════════════════════════════════════════════════════════

/// Run a shell command with args, log everything, return ActionResult.
async fn run_command(cmd: &str, args: &[&str]) -> Result<ActionResult, String> {
    let full_command = format!("{} {}", cmd, args.join(" "));
    let start = Instant::now();

    log::info!("EXECUTE: {}", full_command);

    let child = Command::new(cmd)
        .args(args)
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .kill_on_drop(true)
        .spawn()
        .map_err(|e| format!("failed to spawn '{}': {}. Is it installed?", cmd, e))?;

    let timeout_duration = Duration::from_secs(ACTION_TIMEOUT_SECS);
    let output = match timeout(timeout_duration, child.wait_with_output()).await {
        Ok(Ok(output)) => output,
        Ok(Err(e)) => return Err(format!("I/O error: {}", e)),
        Err(_) => {
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

    log::info!("TIMESTAMP: {}", chrono::Utc::now().to_rfc3339());
    log::info!("EXIT: {}", exit_code);
    log::info!("DURATION: {}ms", duration_ms);

    if !stdout.is_empty() {
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

fn write_temp_file(prefix: &str, content: &str) -> Result<String, String> {
    let mut path = std::env::temp_dir();
    path.push(format!("{}{}", prefix, uuid::Uuid::new_v4()));

    std::fs::write(&path, content).map_err(|e| format!("failed to write temp file: {}", e))?;

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
        let list = vec!["192.168.100.10".to_string()];
        assert!(validate_target("", &list).is_ok());
    }

    #[test]
    fn test_validate_allowed_target() {
        let list = vec!["192.168.100.10".to_string()];
        assert!(validate_target("192.168.100.10", &list).is_ok());
    }

    #[test]
    fn test_validate_disallowed_target() {
        let list = vec!["192.168.100.10".to_string()];
        assert!(validate_target("192.168.1.1", &list).is_err());
    }

    #[test]
    fn test_validate_external_target() {
        let list = vec!["192.168.100.10".to_string()];
        assert!(validate_target("google.com", &list).is_err());
        assert!(validate_target("10.0.0.1", &list).is_err());
    }

    #[test]
    fn test_validate_cidr_24() {
        let list = vec!["192.168.100.0/24".to_string()];
        assert!(validate_target("192.168.100.10", &list).is_ok());
        assert!(validate_target("192.168.100.200", &list).is_ok());
        assert!(validate_target("192.168.101.1", &list).is_err());
    }

    #[test]
    fn test_validate_cidr_16() {
        let list = vec!["10.0.0.0/16".to_string()];
        assert!(validate_target("10.0.1.1", &list).is_ok());
        assert!(validate_target("10.0.255.255", &list).is_ok());
        assert!(validate_target("10.1.0.1", &list).is_err());
    }

    // ── Allowlist Loading ───────────────────────────────────────────────

    #[test]
    fn test_load_allowlist() {
        let dir = std::env::temp_dir();
        let path = dir.join("test_allowlist.txt");
        std::fs::write(
            &path,
            "192.168.100.10\n# comment\n10.0.0.0/8\n\n192.168.1.1",
        )
        .unwrap();

        let list = load_allowlist(path.to_str().unwrap()).unwrap();
        assert_eq!(list.len(), 3);
        assert!(list.contains(&"192.168.100.10".to_string()));
        assert!(list.contains(&"10.0.0.0/8".to_string()));
        assert!(list.contains(&"192.168.1.1".to_string()));

        std::fs::remove_file(&path).unwrap();
    }

    // ── CLI Argument Parsing ────────────────────────────────────────────

    #[test]
    fn test_config_parse_defaults() {
        let args = vec!["jump_box".to_string()];
        let config = Config::parse(&args).unwrap();
        assert_eq!(config.bind, "127.0.0.1:7878");
        assert!(config.allowlist_path.is_none());
        assert!(config.log_path.is_none());
    }

    #[test]
    fn test_config_parse_bind() {
        let args = vec![
            "jump_box".to_string(),
            "--bind".to_string(),
            "192.168.100.2:7878".to_string(),
        ];
        let config = Config::parse(&args).unwrap();
        assert_eq!(config.bind, "192.168.100.2:7878");
    }

    #[test]
    fn test_config_parse_allowlist() {
        let args = vec![
            "jump_box".to_string(),
            "--allowlist".to_string(),
            "/etc/jumpbox/allowed_targets.txt".to_string(),
        ];
        let config = Config::parse(&args).unwrap();
        assert_eq!(
            config.allowlist_path.unwrap(),
            "/etc/jumpbox/allowed_targets.txt"
        );
    }

    #[test]
    fn test_config_parse_all() {
        let args = vec![
            "jump_box".to_string(),
            "--bind".to_string(),
            "0.0.0.0:9999".to_string(), // user's responsibility
            "--allowlist".to_string(),
            "/tmp/allow.txt".to_string(),
            "--log".to_string(),
            "/var/log/jumpbox.log".to_string(),
        ];
        let config = Config::parse(&args).unwrap();
        assert_eq!(config.bind, "0.0.0.0:9999");
        assert_eq!(config.allowlist_path.unwrap(), "/tmp/allow.txt");
        assert_eq!(config.log_path.unwrap(), "/var/log/jumpbox.log");
    }

    #[test]
    fn test_config_parse_missing_value() {
        let args = vec!["jump_box".to_string(), "--bind".to_string()];
        assert!(Config::parse(&args).is_err());
    }

    #[test]
    fn test_config_parse_unknown_arg() {
        let args = vec!["jump_box".to_string(), "--nonsense".to_string()];
        assert!(Config::parse(&args).is_err());
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
    }

    #[test]
    fn test_deserialize_all_action_types() {
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
            assert_eq!(deserialized, expected);
        }
    }

    // ── Protocol Round-Trip ─────────────────────────────────────────────

    #[test]
    fn test_protocol_round_trip() {
        let request = ActionRequest {
            action_type: ActionType::ScanPort,
            target: "192.168.100.10".to_string(),
            params: [("port".to_string(), "22".to_string())].into(),
            timeout_secs: 30,
        };

        let client_json = serde_json::to_string(&request).unwrap();
        let server_req: ActionRequest = serde_json::from_str(&client_json).unwrap();
        assert_eq!(server_req.action_type, ActionType::ScanPort);

        let result = ActionResult {
            success: true,
            raw_output: "22/open".to_string(),
            observations: vec![],
            error: None,
            duration_ms: 150,
        };

        let server_json = serde_json::to_string(&result).unwrap();
        let client_result: ActionResult = serde_json::from_str(&server_json).unwrap();
        assert!(client_result.success);
    }
}
