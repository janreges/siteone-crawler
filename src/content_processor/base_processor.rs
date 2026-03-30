// SiteOne Crawler - BaseProcessor shared utilities
// (c) Jan Reges <jan.reges@siteone.cz>
//
// Provides shared utility methods used by all content processors.

use crate::engine::parsed_url::ParsedUrl;
use crate::export::utils::offline_url_converter::OfflineUrlConverter;
use crate::types::ContentTypeId;

/// Configuration extracted from CoreOptions, shared across processors.
/// This avoids each processor needing a reference to the full crawler.
#[derive(Debug, Clone)]
pub struct ProcessorConfig {
    pub single_page: bool,
    pub single_foreign_page: bool,
    pub max_depth: i64,
    pub files_enabled: bool,
    pub images_enabled: bool,
    pub scripts_enabled: bool,
    pub styles_enabled: bool,
    pub fonts_enabled: bool,
    pub disable_javascript: bool,
    pub remove_all_anchor_listeners: bool,
    pub ignore_regex: Vec<String>,
    /// Pre-compiled ignore regexes for hot path usage
    pub compiled_ignore_regex: Vec<regex::Regex>,
    pub disable_astro_inline_modules: bool,
    pub offline_export_preserve_urls: bool,
    pub initial_url: ParsedUrl,
}

impl ProcessorConfig {
    pub fn new(initial_url: ParsedUrl) -> Self {
        Self {
            single_page: false,
            single_foreign_page: false,
            max_depth: 0,
            files_enabled: true,
            images_enabled: true,
            scripts_enabled: true,
            styles_enabled: true,
            fonts_enabled: true,
            disable_javascript: false,
            remove_all_anchor_listeners: false,
            ignore_regex: Vec::new(),
            compiled_ignore_regex: Vec::new(),
            disable_astro_inline_modules: false,
            offline_export_preserve_urls: false,
            initial_url,
        }
    }

    /// Compile ignore_regex patterns into Regex objects for hot path usage.
    /// Call this after setting ignore_regex.
    pub fn compile_ignore_regex(&mut self) {
        self.compiled_ignore_regex = self
            .ignore_regex
            .iter()
            .filter_map(|pattern| regex::Regex::new(pattern).ok())
            .collect();
    }
}

/// Check if a content type is in the list of relevant types
pub fn is_relevant(content_type: ContentTypeId, relevant_types: &[ContentTypeId]) -> bool {
    relevant_types.contains(&content_type)
}

/// Normalize a URL path by resolving `.` and `..` segments.
fn normalize_path(path: &str) -> String {
    let mut segments: Vec<&str> = Vec::new();
    for segment in path.split('/') {
        match segment {
            "." => {}
            ".." => {
                segments.pop();
            }
            s => segments.push(s),
        }
    }
    let result = segments.join("/");
    if result.starts_with('/') {
        result
    } else {
        format!("/{}", result)
    }
}

