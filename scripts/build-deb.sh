#!/usr/bin/env bash
# scripts/build-deb.sh
# Complete, reproducible Debian packaging automation for Shadhin Browser.
# Builds the release binary, audits dependencies via dpkg-shlibdeps,
# stages all desktop integration files, and packages into dist/deb/*.deb.

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(cd "${SCRIPT_DIR}/.." && pwd)"
DIST_DEB_DIR="${ROOT_DIR}/dist/deb"

cd "${ROOT_DIR}"

# Ensure cargo is in PATH
if [ -f "$HOME/.cargo/env" ]; then
    # shellcheck disable=SC1091
    source "$HOME/.cargo/env"
fi

echo "========================================================================"
echo "SHADHIN BROWSER — DEBIAN (.deb) PACKAGE BUILD SYSTEM"
echo "========================================================================"

# -----------------------------------------------------------------------------
# STEP 1: Verify Required Packaging Tools
# -----------------------------------------------------------------------------
echo "[1/10] Verifying packaging prerequisites..."
REQUIRED_TOOLS=("cargo" "rustc" "dpkg-deb" "gzip")
for tool in "${REQUIRED_TOOLS[@]}"; do
    if ! command -v "${tool}" >/dev/null 2>&1; then
        echo "ERROR: Required tool '${tool}' is not installed or not in PATH." >&2
        exit 1
    fi
done
echo "  ✔ Prerequisites verified (cargo, rustc, dpkg-deb, gzip present)"

# Extract canonical version from Cargo.toml
VERSION=$(grep -m1 '^version =' "${ROOT_DIR}/Cargo.toml" | cut -d '"' -f2)
ARCH=$(dpkg --print-architecture 2>/dev/null || echo "amd64")
echo "  ℹ Package: shadhin-browser"
echo "  ℹ Version: ${VERSION}-1"
echo "  ℹ Architecture: ${ARCH}"

# -----------------------------------------------------------------------------
# STEP 2: Code Formatting Check
# -----------------------------------------------------------------------------
echo "[2/10] Running cargo fmt --check..."
if cargo fmt --check; then
    echo "  ✔ Code formatting is compliant"
else
    echo "ERROR: Code formatting check failed! Run 'cargo fmt' to fix." >&2
    exit 1
fi

# -----------------------------------------------------------------------------
# STEP 3: Compiler & Clippy Linter Check
# -----------------------------------------------------------------------------
echo "[3/10] Running cargo clippy --all-targets -- -D warnings..."
if cargo clippy --all-targets -- -D warnings; then
    echo "  ✔ Clippy linter passed with zero warnings"
else
    echo "ERROR: Clippy linter failed!" >&2
    exit 1
fi

# -----------------------------------------------------------------------------
# STEP 4: Automated Test Suite Execution
# -----------------------------------------------------------------------------
echo "[4/10] Running cargo test..."
if WEBKIT_DISABLE_DMABUF_RENDERER=1 cargo test; then
    echo "  ✔ All test suites passed successfully"
else
    echo "ERROR: Test suite failed!" >&2
    exit 1
fi

# -----------------------------------------------------------------------------
# STEP 5: Release Compilation
# -----------------------------------------------------------------------------
echo "[5/10] Compiling optimized release binary (cargo build --release --locked)..."
cargo build --release --locked
if [ ! -f "${ROOT_DIR}/target/release/shadhin-browser" ]; then
    echo "ERROR: Release binary target/release/shadhin-browser was not produced!" >&2
    exit 1
fi
echo "  ✔ Release binary compiled successfully"

# Ensure icon assets are generated and up to date
if [ -f "${ROOT_DIR}/scripts/make-icons.sh" ]; then
    echo "  ℹ Synchronizing multi-resolution icons..."
    "${ROOT_DIR}/scripts/make-icons.sh"
fi

# -----------------------------------------------------------------------------
# STEP 6: Stage Application Files
# -----------------------------------------------------------------------------
echo "[6/10] Staging application files into packaging root..."
STAGING_DIR="$(mktemp -d -t shadhin_deb_stage_XXXXXX)"
trap 'rm -rf "${STAGING_DIR}"' EXIT

mkdir -p "${STAGING_DIR}/DEBIAN"
mkdir -p "${STAGING_DIR}/usr/bin"
mkdir -p "${STAGING_DIR}/usr/share/applications"
mkdir -p "${STAGING_DIR}/usr/share/metainfo"
mkdir -p "${STAGING_DIR}/usr/share/icons/hicolor"
mkdir -p "${STAGING_DIR}/usr/share/man/man1"
mkdir -p "${STAGING_DIR}/usr/share/doc/shadhin-browser"
mkdir -p "${DIST_DEB_DIR}"

# 1. Install release binary (0755)
install -m 0755 "${ROOT_DIR}/target/release/shadhin-browser" "${STAGING_DIR}/usr/bin/shadhin-browser"

