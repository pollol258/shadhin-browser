# Shadhin Browser — Manual GUI Verification Checklist

This document provides a comprehensive, reproducible checklist for manual user verification of Shadhin Browser features that require direct graphical interaction on Linux (Wayland or X11 / WSLg).

Each test includes step-by-step instructions and a verification blank `[ ] PASS / FAIL`.

---

## 1. Core Window & Theme
- [ ] **Window Shell & Decorations**:
  - Launch `cargo run` or the installed binary.
  - Verify client-side decorated title bar contains minimize, maximize, and close buttons on the top right.
  - Verify dragging the window by empty titlebar regions moves the window smoothly.
  - *Result*: [ ] PASS / [ ] FAIL

- [ ] **Visual Glassmorphism**:
  - Compare visually against `resources/reference/ui-reference.png`.
  - Verify dark burgundy/crimson background gradient on top-left and emerald green accent on bottom-right.
  - Verify web content sits inside the 16px rounded frame (`.shadhin-content-frame`).
  - *Result*: [ ] PASS / [ ] FAIL

---

## 2. Tab Lifecycle & Navigation
- [ ] **Multiple Tabs & Tab Strip**:
  - Click the `+` button in the titlebar. A new tab opens showing the New Tab dashboard.
  - Open 5 tabs. Verify tab buttons resize cleanly, show truncated titles, favicons, and close `✕` buttons.
  - Click `✕` to close tabs. Verify closing the last tab opens a fresh New Tab.
  - *Result*: [ ] PASS / [ ] FAIL

- [ ] **Navigation & Autocomplete**:
  - Focus address bar (Ctrl+L). Type `https://duckduckgo.com` and press Enter. Verify page loads.
  - Click Back (Alt+Left), Forward (Alt+Right), and Reload (Ctrl+R).
  - Verify loading progress bar animates across the bottom of the pill address bar.
  - *Result*: [ ] PASS / [ ] FAIL

- [ ] **Search Engine Query**:
  - In the address bar or New Tab search box, type `rust webkit6` and press Enter.
  - Verify browser navigates to DuckDuckGo search results for the query.
  - *Result*: [ ] PASS / [ ] FAIL

---

## 3. Sidebar Tri-State & Responsive Layout
- [ ] **Sidebar States**:
  - Click the sidebar toggle button in the toolbar or press `Ctrl+B` / `F9`.
  - Verify cycling through:
    1. **Expanded**: ~240px wide with full labels and section headers.
    2. **Compact**: ~56px icon rail. Clicking an icon opens a lightweight popover overlay.
    3. **Hidden**: Sidebar completely collapsed; web content occupies 100% width.
  - *Result*: [ ] PASS / [ ] FAIL

- [ ] **Responsive Breakpoints**:
  - Resize the window width below 900px: verify sidebar collapses automatically to Compact mode.
  - Resize below 650px: verify sidebar collapses to Hidden.
  - Resize back above 900px: verify saved preference (e.g. Expanded) is restored.
  - *Result*: [ ] PASS / [ ] FAIL

---

## 4. Privacy Shield & Security
- [ ] **Privacy Shield Popover**:
  - Click the Shield icon button in the toolbar.
  - Verify popover opens anchored to the button.
  - Verify toggles: Master Switch, Site Shield, Block Trackers, Block Ads, Fingerprinting Protection, HTTPS-Only Mode, Cookie Protection.
  - Toggle "Shield off for this site": verify site label updates to "Shield off for this site" and whitelist is remembered.
  - *Result*: [ ] PASS / [ ] FAIL

- [ ] **Honest Telemetry Indicators**:
  - Open a New Tab.
  - Verify the stats card displays `—` for Trackers Blocked, Ads Blocked, Fingerprints Blocked, and Data Saved.
  - Hover mouse over numbers: verify tooltip reads `"Not measurable on this WebKitGTK version"`.
  - *Result*: [ ] PASS / [ ] FAIL

- [ ] **Private Browsing Window**:
  - Press `Ctrl+Shift+N` or launch `shadhin-browser --private-window`.
  - Verify window opens with a distinct purple "PRIVATE" badge in the title bar.
  - Browse a website in the private window. Close the window.
  - Check History (`Ctrl+H`) in the regular window: verify zero visited pages from the private window are recorded.
  - *Result*: [ ] PASS / [ ] FAIL

---

## 5. Security Interstitials & BadSSL Test Cases
- [ ] **TLS Certificate Warning (Self-Signed / Expired)**:
  - In address bar, enter `https://self-signed.badssl.com/`.
  - Verify warning page appears displaying certificate error and host risk details.
  - Verify NO automatic bypass occurs; "Go Back to Safety" is primary button.
  - Test `https://expired.badssl.com/`: verify TLS warning appears.
  - Test `https://wrong.host.badssl.com/`: verify TLS warning appears.
  - *Result*: [ ] PASS / [ ] FAIL

- [ ] **Dangerous Scheme Rejection**:
  - Type `javascript:alert(1)` into the address bar and press Enter.
  - Verify navigation is rejected and an error page is shown ("Navigation Blocked").
  - Type `file:///etc/passwd` and press Enter: verify blocked.
  - *Result*: [ ] PASS / [ ] FAIL

---

## 6. Workspaces & Storage Isolation
- [ ] **Workspace Creation & Switching**:
  - Click "+ New Workspace" in the sidebar. Enter name "Research" and pick an accent color.
  - Switch to "Work" workspace. Open 2 tabs (`https://example.com`, `https://wikipedia.org`).
  - Switch to "Personal" workspace: verify only Personal tabs are shown.
  - *Result*: [ ] PASS / [ ] FAIL

- [ ] **Workspace Cookie Isolation**:
  - In Workspace "Personal", log into a site or set a test cookie (`document.cookie = "ws=personal"` in web console).
  - Switch to Workspace "Work". Navigate to the same site: verify the cookie is NOT present.
  - *Result*: [ ] PASS / [ ] FAIL

---

## 7. Bookmarks, History & Downloads
- [ ] **Bookmark Management**:
  - Navigate to a webpage and click the star icon in the address bar (or Ctrl+D).
  - Open Bookmarks (`Ctrl+B` or sidebar). Verify bookmark is listed with title and favicon.
  - Export bookmarks (`shadhin-bookmarks.html`). Open in another browser: verify valid HTML.
  - *Result*: [ ] PASS / [ ] FAIL

- [ ] **History & Search**:
  - Press `Ctrl+H` to open History panel.
  - Verify visited URLs, visit timestamps, and page titles are recorded.
  - Filter history using search input. Clear browsing data (`Ctrl+Shift+Del`): verify history is erased.
  - *Result*: [ ] PASS / [ ] FAIL

- [ ] **Download Safety**:
  - Download a test file. Verify download progress appears in Downloads panel.
  - Verify "Open File" is NOT present; only "Show in Folder" is available.
  - Check file permissions in terminal: `ls -l ~/Downloads/<file>` must be `0600` (`-rw-------`).
  - *Result*: [ ] PASS / [ ] FAIL

---

## 8. Bengali / International Typography
- [ ] **Bangla Text Rendering**:
  - Navigate to `https://bn.wikipedia.org/`.
  - Verify Bengali conjuncts (যুক্তবর্ণ), vowel diacritics (কার), and complex scripts render cleanly without tofu blocks or broken glyphs using system Noto fonts.
  - *Result*: [ ] PASS / [ ] FAIL
