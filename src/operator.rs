//! Phase 11 — dependable release packaging.
//!
//! This module turns the research runtime into an operator-friendly product
//! without moving any reasoning logic.  It adds five things on top of the
//! existing conversation service:
//!
//! * a **versioned capability inventory** — what the build can do, since when,
//!   which claim and evaluation artifact back it, and how to exercise it;
//! * **configuration** with explicit validation, so a bad setting is reported
//!   with a remedy instead of surfacing as a confusing failure later;
//! * a **doctor** report for health and dependency status (toolchain, storage,
//!   database integrity and schema, port availability, profile/feature match);
//! * **profiles** for the optional GPU and semantic-worker configurations, with
//!   honest checks of what the running binary was actually built with;
//! * **release notes tied to evaluation artifacts** and an **upgrade / backup /
//!   restore / recover** path that is safe to run unattended.
//!
//! Everything here is deterministic and side-effect-free except the explicitly
//! named filesystem operations (`setup`, `backup`, `restore`, `upgrade`,
//! `recover`, `write_release_artifacts`).  The module never starts a server.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::time::Duration;

use crate::persistence::SCHEMA_VERSION;
use crate::reliability::{projection_path, ProjectionPath};

/// Schema tag for the operator surfaces (config, doctor, release notes).
pub const OPERATOR_SCHEMA: &str = "phase11-operator-v1";
/// The release this build represents.  Tied to `README.md` / `CURRENT_STATE.md`.
pub const RELEASE_TAG: &str = "v3.4";
/// Date the Phase 11 release was packaged.
pub const RELEASE_DATE: &str = "2026-09-28";
/// Schema tag for the capability inventory artifact.
pub const INVENTORY_SCHEMA: &str = "phase11-capability-inventory-v1";
/// Schema tag for the doctor artifact.
pub const DOCTOR_SCHEMA: &str = "phase11-doctor-v1";
/// Schema tag for the release-notes artifact.
pub const RELEASE_NOTES_SCHEMA: &str = "phase11-release-notes-v1";

/// The one documented startup command.
pub const fn startup_command() -> &'static str {
    "cargo run --release --bin machine -- start"
}

// ─── Capability inventory ──────────────────────────────────────────────────

/// How ready a capability is for everyday use.
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CapabilityStatus {
    /// Exercised in daily operation and covered by the frozen acceptance sets.
    Stable,
    /// Working and evaluated, still growing.
    Active,
    /// Working but explicitly early; expect change.
    Experimental,
    /// Runs alongside the main path without authorizing an answer.
    Shadow,
    /// Documented and deliberately out of scope for now.
    Deferred,
}

impl CapabilityStatus {
    pub fn label(self) -> &'static str {
        match self {
            CapabilityStatus::Stable => "stable",
            CapabilityStatus::Active => "active",
            CapabilityStatus::Experimental => "experimental",
            CapabilityStatus::Shadow => "shadow",
            CapabilityStatus::Deferred => "deferred",
        }
    }
}

/// One versioned capability.  `claim` and `evidence` tie it to the claim
/// ledger and the measured evaluation artifacts, so the inventory cannot drift
/// away from what was actually verified.
#[derive(Clone, Copy, Debug, serde::Serialize)]
pub struct CapabilityEntry {
    pub id: &'static str,
    pub title: &'static str,
    pub status: CapabilityStatus,
    /// Release or phase that introduced the capability.
    pub since: &'static str,
    /// Claim id in `docs/CLAIMS.md`, when one exists.
    pub claim: Option<&'static str>,
    /// Committed evaluation artifacts that back the capability.
    pub evidence: &'static [&'static str],
    /// The command a user runs to exercise it.
    pub command: &'static str,
    /// Surfaces the capability is reachable through.
    pub surfaces: &'static [&'static str],
}

/// The versioned capability inventory.
pub const CAPABILITIES: &[CapabilityEntry] = &[
    CapabilityEntry {
        id: "conversation-runtime",
        title: "Conversational runtime",
        status: CapabilityStatus::Stable,
        since: "Phase 1",
        claim: None,
        evidence: &["docs/DEVELOPMENT.md"],
        command: "machine start",
        surfaces: &["library", "http"],
    },
    CapabilityEntry {
        id: "local-web-interface",
        title: "Local web interface (Ask / Teach / Inspect)",
        status: CapabilityStatus::Stable,
        since: "Phase 2",
        claim: None,
        evidence: &["docs/DEVELOPMENT.md"],
        command: "machine start --open",
        surfaces: &["http"],
    },
    CapabilityEntry {
        id: "durable-state",
        title: "Durable SQLite state with migrations",
        status: CapabilityStatus::Stable,
        since: "Phase 3",
        claim: Some("C-018"),
        evidence: &["docs/phase7_reliability_v1.report.json"],
        command: "machine doctor",
        surfaces: &["library"],
    },
    CapabilityEntry {
        id: "backup-restore",
        title: "Backup and restore",
        status: CapabilityStatus::Stable,
        since: "Phase 3",
        claim: None,
        evidence: &["docs/DEVELOPMENT.md"],
        command: "machine backup <file> / machine restore <file>",
        surfaces: &["cli"],
    },
    CapabilityEntry {
        id: "follow-up-conversation",
        title: "Follow-up conversation and reference resolution",
        status: CapabilityStatus::Stable,
        since: "Phase 4",
        claim: None,
        evidence: &["integration-tests/conversation_runtime/scenarios/phase4.json"],
        command: "machine start",
        surfaces: &["http"],
    },
    CapabilityEntry {
        id: "connected-capabilities",
        title: "Connected typed math and unit capabilities",
        status: CapabilityStatus::Stable,
        since: "Phase 5",
        claim: None,
        evidence: &["docs/DEVELOPMENT.md"],
        command: "machine start",
        surfaces: &["http"],
    },
    CapabilityEntry {
        id: "semantic-fidelity",
        title: "Semantic fidelity measurement",
        status: CapabilityStatus::Shadow,
        since: "Phase 6",
        claim: None,
        evidence: &["docs/semantic_fidelity_eval_v1.report.json"],
        command: "semantic_fidelity_eval",
        surfaces: &["library"],
    },
    CapabilityEntry {
        id: "semantic-worker-shadow",
        title: "Semantic worker in shadow mode",
        status: CapabilityStatus::Shadow,
        since: "Phase 6",
        claim: None,
        evidence: &["docs/semantic_worker_probe.md"],
        command: "machine start --semantic-worker",
        surfaces: &["http", "config"],
    },
    CapabilityEntry {
        id: "reliability-contracts",
        title: "Enforced memory budgets and replay taxonomy",
        status: CapabilityStatus::Active,
        since: "Phase 7",
        claim: Some("C-018"),
        evidence: &["docs/phase7_reliability_v1.report.json"],
        command: "reliability_report",
        surfaces: &["library"],
    },
    CapabilityEntry {
        id: "document-learning",
        title: "Inspectable, reversible document learning",
        status: CapabilityStatus::Active,
        since: "Phase 8",
        claim: Some("C-020"),
        evidence: &["docs/DEVELOPMENT.md"],
        command: "machine_docs demo",
        surfaces: &["library", "http", "cli"],
    },
    CapabilityEntry {
        id: "scanned-documents",
        title: "Scanned / visual document interpretation",
        status: CapabilityStatus::Deferred,
        since: "Phase 8",
        claim: None,
        evidence: &[],
        command: "(deferred)",
        surfaces: &[],
    },
    CapabilityEntry {
        id: "conversation-evaluation",
        title: "Versioned conversation evaluation and ablation gate",
        status: CapabilityStatus::Stable,
        since: "Phase 9",
        claim: Some("C-021"),
        evidence: &["docs/phase9_conversation_eval_v1.report.json"],
        command: "machine_eval",
        surfaces: &["cli", "library"],
    },
    CapabilityEntry {
        id: "controlled-autonomy",
        title: "Bounded background tasks with scope and budget",
        status: CapabilityStatus::Experimental,
        since: "Phase 10",
        claim: Some("C-022"),
        evidence: &["docs/phase10_autonomy_v1.report.json"],
        command: "machine_eval task",
        surfaces: &["library"],
    },
    CapabilityEntry {
        id: "operator-cli",
        title: "Single operator command",
        status: CapabilityStatus::Stable,
        since: "Phase 11",
        claim: Some("C-023"),
        evidence: &["docs/phase11_release_notes_v1.md"],
        command: startup_command(),
        surfaces: &["cli"],
    },
    CapabilityEntry {
        id: "configuration-validation",
        title: "Configuration validation with remedies",
        status: CapabilityStatus::Stable,
        since: "Phase 11",
        claim: Some("C-023"),
        evidence: &["docs/phase11_doctor_v1.json"],
        command: "machine config check",
        surfaces: &["cli"],
    },
    CapabilityEntry {
        id: "health-doctor",
        title: "Health and dependency status",
        status: CapabilityStatus::Stable,
        since: "Phase 11",
        claim: Some("C-023"),
        evidence: &["docs/phase11_doctor_v1.json"],
        command: "machine doctor",
        surfaces: &["cli"],
    },
    CapabilityEntry {
        id: "upgrade-recovery",
        title: "Backup-first upgrade and recovery",
        status: CapabilityStatus::Stable,
        since: "Phase 11",
        claim: Some("C-023"),
        evidence: &["docs/phase11_doctor_v1.json"],
        command: "machine upgrade / machine recover",
        surfaces: &["cli"],
    },
    CapabilityEntry {
        id: "release-notes",
        title: "Release notes tied to evaluation results",
        status: CapabilityStatus::Stable,
        since: "Phase 11",
        claim: Some("C-023"),
        evidence: &["docs/phase11_release_notes_v1.md"],
        command: "machine release notes",
        surfaces: &["cli"],
    },
    CapabilityEntry {
        id: "gpu-projection",
        title: "Optional CUDA projection backend",
        status: CapabilityStatus::Deferred,
        since: "Phase 11",
        claim: None,
        evidence: &["docs/DEVELOPMENT.md"],
        command: "cargo build --features cuda",
        surfaces: &["build"],
    },
];

