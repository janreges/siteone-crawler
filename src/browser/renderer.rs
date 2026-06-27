// SiteOne Crawler - BrowserRenderer
// (c) Jan Reges <jan.reges@siteone.cz>
//
// Compiled only with the `browser` Cargo feature. Implements `Fetcher` by rendering
// HTML documents in a real Chromium via CDP. Status code, headers, caching, auth and
// redirect handling come from the inner `HttpClient`; only the body is replaced with the
// post-JS rendered DOM. Non-HTML responses (and errors/redirects) pass straight through.

use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use chromiumoxide::Browser;
use chromiumoxide::cdp::browser_protocol::network::SetUserAgentOverrideParams;
use chromiumoxide::cdp::browser_protocol::page::{
    EnableParams as PageEnable, EventLifecycleEvent, NavigateParams, SetLifecycleEventsEnabledParams,
};
use futures::StreamExt;
use tokio::sync::{Mutex, Semaphore};
use tokio::task::JoinHandle;

use crate::browser::diagnostics::{BrowserDiagnostics, ConsoleMessage, Severity};
use crate::browser::launcher;
use crate::engine::fetcher::Fetcher;
use crate::engine::http_client::HttpClient;
use crate::engine::http_response::HttpResponse;
use crate::error::CrawlerResult;
use crate::options::core_options::CoreOptions;

pub struct BrowserRenderer {
    browser: Mutex<Browser>,
    handler_handle: JoinHandle<()>,
    http: HttpClient,
    options: Arc<CoreOptions>,
    page_semaphore: Arc<Semaphore>,
    /// Unique per-run Chrome user-data dir, removed on shutdown.
    profile_dir: std::path::PathBuf,
}

impl BrowserRenderer {
    /// Launch the browser and build the renderer. The inner `HttpClient` is reused for
    /// metadata (status/headers/cache) and for fetching non-rendered assets.
    pub async fn new(options: Arc<CoreOptions>, http: HttpClient) -> CrawlerResult<Self> {
        let executable = launcher::resolve_executable(&options).await?;
        let (browser, handler_handle, profile_dir) = launcher::launch(&options, &executable).await?;
        // Headful mode renders one page at a time so the windows are watchable.
        let workers = if options.browser_headful {
            1
        } else {
            options.browser_workers.max(1) as usize
        };
        Ok(Self {
            browser: Mutex::new(browser),
            handler_handle,
            http,
            options,
            page_semaphore: Arc::new(Semaphore::new(workers)),
            profile_dir,
        })
    }

    /// Whether this response should be rendered in the browser (vs. returned as-is).
    fn should_render(&self, resp: &HttpResponse) -> bool {
        if resp.is_skipped() {
            return false;
        }
        let status = resp.status_code;
        if status <= 0 {
            return false; // connection/timeout errors
        }
        if (300..320).contains(&status) {
            return false; // redirects are followed by the crawler
        }
        if self.options.browser_render_all {
            return status >= 200;
        }
        is_html(resp)
    }

