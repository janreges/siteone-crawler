// SiteOne Crawler - ParsedUrl
// (c) Jan Reges <jan.reges@siteone.cz>

use std::collections::HashMap;
use std::path::Path;
use std::sync::Mutex;

use once_cell::sync::Lazy;
use regex::Regex;

/// Regex for detecting HTML page extensions (not static files)
static HTML_EXTENSIONS_RE: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"(?i)\.(htm|html|shtml|php|phtml|ashx|xhtml|asp|aspx|jsp|jspx|do|cfm|cgi|pl|rb|erb|gsp)$").unwrap()
});

/// Regex for detecting file extension at end of path
static FILE_EXTENSION_RE: Lazy<Regex> = Lazy::new(|| Regex::new(r"(?i)\.([a-z0-9]{1,10})$").unwrap());

/// Regex for detecting image extensions in path
static IMAGE_PATH_RE: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"(?i)\.(png|gif|jpg|jpeg|ico|webp|avif|tif|bmp|svg)").unwrap());

/// Regex for detecting dynamic image query params
static IMAGE_QUERY_RE: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"(?i)(png|gif|jpg|jpeg|ico|webp|avif|tif|bmp|svg|crop|size|landscape)").unwrap());

/// Regex for detecting font extensions
static FONT_RE: Lazy<Regex> = Lazy::new(|| Regex::new(r"(?i)\.(eot|ttf|woff2|woff|otf)").unwrap());

/// Regex for 2nd level domain extraction
static DOMAIN_2ND_LEVEL_RE: Lazy<Regex> = Lazy::new(|| Regex::new(r"(?i)([a-z0-9\-]+\.[a-z][a-z0-9]{0,10})$").unwrap());

/// Regex for extracting extensions from path+query
static ESTIMATE_EXT_RE: Lazy<Regex> = Lazy::new(|| Regex::new(r"(?i)\.([0-9a-z]{1,5})").unwrap());

/// Regex for relative URL detection (starts with alphanumeric or underscore)
static RELATIVE_URL_RE: Lazy<Regex> = Lazy::new(|| Regex::new(r"(?i)^[a-z0-9_]").unwrap());

/// Parsed URL struct with all URL components
#[derive(Debug)]
pub struct ParsedUrl {
    pub url: String,
    pub scheme: Option<String>,
    pub host: Option<String>,
    pub port: Option<u16>,
    pub path: String,
    pub query: Option<String>,
    pub fragment: Option<String>,
    pub extension: Option<String>,
    pub domain_2nd_level: Option<String>,

    full_url_cache: Mutex<HashMap<String, String>>,
    debug: bool,
}

impl Clone for ParsedUrl {
    fn clone(&self) -> Self {
        Self {
            url: self.url.clone(),
            scheme: self.scheme.clone(),
            host: self.host.clone(),
            port: self.port,
            path: self.path.clone(),
            query: self.query.clone(),
            fragment: self.fragment.clone(),
            extension: self.extension.clone(),
            domain_2nd_level: self.domain_2nd_level.clone(),
            full_url_cache: Mutex::new(HashMap::new()),
            debug: self.debug,
        }
    }
}

impl PartialEq for ParsedUrl {
    fn eq(&self, other: &Self) -> bool {
        self.url == other.url
            && self.scheme == other.scheme
            && self.host == other.host
            && self.port == other.port
            && self.path == other.path
            && self.query == other.query
            && self.fragment == other.fragment
    }
}

impl Eq for ParsedUrl {}

impl std::hash::Hash for ParsedUrl {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.url.hash(state);
        self.scheme.hash(state);
        self.host.hash(state);
        self.port.hash(state);
        self.path.hash(state);
        self.query.hash(state);
        self.fragment.hash(state);
    }
}

