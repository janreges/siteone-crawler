// SiteOne Crawler - Browser launcher (detection / download / launch)
// (c) Jan Reges <jan.reges@siteone.cz>
//
// Compiled only with the `browser` Cargo feature. Resolves a Chromium-family
// executable (explicit path → system detection → consented download) and launches it.

use std::path::{Path, PathBuf};

use chromiumoxide::detection::{DetectionOptions, default_executable};
use chromiumoxide::handler::viewport::Viewport;
use chromiumoxide::{Browser, BrowserConfig};
use futures::StreamExt;
use tokio::task::JoinHandle;

use crate::error::{CrawlerError, CrawlerResult};
use crate::options::core_options::CoreOptions;

/// Resolve the browser executable to drive.
///
/// Tier 0: explicit `--browser-path`.
/// Tier 1: system detection (also honors the `CHROME` env var) — Chrome/Chromium/Edge/Brave.
/// Tier 2: download `chrome-headless-shell` (with consent / `--browser-auto-download`).
pub async fn resolve_executable(options: &CoreOptions) -> CrawlerResult<PathBuf> {
    // Tier 0: explicit path.
    if let Some(p) = &options.browser_path {
        let pb = PathBuf::from(p);
        if pb.exists() {
            return Ok(pb);
        }
        return Err(CrawlerError::Config(format!(
            "--browser-path '{}' does not exist or is not accessible",
            p
        )));
    }

    // Tier 1: detect an installed Chromium-family browser.
    if let Ok(path) = default_executable(DetectionOptions::default()) {
        return Ok(path);
    }

    // Tier 2: download (with consent).
    download_browser(options).await
}

/// Download `chrome-headless-shell` from Chrome for Testing into a per-user cache dir.
async fn download_browser(options: &CoreOptions) -> CrawlerResult<PathBuf> {
    use chromiumoxide::fetcher::{BrowserFetcher, BrowserFetcherOptions};

    let cache = dirs::cache_dir()
        .map(|d| d.join("siteone-crawler").join("browser"))
        .ok_or_else(|| {
            CrawlerError::Config("could not determine a cache directory for the browser download".to_string())
        })?;

    // Consent: explicit pre-consent for CI/non-interactive, otherwise an interactive prompt.
    let consented = if options.browser_auto_download {
        true
    } else if std::io::IsTerminal::is_terminal(&std::io::stdin()) {
        inquire::Confirm::new(&format!(
            "No Chromium-family browser found. Download chrome-headless-shell into '{}'?",
            cache.display()
        ))
        .with_default(true)
        .prompt()
        .unwrap_or(false)
    } else {
        false
    };

    if !consented {
        return Err(CrawlerError::Config(
            "no browser found and download was not permitted. Install Chrome/Chromium/Edge/Brave, pass --browser-path=<exe>, or allow --browser-auto-download.".to_string(),
        ));
    }

    std::fs::create_dir_all(&cache).ok();

    let fetcher_opts = BrowserFetcherOptions::builder()
        .with_path(&cache)
        .build()
        .map_err(|e| CrawlerError::Config(format!("browser fetcher options error: {}", e)))?;
    let installation = BrowserFetcher::new(fetcher_opts)
        .fetch()
        .await
        .map_err(|e| CrawlerError::Config(format!("browser download failed: {}", e)))?;

    Ok(installation.executable_path)
}

/// Launch the browser and spawn its CDP handler loop on the tokio runtime.
/// Returns the `Browser` plus the handler `JoinHandle` (abort it on shutdown).
pub async fn launch(options: &CoreOptions, executable: &Path) -> CrawlerResult<(Browser, JoinHandle<()>, PathBuf)> {
    let mut builder = BrowserConfig::builder().chrome_executable(executable);

    if options.browser_headful {
        builder = builder.with_head();
    }

    // Unique per-run user-data dir, so a leftover or concurrent Chrome using the default profile
    // can't crash this run on a SingletonLock. Cache (CSS/JS/img) stays shared within this run
    // but never collides between runs. PID + a nanosecond timestamp make it robust even against
    // PID reuse (tests/embedding/crash). Removed on shutdown.
    let unique = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let profile_dir = std::env::temp_dir().join(format!("siteone-crawler-browser-{}-{}", std::process::id(), unique));
    let _ = std::fs::create_dir_all(&profile_dir);
    builder = builder.user_data_dir(&profile_dir);

    // Reject invalid TLS certificates by default (match the direct-HTTP path); chromiumoxide
    // ignores HTTPS errors by default, which would otherwise let browser sub-resources load
    // over invalid certs even when the HTTP path wouldn't. Honor --accept-invalid-certs.
    if !options.accept_invalid_certs {
        builder = builder.respect_https_errors();
    }

    // Viewport used for rendering and viewport screenshots (from --screenshot-viewport).
    // window_size affects the OS window (headful); viewport sets the actual render surface
    // (headless otherwise defaults to 800x600).
    let (vw, vh) = parse_viewport(&options.screenshot_viewport);
    builder = builder.window_size(vw, vh).viewport(Viewport {
        width: vw,
        height: vh,
        ..Default::default()
    });

    // Forward the crawler's egress controls so browser traffic matches the HTTP path
    // (otherwise Chromium would bypass --proxy and --resolve entirely).
    if let Some(proxy) = proxy_server_arg(options.proxy.as_deref()) {
        builder = builder.arg(format!("--proxy-server={}", proxy));
    }
    if let Some(rules) = host_resolver_rules(&options.resolve) {
        builder = builder.arg(format!("--host-resolver-rules={}", rules));
    }

    // The Chrome sandbox is the primary containment boundary for untrusted page JS; keep it
    // ON by default. --no-sandbox is opt-in via --browser-no-sandbox (commonly required in
    // Docker/CI/WSL or when running as root).
    if options.browser_no_sandbox {
        builder = builder.no_sandbox();
    }
    // /dev/shm exhaustion crashes headless Chrome in constrained envs; this does NOT weaken
    // the sandbox, so it is always safe on Linux.
    #[cfg(target_os = "linux")]
    {
        builder = builder.arg("--disable-dev-shm-usage");
    }

    let config = builder
        .build()
        .map_err(|e| CrawlerError::Config(format!("invalid browser configuration: {}", e)))?;

    let (browser, mut handler) = Browser::launch(config)
        .await
        .map_err(|e| CrawlerError::Config(format!("failed to launch browser '{}': {}", executable.display(), e)))?;

    let handle = tokio::spawn(async move {
        while let Some(event) = handler.next().await {
            // Drain the handler stream; errors are non-fatal to the crawl.
            let _ = event;
        }
    });

    Ok((browser, handle, profile_dir))
}

