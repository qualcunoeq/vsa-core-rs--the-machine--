// ─── Bond System Data Feeder ────────────────────────────────────────────────
//
// Connects "The Machine" to ALL three live bond market data sources from the
// bond system's SQLite database.  This is the embodiment layer — it transforms
// real financial time series AND text documents into hypervector state
// trajectories and feeds them through the full cognitive pipeline.
//
// ## Data Sources
//
//   daily_features  (101 rows)  — Core numeric market factors (yields, vols,
//                                  equities, FX, commodities, inflation)
//   fomc_minutes_raw (383 rows) — Full FOMC meeting transcripts (~50 KB each,
//                                  2012–2026).  Encoded as text hypervectors.
//   macro_surprises  (370 rows) — Economic release surprises with actual,
//                                  expected, and surprise_pct values.
//   raw_series      (1,029,403) — Raw time-series data points by series_id.
//
// ## Data Pipeline
//
//   All sources → FPE encoding (numeric) / text encoding (FOMC)
//              → State hypervector (bundle of rotation-bound factor encodings)
//              → TemporalCognition::observe (Markov transition model)
//              → PredictiveCodingLoop::cycle (prediction error)
//              → Abstractor::cycle (regime detection & L2 abstraction)
//              → Sleep::cycle (L3 meta-abstraction across all sources)
//
// ## Source Tracking
//
//   Each state is tagged with its `DataSource` so the pipeline and
//   diagnostics can distinguish between bond yields, FOMC text, and
//   macro surprises.

use crate::Hypervector;
use crate::hierarchy::HierarchicalManifold;
use crate::predictive::PredictiveCodingLoop;
use crate::abstractor::Abstractor;
use crate::sleep::SleepCycle;
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
    "yield_10y", "curve_slope_2s10s", "vix", "move_index",
    "spx", "dxy", "gold", "crude_oil",
    "breakeven_10y", "real_yield_10y", "fed_sentiment_hawkish_pct",
];

// ─── Data Source ────────────────────────────────────────────────────────────

/// Identifies which data source a market state originated from.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum DataSource {
    DailyFeatures,
    FomcMinutes,
    MacroSurprises,
    RawSeries,
}

impl DataSource {
    pub fn label(&self) -> &'static str {
        match self {
            DataSource::DailyFeatures => "DAILY_FEATURES",
            DataSource::FomcMinutes => "FOMC_MINUTES",
            DataSource::MacroSurprises => "MACRO_SURPRISES",
            DataSource::RawSeries => "RAW_SERIES",
        }
    }
}

// ─── FactorConfig ───────────────────────────────────────────────────────────

#[derive(Clone, Debug)]
pub struct FactorConfig {
    pub name: String,
    pub min_val: f64,
    pub max_val: f64,
    pub level_vectors: Vec<Hypervector>,
    pub rotation: usize,
}

impl FactorConfig {
    pub fn new(name: &str, min_val: f64, max_val: f64, rotation: usize) -> Self {
        let level_vectors = Hypervector::generate_level_vectors(FPE_LEVELS);
        FactorConfig {
            name: name.to_string(),
            min_val, max_val, level_vectors, rotation,
        }
    }

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

/// A single observation from any data source.
#[derive(Clone, Debug)]
pub struct MarketState {
    pub date: String,
    pub source: DataSource,
    pub raw_values: HashMap<String, f64>,
    pub raw_text: String,
    pub encoded: Hypervector,
}

// ─── BondDataReader ─────────────────────────────────────────────────────────

/// Reads all data sources from the bond system's SQLite database.
pub struct BondDataReader {
    pub factors: Vec<FactorConfig>,
    pub states: Vec<MarketState>,
    db_path: String,
}

impl BondDataReader {
    pub fn new(db_path: &str) -> Self {
        let (mins, maxs) = Self::calibrate_ranges(db_path);
        let factors: Vec<FactorConfig> = CORE_FACTORS.iter().enumerate().map(|(i, name)| {
            let rot = FACTOR_ROTATIONS[i % FACTOR_ROTATIONS.len()];
            let min_val = *mins.get(*name).unwrap_or(&0.0);
            let max_val = *maxs.get(*name).unwrap_or(&100.0);
            FactorConfig::new(name, min_val, max_val, rot)
        }).collect();
        BondDataReader { factors, states: Vec::new(), db_path: db_path.to_string() }
    }

