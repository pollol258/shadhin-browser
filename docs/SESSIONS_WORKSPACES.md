# Sessions, Workspaces & Crash Recovery Specification

## 1. Workspaces Architecture
Shadhin Browser provides compartmentalization between workspaces (`Personal`, `Work`, `Project`, and custom workspaces).

### Cookie & Site Data Isolation Status
Each workspace in Shadhin Browser is bound to a dedicated WebKit `NetworkSession`:
- Dedicated data directory: `~/.local/share/shadhin-browser/sessions/ws_<id>/` (Mode `0700`)
- Dedicated cache directory: `~/.cache/shadhin-browser/sessions/ws_<id>/` (Mode `0700`)
- WebViews created within a workspace are constructed with `.network_session(&session)`.

> [!NOTE]
> **Verification Status**: Implemented via separate `NetworkSession` instances; cookie and site data isolation is NOT verified via interactive/end-to-end testing.

## 2. Session Persistence
Open tabs for each workspace are tracked and persisted in the SQLite `session_tabs` table:
- Columns: `workspace_id`, `url`, `title`, `position`, `is_active`.
- Saved automatically on workspace switch, tab close, and browser shutdown.

### Startup Behavior
Configured via `BrowserConfig::startup_behavior`:
- `"open_new_tab"`: Opens a clean single tab with `about:newtab`.
- `"continue_previous_session"`: Restores the tabs that were open when the browser last exited.

### Lazy WebView Restoration
When restoring tabs from a previous session, tabs are allocated lazily:
- Titles, URLs, and icons are rendered immediately in the tab strip and sidebar.
- Underlying heavy WebKit `WebView` instances are only initialized when the user switches to or clicks the tab, saving memory and CPU on launch.

## 3. Crash Recovery Detection
Shadhin Browser maintains a lockfile `~/.local/share/shadhin-browser/running.lock`:
- Created on startup with mode `0600`.
- Removed upon clean application exit (`window.connect_close_request`).
- If `running.lock` is detected during launch, an unclean exit or crash occurred.
- The browser presents an `adw::Banner` at the top:
  `"Shadhin Browser closed unexpectedly. [Restore Session]"`
  allowing the user to restore their previous session or start fresh.
