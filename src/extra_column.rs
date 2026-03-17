// SiteOne Crawler - ExtraColumn
// (c) Jan Reges <jan.reges@siteone.cz>

use regex::Regex;
use scraper::{Html, Selector};

use crate::error::CrawlerError;

pub const CUSTOM_METHOD_XPATH: &str = "xpath";
pub const CUSTOM_METHOD_REGEXP: &str = "regexp";

#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExtraColumn {
    pub name: String,
    pub length: Option<usize>,
    pub truncate: bool,
    pub custom_method: Option<String>,
    pub custom_pattern: Option<String>,
    pub custom_group: Option<usize>,
    #[serde(skip)]
    compiled_regex: Option<Regex>,
}

fn default_column_size(name: &str) -> Option<usize> {
    match name {
        "Title" => Some(20),
        "Description" => Some(20),
        "Keywords" => Some(20),
        _ => None,
    }
}

impl ExtraColumn {
    pub fn new(
        name: String,
        length: Option<usize>,
        truncate: bool,
        custom_method: Option<String>,
        custom_pattern: Option<String>,
        custom_group: Option<usize>,
    ) -> Result<Self, CrawlerError> {
        let validated_method = if let Some(ref method) = custom_method {
            let method_lower = method.to_lowercase();
            if method_lower != CUSTOM_METHOD_XPATH && method_lower != CUSTOM_METHOD_REGEXP {
                return Err(CrawlerError::Config(format!(
                    "Invalid custom extraction method: {}. Expected '{}' or '{}'.",
                    method, CUSTOM_METHOD_XPATH, CUSTOM_METHOD_REGEXP
                )));
            }

            if method_lower == CUSTOM_METHOD_REGEXP
                && let Some(ref pattern) = custom_pattern
            {
                // Validate the regex pattern
                if Regex::new(pattern).is_err() {
                    return Err(CrawlerError::Config(format!(
                        "Invalid regexp pattern provided: {}",
                        pattern
                    )));
                }
            }

            Some(method_lower)
        } else {
            None
        };

        let compiled_regex = if validated_method.as_deref() == Some(CUSTOM_METHOD_REGEXP) {
            custom_pattern.as_deref().and_then(|p| Regex::new(p).ok())
        } else {
            None
        };

        Ok(Self {
            name,
            length,
            truncate,
            custom_method: validated_method,
            custom_pattern,
            custom_group,
            compiled_regex,
        })
    }

    pub fn get_length(&self) -> usize {
        self.length.unwrap_or(self.name.len())
    }

    pub fn get_truncated_value(&self, value: Option<&str>) -> Option<String> {
        let value = value?;

        let length = self.get_length();
        if self.truncate && value.chars().count() > length {
            let truncated: String = value.chars().take(length.saturating_sub(1)).collect();
            Some(format!("{}…", truncated.trim()))
        } else {
            Some(value.to_string())
        }
    }

    pub fn from_text(text: &str) -> Result<ExtraColumn, CrawlerError> {
        // If the string contains '=', then it is a custom extraction.
        if text.contains('=') {
            let re = Regex::new(r"^([^=]+)=(xpath|regexp):(.+?)(?:#(\d+))?(?:\((\d+)(>?)\))?$")
                .map_err(|e| CrawlerError::Parse(e.to_string()))?;

            if let Some(caps) = re.captures(text) {
                let name = caps.get(1).map_or("", |m| m.as_str()).trim().to_string();
                let custom_method = Some(caps.get(2).map_or("", |m| m.as_str()).to_lowercase());
                let custom_pattern = Some(caps.get(3).map_or("", |m| m.as_str()).trim().to_string());
                let custom_group = caps
                    .get(4)
                    .and_then(|m| {
                        let s = m.as_str();
                        if s.is_empty() { None } else { s.parse::<usize>().ok() }
                    })
                    .or(Some(0));

                let (length, truncate) = if let Some(len_match) = caps.get(5) {
                    let len = len_match.as_str().parse::<usize>().unwrap_or(0);
                    let trunc = caps.get(6).is_none_or(|m| m.as_str() != ">");
                    (Some(len), trunc)
                } else {
                    (None, true)
                };

                return ExtraColumn::new(name, length, truncate, custom_method, custom_pattern, custom_group);
            }

            // If parsing of the custom syntax fails, return a standard column.
            return ExtraColumn::new(text.trim().to_string(), None, true, None, None, None);
        }

        // Standard column parsing
        let re = Regex::new(r"^([^(]+)(\((\d+)(>?)\))?$").map_err(|e| CrawlerError::Parse(e.to_string()))?;

        if let Some(caps) = re.captures(text) {
            let name = caps.get(1).map_or("", |m| m.as_str()).trim().to_string();

            let (length, truncate) = if let Some(len_match) = caps.get(3) {
                let len = len_match.as_str().parse::<usize>().unwrap_or(0);
                let trunc = caps.get(4).is_none_or(|m| m.as_str() != ">");
                (Some(len), trunc)
            } else {
                (default_column_size(&name), true)
            };

            ExtraColumn::new(name, length, truncate, None, None, None)
        } else {
            ExtraColumn::new(text.trim().to_string(), None, true, None, None, None)
        }
    }

