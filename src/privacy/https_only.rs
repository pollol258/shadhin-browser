//! HTTPS-only enforcement, automatic main-frame upgrades, and safe fallback handling

use crate::browser::tabs::TabId;
use std::collections::{HashMap, HashSet};
use std::net::IpAddr;

/// HTTPS upgrade policy mode
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum HttpsMode {
    /// No automatic upgrades; navigations proceed with requested scheme
    Off,
    /// Automatically try HTTPS first; show themed interstitial if HTTPS fails (Default)
    #[default]
    Prefer,
    /// Block all insecure HTTP connections without exception
    Strict,
}

impl HttpsMode {
    pub fn from_str_opt(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "off" => Self::Off,
            "strict" => Self::Strict,
            _ => Self::Prefer,
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Off => "off",
            Self::Prefer => "prefer",
            Self::Strict => "strict",
        }
    }
}

/// Checks if a URI is exempt from HTTPS upgrade attempts (localhost, loopback, private RFC1918 IPs)
pub fn is_exempt_from_https_upgrade(uri: &str) -> bool {
    let host = match extract_host_from_uri(uri) {
        Some(h) => h,
        None => return true,
    };

    let host_lower = host.to_lowercase();

    // 1. Localhost and local domain names
    if host_lower == "localhost"
        || host_lower.ends_with(".localhost")
        || host_lower.ends_with(".local")
        || host_lower.ends_with(".test")
    {
        return true;
    }

    // 2. IP literal addresses
    let ip_str = host_lower.trim_start_matches('[').trim_end_matches(']');
    if let Ok(ip) = ip_str.parse::<IpAddr>() {
        match ip {
            IpAddr::V4(v4) => {
                // Loopback (127.0.0.0/8)
                if v4.is_loopback() {
                    return true;
                }
                // Private IPv4 ranges (10.0.0.0/8, 172.16.0.0/12, 192.168.0.0/16)
                if v4.is_private() {
                    return true;
                }
                // Link-local (169.254.0.0/16)
                if v4.is_link_local() {
                    return true;
                }
            }
            IpAddr::V6(v6) => {
                // IPv6 Loopback (::1)
                if v6.is_loopback() {
                    return true;
                }
                // Unique Local Address (fc00::/7)
                let segments = v6.segments();
                if (segments[0] & 0xfe00) == 0xfc00 {
                    return true;
                }
            }
        }
    }

    false
}

/// Upgrades an HTTP URI to HTTPS, preserving query parameters and path
pub fn upgrade_uri_to_https(uri: &str) -> Option<String> {
    uri.strip_prefix("http://")
        .map(|stripped| format!("https://{stripped}"))
}

/// Extracts the host string from a URI
pub fn extract_host_from_uri(uri: &str) -> Option<String> {
    let without_scheme = uri
        .strip_prefix("http://")
        .or_else(|| uri.strip_prefix("https://"))?;

    let host_port = without_scheme.split(['/', '?', '#']).next()?;
    let host = if host_port.starts_with('[') {
        // IPv6 bracketed
        if let Some(end) = host_port.find(']') {
            &host_port[..=end]
        } else {
            host_port
        }
    } else {
        host_port.split(':').next()?
    };

    if host.is_empty() {
        None
    } else {
        Some(host.to_string())
    }
}

/// Information about an in-flight HTTPS upgrade attempt for a specific tab
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PendingUpgrade {
    pub original_http_uri: String,
    pub upgraded_https_uri: String,
    pub host: String,
    pub attempt_count: u32,
}

/// Coordinator for HTTPS upgrades and session-specific fallback exemptions
#[derive(Debug, Clone, Default)]
pub struct HttpsUpgradeManager {
    session_exemptions: HashSet<String>,
    pending_upgrades: HashMap<TabId, PendingUpgrade>,
}

impl HttpsUpgradeManager {
    pub fn new() -> Self {
        Self {
            session_exemptions: HashSet::new(),
            pending_upgrades: HashMap::new(),
        }
    }

