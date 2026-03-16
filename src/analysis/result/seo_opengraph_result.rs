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

    pub title: Option<String>,
    pub description: Option<String>,
    pub keywords: Option<String>,
    pub h1: Option<String>,

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
            title: None,
            description: None,
            keywords: None,
            h1: None,
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

    /// Check if URL is denied by robots.txt
    pub fn is_denied_by_robots_txt(url_path_and_query: &str, robots_txt_content: &str) -> bool {
        if robots_txt_content.is_empty() {
            return false;
        }

        // Remove query string from URL
        let url_path = if let Some(pos) = url_path_and_query.find('?') {
            &url_path_and_query[..pos]
        } else {
            url_path_and_query
        };

        // Remove scheme and host from URL if present
        let url_path = if url_path.contains("://") {
            if let Ok(parsed) = url::Url::parse(url_path) {
                parsed.path().to_string()
            } else {
                url_path.to_string()
            }
        } else {
            url_path.to_string()
        };

        for line in robots_txt_content.lines() {
            let line = line.trim();
            if let Some(disallowed_path) = line.strip_prefix("Disallow:") {
                let disallowed_path = disallowed_path.trim();
                if !disallowed_path.is_empty() && url_path.starts_with(disallowed_path) {
                    return true;
                }
            }
        }

        false
    }
}
