// SiteOne Crawler - brand elaborate: bounded targeted gap-fill fetch
// (c) Jan Reges <jan.reges@siteone.cz>
//
// The crawl's Fetcher is shut down before the AI phase, so when brand-elaborate finds important
// global-navigation pages the crawl never visited (typical under --single-page or a page cap), it
// fetches a SMALL, capped set here through its own short-lived HttpClient. Robots.txt is honored by
// reusing the robots content the crawler already stored in Status, and only 200 text/html responses
// are stored back into Status so the normal candidate/extraction path can pick them up — anything
// else is counted and dropped (never fabricated).

use std::collections::HashSet;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use md5::{Digest, Md5};

use crate::ai::progress;
use crate::engine::crawler::Crawler;
use crate::engine::http_client::HttpClient;
use crate::engine::parsed_url::ParsedUrl;
use crate::engine::robots_txt::RobotsTxt;
use crate::options::core_options::CoreOptions;
use crate::result::status::Status;
use crate::result::visited_url::{SOURCE_A_HREF, VisitedUrl};
use crate::types::ContentTypeId;

/// Progress task: one unit per page to fetch.
const TASK: &str = "elaborate:gapfill";

const ACCEPT_HEADER: &str = "text/html,application/xhtml+xml,application/xml;q=0.9,image/avif,image/webp,image/apng,*/*;q=0.8,application/signed-exchange;v=b3;q=0.7";

#[derive(Debug, Default, Clone)]
pub struct GapFillReport {
    pub fetched: usize,
    pub skipped_robots: usize,
    pub failed: usize,
}

/// Select the URLs actually eligible to fetch: deduped, robots-cleared, capped. Pure (no I/O) so it
/// is unit-testable; `robots` is the parsed robots.txt (`None` = allow all / `--ignore-robots-txt`).
/// Returns `(targets, skipped_by_robots)`.
pub fn plan_fetch(urls: &[String], robots: Option<&RobotsTxt>, cap: usize) -> (Vec<String>, usize) {
    let mut seen: HashSet<&str> = HashSet::new();
    let mut targets = Vec::new();
    let mut skipped_robots = 0usize;
    for u in urls {
        if targets.len() >= cap {
            break;
        }
        if !seen.insert(u.as_str()) {
            continue;
        }
        if let Some(r) = robots
            && !r.is_allowed(u)
        {
            skipped_robots += 1;
            continue;
        }
        targets.push(u.clone());
    }
    (targets, skipped_robots)
}

