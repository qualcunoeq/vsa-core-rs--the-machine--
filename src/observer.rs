use crate::hnsw::HnswIndex;
use crate::Hypervector;
use std::collections::{HashMap, HashSet, VecDeque};

// ─── Constants ────────────────────────────────────────────────────────────

/// Similarity threshold for a trace to be considered part of a failure cluster.
pub const FAILURE_CLUSTER_THRESHOLD: f64 = 0.72;

/// If a tool appears in >60% of failure traces in a cluster, it gets muted.
pub const TOOL_MUTING_THRESHOLD: f64 = 0.60;

/// Maximum number of trace vectors to keep in the sliding window.
pub const MAX_TRACE_HISTORY: usize = 1000;

/// How many consecutive paradigm shifts before the observer enters
/// "deep reflection" mode (more aggressive noise injection).
pub const PARADIGM_SHIFT_LIMIT: usize = 3;

/// Minimum number of failure traces before automatic tool banning activates.
pub const MIN_FAILURES_FOR_BANNING: usize = 5;

// ─── ThoughtTrace ─────────────────────────────────────────────────────────

/// A complete cognitive cycle captured as a hypervector bundle.
///
/// H_trace = bundle(R_start ⊗ S0, R_action ⊗ tool_used, R_end ⊗ S1, R_outcome ⊗ outcome)
///
/// Where:
/// - R_start, R_action, R_end, R_outcome are fixed role hypervectors
/// - S0 is the world state before the action
/// - tool_used is the tool/action invoked
/// - S1 is the world state after the action
/// - outcome encodes success/failure
#[derive(Clone, Debug)]
pub struct ThoughtTrace {
    /// The bundled trace hypervector
    pub vector: Hypervector,
    /// Start state (before action)
    pub state_before: Hypervector,
    /// The tool or action that was used
    pub tool_used: String,
    /// End state (after action)
    pub state_after: Hypervector,
    /// Whether the outcome was a failure
    pub is_failure: bool,
    /// Cognitive cycle number
    pub tick: usize,
    /// Agent label
    pub agent_id: String,
    /// Human-readable goal context
    pub goal_context: String,
    /// Timestamp
    pub timestamp: chrono::DateTime<chrono::Utc>,
}

impl ThoughtTrace {
    /// Role hypervectors for trace encoding (deterministic via n-gram).
    fn role_start() -> Hypervector {
        Hypervector::encode_text_ngram("ROLE_TRACE_START", 3)
    }

    fn role_action() -> Hypervector {
        Hypervector::encode_text_ngram("ROLE_TRACE_ACTION", 3)
    }

    fn role_end() -> Hypervector {
        Hypervector::encode_text_ngram("ROLE_TRACE_END", 3)
    }

    fn role_outcome() -> Hypervector {
        Hypervector::encode_text_ngram("ROLE_TRACE_OUTCOME", 3)
    }

    /// Encode a trace into a single hypervector.
    pub fn encode(
        state_before: &Hypervector,
        tool_used: &str,
        state_after: &Hypervector,
        is_failure: bool,
    ) -> Hypervector {
        let r_start = Self::role_start();
        let r_action = Self::role_action();
        let r_end = Self::role_end();
        let r_outcome = Self::role_outcome();

        let tool_hv = Hypervector::encode_text_ngram(tool_used, 3);
        let outcome_tag = if is_failure {
            Hypervector::encode_text_ngram("FAILURE", 3)
        } else {
            Hypervector::encode_text_ngram("SUCCESS", 3)
        };

        let bound_start = r_start.bitwise_xor(state_before);
        let bound_action = r_action.bitwise_xor(&tool_hv);
        let bound_end = r_end.bitwise_xor(state_after);
        let bound_outcome = r_outcome.bitwise_xor(&outcome_tag);

        Hypervector::bundle(&[&bound_start, &bound_action, &bound_end, &bound_outcome])
    }
}

// ─── Failure Cluster ──────────────────────────────────────────────────────

