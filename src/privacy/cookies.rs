//! Cookie accept policies, Intelligent Tracking Prevention (ITP), and site data management

use gtk4::gio;
use gtk4::glib;
use tracing::{error, info};
use webkit6::{CookieAcceptPolicy, NetworkSession, WebsiteDataTypes};

/// High-level cookie acceptance policy options
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum CookiePolicy {
    /// Accept all cookies (first-party and third-party)
    AllowAll,
    /// Block third-party / cross-site tracking cookies (Default)
    #[default]
    BlockThirdParty,
    /// Reject all cookies
    BlockAll,
}

impl CookiePolicy {
    pub fn from_str_opt(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "allow_all" | "all" => Self::AllowAll,
            "block_all" => Self::BlockAll,
            _ => Self::BlockThirdParty,
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            Self::AllowAll => "block_third_party",
            Self::BlockThirdParty => "block_third_party",
            Self::BlockAll => "block_all",
        }
    }

    /// Maps to WebKitGTK 6.0 engine CookieAcceptPolicy
    pub fn to_webkit_policy(self) -> CookieAcceptPolicy {
        match self {
            Self::AllowAll => CookieAcceptPolicy::Always,
            Self::BlockThirdParty => CookieAcceptPolicy::NoThirdParty,
            Self::BlockAll => CookieAcceptPolicy::Never,
        }
    }
}

/// Applies cookie accept policy to a WebKit NetworkSession
pub fn apply_cookie_policy(session: &NetworkSession, policy: CookiePolicy) {
    if let Some(cookie_mgr) = session.cookie_manager() {
        let webkit_policy = policy.to_webkit_policy();
        info!("Applying CookieAcceptPolicy {:?} to session", webkit_policy);
        cookie_mgr.set_accept_policy(webkit_policy);
    }
}

/// Applies Intelligent Tracking Prevention (ITP) to a WebKit NetworkSession
pub fn apply_itp(session: &NetworkSession, enabled: bool) {
    info!("Setting Intelligent Tracking Prevention (ITP) = {enabled} on NetworkSession");
    session.set_itp_enabled(enabled);
}

/// Time range selection for Clear Browsing Data dialog
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ClearTimeRange {
    LastHour,
    Last24Hours,
    Last7Days,
    #[default]
    AllTime,
}

impl ClearTimeRange {
    /// Converts to GLib TimeSpan (in microseconds) for `WebsiteDataManager::clear`
    pub fn to_timespan(self) -> glib::TimeSpan {
        match self {
            Self::LastHour => glib::TimeSpan::from_seconds(3600),
            Self::Last24Hours => glib::TimeSpan::from_seconds(86400),
            Self::Last7Days => glib::TimeSpan::from_seconds(7 * 86400),
            Self::AllTime => glib::TimeSpan::from_seconds(0), // 0 means all time in WebKit
        }
    }

    /// Converts to Unix epoch cutoff timestamp in seconds
    pub fn to_cutoff_timestamp(self) -> i64 {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs() as i64;

        match self {
            Self::LastHour => now - 3600,
            Self::Last24Hours => now - 86400,
            Self::Last7Days => now - (7 * 86400),
            Self::AllTime => 0,
        }
    }
}

/// Specification of data categories to clear
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct ClearDataSelection {
    pub history: bool,
    pub downloads: bool,
    pub cookies_and_site_data: bool,
    pub cache: bool,
    pub permissions: bool,
    pub time_range: ClearTimeRange,
}

/// Summary record for stored per-site data
#[derive(Debug, Clone)]
pub struct SiteDataRecord {
    pub name: String,
    pub types: String,
}

/// Fetches list of all sites with stored data in the WebsiteDataManager
pub fn fetch_stored_site_data<F: FnOnce(Vec<SiteDataRecord>) + 'static>(
    session: &NetworkSession,
    callback: F,
) {
    if let Some(wdm) = session.website_data_manager() {
        wdm.fetch(
            WebsiteDataTypes::ALL,
            Option::<&gio::Cancellable>::None,
            move |res| match res {
                Ok(data_list) => {
                    let mut records = Vec::new();
                    for d in data_list {
                        let name = d.name().map(|s| s.to_string()).unwrap_or_default();
                        let types_flag = d.types();
                        let mut types_desc = Vec::new();
                        if types_flag.contains(WebsiteDataTypes::COOKIES) {
                            types_desc.push("Cookies");
                        }
                        if types_flag.contains(WebsiteDataTypes::LOCAL_STORAGE) {
                            types_desc.push("Local Storage");
                        }
                        if types_flag.contains(WebsiteDataTypes::DISK_CACHE)
                            || types_flag.contains(WebsiteDataTypes::MEMORY_CACHE)
                        {
                            types_desc.push("Cache");
                        }
                        if types_flag.contains(WebsiteDataTypes::INDEXEDDB_DATABASES) {
                            types_desc.push("IndexedDB");
                        }

                        records.push(SiteDataRecord {
                            name,
                            types: types_desc.join(", "),
                        });
                    }
                    records.sort_by(|a, b| a.name.cmp(&b.name));
                    callback(records);
                }
                Err(e) => {
                    error!("Failed to fetch website data: {e}");
                    callback(Vec::new());
                }
            },
        );
    } else {
        callback(Vec::new());
    }
}

/// Clears WebKit website data (cookies, storage, cache) according to selection
pub fn clear_website_data<F: FnOnce() + Send + 'static>(
    session: &NetworkSession,
    selection: ClearDataSelection,
    callback: F,
) {
    if let Some(wdm) = session.website_data_manager() {
        let mut types = WebsiteDataTypes::empty();
        if selection.cookies_and_site_data {
            types.insert(
                WebsiteDataTypes::COOKIES
                    | WebsiteDataTypes::LOCAL_STORAGE
                    | WebsiteDataTypes::INDEXEDDB_DATABASES
                    | WebsiteDataTypes::SESSION_STORAGE,
            );
        }
        if selection.cache {
            types.insert(WebsiteDataTypes::DISK_CACHE | WebsiteDataTypes::MEMORY_CACHE);
        }

        if types.is_empty() {
            callback();
            return;
        }

        let timespan = selection.time_range.to_timespan();
        info!("Executing WebsiteDataManager clear with types: {:?}", types);

        wdm.clear(
            types,
            timespan,
            Option::<&gio::Cancellable>::None,
            move |res| {
                if let Err(e) = res {
                    error!("WebsiteDataManager clear returned error: {e}");
                } else {
                    info!("WebsiteDataManager cleared successfully");
                }
                callback();
            },
        );
    } else {
        callback();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cookie_policy_mapping() {
        assert_eq!(
            CookiePolicy::AllowAll.to_webkit_policy(),
            CookieAcceptPolicy::Always
        );
        assert_eq!(
            CookiePolicy::BlockThirdParty.to_webkit_policy(),
            CookieAcceptPolicy::NoThirdParty
        );
        assert_eq!(
            CookiePolicy::BlockAll.to_webkit_policy(),
            CookieAcceptPolicy::Never
        );
    }

    #[test]
    fn test_clear_time_range_calculations() {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs() as i64;

        assert_eq!(ClearTimeRange::AllTime.to_cutoff_timestamp(), 0);
        assert!(ClearTimeRange::LastHour.to_cutoff_timestamp() <= now - 3590);
        assert!(ClearTimeRange::Last24Hours.to_cutoff_timestamp() <= now - 86390);
    }
}
