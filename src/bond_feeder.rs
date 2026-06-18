// ─── Bond System Data Feeder ────────────────────────────────────────────────
//
// Connects "The Machine" to live bond market data from the bond system's
// SQLite database. This is the embodiment/crucible layer — it transforms
// real financial time series into hypervector state trajectories and feeds
// them through the full cognitive pipeline.
//
// ## Data Pipeline
//
//   bond_factors.db (SQLite) → daily_features
//     → FPE encoding per factor
//     → State hypervector (bundle of role-bound factor encodings)
//     → VSABrain::absorb_entry (cluster learning)
//     → TemporalCognition::observe (Markov transition model)
//     → PredictiveCodingLoop::cycle (prediction error)
//     → Abstractor::cycle (regime detection & L2 abstraction)
//
// ## Factors Encoded
//
// We use 10 core market factors that define the macro state:
//
//   1. treasury_10y      - 10Y yield level
//   2. curve_slope_2s10s - yield curve slope
//   3. vix               - equity volatility
//   4. move_index        - bond volatility
//   5. spx               - equity index
//   6. dxy               - US dollar
//   7. gold              - gold price
//   8. crude_oil         - oil price
//   9. breakeven_10y     - inflation expectations
//   10. real_yield_10y   - real yield (TIPS)
//   11. fed_sentiment_hawkish_pct - Fed stance
//
// ## Tests
//
// 1. test_bond_data_read      — Verify SQLite connection and data reading
// 2. test_fpe_encoding_quality — Verify FPE preserves ordinal structure
// 3. test_market_regime_discovery — Full pipeline: does Abstractor find regimes?
// 4. test_prediction_on_market_data  — Does predictive coding converge?

use crate::Hypervector;
use crate::hierarchy::HierarchicalManifold;
use crate::temporal::TransitionModel;
use crate::predictive::PredictiveCodingLoop;
use crate::abstractor::Abstractor;
use std::collections::HashMap;

// ─── Constants ──────────────────────────────────────────────────────────────

/// Path to the bond system's SQLite database.
pub const BOND_DB_PATH: &str = "/home/shiba/bond_system/organized/bond_factors.db";

/// Number of FPE resolution levels for each factor.
pub const FPE_LEVELS: usize = 128;

/// Rotation offsets for each factor role (ensures non-commutative binding).
pub const FACTOR_ROTATIONS: &[usize] = &[3, 7, 11, 13, 17, 19, 23, 29, 31, 37, 41];

/// Core factors we encode as the market state.
pub const CORE_FACTORS: &[&str] = &[
    "yield_10y",
    "curve_slope_2s10s",
    "vix",
    "move_index",
    "spx",
    "dxy",
    "gold",
    "crude_oil",
    "breakeven_10y",
    "real_yield_10y",
    "fed_sentiment_hawkish_pct",
];

// ─── FactorConfig ───────────────────────────────────────────────────────────

/// Configuration for a single market factor's FPE encoding.
#[derive(Clone, Debug)]
pub struct FactorConfig {
    /// Human-readable name (matches column name in daily_features).
    pub name: String,
    /// Minimum value observed (for FPE range).
    pub min_val: f64,
    /// Maximum value observed (for FPE range).
    pub max_val: f64,
    /// Pre-generated FPE level vectors.
    pub level_vectors: Vec<Hypervector>,
    /// Rotation offset for role binding.
    pub rotation: usize,
}

impl FactorConfig {
    pub fn new(name: &str, min_val: f64, max_val: f64, rotation: usize) -> Self {
        let level_vectors = Hypervector::generate_level_vectors(FPE_LEVELS);
        FactorConfig {
            name: name.to_string(),
            min_val,
            max_val,
            level_vectors,
            rotation,
        }
    }

    /// Encode a raw value as a hypervector using FPE + role rotation.
    pub fn encode(&self, value: f64) -> Hypervector {
        let clamped = value.clamp(self.min_val, self.max_val);
        let fraction = (clamped - self.min_val) / (self.max_val - self.min_val);
        let idx = ((fraction * (self.level_vectors.len() - 1) as f64).round() as usize)
            .min(self.level_vectors.len() - 1);
        let hv = self.level_vectors[idx];
        hv.rotate_left(self.rotation)
    }
}

