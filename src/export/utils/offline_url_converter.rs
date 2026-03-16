// SiteOne Crawler - OfflineUrlConverter
// (c) Jan Reges <jan.reges@siteone.cz>
//
// Converts absolute URLs to relative paths for offline browsing.

use std::sync::Mutex;

use once_cell::sync::Lazy;
use regex::Regex;

use crate::engine::parsed_url::ParsedUrl;
use crate::utils;

use super::target_domain_relation::TargetDomainRelation;

/// Static replace_query_string configuration
static REPLACE_QUERY_STRING: Lazy<Mutex<Vec<String>>> = Lazy::new(|| Mutex::new(Vec::new()));

/// Static lowercase configuration for offline export
static LOWERCASE: Lazy<Mutex<bool>> = Lazy::new(|| Mutex::new(false));

/// Regex for removing file extension from path
static STRIP_EXT_RE: Lazy<Regex> = Lazy::new(|| Regex::new(r"(?i)\.[a-z0-9]{1,10}$").unwrap());

/// Regex for removing domain from path
static DOMAIN_IN_PATH_RE: Lazy<Regex> = Lazy::new(|| Regex::new(r"(?i)^(//|https?://)([^/]+)(:[0-9]+)?").unwrap());

/// Static files extensions regex pattern
static STATIC_FILES_EXTENSIONS: &str = "jpg|jpeg|png|gif|webp|svg|ico|js|css|txt|woff2|woff|ttf|eot|mp4|webm|ogg|mp3|wav|flac|pdf|doc\
     |docx|xls|xlsx|ppt|pptx|zip|rar|gz|bz2|7z|tar|xml|json|action|asp|aspx|cfm|cfml|cgi|do|gsp|jsp|jspx|lasso|phtml\
     |php|php3|php4|php5|php7|php8|php9|pl|py|rb|rbw|rhtml|shtml|srv|vm|vmdk";

/// Dynamic page extensions that get .html appended
static DYNAMIC_PAGE_EXTENSIONS: &str = "action|asp|aspx|cfm|cfml|cgi|do|gsp|jsp|jspx|lasso|phtml|php3|php4|php5|php7|php8|php9|php|pl|py|rb|rbw|rhtml|shtml|srv|vm";

// Pre-compiled regexes for sanitize_file_path hot path
static RE_PATH_EXTENSION: Lazy<Regex> = Lazy::new(|| Regex::new(r"(?i)^(.+)\.([a-z0-9]{1,10})").unwrap());
static RE_CONTROL_CHARS: Lazy<Regex> = Lazy::new(|| Regex::new(r"[\x00-\x1F\x7F]").unwrap());
static RE_WHITESPACE: Lazy<Regex> = Lazy::new(|| Regex::new(r"\s+").unwrap());
static RE_MULTI_UNDERSCORE: Lazy<Regex> = Lazy::new(|| Regex::new(r"_{2,}").unwrap());
static RE_FRAGMENT_SUFFIX: Lazy<Regex> = Lazy::new(|| Regex::new(r"#.+$").unwrap());
static RE_DOTTED_FOLDER: Lazy<Regex> = Lazy::new(|| Regex::new(r"(?i)([^/]+)\.([a-z0-9]+)/").unwrap());
static RE_DOMAIN_TLD: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"(?i)\.(com|org|net|dev|io|test|local|localhost)$").unwrap());
static RE_STATIC_EXT_FOLDER: Lazy<Regex> = Lazy::new(|| {
    let pattern = format!(r"(?i)([^.]+)\.({})\/", STATIC_FILES_EXTENSIONS);
    Regex::new(&pattern).unwrap()
});
static RE_STATIC_EXT_MATCH: Lazy<Regex> = Lazy::new(|| {
    let pattern = format!(r"(?i)^({})$", STATIC_FILES_EXTENSIONS);
    Regex::new(&pattern).unwrap()
});
static RE_DYNAMIC_EXT: Lazy<Regex> = Lazy::new(|| {
    let pattern = format!(r"(?i)\.({})$", DYNAMIC_PAGE_EXTENSIONS);
    Regex::new(&pattern).unwrap()
});

/// Converts absolute URLs to relative paths for offline browsing.
pub struct OfflineUrlConverter {
    initial_url: ParsedUrl,
    base_url: ParsedUrl,
    target_url: ParsedUrl,
    relative_target_url: ParsedUrl,
    target_url_source_attribute: Option<String>,
    #[allow(clippy::type_complexity)]
    callback_is_domain_allowed_for_static_files: Option<Box<dyn Fn(&str) -> bool + Send + Sync>>,
    #[allow(clippy::type_complexity)]
    callback_is_external_domain_allowed_for_crawling: Option<Box<dyn Fn(&str) -> bool + Send + Sync>>,
    target_domain_relation: TargetDomainRelation,
    preserve_url_structure: bool,
}

impl OfflineUrlConverter {
    #[allow(clippy::type_complexity)]
    pub fn new(
        initial_url: ParsedUrl,
        base_url: ParsedUrl,
        target_url: ParsedUrl,
        callback_is_domain_allowed_for_static_files: Option<Box<dyn Fn(&str) -> bool + Send + Sync>>,
        callback_is_external_domain_allowed_for_crawling: Option<Box<dyn Fn(&str) -> bool + Send + Sync>>,
        attribute: Option<&str>,
    ) -> Self {
        let relative_target_url = target_url.clone();
        let target_domain_relation = TargetDomainRelation::get_by_urls(&initial_url, &base_url, &target_url);

        Self {
            initial_url,
            base_url,
            target_url,
            relative_target_url,
            target_url_source_attribute: attribute.map(|s| s.to_string()),
            callback_is_domain_allowed_for_static_files,
            callback_is_external_domain_allowed_for_crawling,
            target_domain_relation,
            preserve_url_structure: false,
        }
    }

