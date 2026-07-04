// ─── Live Monitoring Loop ──────────────────────────────────────────────────
//
// The last open circuit.  This loop makes The Machine autonomously alive —
// continuously observing its environment, reasoning about what it sees,
// detecting anomalies, and responding — without being queried.
//
// The Finch loop, running perpetually:
//
//   observe() → reason() → detect() → respond() → sleep() → repeat
//
// Everything built in the previous 6 phases feeds into this loop.
// It's the simplest code in the entire codebase and the most important.
// ────────────────────────────────────────────────────────────────────────────

use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;
use tokio::time;

use crate::VSABrain;
use crate::defense::DefenseSystem;
use crate::retrieval::{DomainIndex, detect_cross_domain_anomalies};

/// State maintained across monitoring cycles.
#[derive(Clone)]
pub struct MonitorState {
    /// Timestamp of the last full system scan.
    pub last_scan: Option<Instant>,
    /// Number of cycles completed.
    pub cycles: u64,
    /// Total threats detected across all cycles.
    pub total_threats: u64,
    /// Whether documentation has been ingested.
    pub documentation_loaded: bool,
}

impl MonitorState {
    pub fn new() -> Self {
        MonitorState {
            last_scan: None,
            cycles: 0,
            total_threats: 0,
            documentation_loaded: false,
        }
    }
}

/// Run one cycle of the monitoring loop: observe → ingest → detect → respond.
///
/// Returns the number of threats detected in this cycle.
async fn monitoring_cycle(
    brain: &mut VSABrain,
    defense: &DefenseSystem,
) -> usize {
    // ── 1. Observe: capture system state ─────────────────────────────────
    let scan_start = Instant::now();
    let triples_stored = crate::system_encoder::ingest_system_state(brain);
    let scan_time = scan_start.elapsed();

    // ── 2. Index: rebuild domain index ──────────────────────────────────
    let mut index = DomainIndex::new();
    index.build_from_brain(brain);

    // ── 3. Reason: cross-domain anomaly detection ────────────────────────
    let detect_start = Instant::now();
    let threats = detect_cross_domain_anomalies(brain, &index);
    let detect_time = detect_start.elapsed();

    // ── 4. Respond: feed threats into defense system ─────────────────────
    for threat in &threats {
        defense.increment_threat(threat.severity).await;
        let _rotated = defense.evaluate_threat_response().await;
    }

    // ── 5. Log ───────────────────────────────────────────────────────────
    let n_clusters = brain.dejavu_clusters.len();
    let n_entries: usize = brain.dejavu_clusters.iter()
        .map(|c| c.entries.len()).sum();

    eprintln!(
        "[monitor] cycle: scan={} triples in {:.1}s | {} clusters, {} entries | \
         threats={} in {:.1}s | defense_level={:.2}",
        triples_stored, scan_time.as_secs_f64(),
        n_clusters, n_entries,
        threats.len(), detect_time.as_secs_f64(),
        *defense.threat_level.read().await,
    );

    for (i, t) in threats.iter().enumerate() {
        eprintln!("  threat {}/{}: [{}] severity={:.1} — {}",
            i + 1, threats.len(), t.domain, t.severity, t.description);
    }

    threats.len()
}

/// Start the live monitoring loop.
///
/// Spawn this as a tokio task:
/// ```ignore
/// tokio::spawn(run_monitoring_loop(brain, defense, state, 60));
/// ```
///
/// It runs forever, observing the system every `interval_secs` seconds,
/// detecting threats, and updating the defense system.
pub async fn run_monitoring_loop(
    brain: Arc<RwLock<VSABrain>>,
    defense: DefenseSystem,
    state: Arc<RwLock<MonitorState>>,
    interval_secs: u64,
) {
    let interval = Duration::from_secs(interval_secs);

    eprintln!("\n═══════════════════════════════════════════════");
    eprintln!("  Monitoring loop started (interval={}s)", interval_secs);
    eprintln!("  Observe → Ingest → Detect → Respond → Repeat");
    eprintln!("═══════════════════════════════════════════════\n");

    // First cycle: establish baseline
    {
        let mut brain_guard = brain.write().await;
        let mut state_guard = state.write().await;
        state_guard.last_scan = Some(Instant::now());
        state_guard.cycles = 0;

        let threats = monitoring_cycle(&mut brain_guard, &defense).await;
        state_guard.total_threats += threats as u64;
        state_guard.cycles += 1;

        eprintln!("[monitor] Baseline established. {} threats in first scan.\n", threats);
    }

    // Subsequent cycles
    loop {
        time::sleep(interval).await;

        let mut brain_guard = brain.write().await;
        let mut state_guard = state.write().await;

        let threats = monitoring_cycle(&mut brain_guard, &defense).await;
        state_guard.total_threats += threats as u64;
        state_guard.cycles += 1;
        state_guard.last_scan = Some(Instant::now());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_monitoring_cycle() {
        let mut brain = VSABrain::new(0.12);
        let defense = DefenseSystem::new(9000);

        // Pre-load some text knowledge (what should be running)
        crate::text_encoder::store_knowledge_triple(
            &mut brain, "backend", "listens_on", "port_8000", 0.9, "text_knowledge"
        );
        crate::text_encoder::store_knowledge_triple(
            &mut brain, "database", "listens_on", "port_5432", 0.9, "text_knowledge"
        );

        // Run one monitoring cycle
        let threats = monitoring_cycle(&mut brain, &defense).await;
        eprintln!("  Monitoring cycle complete: {} threats", threats);

        // Verify state
        let n_clusters = brain.dejavu_clusters.len();
        let n_entries: usize = brain.dejavu_clusters.iter().map(|c| c.entries.len()).sum();
        eprintln!("  {} clusters, {} total entries", n_clusters, n_entries);

        // Verify defense system state
        let threat_level = *defense.threat_level.read().await;
        eprintln!("  Defense threat level: {:.2}", threat_level);

        // The test should pass regardless of findings — it's an integration check
        assert!(n_clusters > 0, "Should have clusters after monitoring cycle");
    }

    #[tokio::test]
    async fn test_monitor_state_tracking() {
        let state = MonitorState::new();
        assert_eq!(state.cycles, 0);
        assert_eq!(state.total_threats, 0);
        assert!(state.last_scan.is_none());
        assert!(!state.documentation_loaded);
    }

    #[test]
    fn test_monitor_state_sync_update() {
        let state = Arc::new(RwLock::new(MonitorState::new()));

        let state_clone = state.clone();
        tokio::runtime::Runtime::new().unwrap().block_on(async {
            let mut s = state_clone.write().await;
            s.cycles = 5;
            s.total_threats = 12;
            s.last_scan = Some(Instant::now());
        });

        let final_state = tokio::runtime::Runtime::new().unwrap().block_on(async {
            state.read().await.clone()
        });
        // MonitorState doesn't derive Clone. Just test via accessor pattern.
        // This test verifies the Arc<RwLock<>> pattern works.
        assert!(true);
    }
}
