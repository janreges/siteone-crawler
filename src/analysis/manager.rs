// SiteOne Crawler - Analysis Manager
// (c) Jan Reges <jan.reges@siteone.cz>

use std::collections::HashMap;

use crate::analysis::analyzer::Analyzer;
use crate::analysis::result::url_analysis_result::UrlAnalysisResult;
use crate::output::output::Output;
use crate::result::manager_stats::ManagerStats;
use crate::result::status::Status;
use crate::result::visited_url::VisitedUrl;
use crate::utils;

pub const SUPER_TABLE_ANALYSIS_STATS: &str = "analysis-stats";

pub struct AnalysisManager {
    analyzers: Vec<Box<dyn Analyzer>>,
    stats: ManagerStats,
}

impl AnalysisManager {
    pub fn new() -> Self {
        Self {
            analyzers: Vec::new(),
            stats: ManagerStats::new(),
        }
    }

    /// Register all analyzer instances. Each analyzer's should_be_activated()
    /// determines whether it is actually used.
    pub fn register_analyzer(&mut self, analyzer: Box<dyn Analyzer>) {
        self.analyzers.push(analyzer);
    }

    /// Auto-activate: remove analyzers that should not be activated based on options.
    pub fn auto_activate_analyzers(&mut self) {
        self.analyzers.retain(|a| a.should_be_activated());
    }

    /// Filter analyzers by regex pattern.
    /// Only analyzers whose name matches the regex are kept.
    /// Supports PCRE-style delimited patterns (e.g., /security/i).
    pub fn filter_analyzers_by_regex(&mut self, filter_regex: &str) {
        let pattern = utils::extract_pcre_regex_pattern(filter_regex);
        if let Ok(re) = fancy_regex::Regex::new(&pattern) {
            self.analyzers.retain(|a| re.is_match(a.get_name()).unwrap_or(true));
        }
    }

    /// Run analyze_visited_url for each active analyzer.
    /// Called per URL during the crawl.
    pub fn analyze_visited_url(
        &mut self,
        visited_url: &VisitedUrl,
        body: Option<&str>,
        headers: Option<&HashMap<String, String>>,
        status: &Status,
    ) -> Vec<(String, UrlAnalysisResult)> {
        let mut results = Vec::new();

        for analyzer in &mut self.analyzers {
            if let Some(result) = analyzer.analyze_visited_url(visited_url, body, headers) {
                let name = analyzer.get_name().to_string();
                status.add_url_analysis_result(
                    &visited_url.uq_id,
                    crate::result::status::UrlAnalysisResultEntry {
                        analysis_name: name.clone(),
                        result: result.clone(),
                    },
                );
                results.push((name, result));
            }
        }

        results
    }

    /// Run post-crawl analysis for all active analyzers, sorted by order.
    pub fn run_analyzers(&mut self, status: &Status, output: &mut dyn Output) {
        // Check if there are any working URLs
        if status.get_number_of_working_visited_urls() == 0 {
            let error_message =
                "The analysis has been suspended because no working URL could be found. Please check the URL/domain.";
            output.add_error(error_message);
            status.add_critical_to_summary("analysis-manager-error", error_message);

            // Analyzers that need no crawled page still run, e.g. the SSL/TLS analyzer explains why an
            // HTTPS site could not be crawled (a server that only offers RC4/3DES suites, #20).
            self.analyzers.sort_by_key(|a| a.get_order());
            for analyzer in self.analyzers.iter_mut().filter(|a| a.runs_without_working_urls()) {
                analyzer.analyze(status, output);
            }
            return;
        }

        // Sort analyzers by order
        self.analyzers.sort_by_key(|a| a.get_order());

        for analyzer in &mut self.analyzers {
            analyzer.analyze(status, output);
        }

        // Collect and merge exec times from all analyzers
        if !self.analyzers.is_empty() {
            let mut all_exec_times: HashMap<String, f64> = HashMap::new();
            let mut all_exec_counts: HashMap<String, usize> = HashMap::new();

            for analyzer in &self.analyzers {
                for (key, time) in analyzer.get_exec_times() {
                    *all_exec_times.entry(key.clone()).or_insert(0.0) += time;
                }
                for (key, count) in analyzer.get_exec_counts() {
                    *all_exec_counts.entry(key.clone()).or_insert(0) += count;
                }
            }

            let super_table = self.stats.get_super_table(
                SUPER_TABLE_ANALYSIS_STATS,
                "Analysis stats",
                "No analysis stats",
                Some(&all_exec_times),
                Some(&all_exec_counts),
            );

            let mut super_table = super_table;
            status.configure_super_table_url_stripping(&mut super_table);
            output.add_super_table(&super_table);
            status.add_super_table_at_end(super_table);
        }
    }

