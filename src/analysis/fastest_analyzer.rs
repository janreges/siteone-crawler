// SiteOne Crawler - FastestAnalyzer
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

const SUPER_TABLE_FASTEST_URLS: &str = "fastest-urls";

pub struct FastestAnalyzer {
    base: BaseAnalyzer,
    fastest_top_limit: usize,
    fastest_max_time: f64,
}

impl Default for FastestAnalyzer {
    fn default() -> Self {
        Self::new()
    }
}

impl FastestAnalyzer {
    pub fn new() -> Self {
        Self {
            base: BaseAnalyzer::new(),
            fastest_top_limit: 20,
            fastest_max_time: 1.0,
        }
    }

    /// Set configuration from CoreOptions.
    pub fn set_config(&mut self, fastest_top_limit: usize, fastest_max_time: f64) {
        self.fastest_top_limit = fastest_top_limit;
        self.fastest_max_time = fastest_max_time;
    }
}

impl Analyzer for FastestAnalyzer {
    fn analyze(&mut self, status: &Status, output: &mut dyn Output) {
        let visited_urls = status.get_visited_urls();

        let mut fast_urls: Vec<_> = visited_urls
            .into_iter()
            .filter(|u| {
                u.status_code == 200
                    && u.is_allowed_for_crawling
                    && u.content_type == ContentTypeId::Html
                    && u.request_time <= self.fastest_max_time
            })
            .collect();

        fast_urls.sort_by(|a, b| {
            a.request_time
                .partial_cmp(&b.request_time)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        fast_urls.truncate(self.fastest_top_limit);

        let console_width = utils::get_console_width();
        let url_column_width = (console_width as i32 - 20).max(20);

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
                "Fast URL".to_string(),
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

        let data: Vec<HashMap<String, String>> = fast_urls
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
            SUPER_TABLE_FASTEST_URLS.to_string(),
            "TOP fastest URLs".to_string(),
            format!("No fast URLs faster than {} second(s) found.", self.fastest_max_time),
            columns,
            true,
            Some("requestTime".to_string()),
            "ASC".to_string(),
            None,
            None,
            None,
        );

        super_table.set_data(data);
        status.configure_super_table_url_stripping(&mut super_table);
        output.add_super_table(&super_table);
        status.add_super_table_at_beginning(super_table);
    }

    fn should_be_activated(&self) -> bool {
        true
    }

    fn get_order(&self) -> i32 {
        100
    }

    fn get_name(&self) -> &str {
        "FastestAnalyzer"
    }

    fn get_exec_times(&self) -> &HashMap<String, f64> {
        self.base.get_exec_times()
    }

    fn get_exec_counts(&self) -> &HashMap<String, usize> {
        self.base.get_exec_counts()
    }
}
