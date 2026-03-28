// SiteOne Crawler - MarkdownExporter
// (c) Jan Reges <jan.reges@siteone.cz>
//
// Converts crawled HTML pages to Markdown format.

use std::collections::HashMap;
use std::fs;
use std::path::Path;
use std::sync::{Arc, Mutex};
use std::time::Instant;

use regex::Regex;

use crate::content_processor::manager::ContentProcessorManager;
use crate::engine::parsed_url::ParsedUrl;
use crate::error::{CrawlerError, CrawlerResult};
use crate::export::exporter::Exporter;
use crate::export::utils::html_to_markdown::HtmlToMarkdownConverter;
use crate::export::utils::markdown_site_aggregator::MarkdownSiteAggregator;
use crate::export::utils::offline_url_converter::OfflineUrlConverter;
use crate::export::utils::target_domain_relation::TargetDomainRelation;
use crate::output::output::Output;
use crate::result::status::Status;
use crate::result::visited_url::{SOURCE_A_HREF, SOURCE_IMG_SRC, VisitedUrl};
use crate::types::ContentTypeId;
use crate::utils;

/// Content types that require URL rewriting
const CONTENT_TYPES_REQUIRING_CHANGES: &[ContentTypeId] = &[ContentTypeId::Html, ContentTypeId::Redirect];

/// Exports crawled HTML pages as Markdown files.
/// Supports single-file combination, selector exclusion, content replacement.
pub struct MarkdownExporter {
    markdown_export_directory: Option<String>,
    markdown_export_single_file: Option<String>,
    markdown_disable_images: bool,
    markdown_disable_files: bool,
    markdown_remove_links_and_images_from_single_file: bool,
    markdown_exclude_selector: Vec<String>,
    markdown_export_store_only_url_regex: Vec<String>,
    markdown_ignore_store_file_error: bool,
    markdown_replace_content: Vec<String>,
    markdown_replace_query_string: Vec<String>,
    markdown_move_content_before_h1_to_end: bool,
    initial_parsed_url: Option<ParsedUrl>,
    ignore_regexes: Vec<String>,
    initial_url: String,
    content_processor_manager: Option<Arc<Mutex<ContentProcessorManager>>>,
    /// Maps URL -> relative file path for successfully exported files
    exported_file_paths: HashMap<String, String>,
}

impl Default for MarkdownExporter {
    fn default() -> Self {
        Self::new()
    }
}

impl MarkdownExporter {
    pub fn new() -> Self {
        Self {
            markdown_export_directory: None,
            markdown_export_single_file: None,
            markdown_disable_images: false,
            markdown_disable_files: false,
            markdown_remove_links_and_images_from_single_file: false,
            markdown_exclude_selector: Vec::new(),
            markdown_export_store_only_url_regex: Vec::new(),
            markdown_ignore_store_file_error: false,
            markdown_replace_content: Vec::new(),
            markdown_replace_query_string: Vec::new(),
            markdown_move_content_before_h1_to_end: false,
            initial_parsed_url: None,
            ignore_regexes: Vec::new(),
            initial_url: String::new(),
            content_processor_manager: None,
            exported_file_paths: HashMap::new(),
        }
    }

    pub fn set_markdown_export_directory(&mut self, dir: Option<String>) {
        self.markdown_export_directory = dir.map(|d| d.trim_end_matches('/').to_string());
    }

    pub fn set_markdown_export_single_file(&mut self, file: Option<String>) {
        self.markdown_export_single_file = file;
    }

    pub fn set_markdown_disable_images(&mut self, disable: bool) {
        self.markdown_disable_images = disable;
    }

    pub fn set_markdown_disable_files(&mut self, disable: bool) {
        self.markdown_disable_files = disable;
    }

    pub fn set_markdown_remove_links_and_images_from_single_file(&mut self, remove: bool) {
        self.markdown_remove_links_and_images_from_single_file = remove;
    }

    pub fn set_markdown_exclude_selector(&mut self, selectors: Vec<String>) {
        self.markdown_exclude_selector = selectors;
    }

    pub fn set_markdown_export_store_only_url_regex(&mut self, regexes: Vec<String>) {
        self.markdown_export_store_only_url_regex = regexes;
    }

    pub fn set_markdown_ignore_store_file_error(&mut self, ignore: bool) {
        self.markdown_ignore_store_file_error = ignore;
    }

    pub fn set_markdown_replace_content(&mut self, replacements: Vec<String>) {
        self.markdown_replace_content = replacements;
    }

    pub fn set_markdown_replace_query_string(&mut self, replacements: Vec<String>) {
        self.markdown_replace_query_string = replacements;
    }

    pub fn set_markdown_move_content_before_h1_to_end(&mut self, move_content: bool) {
        self.markdown_move_content_before_h1_to_end = move_content;
    }

    pub fn set_initial_parsed_url(&mut self, url: ParsedUrl) {
        self.initial_parsed_url = Some(url);
    }