    /// Render a URL and return the post-JS rendered HTML + diagnostics. Errors are returned as
    /// strings so the caller can fall back to the plain HTTP response. The page is always closed
    /// (bounded) and every long step is timeout-guarded so a wedged page can't hang the crawl.
    async fn render(&self, url: &str, user_agent: &str) -> Result<(String, BrowserDiagnostics), String> {
        let _permit = self
            .page_semaphore
            .acquire()
            .await
            .map_err(|e| format!("page semaphore closed: {}", e))?;

        let start = std::time::Instant::now();
        let hard_timeout = Duration::from_secs(self.options.browser_timeout.max(1) as u64);

        // Open a blank page first so the diagnostics collector can attach BEFORE navigation
        // (CDP domains replay their backlog only after `enable`). Page creation is serialized
        // (cheap); the heavy rendering then runs without holding the browser lock.
        let page = {
            let browser = self.browser.lock().await;
            browser
                .new_page("about:blank")
                .await
                .map_err(|e| format!("new_page failed: {}", e))?
        };

        // Match the crawler's request identity (same User-Agent) so the rendered DOM and the
        // HTTP metadata describe the same variant of the page.
        let _ = page
            .execute(SetUserAgentOverrideParams::new(user_agent.to_string()))
            .await;

        let collector = crate::browser::diagnostics::collect::Collector::attach(&page, url).await;

        // Enable the Page domain + lifecycle events so we can honor --browser-wait. The Page
        // domain is required for lifecycle events; chromiumoxide's goto() enabled it internally,
        // but we navigate via raw Page.navigate (so the wait can stop before `load`).
        let _ = page.execute(PageEnable::default()).await;
        let _ = page.execute(SetLifecycleEventsEnabledParams::new(true)).await;
        let mut lifecycle = page.event_listener::<EventLifecycleEvent>().await.ok();

        // URL-encode spaces the way the HTTP client does, for a consistent request.
        let nav_url = url.replace(' ', "%20");
        let wait_for = self.options.browser_wait.as_str();

        // Navigate AND wait for the requested readiness signal under ONE hard timeout, so a slow
        // navigation / slow subresource can't hang the crawl. A navigation error returned within
        // the budget is a hard failure (caller keeps the HTTP response); a timeout means we
        // capture whatever has rendered so far.
        let nav = async {
            // Trigger navigation via raw Page.navigate, which returns immediately — `page.goto()`
            // blocks until the `load` event, which would defeat the `domcontentloaded` wait
            // semantics (we could never stop earlier than `load`). A hard navigation failure
            // (DNS/conn/cert) surfaces as `error_text`.
            let resp = page
                .execute(NavigateParams::new(nav_url.clone()))
                .await
                .map_err(|e| format!("navigate failed: {}", e))?;
            if let Some(err) = resp.result.error_text.as_deref()
                && !err.is_empty()
            {
                return Err(format!("navigation failed: {}", err));
            }

            // Poll document.readyState — a STATE, not an event, so there is no race with a
            // DOMContentLoaded/load that fires before we'd start reading an event stream (that
            // race made very fast pages hang). `interactive` = DOMContentLoaded, `complete` = load.
            let need_complete = wait_for != "domcontentloaded";
            loop {
                // Bound each evaluate so a pathological page (a sub-resource that hangs at the TCP
                // level, keeping the navigation in-flight) can't wedge the poll; an evaluate
                // failure/timeout just means "not ready yet" and we retry under the outer budget.
                let ready_state =
                    match tokio::time::timeout(Duration::from_secs(2), page.evaluate("document.readyState")).await {
                        Ok(Ok(r)) => r.into_value::<String>().unwrap_or_default(),
                        _ => String::new(),
                    };
                let reached = if need_complete {
                    ready_state == "complete"
                } else {
                    ready_state == "interactive" || ready_state == "complete"
                };
                if reached {
                    break;
                }
                tokio::time::sleep(Duration::from_millis(50)).await;
            }

            // For networkidle, additionally wait (briefly, bounded) for the network to go
            // (near-)idle after load. These lifecycle events fire late; bound the wait so a
            // missed event can't block the whole render budget.
            if wait_for == "networkidle"
                && let Some(stream) = lifecycle.as_mut()
            {
                let _ = tokio::time::timeout(Duration::from_secs(3), async {
                    while let Some(ev) = stream.next().await {
                        if ev.name == "networkIdle" || ev.name == "networkAlmostIdle" {
                            break;
                        }
                    }
                })
                .await;
            }
            Ok::<(), String>(())
        };

        // Run the navigation/content body, then ALWAYS finish the collector (aborts the listener
        // tasks) and close the page — on every path, including errors — so nothing lingers on a
        // wedged page.
        let nav_result = tokio::time::timeout(hard_timeout, nav).await;
        let nav_timed_out = nav_result.is_err();
        let outcome: Result<String, String> = match nav_result {
            // goto failed → keep the HTTP response, don't capture about:blank.
            Ok(Err(e)) => Err(e),
            // Settled or timed out: capture whatever rendered.
            _ => {
                if self.options.browser_wait_extra_ms > 0 {
                    tokio::time::sleep(Duration::from_millis(self.options.browser_wait_extra_ms as u64)).await;
                }
                // Brief grace so late CDP diagnostic events are collected before we stop listening.
                tokio::time::sleep(Duration::from_millis(50)).await;
                // content() must not hang the crawl — bound it.
                match tokio::time::timeout(Duration::from_secs(15), page.content()).await {
                    Ok(Ok(h)) => Ok(h),
                    Ok(Err(e)) => Err(format!("content() failed: {}", e)),
                    Err(_) => Err("content() timed out".to_string()),
                }
            }
        };

        // Screenshot (bounded) only when content succeeded; capture the error instead of dropping it.
        let mut screenshot_path = None;
        let mut screenshot_error = None;
        if outcome.is_ok() && self.options.screenshots {
            // Best-effort cookie-banner removal before capture (fail-soft).
            if self.options.screenshot_hide_cookie_banners || self.options.screenshot_hide_selector.is_some() {
                let _ = tokio::time::timeout(
                    Duration::from_secs(5),
                    crate::browser::cookie_consent::dismiss(&page, &self.options),
                )
                .await;
                // Let the hide/animations settle before snapping.
                tokio::time::sleep(Duration::from_millis(400)).await;
            }
            match tokio::time::timeout(
                Duration::from_secs(30),
                crate::browser::screenshot::capture(&page, &self.options, url),
            )
            .await
            {
                Ok(Ok(p)) => screenshot_path = Some(p),
                Ok(Err(e)) => screenshot_error = Some(e),
                Err(_) => screenshot_error = Some("screenshot timed out".to_string()),
            }
        }

        // Cleanup — ALWAYS abort the collector tasks and close the page, on success and error.
        let mut diagnostics = collector.finish();
        diagnostics.render_total_ms = start.elapsed().as_millis() as u64;
        diagnostics.screenshot_path = screenshot_path;
        diagnostics.screenshot_error = screenshot_error;
        // A navigation that hit the hard timeout (readiness signal never reached) → record a
        // warning so the page isn't reported as fully OK despite an incomplete render.
        if nav_timed_out && outcome.is_ok() {
            diagnostics.console.push(ConsoleMessage {
                severity: Severity::Warning,
                kind: "navigation".to_string(),
                text: format!(
                    "navigation wait '{}' did not complete within --browser-timeout={}s; page may be incompletely rendered",
                    self.options.browser_wait,
                    self.options.browser_timeout.max(1)
                ),
                url: None,
                line: None,
            });
        }
        // Re-tally AFTER setting screenshot_error / the nav-timeout warning, so they are
        // reflected in the severity counts (collector.finish() ran recount() before these).
        diagnostics.recount();
        close_page(page).await;

        outcome.map(|html| (html, diagnostics))
    }
}

