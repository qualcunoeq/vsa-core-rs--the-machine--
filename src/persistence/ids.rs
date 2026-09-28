//! Stable identifier generation.
//!
//! Every durable record carries an opaque, globally unique id with a
//! human-readable prefix. Ids are generated once and never reused, so a
//! record can be referenced across restarts, backups, and exports.

/// Prefix for conversation session ids.
pub const SESSION_PREFIX: &str = "session";
/// Prefix for turn ids.
pub const TURN_PREFIX: &str = "turn";
/// Prefix for fact assertions.
pub const FACT_PREFIX: &str = "fact";
/// Prefix for rule assertions.
pub const RULE_PREFIX: &str = "rule";
/// Prefix for provenance source records.
pub const SOURCE_PREFIX: &str = "source";
/// Prefix for conclusion records.
pub const CONCLUSION_PREFIX: &str = "conclusion";
/// Prefix for imported source documents.
pub const DOCUMENT_PREFIX: &str = "document";
/// Prefix for proposed/committed items extracted from a document.
pub const DOCUMENT_ITEM_PREFIX: &str = "item";

/// Generate a new id with the given prefix, e.g. `fact-3f2a...`.
pub fn new_id(prefix: &str) -> String {
    format!("{prefix}-{}", uuid::Uuid::new_v4().simple())
}

pub fn new_session_id() -> String {
    new_id(SESSION_PREFIX)
}

pub fn new_turn_id() -> String {
    new_id(TURN_PREFIX)
}

pub fn new_fact_id() -> String {
    new_id(FACT_PREFIX)
}

pub fn new_rule_id() -> String {
    new_id(RULE_PREFIX)
}

pub fn new_source_id() -> String {
    new_id(SOURCE_PREFIX)
}

pub fn new_conclusion_id() -> String {
    new_id(CONCLUSION_PREFIX)
}

pub fn new_document_id() -> String {
    new_id(DOCUMENT_PREFIX)
}

pub fn new_document_item_id() -> String {
    new_id(DOCUMENT_ITEM_PREFIX)
}

/// True when `id` has the given prefix followed by a non-empty suffix.
pub fn has_prefix(id: &str, prefix: &str) -> bool {
    id.strip_prefix(prefix)
        .map(|rest| rest.starts_with('-') && rest.len() > 1)
        .unwrap_or(false)
}
