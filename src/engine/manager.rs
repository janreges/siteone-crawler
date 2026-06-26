// SiteOne Crawler - Manager
// (c) Jan Reges <jan.reges@siteone.cz>
//
// Orchestrates the crawler: initializes all components, runs the crawl,
// then runs post-crawl analysis and exporters.

use std::sync::Arc;
use std::time::Instant;

use crate::analysis::manager::AnalysisManager;
use crate::components::super_table::SuperTable;
use crate::content_processor::astro_processor::AstroProcessor;
use crate::content_processor::base_processor::{DomainAllowFn, ProcessorConfig};
use crate::content_processor::css_processor::CssProcessor;
use crate::content_processor::html_processor::HtmlProcessor;
use crate::content_processor::javascript_processor::JavaScriptProcessor;
use crate::content_processor::manager::{ContentProcessorManager, SUPER_TABLE_CONTENT_PROCESSORS_STATS};
use crate::content_processor::nextjs_processor::NextJsProcessor;
use crate::content_processor::svelte_processor::SvelteProcessor;
use crate::content_processor::xml_processor::XmlProcessor;
use crate::engine::crawler::{Crawler, compile_domain_patterns};
use crate::engine::http_client::HttpClient;
use crate::engine::parsed_url::ParsedUrl;
use crate::error::{CrawlerError, CrawlerResult};
use crate::export::exporter::Exporter;
use crate::export::file_exporter::FileExporter;
use crate::export::html_report::report::HtmlReport;
use crate::export::mailer_exporter::MailerExporter;
use crate::export::markdown_exporter::MarkdownExporter;
use crate::export::offline_website_exporter::OfflineWebsiteExporter;
use crate::export::sitemap_exporter::SitemapExporter;
use crate::export::upload_exporter::UploadExporter;
use crate::info::Info;
use crate::options::core_options::{CoreOptions, StorageType};
use crate::output::json_output::JsonOutput;
use crate::output::multi_output::MultiOutput;
use crate::output::output::{CrawlerInfo, Output};
use crate::output::text_output::TextOutput;
use crate::result::status::Status;
use crate::result::storage::file_storage::FileStorage;
use crate::result::storage::memory_storage::MemoryStorage;
use crate::scoring::ci_gate;
use crate::scoring::scorer;
use crate::types::OutputType;
use crate::utils;
use crate::version;

pub struct Manager {
    options: Arc<CoreOptions>,
    analysis_manager: Option<AnalysisManager>,
    start_time: Instant,
}

impl Manager {
    pub fn new(options: CoreOptions, analysis_manager: AnalysisManager) -> CrawlerResult<Self> {
        let start_time = Instant::now();

        // Apply color settings
        if options.no_color {
            utils::disable_colors();
        } else if options.force_color {
            utils::force_enabled_colors();
        }

        // Apply forced console width if specified
        if let Some(width) = options.console_width
            && width > 0
        {
            utils::set_forced_console_width(width as usize);
        }

        // Apply hard rows limit for analysis tables
        SuperTable::set_hard_rows_limit(options.rows_limit as usize);

        Ok(Self {
            options: Arc::new(options),
            analysis_manager: Some(analysis_manager),
            start_time,
        })
    }

