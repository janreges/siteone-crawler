// SiteOne Crawler - Core Crawler Engine
// (c) Jan Reges <jan.reges@siteone.cz>
//
// Main crawling engine with concurrent URL processing.

use std::collections::{HashMap, VecDeque};
use std::sync::atomic::{AtomicBool, AtomicI64, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

use dashmap::DashMap;
use md5::{Digest, Md5};
use once_cell::sync::Lazy;
use regex::Regex;
use tokio::sync::Semaphore;

/// Regex to extract <base href="..."> from HTML
static RE_BASE_HREF: Lazy<Regex> = Lazy::new(|| Regex::new(r#"(?is)<base[^>]+href\s*=\s*["']?([^"'\s>]+)"#).unwrap());

/// Static regexes for title/description/keywords extraction (Fix #12)
static RE_TITLE: Lazy<Regex> = Lazy::new(|| Regex::new(r"(?is)<title[^>]*>([^<]*)</title>").unwrap());
static RE_DESCRIPTION: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r#"(?is)<meta\s+[^>]*name=["']description["']\s+[^>]*content=["']([^"']+)["'][^>]*>"#).unwrap()
});
static RE_KEYWORDS: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r#"(?is)<meta\s+[^>]*name=["']keywords["']\s+[^>]*content=["']([^"']+)["'][^>]*>"#).unwrap()
});
static RE_DOM_COUNT: Lazy<Regex> = Lazy::new(|| Regex::new(r"<\w+").unwrap());

use crate::analysis::manager::AnalysisManager;
use crate::content_processor::html_processor::HTML_PAGES_EXTENSIONS;
use crate::content_processor::manager::ContentProcessorManager;
use crate::engine::found_url::UrlSource;
use crate::engine::found_urls::FoundUrls;
use crate::engine::http_client::HttpClient;
use crate::engine::http_response::HttpResponse;
use crate::engine::parsed_url::ParsedUrl;
use crate::engine::robots_txt::RobotsTxt;
use crate::error::CrawlerResult;
use crate::options::core_options::CoreOptions;
use crate::output::output::Output;
use crate::result::status::Status;
use crate::result::visited_url::VisitedUrl;
use crate::types::{ContentTypeId, DeviceType, SkippedReason};
use crate::utils;
use crate::version;

/// Entry in the URL queue
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct QueueEntry {
    pub url: String,
    pub uq_id: String,
    pub source_uq_id: String,
    pub source_attr: i32,
}

/// Entry for a visited URL in the visited table
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct VisitedEntry {
    pub url: String,
    pub uq_id: String,
    pub source_uq_id: String,
    pub source_attr: i32,
}

/// Entry for a skipped URL
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct SkippedEntry {
    pub url: String,
    pub reason: SkippedReason,
    pub source_uq_id: String,
    pub source_attr: i32,
}

/// Accept header for HTTP requests
const ACCEPT_HEADER: &str = "text/html,application/xhtml+xml,application/xml;q=0.9,image/avif,image/webp,image/apng,*/*;q=0.8,application/signed-exchange;v=b3;q=0.7";

/// Main crawler engine
pub struct Crawler {
    options: Arc<CoreOptions>,
    http_client: Arc<HttpClient>,
    content_processor_manager: Arc<Mutex<ContentProcessorManager>>,
    analysis_manager: Arc<Mutex<AnalysisManager>>,
    output: Arc<Mutex<Box<dyn Output>>>,
    status: Arc<Mutex<Status>>,

    /// URL queue (key = md5 of full URL, value = QueueEntry)
    queue: Arc<DashMap<String, QueueEntry>>,
    /// Insertion-ordered queue keys for breadth-first processing
    queue_order: Arc<Mutex<VecDeque<String>>>,
    /// Visited URLs (key = md5 of full URL, value = VisitedEntry)
    visited: Arc<DashMap<String, VisitedEntry>>,
    /// Skipped URLs (key = md5 of full URL, value = SkippedEntry)
    skipped: Arc<DashMap<String, SkippedEntry>>,

    /// Initial parsed URL
    initial_parsed_url: ParsedUrl,
    /// Final user agent string
    final_user_agent: String,
    /// Accept header (may be modified for offline export)
    accept_header: String,
    /// Whether the initial URL has been found as existing HTML
    initial_existing_url_found: Arc<AtomicBool>,
    /// Whether the crawler has been terminated
    terminated: Arc<AtomicBool>,

    /// Rate limiting: optimal delay between requests in seconds
    optimal_delay_between_requests: f64,
    /// Last request timestamp (epoch seconds)
    last_request_time: Arc<Mutex<f64>>,

    /// Counter for done URLs
    done_urls_count: Arc<AtomicUsize>,

    /// Non-200 basenames to their occurrence counts
    non200_basenames_to_occurrences: Arc<DashMap<String, i64>>,

    /// Cached robots.txt data per domain:port
    robots_txt_cache: Arc<DashMap<String, Option<RobotsTxt>>>,
    /// Counter for loaded robots.txt files
    loaded_robots_txt_count: Arc<AtomicI64>,

    /// Cached resolve mappings (domain:port -> IP)
    resolve_cache: Arc<DashMap<String, String>>,

    /// Pre-compiled include regex patterns
    compiled_include_regex: Arc<Vec<Regex>>,
    /// Pre-compiled ignore regex patterns
    compiled_ignore_regex: Arc<Vec<Regex>>,
}

impl Crawler {
    pub fn new(
        options: Arc<CoreOptions>,
        http_client: HttpClient,
        content_processor_manager: ContentProcessorManager,
        analysis_manager: AnalysisManager,
        output: Box<dyn Output>,
        status: Status,
    ) -> Self {
        let initial_parsed_url = ParsedUrl::parse(&options.url, None);
        let final_user_agent = Self::build_final_user_agent(&options);

        // Set the final user agent in status
        let status = {
            status.set_final_user_agent(&final_user_agent);
            status
        };

        let optimal_delay = (1.0 / options.max_reqs_per_sec).max(0.001);

        // Pre-compile include/ignore regex patterns
        let compiled_include_regex: Vec<Regex> = options
            .include_regex
            .iter()
            .filter_map(|p| {
                let pattern = utils::extract_pcre_regex_pattern(p);
                Regex::new(&pattern).ok()
            })
            .collect();
        let compiled_ignore_regex: Vec<Regex> = options
            .ignore_regex
            .iter()
            .filter_map(|p| {
                let pattern = utils::extract_pcre_regex_pattern(p);
                Regex::new(&pattern).ok()
            })
            .collect();

        // Build resolve cache
        let resolve_cache = DashMap::new();
        let resolve_re = Regex::new(r"^([^:]+):([0-9]+):(.+)$");
        for resolve in &options.resolve {
            if let Ok(ref re) = resolve_re
                && let Some(caps) = re.captures(resolve)
            {
                let domain = caps.get(1).map_or("", |m| m.as_str());
                let port = caps.get(2).map_or("", |m| m.as_str());
                let ip = caps.get(3).map_or("", |m| m.as_str());
                resolve_cache.insert(format!("{}:{}", domain, port), ip.to_string());
            }
        }

        Crawler {
            options,
            http_client: Arc::new(http_client),
            content_processor_manager: Arc::new(Mutex::new(content_processor_manager)),
            analysis_manager: Arc::new(Mutex::new(analysis_manager)),
            output: Arc::new(Mutex::new(output)),
            status: Arc::new(Mutex::new(status)),
            queue: Arc::new(DashMap::new()),
            queue_order: Arc::new(Mutex::new(VecDeque::new())),
            visited: Arc::new(DashMap::new()),
            skipped: Arc::new(DashMap::new()),
            initial_parsed_url,
            final_user_agent,
            accept_header: ACCEPT_HEADER.to_string(),
            initial_existing_url_found: Arc::new(AtomicBool::new(false)),
            terminated: Arc::new(AtomicBool::new(false)),
            optimal_delay_between_requests: optimal_delay,
            last_request_time: Arc::new(Mutex::new(0.0)),
            done_urls_count: Arc::new(AtomicUsize::new(0)),
            non200_basenames_to_occurrences: Arc::new(DashMap::new()),
            robots_txt_cache: Arc::new(DashMap::new()),
            loaded_robots_txt_count: Arc::new(AtomicI64::new(0)),
            resolve_cache: Arc::new(resolve_cache),
            compiled_include_regex: Arc::new(compiled_include_regex),
            compiled_ignore_regex: Arc::new(compiled_ignore_regex),
        }
    }

