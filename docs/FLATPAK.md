# Flatpak Packaging and Sandbox Guide

This document describes the Flatpak packaging architecture for Shadhin Browser (`com.shadhin.browser`), its sandboxing permissions, WebKitGTK 6.0 engine sandboxing inside Flatpak, and the Flathub publication workflow.

---

## 1. Sandbox Permissions and Security Rationale

Shadhin Browser is packaged following the principle of least privilege while providing full modern browser capability:

| Finish Argument | Rationale | Security Implication |
|---|---|---|
| `--socket=wayland` | Native Wayland display protocol | Strongly preferred; avoids X11 security concerns (e.g., keylogging, window snooping). |
| `--socket=fallback-x11` | X11 fallback for X11 sessions | Only active when Wayland is unavailable. |
| `--share=ipc` | Shared IPC for X11 / shared memory | Required for Wayland/X11 shared memory buffers (MIT-SHM). |
| `--device=dri` | Direct Rendering Infrastructure | Required for WebKit hardware-accelerated web page rendering (WebGL, canvas). |
| `--socket=pulseaudio` | Audio playback | Required for HTML5 `<audio>` and `<video>` sound output. |
| `--share=network` | Network access | Required for a web browser to fetch HTTP/HTTPS web resources. |
| `--filesystem=xdg-download:rw` | User downloads directory | Restricted to `~/Downloads` only. Browser cannot read general user home directories (`~/.ssh`, `~/Documents`, etc.). |
| `--filesystem=xdg-run/dconf` & `--talk-name=ca.desrt.dconf` | GTK desktop settings | Read desktop dark/light mode preference and font settings without full filesystem access. |
| `--talk-name=org.freedesktop.portal.FileChooser` | Portal File Chooser | Prompts user through host desktop file picker when uploading files or downloading to custom locations; application never has direct filesystem access outside `~/Downloads`. |
| `--talk-name=org.freedesktop.portal.Print` | Portal Printing | Allows native printing to host printers via CUPS/portal without direct printer hardware or daemon access. |
| `--talk-name=org.freedesktop.portal.OpenURI` | External URI opener | Enables opening links in external default applications when requested by the user. |
| `--talk-name=org.freedesktop.portal.FdoStyle` | System theme portal | Allows query of host dark/light preference and accent color. |
| `--talk-name=org.freedesktop.portal.Flatpak` | WebKitGTK sub-sandboxing | Grants permission for WebKit's UI process to request a nested isolated sandbox for `WebKitWebProcess` through the Flatpak portal. |

---

## 2. WebKitGTK 6.0 Sub-Sandboxing inside Flatpak

### How WebKitGTK Operates in Flatpak
On standard host systems (DEB/RPM), WebKitGTK uses Bubblewrap (`bwrap`) directly to sandbox its auxiliary processes (`WebKitWebProcess`, `WebKitNetworkProcess`).

Inside a Flatpak container:
1. An unprivileged container cannot directly invoke `bwrap` because creating user namespaces (`CLONE_NEWUSER`) is typically restricted inside unprivileged containers.
2. WebKitGTK detects that it is executing inside Flatpak by checking for the `/.flatpak-info` file.
3. When running inside Flatpak, WebKitGTK automatically switches to communicating with the host Flatpak portal (`org.freedesktop.portal.Flatpak`) via `flatpak-spawn --sandbox`.
4. The host Flatpak portal spawns each `WebKitWebProcess` in its own nested, tightly constrained Flatpak sub-sandbox with no access to the browser's profile directory (`~/.var/app/com.shadhin.browser`).

This ensures that even if malicious web content achieves remote code execution in WebKitWebProcess, it is trapped inside the sub-sandbox and cannot access the user's cookies, bookmarks, or profile database.

---

## 3. Building the Flatpak Locally

### Prerequisites
Install `flatpak` and `flatpak-builder`:
```bash
# Ubuntu / Debian
sudo apt install flatpak flatpak-builder

# Fedora
sudo dnf install flatpak flatpak-builder
```

Install GNOME 47 Platform runtime and Rust SDK:
```bash
flatpak remote-add --if-not-exists flathub https://dl.flathub.org/repo/flathub.flatpakrepo
flatpak install flathub org.gnome.Platform//47 org.gnome.Sdk//47
flatpak install flathub org.freedesktop.Sdk.Extension.rust-stable//24.08
```

### Build and Package
Run the automated packaging script:
```bash
./scripts/package-flatpak.sh
```

Or run `flatpak-builder` manually:
```bash
# 1. Generate vendored crates list
python3 packaging/flatpak/flatpak-cargo-generator.py Cargo.lock -o packaging/flatpak/cargo-sources.json

# 2. Build and export to local repo
flatpak-builder --force-clean --user --install-deps-from=flathub \
    --repo=dist/flatpak/repo dist/flatpak/build packaging/flatpak/com.shadhin.browser.yml

# 3. Create standalone bundle
flatpak build-bundle dist/flatpak/repo dist/com.shadhin.browser.flatpak com.shadhin.browser
```

### Installing and Running the Local Build
```bash
# Install local build into user Flatpak storage
flatpak-builder --user --install --force-clean dist/flatpak/build packaging/flatpak/com.shadhin.browser.yml

# Run the installed Flatpak application
flatpak run com.shadhin.browser

# Run with URL
flatpak run com.shadhin.browser https://example.com
```

---

## 4. Flathub Submission Checklist

Before submitting to [Flathub](https://github.com/flathub/flathub):

- [x] **Application ID**: Unique reverse-DNS identifier `com.shadhin.browser`.
- [x] **AppStream Metainfo**: Validated with `appstreamcli validate --no-net --pedantic data/com.shadhin.browser.metainfo.xml` (0 errors, 0 warnings).
- [x] **Desktop File**: Validated with `desktop-file-validate data/com.shadhin.browser.desktop`.
- [x] **Icons**: Scalable SVG (`data/icons/hicolor/scalable/apps/com.shadhin.browser.svg`) and symbolic SVG (`data/icons/hicolor/symbolic/apps/com.shadhin.browser-symbolic.svg`).
- [x] **Offline Vendored Cargo**: All 110 crate dependencies vendored via `cargo-sources.json` with SHA256 checksums (no network access during build phase).
- [x] **License**: Project licensed under GPL-3.0-or-later; metainfo license `CC0-1.0` or `FSFAP`.
- [ ] **Upstream Repository**: Replace placeholder repository URL with real public Git repository hosting tagged releases before submitting Flathub pull request.
