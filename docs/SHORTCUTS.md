# Shadhin Browser — Keyboard Shortcuts Reference

Shadhin Browser implements a comprehensive keyboard-driven workflow using GTK4 event controllers (`gtk4::EventControllerKey`). All shortcuts are active and conflict-free.

---

## 1. Tab Management

| Shortcut | Action | Scope / Phase |
|---|---|---|
| `Ctrl + T` | Open a new tab (loads internal New Tab dashboard) | Phase 3 |
| `Ctrl + W` | Close the active tab (leaves fresh New Tab if last tab) | Phase 3 |
| `Ctrl + Shift + T` | Reopen the last closed tab (stack of 10) | Phase 3 |
| `Ctrl + Tab` | Cycle forward to the next tab | Phase 3 |
| `Ctrl + Shift + Tab` | Cycle backward to the previous tab | Phase 3 |
| `Alt + 1` .. `Alt + 9` | Direct switch to tab at position 1 through 9 | Phase 3 |

---

## 2. Navigation & Address Bar

| Shortcut | Action | Scope / Phase |
|---|---|---|
| `Ctrl + L` or `F6` | Focus the address bar and select all text | Phase 3 |
| `Ctrl + R` or `F5` | Reload the active page | Phase 3 |
| `Ctrl + Shift + R` | Hard reload bypassing HTTP caches (`reload_bypass_cache`) | Phase 3 |
| `Alt + Left` | Navigate back in session history | Phase 3 |
| `Alt + Right` | Navigate forward in session history | Phase 3 |
| `Alt + Home` | Navigate to configured Homepage | Phase 4 |
| `Ctrl + D` | Star / bookmark current tab (opens bookmark popover) | Phase 4 |

---

## 3. Sidebar, Panels & Settings

| Shortcut / Control | Action | Scope / Phase |
|---|---|---|
| `Ctrl + B` or `F9` | Toggle sidebar visibility (Expanded / Compact <-> Hidden) | Phase 2 / Phase 4+ |
| Titlebar Toggle Button | Left-click: toggle sidebar; Right-click: layout menu (Expanded, Compact, Hidden) | Phase 4+ UI Polish |
| `Ctrl + Shift + B` | Toggle Bookmarks Bar visibility | Phase 4 |
| `Ctrl + Shift + O` | Open Bookmarks manager panel (`about:bookmarks`) | Phase 4 |
| `Ctrl + H` | Open Browsing History panel (`about:history`) | Phase 4 |
| `Ctrl + J` | Open Downloads manager panel (`about:downloads`) | Phase 4 |
| `Ctrl + ,` | Open Preferences / Settings window | Phase 4 |

---

## 4. Page Search & View

| Shortcut | Action | Scope / Phase |
|---|---|---|
| `Ctrl + F` | Open floating Find in Page search bar (`FindController`) | Phase 3 |
| `Esc` | Close Find in Page bar and dismiss highlights | Phase 3 |
| `Ctrl + +` / `Ctrl + =` | Zoom in page content (+10%, up to 500%) | Phase 3 |
| `Ctrl + -` | Zoom out page content (-10%, down to 25%) | Phase 3 |
| `Ctrl + 0` | Reset zoom level to 100% | Phase 3 |
| `F11` | Toggle window fullscreen mode | Phase 3 |

---

## 5. Privacy & Ephemeral Modes

| Shortcut | Action | Scope / Phase |
|---|---|---|
| `Ctrl + Shift + N` | Open Private Browsing window (stubbed toast; isolated storage arrives in Phase 5) | Phase 5 Stub |