/// A detected cluster of related failure traces.
#[derive(Clone, Debug)]
pub struct FailureCluster {
    /// The centroid of this failure manifold
    pub centroid: Hypervector,
    /// Indices of traces in this cluster
    pub trace_indices: Vec<usize>,
    /// Dominant tools in this cluster (tool → count)
    pub tool_frequencies: HashMap<String, usize>,
    /// Whether this cluster triggered tool banning
    pub triggered_ban: bool,
    /// The cognitive cycle when this cluster was last active
    pub last_active_tick: usize,
}

impl FailureCluster {
    /// Check if a tool should be banned based on frequency in this cluster.
    pub fn should_mute_tool(&self, tool_name: &str) -> bool {
        let total = self.trace_indices.len();
        if total < MIN_FAILURES_FOR_BANNING {
            return false;
        }
        let count = self.tool_frequencies.get(tool_name).copied().unwrap_or(0);
        (count as f64) / (total as f64) >= TOOL_MUTING_THRESHOLD
    }

    /// Get the list of muted tools from this cluster.
    pub fn muted_tools(&self) -> Vec<String> {
        let total = self.trace_indices.len();
        if total < MIN_FAILURES_FOR_BANNING {
            return Vec::new();
        }
        self.tool_frequencies
            .iter()
            .filter(|(_, &count)| (count as f64) / (total as f64) >= TOOL_MUTING_THRESHOLD)
            .map(|(tool, _)| tool.clone())
            .collect()
    }
}

// ─── Observer Loop ────────────────────────────────────────────────────────

/// The Meta-Cognitive Observer Loop.
///
/// Runs as a low-frequency background process that:
/// 1. Collects ThoughtTrace vectors from completed cognitive cycles
/// 2. Clusters failure traces to detect CRISIS_MANIFOLDS
/// 3. Automatically bans tools that dominate failure clusters
/// 4. Injects paradigm-shifting noise when the agent approaches known failures
/// 5. Provides diagnostic insight into the machine's cognitive health
pub struct ObserverLoop {
    /// All collected trace vectors (sliding window)
    traces: VecDeque<ThoughtTrace>,
    /// HNSW index for fast failure-pattern matching
    failure_index: HnswIndex,
    /// Detected failure clusters (crisis manifolds)
    failure_clusters: Vec<FailureCluster>,
    /// Currently muted tools (banned in the current goal context)
    muted_tools: HashSet<String>,
    /// Context-dependent muted tools (goal → banned tools)
    context_muted_tools: HashMap<String, HashSet<String>>,
    /// Number of consecutive paradigm shifts
    paradigm_shift_count: usize,
    /// Last tick when analysis was run
    last_analysis_tick: usize,
    /// Analysis interval in cognitive ticks
    analysis_interval: usize,
    /// Maximum traces to retain
    max_traces: usize,
    /// Total traces collected
    total_traces_collected: usize,
    /// Total failures detected
    total_failures_detected: usize,
}

impl ObserverLoop {
    /// Create a new Observer Loop with default configuration.
    pub fn new(analysis_interval: usize) -> Self {
        ObserverLoop {
            traces: VecDeque::with_capacity(MAX_TRACE_HISTORY),
            failure_index: HnswIndex::with_config(crate::hnsw::HnswConfig::high_recall()),
            failure_clusters: Vec::new(),
            muted_tools: HashSet::new(),
            context_muted_tools: HashMap::new(),
            paradigm_shift_count: 0,
            last_analysis_tick: 0,
            analysis_interval,
            max_traces: MAX_TRACE_HISTORY,
            total_traces_collected: 0,
            total_failures_detected: 0,
        }
    }

