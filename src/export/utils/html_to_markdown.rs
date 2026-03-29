// SiteOne Crawler - HtmlToMarkdownConverter
// (c) Jan Reges <jan.reges@siteone.cz>
//
// Converts HTML to Markdown format using the scraper crate for HTML parsing.

use std::collections::HashMap;

use ego_tree::NodeRef;
use once_cell::sync::Lazy;
use regex::Regex;
use scraper::{Html, Node, Selector};

static RE_NON_ALNUM: Lazy<Regex> = Lazy::new(|| Regex::new(r"[^a-z0-9]+").unwrap());

/// Converts HTML content to Markdown format.
/// Handles all HTML elements: headings, paragraphs, bold/italic, links, images,
/// lists, tables, blockquotes, code blocks, horizontal rules, etc.
pub struct HtmlToMarkdownConverter {
    html: String,
    excluded_selectors: Vec<String>,
    implicit_excluded_selectors: Vec<String>,
    strong_delimiter: String,
    em_delimiter: String,
    bullet_list_marker: String,
    code_block_fence: String,
    horizontal_rule: String,
    heading_style: HeadingStyle,
    escape_mode: bool,
    include_images: bool,
    convert_tables: bool,
    convert_strikethrough: bool,
    strikethrough_delimiter: String,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum HeadingStyle {
    Atx,
    Setext,
}

impl HtmlToMarkdownConverter {
    pub fn new(html: &str, excluded_selectors: Vec<String>) -> Self {
        Self {
            html: html.to_string(),
            excluded_selectors,
            implicit_excluded_selectors: vec![
                // Hidden elements
                ".hidden".to_string(),
                ".hide".to_string(),
                ".invisible".to_string(),
                ".lg\\:sl-hidden".to_string(),
                ".md\\:sl-hidden".to_string(),
                ".lg\\:hidden".to_string(),
                ".md\\:hidden".to_string(),
                // ARIA hidden and menu elements
                "[aria-hidden='true']".to_string(),
                "[role='menu']".to_string(),
                // Cookie consent banners
                ".cookie-panel".to_string(),
                ".cookie-banner".to_string(),
                ".cookie-consent".to_string(),
                ".cookie-notice".to_string(),
                ".cookie-bar".to_string(),
                "#cookie-banner".to_string(),
                "#cookie-consent".to_string(),
                "#cookie-notice".to_string(),
                "#cookiebanner".to_string(),
                "#CybotCookiebotDialog".to_string(),
                ".cc-window".to_string(),
                "#onetrust-banner-sdk".to_string(),
            ],
            strong_delimiter: "**".to_string(),
            em_delimiter: "*".to_string(),
            bullet_list_marker: "-".to_string(),
            code_block_fence: "```".to_string(),
            horizontal_rule: "* * *".to_string(),
            heading_style: HeadingStyle::Atx,
            escape_mode: true,
            include_images: true,
            convert_tables: true,
            convert_strikethrough: true,
            strikethrough_delimiter: "~~".to_string(),
        }
    }

    pub fn set_strong_delimiter(&mut self, delimiter: &str) -> &mut Self {
        self.strong_delimiter = delimiter.to_string();
        self
    }

    pub fn set_em_delimiter(&mut self, delimiter: &str) -> &mut Self {
        self.em_delimiter = delimiter.to_string();
        self
    }

    pub fn set_bullet_list_marker(&mut self, marker: &str) -> &mut Self {
        if ["-", "*", "+"].contains(&marker) {
            self.bullet_list_marker = marker.to_string();
        }
        self
    }

    pub fn set_code_block_fence(&mut self, fence: &str) -> &mut Self {
        if fence.len() >= 3 && fence.starts_with('`') {
            self.code_block_fence = fence.to_string();
        }
        self
    }

    pub fn set_horizontal_rule(&mut self, rule: &str) -> &mut Self {
        self.horizontal_rule = rule.to_string();
        self
    }

    pub fn set_heading_style(&mut self, style: HeadingStyle) -> &mut Self {
        self.heading_style = style;
        self
    }

    pub fn set_escape_mode(&mut self, enable: bool) -> &mut Self {
        self.escape_mode = enable;
        self
    }

    pub fn set_include_images(&mut self, include: bool) -> &mut Self {
        self.include_images = include;
        self
    }

    pub fn set_convert_tables(&mut self, convert: bool) -> &mut Self {
        self.convert_tables = convert;
        self
    }

    pub fn set_convert_strikethrough(&mut self, convert: bool) -> &mut Self {
        self.convert_strikethrough = convert;
        self
    }

    pub fn set_strikethrough_delimiter(&mut self, delimiter: &str) -> &mut Self {
        self.strikethrough_delimiter = delimiter.to_string();
        self
    }

    /// Convert the HTML to Markdown.
    pub fn get_markdown(&self) -> String {
        let document = Html::parse_document(&self.html);

        // Remove excluded selectors from the document - we'll skip these during conversion
        let excluded_ids = self.collect_excluded_node_ids(&document);

        // Try to get the body element first, fallback to documentElement
        let body_selector = Selector::parse("body").unwrap_or_else(|_| Selector::parse("*").unwrap());
        let start_node = document
            .select(&body_selector)
            .next()
            .map(|el| el.id())
            .unwrap_or_else(|| document.root_element().id());

        let node_ref = document.tree.get(start_node);
        let raw_markdown = match node_ref {
            Some(node) => self.convert_node(&node, &document, &excluded_ids),
            None => return String::new(),
        };

        let normalized = self.normalize_whitespace(&raw_markdown);

        // Deduplication logic
        let blocks: Vec<&str> = normalized.split("\n\n").collect();
        if blocks.len() <= 1 {
            let result = normalized.trim().to_string();
            return self.post_process(&result);
        }

        let mut fingerprints: HashMap<String, (String, usize)> = HashMap::new();
        let mut unique_blocks: Vec<(usize, String)> = Vec::new();

        for (index, original_block) in blocks.iter().enumerate() {
            let trimmed = original_block.trim();

            if trimmed.is_empty() {
                unique_blocks.push((index, original_block.to_string()));
                continue;
            }

            // Create fingerprint: lowercase alphanumeric only
            let fingerprint = RE_NON_ALNUM.replace_all(&trimmed.to_lowercase(), "").to_string();

            if fingerprint.is_empty() {
                unique_blocks.push((index, original_block.to_string()));
                continue;
            }

            if let Some((existing_block, existing_index)) = fingerprints.get(&fingerprint) {
                // Duplicate found - keep the longer one
                if trimmed.len() > existing_block.trim().len() {
                    // Remove the shorter one
                    unique_blocks.retain(|(idx, _)| *idx != *existing_index);
                    unique_blocks.push((index, original_block.to_string()));
                    fingerprints.insert(fingerprint, (original_block.to_string(), index));
                }
                // else: existing is longer or equal, discard current
            } else {
                fingerprints.insert(fingerprint, (original_block.to_string(), index));
                unique_blocks.push((index, original_block.to_string()));
            }
        }

        // Sort by original index to preserve order
        unique_blocks.sort_by_key(|(idx, _)| *idx);

        let final_markdown: String = unique_blocks
            .into_iter()
            .map(|(_, block)| block)
            .collect::<Vec<_>>()
            .join("\n\n");

        self.post_process(&final_markdown)
    }

