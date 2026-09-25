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
use crate::engine::fetcher::Fetcher;
use crate::error::{CrawlerError, CrawlerResult};
use crate::version;
use async_trait::async_trait;

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
    /// Automatic decompression is off: reqwest would drop `Content-Encoding` and `Content-Length`
    /// after decoding, so bodies are decoded by `decode_body` and headers stay as the server sent them.
    fn build_shared_client(proxy: &Option<String>, accept_invalid_certs: bool) -> reqwest::Client {
        let mut builder = Self::client_builder().danger_accept_invalid_certs(accept_invalid_certs);

        if let Some(proxy_str) = proxy {
            let parts: Vec<&str> = proxy_str.splitn(2, ':').collect();
            if parts.len() == 2 {
                let proxy_url = format!("http://{}:{}", parts[0], parts[1]);
                if let Ok(proxy) = reqwest::Proxy::all(&proxy_url) {
                    builder = builder.proxy(proxy);
                }
            }
        }

        // If the proxy or certificate settings cannot be applied, fall back to a client without
        // them, but never to `reqwest::Client::new()`, which decompresses automatically.
        builder.build().unwrap_or_else(|_| {
            Self::client_builder()
                .build()
                .expect("a default HTTP client can be built")
        })
    }

    /// Settings every crawler client shares: redirects are not followed, and there is no automatic
    /// decompression (see `build_shared_client`).
    fn client_builder() -> reqwest::ClientBuilder {
        reqwest::Client::builder()
            .redirect(reqwest::redirect::Policy::none())
            .no_gzip()
            .no_brotli()
            .no_deflate()
            .no_zstd()
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
                let resp_headers = convert_response_headers(resp.headers());
                let content_encoding = resp_headers
                    .get("content-encoding")
                    .map(|values| values.join(", "))
                    .unwrap_or_default();
                // A body that cannot be read or decoded is handled alike: no body.
                let body = resp
                    .bytes()
                    .await
                    .ok()
                    .and_then(|raw| decode_body(&raw, &content_encoding).ok());
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
        let md5 = crate::utils::to_lower_hex(hasher.finalize());
        let ext_suffix = extension.map(|e| format!(".{}", e)).unwrap_or_default();
        format!("{}-{}/{}/{}{}", host, port, &md5[..2], md5, ext_suffix)
    }
}

/// Direct-HTTP implementation of the `Fetcher` trait. Delegates to the existing
/// inherent methods so the default crawl path is byte-for-byte unchanged.
#[async_trait]
impl Fetcher for HttpClient {
    async fn fetch(
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
        self.request(
            host,
            port,
            scheme,
            url,
            http_method,
            timeout_secs,
            user_agent,
            accept,
            accept_encoding,
            origin,
            use_http_auth_if_configured,
            forced_ip,
        )
        .await
    }

