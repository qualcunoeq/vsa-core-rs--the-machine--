use the_machine::{
    action::ToolRegistry,
    autonomy::{AutonomyDrive, CuriosityDrive, DriveLabel, DriveSystem, GoalFormulationEngine},
    broker::NeocortexBroker,
    forager::VSAForager,
    graph::{ConditionalBranch, GraphReasoningEngine},
    sensory::{
        AudioModality, SensoryModality, SystemTelemetryModality, TextSensoryModality,
        UnifiedLatentSpace, VisualModality,
    },
    socket::AdminSocketServer,
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
    // New fields
    pub drive_mode: String,
    pub discoveries: usize,
    pub active_goal: String,
    pub tools_available: usize,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Ensure data directory exists
    std::fs::create_dir_all("data").unwrap_or(());

    let args: Vec<String> = std::env::args().collect();

    if args.contains(&"--broker".to_string()) {
        // ----------------- STANDALONE BROKER MODE -----------------
        run_broker(args).await
    } else if args.contains(&"--dht-broker".to_string()) {
        // ----------------- DHT-ENABLED FEDERATED BROKER MODE -----------------
        run_dht_broker(args).await
    } else if args.contains(&"--agent".to_string()) {
        // ----------------- STANDALONE AGENT MODE -----------------
        run_standalone_agent(args).await
    } else if args.contains(&"--graph-demo".to_string()) {
        // ----------------- GRAPH REASONING DEMO -----------------
        run_graph_demo().await
    } else if args.contains(&"--sensory-demo".to_string()) {
        // ----------------- MULTIMODAL SENSORY DEMO -----------------
        run_sensory_demo().await
    } else if args.contains(&"--curiosity-demo".to_string()) {
        // ----------------- CURIOSITY-DRIVEN EXPLORATION DEMO -----------------
        run_curiosity_demo().await
    } else if args.contains(&"--tool-demo".to_string()) {
        // ----------------- DYNAMIC TOOL REGISTRY DEMO -----------------
        run_tool_demo().await
    } else if args.contains(&"--goal-demo".to_string()) {
        // ----------------- GOAL-DIRECTED PLANNING DEMO -----------------
        run_goal_demo().await
    } else {
        // ----------------- DEFAULT PATH: MULTI-AGENT SIMULATION (UPGRADED) -----------------
        run_multi_agent_simulation(args).await
    }
}

// ─── BROKER MODE ──────────────────────────────────────────────────────────

async fn run_broker(_args: Vec<String>) -> Result<(), Box<dyn std::error::Error>> {
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
    Ok(())
}

// ─── DHT BROKER MODE ──────────────────────────────────────────────────────

async fn run_dht_broker(args: Vec<String>) -> Result<(), Box<dyn std::error::Error>> {
    let (log_tx, mut log_rx) = mpsc::unbounded_channel::<String>();
    let _ = log_tx.send("DHT BROKER: Federated Memory System Initialized.".to_string());

    tokio::spawn(async move {
        while let Some(msg) = log_rx.recv().await {
            println!("[{}] {}", Utc::now().format("%H:%M:%S"), msg);
        }
    });

    let mut broker = NeocortexBroker::new(
        "HAROLD_FINCH_API_KEY_SECRET",
        "data/long_term_ledger.bin",
        9050,
    );

    // Parse DHT args
    let peer_id = args.iter()
        .skip_while(|a| *a != "--peer-id")
        .nth(1)
        .map(|s| s.clone())
        .unwrap_or_else(|| format!("broker_{}", rand::thread_rng().gen_range(1000..9999)));

    let dht_port = args.iter()
        .skip_while(|a| *a != "--dht-port")
        .nth(1)
        .and_then(|s| s.parse::<u16>().ok())
        .unwrap_or(19050);

    let seed_host = args.iter()
        .skip_while(|a| *a != "--seed-host")
        .nth(1)
        .map(|s| s.clone());

    let seed_port = args.iter()
        .skip_while(|a| *a != "--seed-port")
        .nth(1)
        .and_then(|s| s.parse::<u16>().ok());

    let host = args.iter()
        .skip_while(|a| *a != "--host")
        .nth(1)
        .map(|s| s.clone())
        .unwrap_or_else(|| "127.0.0.1".to_string());

    let _ = log_tx.send(format!(
        "DHT BROKER: Peer ID={}, DHT Port={}, Host={}",
        peer_id, dht_port, host
    ));

    broker.init_dht(
        &peer_id,
        &host,
        dht_port,
        seed_host.as_deref(),
        seed_port,
    ).await;

    // Spawn DHT listener
    if let Some(ref dht) = broker.dht_node {
        let dht_clone = Arc::clone(dht);
        let dht_log = log_tx.clone();
        tokio::spawn(async move {
            let _ = dht_clone.run_dht_listener(dht_log).await;
        });
    }

    // Run broker
    let broker_arc = Arc::new(broker);
    let broker_log_tx = log_tx.clone();
    broker_arc.run(broker_log_tx).await?;
    Ok(())
}