    /// Run the complete crawl process: init, crawl, analyze, export, summarize.
    /// Returns an exit code: 0 = success, 10 = CI gate failed.
    pub async fn run(&mut self) -> CrawlerResult<i32> {
        let options = self.options.clone();

        // Build crawler info. Reconstruct a copy-pasteable command: drop the binary's path and
        // shell-quote arguments (e.g. JSON in --ai-extra-body) so it can be pasted as-is.
        let command = utils::format_command_from_argv(&std::env::args().collect::<Vec<_>>());
        let hostname = gethostname::gethostname().to_string_lossy().to_string();

        // Build the final user agent the same way Crawler does
        let final_user_agent = {
            let base = if let Some(ref ua) = options.user_agent {
                ua.clone()
            } else {
                match options.device {
                    crate::types::DeviceType::Desktop => format!(
                        "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/{}.0.0.0 Safari/537.36",
                        chrono::Utc::now().format("%y")
                    ),
                    crate::types::DeviceType::Mobile => "Mozilla/5.0 (iPhone; CPU iPhone OS 15_0 like Mac OS X) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/15.0 Mobile/15A5370a Safari/604.1".to_string(),
                    crate::types::DeviceType::Tablet => "Mozilla/5.0 (Linux; Android 11; SAMSUNG SM-T875) AppleWebKit/537.36 (KHTML, like Gecko) SamsungBrowser/14.0 Chrome/87.0.4280.141 Safari/537.36".to_string(),
                }
            };
            if base.ends_with('!') {
                base.trim_end_matches('!').trim_end().to_string()
            } else {
                format!("{} SiteOne-Crawler/{}", base, version::CODE)
            }
        };

        let crawler_info = Info::new(
            "SiteOne Crawler".to_string(),
            version::CODE.to_string(),
            chrono::Utc::now().format("%Y-%m-%d %H:%M:%S").to_string(),
            utils::get_safe_command(&command),
            hostname,
            final_user_agent,
            options.url.clone(),
        );

        // Create storage
        let origin_url = ParsedUrl::parse(&options.url, None);
        let origin_url_domain = format!(
            "{}{}",
            origin_url.host.as_deref().unwrap_or(""),
            origin_url.port.map(|p| format!("-{}", p)).unwrap_or_default()
        );

        let storage: Box<dyn crate::result::storage::storage::Storage> = match options.result_storage {
            StorageType::Memory => Box::new(MemoryStorage::new(options.result_storage_compression)),
            StorageType::File => {
                let result_storage_dir = crate::utils::get_absolute_path(&options.result_storage_dir);
                Box::new(FileStorage::new(
                    &result_storage_dir,
                    options.result_storage_compression,
                    &origin_url_domain,
                )?)
            }
        };

        // Create status
        let status = Status::new(storage, true, crawler_info.clone(), self.start_time);

        // Create output
        let output = self.create_output(&options, &crawler_info)?;

        // Create HTTP client
        let http_cache_dir =
            if options.http_cache_dir.as_deref() == Some("off") || options.http_cache_dir.as_deref() == Some("") {
                None
            } else {
                options
                    .http_cache_dir
                    .as_ref()
                    .map(|dir| crate::utils::get_absolute_path(dir))
            };

        let http_client = HttpClient::new(
            options.proxy.clone(),
            options.http_auth.clone(),
            http_cache_dir,
            options.http_cache_compression,
            options.http_cache_ttl,
            options.accept_invalid_certs,
        );

        // Select the fetcher. Direct HTTP is the default path (unchanged behavior).
        // With the `browser` feature and `--browser`, render pages in a real Chromium instead.
        // (The `--browser` without the feature is rejected earlier in `CoreOptions::from_options`.)
        let fetcher: std::sync::Arc<dyn crate::engine::fetcher::Fetcher> = {
            #[cfg(feature = "browser")]
            {
                if options.browser_enabled {
                    if options.http_auth.is_some() {
                        eprintln!(
                            "⚠️  --browser: HTTP auth is applied to the status/headers fetch but NOT to the browser navigation; the rendered page may be the unauthenticated version."
                        );
                    }
                    // Browser renders live; give it a cache-free inner client so the metadata
                    // (status/headers) matches the live rendered DOM rather than a cached snapshot.
                    let browser_http = HttpClient::new(
                        options.proxy.clone(),
                        options.http_auth.clone(),
                        None,
                        false,
                        None,
                        options.accept_invalid_certs,
                    );
                    std::sync::Arc::new(crate::browser::BrowserRenderer::new(options.clone(), browser_http).await?)
                } else {
                    std::sync::Arc::new(http_client)
                }
            }
            #[cfg(not(feature = "browser"))]
            {
                std::sync::Arc::new(http_client)
            }
        };
        // Keep a handle so the browser can be shut down cleanly after the crawl.
        let fetcher_handle = fetcher.clone();

        // Create content processor manager and register processors
        let content_processor_manager = Self::create_content_processor_manager(&options);

        // Take the analysis_manager out of self (it will live inside the Crawler)
        let analysis_manager = self
            .analysis_manager
            .take()
            .ok_or_else(|| CrawlerError::Config("AnalysisManager already consumed".to_string()))?;

        // Create crawler
        let mut crawler = Crawler::new(
            options.clone(),
            fetcher,
            content_processor_manager,
            analysis_manager,
            output,
            status,
        );

        // Set extra columns from analyzers (for Access., Best pr. columns in progress table)
        if let (Ok(am), Ok(mut out)) = (crawler.get_analysis_manager().lock(), crawler.get_output().lock()) {
            let extra_cols = am.get_extra_columns();
            out.set_extra_columns_from_analysis(extra_cols);
        }

        // Print banner
        if let Ok(mut out) = crawler.get_output().lock() {
            out.add_banner();
        }

        // Fetch initial robots.txt
        let initial_scheme = options.get_initial_scheme();
        let initial_host = options.get_initial_host(false);
        let initial_port = ParsedUrl::parse(&options.url, None)
            .port
            .unwrap_or(if initial_scheme == "https" { 443 } else { 80 });
        crawler
            .fetch_robots_txt(&initial_host, initial_port, &initial_scheme)
            .await;

        // Run the crawler
        let run_result = crawler.run().await;

        // Shut down the browser on EVERY exit path (no-op for the direct-HTTP fetcher) so a
        // crawl error never leaks the Chromium process / handler task. No fetches occur after this.
        fetcher_handle.shutdown().await;

        run_result?;

        // Optional AI phase (post-crawl, before analyzers/exporters). Fail-soft.
        if options.ai_enabled {
            crate::ai::runner::run_ai(options.as_ref(), crawler.get_status(), crawler.get_output()).await;
        }

        // Post-crawl: run analyzers
        let exit_code = self.run_post_crawl(&crawler).await;

        Ok(exit_code)
    }

