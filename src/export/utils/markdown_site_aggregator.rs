// SiteOne Crawler - MarkdownSiteAggregator
// (c) Jan Reges <jan.reges@siteone.cz>
//
// Combines multiple markdown files into a single file.

use std::fs;
use std::path::Path;

use regex::Regex;

use crate::error::{CrawlerError, CrawlerResult};

/// Similarity threshold for common header/footer detection (percentage).
const SIMILARITY_THRESHOLD: f64 = 80.0;

/// Combines multiple Markdown files from a directory into a single document.
/// Detects and extracts common headers/footers, adds page separators and URLs.
pub struct MarkdownSiteAggregator {
    base_url: String,
}

impl MarkdownSiteAggregator {
    pub fn new(base_url: &str) -> Self {
        Self {
            base_url: base_url.trim_end_matches('/').to_string(),
        }
    }

    /// Combine all markdown files in a directory into a single document.
    pub fn combine_directory(&self, directory_path: &str, remove_links_and_images: bool) -> CrawlerResult<String> {
        let files = self.get_markdown_files(directory_path)?;

        // Load content of all files into a map [url => lines]
        let mut pages: Vec<(String, Vec<String>)> = Vec::new();
        for file_path in &files {
            let url = self.make_url_from_path(file_path, directory_path);
            let content = fs::read_to_string(file_path)
                .map_err(|e| CrawlerError::Export(format!("Cannot read file '{}': {}", file_path, e)))?;
            let lines: Vec<String> = content.trim_end().split('\n').map(|s| s.to_string()).collect();
            pages.push((url, lines));
        }

        // Sort URLs to ensure index pages come first
        let base_url = self.base_url.clone();
        pages.sort_by(|(url_a, _), (url_b, _)| {
            // Root URL should always be first
            if url_a == &base_url || url_a.is_empty() {
                return std::cmp::Ordering::Less;
            }
            if url_b == &base_url || url_b.is_empty() {
                return std::cmp::Ordering::Greater;
            }

            let parts_a: Vec<&str> = url_a.trim_end_matches('/').split('/').collect();
            let parts_b: Vec<&str> = url_b.trim_end_matches('/').split('/').collect();

            let min_len = parts_a.len().min(parts_b.len());
            for i in 0..min_len {
                if parts_a[i] != parts_b[i] {
                    return parts_a[i].cmp(parts_b[i]);
                }
            }

            parts_a.len().cmp(&parts_b.len())
        });

        // Detect common header and footer
        let page_lines: Vec<&Vec<String>> = pages.iter().map(|(_, lines)| lines).collect();
        let header_lines = self.detect_common_header(&page_lines);
        let footer_lines = self.detect_common_footer(&page_lines);

        // Remove header and footer from individual pages
        for (_, lines) in &mut pages {
            if !header_lines.is_empty() {
                *lines = self.remove_prefix(lines, &header_lines);
            }
            if !footer_lines.is_empty() {
                *lines = self.remove_suffix(lines, &footer_lines);
            }
        }

        // Build resulting markdown
        let mut result_lines: Vec<String> = Vec::new();
        if !header_lines.is_empty() {
            result_lines.extend(header_lines.iter().cloned());
            result_lines.push(String::new());
        }

        // Add content of all pages with their URLs
        for (url, lines) in &pages {
            // Use emoji + URL marker
            result_lines.push(format!("\u{2B07}\u{FE0F} `URL: {}`\n\n---\n\n", url));
            for line in lines {
                result_lines.push(line.clone());
            }
            result_lines.push("\n\n---\n".to_string());
        }

        if !footer_lines.is_empty() {
            // Remove the last empty line before footer if present
            if result_lines.last().map(|s| s.is_empty()).unwrap_or(false) {
                result_lines.pop();
            }
            result_lines.push(String::new());
            result_lines.extend(footer_lines.iter().cloned());
        }

        let mut final_markdown = result_lines.join("\n");

        if remove_links_and_images {
            final_markdown = self.remove_links_and_images(&final_markdown);
        }

        Ok(final_markdown)
    }

    /// Get all markdown files in a directory recursively.
    fn get_markdown_files(&self, dir: &str) -> CrawlerResult<Vec<String>> {
        let mut paths = Vec::new();
        self.collect_markdown_files(dir, &mut paths)?;
        Ok(paths)
    }

