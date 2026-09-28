//! Persistent conversation state: sessions, turns, and pending clarifications.

use std::path::Path;

use serde::{Deserialize, Serialize};

use super::types::{SessionId, TurnResult};

fn now_iso() -> String {
    chrono::Utc::now().to_rfc3339()
}

/// What kind of answer the runtime is waiting for.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ClarificationKind {
    /// A complete subject-verb-object statement or rule was needed.
    #[default]
    MissingDetail,
    /// A pronoun or ellipsis matched several entities; the user must choose.
    AmbiguousReference,
}

/// A clarification the runtime is still waiting on.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PendingClarification {
    pub turn_id: String,
    pub question: String,
    pub missing: String,
    #[serde(default)]
    pub kind: ClarificationKind,
    /// Candidate referents when the clarification is about an ambiguous
    /// reference (exact entity strings, never paraphrases).
    #[serde(default)]
    pub candidates: Vec<String>,
    /// The ambiguous utterance as the user typed it, so a chosen candidate
    /// can be substituted back into the original request.
    #[serde(default)]
    pub original_input: String,
}

impl PendingClarification {
    /// Which candidate (if any) a reply selects. A reply selects a candidate
    /// when it names exactly one of them; naming several keeps the question
    /// open.
    pub fn selected_candidate(&self, reply: &str) -> Option<&str> {
        let normalized = reply.trim().trim_end_matches(['.', '?', '!']).to_lowercase();
        if normalized.is_empty() {
            return None;
        }
        let mut matches: Vec<&str> = self
            .candidates
            .iter()
            .filter(|candidate| {
                let candidate = candidate.to_lowercase();
                normalized == candidate
                    || normalized.contains(&candidate)
                    || candidate.contains(&normalized)
            })
            .map(String::as_str)
            .collect();
        matches.dedup();
        match matches.as_slice() {
            [only] => Some(*only),
            _ => None,
        }
    }
}

/// Why a stored turn's conclusion is no longer trustworthy.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct StaleMark {
    pub reason: String,
    pub at: String,
}

impl StaleMark {
    pub fn new(reason: impl Into<String>) -> Self {
        StaleMark {
            reason: reason.into(),
            at: now_iso(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ConversationTurn {
    pub turn_id: String,
    pub at: String,
    pub input: String,
    pub result: TurnResult,
    /// Set when the knowledge this turn depended on was corrected, retracted,
    /// or forgotten after the turn was recorded.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stale: Option<StaleMark>,
}

impl ConversationTurn {
    pub fn new(turn_id: impl Into<String>, input: impl Into<String>, result: TurnResult) -> Self {
        ConversationTurn {
            turn_id: turn_id.into(),
            at: now_iso(),
            input: input.into(),
            result,
            stale: None,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ConversationSession {
    pub id: SessionId,
    pub created_at: String,
    pub turns: Vec<ConversationTurn>,
    pub pending_clarification: Option<PendingClarification>,
}

/// What the last turn asked the runtime to do.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OperationKind {
    Ask,
    Teach,
    Correct,
    Retract,
    Forget,
    Solve,
    Explain,
    About,
    Before,
    Clarify,
    Other,
}

/// The last requested operation and its arguments, kept as exact key/value
/// pairs (never as hypervectors) so a follow-up can reuse them verbatim.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct LastOperation {
    pub kind: OperationKind,
    pub arguments: Vec<(String, String)>,
    pub turn_id: String,
    pub at: String,
}

impl LastOperation {
    pub fn argument(&self, key: &str) -> Option<&str> {
        self.arguments
            .iter()
            .find(|(name, _)| name == key)
            .map(|(_, value)| value.as_str())
    }
}

/// A compact record of the last result and where it came from.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct LastResult {
    pub turn_id: String,
    pub outcome: String,
    pub answer: Option<String>,
    pub provenance: Vec<String>,
    pub capability: String,
    pub at: String,
}

/// A temporary assumption introduced by a follow-up (for example a new
/// right-hand side for the equation currently being solved). Assumptions are
/// conversation state, not knowledge, and are dropped with the context.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Assumption {
    pub key: String,
    pub value: String,
    pub statement: String,
    pub turn_id: String,
    pub at: String,
}

/// An entity mentioned in the conversation, with the turn that introduced it.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct EntityRef {
    pub text: String,
    pub turn_id: String,
    /// `person` for bare proper nouns, `thing` for article-led noun phrases.
    pub kind: String,
}

/// A reference to an earlier turn that follow-ups can point back at.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TurnReference {
    pub turn_id: String,
    pub kind: String,
    pub summary: String,
    pub at: String,
}

/// Bounded, reconstructable working context for one conversation.
///
/// This is convenience state, never knowledge: the current topic, a short
/// list of salient entities with the turn that introduced each one, the last
/// requested operation and its arguments, the last result and its
/// provenance, temporary assumptions, and the question the runtime is still
/// waiting on. It can always be rebuilt from the session's turns.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, Default)]
pub struct WorkingContext {
    pub topic: Option<String>,
    pub entities: Vec<String>,
    pub pending: Option<PendingClarification>,
    pub updated_at: String,
    #[serde(default)]
    pub last_operation: Option<LastOperation>,
    #[serde(default)]
    pub last_result: Option<LastResult>,
    #[serde(default)]
    pub assumptions: Vec<Assumption>,
    #[serde(default)]
    pub entity_refs: Vec<EntityRef>,
    #[serde(default)]
    pub references: Vec<TurnReference>,
}

impl WorkingContext {
    pub const MAX_ENTITIES: usize = 8;
    pub const MAX_TEXT: usize = 160;
    pub const MAX_ENTITY_REFS: usize = 16;
    pub const MAX_REFERENCES: usize = 8;
    pub const MAX_ASSUMPTIONS: usize = 4;
    pub const MAX_ARGUMENTS: usize = 8;

