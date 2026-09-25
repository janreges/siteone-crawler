// SiteOne Crawler - BaseProcessor shared utilities
// (c) Jan Reges <jan.reges@siteone.cz>
//
// Provides shared utility methods used by all content processors.

use std::collections::HashMap;
use std::sync::Arc;

use crate::engine::parsed_url::ParsedUrl;
use crate::export::utils::offline_url_converter::OfflineUrlConverter;
use crate::result::visited_url::VisitedUrl;
use crate::types::ContentTypeId;
use crate::utils;

/// Predicate that decides whether a domain is allowed (for static files / crawling).
pub type DomainAllowFn = Arc<dyn Fn(&str) -> bool + Send + Sync>;

/// How the crawler stored a URL it visited: its own content, or a redirect to this absolute URL.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StoredUrl {
    Content,
    Redirect(String),
}

/// Looks up how the crawler stored a visited URL (absolute, without fragment); None when it did not.
pub type StoredUrlFn = Arc<dyn Fn(&str) -> Option<StoredUrl> + Send + Sync>;

/// How the crawler stored `visited_urls`: the exported successful responses with their content, the
/// redirects as a redirect to their Location.
pub fn stored_url_lookup(visited_urls: &[VisitedUrl]) -> StoredUrlFn {
    let stored: HashMap<String, StoredUrl> = visited_urls
        .iter()
        .filter_map(|visited| {
            let stored = match visited.status_code {
                200 | 201 => StoredUrl::Content,
                301..=308 => {
                    let location = visited.extras.as_ref()?.get("Location")?;
                    StoredUrl::Redirect(utils::get_absolute_url_by_base_url(&visited.url, location))
                }
                _ => return None,
            };
            Some((ParsedUrl::parse(&visited.url, None).get_full_url(true, false), stored))
        })
        .collect();
    Arc::new(move |url| stored.get(url).cloned())
}

/// Configuration extracted from CoreOptions, shared across processors.
/// This avoids each processor needing a reference to the full crawler.
#[derive(Clone)]
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
    pub offline_export_no_url_rewriting: bool,
    /// `--force-relative-urls`: http/https and www/non-www variants of the initial host are
    /// rewritten like the initial host in the offline export.
    pub force_relative_urls: bool,
    /// `--offline-export-preserve-url-structure`: extension-less pages are stored as
    /// `<path>/index.html` and links must point there.
    pub offline_export_preserve_url_structure: bool,
    /// When true, URLs inside HTML comments (`<!-- ... -->`) are stripped before
    /// URL extraction, so commented links are not crawled or reported as broken.
    pub ignore_html_comments: bool,
    pub initial_url: ParsedUrl,
    /// Returns true if the given host is allowed for downloading static files
    /// (`--allowed-domain-for-external-files`). Used by offline export so that
    /// downloaded external/CDN assets are rewritten to their local copies.
    pub is_domain_allowed_for_static_files: Option<DomainAllowFn>,
    /// Returns true if the given host is allowed for whole-domain crawling
    /// (`--allowed-domain-for-crawling`).
    pub is_external_domain_allowed_for_crawling: Option<DomainAllowFn>,
    /// How the crawler stored each visited URL, known once the crawl is over (set before the
    /// offline and markdown exports). Used by `--force-relative-urls`.
    pub stored_url: Option<StoredUrlFn>,
}