    pub fn set_preserve_url_structure(&mut self, preserve: bool) {
        self.preserve_url_structure = preserve;
    }

    /// Convert URL to relative path for offline browsing.
    pub fn convert_url_to_relative(&mut self, keep_fragment: bool) -> String {
        if let Some(forced_url) = self.get_forced_url_if_needed() {
            return forced_url;
        }

        self.detect_and_set_file_name_with_extension();
        self.calculate_and_apply_depth();

        let pre_final_url = self.relative_target_url.get_full_url(false, keep_fragment);
        Self::sanitize_file_path(&pre_final_url, keep_fragment)
    }

    pub fn get_relative_target_url(&self) -> &ParsedUrl {
        &self.relative_target_url
    }

    pub fn get_target_domain_relation(&self) -> TargetDomainRelation {
        self.target_domain_relation
    }

    /// Set global replace_query_string configuration
    pub fn set_replace_query_string(replace: Vec<String>) {
        if let Ok(mut rqs) = REPLACE_QUERY_STRING.lock() {
            *rqs = replace;
        }
    }

    /// Set global lowercase configuration for offline export
    pub fn set_lowercase(lowercase: bool) {
        if let Ok(mut lc) = LOWERCASE.lock() {
            *lc = lowercase;
        }
    }

    /// Get depth of base URL path in target offline version.
    pub fn get_offline_base_url_depth(url: &ParsedUrl) -> usize {
        let trimmed = url.path.trim_start_matches('/').trim();
        if trimmed.is_empty() {
            return 0;
        }
        trimmed.matches('/').count()
    }

    /// Check if URL needs to be forced (not converted to relative).
    fn get_forced_url_if_needed(&self) -> Option<String> {
        if self.relative_target_url.is_only_fragment()
            && let Some(ref f) = self.relative_target_url.fragment
        {
            return Some(format!("#{}", f));
        }

        // when URL is not requestable resource, it is not possible to convert it to relative URL
        if !utils::is_href_for_requestable_resource(&self.target_url.get_full_url(true, true)) {
            return Some(self.target_url.get_full_url(false, true));
        }

        // when target host is external and not allowed
        let is_external_host = matches!(
            self.target_domain_relation,
            TargetDomainRelation::InitialDifferentBaseDifferent | TargetDomainRelation::InitialDifferentBaseSame
        );

        if is_external_host && let Some(ref host) = self.target_url.host {
            if self.is_external_domain_allowed_for_crawling(host)
                || (self.target_url.is_static_file() && self.is_domain_allowed_for_static_files(host))
                || (!self.target_url.is_static_file()
                    && self.target_url_source_attribute.as_deref() == Some("src")
                    && self.is_domain_allowed_for_static_files(host))
            {
                return None;
            } else {
                return Some(self.target_url.get_full_url(true, true));
            }
        }

        None
    }

    /// Add '*.html' or '/index.html' to path when needed.
    fn detect_and_set_file_name_with_extension(&mut self) {
        let query_hash = self
            .relative_target_url
            .query
            .as_ref()
            .map(|q| Self::get_query_hash_from_query_string(q))
            .filter(|h| !h.trim().is_empty());

        // when the path is empty or '/'
        let trimmed_path = self
            .relative_target_url
            .path
            .trim_matches(|c: char| c == '/' || c == ' ');
        if trimmed_path.is_empty() {
            if let Some(ref hash) = query_hash {
                self.relative_target_url.set_path(format!("/index.{}.html", hash));
                self.relative_target_url.set_query(None);
            } else if self.relative_target_url.path.is_empty() && self.relative_target_url.fragment.is_some() {
                // only #fragment
                return;
            } else {
                self.relative_target_url.set_path("/index.html".to_string());
            }
            return;
        }

        let is_image_attribute = matches!(
            self.target_url_source_attribute.as_deref(),
            Some("src") | Some("srcset")
        );

        // if the URL is probably icon, we use SVG extension, otherwise we use JPG
        let full_url_lower = self.relative_target_url.get_full_url(true, true).to_lowercase();
        let img_extension = if full_url_lower.contains("icon") { "svg" } else { "jpg" };

        // when the URL is probably font from Google Fonts, we use CSS extension
        let other_file_extension = if self.target_url_source_attribute.as_deref() == Some("href")
            && self
                .relative_target_url
                .url
                .to_lowercase()
                .contains("fonts.googleapis.com/css")
        {
            "css"
        } else {
            "html"
        };

        let extension = self.relative_target_url.estimate_extension().unwrap_or_else(|| {
            if is_image_attribute {
                img_extension.to_string()
            } else {
                other_file_extension.to_string()
            }
        });

        if self.relative_target_url.path.ends_with('/') {
            let base_name = "index";
            if let Some(ref hash) = query_hash {
                self.relative_target_url.set_path(format!(
                    "{}{}.{}.{}",
                    self.relative_target_url.path, base_name, hash, extension
                ));
                self.relative_target_url.set_query(None);
            } else {
                self.relative_target_url
                    .set_path(format!("{}{}.{}", self.relative_target_url.path, base_name, extension));
            }
        } else if self.preserve_url_structure && self.target_url.estimate_extension().is_none() {
            // Preserve URL structure: /about → /about/index.html (instead of /about.html)
            // Only for page-like URLs without a real file extension
            if let Some(ref hash) = query_hash {
                self.relative_target_url
                    .set_path(format!("{}/index.{}.html", self.relative_target_url.path, hash));
                self.relative_target_url.set_query(None);
            } else {
                self.relative_target_url
                    .set_path(format!("{}/index.html", self.relative_target_url.path));
            }
        } else {
            // Remove existing extension from path
            let path_without_ext = STRIP_EXT_RE.replace(&self.relative_target_url.path, "").to_string();
            if let Some(ref hash) = query_hash {
                self.relative_target_url
                    .set_path(format!("{}.{}.{}", path_without_ext, hash, extension));
                self.relative_target_url.set_query(None);
            } else {
                self.relative_target_url
                    .set_path(format!("{}.{}", path_without_ext, extension));
            }
        }
    }

