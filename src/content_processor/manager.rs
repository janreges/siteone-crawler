// SiteOne Crawler - ContentProcessorManager
// (c) Jan Reges <jan.reges@siteone.cz>
//
// Holds all registered processors and delegates operations to them.

use std::time::Instant;

use crate::engine::found_urls::FoundUrls;
use crate::engine::parsed_url::ParsedUrl;
use crate::result::manager_stats::ManagerStats;
use crate::types::ContentTypeId;

use super::content_processor::ContentProcessor;

pub const SUPER_TABLE_CONTENT_PROCESSORS_STATS: &str = "content-processors-stats";

pub struct ContentProcessorManager {
    processors: Vec<Box<dyn ContentProcessor>>,
    stats: ManagerStats,
}

impl ContentProcessorManager {
    pub fn new() -> Self {
        Self {
            processors: Vec::new(),
            stats: ManagerStats::new(),
        }
    }

    /// Register a content processor. Returns error if a processor with the
    /// same name is already registered.
    pub fn register_processor(&mut self, processor: Box<dyn ContentProcessor>) -> Result<(), String> {
        let name = processor.get_name().to_string();
        if self.processors.iter().any(|p| p.get_name() == name) {
            return Err(format!("Content processor '{}' is already registered", name));
        }
        self.processors.push(processor);
        Ok(())
    }

    /// Get references to all registered processors
    pub fn get_processors(&self) -> &[Box<dyn ContentProcessor>] {
        &self.processors
    }

    /// Find URLs in content using all relevant processors.
    /// Returns a Vec of FoundUrls from each processor that found something.
    pub fn find_urls(&mut self, content: &str, content_type: ContentTypeId, url: &ParsedUrl) -> Vec<FoundUrls> {
        let mut result = Vec::new();

        for processor in &self.processors {
            if processor.is_content_type_relevant(content_type) {
                let start = Instant::now();
                let found_urls = processor.find_urls(content, url);
                self.stats.measure_exec_time(processor.get_name(), "findUrls", start);

                if let Some(urls) = found_urls
                    && urls.get_count() > 0
                {
                    result.push(urls);
                }
            }
        }

        result
    }

    /// Apply content changes for offline version using all relevant processors.
    pub fn apply_content_changes_for_offline_version(
        &mut self,
        content: &mut String,
        content_type: ContentTypeId,
        url: &ParsedUrl,
        remove_unwanted_code: bool,
    ) {
        for processor in &self.processors {
            if processor.is_content_type_relevant(content_type) {
                let start = Instant::now();
                processor.apply_content_changes_for_offline_version(content, content_type, url, remove_unwanted_code);
                self.stats
                    .measure_exec_time(processor.get_name(), "applyContentChangesForOfflineVersion", start);
            }
        }
    }

    /// Apply content changes for offline version with a content loader callback.
    /// Used when storage access is available (e.g., from the offline exporter).
    pub fn apply_content_changes_for_offline_version_with_loader(
        &mut self,
        content: &mut String,
        content_type: ContentTypeId,
        url: &ParsedUrl,
        remove_unwanted_code: bool,
        content_loader: &dyn Fn(&str) -> Option<String>,
    ) {
        for processor in &self.processors {
            if processor.is_content_type_relevant(content_type) {
                let start = Instant::now();
                processor.apply_content_changes_for_offline_version_with_loader(
                    content,
                    content_type,
                    url,
                    remove_unwanted_code,
                    content_loader,
                );
                self.stats
                    .measure_exec_time(processor.get_name(), "applyContentChangesForOfflineVersion", start);
            }
        }
    }

    /// Apply content changes before URL parsing using all relevant processors.
    pub fn apply_content_changes_before_url_parsing(
        &mut self,
        content: &mut String,
        content_type: ContentTypeId,
        url: &ParsedUrl,
    ) {
        for processor in &self.processors {
            if processor.is_content_type_relevant(content_type) {
                let start = Instant::now();
                processor.apply_content_changes_before_url_parsing(content, content_type, url);
                self.stats
                    .measure_exec_time(processor.get_name(), "applyContentChangesBeforeUrlParsing", start);
            }
        }
    }

    /// Get reference to the stats tracker
    pub fn get_stats(&self) -> &ManagerStats {
        &self.stats
    }
}

impl Default for ContentProcessorManager {
    fn default() -> Self {
        Self::new()
    }
}
