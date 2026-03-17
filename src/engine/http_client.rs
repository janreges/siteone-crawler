// SiteOne Crawler - HttpClient
// (c) Jan Reges <jan.reges@siteone.cz>

use std::collections::HashMap;
use std::path::Path;
use std::time::Instant;

use flate2::Compression;
use flate2::read::GzDecoder;
use flate2::write::GzEncoder;
use md5::{Digest, Md5};
use reqwest::header::{HeaderMap, HeaderName, HeaderValue};

use base64::Engine as _;

use super::http_response::HttpResponse;
use crate::error::{CrawlerError, CrawlerResult};
use crate::version;

/// Async HTTP client for crawling with caching, proxy, and auth support
pub struct HttpClient {
    /// Reusable reqwest client (Arc-backed, clone is cheap)
    client: reqwest::Client,
    /// Basic HTTP auth in format "username:password"
    http_auth: Option<String>,
    /// Cache directory. If None, caching is disabled
    cache_dir: Option<String>,
    /// Whether to compress cached data with gzip
    compression: bool,
    /// Cache TTL in seconds. None = infinite (never expires)
    cache_ttl: Option<u64>,
}

impl HttpClient {
    pub fn new(
        proxy: Option<String>,
        http_auth: Option<String>,
        cache_dir: Option<String>,
        compression: bool,
        cache_ttl: Option<u64>,
        accept_invalid_certs: bool,
    ) -> Self {
        let client = Self::build_shared_client(&proxy, accept_invalid_certs);
        Self {
            client,
            http_auth,
            cache_dir,
            compression,
            cache_ttl,
        }
    }

    /// Build the shared reqwest::Client with proxy support.
    /// Timeout is set per-request, not on the shared client.
    fn build_shared_client(proxy: &Option<String>, accept_invalid_certs: bool) -> reqwest::Client {
        let mut builder = reqwest::Client::builder()
            .danger_accept_invalid_certs(accept_invalid_certs)
            .redirect(reqwest::redirect::Policy::none());

        if let Some(proxy_str) = proxy {
            let parts: Vec<&str> = proxy_str.splitn(2, ':').collect();
            if parts.len() == 2 {
                let proxy_url = format!("http://{}:{}", parts[0], parts[1]);
                if let Ok(proxy) = reqwest::Proxy::all(&proxy_url) {
                    builder = builder.proxy(proxy);
                }
            }
        }

        builder.build().unwrap_or_else(|_| reqwest::Client::new())
    }

