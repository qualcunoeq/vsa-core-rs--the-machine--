//! SQLite connection management, schema migrations, and backup/restore.
//!
//! All durable runtime state lives in one database file. Writes are
//! transactional; migrations are applied in order and recorded so an older
//! database is upgraded exactly once and a newer one is refused rather than
//! silently misinterpreted.

use std::path::{Path, PathBuf};

use rusqlite::Connection;

/// Current schema version. Bump when adding a migration.
pub const SCHEMA_VERSION: u32 = 2;

/// Migration statements, applied in order. Index + 1 is the schema version
/// the migration produces.
const MIGRATIONS: &[&str] = &[
    include_str!("migrations/0001_initial.sql"),
    include_str!("migrations/0002_documents.sql"),
];

/// A durable store backed by SQLite.
///
/// The connection is intentionally owned by a single service behind a mutex:
/// SQLite serializes writers anyway, and this keeps turn processing, memory
/// operations, and persistence on one consistent timeline.
pub struct Database {
    conn: Connection,
    path: Option<PathBuf>,
}

impl Database {
    /// Open (or create) the database at `path`, apply migrations, and return
    /// the ready store. Parent directories are created as needed.
    pub fn open(path: impl AsRef<Path>) -> Result<Self, String> {
        let path = path.as_ref().to_path_buf();
        if let Some(parent) = path.parent() {
            if !parent.as_os_str().is_empty() {
                std::fs::create_dir_all(parent)
                    .map_err(|error| format!("create database directory: {error}"))?;
            }
        }
        let conn = Connection::open(&path)
            .map_err(|error| format!("open database {}: {error}", path.display()))?;
        let mut database = Database {
            conn,
            path: Some(path),
        };
        database.configure()?;
        database.migrate()?;
        Ok(database)
    }

    /// Open a private in-memory database (used by tests and `--memory` only
    /// servers). State does not survive the process.
    pub fn open_in_memory() -> Result<Self, String> {
        let conn = Connection::open_in_memory()
            .map_err(|error| format!("open in-memory database: {error}"))?;
        let mut database = Database { conn, path: None };
        database.configure()?;
        database.migrate()?;
        Ok(database)
    }

    fn configure(&mut self) -> Result<(), String> {
        self.conn
            .execute_batch(
                "PRAGMA foreign_keys = ON;\n\
                 PRAGMA journal_mode = WAL;\n\
                 PRAGMA synchronous = NORMAL;\n\
                 PRAGMA busy_timeout = 5000;",
            )
            .map_err(|error| format!("configure database: {error}"))
    }

    /// Apply any pending migrations inside one transaction per version.
    pub fn migrate(&mut self) -> Result<(), String> {
        let current: u32 = self
            .conn
            .query_row("PRAGMA user_version", [], |row| row.get(0))
            .map_err(|error| format!("read schema version: {error}"))?;
        if current > SCHEMA_VERSION {
            return Err(format!(
                "database schema version {current} is newer than this build supports \
                 ({SCHEMA_VERSION}); refusing to open"
            ));
        }
        for version in current..SCHEMA_VERSION {
            let statements = MIGRATIONS[version as usize];
            let transaction = self
                .conn
                .transaction()
                .map_err(|error| format!("begin migration {}: {error}", version + 1))?;
            transaction
                .execute_batch(statements)
                .map_err(|error| format!("apply migration {}: {error}", version + 1))?;
            transaction
                .pragma_update(None, "user_version", version + 1)
                .map_err(|error| format!("record migration {}: {error}", version + 1))?;
            transaction
                .commit()
                .map_err(|error| format!("commit migration {}: {error}", version + 1))?;
        }
        Ok(())
    }

    pub fn schema_version(&self) -> Result<u32, String> {
        self.conn
            .query_row("PRAGMA user_version", [], |row| row.get(0))
            .map_err(|error| format!("read schema version: {error}"))
    }

    pub fn path(&self) -> Option<&Path> {
        self.path.as_deref()
    }

