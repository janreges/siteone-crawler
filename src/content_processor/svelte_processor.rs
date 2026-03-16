// SiteOne Crawler - SvelteProcessor
// (c) Jan Reges <jan.reges@siteone.cz>
//
// Handles SvelteKit specific patterns.

use once_cell::sync::Lazy;
use regex::Regex;

use crate::content_processor::base_processor::ProcessorConfig;
use crate::content_processor::content_processor::ContentProcessor;
use crate::engine::found_urls::FoundUrls;
use crate::engine::parsed_url::ParsedUrl;
use crate::types::ContentTypeId;

static RE_SVELTE_TAG: Lazy<Regex> = Lazy::new(|| Regex::new(r"(?i)<svelte:[^>]+>\s*").unwrap());

pub struct SvelteProcessor {
    #[allow(dead_code)]
    config: ProcessorConfig,
    debug_mode: bool,
}

impl SvelteProcessor {
    pub fn new(config: ProcessorConfig) -> Self {
        Self {
            config,
            debug_mode: false,
        }
    }
}

impl ContentProcessor for SvelteProcessor {
    fn find_urls(&self, _content: &str, _source_url: &ParsedUrl) -> Option<FoundUrls> {
        // SvelteProcessor doesn't extract URLs
        None
    }

    fn apply_content_changes_before_url_parsing(
        &self,
        _content: &mut String,
        _content_type: ContentTypeId,
        _url: &ParsedUrl,
    ) {
        // No changes needed before URL parsing in SvelteProcessor
    }

    fn apply_content_changes_for_offline_version(
        &self,
        content: &mut String,
        _content_type: ContentTypeId,
        _url: &ParsedUrl,
        _remove_unwanted_code: bool,
    ) {
        // Remove <svelte:*> tags for offline version
        if content.contains("<svelte:") {
            *content = RE_SVELTE_TAG.replace_all(content, "").to_string();
        }
    }

    fn is_content_type_relevant(&self, content_type: ContentTypeId) -> bool {
        // SvelteProcessor is only relevant for HTML (overrides the base relevantContentTypes)
        content_type == ContentTypeId::Html
    }

    fn get_name(&self) -> &str {
        "SvelteProcessor"
    }

    fn set_debug_mode(&mut self, debug_mode: bool) {
        self.debug_mode = debug_mode;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_config() -> ProcessorConfig {
        ProcessorConfig::new(ParsedUrl::parse("https://example.com/", None))
    }

    #[test]
    fn test_remove_svelte_tags() {
        let processor = SvelteProcessor::new(make_config());
        let mut content = r#"<html><head><svelte:head></svelte:head></head><body>test</body></html>"#.to_string();
        let url = ParsedUrl::parse("https://example.com/", None);
        processor.apply_content_changes_for_offline_version(&mut content, ContentTypeId::Html, &url, false);
        assert!(!content.contains("<svelte:"));
    }

    #[test]
    fn test_is_relevant_only_for_html() {
        let processor = SvelteProcessor::new(make_config());
        assert!(processor.is_content_type_relevant(ContentTypeId::Html));
        assert!(!processor.is_content_type_relevant(ContentTypeId::Script));
        assert!(!processor.is_content_type_relevant(ContentTypeId::Stylesheet));
    }
}
