// SiteOne Crawler - VisitedUrl
// (c) Jan Reges <jan.reges@siteone.cz>

use std::collections::HashMap;

use regex::Regex;
use serde::{Deserialize, Serialize};

use crate::types::ContentTypeId;
use crate::utils;

// Error status codes (negative values)
pub const ERROR_CONNECTION_FAIL: i32 = -1;
pub const ERROR_TIMEOUT: i32 = -2;
pub const ERROR_SERVER_RESET: i32 = -3;
pub const ERROR_SEND_ERROR: i32 = -4;
pub const ERROR_SKIPPED: i32 = -6;

// Cache type flags (bitwise OR)
pub const CACHE_TYPE_HAS_CACHE_CONTROL: u32 = 1;
pub const CACHE_TYPE_HAS_EXPIRES: u32 = 2;
pub const CACHE_TYPE_HAS_ETAG: u32 = 4;
pub const CACHE_TYPE_HAS_LAST_MODIFIED: u32 = 8;
pub const CACHE_TYPE_HAS_MAX_AGE: u32 = 16;
pub const CACHE_TYPE_HAS_S_MAX_AGE: u32 = 32;
pub const CACHE_TYPE_HAS_STALE_WHILE_REVALIDATE: u32 = 64;
pub const CACHE_TYPE_HAS_STALE_IF_ERROR: u32 = 128;
pub const CACHE_TYPE_HAS_PUBLIC: u32 = 256;
pub const CACHE_TYPE_HAS_PRIVATE: u32 = 512;
pub const CACHE_TYPE_HAS_NO_CACHE: u32 = 1024;
pub const CACHE_TYPE_HAS_NO_STORE: u32 = 2048;
pub const CACHE_TYPE_HAS_MUST_REVALIDATE: u32 = 4096;
pub const CACHE_TYPE_HAS_PROXY_REVALIDATE: u32 = 8192;
pub const CACHE_TYPE_HAS_IMMUTABLE: u32 = 16384;
pub const CACHE_TYPE_NO_CACHE_HEADERS: u32 = 32768;
pub const CACHE_TYPE_NOT_AVAILABLE: u32 = 65536;

// Source attribute constants
pub const SOURCE_INIT_URL: i32 = 5;
pub const SOURCE_A_HREF: i32 = 10;
pub const SOURCE_IMG_SRC: i32 = 20;
pub const SOURCE_IMG_SRCSET: i32 = 21;
pub const SOURCE_INPUT_SRC: i32 = 22;
pub const SOURCE_SOURCE_SRC: i32 = 23;
pub const SOURCE_VIDEO_SRC: i32 = 24;
pub const SOURCE_AUDIO_SRC: i32 = 25;
pub const SOURCE_SCRIPT_SRC: i32 = 30;
pub const SOURCE_INLINE_SCRIPT_SRC: i32 = 40;
pub const SOURCE_LINK_HREF: i32 = 50;
pub const SOURCE_CSS_URL: i32 = 60;
pub const SOURCE_JS_URL: i32 = 70;
pub const SOURCE_REDIRECT: i32 = 80;
pub const SOURCE_SITEMAP: i32 = 90;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VisitedUrl {
    /// Unique ID hash of this URL
    pub uq_id: String,

    /// Unique ID hash of the source URL where this URL was found
    pub source_uq_id: String,

    /// Source attribute where this URL was found (see SOURCE_* constants)
    pub source_attr: i32,

    /// Full URL with scheme, domain, path and query
    pub url: String,

    /// HTTP status code of the request (negative values are errors, see ERROR_* constants)
    pub status_code: i32,

    /// Request time in seconds
    pub request_time: f64,

    /// Request time formatted as "32 ms" or "7.4 s"
    pub request_time_formatted: String,

    /// Size of the response in bytes
    pub size: Option<i64>,

    /// Size of the response formatted as "1.23 MB"
    pub size_formatted: Option<String>,

    /// Content-Encoding header value (br, gzip, ...)
    pub content_encoding: Option<String>,

    /// Content type ID
    pub content_type: ContentTypeId,

    /// Content type header value (text/html, application/json, ...)
    pub content_type_header: Option<String>,

    /// Extra data from the response required by --extra-columns
    pub extras: Option<HashMap<String, String>>,

    /// Is this URL external (not from the same domain as the initial URL)
    pub is_external: bool,

    /// Is this URL allowed for crawling (based on --allowed-domain-for-crawling)
    pub is_allowed_for_crawling: bool,

    /// Cache type flags of the response (bitwise OR). See CACHE_TYPE_* constants
    pub cache_type_flags: u32,

    /// How long the response is allowed to be cached in seconds
    pub cache_lifetime: Option<i64>,
}

