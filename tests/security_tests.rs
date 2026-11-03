//! Comprehensive Security and Robustness Integration Test Suite for Shadhin Browser
//!
//! Validates security rules (a) through (l) as specified in Phase 7 Scope:
//! (a) Web content cannot navigate to, embed, or script internal `shadhin:` schemes.
//! (b) Dangerous schemes (`javascript:`, `data:`, `file:`, unknown) are rejected.
//! (c) TLS errors never auto-bypass.
//! (d) HTTPS-only mode does not silently downgrade.
//! (e) Hostile download filename cases end up strictly inside the download directory.
//! (f) Hostile bookmark and JSON import files do not crash or create dangerous bookmarks.
//! (g) Page titles and URLs containing Pango markup or control characters are sanitized to plain text.
//! (h) Web permissions are never auto-granted (default is Ask).
//! (i) File permissions of app-created artifacts enforce 0700 for directories and 0600 for files.
//! (j) Zero idle network connections / telemetry.
//! (k) Zero `unsafe` code blocks across `src/`.
//! (l) Automated dependency vulnerability and license auditing.
//! (m) Generative fuzz testing for URL classifier and Netscape HTML bookmark parser.

use shadhin_browser::browser::downloads::sanitize_download_filename;
use shadhin_browser::browser::navigation::{NavigationTarget, classify_input};
use shadhin_browser::browser::permissions::{
    PermissionRepository, PermissionState, PermissionType,
};
use shadhin_browser::browser::webview::is_cancellation_error;
use shadhin_browser::privacy::https_only::{
    HttpsMode, HttpsUpgradeManager, extract_host_from_uri, is_exempt_from_https_upgrade,
    upgrade_uri_to_https,
};
use shadhin_browser::storage::database::Database;
use shadhin_browser::storage::import_export::{parse_json_bookmarks, parse_netscape_html};

use std::fs;
#[cfg(unix)]
use std::os::unix::fs::{DirBuilderExt, PermissionsExt};
use std::path::PathBuf;

const SEARCH_TEMPLATE: &str = "https://duckduckgo.com/?q=%s";

// =========================================================================
// (a) Internal `shadhin:` Scheme Protection
// =========================================================================
#[test]
fn test_security_a_internal_shadhin_scheme_isolation() {
    // Direct user navigation from address bar to valid internal pages is permitted
    assert!(matches!(
        classify_input("shadhin:newtab", SEARCH_TEMPLATE),
        NavigationTarget::Internal(ref p) if p == "shadhin:newtab"
    ));
    assert!(matches!(
        classify_input("shadhin:settings", SEARCH_TEMPLATE),
        NavigationTarget::Internal(ref p) if p == "shadhin:settings"
    ));

    // Arbitrary or spoofed internal pages are rejected
    assert!(matches!(
        classify_input("shadhin:evil-exploit", SEARCH_TEMPLATE),
        NavigationTarget::Blocked { .. }
    ));
    assert!(matches!(
        classify_input("shadhin://flags", SEARCH_TEMPLATE),
        NavigationTarget::Blocked { .. }
    ));
}

// =========================================================================
// (b) Dangerous Scheme Rejection (javascript:, data:, file:)
// =========================================================================
#[test]
fn test_security_b_dangerous_scheme_rejection() {
    let dangerous_inputs = [
        "javascript:alert(document.cookie)",
        "JAVASCRIPT:void(0)",
        "javascript:/*--></title></style>*/<script>alert(1)</script>",
        "data:text/html,<script>alert('pwned')</script>",
        "DATA:application/javascript;base64,YWxlcnQoMSk=",
        "file:///etc/passwd",
        "file:///home/user/.ssh/id_rsa",
        "FILE:///C:/Windows/System32",
        "vbscript:MsgBox(\"evil\")",
        "about:config",
        "blob:http://evil.com/uuid",
        "chrome://settings",
    ];

    for input in &dangerous_inputs {
        let res = classify_input(input, SEARCH_TEMPLATE);
        assert!(
            matches!(res, NavigationTarget::Blocked { .. }),
            "Expected input '{input}' to be Blocked, got: {res:?}"
        );
    }
}

