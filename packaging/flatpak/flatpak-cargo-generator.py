#!/usr/bin/env python3
"""
flatpak-cargo-generator.py
Generates flatpak-builder cargo-sources.json from Cargo.lock for offline building.
Compatible with Python 3.11+ using standard library tomllib.
"""

import argparse
import json
import os
import sys
import tomllib


def parse_cargo_lock(lockfile_path):
    with open(lockfile_path, "rb") as f:
        return tomllib.load(f)


def generate_sources(lock_data):
    sources = []
    packages = lock_data.get("package", [])

    for pkg in packages:
        name = pkg.get("name")
        version = pkg.get("version")
        checksum = pkg.get("checksum")
        source = pkg.get("source", "")

        # Only process crates.io packages (which have a checksum)
        if not checksum or "crates.io" not in source:
            continue

        crate_name = f"{name}-{version}"
        url = f"https://static.crates.io/crates/{name}/{crate_name}.crate"

        # Archive entry: extracts crate into cargo/vendor/<name>-<version>
        sources.append({
            "type": "archive",
            "archive-type": "tar-gzip",
            "url": url,
            "sha256": checksum,
            "dest": f"cargo/vendor/{crate_name}"
        })

        # Cargo checksum metadata entry required by cargo --offline
        checksum_obj = {
            "package": checksum,
            "files": {}
        }
        sources.append({
            "type": "inline",
            "contents": json.dumps(checksum_obj),
            "dest": f"cargo/vendor/{crate_name}",
            "dest-filename": ".cargo-checksum.json"
        })

    # Cargo vendor config source to redirect crates-io to vendored dir
    cargo_config = """[source.crates-io]
replace-with = "vendored-sources"

[source.vendored-sources]
directory = "cargo/vendor"

[net]
offline = true
"""
    sources.append({
        "type": "inline",
        "contents": cargo_config,
        "dest": "cargo",
        "dest-filename": "config.toml"
    })

    return sources


def main():
    parser = argparse.ArgumentParser(description="Generate flatpak cargo sources JSON from Cargo.lock")
    parser.add_argument("cargo_lock", help="Path to Cargo.lock")
    parser.add_argument("-o", "--output", default="cargo-sources.json", help="Output file (default: cargo-sources.json)")
    args = parser.parse_args()

    if not os.path.isfile(args.cargo_lock):
        print(f"Error: {args.cargo_lock} not found", file=sys.stderr)
        sys.exit(1)

    lock_data = parse_cargo_lock(args.cargo_lock)
    sources = generate_sources(lock_data)

    with open(args.output, "w", encoding="utf-8") as f:
        json.dump(sources, f, indent=2)

    pkg_count = len([s for s in sources if s.get("archive-type") == "tar-gzip"])
    print(f"Successfully generated {args.output} with {pkg_count} vendored crates.")


if __name__ == "__main__":
    main()
