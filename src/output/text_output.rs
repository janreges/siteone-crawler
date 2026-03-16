// SiteOne Crawler - TextOutput (console output)
// (c) Jan Reges <jan.reges@siteone.cz>
//

use std::collections::HashMap;
use std::io::Write;

use crate::components::summary::summary::Summary;
use crate::components::super_table::SuperTable;
use crate::extra_column::ExtraColumn;
use crate::output::output::{BasicStats, CrawlerInfo, Output};
use crate::output::output_type::OutputType;
use crate::scoring::ci_gate::CiGateResult;
use crate::scoring::quality_score::QualityScores;
use crate::types::ContentTypeId;
use crate::utils;

pub struct TextOutput {
    version: String,
    print_to_output: bool,
    extra_columns_from_analysis_width: usize,
    extra_columns_width: usize,

    terminal_width: usize,
    compact_mode: bool,
    progress_bar_width: usize,

    /// Extra columns from analysis that will be added to the table
    extra_columns_from_analysis: Vec<ExtraColumn>,

    /// Extra columns from options (user-specified)
    extra_columns: Vec<ExtraColumn>,

    output_text: String,

    origin_host: String,

    // Options that control output behavior
    hide_progress_bar: bool,
    show_scheme_and_host: bool,
    do_not_truncate_url: bool,
    add_random_query_params: bool,
    url_column_size: Option<usize>,
    show_inline_criticals: bool,
    show_inline_warnings: bool,
    workers: usize,
    memory_limit: String,
    disable_animation: bool,

    /// Cached computed URL column size
    cached_url_column_size: Option<usize>,
}

impl TextOutput {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        crawler_info: CrawlerInfo,
        extra_columns: Vec<ExtraColumn>,
        hide_progress_bar: bool,
        show_scheme_and_host: bool,
        do_not_truncate_url: bool,
        add_random_query_params: bool,
        url_column_size: Option<usize>,
        show_inline_criticals: bool,
        show_inline_warnings: bool,
        workers: usize,
        memory_limit: String,
        print_to_output: bool,
        disable_animation: bool,
    ) -> Self {
        let terminal_width = utils::get_console_width();
        let compact_mode = terminal_width < 140;

        let mut extra_columns_width: usize = 0;
        for extra_column in &extra_columns {
            extra_columns_width += extra_column.get_length() + 3; // 3 = 2 spaces + 1 pipe
        }

        let progress_bar_width = if hide_progress_bar {
            0
        } else if compact_mode {
            8
        } else {
            26
        };

        let origin_host = extract_host(&crawler_info.url);

        Self {
            version: crawler_info.version.clone(),
            print_to_output,
            extra_columns_from_analysis_width: 0,
            extra_columns_width,
            terminal_width,
            compact_mode,
            progress_bar_width,
            extra_columns_from_analysis: Vec::new(),
            extra_columns,
            output_text: String::new(),
            origin_host,
            hide_progress_bar,
            show_scheme_and_host,
            do_not_truncate_url,
            add_random_query_params,
            url_column_size,
            show_inline_criticals,
            show_inline_warnings,
            workers,
            memory_limit,
            disable_animation,
            cached_url_column_size: None,
        }
    }

    fn add_to_output(&mut self, output: &str) {
        if self.print_to_output {
            print!("{}", output);
            // Flush stdout to ensure immediate display
            let _ = std::io::stdout().flush();
        }
        self.output_text.push_str(output);
    }

    pub fn get_output_text(&self) -> &str {
        &self.output_text
    }

    fn get_url_column_size(&mut self) -> usize {
        if let Some(cached) = self.cached_url_column_size {
            return cached;
        }

        let size = if let Some(url_col_size) = self.url_column_size {
            url_col_size
        } else {
            let status_type_time_size_cache_width: usize = 49;
            let free_reserve: usize = 5;

            let url_column_size = self
                .terminal_width
                .saturating_sub(self.progress_bar_width)
                .saturating_sub(status_type_time_size_cache_width)
                .saturating_sub(self.extra_columns_width)
                .saturating_sub(self.extra_columns_from_analysis_width)
                .saturating_sub(free_reserve);

            url_column_size.max(20)
        };

        self.cached_url_column_size = Some(size);
        size
    }

    /// Generate polynomial delays for banner animation.
    fn get_polynomial_delays(total_time: f64, iterations: usize, power: u32) -> Vec<f64> {
        let mut delays = Vec::with_capacity(iterations);
        let mut total_poly_sum: f64 = 0.0;

        for i in 1..=iterations {
            total_poly_sum += (i as f64).powi(power as i32);
        }

        for i in 1..=iterations {
            delays.push(((i as f64).powi(power as i32) / total_poly_sum) * total_time);
        }

        delays
    }
}

