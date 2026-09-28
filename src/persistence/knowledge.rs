//! Knowledge memory: assertions, provenance sources, versions, and the
//! conclusions that depend on them.
//!
//! Every stored assertion is a deliberate, versioned record with a source.
//! Correcting or retracting an assertion marks every turn whose answer
//! depended on it as stale, so a later question recomputes against current
//! memory instead of reusing an invalidated conclusion.

use serde::{Deserialize, Serialize};

use super::db::Database;
use super::ids;

fn now_iso() -> String {
    chrono::Utc::now().to_rfc3339()
}

fn conversion_error(index: usize, message: String) -> rusqlite::Error {
    rusqlite::Error::FromSqlConversionFailure(
        index,
        rusqlite::types::Type::Text,
        Box::new(std::io::Error::new(std::io::ErrorKind::InvalidData, message)),
    )
}

/// Whether an assertion is a fact (single SVO) or a causal rule.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AssertionKind {
    Fact,
    Rule,
}

impl AssertionKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            AssertionKind::Fact => "fact",
            AssertionKind::Rule => "rule",
        }
    }

    pub fn parse(value: &str) -> Result<Self, String> {
        match value {
            "fact" => Ok(AssertionKind::Fact),
            "rule" => Ok(AssertionKind::Rule),
            other => Err(format!("unknown assertion kind: {other}")),
        }
    }
}

/// Lifecycle status of an assertion.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AssertionStatus {
    Active,
    Superseded,
    Retracted,
}

impl AssertionStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            AssertionStatus::Active => "active",
            AssertionStatus::Superseded => "superseded",
            AssertionStatus::Retracted => "retracted",
        }
    }

    pub fn parse(value: &str) -> Result<Self, String> {
        match value {
            "active" => Ok(AssertionStatus::Active),
            "superseded" => Ok(AssertionStatus::Superseded),
            "retracted" => Ok(AssertionStatus::Retracted),
            other => Err(format!("unknown assertion status: {other}")),
        }
    }
}

/// A subject-verb-object claim.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct StoredFact {
    pub subject: String,
    pub verb: String,
    pub object: String,
}

impl StoredFact {
    pub fn new(subject: impl Into<String>, verb: impl Into<String>, object: impl Into<String>) -> Self {
        StoredFact {
            subject: subject.into(),
            verb: verb.into(),
            object: object.into(),
        }
    }

    pub fn statement(&self) -> String {
        format!("{} {} {}", self.subject, self.verb, self.object)
    }
}

/// An antecedent -> consequent causal rule.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct StoredRule {
    pub antecedent: StoredFact,
    pub consequent: StoredFact,
    #[serde(default = "default_confidence")]
    pub confidence: f64,
}

fn default_confidence() -> f64 {
    1.0
}

impl StoredRule {
    pub fn statement(&self) -> String {
        format!(
            "if {} then {}",
            self.antecedent.statement(),
            self.consequent.statement()
        )
    }
}

/// The assertion's content, tagged for serialization.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum AssertionPayload {
    Fact {
        subject: String,
        verb: String,
        object: String,
    },
    Rule {
        antecedent: StoredFact,
        consequent: StoredFact,
        confidence: f64,
    },
}

impl AssertionPayload {
    pub fn fact(subject: &str, verb: &str, object: &str) -> Self {
        AssertionPayload::Fact {
            subject: subject.to_string(),
            verb: verb.to_string(),
            object: object.to_string(),
        }
    }

    pub fn rule(antecedent: StoredFact, consequent: StoredFact, confidence: f64) -> Self {
        AssertionPayload::Rule {
            antecedent,
            consequent,
            confidence,
        }
    }

    pub fn kind(&self) -> AssertionKind {
        match self {
            AssertionPayload::Fact { .. } => AssertionKind::Fact,
            AssertionPayload::Rule { .. } => AssertionKind::Rule,
        }
    }

    pub fn as_fact(&self) -> Option<StoredFact> {
        match self {
            AssertionPayload::Fact {
                subject,
                verb,
                object,
            } => Some(StoredFact::new(subject, verb, object)),
            AssertionPayload::Rule { .. } => None,
        }
    }

