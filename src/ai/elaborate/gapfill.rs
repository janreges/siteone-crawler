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

use crate::engine::http_client::HttpClient;
use crate::engine::parsed_url::ParsedUrl;
use crate::engine::robots_txt::RobotsTxt;
use crate::options::core_options::CoreOptions;
use crate::result::status::Status;
use crate::result::visited_url::{SOURCE_A_HREF, VisitedUrl};
use crate::types::ContentTypeId;

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
    );

    // Politeness: at least 250 ms between sends, or the configured 1/max-reqs-per-sec if slower.
    let delay = if options.max_reqs_per_sec > 0.0 {
        Duration::from_secs_f64((1.0 / options.max_reqs_per_sec).max(0.25))
    } else {
        Duration::from_millis(250)
    };
    let user_agent = status
        .lock()
        .ok()
        .map(|st| st.get_crawler_info().final_user_agent)
        .filter(|ua| !ua.is_empty())
        .unwrap_or_else(|| format!("SiteOne-Crawler/{}", crate::version::CODE));
    let timeout = options.timeout.clamp(1, 3600) as u64;

    for (i, target) in targets.iter().enumerate() {
        if i > 0 {
            tokio::time::sleep(delay).await;
        }
        let Some((host, port, scheme)) = split_url(target) else {
            report.failed += 1;
            continue;
        };
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
                true,
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
}
