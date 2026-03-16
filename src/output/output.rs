// SiteOne Crawler - Output trait
// (c) Jan Reges <jan.reges@siteone.cz>

use std::collections::{BTreeMap, HashMap};

use crate::components::summary::summary::Summary;
use crate::components::super_table::SuperTable;
use crate::extra_column::ExtraColumn;
use crate::output::output_type::OutputType;
use crate::scoring::ci_gate::CiGateResult;
use crate::scoring::quality_score::QualityScores;

/// Trait for crawler output implementations (text console, JSON, multi-output).
///
/// All implementations must be Send + Sync for use in async contexts.
pub trait Output: Send + Sync {
    /// Print the banner (ASCII art for text, crawler info for JSON).
    fn add_banner(&mut self);

    /// Print the used crawler options.
    fn add_used_options(&mut self);

    /// Set extra columns from analysis that will be added to the URL table.
    fn set_extra_columns_from_analysis(&mut self, extra_columns: Vec<ExtraColumn>);

    /// Print the URL table header row.
    fn add_table_header(&mut self);

    /// Print a single URL table row with crawl result data.
    ///
    /// # Arguments
    /// * `response_headers` - flat response headers (lowercase key -> value)
    /// * `url` - the visited URL
    /// * `status` - HTTP status code (negative for errors)
    /// * `elapsed_time` - request duration in seconds
    /// * `size` - response body size in bytes
    /// * `content_type` - content type ID (see ContentTypeId)
    /// * `extra_parsed_content` - extra column values extracted from the response
    /// * `progress_status` - progress string like "45/100"
    /// * `cache_type_flags` - bitwise cache type flags
    /// * `cache_lifetime` - cache lifetime in seconds, if known
    #[allow(clippy::too_many_arguments)]
    fn add_table_row(
        &mut self,
        response_headers: &HashMap<String, String>,
        url: &str,
        status: i32,
        elapsed_time: f64,
        size: i64,
        content_type: i32,
        extra_parsed_content: &HashMap<String, String>,
        progress_status: &str,
        cache_type_flags: i32,
        cache_lifetime: Option<i32>,
    );

    /// Add a SuperTable to the output.
    fn add_super_table(&mut self, table: &SuperTable);

    /// Add total crawl statistics.
    ///
    /// # Arguments
    /// * `stats` - basic crawl statistics
    fn add_total_stats(&mut self, stats: &BasicStats);

    /// Add a notice/informational message.
    fn add_notice(&mut self, text: &str);

    /// Add an error message.
    fn add_error(&mut self, text: &str);

    /// Add quality scores before the summary.
    fn add_quality_scores(&mut self, _scores: &QualityScores) {}

    /// Add CI/CD quality gate result after quality scores.
    fn add_ci_gate_result(&mut self, _result: &CiGateResult) {}

    /// Add the final summary with status items.
    fn add_summary(&mut self, summary: &mut Summary);

    /// Get the output type enum variant.
    fn get_type(&self) -> OutputType;

    /// Finalize and flush the output.
    fn end(&mut self);

    /// Get the accumulated text output content (for file export).
    /// Only TextOutput implements this meaningfully.
    fn get_output_text(&self) -> Option<String> {
        None
    }

    /// Get the accumulated JSON output content (for file export).
    /// Only JsonOutput implements this meaningfully.
    fn get_json_content(&self) -> Option<String> {
        None
    }
}

/// Basic crawl statistics, used by add_total_stats().
/// This is a simplified version; the full Status/BasicStats will be provided by the result module.
#[derive(Debug, Clone, Default)]
pub struct BasicStats {
    pub total_urls: usize,
    pub total_size: i64,
    pub total_size_formatted: String,
    pub total_execution_time: f64,
    pub total_requests_times: f64,
    pub total_requests_times_avg: f64,
    pub total_requests_times_min: f64,
    pub total_requests_times_max: f64,
    pub count_by_status: BTreeMap<i32, usize>,
    pub count_by_content_type: BTreeMap<i32, usize>,
}

/// Crawler info for the JSON banner output.
#[derive(Debug, Clone, Default, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CrawlerInfo {
    pub name: String,
    pub version: String,
    pub executed_at: String,
    pub command: String,
    pub hostname: String,
    pub final_user_agent: String,
    // Used by TextOutput for the banner (not serialized to JSON)
    #[serde(skip)]
    pub url: String,
    #[serde(skip)]
    pub device: String,
    #[serde(skip)]
    pub workers: usize,
}
