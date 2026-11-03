#!/usr/bin/env bash
# scripts/package-rpm.sh
# Packages Shadhin Browser into an RPM package (.rpm) for Fedora / RHEL.

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(cd "${SCRIPT_DIR}/.." && pwd)"
DIST_DIR="${ROOT_DIR}/dist"
RPM_SPEC="${ROOT_DIR}/packaging/rpm/shadhin-browser.spec"

cd "${ROOT_DIR}"

echo "========================================================================"
echo "BUILDING RPM PACKAGE"
echo "========================================================================"

if ! command -v rpmbuild &>/dev/null; then
    echo "ERROR: 'rpmbuild' command not found."
    echo "To build RPM packages, install rpm-build tools:"
    echo "  sudo dnf install rpm-build rpmdevtools  # Fedora / RHEL"
    echo "  sudo apt install rpm                    # Debian / Ubuntu"
    exit 1
fi

RPM_TOPDIR="$(mktemp -d -t shadhin_rpm_XXXXXX)"
trap 'rm -rf "${RPM_TOPDIR}"' EXIT

mkdir -p "${RPM_TOPDIR}"/{BUILD,RPMS,SOURCES,SPECS,SRPMS}
mkdir -p "${DIST_DIR}"

VERSION="0.1.0"
NAME="shadhin-browser"
TARBALL="${NAME}-${VERSION}.tar.gz"

echo "Creating source archive ${TARBALL}..."
git archive --format=tar.gz --prefix="${NAME}-${VERSION}/" HEAD > "${RPM_TOPDIR}/SOURCES/${TARBALL}"

echo "Copying spec file..."
cp "${RPM_SPEC}" "${RPM_TOPDIR}/SPECS/"

echo "Running rpmbuild..."
rpmbuild --define "_topdir ${RPM_TOPDIR}" -ba "${RPM_TOPDIR}/SPECS/shadhin-browser.spec"

echo "Copying generated RPMs to ${DIST_DIR}..."
find "${RPM_TOPDIR}/RPMS" -name "*.rpm" -exec cp {} "${DIST_DIR}/" \;
find "${RPM_TOPDIR}/SRPMS" -name "*.rpm" -exec cp {} "${DIST_DIR}/" \;

echo "RPM build complete. Generated packages:"
ls -lh "${DIST_DIR}"/*.rpm 2>/dev/null || true
