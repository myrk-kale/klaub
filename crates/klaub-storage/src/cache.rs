//! The derived-data cache (SQLite) in the user's private cache directory.
//!
//! Nothing here is precious: every entry can be recomputed, so an unreadable or incompatible
//! database is deleted and recreated rather than reported as a failure.

use std::path::Path;
use std::sync::Mutex;

use klaub_core::ports::{AnalysisCache, CacheKey};
use klaub_core::{Error, Result};
use rusqlite::{Connection, OptionalExtension, params};

use crate::fsutil::private_dir;

const SCHEMA_VERSION: i64 = 1;
/// When the table holds more rows than this, the least recently used ones are dropped.
const MAX_ROWS: i64 = 200_000;

/// What [`SqliteCache::check`] found in a cache file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CacheHealth {
    /// Stored entries.
    pub entries: u64,
    /// SQLite's own integrity verdict: `ok`, or the first problem it reported.
    pub integrity: String,
    /// The file's schema version, which `SqliteCache::open` replaces when it is not the current one.
    pub schema_version: i64,
}

impl CacheHealth {
    /// The file is intact and of the current schema.
    pub fn is_healthy(&self) -> bool {
        self.integrity == "ok" && self.schema_version == SCHEMA_VERSION
    }
}

/// A SQLite-backed [`AnalysisCache`].
pub struct SqliteCache {
    connection: Mutex<Connection>,
}

fn sql_error(error: &rusqlite::Error) -> Error {
    Error::Storage(format!("cache database: {error}"))
}

impl SqliteCache {
    /// Opens (creating if necessary) the cache at `path`. A broken file is replaced.
    pub fn open(path: &Path) -> Result<Self> {
        if let Some(parent) = path.parent() {
            private_dir(parent)?;
        }
        match Self::try_open(path) {
            Ok(cache) => Ok(cache),
            Err(error) => {
                tracing::warn!(%error, "the cache database is unusable; recreating it");
                for suffix in ["", "-wal", "-shm"] {
                    let _ = std::fs::remove_file(format!("{}{suffix}", path.display()));
                }
                Self::try_open(path)
            }
        }
    }

    /// An in-memory cache, for tests.
    pub fn in_memory() -> Result<Self> {
        let connection = Connection::open_in_memory().map_err(|e| sql_error(&e))?;
        Self::init(connection)
    }

    fn try_open(path: &Path) -> Result<Self> {
        let connection = Connection::open(path).map_err(|e| sql_error(&e))?;
        Self::init(connection)
    }

