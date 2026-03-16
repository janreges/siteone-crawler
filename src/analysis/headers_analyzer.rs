// SiteOne Crawler - HeadersAnalyzer
// (c) Jan Reges <jan.reges@siteone.cz>

use std::collections::HashMap;

use crate::analysis::analyzer::Analyzer;
use crate::analysis::base_analyzer::BaseAnalyzer;
use crate::analysis::result::header_stats::HeaderStats;
use crate::analysis::result::url_analysis_result::UrlAnalysisResult;
use crate::components::super_table::SuperTable;
use crate::components::super_table_column::SuperTableColumn;
use crate::output::output::Output;
use crate::result::status::Status;
use crate::result::visited_url::VisitedUrl;
use crate::utils;

const SUPER_TABLE_HEADERS: &str = "headers";
const SUPER_TABLE_HEADERS_VALUES: &str = "headers-values";

pub struct HeadersAnalyzer {
    base: BaseAnalyzer,
    header_stats: HashMap<String, HeaderStats>,
}

impl Default for HeadersAnalyzer {
    fn default() -> Self {
        Self::new()
    }
}

impl HeadersAnalyzer {
    pub fn new() -> Self {
        Self {
            base: BaseAnalyzer::new(),
            header_stats: HashMap::new(),
        }
    }
}

