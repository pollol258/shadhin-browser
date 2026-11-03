//! Content filter rule compilation and WebKitUserContentFilterStore management
//!
//! Provides rule compilation, WebKit JSON conversion, and filter lifecycle
//! management for ad and tracker blocking.

use gtk4::gio;
use gtk4::glib;
use rusqlite::{Connection, Result as SqlResult, params};
use serde::{Deserialize, Serialize};
use std::cell::RefCell;
use std::path::PathBuf;
use std::rc::Rc;
use tracing::{error, info, warn};
use webkit6::{UserContentFilter, UserContentFilterStore, UserContentManager};

/// Filter identifier constants
pub const FILTER_ID_ADS: &str = "easylist_ads";
pub const FILTER_ID_TRACKERS: &str = "easyprivacy_trackers";

/// WebKit Content Blocker Action
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ContentBlockerAction {
    #[serde(rename = "type")]
    pub action_type: String, // "block", "ignore-previous-rules", "block-cookies", "css-display-none"
    #[serde(skip_serializing_if = "Option::is_none")]
    pub selector: Option<String>,
}

/// WebKit Content Blocker Trigger
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ContentBlockerTrigger {
    #[serde(rename = "url-filter")]
    pub url_filter: String,
    #[serde(
        rename = "url-filter-is-case-sensitive",
        skip_serializing_if = "Option::is_none"
    )]
    pub url_filter_is_case_sensitive: Option<bool>,
    #[serde(rename = "resource-type", skip_serializing_if = "Option::is_none")]
    pub resource_type: Option<Vec<String>>,
    #[serde(rename = "load-type", skip_serializing_if = "Option::is_none")]
    pub load_type: Option<Vec<String>>,
    #[serde(rename = "if-domain", skip_serializing_if = "Option::is_none")]
    pub if_domain: Option<Vec<String>>,
    #[serde(rename = "unless-domain", skip_serializing_if = "Option::is_none")]
    pub unless_domain: Option<Vec<String>>,
}

/// Complete WebKit Content Blocker Rule representation
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ContentBlockerRule {
    pub trigger: ContentBlockerTrigger,
    pub action: ContentBlockerAction,
}

/// Converts a standard Adblock Plus (ABP) rule syntax into a WebKit JSON ContentBlockerRule
pub fn convert_abp_to_webkit_rule(raw_line: &str) -> Option<ContentBlockerRule> {
    let line = raw_line.trim();

    // Ignore empty lines and comments
    if line.is_empty() || line.starts_with('!') || line.starts_with('[') {
        return None;
    }

    // Ignore cosmetic / element hiding rules (##) as WebKit content filters are for network requests
    if line.contains("##") || line.contains("#@#") {
        return None;
    }

    let is_exception = line.starts_with("@@");
    let content = if is_exception { &line[2..] } else { line };

    // Separate options ($third-party, $script, etc.)
    let (pattern_part, options_part) = match content.split_once('$') {
        Some((p, opts)) => (p, Some(opts)),
        None => (content, None),
    };

    let mut load_type = None;
    let mut resource_type = None;

    if let Some(opts) = options_part {
        let opt_list: Vec<&str> = opts.split(',').map(|s| s.trim()).collect();
        for opt in opt_list {
            match opt {
                "third-party" | "~first-party" => {
                    load_type
                        .get_or_insert_with(Vec::new)
                        .push("third-party".to_string());
                }
                "first-party" | "~third-party" => {
                    load_type
                        .get_or_insert_with(Vec::new)
                        .push("first-party".to_string());
                }
                "script" => {
                    resource_type
                        .get_or_insert_with(Vec::new)
                        .push("script".to_string());
                }
                "image" => {
                    resource_type
                        .get_or_insert_with(Vec::new)
                        .push("image".to_string());
                }
                "stylesheet" => {
                    resource_type
                        .get_or_insert_with(Vec::new)
                        .push("style-sheet".to_string());
                }
                _ => {}
            }
        }
    }

    // Transform pattern to WebKit regex
    let regex_pattern = if let Some(stripped) = pattern_part.strip_prefix("||") {
        let domain = stripped.trim_end_matches('^');
        let escaped = regex_escape(domain);
        format!(r".*://([^/]+\.)?{escaped}/.*")
    } else if let Some(stripped) = pattern_part.strip_prefix('|') {
        let path = stripped.trim_end_matches('^');
        let escaped = regex_escape(path);
        format!(r"^{escaped}.*")
    } else if let Some(path) = pattern_part.strip_suffix('|') {
        let escaped = regex_escape(path);
        format!(r".*{escaped}$")
    } else {
        let trimmed = pattern_part.trim_matches('*');
        let escaped = regex_escape(trimmed);
        format!(r".*{escaped}.*")
    };

    let action_type = if is_exception {
        "ignore-previous-rules".to_string()
    } else {
        "block".to_string()
    };

    Some(ContentBlockerRule {
        trigger: ContentBlockerTrigger {
            url_filter: regex_pattern,
            url_filter_is_case_sensitive: Some(false),
            resource_type,
            load_type,
            if_domain: None,
            unless_domain: None,
        },
        action: ContentBlockerAction {
            action_type,
            selector: None,
        },
    })
}