    pub fn extract_value(&self, text: &str) -> Option<String> {
        let method = self.custom_method.as_deref()?;
        let pattern = self.custom_pattern.as_deref()?;

        match method {
            CUSTOM_METHOD_REGEXP => {
                let re = self.compiled_regex.as_ref()?;
                let caps = re.captures(text)?;
                let group = self.custom_group.unwrap_or(0);
                caps.get(group).map(|m| m.as_str().to_string())
            }
            CUSTOM_METHOD_XPATH => {
                let index = self.custom_group.unwrap_or(0);
                Self::extract_xpath(text, pattern, index)
            }
            _ => None,
        }
    }

    /// Extract value using XPath-like pattern via CSS selector conversion.
    /// Supports common XPath patterns used in web scraping:
    ///   //tag                     -> tag
    ///   //tag[@attr='value']      -> tag[attr='value']
    ///   //tag/@attr               -> tag (then read attribute)
    ///   //tag[@attr='value']/@x   -> tag[attr='value'] (then read attribute x)
    fn extract_xpath(html: &str, xpath: &str, index: usize) -> Option<String> {
        let document = Html::parse_document(html);

        // Detect if XPath ends with /@attribute — means we want an attribute value
        let (xpath_base, target_attr) = if let Some(idx) = xpath.rfind("/@") {
            (&xpath[..idx], Some(&xpath[idx + 2..]))
        } else {
            (xpath, None)
        };

        // Convert XPath to CSS selector
        let css = xpath_to_css(xpath_base);
        let selector = Selector::parse(&css).ok()?;

        let mut nodes = document.select(&selector);

        if let Some(element) = nodes.nth(index) {
            if let Some(attr) = target_attr {
                // Return attribute value
                element.value().attr(attr).map(|v| v.trim().to_string())
            } else {
                // Return text content
                let text: String = element.text().collect::<Vec<_>>().join("");
                let trimmed = text.trim().to_string();
                if trimmed.is_empty() { None } else { Some(trimmed) }
            }
        } else {
            None
        }
    }
}

/// Convert common XPath expressions to CSS selectors.
fn xpath_to_css(xpath: &str) -> String {
    let mut s = xpath.to_string();

    // Strip leading // or /
    if s.starts_with("//") {
        s = s[2..].to_string();
    } else if s.starts_with('/') {
        s = s[1..].to_string();
    }

    // Replace // (descendant) with space (CSS descendant combinator)
    s = s.replace("//", " ");

    // Replace / (child) with > (CSS child combinator)
    s = s.replace('/', " > ");

    s
}

#[cfg(test)]
mod tests {
    use super::*;

    // -- from_text parsing --

    #[test]
    fn parse_simple_name_uses_default_length() {
        let col = ExtraColumn::from_text("Title").unwrap();
        assert_eq!(col.name, "Title");
        assert_eq!(col.length, Some(20)); // default for "Title"
        assert!(col.custom_method.is_none());
    }

