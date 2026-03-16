// SiteOne Crawler - Status (central crawl state)
// (c) Jan Reges <jan.reges@siteone.cz>

use std::collections::HashMap;
use std::sync::{Mutex, RwLock};
use std::time::Instant;

use indexmap::IndexMap;

use crate::analysis::result::url_analysis_result::UrlAnalysisResult;
use crate::components::summary::item::Item;
use crate::components::summary::item_status::ItemStatus;
use crate::components::summary::summary::Summary;
use crate::components::super_table::SuperTable;
use crate::info::Info;
use crate::result::basic_stats::BasicStats;
use crate::result::storage::storage::Storage;
use crate::result::visited_url::VisitedUrl;
use crate::types::{ContentTypeId, SkippedReason};

/// Central state for the crawl result.
/// Must be Send + Sync for concurrent access from multiple workers.
pub struct Status {
    /// Content storage (memory or file) - used only if store_content is true
    storage: Box<dyn Storage>,

    /// Store content of visited URLs (HTML, CSS, JS, images, ...) to storage
    store_content: bool,

    /// Crawl start time
    start_time: Instant,

    /// Basic stats/metrics about visited URLs (lazily computed)
    basic_stats: RwLock<Option<BasicStats>>,

    /// Overall summary of the crawl
    summary: Mutex<Summary>,

    /// SuperTables that are at the beginning of the page
    super_tables_at_beginning: Mutex<Vec<SuperTable>>,

    /// SuperTables that are at the end of the page
    super_tables_at_end: Mutex<Vec<SuperTable>>,

    /// Crawler info
    crawler_info: RwLock<Info>,

    /// Visited URLs, keyed by uq_id (IndexMap preserves crawl/insertion order)
    visited_urls: Mutex<IndexMap<String, VisitedUrl>>,

    /// Analysis results per visited URL uq_id
    visited_url_to_analysis_result: Mutex<HashMap<String, Vec<UrlAnalysisResultEntry>>>,

    /// Robots.txt content - key is "scheme://host:port"
    robots_txt_content: RwLock<HashMap<String, String>>,

    /// Skipped URLs (transferred from crawler after crawling)
    skipped_urls: Mutex<Vec<SkippedUrlEntry>>,
}

/// Entry for a skipped URL stored in Status
#[derive(Debug, Clone)]
pub struct SkippedUrlEntry {
    pub url: String,
    pub reason: SkippedReason,
    pub source_uq_id: String,
    pub source_attr: i32,
}

/// Per-URL analysis result entry stored in Status
#[derive(Debug, Clone)]
pub struct UrlAnalysisResultEntry {
    pub analysis_name: String,
    pub result: UrlAnalysisResult,
}

// SAFETY: Status uses internal synchronization primitives (Mutex, RwLock)
// for all mutable state, making it safe to share across threads.
unsafe impl Send for Status {}
unsafe impl Sync for Status {}

impl Status {
    pub fn new(storage: Box<dyn Storage>, store_content: bool, crawler_info: Info, start_time: Instant) -> Self {
        Self {
            storage,
            store_content,
            start_time,
            basic_stats: RwLock::new(None),
            summary: Mutex::new(Summary::new()),
            super_tables_at_beginning: Mutex::new(Vec::new()),
            super_tables_at_end: Mutex::new(Vec::new()),
            crawler_info: RwLock::new(crawler_info),
            visited_urls: Mutex::new(IndexMap::new()),
            visited_url_to_analysis_result: Mutex::new(HashMap::new()),
            robots_txt_content: RwLock::new(HashMap::new()),
            skipped_urls: Mutex::new(Vec::new()),
        }
    }

