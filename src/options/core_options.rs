// SiteOne Crawler - Core options (all CLI options)
// (c) Jan Reges <jan.reges@siteone.cz>
//

use regex::Regex;

use crate::ai::secret::SecretString;
use crate::debugger;
use crate::error::CrawlerError;
use crate::extra_column::ExtraColumn;
use crate::types::{DeviceType, OutputType};

use super::group::OptionGroup;
use super::option::{CrawlerOption, OptionValue};
use super::option_type::OptionType;
use super::options::Options;

pub const GROUP_BASIC_SETTINGS: &str = "basic-settings";
pub const GROUP_OUTPUT_SETTINGS: &str = "output-settings";
pub const GROUP_RESOURCE_FILTERING: &str = "resource-filtering";
pub const GROUP_ADVANCED_CRAWLER_SETTINGS: &str = "advanced-crawler-settings";
pub const GROUP_EXPERT_SETTINGS: &str = "expert-settings";
pub const GROUP_FILE_EXPORT_SETTINGS: &str = "file-export-settings";
pub const GROUP_MAILER_SETTINGS: &str = "mailer-settings";
pub const GROUP_MARKDOWN_EXPORT_SETTINGS: &str = "markdown-export-settings";
pub const GROUP_OFFLINE_EXPORT_SETTINGS: &str = "offline-export-settings";
pub const GROUP_SITEMAP_SETTINGS: &str = "sitemap-settings";
pub const GROUP_UPLOAD_SETTINGS: &str = "upload-settings";
pub const GROUP_FASTEST_ANALYZER: &str = "fastest-analyzer";
pub const GROUP_SEO_AND_OPENGRAPH_ANALYZER: &str = "seo-and-opengraph-analyzer";
pub const GROUP_SLOWEST_ANALYZER: &str = "slowest-analyzer";
pub const GROUP_CI_CD_SETTINGS: &str = "ci-cd-settings";
pub const GROUP_SERVER_SETTINGS: &str = "server-settings";
pub const GROUP_AI_SETTINGS: &str = "ai-settings";
pub const GROUP_BROWSER: &str = "browser-settings";

/// Result storage type
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "lowercase")]
pub enum StorageType {
    Memory,
    File,
}

impl StorageType {
    pub fn from_text(text: &str) -> Result<Self, CrawlerError> {
        match text.trim().to_lowercase().as_str() {
            "memory" => Ok(StorageType::Memory),
            "file" => Ok(StorageType::File),
            other => Err(CrawlerError::Config(format!(
                "Unknown storage type '{}'. Supported values are: memory, file",
                other
            ))),
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            StorageType::Memory => "memory",
            StorageType::File => "file",
        }
    }
}

#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CoreOptions {
    // basic settings
    pub url: String,
    pub url_list: Option<String>,
    pub url_list_urls: Vec<String>,
    pub single_page: bool,
    pub max_depth: i64,
    pub device: DeviceType,
    pub user_agent: Option<String>,
    pub timeout: i64,
    pub proxy: Option<String>,
    pub http_auth: Option<String>,
    pub accept_invalid_certs: bool,
    pub timezone: Option<String>,
    pub show_version_only: bool,
    pub show_help_only: bool,

    // output settings
    pub output_type: OutputType,
    pub url_column_size: Option<i64>,
    pub show_inline_criticals: bool,
    pub show_inline_warnings: bool,
    pub rows_limit: i64,
    pub extra_columns: Vec<ExtraColumn>,
    pub extra_columns_names_only: Vec<String>,
    pub show_scheme_and_host: bool,
    pub do_not_truncate_url: bool,
    pub hide_progress_bar: bool,
    pub hide_columns: Vec<String>,
    pub no_color: bool,
    pub force_color: bool,
    pub console_width: Option<i64>,

    // resource filtering
    pub disable_all_assets: bool,
    pub disable_javascript: bool,
    pub disable_styles: bool,
    pub disable_fonts: bool,
    pub disable_images: bool,
    pub disable_files: bool,
    pub remove_all_anchor_listeners: bool,

    // advanced crawler settings
    pub workers: i64,
    pub max_reqs_per_sec: f64,
    pub memory_limit: String,
    pub resolve: Vec<String>,
    pub websocket_server: Option<String>,
    pub ignore_robots_txt: bool,
    pub ignore_html_comments: bool,
    pub allowed_domains_for_external_files: Vec<String>,
    pub allowed_domains_for_crawling: Vec<String>,
    pub single_foreign_page: bool,
    pub result_storage: StorageType,
    pub result_storage_dir: String,
    pub result_storage_compression: bool,
    pub accept_encoding: String,
    pub max_queue_length: i64,
    pub max_visited_urls: i64,
    pub max_url_length: i64,
    pub max_skipped_urls: i64,
    pub max_non200_responses_per_basename: i64,
    pub include_regex: Vec<String>,
    pub ignore_regex: Vec<String>,
    pub regex_filtering_only_for_pages: bool,
    pub analyzer_filter_regex: Option<String>,
    pub add_random_query_params: bool,
    pub remove_query_params: bool,
    pub keep_query_params: Vec<String>,
    pub transform_url: Vec<String>,
    pub force_relative_urls: bool,

    // file export settings
    pub output_html_report: Option<String>,
    pub html_report_options: Option<String>,
    pub output_json_file: Option<String>,
    pub output_text_file: Option<String>,
    pub add_host_to_output_file: bool,
    pub add_timestamp_to_output_file: bool,

    // sitemap settings
    pub sitemap_xml_file: Option<String>,
    pub sitemap_txt_file: Option<String>,
    pub sitemap_base_priority: f64,
    pub sitemap_priority_increase: f64,

    // offline export settings
    pub offline_export_dir: Option<String>,
    pub offline_export_store_only_url_regex: Vec<String>,
    pub offline_export_remove_unwanted_code: bool,
    pub offline_export_no_auto_redirect_html: bool,
    pub offline_export_preserve_url_structure: bool,
    pub offline_export_preserve_urls: bool,
    pub offline_export_no_url_rewriting: bool,
    pub replace_content: Vec<String>,
    pub replace_query_string: Vec<String>,
    pub offline_export_lowercase: bool,
    pub ignore_store_file_error: bool,
    pub disable_astro_inline_modules: bool,

    // markdown export settings
    pub markdown_export_dir: Option<String>,
    pub markdown_export_single_file: Option<String>,
    pub markdown_move_content_before_h1_to_end: bool,
    pub markdown_disable_images: bool,
    pub markdown_disable_files: bool,
    pub markdown_remove_links_and_images_from_single_file: bool,
    pub markdown_exclude_selector: Vec<String>,
    pub markdown_replace_content: Vec<String>,
    pub markdown_replace_query_string: Vec<String>,
    pub markdown_export_store_only_url_regex: Vec<String>,
    pub markdown_ignore_store_file_error: bool,

    // mailer settings
    pub mail_to: Vec<String>,
    pub mail_from: String,
    pub mail_from_name: String,
    pub mail_subject_template: String,
    pub mail_smtp_host: String,
    pub mail_smtp_port: i64,
    pub mail_smtp_user: Option<String>,
    pub mail_smtp_pass: Option<String>,

    // upload settings
    pub upload_enabled: bool,
    pub upload_to: String,
    pub upload_retention: String,
    pub upload_password: Option<String>,
    pub upload_timeout: i64,

    // expert settings
    pub http_cache_dir: Option<String>,
    pub http_cache_compression: bool,
    pub http_cache_ttl: Option<u64>,
    pub debug: bool,
    pub debug_log_file: Option<String>,
    pub debug_url_regex: Vec<String>,

    // fastest analyzer settings
    pub fastest_top_limit: i64,
    pub fastest_max_time: f64,

    // seo and opengraph analyzer settings
    pub max_heading_level: i64,

    // slowest analyzer settings
    pub slowest_top_limit: i64,
    pub slowest_min_time: f64,
    pub slowest_max_time: f64,

    // server settings
    pub serve_markdown_dir: Option<String>,
    pub serve_offline_dir: Option<String>,
    pub serve_port: i64,
    pub serve_bind_address: String,

    // html-to-markdown converter mode (standalone, no crawl)
    pub html_to_markdown_file: Option<String>,
    pub html_to_markdown_output: Option<String>,

    // ci/cd settings
    pub ci: bool,
    pub ci_min_score: f64,
    pub ci_min_performance: Option<f64>,
    pub ci_min_seo: Option<f64>,
    pub ci_min_security: Option<f64>,
    pub ci_min_accessibility: Option<f64>,
    pub ci_min_best_practices: Option<f64>,
    pub ci_max_404: i64,
    pub ci_max_5xx: i64,
    pub ci_max_criticals: i64,
    pub ci_max_warnings: Option<i64>,
    pub ci_max_avg_response: Option<f64>,
    pub ci_min_pages: i64,
    pub ci_min_assets: i64,
    pub ci_min_documents: i64,
    pub ci_baseline: Option<String>,
    pub ci_max_score_drop: Option<f64>,
    pub ci_fail_on_code: Vec<String>,
    pub ci_ignore_code: Vec<String>,
    pub ci_junit_file: Option<String>,
    pub ci_github_annotations: bool,

    // ai settings (optional AI features; nothing runs unless ai_enabled)
    #[serde(skip)]
    pub ai_enabled: bool,
    pub ai_provider: String,
    pub ai_endpoint: Option<String>,
    pub ai_model: Option<String>,
    /// Raw `--ai-api-key` value (redacted in Debug/Serialize via SecretString).
    pub ai_api_key: Option<SecretString>,
    pub ai_api_key_env: Option<String>,
    pub ai_api_key_file: Option<String>,
    pub ai_max_tokens: i64,
    pub ai_use_max_completion_tokens: bool,
    pub ai_temperature: f64,
    pub ai_extra_body: Option<String>,
    pub ai_synthesis_extra_body: Option<String>,
    pub ai_actions: Vec<String>,
    pub ai_prompt_file: Option<String>,
    pub ai_prompt: Option<String>,
    pub ai_language: Option<String>,
    pub ai_include: Vec<String>,
    pub ai_exclude: Vec<String>,
    pub ai_max_pages: i64,
    pub ai_max_concurrency: i64,
    pub ai_max_reqs_per_sec: Option<f64>,
    pub ai_timeout: i64,
    pub ai_cache_dir: Option<String>,
    pub ai_seo_affects_score: bool,
    pub ai_dry_run: bool,

    // browser rendering settings (optional; nothing runs unless browser_enabled)
    #[serde(skip)]
    pub browser_enabled: bool,
    pub browser_path: Option<String>,
    pub browser_headful: bool,
    pub browser_workers: i64,
    pub browser_wait: String,
    pub browser_wait_extra_ms: i64,
    pub browser_timeout: i64,
    pub browser_render_all: bool,
    pub browser_auto_download: bool,

    // screenshot settings (browser mode only)
    pub screenshots: bool,
    pub screenshots_dir: Option<String>,
    pub screenshot_mode: String,
    pub screenshot_viewport: String,
    pub screenshot_format: String,
    pub screenshot_quality: i64,
    pub screenshot_hide_cookie_banners: bool,
    pub screenshot_hide_selector: Option<String>,
    // screenshot animation (browser mode only; see --screenshots-animation)
    pub screenshots_animation: String,
    pub screenshots_animation_frame_duration: f64,
    pub screenshots_animation_width: i64,
    pub ffmpeg_path: Option<String>,
    pub browser_no_sandbox: bool,

    // console diagnostics → AI payload limits (browser mode only)
    pub console_max_messages: i64,
    pub console_msg_max_chars: i64,
    pub console_total_max_kb: i64,
}