    pub fn set_ignore_regexes(&mut self, regexes: Vec<String>) {
        self.ignore_regexes = regexes;
    }

    pub fn set_initial_url(&mut self, url: String) {
        self.initial_url = url;
    }

    pub fn set_content_processor_manager(&mut self, cpm: Arc<Mutex<ContentProcessorManager>>) {
        self.content_processor_manager = Some(cpm);
    }

    /// Get the mapping of URL -> relative file path for all successfully exported files.
    pub fn get_exported_file_paths(&self) -> &HashMap<String, String> {
        &self.exported_file_paths
    }

    /// Store a file to the markdown export directory.
    fn store_file(&mut self, visited_url: &VisitedUrl, status: &Status) -> CrawlerResult<()> {
        let export_dir = self
            .markdown_export_directory
            .as_ref()
            .ok_or_else(|| CrawlerError::Export("Markdown export directory not set".to_string()))?;

        let body_bytes = status.get_url_body(&visited_url.uq_id).unwrap_or_default();

        // For content types requiring URL rewriting (HTML, CSS, JS), work with text.
        // For binary content (images, fonts), keep raw bytes to avoid UTF-8 corruption.
        let final_bytes =
            if !body_bytes.is_empty() && CONTENT_TYPES_REQUIRING_CHANGES.contains(&visited_url.content_type) {
                let mut content = String::from_utf8_lossy(&body_bytes).into_owned();

                // Apply content changes for offline version through content processors
                if let Some(ref cpm) = self.content_processor_manager {
                    let parsed_url = ParsedUrl::parse(&visited_url.url, None);
                    if let Ok(mut manager) = cpm.lock() {
                        manager.apply_content_changes_for_offline_version(
                            &mut content,
                            visited_url.content_type,
                            &parsed_url,
                            true,
                        );
                    }
                }

                // Apply custom content replacements
                if !content.is_empty() && !self.markdown_replace_content.is_empty() {
                    for replace in &self.markdown_replace_content {
                        let parts: Vec<&str> = replace.splitn(2, "->").collect();
                        let replace_from = parts[0].trim();
                        let replace_to = if parts.len() > 1 { parts[1].trim() } else { "" };

                        let is_regex = crate::utils::is_regex_pattern(replace_from);

                        if is_regex {
                            if let Some(pattern) = extract_regex_pattern(replace_from)
                                && let Ok(re) = Regex::new(&pattern)
                            {
                                content = re.replace_all(&content, replace_to).to_string();
                            }
                        } else {
                            content = content.replace(replace_from, replace_to);
                        }
                    }
                }

                content.into_bytes()
            } else {
                body_bytes
            };

        // Build store file path
        let relative_path = self.get_relative_file_path_for_file_by_url(visited_url, status);
        let sanitized_path = OfflineUrlConverter::sanitize_file_path(&relative_path, false);
        // Path traversal protection: strip "../" sequences from sanitized path
        let sanitized_path = sanitized_path.replace("../", "").replace("..\\", "");
        let store_file_path = format!("{}/{}", export_dir, sanitized_path);

        // Create directory structure
        let dir_path = Path::new(&store_file_path).parent().ok_or_else(|| {
            CrawlerError::Export(format!("Cannot determine parent directory for '{}'", store_file_path))
        })?;

        if !dir_path.exists() {
            fs::create_dir_all(dir_path).map_err(|e| {
                CrawlerError::Export(format!("Cannot create directory '{}': {}", dir_path.display(), e))
            })?;
        }

        // Check if we should overwrite
        if Path::new(&store_file_path).exists()
            && let Some(ref initial_url) = self.initial_parsed_url
            && !visited_url.is_https()
            && initial_url.is_https()
        {
            let message = format!(
                "File '{}' already exists and will not be overwritten because initial request was HTTPS and this request is HTTP: {}",
                store_file_path, visited_url.url
            );
            status.add_notice_to_summary("markdown-exporter-store-file-ignored", &message);
            return Ok(());
        }

        // Write the content (raw bytes to preserve binary data for images/fonts)
        if let Err(e) = fs::write(&store_file_path, &final_bytes) {
            let has_extension = Regex::new(r"(?i)\.[a-z0-9\-]{1,15}$")
                .map(|re| re.is_match(&store_file_path))
                .unwrap_or(false);

            if has_extension && !self.markdown_ignore_store_file_error {
                return Err(CrawlerError::Export(format!(
                    "Cannot store file '{}': {}",
                    store_file_path, e
                )));
            } else {
                let message = format!(
                    "Cannot store file '{}' (undefined extension). Original URL: {}",
                    store_file_path, visited_url.url
                );
                status.add_notice_to_summary("markdown-exporter-store-file-error", &message);
                return Ok(());
            }
        }

        // Convert HTML to Markdown
        if store_file_path.ends_with(".html") {
            let md_file_path = format!("{}md", &store_file_path[..store_file_path.len() - 4]);

            let html_content = fs::read_to_string(&store_file_path).unwrap_or_default();
            let converter = HtmlToMarkdownConverter::new(&html_content, self.markdown_exclude_selector.clone());
            let markdown = converter.get_markdown();

            if let Err(_e) = fs::write(&md_file_path, &markdown) {
                let message = format!(
                    "Cannot convert HTML file to Markdown file '{}'. Original URL: {}",
                    md_file_path, visited_url.url
                );
                status.add_notice_to_summary("markdown-exporter-store-file-error", &message);
                return Ok(());
            }

            // Remove the HTML file
            let _ = fs::remove_file(&store_file_path);

            if !Path::new(&md_file_path).exists() {
                let message = format!(
                    "Cannot convert HTML file to Markdown file '{}'. Original URL: {}",
                    md_file_path, visited_url.url
                );
                status.add_notice_to_summary("markdown-exporter-store-file-error", &message);
                return Ok(());
            }

            // Normalize the markdown file
            self.normalize_markdown_file(&md_file_path);
        }

        // Record the mapping — for HTML files, use the .md path
        let final_relative_path = if sanitized_path.ends_with(".html") {
            format!("{}md", &sanitized_path[..sanitized_path.len() - 4])
        } else {
            sanitized_path.clone()
        };
        self.exported_file_paths
            .insert(visited_url.url.clone(), final_relative_path);

        Ok(())
    }