impl ParsedUrl {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        url: String,
        scheme: Option<String>,
        host: Option<String>,
        port: Option<u16>,
        path: String,
        query: Option<String>,
        fragment: Option<String>,
        extension: Option<String>,
        domain_2nd_level: Option<String>,
    ) -> Self {
        let fragment = match fragment.as_deref() {
            Some("") => None,
            _ => fragment,
        };
        Self {
            url,
            scheme,
            host,
            port,
            path,
            query,
            fragment,
            extension,
            domain_2nd_level,
            full_url_cache: Mutex::new(HashMap::new()),
            debug: false,
        }
    }

    /// Get full URL with optional scheme+host and optional fragment
    pub fn get_full_url(&self, include_scheme_and_host: bool, include_fragment: bool) -> String {
        let cache_key = format!(
            "{}{}",
            if include_scheme_and_host { '1' } else { '0' },
            if include_fragment { '1' } else { '0' }
        );

        if let Ok(cache) = self.full_url_cache.lock()
            && let Some(cached) = cache.get(&cache_key)
        {
            return cached.clone();
        }

        let mut full_url = self.path.clone();
        if let Some(ref q) = self.query {
            full_url.push('?');
            full_url.push_str(q);
        }
        if include_fragment && let Some(ref f) = self.fragment {
            full_url.push('#');
            full_url.push_str(f);
        }

        if include_scheme_and_host {
            if let (Some(scheme), Some(host)) = (&self.scheme, &self.host) {
                let mut port = self.port;
                if (port == Some(80) && scheme == "http") || (port == Some(443) && scheme == "https") {
                    port = None;
                }
                let port_str = match port {
                    Some(p) => format!(":{}", p),
                    None => String::new(),
                };
                full_url = format!("{}://{}{}{}", scheme, host, port_str, full_url);
            } else if self.scheme.is_none()
                && let Some(ref host) = self.host
            {
                let port = match self.port {
                    Some(p) if p != 80 && p != 443 => Some(p),
                    _ => None,
                };
                let port_str = match port {
                    Some(p) => format!(":{}", p),
                    None => String::new(),
                };
                full_url = format!("//{}{}{}", host, port_str, full_url);
            }
        }

        if let Ok(mut cache) = self.full_url_cache.lock() {
            cache.insert(cache_key, full_url.clone());
        }

        full_url
    }

    /// Is probably static file/asset and probably not the HTML page?
    pub fn is_static_file(&self) -> bool {
        if FILE_EXTENSION_RE.is_match(&self.path) {
            // Has extension - check it's not numeric
            let is_numeric = self
                .extension
                .as_ref()
                .map(|e| e.parse::<f64>().is_ok())
                .unwrap_or(false);

            if !is_numeric && !HTML_EXTENSIONS_RE.is_match(&self.path) {
                return true;
            }
        }

        if self.is_image() || self.is_css() {
            return true;
        }

        false
    }

    /// Is probably image? Has an image extension or is dynamic image
    pub fn is_image(&self) -> bool {
        let has_image_extension = IMAGE_PATH_RE.is_match(&self.path);
        let is_dynamic_image = self.query.as_ref().map(|q| IMAGE_QUERY_RE.is_match(q)).unwrap_or(false);
        has_image_extension || is_dynamic_image
    }

    /// Is font file?
    pub fn is_font(&self) -> bool {
        FONT_RE.is_match(&self.path)
    }

    /// Is CSS file?
    pub fn is_css(&self) -> bool {
        self.extension.as_deref() == Some("css") || self.url.to_lowercase().contains("fonts.googleapis.com/css")
    }

    /// Is Origin header required for this resource?
    pub fn is_origin_required(&self) -> bool {
        self.is_font()
    }

    /// Estimate file extension from URL
    pub fn estimate_extension(&self) -> Option<String> {
        // if extension is numeric, it is probably not a real extension
        if let Some(ref ext) = self.extension {
            if ext.parse::<f64>().is_ok() {
                return None;
            }
            return Some(ext.to_lowercase());
        }

        let combined = format!("{}?{}", self.path, self.query.as_deref().unwrap_or(""));
        let mut last_ext = None;
        for caps in ESTIMATE_EXT_RE.captures_iter(&combined) {
            if let Some(m) = caps.get(1) {
                last_ext = Some(m.as_str().to_lowercase());
            }
        }
        last_ext
    }

    /// Copy scheme/host/port from another ParsedUrl
    pub fn set_attributes(&mut self, url: &ParsedUrl, scheme: bool, host: bool, port: bool) {
        if scheme {
            self.scheme = url.scheme.clone();
        }
        if host {
            self.host = url.host.clone();
        }
        if port {
            self.port = url.port;
        }
        self.clear_cache();
    }

    pub fn set_path(&mut self, path: String) {
        self.path = path;
        self.extension = extract_extension(&self.path);
        self.clear_cache();
    }

    /// Change depth by adding/removing ../ prefixes
    pub fn change_depth(&mut self, change: i32) {
        let mut new_path = self.path.clone();
        if change > 0 {
            let clean_path = new_path.trim_start_matches('/');
            new_path = format!("{}{}", "../".repeat(change as usize), clean_path);
        } else if change < 0 {
            let count = change.unsigned_abs() as usize;
            for _ in 0..count {
                if let Some(rest) = new_path.strip_prefix("../") {
                    new_path = rest.to_string();
                } else {
                    break;
                }
            }
        }

        if new_path != self.path {
            self.set_path(new_path);
        }
        self.clear_cache();
    }

    pub fn set_query(&mut self, query: Option<String>) {
        self.query = query;
        self.clear_cache();
    }

    pub fn set_fragment(&mut self, fragment: Option<String>) {
        self.fragment = fragment;
        self.clear_cache();
    }

    pub fn set_extension(&mut self, extension: Option<String>) {
        self.extension = extension;
        self.clear_cache();
    }

    pub fn set_debug(&mut self, debug: bool) {
        self.debug = debug;
    }

    /// URL is only a fragment reference (#something)
    pub fn is_only_fragment(&self) -> bool {
        self.path.is_empty() && self.query.is_none() && self.host.is_none() && self.fragment.is_some()
    }

    /// Get full homepage URL (scheme://host[:port]) without trailing slash
    pub fn get_full_homepage_url(&self) -> String {
        let port_str = match self.port {
            Some(p) => format!(":{}", p),
            None => String::new(),
        };
        format!(
            "{}://{}{}",
            self.scheme.as_deref().unwrap_or("https"),
            self.host.as_deref().unwrap_or(""),
            port_str
        )
    }

    /// Parse URL string and return ParsedUrl object
    /// When base_url is provided, it fills in missing parts (scheme, host, port)
    pub fn parse(url: &str, base_url: Option<&ParsedUrl>) -> Self {
        let mut url = url.to_string();

        if let Some(base) = base_url {
            if url.starts_with("./") {
                // Relative URL via ./xyz
                if base.path.ends_with('/') {
                    url = format!("{}{}", base.path, &url[2..]);
                } else {
                    let dir = parent_path(&base.path);
                    let file = &url[2..];
                    if dir == "/" {
                        url = format!("/{}", file);
                    } else {
                        url = format!("{}/{}", dir, file);
                    }
                }
            } else if !url.starts_with("http:") && !url.starts_with("https:") && RELATIVE_URL_RE.is_match(&url) {
                // Relative URL via xyz/abc
                if base.path.ends_with('/') {
                    url = format!("{}{}", base.path, url);
                } else {
                    url = format!("{}{}", parent_path(&base.path), url);
                }
            } else if url.starts_with('/') && !url.starts_with("//") {
                // Absolute path /xyz/abc
                url = format!("{}{}", base.get_full_homepage_url(), url);
            }
        }

        // Use url::Url for parsing when it's a full URL, otherwise manual parse
        let (scheme, host, port_parsed, path, query, fragment) =
            if url.starts_with("http://") || url.starts_with("https://") || url.starts_with("//") {
                // For protocol-relative URLs, prepend a scheme for parsing
                let parse_url = if url.starts_with("//") {
                    format!("https:{}", url)
                } else {
                    url.clone()
                };

                match url::Url::parse(&parse_url) {
                    Ok(parsed) => {
                        let s = if url.starts_with("//") {
                            None
                        } else {
                            Some(parsed.scheme().to_string())
                        };
                        let h = parsed.host_str().map(|h| h.to_string());
                        let p = parsed.port();
                        let path = if parsed.path().is_empty() {
                            "/".to_string()
                        } else {
                            parsed.path().to_string()
                        };
                        let q = parsed.query().map(|q| q.to_string());
                        let f = parsed.fragment().map(|f| f.to_string());
                        (s, h, p, path, q, f)
                    }
                    Err(_) => parse_url_manually(&url),
                }
            } else {
                parse_url_manually(&url)
            };

        let scheme = scheme.or_else(|| base_url.and_then(|b| b.scheme.clone()));
        let has_parsed_host = host.is_some();
        let host = host.or_else(|| base_url.and_then(|b| b.host.clone()));
        let port = port_parsed.or_else(|| {
            if !has_parsed_host {
                base_url.and_then(|b| b.port)
            } else {
                None
            }
        });
        let port = port.or(match scheme.as_deref() {
            Some("http") => Some(80),
            _ => Some(443),
        });

        let path = if path.is_empty() && has_parsed_host {
            "/".to_string()
        } else {
            path
        };

        let extension = if !path.is_empty() && path.contains('.') {
            extract_extension(&path)
        } else {
            None
        };

        let domain_2nd_level = host.as_ref().and_then(|h| {
            DOMAIN_2ND_LEVEL_RE
                .captures(h)
                .and_then(|c| c.get(1))
                .map(|m| m.as_str().to_string())
        });

        Self::new(
            url,
            scheme,
            host,
            port,
            path,
            query,
            fragment,
            extension,
            domain_2nd_level,
        )
    }

    pub fn is_https(&self) -> bool {
        self.scheme.as_deref() == Some("https")
    }

    /// Extract 2nd-level domain from a host string (e.g., "www.example.com" -> "example.com")
    pub fn extract_2nd_level_domain(host: &str) -> Option<String> {
        DOMAIN_2ND_LEVEL_RE
            .captures(host)
            .and_then(|c| c.get(1))
            .map(|m| m.as_str().to_string())
    }

    /// Get base name (last path part) of the URL
    pub fn get_base_name(&self) -> Option<String> {
        if self.path.is_empty() || self.path == "/" {
            return None;
        }

        let path = self.path.trim_end_matches('/');
        let result = path.rsplit('/').next().filter(|s| !s.is_empty());

        result.map(|r| {
            // if query string contains path, return path with this query
            if let Some(ref q) = self.query
                && (q.contains('/') || q.contains("%2F"))
            {
                return format!("{}?{}", r, q);
            }
            r.to_string()
        })
    }

    /// Get depth of the URL path
    /// / -> 0, /about -> 1, /about/me -> 2, etc.
    pub fn get_depth(&self) -> usize {
        let trimmed = self.path.trim_end_matches('/');
        let slash_count = trimmed.matches('/').count();
        let dotdot_count = self.path.matches("/..").count();
        slash_count.saturating_sub(dotdot_count)
    }

    fn clear_cache(&self) {
        if let Ok(mut cache) = self.full_url_cache.lock() {
            cache.clear();
        }
    }
}