impl CoreOptions {
    /// Create CoreOptions by parsing values from a populated Options registry.
    pub fn from_options(options: &Options) -> Result<Self, CrawlerError> {
        // Determine output directory prefix: try ./tmp/ first, fallback to system data dir
        let output_prefix = default_output_prefix();

        let mut core = CoreOptions {
            // basic settings
            url: String::new(),
            url_list: None,
            url_list_urls: Vec::new(),
            single_page: false,
            max_depth: 0,
            device: DeviceType::Desktop,
            user_agent: None,
            timeout: 5,
            proxy: None,
            http_auth: None,
            accept_invalid_certs: false,
            timezone: None,
            show_version_only: false,
            show_help_only: false,

            // output settings
            output_type: OutputType::Text,
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

            // resource filtering
            disable_all_assets: false,
            disable_javascript: false,
            disable_styles: false,
            disable_fonts: false,
            disable_images: false,
            disable_files: false,
            remove_all_anchor_listeners: false,

            // advanced crawler settings
            workers: 3,
            max_reqs_per_sec: 10.0,
            memory_limit: "2048M".to_string(),
            resolve: Vec::new(),
            websocket_server: None,
            ignore_robots_txt: false,
            ignore_html_comments: false,
            allowed_domains_for_external_files: Vec::new(),
            allowed_domains_for_crawling: Vec::new(),
            single_foreign_page: false,
            result_storage: StorageType::Memory,
            result_storage_dir: format!("{output_prefix}/result-storage"),
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

            // file export settings (all 3 reports enabled by default)
            output_html_report: Some(
                std::path::Path::new(&output_prefix)
                    .join("%domain%.report.%datetime%.html")
                    .to_string_lossy()
                    .to_string(),
            ),
            html_report_options: None,
            output_json_file: Some(
                std::path::Path::new(&output_prefix)
                    .join("%domain%.output.%datetime%.json")
                    .to_string_lossy()
                    .to_string(),
            ),
            output_text_file: Some(
                std::path::Path::new(&output_prefix)
                    .join("%domain%.output.%datetime%.txt")
                    .to_string_lossy()
                    .to_string(),
            ),
            add_host_to_output_file: false,
            add_timestamp_to_output_file: false,

            // sitemap settings
            sitemap_xml_file: None,
            sitemap_txt_file: None,
            sitemap_base_priority: 0.5,
            sitemap_priority_increase: 0.1,

            // offline export settings
            offline_export_dir: None,
            offline_export_store_only_url_regex: Vec::new(),
            offline_export_remove_unwanted_code: true,
            offline_export_no_auto_redirect_html: false,
            offline_export_preserve_url_structure: false,
            offline_export_preserve_urls: false,
            offline_export_no_url_rewriting: false,
            replace_content: Vec::new(),
            replace_query_string: Vec::new(),
            offline_export_lowercase: false,
            ignore_store_file_error: false,
            disable_astro_inline_modules: false,

            // markdown export settings
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

            // mailer settings
            mail_to: Vec::new(),
            mail_from: "siteone-crawler@your-hostname.com".to_string(),
            mail_from_name: "SiteOne Crawler".to_string(),
            mail_subject_template: "Crawler Report for %domain% (%date%)".to_string(),
            mail_smtp_host: "localhost".to_string(),
            mail_smtp_port: 25,
            mail_smtp_user: None,
            mail_smtp_pass: None,

            // upload settings
            upload_enabled: false,
            upload_to: "https://crawler.siteone.io/up".to_string(),
            upload_retention: "30d".to_string(),
            upload_password: None,
            upload_timeout: 3600,

            // expert settings
            http_cache_dir: Some(default_http_cache_dir()),
            http_cache_compression: false,
            http_cache_ttl: Some(24 * 3600), // 24 hours in seconds
            debug: false,
            debug_log_file: None,
            debug_url_regex: Vec::new(),

            // fastest analyzer settings
            fastest_top_limit: 20,
            fastest_max_time: 1.0,

            // seo and opengraph analyzer settings
            max_heading_level: 3,

            // slowest analyzer settings
            slowest_top_limit: 20,
            slowest_min_time: 0.01,
            slowest_max_time: 3.0,

            // server settings
            serve_markdown_dir: None,
            serve_offline_dir: None,
            serve_port: 8321,
            serve_bind_address: "127.0.0.1".to_string(),

            html_to_markdown_file: None,
            html_to_markdown_output: None,

            // ci/cd settings
            ci: false,
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
            ci_min_pages: 10,
            ci_min_assets: 10,
            ci_min_documents: 0,
            ci_baseline: None,
            ci_max_score_drop: None,
            ci_fail_on_code: Vec::new(),
            ci_ignore_code: Vec::new(),
            ci_junit_file: None,
            ci_github_annotations: false,

            // ai settings
            ai_enabled: false,
            ai_provider: "openai-compatible".to_string(),
            ai_endpoint: None,
            ai_model: None,
            ai_api_key: None,
            ai_api_key_env: None,
            ai_api_key_file: None,
            ai_max_tokens: 32000,
            ai_use_max_completion_tokens: false,
            ai_temperature: 0.0,
            ai_extra_body: None,
            ai_synthesis_extra_body: None,
            ai_actions: vec!["seo".to_string(), "typos".to_string(), "summary".to_string()],
            ai_prompt_file: None,
            ai_prompt: None,
            ai_language: None,
            ai_include: Vec::new(),
            ai_exclude: Vec::new(),
            ai_max_pages: 100,
            ai_max_concurrency: 4,
            ai_max_reqs_per_sec: None,
            ai_timeout: 180,
            ai_cache_dir: Some("tmp/ai-cache".to_string()),
            ai_seo_affects_score: false,
            ai_dry_run: false,

            // browser rendering settings
            browser_enabled: false,
            browser_path: None,
            browser_headful: false,
            browser_workers: 3,
            browser_wait: "networkidle".to_string(),
            browser_wait_extra_ms: 0,
            browser_timeout: 30,
            browser_render_all: false,
            browser_auto_download: false,

            // screenshot settings
            screenshots: false,
            screenshots_dir: None,
            screenshot_mode: "viewport".to_string(),
            screenshot_viewport: "1920x1080".to_string(),
            screenshot_format: "png".to_string(),
            screenshot_quality: 80,
            screenshot_hide_cookie_banners: false,
            screenshot_hide_selector: None,
            // screenshot animation
            screenshots_animation: String::new(),
            screenshots_animation_frame_duration: 2.0,
            screenshots_animation_width: 1024,
            ffmpeg_path: None,
            browser_no_sandbox: false,
            console_max_messages: 100,
            console_msg_max_chars: 200,
            console_total_max_kb: 128,
        };

        // Populate from option groups
        for (_apl_code, group) in options.get_groups() {
            for (_prop_name, option) in &group.options {
                let value = option.get_value()?;
                core.apply_option_value(&option.property_to_fill, value)?;
            }
        }

        // AI is active only if the user explicitly set at least one --ai-* flag.
        core.ai_enabled = [
            "aiProvider",
            "aiEndpoint",
            "aiModel",
            "aiApiKey",
            "aiApiKeyEnv",
            "aiApiKeyFile",
            "aiActions",
            "aiPromptFile",
            "aiPrompt",
            "aiExtraBody",
            "aiSynthesisExtraBody",
            "aiDryRun",
            "aiSeoAffectsScore",
            "aiInclude",
            "aiExclude",
            "aiMaxPages",
            "aiMaxTokens",
            "aiTemperature",
            "aiLanguage",
            "aiMaxConcurrency",
            "aiMaxReqsPerSec",
            "aiTimeout",
            "aiUseMaxCompletionTokens",
            "aiCacheDir",
        ]
        .iter()
        .any(|p| options.is_explicitly_set(p));

        if core.ai_enabled {
            let provider = crate::ai::provider::Provider::parse(&core.ai_provider).ok_or_else(|| {
                CrawlerError::Config(format!(
                    "Invalid --ai-provider '{}'. Use openai, anthropic, gemini, or openai-compatible.",
                    core.ai_provider
                ))
            })?;
            if provider == crate::ai::provider::Provider::OpenAiCompatible && core.ai_endpoint.is_none() {
                return Err(CrawlerError::Config(
                    "--ai-provider=openai-compatible requires --ai-endpoint=URL.".to_string(),
                ));
            }
            if core.ai_model.as_deref().map(|s| s.trim().is_empty()).unwrap_or(true) {
                return Err(CrawlerError::Config(
                    "AI is enabled but --ai-model is missing.".to_string(),
                ));
            }
            const KNOWN_ACTIONS: [&str; 6] = ["seo", "llms-txt", "llms-full", "typos", "custom", "summary"];
            for a in &core.ai_actions {
                if !KNOWN_ACTIONS.contains(&a.as_str()) {
                    return Err(CrawlerError::Config(format!(
                        "Unknown --ai-actions value '{}'. Known values: {}.",
                        a,
                        KNOWN_ACTIONS.join(", ")
                    )));
                }
            }
            if core.ai_actions.iter().any(|a| a == "custom")
                && core.ai_prompt_file.is_none()
                && core.ai_prompt.is_none()
            {
                return Err(CrawlerError::Config(
                    "--ai-actions=custom requires --ai-prompt-file=PATH or --ai-prompt=TEXT.".to_string(),
                ));
            }
            for (flag, body) in [
                ("--ai-extra-body", &core.ai_extra_body),
                ("--ai-synthesis-extra-body", &core.ai_synthesis_extra_body),
            ] {
                if let Some(body) = body {
                    match serde_json::from_str::<serde_json::Value>(body) {
                        Ok(v) if v.is_object() => {}
                        Ok(_) => {
                            return Err(CrawlerError::Config(format!("{} must be a JSON object.", flag)));
                        }
                        Err(e) => {
                            return Err(CrawlerError::Config(format!("{} is not valid JSON: {}", flag, e)));
                        }
                    }
                }
            }
        }

        // Browser mode requires the `browser` feature to be compiled in. Reject it here (config
        // error, exit 101) instead of only at runtime.
        #[cfg(not(feature = "browser"))]
        if core.browser_enabled {
            return Err(CrawlerError::Config(
                "--browser requires a browser-enabled build (compile with `--features browser`, or use a browser-enabled release artifact).".to_string(),
            ));
        }

        // Browser rendering validation.
        if core.browser_enabled {
            const WAIT: [&str; 3] = ["load", "domcontentloaded", "networkidle"];
            if !WAIT.contains(&core.browser_wait.as_str()) {
                return Err(CrawlerError::Config(format!(
                    "Invalid --browser-wait '{}'. Use load, domcontentloaded, or networkidle.",
                    core.browser_wait
                )));
            }
            // Viewport must be WxH (otherwise it would silently fall back to 1920x1080).
            let viewport_ok = core
                .screenshot_viewport
                .split_once(['x', 'X'])
                .map(|(w, h)| {
                    matches!(
                        (w.trim().parse::<u32>(), h.trim().parse::<u32>()),
                        (Ok(w), Ok(h)) if w > 0 && h > 0
                    )
                })
                .unwrap_or(false);
            if !viewport_ok {
                return Err(CrawlerError::Config(format!(
                    "Invalid --screenshot-viewport '{}'. Use WxH, e.g. 1920x1080.",
                    core.screenshot_viewport
                )));
            }
            if core.screenshots {
                const MODE: [&str; 2] = ["viewport", "full-page"];
                if !MODE.contains(&core.screenshot_mode.as_str()) {
                    return Err(CrawlerError::Config(format!(
                        "Invalid --screenshot-mode '{}'. Use viewport or full-page.",
                        core.screenshot_mode
                    )));
                }
                const FORMAT: [&str; 4] = ["png", "jpg", "jpeg", "webp"];
                if !FORMAT.contains(&core.screenshot_format.to_lowercase().as_str()) {
                    return Err(CrawlerError::Config(format!(
                        "Invalid --screenshot-format '{}'. Use png, jpg, or webp.",
                        core.screenshot_format
                    )));
                }
            }
        } else if core.screenshots {
            return Err(CrawlerError::Config(
                "--screenshots requires --browser (screenshots are captured during browser rendering).".to_string(),
            ));
        }

        // Screenshots animation validation.
        if !core.screenshots_animation.trim().is_empty() {
            if !core.browser_enabled || !core.screenshots {
                return Err(CrawlerError::Config(
                    "--screenshots-animation requires --browser and --screenshots.".to_string(),
                ));
            }
            for token in core.screenshots_animation.split(',') {
                let t = token.trim().to_lowercase();
                if t.is_empty() {
                    continue;
                }
                if t != "gif" && t != "mp4" {
                    return Err(CrawlerError::Config(format!(
                        "Invalid --screenshots-animation value '{}'. Use gif and/or mp4.",
                        token.trim()
                    )));
                }
            }
            if !(2..=8192).contains(&core.screenshots_animation_width) {
                return Err(CrawlerError::Config(
                    "--screenshots-animation-width must be between 2 and 8192.".to_string(),
                ));
            }
        }

        // Cookie-banner hiding only applies during screenshot capture.
        if (core.screenshot_hide_cookie_banners || core.screenshot_hide_selector.is_some())
            && (!core.browser_enabled || !core.screenshots)
        {
            return Err(CrawlerError::Config(
                "--screenshot-hide-cookie-banners/--screenshot-hide-selector require --browser and --screenshots."
                    .to_string(),
            ));
        }

        // Disable all assets if set
        if core.disable_all_assets {
            core.disable_javascript = true;
            core.disable_styles = true;
            core.disable_fonts = true;
            core.disable_images = true;
            core.disable_files = true;
        }

        // In CI mode, disable default report outputs (user cares about exit code, not files).
        // Only suppress outputs that weren't explicitly set by the user on the command line.
        if core.ci {
            if !options.is_explicitly_set("outputHtmlReport") {
                core.output_html_report = None;
            }
            if !options.is_explicitly_set("outputJsonFile") {
                core.output_json_file = None;
            }
            if !options.is_explicitly_set("outputTextFile") {
                core.output_text_file = None;
            }
        }

        // Warn if --html-to-markdown-output is set without --html-to-markdown
        if core.html_to_markdown_output.is_some() && core.html_to_markdown_file.is_none() {
            return Err(CrawlerError::Config(
                "--html-to-markdown-output requires --html-to-markdown to be set.".to_string(),
            ));
        }

        // In html-to-markdown mode, validate input file and return early
        if let Some(ref html_file) = core.html_to_markdown_file {
            if !std::path::Path::new(html_file).exists() {
                return Err(CrawlerError::Config(format!(
                    "HTML file '{}' does not exist.",
                    html_file
                )));
            }
            if !std::path::Path::new(html_file).is_file() {
                return Err(CrawlerError::Config(format!("'{}' is not a file.", html_file)));
            }
            return Ok(core);
        }

        // In serve mode, skip normal crawl validation and return early
        if core.serve_markdown_dir.is_some() || core.serve_offline_dir.is_some() {
            return Ok(core);
        }

        // Process --url-list: read & parse the file, then optionally use the first
        // URL as the crawl base when --url was not provided.
        if let Some(ref path) = core.url_list {
            if !std::path::Path::new(path).is_file() {
                return Err(CrawlerError::Config(format!(
                    "URL list file '{}' does not exist or is not a file.",
                    path
                )));
            }
            let content = std::fs::read_to_string(path)
                .map_err(|e| CrawlerError::Config(format!("Cannot read URL list file '{}': {}", path, e)))?;
            let all = parse_line_list(&content);
            if all.is_empty() {
                return Err(CrawlerError::Config(format!(
                    "URL list file '{}' contains no URLs (empty list).",
                    path
                )));
            }
            // Keep only absolute http(s) URLs and report the rest, instead of
            // silently dropping them later during the crawl. Relative or
            // scheme-less lines cannot be resolved from a flat list.
            let (urls, invalid): (Vec<String>, Vec<String>) = all.into_iter().partition(|u| is_http_url(u));
            if !invalid.is_empty() {
                let sample = invalid.iter().take(3).cloned().collect::<Vec<_>>().join(", ");
                eprintln!(
                    "Warning: --url-list '{}': skipped {} line(s) that are not absolute http(s) URLs (e.g. {}).",
                    path,
                    invalid.len(),
                    sample
                );
            }
            if urls.is_empty() {
                // The list yielded nothing usable. Only fail hard if there is also
                // no explicit --url to fall back on; otherwise proceed with --url
                // alone (the skipped lines were already reported above).
                if core.url.is_empty() {
                    return Err(CrawlerError::Config(format!(
                        "URL list file '{}' contains no valid http(s) URLs.",
                        path
                    )));
                }
            } else {
                // The base URL (when --url is omitted) is the first valid http(s) entry.
                if core.url.is_empty() {
                    core.url = urls[0].clone();
                }
                core.url_list_urls = urls;
            }
        }

        // Validate required fields
        if core.url.is_empty() {
            return Err(CrawlerError::Config(
                "Invalid or undefined input. Provide --url=<url> or --url-list=<file>.".to_string(),
            ));
        }
        if core.workers < 1 {
            return Err(CrawlerError::Config(format!(
                "Invalid value '{}' (minimum is 1) for --workers",
                core.workers
            )));
        }

        // Build extra_columns_names_only
        core.extra_columns_names_only = core
            .extra_columns
            .iter()
            .map(|ec| {
                let re = Regex::new(r"\s*\(.+$").ok();
                match re {
                    Some(r) => r.replace(&ec.name, "").to_string(),
                    None => ec.name.clone(),
                }
            })
            .collect();

        // Configure debugger
        debugger::set_config(core.debug, core.debug_log_file.as_deref());

        Ok(core)
    }

