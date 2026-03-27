// SiteOne Crawler - CI/CD Quality Gate
// (c) Jan Reges <jan.reges@siteone.cz>
//
// Evaluates crawler results against configurable thresholds.
// Returns exit code 10 when any check fails.

use serde::Serialize;

use crate::components::summary::item_status::ItemStatus;
use crate::components::summary::summary::Summary;
use crate::options::core_options::CoreOptions;
use crate::output::output::BasicStats;
use crate::scoring::quality_score::QualityScores;
use crate::types::ContentTypeId;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CiCheck {
    pub metric: String,
    pub operator: String,
    pub threshold: f64,
    pub actual: f64,
    pub passed: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CiGateResult {
    pub passed: bool,
    pub exit_code: i32,
    pub checks: Vec<CiCheck>,
}

pub fn evaluate(options: &CoreOptions, scores: &QualityScores, stats: &BasicStats, summary: &Summary) -> CiGateResult {
    let mut checks = Vec::new();

    // If no pages were successfully crawled, fail immediately.
    // URLs with negative status codes (-1 connection error, -2 timeout, etc.) don't count.
    let has_successful_response = stats.count_by_status.keys().any(|&code| code > 0);
    if stats.total_urls == 0 || !has_successful_response {
        checks.push(CiCheck {
            metric: "Pages crawled".to_string(),
            operator: ">".to_string(),
            threshold: 0.0,
            actual: 0.0,
            passed: false,
        });
        return CiGateResult {
            passed: false,
            exit_code: 10,
            checks,
        };
    }

    // Overall score
    checks.push(check_min("Overall score", scores.overall.score, options.ci_min_score));

    // Category scores
    if let Some(threshold) = options.ci_min_performance {
        let actual = find_category_score(scores, "performance");
        checks.push(check_min("Performance score", actual, threshold));
    }
    if let Some(threshold) = options.ci_min_seo {
        let actual = find_category_score(scores, "seo");
        checks.push(check_min("SEO score", actual, threshold));
    }
    if let Some(threshold) = options.ci_min_security {
        let actual = find_category_score(scores, "security");
        checks.push(check_min("Security score", actual, threshold));
    }
    if let Some(threshold) = options.ci_min_accessibility {
        let actual = find_category_score(scores, "accessibility");
        checks.push(check_min("Accessibility score", actual, threshold));
    }
    if let Some(threshold) = options.ci_min_best_practices {
        let actual = find_category_score(scores, "best-practices");
        checks.push(check_min("Best Practices score", actual, threshold));
    }

    // 404 errors
    let count_404 = stats.count_by_status.get(&404).copied().unwrap_or(0) as f64;
    checks.push(check_max("404 errors", count_404, options.ci_max_404 as f64));

    // 5xx errors
    let count_5xx: usize = stats
        .count_by_status
        .iter()
        .filter(|&(&code, _)| (500..600).contains(&code))
        .map(|(_, &count)| count)
        .sum();
    checks.push(check_max("5xx errors", count_5xx as f64, options.ci_max_5xx as f64));

    // Critical findings
    let criticals = summary.get_count_by_item_status(ItemStatus::Critical) as f64;
    checks.push(check_max(
        "Critical findings",
        criticals,
        options.ci_max_criticals as f64,
    ));

    // Warning findings (optional)
    if let Some(max_warnings) = options.ci_max_warnings {
        let warnings = summary.get_count_by_item_status(ItemStatus::Warning) as f64;
        checks.push(check_max("Warning findings", warnings, max_warnings as f64));
    }

    // Average response time (optional)
    if let Some(max_avg) = options.ci_max_avg_response {
        checks.push(check_max(
            "Avg response time (s)",
            stats.total_requests_times_avg,
            max_avg,
        ));
    }

    // Minimum content type counts
    let pages = count_content_types(stats, &[ContentTypeId::Html]);
    checks.push(check_min("HTML pages", pages as f64, options.ci_min_pages as f64));

    let assets = count_content_types(
        stats,
        &[
            ContentTypeId::Script,
            ContentTypeId::Stylesheet,
            ContentTypeId::Image,
            ContentTypeId::Font,
        ],
    );
    checks.push(check_min(
        "Assets (JS/CSS/img/font)",
        assets as f64,
        options.ci_min_assets as f64,
    ));

    if options.ci_min_documents > 0 {
        let documents = count_content_types(stats, &[ContentTypeId::Document]);
        checks.push(check_min(
            "Documents",
            documents as f64,
            options.ci_min_documents as f64,
        ));
    }

    let passed = checks.iter().all(|c| c.passed);
    CiGateResult {
        passed,
        exit_code: if passed { 0 } else { 10 },
        checks,
    }
}

fn check_min(metric: &str, actual: f64, threshold: f64) -> CiCheck {
    CiCheck {
        metric: metric.to_string(),
        operator: ">=".to_string(),
        threshold,
        actual,
        passed: actual >= threshold,
    }
}

fn check_max(metric: &str, actual: f64, threshold: f64) -> CiCheck {
    CiCheck {
        metric: metric.to_string(),
        operator: "<=".to_string(),
        threshold,
        actual,
        passed: actual <= threshold,
    }
}

fn count_content_types(stats: &BasicStats, types: &[ContentTypeId]) -> usize {
    types
        .iter()
        .map(|t| stats.count_by_content_type.get(&(*t as i32)).copied().unwrap_or(0))
        .sum()
}

fn find_category_score(scores: &QualityScores, code: &str) -> f64 {
    scores
        .categories
        .iter()
        .find(|c| c.code == code)
        .map(|c| c.score)
        .unwrap_or(0.0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::components::summary::item::Item;
    use crate::scoring::quality_score::{CategoryScore, score_label};
    use std::collections::BTreeMap;

    fn make_options() -> CoreOptions {
        CoreOptions {
            url: "https://test.com".to_string(),
            single_page: false,
            max_depth: 0,
            device: crate::types::DeviceType::Desktop,
            user_agent: None,
            timeout: 5,
            proxy: None,
            http_auth: None,
            accept_invalid_certs: false,
            timezone: None,
            show_version_only: false,
            show_help_only: false,
            output_type: crate::types::OutputType::Text,
            url_column_size: None,
            show_inline_criticals: false,
            show_inline_warnings: false,
            rows_limit: 200,
            extra_columns: Vec::new(),
            extra_columns_names_only: Vec::new(),
            show_scheme_and_host: false,
            do_not_truncate_url: false,
            hide_progress_bar: false,
            hide_columns: Vec::new(),
            no_color: false,
            force_color: false,
            console_width: None,
            disable_all_assets: false,
            disable_javascript: false,
            disable_styles: false,
            disable_fonts: false,
            disable_images: false,
            disable_files: false,
            remove_all_anchor_listeners: false,
            workers: 3,
            max_reqs_per_sec: 10.0,
            memory_limit: "2048M".to_string(),
            resolve: Vec::new(),
            websocket_server: None,
            ignore_robots_txt: false,
            allowed_domains_for_external_files: Vec::new(),
            allowed_domains_for_crawling: Vec::new(),
            single_foreign_page: false,
            result_storage: crate::options::core_options::StorageType::Memory,
            result_storage_dir: "tmp/result-storage".to_string(),
            result_storage_compression: false,
            accept_encoding: "gzip, deflate, br".to_string(),
            max_queue_length: 9000,
            max_visited_urls: 10000,
            max_url_length: 2083,
            max_skipped_urls: 10000,
            max_non200_responses_per_basename: 5,
            include_regex: Vec::new(),
            ignore_regex: Vec::new(),
            regex_filtering_only_for_pages: false,
            analyzer_filter_regex: None,
            add_random_query_params: false,
            remove_query_params: false,
            keep_query_params: Vec::new(),
            transform_url: Vec::new(),
            force_relative_urls: false,
            output_html_report: None,
            html_report_options: None,
            output_json_file: None,
            output_text_file: None,
            add_host_to_output_file: false,
            add_timestamp_to_output_file: false,
            sitemap_xml_file: None,
            sitemap_txt_file: None,
            sitemap_base_priority: 0.5,
            sitemap_priority_increase: 0.1,
            offline_export_dir: None,
            offline_export_store_only_url_regex: Vec::new(),
            offline_export_remove_unwanted_code: true,
            offline_export_no_auto_redirect_html: false,
            offline_export_preserve_url_structure: false,
            replace_content: Vec::new(),
            replace_query_string: Vec::new(),
            offline_export_lowercase: false,
            ignore_store_file_error: false,
            disable_astro_inline_modules: false,
            markdown_export_dir: None,
            markdown_export_single_file: None,
            markdown_move_content_before_h1_to_end: false,
            markdown_disable_images: false,
            markdown_disable_files: false,
            markdown_remove_links_and_images_from_single_file: false,
            markdown_exclude_selector: Vec::new(),
            markdown_replace_content: Vec::new(),
            markdown_replace_query_string: Vec::new(),
            markdown_export_store_only_url_regex: Vec::new(),
            markdown_ignore_store_file_error: false,
            mail_to: Vec::new(),
            mail_from: "test@test.com".to_string(),
            mail_from_name: "Test".to_string(),
            mail_subject_template: "Test".to_string(),
            mail_smtp_host: "localhost".to_string(),
            mail_smtp_port: 25,
            mail_smtp_user: None,
            mail_smtp_pass: None,
            upload_enabled: false,
            upload_to: String::new(),
            upload_retention: "30d".to_string(),
            upload_password: None,
            upload_timeout: 3600,
            http_cache_dir: None,
            http_cache_compression: false,
            http_cache_ttl: None,
            debug: false,
            debug_log_file: None,
            debug_url_regex: Vec::new(),
            fastest_top_limit: 20,
            fastest_max_time: 1.0,
            max_heading_level: 3,
            slowest_top_limit: 20,
            slowest_min_time: 0.01,
            slowest_max_time: 3.0,
            serve_markdown_dir: None,
            serve_offline_dir: None,
            serve_port: 8321,
            serve_bind_address: "127.0.0.1".to_string(),
            ci: true,
            ci_min_score: 5.0,
            ci_min_performance: Some(5.0),
            ci_min_seo: Some(5.0),
            ci_min_security: Some(5.0),
            ci_min_accessibility: Some(3.0),
            ci_min_best_practices: Some(5.0),
            ci_max_404: 0,
            ci_max_5xx: 0,
            ci_max_criticals: 0,
            ci_max_warnings: None,
            ci_max_avg_response: None,
            ci_min_pages: 0,
            ci_min_assets: 0,
            ci_min_documents: 0,
        }
    }

    fn make_scores(overall: f64) -> QualityScores {
        let cats = vec![
            ("Performance", "performance", 0.20),
            ("SEO", "seo", 0.20),
            ("Security", "security", 0.25),
            ("Accessibility", "accessibility", 0.20),
            ("Best Practices", "best-practices", 0.15),
        ];
        QualityScores {
            overall: CategoryScore {
                name: "Overall".to_string(),
                code: "overall".to_string(),
                score: overall,
                label: score_label(overall).to_string(),
                weight: 1.0,
                deductions: Vec::new(),
            },
            categories: cats
                .into_iter()
                .map(|(name, code, weight)| CategoryScore {
                    name: name.to_string(),
                    code: code.to_string(),
                    score: overall,
                    label: score_label(overall).to_string(),
                    weight,
                    deductions: Vec::new(),
                })
                .collect(),
        }
    }

    fn make_stats(total_urls: usize) -> BasicStats {
        let mut count_by_status = BTreeMap::new();
        if total_urls > 0 {
            count_by_status.insert(200, total_urls);
        }
        BasicStats {
            total_urls,
            count_by_status,
            ..Default::default()
        }
    }

    fn make_stats_with_status(total_urls: usize, status_counts: &[(i32, usize)]) -> BasicStats {
        let mut count_by_status = BTreeMap::new();
        for &(code, count) in status_counts {
            count_by_status.insert(code, count);
        }
        BasicStats {
            total_urls,
            count_by_status,
            ..Default::default()
        }
    }

    #[test]
    fn all_checks_pass() {
        let options = make_options();
        let scores = make_scores(8.0);
        let stats = make_stats(100);
        let summary = Summary::new();
        let result = evaluate(&options, &scores, &stats, &summary);
        assert!(result.passed);
        assert_eq!(result.exit_code, 0);
    }

    #[test]
    fn fail_low_overall_score() {
        let options = make_options();
        let scores = make_scores(3.0);
        let stats = make_stats(100);
        let summary = Summary::new();
        let result = evaluate(&options, &scores, &stats, &summary);
        assert!(!result.passed);
        assert_eq!(result.exit_code, 10);
    }

    #[test]
    fn fail_404_count() {
        let options = make_options();
        let scores = make_scores(8.0);
        let stats = make_stats_with_status(100, &[(404, 3)]);
        let summary = Summary::new();
        let result = evaluate(&options, &scores, &stats, &summary);
        assert!(!result.passed);
    }

    #[test]
    fn fail_5xx_count() {
        let options = make_options();
        let scores = make_scores(8.0);
        let stats = make_stats_with_status(100, &[(500, 2)]);
        let summary = Summary::new();
        let result = evaluate(&options, &scores, &stats, &summary);
        assert!(!result.passed);
    }

    #[test]
    fn fail_criticals() {
        let options = make_options();
        let scores = make_scores(8.0);
        let stats = make_stats(100);
        let mut summary = Summary::new();
        summary.add_item(Item::new(
            "test".to_string(),
            "Test critical".to_string(),
            ItemStatus::Critical,
        ));
        let result = evaluate(&options, &scores, &stats, &summary);
        assert!(!result.passed);
    }

    #[test]
    fn optional_warnings() {
        let mut options = make_options();
        options.ci_max_warnings = Some(0);
        let scores = make_scores(8.0);
        let stats = make_stats(100);
        let mut summary = Summary::new();
        summary.add_item(Item::new(
            "test".to_string(),
            "Test warning".to_string(),
            ItemStatus::Warning,
        ));
        let result = evaluate(&options, &scores, &stats, &summary);
        assert!(!result.passed);
    }

    #[test]
    fn optional_avg_response() {
        let mut options = make_options();
        options.ci_max_avg_response = Some(0.5);
        let scores = make_scores(8.0);
        let mut stats = make_stats(100);
        stats.total_requests_times_avg = 1.0;
        let summary = Summary::new();
        let result = evaluate(&options, &scores, &stats, &summary);
        assert!(!result.passed);
    }

    #[test]
    fn zero_urls_immediate_fail() {
        let options = make_options();
        let scores = make_scores(10.0);
        let stats = make_stats(0);
        let summary = Summary::new();
        let result = evaluate(&options, &scores, &stats, &summary);
        assert!(!result.passed);
        assert_eq!(result.exit_code, 10);
    }

    #[test]
    fn only_negative_status_codes_immediate_fail() {
        let options = make_options();
        let scores = make_scores(10.0);
        // 1 URL visited but only with negative status (e.g. timeout = -2)
        let stats = make_stats_with_status(1, &[(-2, 1)]);
        let summary = Summary::new();
        let result = evaluate(&options, &scores, &stats, &summary);
        assert!(!result.passed);
        assert_eq!(result.exit_code, 10);
    }

    #[test]
    fn category_threshold() {
        let mut options = make_options();
        options.ci_min_performance = Some(8.0);
        let mut scores = make_scores(9.0);
        // Set performance score to 6.0 while keeping overall high
        scores.categories[0].score = 6.0;
        let stats = make_stats(100);
        let summary = Summary::new();
        let result = evaluate(&options, &scores, &stats, &summary);
        assert!(!result.passed);
    }

    #[test]
    fn fail_min_pages() {
        let mut options = make_options();
        options.ci_min_pages = 5;
        let scores = make_scores(8.0);
        let mut stats = make_stats(100);
        // Only 3 HTML pages
        stats.count_by_content_type.insert(ContentTypeId::Html as i32, 3);
        let summary = Summary::new();
        let result = evaluate(&options, &scores, &stats, &summary);
        assert!(!result.passed);
        assert!(result.checks.iter().any(|c| c.metric == "HTML pages" && !c.passed));
    }

    #[test]
    fn pass_min_pages() {
        let mut options = make_options();
        options.ci_min_pages = 5;
        let scores = make_scores(8.0);
        let mut stats = make_stats(100);
        stats.count_by_content_type.insert(ContentTypeId::Html as i32, 10);
        let summary = Summary::new();
        let result = evaluate(&options, &scores, &stats, &summary);
        assert!(result.checks.iter().any(|c| c.metric == "HTML pages" && c.passed));
    }

    #[test]
    fn fail_min_assets() {
        let mut options = make_options();
        options.ci_min_assets = 5;
        let scores = make_scores(8.0);
        let mut stats = make_stats(100);
        stats.count_by_content_type.insert(ContentTypeId::Script as i32, 1);
        stats.count_by_content_type.insert(ContentTypeId::Stylesheet as i32, 1);
        // Total assets = 2, below threshold 5
        let summary = Summary::new();
        let result = evaluate(&options, &scores, &stats, &summary);
        assert!(!result.passed);
        assert!(result.checks.iter().any(|c| c.metric.contains("Assets") && !c.passed));
    }

    #[test]
    fn documents_check_skipped_when_zero() {
        let options = make_options(); // ci_min_documents = 0
        let scores = make_scores(8.0);
        let stats = make_stats(100);
        let summary = Summary::new();
        let result = evaluate(&options, &scores, &stats, &summary);
        // Documents check should not appear at all
        assert!(!result.checks.iter().any(|c| c.metric == "Documents"));
    }

    #[test]
    fn fail_min_documents() {
        let mut options = make_options();
        options.ci_min_documents = 2;
        let scores = make_scores(8.0);
        let mut stats = make_stats(100);
        stats.count_by_content_type.insert(ContentTypeId::Document as i32, 1);
        let summary = Summary::new();
        let result = evaluate(&options, &scores, &stats, &summary);
        assert!(!result.passed);
        assert!(result.checks.iter().any(|c| c.metric == "Documents" && !c.passed));
    }
}
