// SiteOne Crawler - Page404Analyzer
// (c) Jan Reges <jan.reges@siteone.cz>

use std::collections::HashMap;

use crate::analysis::analyzer::Analyzer;
use crate::analysis::base_analyzer::BaseAnalyzer;
use crate::components::super_table::SuperTable;
use crate::components::super_table_column::SuperTableColumn;
use crate::output::output::Output;
use crate::result::status::Status;
use crate::utils;

const SUPER_TABLE_404: &str = "404";

pub struct Page404Analyzer {
    base: BaseAnalyzer,
}

impl Default for Page404Analyzer {
    fn default() -> Self {
        Self::new()
    }
}

impl Page404Analyzer {
    pub fn new() -> Self {
        Self {
            base: BaseAnalyzer::new(),
        }
    }
}

impl Analyzer for Page404Analyzer {
    fn analyze(&mut self, status: &Status, output: &mut dyn Output) {
        let visited_urls = status.get_visited_urls();

        let urls_404: Vec<_> = visited_urls.iter().filter(|u| u.status_code == 404).cloned().collect();

        let console_width = utils::get_console_width();
        let url_column_size = ((console_width as i32 - 16) / 2).max(20);

        let status_ref = status;
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
                "URL 404".to_string(),
                url_column_size,
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
                url_column_size,
                None,
                None,
                true,
                true,
                false,
                true,
                None,
            ),
        ];

        let data: Vec<HashMap<String, String>> = urls_404
            .iter()
            .map(|u| {
                let mut row = HashMap::new();
                row.insert("statusCode".to_string(), u.status_code.to_string());
                row.insert("url".to_string(), u.url.clone());
                let source_url = if !u.source_uq_id.is_empty() {
                    status_ref.get_url_by_uq_id(&u.source_uq_id).unwrap_or_default()
                } else {
                    String::new()
                };
                row.insert("sourceUqId".to_string(), source_url);
                row
            })
            .collect();

        let count_404 = data.len();

        let mut super_table = SuperTable::new(
            SUPER_TABLE_404.to_string(),
            "404 URLs".to_string(),
            "No 404 URLs found.".to_string(),
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
            "404",
            count_404 as f64,
            &[(0.0, 0.0), (1.0, 2.0), (3.0, 5.0), (6.0, f64::MAX)],
            &[
                "404 OK - all pages exists, no non-existent pages found",
                "404 NOTICE - {} non-existent page(s) found",
                "404 WARNING - {} non-existent pages found",
                "404 CRITICAL - {} non-existent pages found",
            ],
        );
    }

    fn should_be_activated(&self) -> bool {
        true
    }

    fn get_order(&self) -> i32 {
        20
    }

    fn get_name(&self) -> &str {
        "Page404Analyzer"
    }

    fn get_exec_times(&self) -> &HashMap<String, f64> {
        self.base.get_exec_times()
    }

    fn get_exec_counts(&self) -> &HashMap<String, usize> {
        self.base.get_exec_counts()
    }
}