/// Fetch up to `cap` of the given (already mask-cleared, internal) URLs and store the 200-HTML ones
/// back into `Status`. Fail-soft: any error on a single URL just increments `failed`.
pub async fn fetch_missing(
    urls: &[String],
    options: &CoreOptions,
    status: &Arc<Mutex<Status>>,
    cap: usize,
) -> GapFillReport {
    if cap == 0 || urls.is_empty() {
        return GapFillReport::default();
    }

    // Robots.txt: reuse the content the crawler already fetched for the initial host.
    let robots = if options.ignore_robots_txt {
        None
    } else {
        let scheme = options.get_initial_scheme();
        let host = options.get_initial_host(false);
        let port = initial_port(options);
        status
            .lock()
            .ok()
            .and_then(|st| st.get_robots_txt_content(&scheme, &host, port))
            .map(|c| RobotsTxt::parse(&c))
    };

    let (targets, skipped_robots) = plan_fetch(urls, robots.as_ref(), cap);
    let mut report = GapFillReport {
        skipped_robots,
        ..Default::default()
    };
    if targets.is_empty() {
        return report;
    }

    // Own short-lived client, built exactly like the crawler's (proxy, auth, cache, TLS).
    let http_cache_dir =
        if options.http_cache_dir.as_deref() == Some("off") || options.http_cache_dir.as_deref() == Some("") {
            None
        } else {
            options
                .http_cache_dir
                .as_ref()
                .map(|dir| crate::utils::get_absolute_path(dir))
        };
    let client = HttpClient::new(
        options.proxy.clone(),
        options.http_auth.clone(),
        http_cache_dir,
        options.http_cache_compression,
        options.http_cache_ttl,
        options.accept_invalid_certs,
    )
    .with_custom_headers(&options.http_headers);

    // Politeness: at least 250 ms between sends, or the configured 1/max-reqs-per-sec if slower.
    let delay = if options.max_reqs_per_sec > 0.0 {
        Duration::from_secs_f64((1.0 / options.max_reqs_per_sec).max(0.25))
    } else {
        Duration::from_millis(250)
    };
    // The crawler's own user agent (the reported one may be a `--header` value, which the client
    // adds only within the crawled site).
    let user_agent = Crawler::build_final_user_agent(options);
    let timeout = options.timeout.clamp(1, 3600) as u64;
    let initial_url = ParsedUrl::parse(&options.url, None);

    progress::start(TASK, "Elaborate: gap-fill", targets.len() as u64);
    for (i, target) in targets.iter().enumerate() {
        // Every target counts once, fetched or not.
        let _counted = progress::advance_on_drop(TASK);
        if i > 0 {
            tokio::time::sleep(delay).await;
        }
        let Some((host, port, scheme)) = split_url(target) else {
            report.failed += 1;
            continue;
        };
        // Credentials (--http-auth, --header) only where the crawler itself would send them.
        let use_credentials = ParsedUrl::may_send_credentials(&initial_url, &scheme, &host, port);
        let resp = client
            .request(
                &host,
                port,
                &scheme,
                target,
                "GET",
                timeout,
                &user_agent,
                ACCEPT_HEADER,
                &options.accept_encoding,
                None,
                use_credentials,
                None,
            )
            .await;
        match resp {
            Ok(r) if r.status_code == 200 && is_html(&r) => {
                let body = r.body.clone().unwrap_or_default();
                if body.is_empty() {
                    report.failed += 1;
                    continue;
                }
                let content_type_header = r.get_header("content-type").cloned();
                let content_encoding = r.get_header("content-encoding").cloned();
                let visited = VisitedUrl::new(
                    uq_id_for(target),
                    String::new(),
                    SOURCE_A_HREF,
                    target.clone(),
                    200,
                    r.exec_time,
                    Some(body.len() as i64),
                    ContentTypeId::Html,
                    content_type_header,
                    content_encoding,
                    None,
                    false,
                    true,
                    0,
                    None,
                );
                if let Ok(mut st) = status.lock() {
                    st.add_visited_url(visited, Some(&body), None);
                    report.fetched += 1;
                } else {
                    report.failed += 1;
                }
            }
            _ => report.failed += 1,
        }
    }
    progress::finish(TASK);
    report
}

fn is_html(resp: &crate::engine::http_response::HttpResponse) -> bool {
    resp.get_header("content-type")
        .map(|ct| ct.to_lowercase().contains("text/html"))
        .unwrap_or(false)
}

/// uq_id for a gap-filled page: MD5(url)[..8]. These URLs were never crawled (that is why they are
/// gap-filled), so this cannot collide with an existing entry for a different URL.
fn uq_id_for(url: &str) -> String {
    let mut hasher = Md5::new();
    hasher.update(url.as_bytes());
    crate::utils::to_lower_hex(hasher.finalize())[..8].to_string()
}

fn split_url(url: &str) -> Option<(String, u16, String)> {
    let parsed = url::Url::parse(url).ok()?;
    let host = parsed.host_str()?.to_string();
    let scheme = parsed.scheme().to_string();
    let port = parsed
        .port_or_known_default()
        .unwrap_or(if scheme == "https" { 443 } else { 80 });
    Some((host, port, scheme))
}

