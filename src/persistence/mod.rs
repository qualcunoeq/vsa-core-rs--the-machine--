//! Durable state for the conversational runtime.
//!
//! Three kinds of state are kept apart:
//!
//! | State               | Storage                              | Retention                    |
//! |---------------------|--------------------------------------|------------------------------|
//! | Conversation history| `sessions` / `turns` tables          | saved per session            |
//! | Working context     | `sessions.working_context` (bounded) | reconstructable from turns   |
//! | Knowledge memory    | `assertions` / `sources` tables      | persisted with provenance    |
//! | Imported documents  | `documents` / `document_items`       | persisted until removed      |
//!
//! Everything durable lives in one SQLite database. Writes go through
//! transactions, the schema is versioned and migrated in order, and backups
//! are made with `VACUUM INTO` and restored through SQLite's backup API.
//!
//! Knowledge assertions are versioned: correcting or retracting one marks
//! every turn whose conclusion depended on it as stale, so a later question
//! recomputes instead of reusing an invalidated answer. Retractions keep a
//! tombstone row, so imported snapshots and reloads cannot resurrect
//! knowledge that was deliberately withdrawn.

pub mod db;
pub mod documents;
pub mod ids;
pub mod import;
pub mod knowledge;
pub mod sessions;

#[cfg(test)]
mod tests;

pub use db::{Database, SCHEMA_VERSION};
pub use documents::{
    DocumentKind, DocumentStatus, ItemKind, ItemStatus, NewDocument, NewDocumentItem,
    StoredDocument, StoredDocumentItem,
};
pub use knowledge::{
    AssertionKind, AssertionPayload, AssertionSource, AssertionStatus, AssertionVersion,
    NewSource, StaleTurn, StoredAssertion, StoredFact, StoredRule,
};
pub use sessions::{SessionRecord, MAX_CONTEXT_ENTITIES, MAX_CONTEXT_TEXT};
