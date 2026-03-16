// SiteOne Crawler - ExternalLinksAnalyzer
// (c) Jan Reges <jan.reges@siteone.cz>
//
// Presents external URLs discovered during crawling as a dedicated section.
// Groups external URLs, shows occurrence count and up to 5 source pages.

use std::collections::HashMap;

use crate::analysis::analyzer::Analyzer;
use crate::analysis::base_analyzer::BaseAnalyzer;
use crate::components::super_table::SuperTable;
use crate::components::super_table_column::SuperTableColumn;
use crate::output::output::Output;
use crate::result::status::Status;
use crate::types::SkippedReason;

const SUPER_TABLE_EXTERNAL_URLS: &str = "external-urls";
const MAX_SOURCE_PAGES: usize = 5;

pub struct ExternalLinksAnalyzer {
    base: BaseAnalyzer,
}

impl Default for ExternalLinksAnalyzer {
    fn default() -> Self {
        Self::new()
    }
}

impl ExternalLinksAnalyzer {
    pub fn new() -> Self {
        Self {
            base: BaseAnalyzer::new(),
        }
    }
}

impl Analyzer for ExternalLinksAnalyzer {
    fn analyze(&mut self, status: &Status, output: &mut dyn Output) {
        let skipped_entries = status.get_skipped_urls();

        // Filter only external links (NotAllowedHost reason)
        let external_entries: Vec<_> = skipped_entries
            .iter()
            .filter(|e| matches!(e.reason, SkippedReason::NotAllowedHost))
            .collect();

        // Group by external URL: collect count and source page URLs
        let mut url_data: HashMap<String, Vec<String>> = HashMap::new();
        for entry in &external_entries {
            let source_url = status.get_url_by_uq_id(&entry.source_uq_id).unwrap_or_default();
            let sources = url_data.entry(entry.url.clone()).or_default();
            if !source_url.is_empty() && !sources.contains(&source_url) {
                sources.push(source_url);
            }
        }

        let total_urls = url_data.len();

        let mut rows: Vec<HashMap<String, String>> = url_data
            .iter()
            .map(|(ext_url, sources)| {
                let mut row = HashMap::new();
                row.insert("url".to_string(), ext_url.clone());
                row.insert("count".to_string(), sources.len().to_string());
                let display_sources: Vec<&str> = sources.iter().take(MAX_SOURCE_PAGES).map(|s| s.as_str()).collect();
                let mut found_on = display_sources.join(", ");
                if sources.len() > MAX_SOURCE_PAGES {
                    found_on.push_str(&format!(" (+{})", sources.len() - MAX_SOURCE_PAGES));
                }
                row.insert("foundOn".to_string(), found_on);
                row
            })
            .collect();
        rows.sort_by(|a, b| {
            let count_a: usize = a.get("count").and_then(|c| c.parse().ok()).unwrap_or(0);
            let count_b: usize = b.get("count").and_then(|c| c.parse().ok()).unwrap_or(0);
            count_b.cmp(&count_a).then_with(|| a.get("url").cmp(&b.get("url")))
        });

        let columns = vec![
            SuperTableColumn::new(
                "url".to_string(),
                "External URL".to_string(),
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
                "Pages".to_string(),
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
                "foundOn".to_string(),
                "Found on URL (max 5)".to_string(),
                -1, // AUTO_WIDTH
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
            SUPER_TABLE_EXTERNAL_URLS.to_string(),
            "External URLs".to_string(),
            "No external URLs found.".to_string(),
            columns,
            true,
            Some("count".to_string()),
            "DESC".to_string(),
            Some(format!("{} external URL(s)", total_urls)),
            None,
            None,
        );

        super_table.set_data(rows);
        status.configure_super_table_url_stripping(&mut super_table);
        output.add_super_table(&super_table);
        status.add_super_table_at_beginning(super_table);

        status.add_summary_item_by_ranges(
            "external-urls",
            total_urls as f64,
            &[(0.0, 0.0), (1.0, f64::MAX)],
            &[
                "External URLs - no external URLs found",
                "External URLs - {} external URL(s) found",
            ],
        );
    }

    fn should_be_activated(&self) -> bool {
        true
    }

    fn get_order(&self) -> i32 {
        7 // After skipped URLs (6)
    }

    fn get_name(&self) -> &str {
        "ExternalLinksAnalyzer"
    }

    fn get_exec_times(&self) -> &HashMap<String, f64> {
        self.base.get_exec_times()
    }

    fn get_exec_counts(&self) -> &HashMap<String, usize> {
        self.base.get_exec_counts()
    }
}
