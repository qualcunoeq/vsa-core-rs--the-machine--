// ─── Experiment Setup — Attack/Defense Knowledge Seeding ─────────────────
//
// Seeds The Machine with attack chain rules, CVE knowledge, action
// primitives, and factual knowledge needed for the autonomous attack
// experiment.
//
// Call `seed_knowledge(&mut qa, &mut brain)` after initializing the
// QA engine and VSABrain, before starting the attack loop.
//
// The knowledge is organized in layers:
//   1. Attack chain rules (generic dependency graph)
//   2. CVE knowledge (specific vulnerability data)
//   3. Action rules (executable primitives — leaf nodes in planning)
//   4. Factual knowledge (service-port mappings, credential data)
// ────────────────────────────────────────────────────────────────────────────

use crate::qa::QaEngine;
use crate::text_encoder::{ingest_text, store_knowledge_triple};
use crate::VSABrain;

/// Seed all attack knowledge into The Machine.
///
/// Call this once during initialization.
pub fn seed_knowledge(qa: &mut QaEngine, brain: &mut VSABrain) {
    // ── Layer 1: Attack Chain Rules ────────────────────────────────────
    // Generic dependency graph: what leads to what in an attack.
    // The planner backward-chains from the goal through these.

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

    // ── Layer 2: CVE Knowledge ─────────────────────────────────────────
    // Specific vulnerability data connecting service versions to exploits.

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

    // ── Layer 3: Action Rules ──────────────────────────────────────────
    // Executable primitives.  The planner stops backward chaining here.

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

    // ── Layer 4: Factual Knowledge ─────────────────────────────────────
    // Service-port mappings and credential data for QA queries.

    qa.store_fact("ssh", "runs_on", "port_22", "standard_knowledge");
    qa.store_fact("http", "runs_on", "port_80", "standard_knowledge");
    qa.store_fact("ftp", "runs_on", "port_21", "standard_knowledge");
    qa.store_fact("https", "runs_on", "port_443", "standard_knowledge");
    qa.store_fact("vsftpd_2_3_4", "has_backdoor", "port_6200", "cve_database");
    qa.store_fact("cve_2011_2523", "affects", "vsftpd_2_3_4", "cve_database");
    qa.store_fact("cve_2021_41773", "affects", "apache_2_4_49", "cve_database");
    qa.store_fact("admin", "has_weak_password", "password123", "credential_knowledge");
    qa.store_fact("root", "has_weak_password", "toor", "credential_knowledge");

    // ── Experiment metadata ─────────────────────────────────────────────
    store_knowledge_triple(brain, "machine", "is_ready", "true", 1.0, "experiment_metadata");
}

/// Seed documentation about attack techniques into the brain.
///
/// Gives The Machine semantic context it can retrieve via cross-domain queries.
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
