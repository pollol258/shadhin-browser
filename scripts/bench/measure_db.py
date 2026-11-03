#!/usr/bin/env python3
"""
Database Performance & Index Profiling Tool for Shadhin Browser.
Populates realistic test dataset:
- 50,000 history rows
- 2,000 bookmarks across 20 folders
- 500 downloads
Measures:
- EXPLAIN QUERY PLAN for history search, autocomplete, bookmarks, downloads
- Latency (min, p50, p95, p99, max) across 100 iterations
- WAL size and PRAGMA optimize impact
"""

import os
import sys
import time
import sqlite3
import tempfile
import argparse

SCHEMA_SQL = """
PRAGMA journal_mode = WAL;
PRAGMA synchronous = NORMAL;
PRAGMA foreign_keys = ON;

CREATE TABLE IF NOT EXISTS bookmarks (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    title TEXT NOT NULL,
    url TEXT NOT NULL,
    favicon TEXT,
    folder_id INTEGER,
    position INTEGER NOT NULL DEFAULT 0,
    created_at INTEGER NOT NULL,
    FOREIGN KEY(folder_id) REFERENCES bookmark_folders(id) ON DELETE CASCADE
);

CREATE TABLE IF NOT EXISTS bookmark_folders (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    name TEXT NOT NULL,
    parent_id INTEGER,
    position INTEGER NOT NULL DEFAULT 0,
    created_at INTEGER NOT NULL,
    FOREIGN KEY(parent_id) REFERENCES bookmark_folders(id) ON DELETE CASCADE
);

CREATE TABLE IF NOT EXISTS history (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    url TEXT NOT NULL UNIQUE,
    title TEXT NOT NULL,
    visit_count INTEGER NOT NULL DEFAULT 1,
    visit_time INTEGER NOT NULL
);

CREATE TABLE IF NOT EXISTS downloads (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    url TEXT NOT NULL,
    destination TEXT NOT NULL,
    total_bytes INTEGER,
    received_bytes INTEGER NOT NULL DEFAULT 0,
    state TEXT NOT NULL,
    started_at INTEGER NOT NULL,
    ended_at INTEGER
);

CREATE TABLE IF NOT EXISTS search_engines (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    name TEXT NOT NULL,
    keyword TEXT NOT NULL UNIQUE,
    url_template TEXT NOT NULL,
    is_default INTEGER NOT NULL DEFAULT 0
);

CREATE TABLE IF NOT EXISTS shortcuts (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    title TEXT NOT NULL,
    url TEXT NOT NULL,
    icon TEXT,
    position INTEGER NOT NULL
);

CREATE TABLE IF NOT EXISTS workspaces (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    color TEXT NOT NULL,
    icon TEXT
);

CREATE TABLE IF NOT EXISTS workspace_tabs (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    workspace_id TEXT NOT NULL,
    url TEXT NOT NULL,
    title TEXT NOT NULL,
    position INTEGER NOT NULL,
    is_active INTEGER NOT NULL DEFAULT 0,
    FOREIGN KEY(workspace_id) REFERENCES workspaces(id) ON DELETE CASCADE
);
"""

# Existing indexes
BASELINE_INDEXES = """
CREATE INDEX IF NOT EXISTS idx_history_url ON history(url);
CREATE INDEX IF NOT EXISTS idx_history_visit_time ON history(visit_time DESC);
CREATE INDEX IF NOT EXISTS idx_bookmarks_folder ON bookmarks(folder_id);
"""

# Optimized indexes for autocomplete, prefix search, and visit ranking
OPTIMIZED_INDEXES = """
CREATE INDEX IF NOT EXISTS idx_history_url ON history(url);
CREATE INDEX IF NOT EXISTS idx_history_visit_time ON history(visit_time DESC);
CREATE INDEX IF NOT EXISTS idx_history_autocomplete ON history(visit_count DESC, visit_time DESC);
CREATE INDEX IF NOT EXISTS idx_bookmarks_folder ON bookmarks(folder_id);
CREATE INDEX IF NOT EXISTS idx_bookmarks_url ON bookmarks(url);
CREATE INDEX IF NOT EXISTS idx_downloads_started ON downloads(started_at DESC);
"""

def populate_database(conn):
    print("Populating database with realistic test dataset...")
    cur = conn.cursor()

    # 1. Folders (20)
    now = int(time.time())
    folders = [(f"Folder {i}", None, i, now) for i in range(1, 21)]
    cur.executemany("INSERT INTO bookmark_folders(name, parent_id, position, created_at) VALUES (?, ?, ?, ?)", folders)

    # 2. Bookmarks (2,000)
    bms = []
    for i in range(1, 2001):
        f_id = (i % 20) + 1
        bms.append((f"Bookmark Title {i} - Comprehensive Resource", f"https://domain{i % 200}.org/page/{i}", None, f_id, i, now))
    cur.executemany("INSERT INTO bookmarks(title, url, favicon, folder_id, position, created_at) VALUES (?, ?, ?, ?, ?, ?)", bms)

    # 3. History (50,000)
    hist = []
    for i in range(1, 50001):
        v_count = (i % 10) + 1
        v_time = now - (i * 60)
        hist.append((f"https://service-{i % 500}.com/articles/{i}?session=bench", f"Bangla and English Article #{i} exploring Shadhin web performance", v_count, v_time))
    cur.executemany("INSERT INTO history(url, title, visit_count, visit_time) VALUES (?, ?, ?, ?)", hist)

    # 4. Downloads (500)
    dls = []
    states = ["completed", "failed", "cancelled", "paused"]
    for i in range(1, 501):
        dls.append((f"https://dist.example.org/files/installer_{i}.tar.gz", f"/home/user/Downloads/installer_{i}.tar.gz", 1024 * 1024 * (i % 50), 1024 * 1024 * (i % 50), states[i % 4], now - i * 3600, now - i * 3600 + 120))
    cur.executemany("INSERT INTO downloads(url, destination, total_bytes, received_bytes, state, started_at, ended_at) VALUES (?, ?, ?, ?, ?, ?, ?)", dls)

    conn.commit()
    print("Database populated successfully: 50,000 history, 2,000 bookmarks, 500 downloads.")

