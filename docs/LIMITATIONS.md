# Shadhin Browser — Technical Limitations & Design Decisions

This document records the exact architectural constraints, platform limitations, and honest trade-offs in Shadhin Browser.

---

## 1. UI & Styling Limitations

### 1.1 Lack of `backdrop-filter` in GTK4 CSS
- **Limitation**: GTK4 CSS does not implement the CSS `backdrop-filter: blur(...)` property. Live dynamic Gaussian blur of content behind a window or surface is not supported by GTK's GSK (GTK Scene Kit) CSS engine.
- **Design Resolution**: Glassmorphism is achieved honestly via translucent background RGBA fills, fine diagonal gradients, subtle light top borders (`border-top: 1px solid rgba(255, 255, 255, 0.15)`), soft drop-shadows, and curated color palettes. Fake static blurs or rasterized snapshots behind widgets are deliberately avoided.

### 1.2 Titlebar Tab Strip Integration (`AdwTabBar` vs Custom Strip)
- **Limitation**: `AdwTabBar` is designed for full-width, standalone tab bars with built-in window dragging behaviors, pinned tabs, and scroll controls that clash with compact client-side decorated title bars containing custom brand logos and trailing window controls.
- **Design Resolution**: A custom `ShadhinTabStrip` component binds directly to the underlying `AdwTabView` model. It replicates the reference design's rounded tabs, lighter active tab highlighting, title truncation, close buttons (`✕`), and a trailing `+` button, while allowing natural window dragging via `GtkWindowHandle`.

### 1.3 Popover & Floating Glass Transparency
- **Limitation**: GTK4 popovers on Wayland / WSLg rely on compositor subsurfaces. Full translucency on popover backgrounds without a compositor blur filter can lead to background text bleed-through if transparency is too high.
- **Design Resolution**: The Privacy Shield popover uses a high-opacity dark wine surface (`rgba(34, 20, 32, 0.95)`) with crisp borders (`rgba(255, 255, 255, 0.16)`) and deep drop shadows, preserving legibility while remaining harmonious with the glassmorphism theme.

### 1.4 Content Frame Rounded Corner Clipping under WSLg
- **Limitation**: In GTK4, CSS `overflow: hidden` is not a recognized CSS property. Instead, container clipping is handled by GSK rounded clip nodes on widget boundaries. In WebKitGTK 6.0 under WSLg (using shared-memory rendering via `WEBKIT_DISABLE_DMABUF_RENDERER=1`), GSK correctly applies rounded border clipping to native GTK widgets (such as the New Tab dashboard and error pages). If GPU-accelerated Wayland subsurfaces bypass GSK clipping on specific host graphics drivers, web content may render with square corners inside the 16px rounded frame.
- **Design Resolution**: The `.shadhin-content-frame` wraps the active view with `border-radius: 16px`, `margin: 6px 12px 12px 8px`, and an emerald accent border (`rgba(41, 184, 118, 0.38)`), exposing the outer diagonal gradient around the frame while clipping native views cleanly.

### 1.5 Window Controls & Decoration Layout on WSLg
- **Limitation**: Under WSLg and Ubuntu GNOME defaults, `gtk-decoration-layout` defaults to `:close` (only the close button is displayed by default on client-side decorations).
- **Design Resolution**: The titlebar explicitly sets `.decoration_layout(":minimize,maximize,close")` on `GtkWindowControls` and configures `GtkSettings::set_gtk_decoration_layout(Some(":minimize,maximize,close"))`, ensuring minimize, maximize, and close buttons are always rendered and operational.

### 1.6 Sidebar Tri-State & Responsive Breakpoints
- **Design & Layout**: Shadhin Browser provides three distinct sidebar states: `Expanded` (~240px full rail with labels), `Compact` (~56px icon rail with tooltips and popover overlays), and `Hidden` (fully collapsed, 100% content width).
- **Responsiveness**: Pure function `compute_effective_sidebar_state` and Libadwaita `AdwBreakpoint` automatically collapse the sidebar below 900px (to Compact) and below 650px (to Hidden) without mutating the saved user preference in `config.toml`. When the window is resized back to >= 900px, the saved preference is restored immediately.
- **Overlay Panels**: Clicking icons in compact mode presents lightweight Popover overlays anchored directly to the clicked icon, dismissable automatically on outside clicks or pressing `Escape`.

---

## 2. WebKitGTK Engine & Privacy Metrics

