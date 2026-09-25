// SiteOne Crawler - BrowserConsoleAnalyzer
// (c) Jan Reges <jan.reges@siteone.cz>
//
// Reports per-page browser diagnostics (console errors/warnings, uncaught JS exceptions,
// failed sub-requests, security violations) collected while rendering in --browser mode.
// Activated only when browser rendering is enabled; otherwise the diagnostics map is empty.

use std::collections::HashMap;

use crate::analysis::analyzer::Analyzer;
use crate::analysis::base_analyzer::BaseAnalyzer;
use crate::browser::diagnostics::Severity;
use crate::components::super_table::SuperTable;
use crate::components::super_table_column::SuperTableColumn;
use crate::output::output::Output;
use crate::result::status::Status;
use crate::utils;

const SUPER_TABLE_BROWSER_CONSOLE: &str = "browser-console";

pub struct BrowserConsoleAnalyzer {
    base: BaseAnalyzer,
    activated: bool,
}

impl BrowserConsoleAnalyzer {
    pub fn new() -> Self {
        Self {
            base: BaseAnalyzer::new(),
            activated: false,
        }
    }

    /// Activate this analyzer (only meaningful when --browser is in use).
    pub fn set_activated(&mut self, activated: bool) {
        self.activated = activated;
    }
}

impl Default for BrowserConsoleAnalyzer {
    fn default() -> Self {
        Self::new()
    }
}

impl Analyzer for BrowserConsoleAnalyzer {
    fn analyze(&mut self, status: &Status, output: &mut dyn Output) {
        let visited = status.get_visited_urls();
        let console_width = utils::get_console_width();
        let url_column_size = (console_width as i32 - 70).max(20);

        let mut data: Vec<HashMap<String, String>> = Vec::new();
        let mut screenshots_data: Vec<HashMap<String, String>> = Vec::new();
        // Count pages with ANY issue (errors OR warnings) so a warning-only page (e.g. a 4xx
        // sub-request or a screenshot failure) doesn't get masked as "Browser OK".
        let mut pages_with_issues = 0usize;
        let mut screenshot_count = 0usize;
        let mut screenshot_pages = 0usize;
        let mut screenshot_dir = String::new();

        for u in &visited {
            let Some(diag) = status.get_browser_diagnostics(&u.uq_id) else {
                continue;
            };
            if diag.screenshot_path.is_some() || !diag.extra_screenshots.is_empty() {
                screenshot_pages += 1;
            }
            // One row per file: the first --screenshot-viewport size, then the further sizes.
            for path in diag.screenshot_path.iter().chain(diag.extra_screenshots.iter()) {
                screenshot_count += 1;
                if screenshot_dir.is_empty() {
                    screenshot_dir = std::path::Path::new(path)
                        .parent()
                        .map(|d| d.to_string_lossy().to_string())
                        .unwrap_or_default();
                }
                let mut srow = HashMap::new();
                srow.insert("url".to_string(), u.url.clone());
                srow.insert("path".to_string(), path.clone());
                screenshots_data.push(srow);
            }
            if diag.issue_count() == 0 {
                continue;
            }
            pages_with_issues += 1;

            // Pick a representative message by importance: render failure → console error →
            // uncaught exception → console warning → screenshot failure.
            let top = diag
                .render_error
                .clone()
                .or_else(|| {
                    diag.console
                        .iter()
                        .find(|c| matches!(c.severity, Severity::Error))
                        .map(|c| c.text.clone())
                })
                .or_else(|| diag.exceptions.first().map(|e| e.text.clone()))
                .or_else(|| {
                    diag.console
                        .iter()
                        .find(|c| matches!(c.severity, Severity::Warning))
                        .map(|c| c.text.clone())
                })
                .or_else(|| diag.screenshot_error.clone())
                .unwrap_or_default();
            let top: String = top.chars().take(120).collect();

            let mut row = HashMap::new();
            row.insert("url".to_string(), u.url.clone());
            row.insert("errors".to_string(), diag.errors.to_string());
            row.insert("warnings".to_string(), diag.warnings.to_string());
            row.insert("network".to_string(), diag.network_errors.len().to_string());
            row.insert("message".to_string(), top);
            data.push(row);
        }

        let columns = vec![
            SuperTableColumn::new(
                "url".to_string(),
                "URL".to_string(),
                url_column_size,
                None,
                None,
                true,
                false,
                false,
                true,
                None,
            ),
            SuperTableColumn::new(
                "errors".to_string(),
                "Errors".to_string(),
                7,
                None,
                None,
                false,
                false,
                false,
                true,
                None,
            ),
            SuperTableColumn::new(
                "warnings".to_string(),
                "Warns".to_string(),
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
                "network".to_string(),
                "NetErr".to_string(),
                7,
                None,
                None,
                false,
                false,
                false,
                true,
                None,
            ),
            SuperTableColumn::new(
                "message".to_string(),
                "Top message".to_string(),
                40,
                None,
                None,
                true,
                false,
                false,
                true,
                None,
            ),
        ];

        let mut super_table = SuperTable::new(
            SUPER_TABLE_BROWSER_CONSOLE.to_string(),
            "Browser issues (console / JS / network / security)".to_string(),
            "No browser console / JS / network / security issues found on rendered pages.".to_string(),
            columns,
            true,
            Some("errors".to_string()),
            "DESC".to_string(),
            None,
            None,
            None,
        );

        super_table.set_data(data);
        status.configure_super_table_url_stripping(&mut super_table);
        output.add_super_table(&super_table);
        status.add_super_table_at_end(super_table);

        // Screenshots table (each rendered page → saved file; clickable in the HTML report).
        if !screenshots_data.is_empty() {
            let shot_columns = vec![
                SuperTableColumn::new(
                    "url".to_string(),
                    "URL".to_string(),
                    url_column_size,
                    None,
                    None,
                    true,
                    false,
                    false,
                    true,
                    None,
                ),
                SuperTableColumn::new(
                    "path".to_string(),
                    "Screenshot file".to_string(),
                    60,
                    None,
                    None,
                    true,
                    false,
                    false,
                    true,
                    None,
                ),
            ];
            let mut shot_table = SuperTable::new(
                "browser-screenshots".to_string(),
                "Browser screenshots".to_string(),
                "No screenshots captured.".to_string(),
                shot_columns,
                true,
                Some("url".to_string()),
                "ASC".to_string(),
                None,
                None,
                None,
            );
            shot_table.set_data(screenshots_data);
            status.configure_super_table_url_stripping(&mut shot_table);
            output.add_super_table(&shot_table);
            status.add_super_table_at_end(shot_table);
        }

        status.add_summary_item_by_ranges(
            "browser-console",
            pages_with_issues as f64,
            &[(0.0, 0.0), (1.0, 2.0), (3.0, 5.0), (6.0, f64::MAX)],
            &[
                "Browser OK - no JS/console/network/security issues on rendered pages",
                "Browser NOTICE - {} page(s) with browser issues",
                "Browser WARNING - {} pages with browser issues",
                "Browser CRITICAL - {} pages with browser issues",
            ],
        );

        if screenshot_count > 0 {
            // With several --screenshot-viewport sizes a page has several files.
            let text = if screenshot_count == screenshot_pages {
                format!(
                    "Captured {} page screenshot(s) into '{}'.",
                    screenshot_count, screenshot_dir
                )
            } else {
                format!(
                    "Captured {} screenshot(s) of {} page(s) into '{}'.",
                    screenshot_count, screenshot_pages, screenshot_dir
                )
            };
            status.add_ok_to_summary("screenshots", &text);
        }
    }