    fn is_url_cached(
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
        // Explicit path to the inherent method (the trait method shares its name).
        HttpClient::is_url_cached(
            self,
            host,
            port,
            scheme,
            url,
            http_method,
            user_agent,
            accept,
            accept_encoding,
            origin,
        )
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

/// Decode a response body according to its `Content-Encoding` value: `br`, `gzip`/`x-gzip` and
/// `deflate`; a comma-separated list is removed in reverse order of application. `identity`, an
/// empty value and an empty body (HEAD, 204, 304) leave the bytes unchanged, and so does a coding
/// this client cannot decode (e.g. `zstd`) — the body is then kept exactly as received.
pub fn decode_body(raw: &[u8], content_encoding: &str) -> std::io::Result<Vec<u8>> {
    let codings: Vec<String> = content_encoding
        .split(',')
        .map(|coding| coding.trim().to_ascii_lowercase())
        .filter(|coding| !coding.is_empty() && coding.as_str() != "identity")
        .collect();
    let decodable = codings
        .iter()
        .all(|coding| matches!(coding.as_str(), "br" | "gzip" | "x-gzip" | "deflate"));
    if raw.is_empty() || codings.is_empty() || !decodable {
        return Ok(raw.to_vec());
    }

    let mut body = raw.to_vec();
    for coding in codings.iter().rev() {
        let mut decoded = Vec::new();
        match coding.as_str() {
            "br" => {
                std::io::Read::read_to_end(&mut brotli::Decompressor::new(&body[..], 4096), &mut decoded)?;
            }
            "gzip" | "x-gzip" => {
                std::io::Read::read_to_end(&mut flate2::read::MultiGzDecoder::new(&body[..]), &mut decoded)?;
            }
            _ => {
                // `deflate` is zlib-wrapped by the spec, but some servers send a raw deflate stream.
                if std::io::Read::read_to_end(&mut flate2::read::ZlibDecoder::new(&body[..]), &mut decoded).is_err() {
                    decoded.clear();
                    std::io::Read::read_to_end(&mut flate2::read::DeflateDecoder::new(&body[..]), &mut decoded)?;
                }
            }
        }
        body = decoded;
    }
    Ok(body)
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

    #[test]
    fn http_client_implements_fetcher() {
        fn assert_fetcher<T: crate::engine::fetcher::Fetcher>() {}
        assert_fetcher::<HttpClient>();
    }

    const PAGE: &[u8] = b"<html><body><p>Hello, compressed world!</p></body></html>";

    fn gzip_compress(data: &[u8]) -> Vec<u8> {
        let mut encoder = GzEncoder::new(Vec::new(), Compression::default());
        std::io::Write::write_all(&mut encoder, data).unwrap();
        encoder.finish().unwrap()
    }

    fn zlib_compress(data: &[u8]) -> Vec<u8> {
        let mut encoder = flate2::write::ZlibEncoder::new(Vec::new(), Compression::default());
        std::io::Write::write_all(&mut encoder, data).unwrap();
        encoder.finish().unwrap()
    }

    fn raw_deflate_compress(data: &[u8]) -> Vec<u8> {
        let mut encoder = flate2::write::DeflateEncoder::new(Vec::new(), Compression::default());
        std::io::Write::write_all(&mut encoder, data).unwrap();
        encoder.finish().unwrap()
    }

    fn br_compress(data: &[u8]) -> Vec<u8> {
        let mut writer = brotli::CompressorWriter::new(Vec::new(), 4096, 5, 22);
        std::io::Write::write_all(&mut writer, data).unwrap();
        writer.into_inner()
    }

    #[test]
    fn decode_body_handles_each_supported_coding() {
        assert_eq!(decode_body(&br_compress(PAGE), "br").unwrap(), PAGE);
        assert_eq!(decode_body(&br_compress(PAGE), "BR").unwrap(), PAGE);
        assert_eq!(decode_body(&gzip_compress(PAGE), "gzip").unwrap(), PAGE);
        assert_eq!(decode_body(&gzip_compress(PAGE), "x-gzip").unwrap(), PAGE);
        assert_eq!(decode_body(&zlib_compress(PAGE), "deflate").unwrap(), PAGE);
        // Some servers send a raw deflate stream although `deflate` means zlib-wrapped data.
        assert_eq!(decode_body(&raw_deflate_compress(PAGE), "deflate").unwrap(), PAGE);
    }

    #[test]
    fn decode_body_removes_chained_codings_in_reverse_order() {
        // `gzip, br` = gzip was applied first, then Brotli.
        let body = br_compress(&gzip_compress(PAGE));
        assert_eq!(decode_body(&body, "gzip, br").unwrap(), PAGE);
    }

    #[test]
    fn decode_body_keeps_identity_unknown_and_empty_bodies() {
        assert_eq!(decode_body(PAGE, "identity").unwrap(), PAGE);
        assert_eq!(decode_body(PAGE, "").unwrap(), PAGE);
        // zstd is not decoded: the body stays exactly as received, as before.
        let zstd_like = b"\x28\xb5\x2f\xfd raw zstd frame";
        assert_eq!(decode_body(zstd_like, "zstd").unwrap(), zstd_like);
        assert_eq!(decode_body(PAGE, "br, zstd").unwrap(), PAGE);
        // Empty bodies (HEAD, 204, 304) pass through.
        assert!(decode_body(b"", "br").unwrap().is_empty());
    }

    #[test]
    fn decode_body_reports_corrupt_data() {
        assert!(decode_body(b"definitely not brotli", "br").is_err());
        assert!(decode_body(b"definitely not gzip", "gzip").is_err());
    }

    /// A complete HTTP/1.1 200 response; `headers` are CRLF-terminated header lines.
    fn raw_response(headers: &str, body: &[u8]) -> Vec<u8> {
        let mut response = format!(
            "HTTP/1.1 200 OK\r\n{headers}Content-Length: {}\r\nConnection: close\r\n\r\n",
            body.len()
        )
        .into_bytes();
        response.extend_from_slice(body);
        response
    }

    /// Answers the first connection with `response` and passes the received request head back,
    /// so a test can also check what the client sent.
    fn serve_once(response: Vec<u8>) -> (u16, std::sync::mpsc::Receiver<String>) {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        let (sender, receiver) = std::sync::mpsc::channel();
        std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            let mut head = Vec::new();
            let mut buf = [0u8; 1024];
            while !head.windows(4).any(|window| window == b"\r\n\r\n") {
                match std::io::Read::read(&mut stream, &mut buf) {
                    Ok(0) | Err(_) => break,
                    Ok(n) => head.extend_from_slice(&buf[..n]),
                }
            }
            sender.send(String::from_utf8_lossy(&head).into_owned()).ok();
            std::io::Write::write_all(&mut stream, &response).ok();
        });
        (port, receiver)
    }