    pub fn add_visited_url(
        &mut self,
        visited_url: VisitedUrl,
        body: Option<&[u8]>,
        headers: Option<&HashMap<String, String>>,
    ) {
        let uq_id = visited_url.uq_id.clone();
        let content_type = visited_url.content_type;

        if let Ok(mut urls) = self.visited_urls.lock() {
            urls.insert(uq_id.clone(), visited_url);
        }

        if self.store_content {
            if let Some(body_bytes) = body {
                let content = if content_type == ContentTypeId::Html {
                    // Trim whitespace for HTML (text-safe operation)
                    let text = String::from_utf8_lossy(body_bytes);
                    text.trim().as_bytes().to_vec()
                } else {
                    body_bytes.to_vec()
                };
                // Ignore storage errors - they are non-fatal
                let _ = self.storage.save(&uq_id, &content);
            }

            if let Some(hdrs) = headers {
                // Serialize headers as JSON for storage
                if let Ok(serialized) = serde_json::to_string(hdrs) {
                    let _ = self.storage.save(&format!("{}.headers", uq_id), serialized.as_bytes());
                }
            }
        }

        // Invalidate cached basic stats
        if let Ok(mut stats) = self.basic_stats.write() {
            *stats = None;
        }
    }

    pub fn add_summary_item_by_ranges(
        &self,
        apl_code: &str,
        value: f64,
        ranges: &[(f64, f64)],
        text_per_range: &[&str],
    ) {
        let mut status = ItemStatus::Info;
        let mut text = format!("{} out of range ({})", apl_code, value);

        for (range_id, range) in ranges.iter().enumerate() {
            if value >= range.0 && value <= range.1 {
                if let Ok(s) = ItemStatus::from_range_id(range_id as i32) {
                    status = s;
                }
                if let Some(tmpl) = text_per_range.get(range_id) {
                    text = tmpl.replace("{}", &format!("{}", value));
                }
                break;
            }
        }

        if let Ok(mut summary) = self.summary.lock() {
            summary.add_item(Item::new(apl_code.to_string(), text, status));
        }
    }

    pub fn add_ok_to_summary(&self, apl_code: &str, text: &str) {
        if let Ok(mut summary) = self.summary.lock() {
            summary.add_item(Item::new(apl_code.to_string(), text.to_string(), ItemStatus::Ok));
        }
    }

    pub fn add_notice_to_summary(&self, apl_code: &str, text: &str) {
        if let Ok(mut summary) = self.summary.lock() {
            summary.add_item(Item::new(apl_code.to_string(), text.to_string(), ItemStatus::Notice));
        }
    }

    pub fn add_info_to_summary(&self, apl_code: &str, text: &str) {
        if let Ok(mut summary) = self.summary.lock() {
            summary.add_item(Item::new(apl_code.to_string(), text.to_string(), ItemStatus::Info));
        }
    }

    pub fn add_warning_to_summary(&self, apl_code: &str, text: &str) {
        if let Ok(mut summary) = self.summary.lock() {
            summary.add_item(Item::new(apl_code.to_string(), text.to_string(), ItemStatus::Warning));
        }
    }

    pub fn add_critical_to_summary(&self, apl_code: &str, text: &str) {
        if let Ok(mut summary) = self.summary.lock() {
            summary.add_item(Item::new(apl_code.to_string(), text.to_string(), ItemStatus::Critical));
        }
    }

    pub fn get_summary(&self) -> Summary {
        self.summary.lock().map(|s| s.clone()).unwrap_or_default()
    }

    pub fn with_summary<F, R>(&self, f: F) -> Option<R>
    where
        F: FnOnce(&mut Summary) -> R,
    {
        self.summary.lock().ok().map(|mut s| f(&mut s))
    }

    /// Get stored body as raw bytes (preserves binary data for images, fonts, etc.)
    pub fn get_url_body(&self, uq_id: &str) -> Option<Vec<u8>> {
        if !self.store_content {
            return None;
        }
        self.storage.load(uq_id).ok().filter(|b| !b.is_empty())
    }

    /// Get stored body as text (lossy UTF-8 conversion). Use for HTML/CSS/JS processing.
    pub fn get_url_body_text(&self, uq_id: &str) -> Option<String> {
        self.get_url_body(uq_id)
            .map(|b| String::from_utf8_lossy(&b).into_owned())
    }

    pub fn get_url_headers(&self, uq_id: &str) -> Option<HashMap<String, String>> {
        let key = format!("{}.headers", uq_id);
        let data = self.storage.load(&key).ok()?;
        if data.is_empty() {
            return None;
        }
        serde_json::from_slice(&data).ok()
    }

