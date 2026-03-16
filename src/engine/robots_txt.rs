// SiteOne Crawler - robots.txt parser
// (c) Jan Reges <jan.reges@siteone.cz>

use once_cell::sync::Lazy;
use regex::Regex;

/// Regex for matching frontend asset extensions that are always allowed
static ASSET_EXTENSION_RE: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"(?i)\.(js|css|json|eot|ttf|woff2|woff|otf|png|gif|jpg|jpeg|ico|webp|avif|tif|bmp|svg)").unwrap()
});

/// Regex for User-agent directive
static USER_AGENT_RE: Lazy<Regex> = Lazy::new(|| Regex::new(r"(?i)^User-agent:\s*(.*)").unwrap());

/// Regex for Disallow directive
static DISALLOW_RE: Lazy<Regex> = Lazy::new(|| Regex::new(r"(?i)^Disallow:\s*(.*)").unwrap());

/// Regex for Allow directive
static ALLOW_RE: Lazy<Regex> = Lazy::new(|| Regex::new(r"(?i)^Allow:\s*(.*)").unwrap());

/// Regex for Sitemap directive
static SITEMAP_RE: Lazy<Regex> = Lazy::new(|| Regex::new(r"(?i)^Sitemap:\s*(.*)").unwrap());

/// Regex to strip comments
static COMMENT_RE: Lazy<Regex> = Lazy::new(|| Regex::new(r"#.*").unwrap());

/// Parsed robots.txt data for a single domain
#[derive(Debug, Clone)]
pub struct RobotsTxt {
    /// Disallowed paths for the relevant user agents (* and SiteOne-Crawler)
    disallowed_paths: Vec<String>,
    /// Allowed paths (override disallows) for the relevant user agents
    allowed_paths: Vec<String>,
    /// Sitemap URLs declared in robots.txt
    sitemaps: Vec<String>,
    /// Raw content of robots.txt
    raw_content: String,
}

impl RobotsTxt {
    /// Parse robots.txt content and extract rules for * and SiteOne-Crawler user agents
    pub fn parse(content: &str) -> Self {
        let mut disallowed_paths = Vec::new();
        let mut allowed_paths = Vec::new();
        let mut sitemaps = Vec::new();
        let mut current_user_agent: Option<String> = None;

        for line in content.lines() {
            // Remove comments
            let line = COMMENT_RE.replace(line, "");
            let line = line.trim();

            if line.is_empty() {
                continue;
            }

            if let Some(caps) = USER_AGENT_RE.captures(line) {
                if let Some(m) = caps.get(1) {
                    current_user_agent = Some(m.as_str().trim().to_string());
                }
            } else if let Some(ref ua) = current_user_agent
                && (ua == "*" || ua == "SiteOne-Crawler")
            {
                if let Some(caps) = DISALLOW_RE.captures(line) {
                    if let Some(m) = caps.get(1) {
                        let path = m.as_str().trim().to_string();
                        if !path.is_empty() {
                            disallowed_paths.push(path);
                        }
                    }
                } else if let Some(caps) = ALLOW_RE.captures(line)
                    && let Some(m) = caps.get(1)
                {
                    let path = m.as_str().trim().to_string();
                    if !path.is_empty() {
                        allowed_paths.push(path);
                    }
                }
            }

            // Sitemaps are always parsed regardless of user-agent section
            if let Some(caps) = SITEMAP_RE.captures(line)
                && let Some(m) = caps.get(1)
            {
                let sitemap_url = m.as_str().trim().to_string();
                if !sitemap_url.is_empty() {
                    sitemaps.push(sitemap_url);
                }
            }
        }

        Self {
            disallowed_paths,
            allowed_paths,
            sitemaps,
            raw_content: content.to_string(),
        }
    }

    /// Check if a URL path is allowed by the robots.txt rules.
    /// Frontend assets (js, css, images, fonts) are always allowed.
    ///
    /// A URL is disallowed if its path starts with any disallowed path
    /// (case-insensitive prefix match).
    pub fn is_allowed(&self, url: &str) -> bool {
        // Frontend assets are always allowed
        if ASSET_EXTENSION_RE.is_match(url) {
            return true;
        }

        // If no disallowed paths, everything is allowed
        if self.disallowed_paths.is_empty() {
            return true;
        }

        // Extract path from URL
        let url_path = url::Url::parse(url).ok().map(|u| u.path().to_string()).or_else(|| {
            // If it's not a full URL, try treating it as a path
            let path_part = if let Some(q_pos) = url.find('?') {
                &url[..q_pos]
            } else {
                url
            };
            Some(path_part.to_string())
        });

        let url_path = match url_path {
            Some(p) => p,
            None => return true,
        };

        // Check allowed paths first (they override disallows)
        for allowed_path in &self.allowed_paths {
            if path_matches(&url_path, allowed_path) {
                return true;
            }
        }

        // Check disallowed paths
        for disallowed_path in &self.disallowed_paths {
            if path_matches(&url_path, disallowed_path) {
                return false;
            }
        }

        true
    }

    /// Get sitemap URLs declared in robots.txt
    pub fn get_sitemaps(&self) -> &[String] {
        &self.sitemaps
    }

    /// Get disallowed paths
    pub fn get_disallowed_paths(&self) -> &[String] {
        &self.disallowed_paths
    }

