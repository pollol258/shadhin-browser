//! Verifiable privacy statistics repository and telemetry models
//!
//! Enforces honesty rules: only real observable events are counted.
//! Subresource blocking dropped silently by WebKitGTK's WebProcess is not fabricated.

use rusqlite::{Connection, Result as SqlResult, params};
use tracing::info;

/// Cumulative privacy statistics model
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct PrivacyStats {
    pub trackers_blocked: u64,
    pub ads_blocked: u64,
    pub fingerprints_blocked: u64,
    pub data_saved_bytes: u64,
}

impl PrivacyStats {
    /// Formatted data saved string ("—" because WebKit drops requests before Content-Length headers arrive)
    pub fn formatted_data_saved(&self) -> &'static str {
        "—"
    }

    /// Tooltip explanation for data saved metric
    pub fn data_saved_tooltip() -> &'static str {
        "Not measurable on this WebKitGTK version"
    }

    /// Tooltip explanation for subresource block counters in WebKitGTK 6.0
    pub fn subresource_blocking_tooltip() -> &'static str {
        "Not measurable on this WebKitGTK version"
    }
}

/// Active per-page privacy stats
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct PagePrivacyStats {
    pub ads_blocked: u32,
    pub trackers_blocked: u32,
    pub https_upgraded: bool,
}

/// SQLite repository for persistent privacy statistics
pub struct PrivacyStatsRepository<'a> {
    conn: &'a Connection,
}

impl<'a> PrivacyStatsRepository<'a> {
    pub fn new(conn: &'a Connection) -> Self {
        Self { conn }
    }

    /// Retrieves current cumulative stats from SQLite database
    pub fn get_stats(&self) -> SqlResult<PrivacyStats> {
        let mut stmt = self.conn.prepare(
            "SELECT trackers_blocked, ads_blocked, fingerprints_blocked, data_saved_bytes
             FROM privacy_stats WHERE id = 1;",
        )?;

        let mut rows = stmt.query([])?;
        if let Some(row) = rows.next()? {
            Ok(PrivacyStats {
                trackers_blocked: row.get::<_, i64>(0)? as u64,
                ads_blocked: row.get::<_, i64>(1)? as u64,
                fingerprints_blocked: row.get::<_, i64>(2)? as u64,
                data_saved_bytes: row.get::<_, i64>(3)? as u64,
            })
        } else {
            Ok(PrivacyStats::default())
        }
    }

    /// Records an observable ad block event (e.g. navigation policy ad redirection drop)
    pub fn record_ad_blocked(&self, count: u64, persistence_allowed: bool) -> SqlResult<()> {
        if !persistence_allowed || count == 0 {
            return Ok(());
        }

        self.conn.execute(
            "UPDATE privacy_stats
             SET ads_blocked = ads_blocked + ?1,
                 updated_at = unixepoch()
             WHERE id = 1;",
            params![count as i64],
        )?;

        Ok(())
    }

    /// Records an observable tracker block event
    pub fn record_tracker_blocked(&self, count: u64, persistence_allowed: bool) -> SqlResult<()> {
        if !persistence_allowed || count == 0 {
            return Ok(());
        }

        self.conn.execute(
            "UPDATE privacy_stats
             SET trackers_blocked = trackers_blocked + ?1,
                 updated_at = unixepoch()
             WHERE id = 1;",
            params![count as i64],
        )?;

        Ok(())
    }

    /// Resets all counters to 0 (user-triggered from Settings)
    pub fn reset_stats(&self, persistence_allowed: bool) -> SqlResult<()> {
        if !persistence_allowed {
            return Ok(());
        }

        info!("Resetting privacy statistics counters to 0");
        self.conn.execute(
            "UPDATE privacy_stats
             SET trackers_blocked = 0,
                 ads_blocked = 0,
                 fingerprints_blocked = 0,
                 data_saved_bytes = 0,
                 updated_at = unixepoch()
             WHERE id = 1;",
            [],
        )?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rusqlite::Connection;

    fn setup_test_db() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute(
            "CREATE TABLE privacy_stats (
                id INTEGER PRIMARY KEY CHECK (id = 1),
                trackers_blocked INTEGER NOT NULL DEFAULT 0,
                ads_blocked INTEGER NOT NULL DEFAULT 0,
                fingerprints_blocked INTEGER NOT NULL DEFAULT 0,
                data_saved_bytes INTEGER NOT NULL DEFAULT 0,
                updated_at INTEGER NOT NULL
            );",
            [],
        )
        .unwrap();

        conn.execute(
            "INSERT INTO privacy_stats (id, trackers_blocked, ads_blocked, fingerprints_blocked, data_saved_bytes, updated_at)
             VALUES (1, 0, 0, 0, 0, 1000);",
            [],
        )
        .unwrap();

        conn
    }

    #[test]
    fn test_privacy_stats_lifecycle() {
        let conn = setup_test_db();
        let repo = PrivacyStatsRepository::new(&conn);

        // Fresh install shows 0
        let initial = repo.get_stats().unwrap();
        assert_eq!(initial.ads_blocked, 0);
        assert_eq!(initial.trackers_blocked, 0);
        assert_eq!(initial.formatted_data_saved(), "—");

        // Record real events
        repo.record_ad_blocked(5, true).unwrap();
        repo.record_tracker_blocked(3, true).unwrap();

        let updated = repo.get_stats().unwrap();
        assert_eq!(updated.ads_blocked, 5);
        assert_eq!(updated.trackers_blocked, 3);

        // Private mode guard (persistence_allowed = false)
        repo.record_ad_blocked(10, false).unwrap();
        let private_check = repo.get_stats().unwrap();
        assert_eq!(private_check.ads_blocked, 5); // Unchanged

        // Reset
        repo.reset_stats(true).unwrap();
        let reset = repo.get_stats().unwrap();
        assert_eq!(reset.ads_blocked, 0);
        assert_eq!(reset.trackers_blocked, 0);
    }
}
