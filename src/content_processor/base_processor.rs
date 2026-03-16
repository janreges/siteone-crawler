// SiteOne Crawler - BaseProcessor shared utilities
// (c) Jan Reges <jan.reges@siteone.cz>
//
// Provides shared utility methods used by all content processors.

use crate::engine::parsed_url::ParsedUrl;
use crate::export::utils::offline_url_converter::OfflineUrlConverter;
use crate::types::ContentTypeId;

/// Configuration extracted from CoreOptions, shared across processors.
/// This avoids each processor needing a reference to the full crawler.
#[derive(Debug, Clone)]
pub struct ProcessorConfig {
    pub single_page: bool,
    pub single_foreign_page: bool,
    pub max_depth: i64,
    pub files_enabled: bool,
    pub images_enabled: bool,
    pub scripts_enabled: bool,
    pub styles_enabled: bool,
    pub fonts_enabled: bool,
    pub disable_javascript: bool,
    pub remove_all_anchor_listeners: bool,
    pub ignore_regex: Vec<String>,
    /// Pre-compiled ignore regexes for hot path usage
    pub compiled_ignore_regex: Vec<regex::Regex>,
    pub disable_astro_inline_modules: bool,
    pub initial_url: ParsedUrl,
}

impl ProcessorConfig {
    pub fn new(initial_url: ParsedUrl) -> Self {
        Self {
            single_page: false,
            single_foreign_page: false,
            max_depth: 0,
            files_enabled: true,
            images_enabled: true,
            scripts_enabled: true,
            styles_enabled: true,
            fonts_enabled: true,
            disable_javascript: false,
            remove_all_anchor_listeners: false,
            ignore_regex: Vec::new(),
            compiled_ignore_regex: Vec::new(),
            disable_astro_inline_modules: false,
            initial_url,
        }
    }

    /// Compile ignore_regex patterns into Regex objects for hot path usage.
    /// Call this after setting ignore_regex.
    pub fn compile_ignore_regex(&mut self) {
        self.compiled_ignore_regex = self
            .ignore_regex
            .iter()
            .filter_map(|pattern| regex::Regex::new(pattern).ok())
            .collect();
    }
}

/// Check if a content type is in the list of relevant types
pub fn is_relevant(content_type: ContentTypeId, relevant_types: &[ContentTypeId]) -> bool {
    relevant_types.contains(&content_type)
}

/// Convert a URL to a relative path for offline use.
pub fn convert_url_to_relative(
    base_url: &ParsedUrl,
    target_url: &str,
    initial_url: &ParsedUrl,
    attribute: Option<&str>,
) -> String {
    // If it's a data URI, anchor, or non-http scheme, return as-is
    if target_url.starts_with("data:")
        || target_url.starts_with("javascript:")
        || target_url.starts_with("mailto:")
        || target_url.starts_with("tel:")
    {
        return target_url.to_string();
    }

    let parsed_target = ParsedUrl::parse(target_url, Some(base_url));

    let mut converter = OfflineUrlConverter::new(
        initial_url.clone(),
        base_url.clone(),
        parsed_target,
        None,
        None,
        attribute,
    );

    converter.convert_url_to_relative(true)
}
