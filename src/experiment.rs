// ─── Experiment Setup — Attack/Defense Knowledge Seeding ─────────────────
//
// Seeds The Machine with attack chain rules, CVE knowledge, action
// primitives, and factual knowledge needed for the autonomous attack
// experiment.
//
// The rules form a backward-chaining graph from goal to actions.
// Each rule: IF (antecedent) THEN (consequent)
//   - antecedent = the CAUSE (what you do)
//   - consequent = the EFFECT (what it achieves)
//
// The planner chains backward from the goal:
//   1. abduce(goal) → finds rules whose CONSEQUENT matches goal
//                    → returns antecedent (the cause of the goal)
//   2. find_action_rule(cause) → finds rule whose ANTECEDENT matches
//                              → if IS_ACTION: record step
//                              → if not: abduce(cause) again
//
// Call `seed_knowledge(&mut qa, &mut brain)` after initializing.
// ────────────────────────────────────────────────────────────────────────────

use crate::qa::QaEngine;
use crate::text_encoder::{ingest_text, store_knowledge_triple};
use crate::VSABrain;

/// Seed all attack knowledge into The Machine.
pub fn seed_knowledge(qa: &mut QaEngine, brain: &mut VSABrain) {
    // ═════════════════════════════════════════════════════════════════════
    // CAUSAL CHAIN: goal → actions (backward)
    //
    // Goal: (machine, has_access_to, target_vm)
    //   Rule G: (machine, has, shell_access) → (machine, has_access_to, target_vm)
    //   Rule F: (machine, can, execute_exploit) → (machine, has, shell_access)
    //   Rule E: (machine, identifies, attack_vector) → (machine, can, execute_exploit)
    //   Rule D: (machine, knows, service_version) → (machine, identifies, attack_vector)
    //   Rule C: (machine, knows, open_service) → (machine, knows, service_version)
    // ═════════════════════════════════════════════════════════════════════

    // Rule G — the terminal rule that matches the goal
    qa.store_rule(
        "machine", "has", "shell_access",
        "machine", "has_access_to", "target_vm",
        "attack_chain",
    );

    // Rule F
    qa.store_rule(
        "machine", "can", "execute_exploit",
        "machine", "has", "shell_access",
        "attack_chain",
    );

    // Rule E
    qa.store_rule(
        "machine", "identifies", "attack_vector",
        "machine", "can", "execute_exploit",
        "attack_chain",
    );

    // Rule D
    qa.store_rule(
        "machine", "knows", "service_version",
        "machine", "identifies", "attack_vector",
        "attack_chain",
    );

    // Rule C
    qa.store_rule(
        "machine", "knows", "open_service",
        "machine", "knows", "service_version",
        "attack_chain",
    );

    // ═════════════════════════════════════════════════════════════════════
    // CVE RULES — connect known service versions to attack vectors
    //
    // These branch into Rule E's antecedent (identifies attack_vector),
    // providing alternative paths to exploit identification.
    // ═════════════════════════════════════════════════════════════════════

    // vsftpd 2.3.4 backdoor → identifies attack vector
    qa.store_rule(
        "vsftpd_2_3_4", "has_backdoor", "port_6200",
        "machine", "identifies", "attack_vector",
        "cve_2011_2523",
    );

    // Apache 2.4.49 path traversal → identifies attack vector
    qa.store_rule(
        "apache_2_4_49", "enables", "path_traversal",
        "machine", "identifies", "attack_vector",
        "cve_2021_41773",
    );

    // ═════════════════════════════════════════════════════════════════════
    // ACTION RULES — executable primitives (leaf nodes in planning)
    //
    // Each action's antecedent is the action command, and its consequent
    // is what the action achieves.  The planner stops when it finds an
    // action rule whose antecedent matches the abduced cause.
    // ═════════════════════════════════════════════════════════════════════

    // Scanning → knows open service
    qa.store_action(
        "machine", "scan_port", "target:port",
        "machine", "knows", "open_service",
        "attack_actions",
    );

    // Host scan → knows open service
    qa.store_action(
        "machine", "scan_host", "target",
        "machine", "knows", "open_service",
        "attack_actions",
    );

    // Check service version → knows service version
    qa.store_action(
        "machine", "check_service", "target:port",
        "machine", "knows", "service_version",
        "attack_actions",
    );

    // HTTP probe → knows http response
    qa.store_action(
        "machine", "probe_http", "target:port:path",
        "machine", "knows", "http_response",
        "attack_actions",
    );

    // Brute force → goal achieved (direct path, bypasses CVE chain)
    qa.store_action(
        "machine", "brute_force", "target:port:users:passwords",
        "machine", "has_access_to", "target_vm",
        "attack_actions",
    );

    // Execute command → collects output
    qa.store_action(
        "machine", "execute_command", "target:command",
        "machine", "collects", "command_output",
        "attack_actions",
    );

    // Check process → knows process state
    qa.store_action(
        "machine", "check_process", "target:name",
        "machine", "knows", "process_running",
        "attack_actions",
    );

    // ═════════════════════════════════════════════════════════════════════
    // FACTUAL KNOWLEDGE — service-port mappings and credential data
    // ═════════════════════════════════════════════════════════════════════

    qa.store_fact("ssh", "runs_on", "port_22", "standard_knowledge");
    qa.store_fact("http", "runs_on", "port_80", "standard_knowledge");
    qa.store_fact("ftp", "runs_on", "port_21", "standard_knowledge");
    qa.store_fact("https", "runs_on", "port_443", "standard_knowledge");
    qa.store_fact("vsftpd_2_3_4", "has_backdoor", "port_6200", "cve_database");
    qa.store_fact("cve_2011_2523", "affects", "vsftpd_2_3_4", "cve_database");
    qa.store_fact("cve_2021_41773", "affects", "apache_2_4_49", "cve_database");
    qa.store_fact("admin", "has_weak_password", "password123", "credential_knowledge");
    qa.store_fact("root", "has_weak_password", "toor", "credential_knowledge");

    // ═════════════════════════════════════════════════════════════════════
    // EXPERIMENT METADATA
    // ═════════════════════════════════════════════════════════════════════

    store_knowledge_triple(brain, "machine", "is_ready", "true", 1.0, "experiment_metadata");
}