    fn post_process(&self, markdown: &str) -> String {
        // Replace backslashes with actual characters
        let result = Regex::new(r"\\([.\-])")
            .map(|re| re.replace_all(markdown, "$1").to_string())
            .unwrap_or_else(|_| markdown.to_string());

        result.trim().to_string()
    }

    /// Minimum number of links in a list block to trigger collapsing into accordion.
    pub const MIN_LINKS_FOR_COLLAPSE: usize = 8;

    /// Collapse large link lists into `<details>` accordions.
    /// First collapsed list on the page gets "Menu" label, subsequent get "Links".
    pub fn collapse_large_link_lists(markdown: &str) -> String {
        let lines: Vec<&str> = markdown.lines().collect();
        let len = lines.len();
        let mut result_lines: Vec<String> = Vec::with_capacity(len);
        let mut is_first_collapse = true;
        let mut i = 0;

        while i < len {
            // Check if this line starts a list block
            if Self::is_list_item(lines[i]) {
                let block_start = i;

                // Consume the entire list block
                while i < len {
                    if Self::is_list_item(lines[i]) || Self::is_list_continuation(lines[i]) {
                        i += 1;
                    } else if lines[i].trim().is_empty() {
                        // Blank line — check if the list continues after it
                        let mut next_non_blank = i + 1;
                        while next_non_blank < len && lines[next_non_blank].trim().is_empty() {
                            next_non_blank += 1;
                        }
                        if next_non_blank < len && Self::is_list_item(lines[next_non_blank]) {
                            // List continues after blank line(s)
                            i = next_non_blank;
                        } else {
                            break;
                        }
                    } else {
                        break;
                    }
                }

                let block_end = i;
                let block_lines = &lines[block_start..block_end];

                // Count lines containing markdown links
                let link_count = block_lines.iter().filter(|line| line.contains("](")).count();

                if link_count > Self::MIN_LINKS_FOR_COLLAPSE {
                    let label = if is_first_collapse { "Menu" } else { "Links" };
                    is_first_collapse = false;
                    result_lines.push("<details>".to_string());
                    result_lines.push(format!("<summary>{}</summary>", label));
                    result_lines.push(String::new());
                    for line in block_lines {
                        result_lines.push(line.to_string());
                    }
                    result_lines.push(String::new());
                    result_lines.push("</details>".to_string());
                    result_lines.push(String::new()); // blank line required so next Markdown (e.g. heading) isn't swallowed into the HTML block
                } else {
                    for line in block_lines {
                        result_lines.push(line.to_string());
                    }
                }
            } else {
                result_lines.push(lines[i].to_string());
                i += 1;
            }
        }

        result_lines.join("\n")
    }

    /// Check if a line is a list item (starts with `- `, `* `, `+ `, or numbered `1. `).
    fn is_list_item(line: &str) -> bool {
        let trimmed = line.trim_start();
        trimmed.starts_with("- ")
            || trimmed.starts_with("* ")
            || trimmed.starts_with("+ ")
            || trimmed.bytes().next().is_some_and(|b| b.is_ascii_digit()) && trimmed.contains(". ")
    }

    /// Check if a line is a continuation of a list item (indented text that's not a new item).
    fn is_list_continuation(line: &str) -> bool {
        let trimmed = line.trim_start();
        // Indented non-empty line that's not a list item itself
        line.len() > trimmed.len() && !trimmed.is_empty()
    }

    /// Collect node IDs of elements matching excluded selectors
    fn collect_excluded_node_ids(&self, document: &Html) -> Vec<ego_tree::NodeId> {
        let mut excluded = Vec::new();
        let all_selectors: Vec<&str> = self
            .excluded_selectors
            .iter()
            .chain(self.implicit_excluded_selectors.iter())
            .map(|s| s.as_str())
            .collect();

        for selector_str in all_selectors {
            if let Ok(selector) = Selector::parse(selector_str) {
                for element in document.select(&selector) {
                    excluded.push(element.id());
                    // Also exclude all descendants
                    for descendant in element.descendants() {
                        excluded.push(descendant.id());
                    }
                }
            }
        }

        // Also collect unwanted tags: script, style, noscript, head, meta, link, iframe, frame
        for tag in &["script", "style", "noscript", "head", "meta", "link", "iframe", "frame"] {
            if let Ok(selector) = Selector::parse(tag) {
                for element in document.select(&selector) {
                    excluded.push(element.id());
                    for descendant in element.descendants() {
                        excluded.push(descendant.id());
                    }
                }
            }
        }

        excluded
    }

