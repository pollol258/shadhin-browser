#!/usr/bin/env bash
# scripts/package-flatpak.sh
# Builds the Shadhin Browser Flatpak package using flatpak-builder.

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(cd "${SCRIPT_DIR}/.." && pwd)"
DIST_DIR="${ROOT_DIR}/dist"
FLATPAK_DIR="${ROOT_DIR}/packaging/flatpak"

cd "${ROOT_DIR}"

echo "========================================================================"
echo "BUILDING FLATPAK PACKAGE"
echo "========================================================================"

# Check for required tools
if ! command -v flatpak-builder &>/dev/null; then
    echo "ERROR: 'flatpak-builder' command not found."
    echo "To build flatpak packages, install flatpak and flatpak-builder:"
    echo "  sudo apt install flatpak flatpak-builder  # Debian / Ubuntu"
    echo "  sudo dnf install flatpak flatpak-builder  # Fedora"
    echo ""
    echo "Also install the GNOME 47 runtime and Rust SDK extension:"
    echo "  flatpak install flathub org.gnome.Platform//47 org.gnome.Sdk//47"
    echo "  flatpak install flathub org.freedesktop.Sdk.Extension.rust-stable//24.08"
    exit 1
fi

# 1. Generate cargo-sources.json
echo "Generating vendored Cargo sources JSON..."
python3 "${FLATPAK_DIR}/flatpak-cargo-generator.py" "${ROOT_DIR}/Cargo.lock" -o "${FLATPAK_DIR}/cargo-sources.json"

# 2. Prepare build and repo directories
BUILD_DIR="${DIST_DIR}/flatpak/build"
REPO_DIR="${DIST_DIR}/flatpak/repo"
mkdir -p "${BUILD_DIR}" "${REPO_DIR}"

# 3. Execute flatpak-builder
echo "Running flatpak-builder..."
flatpak-builder \
    --force-clean \
    --user \
    --install-deps-from=flathub \
    --default-branch=stable \
    --repo="${REPO_DIR}" \
    "${BUILD_DIR}" \
    "${FLATPAK_DIR}/com.shadhin.browser.yml"

# 4. Create standalone single-file .flatpak bundle
BUNDLE_OUTPUT="${DIST_DIR}/com.shadhin.browser.flatpak"
echo "Exporting single-file Flatpak bundle: ${BUNDLE_OUTPUT}..."
flatpak build-bundle "${REPO_DIR}" "${BUNDLE_OUTPUT}" com.shadhin.browser stable

echo "Flatpak build complete:"
ls -lh "${BUNDLE_OUTPUT}"
