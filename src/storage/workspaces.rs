use rusqlite::{Connection, Result, params};
use std::time::SystemTime;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkspaceItem {
    pub id: String,
    pub name: String,
    pub color: String,
    pub position: i32,
    pub data_dir_id: String,
    pub isolate_data: bool,
    pub created_at: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SessionTabItem {
    pub id: i64,
    pub workspace_id: String,
    pub url: String,
    pub title: String,
    pub position: i32,
    pub is_active: bool,
}

pub struct WorkspaceRepository<'a> {
    conn: &'a Connection,
}

impl<'a> WorkspaceRepository<'a> {
    pub fn new(conn: &'a Connection) -> Self {
        Self { conn }
    }

    /// Fetches all workspaces ordered by position
    pub fn get_all(&self) -> Result<Vec<WorkspaceItem>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, name, color, position, data_dir_id, isolate_data, created_at
             FROM workspaces
             ORDER BY position ASC, created_at ASC;",
        )?;

        let rows = stmt.query_map([], |row| {
            Ok(WorkspaceItem {
                id: row.get(0)?,
                name: row.get(1)?,
                color: row.get(2)?,
                position: row.get(3)?,
                data_dir_id: row.get(4)?,
                isolate_data: row.get::<_, i32>(5)? != 0,
                created_at: row.get(6)?,
            })
        })?;

        let mut list = Vec::new();
        for item in rows {
            list.push(item?);
        }
        Ok(list)
    }

    /// Gets a single workspace by ID
    pub fn get_by_id(&self, id: &str) -> Result<Option<WorkspaceItem>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, name, color, position, data_dir_id, isolate_data, created_at
             FROM workspaces
             WHERE id = ?1;",
        )?;

        let mut rows = stmt.query_map(params![id], |row| {
            Ok(WorkspaceItem {
                id: row.get(0)?,
                name: row.get(1)?,
                color: row.get(2)?,
                position: row.get(3)?,
                data_dir_id: row.get(4)?,
                isolate_data: row.get::<_, i32>(5)? != 0,
                created_at: row.get(6)?,
            })
        })?;

        if let Some(res) = rows.next() {
            Ok(Some(res?))
        } else {
            Ok(None)
        }
    }

    /// Creates a new workspace
    pub fn create(&self, id: &str, name: &str, color: &str) -> Result<()> {
        let max_pos: i32 = self
            .conn
            .query_row(
                "SELECT COALESCE(MAX(position), -1) FROM workspaces;",
                [],
                |r| r.get(0),
            )
            .unwrap_or(-1);

        let next_pos = max_pos + 1;
        let data_dir_id = format!("ws_{}", id);
        let now = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs() as i64;

        self.conn.execute(
            "INSERT INTO workspaces (id, name, color, position, data_dir_id, isolate_data, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, 1, ?6);",
            params![id, name, color, next_pos, data_dir_id, now],
        )?;

        Ok(())
    }

    /// Updates name and color of an existing workspace
    pub fn update(&self, id: &str, name: &str, color: &str) -> Result<()> {
        self.conn.execute(
            "UPDATE workspaces SET name = ?1, color = ?2 WHERE id = ?3;",
            params![name, color, id],
        )?;
        Ok(())
    }

    /// Deletes a workspace. The default 'default' workspace cannot be deleted.
    pub fn delete(&self, id: &str) -> Result<()> {
        if id == "default" {
            return Err(rusqlite::Error::ToSqlConversionFailure(
                "Cannot delete default workspace".into(),
            ));
        }

        self.conn
            .execute("DELETE FROM workspaces WHERE id = ?1;", params![id])?;
        Ok(())
    }

    /// Saves session tabs for a workspace, replacing previous session tabs
    pub fn save_session_tabs(
        &self,
        workspace_id: &str,
        tabs: &[(&str, &str, bool)], // (url, title, is_active)
    ) -> Result<()> {
        let tx = self.conn.unchecked_transaction()?;

        tx.execute(
            "DELETE FROM session_tabs WHERE workspace_id = ?1;",
            params![workspace_id],
        )?;

        for (pos, (url, title, is_active)) in tabs.iter().enumerate() {
            tx.execute(
                "INSERT INTO session_tabs (workspace_id, url, title, position, is_active)
                 VALUES (?1, ?2, ?3, ?4, ?5);",
                params![
                    workspace_id,
                    *url,
                    *title,
                    pos as i32,
                    if *is_active { 1 } else { 0 }
                ],
            )?;
        }

        tx.commit()?;
        Ok(())
    }

    /// Fetches saved session tabs for a workspace
    pub fn get_session_tabs(&self, workspace_id: &str) -> Result<Vec<SessionTabItem>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, workspace_id, url, title, position, is_active
             FROM session_tabs
             WHERE workspace_id = ?1
             ORDER BY position ASC;",
        )?;

        let rows = stmt.query_map(params![workspace_id], |row| {
            Ok(SessionTabItem {
                id: row.get(0)?,
                workspace_id: row.get(1)?,
                url: row.get(2)?,
                title: row.get(3)?,
                position: row.get(4)?,
                is_active: row.get::<_, i32>(5)? != 0,
            })
        })?;

        let mut list = Vec::new();
        for item in rows {
            list.push(item?);
        }
        Ok(list)
    }

    /// Clears all session tabs across all workspaces
    pub fn clear_all_session_tabs(&self) -> Result<()> {
        self.conn.execute("DELETE FROM session_tabs;", [])?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::database::Database;

    #[test]
    fn test_workspaces_crud_and_tabs_session() {
        let db = Database::open_in_memory().unwrap();
        let repo = WorkspaceRepository::new(db.conn());

        // Default seeded workspaces: personal (default), work, project
        let list = repo.get_all().unwrap();
        assert_eq!(list.len(), 3);
        assert_eq!(list[0].id, "default");
        assert_eq!(list[0].name, "Personal");

        // Create new workspace
        repo.create("gaming", "Gaming", "orange").unwrap();
        let ws = repo.get_by_id("gaming").unwrap().unwrap();
        assert_eq!(ws.name, "Gaming");
        assert_eq!(ws.data_dir_id, "ws_gaming");

        // Save session tabs
        let tabs = [
            ("https://example.com/1", "Tab One", true),
            ("https://example.com/2", "Tab Two", false),
        ];
        repo.save_session_tabs("gaming", &tabs).unwrap();

        let loaded_tabs = repo.get_session_tabs("gaming").unwrap();
        assert_eq!(loaded_tabs.len(), 2);
        assert_eq!(loaded_tabs[0].url, "https://example.com/1");
        assert!(loaded_tabs[0].is_active);
        assert_eq!(loaded_tabs[1].url, "https://example.com/2");
        assert!(!loaded_tabs[1].is_active);

        // Cannot delete default
        assert!(repo.delete("default").is_err());

        // Can delete custom workspace (cascade deletes session tabs)
        repo.delete("gaming").unwrap();
        let empty_tabs = repo.get_session_tabs("gaming").unwrap();
        assert!(empty_tabs.is_empty());
    }
}
