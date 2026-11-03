# Storage Architecture & Persistence Specification

## 1. Storage Location and Permissions
Shadhin Browser persists user bookmarks, browsing history, downloads metadata, search engines, new tab shortcuts, and workspaces in a local SQLite database adhering strictly to XDG Directory specifications:

- **Root App Data Directory**: `~/.local/share/shadhin-browser/` (Mode `0700` = `rwx------`)
- **Database File**: `~/.local/share/shadhin-browser/shadhin.db` (Mode `0600` = `rw-------`)
- **WAL Files**: `shadhin.db-wal` and `shadhin.db-shm` inherit the strict `0600` permissions.
- **Sessions Directory**: `~/.local/share/shadhin-browser/sessions/` (Mode `0700` = `rwx------`)

Both directory and database permissions are checked and enforced at application startup.

### 1.1 WebKit Subdirectories & Top-Level `0700` Root Barrier
When WebKitGTK's internal `NetworkSession` initializes, WebKit's engine creates subdirectories (such as `applications/`, `HSTS/`, `localstorage/`) using standard system umask permissions (often `0755` = `rwxr-xr-x`).
Under POSIX filesystem access controls, path traversal requires search/execute (`x`) permission on **every enclosing parent directory** leading to a target. Because the top-level directory `~/.local/share/shadhin-browser/` is strictly locked to mode `0700` (`rwx------`, owner-only):
- Any other user or unprivileged process on the operating system attempting to access, list, or traverse any WebKit-created subdirectory receives an immediate `EACCES (Permission denied)` at the root `shadhin-browser/` boundary.
- All internal site data, cookies, and local storage remain completely inaccessible to other local users.

## 2. SQLite Configuration & PRAGMAs
To guarantee high responsiveness without stalling the GTK main loop or Wayland frame compositor, the database is opened with the following integrity and concurrency PRAGMAs:

```sql
PRAGMA journal_mode = WAL;
PRAGMA foreign_keys = ON;
PRAGMA synchronous = NORMAL;
PRAGMA temp_store = MEMORY;
PRAGMA busy_timeout = 5000;
```

- **WAL (Write-Ahead Logging)**: Readers never block writers, and writers never block readers.
- **Foreign Keys**: Enabled to enforce relational integrity and cascade deletions across folders, workspaces, and session tabs.
- **Synchronous NORMAL**: Eliminates redundant disk fsync stalls while remaining crash-safe in WAL mode.
- **Temp Store MEMORY**: Intermediate sort tables and indices reside in memory.
- **Busy Timeout (5000ms)**: Gracefully handles transaction wait times.

## 3. Asynchronous Worker Architecture
All database access is executed off the GTK main thread via `StorageService`:
- Dedicated OS thread named `shadhin-storage-worker`.
- Work submitted via thread-safe MPSC channel.
- Asynchronous query results delivered to the main GTK context using GLib thread-local callback invocation (`glib::MainContext::invoke`), eliminating race conditions and UI freezing.

## 4. Forward-Only Migrations
Schema changes are managed via the `schema_version` table:
- Migration V1: Initial schema creating `bookmark_folders`, `bookmarks`, `history`, `downloads`, `shortcuts`, `search_engines`, `workspaces`, and `session_tabs`.
- Seeds 6 built-in search engines, 5 default new tab shortcut tiles, and 3 default workspaces (`Personal`, `Work`, `Project`).

## 5. Private Mode Groundwork
All storage mutations accept a `persistence_allowed: bool` parameter. When set to `false`:
- History visits are rejected immediately without DB queries.
- Download records are kept in memory only without disk persistence.
- Session tabs are never written to disk.
Unit tests verify zero database writes when `persistence_allowed` is `false`.