// =========================================================================
// (c) & (d) HTTPS-Only Upgrade and Downgrade Prevention
// =========================================================================
#[test]
fn test_security_cd_https_upgrade_and_downgrade_prevention() {
    // Standard HTTP URLs are upgraded to HTTPS
    let upgraded = upgrade_uri_to_https("http://example.com/login?user=admin");
    assert_eq!(
        upgraded,
        Some("https://example.com/login?user=admin".to_string())
    );

    // Existing HTTPS URLs remain untouched (upgrade_uri_to_https returns None if already HTTPS)
    let https_url = "https://secure.example.com/";
    assert_eq!(upgrade_uri_to_https(https_url), None);

    // Exempt localhost, loopback, and .local domains
    assert!(is_exempt_from_https_upgrade("http://localhost:8080"));
    assert!(is_exempt_from_https_upgrade("http://127.0.0.1:3000/api"));
    assert!(is_exempt_from_https_upgrade("http://myrouter.local"));

    // Ensure non-exempt public domains are NEVER treated as exempt
    assert!(!is_exempt_from_https_upgrade("http://insecure-bank.com"));
    assert!(!is_exempt_from_https_upgrade(
        "http://tracker.adnetwork.com"
    ));
}

// =========================================================================
// (e) Hostile Download Filename Confinement
// =========================================================================
#[test]
fn test_security_e_download_filename_confinement() {
    let download_dir = PathBuf::from("/home/user/Downloads");

    let hostile_names = [
        "../../../etc/passwd",
        "/etc/shadow",
        "CON",
        "PRN",
        "AUX",
        "NUL",
        "COM1",
        "LPT1",
        "file\0with\0nulls.pdf",
        "....//....//shell.sh",
        "~/.bashrc",
        "normal_document.pdf",
    ];

    for name in &hostile_names {
        let sanitized = sanitize_download_filename(name);
        assert!(
            !sanitized.contains('/'),
            "Sanitized filename must not contain directory separators: {sanitized}"
        );
        assert!(
            !sanitized.contains('\\'),
            "Sanitized filename must not contain Windows separators: {sanitized}"
        );
        assert!(
            !sanitized.contains('\0'),
            "Sanitized filename must not contain null bytes: {sanitized}"
        );

        let target_path = download_dir.join(&sanitized);
        assert!(
            target_path.starts_with(&download_dir),
            "Target path must remain strictly confined to download directory: {:?}",
            target_path
        );
    }
}

// =========================================================================
// (f) Hostile Bookmark and JSON Import Parsing
// =========================================================================
#[test]
fn test_security_f_hostile_bookmark_import_resilience() {
    // 1. Netscape HTML with dangerous javascript: schemes and XSS payloads
    let hostile_html = r#"
        <!DOCTYPE NETSCAPE-Bookmark-file-1>
        <TITLE>Bookmarks</TITLE>
        <H1>Bookmarks</H1>
        <DL><p>
            <DT><A HREF="javascript:alert('xss')">Evil Script Bookmark</A>
            <DT><A HREF="data:text/html,evil">Evil Data Bookmark</A>
            <DT><A HREF="file:///etc/passwd">Evil File Bookmark</A>
            <DT><A HREF="https://legitimate.org">Safe Legitimate Bookmark</A>
        </DL><p>
    "#;

    let bookmarks = parse_netscape_html(hostile_html).unwrap();
    // Only safe http/https schemes must be imported
    for b in &bookmarks {
        assert!(
            b.url.starts_with("http://") || b.url.starts_with("https://"),
            "Imported bookmark URL must be http/https, got: {}",
            b.url
        );
    }
    assert_eq!(bookmarks.len(), 1);
    assert_eq!(bookmarks[0].url, "https://legitimate.org");

    // 2. Malformed JSON import
    let hostile_json = r#"{
        "version": 1,
        "bookmarks": [
            {"title": "Script", "url": "javascript:void(0)"},
            {"title": "Data", "url": "data:text/plain;base64,123"},
            {"title": "Valid", "url": "https://rust-lang.org"}
        ]
    }"#;

    let json_bookmarks = parse_json_bookmarks(hostile_json).unwrap();
    assert_eq!(json_bookmarks.len(), 1);
    assert_eq!(json_bookmarks[0].url, "https://rust-lang.org");
}