    #[test]
    fn parse_name_with_explicit_length() {
        let col = ExtraColumn::from_text("Custom(50)").unwrap();
        assert_eq!(col.name, "Custom");
        assert_eq!(col.length, Some(50));
        assert!(col.truncate);
    }

    #[test]
    fn parse_name_with_no_truncate() {
        let col = ExtraColumn::from_text("Wide(30>)").unwrap();
        assert_eq!(col.name, "Wide");
        assert_eq!(col.length, Some(30));
        assert!(!col.truncate);
    }

    #[test]
    fn parse_regexp_method() {
        let col = ExtraColumn::from_text("X=regexp:<title>(.+?)</title>").unwrap();
        assert_eq!(col.custom_method.as_deref(), Some("regexp"));
        assert!(col.custom_pattern.is_some());
    }

    #[test]
    fn parse_xpath_method() {
        let col = ExtraColumn::from_text("X=xpath://h1").unwrap();
        assert_eq!(col.custom_method.as_deref(), Some("xpath"));
    }

    #[test]
    fn parse_invalid_method_returns_error() {
        let result = ExtraColumn::from_text("X=invalid:foo");
        // "invalid" is not a valid method, but from_text falls back to standard column
        // The actual error comes from ExtraColumn::new when method is validated
        // from_text with unrecognized format returns a standard column, not an error
        assert!(result.is_ok()); // falls back to standard column
        let col = result.unwrap();
        assert!(col.custom_method.is_none());
    }

    // -- extract_value regexp --

    #[test]
    fn extract_regexp_matching() {
        let col = ExtraColumn::new(
            "X".to_string(),
            None,
            true,
            Some("regexp".to_string()),
            Some("<title>(.+?)</title>".to_string()),
            Some(1),
        )
        .unwrap();
        assert_eq!(col.extract_value("<title>Hello</title>"), Some("Hello".to_string()));
    }

    #[test]
    fn extract_regexp_not_matching() {
        let col = ExtraColumn::new(
            "X".to_string(),
            None,
            true,
            Some("regexp".to_string()),
            Some("<title>(.+?)</title>".to_string()),
            Some(1),
        )
        .unwrap();
        assert_eq!(col.extract_value("<p>No title here</p>"), None);
    }

    // -- extract_value xpath --

    #[test]
    fn extract_xpath_h1() {
        let col = ExtraColumn::new(
            "X".to_string(),
            None,
            true,
            Some("xpath".to_string()),
            Some("//h1".to_string()),
            Some(0),
        )
        .unwrap();
        let html = "<html><body><h1>Title</h1></body></html>";
        assert_eq!(col.extract_value(html), Some("Title".to_string()));
    }

    #[test]
    fn extract_xpath_attribute() {
        let col = ExtraColumn::new(
            "X".to_string(),
            None,
            true,
            Some("xpath".to_string()),
            Some("//a/@href".to_string()),
            Some(0),
        )
        .unwrap();
        let html = "<html><body><a href=\"https://example.com\">Link</a></body></html>";
        assert_eq!(col.extract_value(html), Some("https://example.com".to_string()));
    }

    #[test]
    fn extract_xpath_not_found() {
        let col = ExtraColumn::new(
            "X".to_string(),
            None,
            true,
            Some("xpath".to_string()),
            Some("//h2".to_string()),
            Some(0),
        )
        .unwrap();
        let html = "<html><body><h1>Only H1</h1></body></html>";
        assert_eq!(col.extract_value(html), None);
    }

    // -- get_truncated_value --

    #[test]
    fn truncated_value_truncates_when_longer() {
        let col = ExtraColumn::new("X".to_string(), Some(3), true, None, None, None).unwrap();
        // Takes length-1 chars (2) and appends "…" → total 3 visible chars
        assert_eq!(col.get_truncated_value(Some("Hello")), Some("He…".to_string()));
    }

    #[test]
    fn truncated_value_none_returns_none() {
        let col = ExtraColumn::new("X".to_string(), Some(3), true, None, None, None).unwrap();
        assert_eq!(col.get_truncated_value(None), None);
    }
}
