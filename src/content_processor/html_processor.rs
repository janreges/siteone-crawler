// SiteOne Crawler - HtmlProcessor
// (c) Jan Reges <jan.reges@siteone.cz>
//
// Extracts URLs from HTML content and applies offline conversion changes.

use once_cell::sync::Lazy;
use regex::Regex;

use crate::content_processor::base_processor::{ProcessorConfig, convert_url_to_relative, is_relevant};
use crate::content_processor::content_processor::ContentProcessor;
use crate::engine::found_url::UrlSource;
use crate::engine::found_urls::FoundUrls;
use crate::engine::parsed_url::ParsedUrl;
use crate::types::ContentTypeId;
use crate::utils;

pub const JS_VARIABLE_NAME_URL_DEPTH: &str = "_SiteOneUrlDepth";

pub const HTML_PAGES_EXTENSIONS: &[&str] = &[
    "htm", "html", "shtml", "php", "phtml", "ashx", "xhtml", "asp", "aspx", "jsp", "jspx", "do", "cfm", "cgi", "pl",
];

static HTML_EXT_REGEX: Lazy<Regex> = Lazy::new(|| {
    let pattern = format!(r"(?i)\.({})", HTML_PAGES_EXTENSIONS.join("|"));
    Regex::new(&pattern).unwrap()
});

static RE_A_HREF: Lazy<Regex> =
    Lazy::new(|| Regex::new(r#"(?is)<a[^>]*\shref=["']?([^#][^"'\s>]+)["'\s]?[^>]*>"#).unwrap());

static RE_ESCAPED_HREF: Lazy<Regex> =
    Lazy::new(|| Regex::new(r#"(?i)href\\["'][:=]\\["'](https?://[^"'\\]+)\\["']"#).unwrap());

static RE_FONT_URL: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r#"(?is)url\s*\(\s*['"]?([^'"\s>]+\.(eot|ttf|woff2|woff|otf)[^'")\s]*)['"]?\s*\)"#).unwrap()
});

static RE_FONT_LINK: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r#"(?is)<link\s+[^>]*href=["']?([^"' ]+\.(eot|ttf|woff2|woff|otf)[^"' ]*)["']?[^>]*>"#).unwrap()
});

