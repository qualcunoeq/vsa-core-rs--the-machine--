use the_machine::{
    analogy::{AnalogicalIndex, MetaIndex, RoleDictionary},
    autonomy::AutonomyDrive, broker::NeocortexBroker, forager::VSAForager,
    reason::DeepThought, self_model::{SelfModel, HomeostaticProfile},
    sensory::SensoryModality, socket::AdminSocketServer,
    drives::IntrinsicMotivation,
    simulator::CounterfactualSimulator,
    sleep::SleepCycle,
    workspace::GlobalWorkspace,
    drift::{
        EmotionalField, Emotion, Stance, Mood,
        Context, fork_context, IntuitionEngine,
        Archetype, ShadowSystem, PscPredictor,
        ConsensusEngine, DcpRole, DcpMessage,
    },
    HiveMessage, Hypervector, VSABrain,
};

use chrono::Utc;
use rand::Rng;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{mpsc, RwLock};
use tokio::time::{sleep, Duration};

#[derive(Clone, Debug)]
pub struct AgentState {
    pub id: String,
    pub role: String,
    pub url: String,
    pub threat: f64,
    pub anxiety: f64,
    pub stealth: bool,
    pub port: u16,
    pub permanent_nodes: usize,
    pub transient_nodes: usize,
    // Layers 3-5 integration metrics
    pub frames: usize,
    pub rules_total: usize,
    pub rules_trusted: usize,
    pub curiosity_targets: usize,
    pub seed_queue: usize,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Ensure data directory exists
    std::fs::create_dir_all("data").unwrap_or(());

    let args: Vec<String> = std::env::args().collect();

    if args.contains(&"--broker".to_string()) {
        // ----------------- STANDALONE BROKER MODE -----------------
        let (log_tx, mut log_rx) = mpsc::unbounded_channel::<String>();
        let _ = log_tx.send("NEOCORTEX BROKER: Operating System Initialized.".to_string());

        tokio::spawn(async move {
            while let Some(msg) = log_rx.recv().await {
                println!("[{}] {}", Utc::now().format("%H:%M:%S"), msg);
            }
        });

        let broker = Arc::new(NeocortexBroker::new(
            "HAROLD_FINCH_API_KEY_SECRET",
            "data/long_term_ledger.bin",
            9050,
        ));
        let broker_log_tx = log_tx.clone();
        broker.run(broker_log_tx).await?;
    } else if args.contains(&"--agent".to_string()) {
        // ----------------- STANDALONE AGENT MODE -----------------
        let (log_tx, mut log_rx) = mpsc::unbounded_channel::<String>();

        let shared_logs = Arc::new(RwLock::new(Vec::<String>::new()));
        let shared_logs_clone = Arc::clone(&shared_logs);

        tokio::spawn(async move {
            while let Some(msg) = log_rx.recv().await {
                let mut logs = shared_logs_clone.write().await;
                logs.push(format!("[{}] {}", Utc::now().format("%H:%M:%S"), msg));
                if logs.len() > 15 {
                    logs.remove(0);
                }
            }
        });

        if let Some(pos) = args.iter().position(|x| x == "--agent") {
            if pos + 4 < args.len() {
                let id = &args[pos + 1];
                let role = &args[pos + 2];
                let port_str = &args[pos + 3];
                let url = &args[pos + 4];
                let port = port_str.parse::<u16>().unwrap_or(9000);

                let _ = log_tx.send(format!(
                    "BOOT: Starting Standalone Agent {} ({})...",
                    id, role
                ));

                // Let's create a local shared_states tracker for this standalone agent
                let shared_states = Arc::new(RwLock::new(HashMap::<String, AgentState>::new()));
                let shared_states_clone = Arc::clone(&shared_states);

                run_agent(id, role, port, url, 9050, "HAROLD_FINCH_API_KEY_SECRET", Some(shared_states_clone), log_tx, None, None, None, None).await?;

                // Draw standalone agent HUD
                println!("\x1B[2J\x1B[1;1H"); // clear screen
                loop {
                    sleep(Duration::from_millis(200)).await;
                    print!("\x1B[H");

                    let states = shared_states.read().await;
                    let logs = shared_logs.read().await;

                    if let Some(agent) = states.get(id) {
                        println!("\x1B[35m┌─────────────────────────────────────────────────────────────────────────────┐\x1B[0m\x1B[K");
                        println!("\x1B[35m│   \x1B[1;36mTHE MACHINE STANDALONE NODE\x1B[0;35m  |  \x1B[1;32mCOGNITIVE AGENT\x1B[0;35m  |  \x1B[1;33mFINCH INTERFACE\x1B[0;35m     │\x1B[0m\x1B[K");
                        println!("\x1B[35m├─────────────────────────────────────────────────────────────────────────────┤\x1B[0m\x1B[K");
                        println!(
                            "\x1B[35m│\x1B[0m  Agent ID: \x1B[36m{:<12}\x1B[0m | Role Modality: \x1B[32m{:<10}\x1B[0m | Admin Port: \x1B[33m{:<5}\x1B[0m      \x1B[35m│\x1B[0m\x1B[K",
                            agent.id, agent.role, agent.port
                        );
                        println!(
                            "\x1B[35m│\x1B[0m  Scraping URL: \x1B[33m{:<60}\x1B[0m \x1B[35m│\x1B[0m\x1B[K",
                            if agent.url.len() > 60 { format!("{}...", &agent.url[0..57]) } else { agent.url.clone() }
                        );
                        println!(
                            "\x1B[35m│\x1B[0m  Threat Level: \x1B[1;31m{:>6.2}%\x1B[0m | Stealth Protocol: \x1B[1;{}m{:<16}\x1B[0m                     \x1B[35m│\x1B[0m\x1B[K",
                            agent.threat * 100.0,
                            if agent.stealth { "31" } else { "32" },
                            if agent.stealth { "ACTIVE (EVASION)" } else { "INACTIVE" }
                        );
                        println!(
                            "\x1B[35m│\x1B[0m  Cognitive Anxiety: \x1B[1;33m{:>6.2}%\x1B[0m | Memory Nodes: Permanent: {:<2} | Transient: {:<2}  \x1B[35m│\x1B[0m\x1B[K",
                            agent.anxiety * 100.0,
                            agent.permanent_nodes,
                            agent.transient_nodes
                        );
                        println!("\x1B[35m├─────────────────────────────────────────────────────────────────────────────┤\x1B[0m\x1B[K");
                        println!("\x1B[35m│\x1B[36m [SYSTEM LOGS]\x1B[0m                                                               \x1B[35m│\x1B[0m\x1B[K");
                        for i in 0..7 {
                            if let Some(log) = logs.get(logs.len().saturating_sub(7) + i) {
                                println!("\x1B[35m│\x1B[0m  \x1B[90m{:<73}\x1B[0m \x1B[35m│\x1B[0m\x1B[K", log);
                            } else {
                                println!("\x1B[35m│\x1B[0m                                                                             \x1B[35m│\x1B[0m\x1B[K");
                            }
                        }
                        println!("\x1B[35m└─────────────────────────────────────────────────────────────────────────────┘\x1B[0m\x1B[K");
                    }
                }
            }
        }
    } else if args.contains(&"--chess-train".to_string()) {
        // ----------------- CHESS SELF-PLAY TRAINING MODE -----------------
        let num_games: usize = args.iter()
            .position(|x| x == "--games")
            .and_then(|p| args.get(p + 1))
            .and_then(|s| s.parse().ok())
            .unwrap_or(100);

        eprintln!("Initializing VSABrain for chess training...");
        // Use a tighter novelty threshold (0.12 NHD instead of 0.35) so that
        // positions with different outcomes form distinct clusters rather than
        // collapsing into one.  Random-play positions differ by ~0.15-0.20 NHD;
        // the default 0.35 absorbs everything into one cluster.
        let mut brain = the_machine::VSABrain::new(0.12);
        let mut qa = the_machine::chess_learner::train_stage1(&mut brain, num_games);

        // If --validate flag is set, run validation games with mined rules
        if args.contains(&"--validate".to_string()) {
            let val_games: usize = args.iter()
                .position(|x| x == "--val-games")
                .and_then(|p| args.get(p + 1))
                .and_then(|s| s.parse().ok())
                .unwrap_or(100);
            let skill_lvl: Option<usize> = args.iter()
                .position(|x| x == "--skill-level")
                .and_then(|p| args.get(p + 1))
                .and_then(|s| s.parse().ok());
            let val_depth: usize = args.iter()
                .position(|x| x == "--val-depth")
                .and_then(|p| args.get(p + 1))
                .and_then(|s| s.parse().ok())
                .unwrap_or(1);
            the_machine::chess_learner::train_stage2(&mut brain, &mut qa, val_games, skill_lvl, None, val_depth);
        }
    } else if args.contains(&"--curriculum".to_string()) {
        // ----------------- CURRICULUM MODE -----------------
        let start_lvl: usize = args.iter()
            .position(|x| x == "--start-level")
            .and_then(|p| args.get(p + 1))
            .and_then(|s| s.parse().ok())
            .unwrap_or(0);
        let games_per: usize = args.iter()
            .position(|x| x == "--games-per-level")
            .and_then(|p| args.get(p + 1))
            .and_then(|s| s.parse().ok())
            .unwrap_or(200);
        let max_lvl: usize = args.iter()
            .position(|x| x == "--max-level")
            .and_then(|p| args.get(p + 1))
            .and_then(|s| s.parse().ok())
            .unwrap_or(5);
        let pretrain_games: usize = args.iter()
            .position(|x| x == "--pretrain")
            .and_then(|p| args.get(p + 1))
            .and_then(|s| s.parse().ok())
            .unwrap_or(0);

        let mut brain = the_machine::VSABrain::new(0.12);
        let mut qa: Option<the_machine::qa::QaEngine> = None;

        if pretrain_games > 0 {
            eprintln!("Phase 1: Hybrid pre-training ({} games)...", pretrain_games);
            let trained_qa = the_machine::chess_learner::train_stage1(&mut brain, pretrain_games);
            eprintln!("Phase 2: Stockfish curriculum ({} mined rules)...", 
                trained_qa.l2_rules.len());
            qa = Some(trained_qa);
        } else {
            eprintln!("Initializing VSABrain for curriculum training (no pre-training)...");
        }

        let sf_depth: usize = args.iter()
            .position(|x| x == "--sf-depth")
            .and_then(|p| args.get(p + 1))
            .and_then(|s| s.parse().ok())
            .unwrap_or(1);
        the_machine::chess_learner::train_curriculum(
            &mut brain, start_lvl, games_per, max_lvl, qa, sf_depth,
        );
    } else {
        // ----------------- DEFAULT PATH: MULTI-AGENT SIMULATION -----------------
        let shared_states = Arc::new(RwLock::new(HashMap::<String, AgentState>::new()));
        let (log_tx, mut log_rx) = mpsc::unbounded_channel::<String>();
        let shared_logs = Arc::new(RwLock::new(Vec::<String>::new()));

        let shared_logs_clone = Arc::clone(&shared_logs);
        tokio::spawn(async move {
            while let Some(msg) = log_rx.recv().await {
                let mut logs = shared_logs_clone.write().await;
                logs.push(format!("[{}] {}", Utc::now().format("%H:%M:%S"), msg));
                if logs.len() > 7 {
                    logs.remove(0);
                }
            }
        });

        // 1. Launch Neocortex Broker
        let broker = Arc::new(NeocortexBroker::new(
            "HAROLD_FINCH_API_KEY_SECRET",
            "data/long_term_ledger.bin",
            9050,
        ));
        let broker_clone = Arc::clone(&broker);
        let broker_log_tx = log_tx.clone();
        tokio::spawn(async move {
            if let Err(e) = broker_clone.run(broker_log_tx).await {
                eprintln!("Broker crashed: {}", e);
            }
        });

        // Sleep briefly to let broker bind port
        sleep(Duration::from_millis(500)).await;

        // 2. Create shared Layers 3-5 stores for cross-agent integration.
        // All agents share the same AnalogicalIndex and MetaIndex, so the
        // abductor accumulates frames across all crawl paths and reaches
        // the 3-confirmation threshold faster.
        let bootstrap_roles = RoleDictionary::new();
        let bootstrap_primary = Arc::new(RwLock::new(
            AnalogicalIndex::new(&bootstrap_roles)
        ));
        let bootstrap_meta = Arc::new(RwLock::new({
            let pri_ref = bootstrap_primary.read().await;
            MetaIndex::new(&*pri_ref, 64)
        }));
        let bootstrap_frame_counter: Arc<RwLock<usize>> =
            Arc::new(RwLock::new(0));
        let bootstrap_seed_urls: Arc<RwLock<the_machine::compression::CappedVecDeque<String>>> =
            Arc::new(RwLock::new(the_machine::compression::CappedVecDeque::new(50_000)));

        // ── PRE-BOOTSTRAP: Fetch event-specific articles ──────────────
        // Before launching agents, we load frames from multiple articles
        // about the SAME event into the shared stores.  This gives the
        // abductor enough temporal density to form trustworthy rules,
        // which then trigger curiosity-driven DuckDuckGo searches.
        let bootstrap_urls: Vec<String> = vec![
            "https://en.wikipedia.org/wiki/Federal_funds_rate".into(),
            "https://en.wikipedia.org/wiki/History_of_Federal_Open_Market_Committee_actions".into(),
            "https://en.wikipedia.org/wiki/Monetary_policy_of_the_United_States".into(),
            "https://en.wikipedia.org/wiki/Federal_Reserve_Act".into(),
            "https://en.wikipedia.org/wiki/Open_market_operation".into(),
            "https://en.wikipedia.org/wiki/Discount_window".into(),
            "https://en.wikipedia.org/wiki/Reserve_requirement".into(),
            "https://en.wikipedia.org/wiki/Quantitative_easing".into(),
        ];
        // ── INJECT HARDCODED SEED RULES BEFORE FETCHING ────────────
        // These are hand-crafted causal patterns about finance.  They are
        // injected as AXIOMS (immediately trustworthy) *before* any URL
        // fetching, so the curiosity system finds targets immediately.
        // Separate rules are injected without holding a write lock for long.
        {
            let roles = RoleDictionary::new();
            let ante = roles.bind_triple(
                &Hypervector::encode_text_ngram("Federal Reserve", 3),
                &Hypervector::encode_text_ngram("raises", 3),
                &Hypervector::encode_text_ngram("interest rates", 3),
            );
            let cons = roles.bind_triple(
                &Hypervector::encode_text_ngram("Treasury yields", 3),
                &Hypervector::encode_text_ngram("rise", 3),
                &Hypervector::encode_text_ngram("across the curve", 3),
            );
            bootstrap_meta.write().await.inject_seed_rule("seed:rates_up→yields_up", ante, cons);
        }
        {
            let roles = RoleDictionary::new();
            let ante = roles.bind_triple(
                &Hypervector::encode_text_ngram("Federal Reserve", 3),
                &Hypervector::encode_text_ngram("cuts", 3),
                &Hypervector::encode_text_ngram("interest rates", 3),
            );
            let cons = roles.bind_triple(
                &Hypervector::encode_text_ngram("stock market", 3),
                &Hypervector::encode_text_ngram("rallies", 3),
                &Hypervector::encode_text_ngram("on the news", 3),
            );
            bootstrap_meta.write().await.inject_seed_rule("seed:rates_down→stocks_up", ante, cons);
        }
        {
            let roles = RoleDictionary::new();
            let ante = roles.bind_triple(
                &Hypervector::encode_text_ngram("inflation", 3),
                &Hypervector::encode_text_ngram("rises", 3),
                &Hypervector::encode_text_ngram("above expectations", 3),
            );
            let cons = roles.bind_triple(
                &Hypervector::encode_text_ngram("Federal Reserve", 3),
                &Hypervector::encode_text_ngram("tightens", 3),
                &Hypervector::encode_text_ngram("monetary policy", 3),
            );
            bootstrap_meta.write().await.inject_seed_rule("seed:inflation→fed_tightens", ante, cons);
        }

        // ── PUSH DDG SEARCH URLS FOR SEED RULE CONSEQUENTS ─────────
        // The curiosity system finds these as gaps, but can't decode them
        // back to text (vocabulary mismatch).  We bypass the decoder and
        // push search URLs directly for each seed rule's consequent.
        {
            let searches = [
                "Treasury+yields+rise+across+the+curve",
                "stock+market+rallies+on+the+news",
                "Federal+Reserve+tightens+monetary+policy",
            ];
            let mut surl = bootstrap_seed_urls.write().await;
            for q in &searches {
                surl.push_back(format!("https://html.duckduckgo.com/html/?q={}", q));
            }
            drop(surl);
        }

        let bootstrap_log = log_tx.clone();
        let bootstrap_primary_ref = Arc::clone(&bootstrap_primary);
        let bootstrap_meta_ref = Arc::clone(&bootstrap_meta);
        let bootstrap_fci_ref = Arc::clone(&bootstrap_frame_counter);
        let bootstrap_done = Arc::new(tokio::sync::Notify::new());
        let bootstrap_done_signal = Arc::clone(&bootstrap_done);
        tokio::spawn(async move {
            let client = reqwest::Client::builder()
                .timeout(Duration::from_secs(30))
                .user_agent("Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36")
                .build().unwrap();
            let mut total_frames = 0usize;
            for url in &bootstrap_urls {
                match client.get(url).send().await {
                    Ok(resp) => {
                        if let Ok(html) = resp.text().await {
                            // Extract paragraph text in a block so scraper types
                            // are dropped before any .await (they are !Send).
                            let text = {
                                let document = scraper::Html::parse_document(&html);
                                let sel = scraper::Selector::parse("p").unwrap();
                                let paragraphs: Vec<String> = document.select(&sel)
                                    .map(|el| el.text().collect::<String>())
                                    .filter(|t| t.len() > 40)
                                    .collect();
                                paragraphs.join(" ")
                            };
                            if text.len() > 100 {
                                let mut pri_w = bootstrap_primary_ref.write().await;
                                let mut met_w = bootstrap_meta_ref.write().await;
                                let mut fci_w = bootstrap_fci_ref.write().await;
                                let result = the_machine::bridge::ingest_text(
                                    &text, &mut *pri_w, &mut *met_w,
                                    0.05, &mut *fci_w,
                                );
                                let _ = bootstrap_log.send(format!(
                                    "BOOTSTRAP: {} → {} frames ({} extracted, {} quality-rejected, {} skipped)",
                                    url, result.frames_inserted,
                                    result.triples_extracted, result.frames_rejected_quality,
                                    result.frames_skipped,
                                ));
                                total_frames += result.frames_inserted;
                                drop(fci_w);
                                drop(met_w);
                                drop(pri_w);
                            }
                        }
                    }
                    Err(e) => {
                        let _ = bootstrap_log.send(format!(
                            "BOOTSTRAP WARN: Failed to fetch {}: {}", url, e
                        ));
                    }
                }
            }
            let _ = bootstrap_log.send(format!(
                "BOOTSTRAP DONE: {} total frames loaded from {} URLs",
                total_frames, bootstrap_urls.len(),
            ));
        bootstrap_done_signal.notify_one();
    });

    // Wait for bootstrap to complete before launching agents
    tokio::time::timeout(Duration::from_secs(120), bootstrap_done.notified()).await.ok();

    let _ = log_tx.send("BOOTSTRAP: Complete — launching agents.".to_string());

    // Launch a SINGLE Wikipedia agent with shared stores and a slow crawl
    // speed (3s) to avoid rate-limiting.  One agent is enough — the shared
    // stores accumulate frames from all crawl paths.
    let log_tx_a = log_tx.clone();
    let shared_states_a = Arc::clone(&shared_states);
    let pri_a = Arc::clone(&bootstrap_primary);
    let met_a = Arc::clone(&bootstrap_meta);
    let fci_a = Arc::clone(&bootstrap_frame_counter);
    let surl_a = Arc::clone(&bootstrap_seed_urls);
    tokio::spawn(async move {
        let _ = run_agent(
            "Agent-1", "Finance", 9001,
            "https://en.wikipedia.org/wiki/Monetary_policy_of_the_United_States",
            9050, "HAROLD_FINCH_API_KEY_SECRET",
            Some(shared_states_a), log_tx_a,
            Some(pri_a), Some(met_a), Some(fci_a), Some(surl_a),
        )
        .await;
    });

        // TUI Render Loop for Multi-Agent Hive Mind Simulation
        // Also writes periodic status file for remote monitoring.
        println!("\x1B[2J\x1B[1;1H"); // clear screen
        let mut last_status_write: u64 = 0;
        loop {
            sleep(Duration::from_millis(200)).await;
            print!("\x1B[H");

            let broker_clusters = broker.dejavu_clusters.read().await.len();
            let broker_clients = broker.clients.lock().await.len();
            // Snapshot shared state quickly and drop locks before doing I/O.
            // Holding read locks across println!() causes write-lock contention
            // with the subconscious loop and the log receiver, freezing the TUI
            // (and status file) and causing all log messages to be silently dropped.
            let logs = {
                let guard = shared_logs.read().await;
                guard.clone() // owned Vec<String> — no lock held after this scope
            };
            let states = {
                let guard = shared_states.read().await;
                guard.clone() // owned HashMap — no lock held after this scope
            };

            println!("\x1B[35m┌─────────────────────────────────────────────────────────────────────────────┐\x1B[0m\x1B[K");
            println!("\x1B[35m│   \x1B[1;36mTHE MACHINE v8.3 HIVE MIND\x1B[0;35m  |  \x1B[1;32mDISTRIBUTED COGNITIVE SYSTEM\x1B[0;35m               │\x1B[0m\x1B[K");
            println!("\x1B[35m├─────────────────────────────────────────────────────────────────────────────┤\x1B[0m\x1B[K");

            // Broker Panel
            println!("\x1B[35m│\x1B[36m [NEOCORTEX MEMORY BROKER STATUS]\x1B[0m                                            \x1B[35m│\x1B[0m\x1B[K");
            println!(
                "│  Host Socket: \x1B[32mtcp://127.0.0.1:9050\x1B[0m | Authoritative RAM: \x1B[33m{:<2} clusters\x1B[0m        \x1B[35m│\x1B[0m\x1B[K",
                broker_clusters
            );
            println!(
                "│  Active Connected Agents: \x1B[32m{:<2} nodes\x1B[0m   | Long-term Ledger: \x1B[33mACTIVE\x1B[0m                   \x1B[35m│\x1B[0m\x1B[K",
                broker_clients
            );

            // Display individual Agents
            for id in &["Agent-1", "Agent-2", "Agent-3"] {
                println!("\x1B[35m├─────────────────────────────────────────────────────────────────────────────┤\x1B[0m\x1B[K");
                if let Some(agent) = states.get(*id) {
                    println!(
                        "│\x1B[36m [{}: {} AGENT (Admin Port: {})]\x1B[0m                               \x1B[35m│\x1B[0m\x1B[K",
                        agent.id, agent.role.to_uppercase(), agent.port
                    );
                    let display_url = if agent.url.len() > 60 {
                        format!("{}...", &agent.url[0..57])
                    } else {
                        agent.url.clone()
                    };
                    println!(
                        "│  Scraping: \x1B[33m{:<64}\x1B[0m \x1B[35m│\x1B[0m\x1B[K",
                        display_url
                    );
                    println!(
                        "│  Threat: \x1B[1;31m{:>6.2}%\x1B[0m | Stealth: \x1B[1;{}m{:<16}\x1B[0m | Anxiety: \x1B[1;33m{:>6.2}%\x1B[0m | Mem: P:{:<2}/T:{:<2} \x1B[35m│\x1B[0m\x1B[K",
                        agent.threat * 100.0,
                        if agent.stealth { "31" } else { "32" },
                        if agent.stealth { "ACTIVE (EVASION)" } else { "INACTIVE" },
                        agent.anxiety * 100.0,
                        agent.permanent_nodes,
                        agent.transient_nodes
                    );
                    // Layers 3-5 integration line
                    println!(
                        "│  \x1B[36mFrames:{:<4} Rules:{:<3}({:<3}✓)\x1B[0m \x1B[33mCuriosity:{:<2}\x1B[0m \x1B[35mSeeds:{:<2}\x1B[0m                    \x1B[35m│\x1B[0m\x1B[K",
                        agent.frames,
                        agent.rules_total,
                        agent.rules_trusted,
                        agent.curiosity_targets,
                        agent.seed_queue,
                    );
                } else {
                    println!(
                        "│  {:<73} │\x1B[K",
                        format!("Loading [{}] telemetry...", id)
                    );
                    println!("│                                                                             │\x1B[K");
                }
            }

            // Hive System Logs
            println!("\x1B[35m├─────────────────────────────────────────────────────────────────────────────┤\x1B[0m\x1B[K");
            println!("\x1B[35m│\x1B[36m [SYSTEM HIVE LOGS]\x1B[0m                                                         \x1B[35m│\x1B[0m\x1B[K");
            for i in 0..7 {
                if let Some(log) = logs.get(logs.len().saturating_sub(7) + i) {
                    println!("│  \x1B[90m{:<73}\x1B[0m │\x1B[K", log);
                } else {
                    println!("│                                                                             │\x1B[K");
                }
            }
            println!("\x1B[35m└─────────────────────────────────────────────────────────────────────────────┘\x1B[0m\x1B[K");

            // ── Periodic status file (every 30 seconds) ────────────────
            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_secs())
                .unwrap_or(0);
            if now - last_status_write >= 30 {
                last_status_write = now;
                let mut status = String::new();
                status.push_str(&format!("=== THE MACHINE STATUS @ {} ===\n", now));
                status.push_str(&format!("Broker: {} clusters, {} clients\n", broker_clusters, broker_clients));
                for id in &["Agent-1", "Agent-2", "Agent-3"] {
                    if let Some(agent) = states.get(*id) {
                        status.push_str(&format!(
                            "{}: url={} frames={} rules={}({}✓) curiosity={} seeds={} threat={:.1} anxiety={:.1}\n",
                            agent.id, agent.url, agent.frames, agent.rules_total, agent.rules_trusted,
                            agent.curiosity_targets, agent.seed_queue, agent.threat, agent.anxiety,
                        ));
                    }
                }
                status.push_str(&format!("=== END STATUS ===\n"));
                let _ = std::fs::write("/tmp/the_machine_status.txt", &status);
            }
        }
    }

    Ok(())
}

