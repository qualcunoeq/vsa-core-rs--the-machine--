//! The Machine — single operator command (Phase 11).
//!
//! ```text
//! cargo run --release --bin machine -- start
//! ```
//!
//! Everyday operation goes through this binary so installation, daily use,
//! upgrades, and recovery no longer require remembering research-specific
//! commands:
//!
//! * `start`   — run the chat interface (delegates to the Phase 2 server)
//! * `setup`   — create runtime directories and open the database once
//! * `doctor`  — health and dependency status
//! * `config`  — `check` (validate) or `show` (effective configuration)
//! * `version` — release, crate, schema, projection, profile
//! * `capabilities` — the versioned capability inventory
//! * `release` — `notes`, `inventory`, or `write` (regenerate artifacts)
//! * `backup` / `restore` — copy state to or from a file
//! * `upgrade` / `recover` — migrate with a backup, or roll back
//!
//! All operational logic lives in `the_machine::operator`; this binary is a
//! thin argument parser and renderer.

use the_machine::chat_server::{serve, ChatServerConfig, ChatServerState};
use the_machine::operator::{
    self, doctor, inventory_json, inventory_markdown, release_notes, release_notes_markdown,
    startup_command, write_release_artifacts, OperatorConfig, Profile, Severity,
};

#[tokio::main]
async fn main() {
    let mut config = OperatorConfig::from_env();
    let mut command: Option<String> = None;
    let mut positional: Vec<String> = Vec::new();
    let mut open = false;
    let mut json = false;
    let mut dry_run = false;
    let mut write = false;

    let args: Vec<String> = std::env::args().skip(1).collect();
    let mut index = 0;
    while index < args.len() {
        let arg = args[index].as_str();
        match arg {
            "--profile" => {
                if let Some(value) = take(&args, &mut index, "--profile") {
                    match Profile::parse(&value) {
                        Some(profile) => config.profile = profile,
                        None => fail(&format!("unknown profile '{value}'")),
                    }
                }
            }
            "--data-dir" => config.data_dir = take_value(&args, &mut index, "--data-dir"),
            "--db" => config.db_path = take_value(&args, &mut index, "--db"),
            "--memory" => config.qa_path = take_value(&args, &mut index, "--memory"),
            "--store" => config.store_path = take_value(&args, &mut index, "--store"),
            "--backup-dir" => config.backup_dir = take_value(&args, &mut index, "--backup-dir"),
            "--host" => config.host = take_value(&args, &mut index, "--host"),
            "--port" => {
                let value = take_value(&args, &mut index, "--port");
                config.port = value.parse().unwrap_or_else(|_| fail("--port must be a number"));
            }
            "--memory-budget" => {
                let value = take_value(&args, &mut index, "--memory-budget");
                config.memory_budget_bytes =
                    value.parse().unwrap_or_else(|_| fail("--memory-budget must be a number"));
            }
            "--allow-remote" => config.allow_remote = true,
            "--semantic-worker" => config.semantic_worker = true,
            "--token" => config.token = Some(take_value(&args, &mut index, "--token")),
            "--open" => open = true,
            "--json" => json = true,
            "--dry-run" => dry_run = true,
            "--write" => write = true,
            "--help" | "-h" => {
                print_help();
                return;
            }
            other if other.starts_with('-') => {
                eprintln!("unknown flag: {other}\n");
                print_help();
                std::process::exit(2);
            }
            other => {
                if command.is_none() {
                    command = Some(other.to_string());
                } else {
                    positional.push(other.to_string());
                }
            }
        }
        index += 1;
    }

    if config.profile.implies_semantic_worker() {
        config.semantic_worker = true;
    }

    match command.as_deref() {
        Some("start") | None => start(config, open).await,
        Some("setup") => setup(config),
        Some("doctor") => doctor_command(config, json),
        Some("config") => config_command(config, positional.first().map(String::as_str), json),
        Some("version") => version(config, json),
        Some("capabilities") => capabilities(json),
        Some("release") => release(config, positional.first().map(String::as_str), json, write),
        Some("backup") => backup(config, positional.first()),
        Some("restore") => restore(config, positional.first()),
        Some("upgrade") => upgrade(config, dry_run),
        Some("recover") => recover(config),
        Some(other) => {
            eprintln!("unknown command: {other}\n");
            print_help();
            std::process::exit(2);
        }
    }
}