    /// Normalize a markdown file after conversion from HTML.
    fn normalize_markdown_file(&self, md_file_path: &str) {
        let mut md_content = match fs::read_to_string(md_file_path) {
            Ok(content) => content,
            Err(_) => return,
        };

        // Replace .html with .md in links
        if let Ok(link_re) = Regex::new(r"\[([^\]]*)\]\(([^)]+)\)") {
            let ignore_regexes = &self.ignore_regexes;
            md_content = link_re
                .replace_all(&md_content, |caps: &regex::Captures| {
                    let link_text = caps.get(1).map_or("", |m| m.as_str());
                    let url = caps.get(2).map_or("", |m| m.as_str());

                    // Check if URL matches any ignore pattern
                    for ignore_regex in ignore_regexes {
                        if let Ok(re) = Regex::new(ignore_regex)
                            && re.is_match(url)
                        {
                            return format!("[{}]({})", link_text, url);
                        }
                    }

                    // Replace .html with .md
                    let new_url = url.replace(".html", ".md").replace(".html#", ".md#");
                    format!("[{}]({})", link_text, new_url)
                })
                .to_string();
        }

        // Disable images if configured
        if self.markdown_disable_images {
            // Replace image in anchor text
            if let Ok(re) = Regex::new(r"\[!\[[^\]]*\]\([^\)]*\)\]\([^\)]*\)") {
                md_content = re.replace_all(&md_content, "").to_string();
            }
            // Replace standard images
            if let Ok(re) = Regex::new(r"!\[.*?\]\(.*?\)") {
                md_content = re.replace_all(&md_content, "").to_string();
            }
            // Normalize leading whitespace inside link text: [ text](url) → [text](url)
            if let Ok(re) = Regex::new(r"\[\s+([^\]]+)\]\(") {
                md_content = re.replace_all(&md_content, "[$1](").to_string();
            }
        }

        // Disable files if configured
        if self.markdown_disable_files
            && let Ok(re) = Regex::new(r"(?i)\[([^\]]+)\]\(([^)]+)\)")
        {
            let ignore_regexes = self.ignore_regexes.clone();
            md_content = re
                .replace_all(&md_content, |caps: &regex::Captures| {
                    let url = caps.get(2).map_or("", |m| m.as_str());

                    // Skip http(s), tel:, mailto: and other protocol URLs
                    if url.starts_with("http://") || url.starts_with("https://")
                        || url.starts_with("tel:") || url.starts_with("mailto:")
                    {
                        return caps[0].to_string();
                    }

                    let full_url = url.to_string();
                    let ext = url.rsplit('.').next().unwrap_or("").to_lowercase();

                    // Check ignore patterns
                    for ignore_regex in &ignore_regexes {
                        if let Ok(re) = Regex::new(ignore_regex)
                            && re.is_match(&full_url)
                        {
                            return caps[0].to_string();
                        }
                    }

                    // Keep page links and images (disable-files targets downloadable documents)
                    if ["md", "html", "htm", "jpg", "png", "gif", "webp", "avif"].contains(&ext.as_str()) {
                        return caps[0].to_string();
                    }

                    String::new()
                })
                .to_string();

            md_content = md_content.replace("  ", " ");
        }

        // Remove empty links
        if let Ok(re) = Regex::new(r"\[[^\]]*\]\(\)") {
            md_content = re.replace_all(&md_content, "").to_string();
        }

        // Remove empty list items (e.g. after disabling images and files)
        if let Ok(re) = Regex::new(r"(?m)^\s*[-*+]\s*$\n?") {
            md_content = re.replace_all(&md_content, "").to_string();
        }

        // Remove links where text is a bare filename (fallback from removed media like <video>)
        // e.g. [some-page.html](some-page.md) — real link text never looks like a raw filename
        if let Ok(re) = Regex::new(r"(?m)^\[([^\]\s]+\.html?)\]\([^\)]+\)\s*$\n?") {
            md_content = re.replace_all(&md_content, "").to_string();
        }

        // Remove empty lines in code blocks
        md_content = md_content.replace("\\\n\n  -", "\\\n  -");

        // Remove empty lines at beginning of code blocks
        if let Ok(re) = Regex::new(r"```\n{2,}") {
            md_content = re.replace_all(&md_content, "```\n").to_string();
        }

        // Apply additional fixes
        md_content = self.remove_empty_lines_in_lists(&md_content);
        md_content = self.move_content_before_main_heading_to_end(&md_content);
        md_content = self.fix_multiline_images(&md_content);
        md_content = self.detect_and_set_code_language(&md_content);

        // Add backticks around --param inside tables
        if let Ok(re) = Regex::new(r"(?i)\| -{1,2}([a-z0-9][a-z0-9-]*) \|") {
            md_content = re.replace_all(&md_content, "| `--$1` |").to_string();
        }

        // Remove 3+ empty lines to 2 empty lines
        if let Ok(re) = Regex::new(r"\n{3,}") {
            md_content = re.replace_all(&md_content, "\n\n").to_string();
        }

        // Trim special chars (only whitespace from start, all special chars from end
        // to preserve markdown-significant characters like # headings and - lists at the start)
        md_content = md_content
            .trim_start_matches(|c: char| c == '\n' || c == '\t' || c == ' ')
            .trim_end_matches(|c: char| c == '\n' || c == '\t' || c == ' ' || c == '-' || c == '#' || c == '*')
            .to_string();

        // Fix excessive whitespace
        md_content = self.remove_excessive_whitespace(&md_content);

        // Collapse large link lists into accordions (must run after all list normalization)
        md_content = HtmlToMarkdownConverter::collapse_large_link_lists(&md_content);

        let _ = fs::write(md_file_path, &md_content);
    }