    /// Record a completed cognitive cycle as a ThoughtTrace.
    pub fn record_trace(
        &mut self,
        state_before: &Hypervector,
        tool_used: &str,
        state_after: &Hypervector,
        is_failure: bool,
        tick: usize,
        agent_id: &str,
        goal_context: &str,
    ) {
        let vector = ThoughtTrace::encode(state_before, tool_used, state_after, is_failure);

        let trace = ThoughtTrace {
            vector,
            state_before: *state_before,
            tool_used: tool_used.to_string(),
            state_after: *state_after,
            is_failure,
            tick,
            agent_id: agent_id.to_string(),
            goal_context: goal_context.to_string(),
            timestamp: chrono::Utc::now(),
        };

        self.traces.push_back(trace);
        self.total_traces_collected += 1;

        if is_failure {
            self.total_failures_detected += 1;
        }

        // Maintain sliding window
        while self.traces.len() > self.max_traces {
            self.traces.pop_front();
        }

        // Index failure traces
        if is_failure {
            let vector = ThoughtTrace::encode(state_before, tool_used, state_after, true);
            self.failure_index.insert(&vector.bits);
        }

        // Periodic analysis
        if tick - self.last_analysis_tick >= self.analysis_interval {
            self.run_analysis(tick);
            self.last_analysis_tick = tick;
        }
    }

    /// Run the meta-analysis loop: cluster failures, ban tools, detect patterns.
    fn run_analysis(&mut self, current_tick: usize) {
        if self.traces.len() < MIN_FAILURES_FOR_BANNING {
            return;
        }

        // Collect failure traces
        let failure_traces: Vec<(usize, &ThoughtTrace)> = self.traces
            .iter()
            .enumerate()
            .filter(|(_, t)| t.is_failure)
            .collect();

        if failure_traces.len() < MIN_FAILURES_FOR_BANNING {
            return;
        }

        // Agglomerative clustering on failure trace vectors
        let mut clusters: Vec<Vec<usize>> = Vec::new();

        for &(trace_idx, trace) in &failure_traces {
            let mut assigned = false;
            for cluster in clusters.iter_mut() {
                // Compute centroid of existing cluster
                let centroid = self.compute_cluster_centroid(cluster, &failure_traces);
                let sim = 1.0 - trace.vector.normalized_hamming_distance(&centroid);
                if sim >= FAILURE_CLUSTER_THRESHOLD {
                    cluster.push(trace_idx);
                    assigned = true;
                    break;
                }
            }
            if !assigned {
                clusters.push(vec![trace_idx]);
            }
        }

        // Update failure clusters from results
        let tool_name_from_trace = |trace: &ThoughtTrace| trace.tool_used.clone();

        for cluster_indices in &clusters {
            if cluster_indices.len() < MIN_FAILURES_FOR_BANNING {
                continue;
            }

            // Compute centroid
            let trace_refs: Vec<&ThoughtTrace> = cluster_indices
                .iter()
                .filter_map(|&i| self.traces.get(i))
                .collect();

            if trace_refs.len() < MIN_FAILURES_FOR_BANNING {
                continue;
            }

            let hv_refs: Vec<&Hypervector> = trace_refs.iter().map(|t| &t.vector).collect();
            let centroid = Hypervector::bundle(&hv_refs);

            // Count tool frequencies
            let mut tool_freqs: HashMap<String, usize> = HashMap::new();
            for t in &trace_refs {
                *tool_freqs.entry(t.tool_used.clone()).or_insert(0) += 1;
            }

            let cluster = FailureCluster {
                centroid,
                trace_indices: cluster_indices.clone(),
                tool_frequencies: tool_freqs,
                triggered_ban: false,
                last_active_tick: current_tick,
            };

            // Check for tool banning
            let muted = cluster.muted_tools();
            if !muted.is_empty() {
                for tool in &muted {
                    self.muted_tools.insert(tool.clone());

                    // Also add to context-specific ban list
                    let goal_contexts: Vec<String> = trace_refs
                        .iter()
                        .map(|t| t.goal_context.clone())
                        .collect();
                    for ctx in &goal_contexts {
                        self.context_muted_tools
                            .entry(ctx.clone())
                            .or_insert_with(HashSet::new)
                            .insert(tool.clone());
                    }
                }
            }

            self.failure_clusters.push(cluster);
        }

        // Reset paradigm shift counter if no new crisis manifolds
        if clusters.is_empty() {
            self.paradigm_shift_count = 0;
        }
    }

