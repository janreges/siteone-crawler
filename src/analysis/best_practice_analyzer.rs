// SiteOne Crawler - BestPracticeAnalyzer
// (c) Jan Reges <jan.reges@siteone.cz>

use std::collections::HashMap;
use std::time::Instant;

use regex::Regex;
use scraper::{Html, Selector};

use crate::analysis::analyzer::Analyzer;
use crate::analysis::base_analyzer::BaseAnalyzer;
use crate::analysis::result::analyzer_stats::AnalyzerStats;
use crate::analysis::result::url_analysis_result::UrlAnalysisResult;
use crate::components::super_table::SuperTable;
use crate::components::super_table_column::SuperTableColumn;
use crate::extra_column::ExtraColumn;
use crate::output::output::Output;
use crate::result::status::Status;
use crate::result::visited_url::VisitedUrl;
use crate::types::ContentTypeId;
use crate::utils;

const ANALYSIS_LARGE_SVGS: &str = "Large inline SVGs";
const ANALYSIS_DUPLICATED_SVGS: &str = "Duplicate inline SVGs";
const ANALYSIS_INVALID_SVGS: &str = "Invalid inline SVGs";
const ANALYSIS_MISSING_QUOTES: &str = "Missing quotes on attributes";
const ANALYSIS_HEADING_STRUCTURE: &str = "Heading structure";
const ANALYSIS_NON_CLICKABLE_PHONE_NUMBERS: &str = "Non-clickable phone numbers";
const ANALYSIS_DOM_DEPTH: &str = "DOM depth";
const ANALYSIS_TITLE_UNIQUENESS: &str = "Title uniqueness";
const ANALYSIS_DESCRIPTION_UNIQUENESS: &str = "Description uniqueness";
const ANALYSIS_BROTLI_SUPPORT: &str = "Brotli support";
const ANALYSIS_WEBP_SUPPORT: &str = "WebP support";
const ANALYSIS_AVIF_SUPPORT: &str = "AVIF support";

const SUPER_TABLE_BEST_PRACTICES: &str = "best-practices";
const SUPER_TABLE_NON_UNIQUE_TITLES: &str = "non-unique-titles";
const SUPER_TABLE_NON_UNIQUE_DESCRIPTIONS: &str = "non-unique-descriptions";

pub struct BestPracticeAnalyzer {
    base: BaseAnalyzer,
    stats: AnalyzerStats,

    // options
    max_inline_svg_size: usize,
    max_inline_svg_duplicate_size: usize,
    max_inline_svg_duplicates: usize,
    title_uniqueness_percentage: usize,
    meta_description_uniqueness_percentage: usize,
    max_dom_depth_warning: usize,
    max_dom_depth_critical: usize,

    // stats counters
    pages_with_large_svgs: usize,
    pages_with_duplicated_svgs: usize,
    pages_with_invalid_svgs: usize,
    pages_with_missing_quotes: usize,
    pages_with_multiple_h1: usize,
    pages_without_h1: usize,
    pages_with_skipped_heading_levels: usize,
    pages_with_deep_dom: usize,
    pages_with_non_clickable_phone_numbers: usize,
}

impl Default for BestPracticeAnalyzer {
    fn default() -> Self {
        Self::new()
    }
}

impl BestPracticeAnalyzer {
    pub fn new() -> Self {
        Self {
            base: BaseAnalyzer::new(),
            stats: AnalyzerStats::new(),

            max_inline_svg_size: 5 * 1024,
            max_inline_svg_duplicate_size: 1024,
            max_inline_svg_duplicates: 5,
            title_uniqueness_percentage: 10,
            meta_description_uniqueness_percentage: 10,
            max_dom_depth_warning: 30,
            max_dom_depth_critical: 50,

            pages_with_large_svgs: 0,
            pages_with_duplicated_svgs: 0,
            pages_with_invalid_svgs: 0,
            pages_with_missing_quotes: 0,
            pages_with_multiple_h1: 0,
            pages_without_h1: 0,
            pages_with_skipped_heading_levels: 0,
            pages_with_deep_dom: 0,
            pages_with_non_clickable_phone_numbers: 0,
        }
    }

    fn get_analysis_result(
        analysis_name: &str,
        ok: usize,
        notice: usize,
        warning: usize,
        critical: usize,
    ) -> HashMap<String, String> {
        let mut row = HashMap::new();
        row.insert("analysisName".to_string(), analysis_name.to_string());
        row.insert("ok".to_string(), ok.to_string());
        row.insert("notice".to_string(), notice.to_string());
        row.insert("warning".to_string(), warning.to_string());
        row.insert("critical".to_string(), critical.to_string());
        row
    }