    /// Calculate and apply depth for relative path conversion.
    fn calculate_and_apply_depth(&mut self) {
        let base_path_trimmed = self.base_url.path.trim_start_matches(['/', ' ']);
        let base_depth = if base_path_trimmed.is_empty() {
            0usize
        } else {
            base_path_trimmed.matches('/').count()
        };

        match self.target_domain_relation {
            TargetDomainRelation::InitialSameBaseSame | TargetDomainRelation::InitialDifferentBaseSame => {
                if self.relative_target_url.path.starts_with('/') {
                    if base_depth > 0 {
                        self.relative_target_url.change_depth(base_depth as i32);
                    } else {
                        let new_path = self.relative_target_url.path.trim_start_matches('/').to_string();
                        self.relative_target_url.set_path(new_path);
                    }
                }
            }
            TargetDomainRelation::InitialSameBaseDifferent => {
                // backlink from the other domain back to initial domain
                let cleaned_path = DOMAIN_IN_PATH_RE
                    .replace(&self.relative_target_url.path, "")
                    .to_string();
                let cleaned_path = cleaned_path.trim_start_matches(['/', ' ']);
                let prefix = "../".repeat(base_depth + 1);
                self.relative_target_url.set_path(format!("{}{}", prefix, cleaned_path));
            }
            TargetDomainRelation::InitialDifferentBaseDifferent => {
                let extra_depth = if self.base_url.host != self.initial_url.host {
                    1
                } else {
                    0
                };
                let host = self.relative_target_url.host.clone().unwrap_or_default();
                let path = self.relative_target_url.path.clone();
                let prefix = "../".repeat(base_depth + extra_depth);
                self.relative_target_url
                    .set_path(format!("{}_{}{}", prefix, host, path));
            }
        }
    }

    fn is_domain_allowed_for_static_files(&self, domain: &str) -> bool {
        self.callback_is_domain_allowed_for_static_files
            .as_ref()
            .map(|cb| cb(domain))
            .unwrap_or(false)
    }

    fn is_external_domain_allowed_for_crawling(&self, domain: &str) -> bool {
        self.callback_is_external_domain_allowed_for_crawling
            .as_ref()
            .map(|cb| cb(domain))
            .unwrap_or(false)
    }

    /// Sanitize file path and replace special chars.
    pub fn sanitize_file_path(file_path: &str, keep_fragment: bool) -> String {
        // First decode URL-encoded characters
        let file_path = percent_encoding::percent_decode_str(file_path)
            .decode_utf8_lossy()
            .to_string();

        // Parse the file path to extract components
        let (parsed_path, parsed_query, parsed_fragment) = parse_file_path_components(&file_path);

        // Check if path has an extension
        let path_with_extension = RE_PATH_EXTENSION.captures(&parsed_path);

        let mut result = file_path.clone();
        let mut extension: Option<String> = None;

        if let Some(caps) = path_with_extension {
            let start = caps.get(1).map(|m| m.as_str()).unwrap_or("");
            let ext = caps.get(2).map(|m| m.as_str()).unwrap_or("");
            extension = Some(ext.to_string());

            if let Some(ref query_string) = parsed_query {
                let trimmed_query = query_string.trim();
                if !trimmed_query.is_empty() {
                    let query_hash = Self::get_query_hash_from_query_string(trimmed_query);
                    // Only add query hash if it's not empty after processing
                    if !query_hash.trim().is_empty() {
                        result = format!("{}.{}.{}", start, query_hash, ext);
                    } else {
                        result = format!("{}.{}", start, ext);
                    }

                    // add fragment to the end of the file path
                    if keep_fragment && let Some(ref frag) = parsed_fragment {
                        result = format!("{}#{}", result, frag);
                    }
                }
            }
        }

        // Remove characters that are dangerous for filesystems
        let dangerous_characters = ['\\', ':', '*', '?', '"', '<', '>', '|'];
        for ch in &dangerous_characters {
            result = result.replace(*ch, "_");
        }

        // Replace control characters
        result = RE_CONTROL_CHARS.replace_all(&result, "_").to_string();

        // Handle filesystem-specific limitations
        result = result
            .trim_matches(|c: char| c == ' ' || c == '\t' || c == '\n' || c == '\r' || c == '\0' || c == '\x0B')
            .to_string();

        // Replace multiple spaces with single underscore
        result = RE_WHITESPACE.replace_all(&result, "_").to_string();

        // Remove multiple underscores
        result = RE_MULTI_UNDERSCORE.replace_all(&result, "_").to_string();

        // When filepath is too long and there is a long filename, replace filename with shorter md5
        let file_path_for_length = RE_FRAGMENT_SUFFIX.replace(&result, "").to_string();
        if file_path_for_length.len() > 200 {
            let basename = std::path::Path::new(&result)
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("")
                .to_string();
            if basename.len() > 40 {
                let ext = extension
                    .as_deref()
                    .or_else(|| std::path::Path::new(&basename).extension().and_then(|e| e.to_str()))
                    .unwrap_or("html");
                let hash = {
                    use md5::{Digest, Md5};
                    let mut hasher = Md5::new();
                    hasher.update(basename.as_bytes());
                    format!("{:x}", hasher.finalize())
                };
                let short_hash = &hash[..10.min(hash.len())];
                result = result.replace(&basename, &format!("{}.{}", short_hash, ext));
            }
        }

        // Adding "_" to the end of the folder that contains the potential file extension
        result = RE_STATIC_EXT_FOLDER.replace_all(&result, "${1}.${2}_/").to_string();

        // Handle any other dotted folder names that might conflict
        {
            let re = &*RE_DOTTED_FOLDER;

            let result_clone = result.clone();
            let mut new_result = String::new();
            let mut last_end = 0;

            for caps in re.captures_iter(&result_clone) {
                let Some(full_match) = caps.get(0) else {
                    continue;
                };
                let name = caps.get(1).map(|m| m.as_str()).unwrap_or("");
                let ext = caps.get(2).map(|m| m.as_str()).unwrap_or("");

                new_result.push_str(&result_clone[last_end..full_match.start()]);

                // Skip if starts with underscore (domain name)
                if name.starts_with('_') {
                    new_result.push_str(full_match.as_str());
                } else if RE_DOMAIN_TLD.is_match(&format!("{}.{}", name, ext)) {
                    // Skip domain-like names
                    new_result.push_str(full_match.as_str());
                } else if RE_STATIC_EXT_MATCH.is_match(ext) {
                    // Already handled by the previous regex
                    new_result.push_str(full_match.as_str());
                } else {
                    new_result.push_str(&format!("{}.{}_/", name, ext));
                }

                last_end = full_match.end();
            }
            new_result.push_str(&result_clone[last_end..]);
            result = new_result;
        }

        // Replace extensions of typical dynamic pages
        result = RE_DYNAMIC_EXT.replace(&result, ".$1.html").to_string();

        if !keep_fragment && result.contains('#') {
            result = RE_FRAGMENT_SUFFIX.replace(&result, "").to_string();
        }

        // Convert to lowercase if configured
        if let Ok(lc) = LOWERCASE.lock()
            && *lc
        {
            result = result.to_lowercase();
        }

        result
    }

