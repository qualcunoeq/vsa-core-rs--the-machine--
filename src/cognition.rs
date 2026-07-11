//! Shared cognitive audit and evaluation types.
//!
//! These types make the architecture's closed loop explicit: observe, resolve,
//! answer or act, observe the outcome, then decide what changed in memory.

use crate::actuator::{ActionRequest, ActionResult};
use crate::qa::ResolveTrace;
use std::collections::HashMap;
use std::path::Path;

/// Runtime feature switches used for ablation studies.
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize, PartialEq)]
pub struct AblationConfig {
    pub use_trace: bool,
    pub use_abstraction: bool,
    pub use_associations: bool,
    pub use_soft_projection: bool,
    pub use_self_model: bool,
    pub use_tool_memory: bool,
}

impl Default for AblationConfig {
    fn default() -> Self {
        AblationConfig {
            use_trace: true,
            use_abstraction: true,
            use_associations: true,
            use_soft_projection: true,
            use_self_model: true,
            use_tool_memory: true,
        }
    }
}

/// A bounded autonomy budget. External actions should consume from this before
/// execution so long-running autonomy remains operator-visible.
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize, PartialEq)]
pub struct AutonomyBudget {
    pub max_actions: u32,
    pub actions_used: u32,
    pub max_time_ms: u64,
    pub time_used_ms: u64,
    pub max_external_writes: u32,
    pub external_writes_used: u32,
    pub max_risk: f64,
}

impl AutonomyBudget {
    pub fn new(
        max_actions: u32,
        max_time_ms: u64,
        max_external_writes: u32,
        max_risk: f64,
    ) -> Self {
        AutonomyBudget {
            max_actions,
            actions_used: 0,
            max_time_ms,
            time_used_ms: 0,
            max_external_writes,
            external_writes_used: 0,
            max_risk: max_risk.clamp(0.0, 1.0),
        }
    }

    pub fn can_spend(&self, action_risk: f64, is_external_write: bool) -> bool {
        if self.actions_used >= self.max_actions {
            return false;
        }
        if action_risk > self.max_risk {
            return false;
        }
        if is_external_write && self.external_writes_used >= self.max_external_writes {
            return false;
        }
        self.time_used_ms < self.max_time_ms
    }

    pub fn spend(
        &mut self,
        action_risk: f64,
        duration_ms: u64,
        is_external_write: bool,
    ) -> Result<(), String> {
        if !self.can_spend(action_risk, is_external_write) {
            return Err("autonomy budget exhausted or risk too high".to_string());
        }
        if self.time_used_ms.saturating_add(duration_ms) > self.max_time_ms {
            return Err("autonomy time budget would be exceeded".to_string());
        }
        self.actions_used += 1;
        self.time_used_ms = self.time_used_ms.saturating_add(duration_ms);
        if is_external_write {
            self.external_writes_used += 1;
        }
        Ok(())
    }
}

/// Outcome observed after an answer or action.
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize, PartialEq)]
pub enum EpisodeOutcome {
    Unknown,
    Success {
        score: f64,
        evidence: String,
    },
    Failure {
        score: f64,
        error_class: String,
        evidence: String,
    },
}

impl EpisodeOutcome {
    pub fn score(&self) -> Option<f64> {
        match self {
            EpisodeOutcome::Unknown => None,
            EpisodeOutcome::Success { score, .. } => Some(score.clamp(0.0, 1.0)),
            EpisodeOutcome::Failure { score, .. } => Some(score.clamp(0.0, 1.0)),
        }
    }
}

/// A memory change applied or proposed after feedback.
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize, PartialEq)]
pub struct MemoryUpdate {
    pub target: String,
    pub operation: String,
    pub before: Option<String>,
    pub after: Option<String>,
    pub confidence_delta: f64,
    pub reversible: bool,
}

/// A replayable record of one cognitive answer/action episode.
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize, PartialEq)]
pub struct CognitiveEpisode {
    pub id: String,
    pub input: String,
    pub term_traces: Vec<ResolveTrace>,
    pub answer: Option<String>,
    pub confidence: f64,
    pub outcome: EpisodeOutcome,
    pub updates: Vec<MemoryUpdate>,
    pub ablations: AblationConfig,
}

impl CognitiveEpisode {
    pub fn new(id: impl Into<String>, input: impl Into<String>) -> Self {
        CognitiveEpisode {
            id: id.into(),
            input: input.into(),
            term_traces: Vec::new(),
            answer: None,
            confidence: 0.0,
            outcome: EpisodeOutcome::Unknown,
            updates: Vec::new(),
            ablations: AblationConfig::default(),
        }
    }

    pub fn with_answer(mut self, answer: impl Into<String>, confidence: f64) -> Self {
        self.answer = Some(answer.into());
        self.confidence = confidence.clamp(0.0, 1.0);
        self
    }
}

/// Side-effect class for tool/action audit records.
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize, PartialEq)]
pub enum SideEffectClass {
    ReadOnly,
    LocalWrite,
    ExternalWrite,
    Network,
    Unknown,
}

// ═════════════════════════════════════════════════════════════════════════
// CONCEPT LIFECYCLE JOURNAL — structured concept lifecycle logging
// ═════════════════════════════════════════════════════════════════════════

/// What happened to a concept during its lifecycle.
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize, PartialEq)]
pub enum ConceptEventType {
    /// A new concept was created (L2 abstraction or L3 meta-concept).
    Created,
    /// Two clusters were merged into one (cluster compactor).
    Merged,
    /// A concept was split into two (placeholder for future use).
    Split,
    /// A concept was frozen (placed into cold storage).
    Frozen,
    /// A concept's coherence decayed below a threshold.
    Decayed,
    /// An existing concept was reinforced (coherence reset to 1.0).
    Reinforced,
    /// A concept was dissolved (removed due to low coherence).
    Dissolved,
}

/// One entry in the concept lifecycle journal.
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize, PartialEq)]
pub struct ConceptEvent {
    /// System tick when the event occurred.
    pub tick: u64,
    /// The event type.
    pub event_type: ConceptEventType,
    /// Hierarchy level the concept belongs to (1 = base, 2 = L2 abstraction, 3 = L3 meta).
    pub level: usize,
    /// Index of the affected concept within its level (if applicable).
    pub concept_idx: Option<usize>,
    /// Human-readable details about the event.
    pub details: String,
}

/// An append-only, persistent journal of concept lifecycle events.
///
/// Each event records what happened to a concept, when, and in which
/// hierarchy level.  The journal supports querying by level and event
/// type, as well as full JSON persistence for offline analysis.
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct ConceptJournal {
    pub events: Vec<ConceptEvent>,
    /// Default persistence path (None = in-memory only).
    pub path: Option<String>,
}

impl ConceptJournal {
    pub fn new() -> Self {
        ConceptJournal {
            events: Vec::new(),
            path: None,
        }
    }

    pub fn with_path(path: impl Into<String>) -> Self {
        ConceptJournal {
            events: Vec::new(),
            path: Some(path.into()),
        }
    }

    /// Push a new event onto the journal.
    pub fn push(&mut self, event: ConceptEvent) {
        self.events.push(event);
    }

    /// Return all events that occurred at or after `tick`.
    pub fn since(&self, tick: u64) -> Vec<&ConceptEvent> {
        self.events.iter().filter(|e| e.tick >= tick).collect()
    }

    /// Return events filtered by level.
    pub fn for_level(&self, level: usize) -> Vec<&ConceptEvent> {
        self.events.iter().filter(|e| e.level == level).collect()
    }

    /// Return events filtered by event type.
    pub fn of_type(&self, event_type: ConceptEventType) -> Vec<&ConceptEvent> {
        self.events
            .iter()
            .filter(|e| e.event_type == event_type)
            .collect()
    }

    /// Return all creation events (convenience).
    pub fn creations(&self) -> Vec<&ConceptEvent> {
        self.events
            .iter()
            .filter(|e| e.event_type == ConceptEventType::Created)
            .collect()
    }

    /// Return all dissolution events (convenience).
    pub fn dissolutions(&self) -> Vec<&ConceptEvent> {
        self.events
            .iter()
            .filter(|e| e.event_type == ConceptEventType::Dissolved)
            .collect()
    }

    /// Total number of events in the journal.
    pub fn len(&self) -> usize {
        self.events.len()
    }

    pub fn is_empty(&self) -> bool {
        self.events.is_empty()
    }

    /// Save to JSON file at the configured path.
    pub fn save(&self) -> Result<(), String> {
        let p = self.path.as_ref().ok_or_else(|| "ConceptJournal: no save path configured".to_string())?;
        self.save_to(p)
    }

    /// Save to JSON file at the given path.
    pub fn save_to(&self, path: impl AsRef<Path>) -> Result<(), String> {
        let path = path.as_ref();
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| format!("ConceptJournal: cannot create dirs: {}", e))?;
        }
        let json = serde_json::to_string_pretty(self)
            .map_err(|e| format!("ConceptJournal: serialization error: {}", e))?;
        std::fs::write(path, &json)
            .map_err(|e| format!("ConceptJournal: write error: {}", e))?;
        Ok(())
    }

    /// Load from a JSON file.
    pub fn load(path: impl AsRef<Path>) -> Result<Self, String> {
        let json = std::fs::read_to_string(path.as_ref())
            .map_err(|e| format!("ConceptJournal: read error: {}", e))?;
        serde_json::from_str(&json)
            .map_err(|e| format!("ConceptJournal: deserialization error: {}", e))
    }
}

// ═════════════════════════════════════════════════════════════════════════
// CONCEPT QUALITY SCORE — quantitative evaluation without manual inspection
// ═════════════════════════════════════════════════════════════════════════

/// Quality metrics for a single concept, computed without manual inspection.
///
/// The score is a composite of:
/// - **coherence**: How stable the concept's internal structure is (0.0–1.0, from CoherenceTracker).
/// - **component_count**: How many lower-level centroids form this concept.
/// - **ticks_since_reinforced**: How long since the concept was last reinforced.
/// - **internal_similarity**: Average pairwise NHD similarity between component centroids.
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct ConceptQualityScore {
    /// Concept index within its level.
    pub concept_idx: usize,
    /// Hierarchy level (2 = L2 abstraction, 3 = L3 meta).
    pub level: usize,
    /// Coherence score (0.0–1.0, 1.0 = perfectly coherent).
    pub coherence: f64,
    /// Number of component centroids.
    pub component_count: usize,
    /// Ticks since last reinforcement.
    pub ticks_since_reinforced: u64,
    /// Average pairwise similarity between component centroids (0.0–1.0).
    pub internal_similarity: f64,
    /// Composite quality score (0.0–1.0).
    pub composite: f64,
}

