//! Phase 3 persistence tests: migrations, versioned assertions, provenance,
//! staleness, restart, import-once, and backup/restore.

use super::*;
use crate::conversation::{ConversationSession, ConversationTurn, PendingClarification, WorkingContext};
use crate::conversation::{RequestKind, TurnDiagnostics, TurnOutcome, TurnResult, TurnTiming};

struct TempDir {
    path: std::path::PathBuf,
}

impl TempDir {
    fn new(tag: &str) -> Self {
        let path = std::env::temp_dir().join(format!(
            "the_machine_persistence_{}_{}",
            std::process::id(),
            tag
        ));
        std::fs::create_dir_all(&path).expect("temp dir");
        TempDir { path }
    }

    fn db(&self) -> String {
        self.path.join("machine.db").to_string_lossy().to_string()
    }

    fn file(&self, name: &str) -> String {
        self.path.join(name).to_string_lossy().to_string()
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.path);
    }
}

fn fact_payload(subject: &str, verb: &str, object: &str) -> AssertionPayload {
    AssertionPayload::fact(subject, verb, object)
}

fn dummy_turn(id: &str) -> ConversationTurn {
    ConversationTurn::new(id, "Who raised rates?", dummy_result(id))
}

fn dummy_result(turn_id: &str) -> TurnResult {
    TurnResult {
        outcome: TurnOutcome::Answered,
        answer_text: "The Fed".to_string(),
        answer: Some("The Fed".to_string()),
        interpretation: crate::conversation::Interpretation {
            kind: RequestKind::Question,
            normalized_input: "Who raised rates?".to_string(),
            capability: crate::conversation::Capability::FactualQa,
            notes: Vec::new(),
            follow_up: None,
        },
        evidence: Vec::new(),
        capability: crate::conversation::Capability::FactualQa,
        verification: crate::conversation::VerificationStatus::NotAttempted,
        memory_changes: Vec::new(),
        timing: TurnTiming {
            started_at: chrono::Utc::now().to_rfc3339(),
            elapsed_ms: 1.0,
        },
        diagnostics: TurnDiagnostics {
            session_id: "session-test".to_string(),
            turn_id: turn_id.to_string(),
            episode_id: None,
            fact_count_before: 1,
            fact_count_after: 1,
            rule_count_before: 0,
            rule_count_after: 0,
            storage_error: None,
        },
    }
}

#[test]
fn migrations_are_idempotent_and_versioned() {
    let dir = TempDir::new("migrations");
    let mut db = Database::open(dir.db()).expect("open");
    assert_eq!(db.schema_version().expect("version"), SCHEMA_VERSION);
    db.migrate().expect("second migrate is a no-op");
    assert_eq!(db.schema_version().expect("version"), SCHEMA_VERSION);
    db.integrity_check().expect("integrity");

    // Reopen: still one schema, no duplicate tables.
    drop(db);
    let db = Database::open(dir.db()).expect("reopen");
    assert_eq!(db.schema_version().expect("version"), SCHEMA_VERSION);
}

