// SiteOne Crawler - Analyzer trait
// (c) Jan Reges <jan.reges@siteone.cz>

use std::collections::HashMap;

use crate::analysis::result::url_analysis_result::UrlAnalysisResult;
use crate::extra_column::ExtraColumn;
use crate::result::visited_url::VisitedUrl;

/// Trait that all analyzers must implement.
pub trait Analyzer: Send + Sync {
    /// Do your analysis and set results to output (post-crawl).
    /// Called after all URLs have been visited.
    fn analyze(&mut self, status: &crate::result::status::Status, output: &mut dyn crate::output::output::Output);

    /// Do your analysis for a just-visited URL.
    /// Body and headers are already downloaded and decompressed.
    /// Return None if you don't want to analyze this URL,
    /// otherwise return UrlAnalysisResult with your results.
    fn analyze_visited_url(
        &mut self,
        _visited_url: &VisitedUrl,
        _body: Option<&str>,
        _headers: Option<&HashMap<String, String>>,
    ) -> Option<UrlAnalysisResult> {
        None
    }

    /// If you want to show URL analysis results in table column,
    /// return the ExtraColumn under which results will be shown.
    fn show_analyzed_visited_url_result_as_column(&self) -> Option<ExtraColumn> {
        None
    }

    /// Should this analyzer be activated based on options?
    fn should_be_activated(&self) -> bool;

    /// Get order of this analyzer (lower = earlier).
    fn get_order(&self) -> i32;

    /// Get the name of this analyzer.
    fn get_name(&self) -> &str;

    /// Get execution times of analyzer methods.
    fn get_exec_times(&self) -> &HashMap<String, f64>;

    /// Get execution counts of analyzer methods.
    fn get_exec_counts(&self) -> &HashMap<String, usize>;
}