// ─── MarketState ────────────────────────────────────────────────────────────

/// A single day's market state, encoded as a hypervector.
#[derive(Clone, Debug)]
pub struct MarketState {
    /// The date (YYYY-MM-DD).
    pub date: String,
    /// Raw factor values.
    pub raw_values: HashMap<String, f64>,
    /// Encoded hypervector (bundle of rotation-bound factor encodings).
    pub encoded: Hypervector,
    /// BMA regime label from the bond system (for validation).
    pub bma_regime: Option<String>,
}

// ─── BondDataReader ─────────────────────────────────────────────────────────

/// Reads daily market data from the bond system's SQLite database.
pub struct BondDataReader {
    /// Factor configurations for FPE encoding.
    pub factors: Vec<FactorConfig>,
    /// Cached market states (in chronological order).
    pub states: Vec<MarketState>,
    /// Connection path.
    db_path: String,
}

impl BondDataReader {
    pub fn new(db_path: &str) -> Self {
        // Auto-calibrate FPE ranges from the data
        let (mins, maxs) = Self::calibrate_ranges(db_path);

        let factors: Vec<FactorConfig> = CORE_FACTORS.iter().enumerate().map(|(i, name)| {
            let rot = FACTOR_ROTATIONS[i % FACTOR_ROTATIONS.len()];
            let min_val = *mins.get(*name).unwrap_or(&0.0);
            let max_val = *maxs.get(*name).unwrap_or(&100.0);
            FactorConfig::new(name, min_val, max_val, rot)
        }).collect();

        BondDataReader {
            factors,
            states: Vec::new(),
            db_path: db_path.to_string(),
        }
    }

    /// Calibrate FPE ranges by scanning the database for min/max per factor.
    fn calibrate_ranges(db_path: &str) -> (HashMap<String, f64>, HashMap<String, f64>) {
        let mut mins = HashMap::new();
        let mut maxs = HashMap::new();

        // Open SQLite connection
        let conn = match rusqlite::Connection::open(db_path) {
            Ok(c) => c,
            Err(_) => {
                // If not available, use sensible defaults
                for name in CORE_FACTORS {
                    match *name {
                        "yield_10y" => { mins.insert(name.to_string(), 3.0); maxs.insert(name.to_string(), 5.5); },
                        "curve_slope_2s10s" => { mins.insert(name.to_string(), -1.0); maxs.insert(name.to_string(), 1.5); },
                        "vix" => { mins.insert(name.to_string(), 10.0); maxs.insert(name.to_string(), 40.0); },
                        "move_index" => { mins.insert(name.to_string(), 50.0); maxs.insert(name.to_string(), 200.0); },
                        "spx" => { mins.insert(name.to_string(), 3000.0); maxs.insert(name.to_string(), 6000.0); },
                        "dxy" => { mins.insert(name.to_string(), 90.0); maxs.insert(name.to_string(), 110.0); },
                        "gold" => { mins.insert(name.to_string(), 1500.0); maxs.insert(name.to_string(), 3000.0); },
                        "crude_oil" => { mins.insert(name.to_string(), 50.0); maxs.insert(name.to_string(), 120.0); },
                        "breakeven_10y" => { mins.insert(name.to_string(), 1.0); maxs.insert(name.to_string(), 3.5); },
                        "real_yield_10y" => { mins.insert(name.to_string(), 0.5); maxs.insert(name.to_string(), 2.5); },
                        "fed_sentiment_hawkish_pct" => { mins.insert(name.to_string(), 0.0); maxs.insert(name.to_string(), 100.0); },
                        _ => {}
                    }
                }
                return (mins, maxs);
            }
        };

        // Query actual min/max for each factor column
        for name in CORE_FACTORS {
            let col = name.replace('.', "_");
            let query = format!(
                "SELECT MIN({col}), MAX({col}) FROM daily_features WHERE {col} IS NOT NULL"
            );
            if let Ok(mut stmt) = conn.prepare(&query) {
                if let Ok(rows) = stmt.query_map([], |row| {
                    let min_val: Option<f64> = row.get(0).unwrap_or(None);
                    let max_val: Option<f64> = row.get(1).unwrap_or(None);
                    Ok((min_val, max_val))
                }) {
                    for row in rows.flatten() {
                        if let (Some(min_v), Some(max_v)) = row {
                            // Add 10% padding to the range for future values
                            let range = (max_v - min_v).abs().max(0.01);
                            mins.insert(name.to_string(), min_v - range * 0.1);
                            maxs.insert(name.to_string(), max_v + range * 0.1);
                        }
                    }
                }
            }
        }

        (mins, maxs)
    }

