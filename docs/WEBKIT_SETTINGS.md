# WebKitGTK 6.0 Settings & Hardening Rationale

This document details each WebKitGTK 6.0 setting configured in `src/browser/webview.rs` and its security and functional rationale in Shadhin Browser.

---

## 1. Hardened Settings Matrix

| Setting | Value / Policy | Security & Operational Rationale |
|---|---|---|
| `enable_javascript` | Config-driven (`true` by default) | Provides standard rich web functionality while allowing complete script execution toggling via browser privacy preferences. |
| `enable_developer_extras` | `false` (enabled only if `SHADHIN_DEV=1` or `SHADHIN_DEV_CSS=1`) | Prevents exposing Web Inspector instrumentation and internal debugging endpoints in production builds. |
| `enable_smooth_scrolling` | `true` | Enhances user experience with fluid page scrolling across touchpad, mousewheel, and keyboard navigation. |
| `hardware_acceleration_policy` | `HardwareAccelerationPolicy::Always` | Utilizes hardware rendering pipelines without compromising memory safety. On WSLg, SHM fallback operates cleanly via `WEBKIT_DISABLE_DMABUF_RENDERER=1`. |
| `allow_file_access_from_file_urls` | `false` | Critical security boundary: Prevents documents loaded under `file://` from reading arbitrary files from the user's filesystem. |
| `allow_universal_access_from_file_urls` | `false` | Critical security boundary: Prevents documents loaded under `file://` from making cross-origin requests to any origin. |
| `media_playback_requires_user_gesture` | `true` | Prevents intrusive background audio and video autoplay upon page load until the user interacts with the document. |
| `javascript_can_open_windows_automatically` | `false` | Prevents unrequested popup windows and aggressive script-initiated redirects. |
| `enable_webaudio` | `true` | Supports modern web audio standards for media and communication services. |
| `user_agent` | Default system WebKitGTK string | Spoofing custom strings increases browser fingerprint uniqueness across fingerprinting scripts (e.g. CreepJS, EFF Cover Your Tracks). The default WebKitGTK user agent provides the highest degree of standard anonymity set and web compatibility. |
| `enable_page_cache` | `false` | Memory optimization: Disables WebKit's in-memory back/forward page cache. Prevents inactive background tabs from retaining full DOM trees and execution state in RAM, saving 30–80 MB per tab on modest (4–8 GB RAM) hardware. |
| `cache_model` | `CacheModel::DocumentBrowser` | Process memory tuning: Sets WebKit's global caching profile to DocumentBrowser rather than heavy WebBrowser, reducing speculative memory pre-allocation and page caching while maintaining snappy local disk cache performance. |

---

## 2. Process Isolation & Sandbox Policy

> [!IMPORTANT]
> Under no circumstances does Shadhin Browser disable WebKitGTK's multi-process sandbox (`WEBKIT_FORCE_SANDBOX=0` is never set). Web content processes, network services, and GPU renderers run in isolated containers.

---

## 3. Tab Lifecycle & Memory Discard Policy

To maintain modest memory usage under 4–8 GB RAM systems:
1. **Lazy WebView Allocation**: New tabs default to the lightweight native Libadwaita New Tab page without allocating a `WebKitWebProcess` or `webkit6::WebView` instance until a real web URL (`http://` or `https://`) is navigated.
2. **Idle Tab Discarding**: When open tabs exceed `TAB_DISCARD_THRESHOLD` (5 tabs) and background tabs remain inactive for > 5 minutes, their `webkit6::WebView` instances are detached and dropped from the widget stack, freeing associated WebProcess memory.
3. **Seamless State Restoration**: When a discarded tab is reactivated, the UI restores the `WebView` dynamically, re-hooks content filters, and reloads the saved URL.
4. **Visual Indicator**: Discarded tabs are rendered in the tab strip with a 70% opacity visual indicator (`.shadhin-tab.discarded`).
