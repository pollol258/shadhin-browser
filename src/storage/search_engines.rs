use gtk4::glib;
use rusqlite::{Connection, Result, params};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SearchEngineItem {
    pub id: i64,
    pub name: String,
    pub keyword: String,
    pub url_template: String,
    pub is_default: bool,
    pub is_builtin: bool,
}

pub struct SearchEngineRepository<'a> {
    conn: &'a Connection,
}

impl<'a> SearchEngineRepository<'a> {
    pub fn new(conn: &'a Connection) -> Self {
        Self { conn }
    }

    /// Fetches all configured search engines
    pub fn get_all(&self) -> Result<Vec<SearchEngineItem>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, name, keyword, url_template, is_default, is_builtin
             FROM search_engines
             ORDER BY is_default DESC, is_builtin DESC, name ASC;",
        )?;

        let rows = stmt.query_map([], |row| {
            Ok(SearchEngineItem {
                id: row.get(0)?,
                name: row.get(1)?,
                keyword: row.get(2)?,
                url_template: row.get(3)?,
                is_default: row.get::<_, i32>(4)? != 0,
                is_builtin: row.get::<_, i32>(5)? != 0,
            })
        })?;

        let mut list = Vec::new();
        for item in rows {
            list.push(item?);
        }
        Ok(list)
    }

    /// Gets the current default search engine (fallback to DuckDuckGo if none selected)
    pub fn get_default(&self) -> Result<SearchEngineItem> {
        let engine = self.conn.query_row(
            "SELECT id, name, keyword, url_template, is_default, is_builtin
             FROM search_engines
             WHERE is_default = 1
             LIMIT 1;",
            [],
            |row| {
                Ok(SearchEngineItem {
                    id: row.get(0)?,
                    name: row.get(1)?,
                    keyword: row.get(2)?,
                    url_template: row.get(3)?,
                    is_default: row.get::<_, i32>(4)? != 0,
                    is_builtin: row.get::<_, i32>(5)? != 0,
                })
            },
        );

        match engine {
            Ok(item) => Ok(item),
            Err(_) => {
                // Fallback to first available
                self.conn.query_row(
                    "SELECT id, name, keyword, url_template, is_default, is_builtin
                     FROM search_engines
                     LIMIT 1;",
                    [],
                    |row| {
                        Ok(SearchEngineItem {
                            id: row.get(0)?,
                            name: row.get(1)?,
                            keyword: row.get(2)?,
                            url_template: row.get(3)?,
                            is_default: true,
                            is_builtin: row.get::<_, i32>(5)? != 0,
                        })
                    },
                )
            }
        }
    }

    /// Sets the specified engine as default, clearing any previous default
    pub fn set_default(&self, id: i64) -> Result<()> {
        let tx = self.conn.unchecked_transaction()?;
        tx.execute("UPDATE search_engines SET is_default = 0;", [])?;
        tx.execute(
            "UPDATE search_engines SET is_default = 1 WHERE id = ?1;",
            params![id],
        )?;
        tx.commit()?;
        Ok(())
    }

    /// Finds search engine matching a prefix keyword (e.g. "w" for Wikipedia)
    pub fn get_by_keyword(&self, keyword: &str) -> Result<Option<SearchEngineItem>> {
        let kw_lower = keyword.trim().to_lowercase();
        let mut stmt = self.conn.prepare(
            "SELECT id, name, keyword, url_template, is_default, is_builtin
             FROM search_engines
             WHERE LOWER(keyword) = ?1
             LIMIT 1;",
        )?;

        let mut rows = stmt.query_map(params![kw_lower], |row| {
            Ok(SearchEngineItem {
                id: row.get(0)?,
                name: row.get(1)?,
                keyword: row.get(2)?,
                url_template: row.get(3)?,
                is_default: row.get::<_, i32>(4)? != 0,
                is_builtin: row.get::<_, i32>(5)? != 0,
            })
        })?;

        if let Some(res) = rows.next() {
            Ok(Some(res?))
        } else {
            Ok(None)
        }
    }

    /// Adds a new custom search engine
    pub fn add(&self, name: &str, keyword: &str, url_template: &str) -> Result<i64> {
        let name = name.trim();
        let keyword = keyword.trim().to_lowercase();
        let url_template = url_template.trim();

        if name.is_empty() || keyword.is_empty() {
            return Err(rusqlite::Error::ToSqlConversionFailure(
                "Engine name and keyword cannot be empty".into(),
            ));
        }

        if !url_template.contains("%s") {
            return Err(rusqlite::Error::ToSqlConversionFailure(
                "Search engine URL template must contain '%s'".into(),
            ));
        }

        if !url_template.starts_with("https://") && !url_template.starts_with("http://") {
            return Err(rusqlite::Error::ToSqlConversionFailure(
                "Search engine URL must use HTTP or HTTPS protocol".into(),
            ));
        }

        self.conn.execute(
            "INSERT INTO search_engines (name, keyword, url_template, is_default, is_builtin)
             VALUES (?1, ?2, ?3, 0, 0);",
            params![name, keyword, url_template],
        )?;

        Ok(self.conn.last_insert_rowid())
    }

    /// Deletes a custom search engine. Built-in engines cannot be deleted.
    pub fn delete(&self, id: i64) -> Result<()> {
        let is_builtin: i32 = self.conn.query_row(
            "SELECT is_builtin FROM search_engines WHERE id = ?1;",
            params![id],
            |r| r.get(0),
        )?;

        if is_builtin != 0 {
            return Err(rusqlite::Error::ToSqlConversionFailure(
                "Built-in search engines cannot be deleted".into(),
            ));
        }

        // If deleting default, fall back to DuckDuckGo or first available
        let is_default: i32 = self.conn.query_row(
            "SELECT is_default FROM search_engines WHERE id = ?1;",
            params![id],
            |r| r.get(0),
        )?;

        self.conn
            .execute("DELETE FROM search_engines WHERE id = ?1;", params![id])?;

        if is_default != 0 {
            let _ = self.conn.execute(
                "UPDATE search_engines SET is_default = 1 WHERE keyword = 'd';",
                [],
            );
        }

        Ok(())
    }
}