    /// Run `work` in a single transaction; roll back on any error.
    pub fn transaction<T>(
        &mut self,
        work: impl FnOnce(&rusqlite::Transaction<'_>) -> Result<T, String>,
    ) -> Result<T, String> {
        let transaction = self
            .conn
            .transaction()
            .map_err(|error| format!("begin transaction: {error}"))?;
        let value = work(&transaction)?;
        transaction
            .commit()
            .map_err(|error| format!("commit transaction: {error}"))?;
        Ok(value)
    }

    /// Flush the write-ahead log into the main database file.
    pub fn checkpoint(&self) -> Result<(), String> {
        self.conn
            .query_row("PRAGMA wal_checkpoint(TRUNCATE)", [], |_| Ok(()))
            .map_err(|error| format!("checkpoint database: {error}"))
    }

    /// Verify internal consistency. Returns an error describing the first
    /// problem when the database is damaged.
    pub fn integrity_check(&self) -> Result<(), String> {
        let result: String = self
            .conn
            .query_row("PRAGMA integrity_check", [], |row| row.get(0))
            .map_err(|error| format!("integrity check: {error}"))?;
        if result == "ok" {
            Ok(())
        } else {
            Err(format!("integrity check failed: {result}"))
        }
    }

    /// Write a consistent snapshot of the database to `target`.
    ///
    /// Uses SQLite's `VACUUM INTO`, which is safe while the database is in
    /// use and produces a single self-contained file. The target must not
    /// already exist.
    pub fn backup_to(&self, target: impl AsRef<Path>) -> Result<(), String> {
        let target = target.as_ref();
        if target.exists() {
            return Err(format!("backup target already exists: {}", target.display()));
        }
        if let Some(parent) = target.parent() {
            if !parent.as_os_str().is_empty() {
                std::fs::create_dir_all(parent)
                    .map_err(|error| format!("create backup directory: {error}"))?;
            }
        }
        let target_text = target
            .to_str()
            .ok_or_else(|| "backup target is not valid UTF-8".to_string())?;
        self.conn
            .execute("VACUUM INTO ?1", [target_text])
            .map_err(|error| format!("backup database: {error}"))?;
        Ok(())
    }

    /// Replace the current database contents with a backup file.
    ///
    /// The backup is validated first (readable, consistent, schema not newer
    /// than this build). The current contents are then overwritten in place
    /// through SQLite's backup API, so the destination stays a valid database
    /// even if the process dies mid-restore.
    pub fn restore_from(&mut self, source: impl AsRef<Path>) -> Result<(), String> {
        let source = source.as_ref();
        if !source.exists() {
            return Err(format!("backup file not found: {}", source.display()));
        }
        let source_conn = Connection::open_with_flags(
            source,
            rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY,
        )
        .map_err(|error| format!("open backup {}: {error}", source.display()))?;
        let version: u32 = source_conn
            .query_row("PRAGMA user_version", [], |row| row.get(0))
            .map_err(|error| format!("read backup schema version: {error}"))?;
        if version > SCHEMA_VERSION {
            return Err(format!(
                "backup schema version {version} is newer than this build supports \
                 ({SCHEMA_VERSION})"
            ));
        }
        let integrity: String = source_conn
            .query_row("PRAGMA integrity_check", [], |row| row.get(0))
            .map_err(|error| format!("check backup: {error}"))?;
        if integrity != "ok" {
            return Err(format!("backup integrity check failed: {integrity}"));
        }

        {
            let backup = rusqlite::backup::Backup::new(&source_conn, &mut self.conn)
                .map_err(|error| format!("start restore: {error}"))?;
            backup
                .run_to_completion(64, std::time::Duration::from_millis(5), None)
                .map_err(|error| format!("restore database: {error}"))?;
        }
        self.migrate()
    }

    pub fn get_meta(&self, key: &str) -> Result<Option<String>, String> {
        let mut statement = self
            .conn
            .prepare("SELECT value FROM meta WHERE key = ?1")
            .map_err(|error| format!("prepare meta lookup: {error}"))?;
        let mut rows = statement
            .query([key])
            .map_err(|error| format!("query meta: {error}"))?;
        match rows.next().map_err(|error| format!("read meta: {error}"))? {
            Some(row) => row
                .get(0)
                .map(Some)
                .map_err(|error| format!("decode meta: {error}")),
            None => Ok(None),
        }
    }

    pub fn set_meta(&mut self, key: &str, value: &str) -> Result<(), String> {
        self.conn
            .execute(
                "INSERT INTO meta (key, value) VALUES (?1, ?2)
                 ON CONFLICT(key) DO UPDATE SET value = excluded.value",
                [key, value],
            )
            .map_err(|error| format!("write meta {key}: {error}"))?;
        Ok(())
    }

    pub(crate) fn connection(&self) -> &Connection {
        &self.conn
    }
}
