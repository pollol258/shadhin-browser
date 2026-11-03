//! URL navigation policy, scheme validation, and address bar classification
//!
//! Enforces strict security boundaries on address bar inputs, rejecting dangerous
//! pseudo-schemes (javascript:, data:, file:) and translating bare domains,
//! IP addresses, internal pages, and search phrases into validated targets.

use gtk4::glib;
use std::net::IpAddr;

/// Result of classifying a user's address bar input
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum NavigationTarget {
    /// Valid web URL (HTTP or HTTPS)
    Url(String),
    /// Search engine query with encoded search URL
    Search(String),
    /// Safe internal application page (e.g., shadhin:newtab, shadhin:settings)
    Internal(String),
    /// Prohibited or dangerous scheme rejected by security policy
    Blocked { scheme: String, reason: String },
}

/// Permitted internal schemes and page targets
pub const ALLOWED_INTERNAL_PAGES: &[&str] = &[
    "shadhin:newtab",
    "shadhin:settings",
    "shadhin:bookmarks",
    "shadhin:history",
    "shadhin:downloads",
    "shadhin:error",
    "about:blank",
    "about:newtab",
    "about:bookmarks",
    "about:history",
    "about:downloads",
];

/// Prohibited dangerous schemes that must never be navigated from the address bar
pub const BLOCKED_SCHEMES: &[(&str, &str)] = &[
    (
        "javascript",
        "JavaScript execution from address bar is prohibited for security",
    ),
    (
        "data",
        "Data URLs in address bar are blocked to prevent phishing and XSS",
    ),
    (
        "file",
        "Local filesystem access is prohibited from address bar",
    ),
    ("ftp", "FTP protocol is deprecated and insecure"),
    (
        "blob",
        "Blob URLs cannot be directly navigated from address bar",
    ),
];

/// Classifies user address bar input into a safe `NavigationTarget`
pub fn classify_input(raw_input: &str, search_template: &str) -> NavigationTarget {
    // Reject illegal null bytes immediately to prevent C FFI panics and injection
    if raw_input.contains('\0') {
        return NavigationTarget::Blocked {
            scheme: "null-byte".to_string(),
            reason: "Input contains prohibited null byte".to_string(),
        };
    }

    let trimmed = raw_input.trim();
    if trimmed.is_empty() {
        return NavigationTarget::Internal("shadhin:newtab".to_string());
    }

    // 1. Check for exact internal pages
    for &page in ALLOWED_INTERNAL_PAGES {
        if trimmed.eq_ignore_ascii_case(page) {
            return NavigationTarget::Internal(page.to_string());
        }
    }

    // 2. If the input contains whitespace, it's definitely a search query
    if trimmed.contains(char::is_whitespace) {
        return build_search_target(trimmed, search_template);
    }

    // 3. Check for localhost or localhost:port
    if is_localhost(trimmed) {
        return NavigationTarget::Url(format!("http://{trimmed}"));
    }

    // 4. Check for IP address (IPv4 or IPv6 literal)
    if is_ip_address_or_host(trimmed) {
        return NavigationTarget::Url(format!("http://{trimmed}"));
    }

    // 5. Check for scheme prefix (e.g., http://, https://, javascript:, data:)
    if let Some(colon_pos) = trimmed.find(':') {
        let after_colon = &trimmed[colon_pos + 1..];
        let port_candidate = after_colon.split('/').next().unwrap_or(after_colon);

        // If after colon is pure digits, it's a host:port (handled below), not a scheme!
        let is_port =
            !port_candidate.is_empty() && port_candidate.chars().all(|c| c.is_ascii_digit());

        if !is_port {
            let scheme_raw = &trimmed[..colon_pos];
            let scheme_lower = scheme_raw.to_ascii_lowercase();

            // Blocked schemes check
            for &(blocked, reason) in BLOCKED_SCHEMES {
                if scheme_lower == blocked {
                    return NavigationTarget::Blocked {
                        scheme: scheme_lower,
                        reason: reason.to_string(),
                    };
                }
            }

            // HTTP / HTTPS allowed schemes
            if scheme_lower == "http" || scheme_lower == "https" {
                if trimmed.len() > colon_pos + 2 && trimmed[colon_pos..].starts_with("://") {
                    let after_slash = &trimmed[colon_pos + 3..];
                    if !after_slash.is_empty() && !after_slash.starts_with('/') {
                        return NavigationTarget::Url(trimmed.to_string());
                    }
                }
                // Malformed http:/ or http: without host
                return NavigationTarget::Blocked {
                    scheme: scheme_lower,
                    reason: "Malformed URL scheme syntax".to_string(),
                };
            }

            // Allowed custom internal scheme (shadhin:)
            if scheme_lower == "shadhin" {
                if ALLOWED_INTERNAL_PAGES
                    .iter()
                    .any(|&p| p.eq_ignore_ascii_case(trimmed))
                {
                    return NavigationTarget::Internal(trimmed.to_string());
                } else {
                    return NavigationTarget::Blocked {
                        scheme: scheme_lower,
                        reason: "Unknown or prohibited internal page".to_string(),
                    };
                }
            }

            // Any other custom or unknown scheme is blocked
            return NavigationTarget::Blocked {
                scheme: scheme_lower,
                reason: "Unsupported or unverified URI scheme".to_string(),
            };
        }
    }

    // 6. Check if input looks like a valid domain name or domain with path
    if looks_like_domain(trimmed) {
        return NavigationTarget::Url(format!("https://{trimmed}"));
    }

    // 7. Fallback: Search engine query
    build_search_target(trimmed, search_template)
}

