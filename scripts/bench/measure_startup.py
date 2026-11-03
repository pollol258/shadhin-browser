#!/usr/bin/env python3
"""
Startup Latency Measurement Tool for Shadhin Browser.
Measures time from launch to initial window presentation & idle event.
Supports:
- Empty profile (clean state)
- Populated profile (50,000 history items, 2,000 bookmarks)
Outputs min, median, max, and variance across repeat runs.
"""

import os
import sys
import time
import shutil
import tempfile
import argparse
import subprocess
import sqlite3

SCHEMA_SQL = """
PRAGMA journal_mode = WAL;
PRAGMA synchronous = NORMAL;

CREATE TABLE IF NOT EXISTS bookmarks (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    folder_id INTEGER,
    title TEXT NOT NULL,
    url TEXT NOT NULL,
    favicon BLOB,
    position INTEGER NOT NULL DEFAULT 0,
    created_at INTEGER NOT NULL
);

CREATE TABLE IF NOT EXISTS bookmark_folders (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    parent_id INTEGER,
    name TEXT NOT NULL,
    position INTEGER NOT NULL DEFAULT 0
);

CREATE TABLE IF NOT EXISTS history (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    url TEXT UNIQUE NOT NULL,
    title TEXT NOT NULL,
    visit_count INTEGER NOT NULL DEFAULT 1,
    first_visit INTEGER NOT NULL,
    last_visit INTEGER NOT NULL
);

CREATE TABLE IF NOT EXISTS shortcuts (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    title TEXT NOT NULL,
    url TEXT NOT NULL,
    position INTEGER NOT NULL DEFAULT 0
);

CREATE TABLE IF NOT EXISTS search_engines (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    name TEXT NOT NULL,
    keyword TEXT UNIQUE NOT NULL,
    url_template TEXT NOT NULL,
    is_default INTEGER NOT NULL DEFAULT 0,
    is_builtin INTEGER NOT NULL DEFAULT 0
);

CREATE TABLE IF NOT EXISTS workspaces (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    color TEXT NOT NULL,
    position INTEGER NOT NULL DEFAULT 0,
    data_dir_id TEXT NOT NULL,
    isolate_data INTEGER NOT NULL DEFAULT 1,
    created_at INTEGER NOT NULL
);

CREATE TABLE IF NOT EXISTS session_tabs (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    workspace_id TEXT NOT NULL,
    url TEXT NOT NULL,
    title TEXT NOT NULL,
    position INTEGER NOT NULL DEFAULT 0,
    is_active INTEGER NOT NULL DEFAULT 0
);

CREATE TABLE IF NOT EXISTS schema_version (
    version INTEGER PRIMARY KEY,
    applied_at TEXT NOT NULL
);

INSERT INTO schema_version(version, applied_at) VALUES (3, datetime('now'));
"""

def populate_test_profile(data_dir):
    os.makedirs(data_dir, mode=0o700, exist_ok=True)
    db_path = os.path.join(data_dir, "shadhin.db")
    conn = sqlite3.connect(db_path)
    conn.executescript(SCHEMA_SQL)
    cur = conn.cursor()

    now = int(time.time())
    # 20 Folders
    folders = [(None, f"Folder {i}", i) for i in range(1, 21)]
    cur.executemany("INSERT INTO bookmark_folders(parent_id, name, position) VALUES (?, ?, ?)", folders)

    # 2,000 Bookmarks
    bms = []
    for i in range(1, 2001):
        f_id = (i % 20) + 1
        bms.append((f_id, f"Bookmark Title {i}", f"https://example.org/item/{i}", None, i, now))
    cur.executemany("INSERT INTO bookmarks(folder_id, title, url, favicon, position, created_at) VALUES (?, ?, ?, ?, ?, ?)", bms)

    # 50,000 History rows
    hist = []
    for i in range(1, 50001):
        hist.append((f"https://service-{i % 500}.com/page/{i}", f"History Visit Item #{i} Article Details", (i % 5) + 1, now - i * 60, now - i * 60))
    cur.executemany("INSERT INTO history(url, title, visit_count, first_visit, last_visit) VALUES (?, ?, ?, ?, ?)", hist)

    conn.commit()
    conn.close()

def find_binary():
    rel = "./target/release/shadhin-browser"
    deb = "./target/debug/shadhin-browser"
    if os.path.exists(rel):
        return rel
    if os.path.exists(deb):
        return deb
    return None

def run_single_startup(binary_path, env):
    t0 = time.perf_counter()
    try:
        proc = subprocess.run(
            [binary_path],
            env=env,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            text=True,
            timeout=15.0
        )
        t1 = time.perf_counter()
        elapsed_ms = (t1 - t0) * 1000.0
        return elapsed_ms
    except subprocess.TimeoutExpired:
        sys.stderr.write("Timeout waiting for startup benchmark!\n")
        return None

def main():
    parser = argparse.ArgumentParser(description="Shadhin Startup Latency Benchmark")
    parser.add_argument("--bin", type=str, help="Path to browser binary")
    parser.add_argument("--runs", type=int, default=3, help="Number of test iterations (default: 3)")
    parser.add_argument("--profile", choices=["empty", "populated", "both"], default="both", help="Profile type to benchmark")
    args = parser.parse_args()

    binary = args.bin or find_binary()
    if not binary:
        sys.stderr.write("Error: Could not locate browser binary. Run `cargo build --release` first.\n")
        sys.exit(1)

    profiles = ["empty", "populated"] if args.profile == "both" else [args.profile]

    for prof in profiles:
        print(f"\n--- Benchmarking Startup ({prof.upper()} Profile, {args.runs} runs) ---")
        temp_root = tempfile.mkdtemp(prefix=f"shadhin_bench_startup_{prof}_")
        xdg_data = os.path.join(temp_root, "data")
        xdg_config = os.path.join(temp_root, "config")
        xdg_cache = os.path.join(temp_root, "cache")
        app_data_dir = os.path.join(xdg_data, "shadhin-browser")

        os.makedirs(xdg_data, mode=0o700, exist_ok=True)
        os.makedirs(xdg_config, mode=0o700, exist_ok=True)
        os.makedirs(xdg_cache, mode=0o700, exist_ok=True)

        if prof == "populated":
            populate_test_profile(app_data_dir)

        env = os.environ.copy()
        env["XDG_DATA_HOME"] = xdg_data
        env["XDG_CONFIG_HOME"] = xdg_config
        env["XDG_CACHE_HOME"] = xdg_cache
        env["SHADHIN_BENCH_STARTUP"] = "1"
        env["WEBKIT_DISABLE_DMABUF_RENDERER"] = "1"

        latencies = []
        for r in range(1, args.runs + 1):
            ms = run_single_startup(binary, env)
            if ms is not None:
                latencies.append(ms)
                print(f"  Run {r}: {ms:.1f} ms")

        if latencies:
            latencies.sort()
            min_ms = latencies[0]
            max_ms = latencies[-1]
            med_ms = latencies[len(latencies) // 2]
            variance = max_ms - min_ms
            print(f"Summary ({prof.upper()}): min={min_ms:.1f} ms, median={med_ms:.1f} ms, max={max_ms:.1f} ms (noise: ±{variance/2.0:.1f} ms)")

        shutil.rmtree(temp_root, ignore_errors=True)

if __name__ == '__main__':
    main()
