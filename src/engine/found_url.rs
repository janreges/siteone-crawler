// SiteOne Crawler - FoundUrl
// (c) Jan Reges <jan.reges@siteone.cz>

use once_cell::sync::Lazy;
use regex::Regex;

use super::parsed_url::ParsedUrl;

/// Source of discovered URL - where in HTML/CSS/JS was found
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum UrlSource {
    InitUrl = 5,
    AHref = 10,
    ImgSrc = 20,
    ImgSrcset = 21,
    InputSrc = 22,
    SourceSrc = 23,
    VideoSrc = 24,
    AudioSrc = 25,
    ScriptSrc = 30,
    InlineScriptSrc = 40,
    LinkHref = 50,
    CssUrl = 60,
    JsUrl = 70,
    Redirect = 80,
    Sitemap = 90,
}

impl UrlSource {
    /// Get short human-readable name for this source type
    pub fn short_name(&self) -> &'static str {
        match self {
            UrlSource::InitUrl => "Initial URL",
            UrlSource::AHref => "<a href>",
            UrlSource::ImgSrc => "<img src>",
            UrlSource::ImgSrcset => "<img srcset>",
            UrlSource::InputSrc => "<input src>",
            UrlSource::SourceSrc => "<source src>",
            UrlSource::VideoSrc => "<video src>",
            UrlSource::AudioSrc => "<audio src>",
            UrlSource::ScriptSrc => "<script src>",
            UrlSource::InlineScriptSrc => "inline <script src>",
            UrlSource::LinkHref => "<link href>",
            UrlSource::CssUrl => "css url()",
            UrlSource::JsUrl => "js url",
            UrlSource::Redirect => "redirect",
            UrlSource::Sitemap => "sitemap",
        }
    }

    /// Convert from integer source code.
    pub fn from_code(code: u8) -> Option<Self> {
        match code {
            5 => Some(UrlSource::InitUrl),
            10 => Some(UrlSource::AHref),
            20 => Some(UrlSource::ImgSrc),
            21 => Some(UrlSource::ImgSrcset),
            22 => Some(UrlSource::InputSrc),
            23 => Some(UrlSource::SourceSrc),
            24 => Some(UrlSource::VideoSrc),
            25 => Some(UrlSource::AudioSrc),
            30 => Some(UrlSource::ScriptSrc),
            40 => Some(UrlSource::InlineScriptSrc),
            50 => Some(UrlSource::LinkHref),
            60 => Some(UrlSource::CssUrl),
            70 => Some(UrlSource::JsUrl),
            80 => Some(UrlSource::Redirect),
            90 => Some(UrlSource::Sitemap),
            _ => None,
        }
    }
}

impl std::fmt::Display for UrlSource {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.short_name())
    }
}

/// Regex to match absolute HTTP URLs
static HTTP_URL_RE: Lazy<Regex> = Lazy::new(|| Regex::new(r"(?i)^https?://").unwrap());

/// A URL found during crawling, with information about where it was found
#[derive(Debug, Clone)]
pub struct FoundUrl {
    /// The normalized found URL
    pub url: String,
    /// URL of the page where this URL was found
    pub source_url: String,
    /// Source type (where in HTML/CSS the URL was found)
    pub source: UrlSource,
}

impl FoundUrl {
    pub fn new(url: &str, source_url: &str, source: UrlSource) -> Self {
        let normalized = normalize_url(url, source_url);
        Self {
            url: normalized,
            source_url: source_url.to_string(),
            source,
        }
    }

    /// Is this URL an included asset (img src, script src, link href) and not linked by href?
    pub fn is_included_asset(&self) -> bool {
        self.source != UrlSource::AHref
    }
}

impl std::fmt::Display for FoundUrl {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.url)
    }
}

/// Normalize URL and remove strange characters/behavior.
/// Remove unwanted http(s)://SAME_DOMAIN:SAME_OPTIONAL_PORT prefix when it matches the source URL.
fn normalize_url(url: &str, source_url: &str) -> String {
    // Replace HTML entities and escape sequences
    let mut normalized = url
        .replace("&#38;", "&")
        .replace("&amp;", "&")
        .replace("\\ ", "%20")
        .replace(' ', "%20");

    // Trim leading quotes/tabs/spaces
    normalized = normalized.trim_start_matches(['"', '\'', '\t', ' ']).to_string();
    // Trim trailing &, quotes, tabs, spaces
    normalized = normalized.trim_end_matches(['&', '"', '\'', '\t', ' ']).to_string();

    // Remove unwanted http(s)://SAME_DOMAIN:SAME_OPTIONAL_PORT
    if HTTP_URL_RE.is_match(&normalized) {
        let parsed_url = ParsedUrl::parse(&normalized, Some(&ParsedUrl::parse(source_url, None)));
        let parsed_source = ParsedUrl::parse(source_url, None);

        if parsed_url.host == parsed_source.host
            && parsed_source.port == parsed_url.port
            && parsed_source.port.is_some()
            && let (Some(scheme), Some(host)) = (&parsed_url.scheme, &parsed_url.host)
        {
            // Build regex pattern to strip scheme://host[:port]
            let port_pattern = match parsed_url.port {
                Some(p) => format!("(:{p})?"),
                None => String::new(),
            };
            let pattern = format!(
                r"(?i){}://{}{}",
                regex::escape(scheme),
                regex::escape(host),
                port_pattern
            );
            if let Ok(re) = Regex::new(&pattern) {
                normalized = re.replace(&normalized, "").to_string();
            }
        }
    }

    normalized
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_normalize_url_entities() {
        let found = FoundUrl::new("/page?a=1&amp;b=2", "https://example.com/", UrlSource::AHref);
        assert_eq!(found.url, "/page?a=1&b=2");
    }

    #[test]
    fn test_normalize_url_spaces() {
        let found = FoundUrl::new("/path with spaces", "https://example.com/", UrlSource::AHref);
        assert_eq!(found.url, "/path%20with%20spaces");
    }

    #[test]
    fn test_is_included_asset() {
        let link = FoundUrl::new("/page", "https://example.com/", UrlSource::AHref);
        assert!(!link.is_included_asset());

        let img = FoundUrl::new("/img.png", "https://example.com/", UrlSource::ImgSrc);
        assert!(img.is_included_asset());
    }

    #[test]
    fn test_source_short_name() {
        assert_eq!(UrlSource::AHref.short_name(), "<a href>");
        assert_eq!(UrlSource::Redirect.short_name(), "redirect");
    }
}
