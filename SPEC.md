# Shadhin Browser — Project Specification

## 0. HOW YOU MUST WORK (READ FIRST)

You are an autonomous coding agent working inside a real workspace with a terminal.

- Create and edit files DIRECTLY in the workspace. Do NOT paste full source code into chat.
- Start every phase by producing an **Implementation Plan** and a **Task List** artifact. Then execute.
- Run every verification and build command yourself. Report the REAL output, never an assumed one.
- If a crate version, crate API, or system library detail is uncertain, check docs.rs, crates.io, and the installed headers/typelibs on this machine BEFORE writing code. Never guess or invent APIs.
- Keep the project compiling after every task. Run `cargo fmt`, `cargo clippy`, `cargo check`, and `cargo test` regularly.
- Work on ONE phase at a time. When the phase is done, STOP and give a short report: what was built, what was verified (with real command output), what failed, known limitations. Wait for my approval before the next phase.
- Do not mark any feature complete until it is implemented AND tested.
- Write install steps and build/run/test commands into `README.md` instead of repeating them in chat.

## 1. PROJECT

**Name:** Shadhin Browser (স্বাধীন ব্রাউজার; "Shadhin" means free/independent)
**Application ID:** com.shadhin.browser
**Tagline:** Fast · Private · Open
**Goal:** a real, working, production-oriented native Linux web browser. NOT Electron, NOT a web app, NOT a static UI prototype.

**Mandatory stack**
- Rust (stable), Cargo
- GTK4 + libadwaita (use `gtk4` and `libadwaita` Rust crates)
- WebKitGTK 6.0 via the `webkit6` crate (never WebKit2GTK 4.x, never GTK3)
- GLib/GIO main loop for async; Tokio only where truly needed
- SQLite (rusqlite or sqlx, choose one and justify), Serde, tracing
- Config: TOML
- Styling: GTK CSS (loaded from `resources/styles/`, bundled with GResource)
- Icons: Adwaita/Freedesktop symbolic icons; custom SVG icons only if original or license-compatible (record licenses)

Official references:
- https://docs.rs/webkit6/latest/webkit6/
- https://gtk-rs.org/gtk4-rs/stable/latest/book/
- https://gtk-rs.org/gtk4-rs/stable/latest/book/libadwaita.html
- https://webkitgtk.org/reference/webkit2gtk/unstable/migrating-to-webkitgtk-6.0.html

## 2. UI SPECIFICATION (MUST MATCH THE ATTACHED REFERENCE IMAGE)

The attached image `Falcon_Browser_Privacy_Dashboard.png` is the visual target. (The image still shows the old working name "Falcon Browser"; replace ALL branding text with "Shadhin Browser" in the real UI.) Save the image to `resources/reference/ui-reference.png` and compare against it throughout. Sample exact colors from it and put them in one CSS variables block (`@define-color`) so the whole theme is changeable from one place.

Important: the bottom strip of the image ("Work Plan" and "Key Features") is presentation material, NOT part of the browser UI. Do not build it.

### 2.1 Overall look
- Dark, glassmorphism-inspired theme. Force the dark libadwaita color scheme.
- Background: diagonal gradient from burgundy/red (top-left, sidebar, title area) to emerald green (bottom-right, content frame).
- Translucent panels (rgba), rounded corners (about 12–18 px on panels, pill shape on inputs), subtle gradients, soft borders, smooth transitions.
- GTK4 CSS has NO `backdrop-filter`. Do not fake real blur. Achieve the glass look with translucency, gradients, borders, and shadows. Document this limitation.
- Window uses client-side decorations: tabs live inside the title bar (as in the image), with minimize/maximize/close on the right.

### 2.2 Title bar row (top)
- Left: Shadhin logo mark + text "Shadhin Browser".
- Tabs: rounded tab shapes. Active tab is lighter/translucent with a close (×) button. Each tab shows favicon (or a home icon for New Tab), title, close button.
- "+" new tab button after the tabs, then a separator.
- Window controls on the far right.

### 2.3 Toolbar row
- Back, Forward, Reload/Stop (Reload becomes Stop while loading).
- Large pill-shaped address bar with: privacy shield icon on the left (shows site security state), placeholder "Search or enter website address", bookmark star on the right. Loading progress shown as a thin bar inside/under the address bar.
- Right of the address bar: Privacy Shield button, Profile button, Main menu (hamburger).

