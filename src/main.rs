use the_machine::{
    autonomy::AutonomyDrive,
    forager::VSAForager,
    ledger::LongTermLedger,
    socket::AdminSocketServer,
    Hypervector,
    VSABrain,
};

use chrono::Utc;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{mpsc, RwLock};
use tokio::time::{sleep, Duration};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Ensure data directory exists
    std::fs::create_dir_all("data").unwrap_or(());

    // Channel for routing logs to TUI
    let (log_tx, mut log_rx) = mpsc::unbounded_channel::<String>();
    let _ = log_tx.send("THE MACHINE: Operating System Initialized.".to_string());
    let _ = log_tx.send("Bypassing modern deep learning stack. Native VSA Core loaded.".to_string());

    // 1. Initialize VSA Cognitive Brain
    let mut brain = VSABrain::new(0.43);
    let _ = log_tx.send("Registering sensory telemetry slots...".to_string());
    
    // Telemetry registers (z-scores)
    brain.register_variable("vix_zscore", -3.0, 3.0);
    brain.register_variable("move_zscore", -3.0, 3.0);
    brain.register_variable("level_zscore", -3.0, 3.0);
    brain.register_variable("slope_zscore", -3.0, 3.0);
    brain.register_variable("curvature_zscore", -3.0, 3.0);

    // Register semantic contexts
    let c_crisis = brain.register_concept("SystemicCrisis");
    let c_normal = brain.register_concept("Equilibrium");

    // Permanent orthogonal role vectors for modality sieve
    let v_role_market = Hypervector::role_market();
    let v_role_news = Hypervector::role_news();
    let v_role_infra = Hypervector::role_infra();

    // 2. Setup historical database templates (Deja Vu)
    let mut crisis_telemetry = HashMap::new();
    crisis_telemetry.insert("level_zscore".to_string(), -2.0);
    crisis_telemetry.insert("slope_zscore".to_string(), -2.5);
    crisis_telemetry.insert("curvature_zscore".to_string(), 0.5);
    crisis_telemetry.insert("vix_zscore".to_string(), 3.0);
    crisis_telemetry.insert("move_zscore".to_string(), 3.0);
    
    let crisis_state = brain.compile_state_vector(&crisis_telemetry);
    let crisis_memory = brain.bind(&crisis_state, &c_crisis);
    
    let mut metadata = HashMap::new();
    metadata.insert("class".to_string(), "Liquidity Crisis / Market Inversion".to_string());
    metadata.insert("severity".to_string(), "CRITICAL".to_string());
    brain.add_to_dejavu_db(crisis_memory, "2008 Lehman / 2020 Liquidity Analogue", metadata.clone());

    let mut normal_telemetry = HashMap::new();
    normal_telemetry.insert("level_zscore".to_string(), 0.0);
    normal_telemetry.insert("slope_zscore".to_string(), 0.0);
    normal_telemetry.insert("curvature_zscore".to_string(), 0.0);
    normal_telemetry.insert("vix_zscore".to_string(), 0.0);
    normal_telemetry.insert("move_zscore".to_string(), 0.0);
    
    let normal_state = brain.compile_state_vector(&normal_telemetry);
    let normal_memory = brain.bind(&normal_state, &c_normal);
    let mut normal_meta = HashMap::new();
    normal_meta.insert("class".to_string(), "Stable Growth Expansion".to_string());
    normal_meta.insert("severity".to_string(), "LOW".to_string());
    brain.add_to_dejavu_db(normal_memory, "Equilibrium Regime", normal_meta);

    // 3. Initialize Shared Active Intent and World State
    let initial_intent = Hypervector::new_random();
    let active_intent = Arc::new(RwLock::new(initial_intent));
    let active_world_state = Arc::new(RwLock::new(Hypervector::new_zero()));
    
    // Shared parameters for TUI updates
    let shared_current_url = Arc::new(RwLock::new("https://news.ycombinator.com".to_string()));
    let shared_metrics = Arc::new(RwLock::new(HashMap::<String, f64>::new()));
    let shared_logs = Arc::new(RwLock::new(Vec::<String>::new()));
    let shared_phantom_pain = Arc::new(RwLock::new(0.0));
    let shared_active_drive = Arc::new(RwLock::new("Subconscious".to_string()));

    // Clones for tasks
    let current_url_forager = Arc::clone(&shared_current_url);
    let metrics_clone = Arc::clone(&shared_metrics);
    
    // 4. Initialize Encrypted Ledger
    let ledger = LongTermLedger::new("HAROLD_FINCH_API_KEY_SECRET", "data/long_term_ledger.bin");
    let _ = log_tx.send(format!("Loaded long-term ledger. Found {} encrypted daily records.", 
        ledger.load_records().unwrap_or(Vec::new()).len()));

    // 5. Spawn Asynchronous Web Forager Loop
    let mut forager = VSAForager::new(initial_intent, "https://news.ycombinator.com".to_string(), 1500);
    // Bind forager inner references to shared dashboard states
    forager.intent = Arc::clone(&active_intent);
    forager.current_url = Arc::clone(&shared_current_url);
    let forager_arc = Arc::new(tokio::sync::Mutex::new(forager));
    
    let forager_task_arc = Arc::clone(&forager_arc);
    let forager_log_tx = log_tx.clone();
    tokio::spawn(async move {
        VSAForager::run_loop(forager_task_arc, forager_log_tx).await;
    });

    // 6. Spawn TCP Admin Overrides Server (Port 9000)
    let admin_server = AdminSocketServer::new(Arc::clone(&active_intent), 9000);
    let admin_log_tx = log_tx.clone();
    tokio::spawn(async move {
        if let Err(e) = admin_server.run(admin_log_tx).await {
            eprintln!("Socket override server crashed: {}", e);
        }
    });

    // 7. Spawn Sensor Ingestion & Subconscious Drive Loop
    let brain_shared = Arc::new(RwLock::new(brain));
    let subconscious_log_tx = log_tx.clone();
    let intent_subconscious = Arc::clone(&active_intent);
    let world_state_subconscious = Arc::clone(&active_world_state);
    
    let phantom_pain_subconscious = Arc::clone(&shared_phantom_pain);
    let active_drive_subconscious = Arc::clone(&shared_active_drive);
    
    tokio::spawn(async move {
        let mut ticker = 0;
        loop {
            sleep(Duration::from_secs(2)).await;
            ticker += 1;
            
            // Simulating continuous multi-modal feeds
            let mut telemetry = HashMap::new();
            
            // Periodically inject a massive crisis state to show the subconscious drive reaction
            let is_crisis_tick = ticker % 20 > 15;
            
            let vix = if is_crisis_tick { 2.9 + (ticker % 3) as f64 * 0.05 } else { 0.2 + (ticker % 5) as f64 * 0.1 };
            let mov = if is_crisis_tick { 3.0 } else { 0.1 };
            let slope = if is_crisis_tick { -2.4 } else { 0.5 };
            
            telemetry.insert("vix_zscore".to_string(), vix);
            telemetry.insert("move_zscore".to_string(), mov);
            telemetry.insert("level_zscore".to_string(), if is_crisis_tick { -1.8 } else { 0.1 });
            telemetry.insert("slope_zscore".to_string(), slope);
            telemetry.insert("curvature_zscore".to_string(), if is_crisis_tick { 0.4 } else { 0.0 });
            
            // Update shared metrics
            {
                let mut metrics_guard = metrics_clone.write().await;
                *metrics_guard = telemetry.clone();
            }

            let brain_guard = brain_shared.read().await;
            
            // A. Numeric market state vector
            let market_state = brain_guard.compile_state_vector(&telemetry);
            
            // B. Scraped news text vector (simulated from current anchor text)
            let curr_url = current_url_forager.read().await;
            let news_headline = curr_url.split('/').last().unwrap_or("Hacker News Article Index");
            let news_state = Hypervector::encode_text_ngram(news_headline, 3);
            
            // C. Infrastructure stability vector (discrete states: "stable" vs "outage")
            let ping_status = if is_crisis_tick { "OUTAGE_THREAT" } else { "STABLE" };
            let infra_state = Hypervector::encode_text_ngram(ping_status, 3);
            
            // D. Bind all fields to their semantic roles & majority bundle (World State Hypervector)
            let bound_market = market_state.bitwise_xor(&v_role_market);
            let bound_news = news_state.bitwise_xor(&v_role_news);
            let bound_infra = infra_state.bitwise_xor(&v_role_infra);
            
            let current_world_state = Hypervector::bundle(&[&bound_market, &bound_news, &bound_infra]);
            
            // Save to active world state
            {
                let mut ws_guard = world_state_subconscious.write().await;
                *ws_guard = current_world_state;
            }

            // E. Autonomy Drive / Dissonance Evaluation
            let historical_baseline = match ledger.load_records() {
                Ok(records) => {
                    if let Some((_date, last_vec)) = records.last() {
                        *last_vec
                    } else {
                        normal_state // Fallback to equilibrium state
                    }
                }
                Err(_) => normal_state,
            };

            let drive = AutonomyDrive::new(0.44); // Dissonance threshold
            let dissonance = AutonomyDrive::calculate_dissonance(&current_world_state, &historical_baseline);
            let should_pivot = drive.evaluates_necessity_to_pivot(&dissonance);

            let crisis_sim = 1.0 - current_world_state.normalized_hamming_distance(&crisis_memory);
            
            {
                let mut pain_guard = phantom_pain_subconscious.write().await;
                *pain_guard = crisis_sim;
            }

            if should_pivot {
                let mut drive_guard = active_drive_subconscious.write().await;
                *drive_guard = "Subconscious (Dissonance Pivot)".to_string();

                let mut intent_guard = intent_subconscious.write().await;
                *intent_guard = dissonance;

                let dist = dissonance.normalized_hamming_distance(&Hypervector::new_zero());
                let _ = subconscious_log_tx.send(format!(
                    "SUB-DRIVE: Cognitive dissonance detected (d_H: {:.4}). Pivot active intent to discrepancy vector.",
                    dist
                ));
            } else if crisis_sim > 0.55 {
                // System detects drift to crisis signature: MATHEMATICAL PHANTOM PAIN
                let mut drive_guard = active_drive_subconscious.write().await;
                *drive_guard = "Subconscious (Phantom Pain Intervention)".to_string();
                
                // Extract anomalous features using VSA subtraction (XOR) into Investigative Intent Vector
                let anomaly_intent = current_world_state.bitwise_xor(&crisis_memory);
                
                let mut intent_guard = intent_subconscious.write().await;
                *intent_guard = anomaly_intent;
                
                let _ = subconscious_log_tx.send(format!(
                    "SUB-DRIVE: Mathematical phantom pain high ({:.2}%). Steering intent to anomaly difference vector.",
                    crisis_sim * 100.0
                ));
            } else {
                let mut drive_guard = active_drive_subconscious.write().await;
                *drive_guard = "Autonomous / Idle Search".to_string();
            }

            // Daily ledger consensus consolidation simulation: consolidate daily memories at midnight
            // For demo, we append to long term ledger every 30 seconds
            if ticker % 15 == 0 {
                let today_str = Utc::now().format("%Y-%m-%d").to_string();
                if let Err(e) = ledger.append_record(&today_str, &current_world_state) {
                    let _ = subconscious_log_tx.send(format!("LEDGER ERROR: {}", e));
                } else {
                    let _ = subconscious_log_tx.send("LEDGER: Successfully saved daily consolidated world vector.".to_string());
                }
            }
        }
    });

    // 8. Capture logs channel in background to populate shared logs
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

    // 9. Cyberpunk HUD TUI Dashboard (CLI Console Render)
    println!("\x1B[2J\x1B[1;1H"); // clear screen
    loop {
        sleep(Duration::from_millis(150)).await;
        
        let url = shared_current_url.read().await;
        let metrics = shared_metrics.read().await;
        let logs = shared_logs.read().await;
        let pain = shared_phantom_pain.read().await;
        let drive = shared_active_drive.read().await;
        let intent = active_intent.read().await;
        let world = active_world_state.read().await;

        let intent_ones = intent.count_ones();
        let world_ones = world.count_ones();

        // Standard ANSI Escape commands to print HUD cleanly without flicker
        print!("\x1B[H"); // Cursor to home (top left)
        
        println!("\x1B[35m┌─────────────────────────────────────────────────────────────────────────────┐\x1B[0m\x1B[K");
        println!("\x1B[35m│   \x1B[1;36mTHE MACHINE v8.0\x1B[0;35m  |  \x1B[1;32mGENERAL COGNITIVE CORE\x1B[0;35m  |  \x1B[1;33mHAROLD FINCH INTERFACE\x1B[0;35m     │\x1B[0m\x1B[K");
        println!("\x1B[35m├─────────────────────────────────────────────────────────────────────────────┤\x1B[0m\x1B[K");
        
        // Modal Sensory Ingestion Grid
        println!(
            "\x1B[35m│\x1B[36m [SENSORY METRICS GRID]\x1B[0m                                                      \x1B[35m│\x1B[0m\x1B[K"
        );
        println!(
            "\x1B[35m│\x1B[0m  VIX Z-score: \x1B[32m{:+.2}\x1B[0m | MOVE Z-score: \x1B[32m{:+.2}\x1B[0m | 10Y-2Y Slope: \x1B[32m{:+.2}\x1B[0m           \x1B[35m│\x1B[0m\x1B[K",
            metrics.get("vix_zscore").unwrap_or(&0.0),
            metrics.get("move_zscore").unwrap_or(&0.0),
            metrics.get("slope_zscore").unwrap_or(&0.0)
        );
        println!(
            "\x1B[35m│\x1B[0m  Level Z:     \x1B[32m{:+.2}\x1B[0m | Curvature Z:  \x1B[32m{:+.2}\x1B[0m | Network Status: \x1B[33m{:<12}\x1B[0m      \x1B[35m│\x1B[0m\x1B[K",
            metrics.get("level_zscore").unwrap_or(&0.0),
            metrics.get("curvature_zscore").unwrap_or(&0.0),
            if *pain > 0.55 { "OUTAGE THREAT" } else { "STABLE" }
        );
        
        // VSA State Hypervectors
        println!("\x1B[35m├─────────────────────────────────────────────────────────────────────────────┤\x1B[0m\x1B[K");
        println!(
            "\x1B[35m│\x1B[36m [NEURO-SYMBOLIC HYPERSPACE STATE]\x1B[0m                                           \x1B[35m│\x1B[0m\x1B[K"
        );
        println!(
            "\x1B[35m│\x1B[0m  World State Vector  ($H_{{world}}$):  \x1B[32m[{:5} / 10048 bits set]\x1B[0m                    \x1B[35m│\x1B[0m\x1B[K",
            world_ones
        );
        println!(
            "\x1B[35m│\x1B[0m  Active Intent Vector ($H_{{intent}}$): \x1B[32m[{:5} / 10048 bits set]\x1B[0m                    \x1B[35m│\x1B[0m\x1B[K",
            intent_ones
        );
        
        // Cybernetic Loops & Cognitive Drives
        println!("\x1B[35m├─────────────────────────────────────────────────────────────────────────────┤\x1B[0m\x1B[K");
        println!(
            "\x1B[35m│\x1B[36m [CYBERNETIC COGNITIVE DRIVE STATUS]\x1B[0m                                         \x1B[35m│\x1B[0m\x1B[K"
        );
        println!(
            "\x1B[35m│\x1B[0m  Active Drive Layer: \x1B[1;36m{:<47}\x1B[0m     \x1B[35m│\x1B[0m\x1B[K",
            drive
        );
        println!(
            "\x1B[35m│\x1B[0m  Drift Signature to Lehman Crisis: \x1B[1;31m{:.2}%\x1B[0m (Phantom Pain threshold: 55.0%)  \x1B[35m│\x1B[0m\x1B[K",
            *pain * 100.0
        );
        
        // Asynchronous Forager Crawler
        println!("\x1B[35m├─────────────────────────────────────────────────────────────────────────────┤\x1B[0m\x1B[K");
        println!(
            "\x1B[35m│\x1B[36m [AUTONAVIGATION CRAWLER LOOP]\x1B[0m                                               \x1B[35m│\x1B[0m\x1B[K"
        );
        println!(
            "\x1B[35m│\x1B[0m  Scraping URL: \x1B[1;33m{:<60}\x1B[0m \x1B[35m│\x1B[0m\x1B[K",
            if url.len() > 60 { format!("{}...", &url[0..57]) } else { url.clone() }
        );
        
        // Admin overrides info
        println!("\x1B[35m├─────────────────────────────────────────────────────────────────────────────┤\x1B[0m\x1B[K");
        println!(
            "\x1B[35m│\x1B[36m [EXOGENOUS SOCKET OVERRIDE]\x1B[0m                                                 \x1B[35m│\x1B[0m\x1B[K"
        );
        println!(
            "\x1B[35m│\x1B[0m  TCP Socket Node: \x1B[32mtcp://127.0.0.1:9000\x1B[0m (Send override strings here)         \x1B[35m│\x1B[0m\x1B[K"
        );
        
        // Core Console Logs
        println!("\x1B[35m├─────────────────────────────────────────────────────────────────────────────┤\x1B[0m");
        println!(
            "\x1B[35m│\x1B[36m [SYSTEM LOGS]\x1B[0m                                                               \x1B[35m│\x1B[0m"
        );
        for i in 0..7 {
            if let Some(log) = logs.get(logs.len().saturating_sub(7) + i) {
                println!("\x1B[35m│\x1B[0m  \x1B[90m{:<73}\x1B[0m \x1B[35m│\x1B[0m", log);
            } else {
                println!("\x1B[35m│\x1B[0m                                                                             \x1B[35m│\x1B[0m");
            }
        }
        println!("\x1B[35m└─────────────────────────────────────────────────────────────────────────────┘\x1B[0m");
    }
}

// Thread-safe helper extensions for shared states