    /// Main crawl loop. Processes URLs concurrently with rate limiting.
    pub async fn run(&mut self) -> CrawlerResult<()> {
        // Add initial URL to queue
        self.add_url_to_queue(&self.initial_parsed_url.clone(), None, UrlSource::InitUrl as i32);

        // Print table header
        if let Ok(mut output) = self.output.lock() {
            output.add_table_header();
        }

        // Set up Ctrl+C handler
        let terminated = self.terminated.clone();
        let ctrl_c_handler = tokio::spawn(async move {
            if let Ok(()) = tokio::signal::ctrl_c().await {
                terminated.store(true, Ordering::SeqCst);
            }
        });

        // Semaphore for controlling concurrent workers
        let semaphore = Arc::new(Semaphore::new(self.options.workers as usize));
        let mut join_handles = Vec::new();

        loop {
            if self.terminated.load(Ordering::SeqCst) {
                if let Ok(mut output) = self.output.lock() {
                    output.add_notice(
                        "Crawler interrupted by user (Ctrl+C). Processing will stop after in-flight requests complete.",
                    );
                }
                break;
            }

            // Take the next URL from the queue
            let entry = self.take_next_from_queue();
            let entry = match entry {
                Some(e) => e,
                None => {
                    // Queue is empty - check if there are still active workers
                    let avail = semaphore.available_permits();
                    let total = self.options.workers as usize;
                    if avail == total {
                        // No active workers and empty queue = done
                        break;
                    }
                    // Wait a bit for workers to finish and potentially add new URLs
                    tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;
                    continue;
                }
            };

            // Acquire semaphore permit
            let permit = semaphore.clone().acquire_owned().await;
            let permit = match permit {
                Ok(p) => p,
                Err(_) => break,
            };

            // Clone all needed Arcs for the spawned task
            let options = self.options.clone();
            let http_client = self.http_client.clone();
            let content_processor_manager = self.content_processor_manager.clone();
            let analysis_manager = self.analysis_manager.clone();
            let output = self.output.clone();
            let status = self.status.clone();
            let queue = self.queue.clone();
            let queue_order = self.queue_order.clone();
            let visited = self.visited.clone();
            let skipped = self.skipped.clone();
            let initial_parsed_url = self.initial_parsed_url.clone();
            let final_user_agent = self.final_user_agent.clone();
            let accept_header = self.accept_header.clone();
            let initial_existing_url_found = self.initial_existing_url_found.clone();
            let terminated = self.terminated.clone();
            let done_urls_count = self.done_urls_count.clone();
            let non200_basenames = self.non200_basenames_to_occurrences.clone();
            let robots_txt_cache = self.robots_txt_cache.clone();
            let loaded_robots_txt_count = self.loaded_robots_txt_count.clone();
            let resolve_cache = self.resolve_cache.clone();
            let last_request_time = self.last_request_time.clone();
            let optimal_delay = self.optimal_delay_between_requests;
            let compiled_include_regex = self.compiled_include_regex.clone();
            let compiled_ignore_regex = self.compiled_ignore_regex.clone();

            let handle = tokio::spawn(async move {
                let _permit = permit; // Hold permit until task completes

                if terminated.load(Ordering::SeqCst) {
                    return;
                }

                Self::process_url(
                    entry,
                    &options,
                    &http_client,
                    &content_processor_manager,
                    &analysis_manager,
                    &output,
                    &status,
                    &queue,
                    &queue_order,
                    &visited,
                    &skipped,
                    &initial_parsed_url,
                    &final_user_agent,
                    &accept_header,
                    &initial_existing_url_found,
                    &terminated,
                    &done_urls_count,
                    &non200_basenames,
                    &robots_txt_cache,
                    &loaded_robots_txt_count,
                    &resolve_cache,
                    &last_request_time,
                    optimal_delay,
                    &compiled_include_regex,
                    &compiled_ignore_regex,
                )
                .await;
            });

            join_handles.push(handle);

            // Clean up finished handles periodically
            if join_handles.len() > 100 {
                let mut remaining = Vec::new();
                for h in join_handles {
                    if !h.is_finished() {
                        remaining.push(h);
                    }
                }
                join_handles = remaining;
            }
        }

        // Wait for all in-flight workers to complete
        for handle in join_handles {
            let _ = handle.await;
        }

        ctrl_c_handler.abort();

        Ok(())
    }

    /// Take the next URL from the queue (breadth-first order)
    fn take_next_from_queue(&self) -> Option<QueueEntry> {
        let mut order = self.queue_order.lock().unwrap_or_else(|e| e.into_inner());
        while !order.is_empty() {
            let Some(key) = order.pop_front() else { break };
            if let Some((_, entry)) = self.queue.remove(&key) {
                // Add to visited table
                self.visited.insert(
                    key.clone(),
                    VisitedEntry {
                        url: entry.url.clone(),
                        uq_id: entry.uq_id.clone(),
                        source_uq_id: entry.source_uq_id.clone(),
                        source_attr: entry.source_attr,
                    },
                );
                return Some(entry);
            }
        }
        None
    }

