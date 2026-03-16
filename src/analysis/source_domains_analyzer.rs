// SiteOne Crawler - SourceDomainsAnalyzer
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

const SUPER_TABLE_SOURCE_DOMAINS: &str = "source-domains";

pub struct SourceDomainsAnalyzer {
    base: BaseAnalyzer,
}

impl Default for SourceDomainsAnalyzer {
    fn default() -> Self {
        Self::new()
    }
}

impl SourceDomainsAnalyzer {
    pub fn new() -> Self {
        Self {
            base: BaseAnalyzer::new(),
        }
    }
}

impl Analyzer for SourceDomainsAnalyzer {
    fn analyze(&mut self, status: &Status, output: &mut dyn Output) {
        let visited_urls = status.get_visited_urls();
        let content_type_ids = get_all_content_type_ids();

        // Gather stats per domain and content type
        let mut stats: HashMap<String, HashMap<String, DomainContentTypeStat>> = HashMap::new();

        for visited_url in &visited_urls {
            if visited_url.has_error_status_code() {
                continue;
            }
            let url_host = visited_url.get_host().unwrap_or_else(|| "unknown".to_string());

            let host_stats = stats.entry(url_host.clone()).or_default();
            let content_type_id = visited_url.content_type;
            let key = format!("{:?}", content_type_id);

            let stat = host_stats.entry(key).or_insert_with(|| DomainContentTypeStat {
                count: 0,
                total_size: 0,
                total_exec_time: 0.0,
            });

            stat.count += 1;
            stat.total_size += visited_url.size.unwrap_or(0);
            stat.total_exec_time += visited_url.request_time;
        }

        // Convert stats to data rows
        let delimiter = utils::get_color_text("/", "dark-gray", false);
        let mut data: Vec<HashMap<String, String>> = Vec::new();
        let mut used_content_types: Vec<String> = Vec::new();

        for (domain, host_stats) in &stats {
            let mut row = HashMap::new();
            row.insert("domain".to_string(), domain.clone());

            let mut total_count: usize = 0;
            let mut total_size: i64 = 0;
            let mut total_time: f64 = 0.0;

            for ct_id in &content_type_ids {
                let key = format!("{:?}", ct_id);
                let ct_name = ct_id.name().to_string();

                if let Some(stat) = host_stats.get(&key) {
                    total_count += stat.count;
                    total_size += stat.total_size;
                    total_time += stat.total_exec_time;

                    let value = format!(
                        "{}/{}/{}",
                        stat.count,
                        utils::get_formatted_size(stat.total_size, 0).replace(' ', ""),
                        utils::get_formatted_duration(stat.total_exec_time).replace(' ', ""),
                    );
                    row.insert(ct_name.clone(), value);

                    if !used_content_types.contains(&ct_name) {
                        used_content_types.push(ct_name);
                    }
                } else {
                    row.insert(ct_name, String::new());
                }
            }

            row.insert(
                "totals".to_string(),
                format!(
                    "{}/{}/{}",
                    total_count,
                    utils::get_formatted_size(total_size, 0).replace(' ', ""),
                    utils::get_formatted_duration(total_time).replace(' ', ""),
                ),
            );
            row.insert("totalCount".to_string(), total_count.to_string());
            data.push(row);
        }

        // Build columns
        let mut columns = vec![
            SuperTableColumn::new(
                "domain".to_string(),
                "Domain".to_string(),
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
                "totals".to_string(),
                "Totals".to_string(),
                -1, // AUTO_WIDTH
                Some(Box::new({
                    let delim = delimiter.clone();
                    move |value: &str, render_into: &str| {
                        if render_into == "html" {
                            value.replace('/', &format!(" {} ", delim))
                        } else {
                            value.replace('/', &delim)
                        }
                    }
                })),
                None,
                false,
                false,
                false,
                true,
                None,
            ),
        ];

        for ct_name in &used_content_types {
            let delim = delimiter.clone();
            columns.push(SuperTableColumn::new(
                ct_name.clone(),
                ct_name.clone(),
                -1, // AUTO_WIDTH
                Some(Box::new(move |value: &str, render_into: &str| {
                    if render_into == "html" {
                        value.replace('/', &format!(" {} ", delim))
                    } else {
                        value.replace('/', &delim)
                    }
                })),
                None,
                false,
                false,
                false,
                true,
                None,
            ));
        }

        let mut super_table = SuperTable::new(
            SUPER_TABLE_SOURCE_DOMAINS.to_string(),
            "Source domains".to_string(),
            "No source domains found.".to_string(),
            columns,
            false,
            Some("totalCount".to_string()),
            "DESC".to_string(),
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
        205
    }

    fn get_name(&self) -> &str {
        "SourceDomainsAnalyzer"
    }

    fn get_exec_times(&self) -> &HashMap<String, f64> {
        self.base.get_exec_times()
    }

    fn get_exec_counts(&self) -> &HashMap<String, usize> {
        self.base.get_exec_counts()
    }
}

struct DomainContentTypeStat {
    count: usize,
    total_size: i64,
    total_exec_time: f64,
}

fn get_all_content_type_ids() -> Vec<ContentTypeId> {
    vec![
        ContentTypeId::Html,
        ContentTypeId::Image,
        ContentTypeId::Script,
        ContentTypeId::Stylesheet,
        ContentTypeId::Font,
        ContentTypeId::Document,
        ContentTypeId::Audio,
        ContentTypeId::Video,
        ContentTypeId::Json,
        ContentTypeId::Xml,
        ContentTypeId::Redirect,
        ContentTypeId::Other,
    ]
}
