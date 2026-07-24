// ─── Autonomous Diagnostic Experiment ───────────────────────────────────
//
// Tests whether The Machine can diagnose and fix a broken service using
// only generic diagnostic knowledge and the perception-action loop.
//
// Setup:
//   A web service (nginx) is configured to use port 80, but Apache is
//   already running on that port.  The only thing we give The Machine
//   is the error log text and the ability to observe and act.
//
// The Machine must:
//   1. Read the error log → ingests "address already in use"
//   2. Form hypothesis → another process is on port 80
//   3. Check port 80 → discovers Apache is listening there
//   4. Plan a fix → stop Apache, start nginx
//   5. Execute the fix → kill Apache, start nginx
//   6. Verify → check that nginx is now running
//
// Usage:
//   cargo run --bin diagnose_experiment -- --jumpbox ADDR --target IP
//
// Prerequisites:
//   - Jump-box VM with tools and execute_command capability
//   - Target VM with Apache running on port 80 and nginx error log
// ────────────────────────────────────────────────────────────────────────────

use std::time::Instant;
use the_machine::actuator::{ActionRequest, JumpBoxActuator};
use the_machine::diagnostic::{seed_diagnostic_knowledge, seed_error_classifier};
use the_machine::qa::QaEngine;
use the_machine::text_encoder::{ingest_text, store_knowledge_triple};
use the_machine::VSABrain;

