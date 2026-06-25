// SiteOne Crawler - SeoAndOpenGraphResult
// (c) Jan Reges <jan.reges@siteone.cz>

use super::heading_tree_item::HeadingTreeItem;

pub const ROBOTS_INDEX: i32 = 1;
pub const ROBOTS_NOINDEX: i32 = 0;
pub const ROBOTS_FOLLOW: i32 = 1;
pub const ROBOTS_NOFOLLOW: i32 = 2;

#[derive(Debug, Clone)]
pub struct SeoAndOpenGraphResult {
    pub url_uq_id: String,
    pub url_path_and_query: String,
    /// Host of the page itself, used to detect off-domain canonical links.
    pub page_host: Option<String>,

    pub title: Option<String>,
    pub description: Option<String>,
    pub keywords: Option<String>,
    pub h1: Option<String>,
    pub canonical: Option<String>,

    pub robots_index: Option<i32>,
    pub robots_follow: Option<i32>,
    pub denied_by_robots_txt: bool,

    pub og_title: Option<String>,
    pub og_type: Option<String>,
    pub og_image: Option<String>,
    pub og_url: Option<String>,
    pub og_description: Option<String>,
    pub og_site_name: Option<String>,

    pub twitter_card: Option<String>,
    pub twitter_site: Option<String>,
    pub twitter_creator: Option<String>,
    pub twitter_title: Option<String>,
    pub twitter_description: Option<String>,
    pub twitter_image: Option<String>,

    pub heading_tree_items: Vec<HeadingTreeItem>,
    pub headings_count: usize,
    pub headings_errors_count: usize,
}

impl SeoAndOpenGraphResult {
    pub fn new(url_uq_id: String, url_path_and_query: String) -> Self {
        Self {
            url_uq_id,
            url_path_and_query,
            page_host: None,
            title: None,
            description: None,
            keywords: None,
            h1: None,
            canonical: None,
            robots_index: None,
            robots_follow: None,
            denied_by_robots_txt: false,
            og_title: None,
            og_type: None,
            og_image: None,
            og_url: None,
            og_description: None,
            og_site_name: None,
            twitter_card: None,
            twitter_site: None,
            twitter_creator: None,
            twitter_title: None,
            twitter_description: None,
            twitter_image: None,
            heading_tree_items: Vec::new(),
            headings_count: 0,
            headings_errors_count: 0,
        }
    }

    /// Check if a URL is denied by robots.txt for our crawler's user agent.
    ///
    /// Delegates to the shared [`RobotsTxt`](crate::engine::robots_txt::RobotsTxt) parser so the
    /// SEO "Indexing" column uses the exact same rules as the crawler itself: user-agent scoping
    /// (`*` / `SiteOne-Crawler`), `Allow:` overrides, and wildcard/`$`-anchor matching.
    ///
    /// Historically this did a naive global scan of every `Disallow:` line regardless of its
    /// `User-agent:` block, so a single `Disallow: /` targeting another bot (e.g. GPTBot/CCBot)
    /// flagged every page as denied — the v2.4.0 "all pages DENY" regression (issue #105).
    pub fn is_denied_by_robots_txt(url: &str, robots_txt_content: &str) -> bool {
        if robots_txt_content.is_empty() {
            return false;
        }
        !crate::engine::robots_txt::RobotsTxt::parse(robots_txt_content).is_allowed(url)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Regression test for the v2.4.0 SEO "Indexing" column reporting every page as
    /// `DENY (robots.txt)` (issue #105). A `Disallow: /` under a *specific* bot's
    /// user-agent block (e.g. GPTBot/CCBot) must not deny pages for our crawler — only
    /// rules under `*` / `SiteOne-Crawler` apply.
    #[test]
    fn disallow_all_for_other_bot_does_not_deny_our_crawler() {
        let robots = "\
User-agent: GPTBot
Disallow: /

User-agent: CCBot
Disallow: /

User-agent: *
Disallow: /checkout/cart
";
        // Normal pages stay allowed despite GPTBot/CCBot being fully blocked.
        assert!(!SeoAndOpenGraphResult::is_denied_by_robots_txt(
            "https://example.com/about",
            robots
        ));
        assert!(!SeoAndOpenGraphResult::is_denied_by_robots_txt(
            "https://example.com/",
            robots
        ));
        // Paths disallowed for `*` are still correctly denied.
        assert!(SeoAndOpenGraphResult::is_denied_by_robots_txt(
            "https://example.com/checkout/cart",
            robots
        ));
    }

    /// `Allow:` rules must override a broader `Disallow:` (the naive scanner ignored them).
    #[test]
    fn allow_overrides_disallow() {
        let robots = "\
User-agent: *
Disallow: /admin/
Allow: /admin/public/
";
        assert!(SeoAndOpenGraphResult::is_denied_by_robots_txt(
            "https://example.com/admin/secret",
            robots
        ));
        assert!(!SeoAndOpenGraphResult::is_denied_by_robots_txt(
            "https://example.com/admin/public/page",
            robots
        ));
    }

    #[test]
    fn empty_robots_txt_denies_nothing() {
        assert!(!SeoAndOpenGraphResult::is_denied_by_robots_txt(
            "https://example.com/anything",
            ""
        ));
    }
}
