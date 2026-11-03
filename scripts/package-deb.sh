#!/usr/bin/env bash
# scripts/package-deb.sh
# Packages Shadhin Browser into a Debian/Ubuntu .deb archive.

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(cd "${SCRIPT_DIR}/.." && pwd)"
DIST_DIR="${ROOT_DIR}/dist"
PKG_DEB_DIR="${ROOT_DIR}/packaging/deb"

cd "${ROOT_DIR}"

if [ -f "$HOME/.cargo/env" ]; then
    # shellcheck disable=SC1091
    source "$HOME/.cargo/env"
fi

echo "========================================================================"
echo "BUILDING DEBIAN / UBUNTU PACKAGE (.deb)"
echo "========================================================================"

# 1. Build release binary
echo "Compiling release binary..."
cargo build --release --locked

# 2. Ensure icons are generated
echo "Verifying icons..."
"${ROOT_DIR}/scripts/make-icons.sh"

# 3. Create staging area
STAGING_DIR="$(mktemp -d -t shadhin_deb_XXXXXX)"
trap 'rm -rf "${STAGING_DIR}"' EXIT
chmod 0755 "${STAGING_DIR}"

mkdir -p "${STAGING_DIR}/DEBIAN"
chmod 0755 "${STAGING_DIR}/DEBIAN"
mkdir -p "${STAGING_DIR}/usr/bin"
mkdir -p "${STAGING_DIR}/usr/share/applications"
mkdir -p "${STAGING_DIR}/usr/share/metainfo"
mkdir -p "${STAGING_DIR}/usr/share/icons/hicolor"
mkdir -p "${STAGING_DIR}/usr/share/man/man1"
mkdir -p "${STAGING_DIR}/usr/share/doc/shadhin-browser"
mkdir -p "${DIST_DIR}"

# 4. Copy files into staging area
echo "Staging binary and metadata files..."
install -m 0755 "${ROOT_DIR}/target/release/shadhin-browser" "${STAGING_DIR}/usr/bin/shadhin-browser"
install -m 0644 "${ROOT_DIR}/data/com.shadhin.browser.desktop" "${STAGING_DIR}/usr/share/applications/com.shadhin.browser.desktop"
install -m 0644 "${ROOT_DIR}/data/com.shadhin.browser.metainfo.xml" "${STAGING_DIR}/usr/share/metainfo/com.shadhin.browser.metainfo.xml"

# Copy hicolor icons tree
cp -r "${ROOT_DIR}/data/icons/hicolor/"* "${STAGING_DIR}/usr/share/icons/hicolor/"
find "${STAGING_DIR}/usr/share/icons/hicolor" -type f -exec chmod 0644 {} +

# Compress and install man page
gzip -9cn "${PKG_DEB_DIR}/usr/share/man/man1/shadhin-browser.1" > "${STAGING_DIR}/usr/share/man/man1/shadhin-browser.1.gz"
chmod 0644 "${STAGING_DIR}/usr/share/man/man1/shadhin-browser.1.gz"

# Install documentation & changelog
install -m 0644 "${PKG_DEB_DIR}/usr/share/doc/shadhin-browser/copyright" "${STAGING_DIR}/usr/share/doc/shadhin-browser/copyright"
gzip -9cn "${ROOT_DIR}/CHANGELOG.md" > "${STAGING_DIR}/usr/share/doc/shadhin-browser/changelog.Debian.gz"
chmod 0644 "${STAGING_DIR}/usr/share/doc/shadhin-browser/changelog.Debian.gz"

# Copy DEBIAN control and maintainer scripts
cp "${PKG_DEB_DIR}/DEBIAN/control" "${STAGING_DIR}/DEBIAN/control"
chmod 0644 "${STAGING_DIR}/DEBIAN/control"

if [ -f "${PKG_DEB_DIR}/DEBIAN/postinst" ]; then
    cp "${PKG_DEB_DIR}/DEBIAN/postinst" "${STAGING_DIR}/DEBIAN/postinst"
    chmod 0755 "${STAGING_DIR}/DEBIAN/postinst"
fi

if [ -f "${PKG_DEB_DIR}/DEBIAN/postrm" ]; then
    cp "${PKG_DEB_DIR}/DEBIAN/postrm" "${STAGING_DIR}/DEBIAN/postrm"
    chmod 0755 "${STAGING_DIR}/DEBIAN/postrm"
fi

# Ensure all directory permissions are strictly 0755
find "${STAGING_DIR}" -type d -exec chmod 0755 {} +

# Calculate accurate Installed-Size in KB
INSTALLED_SIZE_KB=$(du -sk "${STAGING_DIR}/usr" | cut -f1)
sed -i "s/^Installed-Size:.*/Installed-Size: ${INSTALLED_SIZE_KB}/" "${STAGING_DIR}/DEBIAN/control"

# 5. Build .deb with dpkg-deb enforcing root ownership
DEB_OUTPUT="${DIST_DIR}/shadhin-browser_0.1.0_amd64.deb"
echo "Invoking dpkg-deb to package: ${DEB_OUTPUT}..."
dpkg-deb --root-owner-group --build "${STAGING_DIR}" "${DEB_OUTPUT}"

echo "Debian package successfully built:"
ls -lh "${DEB_OUTPUT}"
echo ""
echo "=== Package Control Metadata (dpkg-deb -I) ==="
dpkg-deb -I "${DEB_OUTPUT}"
echo ""
echo "=== Package Content Listing (dpkg-deb -c) ==="
dpkg-deb -c "${DEB_OUTPUT}"
