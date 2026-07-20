// ─── Bond Pipeline: Periodic Inference Runner ────────────────────────────────
//
// Standalone binary that loads ALL bond data sources from the SQLite database
// and runs the full cognitive pipeline (Temporal → Predictive → Abstractor →
// Sleep → L3).  Designed to be invoked periodically via cron or systemd timer.
//
// ## Usage
//
//   cargo run --bin bond_pipeline --release          # run once
//   cargo run --bin bond_pipeline --release -- --warm # skip pipeline, just load
//   cargo run --bin bond_pipeline --release -- --inspect  # verbose debug output
//
// ## Exit Codes
//
//   0 — success (pipeline completed)
//   1 — data load failure
//   2 — pipeline runtime error
//
// ## Cron Setup (every hour)
//
//   crontab -e
//   0 * * * * cd /home/shiba/the-machine && \
//     cargo run --bin bond_pipeline --release 2>&1 >> /tmp/bond_pipeline.log
//
// For faster startup (no compilation on each run), build the binary once:
//
//   cargo build --release --bin bond_pipeline
//   0 * * * * /home/shiba/the-machine/target/release/bond_pipeline \
//     2>&1 >> /tmp/bond_pipeline.log
//
// The pipeline updates `/tmp/bond_pipeline_status.json` on each run with
// the latest L2/L3 concept counts, average error, and sleep report.

