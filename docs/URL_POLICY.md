# Shadhin Browser — Address Bar & URL Policy

This document defines the formal URL navigation policy, scheme classification, and address bar security rules enforced by `src/browser/navigation.rs`.

---

## 1. Scheme Classification Matrix

| Scheme / Input Pattern | Classification | Action / Destination | Security Rationale |
|---|---|---|---|
| `https://<host>` | Allowed Web URL | Direct HTTPS navigation | Secure, encrypted transport protocol. |
| `http://<host>` | Allowed Web URL | Direct HTTP navigation | Legacy unencrypted web; flagged with insecure shield in UI. |
| `shadhin:newtab` | Internal Application Page | Displays native New Tab dashboard | Safe internal native UI. |
| `shadhin:settings` | Internal Application Page | Opens preferences dialog | Safe internal native UI. |
| `shadhin:error` | Internal Application Page | Displays themed error/recovery page | Safe internal error renderer. |
| `about:blank`, `about:newtab` | Allowed Internal Aliases | Equivalent to New Tab or blank page | Compatibility with standard web conventions. |
| `localhost`, `localhost:<port>` | Bare Local Host | Rewritten to `http://localhost:<port>` | Local developer servers (allowed unencrypted HTTP). |
| IPv4 literal (`127.0.0.1:<port>`) | Bare IP Address | Rewritten to `http://<ip>:<port>` | Local / private network testing. |
| IPv6 literal (`[::1]:<port>`) | Bare IPv6 Address | Rewritten to `http://[::1]:<port>` | Local / private network testing. |
| `<domain>.<tld>` (e.g. `example.com`) | Bare Domain Name | Rewritten to `https://<domain>.<tld>` | Defaulting to HTTPS prevents downgrade attacks. |
| IDN / Unicode (e.g. `বাংলা.ভারত`) | Internationalized Domain | Rewritten to `https://<unicode_domain>` | Supports international script domain resolutions safely. |
| Multi-word query (e.g. `rust lang`) | Search Query | `https://duckduckgo.com/?q=<encoded>` | Percent-escaped query dispatched to default search engine. |
| Single token non-domain | Search Query | `https://duckduckgo.com/?q=<encoded>` | Dispatched to search engine when no valid TLD exists. |

---

## 2. Prohibited Schemes (Explicitly Blocked)

| Scheme | Reason for Prohibition | User Experience |
|---|---|---|
| `javascript:` | Address-bar script execution is an attack vector for Self-XSS and session theft. | Rejected with clear UI message: *"JavaScript execution from address bar is prohibited for security"*. |
| `data:` | Data URLs typed into address bar facilitate credential phishing and script injection. | Rejected with clear UI message: *"Data URLs in address bar are blocked to prevent phishing and XSS"*. |
| `file:` | Directly accessing arbitrary local filesystem paths from untrusted input risks data exfiltration. | Rejected with clear UI message: *"Local filesystem access is prohibited from address bar"*. |
| `ftp:` | Cleartext, unmaintained legacy protocol with severe vulnerability surface. | Rejected with clear UI message: *"FTP protocol is deprecated and insecure"*. |
| `blob:` | Blob URIs cannot be resolved without active origin context in a web document. | Rejected with clear UI message: *"Blob URLs cannot be directly navigated from address bar"*. |
| Custom app schemes (e.g. `steam:`, `tg:`) | Unverified protocol handler invocation can trigger local command execution vulnerabilities. | Rejected with clear UI message: *"Unsupported or unverified URI scheme"*. |

---

## 3. Input Handling Rules

1. **Whitespace Normalization**: Leading and trailing whitespace is stripped immediately upon input submission.
2. **Focus Protection**: While the user is actively typing in the address bar (the entry has focus), background navigation state updates (e.g., redirects, iframe events) MUST NOT overwrite or replace the text in the entry.
3. **Display Host Extraction**: When presenting the active URL in the address bar, the scheme, host, and path are formatted cleanly. The host is clearly distinguishable for user identity verification.
4. **Percent Encoding**: Search queries, including non-ASCII Unicode (such as Bangla script), are safely percent-encoded using standard GLib URI encoding functions before passing to search engine endpoints.