    pub fn as_rule(&self) -> Option<StoredRule> {
        match self {
            AssertionPayload::Rule {
                antecedent,
                consequent,
                confidence,
            } => Some(StoredRule {
                antecedent: antecedent.clone(),
                consequent: consequent.clone(),
                confidence: *confidence,
            }),
            AssertionPayload::Fact { .. } => None,
        }
    }

    pub fn statement(&self) -> String {
        match self {
            AssertionPayload::Fact { .. } => self
                .as_fact()
                .map(|fact| fact.statement())
                .unwrap_or_default(),
            AssertionPayload::Rule { .. } => self
                .as_rule()
                .map(|rule| rule.statement())
                .unwrap_or_default(),
        }
    }
}

/// Provenance for one stored assertion (or one revision of it).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct AssertionSource {
    pub id: String,
    pub kind: String,
    pub session_id: Option<String>,
    pub turn_id: Option<String>,
    pub note: String,
    pub created_at: String,
}

/// A source to be recorded alongside a write.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct NewSource {
    pub kind: String,
    pub session_id: Option<String>,
    pub turn_id: Option<String>,
    pub note: String,
}

impl NewSource {
    pub fn new(kind: impl Into<String>) -> Self {
        NewSource {
            kind: kind.into(),
            session_id: None,
            turn_id: None,
            note: String::new(),
        }
    }

    pub fn in_session(mut self, session_id: Option<&str>) -> Self {
        self.session_id = session_id.map(str::to_string);
        self
    }

    pub fn in_turn(mut self, turn_id: Option<&str>) -> Self {
        self.turn_id = turn_id.map(str::to_string);
        self
    }

    pub fn with_note(mut self, note: impl Into<String>) -> Self {
        self.note = note.into();
        self
    }
}

/// A stored, versioned assertion.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct StoredAssertion {
    pub id: String,
    pub kind: AssertionKind,
    pub scope: String,
    pub status: AssertionStatus,
    pub version: u32,
    pub source: AssertionSource,
    pub session_id: Option<String>,
    pub created_at: String,
    pub updated_at: String,
    pub superseded_by: Option<String>,
    pub payload: AssertionPayload,
}

impl StoredAssertion {
    pub fn statement(&self) -> String {
        self.payload.statement()
    }

    pub fn is_active(&self) -> bool {
        self.status == AssertionStatus::Active
    }
}

/// One revision in an assertion's history.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct AssertionVersion {
    pub assertion_id: String,
    pub version: u32,
    pub status: AssertionStatus,
    pub source_id: String,
    pub payload: String,
    pub note: String,
    pub recorded_at: String,
}

/// A turn that depended on changed knowledge.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct StaleTurn {
    pub session_id: String,
    pub turn_id: String,
    pub reason: String,
}

const ASSERTION_COLUMNS: &str = "a.id, a.kind, a.scope, a.status, a.version, \
     a.session_id, a.created_at, a.updated_at, a.superseded_by, a.payload, \
     s.id, s.kind, s.session_id, s.turn_id, s.note, s.created_at";

fn row_to_assertion(row: &rusqlite::Row<'_>) -> rusqlite::Result<StoredAssertion> {
    let kind_text: String = row.get(1)?;
    let status_text: String = row.get(3)?;
    let payload_text: String = row.get(9)?;
    let kind = AssertionKind::parse(&kind_text).map_err(|error| conversion_error(1, error))?;
    let status =
        AssertionStatus::parse(&status_text).map_err(|error| conversion_error(3, error))?;
    let payload: AssertionPayload =
        serde_json::from_str(&payload_text).map_err(|error| conversion_error(9, error.to_string()))?;
    Ok(StoredAssertion {
        id: row.get(0)?,
        kind,
        scope: row.get(2)?,
        status,
        version: row.get(4)?,
        session_id: row.get(5)?,
        created_at: row.get(6)?,
        updated_at: row.get(7)?,
        superseded_by: row.get(8)?,
        payload,
        source: AssertionSource {
            id: row.get(10)?,
            kind: row.get(11)?,
            session_id: row.get(12)?,
            turn_id: row.get(13)?,
            note: row.get(14)?,
            created_at: row.get(15)?,
        },
    })
}

