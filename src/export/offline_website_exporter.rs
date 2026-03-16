// SiteOne Crawler - OfflineWebsiteExporter
// (c) Jan Reges <jan.reges@siteone.cz>
//
// Saves all crawled pages to local filesystem for offline browsing.

use std::fs;
use std::path::Path;
use std::sync::{Arc, Mutex};
use std::time::Instant;

use md5::{Digest, Md5};
use regex::Regex;

use crate::content_processor::manager::ContentProcessorManager;
use crate::engine::parsed_url::ParsedUrl;
use crate::error::{CrawlerError, CrawlerResult};
use crate::export::exporter::Exporter;
use crate::export::utils::offline_url_converter::OfflineUrlConverter;
use crate::export::utils::target_domain_relation::TargetDomainRelation;
use crate::output::output::Output;
use crate::result::status::Status;
use crate::result::visited_url::VisitedUrl;
use crate::types::ContentTypeId;
use crate::utils;

/// Content types that require URL rewriting for offline browsing
const CONTENT_TYPES_REQUIRING_CHANGES: &[ContentTypeId] = &[
    ContentTypeId::Html,
    ContentTypeId::Script,
    ContentTypeId::Stylesheet,
    ContentTypeId::Redirect,
];

/// Exports all crawled pages to a local directory for offline browsing.
/// Rewrites URLs in HTML/CSS/JS for offline navigation.
pub struct OfflineWebsiteExporter {
    offline_export_directory: Option<String>,
    offline_export_store_only_url_regex: Vec<String>,
    offline_export_remove_unwanted_code: bool,
    offline_export_no_auto_redirect_html: bool,
    offline_export_preserve_url_structure: bool,
    offline_export_lowercase: bool,
    ignore_store_file_error: bool,
    replace_content: Vec<String>,
    replace_query_string: Vec<String>,
    initial_parsed_url: Option<ParsedUrl>,
    content_processor_manager: Option<Arc<Mutex<ContentProcessorManager>>>,
    #[allow(clippy::type_complexity)]
    is_domain_allowed_for_static_files: Option<Box<dyn Fn(&str) -> bool + Send + Sync>>,
    #[allow(clippy::type_complexity)]
    is_external_domain_allowed_for_crawling: Option<Box<dyn Fn(&str) -> bool + Send + Sync>>,
}

impl Default for OfflineWebsiteExporter {
    fn default() -> Self {
        Self::new()
    }
}

impl OfflineWebsiteExporter {
    pub fn new() -> Self {
        Self {
            offline_export_directory: None,
            offline_export_store_only_url_regex: Vec::new(),
            offline_export_remove_unwanted_code: false,
            offline_export_no_auto_redirect_html: false,
            offline_export_preserve_url_structure: false,
            offline_export_lowercase: false,
            ignore_store_file_error: false,
            replace_content: Vec::new(),
            replace_query_string: Vec::new(),
            initial_parsed_url: None,
            content_processor_manager: None,
            is_domain_allowed_for_static_files: None,
            is_external_domain_allowed_for_crawling: None,
        }
    }

    pub fn set_offline_export_directory(&mut self, dir: Option<String>) {
        self.offline_export_directory = dir.map(|d| d.trim_end_matches('/').to_string());
    }

    pub fn set_offline_export_store_only_url_regex(&mut self, regexes: Vec<String>) {
        self.offline_export_store_only_url_regex = regexes;
    }

    pub fn set_offline_export_remove_unwanted_code(&mut self, remove: bool) {
        self.offline_export_remove_unwanted_code = remove;
    }

    pub fn set_offline_export_no_auto_redirect_html(&mut self, disable: bool) {
        self.offline_export_no_auto_redirect_html = disable;
    }

    pub fn set_offline_export_preserve_url_structure(&mut self, preserve: bool) {
        self.offline_export_preserve_url_structure = preserve;
    }

    pub fn set_offline_export_lowercase(&mut self, lowercase: bool) {
        self.offline_export_lowercase = lowercase;
    }

    pub fn set_ignore_store_file_error(&mut self, ignore: bool) {
        self.ignore_store_file_error = ignore;
    }

    pub fn set_replace_content(&mut self, replacements: Vec<String>) {
        self.replace_content = replacements;
    }

    pub fn set_replace_query_string(&mut self, replacements: Vec<String>) {
        self.replace_query_string = replacements;
    }

    pub fn set_initial_parsed_url(&mut self, url: ParsedUrl) {
        self.initial_parsed_url = Some(url);
    }

    pub fn set_content_processor_manager(&mut self, cpm: Arc<Mutex<ContentProcessorManager>>) {
        self.content_processor_manager = Some(cpm);
    }