    /// Remove excessive whitespace from markdown content.
    fn remove_excessive_whitespace(&self, md: &str) -> String {
        let lines: Vec<&str> = md.split('\n').collect();
        let mut result: Vec<String> = Vec::new();
        let mut in_code_block = false;
        let mut last_line_was_empty = false;

        let code_block_re = Regex::new(r"^```").ok();
        let list_item_re = Regex::new(r"^(\s*)([-*+]|\d+\.)\s").ok();
        let table_row_re = Regex::new(r"^\s*\|.*\|\s*$").ok();
        let heading_re = Regex::new(r"^#+\s+").ok();
        let whitespace_re = Regex::new(r"\s+").ok();

        for line in &lines {
            if code_block_re.as_ref().map(|re| re.is_match(line)).unwrap_or(false) {
                in_code_block = !in_code_block;
                result.push(line.to_string());
                last_line_was_empty = false;
                continue;
            }

            if in_code_block {
                result.push(line.to_string());
                last_line_was_empty = false;
                continue;
            }

            let is_list_item = list_item_re.as_ref().map(|re| re.is_match(line)).unwrap_or(false);
            let is_table_row = table_row_re.as_ref().map(|re| re.is_match(line)).unwrap_or(false);
            let is_heading = heading_re.as_ref().map(|re| re.is_match(line)).unwrap_or(false);

            if line.trim().is_empty() {
                if !last_line_was_empty {
                    result.push(String::new());
                    last_line_was_empty = true;
                }
                continue;
            }

            if is_list_item || is_table_row || is_heading {
                result.push(line.to_string());
            } else {
                let trimmed = whitespace_re
                    .as_ref()
                    .map(|re| re.replace_all(line.trim(), " ").to_string())
                    .unwrap_or_else(|| line.trim().to_string());
                if !trimmed.is_empty() {
                    result.push(trimmed);
                }
            }
            last_line_was_empty = false;
        }

        let mut content = result.join("\n");

        // Remove spaces at the end of lines
        if let Ok(re) = Regex::new(r"(?m)[ \t]+$") {
            content = re.replace_all(&content, "").to_string();
        }

        content
    }

