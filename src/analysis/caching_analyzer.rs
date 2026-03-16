// SiteOne Crawler - CachingAnalyzer
// (c) Jan Reges <jan.reges@siteone.cz>

use std::collections::HashMap;

use crate::analysis::analyzer::Analyzer;
use crate::analysis::base_analyzer::BaseAnalyzer;
use crate::components::super_table::SuperTable;
use crate::components::super_table_column::SuperTableColumn;
use crate::output::output::Output;
use crate::result::status::Status;
use crate::result::visited_url::VisitedUrl;
use crate::utils;

const SUPER_TABLE_CACHING_PER_CONTENT_TYPE: &str = "caching-per-content-type";
const SUPER_TABLE_CACHING_PER_DOMAIN: &str = "caching-per-domain";
const SUPER_TABLE_CACHING_PER_DOMAIN_AND_CONTENT_TYPE: &str = "caching-per-domain-and-content-type";

pub struct CachingAnalyzer {
    base: BaseAnalyzer,
}

impl Default for CachingAnalyzer {
    fn default() -> Self {
        Self::new()
    }
}

impl CachingAnalyzer {
    pub fn new() -> Self {
        Self {
            base: BaseAnalyzer::new(),
        }
    }

    fn update_cache_stat(stat: &mut CacheStat, visited_url: &VisitedUrl) {
        stat.count += 1;
        if let Some(lifetime) = visited_url.cache_lifetime {
            stat.count_with_lifetime += 1;
            stat.total_lifetime += lifetime;
            stat.avg_lifetime = Some(stat.total_lifetime as f64 / stat.count_with_lifetime as f64);
            stat.min_lifetime = Some(match stat.min_lifetime {
                Some(min) => min.min(lifetime),
                None => lifetime,
            });
            stat.max_lifetime = Some(match stat.max_lifetime {
                Some(max) => max.max(lifetime),
                None => lifetime,
            });
        }
    }