fn insert_source(
    transaction: &rusqlite::Transaction<'_>,
    source: &NewSource,
) -> Result<String, String> {
    let id = ids::new_source_id();
    transaction
        .execute(
            "INSERT INTO sources (id, kind, session_id, turn_id, note, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            rusqlite::params![
                id,
                source.kind,
                source.session_id,
                source.turn_id,
                source.note,
                now_iso()
            ],
        )
        .map_err(|error| format!("insert source: {error}"))?;
    Ok(id)
}

fn payload_columns(payload: &AssertionPayload) -> [Option<String>; 9] {
    match payload {
        AssertionPayload::Fact {
            subject,
            verb,
            object,
        } => [
            Some(subject.clone()),
            Some(verb.clone()),
            Some(object.clone()),
            None,
            None,
            None,
            None,
            None,
            None,
        ],
        AssertionPayload::Rule {
            antecedent,
            consequent,
            ..
        } => [
            None,
            None,
            None,
            Some(antecedent.subject.clone()),
            Some(antecedent.verb.clone()),
            Some(antecedent.object.clone()),
            Some(consequent.subject.clone()),
            Some(consequent.verb.clone()),
            Some(consequent.object.clone()),
        ],
    }
}

fn insert_assertion_row(
    transaction: &rusqlite::Transaction<'_>,
    id: &str,
    payload: &AssertionPayload,
    scope: &str,
    session_id: Option<&str>,
    source_id: &str,
) -> Result<(), String> {
    let columns = payload_columns(payload);
    let payload_text = serde_json::to_string(payload)
        .map_err(|error| format!("serialize assertion: {error}"))?;
    let confidence = payload.as_rule().map(|rule| rule.confidence);
    let now = now_iso();
    transaction
        .execute(
            "INSERT INTO assertions (
                 id, kind, scope, status, version, source_id, session_id,
                 created_at, updated_at, subject, verb, object,
                 ante_subject, ante_verb, ante_object,
                 cons_subject, cons_verb, cons_object, confidence, payload
             ) VALUES (
                 ?1, ?2, ?3, 'active', 1, ?4, ?5, ?6, ?6,
                 ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17
             )",
            rusqlite::params![
                id,
                payload.kind().as_str(),
                scope,
                source_id,
                session_id,
                now,
                columns[0],
                columns[1],
                columns[2],
                columns[3],
                columns[4],
                columns[5],
                columns[6],
                columns[7],
                columns[8],
                confidence,
                payload_text,
            ],
        )
        .map_err(|error| format!("insert assertion: {error}"))?;
    insert_version_row(
        transaction,
        id,
        1,
        AssertionStatus::Active,
        source_id,
        &payload_text,
        "created",
    )
}