#[test]
fn assertion_lifecycle_is_versioned_with_provenance() {
    let dir = TempDir::new("lifecycle");
    let mut db = Database::open(dir.db()).expect("open");

    let assertion = db
        .insert_assertion(
            fact_payload("The Fed", "raise", "rates"),
            NewSource::new("user_teach")
                .in_session(Some("session-1"))
                .in_turn(Some("turn-1"))
                .with_note("taught in the interface"),
        )
        .expect("insert");
    assert!(assertion.id.starts_with("fact-"));
    assert_eq!(assertion.version, 1);
    assert_eq!(assertion.scope, "shared");
    assert_eq!(assertion.status, AssertionStatus::Active);
    assert_eq!(assertion.source.kind, "user_teach");
    assert_eq!(assertion.source.session_id.as_deref(), Some("session-1"));
    assert_eq!(assertion.source.turn_id.as_deref(), Some("turn-1"));

    let (corrected, stale) = db
        .correct_assertion(
            &assertion.id,
            fact_payload("The Fed", "lower", "rates"),
            NewSource::new("user_correction").with_note("policy reversal"),
            "policy reversal",
        )
        .expect("correct");
    assert_eq!(corrected.version, 2);
    assert_eq!(
        corrected.payload.as_fact().expect("fact").verb,
        "lower"
    );
    assert!(stale.is_empty());

    let versions = db.assertion_versions(&assertion.id).expect("versions");
    assert_eq!(versions.len(), 2);
    assert_eq!(versions[0].version, 1);
    assert_eq!(versions[1].version, 2);
    assert!(versions[1].note.contains("policy"));

    let (retracted, _) = db
        .retract_assertion(
            &assertion.id,
            NewSource::new("user_retraction").with_note("no longer true"),
            "no longer true",
        )
        .expect("retract");
    assert_eq!(retracted.status, AssertionStatus::Retracted);
    assert_eq!(retracted.version, 3);
    assert!(db
        .active_assertions()
        .expect("active")
        .iter()
        .all(|item| item.id != assertion.id));

    // Tombstone survives a restart.
    drop(db);
    let mut db = Database::open(dir.db()).expect("reopen");
    let tombstone = db.assertion(&assertion.id).expect("lookup").expect("exists");
    assert_eq!(tombstone.status, AssertionStatus::Retracted);

    let (existed, _) = db.forget_assertion(&assertion.id).expect("forget");
    assert!(existed);
    assert!(db.assertion(&assertion.id).expect("lookup").is_none());
    assert!(db
        .assertion_versions(&assertion.id)
        .expect("versions")
        .is_empty());
}

#[test]
fn correction_marks_dependent_turns_stale() {
    let dir = TempDir::new("staleness");
    let mut db = Database::open(dir.db()).expect("open");

    let assertion = db
        .insert_assertion(
            fact_payload("The Fed", "raise", "rates"),
            NewSource::new("user_teach"),
        )
        .expect("insert");
    let session = ConversationSession::new("session-stale");
    db.upsert_session(&session).expect("session");
    let turn = dummy_turn("turn-stale");
    db.record_turn("session-stale", &turn, &[assertion.id.clone()], None)
        .expect("turn");

    let (_, stale) = db
        .correct_assertion(
            &assertion.id,
            fact_payload("The Fed", "lower", "rates"),
            NewSource::new("user_correction"),
            "reversal",
        )
        .expect("correct");
    assert_eq!(stale.len(), 1);
    assert_eq!(stale[0].turn_id, "turn-stale");
    assert!(stale[0].reason.contains("corrected"));

    // Reload: the turn carries the stale mark.
    let store = db.load_store().expect("load");
    let session = store.session("session-stale").expect("session");
    let turn = &session.turns[0];
    let mark = turn.stale.as_ref().expect("stale mark");
    assert!(mark.reason.contains("lower"));

    // A conclusion for a forgotten assertion is still marked first.
    let (existed, stale) = db.forget_assertion(&assertion.id).expect("forget");
    assert!(existed);
    assert_eq!(stale.len(), 1);
}

#[test]
fn retraction_is_not_resurrected_by_import_or_restart() {
    let dir = TempDir::new("resurrection");
    let qa_path = dir.file("qa_memory.json");
    {
        let mut engine = crate::qa::QaEngine::new();
        engine.store_fact("The Fed", "raise", "rates", "legacy");
        engine.save_to_file(&qa_path).expect("save engine");
    }

    let mut db = Database::open(dir.db()).expect("open");
    let imported = import::import_qa_snapshot(&mut db, &qa_path).expect("import");
    assert_eq!(imported, 1);
    let assertion = db
        .list_assertions(None, None, None, None)
        .expect("list")
        .into_iter()
        .next()
        .expect("assertion");
    db.retract_assertion(
        &assertion.id,
        NewSource::new("user_retraction"),
        "withdrawn",
    )
    .expect("retract");
    drop(db);

    // Reopen: import is recorded, so the snapshot must not resurrect it.
    let mut db = Database::open(dir.db()).expect("reopen");
    let again = import::import_qa_snapshot(&mut db, &qa_path).expect("import again");
    assert_eq!(again, 0);
    let tombstone = db.assertion(&assertion.id).expect("lookup").expect("exists");
    assert_eq!(tombstone.status, AssertionStatus::Retracted);
    assert!(db.active_assertions().expect("active").is_empty());
}

