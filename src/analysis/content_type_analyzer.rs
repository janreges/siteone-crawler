// SiteOne Crawler - ContentTypeAnalyzer
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

const SUPER_TABLE_CONTENT_TYPES: &str = "content-types";
const SUPER_TABLE_CONTENT_MIME_TYPES: &str = "content-types-raw";

pub struct ContentTypeAnalyzer {
    base: BaseAnalyzer,
}

impl Default for ContentTypeAnalyzer {
    fn default() -> Self {
        Self::new()
    }
}

impl ContentTypeAnalyzer {
    pub fn new() -> Self {
        Self {
            base: BaseAnalyzer::new(),
        }
    }

    fn add_content_type_super_table(&self, status: &Status, output: &mut dyn Output) {
        let visited_urls = status.get_visited_urls();
        let content_type_ids = get_all_content_type_ids();

        let mut stats: HashMap<String, ContentTypeStat> = HashMap::new();
        for ct_id in &content_type_ids {
            let key = format!("{:?}", ct_id);
            stats.insert(
                key,
                ContentTypeStat {
                    content_type_id: *ct_id,
                    content_type: ct_id.name().to_string(),
                    count: 0,
                    total_size: 0,
                    total_time: 0.0,
                    status_20x: 0,
                    status_30x: 0,
                    status_40x: 0,
                    status_42x: 0,
                    status_50x: 0,
                    status_other: 0,
                },
            );
        }

        for visited_url in &visited_urls {
            if visited_url.has_error_status_code() {
                continue;
            }
            let key = format!("{:?}", visited_url.content_type);
            if let Some(stat) = stats.get_mut(&key) {
                stat.count += 1;
                stat.total_size += visited_url.size.unwrap_or(0);
                stat.total_time += visited_url.request_time;

                let status_code = visited_url.status_code;
                if (200..300).contains(&status_code) {
                    stat.status_20x += 1;
                } else if (300..400).contains(&status_code) {
                    stat.status_30x += 1;
                } else if (400..420).contains(&status_code) {
                    stat.status_40x += 1;
                } else if (420..500).contains(&status_code) {
                    stat.status_42x += 1;
                } else if (500..600).contains(&status_code) {
                    stat.status_50x += 1;
                } else {
                    stat.status_other += 1;
                }
            }
        }

        // Remove empty stats and compute avg time
        let data: Vec<HashMap<String, String>> = stats
            .values()
            .filter(|s| s.count > 0)
            .map(|s| {
                let avg_time = s.total_time / s.count as f64;
                let mut row = HashMap::new();
                row.insert("contentType".to_string(), s.content_type.clone());
                row.insert("count".to_string(), s.count.to_string());
                row.insert("totalSize".to_string(), s.total_size.to_string());
                row.insert("totalTime".to_string(), format!("{:.4}", s.total_time));
                row.insert("avgTime".to_string(), format!("{:.4}", avg_time));
                row.insert("status20x".to_string(), s.status_20x.to_string());
                row.insert("status30x".to_string(), s.status_30x.to_string());
                row.insert("status40x".to_string(), s.status_40x.to_string());
                row.insert("status42x".to_string(), s.status_42x.to_string());
                row.insert("status50x".to_string(), s.status_50x.to_string());
                row.insert("statusOther".to_string(), s.status_other.to_string());
                row
            })
            .collect();

        let columns = build_content_type_columns();

        let mut super_table = SuperTable::new(
            SUPER_TABLE_CONTENT_TYPES.to_string(),
            "Content types".to_string(),
            "No URLs found.".to_string(),
            columns,
            true,
            Some("count".to_string()),
            "DESC".to_string(),
            None,
            None,
            None,
        );

        super_table.set_show_only_columns_with_values(true);
        super_table.set_data(data);
        status.configure_super_table_url_stripping(&mut super_table);
        output.add_super_table(&super_table);
        status.add_super_table_at_beginning(super_table);
    }

