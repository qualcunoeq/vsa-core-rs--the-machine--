// ─── Diagnostic Knowledge — Generic Reasoning About Broken Systems ─────
//
// These rules encode HOW to diagnose, not WHAT to diagnose.
// The same rules work for any service that fails to start because a
// port is in use, a config is wrong, or a dependency is missing.
//
// The diagnostic loop:
//   1. Read error log → extract error pattern
//   2. Form hypothesis about cause (abduction via causal rules)
//   3. Verify hypothesis by checking system state
//   4. Plan and execute fix actions
//   5. Verify fix succeeded
//
// All rules are generic — they describe diagnostic strategies, not
// specific knowledge about nginx, Apache, or any particular system.
// ────────────────────────────────────────────────────────────────────────────

use crate::qa::QaEngine;
use crate::text_encoder::{ingest_text, store_knowledge_triple};
use crate::VSABrain;

/// Seed generic diagnostic knowledge into The Machine.
///
/// Rules are organized in layers:
///   1. Error pattern → cause (what does this error mean?)
///   2. Cause → verification (how do I check if this cause is real?)
///   3. Verified cause → fix action (what do I do about it?)
///   4. Fix action → goal state (what does success look like?)
pub fn seed_diagnostic_knowledge(qa: &mut QaEngine, brain: &mut VSABrain) {
    // ═════════════════════════════════════════════════════════════════════
    // LAYER 1: Error Pattern → Possible Cause
    //
    // These are abductive rules: given an observed error, what might
    // have caused it?  Multiple causes per error pattern are allowed.
    // ═════════════════════════════════════════════════════════════════════

    // "bind() failed" → port conflict (catches both "Address already in use"
    // and "Unknown error" — error 98 is EADDRINUSE on Linux either way)
    qa.store_rule(
        "error", "contains", "bind_failed",
        "another_process", "is_listening_on", "same_port",
        "diagnostic_port_conflict",
    );

    // Generic connection error → service not listening
    qa.store_rule(
        "error", "contains", "connection_refused",
        "target_service", "is_not", "listening",
        "diagnostic_connection",
    );

    // "File not found" → missing dependency or config
    qa.store_rule(
        "error", "contains", "no_such_file",
        "required_file", "is", "missing",
        "diagnostic_missing_file",
    );

    // "Permission denied" → wrong file permissions
    qa.store_rule(
        "error", "contains", "permission_denied",
        "file_permissions", "are", "incorrect",
        "diagnostic_permissions",
    );

    // "Failed" (generic) → service has a startup problem
    qa.store_rule(
        "error", "contains", "failed",
        "service", "has", "startup_problem",
        "diagnostic_startup",
    );

    // Generic: service not running → something is wrong
    qa.store_rule(
        "service", "is_not", "running",
        "service", "has", "startup_problem",
        "diagnostic_generic",
    );

    // ═════════════════════════════════════════════════════════════════════
    // LAYER 2: Cause → Verification Action
    //
    // Given a hypothesized cause, what action do I take to verify it?
    // These are action rules — the action tells me if my hypothesis
    // is correct by revealing the actual system state.
    // ═════════════════════════════════════════════════════════════════════

    // To check what's on a port, run ss/lsof/port check
    qa.store_action(
        "machine", "check_port", "target:port",
        "machine", "knows", "process_on_port",
        "diagnostic_actions",
    );

    // To check if a service is running, check its process
    qa.store_action(
        "machine", "check_service_running", "target:name",
        "machine", "knows", "service_status",
        "diagnostic_actions",
    );

    // To read an error log, cat the file
    qa.store_action(
        "machine", "read_error_log", "target:path",
        "machine", "knows", "error_content",
        "diagnostic_actions",
    );

    // ═════════════════════════════════════════════════════════════════════
    // LAYER 3: Verified Cause → Fix Action
    //
    // Once a cause is confirmed, what action fixes it?
    // ═════════════════════════════════════════════════════════════════════

    // Port conflict → kill the process on that port, then restart
    qa.store_action(
        "machine", "free_port_and_restart", "target:port:service",
        "machine", "has", "fixed_port_conflict",
        "diagnostic_actions",
    );

    // Missing file → check what's missing and create it
    qa.store_action(
        "machine", "resolve_missing_file", "target:path:content",
        "machine", "has", "fixed_missing_file",
        "diagnostic_actions",
    );

    // Bad permissions → fix permissions
    qa.store_action(
        "machine", "fix_permissions", "target:path:perms",
        "machine", "has", "fixed_permissions",
        "diagnostic_actions",
    );

    // Generic service restart
    qa.store_action(
        "machine", "restart_service", "target:name",
        "service", "is", "running",
        "diagnostic_actions",
    );

    // ═════════════════════════════════════════════════════════════════════
    // LAYER 4: Causal chain linking diagnostics to the goal
    //
    // Goal: (service, is, running)
    //   The plan backward-chains from this goal through the rules
    //   below to find the right diagnostic and fix actions.
    // ═════════════════════════════════════════════════════════════════════

    // If the machine knows the error content, it can identify the cause
    qa.store_rule(
        "machine", "knows", "error_content",
        "machine", "identifies", "possible_cause",
        "diagnostic_chain",
    );

    // If the machine knows what process is on the port, it can confirm
    qa.store_rule(
        "machine", "knows", "process_on_port",
        "machine", "confirms", "cause",
        "diagnostic_chain",
    );

    // If cause is confirmed, machine can fix it
    qa.store_rule(
        "machine", "confirms", "cause",
        "machine", "can", "fix_problem",
        "diagnostic_chain",
    );

    // If problem is fixed, service is running
    qa.store_rule(
        "machine", "has", "fixed_port_conflict",
        "service", "is", "running",
        "diagnostic_chain",
    );

    // Direct: restart service → service is running
    qa.store_rule(
        "machine", "restarts", "service",
        "service", "is", "running",
        "diagnostic_chain",
    );

    // ═════════════════════════════════════════════════════════════════════
    // DOCUMENTATION — generic diagnostic knowledge as text
    // ═════════════════════════════════════════════════════════════════════

    let diagnostic_text = concat!(
        "When a service fails to start, the error log contains information about what went wrong. ",
        "Common errors include: address already in use (another process is using the port), ",
        "file not found (a configuration file or dependency is missing), ",
        "permission denied (the process cannot access a file it needs), ",
        "and connection refused (a service it depends on is not running). ",
        "To diagnose: read the error log, check which process is on the conflicting port, ",
        "verify the configuration file exists, and check that all dependencies are running. ",
        "To fix a port conflict: stop the process using the port, then restart the target service.",
    );

    ingest_text(brain, diagnostic_text, "diagnostic_knowledge");

    // ── Experiment metadata ─────────────────────────────────────────────
    store_knowledge_triple(brain, "diagnostic_system", "is_ready", "true", 1.0, "experiment_metadata");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_diagnostic_chain_planning() {
        let mut qa = QaEngine::new();
        let mut brain = VSABrain::new(0.12);
        seed_diagnostic_knowledge(&mut qa, &mut brain);

        // Goal: service is running — what diagnostic actions are available?
        let plan = qa.plan_for_goal("service", "is", "running", 10);

        eprintln!("=== Diagnostic Plan ===");
        eprintln!("Goal: service is running");
        eprintln!("Plan steps: {}", plan.len());
        for (i, step) in plan.iter().enumerate() {
            eprintln!("  Step {}: ({}, {}, {}) → ({}, {}, {}) [depth={}, conf={:.3}]",
                i,
                step.action.0, step.action.1, step.action.2,
                step.achieves.0, step.achieves.1, step.achieves.2,
                step.depth, step.confidence);
        }

        assert!(!plan.is_empty(),
            "Should find at least one action to diagnose/restart service");
    }

    #[test]
    fn test_diagnostic_forward_chain() {
        let mut qa = QaEngine::new();
        let mut brain = VSABrain::new(0.12);
        seed_diagnostic_knowledge(&mut qa, &mut brain);

        // Simulate: we read an error log that contains "address_already_in_use"
        qa.store_fact("error", "contains", "address_already_in_use", "simulated_log");

        // Forward chain should derive: another_process is_listening_on same_port
        let n = qa.forward_chain(0.75);
        eprintln!("  Forward chain derived {} facts from error pattern", n);

        // The cause should now be known
        let (verified, _conf) = qa.verify_fact("another_process", "is_listening_on", "same_port");
        assert!(verified, "Forward chain should derive the port conflict cause");

        // Now simulate checking the port and confirming the conflict
        qa.store_fact("machine", "knows", "process_on_port", "simulated_check");

        let n2 = qa.forward_chain(0.75);
        eprintln!("  Forward chain derived {} more facts after port check", n2);

        // Should now have confirmed cause and be able to fix
        let (can_fix, _) = qa.verify_fact("machine", "can", "fix_problem");
        assert!(can_fix, "Forward chain should conclude fix is possible");
    }
}