/// Formats a search query string into the engine's URL template
pub fn build_search_url(url_template: &str, query: &str) -> String {
    let encoded = glib::Uri::escape_string(query, None, true);
    url_template.replace("%s", &encoded)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::database::Database;

    #[test]
    fn test_search_engines_crud_and_lookup() {
        let db = Database::open_in_memory().unwrap();
        let repo = SearchEngineRepository::new(db.conn());

        // Default seeds exist (6 items)
        let engines = repo.get_all().unwrap();
        assert_eq!(engines.len(), 6);

        // Check default engine
        let def = repo.get_default().unwrap();
        assert_eq!(def.keyword, "d");
        assert_eq!(def.name, "DuckDuckGo");

        // Keyword lookup
        let wiki = repo
            .get_by_keyword("w")
            .unwrap()
            .expect("Wikipedia keyword found");
        assert_eq!(wiki.name, "Wikipedia");

        let non_existent = repo.get_by_keyword("xyz").unwrap();
        assert!(non_existent.is_none());

        // Add custom engine
        let custom_id = repo
            .add("Ecosia", "e", "https://www.ecosia.org/search?q=%s")
            .unwrap();
        assert!(custom_id > 0);

        let ecosia = repo.get_by_keyword("e").unwrap().unwrap();
        assert_eq!(ecosia.name, "Ecosia");

        // Change default
        repo.set_default(custom_id).unwrap();
        let new_def = repo.get_default().unwrap();
        assert_eq!(new_def.id, custom_id);
        assert_eq!(new_def.name, "Ecosia");

        // Built-ins cannot be deleted
        assert!(repo.delete(def.id).is_err());

        // Custom can be deleted
        repo.delete(custom_id).unwrap();
        let after_delete = repo.get_by_keyword("e").unwrap();
        assert!(after_delete.is_none());
    }

    #[test]
    fn test_build_search_url() {
        let template = "https://duckduckgo.com/?q=%s";
        let url = build_search_url(template, "rust programming language");
        assert_eq!(
            url,
            "https://duckduckgo.com/?q=rust%20programming%20language"
        );

        // Bangla characters query
        let url_bangla = build_search_url(template, "বাংলাদেশ");
        println!("Bangla search URL: {}", url_bangla);
        assert!(url_bangla.starts_with("https://duckduckgo.com/?q="));
    }
}