    /// Records an active attempt to upgrade a tab's navigation to HTTPS.
    /// If an upgrade is already pending for this tab and URL, increments attempt count.
    pub fn record_upgrade_attempt(
        &mut self,
        tab_id: TabId,
        original_uri: &str,
        upgraded_uri: &str,
        host: &str,
    ) {
        let host_lower = host.to_lowercase();
        if let Some(existing) = self.pending_upgrades.get_mut(&tab_id)
            && existing.original_http_uri == original_uri
        {
            existing.attempt_count += 1;
            return;
        }

        self.pending_upgrades.insert(
            tab_id,
            PendingUpgrade {
                original_http_uri: original_uri.to_string(),
                upgraded_https_uri: upgraded_uri.to_string(),
                host: host_lower,
                attempt_count: 1,
            },
        );
    }

    /// Returns a reference to the pending upgrade for a tab if one exists
    pub fn get_pending_upgrade(&self, tab_id: TabId) -> Option<&PendingUpgrade> {
        self.pending_upgrades.get(&tab_id)
    }

    /// Checks if a tab has an active pending HTTPS upgrade
    pub fn has_pending_upgrade(&self, tab_id: TabId) -> bool {
        self.pending_upgrades.contains_key(&tab_id)
    }

    /// Clears and returns the pending upgrade attempt record for a tab
    pub fn clear_upgrade_attempt(&mut self, tab_id: TabId) -> Option<PendingUpgrade> {
        self.pending_upgrades.remove(&tab_id)
    }

    /// Detects if an HTTP navigation is entering a redirect loop.
    /// A redirect loop occurs when an upgraded site attempts to redirect back to the
    /// same insecure HTTP URI that was already upgraded (attempt_count >= 2).
    pub fn is_redirect_loop(&self, tab_id: TabId, uri: &str) -> bool {
        if let Some(pending) = self.pending_upgrades.get(&tab_id)
            && pending.original_http_uri == uri
            && pending.attempt_count >= 2
        {
            return true;
        }
        false
    }

    /// Evaluates whether an HTTP navigation should be upgraded to HTTPS
    pub fn should_upgrade(&self, uri: &str, mode: HttpsMode) -> bool {
        if mode == HttpsMode::Off {
            return false;
        }

        if !uri.starts_with("http://") {
            return false;
        }

        // Exempt localhost and private networks
        if is_exempt_from_https_upgrade(uri) {
            return false;
        }

        // Exempt hosts explicitly bypassed by user for this session
        if let Some(host) = extract_host_from_uri(uri)
            && self.session_exemptions.contains(&host.to_lowercase())
        {
            return false;
        }

        true
    }

    /// Records a user's temporary decision to continue over plain HTTP for this host
    pub fn add_session_exemption(&mut self, host: &str) {
        self.session_exemptions.insert(host.to_lowercase());
    }

    /// Checks if a host is currently exempt from HTTPS upgrades in this session
    pub fn is_session_exempt(&self, host: &str) -> bool {
        self.session_exemptions.contains(&host.to_lowercase())
    }