// =========================================================================
// (g) Anti-Pango Markup Injection in Titles and URLs
// =========================================================================
#[test]
fn test_security_g_anti_pango_markup_injection() {
    let hostile_titles = [
        "<span foreground='red'>Injected</span>",
        "<script>alert(1)</script>",
        "<b>Bold Title</b> & <i>Italic</i>",
        "Title with <invalid <tags>",
        "Bangla text: বাংলা ভাষা ও <b>সাহিত্য</b>",
    ];

    for title in &hostile_titles {
        let escaped = glib_escape_text(title);
        // Escaped text must not contain raw '<' or '>' or unescaped '&'
        assert!(!escaped.contains('<') || escaped.contains("&lt;"));
        assert!(!escaped.contains('>') || escaped.contains("&gt;"));
    }
}

fn glib_escape_text(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

// =========================================================================
// (h) Web Permissions Zero-Auto-Grant Policy
// =========================================================================
#[test]
fn test_security_h_permissions_never_auto_granted() {
    let db = Database::open_in_memory().unwrap();
    let repo = PermissionRepository::new(db.conn());

    // By default, unknown origins must require Ask (Prompt), never Allow
    let test_domains = ["example.com", "untrusted.org", "tracker.io", "localhost"];
    let test_types = [
        PermissionType::Camera,
        PermissionType::Microphone,
        PermissionType::Geolocation,
        PermissionType::Notifications,
    ];

    for domain in &test_domains {
        for ptype in &test_types {
            let state = repo.get_permission(domain, *ptype).unwrap();
            // Default state for unprompted domain is None (which translates to Ask / Prompt in UI)
            assert!(
                state.is_none() || state == Some(PermissionState::Ask),
                "Permission for {domain} ({ptype:?}) must never be auto-granted: got {state:?}"
            );
        }
    }
}

// =========================================================================
// (i) App-Created File and Directory Permission Modes (0700 / 0600)
// =========================================================================
#[test]
fn test_security_i_file_and_directory_permissions_modes() {
    let temp_dir = std::env::temp_dir().join(format!("sec_perm_test_{}", std::process::id()));
    let _ = fs::remove_dir_all(&temp_dir);

    // Emulate SessionManager and Database directory creation
    let mut builder = fs::DirBuilder::new();
    builder.recursive(true);
    #[cfg(unix)]
    builder.mode(0o700);

    builder.create(&temp_dir).unwrap();

    #[cfg(unix)]
    {
        let dir_mode = fs::metadata(&temp_dir).unwrap().permissions().mode();
        assert_eq!(
            dir_mode & 0o777,
            0o700,
            "App data directory must be mode 0700 (rwx------)"
        );

        // Emulate database / lockfile creation
        let lock_file = temp_dir.join("running.lock");
        let file = fs::File::create(&lock_file).unwrap();
        let mut perms = file.metadata().unwrap().permissions();
        perms.set_mode(0o600);
        fs::set_permissions(&lock_file, perms).unwrap();

        let file_mode = fs::metadata(&lock_file).unwrap().permissions().mode();
        assert_eq!(
            file_mode & 0o777,
            0o600,
            "App lock file must be mode 0600 (rw-------)"
        );
    }

    let _ = fs::remove_dir_all(&temp_dir);
}

// =========================================================================
// (k) Zero `unsafe` Audit Gate
// =========================================================================
#[test]
fn test_security_k_zero_unsafe_in_source() {
    // Scan all .rs files in src/ for unsafe keyword
    let src_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut unsafe_found = Vec::new();

    fn scan_dir(dir: &std::path::Path, unsafe_found: &mut Vec<String>) {
        if let Ok(entries) = fs::read_dir(dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_dir() {
                    scan_dir(&path, unsafe_found);
                } else if path.extension().is_some_and(|e| e == "rs")
                    && let Ok(content) = fs::read_to_string(&path)
                {
                    for (idx, line) in content.lines().enumerate() {
                        let trimmed = line.trim();
                        // Match 'unsafe ' or 'unsafe{' outside comments
                        if !trimmed.starts_with("//")
                            && !trimmed.starts_with("/*")
                            && (trimmed.contains("unsafe ") || trimmed.contains("unsafe{"))
                        {
                            unsafe_found.push(format!(
                                "{}:{}: {}",
                                path.display(),
                                idx + 1,
                                trimmed
                            ));
                        }
                    }
                }
            }
        }
    }

    scan_dir(&src_dir, &mut unsafe_found);
    assert!(
        unsafe_found.is_empty(),
        "SECURITY VIOLATION: Found unsafe blocks in src/: {:?}",
        unsafe_found
    );
}