### 2.1 Content Filter Telemetry & Block Counting
- **Limitation**: `WebKitUserContentFilterStore` compiles rule sets directly into WebKit's internal bytecode format. WebKitGTK processes subresource filtering within the sandboxed web engine and does not emit a public signal (`resource_load_started`, `connect_failed`, or WebProcess notification) for blocked subresources (scripts, images, stylesheets, beacons) to the UI process.
- **Empirical Proof (`tests/empirical_filter_test.rs`)**:
  - Control run without filter: Local HTTP server receives requests for all subresources (`/test_page.html`, `/content/normal.js`, `/ads/banner.js`, `/track/pixel.gif`).
  - Filtered run with filter: Local HTTP server receives only allowed resources (`/test_page.html`, `/content/normal.js`). Blocked subresources (`/ads/banner.js`, `/track/pixel.gif`) are stopped at the engine level and never reach the server.
  - Signal observability: WebKitGTK 6.0 emits zero signals to the UI process for blocked subresources.
- **Design Resolution**:
  - Because subresource block counts cannot be observed without introducing a secondary filter or proxy layer, Shadhin Browser refuses to synthesize or fabricate numbers.
  - The New Tab stats indicators display `—` for Trackers Blocked, Ads Blocked, Fingerprints Blocked, and Data Saved with the exact tooltip `"Not measurable on this WebKitGTK version"`.
  - The Privacy Shield popover exposes active protection toggles and site-specific shields rather than misleading numeric gauges.

### 2.2 Fingerprint Blocking
- **Limitation**: WebKitGTK provides canvas protection and header normalization options in newer releases, but does not provide complete immunity against advanced behavioral or hardware canvas/WebGL fingerprinting.
- **Design Resolution**: The setting is described as **Fingerprinting Protection (standard/limited)**. Counters are not artificially incremented; only actual blocked fingerprinting script events or blocked API probes are reflected.

### 2.3 "Data Saved" Metric
- **Limitation**: Because WebKitGTK drops filtered network requests before content headers (and `Content-Length`) are downloaded over the wire, the precise byte size of prevented downloads cannot be known without external estimations.
- **Design Resolution**: Shadhin Browser displays `—` for Data Saved unless an exact known resource length is available, avoiding fictitious megabyte totals.

### 2.4 Find in Page Active Index Exposure
- **Limitation**: WebKitGTK 6.0's `FindController` emits `counted-matches` (reporting total matches found) and `failed-to-find-text`, but does not provide an API property or signal reporting the 1-based index of the currently highlighted match (e.g. "3 of 12").
- **Design Resolution**: The FindBar displays honest match totals ("X matches", "1 match", or "No matches") rather than estimating or fabricating an internal active index.

### 2.5 TLS Error Handling & Bypass Protection
- **Limitation**: Untrusted TLS certificates (self-signed, expired, domain mismatches) compromise transport layer privacy.
- **Design Resolution**: `connect_load_failed_with_tls_errors` presents a styled internal warning screen displaying the target host and risk details. Silent downgrades and automatic bypasses are strictly prevented. "Go Back to Safety" is the primary action.

### 2.6 Real Site Favicons in WebKitGTK 6.0
- **Limitation**: In WebKitGTK 6.0, `WebsiteDataManager::favicons_enabled` defaults to `false`. Without explicit configuration, WebKit never downloads or caches favicon images, causing `WebView::favicon()` to always return `None`.
- **Design Resolution**: During WebKit webview allocation in `create_configured_webview_with_session`, Shadhin Browser explicitly invokes `wdm.set_favicons_enabled(true)`. The `connect_favicon_notify` signal binds directly to the tab model (`Rc<RefCell<Tab>>`), dynamically refreshing tab buttons and the sidebar open tabs list with decoded `gdk::Texture` paintables and falling back to generic icons only when no favicon exists.

---

## 3. Host System Build Requirements
- Building native GTK4 + Libadwaita + WebKitGTK 6.0 in Rust requires Linux system development headers (`libgtk-4-dev`, `libadwaita-1-dev`, `libwebkitgtk-6.0-dev`).
- Running or cross-compiling on native Windows requires an MSYS2 or WSL2 environment with these packages provisioned.
- **Verified Installed System Library Versions (Ubuntu 24.04 WSL2)**:
  - `gtk4`: `4.14.5`
  - `libadwaita-1`: `1.5.0`
  - `webkitgtk-6.0`: `2.52.6`

---

## 4. Running on WSL2/WSLg

### 4.1 Mesa/Zink Virtual Driver & DMABUF Rendering
- **Limitation**: In WSLg environments, virtual GPU translation through Mesa/Zink can report EGL initialization warnings (`MESA: error: ZINK: failed to choose pdev`, `libEGL warning: egl: failed to create dri2 screen`). WebKitGTK 6.0's DMABUF renderer may also encounter inter-process surface sharing issues between the sandboxed WebProcess and the UI process depending on host GPU drivers.
- **Working Variables / Workarounds**:
  - `WEBKIT_DISABLE_DMABUF_RENDERER=1`: Falls back to shared memory (SHM) buffers for inter-process rendering, resolving black/blank views while retaining full engine functionality.
  - `WEBKIT_DISABLE_COMPOSITING_MODE=1`: Disables accelerated compositing as a secondary fallback if hardware rasterization fails.