/// Constructs search target with proper percent-encoding
fn build_search_target(query: &str, template: &str) -> NavigationTarget {
    let escaped = glib::Uri::escape_string(query, None, false);
    let search_url = if template.contains("%s") {
        template.replace("%s", &escaped)
    } else {
        format!("https://duckduckgo.com/?q={escaped}")
    };
    NavigationTarget::Search(search_url)
}

/// Checks if token is localhost or localhost:port
fn is_localhost(token: &str) -> bool {
    let host_part = token.split('/').next().unwrap_or(token);
    let host_only = host_part.split(':').next().unwrap_or(host_part);
    host_only.eq_ignore_ascii_case("localhost")
}

/// Checks if token is a valid IPv4 or IPv6 address (with optional port and path)
fn is_ip_address_or_host(token: &str) -> bool {
    let host_part = token.split('/').next().unwrap_or(token);

    // IPv6 literal enclosed in brackets [::1]:8080
    if host_part.starts_with('[')
        && let Some(end_bracket) = host_part.find(']')
    {
        let ip_str = &host_part[1..end_bracket];
        return ip_str.parse::<IpAddr>().is_ok();
    }

    // IPv4 with optional port
    let host_only = host_part.split(':').next().unwrap_or(host_part);
    host_only.parse::<IpAddr>().is_ok()
}

/// Checks if string has domain-like structure (e.g., example.com, test.co.uk, বাংলা.ভারত)
fn looks_like_domain(token: &str) -> bool {
    let host_part = token.split('/').next().unwrap_or(token);
    let host_only = host_part.split(':').next().unwrap_or(host_part);

    if host_only.is_empty() || host_only.starts_with('.') || host_only.ends_with('.') {
        return false;
    }

    let parts: Vec<&str> = host_only.split('.').collect();
    if parts.len() < 2 {
        return false;
    }

    // Validate that labels are non-empty
    for part in &parts {
        if part.is_empty() {
            return false;
        }
    }

    // Check last label (TLD)
    let Some(tld) = parts.last() else {
        return false;
    };
    // Valid TLD: at least 2 chars, letters, digits, hyphens or non-ascii unicode
    tld.chars().count() >= 2
        && tld
            .chars()
            .all(|c| c.is_alphanumeric() || c == '-' || !c.is_ascii())
}