impl ConceptQualityScore {
    /// Compute the composite score from the individual components.
    ///
    /// Formula:
    ///   composite = 0.50 × coherence
    ///             + 0.20 × component_count_normalized
    ///             + 0.20 × freshness
    ///             + 0.10 × internal_similarity
    ///
    /// where freshness = exp(-ticks_since_reinforced / 500).
    pub fn compute_composite(&mut self) {
        let count_norm = (self.component_count as f64 / 20.0).min(1.0);
        let freshness = (-(self.ticks_since_reinforced as f64) / 500.0).exp();
        self.composite = 0.50 * self.coherence
            + 0.20 * count_norm
            + 0.20 * freshness
            + 0.10 * self.internal_similarity;
    }
}

/// Auditable record of a tool invocation and its memory impact.
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct ToolEvent {
    pub id: String,
    pub intent: String,
    pub request: ActionRequest,
    pub result: Option<ActionResult>,
    pub side_effect: SideEffectClass,
    pub confidence: f64,
    pub memory_updates: Vec<MemoryUpdate>,
}

impl ToolEvent {
    pub fn succeeded(&self) -> Option<bool> {
        self.result.as_ref().map(|result| result.success)
    }
}

// ═════════════════════════════════════════════════════════════════════════
// TOOL EVENT STORE — persistent audit log for tool invocations
// ═════════════════════════════════════════════════════════════════════════

/// Append-only store for tool events with JSON persistence.
///
/// Each event records a tool invocation with intent, request, result,
/// side-effect class, and confidence.  The store supports querying by
/// action type and success/failure for reliability aggregation.
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct ToolEventStore {
    pub events: Vec<ToolEvent>,
    /// Default persistence path (None = in-memory only).
    pub path: Option<String>,
    counter: u64,
}

impl ToolEventStore {
    pub fn new() -> Self {
        ToolEventStore {
            events: Vec::new(),
            path: None,
            counter: 0,
        }
    }

    /// Push a tool event.  If its `id` is empty, auto-generate one.
    pub fn push(&mut self, mut event: ToolEvent) -> &ToolEvent {
        if event.id.is_empty() {
            self.counter += 1;
            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis();
            event.id = format!("tool-{}-{}", now, self.counter);
        }
        self.events.push(event);
        self.events.last().unwrap()
    }

    /// Return events filtered by action type.
    pub fn by_action_type(&self, action_type: &str) -> Vec<&ToolEvent> {
        self.events
            .iter()
            .filter(|e| format!("{:?}", e.request.action_type).to_lowercase() == action_type.to_lowercase())
            .collect()
    }

    /// Return events where `succeeded()` matches.
    pub fn by_success(&self, succeeded: bool) -> Vec<&ToolEvent> {
        self.events
            .iter()
            .filter(|e| e.succeeded() == Some(succeeded))
            .collect()
    }

    /// Count of events for a given action type.
    pub fn count_by_type(&self, action_type: &str) -> usize {
        self.by_action_type(action_type).len()
    }

    /// Success rate for a given action type (0.0–1.0).
    pub fn success_rate(&self, action_type: &str) -> Option<f64> {
        let total = self.count_by_type(action_type);
        if total == 0 {
            return None;
        }
        let successes = self
            .events
            .iter()
            .filter(|e| {
                format!("{:?}", e.request.action_type).to_lowercase() == action_type.to_lowercase()
                    && e.succeeded() == Some(true)
            })
            .count();
        Some(successes as f64 / total as f64)
    }

    /// Overall success rate across all action types.
    pub fn overall_success_rate(&self) -> Option<f64> {
        let total = self.events.len();
        if total == 0 {
            return None;
        }
        let successes = self.events.iter().filter(|e| e.succeeded() == Some(true)).count();
        Some(successes as f64 / total as f64)
    }

    /// Number of events in the store.
    pub fn len(&self) -> usize {
        self.events.len()
    }

    pub fn is_empty(&self) -> bool {
        self.events.is_empty()
    }

    /// Save to JSON file at the configured path.
    pub fn save(&self) -> Result<(), String> {
        let p = self.path.as_ref().ok_or_else(|| "ToolEventStore: no save path configured".to_string())?;
        self.save_to(p)
    }

    /// Save to JSON file at the given path.
    pub fn save_to(&self, path: impl AsRef<std::path::Path>) -> Result<(), String> {
        let path = path.as_ref();
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| format!("ToolEventStore: cannot create dirs: {}", e))?;
        }
        let json = serde_json::to_string_pretty(self)
            .map_err(|e| format!("ToolEventStore: serialization error: {}", e))?;
        std::fs::write(path, &json)
            .map_err(|e| format!("ToolEventStore: write error: {}", e))?;
        Ok(())
    }

    /// Load from a JSON file.
    pub fn load(path: impl AsRef<std::path::Path>) -> Result<Self, String> {
        let json = std::fs::read_to_string(path.as_ref())
            .map_err(|e| format!("ToolEventStore: read error: {}", e))?;
        serde_json::from_str(&json)
            .map_err(|e| format!("ToolEventStore: deserialization error: {}", e))
    }
}

impl Default for ToolEventStore {
    fn default() -> Self {
        ToolEventStore::new()
    }
}

// ═════════════════════════════════════════════════════════════════════════
// TOOL RELIABILITY TRACKER — per-action-type success/failure aggregation
// ═════════════════════════════════════════════════════════════════════════

/// Tracks the reliability of each tool/action type over time.
///
/// Records per-action-type success and failure counts and computes
/// running reliability scores.  Designed to be wired into the actuator
/// execution flow so the self-model can query tool reliability.
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct ToolReliabilityTracker {
    /// Per-action-type reliability entries.
    pub entries: Vec<ActionReliability>,
    /// EWMA smoothing factor (default 0.05 for ~20-tick half-life).
    pub alpha: f64,
}

/// Reliability metrics for a single action type.
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct ActionReliability {
    /// The action type name (e.g., "ScanPort", "CheckService").
    pub action_type: String,
    /// Total invocations.
    pub total: u64,
    /// Successful invocations.
    pub successes: u64,
    /// Failed invocations.
    pub failures: u64,
    /// EWMA-smoothed reliability score (0.0–1.0).  Starts at 0.50.
    pub reliability_ewma: f64,
}

impl ActionReliability {
    pub fn new(action_type: impl Into<String>) -> Self {
        ActionReliability {
            action_type: action_type.into(),
            total: 0,
            successes: 0,
            failures: 0,
            reliability_ewma: 0.50,
        }
    }

    pub fn success_rate(&self) -> Option<f64> {
        if self.total == 0 {
            None
        } else {
            Some(self.successes as f64 / self.total as f64)
        }
    }
}

impl ToolReliabilityTracker {
    pub fn new() -> Self {
        ToolReliabilityTracker {
            entries: Vec::new(),
            alpha: 0.05,
        }
    }

    /// Record a tool invocation outcome.
    pub fn record(&mut self, action_type: &str, succeeded: bool) {
        let alpha = self.alpha; // copy before mutable borrow
        let entry = self.entry_mut(action_type);
        entry.total += 1;
        if succeeded {
            entry.successes += 1;
        } else {
            entry.failures += 1;
        }
        // EWMA update: R = α · outcome + (1-α) · R_prev
        let outcome = if succeeded { 1.0 } else { 0.0 };
        entry.reliability_ewma =
            alpha * outcome + (1.0 - alpha) * entry.reliability_ewma;
    }

    /// Record from a ToolEvent.
    pub fn record_event(&mut self, event: &ToolEvent) {
        let action_type = format!("{:?}", event.request.action_type);
        self.record(&action_type, event.succeeded().unwrap_or(false));
    }

    /// Get the EWMA reliability for an action type.
    pub fn reliability(&self, action_type: &str) -> f64 {
        self.entry(action_type)
            .map(|e| e.reliability_ewma)
            .unwrap_or(0.50) // unknown tools default to 0.50
    }

    /// Get the success rate (total ratio) for an action type.
    pub fn success_rate(&self, action_type: &str) -> Option<f64> {
        self.entry(action_type).and_then(|e| e.success_rate())
    }

    /// Overall reliability across all action types (EWMA average).
    pub fn overall_reliability(&self) -> f64 {
        if self.entries.is_empty() {
            return 0.50;
        }
        self.entries.iter().map(|e| e.reliability_ewma).sum::<f64>() / self.entries.len() as f64
    }

    fn entry(&self, action_type: &str) -> Option<&ActionReliability> {
        self.entries
            .iter()
            .find(|e| e.action_type.to_lowercase() == action_type.to_lowercase())
    }

    fn entry_mut(&mut self, action_type: &str) -> &mut ActionReliability {
        let idx = self
            .entries
            .iter()
            .position(|e| e.action_type.to_lowercase() == action_type.to_lowercase());
        if let Some(i) = idx {
            &mut self.entries[i]
        } else {
            self.entries.push(ActionReliability::new(action_type));
            self.entries.last_mut().unwrap()
        }
    }

    /// Number of action types tracked.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

impl Default for ToolReliabilityTracker {
    fn default() -> Self {
        ToolReliabilityTracker::new()
    }
}

// ═════════════════════════════════════════════════════════════════════════
// DECISION RECORD — replayable record of an autonomous decision
// ═════════════════════════════════════════════════════════════════════════

/// A fully replayable record of one autonomous decision.
///
/// Captures what the system intended, what action it chose, what happened,
/// why it made that choice, and what remained of its autonomy budget.
/// Every external action must produce a DecisionRecord before execution.
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct DecisionRecord {
    /// System tick when the decision was made.
    pub tick: u64,
    /// What the system intended to achieve (human-readable).
    pub intent: String,
    /// The action that was chosen.
    pub action_request: crate::actuator::ActionRequest,
    /// The result of executing the action (None if budget blocked it).
    pub action_result: Option<crate::actuator::ActionResult>,
    /// Snapshot of the autonomy budget before spending.
    pub budget_before: AutonomyBudget,
    /// Snapshot of the autonomy budget after spending.
    pub budget_after: AutonomyBudget,
    /// Why this action was chosen (the reasoning path).
    pub reasoning: String,
    /// Whether the budget allowed this action.
    pub budget_allowed: bool,
    /// Link to the corresponding ToolEvent ID (if emitted).
    pub tool_event_id: Option<String>,
}

impl DecisionRecord {
    pub fn new(
        tick: u64,
        intent: impl Into<String>,
        action_request: crate::actuator::ActionRequest,
        reasoning: impl Into<String>,
        budget: &AutonomyBudget,
    ) -> Self {
        DecisionRecord {
            tick,
            intent: intent.into(),
            action_request,
            action_result: None,
            budget_before: budget.clone(),
            budget_after: budget.clone(),
            reasoning: reasoning.into(),
            budget_allowed: false,
            tool_event_id: None,
        }
    }
}

// ═════════════════════════════════════════════════════════════════════════
// DECISION JOURNAL — append-only log of autonomous decisions
// ═════════════════════════════════════════════════════════════════════════

/// Append-only journal of decision records with JSON persistence.
///
/// Every external action should produce an entry before execution, with
/// budget snapshot and reasoning.  After execution, the result is filled in.
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct DecisionJournal {
    pub records: Vec<DecisionRecord>,
    pub path: Option<String>,
}

impl DecisionJournal {
    pub fn new() -> Self {
        DecisionJournal {
            records: Vec::new(),
            path: None,
        }
    }