    /// The fallback used when the configured client cannot be built must not turn automatic
    /// decompression back on: it would strip `Content-Encoding` again (#107).
    #[tokio::test]
    async fn fallback_client_keeps_automatic_decompression_off() {
        let compressed = br_compress(PAGE);
        let (port, _) = serve_once(raw_response(
            "Content-Type: text/html\r\nContent-Encoding: br\r\n",
            &compressed,
        ));
        let response = HttpClient::client_builder()
            .build()
            .unwrap()
            .get(format!("http://127.0.0.1:{port}/"))
            .header(reqwest::header::ACCEPT_ENCODING, "br")
            .send()
            .await
            .unwrap();
        assert_eq!(
            response.headers().get("content-encoding").map(|v| v.to_str().unwrap()),
            Some("br")
        );
        assert_eq!(response.bytes().await.unwrap().as_ref(), compressed.as_slice());
    }

    #[tokio::test]
    async fn brotli_body_is_decoded_and_content_encoding_is_kept() {
        let page = PAGE.repeat(50);
        let compressed = br_compress(&page);
        let (port, request_head) = serve_once(raw_response(
            "Content-Type: text/html\r\nContent-Encoding: br\r\nVary: Accept-Encoding\r\n",
            &compressed,
        ));

        let client = HttpClient::new(None, None, None, false, None, false);
        let response = client
            .request(
                "127.0.0.1",
                port,
                "http",
                "/",
                "GET",
                5,
                "test-agent",
                "*/*",
                "gzip, deflate, br",
                None,
                false,
                None,
            )
            .await
            .unwrap();

        assert_eq!(response.status_code, 200);
        assert_eq!(response.body.as_deref(), Some(page.as_slice()), "the body is decoded");
        assert_eq!(response.get_header("content-encoding").map(String::as_str), Some("br"));
        assert_eq!(
            response.get_header("content-length"),
            Some(&compressed.len().to_string()),
            "Content-Length keeps the size on the wire"
        );
        let head = request_head.recv().unwrap().to_ascii_lowercase();
        assert!(head.contains("accept-encoding: gzip, deflate, br"), "{head}");
    }
}