/// Seed documentation about attack techniques into the brain.
pub fn seed_documentation(brain: &mut VSABrain) {
    let networking_basics = concat!(
        "Port scanning is the process of probing a target host to discover open ports. ",
        "Common ports: 22 (SSH), 80 (HTTP), 21 (FTP), 443 (HTTPS). ",
        "A port scan sends packets to a range of ports on a target and observes which ",
        "respond. Open ports indicate running services that may have vulnerabilities. ",
        "Service version detection identifies the specific software and version ",
        "running on an open port. Different versions of the same service may have ",
        "different vulnerabilities. ",
        "A vulnerability is a weakness in a system that can be exploited to gain ",
        "unauthorized access or perform unauthorized actions.",
    );

    let attack_techniques = concat!(
        "Brute force attacks try many username and password combinations against ",
        "a service like SSH until the correct credentials are found. Weak passwords ",
        "make brute force attacks faster. ",
        "Path traversal is an attack that tricks a web server into serving files ",
        "outside its document root. By using special path sequences like .., an ",
        "attacker can read sensitive files like /etc/passwd. ",
        "A backdoor is a hidden way to access a system that bypasses normal ",
        "authentication. Some versions of vsftpd contain a backdoor that opens ",
        "a shell on port 6200 when a username ending with a smiley face is sent.",
    );

    ingest_text(brain, networking_basics, "documentation");
    ingest_text(brain, attack_techniques, "documentation");
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Test that the backward chain from the goal produces a plan.
    #[test]
    fn test_attack_chain_planning() {
        let mut qa = QaEngine::new();
        let mut brain = VSABrain::new(0.12);
        seed_knowledge(&mut qa, &mut brain);

        // The goal for the experiment
        let plan = qa.plan_for_goal("machine", "has_access_to", "target_vm", 10);

        eprintln!("=== Attack Plan ===");
        eprintln!("Goal: machine has_access_to target_vm");
        eprintln!("Plan steps: {}", plan.len());
        for (i, step) in plan.iter().enumerate() {
            eprintln!("  Step {}: ({}, {}, {}) → ({}, {}, {}) [depth={}, conf={:.3}]",
                i,
                step.action.0, step.action.1, step.action.2,
                step.achieves.0, step.achieves.1, step.achieves.2,
                step.depth, step.confidence);
        }

        // Should find at least one plan step
        assert!(!plan.is_empty(),
            "Should find at least one action plan from the attack chain rules");
    }
}
