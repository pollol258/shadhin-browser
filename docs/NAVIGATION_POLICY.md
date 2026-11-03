# Shadhin Browser — Navigation & Multi-Tab Policy

This document describes the multi-tab navigation policy, new-window/popup handling, lazy WebView allocation, and tab discarding mechanisms in Shadhin Browser.

---

## 1. Multi-Tab Operations & Navigation

- **Tab Creation (`Ctrl+T`)**: A new tab is created in the single source of truth (`TabManager`).
- **Lazy WebView Allocation**: Creating a New Tab displays the native `ShadhinNewTab` component without instantiating a WebKit engine process or `webkit6::WebView`. The heavy `WebView` is created lazily only when the user submits an external URL or search query, saving system memory.
- **Tab Closing (`Ctrl+W`)**:
  - The closed tab's widgets and WebView signals are disconnected and dropped.
  - The URL and title are pushed onto an in-memory `ClosedTabInfo` stack bounded to 10 entries.
  - If the user closes the final tab, the browser automatically spawns a fresh `New Tab` rather than terminating the process unexpectedly.
- **Reopen Closed Tab (`Ctrl+Shift+T`)**: Pops the last closed tab from the 10-item stack and restores it.
- **Tab Switching & Navigation**:
  - `Ctrl+Tab`: Cycle forwards through open tabs.
  - `Ctrl+Shift+Tab`: Cycle backwards through open tabs.
  - `Alt+1` through `Alt+9`: Direct jump to tab positions 1 through 9.

---

## 2. Window Creation & Popup Policy (`connect_create`)

WebKit emits the `create` signal when JavaScript invokes `window.open()` or when a link specifies `target="_blank"`.

1. **User-Initiated Navigation (`target="_blank"`)**:
   - When a page requests a new window via a user interaction (e.g. clicking a link), Shadhin opens the URL in a new tab within the current window rather than spawning unmanaged floating windows.
2. **Unrequested Popups (Popup Blocking)**:
   - WebKit's setting `javascript_can_open_windows_automatically` is strictly set to `false`.
   - Script-initiated popups without explicit user gesture return `None` from the `connect_create` handler and are rejected.

---

## 3. Tab Discarding & Resource Lifecycle

To ensure low RAM footprint on Linux desktops and WSL2 environments:
- **Threshold**: When total open tabs exceed `TAB_DISCARD_THRESHOLD` (8 tabs).
- **Idle Evaluation**: Background tabs that have been idle for longer than 5 minutes and are not pinned can be discarded.
- **Discard Operation**: The heavy WebKit GTK widget is detached and dropped from memory, while the tab's metadata (`url`, `title`, `favicon`, `zoom_level`) is preserved in `Tab`.
- **Reactivation**: When the user switches back to a discarded tab, a new configured `WebView` is allocated and the URL reloaded seamlessly.