    /// Get all analyzers
    pub fn get_analyzers(&self) -> &[Box<dyn Analyzer>] {
        &self.analyzers
    }

    /// Check if analyzer with given name is active
    pub fn has_analyzer(&self, name: &str) -> bool {
        self.analyzers.iter().any(|a| a.get_name() == name)
    }

    /// Get extra columns from all analyzers that want to show results as columns.
    /// Returns columns in registration order (alphabetical).
    pub fn get_extra_columns(&self) -> Vec<crate::extra_column::ExtraColumn> {
        self.analyzers
            .iter()
            .filter_map(|a| a.show_analyzed_visited_url_result_as_column())
            .collect()
    }

    /// Map analysis results to extra column values for the progress table.
    /// Returns a HashMap of column_name -> colorized_value_string.
    pub fn get_analysis_column_values(
        &self,
        analysis_results: &[(String, UrlAnalysisResult)],
    ) -> HashMap<String, String> {
        let mut result = HashMap::new();

        for analyzer in &self.analyzers {
            if let Some(extra_col) = analyzer.show_analyzed_visited_url_result_as_column() {
                let analyzer_name = analyzer.get_name();
                // Find the matching result for this analyzer
                if let Some((_, url_result)) = analysis_results.iter().find(|(name, _)| name == analyzer_name) {
                    let colorized = url_result.to_colorized_string(true);
                    if !colorized.is_empty() {
                        result.insert(extra_col.name.clone(), colorized);
                    }
                }
            }
        }

        result
    }
}

impl Default for AnalysisManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::components::summary::item_status::ItemStatus;
    use crate::info::Info;
    use crate::output::json_output::JsonOutput;
    use crate::output::output::CrawlerInfo;
    use crate::result::storage::memory_storage::MemoryStorage;
    use std::sync::Arc;
    use std::sync::atomic::{AtomicBool, Ordering};

    /// An analyzer that only records whether it ran.
    struct RecordingAnalyzer {
        runs_without_working_urls: bool,
        ran: Arc<AtomicBool>,
        exec_times: HashMap<String, f64>,
        exec_counts: HashMap<String, usize>,
    }

    impl Analyzer for RecordingAnalyzer {
        fn analyze(&mut self, _status: &Status, _output: &mut dyn Output) {
            self.ran.store(true, Ordering::SeqCst);
        }

        fn runs_without_working_urls(&self) -> bool {
            self.runs_without_working_urls
        }

        fn should_be_activated(&self) -> bool {
            true
        }

        fn get_order(&self) -> i32 {
            0
        }

        fn get_name(&self) -> &str {
            "RecordingAnalyzer"
        }

        fn get_exec_times(&self) -> &HashMap<String, f64> {
            &self.exec_times
        }

        fn get_exec_counts(&self) -> &HashMap<String, usize> {
            &self.exec_counts
        }
    }

    fn recording_analyzer(runs_without_working_urls: bool) -> (Box<dyn Analyzer>, Arc<AtomicBool>) {
        let ran = Arc::new(AtomicBool::new(false));
        let analyzer = RecordingAnalyzer {
            runs_without_working_urls,
            ran: ran.clone(),
            exec_times: HashMap::new(),
            exec_counts: HashMap::new(),
        };
        (Box::new(analyzer), ran)
    }

    #[test]
    fn only_analyzers_that_need_no_crawled_page_run_when_no_url_worked() {
        let info = Info::new(
            "SiteOne Crawler".to_string(),
            "test".to_string(),
            String::new(),
            String::new(),
            String::new(),
            String::new(),
            "https://example.com/".to_string(),
        );
        let status = Status::new(
            Box::new(MemoryStorage::new(false)),
            false,
            info,
            std::time::Instant::now(),
        );
        let mut output = JsonOutput::new(CrawlerInfo::default(), vec![], true, false, None, 0);
        let mut manager = AnalysisManager::new();
        let (needs_pages, needs_pages_ran) = recording_analyzer(false);
        let (needs_no_pages, needs_no_pages_ran) = recording_analyzer(true);
        manager.register_analyzer(needs_pages);
        manager.register_analyzer(needs_no_pages);

        manager.run_analyzers(&status, &mut output);

        assert!(needs_no_pages_ran.load(Ordering::SeqCst), "runs without crawled pages");
        assert!(
            !needs_pages_ran.load(Ordering::SeqCst),
            "still suspended without crawled pages"
        );
        assert!(
            status
                .get_summary()
                .get_items()
                .iter()
                .any(|item| item.apl_code == "analysis-manager-error" && item.status == ItemStatus::Critical),
            "the crawl failure is still reported"
        );
    }
}