    fn apply_option_value(&mut self, property: &str, value: &OptionValue) -> Result<(), CrawlerError> {
        match property {
            "url" => {
                if let Some(s) = value.as_str() {
                    self.url = s.to_string();
                }
            }
            "urlList" => {
                if let Some(s) = value.as_str() {
                    self.url_list = Some(s.to_string());
                }
            }
            "singlePage" => {
                if let Some(b) = value.as_bool() {
                    self.single_page = b;
                }
            }
            "maxDepth" => {
                if let Some(n) = value.as_int() {
                    self.max_depth = n;
                }
            }
            "device" => {
                if let Some(s) = value.as_str() {
                    self.device = DeviceType::from_text(s)?;
                }
            }
            "userAgent" => {
                if let Some(s) = value.as_str() {
                    self.user_agent = Some(s.to_string());
                }
            }
            "timeout" => {
                if let Some(n) = value.as_int() {
                    self.timeout = n;
                }
            }
            "proxy" => {
                if let Some(s) = value.as_str() {
                    self.proxy = Some(s.to_string());
                }
            }
            "httpAuth" => {
                if let Some(s) = value.as_str() {
                    self.http_auth = Some(s.to_string());
                }
            }
            "acceptInvalidCerts" => {
                if let Some(b) = value.as_bool() {
                    self.accept_invalid_certs = b;
                }
            }
            "timezone" => {
                if let Some(s) = value.as_str() {
                    self.timezone = Some(s.to_string());
                }
            }
            "showHelpOnly" => {
                if let Some(b) = value.as_bool() {
                    self.show_help_only = b;
                }
            }
            "showVersionOnly" => {
                if let Some(b) = value.as_bool() {
                    self.show_version_only = b;
                }
            }
            "outputType" => {
                if let Some(s) = value.as_str() {
                    self.output_type = OutputType::from_text(s)?;
                }
            }
            "urlColumnSize" => {
                if let Some(n) = value.as_int() {
                    self.url_column_size = Some(n);
                }
            }
            "showInlineCriticals" => {
                if let Some(b) = value.as_bool() {
                    self.show_inline_criticals = b;
                }
            }
            "showInlineWarnings" => {
                if let Some(b) = value.as_bool() {
                    self.show_inline_warnings = b;
                }
            }
            "rowsLimit" => {
                if let Some(n) = value.as_int() {
                    self.rows_limit = n;
                }
            }
            "extraColumns" => {
                if let Some(arr) = value.as_array() {
                    for column_text in arr {
                        self.extra_columns.push(ExtraColumn::from_text(column_text)?);
                    }
                }
            }
            "showSchemeAndHost" => {
                if let Some(b) = value.as_bool() {
                    self.show_scheme_and_host = b;
                }
            }
            "doNotTruncateUrl" => {
                if let Some(b) = value.as_bool() {
                    self.do_not_truncate_url = b;
                }
            }
            "hideProgressBar" => {
                if let Some(b) = value.as_bool() {
                    self.hide_progress_bar = b;
                }
            }
            "hideColumns" => {
                if let Some(s) = value.as_str() {
                    self.hide_columns = s.split(',').map(|c| c.trim().to_lowercase()).collect();
                }
            }
            "noColor" => {
                if let Some(b) = value.as_bool() {
                    self.no_color = b;
                }
            }
            "forceColor" => {
                if let Some(b) = value.as_bool() {
                    self.force_color = b;
                }
            }
            "consoleWidth" => {
                if let Some(n) = value.as_int() {
                    self.console_width = Some(n);
                }
            }
            "disableAllAssets" => {
                if let Some(b) = value.as_bool() {
                    self.disable_all_assets = b;
                }
            }
            "disableJavascript" => {
                if let Some(b) = value.as_bool() {
                    self.disable_javascript = b;
                }
            }
            "disableStyles" => {
                if let Some(b) = value.as_bool() {
                    self.disable_styles = b;
                }
            }
            "disableFonts" => {
                if let Some(b) = value.as_bool() {
                    self.disable_fonts = b;
                }
            }
            "disableImages" => {
                if let Some(b) = value.as_bool() {
                    self.disable_images = b;
                }
            }
            "disableFiles" => {
                if let Some(b) = value.as_bool() {
                    self.disable_files = b;
                }
            }
            "removeAllAnchorListeners" => {
                if let Some(b) = value.as_bool() {
                    self.remove_all_anchor_listeners = b;
                }
            }
            "workers" => {
                if let Some(n) = value.as_int() {
                    self.workers = n;
                }
            }
            "maxReqsPerSec" => {
                if let Some(n) = value.as_float() {
                    self.max_reqs_per_sec = n;
                }
            }
            "memoryLimit" => {
                if let Some(s) = value.as_str() {
                    self.memory_limit = s.to_string();
                }
            }
            "resolve" => {
                if let Some(arr) = value.as_array() {
                    self.resolve = arr.clone();
                }
            }
            "websocketServer" => {
                if let Some(s) = value.as_str() {
                    self.websocket_server = Some(s.to_string());
                }
            }
            "ignoreRobotsTxt" => {
                if let Some(b) = value.as_bool() {
                    self.ignore_robots_txt = b;
                }
            }
            "ignoreHtmlComments" => {
                if let Some(b) = value.as_bool() {
                    self.ignore_html_comments = b;
                }
            }
            "allowedDomainsForExternalFiles" => {
                if let Some(arr) = value.as_array() {
                    self.allowed_domains_for_external_files = arr.clone();
                }
            }
            "allowedDomainsForCrawling" => {
                if let Some(arr) = value.as_array() {
                    self.allowed_domains_for_crawling = arr.clone();
                }
            }
            "singleForeignPage" => {
                if let Some(b) = value.as_bool() {
                    self.single_foreign_page = b;
                }
            }
            "resultStorage" => {
                if let Some(s) = value.as_str() {
                    self.result_storage = StorageType::from_text(s)?;
                }
            }
            "resultStorageDir" => {
                if let Some(s) = value.as_str() {
                    self.result_storage_dir = s.to_string();
                }
            }
            "resultStorageCompression" => {
                if let Some(b) = value.as_bool() {
                    self.result_storage_compression = b;
                }
            }
            "acceptEncoding" => {
                if let Some(s) = value.as_str() {
                    self.accept_encoding = s.to_string();
                }
            }
            "maxQueueLength" => {
                if let Some(n) = value.as_int() {
                    self.max_queue_length = n;
                }
            }
            "maxVisitedUrls" => {
                if let Some(n) = value.as_int() {
                    self.max_visited_urls = n;
                }
            }
            "maxUrlLength" => {
                if let Some(n) = value.as_int() {
                    self.max_url_length = n;
                }
            }
            "maxSkippedUrls" => {
                if let Some(n) = value.as_int() {
                    self.max_skipped_urls = n;
                }
            }
            "maxNon200ResponsesPerBasename" => {
                if let Some(n) = value.as_int() {
                    self.max_non200_responses_per_basename = n;
                }
            }
            "includeRegex" => {
                if let Some(arr) = value.as_array() {
                    self.include_regex = arr.clone();
                }
            }
            "ignoreRegex" => {
                if let Some(arr) = value.as_array() {
                    self.ignore_regex = arr.clone();
                }
            }
            "regexFilteringOnlyForPages" => {
                if let Some(b) = value.as_bool() {
                    self.regex_filtering_only_for_pages = b;
                }
            }
            "analyzerFilterRegex" => {
                if let Some(s) = value.as_str() {
                    self.analyzer_filter_regex = Some(s.to_string());
                }
            }
            "addRandomQueryParams" => {
                if let Some(b) = value.as_bool() {
                    self.add_random_query_params = b;
                }
            }
            "removeQueryParams" => {
                if let Some(b) = value.as_bool() {
                    self.remove_query_params = b;
                }
            }
            "keepQueryParams" => {
                if let Some(arr) = value.as_array() {
                    self.keep_query_params = arr.clone();
                }
            }
            "transformUrl" => {
                if let Some(arr) = value.as_array() {
                    self.transform_url = arr.clone();
                }
            }
            "forceRelativeUrls" => {
                if let Some(b) = value.as_bool() {
                    self.force_relative_urls = b;
                }
            }
            // file export options — support empty string to disable (set to None)
            "outputHtmlReport" => match value.as_str() {
                Some(s) => self.output_html_report = Some(s.to_string()),
                None => self.output_html_report = None,
            },
            "htmlReportOptions" => {
                if let Some(s) = value.as_str() {
                    self.html_report_options = Some(s.to_string());
                }
            }
            "outputJsonFile" => match value.as_str() {
                Some(s) => self.output_json_file = Some(s.to_string()),
                None => self.output_json_file = None,
            },
            "outputTextFile" => match value.as_str() {
                Some(s) => self.output_text_file = Some(s.to_string()),
                None => self.output_text_file = None,
            },
            "addHostToOutputFile" => {
                if let Some(b) = value.as_bool() {
                    self.add_host_to_output_file = b;
                }
            }
            "addTimestampToOutputFile" => {
                if let Some(b) = value.as_bool() {
                    self.add_timestamp_to_output_file = b;
                }
            }
            // sitemap options
            "outputSitemapXml" => {
                if let Some(s) = value.as_str() {
                    self.sitemap_xml_file = Some(s.to_string());
                }
            }
            "outputSitemapTxt" => {
                if let Some(s) = value.as_str() {
                    self.sitemap_txt_file = Some(s.to_string());
                }
            }
            "sitemapBasePriority" => {
                if let Some(n) = value.as_float() {
                    self.sitemap_base_priority = n;
                }
            }
            "sitemapPriorityIncrease" => {
                if let Some(n) = value.as_float() {
                    self.sitemap_priority_increase = n;
                }
            }
            // offline export options
            "offlineExportDirectory" => {
                if let Some(s) = value.as_str() {
                    self.offline_export_dir = Some(s.to_string());
                }
            }
            "offlineExportStoreOnlyUrlRegex" => {
                if let Some(arr) = value.as_array() {
                    self.offline_export_store_only_url_regex = arr.clone();
                }
            }
            "offlineExportRemoveUnwantedCode" => {
                if let Some(b) = value.as_bool() {
                    self.offline_export_remove_unwanted_code = b;
                }
            }
            "offlineExportNoAutoRedirectHtml" => {
                if let Some(b) = value.as_bool() {
                    self.offline_export_no_auto_redirect_html = b;
                }
            }
            "offlineExportPreserveUrlStructure" => {
                if let Some(b) = value.as_bool() {
                    self.offline_export_preserve_url_structure = b;
                }
            }
            "offlineExportPreserveUrls" => {
                if let Some(b) = value.as_bool() {
                    self.offline_export_preserve_urls = b;
                }
            }
            "offlineExportNoUrlRewriting" => {
                if let Some(b) = value.as_bool() {
                    self.offline_export_no_url_rewriting = b;
                }
            }
            "replaceContent" => {
                if let Some(arr) = value.as_array() {
                    self.replace_content = arr.clone();
                }
            }
            "replaceQueryString" => {
                if let Some(arr) = value.as_array() {
                    self.replace_query_string = arr.clone();
                }
            }
            "offlineExportLowercase" => {
                if let Some(b) = value.as_bool() {
                    self.offline_export_lowercase = b;
                }
            }
            "ignoreStoreFileError" => {
                if let Some(b) = value.as_bool() {
                    self.ignore_store_file_error = b;
                }
            }
            "disableAstroInlineModules" => {
                if let Some(b) = value.as_bool() {
                    self.disable_astro_inline_modules = b;
                }
            }
            // markdown export options
            "markdownExportDirectory" => {
                if let Some(s) = value.as_str() {
                    self.markdown_export_dir = Some(s.to_string());
                }
            }
            "markdownExportSingleFile" => {
                if let Some(s) = value.as_str() {
                    self.markdown_export_single_file = Some(s.to_string());
                }
            }
            "markdownMoveContentBeforeH1ToEnd" => {
                if let Some(b) = value.as_bool() {
                    self.markdown_move_content_before_h1_to_end = b;
                }
            }
            "markdownDisableImages" => {
                if let Some(b) = value.as_bool() {
                    self.markdown_disable_images = b;
                }
            }
            "markdownDisableFiles" => {
                if let Some(b) = value.as_bool() {
                    self.markdown_disable_files = b;
                }
            }
            "markdownRemoveLinksAndImagesFromSingleFile" => {
                if let Some(b) = value.as_bool() {
                    self.markdown_remove_links_and_images_from_single_file = b;
                }
            }
            "markdownExcludeSelector" => {
                if let Some(arr) = value.as_array() {
                    self.markdown_exclude_selector = arr.clone();
                }
            }
            "markdownReplaceContent" => {
                if let Some(arr) = value.as_array() {
                    self.markdown_replace_content = arr.clone();
                }
            }
            "markdownReplaceQueryString" => {
                if let Some(arr) = value.as_array() {
                    self.markdown_replace_query_string = arr.clone();
                }
            }
            "markdownExportStoreOnlyUrlRegex" => {
                if let Some(arr) = value.as_array() {
                    self.markdown_export_store_only_url_regex = arr.clone();
                }
            }
            "markdownIgnoreStoreFileError" => {
                if let Some(b) = value.as_bool() {
                    self.markdown_ignore_store_file_error = b;
                }
            }
            // mailer options
            "mailTo" => {
                if let Some(arr) = value.as_array() {
                    self.mail_to = arr.clone();
                }
            }
            "mailFrom" => {
                if let Some(s) = value.as_str() {
                    self.mail_from = s.to_string();
                }
            }
            "mailFromName" => {
                if let Some(s) = value.as_str() {
                    self.mail_from_name = s.to_string();
                }
            }
            "mailSubjectTemplate" => {
                if let Some(s) = value.as_str() {
                    self.mail_subject_template = s.to_string();
                }
            }
            "mailSmtpHost" => {
                if let Some(s) = value.as_str() {
                    self.mail_smtp_host = s.to_string();
                }
            }
            "mailSmtpPort" => {
                if let Some(n) = value.as_int() {
                    self.mail_smtp_port = n;
                }
            }
            "mailSmtpUser" => {
                if let Some(s) = value.as_str() {
                    self.mail_smtp_user = Some(s.to_string());
                }
            }
            "mailSmtpPass" => {
                if let Some(s) = value.as_str() {
                    self.mail_smtp_pass = Some(s.to_string());
                }
            }
            // upload options
            "uploadEnabled" => {
                if let Some(b) = value.as_bool() {
                    self.upload_enabled = b;
                }
            }
            "uploadTo" => {
                if let Some(s) = value.as_str() {
                    self.upload_to = s.to_string();
                }
            }
            "uploadRetention" => {
                if let Some(s) = value.as_str() {
                    self.upload_retention = s.to_string();
                }
            }
            "uploadPassword" => {
                if let Some(s) = value.as_str() {
                    self.upload_password = Some(s.to_string());
                }
            }
            "uploadTimeout" => {
                if let Some(n) = value.as_int() {
                    self.upload_timeout = n;
                }
            }
            "httpCacheDir" => match value.as_str() {
                Some(s) => self.http_cache_dir = Some(s.to_string()),
                None => self.http_cache_dir = None,
            },
            "httpCacheCompression" => {
                if let Some(b) = value.as_bool() {
                    self.http_cache_compression = b;
                }
            }
            "httpCacheTtl" => {
                if let Some(s) = value.as_str() {
                    if s == "0" || s.is_empty() || s == "off" {
                        self.http_cache_ttl = None; // infinite
                    } else {
                        self.http_cache_ttl = Some(parse_duration_to_secs(s));
                    }
                }
            }
            "noCache" => {
                if value.as_bool() == Some(true) {
                    self.http_cache_dir = Some("off".to_string());
                }
            }
            "debug" => {
                if let Some(b) = value.as_bool() {
                    self.debug = b;
                }
            }
            "debugLogFile" => {
                if let Some(s) = value.as_str() {
                    self.debug_log_file = Some(s.to_string());
                }
            }
            "debugUrlRegex" => {
                if let Some(arr) = value.as_array() {
                    self.debug_url_regex = arr.clone();
                }
            }
            // fastest analyzer options
            "fastestTopLimit" => {
                if let Some(n) = value.as_int() {
                    self.fastest_top_limit = n;
                }
            }
            "fastestMaxTime" => {
                if let Some(n) = value.as_float() {
                    self.fastest_max_time = n;
                }
            }
            // seo and opengraph analyzer options
            "maxHeadingLevel" => {
                if let Some(n) = value.as_int() {
                    self.max_heading_level = n;
                }
            }
            // slowest analyzer options
            "slowestTopLimit" => {
                if let Some(n) = value.as_int() {
                    self.slowest_top_limit = n;
                }
            }
            "slowestMinTime" => {
                if let Some(n) = value.as_float() {
                    self.slowest_min_time = n;
                }
            }
            "slowestMaxTime" => {
                if let Some(n) = value.as_float() {
                    self.slowest_max_time = n;
                }
            }
            // ci/cd options
            "ci" => {
                if let Some(b) = value.as_bool() {
                    self.ci = b;
                }
            }
            "ciMinScore" => {
                if let Some(n) = value.as_float() {
                    self.ci_min_score = n;
                }
            }
            "ciMinPerformance" => {
                if let Some(n) = value.as_float() {
                    self.ci_min_performance = Some(n);
                }
            }
            "ciMinSeo" => {
                if let Some(n) = value.as_float() {
                    self.ci_min_seo = Some(n);
                }
            }
            "ciMinSecurity" => {
                if let Some(n) = value.as_float() {
                    self.ci_min_security = Some(n);
                }
            }
            "ciMinAccessibility" => {
                if let Some(n) = value.as_float() {
                    self.ci_min_accessibility = Some(n);
                }
            }
            "ciMinBestPractices" => {
                if let Some(n) = value.as_float() {
                    self.ci_min_best_practices = Some(n);
                }
            }
            "ciMax404" => {
                if let Some(n) = value.as_int() {
                    self.ci_max_404 = n;
                }
            }
            "ciMax5xx" => {
                if let Some(n) = value.as_int() {
                    self.ci_max_5xx = n;
                }
            }
            "ciMaxCriticals" => {
                if let Some(n) = value.as_int() {
                    self.ci_max_criticals = n;
                }
            }
            "ciMaxWarnings" => {
                if let Some(n) = value.as_int() {
                    self.ci_max_warnings = Some(n);
                }
            }
            "ciMaxAvgResponse" => {
                if let Some(n) = value.as_float() {
                    self.ci_max_avg_response = Some(n);
                }
            }
            "ciMinPages" => {
                if let Some(n) = value.as_int() {
                    self.ci_min_pages = n;
                }
            }
            "ciMinAssets" => {
                if let Some(n) = value.as_int() {
                    self.ci_min_assets = n;
                }
            }
            "ciMinDocuments" => {
                if let Some(n) = value.as_int() {
                    self.ci_min_documents = n;
                }
            }
            "ciBaseline" => {
                if let Some(s) = value.as_str() {
                    self.ci_baseline = Some(s.to_string());
                }
            }
            "ciMaxScoreDrop" => {
                if let Some(n) = value.as_float() {
                    self.ci_max_score_drop = Some(n);
                }
            }
            "ciFailOnCode" => {
                if let Some(arr) = value.as_array() {
                    self.ci_fail_on_code = arr.clone();
                }
            }
            "ciIgnoreCode" => {
                if let Some(arr) = value.as_array() {
                    self.ci_ignore_code = arr.clone();
                }
            }
            "ciJunitFile" => {
                if let Some(s) = value.as_str() {
                    self.ci_junit_file = Some(s.to_string());
                }
            }
            "ciGithubAnnotations" => {
                if let Some(b) = value.as_bool() {
                    self.ci_github_annotations = b;
                }
            }
            "serveMarkdownDirectory" => {
                if let Some(s) = value.as_str() {
                    self.serve_markdown_dir = Some(s.to_string());
                }
            }
            "serveOfflineDirectory" => {
                if let Some(s) = value.as_str() {
                    self.serve_offline_dir = Some(s.to_string());
                }
            }
            "servePort" => {
                if let Some(n) = value.as_int() {
                    self.serve_port = n;
                }
            }
            "serveBindAddress" => {
                if let Some(s) = value.as_str() {
                    self.serve_bind_address = s.to_string();
                }
            }
            "htmlToMarkdownFile" => {
                if let Some(s) = value.as_str() {
                    self.html_to_markdown_file = Some(s.to_string());
                }
            }
            "htmlToMarkdownOutput" => {
                if let Some(s) = value.as_str() {
                    self.html_to_markdown_output = Some(s.to_string());
                }
            }
            // ai options
            "aiProvider" => {
                if let Some(s) = value.as_str() {
                    self.ai_provider = s.to_string();
                }
            }
            "aiEndpoint" => {
                if let Some(s) = value.as_str() {
                    self.ai_endpoint = Some(s.to_string());
                }
            }
            "aiModel" => {
                if let Some(s) = value.as_str() {
                    self.ai_model = Some(s.to_string());
                }
            }
            "aiApiKey" => {
                if let Some(s) = value.as_str() {
                    self.ai_api_key = Some(SecretString::new(s));
                }
            }
            "aiApiKeyEnv" => {
                if let Some(s) = value.as_str() {
                    self.ai_api_key_env = Some(s.to_string());
                }
            }
            "aiApiKeyFile" => {
                if let Some(s) = value.as_str() {
                    self.ai_api_key_file = Some(s.to_string());
                }
            }
            "aiMaxTokens" => {
                if let Some(n) = value.as_int() {
                    self.ai_max_tokens = n;
                }
            }
            "aiUseMaxCompletionTokens" => {
                if let Some(b) = value.as_bool() {
                    self.ai_use_max_completion_tokens = b;
                }
            }
            "aiTemperature" => {
                if let Some(f) = value.as_float() {
                    self.ai_temperature = f;
                }
            }
            "aiExtraBody" => {
                if let Some(s) = value.as_str() {
                    self.ai_extra_body = Some(s.to_string());
                }
            }
            "aiSynthesisExtraBody" => {
                if let Some(s) = value.as_str() {
                    self.ai_synthesis_extra_body = Some(s.to_string());
                }
            }
            "aiActions" => {
                if let Some(arr) = value.as_array() {
                    self.ai_actions = arr.clone();
                }
            }
            "aiPromptFile" => {
                if let Some(s) = value.as_str() {
                    self.ai_prompt_file = Some(s.to_string());
                }
            }
            "aiPrompt" => {
                if let Some(s) = value.as_str() {
                    self.ai_prompt = Some(s.to_string());
                }
            }
            "aiLanguage" => {
                if let Some(s) = value.as_str() {
                    self.ai_language = Some(s.to_string());
                }
            }
            "aiInclude" => {
                if let Some(arr) = value.as_array() {
                    self.ai_include = arr.clone();
                }
            }
            "aiExclude" => {
                if let Some(arr) = value.as_array() {
                    self.ai_exclude = arr.clone();
                }
            }
            "aiMaxPages" => {
                if let Some(n) = value.as_int() {
                    self.ai_max_pages = n;
                }
            }
            "aiMaxConcurrency" => {
                if let Some(n) = value.as_int() {
                    self.ai_max_concurrency = n;
                }
            }
            "aiMaxReqsPerSec" => {
                if let Some(f) = value.as_float() {
                    self.ai_max_reqs_per_sec = Some(f);
                }
            }
            "aiTimeout" => {
                if let Some(n) = value.as_int() {
                    self.ai_timeout = n;
                }
            }
            "aiCacheDir" => match value.as_str() {
                Some(s) => self.ai_cache_dir = Some(s.to_string()),
                None => self.ai_cache_dir = None,
            },
            "aiSeoAffectsScore" => {
                if let Some(b) = value.as_bool() {
                    self.ai_seo_affects_score = b;
                }
            }
            "aiDryRun" => {
                if let Some(b) = value.as_bool() {
                    self.ai_dry_run = b;
                }
            }
            "browserEnabled" => {
                if let Some(b) = value.as_bool() {
                    self.browser_enabled = b;
                }
            }
            "browserPath" => {
                if let Some(s) = value.as_str() {
                    self.browser_path = Some(s.to_string());
                }
            }
            "browserHeadful" => {
                if let Some(b) = value.as_bool() {
                    self.browser_headful = b;
                }
            }
            "browserWorkers" => {
                if let Some(n) = value.as_int() {
                    self.browser_workers = n;
                }
            }
            "browserWait" => {
                if let Some(s) = value.as_str() {
                    self.browser_wait = s.to_string();
                }
            }
            "browserWaitExtraMs" => {
                if let Some(n) = value.as_int() {
                    self.browser_wait_extra_ms = n;
                }
            }
            "browserTimeout" => {
                if let Some(n) = value.as_int() {
                    self.browser_timeout = n;
                }
            }
            "browserRenderAll" => {
                if let Some(b) = value.as_bool() {
                    self.browser_render_all = b;
                }
            }
            "browserAutoDownload" => {
                if let Some(b) = value.as_bool() {
                    self.browser_auto_download = b;
                }
            }
            "screenshots" => {
                if let Some(b) = value.as_bool() {
                    self.screenshots = b;
                }
            }
            "screenshotsDir" => {
                if let Some(s) = value.as_str() {
                    self.screenshots_dir = Some(s.to_string());
                }
            }
            "screenshotMode" => {
                if let Some(s) = value.as_str() {
                    self.screenshot_mode = s.to_string();
                }
            }
            "screenshotViewport" => {
                if let Some(s) = value.as_str() {
                    self.screenshot_viewport = s.to_string();
                }
            }
            "screenshotFormat" => {
                if let Some(s) = value.as_str() {
                    self.screenshot_format = s.to_string();
                }
            }
            "screenshotQuality" => {
                if let Some(n) = value.as_int() {
                    self.screenshot_quality = n;
                }
            }
            "screenshotHideCookieBanners" => {
                if let Some(b) = value.as_bool() {
                    self.screenshot_hide_cookie_banners = b;
                }
            }
            "screenshotHideSelector" => {
                if let Some(s) = value.as_str() {
                    self.screenshot_hide_selector = Some(s.to_string());
                }
            }
            "screenshotsAnimation" => {
                if let Some(s) = value.as_str() {
                    self.screenshots_animation = s.to_string();
                }
            }
            "screenshotsAnimationFrameDuration" => {
                if let Some(n) = value.as_float() {
                    self.screenshots_animation_frame_duration = n;
                }
            }
            "screenshotsAnimationWidth" => {
                if let Some(n) = value.as_int() {
                    self.screenshots_animation_width = n;
                }
            }
            "ffmpegPath" => {
                if let Some(s) = value.as_str() {
                    self.ffmpeg_path = Some(s.to_string());
                }
            }
            "browserNoSandbox" => {
                if let Some(b) = value.as_bool() {
                    self.browser_no_sandbox = b;
                }
            }
            "consoleMaxMessages" => {
                if let Some(n) = value.as_int() {
                    self.console_max_messages = n;
                }
            }
            "consoleMsgMaxChars" => {
                if let Some(n) = value.as_int() {
                    self.console_msg_max_chars = n;
                }
            }
            "consoleTotalMaxKb" => {
                if let Some(n) = value.as_int() {
                    self.console_total_max_kb = n;
                }
            }
            _ => {
                // Unknown property - ignore (may be from analyzer/exporter options)
            }
        }
        Ok(())
    }