impl Output for TextOutput {
    fn add_banner(&mut self) {
        // ASCII art banner - generated by https://www.asciiart.eu/image-to-ascii :-)
        let mut banner = String::from("\n");
        banner.push_str(" ####                ####             #####        \n");
        banner.push_str(" ####                ####           #######        \n");
        banner.push_str(" ####      ###       ####         #########        \n");
        banner.push_str(" ####     ######     ####       ###### ####        \n");
        banner.push_str("  ######################       #####   ####        \n");
        banner.push_str("    #######    #######       #####     ####        \n");
        banner.push_str("    #######    #######         #       ####        \n");
        banner.push_str("  ######################               ####        \n");
        banner.push_str(" ####     ######     ####              ####        \n");
        banner.push_str(" ####       ##       ####              ####        \n");
        banner.push_str(" ####                ####       ################## \n");
        banner.push_str(" ####                ####       ################## \n");
        banner.push('\n');
        banner.push_str(&"=".repeat(50));
        banner.push('\n');

        let texts = [
            format!("SiteOne Crawler, v{}", self.version),
            "Author: jan.reges@siteone.cz".to_string(),
        ];

        for text in &texts {
            banner.push_str(&format!("# {:<46} #\n", text));
        }
        banner.push_str(&"=".repeat(50));

        // Loading the rocket on the ramp and show banner with fancy polynomial delays
        let lines: Vec<&str> = banner.split('\n').collect();
        if self.disable_animation {
            for line in &lines {
                self.add_to_output(&format!("{}\n", utils::get_color_text(line, "yellow", false)));
            }
            self.add_to_output("\n\n");
        } else {
            let delays = Self::get_polynomial_delays(1.2, lines.len(), 2);
            for (counter, line) in lines.iter().enumerate() {
                self.add_to_output(&format!("{}\n", utils::get_color_text(line, "yellow", false)));

                // Add delay between lines
                if counter < delays.len() {
                    let usleep_time = std::time::Duration::from_micros((delays[counter] * 1_000_000.0) as u64);
                    std::thread::sleep(usleep_time);
                }
            }

            // The rocket takes off smoothly :)
            std::thread::sleep(std::time::Duration::from_millis(300));
            self.add_to_output("\n");
            std::thread::sleep(std::time::Duration::from_millis(150));
            self.add_to_output("\n");
        }

        if self.compact_mode {
            self.add_to_output(&utils::get_color_text(
                &format!(
                    "Detected terminal width {} < 140 chars - compact mode activated.\n\n",
                    self.terminal_width
                ),
                "yellow",
                false,
            ));
        }
    }

    fn add_used_options(&mut self) {
        // Intentionally left empty
    }

    fn set_extra_columns_from_analysis(&mut self, extra_columns: Vec<ExtraColumn>) {
        self.extra_columns_from_analysis_width = 0;
        for extra_column in &extra_columns {
            self.extra_columns_from_analysis_width += extra_column.get_length() + 3;
            // 3 = 2 spaces + 1 pipe
        }
        self.extra_columns_from_analysis = extra_columns;
        // Reset cached URL column size since widths changed
        self.cached_url_column_size = None;
    }

    fn add_table_header(&mut self) {
        let url_col_size = self.get_url_column_size();
        let mut header = format!(
            "{:<width$} | Status | Type     | Time   | Size   | Cache ",
            "URL",
            width = url_col_size
        );

        if !self.hide_progress_bar {
            let progress_label = if self.compact_mode {
                "Progress"
            } else {
                "Progress report"
            };
            header = format!(
                "{:<width$}| {}",
                progress_label,
                header,
                width = self.progress_bar_width
            );
        }

        for extra_column in &self.extra_columns_from_analysis {
            header.push_str(&format!(
                " | {:<width$}",
                extra_column.name,
                width = extra_column.get_length().max(4)
            ));
        }

        for extra_column in &self.extra_columns {
            header.push_str(&format!(
                " | {:<width$}",
                extra_column.name,
                width = extra_column.get_length().max(4)
            ));
        }
        header.push('\n');

        let header_len = header.len();
        self.add_to_output(&format!(
            "{}{}\n",
            utils::get_color_text(&header, "gray", false),
            "-".repeat(header_len)
        ));
    }