    fn build_lifetime_columns(first_col_name: &str, first_col_key: &str) -> Vec<SuperTableColumn> {
        let mut columns = vec![SuperTableColumn::new(
            first_col_key.to_string(),
            first_col_name.to_string(),
            if first_col_key == "domain" { 20 } else { 12 },
            None,
            None,
            false,
            false,
            false,
            true,
            None,
        )];

        // Add cacheType column only when not the first column
        if first_col_key != "cacheType" {
            columns.push(SuperTableColumn::new(
                "cacheType".to_string(),
                "Cache type".to_string(),
                12,
                None,
                None,
                false,
                false,
                false,
                true,
                None,
            ));
        }

        columns.extend(vec![
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
                "avgLifetime".to_string(),
                "AVG lifetime".to_string(),
                10,
                Some(Box::new(|value: &str, _render_into: &str| {
                    if let Ok(v) = value.parse::<i64>() {
                        utils::get_colored_cache_lifetime(v, 6)
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
                "minLifetime".to_string(),
                "MIN lifetime".to_string(),
                10,
                Some(Box::new(|value: &str, _render_into: &str| {
                    if let Ok(v) = value.parse::<i64>() {
                        utils::get_colored_cache_lifetime(v, 6)
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
                "maxLifetime".to_string(),
                "MAX lifetime".to_string(),
                10,
                Some(Box::new(|value: &str, _render_into: &str| {
                    if let Ok(v) = value.parse::<i64>() {
                        utils::get_colored_cache_lifetime(v, 6)
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
        ]);

        columns
    }
}

impl Analyzer for CachingAnalyzer {
    fn analyze(&mut self, status: &Status, output: &mut dyn Output) {
        let visited_urls = status.get_visited_urls();

        let mut stats_per_content_type: HashMap<String, CacheStatWithType> = HashMap::new();
        let mut stats_per_domain: HashMap<String, CacheStatWithDomain> = HashMap::new();
        let mut stats_per_domain_and_ct: HashMap<String, CacheStatWithDomainAndType> = HashMap::new();

        for visited_url in &visited_urls {
            let content_type_name = visited_url.content_type.name().to_string();
            let cache_type_label = visited_url.get_cache_type_label();
            let domain_name = visited_url.get_host().unwrap_or_else(|| "unknown".to_string());

            // Per domain
            {
                let key = format!("{}.{}", domain_name, cache_type_label);
                let stat = stats_per_domain.entry(key).or_insert_with(|| CacheStatWithDomain {
                    domain: domain_name.clone(),
                    cache_type: cache_type_label.clone(),
                    stat: CacheStat::default(),
                });
                Self::update_cache_stat(&mut stat.stat, visited_url);
            }

            // Per domain and content type
            {
                let key = format!("{}.{}.{}", domain_name, content_type_name, cache_type_label);
                let stat = stats_per_domain_and_ct
                    .entry(key)
                    .or_insert_with(|| CacheStatWithDomainAndType {
                        domain: domain_name.clone(),
                        content_type: content_type_name.clone(),
                        cache_type: cache_type_label.clone(),
                        stat: CacheStat::default(),
                    });
                Self::update_cache_stat(&mut stat.stat, visited_url);
            }

            // Per content type (only crawlable domains)
            if visited_url.is_allowed_for_crawling {
                let key = format!("{}.{}", content_type_name, cache_type_label);
                let stat = stats_per_content_type.entry(key).or_insert_with(|| CacheStatWithType {
                    content_type: content_type_name.clone(),
                    cache_type: cache_type_label.clone(),
                    stat: CacheStat::default(),
                });
                Self::update_cache_stat(&mut stat.stat, visited_url);
            }
        }

        // Per content type table
        if !stats_per_content_type.is_empty() {
            let data: Vec<HashMap<String, String>> = stats_per_content_type.values().map(|s| s.to_row()).collect();

            let columns = Self::build_lifetime_columns("Content type", "contentType");

            let mut super_table = SuperTable::new(
                SUPER_TABLE_CACHING_PER_CONTENT_TYPE.to_string(),
                "HTTP Caching by content type (only from crawlable domains)".to_string(),
                "No URLs found.".to_string(),
                columns,
                true,
                Some("count".to_string()),
                "DESC".to_string(),
                None,
                None,
                Some("HTTP cache".to_string()),
            );

            super_table.set_data(data);
            status.configure_super_table_url_stripping(&mut super_table);
            output.add_super_table(&super_table);
            status.add_super_table_at_beginning(super_table);
        }

        // Per domain table
        {
            let data: Vec<HashMap<String, String>> = stats_per_domain.values().map(|s| s.to_row()).collect();

            let columns = Self::build_lifetime_columns("Domain", "domain");

            let mut super_table = SuperTable::new(
                SUPER_TABLE_CACHING_PER_DOMAIN.to_string(),
                "HTTP Caching by domain".to_string(),
                "No URLs found.".to_string(),
                columns,
                true,
                Some("count".to_string()),
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

        // Per domain and content type table
        {
            let data: Vec<HashMap<String, String>> = stats_per_domain_and_ct.values().map(|s| s.to_row()).collect();

            let mut columns = Self::build_lifetime_columns("Domain", "domain");
            columns.insert(
                1,
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
            );

            let mut super_table = SuperTable::new(
                SUPER_TABLE_CACHING_PER_DOMAIN_AND_CONTENT_TYPE.to_string(),
                "HTTP Caching by domain and content type".to_string(),
                "No URLs found.".to_string(),
                columns,
                true,
                Some("count".to_string()),
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
    }

    fn should_be_activated(&self) -> bool {
        true
    }

    fn get_order(&self) -> i32 {
        116
    }

    fn get_name(&self) -> &str {
        "CachingAnalyzer"
    }

    fn get_exec_times(&self) -> &HashMap<String, f64> {
        self.base.get_exec_times()
    }

    fn get_exec_counts(&self) -> &HashMap<String, usize> {
        self.base.get_exec_counts()
    }
}

#[derive(Default)]
struct CacheStat {
    count: usize,
    count_with_lifetime: usize,
    total_lifetime: i64,
    avg_lifetime: Option<f64>,
    min_lifetime: Option<i64>,
    max_lifetime: Option<i64>,
}

struct CacheStatWithType {
    content_type: String,
    cache_type: String,
    stat: CacheStat,
}

impl CacheStatWithType {
    fn to_row(&self) -> HashMap<String, String> {
        let mut row = HashMap::new();
        row.insert("contentType".to_string(), self.content_type.clone());
        row.insert("cacheType".to_string(), self.cache_type.clone());
        row.insert("count".to_string(), self.stat.count.to_string());
        row.insert(
            "avgLifetime".to_string(),
            self.stat
                .avg_lifetime
                .map(|v| format!("{}", v as i64))
                .unwrap_or_default(),
        );
        row.insert(
            "minLifetime".to_string(),
            self.stat.min_lifetime.map(|v| v.to_string()).unwrap_or_default(),
        );
        row.insert(
            "maxLifetime".to_string(),
            self.stat.max_lifetime.map(|v| v.to_string()).unwrap_or_default(),
        );
        row
    }
}

struct CacheStatWithDomain {
    domain: String,
    cache_type: String,
    stat: CacheStat,
}

impl CacheStatWithDomain {
    fn to_row(&self) -> HashMap<String, String> {
        let mut row = HashMap::new();
        row.insert("domain".to_string(), self.domain.clone());
        row.insert("cacheType".to_string(), self.cache_type.clone());
        row.insert("count".to_string(), self.stat.count.to_string());
        row.insert(
            "avgLifetime".to_string(),
            self.stat
                .avg_lifetime
                .map(|v| format!("{}", v as i64))
                .unwrap_or_default(),
        );
        row.insert(
            "minLifetime".to_string(),
            self.stat.min_lifetime.map(|v| v.to_string()).unwrap_or_default(),
        );
        row.insert(
            "maxLifetime".to_string(),
            self.stat.max_lifetime.map(|v| v.to_string()).unwrap_or_default(),
        );
        row
    }
}

struct CacheStatWithDomainAndType {
    domain: String,
    content_type: String,
    cache_type: String,
    stat: CacheStat,
}

impl CacheStatWithDomainAndType {
    fn to_row(&self) -> HashMap<String, String> {
        let mut row = HashMap::new();
        row.insert("domain".to_string(), self.domain.clone());
        row.insert("contentType".to_string(), self.content_type.clone());
        row.insert("cacheType".to_string(), self.cache_type.clone());
        row.insert("count".to_string(), self.stat.count.to_string());
        row.insert(
            "avgLifetime".to_string(),
            self.stat
                .avg_lifetime
                .map(|v| format!("{}", v as i64))
                .unwrap_or_default(),
        );
        row.insert(
            "minLifetime".to_string(),
            self.stat.min_lifetime.map(|v| v.to_string()).unwrap_or_default(),
        );
        row.insert(
            "maxLifetime".to_string(),
            self.stat.max_lifetime.map(|v| v.to_string()).unwrap_or_default(),
        );
        row
    }
}