impl Analyzer for HeadersAnalyzer {
    fn analyze(&mut self, status: &Status, output: &mut dyn Output) {
        let console_width = utils::get_console_width();

        // Basic header stats table
        let data: Vec<HashMap<String, String>> = self
            .header_stats
            .values()
            .map(|hs| {
                let mut row = HashMap::new();
                row.insert("header".to_string(), hs.get_formatted_header_name());
                row.insert("occurrences".to_string(), hs.occurrences.to_string());

                let unique_count = hs.unique_values.len();
                let unique_str = if unique_count == 0 {
                    "-".to_string()
                } else if hs.unique_values_limit_reached {
                    format!("{}+", unique_count)
                } else {
                    unique_count.to_string()
                };
                row.insert("uniqueValues".to_string(), unique_str);

                row.insert("valuesPreview".to_string(), hs.get_values_preview(120));

                let min_value = hs.get_min_value().unwrap_or_default();
                let max_value = hs.get_max_value().unwrap_or_default();

                // Format min/max for content-length and age
                if hs.header == "content-length" {
                    if let Some(min_int) = hs.min_int_value {
                        row.insert("minValue".to_string(), utils::get_formatted_size(min_int, 0));
                    } else {
                        row.insert("minValue".to_string(), String::new());
                    }
                    if let Some(max_int) = hs.max_int_value {
                        row.insert("maxValue".to_string(), utils::get_formatted_size(max_int, 0));
                    } else {
                        row.insert("maxValue".to_string(), String::new());
                    }
                } else if hs.header == "age" {
                    if let Some(min_int) = hs.min_int_value {
                        row.insert("minValue".to_string(), utils::get_formatted_age(min_int));
                    } else {
                        row.insert("minValue".to_string(), String::new());
                    }
                    if let Some(max_int) = hs.max_int_value {
                        row.insert("maxValue".to_string(), utils::get_formatted_age(max_int));
                    } else {
                        row.insert("maxValue".to_string(), String::new());
                    }
                } else {
                    row.insert("minValue".to_string(), min_value);
                    row.insert("maxValue".to_string(), max_value);
                }

                row
            })
            .collect();

        let columns = vec![
            SuperTableColumn::new(
                "header".to_string(),
                "Header".to_string(),
                -1, // AUTO_WIDTH
                None,
                None,
                false,
                false,
                false,
                true,
                None,
            ),
            SuperTableColumn::new(
                "occurrences".to_string(),
                "Occurs".to_string(),
                6,
                None,
                None,
                false,
                false,
                false,
                true,
                None,
            ),
            SuperTableColumn::new(
                "uniqueValues".to_string(),
                "Unique".to_string(),
                6,
                None,
                None,
                false,
                false,
                false,
                true,
                None,
            ),
            SuperTableColumn::new(
                "valuesPreview".to_string(),
                "Values preview".to_string(),
                (console_width as i32 - 90).max(20),
                None,
                None,
                true,
                true,
                false,
                false,
                None,
            ),
            SuperTableColumn::new(
                "minValue".to_string(),
                "Min value".to_string(),
                10,
                None,
                None,
                false,
                false,
                false,
                true,
                None,
            ),
            SuperTableColumn::new(
                "maxValue".to_string(),
                "Max value".to_string(),
                10,
                None,
                None,
                false,
                false,
                false,
                true,
                None,
            ),
        ];

        let mut super_table = SuperTable::new(
            SUPER_TABLE_HEADERS.to_string(),
            "HTTP headers".to_string(),
            "No HTTP headers found.".to_string(),
            columns,
            true,
            Some("header".to_string()),
            "ASC".to_string(),
            None,
            None,
            None,
        );

        super_table.set_data(data);
        status.configure_super_table_url_stripping(&mut super_table);
        output.add_super_table(&super_table);
        status.add_super_table_at_end(super_table);

        let unique_count = self.header_stats.len();
        status.add_summary_item_by_ranges(
            "unique-headers",
            unique_count as f64,
            &[(0.0, 30.0), (31.0, 40.0), (41.0, 50.0), (51.0, f64::MAX)],
            &[
                "HTTP headers - found {} unique headers",
                "HTTP headers - found {} unique headers",
                "HTTP headers - found {} unique headers (too many)",
                "HTTP headers - found {} unique headers (too many)",
            ],
        );

        // Detail info with header values
        let mut details: Vec<HashMap<String, String>> = Vec::new();
        for header_stat in self.header_stats.values() {
            for (value, count) in &header_stat.unique_values {
                let mut row = HashMap::new();
                row.insert("header".to_string(), header_stat.get_formatted_header_name());
                row.insert("occurrences".to_string(), count.to_string());
                row.insert("value".to_string(), value.clone());
                details.push(row);
            }
        }

        // Sort by header asc, then by occurrences desc
        details.sort_by(|a, b| {
            let header_a = a.get("header").cloned().unwrap_or_default();
            let header_b = b.get("header").cloned().unwrap_or_default();
            if header_a == header_b {
                let occ_a = a.get("occurrences").and_then(|v| v.parse::<usize>().ok()).unwrap_or(0);
                let occ_b = b.get("occurrences").and_then(|v| v.parse::<usize>().ok()).unwrap_or(0);
                occ_b.cmp(&occ_a)
            } else {
                header_a.cmp(&header_b)
            }
        });

        let detail_columns = vec![
            SuperTableColumn::new(
                "header".to_string(),
                "Header".to_string(),
                -1, // AUTO_WIDTH
                None,
                None,
                false,
                false,
                false,
                true,
                None,
            ),
            SuperTableColumn::new(
                "occurrences".to_string(),
                "Occurs".to_string(),
                6,
                None,
                None,
                false,
                false,
                false,
                true,
                None,
            ),
            SuperTableColumn::new(
                "value".to_string(),
                "Value".to_string(),
                (console_width as i32 - 56).max(20),
                None,
                None,
                true,
                true,
                false,
                true,
                None,
            ),
        ];

        let mut detail_table = SuperTable::new(
            SUPER_TABLE_HEADERS_VALUES.to_string(),
            "HTTP header values".to_string(),
            "No HTTP headers found.".to_string(),
            detail_columns,
            true,
            None,
            "ASC".to_string(),
            None,
            None,
            None,
        );

        detail_table.set_data(details);
        status.configure_super_table_url_stripping(&mut detail_table);
        output.add_super_table(&detail_table);
        status.add_super_table_at_end(detail_table);
    }

    fn analyze_visited_url(
        &mut self,
        visited_url: &VisitedUrl,
        _body: Option<&str>,
        headers: Option<&HashMap<String, String>>,
    ) -> Option<UrlAnalysisResult> {
        let headers = headers?;
        if !visited_url.is_allowed_for_crawling {
            return None;
        }

        for (header, values) in headers {
            let header_lower = header.to_lowercase();
            let stat = self
                .header_stats
                .entry(header_lower.clone())
                .or_insert_with(|| HeaderStats::new(header_lower));

            stat.add_value(values);
        }

        None
    }

    fn should_be_activated(&self) -> bool {
        true
    }

    fn get_order(&self) -> i32 {
        115
    }

    fn get_name(&self) -> &str {
        "HeadersAnalyzer"
    }

    fn get_exec_times(&self) -> &HashMap<String, f64> {
        self.base.get_exec_times()
    }

    fn get_exec_counts(&self) -> &HashMap<String, usize> {
        self.base.get_exec_counts()
    }
}