    /// Run post-crawl analysis and produce final output.
    /// Returns exit code: 0 = success, 3 = no pages crawled, 10 = CI gate failed.
    async fn run_post_crawl(&mut self, crawler: &Crawler) -> i32 {
        let status = crawler.get_status();
        let output = crawler.get_output();
        let analysis_manager = crawler.get_analysis_manager();

        // Transfer skipped URLs from crawler to status
        {
            let skipped = crawler.get_skipped();
            if let Ok(mut st) = status.lock() {
                for entry in skipped.iter() {
                    st.add_skipped_url(
                        entry.url.clone(),
                        entry.reason,
                        entry.source_uq_id.clone(),
                        entry.source_attr,
                    );
                }
            }
        }

        // Run post-crawl analyzers
        if let (Ok(mut am), Ok(st), Ok(mut out)) = (analysis_manager.lock(), status.lock(), output.lock()) {
            am.run_analyzers(&st, &mut **out);
        }

        // Add content processor stats
        if let Ok(cpm) = crawler.get_content_processor_manager().lock() {
            let mut super_table = cpm.get_stats().get_super_table(
                SUPER_TABLE_CONTENT_PROCESSORS_STATS,
                "Content processor stats",
                "No content processors found.",
                None,
                None,
            );

            if let Ok(st) = status.lock() {
                st.configure_super_table_url_stripping(&mut super_table);
            }
            if let Ok(mut out) = output.lock() {
                out.add_super_table(&super_table);
            }
            if let Ok(st) = status.lock() {
                st.add_super_table_at_end(super_table);
            }
        }

        // Optional AI executive summary (after analyzers populate findings/tables, before the
        // HTML report is generated so the AI box can be embedded in the Summary tab).
        if self.options.ai_enabled && self.options.ai_actions.iter().any(|a| a == "summary") {
            crate::ai::summary::run(self.options.as_ref(), status, output).await;
        }

        // Record total LLM usage (all per-page actions + the summary) as summary items so they
        // appear in text, JSON, and the HTML report: one headline line (with the model name),
        // then a per-analysis-type breakdown of requests + input/output tokens.
        if self.options.ai_enabled
            && let Some(usage_line) = crate::ai::usage::summary_line()
            && let Ok(st) = status.lock()
        {
            st.add_info_to_summary("ai-usage", &usage_line);
            for line in crate::ai::usage::breakdown_lines() {
                st.add_info_to_summary("ai-usage-by-type", &line);
            }
        }

        // Run exporters
        self.run_exporters(crawler);

        // Print used options
        if let Ok(mut out) = output.lock() {
            out.add_used_options();
        }

        // Print total stats
        if let Ok(st) = status.lock() {
            let basic_stats = st.get_basic_stats();
            let output_stats = crate::output::output::BasicStats {
                total_urls: basic_stats.total_urls,
                total_size: basic_stats.total_size,
                total_size_formatted: basic_stats.total_size_formatted.clone(),
                total_execution_time: basic_stats.total_execution_time,
                total_requests_times: basic_stats.total_requests_times,
                total_requests_times_avg: basic_stats.total_requests_times_avg,
                total_requests_times_min: basic_stats.total_requests_times_min,
                total_requests_times_max: basic_stats.total_requests_times_max,
                total_requests_times_p90: basic_stats.total_requests_times_p90,
                count_by_status: basic_stats.count_by_status.clone(),
                count_by_content_type: basic_stats.count_by_content_type.clone(),
            };
            if let Ok(mut out) = output.lock() {
                out.add_total_stats(&output_stats);
            }
        }

        // Calculate and print quality scores, then CI gate, then summary
        let mut ci_exit_code = 0i32;
        if let Ok(st) = status.lock() {
            let mut summary = st.get_summary();
            let basic_stats = st.get_basic_stats();
            let output_stats = crate::output::output::BasicStats {
                total_urls: basic_stats.total_urls,
                total_size: basic_stats.total_size,
                total_size_formatted: basic_stats.total_size_formatted.clone(),
                total_execution_time: basic_stats.total_execution_time,
                total_requests_times: basic_stats.total_requests_times,
                total_requests_times_avg: basic_stats.total_requests_times_avg,
                total_requests_times_min: basic_stats.total_requests_times_min,
                total_requests_times_max: basic_stats.total_requests_times_max,
                total_requests_times_p90: basic_stats.total_requests_times_p90,
                count_by_status: basic_stats.count_by_status.clone(),
                count_by_content_type: basic_stats.count_by_content_type.clone(),
            };
            let quality_scores = scorer::calculate_scores(&summary, &output_stats);
            if let Ok(mut out) = output.lock() {
                out.add_quality_scores(&quality_scores);
            }

            // CI/CD quality gate evaluation
            if self.options.ci {
                let ci_result = ci_gate::evaluate(&self.options, &quality_scores, &output_stats, &summary);
                ci_exit_code = ci_result.exit_code;
                if let Ok(mut out) = output.lock() {
                    out.add_ci_gate_result(&ci_result);
                }

                // Machine-readable CI artifacts
                if let Some(junit_path) = &self.options.ci_junit_file {
                    let xml = ci_gate::to_junit_xml(&ci_result);
                    if let Err(e) = std::fs::write(junit_path, xml) {
                        eprintln!("Failed to write JUnit report to {}: {}", junit_path, e);
                    }
                }
                let in_github_actions = std::env::var("GITHUB_ACTIONS").map(|v| v == "true").unwrap_or(false);
                if self.options.ci_github_annotations || in_github_actions {
                    for line in ci_gate::github_annotations(&ci_result) {
                        println!("{}", line);
                    }
                }
            }

            if let Ok(mut out) = output.lock() {
                out.add_summary(&mut summary);
            }
        }

        // Check if no pages were successfully crawled (e.g. initial URL failed with timeout, DNS error, etc.)
        // URLs with negative status codes (-1 connection error, -2 timeout, etc.) are counted in
        // total_urls but don't represent successful responses, so we check for any positive status code.
        let no_pages_crawled = match status.lock() {
            Ok(st) => {
                let stats = st.get_basic_stats();
                !stats.count_by_status.keys().any(|&code| code > 0)
            }
            _ => false,
        };

        // Finalize output
        if let Ok(mut out) = output.lock() {
            out.end();
        }

        // Save text/JSON report files after output is finalized (includes quality scores,
        // CI gate result, and summary that were missing when run_exporters captured content)
        if self.options.output_text_file.is_some() || self.options.output_json_file.is_some() {
            let initial_host = Some(self.options.get_initial_host(false));
            let mut file_exporter = FileExporter::new(
                None,
                None,
                self.options.output_json_file.clone(),
                self.options.output_text_file.clone(),
                self.options.add_timestamp_to_output_file,
                self.options.add_host_to_output_file,
                initial_host,
            );
            if let Ok(out) = output.lock() {
                if let Some(text) = out.get_output_text() {
                    file_exporter.set_text_output_content(text);
                }
                if let Some(json) = out.get_json_content() {
                    file_exporter.set_json_output_content(json);
                }
            }
            if let Ok(st) = status.lock()
                && let Ok(out) = output.lock()
                && let Err(e) = file_exporter.export(&st, &**out)
            {
                eprintln!("Error saving text/JSON report files: {}", e);
            }
        }

        if ci_exit_code != 0 {
            ci_exit_code
        } else if no_pages_crawled {
            3
        } else {
            0
        }
    }