    /// Observe a finished turn and fold its salient terms into the context.
    pub fn observe_turn(&mut self, result: &TurnResult) {
        let mut candidates: Vec<String> = Vec::new();
        for evidence in &result.evidence {
            candidates.push(evidence.content.clone());
        }
        for change in &result.memory_changes {
            candidates.push(change.detail.clone());
        }
        let turn_id = result.diagnostics.turn_id.clone();
        for candidate in candidates {
            if let Some((subject, object)) = Self::salient_terms(&candidate) {
                if self.topic.is_none() {
                    self.topic = Some(subject.clone());
                }
                Self::push_unique(&mut self.entities, subject.clone());
                Self::push_unique(&mut self.entities, object.clone());
                self.remember_entity(&subject, &turn_id);
                self.remember_entity(&object, &turn_id);
            }
        }
        self.last_result = Some(LastResult {
            turn_id: turn_id.clone(),
            outcome: result.outcome.label().to_string(),
            answer: result.answer.clone(),
            provenance: {
                let mut seen: Vec<String> = Vec::new();
                for item in &result.evidence {
                    if !item.provenance.is_empty() && !seen.contains(&item.provenance) {
                        seen.push(item.provenance.clone());
                    }
                }
                seen.truncate(5);
                seen
            },
            capability: result.capability.id(),
            at: now_iso(),
        });
        self.record_reference(
            &Self::reference_kind(result),
            &result.interpretation.normalized_input,
            &turn_id,
        );
        self.enforce_bounds();
        self.updated_at = now_iso();
    }

    /// Remember which turn introduced an entity (most recent occurrence wins).
    pub fn remember_entity(&mut self, text: &str, turn_id: &str) {
        let text = text.trim();
        if text.is_empty() {
            return;
        }
        self.entity_refs.retain(|reference| reference.text != text);
        self.entity_refs.push(EntityRef {
            text: text.to_string(),
            turn_id: turn_id.to_string(),
            kind: Self::entity_kind(text).to_string(),
        });
        if self.entity_refs.len() > Self::MAX_ENTITY_REFS {
            let overflow = self.entity_refs.len() - Self::MAX_ENTITY_REFS;
            self.entity_refs.drain(0..overflow);
        }
    }