# 2. Install desktop entries (0644)
install -m 0644 "${ROOT_DIR}/data/com.shadhin.browser.desktop" "${STAGING_DIR}/usr/share/applications/com.shadhin.browser.desktop"
# Provide compatibility desktop filename as requested by packaging specifications
ln -sf com.shadhin.browser.desktop "${STAGING_DIR}/usr/share/applications/shadhin-browser.desktop"

# 3. Install AppStream metadata (0644)
install -m 0644 "${ROOT_DIR}/data/com.shadhin.browser.metainfo.xml" "${STAGING_DIR}/usr/share/metainfo/com.shadhin.browser.metainfo.xml"

# 4. Install hicolor icon theme tree (0644)
if [ -d "${ROOT_DIR}/data/icons/hicolor" ]; then
    cp -r "${ROOT_DIR}/data/icons/hicolor/"* "${STAGING_DIR}/usr/share/icons/hicolor/"
    # Provide symlinks for shadhin-browser icon name
    for res_dir in "${STAGING_DIR}/usr/share/icons/hicolor/"*; do
        if [ -d "${res_dir}/apps" ]; then
            if [ -f "${res_dir}/apps/com.shadhin.browser.png" ]; then
                ln -sf com.shadhin.browser.png "${res_dir}/apps/shadhin-browser.png"
            elif [ -f "${res_dir}/apps/com.shadhin.browser.svg" ]; then
                ln -sf com.shadhin.browser.svg "${res_dir}/apps/shadhin-browser.svg"
            fi
        fi
    done
    find "${STAGING_DIR}/usr/share/icons/hicolor" -type f -exec chmod 0644 {} +
fi

# 5. Install compressed man page (0644)
if [ -f "${ROOT_DIR}/packaging/deb/usr/share/man/man1/shadhin-browser.1" ]; then
    gzip -9cn "${ROOT_DIR}/packaging/deb/usr/share/man/man1/shadhin-browser.1" > "${STAGING_DIR}/usr/share/man/man1/shadhin-browser.1.gz"
    chmod 0644 "${STAGING_DIR}/usr/share/man/man1/shadhin-browser.1.gz"
fi

# 6. Install documentation & changelogs (0644)
if [ -f "${ROOT_DIR}/debian/copyright" ]; then
    install -m 0644 "${ROOT_DIR}/debian/copyright" "${STAGING_DIR}/usr/share/doc/shadhin-browser/copyright"
elif [ -f "${ROOT_DIR}/packaging/deb/usr/share/doc/shadhin-browser/copyright" ]; then
    install -m 0644 "${ROOT_DIR}/packaging/deb/usr/share/doc/shadhin-browser/copyright" "${STAGING_DIR}/usr/share/doc/shadhin-browser/copyright"
fi

if [ -f "${ROOT_DIR}/CHANGELOG.md" ]; then
    gzip -9cn "${ROOT_DIR}/CHANGELOG.md" > "${STAGING_DIR}/usr/share/doc/shadhin-browser/changelog.Debian.gz"
    chmod 0644 "${STAGING_DIR}/usr/share/doc/shadhin-browser/changelog.Debian.gz"
fi

if [ -f "${ROOT_DIR}/README.md" ]; then
    gzip -9cn "${ROOT_DIR}/README.md" > "${STAGING_DIR}/usr/share/doc/shadhin-browser/README.gz"
    chmod 0644 "${STAGING_DIR}/usr/share/doc/shadhin-browser/README.gz"
fi

# -----------------------------------------------------------------------------
# STEP 7: Generate Shared Library Dependencies (dpkg-shlibdeps)
# -----------------------------------------------------------------------------
echo "[7/10] Analyzing shared library dependencies via dpkg-shlibdeps..."
SHLIB_DEPS=""
if command -v dpkg-shlibdeps >/dev/null 2>&1; then
    TMP_SHLIB_DIR="$(mktemp -d -t shadhin_shlib_XXXXXX)"
    mkdir -p "${TMP_SHLIB_DIR}/debian"
    cat <<EOF > "${TMP_SHLIB_DIR}/debian/control"
Source: shadhin-browser
Section: web
Priority: optional
Maintainer: Shadhin Browser Contributors <placeholder@example.com>

Package: shadhin-browser
Architecture: ${ARCH}
Description: Shadhin Browser
EOF
    SHLIB_OUT=$( (cd "${TMP_SHLIB_DIR}" && dpkg-shlibdeps -O "${STAGING_DIR}/usr/bin/shadhin-browser" 2>/dev/null) || true)
    rm -rf "${TMP_SHLIB_DIR}"

    SHLIB_DEPS=$(echo "${SHLIB_OUT}" | sed -n 's/^shlibs:Depends=//p' | tr -d '\n')
fi

