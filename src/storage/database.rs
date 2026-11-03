//! SQLite database connection, PRAGMA configuration, and schema migrations
//!
//! Enforces secure file modes (0700 for directories, 0600 for database files)
//! and sets up WAL mode, foreign keys, and forward-only versioned migrations.

use rusqlite::{Connection, Result, params};
use std::fs::{DirBuilder, File, OpenOptions};
use std::os::unix::fs::{DirBuilderExt, PermissionsExt};
use std::path::{Path, PathBuf};
use tracing::info;

/// Current schema version of the database
pub const CURRENT_SCHEMA_VERSION: i32 = 3;

/// Database connection manager and migration runner
pub struct Database {
    conn: Connection,
    path: Option<PathBuf>,
}

impl Database {
    /// Opens or creates the SQLite database at the standard XDG data directory path
    pub fn open_default() -> Result<Self> {
        let db_path = Self::default_db_path();
        Self::open_at(&db_path)
    }

    /// Returns the standard XDG data path for Shadhin Browser (`~/.local/share/shadhin-browser/shadhin.db`)
    pub fn default_db_path() -> PathBuf {
        let base_dir = std::env::var("XDG_DATA_HOME")
            .map(PathBuf::from)
            .unwrap_or_else(|_| {
                let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
                PathBuf::from(home).join(".local").join("share")
            });
        base_dir.join("shadhin-browser").join("shadhin.db")
    }

    /// Opens or creates a database at a specific path with strict 0700/0600 permissions
    pub fn open_at(path: &Path) -> Result<Self> {
        if let Some(parent) = path.parent() {
            if !parent.exists() {
                let mut builder = DirBuilder::new();
                builder.recursive(true);
                builder.mode(0o700); // Only accessible by owner
                if let Err(e) = builder.create(parent) {
                    tracing::error!("Failed to create database directory {parent:?}: {e}");
                }
            } else if let Ok(mut perms) = std::fs::metadata(parent).map(|m| m.permissions()) {
                perms.set_mode(0o700);
                let _ = std::fs::set_permissions(parent, perms);
            }
        }

        // Ensure file exists with 0600 permissions
        if !path.exists() {
            if let Ok(file) = OpenOptions::new()
                .read(true)
                .write(true)
                .create(true)
                .truncate(false)
                .open(path)
            {
                let mut perms = file
                    .metadata()
                    .map(|m| m.permissions())
                    .unwrap_or_else(|_| {
                        let f = File::open(path).unwrap();
                        f.metadata().unwrap().permissions()
                    });
                perms.set_mode(0o600); // rw-------
                let _ = std::fs::set_permissions(path, perms);
            }
        } else if let Ok(mut perms) = std::fs::metadata(path).map(|m| m.permissions()) {
            perms.set_mode(0o600);
            let _ = std::fs::set_permissions(path, perms);
        }

        let conn = Connection::open(path)?;
        let mut db = Self {
            conn,
            path: Some(path.to_path_buf()),
        };

        db.apply_pragmas()?;
        db.run_migrations()?;

        Ok(db)
    }

    /// Opens an in-memory database for unit testing
    pub fn open_in_memory() -> Result<Self> {
        let conn = Connection::open_in_memory()?;
        let mut db = Self { conn, path: None };

        db.apply_pragmas()?;
        db.run_migrations()?;

        Ok(db)
    }

    /// Applies performance and integrity PRAGMAs
    fn apply_pragmas(&self) -> Result<()> {
        // WAL mode: provides concurrency where readers do not block writers and writers do not block readers
        self.conn.pragma_update(None, "journal_mode", "WAL")?;
        // Foreign keys: mandatory for cascade deletes across folders and tabs
        self.conn.pragma_update(None, "foreign_keys", "ON")?;
        // Normal synchronous: safe with WAL mode while eliminating redundant fsync stalls
        self.conn.pragma_update(None, "synchronous", "NORMAL")?;
        // WAL auto-checkpoint: keeps WAL size bounded (~4MB / 1000 pages)
        self.conn.pragma_update(None, "wal_autocheckpoint", 1000)?;
        // In-memory temp store: keeps temporary tables and indices in RAM
        self.conn.pragma_update(None, "temp_store", "MEMORY")?;
        // Busy timeout: waits up to 5 seconds if database is locked by concurrent transaction
        self.conn.busy_timeout(std::time::Duration::from_secs(5))?;
        Ok(())
    }