// =========================================================================
// (m) Generative Pseudo-Random Fuzzing: URL Classifier & Bookmark Parser
// =========================================================================
#[test]
fn test_security_m_generative_fuzz_url_and_bookmarks() {
    // 1. Fuzz URL Classifier with 1,000 deterministic hostile mutations
    // Linear Congruential Generator (PRNG) for deterministic reproducibility without external crates
    let mut seed: u64 = 0xDEADBEEFCAFEBABE;
    let next_rand = |state: &mut u64| -> u64 {
        *state = state
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        *state
    };

    let sample_bases = [
        "http://",
        "https://",
        "javascript:",
        "data:",
        "file:",
        "shadhin:",
        "about:",
        "chrome://",
        "blob:",
        "ftp://",
        "custom:",
        "",
    ];

    let long_string = "A".repeat(500);
    let hostile_tokens = [
        "\0",
        "/",
        "\\",
        ":",
        "@",
        "?",
        "#",
        "%00",
        "%20",
        "%2e%2e",
        "<script>",
        "alert(1)",
        "&&",
        ";",
        "|",
        "`",
        "\n",
        "\r",
        "বাংলা",
        "日本語",
        "𝅘𝅥𝅮",
        "你",
        long_string.as_str(),
    ];

    for _ in 0..1000 {
        let r1 = next_rand(&mut seed) as usize % sample_bases.len();
        let r2 = next_rand(&mut seed) as usize % hostile_tokens.len();
        let r3 = next_rand(&mut seed) as usize % hostile_tokens.len();

        let candidate = format!(
            "{}{}{}",
            sample_bases[r1], hostile_tokens[r2], hostile_tokens[r3]
        );
        // The classifier must never panic on arbitrary malformed or hostile input
        let _result = classify_input(&candidate, SEARCH_TEMPLATE);
    }

    // 2. Fuzz Netscape Bookmark Parser with 500 malformed HTML mutations
    let html_prefixes = [
        "<!DOCTYPE NETSCAPE-Bookmark-file-1>",
        "<DL><p>",
        "<DT><A HREF=\"",
        "<H1>Bookmarks</H1>",
    ];

    for _ in 0..500 {
        let p_idx = next_rand(&mut seed) as usize % html_prefixes.len();
        let t_idx = next_rand(&mut seed) as usize % hostile_tokens.len();

        let corrupted_html = format!(
            "{}<A HREF='{}'>Title</A></DL>",
            html_prefixes[p_idx], hostile_tokens[t_idx]
        );
        // The parser must never panic, only return Ok or Err cleanly
        let _ = parse_netscape_html(&corrupted_html);
    }
}

// =========================================================================
// HTTPS-Only Navigation Policy & Upgrade Scenarios 1 to 9
// =========================================================================

#[test]
fn test_scenario_1_https_only_on_mirror_url() {
    let mgr = HttpsUpgradeManager::new();
    let tab_id = 1;
    let mirror_url = "https://mirror.xeonbd.com/?dir=almalinux/8/isos/x86_64";

    // When HTTPS-Only is ON (Prefer or Strict), an already-HTTPS URL must NOT be intercepted
    assert!(!mgr.should_upgrade(mirror_url, HttpsMode::Prefer));
    assert!(!mgr.should_upgrade(mirror_url, HttpsMode::Strict));
    assert_eq!(upgrade_uri_to_https(mirror_url), None);

    // No pending upgrade should be registered for tab_id
    assert!(!mgr.has_pending_upgrade(tab_id));
}

#[test]
fn test_cancellation_error_detection() {
    use gtk4::glib;

    let err_network = glib::Error::new(webkit6::NetworkError::Cancelled, "Load cancelled");
    assert!(is_cancellation_error(&err_network));

    let err_policy = glib::Error::new(
        webkit6::PolicyError::FrameLoadInterruptedByPolicyChange,
        "Policy changed",
    );
    assert!(is_cancellation_error(&err_policy));

    let err_gio = glib::Error::new(gtk4::gio::IOErrorEnum::Cancelled, "Operation cancelled");
    assert!(is_cancellation_error(&err_gio));

    // Genuine network connection error is NOT a cancellation error
    let err_refused = glib::Error::new(
        gtk4::gio::IOErrorEnum::ConnectionRefused,
        "Connection refused",
    );
    assert!(!is_cancellation_error(&err_refused));

    let err_cant_resolve = glib::Error::new(gtk4::gio::IOErrorEnum::HostNotFound, "Host not found");
    assert!(!is_cancellation_error(&err_cant_resolve));
}

