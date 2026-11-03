# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.1.0] - 2026-11-03

### Added
- **Native User Interface**: Built with GTK4 and Libadwaita featuring a dark glassmorphism design, client-side titlebar tab strip, pill address bar, collapsible tri-state sidebar (Expanded, Compact, Hidden), and responsive breakpoints (`AdwBreakpoint`).
- **WebKitGTK 6.0 Engine**: Real multi-process browsing with process isolation, bubblewrap sandboxing, hardware-accelerated rendering, and ephemeral session isolation for Private Windows.
- **Privacy Shield & Content Blocking**: Native filter compilation via `WebKitUserContentFilterStore` with bundled EasyList and EasyPrivacy rule subsets; per-site exception management; HTTPS-Only navigation upgrade policy; and cookie accept policies.
- **Tab & Session Management**: Multi-tab browsing with lazy WebView allocation, background tab discarding (>5 min idle), crash recovery with single-instance lock file (`running.lock`), and workspace compartmentalization (Personal, Work, Project) using dedicated `NetworkSession` directories (`sessions/ws_<id>/`).
- **Storage Subsystem**: Asynchronous SQLite database (`shadhin.db`) with Write-Ahead Logging (WAL) and PRAGMA optimize on clean shutdown; Netscape HTML and JSON import/export for bookmarks; URL autocomplete indexes; and persistent download history with dangerous extension warnings.
- **CLI & Desktop Integration**: Command-line interface with `--version`, `--help`, `--new-window`, `--private-window`, `--profile-dir <PATH>`, and `--new-tab <URL>`; FreeDesktop desktop entry (`com.shadhin.browser.desktop`) with `%U` handling and single-instance routing; AppStream metadata (`com.shadhin.browser.metainfo.xml`); and multi-resolution hicolor icons.
- **Packaging Support**: Distribution manifests and build scripts for Flatpak (`com.shadhin.browser.yml`), Debian/Ubuntu `.deb` (`packaging/deb/`), and Fedora `.rpm` (`packaging/rpm/shadhin-browser.spec`).

### Security
- Zero `unsafe` code blocks across the browser codebase.
- File permission enforcement: `0700` (`rwx------`) on data, config, and session directories; `0600` (`rw-------`) on database and lock files.
- Refusal to synthesize unmeasurable ad/tracker metrics; transparent display of `—` indicators with WebKitGTK limitation notices.
- Disallowance of untrusted internal scheme execution (`javascript:`, `data:`, `file:`, `shadhin:`) from web content contexts.