    /// Record the operation the last turn performed.
    pub fn record_operation(
        &mut self,
        kind: OperationKind,
        arguments: Vec<(String, String)>,
        turn_id: &str,
    ) {
        let mut arguments = arguments;
        arguments.truncate(Self::MAX_ARGUMENTS);
        self.last_operation = Some(LastOperation {
            kind,
            arguments,
            turn_id: turn_id.to_string(),
            at: now_iso(),
        });
    }

    /// Record a pointer to an earlier turn.
    pub fn record_reference(&mut self, kind: &str, summary: &str, turn_id: &str) {
        let mut summary = summary.trim().to_string();
        Self::truncate(&mut summary);
        self.references.retain(|reference| reference.turn_id != turn_id);
        self.references.push(TurnReference {
            turn_id: turn_id.to_string(),
            kind: kind.to_string(),
            summary,
            at: now_iso(),
        });
        if self.references.len() > Self::MAX_REFERENCES {
            let overflow = self.references.len() - Self::MAX_REFERENCES;
            self.references.drain(0..overflow);
        }
    }

    /// Add a temporary assumption, replacing an earlier one with the same key.
    pub fn add_assumption(&mut self, assumption: Assumption) {
        self.assumptions
            .retain(|existing| existing.key != assumption.key);
        self.assumptions.push(assumption);
        if self.assumptions.len() > Self::MAX_ASSUMPTIONS {
            let overflow = self.assumptions.len() - Self::MAX_ASSUMPTIONS;
            self.assumptions.drain(0..overflow);
        }
    }

    pub fn assumption(&self, key: &str) -> Option<&Assumption> {
        self.assumptions.iter().find(|item| item.key == key)
    }

    /// Entities in the order they were last mentioned (oldest first).
    pub fn entity_ref(&self, text: &str) -> Option<&EntityRef> {
        self.entity_refs
            .iter()
            .find(|reference| reference.text.eq_ignore_ascii_case(text))
    }

    /// A bare capitalized proper noun is treated as a person ("Alice",
    /// "Bob"); article-led phrases are things ("the observatory").
    pub fn entity_kind(text: &str) -> &'static str {
        let trimmed = text.trim();
        let first = trimmed.chars().next();
        let article_led = ["the ", "a ", "an "]
            .iter()
            .any(|article| trimmed.to_lowercase().starts_with(article));
        match first {
            Some(character)
                if character.is_uppercase()
                    && !article_led
                    && !trimmed.contains(' ')
                    && trimmed.chars().all(|c| c.is_alphabetic() || c == '\'') =>
            {
                "person"
            }
            _ => "thing",
        }
    }

    fn reference_kind(result: &TurnResult) -> String {
        match result.interpretation.kind {
            super::types::RequestKind::Teaching => "teach".to_string(),
            super::types::RequestKind::Question => match &result.capability {
                super::types::Capability::StructuredSolver { .. } => "solve".to_string(),
                super::types::Capability::CausalChain => "chain".to_string(),
                _ => "ask".to_string(),
            },
            super::types::RequestKind::Unsupported => match result.outcome {
                super::types::TurnOutcome::ClarificationNeeded { .. } => {
                    "clarification".to_string()
                }
                _ => "unsupported".to_string(),
            },
        }
    }

    /// Rebuild a context from stored turns (newest first) plus the session's
    /// pending clarification. Used when a session is loaded without context.
    pub fn reconstruct(session: &ConversationSession) -> Self {
        let mut context = WorkingContext::default();
        for turn in session.turns.iter().rev() {
            context.observe_turn(&turn.result);
        }
        if let Some(turn) = session.turns.last() {
            if let Some(operation) = Self::derive_operation(turn) {
                context.last_operation = Some(operation);
            }
        }
        context.pending = session.pending_clarification.clone();
        context.updated_at = now_iso();
        context.enforce_bounds();
        context
    }

    /// Approximate the last operation from a stored turn when the persisted
    /// context is unavailable.
    fn derive_operation(turn: &ConversationTurn) -> Option<LastOperation> {
        let result = &turn.result;
        let kind = match result.interpretation.kind {
            super::types::RequestKind::Teaching => OperationKind::Teach,
            super::types::RequestKind::Question => match &result.capability {
                super::types::Capability::StructuredSolver { .. } => OperationKind::Solve,
                _ => OperationKind::Ask,
            },
            super::types::RequestKind::Unsupported => OperationKind::Other,
        };
        Some(LastOperation {
            kind,
            arguments: vec![(
                "input".to_string(),
                result.interpretation.normalized_input.clone(),
            )],
            turn_id: turn.turn_id.clone(),
            at: turn.at.clone(),
        })
    }

