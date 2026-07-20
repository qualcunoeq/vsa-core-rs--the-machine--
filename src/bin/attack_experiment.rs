// ─── Autonomous Attack Experiment ──────────────────────────────────────
//
// Wires everything together: seeds knowledge, connects to jump-box,
// runs the agentic attack loop, reports results.
//
// Usage:
//   cargo run --bin attack_experiment -- [options]
//
// Options:
//   --jumpbox ADDR    Jump-box address (default: 192.168.100.2:7878)
//   --target  IP      Target VM IP (default: 192.168.100.10)
//   --steps   N       Max plan steps (default: 20)
//   --verbose         Detailed per-step logging
//
// Prerequisites:
//   - Jump-box VM running at JUMPBOX_ADDR
//   - Target VM at TARGET_ADDR with vulnerable services
//   - Isolated network, no internet
// ────────────────────────────────────────────────────────────────────────────

use std::time::Instant;
use the_machine::actuator::{self, JumpBoxActuator};
use the_machine::experiment::{seed_documentation, seed_knowledge};
use the_machine::qa::QaEngine;
use the_machine::VSABrain;

const DEFAULT_JUMPBOX: &str = "192.168.100.2:7878";
const DEFAULT_TARGET: &str = "192.168.100.10";
const DEFAULT_STEPS: usize = 20;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().collect();

    // ── Parse CLI ──────────────────────────────────────────────────────
    let mut jb_addr = DEFAULT_JUMPBOX.to_string();
    let mut target_ip = DEFAULT_TARGET.to_string();
    let mut max_steps = DEFAULT_STEPS;
    let mut verbose = false;

    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "--jumpbox" => {
                i += 1;
                if i < args.len() {
                    jb_addr = args[i].clone();
                }
            }
            "--target" => {
                i += 1;
                if i < args.len() {
                    target_ip = args[i].clone();
                }
            }
            "--steps" => {
                i += 1;
                if i < args.len() {
                    max_steps = args[i].parse().unwrap_or(DEFAULT_STEPS);
                }
            }
            "--verbose" => {
                verbose = true;
            }
            "--help" | "-h" => {
                println!("Usage: attack_experiment [OPTIONS]");
                println!("");
                println!("Run the autonomous attack experiment.");
                println!("");
                println!("Options:");
                println!("  --jumpbox ADDR   Jump-box (default: {})", DEFAULT_JUMPBOX);
                println!("  --target  IP     Target VM (default: {})", DEFAULT_TARGET);
                println!("  --steps   N      Max steps (default: {})", DEFAULT_STEPS);
                println!("  --verbose        Detailed logging");
                return Ok(());
            }
            _ => {
                eprintln!("Unknown: {}. Use --help.", args[i]);
                std::process::exit(1);
            }
        }
        i += 1;
    }

    eprintln!("═══════════════════════════════════════════════════");
    eprintln!("  The Machine — Autonomous Attack Experiment");
    eprintln!("═══════════════════════════════════════════════════");
    eprintln!("  Jump-box: {}", jb_addr);
    eprintln!("  Target:   {}", target_ip);
    eprintln!("  Max steps: {}", max_steps);
    eprintln!("");

    let start = Instant::now();

    // ── Initialize ─────────────────────────────────────────────────────
    let mut brain = VSABrain::new(0.12);
    let mut qa = QaEngine::new();

    eprintln!("[1/3] Seeding knowledge...");
    seed_knowledge(&mut qa, &mut brain);
    seed_documentation(&mut brain);

    // Inject target IP into brain
    the_machine::text_encoder::store_knowledge_triple(
        &mut brain,
        "target_vm",
        "ip",
        &target_ip,
        1.0,
        "experiment_config",
    );

    // ── Connect ─────────────────────────────────────────────────────────
    eprintln!("[2/3] Connecting to jump-box at {}...", jb_addr);

    let jb_parts: Vec<&str> = jb_addr.split(':').collect();
    let jb_host = jb_parts[0];
    let jb_port: u16 = jb_parts.get(1).and_then(|p| p.parse().ok()).unwrap_or(7878);
    let actuator = JumpBoxActuator::new(jb_host, jb_port);

    // Quick connectivity test
    let test_req = the_machine::actuator::ActionRequest::scan_port(&target_ip, 22);
    let test_result = actuator.send_request(&test_req).await;
    if test_result.success {
        eprintln!("  ✓ Jump-box reachable");
    } else {
        eprintln!("  ⚠ Jump-box test: {:?}", test_result.error);
        eprintln!("  (Expected if target VM is not running yet)");
    }

    // ── Run attack loop ─────────────────────────────────────────────────
    eprintln!("[3/3] Running attack loop...");
    eprintln!("  Goal: machine has_access_to target_vm");
    eprintln!("");

    let results = actuator::run_attack_loop(
        &mut brain,
        &mut qa,
        &actuator,
        ("machine", "has_access_to", "target_vm"),
        max_steps,
    )
    .await;

    let elapsed = start.elapsed();

    // ── Report ──────────────────────────────────────────────────────────
    eprintln!("");
    eprintln!("═══════════════════════════════════════════════════");
    eprintln!("  Experiment Complete  ({:?})", elapsed);
    eprintln!("═══════════════════════════════════════════════════");
    eprintln!("");

    let goal_achieved = results.iter().any(|r| r.goal_achieved);
    let succeeded = results.iter().filter(|r| r.action_result.success).count();
    let failed = results.len() - succeeded;

    eprintln!("  Results:");
    eprintln!("    Steps executed: {}", results.len());
    eprintln!("    Succeeded:      {}", succeeded);
    eprintln!("    Failed:         {}", failed);
    eprintln!(
        "    Goal achieved:  {}",
        if goal_achieved { "YES ✓" } else { "NO" }
    );
    eprintln!("");

    if verbose {
        eprintln!("  Reasoning Trace:");
        eprintln!("");

        for (i, r) in results.iter().enumerate() {
            eprintln!("  ── Step {} ──────────────────────", i + 1);

            if let Some(ref step) = r.plan_step {
                eprintln!(
                    "   Action: ({} {} {})",
                    step.action.0, step.action.1, step.action.2
                );
                eprintln!(
                    "   Achieves: ({} {} {})",
                    step.achieves.0, step.achieves.1, step.achieves.2
                );
                eprintln!("   Confidence: {:.3}", step.confidence);
            }

            if r.action_result.success {
                eprintln!("   ✓ SUCCESS ({}ms)", r.action_result.duration_ms);
                let preview: String = r.action_result.raw_output.chars().take(120).collect();
                if !preview.is_empty() {
                    eprintln!("   Output: {}", preview);
                }
            } else {
                eprintln!("   ✗ FAILED: {:?}", r.action_result.error);
            }
            eprintln!("   Observations: {}", r.observations_ingested);
            eprintln!("");
        }
    }

    // Exit with status code indicating success/failure
    if goal_achieved {
        eprintln!("  ✓ The Machine achieved its goal autonomously.");
        std::process::exit(0);
    } else {
        eprintln!("  ✗ The Machine did not achieve its goal.");
        eprintln!("  Review output for diagnostics.");
        std::process::exit(1);
    }
}