#[test]
fn backup_and_restore_round_trip() {
    let dir = TempDir::new("backup");
    let mut db = Database::open(dir.db()).expect("open");
    let assertion = db
        .insert_assertion(
            fact_payload("The Fed", "raise", "rates"),
            NewSource::new("user_teach"),
        )
        .expect("insert");
    db.record_conclusion("session-1", "turn-1", &[assertion.id.clone()])
        .expect("conclusion");

    let backup_path = dir.file("backup.db");
    db.backup_to(&backup_path).expect("backup");
    assert!(std::path::Path::new(&backup_path).exists());

    // Mutate after the backup.
    db.forget_assertion(&assertion.id).expect("forget");
    assert!(db.active_assertions().expect("active").is_empty());

    db.restore_from(&backup_path).expect("restore");
    let restored = db.active_assertions().expect("active");
    assert_eq!(restored.len(), 1);
    assert_eq!(restored[0].id, assertion.id);
    assert_eq!(
        db.assertions_for_turn("turn-1").expect("turn links"),
        vec![assertion.id.clone()]
    );
    db.integrity_check().expect("integrity after restore");
}

#[test]
fn working_context_is_bounded_and_reconstructable() {
    let dir = TempDir::new("context");
    let mut db = Database::open(dir.db()).expect("open");

    let mut session = ConversationSession::new("session-context");
    session.turns.push(dummy_turn("turn-1"));
    session.pending_clarification = Some(PendingClarification {
        turn_id: "turn-1".to_string(),
        question: "Who raised rates?".to_string(),
        missing: "complete statement".to_string(),
        kind: crate::conversation::ClarificationKind::MissingDetail,
        candidates: Vec::new(),
        original_input: "Who raised rates?".to_string(),
    });
    db.upsert_session(&session).expect("session");

    let mut context = WorkingContext::default();
    for index in 0..20 {
        context.entities.push(format!("entity-{index}"));
    }
    context.topic = Some("x".repeat(500));
    context.pending = session.pending_clarification.clone();
    db.save_working_context(&session.id, &context)
        .expect("save context");

    let loaded = db
        .load_working_context(&session.id)
        .expect("load")
        .expect("present");
    assert_eq!(loaded.entities.len(), WorkingContext::MAX_ENTITIES);
    assert!(loaded.topic.as_ref().expect("topic").chars().count() <= WorkingContext::MAX_TEXT);
    assert_eq!(loaded.pending.expect("pending").question, "Who raised rates?");

    let rebuilt = WorkingContext::reconstruct(&session);
    assert!(rebuilt.pending.is_some());
}

#[test]
fn sessions_and_turns_survive_restart() {
    let dir = TempDir::new("sessions");
    let mut db = Database::open(dir.db()).expect("open");
    let session = ConversationSession::new("session-roundtrip");
    db.upsert_session_with_title(&session, Some("first question"))
        .expect("session");
    db.record_turn(
        "session-roundtrip",
        &dummy_turn("turn-1"),
        &[],
        Some(&WorkingContext::default()),
    )
    .expect("turn");
    drop(db);

    let db = Database::open(dir.db()).expect("reopen");
    let store = db.load_store().expect("load");
    let session = store.session("session-roundtrip").expect("session");
    assert_eq!(session.turns.len(), 1);
    assert_eq!(session.turns[0].turn_id, "turn-1");
    assert_eq!(session.turns[0].result.outcome, TurnOutcome::Answered);
    let record = db
        .session_record("session-roundtrip")
        .expect("record")
        .expect("present");
    assert_eq!(record.title.as_deref(), Some("first question"));
    assert_eq!(db.session_turn_counts().expect("counts"), (1, 1));
}

#[test]
fn assertions_are_addressable_by_svo_and_rule_parts() {
    let dir = TempDir::new("lookup");
    let mut db = Database::open(dir.db()).expect("open");
    db.insert_assertion(
        fact_payload("The Fed", "raise", "rates"),
        NewSource::new("user_teach"),
    )
    .expect("fact");
    db.insert_assertion(
        AssertionPayload::rule(
            StoredFact::new("The Fed", "raise", "rates"),
            StoredFact::new("treasury yields", "rise", "across the curve"),
            1.0,
        ),
        NewSource::new("user_teach"),
    )
    .expect("rule");

    let fact = db
        .find_active_fact("The Fed", "raise", "rates")
        .expect("lookup")
        .expect("found");
    assert_eq!(fact.kind, AssertionKind::Fact);
    let rule = db
        .find_active_rule(
            &StoredFact::new("The Fed", "raise", "rates"),
            &StoredFact::new("treasury yields", "rise", "across the curve"),
        )
        .expect("lookup")
        .expect("found");
    assert_eq!(rule.kind, AssertionKind::Rule);
    assert_eq!(db.assertion_counts().expect("counts"), (1, 1));
}

