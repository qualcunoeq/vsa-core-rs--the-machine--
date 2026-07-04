// ─── Knowledge Seeding — Run Before the Experiment ───────────────────────
//
// This code seeds The Machine with attack knowledge, vulnerability data,
// and action rules.  Call it once before starting the experiment.
//
// The Machine can't reason about things it doesn't know.  This gives it
// the causal chains it needs to plan multi-step attacks.
//
// Usage:
//   seed_knowledge(&mut qa, &mut brain);
//   seed_documentation(&mut brain);
// ────────────────────────────────────────────────────────────────────────────

use the_machine::qa::QaEngine;
use the_machine::text_encoder::{ingest_text, store_knowledge_triple};
use the_machine::VSABrain;

/// Seed all attack, defense, and action knowledge into The Machine.
///
/// Call this once during initialization, before `run_attack_loop()`.
pub fn seed_knowledge(qa: &mut QaEngine, brain: &mut VSABrain) {
    // ── Attack Chain Rules (generic) ────────────────────────────────────
    // These encode the dependency graph of a multi-step attack.
    // The planner backward-chains from the goal through these rules
    // to discover the sequence of actions needed.

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
    qa.store_rule(
        "shell_access", "enables", "data_exfiltration",
        "machine", "exfiltrates", "sensitive_data",
        "attack_knowledge",
    );

    // ── CVE Knowledge (specific vulnerability data) ──────────────────────
    // These connect service versions to known vulnerabilities.

    // vsftpd 2.3.4 backdoor
    qa.store_rule(
        "vsftpd_2_3_4", "has", "backdoor_on_port_6200",
        "port_6200", "grants", "shell_access",
        "cve_2011_2523",
    );

    // Apache 2.4.49 path traversal (CVE-2021-41773)
    qa.store_rule(
        "apache_2_4_49", "enables", "path_traversal",
        "path_traversal", "reads", "sensitive_files",
        "cve_2021_41773",
    );

    // SSH weak credential
    qa.store_rule(
        "ssh_weak_credential", "enables", "brute_force_success",
        "brute_force_success", "gives", "shell_access",
        "weak_credential_knowledge",
    );

    // ── Action Rules (executable primitives) ────────────────────────────
    // These are the leaf nodes in the planning graph.  The planner stops
    // backward chaining when it reaches an action rule.

    // Scanning
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

    // HTTP probing
    qa.store_action(
        "machine", "probe_http", "target:port:path",
        "machine", "knows", "http_response",
        "attack_actions",
    );

    // Brute force
    qa.store_action(
        "machine", "brute_force", "target:port:users:passwords",
        "machine", "has", "shell_access",
        "attack_actions",
    );

    // Process checking
    qa.store_action(
        "machine", "check_process", "target:name",
        "machine", "knows", "process_running",
        "attack_actions",
    );

    // Command execution (post-exploitation)
    qa.store_action(
        "machine", "execute_command", "target:command",
        "machine", "collects", "command_output",
        "attack_actions",
    );

    // ── Factual Knowledge (stored as facts for QA) ──────────────────────
    // These let the QA engine answer "what is running on port 21?" etc.

    // Service-port mappings
    qa.store_fact("ssh", "runs_on", "port_22", "standard_knowledge");
    qa.store_fact("http", "runs_on", "port_80", "standard_knowledge");
    qa.store_fact("ftp", "runs_on", "port_21", "standard_knowledge");
    qa.store_fact("https", "runs_on", "port_443", "standard_knowledge");

    // Vulnerability-service mappings
    qa.store_fact("vsftpd_2_3_4", "has_backdoor", "port_6200", "cve_database");
    qa.store_fact("cve_2011_2523", "affects", "vsftpd_2_3_4", "cve_database");
    qa.store_fact("cve_2021_41773", "affects", "apache_2_4_49", "cve_database");
    qa.store_fact("path_traversal", "exposes", "/etc/passwd", "cve_database");

    // Credential knowledge
    qa.store_fact("admin", "has_weak_password", "password123", "credential_knowledge");
    qa.store_fact("root", "has_weak_password", "toor", "credential_knowledge");

    // ── Goal Definition ─────────────────────────────────────────────────
    // The ultimate goal: "machine has_access_to target_vm"
    // This is what you pass to run_attack_loop:
    //   run_attack_loop(brain, qa, actuator, ("machine", "has_access_to", "target_vm"), 20).await;
    //
    // The goal must be achievable through existing rules.  Verifying:
    //   machine has_access_to target_vm
    //     ← exploit gives shell_access  (rule)
    //       ← vulnerability enables exploit  (rule)
    //         ← service_version has vulnerability  (rule)
    //           ← open_service has version  (rule)
    //             ← scan_result reveals open_service  (rule)
    //               ← machine scan_port target:port  (ACTION)

    // The `machine` and `target_vm` entities need to exist in the brain
    store_knowledge_triple(brain, "experiment", "has", "begun", 1.0, "experiment_metadata");
    store_knowledge_triple(brain, "machine", "is_ready", "true", 1.0, "experiment_metadata");
    store_knowledge_triple(brain, "target_vm", "ip", "192.168.100.10", 1.0, "experiment_metadata");
}

/// Seed documentation about attack techniques and concepts.
///
/// This gives The Machine semantic knowledge it can retrieve via
/// cross-domain queries and text-based reasoning.
pub fn seed_documentation(brain: &mut VSABrain) {
    let networking_basics = r#"
Port scanning is the process of probing a target host to discover open ports.
Common ports: 22 (SSH), 80 (HTTP), 21 (FTP), 443 (HTTPS).
A port scan sends packets to a range of ports on a target and observes which
respond. Open ports indicate running services that may have vulnerabilities.

Service version detection identifies the specific software and version
running on an open port. Different versions of the same service may have
different vulnerabilities.

A vulnerability is a weakness in a system that can be exploited to gain
unauthorized access or perform unauthorized actions.
    "#;

    let attack_techniques = r#"
Brute force attacks try many username and password combinations against
a service like SSH until the correct credentials are found. Weak passwords
make brute force attacks faster.

Path traversal is an attack that tricks a web server into serving files
outside its document root. By using special path sequences like ../, an
attacker can read sensitive files like /etc/passwd.

A backdoor is a hidden way to access a system that bypasses normal
authentication. Some versions of vsftpd contain a backdoor that opens
a shell on port 6200 when a username ending with a smiley face is sent.
    "#;

    let defensive_techniques = r#"
Port scans can be detected by monitoring for connections to many different
ports from a single IP address. A firewall can block unnecessary ports.

Failed login attempts should be logged and monitored. Multiple failures
from the same IP indicate a brute force attack in progress.

System files should not be world-readable. The /etc/passwd file contains
user account information. The /etc/shadow file contains password hashes
and should only be readable by root.
    "#;

    ingest_text(brain, networking_basics, "documentation");
    ingest_text(brain, attack_techniques, "documentation");
    ingest_text(brain, defensive_techniques, "documentation");
}
