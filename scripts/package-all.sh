#!/usr/bin/env bash
# scripts/package-all.sh
# Master packaging script for Shadhin Browser 0.1.0.
# Orchestrates release compilation, icon generation, .deb, Flatpak, and .rpm builds,
# performs host-path leak audits, and computes distribution SHA256 checksums.

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(cd "${SCRIPT_DIR}/.." && pwd)"
DIST_DIR="${ROOT_DIR}/dist"

cd "${ROOT_DIR}"

if [ -f "$HOME/.cargo/env" ]; then
    # shellcheck disable=SC1091
    source "$HOME/.cargo/env"
fi

echo "========================================================================"
echo "SHADHIN BROWSER 0.1.0 — MASTER PACKAGING & DISTRIBUTION PIPELINE"
echo "========================================================================"

mkdir -p "${DIST_DIR}"

# 1. Compile release binary with path remapping for reproducible builds
echo "==> Step 1: Compiling release binary with path remapping..."
RUSTFLAGS="--remap-path-prefix=${ROOT_DIR}=/build --remap-path-prefix=${HOME}=/cargo" \
    cargo build --release --locked

# 2. Audit binary for host path / username leaks
echo "==> Step 2: Running binary privacy and host-leak audit..."
if strings "${ROOT_DIR}/target/release/shadhin-browser" | grep -E "${USER}|/home/${USER}" &>/dev/null; then
    echo "WARNING: Host username or personal paths found in release binary!"
else
    echo "SUCCESS: Host path leak check passed. No personal paths or username found in binary."
fi

# 3. Generate icon assets
echo "==> Step 3: Generating all icon assets..."
"${ROOT_DIR}/scripts/make-icons.sh"

# 4. Build Debian package (.deb)
echo "==> Step 4: Packaging Debian/Ubuntu .deb..."
if command -v dpkg-deb &>/dev/null; then
    "${ROOT_DIR}/scripts/package-deb.sh"
else
    echo "SKIPPED: dpkg-deb not found. Install dpkg to package Debian archives."
fi

# 5. Build Flatpak (.flatpak)
echo "==> Step 5: Packaging Flatpak bundle..."
if command -v flatpak-builder &>/dev/null; then
    "${ROOT_DIR}/scripts/package-flatpak.sh"
else
    echo "NOTICE: flatpak-builder not installed on host. Skipping Flatpak build."
    echo "        See docs/FLATPAK.md for instructions on building the Flatpak container."
fi

# 6. Build RPM (.rpm)
echo "==> Step 6: Packaging Fedora/RHEL .rpm..."
if command -v rpmbuild &>/dev/null; then
    "${ROOT_DIR}/scripts/package-rpm.sh"
else
    echo "NOTICE: rpmbuild not installed on host. Skipping RPM build."
    echo "        See docs/PACKAGING.md for instructions on building the RPM package."
fi

# 7. Compute SHA256 checksums
echo "==> Step 7: Generating distribution SHA256 checksums..."
cd "${DIST_DIR}"
rm -f SHA256SUMS
if compgen -G "*.deb" > /dev/null || compgen -G "*.rpm" > /dev/null || compgen -G "*.flatpak" > /dev/null; then
    sha256sum *.* > SHA256SUMS
    echo "SHA256SUMS generated successfully:"
    cat SHA256SUMS
else
    echo "No release artifacts found in ${DIST_DIR}."
fi

echo ""
echo "========================================================================"
echo "PACKAGING PIPELINE COMPLETE"
echo "Distribution directory contents:"
ls -lh "${DIST_DIR}"
echo "========================================================================"