    /// Convert a DOM node to Markdown.
    fn convert_node(&self, node: &NodeRef<Node>, document: &Html, excluded: &[ego_tree::NodeId]) -> String {
        if excluded.contains(&node.id()) {
            return String::new();
        }

        match node.value() {
            Node::Text(text) => {
                let text_content = text.text.to_string();
                // Check parent context
                if let Some(parent) = node.parent()
                    && let Node::Element(el) = parent.value()
                {
                    let tag = el.name.local.as_ref();
                    if tag == "code" || tag == "pre" {
                        return text_content;
                    }
                }
                self.escape_markdown_chars(&text_content)
            }
            Node::Element(el) => {
                let tag = el.name.local.as_ref().to_lowercase();
                match tag.as_str() {
                    "strong" | "b" => {
                        let inner = self.collapse_inline_whitespace(&self.get_inner_markdown(node, document, excluded));
                        self.wrap_with_delimiter(&inner, &self.strong_delimiter)
                    }
                    "em" | "i" => {
                        let inner = self.collapse_inline_whitespace(&self.get_inner_markdown(node, document, excluded));
                        self.wrap_with_delimiter(&inner, &self.em_delimiter)
                    }
                    "h1" | "h2" | "h3" | "h4" | "h5" | "h6" => self.convert_heading(node, document, excluded),
                    "p" => {
                        let inner = self.get_inner_markdown(node, document, excluded).trim().to_string();
                        if inner.is_empty() {
                            String::new()
                        } else {
                            format!("\n\n{}\n\n", inner)
                        }
                    }
                    "br" => "  \n".to_string(),
                    "hr" => format!("\n\n{}\n\n", self.horizontal_rule),
                    "a" => self.convert_link(node, document, excluded),
                    "img" => self.convert_image(node),
                    "code" => self.convert_inline_code(node),
                    "pre" => self.convert_code_block(node, document),
                    "ul" | "ol" => self.convert_list_to_markdown(node, document, excluded),
                    "blockquote" => self.convert_blockquote(node, document, excluded),
                    "table" => self.convert_table(node, document, excluded),
                    "s" | "del" | "strike" => {
                        if !self.convert_strikethrough {
                            return self.get_inner_markdown(node, document, excluded);
                        }
                        let inner = self.collapse_inline_whitespace(&self.get_inner_markdown(node, document, excluded));
                        self.wrap_with_delimiter(&inner, &self.strikethrough_delimiter)
                    }
                    "dl" => self.convert_definition_list(node, document, excluded),
                    "dt" | "dd" => self.get_inner_markdown(node, document, excluded),
                    "sup" => {
                        let inner = self.collapse_inline_whitespace(&self.get_inner_markdown(node, document, excluded));
                        format!("^{}^", inner)
                    }
                    "sub" => {
                        let inner = self.collapse_inline_whitespace(&self.get_inner_markdown(node, document, excluded));
                        format!("~{}~", inner)
                    }
                    // Ignored form/non-content elements
                    "form" | "fieldset" | "legend" | "label" | "dialog" | "button" | "input" | "select"
                    | "textarea" | "script" | "style" | "noscript" | "head" | "meta" | "link" | "iframe" | "frame" => {
                        String::new()
                    }
                    // Block container elements - wrap with newlines to prevent text concatenation
                    "nav" | "header" | "footer" | "aside" | "article" | "section" | "main" | "figure"
                    | "figcaption" | "div" => {
                        let inner = self.get_inner_markdown(node, document, excluded);
                        let trimmed = inner.trim();
                        if trimmed.is_empty() {
                            String::new()
                        } else {
                            format!("\n\n{}\n\n", trimmed)
                        }
                    }
                    // Inline container elements
                    "span" => self.get_inner_markdown(node, document, excluded),
                    _ => self.get_inner_markdown(node, document, excluded),
                }
            }
            Node::Comment(_) => String::new(),
            _ => String::new(),
        }
    }

    /// Get inner markdown by processing all children of a node.
    fn get_inner_markdown(&self, node: &NodeRef<Node>, document: &Html, excluded: &[ego_tree::NodeId]) -> String {
        let mut markdown = String::new();
        let mut consecutive_links: Vec<ego_tree::NodeId> = Vec::new();

        for child in node.children() {
            if excluded.contains(&child.id()) {
                continue;
            }

            let is_valid_link = self.is_valid_link_node(&child);

            if is_valid_link {
                consecutive_links.push(child.id());
            } else if matches!(child.value(), Node::Text(t) if t.text.trim().is_empty())
                && !consecutive_links.is_empty()
            {
                // Ignore whitespace between links
                continue;
            } else {
                // Process collected links
                if consecutive_links.len() >= 2 {
                    markdown.push_str(&self.convert_consecutive_links_to_table(&consecutive_links, document, excluded));
                } else if consecutive_links.len() == 1
                    && let Some(link_node) = document.tree.get(consecutive_links[0])
                {
                    markdown.push_str(&self.convert_link(&link_node, document, excluded));
                }
                consecutive_links.clear();

                markdown.push_str(&self.convert_node(&child, document, excluded));
            }
        }

        // Process remaining links
        if consecutive_links.len() >= 2 {
            markdown.push_str(&self.convert_consecutive_links_to_table(&consecutive_links, document, excluded));
        } else if consecutive_links.len() == 1
            && let Some(link_node) = document.tree.get(consecutive_links[0])
        {
            markdown.push_str(&self.convert_link(&link_node, document, excluded));
        }

        markdown
    }

    /// Check if a node is a valid link for consecutive link detection.
    fn is_valid_link_node(&self, node: &NodeRef<Node>) -> bool {
        if let Node::Element(el) = node.value() {
            if el.name.local.as_ref() != "a" {
                return false;
            }
            let href = el.attr("href");
            if href.map(|v| v.is_empty()).unwrap_or(true) {
                return false;
            }
            // Must have text content or image child
            let text_content = self.extract_text_content(node).trim().to_string();
            let has_image = node
                .descendants()
                .any(|d| matches!(d.value(), Node::Element(e) if e.name.local.as_ref() == "img"));
            !text_content.is_empty() || has_image
        } else {
            false
        }
    }

    /// Extract plain text content from a node recursively.
    fn extract_text_content(&self, node: &NodeRef<Node>) -> String {
        let mut text = String::new();
        for child in node.descendants() {
            if let Node::Text(t) = child.value() {
                text.push_str(&t.text);
            }
        }
        text
    }

    /// Collapse multiple whitespace characters into a single space.
    fn collapse_inline_whitespace(&self, text: &str) -> String {
        let text = text.replace("&nbsp;", " ").replace('\u{00A0}', " ");
        Regex::new(r"\s+")
            .map(|re| re.replace_all(&text, " ").trim().to_string())
            .unwrap_or_else(|_| text.trim().to_string())
    }

