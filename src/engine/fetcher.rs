// SiteOne Crawler - Fetcher
// (c) Jan Reges <jan.reges@siteone.cz>

use async_trait::async_trait;

use crate::engine::http_response::HttpResponse;
use crate::error::CrawlerResult;

/// Abstraction over "how a URL is fetched".
///
/// The default `HttpClient` implements this (direct HTTP request). A feature-gated
/// `BrowserRenderer` (Cargo feature `browser`) implements it by driving a real
/// Chromium via the Chrome DevTools Protocol. Everything downstream of the fetch
/// (content processors, link extraction, analyzers, scoring, exporters) is unchanged,
/// because both implementations return the same `HttpResponse`.
///
/// The method set mirrors exactly what the crawl loop calls on the client today:
/// `fetch` (the request) and `is_url_cached` (used by rate limiting to skip the delay
/// for cache hits).
#[async_trait]
pub trait Fetcher: Send + Sync {
    /// Perform the fetch for a single URL and return the response.
    #[allow(clippy::too_many_arguments)]
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
    ) -> CrawlerResult<HttpResponse>;

    /// Fetch via direct HTTP only, never via browser rendering. Used for internal fetches that
    /// must not be rendered or screenshotted (e.g. `robots.txt`). The default implementation is
    /// identical to `fetch`; the browser renderer overrides it to use its inner HTTP client.
    #[allow(clippy::too_many_arguments)]
    async fn fetch_http_only(
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
        self.fetch(
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

    /// Whether a response for these request parameters already exists in the HTTP cache.
    /// Used by rate limiting to skip the inter-request delay for cache hits. A browser
    /// renderer delegates this to its inner `HttpClient`.
    #[allow(clippy::too_many_arguments)]
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
    ) -> bool;

    /// Optional clean-shutdown hook (e.g. browser teardown). Default is a no-op.
    async fn shutdown(&self) {}
}