#[test]
fn documents_and_items_round_trip_with_review_status() {
    let dir = TempDir::new("documents");
    let mut db = Database::open(dir.db()).expect("open");

    let document = documents::insert_document(
        &mut db,
        &NewDocument {
            title: "mechanics".to_string(),
            kind: DocumentKind::PlainText,
            origin: "mechanics.txt".to_string(),
            sha256: "abc123".to_string(),
            byte_len: 42,
            note: String::new(),
        },
    )
    .expect("insert document");
    assert!(document.id.starts_with("document-"));
    assert_eq!(document.status, DocumentStatus::Imported);

    let items = documents::insert_items(
        &mut db,
        &document.id,
        &[
            NewDocumentItem {
                kind: ItemKind::Fact,
                status: ItemStatus::Proposed,
                payload: "{\"form\":\"triple\"}".to_string(),
                span_start: Some(0),
                span_end: Some(10),
                page: Some(1),
                confidence: 0.6,
                reason: String::new(),
            },
            NewDocumentItem {
                kind: ItemKind::Rejected,
                status: ItemStatus::Rejected,
                payload: "{\"form\":\"none\"}".to_string(),
                span_start: Some(11),
                span_end: Some(20),
                page: None,
                confidence: 0.0,
                reason: "instruction-like".to_string(),
            },
        ],
    )
    .expect("insert items");
    assert_eq!(items.len(), 2);
    assert_eq!(items[1].kind, ItemKind::Rejected);

    // Review and commit the first item, linking an assertion.
    let assertion = db
        .insert_assertion(
            fact_payload("Newton", "publish", "the Principia"),
            NewSource::new("document"),
        )
        .expect("assertion");
    documents::set_item_review(
        &mut db,
        &items[0].id,
        ItemStatus::Committed,
        "document note",
        Some(&assertion.id),
    )
    .expect("commit item");

    let committed = documents::committed_assertion_ids(&db, &document.id).expect("committed");
    assert_eq!(committed, vec![assertion.id.clone()]);
    let counts = documents::item_status_counts(&db, &document.id).expect("counts");
    assert_eq!(counts, (0, 0, 1, 1));

    documents::set_document_status(&mut db, &document.id, DocumentStatus::Removed, "gone")
        .expect("remove document");
    let stored = documents::document(&db, &document.id).expect("lookup").expect("found");
    assert_eq!(stored.status, DocumentStatus::Removed);

    // Hash lookup ignores removed documents.
    assert!(documents::document_by_sha(&db, "abc123").expect("by sha").is_none());
}

#[test]
fn document_items_cascade_when_the_document_is_forgotten() {
    let dir = TempDir::new("document_cascade");
    let mut db = Database::open(dir.db()).expect("open");
    let document = documents::insert_document(
        &mut db,
        &NewDocument {
            title: "t".to_string(),
            kind: DocumentKind::PlainText,
            origin: "t".to_string(),
            sha256: "h".to_string(),
            byte_len: 1,
            note: String::new(),
        },
    )
    .expect("document");
    documents::insert_items(
        &mut db,
        &document.id,
        &[NewDocumentItem {
            kind: ItemKind::Fact,
            status: ItemStatus::Proposed,
            payload: "{}".to_string(),
            span_start: None,
            span_end: None,
            page: None,
            confidence: 0.1,
            reason: String::new(),
        }],
    )
    .expect("items");

    db.transaction(|transaction| {
        transaction
            .execute("DELETE FROM documents WHERE id = ?1", [&document.id])
            .map_err(|error| format!("delete document: {error}"))?;
        Ok(())
    })
    .expect("delete");

    let remaining = documents::items_for_document(&db, &document.id).expect("items");
    assert!(remaining.is_empty(), "items must cascade with the document");
}