    fn should_be_activated(&self) -> bool {
        self.activated
    }

    fn get_order(&self) -> i32 {
        25
    }

    fn get_name(&self) -> &str {
        "BrowserConsoleAnalyzer"
    }

    fn get_exec_times(&self) -> &HashMap<String, f64> {
        self.base.get_exec_times()
    }

    fn get_exec_counts(&self) -> &HashMap<String, usize> {
        self.base.get_exec_counts()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::browser::diagnostics::BrowserDiagnostics;
    use crate::info::Info;
    use crate::output::multi_output::MultiOutput;
    use crate::result::storage::memory_storage::MemoryStorage;
    use crate::result::visited_url::VisitedUrl;
    use crate::types::ContentTypeId;

    /// Runs the analyzer over pages with the given screenshot files (the first one is
    /// `screenshot_path`, the rest `extra_screenshots`); returns the screenshot table's paths and
    /// the "screenshots" summary text.
    fn analyze_screenshots(pages: &[&[&str]]) -> (Vec<String>, Option<String>) {
        let info = Info::new(
            "SiteOne Crawler".to_string(),
            "test".to_string(),
            String::new(),
            String::new(),
            String::new(),
            String::new(),
            "https://example.com/".to_string(),
        );
        let mut status = Status::new(
            Box::new(MemoryStorage::new(false)),
            true,
            info,
            std::time::Instant::now(),
        );
        for (index, files) in pages.iter().enumerate() {
            let uq_id = format!("page{index:04}");
            let visited = VisitedUrl::new(
                uq_id.clone(),
                String::new(),
                0,
                format!("https://example.com/page-{index}"),
                200,
                0.1,
                Some(100),
                ContentTypeId::Html,
                Some("text/html".to_string()),
                None,
                None,
                false,
                true,
                0,
                None,
            );
            status.add_visited_url(visited, None, None);
            status.add_browser_diagnostics(
                &uq_id,
                BrowserDiagnostics {
                    screenshot_path: files.first().map(|file| file.to_string()),
                    extra_screenshots: files.iter().skip(1).map(|file| file.to_string()).collect(),
                    ..Default::default()
                },
            );
        }

        let mut analyzer = BrowserConsoleAnalyzer::new();
        analyzer.set_activated(true);
        analyzer.analyze(&status, &mut MultiOutput::new());

        let paths = status
            .get_super_table_rows("browser-screenshots")
            .into_iter()
            .map(|row| row["path"].clone())
            .collect();
        let captured = status
            .get_summary()
            .get_items()
            .iter()
            .find(|item| item.apl_code == "screenshots")
            .map(|item| item.text.clone());
        (paths, captured)
    }

    #[test]
    fn screenshots_table_has_one_row_per_file() {
        let (paths, captured) =
            analyze_screenshots(&[&["shots/example_com_1920x1080.png", "shots/example_com_390x844.png"]]);

        assert_eq!(paths.len(), 2, "{paths:?}");
        assert!(paths.contains(&"shots/example_com_1920x1080.png".to_string()));
        assert!(paths.contains(&"shots/example_com_390x844.png".to_string()));
        assert_eq!(
            captured.as_deref(),
            Some("Captured 2 screenshot(s) of 1 page(s) into 'shots'.")
        );
    }

    #[test]
    fn one_screenshot_per_page_keeps_the_page_count_text() {
        let (_, captured) = analyze_screenshots(&[&["shots/a.png"], &["shots/b.png"]]);

        assert_eq!(captured.as_deref(), Some("Captured 2 page screenshot(s) into 'shots'."));
    }
}