    /// Process a single URL: fetch, parse content, extract URLs, update status
    #[allow(clippy::too_many_arguments)]
    async fn process_url(
        entry: QueueEntry,
        options: &Arc<CoreOptions>,
        http_client: &Arc<HttpClient>,
        content_processor_manager: &Arc<Mutex<ContentProcessorManager>>,
        analysis_manager: &Arc<Mutex<AnalysisManager>>,
        output: &Arc<Mutex<Box<dyn Output>>>,
        status: &Arc<Mutex<Status>>,
        queue: &Arc<DashMap<String, QueueEntry>>,
        queue_order: &Arc<Mutex<VecDeque<String>>>,
        visited: &Arc<DashMap<String, VisitedEntry>>,
        skipped: &Arc<DashMap<String, SkippedEntry>>,
        initial_parsed_url: &ParsedUrl,
        final_user_agent: &str,
        accept_header: &str,
        initial_existing_url_found: &Arc<AtomicBool>,
        terminated: &Arc<AtomicBool>,
        done_urls_count: &Arc<AtomicUsize>,
        non200_basenames: &Arc<DashMap<String, i64>>,
        robots_txt_cache: &Arc<DashMap<String, Option<RobotsTxt>>>,
        loaded_robots_txt_count: &Arc<AtomicI64>,
        resolve_cache: &Arc<DashMap<String, String>>,
        last_request_time: &Arc<Mutex<f64>>,
        optimal_delay: f64,
        compiled_include_regex: &Arc<Vec<Regex>>,
        compiled_ignore_regex: &Arc<Vec<Regex>>,
    ) {
        let parsed_url = ParsedUrl::parse(&entry.url, None);
        let parsed_url_uq_id = Self::compute_url_uq_id(&parsed_url);

        let is_asset_url = parsed_url
            .extension
            .as_ref()
            .map(|ext| !HTML_PAGES_EXTENSIONS.contains(&ext.to_lowercase().as_str()))
            .unwrap_or(false);

        let scheme = parsed_url
            .scheme
            .as_deref()
            .unwrap_or(initial_parsed_url.scheme.as_deref().unwrap_or("https"));

        let host_and_port =
            if parsed_url.host.is_none() || parsed_url.host.as_deref() == initial_parsed_url.host.as_deref() {
                let host = initial_parsed_url.host.as_deref().unwrap_or("");
                let port = initial_parsed_url.port.unwrap_or(443);
                if port != 80 && port != 443 {
                    format!("{}:{}", host, port)
                } else {
                    host.to_string()
                }
            } else {
                let host = parsed_url.host.as_deref().unwrap_or("");
                let port = parsed_url.port.unwrap_or(443);
                if port != 80 && port != 443 {
                    format!("{}:{}", host, port)
                } else {
                    host.to_string()
                }
            };

        let host = match &parsed_url.host {
            Some(h) => h.clone(),
            None => {
                if let Ok(mut out) = output.lock() {
                    out.add_error(&format!("Invalid/unsupported URL found: {}", entry.url));
                }
                return;
            }
        };

        let absolute_url = format!(
            "{}://{}{}{}",
            scheme,
            host_and_port,
            parsed_url.path,
            parsed_url.query.as_ref().map(|q| format!("?{}", q)).unwrap_or_default()
        );

        let final_url_for_client = if options.add_random_query_params {
            Self::add_random_query_params(&parsed_url.path)
        } else {
            format!(
                "{}{}",
                parsed_url.path,
                parsed_url.query.as_ref().map(|q| format!("?{}", q)).unwrap_or_default()
            )
        };

        // Get origin header from source URL
        let origin = if !entry.source_uq_id.is_empty() {
            match status.lock() {
                Ok(st) => st.get_origin_header_value_by_source_uq_id(&entry.source_uq_id),
                _ => None,
            }
        } else {
            None
        };

        let is_image = parsed_url.is_image();
        let set_origin = origin.is_some() && !is_image;

        // For security: only send HTTP auth to same 2nd-level domain
        let use_http_auth = initial_parsed_url
            .domain_2nd_level
            .as_ref()
            .map(|d2| parsed_url.domain_2nd_level.as_deref() == Some(d2.as_str()))
            .unwrap_or(parsed_url.host == initial_parsed_url.host);

        let url_basename = parsed_url.get_base_name();

        // Check non-200 basename protection
        let http_response = if let Some(ref basename) = url_basename {
            match non200_basenames.get(basename) {
                Some(count) => {
                    if *count > options.max_non200_responses_per_basename {
                        Some(HttpResponse::create_skipped(
                            final_url_for_client.clone(),
                            format!(
                                "URL with basename '{}' has more than {} non-200 responses ({}).",
                                basename, options.max_non200_responses_per_basename, *count
                            ),
                        ))
                    } else {
                        None
                    }
                }
                _ => None,
            }
        } else {
            None
        };

        let http_response = match http_response {
            Some(skipped) => skipped,
            None => {
                let port = parsed_url.port.unwrap_or(if scheme == "https" { 443 } else { 80 });

                // Apply URL transformations
                let (http_request_host, http_request_path) =
                    Self::apply_http_request_transformations(&host, &final_url_for_client, &options.transform_url);

                let forced_ip = resolve_cache
                    .get(&format!("{}:{}", http_request_host, port))
                    .map(|v| v.value().clone());

                // Rate limiting: skip delay for cached responses (no actual HTTP request needed)
                let origin_for_request = if set_origin { origin.as_deref() } else { None };
                if !http_client.is_url_cached(
                    &http_request_host,
                    port,
                    scheme,
                    &http_request_path,
                    "GET",
                    final_user_agent,
                    accept_header,
                    &options.accept_encoding,
                    origin_for_request,
                ) {
                    let sleep_duration = {
                        let now = Self::current_timestamp();
                        let mut last_time = last_request_time.lock().unwrap_or_else(|e| e.into_inner());
                        let elapsed = now - *last_time;
                        if elapsed < optimal_delay {
                            let sleep = optimal_delay - elapsed;
                            *last_time = now + sleep; // Reserve slot immediately to avoid TOCTOU race
                            sleep
                        } else {
                            *last_time = now;
                            0.0
                        }
                    };
                    if sleep_duration > 0.0 {
                        tokio::time::sleep(tokio::time::Duration::from_secs_f64(sleep_duration.max(0.001))).await;
                    }
                }

                match http_client
                    .request(
                        &http_request_host,
                        port,
                        scheme,
                        &http_request_path,
                        "GET",
                        options.timeout as u64,
                        final_user_agent,
                        accept_header,
                        &options.accept_encoding,
                        origin_for_request,
                        use_http_auth,
                        forced_ip.as_deref(),
                    )
                    .await
                {
                    Ok(resp) => resp,
                    Err(e) => {
                        if let Ok(mut out) = output.lock() {
                            out.add_error(&format!("HTTP request error for {}: {}", absolute_url, e));
                        }
                        return;
                    }
                }
            }
        };

        // When the crawler has been terminated, do not process response
        if terminated.load(Ordering::SeqCst) {
            return;
        }

        let response_status = http_response.status_code;
        let elapsed_time = http_response.exec_time;

        // Handle gzip-compressed sitemaps (.xml.gz): decompress body before processing
        let is_gzip_sitemap = parsed_url.path.to_lowercase().ends_with(".xml.gz");
        let (body, body_text) = if is_gzip_sitemap
            && let Some(ref raw_body) = http_response.body
            && !raw_body.is_empty()
        {
            use flate2::read::GzDecoder;
            let mut decoder = GzDecoder::new(&raw_body[..]);
            let mut decompressed = Vec::new();
            if std::io::Read::read_to_end(&mut decoder, &mut decompressed).is_ok() {
                let text = String::from_utf8_lossy(&decompressed).to_string();
                (Some(decompressed), Some(text))
            } else {
                (http_response.body.clone(), http_response.body_text())
            }
        } else {
            (http_response.body.clone(), http_response.body_text())
        };

        let body_size = if is_asset_url {
            http_response
                .get_header("content-length")
                .and_then(|v| v.parse::<i64>().ok())
                .unwrap_or_else(|| body.as_ref().map(|b| b.len() as i64).unwrap_or(0))
        } else {
            body.as_ref().map(|b| b.len() as i64).unwrap_or(0)
        };

        if response_status != 200 {
            Self::process_non200_url(&parsed_url, non200_basenames);
        }

        // Detect content type
        let content_type_header = http_response.get_header("content-type").cloned().unwrap_or_default();
        let is_html_body = content_type_header.to_lowercase().contains("text/html");
        let is_css_body = content_type_header.to_lowercase().contains("text/css");
        let is_js_body = content_type_header.to_lowercase().contains("application/javascript")
            || content_type_header.to_lowercase().contains("text/javascript");
        let is_xml_body = is_gzip_sitemap
            || content_type_header.to_lowercase().contains("application/xml")
            || content_type_header.to_lowercase().contains("text/xml");

        let is_allowed_for_crawling =
            Self::is_url_allowed_by_regexes(&parsed_url, options, compiled_include_regex, compiled_ignore_regex)
                && Self::is_external_domain_allowed_for_crawling(
                    parsed_url.host.as_deref().unwrap_or(""),
                    initial_parsed_url,
                    &options.allowed_domains_for_crawling,
                );

        let mut extra_parsed_content: HashMap<String, String> = HashMap::new();

        // Mark initial URL as found
        if !initial_existing_url_found.load(Ordering::SeqCst) && is_html_body && response_status == 200 && body_size > 0
        {
            initial_existing_url_found.store(true, Ordering::SeqCst);
        }

        // Get content type ID
        let has_location = http_response.get_header("location").is_some();
        let content_type = if has_location && response_status > 300 && response_status < 320 {
            ContentTypeId::Redirect
        } else if is_gzip_sitemap {
            ContentTypeId::Xml
        } else {
            Self::get_content_type_id_by_header(&content_type_header)
        };

        // Apply content changes before URL parsing (text-based, for HTML/CSS/JS)
        let mut body_for_parsing = body_text.clone().unwrap_or_default();
        if let Ok(mut cpm) = content_processor_manager.lock() {
            cpm.apply_content_changes_before_url_parsing(&mut body_for_parsing, content_type, &parsed_url);
        }

        // Parse body and fill queue with new URLs
        if !body_for_parsing.is_empty() && is_html_body && is_allowed_for_crawling {
            let html_extras = Self::parse_html_body_and_fill_queue(
                &body_for_parsing,
                content_type,
                &parsed_url,
                options,
                content_processor_manager,
                queue,
                queue_order,
                visited,
                skipped,
                initial_parsed_url,
                non200_basenames,
                robots_txt_cache,
                loaded_robots_txt_count,
                resolve_cache,
                http_client,
                output,
                status,
                terminated,
                compiled_include_regex,
                compiled_ignore_regex,
            );
            for (k, v) in html_extras {
                extra_parsed_content.insert(k, v);
            }
        } else if !body_for_parsing.is_empty() && (is_js_body || is_css_body || is_xml_body) {
            Self::parse_content_and_fill_url_queue(
                &body_for_parsing,
                content_type,
                &parsed_url,
                options,
                content_processor_manager,
                queue,
                queue_order,
                visited,
                skipped,
                initial_parsed_url,
                non200_basenames,
                robots_txt_cache,
                loaded_robots_txt_count,
                resolve_cache,
                http_client,
                output,
                status,
                terminated,
                compiled_include_regex,
                compiled_ignore_regex,
            );
        }

        // Handle redirect
        if (301..=308).contains(&response_status)
            && let Some(redirect_location) = http_response.get_header("location")
        {
            let redirect_location = redirect_location.clone();
            extra_parsed_content.insert("Location".to_string(), redirect_location.clone());
            Self::add_redirect_location_to_queue_if_suitable(
                &redirect_location,
                &parsed_url_uq_id,
                scheme,
                &host_and_port,
                &parsed_url,
                options,
                queue,
                queue_order,
                visited,
                skipped,
                initial_parsed_url,
                terminated,
                compiled_include_regex,
                compiled_ignore_regex,
            );
        }

        // Set extras from headers
        for extra_column in &options.extra_columns {
            let col_name_lower = extra_column.name.to_lowercase();
            if let Some(header_val) = http_response.get_header(&col_name_lower) {
                extra_parsed_content.insert(extra_column.name.clone(), header_val.clone());
            }
        }

        // Caching
        let (cache_type_flags, cache_lifetime) = if http_response.status_code > 0 {
            (
                Self::get_cache_type_flags(&http_response.headers),
                Self::get_cache_lifetime(&http_response.headers),
            )
        } else {
            (crate::result::visited_url::CACHE_TYPE_NOT_AVAILABLE, None)
        };

        // Create VisitedUrl and update status
        let is_external = parsed_url
            .host
            .as_deref()
            .map(|h| !Self::hosts_are_www_equivalent(h, initial_parsed_url.host.as_deref().unwrap_or("")))
            .unwrap_or(false);

        let visited_url = VisitedUrl::new(
            parsed_url_uq_id.clone(),
            entry.source_uq_id.clone(),
            entry.source_attr,
            absolute_url.clone(),
            response_status,
            elapsed_time,
            Some(body_size),
            content_type,
            Some(content_type_header.clone()),
            http_response.get_header("content-encoding").cloned(),
            if extra_parsed_content.is_empty() {
                None
            } else {
                Some(extra_parsed_content.clone())
            },
            is_external,
            is_allowed_for_crawling,
            cache_type_flags,
            cache_lifetime.map(|l| l as i64),
        );

        if let Ok(mut st) = status.lock() {
            st.add_visited_url(visited_url.clone(), body.as_deref(), Some(&http_response.headers));
        }

        // Run per-URL analysis (headers, security, accessibility, best practices, etc.)
        if let Ok(mut am) = analysis_manager.lock()
            && let Ok(st) = status.lock()
        {
            let analysis_results =
                am.analyze_visited_url(&visited_url, body_text.as_deref(), Some(&http_response.headers), &st);

            // Store analysis results as extra columns for progress table display
            let extra_column_values = am.get_analysis_column_values(&analysis_results);
            for (col_name, col_value) in extra_column_values {
                extra_parsed_content.insert(col_name, col_value);
            }
        }

        // Increment done count
        let done_count = done_urls_count.fetch_add(1, Ordering::SeqCst) + 1;
        let total_count = queue.len() + visited.len();
        let progress_status = format!("{}/{}", done_count, total_count);

        // Print table row to output
        if let Ok(mut out) = output.lock() {
            out.add_table_row(
                &http_response.headers,
                &absolute_url,
                response_status,
                elapsed_time,
                body_size,
                content_type as i32,
                &extra_parsed_content,
                &progress_status,
                cache_type_flags as i32,
                cache_lifetime,
            );
        }
    }