fn insert_version_row(
    transaction: &rusqlite::Transaction<'_>,
    assertion_id: &str,
    version: u32,
    status: AssertionStatus,
    source_id: &str,
    payload_text: &str,
    note: &str,
) -> Result<(), String> {
    transaction
        .execute(
            "INSERT INTO assertion_versions
                 (assertion_id, version, status, source_id, payload, note, recorded_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            rusqlite::params![
                assertion_id,
                version,
                status.as_str(),
                source_id,
                payload_text,
                note,
                now_iso(),
            ],
        )
        .map_err(|error| format!("insert assertion version: {error}"))?;
    Ok(())
}

/// Find the turns whose conclusions depended on any of `assertion_ids`,
/// mark those conclusions and turns stale, and return the affected turns.
fn mark_dependents_stale(
    transaction: &rusqlite::Transaction<'_>,
    assertion_ids: &[String],
    reason: &str,
) -> Result<Vec<StaleTurn>, String> {
    if assertion_ids.is_empty() {
        return Ok(Vec::new());
    }
    let placeholders = std::iter::repeat("?")
        .take(assertion_ids.len())
        .collect::<Vec<_>>()
        .join(", ");
    let affected: Vec<(String, String)> = {
        let sql = format!(
            "SELECT DISTINCT c.session_id, c.turn_id
             FROM conclusions c
             JOIN conclusion_assertions ca ON ca.conclusion_id = c.id
             WHERE ca.assertion_id IN ({placeholders})"
        );
        let mut statement = transaction
            .prepare(&sql)
            .map_err(|error| format!("prepare dependent lookup: {error}"))?;
        let rows = statement
            .query_map(rusqlite::params_from_iter(assertion_ids.iter()), |row| {
                Ok((row.get(0)?, row.get(1)?))
            })
            .map_err(|error| format!("query dependent turns: {error}"))?;
        rows.collect::<rusqlite::Result<Vec<_>>>()
            .map_err(|error| format!("read dependent turns: {error}"))?
    };
    if affected.is_empty() {
        return Ok(Vec::new());
    }
    let update_conclusions = format!(
        "UPDATE conclusions SET status = 'stale', stale_reason = ?1
         WHERE id IN (SELECT conclusion_id FROM conclusion_assertions
                      WHERE assertion_id IN ({placeholders}))"
    );
    let mut statement = transaction
        .prepare(&update_conclusions)
        .map_err(|error| format!("prepare conclusion update: {error}"))?;
    statement
        .execute(rusqlite::params_from_iter(
            std::iter::once(reason).chain(assertion_ids.iter().map(String::as_str)),
        ))
        .map_err(|error| format!("mark conclusions stale: {error}"))?;
    drop(statement);

    let turn_placeholders = std::iter::repeat("?")
        .take(affected.len())
        .collect::<Vec<_>>()
        .join(", ");
    let update_turns = format!(
        "UPDATE turns SET stale = 1, stale_reason = ?1 WHERE id IN ({turn_placeholders})"
    );
    let mut statement = transaction
        .prepare(&update_turns)
        .map_err(|error| format!("prepare turn update: {error}"))?;
    statement
        .execute(rusqlite::params_from_iter(
            std::iter::once(reason).chain(affected.iter().map(|(_, turn)| turn.as_str())),
        ))
        .map_err(|error| format!("mark turns stale: {error}"))?;
    drop(statement);

    Ok(affected
        .into_iter()
        .map(|(session_id, turn_id)| StaleTurn {
            session_id,
            turn_id,
            reason: reason.to_string(),
        })
        .collect())
}

/// Record a conclusion and its dependency links inside an existing
/// transaction.
pub(crate) fn record_conclusion_tx(
    transaction: &rusqlite::Transaction<'_>,
    session_id: &str,
    turn_id: &str,
    assertion_ids: &[String],
) -> Result<(), String> {
    let mut seen = std::collections::HashSet::new();
    let unique: Vec<&String> = assertion_ids
        .iter()
        .filter(|id| seen.insert((*id).clone()))
        .collect();
    if unique.is_empty() {
        return Ok(());
    }
    let conclusion_id = ids::new_conclusion_id();
    transaction
        .execute(
            "INSERT INTO conclusions (id, session_id, turn_id, status, created_at)
             VALUES (?1, ?2, ?3, 'valid', ?4)",
            rusqlite::params![conclusion_id, session_id, turn_id, now_iso()],
        )
        .map_err(|error| format!("insert conclusion: {error}"))?;
    for assertion_id in &unique {
        transaction
            .execute(
                "INSERT OR IGNORE INTO conclusion_assertions
                     (conclusion_id, assertion_id) VALUES (?1, ?2)",
                rusqlite::params![conclusion_id, assertion_id],
            )
            .map_err(|error| format!("link conclusion assertion: {error}"))?;
    }
    Ok(())
}

impl Database {
    /// Insert a new assertion with its provenance source. One transaction.
    pub fn insert_assertion(
        &mut self,
        payload: AssertionPayload,
        source: NewSource,
    ) -> Result<StoredAssertion, String> {
        let id = match payload.kind() {
            AssertionKind::Fact => ids::new_fact_id(),
            AssertionKind::Rule => ids::new_rule_id(),
        };
        let session_id = source.session_id.clone();
        self.transaction(|transaction| {
            let source_id = insert_source(transaction, &source)?;
            insert_assertion_row(
                transaction,
                &id,
                &payload,
                "shared",
                session_id.as_deref(),
                &source_id,
            )?;
            Ok(())
        })?;
        self.assertion(&id)?
            .ok_or_else(|| "assertion vanished after insert".to_string())
    }

    /// Record another provenance event for an existing active assertion
    /// without changing its content. Versions increase so the history shows
    /// every time the claim was reaffirmed and by whom.
    pub fn reaffirm_assertion(
        &mut self,
        id: &str,
        source: NewSource,
        note: &str,
    ) -> Result<StoredAssertion, String> {
        self.transaction(|transaction| {
            let current = fetch_assertion(transaction, id)?
                .ok_or_else(|| format!("assertion not found: {id}"))?;
            if !current.is_active() {
                return Err(format!(
                    "assertion {id} is {} and cannot be reaffirmed",
                    current.status.as_str()
                ));
            }
            let source_id = insert_source(transaction, &source)?;
            let next_version = current.version + 1;
            let payload_text = serde_json::to_string(&current.payload)
                .map_err(|error| format!("serialize assertion: {error}"))?;
            transaction
                .execute(
                    "UPDATE assertions
                     SET version = ?2, source_id = ?3, updated_at = ?4
                     WHERE id = ?1",
                    rusqlite::params![id, next_version, source_id, now_iso()],
                )
                .map_err(|error| format!("reaffirm assertion: {error}"))?;
            insert_version_row(
                transaction,
                id,
                next_version,
                AssertionStatus::Active,
                &source_id,
                &payload_text,
                note,
            )?;
            Ok(())
        })?;
        self.assertion(id)?
            .ok_or_else(|| "assertion vanished after reaffirm".to_string())
    }

    /// Replace an assertion's content with a corrected version. The previous
    /// version stays in the history and every dependent turn is marked stale.
    pub fn correct_assertion(
        &mut self,
        id: &str,
        replacement: AssertionPayload,
        source: NewSource,
        note: &str,
    ) -> Result<(StoredAssertion, Vec<StaleTurn>), String> {
        let mut affected = Vec::new();
        self.transaction(|transaction| {
            let current = fetch_assertion(transaction, id)?
                .ok_or_else(|| format!("assertion not found: {id}"))?;
            if !current.is_active() {
                return Err(format!(
                    "assertion {id} is {} and cannot be corrected",
                    current.status.as_str()
                ));
            }
            if current.kind != replacement.kind() {
                return Err(format!(
                    "correction kind mismatch: {} assertion replaced with {}",
                    current.kind.as_str(),
                    replacement.kind().as_str()
                ));
            }
            let source_id = insert_source(transaction, &source)?;
            let next_version = current.version + 1;
            let payload_text = serde_json::to_string(&replacement)
                .map_err(|error| format!("serialize correction: {error}"))?;
            let columns = payload_columns(&replacement);
            let confidence = replacement.as_rule().map(|rule| rule.confidence);
            transaction
                .execute(
                    "UPDATE assertions
                     SET version = ?2, source_id = ?3, updated_at = ?4, payload = ?5,
                         subject = ?6, verb = ?7, object = ?8,
                         ante_subject = ?9, ante_verb = ?10, ante_object = ?11,
                         cons_subject = ?12, cons_verb = ?13, cons_object = ?14,
                         confidence = ?15
                     WHERE id = ?1",
                    rusqlite::params![
                        id,
                        next_version,
                        source_id,
                        now_iso(),
                        payload_text,
                        columns[0],
                        columns[1],
                        columns[2],
                        columns[3],
                        columns[4],
                        columns[5],
                        columns[6],
                        columns[7],
                        columns[8],
                        confidence,
                    ],
                )
                .map_err(|error| format!("correct assertion: {error}"))?;
            insert_version_row(
                transaction,
                id,
                next_version,
                AssertionStatus::Active,
                &source_id,
                &payload_text,
                note,
            )?;
            let reason = format!(
                "corrected: \"{}\" is now \"{}\"",
                current.statement(),
                replacement.statement()
            );
            affected = mark_dependents_stale(transaction, &[id.to_string()], &reason)?;
            Ok(())
        })?;
        let assertion = self
            .assertion(id)?
            .ok_or_else(|| "assertion vanished after correction".to_string())?;
        Ok((assertion, affected))
    }

    /// Retract an assertion. The row is kept as a tombstone so it cannot be
    /// silently resurrected by a later import or reload.
    pub fn retract_assertion(
        &mut self,
        id: &str,
        source: NewSource,
        reason: &str,
    ) -> Result<(StoredAssertion, Vec<StaleTurn>), String> {
        let mut affected = Vec::new();
        self.transaction(|transaction| {
            let current = fetch_assertion(transaction, id)?
                .ok_or_else(|| format!("assertion not found: {id}"))?;
            if !current.is_active() {
                return Err(format!(
                    "assertion {id} is already {}",
                    current.status.as_str()
                ));
            }
            let source_id = insert_source(transaction, &source)?;
            let next_version = current.version + 1;
            let payload_text = serde_json::to_string(&current.payload)
                .map_err(|error| format!("serialize assertion: {error}"))?;
            transaction
                .execute(
                    "UPDATE assertions
                     SET status = 'retracted', version = ?2, source_id = ?3, updated_at = ?4
                     WHERE id = ?1",
                    rusqlite::params![id, next_version, source_id, now_iso()],
                )
                .map_err(|error| format!("retract assertion: {error}"))?;
            insert_version_row(
                transaction,
                id,
                next_version,
                AssertionStatus::Retracted,
                &source_id,
                &payload_text,
                reason,
            )?;
            let stale_reason = format!("retracted: \"{}\" ({reason})", current.statement());
            affected = mark_dependents_stale(transaction, &[id.to_string()], &stale_reason)?;
            Ok(())
        })?;
        let assertion = self
            .assertion(id)?
            .ok_or_else(|| "assertion vanished after retraction".to_string())?;
        Ok((assertion, affected))
    }

    /// Remove an assertion entirely. Dependent turns are still marked stale
    /// before the record is deleted, and the links to it are removed.
    pub fn forget_assertion(&mut self, id: &str) -> Result<(bool, Vec<StaleTurn>), String> {
        let mut affected = Vec::new();
        let existed = self.transaction(|transaction| {
            let Some(current) = fetch_assertion(transaction, id)? else {
                return Ok(false);
            };
            let reason = format!("forgotten: \"{}\" was removed from memory", current.statement());
            affected = mark_dependents_stale(transaction, &[id.to_string()], &reason)?;
            transaction
                .execute(
                    "DELETE FROM conclusion_assertions WHERE assertion_id = ?1",
                    [id],
                )
                .map_err(|error| format!("unlink conclusion assertions: {error}"))?;
            transaction
                .execute("DELETE FROM assertions WHERE id = ?1", [id])
                .map_err(|error| format!("forget assertion: {error}"))?;
            Ok(true)
        })?;
        Ok((existed, affected))
    }

    pub fn assertion(&self, id: &str) -> Result<Option<StoredAssertion>, String> {
        fetch_assertion(self.connection(), id)
    }

    /// All active assertions, oldest first.
    pub fn active_assertions(&self) -> Result<Vec<StoredAssertion>, String> {
        self.list_assertions(None, None, Some(AssertionStatus::Active), None)
    }

    /// List assertions with optional substring, kind, and status filters.
    pub fn list_assertions(
        &self,
        query: Option<&str>,
        kind: Option<AssertionKind>,
        status: Option<AssertionStatus>,
        limit: Option<usize>,
    ) -> Result<Vec<StoredAssertion>, String> {
        let mut sql = format!(
            "SELECT {ASSERTION_COLUMNS} FROM assertions a
             JOIN sources s ON s.id = a.source_id WHERE 1 = 1"
        );
        let mut parameters: Vec<Box<dyn rusqlite::ToSql>> = Vec::new();
        if let Some(kind) = kind {
            sql.push_str(" AND a.kind = ?");
            parameters.push(Box::new(kind.as_str().to_string()));
        }
        if let Some(status) = status {
            sql.push_str(" AND a.status = ?");
            parameters.push(Box::new(status.as_str().to_string()));
        }
        if let Some(query) = query.map(str::trim).filter(|value| !value.is_empty()) {
            sql.push_str(
                " AND (LOWER(a.subject) LIKE ? OR LOWER(a.verb) LIKE ? OR LOWER(a.object) LIKE ?
                   OR LOWER(a.ante_subject) LIKE ? OR LOWER(a.ante_object) LIKE ?
                   OR LOWER(a.cons_subject) LIKE ? OR LOWER(a.cons_object) LIKE ?
                   OR LOWER(a.payload) LIKE ?)",
            );
            let needle = format!("%{}%", query.to_lowercase());
            for _ in 0..8 {
                parameters.push(Box::new(needle.clone()));
            }
        }
        sql.push_str(" ORDER BY a.created_at ASC, a.id ASC");
        if let Some(limit) = limit {
            sql.push_str(&format!(" LIMIT {}", limit.min(1000)));
        }
        let mut statement = self
            .connection()
            .prepare(&sql)
            .map_err(|error| format!("prepare assertion list: {error}"))?;
        let rows = statement
            .query_map(
                rusqlite::params_from_iter(parameters.iter().map(|value| value.as_ref())),
                row_to_assertion,
            )
            .map_err(|error| format!("query assertions: {error}"))?;
        rows.collect::<rusqlite::Result<Vec<_>>>()
            .map_err(|error| format!("read assertions: {error}"))
    }

    pub fn find_active_fact(
        &self,
        subject: &str,
        verb: &str,
        object: &str,
    ) -> Result<Option<StoredAssertion>, String> {
        let mut statement = self
            .connection()
            .prepare(&format!(
                "SELECT {ASSERTION_COLUMNS} FROM assertions a
                 JOIN sources s ON s.id = a.source_id
                 WHERE a.kind = 'fact' AND a.status = 'active'
                   AND a.subject = ?1 AND a.verb = ?2 AND a.object = ?3
                 ORDER BY a.updated_at DESC LIMIT 1"
            ))
            .map_err(|error| format!("prepare fact lookup: {error}"))?;
        let mut rows = statement
            .query(rusqlite::params![subject, verb, object])
            .map_err(|error| format!("query fact: {error}"))?;
        match rows
            .next()
            .map_err(|error| format!("read fact: {error}"))?
        {
            Some(row) => row_to_assertion(row)
                .map(Some)
                .map_err(|error| format!("decode assertion: {error}")),
            None => Ok(None),
        }
    }

    pub fn find_active_rule(
        &self,
        antecedent: &StoredFact,
        consequent: &StoredFact,
    ) -> Result<Option<StoredAssertion>, String> {
        let mut statement = self
            .connection()
            .prepare(&format!(
                "SELECT {ASSERTION_COLUMNS} FROM assertions a
                 JOIN sources s ON s.id = a.source_id
                 WHERE a.kind = 'rule' AND a.status = 'active'
                   AND a.ante_subject = ?1 AND a.ante_verb = ?2 AND a.ante_object = ?3
                   AND a.cons_subject = ?4 AND a.cons_verb = ?5 AND a.cons_object = ?6
                 ORDER BY a.updated_at DESC LIMIT 1"
            ))
            .map_err(|error| format!("prepare rule lookup: {error}"))?;
        let mut rows = statement
            .query(rusqlite::params![
                antecedent.subject,
                antecedent.verb,
                antecedent.object,
                consequent.subject,
                consequent.verb,
                consequent.object,
            ])
            .map_err(|error| format!("query rule: {error}"))?;
        match rows
            .next()
            .map_err(|error| format!("read rule: {error}"))?
        {
            Some(row) => row_to_assertion(row)
                .map(Some)
                .map_err(|error| format!("decode assertion: {error}")),
            None => Ok(None),
        }
    }

    pub fn assertion_versions(&self, id: &str) -> Result<Vec<AssertionVersion>, String> {
        let mut statement = self
            .connection()
            .prepare(
                "SELECT assertion_id, version, status, source_id, payload, note, recorded_at
                 FROM assertion_versions WHERE assertion_id = ?1 ORDER BY version ASC",
            )
            .map_err(|error| format!("prepare version list: {error}"))?;
        let rows = statement
            .query_map([id], |row| {
                let status_text: String = row.get(2)?;
                let status = AssertionStatus::parse(&status_text)
                    .map_err(|error| conversion_error(2, error))?;
                Ok(AssertionVersion {
                    assertion_id: row.get(0)?,
                    version: row.get(1)?,
                    status,
                    source_id: row.get(3)?,
                    payload: row.get(4)?,
                    note: row.get(5)?,
                    recorded_at: row.get(6)?,
                })
            })
            .map_err(|error| format!("query versions: {error}"))?;
        rows.collect::<rusqlite::Result<Vec<_>>>()
            .map_err(|error| format!("read versions: {error}"))
    }

    /// Record which assertions an answered turn depended on.
    pub fn record_conclusion(
        &mut self,
        session_id: &str,
        turn_id: &str,
        assertion_ids: &[String],
    ) -> Result<(), String> {
        self.transaction(|transaction| {
            record_conclusion_tx(transaction, session_id, turn_id, assertion_ids)
        })
    }

    /// Assertion ids an answered turn depended on.
    pub fn assertions_for_turn(&self, turn_id: &str) -> Result<Vec<String>, String> {
        let mut statement = self
            .connection()
            .prepare(
                "SELECT DISTINCT ca.assertion_id
                 FROM conclusion_assertions ca
                 JOIN conclusions c ON c.id = ca.conclusion_id
                 WHERE c.turn_id = ?1",
            )
            .map_err(|error| format!("prepare turn assertions: {error}"))?;
        let rows = statement
            .query_map([turn_id], |row| row.get::<_, String>(0))
            .map_err(|error| format!("query turn assertions: {error}"))?;
        rows.collect::<rusqlite::Result<Vec<_>>>()
            .map_err(|error| format!("read turn assertions: {error}"))
    }

    /// Count active assertions of each kind: (facts, rules).
    pub fn assertion_counts(&self) -> Result<(usize, usize), String> {
        let mut statement = self
            .connection()
            .prepare(
                "SELECT kind, COUNT(*) FROM assertions WHERE status = 'active' GROUP BY kind",
            )
            .map_err(|error| format!("prepare assertion counts: {error}"))?;
        let rows = statement
            .query_map([], |row| Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?)))
            .map_err(|error| format!("query assertion counts: {error}"))?;
        let mut facts = 0usize;
        let mut rules = 0usize;
        for row in rows {
            let (kind, count) =
                row.map_err(|error| format!("read assertion counts: {error}"))?;
            match kind.as_str() {
                "fact" => facts = count as usize,
                "rule" => rules = count as usize,
                _ => {}
            }
        }
        Ok((facts, rules))
    }
}

fn fetch_assertion(
    connection: &rusqlite::Connection,
    id: &str,
) -> Result<Option<StoredAssertion>, String> {
    let mut statement = connection
        .prepare(&format!(
            "SELECT {ASSERTION_COLUMNS} FROM assertions a
             JOIN sources s ON s.id = a.source_id WHERE a.id = ?1"
        ))
        .map_err(|error| format!("prepare assertion lookup: {error}"))?;
    let mut rows = statement
        .query([id])
        .map_err(|error| format!("query assertion: {error}"))?;
    match rows
        .next()
        .map_err(|error| format!("read assertion: {error}"))?
    {
        Some(row) => row_to_assertion(row)
                .map(Some)
                .map_err(|error| format!("decode assertion: {error}")),
        None => Ok(None),
    }
}
