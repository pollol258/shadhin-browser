# Shadhin Browser — Threat Model & Security Posture

## 1. Scope & Objectives
Shadhin Browser is a desktop browser targeting Linux (GNOME/KDE, Wayland/X11). This document defines the threat actors, trust boundaries, mitigations, and architectural guarantees implemented in Phase 5.

---

## 2. Threat Actors & Scenarios

| Threat Actor | Capabilities & Goals | Mitigations in Shadhin Browser |
|---|---|---|
| **Malicious Web Origin** | Attempts drive-by downloads, XSS, port scanning, fingerprinting, or sandbox escape. | WebKit bubblewrap sandbox, strict scheme whitelist, `shadhin:`/`file:` navigation prevention from web context, no auto-execution of downloads. |
| **Network Attacker (MitM)** | Intercepts plain HTTP connections to eavesdrop or tamper with traffic. | HTTPS-Only Mode upgrades all plain HTTP requests, shows blocking interstitial on downgrade or connection failure, blocks TLS certificate bypass. |
| **Cross-Site Tracker & Profiler** | Correlates user activity across domains using 3rd-party cookies, tracking query params, and browser fingerprinting. | EasyPrivacy content filters, WebKit Intelligent Tracking Prevention (ITP), third-party cookie blocking, 21 known tracking parameters stripped from URLs. |
| **Host Local User / Multi-User System** | Attempts to read browsing history, cookies, or stored sessions from disk. | User data directories (`0700`), database & config files (`0600`), Private Browsing uses purely ephemeral in-memory sessions with zero disk writes. |
| **Malicious Hardware Prober** | Web application attempting to silently access camera, microphone, geolocation, or notifications. | WebKit permission interception with non-modal infobar prompt, explicit user Allow/Block, persistent origin decisions, ephemeral decisions in Private Mode. |

---

## 3. Security Boundaries & Assumptions

### 3.1 Process Isolation & WebKit Sandbox
- **WebKit Process Model**: Web processes execute in separated unprivileged processes isolated via Linux user namespaces, seccomp filters, and Bubblewrap (bwrap).
- **Hard Rule**: Process isolation and sandboxing MUST never be disabled (`WebKitSettings:enable-sandbox` remains active).
- **Compromised Web Content**: Even if an exploit compromises a WebKit web rendering process, it cannot access the host filesystem outside permitted sandbox descriptors or read browser profile secrets directly.

### 3.2 Scheme & Navigation Security
- Only approved schemes (`http`, `https`, `about:blank`, `about:newtab`) are permitted for web navigation.
- Dangerous custom URL schemes (`file://`, `javascript:`, `data:` for top-level navigation, `ghelp:`, `gio:`) are strictly intercepted and rejected in `classify_input`.
- In `connect_decide_policy`, attempts by web content to navigate to `shadhin:` internal pages or `file:` URIs are explicitly blocked and ignored.
- Content filters and navigation policy decisions run before any remote payload is evaluated.

### 3.3 Download Safety
- Shadhin Browser acts strictly as a file fetcher.
- **Downloaded files are NEVER auto-executed**, regardless of MIME type or execution bits.
- Download destinations default to `~/Downloads` or user prompt, with temporary partial files (`.part`).
- Files are saved with non-executable `0600` permissions.
- Dangerous file extensions (`.sh`, `.desktop`, `.AppImage`, `.exe`, `.deb`, `.rpm`) display prominent warning badges.

### 3.4 Profile Data & Storage Permissions
- Configuration: `~/.config/shadhin-browser/` (file mode `0600`, directory mode `0700`).
- Database & History: `~/.local/share/shadhin-browser/` (file mode `0600`, directory mode `0700`).
- Cache: `~/.cache/shadhin-browser/`.
- Private Browsing: Operates with ephemeral, in-memory `WebKitWebsiteDataManager` / `WebKitNetworkSession`. No persistent disk cache, history, or cookies are stored (`persistence_allowed = false` guards all SQLite writes).

### 3.5 Privacy Disclaimers & Truth in Advertising
- **Private Mode vs Anonymity**: Private Browsing prevents local footprint on the device (history, cache, cookies). It does **not** hide IP addresses from websites, ISPs, or network adversaries. Tor/VPN integration is distinct.
- **Fingerprinting Protection**: WebKitGTK provides varying levels of fingerprint mitigation. Shadhin Browser reports actual enforceable protections and never claims 100% fingerprint immunity.
- **Zero Telemetry**: Shadhin Browser initiates zero background network connections on launch, idle, or navigation. No analytics, crash telemetry, or tracking pings are sent.
- **Honest Metrics**: Privacy Shield statistics start strictly at 0 on a fresh profile. Blocked counts and data saved are never fabricated. WebKit's internal subresource drop behavior is documented transparently in `LIMITATIONS.md`.