    /// Parse HTML body, extract URLs, and fill the queue
    #[allow(clippy::too_many_arguments)]
    fn parse_html_body_and_fill_queue(
        body: &str,
        content_type: ContentTypeId,
        url: &ParsedUrl,
        options: &Arc<CoreOptions>,
        content_processor_manager: &Arc<Mutex<ContentProcessorManager>>,
        queue: &Arc<DashMap<String, QueueEntry>>,
        queue_order: &Arc<Mutex<VecDeque<String>>>,
        visited: &Arc<DashMap<String, VisitedEntry>>,
        skipped: &Arc<DashMap<String, SkippedEntry>>,
        initial_parsed_url: &ParsedUrl,
        non200_basenames: &Arc<DashMap<String, i64>>,
        robots_txt_cache: &Arc<DashMap<String, Option<RobotsTxt>>>,
        loaded_robots_txt_count: &Arc<AtomicI64>,
        resolve_cache: &Arc<DashMap<String, String>>,
        http_client: &Arc<HttpClient>,
        output: &Arc<Mutex<Box<dyn Output>>>,
        status: &Arc<Mutex<Status>>,
        terminated: &Arc<AtomicBool>,
        compiled_include_regex: &[Regex],
        compiled_ignore_regex: &[Regex],
    ) -> HashMap<String, String> {
        let mut result = HashMap::new();

        // Skip link following from HTML pages when initial URL is a sitemap.xml
        // (sitemap-only mode: only crawl URLs listed in the sitemap)
        let is_sitemap_only = Self::is_sitemap_url(initial_parsed_url);
        if !is_sitemap_only || content_type == ContentTypeId::Xml {
            Self::parse_content_and_fill_url_queue(
                body,
                content_type,
                url,
                options,
                content_processor_manager,
                queue,
                queue_order,
                visited,
                skipped,
                initial_parsed_url,
                non200_basenames,
                robots_txt_cache,
                loaded_robots_txt_count,
                resolve_cache,
                http_client,
                output,
                status,
                terminated,
                compiled_include_regex,
                compiled_ignore_regex,
            );
        }

        // Extract Title
        if let Some(caps) = RE_TITLE.captures(body) {
            let title = caps.get(1).map_or("", |m| m.as_str()).trim();
            result.insert("Title".to_string(), Self::decode_html_entities(title));
        }

        // Extract Description
        if let Some(caps) = RE_DESCRIPTION.captures(body) {
            let desc = caps.get(1).map_or("", |m| m.as_str()).trim();
            result.insert("Description".to_string(), Self::decode_html_entities(desc));
        }

        // Extract Keywords if needed
        if options.has_header_to_table("Keywords")
            && let Some(caps) = RE_KEYWORDS.captures(body)
        {
            let keywords = caps.get(1).map_or("", |m| m.as_str()).trim();
            result.insert("Keywords".to_string(), Self::decode_html_entities(keywords));
        }

        // Extract DOM count if needed
        if options.has_header_to_table("DOM") {
            let dom_count = RE_DOM_COUNT.find_iter(body).count();
            result.insert("DOM".to_string(), dom_count.to_string());
        }

        // Custom extraction for extra columns
        for extra_column in &options.extra_columns {
            if extra_column.custom_method.is_some()
                && let Some(value) = extra_column.extract_value(body)
            {
                result.insert(extra_column.name.clone(), value);
            }
        }

        result
    }

    /// Parse content (HTML/CSS/JS/XML) and fill URL queue
    #[allow(clippy::too_many_arguments)]
    fn parse_content_and_fill_url_queue(
        content: &str,
        content_type: ContentTypeId,
        url: &ParsedUrl,
        options: &Arc<CoreOptions>,
        content_processor_manager: &Arc<Mutex<ContentProcessorManager>>,
        queue: &Arc<DashMap<String, QueueEntry>>,
        queue_order: &Arc<Mutex<VecDeque<String>>>,
        visited: &Arc<DashMap<String, VisitedEntry>>,
        skipped: &Arc<DashMap<String, SkippedEntry>>,
        initial_parsed_url: &ParsedUrl,
        non200_basenames: &Arc<DashMap<String, i64>>,
        robots_txt_cache: &Arc<DashMap<String, Option<RobotsTxt>>>,
        loaded_robots_txt_count: &Arc<AtomicI64>,
        resolve_cache: &Arc<DashMap<String, String>>,
        http_client: &Arc<HttpClient>,
        output: &Arc<Mutex<Box<dyn Output>>>,
        status: &Arc<Mutex<Status>>,
        terminated: &Arc<AtomicBool>,
        compiled_include_regex: &[Regex],
        compiled_ignore_regex: &[Regex],
    ) {
        // Detect <base href="..."> in HTML content to use as base URL for resolving relative URLs
        let effective_base_url = if content_type == ContentTypeId::Html {
            if let Some(caps) = RE_BASE_HREF.captures(content) {
                if let Some(base_href) = caps.get(1) {
                    let base_href_str = base_href.as_str();
                    // Only use base href if it looks like a valid URL or path
                    if base_href_str.starts_with("http://")
                        || base_href_str.starts_with("https://")
                        || base_href_str.starts_with("//")
                        || base_href_str.starts_with('/')
                    {
                        Some(ParsedUrl::parse(base_href_str, Some(url)))
                    } else {
                        None
                    }
                } else {
                    None
                }
            } else {
                None
            }
        } else {
            None
        };
        let source_url = effective_base_url.as_ref().unwrap_or(url);

        let found_urls_list = match content_processor_manager.lock() {
            Ok(mut cpm) => cpm.find_urls(content, content_type, source_url),
            _ => Vec::new(),
        };

        for found_urls in found_urls_list {
            Self::add_suitable_urls_to_queue(
                &found_urls,
                source_url,
                options,
                queue,
                queue_order,
                visited,
                skipped,
                initial_parsed_url,
                non200_basenames,
                robots_txt_cache,
                loaded_robots_txt_count,
                resolve_cache,
                http_client,
                output,
                status,
                terminated,
                compiled_include_regex,
                compiled_ignore_regex,
            );
        }
    }