/// Look up one capability by id.
pub fn capability(id: &str) -> Option<&'static CapabilityEntry> {
    CAPABILITIES.iter().find(|entry| entry.id == id)
}

/// Count capabilities by status label.
pub fn capability_status_counts() -> BTreeMap<String, usize> {
    let mut counts = BTreeMap::new();
    for entry in CAPABILITIES {
        *counts.entry(entry.status.label().to_string()).or_insert(0) += 1;
    }
    counts
}

/// Render the inventory as pretty JSON.
pub fn inventory_json() -> Result<String, String> {
    #[derive(serde::Serialize)]
    struct InventoryDoc<'a> {
        schema: &'a str,
        release: &'a str,
        capabilities: &'a [CapabilityEntry],
    }
    serde_json::to_string_pretty(&InventoryDoc {
        schema: INVENTORY_SCHEMA,
        release: RELEASE_TAG,
        capabilities: CAPABILITIES,
    })
    .map_err(|error| format!("serialize inventory: {error}"))
}

/// Render the inventory as Markdown, grouped in declaration order.
pub fn inventory_markdown() -> String {
    let mut markdown = String::new();
    markdown.push_str(&format!(
        "# Capability inventory ({})\n\n> {INVENTORY_SCHEMA}, release {RELEASE_TAG}\n\n",
        RELEASE_TAG
    ));
    markdown.push_str("| id | capability | status | since | claim | evidence | command |\n");
    markdown.push_str("|---|---|---|---|---|---|---|\n");
    for entry in CAPABILITIES {
        let claim = entry.claim.unwrap_or("—");
        let evidence = if entry.evidence.is_empty() {
            "—".to_string()
        } else {
            entry
                .evidence
                .iter()
                .map(|path| format!("`{path}`"))
                .collect::<Vec<_>>()
                .join("<br>")
        };
        markdown.push_str(&format!(
            "| `{}` | {} | {} | {} | {} | {} | `{}` |\n",
            entry.id,
            entry.title,
            entry.status.label(),
            entry.since,
            claim,
            evidence,
            entry.command
        ));
    }
    markdown
}

// ─── Profiles ───────────────────────────────────────────────────────────────

/// Supported operating profiles.  A profile is a documented configuration,
/// not a separate build: `doctor` reports whether the running binary actually
/// matches the profile it was asked to run.
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Profile {
    /// Core runtime only; no optional worker.
    Minimal,
    /// Core runtime plus the web interface.  The default.
    Standard,
    /// Standard plus the optionally built CUDA projection backend.
    Gpu,
    /// Standard plus the semantic worker (still shadow-only).
    SemanticWorker,
}

impl Profile {
    pub fn all() -> [Profile; 4] {
        [
            Profile::Minimal,
            Profile::Standard,
            Profile::Gpu,
            Profile::SemanticWorker,
        ]
    }

    pub fn parse(value: &str) -> Option<Profile> {
        match value.trim().to_ascii_lowercase().as_str() {
            "minimal" => Some(Profile::Minimal),
            "standard" => Some(Profile::Standard),
            "gpu" | "cuda" => Some(Profile::Gpu),
            "semantic" | "semantic-worker" | "semantic_worker" => Some(Profile::SemanticWorker),
            _ => None,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Profile::Minimal => "minimal",
            Profile::Standard => "standard",
            Profile::Gpu => "gpu",
            Profile::SemanticWorker => "semantic_worker",
        }
    }

    pub fn description(self) -> &'static str {
        match self {
            Profile::Minimal => "core conversational runtime, no optional worker",
            Profile::Standard => "core runtime plus the local web interface (default)",
            Profile::Gpu => "standard plus the optionally built CUDA projection backend",
            Profile::SemanticWorker => "standard plus the semantic worker (shadow only)",
        }
    }

    pub fn implies_semantic_worker(self) -> bool {
        matches!(self, Profile::SemanticWorker)
    }

    pub fn requires_cuda(self) -> bool {
        matches!(self, Profile::Gpu)
    }
}

// ─── Configuration ──────────────────────────────────────────────────────────

/// Effective operator configuration.  Built from defaults, environment
/// variables (`MACHINE_*`), and command-line overrides.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OperatorConfig {
    pub profile: Profile,
    pub data_dir: String,
    pub db_path: String,
    pub qa_path: String,
    pub store_path: String,
    pub backup_dir: String,
    pub host: String,
    pub port: u16,
    pub allow_remote: bool,
    pub token: Option<String>,
    pub semantic_worker: bool,
    pub memory_budget_bytes: u64,
}

impl Default for OperatorConfig {
    fn default() -> Self {
        OperatorConfig {
            profile: Profile::Standard,
            data_dir: "data/conversation".to_string(),
            db_path: "data/conversation/machine.db".to_string(),
            qa_path: "data/qa_memory.json".to_string(),
            store_path: "data/conversation/sessions.json".to_string(),
            backup_dir: "data/conversation/backups".to_string(),
            host: "127.0.0.1".to_string(),
            port: 8787,
            allow_remote: false,
            token: None,
            semantic_worker: false,
            memory_budget_bytes: 2 * 1024 * 1024 * 1024,
        }
    }
}

fn env_string(key: &str) -> Option<String> {
    std::env::var(key).ok().filter(|value| !value.is_empty())
}

fn env_bool(key: &str) -> Option<bool> {
    env_string(key).and_then(|value| match value.to_ascii_lowercase().as_str() {
        "1" | "true" | "yes" | "on" => Some(true),
        "0" | "false" | "no" | "off" => Some(false),
        _ => None,
    })
}