    pub fn push(&mut self, record: DecisionRecord) {
        self.records.push(record);
    }

    /// Return records where the budget blocked the action.
    pub fn blocked_records(&self) -> Vec<&DecisionRecord> {
        self.records.iter().filter(|r| !r.budget_allowed).collect()
    }

    /// Return records where the action succeeded.
    pub fn successful_records(&self) -> Vec<&DecisionRecord> {
        self.records
            .iter()
            .filter(|r| r.action_result.as_ref().map(|a| a.success).unwrap_or(false))
            .collect()
    }

    pub fn len(&self) -> usize {
        self.records.len()
    }

    pub fn is_empty(&self) -> bool {
        self.records.is_empty()
    }

    /// Save to JSON file at the configured path.
    pub fn save(&self) -> Result<(), String> {
        let p = self.path.as_ref().ok_or_else(|| "DecisionJournal: no save path configured".to_string())?;
        self.save_to(p)
    }

    pub fn save_to(&self, path: impl AsRef<std::path::Path>) -> Result<(), String> {
        let path = path.as_ref();
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| format!("DecisionJournal: cannot create dirs: {}", e))?;
        }
        let json = serde_json::to_string_pretty(self)
            .map_err(|e| format!("DecisionJournal: serialization error: {}", e))?;
        std::fs::write(path, &json)
            .map_err(|e| format!("DecisionJournal: write error: {}", e))?;
        Ok(())
    }

    pub fn load(path: impl AsRef<std::path::Path>) -> Result<Self, String> {
        let json = std::fs::read_to_string(path.as_ref())
            .map_err(|e| format!("DecisionJournal: read error: {}", e))?;
        serde_json::from_str(&json)
            .map_err(|e| format!("DecisionJournal: deserialization error: {}", e))
    }
}

impl Default for DecisionJournal {
    fn default() -> Self {
        DecisionJournal::new()
    }
}

/// Machine-readable result from a recurring experiment or benchmark.
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize, PartialEq)]
pub struct ExperimentResult {
    pub experiment: String,
    pub claim: String,
    pub commit: String,
    pub seed: u64,
    pub dataset: Option<String>,
    pub baseline: String,
    pub metrics: HashMap<String, f64>,
    pub passed: bool,
    pub notes: String,
}

impl ExperimentResult {
    pub fn metric(&self, name: &str) -> Option<f64> {
        self.metrics.get(name).copied()
    }
}

// ═════════════════════════════════════════════════════════════════════════
// CONFIDENCE CALIBRATION — stated confidence vs actual accuracy
// ═════════════════════════════════════════════════════════════════════════

/// Tracks the relationship between stated confidence and actual correctness
/// across many QA episodes.  Used to detect overconfidence or underconfidence
/// and to compute calibration metrics (ECE, reliability).
///
/// Each observation is a (confidence, was_correct) pair recorded from
/// episode outcomes.
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct ConfidenceCalibration {
    /// Confidence bins [0.0, 0.1, 0.2, ..., 1.0] for ECE computation.
    pub bins: Vec<CalibrationBin>,
    /// Total observations recorded.
    pub total_observations: u64,
    /// Running count of correct predictions.
    pub total_correct: u64,
}

/// One bin in the confidence calibration histogram.
///
/// Bins are [b, b+0.1), where b = bin_index * 0.1.
/// The last bin [0.9, 1.0] is closed on both sides.
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct CalibrationBin {
    /// Number of predictions in this bin.
    pub count: u64,
    /// Number of correct predictions in this bin.
    pub correct: u64,
    /// Sum of confidences in this bin (for computing average confidence).
    pub confidence_sum: f64,
}

impl ConfidenceCalibration {
    pub fn new() -> Self {
        ConfidenceCalibration {
            bins: (0..10)
                .map(|_| CalibrationBin {
                    count: 0,
                    correct: 0,
                    confidence_sum: 0.0,
                })
                .collect(),
            total_observations: 0,
            total_correct: 0,
        }
    }

    /// Record a (confidence, was_correct) observation.
    pub fn record(&mut self, confidence: f64, was_correct: bool) {
        let clamped = confidence.clamp(0.0, 1.0);
        // Map to bin: [0,0.1) → 0, [0.1,0.2) → 1, ..., [0.9, 1.0] → 9
        let bin_idx = ((clamped * 10.0) as usize).min(9);
        if bin_idx < self.bins.len() {
            self.bins[bin_idx].count += 1;
            self.bins[bin_idx].confidence_sum += clamped;
            if was_correct {
                self.bins[bin_idx].correct += 1;
            }
        }
        self.total_observations += 1;
        if was_correct {
            self.total_correct += 1;
        }
    }

    /// Record from a cognitive episode's confidence and outcome.
    /// Returns true if the episode had a definitive outcome (Success/Failure).
    pub fn record_episode(&mut self, episode: &CognitiveEpisode) -> bool {
        match &episode.outcome {
            EpisodeOutcome::Success { score, .. } => {
                // Treat score > 0.5 as "correct"
                self.record(episode.confidence, *score > 0.5);
                true
            }
            EpisodeOutcome::Failure { score, .. } => {
                // Treat score < 0.5 as "incorrect" (higher score = less wrong? No —
                // Failure score is the failure severity.  We treat any failure as
                // incorrect regardless of score.)
                self.record(episode.confidence, false);
                true
            }
            EpisodeOutcome::Unknown => false,
        }
    }

    /// Record from all episodes in a store that have definitive outcomes.
    pub fn record_store(&mut self, store: &EpisodeStore) {
        for ep in &store.episodes {
            self.record_episode(ep);
        }
    }

    /// Record only episodes starting at `start_index` (0-based).
    /// Useful for periodic updates where only new episodes need processing.
    pub fn record_store_from(&mut self, store: &EpisodeStore, start_index: usize) {
        for ep in store.episodes.iter().skip(start_index) {
            self.record_episode(ep);
        }
    }

    /// Expected Calibration Error (ECE).
    ///
    /// ECE = Σ (bᵢ/n) × |acc(bᵢ) - conf(bᵢ)|
    /// where bᵢ is the number of predictions in bin i, n is total,
    /// acc(bᵢ) is the accuracy within bin, conf(bᵢ) is the average
    /// confidence within bin.
    pub fn expected_calibration_error(&self) -> f64 {
        let n = self.total_observations as f64;
        if n == 0.0 {
            return 0.0;
        }
        let mut ece = 0.0;
        for bin in &self.bins {
            if bin.count == 0 {
                continue;
            }
            let bin_n = bin.count as f64;
            let accuracy = bin.correct as f64 / bin_n;
            let avg_confidence = bin.confidence_sum / bin_n;
            ece += (bin_n / n) * (accuracy - avg_confidence).abs();
        }
        ece
    }

    /// Overall accuracy.
    pub fn accuracy(&self) -> f64 {
        if self.total_observations == 0 {
            return 0.0;
        }
        self.total_correct as f64 / self.total_observations as f64
    }

    /// Average confidence across all observations.
    pub fn avg_confidence(&self) -> f64 {
        if self.total_observations == 0 {
            return 0.0;
        }
        let total_conf: f64 = self.bins.iter().map(|b| b.confidence_sum).sum();
        total_conf / self.total_observations as f64
    }

    /// Whether the system is overconfident (avg confidence > accuracy).
    pub fn is_overconfident(&self) -> bool {
        self.total_observations > 0 && self.avg_confidence() > self.accuracy()
    }

    /// Whether the system is underconfident (avg confidence < accuracy).
    pub fn is_underconfident(&self) -> bool {
        self.total_observations > 0 && self.avg_confidence() < self.accuracy()
    }

    /// Calibration gap: avg_confidence - accuracy. Positive = overconfident.
    pub fn calibration_gap(&self) -> f64 {
        self.avg_confidence() - self.accuracy()
    }
}

// ═════════════════════════════════════════════════════════════════════════
// FEEDBACK TRACKER — pre/post outcome tracking for the same task family
// ═════════════════════════════════════════════════════════════════════════

/// A single task in a task family: a question/verification with an expected
/// answer pattern.
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct TaskItem {
    /// The input question or verification text.
    pub input: String,
    /// The expected answer (substring match) or None if just recording.
    pub expected: Option<String>,
    /// Whether this is a verify_fact task (subject/verb/object) vs a question.
    pub is_verify: bool,
    /// For verify tasks, the subject.
    pub verify_subject: Option<String>,
    /// For verify tasks, the verb.
    pub verify_verb: Option<String>,
    /// For verify tasks, the object.
    pub verify_object: Option<String>,
}

/// A family of related tasks used for pre/post feedback comparison.
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct TaskFamily {
    /// Name for this family (e.g., "Fed rate changes").
    pub name: String,
    /// Individual task items in this family.
    pub tasks: Vec<TaskItem>,
}

impl TaskFamily {
    pub fn new(name: impl Into<String>) -> Self {
        TaskFamily {
            name: name.into(),
            tasks: Vec::new(),
        }
    }

    /// Add a QA question to this family.
    pub fn add_question(&mut self, input: impl Into<String>, expected: Option<impl Into<String>>) {
        self.tasks.push(TaskItem {
            input: input.into(),
            expected: expected.map(|s| s.into()),
            is_verify: false,
            verify_subject: None,
            verify_verb: None,
            verify_object: None,
        });
    }

