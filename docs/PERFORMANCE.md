# Shadhin Browser — Performance Benchmarking & Methodology

## 1. Principles
- **No Unmeasured Claims**: RAM and CPU consumption figures must be measured on real hardware using standard Linux profiling tools.
- **Process Model Accounting**: Because WebKitGTK uses a multi-process architecture (UI process, Network process, bwrap sandboxes, and one or more `WebKitWebProcess` rendering instances), total memory consumption must aggregate all child processes across the application's process tree.
- **PSS vs. RSS**:
  - **Resident Set Size (RSS)** overcounts multi-process applications because shared libraries (WebKitGTK, GTK4, Mesa, libc) mapped across multiple processes are counted in each process's RSS.
  - **Proportional Set Size (PSS)** from `/proc/<pid>/smaps_rollup` divides shared pages by the number of processes sharing them, providing the true proportional physical RAM footprint. Both are recorded below.

---

## 2. Empirical Benchmark Results (WSL2 Ubuntu 24.04, Linux Kernel 6.18.54)

Measured using a local HTTP server on `127.0.0.1:8089` serving static test pages with CSS and JavaScript:

| Workload | Process Tree PSS | Process Tree RSS | Process Details | Methodology |
|---|---|---|---|---|
| **Idle New Tab** (0 web pages) | **259.26 MB** | **308.88 MB** | `shadhin-browser`: 234.51 MB PSS<br>`WebKitNetworkProcess`: 24.75 MB PSS | Sum of `/proc/<pid>/smaps_rollup` across main UI process and initial Network process |
| **1 Loaded Page** (`127.0.0.1:8089/page1.html`) | **348.91 MB** | **507.49 MB** | `shadhin-browser`: 235.64 MB PSS<br>`WebKitNetworkProcess`: 26.69 MB PSS<br>`WebKitWebProcess`: 85.05 MB PSS<br>`bwrap`/proxies: ~1.53 MB PSS | Measured after full page render and network idle |
| **10 Loaded Pages** (10 tabs on `127.0.0.1`) | **588.37 MB** | **1918.06 MB** | `shadhin-browser`: 260.71 MB PSS<br>`WebKitNetworkProcess`: 19.36 MB PSS<br>10x `WebKitWebProcess`: ~29.6 MB PSS each (~296 MB total)<br>Sandboxes: ~12.2 MB PSS | 10 independent tabs loading separate pages. Notice RSS sums to 1918 MB due to shared lib duplication, while true PSS is 588.37 MB. |
| **30s Idle CPU** (with loaded page) | **3.27%** | — | Process tree ticks delta: 98 ticks over 30.00s | Evaluated via `/proc/<pid>/stat` (utime + stime) delta / (100 * elapsed) across all tree processes |

---

## 3. Repeatable Measurement Procedure

### 3.1 Python Process Tree Scanner Script
The measurement script `/tmp/measure_memory.py` executes each scenario, finds all child and grandchild PIDs (including sandboxed `bwrap` and `WebKitWebProcess` instances), reads `/proc/<pid>/smaps_rollup`, and calculates total PSS and RSS:

```python
import subprocess, time, os, signal

def get_process_stats(pids):
    stats = {}
    for pid in pids:
        try:
            comm = open(f'/proc/{pid}/comm').read().strip()
            pss_kb, rss_kb = 0, 0
            with open(f'/proc/{pid}/smaps_rollup') as f:
                for line in f:
                    if line.startswith('Pss:'):
                        pss_kb = int(line.split()[1])
                    elif line.startswith('Rss:'):
                        rss_kb = int(line.split()[1])
            stats[pid] = {'comm': comm, 'pss_kb': pss_kb, 'rss_kb': rss_kb}
        except Exception:
            pass
    return stats
```

### 3.2 Manual Shell One-Liner
To inspect PSS for a running instance:
```bash
BROWSER_PID=$(pgrep -f "target/.*/shadhin-browser" | head -n 1)
PIDS=$(pgrep -P $BROWSER_PID | tr '\n' ' ')
echo "Root: $BROWSER_PID, Children: $PIDS"
for p in $BROWSER_PID $PIDS; do
    grep -E "^(Pss|Rss):" /proc/$p/smaps_rollup 2>/dev/null | sed "s/^/$p /"
done | awk '{if ($2 ~ /Pss:/) pss+=$3; if ($2 ~ /Rss:/) rss+=$3} END {print "Total PSS: " pss/1024 " MB, RSS: " rss/1024 " MB"}'
```