    /// Runs all forward-only schema migrations up to `CURRENT_SCHEMA_VERSION`
    fn run_migrations(&mut self) -> Result<()> {
        self.conn.execute(
            "CREATE TABLE IF NOT EXISTS schema_version (
                version INTEGER PRIMARY KEY,
                applied_at TEXT NOT NULL
            );",
            [],
        )?;

        let current_version: i32 = self
            .conn
            .query_row(
                "SELECT COALESCE(MAX(version), 0) FROM schema_version;",
                [],
                |row| row.get(0),
            )
            .unwrap_or(0);

        if current_version < 1 {
            self.migrate_to_v1()?;
        }
        if current_version < 2 {
            self.migrate_to_v2()?;
        }
        if current_version < 3 {
            self.migrate_to_v3()?;
        }

        Ok(())
    }

    /// Migration V1: Initial database schema
    fn migrate_to_v1(&mut self) -> Result<()> {
        info!("Applying database migration V1 (initial schema)");
        let tx = self.conn.transaction()?;

        // 1. Bookmarks and Folders
        tx.execute(
            "CREATE TABLE IF NOT EXISTS bookmark_folders (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                parent_id INTEGER REFERENCES bookmark_folders(id) ON DELETE CASCADE,
                name TEXT NOT NULL,
                position INTEGER NOT NULL DEFAULT 0
            );",
            [],
        )?;

        tx.execute(
            "CREATE TABLE IF NOT EXISTS bookmarks (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                folder_id INTEGER REFERENCES bookmark_folders(id) ON DELETE SET NULL,
                title TEXT NOT NULL,
                url TEXT NOT NULL,
                favicon BLOB,
                position INTEGER NOT NULL DEFAULT 0,
                created_at INTEGER NOT NULL
            );",
            [],
        )?;
        tx.execute(
            "CREATE INDEX IF NOT EXISTS idx_bookmarks_url ON bookmarks(url);",
            [],
        )?;
        tx.execute(
            "CREATE INDEX IF NOT EXISTS idx_bookmarks_folder ON bookmarks(folder_id);",
            [],
        )?;

        // 2. History
        tx.execute(
            "CREATE TABLE IF NOT EXISTS history (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                url TEXT UNIQUE NOT NULL,
                title TEXT NOT NULL,
                visit_count INTEGER NOT NULL DEFAULT 1,
                first_visit INTEGER NOT NULL,
                last_visit INTEGER NOT NULL
            );",
            [],
        )?;
        tx.execute(
            "CREATE INDEX IF NOT EXISTS idx_history_last_visit ON history(last_visit DESC);",
            [],
        )?;
        tx.execute(
            "CREATE INDEX IF NOT EXISTS idx_history_url ON history(url);",
            [],
        )?;

        // 3. Downloads
        tx.execute(
            "CREATE TABLE IF NOT EXISTS downloads (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                url TEXT NOT NULL,
                file_path TEXT NOT NULL,
                file_name TEXT NOT NULL,
                mime TEXT,
                total_bytes INTEGER NOT NULL DEFAULT 0,
                received_bytes INTEGER NOT NULL DEFAULT 0,
                state TEXT NOT NULL,
                started_at INTEGER NOT NULL,
                finished_at INTEGER,
                error TEXT
            );",
            [],
        )?;
        tx.execute(
            "CREATE INDEX IF NOT EXISTS idx_downloads_started ON downloads(started_at DESC);",
            [],
        )?;

        // 4. New Tab Shortcuts
        tx.execute(
            "CREATE TABLE IF NOT EXISTS shortcuts (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                title TEXT NOT NULL,
                url TEXT NOT NULL,
                position INTEGER NOT NULL DEFAULT 0
            );",
            [],
        )?;

        // Seed initial shortcuts if table is empty
        let shortcut_count: i64 =
            tx.query_row("SELECT COUNT(*) FROM shortcuts;", [], |r| r.get(0))?;
        if shortcut_count == 0 {
            let initial_shortcuts = [
                ("GitHub", "https://github.com", 0),
                ("YouTube", "https://youtube.com", 1),
                ("Wikipedia", "https://wikipedia.org", 2),
                ("Reddit", "https://reddit.com", 3),
                ("Linux", "https://kernel.org", 4),
            ];
            for (title, url, pos) in initial_shortcuts {
                tx.execute(
                    "INSERT INTO shortcuts (title, url, position) VALUES (?1, ?2, ?3);",
                    params![title, url, pos],
                )?;
            }
        }

        // 5. Search Engines
        tx.execute(
            "CREATE TABLE IF NOT EXISTS search_engines (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                name TEXT NOT NULL,
                keyword TEXT UNIQUE NOT NULL,
                url_template TEXT NOT NULL,
                is_default INTEGER NOT NULL DEFAULT 0,
                is_builtin INTEGER NOT NULL DEFAULT 0
            );",
            [],
        )?;

        // Seed built-in search engines if empty
        let engine_count: i64 =
            tx.query_row("SELECT COUNT(*) FROM search_engines;", [], |r| r.get(0))?;
        if engine_count == 0 {
            let builtins = [
                ("DuckDuckGo", "d", "https://duckduckgo.com/?q=%s", 1, 1),
                (
                    "Brave Search",
                    "b",
                    "https://search.brave.com/search?q=%s",
                    0,
                    1,
                ),
                (
                    "Startpage",
                    "s",
                    "https://www.startpage.com/sp/search?query=%s",
                    0,
                    1,
                ),
                ("Google", "g", "https://www.google.com/search?q=%s", 0, 1),
                ("Bing", "bing", "https://www.bing.com/search?q=%s", 0, 1),
                (
                    "Wikipedia",
                    "w",
                    "https://en.wikipedia.org/wiki/Special:Search?search=%s",
                    0,
                    1,
                ),
            ];
            for (name, kw, tmpl, is_def, is_builtin) in builtins {
                tx.execute(
                    "INSERT INTO search_engines (name, keyword, url_template, is_default, is_builtin)
                     VALUES (?1, ?2, ?3, ?4, ?5);",
                    params![name, kw, tmpl, is_def, is_builtin],
                )?;
            }
        }

        // 6. Workspaces
        tx.execute(
            "CREATE TABLE IF NOT EXISTS workspaces (
                id TEXT PRIMARY KEY,
                name TEXT NOT NULL,
                color TEXT NOT NULL,
                position INTEGER NOT NULL DEFAULT 0,
                data_dir_id TEXT NOT NULL,
                isolate_data INTEGER NOT NULL DEFAULT 1,
                created_at INTEGER NOT NULL
            );",
            [],
        )?;

        // Seed default workspaces (Personal, Work, Project) if empty
        let ws_count: i64 = tx.query_row("SELECT COUNT(*) FROM workspaces;", [], |r| r.get(0))?;
        if ws_count == 0 {
            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs() as i64;

            let defaults = [
                ("default", "Personal", "personal", 0, "ws_personal", 1),
                ("work", "Work", "work", 1, "ws_work", 1),
                ("project", "Project", "project", 2, "ws_project", 1),
            ];
            for (id, name, color, pos, dir, iso) in defaults {
                tx.execute(
                    "INSERT INTO workspaces (id, name, color, position, data_dir_id, isolate_data, created_at)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7);",
                    params![id, name, color, pos, dir, iso, now],
                )?;
            }
        }

        // 7. Session Tabs
        tx.execute(
            "CREATE TABLE IF NOT EXISTS session_tabs (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                workspace_id TEXT NOT NULL REFERENCES workspaces(id) ON DELETE CASCADE,
                url TEXT NOT NULL,
                title TEXT NOT NULL,
                position INTEGER NOT NULL DEFAULT 0,
                is_active INTEGER NOT NULL DEFAULT 0
            );",
            [],
        )?;

        // Record schema version
        let now_str = chrono_or_simple_timestamp();
        tx.execute(
            "INSERT INTO schema_version (version, applied_at) VALUES (?1, ?2);",
            params![1, now_str],
        )?;

        tx.commit()?;
        Ok(())
    }

    /// Migration V2: Privacy & Security schema (site exceptions, site permissions, stats)
    fn migrate_to_v2(&mut self) -> Result<()> {
        info!("Applying database migration V2 (privacy and security tables)");
        let tx = self.conn.transaction()?;

        // 1. Site Exceptions (per-site ad & tracker shield toggle)
        tx.execute(
            "CREATE TABLE IF NOT EXISTS site_exceptions (
                domain TEXT PRIMARY KEY,
                shield_enabled INTEGER NOT NULL DEFAULT 1,
                ads_enabled INTEGER NOT NULL DEFAULT 1,
                trackers_enabled INTEGER NOT NULL DEFAULT 1,
                updated_at INTEGER NOT NULL
            );",
            [],
        )?;

        // 2. Site Permissions (camera, microphone, geolocation, notifications, etc.)
        tx.execute(
            "CREATE TABLE IF NOT EXISTS site_permissions (
                origin TEXT NOT NULL,
                permission_type TEXT NOT NULL,
                state TEXT NOT NULL,
                updated_at INTEGER NOT NULL,
                PRIMARY KEY (origin, permission_type)
            );",
            [],
        )?;

        // 3. Privacy Statistics
        tx.execute(
            "CREATE TABLE IF NOT EXISTS privacy_stats (
                id INTEGER PRIMARY KEY CHECK (id = 1),
                trackers_blocked INTEGER NOT NULL DEFAULT 0,
                ads_blocked INTEGER NOT NULL DEFAULT 0,
                fingerprints_blocked INTEGER NOT NULL DEFAULT 0,
                data_saved_bytes INTEGER NOT NULL DEFAULT 0,
                updated_at INTEGER NOT NULL
            );",
            [],
        )?;

        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs() as i64;

        tx.execute(
            "INSERT OR IGNORE INTO privacy_stats (id, trackers_blocked, ads_blocked, fingerprints_blocked, data_saved_bytes, updated_at)
             VALUES (1, 0, 0, 0, 0, ?1);",
            params![now],
        )?;

        let now_str = chrono_or_simple_timestamp();
        tx.execute(
            "INSERT INTO schema_version (version, applied_at) VALUES (?1, ?2);",
            params![2, now_str],
        )?;

        tx.commit()?;
        Ok(())
    }

    /// Migration V3: Performance indexes for history autocomplete, session tabs, and bookmarks
    fn migrate_to_v3(&mut self) -> Result<()> {
        info!("Applying database migration V3 (performance optimization indexes)");
        let tx = self.conn.transaction()?;

        // Autocomplete composite index: optimizes ORDER BY visit_count DESC, last_visit DESC
        tx.execute(
            "CREATE INDEX IF NOT EXISTS idx_history_autocomplete ON history(visit_count DESC, last_visit DESC);",
            [],
        )?;

        // Session tabs position index
        tx.execute(
            "CREATE INDEX IF NOT EXISTS idx_session_tabs_ws_pos ON session_tabs(workspace_id, position);",
            [],
        )?;

        let now_str = chrono_or_simple_timestamp();
        tx.execute(
            "INSERT INTO schema_version (version, applied_at) VALUES (?1, ?2);",
            params![3, now_str],
        )?;

        tx.commit()?;
        Ok(())
    }

    /// Accessor for underlying SQLite connection
    pub fn conn(&self) -> &Connection {
        &self.conn
    }

    /// Mutable accessor for underlying SQLite connection
    pub fn conn_mut(&mut self) -> &mut Connection {
        &mut self.conn
    }

    /// Path to database file if backed by disk
    pub fn path(&self) -> Option<&Path> {
        self.path.as_deref()
    }

    /// Runs PRAGMA optimize to update query planner statistics and optimize indexes
    pub fn optimize(&self) -> Result<()> {
        self.conn.execute_batch("PRAGMA optimize;")?;
        Ok(())
    }
}