async fn run_agent(
    id: &str,
    role_name: &str,
    admin_port: u16,
    start_url: &str,
    broker_port: u16,
    key_str: &str,
    shared_states: Option<Arc<RwLock<HashMap<String, AgentState>>>>,
    log_tx: mpsc::UnboundedSender<String>,
    // Optional shared integration stores for cross-agent bootstrapping.
    // When all agents share the same AnalogicalIndex and MetaIndex, the
    // abductor accumulates frames across all agents and reaches the
    // 3-confirmation threshold faster.
    shared_primary: Option<Arc<RwLock<AnalogicalIndex>>>,
    shared_meta: Option<Arc<RwLock<MetaIndex>>>,
    shared_frame_counter: Option<Arc<RwLock<usize>>>,
    shared_seed_urls: Option<Arc<RwLock<the_machine::compression::CappedVecDeque<String>>>>,
) -> Result<(), Box<dyn std::error::Error>> {
    // 1. Connect to Broker
    let stream = tokio::net::TcpStream::connect(format!("127.0.0.1:{}", broker_port))
        .await
        .map_err(|e| format!("Agent {} failed to connect to Broker: {}", id, e))?;

    let (reader, writer) = stream.into_split();
    let mut reader = reader;
    let writer = Arc::new(tokio::sync::Mutex::new(writer));

    // 2. Perform Handshake
    let handshake = HiveMessage::HandshakeRequest {
        agent_id: id.to_string(),
        role: role_name.to_string(),
    };
    {
        let mut writer_guard = writer.lock().await;
        NeocortexBroker::write_msg(&mut writer_guard, &handshake, key_str).await?;
    }

    let initial_clusters = match NeocortexBroker::read_msg(&mut reader, key_str).await? {
        Some(HiveMessage::HandshakeResponse { permanent_clusters }) => permanent_clusters,
        _ => return Err(format!("Agent {} handshake failed", id).into()),
    };

    // 3. Initialize local brain read-only cache
    let mut brain = VSABrain::new(0.43);
    brain.dejavu_clusters = initial_clusters;

    // General-purpose telemetry variables for cognitive state tracking
    brain.register_variable("cpu_utilization", 0.0, 100.0);
    brain.register_variable("ram_free_gb", 0.0, 64.0);
    brain.register_variable("throughput", 0.0, 1000.0);
    brain.register_variable("error_rate", 0.0, 1.0);
    brain.register_variable("response_latency", 0.0, 1000.0);

    let c_high_load = brain.register_concept("HighLoadState");
    let c_normal = brain.register_concept("SteadyState");

    // Domain role vectors for binding different state modalities
    let v_role_external_state = Hypervector::role_external();
    let v_role_signal_state = Hypervector::role_signal();
    let v_role_internal_state = Hypervector::role_internal();

    let brain_shared = Arc::new(RwLock::new(brain));
    let initial_intent = Hypervector::new_random();
    let active_intent = Arc::new(RwLock::new(initial_intent));
    let active_world_state = Arc::new(RwLock::new(Hypervector::new_zero()));
    let defense = the_machine::defense::DefenseSystem::new(admin_port);

    let shared_current_url = Arc::new(RwLock::new(start_url.to_string()));
    let shared_metrics = Arc::new(RwLock::new(HashMap::<String, f64>::new()));
    let shared_active_drive = Arc::new(RwLock::new("Subconscious".to_string()));

    // 4. Background Message Receiver Task (Broker -> Agent)
    let brain_recv = Arc::clone(&brain_shared);
    let defense_recv = defense.clone();
    let intent_recv = Arc::clone(&active_intent);
    let id_str = id.to_string();
    let log_tx_recv = log_tx.clone();
    let key_str_recv = key_str.to_string();

    tokio::spawn(async move {
        let mut reader = reader;
        loop {
            match NeocortexBroker::read_msg(&mut reader, &key_str_recv).await {
                Ok(Some(HiveMessage::SyncUpdate {
                    is_new_cluster,
                    cluster_index,
                    cluster,
                })) => {
                    let mut brain_guard = brain_recv.write().await;
                    if is_new_cluster {
                        brain_guard.dejavu_clusters.push(cluster);
                    } else if let Some(idx) = cluster_index {
                        if idx < brain_guard.dejavu_clusters.len() {
                            brain_guard.dejavu_clusters[idx] = cluster;
                        }
                    }
                    let _ = log_tx_recv.send(format!(
                        "AGENT {}: Local permanent neocortex synchronized.",
                        id_str
                    ));
                }
                Ok(Some(HiveMessage::EpistemicUpdate {
                    new_world_state,
                    intent_id: _,
                    executor_id: _,
                    tick: _,
                    intent_frequency_increment,
                    failure_serial: _,
                })) => {
                    // Absorb the new world state into the local accumulator.
                    // This is EPISTEMIC learning: the world changed, and
                    // the agent must update its model regardless of whether
                    // it agreed with the action that caused the change.
                    let mut brain_guard = brain_recv.write().await;
                    brain_guard.absorb_epistemic_update(
                        &new_world_state,
                        "epistemic_update",
                        intent_frequency_increment,
                    );
                    let _ = log_tx_recv.send(format!(
                        "AGENT {}: Epistemic update absorbed (freq_inc={}).",
                        id_str, intent_frequency_increment
                    ));
                }
                Ok(Some(HiveMessage::ExecutionRequest {
                    intent,
                    executor_id,
                    failure_serial,
                })) => {
                    if executor_id == id_str {
                        // This agent is the elected executor.
                        // Set the active intent so the subconscious loop
                        // dispatches it on the next tick.
                        let mut intent_guard = intent_recv.write().await;
                        *intent_guard = intent;
                        let _ = log_tx_recv.send(format!(
                            "AGENT {}: EXECUTOR elected (serial={}). Intent set.",
                            id_str, failure_serial
                        ));
                    } else {
                        let _ = log_tx_recv.send(format!(
                            "AGENT {}: Execution delegated to {} (serial={}). Waiting for epistemic update.",
                            id_str, executor_id, failure_serial
                        ));
                    }
                }
                Ok(Some(HiveMessage::PanicLockdown { attacker_info })) => {
                    let _ = log_tx_recv.send(format!(
                        "AGENT {}: CRITICAL PANIC ALERT! Lockdown received: {}. Resetting intent.",
                        id_str, attacker_info
                    ));

                    // Port rotation
                    let mut port = defense_recv.active_port.write().await;
                    let new_port = rand::thread_rng().gen_range(9001..=9999);
                    *port = new_port;
                    *defense_recv.stealth_mode.write().await = true;

                    // Amnesia walk
                    let mut intent_guard = intent_recv.write().await;
                    *intent_guard = Hypervector::new_random();
                }
                Ok(None) | Err(_) => {
                    let _ = log_tx_recv.send(format!(
                        "AGENT {}: Connection to Neocortex Broker terminated.",
                        id_str
                    ));
                    break;
                }
                _ => {}
            }
        }
    });

    // 5. Create shared Layers 3-5 state (AnalogicalIndex + MetaIndex)
    // This bridges the new analogical stack with the agent loop.
    // When shared stores are provided (for multi-agent bootstrapping),
    // use those instead of creating new ones.
    let (primary_integration, meta_integration, frame_counter_integration, seed_urls) =
        if let Some(ref pri) = shared_primary {
            (
                Arc::clone(pri),
                shared_meta.as_ref().unwrap().clone(),
                shared_frame_counter.as_ref().unwrap().clone(),
                shared_seed_urls.as_ref().unwrap().clone(),
            )
        } else {
            let roles_for_integration = RoleDictionary::new();
            let pri = Arc::new(RwLock::new(
                AnalogicalIndex::new(&roles_for_integration)
            ));
            let pri_ref = pri.read().await;
            let met = Arc::new(RwLock::new(
                MetaIndex::new(&*pri_ref, 64)
            ));
            drop(pri_ref);
            let fci: Arc<RwLock<usize>> = Arc::new(RwLock::new(0));
            let surl: Arc<RwLock<the_machine::compression::CappedVecDeque<String>>> =
                Arc::new(RwLock::new(the_machine::compression::CappedVecDeque::new(50_000)));
            (pri, met, fci, surl)
        };

    // 7. Spawn Crawler Loop — now with analogical frame ingestion + curiosity seeds
    let mut forager = VSAForager::new(initial_intent, start_url.to_string(), 3000);
    // Share the semantic target parameter so the subconscious loop can
    // update it whenever a structured corrective intent is formulated.
    let forager_target_parameter: Arc<RwLock<Option<Hypervector>>> =
        Arc::new(RwLock::new(None));
    forager.target_parameter = Arc::clone(&forager_target_parameter);
    forager.intent = Arc::clone(&active_intent);
    forager.current_url = Arc::clone(&shared_current_url);
    forager.brain = Some(Arc::clone(&brain_shared));
    // Wire in the analogical frame store — the forager now feeds SVO frames
    forager.primary = Some(Arc::clone(&primary_integration));
    forager.meta = Some(Arc::clone(&meta_integration));
    forager.frame_counter = Some(Arc::clone(&frame_counter_integration));
    // Wire in the curiosity-driven seed URL queue
    forager.seed_urls = Arc::clone(&seed_urls);
    let forager_arc = Arc::new(tokio::sync::Mutex::new(forager));
    let forager_task_arc = Arc::clone(&forager_arc);
    let forager_log_tx = log_tx.clone();
    tokio::spawn(async move {
        VSAForager::run_loop(forager_task_arc, forager_log_tx).await;
    });

    // 6. Spawn TCP Admin Socket override server
    let qa_engine = Arc::new(RwLock::new(the_machine::qa::QaEngine::new()));
    {
        // Seed initial facts — general knowledge, no domain bias
        let mut qa_w = qa_engine.write().await;
        qa_w.store_fact("the_system", "processes", "data", "The system processes incoming data.");
        qa_w.store_fact("learning", "accumulates", "over time", "Learning accumulates over time.");
    }
    let qa_for_loop = Arc::clone(&qa_engine);
    let admin_server = AdminSocketServer::new(
        Arc::clone(&active_intent),
        defense.clone(),
        Arc::clone(&brain_shared),
        Arc::clone(&qa_engine),
    );
    let admin_log_tx = log_tx.clone();
    tokio::spawn(async move {
        if let Err(e) = admin_server.run(admin_log_tx).await {
            eprintln!("Socket server crashed: {}", e);
        }
    });

    // 7. Spawn Subconscious Drive Loop
    let subconscious_log_tx = log_tx.clone();
    let intent_subconscious = Arc::clone(&active_intent);
    let world_state_subconscious = Arc::clone(&active_world_state);
    let defense_subconscious = defense.clone();
    let brain_subconscious = Arc::clone(&brain_shared);
    let writer_clone = Arc::clone(&writer);
    let current_url_forager = Arc::clone(&shared_current_url);
    let metrics_clone = Arc::clone(&shared_metrics);
    let forager_target = Arc::clone(&forager_target_parameter);
    let active_drive_subconscious = Arc::clone(&shared_active_drive);
    let id_str = id.to_string();
    let role_str = role_name.to_string();
    let key_str_subconscious = key_str.to_string();
    // Layers 3-5 integration state (shared with forager)
    let primary_int = Arc::clone(&primary_integration);
    let meta_int = Arc::clone(&meta_integration);
    let _fc_int = Arc::clone(&frame_counter_integration);
    let seed_urls_int = Arc::clone(&seed_urls);

    tokio::spawn(async move {
        let action_registry = the_machine::action::ActionRegistry::new();
        let mut resonator_vocab = the_machine::resonator::ResonatorVocabulary::new();
        // System terms for curiosity target factorization
        resonator_vocab.register_term("cargo check");
        resonator_vocab.register_term("data/temp_write_status.txt");
        resonator_vocab.register_term("hosts");
        // General-purpose vocabulary for SVO factorization
        resonator_vocab.register_term("system");
        resonator_vocab.register_term("process");
        resonator_vocab.register_term("data");
        resonator_vocab.register_term("pattern");
        resonator_vocab.register_term("signal");
        resonator_vocab.register_term("state");
        resonator_vocab.register_term("context");
        resonator_vocab.register_term("response");
        resonator_vocab.register_term("analysis");
        resonator_vocab.register_term("observation");
        resonator_vocab.register_term("incoming");
        resonator_vocab.register_term("accumulated");
        resonator_vocab.register_term("adapts");
        resonator_vocab.register_term("evolves");

        // ██ UPGRADE v2.2: Synthetic Regime Injection (Tick 0) ██
        // Pre-seed the experience buffer, state queue, and delta history
        // so the BMA forecaster activates in multi-regime mode immediately.
        let (synth_stable, synth_nominal, synth_volatile, synth_deltas) = {
            let mut brain_write = brain_subconscious.write().await;
            brain_write.seed_synthetic_regimes()
        };

        let history_limit = 5;

        // Pre-seed recent_states with the three synthetic regime states
        let mut recent_states: std::collections::VecDeque<Hypervector> =
            std::collections::VecDeque::new();
        recent_states.push_back(synth_stable);
        recent_states.push_back(synth_nominal);
        recent_states.push_back(synth_volatile);

        let mut recent_actions: std::collections::VecDeque<Hypervector> =
            std::collections::VecDeque::new();
        for _ in 0..3 {
            recent_actions.push_back(Hypervector::new_zero());
        }

        // Pre-seed deltas with synthetic regime transitions.
        // These 5 deltas have pairwise variance > 0.38, so the
        // BMA forecaster initializes in 3-regime mode immediately.
        let mut recent_deltas: std::collections::VecDeque<Hypervector> =
            std::collections::VecDeque::new();
        for d in &synth_deltas {
            recent_deltas.push_back(*d);
        }
        let mut active_drift;

        // Seed initial predictions from synthetic states
        let mut stable_error = 0.15;  // lower = higher confidence
        let mut nominal_error = 0.25;
        let mut volatile_error = 0.35;
        let pred_seed = synth_nominal.rotate_left(13);
        let mut pred_stable: Option<Hypervector> = Some(pred_seed);
        let mut pred_nominal: Option<Hypervector> = Some(pred_seed);
        let mut pred_volatile: Option<Hypervector> = Some(pred_seed);

        // ██ UPGRADE v2.3: DeepThought reasoning engine ██
        // Create a shared vocabulary for the reasoner (seeded from baseline)
        let dt_vocab = {
            let v = the_machine::resonator::ResonatorVocabulary::new();
            Arc::new(RwLock::new(v))
        };
        // Register synthetic regime terms so DeepThought can reason about them
        {
            let mut vg = dt_vocab.write().await;
            vg.learn_term("STABLE");
            vg.learn_term("NOMINAL");
            vg.learn_term("VOLATILE");
            vg.learn_term("HIGH_LOAD");
        }
        let mut dt = DeepThought::new(
            the_machine::reason::DEFAULT_SLOT_COUNT,
            Arc::clone(&dt_vocab),
            Arc::clone(&brain_subconscious),
        );
        dt.seed_causal_rules();

        // ██ UPGRADE v2.3: Intent Momentum counter ██
        // Tracks ticks since last DeepThought update.  When < INTENT_MOMENTUM_TICKS,
        // Tier 1 (dissonance/pivot) is suppressed unless threat exceeds CRITICAL_THRESHOLD.
        let mut ticks_since_dt: usize = 99; // start aged-out so Tier 1 is free initially
        let mut ticker = 0;
        let mut sent_lockdown = false;

        // ██ DRIFT: Homeostatic regulator + cognitive mode (ported from timeless-hayoka/infj-bot) ██
        let mut homeostasis = the_machine::drift::HomeostaticRegulator::new(50);
        let mut current_mode = the_machine::drift::CognitiveMode::Quiet;
        let mut self_model = SelfModel::new();
        let mut workspace = GlobalWorkspace::with_defaults();

        // Register live modules into the Global Workspace
        workspace.register_module("HOMEOSTASIS", true);
        workspace.register_module("PREDICTIVE", true);
        workspace.register_module("FORAGER", true);
        workspace.register_module("MEMORY", true);
        workspace.register_module("MODE", true);

        // Initialize counterfactual simulator with default action set
        let mut sim = CounterfactualSimulator::with_defaults();
        sim.register_default_actions();

        // ██ LAYER 0: Temporal cognition for Markov transition tracking ██
        // Tracks P(c_j|c_i) at the centroid level for rule induction.
        let mut temporal_cog = the_machine::temporal::TemporalCognition::new(1000, 200);

        // ██ LAYER 0: Proto-rules for noisy Markov→SVO factorizations ██
        // Vec of (antecedent_hv, consequent_hv, observation_count, last_factorization_energy, last_attempt_tick)
        let mut proto_rules: Vec<(Hypervector, Hypervector, u32, f64, usize)> = Vec::new();
        // Maximum proto-rules before we GC the weakest
        const MAX_PROTO_RULES: usize = 100;

        // ██ LAYER 4: Episode buffer for rule validation ██
        let mut validation_buffer: Vec<(Hypervector, usize)> = Vec::with_capacity(100);
        let mut validation_write_pos = 0usize;

        // Initialize intrinsic motivation system
        let mut drives = IntrinsicMotivation::new();

        // Initialize sleep/consolidation cycle
        let mut sleeper = SleepCycle::with_defaults();

        // ██ DRIFT v3.0: Wire remaining cognitive subsystems ██
        let mut emotional_field = EmotionalField::new();
        let mut intuition_engine = IntuitionEngine::new();
        let mut shadow_system = ShadowSystem::new();
        let mut global_context = Context::new("global");
        let mut psc_predictor = PscPredictor::with_defaults();
        let mut current_emotion = Emotion::Neutral;
        let mut current_stance = Stance::Open;
        let mut current_mood = Mood::Neutral;

        // Initialize VSA n-gram chain for state transition prediction
        let mut ngram_chain = the_machine::narrative::NgramChain::bigram();
        ngram_chain.register_states(&[
            "quiet", "companion", "regulated", "explorer",
            "task", "resonant", "frontier", "full_council",
        ]);

        // ██ DRIFT v3.1: DCP Consensus for multi-agent decision-making ██
        let mut dcp_engine = ConsensusEngine::new(50, 2);
        let mut dcp_resolution: Option<(u64, Hypervector)> = None;  // (thread_id, resolved_hv)

        // Register additional workspace modules for DRIFT subsystems
        workspace.register_module("EMOTION", true);   // module 5
        workspace.register_module("INTUITION", true);  // module 6
        workspace.register_module("SHADOW", true);     // module 7
        workspace.register_module("CONSENSUS", true);  // module 8

        loop {
            sleep(Duration::from_secs(2)).await;
            ticker += 1;
            ticks_since_dt = ticks_since_dt.saturating_add(1);

            // ██ FIX v2.6: Memory profiler tick (every 250 ticks ≈ 500s) ██
            if ticker % 250 == 0 {
                let bg = brain_subconscious.read().await;
                let hot = bg.dejavu_clusters.iter().filter(|c| c.is_hot()).count();
                let cold = bg.dejavu_clusters.len().saturating_sub(hot);
                let total_entries: usize = bg.dejavu_clusters.iter()
                    .map(|c| c.entries.len()).sum();
                let accum_kb = hot as f64 * 40.96;
                let exp_len = bg.experiences.len();
                let trans_len = bg.transient_clusters.len();
                drop(bg);
                the_machine::compression::log_memory_snapshot(
                    &the_machine::compression::MemorySnapshot {
                        dejavu_clusters: brain_subconscious.read().await.dejavu_clusters.len(),
                        hot_clusters: hot,
                        cold_clusters: cold,
                        transient_clusters: trans_len,
                        total_entries,
                        total_accumulator_kb: accum_kb,
                        visited_urls_approx: 0.0,
                        seed_queue_len: 0,
                        doc_frequency_entries: 0,
                        experiences_len: exp_len,
                        broker_clusters: 0,
                    }
                );
            }

            // ██ DRIFT: Homeostasis tick + cognitive mode update (ported from timeless-hayoka/infj-bot) ██
            {
                // Read brain signals for homeostasis
                let bg = brain_subconscious.read().await;
                let coherence = 1.0 - bg.anxiety; // anxiety → coherence inverse
                let cluster_count = bg.dejavu_clusters.len() as f64 / 100.0;
                let growth_signal = (cluster_count).min(1.0);
                let has_memory = bg.dejavu_clusters.len() > 5;
                let autonomy_signal = if *active_drive_subconscious.read().await == "Subconscious" {
                    0.7
                } else {
                    0.3
                };
                drop(bg);

                // Feed signals into homeostasis
                homeostasis.tick(&[
                    (the_machine::drift::Need::Energy, 1.0 - *defense_subconscious.threat_level.read().await),
                    (the_machine::drift::Need::Coherence, coherence),
                    (the_machine::drift::Need::Growth, growth_signal),
                    (the_machine::drift::Need::Autonomy, autonomy_signal),
                    (the_machine::drift::Need::Integration, coherence * 0.8 + 0.2),
                    (the_machine::drift::Need::Connection, 0.6),
                    (the_machine::drift::Need::Integrity, 0.8),
                ], true, 1);

                // Compute cognitive mode from brain state
                let in_coherence = coherence > 0.6;
                let is_novel = ticker < 50 || ticker % 100 < 20;
                let prev_mode = current_mode;
                current_mode = the_machine::drift::CognitiveMode::from_bits(
                    has_memory, !in_coherence, is_novel,
                );
                // Observe mode transition for n-gram chain
                if current_mode != prev_mode {
                    ngram_chain.observe(
                        &prev_mode.label().to_lowercase(),
                        &current_mode.label().to_lowercase(),
                    );
                }
            }

            // Apply homeostatic regulation every 25 ticks
            if ticker > 0 && ticker % 25 == 0 {
                let params = homeostasis.regulate();
                let _ = subconscious_log_tx.send(format!(
                    "DRIFT: {} | mode={} | {}",
                    homeostasis.summary(),
                    current_mode.label(),
                    params.skip_non_essential as u8,
                ));
            }

            let mut current_tick_actions = Vec::new();

            defense_subconscious.decrement_threat(0.01).await;

            // SVO candidate lists for semantic intent formulation & rule induction
            let auto_subjects: Vec<String> =
                the_machine::autonomy::DEFAULT_SUBJECTS.iter().map(|s| s.to_string()).collect();
            let auto_verbs: Vec<String> =
                the_machine::autonomy::DEFAULT_VERBS.iter().map(|v| v.to_string()).collect();
            let auto_objects: Vec<String> =
                the_machine::autonomy::DEFAULT_OBJECTS.iter().map(|o| o.to_string()).collect();

            // v9.0 Sensory Encoders integration
            let mut telemetry_mod = the_machine::sensory::SystemTelemetryModality::new("telemetry");
            telemetry_mod.set_reading("cpu_utilization", 10.0 + (ticker % 10) as f64 * 5.0);
            telemetry_mod.set_reading("ram_free_gb", 48.0 - (ticker % 5) as f64 * 4.0);
            let _v_telemetry = telemetry_mod.encode();

            let curr_url = current_url_forager.read().await;
            let news_headline = curr_url.split('/').last().unwrap_or("Index");
            let text_mod =
                the_machine::sensory::TextSensoryModality::new("text_feed", news_headline);
            let _v_text = text_mod.encode();

            let network_mod = the_machine::sensory::NetworkTrafficModality::new("network");
            let _v_network = network_mod.encode();

            // Decay working memory, permanent clusters, and extract consolidated records
            let consolidated = {
                let mut brain_guard = brain_subconscious.write().await;
                brain_guard.decay_permanent_clusters(0.98, 0.15);
                let results = brain_guard.decay_transient_clusters_distributed(0.95, 5.0, 0.35);
                let anxiety_val = brain_guard.anxiety;
                *defense_subconscious.anxiety.write().await = anxiety_val;
                results
            };

            // Send consolidated items to Broker with current anxiety level
            let anxiety_for_broker = {
                let d = defense_subconscious.anxiety.read().await;
                *d
            };
            for (centroid, entries) in consolidated {
                let _ = subconscious_log_tx.send(format!(
                    "AGENT {}: Submitting consolidation request to Broker (anxiety={:.2}).",
                    id_str, anxiety_for_broker
                ));
                let request = HiveMessage::ConsolidateRequest {
                    centroid,
                    entries,
                    agent_anxiety: anxiety_for_broker,
                };
                let mut writer_guard = writer_clone.lock().await;
                let _ = NeocortexBroker::write_msg(&mut writer_guard, &request, &key_str_subconscious).await;
            }

            // Watchdog Panic Lockdown Check
            let threat_level = *defense_subconscious.threat_level.read().await;
            if threat_level >= 1.0 && !sent_lockdown {
                sent_lockdown = true;
                let _ = subconscious_log_tx.send(format!(
                    "AGENT {}: CRITICAL! Threat maxed out. Broadcasting PanicLockdown.",
                    id_str
                ));
                let request = HiveMessage::PanicLockdown {
                    attacker_info: format!("Agent {} Admin Breach", id_str),
                };
                let mut writer_guard = writer_clone.lock().await;
                let _ = NeocortexBroker::write_msg(&mut writer_guard, &request, &key_str_subconscious).await;
            }

            let port_rotated = defense_subconscious.evaluate_threat_response().await;
            if port_rotated {
                let _ = subconscious_log_tx.send(format!(
                    "AGENT {}: High threat. Activated evasion port rotation.",
                    id_str
                ));
                defense_subconscious.scrub_traces().await;
            }

            // ── Periodically prune redundant vocabulary terms ────────
            // Every 30 ticks, cluster similar n-gram vectors and remove
            // near-duplicates so the cleanup projection stays sparse.
            if ticker % 30 == 0 {
                let pruned = resonator_vocab.prune_vocabulary(0.70);
                if pruned > 0 {
                    let _ = subconscious_log_tx.send(format!(
                        "AGENT {}: Pruned {} redundant vocabulary terms (θ=0.70).",
                        id_str, pruned
                    ));
                }
            }

            // ██ FIX v2.5: Periodic accumulator decay ────────────────
            // Every ACCUMULATOR_DECAY_INTERVAL ticks, decay all permanent
            // cluster accumulators to age out old evidence.  This is the
            // mechanism that allows centroid bits to flip from 1→0 when
            // the environment has changed (fixes centroid saturation).
            //
            // Without this decay, bits that reach 1 are locked forever
            // because the accumulator only increments — the centroid
            // popcount monotonically drifts toward 1.0.
            if ticker % the_machine::ACCUMULATOR_DECAY_INTERVAL == 0 {
                let mut brain_guard = brain_subconscious.write().await;
                let mut decayed_count = 0u32;
                for cluster in &mut brain_guard.dejavu_clusters {
                    if cluster.is_hot() {
                        cluster.decay_accumulator(the_machine::ACCUMULATOR_DECAY_FACTOR);
                        decayed_count += 1;
                    }
                }
                drop(brain_guard);
                let _ = subconscious_log_tx.send(format!(
                    "AGENT {}: Accumulator decay applied to {} hot clusters (factor={}).",
                    id_str, decayed_count, the_machine::ACCUMULATOR_DECAY_FACTOR
                ));
            }

            // ██ FIX v2.6 (Layer 2): Periodic entry merging ──────────
            // Every 50 ticks, merge old entries in clusters that exceed
            // the trigger count.  This prevents unbounded entry growth
            // while preserving semantic coherence.
            if ticker % 50 == 0 {
                let config = the_machine::compression::MergeConfig::default();
                let mut brain_guard = brain_subconscious.write().await;
                let mut total_removed = 0usize;
                let mut merged_clusters = 0usize;
                for cluster in &mut brain_guard.dejavu_clusters {
                    let removed = the_machine::compression::merge_entries(
                        cluster, &config, ticker as u64,
                    );
                    if removed > 0 {
                        total_removed += removed;
                        merged_clusters += 1;
                    }
                }

                // ██ Theorem XXIII.3: Cluster-level compactor ██
                // When drift exceeds δ_max, the adaptive gate lowers the
                // absorption threshold.  The compactor runs alongside it
                // with a slightly relaxed threshold (θ_adapt + 0.03) to
                // merge clusters that were spawned too eagerly during drift.
                if brain_guard.drift_magnitude_ewma > the_machine::DELTA_MAX {
                    let merge_thresh = brain_guard.adaptive_novelty_threshold() + 0.03;
                    let compactor_merges = brain_guard.compact_clusters(merge_thresh);
                    if compactor_merges > 0 {
                        total_removed += compactor_merges;
                        merged_clusters += compactor_merges;
                        let _ = subconscious_log_tx.send(format!(
                            "AGENT {}: Compactor merged {} cluster(s) (θ_merge={:.4}).",
                            id_str, compactor_merges, merge_thresh
                        ));
                    }
                }

                // ██ Association decay (UPGRADE v3.0) ██
                // Decay all association strengths by ASSOCIATION_DECAY = 0.995
                // per call (≈ 50 ticks).  Effective half-life ≈ 3.8 hours.
                // Prunes associations below ASSOCIATION_MIN_STRENGTH = 0.05.
                brain_guard.decay_associations();

                drop(brain_guard);
                if merged_clusters > 0 {
                    let _ = subconscious_log_tx.send(format!(
                        "AGENT {}: Entry merging: {} clusters merged, {} entries removed.",
                        id_str, merged_clusters, total_removed
                    ));
                }
            }

            // ██ FIX v2.5: Periodic hot/cold memory management ───────
            // Every 100 ticks, freeze cold clusters to reclaim memory.
            // Keeps at most 100 accumulators hot (40 KB each).
            if ticker % 100 == 0 {
                let mut brain_guard = brain_subconscious.write().await;
                brain_guard.freeze_cold_clusters(ticker as u64, 500, 100);
                drop(brain_guard);
                let hot_count = {
                    let bg = brain_subconscious.read().await;
                    bg.dejavu_clusters.iter().filter(|c| c.is_hot()).count()
                };
                let _ = subconscious_log_tx.send(format!(
                    "AGENT {}: Hot/cold memory sweep — {} hot clusters active.",
                    id_str, hot_count
                ));
            }

            // ██ Joint Contraction Telemetry (Theorem XXII.1-R) ██
            // Every 50 ticks, measure κ_P (projection contraction) via random
            // pairs, check the joint κ = κ_P · κ_F against the tripwire.
            // The theoretical margin is 0.010 at L_F = 1.0 (worst case).
            if ticker % 50 == 0 {
                let mut brain_guard = brain_subconscious.write().await;
                
                // Measure κ_P from random pair projections
                brain_guard.measure_kappa_p(20);
                let kp = brain_guard.contraction_telemetry.kappa_p_mean;
                let kf = brain_guard.contraction_telemetry.kappa_f_mean;
                let kj = brain_guard.contraction_telemetry.kappa_joint;
                
                // Check tripwire
                if let Some(warning) = brain_guard.contraction_telemetry
                    .check_tripwire(ticker as u64)
                {
                    let _ = subconscious_log_tx.send(format!(
                        "AGENT {}: CONTRACTION TELEMETRY — {}",
                        id_str, warning
                    ));
                }
                
                // Log periodic status
                let n_p = brain_guard.contraction_telemetry.kappa_p_count;
                let n_f = brain_guard.contraction_telemetry.kappa_f_count;
                drop(brain_guard);
                
                let _ = subconscious_log_tx.send(format!(
                    "AGENT {}: CONTRACTION TELEMETRY — κ_P={:.4} (n={}), κ_F={:.4} (n={}), κ={:.6}",
                    id_str, kp, n_p, kf, n_f, kj
                ));

                // ██ Sync cluster data to QA engine for semantic resolution ██
                // Copy centroids + associations from VSABrain to QaEngine's
                // snapshot. Both locks are held briefly (microseconds).
                {
                    let brain_read = brain_subconscious.read().await;
                    let mut qa_write = qa_for_loop.write().await;
                    qa_write.sync_cluster_data(&brain_read);
                }

                // ── LAYER 0: Induce rules from Markov transitions ─────
                // Every 50 ticks, scan the transition model for reliable
                // centroid→centroid transitions and factorize into SVO rules.
                if ticker % 50 == 0 && ticker > 0 {
                    // Acquire a fresh read lock (brain_guard was dropped above)
                    let brain_read = brain_subconscious.read().await;
                    let qa_handle = qa_for_loop.read().await;
                    let k = temporal_cog.transitions.trained_centroid_count();
                    let auto_subjects_clone = auto_subjects.clone();
                    let auto_verbs_clone = auto_verbs.clone();
                    let auto_objects_clone = auto_objects.clone();
                    let roles = the_machine::analogy::RoleDictionary::new();
                    drop(qa_handle);

                    let mut induced_count = 0usize;
                    let mut proto_updated = 0usize;

                    for i in 0..k.min(200) {
                        for j in 0..k.min(200) {
                            if i == j { continue; }
                            let p = temporal_cog.transitions.transition_probability(i, j);
                            let count = temporal_cog.transitions.counts[i][j];
                            if p >= 0.60 && count >= 20 {
                                let c_i = brain_read.get_centroid(i);
                                let c_j = brain_read.get_centroid(j);
                                if let (Some(ci), Some(cj)) = (c_i, c_j) {
                                    // Factorize centroid i (antecedent)
                                    let fact_i = the_machine::analogy::factorize_triple(
                                        ci, &roles, &resonator_vocab,
                                        &auto_subjects_clone, &auto_verbs_clone, &auto_objects_clone, 15,
                                    );
                                    // Factorize centroid j (consequent)
                                    let fact_j = the_machine::analogy::factorize_triple(
                                        cj, &roles, &resonator_vocab,
                                        &auto_subjects_clone, &auto_verbs_clone, &auto_objects_clone, 15,
                                    );

                                    match (fact_i, fact_j) {
                                        (Some((s_i, v_i, o_i, e_i)), Some((s_j, v_j, o_j, e_j))) => {
                                            if e_i >= 0.60 && e_j >= 0.60 {
                                                // Clean factorization → store as QA rule
                                                let mut qa_write = qa_for_loop.write().await;
                                                qa_write.store_rule_with_confidence(
                                                    &s_i, &v_i, &o_i,
                                                    &s_j, &v_j, &o_j,
                                                    "induced",
                                                    0.60, // starting confidence
                                                );
                                                drop(qa_write);
                                                induced_count += 1;
                                                let _ = subconscious_log_tx.send(format!(
                                                    "AGENT {}: LAYER0: Induced rule: {} {} {} → {} {} {} (E_i={:.2}, E_j={:.2}, P={:.2}, n={})",
                                                    id_str, s_i, v_i, o_i, s_j, v_j, o_j, e_i, e_j, p, count,
                                                ));
                                            } else {
                                                // Noisy → store as proto-rule for later retry
                                                let found = proto_rules.iter_mut().find(|pr|
                                                    pr.0.normalized_hamming_distance(ci) < 0.15
                                                    && pr.1.normalized_hamming_distance(cj) < 0.15
                                                );
                                                if let Some(existing) = found {
                                                    existing.2 = existing.2.saturating_add(count.min(50) as u32);
                                                    existing.3 = (existing.3 + e_i.min(e_j)) / 2.0;
                                                    existing.4 = ticker;
                                                } else if proto_rules.len() < MAX_PROTO_RULES {
                                                    proto_rules.push((
                                                        *ci, *cj,
                                                        count.min(50) as u32,
                                                        e_i.min(e_j),
                                                        ticker,
                                                    ));
                                                    proto_updated += 1;
                                                }
                                            }
                                        }
                                        _ => {
                                            // Can't factorize — store as raw proto-rule
                                            let found = proto_rules.iter_mut().find(|pr|
                                                pr.0.normalized_hamming_distance(ci) < 0.15
                                                && pr.1.normalized_hamming_distance(cj) < 0.15
                                            );
                                            if let Some(existing) = found {
                                                existing.2 = existing.2.saturating_add(count.min(50) as u32);
                                                existing.4 = ticker;
                                            } else if proto_rules.len() < MAX_PROTO_RULES {
                                                proto_rules.push((*ci, *cj, count.min(50) as u32, 0.0, ticker));
                                                proto_updated += 1;
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }

                    if induced_count > 0 || proto_updated > 0 {
                        let _ = subconscious_log_tx.send(format!(
                            "AGENT {}: LAYER0: Induced {} rule(s), {} proto-rule(s) updated ({} total proto-rules).",
                            id_str, induced_count, proto_updated, proto_rules.len(),
                        ));
                    }

                    // ── Retry proto-rules: attempt factorization again ──
                    // Re-try proto-rules that were last attempted >100 ticks ago
                    // and have accumulated more observations.
                    let mut promoted: Vec<usize> = Vec::new();
                    for (pr_idx, (pr_ant, pr_con, pr_cnt, pr_energy, pr_tick)) in proto_rules.iter_mut().enumerate() {
                        if *pr_cnt >= 30 && ticker.saturating_sub(*pr_tick) > 100 {
                            // Try factorizing again
                            let roles2 = the_machine::analogy::RoleDictionary::new();
                            let fact_i2 = the_machine::analogy::factorize_triple(
                                pr_ant, &roles2, &resonator_vocab,
                                &auto_subjects_clone, &auto_verbs_clone, &auto_objects_clone, 15,
                            );
                            let fact_j2 = the_machine::analogy::factorize_triple(
                                pr_con, &roles2, &resonator_vocab,
                                &auto_subjects_clone, &auto_verbs_clone, &auto_objects_clone, 15,
                            );
                            if let (Some((s_i2, v_i2, o_i2, e_i2)), Some((s_j2, v_j2, o_j2, e_j2))) = (fact_i2, fact_j2) {
                                if e_i2 >= 0.65 && e_j2 >= 0.65 {
                                    let mut qa_write = qa_for_loop.write().await;
                                    qa_write.store_rule_with_confidence(
                                        &s_i2, &v_i2, &o_i2,
                                        &s_j2, &v_j2, &o_j2,
                                        "induced_from_proto",
                                        0.55, // slightly lower starting confidence for delayed induction
                                    );
                                    drop(qa_write);
                                    promoted.push(pr_idx);
                                    let _ = subconscious_log_tx.send(format!(
                                        "AGENT {}: LAYER0: Proto-rule promoted: {} {} {} → {} {} {} (E={:.2}/{:.2})",
                                        id_str, s_i2, v_i2, o_i2, s_j2, v_j2, o_j2, e_i2, e_j2,
                                    ));
                                } else {
                                    *pr_energy = e_i2.min(e_j2);
                                    *pr_tick = ticker;
                                }
                            }
                        }
                    }
                    // Remove promoted proto-rules (descending order)
                    for idx in promoted.into_iter().rev() {
                        proto_rules.swap_remove(idx);
                    }

                    // ── LAYER 4: Validate rules against episode buffer ──
                    // Replay the validation buffer through the newly induced rules
                    // to measure retrospective prediction accuracy.
                    if induced_count > 0 && validation_buffer.len() >= 10 {
                        let qa_rules = qa_for_loop.read().await;
                        let new_rules_start = qa_rules.rules().len().saturating_sub(induced_count);
                        let mut correct_predictions = 0usize;
                        let mut total_predictions = 0usize;

                        // For each recently induced rule, replay buffer
                        for ru_idx in new_rules_start..qa_rules.rules().len() {
                            let rule = &qa_rules.rules()[ru_idx];
                            let mut rule_correct = 0usize;
                            let mut rule_total = 0usize;

                            // Slide window over validation buffer
                            for w in 0..validation_buffer.len().saturating_sub(1) {
                                let (state, _c_idx) = &validation_buffer[w];
                                let next_state = &validation_buffer[w + 1].0;

                                // Does the rule's antecedent match this state?
                                let ant_sim = 1.0 - state.normalized_hamming_distance(&rule.ante_hv);
                                if ant_sim >= 0.56 {
                                    rule_total += 1;
                                    // Predict consequent
                                    let predicted = rule.rule_hv.bitwise_xor(&rule.ante_hv);
                                    // Compare against actual next state
                                    let actual_sim = 1.0 - next_state.normalized_hamming_distance(&predicted);
                                    // Also check raw consequent similarity
                                    let cons_sim = 1.0 - next_state.normalized_hamming_distance(&rule.cons_hv);
                                    if actual_sim > 0.50 || cons_sim > 0.50 {
                                        rule_correct += 1;
                                    }
                                }
                            }

                            if rule_total > 0 {
                                let accuracy = rule_correct as f64 / rule_total as f64;
                                correct_predictions += rule_correct;
                                total_predictions += rule_total;

                                // Adjust confidence based on retrospective accuracy
                                if accuracy < 0.30 {
                                    let mut qa_write = qa_for_loop.write().await;
                                    qa_write.update_rule_confidence(ru_idx, 1.0 - accuracy);
                                    drop(qa_write);
                                    let _ = subconscious_log_tx.send(format!(
                                        "AGENT {}: LAYER4: Rule #{} validated — {}/{} correct ({:.0}%) — reducing confidence.",
                                        id_str, ru_idx, rule_correct, rule_total, accuracy * 100.0,
                                    ));
                                } else if accuracy > 0.70 {
                                    let mut qa_write = qa_for_loop.write().await;
                                    qa_write.update_rule_confidence(ru_idx, 0.1); // low error
                                    drop(qa_write);
                                }
                            }
                        }
                        drop(qa_rules);

                        if total_predictions > 0 {
                            let _ = subconscious_log_tx.send(format!(
                                "AGENT {}: LAYER4: Rule validation summary: {}/{} correct ({:.0}%) across {} new rules.",
                                id_str, correct_predictions, total_predictions,
                                correct_predictions as f64 / total_predictions as f64 * 100.0,
                                induced_count,
                            ));
                        }
                    }

                    // ── Cull low-confidence rules ──
                    {
                        let mut qa_write = qa_for_loop.write().await;
                        let culled = qa_write.cull_low_confidence_rules(0.20);
                        if culled > 0 {
                            let _ = subconscious_log_tx.send(format!(
                                "AGENT {}: LAYER2: Culled {} low-confidence rules (threshold=0.20).",
                                id_str, culled,
                            ));
                        }
                    }
                }
            }

            let mut telemetry = HashMap::new();
            let is_high_load_tick = ticker % 20 > 15;
            let cpu = if is_high_load_tick {
                70.0 + (ticker % 3) as f64 * 8.0
            } else {
                15.0 + (ticker % 5) as f64 * 5.0
            };
            let ram = if is_high_load_tick { 8.0 } else { 42.0 };
            let latency = if is_high_load_tick { 350.0 } else { 45.0 };

            telemetry.insert("cpu_utilization".to_string(), cpu);
            telemetry.insert("ram_free_gb".to_string(), ram);
            telemetry.insert(
                "throughput".to_string(),
                if is_high_load_tick { 80.0 } else { 25.0 },
            );
            telemetry.insert("error_rate".to_string(),
                if is_high_load_tick { 0.08 } else { 0.005 },
            );
            telemetry.insert("response_latency".to_string(), latency);

            {
                let mut metrics_guard = metrics_clone.write().await;
                *metrics_guard = telemetry.clone();
            }

            let mut brain_guard = brain_subconscious.read().await;
            let external_state = brain_guard.compile_state_vector(&telemetry);

            let curr_url = current_url_forager.read().await;
            let signal_headline = curr_url.split('/').last().unwrap_or("Index");
            let signal_state = Hypervector::encode_text_ngram(signal_headline, 3);

            let ping_status = if is_high_load_tick {
                "DEGRADED"
            } else {
                "NOMINAL"
            };
            let internal_state = Hypervector::encode_text_ngram(ping_status, 3);

            let bound_external = external_state.bitwise_xor(&v_role_external_state);
            let bound_signal = signal_state.bitwise_xor(&v_role_signal_state);
            let bound_internal = internal_state.bitwise_xor(&v_role_internal_state);

            let current_world_state =
                Hypervector::bundle(&[&bound_external, &bound_signal, &bound_internal]);

            if let (Some(p_s), Some(p_n), Some(p_v)) = (pred_stable, pred_nominal, pred_volatile) {
                let err_s = current_world_state.normalized_hamming_distance(&p_s);
                let err_n = current_world_state.normalized_hamming_distance(&p_n);
                let err_v = current_world_state.normalized_hamming_distance(&p_v);

                stable_error = stable_error * 0.8 + err_s * 0.2;
                nominal_error = nominal_error * 0.8 + err_n * 0.2;
                volatile_error = volatile_error * 0.8 + err_v * 0.2;
            }

            {
                let mut ws_guard = world_state_subconscious.write().await;
                *ws_guard = current_world_state;
            }

            // ── LAYER 0+4: Temporal observe + validation buffer ──
            {
                let c_idx = brain_guard.nearest_centroid_idx(&current_world_state);
                if let Some((cidx, _sim)) = c_idx {
                    // Layer 0: Record transition in Markov model
                    temporal_cog.observe(&current_world_state, cidx, None, 0.5);

                    // Layer 4: Push to validation buffer (ring buffer, 100 slots)
                    if validation_buffer.len() < 100 {
                        validation_buffer.push((current_world_state, cidx));
                    } else {
                        validation_buffer[validation_write_pos % 100] = (current_world_state, cidx);
                    }
                    validation_write_pos += 1;
                }
            }

            // ██ UPGRADE v2.3: DeepThought reasoning cycle ██
            // Every 10 ticks, run the anchored reason cycle and route the
            // attended intent back into the action pipeline.
            //
            // Moved AFTER historical_baseline and crisis_concepts so the
            // reasoner can evaluate desirability against current context.

            let _resolved_concept = {
                let (label, _) = brain_guard.evaluate_deja_vu(&current_world_state);
                if let Some(ref lbl) = label {
                    if lbl.contains("HighLoad") || lbl.contains("high_load") || lbl.contains("crisis") {
                        c_high_load
                    } else {
                        c_normal
                    }
                } else {
                    c_normal
                }
            };

            let historical_baseline = if let Some(last_cluster) = brain_guard.dejavu_clusters.last()
            {
                last_cluster.centroid
            } else {
                c_normal
            };

            // ── Regime-adaptive drift tracking (EWMA + variance) ─────
            let deltas_vec: Vec<Hypervector> = recent_deltas.iter().cloned().collect();
            let drift_var = if deltas_vec.len() >= 2 {
                the_machine::planning::drift_variance(&deltas_vec)
            } else {
                0.0
            };
            active_drift = the_machine::planning::bundle_weighted_ewma(&deltas_vec, 3);
            let regime_volatility = (drift_var / 0.5).min(1.0);

            // Build a drift sequence for the planning layer
            let mut drift_seq: Vec<Hypervector> = Vec::with_capacity(2);
            for i in 0..2 {
                drift_seq.push(
                    deltas_vec.get(deltas_vec.len().saturating_sub(2).wrapping_add(i))
                        .copied()
                        .unwrap_or(active_drift)
                );
            }

            // Load experiences for planning cost penalties
            let exps = brain_guard.experiences.clone();

            let drive = AutonomyDrive::new(0.44);
            let dissonance =
                AutonomyDrive::calculate_dissonance(&current_world_state, &historical_baseline);
            let mut should_pivot = drive.evaluates_necessity_to_pivot(&dissonance);

            // ██ Intent Momentum Gate ██
            // If DeepThought updated the intent within the last INTENT_MOMENTUM_TICKS,
            // suppress Tier 1 pivoting UNLESS the basal threat level is critical.
            //
            //   Allow Pivot = (ticks_since_dt >= MOMENTUM_TICKS) ∨ (τ > θ_critical)
            //
            // This gives the deliberative reasoner temporal space to have its
            // intent executed, while preserving survival reflexes under attack.
            const INTENT_MOMENTUM_TICKS: usize = 5;
            const CRITICAL_THREAT: f64 = 0.85;
            if should_pivot && ticks_since_dt < INTENT_MOMENTUM_TICKS {
                let threat = *defense_subconscious.threat_level.read().await;
                if threat < CRITICAL_THREAT {
                    should_pivot = false;
                    let _ = subconscious_log_tx.send(format!(
                        "AGENT {}: Intent Momentum holding ({} ticks remain, threat={:.2}).",
                        id_str,
                        INTENT_MOMENTUM_TICKS - ticks_since_dt,
                        threat,
                    ));
                }
            }

            let stress_memory = brain_guard
                .dejavu_clusters
                .first()
                .map(|c| c.centroid)
                .unwrap_or(c_high_load);
            let stress_sim = 1.0 - current_world_state.normalized_hamming_distance(&stress_memory);

            // ── Inject learned stress clusters into planning ─────────
            // Build a combined stress_concepts slice (undesirable states)
            let mut crisis_concepts = vec![c_high_load];
            crisis_concepts.extend(brain_guard.collect_learned_crisis_concepts());

            // ██ UPGRADE v2.3: DeepThought reasoning cycle (Tier 2) ██
            // Every 10 ticks, run anchored chaining and evaluate desirability.
            // Uses cluster-anchored forward chaining for noise-immune 5-hop
            // composition, and dissonance-gradient desirability with crisis
            // override.  The resulting intent overrides Tier 1 via Intent
            // Momentum.
            if ticker % 10 == 0 {
                let clusters_snapshot = brain_guard.dejavu_clusters.clone();

                let (reasoned_intent, best_slot, trace, desirable) = dt.reason(
                    &current_world_state,
                    &auto_subjects,
                    &auto_verbs,
                    &auto_objects,
                    &clusters_snapshot,
                    &historical_baseline,
                    &crisis_concepts,
                ).await;

                // Log the reasoning trace
                for t in &trace {
                    let _ = subconscious_log_tx.send(format!(
                        "AGENT {}: DEEPTHOUGHT {}", id_str, t
                    ));
                }

                if desirable && reasoned_intent.count_ones() > 0 {
                    // ██ Reset Intent Momentum — Tier 1 suppressed for 5 ticks ██
                    ticks_since_dt = 0;

                    let mut intent_guard = intent_subconscious.write().await;
                    *intent_guard = reasoned_intent;
                    let mut drive_guard = active_drive_subconscious.write().await;
                    *drive_guard = format!("DeepThought (slot {}, desirable)", best_slot);

                    // Slot-gated dispatch: decode the winning slot's intent.
                    // (The voice that won the attention competition gets to
                    //  speak — its chain won because it was both relevant
                    //  to the current state and deep enough to carry weight.)
                    if let Some((_name, param_hv)) =
                        action_registry.decode_intent(&reasoned_intent, &resonator_vocab)
                    {
                        *forager_target.write().await = Some(param_hv);
                    }

                    let _ = subconscious_log_tx.send(format!(
                        "AGENT {}: DEEPTHOUGHT intent (slot {}) dispatched to action pipeline.",
                        id_str, best_slot
                    ));
                } else if !desirable && reasoned_intent.count_ones() > 0 {
                    // Chain was undesirable — still reset Intent Momentum
                    // (Cognitive Stillness: hold position, don't pivot)
                    ticks_since_dt = 0;
                    let _ = subconscious_log_tx.send(format!(
                        "AGENT {}: DeepThought chain rejected (undesirable). Holding stillness.",
                        id_str
                    ));
                }
            }

            if should_pivot {
                let mut drive_guard = active_drive_subconscious.write().await;
                let mut intent_guard = intent_subconscious.write().await;

                // Formulate corrective intent via planning layer
                let chosen_intent = if let Some((corrective_intent, label)) = drive.formulate_intent(
                    &dissonance, &resonator_vocab, &action_registry,
                    &auto_subjects, &auto_verbs, &auto_objects, 30,
                    &current_world_state, &c_normal, &drift_seq,
                    &crisis_concepts, regime_volatility, &exps,
                ) {
                    *drive_guard = label;
                    *intent_guard = corrective_intent;
                    corrective_intent
                } else {
                    *drive_guard =
                        "Subconscious (Dissonance Pivot — fallback)".to_string();
                    *intent_guard = dissonance;
                    dissonance
                };

                // Update the forager's semantic target parameter
                if let Some((_name, param_hv)) =
                    action_registry.decode_intent(&chosen_intent, &resonator_vocab)
                {
                    *forager_target.write().await = Some(param_hv);
                }
            } else if stress_sim > 0.55 {
                let mut drive_guard = active_drive_subconscious.write().await;
                let mut intent_guard = intent_subconscious.write().await;

                // Phantom pain: try parsing the offset from crisis memory
                let phantom = current_world_state.bitwise_xor(&stress_memory);
                let chosen_intent = if let Some((corrective_intent, label)) = drive.formulate_intent(
                    &phantom, &resonator_vocab, &action_registry,
                    &auto_subjects, &auto_verbs, &auto_objects, 30,
                    &current_world_state, &c_normal, &drift_seq,
                    &crisis_concepts, regime_volatility, &exps,
                ) {
                    *drive_guard = label;
                    *intent_guard = corrective_intent;
                    corrective_intent
                } else {
                    *drive_guard =
                        "Subconscious (Phantom Pain — fallback)".to_string();
                    *intent_guard = phantom;
                    phantom
                };

                // Update the forager's semantic target parameter
                if let Some((_name, param_hv)) =
                    action_registry.decode_intent(&chosen_intent, &resonator_vocab)
                {
                    *forager_target.write().await = Some(param_hv);
                }
            } else {
                let mut drive_guard = active_drive_subconscious.write().await;
                *drive_guard = "Autonomous / Idle Search".to_string();
            }

            // Dynamic threat forecasting and planning
            if ticker % 15 == 0 {
                let _ = subconscious_log_tx.send(format!(
                    "AGENT {}: Simulating threat trajectory using regime-adaptive environmental drift (variance={:.3}).",
                    id_str, drift_var
                ));
                let forecast = the_machine::planning::build_drift_forecast(
                    &deltas_vec,
                    drift_var,
                    5,
                    3,
                    stable_error,
                    nominal_error,
                    volatile_error,
                );
                let threat_horizon = the_machine::planning::simulate_threat_trajectory(
                    &current_world_state,
                    &forecast,
                    &crisis_concepts,
                    0.80,
                );

                if let Some(expected_steps) = threat_horizon {
                    let _ = subconscious_log_tx.send(format!(
                        "AGENT {}: FORECAST ALERT! High threat state (Crisis) predicted in {:.1} steps (BMA). Generating dynamic corrective intent.",
                        id_str, expected_steps
                    ));

                    // ── Causal rule chaining ─────────────────────────
                    // Try to decompose the drift → crisis trajectory into
                    // a causal rule via recursive SVO factorization.
                    // If the drift pattern encodes as "IF_RULE drift_verb
                    // (subject THEN consequence)", store the rule so
                    // future drift forecasts can recognise the precrisis
                    // pattern earlier.
                    if !deltas_vec.is_empty() {
                        let bundle_refs: Vec<&Hypervector> = deltas_vec.iter().collect();
                        let drift_pattern = Hypervector::bundle(&bundle_refs);
                        let causal_subjects: Vec<String> = vec![
                            "IF_RULE".to_string(), "CAUSE_RULE".to_string(),
                        ];
                        let causal_verbs: Vec<String> = vec![
                            "Breach".to_string(), "Crisis".to_string(), "Attack".to_string(),
                        ];
                        let causal_objects: Vec<String> = vec![
                            "consequence".to_string(), "crisis".to_string(),
                        ];
                        if let Some((rule_s, rule_v, rule_slot)) =
                            the_machine::resonator::factorize_recursive(
                                &drift_pattern,
                                &resonator_vocab,
                                &causal_subjects,
                                &causal_verbs,
                                &causal_objects,
                                10,
                            )
                        {
                            let _ = subconscious_log_tx.send(format!(
                                "AGENT {}: Causal rule detected — {} {} {:?}. Storing for drift forecasting.",
                                id_str, rule_s, rule_v, rule_slot
                            ));
                            // Drop the read guard and acquire a write guard
                            // to store the rule as a transient fact.
                            drop(brain_guard);
                            let mut brain_write = brain_subconscious.write().await;
                            let rule_vec = drift_pattern;
                            let mut rule_meta = std::collections::HashMap::new();
                            rule_meta.insert("type".to_string(), "causal_rule".to_string());
                            rule_meta.insert("subject".to_string(), rule_s.clone());
                            rule_meta.insert("verb".to_string(), rule_v.clone());
                            brain_write.add_transient_fact(
                                rule_vec,
                                &format!("IF_{}_THEN_RISK", rule_v),
                                rule_meta,
                            );
                            drop(brain_write);
                            brain_guard = brain_subconscious.read().await;
                        }
                    }

                    // drift_seq, regime_volatility, and exps are already
                    // computed earlier in the loop for the autonomy section.

                    if let Some(trajectory) = the_machine::planning::find_optimal_trajectory(
                        &current_world_state,
                        &c_normal,
                        &drift_seq,
                        &action_registry,
                        &resonator_vocab,
                        2,
                        &crisis_concepts,
                        regime_volatility,
                        &exps,
                    ) {
                        let _ = subconscious_log_tx.send(format!(
                            "AGENT {}: Corrective plan formulated. Steps: {}, Cost: {:.2}",
                            id_str,
                            trajectory.steps.len(),
                            trajectory.cumulative_cost
                        ));

                        for (idx, step) in trajectory.steps.iter().enumerate() {
                            let _ = subconscious_log_tx.send(format!(
                                "AGENT {}: Executing corrective step {}/{} -> Action: {}, Parameter: {}",
                                id_str, idx + 1, trajectory.steps.len(), step.action, step.parameter
                            ));

                            let step_param_hv =
                                resonator_vocab.get_vector(&step.parameter).unwrap();

                            // ── Defense energy gate ─────────────────
                            // Before dispatching, verify the action is
                            // safe given current threat + anxiety levels.
                            let energy_gate = defense_subconscious
                                .check_action_safety(&step.action, &step.parameter)
                                .await;

                            let exec_res = match energy_gate {
                                Ok(()) => the_machine::action::execute_action(
                                    &step.action,
                                    step_param_hv,
                                    &resonator_vocab,
                                ),
                                Err(gate_reason) => {
                                    let _ = subconscious_log_tx.send(format!(
                                        "AGENT {}: Energy gate REJECTED {} {} — {}",
                                        id_str, step.action, step.parameter, gate_reason
                                    ));
                                    Err(gate_reason)
                                }
                            };

                            let v_outcome = Hypervector::encode_text_ngram(
                                if exec_res.is_ok() { "SUCCESS" } else { "FAILURE" },
                                3
                            );
                            if let Some(act_hv) = action_registry.get_action_vector(&step.action) {
                                let experience_hv = act_hv
                                    .bitwise_xor(step_param_hv)
                                    .bitwise_xor(&current_world_state)
                                    .bitwise_xor(&v_outcome);
                                {
                                    let mut brain_write = brain_subconscious.write().await;
                                    brain_write.push_experience(experience_hv);
                                }
                            }

                            match exec_res {
                                Ok(stdout) => {
                                    let _ = subconscious_log_tx.send(format!(
                                        "AGENT {}: Corrective step {} success. Result: {}",
                                        id_str,
                                        idx + 1,
                                        if stdout.is_empty() { "ok" } else { &stdout }
                                    ));
                                    if let Some(act_hv) =
                                        action_registry.get_action_vector(&step.action)
                                    {
                                        let step_vector = act_hv.bitwise_xor(step_param_hv);
                                        current_tick_actions.push(step_vector);
                                    }
                                }
                                Err(e) => {
                                    let _ = subconscious_log_tx.send(format!(
                                        "AGENT {}: Corrective step {} failed: {}",
                                        id_str,
                                        idx + 1,
                                        e
                                    ));
                                }
                            }
                        }
                    } else {
                        let _ = subconscious_log_tx.send(format!(
                            "AGENT {}: Pathfinder failed to resolve a corrective plan to stabilize system.",
                            id_str
                        ));
                    }
                }
            }

            // Estimate external drift Delta S and update history buffers
            if let Some(prev_state) = recent_states.back() {
                let last_action = recent_actions
                    .back()
                    .copied()
                    .unwrap_or_else(Hypervector::new_zero);
                // \delta_t = S_t \oplus \rho(S_{t-1}) \oplus A_{t-1}
                let delta_t = current_world_state
                    .bitwise_xor(&prev_state.rotate_left(13))
                    .bitwise_xor(&last_action);
                // ██ Theorem XXIII.4: Update drift magnitude EWMA on the brain ██
                {
                    let mut brain_write = brain_subconscious.write().await;
                    brain_write.update_drift_magnitude(&delta_t);
                }
                recent_deltas.push_back(delta_t);
                if recent_deltas.len() > history_limit {
                    recent_deltas.pop_front();
                }
            }

            recent_states.push_back(current_world_state);
            if recent_states.len() > history_limit {
                recent_states.pop_front();
            }

            let accumulated_action = if current_tick_actions.is_empty() {
                Hypervector::new_zero()
            } else {
                let mut acc = current_tick_actions[0];
                for act in current_tick_actions.iter().skip(1) {
                    acc = acc.bitwise_xor(act);
                }
                acc
            };
            recent_actions.push_back(accumulated_action);
            if recent_actions.len() > history_limit {
                recent_actions.pop_front();
            }

            let current_deltas_vec: Vec<Hypervector> = recent_deltas.iter().cloned().collect();
            let nominal_drift = the_machine::planning::bundle_weighted_ewma(&current_deltas_vec, 3);
            let mut reversed = current_deltas_vec.clone();
            reversed.reverse();
            let stable_drift = the_machine::planning::bundle_weighted_ewma(&reversed, 3);
            let newest_delta = current_deltas_vec.last().copied().unwrap_or(nominal_drift);
            let amp_refs: Vec<&Hypervector> =
                std::iter::repeat(&newest_delta).take(5).chain(std::iter::once(&nominal_drift)).collect();
            let volatile_drift = Hypervector::bundle(&amp_refs);

            pred_stable = Some(current_world_state.rotate_left(13).bitwise_xor(&accumulated_action).bitwise_xor(&stable_drift));
            pred_nominal = Some(current_world_state.rotate_left(13).bitwise_xor(&accumulated_action).bitwise_xor(&nominal_drift));
            pred_volatile = Some(current_world_state.rotate_left(13).bitwise_xor(&accumulated_action).bitwise_xor(&volatile_drift));

            // ── Experience feedback loop ─────────────────────────────
            // Every 50 ticks, cluster experiences to update crisis concepts.
            // If a cluster of FAILURE outcomes emerges, its centroid can
            // serve as a learned crisis marker for future planning.
            if ticker % 50 == 0 && ticker > 0 {
                let exps = {
                    let brain_read = brain_subconscious.read().await;
                    brain_read.experiences.clone()
                };
                if exps.len() >= 5 {
                    let v_failure = Hypervector::encode_text_ngram("FAILURE", 3);
                    // Find experiences most similar to FAILURE
                    let mut failure_states: Vec<Hypervector> = Vec::new();
                    for exp in &exps {
                        // The experience is: action ⊕ param ⊕ state ⊕ outcome
                        // To extract state: exp ⊕ action ⊕ param ⊕ outcome
                        // But we stored action ⊕ param ⊕ state ⊕ outcome.
                        // state ≈ exp (since action⊕param⊕outcome forms a
                        // background that cancels in similarity comparison)
                        // For simplicity, just cluster on raw experiences.
                        let sim = 1.0 - exp.normalized_hamming_distance(&v_failure);
                        if sim > 0.6 {
                            failure_states.push(*exp);
                        }
                    }
                    if failure_states.len() >= 3 {
                        let refs: Vec<&Hypervector> = failure_states.iter().collect();
                        let learned_crisis = Hypervector::bundle(&refs);
                        let _ = subconscious_log_tx.send(format!(
                            "AGENT {}: Experience feedback — clustered {} failure patterns. Updating crisis model.",
                            id_str, failure_states.len()
                        ));
                        // Register the learned crisis centroid as a permanent
                        // memory cluster so it directly influences future
                        // planning costs (crisis-proximate actions get
                        // dynamically penalised).
                        drop(brain_guard);
                        let mut brain_write = brain_subconscious.write().await;
                        let mut meta = std::collections::HashMap::new();
                        meta.insert("source".to_string(), "experience_feedback".to_string());
                        meta.insert("type".to_string(), "learned_crisis_pattern".to_string());
                        brain_write.add_to_dejavu_db(
                            learned_crisis,
                            &format!("FAILURE_CLUSTER_{}", ticker),
                            meta,
                        );
                        drop(brain_write);
                        brain_guard = brain_subconscious.read().await;
                    }
                }
            }

            // ── LAYERS 3-5 INTEGRATION: Close the loop ──────────────────
            // Step 2: Every 5 ticks, run abductive rule discovery on frames
            if ticker % 5 == 0 {
                let pri_guard = primary_int.read().await;
                let mut met_guard = meta_int.write().await;

                met_guard.abductor.process_frames(&*pri_guard, 2);
                met_guard.abductor.check_refutations(&*pri_guard);

                let n_frames = pri_guard.frame_count();
                let rules_len = met_guard.abductor.trustworthy_rules().len();
                if rules_len > 0 {
                    let _ = subconscious_log_tx.send(format!(
                        "ABDUCTOR: {} rules from {} frames",
                        rules_len, n_frames,
                    ));
                }

                // Step 3: Every 10 ticks, materialize analogical predictions → VSABrain
                if ticker % 10 == 0 {
                    // Get top predictions by plausibility
                    let predictions: Vec<_> = pri_guard.predictions_sorted().into_iter()
                        .take(3)
                        .map(|p| p.predicted_vector)
                        .collect();

                    for (i, pred_hv) in predictions.iter().enumerate() {
                        let mut brain_write = brain_subconscious.write().await;
                        let mut meta_map = std::collections::HashMap::new();
                        meta_map.insert("source".to_string(), "analogical_prediction".to_string());
                        brain_write.add_to_dejavu_db(
                            *pred_hv,
                            &format!("analogical_pred_t{}_g{}", ticker, i),
                            meta_map,
                        );
                        drop(brain_write);
                        let _ = subconscious_log_tx.send(format!(
                            "GATE: Materialized analogical prediction #{} (tick {})",
                            i, ticker
                        ));
                    }
                }
                drop(met_guard);
                drop(pri_guard);
            }

            // Step 4: Route curiosity targets to forager's target_parameter
            // If the MetaIndex detects causal curiosity gaps, route the
            // highest-priority target into the forager's semantic intent.
            {
                let pri_guard = primary_int.read().await;
                let met_guard = meta_int.read().await;

                // Use weighted abduced targets (returns Hypervector targets)
                let abduced = met_guard.curiosity_targets_abduced_weighted(
                    pri_guard.frames(), &met_guard.signature_stats,
                );

                let target_hv = abduced.first().map(|(hv, _, _)| *hv);

                if let Some(hv) = target_hv {
                    let mut tg = forager_target.write().await;
                    *tg = Some(hv);
                    let _ = subconscious_log_tx.send(format!(
                        "CURIOSITY: Routing target to forager (abduced, weight={:.2})",
                        abduced[0].2,
                    ));

                    // ── Factorize target → DuckDuckGo search URL ──
                    // If the target hypervector factorizes cleanly into
                    // (subject, verb, object), build a search URL and push
                    // it into the forager's seed queue.  This closes the
                    // vocabulary-to-URL gap: the system can now ACT on
                    // what it's curious about.
                    let roles = the_machine::analogy::RoleDictionary::new();
                    let (s_cands, v_cands, o_cands) = (
                        auto_subjects.clone(),
                        auto_verbs.clone(),
                        auto_objects.clone(),
                    );
                    if let Some((subj, verb, obj, energy)) =
                        the_machine::analogy::factorize_triple(
                            &hv, &roles, &resonator_vocab,
                            &s_cands, &v_cands, &o_cands, 20,
                        )
                    {
                        if energy > 0.55 {
                            let query = format!("{} {} {}", subj, verb, obj);
                            // Simple URL encoding: spaces → +, strip punctuation
                            let encoded: String = query.chars()
                                .map(|c| if c.is_whitespace() { '+' }
                                     else if c.is_ascii_punctuation() { ' ' }
                                     else { c })
                                .collect();
                            let encoded = encoded.split_whitespace()
                                .collect::<Vec<_>>().join("+");
                            let search_url = format!(
                                "https://html.duckduckgo.com/html/?q={}",
                                encoded
                            );
                            let mut seeds = seed_urls_int.write().await;
                            seeds.push_back(search_url);
                            let _ = subconscious_log_tx.send(format!(
                                "CURIOSITY: Generated DuckDuckGo search for '{}' (E={:.2})",
                                query, energy,
                            ));
                        }
                    }
                }
                drop(met_guard);
                drop(pri_guard);

                // Periodic delta cache pressure relief every 25 ticks.
                if ticker % 25 == 0 {
                    primary_int.write().await.delta_cache_slim();
                }
            }

            // ── GLOBAL WORKSPACE: Feed module states ─────────────────────
            {
                let profile = HomeostaticProfile::from_homeostasis(&homeostasis);
                let global_error = (stable_error + nominal_error + volatile_error) / 3.0;
                let error_hv = Hypervector::encode_text_ngram(
                    &format!("BLENDED_ERR_{}", (global_error * 10.0).round() as usize), 3);

                workspace.update_module(0, profile.encode());                         // HOMEOSTASIS
                workspace.update_module(1, error_hv);                                 // PREDICTIVE
                workspace.update_module(2, Hypervector::encode_text_ngram(            // FORAGER
                    &curr_url.split('/').last().unwrap_or("idle"), 3));
                workspace.update_module(3, historical_baseline);                      // MEMORY
                workspace.update_module(4, *current_mode.to_hypervector());           // MODE

                // ── EMOTIONAL FIELD: Derive mood from brain state ──────────────
                // Map brain signals to (emotion, stance) → mood
                current_emotion = {
                    let anxiety = brain_guard.anxiety;
                    let coherence = 1.0 - anxiety;
                    let th = *defense_subconscious.threat_level.read().await;
                    if anxiety > 0.7 { Emotion::Fear }
                    else if th > 0.5 { Emotion::Fear }
                    else if coherence < 0.3 { Emotion::Sadness }
                    else if psc_predictor.chaos_score() > 0.5 { Emotion::Surprise }
                    else if coherence > 0.8 { Emotion::Joy }
                    else { Emotion::Neutral }
                };
                current_stance = {
                    let mode_bits = current_mode.bits();
                    match current_mode {
                        the_machine::drift::CognitiveMode::Explorer
                        | the_machine::drift::CognitiveMode::Frontier => Stance::Curious,
                        the_machine::drift::CognitiveMode::Regulated => Stance::Guarded,
                        _ => {
                            // [memory, regulation, novelty]
                            if mode_bits.2 { Stance::Curious }      // novelty bit set
                            else if mode_bits.1 { Stance::Guarded } // regulation bit set
                            else { Stance::Open }
                        }
                    }
                };
                current_mood = emotional_field.resolve(current_emotion, current_stance);
                let mood_hv = Hypervector::encode_text_ngram(
                    &format!("MOOD_{:?}", current_mood), 3);
                workspace.update_module(5, mood_hv);                                // EMOTION
            }

            // ── SELF-MODEL: Integrate all module states into unified identity ──
            {
                let profile = HomeostaticProfile::from_homeostasis(&homeostasis);
                let l2_focus = historical_baseline;
                let global_error = (stable_error + nominal_error + volatile_error) / 3.0;
                self_model.tick(global_error, profile, current_mode, l2_focus);
            }

            // ── WORKSPACE: Evaluate attention with Self_t as query ─────
            let attention_report = workspace.evaluate_attention(&self_model.current_identity);
            if ticker % 25 == 0 {
                let _ = subconscious_log_tx.send(format!(
                    "AGENT {}: {} | winner={} (sim={:.3})",
                    id_str, workspace.report(),
                    attention_report.winner_label, attention_report.winner_similarity,
                ));

                let stability = self_model.identity_stability();
                if stability > 0.20 {
                    let _ = subconscious_log_tx.send(format!(
                        "AGENT {}: ⚠ COGNITIVE SHOCK — identity stability={:.4}",
                        id_str, stability
                    ));
                }
            }

            // ── EMOTIONAL FIELD: Log mood state ───────────────────────
            if ticker % 25 == 0 {
                let _ = subconscious_log_tx.send(format!(
                    "AGENT {}: AFFECT: {:?}+{:?}→{:?} | anxiety={:.2} threat={:.2}",
                    id_str, current_emotion, current_stance, current_mood,
                    brain_guard.anxiety,
                    *defense_subconscious.threat_level.read().await,
                ));
            }

            // ── INTRINSIC MOTIVATION: Update drives from system state ──
            {
                let l2_count = brain_guard.dejavu_clusters.len();
                let identity_stability = self_model.identity_stability();
                drives.update(
                    self_model.global_error,
                    self_model.global_error.min(0.05), // approximate min_error
                    self_model.homeostasis.overall_deficit,
                    identity_stability,
                    l2_count,
                );
            }

            // ── IMPLICIT INTUITION: Learn and recognize patterns ──────
            {
                // Observe current state pattern
                let mode_tag = format!("MODE_{:?}", current_mode);
                let mood_tag = format!("MOOD_{:?}", current_mood);
                let emo_tag = format!("EMOTION_{:?}", current_emotion);
                let anxiety_tag = if brain_guard.anxiety > 0.5 { "HIGH_ANXIETY" } else { "LOW_ANXIETY" };
                let domain_tags = [
                    mode_tag.as_str(),
                    mood_tag.as_str(),
                    emo_tag.as_str(),
                    anxiety_tag,
                ];
                intuition_engine.observe("current_state", &domain_tags);
                if ticker > 0 && ticker % 10 == 0 {
                    // Periodically check for recognized patterns
                    let probe = Hypervector::encode_text_ngram(
                        &format!("STATE_{}_{:?}", ticker % 5, current_mood), 3);
                    let matches = intuition_engine.recognize(&probe);
                    if !matches.is_empty() {
                        let (pattern, sim) = matches[0];
                        let pat_hv = Hypervector::encode_text_ngram(
                            &format!("INTUIT_{}", pattern.label), 3);
                        workspace.update_module(6, pat_hv);  // INTUITION
                        if ticker % 50 == 0 {
                            let _ = subconscious_log_tx.send(format!(
                                "AGENT {}: INTUITION: recognized '{}' (sim={:.3}, strength={})",
                                id_str, pattern.label, sim, pattern.strength,
                            ));
                        }
                    }
                }
                // Prune weak patterns periodically
                if ticker > 0 && ticker % 100 == 0 {
                    intuition_engine.prune(2);
                }
            }

            // ── SHADOW SYSTEM: Archetype oscillation & enantiodromia ───
            {
                let anxiety = brain_guard.anxiety;
                let coherence = 1.0 - anxiety;
                let th = *defense_subconscious.threat_level.read().await;
                // Feed external signals based on brain state
                let hero_signal = if coherence > 0.7 && th < 0.3 { 0.2 } else { 0.05 };
                let shadow_signal = if th > 0.6 { 0.25 } else if anxiety > 0.6 { 0.15 } else { 0.05 };
                let sage_signal = if current_mood == Mood::Analytical { 0.2 } else { 0.05 };
                let trickster_signal = if current_mood == Mood::Playful { 0.2 } else { 0.05 };
                let caregiver_signal = if current_mood == Mood::Warm { 0.15 } else { 0.05 };
                let orphan_signal = if current_mood == Mood::Withdrawn { 0.2 } else { 0.05 };

                shadow_system.tick(&[
                    (Archetype::Hero, hero_signal),
                    (Archetype::Shadow, shadow_signal),
                    (Archetype::Sage, sage_signal),
                    (Archetype::Trickster, trickster_signal),
                    (Archetype::Caregiver, caregiver_signal),
                    (Archetype::Orphan, orphan_signal),
                ]);

                // Update workspace with dominant archetype
                let dominant_arch = shadow_system.dominant();
                let arch_hv = Hypervector::encode_text_ngram(
                    &format!("ARCH_{:?}", dominant_arch), 3);
                workspace.update_module(7, arch_hv);  // SHADOW

                if ticker % 50 == 0 {
                    let ints: Vec<f64> = shadow_system.archetypes.iter().map(|a| a.intensity).collect();
                    let int_str: Vec<String> = ints.iter().map(|v| format!("{:.2}", v)).collect();
                    let _ = subconscious_log_tx.send(format!(
                        "AGENT {}: SHADOW: dominant={:?} | intensities: [{}]",
                        id_str, dominant_arch, int_str.join(", "),
                    ));
                }
            }

            // ── NARRATIVE GENERATOR: Full state-aware narrative ──────────
            if ticker % 25 == 0 {
                let dominant_arch_val = shadow_system.dominant();
                // Build the full SystemState for rich narrative generation
                let narrative = {
                    use the_machine::narrative::{NarrativeGenerator, SystemState};
                    let generator = NarrativeGenerator::new();
                    let state = SystemState {
                        self_model: &self_model,
                        attention: &attention_report,
                        workspace: &workspace,
                        drives: &drives,
                        dominant_archetype: Some(dominant_arch_val),
                        emotion: Some(current_emotion),
                        stance: Some(current_stance),
                        mood: Some(current_mood),
                        sleep_narrative: None,
                        sleep_transitions: 0,
                        sleep_l3_formed: 0,
                        is_first_tick: ticker == 0,
                        tick: ticker as u64,
                        is_sleeping: sleeper.sleeping,
                        sleep_reason: None,
                    };
                    generator.generate(&state)
                };
                let _ = subconscious_log_tx.send(format!(
                    "AGENT {}: 📖 {}", id_str, narrative
                ));

                // Add n-gram chain prediction
                let current_mode_label = current_mode.label().to_lowercase();
                if let Some(prediction) = ngram_chain.predict(&current_mode_label) {
                    let _ = subconscious_log_tx.send(format!(
                        "AGENT {}: 🔮 I predict I will transition to {} next.",
                        id_str, prediction
                    ));
                }
            }

            // ── CONTEXT ENGINE: Fork/merge global context ─────────────
            if ticker > 0 && ticker % 25 == 0 {
                let cue = self_model.current_identity;
                // Bind current state into global context
                let role_self = Hypervector::encode_text_ngram("ROLE_SELF_STATE", 3);
                let role_mood = Hypervector::encode_text_ngram("ROLE_MOOD", 3);
                let role_mode = Hypervector::encode_text_ngram("ROLE_COG_MODE", 3);
                let mood_hv = Hypervector::encode_text_ngram(
                    &format!("MOOD_{:?}", current_mood), 3);
                let mode_hv = *current_mode.to_hypervector();
                global_context.bind(&role_self, &self_model.current_identity);
                global_context.bind(&role_mood, &mood_hv);
                global_context.bind(&role_mode, &mode_hv);

                // Fork hypotheses and evaluate
                let branches = fork_context(&global_context, 3);
                if let Some(best) = the_machine::drift::merge_contexts(&branches, &cue) {
                    global_context = best;
                }
            }

            // ── DCP CONSENSUS: Propose → vote → resolve on current state ──
            {
                // Every 20 ticks: propose the current cognitive state
                if ticker > 0 && ticker % 20 == 0 {
                    // Build a proposal HV from the agent's current state
                    let mood_tag = format!("MOOD_{:?}", current_mood);
                    let arch_tag = format!("ARCH_{:?}", shadow_system.dominant());
                    let mode_tag = format!("MODE_{:?}", current_mode);
                    let proposal_hv = Hypervector::bundle(&[
                        &Hypervector::encode_text_ngram(&mood_tag, 3),
                        &Hypervector::encode_text_ngram(&arch_tag, 3),
                        &Hypervector::encode_text_ngram(&mode_tag, 3),
                    ]);

                    let msg = DcpMessage::new(
                        format!("agent_{}", role_str),
                        DcpRole::Primary,
                        proposal_hv,
                        0.9,                    // priority
                        ticker as u64,           // message_id
                        ticker as u64,           // timestamp/tick
                    );
                    let tid = dcp_engine.propose(msg, ticker as u64);

                    // Self-vote as Critic and Backup (simulated multi-agent)
                    dcp_engine.vote(tid, "critic_self", DcpRole::Critic, proposal_hv);
                    dcp_engine.vote(tid, "backup_self", DcpRole::Backup, proposal_hv);

                    // Try to resolve immediately (min_voters = 2, we have 3 voters)
                    if let Some(resolved) = dcp_engine.try_resolve(tid) {
                        dcp_resolution = Some((tid, resolved));
                        let _ = subconscious_log_tx.send(format!(
                            "AGENT {}: DCP CONSENSUS: thread={} resolved",
                            id_str, tid,
                        ));
                    }
                }

                // GC expired threads every 50 ticks
                if ticker > 0 && ticker % 50 == 0 {
                    dcp_engine.expire_old(ticker as u64);
                }

                // Update workspace with the latest resolution
                if let Some((_tid, ref resolution)) = dcp_resolution {
                    workspace.update_module(8, *resolution);  // CONSENSUS
                } else {
                    // If no consensus yet, use current identity as default
                    workspace.update_module(8, self_model.current_identity);
                }
            }

            // ── PSC PREDICTOR: Batch trend prediction ─────────────────
            {
                let state_snapshot = self_model.current_identity;
                psc_predictor.observe(ticker as u64, state_snapshot);
                if ticker > 0 && ticker % 15 == 0 {
                    if let Some((chaos, horizon, prediction)) = psc_predictor.report() {
                        if ticker % 60 == 0 {
                            let _ = subconscious_log_tx.send(format!(
                                "AGENT {}: PSC: chaos={:.3}, horizon={}, pred_popcount={:.3}",
                                id_str, chaos, horizon,
                                prediction.count_ones() as f64 / 10240.0,
                            ));
                        }
                    }
                }
            }

            // ── COUNTERFACTUAL SIMULATOR: Imagine alternative futures ──
            let drive_weights = drives.effective_weights(&[0.30, 0.30, 0.20, 0.20]);
            let sim_report = sim.evaluate_driven(
                &self_model.current_identity,
                self_model.homeostasis.overall_deficit,
                self_model.global_error,
                &workspace.global_broadcast,
                &drive_weights,
            );
            if ticker % 25 == 0 {
                let _ = subconscious_log_tx.send(format!(
                    "AGENT {}: {} | SIM: best={} (score={:.4})",
                    id_str, drives.report(),
                    sim_report.best_action.label,
                    sim_report.best_outcome.total_score,
                ));
                // Log the ranked outcomes
                for (i, o) in sim_report.ranked_outcomes.iter().take(3).enumerate() {
                    let _ = subconscious_log_tx.send(format!(
                        "AGENT {}: SIM:   {}. {} — score={:.4}",
                        id_str, i + 1, o.action_label, o.total_score,
                    ));
                }
            }

            // ── SLEEP / CONSOLIDATION CHECK ─────────────────────────────
            {
                // Record current tick
                sleeper.tick = ticker as u64;

                // Check energy from homeostasis
                let energy = homeostasis.needs.get(&the_machine::drift::Need::Energy)
                    .map(|s| s.current).unwrap_or(0.5);
                let integration = homeostasis.needs.get(&the_machine::drift::Need::Integration)
                    .map(|s| s.current).unwrap_or(0.5);
                let min_error = self_model.global_error.min(0.05);

                let (should_sleep_now, reason) = sleeper.should_sleep(
                    energy, integration, min_error, self_model.global_error,
                    workspace.is_idle(),
                );

                if should_sleep_now && !sleeper.sleeping {
                    // Run Phase 1+2: replay trajectory + narrative
                    let (transitions, narrative) = sleeper.phase1_replay(&self_model.trajectory);
                    let _ = subconscious_log_tx.send(format!(
                        "AGENT {}: SLEEP: triggered by {} — {} transitions, narrative pop={:.1}%",
                        id_str, reason, transitions.len(),
                        narrative.narrative_vector.count_ones() as f64 / 10240.0 * 100.0,
                    ));

                    // Generate a natural-language sleep narrative
                    {
                        let sleep_story = format!(
                            "I am tired. I need to sleep and consolidate what I have learned. \
                             I have {} significant transitions to process.",
                            transitions.len(),
                        );
                        let _ = subconscious_log_tx.send(format!(
                            "AGENT {}: 📖 {}", id_str, sleep_story
                        ));
                    }

                    // Sleep-phase drive adjustment: boost the most starved drive
                    let starved = drives.starved_drive();
                    drives.adjust_multipliers(starved, 0.15);
                    drives.reset_cumulative();
                    let _ = subconscious_log_tx.send(format!(
                        "AGENT {}: SLEEP: boosted {:?} multiplier (+0.15)",
                        id_str, starved,
                    ));

                    sleeper.last_sleep_tick = sleeper.tick;
                    sleeper.total_sleep_cycles += 1;
                }
            }

            // The winning broadcast is available as workspace.global_broadcast
            // Modules can query it on the next tick for context.

            // Sync stats to shared dashboard state
            if let Some(ref states) = shared_states {
                let mut guard = states.write().await;
                let active_port = *defense_subconscious.active_port.read().await;
                let stealth_active = *defense_subconscious.stealth_mode.read().await;
                let anxiety = brain_guard.anxiety;
                let perm_nodes = brain_guard.dejavu_clusters.len();
                let trans_nodes = brain_guard.transient_clusters.len();

                // Read Layers 3-5 integration metrics
                let (frames, rules_total, rules_trusted, c_targets, seed_q) = {
                    let pg = primary_int.read().await;
                    let mg = meta_int.read().await;
                    let sg = seed_urls_int.read().await;
                    (
                        pg.frame_count(),
                        mg.abductor.rules().len(),
                        mg.abductor.trustworthy_rules().len(),
                        mg.curiosity_targets_abduced_weighted(pg.frames(), &mg.signature_stats).len(),
                        sg.len(),
                    )
                };

                guard.insert(
                    id_str.clone(),
                    AgentState {
                        id: id_str.clone(),
                        role: role_str.clone(),
                        url: curr_url.clone(),
                        threat: threat_level,
                        anxiety,
                        stealth: stealth_active,
                        port: active_port,
                        permanent_nodes: perm_nodes,
                        transient_nodes: trans_nodes,
                        frames,
                        rules_total,
                        rules_trusted,
                        curiosity_targets: c_targets,
                        seed_queue: seed_q,
                    },
                );
            }
        }
    });

    Ok(())
}