fn take(args: &[String], index: &mut usize, flag: &str) -> Option<String> {
    *index += 1;
    match args.get(*index) {
        Some(value) => Some(value.clone()),
        None => {
            eprintln!("{flag} needs a value");
            std::process::exit(2);
        }
    }
}

fn take_value(args: &[String], index: &mut usize, flag: &str) -> String {
    take(args, index, flag).unwrap_or_default()
}

fn fail(message: &str) -> ! {
    eprintln!("{message}");
    std::process::exit(2);
}

/// Print validation errors and stop before doing any work.
fn guard(config: &OperatorConfig) {
    let issues = config.validate();
    let errors: Vec<_> = issues
        .iter()
        .filter(|issue| issue.severity == Severity::Error)
        .collect();
    if !errors.is_empty() {
        eprintln!("configuration is not startable:");
        for issue in errors {
            eprintln!("  {}", issue.render());
        }
        eprintln!("\nrun `machine config check` for the full list");
        std::process::exit(2);
    }
    for issue in issues
        .iter()
        .filter(|issue| issue.severity == Severity::Warning)
    {
        eprintln!("warning: {}", issue.render());
    }
}

async fn start(config: OperatorConfig, open: bool) {
    guard(&config);
    let report = doctor(&config);
    for check in report
        .checks
        .iter()
        .filter(|check| check.status == operator::CheckStatus::Warn)
    {
        eprintln!("warning: {}: {}", check.name, check.detail);
    }

    let server_config: ChatServerConfig = config.chat_server_config();
    let state = match ChatServerState::new(server_config) {
        Ok(state) => std::sync::Arc::new(state),
        Err(error) => fail(&format!("could not open the runtime: {error}")),
    };

    let listener = match tokio::net::TcpListener::bind((config.host.as_str(), config.port)).await {
        Ok(listener) => listener,
        Err(error) => fail(&format!(
            "could not bind {}:{}: {error}",
            config.host, config.port
        )),
    };
    let url = format!("http://{}", listener.local_addr().unwrap());

    println!("The Machine {} — chat interface", operator::RELEASE_TAG);
    println!("  url:        {url}");
    println!("  profile:    {}", config.profile.label());
    println!("  database:   {}", config.db_path);
    println!("  backups:    {}", config.backup_dir);
    if config.token.is_some() {
        println!("  auth:       bearer token required");
    }
    println!("  health:     {url}/api/health");
    println!("Press Ctrl+C to stop (state is saved after every turn).");

    if open {
        open_browser(&url);
    }

    if let Err(error) = serve(listener, state).await {
        fail(&format!("server error: {error}"));
    }
}

fn setup(config: OperatorConfig) {
    guard(&config);
    match operator::setup(&config) {
        Ok(state) => {
            println!("Setup complete.");
            println!("  database:   {}", config.db_path);
            println!(
                "  schema:     {}{}",
                state.schema_version.unwrap_or(0),
                if state.integrity_ok {
                    " (integrity ok)"
                } else {
                    " (INTEGRITY FAILED)"
                }
            );
            println!("  backups:    {}", config.backup_dir);
            println!("\nNext: {}", startup_command());
            let report = doctor(&config);
            if report.healthy() {
                println!("Health: ok");
            } else {
                eprintln!("Health: {}", report.worst().label());
                eprintln!("{}", report.render());
                std::process::exit(1);
            }
        }
        Err(error) => fail(&format!("setup failed: {error}")),
    }
}