    /// Convert heading element to Markdown.
    fn convert_heading(&self, node: &NodeRef<Node>, document: &Html, excluded: &[ego_tree::NodeId]) -> String {
        if let Node::Element(el) = node.value() {
            let tag = el.name.local.as_ref();
            let level: usize = tag[1..].parse().unwrap_or(1);
            let content = self.collapse_inline_whitespace(&self.get_inner_markdown(node, document, excluded));
            // Remove markdown characters that might interfere inside headings
            let content = content.replace(['#', '*', '_', '`', '[', ']'], "");
            let content = content.trim().to_string();

            if content.is_empty() {
                return String::new();
            }

            if self.heading_style == HeadingStyle::Setext && level <= 2 {
                let underline_char = if level == 1 { '=' } else { '-' };
                let underline = underline_char.to_string().repeat(content.chars().count());
                format!("\n\n{}\n{}\n\n", content, underline)
            } else {
                let prefix = "#".repeat(level);
                format!("\n\n{} {}\n\n", prefix, content)
            }
        } else {
            String::new()
        }
    }

    /// Convert link element to Markdown.
    fn convert_link(&self, node: &NodeRef<Node>, document: &Html, excluded: &[ego_tree::NodeId]) -> String {
        if let Node::Element(el) = node.value() {
            let href = el.attr("href").unwrap_or("").to_string();

            if href.is_empty() {
                return self.get_inner_markdown(node, document, excluded);
            }

            let text = self.collapse_inline_whitespace(&self.get_inner_markdown(node, document, excluded));

            let text = if !text.is_empty() {
                text
            } else if let Some(aria_label) = el.attr("aria-label") {
                let label = aria_label.trim().to_string();
                if label.is_empty() { href.clone() } else { label }
            } else {
                href.clone()
            };

            let title = el.attr("title").unwrap_or("").to_string();

            let mut markdown = format!("[{}]({}", text, href);
            if !title.is_empty() {
                markdown.push_str(&format!(" \"{}\"", self.escape_markdown_chars(&title)));
            }
            markdown.push(')');

            markdown
        } else {
            String::new()
        }
    }

    /// Convert image element to Markdown.
    fn convert_image(&self, node: &NodeRef<Node>) -> String {
        if let Node::Element(el) = node.value() {
            if !self.include_images {
                let alt = el.attr("alt").unwrap_or("").to_string();
                return if alt.is_empty() {
                    String::new()
                } else {
                    self.escape_markdown_chars(&alt)
                };
            }

            let alt = self.collapse_inline_whitespace(el.attr("alt").unwrap_or(""));
            let src = el.attr("src").unwrap_or("").to_string();
            let title = el.attr("title").unwrap_or("").to_string();

            if src.is_empty() {
                return String::new();
            }

            let title = self.escape_markdown_chars(&title);

            let mut markdown = format!("![{}]({}", alt, src);
            if !title.is_empty() {
                markdown.push_str(&format!(" \"{}\"", title));
            }
            markdown.push(')');

            format!("\n\n{}\n\n", markdown)
        } else {
            String::new()
        }
    }

    /// Convert inline code element to Markdown.
    fn convert_inline_code(&self, node: &NodeRef<Node>) -> String {
        let code = self.extract_text_content(node);
        let trimmed_code = code.trim();

        // Determine required backticks
        let mut max_backticks = 0usize;
        let mut current_count = 0usize;
        for ch in code.chars() {
            if ch == '`' {
                current_count += 1;
                max_backticks = max_backticks.max(current_count);
            } else {
                current_count = 0;
            }
        }
        let fence = "`".repeat(max_backticks + 1);

        let prefix_space = if trimmed_code.starts_with('`') { " " } else { "" };
        let suffix_space = if trimmed_code.ends_with('`') { " " } else { "" };

        format!("{}{}{}{}{}", fence, prefix_space, trimmed_code, suffix_space, fence)
    }

    /// Convert pre/code block to Markdown.
    fn convert_code_block(&self, node: &NodeRef<Node>, _document: &Html) -> String {
        // Find inner <code> element if present
        let code_text = node
            .descendants()
            .find(|d| matches!(d.value(), Node::Element(e) if e.name.local.as_ref() == "code"))
            .map(|code_node| self.extract_text_content(&code_node))
            .unwrap_or_else(|| self.extract_text_content(node));

        let code = code_text.trim_matches(|c: char| c == '\n' || c == '\r');

        // Replace '\' followed by multiple spaces with '\' + newline + spaces
        let code = Regex::new(r"(\\)(\s{2,})")
            .map(|re| re.replace_all(code, "$1\n$2").to_string())
            .unwrap_or_else(|_| code.to_string());

        // Detect language from class attribute
        let mut language = String::new();

        // Check class on <pre> or inner <code>
        let class_attr = if let Node::Element(el) = node.value() {
            el.attr("class").map(|v| v.to_string())
        } else {
            None
        };

        let class_to_check = class_attr.or_else(|| {
            node.descendants()
                .find(|d| matches!(d.value(), Node::Element(e) if e.name.local.as_ref() == "code"))
                .and_then(|code_node| {
                    if let Node::Element(el) = code_node.value() {
                        el.attr("class").map(|v| v.to_string())
                    } else {
                        None
                    }
                })
        });

        if let Some(class_val) = class_to_check {
            for class in class_val.split_whitespace() {
                if let Some(lang) = class.strip_prefix("language-") {
                    language = lang.to_string();
                    break;
                } else if let Some(lang) = class.strip_prefix("lang-") {
                    language = lang.to_string();
                    break;
                }
            }
        }

        // Clean language identifier
        language = language.replace(|c: char| c.is_whitespace() || c == '`', "");

        format!(
            "\n\n{}{}\n{}\n{}\n\n",
            self.code_block_fence, language, code, self.code_block_fence
        )
    }

    /// Convert blockquote element to Markdown.
    fn convert_blockquote(&self, node: &NodeRef<Node>, document: &Html, excluded: &[ego_tree::NodeId]) -> String {
        let content = self.get_inner_markdown(node, document, excluded);
        let content = content.trim();
        if content.is_empty() {
            return String::new();
        }

        let mut markdown = String::new();
        for line in content.lines() {
            markdown.push_str(&format!("> {}\n", line));
        }

        format!("\n\n{}\n\n", markdown.trim_end())
    }