    /// Run all activated exporters after crawling and analysis.
    fn run_exporters(&self, crawler: &Crawler) {
        let status = crawler.get_status();
        let output = crawler.get_output();
        let options = &self.options;

        // Generate HTML report content if any exporter needs it
        let html_report_needed =
            options.output_html_report.is_some() || !options.mail_to.is_empty() || options.upload_enabled;

        let html_report_content = if html_report_needed {
            match status.lock() {
                Ok(st) => {
                    let report = HtmlReport::new(&st, 5, options.html_report_options.as_deref());
                    Some(report.get_html())
                }
                _ => None,
            }
        } else {
            None
        };

        // Build list of activated exporters (excluding offline/markdown which run separately)
        let mut exporters: Vec<Box<dyn Exporter>> = Vec::new();

        // 1. SitemapExporter
        {
            let sitemap = SitemapExporter::new(
                options.sitemap_xml_file.clone(),
                options.sitemap_txt_file.clone(),
                options.sitemap_base_priority,
                options.sitemap_priority_increase,
            );
            if sitemap.should_be_activated() {
                exporters.push(Box::new(sitemap));
            }
        }

        // 2. OfflineWebsiteExporter — run separately to collect exported file paths
        let offline_paths = {
            let mut offline = OfflineWebsiteExporter::new();
            offline.set_offline_export_directory(options.offline_export_dir.clone());
            offline.set_offline_export_store_only_url_regex(options.offline_export_store_only_url_regex.clone());
            offline.set_offline_export_remove_unwanted_code(options.offline_export_remove_unwanted_code);
            offline.set_offline_export_no_auto_redirect_html(options.offline_export_no_auto_redirect_html);
            offline.set_offline_export_preserve_url_structure(options.offline_export_preserve_url_structure);
            offline.set_offline_export_lowercase(options.offline_export_lowercase);
            offline.set_ignore_store_file_error(options.ignore_store_file_error);
            offline.set_replace_content(options.replace_content.clone());
            offline.set_replace_query_string(options.replace_query_string.clone());
            let initial_parsed = ParsedUrl::parse(&options.url, None);
            offline.set_initial_parsed_url(initial_parsed);
            offline.set_content_processor_manager(crawler.get_content_processor_manager().clone());
            let (static_cb, crawling_cb) = Self::build_domain_allow_callbacks(options);
            offline.set_domain_callbacks(static_cb, crawling_cb);
            if offline.should_be_activated() {
                if let (Ok(st), Ok(out)) = (status.lock(), output.lock())
                    && let Err(e) = offline.export(&st, &**out)
                {
                    st.add_critical_to_summary(offline.get_name(), &format!("{} error: {}", offline.get_name(), e));
                }
                let paths = offline.get_exported_file_paths().clone();
                if paths.is_empty() { None } else { Some(paths) }
            } else {
                None
            }
        };

        // 3. MarkdownExporter — run separately to collect exported file paths
        let markdown_paths = {
            let mut markdown = MarkdownExporter::new();
            markdown.set_markdown_export_directory(options.markdown_export_dir.clone());
            markdown.set_markdown_export_single_file(options.markdown_export_single_file.clone());
            markdown.set_markdown_move_content_before_h1_to_end(options.markdown_move_content_before_h1_to_end);
            markdown.set_markdown_disable_images(options.markdown_disable_images);
            markdown.set_markdown_disable_files(options.markdown_disable_files);
            markdown.set_markdown_remove_links_and_images_from_single_file(
                options.markdown_remove_links_and_images_from_single_file,
            );
            markdown.set_markdown_exclude_selector(options.markdown_exclude_selector.clone());
            markdown.set_markdown_replace_content(options.markdown_replace_content.clone());
            markdown.set_markdown_replace_query_string(options.markdown_replace_query_string.clone());
            markdown.set_markdown_export_store_only_url_regex(options.markdown_export_store_only_url_regex.clone());
            markdown.set_markdown_ignore_store_file_error(options.markdown_ignore_store_file_error);
            markdown.set_initial_parsed_url(ParsedUrl::parse(&options.url, None));
            markdown.set_ignore_regexes(options.ignore_regex.clone());
            markdown.set_initial_url(options.url.clone());
            markdown.set_content_processor_manager(crawler.get_content_processor_manager().clone());
            if markdown.should_be_activated() {
                if let (Ok(st), Ok(out)) = (status.lock(), output.lock())
                    && let Err(e) = markdown.export(&st, &**out)
                {
                    st.add_critical_to_summary(markdown.get_name(), &format!("{} error: {}", markdown.get_name(), e));
                }
                let paths = markdown.get_exported_file_paths().clone();
                if paths.is_empty() { None } else { Some(paths) }
            } else {
                None
            }
        };

        // Inject exported file paths into JSON output results
        if (offline_paths.is_some() || markdown_paths.is_some())
            && let Ok(mut out) = output.lock()
        {
            out.set_export_file_paths(offline_paths.as_ref(), markdown_paths.as_ref());
        }

        // 4. FileExporter for HTML report only (text/JSON files are saved later in
        //    run_post_crawl after quality scores and summary have been added to output)
        {
            let initial_host = Some(options.get_initial_host(false));
            let mut file_exporter = FileExporter::new(
                options.output_html_report.clone(),
                options.html_report_options.clone(),
                None,
                None,
                options.add_timestamp_to_output_file,
                options.add_host_to_output_file,
                initial_host,
            );
            if let Some(ref content) = html_report_content {
                file_exporter.set_html_report_content(content.clone());
            }
            if file_exporter.should_be_activated() {
                exporters.push(Box::new(file_exporter));
            }
        }

        // 5. MailerExporter
        {
            let initial_host = Some(options.get_initial_host(false));
            let mut mailer = MailerExporter::new(
                options.mail_to.clone(),
                options.mail_from.clone(),
                options.mail_from_name.clone(),
                options.mail_smtp_host.clone(),
                options.mail_smtp_port.clamp(1, 65535) as u16,
                options.mail_smtp_user.clone(),
                options.mail_smtp_pass.clone(),
                options.mail_subject_template.clone(),
                initial_host,
            );
            if let Some(ref content) = html_report_content {
                mailer.set_html_report_content(content.clone());
            }
            if mailer.should_be_activated() {
                exporters.push(Box::new(mailer));
            }
        }

        // 6. UploadExporter
        {
            let mut upload = UploadExporter::new(
                options.upload_enabled,
                options.upload_to.clone(),
                Some(options.upload_retention.clone()),
                options.upload_password.clone(),
                options.upload_timeout as u64,
            );
            if let Some(ref content) = html_report_content {
                upload.set_html_report_content(content.clone());
            }
            if upload.should_be_activated() {
                exporters.push(Box::new(upload));
            }
        }

        // Run remaining activated exporters (sitemap, file, mailer, upload)
        for exporter in &mut exporters {
            if let (Ok(st), Ok(out)) = (status.lock(), output.lock())
                && let Err(e) = exporter.export(&st, &**out)
            {
                st.add_critical_to_summary(exporter.get_name(), &format!("{} error: {}", exporter.get_name(), e));
            }
        }
    }

