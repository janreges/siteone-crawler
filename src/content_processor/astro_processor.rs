// SiteOne Crawler - AstroProcessor
// (c) Jan Reges <jan.reges@siteone.cz>
//
// Handles Astro specific patterns - extracts component-url and renderer-url,
// and inlines modules for offline version (CORS blocking with file:// protocol).

use std::collections::HashSet;

use md5::{Digest, Md5};
use once_cell::sync::Lazy;
use regex::Regex;

use crate::content_processor::base_processor::{ProcessorConfig, is_relevant};
use crate::content_processor::content_processor::ContentProcessor;
use crate::engine::found_url::{FoundUrl, UrlSource};
use crate::engine::found_urls::FoundUrls;
use crate::engine::parsed_url::ParsedUrl;
use crate::types::ContentTypeId;

static RE_ASTRO_URLS: Lazy<Regex> =
    Lazy::new(|| Regex::new(r#"(?i)(component-url|renderer-url)=["']([^"']+)["']"#).unwrap());

// For offline version - match <script type="module" src="..."> tags
static RE_MODULE_SCRIPT_SRC_FIRST: Lazy<Regex> =
    Lazy::new(|| Regex::new(r#"(?im)<script[^>]+type="module"[^>]+src="([^"]+)"[^>]*>\s*</script>"#).unwrap());

static RE_MODULE_SCRIPT_SRC_SECOND: Lazy<Regex> =
    Lazy::new(|| Regex::new(r#"(?im)<script[^>]+src="([^"]+)"[^>]+type="module"[^>]*>\s*</script>"#).unwrap());

static RE_IMPORT_STATEMENT: Lazy<Regex> = Lazy::new(|| Regex::new(r#"(?i)import\s*["']([^"']+)["']\s*;?"#).unwrap());

pub struct AstroProcessor {
    #[allow(dead_code)]
    config: ProcessorConfig,
    debug_mode: bool,
    relevant_content_types: Vec<ContentTypeId>,
}

impl AstroProcessor {
    pub fn new(config: ProcessorConfig) -> Self {
        Self {
            config,
            debug_mode: false,
            relevant_content_types: vec![ContentTypeId::Html, ContentTypeId::Script],
        }
    }
}

impl AstroProcessor {
    /// Recursively detect and inline imported modules.
    #[allow(clippy::only_used_in_recursion)]
    fn detect_and_include_other_modules(
        &self,
        module_content: &str,
        module_url: &ParsedUrl,
        inline_modules: &mut Vec<String>,
        content_loader: &dyn Fn(&str) -> Option<String>,
        depth: u32,
    ) -> String {
        if depth > 10 {
            return module_content.to_string();
        }

        RE_IMPORT_STATEMENT
            .replace_all(module_content, |caps: &regex::Captures| {
                let src = caps.get(1).map_or("", |m| m.as_str()).trim();
                let src_parsed_url = ParsedUrl::parse(src, Some(module_url));
                let src_full_url = src_parsed_url.get_full_url(true, false);

                if let Some(mut src_content) = content_loader(&src_full_url) {
                    if src_content.contains("import") {
                        src_content = self.detect_and_include_other_modules(
                            &src_content,
                            &src_parsed_url,
                            inline_modules,
                            content_loader,
                            depth + 1,
                        );
                    }
                    inline_modules.push(src_content);

                    if depth == 0 {
                        "/* SiteOne Crawler: imported as inline modules recursively */".to_string()
                    } else {
                        src.to_string()
                    }
                } else {
                    // Module not found in storage, keep original import
                    caps[0].to_string()
                }
            })
            .to_string()
    }

    /// Replace module script tag with inlined content.
    fn inline_module_script(
        &self,
        src: &str,
        url: &ParsedUrl,
        already_included: &mut HashSet<String>,
        content_loader: &dyn Fn(&str) -> Option<String>,
    ) -> String {
        let src_parsed_url = ParsedUrl::parse(src, Some(url));
        let src_full_url = src_parsed_url.get_full_url(true, false);

        if let Some(src_content) = content_loader(&src_full_url) {
            let mut inline_modules: Vec<String> = Vec::new();
            let processed_content = self.detect_and_include_other_modules(
                &src_content,
                &src_parsed_url,
                &mut inline_modules,
                content_loader,
                0,
            );

            let mut result = String::new();
            for inline_module in &inline_modules {
                let mut hasher = Md5::new();
                hasher.update(inline_module.as_bytes());
                let module_md5 = format!("{:x}", hasher.finalize());
                if already_included.contains(&module_md5) {
                    continue;
                }
                result.push_str(&format!("<script type=\"module\">{}</script>\n", inline_module));
                already_included.insert(module_md5);
            }

            result.push_str(&format!("<script type=\"module\">{}</script>", processed_content));
            result
        } else {
            // Module not found - keep script tag but remove type="module" for offline compatibility
            format!("<script src=\"{}\"></script>", src)
        }
    }
}

impl ContentProcessor for AstroProcessor {
    fn find_urls(&self, content: &str, source_url: &ParsedUrl) -> Option<FoundUrls> {
        // Only process content containing "astro"
        if !content.contains("astro") {
            return None;
        }

        let source_url_str = source_url.get_full_url(true, false);
        let mut found_urls = FoundUrls::new();

        for caps in RE_ASTRO_URLS.captures_iter(content) {
            if let Some(m) = caps.get(2) {
                let parsed = ParsedUrl::parse(m.as_str(), Some(source_url));
                found_urls.add_url(FoundUrl::new(
                    &parsed.get_full_url(true, false),
                    &source_url_str,
                    UrlSource::JsUrl,
                ));
            }
        }

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
        // No changes needed before URL parsing in AstroProcessor
    }

    fn apply_content_changes_for_offline_version(
        &self,
        content: &mut String,
        _content_type: ContentTypeId,
        _url: &ParsedUrl,
        _remove_unwanted_code: bool,
    ) {
        // Without a content loader, we can only remove type="module" for offline compatibility.
        // Full module inlining happens in apply_content_changes_for_offline_version_with_loader.
        if !content.contains("astro") || self.config.disable_astro_inline_modules {
            return;
        }

        *content = RE_MODULE_SCRIPT_SRC_FIRST
            .replace_all(content, |caps: &regex::Captures| {
                let src = caps.get(1).map_or("", |m| m.as_str());
                format!("<script src=\"{}\"></script>", src)
            })
            .to_string();

        *content = RE_MODULE_SCRIPT_SRC_SECOND
            .replace_all(content, |caps: &regex::Captures| {
                let src = caps.get(1).map_or("", |m| m.as_str());
                format!("<script src=\"{}\"></script>", src)
            })
            .to_string();
    }

    fn apply_content_changes_for_offline_version_with_loader(
        &self,
        content: &mut String,
        _content_type: ContentTypeId,
        url: &ParsedUrl,
        _remove_unwanted_code: bool,
        content_loader: &dyn Fn(&str) -> Option<String>,
    ) {
        if !content.contains("astro") || self.config.disable_astro_inline_modules {
            return;
        }

        let mut already_included: HashSet<String> = HashSet::new();

        // Inline module scripts - pattern 1: <script type="module" src="...">
        *content = RE_MODULE_SCRIPT_SRC_FIRST
            .replace_all(content, |caps: &regex::Captures| {
                let src = caps.get(1).map_or("", |m| m.as_str());
                self.inline_module_script(src, url, &mut already_included, content_loader)
            })
            .to_string();

        // Inline module scripts - pattern 2: <script src="..." type="module">
        *content = RE_MODULE_SCRIPT_SRC_SECOND
            .replace_all(content, |caps: &regex::Captures| {
                let src = caps.get(1).map_or("", |m| m.as_str());
                self.inline_module_script(src, url, &mut already_included, content_loader)
            })
            .to_string();
    }

    fn is_content_type_relevant(&self, content_type: ContentTypeId) -> bool {
        is_relevant(content_type, &self.relevant_content_types)
    }

    fn get_name(&self) -> &str {
        "AstroProcessor"
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
    fn test_find_astro_urls() {
        let processor = AstroProcessor::new(make_config());
        let html = r#"<astro-island component-url="/_astro/TestSlider.fb32dc5a.js" component-export="default" renderer-url="/_astro/client.c4e17359.js">"#;
        let source = ParsedUrl::parse("https://example.com/page", None);
        let result = processor.find_urls(html, &source);
        assert!(result.is_some());
        assert_eq!(result.unwrap().get_count(), 2);
    }

    #[test]
    fn test_no_astro_content() {
        let processor = AstroProcessor::new(make_config());
        let html = r#"<html><body>Regular page</body></html>"#;
        let source = ParsedUrl::parse("https://example.com/page", None);
        let result = processor.find_urls(html, &source);
        assert!(result.is_none());
    }

    #[test]
    fn test_module_inlining_with_loader() {
        let processor = AstroProcessor::new(make_config());
        let mut content =
            r#"<html><head><!-- astro --><script type="module" src="/_astro/app.js"></script></head></html>"#
                .to_string();
        let url = ParsedUrl::parse("https://example.com/page", None);

        let content_loader = |url_str: &str| -> Option<String> {
            if url_str.contains("app.js") {
                Some("console.log('hello');".to_string())
            } else {
                None
            }
        };

        processor.apply_content_changes_for_offline_version_with_loader(
            &mut content,
            ContentTypeId::Html,
            &url,
            false,
            &content_loader,
        );

        // Should have inlined the module content
        assert!(content.contains("console.log('hello');"));
        // Should not have the original src attribute anymore
        assert!(!content.contains(r#"src="/_astro/app.js""#));
    }

    #[test]
    fn test_module_inlining_without_loader_falls_back() {
        let processor = AstroProcessor::new(make_config());
        let mut content =
            r#"<html><head><!-- astro --><script type="module" src="/_astro/app.js"></script></head></html>"#
                .to_string();
        let url = ParsedUrl::parse("https://example.com/page", None);

        processor.apply_content_changes_for_offline_version(&mut content, ContentTypeId::Html, &url, false);

        // Without loader, should remove type="module" but keep src
        assert!(content.contains(r#"<script src="/_astro/app.js"></script>"#));
        assert!(!content.contains("type=\"module\""));
    }
}
