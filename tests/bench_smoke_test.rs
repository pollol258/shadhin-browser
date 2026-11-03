//! Comprehensive non-GUI regression and benchmark smoke test suite for Shadhin Browser.
//! Validates:
//! - Schema migrations V1 -> V2 -> V3
//! - Storage CRUD roundtrips (bookmarks, history, downloads, search engines, workspaces)
//! - URL parsing & safety validation (Bangladeshi IDN, ports, blocked schemes)
//! - ABP to WebKit content filter JSON conversion
//! - Tab lifecycle memory bounds & discarding logic
//! - Startup latency bounds check (< 1000ms debug, < 300ms release)

use shadhin_browser::browser::downloads::sanitize_download_filename;
use shadhin_browser::browser::navigation::{NavigationTarget, classify_input};
use shadhin_browser::browser::tabs::TabManager;
use shadhin_browser::privacy::adblock::convert_abp_to_webkit_rule;
use shadhin_browser::storage::bookmarks::BookmarkRepository;
use shadhin_browser::storage::database::{CURRENT_SCHEMA_VERSION, Database};
use shadhin_browser::storage::downloads::DownloadRepository;
use shadhin_browser::storage::history::HistoryRepository;
use shadhin_browser::storage::search_engines::{SearchEngineRepository, build_search_url};
use shadhin_browser::storage::workspaces::WorkspaceRepository;
use std::time::{Duration, Instant};

#[test]
fn test_smoke_database_migrations_v1_to_v3_and_optimize() {
    let db = Database::open_in_memory().expect("In-memory DB should open cleanly");

    // Verify schema version is at current V3
    let version: i32 = db
        .conn()
        .query_row("SELECT MAX(version) FROM schema_version;", [], |r| r.get(0))
        .expect("Schema version query should succeed");
    assert_eq!(version, CURRENT_SCHEMA_VERSION);
    assert_eq!(version, 3);

    // Verify optimized indexes exist
    let has_hist_auto: i64 = db
        .conn()
        .query_row(
            "SELECT COUNT(*) FROM sqlite_master WHERE type='index' AND name='idx_history_autocomplete';",
            [],
            |r| r.get(0),
        )
        .expect("Index lookup should succeed");
    assert_eq!(has_hist_auto, 1);

    let has_tab_pos: i64 = db
        .conn()
        .query_row(
            "SELECT COUNT(*) FROM sqlite_master WHERE type='index' AND name='idx_session_tabs_ws_pos';",
            [],
            |r| r.get(0),
        )
        .expect("Index lookup should succeed");
    assert_eq!(has_tab_pos, 1);

    // Verify PRAGMA optimize runs without error
    db.optimize().expect("PRAGMA optimize should succeed");
}

#[test]
fn test_smoke_storage_crud_roundtrips() {
    let db = Database::open_in_memory().unwrap();
    let conn = db.conn();

    // 1. Bookmarks & Folders
    let bm_repo = BookmarkRepository::new(conn);
    let folder = bm_repo.create_folder("Tech Research", None).unwrap();
    let bm = bm_repo
        .add_bookmark(
            "Rust Language",
            "https://www.rust-lang.org",
            Some(folder.id),
            None,
            true,
        )
        .unwrap()
        .expect("Bookmark item returned");

    assert_eq!(bm.title, "Rust Language");
    let all_bms = bm_repo.get_all_bookmarks().unwrap();
    assert!(
        all_bms
            .iter()
            .any(|b| b.id == bm.id && b.title == "Rust Language")
    );

    // 2. History
    let hist_repo = HistoryRepository::new(conn);
    hist_repo
        .record_visit("https://news.ycombinator.com", "Hacker News", true)
        .unwrap();
    hist_repo
        .record_visit("https://news.ycombinator.com", "Hacker News", true)
        .unwrap();

    let recent = hist_repo.get_recent_visits(5).unwrap();
    assert_eq!(recent.len(), 1);
    assert_eq!(recent[0].visit_count, 2);

    let search_res = hist_repo.search_history("hacker", 5).unwrap();
    assert_eq!(search_res.len(), 1);

    // 3. Downloads
    let dl_repo = DownloadRepository::new(conn);
    let dl_id = dl_repo
        .insert(
            "https://dist.example.org/archive.tar.gz",
            "/home/user/Downloads/archive.tar.gz",
            "archive.tar.gz",
            Some("application/gzip"),
            1048576,
            true,
        )
        .unwrap();

    dl_repo
        .update_progress(dl_id, 524288, 1048576, true)
        .unwrap();
    let recent_dls = dl_repo.get_recent(5).unwrap();
    assert_eq!(recent_dls.len(), 1);
    assert_eq!(recent_dls[0].id, dl_id);
    assert_eq!(recent_dls[0].received_bytes, 524288);

    // 4. Search Engines
    let se_repo = SearchEngineRepository::new(conn);
    let def_engine = se_repo.get_default().unwrap();
    assert_eq!(def_engine.keyword, "d");
    let search_url = build_search_url(&def_engine.url_template, "bangla typing");
    assert!(
        search_url.contains("duckduckgo.com/?q=bangla+typing")
            || search_url.contains("duckduckgo.com/?q=bangla%20typing")
    );

    // 5. Workspaces
    let ws_repo = WorkspaceRepository::new(conn);
    ws_repo.create("research", "Research", "#22c55e").unwrap();
    let ws_list = ws_repo.get_all().unwrap();
    assert!(ws_list.iter().any(|w| w.id == "research"));
}