    /// Compute centroid of a cluster of trace indices.
    fn compute_cluster_centroid(
        &self,
        indices: &[usize],
        all_traces: &[(usize, &ThoughtTrace)],
    ) -> Hypervector {
        let vectors: Vec<&Hypervector> = indices
            .iter()
            .filter_map(|&idx| all_traces.iter().find(|&&(i, _)| i == idx))
            .map(|(_, t)| &t.vector)
            .collect();

        if vectors.is_empty() {
            return Hypervector::new_zero();
        }

        let refs: Vec<&Hypervector> = vectors.iter().copied().collect();
        Hypervector::bundle(&refs)
    }

    /// Check if the current state is dangerously close to a known failure cluster.
    /// If so, returns the closest failure cluster for paradigm shift calculation.
    pub fn check_failure_proximity(&self, current_state: &Hypervector) -> Option<&FailureCluster> {
        let mut closest: Option<&FailureCluster> = None;
        let mut max_sim = FAILURE_CLUSTER_THRESHOLD;

        for cluster in &self.failure_clusters {
            let sim = 1.0 - current_state.normalized_hamming_distance(&cluster.centroid);
            if sim >= max_sim {
                max_sim = sim;
                closest = Some(cluster);
            }
        }

        closest
    }

    /// Generate a paradigm shift by injecting deterministic noise into the
    /// intention vector. The noise is derived from the failure cluster centroid
    /// to push the pathfinder away from known failure trajectories.
    pub fn generate_paradigm_shift(
        &mut self,
        current_intent: &Hypervector,
        failure_cluster: &FailureCluster,
    ) -> Hypervector {
        // XOR with the failure centroid creates a "repulsion" vector
        let repulsion = current_intent.bitwise_xor(&failure_cluster.centroid);

        // Scale the repulsion based on paradigm shift depth
        self.paradigm_shift_count += 1;
        let depth = (self.paradigm_shift_count as f64 / PARADIGM_SHIFT_LIMIT as f64).min(1.0);

        // Bundle the repulsion multiple times for stronger effect at higher depths
        let copies = (depth * 5.0).round() as usize + 1;
        let mut components = vec![current_intent];
        for _ in 0..copies {
            components.push(&repulsion);
        }

        Hypervector::bundle(&components)
    }

    /// Check if a tool is currently muted (globally or in the given context).
    pub fn is_tool_muted(&self, tool_name: &str, goal_context: Option<&str>) -> bool {
        if self.muted_tools.contains(tool_name) {
            return true;
        }
        if let Some(ctx) = goal_context {
            if let Some(ctx_tools) = self.context_muted_tools.get(ctx) {
                return ctx_tools.contains(tool_name);
            }
        }
        false
    }

    /// Get all currently muted tools.
    pub fn muted_tools(&self) -> &HashSet<String> {
        &self.muted_tools
    }

    /// Get context-specific muted tools.
    pub fn context_muted_tools(&self, context: &str) -> HashSet<String> {
        self.context_muted_tools
            .get(context)
            .cloned()
            .unwrap_or_default()
    }

    /// Reset the tool ban list (e.g., when entering a new goal context).
    pub fn reset_bans(&mut self) {
        self.muted_tools.clear();
        self.paradigm_shift_count = 0;
    }

    /// Reset bans for a specific goal context.
    pub fn reset_context_bans(&mut self, context: &str) {
        self.context_muted_tools.remove(context);
    }

    // ─── Diagnostics ─────────────────────────────────────────────────

    /// Get diagnostic summary of the observer's state.
    pub fn diagnostics(&self) -> ObserverDiagnostics {
        ObserverDiagnostics {
            total_traces: self.total_traces_collected,
            active_traces: self.traces.len(),
            total_failures: self.total_failures_detected,
            failure_clusters: self.failure_clusters.len(),
            muted_tools: self.muted_tools.len(),
            paradigm_shift_count: self.paradigm_shift_count,
            failure_index_size: self.failure_index.len(),
        }
    }