impl VisitedUrl {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        uq_id: String,
        source_uq_id: String,
        source_attr: i32,
        url: String,
        status_code: i32,
        request_time: f64,
        size: Option<i64>,
        content_type: ContentTypeId,
        content_type_header: Option<String>,
        content_encoding: Option<String>,
        extras: Option<HashMap<String, String>>,
        is_external: bool,
        is_allowed_for_crawling: bool,
        cache_type_flags: u32,
        cache_lifetime: Option<i64>,
    ) -> Self {
        let request_time_formatted = utils::get_formatted_duration(request_time);
        let size_formatted = size.map(|s| utils::get_formatted_size(s, 0));

        Self {
            uq_id,
            source_uq_id,
            source_attr,
            url,
            status_code,
            request_time,
            request_time_formatted,
            size,
            size_formatted,
            content_encoding,
            content_type,
            content_type_header,
            extras,
            is_external,
            is_allowed_for_crawling,
            cache_type_flags,
            cache_lifetime,
        }
    }

    pub fn is_https(&self) -> bool {
        self.url.starts_with("https://")
    }

    pub fn is_static_file(&self) -> bool {
        matches!(
            self.content_type,
            ContentTypeId::Image
                | ContentTypeId::Script
                | ContentTypeId::Stylesheet
                | ContentTypeId::Video
                | ContentTypeId::Audio
                | ContentTypeId::Document
                | ContentTypeId::Font
                | ContentTypeId::Json
                | ContentTypeId::Xml
        )
    }

    pub fn is_image(&self) -> bool {
        self.content_type == ContentTypeId::Image
    }

    pub fn is_video(&self) -> bool {
        self.content_type == ContentTypeId::Video
    }

    pub fn get_source_description(&self, source_url: Option<&str>) -> String {
        let source = source_url.unwrap_or("unknown");
        match self.source_attr {
            SOURCE_INIT_URL => "Initial URL".to_string(),
            SOURCE_A_HREF => format!("<a href> on {}", source),
            SOURCE_IMG_SRC => format!("<img src> on {}", source),
            SOURCE_IMG_SRCSET => format!("<img srcset> on {}", source),
            SOURCE_INPUT_SRC => format!("<input src> on {}", source),
            SOURCE_SOURCE_SRC => format!("<source src> on {}", source),
            SOURCE_VIDEO_SRC => format!("<video src> on {}", source),
            SOURCE_AUDIO_SRC => format!("<audio src> on {}", source),
            SOURCE_SCRIPT_SRC => format!("<script src> on {}", source),
            SOURCE_INLINE_SCRIPT_SRC => format!("<script> on {}", source),
            SOURCE_LINK_HREF => format!("<link href> on {}", source),
            SOURCE_CSS_URL => format!("CSS url() on {}", source),
            SOURCE_JS_URL => format!("JS url on {}", source),
            SOURCE_REDIRECT => format!("Redirect from {}", source),
            SOURCE_SITEMAP => format!("URL in sitemap {}", source),
            _ => "Unknown source".to_string(),
        }
    }

    pub fn get_source_short_name(&self) -> &'static str {
        match self.source_attr {
            SOURCE_INIT_URL => "Initial URL",
            SOURCE_A_HREF => "<a href>",
            SOURCE_IMG_SRC => "<img src>",
            SOURCE_IMG_SRCSET => "<img srcset>",
            SOURCE_INPUT_SRC => "<input src>",
            SOURCE_SOURCE_SRC => "<source src>",
            SOURCE_VIDEO_SRC => "<video src>",
            SOURCE_AUDIO_SRC => "<audio src>",
            SOURCE_SCRIPT_SRC => "<script src>",
            SOURCE_INLINE_SCRIPT_SRC => "inline <script src>",
            SOURCE_LINK_HREF => "<link href>",
            SOURCE_CSS_URL => "css url()",
            SOURCE_JS_URL => "js url",
            SOURCE_REDIRECT => "redirect",
            SOURCE_SITEMAP => "sitemap",
            _ => "unknown",
        }
    }

    pub fn looks_like_static_file_by_url(&self) -> bool {
        use once_cell::sync::Lazy;
        static RE_STATIC_FILE: Lazy<Regex> = Lazy::new(|| {
            Regex::new(
                r"(?i)\.(jpg|jpeg|png|gif|webp|svg|ico|js|css|txt|woff2|woff|ttf|eot|mp4|webm|ogg|mp3|wav|flac|pdf|doc|docx|xls|xlsx|ppt|pptx|zip|rar|gz|bz2|7z|xml|json)",
            ).unwrap()
        });
        RE_STATIC_FILE.is_match(&self.url)
    }

    pub fn has_error_status_code(&self) -> bool {
        self.status_code < 0
    }

    pub fn get_scheme(&self) -> Option<String> {
        url::Url::parse(&self.url).ok().map(|u| u.scheme().to_string())
    }

    pub fn get_host(&self) -> Option<String> {
        url::Url::parse(&self.url)
            .ok()
            .and_then(|u| u.host_str().map(|h| h.to_string()))
    }

    pub fn get_port(&self) -> u16 {
        if let Ok(parsed) = url::Url::parse(&self.url) {
            parsed.port().unwrap_or_else(|| if self.is_https() { 443 } else { 80 })
        } else if self.is_https() {
            443
        } else {
            80
        }
    }

    pub fn get_cache_type_label(&self) -> String {
        let mut labels = Vec::new();

        // Cache-Control or Expires (if Cache-Control is not defined)
        if self.cache_type_flags & CACHE_TYPE_HAS_CACHE_CONTROL != 0 {
            labels.push("Cache-Control");
        } else if self.cache_type_flags & CACHE_TYPE_HAS_EXPIRES != 0 {
            labels.push("Expires");
        }

        // ETag and Last-Modified
        if self.cache_type_flags & CACHE_TYPE_HAS_ETAG != 0 {
            labels.push("ETag");
        }
        if self.cache_type_flags & CACHE_TYPE_HAS_LAST_MODIFIED != 0 {
            labels.push("Last-Modified");
        }

        if labels.is_empty() {
            "No cache headers".to_string()
        } else {
            labels.join(" + ")
        }
    }
}
