use crate::storage::bookmarks::BookmarkRepository;
use gtk4::glib;
use rusqlite::Result;
use std::collections::HashMap;

const MAX_IMPORT_SIZE_BYTES: usize = 10 * 1024 * 1024; // 10 MB limit

#[derive(Debug, PartialEq, Eq)]
pub struct ParsedBookmark {
    pub title: String,
    pub url: String,
    pub folder: Option<String>,
}

/// Validates that a URL does not use dangerous script execution vectors
pub fn is_safe_bookmark_url(url: &str) -> bool {
    let lower = url.trim().to_lowercase();
    if lower.is_empty() {
        return false;
    }
    // Disallow dangerous schemes
    if lower.starts_with("javascript:")
        || lower.starts_with("data:")
        || lower.starts_with("file:")
        || lower.starts_with("vbscript:")
    {
        return false;
    }
    lower.starts_with("http://")
        || lower.starts_with("https://")
        || lower.starts_with("ftp://")
        || lower.starts_with("about:")
        || lower.starts_with("shadhin:")
}

/// Parses standard Netscape Bookmark HTML format (Firefox / Chrome / Safari bookmark exports)
pub fn parse_netscape_html(content: &str) -> Result<Vec<ParsedBookmark>, String> {
    if content.len() > MAX_IMPORT_SIZE_BYTES {
        return Err("Import file exceeds 10 MB maximum limit".into());
    }

    let mut bookmarks = Vec::new();
    let mut current_folder: Option<String> = None;
    let mut folder_stack: Vec<String> = Vec::new();

    for line in content.lines() {
        let trimmed = line.trim();

        // Check for folder opening: <H3 ...>FolderName</H3>
        if let Some(h3_start) = trimmed.to_lowercase().find("<h3")
            && let Some(content_start) = trimmed[h3_start..].find('>')
        {
            let after_tag = &trimmed[h3_start + content_start + 1..];
            if let Some(h3_end) = after_tag.to_lowercase().find("</h3") {
                let folder_name = after_tag[..h3_end].trim().to_string();
                if !folder_name.is_empty() {
                    folder_stack.push(folder_name.clone());
                    current_folder = Some(folder_name);
                }
            }
        }

        // Check for folder closing: </DL>
        if trimmed.to_lowercase().contains("</dl>") {
            folder_stack.pop();
            current_folder = folder_stack.last().cloned();
        }

        // Check for bookmark: <A HREF="url" ...>Title</A>
        if let Some(a_start) = trimmed.to_lowercase().find("<a ") {
            let after_a = &trimmed[a_start + 3..];
            if let Some(href_pos) = after_a.to_lowercase().find("href=") {
                let after_href = &after_a[href_pos + 5..];
                let quote_char = after_href.chars().next();
                let url = match quote_char {
                    Some('"') => {
                        let inner = &after_href[1..];
                        inner.split('"').next().unwrap_or("").trim()
                    }
                    Some('\'') => {
                        let inner = &after_href[1..];
                        inner.split('\'').next().unwrap_or("").trim()
                    }
                    _ => after_href.split_whitespace().next().unwrap_or("").trim(),
                };

                if is_safe_bookmark_url(url) {
                    // Extract link text title
                    let title = if let Some(tag_close) = after_a.find('>') {
                        let text_part = &after_a[tag_close + 1..];
                        if let Some(end_a) = text_part.to_lowercase().find("</a") {
                            text_part[..end_a].trim()
                        } else {
                            url
                        }
                    } else {
                        url
                    };

                    bookmarks.push(ParsedBookmark {
                        title: if title.is_empty() {
                            url.to_string()
                        } else {
                            title.to_string()
                        },
                        url: url.to_string(),
                        folder: current_folder.clone(),
                    });
                }
            }
        }
    }

    Ok(bookmarks)
}