/// Extracts a clean display host from a URL for UI presentation
#[allow(dead_code)]
pub fn extract_display_host(url: &str) -> Option<String> {
    if let Ok(uri) = glib::Uri::parse(url, glib::UriFlags::NONE)
        && let Some(host) = uri.host()
    {
        return Some(host.to_string());
    }

    // Fallback manual parser
    if let Some(pos) = url.find("://") {
        let after_scheme = &url[pos + 3..];
        let host_part = after_scheme.split('/').next().unwrap_or(after_scheme);
        let host_only = host_part.split(':').next().unwrap_or(host_part);
        if !host_only.is_empty() {
            return Some(host_only.to_string());
        }
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;

    const SEARCH_TMPL: &str = "https://duckduckgo.com/?q=%s";

    #[test]
    fn test_empty_and_whitespace() {
        assert_eq!(
            classify_input("", SEARCH_TMPL),
            NavigationTarget::Internal("shadhin:newtab".to_string())
        );
        assert_eq!(
            classify_input("   ", SEARCH_TMPL),
            NavigationTarget::Internal("shadhin:newtab".to_string())
        );
    }

    #[test]
    fn test_internal_pages() {
        assert_eq!(
            classify_input("shadhin:newtab", SEARCH_TMPL),
            NavigationTarget::Internal("shadhin:newtab".to_string())
        );
        assert_eq!(
            classify_input("shadhin:settings", SEARCH_TMPL),
            NavigationTarget::Internal("shadhin:settings".to_string())
        );
        assert_eq!(
            classify_input("about:blank", SEARCH_TMPL),
            NavigationTarget::Internal("about:blank".to_string())
        );
        assert_eq!(
            classify_input("about:newtab", SEARCH_TMPL),
            NavigationTarget::Internal("about:newtab".to_string())
        );
    }

    #[test]
    fn test_full_https_urls() {
        assert_eq!(
            classify_input("https://example.com", SEARCH_TMPL),
            NavigationTarget::Url("https://example.com".to_string())
        );
        assert_eq!(
            classify_input("https://wikipedia.org/wiki/Rust", SEARCH_TMPL),
            NavigationTarget::Url("https://wikipedia.org/wiki/Rust".to_string())
        );
        assert_eq!(
            classify_input("https://sub.domain.co.uk:8443/path?arg=1#top", SEARCH_TMPL),
            NavigationTarget::Url("https://sub.domain.co.uk:8443/path?arg=1#top".to_string())
        );
    }

    #[test]
    fn test_full_http_urls() {
        assert_eq!(
            classify_input("http://insecure-site.org", SEARCH_TMPL),
            NavigationTarget::Url("http://insecure-site.org".to_string())
        );
        assert_eq!(
            classify_input("http://test.com/login", SEARCH_TMPL),
            NavigationTarget::Url("http://test.com/login".to_string())
        );
    }

    #[test]
    fn test_bare_domains() {
        assert_eq!(
            classify_input("example.com", SEARCH_TMPL),
            NavigationTarget::Url("https://example.com".to_string())
        );
        assert_eq!(
            classify_input("crates.io/crates/webkit6", SEARCH_TMPL),
            NavigationTarget::Url("https://crates.io/crates/webkit6".to_string())
        );
        assert_eq!(
            classify_input("docs.rs", SEARCH_TMPL),
            NavigationTarget::Url("https://docs.rs".to_string())
        );
        assert_eq!(
            classify_input("news.ycombinator.com", SEARCH_TMPL),
            NavigationTarget::Url("https://news.ycombinator.com".to_string())
        );
    }

    #[test]
    fn test_localhost_and_ports() {
        assert_eq!(
            classify_input("localhost", SEARCH_TMPL),
            NavigationTarget::Url("http://localhost".to_string())
        );
        assert_eq!(
            classify_input("localhost:3000", SEARCH_TMPL),
            NavigationTarget::Url("http://localhost:3000".to_string())
        );
        assert_eq!(
            classify_input("localhost:8080/api/v1", SEARCH_TMPL),
            NavigationTarget::Url("http://localhost:8080/api/v1".to_string())
        );
    }

    #[test]
    fn test_ipv4_and_ipv6_literals() {
        assert_eq!(
            classify_input("127.0.0.1", SEARCH_TMPL),
            NavigationTarget::Url("http://127.0.0.1".to_string())
        );
        assert_eq!(
            classify_input("192.168.1.1:8080", SEARCH_TMPL),
            NavigationTarget::Url("http://192.168.1.1:8080".to_string())
        );
        assert_eq!(
            classify_input("[::1]", SEARCH_TMPL),
            NavigationTarget::Url("http://[::1]".to_string())
        );
        assert_eq!(
            classify_input("[::1]:8000/test", SEARCH_TMPL),
            NavigationTarget::Url("http://[::1]:8000/test".to_string())
        );
    }

    #[test]
    fn test_blocked_dangerous_schemes() {
        match classify_input("javascript:alert(1)", SEARCH_TMPL) {
            NavigationTarget::Blocked { scheme, .. } => assert_eq!(scheme, "javascript"),
            other => panic!("Expected Blocked, got {other:?}"),
        }

        match classify_input("data:text/html,<h1>Phishing</h1>", SEARCH_TMPL) {
            NavigationTarget::Blocked { scheme, .. } => assert_eq!(scheme, "data"),
            other => panic!("Expected Blocked, got {other:?}"),
        }

        match classify_input("file:///etc/passwd", SEARCH_TMPL) {
            NavigationTarget::Blocked { scheme, .. } => assert_eq!(scheme, "file"),
            other => panic!("Expected Blocked, got {other:?}"),
        }

        match classify_input("ftp://ftp.example.com", SEARCH_TMPL) {
            NavigationTarget::Blocked { scheme, .. } => assert_eq!(scheme, "ftp"),
            other => panic!("Expected Blocked, got {other:?}"),
        }

        match classify_input("blob:https://example.com/uuid", SEARCH_TMPL) {
            NavigationTarget::Blocked { scheme, .. } => assert_eq!(scheme, "blob"),
            other => panic!("Expected Blocked, got {other:?}"),
        }

        match classify_input("steam://rungameid/730", SEARCH_TMPL) {
            NavigationTarget::Blocked { scheme, .. } => assert_eq!(scheme, "steam"),
            other => panic!("Expected Blocked, got {other:?}"),
        }
    }

    #[test]
    fn test_malformed_schemes() {
        match classify_input("http://", SEARCH_TMPL) {
            NavigationTarget::Blocked { scheme, .. } => assert_eq!(scheme, "http"),
            other => panic!("Expected Blocked, got {other:?}"),
        }
        match classify_input("https://", SEARCH_TMPL) {
            NavigationTarget::Blocked { scheme, .. } => assert_eq!(scheme, "https"),
            other => panic!("Expected Blocked, got {other:?}"),
        }
    }

    #[test]
    fn test_search_queries() {
        assert_eq!(
            classify_input("rust programming language", SEARCH_TMPL),
            NavigationTarget::Search(
                "https://duckduckgo.com/?q=rust%20programming%20language".to_string()
            )
        );
        assert_eq!(
            classify_input("what is gtk4", SEARCH_TMPL),
            NavigationTarget::Search("https://duckduckgo.com/?q=what%20is%20gtk4".to_string())
        );
        assert_eq!(
            classify_input("search?query=123", SEARCH_TMPL),
            NavigationTarget::Search("https://duckduckgo.com/?q=search%3Fquery%3D123".to_string())
        );
        assert_eq!(
            classify_input("singleword", SEARCH_TMPL),
            NavigationTarget::Search("https://duckduckgo.com/?q=singleword".to_string())
        );
    }

    #[test]
    fn test_unicode_and_bangla_search() {
        assert_eq!(
            classify_input("বাংলাদেশ", SEARCH_TMPL),
            NavigationTarget::Search(format!(
                "https://duckduckgo.com/?q={}",
                glib::Uri::escape_string("বাংলাদেশ", None, false)
            ))
        );
        assert_eq!(
            classify_input("স্বাধীন ব্রাউজার বৈশিষ্ট্য", SEARCH_TMPL),
            NavigationTarget::Search(format!(
                "https://duckduckgo.com/?q={}",
                glib::Uri::escape_string("স্বাধীন ব্রাউজার বৈশিষ্ট্য", None, false)
            ))
        );
        assert_eq!(
            classify_input("ঢাকা বিশ্ববিদ্যালয় খবর", SEARCH_TMPL),
            NavigationTarget::Search(format!(
                "https://duckduckgo.com/?q={}",
                glib::Uri::escape_string("ঢাকা বিশ্ববিদ্যালয় খবর", None, false)
            ))
        );
    }

    #[test]
    fn test_international_domain_names() {
        assert_eq!(
            classify_input("বাংলা.ভারত", SEARCH_TMPL),
            NavigationTarget::Url("https://বাংলা.ভারত".to_string())
        );
        assert_eq!(
            classify_input("xn--54b7fta0cc.xn--54b7fta0cc", SEARCH_TMPL),
            NavigationTarget::Url("https://xn--54b7fta0cc.xn--54b7fta0cc".to_string())
        );
    }

    #[test]
    fn test_leading_trailing_whitespace_handling() {
        assert_eq!(
            classify_input("   https://example.com   ", SEARCH_TMPL),
            NavigationTarget::Url("https://example.com".to_string())
        );
        assert_eq!(
            classify_input("  rust lang  ", SEARCH_TMPL),
            NavigationTarget::Search("https://duckduckgo.com/?q=rust%20lang".to_string())
        );
    }

    #[test]
    fn test_extract_display_host() {
        assert_eq!(
            extract_display_host("https://example.com/test"),
            Some("example.com".to_string())
        );
        assert_eq!(
            extract_display_host("http://localhost:8080/app"),
            Some("localhost".to_string())
        );
        assert_eq!(
            extract_display_host("https://sub.domain.co.uk:9000/"),
            Some("sub.domain.co.uk".to_string())
        );
    }

    #[test]
    fn test_case_insensitivity_and_upper_schemes() {
        assert_eq!(
            classify_input("HTTPS://EXAMPLE.COM", SEARCH_TMPL),
            NavigationTarget::Url("HTTPS://EXAMPLE.COM".to_string())
        );
        match classify_input("JAVASCRIPT:void(0)", SEARCH_TMPL) {
            NavigationTarget::Blocked { scheme, .. } => assert_eq!(scheme, "javascript"),
            other => panic!("Expected Blocked, got {other:?}"),
        }
        match classify_input("FILE:///etc/shadow", SEARCH_TMPL) {
            NavigationTarget::Blocked { scheme, .. } => assert_eq!(scheme, "file"),
            other => panic!("Expected Blocked, got {other:?}"),
        }
        match classify_input("DATA:text/plain;base64,SGVsbG8=", SEARCH_TMPL) {
            NavigationTarget::Blocked { scheme, .. } => assert_eq!(scheme, "data"),
            other => panic!("Expected Blocked, got {other:?}"),
        }
    }

    #[test]
    fn test_complex_bangla_and_unicode_queries() {
        assert_eq!(
            classify_input("বাংলা ভাষার ইতিহাস কী?", SEARCH_TMPL),
            NavigationTarget::Search(format!(
                "https://duckduckgo.com/?q={}",
                glib::Uri::escape_string("বাংলা ভাষার ইতিহাস কী?", None, false)
            ))
        );
        assert_eq!(
            classify_input("রবীন্দ্রনাথ ঠাকুর কবিতা", SEARCH_TMPL),
            NavigationTarget::Search(format!(
                "https://duckduckgo.com/?q={}",
                glib::Uri::escape_string("রবীন্দ্রনাথ ঠাকুর কবিতা", None, false)
            ))
        );
    }

    #[test]
    fn test_bare_domain_with_port_and_path() {
        assert_eq!(
            classify_input("my-service.local:9090/dashboard", SEARCH_TMPL),
            NavigationTarget::Url("https://my-service.local:9090/dashboard".to_string())
        );
        assert_eq!(
            classify_input("10.0.0.1:8000/metrics", SEARCH_TMPL),
            NavigationTarget::Url("http://10.0.0.1:8000/metrics".to_string())
        );
    }
}