    fn analyze_urls(&mut self, status: &Status, output: &mut dyn Output) -> Vec<HashMap<String, String>> {
        let mut data = self.stats.to_table_data();
        let visited_urls = status.get_visited_urls();

        let html_urls: Vec<&VisitedUrl> = visited_urls
            .iter()
            .filter(|u| u.is_allowed_for_crawling && u.status_code == 200 && u.content_type == ContentTypeId::Html)
            .collect();

        let image_urls: Vec<&VisitedUrl> = visited_urls
            .iter()
            .filter(|u| u.status_code == 200 && u.content_type == ContentTypeId::Image)
            .collect();

        // Check title uniqueness
        let s = Instant::now();
        let titles: Vec<Option<String>> = html_urls
            .iter()
            .map(|u| u.extras.as_ref().and_then(|e| e.get("Title").cloned()))
            .collect();
        data.push(self.check_title_uniqueness(&titles, status, output));
        self.base
            .measure_exec_time("BestPracticeAnalyzer", "checkTitleUniqueness", s);

        // Check meta description uniqueness
        let s = Instant::now();
        let descriptions: Vec<Option<String>> = html_urls
            .iter()
            .map(|u| u.extras.as_ref().and_then(|e| e.get("Description").cloned()))
            .collect();
        data.push(self.check_meta_description_uniqueness(&descriptions, status, output));
        self.base
            .measure_exec_time("BestPracticeAnalyzer", "checkMetaDescriptionUniqueness", s);

        // Check brotli support on internal HTML pages
        let s = Instant::now();
        let internal_html: Vec<&VisitedUrl> = html_urls
            .iter()
            .filter(|u| !u.is_external && u.content_type == ContentTypeId::Html)
            .copied()
            .collect();
        data.push(self.check_brotli_support(&internal_html, status));
        self.base
            .measure_exec_time("BestPracticeAnalyzer", "checkBrotliSupport", s);

        // Check WebP support
        let s = Instant::now();
        data.push(self.check_webp_support(&image_urls, status));
        self.base
            .measure_exec_time("BestPracticeAnalyzer", "checkWebpSupport", s);

        // Check AVIF support
        let s = Instant::now();
        data.push(self.check_avif_support(&image_urls, status));
        self.base
            .measure_exec_time("BestPracticeAnalyzer", "checkAvifSupport", s);

        data
    }

    fn check_inline_svg(&mut self, html: &str, result: &mut UrlAnalysisResult) {
        use once_cell::sync::Lazy;
        static RE_SVG: Lazy<Regex> = Lazy::new(|| Regex::new(r"(?is)<svg[^>]*>(.*?)</svg>").unwrap());
        let svg_re = &*RE_SVG;

        let matches: Vec<String> = svg_re.find_iter(html).map(|m| m.as_str().to_string()).collect();

        if matches.is_empty() {
            return;
        }

        let svg_count = matches.len();
        let mut large_svgs: Vec<String> = Vec::new();
        let mut max_found_svg_size: usize = 0;
        let mut duplicates: HashMap<String, (usize, String, usize)> = HashMap::new();
        let mut invalid_svgs: HashMap<String, (String, Vec<String>)> = HashMap::new();

        for svg in &matches {
            // Skip escaped SVGs (code examples)
            if svg.contains("&#x22;") || svg.contains("&#x27;") {
                continue;
            }

            let svg_trimmed = svg.trim();
            let size = svg_trimmed.len();

            // Use md5 hash as key
            use md5::{Digest, Md5};
            let mut hasher = Md5::new();
            hasher.update(svg_trimmed.as_bytes());
            let svg_hash = format!("{:x}", hasher.finalize());

            // Check inline SVG size
            if size > self.max_inline_svg_size {
                large_svgs.push(sanitize_svg(svg_trimmed));
                max_found_svg_size = max_found_svg_size.max(size);
                self.stats.add_warning(ANALYSIS_LARGE_SVGS, Some(svg_trimmed));
            } else {
                self.stats.add_ok(ANALYSIS_LARGE_SVGS, Some(svg_trimmed));
            }

            // Track duplicates
            let entry = duplicates
                .entry(svg_hash.clone())
                .or_insert((0, sanitize_svg(svg_trimmed), size));
            entry.0 += 1;

            // Check SVG validity
            let errors = validate_svg(svg_trimmed);
            if let Some(errors) = errors {
                invalid_svgs.insert(svg_hash.clone(), (sanitize_svg(svg_trimmed), errors));
                self.stats.add_warning(ANALYSIS_INVALID_SVGS, Some(svg_trimmed));
            } else {
                self.stats.add_ok(ANALYSIS_INVALID_SVGS, Some(svg_trimmed));
            }
        }

        // Evaluate duplicated SVGs
        let mut duplicated_svgs: Vec<String> = Vec::new();
        for (svg_hash, (count, sanitized, size)) in &duplicates {
            if *count > self.max_inline_svg_duplicates && *size > self.max_inline_svg_duplicate_size {
                duplicated_svgs.push(format!("{}x SVG ({} B): {}", count, size, sanitized));
                self.stats.add_warning(ANALYSIS_DUPLICATED_SVGS, Some(svg_hash));
            } else {
                self.stats.add_ok(ANALYSIS_DUPLICATED_SVGS, Some(svg_hash));
            }
        }

        // Report large SVGs
        if !large_svgs.is_empty() {
            result.add_warning(
                format!(
                    "{} inline svg(s) larger than {} bytes. Largest SVG is {} bytes. Consider loading from an external file to minimize HTML size",
                    large_svgs.len(),
                    self.max_inline_svg_size,
                    max_found_svg_size
                ),
                ANALYSIS_LARGE_SVGS,
                Some(large_svgs.clone()),
            );
            self.pages_with_large_svgs += 1;
        }

        let small_svgs = svg_count.saturating_sub(large_svgs.len());
        if small_svgs > 0 {
            result.add_ok(
                format!(
                    "{} inline svg(s) have a size less than {} bytes",
                    small_svgs, self.max_inline_svg_size
                ),
                ANALYSIS_LARGE_SVGS,
                None,
            );
        }

        // Report duplicated SVGs
        let duplicated_count = duplicated_svgs.len();
        if !duplicated_svgs.is_empty() {
            result.add_warning(
                format!(
                    "{} inline svg(s) are duplicated. Consider loading from an external file to minimize HTML size",
                    duplicated_count
                ),
                ANALYSIS_DUPLICATED_SVGS,
                Some(duplicated_svgs),
            );
            self.pages_with_duplicated_svgs += 1;
        }

        let uniq_svgs = svg_count.saturating_sub(duplicated_count);
        if uniq_svgs > 0 {
            result.add_ok(
                format!(
                    "{} inline svg(s) are unique (less than {} duplicates)",
                    uniq_svgs, self.max_inline_svg_duplicates
                ),
                ANALYSIS_DUPLICATED_SVGS,
                None,
            );
        }

        // Report invalid SVGs
        let invalid_count = invalid_svgs.len();
        if !invalid_svgs.is_empty() {
            let invalid_details: Vec<String> = invalid_svgs
                .values()
                .map(|(sanitized, errors)| {
                    format!(
                        "{}<br />Found {} error(s) in SVG. Errors:<br /> &nbsp; &gt; {}",
                        sanitized,
                        errors.len(),
                        errors.join("<br /> &nbsp; &gt; ")
                    )
                })
                .collect();
            result.add_critical(
                format!(
                    "{} invalid inline svg(s). Check the content of the SVG as it may contain invalid XML and cause unexpected display problems",
                    invalid_count
                ),
                ANALYSIS_INVALID_SVGS,
                Some(invalid_details),
            );
            self.pages_with_invalid_svgs += 1;
        }

        let valid_svgs = svg_count.saturating_sub(invalid_count);
        if valid_svgs > 0 {
            result.add_ok(
                format!("{} inline svg(s) are valid", valid_svgs),
                ANALYSIS_INVALID_SVGS,
                None,
            );
        }
    }

