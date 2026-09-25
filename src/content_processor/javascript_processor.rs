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

/// The VuePress client bootstrap: `window.__VUEPRESS__={…}` (1.x) or `window.__VUEPRESS_VERSION__={…}`
/// (0.x).
static RE_VUEPRESS_BOOTSTRAP: Lazy<Regex> = Lazy::new(|| Regex::new(r"__VUEPRESS(?:_VERSION)?__\s*=\s*\{").unwrap());

/// A VuePress 1.x or 0.x client bundle, whose `path:"/…"` keys are page and sidebar links (#62). Only a
/// script that runs the bootstrap counts: VuePress sets the marker in its client bundle, never in a page,
/// so an HTML page or a bundle that merely mentions or reads the marker is not one, and neither is one
/// that shows the bootstrap in a comment, a string or a template (the bootstrap must be code that starts
/// a statement).
fn is_vuepress_bundle(content: &str, content_type: ContentTypeId) -> bool {
    if content_type != ContentTypeId::Script {
        return false;
    }
    let bootstraps: Vec<usize> = RE_VUEPRESS_BOOTSTRAP
        .find_iter(content)
        .map(|bootstrap| bootstrap.start())
        .filter(|&start| starts_statement(&content[..start]))
        .collect();
    if bootstraps.is_empty() {
        return false;
    }
    // A script the scan loses track of (it ends inside a comment, string or template) is judged by the
    // statement boundary alone.
    in_js_code(content, &bootstraps).is_none_or(|in_code| in_code.contains(&true))
}

/// Where the lexical scan of JavaScript is: in code, or in text that does not run.
#[derive(Clone, Copy, PartialEq, Eq)]
enum JsContext {
    Code,
    LineComment,
    BlockComment,
    Quoted(u8),
    Template,
    Regex { in_class: bool },
}

/// For each of the ascending byte `offsets` in `js`: is it in code, not in a comment, a string, the text
/// of a template or a regular expression? None when the scan does not end in code, i.e. it lost track
/// of the source. A `/` starts a regular expression where a value may start (after an operator, an
/// opening bracket or a keyword such as `return`), otherwise it divides.
fn in_js_code(js: &str, offsets: &[usize]) -> Option<Vec<bool>> {
    const REGEX_KEYWORDS: &[&str] = &[
        "return",
        "typeof",
        "instanceof",
        "in",
        "of",
        "new",
        "delete",
        "void",
        "throw",
        "case",
        "do",
        "else",
        "yield",
        "await",
    ];
    let bytes = js.as_bytes();
    let mut in_code = Vec::with_capacity(offsets.len());
    let mut context = JsContext::Code;
    // The brace depth at each open `${` of a template, and the current depth
    let mut templates: Vec<usize> = Vec::new();
    let mut braces = 0usize;
    // The last token in code: a punctuator, a value (`"` for a string, template or regular expression,
    // `a` for a word) and the word itself
    let mut last: Option<u8> = None;
    let mut word = String::new();
    let mut in_word = false;
    let mut i = 0;
    while i < bytes.len() {
        while in_code.len() < offsets.len() && offsets[in_code.len()] <= i {
            in_code.push(offsets[in_code.len()] == i && context == JsContext::Code);
        }
        let c = bytes[i];
        let next = bytes.get(i + 1).copied();
        let mut step = 1;
        match context {
            JsContext::Code => {
                let is_word = c.is_ascii_alphanumeric() || c == b'_' || c == b'$' || c >= 0x80;
                if is_word {
                    if !in_word {
                        word.clear();
                    }
                    word.push(c as char);
                    last = Some(b'a');
                } else {
                    match c {
                        b' ' | b'\t' | b'\n' | b'\r' => {}
                        b'/' if next == Some(b'/') => context = JsContext::LineComment,
                        b'/' if next == Some(b'*') => {
                            context = JsContext::BlockComment;
                            step = 2;
                        }
                        b'/' => {
                            let starts_value = match last {
                                None => true,
                                Some(b'a') => REGEX_KEYWORDS.contains(&word.as_str()),
                                Some(b'"') | Some(b')') | Some(b']') => false,
                                Some(_) => true,
                            };
                            if starts_value {
                                context = JsContext::Regex { in_class: false };
                            } else {
                                last = Some(b'/');
                            }
                        }
                        b'\'' | b'"' => context = JsContext::Quoted(c),
                        b'`' => context = JsContext::Template,
                        b'{' => {
                            braces += 1;
                            last = Some(c);
                        }
                        b'}' if templates.last() == Some(&braces) => {
                            templates.pop();
                            context = JsContext::Template;
                        }
                        b'}' => {
                            braces = braces.saturating_sub(1);
                            last = Some(c);
                        }
                        _ => last = Some(c),
                    }
                }
                in_word = is_word;
            }
            JsContext::LineComment => {
                if c == b'\n' {
                    context = JsContext::Code;
                }
            }
            JsContext::BlockComment => {
                if c == b'*' && next == Some(b'/') {
                    context = JsContext::Code;
                    step = 2;
                }
            }
            JsContext::Quoted(quote) => match c {
                b'\\' => step = 2,
                // An unescaped line break ends a broken string too, so the scan recovers
                b'\n' => {
                    context = JsContext::Code;
                    last = Some(b'"');
                }
                _ if c == quote => {
                    context = JsContext::Code;
                    last = Some(b'"');
                }
                _ => {}
            },
            JsContext::Template => match c {
                b'\\' => step = 2,
                b'`' => {
                    context = JsContext::Code;
                    last = Some(b'"');
                }
                b'$' if next == Some(b'{') => {
                    templates.push(braces);
                    context = JsContext::Code;
                    last = Some(b'{');
                    step = 2;
                }
                _ => {}
            },
            JsContext::Regex { in_class } => match c {
                b'\\' => step = 2,
                b'[' => context = JsContext::Regex { in_class: true },
                b']' => context = JsContext::Regex { in_class: false },
                b'/' if !in_class => {
                    context = JsContext::Code;
                    last = Some(b'"');
                }
                b'\n' => {
                    context = JsContext::Code;
                    last = Some(b'"');
                }
                _ => {}
            },
        }
        if context != JsContext::Code {
            in_word = false;
        }
        i += step;
    }
    while in_code.len() < offsets.len() {
        in_code.push(context == JsContext::Code);
    }
    (context == JsContext::Code && templates.is_empty()).then_some(in_code)
}

