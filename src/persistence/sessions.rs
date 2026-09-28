//! Durable conversation state: sessions, turns, and bounded working context.
//!
//! Conversation history is saved per session. Working context (current topic,
//! selected entities, pending question) is stored alongside the session as
//! bounded JSON and can always be reconstructed from the turns, so it is a
//! convenience rather than a second source of truth.

use serde::{Deserialize, Serialize};

use super::db::Database;
use crate::conversation::{ConversationSession, ConversationStore, ConversationTurn};
use crate::conversation::TurnResult;

fn now_iso() -> String {
    chrono::Utc::now().to_rfc3339()
}

/// Upper bound on entities kept in working context.
pub const MAX_CONTEXT_ENTITIES: usize = 8;
/// Upper bound on a single stored context string.
pub const MAX_CONTEXT_TEXT: usize = 160;

/// A durable session row without its turns.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SessionRecord {
    pub id: String,
    pub title: Option<String>,
    pub created_at: String,
    pub updated_at: String,
    pub working_context: Option<String>,
}

impl Database {
    /// Create or refresh the session row.
    pub fn upsert_session(&mut self, session: &ConversationSession) -> Result<(), String> {
        self.upsert_session_with_title(session, None)
    }

    pub fn upsert_session_with_title(
        &mut self,
        session: &ConversationSession,
        title: Option<&str>,
    ) -> Result<(), String> {
        self.connection()
            .execute(
                "INSERT INTO sessions (id, title, created_at, updated_at)
                 VALUES (?1, ?2, ?3, ?4)
                 ON CONFLICT(id) DO UPDATE SET
                     title = COALESCE(excluded.title, sessions.title),
                     updated_at = excluded.updated_at",
                rusqlite::params![session.id, title, session.created_at, now_iso()],
            )
            .map_err(|error| format!("upsert session: {error}"))?;
        Ok(())
    }

    /// Append a turn, record its dependency links, and persist the bounded
    /// working context in one transaction.
    pub fn record_turn(
        &mut self,
        session_id: &str,
        turn: &ConversationTurn,
        assertion_ids: &[String],
        working_context: Option<&crate::conversation::WorkingContext>,
    ) -> Result<(), String> {
        let turn_id = turn.turn_id.clone();
        let at = turn.at.clone();
        let input = turn.input.clone();
        let mode = serde_json::to_string(&turn.result.interpretation.kind)
            .map_err(|error| format!("serialize turn mode: {error}"))?
            .trim_matches('"')
            .to_string();
        let outcome = serde_json::to_string(&turn.result.outcome)
            .map_err(|error| format!("serialize turn outcome: {error}"))?;
        let result = serde_json::to_string(&turn.result)
            .map_err(|error| format!("serialize turn result: {error}"))?;
        let session = session_id.to_string();
        let assertion_ids: Vec<String> = assertion_ids.to_vec();
        self.transaction(|transaction| {
            transaction
                .execute(
                    "INSERT INTO sessions (id, created_at, updated_at)
                     VALUES (?1, ?2, ?2)
                     ON CONFLICT(id) DO UPDATE SET updated_at = excluded.updated_at",
                    rusqlite::params![session, now_iso()],
                )
                .map_err(|error| format!("upsert session for turn: {error}"))?;
            let seq: i64 = transaction
                .query_row(
                    "SELECT COALESCE(MAX(seq), 0) + 1 FROM turns WHERE session_id = ?1",
                    [&session],
                    |row| row.get(0),
                )
                .map_err(|error| format!("next turn sequence: {error}"))?;
            transaction
                .execute(
                    "INSERT INTO turns
                         (id, session_id, seq, at, input, mode, outcome, result)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
                    rusqlite::params![turn_id, session, seq, at, input, mode, outcome, result],
                )
                .map_err(|error| format!("insert turn: {error}"))?;
            crate::persistence::knowledge::record_conclusion_tx(
                transaction,
                &session,
                &turn_id,
                &assertion_ids,
            )?;
            if let Some(context) = working_context {
                let mut context = context.clone();
                context.enforce_bounds();
                let json = serde_json::to_string(&context)
                    .map_err(|error| format!("serialize working context: {error}"))?;
                transaction
                    .execute(
                        "UPDATE sessions SET working_context = ?2, updated_at = ?3
                         WHERE id = ?1",
                        rusqlite::params![session, json, now_iso()],
                    )
                    .map_err(|error| format!("save working context: {error}"))?;
            }
            Ok(())
        })
    }

    /// Load all sessions (newest last, like the in-memory store) with their
    /// turns and stale marks, bounded by the store's caps.
    pub fn load_store(&self) -> Result<ConversationStore, String> {
        let mut store = ConversationStore::new();
        let max_sessions = store.max_sessions.max(1);
        let max_turns = store.max_turns_per_session.max(1);

        let sessions = {
            let mut statement = self
                .connection()
                .prepare(
                    "SELECT id, created_at FROM sessions
                     ORDER BY created_at DESC, id DESC LIMIT ?1",
                )
                .map_err(|error| format!("prepare session list: {error}"))?;
            let rows = statement
                .query_map([max_sessions as i64], |row| {
                    Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
                })
                .map_err(|error| format!("query sessions: {error}"))?;
            rows.collect::<rusqlite::Result<Vec<_>>>()
                .map_err(|error| format!("read sessions: {error}"))?
        };

        for (id, created_at) in sessions.into_iter().rev() {
            let mut session = ConversationSession::new(id.clone());
            session.created_at = created_at;
            session.turns = self.load_turns(&id, max_turns)?;
            session.pending_clarification = self
                .load_working_context(&id)?
                .and_then(|context| context.pending);
            store.sessions.push(session);
        }
        Ok(store)
    }

