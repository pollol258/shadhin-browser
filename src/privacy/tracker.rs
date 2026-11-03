//! Tracking protection heuristics, parameter stripping, and tracker definitions

/// Common tracking query parameters used for cross-site link decoration
pub const COMMON_TRACKING_PARAMS: &[&str] = &[
    "utm_source",
    "utm_medium",
    "utm_campaign",
    "utm_term",
    "utm_content",
    "fbclid",
    "gclid",
    "msclkid",
    "mc_eid",
    "dclid",
    "zanpid",
    "igshid",
    "_hsenc",
    "_hsmi",
];

/// Strips known tracking query parameters from an outgoing URL
pub fn strip_tracking_parameters(raw_url: &str) -> String {
    let (base_and_path, query_and_frag) = match raw_url.split_once('?') {
        Some(parts) => parts,
        None => return raw_url.to_string(),
    };

    let (query, fragment) = match query_and_frag.split_once('#') {
        Some((q, f)) => (q, Some(f)),
        None => (query_and_frag, None),
    };

    let filtered_params: Vec<&str> = query
        .split('&')
        .filter(|pair| {
            if pair.is_empty() {
                return false;
            }
            let key = match pair.split_once('=') {
                Some((k, _)) => k,
                None => pair,
            };
            !COMMON_TRACKING_PARAMS.contains(&key)
        })
        .collect();

    let mut out = base_and_path.to_string();
    if !filtered_params.is_empty() {
        out.push('?');
        out.push_str(&filtered_params.join("&"));
    }

    if let Some(f) = fragment {
        out.push('#');
        out.push_str(f);
    }

    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_strip_tracking_parameters() {
        let dirty =
            "https://example.com/article?utm_source=twitter&utm_medium=social&id=123&fbclid=abcdef";
        let clean = strip_tracking_parameters(dirty);
        assert_eq!(clean, "https://example.com/article?id=123");

        let no_params = "https://example.com/home";
        assert_eq!(strip_tracking_parameters(no_params), no_params);

        let other_params = "https://example.com/search?q=rust&lang=en";
        assert_eq!(strip_tracking_parameters(other_params), other_params);

        let only_tracking = "https://example.com/?fbclid=123456";
        assert_eq!(
            strip_tracking_parameters(only_tracking),
            "https://example.com/"
        );

        let with_fragment = "https://example.com/page?utm_source=test#section";
        assert_eq!(
            strip_tracking_parameters(with_fragment),
            "https://example.com/page#section"
        );
    }
}
