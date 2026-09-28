//! Structured records for a single conversational turn.
//!
//! The runtime keeps three kinds of evidence clearly separated:
//! retrieved claims (stored facts), computed answers (deterministic
//! solvers/router output), and proof-checked conclusions (rule chains with
//! replay verification). A turn is never reported as `Answered` unless the
//! underlying engine actually produced a non-abstaining answer.

use serde::{Deserialize, Serialize};

pub type SessionId = String;
pub type TurnId = String;

/// How the runtime interpreted the incoming text.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RequestKind {
    Question,
    Teaching,
    Unsupported,
}

/// A milestone reported while a turn is being processed.
///
/// Stages describe work that actually happened; they are emitted by the
/// service at the point each processing step starts, so an interface can show
/// real progress instead of a simulated animation.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TurnStage {
    Interpreting,
    RetrievingFacts,
    Reasoning,
    CheckingResult,
    UpdatingMemory,
    Rendering,
}

impl TurnStage {
    /// Short human-readable description for progress displays.
    pub fn label(&self) -> &'static str {
        match self {
            TurnStage::Interpreting => "Interpreting request",
            TurnStage::RetrievingFacts => "Retrieving facts",
            TurnStage::Reasoning => "Reasoning over rules",
            TurnStage::CheckingResult => "Checking result",
            TurnStage::UpdatingMemory => "Updating memory",
            TurnStage::Rendering => "Preparing answer",
        }
    }
}

/// Which capability actually served the turn.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Capability {
    FactualQa,
    CausalChain,
    StructuredSolver { domain: String },
    None,
}

impl Capability {
    pub fn id(&self) -> String {
        match self {
            Capability::FactualQa => "factual_qa".to_string(),
            Capability::CausalChain => "causal_chain".to_string(),
            Capability::StructuredSolver { domain } => format!("structured_solver:{domain}"),
            Capability::None => "none".to_string(),
        }
    }
}

/// Machine-readable disposition of a turn.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TurnOutcome {
    Answered,
    ClarificationNeeded { question: String },
    Unsupported { reason: String },
    Failed { error: String },
    Cancelled,
}

impl TurnOutcome {
    pub fn label(&self) -> &'static str {
        match self {
            TurnOutcome::Answered => "answered",
            TurnOutcome::ClarificationNeeded { .. } => "clarification_needed",
            TurnOutcome::Unsupported { .. } => "unsupported",
            TurnOutcome::Failed { .. } => "failed",
            TurnOutcome::Cancelled => "cancelled",
        }
    }

    pub fn is_answered(&self) -> bool {
        matches!(self, TurnOutcome::Answered)
    }

    pub fn is_abstention(&self) -> bool {
        matches!(
            self,
            TurnOutcome::Unsupported { .. } | TurnOutcome::ClarificationNeeded { .. }
        )
    }
}

/// How a follow-up turn was grounded in the conversation so far.
///
/// Recorded for audit and for interfaces that want to show *why* a pronoun or
/// an elliptical request was read the way it was. The exact resolved values
/// are kept as plain strings/ids; nothing here is approximate.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct FollowUpInfo {
    /// What kind of follow-up was recognized: `reference`, `about`, `before`,
    /// `correction`, `resolve_equation`, `explain`.
    pub kind: String,
    /// Surface form -> exact referent, e.g. `("her", "Alice")`.
    #[serde(default)]
    pub resolved: Vec<(String, String)>,
    /// How the referent was chosen: `explicit`, `unique_entity`,
    /// `context_similarity`, or `pending_choice`.
    pub source: String,
    /// Confidence of the binding (1.0 for explicit/unique bindings).
    pub confidence: f64,
}

/// The runtime's reading of the request, recorded for audit.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Interpretation {
    pub kind: RequestKind,
    pub normalized_input: String,
    pub capability: Capability,
    pub notes: Vec<String>,
    /// Set when the turn depended on conversation state (references,
    /// ellipsis, corrections, or solver re-parameterization).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub follow_up: Option<FollowUpInfo>,
}

/// Provenance class for an answer-bearing item.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EvidenceKind {
    RetrievedClaim,
    ComputedAnswer,
    ProofCheckedConclusion,
}

impl EvidenceKind {
    pub fn label(&self) -> &'static str {
        match self {
            EvidenceKind::RetrievedClaim => "retrieved_claim",
            EvidenceKind::ComputedAnswer => "computed_answer",
            EvidenceKind::ProofCheckedConclusion => "proof_checked_conclusion",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct EvidenceRef {
    pub kind: EvidenceKind,
    pub content: String,
    pub provenance: String,
    pub confidence: f64,
    pub replay_verified: Option<bool>,
    /// Durable assertion this evidence came from, when it maps to one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub assertion_id: Option<String>,
}

/// Whether the answer was checked against an independent artifact.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum VerificationStatus {
    NotAttempted,
    Verified { method: String, score: f64 },
    Unverified { reason: String },
}

/// A durable change the turn made to memory.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct MemoryChange {
    pub target: String,
    pub operation: String,
    pub detail: String,
    pub provenance: String,
    pub before: Option<String>,
    pub after: Option<String>,
    pub confidence_delta: f64,
    pub reversible: bool,
    /// Durable assertion created or reaffirmed by this change.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub assertion_id: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TurnTiming {
    pub started_at: String,
    pub elapsed_ms: f64,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TurnDiagnostics {
    pub session_id: SessionId,
    pub turn_id: TurnId,
    pub episode_id: Option<String>,
    pub fact_count_before: usize,
    pub fact_count_after: usize,
    pub rule_count_before: usize,
    pub rule_count_after: usize,
    /// Set when durable storage rejected a write for this turn.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub storage_error: Option<String>,
}

/// The structured result of one turn.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TurnResult {
    pub outcome: TurnOutcome,
    /// Rendered, user-facing answer (includes evidence summary when enabled).
    pub answer_text: String,
    /// Raw engine answer, kept verbatim for audit and downstream tooling.
    pub answer: Option<String>,
    pub interpretation: Interpretation,
    pub evidence: Vec<EvidenceRef>,
    pub capability: Capability,
    pub verification: VerificationStatus,
    pub memory_changes: Vec<MemoryChange>,
    pub timing: TurnTiming,
    pub diagnostics: TurnDiagnostics,
}

impl TurnResult {
    pub fn is_answered(&self) -> bool {
        self.outcome.is_answered()
    }

    pub fn is_abstention(&self) -> bool {
        self.outcome.is_abstention()
    }
}
