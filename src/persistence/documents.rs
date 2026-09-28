//! Document memory: imported source documents and the items proposed from them.
//!
//! An imported document is a first-class durable record with its own id and a
//! content hash. Each fact, definition, rule, or rejection discovered in the
//! document is stored as a [`StoredDocumentItem`] that points back at the
//! document and (once accepted and committed) at the durable assertion that
//! carries it. Removing a document flips its status and retracts exactly the
//! assertions whose items point back at it, so its influence disappears
//! predictably while taught-by-hand knowledge is untouched.

use serde::{Deserialize, Serialize};

use super::db::Database;
use super::ids;

fn now_iso() -> String {
    chrono::Utc::now().to_rfc3339()
}

/// How a document's text was obtained.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DocumentKind {
    PlainText,
    TextPdf,
}

impl DocumentKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            DocumentKind::PlainText => "plain_text",
            DocumentKind::TextPdf => "text_pdf",
        }
    }

    pub fn parse(value: &str) -> Result<Self, String> {
        match value {
            "plain_text" => Ok(DocumentKind::PlainText),
            "text_pdf" => Ok(DocumentKind::TextPdf),
            other => Err(format!("unknown document kind: {other}")),
        }
    }
}

/// Lifecycle status of an imported document.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DocumentStatus {
    Imported,
    Committed,
    Removed,
}

impl DocumentStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            DocumentStatus::Imported => "imported",
            DocumentStatus::Committed => "committed",
            DocumentStatus::Removed => "removed",
        }
    }

    pub fn parse(value: &str) -> Result<Self, String> {
        match value {
            "imported" => Ok(DocumentStatus::Imported),
            "committed" => Ok(DocumentStatus::Committed),
            "removed" => Ok(DocumentStatus::Removed),
            other => Err(format!("unknown document status: {other}")),
        }
    }
}

/// What kind of knowledge an item represents.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ItemKind {
    Fact,
    Definition,
    Rule,
    Rejected,
}

impl ItemKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            ItemKind::Fact => "fact",
            ItemKind::Definition => "definition",
            ItemKind::Rule => "rule",
            ItemKind::Rejected => "rejected",
        }
    }

    pub fn parse(value: &str) -> Result<Self, String> {
        match value {
            "fact" => Ok(ItemKind::Fact),
            "definition" => Ok(ItemKind::Definition),
            "rule" => Ok(ItemKind::Rule),
            "rejected" => Ok(ItemKind::Rejected),
            other => Err(format!("unknown item kind: {other}")),
        }
    }
}

/// Review status of a proposed item.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ItemStatus {
    Proposed,
    Accepted,
    Rejected,
    Committed,
}

impl ItemStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            ItemStatus::Proposed => "proposed",
            ItemStatus::Accepted => "accepted",
            ItemStatus::Rejected => "rejected",
            ItemStatus::Committed => "committed",
        }
    }

    pub fn parse(value: &str) -> Result<Self, String> {
        match value {
            "proposed" => Ok(ItemStatus::Proposed),
            "accepted" => Ok(ItemStatus::Accepted),
            "rejected" => Ok(ItemStatus::Rejected),
            "committed" => Ok(ItemStatus::Committed),
            other => Err(format!("unknown item status: {other}")),
        }
    }
}

/// An imported document.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct StoredDocument {
    pub id: String,
    pub title: String,
    pub kind: DocumentKind,
    pub origin: String,
    pub sha256: String,
    pub byte_len: i64,
    pub imported_at: String,
    pub status: DocumentStatus,
    pub note: String,
}

/// One item proposed from a document.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct StoredDocumentItem {
    pub id: String,
    pub document_id: String,
    pub item_index: i64,
    pub kind: ItemKind,
    pub status: ItemStatus,
    pub payload: String,
    pub span_start: Option<i64>,
    pub span_end: Option<i64>,
    pub page: Option<i64>,
    pub confidence: f64,
    pub reason: String,
    /// Durable assertion backing this item once committed.
    pub assertion_id: Option<String>,
    pub updated_at: String,
}

