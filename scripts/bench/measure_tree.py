#!/usr/bin/env python3
"""
Process Tree Resource Measurement Tool for Shadhin Browser.
Measures:
- Total PSS, RSS, and Private Dirty across process tree (Parent + WebKitWebProcess + WebKitNetworkProcess)
- CPU utilization (utime + stime)
- Context switches (voluntary & involuntary)
- Thread counts
Outputs JSON and Markdown tables with min/median/max reporting across repeat iterations.
"""

import os
import sys
import time
import json
import argparse
import subprocess

def get_clk_tck():
    try:
        return os.sysconf(os.sysconf_names['SC_CLK_TCK'])
    except Exception:
        return 100

def get_child_pids(parent_pid):
    """Recursively finds all descendant PIDs of parent_pid."""
    pids = [parent_pid]
    try:
        out = subprocess.check_output(["pgrep", "-P", str(parent_pid)], text=True)
        children = [int(p.strip()) for p in out.strip().splitlines() if p.strip()]
        for child in children:
            pids.extend(get_child_pids(child))
    except (subprocess.CalledProcessError, FileNotFoundError):
        pass
    return sorted(list(set(pids)))

def find_shadhin_pid():
    """Finds the main shadhin-browser PID."""
    try:
        out = subprocess.check_output(["pgrep", "-f", "shadhin-browser"], text=True)
        pids = [int(p.strip()) for p in out.strip().splitlines() if p.strip()]
        # Filter out current python script if it happens to match
        my_pid = os.getpid()
        pids = [p for p in pids if p != my_pid]
        return pids[0] if pids else None
    except Exception:
        return None

def read_smaps_rollup(pid):
    """Parses /proc/<pid>/smaps_rollup or falls back to status."""
    metrics = {"pss_kb": 0, "rss_kb": 0, "private_dirty_kb": 0}
    rollup_path = f"/proc/{pid}/smaps_rollup"
    if os.path.exists(rollup_path):
        try:
            with open(rollup_path, "r") as f:
                for line in f:
                    parts = line.strip().split()
                    if len(parts) >= 2:
                        key = parts[0].rstrip(":")
                        val = int(parts[1])
                        if key == "Pss":
                            metrics["pss_kb"] = val
                        elif key == "Rss":
                            metrics["rss_kb"] = val
                        elif key == "Private_Dirty":
                            metrics["private_dirty_kb"] = val
            return metrics
        except (IOError, ProcessLookupError):
            pass

    status_path = f"/proc/{pid}/status"
    if os.path.exists(status_path):
        try:
            with open(status_path, "r") as f:
                for line in f:
                    if line.startswith("VmRSS:"):
                        parts = line.split()
                        metrics["rss_kb"] = int(parts[1])
                        metrics["pss_kb"] = metrics["rss_kb"]
        except (IOError, ProcessLookupError):
            pass
    return metrics

def read_proc_stat(pid):
    """Reads utime and stime from /proc/<pid>/stat."""
    stat_path = f"/proc/{pid}/stat"
    try:
        with open(stat_path, "r") as f:
            data = f.read().strip()
            # Command name can contain spaces and parentheses; locate last ')'
            rparen = data.rfind(")")
            if rparen != -1:
                fields = data[rparen + 2:].split()
                # fields[11] is utime (14th field in stat, 0-indexed after comm)
                # fields[12] is stime (15th field in stat)
                utime = int(fields[11])
                stime = int(fields[12])
                return utime, stime
    except (IOError, ProcessLookupError, IndexError):
        pass
    return 0, 0

def read_proc_status(pid):
    """Reads thread count and context switches from /proc/<pid>/status."""
    res = {"threads": 0, "vol_cs": 0, "invol_cs": 0, "name": ""}
    status_path = f"/proc/{pid}/status"
    try:
        with open(status_path, "r") as f:
            for line in f:
                if line.startswith("Name:"):
                    res["name"] = line.split(":", 1)[1].strip()
                elif line.startswith("Threads:"):
                    res["threads"] = int(line.split(":", 1)[1].strip())
                elif line.startswith("voluntary_ctxt_switches:"):
                    res["vol_cs"] = int(line.split(":", 1)[1].strip())
                elif line.startswith("nonvoluntary_ctxt_switches:"):
                    res["invol_cs"] = int(line.split(":", 1)[1].strip())
    except (IOError, ProcessLookupError):
        pass
    return res

def sample_tree(parent_pid):
    all_pids = get_child_pids(parent_pid)
    tree_data = {}
    for pid in all_pids:
        st = read_proc_status(pid)
        if not st["name"]:
            continue
        mem = read_smaps_rollup(pid)
        utime, stime = read_proc_stat(pid)
        tree_data[pid] = {
            "name": st["name"],
            "threads": st["threads"],
            "vol_cs": st["vol_cs"],
            "invol_cs": st["invol_cs"],
            "pss_kb": mem["pss_kb"],
            "rss_kb": mem["rss_kb"],
            "private_dirty_kb": mem["private_dirty_kb"],
            "utime": utime,
            "stime": stime,
        }
    return tree_data

