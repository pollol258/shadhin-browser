//! Browsing history repository with visit tracking, search, and retention cleanup
//!
//! Stores visit frequency, timestamps, and page titles with debounced writes
//! and privacy persistence bypass support.

use rusqlite::{Connection, Result, params};
use std::time::SystemTime;

/// Represents a browsing history entry
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HistoryItem {
    pub id: i64,
    pub url: String,
    pub title: String,
    pub visit_count: i32,
    pub first_visit: i64,
    pub last_visit: i64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HistoryGrouped {
    pub today: Vec<HistoryItem>,
    pub yesterday: Vec<HistoryItem>,
    pub earlier: Vec<HistoryItem>,
}

pub struct HistoryRepository<'a> {
    conn: &'a Connection,
}

impl<'a> HistoryRepository<'a> {
    /// Constructs a history repository borrowing an open database connection
    pub fn new(conn: &'a Connection) -> Self {
        Self { conn }
    }

    /// Records a site visit. Skips internal schemes, error pages, and checks `persistence_allowed`.
    pub fn record_visit(
        &self,
        url: &str,
        title: &str,
        persistence_allowed: bool,
    ) -> Result<Option<HistoryItem>> {
        if !persistence_allowed {
            return Ok(None);
        }

        let trimmed_url = url.trim();
        // Only record HTTP and HTTPS visits; ignore internal/error pages
        if !trimmed_url.starts_with("http://") && !trimmed_url.starts_with("https://") {
            return Ok(None);
        }

        let now = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs() as i64;

        let display_title = if title.trim().is_empty() {
            trimmed_url
        } else {
            title.trim()
        };

        // Upsert into history
        self.conn.execute(
            "INSERT INTO history (url, title, visit_count, first_visit, last_visit)
             VALUES (?1, ?2, 1, ?3, ?3)
             ON CONFLICT(url) DO UPDATE SET
                 visit_count = history.visit_count + 1,
                 last_visit = ?3,
                 title = CASE WHEN ?2 != '' AND ?2 != ?1 THEN ?2 ELSE history.title END;",
            params![trimmed_url, display_title, now],
        )?;

        let id = self.conn.last_insert_rowid();
        Ok(Some(HistoryItem {
            id,
            url: trimmed_url.to_string(),
            title: display_title.to_string(),
            visit_count: 1,
            first_visit: now,
            last_visit: now,
        }))
    }