    /// Convert table element to Markdown.
    fn convert_table(&self, node: &NodeRef<Node>, document: &Html, excluded: &[ego_tree::NodeId]) -> String {
        if !self.convert_tables {
            // Return clean HTML table
            return format!("\n\n{}\n\n", self.extract_text_content(node).trim());
        }

        let mut rows: Vec<Vec<String>> = Vec::new();
        let mut header_cells: Vec<String> = Vec::new();
        let mut max_col_lengths: Vec<usize> = Vec::new();
        let mut has_header = false;

        // Process thead
        for child in node.children() {
            if let Node::Element(el) = child.value() {
                let tag = el.name.local.as_ref();
                if tag == "thead" {
                    has_header = true;
                    // Find tr in thead
                    for thead_child in child.children() {
                        if let Node::Element(tr_el) = thead_child.value()
                            && tr_el.name.local.as_ref() == "tr"
                        {
                            let mut col_index = 0;
                            for cell_node in thead_child.children() {
                                if let Node::Element(cell_el) = cell_node.value() {
                                    let cell_tag = cell_el.name.local.as_ref();
                                    if cell_tag == "th" || cell_tag == "td" {
                                        let content = self.extract_header_content(&cell_node, document, excluded);
                                        while max_col_lengths.len() <= col_index {
                                            max_col_lengths.push(0);
                                        }
                                        max_col_lengths[col_index] =
                                            max_col_lengths[col_index].max(content.chars().count());
                                        header_cells.push(content);
                                        col_index += 1;
                                    }
                                }
                            }
                            break; // Only first tr in thead
                        }
                    }
                }
            }
        }

        // Process tbody and direct tr children
        let mut direct_trs: Vec<ego_tree::NodeId> = Vec::new();
        for child in node.children() {
            if let Node::Element(el) = child.value() {
                let tag = el.name.local.as_ref();
                if tag == "tbody" {
                    for tbody_child in child.children() {
                        if let Node::Element(tr_el) = tbody_child.value()
                            && tr_el.name.local.as_ref() == "tr"
                        {
                            direct_trs.push(tbody_child.id());
                        }
                    }
                } else if tag == "tr" && !has_header && direct_trs.is_empty() {
                    direct_trs.push(child.id());
                }
            }
        }

        // If no tbody, look for direct TR children
        if direct_trs.is_empty() && !has_header {
            for child in node.children() {
                if let Node::Element(el) = child.value()
                    && el.name.local.as_ref() == "tr"
                {
                    direct_trs.push(child.id());
                }
            }
        }

        for tr_id in &direct_trs {
            if let Some(tr_node) = document.tree.get(*tr_id) {
                // If no header found yet, check if first row has <th>
                if !has_header && rows.is_empty() {
                    let mut potential_header: Vec<String> = Vec::new();
                    let mut is_potential_header = false;

                    for cell_node in tr_node.children() {
                        if let Node::Element(cell_el) = cell_node.value() {
                            let cell_tag = cell_el.name.local.as_ref();
                            if cell_tag == "th" || cell_tag == "td" {
                                if cell_tag == "th" {
                                    is_potential_header = true;
                                }
                                let content = self.extract_header_content(&cell_node, document, excluded);
                                let col_index = potential_header.len();
                                while max_col_lengths.len() <= col_index {
                                    max_col_lengths.push(0);
                                }
                                max_col_lengths[col_index] = max_col_lengths[col_index].max(content.chars().count());
                                potential_header.push(content);
                            }
                        }
                    }

                    if is_potential_header {
                        header_cells = potential_header;
                        has_header = true;
                        continue;
                    }
                }

                // Process as data row
                let mut row_cells: Vec<String> = Vec::new();
                for cell_node in tr_node.children() {
                    if let Node::Element(cell_el) = cell_node.value() {
                        let cell_tag = cell_el.name.local.as_ref();
                        if cell_tag == "th" || cell_tag == "td" {
                            let content = self
                                .collapse_inline_whitespace(&self.get_inner_markdown(&cell_node, document, excluded));
                            let col_index = row_cells.len();
                            while max_col_lengths.len() <= col_index {
                                max_col_lengths.push(0);
                            }
                            max_col_lengths[col_index] = max_col_lengths[col_index].max(content.chars().count());
                            row_cells.push(content);
                        }
                    }
                }

                // Pad row if fewer cells than max columns
                let num_cols = max_col_lengths.len();
                while row_cells.len() < num_cols {
                    row_cells.push(String::new());
                }

                rows.push(row_cells);
            }
        }

        if header_cells.is_empty() && rows.is_empty() {
            return String::new();
        }

        // Determine number of columns
        let mut num_cols = header_cells.len();
        for row in &rows {
            num_cols = num_cols.max(row.len());
        }

        // Ensure min length 3 for separator
        while max_col_lengths.len() < num_cols {
            max_col_lengths.push(0);
        }
        for length in &mut max_col_lengths {
            *length = (*length).max(3);
        }

        let mut markdown = "\n\n".to_string();
        if !header_cells.is_empty() {
            while header_cells.len() < num_cols {
                header_cells.push(String::new());
            }
            markdown.push_str(&self.format_table_row(&header_cells, &max_col_lengths));
            markdown.push_str(&self.format_table_separator(&max_col_lengths));
        } else {
            markdown.push_str(&self.format_table_separator(&max_col_lengths));
        }

        for row in &rows {
            let mut padded_row = row.clone();
            while padded_row.len() < num_cols {
                padded_row.push(String::new());
            }
            markdown.push_str(&self.format_table_row(&padded_row, &max_col_lengths));
        }

        format!("{}\n\n", markdown.trim_end())
    }

    /// Extract header content from a table header cell.
    fn extract_header_content(&self, cell: &NodeRef<Node>, document: &Html, excluded: &[ego_tree::NodeId]) -> String {
        let content = self.collapse_inline_whitespace(&self.get_inner_markdown(cell, document, excluded));

        if content.trim().is_empty() {
            // Fallback: extract text content directly
            self.collapse_inline_whitespace(&self.extract_text_content(cell))
        } else {
            content
        }
    }