    /// Remove empty lines between list items.
    fn remove_empty_lines_in_lists(&self, md: &str) -> String {
        let lines: Vec<&str> = md.split('\n').collect();
        let mut result: Vec<String> = Vec::new();
        let mut in_list = false;
        let mut last_line_empty = false;
        let mut last_indent_level: i32 = 0;

        let list_re = Regex::new(r"^[ ]{0,3}[-*+][ ]|^[ ]{0,3}\d+\.[ ]|^[ ]{2,}[-*+][ ]").ok();

        for line in &lines {
            let trimmed_line = line.trim();
            let is_empty = trimmed_line.is_empty();

            let is_list_item = list_re.as_ref().map(|re| re.is_match(line)).unwrap_or(false);

            if is_list_item {
                in_list = true;

                if last_line_empty {
                    let leading_spaces: i32 = line.len() as i32 - line.trim_start().len() as i32;

                    if (leading_spaces - last_indent_level).abs() > 2 {
                        // Different nesting level, keep empty line
                    } else {
                        // Same nesting level, remove empty line
                        result.pop();
                    }
                }

                result.push(line.to_string());
                last_line_empty = false;
                let leading_spaces = line.len() as i32 - line.trim_start().len() as i32;
                last_indent_level = leading_spaces;
            } else if is_empty {
                result.push(line.to_string());
                last_line_empty = true;
            } else {
                let leading_spaces = line.len() as i32 - line.trim_start().len() as i32;
                if in_list && leading_spaces < last_indent_level {
                    in_list = false;
                }
                result.push(line.to_string());
                last_line_empty = false;
                last_indent_level = leading_spaces;
            }
        }

        result.join("\n")
    }

    /// Move content before the main heading to the end.
    fn move_content_before_main_heading_to_end(&self, md: &str) -> String {
        if !self.markdown_move_content_before_h1_to_end {
            return md.to_string();
        }

        let mut headings: Vec<(usize, usize)> = Vec::new(); // (offset, level)

        // ATX headings
        if let Ok(re) = Regex::new(r"(?m)^(#{1,6})\s.*$") {
            for mat in re.find_iter(md) {
                let level = mat.as_str().chars().take_while(|c| *c == '#').count();
                headings.push((mat.start(), level));
            }
        }

        // Setext headings
        if let Ok(re) = Regex::new(r"(?m)^(.+?)\n(=+|-+)\s*$") {
            for caps in re.captures_iter(md) {
                if let (Some(text_match), Some(underline_match)) = (caps.get(1), caps.get(2)) {
                    if text_match.as_str().trim().is_empty() {
                        continue;
                    }
                    let underline = underline_match.as_str();
                    let level = if underline.starts_with('=') { 1 } else { 2 };
                    headings.push((text_match.start(), level));
                }
            }
        }

        if headings.is_empty() {
            return md.to_string();
        }

        // Find the highest level (lowest number)
        let min_level = headings.iter().map(|(_, level)| *level).min().unwrap_or(6);

        // Find first heading with that level
        let main_heading = headings
            .iter()
            .filter(|(_, level)| *level == min_level)
            .min_by_key(|(offset, _)| *offset);

        if let Some((heading_pos, _)) = main_heading {
            let content_before = &md[..*heading_pos];
            let content_after = &md[*heading_pos..];

            if content_before.trim().is_empty() {
                return md.to_string();
            }

            format!("{}\n\n---\n\n{}", content_after.trim(), content_before.trim())
        } else {
            md.to_string()
        }
    }

    /// Fix multi-line images and links.
    fn fix_multiline_images(&self, md: &str) -> String {
        md.replace("[\n![", "[![").replace(")\n](", ")](")
    }

    /// Detect and set code language for unlabeled code blocks.
    fn detect_and_set_code_language(&self, md: &str) -> String {
        let code_block_re = match Regex::new(r"(?s)```\s*\n((?:[^`]|`[^`]|``[^`])*?)\n```") {
            Ok(re) => re,
            Err(_) => return md.to_string(),
        };

        code_block_re
            .replace_all(md, |caps: &regex::Captures| {
                let code = caps.get(1).map_or("", |m| m.as_str());
                let detected = self.detect_language(code);
                format!("```{}\n{}\n```", detected, code)
            })
            .to_string()
    }