/// Helper to escape regex special characters
fn regex_escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len() * 2);
    for c in s.chars() {
        match c {
            '.' | '^' | '$' | '*' | '+' | '?' | '(' | ')' | '[' | ']' | '{' | '}' | '|' | '\\' => {
                out.push('\\');
                out.push(c);
            }
            _ => out.push(c),
        }
    }
    out
}

/// Per-site shield exception entry
#[derive(Debug, Clone)]
pub struct SiteException {
    pub domain: String,
    pub shield_enabled: bool,
    pub ads_enabled: bool,
    pub trackers_enabled: bool,
    pub updated_at: i64,
}

/// Repository for per-site exceptions
pub struct SiteExceptionRepository<'a> {
    conn: &'a Connection,
}

impl<'a> SiteExceptionRepository<'a> {
    pub fn new(conn: &'a Connection) -> Self {
        Self { conn }
    }

    /// Retrieves per-site exception configuration for a host
    pub fn get_exception(&self, domain: &str) -> SqlResult<Option<SiteException>> {
        let mut stmt = self.conn.prepare(
            "SELECT domain, shield_enabled, ads_enabled, trackers_enabled, updated_at
             FROM site_exceptions WHERE domain = ?1;",
        )?;

        let mut rows = stmt.query(params![domain])?;
        if let Some(row) = rows.next()? {
            Ok(Some(SiteException {
                domain: row.get(0)?,
                shield_enabled: row.get::<_, i32>(1)? != 0,
                ads_enabled: row.get::<_, i32>(2)? != 0,
                trackers_enabled: row.get::<_, i32>(3)? != 0,
                updated_at: row.get(4)?,
            }))
        } else {
            Ok(None)
        }
    }

    /// Sets or updates a site shield exception
    pub fn set_exception(
        &self,
        domain: &str,
        shield_enabled: bool,
        ads_enabled: bool,
        trackers_enabled: bool,
        persistence_allowed: bool,
    ) -> SqlResult<()> {
        if !persistence_allowed {
            info!("Bypassing site exception persistence (private window active)");
            return Ok(());
        }

        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs() as i64;

        self.conn.execute(
            "INSERT INTO site_exceptions (domain, shield_enabled, ads_enabled, trackers_enabled, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5)
             ON CONFLICT(domain) DO UPDATE SET
                shield_enabled = excluded.shield_enabled,
                ads_enabled = excluded.ads_enabled,
                trackers_enabled = excluded.trackers_enabled,
                updated_at = excluded.updated_at;",
            params![
                domain,
                shield_enabled as i32,
                ads_enabled as i32,
                trackers_enabled as i32,
                now
            ],
        )?;

        Ok(())
    }

    /// Deletes exception for a site
    pub fn remove_exception(&self, domain: &str, persistence_allowed: bool) -> SqlResult<()> {
        if !persistence_allowed {
            return Ok(());
        }
        self.conn.execute(
            "DELETE FROM site_exceptions WHERE domain = ?1;",
            params![domain],
        )?;
        Ok(())
    }