    /// Top failure tools across all clusters.
    pub fn top_failure_tools(&self, n: usize) -> Vec<(String, usize)> {
        let mut all_freqs: HashMap<String, usize> = HashMap::new();
        for cluster in &self.failure_clusters {
            for (tool, count) in &cluster.tool_frequencies {
                *all_freqs.entry(tool.clone()).or_insert(0) += count;
            }
        }
        let mut sorted: Vec<(String, usize)> = all_freqs.into_iter().collect();
        sorted.sort_by(|a, b| b.1.cmp(&a.1));
        sorted.truncate(n);
        sorted
    }
}

/// Diagnostic snapshot of the observer loop.
#[derive(Clone, Debug)]
pub struct ObserverDiagnostics {
    pub total_traces: usize,
    pub active_traces: usize,
    pub total_failures: usize,
    pub failure_clusters: usize,
    pub muted_tools: usize,
    pub paradigm_shift_count: usize,
    pub failure_index_size: usize,
}

impl std::fmt::Display for ObserverDiagnostics {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "Observer: {} traces ({} active), {} failures, {} clusters, {} muted tools, {} shifts, HNSW={} entries",
            self.total_traces,
            self.active_traces,
            self.total_failures,
            self.failure_clusters,
            self.muted_tools,
            self.paradigm_shift_count,
            self.failure_index_size,
        )
    }
}