    /// Add suitable found URLs to the queue after filtering
    #[allow(clippy::too_many_arguments)]
    fn add_suitable_urls_to_queue(
        found_urls: &FoundUrls,
        source_url: &ParsedUrl,
        options: &Arc<CoreOptions>,
        queue: &Arc<DashMap<String, QueueEntry>>,
        queue_order: &Arc<Mutex<VecDeque<String>>>,
        visited: &Arc<DashMap<String, VisitedEntry>>,
        skipped: &Arc<DashMap<String, SkippedEntry>>,
        initial_parsed_url: &ParsedUrl,
        non200_basenames: &Arc<DashMap<String, i64>>,
        robots_txt_cache: &Arc<DashMap<String, Option<RobotsTxt>>>,
        _loaded_robots_txt_count: &Arc<AtomicI64>,
        _resolve_cache: &Arc<DashMap<String, String>>,
        _http_client: &Arc<HttpClient>,
        _output: &Arc<Mutex<Box<dyn Output>>>,
        _status: &Arc<Mutex<Status>>,
        terminated: &Arc<AtomicBool>,
        compiled_include_regex: &[Regex],
        compiled_ignore_regex: &[Regex],
    ) {
        let source_url_uq_id = Self::compute_url_uq_id(source_url);

        for found_url in found_urls.get_urls().values() {
            if terminated.load(Ordering::SeqCst) {
                return;
            }

            let url_for_queue = found_url.url.trim().to_string();
            let parsed_url_for_queue = ParsedUrl::parse(&url_for_queue, Some(source_url));

            // Skip URLs that are not requestable resources
            if !utils::is_href_for_requestable_resource(&url_for_queue) {
                continue;
            }

            // Check if URL is on same host or allowed host
            let is_url_on_same_host = parsed_url_for_queue.host.is_none()
                || parsed_url_for_queue.host == initial_parsed_url.host
                || Self::hosts_are_www_equivalent(
                    parsed_url_for_queue.host.as_deref().unwrap_or(""),
                    initial_parsed_url.host.as_deref().unwrap_or(""),
                );
            let mut is_url_on_allowed_host = false;
            if let Some(ref parsed_host) = parsed_url_for_queue.host
                && Some(parsed_host.as_str()) != initial_parsed_url.host.as_deref()
            {
                let is_allowed_static = !options.allowed_domains_for_external_files.is_empty()
                    && Self::is_domain_allowed_for_static_files(
                        parsed_host,
                        &options.allowed_domains_for_external_files,
                    );
                let is_allowed_crawlable = !options.allowed_domains_for_crawling.is_empty()
                    && Self::is_external_domain_allowed_for_crawling(
                        parsed_host,
                        initial_parsed_url,
                        &options.allowed_domains_for_crawling,
                    );
                if (is_allowed_static && found_url.is_included_asset()) || is_allowed_crawlable {
                    is_url_on_allowed_host = true;
                }
            }

            // Skip basename with too many non-200s
            if let Some(ref basename) = parsed_url_for_queue.get_base_name()
                && let Some(count) = non200_basenames.get(basename)
                && *count >= options.max_non200_responses_per_basename
            {
                continue;
            }

            if !is_url_on_same_host && !is_url_on_allowed_host {
                // Add to skipped
                let url_key = Self::compute_url_key(&parsed_url_for_queue);
                if !skipped.contains_key(&url_key) {
                    skipped.insert(
                        url_key,
                        SkippedEntry {
                            url: parsed_url_for_queue.get_full_url(true, false),
                            reason: SkippedReason::NotAllowedHost,
                            source_uq_id: source_url_uq_id.clone(),
                            source_attr: found_url.source as i32,
                        },
                    );
                }
                continue;
            }

            // Check robots.txt (skip for static files)
            if !parsed_url_for_queue.is_static_file() && !options.ignore_robots_txt {
                let check_host = parsed_url_for_queue
                    .host
                    .as_deref()
                    .unwrap_or(initial_parsed_url.host.as_deref().unwrap_or(""));
                if !Self::is_url_allowed_by_robots_txt_cached(check_host, &url_for_queue, robots_txt_cache) {
                    let url_key = Self::compute_url_key(&parsed_url_for_queue);
                    if !skipped.contains_key(&url_key) {
                        skipped.insert(
                            url_key,
                            SkippedEntry {
                                url: parsed_url_for_queue.get_full_url(true, false),
                                reason: SkippedReason::RobotsTxt,
                                source_uq_id: source_url_uq_id.clone(),
                                source_attr: found_url.source as i32,
                            },
                        );
                    }
                    continue;
                }
            }

            // Build absolute URL
            let source_full_url = source_url.get_full_url(true, false);
            let absolute_url = utils::get_absolute_url_by_base_url(&source_full_url, &url_for_queue);

            if absolute_url.is_empty() {
                continue;
            }

            // Remove fragment
            let absolute_url = if let Some(hash_pos) = absolute_url.find('#') {
                absolute_url[..hash_pos].to_string()
            } else {
                absolute_url
            };

            // Filter query params if configured
            let absolute_url = if options.remove_query_params {
                if let Some(q_pos) = absolute_url.find('?') {
                    absolute_url[..q_pos].to_string()
                } else {
                    absolute_url
                }
            } else if !options.keep_query_params.is_empty() {
                filter_query_params(&absolute_url, &options.keep_query_params)
            } else {
                absolute_url
            };

            // Re-parse and check suitability
            let mut parsed_url_for_queue = ParsedUrl::parse(&absolute_url, Some(source_url));

            // Force relative URLs: normalize host/scheme variants to match initial URL
            if options.force_relative_urls {
                Self::normalize_url_to_initial(&mut parsed_url_for_queue, initial_parsed_url);
            }

            let suitable = Self::is_url_suitable_for_queue_static(
                &parsed_url_for_queue,
                queue,
                visited,
                options,
                compiled_include_regex,
                compiled_ignore_regex,
            );
            if suitable {
                Self::add_url_to_queue_static(
                    &parsed_url_for_queue,
                    Some(&source_url_uq_id),
                    found_url.source as i32,
                    queue,
                    queue_order,
                    visited,
                    options,
                    terminated,
                );
            }
        }
    }

    /// Add URL to the queue
    fn add_url_to_queue(&self, url: &ParsedUrl, source_uq_id: Option<&str>, source_attr: i32) {
        Self::add_url_to_queue_static(
            url,
            source_uq_id,
            source_attr,
            &self.queue,
            &self.queue_order,
            &self.visited,
            &self.options,
            &self.terminated,
        );
    }

    /// Static version of add_url_to_queue for use in async contexts
    #[allow(clippy::too_many_arguments)]
    fn add_url_to_queue_static(
        url: &ParsedUrl,
        source_uq_id: Option<&str>,
        source_attr: i32,
        queue: &DashMap<String, QueueEntry>,
        queue_order: &Mutex<VecDeque<String>>,
        visited: &DashMap<String, VisitedEntry>,
        options: &CoreOptions,
        terminated: &AtomicBool,
    ) {
        if terminated.load(Ordering::SeqCst) {
            return;
        }

        // Check max_visited_urls limit
        if (queue.len() + visited.len()) as i64 >= options.max_visited_urls {
            return;
        }

        let url_str = url.get_full_url(true, false);
        let url_key = Self::compute_url_key(url);
        let uq_id = Self::compute_url_uq_id(url);

        if (queue.len() as i64) >= options.max_queue_length {
            return;
        }

        let entry = QueueEntry {
            url: url_str,
            uq_id,
            source_uq_id: source_uq_id.unwrap_or("").to_string(),
            source_attr,
        };

        queue.insert(url_key.clone(), entry);
        if let Ok(mut order) = queue_order.lock() {
            order.push_back(url_key);
        }
    }

    /// Normalize URL host/scheme to match the initial URL when force_relative_urls is enabled.
    /// Handles www/non-www and http/https variants of the same domain.
    fn normalize_url_to_initial(url: &mut ParsedUrl, initial_url: &ParsedUrl) {
        if let (Some(url_host), Some(initial_host)) = (url.host.as_ref(), initial_url.host.as_ref()) {
            let url_host_no_www = url_host.strip_prefix("www.").unwrap_or(url_host);
            let initial_host_no_www = initial_host.strip_prefix("www.").unwrap_or(initial_host);

            if url_host_no_www.eq_ignore_ascii_case(initial_host_no_www) {
                // Normalize host to match initial URL
                if url.host.as_deref() != initial_url.host.as_deref() {
                    url.host = initial_url.host.clone();
                }
                // Normalize scheme to match initial URL
                if url.scheme != initial_url.scheme {
                    url.scheme = initial_url.scheme.clone();
                }
                // Rebuild the url string
                url.url = url.get_full_url(true, true);
            }
        }
    }

    /// Check if a URL is suitable for the queue
    fn is_url_suitable_for_queue_static(
        url: &ParsedUrl,
        queue: &DashMap<String, QueueEntry>,
        visited: &DashMap<String, VisitedEntry>,
        options: &CoreOptions,
        compiled_include: &[Regex],
        compiled_ignore: &[Regex],
    ) -> bool {
        if !Self::is_url_allowed_by_regexes(url, options, compiled_include, compiled_ignore) {
            return false;
        }

        if (visited.len() + queue.len()) as i64 >= options.max_visited_urls {
            return false;
        }

        let full_url = url.get_full_url(true, false);
        let url_key = Self::compute_url_key(url);

        let is_in_queue = queue.contains_key(&url_key);
        let is_already_visited = visited.contains_key(&url_key);
        let is_url_with_html = url.extension.is_none()
            || HTML_PAGES_EXTENSIONS.contains(&url.extension.as_deref().unwrap_or("").to_lowercase().as_str());
        let path_lower = url.path.to_lowercase();
        let is_url_with_sitemap =
            path_lower.contains("sitemap") && (path_lower.ends_with(".xml") || path_lower.ends_with(".xml.gz"));
        let is_url_too_long = full_url.len() as i64 > options.max_url_length;
        let allowed_only_html = options.crawl_only_html_files();

        if !is_in_queue
            && !is_already_visited
            && !is_url_too_long
            && (is_url_with_html || !allowed_only_html || is_url_with_sitemap)
        {
            return true;
        }

        false
    }

    /// Check if URL is allowed by include/ignore regex rules
    fn is_url_allowed_by_regexes(
        url: &ParsedUrl,
        options: &CoreOptions,
        compiled_include: &[Regex],
        compiled_ignore: &[Regex],
    ) -> bool {
        // Bypass regex filtering for static files if configured
        if options.regex_filtering_only_for_pages && url.is_static_file() {
            return true;
        }

        let full_url = url.get_full_url(true, false);

        let mut is_allowed = compiled_include.is_empty();
        for re in compiled_include {
            if re.is_match(&full_url) {
                is_allowed = true;
                break;
            }
        }

        for re in compiled_ignore {
            if re.is_match(&full_url) {
                is_allowed = false;
                break;
            }
        }

        is_allowed
    }