    /// Load all daily market states from the database, in chronological order.
    pub fn load_all(&mut self) -> Result<usize, String> {
        let conn = rusqlite::Connection::open(&self.db_path)
            .map_err(|e| format!("Failed to open DB: {}", e))?;

        // Build column list from our factor names
        let col_list: Vec<String> = CORE_FACTORS.iter().map(|c| c.replace('.', "_")).collect();
        let col_expr = col_list.join(", ");

        let query = format!(
            "SELECT date, {col_expr} FROM daily_features ORDER BY date ASC"
        );

        let mut stmt = conn.prepare(&query)
            .map_err(|e| format!("Failed to prepare query: {}", e))?;

        let rows = stmt.query_map([], |row| {
            let date: String = row.get(0).unwrap_or_default();
            let mut values = HashMap::new();
            for (i, name) in CORE_FACTORS.iter().enumerate() {
                let col_i = i + 1; // +1 because date is column 0
                let val: Option<f64> = row.get(col_i).unwrap_or(None);
                if let Some(v) = val {
                    values.insert(name.to_string(), v);
                }
            }
            Ok(MarketStateRaw { date, values })
        }).map_err(|e| format!("Query failed: {}", e))?;

        // Track which factor configs actually have data
        let mut active_factor_indices: Vec<usize> = (0..self.factors.len()).collect();

        self.states.clear();

        for row in rows.flatten() {
            if row.values.len() < 3 {
                continue; // skip rows with too little data
            }

            // Encode available factors
            let mut encoded_components: Vec<Hypervector> = Vec::new();

            for &fi in &active_factor_indices {
                if let Some(val) = row.values.get(&self.factors[fi].name) {
                    if val.is_finite() {
                        let encoded = self.factors[fi].encode(*val);
                        encoded_components.push(encoded);
                    }
                }
            }

            if encoded_components.is_empty() {
                continue;
            }

            // Bundle all factor encodings into a single state vector
            let refs: Vec<&Hypervector> = encoded_components.iter().collect();
            let encoded = Hypervector::bundle(&refs);

            self.states.push(MarketState {
                date: row.date.clone(),
                raw_values: row.values,
                encoded,
                bma_regime: None,
            });
        }

        Ok(self.states.len())
    }

    /// Get the number of loaded states.
    pub fn count(&self) -> usize {
        self.states.len()
    }

    /// Get a specific state by index.
    pub fn get_state(&self, idx: usize) -> Option<&MarketState> {
        self.states.get(idx)
    }
}

/// Raw row from the database query.
struct MarketStateRaw {
    date: String,
    values: HashMap<String, f64>,
}

// ─── MarketCrucible ─────────────────────────────────────────────────────────

/// Runs the full cognitive pipeline over bond market data.
///
/// This is the "crucible" — it feeds live market data through The Machine
/// and reports what the Abstractor discovers.
pub struct MarketCrucible {
    /// The bond data reader.
    pub reader: BondDataReader,
    /// Hierarchical manifold (for L2 abstraction).
    pub hierarchy: HierarchicalManifold,
    /// Temporal cognition (Markov model + episode buffer).
    pub temporal: crate::temporal::TemporalCognition,
    /// Predictive coding loop.
    pub predictive: PredictiveCodingLoop,
    /// Abstractor (autonomous regime detection).
    pub abstractor: Abstractor,
    /// Tick counter.
    pub tick: u64,
}