    pub fn has_header_to_table(&self, header_name: &str) -> bool {
        self.extra_columns_names_only.iter().any(|name| name == header_name)
    }

    pub fn is_url_selected_for_debug(&self, url: &str) -> bool {
        if self.debug_url_regex.is_empty() {
            return false;
        }

        for regex_str in &self.debug_url_regex {
            if let Ok(re) = Regex::new(regex_str)
                && re.is_match(url)
            {
                return true;
            }
        }

        false
    }

    pub fn crawl_only_html_files(&self) -> bool {
        self.disable_all_assets
            || (self.disable_javascript
                && self.disable_styles
                && self.disable_fonts
                && self.disable_images
                && self.disable_files)
    }

    /// Get initial host from URL (with port if explicitly set)
    pub fn get_initial_host(&self, include_port_if_defined: bool) -> String {
        if let Ok(parsed) = url::Url::parse(&self.url) {
            let host = parsed.host_str().unwrap_or("").to_string();
            if include_port_if_defined && let Some(port) = parsed.port() {
                return format!("{}:{}", host, port);
            }
            host
        } else {
            String::new()
        }
    }

    /// Get scheme from initial URL
    pub fn get_initial_scheme(&self) -> String {
        if let Ok(parsed) = url::Url::parse(&self.url) {
            parsed.scheme().to_string()
        } else {
            String::new()
        }
    }
}

