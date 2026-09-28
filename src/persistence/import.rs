//! One-time import of legacy JSON snapshots.
//!
//! The runtime used to persist QA memory as `qa_memory.json` and
//! conversations as `sessions.json`. On first open, a fresh database imports
//! those files (tracked in `meta` so it happens exactly once) and never
//! reads them again. Retracted or corrected knowledge in the database is
//! therefore never resurrected by a stale snapshot.

use super::db::Database;
use super::knowledge::{AssertionPayload, NewSource, StoredFact};
use crate::conversation::{ConversationStore, WorkingContext};
use crate::qa::QaEngine;

const QA_IMPORT_KEY: &str = "import.qa_json";
const SESSIONS_IMPORT_KEY: &str = "import.sessions_json";

/// Import a QA engine snapshot if it has not been imported before.
///
/// Returns the number of assertions imported (0 when skipped).
pub fn import_qa_snapshot(db: &mut Database, path: &str) -> Result<usize, String> {
    if db.get_meta(QA_IMPORT_KEY)?.is_some() {
        return Ok(0);
    }
    if !std::path::Path::new(path).exists() {
        return Ok(0);
    }
    let engine = QaEngine::load_from_file(path)?;
    let mut imported = 0usize;
    for fact in engine.facts() {
        let payload = AssertionPayload::fact(&fact.subject, &fact.verb, &fact.object);
        db.insert_assertion(
            payload,
            NewSource::new("json_import")
                .with_note(format!("imported from {path}")),
        )?;
        imported += 1;
    }
    for rule in engine.rules() {
        let payload = AssertionPayload::rule(
            StoredFact::new(
                &rule.antecedent_subject,
                &rule.antecedent_verb,
                &rule.antecedent_object,
            ),
            StoredFact::new(
                &rule.consequent_subject,
                &rule.consequent_verb,
                &rule.consequent_object,
            ),
            rule.confidence,
        );
        db.insert_assertion(
            payload,
            NewSource::new("json_import")
                .with_note(format!("imported from {path}")),
        )?;
        imported += 1;
    }
    db.set_meta(
        QA_IMPORT_KEY,
        &format!("{path} @ {}", chrono::Utc::now().to_rfc3339()),
    )?;
    Ok(imported)
}

/// Import a conversation snapshot if it has not been imported before.
///
/// Returns the number of sessions imported (0 when skipped).
pub fn import_sessions_snapshot(db: &mut Database, path: &str) -> Result<usize, String> {
    if db.get_meta(SESSIONS_IMPORT_KEY)?.is_some() {
        return Ok(0);
    }
    if !std::path::Path::new(path).exists() {
        return Ok(0);
    }
    let store = ConversationStore::load(path)?;
    let mut imported = 0usize;
    for session in &store.sessions {
        let title = session
            .turns
            .first()
            .map(|turn| turn.input.chars().take(60).collect::<String>());
        db.upsert_session_with_title(session, title.as_deref())?;
        for turn in &session.turns {
            db.record_turn(&session.id, turn, &[], None)?;
        }
        let context = WorkingContext::reconstruct(session);
        db.save_working_context(&session.id, &context)?;
        imported += 1;
    }
    db.set_meta(
        SESSIONS_IMPORT_KEY,
        &format!("{path} @ {}", chrono::Utc::now().to_rfc3339()),
    )?;
    Ok(imported)
}