    pub fn get_visited_urls(&self) -> Vec<VisitedUrl> {
        self.visited_urls
            .lock()
            .map(|urls| urls.values().cloned().collect())
            .unwrap_or_default()
    }

    pub fn with_visited_urls<F, R>(&self, f: F) -> Option<R>
    where
        F: FnOnce(&IndexMap<String, VisitedUrl>) -> R,
    {
        self.visited_urls.lock().ok().map(|urls| f(&urls))
    }

    pub fn get_crawler_info(&self) -> Info {
        self.crawler_info.read().map(|info| info.clone()).unwrap_or_else(|_| {
            Info::new(
                String::new(),
                String::new(),
                String::new(),
                String::new(),
                String::new(),
                String::new(),
                String::new(),
            )
        })
    }

    pub fn get_storage(&self) -> &dyn Storage {
        self.storage.as_ref()
    }

    pub fn set_final_user_agent(&self, value: &str) {
        if let Ok(mut info) = self.crawler_info.write() {
            info.set_final_user_agent(value.to_string());
        }
    }

    pub fn get_basic_stats(&self) -> BasicStats {
        // Check if we already have cached stats
        if let Ok(stats_guard) = self.basic_stats.read()
            && let Some(ref stats) = *stats_guard
        {
            return stats.clone();
        }

        // Compute stats
        let stats = match self.visited_urls.lock() {
            Ok(urls) => {
                let url_refs: Vec<&VisitedUrl> = urls.values().collect();
                BasicStats::from_visited_urls(&url_refs, self.start_time)
            }
            _ => BasicStats::from_visited_urls(&[], self.start_time),
        };

        // Cache the result
        if let Ok(mut stats_guard) = self.basic_stats.write() {
            *stats_guard = Some(stats.clone());
        }

        stats
    }

    pub fn add_super_table_at_beginning(&self, super_table: SuperTable) {
        if let Ok(mut tables) = self.super_tables_at_beginning.lock() {
            tables.push(super_table);
        }
    }

    pub fn add_super_table_at_end(&self, super_table: SuperTable) {
        if let Ok(mut tables) = self.super_tables_at_end.lock() {
            tables.push(super_table);
        }
    }

    pub fn with_super_tables_at_beginning<F, R>(&self, f: F) -> Option<R>
    where
        F: FnOnce(&[SuperTable]) -> R,
    {
        self.super_tables_at_beginning.lock().ok().map(|tables| f(&tables))
    }

    pub fn with_super_tables_at_beginning_mut<F, R>(&self, f: F) -> Option<R>
    where
        F: FnOnce(&mut [SuperTable]) -> R,
    {
        self.super_tables_at_beginning
            .lock()
            .ok()
            .map(|mut tables| f(&mut tables))
    }

    pub fn with_super_tables_at_end<F, R>(&self, f: F) -> Option<R>
    where
        F: FnOnce(&[SuperTable]) -> R,
    {
        self.super_tables_at_end.lock().ok().map(|tables| f(&tables))
    }

    pub fn with_super_tables_at_end_mut<F, R>(&self, f: F) -> Option<R>
    where
        F: FnOnce(&mut [SuperTable]) -> R,
    {
        self.super_tables_at_end.lock().ok().map(|mut tables| f(&mut tables))
    }

    /// Set host_to_strip_from_urls and initial_url on a SuperTable based on crawler info.
    /// Used so that URLs matching the initial domain are displayed without protocol+domain.
    pub fn configure_super_table_url_stripping(&self, table: &mut SuperTable) {
        let info = self.get_crawler_info();
        if !info.initial_url.is_empty()
            && let Ok(parsed) = url::Url::parse(&info.initial_url)
        {
            table.set_host_to_strip_from_urls(
                parsed.host_str().map(|h| h.to_string()),
                Some(parsed.scheme().to_string()),
            );
            table.set_initial_url(Some(info.initial_url.clone()));
        }
    }

