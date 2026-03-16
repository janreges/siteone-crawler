// SiteOne Crawler - SkippedUrlsAnalyzer
// (c) Jan Reges <jan.reges@siteone.cz>

use std::collections::HashMap;

use crate::analysis::analyzer::Analyzer;
use crate::analysis::base_analyzer::BaseAnalyzer;
use crate::components::super_table::SuperTable;
use crate::components::super_table_column::SuperTableColumn;
use crate::output::output::Output;
use crate::result::status::Status;
use crate::result::visited_url::VisitedUrl;
use crate::types::SkippedReason;

const SUPER_TABLE_SKIPPED_SUMMARY: &str = "skipped-summary";
const SUPER_TABLE_SKIPPED: &str = "skipped";

pub struct SkippedUrlsAnalyzer {
    base: BaseAnalyzer,
}

impl Default for SkippedUrlsAnalyzer {
    fn default() -> Self {
        Self::new()
    }
}

impl SkippedUrlsAnalyzer {
    pub fn new() -> Self {
        Self {
            base: BaseAnalyzer::new(),
        }
    }

    fn get_reason_label(reason: &SkippedReason) -> &'static str {
        match reason {
            SkippedReason::NotAllowedHost => "Not allowed host",
            SkippedReason::RobotsTxt => "Robots.txt",
            SkippedReason::ExceedsMaxDepth => "Max depth",
        }
    }

    fn get_source_short_name(source_attr: i32) -> &'static str {
        match source_attr {
            5 => "Initial URL",
            10 => "<a href>",
            20 => "<img src>",
            21 => "<img srcset>",
            22 => "<input src>",
            23 => "<source src>",
            24 => "<video src>",
            25 => "<audio src>",
            30 => "<script src>",
            40 => "inline <script src>",
            50 => "<link href>",
            60 => "css url()",
            70 => "js url",
            80 => "redirect",
            90 => "sitemap",
            _ => "unknown",
        }
    }
}

impl Analyzer for SkippedUrlsAnalyzer {
    fn analyze(&mut self, status: &Status, output: &mut dyn Output) {
        let skipped_entries = status.get_skipped_urls();

        // Get initial host and scheme from the first visited URL
        let visited = status.get_visited_urls();
        let (initial_host, initial_scheme) = visited
            .first()
            .and_then(|v| url::Url::parse(&v.url).ok())
            .map(|parsed| {
                (
                    Some(parsed.host_str().unwrap_or("").to_string()),
                    Some(parsed.scheme().to_string()),
                )
            })
            .unwrap_or((None, None));

        // Build summary: group by reason + domain
        let mut summary_map: HashMap<(String, String), usize> = HashMap::new();
        for entry in &skipped_entries {
            let reason_label = Self::get_reason_label(&entry.reason).to_string();
            let domain = url::Url::parse(&entry.url)
                .ok()
                .and_then(|u| u.host_str().map(|h| h.to_string()))
                .unwrap_or_else(|| {
                    // For relative URLs, extract domain from path
                    let visited = status.get_visited_urls();
                    visited
                        .first()
                        .and_then(|v| v.get_host())
                        .unwrap_or_else(|| "unknown".to_string())
                });
            *summary_map.entry((reason_label, domain)).or_insert(0) += 1;
        }

        let mut skipped_urls_summary: Vec<HashMap<String, String>> = summary_map
            .iter()
            .map(|((reason, domain), count)| {
                let mut row = HashMap::new();
                row.insert("reason".to_string(), reason.clone());
                row.insert("domain".to_string(), domain.clone());
                row.insert("count".to_string(), count.to_string());
                row
            })
            .collect();
        skipped_urls_summary.sort_by(|a, b| {
            let count_a: usize = a.get("count").and_then(|c| c.parse().ok()).unwrap_or(0);
            let count_b: usize = b.get("count").and_then(|c| c.parse().ok()).unwrap_or(0);
            count_b.cmp(&count_a)
        });

        // Build detail: each skipped URL as a row
        let visited_urls = status.get_visited_urls();
        let visited_map: HashMap<String, &VisitedUrl> = visited_urls.iter().map(|v| (v.uq_id.clone(), v)).collect();

        let mut skipped_urls: Vec<HashMap<String, String>> = skipped_entries
            .iter()
            .map(|entry| {
                let mut row = HashMap::new();
                row.insert("reason".to_string(), Self::get_reason_label(&entry.reason).to_string());

                // Strip scheme and host only for same-domain URLs
                let skipped_url = crate::utils::get_url_without_scheme_and_host(
                    &entry.url,
                    initial_host.as_deref(),
                    initial_scheme.as_deref(),
                );
                row.insert("url".to_string(), skipped_url);
                row.insert(
                    "sourceAttr".to_string(),
                    Self::get_source_short_name(entry.source_attr).to_string(),
                );

                // Resolve source URL from source_uq_id
                let source_url = visited_map
                    .get(&entry.source_uq_id)
                    .map(|v| {
                        crate::utils::get_url_without_scheme_and_host(
                            &v.url,
                            initial_host.as_deref(),
                            initial_scheme.as_deref(),
                        )
                    })
                    .unwrap_or_default();
                row.insert("sourceUqId".to_string(), source_url);
                row
            })
            .collect();
        skipped_urls.sort_by(|a, b| {
            let url_a = a.get("url").map(|s| s.as_str()).unwrap_or("");
            let url_b = b.get("url").map(|s| s.as_str()).unwrap_or("");
            url_a.cmp(url_b)
        });

        let url_column_width = 60;

        // Skipped URLs summary table
        let summary_columns = vec![
            SuperTableColumn::new(
                "reason".to_string(),
                "Reason".to_string(),
                18,
                None,
                None,
                false,
                false,
                false,
                true,
                None,
            ),
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
                "count".to_string(),
                "Unique URLs".to_string(),
                11,
                None,
                None,
                false,
                false,
                false,
                true,
                None,
            ),
        ];