    /// Detect programming language from code content.
    fn detect_language(&self, code: &str) -> String {
        let patterns: Vec<(&str, Vec<&str>)> = vec![
            (
                "php",
                vec![
                    r"^<\?php",
                    r"\$[a-zA-Z_]",
                    r"\b(?:public|private|protected)\s+function\b",
                    r"\bnamespace\s+[a-zA-Z\\]+;",
                ],
            ),
            (
                "javascript",
                vec![
                    r"\bconst\s+[a-zA-Z_][a-zA-Z0-9_]*\s*=",
                    r"\bfunction\s*\([^)]*\)\s*\{",
                    r"\blet\s+[a-zA-Z_][a-zA-Z0-9_]*\s*=",
                    r"\bconsole\.log\(",
                    r"=>\s*\{",
                ],
            ),
            (
                "jsx",
                vec![
                    r"return\s+\(",
                    r"import\s+[a-zA-Z0-9_,\{\} ]+\s+from",
                    r"export\s+(default|const)",
                ],
            ),
            (
                "typescript",
                vec![
                    r":\s*(?:string|number|boolean|any)\b",
                    r"interface\s+[A-Z][a-zA-Z0-9_]*\s*\{",
                    r"type\s+[A-Z][a-zA-Z0-9_]*\s*=",
                ],
            ),
            (
                "python",
                vec![
                    r"(?m)def\s+[a-zA-Z_][a-zA-Z0-9_]*\s*\([^)]*\):\s*$",
                    r"(?m)^from\s+[a-zA-Z_.]+\s+import\b",
                    r"(?m)^if\s+__name__\s*==\s*['\x22]__main__['\x22]:\s*$",
                ],
            ),
            (
                "java",
                vec![
                    r"public\s+class\s+[A-Z][a-zA-Z0-9_]*",
                    r"System\.out\.println\(",
                    r"private\s+final\s+",
                ],
            ),
            (
                "rust",
                vec![
                    r"fn\s+[a-z_][a-z0-9_]*\s*\([^)]*\)\s*(?:->\s*[a-zA-Z<>]+\s*)?\{",
                    r"let\s+mut\s+",
                    r"impl\s+[A-Z][a-zA-Z0-9_]*",
                ],
            ),
            (
                "ruby",
                vec![
                    r"(?m)^require\s+['\x22][a-zA-Z0-9_/]+['\x22]",
                    r"def\s+[a-z_][a-z0-9_]*\b",
                    r"\battr_accessor\b",
                ],
            ),
            (
                "css",
                vec![
                    r"(?m)^[.#][a-zA-Z\-_][^\{]*\{",
                    r"\b(?:margin|padding|border|color|background):\s*[^;]+;",
                    r"@media\s+",
                ],
            ),
            (
                "bash",
                vec![
                    r"^#!/bin/(?:bash|sh)",
                    r"\$\([^)]+\)",
                    r"(?:^|\s)(?:-{1,2}[a-zA-Z0-9]+)",
                    r"\becho\s+",
                    r"\|\s*grep\b",
                ],
            ),
            (
                "go",
                vec![
                    r"\bfunc\s+[a-zA-Z_][a-zA-Z0-9_]*\s*\([^)]*\)",
                    r"\btype\s+[A-Z][a-zA-Z0-9_]*\s+struct\b",
                    r"\bpackage\s+[a-z][a-z0-9_]*\b",
                    r"\bif\s+err\s*!=\s*nil\b",
                ],
            ),
            (
                "csharp",
                vec![
                    r"\bnamespace\s+[A-Za-z.]+\b",
                    r"\bpublic\s+(?:class|interface|enum)\b",
                    r"\busing\s+[A-Za-z.]+;",
                    r"\basync\s+Task<",
                ],
            ),
            (
                "kotlin",
                vec![
                    r"\bfun\s+[a-zA-Z_][a-zA-Z0-9_]*\s*\(",
                    r"\bval\s+[a-zA-Z_][a-zA-Z0-9_]*:",
                    r"\bvar\s+[a-zA-Z_][a-zA-Z0-9_]*:",
                    r"\bdata\s+class\b",
                ],
            ),
            (
                "swift",
                vec![
                    r"\bfunc\s+[a-zA-Z_][a-zA-Z0-9_]*\s*\(",
                    r"\bvar\s+[a-zA-Z_][a-zA-Z0-9_]*:\s*[A-Z]",
                    r"\blet\s+[a-zA-Z_][a-zA-Z0-9_]*:",
                    r"\bclass\s+[A-Z][A-Za-z0-9_]*:",
                ],
            ),
            (
                "cpp",
                vec![
                    r"\b(?:class|struct)\s+[A-Z][a-zA-Z0-9_]*\b",
                    r"\bstd::[a-z0-9_]+",
                    r"\b#include\s+[<\x22][a-z0-9_.]+[>\x22]",
                    r"\btemplate\s*<[^>]+>",
                ],
            ),
            (
                "scala",
                vec![
                    r"\bdef\s+[a-z][a-zA-Z0-9_]*\s*\(",
                    r"\bcase\s+class\b",
                    r"\bobject\s+[A-Z][a-zA-Z0-9_]*\b",
                    r"\bval\s+[a-z][a-zA-Z0-9_]*\s*=",
                ],
            ),
            (
                "perl",
                vec![
                    r"\buse\s+[A-Z][A-Za-z:]+;",
                    r"\bsub\s+[a-z_][a-z0-9_]*\s*\{",
                    r"@[a-zA-Z_][a-zA-Z0-9_]*",
                ],
            ),
            (
                "lua",
                vec![
                    r"\bfunction\s+[a-z_][a-z0-9_]*\s*\(",
                    r"\blocal\s+[a-z_][a-z0-9_]*\s*=",
                    r"\brequire\s*\(?['\x22][^'\x22]+['\x22]\)?",
                ],
            ),
            (
                "vb",
                vec![
                    r"\bPublic\s+(?:Class|Interface|Module)\b",
                    r"\bPrivate\s+Sub\s+[A-Za-z_][A-Za-z0-9_]*\(",
                    r"\bDim\s+[A-Za-z_][A-Za-z0-9_]*\s+As\b",
                    r"\bEnd\s+(?:Sub|Function|Class|If|While)\b",
                ],
            ),
            (
                "fsharp",
                vec![
                    r"\blet\s+[a-z_][a-zA-Z0-9_]*\s*=",
                    r"\bmodule\s+[A-Z][A-Za-z0-9_]*\s*=",
                    r"\btype\s+[A-Z][A-Za-z0-9_]*\s*=",
                    r"\bmatch\s+.*\bwith\b",
                ],
            ),
            (
                "powershell",
                vec![
                    r"\$[A-Za-z_][A-Za-z0-9_]*",
                    r"\[Parameter\(.*?\)\]",
                    r"\bfunction\s+[A-Z][A-Za-z0-9-]*",
                    r"\b(?:Get|Set|New|Remove)-[A-Z][A-Za-z]*",
                ],
            ),
            (
                "xaml",
                vec![
                    r"<Window\s+[^>]*>",
                    r"<UserControl\s+[^>]*>",
                    r"xmlns:(?:x|d)=\x22[^\x22]+\x22",
                    r"<(?:Grid|StackPanel|DockPanel)[^>]*>",
                ],
            ),
            (
                "razor",
                vec![
                    r"@(?:model|using|inject)",
                    r"@Html\.[A-Za-z]+\(",
                    r"@\{.*?\}",
                    r#"<partial\s+name=\x22[^\x22]+\x22\s*/>"#,
                ],
            ),
            (
                "html",
                vec![r"<(html|head|body|h1|a|img|table|tr|td|ul|ol|li|script|style)[^>]*>"],
            ),
        ];

        let mut best_lang = String::new();
        let mut best_score = 0usize;

        for (lang, lang_patterns) in &patterns {
            let mut score = 0usize;
            for pattern in lang_patterns {
                if let Ok(re) = Regex::new(pattern) {
                    score += re.find_iter(code).count();
                }
            }
            if score > best_score {
                best_score = score;
                best_lang = lang.to_string();
            }
        }

        best_lang
    }