    /// Convert consecutive links to a table.
    fn convert_consecutive_links_to_table(
        &self,
        link_ids: &[ego_tree::NodeId],
        document: &Html,
        excluded: &[ego_tree::NodeId],
    ) -> String {
        let mut cells: Vec<String> = Vec::new();
        let mut max_col_lengths: Vec<usize> = Vec::new();

        for link_id in link_ids {
            if let Some(link_node) = document.tree.get(*link_id) {
                let cell_content = self.convert_link(&link_node, document, excluded);
                if cell_content.is_empty() {
                    continue;
                }
                max_col_lengths.push(cell_content.chars().count().max(3));
                cells.push(cell_content);
            }
        }

        if cells.is_empty() {
            return String::new();
        }

        let mut markdown = "\n\n".to_string();
        markdown.push_str(&self.format_table_row(&cells, &max_col_lengths));

        format!("{}\n", markdown)
    }

    /// Format a table row.
    fn format_table_row(&self, cells: &[String], max_lengths: &[usize]) -> String {
        let mut row = "|".to_string();
        for (i, cell) in cells.iter().enumerate() {
            let max_length = max_lengths.get(i).copied().unwrap_or(cell.chars().count());
            let padding_len = max_length.saturating_sub(cell.chars().count());
            let padding = " ".repeat(padding_len);
            let escaped = self.escape_markdown_table_cell_content(cell);
            row.push_str(&format!(" {}{} |", escaped, padding));
        }
        row.push('\n');
        row
    }

    /// Format a table separator row.
    fn format_table_separator(&self, max_lengths: &[usize]) -> String {
        let mut separator = "|".to_string();
        for length in max_lengths {
            let dash_count = (*length).max(3);
            separator.push_str(&format!(" {} |", "-".repeat(dash_count)));
        }
        separator.push('\n');
        separator
    }

    /// Wrap text with a delimiter.
    fn wrap_with_delimiter(&self, text: &str, delimiter: &str) -> String {
        if text.trim().is_empty() {
            return text.to_string();
        }
        format!("{}{}{}", delimiter, text.trim(), delimiter)
    }

    /// Escape Markdown special characters.
    fn escape_markdown_chars(&self, text: &str) -> String {
        if !self.escape_mode {
            return text.to_string();
        }
        let mut result = text.replace('\\', "\\\\");
        for ch in &[
            '`', '*', '_', '{', '}', '[', ']', '(', ')', '#', '+', '-', '.', '!', '|',
        ] {
            result = result.replace(*ch, &format!("\\{}", ch));
        }
        result
    }

    /// Escape pipe character in table cells.
    fn escape_markdown_table_cell_content(&self, text: &str) -> String {
        text.replace('|', "\\|")
    }

    /// Convert definition list.
    fn convert_definition_list(&self, node: &NodeRef<Node>, document: &Html, excluded: &[ego_tree::NodeId]) -> String {
        let mut markdown = String::new();
        let mut dt_content: Option<String> = None;

        for child in node.children() {
            if let Node::Element(el) = child.value() {
                let tag = el.name.local.as_ref();
                if tag == "dt" {
                    if let Some(ref content) = dt_content {
                        markdown.push_str(&format!("{}\n", content));
                    }
                    dt_content = Some(self.get_inner_markdown(&child, document, excluded));
                } else if tag == "dd" {
                    let dd_content = self.get_inner_markdown(&child, document, excluded);
                    if let Some(ref dt) = dt_content {
                        markdown.push_str(&format!("\n{}\n:   {}\n", dt, dd_content));
                        dt_content = None;
                    } else {
                        markdown.push_str(&format!("\n:   {}\n", dd_content));
                    }
                }
            }
        }

        if let Some(ref content) = dt_content {
            markdown.push_str(&format!("\n{}\n", content));
        }

        if markdown.is_empty() {
            String::new()
        } else {
            format!("\n{}\n\n", markdown.trim())
        }
    }

    /// Convert list (ul/ol) to Markdown.
    fn convert_list_to_markdown(&self, node: &NodeRef<Node>, document: &Html, excluded: &[ego_tree::NodeId]) -> String {
        let list_markdown = self.process_list(node, 0, document, excluded);
        let trimmed = list_markdown.trim();
        if trimmed.is_empty() {
            String::new()
        } else {
            format!("\n\n{}\n\n", trimmed)
        }
    }

    /// Recursively process a list element.
    fn process_list(
        &self,
        list_element: &NodeRef<Node>,
        level: usize,
        document: &Html,
        excluded: &[ego_tree::NodeId],
    ) -> String {
        let mut markdown = String::new();
        let is_ordered = matches!(list_element.value(), Node::Element(el)
            if el.name.local.as_ref() == "ol");

        let mut item_counter: usize = 1;
        if is_ordered
            && let Node::Element(el) = list_element.value()
            && let Some(start_val) = el.attr("start")
            && let Ok(start) = start_val.parse::<usize>()
            && start > 1
        {
            item_counter = start;
        }

        let indent = "    ".repeat(level);

        for child in list_element.children() {
            if excluded.contains(&child.id()) {
                continue;
            }
            if let Node::Element(el) = child.value()
                && el.name.local.as_ref() == "li"
            {
                let marker = if is_ordered {
                    let m = format!("{}.", item_counter);
                    item_counter += 1;
                    m
                } else {
                    self.bullet_list_marker.clone()
                };

                let (item_content, nested_list) = self.extract_li_data(&child, level, document, excluded);

                let trimmed_content = item_content.trim();
                let lines: Vec<&str> = trimmed_content.split('\n').filter(|s| !s.is_empty()).collect();

                let first_line = lines.first().copied().unwrap_or("");
                markdown.push_str(&format!("{}{} {}\n", indent, marker, first_line));

                // Add subsequent lines with proper indentation
                let subsequent_indent = format!("{}{}", indent, " ".repeat(marker.len() + 1));
                for line in lines.iter().skip(1) {
                    markdown.push_str(&format!("{}{}\n", subsequent_indent, line));
                }

                if !nested_list.is_empty() {
                    markdown.push_str(&nested_list);
                    markdown.push('\n');
                }
            }
        }

        markdown
    }

