#!/usr/bin/env bash
# scripts/ci-check.sh
# Comprehensive Continuous Integration verification script for Shadhin Browser.
# Runs code formatting, clippy linter, unit & security tests, dependency audits,
# desktop entry validation, and AppStream metadata checks.
# Exits with non-zero status code on any failure.

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(cd "${SCRIPT_DIR}/.." && pwd)"
cd "${ROOT_DIR}"

# Ensure cargo is available
if [ -f "$HOME/.cargo/env" ]; then
    # shellcheck disable=SC1091
    source "$HOME/.cargo/env"
fi

echo "========================================================================"
echo "SHADHIN BROWSER — CONTINUOUS INTEGRATION PRE-FLIGHT CHECK"
echo "========================================================================"

FAILED_STEPS=()

# 1. Code Formatting
echo "[1/6] Running cargo fmt --check..."
if cargo fmt --check; then
    echo "  ✔ Formatting check passed"
else
    echo "  ✘ Formatting check failed! Run 'cargo fmt' to fix." >&2
    FAILED_STEPS+=("cargo fmt")
fi

# 2. Clippy Linter
echo "[2/6] Running cargo clippy -- -D warnings..."
if cargo clippy -- -D warnings; then
    echo "  ✔ Clippy check passed (zero warnings)"
else
    echo "  ✘ Clippy check failed!" >&2
    FAILED_STEPS+=("cargo clippy")
fi

# 3. Test Suite (Default Parallel Mode)
echo "[3/6] Running cargo test (all unit, integration, and security tests)..."
if cargo test; then
    echo "  ✔ All tests passed successfully"
else
    echo "  ✘ Test suite failed!" >&2
    FAILED_STEPS+=("cargo test")
fi

# 4. Dependency Security Audit
echo "[4/6] Running cargo audit..."
ADVISORY_DB_ARGS=()
if [ -d "$HOME/.cargo/advisory-db" ]; then
    ADVISORY_DB_ARGS=(--db "$HOME/.cargo/advisory-db" --no-fetch)
fi

if cargo audit "${ADVISORY_DB_ARGS[@]}"; then
    echo "  ✔ Cargo audit passed (zero known vulnerabilities)"
else
    echo "  ✘ Cargo audit detected security advisories!" >&2
    FAILED_STEPS+=("cargo audit")
fi

# 5. Dependency License & Bans Check
echo "[5/6] Running cargo deny --offline check..."
if cargo deny --offline check; then
    echo "  ✔ Cargo deny passed (licenses and crate sources verified)"
else
    echo "  ✘ Cargo deny check failed!" >&2
    FAILED_STEPS+=("cargo deny")
fi

# 6. Desktop & AppStream Metadata Validation
echo "[6/6] Validating packaging metadata..."
if command -v appstreamcli >/dev/null 2>&1; then
    if appstreamcli validate --no-net --pedantic data/com.shadhin.browser.metainfo.xml; then
        echo "  ✔ AppStream metadata validation passed"
    else
        echo "  ✘ AppStream metadata validation failed!" >&2
        FAILED_STEPS+=("appstreamcli validate")
    fi
else
    echo "  ℹ appstreamcli not found, skipping AppStream validation"
fi

if command -v desktop-file-validate >/dev/null 2>&1; then
    if desktop-file-validate data/com.shadhin.browser.desktop; then
        echo "  ✔ Desktop entry validation passed"
    else
        echo "  ✘ Desktop entry validation failed!" >&2
        FAILED_STEPS+=("desktop-file-validate")
    fi
else
    echo "  ℹ desktop-file-validate not found on host (install desktop-file-utils)"
fi

echo "========================================================================"
if [ ${#FAILED_STEPS[@]} -eq 0 ]; then
    echo "ALL CI CHECKS PASSED (6/6)."
    echo "========================================================================"
    exit 0
else
    echo "CI CHECK FAILED! Failures detected in: ${FAILED_STEPS[*]}" >&2
    echo "========================================================================"
    exit 1
fi