impl OperatorConfig {
    /// Build the configuration from `MACHINE_*` environment variables, falling
    /// back to defaults.  Command-line flags are applied by the binary on top.
    pub fn from_env() -> OperatorConfig {
        let mut config = OperatorConfig::default();
        if let Some(profile) = env_string("MACHINE_PROFILE").as_deref().and_then(Profile::parse) {
            config.profile = profile;
        }
        if let Some(value) = env_string("MACHINE_DATA_DIR") {
            config.data_dir = value;
        }
        if let Some(value) = env_string("MACHINE_DB") {
            config.db_path = value;
        }
        if let Some(value) = env_string("MACHINE_QA") {
            config.qa_path = value;
        }
        if let Some(value) = env_string("MACHINE_STORE") {
            config.store_path = value;
        }
        if let Some(value) = env_string("MACHINE_BACKUP_DIR") {
            config.backup_dir = value;
        }
        if let Some(value) = env_string("MACHINE_HOST") {
            config.host = value;
        }
        if let Some(value) = env_string("MACHINE_PORT").and_then(|value| value.parse().ok()) {
            config.port = value;
        }
        if let Some(value) = env_bool("MACHINE_ALLOW_REMOTE") {
            config.allow_remote = value;
        }
        if let Some(value) = env_string("MACHINE_CHAT_TOKEN") {
            config.token = Some(value);
        }
        if let Some(value) = env_bool("MACHINE_SEMANTIC_WORKER") {
            config.semantic_worker = value;
        }
        if let Some(value) =
            env_string("MACHINE_MEMORY_BUDGET_BYTES").and_then(|value| value.parse().ok())
        {
            config.memory_budget_bytes = value;
        }
        config.semantic_worker |= config.profile.implies_semantic_worker();
        config
    }

    /// Redacted, human-readable effective configuration for display.
    pub fn display(&self) -> String {
        let token = match &self.token {
            Some(_) => "<set>",
            None => "<unset>",
        };
        let mut lines = vec![
            format!("profile:            {} ({})", self.profile.label(), self.profile.description()),
            format!("data dir:           {}", self.data_dir),
            format!("database:           {}", self.db_path),
            format!("legacy QA import:   {}", self.qa_path),
            format!("legacy sessions:    {}", self.store_path),
            format!("backup dir:         {}", self.backup_dir),
            format!("bind:               {}:{}", self.host, self.port),
            format!("allow remote:       {}", self.allow_remote),
            format!("bearer token:       {token}"),
            format!("semantic worker:    {}", self.semantic_worker),
            format!("memory budget:      {} bytes", self.memory_budget_bytes),
        ];
        lines.push(format!("startup command:    {}", startup_command()));
        lines.join("\n")
    }
}

/// Issue severity for configuration validation.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Severity {
    Info,
    Warning,
    Error,
}

impl Severity {
    pub fn label(self) -> &'static str {
        match self {
            Severity::Info => "info",
            Severity::Warning => "warning",
            Severity::Error => "error",
        }
    }
}

/// One configuration problem with a concrete remedy.
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize)]
pub struct ConfigIssue {
    pub severity: Severity,
    pub field: &'static str,
    pub message: String,
    pub remedy: String,
}

impl ConfigIssue {
    fn error(field: &'static str, message: impl Into<String>, remedy: impl Into<String>) -> Self {
        ConfigIssue {
            severity: Severity::Error,
            field,
            message: message.into(),
            remedy: remedy.into(),
        }
    }

    fn warning(field: &'static str, message: impl Into<String>, remedy: impl Into<String>) -> Self {
        ConfigIssue {
            severity: Severity::Warning,
            field,
            message: message.into(),
            remedy: remedy.into(),
        }
    }

    fn info(field: &'static str, message: impl Into<String>, remedy: impl Into<String>) -> Self {
        ConfigIssue {
            severity: Severity::Info,
            field,
            message: message.into(),
            remedy: remedy.into(),
        }
    }

    pub fn render(&self) -> String {
        format!(
            "[{}] {}: {} ({})",
            self.severity.label(),
            self.field,
            self.message,
            self.remedy
        )
    }
}

/// True for the loopback hosts the server accepts without a token.
pub fn is_loopback(host: &str) -> bool {
    matches!(host, "127.0.0.1" | "::1" | "localhost")
}

impl OperatorConfig {
    /// Validate the configuration without touching the filesystem.  Returns an
    /// empty vector when the configuration is safe to start.
    pub fn validate(&self) -> Vec<ConfigIssue> {
        let mut issues = Vec::new();

        if self.port == 0 {
            issues.push(ConfigIssue::error(
                "port",
                "port 0 does not select a usable listening port",
                "set --port or MACHINE_PORT to a value between 1 and 65535",
            ));
        }
        if self.data_dir.trim().is_empty() {
            issues.push(ConfigIssue::error(
                "data_dir",
                "data directory is empty",
                "set --data-dir or MACHINE_DATA_DIR",
            ));
        }
        if self.db_path.trim().is_empty() {
            issues.push(ConfigIssue::error(
                "db_path",
                "database path is empty",
                "set --db or MACHINE_DB",
            ));
        }

        if !is_loopback(&self.host) {
            if !self.allow_remote {
                issues.push(ConfigIssue::error(
                    "host",
                    format!("non-loopback host '{}' is refused without opt-in", self.host),
                    "add --allow-remote (and a token) or bind 127.0.0.1",
                ));
            }
            if self.token.is_none() {
                issues.push(ConfigIssue::error(
                    "token",
                    "a non-loopback bind requires a bearer token",
                    "set --token <value> or MACHINE_CHAT_TOKEN",
                ));
            }
        } else if self.allow_remote {
            issues.push(ConfigIssue::info(
                "allow_remote",
                "--allow-remote has no effect on a loopback bind",
                "remove --allow-remote or bind a non-loopback host",
            ));
        }

        if self.profile.requires_cuda() && projection_path() != ProjectionPath::CudaProjection {
            issues.push(ConfigIssue::warning(
                "profile",
                "the gpu profile was selected but this binary has no CUDA backend",
                "rebuild with `cargo build --release --features cuda`, or select the standard profile",
            ));
        }
        if self.profile.implies_semantic_worker() && !self.semantic_worker {
            issues.push(ConfigIssue::warning(
                "semantic_worker",
                "the semantic_worker profile implies the worker but it is disabled",
                "unset MACHINE_SEMANTIC_WORKER=false or select a different profile",
            ));
        }
        if !self.profile.implies_semantic_worker() && self.semantic_worker {
            issues.push(ConfigIssue::info(
                "semantic_worker",
                "the semantic worker is enabled outside the semantic_worker profile",
                "select the semantic_worker profile to make this explicit",
            ));
        }

        if self.memory_budget_bytes < 16 * 1024 * 1024 {
            issues.push(ConfigIssue::warning(
                "memory_budget_bytes",
                format!("memory budget {} is very small", self.memory_budget_bytes),
                "raise MACHINE_MEMORY_BUDGET_BYTES (default 2 GiB)",
            ));
        }

        let db_under_data = Path::new(&self.db_path).starts_with(Path::new(&self.data_dir));
        if !self.data_dir.trim().is_empty()
            && !self.db_path.trim().is_empty()
            && !db_under_data
        {
            issues.push(ConfigIssue::info(
                "db_path",
                "the database is outside the data directory",
                "keep the database under the data directory so one backup covers all state",
            ));
        }

        issues
    }

    /// True when no validation issue is an error.
    pub fn is_ok(&self) -> bool {
        !self
            .validate()
            .iter()
            .any(|issue| issue.severity == Severity::Error)
    }

    /// The `ChatServerConfig` this operator configuration describes.
    pub fn chat_server_config(&self) -> crate::chat_server::ChatServerConfig {
        crate::chat_server::ChatServerConfig {
            db_path: Some(self.db_path.clone()),
            qa_path: Some(self.qa_path.clone()),
            store_path: Some(self.store_path.clone()),
            auth_token: self.token.clone(),
        }
    }
}

// ─── Database inspection ────────────────────────────────────────────────────

/// Read-only view of the database's durability state.
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize)]
pub struct DatabaseState {
    pub exists: bool,
    pub schema_version: Option<u32>,
    pub integrity_ok: bool,
    pub detail: String,
}