    /// Check if a domain is allowed for static file downloads
    fn is_domain_allowed_for_static_files(domain: &str, allowed_domains: &[String]) -> bool {
        use std::sync::OnceLock;
        static COMPILED: OnceLock<Vec<Regex>> = OnceLock::new();
        let patterns = COMPILED.get_or_init(|| compile_domain_patterns(allowed_domains));
        patterns.iter().any(|re| re.is_match(domain))
    }

    /// Check if two hosts are www/non-www equivalents
    fn hosts_are_www_equivalent(host_a: &str, host_b: &str) -> bool {
        if host_a == host_b {
            return true;
        }
        let a = host_a.strip_prefix("www.").unwrap_or(host_a);
        let b = host_b.strip_prefix("www.").unwrap_or(host_b);
        a == b
    }

    /// Check if an external domain is allowed for whole-domain crawling
    fn is_external_domain_allowed_for_crawling(
        domain: &str,
        initial_parsed_url: &ParsedUrl,
        allowed_domains: &[String],
    ) -> bool {
        let initial_host = initial_parsed_url.host.as_deref().unwrap_or("");
        if domain == initial_host {
            return true;
        }

        // www/non-www equivalence: handles redirects like
        // www.rust-lang.org -> rust-lang.org (or vice versa)
        if Self::hosts_are_www_equivalent(domain, initial_host) {
            return true;
        }

        use std::sync::OnceLock;
        static COMPILED: OnceLock<Vec<Regex>> = OnceLock::new();
        let patterns = COMPILED.get_or_init(|| compile_domain_patterns(allowed_domains));
        patterns.iter().any(|re| re.is_match(domain))
    }

    /// Add redirect location to queue if suitable
    #[allow(clippy::too_many_arguments)]
    fn add_redirect_location_to_queue_if_suitable(
        redirect_location: &str,
        source_uq_id: &str,
        scheme: &str,
        host_and_port: &str,
        source_url: &ParsedUrl,
        options: &Arc<CoreOptions>,
        queue: &Arc<DashMap<String, QueueEntry>>,
        queue_order: &Arc<Mutex<VecDeque<String>>>,
        visited: &Arc<DashMap<String, VisitedEntry>>,
        _skipped: &Arc<DashMap<String, SkippedEntry>>,
        _initial_parsed_url: &ParsedUrl,
        terminated: &Arc<AtomicBool>,
        compiled_include_regex: &[Regex],
        compiled_ignore_regex: &[Regex],
    ) {
        let redirect_url = if redirect_location.starts_with("//") {
            format!("{}:{}", scheme, redirect_location)
        } else if redirect_location.starts_with('/') {
            format!("{}://{}{}", scheme, host_and_port, redirect_location)
        } else if redirect_location.starts_with("http://") || redirect_location.starts_with("https://") {
            redirect_location.to_string()
        } else {
            format!(
                "{}://{}{}/{}",
                scheme, host_and_port, source_url.path, redirect_location
            )
        };

        let parsed_redirect_url = ParsedUrl::parse(&redirect_url, Some(source_url));

        if Self::is_url_suitable_for_queue_static(
            &parsed_redirect_url,
            queue,
            visited,
            options,
            compiled_include_regex,
            compiled_ignore_regex,
        ) {
            Self::add_url_to_queue_static(
                &parsed_redirect_url,
                Some(source_uq_id),
                UrlSource::Redirect as i32,
                queue,
                queue_order,
                visited,
                options,
                terminated,
            );

            // If initial URL redirects to same 2nd-level domain, the domain checks
            // in is_external_domain_allowed_for_crawling and is_url_on_same_host
            // handle this via 2nd-level domain comparison.
        }
    }

    /// Process a URL that returned non-200 status
    fn process_non200_url(url: &ParsedUrl, non200_basenames: &DashMap<String, i64>) {
        if let Some(basename) = url.get_base_name()
            && basename != "index.html"
            && basename != "index.htm"
            && basename != "index"
        {
            non200_basenames
                .entry(basename)
                .and_modify(|count| *count += 1)
                .or_insert(1);
        }
    }

    /// Check if URL is allowed by robots.txt (using cache)
    fn is_url_allowed_by_robots_txt_cached(
        domain: &str,
        url: &str,
        robots_txt_cache: &DashMap<String, Option<RobotsTxt>>,
    ) -> bool {
        // Only check the matching domain's robots.txt
        for entry in robots_txt_cache.iter() {
            if !entry.key().starts_with(domain) {
                continue;
            }
            if let Some(ref robots_txt) = *entry.value()
                && !robots_txt.is_allowed(url)
            {
                return false;
            }
        }
        true
    }

    /// Fetch and parse robots.txt for a domain
    pub async fn fetch_robots_txt(&self, domain: &str, port: u16, scheme: &str) {
        if self.options.ignore_robots_txt {
            return;
        }

        let cache_key = format!("{}:{}", domain, port);
        if self.robots_txt_cache.contains_key(&cache_key) {
            return;
        }

        // Prevent parallel fetches for same domain
        self.robots_txt_cache.insert(cache_key.clone(), None);

        let use_http_auth = self
            .initial_parsed_url
            .domain_2nd_level
            .as_ref()
            .map(|d2| domain.ends_with(d2.as_str()))
            .unwrap_or(domain == self.initial_parsed_url.host.as_deref().unwrap_or(""));

        let (http_request_host, http_request_path) =
            Self::apply_http_request_transformations(domain, "/robots.txt", &self.options.transform_url);

        let forced_ip = self
            .resolve_cache
            .get(&format!("{}:{}", http_request_host, port))
            .map(|v| v.value().clone());

        let response = self
            .http_client
            .request(
                &http_request_host,
                port,
                scheme,
                &http_request_path,
                "GET",
                3,
                &Self::get_crawler_user_agent_signature(),
                ACCEPT_HEADER,
                "gzip, deflate, br",
                None,
                use_http_auth,
                forced_ip.as_deref(),
            )
            .await;

        let count = self.loaded_robots_txt_count.fetch_add(1, Ordering::SeqCst) + 1;

        if let Ok(resp) = response {
            if count <= 10
                && let Ok(st) = self.status.lock()
            {
                st.add_notice_to_summary(
                    &format!("robots-txt-{}", domain),
                    &format!(
                        "Loaded robots.txt for domain '{}': status code {}, size {} and took {}.",
                        domain,
                        resp.status_code,
                        resp.get_formatted_body_length(),
                        resp.get_formatted_exec_time(),
                    ),
                );
            }

            if resp.status_code == 200
                && let Some(ref body_bytes) = resp.body
            {
                let body_str = String::from_utf8_lossy(body_bytes);
                let robots_txt = RobotsTxt::parse(&body_str);

                if let Ok(st) = self.status.lock() {
                    st.set_robots_txt_content(scheme, domain, port, &body_str);
                }

                self.robots_txt_cache.insert(cache_key, Some(robots_txt));
                return;
            }
        }

        // No valid robots.txt found
        self.robots_txt_cache.insert(cache_key, None);
    }

    /// Get content type ID from Content-Type header
    fn get_content_type_id_by_header(content_type_header: &str) -> ContentTypeId {
        let header_lower = content_type_header.to_lowercase();

        if header_lower.contains("text/html") {
            ContentTypeId::Html
        } else if header_lower.contains("text/javascript")
            || header_lower.contains("application/javascript")
            || header_lower.contains("application/x-javascript")
        {
            ContentTypeId::Script
        } else if header_lower.contains("text/css") {
            ContentTypeId::Stylesheet
        } else if header_lower.contains("image/") {
            ContentTypeId::Image
        } else if header_lower.contains("audio/") {
            ContentTypeId::Audio
        } else if header_lower.contains("video/") {
            ContentTypeId::Video
        } else if header_lower.contains("font/") {
            ContentTypeId::Font
        } else if header_lower.contains("application/json") {
            ContentTypeId::Json
        } else if header_lower.contains("application/xml")
            || header_lower.contains("text/xml")
            || header_lower.contains("+xml")
        {
            ContentTypeId::Xml
        } else if header_lower.contains("application/pdf")
            || header_lower.contains("application/msword")
            || header_lower.contains("application/vnd.ms-excel")
            || header_lower.contains("application/vnd.ms-powerpoint")
            || header_lower.contains("text/plain")
            || header_lower.contains("document")
        {
            ContentTypeId::Document
        } else {
            ContentTypeId::Other
        }
    }

    /// Build final user agent string
    fn build_final_user_agent(options: &CoreOptions) -> String {
        let base = if let Some(ref ua) = options.user_agent {
            ua.clone()
        } else {
            match options.device {
                DeviceType::Desktop => format!(
                    "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/{}.0.0.0 Safari/537.36",
                    chrono::Utc::now().format("%y")
                ),
                DeviceType::Mobile => "Mozilla/5.0 (iPhone; CPU iPhone OS 15_0 like Mac OS X) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/15.0 Mobile/15A5370a Safari/604.1".to_string(),
                DeviceType::Tablet => "Mozilla/5.0 (Linux; Android 11; SAMSUNG SM-T875) AppleWebKit/537.36 (KHTML, like Gecko) SamsungBrowser/14.0 Chrome/87.0.4280.141 Safari/537.36".to_string(),
            }
        };

        // Add signature unless user agent ends with '!'
        if base.ends_with('!') {
            base.trim_end_matches('!').trim_end().to_string()
        } else {
            format!("{} {}", base, Self::get_crawler_user_agent_signature())
        }
    }

