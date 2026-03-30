// SiteOne Crawler - CssProcessor
// (c) Jan Reges <jan.reges@siteone.cz>
//
// Extracts URLs from CSS url() and @import, and converts for offline use.

use once_cell::sync::Lazy;
use regex::Regex;

use crate::content_processor::base_processor::{ProcessorConfig, convert_url_to_relative, is_relevant};
use crate::content_processor::content_processor::ContentProcessor;
use crate::engine::found_url::UrlSource;
use crate::engine::found_urls::FoundUrls;
use crate::engine::parsed_url::ParsedUrl;
use crate::types::ContentTypeId;
use crate::utils;

static RE_CSS_URL: Lazy<Regex> = Lazy::new(|| Regex::new(r#"(?im)url\s*\(\s*["']?([^"')]+)["']?\s*\)"#).unwrap());

static RE_IS_IMAGE: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"(?i)\.(jpg|jpeg|png|gif|webp|avif|svg|ico|tif|bmp)(\?.*|#.*)?$").unwrap());

static RE_IS_FONT: Lazy<Regex> = Lazy::new(|| Regex::new(r"(?i)\.(eot|ttf|woff2|woff|otf)(\?.*|#.*)?$").unwrap());

static RE_IS_CSS: Lazy<Regex> = Lazy::new(|| Regex::new(r"(?i)\.css(\?.*|#.*)?$").unwrap());

static RE_CSS_URL_OFFLINE: Lazy<Regex> =
    Lazy::new(|| Regex::new(r#"(?i)url\((['"]?)((?:[^'")\s]|\([^)]*\))+)['"]?\)"#).unwrap());

pub struct CssProcessor {
    config: ProcessorConfig,
    debug_mode: bool,
    relevant_content_types: Vec<ContentTypeId>,
}

impl CssProcessor {
    pub fn new(config: ProcessorConfig) -> Self {
        Self {
            config,
            debug_mode: false,
            relevant_content_types: vec![ContentTypeId::Html, ContentTypeId::Stylesheet],
        }
    }

    /// Remove unwanted code from CSS based on disable options
    fn remove_unwanted_code_from_css(&self, css: &str) -> String {
        let mut result = css.to_string();

        if !self.config.fonts_enabled {
            result = utils::strip_fonts(&result);
        }
        if !self.config.images_enabled {
            result = utils::strip_images(&result, None);
        }

        result
    }
}

impl ContentProcessor for CssProcessor {
    fn find_urls(&self, content: &str, source_url: &ParsedUrl) -> Option<FoundUrls> {
        let source_url_str = source_url.get_full_url(true, false);

        // Find all url() references in CSS
        let mut url_texts: Vec<&str> = Vec::new();
        for caps in RE_CSS_URL.captures_iter(content) {
            if let Some(m) = caps.get(1) {
                let url = m.as_str();
                let is_image = RE_IS_IMAGE.is_match(url);
                let is_font = RE_IS_FONT.is_match(url);
                let is_css = RE_IS_CSS.is_match(url);

                if (self.config.images_enabled && is_image)
                    || (self.config.fonts_enabled && is_font)
                    || (self.config.styles_enabled && is_css)
                {
                    url_texts.push(url);
                }
            }
        }

        let mut found_urls = FoundUrls::new();
        found_urls.add_urls_from_text_array(&url_texts, &source_url_str, UrlSource::CssUrl);

        if found_urls.get_count() > 0 {
            Some(found_urls)
        } else {
            None
        }
    }

    fn apply_content_changes_before_url_parsing(
        &self,
        _content: &mut String,
        _content_type: ContentTypeId,
        _url: &ParsedUrl,
    ) {
        // No changes needed before URL parsing in CssProcessor
    }

    fn apply_content_changes_for_offline_version(
        &self,
        content: &mut String,
        _content_type: ContentTypeId,
        url: &ParsedUrl,
        _remove_unwanted_code: bool,
    ) {
        let initial_url = &self.config.initial_url;

        *content = RE_CSS_URL_OFFLINE
            .replace_all(content, |caps: &regex::Captures| {
                let quote = caps.get(1).map_or("", |m| m.as_str());
                let found_url = caps.get(2).map_or("", |m| m.as_str());

                // If data URI, anchor, or non-requestable resource, skip
                if !utils::is_href_for_requestable_resource(found_url) || found_url.starts_with('#') {
                    return caps.get(0).map_or("", |m| m.as_str()).to_string();
                }

                let relative_url = convert_url_to_relative(
                    url,
                    found_url,
                    initial_url,
                    None,
                    self.config.offline_export_preserve_urls,
                );
                format!("url({}{}{})", quote, relative_url, quote)
            })
            .to_string();

        *content = self.remove_unwanted_code_from_css(content);
    }

    fn is_content_type_relevant(&self, content_type: ContentTypeId) -> bool {
        is_relevant(content_type, &self.relevant_content_types)
    }

    fn get_name(&self) -> &str {
        "CssProcessor"
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
    fn test_find_css_urls() {
        let processor = CssProcessor::new(make_config());
        let css = r#"
            body { background: url('/img/bg.jpg'); }
            @font-face { src: url('/fonts/custom.woff2'); }
        "#;
        let source = ParsedUrl::parse("https://example.com/style.css", None);
        let result = processor.find_urls(css, &source);
        assert!(result.is_some());
        assert!(result.unwrap().get_count() >= 2);
    }

    #[test]
    fn test_find_css_urls_disabled_images() {
        let mut config = make_config();
        config.images_enabled = false;
        let processor = CssProcessor::new(config);
        let css = r#"body { background: url('/img/bg.jpg'); }"#;
        let source = ParsedUrl::parse("https://example.com/style.css", None);
        let result = processor.find_urls(css, &source);
        // Should be None because images are disabled
        assert!(result.is_none());
    }
}