    /// Create output based on options
    fn create_output(&self, options: &CoreOptions, crawler_info: &Info) -> CrawlerResult<Box<dyn Output>> {
        let output_crawler_info = CrawlerInfo {
            name: crawler_info.name.clone(),
            version: crawler_info.version.clone(),
            executed_at: crawler_info.executed_at.clone(),
            command: crawler_info.command.clone(),
            hostname: crawler_info.hostname.clone(),
            final_user_agent: crawler_info.final_user_agent.clone(),
            url: options.url.clone(),
            device: options.device.as_str().to_string(),
            workers: options.workers as usize,
        };

        // Create MultiOutput with both TextOutput and JsonOutput when FileExporter is active.
        // TextOutput prints to stdout only when output_type == Text.
        // JsonOutput prints to stdout only when output_type == Json.
        // Both are always created so FileExporter can save both formats.
        let file_exporter_active = options.output_html_report.is_some()
            || options.output_json_file.is_some()
            || options.output_text_file.is_some();

        let need_text = options.output_type == OutputType::Text || file_exporter_active;
        let need_json = options.output_type == OutputType::Json
            || file_exporter_active
            || !options.mail_to.is_empty()
            || options.sitemap_xml_file.is_some()
            || options.sitemap_txt_file.is_some();

        let mut outputs: Vec<Box<dyn Output>> = Vec::new();

        if need_text {
            outputs.push(Box::new(TextOutput::new(
                output_crawler_info.clone(),
                options.extra_columns.clone(),
                options.hide_progress_bar,
                options.show_scheme_and_host,
                options.do_not_truncate_url,
                options.add_random_query_params,
                options.url_column_size.map(|s| s as usize),
                options.show_inline_criticals,
                options.show_inline_warnings,
                options.hide_columns.clone(),
                options.workers as usize,
                options.memory_limit.clone(),
                options.output_type == OutputType::Text, // print_to_output
                options.ci,                              // disable_animation
            )));
        }

        if need_json {
            let options_json = serde_json::to_value(options).ok().map(|mut v| {
                // Never expose an internal AI endpoint IP in the serialized options.
                if let Some(ep) = v.get_mut("aiEndpoint")
                    && let Some(s) = ep.as_str()
                {
                    *ep = serde_json::Value::String(utils::mask_ip_addresses(s));
                }
                v
            });
            outputs.push(Box::new(JsonOutput::new(
                output_crawler_info,
                options.extra_columns.clone(),
                options.hide_progress_bar,
                options.output_type == OutputType::Json, // print_to_output
                options_json,
            )));
        }

        if outputs.len() > 1 {
            let mut multi = MultiOutput::new();
            for out in outputs {
                multi.add_output(out);
            }
            Ok(Box::new(multi))
        } else {
            match outputs.into_iter().next() {
                Some(out) => Ok(out),
                _ => Err(CrawlerError::Config("Unknown output type".to_string())),
            }
        }
    }

