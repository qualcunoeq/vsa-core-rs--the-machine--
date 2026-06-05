use the_machine::{
    autonomy::AutonomyDrive, broker::NeocortexBroker, forager::VSAForager,
    sensory::SensoryModality, socket::AdminSocketServer, HiveMessage, Hypervector, VSABrain,
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

                run_agent(id, role, port, url, 9050, "HAROLD_FINCH_API_KEY_SECRET", Some(shared_states_clone), log_tx).await?;

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

        // 2. Launch 3 heterogeneous cognitive Agents
        let log_tx_a1 = log_tx.clone();
        let shared_states_a1 = Arc::clone(&shared_states);
        tokio::spawn(async move {
            let _ = run_agent(
                "Agent-1",
                "News",
                9001,
                "https://news.ycombinator.com",
                9050,
                "HAROLD_FINCH_API_KEY_SECRET",
                Some(shared_states_a1),
                log_tx_a1,
            )
            .await;
        });

        let log_tx_a2 = log_tx.clone();
        let shared_states_a2 = Arc::clone(&shared_states);
        tokio::spawn(async move {
            let _ = run_agent(
                "Agent-2",
                "Infra",
                9002,
                "https://news.ycombinator.com/from?site=espressif.com",
                9050,
                "HAROLD_FINCH_API_KEY_SECRET",
                Some(shared_states_a2),
                log_tx_a2,
            )
            .await;
        });

        let log_tx_a3 = log_tx.clone();
        let shared_states_a3 = Arc::clone(&shared_states);
        tokio::spawn(async move {
            let _ = run_agent(
                "Agent-3",
                "Market",
                9003,
                "https://finance.yahoo.com",
                9050,
                "HAROLD_FINCH_API_KEY_SECRET",
                Some(shared_states_a3),
                log_tx_a3,
            )
            .await;
        });

        // TUI Render Loop for Multi-Agent Hive Mind Simulation
        println!("\x1B[2J\x1B[1;1H"); // clear screen
        loop {
            sleep(Duration::from_millis(200)).await;
            print!("\x1B[H");

            let broker_clusters = broker.dejavu_clusters.read().await.len();
            let broker_clients = broker.clients.lock().await.len();
            let logs = shared_logs.read().await;
            let states = shared_states.read().await;

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

    brain.register_variable("vix_zscore", -3.0, 3.0);
    brain.register_variable("move_zscore", -3.0, 3.0);
    brain.register_variable("level_zscore", -3.0, 3.0);
    brain.register_variable("slope_zscore", -3.0, 3.0);
    brain.register_variable("curvature_zscore", -3.0, 3.0);

    let c_crisis = brain.register_concept("SystemicCrisis");
    let c_normal = brain.register_concept("Equilibrium");

    let v_role_market = Hypervector::role_market();
    let v_role_news = Hypervector::role_news();
    let v_role_infra = Hypervector::role_infra();

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

    // 5. Spawn Crawler Loop
    let mut forager = VSAForager::new(initial_intent, start_url.to_string(), 1500);
    // Share the semantic target parameter so the subconscious loop can
    // update it whenever a structured corrective intent is formulated.
    let forager_target_parameter: Arc<RwLock<Option<Hypervector>>> =
        Arc::new(RwLock::new(None));
    forager.target_parameter = Arc::clone(&forager_target_parameter);
    forager.intent = Arc::clone(&active_intent);
    forager.current_url = Arc::clone(&shared_current_url);
    forager.brain = Some(Arc::clone(&brain_shared));
    let forager_arc = Arc::new(tokio::sync::Mutex::new(forager));
    let forager_task_arc = Arc::clone(&forager_arc);
    let forager_log_tx = log_tx.clone();
    tokio::spawn(async move {
        VSAForager::run_loop(forager_task_arc, forager_log_tx).await;
    });

    // 6. Spawn TCP Admin Socket override server
    let admin_server = AdminSocketServer::new(
        Arc::clone(&active_intent),
        defense.clone(),
        Arc::clone(&brain_shared),
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

    tokio::spawn(async move {
        let action_registry = the_machine::action::ActionRegistry::new();
        let mut resonator_vocab = the_machine::resonator::ResonatorVocabulary::new();
        resonator_vocab.register_term("cargo check");
        resonator_vocab.register_term("data/temp_write_status.txt");
        resonator_vocab.register_term("hosts");

        let mut recent_states: std::collections::VecDeque<Hypervector> =
            std::collections::VecDeque::new();
        let mut recent_actions: std::collections::VecDeque<Hypervector> =
            std::collections::VecDeque::new();
        let mut recent_deltas: std::collections::VecDeque<Hypervector> =
            std::collections::VecDeque::new();
        let mut active_drift;
        let history_limit = 5;

        let mut stable_error = 0.5;
        let mut nominal_error = 0.5;
        let mut volatile_error = 0.5;
        let mut pred_stable: Option<Hypervector> = None;
        let mut pred_nominal: Option<Hypervector> = None;
        let mut pred_volatile: Option<Hypervector> = None;

        let mut ticker = 0;
        let mut sent_lockdown = false;
        loop {
            sleep(Duration::from_secs(2)).await;
            ticker += 1;

            let mut current_tick_actions = Vec::new();

            defense_subconscious.decrement_threat(0.01).await;

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

            let mut telemetry = HashMap::new();
            let is_crisis_tick = ticker % 20 > 15;
            let vix = if is_crisis_tick {
                2.9 + (ticker % 3) as f64 * 0.05
            } else {
                0.2 + (ticker % 5) as f64 * 0.1
            };
            let mov = if is_crisis_tick { 3.0 } else { 0.1 };
            let slope = if is_crisis_tick { -2.4 } else { 0.5 };

            telemetry.insert("vix_zscore".to_string(), vix);
            telemetry.insert("move_zscore".to_string(), mov);
            telemetry.insert(
                "level_zscore".to_string(),
                if is_crisis_tick { -1.8 } else { 0.1 },
            );
            telemetry.insert("slope_zscore".to_string(), slope);
            telemetry.insert(
                "curvature_zscore".to_string(),
                if is_crisis_tick { 0.4 } else { 0.0 },
            );

            {
                let mut metrics_guard = metrics_clone.write().await;
                *metrics_guard = telemetry.clone();
            }

            let mut brain_guard = brain_subconscious.read().await;
            let market_state = brain_guard.compile_state_vector(&telemetry);

            let curr_url = current_url_forager.read().await;
            let news_headline = curr_url.split('/').last().unwrap_or("Index");
            let news_state = Hypervector::encode_text_ngram(news_headline, 3);

            let ping_status = if is_crisis_tick {
                "OUTAGE_THREAT"
            } else {
                "STABLE"
            };
            let infra_state = Hypervector::encode_text_ngram(ping_status, 3);

            let bound_market = market_state.bitwise_xor(&v_role_market);
            let bound_news = news_state.bitwise_xor(&v_role_news);
            let bound_infra = infra_state.bitwise_xor(&v_role_infra);

            let current_world_state =
                Hypervector::bundle(&[&bound_market, &bound_news, &bound_infra]);

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

            let _resolved_concept = {
                let (label, _) = brain_guard.evaluate_deja_vu(&current_world_state);
                if let Some(ref lbl) = label {
                    if lbl.contains("Lehman") {
                        c_crisis
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
            let should_pivot = drive.evaluates_necessity_to_pivot(&dissonance);

            // SVO candidate lists for semantic intent formulation
            let auto_subjects: Vec<String> =
                the_machine::autonomy::DEFAULT_SUBJECTS.iter().map(|s| s.to_string()).collect();
            let auto_verbs: Vec<String> =
                the_machine::autonomy::DEFAULT_VERBS.iter().map(|v| v.to_string()).collect();
            let auto_objects: Vec<String> =
                the_machine::autonomy::DEFAULT_OBJECTS.iter().map(|o| o.to_string()).collect();

            let crisis_memory = brain_guard
                .dejavu_clusters
                .first()
                .map(|c| c.centroid)
                .unwrap_or(c_crisis);
            let crisis_sim = 1.0 - current_world_state.normalized_hamming_distance(&crisis_memory);

            // ── Inject learned crisis clusters into planning ─────────
            // Build a combined crisis_concepts slice that includes both
            // the statically-registered c_crisis vector AND any centroids
            // learned from experience feedback.
            let mut crisis_concepts = vec![c_crisis];
            crisis_concepts.extend(brain_guard.collect_learned_crisis_concepts());

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
            } else if crisis_sim > 0.55 {
                let mut drive_guard = active_drive_subconscious.write().await;
                let mut intent_guard = intent_subconscious.write().await;

                // Phantom pain: try parsing the offset from crisis memory
                let phantom = current_world_state.bitwise_xor(&crisis_memory);
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
                                    brain_write.experiences.push(experience_hv);
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

            // Sync stats to shared dashboard state
            if let Some(ref states) = shared_states {
                let mut guard = states.write().await;
                let active_port = *defense_subconscious.active_port.read().await;
                let stealth_active = *defense_subconscious.stealth_mode.read().await;
                let anxiety = brain_guard.anxiety;
                let perm_nodes = brain_guard.dejavu_clusters.len();
                let trans_nodes = brain_guard.transient_clusters.len();

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
                    },
                );
            }
        }
    });

    Ok(())
}