/// Build the complete Options registry with all option groups.
pub fn get_options() -> Options {
    let mut options = Options::new();

    // -------------------------------------------------------------------------
    // Basic settings (CoreOptions group 1)
    // -------------------------------------------------------------------------
    options.add_group(OptionGroup::new(
        GROUP_BASIC_SETTINGS,
        "Basic settings",
        vec![
            CrawlerOption::new(
                "--url", Some("-u"), "url", OptionType::Url, false,
                "Required URL. It can also be the URL to sitemap.xml. Enclose in quotes if URL contains query parameters.",
                None, true, false, None,
            ),
            CrawlerOption::new(
                "--url-list", None, "urlList", OptionType::String, false,
                "Path to a plain-text file with one URL per line (blank lines and `#` comments ignored). When provided, --url is optional; the first URL in the file is used as the crawl base. All listed URLs are seeded into the crawl queue.",
                None, true, false, None,
            ),
            CrawlerOption::new(
                "--single-page", Some("-sp"), "singlePage", OptionType::Bool, false,
                "Load only one page to which the URL is given (and its assets), but do not follow other pages.",
                Some("false"), false, false, None,
            ),
            CrawlerOption::new(
                "--max-depth", Some("-md"), "maxDepth", OptionType::Int, false,
                "Maximum crawling depth (for pages, not assets). Default is `0` (no limit). `1` means `/about` or `/about/`, `2` means `/about/contacts` etc.",
                Some("0"), false, false, None,
            ),
            CrawlerOption::new(
                "--device", Some("-d"), "device", OptionType::String, false,
                "Device type for User-Agent selection. Values `desktop`, `tablet`, `mobile`. Ignored with `--user-agent`.",
                Some("desktop"), false, false, None,
            ),
            CrawlerOption::new(
                "--user-agent", Some("-ua"), "userAgent", OptionType::String, false,
                "Override User-Agent selected by --device. If you add `!` at the end, the SiteOne-Crawler/version will not be added as a signature at the end of the final user-agent.",
                None, true, false, None,
            ),
            CrawlerOption::new(
                "--timeout", Some("-t"), "timeout", OptionType::Int, false,
                "Request timeout (in sec).",
                Some("5"), false, false, None,
            ),
            CrawlerOption::new(
                "--proxy", Some("-p"), "proxy", OptionType::HostAndPort, false,
                "HTTP proxy in `host:port` format.",
                None, true, false, None,
            ),
            CrawlerOption::new(
                "--http-auth", Some("-ha"), "httpAuth", OptionType::String, false,
                "Basic HTTP authentication in `username:password` format.",
                None, true, false, None,
            ),
            CrawlerOption::new(
                "--accept-invalid-certs", Some("-aic"), "acceptInvalidCerts", OptionType::Bool, false,
                "Accept invalid or incomplete SSL/TLS certificates (e.g. expired, self-signed, or missing intermediate CA). Use with caution.",
                Some("false"), false, false, None,
            ),
            CrawlerOption::new(
                "--help", Some("-h"), "showHelpOnly", OptionType::Bool, false,
                "Show help and exit.",
                Some("false"), false, false, None,
            ),
            CrawlerOption::new(
                "--version", Some("-v"), "showVersionOnly", OptionType::Bool, false,
                "Show crawler version and exit.",
                Some("false"), false, false, None,
            ),
        ],
    ));

    // -------------------------------------------------------------------------
    // Output settings (CoreOptions group 2)
    // -------------------------------------------------------------------------
    options.add_group(OptionGroup::new(
        GROUP_OUTPUT_SETTINGS,
        "Output settings",
        vec![
            CrawlerOption::new(
                "--output", Some("-o"), "outputType", OptionType::String, false,
                "Output type `text` or `json`.",
                Some("text"), false, false, None,
            ),
            CrawlerOption::new(
                "--extra-columns", Some("-ec"), "extraColumns", OptionType::String, true,
                "Extra table headers for output table with option to set width and do-not-truncate (>), e.g., `DOM,X-Cache(10),Title(40>)`.",
                None, true, true, None,
            ),
            CrawlerOption::new(
                "--url-column-size", Some("-ucs"), "urlColumnSize", OptionType::Int, false,
                "URL column width. By default, it is calculated from the size of your terminal window.",
                None, true, false, None,
            ),
            CrawlerOption::new(
                "--timezone", Some("-tz"), "timezone", OptionType::String, false,
                "Timezone for datetimes in HTML reports and timestamps in output folders/files, e.g., `Europe/Prague`. Default is `UTC`.",
                None, true, false, None,
            ),
            CrawlerOption::new(
                "--rows-limit", Some("-rl"), "rowsLimit", OptionType::Int, false,
                "Max. number of rows to display in tables with analysis results (protection against very long and slow report)",
                Some("200"), false, false, None,
            ),
            CrawlerOption::new(
                "--show-inline-criticals", Some("-sic"), "showInlineCriticals", OptionType::Bool, false,
                "Show criticals from the analyzer directly in the URL table.",
                Some("false"), false, false, None,
            ),
            CrawlerOption::new(
                "--show-inline-warnings", Some("-siw"), "showInlineWarnings", OptionType::Bool, false,
                "Show warnings from the analyzer directly in the URL table.",
                Some("false"), false, false, None,
            ),
            CrawlerOption::new(
                "--do-not-truncate-url", Some("-dntu"), "doNotTruncateUrl", OptionType::Bool, false,
                "Avoid truncating URLs to `--url-column-size`.",
                Some("false"), false, false, None,
            ),
            CrawlerOption::new(
                "--show-scheme-and-host", Some("-ssah"), "showSchemeAndHost", OptionType::Bool, false,
                "Show the schema://host also of the original domain URL as well. By default, only path+query is displayed for original domain.",
                Some("false"), false, false, None,
            ),
            CrawlerOption::new(
                "--hide-progress-bar", Some("-hpb"), "hideProgressBar", OptionType::Bool, false,
                "Suppress progress bar in output.",
                Some("false"), false, false, None,
            ),
            CrawlerOption::new(
                "--hide-columns", Some("-hc"), "hideColumns", OptionType::String, false,
                "Hide specified columns from the progress table. Comma-separated list: type, time, size, cache.",
                None, true, false, None,
            ),
            CrawlerOption::new(
                "--no-color", Some("-nc"), "noColor", OptionType::Bool, false,
                "Disable colored output.",
                Some("false"), false, false, None,
            ),
            CrawlerOption::new(
                "--force-color", Some("-fc"), "forceColor", OptionType::Bool, false,
                "Force colored output regardless of support detection.",
                Some("false"), false, false, None,
            ),
        ],
    ));

    // -------------------------------------------------------------------------
    // Resource filtering (CoreOptions group 3)
    // -------------------------------------------------------------------------
    options.add_group(OptionGroup::new(
        GROUP_RESOURCE_FILTERING,
        "Resource filtering",
        vec![
            CrawlerOption::new(
                "--disable-all-assets", Some("-das"), "disableAllAssets", OptionType::Bool, false,
                "Disables crawling of all assets and files and only crawls pages in href attributes. Shortcut for calling all other `--disable-*` flags.",
                Some("false"), false, false, None,
            ),
            CrawlerOption::new(
                "--disable-javascript", Some("-dj"), "disableJavascript", OptionType::Bool, false,
                "Disables JavaScript downloading and removes all JavaScript code from HTML, including onclick and other on* handlers.",
                Some("false"), false, false, None,
            ),
            CrawlerOption::new(
                "--disable-styles", Some("-ds"), "disableStyles", OptionType::Bool, false,
                "Disables CSS file downloading and at the same time removes all style definitions by <style> tag or inline by style attributes.",
                Some("false"), false, false, None,
            ),
            CrawlerOption::new(
                "--disable-fonts", Some("-dfo"), "disableFonts", OptionType::Bool, false,
                "Disables font downloading and also removes all font/font-face definitions from CSS.",
                Some("false"), false, false, None,
            ),
            CrawlerOption::new(
                "--disable-images", Some("-di"), "disableImages", OptionType::Bool, false,
                "Disables downloading of all images and replaces found images in HTML with placeholder image only.",
                Some("false"), false, false, None,
            ),
            CrawlerOption::new(
                "--disable-files", Some("-df"), "disableFiles", OptionType::Bool, false,
                "Disables downloading of any files (typically downloadable documents) to which various links point.",
                Some("false"), false, false, None,
            ),
            CrawlerOption::new(
                "--remove-all-anchor-listeners", Some("-raal"), "removeAllAnchorListeners", OptionType::Bool, false,
                "On all links on the page remove any event listeners. Useful on some types of sites with modern JS frameworks.",
                Some("false"), false, false, None,
            ),
        ],
    ));

    // -------------------------------------------------------------------------
    // Advanced crawler settings (CoreOptions group 4)
    // -------------------------------------------------------------------------
    options.add_group(OptionGroup::new(
        GROUP_ADVANCED_CRAWLER_SETTINGS,
        "Advanced crawler settings",
        vec![
            CrawlerOption::new(
                "--workers", Some("-w"), "workers", OptionType::Int, false,
                "Max concurrent workers (threads). Crawler will not make more simultaneous requests to the server than this number.",
                Some("3"), false, false, None,
            ),
            CrawlerOption::new(
                "--max-reqs-per-sec", Some("-rps"), "maxReqsPerSec", OptionType::Float, false,
                "Max requests/s for whole crawler. Be careful not to cause a DoS attack.",
                Some("10"), false, false, None,
            ),
            CrawlerOption::new(
                "--memory-limit", Some("-ml"), "memoryLimit", OptionType::SizeMG, false,
                "Memory limit in units M (Megabytes) or G (Gigabytes).",
                Some("2048M"), false, false, None,
            ),
            CrawlerOption::new(
                "--resolve", Some("-res"), "resolve", OptionType::Resolve, true,
                "The ability to force the domain+port to resolve to its own IP address, just like CURL --resolve does. Example: `--resolve='www.mydomain.tld:80:127.0.0.1'`",
                None, true, true, None,
            ),
            CrawlerOption::new(
                "--allowed-domain-for-external-files", Some("-adf"), "allowedDomainsForExternalFiles", OptionType::String, true,
                "Primarily, the crawler crawls only the URL within the domain for initial URL. This allows you to enable loading of file content from another domain as well (e.g. if you want to load assets from a CDN). Can be specified multiple times. Use can use domains with wildcard '*'.",
                None, true, true, None,
            ),
            CrawlerOption::new(
                "--allowed-domain-for-crawling", Some("-adc"), "allowedDomainsForCrawling", OptionType::String, true,
                "This option will allow you to crawl all content from other listed domains - typically in the case of language mutations on other domains. Can be specified multiple times. Use can use domains with wildcard '*'.",
                None, true, true, None,
            ),
            CrawlerOption::new(
                "--single-foreign-page", Some("-sfp"), "singleForeignPage", OptionType::Bool, false,
                "If crawling of other domains is allowed (using `--allowed-domain-for-crawling`), it ensures that when another domain is not on same second-level domain, only that linked page and its assets are crawled from that foreign domain.",
                Some("false"), false, false, None,
            ),
            CrawlerOption::new(
                "--include-regex", Some("--include-regexp"), "includeRegex", OptionType::Regex, true,
                "Include only URLs matching at least one PCRE regex. Can be specified multiple times.",
                None, false, true, None,
            ),
            CrawlerOption::new(
                "--ignore-regex", Some("--ignore-regexp"), "ignoreRegex", OptionType::Regex, true,
                "Ignore URLs matching any PCRE regex. Can be specified multiple times.",
                None, false, true, None,
            ),
            CrawlerOption::new(
                "--regex-filtering-only-for-pages", None, "regexFilteringOnlyForPages", OptionType::Bool, false,
                "Set if you want filtering by `*-regex` rules apply only to page URLs, but static assets are loaded regardless of filtering.",
                Some("false"), false, false, None,
            ),
            CrawlerOption::new(
                "--analyzer-filter-regex", Some("--analyzer-filter-regexp"), "analyzerFilterRegex", OptionType::Regex, false,
                "Use only analyzers that match the specified regexp.",
                None, true, false, None,
            ),
            CrawlerOption::new(
                "--accept-encoding", None, "acceptEncoding", OptionType::String, false,
                "Set `Accept-Encoding` request header.",
                Some("gzip, deflate, br"), false, false, None,
            ),
            CrawlerOption::new(
                "--remove-query-params", Some("-rqp"), "removeQueryParams", OptionType::Bool, false,
                "Remove URL query parameters from crawled URLs.",
                Some("false"), false, false, None,
            ),
            CrawlerOption::new(
                "--keep-query-param", Some("-kqp"), "keepQueryParams", OptionType::String, true,
                "Keep only the specified query parameter(s) in discovered URLs. All other query parameters are removed. Can be specified multiple times. Ignored when `--remove-query-params` is active.",
                None, true, true, None,
            ),
            CrawlerOption::new(
                "--add-random-query-params", Some("-arqp"), "addRandomQueryParams", OptionType::Bool, false,
                "Add random query parameters to each crawled URL.",
                Some("false"), false, false, None,
            ),
            CrawlerOption::new(
                "--transform-url", Some("-tu"), "transformUrl", OptionType::ReplaceContent, true,
                "Transform URLs before crawling. Format: `from -> to` or `/regex/ -> replacement`. Example: `live-site.com -> local-site.local` or `/live-site\\.com\\/wp/ -> local-site.local/`. Can be specified multiple times.",
                None, true, true, None,
            ),
            CrawlerOption::new(
                "--force-relative-urls", Some("-fru"), "forceRelativeUrls", OptionType::Bool, false,
                "Normalize all discovered URLs matching the initial domain (incl. www variant and protocol differences) to relative paths. Prevents duplicate files in offline export when the site uses inconsistent URL formats.",
                Some("false"), false, false, None,
            ),
            CrawlerOption::new(
                "--ignore-robots-txt", Some("-irt"), "ignoreRobotsTxt", OptionType::Bool, false,
                "Should robots.txt content be ignored? Useful for crawling an otherwise private/unindexed site.",
                Some("false"), false, false, None,
            ),
            CrawlerOption::new(
                "--ignore-html-comments", Some("-ihc"), "ignoreHtmlComments", OptionType::Bool, false,
                "Ignore URLs found inside HTML comments (<!-- ... -->), which search engines also ignore, so commented links are not crawled or reported as broken.",
                Some("false"), false, false, None,
            ),
            CrawlerOption::new(
                "--max-queue-length", Some("-mql"), "maxQueueLength", OptionType::Int, false,
                "Max URL queue length. It affects memory requirements.",
                Some("9000"), false, false, None,
            ),
            CrawlerOption::new(
                "--max-visited-urls", Some("-mvu"), "maxVisitedUrls", OptionType::Int, false,
                "Max visited URLs. It affects memory requirements.",
                Some("10000"), false, false, None,
            ),
            CrawlerOption::new(
                "--max-skipped-urls", Some("-msu"), "maxSkippedUrls", OptionType::Int, false,
                "Max skipped URLs. It affects memory requirements.",
                Some("10000"), false, false, None,
            ),
            CrawlerOption::new(
                "--max-url-length", Some("-mul"), "maxUrlLength", OptionType::Int, false,
                "Max URL length in chars. It affects memory requirements.",
                Some("2083"), false, false, None,
            ),
            CrawlerOption::new(
                "--max-non200-responses-per-basename", Some("-mnrpb"), "maxNon200ResponsesPerBasename", OptionType::Int, false,
                "Protection against looping with dynamic non-200 URLs. If a basename (the last part of the URL after the last slash) has more non-200 responses than this limit, other URLs with same basename will be ignored/skipped.",
                Some("5"), false, false, None,
            ),
        ],
    ));

    // -------------------------------------------------------------------------
    // Expert settings (CoreOptions group 5)
    // -------------------------------------------------------------------------
    options.add_group(OptionGroup::new(
        GROUP_EXPERT_SETTINGS,
        "Expert settings",
        vec![
            CrawlerOption::new(
                "--debug", None, "debug", OptionType::Bool, false,
                "Activate debug mode.",
                Some("false"), true, false, None,
            ),
            CrawlerOption::new(
                "--debug-log-file", None, "debugLogFile", OptionType::File, false,
                "Log file where to save debug messages. When --debug is not set and --debug-log-file is set, logging will be active without visible output.",
                None, true, false, None,
            ),
            CrawlerOption::new(
                "--debug-url-regex", None, "debugUrlRegex", OptionType::Regex, true,
                "Regex for URL(s) to debug. When crawled URL is matched, parsing, URL replacing and other actions are printed to output. Can be specified multiple times.",
                None, true, true, None,
            ),
            CrawlerOption::new(
                "--result-storage", Some("-rs"), "resultStorage", OptionType::String, false,
                "Result storage type for content and headers. Values: `memory` or `file`. Use `file` for large websites.",
                Some("memory"), false, false, None,
            ),
            {
                let prefix = default_output_prefix();
                CrawlerOption::new(
                    "--result-storage-dir", Some("-rsd"), "resultStorageDir", OptionType::Dir, false,
                    "Directory for --result-storage=file.",
                    Some(&format!("{prefix}/result-storage")), false, false, None,
                )
            },
            CrawlerOption::new(
                "--result-storage-compression", Some("-rsc"), "resultStorageCompression", OptionType::Bool, false,
                "Enable compression for results storage. Saves disk space, but uses more CPU.",
                Some("false"), false, false, None,
            ),
            {
                let cache_default = default_http_cache_dir();
                CrawlerOption::new(
                    "--http-cache-dir", Some("-hcd"), "httpCacheDir", OptionType::Dir, false,
                    "Cache dir for HTTP responses. Disable with --http-cache-dir='off' or --no-cache.",
                    Some(&cache_default), false, false, None,
                )
            },
            CrawlerOption::new(
                "--http-cache-compression", Some("-hcc"), "httpCacheCompression", OptionType::Bool, false,
                "Enable compression for HTTP cache storage. Saves disk space, but uses more CPU.",
                Some("false"), true, false, None,
            ),
            CrawlerOption::new(
                "--http-cache-ttl", Some("-hct"), "httpCacheTtl", OptionType::String, false,
                "TTL for HTTP cache entries (e.g. '1h', '7d', '30m'). Use '0' for infinite. Default: 24h.",
                Some("24h"), false, false, None,
            ),
            CrawlerOption::new(
                "--no-cache", None, "noCache", OptionType::Bool, false,
                "Disable HTTP cache completely. Shortcut for --http-cache-dir='off'.",
                Some("false"), false, false, None,
            ),
            CrawlerOption::new(
                "--websocket-server", Some("-ws"), "websocketServer", OptionType::HostAndPort, false,
                "Start crawler with websocket server on given host:port, typically `0.0.0.0:8000`.",
                None, true, false, None,
            ),
            CrawlerOption::new(
                "--console-width", Some("-cw"), "consoleWidth", OptionType::Int, false,
                "Enforce the definition of the console width and disable automatic detection.",
                None, true, false, None,
            ),
        ],
    ));

    // -------------------------------------------------------------------------
    // File export settings (FileExporter - alphabetically first exporter)
    // -------------------------------------------------------------------------
    options.add_group(OptionGroup::new(
        GROUP_FILE_EXPORT_SETTINGS,
        "File export settings",
        vec![
            {
                let prefix = default_output_prefix();
                let sep = std::path::MAIN_SEPARATOR;
                CrawlerOption::new(
                    "--output-html-report", None, "outputHtmlReport", OptionType::File, false,
                    "Save HTML report into that file. Set to empty '' to disable HTML report.",
                    Some(&format!("{prefix}{sep}%domain%.report.%datetime%.html")), true, false, None,
                )
            },
            CrawlerOption::new(
                "--html-report-options", None, "htmlReportOptions", OptionType::String, false,
                "Comma-separated list of sections to include in HTML report. Available sections: summary, seo-opengraph, image-gallery, video-gallery, visited-urls, dns-ssl, crawler-stats, crawler-info, headers, content-types, skipped-urls, caching, best-practices, accessibility, security, redirects, 404-pages, slowest-urls, fastest-urls, source-domains. Default: all sections.",
                None, true, false, None,
            ),
            {
                let prefix = default_output_prefix();
                let sep = std::path::MAIN_SEPARATOR;
                CrawlerOption::new(
                    "--output-json-file", None, "outputJsonFile", OptionType::File, false,
                    "Save report as JSON. Set to empty '' to disable JSON report.",
                    Some(&format!("{prefix}{sep}%domain%.output.%datetime%.json")), true, false, None,
                )
            },
            {
                let prefix = default_output_prefix();
                let sep = std::path::MAIN_SEPARATOR;
                CrawlerOption::new(
                    "--output-text-file", None, "outputTextFile", OptionType::File, false,
                    "Save output as TXT. Set to empty '' to disable TXT report.",
                    Some(&format!("{prefix}{sep}%domain%.output.%datetime%.txt")), true, false, None,
                )
            },
            CrawlerOption::new(
                "--add-host-to-output-file", None, "addHostToOutputFile", OptionType::Bool, false,
                "Append initial URL host to filename except sitemaps.",
                Some("false"), false, false, None,
            ),
            CrawlerOption::new(
                "--add-timestamp-to-output-file", None, "addTimestampToOutputFile", OptionType::Bool, false,
                "Append timestamp to filename except sitemaps.",
                Some("false"), false, false, None,
            ),
        ],
    ));

    // -------------------------------------------------------------------------
    // Mailer options (MailerExporter)
    // -------------------------------------------------------------------------
    options.add_group(OptionGroup::new(
        GROUP_MAILER_SETTINGS,
        "Mailer options",
        vec![
            CrawlerOption::new(
                "--mail-to",
                None,
                "mailTo",
                OptionType::Email,
                true,
                "E-mail report recipient address(es). Can be specified multiple times.",
                None,
                true,
                true,
                None,
            ),
            CrawlerOption::new(
                "--mail-from",
                None,
                "mailFrom",
                OptionType::Email,
                false,
                "E-mail sender address.",
                Some("siteone-crawler@your-hostname.com"),
                false,
                false,
                None,
            ),
            CrawlerOption::new(
                "--mail-from-name",
                None,
                "mailFromName",
                OptionType::String,
                false,
                "E-mail sender name",
                Some("SiteOne Crawler"),
                false,
                false,
                None,
            ),
            CrawlerOption::new(
                "--mail-subject-template",
                None,
                "mailSubjectTemplate",
                OptionType::String,
                false,
                "E-mail subject template. You can use dynamic variables %domain% and %datetime%",
                Some("Crawler Report for %domain% (%date%)"),
                true,
                false,
                None,
            ),
            CrawlerOption::new(
                "--mail-smtp-host",
                None,
                "mailSmtpHost",
                OptionType::String,
                false,
                "SMTP host.",
                Some("localhost"),
                true,
                false,
                None,
            ),
            CrawlerOption::new(
                "--mail-smtp-port",
                None,
                "mailSmtpPort",
                OptionType::Int,
                false,
                "SMTP port.",
                Some("25"),
                true,
                false,
                Some(vec!["1".to_string(), "65535".to_string()]),
            ),
            CrawlerOption::new(
                "--mail-smtp-user",
                None,
                "mailSmtpUser",
                OptionType::String,
                false,
                "SMTP user for authentication.",
                None,
                true,
                false,
                None,
            ),
            CrawlerOption::new(
                "--mail-smtp-pass",
                None,
                "mailSmtpPass",
                OptionType::String,
                false,
                "SMTP password for authentication.",
                None,
                true,
                false,
                None,
            ),
        ],
    ));

    // -------------------------------------------------------------------------
    // Markdown exporter options (MarkdownExporter)
    // -------------------------------------------------------------------------
    options.add_group(OptionGroup::new(
        GROUP_MARKDOWN_EXPORT_SETTINGS,
        "Markdown exporter options",
        vec![
            CrawlerOption::new(
                "--markdown-export-dir", Some("-med"), "markdownExportDirectory", OptionType::Dir, false,
                "Path to directory where to save the markdown version of the website.",
                None, true, false, None,
            ),
            CrawlerOption::new(
                "--markdown-export-single-file", None, "markdownExportSingleFile", OptionType::File, false,
                "Path to a file where to save the combined markdown files into one document. Requires --markdown-export-dir to be set.",
                None, true, false, None,
            ),
            CrawlerOption::new(
                "--markdown-move-content-before-h1-to-end", None, "markdownMoveContentBeforeH1ToEnd", OptionType::Bool, false,
                "Move all content before the main H1 heading (typically the header with the menu) to the end of the markdown.",
                Some("false"), true, false, None,
            ),
            CrawlerOption::new(
                "--markdown-disable-images", Some("-mdi"), "markdownDisableImages", OptionType::Bool, false,
                "Do not export and show images in markdown files. Images are enabled by default.",
                Some("false"), true, false, None,
            ),
            CrawlerOption::new(
                "--markdown-disable-files", Some("-mdf"), "markdownDisableFiles", OptionType::Bool, false,
                "Do not export and link files other than HTML/CSS/JS/fonts/images - eg. PDF, ZIP, etc. These files are enabled by default.",
                Some("false"), true, false, None,
            ),
            CrawlerOption::new(
                "--markdown-remove-links-and-images-from-single-file", None, "markdownRemoveLinksAndImagesFromSingleFile", OptionType::Bool, false,
                "Remove links and images from the combined single markdown file. Useful for AI tools that don't need these elements.",
                Some("false"), false, false, None,
            ),
            CrawlerOption::new(
                "--markdown-exclude-selector", Some("-mes"), "markdownExcludeSelector", OptionType::String, true,
                "Exclude some page content (DOM elements) from markdown export defined by CSS selectors like 'header', '.header', '#header', etc.",
                None, false, true, None,
            ),
            CrawlerOption::new(
                "--markdown-replace-content", None, "markdownReplaceContent", OptionType::ReplaceContent, true,
                "Replace text content with `foo -> bar` or regexp in PREG format: `/card[0-9]/i -> card`",
                None, true, true, None,
            ),
            CrawlerOption::new(
                "--markdown-replace-query-string", None, "markdownReplaceQueryString", OptionType::ReplaceContent, true,
                "Instead of using a short hash instead of a query string in the filename, just replace some characters. You can use simple format 'foo -> bar' or regexp in PREG format, e.g. '/([a-z]+)=([^&]*)(&|$)/i -> $1__$2'",
                None, true, true, None,
            ),
            CrawlerOption::new(
                "--markdown-export-store-only-url-regex", None, "markdownExportStoreOnlyUrlRegex", OptionType::Regex, true,
                "For debug - when filled it will activate debug mode and store only URLs which match one of these PCRE regexes. Can be specified multiple times.",
                None, true, true, None,
            ),
            CrawlerOption::new(
                "--markdown-ignore-store-file-error", None, "markdownIgnoreStoreFileError", OptionType::Bool, false,
                "Ignores any file storing errors. The export process will continue.",
                Some("false"), false, false, None,
            ),
        ],
    ));

    // -------------------------------------------------------------------------
    // Offline exporter options (OfflineWebsiteExporter)
    // -------------------------------------------------------------------------
    options.add_group(OptionGroup::new(
        GROUP_OFFLINE_EXPORT_SETTINGS,
        "Offline exporter options",
        vec![
            CrawlerOption::new(
                "--offline-export-dir", Some("-oed"), "offlineExportDirectory", OptionType::Dir, false,
                "Path to directory where to save the offline version of the website.",
                None, true, false, None,
            ),
            CrawlerOption::new(
                "--offline-export-store-only-url-regex", None, "offlineExportStoreOnlyUrlRegex", OptionType::Regex, true,
                "For debug - when filled it will activate debug mode and store only URLs which match one of these PCRE regexes. Can be specified multiple times.",
                None, true, true, None,
            ),
            CrawlerOption::new(
                "--offline-export-remove-unwanted-code", None, "offlineExportRemoveUnwantedCode", OptionType::Bool, false,
                "Remove unwanted code for offline mode? Typically JS of the analytics, social networks, cookie consent, cross origins, etc.",
                Some("true"), false, false, None,
            ),
            CrawlerOption::new(
                "--offline-export-no-auto-redirect-html", None, "offlineExportNoAutoRedirectHtml", OptionType::Bool, false,
                "Disable automatic creation of redirect HTML files for subfolders that contain an index.html file. This solves situations for URLs where sometimes the URL ends with a slash, sometimes it doesn't.",
                Some("false"), false, false, None,
            ),
            CrawlerOption::new(
                "--offline-export-preserve-url-structure", None, "offlineExportPreserveUrlStructure", OptionType::Bool, false,
                "Preserve the original URL path structure. E.g. /about is stored as about/index.html instead of about.html. Useful for web server deployment.",
                Some("false"), false, false, None,
            ),
            CrawlerOption::new(
                "--offline-export-preserve-urls", None, "offlineExportPreserveUrls", OptionType::Bool, false,
                "Preserve original URL format in exported HTML/CSS/JS. Same-domain links become root-relative (/path), cross-domain links stay absolute. Useful when exported HTML is processed by tools that need production URLs.",
                Some("false"), false, false, None,
            ),
            CrawlerOption::new(
                "--offline-export-no-url-rewriting", None, "offlineExportNoUrlRewriting", OptionType::Bool, false,
                "Disable all URL rewriting in exported HTML/CSS/JS. URLs remain exactly as in the original source. Useful for RAG indexing or other processing where original URLs must be preserved verbatim.",
                Some("false"), false, false, None,
            ),
            CrawlerOption::new(
                "--replace-content", None, "replaceContent", OptionType::ReplaceContent, true,
                "Replace HTML/JS/CSS content with `foo -> bar` or regexp in PREG format: `/card[0-9]/i -> card`",
                None, true, true, None,
            ),
            CrawlerOption::new(
                "--replace-query-string", None, "replaceQueryString", OptionType::ReplaceContent, true,
                "Instead of using a short hash instead of a query string in the filename, just replace some characters. You can use simple format 'foo -> bar' or regexp in PREG format, e.g. '/([a-z]+)=([^&]*)(&|$)/i -> $1__$2'",
                None, true, true, None,
            ),
            CrawlerOption::new(
                "--offline-export-lowercase", None, "offlineExportLowercase", OptionType::Bool, false,
                "Convert all filenames to lowercase for offline export. Useful for case-insensitive filesystems.",
                Some("false"), false, false, None,
            ),
            CrawlerOption::new(
                "--ignore-store-file-error", None, "ignoreStoreFileError", OptionType::Bool, false,
                "Ignores any file storing errors. The export process will continue.",
                Some("false"), false, false, None,
            ),
            CrawlerOption::new(
                "--disable-astro-inline-modules", None, "disableAstroInlineModules", OptionType::Bool, false,
                "Disables inlining of Astro module scripts for offline export. Scripts will remain as external files with corrected relative paths.",
                Some("false"), false, false, None,
            ),
        ],
    ));

    // -------------------------------------------------------------------------
    // Sitemap options (SitemapExporter)
    // -------------------------------------------------------------------------
    options.add_group(OptionGroup::new(
        GROUP_SITEMAP_SETTINGS,
        "Sitemap options",
        vec![
            CrawlerOption::new(
                "--sitemap-xml-file",
                None,
                "outputSitemapXml",
                OptionType::File,
                false,
                "Save sitemap to XML. `.xml` added if missing.",
                None,
                true,
                false,
                None,
            ),
            CrawlerOption::new(
                "--sitemap-txt-file",
                None,
                "outputSitemapTxt",
                OptionType::File,
                false,
                "Save sitemap to TXT. `.txt` added if missing.",
                None,
                true,
                false,
                None,
            ),
            CrawlerOption::new(
                "--sitemap-base-priority",
                None,
                "sitemapBasePriority",
                OptionType::Float,
                false,
                "Base priority for XML sitemap.",
                Some("0.5"),
                false,
                false,
                None,
            ),
            CrawlerOption::new(
                "--sitemap-priority-increase",
                None,
                "sitemapPriorityIncrease",
                OptionType::Float,
                false,
                "Priority increase value based on slashes count in the URL",
                Some("0.1"),
                false,
                false,
                None,
            ),
        ],
    ));

    // -------------------------------------------------------------------------
    // Upload options (UploadExporter)
    // -------------------------------------------------------------------------
    options.add_group(OptionGroup::new(
        GROUP_UPLOAD_SETTINGS,
        "Upload options",
        vec![
            CrawlerOption::new(
                "--upload", Some("-up"), "uploadEnabled", OptionType::Bool, false,
                "Enable HTML report upload to `--upload-to`.",
                Some("false"), false, false, None,
            ),
            CrawlerOption::new(
                "--upload-to", Some("-upt"), "uploadTo", OptionType::Url, false,
                "URL of the endpoint where to send the HTML report.",
                Some("https://crawler.siteone.io/up"), false, false, None,
            ),
            CrawlerOption::new(
                "--upload-retention", Some("-upr"), "uploadRetention", OptionType::String, false,
                "How long should the HTML report be kept in the online version? Values: 1h / 4h / 12h / 24h / 3d / 7d / 30d / 365d / forever",
                Some("30d"), false, false, None,
            ),
            CrawlerOption::new(
                "--upload-password", Some("-uppass"), "uploadPassword", OptionType::String, false,
                "Optional password, which must be entered (the user will be 'crawler') to display the online HTML report.",
                None, true, false, None,
            ),
            CrawlerOption::new(
                "--upload-timeout", Some("-upti"), "uploadTimeout", OptionType::Int, false,
                "Upload timeout in seconds.",
                Some("3600"), false, false, None,
            ),
        ],
    ));

    // -------------------------------------------------------------------------
    // Fastest URL analyzer (FastestAnalyzer)
    // -------------------------------------------------------------------------
    options.add_group(OptionGroup::new(
        GROUP_FASTEST_ANALYZER,
        "Fastest URL analyzer",
        vec![
            CrawlerOption::new(
                "--fastest-urls-top-limit",
                None,
                "fastestTopLimit",
                OptionType::Int,
                false,
                "Number of URL addresses in TOP fastest URL addresses.",
                Some("20"),
                false,
                false,
                None,
            ),
            CrawlerOption::new(
                "--fastest-urls-max-time",
                None,
                "fastestMaxTime",
                OptionType::Float,
                false,
                "The maximum response time for an URL address to be evaluated as fast.",
                Some("1"),
                false,
                false,
                None,
            ),
        ],
    ));

    // -------------------------------------------------------------------------
    // SEO and OpenGraph analyzer (SeoAndOpenGraphAnalyzer)
    // -------------------------------------------------------------------------
    options.add_group(OptionGroup::new(
        GROUP_SEO_AND_OPENGRAPH_ANALYZER,
        "SEO and OpenGraph analyzer",
        vec![CrawlerOption::new(
            "--max-heading-level",
            None,
            "maxHeadingLevel",
            OptionType::Int,
            false,
            "Maximal analyzer heading level from 1 to 6.",
            Some("3"),
            false,
            false,
            Some(vec!["1".to_string(), "6".to_string()]),
        )],
    ));

    // -------------------------------------------------------------------------
    // Slowest URL analyzer (SlowestAnalyzer)
    // -------------------------------------------------------------------------
    options.add_group(OptionGroup::new(
        GROUP_SLOWEST_ANALYZER,
        "Slowest URL analyzer",
        vec![
            CrawlerOption::new(
                "--slowest-urls-top-limit",
                None,
                "slowestTopLimit",
                OptionType::Int,
                false,
                "Number of URL addresses in TOP slowest URL addresses.",
                Some("20"),
                false,
                false,
                None,
            ),
            CrawlerOption::new(
                "--slowest-urls-min-time",
                None,
                "slowestMinTime",
                OptionType::Float,
                false,
                "The minimum response time for an URL address to be added to TOP slow selection.",
                Some("0.01"),
                false,
                false,
                None,
            ),
            CrawlerOption::new(
                "--slowest-urls-max-time",
                None,
                "slowestMaxTime",
                OptionType::Float,
                false,
                "The maximum response time for an URL address to be evaluated as very slow.",
                Some("3"),
                false,
                false,
                None,
            ),
        ],
    ));

    // -------------------------------------------------------------------------
    // CI/CD settings
    // -------------------------------------------------------------------------
    options.add_group(OptionGroup::new(
        GROUP_CI_CD_SETTINGS,
        "CI/CD settings",
        vec![
            CrawlerOption::new(
                "--ci",
                None,
                "ci",
                OptionType::Bool,
                false,
                "Enable CI/CD quality gate. Crawler exits with code 10 if thresholds are not met.",
                Some("false"),
                false,
                false,
                None,
            ),
            CrawlerOption::new(
                "--ci-min-score",
                None,
                "ciMinScore",
                OptionType::Float,
                false,
                "Minimum overall quality score (0.0-10.0).",
                Some("5.0"),
                false,
                false,
                Some(vec!["0.0".into(), "10.0".into()]),
            ),
            CrawlerOption::new(
                "--ci-min-performance",
                None,
                "ciMinPerformance",
                OptionType::Float,
                false,
                "Minimum Performance category score (0.0-10.0). Default value is `5`.",
                Some("5"),
                true,
                false,
                Some(vec!["0.0".into(), "10.0".into()]),
            ),
            CrawlerOption::new(
                "--ci-min-seo",
                None,
                "ciMinSeo",
                OptionType::Float,
                false,
                "Minimum SEO category score (0.0-10.0). Default value is `5`.",
                Some("5"),
                true,
                false,
                Some(vec!["0.0".into(), "10.0".into()]),
            ),
            CrawlerOption::new(
                "--ci-min-security",
                None,
                "ciMinSecurity",
                OptionType::Float,
                false,
                "Minimum Security category score (0.0-10.0). Default value is `5`.",
                Some("5"),
                true,
                false,
                Some(vec!["0.0".into(), "10.0".into()]),
            ),
            CrawlerOption::new(
                "--ci-min-accessibility",
                None,
                "ciMinAccessibility",
                OptionType::Float,
                false,
                "Minimum Accessibility category score (0.0-10.0). Default value is `3`.",
                Some("3"),
                true,
                false,
                Some(vec!["0.0".into(), "10.0".into()]),
            ),
            CrawlerOption::new(
                "--ci-min-best-practices",
                None,
                "ciMinBestPractices",
                OptionType::Float,
                false,
                "Minimum Best Practices category score (0.0-10.0). Default value is `5`.",
                Some("5"),
                true,
                false,
                Some(vec!["0.0".into(), "10.0".into()]),
            ),
            CrawlerOption::new(
                "--ci-max-404",
                None,
                "ciMax404",
                OptionType::Int,
                false,
                "Maximum number of 404 responses allowed.",
                Some("0"),
                false,
                false,
                None,
            ),
            CrawlerOption::new(
                "--ci-max-5xx",
                None,
                "ciMax5xx",
                OptionType::Int,
                false,
                "Maximum number of 5xx server error responses allowed.",
                Some("0"),
                false,
                false,
                None,
            ),
            CrawlerOption::new(
                "--ci-max-criticals",
                None,
                "ciMaxCriticals",
                OptionType::Int,
                false,
                "Maximum number of critical analysis findings allowed.",
                Some("0"),
                false,
                false,
                None,
            ),
            CrawlerOption::new(
                "--ci-max-warnings",
                None,
                "ciMaxWarnings",
                OptionType::Int,
                false,
                "Maximum number of warning analysis findings allowed.",
                None,
                true,
                false,
                None,
            ),
            CrawlerOption::new(
                "--ci-max-avg-response",
                None,
                "ciMaxAvgResponse",
                OptionType::Float,
                false,
                "Maximum average response time in seconds.",
                None,
                true,
                false,
                None,
            ),
            CrawlerOption::new(
                "--ci-min-pages",
                None,
                "ciMinPages",
                OptionType::Int,
                false,
                "Minimum number of HTML pages that must be found.",
                Some("10"),
                false,
                false,
                None,
            ),
            CrawlerOption::new(
                "--ci-min-assets",
                None,
                "ciMinAssets",
                OptionType::Int,
                false,
                "Minimum number of assets (JS, CSS, images, fonts) that must be found.",
                Some("10"),
                false,
                false,
                None,
            ),
            CrawlerOption::new(
                "--ci-min-documents",
                None,
                "ciMinDocuments",
                OptionType::Int,
                false,
                "Minimum number of documents (PDF, etc.) that must be found.",
                Some("0"),
                false,
                false,
                None,
            ),
            CrawlerOption::new(
                "--ci-baseline",
                None,
                "ciBaseline",
                OptionType::File,
                false,
                "Path to a previous JSON output used as a baseline for regression checks.",
                None,
                true,
                false,
                None,
            ),
            CrawlerOption::new(
                "--ci-max-score-drop",
                None,
                "ciMaxScoreDrop",
                OptionType::Float,
                false,
                "Maximum allowed drop of the overall score vs the --ci-baseline run. Default 0 (any drop fails).",
                None,
                true,
                false,
                None,
            ),
            CrawlerOption::new(
                "--ci-fail-on-code",
                None,
                "ciFailOnCode",
                OptionType::String,
                true,
                "Fail the build if a finding code (aplCode, e.g. seo-noindex-sitewide) is present. Can be specified multiple times.",
                None,
                true,
                true,
                None,
            ),
            CrawlerOption::new(
                "--ci-ignore-code",
                None,
                "ciIgnoreCode",
                OptionType::String,
                true,
                "Ignore a finding code (aplCode, e.g. pages-without-lang) when counting criticals/warnings (also suppresses --ci-fail-on-code). Can be specified multiple times.",
                None,
                true,
                true,
                None,
            ),
            CrawlerOption::new(
                "--ci-junit-file",
                None,
                "ciJunitFile",
                OptionType::File,
                false,
                "Write the CI gate result as a JUnit XML report to this file.",
                None,
                true,
                false,
                None,
            ),
            CrawlerOption::new(
                "--ci-github-annotations",
                None,
                "ciGithubAnnotations",
                OptionType::Bool,
                false,
                "Print GitHub Actions error annotations for failed CI checks.",
                Some("false"),
                false,
                false,
                None,
            ),
        ],
    ));

    // -------------------------------------------------------------------------
    // Server options (built-in HTTP server for serving exports)
    // -------------------------------------------------------------------------
    options.add_group(OptionGroup::new(
        GROUP_SERVER_SETTINGS,
        "Server options",
        vec![
            CrawlerOption::new(
                "--serve-markdown", Some("-sm"), "serveMarkdownDirectory", OptionType::Dir, false,
                "Start HTTP server to browse a markdown export directory. Renders .md files as styled HTML with table and accordion support. No crawling is performed.",
                None, true, false, None,
            ),
            CrawlerOption::new(
                "--serve-offline", Some("-so"), "serveOfflineDirectory", OptionType::Dir, false,
                "Start HTTP server to browse an offline HTML export directory. Serves files with Content-Security-Policy restricting to same origin. No crawling is performed.",
                None, true, false, None,
            ),
            CrawlerOption::new(
                "--serve-port", Some("-sport"), "servePort", OptionType::Int, false,
                "Port for the built-in HTTP server (used with --serve-markdown or --serve-offline).",
                Some("8321"), false, false, None,
            ),
            CrawlerOption::new(
                "--serve-bind-address", Some("-sba"), "serveBindAddress", OptionType::String, false,
                "Bind address for the built-in HTTP server. Default is 127.0.0.1 (localhost only). Use 0.0.0.0 to listen on all network interfaces.",
                Some("127.0.0.1"), false, false, None,
            ),
            CrawlerOption::new(
                "--html-to-markdown", Some("-htm"), "htmlToMarkdownFile", OptionType::String, false,
                "Convert a local HTML file to Markdown and print to stdout. Uses the same pipeline as --markdown-export-dir. Respects --markdown-disable-images, --markdown-disable-files, --markdown-move-content-before-h1-to-end, and --markdown-exclude-selector. No crawling is performed.",
                None, true, false, None,
            ),
            CrawlerOption::new(
                "--html-to-markdown-output", Some("-htmo"), "htmlToMarkdownOutput", OptionType::String, false,
                "Output file path for --html-to-markdown. If not set, markdown is printed to stdout.",
                None, true, false, None,
            ),
        ],
    ));

    // -------------------------------------------------------------------------
    // AI options (optional AI features — nothing runs unless --ai-* is set)
    // -------------------------------------------------------------------------
    options.add_group(OptionGroup::new(
        GROUP_AI_SETTINGS,
        "AI options",
        vec![
            CrawlerOption::new(
                "--ai-provider", None, "aiProvider", OptionType::String, false,
                "AI provider: `openai`, `anthropic`, `gemini`, or `openai-compatible` (vLLM/LiteLLM/MiniMax/self-hosted). Enables the optional AI features.",
                Some("openai-compatible"), false, false, None,
            ),
            CrawlerOption::new(
                "--ai-endpoint", None, "aiEndpoint", OptionType::Url, false,
                "Base API endpoint URL. Required for `openai-compatible`; optional override for the other providers.",
                None, true, false, None,
            ),
            CrawlerOption::new(
                "--ai-model", None, "aiModel", OptionType::String, false,
                "Model name to call, e.g. `MiniMax-M3`, `gpt-5-mini`, `claude-sonnet-4-6`, `gemini-2.5-pro`.",
                None, true, false, None,
            ),
            CrawlerOption::new(
                "--ai-api-key", None, "aiApiKey", OptionType::String, false,
                "API key (DISCOURAGED — leaks into `ps`/shell history/logs). Supports `env:VARNAME` indirection. Prefer the default conventional env var (OPENAI_API_KEY/ANTHROPIC_API_KEY/GEMINI_API_KEY) or --ai-api-key-file.",
                None, true, false, None,
            ),
            CrawlerOption::new(
                "--ai-api-key-env", None, "aiApiKeyEnv", OptionType::String, false,
                "Name of the environment variable to read the API key from.",
                None, true, false, None,
            ),
            CrawlerOption::new(
                "--ai-api-key-file", None, "aiApiKeyFile", OptionType::File, false,
                "Path to a file whose first line is the API key (safest for CI).",
                None, true, false, None,
            ),
            CrawlerOption::new(
                "--ai-max-tokens", None, "aiMaxTokens", OptionType::Int, false,
                "Max output tokens per request. Auto-mapped to max_completion_tokens for OpenAI reasoning models. Raise it further if you enable thinking/reasoning.",
                Some("32000"), false, false, Some(vec!["1".to_string(), "1000000".to_string()]),
            ),
            CrawlerOption::new(
                "--ai-use-max-completion-tokens", None, "aiUseMaxCompletionTokens", OptionType::Bool, false,
                "Force `max_completion_tokens` instead of `max_tokens` (for endpoints/models that require it). Otherwise auto-detected.",
                Some("false"), false, false, None,
            ),
            CrawlerOption::new(
                "--ai-temperature", None, "aiTemperature", OptionType::Float, false,
                "Sampling temperature (omitted automatically for OpenAI reasoning models).",
                Some("0.0"), false, false, Some(vec!["0".to_string(), "2".to_string()]),
            ),
            CrawlerOption::new(
                "--ai-extra-body", None, "aiExtraBody", OptionType::String, false,
                "JSON object deep-merged into the request body, overriding native fields. Use for thinking/reasoning control and any provider-specific knobs, e.g. '{\"chat_template_kwargs\":{\"enable_thinking\":false}}'.",
                None, true, false, None,
            ),
            CrawlerOption::new(
                "--ai-synthesis-extra-body", None, "aiSynthesisExtraBody", OptionType::String, false,
                "Like --ai-extra-body but applied ONLY to the final report-summary synthesis call (the `summary` action). Use to enable thinking/max reasoning just for the synthesis, e.g. '{}' to drop a global enable_thinking:false, or '{\"reasoning_effort\":\"high\"}'.",
                None, true, false, None,
            ),
            CrawlerOption::new(
                "--ai-actions", None, "aiActions", OptionType::String, true,
                "Comma-separated AI analyses to run: `seo`, `llms-txt`, `llms-full`, `typos`, `custom`, `summary`. The default runs the full report set; `custom` (needs a prompt) and `llms-txt`/`llms-full` (extra files) are opt-in.",
                Some("seo,typos,summary"), false, true, None,
            ),
            CrawlerOption::new(
                "--ai-prompt-file", None, "aiPromptFile", OptionType::File, false,
                "Path to a custom prompt file for the `custom` action. Supports placeholders like {{url}}, {{title}}, {{content_markdown}}.",
                None, true, false, None,
            ),
            CrawlerOption::new(
                "--ai-prompt", None, "aiPrompt", OptionType::String, false,
                "Inline custom prompt for the `custom` action (alternative to --ai-prompt-file).",
                None, true, false, None,
            ),
            CrawlerOption::new(
                "--ai-language", None, "aiLanguage", OptionType::String, false,
                "Force content language (BCP-47, e.g. `cs`, `de`) for the `typos` action. Auto-detected if unset.",
                None, true, false, None,
            ),
            CrawlerOption::new(
                "--ai-include", None, "aiInclude", OptionType::Regex, true,
                "Only run AI on URLs matching this regex (repeatable). Applied before ranking.",
                None, true, true, None,
            ),
            CrawlerOption::new(
                "--ai-exclude", None, "aiExclude", OptionType::Regex, true,
                "Skip AI on URLs matching this regex (repeatable, wins over --ai-include). E.g. exclude '/press/'.",
                None, true, true, None,
            ),
            CrawlerOption::new(
                "--ai-max-pages", None, "aiMaxPages", OptionType::Int, false,
                "Hard cap on the number of pages sent to the LLM (highest-ranked pages kept).",
                Some("100"), false, false, Some(vec!["1".to_string(), "100000".to_string()]),
            ),
            CrawlerOption::new(
                "--ai-max-concurrency", None, "aiMaxConcurrency", OptionType::Int, false,
                "Maximum concurrent AI requests.",
                Some("4"), false, false, Some(vec!["1".to_string(), "64".to_string()]),
            ),
            CrawlerOption::new(
                "--ai-max-reqs-per-sec", None, "aiMaxReqsPerSec", OptionType::Float, false,
                "Maximum AI requests per second (rate limit for the LLM API).",
                None, true, false, None,
            ),
            CrawlerOption::new(
                "--ai-timeout", None, "aiTimeout", OptionType::Int, false,
                "Per-request timeout for AI calls in seconds (raise it for slow reasoning models).",
                Some("180"), false, false, Some(vec!["1".to_string(), "3600".to_string()]),
            ),
            CrawlerOption::new(
                "--ai-cache-dir", None, "aiCacheDir", OptionType::Dir, false,
                "Directory for caching AI responses. Empty value disables caching.",
                Some("tmp/ai-cache"), true, false, None,
            ),
            CrawlerOption::new(
                "--ai-seo-affects-score", None, "aiSeoAffectsScore", OptionType::Bool, false,
                "Let the AI SEO assessment apply a small capped deduction to the SEO quality score (off = advisory only). Note: non-deterministic across runs.",
                Some("false"), false, false, None,
            ),
            CrawlerOption::new(
                "--ai-dry-run", None, "aiDryRun", OptionType::Bool, false,
                "Show which pages would be analyzed, the number of LLM calls, and an estimated input-token count, then exit without calling the API.",
                Some("false"), false, false, None,
            ),
        ],
    ));

    // -------------------------------------------------------------------------
    // Browser rendering options (optional — nothing runs unless --browser is set)
    // -------------------------------------------------------------------------
    options.add_group(OptionGroup::new(
        GROUP_BROWSER,
        "Browser rendering options",
        vec![
            CrawlerOption::new(
                "--browser", None, "browserEnabled", OptionType::Bool, false,
                "Render each page in a real Chromium browser (CDP) instead of a direct HTTP request. Enables crawling JS-rendered/SPA sites. Requires a browser-enabled build and a Chromium-family browser (auto-detected, or --browser-path).",
                Some("false"), false, false, None,
            ),
            CrawlerOption::new(
                "--browser-path", None, "browserPath", OptionType::String, false,
                "Explicit path to a Chromium/Chrome/Edge/Brave executable. Skips auto-detection and download.",
                None, true, false, None,
            ),
            CrawlerOption::new(
                "--browser-headful", None, "browserHeadful", OptionType::Bool, false,
                "Show a visible browser window (default is headless). Concurrency is kept low so the windows are watchable.",
                Some("false"), false, false, None,
            ),
            CrawlerOption::new(
                "--browser-workers", None, "browserWorkers", OptionType::Int, false,
                "Max concurrently rendered pages (separate from --workers; browser pages are heavier).",
                Some("3"), false, false, Some(vec!["1".to_string(), "32".to_string()]),
            ),
            CrawlerOption::new(
                "--browser-wait", None, "browserWait", OptionType::String, false,
                "Page-ready wait strategy: `load`, `domcontentloaded`, or `networkidle` (near-idle network).",
                Some("networkidle"), false, false, None,
            ),
            CrawlerOption::new(
                "--browser-wait-extra", None, "browserWaitExtraMs", OptionType::Int, false,
                "Extra settle delay in milliseconds after the wait condition is met.",
                Some("0"), false, false, Some(vec!["0".to_string(), "60000".to_string()]),
            ),
            CrawlerOption::new(
                "--browser-timeout", None, "browserTimeout", OptionType::Int, false,
                "Hard navigation+render timeout per page, in seconds. On timeout, whatever rendered is captured.",
                Some("30"), false, false, Some(vec!["1".to_string(), "600".to_string()]),
            ),
            CrawlerOption::new(
                "--browser-render-all", None, "browserRenderAll", OptionType::Bool, false,
                "Render every URL in the browser. By default only HTML documents are rendered; assets (images/CSS/JS/fonts) are fetched via HTTP.",
                Some("false"), false, false, None,
            ),
            CrawlerOption::new(
                "--browser-auto-download", None, "browserAutoDownload", OptionType::Bool, false,
                "Pre-consent to downloading chrome-headless-shell when no browser is found (for non-interactive/CI runs; interactive runs prompt instead).",
                Some("false"), false, false, None,
            ),
            CrawlerOption::new(
                "--screenshots", None, "screenshots", OptionType::Bool, false,
                "Capture a screenshot of every rendered page (requires --browser).",
                Some("false"), false, false, None,
            ),
            CrawlerOption::new(
                "--screenshots-dir", None, "screenshotsDir", OptionType::String, false,
                "Directory to save screenshots into. Defaults to `tmp/screenshots/`.",
                None, true, false, None,
            ),
            CrawlerOption::new(
                "--screenshot-mode", None, "screenshotMode", OptionType::String, false,
                "Screenshot mode: `viewport` (visible area at the set resolution) or `full-page` (entire scroll height).",
                Some("viewport"), false, false, None,
            ),
            CrawlerOption::new(
                "--screenshot-viewport", None, "screenshotViewport", OptionType::String, false,
                "Viewport size `WxH` used for rendering and viewport screenshots.",
                Some("1920x1080"), false, false, None,
            ),
            CrawlerOption::new(
                "--screenshot-format", None, "screenshotFormat", OptionType::String, false,
                "Screenshot image format: `png`, `jpg`, or `webp`.",
                Some("png"), false, false, None,
            ),
            CrawlerOption::new(
                "--screenshot-quality", None, "screenshotQuality", OptionType::Int, false,
                "Image quality (1-100) for `jpg`/`webp` screenshots.",
                Some("80"), false, false, Some(vec!["1".to_string(), "100".to_string()]),
            ),
            CrawlerOption::new(
                "--screenshots-animation", None, "screenshotsAnimation", OptionType::String, false,
                "Build an animation from the page screenshots: comma-separated `gif`,`mp4` (requires --browser --screenshots).",
                None, true, false, None,
            ),
            CrawlerOption::new(
                "--screenshots-animation-frame-duration", None, "screenshotsAnimationFrameDuration", OptionType::Float, false,
                "Seconds each page is shown in the animation (0.2-10, default 2).",
                Some("2"), false, false, None,
            ),
            CrawlerOption::new(
                "--screenshots-animation-width", None, "screenshotsAnimationWidth", OptionType::Int, false,
                "Animation width in px (default 1024); height is derived from the --screenshot-viewport aspect ratio.",
                Some("1024"), false, false, None,
            ),
            CrawlerOption::new(
                "--ffmpeg-path", None, "ffmpegPath", OptionType::String, false,
                "Path to the ffmpeg binary (auto-detected from PATH if omitted). Required for MP4 output.",
                None, true, false, None,
            ),
            CrawlerOption::new(
                "--screenshot-hide-cookie-banners", None, "screenshotHideCookieBanners", OptionType::Bool, false,
                "Before each screenshot, try to dismiss/hide cookie consent banners (best-effort; requires --browser --screenshots).",
                Some("false"), false, false, None,
            ),
            CrawlerOption::new(
                "--screenshot-hide-selector", None, "screenshotHideSelector", OptionType::String, false,
                "Comma-separated CSS selectors to hide before each screenshot (e.g. a site-specific cookie banner).",
                None, true, false, None,
            ),
            CrawlerOption::new(
                "--browser-no-sandbox", None, "browserNoSandbox", OptionType::Bool, false,
                "Launch Chromium with --no-sandbox. Often required in Docker/CI/WSL or when running as root, but it weakens the renderer's security isolation against untrusted pages.",
                Some("false"), false, false, None,
            ),
            CrawlerOption::new(
                "--console-max-messages", None, "consoleMaxMessages", OptionType::Int, false,
                "Max console/diagnostic messages per page kept for the AI payload.",
                Some("100"), false, false, Some(vec!["1".to_string(), "100000".to_string()]),
            ),
            CrawlerOption::new(
                "--console-msg-max-chars", None, "consoleMsgMaxChars", OptionType::Int, false,
                "Truncate each console/diagnostic message to this many characters in the AI payload.",
                Some("200"), false, false, Some(vec!["1".to_string(), "100000".to_string()]),
            ),
            CrawlerOption::new(
                "--console-total-max-kb", None, "consoleTotalMaxKb", OptionType::Int, false,
                "Total size cap (in KB) of the per-page console diagnostics AI payload.",
                Some("128"), false, false, Some(vec!["1".to_string(), "10000".to_string()]),
            ),
        ],
    ));

    options
}