#[async_trait]
impl Fetcher for BrowserRenderer {
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
        // Authoritative status/headers/cache/redirect handling via the inner HTTP client.
        let http_resp = self
            .http
            .fetch(
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
            .await?;

        if !self.should_render(&http_resp) {
            return Ok(http_resp);
        }

        // Reconstruct the absolute URL for the browser to navigate.
        let authority = if (scheme == "https" && port == 443) || (scheme == "http" && port == 80) {
            host.to_string()
        } else {
            format!("{}:{}", host, port)
        };
        let absolute_url = format!("{}://{}{}", scheme, authority, url);

        let http_exec_time = http_resp.exec_time;
        match self.render(&absolute_url, user_agent).await {
            Ok((rendered_html, diagnostics)) => {
                let mut resp = http_resp;
                // Only replace the body for actual HTML documents. Under --browser-render-all a
                // non-HTML response (PDF/image/...) keeps its original bytes but still gets
                // diagnostics/screenshot — replacing it with the viewer DOM would corrupt exports.
                if is_html(&resp) {
                    resp.body = Some(rendered_html.into_bytes());
                }
                // Reflect the real wall time the user waited (HTTP preflight + browser render),
                // so slowest/performance/CI metrics aren't just the preflight time.
                resp.exec_time = http_exec_time + (diagnostics.render_total_ms as f64) / 1000.0;
                resp.set_browser_diagnostics(diagnostics);
                Ok(resp)
            }
            // Render failed: keep the HTTP response so the crawl never breaks, but record the
            // failure so the analyzer/summary don't falsely report "Browser OK".
            Err(e) => {
                let mut resp = http_resp;
                let mut diag = BrowserDiagnostics {
                    render_error: Some(e),
                    ..Default::default()
                };
                diag.recount();
                resp.set_browser_diagnostics(diag);
                Ok(resp)
            }
        }
    }

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
        // Direct HTTP via the inner client — never rendered or screenshotted (e.g. robots.txt).
        self.http
            .fetch(
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
        _host: &str,
        _port: u16,
        _scheme: &str,
        _url: &str,
        _http_method: &str,
        _user_agent: &str,
        _accept: &str,
        _accept_encoding: &str,
        _origin: Option<&str>,
    ) -> bool {
        // The browser always fetches rendered documents live (it does not use the HTTP cache),
        // so never let the crawl loop skip the inter-request rate-limit delay on a "cache hit".
        false
    }

    async fn shutdown(&self) {
        {
            let mut browser = self.browser.lock().await;
            // Bound BOTH close() and wait() so no shutdown path can hang on a wedged browser.
            let _ = tokio::time::timeout(Duration::from_secs(5), browser.close()).await;
            // Wait for the Chrome process to actually exit (releasing its profile file locks)
            // before we remove the profile dir.
            let _ = tokio::time::timeout(Duration::from_secs(5), browser.wait()).await;
        }
        self.handler_handle.abort();
        // Remove the unique per-run Chrome profile dir (best-effort; a leftover is harmless).
        let _ = std::fs::remove_dir_all(&self.profile_dir);
    }
}

/// Whether a response is an HTML document (by content-type).
fn is_html(resp: &HttpResponse) -> bool {
    resp.get_content_type()
        .map(|ct| ct.to_lowercase().contains("text/html"))
        .unwrap_or(false)
}

/// Close a page, bounded so a wedged page can't hang the crawl.
async fn close_page(page: chromiumoxide::Page) {
    let _ = tokio::time::timeout(Duration::from_secs(5), page.close()).await;
}