- **Security Requirement**: WebKit's process sandbox (`bubblewrap`) must NEVER be disabled (`WEBKIT_FORCE_SANDBOX=0` is strictly forbidden). Working variables must only tune graphics/display protocols, leaving isolation and security boundaries untouched.

---

## 5. Storage, Downloads & Workspace Design Decisions (Phase 4)

### 5.1 SQLite Thread Safety & Concurrency
- **Limitation**: Running SQLite write transactions directly on the GTK main thread causes Wayland frame stalls and perceptible micro-freezes during disk I/O.
- **Design Resolution**: All database interactions are offloaded to a background thread via `StorageService`. Results are returned to the GTK main loop using GLib main context invocation, ensuring the UI remains fluid at 60+ FPS.

### 5.2 Download Safety, Execution Policy & WebKitGTK 6.0 Pause/Resume
- **Limitation**: Automatic execution of downloaded untrusted binaries poses severe security risks. Furthermore, WebKitGTK 6.0 C API for `WebKitDownload` provides `cancel()`, but does not implement pause or resume capabilities.
- **Design Resolution**: Shadhin Browser enforces strict file permissions (`0600`), sanitizes all filenames against directory traversal (`../../`), strips illegal filesystem characters, and detects dangerous executable file extensions (`.sh`, `.desktop`, `.AppImage`, `.exe`). Download controls honestly provide Cancel and Retry (avoiding fake pause/resume toggles). "Open File" securely delegates opening to the system default application launcher via GLib `gio::AppInfo::launch_default_for_uri`, while "Show in Folder" opens the enclosing directory in the system file manager. The browser never runs binaries directly via shell or `Command::new`.

### 5.3 Per-Workspace Storage Isolation
- **Limitation**: Sharing cookies, localStorage, or cache between workspaces compromises organizational compartmentalization (e.g. personal vs. work).
- **Design Resolution**: Each workspace (`Personal`, `Work`, `Project`) allocates an independent WebKit `NetworkSession` pointing to isolated `0700` directories in XDG data and cache locations (`sessions/ws_<id>`). Switching workspaces dynamically updates the active session for the window and download manager, ensuring complete isolation across cookie jars, HTTP cache, and HTML5 localStorage. In Private mode, an ephemeral in-memory session is strictly enforced.

### 5.4 WebKitGTK NetworkSession Subdirectory Permissions & Root `0700` Defense
- **Limitation**: WebKitGTK internal network session directories are created using the process umask (resulting in `0755` permissions for internal folders such as `applications/`, `HSTS/`, and `localstorage/`). WebKit's C API does not expose a parameter to specify explicit POSIX octal modes for internal subdirectories.
- **Design Resolution**: Shadhin Browser enforces strict `0700` (`rwx------`) permissions on the root app data directory (`~/.local/share/shadhin-browser/`) and sessions directory (`~/.local/share/shadhin-browser/sessions/`). Under POSIX access controls, unprivileged other users cannot traverse into or list any subdirectory within a `0700` parent directory, regardless of the child directory's permissions. This guarantees complete multi-user privacy on shared workstations.

---

## 6. Performance & Resource Limitations (Phase 6)

### 6.1 WebKitGTK Multi-Process Base Memory Overhead
- **Limitation**: WebKitGTK employs a modern multi-process security architecture where each web page renders in an isolated `WebKitWebProcess` running inside a bubblewrap container. The baseline PSS memory footprint of a WebKitWebProcess is approximately 30–85 MB depending on page complexity and graphics drivers. This is inherent to the WebKit engine and cannot be eliminated without compromising process isolation or security sandboxing.
- **Design Resolution**: Shadhin Browser addresses this via lazy WebView allocation (avoiding WebProcess creation on New Tab) and proactive tab discarding (releasing the WebView and WebProcess when background tabs remain idle for > 5 minutes while retaining tab metadata for seamless restoration).

### 6.2 WSLg Graphics and Compositor Translation
- **Limitation**: WSLg runs on top of Microsoft's WSL-Wayland compositor, which bridges Linux Wayland/X11 surfaces to the Windows DirectX/Desktop Window Manager. In virtualized GPU environments, frame presentation can introduce higher context switches and rendering latency compared to bare-metal Linux.
- **Design Resolution**: The release build enables thin LTO and symbol stripping to reduce binary size to 3.9 MB, disables in-memory page caching to prevent RAM bloating, and runs all database access on a dedicated worker thread to maintain responsive 60 FPS interactions.