// ─── STANDALONE AGENT MODE ────────────────────────────────────────────────

async fn run_standalone_agent(args: Vec<String>) -> Result<(), Box<dyn std::error::Error>> {
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

            let shared_states = Arc::new(RwLock::new(HashMap::<String, AgentState>::new()));
            let shared_states_clone = Arc::clone(&shared_states);

            run_agent(id, role, port, url, 9050, "HAROLD_FINCH_API_KEY_SECRET", Some(shared_states_clone), log_tx).await?;

            // Draw standalone agent HUD
            println!("\x1B[2J\x1B[1;1H");
            loop {
                sleep(Duration::from_millis(200)).await;
                print!("\x1B[H");

                let states = shared_states.read().await;
                let logs = shared_logs.read().await;

                if let Some(agent) = states.get(id) {
                    println!("\x1B[35m┌─────────────────────────────────────────────────────────────────────────────┐\x1B[0m\x1B[K");
                    println!("\x1B[35m│   \x1B[1;36mTHE MACHINE v9 GP NODE\x1B[0;35m  |  \x1B[1;32mCOGNITIVE AGENT\x1B[0;35m  |  \x1B[1;33mGP PROTOCOL\x1B[0;35m        │\x1B[0m\x1B[K");
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
                        "\x1B[35m│\x1B[0m  Threat Level: \x1B[1;31m{:>6.2}%\x1B[0m | Drive: \x1B[1;33m{:<28}\x1B[0m \x1B[35m│\x1B[0m\x1B[K",
                        agent.threat * 100.0,
                        if agent.drive_mode.len() > 28 { format!("{}...", &agent.drive_mode[0..25]) } else { agent.drive_mode.clone() }
                    );
                    println!(
                        "\x1B[35m│\x1B[0m  Cognitive Anxiety: \x1B[1;33m{:>6.2}%\x1B[0m | Discoveries: {:<3} | Tools: {:<3}      \x1B[35m│\x1B[0m\x1B[K",
                        agent.anxiety * 100.0,
                        agent.discoveries,
                        agent.tools_available
                    );
                    println!(
                        "\x1B[35m│\x1B[0m  Goal: \x1B[36m{:<63}\x1B[0m \x1B[35m│\x1B[0m\x1B[K",
                        if agent.active_goal.len() > 63 { format!("{}...", &agent.active_goal[0..60]) } else { agent.active_goal.clone() }
                    );
                    println!(
                        "\x1B[35m│\x1B[0m  Memory: Perm: {:<2} | Transient: {:<2}                                 \x1B[35m│\x1B[0m\x1B[K",
                        agent.permanent_nodes, agent.transient_nodes
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
    Ok(())
}

// ─── DEMO: GRAPH REASONING ────────────────────────────────────────────────

async fn run_graph_demo() -> Result<(), Box<dyn std::error::Error>> {
    println!("═══ THE MACHINE v9 — GRAPH REASONING DEMO ═══\n");

    let mut engine = GraphReasoningEngine::new();

    // Demonstrate N-ary relations
    println!("1. N-ary Relation Encoding:");
    engine.register_concept("Alice");
    engine.register_concept("give");
    engine.register_concept("book");
    engine.register_concept("Bob");
    let relation = engine.encode_relation(&[
        ("agent", "Alice"),
        ("action", "give"),
        ("object", "book"),
        ("instrument", "Bob"),
    ]).unwrap();
    println!("   Encoded relation: Alice give book to Bob ({} bindings)\n", relation.num_bindings);

    // Demonstrate temporal sequence
    println!("2. Temporal Sequence Reasoning:");
    let seq = engine.encode_temporal_sequence(&[
        vec![("agent", "Alice"), ("action", "open"), ("object", "door")],
        vec![("agent", "Alice"), ("action", "enter"), ("object", "room")],
        vec![("agent", "Alice"), ("action", "sit"), ("object", "chair")],
    ]).unwrap();
    println!("   Encoded 3-step temporal sequence\n");

    // Demonstrate conditional branching
    println!("3. Conditional Branching:");
    let branch = ConditionalBranch::new(
        Hypervector::encode_text_ngram("is_admin", 3),
        Hypervector::encode_text_ngram("grant_access", 3),
        Some(Hypervector::encode_text_ngram("deny_access", 3)),
    );
    let is_admin = Hypervector::encode_text_ngram("is_admin", 3);
    if let Some(result) = branch.evaluate(&is_admin) {
        println!("   Condition 'is_admin' → grants access (correct)\n");
    }

    // Demonstrate analogy
    println!("4. Analogical Reasoning (king:man :: queen:?):");
    let mut vocab = the_machine::resonator::ResonatorVocabulary::new();
    vocab.register_term("king");
    vocab.register_term("queen");
    vocab.register_term("man");
    vocab.register_term("woman");
    let result = engine.analogical_reasoning(
        vocab.get_vector("king").unwrap(),
        vocab.get_vector("man").unwrap(),
        vocab.get_vector("queen").unwrap(),
        &vocab,
    );
    match result {
        Some((term, sim)) => println!("   Result: queen:{} (confidence: {:.2})\n", term, sim),
        None => println!("   Analogy failed\n"),
    }

    println!("═══ GRAPH REASONING DEMO COMPLETE ═══");
    Ok(())
}

// ─── DEMO: MULTIMODAL SENSORY ─────────────────────────────────────────────

async fn run_sensory_demo() -> Result<(), Box<dyn std::error::Error>> {
    println!("═══ THE MACHINE v9 — MULTIMODAL SENSORY DEMO ═══\n");

    // 1. Text modality
    println!("1. Text Modality:");
    let text_mod = TextSensoryModality::new("news", "The machine breached the server admin panel");
    let text_hv = text_mod.encode();
    println!("   Encoded 'The machine breached the server admin panel'");
    println!("   Hypervector bits: {:?}...", &text_hv.bits[..2]);
    println!();

    // 2. Visual modality
    println!("2. Visual Modality (32×32 synthetic image):");
    let mut visual = VisualModality::new("camera", 32, 32);
    let mut pixels = vec![0.0; 32 * 32];
    for y in 8..24 {
        for x in 8..24 {
            pixels[y * 32 + x] = 1.0;
        }
    }
    visual.load_pixels(&pixels);
    let visual_hv = visual.encode();
    println!("   Encoded white square on black background");
    println!();

    // 3. Audio modality
    println!("3. Audio Modality (440Hz sine tone):");
    let mut audio = AudioModality::new("microphone", 44100);
    let mut samples = Vec::new();
    for i in 0..4410 {
        let t = i as f64 / 44100.0;
        samples.push((2.0 * std::f64::consts::PI * 440.0 * t).sin());
    }
    audio.load_samples(&samples);
    let audio_hv = audio.encode();
    println!("   Encoded 440Hz A4 tone (0.1 seconds)");
    println!();

    // 4. Unified Latent Space
    println!("4. Unified Latent Space (Cross-Modal Understanding):");
    let mut uls = UnifiedLatentSpace::new();
    let cat_text = Hypervector::encode_sentence("a cute cat playing");
    let cat_visual = Hypervector::encode_text_ngram("feline_features", 3);
    let cat_audio = Hypervector::encode_text_ngram("meow_sound_features", 3);
    uls.register_concept("cat", cat_text, Some(cat_visual), Some(cat_audio));

    let dog_text = Hypervector::encode_sentence("a friendly dog");
    uls.register_concept("dog", dog_text, None, None);

    let query = Hypervector::encode_sentence("cute feline pet");
    let results = uls.query(&query, 0.50);
    println!("   Query 'cute feline pet' matched:");
    for (label, sim, modality) in &results {
        println!("     - {} (sim={:.3}, {})", label, sim, modality);
    }
    println!();

    println!("═══ MULTIMODAL SENSORY DEMO COMPLETE ═══");
    Ok(())
}

// ─── DEMO: CURIOSITY-DRIVEN EXPLORATION ───────────────────────────────────

async fn run_curiosity_demo() -> Result<(), Box<dyn std::error::Error>> {
    println!("═══ THE MACHINE v9 — CURIOSITY-DRIVEN EXPLORATION DEMO ═══\n");

    let mut curiosity = CuriosityDrive::new(0.43);
    let mut memory: Vec<Hypervector> = Vec::new();

    // Simulate an environment with known and unknown states
    let known_state = Hypervector::encode_text_ngram("known_territory", 3);
    let novel_state = Hypervector::encode_text_ngram("uncharted_region", 3);
    let familiar_state = Hypervector::encode_text_ngram("already_explored", 3);

    // First, record some memory
    memory.push(known_state);
    memory.push(familiar_state);

    println!("1. Novelty Assessment:");
    let known_novelty = curiosity.compute_novelty(&known_state, &memory);
    let novel_novelty = curiosity.compute_novelty(&novel_state, &memory);
    println!("   Known state novelty: {:.3}", known_novelty);
    println!("   Novel state novelty: {:.3}", novel_novelty);
    println!();

    println!("2. Information Gain:");
    let known_gain = curiosity.information_gain(&known_state, &memory);
    let novel_gain = curiosity.information_gain(&novel_state, &memory);
    println!("   Known state info gain: {:.3}", known_gain);
    println!("   Novel state info gain: {:.3}", novel_gain);
    println!();

    println!("3. Exploration Decision (safe environment):");
    let should_explore = curiosity.should_explore(0.2, 0.1);
    println!("   Should explore (low dissonance, low threat)? {}", should_explore);
    println!();

    println!("4. Curiosity Lifecycle:");
    let mut curiosity2 = CuriosityDrive::new(0.43);
    println!("   Initial satiation: {:.3}", curiosity2.satiation);
    for i in 0..5 {
        curiosity2.discover();
        println!("   After discovery {}: satiation={:.3}, total discoveries={}",
            i + 1, curiosity2.satiation, curiosity2.discoveries);
    }
    curiosity2.decay(0.3);
    println!("   After decay: satiation={:.3}", curiosity2.satiation);
    println!();

    println!("5. Exploration Intent Generation:");
    let current = Hypervector::encode_text_ngram("current_pos", 3);
    let intent = curiosity.generate_exploration_intent(&current, &memory);
    curiosity.record_visit(&current);
    println!("   Generated exploration intent from current state");
    println!();

    println!("6. Drive System Integration:");
    let mut drive_sys = the_machine::autonomy::DriveSystem::new(0.43);
    let state = Hypervector::new_random();
    let state2 = Hypervector::new_random();
    let dissonance = AutonomyDrive::calculate_dissonance(&state, &state);
    let drive1 = drive_sys.evaluate_drive(&state, &dissonance, 0.0, &memory);
    let dissonance_high = AutonomyDrive::calculate_dissonance(&state, &state2);
    let drive2 = drive_sys.evaluate_drive(&state, &dissonance_high, 0.8, &memory);
    println!("   Drive (safe, no dissonance): {:?}", drive1);
    println!("   Drive (high threat): {:?}", drive2);
    println!();

    println!("═══ CURIOSITY-DRIVEN EXPLORATION DEMO COMPLETE ═══");
    Ok(())
}

// ─── DEMO: DYNAMIC TOOL REGISTRY ──────────────────────────────────────────

async fn run_tool_demo() -> Result<(), Box<dyn std::error::Error>> {
    println!("═══ THE MACHINE v9 — DYNAMIC TOOL-USE PROTOCOL DEMO ═══\n");

    let mut registry = ToolRegistry::new();

    println!("1. Available Built-in Tools:");
    for tool in registry.list_tools() {
        println!("   - {}: {} (cost={}, risk={})",
            tool.tool_id, tool.description, tool.compute_cost, tool.risk_score);
        if !tool.tags.is_empty() {
            println!("     Tags: {:?}", tool.tags);
        }
    }
    println!();

    println!("2. Tool Discovery by Tag:");
    let network_tools = registry.discover_by_tag("network");
    println!("   Network tools ({} found):", network_tools.len());
    for t in &network_tools {
        println!("     - {}", t.tool_id);
    }
    println!();

    println!("3. Tool Discovery by Similarity:");
    let query = Hypervector::encode_text_ngram("tool_http_get", 3);
    let similar = registry.discover_by_similarity(&query, 0.60);
    println!("   Tools similar to http_get ({} found):", similar.len());
    for (id, sim) in &similar {
        println!("     - {} (sim={:.3})", id, sim);
    }
    println!();

    println!("4. Tool Call Encoding/Decoding:");
    let param = Hypervector::encode_text_ngram("https://api.example.com/data", 3);
    let intent = registry.encode_tool_call("http_get", &[param]).unwrap();
    let (decoded_id, decoded_params) = registry.decode_tool_call(&intent).unwrap();
    println!("   Encoded call to 'http_get'");
    println!("   Decoded tool: '{}'", decoded_id);
    let param_sim = 1.0 - decoded_params[0].normalized_hamming_distance(&param);
    println!("   Parameter recovery similarity: {:.3} (bundling is lossy)", param_sim);
    println!();

    println!("5. Dynamic Tool Registration:");
    let custom_tool = the_machine::action::DynamicTool {
        signature: the_machine::action::ToolSignature {
            tool_id: "analyze_sentiment".to_string(),
            fingerprint: Hypervector::encode_text_ngram("tool_analyze_sentiment", 3),
            description: "Analyze sentiment of text input.".to_string(),
            input_types: vec![the_machine::action::ToolParamType {
                name: "text".to_string(),
                description: "Text to analyze".to_string(),
                type_vector: Hypervector::encode_text_ngram("param_text", 3),
            }],
            output_types: vec![the_machine::action::ToolParamType {
                name: "sentiment".to_string(),
                description: "Sentiment score".to_string(),
                type_vector: Hypervector::encode_text_ngram("param_sentiment", 3),
            }],
            compute_cost: 0.3,
            risk_score: 0.0,
            tags: vec!["ml".to_string(), "nlp".to_string(), "analysis".to_string()],
        },
        invoke_fn: Arc::new(|params: &[Hypervector]| Ok(params[0])),
    };
    registry.register_tool(custom_tool);
    let ml_tools = registry.discover_by_tag("nlp");
    println!("   Registered 'analyze_sentiment' tool");
    println!("   NLP tools now available: {}", ml_tools.len());
    println!();

    println!("═══ DYNAMIC TOOL-USE PROTOCOL DEMO COMPLETE ═══");
    Ok(())
}

// ─── DEMO: GOAL-DIRECTED PLANNING ─────────────────────────────────────────

async fn run_goal_demo() -> Result<(), Box<dyn std::error::Error>> {
    println!("═══ THE MACHINE v9 — GOAL-DIRECTED PLANNING DEMO ═══\n");

    let mut gfe = GoalFormulationEngine::new();

    println!("1. Goal Injection:");
    let target = Hypervector::encode_text_ngram("secure_server_state", 3);
    let current = Hypervector::new_zero();
    let id = gfe.inject_goal("Secure the production server", target, current, 0.95);
    println!("   Injected goal '{}' with priority 0.95", id);
    println!("   Active goal: {:?}", gfe.get_goal_description());
    println!();

    println!("2. Goal Achievement Check:");
    let achieved = gfe.check_achievement(&target, 0.75);
    println!("   State matches target? {}", achieved);
    let different = Hypervector::new_random();
    let not_achieved = gfe.check_achievement(&different, 0.75);
    println!("   Random state matches target? {}", not_achieved);
    println!();

    println!("3. Goal Decomposition:");
    let vocab = the_machine::resonator::ResonatorVocabulary::new();
    let subgoals = gfe.decompose_goal(&vocab);
    println!("   Decomposed into {} sub-goals:", subgoals.len());
    for sg in &subgoals {
        println!("     - {}: {}", sg.id, sg.description);
    }
    println!();

    println!("4. Goal Completion & History:");
    gfe.achieve_goal(12, 3.5);
    println!("   Marked goal as achieved (12 steps, cost=3.5)");
    println!("   History entries: {}", gfe.goal_history.len());

    // Inject another similar goal and check similarity
    let target2 = Hypervector::encode_text_ngram("secure_server_state", 3);
    let current2 = Hypervector::new_zero();
    gfe.inject_goal("Secure another server", target2, current2, 0.8);

    let similar = gfe.get_similar_past_outcomes(&target2, 0.70);
    println!("   Similar past goals found: {}", similar.len());
    for record in &similar {
        println!("     - Previous: {} (success={})", record.goal.description, record.success);
    }
    println!();

    println!("5. Drive System Integration:");
    let mut ds = the_machine::autonomy::DriveSystem::new(0.43);
    let ds_target = Hypervector::encode_text_ngram("optimized_state", 3);
    let ds_current = Hypervector::new_random();
    ds.goal_formulation.inject_goal("Optimize system", ds_target, ds_current, 0.8);
    let state = Hypervector::new_random();
    let dissonance = AutonomyDrive::calculate_dissonance(&state, &state);
    let drive = ds.evaluate_drive(&state, &dissonance, 0.0, &[]);
    println!("   Active drive: {:?}", drive);
    println!();

    println!("═══ GOAL-DIRECTED PLANNING DEMO COMPLETE ═══");
    Ok(())
}

// ══════════════════════════════════════════════════════════════════════════
// MULTI-AGENT SIMULATION (UPGRADED with GP capabilities)
// ══════════════════════════════════════════════════════════════════════════

async fn run_multi_agent_simulation(_args: Vec<String>) -> Result<(), Box<dyn std::error::Error>> {
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

    // 2. Launch 3 upgraded heterogeneous cognitive Agents
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
    println!("\x1B[2J\x1B[1;1H");
    loop {
        sleep(Duration::from_millis(200)).await;
        print!("\x1B[H");

        let broker_clusters = broker.dejavu_clusters.read().await.len();
        let broker_clients = broker.clients.lock().await.len();
        let logs = shared_logs.read().await;
        let states = shared_states.read().await;

        println!("\x1B[35m┌─────────────────────────────────────────────────────────────────────────────┐\x1B[0m\x1B[K");
        println!("\x1B[35m│   \x1B[1;36mTHE MACHINE v9 GP HIVE MIND\x1B[0;35m  |  \x1B[1;32mGENERAL-PURPOSE COGNITIVE SYSTEM\x1B[0;35m         │\x1B[0m\x1B[K");
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
                    "│\x1B[36m [{}: {} AGENT (Admin Port: {})]\x1B[0m                           \x1B[35m│\x1B[0m\x1B[K",
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
                    "│  Threat: \x1B[1;31m{:>6.2}%\x1B[0m | Drive: \x1B[1;33m{:<30}\x1B[0m \x1B[35m│\x1B[0m\x1B[K",
                    agent.threat * 100.0,
                    if agent.drive_mode.len() > 30 { format!("{}...", &agent.drive_mode[0..27]) } else { agent.drive_mode.clone() }
                );
                println!(
                    "│  Anxiety: \x1B[1;33m{:>6.2}%\x1B[0m | Discvrs: {:<3} | Tools: {:<3} | Mem: P:{:<2}/T:{:<2} \x1B[35m│\x1B[0m\x1B[K",
                    agent.anxiety * 100.0,
                    agent.discoveries,
                    agent.tools_available,
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

// ══════════════════════════════════════════════════════════════════════════
// CORE AGENT LOOP (UPGRADED with DriveSystem, ToolRegistry, GoalFormulation)
// ══════════════════════════════════════════════════════════════════════════

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

    // ── Init new GP modules ─────────────────────────────────────────
    let drive_system = Arc::new(RwLock::new(DriveSystem::new(0.44)));
    let tool_registry = Arc::new(ToolRegistry::new());
    let graph_engine = Arc::new(RwLock::new(GraphReasoningEngine::new()));
    let unified_latent = Arc::new(RwLock::new(UnifiedLatentSpace::new()));

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

                    let mut port = defense_recv.active_port.write().await;
                    let new_port = rand::thread_rng().gen_range(9001..=9999);
                    *port = new_port;
                    *defense_recv.stealth_mode.write().await = true;

                    let mut intent_guard = intent_recv.write().await;
                    *intent_guard = Hypervector::new_random();
                }
                Ok(Some(HiveMessage::DissonanceAlert { .. })) => {
                    let _ = log_tx_recv.send(format!(
                        "AGENT {}: Received DissonanceAlert from broker. Resetting curiosity focus.",
                        id_str
                    ));
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

    // 7. Spawn Subconscious Drive Loop (UPGRADED with GP capabilities)
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

    // GP module handles
    let ds_handle = Arc::clone(&drive_system);
    let tr_handle = Arc::clone(&tool_registry);
    let ge_handle = Arc::clone(&graph_engine);
    let uls_handle = Arc::clone(&unified_latent);

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

        // GP memory for curiosity
        let mut memory_history: Vec<Hypervector> = Vec::new();

        loop {
            sleep(Duration::from_secs(2)).await;
            ticker += 1;

            let mut current_tick_actions = Vec::new();

            defense_subconscious.decrement_threat(0.01).await;

            // ── GP: Tool availability log (periodic broadcast) ──────
            if ticker % 20 == 0 {
                let tools = &*tr_handle;
                let tool_count = tools.list_tools().len();
                let _ = subconscious_log_tx.send(format!(
                    "AGENT {}: GP Tool Registry active — {} tools available for dynamic task execution.",
                    id_str, tool_count
                ));
            }

            // ── GP: Unified Latent Space registration (periodic) ────
            if ticker == 1 {
                let mut uls = uls_handle.write().await;
                // Bootstrap with some cross-modal concepts
                let cat_text = Hypervector::encode_sentence("cat animal feline");
                let cat_vis = Hypervector::encode_text_ngram("cat_visual", 3);
                uls.register_concept("cat", cat_text, Some(cat_vis), None);

                let server_text = Hypervector::encode_sentence("server computer system");
                let server_vis = Hypervector::encode_text_ngram("server_visual", 3);
                uls.register_concept("server", server_text, Some(server_vis), None);

                let _ = subconscious_log_tx.send(format!(
                    "AGENT {}: Unified Latent Space initialized with {} concepts.",
                    id_str, uls.len()
                ));
            }

            // v9.0 Sensory Encoders integration
            let mut telemetry_mod = SystemTelemetryModality::new("telemetry");
            telemetry_mod.set_reading("cpu_utilization", 10.0 + (ticker % 10) as f64 * 5.0);
            telemetry_mod.set_reading("ram_free_gb", 48.0 - (ticker % 5) as f64 * 4.0);
            let _v_telemetry = telemetry_mod.encode();

            let curr_url = current_url_forager.read().await;
            let news_headline = curr_url.split('/').last().unwrap_or("Index");
            let text_mod = TextSensoryModality::new("text_feed", news_headline);
            let _v_text = text_mod.encode();

            let network_mod = the_machine::sensory::NetworkTrafficModality::new("network");
            let _v_network = network_mod.encode();

            // Decay working memory
            let consolidated = {
                let mut brain_guard = brain_subconscious.write().await;
                brain_guard.decay_permanent_clusters(0.98, 0.15);
                let results = brain_guard.decay_transient_clusters_distributed(0.95, 5.0, 0.35);
                let anxiety_val = brain_guard.anxiety;
                *defense_subconscious.anxiety.write().await = anxiety_val;
                results
            };

            // Send consolidated items to Broker
            let anxiety_for_broker = {
                let d = defense_subconscious.anxiety.read().await;
                *d
            };
            for (centroid, entries) in consolidated {
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
                let request = HiveMessage::PanicLockdown {
                    attacker_info: format!("Agent {} Admin Breach", id_str),
                };
                let mut writer_guard = writer_clone.lock().await;
                let _ = NeocortexBroker::write_msg(&mut writer_guard, &request, &key_str_subconscious).await;
            }

            let port_rotated = defense_subconscious.evaluate_threat_response().await;
            if port_rotated {
                defense_subconscious.scrub_traces().await;
            }

            // ── Periodically prune vocabulary terms ────────────────
            if ticker % 30 == 0 {
                let pruned = resonator_vocab.prune_vocabulary(0.70);
                if pruned > 0 {
                    let _ = subconscious_log_tx.send(format!(
                        "AGENT {}: Pruned {} redundant vocabulary terms (θ=0.70).",
                        id_str, pruned
                    ));
                }
            }

            // ── GP: Curiosity-driven exploration ───────────────────
            if ticker % 10 == 0 {
                let mut ds = ds_handle.write().await;
                let state = {
                    let ws = world_state_subconscious.read().await;
                    *ws
                };
                let dissonance = AutonomyDrive::calculate_dissonance(&state, &c_normal);
                let drive = ds.evaluate_drive(&state, &dissonance, threat_level, &memory_history);

                match &drive {
                    DriveLabel::Curious(info) => {
                        let _ = subconscious_log_tx.send(format!(
                            "AGENT {}: Curiosity drive active — {}",
                            id_str, info
                        ));
                        // Generate exploration intent
                        let explore_intent = ds.curiosity.generate_exploration_intent(&state, &memory_history);
                        let mut intent_guard = intent_subconscious.write().await;
                        *intent_guard = explore_intent;
                        ds.curiosity.record_visit(&state);
                    }
                    DriveLabel::Reactive(info) => {
                        let _ = subconscious_log_tx.send(format!(
                            "AGENT {}: Reactive drive active — {}",
                            id_str, info
                        ));
                    }
                    DriveLabel::GoalDirected(desc) => {
                        let _ = subconscious_log_tx.send(format!(
                            "AGENT {}: Pursuing goal — {}",
                            id_str, desc
                        ));
                    }
                    DriveLabel::Idle => {
                        // Idle — decay curiosity so it builds again
                        ds.curiosity.decay(0.01);
                    }
                }

                let drive_label = format!("{:?}", drive);
                let mut drive_guard = active_drive_subconscious.write().await;
                *drive_guard = drive_label;
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

            // ── GP: Add world state to memory history for curiosity ──
            memory_history.push(current_world_state);
            if memory_history.len() > 100 {
                memory_history.remove(0);
            }

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

            // ── Regime-adaptive drift tracking (EWMA + variance) ────
            let deltas_vec: Vec<Hypervector> = recent_deltas.iter().cloned().collect();
            let drift_var = if deltas_vec.len() >= 2 {
                the_machine::planning::drift_variance(&deltas_vec)
            } else {
                0.0
            };
            active_drift = the_machine::planning::bundle_weighted_ewma(&deltas_vec, 3);
            let regime_volatility = (drift_var / 0.5).min(1.0);

            let mut drift_seq: Vec<Hypervector> = Vec::with_capacity(2);
            for i in 0..2 {
                drift_seq.push(
                    deltas_vec.get(deltas_vec.len().saturating_sub(2).wrapping_add(i))
                        .copied()
                        .unwrap_or(active_drift)
                );
            }

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

            let mut crisis_concepts = vec![c_crisis];
            crisis_concepts.extend(brain_guard.collect_learned_crisis_concepts());

            if should_pivot {
                let mut drive_guard = active_drive_subconscious.write().await;
                let mut intent_guard = intent_subconscious.write().await;

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

                if let Some((_name, param_hv)) =
                    action_registry.decode_intent(&chosen_intent, &resonator_vocab)
                {
                    *forager_target.write().await = Some(param_hv);
                }
            } else {
                let mut drive_guard = active_drive_subconscious.write().await;
                *drive_guard = "Autonomous / Idle Search".to_string();
            }

            // ── GP: Goal achievement check ─────────────────────────
            {
                let mut ds = ds_handle.write().await;
                if ds.goal_formulation.check_achievement(&current_world_state, 0.75) {
                    ds.goal_formulation.achieve_goal(ticker as usize, 0.0);
                    let _ = subconscious_log_tx.send(format!(
                        "AGENT {}: Goal achieved! Starting new exploration cycle.",
                        id_str
                    ));
                }
            }

            // ── Dynamic threat forecasting and planning ─────────────
            if ticker % 15 == 0 {
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
                        "AGENT {}: FORECAST ALERT! High threat state predicted in {:.1} steps.",
                        id_str, expected_steps
                    ));

                    // Causal rule chaining
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
                                "AGENT {}: Causal rule detected — {} {} {:?}. Storing.",
                                id_str, rule_s, rule_v, rule_slot
                            ));
                            drop(brain_guard);
                            let mut brain_write = brain_subconscious.write().await;
                            let mut rule_meta = std::collections::HashMap::new();
                            rule_meta.insert("type".to_string(), "causal_rule".to_string());
                            rule_meta.insert("subject".to_string(), rule_s.clone());
                            rule_meta.insert("verb".to_string(), rule_v.clone());
                            brain_write.add_transient_fact(
                                drift_pattern,
                                &format!("IF_{}_THEN_RISK", rule_v),
                                rule_meta,
                            );
                            drop(brain_write);
                            brain_guard = brain_subconscious.read().await;
                        }
                    }

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
                                "AGENT {}: Executing step {}/{} -> {} {}",
                                id_str, idx + 1, trajectory.steps.len(), step.action, step.parameter
                            ));

                            let step_param_hv =
                                resonator_vocab.get_vector(&step.parameter).unwrap();

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
                            "AGENT {}: Pathfinder failed to resolve a corrective plan.",
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
            if ticker % 50 == 0 && ticker > 0 {
                let exps = {
                    let brain_read = brain_subconscious.read().await;
                    brain_read.experiences.clone()
                };
                if exps.len() >= 5 {
                    let v_failure = Hypervector::encode_text_ngram("FAILURE", 3);
                    let mut failure_states: Vec<Hypervector> = Vec::new();
                    for exp in &exps {
                        let sim = 1.0 - exp.normalized_hamming_distance(&v_failure);
                        if sim > 0.6 {
                            failure_states.push(*exp);
                        }
                    }
                    if failure_states.len() >= 3 {
                        let refs: Vec<&Hypervector> = failure_states.iter().collect();
                        let learned_crisis = Hypervector::bundle(&refs);
                        let _ = subconscious_log_tx.send(format!(
                            "AGENT {}: Experience feedback — clustered {} failure patterns.",
                            id_str, failure_states.len()
                        ));
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

                let (drive_mode_text, discoveries, goal_text) = {
                    let ds = ds_handle.read().await;
                    let drive_txt = format!("{:?}", ds.active_drive);
                    let disc = ds.curiosity.discoveries;
                    let goal = ds.goal_formulation.get_goal_description()
                        .unwrap_or_else(|| "None".to_string());
                    (drive_txt, disc, goal)
                };

                let tool_count = {
                    tr_handle.list_tools().len()
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
                        drive_mode: drive_mode_text,
                        discoveries,
                        active_goal: goal_text,
                        tools_available: tool_count,
                    },
                );
            }
        }
    });

    Ok(())
}