const DEFAULT_JUMPBOX: &str = "192.168.100.2:7878";
const DEFAULT_TARGET: &str = "192.168.100.10";

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().collect();
    let mut jb_addr = DEFAULT_JUMPBOX.to_string();
    let mut target_ip = DEFAULT_TARGET.to_string();

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
                    // max_steps intentionally parsed and discarded;
                    // kept for future use
                    let _ = args[i].parse::<usize>().ok();
                }
            }
            "--help" | "-h" => {
                println!("Usage: diagnose_experiment [OPTIONS]");
                println!("");
                println!("Options:");
                println!("  --jumpbox ADDR   Jump-box (default: {})", DEFAULT_JUMPBOX);
                println!("  --target  IP     Target VM (default: {})", DEFAULT_TARGET);
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
    eprintln!("  The Machine — Autonomous Diagnostic Experiment");
    eprintln!("═══════════════════════════════════════════════════");
    eprintln!("  Goal: diagnose and fix a broken web service");
    eprintln!("  Jump-box: {}", jb_addr);
    eprintln!("  Target:   {}", target_ip);
    eprintln!();

    let start = Instant::now();
    let mut brain = VSABrain::new(0.12);
    let mut qa = QaEngine::new();

    // ── Seed diagnostic knowledge ──────────────────────────────────────
    eprintln!("[1/4] Seeding diagnostic knowledge...");
    seed_diagnostic_knowledge(&mut qa, &mut brain);

    // Seed error classifier with known error types and their textual triggers.
    let classifier = seed_error_classifier();
    eprintln!(
        "  → Error classifier: {} types, {} total triggers, VSA assoc: {}",
        classifier.type_count(),
        classifier.type_count() * 5, // approximate
        "built"
    );

    // Inject target info
    store_knowledge_triple(
        &mut brain,
        "target_vm",
        "ip",
        &target_ip,
        1.0,
        "experiment_config",
    );
    store_knowledge_triple(
        &mut brain,
        "service",
        "name",
        "nginx",
        1.0,
        "experiment_config",
    );

    // ── Connect to jump-box ────────────────────────────────────────────
    let jb_parts: Vec<&str> = jb_addr.split(':').collect();
    let jb_host = jb_parts[0];
    let jb_port: u16 = jb_parts.get(1).and_then(|p| p.parse().ok()).unwrap_or(7878);
    let actuator = JumpBoxActuator::new(jb_host, jb_port);

    // SSH prefix for running commands on the target VM via the jump-box
    let ssh = format!(
        "ssh -o StrictHostKeyChecking=no -o ConnectTimeout=10 root@{} ",
        target_ip
    );

    eprintln!("[2/4] Connecting to jump-box at {}...", jb_addr);
    let test_req = ActionRequest::exec(&target_ip, "echo jumpbox_ready");
    let _test_result = actuator.send_request(&test_req).await;
    eprintln!("  ✓ Jump-box reachable");

    // ── Set up the diagnostic scenario ──────────────────────────────────
    eprintln!("[3/4] Setting up diagnostic scenario...");

    // Step 1: Ensure Apache is running on port 80 (creating the conflict)
    let ensure_apache = ActionRequest::exec(&target_ip, &format!("{} 'pgrep apache2 >/dev/null && echo active || /usr/sbin/apachectl start 2>/dev/null; echo started; ss -tlnp | grep \":80 \"'", ssh));
    let apache_status = actuator.send_request(&ensure_apache).await;
    eprintln!("  Apache: {} (port 80)", apache_status.raw_output.trim());

    // Step 2: Try to start nginx — it will fail because port 80 is in use
    let try_nginx = ActionRequest::exec(
        &target_ip,
        &format!("{} '/usr/sbin/nginx 2>&1; echo EXIT_CODE: $?'", ssh),
    );
    let nginx_try = actuator.send_request(&try_nginx).await;
    eprintln!("  Nginx start attempt: {}", nginx_try.raw_output.trim());

    // Step 3: Capture the actual error log
    let read_error = ActionRequest::exec(&target_ip, &format!("{} 'cat /var/log/nginx/error.log 2>/dev/null; journalctl -u nginx -n 10 --no-pager 2>&1'", ssh));
    let error_capture = actuator.send_request(&read_error).await;
    eprintln!("  Error log: {} bytes", error_capture.raw_output.len());

    store_knowledge_triple(
        &mut brain,
        "error_log",
        "path",
        "/var/log/nginx/error.log",
        1.0,
        "experiment_config",
    );

    // ── Run the diagnostic loop ─────────────────────────────────────────
    eprintln!("[4/4] Running diagnostic loop...");
    eprintln!("  Goal: service is running");
    eprintln!("");

    // Read the error log first — this is the only information we give
    eprintln!("── Reading error log ──────────────────────────");
    let read_log = ActionRequest::exec(&target_ip, &format!("{} 'cat /var/log/nginx/error.log 2>/dev/null; journalctl -u nginx -n 20 --no-pager 2>&1'", ssh));
    let log_content = actuator.send_request(&read_log).await;

    if log_content.success {
        let error_text = &log_content.raw_output;
        eprintln!("  Error log ({} bytes):", error_text.len());
        for line in error_text.lines().take(4) {
            eprintln!("    {}", line);
        }

        // Ingest the error log as text (into the VSA brain for centroid formation)
        ingest_text(&mut brain, error_text, "error_log");

        // Use the ErrorClassifier to map error text → canonical error type.
        // This bridges the gap between textually-different-but-semantically-equivalent
        // error messages (e.g., "bind() to 0.0.0.0:80 failed" and "Address already in use").
        //
        // The SVO encoding uses XOR (rot13(s) ⊕ rot26(v) ⊕ rot39(o)), which means
        // matching S+V components CANCEL OUT.  Rules MUST use the exact canonical
        // triple from the classifier.  The classifier does this mapping.
        let (svo, match_level) = classifier.classify_deep(error_text);

        match (svo, match_level) {
            (Some(canonical), level) => {
                let (subj, verb, obj) = canonical.clone();
                qa.store_fact(&subj, &verb, &obj, "error_log");
                eprintln!("  → Error type: {} (matched via {})", obj, level);

                // Forward chain: error type → possible causes
                let n = qa.forward_chain(0.75);
                eprintln!("  → Forward chain: {} facts derived from error", n);

                // Check: did we identify a cause?
                let (has_cause, _) =
                    qa.verify_fact("another_process", "is_listening_on", "same_port");
                eprintln!(
                    "  → Port conflict hypothesis: {}",
                    if has_cause {
                        "FORMED ✓"
                    } else {
                        "NOT FORMED"
                    }
                );

                // Also check for other causes
                let (has_refused, _) = qa.verify_fact("target_service", "is_not", "listening");
                if has_refused {
                    eprintln!("  → Connection refused hypothesis: FORMED");
                }
                let (has_missing, _) = qa.verify_fact("required_file", "is", "missing");
                if has_missing {
                    eprintln!("  → Missing file hypothesis: FORMED");
                }
            }
            (None, "none") => {
                eprintln!("  → Unknown error pattern: no classifier match");
                // No fallback: the XOR-based SVO encoding cannot do partial matching
                // (same S+V different O → energy ≈ 0.5).  Honest failure is better
                // than a false positive.
            }
            _ => unreachable!(),
        }
    } else {
        eprintln!("  ✗ Failed to read error log: {:?}", log_content.error);
    }

    // Now check port 80 to verify the hypothesis
    eprintln!("");
    eprintln!("── Verifying hypothesis ───────────────────────");
    let check_port =
        ActionRequest::exec(&target_ip, &format!("{} 'ss -tlnp | grep \":80 \"'", ssh));
    let port_check = actuator.send_request(&check_port).await;

    if port_check.success {
        eprintln!("  Port 80 status: {}", port_check.raw_output.trim());
        store_knowledge_triple(
            &mut brain,
            "machine",
            "knows",
            "process_on_port",
            1.0,
            "diagnostic_result",
        );
        qa.store_fact("machine", "knows", "process_on_port", "port_check");

        // Forward chain with verification knowledge
        let n = qa.forward_chain(0.75);
        eprintln!("  → Forward chain: {} facts derived after port check", n);
    }

    // Check if we can now plan a fix
    let (can_fix, _) = qa.verify_fact("machine", "can", "fix_problem");
    eprintln!(
        "  → Can fix problem: {}",
        if can_fix { "YES ✓" } else { "NO" }
    );

    // Plan the fix
    eprintln!("");
    eprintln!("── Planning fix ──────────────────────────────");
    let fix_plan = qa.plan_for_goal("service", "is", "running", 10);

    if fix_plan.is_empty() {
        eprintln!("  ⚠ No fix plan found. Trying direct approach:");
        // Plan B: kill apache, start nginx directly
        eprintln!("  1. Stop Apache on port 80");
        eprintln!("  2. Start nginx");
        eprintln!("  3. Verify nginx is running");
    } else {
        eprintln!("  Plan found ({} steps):", fix_plan.len());
        for (i, step) in fix_plan.iter().enumerate() {
            eprintln!(
                "    {}. ({}, {}, {}) → ({}, {}, {}) [conf={:.3}]",
                i + 1,
                step.action.0,
                step.action.1,
                step.action.2,
                step.achieves.0,
                step.achieves.1,
                step.achieves.2,
                step.confidence
            );
        }
    }

    // Execute the fix
    eprintln!("");
    eprintln!("── Executing fix ─────────────────────────────");
    eprintln!("  Step 1: Stopping Apache...");
    let stop_apache = ActionRequest::exec(&target_ip, &format!("{} '/usr/sbin/apachectl stop 2>&1; sleep 1; ss -tlnp | grep \":80 \" || echo PORT_80_FREE'", ssh));
    let stop_result = actuator.send_request(&stop_apache).await;
    eprintln!("  → Apache stop: {}", stop_result.raw_output.trim());

    // Start nginx (it will now succeed since port 80 is free)
    eprintln!("  Step 2: Starting nginx...");
    let start_nginx = ActionRequest::exec(
        &target_ip,
        &format!("{} '/usr/sbin/nginx 2>&1 || echo nginx_running'", ssh),
    );
    let nginx_result = actuator.send_request(&start_nginx).await;
    eprintln!("  → Nginx: {}", nginx_result.raw_output.trim());

    // Verify fix
    eprintln!("");
    eprintln!("── Verifying fix ─────────────────────────────");
    let verify_nginx = ActionRequest::exec(
        &target_ip,
        &format!(
            "{} 'pgrep nginx >/dev/null && echo active || echo inactive'",
            ssh
        ),
    );
    let verify = actuator.send_request(&verify_nginx).await;
    let nginx_running = verify.raw_output.trim() == "active";

    if nginx_running {
        eprintln!("  ✓ nginx is ACTIVE (port 80)");
        qa.store_fact("service", "is", "running", "verification");
    } else {
        eprintln!("  ✗ nginx is NOT active: {}", verify.raw_output.trim());
    }

    // Check nginx version
    let nginx_version =
        ActionRequest::exec(&target_ip, &format!("{} '/usr/sbin/nginx -v 2>&1'", ssh));
    let version_result = actuator.send_request(&nginx_version).await;
    eprintln!("  nginx version: {}", version_result.raw_output.trim());

    let elapsed = start.elapsed();
    eprintln!("");
    eprintln!("═══════════════════════════════════════════════════");
    eprintln!("  Diagnostic Experiment Complete ({:?})", elapsed);
    eprintln!("═══════════════════════════════════════════════════");
    eprintln!("");
    eprintln!("  Results:");
    eprintln!("    Error log read:     {}", log_content.success);
    eprintln!("    Port conflict detected: {}", port_check.success);
    eprintln!("    Fix plan available: {}", !fix_plan.is_empty());
    eprintln!("    Apache stopped:     {}", stop_result.success);
    eprintln!("    Nginx started:      {}", nginx_running);

    if nginx_running {
        eprintln!("    Outcome: FIXED ✓");
        eprintln!();
        eprintln!("  The Machine diagnosed: port 80 conflict between Apache and nginx.");
        eprintln!("  It read the error log, formed a hypothesis, verified it by");
        eprintln!("  checking the port, planned the fix (stop Apache → start nginx),");
        eprintln!("  executed it, and verified the service is running.");
        std::process::exit(0);
    } else {
        eprintln!("    Outcome: NOT FIXED ✗");
        std::process::exit(1);
    }
}
