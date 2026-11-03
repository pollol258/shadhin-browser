# Third-Party Licenses and Attributions

Shadhin Browser is licensed under the **GNU General Public License Version 3 or later** (`GPL-3.0-or-later`).

This document lists the third-party software dependencies and content assets bundled with or linked by Shadhin Browser, along with their respective licenses and attribution requirements.

---

## 1. Direct and Transitive Rust Dependencies

All dependencies are verified using `cargo deny --offline check licenses` and `cargo deny list`:

| License | Count | Sample Crates | Compatibility with GPL-3.0 |
|---|---|---|---|
| **MIT** | 106 | `gtk4`, `libadwaita`, `webkit6`, `rusqlite`, `serde`, `tracing`, `glib` | Fully compatible |
| **Apache-2.0** | 59 | `anyhow`, `thiserror`, `toml`, `bitflags`, `indexmap`, `regex-automata` | Compatible with GPL-3.0 per FSF |
| **Apache-2.0 WITH LLVM-exception** | 1 | `target-lexicon` | Fully compatible |
| **Unicode-3.0** | 1 | `unicode-ident` | Fully compatible |
| **Unlicense** | 1 | `memchr` | Fully compatible (Public Domain dedication) |
| **Zlib** | 1 | `foldhash` | Fully compatible |

---

## 2. Bundled Content Filter Rule Snapshots

Shadhin Browser bundles offline filter rule snapshots in `resources/filters/`:

### EasyList & EasyPrivacy
- **Files**:
  - `resources/filters/easylist_ads.json` (Curated ad blocking rule subset)
  - `resources/filters/easyprivacy_trackers.json` (Curated tracking protection rule subset)
- **Source**: [EasyList Project](https://easylist.to/)
- **License**: Dual-licensed under **GNU General Public License Version 3 (GPLv3)** and **Creative Commons Attribution-ShareAlike 3.0 Application (CC BY-SA 3.0)**.
- **Attribution**:
  Copyright (C) EasyList Authors (https://easylist.to/).
  These filter lists are distributed under the terms of the GNU General Public License v3 or CC BY-SA 3.0.
- **Packaging Legality**:
  Because Shadhin Browser is distributed under GPL-3.0-or-later, bundling GPLv3-licensed filter snapshots in `.deb`, `.rpm`, and Flatpak packages is legally compliant under GNU GPLv3 terms.

---

## 3. Metadata & AppStream Documentation License

- **File**: `data/com.shadhin.browser.metainfo.xml`
- **Metadata License**: Creative Commons Zero 1.0 Universal (`CC0-1.0`).