        let mut super_table_summary = SuperTable::new(
            SUPER_TABLE_SKIPPED_SUMMARY.to_string(),
            "Skipped URLs Summary".to_string(),
            "No skipped URLs found.".to_string(),
            summary_columns,
            true,
            Some("count".to_string()),
            "DESC".to_string(),
            None,
            None,
            Some("Skipped URLs".to_string()),
        );

        super_table_summary.set_data(skipped_urls_summary);
        status.configure_super_table_url_stripping(&mut super_table_summary);
        output.add_super_table(&super_table_summary);
        status.add_super_table_at_beginning(super_table_summary);

        // Skipped URLs table
        let detail_columns = vec![
            SuperTableColumn::new(
                "reason".to_string(),
                "Reason".to_string(),
                18,
                None,
                None,
                false,
                false,
                false,
                true,
                None,
            ),
            SuperTableColumn::new(
                "url".to_string(),
                "Skipped URL".to_string(),
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
                "sourceAttr".to_string(),
                "Source".to_string(),
                19,
                None,
                None,
                false,
                false,
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

        let count_skipped = skipped_urls.len();

        let mut super_table = SuperTable::new(
            SUPER_TABLE_SKIPPED.to_string(),
            "Skipped URLs".to_string(),
            "No skipped URLs found.".to_string(),
            detail_columns,
            true,
            Some("url".to_string()),
            "ASC".to_string(),
            None,
            None,
            None,
        );

        super_table.set_data(skipped_urls);
        status.configure_super_table_url_stripping(&mut super_table);
        output.add_super_table(&super_table);
        status.add_super_table_at_beginning(super_table);

        status.add_summary_item_by_ranges(
            "skipped",
            count_skipped as f64,
            &[(0.0, 0.0), (1.0, 2.0), (3.0, 9.0), (10.0, f64::MAX)],
            &[
                "Skipped URLs - no skipped URLs found",
                "Skipped URLs - {} skipped URLs found",
                "Skipped URLs - {} skipped URLs found",
                "Skipped URLs - {} skipped URLs found",
            ],
        );
    }

    fn should_be_activated(&self) -> bool {
        true
    }

    fn get_order(&self) -> i32 {
        6
    }

    fn get_name(&self) -> &str {
        "SkippedUrlsAnalyzer"
    }

    fn get_exec_times(&self) -> &HashMap<String, f64> {
        self.base.get_exec_times()
    }

    fn get_exec_counts(&self) -> &HashMap<String, usize> {
        self.base.get_exec_counts()
    }
}