/// Exports bookmarks to standard Netscape Bookmark HTML format
pub fn export_netscape_html(repo: &BookmarkRepository) -> Result<String> {
    let mut html = String::new();
    html.push_str("<!DOCTYPE NETSCAPE-Bookmark-file-1>\n");
    html.push_str("<!-- This is an automatically generated file. It will be read and overwritten. Do Not Edit! -->\n");
    html.push_str("<META HTTP-EQUIV=\"Content-Type\" CONTENT=\"text/html; charset=UTF-8\">\n");
    html.push_str("<TITLE>Bookmarks</TITLE>\n");
    html.push_str("<H1>Bookmarks</H1>\n");
    html.push_str("<DL><p>\n");

    let folders = repo.get_folders()?;
    for folder in &folders {
        html.push_str(&format!(
            "    <DT><H3 ADD_DATE=\"0\" LAST_MODIFIED=\"0\">{}</H3>\n",
            glib::markup_escape_text(&folder.name)
        ));
        html.push_str("    <DL><p>\n");

        let items = repo.get_bookmarks(Some(folder.id))?;
        for item in &items {
            html.push_str(&format!(
                "        <DT><A HREF=\"{}\" ADD_DATE=\"{}\">{}</A>\n",
                glib::markup_escape_text(&item.url),
                item.created_at,
                glib::markup_escape_text(&item.title)
            ));
        }
        html.push_str("    </DL><p>\n");
    }

    // Direct root bookmarks
    let root_items = repo.get_bookmarks(None)?;
    for item in &root_items {
        html.push_str(&format!(
            "    <DT><A HREF=\"{}\" ADD_DATE=\"{}\">{}</A>\n",
            glib::markup_escape_text(&item.url),
            item.created_at,
            glib::markup_escape_text(&item.title)
        ));
    }

    html.push_str("</DL><p>\n");
    Ok(html)
}

/// JSON bookmark format structures
#[derive(serde::Serialize, serde::Deserialize, Debug)]
pub struct JsonBookmarkFile {
    pub version: u32,
    #[serde(default)]
    pub folders: Vec<JsonBookmarkFolder>,
    #[serde(default)]
    pub bookmarks: Vec<JsonBookmarkItem>,
}

#[derive(serde::Serialize, serde::Deserialize, Debug)]
pub struct JsonBookmarkFolder {
    pub name: String,
    #[serde(default)]
    pub bookmarks: Vec<JsonBookmarkItem>,
}

#[derive(serde::Serialize, serde::Deserialize, Debug)]
pub struct JsonBookmarkItem {
    pub title: String,
    pub url: String,
}

/// Parses bookmarks from JSON string
pub fn parse_json_bookmarks(content: &str) -> Result<Vec<ParsedBookmark>, String> {
    if content.len() > MAX_IMPORT_SIZE_BYTES {
        return Err("Import file exceeds 10 MB maximum limit".into());
    }

    let parsed: JsonBookmarkFile = serde_json::from_str(content)
        .map_err(|e| format!("Invalid JSON bookmark structure: {}", e))?;

    let mut bookmarks = Vec::new();

    for folder in parsed.folders {
        let folder_name = folder.name.trim();
        for item in folder.bookmarks {
            if is_safe_bookmark_url(&item.url) {
                bookmarks.push(ParsedBookmark {
                    title: if item.title.trim().is_empty() {
                        item.url.clone()
                    } else {
                        item.title
                    },
                    url: item.url,
                    folder: if folder_name.is_empty() {
                        None
                    } else {
                        Some(folder_name.to_string())
                    },
                });
            }
        }
    }

    for item in parsed.bookmarks {
        if is_safe_bookmark_url(&item.url) {
            bookmarks.push(ParsedBookmark {
                title: if item.title.trim().is_empty() {
                    item.url.clone()
                } else {
                    item.title
                },
                url: item.url,
                folder: None,
            });
        }
    }

    Ok(bookmarks)
}

/// Exports bookmarks to structured JSON
pub fn export_json_bookmarks(repo: &BookmarkRepository) -> Result<String> {
    let mut file = JsonBookmarkFile {
        version: 1,
        folders: Vec::new(),
        bookmarks: Vec::new(),
    };

    let folders = repo.get_folders()?;
    for folder in folders {
        let items = repo.get_bookmarks(Some(folder.id))?;
        file.folders.push(JsonBookmarkFolder {
            name: folder.name,
            bookmarks: items
                .into_iter()
                .map(|b| JsonBookmarkItem {
                    title: b.title,
                    url: b.url,
                })
                .collect(),
        });
    }

    let root_items = repo.get_bookmarks(None)?;
    for item in root_items {
        file.bookmarks.push(JsonBookmarkItem {
            title: item.title,
            url: item.url,
        });
    }

    serde_json::to_string_pretty(&file)
        .map_err(|e| rusqlite::Error::ToSqlConversionFailure(e.to_string().into()))
}