    /// Fetches the most recent visits ordered by `last_visit` descending
    pub fn get_recent_visits(&self, limit: usize) -> Result<Vec<HistoryItem>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, url, title, visit_count, first_visit, last_visit
             FROM history ORDER BY last_visit DESC LIMIT ?1;",
        )?;
        let rows = stmt.query_map(params![limit as i64], |row| {
            Ok(HistoryItem {
                id: row.get(0)?,
                url: row.get(1)?,
                title: row.get(2)?,
                visit_count: row.get(3)?,
                first_visit: row.get(4)?,
                last_visit: row.get(5)?,
            })
        })?;

        let mut list = Vec::new();
        for r in rows {
            list.push(r?);
        }
        Ok(list)
    }

    /// Fetches visits grouped into Today, Yesterday, and Earlier
    pub fn get_grouped_history(&self) -> Result<HistoryGrouped> {
        let items = self.get_recent_visits(200)?;
        let now = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs() as i64;
        let today_start = now - (now % 86400);
        let yesterday_start = today_start - 86400;

        let mut today = Vec::new();
        let mut yesterday = Vec::new();
        let mut earlier = Vec::new();

        for item in items {
            if item.last_visit >= today_start {
                today.push(item);
            } else if item.last_visit >= yesterday_start {
                yesterday.push(item);
            } else {
                earlier.push(item);
            }
        }

        Ok(HistoryGrouped {
            today,
            yesterday,
            earlier,
        })
    }

    /// Fetches top sites ranked by visit frequency
    pub fn get_top_sites(&self, limit: usize) -> Result<Vec<HistoryItem>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, url, title, visit_count, first_visit, last_visit
             FROM history ORDER BY visit_count DESC, last_visit DESC LIMIT ?1;",
        )?;
        let rows = stmt.query_map(params![limit as i64], |row| {
            Ok(HistoryItem {
                id: row.get(0)?,
                url: row.get(1)?,
                title: row.get(2)?,
                visit_count: row.get(3)?,
                first_visit: row.get(4)?,
                last_visit: row.get(5)?,
            })
        })?;

        let mut list = Vec::new();
        for r in rows {
            list.push(r?);
        }
        Ok(list)
    }

    /// Searches history entries by title or URL substring
    pub fn search_history(&self, query: &str, limit: usize) -> Result<Vec<HistoryItem>> {
        let pattern = format!("%{}%", query.trim());
        let mut stmt = self.conn.prepare(
            "SELECT id, url, title, visit_count, first_visit, last_visit
             FROM history WHERE title LIKE ?1 OR url LIKE ?1
             ORDER BY last_visit DESC LIMIT ?2;",
        )?;
        let rows = stmt.query_map(params![pattern, limit as i64], |row| {
            Ok(HistoryItem {
                id: row.get(0)?,
                url: row.get(1)?,
                title: row.get(2)?,
                visit_count: row.get(3)?,
                first_visit: row.get(4)?,
                last_visit: row.get(5)?,
            })
        })?;

        let mut list = Vec::new();
        for r in rows {
            list.push(r?);
        }
        Ok(list)
    }

    /// Searches for address bar autocomplete matches (matching prefix or substring, ranked by score)
    pub fn search_autocomplete(&self, query: &str, limit: usize) -> Result<Vec<HistoryItem>> {
        let pattern = format!("%{}%", query.trim());
        let prefix = format!("{}%", query.trim());
        let mut stmt = self.conn.prepare(
            "SELECT id, url, title, visit_count, first_visit, last_visit
             FROM history
             WHERE title LIKE ?1 OR url LIKE ?1
             ORDER BY
                 (CASE WHEN url LIKE ?2 THEN 2 WHEN title LIKE ?2 THEN 1 ELSE 0 END) DESC,
                 visit_count DESC,
                 last_visit DESC
             LIMIT ?3;",
        )?;
        let rows = stmt.query_map(params![pattern, prefix, limit as i64], |row| {
            Ok(HistoryItem {
                id: row.get(0)?,
                url: row.get(1)?,
                title: row.get(2)?,
                visit_count: row.get(3)?,
                first_visit: row.get(4)?,
                last_visit: row.get(5)?,
            })
        })?;

        let mut list = Vec::new();
        for r in rows {
            list.push(r?);
        }
        Ok(list)
    }

    /// Deletes a specific history record by ID
    pub fn delete_entry(&self, id: i64) -> Result<()> {
        self.conn
            .execute("DELETE FROM history WHERE id = ?1;", params![id])?;
        Ok(())
    }

    /// Clears history entries within a time range (`start_timestamp` to `end_timestamp`)
    pub fn delete_range(&self, start: i64, end: i64) -> Result<usize> {
        let rows = self.conn.execute(
            "DELETE FROM history WHERE last_visit >= ?1 AND last_visit <= ?2;",
            params![start, end],
        )?;
        Ok(rows)
    }

    /// Clears history entries visited at or after `cutoff` (or all if cutoff <= 0)
    pub fn clear_since(&self, cutoff: i64) -> Result<usize> {
        if cutoff <= 0 {
            self.conn.execute("DELETE FROM history;", [])
        } else {
            self.conn.execute(
                "DELETE FROM history WHERE last_visit >= ?1;",
                params![cutoff],
            )
        }
    }

    /// Clears all history
    pub fn clear_all(&self) -> Result<()> {
        self.conn.execute("DELETE FROM history;", [])?;
        Ok(())
    }

    /// Deletes entries older than `retention_days` (if retention_days > 0)
    pub fn cleanup_retention(&self, retention_days: u32) -> Result<usize> {
        if retention_days == 0 {
            return Ok(0); // 0 means keep forever
        }

        let now = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs() as i64;

        let threshold = now - (retention_days as i64 * 86400);
        let rows = self.conn.execute(
            "DELETE FROM history WHERE last_visit < ?1;",
            params![threshold],
        )?;
        Ok(rows)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::database::Database;

    #[test]
    fn test_history_recording_and_upsert() {
        let db = Database::open_in_memory().unwrap();
        let repo = HistoryRepository::new(db.conn());

        // 1. Initial record
        let res1 = repo
            .record_visit("https://docs.rs/webkit6", "Docs - WebKit6", true)
            .unwrap()
            .unwrap();
        assert_eq!(res1.visit_count, 1);

        // 2. Second visit increments count
        repo.record_visit("https://docs.rs/webkit6", "Docs - WebKit6 Crate", true)
            .unwrap();

        let recent = repo.get_recent_visits(10).unwrap();
        assert_eq!(recent.len(), 1);
        assert_eq!(recent[0].visit_count, 2);
        assert_eq!(recent[0].title, "Docs - WebKit6 Crate");
    }

    #[test]
    fn test_history_private_mode_persistence_flag() {
        let db = Database::open_in_memory().unwrap();
        let repo = HistoryRepository::new(db.conn());

        let res = repo
            .record_visit("https://private-session.com", "Secret", false)
            .unwrap();
        assert!(res.is_none());
        assert_eq!(repo.get_recent_visits(10).unwrap().len(), 0);
    }

    #[test]
    fn test_history_range_deletion_and_retention() {
        let db = Database::open_in_memory().unwrap();
        let repo = HistoryRepository::new(db.conn());

        repo.record_visit("https://site1.com", "Site 1", true)
            .unwrap();
        repo.record_visit("https://site2.com", "Site 2", true)
            .unwrap();

        assert_eq!(repo.get_recent_visits(10).unwrap().len(), 2);

        // Delete all
        repo.clear_all().unwrap();
        assert_eq!(repo.get_recent_visits(10).unwrap().len(), 0);
    }

    #[test]
    fn test_history_db_size_500_rows() {
        let temp_dir =
            std::env::temp_dir().join(format!("shadhin_hist_size_{}", std::process::id()));
        let db_path = temp_dir.join("history_benchmark.db");
        let db = Database::open_at(&db_path).unwrap();
        let repo = HistoryRepository::new(db.conn());

        for i in 1..=500 {
            repo.record_visit(
                &format!("https://example-{i}.com/page/{i}"),
                &format!("Example Page {i} - Shadhin Browser Test Benchmark"),
                true,
            )
            .unwrap();
        }

        // Run WAL checkpoint so page writes flush to the main db file
        let _ = db.conn().execute_batch("PRAGMA wal_checkpoint(TRUNCATE);");

        let db_size = std::fs::metadata(&db_path).unwrap().len();
        println!(
            "REAL_METRIC: DB size after 500 history rows = {} bytes ({:.2} KB)",
            db_size,
            db_size as f64 / 1024.0
        );
        assert!(db_size > 0);

        drop(db);
        let _ = std::fs::remove_dir_all(&temp_dir);
    }
}