    /// Retrieves all registered site exceptions
    pub fn get_all_exceptions(&self) -> SqlResult<Vec<SiteException>> {
        let mut stmt = self.conn.prepare(
            "SELECT domain, shield_enabled, ads_enabled, trackers_enabled, updated_at
             FROM site_exceptions ORDER BY domain ASC;",
        )?;

        let rows = stmt.query_map([], |row| {
            Ok(SiteException {
                domain: row.get(0)?,
                shield_enabled: row.get::<_, i32>(1)? != 0,
                ads_enabled: row.get::<_, i32>(2)? != 0,
                trackers_enabled: row.get::<_, i32>(3)? != 0,
                updated_at: row.get(4)?,
            })
        })?;

        let mut list = Vec::new();
        for r in rows {
            list.push(r?);
        }
        Ok(list)
    }
}

/// Central privacy filter manager coordinating compiled WebKitUserContentFilters
#[derive(Clone)]
pub struct ContentFilterManager {
    store: UserContentFilterStore,
    ads_filter: Rc<RefCell<Option<UserContentFilter>>>,
    trackers_filter: Rc<RefCell<Option<UserContentFilter>>>,
}

impl ContentFilterManager {
    /// Creates a new `ContentFilterManager` storing compiled bytecode in XDG data filters directory
    pub fn new() -> Self {
        let filters_dir = Self::default_filters_dir();
        let _ = std::fs::create_dir_all(&filters_dir);
        let store = UserContentFilterStore::new(filters_dir.to_string_lossy().as_ref());

        Self {
            store,
            ads_filter: Rc::new(RefCell::new(None)),
            trackers_filter: Rc::new(RefCell::new(None)),
        }
    }

    /// Standard XDG filters directory path (`~/.local/share/shadhin-browser/filters`)
    pub fn default_filters_dir() -> PathBuf {
        let base_dir = std::env::var("XDG_DATA_HOME")
            .map(PathBuf::from)
            .unwrap_or_else(|_| {
                let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
                PathBuf::from(home).join(".local").join("share")
            });
        base_dir.join("shadhin-browser").join("filters")
    }

    /// Loads or compiles default bundled filter snapshots
    pub fn initialize<F: Fn() + 'static>(&self, on_complete: F) {
        let on_complete = Rc::new(on_complete);

        // 1. Load or compile Ads filter
        let self_ads = self.clone();
        let oc_ads = on_complete.clone();
        self.load_or_compile_filter(
            FILTER_ID_ADS,
            "resources/filters/easylist_ads.json",
            move |res| {
                if let Ok(filter) = res {
                    info!("Loaded WebKit content filter for Ads ({FILTER_ID_ADS})");
                    *self_ads.ads_filter.borrow_mut() = Some(filter);
                } else {
                    warn!("Failed to load Ads filter: {:?}", res.err());
                }

                // 2. Load or compile Trackers filter
                let self_track = self_ads.clone();
                let oc_track = oc_ads.clone();
                self_ads.load_or_compile_filter(
                    FILTER_ID_TRACKERS,
                    "resources/filters/easyprivacy_trackers.json",
                    move |t_res| {
                        if let Ok(t_filter) = t_res {
                            info!(
                                "Loaded WebKit content filter for Trackers ({FILTER_ID_TRACKERS})"
                            );
                            *self_track.trackers_filter.borrow_mut() = Some(t_filter);
                        } else {
                            warn!("Failed to load Trackers filter: {:?}", t_res.err());
                        }
                        oc_track();
                    },
                );
            },
        );
    }

    /// Tries loading compiled filter from disk; falls back to compiling from JSON file
    fn load_or_compile_filter<F: FnOnce(Result<UserContentFilter, glib::Error>) + 'static>(
        &self,
        identifier: &str,
        source_json_path: &str,
        callback: F,
    ) {
        let store = self.store.clone();
        let store_save = self.store.clone();
        let id_str = identifier.to_string();
        let src_path = source_json_path.to_string();

        store.load(
            identifier,
            Option::<&gio::Cancellable>::None,
            move |load_res| match load_res {
                Ok(filter) => {
                    callback(Ok(filter));
                }
                Err(_) => {
                    info!(
                        "Compiled filter not found on disk. Compiling {id_str} from {src_path}..."
                    );
                    if let Ok(json_content) = std::fs::read_to_string(&src_path) {
                        let bytes = glib::Bytes::from_owned(json_content.into_bytes());
                        store_save.save(
                            &id_str,
                            &bytes,
                            Option::<&gio::Cancellable>::None,
                            callback,
                        );
                    } else {
                        error!("Failed to read filter source file at: {src_path}");
                    }
                }
            },
        );
    }

    /// Applies content filters to a `UserContentManager` according to active flags
    pub fn apply_filters(
        &self,
        manager: &UserContentManager,
        shield_master_enabled: bool,
        ads_enabled: bool,
        trackers_enabled: bool,
    ) {
        manager.remove_all_filters();

        if !shield_master_enabled {
            return;
        }

        if ads_enabled && let Some(ref filter) = *self.ads_filter.borrow() {
            manager.add_filter(filter);
        }

        if trackers_enabled && let Some(ref filter) = *self.trackers_filter.borrow() {
            manager.add_filter(filter);
        }
    }
}