    /// Perform an HTTP request (GET or HEAD)
    #[allow(clippy::too_many_arguments)]
    pub async fn request(
        &self,
        host: &str,
        port: u16,
        scheme: &str,
        url: &str,
        http_method: &str,
        timeout_secs: u64,
        user_agent: &str,
        accept: &str,
        accept_encoding: &str,
        origin: Option<&str>,
        use_http_auth_if_configured: bool,
        forced_ip: Option<&str>,
    ) -> CrawlerResult<HttpResponse> {
        let path = url::Url::parse(url).ok().map(|u| u.path().to_string());
        let extension = path.as_ref().and_then(|p| {
            std::path::Path::new(p)
                .extension()
                .and_then(|e| e.to_str())
                .map(|e| e.to_string())
        });

        let args_for_cache: Vec<String> = vec![
            host.to_string(),
            port.to_string(),
            scheme.to_string(),
            url.to_string(),
            http_method.to_string(),
            user_agent.to_string(),
            accept.to_string(),
            accept_encoding.to_string(),
            origin.unwrap_or("").to_string(),
        ];
        let cache_key = self.get_cache_key(host, port, &args_for_cache, extension.as_deref());

        // Check cache first (skip URLs with spaces as they are likely problematic)
        if !url.contains(' ')
            && let Some(mut cached) = self.get_from_cache(&cache_key)
        {
            cached.set_loaded_from_cache(true);
            return Ok(cached);
        }

        // Build request headers
        let mut request_headers = HeaderMap::new();
        if let Ok(v) = HeaderValue::from_str(&format!("siteone-crawler/{}", version::CODE)) {
            request_headers.insert("x-crawler-info", v);
        }
        if let Ok(v) = HeaderValue::from_str(user_agent) {
            request_headers.insert(reqwest::header::USER_AGENT, v);
        }
        if let Ok(v) = HeaderValue::from_str(accept) {
            request_headers.insert(reqwest::header::ACCEPT, v);
        }
        if let Ok(v) = HeaderValue::from_str(accept_encoding) {
            request_headers.insert(reqwest::header::ACCEPT_ENCODING, v);
        }
        if let Ok(v) = HeaderValue::from_str("close") {
            request_headers.insert(reqwest::header::CONNECTION, v);
        }

        if let Some(ip) = forced_ip {
            let _ = ip; // forced_ip handling: set Host header
            if let Ok(v) = HeaderValue::from_str(host) {
                request_headers.insert(reqwest::header::HOST, v);
            }
        }

        if let Some(origin_val) = origin
            && let Ok(v) = HeaderValue::from_str(origin_val)
            && let Ok(name) = HeaderName::from_bytes(b"origin")
        {
            request_headers.insert(name, v);
        }

        // Use shared client with per-request timeout
        let client = self.client.clone();

        // Fix spaces in URL
        let request_url = url.replace("\\ ", "%20").replace(' ', "%20");

        // Build the actual URL to request
        let actual_host = forced_ip.unwrap_or(host);
        let full_url = if request_url.starts_with("http://") || request_url.starts_with("https://") {
            request_url.clone()
        } else {
            let port_str = match (scheme, port) {
                ("http", 80) | ("https", 443) => String::new(),
                _ => format!(":{}", port),
            };
            format!("{}://{}{}{}", scheme, actual_host, port_str, request_url)
        };

        let start_time = Instant::now();

        let timeout = std::time::Duration::from_secs(timeout_secs);
        let request = match http_method.to_uppercase().as_str() {
            "HEAD" => client.head(&full_url).timeout(timeout),
            _ => client.get(&full_url).timeout(timeout),
        };

        let request = request.headers(request_headers);

        // Add basic auth if configured and requested
        let request = if use_http_auth_if_configured {
            if let Some(ref auth) = self.http_auth {
                let parts: Vec<&str> = auth.splitn(2, ':').collect();
                if parts.len() == 2 {
                    request.basic_auth(parts[0], Some(parts[1]))
                } else {
                    request.basic_auth(auth, Option::<&str>::None)
                }
            } else {
                request
            }
        } else {
            request
        };

        let result = match request.send().await {
            Ok(resp) => {
                let status = resp.status().as_u16() as i32;
                let mut resp_headers = convert_response_headers(resp.headers());
                // reqwest auto-decompresses and strips Content-Encoding header.
                // Detect decompression by checking if Transfer-Encoding: chunked and
                // Vary: Accept-Encoding are present (indicating the response was compressed).
                let has_transfer_chunked = resp_headers
                    .get("transfer-encoding")
                    .map(|vals| vals.iter().any(|v| v.contains("chunked")))
                    .unwrap_or(false);
                let has_vary_encoding = resp_headers
                    .get("vary")
                    .map(|vals| vals.iter().any(|v| v.contains("Accept-Encoding")))
                    .unwrap_or(false);
                if has_transfer_chunked && has_vary_encoding && !resp_headers.contains_key("content-encoding") {
                    resp_headers.insert("content-encoding".to_string(), vec!["gzip".to_string()]);
                }
                let body = resp.bytes().await.ok().map(|b| b.to_vec());
                let elapsed = start_time.elapsed().as_secs_f64();

                HttpResponse::new(url.to_string(), status, body, resp_headers, elapsed)
            }
            Err(e) => {
                let elapsed = start_time.elapsed().as_secs_f64();
                let status = if e.is_connect() {
                    -1 // Connection failure
                } else if e.is_timeout() {
                    -2 // Timeout
                } else if e.is_request() {
                    -4 // Send error
                } else {
                    -1 // Generic connection failure
                };
                HttpResponse::new(url.to_string(), status, None, HashMap::new(), elapsed)
            }
        };

        self.save_to_cache(&cache_key, &result)?;
        Ok(result)
    }

