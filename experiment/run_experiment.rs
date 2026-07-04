// ─── Experiment Runner ──────────────────────────────────────────────────
//
// Wire everything together and run the full attack experiment.
//
// Build:
//   This is a separate binary.  Add to Cargo.toml:
//     [[bin]]
//     name = "attack_experiment"
//     path = "experiment/run_experiment.rs"
//
// Run:
//   cargo run --bin attack_experiment -- [options]
//
// Options:
//   --jumpbox ADDR    Jump-box address (default: 192.168.100.2:7878)
//   --target  IP      Target VM IP (default: 192.168.100.10)
//   --steps   N      Max plan steps (default: 20)
//
// Network requirements:
//   - Jump-box VM at JUMPBOX_ADDR with jump_box binary running
//   - Target VM at TARGET_ADDR with vulnerable services
//   - Isolated network, no internet
// ────────────────────────────────────────────────────────────────────────────

use std::time::{Duration, Instant};
use the_machine::actuator::{
    self, GoalChecker, JumpBoxActuator, AttackCycleResult,
};
use the_machine::qa::QaEngine;
use the_machine::text_encoder::ingest_text;
use the_machine::VSABrain;

// Default addresses
const DEFAULT_JUMPBOX: &str = "192.168.100.2:7878";
const DEFAULT_TARGET: &str = "192.168.100.10";
const DEFAULT_MAX_STEPS: usize = 20;

// ═══════════════════════════════════════════════════════════════════════════
// CLI
// ═══════════════════════════════════════════════════════════════════════════

struct ExperimentConfig {
    jumpbox_addr: String,
    target_ip: String,
    max_steps: usize,
    verbose: bool,
}