impl Default for ContentFilterManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rusqlite::Connection;

    #[test]
    fn test_abp_converter_domain_anchor() {
        let abp = "||doubleclick.net^";
        let rule = convert_abp_to_webkit_rule(abp).expect("Should convert domain anchor");
        assert_eq!(rule.action.action_type, "block");
        assert_eq!(
            rule.trigger.url_filter,
            r".*://([^/]+\.)?doubleclick\.net/.*"
        );
        assert_eq!(rule.trigger.url_filter_is_case_sensitive, Some(false));
    }

    #[test]
    fn test_abp_converter_third_party_and_type() {
        let abp = "||tracker.com^$third-party,script";
        let rule = convert_abp_to_webkit_rule(abp).expect("Should convert third-party script");
        assert_eq!(rule.action.action_type, "block");
        assert_eq!(
            rule.trigger.load_type,
            Some(vec!["third-party".to_string()])
        );
        assert_eq!(rule.trigger.resource_type, Some(vec!["script".to_string()]));
    }

    #[test]
    fn test_abp_converter_exception_rule() {
        let abp = "@@||trusted-site.org^";
        let rule = convert_abp_to_webkit_rule(abp).expect("Should convert exception");
        assert_eq!(rule.action.action_type, "ignore-previous-rules");
    }

    #[test]
    fn test_abp_converter_skips_cosmetic_and_comments() {
        assert!(convert_abp_to_webkit_rule("! This is a comment").is_none());
        assert!(convert_abp_to_webkit_rule("###ad-banner").is_none());
        assert!(convert_abp_to_webkit_rule("example.com##.ad-class").is_none());
        assert!(convert_abp_to_webkit_rule("").is_none());
    }

    #[test]
    fn test_site_exceptions_repository() {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute(
            "CREATE TABLE site_exceptions (
                domain TEXT PRIMARY KEY,
                shield_enabled INTEGER NOT NULL DEFAULT 1,
                ads_enabled INTEGER NOT NULL DEFAULT 1,
                trackers_enabled INTEGER NOT NULL DEFAULT 1,
                updated_at INTEGER NOT NULL
            );",
            [],
        )
        .unwrap();

        let repo = SiteExceptionRepository::new(&conn);

        // Initially empty
        assert!(repo.get_exception("example.com").unwrap().is_none());

        // Set exception
        repo.set_exception("example.com", false, false, false, true)
            .unwrap();
        let ex = repo.get_exception("example.com").unwrap().unwrap();
        assert!(!ex.shield_enabled);
        assert!(!ex.ads_enabled);
        assert!(!ex.trackers_enabled);

        // Update exception
        repo.set_exception("example.com", true, true, false, true)
            .unwrap();
        let ex2 = repo.get_exception("example.com").unwrap().unwrap();
        assert!(ex2.shield_enabled);
        assert!(ex2.ads_enabled);
        assert!(!ex2.trackers_enabled);

        // Test persistence_allowed = false guard (private window)
        repo.set_exception("private-site.com", false, false, false, false)
            .unwrap();
        assert!(repo.get_exception("private-site.com").unwrap().is_none());
    }
}