---

## 4. Phase 6 Baselines, Optimizations & Empirical Measurements

### 4.1 Benchmarking Environment
- **Operating System**: Linux 6.18.54.1-microsoft-standard-WSL2 (x86_64)
- **Rust Toolchain**: rustc 1.99.0
- **GTK4 / Libadwaita / WebKitGTK**: `gtk4 v0.11.5` (GTK 4.14), `libadwaita v0.9.2` (1.5), `webkit6 v0.6.1` (WebKitGTK 2.42)
- **Benchmarking Suite**: `scripts/bench/` (`measure_startup.py`, `measure_db.py`, `measure_tree.py`, `serve_test_pages.py`, `verify_idle_io.py`)

### 4.2 Release Build Optimization & Binary Footprint
Configured in `Cargo.toml`:
```toml
[profile.release]
opt-level = 3
lto = "thin"
codegen-units = 1
panic = "abort"
strip = true
```

| Metric | Baseline Release (Unoptimized) | Optimized Release (LTO + Strip) | Delta / Improvement |
|---|---|---|---|
| **Binary Size** | 5.6 MB (5,836,360 B) | **3.9 MB** (4,061,848 B) | **-30.4% smaller** (-1.7 MB) |
| **Debug Binary** | 107 MB | 107 MB (kept fast with opt-level=0) | — |
| **Startup (Empty Profile)** | min=39.4ms, med=42.0ms, max=208.2ms | **min=40.0ms, med=44.0ms, max=109.2ms** | Consistent < 50ms warm launch |
| **Startup (50k History, 2k Bookmarks)** | min=38.5ms, med=40.9ms, max=41.7ms | **min=37.3ms, med=40.3ms, max=43.6ms** | **~40.3ms median** (no blocking disk I/O on UI thread) |

### 4.3 Database Query Latencies (50,000 History Rows, 2,000 Bookmarks, 500 Downloads)
Measured across 100 iterations per scenario using `python3 scripts/bench/measure_db.py`:

| Query Scenario | Baseline Index p50 | Optimized Index p50 | Speedup | EXPLAIN QUERY PLAN (Optimized) |
|---|---|---|---|---|
| **History Autocomplete Prefix** | 4.621 ms (p95: 6.63ms) | **0.099 ms** (p95: 0.15ms) | **46.7x faster** | `SCAN history USING INDEX idx_history_autocomplete` (eliminates temp B-tree sort) |
| **History Recent (8 items)** | 0.005 ms | **0.005 ms** | 1.0x | `SCAN history USING INDEX idx_history_visit_time` |
| **Downloads Recent (20 items)** | 0.032 ms (p95: 0.04ms) | **0.011 ms** (p95: 0.01ms) | **2.9x faster** | `SCAN downloads USING INDEX idx_downloads_started` |
| **Bookmark Tree Hierarchy Fetch** | 1.332 ms (p95: 1.59ms) | **1.327 ms** (p95: 2.53ms) | 1.0x | `SEARCH bookmarks USING INDEX idx_bookmarks_folder` |

### 4.4 WAL & PRAGMA Settings
- **`journal_mode = WAL`**: Concurrent reading without write stalls.
- **`synchronous = NORMAL`**: Eliminates redundant fsync stalls while preserving WAL durability.
- **`wal_autocheckpoint = 1000`**: Binds WAL checkpoint size to ~4 MB (1000 pages).
- **`PRAGMA optimize`**: Automatically triggered on clean shutdown via `StorageService::optimize()` to refresh query planner statistics.

### 4.5 Idle Network & Disk I/O Verification
- **Idle Network Sockets**: Exactly 0 outbound requests or socket creations over 60s idle. Zero telemetry, crash pings, or phone-home requests.
- **Idle Disk Writes**: 0 bytes written to disk during 60s idle with open static New Tab.
- **Tab Discarding RAM Savings**: Dropping background tabs frees the active `WebKitWebProcess` instance (~85 MB PSS per page), retaining lightweight metadata in memory and reloading seamlessly upon tab reactivation. Discarded tabs show a 70% opacity visual indicator in the tab strip.