    fn add_content_type_raw_super_table(&self, status: &Status, output: &mut dyn Output) {
        let visited_urls = status.get_visited_urls();

        let mut stats: HashMap<String, MimeTypeStat> = HashMap::new();

        for visited_url in &visited_urls {
            if visited_url.has_error_status_code() {
                continue;
            }
            let key = visited_url
                .content_type_header
                .clone()
                .unwrap_or_else(|| "unknown".to_string());

            let stat = stats.entry(key.clone()).or_insert_with(|| MimeTypeStat {
                content_type: key,
                count: 0,
                total_size: 0,
                total_time: 0.0,
                status_20x: 0,
                status_30x: 0,
                status_40x: 0,
                status_42x: 0,
                status_50x: 0,
                status_other: 0,
            });

            stat.count += 1;
            stat.total_size += visited_url.size.unwrap_or(0);
            stat.total_time += visited_url.request_time;

            let status_code = visited_url.status_code;
            if (200..300).contains(&status_code) {
                stat.status_20x += 1;
            } else if (300..400).contains(&status_code) {
                stat.status_30x += 1;
            } else if (400..420).contains(&status_code) {
                stat.status_40x += 1;
            } else if (420..500).contains(&status_code) {
                stat.status_42x += 1;
            } else if (500..600).contains(&status_code) {
                stat.status_50x += 1;
            } else {
                stat.status_other += 1;
            }
        }

        let data: Vec<HashMap<String, String>> = stats
            .values()
            .map(|s| {
                let avg_time = if s.count > 0 {
                    s.total_time / s.count as f64
                } else {
                    0.0
                };
                let mut row = HashMap::new();
                row.insert("contentType".to_string(), s.content_type.clone());
                row.insert("count".to_string(), s.count.to_string());
                row.insert("totalSize".to_string(), s.total_size.to_string());
                row.insert("totalTime".to_string(), format!("{:.4}", s.total_time));
                row.insert("avgTime".to_string(), format!("{:.4}", avg_time));
                row.insert("status20x".to_string(), s.status_20x.to_string());
                row.insert("status30x".to_string(), s.status_30x.to_string());
                row.insert("status40x".to_string(), s.status_40x.to_string());
                row.insert("status42x".to_string(), s.status_42x.to_string());
                row.insert("status50x".to_string(), s.status_50x.to_string());
                row.insert("statusOther".to_string(), s.status_other.to_string());
                row
            })
            .collect();

        let mut columns = build_content_type_columns();
        // Adjust content type column width for MIME types
        if let Some(col) = columns.first_mut() {
            col.width = 26;
        }

        let mut super_table = SuperTable::new(
            SUPER_TABLE_CONTENT_MIME_TYPES.to_string(),
            "Content types (MIME types)".to_string(),
            "No MIME types found.".to_string(),
            columns,
            true,
            Some("count".to_string()),
            "DESC".to_string(),
            None,
            None,
            None,
        );

        super_table.set_show_only_columns_with_values(true);
        super_table.set_data(data);
        status.configure_super_table_url_stripping(&mut super_table);
        output.add_super_table(&super_table);
        status.add_super_table_at_beginning(super_table);
    }
}

impl Analyzer for ContentTypeAnalyzer {
    fn analyze(&mut self, status: &Status, output: &mut dyn Output) {
        self.add_content_type_super_table(status, output);
        self.add_content_type_raw_super_table(status, output);
    }

    fn should_be_activated(&self) -> bool {
        true
    }

    fn get_order(&self) -> i32 {
        210
    }

    fn get_name(&self) -> &str {
        "ContentTypeAnalyzer"
    }

    fn get_exec_times(&self) -> &HashMap<String, f64> {
        self.base.get_exec_times()
    }

    fn get_exec_counts(&self) -> &HashMap<String, usize> {
        self.base.get_exec_counts()
    }
}

struct ContentTypeStat {
    #[allow(dead_code)]
    content_type_id: ContentTypeId,
    content_type: String,
    count: usize,
    total_size: i64,
    total_time: f64,
    status_20x: usize,
    status_30x: usize,
    status_40x: usize,
    status_42x: usize,
    status_50x: usize,
    status_other: usize,
}