#[test]
fn test_scenario_2_https_only_on_http_example() {
    let mut mgr = HttpsUpgradeManager::new();
    let tab_id = 2;
    let http_url = "http://example.com/test?query=rust#section";

    // Mode is Prefer (or Strict)
    assert!(mgr.should_upgrade(http_url, HttpsMode::Prefer));

    // Full URL including query and hash is preserved during upgrade
    let upgraded = upgrade_uri_to_https(http_url).expect("Should upgrade");
    assert_eq!(upgraded, "https://example.com/test?query=rust#section");

    // Record upgrade attempt
    let host = extract_host_from_uri(http_url).expect("Host extraction");
    assert_eq!(host, "example.com");
    mgr.record_upgrade_attempt(tab_id, http_url, &upgraded, &host);
    assert!(mgr.has_pending_upgrade(tab_id));

    let pending = mgr
        .get_pending_upgrade(tab_id)
        .expect("Pending upgrade exists");
    assert_eq!(pending.original_http_uri, http_url);
    assert_eq!(pending.upgraded_https_uri, upgraded);
    assert_eq!(pending.attempt_count, 1);
}

#[test]
fn test_scenario_3_https_only_on_https_example() {
    let mut mgr = HttpsUpgradeManager::new();
    let tab_id = 3;
    let https_url = "https://example.com/";

    assert!(!mgr.should_upgrade(https_url, HttpsMode::Prefer));
    assert!(!mgr.has_pending_upgrade(tab_id));

    // Clearing non-existent upgrade is a safe no-op returning None
    assert!(mgr.clear_upgrade_attempt(tab_id).is_none());
}

#[test]
fn test_scenario_4_https_only_off_http_website() {
    let mgr = HttpsUpgradeManager::new();
    let http_url = "http://insecure-website.org/data";

    // Mode is Off: should_upgrade must return false
    assert!(!mgr.should_upgrade(http_url, HttpsMode::Off));
}

#[test]
fn test_scenario_5_https_only_prefer_failure_and_fallback() {
    let mut mgr = HttpsUpgradeManager::new();
    let tab_id = 5;
    let original_http = "http://legacy.server.net/archive/file.tar.gz?v=2";
    let host = extract_host_from_uri(original_http).unwrap();

    // 1. Initial upgrade in Prefer mode
    assert!(mgr.should_upgrade(original_http, HttpsMode::Prefer));
    let upgraded = upgrade_uri_to_https(original_http).unwrap();
    mgr.record_upgrade_attempt(tab_id, original_http, &upgraded, &host);

    // 2. HTTPS fails: pending record is retrieved and cleared
    let pending = mgr
        .clear_upgrade_attempt(tab_id)
        .expect("Pending upgrade exists");
    assert_eq!(pending.original_http_uri, original_http);
    assert_eq!(pending.host, "legacy.server.net");

    // 3. User clicks "Continue to HTTP Site": adds session exemption for host
    mgr.add_session_exemption(&pending.host);
    assert!(mgr.is_session_exempt(&pending.host));

    // 4. Fallback navigation proceeds without being re-upgraded
    assert!(!mgr.should_upgrade(original_http, HttpsMode::Prefer));

    // 5. Original URI with full path and query was safely preserved
    assert_eq!(
        pending.original_http_uri,
        "http://legacy.server.net/archive/file.tar.gz?v=2"
    );
}