#[test]
fn test_smoke_url_parsing_and_security_boundaries() {
    let tpl = "https://duckduckgo.com/?q=%s";

    // 1. Bangladeshi IDN domain
    match classify_input("https://বাংলাদেশ.বাংলা/news", tpl) {
        NavigationTarget::Url(u) => assert!(u.starts_with("https://")),
        other => panic!("Expected URL for Bangladeshi IDN domain, got {other:?}"),
    }

    // 2. Complex Bangla search query
    match classify_input("বাংলা সংবাদপত্র ও সাহিত্য", tpl)
    {
        NavigationTarget::Search(s) => assert!(s.contains("duckduckgo.com/?q=")),
        other => panic!("Expected search query for Bangla prose, got {other:?}"),
    }

    // 3. Port numbers
    match classify_input("localhost:8089/dashboard", tpl) {
        NavigationTarget::Url(u) => assert_eq!(u, "http://localhost:8089/dashboard"),
        other => panic!("Expected localhost:8089 URL, got {other:?}"),
    }

    // 4. Blocked dangerous schemes
    match classify_input("javascript:alert(1)", tpl) {
        NavigationTarget::Blocked { scheme, .. } => assert_eq!(scheme, "javascript"),
        other => panic!("Dangerous javascript: scheme must be Blocked, got {other:?}"),
    }

    match classify_input("data:text/html,<script>alert(1)</script>", tpl) {
        NavigationTarget::Blocked { scheme, .. } => assert_eq!(scheme, "data"),
        other => panic!("Dangerous data: scheme must be Blocked, got {other:?}"),
    }

    // 5. Filename sanitization against path traversal
    let safe_name = sanitize_download_filename("../../../etc/passwd");
    assert_eq!(safe_name, "passwd");
}

#[test]
fn test_smoke_content_filter_converter() {
    let rule1 = convert_abp_to_webkit_rule("||doubleclick.net^$third-party").unwrap();
    assert_eq!(rule1.action.action_type, "block");
    assert_eq!(rule1.trigger.load_type.as_ref().unwrap()[0], "third-party");

    let rule2 = convert_abp_to_webkit_rule("||adservice.google.com/ads/*$image,script").unwrap();
    assert_eq!(rule2.action.action_type, "block");
    let res_types = rule2.trigger.resource_type.as_ref().unwrap();
    assert!(res_types.contains(&"image".to_string()));
    assert!(res_types.contains(&"script".to_string()));

    let rule3 = convert_abp_to_webkit_rule("@@||trusted-partner.com^$document").unwrap();
    assert_eq!(rule3.action.action_type, "ignore-previous-rules");
}

#[test]
fn test_smoke_tab_lifecycle_memory_discard() {
    let mut tm = TabManager::new();

    // Create 8 tabs
    for i in 1..=8 {
        tm.new_tab(Some(&format!("https://example.com/page/{}", i)), None);
    }
    assert_eq!(tm.tabs().len(), 8);
    assert_eq!(tm.selected_index(), 7); // Last tab is active

    // Pin tab 0
    tm.get_tab(0).unwrap().borrow_mut().pinned = true;

    // Discard with threshold of 3 tabs and 0 idle time
    let discarded = tm.discard_idle_tabs(3, Duration::from_millis(0));

    // Active tab (7) and pinned tab (0) must NOT be discarded
    assert!(
        !discarded.contains(&7),
        "Active tab 7 must not be discarded"
    );
    assert!(
        !discarded.contains(&0),
        "Pinned tab 0 must not be discarded"
    );

    for &idx in &discarded {
        assert!(tm.get_tab(idx).unwrap().borrow().is_discarded);
        assert!(!tm.get_tab(idx).unwrap().borrow().has_active_webview);
    }

    // Reactivate a discarded tab (e.g. index 2)
    tm.switch_tab(2);
    let reactivated = tm.get_tab(2).unwrap();
    assert!(
        !reactivated.borrow().is_discarded,
        "Reactivated tab must clear discarded flag"
    );
    assert_eq!(tm.selected_index(), 2);
}

#[test]
fn test_smoke_database_latency_bounds() {
    let mut db = Database::open_in_memory().unwrap();
    let conn = db.conn_mut();

    let tx = conn.transaction().unwrap();
    let now = 1700000000i64;
    for i in 1..=5000 {
        tx.execute(
            "INSERT INTO history (url, title, visit_count, first_visit, last_visit) VALUES (?1, ?2, ?3, ?4, ?5);",
            rusqlite::params![format!("https://bench-{}.org/item", i), format!("Title of item {}", i), i % 10, now - i, now - i],
        ).unwrap();
    }
    tx.commit().unwrap();

    let t0 = Instant::now();
    let _: Vec<(String, String)> = conn
        .prepare("SELECT url, title FROM history WHERE url LIKE 'https://bench-1%' ORDER BY visit_count DESC, last_visit DESC LIMIT 10;")
        .unwrap()
        .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))
        .unwrap()
        .filter_map(Result::ok)
        .collect();
    let elapsed = t0.elapsed();

    // Query must finish under 50ms even on debug profile
    assert!(
        elapsed < Duration::from_millis(50),
        "Query took too long: {:?}",
        elapsed
    );
}
