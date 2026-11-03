# Shadhin Browser — Filter Lists & Content Blocking Specifications

## 1. Overview
Shadhin Browser protects user privacy using native WebKitGTK 6.0 content filter compilation (`WebKitUserContentFilterStore`) and injection (`WebKitUserContentManager`). Filter rules are compiled into optimized bytecode directly executed within WebKit's rendering engine without requiring external proxy layers.

---

## 2. Bundled Rule Snapshots

Shadhin Browser ships with offline-first bundled snapshots located in `resources/filters/`:

| Filter List | Identifier | Bundled Path | Upstream Source / Provenance | License | Initial Rule Count |
|---|---|---|---|---|---|
| **EasyList (Ads Subset)** | `easylist_ads` | `resources/filters/easylist_ads.json` | [EasyList](https://easylist.to/) (Curated core snapshot for ad networks, banner paths, and ad servers) | GNU GPLv3 / CC BY-SA 3.0 | 15 rules |
| **EasyPrivacy (Trackers Subset)** | `easyprivacy_trackers` | `resources/filters/easyprivacy_trackers.json` | [EasyPrivacy](https://easylist.to/) (Curated core snapshot for telemetry, tracking beacons, analytics) | GNU GPLv3 / CC BY-SA 3.0 | 14 rules |

---

## 3. WebKit Content Blocker Format

WebKit evaluates content filters using an array of JSON objects matching the schema:

```json
[
  {
    "trigger": {
      "url-filter": ".*://([^/]+\\.)?doubleclick\\.net/.*",
      "url-filter-is-case-sensitive": false,
      "resource-type": ["image", "script", "raw"],
      "load-type": ["third-party"]
    },
    "action": {
      "type": "block"
    }
  }
]
```

### Supported Actions:
- `block`: Prevents network fetch and aborts subresource loading.
- `block-cookies`: Blocks cookies from being sent or received.
- `css-display-none`: Hides elements matching `selector`.
- `ignore-previous-rules`: Allows exception handling for whitelisted domains.

---

## 4. Rule Compilation & Storage

Compiled filter bytecode is stored in:
`$XDG_DATA_HOME/shadhin-browser/filters/` (`~/.local/share/shadhin-browser/filters/`)

1. **Compilation**: `WebKitUserContentFilterStore::save` reads the JSON rule definition, parses patterns, and compiles them into binary WebKit bytecode.
2. **Loading**: Subsequent launches invoke `WebKitUserContentFilterStore::load` to load compiled filters instantly without re-parsing JSON.
3. **Activation**: Filters are attached to `WebKitUserContentManager` per tab/WebView using `add_filter()` and `remove_filter_by_id()`.

---

## 5. Per-Site Exceptions

Users can toggle "Shield off for this site" via the Privacy Shield popover.
- Stored in SQLite table `site_exceptions`:
  - `domain`: Fully qualified host (e.g. `example.com`).
  - `shield_enabled`: 0 (disabled) or 1 (enabled).
  - `ads_enabled`: 0 or 1.
  - `trackers_enabled`: 0 or 1.
- In Private Windows (`persistence_allowed = false`), site exceptions are strictly in-memory for the window duration.

---

## 6. Observability & Telemetry Limitations

Empirical testing (`tests/empirical_filter_test.rs`) confirms:
- **Filtering is fully effective**: Network requests matching compiled block triggers are aborted internally by WebKit before transmission over the network. The local HTTP server never receives blocked paths (e.g. `/ads/banner.js`, `/track/pixel.gif`).
- **Subresource blocking is silent**: WebKitGTK 6.0 does not emit any signal (`resource_load_started`, `connect_failed`, or WebProcess notification) to the UI process when a subresource is blocked by a content filter.
- **Honest metrics**: Because the exact count of blocked subresources and prevented byte transfers cannot be observed by the UI process, Shadhin Browser displays `—` with tooltip `"Not measurable on this WebKitGTK version"` across all stats indicators on the New Tab page. Fake counters and estimated data transfer metrics are strictly rejected.