    /// Check if URL should be stored based on filters.
    fn should_be_url_stored(&self, visited_url: &VisitedUrl) -> bool {
        let mut result = false;

        if !self.markdown_export_store_only_url_regex.is_empty() {
            for regex_str in &self.markdown_export_store_only_url_regex {
                let pattern = crate::utils::extract_pcre_regex_pattern(regex_str);
                if let Ok(re) = Regex::new(&pattern)
                    && re.is_match(&visited_url.url)
                {
                    result = true;
                    break;
                }
            }
        } else {
            result = true;
        }

        // Do not store robots.txt
        if visited_url.url.ends_with("robots.txt") {
            result = false;
        }

        result
    }

    /// Get relative file path for storing a visited URL.
    fn get_relative_file_path_for_file_by_url(&self, visited_url: &VisitedUrl, status: &Status) -> String {
        let initial_url = self
            .initial_parsed_url
            .clone()
            .unwrap_or_else(|| ParsedUrl::parse(&visited_url.url, None));

        let source_url = if !visited_url.source_uq_id.is_empty() {
            status
                .get_url_by_uq_id(&visited_url.source_uq_id)
                .unwrap_or_else(|| visited_url.url.clone())
        } else {
            visited_url.url.clone()
        };

        let base_url = ParsedUrl::parse(&source_url, None);
        let target_url = ParsedUrl::parse(&visited_url.url, None);

        let attribute = if visited_url.content_type == ContentTypeId::Image {
            "src"
        } else {
            "href"
        };

        let mut converter = OfflineUrlConverter::new(initial_url, base_url, target_url, None, None, Some(attribute));

        let relative_url = converter.convert_url_to_relative(false);
        let relative_target_url = converter.get_relative_target_url();
        let target_domain_relation = converter.get_target_domain_relation();

        match target_domain_relation {
            TargetDomainRelation::InitialDifferentBaseSame | TargetDomainRelation::InitialDifferentBaseDifferent => {
                let relative_path = relative_url
                    .replace("../", "")
                    .trim_start_matches(['/', ' '])
                    .to_string();
                let host = relative_target_url.host.as_deref().unwrap_or("");
                if !relative_path.starts_with(&format!("_{}", host)) {
                    format!("_{}/{}", host, relative_path)
                } else {
                    relative_path
                }
            }
            TargetDomainRelation::InitialSameBaseSame | TargetDomainRelation::InitialSameBaseDifferent => relative_url
                .replace("../", "")
                .trim_start_matches(['/', ' '])
                .to_string(),
        }
    }

    /// Validate URL.
    fn is_valid_url(url: &str) -> bool {
        url::Url::parse(url).is_ok()
    }
}

impl Exporter for MarkdownExporter {
    fn get_name(&self) -> &str {
        "MarkdownExporter"
    }

    fn should_be_activated(&self) -> bool {
        self.markdown_export_directory.is_some() || self.markdown_export_single_file.is_some()
    }

