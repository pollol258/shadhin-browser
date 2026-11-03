#!/usr/bin/env bash
# scripts/make-icons.sh
# Generates multi-resolution PNG icons from the source SVG.
# Tool used: gdk-pixbuf-thumbnailer (or rsvg-convert / resvg if available).

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(cd "${SCRIPT_DIR}/.." && pwd)"
SVG_SRC="${ROOT_DIR}/data/icons/hicolor/scalable/apps/com.shadhin.browser.svg"
ICONS_BASE="${ROOT_DIR}/data/icons/hicolor"

if [ ! -f "${SVG_SRC}" ]; then
    echo "Error: Source SVG not found at ${SVG_SRC}" >&2
    exit 1
fi

SIZES=(48 64 128 256 512)

# Detect rasterizer tool
RASTERIZER=""
if command -v gdk-pixbuf-thumbnailer >/dev/null 2>&1; then
    RASTERIZER="gdk-pixbuf-thumbnailer"
elif command -v rsvg-convert >/dev/null 2>&1; then
    RASTERIZER="rsvg-convert"
elif command -v resvg >/dev/null 2>&1; then
    RASTERIZER="resvg"
else
    echo "Error: No suitable SVG rasterizer found (need gdk-pixbuf-thumbnailer, rsvg-convert, or resvg)." >&2
    exit 1
fi

echo "Generating icons using: ${RASTERIZER}"

for size in "${SIZES[@]}"; do
    TARGET_DIR="${ICONS_BASE}/${size}x${size}/apps"
    mkdir -p "${TARGET_DIR}"
    TARGET_PNG="${TARGET_DIR}/com.shadhin.browser.png"
    
    if [ "${RASTERIZER}" = "gdk-pixbuf-thumbnailer" ]; then
        gdk-pixbuf-thumbnailer -s "${size}" "${SVG_SRC}" "${TARGET_PNG}"
    elif [ "${RASTERIZER}" = "rsvg-convert" ]; then
        rsvg-convert -w "${size}" -h "${size}" "${SVG_SRC}" -o "${TARGET_PNG}"
    elif [ "${RASTERIZER}" = "resvg" ]; then
        resvg -w "${size}" -h "${size}" "${SVG_SRC}" "${TARGET_PNG}"
    fi
    
    echo "  Generated: ${TARGET_PNG} (${size}x${size})"
done

echo "Icon generation complete."
