// SiteOne Crawler - FileExporter
// (c) Jan Reges <jan.reges@siteone.cz>
//
// Saves crawl results to HTML, JSON, and/or text files.

use std::fs;
use std::time::Instant;

use crate::error::{CrawlerError, CrawlerResult};
use crate::export::base_exporter;
use crate::export::exporter::Exporter;
use crate::output::output::Output;
use crate::result::status::Status;
use crate::utils;

pub struct FileExporter {
    /// Path for HTML report output (--output-html-report)
    pub output_html_report: Option<String>,
    /// Comma-separated list of sections for HTML report (--html-report-options)
    pub html_report_options: Option<String>,
    /// Path for JSON output (--output-json-file)
    pub output_json_file: Option<String>,
    /// Path for text output (--output-text-file)
    pub output_text_file: Option<String>,
    /// Add timestamp to output filename (--add-timestamp-to-output-file)
    pub add_timestamp_to_output_file: bool,
    /// Add host to output filename (--add-host-to-output-file)
    pub add_host_to_output_file: bool,
    /// Initial host from the crawled URL (for filename generation)
    pub initial_host: Option<String>,
    /// Cached text output to save to file
    pub text_output_content: Option<String>,
    /// Cached JSON output to save to file
    pub json_output_content: Option<String>,
    /// Cached HTML report content to save to file
    pub html_report_content: Option<String>,
}

impl FileExporter {
    pub fn new(
        output_html_report: Option<String>,
        html_report_options: Option<String>,
        output_json_file: Option<String>,
        output_text_file: Option<String>,
        add_timestamp_to_output_file: bool,
        add_host_to_output_file: bool,
        initial_host: Option<String>,
    ) -> Self {
        Self {
            output_html_report,
            html_report_options,
            output_json_file,
            output_text_file,
            add_timestamp_to_output_file,
            add_host_to_output_file,
            initial_host,
            text_output_content: None,
            json_output_content: None,
            html_report_content: None,
        }
    }

    /// Set the text output content to be saved (from TextOutput)
    pub fn set_text_output_content(&mut self, content: String) {
        self.text_output_content = Some(content);
    }

    /// Set the JSON output content to be saved (from JsonOutput)
    pub fn set_json_output_content(&mut self, content: String) {
        self.json_output_content = Some(content);
    }

    /// Set the HTML report content to be saved (from HtmlReport)
    pub fn set_html_report_content(&mut self, content: String) {
        self.html_report_content = Some(content);
    }

    /// Get the export file path with host/timestamp modifications.
    fn get_export_file_path(&self, file: &str, extension: &str) -> CrawlerResult<String> {
        base_exporter::get_export_file_path(
            file,
            extension,
            self.add_host_to_output_file,
            self.initial_host.as_deref(),
            self.add_timestamp_to_output_file,
        )
    }
}

impl Exporter for FileExporter {
    fn get_name(&self) -> &str {
        "FileExporter"
    }

    fn should_be_activated(&self) -> bool {
        self.output_html_report.is_some() || self.output_json_file.is_some() || self.output_text_file.is_some()
    }

    fn export(&mut self, status: &Status, _output: &dyn Output) -> CrawlerResult<()> {
        // Export text file
        if let Some(ref output_text_file) = self.output_text_file.clone() {
            let start = Instant::now();
            let report_file = self.get_export_file_path(output_text_file, "txt")?;

            let content = match &self.text_output_content {
                Some(c) => utils::remove_ansi_colors(c),
                None => {
                    return Err(CrawlerError::Export(
                        "Text output content not available for FileExporter".to_string(),
                    ));
                }
            };

            fs::write(&report_file, &content).map_err(|e| {
                CrawlerError::Export(format!("Failed to write text report to '{}': {}", report_file, e))
            })?;

            let elapsed = start.elapsed().as_secs_f64();
            let report_file_display = utils::get_output_formatted_path(&report_file);
            status.add_info_to_summary(
                "export-to-text",
                &format!(
                    "Text report saved to '{}' and took {}",
                    report_file_display,
                    utils::get_formatted_duration(elapsed)
                ),
            );
        }

        // Export JSON file
        if let Some(ref output_json_file) = self.output_json_file.clone() {
            let start = Instant::now();
            let report_file = self.get_export_file_path(output_json_file, "json")?;

            let content = match &self.json_output_content {
                Some(c) => c.clone(),
                None => {
                    return Err(CrawlerError::Export(
                        "JSON output content not available for FileExporter".to_string(),
                    ));
                }
            };

            fs::write(&report_file, &content).map_err(|e| {
                CrawlerError::Export(format!("Failed to write JSON report to '{}': {}", report_file, e))
            })?;

            let elapsed = start.elapsed().as_secs_f64();
            let report_file_display = utils::get_output_formatted_path(&report_file);
            status.add_info_to_summary(
                "export-to-json",
                &format!(
                    "JSON report saved to '{}' and took {}",
                    report_file_display,
                    utils::get_formatted_duration(elapsed)
                ),
            );
        }

        // Export HTML report
        if let Some(ref output_html_report) = self.output_html_report.clone() {
            let start = Instant::now();
            let report_file = self.get_export_file_path(output_html_report, "html")?;

            let content = match &self.html_report_content {
                Some(c) => c.clone(),
                None => {
                    return Err(CrawlerError::Export(
                        "HTML report content not available. Set it via set_html_report_content() before export."
                            .to_string(),
                    ));
                }
            };

            fs::write(&report_file, &content).map_err(|e| {
                CrawlerError::Export(format!("Failed to write HTML report to '{}': {}", report_file, e))
            })?;

            let elapsed = start.elapsed().as_secs_f64();
            let report_file_display = utils::get_output_formatted_path(&report_file);
            status.add_info_to_summary(
                "export-to-html",
                &format!(
                    "HTML report saved to '{}' and took {}",
                    report_file_display,
                    utils::get_formatted_duration(elapsed)
                ),
            );
        }

        Ok(())
    }
}