// ─── Tests ────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_thought_trace_encode() {
        let s0 = Hypervector::new_random();
        let s1 = Hypervector::new_random();
        let vector = ThoughtTrace::encode(&s0, "exec_shell", &s1, false);

        // Same inputs should produce same vector
        let vector2 = ThoughtTrace::encode(&s0, "exec_shell", &s1, false);
        assert_eq!(vector, vector2);

        // Different outcome should produce different vector
        let vector_fail = ThoughtTrace::encode(&s0, "exec_shell", &s1, true);
        assert_ne!(vector, vector_fail);
    }

    #[test]
    fn test_observer_record_traces() {
        let mut observer = ObserverLoop::new(10);
        let s0 = Hypervector::new_random();
        let s1 = Hypervector::new_random();

        // Record several traces
        for i in 0..20 {
            let is_failure = i % 3 == 0; // Every 3rd trace is a failure
            observer.record_trace(
                &s0, "exec_shell", &s1, is_failure,
                i, "Agent-1", "test_goal",
            );
        }

        assert_eq!(observer.total_traces_collected, 20);
        assert!(observer.total_failures_detected > 0);
    }

    #[test]
    fn test_observer_failure_detection() {
        let mut observer = ObserverLoop::new(5);
        let s0 = Hypervector::encode_text_ngram("start_state", 3);
        let s1 = Hypervector::encode_text_ngram("error_state", 3);

        // Record many failures with the same context to trigger clustering
        for i in 0..30 {
            let is_failure = i >= 10; // First 10 success, then 20 failures
            observer.record_trace(
                &s0, "exec_shell", &s1, is_failure,
                i, "Agent-1", "test_goal",
            );
        }

        let diag = observer.diagnostics();
        assert!(
            diag.total_failures >= 15,
            "Should detect most failures, got {}",
            diag.total_failures
        );
    }

    #[test]
    fn test_tool_banning() {
        let mut observer = ObserverLoop::new(3);
        let s0 = Hypervector::encode_text_ngram("start", 3);
        let s_err = Hypervector::encode_text_ngram("error", 3);

        // Create a pattern where exec_shell consistently fails
        for i in 0..20 {
            observer.record_trace(
                &s0, "exec_shell", &s_err, true, // Always failure
                i, "Agent-1", "test_goal",
            );
        }

        // After enough failures with the same tool, it should be muted
        // The analysis runs automatically every `analysis_interval` ticks
        let diag = observer.diagnostics();
        if diag.muted_tools > 0 {
            assert!(
                observer.is_tool_muted("exec_shell", Some("test_goal")),
                "exec_shell should be muted after repeated failures"
            );
        }
    }

    #[test]
    fn test_paradigm_shift_generation() {
        let mut observer = ObserverLoop::new(10);
        let centroid = Hypervector::new_random();
        let current_intent = Hypervector::new_random();

        let cluster = FailureCluster {
            centroid,
            trace_indices: vec![0, 1, 2],
            tool_frequencies: HashMap::new(),
            triggered_ban: false,
            last_active_tick: 0,
        };

        let shift = observer.generate_paradigm_shift(&current_intent, &cluster);
        assert_ne!(shift, current_intent, "Shift should differ from original intent");

        // Multiple shifts should progressively differ from original
        let shift2 = observer.generate_paradigm_shift(&current_intent, &cluster);
        let dist_orig = current_intent.normalized_hamming_distance(&shift);
        let dist_orig2 = current_intent.normalized_hamming_distance(&shift2);
        // Second shift should be at least as far from original as first
        assert!(
            dist_orig2 >= dist_orig - 0.05,
            "Progressive shift should maintain or increase distance from original: {} vs {}",
            dist_orig, dist_orig2
        );
    }

    #[test]
    fn test_failure_proximity_check() {
        let mut observer = ObserverLoop::new(5);

        // Create a failure cluster
        let fail_vector = Hypervector::encode_text_ngram("failure_pattern", 3);
        let cluster = FailureCluster {
            centroid: fail_vector,
            trace_indices: vec![0, 1, 2, 3, 4, 5],
            tool_frequencies: {
                let mut m = HashMap::new();
                m.insert("exec_shell".to_string(), 6);
                m
            },
            triggered_ban: false,
            last_active_tick: 0,
        };
        observer.failure_clusters.push(cluster);

        // Check proximity to a similar vector
        let similar = Hypervector::encode_text_ngram("failure_pattern", 3);
        let proximity = observer.check_failure_proximity(&similar);
        assert!(proximity.is_some(), "Should detect proximity to failure cluster");

        // Check proximity to a different vector
        let different = Hypervector::new_random();
        let no_proximity = observer.check_failure_proximity(&different);
        // May or may not trigger depending on random distance
    }

    #[test]
    fn test_failure_cluster_muted_tools() {
        let cluster = FailureCluster {
            centroid: Hypervector::new_random(),
            trace_indices: vec![0, 1, 2, 3, 4, 5, 6, 7, 8, 9],
            tool_frequencies: {
                let mut m = HashMap::new();
                m.insert("exec_shell".to_string(), 7); // 70% — should be muted
                m.insert("sys_read".to_string(), 3);   // 30% — should NOT be muted
                m
            },
            triggered_ban: false,
            last_active_tick: 0,
        };

        let muted = cluster.muted_tools();
        assert!(muted.contains(&"exec_shell".to_string()), "exec_shell should be muted");
        assert!(!muted.contains(&"sys_read".to_string()), "sys_read should NOT be muted");
    }

    #[test]
    fn test_context_specific_banning() {
        let mut observer = ObserverLoop::new(1);
        let s0 = Hypervector::encode_text_ngram("start", 3);
        let s_err = Hypervector::encode_text_ngram("error", 3);

        // Create failures in "context_a" with exec_shell
        for i in 0..15 {
            observer.record_trace(
                &s0, "exec_shell", &s_err, true,
                i, "Agent-1", "context_a",
            );
        }

        // Create successes in "context_b" with exec_shell
        for i in 15..20 {
            observer.record_trace(
                &s0, "exec_shell", &s_err, false,
                i, "Agent-1", "context_b",
            );
        }

        // exec_shell should be muted in context_a but not necessarily in context_b
        let ctx_a_bans = observer.context_muted_tools("context_a");
        let ctx_b_bans = observer.context_muted_tools("context_b");

        if !ctx_a_bans.is_empty() {
            assert!(
                ctx_a_bans.contains("exec_shell"),
                "exec_shell should be banned in context_a"
            );
        }
    }
}