    fn export(&mut self, status: &Status, _output: &dyn Output) -> CrawlerResult<()> {
        let start_time = Instant::now();

        // Set replace_query_string configuration
        OfflineUrlConverter::set_replace_query_string(self.markdown_replace_query_string.clone());

        // Determine valid content types
        let mut valid_content_types = vec![ContentTypeId::Html, ContentTypeId::Redirect];
        if !self.markdown_disable_images {
            valid_content_types.push(ContentTypeId::Image);
        }
        if !self.markdown_disable_files {
            valid_content_types.push(ContentTypeId::Document);
        }

        let visited_urls = status.get_visited_urls();

        // Filter relevant URLs
        let exported_urls: Vec<&VisitedUrl> = visited_urls
            .iter()
            .filter(|u| {
                // Do not store images from non-img-src sources
                if u.is_image() && !matches!(u.source_attr, SOURCE_IMG_SRC | SOURCE_A_HREF) {
                    return false;
                }

                u.status_code == 200 && valid_content_types.contains(&u.content_type)
            })
            .collect();

        // Store all allowed URLs
        for exported_url in &exported_urls {
            if Self::is_valid_url(&exported_url.url) && self.should_be_url_stored(exported_url) {
                self.store_file(exported_url, status)?;
            }
        }

        // Add info to summary
        let duration = start_time.elapsed().as_secs_f64();
        if let Some(ref export_dir) = self.markdown_export_directory {
            let formatted_path = utils::get_output_formatted_path(export_dir);
            let formatted_duration = utils::get_formatted_duration(duration);
            status.add_info_to_summary(
                "markdown-generated",
                &format!(
                    "Markdown content generated to '{}' and took {}",
                    formatted_path, formatted_duration
                ),
            );
        }

        // Combine markdown files to single file if requested
        if let (Some(single_file), Some(export_dir)) =
            (&self.markdown_export_single_file, &self.markdown_export_directory)
        {
            let combine_start = Instant::now();
            let combiner = MarkdownSiteAggregator::new(&self.initial_url);

            match combiner.combine_directory(export_dir, self.markdown_remove_links_and_images_from_single_file) {
                Ok(combined_markdown) => {
                    // Ensure directory exists
                    if let Some(parent) = Path::new(single_file).parent()
                        && !parent.exists()
                    {
                        fs::create_dir_all(parent).map_err(|e| {
                            CrawlerError::Export(format!(
                                "Cannot create directory for single markdown file: '{}': {}",
                                parent.display(),
                                e
                            ))
                        })?;
                    }

                    fs::write(single_file, &combined_markdown).map_err(|e| {
                        CrawlerError::Export(format!("Cannot write single markdown file '{}': {}", single_file, e))
                    })?;

                    let combine_duration = combine_start.elapsed().as_secs_f64();
                    let formatted_path = utils::get_output_formatted_path(single_file);
                    let formatted_duration = utils::get_formatted_duration(combine_duration);
                    status.add_info_to_summary(
                        "markdown-combined",
                        &format!(
                            "Markdown files combined into single file '{}' and took {}",
                            formatted_path, formatted_duration
                        ),
                    );
                }
                Err(e) => {
                    status.add_critical_to_summary(
                        "markdown-combine-error",
                        &format!("Error combining markdown files: {}", e),
                    );
                }
            }
        }

        Ok(())
    }
}

/// Extract regex pattern from a delimited string.
fn extract_regex_pattern(input: &str) -> Option<String> {
    if input.len() < 2 {
        return None;
    }
    let delimiter = input.chars().next()?;
    let rest = &input[1..];
    if let Some(end_pos) = rest.rfind(delimiter) {
        let pattern = &rest[..end_pos];
        let flags = &rest[end_pos + 1..];
        let mut regex_pattern = String::new();
        if flags.contains('i') {
            regex_pattern.push_str("(?i)");
        }
        regex_pattern.push_str(pattern);
        Some(regex_pattern)
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_should_be_activated() {
        let mut exporter = MarkdownExporter::new();
        assert!(!exporter.should_be_activated());

        exporter.set_markdown_export_directory(Some("/tmp/md".to_string()));
        assert!(exporter.should_be_activated());
    }

    #[test]
    fn test_detect_language_rust() {
        let exporter = MarkdownExporter::new();
        assert_eq!(exporter.detect_language("fn main() {\n    let mut x = 5;\n}"), "rust");
    }

    #[test]
    fn test_detect_language_python() {
        let exporter = MarkdownExporter::new();
        assert_eq!(
            exporter.detect_language("def hello():\n    print('hello')\nfrom os import path"),
            "python"
        );
    }

    #[test]
    fn test_fix_multiline_images() {
        let exporter = MarkdownExporter::new();
        let input = "[\n![image](src)](link)";
        let result = exporter.fix_multiline_images(input);
        assert_eq!(result, "[![image](src)](link)");
    }
}