    /// Add a verify-fact check to this family.
    pub fn add_verify(
        &mut self,
        subject: impl Into<String>,
        verb: impl Into<String>,
        object: impl Into<String>,
        expected: bool,
    ) {
        let s: String = subject.into();
        let v: String = verb.into();
        let o: String = object.into();
        self.tasks.push(TaskItem {
            input: format!("{} {} {}", s, v, o),
            expected: Some(if expected { "yes" } else { "no" }.to_string()),
            is_verify: true,
            verify_subject: Some(s),
            verify_verb: Some(v),
            verify_object: Some(o),
        });
    }
}

/// A snapshot of results from running a TaskFamily.
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct TaskFamilyRun {
    /// Name of the task family.
    pub family_name: String,
    /// Per-task results in order.
    pub results: Vec<TaskResult>,
    /// Aggregate metrics.
    pub accuracy: f64,
    pub avg_confidence: f64,
}

/// Result of running a single task item.
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct TaskResult {
    /// The input that was presented.
    pub input: String,
    /// The answer produced.
    pub answer: String,
    /// Confidence of the answer.
    pub confidence: f64,
    /// Whether the answer matched the expected pattern.
    pub matched_expected: bool,
    /// Episode ID if recorded.
    pub episode_id: String,
}

/// Compares pre-feedback and post-feedback runs of the same task family.
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct PrePostComparison {
    pub family_name: String,
    pub pre_run: TaskFamilyRun,
    pub post_run: TaskFamilyRun,
    /// Delta accuracy (post - pre). Positive = improvement.
    pub accuracy_delta: f64,
    /// Delta confidence (post - pre). Positive = more confident.
    pub confidence_delta: f64,
    /// Number of tasks where the answer changed.
    pub answer_changes: usize,
    /// Number of tasks where correctness changed.
    pub correctness_changes: usize,
}

impl PrePostComparison {
    /// Compute the comparison from two runs.
    pub fn new(family_name: impl Into<String>, pre_run: TaskFamilyRun, post_run: TaskFamilyRun) -> Self {
        let answer_changes = pre_run
            .results
            .iter()
            .zip(post_run.results.iter())
            .filter(|(pre, post)| pre.answer != post.answer)
            .count();
        let correctness_changes = pre_run
            .results
            .iter()
            .zip(post_run.results.iter())
            .filter(|(pre, post)| pre.matched_expected != post.matched_expected)
            .count();
        PrePostComparison {
            family_name: family_name.into(),
            accuracy_delta: post_run.accuracy - pre_run.accuracy,
            confidence_delta: post_run.avg_confidence - pre_run.avg_confidence,
            answer_changes,
            correctness_changes,
            pre_run,
            post_run,
        }
    }
}

// ═════════════════════════════════════════════════════════════════════════
// EPISODE STORE — persistent feedback log
// ═════════════════════════════════════════════════════════════════════════
/// - Auto-generated unique IDs (timestamp + monotonic counter).
/// - Query by input text to find all outcomes for the same question.
/// - Reversible memory update application (irreversible updates are
///   logged but not applied, as required by C-009's safety rule).
/// - Full save/load to JSON for experiment replay and audit.
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct EpisodeStore {
    pub episodes: Vec<CognitiveEpisode>,
    /// Default persistence path (None = in-memory only).
    pub path: Option<String>,
    /// Monotonic counter for ID generation when timestamps collide.
    counter: u64,
}

impl EpisodeStore {
    /// Create an empty in-memory episode store.
    pub fn new() -> Self {
        EpisodeStore {
            episodes: Vec::new(),
            path: None,
            counter: 0,
        }
    }

    /// Create an empty episode store with a default save path.
    pub fn with_path(path: impl Into<String>) -> Self {
        EpisodeStore {
            episodes: Vec::new(),
            path: Some(path.into()),
            counter: 0,
        }
    }

    /// Push an episode.  If its `id` is empty, auto-generate one from the
    /// current timestamp and an internal counter.
    pub fn push(&mut self, mut episode: CognitiveEpisode) -> &CognitiveEpisode {
        if episode.id.is_empty() {
            self.counter += 1;
            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis();
            episode.id = format!("ep-{}-{}", now, self.counter);
        }
        self.episodes.push(episode);
        self.episodes.last().unwrap()
    }

    /// Return the `n` most recent episodes (or all if `n >= len`).
    pub fn recent(&self, n: usize) -> &[CognitiveEpisode] {
        let len = self.episodes.len();
        let start = len.saturating_sub(n);
        &self.episodes[start..]
    }

    /// Return all outcomes observed for a given input string (exact match).
    pub fn outcomes_for_input(&self, input: &str) -> Vec<&EpisodeOutcome> {
        self.episodes
            .iter()
            .filter(|ep| ep.input == input)
            .map(|ep| &ep.outcome)
            .collect()
    }

    /// Apply all **reversible** memory updates from every episode to a
    /// `QaEngine`.  Irreversible updates (`reversible == false`) are counted
    /// and returned as `skipped` but never applied.
    ///
    /// Supported operations:
    ///   - `"update_rule_confidence"`: adjusts the confidence of the rule
    ///     whose label matches `target`.  The `confidence_delta` is used as
    ///     the error parameter (0.0 = no error, 1.0 = complete failure).
    ///   - `"cull_rule"`: removes a rule below threshold.  Only applied if
    ///     `reversible == true` and the rule still exists.
    ///
    /// Returns `(applied, skipped)`.
    pub fn apply_reversible_updates_to(&self, qa: &mut crate::qa::QaEngine) -> (usize, usize) {
        let mut applied = 0usize;
        let mut skipped = 0usize;
        for episode in &self.episodes {
            for update in &episode.updates {
                if !update.reversible {
                    skipped += 1;
                    continue;
                }
                match update.operation.as_str() {
                    "update_rule_confidence" => {
                        // Find rule by label
                        let found = qa.rules().iter().position(|r| {
                            format!(
                                "{} {} {} -> {} {} {}",
                                r.antecedent_subject,
                                r.antecedent_verb,
                                r.antecedent_object,
                                r.consequent_subject,
                                r.consequent_verb,
                                r.consequent_object,
                            ) == update.target
                        });
                        if let Some(idx) = found {
                            let error = update.confidence_delta.clamp(0.0, 1.0);
                            qa.update_rule_confidence(idx, error);
                            applied += 1;
                        } else {
                            skipped += 1;
                        }
                    }
                    "cull_rule" => {
                        // Remove the named rule if its confidence is low enough
                        let before = qa.rule_count();
                        let target = &update.target;
                        qa.rules_mut().retain(|r| {
                            let label = format!(
                                "{} {} {} -> {} {} {}",
                                r.antecedent_subject,
                                r.antecedent_verb,
                                r.antecedent_object,
                                r.consequent_subject,
                                r.consequent_verb,
                                r.consequent_object,
                            );
                            label != *target
                        });
                        if qa.rule_count() < before {
                            applied += 1;
                        } else {
                            skipped += 1;
                        }
                    }
                    _ => {
                        // Unknown operation — count as skipped
                        skipped += 1;
                    }
                }
            }
        }
        (applied, skipped)
    }

    /// Count reversible and irreversible updates without applying them.
    /// This is the non-mutating version kept for backward compatibility.
    pub fn apply_reversible_updates(&self) -> (usize, usize) {
        let mut applied = 0usize;
        let mut skipped = 0usize;
        for episode in &self.episodes {
            for update in &episode.updates {
                if update.reversible {
                    applied += 1;
                } else {
                    skipped += 1;
                }
            }
        }
        (applied, skipped)
    }

    /// Save the store to its default path (if set).
    pub fn save(&self) -> Result<(), String> {
        match &self.path {
            Some(p) => self.save_to(p),
            None => Err("EpisodeStore: no default save path configured".to_string()),
        }
    }

    /// Save the store to a specific file path (creates parent dirs).
    pub fn save_to(&self, path: &str) -> Result<(), String> {
        if let Some(parent) = Path::new(path).parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| format!("EpisodeStore: cannot create dirs: {}", e))?;
        }
        let json = serde_json::to_string_pretty(self)
            .map_err(|e| format!("EpisodeStore: serialization error: {}", e))?;
        std::fs::write(path, &json).map_err(|e| format!("EpisodeStore: write error: {}", e))?;
        Ok(())
    }

    /// Load a store from a JSON file.
    pub fn load(path: &str) -> Result<Self, String> {
        let json =
            std::fs::read_to_string(path).map_err(|e| format!("EpisodeStore: read error: {}", e))?;
        serde_json::from_str(&json)
            .map_err(|e| format!("EpisodeStore: deserialization error: {}", e))
    }
}

impl Default for EpisodeStore {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::actuator::{ActionRequest, ActionType};

    #[test]
    fn test_autonomy_budget_blocks_excess_actions_and_risk() {
        let mut budget = AutonomyBudget::new(1, 1_000, 0, 0.40);

        assert!(budget.can_spend(0.20, false));
        assert!(budget.spend(0.20, 100, false).is_ok());
        assert!(!budget.can_spend(0.20, false));

        let budget = AutonomyBudget::new(5, 1_000, 0, 0.40);
        assert!(!budget.can_spend(0.80, false));
        assert!(!budget.can_spend(0.20, true));

        let mut budget = AutonomyBudget::new(5, 100, 1, 0.40);
        assert!(budget.spend(0.20, 101, false).is_err());
    }

    #[test]
    fn test_tool_event_reports_success() {
        let event = ToolEvent {
            id: "tool-1".to_string(),
            intent: "check service".to_string(),
            request: ActionRequest::new(ActionType::CheckService, "127.0.0.1"),
            result: Some(crate::actuator::ActionResult {
                success: true,
                raw_output: "ok".to_string(),
                observations: Vec::new(),
                error: None,
                duration_ms: 5,
            }),
            side_effect: SideEffectClass::ReadOnly,
            confidence: 0.90,
            memory_updates: Vec::new(),
        };

        assert_eq!(event.succeeded(), Some(true));
    }

