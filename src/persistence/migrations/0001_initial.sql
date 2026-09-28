-- Phase 3 initial schema.
--
-- Three kinds of state are kept apart:
--   * conversation history   -> sessions / turns
--   * working context        -> sessions.working_context (bounded JSON)
--   * knowledge memory       -> assertions / assertion_versions / sources
--
-- Conclusions record which assertions each answered turn depended on so a
-- correction or retraction can mark the affected turns stale.

CREATE TABLE meta (
    key   TEXT PRIMARY KEY,
    value TEXT NOT NULL
);

CREATE TABLE sources (
    id         TEXT PRIMARY KEY,
    kind       TEXT NOT NULL,
    session_id TEXT,
    turn_id    TEXT,
    note       TEXT NOT NULL DEFAULT '',
    created_at TEXT NOT NULL
);

CREATE TABLE assertions (
    id           TEXT PRIMARY KEY,
    kind         TEXT NOT NULL CHECK (kind IN ('fact', 'rule')),
    scope        TEXT NOT NULL DEFAULT 'shared',
    status       TEXT NOT NULL CHECK (status IN ('active', 'superseded', 'retracted')),
    version      INTEGER NOT NULL DEFAULT 1,
    source_id    TEXT NOT NULL REFERENCES sources(id),
    session_id   TEXT,
    created_at   TEXT NOT NULL,
    updated_at   TEXT NOT NULL,
    superseded_by TEXT,
    subject      TEXT,
    verb         TEXT,
    object       TEXT,
    ante_subject TEXT,
    ante_verb    TEXT,
    ante_object  TEXT,
    cons_subject TEXT,
    cons_verb    TEXT,
    cons_object  TEXT,
    confidence   REAL,
    payload      TEXT NOT NULL
);

CREATE INDEX idx_assertions_status ON assertions(status);
CREATE INDEX idx_assertions_kind_status ON assertions(kind, status);
CREATE INDEX idx_assertions_fact_svo ON assertions(subject, verb, object);
CREATE INDEX idx_assertions_rule_parts
    ON assertions(ante_subject, ante_verb, ante_object, cons_subject, cons_verb, cons_object);

CREATE TABLE assertion_versions (
    assertion_id TEXT NOT NULL REFERENCES assertions(id) ON DELETE CASCADE,
    version      INTEGER NOT NULL,
    status       TEXT NOT NULL,
    source_id    TEXT NOT NULL REFERENCES sources(id),
    payload      TEXT NOT NULL,
    note         TEXT NOT NULL DEFAULT '',
    recorded_at  TEXT NOT NULL,
    PRIMARY KEY (assertion_id, version)
);

CREATE TABLE sessions (
    id              TEXT PRIMARY KEY,
    title           TEXT,
    created_at      TEXT NOT NULL,
    updated_at      TEXT NOT NULL,
    working_context TEXT
);

CREATE TABLE turns (
    id           TEXT PRIMARY KEY,
    session_id   TEXT NOT NULL REFERENCES sessions(id) ON DELETE CASCADE,
    seq          INTEGER NOT NULL,
    at           TEXT NOT NULL,
    input        TEXT NOT NULL,
    mode         TEXT,
    outcome      TEXT NOT NULL,
    stale        INTEGER NOT NULL DEFAULT 0,
    stale_reason TEXT,
    result       TEXT NOT NULL,
    UNIQUE (session_id, seq)
);

CREATE INDEX idx_turns_session ON turns(session_id, seq);

CREATE TABLE conclusions (
    id           TEXT PRIMARY KEY,
    session_id   TEXT NOT NULL,
    turn_id      TEXT NOT NULL,
    status       TEXT NOT NULL CHECK (status IN ('valid', 'stale')),
    stale_reason TEXT,
    created_at   TEXT NOT NULL
);

CREATE INDEX idx_conclusions_turn ON conclusions(turn_id);

CREATE TABLE conclusion_assertions (
    conclusion_id TEXT NOT NULL REFERENCES conclusions(id) ON DELETE CASCADE,
    assertion_id  TEXT NOT NULL,
    PRIMARY KEY (conclusion_id, assertion_id)
);

CREATE INDEX idx_conclusion_assertions_assertion
    ON conclusion_assertions(assertion_id);
