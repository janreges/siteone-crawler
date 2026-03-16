// SiteOne Crawler - Tab for HTML Report
// (c) Jan Reges <jan.reges@siteone.cz>

use regex::Regex;

use super::badge::Badge;

/// A tab in the HTML report
#[derive(Debug, Clone)]
pub struct Tab {
    pub name: String,
    pub description: Option<String>,
    pub radio_html_id: String,
    pub content_html_id: String,
    pub tab_content: String,
    pub add_heading: bool,
    pub fixed_order: Option<i32>,
    pub order: Option<i32>,
    pub badges: Vec<Badge>,
}

impl Tab {
    pub fn new(
        name: &str,
        description: Option<&str>,
        tab_content: String,
        add_heading: bool,
        badges: Vec<Badge>,
        fixed_order: Option<i32>,
    ) -> Self {
        let sanitized = sanitize_id(name);
        let radio_html_id = format!("radio_{}", sanitized);
        let content_html_id = format!("content_{}", sanitized);

        Self {
            name: name.to_string(),
            description: description.map(|s| s.to_string()),
            radio_html_id,
            content_html_id,
            tab_content,
            add_heading,
            fixed_order,
            order: None,
            badges,
        }
    }

    pub fn set_order(&mut self, order: Option<i32>) {
        self.order = order;
    }

    /// Returns the final sort order: order > fixed_order > 1000 (default)
    pub fn get_final_sort_order(&self) -> i32 {
        if let Some(order) = self.order {
            order
        } else {
            self.fixed_order.unwrap_or(1000)
        }
    }
}

/// Sanitize a tab name into a valid HTML ID
fn sanitize_id(name: &str) -> String {
    let re = Regex::new(r"[^a-zA-Z0-9\-]+").unwrap_or_else(|_| Regex::new(r"\W+").unwrap());
    re.replace_all(name, "_").to_lowercase()
}