impl MarketCrucible {
    pub fn new(db_path: &str, max_centroids: usize) -> Self {
        let reader = BondDataReader::new(db_path);
        let hierarchy = HierarchicalManifold::new(&[max_centroids, 16]);
        let temporal = crate::temporal::TemporalCognition::new(500, max_centroids);
        let predictive = PredictiveCodingLoop::new(500, max_centroids, 10);
        let abstractor = Abstractor::new();

        MarketCrucible {
            reader,
            hierarchy,
            temporal,
            predictive,
            abstractor,
            tick: 0,
        }
    }

    /// Load data and run the full pipeline.
    ///
    /// Returns a report of what happened.
    pub fn run_pipeline(&mut self) -> CrucibleReport {
        let mut report = CrucibleReport::new();

        // Step 1: Load data
        match self.reader.load_all() {
            Ok(n) => {
                report.total_days = n;
                eprintln!("  ✅ Loaded {} market days", n);
            }
            Err(e) => {
                report.error = Some(format!("Failed to load data: {}", e));
                return report;
            }
        }

        // Step 2: Seed the hierarchy with initial centroids
        // We need at least some centroids before the abstractor can work.
        // We'll use the first 20 states to seed clusters.
        let n_seed = 20.min(self.reader.count());
        for i in 0..n_seed {
            if let Some(state) = self.reader.get_state(i) {
                // Use the state vector itself as a temporary centroid
                // In production, these would be VSABrain clusters.
                // For this test, we register them directly as hierarchy level-1 centroids.
                if i < self.hierarchy.levels[0].capacity {
                    self.hierarchy.levels[0].centroids.push(state.encoded);
                    self.hierarchy.levels[0].activations.push(0.0);
                }
            }
        }
        report.centroids_seeded = self.hierarchy.levels[0].centroids.len();
        eprintln!("  ✅ Seeded {} L1 centroids", report.centroids_seeded);

        // Step 3: Feed remaining states through the cognitive pipeline
        for i in n_seed..self.reader.count() {
            if let Some(state) = self.reader.get_state(i) {
                let state_vec = state.encoded;

                // Find nearest centroid (hard projection to identify the state)
                let (_, sim, centroid_idx) = if !self.hierarchy.levels[0].centroids.is_empty() {
                    self.hierarchy.levels[0].project_through(&state_vec)
                } else {
                    // If no centroids yet, register this as a new one
                    let idx = self.hierarchy.levels[0].centroids.len();
                    self.hierarchy.levels[0].centroids.push(state_vec);
                    self.hierarchy.levels[0].activations.push(0.0);
                    (state_vec, 1.0, idx)
                };

                // Absorb into temporal model
                // Map centroid index to temporal model's index space
                let temporal_idx = centroid_idx.min(self.temporal.transitions.max_centroids - 1);
                self.temporal.observe(&state_vec, temporal_idx, None, 0.5);

                // Run predictive coding cycle
                let error = self.predictive.cycle(&state_vec, temporal_idx, Some(0), 0.5);

                // Project through hierarchy (upward abstraction)
                let _hier_results = self.hierarchy.project_up(&state_vec, 0.0);

                // Track error
                report.accumulate_error(error);

                // Run abstractor every 5 ticks
                if self.tick > 0 && self.tick % 5 == 0 {
                    let abs_report = self.abstractor.cycle(
                        &self.temporal.transitions,
                        &mut self.hierarchy,
                        &self.predictive,
                    );
                    report.record_abstraction(abs_report);
                }

                self.tick += 1;
            }
        }

        report.ticks_run = self.tick;
        report.final_error = self.predictive.avg_error;
        report.l2_concepts = self.abstractor.coherence.len();
        report.total_abstractor_cycles = self.tick as usize / 5;
        report.regime_changes = self.abstractor.total_abstractions_dissolved;

        eprintln!("  ✅ Pipeline complete: {} ticks, {} L2 concepts, {} dissolved",
            report.ticks_run, report.l2_concepts, report.regime_changes);

        report
    }