    /// Clears all temporary exemptions
    pub fn clear_session_exemptions(&mut self) {
        self.session_exemptions.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_exempt_from_https_upgrade() {
        // Localhost
        assert!(is_exempt_from_https_upgrade("http://localhost:8080/app"));
        assert!(is_exempt_from_https_upgrade("http://myhost.localhost/"));
        assert!(is_exempt_from_https_upgrade("http://dev.test/"));

        // Loopback IPv4
        assert!(is_exempt_from_https_upgrade("http://127.0.0.1/"));
        assert!(is_exempt_from_https_upgrade(
            "http://127.0.0.1:8089/page1.html"
        ));
        assert!(is_exempt_from_https_upgrade("http://127.0.1.1:3000/"));

        // Private IPv4 (RFC 1918)
        assert!(is_exempt_from_https_upgrade("http://192.168.1.1/admin"));
        assert!(is_exempt_from_https_upgrade("http://10.0.0.5:8080/"));
        assert!(is_exempt_from_https_upgrade("http://172.16.0.1/"));

        // IPv6 Loopback
        assert!(is_exempt_from_https_upgrade("http://[::1]:8080/"));

        // Public sites (MUST NOT be exempt)
        assert!(!is_exempt_from_https_upgrade("http://example.com/"));
        assert!(!is_exempt_from_https_upgrade("http://8.8.8.8/"));
        assert!(!is_exempt_from_https_upgrade(
            "http://wikipedia.org/wiki/Main_Page"
        ));
    }

    #[test]
    fn test_upgrade_uri_to_https() {
        assert_eq!(
            upgrade_uri_to_https("http://example.com/login?ref=1"),
            Some("https://example.com/login?ref=1".to_string())
        );
        assert_eq!(upgrade_uri_to_https("https://already-secure.org"), None);
        assert_eq!(upgrade_uri_to_https("shadhin:settings"), None);
    }

    #[test]
    fn test_https_upgrade_manager_lifecycle() {
        let mut mgr = HttpsUpgradeManager::new();

        // Normal public site should upgrade under Prefer mode
        assert!(mgr.should_upgrade("http://news.ycombinator.com", HttpsMode::Prefer));

        // Off mode disables upgrade
        assert!(!mgr.should_upgrade("http://news.ycombinator.com", HttpsMode::Off));

        // Add session exemption
        mgr.add_session_exemption("news.ycombinator.com");
        assert!(!mgr.should_upgrade("http://news.ycombinator.com", HttpsMode::Prefer));

        // Other hosts still upgrade
        assert!(mgr.should_upgrade("http://other.org", HttpsMode::Prefer));

        // Clear exemptions
        mgr.clear_session_exemptions();
        assert!(mgr.should_upgrade("http://news.ycombinator.com", HttpsMode::Prefer));
    }

    #[test]
    fn test_per_tab_pending_upgrade_isolation() {
        let mut mgr = HttpsUpgradeManager::new();
        let tab1 = 101;
        let tab2 = 102;

        let uri1 = "http://site-a.com/path?arg=1";
        let uri2 = "http://site-b.com/sub/page";

        mgr.record_upgrade_attempt(tab1, uri1, "https://site-a.com/path?arg=1", "site-a.com");

        assert!(mgr.has_pending_upgrade(tab1));
        assert!(!mgr.has_pending_upgrade(tab2));

        let pending1 = mgr.get_pending_upgrade(tab1).unwrap();
        assert_eq!(pending1.original_http_uri, uri1);
        assert_eq!(pending1.upgraded_https_uri, "https://site-a.com/path?arg=1");
        assert_eq!(pending1.host, "site-a.com");
        assert_eq!(pending1.attempt_count, 1);

        // Record for tab2
        mgr.record_upgrade_attempt(tab2, uri2, "https://site-b.com/sub/page", "site-b.com");
        assert!(mgr.has_pending_upgrade(tab2));

        // Clearing tab1 leaves tab2 intact
        let cleared1 = mgr.clear_upgrade_attempt(tab1).unwrap();
        assert_eq!(cleared1.original_http_uri, uri1);
        assert!(!mgr.has_pending_upgrade(tab1));
        assert!(mgr.has_pending_upgrade(tab2));

        // Clearing tab2
        assert!(mgr.clear_upgrade_attempt(tab2).is_some());
        assert!(!mgr.has_pending_upgrade(tab2));
    }

    #[test]
    fn test_redirect_loop_detection() {
        let mut mgr = HttpsUpgradeManager::new();
        let tab_id = 42;
        let url = "http://redirect-loop.com/login";

        // First attempt: not a loop
        mgr.record_upgrade_attempt(
            tab_id,
            url,
            "https://redirect-loop.com/login",
            "redirect-loop.com",
        );
        assert!(!mgr.is_redirect_loop(tab_id, url));

        // Second attempt for the same URL: enters redirect loop
        mgr.record_upgrade_attempt(
            tab_id,
            url,
            "https://redirect-loop.com/login",
            "redirect-loop.com",
        );
        assert!(mgr.is_redirect_loop(tab_id, url));

        // Different URL is not a loop
        assert!(!mgr.is_redirect_loop(tab_id, "http://redirect-loop.com/other"));
    }
}