impl std::fmt::Debug for ProcessorConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ProcessorConfig")
            .field("single_page", &self.single_page)
            .field("single_foreign_page", &self.single_foreign_page)
            .field("max_depth", &self.max_depth)
            .field("files_enabled", &self.files_enabled)
            .field("images_enabled", &self.images_enabled)
            .field("scripts_enabled", &self.scripts_enabled)
            .field("styles_enabled", &self.styles_enabled)
            .field("fonts_enabled", &self.fonts_enabled)
            .field("disable_javascript", &self.disable_javascript)
            .field("remove_all_anchor_listeners", &self.remove_all_anchor_listeners)
            .field("ignore_regex", &self.ignore_regex)
            .field("compiled_ignore_regex", &self.compiled_ignore_regex)
            .field("disable_astro_inline_modules", &self.disable_astro_inline_modules)
            .field("offline_export_preserve_urls", &self.offline_export_preserve_urls)
            .field("offline_export_no_url_rewriting", &self.offline_export_no_url_rewriting)
            .field("force_relative_urls", &self.force_relative_urls)
            .field(
                "offline_export_preserve_url_structure",
                &self.offline_export_preserve_url_structure,
            )
            .field("ignore_html_comments", &self.ignore_html_comments)
            .field("initial_url", &self.initial_url)
            .field(
                "is_domain_allowed_for_static_files",
                &self.is_domain_allowed_for_static_files.is_some(),
            )
            .field(
                "is_external_domain_allowed_for_crawling",
                &self.is_external_domain_allowed_for_crawling.is_some(),
            )
            .field("stored_url", &self.stored_url.is_some())
            .finish()
    }
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
            offline_export_no_url_rewriting: false,
            force_relative_urls: false,
            offline_export_preserve_url_structure: false,
            ignore_html_comments: false,
            initial_url,
            is_domain_allowed_for_static_files: None,
            is_external_domain_allowed_for_crawling: None,
            stored_url: None,
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

/// True when `url` is on the initial host or its www/non-www twin: the hosts that
/// `--force-relative-urls` treats as the initial host, whatever their scheme or port.
pub fn is_initial_host_variant(url: &ParsedUrl, initial_url: &ParsedUrl) -> bool {
    match (url.host.as_deref(), initial_url.host.as_deref()) {
        (Some(host), Some(initial_host)) => host
            .strip_prefix("www.")
            .unwrap_or(host)
            .eq_ignore_ascii_case(initial_host.strip_prefix("www.").unwrap_or(initial_host)),
        _ => false,
    }
}

/// Where a reference to `url`, which the crawler fetched, leads: to `url` when it was stored with its
/// content, or to where its redirects (the crawler fetches their targets as they are) end at stored
/// content, keeping the fragment of the reference; to the redirect record `url` itself when they do not.
/// None when nothing was stored for `url`.
fn stored_target(url: &ParsedUrl, config: &ProcessorConfig) -> Option<ParsedUrl> {
    let stored_url = config.stored_url.as_ref()?;
    let mut stored = stored_url(&url.get_full_url(true, false))?;
    let mut target = url.clone();
    // A few hops, so that a redirect loop ends
    for _ in 0..10 {
        let StoredUrl::Redirect(location) = stored else {
            return Some(target);
        };
        target = ParsedUrl::parse(&location, None);
        if target.fragment.is_none() {
            target.set_fragment(url.fragment.clone());
        }
        target.url = target.get_full_url(true, true);
        match stored_url(&target.get_full_url(true, false)) {
            Some(next) => stored = next,
            None => break,
        }
    }
    Some(url.clone())
}