    fn add_table_row(
        &mut self,
        response_headers: &HashMap<String, String>,
        url: &str,
        status: i32,
        elapsed_time: f64,
        size: i64,
        content_type: i32,
        extra_parsed_content: &HashMap<String, String>,
        progress_status: &str,
        cache_type_flags: i32,
        cache_lifetime: Option<i32>,
    ) {
        let is_external_url = !url.contains(&format!("://{}", self.origin_host));

        let url_for_table = if !self.show_scheme_and_host && !is_external_url {
            // Strip scheme and host from URL
            strip_scheme_and_host(url)
        } else {
            url.to_string()
        };

        let url_col_size = self.get_url_column_size();

        let colored_status = utils::get_colored_status_code(status, 6);

        let content_type_name = ContentTypeId::from_i32(content_type)
            .map(|ct| ct.name())
            .unwrap_or("Other");
        let content_type_padded = format!("{:<8}", content_type_name);

        let colored_elapsed_time = utils::get_colored_request_time(elapsed_time, 6);

        let colored_size = if size > 1024 * 1024 {
            utils::get_color_text(&format!("{:<6}", utils::get_formatted_size(size, 0)), "red", false)
        } else {
            format!("{:<6}", utils::get_formatted_size(size, 0))
        };

        let content_type_header = response_headers.get("content-type").map(|s| s.as_str()).unwrap_or("");
        let is_asset = utils::is_asset_by_content_type(content_type_header);
        let colored_cache = get_colored_cache_info(cache_type_flags, cache_lifetime, is_asset);

        // Process extra columns from analysis
        let mut extra_headers_content = String::new();
        let mut extra_new_line = String::new();
        let extra_new_line_prefix = "  ";

        for extra_column in &self.extra_columns_from_analysis {
            let value = extra_parsed_content
                .get(&extra_column.name)
                .map(|s| s.as_str())
                .unwrap_or("");

            // For analysis results, we use the value as-is (colored output already applied)
            let truncated = extra_column.get_truncated_value(Some(value)).unwrap_or_default();
            extra_headers_content.push_str(&format!(
                " | {:<width$}",
                truncated,
                width = extra_column.get_length().max(4)
            ));

            // Show inline criticals/warnings if configured
            if self.show_inline_criticals && value.contains("[CRITICAL]") {
                extra_new_line.push_str(&format!("{}\u{26D4} {}\n", extra_new_line_prefix, value));
            }
            if self.show_inline_warnings && value.contains("[WARNING]") {
                extra_new_line.push_str(&format!("{}\u{26A0}\u{FE0F} {}\n", extra_new_line_prefix, value));
            }
        }

        // Process extra columns from options
        for extra_column in &self.extra_columns {
            let mut value = String::new();
            let header_name = &extra_column.name;

            if let Some(v) = extra_parsed_content.get(header_name) {
                value = v.trim().to_string();
            } else if let Some(v) = response_headers.get(&header_name.to_lowercase()) {
                value = v.trim().to_string();
            }

            let truncated = extra_column.get_truncated_value(Some(&value)).unwrap_or_default();
            extra_headers_content.push_str(&format!(
                " | {:<width$}",
                truncated,
                width = extra_column.get_length().max(4)
            ));
        }

        let mut url_display = url_for_table.clone();

        if self.add_random_query_params {
            url_display.push_str(&utils::get_color_text("+%random-query%", "gray", false));
        }

        if !self.do_not_truncate_url {
            url_display = utils::truncate_in_two_thirds(&url_display, url_col_size, "\u{2026}", None);
        }

        // Progress content
        let progress_content = if !self.hide_progress_bar {
            let parts: Vec<&str> = progress_status.splitn(2, '/').collect();
            let done: usize = parts.first().and_then(|s| s.parse().ok()).unwrap_or(0);
            let total: usize = parts.get(1).and_then(|s| s.parse().ok()).unwrap_or(1);

            if self.compact_mode {
                format!("{:<7} |", progress_status)
            } else {
                let progress_to_stderr =
                    format!("{:<7} | {}", progress_status, utils::get_progress_bar(done, total, 10));
                format!("{:<17}", progress_to_stderr)
            }
        } else {
            String::new()
        };

        let output = format!(
            "{} {:<width$} | {} | {} | {} | {} | {} {}\n",
            progress_content,
            url_display,
            colored_status,
            content_type_padded,
            colored_elapsed_time,
            colored_size,
            colored_cache,
            extra_headers_content,
            width = url_col_size,
        );

        if !extra_new_line.is_empty() {
            let combined = format!("{}{}\n", output, extra_new_line.trim_end());
            self.add_to_output(&combined);
        } else {
            self.add_to_output(&output);
        }
    }