fn initial_port(options: &CoreOptions) -> u16 {
    let scheme = options.get_initial_scheme();
    ParsedUrl::parse(&options.url, None)
        .port
        .unwrap_or(if scheme == "https" { 443 } else { 80 })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plan_fetch_caps_dedups_and_respects_robots() {
        let urls = vec![
            "https://x.test/a".to_string(),
            "https://x.test/a".to_string(), // dup
            "https://x.test/private".to_string(),
            "https://x.test/b".to_string(),
            "https://x.test/c".to_string(),
        ];
        let robots = RobotsTxt::parse("User-agent: *\nDisallow: /private");
        let (targets, skipped) = plan_fetch(&urls, Some(&robots), 2);
        // /private is robots-denied (counted), dup collapsed, capped to 2.
        assert_eq!(
            targets,
            vec!["https://x.test/a".to_string(), "https://x.test/b".to_string()]
        );
        assert_eq!(skipped, 1);
    }

    #[test]
    fn plan_fetch_none_robots_allows_all() {
        let urls = vec!["https://x.test/a".to_string(), "https://x.test/b".to_string()];
        let (targets, skipped) = plan_fetch(&urls, None, 10);
        assert_eq!(targets.len(), 2);
        assert_eq!(skipped, 0);
    }

    #[test]
    fn uq_id_is_stable_8_hex() {
        let id = uq_id_for("https://x.test/a");
        assert_eq!(id.len(), 8);
        assert!(id.chars().all(|c| c.is_ascii_hexdigit()));
        assert_eq!(id, uq_id_for("https://x.test/a")); // stable
        assert_ne!(id, uq_id_for("https://x.test/b"));
    }

    #[test]
    fn split_url_derives_host_port_scheme() {
        assert_eq!(
            split_url("https://x.test/a"),
            Some(("x.test".to_string(), 443, "https".to_string()))
        );
        assert_eq!(
            split_url("http://x.test:8080/a"),
            Some(("x.test".to_string(), 8080, "http".to_string()))
        );
        assert_eq!(split_url("not a url"), None);
    }

    /// Answers one request with a small HTML page and passes the request head back.
    fn serve_page_once() -> (u16, std::sync::mpsc::Receiver<String>) {
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
            let body = b"<html><body><p>Page</p></body></html>";
            let mut response = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: text/html\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                body.len()
            )
            .into_bytes();
            response.extend_from_slice(body);
            std::io::Write::write_all(&mut stream, &response).ok();
        });
        (port, receiver)
    }

    /// #21: gap-fill sends `--header` credentials only where the crawler would: to the crawled
    /// site, and for an IP host only on its port, never to another service on the same address.
    #[tokio::test]
    async fn gap_fill_sends_credentials_only_within_the_crawled_site() {
        let (site_port, site_request) = serve_page_once();
        let (other_port, other_request) = serve_page_once();
        let options = crate::options::core_options::parse_argv(&[
            "siteone-crawler".to_string(),
            format!("--url=http://127.0.0.1:{site_port}/"),
            format!("--config-file={}", if cfg!(windows) { "NUL" } else { "/dev/null" }),
            "--header=Cookie: session=abc".to_string(),
            "--http-cache-dir=".to_string(),
        ])
        .unwrap();
        let info = crate::info::Info::new(
            "SiteOne Crawler".to_string(),
            crate::version::CODE.to_string(),
            String::new(),
            String::new(),
            String::new(),
            String::new(),
            options.url.clone(),
        );
        let status = Arc::new(Mutex::new(Status::new(
            Box::new(crate::result::storage::memory_storage::MemoryStorage::new(false)),
            true,
            info,
            std::time::Instant::now(),
        )));

        let urls = vec![
            format!("http://127.0.0.1:{site_port}/missing"),
            format!("http://127.0.0.1:{other_port}/missing"),
        ];
        let report = fetch_missing(&urls, &options, &status, 10).await;
        assert_eq!(report.fetched, 2);

        let site_head = site_request.recv().unwrap().to_ascii_lowercase();
        assert!(site_head.contains("cookie: session=abc"), "{site_head}");
        let other_head = other_request.recv().unwrap().to_ascii_lowercase();
        assert!(!other_head.contains("cookie:"), "{other_head}");
    }
}
