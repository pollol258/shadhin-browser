# Installation and Setup Guide

This guide describes how to install, build, and configure Shadhin Browser (স্বাধীন ব্রাউজার) on Linux distributions.

---

## 1. System Requirements

- **Operating System**: Linux (Kernel 5.15+ recommended)
- **Architecture**: x86_64, aarch64
- **Display Server**: Wayland (recommended) or X11
- **Core Libraries**:
  - GTK 4 (>= 4.10.0)
  - Libadwaita (>= 1.4.0)
  - WebKitGTK 6.0 (>= 2.42.0) — *Note: WebKitGTK 4.x is not supported*
  - SQLite (>= 3.35.0)
  - Bubblewrap (`bwrap`) for WebKit process isolation

---

## 2. Package Installation

### Option A: Debian / Ubuntu (24.04 LTS+)
Download the prebuilt `.deb` package from the release assets:
```bash
# Install the package and automatically resolve dependencies
sudo apt update
sudo apt install ./shadhin-browser_0.1.0_amd64.deb
```

*Note for Debian 12 (Bookworm):* Debian 12 does not ship WebKitGTK 6.0 in its main repository. For Debian 12, the Flatpak installation method below is recommended.

### Option B: Fedora / RHEL (Fedora 40+)
Download the prebuilt `.rpm` package:
```bash
sudo dnf install ./shadhin-browser-0.1.0-1.fc40.x86_64.rpm
```

### Option C: Flatpak (Any Linux Distribution)
Install the standalone single-file Flatpak bundle:
```bash
flatpak install ./com.shadhin.browser.flatpak
```
Or once published on Flathub:
```bash
flatpak install flathub com.shadhin.browser
```

To run the Flatpak version:
```bash
flatpak run com.shadhin.browser
```

---

## 3. Building From Source

### Install Build Dependencies

#### Ubuntu 24.04 / Debian Unstable (Trixie/Sid)
```bash
sudo apt update
sudo apt install -y \
    build-essential \
    curl \
    pkg-config \
    libgtk-4-dev \
    libadwaita-1-dev \
    libwebkitgtk-6.0-dev \
    libsqlite3-dev \
    appstream-util
```

#### Fedora 40 / 41
```bash
sudo dnf install -y \
    gcc \
    pkg-config \
    gtk4-devel \
    libadwaita-devel \
    webkitgtk6.0-devel \
    sqlite-devel \
    libappstream-glib
```

#### Arch Linux
```bash
sudo pacman -Syu --needed \
    base-devel \
    rust \
    cargo \
    gtk4 \
    libadwaita \
    webkit6gtk \
    sqlite
```

### Install Rust Toolchain
Ensure you have Rust 1.80 or newer installed:
```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
source "$HOME/.cargo/env"
rustc --version
```

### Compile and Run
```bash
# Clone repository
git clone https://github.com/placeholder-user/shadhin-browser.git
cd shadhin-browser

# Run all test suites
cargo test

# Build optimized release binary
cargo build --release --locked

# Run the browser
./target/release/shadhin-browser
```

### System-Wide Manual Installation
```bash
# Install binary
sudo install -m 0755 target/release/shadhin-browser /usr/local/bin/

# Install desktop entry and icons
sudo install -m 0644 data/com.shadhin.browser.desktop /usr/local/share/applications/
sudo install -m 0644 data/com.shadhin.browser.metainfo.xml /usr/local/share/metainfo/
sudo cp -r data/icons/hicolor/* /usr/local/share/icons/hicolor/

# Update desktop and icon databases
sudo update-desktop-database /usr/local/share/applications
sudo gtk4-update-icon-cache -q -t -f /usr/local/share/icons/hicolor
```

---

## 4. Default Browser Registration

To set Shadhin Browser as your system default browser:

### Via Command Line (`xdg-settings` / `xdg-mime`)
```bash
# Set as default web browser
xdg-settings set default-web-browser com.shadhin.browser.desktop

# Associate HTTP and HTTPS URL schemes
xdg-mime default com.shadhin.browser.desktop x-scheme-handler/http
xdg-mime default com.shadhin.browser.desktop x-scheme-handler/https

# Associate HTML file types
xdg-mime default com.shadhin.browser.desktop text/html
xdg-mime default com.shadhin.browser.desktop application/xhtml+xml

# Verify current default
xdg-settings get default-web-browser
```

### Via Desktop Settings GUI
1. Open **GNOME Settings** (or your desktop environment's settings panel).
2. Navigate to **Apps** -> **Default Apps**.
3. Under **Web**, select **Shadhin Browser**.

---

## 5. Troubleshooting & Environment Quirks

### WSL2 / WSLg Rendering
In WSL2 environments with older Mesa/vGPU drivers, hardware DMABUF acceleration may cause WebKit rendering glitches or crashes. If you experience rendering issues:
```bash
WEBKIT_DISABLE_DMABUF_RENDERER=1 shadhin-browser
```

### Headless / CI Testing
When executing automated test suites in continuous integration or headless containers:
```bash
# Run tests with headless backend
xvfb-run cargo test
```
The test suite automatically skips WebKit WebView rendering tests gracefully if no display is detected.
