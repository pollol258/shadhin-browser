use rusqlite::{Connection, Result, params};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ShortcutItem {
    pub id: i64,
    pub title: String,
    pub url: String,
    pub position: i32,
}

pub struct ShortcutRepository<'a> {
    conn: &'a Connection,
}

impl<'a> ShortcutRepository<'a> {
    pub fn new(conn: &'a Connection) -> Self {
        Self { conn }
    }

    /// Fetches all new tab shortcut tiles ordered by position
    pub fn get_all(&self) -> Result<Vec<ShortcutItem>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, title, url, position FROM shortcuts ORDER BY position ASC, id ASC;",
        )?;

        let rows = stmt.query_map([], |row| {
            Ok(ShortcutItem {
                id: row.get(0)?,
                title: row.get(1)?,
                url: row.get(2)?,
                position: row.get(3)?,
            })
        })?;

        let mut shortcuts = Vec::new();
        for item in rows {
            shortcuts.push(item?);
        }
        Ok(shortcuts)
    }

    /// Validates url scheme against dangerous execution vectors
    fn validate_url(url: &str) -> Result<(), rusqlite::Error> {
        let trimmed = url.trim().to_lowercase();
        if trimmed.starts_with("javascript:")
            || trimmed.starts_with("data:")
            || trimmed.starts_with("file:")
        {
            return Err(rusqlite::Error::ToSqlConversionFailure(
                "Dangerous URI scheme forbidden in shortcuts".into(),
            ));
        }
        Ok(())
    }

    /// Adds a new shortcut tile at the end of the list
    pub fn add(&self, title: &str, url: &str) -> Result<i64> {
        Self::validate_url(url)?;

        let max_pos: i32 = self
            .conn
            .query_row(
                "SELECT COALESCE(MAX(position), -1) FROM shortcuts;",
                [],
                |r| r.get(0),
            )
            .unwrap_or(-1);

        let next_pos = max_pos + 1;

        self.conn.execute(
            "INSERT INTO shortcuts (title, url, position) VALUES (?1, ?2, ?3);",
            params![title.trim(), url.trim(), next_pos],
        )?;

        Ok(self.conn.last_insert_rowid())
    }

    /// Updates title and URL of an existing shortcut
    pub fn update(&self, id: i64, title: &str, url: &str) -> Result<()> {
        Self::validate_url(url)?;

        self.conn.execute(
            "UPDATE shortcuts SET title = ?1, url = ?2 WHERE id = ?3;",
            params![title.trim(), url.trim(), id],
        )?;

        Ok(())
    }

    /// Deletes a shortcut tile by ID
    pub fn delete(&self, id: i64) -> Result<()> {
        self.conn
            .execute("DELETE FROM shortcuts WHERE id = ?1;", params![id])?;
        Ok(())
    }

    /// Reorders shortcuts according to the provided list of IDs
    pub fn reorder(&self, ids: &[i64]) -> Result<()> {
        let tx = self.conn.unchecked_transaction()?;
        for (pos, &id) in ids.iter().enumerate() {
            tx.execute(
                "UPDATE shortcuts SET position = ?1 WHERE id = ?2;",
                params![pos as i32, id],
            )?;
        }
        tx.commit()?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::database::Database;

    #[test]
    fn test_shortcuts_crud_and_validation() {
        let db = Database::open_in_memory().unwrap();
        let repo = ShortcutRepository::new(db.conn());

        // Initial seeds exist (5 items)
        let initial = repo.get_all().unwrap();
        assert_eq!(initial.len(), 5);

        // Add item
        let id = repo.add("Rust Lang", "https://rust-lang.org").unwrap();
        assert!(id > 0);

        let all = repo.get_all().unwrap();
        assert_eq!(all.len(), 6);
        assert_eq!(all.last().unwrap().title, "Rust Lang");

        // Update item
        repo.update(id, "Rust Official", "https://www.rust-lang.org")
            .unwrap();
        let all2 = repo.get_all().unwrap();
        let updated = all2.iter().find(|s| s.id == id).unwrap();
        assert_eq!(updated.title, "Rust Official");

        // Reject dangerous scheme
        assert!(repo.add("Hax", "javascript:alert(1)").is_err());
        assert!(repo.update(id, "Hax", "data:text/html,bad").is_err());

        // Delete item
        repo.delete(id).unwrap();
        let all3 = repo.get_all().unwrap();
        assert_eq!(all3.len(), 5);
    }
}