use std::collections::HashMap;
use std::time::Instant;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let warm_only = args.contains(&"--warm".to_string());
    let inspect = args.contains(&"--inspect".to_string());

    let start = Instant::now();

    // ── Step 1: Load ALL data sources ──────────────────────────────────
    println!("[bond_pipeline] Loading bond data sources...");
    let db_path = the_machine::bond_feeder::BOND_DB_PATH;
    let mut crucible = the_machine::bond_feeder::MarketCrucible::new(db_path, 50);

    let load_start = Instant::now();
    let load = match crucible.reader.load_all_sources() {
        Ok(report) => {
            println!(
                "  Loaded: {} daily + {} FOMC + {} macro = {} total  [{:?}]",
                report.daily_features,
                report.fomc_minutes,
                report.macro_surprises,
                report.total,
                load_start.elapsed()
            );
            // Verify source tracking
            let counts = crucible.reader.count_by_source();
            println!("  Source counts: {:?}", counts);
            report
        }
        Err(e) => {
            eprintln!("[bond_pipeline] FATAL: Failed to load data: {}", e);
            std::process::exit(1);
        }
    };

    if load.total == 0 {
        eprintln!("[bond_pipeline] FATAL: No data loaded from any source");
        std::process::exit(1);
    }

    // ── Step 2: Diagnostic info (--inspect flag) ──────────────────────
    if inspect {
        println!();
        println!("  ── Encoding Diagnostics ──");
        let mut source_samples: HashMap<&str, usize> = HashMap::new();
        for state in &crucible.reader.states {
            let label = state.source.label();
            let entry = source_samples.entry(label).or_insert(0);
            if *entry < 2 {
                let pop = state.encoded.count_ones() as f64 / 10240.0 * 100.0;
                println!(
                    "    {}[{}]: date={}, popcount={:.1}%",
                    label, *entry, state.date, pop
                );
                *entry += 1;
            }
        }

        if crucible.reader.states.len() >= 2 {
            let d = crucible
                .reader
                .get_state(0)
                .unwrap()
                .encoded
                .normalized_hamming_distance(&crucible.reader.get_state(1).unwrap().encoded);
            println!("  Distance between first two states: {:.4}", d);
        }
    }

    // ── Step 3: Run the cognitive pipeline ─────────────────────────────
    if warm_only {
        println!("[bond_pipeline] --warm mode: data loaded, pipeline skipped.");
        println!(
            "  Summary: {} states across {} sources",
            crucible.reader.count(),
            crucible.reader.count_by_source().len()
        );
        std::process::exit(0);
    }

    println!();
    println!("[bond_pipeline] Running cognitive pipeline...");
    let pipeline_start = Instant::now();
    crucible.run_full_pipeline();
    let pipeline_elapsed = pipeline_start.elapsed();

    // ── Step 4: Collect results ────────────────────────────────────────
    let total_elapsed = start.elapsed();
    let counts = crucible.reader.count_by_source();
    let l2_count = crucible.abstractor.coherence.len();
    let l3_count = crucible
        .hierarchy
        .levels
        .get(2)
        .map(|l| l.centroids.iter().filter(|c| c.count_ones() > 0).count())
        .unwrap_or(0);

    println!();
    println!("  ═══════════════════════════════════════════");
    println!("  BOND PIPELINE RESULTS");
    println!("  ═══════════════════════════════════════════");
    println!(
        "  Total states:   {} ({} daily, {} FOMC, {} macro)",
        crucible.reader.count(),
        counts
            .get(&the_machine::bond_feeder::DataSource::DailyFeatures)
            .unwrap_or(&0),
        counts
            .get(&the_machine::bond_feeder::DataSource::FomcMinutes)
            .unwrap_or(&0),
        counts
            .get(&the_machine::bond_feeder::DataSource::MacroSurprises)
            .unwrap_or(&0)
    );
    println!(
        "  L1 centroids:   {}",
        crucible.hierarchy.levels[0].centroids.len()
    );
    println!("  Avg error:      {:.4}", crucible.predictive.avg_error);
    println!(
        "  Prediction acc: {:.1}%",
        crucible.prediction_accuracy(20) * 100.0
    );
    println!("  L2 concepts:    {}", l2_count);
    println!("  L3 concepts:    {}", l3_count);
    println!(
        "  Abstractions:   {}",
        crucible.abstractor.total_abstractions_created
    );
    println!(
        "  Dissolved:      {}",
        crucible.abstractor.total_abstractions_dissolved
    );
    println!("  Ticks:          {}", crucible.tick);
    println!("  Pipeline time:  {:?}", pipeline_elapsed);
    println!("  Total time:     {:?}", total_elapsed);
    println!("  ═══════════════════════════════════════════");

    // ── Step 5: Write status file for monitoring ──────────────────────
    let status = serde_json::json!({
        "timestamp": chrono::Utc::now().to_rfc3339(),
        "pipeline_time_ms": pipeline_elapsed.as_millis(),
        "total_time_ms": total_elapsed.as_millis(),
        "states": {
            "total": crucible.reader.count(),
            "daily_features": counts.get(&the_machine::bond_feeder::DataSource::DailyFeatures).unwrap_or(&0),
            "fomc_minutes": counts.get(&the_machine::bond_feeder::DataSource::FomcMinutes).unwrap_or(&0),
            "macro_surprises": counts.get(&the_machine::bond_feeder::DataSource::MacroSurprises).unwrap_or(&0),
        },
        "pipeline": {
            "l1_centroids": crucible.hierarchy.levels[0].centroids.len(),
            "l2_concepts": l2_count,
            "l3_concepts": l3_count,
            "abstractions_created": crucible.abstractor.total_abstractions_created,
            "abstractions_dissolved": crucible.abstractor.total_abstractions_dissolved,
            "avg_error": crucible.predictive.avg_error,
            "prediction_accuracy": crucible.prediction_accuracy(20),
            "ticks": crucible.tick,
            "gated": crucible.abstractor.gated,
            "abstractor_cycles": crucible.abstractor.tick,
        },
        "exit_code": 0,
    });

    let status_str = serde_json::to_string_pretty(&status).unwrap_or_else(|_| "{}".to_string());
    match std::fs::write("/tmp/bond_pipeline_status.json", &status_str) {
        Ok(_) => println!("[bond_pipeline] Status written to /tmp/bond_pipeline_status.json"),
        Err(e) => eprintln!("[bond_pipeline] WARN: Could not write status file: {}", e),
    }

    println!("[bond_pipeline] Pipeline complete. ✓");
}