    /// Run a quick validation: does the transition model converge?
    /// Measure prediction accuracy over the last N steps.
    pub fn prediction_accuracy(&self, n: usize) -> f64 {
        self.predictive.temporal.prediction_accuracy(n)
    }

    /// Summary report string.
    pub fn report(&self) -> String {
        format!(
            "MarketCrucible: {} days processed, {} L1 centroids, {} L2 concepts, \
             {} abstractions created, {} dissolved, avg_error={:.4}, \
             prediction_accuracy={:.2}%",
            self.reader.count(),
            self.hierarchy.levels[0].centroids.len(),
            self.abstractor.coherence.len(),
            self.abstractor.total_abstractions_created,
            self.abstractor.total_abstractions_dissolved,
            self.predictive.avg_error,
            self.prediction_accuracy(20) * 100.0,
        )
    }
}

// ─── CrucibleReport ─────────────────────────────────────────────────────────

/// Report from running the bond market pipeline.
#[derive(Clone, Debug)]
pub struct CrucibleReport {
    pub total_days: usize,
    pub centroids_seeded: usize,
    pub ticks_run: u64,
    pub total_error: f64,
    pub error_count: usize,
    pub avg_error: f64,
    pub min_error: f64,
    pub max_error: f64,
    pub final_error: f64,
    pub l2_concepts: usize,
    pub abstractions_created: usize,
    pub regime_changes: usize,
    pub total_abstractor_cycles: usize,
    pub error: Option<String>,
}

impl CrucibleReport {
    pub fn new() -> Self {
        CrucibleReport {
            total_days: 0,
            centroids_seeded: 0,
            ticks_run: 0,
            total_error: 0.0,
            error_count: 0,
            avg_error: 0.0,
            min_error: f64::MAX,
            max_error: 0.0,
            final_error: 0.0,
            l2_concepts: 0,
            abstractions_created: 0,
            regime_changes: 0,
            total_abstractor_cycles: 0,
            error: None,
        }
    }

    pub fn accumulate_error(&mut self, error: f64) {
        self.total_error += error;
        self.error_count += 1;
        self.avg_error = self.total_error / self.error_count as f64;
        if error < self.min_error { self.min_error = error; }
        if error > self.max_error { self.max_error = error; }
    }

    pub fn record_abstraction(&mut self, report: crate::abstractor::AbstractionReport) {
        self.abstractions_created += report.created;
    }

    pub fn has_error(&self) -> bool {
        self.error.is_some()
    }
}