    /// Get crawler user agent signature
    pub fn get_crawler_user_agent_signature() -> String {
        format!("siteone-crawler/{}", version::CODE)
    }

    /// Compute MD5-based key for URL deduplication
    fn compute_url_key(url: &ParsedUrl) -> String {
        let relevant_parts = url.get_full_url(true, false);
        let mut hasher = Md5::new();
        hasher.update(relevant_parts.as_bytes());
        format!("{:x}", hasher.finalize())
    }

    /// Check if URL points to a sitemap.xml or sitemap.xml.gz file
    fn is_sitemap_url(url: &ParsedUrl) -> bool {
        let path_lower = url.path.to_lowercase();
        path_lower.contains("sitemap") && (path_lower.ends_with(".xml") || path_lower.ends_with(".xml.gz"))
    }

    /// Compute short unique ID for a URL (first 8 chars of MD5)
    fn compute_url_uq_id(url: &ParsedUrl) -> String {
        let full_url = url.get_full_url(true, false);
        let mut hasher = Md5::new();
        hasher.update(full_url.as_bytes());
        let hash = format!("{:x}", hasher.finalize());
        hash[..8].to_string()
    }

    /// Decode HTML entities
    fn decode_html_entities(text: &str) -> String {
        text.replace("&amp;", "&")
            .replace("&lt;", "<")
            .replace("&gt;", ">")
            .replace("&quot;", "\"")
            .replace("&#39;", "'")
            .replace("&ndash;", "\u{2013}")
            .replace("&mdash;", "\u{2014}")
    }

    /// Get current timestamp in seconds
    fn current_timestamp() -> f64 {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs_f64()
    }

    /// Add random query parameters to a URL path
    fn add_random_query_params(path: &str) -> String {
        let random_val = rand_simple();
        if path.contains('?') {
            format!("{}&_soc={}", path, random_val)
        } else {
            format!("{}?_soc={}", path, random_val)
        }
    }

    /// Apply URL transformations for HTTP request (--transform-url)
    fn apply_http_request_transformations(host: &str, path: &str, transform_url: &[String]) -> (String, String) {
        if transform_url.is_empty() {
            return (host.to_string(), path.to_string());
        }

        let mut full_url = format!("{}{}", host, path);
        let original_url = full_url.clone();

        for transform in transform_url {
            let parts: Vec<&str> = transform.splitn(2, "->").collect();
            if parts.len() != 2 {
                continue;
            }

            let from = parts[0].trim();
            let to = parts[1].trim();

            // Check if it's a regex pattern
            let is_regex = utils::is_regex_pattern(from);

            if is_regex {
                if let Ok(re) = Regex::new(from) {
                    full_url = re.replace_all(&full_url, to).to_string();
                }
            } else {
                full_url = full_url.replace(from, to);
            }
        }

        if full_url != original_url {
            // Parse transformed URL back to host and path
            if let Ok(parsed) = url::Url::parse(&format!("http://{}", full_url)) {
                let new_host = parsed.host_str().unwrap_or(host).to_string();
                let new_path = if let Some(query) = parsed.query() {
                    format!("{}?{}", parsed.path(), query)
                } else {
                    parsed.path().to_string()
                };
                return (new_host, new_path);
            }
        }

        (host.to_string(), path.to_string())
    }

    /// Remove AVIF and WebP support from Accept header (for offline export)
    pub fn remove_avif_and_webp_support_from_accept_header(&mut self) {
        self.accept_header = self.accept_header.replace("image/avif,", "").replace("image/webp,", "");
    }

    /// Terminate the crawler
    pub fn terminate(&self) {
        self.terminated.store(true, Ordering::SeqCst);
    }

    /// Get forced IP for domain and port from --resolve options
    pub fn get_forced_ip_for_domain_and_port(&self, domain: &str, port: u16) -> Option<String> {
        self.resolve_cache
            .get(&format!("{}:{}", domain, port))
            .map(|v| v.value().clone())
    }

    /// Get cache type flags from response headers
    fn get_cache_type_flags(headers: &HashMap<String, String>) -> u32 {
        use crate::result::visited_url::*;

        let mut flags: u32 = 0;

        if let Some(cache_control) = headers.get("cache-control") {
            flags |= CACHE_TYPE_HAS_CACHE_CONTROL;
            let cc_lower = cache_control.to_lowercase();
            if cc_lower.contains("max-age") {
                flags |= CACHE_TYPE_HAS_MAX_AGE;
            }
            if cc_lower.contains("s-maxage") || cc_lower.contains("s-max-age") {
                flags |= CACHE_TYPE_HAS_S_MAX_AGE;
            }
            if cc_lower.contains("stale-while-revalidate") {
                flags |= CACHE_TYPE_HAS_STALE_WHILE_REVALIDATE;
            }
            if cc_lower.contains("stale-if-error") {
                flags |= CACHE_TYPE_HAS_STALE_IF_ERROR;
            }
            if cc_lower.contains("public") {
                flags |= CACHE_TYPE_HAS_PUBLIC;
            }
            if cc_lower.contains("private") {
                flags |= CACHE_TYPE_HAS_PRIVATE;
            }
            if cc_lower.contains("no-cache") {
                flags |= CACHE_TYPE_HAS_NO_CACHE;
            }
            if cc_lower.contains("no-store") {
                flags |= CACHE_TYPE_HAS_NO_STORE;
            }
            if cc_lower.contains("must-revalidate") {
                flags |= CACHE_TYPE_HAS_MUST_REVALIDATE;
            }
            if cc_lower.contains("proxy-revalidate") {
                flags |= CACHE_TYPE_HAS_PROXY_REVALIDATE;
            }
            if cc_lower.contains("immutable") {
                flags |= CACHE_TYPE_HAS_IMMUTABLE;
            }
        }

        if headers.contains_key("expires") {
            flags |= CACHE_TYPE_HAS_EXPIRES;
        }
        if headers.contains_key("etag") {
            flags |= CACHE_TYPE_HAS_ETAG;
        }
        if headers.contains_key("last-modified") {
            flags |= CACHE_TYPE_HAS_LAST_MODIFIED;
        }

        if flags == 0 {
            flags = CACHE_TYPE_NO_CACHE_HEADERS;
        }

        flags
    }

    /// Get cache lifetime from response headers (in seconds)
    fn get_cache_lifetime(headers: &HashMap<String, String>) -> Option<i32> {
        if let Some(cache_control) = headers.get("cache-control") {
            let cc_lower = cache_control.to_lowercase();
            // Try max-age first
            if let Some(pos) = cc_lower.find("max-age=") {
                let after = &cc_lower[pos + 8..];
                let num_str: String = after.chars().take_while(|c| c.is_ascii_digit()).collect();
                if let Ok(seconds) = num_str.parse::<i32>() {
                    return Some(seconds);
                }
            }
        }
        None
    }

    // --- Public accessors ---

    pub fn get_content_processor_manager(&self) -> &Arc<Mutex<ContentProcessorManager>> {
        &self.content_processor_manager
    }

    pub fn get_initial_parsed_url(&self) -> &ParsedUrl {
        &self.initial_parsed_url
    }

    pub fn get_options(&self) -> &Arc<CoreOptions> {
        &self.options
    }

    pub fn get_output(&self) -> &Arc<Mutex<Box<dyn Output>>> {
        &self.output
    }

    pub fn get_status(&self) -> &Arc<Mutex<Status>> {
        &self.status
    }

    pub fn get_visited(&self) -> &Arc<DashMap<String, VisitedEntry>> {
        &self.visited
    }

    pub fn get_queue(&self) -> &Arc<DashMap<String, QueueEntry>> {
        &self.queue
    }

    pub fn get_skipped(&self) -> &Arc<DashMap<String, SkippedEntry>> {
        &self.skipped
    }

    pub fn get_analysis_manager(&self) -> &Arc<Mutex<AnalysisManager>> {
        &self.analysis_manager
    }

    pub fn get_done_urls_count(&self) -> usize {
        self.done_urls_count.load(Ordering::SeqCst)
    }
}

/// Simple pseudo-random number for query params
fn rand_simple() -> u64 {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default();
    now.as_nanos() as u64 % 1_000_000
}

/// Pre-compile domain wildcard patterns into regex (e.g. "*.example.com" → "^.*\.example\.com$")
fn compile_domain_patterns(domains: &[String]) -> Vec<Regex> {
    domains
        .iter()
        .filter_map(|d| {
            let pattern = format!("^{}$", regex::escape(d).replace(r"\*", ".*"));
            Regex::new(&pattern).ok()
        })
        .collect()
}