/// A document row to be inserted.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct NewDocument {
    pub title: String,
    pub kind: DocumentKind,
    pub origin: String,
    pub sha256: String,
    pub byte_len: i64,
    pub note: String,
}

/// A proposed item row to be inserted.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct NewDocumentItem {
    pub kind: ItemKind,
    pub status: ItemStatus,
    pub payload: String,
    pub span_start: Option<i64>,
    pub span_end: Option<i64>,
    pub page: Option<i64>,
    pub confidence: f64,
    pub reason: String,
}

const DOCUMENT_COLUMNS: &str = "id, title, kind, origin, sha256, byte_len, imported_at, status, note";
const ITEM_COLUMNS: &str = "id, document_id, item_index, item_kind, status, payload, \
     span_start, span_end, page, confidence, reason, assertion_id, updated_at";

fn row_to_document(row: &rusqlite::Row<'_>) -> rusqlite::Result<StoredDocument> {
    let kind_text: String = row.get(2)?;
    let status_text: String = row.get(7)?;
    Ok(StoredDocument {
        id: row.get(0)?,
        title: row.get(1)?,
        kind: DocumentKind::parse(&kind_text).map_err(|error| {
            rusqlite::Error::FromSqlConversionFailure(
                2,
                rusqlite::types::Type::Text,
                Box::new(std::io::Error::new(std::io::ErrorKind::InvalidData, error)),
            )
        })?,
        origin: row.get(3)?,
        sha256: row.get(4)?,
        byte_len: row.get(5)?,
        imported_at: row.get(6)?,
        status: DocumentStatus::parse(&status_text).map_err(|error| {
            rusqlite::Error::FromSqlConversionFailure(
                7,
                rusqlite::types::Type::Text,
                Box::new(std::io::Error::new(std::io::ErrorKind::InvalidData, error)),
            )
        })?,
        note: row.get(8)?,
    })
}

fn row_to_item(row: &rusqlite::Row<'_>) -> rusqlite::Result<StoredDocumentItem> {
    let kind_text: String = row.get(3)?;
    let status_text: String = row.get(4)?;
    Ok(StoredDocumentItem {
        id: row.get(0)?,
        document_id: row.get(1)?,
        item_index: row.get(2)?,
        kind: ItemKind::parse(&kind_text).map_err(|error| {
            rusqlite::Error::FromSqlConversionFailure(
                3,
                rusqlite::types::Type::Text,
                Box::new(std::io::Error::new(std::io::ErrorKind::InvalidData, error)),
            )
        })?,
        status: ItemStatus::parse(&status_text).map_err(|error| {
            rusqlite::Error::FromSqlConversionFailure(
                4,
                rusqlite::types::Type::Text,
                Box::new(std::io::Error::new(std::io::ErrorKind::InvalidData, error)),
            )
        })?,
        payload: row.get(5)?,
        span_start: row.get(6)?,
        span_end: row.get(7)?,
        page: row.get(8)?,
        confidence: row.get(9)?,
        reason: row.get(10)?,
        assertion_id: row.get(11)?,
        updated_at: row.get(12)?,
    })
}

/// Insert a new document and return it. The document starts `imported`.
pub fn insert_document(db: &mut Database, new: &NewDocument) -> Result<StoredDocument, String> {
    let id = ids::new_document_id();
    db.transaction(|transaction| {
        transaction
            .execute(
                "INSERT INTO documents
                     (id, title, kind, origin, sha256, byte_len, imported_at, status, note)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, 'imported', ?8)",
                rusqlite::params![
                    id,
                    new.title,
                    new.kind.as_str(),
                    new.origin,
                    new.sha256,
                    new.byte_len,
                    now_iso(),
                    new.note,
                ],
            )
            .map_err(|error| format!("insert document: {error}"))?;
        Ok(())
    })?;
    document(db, &id)?.ok_or_else(|| "document vanished after insert".to_string())
}