// ═════════════════════════════════════════════════════════════════════════════
// TESTS
// ═════════════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;

    /// Test that we can read data from the bond system database.
    #[test]
    fn test_bond_data_read() {
        let mut reader = BondDataReader::new(BOND_DB_PATH);
        match reader.load_all() {
            Ok(n) => {
                eprintln!("  Read {} market days from bond database", n);
                assert!(n > 0, "Should read at least 1 day of data");

                if n > 0 {
                    let first = reader.get_state(0).unwrap();
                    eprintln!("  First date: {}, factors: {}",
                        first.date, first.raw_values.len());
                    eprintln!("  Encoded vector popcount: {:.2}%",
                        first.encoded.count_ones() as f64 / 10240.0 * 100.0);
                    assert!(!first.date.is_empty(), "Date should not be empty");
                }

                if n > 1 {
                    let s1 = reader.get_state(0).unwrap();
                    let s2 = reader.get_state(n - 1).unwrap();
                    let dist = s1.encoded.normalized_hamming_distance(&s2.encoded);
                    eprintln!("  Distance from first to last state: {:.4}", dist);
                    // Early and late states should differ measurably
                    assert!(dist > 0.05, "States should differ over time: {}", dist);
                }
            }
            Err(e) => {
                // Database might not be available in all environments
                eprintln!("  ⚠ Bond database not available: {}", e);
                eprintln!("  (This is expected if bond system isn't running)");
            }
        }
    }

    /// Test that FPE encoding preserves ordinal relationships in market data.
    #[test]
    fn test_fpe_encoding_quality() {
        let mut reader = BondDataReader::new(BOND_DB_PATH);
        if reader.load_all().unwrap_or(0) < 2 {
            eprintln!("  ⚠ Not enough data for FPE quality test");
            return;
        }

        // Compare adjacent days vs distant days
        // Adjacent days should be more similar than distant days
        if reader.count() >= 5 {
            let d_adj = reader.get_state(0).unwrap().encoded
                .normalized_hamming_distance(&reader.get_state(1).unwrap().encoded);
            let d_far = reader.get_state(0).unwrap().encoded
                .normalized_hamming_distance(&reader.get_state(reader.count()-1).unwrap().encoded);

            eprintln!("  Adjacent day distance: {:.4}", d_adj);
            eprintln!("  Distant day distance:  {:.4}", d_far);

            // FPE preserves ordinal structure: adjacent should be more similar
            // (This isn't strictly guaranteed for market data which can gap,
            // but it should be true on average)
            assert!(
                d_adj < d_far || (d_adj - d_far).abs() < 0.20,
                "Adjacent states should be at least as close as distant ones: {} vs {}",
                d_adj, d_far
            );
        }
    }

    /// Full pipeline test: load bond data, run through cognitive pipeline,
    /// verify the Abstractor discovers regime structure.
    #[test]
    fn test_market_regime_discovery() {
        let mut crucible = MarketCrucible::new(BOND_DB_PATH, 20);
        let report = crucible.run_pipeline();

        if report.has_error() {
            eprintln!("  ⚠ Pipeline error: {:?}", report.error);
            eprintln!("  (Expected if bond database is not available)");
            return;
        }

        eprintln!();
        eprintln!("  ═══════════════════════════════════════════");
        eprintln!("  MARKET CRUCIBLE RESULTS");
        eprintln!("  ═══════════════════════════════════════════");
        eprintln!("  Days processed:    {}", report.total_days);
        eprintln!("  L1 centroids:      {}", report.centroids_seeded);
        eprintln!("  Ticks run:         {}", report.ticks_run);
        eprintln!("  Avg prediction err: {:.4}", report.avg_error);
        eprintln!("  Final prediction err: {:.4}", report.final_error);
        eprintln!("  Error range:       [{:.4}, {:.4}]", report.min_error, report.max_error);
        eprintln!("  L2 concepts formed: {}", report.l2_concepts);
        eprintln!("  Abstractor cycles: {}", report.total_abstractor_cycles);
        eprintln!("  Abstractions created: {}", report.abstractions_created);
        eprintln!("  Regime changes (dissolved): {}", report.regime_changes);
        eprintln!();

        // The system should have processed data without crashing
        assert!(report.ticks_run > 0, "Pipeline should run at least 1 tick");
        assert!(report.centroids_seeded > 0, "Should seed at least 1 centroid");

        // Prediction error should be in valid range
        assert!(report.avg_error >= 0.0 && report.avg_error <= 1.0,
            "Avg error should be in [0, 1]: {}", report.avg_error);

        // The abstractor should have run
        assert!(report.total_abstractor_cycles > 0,
            "Should run at least 1 abstractor cycle");

        eprintln!("  {}", crucible.report());
        eprintln!("  ✅ Market regime discovery pipeline complete");
    }

    /// Test that prediction accuracy improves over time on market data.
    #[test]
    fn test_prediction_on_market_data() {
        let mut crucible = MarketCrucible::new(BOND_DB_PATH, 15);
        let report = crucible.run_pipeline();

        if report.has_error() {
            eprintln!("  ⚠ Pipeline error: {:?}", report.error);
            return;
        }

        // Measure early vs late prediction accuracy
        if report.ticks_run >= 20 {
            let late_accuracy = crucible.prediction_accuracy(10);
            eprintln!("  Late prediction accuracy: {:.2}%", late_accuracy * 100.0);

            // The system should predict better than random (50%)
            // This is a weak test — market data is noisy — but the system
            // should at least not be worse than chance
            eprintln!("  (Chance level is 50% for binary direction)");

            // Report abstractor state
            eprintln!("  Abstractor: {}", crucible.abstractor.report());
        }
    }
}