    /// Get query hash from query string.
    fn get_query_hash_from_query_string(query_string: &str) -> String {
        let replace_qs = REPLACE_QUERY_STRING.lock().unwrap_or_else(|e| e.into_inner());
        let has_replacements = !replace_qs.is_empty();

        if has_replacements {
            let replacements = &replace_qs;
            let mut qs = query_string.to_string();

            for replace in replacements.iter() {
                let parts: Vec<&str> = replace.splitn(2, "->").collect();
                let replace_from = parts[0].trim();
                let replace_to = if parts.len() > 1 { parts[1].trim() } else { "" };

                // Check if it's a regex
                let is_regex = crate::utils::is_regex_pattern(replace_from);

                if is_regex {
                    // Extract the pattern from delimiters
                    if let Some(pattern) = extract_regex_pattern(replace_from)
                        && let Ok(re) = Regex::new(&pattern)
                    {
                        qs = re.replace_all(&qs, replace_to).to_string();
                    }
                } else {
                    qs = qs.replace(replace_from, replace_to);
                }
            }

            // replace slashes with '~'
            qs.replace('/', "~")
        } else {
            // Use MD5 hash (first 10 chars)
            let decoded = html_entities_decode(&percent_encoding::percent_decode_str(query_string).decode_utf8_lossy());
            let hash = {
                use md5::{Digest, Md5};
                let mut hasher = Md5::new();
                hasher.update(decoded.as_bytes());
                format!("{:x}", hasher.finalize())
            };
            hash[..10.min(hash.len())].to_string()
        }
    }
}