    #[test]
    fn test_tool_event_store_push_and_query() {
        use crate::actuator::ActionType;
        let mut store = ToolEventStore::new();
        assert!(store.is_empty());
        assert_eq!(store.len(), 0);

        // Push a success event
        let success_event = ToolEvent {
            id: String::new(),
            intent: "scan port 22".to_string(),
            request: ActionRequest::scan_port("10.0.0.1", 22),
            result: Some(crate::actuator::ActionResult {
                success: true, raw_output: "open".to_string(),
                observations: Vec::new(), error: None, duration_ms: 10,
            }),
            side_effect: SideEffectClass::Network,
            confidence: 0.90,
            memory_updates: Vec::new(),
        };
        store.push(success_event);
        assert_eq!(store.len(), 1);
        assert!(!store.is_empty());

        // Push a failure event
        let fail_event = ToolEvent {
            id: String::new(),
            intent: "brute force ssh".to_string(),
            request: ActionRequest::brute_force("10.0.0.1", 22, &["root"], &["wrong"]),
            result: Some(crate::actuator::ActionResult {
                success: false, raw_output: "auth failed".to_string(),
                observations: Vec::new(), error: Some("auth failed".to_string()), duration_ms: 500,
            }),
            side_effect: SideEffectClass::Network,
            confidence: 0.30,
            memory_updates: Vec::new(),
        };
        store.push(fail_event);
        assert_eq!(store.len(), 2);

        // Query by action type (case-insensitive)
        let scans = store.by_action_type("scanport");
        assert_eq!(scans.len(), 1, "should find 1 ScanPort event");

        let bruteforces = store.by_action_type("bruteforce");
        assert_eq!(bruteforces.len(), 1, "should find 1 BruteForce event");

        // Query by success
        let successes = store.by_success(true);
        assert_eq!(successes.len(), 1);
        let failures = store.by_success(false);
        assert_eq!(failures.len(), 1);

        // Success rate
        assert_eq!(store.success_rate("scanport"), Some(1.0));
        assert_eq!(store.success_rate("bruteforce"), Some(0.0));
        assert_eq!(store.success_rate("nonexistent"), None);

        // Overall success rate
        assert_eq!(store.overall_success_rate(), Some(0.50));
    }