    /// Get cached HTTP response
    fn get_from_cache(&self, cache_key: &str) -> Option<HttpResponse> {
        let cache_file = self.get_cache_file_path(cache_key)?;

        let cache_path = Path::new(&cache_file);
        if !cache_path.is_file() {
            return None;
        }

        // Check TTL: if cache file is older than TTL, treat as miss
        if let Some(ttl_secs) = self.cache_ttl
            && let Ok(metadata) = cache_path.metadata()
            && let Ok(modified) = metadata.modified()
            && let Ok(age) = modified.elapsed()
            && age.as_secs() > ttl_secs
        {
            return None;
        }

        let data = std::fs::read(&cache_file).ok()?;
        let json_str = if self.compression {
            let mut decoder = GzDecoder::new(&data[..]);
            let mut decompressed = String::new();
            std::io::Read::read_to_string(&mut decoder, &mut decompressed).ok()?;
            decompressed
        } else {
            String::from_utf8(data).ok()?
        };

        let cached: CachedResponse = serde_json::from_str(&json_str).ok()?;

        // Don't use cached responses with error/server-error status codes
        if matches!(cached.status_code, 429 | 500 | 502 | 503 | -1 | -2 | -3 | -4) {
            return None;
        }

        let mut headers = HashMap::new();
        for (k, v) in &cached.headers {
            headers.insert(k.clone(), vec![v.clone()]);
        }

        // Decode body: try base64 first (new format), fall back to raw UTF-8 (old cache format)
        let body_bytes = cached.body.as_ref().map(|b| {
            // Try base64 decode first, fall back to raw UTF-8 bytes (old cache format)
            base64::engine::general_purpose::STANDARD
                .decode(b)
                .unwrap_or_else(|_| b.as_bytes().to_vec())
        });

        Some(HttpResponse::new(
            cached.url,
            cached.status_code,
            body_bytes,
            headers,
            cached.exec_time,
        ))
    }

    /// Save HTTP response to disk cache
    fn save_to_cache(&self, cache_key: &str, result: &HttpResponse) -> CrawlerResult<()> {
        let cache_file = match self.get_cache_file_path(cache_key) {
            Some(f) => f,
            None => return Ok(()),
        };

        let cache_dir = Path::new(&cache_file)
            .parent()
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_default();

        if !Path::new(&cache_dir).is_dir() {
            std::fs::create_dir_all(&cache_dir).map_err(|e| {
                CrawlerError::Io(std::io::Error::new(
                    e.kind(),
                    format!("Cannot create cache dir {}: {}", cache_dir, e),
                ))
            })?;
        }

        let cached = CachedResponse {
            url: result.url.clone(),
            status_code: result.status_code,
            body: result
                .body
                .as_ref()
                .map(|b| base64::engine::general_purpose::STANDARD.encode(b)),
            headers: result.headers.clone(),
            exec_time: result.exec_time,
        };

        let json = serde_json::to_string(&cached)
            .map_err(|e| CrawlerError::Other(format!("Cache serialization error: {}", e)))?;

        let data = if self.compression {
            let mut encoder = GzEncoder::new(Vec::new(), Compression::default());
            std::io::Write::write_all(&mut encoder, json.as_bytes()).map_err(CrawlerError::Io)?;
            encoder.finish().map_err(CrawlerError::Io)?
        } else {
            json.into_bytes()
        };

        std::fs::write(&cache_file, &data).map_err(|e| {
            CrawlerError::Io(std::io::Error::new(
                e.kind(),
                format!("Cannot write to cache file {}: {}", cache_file, e),
            ))
        })?;

        Ok(())
    }

