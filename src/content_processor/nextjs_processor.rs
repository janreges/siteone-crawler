// SiteOne Crawler - NextJsProcessor
// (c) Jan Reges <jan.reges@siteone.cz>
//
// Handles Next.js specific URL extraction and offline conversion.

use once_cell::sync::Lazy;
use regex::Regex;

use crate::content_processor::base_processor::{ProcessorConfig, is_relevant};
use crate::content_processor::content_processor::ContentProcessor;
use crate::content_processor::html_processor::JS_VARIABLE_NAME_URL_DEPTH;
use crate::engine::found_url::UrlSource;
use crate::engine::found_urls::FoundUrls;
use crate::engine::parsed_url::ParsedUrl;
use crate::types::ContentTypeId;

static RE_MANIFEST_JS: Lazy<Regex> = Lazy::new(|| Regex::new(r#"(?is)["']([a-z0-9/._\-\[\]]\.js)["']"#).unwrap());

// Offline conversion regexes
static RE_DISABLE_PREFETCH: Lazy<Regex> = Lazy::new(|| Regex::new(r"(?i)(prefetch:\([a-z]+,[a-z]+\)=>\{)if").unwrap());

static RE_ESCAPED_NEXT: Lazy<Regex> = Lazy::new(|| Regex::new(r#"(?i)\\(["'])/_next/"#).unwrap());

static RE_ASSIGN_NEXT: Lazy<Regex> = Lazy::new(|| Regex::new(r#"(?i)([a-z0-9]+\.[a-z0-9]+=|:)(["'])/_next/"#).unwrap());

static RE_CONCAT_NEXT: Lazy<Regex> = Lazy::new(|| Regex::new(r#"(?i)(concat\([a-z]+,)(["']/_next/)(["'])"#).unwrap());

static RE_NEXT_DATA: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"(?is)<script[^>]+__NEXT_DATA__[^>]*>.*?</script>").unwrap());

static RE_PREFETCH_FUNC: Lazy<Regex> = Lazy::new(|| Regex::new(r"(?i)(prefetch\()([a-z]+)(\)\s*\{)\s*let").unwrap());

static RE_HREF_CONCAT: Lazy<Regex> = Lazy::new(|| Regex::new(r#"(?i)(\{href:)(["'])(/)(['"]\.)"#).unwrap());

static RE_PUSH_SLASH: Lazy<Regex> = Lazy::new(|| Regex::new(r#"(?i)(push\(\[)(["']/)"#).unwrap());

static RE_RETURN_QUERY: Lazy<Regex> =
    Lazy::new(|| Regex::new(r#"(?i)(return\s*["'])\s*\?[^"']+=[^"']*(["'])"#).unwrap());

static RE_NEXT_QUERY_PARAMS: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"(?i)((_next|chunks)/[a-z0-9/()\[\]._@%^{}-]+\.[a-z0-9]{1,5})\?[a-z0-9_&=.-]+").unwrap());

static RE_DPL_QUERY: Lazy<Regex> = Lazy::new(|| Regex::new(r#"(?i)\?dpl=[^"' ]+"#).unwrap());

pub struct NextJsProcessor {
    #[allow(dead_code)]
    config: ProcessorConfig,
    debug_mode: bool,
    relevant_content_types: Vec<ContentTypeId>,
}

impl NextJsProcessor {
    pub fn new(config: ProcessorConfig) -> Self {
        Self {
            config,
            debug_mode: false,
            relevant_content_types: vec![ContentTypeId::Html, ContentTypeId::Script, ContentTypeId::Stylesheet],
        }
    }
}

impl ContentProcessor for NextJsProcessor {
    fn find_urls(&self, content: &str, source_url: &ParsedUrl) -> Option<FoundUrls> {
        // Only process Next.js manifest files
        let is_nextjs_manifest =
            source_url.path.contains("_next/") && source_url.path.to_lowercase().contains("manifest");
        if !is_nextjs_manifest {
            return None;
        }

        let nextjs_base_dir = if let Some(pos) = source_url.path.find("/_next/") {
            source_url.path[..pos + 7].to_string() // include "/_next/"
        } else {
            return None;
        };

        let mut found_urls_txt: Vec<String> = Vec::new();
        for caps in RE_MANIFEST_JS.captures_iter(content) {
            if let Some(m) = caps.get(1) {
                found_urls_txt.push(format!("{}{}", nextjs_base_dir, m.as_str()));
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

    fn apply_content_changes_before_url_parsing(
        &self,
        content: &mut String,
        _content_type: ContentTypeId,
        _url: &ParsedUrl,
    ) {
        // Only process content containing _next
        if !content.to_lowercase().contains("_next") {
            return;
        }

        // Remove query params from static assets in NextJS
        *content = RE_NEXT_QUERY_PARAMS.replace_all(content, "$1").to_string();
        *content = RE_DPL_QUERY.replace_all(content, "").to_string();
    }

    fn apply_content_changes_for_offline_version(
        &self,
        content: &mut String,
        _content_type: ContentTypeId,
        url: &ParsedUrl,
        _remove_unwanted_code: bool,
    ) {
        // Only process content containing _next
        if !content.to_lowercase().contains("_next") {
            return;
        }

        // Disable prefetching in NextJS
        *content = RE_DISABLE_PREFETCH.replace_all(content, "$1 return; if").to_string();

        // Calculate depth for relative prefix
        let base_path = &url.path;
        let trimmed = base_path.trim_start_matches('/');
        let mut depth = trimmed.matches('/').count();
        let needs_index = base_path != "/" && !base_path.is_empty() && base_path.ends_with('/');
        if needs_index {
            depth += 1;
        }

        let nextjs_prefix1 = if depth > 0 {
            "../".repeat(depth)
        } else {
            "./".to_string()
        };

        // Replace escaped /_next/ paths
        *content = RE_ESCAPED_NEXT
            .replace_all(content, |caps: &regex::Captures| {
                let quote = caps.get(1).map_or("", |m| m.as_str());
                format!("\\{}{}_next/", quote, nextjs_prefix1)
            })
            .to_string();

        let nextjs_prefix2 = format!(
            "({} > 0 ? \"../\".repeat({}) : \"./\")",
            JS_VARIABLE_NAME_URL_DEPTH, JS_VARIABLE_NAME_URL_DEPTH
        );

        // Replace assignment /_next/ patterns
        *content = RE_ASSIGN_NEXT
            .replace_all(content, |caps: &regex::Captures| {
                let prefix = caps.get(1).map_or("", |m| m.as_str());
                let quote = caps.get(2).map_or("", |m| m.as_str());
                format!("{}{} + {}_next/", prefix, nextjs_prefix2, quote)
            })
            .to_string();

        // concat(e,"/_next/" -> concat(e,PREFIX+"/_next/")
        *content = RE_CONCAT_NEXT
            .replace_all(content, |caps: &regex::Captures| {
                let concat_start = caps.get(1).map_or("", |m| m.as_str());
                let next_path = caps.get(2).map_or("", |m| m.as_str());
                let end_quote = caps.get(3).map_or("", |m| m.as_str());
                format!("{}{}+{}{}", concat_start, nextjs_prefix2, next_path, end_quote)
            })
            .to_string();

        // Remove __NEXT_DATA__ script and replace with empty
        let empty_next_data =
            r#"<script id="__NEXT_DATA__" type="application/json">{"props":{"pageProps":{}}}</script>"#;
        *content = RE_NEXT_DATA.replace_all(content, empty_next_data).to_string();

        // Add prefix to prefetch function
        *content = RE_PREFETCH_FUNC
            .replace_all(content, |caps: &regex::Captures| {
                let start = caps.get(1).map_or("", |m| m.as_str());
                let arg = caps.get(2).map_or("", |m| m.as_str());
                let mid = caps.get(3).map_or("", |m| m.as_str());
                format!("{}{}{} {}={}+{}; let", start, arg, mid, arg, nextjs_prefix2, arg)
            })
            .to_string();

        // {href:"/".concat
        *content = RE_HREF_CONCAT
            .replace_all(content, |caps: &regex::Captures| {
                let start = caps.get(1).map_or("", |m| m.as_str());
                let q1 = caps.get(2).map_or("", |m| m.as_str());
                let slash = caps.get(3).map_or("", |m| m.as_str());
                let end = caps.get(4).map_or("", |m| m.as_str());
                format!("{}{}+{}{}{}", start, nextjs_prefix2, q1, slash, end)
            })
            .to_string();

        // push(["/
        *content = RE_PUSH_SLASH
            .replace_all(content, |caps: &regex::Captures| {
                let start = caps.get(1).map_or("", |m| m.as_str());
                let path = caps.get(2).map_or("", |m| m.as_str());
                format!("{}{}+{}", start, nextjs_prefix2, path)
            })
            .to_string();

        // return"?dpl=..." -> return""
        *content = RE_RETURN_QUERY.replace_all(content, "$1$2").to_string();

        // Remove query params from _next/static/ paths
        *content = RE_NEXT_QUERY_PARAMS.replace_all(content, "$1").to_string();
        *content = RE_DPL_QUERY.replace_all(content, "").to_string();
    }

    fn is_content_type_relevant(&self, content_type: ContentTypeId) -> bool {
        is_relevant(content_type, &self.relevant_content_types)
    }

    fn get_name(&self) -> &str {
        "NextJsProcessor"
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
    fn test_non_manifest_returns_none() {
        let processor = NextJsProcessor::new(make_config());
        let content = r#"some javascript content"#;
        let source = ParsedUrl::parse("https://example.com/app.js", None);
        let result = processor.find_urls(content, &source);
        assert!(result.is_none());
    }

    #[test]
    fn test_before_url_parsing_removes_dpl() {
        let processor = NextJsProcessor::new(make_config());
        let mut content = r#"/_next/static/css/file.css?dpl=dpl_abc123"#.to_string();
        let source = ParsedUrl::parse("https://example.com/page", None);
        processor.apply_content_changes_before_url_parsing(&mut content, ContentTypeId::Html, &source);
        assert!(!content.contains("?dpl="));
    }
}