    fn add_super_table(&mut self, table: &SuperTable) {
        self.add_to_output("\n");
        self.add_to_output(&table.get_console_output());
    }

    fn add_total_stats(&mut self, stats: &BasicStats) {
        self.add_to_output("\n");
        self.add_to_output(&"=".repeat(self.terminal_width));
        self.add_to_output("\n");

        let peak_memory = utils::get_peak_memory_usage();
        let peak_memory_str = if peak_memory > 0 {
            format!(
                " (max used {})",
                utils::get_color_text(&utils::get_formatted_size(peak_memory, 0), "cyan", false,)
            )
        } else {
            String::new()
        };
        let result_header = format!(
            "Total execution time {} using {} workers and {} memory limit{}\n",
            utils::get_color_text(
                &utils::get_formatted_duration(stats.total_execution_time),
                "cyan",
                false,
            ),
            utils::get_color_text(&self.workers.to_string(), "cyan", false),
            utils::get_color_text(&self.memory_limit, "cyan", false),
            peak_memory_str,
        );
        self.add_to_output(&result_header);

        let reqs_per_sec = if stats.total_execution_time > 0.0 {
            (stats.total_urls as f64 / stats.total_execution_time) as i64
        } else {
            0
        };
        let bytes_per_sec = if stats.total_execution_time > 0.0 {
            (stats.total_size as f64 / stats.total_execution_time) as i64
        } else {
            0
        };

        self.add_to_output(&format!(
            "Total of {} visited URLs with a total size of {} and power of {} with download speed {}\n",
            utils::get_color_text(&stats.total_urls.to_string(), "cyan", false),
            utils::get_color_text(&stats.total_size_formatted, "cyan", false),
            utils::get_color_text(&format!("{} reqs/s", reqs_per_sec), "magenta", false),
            utils::get_color_text(
                &format!("{}/s", utils::get_formatted_size(bytes_per_sec, 0)),
                "magenta",
                false,
            ),
        ));

        self.add_to_output(&format!(
            "Response times: AVG {} MIN {} MAX {} TOTAL {}\n",
            utils::get_color_text(
                &utils::get_formatted_duration(stats.total_requests_times_avg),
                "magenta",
                false,
            ),
            utils::get_color_text(
                &utils::get_formatted_duration(stats.total_requests_times_min),
                "green",
                false,
            ),
            utils::get_color_text(
                &utils::get_formatted_duration(stats.total_requests_times_max),
                "red",
                false,
            ),
            utils::get_color_text(
                &utils::get_formatted_duration(stats.total_requests_times),
                "cyan",
                false,
            ),
        ));

        self.add_to_output(&"=".repeat(self.terminal_width));
        self.add_to_output("\n");
    }

    fn add_notice(&mut self, text: &str) {
        self.add_to_output(&format!("{}\n", utils::get_color_text(text, "blue", false)));
    }

    fn add_error(&mut self, text: &str) {
        self.add_to_output(&format!("{}\n", utils::get_color_text(text, "red", false)));
    }

    fn add_quality_scores(&mut self, scores: &QualityScores) {
        // Content: "  " + name(16) + bar(25) + "  " + score(7) + "  " + label(9) + "  " = 65
        let inner = 65;

        let mut out = String::new();
        out.push('\n');

        // Top border
        out.push_str(&format!("\u{2554}{}\u{2557}\n", "\u{2550}".repeat(inner)));

        // Title
        let title = "WEBSITE QUALITY SCORE";
        let pad = (inner as isize - title.len() as isize) / 2;
        let pad = pad.max(0) as usize;
        out.push_str(&format!(
            "\u{2551}{}{:<width$}\u{2551}\n",
            " ".repeat(pad),
            title,
            width = inner - pad,
        ));

        // Separator
        out.push_str(&format!("\u{2560}{}\u{2563}\n", "\u{2550}".repeat(inner)));

        // Overall score bar
        out.push_str(&format_score_line(&scores.overall, inner, true));

        // Separator
        out.push_str(&format!("\u{2560}{}\u{2563}\n", "\u{2550}".repeat(inner)));

        // Category scores
        for cat in &scores.categories {
            out.push_str(&format_score_line(cat, inner, false));
        }

        // Bottom border
        out.push_str(&format!("\u{255A}{}\u{255D}\n", "\u{2550}".repeat(inner)));

        self.add_to_output(&out);
    }