    fn init(connection: Connection) -> Result<Self> {
        connection.execute_batch("PRAGMA journal_mode = WAL; PRAGMA synchronous = NORMAL; PRAGMA busy_timeout = 2000;").map_err(|e| sql_error(&e))?;
        let version: i64 = connection
            .query_row("PRAGMA user_version", [], |row| row.get(0))
            .map_err(|e| sql_error(&e))?;
        if version != 0 && version != SCHEMA_VERSION {
            // Derived data of another schema is simply dropped.
            connection
                .execute_batch("DROP TABLE IF EXISTS entries;")
                .map_err(|e| sql_error(&e))?;
        }
        connection
            .execute_batch(&format!(
                "CREATE TABLE IF NOT EXISTS entries (
                    worktree TEXT NOT NULL,
                    content_hash TEXT NOT NULL,
                    parser_version INTEGER NOT NULL,
                    kind TEXT NOT NULL,
                    value BLOB NOT NULL,
                    accessed INTEGER NOT NULL,
                    PRIMARY KEY (worktree, content_hash, parser_version, kind)
                ) WITHOUT ROWID;
                CREATE INDEX IF NOT EXISTS entries_accessed ON entries (accessed);
                PRAGMA user_version = {SCHEMA_VERSION};"
            ))
            .map_err(|e| sql_error(&e))?;
        Ok(Self {
            connection: Mutex::new(connection),
        })
    }

    fn connection(&self) -> Result<std::sync::MutexGuard<'_, Connection>> {
        self.connection
            .lock()
            .map_err(|_| Error::Storage("the cache lock was poisoned".to_owned()))
    }

    /// Looks at a cache file without changing it: no table is created, no row is written and a
    /// damaged file is not replaced. `klaub doctor` uses this so that asking is never a repair.
    ///
    /// # Errors
    ///
    /// Fails when the file cannot be opened as a SQLite database at all.
    pub fn check(path: &Path) -> Result<CacheHealth> {
        let connection = Connection::open_with_flags(
            path,
            rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY | rusqlite::OpenFlags::SQLITE_OPEN_NO_MUTEX,
        )
        .map_err(|e| sql_error(&e))?;
        let integrity: String = connection
            .query_row("PRAGMA integrity_check(1)", [], |row| row.get(0))
            .map_err(|e| sql_error(&e))?;
        let schema_version: i64 = connection
            .query_row("PRAGMA user_version", [], |row| row.get(0))
            .map_err(|e| sql_error(&e))?;
        let entries: i64 = connection
            .query_row("SELECT COUNT(*) FROM entries", [], |row| row.get(0))
            .unwrap_or(0);
        Ok(CacheHealth {
            entries: u64::try_from(entries).unwrap_or(0),
            integrity,
            schema_version,
        })
    }

    /// Number of stored entries.
    pub fn len(&self) -> Result<usize> {
        let count: i64 = self
            .connection()?
            .query_row("SELECT COUNT(*) FROM entries", [], |row| row.get(0))
            .map_err(|e| sql_error(&e))?;
        Ok(usize::try_from(count).unwrap_or(0))
    }

    /// Whether the cache holds nothing.
    pub fn is_empty(&self) -> Result<bool> {
        Ok(self.len()? == 0)
    }
}

impl AnalysisCache for SqliteCache {
    fn get(&self, key: &CacheKey) -> Result<Option<Vec<u8>>> {
        let connection = self.connection()?;
        let found: Option<Vec<u8>> = connection
            .query_row(
                "SELECT value FROM entries WHERE worktree = ?1 AND content_hash = ?2 AND parser_version = ?3 AND kind = ?4",
                params![key.worktree.as_str(), key.content_hash, key.parser_version, key.kind],
                |row| row.get(0),
            )
            .optional()
            .map_err(|e| sql_error(&e))?;
        if found.is_some() {
            let _ = connection.execute(
                "UPDATE entries SET accessed = strftime('%s','now') WHERE worktree = ?1 AND content_hash = ?2 AND parser_version = ?3 AND kind = ?4",
                params![key.worktree.as_str(), key.content_hash, key.parser_version, key.kind],
            );
        }
        Ok(found)
    }

    fn put(&self, key: &CacheKey, value: &[u8]) -> Result<()> {
        let connection = self.connection()?;
        connection
            .execute(
                "INSERT OR REPLACE INTO entries (worktree, content_hash, parser_version, kind, value, accessed)
                 VALUES (?1, ?2, ?3, ?4, ?5, strftime('%s','now'))",
                params![key.worktree.as_str(), key.content_hash, key.parser_version, key.kind, value],
            )
            .map_err(|e| sql_error(&e))?;
        let count: i64 = connection
            .query_row("SELECT COUNT(*) FROM entries", [], |row| row.get(0))
            .map_err(|e| sql_error(&e))?;
        if count > MAX_ROWS {
            let excess = count - MAX_ROWS + MAX_ROWS / 10;
            connection
                .execute(
                    "DELETE FROM entries WHERE rowid IN (SELECT rowid FROM entries ORDER BY accessed ASC LIMIT ?1)",
                    params![excess],
                )
                .map_err(|e| sql_error(&e))?;
        }
        Ok(())
    }
}

/// A cache that remembers nothing; used when the database cannot be opened.
#[derive(Debug, Clone, Copy, Default)]
pub struct NoCache;

impl AnalysisCache for NoCache {
    fn get(&self, _key: &CacheKey) -> Result<Option<Vec<u8>>> {
        Ok(None)
    }

    fn put(&self, _key: &CacheKey, _value: &[u8]) -> Result<()> {
        Ok(())
    }
}