# Verified fallback dependencies if dpkg-shlibdeps could not resolve
if [ -z "${SHLIB_DEPS}" ]; then
    echo "  ⚠ dpkg-shlibdeps returned empty; utilizing verified Ubuntu 24.04 runtime dependencies"
    SHLIB_DEPS="libadwaita-1-0 (>= 1.4~beta), libc6 (>= 2.34), libgcc-s1 (>= 4.2), libglib2.0-0t64 (>= 2.54.0) | libglib2.0-0 (>= 2.54.0), libgtk-4-1 (>= 4.9.2), libpango-1.0-0 (>= 1.14.0), libsoup-3.0-0 (>= 3.0.3), libwebkitgtk-6.0-4 (>= 2.41.4)"
else
    echo "  ✔ Successfully computed shlibs: ${SHLIB_DEPS}"
fi

# Combine with runtime platform tools (sandbox, mime, tls, icons)
RUNTIME_RECOMMENDS="bubblewrap, xdg-utils, shared-mime-info, ca-certificates, hicolor-icon-theme, fonts-noto-core, fonts-noto-bengali"
FINAL_DEPENDS="${SHLIB_DEPS}, bubblewrap, xdg-utils, shared-mime-info, ca-certificates, hicolor-icon-theme"

# Compute accurate Installed-Size in KB
INSTALLED_SIZE_KB=$(du -sk "${STAGING_DIR}/usr" | cut -f1)

# Generate DEBIAN/control
cat <<EOF > "${STAGING_DIR}/DEBIAN/control"
Package: shadhin-browser
Version: ${VERSION}-1
Section: web
Priority: optional
Architecture: ${ARCH}
Maintainer: Shadhin Browser Contributors <placeholder@example.com>
Depends: ${FINAL_DEPENDS}
Recommends: fonts-noto-core, fonts-noto-bengali
Installed-Size: ${INSTALLED_SIZE_KB}
Homepage: https://github.com/placeholder-user/shadhin-browser
Description: Fast, private, and open native Linux web browser
 Shadhin Browser (স্বাধীন ব্রাউজার) is a fast, privacy-respecting native
 Linux web browser built with Rust, GTK4, Libadwaita, and WebKitGTK 6.0.
 .
 Features:
  * Native Wayland and X11 rendering via GTK4 and Libadwaita.
  * Real multi-process WebKitGTK 6.0 engine with bubblewrap sandboxing.
  * Native tracker and ad-blocking using WebKitUserContentFilterStore.
  * Isolated workspaces with dedicated cookie and cache compartmentalization.
  * Strict telemetry-free architecture and verified privacy metrics.
EOF

chmod 0644 "${STAGING_DIR}/DEBIAN/control"
chmod 0755 "${STAGING_DIR}/DEBIAN"
find "${STAGING_DIR}" -type d -exec chmod 0755 {} +

# -----------------------------------------------------------------------------
# STEP 8: Build the Debian Package (.deb)
# -----------------------------------------------------------------------------
DEB_PACKAGE_NAME="shadhin-browser_${VERSION}-1_${ARCH}.deb"
FINAL_DEB_PATH="${DIST_DEB_DIR}/${DEB_PACKAGE_NAME}"

echo "[8/10] Building .deb archive: ${FINAL_DEB_PATH}..."
dpkg-deb --root-owner-group --build "${STAGING_DIR}" "${FINAL_DEB_PATH}"

# Also update symlink or copy to dist/ for legacy build references
mkdir -p "${ROOT_DIR}/dist"
cp -f "${FINAL_DEB_PATH}" "${ROOT_DIR}/dist/${DEB_PACKAGE_NAME}"

echo "  ✔ Debian package successfully built"

# -----------------------------------------------------------------------------
# STEP 9: Validate Package Using dpkg-deb & Lintian
# -----------------------------------------------------------------------------
echo "[9/10] Validating package contents and control headers..."

echo ""
echo "--- dpkg-deb -I (Control Metadata) ---"
dpkg-deb -I "${FINAL_DEB_PATH}"

echo ""
echo "--- dpkg-deb -c (Contents Sample) ---"
dpkg-deb -c "${FINAL_DEB_PATH}" > "${STAGING_DIR}/deb-contents.txt"
head -n 30 "${STAGING_DIR}/deb-contents.txt"
TOTAL_ENTRIES=$(wc -l < "${STAGING_DIR}/deb-contents.txt")
echo "  [... ${TOTAL_ENTRIES} total staged entries ...]"

if command -v lintian >/dev/null 2>&1; then
    echo ""
    echo "--- Lintian Quality Audit ---"
    lintian --info --display-info "${FINAL_DEB_PATH}" || true
else
    echo "  ℹ lintian not installed on host; skipping lintian audit"
fi

# -----------------------------------------------------------------------------
# STEP 10: Display Package Summary & Checksums
# -----------------------------------------------------------------------------
echo ""
echo "[10/10] Final Package Deliverable:"
echo "========================================================================"
ls -lh "${FINAL_DEB_PATH}"
echo "SHA-256 Checksum:"
sha256sum "${FINAL_DEB_PATH}"
echo "MD5 Checksum:"
md5sum "${FINAL_DEB_PATH}"
echo "========================================================================"
echo "DEBIAN PACKAGE GENERATION COMPLETE: ${FINAL_DEB_PATH}"