    pub fn get_super_table_by_apl_code(&self, apl_code: &str) -> bool {
        let found_beginning = self
            .super_tables_at_beginning
            .lock()
            .ok()
            .map(|tables| tables.iter().any(|t| t.apl_code == apl_code))
            .unwrap_or(false);

        if found_beginning {
            return true;
        }

        self.super_tables_at_end
            .lock()
            .ok()
            .map(|tables| tables.iter().any(|t| t.apl_code == apl_code))
            .unwrap_or(false)
    }

    pub fn get_url_by_uq_id(&self, uq_id: &str) -> Option<String> {
        self.visited_urls
            .lock()
            .ok()
            .and_then(|urls| urls.get(uq_id).map(|v| v.url.clone()))
    }

    pub fn get_origin_header_value_by_source_uq_id(&self, source_uq_id: &str) -> Option<String> {
        self.visited_urls.lock().ok().and_then(|urls| {
            urls.get(source_uq_id).and_then(|visited_url| {
                url::Url::parse(&visited_url.url).ok().map(|parsed| {
                    let scheme = parsed.scheme();
                    let host = parsed.host_str().unwrap_or("");
                    let port = parsed.port();
                    if let Some(p) = port {
                        format!("{}://{}:{}", scheme, host, p)
                    } else {
                        format!("{}://{}", scheme, host)
                    }
                })
            })
        })
    }

    pub fn add_url_analysis_result(&self, visited_url_uq_id: &str, result: UrlAnalysisResultEntry) {
        if let Ok(mut map) = self.visited_url_to_analysis_result.lock() {
            map.entry(visited_url_uq_id.to_string()).or_default().push(result);
        }
    }

    pub fn get_url_analysis_results(&self, visited_url_uq_id: &str) -> Vec<UrlAnalysisResultEntry> {
        self.visited_url_to_analysis_result
            .lock()
            .ok()
            .and_then(|map| map.get(visited_url_uq_id).cloned())
            .unwrap_or_default()
    }

    pub fn add_skipped_url(&mut self, url: String, reason: SkippedReason, source_uq_id: String, source_attr: i32) {
        if let Ok(mut skipped) = self.skipped_urls.lock() {
            skipped.push(SkippedUrlEntry {
                url,
                reason,
                source_uq_id,
                source_attr,
            });
        }
    }

    pub fn get_skipped_urls(&self) -> Vec<SkippedUrlEntry> {
        self.skipped_urls.lock().ok().map(|v| v.clone()).unwrap_or_default()
    }

    pub fn get_details_by_analysis_name_and_severity(&self, analysis_name: &str, severity: &str) -> Vec<String> {
        let mut result = Vec::new();
        if let Ok(map) = self.visited_url_to_analysis_result.lock() {
            for entries in map.values() {
                for entry in entries {
                    let details = entry
                        .result
                        .get_details_of_severity_and_analysis_name(severity, analysis_name);
                    result.extend(details);
                }
            }
        }
        result
    }

    pub fn get_visited_url_to_analysis_result(&self) -> HashMap<String, Vec<UrlAnalysisResultEntry>> {
        self.visited_url_to_analysis_result
            .lock()
            .map(|map| map.clone())
            .unwrap_or_default()
    }

    /// Get number of visited URLs with HTTP code >= 200
    pub fn get_number_of_working_visited_urls(&self) -> usize {
        self.visited_urls
            .lock()
            .map(|urls| urls.values().filter(|u| u.status_code >= 200).count())
            .unwrap_or(0)
    }

    pub fn set_robots_txt_content(&self, scheme: &str, host: &str, port: u16, content: &str) {
        let key = format!("{}://{}:{}", scheme, host, port);
        if let Ok(mut map) = self.robots_txt_content.write() {
            map.insert(key, content.to_string());
        }
    }

    pub fn get_robots_txt_content(&self, scheme: &str, host: &str, port: u16) -> Option<String> {
        let key = format!("{}://{}:{}", scheme, host, port);
        self.robots_txt_content
            .read()
            .ok()
            .and_then(|map| map.get(&key).cloned())
    }
}

impl std::fmt::Debug for Status {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Status")
            .field("store_content", &self.store_content)
            .field(
                "visited_urls_count",
                &self.visited_urls.lock().map(|u| u.len()).unwrap_or(0),
            )
            .finish()
    }
}