/// Parse CLI arguments (raw argv) into a fully populated CoreOptions.
/// Read config file and return its lines as CLI-style arguments.
/// Config file format: one argument per line, `#` for comments, blank lines ignored.
/// Example:
///   --workers=5
///   --max-reqs-per-sec=20
///   # This is a comment
///   --output=json
fn read_config_file(path: &str) -> Result<Vec<String>, CrawlerError> {
    let content = std::fs::read_to_string(path)
        .map_err(|e| CrawlerError::Config(format!("Cannot read config file '{}': {}", path, e)))?;
    Ok(parse_line_list(&content))
}

/// Whether `s` is an absolute http(s) URL — the only scheme the crawler fetches.
fn is_http_url(s: &str) -> bool {
    let l = s.trim_start().to_ascii_lowercase();
    l.starts_with("http://") || l.starts_with("https://")
}

/// Parse a newline-delimited list (config files, --url-list): trim each line,
/// drop blank lines and `#` comments. A leading UTF-8 BOM is stripped first —
/// it is not whitespace, so `trim()` would leave it on the first entry and
/// corrupt it (common in files saved on Windows).
fn parse_line_list(content: &str) -> Vec<String> {
    content
        .strip_prefix('\u{feff}')
        .unwrap_or(content)
        .lines()
        .map(|line| line.trim())
        .filter(|line| !line.is_empty() && !line.starts_with('#'))
        .map(|line| line.to_string())
        .collect()
}