### 2.4 Left sidebar (burgundy, translucent)
- Items with icons: Home (active item has a highlighted rounded background), Bookmarks, History, Downloads, Tabs (with an expand/collapse chevron that shows a vertical tab list).
- Divider, then a "Workspaces" section: Personal (pink folder), Work (yellow folder), Project (green folder), and "+ New Workspace".
- "Settings" pinned at the bottom.
- Sidebar should be collapsible (use AdwOverlaySplitView or equivalent; verify the API).

### 2.5 Main content area
- Rounded frame containing the real WebKitGTK view.
- When the tab is "New Tab", show the New Tab page (see 2.6). Otherwise show the web content.
- Loading indicator, error page (custom, styled to match theme, for network errors and TLS errors), context menu, find-in-page bar.

### 2.6 New Tab page
- Shadhin logo (create an ORIGINAL SVG mark in a red/coral gradient that expresses freedom/openness; do NOT reuse the falcon-wing shape from the reference image) with "Shadhin Browser" title and "Fast · Private · Open" below.
- Centered pill search box "Search the web privately" using the selected search engine.
- Row of 6 shortcut tiles: GitHub, YouTube, Wikipedia, Reddit, Linux, and an "Add Shortcut" tile. Shortcuts are user-editable and stored in the database. Do NOT bundle third-party brand logos; fetch favicons at runtime through WebKit's favicon facilities, with a neutral symbolic fallback icon.
- Recently visited sites (from real history).
- Wallpaper: configurable. Ship a gradient default and a free/CC0 or self-generated wallpaper only. Do not copy the mountain photo from the reference image. User can pick their own image in Settings.
- Bottom "Privacy Shield" stats bar with four items: Trackers Blocked, Ads Blocked, Fingerprints Blocked, Data Saved. See honesty rules in 2.8.
- New Tab page may be built as local HTML/CSS served through a safe internal mechanism, or as native GTK widgets. Choose one, justify it, and make sure internal pages cannot be abused by web content.

### 2.7 Privacy Shield popover (top-right, opened by the shield button)
- Header "Privacy Shield" with a master on/off switch.
- Rows with status indicators: Block Trackers, Block Ads, Block Fingerprinting, HTTPS-Only, Cookie Protection.
- Actions: Private Window (Ctrl+Shift+N), Clear Browsing Data… (Ctrl+Shift+Del), Settings.
- Each row must be a real toggle bound to a real setting, not a decoration.

### 2.8 HONESTY RULES FOR PRIVACY UI (STRICT)
The reference image shows sample numbers (1,248 / 87 / 23 / 400 MB). These are mockup values.
- Every counter must come from real events. Fresh install shows 0.
- Trackers/Ads Blocked: count only resources actually blocked by the content filter or navigation policy, where WebKit exposes a way to observe it. If WebKitGTK cannot report per-request block events for the content filter, say so clearly, find the closest verifiable method, and document the limitation. Do not fabricate.
- Fingerprints Blocked and the "Block Fingerprinting" row: implement ONLY protections you can verify on this WebKitGTK version. If nothing is measurable, rename the row to something accurate (for example "Fingerprinting Protection (limited)"), show no invented count, and explain in docs. Never claim fingerprinting is fully blocked.
- Data Saved: show a value only if it can be computed from real blocked-resource sizes; otherwise show "—" and document why.
- Never auto-bypass TLS errors. Never silently downgrade HTTPS to HTTP.

## 3. FEATURES (build in the phases of section 5)

**Foundation:** native window, WebView, URL navigation with scheme validation, search engine integration, back/forward, reload/stop, title and favicon updates, loading progress.

**Tabs:** multiple tabs, new/close/duplicate/reorder/switch, titles + favicons, active indicator, keyboard shortcuts, session restore (optional setting), lazy/discarded background tabs to save memory.

**Privacy and security (real WebKitGTK capabilities only):** tracker and ad blocking via WebKit content filters (WebKitUserContentFilterStore) with maintained filter lists converted to WebKit's JSON format, HTTPS-only with safe fallback and a clear user prompt, cookie policy controls, third-party cookie restriction and ITP where WebKitGTK supports it (verify and document), JavaScript toggle, permission management (camera, microphone, location, notifications), popup blocking, referrer controls where supported, clear browsing data, private windows (ephemeral WebsiteDataManager / NetworkSession, nothing persisted), privacy dashboard.

**User features:** bookmarks + folders, history, download manager (never auto-execute downloads), search engine settings, homepage settings, custom shortcuts, workspaces (separate tab groups and optionally separate data), theme and wallpaper settings, bookmark import/export, find in page.

Private mode must not claim anonymity from websites or network operators.

