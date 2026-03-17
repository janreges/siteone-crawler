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
