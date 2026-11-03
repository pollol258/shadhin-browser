//! Privacy-safe network diagnostics and request/response instrumentation for WebKitGTK 6.0
//!
//! Provides structured logging for main-frame requests, response headers, redirects,
//! TLS validation events, and CDN diagnostics (e.g. Akamai Edge error tracking).
//!
//! Strict privacy invariants enforced:
//! - Never logs passwords, auth tokens, session IDs, or private user data
//! - Redacts sensitive query parameter values
//! - Masks Authorization and Proxy-Authorization headers
//! - Logs Cookie and Set-Cookie presence as boolean / count only (never cookie contents)

use gtk4::gio::TlsCertificateFlags;
use gtk4::glib;
use tracing::{debug, error, info, warn};
use webkit6::{LoadEvent, URIRequest, URIResponse};

/// List of sensitive query parameter keys whose values must be masked in logs
pub const SENSITIVE_QUERY_KEYS: &[&str] = &[
    "token",
    "access_token",
    "refresh_token",
    "auth",
    "key",
    "api_key",
    "apikey",
    "secret",
    "password",
    "passwd",
    "pwd",
    "code",
    "session",
    "sig",
    "signature",
    "client_secret",
];

/// Redacts sensitive query parameter values from a URL while preserving scheme,
/// host, path, and non-sensitive query keys for diagnostic tracing.
pub fn sanitize_url(raw_url: &str) -> String {
    let (base_and_path, query_and_frag) = match raw_url.split_once('?') {
        Some(parts) => parts,
        None => return raw_url.to_string(),
    };

    let (query, fragment) = match query_and_frag.split_once('#') {
        Some((q, f)) => (q, Some(f)),
        None => (query_and_frag, None),
    };

    let mut sanitized_pairs = Vec::new();
    for pair in query.split('&') {
        if pair.is_empty() {
            continue;
        }
        if let Some((key, _val)) = pair.split_once('=') {
            let key_lower = key.to_ascii_lowercase();
            let is_sensitive = SENSITIVE_QUERY_KEYS
                .iter()
                .any(|&s| key_lower == s || key_lower.contains(s));
            if is_sensitive {
                sanitized_pairs.push(format!("{key}=[REDACTED]"));
            } else {
                sanitized_pairs.push(pair.to_string());
            }
        } else {
            sanitized_pairs.push(pair.to_string());
        }
    }

    let mut out = base_and_path.to_string();
    if !sanitized_pairs.is_empty() {
        out.push('?');
        out.push_str(&sanitized_pairs.join("&"));
    }
    if let Some(f) = fragment {
        out.push('#');
        out.push_str(f);
    }
    out
}

/// Records diagnostic information for a main-frame navigation request
pub fn record_navigation_request(req: &URIRequest, is_redirect: bool, frame_name: Option<&str>) {
    let raw_uri = req.uri().map(|u| u.to_string()).unwrap_or_default();
    let sanitized_uri = sanitize_url(&raw_uri);
    let method = req
        .http_method()
        .map(|m| m.to_string())
        .unwrap_or_else(|| "GET".to_string());
    let frame_desc = frame_name.unwrap_or("main-frame");

    info!(
        "[NET REQ] {} {} (redirect={}, frame={})",
        method, sanitized_uri, is_redirect, frame_desc
    );

    if let Some(headers) = req.http_headers() {
        let mut user_agent = None;
        let mut accept = None;
        let mut accept_lang = None;
        let mut accept_enc = None;
        let mut referer = None;
        let mut origin = None;
        let mut has_cookie = false;
        let mut other_headers = Vec::new();

        headers.foreach(|name, val| {
            let n_lower = name.to_ascii_lowercase();
            match n_lower.as_str() {
                "user-agent" => user_agent = Some(val.to_string()),
                "accept" => accept = Some(val.to_string()),
                "accept-language" => accept_lang = Some(val.to_string()),
                "accept-encoding" => accept_enc = Some(val.to_string()),
                "referer" => referer = Some(sanitize_url(val)),
                "origin" => origin = Some(sanitize_url(val)),
                "cookie" => {
                    has_cookie = true;
                }
                "authorization" | "proxy-authorization" => {
                    other_headers.push(format!("{}: [PRESENT - REDACTED]", name));
                }
                _ => {
                    if n_lower.starts_with("sec-") || n_lower == "upgrade-insecure-requests" {
                        other_headers.push(format!("{}: {}", name, val));
                    }
                }
            }
        });

        debug!(
            "[NET REQ HEADERS] User-Agent: {:?}, Accept: {:?}, Accept-Language: {:?}, Accept-Encoding: {:?}, Cookie-Present: {}, Referer: {:?}, Origin: {:?}",
            user_agent, accept, accept_lang, accept_enc, has_cookie, referer, origin
        );
        if !other_headers.is_empty() {
            debug!("[NET REQ EXTRA] {}", other_headers.join(", "));
        }
    }
}