## 4. ARCHITECTURE AND RULES

Use a modular layout (you may improve it):

```
shadhin-browser/
├── Cargo.toml  Cargo.lock  README.md  LICENSE  SPEC.md
├── src/
│   ├── main.rs  app.rs  config.rs
│   ├── browser/   (webview, navigation, tabs, downloads, permissions, session)
│   ├── privacy/   (adblock, tracker, cookies, https_only, stats)
│   ├── ui/        (window, titlebar, toolbar, sidebar, tabbar, addressbar, newtab, settings, privacy_panel)
│   ├── storage/   (database, bookmarks, history, shortcuts, workspaces)
│   └── utils/     (logging)
├── resources/  (icons, styles, images, reference, gresource xml)
├── data/       (desktop file, metainfo, app icon)
├── tests/
└── packaging/
```

Security and quality rules:
- Safe Rust; avoid `unsafe` unless unavoidable and then justify it in a comment.
- Keep WebKit's sandbox and process isolation enabled. Never disable security to gain speed.
- Validate all URL schemes; reject dangerous custom schemes. Never execute downloaded files.
- Store data in XDG directories (`~/.config/shadhin-browser`, `~/.local/share/shadhin-browser`, `~/.cache/shadhin-browser`) with restrictive file permissions.
- No telemetry. No API keys or secrets in source.
- Never block the GTK main thread; no unnecessary database writes or WebViews.
- Clear error handling (`thiserror`/`anyhow`), `tracing` for logs.
- No TODO placeholders for mandatory features. No pseudocode.
- Write `docs/THREAT_MODEL.md` and `docs/LIMITATIONS.md` (update them as you go).
- Do not promise RAM or CPU numbers without measuring. Write `docs/PERFORMANCE.md` with a repeatable measurement procedure.

Platform targets: primary Debian/Ubuntu (state the minimum release that ships WebKitGTK 6.0 dev packages; verify), secondary Fedora, Arch, openSUSE. GNOME and KDE. Packaging later: .deb, .rpm, Flatpak; AppImage only if WebKitGTK compatibility can be handled correctly (never bundle incompatible WebKitGTK libs).

## 5. PHASES (STOP AFTER EACH ONE AND WAIT FOR MY APPROVAL)

- **PHASE 1 — Environment and architecture**
- **PHASE 2 — UI shell matching the reference image** (theme, title bar, tabs UI, toolbar, address bar, sidebar, New Tab page, Privacy Shield popover; with static/mock data clearly marked as dev-only)
- **PHASE 3 — Browser engine** (real WebKit browsing, navigation, multiple tabs wired to the UI)
- **PHASE 4 — Browser functionality** (bookmarks, history, downloads, search engines, workspaces, sessions)
- **PHASE 5 — Privacy and security** (content filters, HTTPS-only, cookies, private windows, permissions, real counters)
- **PHASE 6 — Optimization** (profiling, DB tuning, tab lifecycle)
- **PHASE 7 — Testing and packaging** (unit, integration, security tests, .deb, .rpm, Flatpak, install docs)

## 6. YOUR FIRST TASK: PHASE 1 ONLY

1. Save this prompt as `SPEC.md` and the attached image as `resources/reference/ui-reference.png`.
2. Verify the environment by running and reporting real output:
   `rustc --version`, `cargo --version`, `cargo search webkit6`, `cargo search libadwaita`, `cargo search gtk4`,
   `pkg-config --modversion gtk4`, `pkg-config --modversion libadwaita-1`, `pkg-config --modversion webkitgtk-6.0`,
   and detect the distro (`/etc/os-release`).
3. If any system dependency is missing, tell me the exact install command for Debian/Ubuntu, Fedora, and Arch. Do not proceed with guessed versions.
4. Choose mutually compatible crate versions (gtk4, libadwaita, webkit6 must share compatible gtk4-rs/glib versions) based on what you verified, and create `Cargo.toml` (package name `shadhin-browser`).
5. Create the project directory structure from section 4 (empty modules with proper `mod.rs` are fine, no fake features).
6. Write a minimal libadwaita application with application ID `com.shadhin.browser`, one window titled "Shadhin Browser", and ONE real `webkit6::WebView` loading a URL, to prove the binding works.
7. Run `cargo fmt`, `cargo clippy`, `cargo build`, then launch the app and confirm that it opens and renders a page. Report the real output.
8. Write `README.md` with system dependencies per distro, build, run, and test commands.
9. Report: what works, what was verified, what failed, API surprises in `webkit6`, known limitations. Then STOP and wait for approval.