    /// Extract content and nested list markdown from a <li> element.
    fn extract_li_data(
        &self,
        li_element: &NodeRef<Node>,
        level: usize,
        document: &Html,
        excluded: &[ego_tree::NodeId],
    ) -> (String, String) {
        let mut item_content = String::new();
        let mut nested_list = String::new();

        for child in li_element.children() {
            if excluded.contains(&child.id()) {
                continue;
            }
            if let Node::Element(el) = child.value() {
                let tag = el.name.local.as_ref();
                if tag == "ul" || tag == "ol" {
                    nested_list.push('\n');
                    nested_list.push_str(&self.process_list(&child, level + 1, document, excluded));
                } else if tag == "p" {
                    item_content.push_str(self.get_inner_markdown(&child, document, excluded).trim());
                    item_content.push('\n');
                } else {
                    item_content.push_str(&self.convert_node(&child, document, excluded));
                }
            } else {
                item_content.push_str(&self.convert_node(&child, document, excluded));
            }
        }

        let cleaned_item = item_content.trim().to_string();
        let cleaned_nested = nested_list.trim().to_string();

        let final_nested = if !cleaned_nested.is_empty() && !cleaned_item.is_empty() {
            format!("\n{}", cleaned_nested)
        } else {
            cleaned_nested
        };

        (cleaned_item, final_nested)
    }