/// Load config from file: --config-file=PATH, ~/.siteone-crawler.conf, or /etc/siteone-crawler.conf.
/// Returns merged argv with config args prepended (CLI args take precedence).
fn merge_config_file_args(argv: &[String]) -> Result<Vec<String>, CrawlerError> {
    // Extract --config-file from argv
    let mut config_path: Option<String> = None;
    for arg in argv {
        if let Some(path) = arg.strip_prefix("--config-file=") {
            config_path = Some(path.to_string());
            break;
        }
    }

    // If no explicit config file, try auto-discovery
    if config_path.is_none() {
        let home_conf = std::env::var("HOME")
            .ok()
            .map(|h| format!("{}/.siteone-crawler.conf", h));
        let candidates = [home_conf, Some("/etc/siteone-crawler.conf".to_string())];
        for candidate in candidates.iter().flatten() {
            if std::path::Path::new(candidate).exists() {
                config_path = Some(candidate.clone());
                break;
            }
        }
    }

    if let Some(ref path) = config_path {
        let config_args = read_config_file(path)?;
        // Merge: config args first, then real argv (CLI overrides config)
        // Filter out --config-file from real argv
        let real_args: Vec<String> = argv
            .iter()
            .filter(|a| !a.starts_with("--config-file="))
            .cloned()
            .collect();
        let mut merged = Vec::new();
        if !real_args.is_empty() {
            merged.push(real_args[0].clone()); // binary name
        }
        merged.extend(config_args);
        if real_args.len() > 1 {
            merged.extend_from_slice(&real_args[1..]);
        }
        Ok(merged)
    } else {
        Ok(argv.to_vec())
    }
}

/// This is the main entry point for option parsing.
pub fn parse_argv(argv: &[String]) -> Result<CoreOptions, CrawlerError> {
    // Merge config file args with CLI args (CLI takes precedence)
    let merged_argv = merge_config_file_args(argv)?;
    let argv = &merged_argv;

    let mut options = get_options();

    // Collect all known option names and alt names for unknown detection
    let mut known_options: Vec<String> = Vec::new();
    let mut bool_options: std::collections::HashSet<String> = std::collections::HashSet::new();
    for (_apl_code, group) in options.get_groups() {
        for (_prop_name, option) in &group.options {
            known_options.push(option.name.clone());
            if matches!(option.option_type, OptionType::Bool) {
                bool_options.insert(option.name.clone());
            }
            if let Some(ref alt) = option.alt_name {
                known_options.push(alt.clone());
                if matches!(option.option_type, OptionType::Bool) {
                    bool_options.insert(alt.clone());
                }
            }
        }
    }
    // Also accept --config-file as known
    known_options.push("--config-file".to_string());

    // Check for unknown options
    let mut unknown_options: Vec<String> = Vec::new();
    let mut i = 0;
    while i < argv.len() {
        let arg = argv[i].trim();
        if arg.is_empty() || arg.starts_with('#') {
            i += 1;
            continue;
        }
        // Skip the program name (first argv element or any non-option arg)
        if !arg.starts_with('-') {
            // Check if this is a value consumed by a previous space-separated option
            // (non-option args that aren't the script name are potentially unknown)
            if i > 0 {
                // Check if previous arg was a known option that could consume this as value
                let prev = &argv[i - 1];
                let prev_name = prev.split('=').next().unwrap_or(prev);
                let is_prev_known_non_bool = known_options.iter().any(|k| k == prev_name) && !prev.contains('=');
                if !is_prev_known_non_bool {
                    // Not a consumed value — could be unknown, but skip argv[0] (binary name)
                    // We just skip non-dash args silently (they might be the binary path)
                }
            }
            i += 1;
            continue;
        }
        // Extract option name without value (strip =...)
        let arg_without_value = if let Some(eq_pos) = arg.find('=') {
            &arg[..eq_pos]
        } else {
            arg
        };
        if !known_options.iter().any(|k| k == arg_without_value) {
            unknown_options.push(arg.to_string());
        } else if !arg.contains('=') && !bool_options.contains(arg_without_value) {
            // Known non-bool option without '=' — the next token is its value, skip it
            i += 1;
        }
        i += 1;
    }
    if !unknown_options.is_empty() {
        return Err(CrawlerError::Config(format!(
            "Unknown options: {}",
            unknown_options.join(", ")
        )));
    }

    // Parse all options from argv
    for (_apl_code, group) in options.get_groups_mut() {
        for (_prop_name, option) in group.options.iter_mut() {
            option.set_value_from_argv(argv)?;

            // Set domain for use in file/dir %domain% placeholder
            if option.property_to_fill == "url"
                && let Ok(value) = option.get_value()
                && let Some(url_str) = value.as_str()
                && let Ok(parsed) = url::Url::parse(url_str)
            {
                CrawlerOption::set_extras_domain(parsed.host_str());
            }
        }
    }

    CoreOptions::from_options(&options)
}