/// Extract regex pattern from a delimited string (e.g., /pattern/flags)
fn extract_regex_pattern(input: &str) -> Option<String> {
    if input.len() < 2 {
        return None;
    }
    let delimiter = input.chars().next()?;
    let rest = &input[1..];

    // Find the last occurrence of the delimiter
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

/// Parse file path into path, query, and fragment components
fn parse_file_path_components(file_path: &str) -> (String, Option<String>, Option<String>) {
    let mut remaining = file_path;

    // Extract fragment
    let fragment = if let Some(hash_pos) = remaining.find('#') {
        let f = &remaining[hash_pos + 1..];
        remaining = &remaining[..hash_pos];
        if f.is_empty() { None } else { Some(f.to_string()) }
    } else {
        None
    };

    // Extract query
    let query = if let Some(q_pos) = remaining.find('?') {
        let q = &remaining[q_pos + 1..];
        remaining = &remaining[..q_pos];
        if q.is_empty() { None } else { Some(q.to_string()) }
    } else {
        None
    };

    (remaining.to_string(), query, fragment)
}

/// Decode HTML entities
fn html_entities_decode(input: &str) -> String {
    input
        .replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&#039;", "'")
}

#[cfg(test)]
mod tests {
    use super::*;

    // Helper: create converter with siteone.io as initial URL, with domain allow callbacks
    fn make_converter(initial: &str, base: &str, target: &str, attribute: Option<&str>) -> OfflineUrlConverter {
        let initial_url = ParsedUrl::parse(initial, None);
        let base_url = ParsedUrl::parse(base, None);
        let base_url_ref = if target.starts_with("//")
            || target.starts_with("http")
            || target.starts_with('#')
            || target.starts_with('?')
        {
            None
        } else {
            Some(&base_url)
        };
        let target_url = ParsedUrl::parse(target, base_url_ref);

        let allowed_static: Box<dyn Fn(&str) -> bool + Send + Sync> =
            Box::new(|domain: &str| matches!(domain, "cdn.siteone.io" | "cdn.webflow.com" | "nextjs.org"));
        let allowed_crawling: Box<dyn Fn(&str) -> bool + Send + Sync> =
            Box::new(|domain: &str| matches!(domain, "svelte.dev" | "nextjs.org"));

        OfflineUrlConverter::new(
            initial_url,
            base_url,
            target_url,
            Some(allowed_static),
            Some(allowed_crawling),
            attribute,
        )
    }

    fn convert(initial: &str, base: &str, target: &str, attribute: Option<&str>) -> String {
        let mut converter = make_converter(initial, base, target, attribute);
        converter.convert_url_to_relative(true)
    }

    // =========================================================================
    // getOfflineBaseUrlDepth tests
    // =========================================================================

    #[test]
    fn depth_root() {
        assert_eq!(
            OfflineUrlConverter::get_offline_base_url_depth(&ParsedUrl::parse("/", None)),
            0
        );
    }

    #[test]
    fn depth_file() {
        assert_eq!(
            OfflineUrlConverter::get_offline_base_url_depth(&ParsedUrl::parse("/foo", None)),
            0
        );
    }

    #[test]
    fn depth_dir() {
        assert_eq!(
            OfflineUrlConverter::get_offline_base_url_depth(&ParsedUrl::parse("/foo/", None)),
            1
        );
    }

    #[test]
    fn depth_file_in_dir() {
        assert_eq!(
            OfflineUrlConverter::get_offline_base_url_depth(&ParsedUrl::parse("/foo/bar", None)),
            1
        );
    }

    #[test]
    fn depth_nested_dir() {
        assert_eq!(
            OfflineUrlConverter::get_offline_base_url_depth(&ParsedUrl::parse("/foo/bar/", None)),
            2
        );
    }

    #[test]
    fn depth_root_with_query() {
        // /?param=1 → /index.queryMd5Hash.html → depth 0
        assert_eq!(
            OfflineUrlConverter::get_offline_base_url_depth(&ParsedUrl::parse("/?param=1", None)),
            0
        );
    }

    #[test]
    fn depth_file_with_query() {
        // /foo?param=1 → /foo.queryMd5Hash.html → depth 0
        assert_eq!(
            OfflineUrlConverter::get_offline_base_url_depth(&ParsedUrl::parse("/foo?param=1", None)),
            0
        );
    }

    #[test]
    fn depth_dir_with_query() {
        // /foo/?param=1 → /foo/index.queryMd5Hash.html → depth 1
        assert_eq!(
            OfflineUrlConverter::get_offline_base_url_depth(&ParsedUrl::parse("/foo/?param=1", None)),
            1
        );
    }

    #[test]
    fn depth_file_in_dir_with_query() {
        // /foo/bar?param=1 → /foo/bar.queryMd5Hash.html → depth 1
        assert_eq!(
            OfflineUrlConverter::get_offline_base_url_depth(&ParsedUrl::parse("/foo/bar?param=1", None)),
            1
        );
    }

    #[test]
    fn depth_nested_dir_with_query() {
        // /foo/bar/?param=1 → /foo/bar/index.queryMd5Hash.html → depth 2
        assert_eq!(
            OfflineUrlConverter::get_offline_base_url_depth(&ParsedUrl::parse("/foo/bar/?param=1", None)),
            2
        );
    }

    // =========================================================================
    // Core URL-to-file conversion tests (the most critical ones)
    // =========================================================================

    #[test]
    fn convert_root_to_root() {
        assert_eq!(
            convert(
                "https://siteone.io/",
                "https://siteone.io/",
                "https://siteone.io/",
                None
            ),
            "index.html"
        );
    }

    #[test]
    fn convert_root_page() {
        assert_eq!(
            convert(
                "https://siteone.io/",
                "https://siteone.io/",
                "https://siteone.io/page",
                None
            ),
            "page.html"
        );
    }

    #[test]
    fn convert_root_page_trailing_slash() {
        assert_eq!(
            convert(
                "https://siteone.io/",
                "https://siteone.io",
                "https://siteone.io/page/",
                None
            ),
            "page/index.html"
        );
    }

    #[test]
    fn convert_from_subdir_with_fragment() {
        let result = convert(
            "https://siteone.io/",
            "https://siteone.io/t/",
            "https://siteone.io/page#fragment",
            None,
        );
        assert_eq!(result, "../page.html#fragment");
    }

    #[test]
    fn convert_relative_page() {
        assert_eq!(
            convert("https://siteone.io/", "https://siteone.io/", "/page", None),
            "page.html"
        );
    }

    #[test]
    fn convert_relative_page_dir() {
        assert_eq!(
            convert("https://siteone.io/", "https://siteone.io/", "/page/", None),
            "page/index.html"
        );
    }

    #[test]
    fn convert_relative_plain() {
        assert_eq!(
            convert("https://siteone.io/", "https://siteone.io/", "page", None),
            "page.html"
        );
    }

    #[test]
    fn convert_relative_parent() {
        assert_eq!(
            convert("https://siteone.io/", "https://siteone.io/path/", "../page", None),
            "../page.html"
        );
    }

    #[test]
    fn convert_relative_parent_dir() {
        assert_eq!(
            convert("https://siteone.io/", "https://siteone.io/path/", "../page/", None),
            "../page/index.html"
        );
    }

    #[test]
    fn convert_from_subpath_same_dir() {
        assert_eq!(
            convert(
                "https://siteone.io/",
                "https://siteone.io/path/",
                "https://siteone.io/path/page",
                None
            ),
            "../path/page.html"
        );
    }

    // ---- External domains ----

    #[test]
    fn convert_external_allowed_domain_root() {
        assert_eq!(
            convert(
                "https://siteone.io/",
                "https://siteone.io/",
                "https://nextjs.org/",
                None
            ),
            "_nextjs.org/index.html"
        );
    }

    #[test]
    fn convert_external_allowed_domain_from_subdir() {
        assert_eq!(
            convert(
                "https://siteone.io/",
                "https://siteone.io/t/",
                "https://svelte.dev/x",
                None
            ),
            "../_svelte.dev/x.html"
        );
    }

    #[test]
    fn convert_external_css_file() {
        assert_eq!(
            convert(
                "https://siteone.io/",
                "https://siteone.io/t/",
                "https://svelte.dev/x/file.css",
                None
            ),
            "../_svelte.dev/x/file.css"
        );
    }

    // ---- Backlinks ----

    #[test]
    fn convert_backlink_to_initial_domain() {
        assert_eq!(
            convert(
                "https://siteone.io/",
                "https://nextjs.org/",
                "https://siteone.io/",
                None
            ),
            "../index.html"
        );
    }

    #[test]
    fn convert_backlink_subpage_to_initial() {
        assert_eq!(
            convert(
                "https://siteone.io/",
                "https://nextjs.org/subpage",
                "https://siteone.io/",
                None
            ),
            "../index.html"
        );
    }

    #[test]
    fn convert_backlink_subdir_to_initial() {
        assert_eq!(
            convert(
                "https://siteone.io/",
                "https://nextjs.org/subpage/",
                "https://siteone.io/a",
                None
            ),
            "../../a.html"
        );
    }

    #[test]
    fn convert_backlink_to_third_domain() {
        assert_eq!(
            convert(
                "https://siteone.io/",
                "https://nextjs.org/",
                "https://svelte.dev/page",
                None
            ),
            "../_svelte.dev/page.html"
        );
    }

    // ---- Protocol-relative ----

    #[test]
    fn convert_protocol_relative_external() {
        assert_eq!(
            convert("https://siteone.io/", "https://siteone.io/", "//nextjs.org/", None),
            "_nextjs.org/index.html"
        );
    }

    #[test]
    fn convert_protocol_relative_backlink() {
        assert_eq!(
            convert("https://siteone.io/", "https://nextjs.org/", "//siteone.io/page", None),
            "../page.html"
        );
    }

    // ---- Fragment only ----

    #[test]
    fn convert_fragment_only() {
        assert_eq!(
            convert("https://siteone.io/", "https://siteone.io/", "#fragment2", None),
            "#fragment2"
        );
    }

    #[test]
    fn convert_fragment_only_external() {
        assert_eq!(
            convert("https://siteone.io/", "https://nextjs.org/", "#fragment3", None),
            "#fragment3"
        );
    }

    // ---- Query string handling (md5 hash) ----

    #[test]
    fn convert_page_with_query() {
        let result = convert(
            "https://siteone.io/",
            "https://siteone.io/",
            "https://siteone.io/page?p=1",
            None,
        );
        // Should have query hash between basename and extension: page.HASH.html
        assert!(
            result.starts_with("page."),
            "expected 'page.HASH.html', got '{}'",
            result
        );
        assert!(result.ends_with(".html"), "expected '*.html', got '{}'", result);
        assert!(!result.contains('?'));
    }

    #[test]
    fn convert_query_only() {
        let result = convert("https://siteone.io/", "https://siteone.io/", "?p=1", None);
        // Should be: index.HASH.html
        assert!(
            result.starts_with("index."),
            "expected 'index.HASH.html', got '{}'",
            result
        );
        assert!(result.ends_with(".html"), "expected '*.html', got '{}'", result);
    }

    #[test]
    fn convert_css_with_query() {
        let result = convert(
            "https://siteone.io/",
            "https://siteone.io/",
            "https://siteone.io/file.css?p=1",
            None,
        );
        // Should be: file.HASH.css
        assert!(result.ends_with(".css"), "expected '*.css', got '{}'", result);
        assert!(!result.contains('?'));
    }

    // ---- Complex relative paths ----

    #[test]
    fn convert_double_parent_relative() {
        assert_eq!(
            convert(
                "https://siteone.io/",
                "https://siteone.io/path/more/",
                "../../page",
                None
            ),
            "../../page.html"
        );
    }

    #[test]
    fn convert_double_parent_relative_dir() {
        assert_eq!(
            convert(
                "https://siteone.io/",
                "https://siteone.io/path/more/",
                "../../page/",
                None
            ),
            "../../page/index.html"
        );
    }

    // ---- External CSS references ----

    #[test]
    fn convert_from_external_css_to_external_image() {
        let result = convert(
            "https://siteone.io/",
            "https://cdn.siteone.io/siteone.io/css/styles.css",
            "https://cdn.webflow.com/a/b1.jpg",
            None,
        );
        assert_eq!(result, "../../../_cdn.webflow.com/a/b1.jpg");
    }

    #[test]
    fn convert_from_deep_external_css_to_image() {
        let result = convert(
            "https://siteone.io/",
            "https://cdn.siteone.io/siteone.io/css/hello/hi/styles.css",
            "https://cdn.webflow.com/b2.jpg",
            None,
        );
        assert_eq!(result, "../../../../../_cdn.webflow.com/b2.jpg");
    }

    #[test]
    fn convert_from_external_css_to_initial_domain() {
        let result = convert(
            "https://siteone.io/",
            "https://cdn.siteone.io/siteone.io/css/hello/hi/styles.css",
            "https://siteone.io/test/image.jpg",
            None,
        );
        assert_eq!(result, "../../../../../test/image.jpg");
    }

    #[test]
    fn convert_from_external_css_relative_root() {
        let result = convert(
            "https://siteone.io/",
            "https://cdn.siteone.io/siteone.io/css/styles.css",
            "/abt.jpg",
            None,
        );
        assert_eq!(result, "../../abt.jpg");
    }

    #[test]
    fn convert_from_external_css_relative_parent() {
        let result = convert(
            "https://siteone.io/",
            "https://cdn.siteone.io/siteone.io/css/styles.css",
            "../abz.jpg",
            None,
        );
        assert_eq!(result, "../abz.jpg");
    }

    // ---- Unknown/not-allowed domains → keep absolute ----

    #[test]
    fn convert_unknown_domain_stays_absolute() {
        let result = convert(
            "https://siteone.io/",
            "https://siteone.io/",
            "https://unknown.com/",
            None,
        );
        assert_eq!(result, "https://unknown.com/");
    }

    #[test]
    fn convert_unknown_domain_http_stays_absolute() {
        let result = convert(
            "https://siteone.io/",
            "https://siteone.io/",
            "http://unknown.com/page",
            None,
        );
        assert_eq!(result, "http://unknown.com/page");
    }

    // =========================================================================
    // sanitizeFilePath (UTF-8 subset)
    // =========================================================================

    #[test]
    fn sanitize_utf8_czech() {
        assert_eq!(
            OfflineUrlConverter::sanitize_file_path("české-výrobky", false),
            "české-výrobky"
        );
    }

    #[test]
    fn sanitize_utf8_german() {
        assert_eq!(OfflineUrlConverter::sanitize_file_path("über-uns", false), "über-uns");
    }

    #[test]
    fn sanitize_utf8_chinese() {
        assert_eq!(OfflineUrlConverter::sanitize_file_path("电子产品", false), "电子产品");
    }

    #[test]
    fn sanitize_url_encoded_czech() {
        assert_eq!(
            OfflineUrlConverter::sanitize_file_path("%C4%8Desk%C3%A9-v%C3%BDrobky", false),
            "české-výrobky"
        );
    }

    #[test]
    fn sanitize_url_encoded_german() {
        assert_eq!(
            OfflineUrlConverter::sanitize_file_path("%C3%BCber-uns", false),
            "über-uns"
        );
    }

    #[test]
    fn sanitize_url_encoded_chinese() {
        assert_eq!(
            OfflineUrlConverter::sanitize_file_path("%E7%94%B5%E5%AD%90%E4%BA%A7%E5%93%81", false),
            "电子产品"
        );
    }

    #[test]
    fn sanitize_dangerous_chars_colon() {
        assert_eq!(
            OfflineUrlConverter::sanitize_file_path("file:with:colons", false),
            "file_with_colons"
        );
    }

    #[test]
    fn sanitize_dangerous_chars_asterisk() {
        assert_eq!(
            OfflineUrlConverter::sanitize_file_path("file*with*asterisks", false),
            "file_with_asterisks"
        );
    }

    #[test]
    fn sanitize_dangerous_chars_question() {
        assert_eq!(
            OfflineUrlConverter::sanitize_file_path("file?with?questions", false),
            "file_with_questions"
        );
    }

    #[test]
    fn sanitize_dangerous_chars_quotes() {
        assert_eq!(
            OfflineUrlConverter::sanitize_file_path("file\"with\"quotes", false),
            "file_with_quotes"
        );
    }

    #[test]
    fn sanitize_dangerous_chars_brackets() {
        assert_eq!(
            OfflineUrlConverter::sanitize_file_path("file<with>brackets", false),
            "file_with_brackets"
        );
    }

    #[test]
    fn sanitize_dangerous_chars_pipes() {
        assert_eq!(
            OfflineUrlConverter::sanitize_file_path("file|with|pipes", false),
            "file_with_pipes"
        );
    }

    #[test]
    fn sanitize_dangerous_chars_backslash() {
        assert_eq!(
            OfflineUrlConverter::sanitize_file_path("file\\with\\backslashes", false),
            "file_with_backslashes"
        );
    }

    #[test]
    fn sanitize_mixed_utf8_and_dangerous() {
        assert_eq!(
            OfflineUrlConverter::sanitize_file_path("české:výrobky", false),
            "české_výrobky"
        );
    }

    #[test]
    fn sanitize_empty() {
        assert_eq!(OfflineUrlConverter::sanitize_file_path("", false), "");
    }

    #[test]
    fn sanitize_dots() {
        assert_eq!(OfflineUrlConverter::sanitize_file_path(".", false), ".");
        assert_eq!(OfflineUrlConverter::sanitize_file_path("..", false), "..");
    }

    // =========================================================================
    // Direct OfflineUrlConverter URL conversion tests
    // =========================================================================

    fn convert_simple(base: &str, target: &str) -> String {
        let initial_url = ParsedUrl::parse("https://example.com/", None);
        let base_url = ParsedUrl::parse(base, None);
        let base_url_for_ref = ParsedUrl::parse(base, None);
        let target_url = ParsedUrl::parse(target, Some(&base_url_for_ref));

        let false_cb1: Box<dyn Fn(&str) -> bool + Send + Sync> = Box::new(|_| false);
        let false_cb2: Box<dyn Fn(&str) -> bool + Send + Sync> = Box::new(|_| false);

        let mut converter = OfflineUrlConverter::new(
            initial_url,
            base_url,
            target_url,
            Some(false_cb1),
            Some(false_cb2),
            None,
        );
        converter.convert_url_to_relative(false)
    }

    #[test]
    fn simple_from_subdir_to_root_asset() {
        assert_eq!(
            convert_simple("https://example.com/page/", "/style.css"),
            "../style.css"
        );
    }

    #[test]
    fn simple_from_subdir_to_root_image() {
        assert_eq!(
            convert_simple("https://example.com/page/", "/images/logo.png"),
            "../images/logo.png"
        );
    }

    #[test]
    fn simple_from_deep_subdir_to_root_asset() {
        assert_eq!(
            convert_simple("https://example.com/dir/page/", "/style.css"),
            "../../style.css"
        );
    }

    #[test]
    fn simple_from_root_to_root_asset() {
        assert_eq!(convert_simple("https://example.com/", "/style.css"), "style.css");
    }

    #[test]
    fn simple_from_root_to_subdir_image() {
        assert_eq!(
            convert_simple("https://example.com/", "/images/logo.png"),
            "images/logo.png"
        );
    }

    // ---- UTF-8 URL conversion ----

    fn convert_utf8(base: &str, target: &str) -> String {
        let initial_url = ParsedUrl::parse("https://example.com/", None);
        let base_url = ParsedUrl::parse(base, None);
        let target_url = ParsedUrl::parse(target, None);

        let false_cb1: Box<dyn Fn(&str) -> bool + Send + Sync> = Box::new(|_| false);
        let false_cb2: Box<dyn Fn(&str) -> bool + Send + Sync> = Box::new(|_| false);

        let mut converter = OfflineUrlConverter::new(
            initial_url,
            base_url,
            target_url,
            Some(false_cb1),
            Some(false_cb2),
            None,
        );
        converter.convert_url_to_relative(true)
    }

    #[test]
    fn utf8_czech_from_root() {
        assert_eq!(
            convert_utf8("https://example.com/", "https://example.com/české-výrobky"),
            "české-výrobky.html"
        );
    }

    #[test]
    fn utf8_czech_in_subdir() {
        assert_eq!(
            convert_utf8("https://example.com/", "https://example.com/products/české-výrobky"),
            "products/české-výrobky.html"
        );
    }

    #[test]
    fn utf8_german_from_root() {
        assert_eq!(
            convert_utf8("https://example.com/", "https://example.com/über-uns"),
            "über-uns.html"
        );
    }

    #[test]
    fn utf8_chinese_from_root() {
        assert_eq!(
            convert_utf8("https://example.com/", "https://example.com/电子产品"),
            "电子产品.html"
        );
    }

    #[test]
    fn utf8_czech_trailing_slash() {
        assert_eq!(
            convert_utf8("https://example.com/", "https://example.com/české-výrobky/"),
            "české-výrobky/index.html"
        );
    }

    #[test]
    fn utf8_chinese_trailing_slash() {
        assert_eq!(
            convert_utf8("https://example.com/", "https://example.com/电子产品/"),
            "电子产品/index.html"
        );
    }

    #[test]
    fn utf8_czech_from_subdir() {
        assert_eq!(
            convert_utf8("https://example.com/page/", "https://example.com/české-výrobky"),
            "../české-výrobky.html"
        );
    }

    #[test]
    fn utf8_chinese_from_subdir() {
        assert_eq!(
            convert_utf8("https://example.com/dir/", "https://example.com/电子产品"),
            "../电子产品.html"
        );
    }

    #[test]
    fn utf8_czech_with_fragment() {
        assert_eq!(
            convert_utf8("https://example.com/", "https://example.com/české#sekce"),
            "české.html#sekce"
        );
    }

    // =========================================================================
    // Existing tests preserved
    // =========================================================================

    #[test]
    fn test_sanitize_file_path_basic() {
        let result = OfflineUrlConverter::sanitize_file_path("/index.html", true);
        assert_eq!(result, "/index.html");
    }

    #[test]
    fn test_sanitize_file_path_with_query() {
        let result = OfflineUrlConverter::sanitize_file_path("/page.html?foo=bar", false);
        assert!(result.contains(".html"));
        assert!(!result.contains('?'));
    }

    #[test]
    fn test_extract_regex_pattern() {
        assert_eq!(extract_regex_pattern("/foo/i"), Some("(?i)foo".to_string()));
        assert_eq!(extract_regex_pattern("/bar/"), Some("bar".to_string()));
        assert_eq!(extract_regex_pattern("#test#"), Some("test".to_string()));
    }

    // =========================================================================
    // Preserve URL structure tests (--offline-export-preserve-url-structure)
    // =========================================================================

    fn convert_preserve(initial: &str, base: &str, target: &str) -> String {
        let initial_url = ParsedUrl::parse(initial, None);
        let base_url = ParsedUrl::parse(base, None);
        let target_url = ParsedUrl::parse(target, None);

        let false_cb1: Box<dyn Fn(&str) -> bool + Send + Sync> = Box::new(|_| false);
        let false_cb2: Box<dyn Fn(&str) -> bool + Send + Sync> = Box::new(|_| false);

        let mut converter = OfflineUrlConverter::new(
            initial_url,
            base_url,
            target_url,
            Some(false_cb1),
            Some(false_cb2),
            None,
        );
        converter.set_preserve_url_structure(true);
        converter.convert_url_to_relative(true)
    }

    #[test]
    fn preserve_extensionless_page_becomes_dir_index() {
        // /about → /about/index.html (not /about.html)
        assert_eq!(
            convert_preserve(
                "https://example.com/",
                "https://example.com/",
                "https://example.com/about"
            ),
            "about/index.html"
        );
    }

    #[test]
    fn preserve_trailing_slash_unchanged() {
        // /about/ → /about/index.html (same as without preserve)
        assert_eq!(
            convert_preserve(
                "https://example.com/",
                "https://example.com/",
                "https://example.com/about/"
            ),
            "about/index.html"
        );
    }

    #[test]
    fn preserve_with_real_extension_unchanged() {
        // /style.css → /style.css (not /style.css/index.html)
        assert_eq!(
            convert_preserve(
                "https://example.com/",
                "https://example.com/",
                "https://example.com/style.css"
            ),
            "style.css"
        );
    }

    #[test]
    fn preserve_nested_path() {
        // /docs/guide → /docs/guide/index.html
        assert_eq!(
            convert_preserve(
                "https://example.com/",
                "https://example.com/",
                "https://example.com/docs/guide"
            ),
            "docs/guide/index.html"
        );
    }

    #[test]
    fn preserve_with_query_string() {
        // /about?lang=en → /about/index.HASH.html
        let result = convert_preserve(
            "https://example.com/",
            "https://example.com/",
            "https://example.com/about?lang=en",
        );
        assert!(
            result.starts_with("about/index."),
            "expected 'about/index.HASH.html', got '{}'",
            result
        );
        assert!(result.ends_with(".html"), "expected '*.html', got '{}'", result);
        assert!(!result.contains('?'));
    }

    #[test]
    fn preserve_root_page_unchanged() {
        // / → index.html (root is always index.html)
        assert_eq!(
            convert_preserve("https://example.com/", "https://example.com/", "https://example.com/"),
            "index.html"
        );
    }
}