    fn add_ci_gate_result(&mut self, result: &CiGateResult) {
        let inner = 62;
        let mut out = String::new();
        out.push('\n');

        let border_color = if result.passed { "green" } else { "red" };

        // Top border
        out.push_str(&utils::get_color_text(
            &format!("\u{2554}{}\u{2557}", "\u{2550}".repeat(inner)),
            border_color,
            false,
        ));
        out.push('\n');

        // Title
        let title = "CI/CD QUALITY GATE";
        let pad = (inner as isize - title.len() as isize) / 2;
        let pad = pad.max(0) as usize;
        let title_line = format!("{}{:<width$}", " ".repeat(pad), title, width = inner - pad,);
        out.push_str(&utils::get_color_text("\u{2551}", border_color, false));
        out.push_str(&title_line);
        out.push_str(&utils::get_color_text("\u{2551}", border_color, false));
        out.push('\n');

        // Separator
        out.push_str(&utils::get_color_text(
            &format!("\u{2560}{}\u{2563}", "\u{2550}".repeat(inner)),
            border_color,
            false,
        ));
        out.push('\n');

        // Check lines
        for check in &result.checks {
            let (tag, tag_color) = if check.passed {
                ("[PASS]", "green")
            } else {
                ("[FAIL]", "red")
            };

            let detail = if check.passed {
                format!(
                    "{}: {} {} {}",
                    check.metric,
                    format_num(check.actual),
                    check.operator,
                    format_num(check.threshold)
                )
            } else if check.operator == ">=" {
                format!(
                    "{}: {} < {} (min: {})",
                    check.metric,
                    format_num(check.actual),
                    format_num(check.threshold),
                    format_num(check.threshold)
                )
            } else {
                format!(
                    "{}: {} > {} (max: {})",
                    check.metric,
                    format_num(check.actual),
                    format_num(check.threshold),
                    format_num(check.threshold)
                )
            };

            let content = format!("  {} {}", tag, detail);
            let visible_len = content.chars().count();
            let padding = inner.saturating_sub(visible_len);

            let colored_tag = utils::get_color_text(tag, tag_color, false);
            let line_content = format!("  {} {}{}", colored_tag, detail, " ".repeat(padding));

            out.push_str(&utils::get_color_text("\u{2551}", border_color, false));
            out.push_str(&line_content);
            out.push_str(&utils::get_color_text("\u{2551}", border_color, false));
            out.push('\n');
        }

        // Result separator
        out.push_str(&utils::get_color_text(
            &format!("\u{2560}{}\u{2563}", "\u{2550}".repeat(inner)),
            border_color,
            false,
        ));
        out.push('\n');

        // Result line
        let failed_count = result.checks.iter().filter(|c| !c.passed).count();
        let total_count = result.checks.len();
        let result_text = if result.passed {
            format!(
                "RESULT: PASS ({} of {} checks passed) \u{2014} exit code 0",
                total_count, total_count
            )
        } else {
            format!(
                "RESULT: FAIL ({} of {} checks failed) \u{2014} exit code 10",
                failed_count, total_count
            )
        };
        let result_content = format!("  {}", result_text);
        let visible_len = result_content.chars().count();
        let padding = inner.saturating_sub(visible_len);

        out.push_str(&utils::get_color_text("\u{2551}", border_color, false));
        out.push_str(&utils::get_color_text(
            &format!("{}{}", result_content, " ".repeat(padding)),
            border_color,
            false,
        ));
        out.push_str(&utils::get_color_text("\u{2551}", border_color, false));
        out.push('\n');

        // Bottom border
        out.push_str(&utils::get_color_text(
            &format!("\u{255A}{}\u{255D}", "\u{2550}".repeat(inner)),
            border_color,
            false,
        ));
        out.push('\n');

        self.add_to_output(&out);
    }