    /// Build domain-allow predicates used by the offline exporter / content processors so that
    /// downloaded external assets are rewritten to their local copies (issue #101).
    ///
    /// Returns `(is_domain_allowed_for_static_files, is_external_domain_allowed_for_crawling)`.
    /// The closures mirror the crawler's own matching rules (wildcard `*` support, www/non-www
    /// equivalence, initial-host always allowed for crawling).
    fn build_domain_allow_callbacks(options: &CoreOptions) -> (DomainAllowFn, DomainAllowFn) {
        let initial_host = ParsedUrl::parse(&options.url, None).host.unwrap_or_default();

        let static_patterns = compile_domain_patterns(&options.allowed_domains_for_external_files);
        let static_cb: DomainAllowFn =
            Arc::new(move |domain: &str| static_patterns.iter().any(|re| re.is_match(domain)));

        let crawling_patterns = compile_domain_patterns(&options.allowed_domains_for_crawling);
        let crawling_initial_host = initial_host.clone();
        let crawling_cb: DomainAllowFn = Arc::new(move |domain: &str| {
            if domain == crawling_initial_host {
                return true;
            }
            // www/non-www equivalence (mirrors Crawler::hosts_are_www_equivalent)
            let a = domain.strip_prefix("www.").unwrap_or(domain);
            let b = crawling_initial_host
                .strip_prefix("www.")
                .unwrap_or(&crawling_initial_host);
            if a == b {
                return true;
            }
            crawling_patterns.iter().any(|re| re.is_match(domain))
        });

        (static_cb, crawling_cb)
    }