/// Store a batch of proposed items for a document, in order. Existing items
/// for the document are replaced so re-importing is idempotent.
pub fn insert_items(
    db: &mut Database,
    document_id: &str,
    items: &[NewDocumentItem],
) -> Result<Vec<StoredDocumentItem>, String> {
    let now = now_iso();
    let ids_and_items: Vec<(String, &NewDocumentItem)> = items
        .iter()
        .map(|item| (ids::new_document_item_id(), item))
        .collect();
    db.transaction(|transaction| {
        transaction
            .execute(
                "DELETE FROM document_items WHERE document_id = ?1",
                [document_id],
            )
            .map_err(|error| format!("clear document items: {error}"))?;
        for (index, (id, item)) in ids_and_items.iter().enumerate() {
            transaction
                .execute(
                    "INSERT INTO document_items
                         (id, document_id, item_index, item_kind, status, payload,
                          span_start, span_end, page, confidence, reason, assertion_id, updated_at)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, NULL, ?12)",
                    rusqlite::params![
                        id,
                        document_id,
                        index as i64,
                        item.kind.as_str(),
                        item.status.as_str(),
                        item.payload,
                        item.span_start,
                        item.span_end,
                        item.page,
                        item.confidence,
                        item.reason,
                        now,
                    ],
                )
                .map_err(|error| format!("insert document item: {error}"))?;
        }
        Ok(())
    })?;
    items_for_document(db, document_id)
}

/// Fetch a document by id.
pub fn document(db: &Database, id: &str) -> Result<Option<StoredDocument>, String> {
    let sql = format!("SELECT {DOCUMENT_COLUMNS} FROM documents WHERE id = ?1");
    let mut statement = db
        .connection()
        .prepare(&sql)
        .map_err(|error| format!("prepare document lookup: {error}"))?;
    let mut rows = statement
        .query([id])
        .map_err(|error| format!("query document: {error}"))?;
    match rows.next().map_err(|error| format!("read document: {error}"))? {
        Some(row) => row_to_document(row)
            .map(Some)
            .map_err(|error| format!("decode document: {error}")),
        None => Ok(None),
    }
}

/// Find an existing document with the same content hash, if any. Used to make
/// re-importing the same content detectable.
pub fn document_by_sha(db: &Database, sha256: &str) -> Result<Option<StoredDocument>, String> {
    let sql = format!(
        "SELECT {DOCUMENT_COLUMNS} FROM documents WHERE sha256 = ?1 AND status != 'removed' \
         ORDER BY imported_at ASC LIMIT 1"
    );
    let mut statement = db
        .connection()
        .prepare(&sql)
        .map_err(|error| format!("prepare document hash lookup: {error}"))?;
    let mut rows = statement
        .query([sha256])
        .map_err(|error| format!("query document by hash: {error}"))?;
    match rows.next().map_err(|error| format!("read document: {error}"))? {
        Some(row) => row_to_document(row)
            .map(Some)
            .map_err(|error| format!("decode document: {error}")),
        None => Ok(None),
    }
}

/// All documents, newest first.
pub fn list_documents(db: &Database) -> Result<Vec<StoredDocument>, String> {
    let sql = format!("SELECT {DOCUMENT_COLUMNS} FROM documents ORDER BY imported_at ASC, id ASC");
    let mut statement = db
        .connection()
        .prepare(&sql)
        .map_err(|error| format!("prepare document list: {error}"))?;
    let rows = statement
        .query_map([], row_to_document)
        .map_err(|error| format!("query documents: {error}"))?;
    rows.collect::<rusqlite::Result<Vec<_>>>()
        .map_err(|error| format!("read documents: {error}"))
}

/// Items of one document, in proposal order.
pub fn items_for_document(
    db: &Database,
    document_id: &str,
) -> Result<Vec<StoredDocumentItem>, String> {
    let sql = format!(
        "SELECT {ITEM_COLUMNS} FROM document_items WHERE document_id = ?1 ORDER BY item_index ASC"
    );
    let mut statement = db
        .connection()
        .prepare(&sql)
        .map_err(|error| format!("prepare item list: {error}"))?;
    let rows = statement
        .query_map([document_id], row_to_item)
        .map_err(|error| format!("query document items: {error}"))?;
    rows.collect::<rusqlite::Result<Vec<_>>>()
        .map_err(|error| format!("read document items: {error}"))
}