    fn check_missing_quotes_on_attributes(&mut self, html: &str, result: &mut UrlAnalysisResult) {
        use once_cell::sync::Lazy;
        static RE_UNQUOTED_ATTRS: Lazy<Regex> =
            Lazy::new(|| Regex::new(r#"<[^>]*\s(href|src|content|alt|title)\s*=\s*([^"'][^\s>]*)[^>]*>"#).unwrap());
        let re = &*RE_UNQUOTED_ATTRS;

        let mut issues: Vec<String> = Vec::new();

        for caps in re.captures_iter(html) {
            let full_match = match caps.get(0) {
                Some(m) => m.as_str(),
                None => continue,
            };
            let attribute = match caps.get(1) {
                Some(m) => m.as_str(),
                None => continue,
            };
            let value = match caps.get(2) {
                Some(m) => m.as_str(),
                None => continue,
            };

            // Skip attributes without value or with very long value
            if value.trim().is_empty() || full_match.len() > 1000 {
                continue;
            }

            // Skip escaped quotes and special cases
            if full_match.contains("\\\"")
                || full_match.contains("\\'")
                || full_match.contains("&#")
                || full_match.starts_with("<astro")
            {
                continue;
            }

            // Skip numeric values
            if value.trim().is_empty() || value.parse::<f64>().is_ok() {
                continue;
            }

            issues.push(format!(
                "The attribute '{}' has a value '{}' not enclosed in quotes in tag {}",
                attribute, value, full_match
            ));
            self.stats.add_warning(ANALYSIS_MISSING_QUOTES, Some(full_match));
        }

        if !issues.is_empty() {
            result.add_warning(
                format!("{} attribute(s) with missing quotes", issues.len()),
                ANALYSIS_MISSING_QUOTES,
                Some(issues),
            );
            self.pages_with_missing_quotes += 1;
        }
    }

    fn check_max_dom_depth(&mut self, html: &str, url: &str, result: &mut UrlAnalysisResult) {
        let document = Html::parse_document(html);

        // Find the body element and compute max depth
        let body_selector = match Selector::parse("body") {
            Ok(s) => s,
            Err(_) => return,
        };

        let body_node_id = match document.select(&body_selector).next() {
            Some(b) => b.id(),
            None => return,
        };

        let body_node = match document.tree.get(body_node_id) {
            Some(n) => n,
            None => return,
        };

        let max_depth = find_max_depth(body_node, 0);

        if max_depth >= self.max_dom_depth_critical {
            let msg = format!(
                "The DOM depth exceeds the critical limit: {}. Found depth: {}.",
                self.max_dom_depth_critical, max_depth
            );
            result.add_critical(msg.clone(), ANALYSIS_DOM_DEPTH, Some(vec![msg]));
            self.stats.add_critical(ANALYSIS_DOM_DEPTH, Some(url));
            self.pages_with_deep_dom += 1;
        } else if max_depth >= self.max_dom_depth_warning {
            let msg = format!(
                "The DOM depth exceeds the warning limit: {}. Found depth: {}.",
                self.max_dom_depth_warning, max_depth
            );
            result.add_warning(msg.clone(), ANALYSIS_DOM_DEPTH, Some(vec![msg]));
            self.stats.add_warning(ANALYSIS_DOM_DEPTH, Some(url));
            self.pages_with_deep_dom += 1;
        } else {
            result.add_ok(
                format!("The DOM depth is within acceptable limits. Found depth: {}", max_depth),
                ANALYSIS_DOM_DEPTH,
                None,
            );
            self.stats.add_ok(ANALYSIS_DOM_DEPTH, Some(url));
        }
    }

    fn check_heading_structure(&mut self, html: &str, result: &mut UrlAnalysisResult) {
        let document = Html::parse_document(html);

        let heading_selector = match Selector::parse("h1, h2, h3, h4, h5, h6") {
            Ok(s) => s,
            Err(_) => return,
        };

        let headings: Vec<(i32, String)> = document
            .select(&heading_selector)
            .filter_map(|el| {
                // Skip headings inside SVG, script, style, template, noscript
                // (headings inside foreign content are not relevant)
                let mut parent = el.parent();
                while let Some(p) = parent {
                    if let Some(p_el) = p.value().as_element() {
                        match p_el.name() {
                            "svg" | "script" | "style" | "template" | "noscript" => return None,
                            _ => {}
                        }
                    }
                    parent = p.parent();
                }

                let tag = el.value().name();
                let level = tag.strip_prefix('h').and_then(|s| s.parse::<i32>().ok())?;
                let text = el.text().collect::<String>();
                Some((level, text))
            })
            .collect();

        if headings.is_empty() {
            result.add_notice(
                "No headings found in the HTML content.".to_string(),
                ANALYSIS_HEADING_STRUCTURE,
                Some(vec!["No headings found in the HTML content.".to_string()]),
            );
            self.stats.add_notice(ANALYSIS_HEADING_STRUCTURE, Some(html));
            return;
        }

        let mut warning_issues: Vec<String> = Vec::new();
        let mut critical_issues: Vec<String> = Vec::new();
        let mut found_h1 = false;
        let mut previous_level = 0i32;

        for (level, _text) in &headings {
            let current_level = *level;

            if current_level == 1 {
                if found_h1 {
                    critical_issues.push("Multiple <h1> headings found.".to_string());
                    self.stats.add_critical(
                        ANALYSIS_HEADING_STRUCTURE,
                        Some(&format!("{} - multiple h1 tags found", html)),
                    );
                } else {
                    found_h1 = true;
                }
            }

            if current_level > previous_level + 1 {
                let msg = if previous_level > 0 {
                    format!(
                        "Heading structure is skipping levels: found an <h{}> after an <h{}>.",
                        current_level, previous_level
                    )
                } else {
                    format!(
                        "Heading structure is skipping levels: found an <h{}> without a previous higher heading.",
                        current_level
                    )
                };
                warning_issues.push(msg);
                self.stats.add_warning(
                    ANALYSIS_HEADING_STRUCTURE,
                    Some(&format!(
                        "{} - found <h{}> {}",
                        html,
                        current_level,
                        if previous_level > 0 {
                            format!("after an <h{}>.", previous_level)
                        } else {
                            "without a previous higher heading.".to_string()
                        }
                    )),
                );
            }

            previous_level = current_level;
        }

        if !found_h1 {
            critical_issues.push("No <h1> tag found in the HTML content.".to_string());
            self.pages_without_h1 += 1;
        } else {
            result.add_ok(
                "At least one h1 tag was found.".to_string(),
                ANALYSIS_HEADING_STRUCTURE,
                None,
            );
            self.stats.add_ok(
                ANALYSIS_HEADING_STRUCTURE,
                Some(&format!("{} - at least one h1 tag found", html)),
            );

            if !critical_issues.is_empty() {
                self.pages_with_multiple_h1 += 1;
            }
        }

        let has_critical = !critical_issues.is_empty();
        let has_warning = !warning_issues.is_empty();
        let critical_count = critical_issues.len();
        let warning_count = warning_issues.len();

        if has_critical {
            if !found_h1 {
                result.add_critical(
                    "No <h1> found.".to_string(),
                    ANALYSIS_HEADING_STRUCTURE,
                    Some(critical_issues),
                );
            } else {
                result.add_critical(
                    format!("Up to {} headings <h1> found.", critical_count + 1),
                    ANALYSIS_HEADING_STRUCTURE,
                    Some(critical_issues),
                );
            }
        }
        if has_warning {
            result.add_warning(
                format!("{} heading structure issue(s) found.", warning_count),
                ANALYSIS_HEADING_STRUCTURE,
                Some(warning_issues),
            );
            self.pages_with_skipped_heading_levels += 1;
        }
        if !has_critical && !has_warning {
            result.add_ok(
                "Heading structure is valid.".to_string(),
                ANALYSIS_HEADING_STRUCTURE,
                None,
            );
            self.stats.add_ok(
                ANALYSIS_HEADING_STRUCTURE,
                Some(&format!("{} - heading structure is valid", html)),
            );
        }
    }

    fn check_non_clickable_phone_numbers(&mut self, html: &str, result: &mut UrlAnalysisResult) {
        let all_phones = parse_phone_numbers_from_html(html, false);
        let non_clickable = parse_phone_numbers_from_html(html, true);

        if !non_clickable.is_empty() {
            result.add_warning(
                format!("{} non-clickable phone number(s) found.", non_clickable.len()),
                ANALYSIS_NON_CLICKABLE_PHONE_NUMBERS,
                Some(non_clickable.clone()),
            );
            for phone in &non_clickable {
                self.stats
                    .add_warning(ANALYSIS_NON_CLICKABLE_PHONE_NUMBERS, Some(phone));
            }
            self.pages_with_non_clickable_phone_numbers += 1;
        } else {
            result.add_ok(
                "No non-clickable phone numbers found.".to_string(),
                ANALYSIS_NON_CLICKABLE_PHONE_NUMBERS,
                None,
            );
            for phone in &all_phones {
                if !non_clickable.contains(phone) {
                    self.stats.add_ok(ANALYSIS_NON_CLICKABLE_PHONE_NUMBERS, Some(phone));
                }
            }
        }
    }

    fn check_title_uniqueness(
        &mut self,
        titles: &[Option<String>],
        status: &Status,
        output: &mut dyn Output,
    ) -> HashMap<String, String> {
        let summary_code = "title-uniqueness";

        // Check unfiltered array first, then filter nulls
        if titles.is_empty() {
            status.add_warning_to_summary(summary_code, "No titles provided for uniqueness check.");
            return Self::get_analysis_result(ANALYSIS_TITLE_UNIQUENESS, 0, 0, 1, 0);
        }

        let filtered: Vec<&str> = titles.iter().filter_map(|t| t.as_deref()).collect();

        if filtered.len() <= 1 {
            status.add_ok_to_summary(summary_code, "Only one title provided for uniqueness check.");
            return Self::get_analysis_result(ANALYSIS_TITLE_UNIQUENESS, 1, 0, 0, 0);
        }

        let mut counts: HashMap<&str, usize> = HashMap::new();
        for title in &filtered {
            *counts.entry(title).or_insert(0) += 1;
        }

        let total = filtered.len();
        let mut ok = 0usize;
        let mut warnings = 0usize;
        let mut highest_pct = 0usize;
        let mut non_unique_found = false;

        for (title, count) in &counts {
            let pct = (*count * 100) / total;
            highest_pct = highest_pct.max(pct);

            if *count > 1 && pct > self.title_uniqueness_percentage {
                status.add_warning_to_summary(
                    summary_code,
                    &format!(
                        "The title '{}' exceeds the allowed {}% duplicity. {}% of pages have this same title.",
                        title, self.title_uniqueness_percentage, pct
                    ),
                );
                non_unique_found = true;
                warnings += 1;
            } else {
                ok += 1;
            }
        }

        // Build top non-unique titles table
        let mut sorted_counts: Vec<(&str, usize)> = counts.into_iter().collect();
        sorted_counts.sort_by(|a, b| b.1.cmp(&a.1));

        let mut top_titles_data: Vec<HashMap<String, String>> = Vec::new();
        for (title, count) in sorted_counts.iter().take(10) {
            if *count > 1 {
                let mut row = HashMap::new();
                row.insert("count".to_string(), count.to_string());
                row.insert("title".to_string(), title.to_string());
                top_titles_data.push(row);
            }
        }

        let console_width = utils::get_console_width();
        let title_col_width = (console_width as i32 - 10).clamp(20, 200);

        let columns = vec![
            SuperTableColumn::new(
                "count".to_string(),
                "Count".to_string(),
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
                "title".to_string(),
                "Title".to_string(),
                title_col_width,
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
            SUPER_TABLE_NON_UNIQUE_TITLES.to_string(),
            "TOP non-unique titles".to_string(),
            "Nothing to report.".to_string(),
            columns,
            true,
            Some("count".to_string()),
            "DESC".to_string(),
            None,
            None,
            None,
        );
        super_table.set_data(top_titles_data);
        status.configure_super_table_url_stripping(&mut super_table);
        output.add_super_table(&super_table);
        status.add_super_table_at_end(super_table);

        if !non_unique_found {
            status.add_ok_to_summary(
                summary_code,
                &format!(
                    "All {} unique title(s) are within the allowed {}% duplicity. Highest duplicity title has {}%.",
                    ok, self.title_uniqueness_percentage, highest_pct
                ),
            );
        }

        Self::get_analysis_result(ANALYSIS_TITLE_UNIQUENESS, ok, 0, warnings, 0)
    }

    fn check_meta_description_uniqueness(
        &mut self,
        descriptions: &[Option<String>],
        status: &Status,
        output: &mut dyn Output,
    ) -> HashMap<String, String> {
        let summary_code = "meta-description-uniqueness";

        // Include empty strings for pages without descriptions
        let filtered: Vec<&str> = descriptions.iter().map(|d| d.as_deref().unwrap_or("")).collect();

        if filtered.is_empty() {
            status.add_warning_to_summary(summary_code, "No meta descriptions provided for uniqueness check.");
            return Self::get_analysis_result(ANALYSIS_DESCRIPTION_UNIQUENESS, 0, 0, 1, 0);
        }

        if filtered.len() <= 1 {
            status.add_ok_to_summary(summary_code, "Only one meta description provided for uniqueness check.");
            return Self::get_analysis_result(ANALYSIS_DESCRIPTION_UNIQUENESS, 1, 0, 0, 0);
        }

        let mut counts: HashMap<&str, usize> = HashMap::new();
        for desc in &filtered {
            *counts.entry(desc).or_insert(0) += 1;
        }

        let total = filtered.len();
        let mut ok = 0usize;
        let mut warnings = 0usize;
        let mut highest_pct = 0usize;
        let mut non_unique_found = false;

        for (desc, count) in &counts {
            let pct = (*count * 100) / total;
            highest_pct = highest_pct.max(pct);

            if *count > 1 && pct > self.meta_description_uniqueness_percentage {
                status.add_warning_to_summary(
                    summary_code,
                    &format!(
                        "The description '{}' exceeds the allowed {}% duplicity. {}% of pages have this same description.",
                        desc, self.meta_description_uniqueness_percentage, pct
                    ),
                );
                non_unique_found = true;
                warnings += 1;
            } else {
                ok += 1;
            }
        }

        let mut sorted_counts: Vec<(&str, usize)> = counts.into_iter().collect();
        sorted_counts.sort_by(|a, b| b.1.cmp(&a.1));

        let mut top_desc_data: Vec<HashMap<String, String>> = Vec::new();
        for (desc, count) in sorted_counts.iter().take(10) {
            if *count > 1 {
                let mut row = HashMap::new();
                row.insert("count".to_string(), count.to_string());
                row.insert("description".to_string(), desc.to_string());
                top_desc_data.push(row);
            }
        }

        let console_width = utils::get_console_width();
        let desc_col_width = (console_width as i32 - 10).clamp(20, 200);

        let columns = vec![
            SuperTableColumn::new(
                "count".to_string(),
                "Count".to_string(),
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
                "description".to_string(),
                "Description".to_string(),
                desc_col_width,
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
            SUPER_TABLE_NON_UNIQUE_DESCRIPTIONS.to_string(),
            "TOP non-unique descriptions".to_string(),
            "Nothing to report.".to_string(),
            columns,
            true,
            Some("count".to_string()),
            "DESC".to_string(),
            None,
            None,
            None,
        );
        super_table.set_data(top_desc_data);
        status.configure_super_table_url_stripping(&mut super_table);
        output.add_super_table(&super_table);
        status.add_super_table_at_end(super_table);

        if !non_unique_found {
            status.add_ok_to_summary(
                summary_code,
                &format!(
                    "All {} description(s) are within the allowed {}% duplicity. Highest duplicity description has {}%.",
                    ok, self.meta_description_uniqueness_percentage, highest_pct
                ),
            );
        }

        Self::get_analysis_result(ANALYSIS_DESCRIPTION_UNIQUENESS, ok, 0, warnings, 0)
    }

    fn check_brotli_support(&self, urls: &[&VisitedUrl], status: &Status) -> HashMap<String, String> {
        let summary_code = "brotli-support";
        let without_brotli = urls
            .iter()
            .filter(|u| u.content_encoding.as_deref() != Some("br"))
            .count();
        let with_brotli = urls.len().saturating_sub(without_brotli);

        if without_brotli > 0 {
            status.add_warning_to_summary(
                summary_code,
                &format!("{} page(s) do not support Brotli compression.", without_brotli),
            );
        } else {
            status.add_ok_to_summary(summary_code, "All pages support Brotli compression.");
        }

        Self::get_analysis_result(ANALYSIS_BROTLI_SUPPORT, with_brotli, 0, without_brotli, 0)
    }

    fn check_webp_support(&self, urls: &[&VisitedUrl], status: &Status) -> HashMap<String, String> {
        let summary_code = "webp-support";
        let webp_count = urls
            .iter()
            .filter(|u| u.content_type_header.as_deref() == Some("image/webp"))
            .count();
        let avif_count = urls
            .iter()
            .filter(|u| u.content_type_header.as_deref() == Some("image/avif"))
            .count();

        if webp_count > 0 {
            status.add_ok_to_summary(
                summary_code,
                &format!("{} WebP image(s) found on the website.", webp_count),
            );
        } else if avif_count > 0 {
            status.add_ok_to_summary(
                summary_code,
                &format!(
                    "No WebP images found, but AVIF (more modern format) is supported with {} image(s).",
                    avif_count
                ),
            );
            return Self::get_analysis_result(ANALYSIS_WEBP_SUPPORT, 1, 0, 0, 0);
        } else {
            status.add_warning_to_summary(summary_code, "No WebP image found on the website.");
        }

        Self::get_analysis_result(
            ANALYSIS_WEBP_SUPPORT,
            webp_count,
            0,
            if webp_count > 0 { 0 } else { 1 },
            0,
        )
    }

    fn check_avif_support(&self, urls: &[&VisitedUrl], status: &Status) -> HashMap<String, String> {
        let summary_code = "avif-support";
        let avif_count = urls
            .iter()
            .filter(|u| u.content_type_header.as_deref() == Some("image/avif"))
            .count();

        if avif_count > 0 {
            status.add_ok_to_summary(
                summary_code,
                &format!("{} AVIF image(s) found on the website.", avif_count),
            );
        } else {
            status.add_warning_to_summary(summary_code, "No AVIF image found on the website.");
        }

        Self::get_analysis_result(
            ANALYSIS_AVIF_SUPPORT,
            avif_count,
            0,
            if avif_count > 0 { 0 } else { 1 },
            0,
        )
    }

    fn set_findings_to_summary(&self, status: &Status) {
        // Missing quotes
        if self.pages_with_missing_quotes > 0 {
            status.add_warning_to_summary(
                "pages-with-missing-quotes",
                &format!(
                    "{} page(s) with missing quotes on attributes",
                    self.pages_with_missing_quotes
                ),
            );
        } else {
            status.add_ok_to_summary("pages-with-missing-quotes", "All pages have quoted attributes");
        }

        // Inline SVGs
        if self.pages_with_large_svgs > 0 {
            status.add_warning_to_summary(
                "pages-with-large-svgs",
                &format!(
                    "{} page(s) with large inline SVGs (> {} bytes)",
                    self.pages_with_large_svgs, self.max_inline_svg_size
                ),
            );
        } else {
            status.add_ok_to_summary(
                "pages-with-large-svgs",
                &format!(
                    "All pages have inline SVGs smaller than {} bytes",
                    self.max_inline_svg_size
                ),
            );
        }

        if self.pages_with_duplicated_svgs > 0 {
            status.add_warning_to_summary(
                "pages-with-duplicated-svgs",
                &format!(
                    "{} page(s) with duplicated inline SVGs (> {} duplicates)",
                    self.pages_with_duplicated_svgs, self.max_inline_svg_duplicates
                ),
            );
        } else {
            status.add_ok_to_summary(
                "pages-with-duplicated-svgs",
                &format!(
                    "All pages have inline SVGs with less than {} duplicates",
                    self.max_inline_svg_duplicates
                ),
            );
        }

        if self.pages_with_invalid_svgs > 0 {
            status.add_warning_to_summary(
                "pages-with-invalid-svgs",
                &format!("{} page(s) with invalid inline SVGs", self.pages_with_invalid_svgs),
            );
        } else {
            status.add_ok_to_summary("pages-with-invalid-svgs", "All pages have valid or none inline SVGs");
        }

        // Heading structure
        if self.pages_with_multiple_h1 > 0 {
            status.add_critical_to_summary(
                "pages-with-multiple-h1",
                &format!("{} page(s) with multiple <h1> headings", self.pages_with_multiple_h1),
            );
        } else {
            status.add_ok_to_summary("pages-with-multiple-h1", "All pages without multiple <h1> headings");
        }

        if self.pages_without_h1 > 0 {
            status.add_critical_to_summary(
                "pages-without-h1",
                &format!("{} page(s) without <h1> heading", self.pages_without_h1),
            );
        } else {
            status.add_ok_to_summary("pages-without-h1", "All pages have <h1> heading");
        }

        if self.pages_with_skipped_heading_levels > 0 {
            status.add_warning_to_summary(
                "pages-with-skipped-heading-levels",
                &format!(
                    "{} page(s) with skipped heading levels",
                    self.pages_with_skipped_heading_levels
                ),
            );
        } else {
            status.add_ok_to_summary(
                "pages-with-skipped-heading-levels",
                "All pages have heading structure without skipped levels",
            );
        }

        // DOM depth
        if self.pages_with_deep_dom > 0 {
            status.add_warning_to_summary(
                "pages-with-deep-dom",
                &format!(
                    "{} page(s) with deep DOM (> {} levels)",
                    self.pages_with_deep_dom, self.max_dom_depth_warning
                ),
            );
        } else {
            status.add_ok_to_summary(
                "pages-with-deep-dom",
                &format!("All pages have DOM depth less than {}", self.max_dom_depth_warning),
            );
        }

        // Non-clickable phone numbers
        if self.pages_with_non_clickable_phone_numbers > 0 {
            status.add_warning_to_summary(
                "pages-with-non-clickable-phone-numbers",
                &format!(
                    "{} page(s) with non-clickable (non-interactive) phone numbers",
                    self.pages_with_non_clickable_phone_numbers
                ),
            );
        } else {
            status.add_ok_to_summary(
                "pages-with-non-clickable-phone-numbers",
                "All pages have clickable (interactive) phone numbers",
            );
        }
    }
}

impl Analyzer for BestPracticeAnalyzer {
    fn analyze(&mut self, status: &Status, output: &mut dyn Output) {
        let max_svg_size = self.max_inline_svg_size;
        let max_svg_dup = self.max_inline_svg_duplicates;
        let max_svg_dup_size = self.max_inline_svg_duplicate_size;
        let max_dom_depth = self.max_dom_depth_warning;
        let title_pct = self.title_uniqueness_percentage;
        let desc_pct = self.meta_description_uniqueness_percentage;

        let columns = vec![
            SuperTableColumn::new(
                "analysisName".to_string(),
                "Analysis name".to_string(),
                -1, // AUTO_WIDTH
                Some(Box::new(move |value: &str, _render_into: &str| match value {
                    "Large inline SVGs" => format!("{} (> {} B)", value, max_svg_size),
                    "Duplicate inline SVGs" => format!("{} (> {} and > {} B)", value, max_svg_dup, max_svg_dup_size),
                    "DOM depth" => format!("{} (> {})", value, max_dom_depth),
                    "Title uniqueness" => format!("{} (> {}%)", value, title_pct),
                    "Description uniqueness" => format!("{} (> {}%)", value, desc_pct),
                    _ => value.to_string(),
                })),
                None,
                false,
                true,
                false,
                true,
                None,
            ),
            SuperTableColumn::new(
                "ok".to_string(),
                "OK".to_string(),
                5,
                Some(Box::new(|value: &str, _render_into: &str| {
                    if let Ok(v) = value.parse::<usize>()
                        && v > 0
                    {
                        return utils::get_color_text(&v.to_string(), "green", false);
                    }
                    "0".to_string()
                })),
                None,
                false,
                false,
                false,
                true,
                None,
            ),
            SuperTableColumn::new(
                "notice".to_string(),
                "Notice".to_string(),
                6,
                Some(Box::new(|value: &str, _render_into: &str| {
                    if let Ok(v) = value.parse::<usize>()
                        && v > 0
                    {
                        return utils::get_color_text(&v.to_string(), "blue", false);
                    }
                    "0".to_string()
                })),
                None,
                false,
                false, // color-only formatter doesn't change visible length
                false,
                true,
                None,
            ),
            SuperTableColumn::new(
                "warning".to_string(),
                "Warning".to_string(),
                7,
                Some(Box::new(|value: &str, _render_into: &str| {
                    if let Ok(v) = value.parse::<usize>()
                        && v > 0
                    {
                        return utils::get_color_text(&v.to_string(), "magenta", true);
                    }
                    "0".to_string()
                })),
                None,
                false,
                false, // color-only formatter doesn't change visible length
                false,
                true,
                None,
            ),
            SuperTableColumn::new(
                "critical".to_string(),
                "Critical".to_string(),
                8,
                Some(Box::new(|value: &str, _render_into: &str| {
                    if let Ok(v) = value.parse::<usize>()
                        && v > 0
                    {
                        return utils::get_color_text(&v.to_string(), "red", true);
                    }
                    "0".to_string()
                })),
                None,
                false,
                false, // color-only formatter doesn't change visible length
                false,
                true,
                None,
            ),
        ];

        let data = self.analyze_urls(status, output);

        let mut super_table = SuperTable::new(
            SUPER_TABLE_BEST_PRACTICES.to_string(),
            "Best practices".to_string(),
            "Nothing to report.".to_string(),
            columns,
            true,
            None,
            "ASC".to_string(),
            None,
            None,
            None,
        );

        super_table.set_data(data);
        status.configure_super_table_url_stripping(&mut super_table);
        output.add_super_table(&super_table);
        status.add_super_table_at_end(super_table);

        self.set_findings_to_summary(status);
    }

    fn analyze_visited_url(
        &mut self,
        visited_url: &VisitedUrl,
        body: Option<&str>,
        _headers: Option<&HashMap<String, String>>,
    ) -> Option<UrlAnalysisResult> {
        let is_html = visited_url.content_type == ContentTypeId::Html && body.is_some();

        if !is_html {
            return None;
        }

        let html = body?;
        let mut result = UrlAnalysisResult::new();

        let s = Instant::now();
        self.check_inline_svg(html, &mut result);
        self.base.measure_exec_time("BestPracticeAnalyzer", "checkInlineSvg", s);

        let s = Instant::now();
        self.check_missing_quotes_on_attributes(html, &mut result);
        self.base
            .measure_exec_time("BestPracticeAnalyzer", "checkMissingQuotesOnAttributes", s);

        let s = Instant::now();
        self.check_max_dom_depth(html, &visited_url.url, &mut result);
        self.base
            .measure_exec_time("BestPracticeAnalyzer", "checkMaxDOMDepth", s);

        let s = Instant::now();
        self.check_heading_structure(html, &mut result);
        self.base
            .measure_exec_time("BestPracticeAnalyzer", "checkHeadingStructure", s);

        let s = Instant::now();
        self.check_non_clickable_phone_numbers(html, &mut result);
        self.base
            .measure_exec_time("BestPracticeAnalyzer", "checkNonClickablePhoneNumbers", s);

        Some(result)
    }

    fn show_analyzed_visited_url_result_as_column(&self) -> Option<ExtraColumn> {
        ExtraColumn::new("Best pr.".to_string(), Some(8), false, None, None, None).ok()
    }

    fn should_be_activated(&self) -> bool {
        true
    }

    fn get_order(&self) -> i32 {
        170
    }

    fn get_name(&self) -> &str {
        "BestPracticeAnalyzer"
    }

    fn get_exec_times(&self) -> &HashMap<String, f64> {
        self.base.get_exec_times()
    }

    fn get_exec_counts(&self) -> &HashMap<String, usize> {
        self.base.get_exec_counts()
    }
}

/// Validate SVG XML and return None for valid or Some(errors) for invalid
fn validate_svg(svg: &str) -> Option<Vec<String>> {
    use quick_xml::Reader;
    use quick_xml::events::Event;

    let mut reader = Reader::from_str(svg);
    let mut errors = Vec::new();

    loop {
        match reader.read_event() {
            Ok(Event::Eof) => break,
            Ok(_) => {}
            Err(e) => {
                errors.push(format!("{}", e));
            }
        }
    }

    if errors.is_empty() { None } else { Some(errors) }
}

/// Sanitize SVG: remove content, keep only the opening tag
fn sanitize_svg(svg: &str) -> String {
    if let Some(end) = svg.find('>') {
        format!("{}> ...", &svg[..end])
    } else {
        svg.to_string()
    }
}

/// Find max DOM depth using the scraper tree
fn find_max_depth(node_ref: ego_tree::NodeRef<scraper::Node>, depth: usize) -> usize {
    let mut max = depth;
    for child in node_ref.children() {
        let child_depth = find_max_depth(child, depth + 1);
        max = max.max(child_depth);
    }
    max
}

/// Parse phone numbers from HTML. Returns numbers found outside tel: links if only_non_clickable is true.
fn parse_phone_numbers_from_html(html: &str, only_non_clickable: bool) -> Vec<String> {
    use once_cell::sync::Lazy;
    // Formats with country codes and spaces, e.g.: +420 123 456 789 or +1234 1234567890
    static RE_PHONE_COUNTRY: Lazy<Regex> = Lazy::new(|| Regex::new(r"\+\d{1,4}(\s?[0-9\- ]{1,5}){1,5}").unwrap());
    // Formats with country codes without spaces, e.g.: +420123456789
    static RE_PHONE_NO_SPACE: Lazy<Regex> = Lazy::new(|| Regex::new(r"\+[0-9\- ]{7,20}").unwrap());
    // US format with parentheses, e.g.: (123) 456-7890
    static RE_PHONE_US: Lazy<Regex> = Lazy::new(|| Regex::new(r"\(\d{1,5}\)\s?\d{3,4}-\d{4}").unwrap());
    // Regular format with dashes, e.g.: 123-456-7890
    static RE_PHONE_DASH: Lazy<Regex> = Lazy::new(|| Regex::new(r"\d{1,5}-\d{3,4}-\d{4}").unwrap());

    let mut phones: Vec<String> = Vec::new();

    // Strip JavaScript and CSS content first (phone numbers are not visible in these)
    let html_clean = strip_js_and_css(html);

    // Replace &nbsp; with space
    let html_clean = html_clean.replace("&nbsp;", " ");

    let phone_regexes: [&Regex; 4] = [&RE_PHONE_COUNTRY, &RE_PHONE_NO_SPACE, &RE_PHONE_US, &RE_PHONE_DASH];
    for re in &phone_regexes {
        for m in re.find_iter(&html_clean) {
            let phone = m.as_str().trim().to_string();
            if !phones.contains(&phone) {
                phones.push(phone);
            }
        }
    }

    // Filter: phone number must be at least 8 chars
    phones.retain(|p| p.len() >= 8);

    if only_non_clickable {
        phones.retain(|phone| {
            let escaped = regex::escape(phone);

            // Check pattern 1: <a href="tel:PHONE">...</a>
            let tel_pattern1 = format!(r#"<a[^>]*href=["']tel:{}["'][^>]*>.*?</a>"#, escaped);
            let in_tel1 = Regex::new(&tel_pattern1).map(|re| re.is_match(html)).unwrap_or(false);

            // Check pattern 2: <a href="tel:...">...PHONE...</a>
            let tel_pattern2 = format!(r#"(?is)<a[^>]*href=["']tel:[^"'>]+["'][^>]*>.*?{}.*?</a>"#, escaped);
            let in_tel2 = Regex::new(&tel_pattern2).map(|re| re.is_match(html)).unwrap_or(false);

            // Check unwanted pattern: phone number is part of a larger alphanumeric string
            let unwanted_pattern = format!(r"(?i)[0-9a-z._-]{}[0-9a-z._-]", escaped);
            let is_unwanted = Regex::new(&unwanted_pattern)
                .map(|re| re.is_match(html))
                .unwrap_or(false);

            !in_tel1 && !in_tel2 && !is_unwanted
        });
    }

    phones
}

/// Strip JavaScript content from HTML
fn strip_js_and_css(html: &str) -> String {
    use once_cell::sync::Lazy;
    static RE_SCRIPT: Lazy<Regex> = Lazy::new(|| Regex::new(r"(?is)<script[^>]*>.*?</script>").unwrap());
    static RE_STYLE: Lazy<Regex> = Lazy::new(|| Regex::new(r"(?is)<style[^>]*>.*?</style>").unwrap());

    let result = RE_SCRIPT.replace_all(html, " ").to_string();
    RE_STYLE.replace_all(&result, " ").to_string()
}