    /// Create and register all content processors
    fn create_content_processor_manager(options: &CoreOptions) -> ContentProcessorManager {
        let initial_url = ParsedUrl::parse(&options.url, None);
        let mut config = ProcessorConfig::new(initial_url);
        config.single_page = options.single_page;
        config.single_foreign_page = options.single_foreign_page;
        config.max_depth = options.max_depth;
        config.files_enabled = !options.disable_files;
        config.images_enabled = !options.disable_images;
        config.scripts_enabled = !options.disable_javascript;
        config.styles_enabled = !options.disable_styles;
        config.fonts_enabled = !options.disable_fonts;
        config.disable_javascript = options.disable_javascript;
        config.remove_all_anchor_listeners = options.remove_all_anchor_listeners;
        config.ignore_regex = options.ignore_regex.clone();
        config.disable_astro_inline_modules = options.disable_astro_inline_modules;
        config.offline_export_preserve_urls = options.offline_export_preserve_urls;
        config.offline_export_no_url_rewriting = options.offline_export_no_url_rewriting;
        config.ignore_html_comments = options.ignore_html_comments;
        let (static_cb, crawling_cb) = Self::build_domain_allow_callbacks(options);
        config.is_domain_allowed_for_static_files = Some(static_cb);
        config.is_external_domain_allowed_for_crawling = Some(crawling_cb);
        config.compile_ignore_regex();

        let mut cpm = ContentProcessorManager::new();

        // Register processors
        let _ = cpm.register_processor(Box::new(AstroProcessor::new(config.clone())));
        let _ = cpm.register_processor(Box::new(HtmlProcessor::new(config.clone())));
        let _ = cpm.register_processor(Box::new(JavaScriptProcessor::new(config.clone())));
        let _ = cpm.register_processor(Box::new(CssProcessor::new(config.clone())));
        let _ = cpm.register_processor(Box::new(XmlProcessor::new(config.clone())));
        let _ = cpm.register_processor(Box::new(NextJsProcessor::new(config.clone())));
        let _ = cpm.register_processor(Box::new(SvelteProcessor::new(config)));

        cpm
    }
}