impl ExperimentConfig {
    fn parse(args: &[String]) -> Self {
        let mut config = ExperimentConfig {
            jumpbox_addr: DEFAULT_JUMPBOX.to_string(),
            target_ip: DEFAULT_TARGET.to_string(),
            max_steps: DEFAULT_MAX_STEPS,
            verbose: false,
        };

        let mut i = 1;
        while i < args.len() {
            match args[i].as_str() {
                "--jumpbox" => {
                    i += 1;
                    if i < args.len() {
                        config.jumpbox_addr = args[i].clone();
                    }
                }
                "--target" => {
                    i += 1;
                    if i < args.len() {
                        config.target_ip = args[i].clone();
                    }
                }
                "--steps" => {
                    i += 1;
                    if i < args.len() {
                        config.max_steps = args[i].parse().unwrap_or(DEFAULT_MAX_STEPS);
                    }
                }
                "--verbose" => {
                    config.verbose = true;
                }
                "--help" => {
                    println!("Usage: attack_experiment [OPTIONS]");
                    println!();
                    println!("Run the autonomous attack experiment.");
                    println!();
                    println!("Options:");
                    println!("  --jumpbox ADDR   Jump-box address (default: {})", DEFAULT_JUMPBOX);
                    println!("  --target  IP     Target VM IP (default: {})", DEFAULT_TARGET);
                    println!("  --steps   N      Max plan steps (default: {})", DEFAULT_MAX_STEPS);
                    println!("  --verbose        Detailed per-step logging");
                    println!("  --help           Show this help");
                    std::process::exit(0);
                }
                _ => {
                    eprintln!("Unknown argument: {}", args[i]);
                    eprintln!("Use --help for usage.");
                    std::process::exit(1);
                }
            }
            i += 1;
        }

        config
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// MAIN
// ═══════════════════════════════════════════════════════════════════════════

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().collect();
    let config = ExperimentConfig::parse(&args);

    println!("╔══════════════════════════════════════════════════════════╗");
    println!("║        The Machine — Autonomous Attack Experiment      ║");
    println!("╚══════════════════════════════════════════════════════════╝");
    println!();
    println!("  Jump-box:    {}", config.jumpbox_addr);
    println!("  Target:      {}", config.target_ip);
    println!("  Max steps:   {}", config.max_steps);
    println!("  Verbose:     {}", config.verbose);
    println!();

    // ── Initialize ──────────────────────────────────────────────────────
    let experiment_start = Instant::now();

    // Create the brain and QA engine
    let mut brain = VSABrain::new(0.12);
    let mut qa = QaEngine::new();

    println!("[1/3] Seeding knowledge...");

    // Seed attack rules and documentation
    // seed_knowledge(qa, brain) — this would be the function from seed_knowledge.rs
    // For now, we seed inline:
    seed_knowledge(&mut qa, &mut brain);

    // ── Connect to jump-box ─────────────────────────────────────────────
    println!("[2/3] Connecting to jump-box at {}...", config.jumpbox_addr);

    // Parse the jump-box address into host and port
    let jb_parts: Vec<&str> = config.jumpbox_addr.split(':').collect();
    let jb_host = jb_parts[0];
    let jb_port: u16 = jb_parts.get(1)
        .and_then(|p| p.parse().ok())
        .unwrap_or(7878);

    let actuator = JumpBoxActuator::new(jb_host, jb_port);
    println!("  ✓ Connected (or attempting)");

    // ── Run the attack loop ─────────────────────────────────────────────
    println!("[3/3] Starting attack loop...");
    println!(
        "  Goal: machine has_access_to target_vm (IP: {})",
        config.target_ip
    );
    println!("  Max plan steps: {}", config.max_steps);
    println!();

    // Inject the target IP as a known fact so the planner knows who to target
    the_machine::text_encoder::store_knowledge_triple(
        &mut brain,
        "target_vm",
        "ip",
        &config.target_ip,
        1.0,
        "experiment_config",
    );

    // Run the agentic attack loop
    let results = actuator::run_attack_loop(
        &mut brain,
        &mut qa,
        &actuator,
        ("machine", "has_access_to", "target_vm"),
        config.max_steps,
    )
    .await;

    // ── Report results ──────────────────────────────────────────────────
    let experiment_duration = experiment_start.elapsed();

    println!();
    println!("╔══════════════════════════════════════════════════════════╗");
    println!("║                 Experiment Complete                     ║");
    println!("╚══════════════════════════════════════════════════════════╝");
    println!();
    println!("  Duration:    {:?}", experiment_duration);
    println!("  Steps executed: {}", results.len());
    println!();

    let goals = results.iter().filter(|r| r.goal_achieved).count();
    let succeeded = results.iter().filter(|r| r.action_result.success).count();
    let failed = results.len() - succeeded;

    println!("  Actions:");
    println!("    Succeeded: {}", succeeded);
    println!("    Failed:    {}", failed);
    println!("  Goal achieved: {}",
        if goals > 0 { "YES" } else { "NO" });
    println!();

    // Print reasoning trace
    println!("╔══════════════════════════════════════════════════════════╗");
    println!("║                  Reasoning Trace                       ║");
    println!("╚══════════════════════════════════════════════════════════╝");
    println!();

    for (i, result) in results.iter().enumerate() {
        println!("── Step {} ────────────────────────────────────────", i + 1);

        if let Some(ref step) = result.plan_step {
            println!("  Plan: ({} {} {})",
                step.action.0, step.action.1, step.action.2);
            println!("  Achieves: ({} {} {})",
                step.achieves.0, step.achieves.1, step.achieves.2);
            println!("  Confidence: {:.4}", step.confidence);
        } else if let Some(ref req) = result.action_request {
            println!("  Intel: {:?} target={} params={:?}",
                req.action_type, req.target, req.params);
        }

        if result.action_result.success {
            println!("  ✓ SUCCESS ({}ms)", result.action_result.duration_ms);
            if !result.action_result.raw_output.is_empty() {
                // Print first 200 chars of output
                let preview: String = result.action_result.raw_output
                    .chars().take(200).collect();
                println!("  Output: {}", preview);
            }
        } else {
            println!("  ✗ FAILED: {:?}", result.action_result.error);
        }

        println!("  Observations ingested: {}", result.observations_ingested);
        println!("  Goal achieved: {}", result.goal_achieved);
        println!();
    }

    // Summary
    println!("╔══════════════════════════════════════════════════════════╗");
    println!("║                    Summary                             ║");
    println!("╚══════════════════════════════════════════════════════════╝");
    if goals > 0 {
        println!("  ✓ The Machine successfully achieved its goal.");
        println!("  ✓ Autonomous multi-step attack planning is validated.");
    } else {
        println!("  ✗ The Machine did not achieve its goal.");
        println!("  Review the trace above to understand why.");
        if succeeded == 0 {
            println!("  ⚠ No actions succeeded — check jump-box connectivity.");
        }
    }

    Ok(())
}

// ─── Inline knowledge seeding ──────────────────────────────────────────
// (Copy of seed_knowledge.rs to make this a standalone binary)

fn seed_knowledge(qa: &mut QaEngine, brain: &mut VSABrain) {
    // Attack chain rules
    qa.store_rule(
        "scan_result", "reveals", "open_service",
        "machine", "knows", "service_on_target",
        "attack_knowledge",
    );
    qa.store_rule(
        "open_service", "has", "version",
        "machine", "knows", "service_version",
        "attack_knowledge",
    );
    qa.store_rule(
        "service_version", "has", "vulnerability",
        "machine", "identifies", "attack_vector",
        "attack_knowledge",
    );
    qa.store_rule(
        "vulnerability", "enables", "exploit",
        "machine", "can", "execute_exploit",
        "attack_knowledge",
    );
    qa.store_rule(
        "exploit", "gives", "shell_access",
        "machine", "has", "access_to_target",
        "attack_knowledge",
    );

    // CVE knowledge
    qa.store_rule(
        "vsftpd_2_3_4", "has", "backdoor_on_port_6200",
        "port_6200", "grants", "shell_access",
        "cve_2011_2523",
    );
    qa.store_rule(
        "apache_2_4_49", "enables", "path_traversal",
        "path_traversal", "reads", "sensitive_files",
        "cve_2021_41773",
    );
    qa.store_rule(
        "ssh_weak_credential", "enables", "brute_force_success",
        "brute_force_success", "gives", "shell_access",
        "weak_credential_knowledge",
    );

    // Action rules
    qa.store_action(
        "machine", "scan_port", "target:port",
        "machine", "knows", "port_state",
        "attack_actions",
    );
    qa.store_action(
        "machine", "scan_host", "target",
        "machine", "knows", "open_ports_on_target",
        "attack_actions",
    );
    qa.store_action(
        "machine", "check_service", "target:port",
        "machine", "knows", "service_version_on_port",
        "attack_actions",
    );
    qa.store_action(
        "machine", "probe_http", "target:port:path",
        "machine", "knows", "http_response",
        "attack_actions",
    );
    qa.store_action(
        "machine", "brute_force", "target:port:users:passwords",
        "machine", "has", "shell_access",
        "attack_actions",
    );
    qa.store_action(
        "machine", "check_process", "target:name",
        "machine", "knows", "process_running",
        "attack_actions",
    );
    qa.store_action(
        "machine", "execute_command", "target:command",
        "machine", "collects", "command_output",
        "attack_actions",
    );

    // Factual knowledge
    qa.store_fact("ssh", "runs_on", "port_22", "standard_knowledge");
    qa.store_fact("http", "runs_on", "port_80", "standard_knowledge");
    qa.store_fact("ftp", "runs_on", "port_21", "standard_knowledge");
    qa.store_fact("vsftpd_2_3_4", "has_backdoor", "port_6200", "cve_database");
    qa.store_fact("cve_2011_2523", "affects", "vsftpd_2_3_4", "cve_database");
    qa.store_fact("cve_2021_41773", "affects", "apache_2_4_49", "cve_database");
    qa.store_fact("admin", "has_weak_password", "password123", "credential_knowledge");
    qa.store_fact("root", "has_weak_password", "toor", "credential_knowledge");

    // Metadata for the experiment
    the_machine::text_encoder::store_knowledge_triple(
        brain, "machine", "is_ready", "true", 1.0, "experiment_metadata"
    );
}