    /// Normalize whitespace in converted Markdown.
    fn normalize_whitespace(&self, text: &str) -> String {
        // Replace CRLF with LF
        let text = text.replace("\r\n", "\n");
        // Replace multiple consecutive newlines with max two
        let text = Regex::new(r"\n{3,}")
            .map(|re| re.replace_all(&text, "\n\n").to_string())
            .unwrap_or(text);
        // Trim trailing spaces/tabs from each line
        let text = Regex::new(r"[ \t]+$")
            .map(|re| {
                text.lines()
                    .map(|line| re.replace_all(line, "").to_string())
                    .collect::<Vec<_>>()
                    .join("\n")
            })
            .unwrap_or(text);

        text.trim().to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_simple_paragraph() {
        let converter = HtmlToMarkdownConverter::new("<p>Hello world</p>", vec![]);
        let md = converter.get_markdown();
        assert!(md.contains("Hello world"));
    }

    #[test]
    fn test_heading_atx() {
        let mut converter = HtmlToMarkdownConverter::new("<h1>Title</h1>", vec![]);
        converter.set_heading_style(HeadingStyle::Atx);
        let md = converter.get_markdown();
        assert!(md.contains("# Title"));
    }

    #[test]
    fn test_heading_setext() {
        let mut converter = HtmlToMarkdownConverter::new("<h1>Title</h1>", vec![]);
        converter.set_heading_style(HeadingStyle::Setext);
        let md = converter.get_markdown();
        assert!(md.contains("Title\n====="));
    }

    #[test]
    fn test_bold() {
        let converter = HtmlToMarkdownConverter::new("<strong>bold text</strong>", vec![]);
        let md = converter.get_markdown();
        assert!(md.contains("**bold text**"));
    }

    #[test]
    fn test_italic() {
        let converter = HtmlToMarkdownConverter::new("<em>italic text</em>", vec![]);
        let md = converter.get_markdown();
        assert!(md.contains("*italic text*"));
    }

    #[test]
    fn test_link() {
        let converter = HtmlToMarkdownConverter::new("<a href=\"https://example.com\">Example</a>", vec![]);
        let md = converter.get_markdown();
        assert!(md.contains("[Example](https://example.com)"));
    }

    #[test]
    fn test_image() {
        let converter = HtmlToMarkdownConverter::new("<img src=\"image.jpg\" alt=\"An image\">", vec![]);
        let md = converter.get_markdown();
        assert!(md.contains("![An image](image.jpg)"));
    }

    #[test]
    fn test_unordered_list() {
        let converter = HtmlToMarkdownConverter::new("<ul><li>Item 1</li><li>Item 2</li></ul>", vec![]);
        let md = converter.get_markdown();
        assert!(md.contains("- Item 1"));
        assert!(md.contains("- Item 2"));
    }

    #[test]
    fn test_ordered_list() {
        let converter = HtmlToMarkdownConverter::new("<ol><li>First</li><li>Second</li></ol>", vec![]);
        let md = converter.get_markdown();
        assert!(md.contains("1. First"));
        assert!(md.contains("2. Second"));
    }

    #[test]
    fn test_code_block() {
        let converter =
            HtmlToMarkdownConverter::new("<pre><code class=\"language-rust\">fn main() {}</code></pre>", vec![]);
        let md = converter.get_markdown();
        assert!(md.contains("```rust"));
        assert!(md.contains("fn main() {}"));
        assert!(md.contains("```"));
    }

    #[test]
    fn test_inline_code() {
        let converter = HtmlToMarkdownConverter::new("<code>foo</code>", vec![]);
        let md = converter.get_markdown();
        assert!(md.contains("`foo`"));
    }

    #[test]
    fn test_blockquote() {
        let converter = HtmlToMarkdownConverter::new("<blockquote>Quoted text</blockquote>", vec![]);
        let md = converter.get_markdown();
        assert!(md.contains("> Quoted text"));
    }

    #[test]
    fn test_horizontal_rule() {
        let converter = HtmlToMarkdownConverter::new("<hr>", vec![]);
        let md = converter.get_markdown();
        assert!(md.contains("* * *"));
    }

    #[test]
    fn test_table() {
        let converter = HtmlToMarkdownConverter::new(
            "<table><thead><tr><th>Name</th><th>Value</th></tr></thead>\
             <tbody><tr><td>A</td><td>1</td></tr></tbody></table>",
            vec![],
        );
        let md = converter.get_markdown();
        assert!(md.contains("| Name"));
        assert!(md.contains("| A"));
        assert!(md.contains("---"));
    }

    #[test]
    fn test_strikethrough() {
        let converter = HtmlToMarkdownConverter::new("<del>deleted text</del>", vec![]);
        let md = converter.get_markdown();
        assert!(md.contains("~~deleted text~~"));
    }

    #[test]
    fn test_excluded_selector() {
        let converter = HtmlToMarkdownConverter::new(
            "<div><p>Keep this</p><div class=\"hidden\">Remove this</div></div>",
            vec![],
        );
        let md = converter.get_markdown();
        assert!(md.contains("Keep this"));
        assert!(!md.contains("Remove this"));
    }

    #[test]
    fn test_script_removed() {
        let converter = HtmlToMarkdownConverter::new("<div><p>Content</p><script>alert('test')</script></div>", vec![]);
        let md = converter.get_markdown();
        assert!(md.contains("Content"));
        assert!(!md.contains("alert"));
    }

    // --- Tests for aria-hidden and role=menu exclusion ---

    #[test]
    fn test_aria_hidden_excluded() {
        let converter = HtmlToMarkdownConverter::new(
            "<div><p>Visible</p><div aria-hidden=\"true\"><p>Hidden mega-menu</p></div></div>",
            vec![],
        );
        let md = converter.get_markdown();
        assert!(md.contains("Visible"));
        assert!(!md.contains("Hidden mega-menu"));
    }

    #[test]
    fn test_aria_hidden_children_excluded() {
        let converter = HtmlToMarkdownConverter::new(
            "<div><p>Content</p><nav aria-hidden=\"true\"><ul><li><a href=\"/\">Home</a></li><li><a href=\"/about\">About</a></li></ul></nav></div>",
            vec![],
        );
        let md = converter.get_markdown();
        assert!(md.contains("Content"));
        assert!(!md.contains("Home"));
        assert!(!md.contains("About"));
    }

    #[test]
    fn test_role_menu_excluded() {
        let converter = HtmlToMarkdownConverter::new(
            "<div><p>Page content</p><ul role=\"menu\"><li>Menu Item 1</li><li>Menu Item 2</li></ul></div>",
            vec![],
        );
        let md = converter.get_markdown();
        assert!(md.contains("Page content"));
        assert!(!md.contains("Menu Item"));
    }

    // --- Tests for block element spacing ---

    #[test]
    fn test_adjacent_divs_have_spacing() {
        let converter = HtmlToMarkdownConverter::new("<div>text one</div><div>text two</div>", vec![]);
        let md = converter.get_markdown();
        assert!(
            !md.contains("text onetext two"),
            "Adjacent divs should not concatenate: {}",
            md
        );
        assert!(md.contains("text one"));
        assert!(md.contains("text two"));
    }

    #[test]
    fn test_adjacent_sections_have_spacing() {
        let converter = HtmlToMarkdownConverter::new(
            "<section><p>First</p></section><section><p>Second</p></section>",
            vec![],
        );
        let md = converter.get_markdown();
        assert!(
            !md.contains("FirstSecond"),
            "Adjacent sections should not concatenate: {}",
            md
        );
    }

    #[test]
    fn test_span_remains_inline() {
        let converter = HtmlToMarkdownConverter::new("<p>Hello <span>world</span> test</p>", vec![]);
        let md = converter.get_markdown();
        assert!(md.contains("Hello world test"));
    }

    #[test]
    fn test_nested_divs_no_excessive_whitespace() {
        let converter = HtmlToMarkdownConverter::new("<div><div><div>deep text</div></div></div>", vec![]);
        let md = converter.get_markdown();
        assert!(md.contains("deep text"));
        // Should not have more than two consecutive newlines after normalization
        assert!(
            !md.contains("\n\n\n"),
            "Nested divs should not produce excessive newlines"
        );
    }

    #[test]
    fn test_empty_div_produces_no_output() {
        let converter = HtmlToMarkdownConverter::new("<p>Before</p><div></div><p>After</p>", vec![]);
        let md = converter.get_markdown();
        assert!(md.contains("Before"));
        assert!(md.contains("After"));
    }

    // --- Tests for aria-label fallback on links ---

    #[test]
    fn test_link_aria_label_fallback() {
        let converter = HtmlToMarkdownConverter::new(
            r#"<a href="https://facebook.com/page" aria-label="Facebook"><svg><path d="M0 0"/></svg></a>"#,
            vec![],
        );
        let md = converter.get_markdown();
        assert!(
            md.contains("[Facebook](https://facebook.com/page)"),
            "Should use aria-label: {}",
            md
        );
    }

    #[test]
    fn test_link_visible_text_preferred_over_aria_label() {
        let converter = HtmlToMarkdownConverter::new(
            r#"<a href="https://example.com" aria-label="Aria Label">Visible Text</a>"#,
            vec![],
        );
        let md = converter.get_markdown();
        assert!(md.contains("[Visible Text](https://example.com)"));
        assert!(!md.contains("Aria Label"));
    }

    #[test]
    fn test_link_url_fallback_without_aria_label() {
        let converter = HtmlToMarkdownConverter::new(r#"<a href="https://example.com"><svg></svg></a>"#, vec![]);
        let md = converter.get_markdown();
        assert!(
            md.contains("[https://example.com](https://example.com)"),
            "Should fall back to URL: {}",
            md
        );
    }

    #[test]
    fn test_link_empty_aria_label_falls_back_to_url() {
        let converter = HtmlToMarkdownConverter::new(
            r#"<a href="https://example.com" aria-label="  "><svg></svg></a>"#,
            vec![],
        );
        let md = converter.get_markdown();
        assert!(
            md.contains("[https://example.com](https://example.com)"),
            "Empty aria-label should fall back to URL: {}",
            md
        );
    }

    // --- Tests for cookie banner exclusion ---

    #[test]
    fn test_cookie_banner_excluded() {
        let converter = HtmlToMarkdownConverter::new(
            "<div><p>Content</p><div class=\"cookie-banner\"><p>We use cookies</p><button>Accept</button></div></div>",
            vec![],
        );
        let md = converter.get_markdown();
        assert!(md.contains("Content"));
        assert!(!md.contains("cookies"));
    }

    #[test]
    fn test_onetrust_banner_excluded() {
        let converter = HtmlToMarkdownConverter::new(
            "<div><p>Content</p><div id=\"onetrust-banner-sdk\"><p>Cookie preferences</p></div></div>",
            vec![],
        );
        let md = converter.get_markdown();
        assert!(md.contains("Content"));
        assert!(!md.contains("Cookie preferences"));
    }
}