    pub fn set_domain_callbacks(
        &mut self,
        static_files: Box<dyn Fn(&str) -> bool + Send + Sync>,
        crawling: Box<dyn Fn(&str) -> bool + Send + Sync>,
    ) {
        self.is_domain_allowed_for_static_files = Some(static_files);
        self.is_external_domain_allowed_for_crawling = Some(crawling);
    }

    /// Store a single file to the offline export directory.
    fn store_file(&self, visited_url: &VisitedUrl, status: &Status, _output: &dyn Output) -> CrawlerResult<()> {
        let export_dir = self
            .offline_export_directory
            .as_ref()
            .ok_or_else(|| CrawlerError::Export("Offline export directory not set".to_string()))?;

        let body_bytes = status.get_url_body(&visited_url.uq_id).unwrap_or_default();

        // For content types requiring URL rewriting (HTML, CSS, JS), work with text
        // For binary content types (images, fonts), keep raw bytes
        let final_bytes =
            if !body_bytes.is_empty() && CONTENT_TYPES_REQUIRING_CHANGES.contains(&visited_url.content_type) {
                let mut content = String::from_utf8_lossy(&body_bytes).into_owned();

                // Apply content changes through all content processors (URL rewriting for offline)
                if let Some(ref cpm) = self.content_processor_manager {
                    let parsed_url = ParsedUrl::parse(&visited_url.url, None);
                    if let Ok(mut manager) = cpm.lock() {
                        let original_content = content.clone();

                        // Create a content loader that loads module content by URL from storage.
                        // This enables Astro module inlining (and any future processor that needs it).
                        let content_loader = |url_str: &str| -> Option<String> {
                            let parsed = ParsedUrl::parse(url_str, None);
                            let full_url = parsed.get_full_url(true, false);
                            let mut hasher = Md5::new();
                            hasher.update(full_url.as_bytes());
                            let hash = format!("{:x}", hasher.finalize());
                            let uq_id = hash[..8].to_string();
                            status.get_url_body_text(&uq_id)
                        };

                        manager.apply_content_changes_for_offline_version_with_loader(
                            &mut content,
                            visited_url.content_type,
                            &parsed_url,
                            self.offline_export_remove_unwanted_code,
                            &content_loader,
                        );
                        // If content was somehow corrupted, use original
                        if content.is_empty() {
                            content = original_content;
                        }
                    }
                }

                // Apply custom content replacements
                if !self.replace_content.is_empty() {
                    for replace in &self.replace_content {
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

        // Check if we should save the file
        let mut save_file = true;
        if Path::new(&store_file_path).exists()
            && let Some(ref initial_url) = self.initial_parsed_url
            && !visited_url.is_https()
            && initial_url.is_https()
        {
            save_file = false;
            let message = format!(
                "File '{}' already exists and will not be overwritten because initial request was HTTPS and this request is HTTP: {}",
                store_file_path, visited_url.url
            );
            status.add_notice_to_summary("offline-exporter-store-file-ignored", &message);
        }

        if save_file && let Err(e) = fs::write(&store_file_path, &final_bytes) {
            let has_extension = Regex::new(r"(?i)\.[a-z0-9\-]{1,15}$")
                .map(|re| re.is_match(&store_file_path))
                .unwrap_or(false);

            if has_extension && !self.ignore_store_file_error {
                return Err(CrawlerError::Export(format!(
                    "Cannot store file '{}': {}",
                    store_file_path, e
                )));
            } else {
                let message = format!(
                    "Cannot store file '{}' (undefined extension). Original URL: {}",
                    store_file_path, visited_url.url
                );
                status.add_notice_to_summary("offline-exporter-store-file-error", &message);
            }
        }

        Ok(())
    }

    /// Check if URL should be stored based on filters.
    fn should_be_url_stored(&self, visited_url: &VisitedUrl) -> bool {
        let mut result = false;

        // Check --offline-export-store-only-url-regex
        if !self.offline_export_store_only_url_regex.is_empty() {
            for regex_str in &self.offline_export_store_only_url_regex {
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

        // Check --allow-domain-* for external domains
        if result && visited_url.is_external {
            let parsed_url = ParsedUrl::parse(&visited_url.url, None);
            if let Some(ref host) = parsed_url.host
                && let Some(ref cb) = self.is_external_domain_allowed_for_crawling
            {
                if cb(host) {
                    result = true;
                } else if visited_url.is_static_file() || parsed_url.is_static_file() {
                    if let Some(ref static_cb) = self.is_domain_allowed_for_static_files {
                        result = static_cb(host);
                    } else {
                        result = false;
                    }
                } else {
                    result = false;
                }
            }
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

        // Determine source attribute hint
        let attribute = if visited_url.content_type == ContentTypeId::Image {
            "src"
        } else {
            "href"
        };

        let mut converter = OfflineUrlConverter::new(initial_url, base_url, target_url, None, None, Some(attribute));
        converter.set_preserve_url_structure(self.offline_export_preserve_url_structure);

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

    /// Validate URL for export.
    fn is_valid_url(url: &str) -> bool {
        // First try standard URL parsing
        if url::Url::parse(url).is_ok() {
            return true;
        }

        // Try with URL-encoded version for international characters
        let encoded: String = url
            .chars()
            .map(|c| {
                if c.is_ascii() && c as u32 >= 0x20 && (c as u32) <= 0x7E {
                    c.to_string()
                } else {
                    percent_encoding::utf8_percent_encode(&c.to_string(), percent_encoding::NON_ALPHANUMERIC)
                        .to_string()
                }
            })
            .collect();

        url::Url::parse(&encoded).is_ok()
    }

    /// Add redirect HTML files to subfolders that contain index.html.
    fn add_redirect_html_to_subfolders(dir: &str) -> CrawlerResult<()> {
        let dir_path = Path::new(dir);
        if !dir_path.is_dir() {
            return Ok(());
        }

        let entries = fs::read_dir(dir_path)
            .map_err(|e| CrawlerError::Export(format!("Cannot read directory '{}': {}", dir, e)))?;

        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                let dir_name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
                let index_html_path = path.join("index.html");

                if index_html_path.exists() {
                    // Create redirect HTML file for the folder
                    let redirect_html_path = format!("{}.html", path.display());
                    if !Path::new(&redirect_html_path).exists() {
                        let redirect_content = format!(
                            "<!DOCTYPE html><meta http-equiv=\"refresh\" content=\"0;url={}/index.html\">",
                            dir_name
                        );
                        let _ = fs::write(&redirect_html_path, redirect_content);
                    }
                }

                // Recurse into subdirectories
                Self::add_redirect_html_to_subfolders(&path.to_string_lossy())?;
            }
        }

        Ok(())
    }
}

impl Exporter for OfflineWebsiteExporter {
    fn get_name(&self) -> &str {
        "OfflineWebsiteExporter"
    }

    fn should_be_activated(&self) -> bool {
        self.offline_export_directory.is_some()
    }

    fn export(&mut self, status: &Status, output: &dyn Output) -> CrawlerResult<()> {
        let start_time = Instant::now();
        let export_dir = match self.offline_export_directory {
            Some(ref dir) => dir.clone(),
            None => return Ok(()),
        };

        // Set replace_query_string configuration
        OfflineUrlConverter::set_replace_query_string(self.replace_query_string.clone());

        // Set lowercase configuration for all URL conversions
        OfflineUrlConverter::set_lowercase(self.offline_export_lowercase);

        let visited_urls = status.get_visited_urls();

        // Filter relevant URLs with OK status codes
        let exported_urls: Vec<&VisitedUrl> = visited_urls
            .iter()
            .filter(|u| matches!(u.status_code, 200 | 201 | 301 | 302 | 303 | 308))
            .collect();

        // Store all allowed URLs
        for exported_url in &exported_urls {
            if Self::is_valid_url(&exported_url.url) && self.should_be_url_stored(exported_url) {
                self.store_file(exported_url, status, output)?;
            }
        }

        // Add redirect HTML files for subfolders
        if !self.offline_export_no_auto_redirect_html {
            let _ = Self::add_redirect_html_to_subfolders(&export_dir);
        }

        // Add info to summary
        let duration = start_time.elapsed().as_secs_f64();
        let formatted_path = utils::get_output_formatted_path(&export_dir);
        let formatted_duration = utils::get_formatted_duration(duration);
        status.add_info_to_summary(
            "offline-website-generated",
            &format!(
                "Offline website generated to '{}' and took {}",
                formatted_path, formatted_duration
            ),
        );

        Ok(())
    }
}

/// Extract regex pattern from a delimited string (e.g., /pattern/flags)
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
    fn test_is_valid_url() {
        assert!(OfflineWebsiteExporter::is_valid_url("https://example.com/"));
        assert!(OfflineWebsiteExporter::is_valid_url("https://example.com/path/page"));
        assert!(!OfflineWebsiteExporter::is_valid_url("not-a-url"));
    }

    #[test]
    fn test_should_be_activated() {
        let mut exporter = OfflineWebsiteExporter::new();
        assert!(!exporter.should_be_activated());

        exporter.set_offline_export_directory(Some("/tmp/offline".to_string()));
        assert!(exporter.should_be_activated());
    }
}
