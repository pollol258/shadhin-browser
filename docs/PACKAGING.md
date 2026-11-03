# Packaging Guide and Architecture

This document details the packaging infrastructure, distribution channels, and reproducibility standards for Shadhin Browser 0.1.0.

---

## 1. Packaging Layout

The repository maintains native packaging assets in `debian/` (standard Debian layout), `packaging/`, and automation scripts in `scripts/`:

```
debian/
├── control                       # Canonical Debian source & binary package definitions
├── changelog                     # Formatted changelog with release versions & distribution
├── copyright                     # Machine-readable Debian copyright (DEP-5)
└── rules                         # Debian makefile rules (dh $@, build, install)

packaging/
├── deb/
│   ├── DEBIAN/
│   │   ├── control               # Package control file, dependencies, metadata
│   │   ├── postinst              # Post-installation script (icon/desktop cache refresh)
│   │   └── postrm                # Post-removal script (cache cleanup)
│   └── usr/
│       ├── share/
│       │   ├── doc/shadhin-browser/
│       │   │   └── copyright     # Machine-readable Debian copyright (DEP-5)
│       │   └── man/man1/
│       │       └── shadhin-browser.1 # Standard Unix man page
├── flatpak/
│   ├── com.shadhin.browser.yml   # Flatpak manifest targeting GNOME 47 runtime
│   ├── flatpak-cargo-generator.py # Python script parsing Cargo.lock for offline vendor
│   └── cargo-sources.json        # 110 vendored crate sources with SHA256 hashes
└── rpm/
    └── shadhin-browser.spec      # RPM spec file targeting Fedora 40/41

scripts/
├── build-deb.sh                  # 10-step Debian packaging pipeline with dpkg-shlibdeps
├── ci-check.sh                   # Comprehensive CI gate (fmt, clippy, test, audit, deny, validate)
├── make-icons.sh                 # Generates 48, 64, 128, 256, 512 PNGs from SVG
├── package-deb.sh                # Builds .deb package with dpkg-deb
├── package-flatpak.sh            # Builds Flatpak bundle with flatpak-builder
├── package-rpm.sh                # Builds .rpm package with rpmbuild
└── package-all.sh                # Master script orchestrating all packaging and checksums
```

---

## 2. Reproducibility & Security Standards

To satisfy security audit and clean-room packaging rules:

1. **Path Remapping**:
   Rust binaries embed source code paths in panic locations by default. To prevent host environment leakage (usernames, build directories):
   ```bash
   RUSTFLAGS="--remap-path-prefix=${PWD}=/build --remap-path-prefix=${HOME}=/cargo" cargo build --release --locked
   ```
2. **File Permissions**:
   - Staged package directories are strictly `0755` (`rwxr-xr-x`).
   - The compiled executable `/usr/bin/shadhin-browser` is strictly `0755`.
   - Data files, desktop entries, and documentation are strictly `0644` (`rw-r--r--`).
   - Private runtime directories (`~/.local/share/shadhin/`) created by the browser are enforced at runtime to `0700` (`rwx------`), and SQLite DB / config files are `0600` (`rw-------`).
3. **No Embedded Secrets / Telemetry**:
   - Zero analytics endpoints, tracking SDKs, or reporting beacons.
   - Zero hardcoded passwords, tokens, API keys, or private developer paths.

---

## 3. Package Formats

### Debian / Ubuntu (`.deb`)
- Built using native `dpkg-deb` with `--root-owner-group`.
- Strict dependencies:
  - `libc6 (>= 2.38)`
  - `libgtk-4-1 (>= 4.10.0)`
  - `libadwaita-1-0 (>= 1.4.0)`
  - `libwebkitgtk-6.0-4 (>= 2.42.0)`
  - `libsqlite3-0 (>= 3.35.0)`
- Output artifact: `dist/shadhin-browser_0.1.0_amd64.deb`

### Flatpak (`.flatpak`)
- Manifest: `packaging/flatpak/com.shadhin.browser.yml`
- Runtime: `org.gnome.Platform//47` with `org.freedesktop.Sdk.Extension.rust-stable`
- Full offline build compliance with 110 vendored crates.
- Output artifact: `dist/com.shadhin.browser.flatpak`

### RPM (`.rpm`)
- Spec file: `packaging/rpm/shadhin-browser.spec`
- Target: Fedora 40 / 41 / RHEL 10
- Output artifact: `dist/shadhin-browser-0.1.0-1.x86_64.rpm`

---

## 4. Building Packages with Master Script

Run the master packaging script to produce all available packages and compute cryptographic hashes:
```bash
./scripts/package-all.sh
```

All built artifacts and their SHA-256 checksums are placed into the `dist/` directory:
- `dist/shadhin-browser_0.1.0_amd64.deb`
- `dist/SHA256SUMS`

Verify checksums anytime:
```bash
cd dist && sha256sum -c SHA256SUMS
```

---

## 5. Clean-Room Container Verification (Docker / Podman)

If a clean Docker or Podman host is available (outside restricted sandboxes):

### Testing Debian Package on Clean Ubuntu 24.04:
```bash
docker run --rm -v "$(pwd)/dist:/dist:ro" -it ubuntu:24.04 bash -c '
    apt update && \
    apt install -y /dist/shadhin-browser_0.1.0_amd64.deb && \
    shadhin-browser --version && \
    shadhin-browser --help
'
```

### Testing RPM Package on Clean Fedora 40:
```bash
podman run --rm -v "$(pwd)/dist:/dist:ro" -it fedora:40 bash -c '
    dnf install -y /dist/*.rpm && \
    shadhin-browser --version && \
    shadhin-browser --help
'
```
