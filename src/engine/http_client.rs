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
    /// Custom request headers from `--header`, sent in the same scope as `http_auth`
    custom_headers: Vec<(HeaderName, HeaderValue)>,
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
            custom_headers: Vec::new(),
            cache_dir,
            compression,
            cache_ttl,
        }
    }

    /// Set the `--header` values (`Name: value`, validated when the options were parsed). A name
    /// given more than once keeps its last value, so the command line overrides the config file.
    pub fn with_custom_headers(mut self, headers: &[String]) -> Self {
        self.custom_headers.clear();
        for (name, value) in headers.iter().filter_map(|raw| parse_custom_header(raw).ok()) {
            self.custom_headers.retain(|(existing, _)| *existing != name);
            self.custom_headers.push((name, value));
        }
        self
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

    /// Perform an HTTP request (GET or HEAD).
    /// `use_http_auth_if_configured` marks a request within the crawled site's scope: only then are
    /// the `--http-auth` credentials and the `--header` values sent.
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

        // `--header` values are credentials like `--http-auth`: sent only within its scope, and
        // applied last so they replace a default header of the same name.
        if use_http_auth_if_configured {
            for (name, value) in &self.custom_headers {
                request_headers.insert(name.clone(), value.clone());
            }
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

        // Add basic auth if configured and requested; a custom `Authorization` header wins
        let has_custom_authorization = self
            .custom_headers
            .iter()
            .any(|(name, _)| *name == reqwest::header::AUTHORIZATION);
        let request = if use_http_auth_if_configured && !has_custom_authorization {
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
                // A body that cannot be read or decoded is handled alike: no body. Decoding runs off the
                // async workers and must finish within the request timeout, like reading the body.
                let deadline = start_time + timeout;
                let body = match resp.bytes().await {
                    Ok(raw) => {
                        tokio::task::spawn_blocking(move || decode_body_until(&raw, &content_encoding, Some(deadline)))
                            .await
                            .ok()
                            .and_then(Result::ok)
                    }
                    Err(_) => None,
                };
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

    /// Generate a cache key from request parameters. Configured credentials (`--http-auth`,
    /// `--header`) are part of the key, so authenticated and anonymous responses never share an
    /// entry; without credentials the key is the same as in earlier versions.
    fn get_cache_key(&self, host: &str, port: u16, args: &[String], extension: Option<&str>) -> String {
        let mut hasher = Md5::new();
        for arg in args {
            hasher.update(arg.as_bytes());
        }
        if let Some(auth) = &self.http_auth {
            hasher.update(b"\0http-auth:");
            hasher.update(auth.as_bytes());
        }
        for (name, value) in &self.custom_headers {
            hasher.update(b"\0header:");
            hasher.update(name.as_str().as_bytes());
            hasher.update(b":");
            hasher.update(value.as_bytes());
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
    decode_body_until(raw, content_encoding, None)
}

/// [`decode_body`] that gives up with `ErrorKind::TimedOut` once `deadline` passes, so decompressing
/// a response cannot outlive the request timeout. Every coding must reach the end of its stream: a
/// truncated `deflate` body is an error, not a shorter page.
pub fn decode_body_until(raw: &[u8], content_encoding: &str, deadline: Option<Instant>) -> std::io::Result<Vec<u8>> {
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
        body = match coding.as_str() {
            "br" => read_until(brotli::Decompressor::new(&body[..], 4096), deadline)?,
            "gzip" | "x-gzip" => read_until(flate2::read::MultiGzDecoder::new(&body[..]), deadline)?,
            // `deflate` is zlib-wrapped by the spec, but some servers send a raw deflate stream.
            _ => match inflate_until(&body, true, deadline) {
                Err(error) if error.kind() != std::io::ErrorKind::TimedOut => inflate_until(&body, false, deadline)?,
                result => result?,
            },
        };
    }
    Ok(body)
}

fn check_deadline(deadline: Option<Instant>) -> std::io::Result<()> {
    if deadline.is_some_and(|deadline| Instant::now() >= deadline) {
        return Err(std::io::Error::new(
            std::io::ErrorKind::TimedOut,
            "decoding the response body exceeded the request timeout",
        ));
    }
    Ok(())
}

/// Read a decoder to its end in chunks, checking `deadline` between them.
fn read_until(mut reader: impl std::io::Read, deadline: Option<Instant>) -> std::io::Result<Vec<u8>> {
    let mut decoded = Vec::new();
    let mut chunk = vec![0u8; 64 * 1024];
    loop {
        check_deadline(deadline)?;
        let read = reader.read(&mut chunk)?;
        if read == 0 {
            return Ok(decoded);
        }
        decoded.extend_from_slice(&chunk[..read]);
    }
}

/// Inflate a zlib-wrapped (`zlib_header`) or raw deflate stream up to its end marker. flate2's
/// readers report a stream that stops early as a normal end of input, so this drives the inflater
/// directly and treats input that runs out before the end marker as an error.
fn inflate_until(input: &[u8], zlib_header: bool, deadline: Option<Instant>) -> std::io::Result<Vec<u8>> {
    let mut inflater = flate2::Decompress::new(zlib_header);
    let mut decoded: Vec<u8> = Vec::with_capacity(input.len().saturating_mul(4).max(1024));
    loop {
        check_deadline(deadline)?;
        if decoded.len() == decoded.capacity() {
            decoded.reserve(decoded.capacity().max(64 * 1024));
        }
        let consumed = inflater.total_in() as usize;
        let produced = decoded.len();
        let status = inflater
            .decompress_vec(&input[consumed..], &mut decoded, flate2::FlushDecompress::None)
            .map_err(|error| std::io::Error::new(std::io::ErrorKind::InvalidData, error))?;
        if matches!(status, flate2::Status::StreamEnd) {
            return Ok(decoded);
        }
        let progressed = inflater.total_in() as usize != consumed || decoded.len() != produced;
        if !progressed && decoded.len() < decoded.capacity() {
            return Err(std::io::Error::new(
                std::io::ErrorKind::UnexpectedEof,
                "the deflate stream ends before its end marker",
            ));
        }
    }
}

/// Parse one `--header` value (`Name: value`). The name must be an HTTP token and the value must
/// not contain line breaks; `Host`, `Content-Length` and the hop-by-hop headers come from the
/// request or the connection itself and cannot be set. Errors continue the sentence
/// "Option --header …" and never repeat the input, which may be a secret (cookie, token).
pub fn parse_custom_header(raw: &str) -> Result<(HeaderName, HeaderValue), String> {
    if raw.contains(['\r', '\n']) {
        return Err("must not contain line breaks".to_string());
    }
    let Some((name, value)) = raw.split_once(':') else {
        return Err("must be in `Name: value` format".to_string());
    };
    let name = name.trim();
    let header_name = HeaderName::from_bytes(name.as_bytes()).map_err(|_| "has an invalid header name".to_string())?;
    if matches!(
        header_name.as_str(),
        "host"
            | "content-length"
            | "connection"
            | "keep-alive"
            | "proxy-connection"
            | "te"
            | "trailer"
            | "transfer-encoding"
            | "upgrade"
    ) {
        return Err(format!("cannot set the {name} header"));
    }
    let header_value = HeaderValue::from_bytes(value.trim().as_bytes())
        .map_err(|_| format!("has an invalid value for header '{name}'"))?;
    Ok((header_name, header_value))
}

/// The `User-Agent` set with `--header`, if any (the last one wins, as on requests). Reports show
/// it as the crawler's user agent, because it is what the crawled site receives.
pub fn custom_user_agent(headers: &[String]) -> Option<String> {
    headers
        .iter()
        .rev()
        .filter_map(|raw| parse_custom_header(raw).ok())
        .find(|(name, _)| *name == reqwest::header::USER_AGENT)
        .and_then(|(_, value)| value.to_str().ok().map(str::to_string))
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
    fn decode_body_rejects_truncated_deflate_streams() {
        let zlib = zlib_compress(&PAGE.repeat(20));
        assert!(
            decode_body(&zlib[..zlib.len() / 2], "deflate").is_err(),
            "truncated zlib"
        );
        assert!(
            decode_body(&zlib[..zlib.len() - 4], "deflate").is_err(),
            "zlib without its checksum"
        );
        let raw = raw_deflate_compress(&PAGE.repeat(20));
        assert!(
            decode_body(&raw[..raw.len() / 2], "deflate").is_err(),
            "truncated raw deflate"
        );
        // Complete streams still decode, including the raw-deflate fallback.
        assert_eq!(decode_body(&zlib, "deflate").unwrap(), PAGE.repeat(20));
        assert_eq!(decode_body(&raw, "deflate").unwrap(), PAGE.repeat(20));
    }

    #[test]
    fn decoding_stops_once_the_request_deadline_has_passed() {
        let page = vec![b'x'; 8 * 1024 * 1024];
        let passed = Some(Instant::now() - std::time::Duration::from_millis(1));
        for (body, coding) in [
            (gzip_compress(&page), "gzip"),
            (br_compress(&page), "br"),
            (zlib_compress(&page), "deflate"),
        ] {
            let error = decode_body_until(&body, coding, passed).unwrap_err();
            assert_eq!(error.kind(), std::io::ErrorKind::TimedOut, "{coding}");
        }
        let future = Some(Instant::now() + std::time::Duration::from_secs(60));
        assert_eq!(decode_body_until(&gzip_compress(PAGE), "gzip", future).unwrap(), PAGE);
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

    #[test]
    fn parse_custom_header_accepts_name_value_pairs() {
        let (name, value) = parse_custom_header("Cookie: a=1; b=2").unwrap();
        assert_eq!(name.as_str(), "cookie");
        assert_eq!(value.to_str().unwrap(), "a=1; b=2");
        let (_, value) = parse_custom_header("Accept-Language:cs,en;q=0.8").unwrap();
        assert_eq!(value.to_str().unwrap(), "cs,en;q=0.8");
        let (_, value) = parse_custom_header("X-Empty:").unwrap();
        assert_eq!(value.to_str().unwrap(), "");
    }

    #[test]
    fn parse_custom_header_rejects_invalid_input() {
        for raw in [
            "NoColonHere",
            "Bad Name: x",
            ": no-name",
            "X-Test: a\r\nInjected: b",
            "X-Test: a\nb",
            "Host: example.test",
            "content-length: 5",
            // Hop-by-hop headers belong to the connection, not to the request.
            "Connection: keep-alive",
            "Keep-Alive: timeout=5",
            "Proxy-Connection: keep-alive",
            "TE: trailers",
            "Trailer: Expires",
            "Transfer-Encoding: chunked",
            "Upgrade: websocket",
        ] {
            assert!(parse_custom_header(raw).is_err(), "{raw:?}");
        }
    }

    #[test]
    fn parse_custom_header_errors_never_repeat_the_input() {
        for raw in [
            "Cookie=session:SECRET_SUFFIX",
            "X SECRET_SUFFIX: value",
            "SECRET_SUFFIX",
            "X-Token: SECRET_SUFFIX\u{7f}",
        ] {
            let error = parse_custom_header(raw).unwrap_err();
            assert!(!error.contains("SECRET_SUFFIX"), "{raw:?}: {error}");
            assert!(!error.contains("session"), "{raw:?}: {error}");
        }
    }

    #[tokio::test]
    async fn custom_headers_are_sent_only_within_the_auth_scope() {
        let client = HttpClient::new(None, None, None, false, None, false).with_custom_headers(&[
            "User-Agent: CustomAgent/1.0".to_string(),
            "Cookie: session=abc".to_string(),
            "Accept-Language: cs,en;q=0.8".to_string(),
        ]);

        let (port, request_head) = serve_once(raw_response("Content-Type: text/html\r\n", b"ok"));
        client
            .request(
                "127.0.0.1",
                port,
                "http",
                "/",
                "GET",
                5,
                "default-agent",
                "*/*",
                "gzip",
                None,
                true,
                None,
            )
            .await
            .unwrap();
        let head = request_head.recv().unwrap().to_ascii_lowercase();
        assert!(head.contains("user-agent: customagent/1.0"), "{head}");
        assert!(
            !head.contains("default-agent"),
            "the custom header replaces the default: {head}"
        );
        assert!(head.contains("cookie: session=abc"), "{head}");
        assert!(head.contains("accept-language: cs,en;q=0.8"), "{head}");

        let (port, request_head) = serve_once(raw_response("Content-Type: text/html\r\n", b"ok"));
        client
            .request(
                "127.0.0.1",
                port,
                "http",
                "/",
                "GET",
                5,
                "default-agent",
                "*/*",
                "gzip",
                None,
                false,
                None,
            )
            .await
            .unwrap();
        let head = request_head.recv().unwrap().to_ascii_lowercase();
        assert!(head.contains("user-agent: default-agent"), "{head}");
        assert!(
            !head.contains("cookie:"),
            "outside the scope nothing custom is sent: {head}"
        );
    }

    #[test]
    fn credentials_are_part_of_the_cache_key() {
        let cache = || Some("/tmp/cache".to_string());
        let args = vec!["example.com".to_string(), "/page".to_string()];
        let key = |client: &HttpClient| client.get_cache_key("example.com", 443, &args, Some("html"));

        let anonymous = HttpClient::new(None, None, cache(), false, None, false);
        let cookie_a = HttpClient::new(None, None, cache(), false, None, false)
            .with_custom_headers(&["Cookie: session=a".to_string()]);
        let cookie_b = HttpClient::new(None, None, cache(), false, None, false)
            .with_custom_headers(&["Cookie: session=b".to_string()]);
        let basic_auth = HttpClient::new(None, Some("user:pass".to_string()), cache(), false, None, false);

        assert_ne!(key(&anonymous), key(&cookie_a));
        assert_ne!(key(&cookie_a), key(&cookie_b));
        assert_ne!(key(&anonymous), key(&basic_auth));
        assert_eq!(
            key(&cookie_a),
            key(&HttpClient::new(None, None, cache(), false, None, false)
                .with_custom_headers(&["Cookie: session=a".to_string()])),
            "the same credentials give the same key"
        );

        // Without credentials the key is unchanged, so existing caches stay valid.
        let mut hasher = Md5::new();
        for arg in &args {
            hasher.update(arg.as_bytes());
        }
        let md5 = crate::utils::to_lower_hex(hasher.finalize());
        assert_eq!(key(&anonymous), format!("example.com-443/{}/{}.html", &md5[..2], md5));
    }

    /// The config file comes before the command line, so the last value of a header wins and a
    /// `--header` on the command line overrides the config file; only one line is sent.
    #[tokio::test]
    async fn repeated_custom_header_names_keep_the_last_value() {
        let client = HttpClient::new(None, None, None, false, None, false).with_custom_headers(&[
            "Cookie: from=config".to_string(),
            "User-Agent: ConfigAgent/1.0".to_string(),
            "cookie: from=cli".to_string(),
        ]);
        let (port, request_head) = serve_once(raw_response("Content-Type: text/html\r\n", b"ok"));
        client
            .request(
                "127.0.0.1",
                port,
                "http",
                "/",
                "GET",
                5,
                "default-agent",
                "*/*",
                "gzip",
                None,
                true,
                None,
            )
            .await
            .unwrap();
        let head = request_head.recv().unwrap().to_ascii_lowercase();
        let cookies: Vec<&str> = head.lines().filter(|line| line.starts_with("cookie:")).collect();
        assert_eq!(cookies, vec!["cookie: from=cli"], "{head}");
        assert!(head.contains("user-agent: configagent/1.0"), "{head}");
    }

    #[test]
    fn custom_user_agent_is_the_last_user_agent_header() {
        let headers = |raw: &[&str]| raw.iter().map(|h| h.to_string()).collect::<Vec<_>>();
        assert_eq!(custom_user_agent(&headers(&["Cookie: a=1"])), None);
        assert_eq!(
            custom_user_agent(&headers(&[
                "User-Agent: First/1.0",
                "Cookie: a=1",
                "user-agent: Second/2.0"
            ]))
            .as_deref(),
            Some("Second/2.0")
        );
    }
}