/// Build the `--proxy-server` value from the crawler's `host:port` proxy option.
fn proxy_server_arg(proxy: Option<&str>) -> Option<String> {
    let p = proxy?.trim();
    if p.is_empty() { None } else { Some(p.to_string()) }
}

/// Build a `--host-resolver-rules` value ("MAP host ip, ...") from the crawler's
/// `--resolve` entries (each `domain:port:IP`; the port is not part of resolver rules).
/// Uses the same `host:port:ip` parse as the HTTP path so IPv6 IPs (which contain colons)
/// are captured correctly, and wraps IPv6 literals in brackets for Chromium's MAP syntax.
fn host_resolver_rules(resolve: &[String]) -> Option<String> {
    let re = regex::Regex::new(r"^([^:]+):([0-9]+):(.+)$").ok()?;
    let rules: Vec<String> = resolve
        .iter()
        .filter_map(|entry| {
            let caps = re.captures(entry)?;
            let host = caps.get(1)?.as_str();
            let ip = caps.get(3)?.as_str();
            if host.is_empty() || ip.is_empty() {
                return None;
            }
            let ip_fmt = if ip.contains(':') && !ip.starts_with('[') {
                format!("[{}]", ip)
            } else {
                ip.to_string()
            };
            Some(format!("MAP {} {}", host, ip_fmt))
        })
        .collect();
    if rules.is_empty() { None } else { Some(rules.join(", ")) }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolver_rules_ipv4() {
        let r = host_resolver_rules(&["example.com:443:1.2.3.4".to_string()]);
        assert_eq!(r.as_deref(), Some("MAP example.com 1.2.3.4"));
    }

    #[test]
    fn resolver_rules_ipv6_is_bracketed() {
        let r = host_resolver_rules(&["example.com:443:2001:db8::1".to_string()]);
        assert_eq!(r.as_deref(), Some("MAP example.com [2001:db8::1]"));
    }

    #[test]
    fn resolver_rules_skips_invalid_and_joins() {
        let r = host_resolver_rules(&[
            "a.com:80:1.1.1.1".to_string(),
            "garbage".to_string(),
            "b.com:443:2.2.2.2".to_string(),
        ]);
        assert_eq!(r.as_deref(), Some("MAP a.com 1.1.1.1, MAP b.com 2.2.2.2"));
    }

    #[test]
    fn proxy_arg_passthrough_and_empty() {
        assert_eq!(
            proxy_server_arg(Some("127.0.0.1:8080")).as_deref(),
            Some("127.0.0.1:8080")
        );
        assert_eq!(proxy_server_arg(None), None);
        assert_eq!(proxy_server_arg(Some("   ")), None);
    }

    #[test]
    fn viewport_parsing() {
        assert_eq!(parse_viewport("1280x720"), (1280, 720));
        assert_eq!(parse_viewport("bad"), (1920, 1080));
    }
}

/// Parse a `WxH` viewport string into `(width, height)`, falling back to 1920x1080.
fn parse_viewport(s: &str) -> (u32, u32) {
    let mut parts = s.split(['x', 'X']);
    let w = parts
        .next()
        .and_then(|p| p.trim().parse::<u32>().ok())
        .filter(|&w| w > 0)
        .unwrap_or(1920);
    let h = parts
        .next()
        .and_then(|p| p.trim().parse::<u32>().ok())
        .filter(|&h| h > 0)
        .unwrap_or(1080);
    (w, h)
}
