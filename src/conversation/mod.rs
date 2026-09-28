//! # Conversational runtime
//!
//! A small application service over the library's existing capabilities,
//! deliberately independent of the multi-agent simulation in `main.rs`.
//!
//! | Component | Type |
//! |---|---|
//! | Conversation service | [`ConversationService`] |
//! | Capability adapters | QA / causal-chain / teaching paths inside [`ConversationService`] |
//! | Conversation store | [`ConversationStore`] |
//! | Memory service | [`MemoryService`] |
//! | Response renderer | [`ResponseRenderer`] |
//!
//! Each turn produces a [`TurnResult`] with a machine-readable outcome
//! (`answered`, `clarification_needed`, `unsupported`, `failed`, `cancelled`),
//! the raw and rendered answers, the interpretation, evidence, the capability
//! used, verification status, memory changes, timing, and diagnostic
//! identifiers. [`TurnOptions`] adds caller-supplied modes and turn ids,
//! cooperative cancellation, and a [`TurnStage`] progress sink; the web
//! interface in `crate::chat_server` is built on those hooks.
//!
//! ```no_run
//! use the_machine::conversation::{ConversationService, TurnOutcome};
//!
//! let mut service = ConversationService::new();
//! let session = service.open_session("demo");
//! service.memory().teach_fact("the_fed", "raise", "rates", "demo");
//! let turn = service.handle_turn(&session, "Who raised rates?");
//! assert_eq!(turn.outcome, TurnOutcome::Answered);
//! assert!(!turn.evidence.is_empty());
//! ```

mod capability_adapter;
mod followup;
mod renderer;
mod service;
mod store;
mod types;

pub use capability_adapter::{CapabilityAttempt, CapabilityDisposition, SupportedCapability};
pub use followup::{FollowUp, Resolution};
pub use renderer::ResponseRenderer;
pub use service::{
    ConversationService, CorrectionReport, DocumentCommitReport, DocumentImport,
    DocumentInspection, DocumentRemovalReport, EvalConfig, KnowledgeOutcome, MemoryService,
    TurnOptions,
};
pub use store::{
    Assumption, ClarificationKind, ConversationSession, ConversationStore, ConversationTurn,
    EntityRef, LastOperation, LastResult, OperationKind, PendingClarification, StaleMark,
    TurnReference, WorkingContext,
};
pub use types::{
    Capability, EvidenceKind, EvidenceRef, FollowUpInfo, Interpretation, MemoryChange, RequestKind,
    SessionId, TurnDiagnostics, TurnId, TurnOutcome, TurnResult, TurnStage, TurnTiming,
    VerificationStatus,
};