/// Filter query parameters in a URL, keeping only those whose names are in the allowlist.
fn filter_query_params(url: &str, keep_params: &[String]) -> String {
    if let Some(q_pos) = url.find('?') {
        let base = &url[..q_pos];
        let query_str = &url[q_pos + 1..];
        let filtered: Vec<&str> = query_str
            .split('&')
            .filter(|pair| {
                let name = pair.split('=').next().unwrap_or("");
                !name.is_empty() && keep_params.iter().any(|k| k == name)
            })
            .collect();
        if filtered.is_empty() {
            base.to_string()
        } else {
            format!("{}?{}", base, filtered.join("&"))
        }
    } else {
        url.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // =========================================================================
    // <base href> regex tests (#68)
    // =========================================================================

    #[test]
    fn base_href_double_quotes() {
        let html = r#"<html><head><base href="https://example.com/subdir/"></head></html>"#;
        let caps = RE_BASE_HREF.captures(html).unwrap();
        assert_eq!(caps.get(1).unwrap().as_str(), "https://example.com/subdir/");
    }

    #[test]
    fn base_href_single_quotes() {
        let html = r#"<html><head><base href='https://example.com/'></head></html>"#;
        let caps = RE_BASE_HREF.captures(html).unwrap();
        assert_eq!(caps.get(1).unwrap().as_str(), "https://example.com/");
    }

    #[test]
    fn base_href_no_quotes() {
        let html = r#"<base href=https://example.com/dir/>"#;
        let caps = RE_BASE_HREF.captures(html).unwrap();
        assert_eq!(caps.get(1).unwrap().as_str(), "https://example.com/dir/");
    }

    #[test]
    fn base_href_relative_path() {
        let html = r#"<base href="/subdir/">"#;
        let caps = RE_BASE_HREF.captures(html).unwrap();
        assert_eq!(caps.get(1).unwrap().as_str(), "/subdir/");
    }

    #[test]
    fn base_href_case_insensitive() {
        let html = r#"<BASE HREF="https://example.com/">"#;
        let caps = RE_BASE_HREF.captures(html).unwrap();
        assert_eq!(caps.get(1).unwrap().as_str(), "https://example.com/");
    }

    #[test]
    fn base_href_absent() {
        let html = r#"<html><head><title>No base</title></head></html>"#;
        assert!(RE_BASE_HREF.captures(html).is_none());
    }

    #[test]
    fn base_href_with_other_attrs() {
        let html = r#"<base target="_blank" href="https://example.com/app/">"#;
        let caps = RE_BASE_HREF.captures(html).unwrap();
        assert_eq!(caps.get(1).unwrap().as_str(), "https://example.com/app/");
    }

    // =========================================================================
    // is_sitemap_url tests (#69)
    // =========================================================================

    #[test]
    fn sitemap_url_standard() {
        let url = ParsedUrl::parse("https://example.com/sitemap.xml", None);
        assert!(Crawler::is_sitemap_url(&url));
    }

    #[test]
    fn sitemap_url_with_index() {
        let url = ParsedUrl::parse("https://example.com/sitemap-index.xml", None);
        assert!(Crawler::is_sitemap_url(&url));
    }

    #[test]
    fn sitemap_url_nested() {
        let url = ParsedUrl::parse("https://example.com/sitemaps/sitemap-pages.xml", None);
        assert!(Crawler::is_sitemap_url(&url));
    }

    #[test]
    fn sitemap_url_case_insensitive() {
        let url = ParsedUrl::parse("https://example.com/Sitemap.XML", None);
        assert!(Crawler::is_sitemap_url(&url));
    }

    #[test]
    fn not_sitemap_regular_page() {
        let url = ParsedUrl::parse("https://example.com/about", None);
        assert!(!Crawler::is_sitemap_url(&url));
    }

    #[test]
    fn not_sitemap_xml_without_sitemap() {
        let url = ParsedUrl::parse("https://example.com/feed.xml", None);
        assert!(!Crawler::is_sitemap_url(&url));
    }

    #[test]
    fn not_sitemap_html_page() {
        let url = ParsedUrl::parse("https://example.com/sitemap.html", None);
        assert!(!Crawler::is_sitemap_url(&url));
    }

    #[test]
    fn sitemap_url_gzip() {
        let url = ParsedUrl::parse("https://example.com/sitemap.xml.gz", None);
        assert!(Crawler::is_sitemap_url(&url));
    }

    #[test]
    fn sitemap_url_gzip_nested() {
        let url = ParsedUrl::parse("https://example.com/sitemaps/sitemap-posts.xml.gz", None);
        assert!(Crawler::is_sitemap_url(&url));
    }

    #[test]
    fn not_sitemap_tar_gz() {
        let url = ParsedUrl::parse("https://example.com/archive.tar.gz", None);
        assert!(!Crawler::is_sitemap_url(&url));
    }

    // =========================================================================
    // normalize_url_to_initial tests (#35)
    // =========================================================================

    #[test]
    fn normalize_www_to_no_www() {
        let initial = ParsedUrl::parse("https://example.com/", None);
        let mut url = ParsedUrl::parse("https://www.example.com/page", None);
        Crawler::normalize_url_to_initial(&mut url, &initial);
        assert_eq!(url.host.as_deref(), Some("example.com"));
        assert_eq!(url.scheme, Some("https".to_string()));
    }

    #[test]
    fn normalize_no_www_to_www() {
        let initial = ParsedUrl::parse("https://www.example.com/", None);
        let mut url = ParsedUrl::parse("https://example.com/page", None);
        Crawler::normalize_url_to_initial(&mut url, &initial);
        assert_eq!(url.host.as_deref(), Some("www.example.com"));
    }

    #[test]
    fn normalize_http_to_https() {
        let initial = ParsedUrl::parse("https://example.com/", None);
        let mut url = ParsedUrl::parse("http://example.com/page", None);
        Crawler::normalize_url_to_initial(&mut url, &initial);
        assert_eq!(url.scheme, Some("https".to_string()));
    }

    #[test]
    fn normalize_both_www_and_scheme() {
        let initial = ParsedUrl::parse("https://example.com/", None);
        let mut url = ParsedUrl::parse("http://www.example.com/page", None);
        Crawler::normalize_url_to_initial(&mut url, &initial);
        assert_eq!(url.host.as_deref(), Some("example.com"));
        assert_eq!(url.scheme, Some("https".to_string()));
    }

    #[test]
    fn normalize_leaves_different_domain_unchanged() {
        let initial = ParsedUrl::parse("https://example.com/", None);
        let mut url = ParsedUrl::parse("https://other.com/page", None);
        Crawler::normalize_url_to_initial(&mut url, &initial);
        assert_eq!(url.host.as_deref(), Some("other.com"));
    }

    #[test]
    fn normalize_same_url_no_change() {
        let initial = ParsedUrl::parse("https://example.com/", None);
        let mut url = ParsedUrl::parse("https://example.com/page", None);
        Crawler::normalize_url_to_initial(&mut url, &initial);
        assert_eq!(url.host.as_deref(), Some("example.com"));
        assert_eq!(url.scheme, Some("https".to_string()));
    }

    #[test]
    fn normalize_preserves_path() {
        let initial = ParsedUrl::parse("https://example.com/", None);
        let mut url = ParsedUrl::parse("http://www.example.com/some/deep/path?q=1", None);
        Crawler::normalize_url_to_initial(&mut url, &initial);
        assert_eq!(url.path, "/some/deep/path");
        assert_eq!(url.query.as_deref(), Some("q=1"));
    }

    #[test]
    fn filter_query_params_keeps_specified() {
        let keep = vec!["foo".to_string(), "baz".to_string()];
        let result = filter_query_params("https://example.com/page?foo=1&bar=2&baz=3", &keep);
        assert_eq!(result, "https://example.com/page?foo=1&baz=3");
    }

    #[test]
    fn filter_query_params_removes_all_when_none_match() {
        let keep = vec!["xyz".to_string()];
        let result = filter_query_params("https://example.com/page?foo=1&bar=2", &keep);
        assert_eq!(result, "https://example.com/page");
    }

    #[test]
    fn filter_query_params_no_query_string() {
        let keep = vec!["foo".to_string()];
        let result = filter_query_params("https://example.com/page", &keep);
        assert_eq!(result, "https://example.com/page");
    }

    #[test]
    fn filter_query_params_keeps_param_without_value() {
        let keep = vec!["debug".to_string()];
        let result = filter_query_params("https://example.com/page?debug&foo=bar", &keep);
        assert_eq!(result, "https://example.com/page?debug");
    }

    #[test]
    fn filter_query_params_preserves_order() {
        let keep = vec!["c".to_string(), "a".to_string()];
        let result = filter_query_params("https://example.com/?a=1&b=2&c=3", &keep);
        assert_eq!(result, "https://example.com/?a=1&c=3");
    }

    #[test]
    fn filter_query_params_single_kept_param() {
        let keep = vec!["id".to_string()];
        let result = filter_query_params("https://example.com/page?id=42&session=abc&tracking=xyz", &keep);
        assert_eq!(result, "https://example.com/page?id=42");
    }
}