/// Records diagnostic information for a main-frame response
pub fn record_response_decision(resp: &URIResponse, is_main_frame: bool) {
    let raw_uri = resp.uri().map(|u| u.to_string()).unwrap_or_default();
    let sanitized_uri = sanitize_url(&raw_uri);
    let status = resp.status_code();
    let mime = resp
        .mime_type()
        .map(|m| m.to_string())
        .unwrap_or_else(|| "unknown".to_string());

    info!(
        "[NET RESP] status={} mime={} uri={} (main_frame={})",
        status, mime, sanitized_uri, is_main_frame
    );

    if let Some(headers) = resp.http_headers() {
        let mut server = None;
        let mut location = None;
        let mut via = None;
        let mut akamai_headers = Vec::new();
        let mut set_cookie_count = 0usize;

        headers.foreach(|name, val| {
            let n_lower = name.to_ascii_lowercase();
            if n_lower == "server" {
                server = Some(val.to_string());
            } else if n_lower == "location" {
                location = Some(sanitize_url(val));
            } else if n_lower == "via" {
                via = Some(val.to_string());
            } else if n_lower.starts_with("x-akamai")
                || n_lower == "akamai-grn"
                || n_lower == "bot_id"
            {
                akamai_headers.push(format!("{}: {}", name, val));
            } else if n_lower == "set-cookie" {
                set_cookie_count += 1;
            }
        });

        if let Some(loc) = location.as_ref() {
            info!("[NET REDIRECT] {} -> Location: {}", sanitized_uri, loc);
        }

        if let Some(srv) = server.as_ref() {
            debug!("[NET SERVER] Server: {}", srv);
        }
        if let Some(v) = via.as_ref() {
            debug!("[NET VIA] Via: {}", v);
        }
        if set_cookie_count > 0 {
            debug!(
                "[NET COOKIES] Set-Cookie headers received: {}",
                set_cookie_count
            );
        }

        // Check for CDN specific signatures (e.g. Akamai Edge)
        let is_akamai = server
            .as_deref()
            .map(|s| s.contains("Akamai"))
            .unwrap_or(false)
            || !akamai_headers.is_empty();

        if is_akamai {
            let ak_str = if akamai_headers.is_empty() {
                "none".to_string()
            } else {
                akamai_headers.join(" | ")
            };
            if status >= 400 {
                warn!(
                    "[CDN DIAGNOSTIC - AKAMAI ERROR] Status: {} for {} | Server: {:?} | Akamai Metadata: [{}]",
                    status, sanitized_uri, server, ak_str
                );
            } else {
                info!(
                    "[CDN DIAGNOSTIC - AKAMAI OK] Status: {} for {} | Server: {:?} | Akamai Metadata: [{}]",
                    status, sanitized_uri, server, ak_str
                );
            }
        }
    }
}

/// Records a failed load event with detailed error domain and code
pub fn record_load_failed(failing_uri: &str, event: LoadEvent, error: &glib::Error) {
    let sanitized_uri = sanitize_url(failing_uri);
    warn!(
        "[NET FAIL] Load failed for {} at stage {:?}: {} (domain: {:?}, code: {})",
        sanitized_uri,
        event,
        error,
        error.domain(),
        error.code()
    );
}

/// Records a TLS certificate validation failure with specific certificate error flags
pub fn record_tls_failure(failing_uri: &str, errors: TlsCertificateFlags) {
    let sanitized_uri = sanitize_url(failing_uri);
    error!(
        "[NET TLS ERROR] TLS certificate validation failed for {}: flags={:?}",
        sanitized_uri, errors
    );
}

/// Records successful load completion
pub fn record_load_finished(final_uri: &str) {
    let sanitized_uri = sanitize_url(final_uri);
    info!("[NET FINISH] Document load completed: {}", sanitized_uri);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sanitize_url_redaction() {
        let input = "https://example.com/api/v1?token=secret12345&dir=almalinux/8&password=mypass&user=john";
        let clean = sanitize_url(input);
        assert!(clean.contains("token=[REDACTED]"));
        assert!(clean.contains("password=[REDACTED]"));
        assert!(clean.contains("dir=almalinux/8"));
        assert!(clean.contains("user=john"));
        assert!(!clean.contains("secret12345"));
        assert!(!clean.contains("mypass"));
    }

    #[test]
    fn test_sanitize_url_no_query() {
        let input = "https://example.com/path/to/resource";
        assert_eq!(sanitize_url(input), input);
    }

    #[test]
    fn test_sanitize_url_fragment_preserved() {
        let input = "https://example.com/login?auth=123#section-2";
        let clean = sanitize_url(input);
        assert_eq!(clean, "https://example.com/login?auth=[REDACTED]#section-2");
    }
}