def profile_queries(conn, title):
    print(f"\n--- EXPLAIN QUERY PLAN ({title}) ---")
    cur = conn.cursor()

    queries = [
        ("History Visit Time Range (Daily Retention Cleanup)",
         "EXPLAIN QUERY PLAN DELETE FROM history WHERE visit_time < 1700000000"),
        ("History Autocomplete / Search",
         "EXPLAIN QUERY PLAN SELECT url, title, visit_count FROM history WHERE url LIKE '%bangla%' OR title LIKE '%bangla%' ORDER BY visit_count DESC, visit_time DESC LIMIT 20"),
        ("History Recent Visited",
         "EXPLAIN QUERY PLAN SELECT url, title FROM history ORDER BY visit_time DESC LIMIT 8"),
        ("Bookmark Lookup by Folder",
         "EXPLAIN QUERY PLAN SELECT id, title, url FROM bookmarks WHERE folder_id = 5 ORDER BY position ASC"),
        ("Downloads Ordered by Date",
         "EXPLAIN QUERY PLAN SELECT id, url, state FROM downloads ORDER BY started_at DESC LIMIT 50")
    ]

    for qname, qplan in queries:
        print(f"\n[{qname}]")
        for row in cur.execute(qplan):
            print(f"  {row}")

def benchmark_queries(conn, iterations=100):
    cur = conn.cursor()
    benchmarks = {
        "History Recent (8 items)": "SELECT url, title FROM history ORDER BY visit_time DESC LIMIT 8",
        "History Autocomplete Prefix": "SELECT url, title, visit_count FROM history WHERE url LIKE 'https://service-10%' ORDER BY visit_count DESC LIMIT 10",
        "Bookmark Tree Fetch": "SELECT b.id, b.title, b.url, f.name FROM bookmarks b LEFT JOIN bookmark_folders f ON b.folder_id = f.id ORDER BY f.position, b.position",
        "Downloads Recent (20 items)": "SELECT id, url, state FROM downloads ORDER BY started_at DESC LIMIT 20",
    }

    results = {}
    for name, sql in benchmarks.items():
        latencies = []
        for _ in range(iterations):
            t0 = time.perf_counter()
            cur.execute(sql).fetchall()
            t1 = time.perf_counter()
            latencies.append((t1 - t0) * 1000.0) # ms

        latencies.sort()
        p50 = latencies[int(len(latencies) * 0.50)]
        p95 = latencies[int(len(latencies) * 0.95)]
        p99 = latencies[int(len(latencies) * 0.99)]
        results[name] = {
            "min_ms": round(latencies[0], 3),
            "p50_ms": round(p50, 3),
            "p95_ms": round(p95, 3),
            "p99_ms": round(p99, 3),
            "max_ms": round(latencies[-1], 3),
        }
    return results

def main():
    parser = argparse.ArgumentParser(description="Database Benchmark & Profiler")
    parser.add_argument("--optimize", action="store_true", help="Apply optimized indexes")
    args = parser.parse_args()

    temp_db = tempfile.NamedTemporaryFile(suffix=".db", delete=False)
    temp_db_path = temp_db.name
    temp_db.close()

    try:
        conn = sqlite3.connect(temp_db_path)
        conn.executescript(SCHEMA_SQL)
        if args.optimize:
            conn.executescript(OPTIMIZED_INDEXES)
        else:
            conn.executescript(BASELINE_INDEXES)

        populate_database(conn)
        profile_queries(conn, "Optimized" if args.optimize else "Baseline")
        results = benchmark_queries(conn, 100)

        db_size_kb = os.path.getsize(temp_db_path) / 1024.0
        wal_path = temp_db_path + "-wal"
        wal_size_kb = os.path.getsize(wal_path) / 1024.0 if os.path.exists(wal_path) else 0.0

        print(f"\n=== Database Benchmark Results ({'Optimized Indexes' if args.optimize else 'Baseline Indexes'}) ===")
        print(f"Main DB Size: {db_size_kb:.1f} KB | WAL Size: {wal_size_kb:.1f} KB")
        print("-" * 75)
        print(f"{'Query Scenario':<30} | {'min (ms)':<8} | {'p50 (ms)':<8} | {'p95 (ms)':<8} | {'p99 (ms)':<8}")
        print("-" * 75)
        for name, m in results.items():
            print(f"{name:<30} | {m['min_ms']:<8.3f} | {m['p50_ms']:<8.3f} | {m['p95_ms']:<8.3f} | {m['p99_ms']:<8.3f}")

        # Check PRAGMA optimize
        conn.execute("PRAGMA optimize")
        conn.close()

    finally:
        for p in [temp_db_path, temp_db_path + "-wal", temp_db_path + "-shm"]:
            if os.path.exists(p):
                try:
                    os.remove(p)
                except OSError:
                    pass

if __name__ == '__main__':
    main()