    /// Get allowed paths
    pub fn get_allowed_paths(&self) -> &[String] {
        &self.allowed_paths
    }

    /// Get raw robots.txt content
    pub fn get_raw_content(&self) -> &str {
        &self.raw_content
    }
}

/// Check if a URL path matches a robots.txt path pattern.
/// Supports:
/// - Simple prefix matching
/// - Wildcard (*) matching
/// - End-of-string ($) anchor
fn path_matches(url_path: &str, pattern: &str) -> bool {
    // Handle $ anchor at end
    if let Some(pattern_without_anchor) = pattern.strip_suffix('$') {
        if pattern_without_anchor.contains('*') {
            return wildcard_match(url_path, pattern_without_anchor, true);
        }
        return url_path.to_lowercase() == pattern_without_anchor.to_lowercase();
    }

    // Handle wildcard patterns
    if pattern.contains('*') {
        return wildcard_match(url_path, pattern, false);
    }

    // Simple case-insensitive prefix match
    url_path.to_lowercase().starts_with(&pattern.to_lowercase())
}

/// Match a URL path against a wildcard pattern (* matches any sequence of characters)
fn wildcard_match(url_path: &str, pattern: &str, exact_end: bool) -> bool {
    let parts: Vec<&str> = pattern.split('*').collect();
    let url_lower = url_path.to_lowercase();
    let mut search_from = 0;

    for (i, part) in parts.iter().enumerate() {
        if part.is_empty() {
            continue;
        }
        let part_lower = part.to_lowercase();

        match url_lower[search_from..].find(&part_lower) {
            Some(pos) => {
                // First part must match at start
                if i == 0 && pos != 0 {
                    return false;
                }
                search_from += pos + part_lower.len();
            }
            None => return false,
        }
    }

    if exact_end {
        // The last part must end at the end of the URL path
        return search_from == url_lower.len();
    }

    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_basic() {
        let content = r#"
User-agent: *
Disallow: /admin/
Disallow: /private/
Allow: /admin/public/

Sitemap: https://example.com/sitemap.xml
"#;
        let robots = RobotsTxt::parse(content);
        assert_eq!(robots.disallowed_paths.len(), 2);
        assert_eq!(robots.allowed_paths.len(), 1);
        assert_eq!(robots.sitemaps.len(), 1);
        assert_eq!(robots.sitemaps[0], "https://example.com/sitemap.xml");
    }

    #[test]
    fn test_is_allowed() {
        let content = r#"
User-agent: *
Disallow: /admin/
Disallow: /private/
"#;
        let robots = RobotsTxt::parse(content);
        assert!(robots.is_allowed("/public/page"));
        assert!(!robots.is_allowed("/admin/settings"));
        assert!(!robots.is_allowed("/private/data"));
        assert!(robots.is_allowed("/"));
    }

    #[test]
    fn test_assets_always_allowed() {
        let content = r#"
User-agent: *
Disallow: /
"#;
        let robots = RobotsTxt::parse(content);
        assert!(robots.is_allowed("/style.css"));
        assert!(robots.is_allowed("/script.js"));
        assert!(robots.is_allowed("/image.png"));
        assert!(!robots.is_allowed("/page"));
    }

    #[test]
    fn test_wildcard_matching() {
        assert!(path_matches("/search?q=test", "/search"));
        assert!(path_matches("/admin/page", "/admin/"));
        assert!(!path_matches("/public/page", "/admin/"));
    }

    #[test]
    fn test_wildcard_star() {
        assert!(path_matches("/path/to/file.pdf", "/*.pdf"));
        assert!(!path_matches("/path/to/file.html", "/*.pdf"));
    }

    #[test]
    fn test_anchor_matching() {
        assert!(path_matches("/page.html", "/page.html$"));
        assert!(!path_matches("/page.html?q=1", "/page.html$"));
    }

    #[test]
    fn test_siteone_crawler_user_agent() {
        let content = r#"
User-agent: SiteOne-Crawler
Disallow: /blocked/

User-agent: Googlebot
Disallow: /google-only/
"#;
        let robots = RobotsTxt::parse(content);
        assert!(!robots.is_allowed("/blocked/page"));
        // /google-only/ is not disallowed for SiteOne-Crawler or *
        assert!(robots.is_allowed("/google-only/page"));
    }

    #[test]
    fn test_comments_stripped() {
        let content = r#"
User-agent: * # all bots
Disallow: /admin/ # admin panel
# Disallow: /not-really-disallowed/
"#;
        let robots = RobotsTxt::parse(content);
        assert_eq!(robots.disallowed_paths.len(), 1);
        assert_eq!(robots.disallowed_paths[0], "/admin/");
    }

    #[test]
    fn test_empty_disallow() {
        let content = r#"
User-agent: *
Disallow:
"#;
        let robots = RobotsTxt::parse(content);
        assert!(robots.disallowed_paths.is_empty());
        assert!(robots.is_allowed("/anything"));
    }

    #[test]
    fn test_multiple_sitemaps() {
        let content = r#"
User-agent: *
Disallow:

Sitemap: https://example.com/sitemap1.xml
Sitemap: https://example.com/sitemap2.xml
"#;
        let robots = RobotsTxt::parse(content);
        assert_eq!(robots.sitemaps.len(), 2);
    }
}
