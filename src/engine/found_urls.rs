// SiteOne Crawler - FoundUrls collection
// (c) Jan Reges <jan.reges@siteone.cz>

use std::collections::HashMap;

use md5::{Digest, Md5};
use once_cell::sync::Lazy;
use regex::Regex;

use super::found_url::{FoundUrl, UrlSource};

/// Regex for detecting non-http scheme URLs (mailto:, javascript:, data:, tel:, etc.)
static NON_HTTP_SCHEME_RE: Lazy<Regex> = Lazy::new(|| Regex::new(r"(?i)^[a-z]+:[a-z0-9]").unwrap());

/// Collection of found URLs, deduplicated by MD5 hash of URL
#[derive(Debug, Clone)]
pub struct FoundUrls {
    found_urls: HashMap<String, FoundUrl>,
}

impl FoundUrls {
    pub fn new() -> Self {
        Self {
            found_urls: HashMap::new(),
        }
    }

    /// Add a found URL, deduplicated by MD5 hash
    pub fn add_url(&mut self, found_url: FoundUrl) {
        let key = md5_hex(&found_url.url);
        self.found_urls.entry(key).or_insert(found_url);
    }

    /// Add URLs from a text array, filtering out invalid ones
    pub fn add_urls_from_text_array(&mut self, urls: &[&str], source_url: &str, source: UrlSource) {
        for url in urls {
            if is_url_valid_for_crawling(url) {
                self.add_url(FoundUrl::new(url, source_url, source));
            }
        }
    }

    /// Get all found URLs
    pub fn get_urls(&self) -> &HashMap<String, FoundUrl> {
        &self.found_urls
    }

    /// Get count of found URLs
    pub fn get_count(&self) -> usize {
        self.found_urls.len()
    }
}

impl Default for FoundUrls {
    fn default() -> Self {
        Self::new()
    }
}

/// Compute MD5 hex hash of a string
fn md5_hex(input: &str) -> String {
    let mut hasher = Md5::new();
    hasher.update(input.as_bytes());
    format!("{:x}", hasher.finalize())
}

/// Check if URL is valid for crawling. Ignored are:
/// - anchor #fragment links
/// - data:, mailto:, javascript: and other non-http(s) links
/// - file:// links
fn is_url_valid_for_crawling(url: &str) -> bool {
    let url = url.trim();
    if url.starts_with('#') {
        return false;
    }
    if NON_HTTP_SCHEME_RE.is_match(url) {
        return false;
    }
    if url.to_lowercase().starts_with("file://") {
        return false;
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_dedup_by_md5() {
        let mut urls = FoundUrls::new();
        urls.add_url(FoundUrl::new("/page", "https://example.com/", UrlSource::AHref));
        urls.add_url(FoundUrl::new("/page", "https://example.com/other", UrlSource::AHref));
        assert_eq!(urls.get_count(), 1);
    }

    #[test]
    fn test_add_urls_from_text_array() {
        let mut urls = FoundUrls::new();
        urls.add_urls_from_text_array(
            &["/page1", "/page2", "#fragment", "mailto:test@test.com", "/page1"],
            "https://example.com/",
            UrlSource::AHref,
        );
        assert_eq!(urls.get_count(), 2);
    }

    #[test]
    fn test_is_url_valid_for_crawling() {
        assert!(is_url_valid_for_crawling("/page"));
        assert!(is_url_valid_for_crawling("https://example.com"));
        assert!(!is_url_valid_for_crawling("#fragment"));
        assert!(!is_url_valid_for_crawling("mailto:test@test.com"));
        assert!(!is_url_valid_for_crawling("javascript:void(0)"));
        assert!(!is_url_valid_for_crawling("data:text/html,test"));
        assert!(!is_url_valid_for_crawling("file:///etc/passwd"));
    }
}