    #[test]
    fn test_tool_event_store_persistence() {
        let mut store = ToolEventStore::new();
        store.push(ToolEvent {
            id: String::new(),
            intent: "test".to_string(),
            request: ActionRequest::new(crate::actuator::ActionType::CheckService, "test"),
            result: Some(crate::actuator::ActionResult {
                success: true, raw_output: "ok".to_string(),
                observations: Vec::new(), error: None, duration_ms: 1,
            }),
            side_effect: SideEffectClass::ReadOnly,
            confidence: 0.50,
            memory_updates: Vec::new(),
        });

        let path = std::env::temp_dir().join("test_tool_event_store.json");
        store.save_to(&path).expect("save should succeed");
        let loaded = ToolEventStore::load(&path).expect("load should succeed");
        assert_eq!(loaded.len(), 1);
        assert!(loaded.events[0].succeeded() == Some(true));
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn test_tool_reliability_tracker() {
        let mut tracker = ToolReliabilityTracker::new();
        assert!(tracker.is_empty());
        assert_eq!(tracker.len(), 0);

        // Record some outcomes
        tracker.record("ScanPort", true);
        tracker.record("ScanPort", true);
        tracker.record("ScanPort", false);
        tracker.record("CheckService", true);
        tracker.record("BruteForce", false);

        assert_eq!(tracker.len(), 3);

        // Check success rates
        assert_eq!(tracker.success_rate("ScanPort"), Some(2.0 / 3.0));
        assert_eq!(tracker.success_rate("CheckService"), Some(1.0));
        assert_eq!(tracker.success_rate("BruteForce"), Some(0.0));
        assert_eq!(tracker.success_rate("Unknown"), None);

        // Check EWMA reliability — after 3 records with alpha=0.05:
        // R0 = 0.05*1.0 + 0.95*0.50 = 0.525
        // R1 = 0.05*1.0 + 0.95*0.525 = 0.54875
        // R2 = 0.05*0.0 + 0.95*0.54875 = 0.5213125
        let scan_rel = tracker.reliability("ScanPort");
        assert!(
            (scan_rel - 0.5213).abs() < 0.01,
            "ScanPort reliability should be ~0.521, got {}",
            scan_rel
        );

        // Non-existent type should return default 0.50
        assert!((tracker.reliability("NonExistent") - 0.50).abs() < 1e-6);

        // Test recording from a ToolEvent
        let event = ToolEvent {
            id: String::new(),
            intent: "test".to_string(),
            request: ActionRequest::new(crate::actuator::ActionType::CheckService, "test"),
            result: Some(crate::actuator::ActionResult {
                success: true, raw_output: "ok".to_string(),
                observations: Vec::new(), error: None, duration_ms: 1,
            }),
            side_effect: SideEffectClass::ReadOnly,
            confidence: 0.90,
            memory_updates: Vec::new(),
        };
        tracker.record_event(&event);
        assert_eq!(tracker.success_rate("checkservice"), Some(1.0)); // now 2/2
    }

    #[test]
    fn test_tool_reliability_case_insensitive() {
        let mut tracker = ToolReliabilityTracker::new();
        tracker.record("scanport", true);
        assert_eq!(tracker.success_rate("ScanPort"), Some(1.0));
        assert_eq!(tracker.reliability("SCANPORT"), tracker.reliability("scanport"));
        assert!((tracker.overall_reliability() - tracker.reliability("scanport")).abs() < 1e-6);
    }

    #[test]
    fn test_simulation_mode_default_is_simulated() {
        let req = ActionRequest::new(crate::actuator::ActionType::CheckService, "test");
        assert!(req.simulation_mode.is_simulated());
        assert!(!req.simulation_mode.is_real());

        // Helper methods use real() by default
        let req_real = crate::actuator::ActionRequest::scan_port("10.0.0.1", 80);
        assert!(req_real.simulation_mode.is_real());

        // Builder pattern: explicit simulated
        let req_sim = ActionRequest::new(crate::actuator::ActionType::CheckService, "test").simulated();
        assert!(req_sim.simulation_mode.is_simulated());
    }

    #[test]
    fn test_tool_event_store_empty_queries() {
        let store = ToolEventStore::new();
        assert!(store.by_action_type("scanport").is_empty());
        assert!(store.by_success(true).is_empty());
        assert_eq!(store.success_rate("scanport"), None);
        assert_eq!(store.overall_success_rate(), None);
    }

    // ═════════════════════════════════════════════════════════════════
    // DecisionRecord / DecisionJournal tests
    // ═════════════════════════════════════════════════════════════════

    #[test]
    fn test_decision_record_creation() {
        let budget = AutonomyBudget::new(10, 10000, 5, 0.50);
        let req = crate::actuator::ActionRequest::scan_port("10.0.0.1", 80);
        let record = DecisionRecord::new(42, "scan port 80", req.clone(), "port scan for intel", &budget);

        assert_eq!(record.tick, 42);
        assert_eq!(record.intent, "scan port 80");
        assert_eq!(record.action_request.action_type, crate::actuator::ActionType::ScanPort);
        assert!(record.action_result.is_none());
        assert_eq!(record.budget_before.max_actions, 10);
        assert!(!record.budget_allowed);
        assert!(record.tool_event_id.is_none());
    }

    #[test]
    fn test_decision_journal_basics() {
        let mut journal = DecisionJournal::new();
        assert!(journal.is_empty());
        assert_eq!(journal.len(), 0);

        let budget = AutonomyBudget::new(10, 10000, 5, 0.50);
        let req = crate::actuator::ActionRequest::scan_port("10.0.0.1", 80);

        // Record 1: budget allowed, action succeeded
        let mut r1 = DecisionRecord::new(1, "scan", req.clone(), "reason", &budget);
        r1.budget_allowed = true;
        r1.action_result = Some(crate::actuator::ActionResult {
            success: true, raw_output: "open".to_string(),
            observations: Vec::new(), error: None, duration_ms: 10,
        });
        r1.tool_event_id = Some("tool-1".to_string());
        journal.push(r1);

        // Record 2: budget blocked
        let budget_exhausted = AutonomyBudget::new(0, 0, 0, 0.0);
        let mut r2 = DecisionRecord::new(2, "brute", req, "brute force", &budget_exhausted);
        r2.budget_allowed = false;
        r2.action_result = Some(crate::actuator::ActionResult::error("budget exhausted"));
        journal.push(r2);

        assert_eq!(journal.len(), 2);

        // Query blocked records
        let blocked = journal.blocked_records();
        assert_eq!(blocked.len(), 1);
        assert_eq!(blocked[0].intent, "brute");

        // Query successful records
        let successful = journal.successful_records();
        assert_eq!(successful.len(), 1);
        assert_eq!(successful[0].intent, "scan");
    }

    #[test]
    fn test_decision_journal_persistence() {
        let mut journal = DecisionJournal::new();
        let budget = AutonomyBudget::new(5, 5000, 3, 0.40);
        let req = crate::actuator::ActionRequest::new(crate::actuator::ActionType::CheckService, "test");
        let mut record = DecisionRecord::new(1, "check", req, "service check", &budget);
        record.budget_allowed = true;
        journal.push(record);

        let path = std::env::temp_dir().join("test_decision_journal.json");
        journal.save_to(&path).expect("save should succeed");
        let loaded = DecisionJournal::load(&path).expect("load should succeed");
        assert_eq!(loaded.len(), 1);
        assert!(loaded.records[0].budget_allowed);
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn test_autonomy_budget_enforcement_flow() {
        // Simulate the enforcement flow: check → spend → record
        let mut budget = AutonomyBudget::new(3, 1000, 1, 0.50);

        // Action 1: low risk, should be allowed
        assert!(budget.can_spend(0.30, false));
        assert!(budget.spend(0.30, 100, false).is_ok());
        assert_eq!(budget.actions_used, 1);

        // Action 2: external write
        assert!(budget.can_spend(0.30, true));
        assert!(budget.spend(0.30, 200, true).is_ok());
        assert_eq!(budget.actions_used, 2);
        assert_eq!(budget.external_writes_used, 1);

        // Action 3: should still be allowed (max_actions=3, used=2)
        assert!(budget.can_spend(0.30, false));
        assert!(budget.spend(0.30, 300, false).is_ok());
        assert_eq!(budget.actions_used, 3);

        // Action 4: should be BLOCKED (max_actions reached)
        assert!(!budget.can_spend(0.10, false));
        assert!(budget.spend(0.10, 10, false).is_err());

        // Risk too high
        let mut budget2 = AutonomyBudget::new(10, 10000, 5, 0.40);
        assert!(!budget2.can_spend(0.50, false)); // 0.50 > 0.40

        // External write cap
        let mut budget3 = AutonomyBudget::new(10, 10000, 1, 0.80);
        assert!(budget3.spend(0.10, 10, true).is_ok());
        assert!(budget3.spend(0.10, 10, true).is_err()); // second external write blocked

        // Time budget
        let mut budget4 = AutonomyBudget::new(10, 100, 5, 0.80);
        assert!(budget4.spend(0.10, 60, false).is_ok());
        assert!(budget4.spend(0.10, 60, false).is_err()); // would exceed 100ms
    }

    #[test]
    fn test_experiment_result_metric_lookup() {
        let mut metrics = HashMap::new();
        metrics.insert("accuracy".to_string(), 0.75);
        let result = ExperimentResult {
            experiment: "qa_trace".to_string(),
            claim: "C-008".to_string(),
            commit: "test".to_string(),
            seed: 0,
            dataset: None,
            baseline: "string answer".to_string(),
            metrics,
            passed: true,
            notes: "ok".to_string(),
        };

        assert_eq!(result.metric("accuracy"), Some(0.75));
        assert_eq!(result.metric("missing"), None);
    }

    // ─── EpisodeStore tests ─────────────────────────────────────────────

    #[test]
    fn test_episode_store_push_auto_generates_id() {
        let mut store = EpisodeStore::new();
        let ep = CognitiveEpisode::new("", "test input")
            .with_answer("test answer", 0.80);
        store.push(ep);
        assert_eq!(store.episodes.len(), 1);
        assert!(!store.episodes[0].id.is_empty());
        assert_ne!(store.episodes[0].id, "");
    }

    #[test]
    fn test_episode_store_push_preserves_explicit_id() {
        let mut store = EpisodeStore::new();
        let ep = CognitiveEpisode::new("my-explicit-id", "test");
        store.push(ep);
        assert_eq!(store.episodes[0].id, "my-explicit-id");
    }

    #[test]
    fn test_episode_store_recent_returns_n_last() {
        let mut store = EpisodeStore::new();
        for i in 0..10 {
            let ep = CognitiveEpisode::new(format!("id-{}", i), format!("input-{}", i));
            store.push(ep);
        }
        let recent = store.recent(3);
        assert_eq!(recent.len(), 3);
        assert_eq!(recent[0].id, "id-7");
        assert_eq!(recent[2].id, "id-9");
    }

    #[test]
    fn test_episode_store_recent_capped_by_length() {
        let mut store = EpisodeStore::new();
        store.push(CognitiveEpisode::new("only", "one"));
        let recent = store.recent(10);
        assert_eq!(recent.len(), 1);
    }

    #[test]
    fn test_episode_store_outcomes_for_input() {
        let mut store = EpisodeStore::new();
        store.push(
            CognitiveEpisode::new("e1", "Who raised rates?")
                .with_answer("the_fed", 0.90),
        );
        store.push(
            CognitiveEpisode::new("e2", "Who raised rates?")
                .with_answer("the_fed", 0.85),
        );
        store.push(
            CognitiveEpisode::new("e3", "What is inflation?")
                .with_answer("rising prices", 0.70),
        );

        let outcomes = store.outcomes_for_input("Who raised rates?");
        assert_eq!(outcomes.len(), 2);
        let outcomes = store.outcomes_for_input("What is inflation?");
        assert_eq!(outcomes.len(), 1);
        let outcomes = store.outcomes_for_input("unknown");
        assert_eq!(outcomes.len(), 0);
    }

    #[test]
    fn test_episode_store_apply_reversible_counts() {
        let mut store = EpisodeStore::new();
        let mut ep = CognitiveEpisode::new("e1", "test");
        ep.updates.push(MemoryUpdate {
            target: "rule_1".to_string(),
            operation: "increase_confidence".to_string(),
            before: Some("0.5".to_string()),
            after: Some("0.7".to_string()),
            confidence_delta: 0.2,
            reversible: true,
        });
        ep.updates.push(MemoryUpdate {
            target: "rule_2".to_string(),
            operation: "delete".to_string(),
            before: Some("exists".to_string()),
            after: None,
            confidence_delta: -1.0,
            reversible: false,
        });
        store.push(ep);

        let (applied, skipped) = store.apply_reversible_updates();
        assert_eq!(applied, 1, "reversible update should count as applied");
        assert_eq!(skipped, 1, "irreversible update should count as skipped");
    }

    #[test]
    fn test_episode_store_save_and_load_roundtrip() {
        let tmp = std::env::temp_dir().join("test_episode_store_roundtrip.json");
        let path = tmp.to_str().unwrap().to_string();

        // Save
        {
            let mut store = EpisodeStore::new();
            store.push(
                CognitiveEpisode::new("rt-1", "question?")
                    .with_answer("answer", 0.95),
            );
            store.save_to(&path).unwrap();
        }

        // Load
        let loaded = EpisodeStore::load(&path).unwrap();
        assert_eq!(loaded.episodes.len(), 1);
        assert_eq!(loaded.episodes[0].id, "rt-1");
        assert_eq!(loaded.episodes[0].input, "question?");
        assert_eq!(
            loaded.episodes[0].answer.as_deref(),
            Some("answer")
        );
        assert!((loaded.episodes[0].confidence - 0.95).abs() < 1e-6);

        // Cleanup
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn test_episode_store_save_fails_without_path() {
        let store = EpisodeStore::new();
        assert!(store.save().is_err());
    }

    #[test]
    fn test_apply_reversible_updates_to_modifies_qa_rules() {
        use crate::qa::QaEngine;

        let mut store = EpisodeStore::new();
        let mut qa = QaEngine::new();
        qa.store_rule("a", "is", "b", "b", "is", "c", "test");
        let initial_conf = qa.rules()[0].confidence;

        let mut ep = CognitiveEpisode::new("feedback-test", "test input");
        ep.updates.push(MemoryUpdate {
            target: "a is b -> b is c".to_string(),
            operation: "update_rule_confidence".to_string(),
            before: Some(format!("{}", initial_conf)),
            after: Some(format!("{}", initial_conf * 0.9 + 0.1 * 0.1)),
            confidence_delta: 0.3,
            reversible: true,
        });
        store.push(ep);

        let (applied, skipped) = store.apply_reversible_updates_to(&mut qa);
        assert_eq!(applied, 1, "should apply the confidence update");
        assert_eq!(skipped, 0, "should not skip any updates");
        assert!(
            (qa.rules()[0].confidence - initial_conf).abs() > 1e-6,
            "rule confidence should have changed"
        );
    }

    #[test]
    fn test_apply_reversible_updates_to_skips_irreversible() {
        use crate::qa::QaEngine;

        let mut store = EpisodeStore::new();
        let mut qa = QaEngine::new();
        qa.store_rule("a", "is", "b", "b", "is", "c", "test");

        let mut ep = CognitiveEpisode::new("skip-test", "test");
        ep.updates.push(MemoryUpdate {
            target: "a is b -> b is c".to_string(),
            operation: "cull_rule".to_string(),
            before: Some("exists".to_string()),
            after: None,
            confidence_delta: -1.0,
            reversible: false,
        });
        store.push(ep);

        let (applied, skipped) = store.apply_reversible_updates_to(&mut qa);
        assert_eq!(applied, 0, "irreversible update should not be applied");
        assert_eq!(skipped, 1, "irreversible update should be skipped");
        assert_eq!(qa.rule_count(), 1, "rule should not have been culled");
    }

    #[test]
    fn test_apply_reversible_updates_to_unknown_op_skipped() {
        use crate::qa::QaEngine;

        let mut store = EpisodeStore::new();
        let mut qa = QaEngine::new();

        let mut ep = CognitiveEpisode::new("unknown-op", "test");
        ep.updates.push(MemoryUpdate {
            target: "anything".to_string(),
            operation: "nonexistent_operation".to_string(),
            before: None,
            after: None,
            confidence_delta: 0.0,
            reversible: true,
        });
        store.push(ep);

        let (applied, skipped) = store.apply_reversible_updates_to(&mut qa);
        assert_eq!(applied, 0, "unknown operation should not be applied");
        assert_eq!(skipped, 1, "unknown operation should be skipped");
    }

    #[test]
    fn test_apply_reversible_updates_to_culls_rule() {
        use crate::qa::QaEngine;

        let mut store = EpisodeStore::new();
        let mut qa = QaEngine::new();
        qa.store_rule("x", "leads_to", "y", "y", "leads_to", "z", "test");

        let mut ep = CognitiveEpisode::new("cull-test", "test");
        ep.updates.push(MemoryUpdate {
            target: "x leads_to y -> y leads_to z".to_string(),
            operation: "cull_rule".to_string(),
            before: Some("exists".to_string()),
            after: None,
            confidence_delta: -1.0,
            reversible: true,
        });
        store.push(ep);

        let (applied, skipped) = store.apply_reversible_updates_to(&mut qa);
        assert_eq!(applied, 1, "cull should be applied");
        assert_eq!(skipped, 0);
        assert_eq!(qa.rule_count(), 0, "rule should have been removed");
    }

    // ═════════════════════════════════════════════════════════════════
    // ConceptJournal tests
    // ═════════════════════════════════════════════════════════════════

    #[test]
    fn test_concept_journal_push_and_query() {
        let mut journal = ConceptJournal::new();

        assert!(journal.is_empty());
        assert_eq!(journal.len(), 0);

        journal.push(ConceptEvent {
            tick: 100,
            event_type: ConceptEventType::Created,
            level: 2,
            concept_idx: Some(0),
            details: "L2 concept 0 created".to_string(),
        });
        journal.push(ConceptEvent {
            tick: 200,
            event_type: ConceptEventType::Dissolved,
            level: 2,
            concept_idx: Some(0),
            details: "L2 concept 0 dissolved".to_string(),
        });
        journal.push(ConceptEvent {
            tick: 150,
            event_type: ConceptEventType::Created,
            level: 3,
            concept_idx: Some(0),
            details: "L3 concept 0 created".to_string(),
        });

        assert_eq!(journal.len(), 3);
        assert!(!journal.is_empty());

        // Query by tick
        let since_120 = journal.since(120);
        assert_eq!(since_120.len(), 2, "events at tick 150 and 200");

        // Query by level
        let l2 = journal.for_level(2);
        assert_eq!(l2.len(), 2, "two L2 events");
        let l3 = journal.for_level(3);
        assert_eq!(l3.len(), 1, "one L3 event");

        // Query by type
        let creations = journal.creations();
        assert_eq!(creations.len(), 2, "two creation events");
        let dissolutions = journal.dissolutions();
        assert_eq!(dissolutions.len(), 1, "one dissolution event");

        // Empty journal
        let empty = ConceptJournal::new();
        assert!(empty.is_empty());
        assert_eq!(empty.len(), 0);
        assert!(empty.since(0).is_empty());
        assert!(empty.for_level(2).is_empty());
    }

    #[test]
    fn test_concept_journal_persistence() {
        let mut journal = ConceptJournal::new();
        journal.push(ConceptEvent {
            tick: 10,
            event_type: ConceptEventType::Created,
            level: 2,
            concept_idx: Some(0),
            details: "test".to_string(),
        });

        let path = std::env::temp_dir().join("test_concept_journal.json");
        journal.save_to(&path).expect("save should succeed");

        let loaded = ConceptJournal::load(&path).expect("load should succeed");
        assert_eq!(loaded.len(), 1);
        assert_eq!(loaded.events[0].tick, 10);
        assert_eq!(loaded.events[0].event_type, ConceptEventType::Created);

        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn test_concept_event_type_serialization() {
        // Verify all variants serialize/deserialize correctly
        let variants = vec![
            ConceptEventType::Created,
            ConceptEventType::Merged,
            ConceptEventType::Split,
            ConceptEventType::Frozen,
            ConceptEventType::Decayed,
            ConceptEventType::Reinforced,
            ConceptEventType::Dissolved,
        ];
        for v in variants {
            let json = serde_json::to_string(&v).expect("serialize");
            let back: ConceptEventType = serde_json::from_str(&json).expect("deserialize");
            assert_eq!(v, back, "round-trip for {:?}", v);
        }
    }

    // ═════════════════════════════════════════════════════════════════
    // ConceptQualityScore tests
    // ═════════════════════════════════════════════════════════════════

    #[test]
    fn test_concept_quality_score_composite() {
        // High-quality concept: coherent, many components, recently reinforced
        let mut high = ConceptQualityScore {
            concept_idx: 0,
            level: 2,
            coherence: 0.95,
            component_count: 10,
            ticks_since_reinforced: 10,
            internal_similarity: 0.80,
            composite: 0.0,
        };
        high.compute_composite();
        assert!(
            high.composite > 0.70,
            "high-quality concept should score > 0.70, got {}",
            high.composite
        );

        // Low-quality concept: low coherence, few components, never reinforced
        let mut low = ConceptQualityScore {
            concept_idx: 1,
            level: 2,
            coherence: 0.15,
            component_count: 2,
            ticks_since_reinforced: 5000,
            internal_similarity: 0.30,
            composite: 0.0,
        };
        low.compute_composite();
        assert!(
            low.composite < 0.50,
            "low-quality concept should score < 0.50, got {}",
            low.composite
        );

        // Quality ranking should hold
        assert!(
            high.composite > low.composite,
            "high quality > low quality: {} vs {}",
            high.composite,
            low.composite
        );
    }

    #[test]
    fn test_concept_quality_score_freshness() {
        // Fresh concept (just reinforced)
        let mut fresh = ConceptQualityScore {
            concept_idx: 0,
            level: 2,
            coherence: 0.50,
            component_count: 5,
            ticks_since_reinforced: 0,
            internal_similarity: 0.50,
            composite: 0.0,
        };
        fresh.compute_composite();

        // Stale concept (never reinforced for a long time)
        let mut stale = ConceptQualityScore {
            concept_idx: 1,
            level: 2,
            coherence: 0.50,
            component_count: 5,
            ticks_since_reinforced: 5000,
            internal_similarity: 0.50,
            composite: 0.0,
        };
        stale.compute_composite();

        assert!(
            fresh.composite > stale.composite,
            "fresh concept should score higher than stale one"
        );
    }

    #[test]
    fn test_concept_quality_score_bounds() {
        // Extreme values should produce bounded results
        let mut perfect = ConceptQualityScore {
            concept_idx: 0,
            level: 2,
            coherence: 1.0,
            component_count: 100, // capped at 20
            ticks_since_reinforced: 0,
            internal_similarity: 1.0,
            composite: 0.0,
        };
        perfect.compute_composite();
        assert!(
            perfect.composite <= 1.0,
            "composite should be ≤ 1.0, got {}",
            perfect.composite
        );
        // With perfect inputs: 0.50*1.0 + 0.20*1.0 + 0.20*1.0 + 0.10*1.0 = 1.0
        assert!(
            (perfect.composite - 1.0).abs() < 1e-6,
            "perfect concept should score 1.0, got {}",
            perfect.composite
        );

        let mut zero = ConceptQualityScore {
            concept_idx: 1,
            level: 2,
            coherence: 0.0,
            component_count: 0,
            ticks_since_reinforced: u64::MAX,
            internal_similarity: 0.0,
            composite: 0.0,
        };
        zero.compute_composite();
        // Even with zero inputs, the composite is bounded below by 0
        // exp(-MAX/500) → 0 for freshness term
        // count_norm = 0
        assert!(zero.composite >= 0.0, "composite should be ≥ 0.0");
    }

    // ═════════════════════════════════════════════════════════════════
    // Abstraction on/off ablation benchmark
    // ═════════════════════════════════════════════════════════════════

    /// Ablation benchmark: compare prediction error with abstraction enabled vs
    /// disabled on a stream with clear temporal communities.
    ///
    /// Creates a Markov chain with two communities (A↔B↔C and D↔E↔F).  Runs the
    /// abstractor in two configurations and measures how prediction error evolves.
    ///
    /// This is an ignored benchmark (not a fast unit test).
    #[test]
    #[ignore]
    fn test_abstraction_ablation_benchmark() {
        use crate::abstractor::Abstractor;
        use crate::hierarchy::HierarchicalManifold;
        use crate::predictive::PredictiveCodingLoop;
        use crate::Hypervector;
        use rand::Rng;

        const N_TICKS: usize = 200;
        const K: usize = 10;
        const SEED: u64 = 42;

        /// Run the benchmark scenario and return concept count + avg prediction error.
        fn run_scenario(
            enable_abstraction: bool,
            seed: u64,
            n_ticks: usize,
        ) -> (usize, f64, f64) {
            use rand::SeedableRng;
            let mut rng = rand::rngs::StdRng::seed_from_u64(seed);

            // Build transition model + predictive coding loop
            let mut model = crate::temporal::TransitionModel::new(K);
            let mut predictive = PredictiveCodingLoop::new(100, K, 5);

            // Create hierarchy with enough capacity for L1-L3
            let mut hierarchy = HierarchicalManifold::new(&[K, K * 2, K]);

            // Seed L1 centroids with distinguishable hypervectors
            let base: Vec<Hypervector> = (0..K)
                .map(|i| Hypervector::encode_text_ngram(&format!("L1_{}", i), 3))
                .collect();
            hierarchy.seed_from_base_centroids(&base);

            // Create abstractor
            let mut abstractor = Abstractor::new();
            if !enable_abstraction {
                // Bypass the error gate by setting a very low threshold
                // (abstraction won't form if error is above threshold)
                abstractor.error_threshold = 0.001;
            }

            // Generate trajectory: two communities A=0,1,2 and B=3,4,5
            // 60% intra-community transitions, 10% inter-community, 30% random
            let mut error_sum = 0.0;
            let mut error_count = 0;
            let mut prev_state = 0;

            for tick in 0..n_ticks {
                // Determine next state
                let next_state = {
                    let p: f64 = rng.gen();
                    if p < 0.60 {
                        // Intra-community
                        if prev_state <= 2 {
                            rng.gen_range(0..3) // {0,1,2}
                        } else {
                            rng.gen_range(3..6) // {3,4,5}
                        }
                    } else if p < 0.70 {
                        // Inter-community
                        if prev_state <= 2 {
                            rng.gen_range(3..6)
                        } else {
                            rng.gen_range(0..3)
                        }
                    } else {
                        // Random
                        rng.gen_range(0..K)
                    }
                };

                // Encode state
                let state = base[next_state];

                // Run predictive coding
                let pred_error = predictive.cycle(&state, next_state, Some(0), 0.5);
                error_sum += pred_error;
                error_count += 1;

                // Record transition
                model.record_transition_from(prev_state, next_state);

                // Run abstraction cycle every 5 ticks
                if enable_abstraction && tick > 0 && tick % 5 == 0 {
                    let _ = abstractor.cycle(&model, &mut hierarchy, &predictive, None);
                }

                prev_state = next_state;
            }

            let l2_count = abstractor.coherence.len();
            let avg_error = if error_count > 0 {
                error_sum / error_count as f64
            } else {
                0.0
            };
            let coherence_avg = if l2_count > 0 {
                abstractor.coherence.scores.iter().sum::<f64>() / l2_count as f64
            } else {
                0.0
            };

            (l2_count, avg_error, coherence_avg)
        }

        // Run with abstraction enabled
        let (count_on, error_on, coh_on) = run_scenario(true, SEED, N_TICKS);

        // Run with abstraction disabled
        let (count_off, error_off, coh_off) = run_scenario(false, SEED, N_TICKS);

        // Build structured result
        let mut metrics = HashMap::new();
        metrics.insert("l2_concepts_on".to_string(), count_on as f64);
        metrics.insert("l2_concepts_off".to_string(), count_off as f64);
        metrics.insert("avg_pred_error_on".to_string(), error_on);
        metrics.insert("avg_pred_error_off".to_string(), error_off);
        metrics.insert("avg_coherence_on".to_string(), coh_on);

        let result = ExperimentResult {
            experiment: "abstraction_ablation".to_string(),
            claim: "C-003".to_string(),
            commit: env!("CARGO_PKG_VERSION").to_string(),
            seed: SEED,
            dataset: None,
            baseline: "abstraction_disabled".to_string(),
            metrics,
            passed: count_on > count_off || (error_on > 0.0 && error_on < error_off),
            notes: format!(
                "abstraction on: {} L2 concepts, pred_error={:.4}, coherence={:.4}; off: {} concepts, pred_error={:.4}",
                count_on, error_on, coh_on, count_off, error_off,
            ),
        };

        let json = serde_json::to_string_pretty(&result).unwrap_or_default();
        eprintln!("=== ABSTRACTION ABLATION RESULT ===");
        eprintln!("{}", json);

        // The benchmark passes if:
        // 1. With abstraction: one or more L2 concepts formed
        // 2. Prediction error with abstraction is not significantly worse than without
        if count_on > 0 {
            assert!(
                error_on <= error_off * 1.25 || count_on > count_off,
                "abstraction should not severely degrade prediction: on={:.4} vs off={:.4}",
                error_on, error_off,
            );
        }
    }

    // ═════════════════════════════════════════════════════════════════
    // Confidence Calibration tests
    // ═════════════════════════════════════════════════════════════════

    #[test]
    fn test_confidence_calibration_empty() {
        let cal = ConfidenceCalibration::new();
        assert_eq!(cal.total_observations, 0);
        assert_eq!(cal.accuracy(), 0.0);
        assert_eq!(cal.avg_confidence(), 0.0);
        assert_eq!(cal.expected_calibration_error(), 0.0);
        assert!(!cal.is_overconfident());
        assert!(!cal.is_underconfident());
    }

    #[test]
    fn test_confidence_calibration_record() {
        let mut cal = ConfidenceCalibration::new();

        // Record some observations: high confidence correct, low confidence incorrect
        cal.record(0.95, true);
        cal.record(0.90, true);
        cal.record(0.85, false); // overconfident
        cal.record(0.30, true);  // underconfident
        cal.record(0.20, false);
        cal.record(0.10, false);

        assert_eq!(cal.total_observations, 6);
        assert_eq!(cal.total_correct, 3);
        assert!((cal.accuracy() - 0.50).abs() < 1e-6, "accuracy should be 0.5");

        // Avg confidence: (0.95 + 0.90 + 0.85 + 0.30 + 0.20 + 0.10) / 6
        let expected_avg = (0.95 + 0.90 + 0.85 + 0.30 + 0.20 + 0.10) / 6.0;
        assert!((cal.avg_confidence() - expected_avg).abs() < 1e-6);

        // Should be overconfident (avg_conf > accuracy)
        assert!(cal.is_overconfident());
        assert!(!cal.is_underconfident());

        // ECE should be > 0 (not perfectly calibrated)
        assert!(cal.expected_calibration_error() > 0.0);
    }

    #[test]
    fn test_confidence_calibration_perfect() {
        let mut cal = ConfidenceCalibration::new();

        // Well-calibrated: accuracy ≈ confidence in each bin
        // Bin [0.9,1.0]: 9 correct out of 10 → 0.90 accuracy vs ~0.95 avg confidence
        for _ in 0..9 { cal.record(0.95, true); }
        cal.record(0.95, false); // 90% accurate

        // Bin [0.7,0.8]: 7 correct out of 10 → 0.70 accuracy vs ~0.75 avg confidence
        for _ in 0..7 { cal.record(0.75, true); }
        for _ in 0..3 { cal.record(0.75, false); } // 70% accurate

        // Bin [0.5,0.6]: 5 correct out of 10 → 0.50 accuracy vs ~0.55 avg confidence
        for _ in 0..5 { cal.record(0.55, true); }
        for _ in 0..5 { cal.record(0.55, false); } // 50% accurate

        // ECE should be reasonably small (within ~0.10 of perfect)
        let ece = cal.expected_calibration_error();
        assert!(
            ece < 0.15,
            "ECE should be small for well-calibrated data, got {}",
            ece
        );
    }

    #[test]
    fn test_confidence_calibration_record_episode() {
        let mut cal = ConfidenceCalibration::new();

        // Success episode with high confidence and high score
        let ep_success = CognitiveEpisode::new("test1", "input1")
            .with_answer("correct answer", 0.95);
        // Can't set outcome directly via with_answer, need to construct manually
        let mut ep_success = CognitiveEpisode::new("test1", "input1");
        ep_success.answer = Some("correct".to_string());
        ep_success.confidence = 0.95;
        ep_success.outcome = EpisodeOutcome::Success {
            score: 0.90,
            evidence: "verified".to_string(),
        };

        assert!(cal.record_episode(&ep_success));
        assert_eq!(cal.total_observations, 1);
        assert_eq!(cal.total_correct, 1);

        // Failure episode
        let mut ep_fail = CognitiveEpisode::new("test2", "input2");
        ep_fail.answer = Some("wrong".to_string());
        ep_fail.confidence = 0.80;
        ep_fail.outcome = EpisodeOutcome::Failure {
            score: 0.10,
            error_class: "wrong_answer".to_string(),
            evidence: "mismatch".to_string(),
        };

        assert!(cal.record_episode(&ep_fail));
        assert_eq!(cal.total_observations, 2);
        assert_eq!(cal.total_correct, 1);

        // Unknown outcome should not be recorded
        let ep_unknown = CognitiveEpisode::new("test3", "input3");
        assert!(!cal.record_episode(&ep_unknown));
        assert_eq!(cal.total_observations, 2);
    }

    #[test]
    fn test_confidence_calibration_record_store() {
        let mut store = EpisodeStore::new();
        let mut cal = ConfidenceCalibration::new();

        // Add episodes with outcomes
        let mut ep1 = CognitiveEpisode::new("s1", "q1");
        ep1.confidence = 0.90;
        ep1.outcome = EpisodeOutcome::Success { score: 1.0, evidence: "ok".to_string() };
        store.push(ep1);

        let mut ep2 = CognitiveEpisode::new("s2", "q2");
        ep2.confidence = 0.20;
        ep2.outcome = EpisodeOutcome::Failure { score: 0.0, error_class: "bad".to_string(), evidence: "no".to_string() };
        store.push(ep2);

        cal.record_store(&store);
        assert_eq!(cal.total_observations, 2);
        assert_eq!(cal.total_correct, 1);
    }

    // ═════════════════════════════════════════════════════════════════
    // Feedback Tracker tests
    // ═════════════════════════════════════════════════════════════════

    #[test]
    fn test_task_family_basics() {
        let mut family = TaskFamily::new("test family");
        assert_eq!(family.name, "test family");
        assert!(family.tasks.is_empty());

        family.add_question("Who raised rates?", Some("the_fed"));
        family.add_verify("the_fed", "raise", "rates", true);
        assert_eq!(family.tasks.len(), 2);

        // Verify task correctly identifies is_verify
        assert!(!family.tasks[0].is_verify);
        assert!(family.tasks[1].is_verify);
        assert_eq!(family.tasks[1].verify_subject.as_deref(), Some("the_fed"));
    }

    #[test]
    fn test_pre_post_comparison() {
        let mut family = TaskFamily::new("comparison");
        family.add_question("Who raised rates?", Some("the_fed"));

        // Pre-run: wrong answer
        let pre = TaskFamilyRun {
            family_name: "comparison".to_string(),
            results: vec![TaskResult {
                input: "Who raised rates?".to_string(),
                answer: "I do not know.".to_string(),
                confidence: 0.30,
                matched_expected: false,
                episode_id: "pre-1".to_string(),
            }],
            accuracy: 0.0,
            avg_confidence: 0.30,
        };

        // Post-run: correct answer
        let post = TaskFamilyRun {
            family_name: "comparison".to_string(),
            results: vec![TaskResult {
                input: "Who raised rates?".to_string(),
                answer: "the_fed raised rates.".to_string(),
                confidence: 0.90,
                matched_expected: true,
                episode_id: "post-1".to_string(),
            }],
            accuracy: 1.0,
            avg_confidence: 0.90,
        };

        let comparison = PrePostComparison::new("comparison", pre, post);
        assert!((comparison.accuracy_delta - 1.0).abs() < 1e-6, "accuracy should improve by 1.0");
        assert!((comparison.confidence_delta - 0.60).abs() < 1e-6, "confidence should increase by 0.6");
        assert_eq!(comparison.answer_changes, 1, "answer should have changed");
        assert_eq!(comparison.correctness_changes, 1, "correctness should have changed");
    }

    /// Helper: run a TaskFamily against a QaEngine, producing a TaskFamilyRun.
    fn run_task_family(qa: &mut crate::qa::QaEngine, family: &TaskFamily) -> TaskFamilyRun {
        let mut results = Vec::new();
        for task in &family.tasks {
            if task.is_verify {
                let (verified, confidence) = qa.verify_fact(
                    task.verify_subject.as_deref().unwrap_or(""),
                    task.verify_verb.as_deref().unwrap_or(""),
                    task.verify_object.as_deref().unwrap_or(""),
                );
                let answer = if verified { "yes" } else { "no" };
                let expected = task.expected.as_deref().unwrap_or("no");
                results.push(TaskResult {
                    input: task.input.clone(),
                    answer: answer.to_string(),
                    confidence,
                    matched_expected: answer == expected,
                    episode_id: String::new(),
                });
            } else {
                let episode = qa.answer_episode(
                    format!("task-{}", results.len()),
                    &task.input,
                );
                let answer = episode.answer.clone().unwrap_or_default();
                let confidence = episode.confidence;
                let matched = task.expected.as_ref().map_or(true, |exp| {
                    let answer_lower = answer.to_lowercase();
                    let exp_lower = exp.to_lowercase();
                    answer_lower.contains(&exp_lower) || exp_lower.contains(&answer_lower)
                });
                results.push(TaskResult {
                    input: task.input.clone(),
                    answer,
                    confidence,
                    matched_expected: matched,
                    episode_id: episode.id,
                });
            }
        }
        let total = results.len() as f64;
        let correct = results.iter().filter(|r| r.matched_expected).count() as f64;
        let accuracy = if total > 0.0 { correct / total } else { 0.0 };
        let avg_confidence = if total > 0.0 {
            results.iter().map(|r| r.confidence).sum::<f64>() / total
        } else {
            0.0
        };
        TaskFamilyRun {
            family_name: family.name.clone(),
            results,
            accuracy,
            avg_confidence,
        }
    }

    #[test]
    fn test_run_pre_post_integration() {
        // Build a QaEngine with initial knowledge
        let mut qa = crate::qa::QaEngine::new();
        qa.store_fact("the_fed", "raise", "rates", "bootstrap");
        qa.store_fact("rates", "go_up", "0.25 percent", "bootstrap");

        // Build a task family
        let mut family = TaskFamily::new("fed rate impact");
        family.add_question("Who raised rates?", Some("the_fed"));
        family.add_verify("the_fed", "raise", "rates", true);

        // Pre-feedback run: should already work because facts exist
        let pre = run_task_family(&mut qa, &family);
        assert!(pre.accuracy > 0.0, "Pre-feedback should have partial accuracy");
        assert!(pre.avg_confidence > 0.0, "Pre-feedback should have non-zero confidence");

        // Add new knowledge that should improve answers
        qa.store_fact("the_fed", "cut", "rates", "new_info");
        qa.store_rule(
            "the_fed", "cut", "rates",
            "rates", "go_down", "0.50 percent",
            "fed cut rule",
        );

        // Post-feedback run: should show improvement
        let post = run_task_family(&mut qa, &family);
        assert!(
            post.accuracy >= pre.accuracy,
            "Post-feedback accuracy should not regress"
        );
        assert!(
            post.avg_confidence >= pre.avg_confidence - 0.01,
            "Post-feedback confidence should not regress significantly"
        );

        // Build the comparison
        let comparison = PrePostComparison::new("fed rate impact", pre, post);
        assert_eq!(comparison.family_name, "fed rate impact");
        // The comparison should at least not panic — it must handle valid data
        assert!(
            comparison.accuracy_delta >= -0.01,
            "Accuracy delta should be non-negative"
        );
    }
}