static RE_IMG_SRC: Lazy<Regex> =
    Lazy::new(|| Regex::new(r#"(?is)<img\s+[^>]*?src=["']?([^"'> ]+)["']?[^>]*>"#).unwrap());

static RE_IMG_DATA_SRC: Lazy<Regex> =
    Lazy::new(|| Regex::new(r#"(?is)<img\s+[^>]*?data-src=["']?([^"'> ]+)["']?[^>]*>"#).unwrap());

static RE_INPUT_SRC: Lazy<Regex> =
    Lazy::new(|| Regex::new(r#"(?is)<input\s+[^>]*?src=["']?([^"'> ]+\.[a-z0-9]{1,10})["']?[^>]*>"#).unwrap());

static RE_LINK_IMAGE: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r#"(?is)<link\s+[^>]*?href=["']?([^"'> ]+\.(png|gif|jpg|jpeg|webp|avif|tif|bmp|svg|ico)(\?[^"' ]*|))["']?[^>]*>"#).unwrap()
});

static RE_SOURCE_SRC: Lazy<Regex> =
    Lazy::new(|| Regex::new(r#"(?is)<source\s+[^>]*?src=["']([^"'>]+)["'][^>]*>"#).unwrap());

static RE_CSS_URL_IMAGE: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r#"(?is)url\s*\(\s*['"]?([^'")\s]+\.(jpg|jpeg|png|gif|bmp|tif|webp|avif)[^'")\s]*)['"]?\s*\)"#).unwrap()
});

static RE_SOURCE_SRCSET: Lazy<Regex> =
    Lazy::new(|| Regex::new(r#"(?is)<source\s+[^>]*?srcset=["']([^"'>]+)["'][^>]*>"#).unwrap());

static RE_IMG_SRCSET: Lazy<Regex> = Lazy::new(|| Regex::new(r#"(?is)<img[^>]+srcset=["']([^"']+)["']"#).unwrap());

static RE_IMAGESRCSET: Lazy<Regex> =
    Lazy::new(|| Regex::new(r#"(?is)<[a-z]+[^>]+imagesrcset=["']([^"']+)["']"#).unwrap());

static RE_AUDIO_SRC: Lazy<Regex> =
    Lazy::new(|| Regex::new(r#"(?is)<audio\s+[^>]*?src=["']?([^"'> ]+)["']?[^>]*>"#).unwrap());

static RE_VIDEO_SRC: Lazy<Regex> =
    Lazy::new(|| Regex::new(r#"(?is)<video\s+[^>]*?src=["']?([^"'> ]+)["']?[^>]*>"#).unwrap());

static RE_SCRIPT_SRC: Lazy<Regex> =
    Lazy::new(|| Regex::new(r#"(?is)<script\s+[^>]*?src=["']?([^"' ]+)["']?[^>]*>"#).unwrap());

static RE_LINK_JS: Lazy<Regex> =
    Lazy::new(|| Regex::new(r#"(?is)<link\s+[^>]*href=["']?([^"'> ]+\.(json|js)(\?[^"']*|))["']?[^>]*>"#).unwrap());

static RE_DOT_SRC: Lazy<Regex> = Lazy::new(|| Regex::new(r#"(?is)\.src\s*=\s*["']([^"']+)["']"#).unwrap());

static RE_NEXTJS_CHUNKS: Lazy<Regex> =
    Lazy::new(|| Regex::new(r#"(?is):([a-z0-9/._\-\[\]]+chunks[a-z0-9/._\-\[\]]+\.js)"#).unwrap());

static RE_LINK_STYLESHEET: Lazy<Regex> =
    Lazy::new(|| Regex::new(r#"(?is)<link\s+[^>]*?href=["']([^"']+)["'][^>]*>"#).unwrap());

static RE_FILE_EXTENSION: Lazy<Regex> = Lazy::new(|| Regex::new(r"(?i)\.[a-z0-9]{1,10}(\?.*)?$").unwrap());

// Offline version regexes
static RE_HREF_SRC: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r#"(?is)(\.|<[a-z0-9]{1,10}[^>]*\s+)(href|src|component-url)\s*(=)\s*(['"]?)([^'">]+)['"]?([^>]*)"#)
        .unwrap()
});

static RE_SRCSET_ATTR: Lazy<Regex> = Lazy::new(|| {
    Regex::new(
        r#"(?is)(\.|<[a-z0-9]{1,10}[^>]*\s+)(imagesrcset|srcset|renderer-url)\s*(=)\s*(['"]?)([^'">]+)['"]?([^>]*)"#,
    )
    .unwrap()
});

static RE_META_URL: Lazy<Regex> =
    Lazy::new(|| Regex::new(r#"(?im)(<meta[^>]*)(url)\s*(=)\s*(['"]?)([^'">]+)['"]?(")"#).unwrap());

static RE_ESCAPED_HREF_SRC: Lazy<Regex> =
    Lazy::new(|| Regex::new(r#"(?is)(.)(href\\["']|src\\["'])([:=])(\\["'])([^"'\\]+)\\["'](.)"#).unwrap());

static RE_META_REFRESH: Lazy<Regex> = Lazy::new(|| Regex::new(r#"(?i)(<meta[^>]*url=)([^"']+)(["'][^>]*>)"#).unwrap());

static RE_PORT_NORMALIZE: Lazy<Regex> = Lazy::new(|| Regex::new(r"(?i)((https?:)?//[a-z0-9._-]+):[0-9]+").unwrap());

static RE_CLOSE_HEAD: Lazy<Regex> = Lazy::new(|| Regex::new(r"(?i)<\s*/\s*head\s*>").unwrap());

static RE_CLOSE_BODY: Lazy<Regex> = Lazy::new(|| Regex::new(r"(?i)<\s*/\s*body\s*>").unwrap());

static RE_NON_HTTP_SCHEME: Lazy<Regex> = Lazy::new(|| Regex::new(r"(?i)^[a-z]+:[a-z0-9+]").unwrap());

static RE_EXTERNAL_SCRIPT: Lazy<Regex> =
    Lazy::new(|| Regex::new(r#"(?is)<script[^>]*src=["']?(.*?)["']?[^>]*>.*?</script>"#).unwrap());

static RE_EXTERNAL_URL: Lazy<Regex> = Lazy::new(|| Regex::new(r"(?i)^(https?:)?//").unwrap());

static RE_CROSSORIGIN_LINK: Lazy<Regex> =
    Lazy::new(|| Regex::new(r#"(?i)(<link[^>]+)\s*crossorigin(\s*=\s*["']?.*?["']?)?(\s*[^>]*>)"#).unwrap());

static RE_CROSSORIGIN_SCRIPT: Lazy<Regex> =
    Lazy::new(|| Regex::new(r#"(?i)(<script[^>]+)\s*crossorigin(\s*=\s*["']?.*?["']?)?(\s*[^>]*>)"#).unwrap());

static RE_SCRIPT_BLOCK: Lazy<Regex> = Lazy::new(|| Regex::new(r"(?is)<script[^>]*>(.*?)</script>").unwrap());

static RE_SOCNET_IFRAME: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"(?is)<iframe[^>]*(facebook\.com|twitter\.com|linkedin\.com)[^>]*>.*?</iframe>").unwrap());

pub struct HtmlProcessor {
    config: ProcessorConfig,
    debug_mode: bool,
    relevant_content_types: Vec<ContentTypeId>,
}

impl HtmlProcessor {
    pub fn new(config: ProcessorConfig) -> Self {
        Self {
            config,
            debug_mode: false,
            relevant_content_types: vec![ContentTypeId::Html, ContentTypeId::Redirect],
        }
    }

    /// Find <a href> URLs
    fn find_href_urls(&self, html: &str, source_url: &ParsedUrl, found_urls: &mut FoundUrls) {
        let source_url_str = source_url.get_full_url(true, false);

        // Standard <a href="..."> links
        let mut urls: Vec<String> = Vec::new();
        for caps in RE_A_HREF.captures_iter(html) {
            if let Some(m) = caps.get(1) {
                urls.push(m.as_str().to_string());
            }
        }

        // Escaped href URLs (e.g., href\":\")
        for caps in RE_ESCAPED_HREF.captures_iter(html) {
            if let Some(m) = caps.get(1) {
                urls.push(m.as_str().to_string());
            }
        }

        // If single_foreign_page is set and source is on a different 2nd-level domain, skip
        if self.config.single_foreign_page && source_url.domain_2nd_level != self.config.initial_url.domain_2nd_level {
            return;
        }

        // Filter by max depth
        if self.config.max_depth > 0 {
            urls.retain(|url_str| {
                let parsed = ParsedUrl::parse(url_str, Some(source_url));
                parsed.get_depth() <= self.config.max_depth as usize
            });
        }

        // Filter out files if files are disabled
        if !self.config.files_enabled {
            urls.retain(|url_str| !RE_FILE_EXTENSION.is_match(url_str) || HTML_EXT_REGEX.is_match(url_str));
        }

        let url_refs: Vec<&str> = urls.iter().map(|s| s.as_str()).collect();
        found_urls.add_urls_from_text_array(&url_refs, &source_url_str, UrlSource::AHref);
    }

    /// Find font URLs in CSS and link tags
    fn find_fonts(&self, html: &str, source_url: &ParsedUrl, found_urls: &mut FoundUrls) {
        let source_url_str = source_url.get_full_url(true, false);

        // CSS @font-face url()
        let font_urls: Vec<&str> = RE_FONT_URL
            .captures_iter(html)
            .filter_map(|caps| caps.get(1).map(|m| m.as_str()))
            .collect();
        found_urls.add_urls_from_text_array(&font_urls, &source_url_str, UrlSource::CssUrl);

        // <link href="...(font extensions)"
        let link_fonts: Vec<&str> = RE_FONT_LINK
            .captures_iter(html)
            .filter_map(|caps| caps.get(1).map(|m| m.as_str()))
            .collect();
        found_urls.add_urls_from_text_array(&link_fonts, &source_url_str, UrlSource::LinkHref);
    }

    /// Find image URLs from various sources
    fn find_images(&self, html: &str, source_url: &ParsedUrl, found_urls: &mut FoundUrls) {
        let source_url_str = source_url.get_full_url(true, false);

        // <img src="..."
        let img_srcs: Vec<&str> = RE_IMG_SRC
            .captures_iter(html)
            .filter_map(|caps| caps.get(1).map(|m| m.as_str()))
            .collect();
        found_urls.add_urls_from_text_array(&img_srcs, &source_url_str, UrlSource::ImgSrc);

        // <img data-src="..." (lazy loading)
        let data_srcs: Vec<&str> = RE_IMG_DATA_SRC
            .captures_iter(html)
            .filter_map(|caps| caps.get(1).map(|m| m.as_str()))
            .collect();
        found_urls.add_urls_from_text_array(&data_srcs, &source_url_str, UrlSource::ImgSrc);

        // <input src="..."
        let input_srcs: Vec<&str> = RE_INPUT_SRC
            .captures_iter(html)
            .filter_map(|caps| caps.get(1).map(|m| m.as_str()))
            .collect();
        found_urls.add_urls_from_text_array(&input_srcs, &source_url_str, UrlSource::InputSrc);

        // <link href="...(image extensions)"
        let link_imgs: Vec<&str> = RE_LINK_IMAGE
            .captures_iter(html)
            .filter_map(|caps| caps.get(1).map(|m| m.as_str()))
            .collect();
        found_urls.add_urls_from_text_array(&link_imgs, &source_url_str, UrlSource::LinkHref);

        // <source src="..."
        let source_srcs: Vec<&str> = RE_SOURCE_SRC
            .captures_iter(html)
            .filter_map(|caps| caps.get(1).map(|m| m.as_str()))
            .collect();
        found_urls.add_urls_from_text_array(&source_srcs, &source_url_str, UrlSource::SourceSrc);

        // CSS url() with image extensions
        let css_imgs: Vec<&str> = RE_CSS_URL_IMAGE
            .captures_iter(html)
            .filter_map(|caps| caps.get(1).map(|m| m.as_str()))
            .collect();
        found_urls.add_urls_from_text_array(&css_imgs, &source_url_str, UrlSource::CssUrl);

        // srcset from <source>, <img>, and imagesrcset
        let mut srcset_urls: Vec<String> = Vec::new();

        let mut srcset_values: Vec<&str> = Vec::new();
        for caps in RE_SOURCE_SRCSET.captures_iter(html) {
            if let Some(m) = caps.get(1) {
                srcset_values.push(m.as_str());
            }
        }
        for caps in RE_IMG_SRCSET.captures_iter(html) {
            if let Some(m) = caps.get(1) {
                srcset_values.push(m.as_str());
            }
        }
        for caps in RE_IMAGESRCSET.captures_iter(html) {
            if let Some(m) = caps.get(1) {
                srcset_values.push(m.as_str());
            }
        }

        for srcset in &srcset_values {
            // srcset sources are separated by ", " (comma+space)
            for source in srcset.split(", ") {
                let trimmed = source.trim();
                if trimmed.is_empty() {
                    continue;
                }
                // Split by whitespace to separate URL from size descriptor
                let url_part = trimmed.split_whitespace().next().unwrap_or("");
                let url_trimmed = url_part.trim().to_string();
                if !url_trimmed.is_empty() && !srcset_urls.contains(&url_trimmed) {
                    srcset_urls.push(url_trimmed);
                }
            }
        }

        let srcset_refs: Vec<&str> = srcset_urls.iter().map(|s| s.as_str()).collect();
        found_urls.add_urls_from_text_array(&srcset_refs, &source_url_str, UrlSource::ImgSrcset);
    }

    /// Find audio URLs
    fn find_audio(&self, html: &str, source_url: &ParsedUrl, found_urls: &mut FoundUrls) {
        let source_url_str = source_url.get_full_url(true, false);
        let urls: Vec<&str> = RE_AUDIO_SRC
            .captures_iter(html)
            .filter_map(|caps| caps.get(1).map(|m| m.as_str()))
            .collect();
        found_urls.add_urls_from_text_array(&urls, &source_url_str, UrlSource::AudioSrc);
    }

    /// Find video URLs
    fn find_video(&self, html: &str, source_url: &ParsedUrl, found_urls: &mut FoundUrls) {
        let source_url_str = source_url.get_full_url(true, false);
        let urls: Vec<&str> = RE_VIDEO_SRC
            .captures_iter(html)
            .filter_map(|caps| caps.get(1).map(|m| m.as_str()))
            .collect();
        found_urls.add_urls_from_text_array(&urls, &source_url_str, UrlSource::VideoSrc);
    }

    /// Find script URLs from <script src>, <link href=".js">, .src= assignments, and NextJS chunks
    fn find_scripts(&self, html: &str, source_url: &ParsedUrl, found_urls: &mut FoundUrls) {
        let source_url_str = source_url.get_full_url(true, false);

        // <script src="..."
        let script_srcs: Vec<&str> = RE_SCRIPT_SRC
            .captures_iter(html)
            .filter_map(|caps| caps.get(1).map(|m| m.as_str()))
            .collect();
        found_urls.add_urls_from_text_array(&script_srcs, &source_url_str, UrlSource::ScriptSrc);

        // <link href="...(json|js)"
        let link_js: Vec<&str> = RE_LINK_JS
            .captures_iter(html)
            .filter_map(|caps| caps.get(1).map(|m| m.as_str()))
            .collect();
        found_urls.add_urls_from_text_array(&link_js, &source_url_str, UrlSource::LinkHref);

        // .src = "..." (lazy loading in JS)
        let dot_srcs: Vec<&str> = RE_DOT_SRC
            .captures_iter(html)
            .filter_map(|caps| caps.get(1).map(|m| m.as_str()))
            .collect();
        found_urls.add_urls_from_text_array(&dot_srcs, &source_url_str, UrlSource::InlineScriptSrc);

        // NextJS chunks
        let mut next_js_chunks: Vec<String> = Vec::new();
        for caps in RE_NEXTJS_CHUNKS.captures_iter(html) {
            if let Some(m) = caps.get(1) {
                let matched = m.as_str();
                let chunk_url = if matched.starts_with("//") {
                    format!("{}:{}", source_url.scheme.as_deref().unwrap_or("https"), matched)
                } else if matched.starts_with("http://") || matched.starts_with("https://") {
                    matched.to_string()
                } else if matched.contains("/_next/") {
                    let mut url = matched.to_string();
                    if source_url.host.is_some() && source_url.host != self.config.initial_url.host {
                        url = format!("{}{}", source_url.get_full_homepage_url(), url);
                    }
                    url
                } else {
                    format!("{}/_next/{}", source_url.get_full_homepage_url(), matched)
                };
                next_js_chunks.push(chunk_url);
            }
        }
        let chunk_refs: Vec<&str> = next_js_chunks.iter().map(|s| s.as_str()).collect();
        found_urls.add_urls_from_text_array(&chunk_refs, &source_url_str, UrlSource::InlineScriptSrc);
    }

    /// Find stylesheet URLs from <link> tags with rel="stylesheet"
    fn find_stylesheets(&self, html: &str, source_url: &ParsedUrl, found_urls: &mut FoundUrls) {
        let source_url_str = source_url.get_full_url(true, false);

        let mut stylesheet_urls: Vec<String> = Vec::new();
        for caps in RE_LINK_STYLESHEET.captures_iter(html) {
            let full_match = caps.get(0).map(|m| m.as_str()).unwrap_or("");
            if let Some(href) = caps.get(1) {
                // Only include if no rel= attribute or rel="stylesheet"
                let full_lower = full_match.to_lowercase();
                if !full_lower.contains("rel=") || full_lower.contains("stylesheet") {
                    stylesheet_urls.push(href.as_str().to_string());
                }
            }
        }

        let url_refs: Vec<&str> = stylesheet_urls.iter().map(|s| s.as_str()).collect();
        found_urls.add_urls_from_text_array(&url_refs, &source_url_str, UrlSource::LinkHref);
    }

    /// Remove all unwanted code from HTML with respect to --disable-* options
    fn remove_unwanted_code_from_html(&self, html: &str) -> String {
        let mut result = html.to_string();

        if !self.config.scripts_enabled {
            result = utils::strip_javascript(&result);
        }
        if !self.config.styles_enabled {
            result = utils::strip_styles(&result);
        }
        if !self.config.fonts_enabled {
            result = utils::strip_fonts(&result);
        }
        if !self.config.images_enabled && result.to_lowercase().contains("<img") {
            result = utils::strip_images(&result, None);
            result = self.set_custom_css_for_tile_images(&result);
            result = utils::add_class_to_html_images(&result, "siteone-crawler-bg");
        }

        result
    }

    /// Add custom CSS for placeholder tile images
    fn set_custom_css_for_tile_images(&self, html: &str) -> String {
        let background_base64 = "iVBORw0KGgoAAAANSUhEUgAAAEAAAAAkCAMAAAAO0sygAAAAAXNSR0IB2cksfwAAAAlwSFlzAAALEwAACxMBAJqcGAAAAMlQTFRFFxcXwMDA////1NTU5+fnpaWl0tLSIyMj5ubmlJSUxcXF29vbz8/P9PT01tbWxMTE8fHxaWlp39/f9fX1yMjI3NzciYmJeXl5Gxsb2dnZNTU18/PzXFxc5eXlJycnysrKZGRk6enp3d3dW1tbsrKyWFhYIiIi19fXvLy8w8PDuLi47e3tzMzM0dHRx8fH09PTHR0dzs7OLy8vwcHB0NDQSEhIqamp4uLiHh4eOzs74ODg3t7ewsLCISEhJCQkaGhoy8vLzc3N2NjYEPdgjAAAAaRJREFUeJzdlWlTgzAQhiGlWlsCCHog9vAWpalavK3X//9RJptACoEwTp1x9MPOht19k+VJCIZhIvNXzVh1DmPVHowfe5cOsrpr5dh6D1kb/VJs0LWQ3cAAO7gyp0tjXinGavBmPQOf5gaVvgIaC5eet5jeceoZbNPcjhjvcu9GGHl7sjYGfbXP3HyaG/Dx/pD7UYDwOFT0ThuDSYwOahgcCn0rgyNWZykMQNtpYXBM9+wkhtreKZ8zZ8D52RoGEeT6E9EntTPmzxP5/mEIXscAX8SF/hL6TSEHc6VTRGbNDMyrYaElRPRxncr1bVpzEzczyHugNjeIGHO9ydbNMjomGgbUUq5PlDMzp3p5FpoYmLei77sERUJ//yBy0wy8lkFf8s/XV/rVMXjk9VahnYF/KvWrY/AMuZdFrvdQCHtPSnVt52BOfa/YP6LcBzoGfj73K1s/q70PtAzYt/DGx8P3Dx6r3Ad6Br6ce2GLmLwPknGAgigAPfNBFDffB6WzKRiMvGJfK/urMlgyycBV9FjDoLAlBl5V/9nwPXzb/tO/8e8y+AJh0S3ETlwQiAAAAABJRU5ErkJggg==";

        RE_CLOSE_HEAD
            .replace(
                html,
                &format!(
                    "<style>\n\
                .siteone-crawler-bg {{\n\
                    background-image: url(\"data:image/png;base64,{}\");\n\
                    background-repeat: repeat;\n\
                    opacity: 0.15;\n\
                }}\n\
            </style></head>",
                    background_base64
                ),
            )
            .to_string()
    }

    /// Set JS variable _SiteOneUrlDepth with number of levels before </head>
    fn set_js_variable_with_url_depth(&self, html: &str, base_url: &str) -> String {
        let base_path = if let Ok(parsed) = url::Url::parse(base_url) {
            parsed.path().to_string()
        } else {
            "/".to_string()
        };

        let trimmed = base_path.trim_start_matches('/');
        let mut depth = trimmed.matches('/').count();

        let needs_index_html = base_path != "/" && base_path.ends_with('/');
        if needs_index_html {
            depth += 1;
        }

        RE_CLOSE_HEAD
            .replace(
                html,
                &format!(
                    "<script>var {} = {};</script></head>",
                    JS_VARIABLE_NAME_URL_DEPTH, depth
                ),
            )
            .to_string()
    }

    /// Set JS function to remove all anchor listeners before </body>
    fn set_js_function_to_remove_all_anchor_listeners(&self, html: &str) -> String {
        RE_CLOSE_BODY
            .replace(
                html,
                concat!(
                    "<script>\n",
                    "function _SiteOneRemoveAllAnchorListeners(){\n",
                    "    var anchors=document.getElementsByTagName('a');\n",
                    "    for(var i=0;i<anchors.length;i++){\n",
                    "        var anchor=anchors[i];\n",
                    "        var newAnchor=anchor.cloneNode(true);\n",
                    "        anchor.parentNode.replaceChild(newAnchor,anchor);\n",
                    "    }\n",
                    "}\n",
                    "setTimeout(_SiteOneRemoveAllAnchorListeners, 200);\n",
                    "setTimeout(_SiteOneRemoveAllAnchorListeners, 1000);\n",
                    "setTimeout(_SiteOneRemoveAllAnchorListeners, 5000);\n",
                    "</script></body>",
                ),
            )
            .to_string()
    }

    /// Remove scheme and host from full origin URLs to simplify relative paths conversion
    fn remove_schema_and_host_from_full_origin_urls(&self, url: &ParsedUrl, content: &str) -> String {
        static RE_BASE_URL_ROOT: Lazy<Regex> = Lazy::new(|| Regex::new(r"(?i)((https?:)?//[^/]+/?).*").unwrap());

        let full_url = url.get_full_url(true, false);
        // Extract base URL root (scheme://host/)
        let base_url_root = RE_BASE_URL_ROOT.replace(&full_url, "$1").to_string();

        let mut result = content.to_string();

        // Normalize port numbers
        result = RE_PORT_NORMALIZE.replace_all(&result, "$1").to_string();

        // Build patterns for href=, src=, url=, url( attributes
        let escaped_root = regex::escape(&base_url_root);
        let attr_patterns = [
            format!(r#"(?i)(href=(["'])){esc}([^"']*)(["'])"#, esc = escaped_root),
            format!(r#"(?i)(src=(["'])){esc}([^"']*)(["'])"#, esc = escaped_root),
            format!(r#"(?i)(url=(["'])){esc}([^"']*)(["'])"#, esc = escaped_root),
            format!(r#"(?i)(url\((["']?)){esc}([^"')]*)(["']\)|\))"#, esc = escaped_root),
        ];

        for pattern in &attr_patterns {
            if let Ok(re) = Regex::new(pattern) {
                let compiled_ignore = &self.config.compiled_ignore_regex;
                result = re
                    .replace_all(&result, |caps: &regex::Captures| {
                        let full_match = caps.get(0).map_or("", |m| m.as_str());

                        // Check against pre-compiled ignore patterns
                        for ire in compiled_ignore {
                            if ire.is_match(full_match) {
                                return full_match.to_string();
                            }
                        }

                        let attr_start = caps.get(1).map_or("", |m| m.as_str());
                        let path = caps.get(3).map_or("", |m| m.as_str());
                        let attr_end = caps.get(4).map_or("", |m| m.as_str());

                        format!("{}/{}{}", attr_start, path, attr_end)
                    })
                    .to_string();
            }
        }

        result
    }

    /// Update all HTML paths to relative for offline version
    fn update_html_paths_to_relative(&self, html: &str, parsed_base_url: &ParsedUrl) -> String {
        let initial_url = &self.config.initial_url;
        let compiled_ignore = &self.config.compiled_ignore_regex;

        let replace_callback = |caps: &regex::Captures| -> String {
            let full_match = caps.get(0).map_or("", |m| m.as_str());
            let start = caps.get(1).map_or("", |m| m.as_str());
            let attribute_raw = caps.get(2).map_or("", |m| m.as_str());
            let attribute = attribute_raw.trim_matches(|c: char| c == ' ' || c == '\\' || c == '"' || c == '\'');
            let assignment_char = caps.get(3).map_or("", |m| m.as_str());
            let quote = caps.get(4).map_or("", |m| m.as_str());
            // Decode HTML entities in URL values (fixes Astro image query params like &#38; → &)
            let value_raw = caps.get(5).map_or("", |m| m.as_str());
            let value_decoded = html_entity_decode(value_raw);
            let value = value_decoded.as_str();
            let end = caps.get(6).map_or("", |m| m.as_str());

            // When modifying x.src (JS) and there is no quote, do not convert
            if start == "." && quote.is_empty() {
                return full_match.to_string();
            }

            // Ignore data URI, anchor, or non-http scheme
            if value.starts_with('#') || RE_NON_HTTP_SCHEME.is_match(value) {
                return full_match.to_string();
            }

            // Check against pre-compiled ignore regex patterns
            for ire in compiled_ignore {
                if ire.is_match(value) {
                    return full_match.to_string();
                }
            }

            let attr_lower = attribute.to_lowercase();
            let new_value = if attr_lower == "srcset" || attr_lower == "imagesrcset" {
                // Handle srcset: multiple sources separated by ", "
                let sources: Vec<&str> = value.split(", ").collect();
                let converted: Vec<String> = sources
                    .iter()
                    .map(|source| {
                        let trimmed = source.trim();
                        if !trimmed.contains(' ') {
                            // URL without size descriptor
                            convert_url_to_relative(parsed_base_url, trimmed, initial_url, Some(&attr_lower))
                        } else {
                            // URL with size descriptor (e.g., "url 2x")
                            let mut parts = trimmed.splitn(2, char::is_whitespace);
                            let url_part = parts.next().unwrap_or("");
                            let size_part = parts.next().unwrap_or("");
                            let relative_url =
                                convert_url_to_relative(parsed_base_url, url_part, initial_url, Some(&attr_lower));
                            format!("{} {}", relative_url, size_part)
                        }
                    })
                    .collect();
                converted.join(", ")
            } else {
                let mut converted = convert_url_to_relative(parsed_base_url, value, initial_url, Some(attribute));

                // Handle component-url and renderer-url (Astro)
                if attribute == "component-url" || attribute == "renderer-url" {
                    converted = format!("./{}", converted);
                }

                converted
            };

            format!(
                "{}{}{}{}{}{}{}",
                start, attribute_raw, assignment_char, quote, new_value, quote, end
            )
        };

        let mut result = html.to_string();
        result = RE_HREF_SRC.replace_all(&result, replace_callback).to_string();
        result = RE_SRCSET_ATTR.replace_all(&result, replace_callback).to_string();
        result = RE_META_URL.replace_all(&result, replace_callback).to_string();
        result = RE_ESCAPED_HREF_SRC.replace_all(&result, replace_callback).to_string();
        result
    }

    /// Apply specific HTML changes for offline version
    #[allow(clippy::too_many_arguments)]
    fn apply_specific_html_changes(
        &self,
        html: &mut String,
        parsed_base_url: &ParsedUrl,
        remove_external_js: bool,
        remove_cross_origins: bool,
        remove_analytics: bool,
        remove_socnets: bool,
        remove_cookies_related: bool,
    ) {
        if html.trim().is_empty() {
            return;
        }

        let base_host = parsed_base_url.host.as_deref().unwrap_or("");

        // Remove external JS
        if remove_external_js {
            let base_host_owned = base_host.to_string();
            *html = RE_EXTERNAL_SCRIPT
                .replace_all(html, |caps: &regex::Captures| {
                    let full_match = caps.get(0).map_or("", |m| m.as_str());
                    let src = caps.get(1).map_or("", |m| m.as_str());

                    if RE_EXTERNAL_URL.is_match(src) {
                        // Parse host from the src URL
                        let parsed_src = if src.starts_with("//") {
                            format!("https:{}", src)
                        } else {
                            src.to_string()
                        };
                        if let Ok(parsed) = url::Url::parse(&parsed_src)
                            && parsed.host_str().unwrap_or("") != base_host_owned
                        {
                            return String::new();
                        }
                    }
                    full_match.to_string()
                })
                .to_string();
        }

        // Remove crossorigin attributes
        if remove_cross_origins {
            *html = RE_CROSSORIGIN_LINK.replace_all(html, "$1$3").to_string();
            *html = RE_CROSSORIGIN_SCRIPT.replace_all(html, "$1$3").to_string();
        }

        // Remove analytics and social network scripts
        if remove_analytics || remove_socnets || remove_cookies_related {
            let mut patterns: Vec<&str> = Vec::new();

            if remove_analytics {
                patterns.extend_from_slice(&[
                    "googletagmanager.com",
                    "google-analytics.com",
                    "ga.js",
                    "gtag.js",
                    "gtag(",
                    "analytics.",
                    "connect.facebook.net",
                    "fbq(",
                ]);
            }

            if remove_socnets {
                patterns.extend_from_slice(&[
                    "connect.facebook.net",
                    "connect.facebook.com",
                    "twitter.com",
                    ".x.com",
                    "linkedin.com",
                    "instagram.com",
                    "pinterest.com",
                    "tumblr.com",
                    "plus.google.com",
                    "curator.io",
                ]);
            }

            if remove_cookies_related {
                patterns.extend_from_slice(&["cookies", "cookiebot"]);
            }

            // Deduplicate
            patterns.sort();
            patterns.dedup();

            *html = RE_SCRIPT_BLOCK
                .replace_all(html, |caps: &regex::Captures| {
                    let full_match = caps.get(0).map_or("", |m| m.as_str());
                    let full_lower = full_match.to_lowercase();

                    for keyword in &patterns {
                        if full_lower.contains(&keyword.to_lowercase()) {
                            return String::new();
                        }
                    }

                    full_match.to_string()
                })
                .to_string();

            // Remove social network iframes
            if remove_socnets {
                *html = RE_SOCNET_IFRAME.replace_all(html, "").to_string();
            }
        }
    }

    /// Check if anchor listener removal is forced (e.g., for NextJS sites)
    fn is_forced_to_remove_anchor_listeners(&self, html: &str) -> bool {
        html.contains("_next/")
    }
}

impl ContentProcessor for HtmlProcessor {
    fn find_urls(&self, content: &str, source_url: &ParsedUrl) -> Option<FoundUrls> {
        let mut found_urls = FoundUrls::new();

        if !self.config.single_page {
            self.find_href_urls(content, source_url, &mut found_urls);
        }

        if self.config.fonts_enabled {
            self.find_fonts(content, source_url, &mut found_urls);
        }

        if self.config.images_enabled {
            self.find_images(content, source_url, &mut found_urls);
        }

        if self.config.files_enabled {
            self.find_audio(content, source_url, &mut found_urls);
            self.find_video(content, source_url, &mut found_urls);
        }

        if self.config.scripts_enabled {
            self.find_scripts(content, source_url, &mut found_urls);
        }

        if self.config.styles_enabled {
            self.find_stylesheets(content, source_url, &mut found_urls);
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
        // No changes needed before URL parsing in HtmlProcessor
    }

    fn apply_content_changes_for_offline_version(
        &self,
        content: &mut String,
        _content_type: ContentTypeId,
        url: &ParsedUrl,
        remove_unwanted_code: bool,
    ) {
        let base_url = url.get_full_url(true, false);

        // Remove schema and host from full origin URLs
        *content = self.remove_schema_and_host_from_full_origin_urls(url, content);

        // Remove unwanted code from HTML
        *content = self.remove_unwanted_code_from_html(content);

        // Update all paths to relative
        *content = self.update_html_paths_to_relative(content, url);

        // Meta redirects (e.g., in Astro projects)
        if let Some(caps) = RE_META_REFRESH.captures(content) {
            let full_match = caps.get(0).map_or("", |m| m.as_str());
            let prefix = caps.get(1).map_or("", |m| m.as_str());
            let meta_url = caps.get(2).map_or("", |m| m.as_str());
            let suffix = caps.get(3).map_or("", |m| m.as_str());

            let relative = convert_url_to_relative(url, meta_url, &self.config.initial_url, None);
            *content = content.replace(full_match, &format!("{}{}{}", prefix, relative, suffix));
        }

        // Apply specific HTML changes
        self.apply_specific_html_changes(
            content,
            url,
            self.config.disable_javascript,
            remove_unwanted_code,
            remove_unwanted_code,
            remove_unwanted_code,
            remove_unwanted_code,
        );

        // Set JS variable and remove anchor listeners
        if self.config.scripts_enabled {
            *content = self.set_js_variable_with_url_depth(content, &base_url);
            if self.config.remove_all_anchor_listeners || self.is_forced_to_remove_anchor_listeners(content) {
                *content = self.set_js_function_to_remove_all_anchor_listeners(content);
            }
        }
    }

    fn is_content_type_relevant(&self, content_type: ContentTypeId) -> bool {
        is_relevant(content_type, &self.relevant_content_types)
    }

    fn get_name(&self) -> &str {
        "HtmlProcessor"
    }

    fn set_debug_mode(&mut self, debug_mode: bool) {
        self.debug_mode = debug_mode;
    }
}

/// Decode HTML entities in URL attribute values.
/// Single-pass implementation to avoid double-decoding (e.g. `&#38;amp;` → `&amp;`, not `&`).
fn html_entity_decode(input: &str) -> String {
    let mut result = String::with_capacity(input.len());
    let bytes = input.as_bytes();
    let len = bytes.len();
    let mut i = 0;

    while i < len {
        if bytes[i] == b'&' {
            // Try to match a named or numeric entity
            if let Some((decoded, advance)) = try_decode_entity(&input[i..]) {
                result.push_str(decoded);
                i += advance;
                continue;
            }
        }
        result.push(input[i..].chars().next().unwrap());
        i += input[i..].chars().next().unwrap().len_utf8();
    }

    result
}

/// Try to decode a single HTML entity at the start of `s`. Returns (decoded, bytes_consumed).
fn try_decode_entity(s: &str) -> Option<(&'static str, usize)> {
    // Named entities
    for (entity, decoded) in [
        ("&amp;", "&"),
        ("&lt;", "<"),
        ("&gt;", ">"),
        ("&quot;", "\""),
        ("&apos;", "'"),
    ] {
        if s.starts_with(entity) {
            return Some((decoded, entity.len()));
        }
    }

    // Numeric entities (decimal and hex)
    for (entity, decoded) in [
        ("&#38;", "&"),
        ("&#x26;", "&"),
        ("&#60;", "<"),
        ("&#x3C;", "<"),
        ("&#x3c;", "<"),
        ("&#62;", ">"),
        ("&#x3E;", ">"),
        ("&#x3e;", ">"),
        ("&#34;", "\""),
        ("&#x22;", "\""),
        ("&#39;", "'"),
        ("&#x27;", "'"),
        ("&#039;", "'"),
    ] {
        if s.starts_with(entity) {
            return Some((decoded, entity.len()));
        }
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_config() -> ProcessorConfig {
        ProcessorConfig::new(ParsedUrl::parse("https://example.com/", None))
    }

    #[test]
    fn test_find_href_urls() {
        let processor = HtmlProcessor::new(make_config());
        let html = r#"<html><body><a href="/about">About</a><a href="/contact">Contact</a></body></html>"#;
        let source = ParsedUrl::parse("https://example.com/", None);
        let result = processor.find_urls(html, &source);
        assert!(result.is_some());
        let found = result.unwrap();
        assert!(found.get_count() >= 2);
    }

    #[test]
    fn test_find_images() {
        let processor = HtmlProcessor::new(make_config());
        let html = r#"<html><body><img src="/img/logo.png"><img data-src="/img/lazy.jpg"></body></html>"#;
        let source = ParsedUrl::parse("https://example.com/", None);
        let result = processor.find_urls(html, &source);
        assert!(result.is_some());
    }

    #[test]
    fn test_find_scripts() {
        let processor = HtmlProcessor::new(make_config());
        let html = r#"<html><head><script src="/js/app.js"></script></head></html>"#;
        let source = ParsedUrl::parse("https://example.com/", None);
        let result = processor.find_urls(html, &source);
        assert!(result.is_some());
    }

    #[test]
    fn test_single_page_no_hrefs() {
        let mut config = make_config();
        config.single_page = true;
        let processor = HtmlProcessor::new(config);
        let html = r#"<html><body><a href="/about">About</a><script src="/app.js"></script></body></html>"#;
        let source = ParsedUrl::parse("https://example.com/", None);
        let result = processor.find_urls(html, &source);
        assert!(result.is_some());
        // Should only find script, not href
        let found = result.unwrap();
        for (_key, url) in found.get_urls() {
            assert_ne!(url.url, "/about");
        }
    }

    #[test]
    fn test_find_srcset() {
        let processor = HtmlProcessor::new(make_config());
        let html = r#"<img srcset="/img/small.jpg 1x, /img/large.jpg 2x">"#;
        let source = ParsedUrl::parse("https://example.com/", None);
        let result = processor.find_urls(html, &source);
        assert!(result.is_some());
    }
}
