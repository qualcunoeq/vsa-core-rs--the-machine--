//! Launch the local web interface for the conversational runtime.
//!
//! ```text
//! cargo run --bin machine_chat -- --open
//! ```
//!
//! The interface is embedded in the binary; nothing else needs to run.
//! Durable state lives in one SQLite database; legacy JSON snapshots are
//! imported once into a fresh database and never written again.

use std::sync::Arc;

use the_machine::chat_server::{serve, ChatServerConfig, ChatServerState};
use the_machine::conversation::ConversationService;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut host = "127.0.0.1".to_string();
    let mut port: u16 = 8787;
    let mut data_dir = "data/conversation".to_string();
    let mut db_path = "data/conversation/machine.db".to_string();
    let mut qa_path = "data/qa_memory.json".to_string();
    let mut open = false;
    let mut backup: Option<String> = None;
    let mut restore: Option<String> = None;
    let mut allow_remote = false;
    // A token may come from the flag or the environment.  It is required
    // before binding a non-loopback address.
    let mut token: Option<String> = std::env::var("MACHINE_CHAT_TOKEN").ok();

    let args: Vec<String> = std::env::args().skip(1).collect();
    let mut index = 0;
    while index < args.len() {
        match args[index].as_str() {
            "--host" => {
                index += 1;
                if let Some(value) = args.get(index) {
                    host = value.clone();
                }
            }
            "--port" => {
                index += 1;
                if let Some(value) = args.get(index) {
                    port = value.parse()?;
                }
            }
            "--data-dir" => {
                index += 1;
                if let Some(value) = args.get(index) {
                    data_dir = value.clone();
                }
            }
            "--db" => {
                index += 1;
                if let Some(value) = args.get(index) {
                    db_path = value.clone();
                }
            }
            "--memory" => {
                index += 1;
                if let Some(value) = args.get(index) {
                    qa_path = value.clone();
                }
            }
            "--backup" => {
                index += 1;
                backup = args.get(index).cloned();
                if backup.is_none() {
                    eprintln!("--backup needs a target file path");
                    std::process::exit(2);
                }
            }
            "--restore" => {
                index += 1;
                restore = args.get(index).cloned();
                if restore.is_none() {
                    eprintln!("--restore needs a backup file path");
                    std::process::exit(2);
                }
            }
            "--token" => {
                index += 1;
                match args.get(index) {
                    Some(value) => token = Some(value.clone()),
                    None => {
                        eprintln!("--token needs a value");
                        std::process::exit(2);
                    }
                }
            }
            "--allow-remote" => allow_remote = true,
            "--open" => open = true,
            "--help" | "-h" => {
                print_help();
                return Ok(());
            }
            other => {
                eprintln!("unknown argument: {other}\n");
                print_help();
                std::process::exit(2);
            }
        }
        index += 1;
    }

    let store_path = format!("{data_dir}/sessions.json");
    for path in [&qa_path, &store_path, &db_path] {
        if let Some(parent) = std::path::Path::new(path).parent() {
            if !parent.as_os_str().is_empty() {
                std::fs::create_dir_all(parent)?;
            }
        }
    }

    if let Some(target) = &backup {
        let state = ChatServerState::new(ChatServerConfig {
            db_path: Some(db_path.clone()),
            qa_path: None,
            store_path: None,
            auth_token: None,
        })?;
        state.backup(target)?;
        println!("Backup written: {target}");
        return Ok(());
    }

    if let Some(source) = &restore {
        let mut service =
            ConversationService::with_database(&db_path, None, None)?;
        service.restore(source)?;
        println!("Restored {db_path} from {source}");
        return Ok(());
    }

    // Phase 7: bind loopback by default.  A non-loopback bind requires the
    // explicit --allow-remote opt-in *and* a bearer token, so the interface
    // can never be exposed to another host unauthenticated by accident.
    let loopback = matches!(host.as_str(), "127.0.0.1" | "::1" | "localhost");
    if !loopback {
        if !allow_remote {
            eprintln!(
                "refusing to bind non-loopback address '{host}' without --allow-remote"
            );
            std::process::exit(2);
        }
        if token.is_none() {
            eprintln!(
                "refusing to expose '{host}' without a bearer token; \
                 set --token <value> or MACHINE_CHAT_TOKEN"
            );
            std::process::exit(2);
        }
    }

    let state = Arc::new(ChatServerState::new(ChatServerConfig {
        db_path: Some(db_path.clone()),
        qa_path: Some(qa_path.clone()),
        store_path: Some(store_path.clone()),
        auth_token: token.clone(),
    })?);

    let listener = tokio::net::TcpListener::bind((host.as_str(), port)).await?;
    let url = format!("http://{}", listener.local_addr()?);
    println!("The Machine chat interface: {url}");
    println!("  database:      {db_path}");
    println!("  imports:       {qa_path}, {store_path} (first run only)");
    if token.is_some() {
        println!("  auth:          bearer token required");
    }
    println!("Press Ctrl+C to stop (state is saved after every turn).");

    if open {
        open_browser(&url);
    }

    serve(listener, state).await?;
    Ok(())
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
        "machine_chat — local web interface for The Machine\n\
         \n\
         Usage: machine_chat [options]\n\
         \n\
         Options:\n\
           --host <addr>      interface to bind (default 127.0.0.1, loopback only)\n\
           --port <port>      port to bind (default 8787)\n\
           --allow-remote     permit a non-loopback --host (requires --token)\n\
           --token <value>    bearer token required on /api requests\n\
                              (or set MACHINE_CHAT_TOKEN)\n\
           --db <file>        SQLite database (default data/conversation/machine.db)\n\
           --data-dir <dir>   legacy sessions.json location for first-run import\n\
                              (default data/conversation)\n\
           --memory <file>    legacy QA memory JSON for first-run import\n\
                              (default data/qa_memory.json)\n\
           --backup <file>    write a consistent database backup and exit\n\
           --restore <file>   restore the database from a backup and exit\n\
           --open             open the interface in the default browser\n\
           -h, --help         show this help"
    );
}