/// Convert a URL to a relative path for offline use, following the offline-export options of `config`.
/// When `offline_export_preserve_urls` is set, same-domain links become root-relative and cross-domain
/// links stay absolute.
pub fn convert_url_to_relative(
    base_url: &ParsedUrl,
    target_url: &str,
    attribute: Option<&str>,
    config: &ProcessorConfig,
) -> String {
    // No URL rewriting → return target URL completely unchanged
    if config.offline_export_no_url_rewriting {
        return target_url.to_string();
    }

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
    let mut normalized = target_url.replace("&#38;", "&").replace("&amp;", "&");
    // A query-only reference (`?page=2`) belongs to the page's own path, not to the site root
    if normalized.starts_with('?') {
        normalized = format!("{}{}", base_url.path, normalized);
    }
    let mut parsed_target = ParsedUrl::parse(&normalized, Some(base_url));

    // --force-relative-urls: http/https and www/non-www variants of the initial host are the initial
    // host (#35): the crawler fetched them from the initial origin, whatever host they were found on.
    // Redirect records are not normalized at all (see HtmlProcessor).
    if config.force_relative_urls && is_initial_host_variant(&parsed_target, &config.initial_url) {
        let mut initial_origin = parsed_target.clone();
        // The crawler fetched the reference resolved against the page: resolve a path relative to it
        // (`../next`) and dot segments (`./x/../next`), so the lookup finds that fetch
        if !initial_origin.path.is_empty() {
            let path = if initial_origin.path.starts_with('/') {
                initial_origin.path.clone()
            } else {
                let base_directory = base_url.path.rfind('/').map_or("/", |i| &base_url.path[..=i]);
                format!("{}{}", base_directory, initial_origin.path)
            };
            initial_origin.path = normalize_path(&path);
        }
        initial_origin.set_attributes(&config.initial_url, true, true, true);
        initial_origin.url = initial_origin.get_full_url(true, true);
        let is_on_initial_host_twin =
            base_url.host != config.initial_url.host && is_initial_host_variant(base_url, &config.initial_url);
        match stored_target(&initial_origin, config) {
            Some(target) => parsed_target = target,
            // Nothing known about that fetch: a page stored under the initial host's www twin (the
            // initial URL redirects there, so the pages live under _www.host/) keeps its links within
            // that copy.
            None if is_on_initial_host_twin => {}
            None => parsed_target = initial_origin,
        }
    }

    if config.offline_export_preserve_urls {
        let target_host = parsed_target.host.as_deref().unwrap_or("");
        let initial_host = config.initial_url.host.as_deref().unwrap_or("");
        if target_host.is_empty() || target_host == initial_host {
            // Same domain → root-relative (path + query + fragment)
            // A path relative to the page (../sibling on /a/b/page) starts in the page's directory
            let path = if parsed_target.path.is_empty() || parsed_target.path.starts_with('/') {
                parsed_target.path.clone()
            } else {
                let base_directory = base_url.path.rfind('/').map_or("/", |i| &base_url.path[..=i]);
                format!("{}{}", base_directory, parsed_target.path)
            };
            // Normalize path segments (resolve .. and .)
            let normalized_path = normalize_path(&path);
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
        config.initial_url.clone(),
        base_url.clone(),
        parsed_target,
        config.is_domain_allowed_for_static_files.clone(),
        config.is_external_domain_allowed_for_crawling.clone(),
        attribute,
    );
    converter.set_preserve_url_structure(config.offline_export_preserve_url_structure);

    converter.convert_url_to_relative(true)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn initial_url() -> ParsedUrl {
        ParsedUrl::parse("https://example.com/", None)
    }

    fn config(preserve_urls: bool, no_url_rewriting: bool) -> ProcessorConfig {
        let mut cfg = ProcessorConfig::new(initial_url());
        cfg.offline_export_preserve_urls = preserve_urls;
        cfg.offline_export_no_url_rewriting = no_url_rewriting;
        cfg
    }

    #[test]
    fn decode_amp_entity_before_offline_conversion() {
        let base = ParsedUrl::parse("https://example.com/blog/", None);
        let result = convert_url_to_relative(&base, "/style.css?v=1&amp;t=2", Some("href"), &config(false, false));
        // &amp; must be decoded to & so the query hash matches what FoundUrl stored
        assert!(
            !result.contains("&amp;"),
            "HTML entity &amp; should be decoded before conversion"
        );
    }

    #[test]
    fn decode_numeric_entity_before_offline_conversion() {
        let base = ParsedUrl::parse("https://example.com/", None);
        let result = convert_url_to_relative(&base, "/page?a=1&#38;b=2", Some("href"), &config(false, false));
        assert!(
            !result.contains("&#38;"),
            "HTML entity &#38; should be decoded before conversion"
        );
    }

    #[test]
    fn preserve_trailing_ampersand() {
        // Trailing & in a query string should NOT be stripped (unlike in FoundUrl discovery)
        let base = ParsedUrl::parse("https://example.com/", None);
        let a = convert_url_to_relative(&base, "/page?a=1&", Some("href"), &config(false, false));
        let b = convert_url_to_relative(&base, "/page?a=1&b=", Some("href"), &config(false, false));
        // Both should produce different results (trailing & matters for hash)
        assert_ne!(a, b, "trailing & should be preserved, not stripped");
    }

    #[test]
    fn skip_data_uri() {
        let base = ParsedUrl::parse("https://example.com/", None);
        let result = convert_url_to_relative(&base, "data:image/png;base64,abc", None, &config(false, false));
        assert_eq!(result, "data:image/png;base64,abc");
    }

    #[test]
    fn skip_javascript_uri() {
        let base = ParsedUrl::parse("https://example.com/", None);
        let result = convert_url_to_relative(&base, "javascript:void(0)", None, &config(false, false));
        assert_eq!(result, "javascript:void(0)");
    }

    // --- preserve_urls tests ---

    #[test]
    fn preserve_urls_same_domain_absolute() {
        let base = ParsedUrl::parse("https://example.com/blog/post", None);
        let result = convert_url_to_relative(
            &base,
            "https://example.com/designy/classic",
            Some("href"),
            &config(true, false),
        );
        assert_eq!(result, "/designy/classic");
    }

    #[test]
    fn preserve_urls_same_domain_root_relative() {
        let base = ParsedUrl::parse("https://example.com/blog/post", None);
        let result = convert_url_to_relative(&base, "/about", Some("href"), &config(true, false));
        assert_eq!(result, "/about");
    }

    #[test]
    fn preserve_urls_same_domain_relative() {
        let base = ParsedUrl::parse("https://example.com/blog/post", None);
        let result = convert_url_to_relative(&base, "../images/logo.png", Some("src"), &config(true, false));
        assert_eq!(result, "/images/logo.png");
    }

    #[test]
    fn preserve_urls_parent_relative_starts_in_the_page_directory() {
        // #55: ../sibling on /a/b/page is /a/sibling, as the browser resolves it
        let page = ParsedUrl::parse("https://example.com/a/b/page", None);
        assert_eq!(
            convert_url_to_relative(&page, "../sibling", Some("href"), &config(true, false)),
            "/a/sibling"
        );
        assert_eq!(
            convert_url_to_relative(&page, "../../img/logo.png?v=2#x", Some("src"), &config(true, false)),
            "/img/logo.png?v=2#x"
        );
        let directory = ParsedUrl::parse("https://example.com/a/b/", None);
        assert_eq!(
            convert_url_to_relative(&directory, "../sibling", Some("href"), &config(true, false)),
            "/a/sibling"
        );
        let css = ParsedUrl::parse("https://example.com/assets/css/site.css", None);
        assert_eq!(
            convert_url_to_relative(&css, "../img/bg.png", None, &config(true, false)),
            "/assets/img/bg.png"
        );
        let root = ParsedUrl::parse("https://example.com/", None);
        assert_eq!(
            convert_url_to_relative(&root, "../about", Some("href"), &config(true, false)),
            "/about"
        );
    }

    #[test]
    fn preserve_urls_cross_domain() {
        let base = ParsedUrl::parse("https://example.com/page", None);
        let result = convert_url_to_relative(
            &base,
            "https://cdn.other.com/style.css",
            Some("href"),
            &config(true, false),
        );
        assert_eq!(result, "https://cdn.other.com/style.css");
    }

    #[test]
    fn preserve_urls_with_query_and_fragment() {
        let base = ParsedUrl::parse("https://example.com/", None);
        let result = convert_url_to_relative(&base, "/page?key=val#section", Some("href"), &config(true, false));
        assert_eq!(result, "/page?key=val#section");
    }

    #[test]
    fn preserve_urls_data_uri_unchanged() {
        let base = ParsedUrl::parse("https://example.com/", None);
        let result = convert_url_to_relative(&base, "data:image/png;base64,abc", None, &config(true, false));
        assert_eq!(result, "data:image/png;base64,abc");
    }

    #[test]
    fn preserve_urls_mailto_unchanged() {
        let base = ParsedUrl::parse("https://example.com/", None);
        let result = convert_url_to_relative(&base, "mailto:test@example.com", None, &config(true, false));
        assert_eq!(result, "mailto:test@example.com");
    }

    // --- no_url_rewriting tests ---

    #[test]
    fn no_rewriting_absolute_url_unchanged() {
        let base = ParsedUrl::parse("https://example.com/blog/post", None);
        let result = convert_url_to_relative(
            &base,
            "https://example.com/designy/classic",
            Some("href"),
            &config(false, true),
        );
        assert_eq!(result, "https://example.com/designy/classic");
    }

    #[test]
    fn no_rewriting_relative_url_unchanged() {
        let base = ParsedUrl::parse("https://example.com/blog/post", None);
        let result = convert_url_to_relative(&base, "../images/logo.png", Some("src"), &config(false, true));
        assert_eq!(result, "../images/logo.png");
    }

    #[test]
    fn no_rewriting_root_relative_unchanged() {
        let base = ParsedUrl::parse("https://example.com/blog/post", None);
        let result = convert_url_to_relative(&base, "/about", Some("href"), &config(false, true));
        assert_eq!(result, "/about");
    }

    #[test]
    fn no_rewriting_cross_domain_unchanged() {
        let base = ParsedUrl::parse("https://example.com/page", None);
        let result = convert_url_to_relative(
            &base,
            "https://cdn.other.com/style.css",
            Some("href"),
            &config(false, true),
        );
        assert_eq!(result, "https://cdn.other.com/style.css");
    }

    #[test]
    fn no_rewriting_preserves_html_entities() {
        let base = ParsedUrl::parse("https://example.com/", None);
        let result = convert_url_to_relative(&base, "/page?a=1&amp;b=2", Some("href"), &config(false, true));
        // With no_url_rewriting, even HTML entities are preserved verbatim
        assert_eq!(result, "/page?a=1&amp;b=2");
    }

    #[test]
    fn no_rewriting_with_query_and_fragment() {
        let base = ParsedUrl::parse("https://example.com/", None);
        let result = convert_url_to_relative(&base, "/page?key=val#section", Some("href"), &config(false, true));
        assert_eq!(result, "/page?key=val#section");
    }

    #[test]
    fn force_relative_urls_leaves_variants_on_pages_stored_under_the_variant_host() {
        // #35: when the initial URL redirects to its www twin, the pages are stored under _www.host/
        // and their links stay within that copy
        let mut cfg = config(false, false);
        cfg.force_relative_urls = true;
        let allow_www: DomainAllowFn = Arc::new(|domain: &str| domain == "www.example.com");
        cfg.is_external_domain_allowed_for_crawling = Some(allow_www);
        let www_page = ParsedUrl::parse("https://www.example.com/blog/", None);
        assert_eq!(
            convert_url_to_relative(&www_page, "/about", Some("href"), &cfg),
            "../about.html"
        );
        assert_eq!(
            convert_url_to_relative(&www_page, "https://www.example.com/style.css", Some("href"), &cfg),
            "../style.css"
        );
    }

    #[test]
    fn stored_url_lookup_tells_content_from_redirects() {
        let visited = |url: &str, status_code: i32, location: Option<&str>| {
            VisitedUrl::new(
                String::new(),
                String::new(),
                0,
                url.to_string(),
                status_code,
                0.1,
                None,
                ContentTypeId::Html,
                None,
                None,
                location
                    .map(|location| std::collections::HashMap::from([("Location".to_string(), location.to_string())])),
                false,
                true,
                0,
                None,
            )
        };
        let lookup = stored_url_lookup(&[
            visited("https://example.com:8443/", 200, None),
            visited("https://example.com/jump", 301, Some("https://www.example.com/landing")),
            visited("https://example.com/old?page=2", 302, Some("/new")),
            visited("https://example.com/missing", 404, None),
        ]);
        assert_eq!(lookup("https://example.com:8443/"), Some(StoredUrl::Content));
        assert_eq!(
            lookup("https://example.com/jump"),
            Some(StoredUrl::Redirect("https://www.example.com/landing".to_string()))
        );
        assert_eq!(
            lookup("https://example.com/old?page=2"),
            Some(StoredUrl::Redirect("https://example.com/new".to_string()))
        );
        assert_eq!(lookup("https://example.com/missing"), None);
        assert_eq!(lookup("https://example.com/unknown"), None);
    }

    #[test]
    fn force_relative_urls_leads_variants_to_where_the_crawler_stored_them() {
        // #35: the crawler fetches a variant from the initial origin, whatever host the page is on, so
        // the reference leads to that fetch's file or, when the initial origin redirected, to the file
        // of the redirect's target
        let mut cfg = config(false, false);
        cfg.force_relative_urls = true;
        let allow_www: DomainAllowFn = Arc::new(|domain: &str| domain == "www.example.com");
        cfg.is_external_domain_allowed_for_crawling = Some(allow_www);
        let stored_url: StoredUrlFn = Arc::new(|url: &str| match url {
            "https://example.com/next"
            | "https://example.com/docs/next"
            | "https://example.com/photo.png"
            | "https://www.example.com/about"
            | "https://www.example.com/img/a.png"
            | "https://www.example.com/landing" => Some(StoredUrl::Content),
            "https://example.com/about" | "https://example.com/img/a.png" => {
                Some(StoredUrl::Redirect(url.replace("://", "://www.")))
            }
            "https://example.com/jump" => Some(StoredUrl::Redirect("https://www.example.com/landing".to_string())),
            "https://example.com/away" => Some(StoredUrl::Redirect("https://other.com/".to_string())),
            _ => None,
        });
        cfg.stored_url = Some(stored_url);

        // A page stored under the www twin because /jump redirected there
        let twin_page = ParsedUrl::parse("https://www.example.com/landing", None);
        for (reference, attribute, expected) in [
            ("/next", "href", "../next.html"),
            ("/photo.png", "src", "../photo.png"),
            // the initial origin redirected to the twin: its copy, not the redirect record
            ("/about#team", "href", "about.html#team"),
            ("https://example.com/img/a.png", "src", "img/a.png"),
            // nothing known about it: kept within the twin's copy
            ("/unknown", "href", "unknown.html"),
        ] {
            assert_eq!(
                convert_url_to_relative(&twin_page, reference, Some(attribute), &cfg),
                expected,
                "{reference}"
            );
        }

        // References relative to a nested twin page are resolved before the lookup
        let nested_twin_page = ParsedUrl::parse("https://www.example.com/docs/landing", None);
        for (reference, attribute, expected) in [
            ("../next", "href", "../../next.html"),
            ("./x/../next", "href", "../../docs/next.html"),
            ("next", "href", "../../docs/next.html"),
            ("../photo.png", "src", "../../photo.png"),
        ] {
            assert_eq!(
                convert_url_to_relative(&nested_twin_page, reference, Some(attribute), &cfg),
                expected,
                "{reference}"
            );
        }

        let page = ParsedUrl::parse("https://example.com/", None);
        for (reference, expected) in [
            ("https://www.example.com/next", "next.html"),
            ("https://www.example.com/jump", "_www.example.com/landing.html"),
            // a redirect whose target was not stored leads to the redirect record
            ("/away", "away.html"),
        ] {
            assert_eq!(
                convert_url_to_relative(&page, reference, Some("href"), &cfg),
                expected,
                "{reference}"
            );
        }
    }

    #[test]
    fn force_relative_urls_links_a_page_to_itself_via_its_own_file() {
        // #35: a variant link to the page itself leads to the page's own file like any other variant
        // link (redirect records, which are not normalized, are handled by the HTML processor)
        let mut cfg = config(false, false);
        cfg.force_relative_urls = true;
        let allow_www: DomainAllowFn = Arc::new(|domain: &str| domain == "www.example.com");
        cfg.is_external_domain_allowed_for_crawling = Some(allow_www);
        let page = ParsedUrl::parse("https://example.com/about", None);
        assert_eq!(
            convert_url_to_relative(&page, "https://www.example.com/about#team", Some("href"), &cfg),
            "about.html#team"
        );
        assert_eq!(
            convert_url_to_relative(&page, "https://www.example.com/contact", Some("href"), &cfg),
            "contact.html"
        );
    }

    #[test]
    fn force_relative_urls_normalizes_variant_references_in_external_files() {
        // #35: the crawler fetches a variant found in a downloaded external file (a CDN stylesheet)
        // from the initial origin, so the reference leads to the initial host's file
        let mut cfg = config(false, false);
        cfg.force_relative_urls = true;
        let allow_cdn: DomainAllowFn = Arc::new(|domain: &str| domain == "cdn.other.com");
        cfg.is_domain_allowed_for_static_files = Some(allow_cdn);
        let stylesheet = ParsedUrl::parse("https://cdn.other.com/style.css", None);
        assert_eq!(
            convert_url_to_relative(&stylesheet, "https://www.example.com/img.png", None, &cfg),
            "../img.png"
        );
    }

    #[test]
    fn force_relative_urls_treats_www_and_scheme_variants_as_the_initial_host() {
        // #35: links to http/https and www/non-www variants lead to the initial host's local files
        let base = ParsedUrl::parse("https://example.com/blog/", None);
        let mut cfg = config(false, false);
        cfg.force_relative_urls = true;
        assert_eq!(
            convert_url_to_relative(&base, "http://www.example.com/about", Some("href"), &cfg),
            "../about.html"
        );
        assert_eq!(
            convert_url_to_relative(&base, "//www.example.com/img/a.jpg", Some("src"), &cfg),
            "../img/a.jpg"
        );
        assert_eq!(
            convert_url_to_relative(&base, "https://other.com/about", Some("href"), &cfg),
            "https://other.com/about"
        );

        cfg.offline_export_preserve_urls = true;
        assert_eq!(
            convert_url_to_relative(&base, "https://www.example.com/about", Some("href"), &cfg),
            "/about"
        );
    }

    #[test]
    fn preserve_url_structure_links_to_directory_index_files() {
        // #55: links lead to the files the exporter writes, relative to where the page is stored
        let mut cfg = config(false, false);
        cfg.offline_export_preserve_url_structure = true;
        let root = ParsedUrl::parse("https://example.com/", None);
        assert_eq!(
            convert_url_to_relative(&root, "/about", Some("href"), &cfg),
            "about/index.html"
        );
        let guide = ParsedUrl::parse("https://example.com/docs/guide", None);
        assert_eq!(
            convert_url_to_relative(&guide, "/about", Some("href"), &cfg),
            "../../about/index.html"
        );
        assert_eq!(
            convert_url_to_relative(&guide, "../style.css", Some("href"), &cfg),
            "../../style.css"
        );
    }

    #[test]
    fn query_only_reference_resolves_against_the_page_path() {
        // `?tab=2` on /about means /about?tab=2, not /?tab=2
        let about = ParsedUrl::parse("https://example.com/about", None);
        assert_eq!(
            convert_url_to_relative(&about, "?tab=2", Some("href"), &config(false, false)),
            "about.d686b9030b.html"
        );
        let blog = ParsedUrl::parse("https://example.com/blog/", None);
        assert_eq!(
            convert_url_to_relative(&blog, "?tab=2#top", Some("href"), &config(false, false)),
            "../blog/index.d686b9030b.html#top"
        );

        let mut cfg = config(false, false);
        cfg.offline_export_preserve_url_structure = true;
        assert_eq!(
            convert_url_to_relative(&about, "?tab=2", Some("href"), &cfg),
            "../about/index.d686b9030b.html"
        );
    }
}
