# Shadhin Browser — Developer Placeholders & Mock Registry

This document catalogs all developer placeholders, temporary in-memory stubs, and Phase 5 integration markers in Shadhin Browser. Each item is marked in source for systematic tracking and removal during subsequent phases.

---

## Active Mock / Placeholder Registry (Post Phase 7)

| Component | Source File | Current Behavior | Target Phase |
|---|---|---|---|
| **Profile Button** | `src/ui/toolbar.rs` | Visual action | **Future / Post Phase 7** (Cloud sync / multi-user profiles) |

---

## Phase 7 Completed Real Implementations (Testing & Packaging)
- **Zero `unsafe` in Rust Source**: Eliminated all `unsafe` blocks across `src/` (including debounce timer refactor). `grep -rn unsafe src/` returns 0 hits.
- **Reproducible & Stripped Release Binary**: Configured `Cargo.toml` with `panic = "abort"`, `lto = true`, `codegen-units = 1`, and `--remap-path-prefix` eliminating host paths and usernames from binaries.
- **Command Line Parser**: Robust CLI interface supporting `--version`, `--help`, `--new-window`, `--private-window`, `--profile-dir <PATH>` (with absolute path & 0700 permission validation), and positional `%U` URL arguments (`src/browser/cli.rs`, `src/app.rs`).
- **Comprehensive Security Test Suite**: 10 unit and fuzz tests in `tests/security_tests.rs` covering input sanitization, navigation policies, path traversals, and bookmark integrity (`docs/SECURITY_TESTS.md`).
- **Standard Packaging Matrix**:
  - Debian/Ubuntu (`.deb`) via `dpkg-deb` with strict 0755/0644 file modes, maintainer scripts, and man page (`packaging/deb/`, `scripts/package-deb.sh`).
  - Flatpak manifest targeting GNOME 47 with 110 vendored crates and least-privilege sandboxing (`packaging/flatpak/`, `scripts/package-flatpak.sh`).
  - Fedora/RHEL (`.rpm`) spec file targeting Fedora 40/41 (`packaging/rpm/`, `scripts/package-rpm.sh`).
  - Master pipeline and checksum generator (`scripts/package-all.sh`).
- **Automated CI Validation**: Master script `scripts/ci-check.sh` aggregating format, clippy, unit/smoke/security tests, cargo-audit, cargo-deny, and appstreamcli validation.

---

## Phase 6 Completed Real Implementations (Optimization)
- **Zero-Allocation Debouncer**: Safe `glib::timeout_add_local_once` debounce pattern eliminating CPU spikes.
- **SQLite Performance**: Prepared statements, WAL mode, memory cache, and optimized compound indexes (`idx_history_visit_time`, `idx_bookmarks_parent_order`).
- **Tab Lifecycle Management**: Lazy tab loading on session restore and memory conservation.

---

## Phase 5 Completed Real Implementations (Mocks Removed)
- **WebKit Content Filtering Engine**: Native EasyList & EasyPrivacy ABP rule compilation into WebKit bytecode via `WebKitUserContentFilterStore`. Dynamic per-site shield bypass stored in SQLite `site_exceptions` table (`src/privacy/adblock.rs`).
- **Link Tracking Stripper**: Non-destructive URL query parameter stripper stripping 21 known telemetry tracking parameters (`utm_*`, `fbclid`, `gclid`, etc.) on address bar submission and link traversal (`src/privacy/tracker.rs`).
- **HTTPS-Only Mode & Downgrade Interstitial**: Automatic main-frame HTTP -> HTTPS upgrade in `connect_decide_policy`, with local/RFC1918 exemptions, failed upgrade tracking, and styled fallback interstitial view with temporary session bypass (`src/privacy/https_only.rs`, `src/ui/window.rs`).
- **Cookie Acceptance & ITP Integration**: WebKit cookie accept policies (`BlockThirdParty`, `BlockAll`, `AllowAll`) and Intelligent Tracking Prevention (ITP) applied to active network sessions (`src/privacy/cookies.rs`).
- **Clear Browsing Data Dialog**: Full multi-category clearing dialog (`Ctrl+Shift+Delete`) supporting Time Ranges (Last hour, 24 hours, 7 days, All time) for history, cookies, cache, local storage, and stored permissions via `WebKitWebsiteDataManager` (`src/ui/clear_data.rs`).
- **Site Permission Request Infobar**: Non-modal prompt bar for camera, microphone, geolocation, and notifications with Remember Decision checkbox and persistent SQLite backing (`src/browser/permissions.rs`, `src/ui/window.rs`, `src/ui/settings.rs`).
- **Private Browsing Windows**: Dedicated `ShadhinWindow::new_private` using ephemeral in-memory `WebKitNetworkSession`, violet titlebar badge, banner disclaimer, and strict `persistence_allowed = false` write suppression (`src/ui/window.rs`, `src/ui/titlebar.rs`).
- **Strict User Data Permissions**: `0700` directory and `0600` file modes strictly enforced on `~/.local/share/shadhin-browser/` and `~/.config/shadhin-browser/` (`src/storage/database.rs`, `src/config.rs`).

---

## Phase 4 Completed Real Implementations (Mocks Removed)
- **SQLite Storage Subsystem**: Forward-only migrations, strict `0700` directory and `0600` file modes, WAL mode, foreign keys, asynchronous worker thread with GLib MainContext invocation (`src/storage/`).
- **Bookmarks Management & Import/Export**: Relational bookmark folders and items, address bar star toggle with popover, sidebar bookmarks panel, Netscape HTML & JSON import/export with 10MB hostile file protection (`src/storage/bookmarks.rs`, `src/storage/import_export.rs`, `src/ui/bookmarks_panel.rs`).
- **Browsing History & Autocomplete**: Day-grouped history (Today, Yesterday, Earlier), address bar autocomplete dropdown (debounced ~150ms, bookmarks + history matches + keyword search), clear history dialog with range deletion (`src/storage/history.rs`, `src/ui/history_panel.rs`, `src/ui/addressbar.rs`).
- **Download Manager**: WebKit download interception, filename sanitizer with directory traversal neutralization, collision evasion (" (1)"), non-executable `0600` permissions, dangerous extension warning badges, "Show in folder" safe action (`src/browser/downloads.rs`, `src/ui/downloads_panel.rs`).
- **Search Engines & New Tab Shortcuts**: 6 built-in engines with `%s` template queries and keyword shortcuts (`w linux`), dynamic shortcut tiles on New Tab with SQLite backing (`src/storage/search_engines.rs`, `src/storage/shortcuts.rs`, `src/ui/newtab.rs`).
- **Workspaces & Session Isolation**: Multi-workspace tab management implemented via separate NetworkSession data directories (`ws_personal`, `ws_work`, `ws_project`); runtime isolation NOT verified. Session tab persistence and crash recovery lockfile banner implemented (`src/browser/session.rs`, `src/storage/workspaces.rs`).
- **Libadwaita Preferences Window**: Real multi-section settings window (General, Search, Appearance, Downloads, Privacy, About) with wallpaper image validation and corrupt TOML fallback (`src/ui/settings.rs`, `src/config.rs`).