/// Generate help text for all options, organized by groups.
pub fn get_help_text() -> String {
    use crate::options::option_type::OptionType;
    use crate::utils;

    let options = get_options();
    let mut help = String::new();

    for (_apl_code, group) in options.get_groups() {
        let group_label = format!("{}:", group.name);
        let dashes = "-".repeat(group_label.len());
        help.push_str(&format!(
            "{}\n{}\n",
            utils::get_color_text(&group_label, "yellow", false),
            utils::get_color_text(&dashes, "yellow", false),
        ));

        for (_prop_name, option) in &group.options {
            // Build option name with type suffix
            let type_suffix = match option.option_type {
                OptionType::Int => "=<int>",
                OptionType::String | OptionType::Float | OptionType::ReplaceContent => "=<val>",
                OptionType::SizeMG => "=<size>",
                OptionType::Regex => "=<regex>",
                OptionType::Email => "=<email>",
                OptionType::Url => "=<url>",
                OptionType::File => "=<file>",
                OptionType::Dir => "=<dir>",
                OptionType::HostAndPort => "=<host:port>",
                OptionType::Resolve => "=<domain:port:ip>",
                OptionType::Bool => "",
            };
            let name_and_value = format!("{}{}", option.name, type_suffix);

            // Description: trim trailing '. ' then append '.'
            let desc = option.description.trim_end_matches(['.', ' ']);
            let desc_with_period = format!("{}.", desc);

            // Default value display logic:
            // Bool options with default false don't show a default.
            // Bool options with default true show as "1".
            let default_info = match option.default_value {
                Some(ref dv) if !dv.is_empty() && !desc_with_period.contains("Default") => {
                    if option.option_type == OptionType::Bool {
                        // true displays as "1", false is not shown
                        if dv == "true" || dv == "1" {
                            " Default value is `1`.".to_string()
                        } else {
                            String::new()
                        }
                    } else {
                        format!(" Default value is `{}`.", dv)
                    }
                }
                _ => String::new(),
            };

            // Ensure at least one space between name+type and description
            let padded = if name_and_value.len() >= 33 {
                format!("{} ", name_and_value)
            } else {
                format!("{:<33}", name_and_value)
            };

            help.push_str(&format!("{}{}{}\n", padded, desc_with_period, default_info));
        }

        help.push('\n');
    }

    help
}

/// Parse a human-readable duration string (e.g. "24h", "7d", "30m", "3600s", "3600") to seconds.
fn parse_duration_to_secs(s: &str) -> u64 {
    let s = s.trim();
    if let Some(num) = s.strip_suffix('d') {
        num.parse::<u64>().unwrap_or(1) * 86400
    } else if let Some(num) = s.strip_suffix('h') {
        num.parse::<u64>().unwrap_or(1) * 3600
    } else if let Some(num) = s.strip_suffix('m') {
        num.parse::<u64>().unwrap_or(1) * 60
    } else if let Some(num) = s.strip_suffix('s') {
        num.parse::<u64>().unwrap_or(0)
    } else {
        // Plain number = seconds
        s.parse::<u64>().unwrap_or(86400)
    }
}

/// Returns the platform-appropriate default HTTP cache directory.
/// Uses dirs::cache_dir() for XDG/macOS/Windows compliance:
///   Linux:   ~/.cache/siteone-crawler/http-cache
///   macOS:   ~/Library/Caches/siteone-crawler/http-cache
///   Windows: C:\Users\<user>\AppData\Local\siteone-crawler\http-cache
/// Falls back to "tmp/http-client-cache" if system cache dir is unavailable.
fn default_http_cache_dir() -> String {
    dirs::cache_dir()
        .map(|p| {
            p.join("siteone-crawler")
                .join("http-cache")
                .to_string_lossy()
                .to_string()
        })
        .unwrap_or_else(|| "tmp/http-client-cache".to_string())
}

/// Returns the default output directory prefix for reports and result storage.
/// Tries `./tmp/` in CWD first; if it can't be created (e.g. read-only filesystem),
/// falls back to `dirs::data_local_dir()/siteone-crawler/` (platform-appropriate).
/// Result is cached via OnceLock so the notice is printed at most once.
fn default_output_prefix() -> String {
    static PREFIX: std::sync::OnceLock<String> = std::sync::OnceLock::new();
    PREFIX
        .get_or_init(|| {
            let tmp_path = std::path::Path::new("tmp");
            if tmp_path.is_dir() || std::fs::create_dir_all(tmp_path).is_ok() {
                return "tmp".to_string();
            }
            if let Some(data_dir) = dirs::data_local_dir() {
                let fallback = data_dir.join("siteone-crawler");
                if fallback.is_dir() || std::fs::create_dir_all(&fallback).is_ok() {
                    let path = fallback.to_string_lossy().to_string();
                    eprintln!(
                        "Notice: Cannot create ./tmp/ in current directory. Output files will be stored in: {}",
                        path
                    );
                    return path;
                }
            }
            // Last resort — use tmp and let it fail later with a clear error
            "tmp".to_string()
        })
        .clone()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::options::option::OptionValue;

    fn make_default_core_options() -> CoreOptions {
        CoreOptions {
            url: "https://test.com".to_string(),
            url_list: None,
            url_list_urls: Vec::new(),
            single_page: false,
            max_depth: 0,
            device: DeviceType::Desktop,
            user_agent: None,
            timeout: 5,
            proxy: None,
            http_auth: None,
            accept_invalid_certs: false,
            timezone: None,
            show_version_only: false,
            show_help_only: false,
            output_type: OutputType::Text,
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
            ignore_html_comments: false,
            allowed_domains_for_external_files: Vec::new(),
            allowed_domains_for_crawling: Vec::new(),
            single_foreign_page: false,
            result_storage: StorageType::Memory,
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
            offline_export_preserve_urls: false,
            offline_export_no_url_rewriting: false,
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
            html_to_markdown_file: None,
            html_to_markdown_output: None,
            ci: false,
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
            ci_min_pages: 10,
            ci_min_assets: 10,
            ci_min_documents: 0,
            ci_baseline: None,
            ci_max_score_drop: None,
            ci_fail_on_code: Vec::new(),
            ci_ignore_code: Vec::new(),
            ci_junit_file: None,
            ci_github_annotations: false,

            // ai settings
            ai_enabled: false,
            ai_provider: "openai-compatible".to_string(),
            ai_endpoint: None,
            ai_model: None,
            ai_api_key: None,
            ai_api_key_env: None,
            ai_api_key_file: None,
            ai_max_tokens: 32000,
            ai_use_max_completion_tokens: false,
            ai_temperature: 0.0,
            ai_extra_body: None,
            ai_synthesis_extra_body: None,
            ai_actions: vec!["seo".to_string(), "typos".to_string(), "summary".to_string()],
            ai_prompt_file: None,
            ai_prompt: None,
            ai_language: None,
            ai_include: Vec::new(),
            ai_exclude: Vec::new(),
            ai_max_pages: 100,
            ai_max_concurrency: 4,
            ai_max_reqs_per_sec: None,
            ai_timeout: 180,
            ai_cache_dir: Some("tmp/ai-cache".to_string()),
            ai_seo_affects_score: false,
            ai_dry_run: false,

            // browser rendering settings
            browser_enabled: false,
            browser_path: None,
            browser_headful: false,
            browser_workers: 3,
            browser_wait: "networkidle".to_string(),
            browser_wait_extra_ms: 0,
            browser_timeout: 30,
            browser_render_all: false,
            browser_auto_download: false,

            // screenshot settings
            screenshots: false,
            screenshots_dir: None,
            screenshot_mode: "viewport".to_string(),
            screenshot_viewport: "1920x1080".to_string(),
            screenshot_format: "png".to_string(),
            screenshot_quality: 80,
            screenshot_hide_cookie_banners: false,
            screenshot_hide_selector: None,
            screenshots_animation: String::new(),
            screenshots_animation_frame_duration: 2.0,
            screenshots_animation_width: 1024,
            ffmpeg_path: None,
            browser_no_sandbox: false,
            console_max_messages: 100,
            console_msg_max_chars: 200,
            console_total_max_kb: 128,
        }
    }

    #[test]
    fn ci_defaults() {
        let opts = make_default_core_options();
        assert!(!opts.ci);
        assert_eq!(opts.ci_min_score, 5.0);
        assert_eq!(opts.ci_max_404, 0);
        assert_eq!(opts.ci_max_5xx, 0);
        assert_eq!(opts.ci_max_criticals, 0);
    }

    #[test]
    fn apply_ci_bool() {
        let mut opts = make_default_core_options();
        opts.apply_option_value("ci", &OptionValue::Bool(true)).unwrap();
        assert!(opts.ci);
    }

    #[test]
    fn apply_url_list_string() {
        let mut opts = make_default_core_options();
        opts.apply_option_value("urlList", &OptionValue::Str("urls.txt".into()))
            .unwrap();
        assert_eq!(opts.url_list, Some("urls.txt".to_string()));
    }

    #[test]
    fn apply_ci_min_score() {
        let mut opts = make_default_core_options();
        opts.apply_option_value("ciMinScore", &OptionValue::Float(7.5)).unwrap();
        assert_eq!(opts.ci_min_score, 7.5);
    }

    #[test]
    fn apply_ci_max_404() {
        let mut opts = make_default_core_options();
        opts.apply_option_value("ciMax404", &OptionValue::Int(5)).unwrap();
        assert_eq!(opts.ci_max_404, 5);
    }

    #[test]
    fn apply_ci_max_warnings() {
        let mut opts = make_default_core_options();
        opts.apply_option_value("ciMaxWarnings", &OptionValue::Int(10)).unwrap();
        assert_eq!(opts.ci_max_warnings, Some(10));
    }

    #[test]
    fn apply_ci_max_avg_response() {
        let mut opts = make_default_core_options();
        opts.apply_option_value("ciMaxAvgResponse", &OptionValue::Float(2.0))
            .unwrap();
        assert_eq!(opts.ci_max_avg_response, Some(2.0));
    }

    #[test]
    fn apply_unknown_key_no_error() {
        let mut opts = make_default_core_options();
        let result = opts.apply_option_value("nonExistent", &OptionValue::Bool(true));
        assert!(result.is_ok());
    }

    #[test]
    fn ci_option_group_exists() {
        let options = get_options();
        let group = options.get_group(GROUP_CI_CD_SETTINGS);
        assert!(group.is_some());
        let group = group.unwrap();
        assert_eq!(group.options.len(), 21);
    }

    // ---- Duration parsing tests ----

    #[test]
    fn parse_duration_days() {
        assert_eq!(parse_duration_to_secs("7d"), 7 * 86400);
    }

    #[test]
    fn parse_duration_hours() {
        assert_eq!(parse_duration_to_secs("24h"), 24 * 3600);
    }

    #[test]
    fn parse_duration_minutes() {
        assert_eq!(parse_duration_to_secs("30m"), 30 * 60);
    }

    #[test]
    fn parse_duration_seconds() {
        assert_eq!(parse_duration_to_secs("3600s"), 3600);
        assert_eq!(parse_duration_to_secs("3600"), 3600);
    }

    #[test]
    fn parse_duration_invalid_number() {
        // "abcd" suffix 'd' → parse "abc" fails → fallback to 1 day
        assert_eq!(parse_duration_to_secs("abcd"), 86400);
    }

    // ---- Config file parsing tests ----

    #[test]
    fn read_config_file_parses_args() {
        let dir = std::env::temp_dir();
        let path = dir.join("test_crawler_config_1.conf");
        std::fs::write(&path, "--workers=5\n--max-reqs-per-sec=20\n").unwrap();
        let args = read_config_file(path.to_str().unwrap()).unwrap();
        assert_eq!(args, vec!["--workers=5", "--max-reqs-per-sec=20"]);
        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn read_config_file_ignores_comments_and_blank_lines() {
        let dir = std::env::temp_dir();
        let path = dir.join("test_crawler_config_2.conf");
        std::fs::write(&path, "# comment\n\n--workers=3\n  # another comment\n  \n--debug\n").unwrap();
        let args = read_config_file(path.to_str().unwrap()).unwrap();
        assert_eq!(args, vec!["--workers=3", "--debug"]);
        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn read_config_file_nonexistent_returns_error() {
        let result = read_config_file("/nonexistent/path/config.conf");
        assert!(result.is_err());
    }

    #[test]
    fn parse_line_list_strips_leading_bom() {
        // A UTF-8 BOM at the start must not leak into the first entry.
        let content = "\u{feff}https://example.com/\nhttps://example.com/about\n";
        let urls = parse_line_list(content);
        assert_eq!(urls, vec!["https://example.com/", "https://example.com/about"]);
    }

    #[test]
    fn parse_line_list_trims_and_filters() {
        let content = "# comment\n\n  https://a.test/  \n  # indented comment\n  \nhttps://b.test/\n";
        let urls = parse_line_list(content);
        assert_eq!(urls, vec!["https://a.test/", "https://b.test/"]);
    }

    #[test]
    fn is_http_url_accepts_only_absolute_http_urls() {
        assert!(is_http_url("http://example.com/"));
        assert!(is_http_url("https://example.com/a"));
        assert!(is_http_url("HTTPS://EXAMPLE.COM"));
        assert!(!is_http_url("example.com/x"));
        assert!(!is_http_url("/path"));
        assert!(!is_http_url("ftp://example.com"));
        assert!(!is_http_url("notaurl"));
        assert!(!is_http_url(""));
    }

    #[test]
    fn merge_config_file_args_with_explicit_config() {
        let dir = std::env::temp_dir();
        let path = dir.join("test_crawler_config_3.conf");
        std::fs::write(&path, "--workers=5\n--debug\n").unwrap();
        let argv = vec![
            "siteone-crawler".to_string(),
            format!("--config-file={}", path.display()),
            "--url=https://example.com".to_string(),
        ];
        let merged = merge_config_file_args(&argv).unwrap();
        // Config args prepended after binary name, CLI args follow
        assert_eq!(merged[0], "siteone-crawler");
        assert!(merged.contains(&"--workers=5".to_string()));
        assert!(merged.contains(&"--debug".to_string()));
        assert!(merged.contains(&"--url=https://example.com".to_string()));
        // --config-file itself should be filtered out
        assert!(!merged.iter().any(|a| a.starts_with("--config-file=")));
        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn merge_config_file_args_without_config() {
        let argv = vec!["siteone-crawler".to_string(), "--url=https://example.com".to_string()];
        let merged = merge_config_file_args(&argv).unwrap();
        // No config file exists, so argv is returned as-is
        assert_eq!(merged, argv);
    }

    // ---- New option apply tests for recent features ----

    #[test]
    fn apply_force_relative_urls() {
        let mut opts = make_default_core_options();
        assert!(!opts.force_relative_urls);
        opts.apply_option_value("forceRelativeUrls", &OptionValue::Bool(true))
            .unwrap();
        assert!(opts.force_relative_urls);
    }

    #[test]
    fn apply_offline_export_preserve_url_structure() {
        let mut opts = make_default_core_options();
        assert!(!opts.offline_export_preserve_url_structure);
        opts.apply_option_value("offlineExportPreserveUrlStructure", &OptionValue::Bool(true))
            .unwrap();
        assert!(opts.offline_export_preserve_url_structure);
    }

    #[test]
    fn apply_offline_export_preserve_urls() {
        let mut opts = make_default_core_options();
        assert!(!opts.offline_export_preserve_urls);
        opts.apply_option_value("offlineExportPreserveUrls", &OptionValue::Bool(true))
            .unwrap();
        assert!(opts.offline_export_preserve_urls);
    }

    #[test]
    fn apply_offline_export_no_url_rewriting() {
        let mut opts = make_default_core_options();
        assert!(!opts.offline_export_no_url_rewriting);
        opts.apply_option_value("offlineExportNoUrlRewriting", &OptionValue::Bool(true))
            .unwrap();
        assert!(opts.offline_export_no_url_rewriting);
    }

    #[test]
    fn apply_ignore_html_comments() {
        let mut opts = make_default_core_options();
        assert!(!opts.ignore_html_comments);
        opts.apply_option_value("ignoreHtmlComments", &OptionValue::Bool(true))
            .unwrap();
        assert!(opts.ignore_html_comments);
    }

    #[test]
    fn test_apply_screenshots_animation_options() {
        let mut opts = make_default_core_options();
        opts.apply_option_value("screenshotsAnimation", &OptionValue::Str("gif,mp4".into()))
            .unwrap();
        opts.apply_option_value("screenshotsAnimationFrameDuration", &OptionValue::Float(3.5))
            .unwrap();
        opts.apply_option_value("screenshotsAnimationWidth", &OptionValue::Int(800))
            .unwrap();
        opts.apply_option_value("ffmpegPath", &OptionValue::Str("/usr/bin/ffmpeg".into()))
            .unwrap();
        assert_eq!(opts.screenshots_animation, "gif,mp4");
        assert_eq!(opts.screenshots_animation_frame_duration, 3.5);
        assert_eq!(opts.screenshots_animation_width, 800);
        assert_eq!(opts.ffmpeg_path.as_deref(), Some("/usr/bin/ffmpeg"));
    }
}