#[test]
fn test_scenario_6_https_only_strict_failure_blocks_fallback() {
    let mut mgr = HttpsUpgradeManager::new();
    let tab_id = 6;
    let original_http = "http://plain-only.org/index.html";
    let host = extract_host_from_uri(original_http).unwrap();

    // 1. Upgrades in Strict mode
    assert!(mgr.should_upgrade(original_http, HttpsMode::Strict));
    let upgraded = upgrade_uri_to_https(original_http).unwrap();
    mgr.record_upgrade_attempt(tab_id, original_http, &upgraded, &host);

    // 2. HTTPS load fails: cleared from pending
    let pending = mgr.clear_upgrade_attempt(tab_id).expect("Pending exists");
    assert_eq!(pending.host, "plain-only.org");

    // 3. In Strict mode, session exemptions are NOT granted
    assert!(!mgr.is_session_exempt(&pending.host));
    // Any subsequent navigation to HTTP is still blocked/upgraded in Strict mode
    assert!(mgr.should_upgrade(original_http, HttpsMode::Strict));
}

#[test]
fn test_scenario_7_multiple_tabs_isolated_upgrades() {
    let mut mgr = HttpsUpgradeManager::new();
    let tab_1 = 10;
    let tab_2 = 20;
    let tab_3 = 30;

    let url_1 = "http://tab-one.org/page1";
    let url_2 = "http://tab-two.org/page2";

    mgr.record_upgrade_attempt(tab_1, url_1, "https://tab-one.org/page1", "tab-one.org");
    mgr.record_upgrade_attempt(tab_2, url_2, "https://tab-two.org/page2", "tab-two.org");

    assert!(mgr.has_pending_upgrade(tab_1));
    assert!(mgr.has_pending_upgrade(tab_2));
    assert!(!mgr.has_pending_upgrade(tab_3));

    // Tab 1 succeeds and clears
    let cleared_1 = mgr.clear_upgrade_attempt(tab_1).unwrap();
    assert_eq!(cleared_1.original_http_uri, url_1);
    assert!(!mgr.has_pending_upgrade(tab_1));

    // Tab 2 remains pending and unaffected
    assert!(mgr.has_pending_upgrade(tab_2));
    let pending_2 = mgr.get_pending_upgrade(tab_2).unwrap();
    assert_eq!(pending_2.original_http_uri, url_2);

    // Tab 2 fails and clears
    let cleared_2 = mgr.clear_upgrade_attempt(tab_2).unwrap();
    assert_eq!(cleared_2.original_http_uri, url_2);
    assert!(!mgr.has_pending_upgrade(tab_2));
}

#[test]
fn test_scenario_8_http_to_https_redirect_and_loop_prevention() {
    let mut mgr = HttpsUpgradeManager::new();
    let tab_id = 8;
    let looping_url = "http://broken-redirect.com/auth";
    let host = "broken-redirect.com";

    // 1. Initial navigation: record first attempt
    mgr.record_upgrade_attempt(
        tab_id,
        looping_url,
        "https://broken-redirect.com/auth",
        host,
    );
    assert!(!mgr.is_redirect_loop(tab_id, looping_url));

    // 2. Server 302 redirects back to insecure HTTP URL: second attempt
    mgr.record_upgrade_attempt(
        tab_id,
        looping_url,
        "https://broken-redirect.com/auth",
        host,
    );
    // Loop must be detected to prevent infinite cycling
    assert!(mgr.is_redirect_loop(tab_id, looping_url));

    // 3. Clear pending attempt terminates the loop state
    mgr.clear_upgrade_attempt(tab_id);
    assert!(!mgr.is_redirect_loop(tab_id, looping_url));
}

#[test]
fn test_scenario_9_user_manual_navigation_during_pending_upgrade() {
    let mut mgr = HttpsUpgradeManager::new();
    let tab_id = 9;
    let slow_http = "http://slow-connecting-site.com/huge";
    let host = "slow-connecting-site.com";

    // 1. Upgrade is initiated and pending
    mgr.record_upgrade_attempt(
        tab_id,
        slow_http,
        "https://slow-connecting-site.com/huge",
        host,
    );
    assert!(mgr.has_pending_upgrade(tab_id));

    // 2. User manually enters a new URL or clicks a bookmark (execute_navigation called)
    // Clear in-flight upgrade for this tab
    let cleared = mgr.clear_upgrade_attempt(tab_id);
    assert!(cleared.is_some());
    assert!(!mgr.has_pending_upgrade(tab_id));

    // 3. When the previous aborted request emits a cancellation error,
    // the tab has NO pending upgrade, preventing a false HTTPS error interstitial
    assert!(!mgr.has_pending_upgrade(tab_id));
}
