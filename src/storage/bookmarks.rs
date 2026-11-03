//! Bookmarks and bookmark folders repository
//!
//! Provides relational persistence for user bookmarks, folder trees,
//! search queries, and URL bookmark detection.

use rusqlite::{Connection, Result, params};
use std::time::SystemTime;

/// Represents a user bookmark folder
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BookmarkFolder {
    pub id: i64,
    pub parent_id: Option<i64>,
    pub name: String,
    pub position: i32,
}

/// Represents an individual bookmark entry
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BookmarkItem {
    pub id: i64,
    pub folder_id: Option<i64>,
    pub title: String,
    pub url: String,
    pub favicon: Option<Vec<u8>>,
    pub position: i32,
    pub created_at: i64,
}

pub struct BookmarkRepository<'a> {
    conn: &'a Connection,
}

impl<'a> BookmarkRepository<'a> {
    /// Constructs a repository borrowing an open database connection
    pub fn new(conn: &'a Connection) -> Self {
        Self { conn }
    }

    /// Fetches all bookmark folders ordered by position
    pub fn get_folders(&self) -> Result<Vec<BookmarkFolder>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, parent_id, name, position FROM bookmark_folders ORDER BY position ASC, name ASC;",
        )?;
        let rows = stmt.query_map([], |row| {
            Ok(BookmarkFolder {
                id: row.get(0)?,
                parent_id: row.get(1)?,
                name: row.get(2)?,
                position: row.get(3)?,
            })
        })?;

        let mut folders = Vec::new();
        for f in rows {
            folders.push(f?);
        }
        Ok(folders)
    }

    /// Creates a new bookmark folder
    pub fn create_folder(&self, name: &str, parent_id: Option<i64>) -> Result<BookmarkFolder> {
        let max_pos: i32 = self
            .conn
            .query_row(
                "SELECT COALESCE(MAX(position), -1) FROM bookmark_folders WHERE parent_id IS ?1;",
                params![parent_id],
                |r| r.get(0),
            )
            .unwrap_or(-1);

        let next_pos = max_pos + 1;
        self.conn.execute(
            "INSERT INTO bookmark_folders (name, parent_id, position) VALUES (?1, ?2, ?3);",
            params![name.trim(), parent_id, next_pos],
        )?;

        let id = self.conn.last_insert_rowid();
        Ok(BookmarkFolder {
            id,
            parent_id,
            name: name.trim().to_string(),
            position: next_pos,
        })
    }

    /// Renames a bookmark folder
    pub fn rename_folder(&self, id: i64, new_name: &str) -> Result<()> {
        self.conn.execute(
            "UPDATE bookmark_folders SET name = ?1 WHERE id = ?2;",
            params![new_name.trim(), id],
        )?;
        Ok(())
    }

    /// Deletes a bookmark folder and its subfolders/bookmarks
    pub fn delete_folder(&self, id: i64) -> Result<()> {
        self.conn
            .execute("DELETE FROM bookmark_folders WHERE id = ?1;", params![id])?;
        Ok(())
    }

    /// Checks if a folder contains any bookmarks or child folders
    pub fn is_folder_non_empty(&self, id: i64) -> Result<bool> {
        let b_count: i64 = self.conn.query_row(
            "SELECT COUNT(*) FROM bookmarks WHERE folder_id = ?1;",
            params![id],
            |r| r.get(0),
        )?;
        if b_count > 0 {
            return Ok(true);
        }
        let f_count: i64 = self.conn.query_row(
            "SELECT COUNT(*) FROM bookmark_folders WHERE parent_id = ?1;",
            params![id],
            |r| r.get(0),
        )?;
        Ok(f_count > 0)
    }

    /// Adds a bookmark. Validates URL and respects the private mode `persistence_allowed` flag.
    pub fn add_bookmark(
        &self,
        title: &str,
        url: &str,
        folder_id: Option<i64>,
        favicon: Option<&[u8]>,
        persistence_allowed: bool,
    ) -> Result<Option<BookmarkItem>> {
        if !persistence_allowed {
            return Ok(None);
        }

        let trimmed_url = url.trim();
        // Disallow dangerous schemes
        if trimmed_url.starts_with("javascript:")
            || trimmed_url.starts_with("data:")
            || trimmed_url.starts_with("file:")
            || trimmed_url.is_empty()
        {
            return Ok(None);
        }

        let now = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs() as i64;

        let max_pos: i32 = self
            .conn
            .query_row(
                "SELECT COALESCE(MAX(position), -1) FROM bookmarks WHERE folder_id IS ?1;",
                params![folder_id],
                |r| r.get(0),
            )
            .unwrap_or(-1);

        let next_pos = max_pos + 1;
        let display_title = if title.trim().is_empty() {
            trimmed_url
        } else {
            title.trim()
        };

        self.conn.execute(
            "INSERT INTO bookmarks (folder_id, title, url, favicon, position, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6);",
            params![
                folder_id,
                display_title,
                trimmed_url,
                favicon,
                next_pos,
                now
            ],
        )?;

        let id = self.conn.last_insert_rowid();
        Ok(Some(BookmarkItem {
            id,
            folder_id,
            title: display_title.to_string(),
            url: trimmed_url.to_string(),
            favicon: favicon.map(|b| b.to_vec()),
            position: next_pos,
            created_at: now,
        }))
    }

    /// Removes a bookmark by its primary ID
    pub fn remove_bookmark(&self, id: i64) -> Result<()> {
        self.conn
            .execute("DELETE FROM bookmarks WHERE id = ?1;", params![id])?;
        Ok(())
    }

    /// Removes a bookmark by its URL
    pub fn remove_bookmark_by_url(&self, url: &str) -> Result<()> {
        self.conn
            .execute("DELETE FROM bookmarks WHERE url = ?1;", params![url.trim()])?;
        Ok(())
    }

    /// Checks if a given URL is currently bookmarked
    pub fn is_bookmarked(&self, url: &str) -> Result<bool> {
        let count: i64 = self.conn.query_row(
            "SELECT COUNT(*) FROM bookmarks WHERE url = ?1;",
            params![url.trim()],
            |r| r.get(0),
        )?;
        Ok(count > 0)
    }

    /// Retrieves bookmark details by URL
    pub fn get_bookmark_by_url(&self, url: &str) -> Result<Option<BookmarkItem>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, folder_id, title, url, favicon, position, created_at
             FROM bookmarks WHERE url = ?1 LIMIT 1;",
        )?;
        let mut rows = stmt.query(params![url.trim()])?;
        if let Some(row) = rows.next()? {
            Ok(Some(BookmarkItem {
                id: row.get(0)?,
                folder_id: row.get(1)?,
                title: row.get(2)?,
                url: row.get(3)?,
                favicon: row.get(4)?,
                position: row.get(5)?,
                created_at: row.get(6)?,
            }))
        } else {
            Ok(None)
        }
    }

    /// Updates title, URL, or parent folder of an existing bookmark
    pub fn update_bookmark(
        &self,
        id: i64,
        title: &str,
        url: &str,
        folder_id: Option<i64>,
    ) -> Result<()> {
        self.conn.execute(
            "UPDATE bookmarks SET title = ?1, url = ?2, folder_id = ?3 WHERE id = ?4;",
            params![title.trim(), url.trim(), folder_id, id],
        )?;
        Ok(())
    }

    /// Fetches all bookmarks inside a specific folder (or root if None)
    pub fn get_bookmarks(&self, folder_id: Option<i64>) -> Result<Vec<BookmarkItem>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, folder_id, title, url, favicon, position, created_at
             FROM bookmarks WHERE folder_id IS ?1 ORDER BY position ASC, id ASC;",
        )?;
        let rows = stmt.query_map(params![folder_id], |row| {
            Ok(BookmarkItem {
                id: row.get(0)?,
                folder_id: row.get(1)?,
                title: row.get(2)?,
                url: row.get(3)?,
                favicon: row.get(4)?,
                position: row.get(5)?,
                created_at: row.get(6)?,
            })
        })?;

        let mut list = Vec::new();
        for b in rows {
            list.push(b?);
        }
        Ok(list)
    }

    /// Fetches all bookmarks across all folders
    pub fn get_all_bookmarks(&self) -> Result<Vec<BookmarkItem>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, folder_id, title, url, favicon, position, created_at
             FROM bookmarks ORDER BY title ASC;",
        )?;
        let rows = stmt.query_map([], |row| {
            Ok(BookmarkItem {
                id: row.get(0)?,
                folder_id: row.get(1)?,
                title: row.get(2)?,
                url: row.get(3)?,
                favicon: row.get(4)?,
                position: row.get(5)?,
                created_at: row.get(6)?,
            })
        })?;

        let mut list = Vec::new();
        for b in rows {
            list.push(b?);
        }
        Ok(list)
    }

    /// Searches bookmarks by title or URL substring
    pub fn search_bookmarks(&self, query: &str) -> Result<Vec<BookmarkItem>> {
        let pattern = format!("%{}%", query.trim());
        let mut stmt = self.conn.prepare(
            "SELECT id, folder_id, title, url, favicon, position, created_at
             FROM bookmarks WHERE title LIKE ?1 OR url LIKE ?1 ORDER BY title ASC LIMIT 50;",
        )?;
        let rows = stmt.query_map(params![pattern], |row| {
            Ok(BookmarkItem {
                id: row.get(0)?,
                folder_id: row.get(1)?,
                title: row.get(2)?,
                url: row.get(3)?,
                favicon: row.get(4)?,
                position: row.get(5)?,
                created_at: row.get(6)?,
            })
        })?;

        let mut list = Vec::new();
        for b in rows {
            list.push(b?);
        }
        Ok(list)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::database::Database;

    #[test]
    fn test_bookmark_and_folder_lifecycle() {
        let db = Database::open_in_memory().unwrap();
        let repo = BookmarkRepository::new(db.conn());

        // 1. Create folders
        let f1 = repo.create_folder("Tech", None).unwrap();
        let f2 = repo.create_folder("Rust", Some(f1.id)).unwrap();

        assert_eq!(f1.name, "Tech");
        assert_eq!(f2.parent_id, Some(f1.id));

        // 2. Add bookmarks
        let b1 = repo
            .add_bookmark(
                "Rust Language",
                "https://www.rust-lang.org",
                Some(f2.id),
                None,
                true,
            )
            .unwrap()
            .unwrap();

        assert_eq!(b1.title, "Rust Language");
        assert!(repo.is_bookmarked("https://www.rust-lang.org").unwrap());
        assert!(!repo.is_bookmarked("https://nonexistent.org").unwrap());

        // 3. Search
        let results = repo.search_bookmarks("rust").unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].id, b1.id);

        // 4. Update
        repo.update_bookmark(
            b1.id,
            "Rust Official Site",
            "https://www.rust-lang.org",
            Some(f1.id),
        )
        .unwrap();
        let updated = repo
            .get_bookmark_by_url("https://www.rust-lang.org")
            .unwrap()
            .unwrap();
        assert_eq!(updated.title, "Rust Official Site");
        assert_eq!(updated.folder_id, Some(f1.id));

        // 5. Delete bookmark
        repo.remove_bookmark(b1.id).unwrap();
        assert!(!repo.is_bookmarked("https://www.rust-lang.org").unwrap());
    }

    #[test]
    fn test_private_mode_persistence_flag() {
        let db = Database::open_in_memory().unwrap();
        let repo = BookmarkRepository::new(db.conn());

        // With persistence_allowed = false, nothing should be saved
        let res = repo
            .add_bookmark("Private Site", "https://private.org", None, None, false)
            .unwrap();
        assert!(res.is_none());
        assert!(!repo.is_bookmarked("https://private.org").unwrap());
    }

    #[test]
    fn test_disallow_dangerous_schemes() {
        let db = Database::open_in_memory().unwrap();
        let repo = BookmarkRepository::new(db.conn());

        assert!(
            repo.add_bookmark("XSS", "javascript:alert(1)", None, None, true)
                .unwrap()
                .is_none()
        );
        assert!(
            repo.add_bookmark("Phish", "data:text/html,bad", None, None, true)
                .unwrap()
                .is_none()
        );
        assert!(
            repo.add_bookmark("Local", "file:///etc/passwd", None, None, true)
                .unwrap()
                .is_none()
        );
    }
}