fn doctor_command(config: OperatorConfig, json: bool) {
    let report = doctor(&config);
    if json {
        match report.to_json() {
            Ok(text) => println!("{text}"),
            Err(error) => fail(&error),
        }
    } else {
        print!("{}", report.render());
    }
    if !report.healthy() {
        std::process::exit(1);
    }
}

fn config_command(config: OperatorConfig, action: Option<&str>, json: bool) {
    match action {
        Some("show") | None => {
            if json {
                println!(
                    "{{\n  \"profile\": \"{}\",\n  \"db_path\": \"{}\",\n  \"host\": \"{}\",\n  \"port\": {},\n  \"semantic_worker\": {}\n}}",
                    config.profile.label(),
                    config.db_path,
                    config.host,
                    config.port,
                    config.semantic_worker
                );
            } else {
                println!("{}", config.display());
            }
        }
        Some("check") => {
            let issues = config.validate();
            if issues.is_empty() {
                println!("configuration is valid");
                return;
            }
            for issue in &issues {
                println!("{}", issue.render());
            }
            if issues.iter().any(|issue| issue.severity == Severity::Error) {
                std::process::exit(1);
            }
        }
        Some(other) => fail(&format!("unknown config action '{other}' (try check or show)")),
    }
}

fn version(config: OperatorConfig, json: bool) {
    let projection = the_machine::reliability::projection_path().label();
    let schema = the_machine::persistence::SCHEMA_VERSION;
    if json {
        println!(
            "{{\n  \"release\": \"{}\",\n  \"crate\": \"{}\",\n  \"schema\": {},\n  \"projection\": \"{}\",\n  \"profile\": \"{}\",\n  \"semantic_worker\": {}\n}}",
            operator::RELEASE_TAG,
            env!("CARGO_PKG_VERSION"),
            schema,
            projection,
            config.profile.label(),
            config.semantic_worker
        );
    } else {
        println!("release:          {}", operator::RELEASE_TAG);
        println!("crate:            {}", env!("CARGO_PKG_VERSION"));
        println!("persistence:      schema {schema}");
        println!("projection:       {projection}");
        println!("profile:          {}", config.profile.label());
        println!("semantic worker:  {}", config.semantic_worker);
        println!("startup command:  {}", startup_command());
    }
}

fn capabilities(json: bool) {
    if json {
        match inventory_json() {
            Ok(text) => println!("{text}"),
            Err(error) => fail(&error),
        }
    } else {
        print!("{}", inventory_markdown());
    }
}

fn release(config: OperatorConfig, action: Option<&str>, json: bool, write: bool) {
    match action {
        Some("notes") | None => {
            if write {
                write_artifacts(&config);
            } else if json {
                match serde_json::to_string_pretty(&release_notes()) {
                    Ok(text) => println!("{text}"),
                    Err(error) => fail(&format!("serialize release notes: {error}")),
                }
            } else {
                print!("{}", release_notes_markdown());
            }
        }
        Some("inventory") => {
            if write {
                write_artifacts(&config);
            } else if json {
                match inventory_json() {
                    Ok(text) => println!("{text}"),
                    Err(error) => fail(&error),
                }
            } else {
                print!("{}", inventory_markdown());
            }
        }
        Some("write") => write_artifacts(&config),
        Some(other) => fail(&format!(
            "unknown release action '{other}' (try notes, inventory, write)"
        )),
    }
}

fn write_artifacts(config: &OperatorConfig) {
    let docs = operator::repo_docs_dir();
    match write_release_artifacts(config, &docs) {
        Ok(artifacts) => {
            println!("Wrote:");
            println!("  {}", artifacts.inventory.display());
            println!("  {}", artifacts.release_notes.display());
            println!("  {}", artifacts.doctor.display());
        }
        Err(error) => fail(&format!("could not write artifacts: {error}")),
    }
}

