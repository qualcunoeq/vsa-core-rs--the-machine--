-- Phase 8 document learning.
--
-- Imported documents and the knowledge proposed from them are kept apart from
-- the assertions they eventually produce.  Each proposed item records where it
-- came from (character span and page), whether it was accepted or rejected,
-- and -- once committed -- which durable assertion carries it.  Removing a
-- document therefore retracts exactly the assertions whose items point back at
-- it, without touching knowledge taught another way.

CREATE TABLE documents (
    id          TEXT PRIMARY KEY,
    title       TEXT NOT NULL,
    kind        TEXT NOT NULL CHECK (kind IN ('plain_text', 'text_pdf')),
    origin      TEXT NOT NULL DEFAULT '',
    sha256      TEXT NOT NULL,
    byte_len    INTEGER NOT NULL,
    imported_at TEXT NOT NULL,
    status      TEXT NOT NULL CHECK (status IN ('imported', 'committed', 'removed')),
    note        TEXT NOT NULL DEFAULT ''
);

CREATE INDEX idx_documents_sha ON documents(sha256);
CREATE INDEX idx_documents_status ON documents(status);

CREATE TABLE document_items (
    id            TEXT PRIMARY KEY,
    document_id   TEXT NOT NULL REFERENCES documents(id) ON DELETE CASCADE,
    item_index    INTEGER NOT NULL,
    item_kind     TEXT NOT NULL CHECK (item_kind IN ('fact', 'definition', 'rule', 'rejected')),
    status        TEXT NOT NULL CHECK (status IN ('proposed', 'accepted', 'rejected', 'committed')),
    payload       TEXT NOT NULL,
    span_start    INTEGER,
    span_end      INTEGER,
    page          INTEGER,
    confidence    REAL NOT NULL DEFAULT 0.0,
    reason        TEXT NOT NULL DEFAULT '',
    assertion_id  TEXT,
    updated_at    TEXT NOT NULL,
    UNIQUE (document_id, item_index)
);

CREATE INDEX idx_document_items_document ON document_items(document_id, item_index);
CREATE INDEX idx_document_items_status ON document_items(status);
CREATE INDEX idx_document_items_assertion ON document_items(assertion_id);
