// SiteOne Crawler - HeadingTreeItem
// (c) Jan Reges <jan.reges@siteone.cz>

fn html_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}

#[derive(Debug, Clone)]
pub struct HeadingTreeItem {
    /// Heading level (1-6)
    pub level: i32,
    /// Real heading level by heading structure in HTML
    pub real_level: Option<i32>,
    /// Heading text
    pub text: String,
    /// Heading ID attribute
    pub id: Option<String>,
    /// Children headings
    pub children: Vec<HeadingTreeItem>,
    /// Error text in case of error (typically multiple H1s or wrong heading level)
    pub error_text: Option<String>,
}

impl HeadingTreeItem {
    pub fn new(level: i32, text: String, id: Option<String>) -> Self {
        Self {
            level,
            real_level: None,
            text,
            id,
            children: Vec::new(),
            error_text: None,
        }
    }

    pub fn has_error(&self) -> bool {
        self.error_text.is_some()
    }

    /// Get heading tree as a plain text list
    pub fn get_heading_tree_txt_list(items: &[HeadingTreeItem]) -> String {
        let mut result = String::new();
        for item in items {
            result.push_str(&Self::get_heading_tree_txt(item, true));
        }
        // Collapse whitespace
        let re = regex::Regex::new(r"\s+").unwrap_or_else(|_| regex::Regex::new(".^").unwrap());
        re.replace_all(&result, " ").trim().to_string()
    }

    fn get_heading_tree_txt(item: &HeadingTreeItem, add_item: bool) -> String {
        let mut result = String::new();
        if add_item {
            result.push_str(&format!("<h{}> {}", item.level, item.text));
            if let Some(ref id) = item.id {
                result.push_str(&format!(" [#{}]", id));
            }
            result.push('\n');
        }
        for child in &item.children {
            result.push_str(&"  ".repeat((child.level - 1) as usize));
            result.push_str(&format!("<h{}> {}", child.level, child.text));
            if let Some(ref id) = child.id {
                result.push_str(&format!(" [#{}]", id));
            }
            result.push('\n');
            result.push_str(&Self::get_heading_tree_txt(child, false));
        }
        result
    }

    /// Get heading tree as an HTML `<ul><li>` list.
    pub fn get_heading_tree_ul_li_list(items: &[HeadingTreeItem]) -> String {
        let mut result = String::from("<ul>");
        for item in items {
            result.push_str("<li>");
            result.push_str(&Self::get_heading_tree_ul_li(item, true));
            result.push_str("</li>");
        }
        result.push_str("</ul>");
        result
    }

    fn get_heading_tree_ul_li(item: &HeadingTreeItem, add_item: bool) -> String {
        let mut result = String::new();
        if add_item {
            let txt_row = format!(
                "&lt;h{}&gt; {}{}",
                item.level,
                html_escape(&item.text),
                item.id
                    .as_ref()
                    .map(|id| format!(" [#{}]", html_escape(id)))
                    .unwrap_or_default()
            );
            if item.has_error() {
                let error_text = html_escape(item.error_text.as_deref().unwrap_or(""));
                let colored = crate::utils::get_color_text(&txt_row, "magenta", false);
                let colored_html = crate::utils::convert_bash_colors_in_text_to_html(&colored);
                result.push_str(&format!(
                    "<span class=\"help\" title=\"{}\">{}</span>",
                    error_text, colored_html
                ));
            } else {
                result.push_str(&txt_row);
            }
        }

        if !item.children.is_empty() {
            result.push_str("<ul>");
            for child in &item.children {
                result.push_str("<li>");
                let txt_row = format!(
                    "&lt;h{}&gt; {}{}",
                    child.level,
                    html_escape(&child.text),
                    child
                        .id
                        .as_ref()
                        .map(|id| format!(" [#{}]", html_escape(id)))
                        .unwrap_or_default()
                );
                if child.has_error() {
                    let error_text = html_escape(child.error_text.as_deref().unwrap_or(""));
                    let colored = crate::utils::get_color_text(&txt_row, "magenta", false);
                    let colored_html = crate::utils::convert_bash_colors_in_text_to_html(&colored);
                    result.push_str(&format!(
                        "<span class=\"help\" title=\"{}\">{}</span>",
                        error_text, colored_html
                    ));
                } else {
                    result.push_str(&txt_row);
                }
                result.push_str(&Self::get_heading_tree_ul_li(child, false));
                result.push_str("</li>");
            }
            result.push_str("</ul>");
        }
        result
    }

    /// Count total headings in tree
    pub fn get_headings_count(items: &[HeadingTreeItem]) -> usize {
        let mut count = 0;
        for item in items {
            count += 1;
            count += Self::get_headings_count(&item.children);
        }
        count
    }

    /// Count headings with errors in tree
    pub fn get_headings_with_error_count(items: &[HeadingTreeItem]) -> usize {
        let mut count = 0;
        for item in items {
            if item.has_error() {
                count += 1;
            }
            count += Self::get_headings_with_error_count(&item.children);
        }
        count
    }
}