struct MimeTypeStat {
    content_type: String,
    count: usize,
    total_size: i64,
    total_time: f64,
    status_20x: usize,
    status_30x: usize,
    status_40x: usize,
    status_42x: usize,
    status_50x: usize,
    status_other: usize,
}

fn build_content_type_columns() -> Vec<SuperTableColumn> {
    vec![
        SuperTableColumn::new(
            "contentType".to_string(),
            "Content type".to_string(),
            12,
            None,
            None,
            false,
            false,
            false,
            true,
            None,
        ),
        SuperTableColumn::new(
            "count".to_string(),
            "URLs".to_string(),
            5,
            None,
            None,
            false,
            false,
            false,
            true,
            None,
        ),
        SuperTableColumn::new(
            "totalSize".to_string(),
            "Total size".to_string(),
            10,
            Some(Box::new(|value: &str, _render_into: &str| {
                if let Ok(v) = value.parse::<i64>() {
                    if v > 0 {
                        utils::get_formatted_size(v, 0)
                    } else {
                        "-".to_string()
                    }
                } else {
                    "-".to_string()
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
            "totalTime".to_string(),
            "Total time".to_string(),
            10,
            Some(Box::new(|value: &str, _render_into: &str| {
                if let Ok(v) = value.parse::<f64>() {
                    utils::get_formatted_duration(v)
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
            "avgTime".to_string(),
            "Avg time".to_string(),
            8,
            Some(Box::new(|value: &str, _render_into: &str| {
                if let Ok(v) = value.parse::<f64>() {
                    utils::get_colored_request_time(v, 8)
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
            "status20x".to_string(),
            "Status 20x".to_string(),
            10,
            Some(Box::new(|value: &str, _render_into: &str| {
                if let Ok(v) = value.parse::<i32>() {
                    if v > 0 {
                        utils::get_color_text(&format!("{:<10}", v), "green", false)
                    } else {
                        value.to_string()
                    }
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
            "status30x".to_string(),
            "Status 30x".to_string(),
            10,
            Some(Box::new(|value: &str, _render_into: &str| {
                if let Ok(v) = value.parse::<i32>() {
                    if v > 0 {
                        utils::get_color_text(&format!("{:<10}", v), "yellow", true)
                    } else {
                        value.to_string()
                    }
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
            "status40x".to_string(),
            "Status 40x".to_string(),
            10,
            Some(Box::new(|value: &str, _render_into: &str| {
                if let Ok(v) = value.parse::<i32>() {
                    if v > 0 {
                        utils::get_color_text(&format!("{:<10}", v), "magenta", true)
                    } else {
                        value.to_string()
                    }
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
            "status42x".to_string(),
            "Status 42x".to_string(),
            10,
            Some(Box::new(|value: &str, _render_into: &str| {
                if let Ok(v) = value.parse::<i32>() {
                    if v > 0 {
                        utils::get_color_text(&format!("{:<10}", v), "magenta", true)
                    } else {
                        value.to_string()
                    }
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
            "status50x".to_string(),
            "Status 50x".to_string(),
            10,
            Some(Box::new(|value: &str, _render_into: &str| {
                if let Ok(v) = value.parse::<i32>() {
                    if v > 0 {
                        utils::get_color_text(&format!("{:<10}", v), "red", true)
                    } else {
                        value.to_string()
                    }
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
            "statusOther".to_string(),
            "Status ERR".to_string(),
            10,
            Some(Box::new(|value: &str, _render_into: &str| {
                if let Ok(v) = value.parse::<i32>() {
                    if v > 0 {
                        utils::get_color_text(&format!("{:<10}", v), "red", true)
                    } else {
                        value.to_string()
                    }
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
    ]
}

fn get_all_content_type_ids() -> Vec<ContentTypeId> {
    vec![
        ContentTypeId::Html,
        ContentTypeId::Script,
        ContentTypeId::Stylesheet,
        ContentTypeId::Image,
        ContentTypeId::Video,
        ContentTypeId::Audio,
        ContentTypeId::Font,
        ContentTypeId::Document,
        ContentTypeId::Json,
        ContentTypeId::Xml,
        ContentTypeId::Redirect,
        ContentTypeId::Other,
    ]
}
