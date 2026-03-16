// SiteOne Crawler - SlowestAnalyzer
// (c) Jan Reges <jan.reges@siteone.cz>

use std::collections::HashMap;

use crate::analysis::analyzer::Analyzer;
use crate::analysis::base_analyzer::BaseAnalyzer;
use crate::components::super_table::SuperTable;
use crate::components::super_table_column::SuperTableColumn;
use crate::output::output::Output;
use crate::result::status::Status;
use crate::types::ContentTypeId;
use crate::utils;

const SUPER_TABLE_SLOWEST_URLS: &str = "slowest-urls";

pub struct SlowestAnalyzer {
    base: BaseAnalyzer,
    slowest_top_limit: usize,
    slowest_min_time: f64,
    slowest_max_time: f64,
}

impl Default for SlowestAnalyzer {
    fn default() -> Self {
        Self::new()
    }
}

impl SlowestAnalyzer {
    pub fn new() -> Self {
        Self {
            base: BaseAnalyzer::new(),
            slowest_top_limit: 20,
            slowest_min_time: 0.01,
            slowest_max_time: 3.0,
        }
    }

    /// Set configuration from CoreOptions.
    pub fn set_config(&mut self, slowest_top_limit: usize, slowest_min_time: f64, slowest_max_time: f64) {
        self.slowest_top_limit = slowest_top_limit;
        self.slowest_min_time = slowest_min_time;
        self.slowest_max_time = slowest_max_time;
    }
}

impl Analyzer for SlowestAnalyzer {
    fn analyze(&mut self, status: &Status, output: &mut dyn Output) {
        let visited_urls = status.get_visited_urls();

        let mut slow_urls: Vec<_> = visited_urls
            .iter()
            .filter(|u| {
                u.is_allowed_for_crawling
                    && u.content_type == ContentTypeId::Html
                    && u.request_time >= self.slowest_min_time
            })
            .cloned()
            .collect();

        slow_urls.sort_by(|a, b| {
            b.request_time
                .partial_cmp(&a.request_time)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        slow_urls.truncate(self.slowest_top_limit);

        let console_width = utils::get_console_width();
        let url_column_width = (console_width as i32 - 25).max(20);

        let columns = vec![
            SuperTableColumn::new(
                "requestTime".to_string(),
                "Time".to_string(),
                6,
                Some(Box::new(|value: &str, _render_into: &str| {
                    if let Ok(v) = value.parse::<f64>() {
                        utils::get_colored_request_time(v, 6)
                    } else {
                        value.to_string()
                    }
                })),
                None,
                false,
                false,
                false,
                true,
                None,
            ),
            SuperTableColumn::new(
                "statusCode".to_string(),
                "Status".to_string(),
                6,
                Some(Box::new(|value: &str, _render_into: &str| {
                    if let Ok(v) = value.parse::<i32>() {
                        utils::get_colored_status_code(v, 6)
                    } else {
                        value.to_string()
                    }
                })),
                None,
                false,
                false,
                false,
                true,
                None,
            ),
            SuperTableColumn::new(
                "url".to_string(),
                "Slow URL".to_string(),
                url_column_width,
                None,
                None,
                true,
                true,
                false,
                true,
                None,
            ),
        ];

        let data: Vec<HashMap<String, String>> = slow_urls
            .iter()
            .map(|u| {
                let mut row = HashMap::new();
                row.insert("requestTime".to_string(), format!("{:.4}", u.request_time));
                row.insert("statusCode".to_string(), u.status_code.to_string());
                row.insert("url".to_string(), u.url.clone());
                row
            })
            .collect();

        let mut super_table = SuperTable::new(
            SUPER_TABLE_SLOWEST_URLS.to_string(),
            "TOP slowest URLs".to_string(),
            format!("No slow URLs slower than {} second(s) found.", self.slowest_min_time),
            columns,
            true,
            Some("requestTime".to_string()),
            "DESC".to_string(),
            None,
            None,
            None,
        );

        super_table.set_data(data);
        status.configure_super_table_url_stripping(&mut super_table);
        output.add_super_table(&super_table);
        status.add_super_table_at_beginning(super_table);

        // Summary for very slow URLs
        let very_slow_count = visited_urls
            .iter()
            .filter(|u| u.content_type == ContentTypeId::Html && u.request_time >= self.slowest_max_time)
            .count();

        status.add_summary_item_by_ranges(
            "slowUrls",
            very_slow_count as f64,
            &[(0.0, 0.0), (1.0, 2.0), (3.0, 5.0), (6.0, f64::MAX)],
            &[
                &format!(
                    "Performance OK - all non-media URLs are faster than {} seconds",
                    self.slowest_max_time
                ),
                &format!(
                    "Performance NOTICE - {{}} slow non-media URL(s) found (slower than {} seconds)",
                    self.slowest_max_time
                ),
                &format!(
                    "Performance WARNING - {{}} slow non-media URLs found (slower than {} seconds)",
                    self.slowest_max_time
                ),
                &format!(
                    "Performance CRITICAL - {{}} slow non-media URLs found (slower than {} seconds)",
                    self.slowest_max_time
                ),
            ],
        );
    }

    fn should_be_activated(&self) -> bool {
        true
    }

    fn get_order(&self) -> i32 {
        110
    }

    fn get_name(&self) -> &str {
        "SlowestAnalyzer"
    }

    fn get_exec_times(&self) -> &HashMap<String, f64> {
        self.base.get_exec_times()
    }

    fn get_exec_counts(&self) -> &HashMap<String, usize> {
        self.base.get_exec_counts()
    }
}