    /// Enforce size bounds so a long conversation cannot grow context without
    /// limit.
    pub fn enforce_bounds(&mut self) {
        if let Some(topic) = &mut self.topic {
            Self::truncate(topic);
        }
        for entity in &mut self.entities {
            Self::truncate(entity);
        }
        self.entities.retain(|entity| !entity.is_empty());
        self.entities.dedup();
        if self.entities.len() > Self::MAX_ENTITIES {
            let overflow = self.entities.len() - Self::MAX_ENTITIES;
            self.entities.drain(0..overflow);
        }
        for reference in &mut self.entity_refs {
            Self::truncate(&mut reference.text);
        }
        self.entity_refs.retain(|reference| !reference.text.is_empty());
        if self.entity_refs.len() > Self::MAX_ENTITY_REFS {
            let overflow = self.entity_refs.len() - Self::MAX_ENTITY_REFS;
            self.entity_refs.drain(0..overflow);
        }
        for reference in &mut self.references {
            Self::truncate(&mut reference.summary);
        }
        if self.references.len() > Self::MAX_REFERENCES {
            let overflow = self.references.len() - Self::MAX_REFERENCES;
            self.references.drain(0..overflow);
        }
        for assumption in &mut self.assumptions {
            Self::truncate(&mut assumption.value);
            Self::truncate(&mut assumption.statement);
        }
        if self.assumptions.len() > Self::MAX_ASSUMPTIONS {
            let overflow = self.assumptions.len() - Self::MAX_ASSUMPTIONS;
            self.assumptions.drain(0..overflow);
        }
        if let Some(operation) = &mut self.last_operation {
            operation.arguments.truncate(Self::MAX_ARGUMENTS);
            for (_, value) in &mut operation.arguments {
                Self::truncate(value);
            }
        }
        if let Some(result) = &mut self.last_result {
            if let Some(answer) = &mut result.answer {
                Self::truncate(answer);
            }
        }
    }

    fn truncate(value: &mut String) {
        if value.chars().count() > Self::MAX_TEXT {
            *value = value.chars().take(Self::MAX_TEXT).collect();
        }
    }

    fn push_unique(values: &mut Vec<String>, value: String) {
        if !value.trim().is_empty() && !values.contains(&value) {
            values.push(value);
        }
    }

    pub(crate) fn salient_terms(text: &str) -> Option<(String, String)> {
        let after_label = text.rsplit(": ").next().unwrap_or(text);
        let clause = after_label.split("->").next().unwrap_or(after_label);
        let clause = clause.trim().trim_start_matches("if ").trim();
        // Parenthesized triples: "(subject, verb, object)".
        if let Some(inner) = clause
            .strip_prefix('(')
            .and_then(|rest| rest.split(')').next())
        {
            let parts: Vec<&str> = inner
                .split(',')
                .map(str::trim)
                .filter(|part| !part.is_empty())
                .collect();
            if parts.len() >= 2 {
                return Some((
                    parts[0].to_string(),
                    parts[parts.len() - 1].to_string(),
                ));
            }
        }
        let clause = clause.trim_matches(|character| {
            character == '(' || character == ')' || character == ',' || character == ' '
        });
        crate::nlp::extract_svo(clause)
            .into_iter()
            .next()
            .map(|triple| (triple.subject, triple.object))
    }
}

impl ConversationSession {
    pub fn new(id: impl Into<String>) -> Self {
        ConversationSession {
            id: id.into(),
            created_at: now_iso(),
            turns: Vec::new(),
            pending_clarification: None,
        }
    }

    pub fn stale_count(&self) -> usize {
        self.turns.iter().filter(|turn| turn.stale.is_some()).count()
    }

    pub fn last_turn(&self) -> Option<&ConversationTurn> {
        self.turns.last()
    }