    fn add_summary(&mut self, summary: &mut Summary) {
        self.add_to_output("\n");
        self.add_to_output(&summary.get_as_console_text());
    }

    fn get_type(&self) -> OutputType {
        OutputType::Text
    }

    fn end(&mut self) {
        self.add_to_output("\n");
    }

    fn get_output_text(&self) -> Option<String> {
        Some(self.output_text.clone())
    }
}

// ---- Helper functions ----

/// Extract the host part from a URL.
fn extract_host(url: &str) -> String {
    if let Ok(parsed) = url::Url::parse(url) {
        parsed.host_str().unwrap_or("").to_string()
    } else {
        String::new()
    }
}

/// Strip scheme and host from a URL, leaving only the path (and query).
fn strip_scheme_and_host(url: &str) -> String {
    if let Ok(parsed) = url::Url::parse(url) {
        let path = parsed.path();
        if let Some(query) = parsed.query() {
            format!("{}?{}", path, query)
        } else {
            path.to_string()
        }
    } else {
        url.to_string()
    }
}

// Cache type flag constants
const CACHE_TYPE_HAS_NO_STORE: i32 = 2048;
const CACHE_TYPE_HAS_ETAG: i32 = 4;
const CACHE_TYPE_HAS_LAST_MODIFIED: i32 = 8;

/// Get colored cache info string.
fn get_colored_cache_info(cache_type_flags: i32, cache_lifetime: Option<i32>, is_asset: bool) -> String {
    let critical_color = "red";
    let warning_color = "yellow";
    let notice_color = "magenta";
    let neutral_color = "gray";
    let ok_color = "green";

    let str_pad_to = 6;

    if let Some(lifetime) = cache_lifetime {
        let color = if is_asset {
            if lifetime <= 0 {
                critical_color
            } else if lifetime < 7200 {
                warning_color
            } else if lifetime < 86400 {
                notice_color
            } else {
                ok_color
            }
        } else {
            neutral_color
        };
        utils::get_color_text(
            &format!(
                "{:<width$}",
                utils::get_formatted_cache_lifetime(lifetime as i64),
                width = str_pad_to
            ),
            color,
            false,
        )
    } else if cache_type_flags & CACHE_TYPE_HAS_NO_STORE != 0 {
        let color = if is_asset { critical_color } else { notice_color };
        utils::get_color_text(&format!("{:<width$}", "0s", width = str_pad_to), color, false)
    } else if cache_type_flags & CACHE_TYPE_HAS_ETAG != 0 {
        let color = if is_asset { warning_color } else { notice_color };
        utils::get_color_text(&format!("{:<width$}", "etag", width = str_pad_to), color, false)
    } else if cache_type_flags & CACHE_TYPE_HAS_LAST_MODIFIED != 0 {
        let color = if is_asset { warning_color } else { notice_color };
        utils::get_color_text(&format!("{:<width$}", "lm", width = str_pad_to), color, false)
    } else {
        let color = if is_asset { critical_color } else { notice_color };
        utils::get_color_text(&format!("{:<width$}", "none", width = str_pad_to), color, false)
    }
}

/// Format a number for CI gate display: integers without decimals, floats with one decimal.
fn format_num(v: f64) -> String {
    if v == v.floor() && v.abs() < 1e15 {
        format!("{}", v as i64)
    } else {
        format!("{:.1}", v)
    }
}

/// Format a single score line for the quality score box.
fn format_score_line(
    cat: &crate::scoring::quality_score::CategoryScore,
    inner_width: usize,
    _is_overall: bool,
) -> String {
    let bar_width = 25;
    let filled = ((cat.score / 10.0) * bar_width as f64).round() as usize;
    let empty = bar_width - filled;
    let bar = format!("{}{}", "\u{2588}".repeat(filled), "\u{2591}".repeat(empty),);

    let score_str = format!("{:>7}", format!("{:.1}/10", cat.score));
    let label_padded = format!("{:<16}", cat.name);
    let label_str = format!("{:<9}", cat.label);
    let content = format!("  {}{}  {}  {}", label_padded, bar, score_str, label_str);

    // Calculate visible width using char count (Unicode block chars are 1 display char each)
    let visible_width = content.chars().count();
    let padding = inner_width.saturating_sub(visible_width);

    // Colorize the entire content
    let colored = utils::get_color_text(&content, cat.console_color(), false);

    format!("\u{2551}{}{}\u{2551}\n", colored, " ".repeat(padding))
}