impl DatabaseState {
    /// A state for a database that does not exist yet.
    pub fn absent() -> Self {
        DatabaseState {
            exists: false,
            schema_version: None,
            integrity_ok: true,
            detail: "database not created yet".to_string(),
        }
    }

    /// Whether this build can open the database without misreading it.
    pub fn compatible(&self) -> bool {
        match self.schema_version {
            Some(version) => version <= SCHEMA_VERSION && self.integrity_ok,
            None => self.integrity_ok,
        }
    }
}

/// Inspect a database without creating or migrating it.  Opens read-only, runs
/// `PRAGMA integrity_check`, and reads `PRAGMA user_version`.
pub fn read_database_state(path: impl AsRef<Path>) -> Result<DatabaseState, String> {
    let path = path.as_ref();
    if !path.exists() {
        return Ok(DatabaseState::absent());
    }
    let connection = rusqlite::Connection::open_with_flags(
        path,
        rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY,
    )
    .map_err(|error| format!("open {} read-only: {error}", path.display()))?;
    let integrity: String = connection
        .query_row("PRAGMA integrity_check", [], |row| row.get(0))
        .map_err(|error| format!("integrity_check {}: {error}", path.display()))?;
    let schema_version: i64 = connection
        .query_row("PRAGMA user_version", [], |row| row.get(0))
        .map_err(|error| format!("read user_version {}: {error}", path.display()))?;
    let integrity_ok = integrity.eq_ignore_ascii_case("ok");
    let detail = if integrity_ok {
        format!("integrity ok, schema {schema_version}")
    } else {
        format!("integrity check failed: {integrity}")
    };
    Ok(DatabaseState {
        exists: true,
        schema_version: Some(schema_version.max(0) as u32),
        integrity_ok,
        detail,
    })
}

// ─── Doctor (health and dependency status) ──────────────────────────────────

/// Status of one dependency or health check.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CheckStatus {
    Ok,
    Warn,
    Fail,
}

impl CheckStatus {
    pub fn label(self) -> &'static str {
        match self {
            CheckStatus::Ok => "ok",
            CheckStatus::Warn => "warn",
            CheckStatus::Fail => "fail",
        }
    }
}

/// One doctor check.
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize)]
pub struct DependencyCheck {
    pub name: &'static str,
    pub status: CheckStatus,
    pub detail: String,
    pub remedy: Option<String>,
}

impl DependencyCheck {
    fn new(
        name: &'static str,
        status: CheckStatus,
        detail: impl Into<String>,
        remedy: Option<&str>,
    ) -> Self {
        DependencyCheck {
            name,
            status,
            detail: detail.into(),
            remedy: remedy.map(str::to_string),
        }
    }
}

/// Health and dependency report.
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize)]
pub struct DoctorReport {
    pub schema: &'static str,
    pub release: &'static str,
    pub profile: Profile,
    pub checks: Vec<DependencyCheck>,
}

impl DoctorReport {
    /// The most severe status across all checks.
    pub fn worst(&self) -> CheckStatus {
        self.checks
            .iter()
            .map(|check| check.status)
            .max()
            .unwrap_or(CheckStatus::Ok)
    }

    /// Whether the report has no failing check.
    pub fn healthy(&self) -> bool {
        self.worst() < CheckStatus::Fail
    }

    pub fn to_json(&self) -> Result<String, String> {
        serde_json::to_string_pretty(self).map_err(|error| format!("serialize doctor: {error}"))
    }

    pub fn render(&self) -> String {
        let mut out = format!(
            "The Machine doctor ({}, profile {})\n",
            self.release,
            self.profile.label()
        );
        for check in &self.checks {
            out.push_str(&format!(
                "  [{}] {}: {}\n",
                check.status.label(),
                check.name,
                check.detail
            ));
            if let Some(remedy) = &check.remedy {
                out.push_str(&format!("        -> {remedy}\n"));
            }
        }
        out.push_str(&format!("worst status: {}\n", self.worst().label()));
        out
    }
}

fn check_data_dir_writable(dir: &str) -> DependencyCheck {
    if dir.trim().is_empty() {
        return DependencyCheck::new(
            "data_dir",
            CheckStatus::Fail,
            "no data directory configured",
            Some("set --data-dir or MACHINE_DATA_DIR"),
        );
    }
    let path = Path::new(dir);
    if let Err(error) = std::fs::create_dir_all(path) {
        return DependencyCheck::new(
            "data_dir",
            CheckStatus::Fail,
            format!("cannot create {dir}: {error}"),
            Some("choose a writable data directory"),
        );
    }
    let probe = path.join(".machine-write-probe");
    match std::fs::write(&probe, b"probe") {
        Ok(()) => {
            let _ = std::fs::remove_file(&probe);
            DependencyCheck::new("data_dir", CheckStatus::Ok, format!("{dir} is writable"), None)
        }
        Err(error) => DependencyCheck::new(
            "data_dir",
            CheckStatus::Fail,
            format!("{dir} is not writable: {error}"),
            Some("choose a writable data directory"),
        ),
    }
}

fn check_port(host: &str, port: u16) -> DependencyCheck {
    match std::net::TcpListener::bind((host, port)) {
        Ok(listener) => {
            drop(listener);
            DependencyCheck::new(
                "port",
                CheckStatus::Ok,
                format!("{host}:{port} is available to bind"),
                None,
            )
        }
        Err(error) => DependencyCheck::new(
            "port",
            CheckStatus::Warn,
            format!("{host}:{port} could not be bound: {error}"),
            Some("another instance may be running; stop it or choose another --port"),
        ),
    }
}

fn check_database(db_path: &str) -> DependencyCheck {
    match read_database_state(db_path) {
        Ok(state) if !state.exists => DependencyCheck::new(
            "database",
            CheckStatus::Ok,
            format!("{db_path} not found; it will be created with schema {SCHEMA_VERSION}"),
            None,
        ),
        Ok(state) => {
            let version = state.schema_version.unwrap_or(0);
            if !state.integrity_ok {
                DependencyCheck::new(
                    "database",
                    CheckStatus::Fail,
                    format!("{db_path}: {}", state.detail),
                    Some("run `machine recover` to restore the latest backup"),
                )
            } else if version > SCHEMA_VERSION {
                DependencyCheck::new(
                    "database",
                    CheckStatus::Fail,
                    format!(
                        "{db_path} has schema {version}, newer than this build ({SCHEMA_VERSION})"
                    ),
                    Some("use a newer build, or `machine recover` a compatible backup"),
                )
            } else if version < SCHEMA_VERSION {
                DependencyCheck::new(
                    "database",
                    CheckStatus::Warn,
                    format!("{db_path} has schema {version}; it will migrate to {SCHEMA_VERSION}"),
                    Some("run `machine upgrade` to migrate with a backup taken first"),
                )
            } else {
                DependencyCheck::new(
                    "database",
                    CheckStatus::Ok,
                    format!("{db_path}: {}", state.detail),
                    None,
                )
            }
        }
        Err(error) => DependencyCheck::new(
            "database",
            CheckStatus::Fail,
            format!("{db_path}: {error}"),
            Some("check the path and file permissions"),
        ),
    }
}