/// Does code that continues after `before` start a statement (or an expression within one)? That is
/// after an optional `window.`, at the start of the script, on a new line, after a comment, or after
/// a punctuator such as `;`, `,`, `{`, `(` or `=`.
fn starts_statement(before: &str) -> bool {
    let before = before.strip_suffix("window.").unwrap_or(before);
    let trimmed = before.trim_end_matches([' ', '\t']);
    if trimmed.ends_with("*/") {
        return true;
    }
    match trimmed.chars().last() {
        None | Some('\n' | '\r') => true,
        Some(c) => ";,{}()=&|?!".contains(c),
    }
}

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
        content_type: ContentTypeId,
        _url: &ParsedUrl,
        _remove_unwanted_code: bool,
    ) {
        // Replace crossorigin keyword (case-insensitive)
        if RE_CROSSORIGIN.is_match(content) {
            *content = RE_CROSSORIGIN.replace_all(content, "_SiteOne_CO_").to_string();
        }

        // When preserving URLs or disabling URL rewriting, skip webpack path transformations
        if self.config.offline_export_preserve_urls || self.config.offline_export_no_url_rewriting {
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
        // path/Path keys become href only in VuePress bundles, where they are sidebar links (PHP
        // commit 9bea99b); elsewhere they are route definitions, e.g. React Router's (#62)
        if is_vuepress_bundle(content, content_type) {
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

    #[test]
    fn vuepress_bundle_path_keys_point_to_offline_files() {
        // VuePress 1.x (window.__VUEPRESS__) and 0.x (window.__VUEPRESS_VERSION__) bundles keep the
        // rewrite of their page/sidebar `path` and `regularPath` keys (PHP commit 9bea99b).
        let processor = JavaScriptProcessor::new(make_config());
        let url = ParsedUrl::parse("https://example.com/assets/js/app.f00cc16f.js", None);
        for marker in [
            r#"window.__VUEPRESS__={version:"1.9.9",hash:"de4f7cf8"};"#,
            r#"window.__VUEPRESS_VERSION__={version:"0.14.11",hash:"de4f7cf8"};"#,
        ] {
            let mut js = format!(
                r#"{marker}const Es=[{{name:"v-27315abc",path:"/guide/",component:x}}];var s={{pages:[{{title:"Guide",regularPath:"/guide/"}}]}};"#
            );
            processor.apply_content_changes_for_offline_version(&mut js, ContentTypeId::Script, &url, false);
            assert!(
                js.contains(
                    r#"{name:"v-27315abc",href:(_SiteOneUrlDepth > 0 ? "../".repeat(_SiteOneUrlDepth) : "./")+"guide/""#
                ),
                "{js}"
            );
            assert!(
                js.contains(r#"regularhref:(_SiteOneUrlDepth > 0 ? "../".repeat(_SiteOneUrlDepth) : "./")+"guide/""#),
                "{js}"
            );
        }
    }

    #[test]
    fn spa_route_table_path_keys_are_kept() {
        // #62: outside VuePress, `path:"/…"` keys are route definitions (here React Router's, from a
        // Lovable-built SPA); renaming them to `href` breaks the offline copy.
        let processor = JavaScriptProcessor::new(make_config());
        let url = ParsedUrl::parse("https://example.com/assets/index-B0occCfy.js", None);
        let original = r#"l.jsx(kW,{children:l.jsxs(wW,{children:[l.jsx(vc,{path:"/",element:l.jsx(TSe,{})}),l.jsx(vc,{path:"/auth",element:l.jsx(dje,{})}),l.jsx(vc,{path:"*",element:l.jsx(lje,{})})]})})"#;
        let mut js = original.to_string();
        processor.apply_content_changes_for_offline_version(&mut js, ContentTypeId::Script, &url, false);
        assert_eq!(js, original);
    }

    #[test]
    fn html_mentioning_the_vuepress_marker_keeps_its_route_paths() {
        // #62: a page that only writes about the VuePress marker (in prose or a code sample) is not
        // a VuePress bundle; its inline route table keeps its `path` keys.
        let processor = JavaScriptProcessor::new(make_config());
        let url = ParsedUrl::parse("https://example.com/marker-text/", None);
        for mention in [
            "<p>VuePress detection checks __VUEPRESS__.</p>",
            r#"<pre><code>window.__VUEPRESS__={version:"1.9.9",hash:"de4f7cf8"};</code></pre>"#,
        ] {
            let original = format!(
                r#"<!doctype html><html><head><title>React docs site</title></head><body>{mention}<div id="app">Initial</div><script>const routes=[{{path:"/marker-text/",name:"Working route"}}];</script></body></html>"#
            );
            let mut html = original.clone();
            processor.apply_content_changes_for_offline_version(&mut html, ContentTypeId::Html, &url, false);
            assert_eq!(html, original);
        }
    }

    #[test]
    fn bundle_reading_the_vuepress_marker_keeps_its_route_paths() {
        // #62: a bundle that only reads the marker (e.g. to detect VuePress) does not set it, so it
        // is not a VuePress client bundle.
        let processor = JavaScriptProcessor::new(make_config());
        let url = ParsedUrl::parse("https://example.com/assets/index-B0occCfy.js", None);
        let original = r#"var isVuePress=!!window.__VUEPRESS__;l.jsx(vc,{path:"/auth",element:l.jsx(dje,{})})"#;
        let mut js = original.to_string();
        processor.apply_content_changes_for_offline_version(&mut js, ContentTypeId::Script, &url, false);
        assert_eq!(js, original);
    }

    #[test]
    fn bundle_quoting_the_vuepress_bootstrap_keeps_its_route_paths() {
        // #62: a bundle that shows the bootstrap in a string or a comment (e.g. a docs page compiled
        // into JS) does not run it, so it is not a VuePress client bundle.
        let processor = JavaScriptProcessor::new(make_config());
        let url = ParsedUrl::parse("https://example.com/js/docs.js", None);
        for example in [
            r#"const example='window.__VUEPRESS__={version:"1.9.9"}';"#,
            r#"const example="__VUEPRESS_VERSION__ = {version:'0.14.11'}";"#,
            r#"/* Example: window.__VUEPRESS__={version:"1.9.9"} */"#,
            "// window.__VUEPRESS__={version:\"1.9.9\"}\n",
            // a line break or a punctuator inside a comment, a template or a string
            "/* VuePress bootstrap example:\nwindow.__VUEPRESS__={version:\"1.9.9\"}\n*/\n",
            "const example=`VuePress bootstrap example:\nwindow.__VUEPRESS__={version:\"1.9.9\"}`;",
            r#"const example='Example: ;window.__VUEPRESS__={version:"1.9.9"}';"#,
            // a regular expression holding a quote does not start a string
            r#"var q=/'/g;/* window.__VUEPRESS__={version:"1.9.9"} */"#,
        ] {
            let original = format!(r#"{example}const routes=[{{path:"/marker-text/",name:"Working route"}}];"#);
            let mut js = original.clone();
            processor.apply_content_changes_for_offline_version(&mut js, ContentTypeId::Script, &url, false);
            assert_eq!(js, original);
        }
    }

    #[test]
    fn vuepress_bootstrap_after_a_statement_boundary_is_recognized() {
        // The bootstrap as bundlers emit it: after `;`, `,`, `{`, a comment or a line break.
        let processor = JavaScriptProcessor::new(make_config());
        let url = ParsedUrl::parse("https://example.com/assets/js/app.f00cc16f.js", None);
        for prefix in [
            "var a=1;",
            "function(e,t,n){",
            "n.r(t),",
            "/* harmony import */ ",
            "import x from 'y'\n\n",
            // strings, templates and regular expressions before it are skipped whole
            r#"var s="a;b",r=/["'`]/g,t=`x${"}"}y`,d=4/2/1;"#,
        ] {
            let mut js = format!(
                r#"{prefix}window.__VUEPRESS__ = {{version:"1.9.9",hash:"de4f7cf8"}};const Es=[{{name:"v-1",path:"/guide/"}}];"#
            );
            processor.apply_content_changes_for_offline_version(&mut js, ContentTypeId::Script, &url, false);
            assert!(js.contains(r#"{name:"v-1",href:"#), "{prefix}: {js}");
        }
    }
}