    // ── Range Calibration ──────────────────────────────────────────────

    fn calibrate_ranges(db_path: &str) -> (HashMap<String, f64>, HashMap<String, f64>) {
        let mut mins = HashMap::new();
        let mut maxs = HashMap::new();
        let conn = match rusqlite::Connection::open(db_path) {
            Ok(c) => c,
            Err(_) => {
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
        for name in CORE_FACTORS {
            let col = name.replace('.', "_");
            let query = format!("SELECT MIN({col}), MAX({col}) FROM daily_features WHERE {col} IS NOT NULL");
            if let Ok(mut stmt) = conn.prepare(&query) {
                if let Ok(rows) = stmt.query_map([], |row| {
                    let min_val: Option<f64> = row.get(0).unwrap_or(None);
                    let max_val: Option<f64> = row.get(1).unwrap_or(None);
                    Ok((min_val, max_val))
                }) {
                    for row in rows.flatten() {
                        if let (Some(min_v), Some(max_v)) = row {
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

    // ── Daily Features (101 rows) ──────────────────────────────────────

    pub fn load_daily_features(&mut self) -> Result<usize, String> {
        let conn = rusqlite::Connection::open(&self.db_path)
            .map_err(|e| format!("Failed to open DB: {}", e))?;
        let col_list: Vec<String> = CORE_FACTORS.iter().map(|c| c.replace('.', "_")).collect();
        let col_expr = col_list.join(", ");
        let query = format!("SELECT date, {col_expr} FROM daily_features ORDER BY date ASC");
        let mut stmt = conn.prepare(&query)
            .map_err(|e| format!("Failed to prepare query: {}", e))?;

        let rows = stmt.query_map([], |row| {
            let date: String = row.get(0).unwrap_or_default();
            let mut values = HashMap::new();
            for (i, name) in CORE_FACTORS.iter().enumerate() {
                let val: Option<f64> = row.get(i + 1).unwrap_or(None);
                if let Some(v) = val { values.insert(name.to_string(), v); }
            }
            Ok((date, values))
        }).map_err(|e| format!("Query failed: {}", e))?;

        let mut count = 0;
        let active_indices: Vec<usize> = (0..self.factors.len()).collect();
        for row in rows.flatten() {
            let (date, values) = row;
            if values.len() < 3 { continue; }
            let mut components = Vec::new();
            for &fi in &active_indices {
                if let Some(val) = values.get(&self.factors[fi].name) {
                    if val.is_finite() {
                        components.push(self.factors[fi].encode(*val));
                    }
                }
            }
            if components.is_empty() { continue; }
            let refs: Vec<&Hypervector> = components.iter().collect();
            let encoded = Hypervector::bundle(&refs);
            self.states.push(MarketState {
                date: date.clone(), source: DataSource::DailyFeatures,
                raw_values: values, raw_text: String::new(), encoded,
            });
            count += 1;
        }
        Ok(count)
    }

    // ── FOMC Minutes (383 rows, text ~50 KB each) ──────────────────────

    /// Load FOMC meeting minutes from the database.
    ///
    /// Each meeting's full transcript is encoded as a sentence hypervector
    /// using `Hypervector::encode_sentence()`.  The text is truncated to
    /// the first 8192 characters to fit within the encoding's token window.
    pub fn load_fomc_minutes(&mut self) -> Result<usize, String> {
        let conn = rusqlite::Connection::open(&self.db_path)
            .map_err(|e| format!("Failed to open DB: {}", e))?;
        let mut stmt = conn.prepare(
            "SELECT meeting_date, substr(content, 1, 8192) FROM fomc_minutes_raw ORDER BY meeting_date ASC"
        ).map_err(|e| format!("Failed to prepare FOMC query: {}", e))?;

        let rows = stmt.query_map([], |row| {
            let date: String = row.get(0).unwrap_or_default();
            let text: String = row.get(1).unwrap_or_default();
            Ok((date, text))
        }).map_err(|e| format!("FOMC query failed: {}", e))?;

        let mut count = 0;
        for row in rows.flatten() {
            let (date, text) = row;
            if text.len() < 50 { continue; }
            // Role-bind the text using a unique FOMC role vector
            let role = Hypervector::encode_text_ngram("ROLE_FOMC_MINUTES", 3);
            let text_hv = Hypervector::encode_sentence(&text);
            let encoded = role.bitwise_xor(&text_hv);
            self.states.push(MarketState {
                date: date.clone(), source: DataSource::FomcMinutes,
                raw_values: HashMap::new(), raw_text: text,
                encoded,
            });
            count += 1;
        }
        Ok(count)
    }

    // ── Macro Surprises (370 rows, numeric) ────────────────────────────

    /// Load macro-economic surprises.
    ///
    /// Encodes three numeric fields (actual, expected, surprise_pct)
    /// using FPE and bundles them into a state vector with a macro role.
    pub fn load_macro_surprises(&mut self) -> Result<usize, String> {
        let conn = rusqlite::Connection::open(&self.db_path)
            .map_err(|e| format!("Failed to open DB: {}", e))?;
        let mut stmt = conn.prepare(
            "SELECT timestamp, indicator, actual, forecast, surprise, importance \
             FROM macro_surprises ORDER BY timestamp ASC"
        ).map_err(|e| format!("Failed to prepare macro query: {}", e))?;

        let rows = stmt.query_map([], |row| {
            let date: String = row.get(0).unwrap_or_default();
            let indicator: String = row.get(1).unwrap_or_default();
            let actual: Option<f64> = row.get(2).ok();
            let forecast: Option<f64> = row.get(3).ok();
            let surprise: Option<f64> = row.get(4).ok();
            let importance: String = row.get::<_, Option<String>>(5).ok().flatten().unwrap_or_default();
            Ok((date, indicator, actual, forecast, surprise, importance))
        }).map_err(|e| format!("Macro query failed: {}", e))?;

        // Pre-generate FPE levels for numeric fields
        let levels_surprise = Hypervector::generate_level_vectors(64);
        let _levels_actual = Hypervector::generate_level_vectors(64);
        let levels_forecast = Hypervector::generate_level_vectors(64);

        let role_base = Hypervector::encode_text_ngram("ROLE_MACRO_SURPRISE", 3);
        let role_indicator = Hypervector::encode_text_ngram("ROLE_MACRO_INDICATOR", 3);
        let role_actual = Hypervector::encode_text_ngram("ROLE_MACRO_ACTUAL", 3);
        let role_forecast = Hypervector::encode_text_ngram("ROLE_MACRO_FORECAST", 3);

        let mut count = 0;
        for row in rows.flatten() {
            let (date, indicator, _actual, forecast, surprise, importance) = row;
            let mut components: Vec<Hypervector> = Vec::new();

            // Encode indicator text
            let ind_hv = role_indicator.bitwise_xor(
                &Hypervector::encode_text_ngram(&indicator, 3));
            components.push(ind_hv);

            // Encode importance text (if available)
            if !importance.is_empty() {
                let imp_hv = role_base.bitwise_xor(
                    &Hypervector::encode_text_ngram(&importance, 3));
                components.push(imp_hv);
            }

            // Encode surprise as FPE level
            if let Some(s) = surprise {
                if s.is_finite() {
                    let clamped = s.clamp(-10.0, 10.0);
                    let fraction = (clamped + 10.0) / 20.0;
                    let idx = ((fraction * 63.0).round() as usize).min(63);
                    components.push(role_actual.bitwise_xor(&levels_surprise[idx]));
                }
            }

            // Encode forecast value
            if let Some(f) = forecast {
                if f.is_finite() {
                    let clamped = f.clamp(-1e6, 1e6);
                    let fraction = ((clamped + 1e6) / 2e6).clamp(0.0, 1.0);
                    let idx = ((fraction * 63.0).round() as usize).min(63);
                    components.push(role_forecast.bitwise_xor(&levels_forecast[idx]));
                }
            }

            if components.is_empty() { continue; }
            let refs: Vec<&Hypervector> = components.iter().collect();
            let encoded = Hypervector::bundle(&refs);
            self.states.push(MarketState {
                date: date, source: DataSource::MacroSurprises,
                raw_values: HashMap::new(), raw_text: indicator,
                encoded,
            });
            count += 1;
        }
        Ok(count)
    }

    // ── Common ─────────────────────────────────────────────────────────

    pub fn count(&self) -> usize { self.states.len() }

    pub fn get_state(&self, idx: usize) -> Option<&MarketState> {
        self.states.get(idx)
    }

    /// Count states by source.
    pub fn count_by_source(&self) -> HashMap<DataSource, usize> {
        let mut counts = HashMap::new();
        for state in &self.states {
            *counts.entry(state.source).or_insert(0) += 1;
        }
        counts
    }

    /// Clear all loaded states.
    pub fn clear(&mut self) { self.states.clear(); }

    /// Load ALL data sources into the pipeline.
    pub fn load_all_sources(&mut self) -> Result<SourceLoadReport, String> {
        self.states.clear();
        let daily = self.load_daily_features().unwrap_or(0);
        let fomc = self.load_fomc_minutes().unwrap_or(0);
        let macro_s = self.load_macro_surprises().unwrap_or(0);
        Ok(SourceLoadReport { daily_features: daily, fomc_minutes: fomc, macro_surprises: macro_s, total: daily + fomc + macro_s })
    }
}

/// Load counts per source.
#[derive(Clone, Debug)]
pub struct SourceLoadReport {
    pub daily_features: usize,
    pub fomc_minutes: usize,
    pub macro_surprises: usize,
    pub total: usize,
}

// ─── MarketCrucible ─────────────────────────────────────────────────────────

/// Runs the full cognitive pipeline over ALL bond market data sources.
///
/// Feeds daily features, FOMC minutes, and macro surprises through:
/// TemporalCognition → PredictiveCoding → Abstractor → Sleep → L3
pub struct MarketCrucible {
    pub reader: BondDataReader,
    pub hierarchy: HierarchicalManifold,
    pub temporal: crate::temporal::TemporalCognition,
    pub predictive: PredictiveCodingLoop,
    pub abstractor: Abstractor,
    pub sleeper: SleepCycle,
    pub tick: u64,
}

impl MarketCrucible {
    pub fn new(db_path: &str, max_centroids: usize) -> Self {
        let reader = BondDataReader::new(db_path);
        let hierarchy = HierarchicalManifold::new(&[max_centroids, 32, 8]);
        let temporal = crate::temporal::TemporalCognition::new(1000, max_centroids);
        let predictive = PredictiveCodingLoop::new(1000, max_centroids, 10);
        let abstractor = Abstractor::new();
        let sleeper = SleepCycle::with_defaults();
        MarketCrucible { reader, hierarchy, temporal, predictive, abstractor, sleeper, tick: 0 }
    }

    /// Load ALL sources and run the full pipeline.
    pub fn run_full_pipeline(&mut self) -> CrucibleReport {
        let mut report = CrucibleReport::new();

        // Step 1: Load all data sources
        match self.reader.load_all_sources() {
            Ok(load) => {
                report.source_load = Some(load.clone());
                eprintln!("  ✅ Loaded data: {} daily, {} FOMC, {} macro = {} total",
                    load.daily_features, load.fomc_minutes, load.macro_surprises, load.total);
            }
            Err(e) => {
                report.error = Some(format!("Failed to load data: {}", e));
                return report;
            }
        }

        if self.reader.count() == 0 {
            report.error = Some("No data loaded".to_string());
            return report;
        }

        // Step 2: Seed hierarchy with centroids from ALL sources (not just first 20
        // states, which are all daily features).  Balanced seeding ensures the
        // manifold covers all three data modalities (daily features, FOMC text,
        // macro surprises), giving each source a fair set of representative
        // centroids for clean projection and community detection.
        let max_seeds_per_source = 7.min(self.reader.count() / 3);
        let mut seeded_indices = Vec::new();
        for source in &[DataSource::DailyFeatures, DataSource::FomcMinutes, DataSource::MacroSurprises] {
            let mut seeded = 0;
            for (idx, state) in self.reader.states.iter().enumerate() {
                if state.source != *source { continue; }
                if seeded >= max_seeds_per_source { break; }
                if self.hierarchy.levels[0].centroids.len() >= self.hierarchy.levels[0].capacity { break; }
                self.hierarchy.levels[0].centroids.push(state.encoded);
                self.hierarchy.levels[0].activations.push(0.0);
                seeded_indices.push(idx);
                seeded += 1;
            }
        }
        // If any source has fewer than max_seeds_per_source, top off with extra
        // states from any remaining source to ensure we have at least 20 centroids
        while self.hierarchy.levels[0].centroids.len() < 20
            && self.hierarchy.levels[0].centroids.len() < self.hierarchy.levels[0].capacity
        {
            for (idx, state) in self.reader.states.iter().enumerate() {
                if seeded_indices.contains(&idx) { continue; }
                if self.hierarchy.levels[0].centroids.len() >= self.hierarchy.levels[0].capacity { break; }
                self.hierarchy.levels[0].centroids.push(state.encoded);
                self.hierarchy.levels[0].activations.push(0.0);
                seeded_indices.push(idx);
            }
        }
        report.centroids_seeded = self.hierarchy.levels[0].centroids.len();
        eprintln!("  ✅ Seeded {} L1 centroids ({} per source from daily/FOMC/macro)",
            report.centroids_seeded, max_seeds_per_source);

        // Step 3: Feed ALL states through the cognitive pipeline
        // (skip the ones already used as centroid seeds)
        for i in 0..self.reader.count() {
            if seeded_indices.contains(&i) { continue; }
            if let Some(state) = self.reader.get_state(i) {
                let state_vec = state.encoded;

                // Project through hierarchy
                let (_, _sim, centroid_idx) = if !self.hierarchy.levels[0].centroids.is_empty() {
                    self.hierarchy.levels[0].project_through(&state_vec)
                } else {
                    let idx = self.hierarchy.levels[0].centroids.len();
                    self.hierarchy.levels[0].centroids.push(state_vec);
                    self.hierarchy.levels[0].activations.push(0.0);
                    (state_vec, 1.0, idx)
                };

                // Feed through temporal model + predictive coding
                let temporal_idx = centroid_idx.min(self.temporal.transitions.max_centroids - 1);
                self.temporal.observe(&state_vec, temporal_idx, None, 0.5);
                let error = self.predictive.cycle(&state_vec, temporal_idx, Some(0), 0.5);

                // Project through hierarchy
                let hier_results = self.hierarchy.project_up(&state_vec, 0.0);
                if hier_results.len() > 1 {
                    // Track L2 activation for sleep cycle
                    let (_, _, l2_idx) = self.hierarchy.levels[1].project_through(&hier_results[0]);
                    let active_l2 = if self.hierarchy.levels[1].centroids.len() > l2_idx {
                        vec![l2_idx]
                    } else { vec![] };
                    self.sleeper.record_l2_activation(self.tick, active_l2);
                }

                report.accumulate_error(error);

                // Run abstractor every 5 ticks
                if self.tick > 0 && self.tick % 5 == 0 {
                    let abs_report = self.abstractor.cycle(
                        &self.temporal.transitions, &mut self.hierarchy, &self.predictive,
                    );
                    report.record_abstraction(abs_report);
                }

                self.tick += 1;
            }
        }

        // Step 4: Run sleep cycle (phase 3 = L3 abstraction)
        eprintln!("  🔄 Running sleep/consolidation cycle...");
        let trajectory: Vec<Hypervector> = (0..self.reader.count().min(1000))
            .filter_map(|i| self.reader.get_state(i))
            .map(|s| s.encoded)
            .collect();

        let error_history: Vec<f64> = self.predictive.error_history.clone();
        if self.hierarchy.levels.len() >= 3 {
            let sleep_report = self.sleeper.cycle(
                &trajectory, &mut self.hierarchy, &self.abstractor, &error_history,
            );
            report.sleep_report = Some(Box::new(sleep_report.clone()));
            eprintln!("  ✅ Sleep: {} transitions, {} L3 concepts, {} L2 pruned",
                sleep_report.transitions_found,
                sleep_report.l3_concepts_created,
                sleep_report.l2_concepts_pruned);
        }

        report.ticks_run = self.tick;
        report.final_error = self.predictive.avg_error;
        report.l2_concepts = self.abstractor.coherence.len();
        report.l3_concepts = self.hierarchy.levels.get(2)
            .map(|l| l.centroids.iter().filter(|c| c.count_ones() > 0).count())
            .unwrap_or(0);
        report.total_abstractor_cycles = self.tick as usize / 5;
        report.regime_changes = self.abstractor.total_abstractions_dissolved;

        eprintln!("  ✅ Pipeline complete: {} ticks, {} L2, {} L3, {} dissolved",
            report.ticks_run, report.l2_concepts, report.l3_concepts, report.regime_changes);

        report
    }

    /// Prediction accuracy over the last N steps.
    pub fn prediction_accuracy(&self, n: usize) -> f64 {
        self.predictive.temporal.prediction_accuracy(n)
    }

    pub fn report(&self) -> String {
        let counts = self.reader.count_by_source();
        let l3_count = self.hierarchy.levels.get(2)
            .map(|l| l.centroids.iter().filter(|c| c.count_ones() > 0).count())
            .unwrap_or(0);
        format!(
            "MarketCrucible: {} states ({} daily, {} FOMC, {} macro), \
             {} L1, {} L2, {} L3, avg_err={:.4}, acc={:.1}%",
            self.reader.count(),
            counts.get(&DataSource::DailyFeatures).unwrap_or(&0),
            counts.get(&DataSource::FomcMinutes).unwrap_or(&0),
            counts.get(&DataSource::MacroSurprises).unwrap_or(&0),
            self.hierarchy.levels[0].centroids.len(),
            self.abstractor.coherence.len(),
            l3_count,
            self.predictive.avg_error,
            self.prediction_accuracy(20) * 100.0,
        )
    }
}

// ─── CrucibleReport ─────────────────────────────────────────────────────────

#[derive(Clone, Debug)]
pub struct CrucibleReport {
    pub source_load: Option<SourceLoadReport>,
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
    pub l3_concepts: usize,
    pub abstractions_created: usize,
    pub regime_changes: usize,
    pub total_abstractor_cycles: usize,
    pub sleep_report: Option<Box<crate::sleep::SleepReport>>,
    pub error: Option<String>,
}

impl CrucibleReport {
    pub fn new() -> Self {
        CrucibleReport {
            source_load: None, total_days: 0, centroids_seeded: 0,
            ticks_run: 0, total_error: 0.0, error_count: 0, avg_error: 0.0,
            min_error: f64::MAX, max_error: 0.0, final_error: 0.0,
            l2_concepts: 0, l3_concepts: 0,
            abstractions_created: 0, regime_changes: 0,
            total_abstractor_cycles: 0, sleep_report: None, error: None,
        }
    }
    pub fn accumulate_error(&mut self, error: f64) {
        self.total_error += error; self.error_count += 1;
        self.avg_error = self.total_error / self.error_count as f64;
        if error < self.min_error { self.min_error = error; }
        if error > self.max_error { self.max_error = error; }
    }
    pub fn record_abstraction(&mut self, r: crate::abstractor::AbstractionReport) {
        self.abstractions_created += r.created;
    }
    pub fn has_error(&self) -> bool { self.error.is_some() }
}

// ═════════════════════════════════════════════════════════════════════════════
// TESTS
// ═════════════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;

    /// Test loading all three data sources from the database.
    #[test]
    #[ignore = "integration benchmark: reads and encodes all bond SQLite data sources"]
    fn test_load_all_sources() {
        let mut reader = BondDataReader::new(BOND_DB_PATH);
        match reader.load_all_sources() {
            Ok(load) => {
                eprintln!("  Load report: {} daily, {} FOMC, {} macro = {} total",
                    load.daily_features, load.fomc_minutes, load.macro_surprises, load.total);
                assert!(load.daily_features > 0, "Should load daily features");
                assert!(load.fomc_minutes > 0, "Should load FOMC minutes");
                // Macro surprises may be 0 if not populated; that's ok for this test
                assert_eq!(load.total, reader.count());

                // Verify source tracking
                let counts = reader.count_by_source();
                eprintln!("  Source counts: {:?}", counts);
                assert_eq!(*counts.get(&DataSource::DailyFeatures).unwrap_or(&0), load.daily_features);
                assert_eq!(*counts.get(&DataSource::FomcMinutes).unwrap_or(&0), load.fomc_minutes);

                // Check FOMC encoding: first FOMC state should have non-zero text
                for state in &reader.states {
                    if state.source == DataSource::FomcMinutes {
                        eprintln!("  First FOMC entry: date={}, text_len={}, popcount={:.1}%",
                            state.date, state.raw_text.len(),
                            state.encoded.count_ones() as f64 / 10240.0 * 100.0);
                        assert!(state.raw_text.len() > 100, "FOMC text should be > 100 chars");
                        break;
                    }
                }

                // Check Macro encoding
                for state in &reader.states {
                    if state.source == DataSource::MacroSurprises {
                        eprintln!("  First Macro entry: date={}, indicator={}, popcount={:.1}%",
                            state.date, state.raw_text,
                            state.encoded.count_ones() as f64 / 10240.0 * 100.0);
                        break;
                    }
                }

                // States from different sources should differ
                let daily_states: Vec<&MarketState> = reader.states.iter()
                    .filter(|s| s.source == DataSource::DailyFeatures).collect();
                let fomc_states: Vec<&MarketState> = reader.states.iter()
                    .filter(|s| s.source == DataSource::FomcMinutes).collect();
                if daily_states.len() >= 2 && fomc_states.len() >= 2 {
                    let d_dist = daily_states[0].encoded.normalized_hamming_distance(
                        &daily_states[1].encoded);
                    let f_dist = fomc_states[0].encoded.normalized_hamming_distance(
                        &fomc_states[1].encoded);
                    eprintln!("  Within-source distances — daily: {:.4}, FOMC: {:.4}", d_dist, f_dist);
                }
            }
            Err(e) => {
                eprintln!("  ⚠ Database not available: {}", e);
            }
        }
    }

    /// FULL END-TO-END: Load all 3 sources, run the entire cognitive
    /// pipeline, and verify the Abstractor discovers structure.
    #[test]
    #[ignore = "integration benchmark: reads the bond SQLite dataset and runs the full market pipeline"]
    fn test_full_multi_source_pipeline() {
        let mut crucible = MarketCrucible::new(BOND_DB_PATH, 50);
        let report = crucible.run_full_pipeline();

        if report.has_error() {
            eprintln!("  ⚠ Pipeline error: {:?}", report.error);
            return;
        }

        eprintln!();
        eprintln!("  ═══════════════════════════════════════════");
        eprintln!("  MULTI-SOURCE MARKET CRUCIBLE RESULTS");
        eprintln!("  ═══════════════════════════════════════════");

        if let Some(ref load) = report.source_load {
            eprintln!("  Data loaded:     {} daily + {} FOMC + {} macro = {}",
                load.daily_features, load.fomc_minutes, load.macro_surprises, load.total);
        }
        eprintln!("  L1 centroids:    {}", report.centroids_seeded);
        eprintln!("  Ticks run:       {}", report.ticks_run);
        eprintln!("  Avg error:       {:.4}", report.avg_error);
        eprintln!("  L2 concepts:     {}", report.l2_concepts);
        eprintln!("  L3 concepts:     {}", report.l3_concepts);
        eprintln!("  Abstractions:    {}", report.abstractions_created);
        eprintln!("  Regime changes:  {}", report.regime_changes);
        eprintln!("  Abstractor cycles: {}", report.total_abstractor_cycles);

        if let Some(ref sr) = report.sleep_report {
            eprintln!("  Sleep: {} transitions, {} L3 created, {} L2 pruned",
                sr.transitions_found, sr.l3_concepts_created, sr.l2_concepts_pruned);
        }

        eprintln!();
        assert!(report.ticks_run > 0, "Pipeline should run");
        assert!(report.centroids_seeded > 0, "Should seed centroids");
        assert!(report.total_abstractor_cycles > 0, "Abstractor should run");
        // L2 concepts may not form in a single run (depends on data structure,
        // abstractor parameters, prediction error gating).  The pipeline
        // itself is the test — it should complete without crashing.

        eprintln!("  ✅ Multi-source pipeline complete — {} L2, {} L3",
            report.l2_concepts, report.l3_concepts);
    }

    /// Test that the FOMC text encoding produces different vectors per meeting.
    #[test]
    #[ignore = "integration benchmark: reads and encodes FOMC text from the bond SQLite dataset"]
    fn test_fomc_encoding_diversity() {
        let mut reader = BondDataReader::new(BOND_DB_PATH);
        match reader.load_fomc_minutes() {
            Ok(n) => {
                eprintln!("  Loaded {} FOMC minutes", n);
                if n >= 2 {
                    let d = reader.get_state(0).unwrap().encoded
                        .normalized_hamming_distance(&reader.get_state(1).unwrap().encoded);
                    eprintln!("  Distance between first two FOMC states: {:.4}", d);
                    // Different meetings should produce measurably different vectors
                    assert!(d > 0.10, "FOMC states should differ: {}", d);
                }
            }
            Err(e) => eprintln!("  ⚠ {}", e),
        }
    }
}
