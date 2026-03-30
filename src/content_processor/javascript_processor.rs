// SiteOne Crawler - JavaScriptProcessor
// (c) Jan Reges <jan.reges@siteone.cz>
//
// Extracts URLs from JS import/from statements and applies offline conversion.

use once_cell::sync::Lazy;
use regex::Regex;

use crate::content_processor::base_processor::{ProcessorConfig, is_relevant};
use crate::content_processor::content_processor::ContentProcessor;
use crate::content_processor::html_processor::JS_VARIABLE_NAME_URL_DEPTH;
use crate::engine::found_url::UrlSource;
use crate::engine::found_urls::FoundUrls;
use crate::engine::parsed_url::ParsedUrl;
use crate::types::ContentTypeId;

static RE_IMPORT_FROM: Lazy<Regex> = Lazy::new(|| Regex::new(r#"(?i)from\s*["']([^"']+\.js[^"']*)["']"#).unwrap());

static RE_QUOTED_JS_PATH: Lazy<Regex> = Lazy::new(|| Regex::new(r#"(?i)["'](/[^"']+\.js)["']"#).unwrap());

static RE_QUOTED_HTTPS_JS: Lazy<Regex> = Lazy::new(|| Regex::new(r#"(?i)["'](https://[^"']+\.js)["']"#).unwrap());

static RE_WEBPACK_CHUNKS: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r#"(?i)"assets/js/".*\+.*\(\{([^}]*)\}.*\[e\].*\|\|.*e\)\s*\+\s*"\.".*\+\s*\{([^}]+)\}"#).unwrap()
});

static RE_WEBPACK_NAME_ITEM: Lazy<Regex> = Lazy::new(|| Regex::new(r#"([0-9]+):\s*"([^"']+)""#).unwrap());

static RE_WEBPACK_HASH_ITEM: Lazy<Regex> = Lazy::new(|| Regex::new(r#"([0-9]+):\s*"([a-f0-9]+)""#).unwrap());

// Offline conversion regexes
static RE_WEBPACK_AP: Lazy<Regex> = Lazy::new(|| Regex::new(r#"a\.p="/""#).unwrap());

static RE_HREF_SLASH: Lazy<Regex> = Lazy::new(|| Regex::new(r#"href:"/"#).unwrap());

static RE_PATH_SLASH: Lazy<Regex> = Lazy::new(|| Regex::new(r#"path:"/"#).unwrap());

static RE_PATH_UPPER_SLASH: Lazy<Regex> = Lazy::new(|| Regex::new(r#"Path:"/"#).unwrap());

static RE_CROSSORIGIN: Lazy<Regex> = Lazy::new(|| Regex::new(r"(?i)crossorigin").unwrap());

pub struct JavaScriptProcessor {
    #[allow(dead_code)]
    config: ProcessorConfig,
    debug_mode: bool,
    relevant_content_types: Vec<ContentTypeId>,
}

impl JavaScriptProcessor {
    pub fn new(config: ProcessorConfig) -> Self {
        Self {
            config,
            debug_mode: false,
            relevant_content_types: vec![ContentTypeId::Html, ContentTypeId::Script],
        }
    }

    /// Find URLs in JavaScript import from statements and quoted JS paths
    fn find_urls_import_from(&self, content: &str, source_url: &ParsedUrl) -> Option<FoundUrls> {
        // Don't process HTML files
        if content.to_lowercase().contains("<html") {
            return None;
        }
        if !content.contains("from") {
            return None;
        }

        let mut found_urls_txt: Vec<String> = Vec::new();

        // import ... from "path.js"
        for caps in RE_IMPORT_FROM.captures_iter(content) {
            if let Some(m) = caps.get(1) {
                found_urls_txt.push(m.as_str().trim().to_string());
            }
        }

        // "/assets/js/12.c6446aa6.js" style paths
        for caps in RE_QUOTED_JS_PATH.captures_iter(content) {
            if let Some(m) = caps.get(1) {
                found_urls_txt.push(m.as_str().trim().to_string());
            }
        }

        // "https://..." style JS URLs
        for caps in RE_QUOTED_HTTPS_JS.captures_iter(content) {
            if let Some(m) = caps.get(1) {
                found_urls_txt.push(m.as_str().trim().to_string());
            }
        }

        // Webpack chunks pattern
        if let Some(caps) = RE_WEBPACK_CHUNKS.captures(content) {
            let mut tmp_webpack: std::collections::HashMap<String, String> = std::collections::HashMap::new();

            // Parse name mappings: {5:"vendors~docsearch"}
            if let Some(names_str) = caps.get(1) {
                for item in names_str.as_str().split(',') {
                    if let Some(item_caps) = RE_WEBPACK_NAME_ITEM.captures(item) {
                        let id = item_caps.get(1).map_or("", |m| m.as_str()).to_string();
                        let name = item_caps.get(2).map_or("", |m| m.as_str()).to_string();
                        tmp_webpack.insert(id, name);
                    }
                }
            }

            // Parse hash mappings and build URLs
            if let Some(hashes_str) = caps.get(2) {
                for item in hashes_str.as_str().split(',') {
                    if let Some(item_caps) = RE_WEBPACK_HASH_ITEM.captures(item) {
                        let id = item_caps.get(1).map_or("", |m| m.as_str());
                        let hash = item_caps.get(2).map_or("", |m| m.as_str());

                        found_urls_txt.push(format!("/assets/js/{}.{}.js", id, hash));

                        // Special case: named webpack chunks
                        if let Some(name) = tmp_webpack.get(id) {
                            found_urls_txt.push(format!("/assets/js/{}.{}.js", name, hash));
                        }
                    }
                }
            }
        }

        if found_urls_txt.is_empty() {
            return None;
        }

        let mut found_urls = FoundUrls::new();
        let url_refs: Vec<&str> = found_urls_txt.iter().map(|s| s.as_str()).collect();
        found_urls.add_urls_from_text_array(&url_refs, &source_url.path, UrlSource::JsUrl);

        if found_urls.get_count() > 0 {
            Some(found_urls)
        } else {
            None
        }
    }
}

impl ContentProcessor for JavaScriptProcessor {
    fn find_urls(&self, content: &str, source_url: &ParsedUrl) -> Option<FoundUrls> {
        self.find_urls_import_from(content, source_url)
    }

    fn apply_content_changes_before_url_parsing(
        &self,
        _content: &mut String,
        _content_type: ContentTypeId,
        _url: &ParsedUrl,
    ) {
        // No changes needed before URL parsing in JavaScriptProcessor
    }

    fn apply_content_changes_for_offline_version(
        &self,
        content: &mut String,
        _content_type: ContentTypeId,
        _url: &ParsedUrl,
        _remove_unwanted_code: bool,
    ) {
        // Replace crossorigin keyword (case-insensitive)
        if RE_CROSSORIGIN.is_match(content) {
            *content = RE_CROSSORIGIN.replace_all(content, "_SiteOne_CO_").to_string();
        }

        // When preserving URLs, skip webpack path transformations since paths remain root-relative
        if self.config.offline_export_preserve_urls {
            return;
        }

        let webpack_path_prefix = format!(
            "({} > 0 ? \"../\".repeat({}) : \"./\")",
            JS_VARIABLE_NAME_URL_DEPTH, JS_VARIABLE_NAME_URL_DEPTH
        );

        // webpack case: a.p="/"
        if content.to_lowercase().contains("a.p=") {
            *content = RE_WEBPACK_AP
                .replace_all(content, &format!("a.p={}", webpack_path_prefix))
                .to_string();
        }

        // webpack href/path/Path cases
        if content.to_lowercase().contains("href:\"/") {
            *content = RE_HREF_SLASH
                .replace_all(content, &format!("href:{}+\"", webpack_path_prefix))
                .to_string();
        }
        if content.to_lowercase().contains("path:\"/") {
            *content = RE_PATH_SLASH
                .replace_all(content, &format!("href:{}+\"", webpack_path_prefix))
                .to_string();
        }
        if content.contains("Path:\"/") {
            *content = RE_PATH_UPPER_SLASH
                .replace_all(content, &format!("href:{}+\"", webpack_path_prefix))
                .to_string();
        }
    }

    fn is_content_type_relevant(&self, content_type: ContentTypeId) -> bool {
        is_relevant(content_type, &self.relevant_content_types)
    }

    fn get_name(&self) -> &str {
        "JavaScriptProcessor"
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
    fn test_find_import_from() {
        let processor = JavaScriptProcessor::new(make_config());
        let js = r#"import{R as W}from"./Repl.209fef3e.js";import{s}from"./stores.js";"#;
        let source = ParsedUrl::parse("https://example.com/app.js", None);
        let result = processor.find_urls(js, &source);
        assert!(result.is_some());
        assert!(result.unwrap().get_count() >= 2);
    }

    #[test]
    fn test_skip_html_content() {
        let processor = JavaScriptProcessor::new(make_config());
        let html = r#"<html><head></head><body>from something</body></html>"#;
        let source = ParsedUrl::parse("https://example.com/page.html", None);
        let result = processor.find_urls(html, &source);
        assert!(result.is_none());
    }

    #[test]
    fn test_find_quoted_js_paths() {
        let processor = JavaScriptProcessor::new(make_config());
        let js = r#"from x; var chunks = ["/assets/js/12.c6446aa6.js","/assets/js/120.03870a87.js"]"#;
        let source = ParsedUrl::parse("https://example.com/bundle.js", None);
        let result = processor.find_urls(js, &source);
        assert!(result.is_some());
    }
}