    #[allow(clippy::only_used_in_recursion)]
    fn collect_markdown_files(&self, dir: &str, paths: &mut Vec<String>) -> CrawlerResult<()> {
        let dir_path = Path::new(dir);
        if !dir_path.is_dir() {
            return Ok(());
        }

        let entries = fs::read_dir(dir_path)
            .map_err(|e| CrawlerError::Export(format!("Cannot read directory '{}': {}", dir, e)))?;

        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                self.collect_markdown_files(&path.to_string_lossy(), paths)?;
            } else if path.is_file()
                && let Some(ext) = path.extension()
                && ext.to_str().map(|e| e.to_lowercase()) == Some("md".to_string())
            {
                paths.push(path.to_string_lossy().to_string());
            }
        }

        Ok(())
    }

    /// Make URL from file path.
    fn make_url_from_path(&self, file_path: &str, root_dir: &str) -> String {
        let root = root_dir.trim_end_matches('/');
        let rel_path = file_path[root.len()..].trim_start_matches('/').replace('\\', "/");

        // Remove .md extension
        let rel_path = if rel_path.ends_with(".md") {
            &rel_path[..rel_path.len() - 3]
        } else {
            &rel_path
        };

        // Replace index at end with /
        let rel_path = Regex::new(r"/index$")
            .map(|re| re.replace(rel_path, "/").to_string())
            .unwrap_or_else(|_| rel_path.to_string());

        // Handle root index.md
        if rel_path == "index" || rel_path.is_empty() {
            return if !self.base_url.is_empty() {
                self.base_url.clone()
            } else {
                String::new()
            };
        }

        if !self.base_url.is_empty() {
            format!("{}/{}", self.base_url, rel_path.trim_start_matches('/'))
        } else {
            rel_path.to_string()
        }
    }

    /// Detect common header across pages.
    fn detect_common_header(&self, pages: &[&Vec<String>]) -> Vec<String> {
        if pages.is_empty() {
            return Vec::new();
        }

        // Use pages starting from index 2 (skip first 2), take up to 3
        let sample_start = 2.min(pages.len());
        let sample_end = (sample_start + 3).min(pages.len());
        if sample_start >= sample_end {
            return Vec::new();
        }

        let sample_pages = &pages[sample_start..sample_end];

        let mut common_header = sample_pages[0].clone();
        for page in sample_pages.iter().skip(1) {
            common_header = self.align_common_prefix(&common_header, page);
            if common_header.is_empty() {
                break;
            }
        }

        common_header
    }

    /// Detect common footer across pages.
    fn detect_common_footer(&self, pages: &[&Vec<String>]) -> Vec<String> {
        if pages.is_empty() {
            return Vec::new();
        }

        let sample_start = 2.min(pages.len());
        let sample_end = (sample_start + 3).min(pages.len());
        if sample_start >= sample_end {
            return Vec::new();
        }

        let sample_pages = &pages[sample_start..sample_end];

        // Reverse the first page
        let mut common_footer: Vec<String> = sample_pages[0].iter().rev().cloned().collect();
        for page in sample_pages.iter().skip(1) {
            let other_rev: Vec<String> = page.iter().rev().cloned().collect();
            common_footer = self.align_common_prefix(&common_footer, &other_rev);
            if common_footer.is_empty() {
                break;
            }
        }

        // Reverse back to correct order
        common_footer.reverse();
        common_footer
    }

    /// Align two line arrays and find their common prefix with fuzzy tolerance.
    fn align_common_prefix(&self, lines_a: &[String], lines_b: &[String]) -> Vec<String> {
        let mut result = Vec::new();
        let mut i = 0;
        let mut j = 0;

        while i < lines_a.len() && j < lines_b.len() {
            if self.lines_similar(&lines_a[i], &lines_b[j]) {
                result.push(lines_a[i].clone());
                i += 1;
                j += 1;
            } else {
                // Try skipping a line in A or B
                let skip_a = i + 1 < lines_a.len() && self.lines_similar(&lines_a[i + 1], &lines_b[j]);
                let skip_b = !skip_a && j + 1 < lines_b.len() && self.lines_similar(&lines_a[i], &lines_b[j + 1]);

                if skip_a {
                    i += 1;
                } else if skip_b {
                    j += 1;
                } else {
                    break;
                }
            }
        }

        result
    }

    /// Evaluate similarity of two lines (ignoring markdown formatting).
    fn lines_similar(&self, a: &str, b: &str) -> bool {
        let normalize = |s: &str| -> String {
            let result = Regex::new(r"[*_]+")
                .map(|re| re.replace_all(s, "").to_string())
                .unwrap_or_else(|_| s.to_string());
            result.trim().to_string()
        };

        let na = normalize(a);
        let nb = normalize(b);

        if na == nb {
            return true;
        }

        // Calculate similarity percentage
        let percent = self.similar_text_percent(&na, &nb);
        percent >= SIMILARITY_THRESHOLD
    }

    /// Calculate similarity percentage between two strings.
    fn similar_text_percent(&self, a: &str, b: &str) -> f64 {
        if a.is_empty() && b.is_empty() {
            return 100.0;
        }
        if a.is_empty() || b.is_empty() {
            return 0.0;
        }

        let matching = self.longest_common_substring_len(a, b);
        let total = (a.len() + b.len()) as f64;
        (2.0 * matching as f64 / total) * 100.0
    }

    /// Find the length of the longest common substring.
    fn longest_common_substring_len(&self, a: &str, b: &str) -> usize {
        let a_bytes = a.as_bytes();
        let b_bytes = b.as_bytes();
        let m = a_bytes.len();
        let n = b_bytes.len();

        if m == 0 || n == 0 {
            return 0;
        }

        let mut max_len = 0;
        let mut prev = vec![0usize; n + 1];
        let mut curr = vec![0usize; n + 1];

        for i in 1..=m {
            for j in 1..=n {
                if a_bytes[i - 1] == b_bytes[j - 1] {
                    curr[j] = prev[j - 1] + 1;
                    max_len = max_len.max(curr[j]);
                } else {
                    curr[j] = 0;
                }
            }
            std::mem::swap(&mut prev, &mut curr);
            curr.fill(0);
        }

        max_len
    }

    /// Remove common prefix (header) from a page's lines.
    fn remove_prefix(&self, lines: &[String], prefix_lines: &[String]) -> Vec<String> {
        if prefix_lines.is_empty() {
            return lines.to_vec();
        }
        let len = prefix_lines.len();
        if lines.len() >= len {
            lines[len..].to_vec()
        } else {
            lines.to_vec()
        }
    }

    /// Remove common suffix (footer) from a page's lines.
    fn remove_suffix(&self, lines: &[String], suffix_lines: &[String]) -> Vec<String> {
        if suffix_lines.is_empty() {
            return lines.to_vec();
        }
        let len = suffix_lines.len();
        if lines.len() >= len {
            lines[..lines.len() - len].to_vec()
        } else {
            lines.to_vec()
        }
    }

    /// Remove links and images from markdown text.
    fn remove_links_and_images(&self, markdown: &str) -> String {
        let mut result = markdown.to_string();

        // Remove image in anchor text
        if let Ok(re) = Regex::new(r"\[!\[[^\]]*\]\([^\)]*\)\]\([^\)]*\)") {
            result = re.replace_all(&result, "").to_string();
        }

        // Remove standalone images
        if let Ok(re) = Regex::new(r#"!\[.*?\]\([^)]*\)(\s*"[^"]*")?"#) {
            result = re.replace_all(&result, "").to_string();
        }

        // Replace links in list items
        if let Ok(re) = Regex::new(r"(?m)^\s*(\*|-|[0-9]+\.)\s*\[([^\]]+)\]\([^)]+\)") {
            result = re.replace_all(&result, "").to_string();
        }

        // Replace empty links
        if let Ok(re) = Regex::new(r"\[\]\([^)]+\)") {
            result = re.replace_all(&result, "").to_string();
        }

        // Clean up tables - remove rows with only whitespace and pipes
        if let Ok(re) = Regex::new(r"(?m)^\s*(\|\s*)+\|\s*$") {
            result = re.replace_all(&result, "").to_string();
        }

        // Clean empty list items
        if let Ok(re) = Regex::new(r"(?m)^\s*(\*|-|[0-9]+\.)\s*$") {
            result = re.replace_all(&result, "").to_string();
        }

        // Remove multiple consecutive empty lines
        if let Ok(re) = Regex::new(r"\n{3,}") {
            result = re.replace_all(&result, "\n\n").to_string();
        }

        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_make_url_from_path() {
        let aggregator = MarkdownSiteAggregator::new("https://example.com");
        assert_eq!(
            aggregator.make_url_from_path("/tmp/export/index.md", "/tmp/export"),
            "https://example.com"
        );
        assert_eq!(
            aggregator.make_url_from_path("/tmp/export/about.md", "/tmp/export"),
            "https://example.com/about"
        );
        assert_eq!(
            aggregator.make_url_from_path("/tmp/export/docs/intro.md", "/tmp/export"),
            "https://example.com/docs/intro"
        );
    }

    #[test]
    fn test_lines_similar() {
        let aggregator = MarkdownSiteAggregator::new("");
        assert!(aggregator.lines_similar("Hello world", "Hello world"));
        assert!(aggregator.lines_similar("**Hello** world", "Hello world"));
        assert!(!aggregator.lines_similar("Hello", "Completely different text"));
    }

    #[test]
    fn test_remove_links_and_images() {
        let aggregator = MarkdownSiteAggregator::new("");
        let input = "Some text ![image](img.jpg) and [link](url)";
        let result = aggregator.remove_links_and_images(input);
        assert!(!result.contains("![image]"));
    }
}