/// Fetch one item by id.
pub fn item(db: &Database, id: &str) -> Result<Option<StoredDocumentItem>, String> {
    let sql = format!("SELECT {ITEM_COLUMNS} FROM document_items WHERE id = ?1");
    let mut statement = db
        .connection()
        .prepare(&sql)
        .map_err(|error| format!("prepare item lookup: {error}"))?;
    let mut rows = statement
        .query([id])
        .map_err(|error| format!("query item: {error}"))?;
    match rows.next().map_err(|error| format!("read item: {error}"))? {
        Some(row) => row_to_item(row)
            .map(Some)
            .map_err(|error| format!("decode item: {error}")),
        None => Ok(None),
    }
}

/// Update an item's review status (accepted/rejected) and its assertion link.
pub fn set_item_review(
    db: &mut Database,
    id: &str,
    status: ItemStatus,
    reason: &str,
    assertion_id: Option<&str>,
) -> Result<(), String> {
    db.transaction(|transaction| {
        transaction
            .execute(
                "UPDATE document_items
                 SET status = ?2, reason = ?3, assertion_id = ?4, updated_at = ?5
                 WHERE id = ?1",
                rusqlite::params![id, status.as_str(), reason, assertion_id, now_iso()],
            )
            .map_err(|error| format!("update document item: {error}"))?;
        Ok(())
    })
}

/// Mark a document's status.
pub fn set_document_status(
    db: &mut Database,
    id: &str,
    status: DocumentStatus,
    note: &str,
) -> Result<(), String> {
    db.transaction(|transaction| {
        transaction
            .execute(
                "UPDATE documents SET status = ?2, note = ?3 WHERE id = ?1",
                rusqlite::params![id, status.as_str(), note],
            )
            .map_err(|error| format!("update document status: {error}"))?;
        Ok(())
    })
}

/// Assertion ids committed from a document (the ones removal must retract).
pub fn committed_assertion_ids(db: &Database, document_id: &str) -> Result<Vec<String>, String> {
    let mut statement = db
        .connection()
        .prepare(
            "SELECT assertion_id FROM document_items
             WHERE document_id = ?1 AND status = 'committed' AND assertion_id IS NOT NULL
             ORDER BY item_index ASC",
        )
        .map_err(|error| format!("prepare committed assertion lookup: {error}"))?;
    let rows = statement
        .query_map([document_id], |row| row.get::<_, String>(0))
        .map_err(|error| format!("query committed assertions: {error}"))?;
    rows.collect::<rusqlite::Result<Vec<_>>>()
        .map_err(|error| format!("read committed assertions: {error}"))
}

/// Counts of items by review status: (proposed, accepted, rejected, committed).
pub fn item_status_counts(
    db: &Database,
    document_id: &str,
) -> Result<(usize, usize, usize, usize), String> {
    let mut statement = db
        .connection()
        .prepare(
            "SELECT status, COUNT(*) FROM document_items
             WHERE document_id = ?1 GROUP BY status",
        )
        .map_err(|error| format!("prepare item count: {error}"))?;
    let rows = statement
        .query_map([document_id], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?))
        })
        .map_err(|error| format!("query item counts: {error}"))?;
    let mut proposed = 0usize;
    let mut accepted = 0usize;
    let mut rejected = 0usize;
    let mut committed = 0usize;
    for row in rows {
        let (status, count) = row.map_err(|error| format!("read item count: {error}"))?;
        let count = count as usize;
        match status.as_str() {
            "proposed" => proposed = count,
            "accepted" => accepted = count,
            "rejected" => rejected = count,
            "committed" => committed = count,
            _ => {}
        }
    }
    Ok((proposed, accepted, rejected, committed))
}
