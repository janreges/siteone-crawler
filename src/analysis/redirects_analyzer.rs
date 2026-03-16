// SiteOne Crawler - RedirectsAnalyzer
// (c) Jan Reges <jan.reges@siteone.cz>

use std::collections::HashMap;

use crate::analysis::analyzer::Analyzer;
use crate::analysis::base_analyzer::BaseAnalyzer;
use crate::components::super_table::SuperTable;
use crate::components::super_table_column::SuperTableColumn;
use crate::output::output::Output;
use crate::result::status::Status;
use crate::utils;

const SUPER_TABLE_REDIRECTS: &str = "redirects";

pub struct RedirectsAnalyzer {
    base: BaseAnalyzer,
}

impl Default for RedirectsAnalyzer {
    fn default() -> Self {
        Self::new()
    }
}

impl RedirectsAnalyzer {
    pub fn new() -> Self {
        Self {
            base: BaseAnalyzer::new(),
        }
    }
}

impl Analyzer for RedirectsAnalyzer {
    fn analyze(&mut self, status: &Status, output: &mut dyn Output) {
        let visited_urls = status.get_visited_urls();

        let url_redirects: Vec<_> = visited_urls
            .iter()
            .filter(|u| u.status_code >= 301 && u.status_code <= 308)
            .cloned()
            .collect();

        let console_width = utils::get_console_width();
        let url_column_width = ((console_width as i32 - 20) / 3).max(20);

        let columns = vec![
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
                "Redirected URL".to_string(),
                url_column_width,
                None,
                None,
                true,
                true,
                false,
                true,
                None,
            ),
            SuperTableColumn::new(
                "targetUrl".to_string(),
                "Target URL".to_string(),
                url_column_width,
                None,
                None,
                true,
                true,
                false,
                true,
                None,
            ),
            SuperTableColumn::new(
                "sourceUqId".to_string(),
                "Found at URL".to_string(),
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

        let data: Vec<HashMap<String, String>> = url_redirects
            .iter()
            .map(|u| {
                let mut row = HashMap::new();
                row.insert("statusCode".to_string(), u.status_code.to_string());
                row.insert("url".to_string(), u.url.clone());
                // Target URL from the Location header in extras
                let target = u
                    .extras
                    .as_ref()
                    .and_then(|e| e.get("Location"))
                    .cloned()
                    .unwrap_or_else(|| "?".to_string());
                row.insert("targetUrl".to_string(), target);
                let source_url = if !u.source_uq_id.is_empty() {
                    status.get_url_by_uq_id(&u.source_uq_id).unwrap_or_default()
                } else {
                    String::new()
                };
                row.insert("sourceUqId".to_string(), source_url);
                row
            })
            .collect();

        let count_redirects = data.len();

        let mut super_table = SuperTable::new(
            SUPER_TABLE_REDIRECTS.to_string(),
            "Redirected URLs".to_string(),
            "No redirects found.".to_string(),
            columns,
            true,
            Some("url".to_string()),
            "ASC".to_string(),
            None,
            None,
            None,
        );

        super_table.set_data(data);
        status.configure_super_table_url_stripping(&mut super_table);
        output.add_super_table(&super_table);
        status.add_super_table_at_beginning(super_table);

        status.add_summary_item_by_ranges(
            "redirects",
            count_redirects as f64,
            &[(0.0, 0.0), (1.0, 2.0), (3.0, 9.0), (10.0, f64::MAX)],
            &[
                "Redirects - no redirects found",
                "Redirects - {} redirect(s) found",
                "Redirects - {} redirects found",
                "Redirects - {} redirects found",
            ],
        );
    }

    fn should_be_activated(&self) -> bool {
        true
    }

    fn get_order(&self) -> i32 {
        10
    }

    fn get_name(&self) -> &str {
        "RedirectsAnalyzer"
    }

    fn get_exec_times(&self) -> &HashMap<String, f64> {
        self.base.get_exec_times()
    }

    fn get_exec_counts(&self) -> &HashMap<String, usize> {
        self.base.get_exec_counts()
    }
}