    pub fn answered_count(&self) -> usize {
        self.turns
            .iter()
            .filter(|turn| turn.result.is_answered())
            .count()
    }
}

/// JSON-backed conversation store.
///
/// The store is bounded: the oldest sessions and the oldest turns inside a
/// session are evicted once the configured caps are exceeded, so a long-lived
/// runtime cannot grow without limit.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ConversationStore {
    pub sessions: Vec<ConversationSession>,
    pub max_sessions: usize,
    pub max_turns_per_session: usize,
    #[serde(skip)]
    pub path: Option<String>,
}

impl Default for ConversationStore {
    fn default() -> Self {
        Self::new()
    }
}

impl ConversationStore {
    pub fn new() -> Self {
        ConversationStore {
            sessions: Vec::new(),
            max_sessions: 32,
            max_turns_per_session: 256,
            path: None,
        }
    }

    pub fn with_path(path: impl Into<String>) -> Self {
        let mut store = Self::new();
        store.path = Some(path.into());
        store
    }

    pub fn session(&self, id: &str) -> Option<&ConversationSession> {
        self.sessions.iter().find(|session| session.id == id)
    }

    pub fn session_mut_or_create(&mut self, id: &str) -> &mut ConversationSession {
        if let Some(index) = self.sessions.iter().position(|session| session.id == id) {
            return &mut self.sessions[index];
        }
        self.sessions.push(ConversationSession::new(id));
        while self.sessions.len() > self.max_sessions.max(1) {
            self.sessions.remove(0);
        }
        self.sessions.last_mut().expect("session was just pushed")
    }

    pub fn ensure_session(&mut self, id: &str) {
        self.session_mut_or_create(id);
    }

    pub fn record_turn(
        &mut self,
        session_id: &str,
        turn: ConversationTurn,
    ) -> &ConversationTurn {
        let max_turns = self.max_turns_per_session.max(1);
        let session = self.session_mut_or_create(session_id);
        session.turns.push(turn);
        while session.turns.len() > max_turns {
            session.turns.remove(0);
        }
        session.turns.last().expect("turn was just pushed")
    }

    pub fn pending_clarification(&self, session_id: &str) -> Option<&PendingClarification> {
        self.session(session_id)
            .and_then(|session| session.pending_clarification.as_ref())
    }

    pub fn set_pending_clarification(
        &mut self,
        session_id: &str,
        pending: PendingClarification,
    ) {
        self.session_mut_or_create(session_id).pending_clarification = Some(pending);
    }

    pub fn clear_pending_clarification(&mut self, session_id: &str) {
        self.session_mut_or_create(session_id).pending_clarification = None;
    }

    /// Mark a recorded turn stale because the knowledge it depended on
    /// changed. Returns true when the turn was found.
    pub fn mark_turn_stale(&mut self, turn_id: &str, reason: &str) -> bool {
        for session in &mut self.sessions {
            if let Some(turn) = session
                .turns
                .iter_mut()
                .find(|turn| turn.turn_id == turn_id)
            {
                turn.stale = Some(StaleMark::new(reason));
                return true;
            }
        }
        false
    }

    pub fn save(&self) -> Result<(), String> {
        let path = self
            .path
            .as_ref()
            .ok_or_else(|| "conversation store has no persistence path".to_string())?;
        self.save_to(path)
    }

    pub fn save_to(&self, path: impl AsRef<Path>) -> Result<(), String> {
        let path = path.as_ref();
        if let Some(parent) = path.parent() {
            if !parent.as_os_str().is_empty() {
                std::fs::create_dir_all(parent)
                    .map_err(|e| format!("Create directory error: {e}"))?;
            }
        }
        let json = serde_json::to_string_pretty(self)
            .map_err(|e| format!("Serialization error: {e}"))?;
        std::fs::write(path, json).map_err(|e| format!("Write error: {e}"))
    }

    pub fn load(path: impl AsRef<Path>) -> Result<Self, String> {
        let json = std::fs::read_to_string(path.as_ref())
            .map_err(|e| format!("Read error: {e}"))?;
        serde_json::from_str(&json).map_err(|e| format!("Deserialization error: {e}"))
    }
}