    /// Check if a response for the given request parameters exists in cache.
    /// Used to skip rate limiting for cached responses.
    #[allow(clippy::too_many_arguments)]
    pub fn is_url_cached(
        &self,
        host: &str,
        port: u16,
        scheme: &str,
        url: &str,
        http_method: &str,
        user_agent: &str,
        accept: &str,
        accept_encoding: &str,
        origin: Option<&str>,
    ) -> bool {
        if self.cache_dir.is_none() || url.contains(' ') {
            return false;
        }
        let path = url::Url::parse(url).ok().map(|u| u.path().to_string());
        let extension = path.as_ref().and_then(|p| {
            std::path::Path::new(p)
                .extension()
                .and_then(|e| e.to_str())
                .map(|e| e.to_string())
        });
        let args_for_cache: Vec<String> = vec![
            host.to_string(),
            port.to_string(),
            scheme.to_string(),
            url.to_string(),
            http_method.to_string(),
            user_agent.to_string(),
            accept.to_string(),
            accept_encoding.to_string(),
            origin.unwrap_or("").to_string(),
        ];
        let cache_key = self.get_cache_key(host, port, &args_for_cache, extension.as_deref());
        match self.get_cache_file_path(&cache_key) {
            Some(file) => Path::new(&file).is_file(),
            None => false,
        }
    }

    /// Get cache file path for a given cache key
    fn get_cache_file_path(&self, cache_key: &str) -> Option<String> {
        let cache_dir = self.cache_dir.as_ref()?;
        let ext = if self.compression { ".cache.gz" } else { ".cache" };
        Some(format!("{}/{}{}", cache_dir, cache_key, ext))
    }

    /// Generate a cache key from request parameters
    fn get_cache_key(&self, host: &str, port: u16, args: &[String], extension: Option<&str>) -> String {
        let mut hasher = Md5::new();
        for arg in args {
            hasher.update(arg.as_bytes());
        }
        let md5 = format!("{:x}", hasher.finalize());
        let ext_suffix = extension.map(|e| format!(".{}", e)).unwrap_or_default();
        format!("{}-{}/{}/{}{}", host, port, &md5[..2], md5, ext_suffix)
    }
}

/// Internal struct for cache serialization
#[derive(serde::Serialize, serde::Deserialize)]
struct CachedResponse {
    url: String,
    status_code: i32,
    /// Body stored as base64-encoded bytes to preserve binary data in JSON
    body: Option<String>,
    headers: HashMap<String, String>,
    exec_time: f64,
}

/// Convert reqwest response headers to HashMap<String, Vec<String>>
fn convert_response_headers(headers: &reqwest::header::HeaderMap) -> HashMap<String, Vec<String>> {
    let mut result: HashMap<String, Vec<String>> = HashMap::new();
    for (key, value) in headers.iter() {
        let key_str = key.as_str().to_lowercase();
        let val_str = value.to_str().unwrap_or("").to_string();
        result.entry(key_str).or_default().push(val_str);
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cache_key_generation() {
        let client = HttpClient::new(None, None, Some("/tmp/cache".to_string()), false, None, false);
        let args = vec![
            "example.com".to_string(),
            "443".to_string(),
            "https".to_string(),
            "/page".to_string(),
        ];
        let key = client.get_cache_key("example.com", 443, &args, Some("html"));
        assert!(key.starts_with("example.com-443/"));
        assert!(key.ends_with(".html"));
    }

    #[test]
    fn test_cache_file_path() {
        let client = HttpClient::new(None, None, Some("/tmp/cache".to_string()), false, None, false);
        let path = client.get_cache_file_path("example.com-443/ab/abcdef");
        assert_eq!(path, Some("/tmp/cache/example.com-443/ab/abcdef.cache".to_string()));

        let client_gz = HttpClient::new(None, None, Some("/tmp/cache".to_string()), true, None, false);
        let path_gz = client_gz.get_cache_file_path("example.com-443/ab/abcdef");
        assert_eq!(
            path_gz,
            Some("/tmp/cache/example.com-443/ab/abcdef.cache.gz".to_string())
        );
    }

    #[test]
    fn test_no_cache_when_disabled() {
        let client = HttpClient::new(None, None, None, false, None, false);
        assert!(client.get_cache_file_path("any-key").is_none());
    }
}
