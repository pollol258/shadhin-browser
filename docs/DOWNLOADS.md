# Download Manager & File Safety Specification

## 1. Overview
Shadhin Browser implements a robust, hardened download management system built upon WebKitGTK 6.0 and GTK4 / Libadwaita. It intercepts network downloads from user clicks, direct file URLs, `Content-Disposition: attachment` responses, and JavaScript triggers, while enforcing filename sanitization, collision evasion, non-executable filesystem permissions (`0600`), live progress tracking, and persistent SQLite history.

## 2. Root Cause Analysis & WebKitGTK 6.0 Architecture

Prior to this implementation, downloads in Shadhin Browser failed due to two architectural root causes:

1. **Absolute Path Requirement (`g_path_is_absolute`)**:
   In `webkit_download_set_destination`, WebKitGTK 6.0 internally asserts `g_return_if_fail(g_path_is_absolute(destination))`. Passing a URI like `file:///home/user/Downloads/...` causes a GLib critical warning (`g_path_is_absolute("file://...") == FALSE`), aborting the download destination assignment. WebKitGTK requires a raw absolute UNIX filesystem path (e.g. `/home/user/Downloads/file.pdf`), NOT a `file://` URI.

2. **Policy Decision Response Delegation**:
   When a web server returns an unrenderable binary MIME type (e.g. `.zip`, `.iso`, `.exe`, `.tar.gz`) or a header `Content-Disposition: attachment`, WebKitGTK emits `WebView::decide-policy` with `PolicyDecisionType::Response`. Previously, `decide-policy` only handled `PolicyDecisionType::NavigationAction`. By defaulting unhandled responses to false without downcasting to `ResponsePolicyDecision` and calling `decision.download()`, WebKitGTK cancelled the navigation with `WebKitPolicyError::CannotShowMimeType` ("Cannot show MIME type") instead of starting a download.

## 3. Filename Safety Sanitizer (`sanitize_download_filename`)

Web servers can provide hostile, arbitrary `Content-Disposition` filenames attempting path traversal or executable injection. Shadhin Browser sanitizes all suggested filenames:

1. **Path Traversal Neutralization**:
   - Both Unix `/` and Windows `\` path separators are normalized and stripped.
   - Basename extraction ensures paths like `../../../../etc/passwd` or `..\..\..\windows\system32\calc.exe` are collapsed strictly to their leaf filename (`passwd` / `calc.exe`).
2. **Hidden File Prevention**:
   - Leading dots (`.`) are stripped so downloaded files cannot overwrite Unix configuration files (such as `.bashrc`, `.profile`, or `.ssh`).
3. **Illegal Character Filtering**:
   - Strips ASCII control characters (`\x00` - `\x1F`), NUL bytes, colons (`:`), quotes (`"`), wildcards (`*`, `?`), redirection pipes (`<`, `>`, `|`), and backslashes.
4. **Length Truncation**:
   - Maximum byte limit: 240 bytes (well within the Linux ext4 255-byte filename limit).
   - Truncation safely preserves multi-byte UTF-8 boundaries and retains the file extension (e.g. `.tar.gz`).
5. **Fallback**:
   - If sanitization results in an empty string or whitespace, it safely defaults to `download`.

## 4. Collision Evasion (`resolve_collision_path`)

If a file with the target name already exists in the destination directory:
- Instead of silently overwriting user data, an incremental numeric suffix is appended before the file extension: `filename (1).ext`, `filename (2).ext`, etc.
- The UI, database records, and filesystem state are all kept in synchronization with the resolved collision path.

## 5. Non-Executable File Permissions (`0600`)

Upon download completion (`DownloadStatus::Completed`), `set_safe_file_permissions` sets the file mode to `0600` (`-rw-------`). Executable permissions (`+x`) are stripped, ensuring that downloaded scripts, binaries, and AppImages cannot be executed accidentally.

## 6. Download Controls & Execution Policy

The Downloads panel provides clear controls:
- **Cancel**: Available for active downloads. Invokes `webkit_download_cancel()`.
- **Retry**: Available for failed, cancelled, or interrupted downloads. Re-initiates the download via `session.download_uri()`.
- **Open File**: Available for completed downloads. Uses the desktop environment's registered default handler via GLib `gio::AppInfo::launch_default_for_uri("file://...")`. The browser never invokes system shells or direct `exec` on untrusted files.
- **Show in Folder**: Opens the containing directory in the system file manager (`file://<dir>`).
- **Delete**: Cleans up the record from history without deleting the user's file.

> [!NOTE]
> WebKitGTK 6.0 does not provide pause or resume APIs in its C library (`WebKitDownload` only provides `cancel()`). Consequently, pause/resume is not implemented; Cancel and Retry represent the correct lifecycle in WebKitGTK 6.0.

## 7. Metrics & Live UI Integration

- **Progress**: Computed as `(received_bytes / total_bytes)` when `total_bytes > 0`. When `Content-Length` is absent (streamed transfers), the progress bar enters indeterminate pulsing mode.
- **Speed**: Calculated from `received_bytes / elapsed_time` and rendered as B/s, KB/s, or MB/s.
- **ETA**: Calculated from `(total_bytes - received_bytes) / speed`.
- **Toast Notifications**: When a download completes in the background, an Adwaita `Toast` notification appears with an `[Open]` button.
- **Persistence**: Managed through the SQLite `DownloadRepository`. Lingering `downloading` states are cleanly marked `interrupted` if the browser is closed or restarts.