def measure_window(parent_pid, duration_sec):
    clk_tck = get_clk_tck()
    t_start = time.time()
    t0_data = sample_tree(parent_pid)
    time.sleep(duration_sec)
    t_end = time.time()
    t1_data = sample_tree(parent_pid)

    elapsed = t_end - t_start
    total_pss_kb = sum(d["pss_kb"] for d in t1_data.values())
    total_rss_kb = sum(d["rss_kb"] for d in t1_data.values())
    total_dirty_kb = sum(d["private_dirty_kb"] for d in t1_data.values())
    total_threads = sum(d["threads"] for d in t1_data.values())

    total_ticks = 0
    vol_cs_delta = 0
    invol_cs_delta = 0

    per_proc = []
    for pid, d1 in t1_data.items():
        d0 = t0_data.get(pid, d1)
        ticks = (d1["utime"] - d0["utime"]) + (d1["stime"] - d0["stime"])
        cpu_pct = (ticks / (clk_tck * elapsed)) * 100.0 if elapsed > 0 else 0.0
        v_cs = d1["vol_cs"] - d0["vol_cs"]
        iv_cs = d1["invol_cs"] - d0["invol_cs"]

        total_ticks += ticks
        vol_cs_delta += v_cs
        invol_cs_delta += iv_cs

        per_proc.append({
            "pid": pid,
            "name": d1["name"],
            "threads": d1["threads"],
            "pss_mb": round(d1["pss_kb"] / 1024.0, 2),
            "rss_mb": round(d1["rss_kb"] / 1024.0, 2),
            "cpu_pct": round(cpu_pct, 2),
            "cs_per_sec": round((v_cs + iv_cs) / elapsed, 1)
        })

    tree_cpu_pct = (total_ticks / (clk_tck * elapsed)) * 100.0 if elapsed > 0 else 0.0
    cs_per_sec = (vol_cs_delta + invol_cs_delta) / elapsed if elapsed > 0 else 0.0

    return {
        "elapsed_sec": round(elapsed, 2),
        "total_pss_mb": round(total_pss_kb / 1024.0, 2),
        "total_rss_mb": round(total_rss_kb / 1024.0, 2),
        "total_private_dirty_mb": round(total_dirty_kb / 1024.0, 2),
        "total_cpu_pct": round(tree_cpu_pct, 2),
        "total_threads": total_threads,
        "cs_per_sec": round(cs_per_sec, 1),
        "processes": per_proc,
    }

def main():
    parser = argparse.ArgumentParser(description="Process Tree Resource Measurement")
    parser.add_argument("--pid", type=int, help="Target main PID (defaults to auto-detecting shadhin-browser)")
    parser.add_argument("--duration", type=float, default=5.0, help="Sampling window in seconds (default: 5.0)")
    parser.add_argument("--runs", type=int, default=3, help="Number of benchmark iterations (default: 3)")
    parser.add_argument("--json", action="store_true", help="Output raw JSON")
    args = parser.parse_args()

    pid = args.pid or find_shadhin_pid()
    if not pid:
        sys.stderr.write("Error: Could not locate running shadhin-browser process. Start the browser first or supply --pid.\n")
        sys.exit(1)

    all_runs = []
    for r in range(1, args.runs + 1):
        sample = measure_window(pid, args.duration)
        all_runs.append(sample)

    pss_vals = [s["total_pss_mb"] for s in all_runs]
    rss_vals = [s["total_rss_mb"] for s in all_runs]
    cpu_vals = [s["total_cpu_pct"] for s in all_runs]
    cs_vals = [s["cs_per_sec"] for s in all_runs]

    pss_vals.sort()
    rss_vals.sort()
    cpu_vals.sort()
    cs_vals.sort()

    median_idx = len(all_runs) // 2

    summary = {
        "pid": pid,
        "runs_count": args.runs,
        "duration_sec": args.duration,
        "pss_mb": {"min": pss_vals[0], "median": pss_vals[median_idx], "max": pss_vals[-1]},
        "rss_mb": {"min": rss_vals[0], "median": rss_vals[median_idx], "max": rss_vals[-1]},
        "cpu_pct": {"min": cpu_vals[0], "median": cpu_vals[median_idx], "max": cpu_vals[-1]},
        "cs_per_sec": {"min": cs_vals[0], "median": cs_vals[median_idx], "max": cs_vals[-1]},
        "latest_snapshot": all_runs[-1],
    }

    if args.json:
        print(json.dumps(summary, indent=2))
    else:
        print(f"\n=== Shadhin Browser Process Tree Metrics (PID {pid}, {args.runs} runs x {args.duration}s) ===")
        print(f"Total PSS: min={summary['pss_mb']['min']} MB, median={summary['pss_mb']['median']} MB, max={summary['pss_mb']['max']} MB")
        print(f"Total RSS: min={summary['rss_mb']['min']} MB, median={summary['rss_mb']['median']} MB, max={summary['rss_mb']['max']} MB")
        print(f"Total CPU: min={summary['cpu_pct']['min']}%, median={summary['cpu_pct']['median']}%, max={summary['cpu_pct']['max']}%")
        print(f"Context Switches/s: min={summary['cs_per_sec']['min']}, median={summary['cs_per_sec']['median']}, max={summary['cs_per_sec']['max']}")
        print("\nProcess Breakdown:")
        for p in summary["latest_snapshot"]["processes"]:
            print(f"  PID {p['pid']:<6} | {p['name']:<24} | Threads: {p['threads']:<2} | PSS: {p['pss_mb']:>7.2f} MB | RSS: {p['rss_mb']:>7.2f} MB | CPU: {p['cpu_pct']:>5.2f}% | CS/s: {p['cs_per_sec']:>6.1f}")

if __name__ == '__main__':
    main()