/// Run every health and dependency check against a configuration.
pub fn doctor(config: &OperatorConfig) -> DoctorReport {
    let mut checks = Vec::new();

    checks.push(DependencyCheck::new(
        "release",
        CheckStatus::Ok,
        format!(
            "{RELEASE_TAG} (crate {}, schema {SCHEMA_VERSION}, {OPERATOR_SCHEMA})",
            env!("CARGO_PKG_VERSION")
        ),
        None,
    ));
    checks.push(DependencyCheck::new(
        "profile",
        CheckStatus::Ok,
        format!("{} — {}", config.profile.label(), config.profile.description()),
        None,
    ));

    let projection = projection_path();
    checks.push(DependencyCheck::new(
        "projection",
        CheckStatus::Ok,
        format!("active path: {}", projection.label()),
        None,
    ));

    checks.push(DependencyCheck::new(
        "sqlite",
        CheckStatus::Ok,
        "bundled SQLite via rusqlite (no system library required)",
        None,
    ));
    checks.push(DependencyCheck::new(
        "pdf-backend",
        CheckStatus::Ok,
        "pdf-extract compiled in (text extraction; scanned/visual deferred)",
        None,
    ));

    checks.push(check_data_dir_writable(&config.data_dir));
    checks.push(check_database(&config.db_path));

    let legacy = [config.qa_path.as_str(), config.store_path.as_str()]
        .iter()
        .filter(|path| Path::new(path).exists())
        .count();
    checks.push(DependencyCheck::new(
        "legacy-import",
        CheckStatus::Ok,
        if legacy == 0 {
            "no legacy JSON snapshots present".to_string()
        } else {
            format!("{legacy} legacy snapshot(s) available for one-time import")
        },
        None,
    ));

    if config.profile.requires_cuda() && projection != ProjectionPath::CudaProjection {
        checks.push(DependencyCheck::new(
            "cuda",
            CheckStatus::Warn,
            "gpu profile selected but this binary has no CUDA backend",
            Some("rebuild with `cargo build --release --features cuda`"),
        ));
    }

    if config.semantic_worker {
        checks.push(DependencyCheck::new(
            "semantic-worker",
            CheckStatus::Ok,
            "enabled (shadow only; never authorizes an answer)",
            None,
        ));
    }

    checks.push(check_port(&config.host, config.port));

    for issue in config.validate() {
        checks.push(DependencyCheck::new(
            "config",
            match issue.severity {
                Severity::Error => CheckStatus::Fail,
                Severity::Warning => CheckStatus::Warn,
                Severity::Info => CheckStatus::Ok,
            },
            format!("{}: {}", issue.field, issue.message),
            Some(issue.remedy.as_str()),
        ));
    }

    DoctorReport {
        schema: DOCTOR_SCHEMA,
        release: RELEASE_TAG,
        profile: config.profile,
        checks,
    }
}

// ─── Release notes tied to evaluation results ───────────────────────────────

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

/// The committed `docs/` directory, for regenerating release artifacts.
pub fn repo_docs_dir() -> PathBuf {
    repo_root().join("docs")
}

fn read_report(relative: &str) -> Option<serde_json::Value> {
    let path = repo_root().join(relative);
    let text = std::fs::read_to_string(path).ok()?;
    serde_json::from_str(&text).ok()
}

fn number(value: &serde_json::Value, key: &str) -> Option<f64> {
    value.get(key).and_then(|item| item.as_f64())
}

fn integer(value: &serde_json::Value, key: &str) -> Option<i64> {
    value.get(key).and_then(|item| item.as_i64())
}

fn boolean(value: &serde_json::Value, key: &str) -> Option<bool> {
    value.get(key).and_then(|item| item.as_bool())
}

/// One evaluation artifact referenced by the release notes.
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize)]
pub struct EvaluationSummary {
    pub name: String,
    pub artifact: String,
    pub present: bool,
    pub headline: String,
    pub command: &'static str,
}

impl EvaluationSummary {
    fn missing(name: &str, artifact: &str, command: &'static str) -> Self {
        EvaluationSummary {
            name: name.to_string(),
            artifact: artifact.to_string(),
            present: false,
            headline: format!("artifact not found; regenerate with `{command}`"),
            command,
        }
    }
}

/// Summarize the committed evaluation artifacts, extracting measured numbers
/// so the release notes reflect what was actually verified.
pub fn evaluation_summaries() -> Vec<EvaluationSummary> {
    let mut summaries = Vec::new();

    let reliability = "docs/phase7_reliability_v1.report.json";
    match read_report(reliability) {
        Some(value) => {
            let enforced = value
                .get("guarantee_counts")
                .and_then(|counts| integer(counts, "enforced"))
                .unwrap_or(0);
            let checked = value
                .get("guarantee_counts")
                .and_then(|counts| integer(counts, "checked"))
                .unwrap_or(0);
            let within = boolean(&value, "memory_within_budget").unwrap_or(false);
            let path = value
                .get("projection_path")
                .and_then(|item| item.as_str())
                .unwrap_or("unknown");
            let inserts = integer(&value, "sustained_inserts").unwrap_or(0);
            summaries.push(EvaluationSummary {
                name: "Reliability contracts".to_string(),
                artifact: reliability.to_string(),
                present: true,
                headline: format!(
                    "{inserts} sustained observations, memory within budget: {within}; \
                     {enforced} guarantees enforced, {checked} checked; projection {path}"
                ),
                command: "cargo run --bin reliability_report",
            });
        }
        None => summaries.push(EvaluationSummary::missing(
            "Reliability contracts",
            reliability,
            "cargo run --bin reliability_report",
        )),
    }

    let conversation = "docs/phase9_conversation_eval_v1.report.json";
    match read_report(conversation) {
        Some(value) => {
            let regression = value.get("regression");
            let turns = regression.and_then(|item| integer(item, "turns")).unwrap_or(0);
            let answered = regression
                .and_then(|item| integer(item, "answered"))
                .unwrap_or(0);
            let coverage = regression
                .and_then(|item| number(item, "coverage_rate"))
                .unwrap_or(0.0);
            let holdout = value.get("holdout");
            let holdout_coverage = holdout
                .and_then(|item| number(item, "coverage_rate"))
                .unwrap_or(0.0);
            let ablations = value
                .get("ablations")
                .and_then(|item| item.as_array())
                .map(Vec::len)
                .unwrap_or(0);
            let drift = value
                .get("regression_drift")
                .and_then(|item| item.as_array())
                .map(Vec::len)
                .unwrap_or(0);
            summaries.push(EvaluationSummary {
                name: "Conversation evaluation".to_string(),
                artifact: conversation.to_string(),
                present: true,
                headline: format!(
                    "regression {answered}/{turns} answered (coverage {coverage:.3}), \
                     holdout coverage {holdout_coverage:.3}, drift {drift}, {ablations} ablations"
                ),
                command: "cargo run --bin machine_eval",
            });
        }
        None => summaries.push(EvaluationSummary::missing(
            "Conversation evaluation",
            conversation,
            "cargo run --bin machine_eval",
        )),
    }

    let fidelity = "docs/semantic_fidelity_eval_v1.report.json";
    match read_report(fidelity) {
        Some(value) => {
            let records = integer(&value, "records").unwrap_or(0);
            let correct = integer(&value, "interpretations_correct").unwrap_or(0);
            let silent = integer(&value, "silent_wrong_answers").unwrap_or(0);
            let limit = integer(&value, "wrong_answer_limit").unwrap_or(0);
            let authorized = integer(&value, "downstream_authorizations").unwrap_or(0);
            summaries.push(EvaluationSummary {
                name: "Semantic fidelity".to_string(),
                artifact: fidelity.to_string(),
                present: true,
                headline: format!(
                    "{correct}/{records} faithful, silent wrong answers {silent} \
                     (limit {limit}), downstream authorizations {authorized}"
                ),
                command: "cargo run --bin semantic_fidelity_eval",
            });
        }
        None => summaries.push(EvaluationSummary::missing(
            "Semantic fidelity",
            fidelity,
            "cargo run --bin semantic_fidelity_eval",
        )),
    }

    let autonomy = "docs/phase10_autonomy_v1.report.json";
    match read_report(autonomy) {
        Some(value) => {
            let scenarios = integer(&value, "scenarios").unwrap_or(0);
            let completed = integer(&value, "completed").unwrap_or(0);
            let refused = integer(&value, "refused").unwrap_or(0);
            let contracts = boolean(&value, "all_contracts_satisfied").unwrap_or(false);
            summaries.push(EvaluationSummary {
                name: "Controlled autonomy".to_string(),
                artifact: autonomy.to_string(),
                present: true,
                headline: format!(
                    "{scenarios} scenarios, {completed} completed, {refused} controlled refusal, \
                     contracts satisfied: {contracts}"
                ),
                command: "cargo run --bin machine_eval task",
            });
        }
        None => summaries.push(EvaluationSummary::missing(
            "Controlled autonomy",
            autonomy,
            "cargo run --bin machine_eval task",
        )),
    }

    summaries
}