    fn load_turns(&self, session_id: &str, limit: usize) -> Result<Vec<ConversationTurn>, String> {
        let mut statement = self
            .connection()
            .prepare(
                "SELECT id, at, input, result, stale, stale_reason FROM turns
                 WHERE session_id = ?1 ORDER BY seq DESC LIMIT ?2",
            )
            .map_err(|error| format!("prepare turn list: {error}"))?;
        let rows = statement
            .query_map(rusqlite::params![session_id, limit as i64], |row| {
                let result_text: String = row.get(3)?;
                let stale: bool = row.get(4)?;
                let stale_reason: Option<String> = row.get(5)?;
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    result_text,
                    stale,
                    stale_reason,
                ))
            })
            .map_err(|error| format!("query turns: {error}"))?;
        let mut turns = Vec::new();
        for row in rows {
            let (turn_id, at, input, result_text, stale, stale_reason) =
                row.map_err(|error| format!("read turn: {error}"))?;
            let result: TurnResult = serde_json::from_str(&result_text)
                .map_err(|error| format!("decode turn {turn_id}: {error}"))?;
            let mut turn = ConversationTurn {
                turn_id,
                at,
                input,
                result,
                stale: None,
            };
            if stale {
                turn.stale = Some(crate::conversation::StaleMark {
                    reason: stale_reason.unwrap_or_else(|| "knowledge changed".to_string()),
                    at: now_iso(),
                });
            }
            turns.push(turn);
        }
        turns.reverse();
        Ok(turns)
    }

    /// Persist the bounded working context for a session.
    pub fn save_working_context(
        &mut self,
        session_id: &str,
        context: &crate::conversation::WorkingContext,
    ) -> Result<(), String> {
        let mut context = context.clone();
        context.enforce_bounds();
        let json = serde_json::to_string(&context)
            .map_err(|error| format!("serialize working context: {error}"))?;
        self.connection()
            .execute(
                "UPDATE sessions SET working_context = ?2, updated_at = ?3 WHERE id = ?1",
                rusqlite::params![session_id, json, now_iso()],
            )
            .map_err(|error| format!("save working context: {error}"))?;
        Ok(())
    }

    pub fn load_working_context(
        &self,
        session_id: &str,
    ) -> Result<Option<crate::conversation::WorkingContext>, String> {
        let mut statement = self
            .connection()
            .prepare("SELECT working_context FROM sessions WHERE id = ?1")
            .map_err(|error| format!("prepare working context: {error}"))?;
        let mut rows = statement
            .query([session_id])
            .map_err(|error| format!("query working context: {error}"))?;
        match rows
            .next()
            .map_err(|error| format!("read working context: {error}"))?
        {
            Some(row) => {
                let text: Option<String> = row
                    .get(0)
                    .map_err(|error| format!("decode working context: {error}"))?;
                match text {
                    Some(text) => {
                        let mut context: crate::conversation::WorkingContext =
                            serde_json::from_str(&text).map_err(|error| {
                                format!("parse working context for {session_id}: {error}")
                            })?;
                        context.enforce_bounds();
                        Ok(Some(context))
                    }
                    None => Ok(None),
                }
            }
            None => Ok(None),
        }
    }

    /// (session count, turn count).
    pub fn session_turn_counts(&self) -> Result<(usize, usize), String> {
        let sessions: i64 = self
            .connection()
            .query_row("SELECT COUNT(*) FROM sessions", [], |row| row.get(0))
            .map_err(|error| format!("count sessions: {error}"))?;
        let turns: i64 = self
            .connection()
            .query_row("SELECT COUNT(*) FROM turns", [], |row| row.get(0))
            .map_err(|error| format!("count turns: {error}"))?;
        Ok((sessions as usize, turns as usize))
    }

    pub fn session_record(&self, session_id: &str) -> Result<Option<SessionRecord>, String> {
        let mut statement = self
            .connection()
            .prepare(
                "SELECT id, title, created_at, updated_at, working_context
                 FROM sessions WHERE id = ?1",
            )
            .map_err(|error| format!("prepare session lookup: {error}"))?;
        let mut rows = statement
            .query([session_id])
            .map_err(|error| format!("query session: {error}"))?;
        match rows
            .next()
            .map_err(|error| format!("read session: {error}"))?
        {
            Some(row) => {
                let record = SessionRecord {
                    id: row.get(0).map_err(|error| format!("decode session id: {error}"))?,
                    title: row.get(1).map_err(|error| format!("decode session title: {error}"))?,
                    created_at: row
                        .get(2)
                        .map_err(|error| format!("decode session created_at: {error}"))?,
                    updated_at: row
                        .get(3)
                        .map_err(|error| format!("decode session updated_at: {error}"))?,
                    working_context: row
                        .get(4)
                        .map_err(|error| format!("decode working context: {error}"))?,
                };
                Ok(Some(record))
            }
            None => Ok(None),
        }
    }
}