/// Simple ISO-8601 UTC timestamp helper without requiring heavy extra crates
fn chrono_or_simple_timestamp() -> String {
    let now = std::time::SystemTime::now();
    let duration = now
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default();
    format!("{}.{:03}Z", duration.as_secs(), duration.subsec_millis())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_in_memory_database_creation_and_migrations() {
        let db = Database::open_in_memory().expect("In-memory DB should open cleanly");
        let version: i32 = db
            .conn()
            .query_row("SELECT MAX(version) FROM schema_version;", [], |r| r.get(0))
            .expect("Schema version query should succeed");
        assert_eq!(version, CURRENT_SCHEMA_VERSION);

        // Verify seeded search engines
        let count: i64 = db
            .conn()
            .query_row("SELECT COUNT(*) FROM search_engines;", [], |r| r.get(0))
            .expect("Search engines query should succeed");
        assert!(count >= 6);

        // Verify seeded shortcuts
        let shortcut_count: i64 = db
            .conn()
            .query_row("SELECT COUNT(*) FROM shortcuts;", [], |r| r.get(0))
            .expect("Shortcuts query should succeed");
        assert_eq!(shortcut_count, 5);

        // Verify seeded workspaces
        let ws_count: i64 = db
            .conn()
            .query_row("SELECT COUNT(*) FROM workspaces;", [], |r| r.get(0))
            .expect("Workspaces query should succeed");
        assert_eq!(ws_count, 3);
    }

    #[test]
    fn test_disk_database_permissions() {
        let temp_dir = std::env::temp_dir().join(format!("shadhin_test_{}", std::process::id()));
        let db_file = temp_dir.join("test_shadhin.db");

        let db = Database::open_at(&db_file).expect("Opening disk DB should succeed");
        assert!(db_file.exists());

        // Check directory permissions mode 0700
        let dir_meta = std::fs::metadata(&temp_dir).unwrap();
        let dir_mode = dir_meta.permissions().mode() & 0o777;
        assert_eq!(dir_mode, 0o700);

        // Check file permissions mode 0600
        let file_meta = std::fs::metadata(&db_file).unwrap();
        let file_mode = file_meta.permissions().mode() & 0o777;
        assert_eq!(file_mode, 0o600);

        // Clean up
        drop(db);
        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_open_default_database() {
        // Test default_db_path format
        let def_path = Database::default_db_path();
        assert!(def_path.ends_with("shadhin-browser/shadhin.db"));

        // Test creation with strict permissions in temp test path
        let test_dir =
            std::env::temp_dir().join(format!("shadhin_test_default_{}", std::process::id()));
        let db_path = test_dir.join("shadhin-browser").join("shadhin.db");
        let db = Database::open_at(&db_path).expect("Database creation should succeed");
        let path = db.path().unwrap();
        assert!(path.exists());

        // Check directory permissions mode 0700
        let dir = path.parent().unwrap();
        let dir_meta = std::fs::metadata(dir).unwrap();
        assert_eq!(dir_meta.permissions().mode() & 0o777, 0o700);

        // Check file permissions mode 0600
        let file_meta = std::fs::metadata(path).unwrap();
        assert_eq!(file_meta.permissions().mode() & 0o777, 0o600);

        drop(db);
        let _ = std::fs::remove_dir_all(&test_dir);
    }
}