impl std::fmt::Display for ParsedUrl {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.get_full_url(true, true))
    }
}

/// Extract file extension from path.
fn extract_extension(path: &str) -> Option<String> {
    Path::new(path)
        .extension()
        .and_then(|e| e.to_str())
        .filter(|e| !e.is_empty())
        .map(|e| e.to_string())
}

/// Get parent directory of a path.
fn parent_path(path: &str) -> String {
    match path.rfind('/') {
        Some(0) => "/".to_string(),
        Some(pos) => path[..pos].to_string(),
        None => ".".to_string(),
    }
}

/// Manual URL parsing for non-standard URLs (relative paths, fragments, etc.)
#[allow(clippy::type_complexity)]
fn parse_url_manually(
    url: &str,
) -> (
    Option<String>,
    Option<String>,
    Option<u16>,
    String,
    Option<String>,
    Option<String>,
) {
    let mut remaining = url;

    // Extract fragment
    let fragment = if let Some(hash_pos) = remaining.find('#') {
        let f = &remaining[hash_pos + 1..];
        remaining = &remaining[..hash_pos];
        if f.is_empty() { None } else { Some(f.to_string()) }
    } else {
        None
    };

    // Extract query
    let query = if let Some(q_pos) = remaining.find('?') {
        let q = &remaining[q_pos + 1..];
        remaining = &remaining[..q_pos];
        if q.is_empty() { None } else { Some(q.to_string()) }
    } else {
        None
    };

    let path = remaining.to_string();

    (None, None, None, path, query, fragment)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_full_url() {
        let parsed = ParsedUrl::parse("https://example.com/path/to/page?q=1#section", None);
        assert_eq!(parsed.scheme.as_deref(), Some("https"));
        assert_eq!(parsed.host.as_deref(), Some("example.com"));
        assert_eq!(parsed.path, "/path/to/page");
        assert_eq!(parsed.query.as_deref(), Some("q=1"));
        assert_eq!(parsed.fragment.as_deref(), Some("section"));
    }

    #[test]
    fn test_depth() {
        assert_eq!(ParsedUrl::parse("/", None).get_depth(), 0);
        assert_eq!(ParsedUrl::parse("/about", None).get_depth(), 1);
        assert_eq!(ParsedUrl::parse("/about/", None).get_depth(), 1);
        assert_eq!(ParsedUrl::parse("/about/me", None).get_depth(), 2);
        assert_eq!(ParsedUrl::parse("/about/me/", None).get_depth(), 2);
    }

    #[test]
    fn test_is_static_file() {
        let css = ParsedUrl::parse("https://example.com/style.css", None);
        assert!(css.is_static_file());

        let html = ParsedUrl::parse("https://example.com/page.html", None);
        assert!(!html.is_static_file());

        let page = ParsedUrl::parse("https://example.com/about", None);
        assert!(!page.is_static_file());
    }

    #[test]
    fn test_relative_url_resolution() {
        let base = ParsedUrl::parse("https://example.com/dir/page", None);
        let relative = ParsedUrl::parse("./other", Some(&base));
        assert_eq!(relative.path, "/dir/other");
    }

    #[test]
    fn test_get_full_url() {
        let parsed = ParsedUrl::parse("https://example.com/path?q=1#frag", None);
        assert_eq!(parsed.get_full_url(true, true), "https://example.com/path?q=1#frag");
        assert_eq!(parsed.get_full_url(true, false), "https://example.com/path?q=1");
        assert_eq!(parsed.get_full_url(false, true), "/path?q=1#frag");
    }

    #[test]
    fn test_get_base_name() {
        let p1 = ParsedUrl::parse("https://example.com/foo/bar", None);
        assert_eq!(p1.get_base_name(), Some("bar".to_string()));

        let p2 = ParsedUrl::parse("https://example.com/", None);
        assert_eq!(p2.get_base_name(), None);
    }

    #[test]
    fn test_domain_2nd_level() {
        let parsed = ParsedUrl::parse("https://sub.example.com/page", None);
        assert_eq!(parsed.domain_2nd_level.as_deref(), Some("example.com"));
    }
}
