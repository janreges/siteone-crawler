// SiteOne Crawler - HttpResponse
// (c) Jan Reges <jan.reges@siteone.cz>

use std::collections::HashMap;

use crate::utils;

/// HTTP response from the crawler's HTTP client.
/// Body is stored as raw bytes (`Vec<u8>`) to preserve binary data (images, fonts, etc.)
/// without UTF-8 corruption. Use `body_text()` when you need a String for text processing.
#[derive(Debug, Clone)]
pub struct HttpResponse {
    pub url: String,
    pub status_code: i32,
    pub body: Option<Vec<u8>>,
    pub headers: HashMap<String, String>,
    pub exec_time: f64,
    pub skipped_reason: Option<String>,
    loaded_from_cache: bool,
}

impl HttpResponse {
    pub fn new(
        url: String,
        status_code: i32,
        body: Option<Vec<u8>>,
        headers: HashMap<String, Vec<String>>,
        exec_time: f64,
    ) -> Self {
        let (status_code, body, headers) = Self::detect_redirect_and_set_meta_redirect(status_code, body, headers);

        let flat_headers = utils::get_flat_response_headers(&headers);

        Self {
            url,
            status_code,
            body,
            headers: flat_headers,
            exec_time,
            skipped_reason: None,
            loaded_from_cache: false,
        }
    }

    /// Get body as text (lossy UTF-8 conversion). Use for HTML/CSS/JS processing.
    pub fn body_text(&self) -> Option<String> {
        self.body.as_ref().map(|b| String::from_utf8_lossy(b).into_owned())
    }

    pub fn get_formatted_exec_time(&self) -> String {
        utils::get_formatted_duration(self.exec_time)
    }

    pub fn get_formatted_body_length(&self) -> String {
        let len = self.body.as_ref().map(|b| b.len()).unwrap_or(0) as i64;
        utils::get_formatted_size(len, 0)
    }

    /// Detect redirect and modify response to text/html with <meta> redirect (required for offline mode)
    fn detect_redirect_and_set_meta_redirect(
        status_code: i32,
        mut body: Option<Vec<u8>>,
        mut headers: HashMap<String, Vec<String>>,
    ) -> (i32, Option<Vec<u8>>, HashMap<String, Vec<String>>) {
        if status_code > 300 && status_code < 320 {
            let location = headers.get("location").and_then(|v| v.first()).cloned();
            if let Some(ref loc) = location {
                body = Some(
                    format!(
                        "<meta http-equiv=\"refresh\" content=\"0; url={}\"> Redirecting to {} ...",
                        loc, loc
                    )
                    .into_bytes(),
                );
                headers.insert("content-type".to_string(), vec!["text/html".to_string()]);
            }
        }
        (status_code, body, headers)
    }

    pub fn set_loaded_from_cache(&mut self, loaded: bool) {
        self.loaded_from_cache = loaded;
    }

    pub fn is_loaded_from_cache(&self) -> bool {
        self.loaded_from_cache
    }

    pub fn is_skipped(&self) -> bool {
        self.skipped_reason.is_some()
    }

    /// Create a skipped response (status code -6)
    pub fn create_skipped(url: String, reason: String) -> Self {
        let mut response = Self {
            url,
            status_code: -6,
            body: Some(Vec::new()),
            headers: HashMap::new(),
            exec_time: 0.0,
            skipped_reason: Some(reason),
            loaded_from_cache: false,
        };
        response.skipped_reason = response.skipped_reason.take();
        response
    }

    /// Get a header value by name (case-insensitive lookup)
    pub fn get_header(&self, name: &str) -> Option<&String> {
        let lower = name.to_lowercase();
        self.headers.get(&lower)
    }

    /// Get the content-type header value
    pub fn get_content_type(&self) -> Option<&String> {
        self.get_header("content-type")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_redirect_meta() {
        let mut headers = HashMap::new();
        headers.insert("location".to_string(), vec!["https://example.com/new".to_string()]);
        let response = HttpResponse::new("https://example.com/old".to_string(), 301, None, headers, 0.1);
        assert!(response.body_text().map(|b| b.contains("Redirecting")).unwrap_or(false));
        assert_eq!(
            response.headers.get("content-type").map(|s| s.as_str()),
            Some("text/html")
        );
    }

    #[test]
    fn test_skipped_response() {
        let response = HttpResponse::create_skipped("https://example.com".to_string(), "test reason".to_string());
        assert!(response.is_skipped());
        assert_eq!(response.status_code, -6);
    }

    #[test]
    fn test_no_redirect_for_200() {
        let headers = HashMap::new();
        let response = HttpResponse::new(
            "https://example.com/".to_string(),
            200,
            Some(b"<html>ok</html>".to_vec()),
            headers,
            0.05,
        );
        assert_eq!(response.body_text().as_deref(), Some("<html>ok</html>"));
    }
}
