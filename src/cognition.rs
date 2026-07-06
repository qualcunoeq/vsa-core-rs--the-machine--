//! Shared cognitive audit and evaluation types.
//!
//! These types make the architecture's closed loop explicit: observe, resolve,
//! answer or act, observe the outcome, then decide what changed in memory.

use crate::actuator::{ActionRequest, ActionResult};
use crate::qa::ResolveTrace;
use std::collections::HashMap;

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
}