fn backup(config: OperatorConfig, target: Option<&String>) {
    let target = target.unwrap_or_else(|| fail("usage: machine backup <file>"));
    match operator::backup_database(&config.db_path, target) {
        Ok(()) => println!("Backup written: {target}"),
        Err(error) => fail(&format!("backup failed: {error}")),
    }
}

fn restore(config: OperatorConfig, source: Option<&String>) {
    let source = source.unwrap_or_else(|| fail("usage: machine restore <file>"));
    match operator::restore_database(source, &config.db_path) {
        Ok(()) => println!("Restored {} from {source}", config.db_path),
        Err(error) => fail(&format!("restore failed: {error}")),
    }
}

fn upgrade(config: OperatorConfig, dry_run: bool) {
    guard(&config);
    if dry_run {
        match operator::plan_upgrade(&config) {
            Ok(plan) => print!("{}", plan.render()),
            Err(error) => fail(&error),
        }
        return;
    }
    match operator::run_upgrade(&config) {
        Ok(outcome) => {
            println!(
                "Upgrade complete: schema {} -> {}",
                outcome.from_schema, outcome.to_schema
            );
            if let Some(path) = &outcome.backup_path {
                println!("  pre-upgrade backup: {path}");
            }
            println!("  recovery:           machine recover");
        }
        Err(error) => fail(&format!("upgrade failed: {error}\nrun `machine recover`")),
    }
}

fn recover(config: OperatorConfig) {
    match operator::recover(&config) {
        Ok(outcome) => {
            println!(
                "Recovered {} from {} (schema {}, integrity ok: {})",
                config.db_path, outcome.restored_from, outcome.schema_version, outcome.integrity_ok
            );
        }
        Err(error) => fail(&format!("recover failed: {error}")),
    }
}

fn open_browser(url: &str) {
    let result = if cfg!(target_os = "macos") {
        std::process::Command::new("open").arg(url).spawn()
    } else if cfg!(target_os = "windows") {
        std::process::Command::new("cmd")
            .args(["/C", "start", "", url])
            .spawn()
    } else {
        std::process::Command::new("xdg-open").arg(url).spawn()
    };
    if result.is_err() {
        eprintln!("could not open a browser automatically; open {url}");
    }
}

fn print_help() {
    println!(
        r#"machine — The Machine operator command ({release})

Usage: machine [command] [options]

Commands:
  start                 run the chat interface (default)
  setup                 create directories and open the database once
  doctor                health and dependency status
  config check|show     validate or display the effective configuration
  version               release, crate, schema, projection, profile
  capabilities          the versioned capability inventory
  release notes|inventory|write
                        show release notes / inventory, or write artifacts
  backup <file>         write a consistent database backup
  restore <file>        restore the database from a backup
  upgrade [--dry-run]   back up, then migrate the schema
  recover               restore the most recent backup

Options:
  --profile <p>         minimal | standard | gpu | semantic_worker
  --data-dir <dir>      data directory (default data/conversation)
  --db <file>           SQLite database (default data/conversation/machine.db)
  --backup-dir <dir>    where upgrades store backups
  --host <addr>         bind address (default 127.0.0.1)
  --port <port>         bind port (default 8787)
  --allow-remote        permit a non-loopback --host (requires --token)
  --token <value>       bearer token (or MACHINE_CHAT_TOKEN)
  --semantic-worker     enable the semantic worker (shadow only)
  --open                open the interface in the default browser
  --json                machine-readable output where supported
  -h, --help            show this help

Environment: MACHINE_PROFILE, MACHINE_DATA_DIR, MACHINE_DB, MACHINE_HOST,
  MACHINE_PORT, MACHINE_CHAT_TOKEN, MACHINE_SEMANTIC_WORKER,
  MACHINE_BACKUP_DIR, MACHINE_MEMORY_BUDGET_BYTES.

First run:   machine setup
Daily use:   {startup}"#,
        release = operator::RELEASE_TAG,
        startup = startup_command()
    );
}