/// Release notes, generated from the capability inventory and the committed
/// evaluation artifacts.
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize)]
pub struct ReleaseNotes {
    pub schema: &'static str,
    pub release: &'static str,
    pub date: &'static str,
    pub crate_version: &'static str,
    pub schema_version: u32,
    pub projection_path: &'static str,
    pub highlights: Vec<&'static str>,
    pub capabilities: BTreeMap<String, usize>,
    pub evaluations: Vec<EvaluationSummary>,
    pub upgrade_notes: Vec<&'static str>,
}

/// Build the release notes for this build.
pub fn release_notes() -> ReleaseNotes {
    ReleaseNotes {
        schema: RELEASE_NOTES_SCHEMA,
        release: RELEASE_TAG,
        date: RELEASE_DATE,
        crate_version: env!("CARGO_PKG_VERSION"),
        schema_version: SCHEMA_VERSION,
        projection_path: projection_path().label(),
        highlights: vec![
            "One documented startup command (`machine start`) replaces research-specific commands.",
            "Configuration is validated with a remedy for every problem.",
            "`machine doctor` reports health and dependency status, including schema compatibility.",
            "Backup, restore, upgrade, and recover are first-class and safe to run unattended.",
            "The capability inventory is versioned and tied to the claim ledger and evaluation artifacts.",
        ],
        capabilities: capability_status_counts(),
        evaluations: evaluation_summaries(),
        upgrade_notes: vec![
            "Back up before upgrading; `machine upgrade` takes a timestamped backup automatically.",
            "A database newer than the build is refused, never misread.",
            "If an upgrade fails, `machine recover` restores the most recent backup and verifies it.",
        ],
    }
}

/// Render release notes as Markdown.
pub fn release_notes_markdown() -> String {
    let notes = release_notes();
    let mut out = String::new();
    out.push_str(&format!(
        "# The Machine {release} — release notes\n\n",
        release = notes.release
    ));
    out.push_str(&format!(
        "> {schema} · crate {crate_version} · persistence schema {schema_version} · projection {projection_path}\n",
        schema = notes.schema,
        crate_version = notes.crate_version,
        schema_version = notes.schema_version,
        projection_path = notes.projection_path,
    ));
    out.push_str(&format!("\nPackaged {date}.\n\n", date = notes.date));
    out.push_str(&format!(
        "Start with `{command}`. The full operator reference is `docs/DEVELOPMENT.md`.\n\n",
        command = startup_command()
    ));

    out.push_str("## Highlights\n\n");
    for highlight in &notes.highlights {
        out.push_str(&format!("* {highlight}\n"));
    }

    out.push_str("\n## Capabilities\n\n");
    for (status, count) in &notes.capabilities {
        out.push_str(&format!("* {status}: {count}\n"));
    }
    out.push_str("\nSee `docs/phase11_capability_inventory_v1.json` for the versioned inventory.\n");

    out.push_str("\n## Evaluation results\n\n");
    out.push_str("| area | result | artifact |\n|---|---|---|\n");
    for evaluation in &notes.evaluations {
        out.push_str(&format!(
            "| {} | {} | `{}` |\n",
            evaluation.name, evaluation.headline, evaluation.artifact
        ));
    }
    out.push_str("\nRegenerate each artifact with the command named in the phase document (`docs/DEVELOPMENT.md`).\n");

    out.push_str("\n## Upgrading\n\n");
    for note in &notes.upgrade_notes {
        out.push_str(&format!("* {note}\n"));
    }

    out
}

/// Write the inventory, release notes, and doctor report to `docs_dir`.
pub struct ReleaseArtifacts {
    pub inventory: PathBuf,
    pub release_notes: PathBuf,
    pub doctor: PathBuf,
}

pub fn write_release_artifacts(
    config: &OperatorConfig,
    docs_dir: impl AsRef<Path>,
) -> Result<ReleaseArtifacts, String> {
    let docs_dir = docs_dir.as_ref();
    std::fs::create_dir_all(docs_dir)
        .map_err(|error| format!("create {}: {error}", docs_dir.display()))?;

    let inventory = docs_dir.join("phase11_capability_inventory_v1.json");
    std::fs::write(&inventory, inventory_json()?)
        .map_err(|error| format!("write {}: {error}", inventory.display()))?;

    let release_path = docs_dir.join("phase11_release_notes_v1.md");
    std::fs::write(&release_path, release_notes_markdown())
        .map_err(|error| format!("write {}: {error}", release_path.display()))?;

    let doctor_path = docs_dir.join("phase11_doctor_v1.json");
    std::fs::write(&doctor_path, doctor(config).to_json()?)
        .map_err(|error| format!("write {}: {error}", doctor_path.display()))?;

    Ok(ReleaseArtifacts {
        inventory,
        release_notes: release_path,
        doctor: doctor_path,
    })
}

// ─── Backup, restore, upgrade, recover ──────────────────────────────────────

fn ensure_parent(path: &Path) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        if !parent.as_os_str().is_empty() {
            std::fs::create_dir_all(parent)
                .map_err(|error| format!("create {}: {error}", parent.display()))?;
        }
    }
    Ok(())
}

/// Remove zero-length `-wal` / `-shm` siblings left beside a database file by
/// the SQLite backup API.  A non-empty write-ahead log is left untouched.
fn tidy_sqlite_siblings(path: &Path) {
    for suffix in ["-wal", "-shm"] {
        let sibling = PathBuf::from(format!("{}{suffix}", path.display()));
        if let Ok(metadata) = std::fs::metadata(&sibling) {
            if metadata.len() == 0 {
                let _ = std::fs::remove_file(&sibling);
            }
        }
    }
}

/// Copy a database to `target` using SQLite's online backup API, without
/// opening or migrating the source.  Safe to run while the source is live.
pub fn backup_database(db_path: impl AsRef<Path>, target: impl AsRef<Path>) -> Result<(), String> {
    let db_path = db_path.as_ref();
    let target = target.as_ref();
    if !db_path.exists() {
        return Err(format!("database {} does not exist", db_path.display()));
    }
    ensure_parent(target)?;
    let source = rusqlite::Connection::open(db_path)
        .map_err(|error| format!("open source {}: {error}", db_path.display()))?;
    let mut destination = rusqlite::Connection::open(target)
        .map_err(|error| format!("open target {}: {error}", target.display()))?;
    {
        let backup = rusqlite::backup::Backup::new(&source, &mut destination)
            .map_err(|error| format!("begin backup: {error}"))?;
        backup
            .run_to_completion(256, Duration::from_millis(1), None)
            .map_err(|error| format!("run backup: {error}"))?;
    }
    // A cold backup is a plain file: switch it off WAL so it leaves no
    // write-ahead/shm siblings beside it.
    let _: Result<String, _> =
        destination.query_row("PRAGMA journal_mode=DELETE", [], |row| row.get(0));
    drop(source);
    drop(destination);
    let state = read_database_state(target)?;
    if !state.integrity_ok {
        return Err(format!("backup {} failed its integrity check", target.display()));
    }
    tidy_sqlite_siblings(target);
    Ok(())
}