/// Convert a URL to a relative path for offline use.
/// When `preserve_urls` is true, same-domain links become root-relative and cross-domain links stay absolute.
pub fn convert_url_to_relative(
    base_url: &ParsedUrl,
    target_url: &str,
    initial_url: &ParsedUrl,
    attribute: Option<&str>,
    preserve_urls: bool,
) -> String {
    // If it's a data URI, anchor, or non-http scheme, return as-is
    if target_url.starts_with("data:")
        || target_url.starts_with("javascript:")
        || target_url.starts_with("mailto:")
        || target_url.starts_with("tel:")
    {
        return target_url.to_string();
    }

    // Normalize HTML entities in URL before parsing so it matches what FoundUrl stored.
    // Only decode entities (not full normalize_url which also trims trailing &, quotes, etc.
    // — those transformations are for discovery, not for offline conversion of already-parsed URLs).
    let normalized = target_url.replace("&#38;", "&").replace("&amp;", "&");
    let parsed_target = ParsedUrl::parse(&normalized, Some(base_url));

    if preserve_urls {
        let target_host = parsed_target.host.as_deref().unwrap_or("");
        let initial_host = initial_url.host.as_deref().unwrap_or("");
        if target_host.is_empty() || target_host == initial_host {
            // Same domain → root-relative (path + query + fragment)
            // Normalize path segments (resolve .. and .)
            let normalized_path = normalize_path(&parsed_target.path);
            let mut result = normalized_path;
            if let Some(ref q) = parsed_target.query {
                result.push('?');
                result.push_str(q);
            }
            if let Some(ref f) = parsed_target.fragment {
                result.push('#');
                result.push_str(f);
            }
            return result;
        } else {
            // Cross domain → full absolute URL
            return parsed_target.get_full_url(true, true);
        }
    }

    let mut converter = OfflineUrlConverter::new(
        initial_url.clone(),
        base_url.clone(),
        parsed_target,
        None,
        None,
        attribute,
    );

    converter.convert_url_to_relative(true)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn initial_url() -> ParsedUrl {
        ParsedUrl::parse("https://example.com/", None)
    }

    #[test]
    fn decode_amp_entity_before_offline_conversion() {
        let base = ParsedUrl::parse("https://example.com/blog/", None);
        let result = convert_url_to_relative(&base, "/style.css?v=1&amp;t=2", &initial_url(), Some("href"), false);
        // &amp; must be decoded to & so the query hash matches what FoundUrl stored
        assert!(
            !result.contains("&amp;"),
            "HTML entity &amp; should be decoded before conversion"
        );
    }

    #[test]
    fn decode_numeric_entity_before_offline_conversion() {
        let base = ParsedUrl::parse("https://example.com/", None);
        let result = convert_url_to_relative(&base, "/page?a=1&#38;b=2", &initial_url(), Some("href"), false);
        assert!(
            !result.contains("&#38;"),
            "HTML entity &#38; should be decoded before conversion"
        );
    }

    #[test]
    fn preserve_trailing_ampersand() {
        // Trailing & in a query string should NOT be stripped (unlike in FoundUrl discovery)
        let base = ParsedUrl::parse("https://example.com/", None);
        let a = convert_url_to_relative(&base, "/page?a=1&", &initial_url(), Some("href"), false);
        let b = convert_url_to_relative(&base, "/page?a=1&b=", &initial_url(), Some("href"), false);
        // Both should produce different results (trailing & matters for hash)
        assert_ne!(a, b, "trailing & should be preserved, not stripped");
    }

    #[test]
    fn skip_data_uri() {
        let base = ParsedUrl::parse("https://example.com/", None);
        let result = convert_url_to_relative(&base, "data:image/png;base64,abc", &initial_url(), None, false);
        assert_eq!(result, "data:image/png;base64,abc");
    }

    #[test]
    fn skip_javascript_uri() {
        let base = ParsedUrl::parse("https://example.com/", None);
        let result = convert_url_to_relative(&base, "javascript:void(0)", &initial_url(), None, false);
        assert_eq!(result, "javascript:void(0)");
    }

    // --- preserve_urls tests ---

    #[test]
    fn preserve_urls_same_domain_absolute() {
        let base = ParsedUrl::parse("https://example.com/blog/post", None);
        let result = convert_url_to_relative(
            &base,
            "https://example.com/designy/classic",
            &initial_url(),
            Some("href"),
            true,
        );
        assert_eq!(result, "/designy/classic");
    }

    #[test]
    fn preserve_urls_same_domain_root_relative() {
        let base = ParsedUrl::parse("https://example.com/blog/post", None);
        let result = convert_url_to_relative(&base, "/about", &initial_url(), Some("href"), true);
        assert_eq!(result, "/about");
    }

    #[test]
    fn preserve_urls_same_domain_relative() {
        let base = ParsedUrl::parse("https://example.com/blog/post", None);
        let result = convert_url_to_relative(&base, "../images/logo.png", &initial_url(), Some("src"), true);
        assert_eq!(result, "/images/logo.png");
    }

    #[test]
    fn preserve_urls_cross_domain() {
        let base = ParsedUrl::parse("https://example.com/page", None);
        let result = convert_url_to_relative(
            &base,
            "https://cdn.other.com/style.css",
            &initial_url(),
            Some("href"),
            true,
        );
        assert_eq!(result, "https://cdn.other.com/style.css");
    }

    #[test]
    fn preserve_urls_with_query_and_fragment() {
        let base = ParsedUrl::parse("https://example.com/", None);
        let result = convert_url_to_relative(&base, "/page?key=val#section", &initial_url(), Some("href"), true);
        assert_eq!(result, "/page?key=val#section");
    }

    #[test]
    fn preserve_urls_data_uri_unchanged() {
        let base = ParsedUrl::parse("https://example.com/", None);
        let result = convert_url_to_relative(&base, "data:image/png;base64,abc", &initial_url(), None, true);
        assert_eq!(result, "data:image/png;base64,abc");
    }

    #[test]
    fn preserve_urls_mailto_unchanged() {
        let base = ParsedUrl::parse("https://example.com/", None);
        let result = convert_url_to_relative(&base, "mailto:test@example.com", &initial_url(), None, true);
        assert_eq!(result, "mailto:test@example.com");
    }
}