/// Imports parsed bookmarks into the database
pub fn import_parsed_bookmarks(
    repo: &BookmarkRepository,
    bookmarks: &[ParsedBookmark],
    persistence_allowed: bool,
) -> Result<usize> {
    if !persistence_allowed {
        return Ok(0);
    }

    let mut folder_cache: HashMap<String, i64> = HashMap::new();

    // Cache existing folders
    for f in repo.get_folders()? {
        folder_cache.insert(f.name.to_lowercase(), f.id);
    }

    let mut imported = 0;
    for b in bookmarks {
        let folder_id = if let Some(ref folder_name) = b.folder {
            let key = folder_name.trim().to_lowercase();
            if let Some(&id) = folder_cache.get(&key) {
                Some(id)
            } else {
                let new_folder = repo.create_folder(folder_name.trim(), None)?;
                let new_id = new_folder.id;
                folder_cache.insert(key, new_id);
                Some(new_id)
            }
        } else {
            None
        };

        if repo
            .add_bookmark(&b.title, &b.url, folder_id, None, persistence_allowed)
            .is_ok()
        {
            imported += 1;
        }
    }

    Ok(imported)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::database::Database;

    const NETSCAPE_FIXTURE: &str = r#"
<!DOCTYPE NETSCAPE-Bookmark-file-1>
<META HTTP-EQUIV="Content-Type" CONTENT="text/html; charset=UTF-8">
<TITLE>Bookmarks</TITLE>
<H1>Bookmarks</H1>
<DL><p>
    <DT><H3>Tech</H3>
    <DL><p>
        <DT><A HREF="https://rust-lang.org">Rust</A>
        <DT><A HREF="javascript:alert(1)">Exploit</A>
        <DT><A HREF="https://news.ycombinator.com">Hacker News</A>
    </DL><p>
    <DT><A HREF="https://duckduckgo.com">DuckDuckGo</A>
    <DT><A HREF="data:text/html,bad">Data URI</A>
</DL><p>
"#;

    const JSON_FIXTURE: &str = r#"
{
  "version": 1,
  "folders": [
    {
      "name": "Dev",
      "bookmarks": [
        { "title": "GitHub", "url": "https://github.com" },
        { "title": "Malicious", "url": "javascript:evil()" }
      ]
    }
  ],
  "bookmarks": [
    { "title": "বাংলা উইকিপিডিয়া", "url": "https://bn.wikipedia.org" }
  ]
}
"#;

    #[test]
    fn test_netscape_html_import_sanitization_and_export() {
        let parsed = parse_netscape_html(NETSCAPE_FIXTURE).unwrap();
        // javascript: and data: must be excluded
        assert_eq!(parsed.len(), 3);
        assert_eq!(parsed[0].title, "Rust");
        assert_eq!(parsed[0].url, "https://rust-lang.org");
        assert_eq!(parsed[0].folder.as_deref(), Some("Tech"));
        assert_eq!(parsed[1].title, "Hacker News");
        assert_eq!(parsed[2].title, "DuckDuckGo");
        assert_eq!(parsed[2].folder, None);

        let db = Database::open_in_memory().unwrap();
        let repo = BookmarkRepository::new(db.conn());
        let count = import_parsed_bookmarks(&repo, &parsed, true).unwrap();
        assert_eq!(count, 3);

        // Export round-trip
        let exported = export_netscape_html(&repo).unwrap();
        assert!(exported.contains("https://rust-lang.org"));
        assert!(exported.contains("<H3 ADD_DATE=\"0\" LAST_MODIFIED=\"0\">Tech</H3>"));
    }

    #[test]
    fn test_json_import_sanitization_and_export() {
        let parsed = parse_json_bookmarks(JSON_FIXTURE).unwrap();
        assert_eq!(parsed.len(), 2);
        assert_eq!(parsed[0].title, "GitHub");
        assert_eq!(parsed[0].folder.as_deref(), Some("Dev"));
        assert_eq!(parsed[1].title, "বাংলা উইকিপিডিয়া");
        assert_eq!(parsed[1].url, "https://bn.wikipedia.org");

        let db = Database::open_in_memory().unwrap();
        let repo = BookmarkRepository::new(db.conn());
        let count = import_parsed_bookmarks(&repo, &parsed, true).unwrap();
        assert_eq!(count, 2);

        let exported = export_json_bookmarks(&repo).unwrap();
        assert!(exported.contains("বাংলা উইকিপিডিয়া"));
        assert!(exported.contains("https://github.com"));
    }

    #[test]
    fn test_import_oversized_file_rejection() {
        let oversized = "a".repeat(MAX_IMPORT_SIZE_BYTES + 10);
        assert!(parse_netscape_html(&oversized).is_err());
        assert!(parse_json_bookmarks(&oversized).is_err());
    }
}