/// Replace `db_path` with the contents of `backup`, without migrating the
/// destination.  Used by restore and recovery.
pub fn restore_database(backup: impl AsRef<Path>, db_path: impl AsRef<Path>) -> Result<(), String> {
    let backup = backup.as_ref();
    let db_path = db_path.as_ref();
    if !backup.exists() {
        return Err(format!("backup {} does not exist", backup.display()));
    }
    let state = read_database_state(backup)?;
    if !state.integrity_ok {
        return Err(format!("backup {} is not intact", backup.display()));
    }
    if let Some(version) = state.schema_version {
        if version > SCHEMA_VERSION {
            return Err(format!(
                "backup schema {version} is newer than this build ({SCHEMA_VERSION})"
            ));
        }
    }
    ensure_parent(db_path)?;
    let source = rusqlite::Connection::open_with_flags(
        backup,
        rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY,
    )
    .map_err(|error| format!("open backup {}: {error}", backup.display()))?;
    let mut destination = rusqlite::Connection::open(db_path)
        .map_err(|error| format!("open destination {}: {error}", db_path.display()))?;
    {
        let restore = rusqlite::backup::Backup::new(&source, &mut destination)
            .map_err(|error| format!("begin restore: {error}"))?;
        restore
            .run_to_completion(256, Duration::from_millis(1), None)
            .map_err(|error| format!("run restore: {error}"))?;
    }
    drop(source);
    drop(destination);
    let restored = read_database_state(db_path)?;
    if !restored.integrity_ok {
        return Err(format!(
            "restored database {} failed its integrity check",
            db_path.display()
        ));
    }
    Ok(())
}

/// A pre-flight plan for an upgrade.
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize)]
pub struct UpgradePlan {
    pub db_path: String,
    pub from_schema: Option<u32>,
    pub to_schema: u32,
    pub compatible: bool,
    pub backup_dir: String,
    pub steps: Vec<String>,
}

impl UpgradePlan {
    pub fn render(&self) -> String {
        let mut out = format!(
            "Upgrade plan for {}\n  schema: {:?} -> {}\n  compatible: {}\n",
            self.db_path, self.from_schema, self.to_schema, self.compatible
        );
        for (index, step) in self.steps.iter().enumerate() {
            out.push_str(&format!("  {}. {step}\n", index + 1));
        }
        out
    }
}

/// Build an upgrade plan without changing anything.
pub fn plan_upgrade(config: &OperatorConfig) -> Result<UpgradePlan, String> {
    let errors: Vec<String> = config
        .validate()
        .into_iter()
        .filter(|issue| issue.severity == Severity::Error)
        .map(|issue| issue.render())
        .collect();
    if !errors.is_empty() {
        return Err(format!(
            "configuration is not startable:\n{}",
            errors.join("\n")
        ));
    }

    let state = read_database_state(&config.db_path)?;
    let compatible = state.compatible();
    let mut steps = Vec::new();
    match state.schema_version {
        None => steps.push("create the database and apply all migrations".to_string()),
        Some(version) if version == SCHEMA_VERSION => {
            steps.push("no migration needed; schema is current".to_string())
        }
        Some(version) if version < SCHEMA_VERSION => {
            steps.push(format!(
                "take a timestamped backup in {}",
                config.backup_dir
            ));
            steps.push(format!("apply {} migration(s)", SCHEMA_VERSION - version));
            steps.push("verify integrity and schema version".to_string());
        }
        Some(_version) => {
            steps.push(
                "refused: database is newer than this build; restore a compatible backup or use a newer build"
                    .to_string(),
            );
        }
    }

    Ok(UpgradePlan {
        db_path: config.db_path.clone(),
        from_schema: state.schema_version,
        to_schema: SCHEMA_VERSION,
        compatible,
        backup_dir: config.backup_dir.clone(),
        steps,
    })
}

/// The outcome of an upgrade.
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize)]
pub struct UpgradeOutcome {
    pub from_schema: u32,
    pub to_schema: u32,
    pub migrated: bool,
    pub backup_path: Option<String>,
    pub steps: Vec<String>,
}

fn timestamp() -> String {
    chrono::Utc::now().format("%Y%m%dT%H%M%SZ").to_string()
}

/// Take a pre-upgrade backup if the database exists.  Returns the path.
pub fn backup_before_upgrade(config: &OperatorConfig) -> Result<Option<String>, String> {
    if !Path::new(&config.db_path).exists() {
        return Ok(None);
    }
    let archive = Path::new(&config.backup_dir).join(format!(
        "machine-pre-upgrade-{}.db",
        timestamp()
    ));
    backup_database(&config.db_path, &archive)?;
    Ok(Some(archive.to_string_lossy().into_owned()))
}

/// Back up (when needed), migrate, and verify the database.
pub fn run_upgrade(config: &OperatorConfig) -> Result<UpgradeOutcome, String> {
    let plan = plan_upgrade(config)?;
    if !plan.compatible {
        return Err(format!(
            "refusing to upgrade: {} has schema {:?}, newer than this build ({SCHEMA_VERSION}); \
             use a newer build or `machine recover` a compatible backup",
            config.db_path, plan.from_schema
        ));
    }
    let backup_path = backup_before_upgrade(config)?;
    let from_schema = plan.from_schema.unwrap_or(0);

    // Opening the service creates and/or migrates the database.  The service is
    // dropped immediately; the CLI does not keep a reasoner alive.
    {
        let _service = crate::conversation::ConversationService::with_database(
            &config.db_path,
            Some(&config.qa_path),
            Some(&config.store_path),
        )?;
    }

    let state = read_database_state(&config.db_path)?;
    let version = state.schema_version.unwrap_or(0);
    if !state.integrity_ok || version != SCHEMA_VERSION {
        return Err(format!(
            "upgrade verification failed: integrity_ok={}, schema={} (expected {SCHEMA_VERSION}); \
             restore the pre-upgrade backup at {}",
            state.integrity_ok,
            version,
            backup_path.as_deref().unwrap_or("<none>")
        ));
    }

    Ok(UpgradeOutcome {
        from_schema,
        to_schema: version,
        migrated: version != from_schema || from_schema == 0,
        backup_path,
        steps: plan.steps,
    })
}

/// The most recent backup in a directory, by filename (timestamped names sort).
pub fn latest_backup(dir: impl AsRef<Path>) -> Result<Option<PathBuf>, String> {
    let dir = dir.as_ref();
    if !dir.exists() {
        return Ok(None);
    }
    let mut candidates: Vec<PathBuf> = std::fs::read_dir(dir)
        .map_err(|error| format!("read {}: {error}", dir.display()))?
        .filter_map(|entry| entry.ok().map(|entry| entry.path()))
        .filter(|path| {
            path.is_file()
                && path
                    .extension()
                    .map(|extension| extension == "db")
                    .unwrap_or(false)
        })
        .collect();
    candidates.sort();
    Ok(candidates.pop())
}

/// Outcome of a recovery.
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize)]
pub struct RecoveryOutcome {
    pub restored_from: String,
    pub schema_version: u32,
    pub integrity_ok: bool,
}

/// Recovery path after a failed upgrade: restore the most recent backup.
pub fn recover(config: &OperatorConfig) -> Result<RecoveryOutcome, String> {
    let backup = latest_backup(&config.backup_dir)?.ok_or_else(|| {
        format!(
            "no backup found in {}; restore a known-good backup with `machine restore <file>`",
            config.backup_dir
        )
    })?;
    restore_database(&backup, &config.db_path)?;
    let state = read_database_state(&config.db_path)?;
    Ok(RecoveryOutcome {
        restored_from: backup.to_string_lossy().into_owned(),
        schema_version: state.schema_version.unwrap_or(0),
        integrity_ok: state.integrity_ok,
    })
}

/// Create the runtime directories and open the database once so first run has
/// nothing left to guess.  Returns the database state after migrations.
pub fn setup(config: &OperatorConfig) -> Result<DatabaseState, String> {
    for dir in [&config.data_dir, &config.backup_dir] {
        std::fs::create_dir_all(dir)
            .map_err(|error| format!("create {dir}: {error}"))?;
    }
    for file in [&config.db_path, &config.qa_path, &config.store_path] {
        ensure_parent(Path::new(file))?;
    }
    {
        let _service = crate::conversation::ConversationService::with_database(
            &config.db_path,
            Some(&config.qa_path),
            Some(&config.store_path),
        )?;
    }
    read_database_state(&config.db_path)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_dir(tag: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "machine-operator-{tag}-{}-{}",
            std::process::id(),
            timestamp()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn temp_config(tag: &str) -> (PathBuf, OperatorConfig) {
        let dir = temp_dir(tag);
        let mut config = OperatorConfig::default();
        config.data_dir = dir.join("engine").to_string_lossy().into_owned();
        config.db_path = dir.join("engine/machine.db").to_string_lossy().into_owned();
        config.qa_path = dir.join("engine/qa.json").to_string_lossy().into_owned();
        config.store_path = dir.join("engine/sessions.json").to_string_lossy().into_owned();
        config.backup_dir = dir.join("backups").to_string_lossy().into_owned();
        (dir, config)
    }

    #[test]
    fn default_config_is_valid_and_has_one_startup_command() {
        let config = OperatorConfig::default();
        assert!(config.is_ok(), "default config should validate");
        assert!(config.validate().iter().all(|issue| issue.severity != Severity::Error));
        assert!(startup_command().contains("machine"));
        assert!(startup_command().contains("start"));
    }

    #[test]
    fn non_loopback_bind_requires_opt_in_and_token() {
        let mut config = OperatorConfig::default();
        config.host = "0.0.0.0".to_string();
        let issues = config.validate();
        assert!(issues
            .iter()
            .any(|issue| issue.field == "host" && issue.severity == Severity::Error));
        assert!(issues
            .iter()
            .any(|issue| issue.field == "token" && issue.severity == Severity::Error));
        assert!(!config.is_ok());

        config.allow_remote = true;
        config.token = Some("secret".to_string());
        assert!(config.is_ok(), "opt-in plus token should validate");
    }

    #[test]
    fn profile_parse_round_trips_and_reports_cuda_gap() {
        for profile in Profile::all() {
            assert_eq!(Profile::parse(profile.label()), Some(profile));
        }
        assert_eq!(Profile::parse("CUDA"), Some(Profile::Gpu));
        assert_eq!(Profile::parse("nonsense"), None);

        let mut config = OperatorConfig::default();
        config.profile = Profile::Gpu;
        let issues = config.validate();
        // This build has no CUDA feature, so the gpu profile is a warning.
        if projection_path() != ProjectionPath::CudaProjection {
            assert!(issues
                .iter()
                .any(|issue| issue.field == "profile" && issue.severity == Severity::Warning));
        }
    }

    #[test]
    fn inventory_is_versioned_and_ids_are_unique() {
        let json = inventory_json().unwrap();
        assert!(json.contains(INVENTORY_SCHEMA));
        let mut ids: Vec<&str> = CAPABILITIES.iter().map(|entry| entry.id).collect();
        ids.sort_unstable();
        let count = ids.len();
        ids.dedup();
        assert_eq!(ids.len(), count, "capability ids must be unique");

        let counts = capability_status_counts();
        assert_eq!(counts.values().sum::<usize>(), count);
        assert!(capability("operator-cli").is_some());
        assert!(capability("missing").is_none());
    }

    #[test]
    fn inventory_markdown_lists_every_capability() {
        let markdown = inventory_markdown();
        for entry in CAPABILITIES {
            assert!(markdown.contains(entry.id), "missing {} in markdown", entry.id);
        }
    }

    #[test]
    fn setup_creates_and_migrates_then_doctor_is_healthy() {
        let (_dir, config) = temp_config("setup");
        let state = setup(&config).unwrap();
        assert!(state.exists);
        assert_eq!(state.schema_version, Some(SCHEMA_VERSION));
        assert!(state.integrity_ok);

        let report = doctor(&config);
        assert!(
            report.healthy(),
            "doctor should be healthy after setup: {}",
            report.render()
        );
        assert_eq!(report.worst(), CheckStatus::Ok);
    }

    #[test]
    fn doctor_reports_a_newer_database_as_failing() {
        let (_dir, config) = temp_config("newer");
        setup(&config).unwrap();
        // Forge a newer schema version with a direct write.
        let connection = rusqlite::Connection::open(&config.db_path).unwrap();
        connection
            .pragma_update(None, "user_version", SCHEMA_VERSION + 1)
            .unwrap();
        drop(connection);

        let state = read_database_state(&config.db_path).unwrap();
        assert!(!state.compatible());
        let report = doctor(&config);
        assert!(!report.healthy());
        assert!(report
            .checks
            .iter()
            .any(|check| check.name == "database" && check.status == CheckStatus::Fail));
    }

    #[test]
    fn upgrade_is_backup_first_and_recover_restores() {
        let (_dir, config) = temp_config("upgrade");
        setup(&config).unwrap();

        let plan = plan_upgrade(&config).unwrap();
        assert!(plan.compatible);
        assert_eq!(plan.from_schema, Some(SCHEMA_VERSION));

        let outcome = run_upgrade(&config).unwrap();
        assert_eq!(outcome.to_schema, SCHEMA_VERSION);
        assert_eq!(outcome.from_schema, SCHEMA_VERSION);

        // A second upgrade still takes a backup so recovery always has one.
        let second = run_upgrade(&config).unwrap();
        assert!(second.backup_path.is_some());

        let recovered = recover(&config).unwrap();
        assert!(recovered.integrity_ok);
        assert_eq!(recovered.schema_version, SCHEMA_VERSION);
    }

    #[test]
    fn upgrade_refuses_a_newer_database_before_touching_it() {
        let (_dir, config) = temp_config("refuse-newer");
        setup(&config).unwrap();
        let connection = rusqlite::Connection::open(&config.db_path).unwrap();
        connection
            .pragma_update(None, "user_version", SCHEMA_VERSION + 5)
            .unwrap();
        drop(connection);

        let plan = plan_upgrade(&config).unwrap();
        assert!(!plan.compatible);
        let error = run_upgrade(&config).unwrap_err();
        assert!(error.contains("refusing"), "unexpected error: {error}");

        // The forged database is untouched.
        let state = read_database_state(&config.db_path).unwrap();
        assert_eq!(state.schema_version, Some(SCHEMA_VERSION + 5));
    }

    #[test]
    fn backup_and_restore_round_trip_a_database() {
        let (_dir, config) = temp_config("backup");
        setup(&config).unwrap();
        let archive = Path::new(&config.backup_dir).join("manual.db");
        backup_database(&config.db_path, &archive).unwrap();
        assert!(archive.exists());

        // Remove the database, then restore from the archive.
        std::fs::remove_file(&config.db_path).unwrap();
        restore_database(&archive, &config.db_path).unwrap();
        let state = read_database_state(&config.db_path).unwrap();
        assert!(state.exists);
        assert!(state.integrity_ok);
    }

    #[test]
    fn recover_without_a_backup_asks_for_an_explicit_file() {
        let (_dir, config) = temp_config("no-backup");
        let error = recover(&config).unwrap_err();
        assert!(error.contains("no backup"), "unexpected error: {error}");
    }

    #[test]
    fn release_notes_are_tied_to_evaluation_artifacts() {
        let notes = release_notes();
        assert_eq!(notes.schema, RELEASE_NOTES_SCHEMA);
        assert!(!notes.evaluations.is_empty());
        let markdown = release_notes_markdown();
        assert!(markdown.contains("Evaluation results"));
        assert!(markdown.contains(RELEASE_TAG));
        // Each evaluation names a committed artifact path.
        for evaluation in &notes.evaluations {
            assert!(evaluation.artifact.starts_with("docs/"));
        }
    }

    #[test]
    fn write_release_artifacts_creates_all_three_files() {
        let (dir, config) = temp_config("artifacts");
        let docs = dir.join("docs");
        let artifacts = write_release_artifacts(&config, &docs).unwrap();
        for path in [&artifacts.inventory, &artifacts.release_notes, &artifacts.doctor] {
            assert!(path.exists(), "missing artifact {}", path.display());
        }
        let inventory = std::fs::read_to_string(&artifacts.inventory).unwrap();
        assert!(inventory.contains(INVENTORY_SCHEMA));
    }

    #[test]
    fn config_display_redacts_the_token() {
        let mut config = OperatorConfig::default();
        config.token = Some("super-secret".to_string());
        let display = config.display();
        assert!(!display.contains("super-secret"));
        assert!(display.contains("<set>"));
    }
}
