// SiteOne Crawler - Wizard preset definitions and state
// (c) Jan Reges <jan.reges@siteone.cz>

use std::fmt;

/// A wizard preset — predefined configuration for common use cases.
pub struct Preset {
    pub name: &'static str,
    pub description: &'static str,
    pub workers: u32,
    pub timeout: u32,
    pub max_reqs_per_sec: u32,
    pub max_visited_urls: u32,
    pub disable_javascript: bool,
    pub disable_styles: bool,
    pub disable_fonts: bool,
    pub disable_images: bool,
    pub disable_files: bool,
    pub single_page: bool,
    pub offline_export_dir: Option<&'static str>,
    pub markdown_export_dir: Option<&'static str>,
    pub sitemap_xml_file: Option<&'static str>,
    pub http_cache_enabled: bool,
    pub result_storage_file: bool,
    pub extra_columns: Option<&'static str>,
    pub ignore_robots_txt: bool,
    pub add_random_query_params: bool,
    pub allowed_domains_for_external_files: Option<&'static str>,
    pub hide_columns: Option<&'static str>,
}

impl fmt::Display for Preset {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:26} {}", self.name, self.description)
    }
}

pub const PRESETS: &[Preset] = &[
    // 1. Quick Audit — the most common starting point
    Preset {
        name: "Quick Audit",
        description: "Fast site health overview — crawls all pages and assets",
        workers: 5,
        timeout: 5,
        max_reqs_per_sec: 10,
        max_visited_urls: 10000,
        disable_javascript: false,
        disable_styles: false,
        disable_fonts: false,
        disable_images: false,
        disable_files: false,
        single_page: false,
        offline_export_dir: None,
        markdown_export_dir: None,
        sitemap_xml_file: None,
        http_cache_enabled: true,
        result_storage_file: false,
        extra_columns: Some("Title(20)"),
        ignore_robots_txt: false,
        add_random_query_params: false,
        allowed_domains_for_external_files: None,
        hide_columns: Some("cache"),
    },
    // 2. SEO Analysis — metadata, headings, OpenGraph
    Preset {
        name: "SEO Analysis",
        description: "Extract titles, descriptions, keywords, and OpenGraph tags",
        workers: 8,
        timeout: 5,
        max_reqs_per_sec: 20,
        max_visited_urls: 50000,
        disable_javascript: true,
        disable_styles: true,
        disable_fonts: true,
        disable_images: true,
        disable_files: true,
        single_page: false,
        offline_export_dir: None,
        markdown_export_dir: None,
        sitemap_xml_file: None,
        http_cache_enabled: true,
        result_storage_file: false,
        extra_columns: Some("Title(20),Description(20),H1=xpath://h1/text()(40)"),
        ignore_robots_txt: false,
        add_random_query_params: false,
        allowed_domains_for_external_files: None,
        hide_columns: Some("cache"),
    },
    // 3. Performance Test — realistic timing, no cache
    Preset {
        name: "Performance Test",
        description: "Measure response times with cache disabled — find bottlenecks",
        workers: 3,
        timeout: 10,
        max_reqs_per_sec: 5,
        max_visited_urls: 5000,
        disable_javascript: false,
        disable_styles: false,
        disable_fonts: false,
        disable_images: false,
        disable_files: false,
        single_page: false,
        offline_export_dir: None,
        markdown_export_dir: None,
        sitemap_xml_file: None,
        http_cache_enabled: false,
        result_storage_file: false,
        extra_columns: Some("Title(30),DOM"),
        ignore_robots_txt: false,
        add_random_query_params: false,
        allowed_domains_for_external_files: None,
        hide_columns: None,
    },
    // 4. Security Check — headers, SSL/TLS, CSP
    Preset {
        name: "Security Check",
        description: "Check SSL/TLS, security headers, and redirects site-wide",
        workers: 5,
        timeout: 5,
        max_reqs_per_sec: 15,
        max_visited_urls: 10000,
        disable_javascript: false,
        disable_styles: true,
        disable_fonts: true,
        disable_images: true,
        disable_files: true,
        single_page: false,
        offline_export_dir: None,
        markdown_export_dir: None,
        sitemap_xml_file: None,
        http_cache_enabled: true,
        result_storage_file: false,
        extra_columns: Some("Title(30)"),
        ignore_robots_txt: false,
        add_random_query_params: false,
        allowed_domains_for_external_files: None,
        hide_columns: Some("cache"),
    },
    // 5. Offline Clone — full site download
    Preset {
        name: "Offline Clone",
        description: "Download entire website with all assets for offline browsing",
        workers: 2,
        timeout: 5,
        max_reqs_per_sec: 8,
        max_visited_urls: 100000,
        disable_javascript: false,
        disable_styles: false,
        disable_fonts: false,
        disable_images: false,
        disable_files: false,
        single_page: false,
        offline_export_dir: Some("./tmp/offline-{domain}-{date}/"),
        markdown_export_dir: None,
        sitemap_xml_file: None,
        http_cache_enabled: false,
        result_storage_file: false,
        extra_columns: None,
        ignore_robots_txt: false,
        add_random_query_params: false,
        allowed_domains_for_external_files: Some("*"),
        hide_columns: Some("cache"),
    },
    // 6. Markdown Export — content for AI/docs
    Preset {
        name: "Markdown Export",
        description: "Convert pages to Markdown for AI models or documentation",
        workers: 3,
        timeout: 5,
        max_reqs_per_sec: 10,
        max_visited_urls: 20000,
        disable_javascript: true,
        disable_styles: true,
        disable_fonts: true,
        disable_images: false,
        disable_files: false,
        single_page: false,
        offline_export_dir: None,
        markdown_export_dir: Some("./tmp/markdown-{domain}-{date}/"),
        sitemap_xml_file: None,
        http_cache_enabled: true,
        result_storage_file: false,
        extra_columns: Some("Title(40)"),
        ignore_robots_txt: false,
        add_random_query_params: false,
        allowed_domains_for_external_files: None,
        hide_columns: Some("cache"),
    },
    // 7. Stress Test — high concurrency load testing with cache busting
    Preset {
        name: "Stress Test",
        description: "High-concurrency load test with cache-busting random params",
        workers: 20,
        timeout: 10,
        max_reqs_per_sec: 20,
        max_visited_urls: 10000,
        disable_javascript: true,
        disable_styles: true,
        disable_fonts: true,
        disable_images: true,
        disable_files: true,
        single_page: false,
        offline_export_dir: None,
        markdown_export_dir: None,
        sitemap_xml_file: None,
        http_cache_enabled: false,
        result_storage_file: false,
        extra_columns: Some("Title(30)"),
        ignore_robots_txt: true,
        add_random_query_params: true,
        allowed_domains_for_external_files: None,
        hide_columns: Some("cache"),
    },
    // 8. Single Page — deep dive on one URL
    Preset {
        name: "Single Page",
        description: "Deep analysis of a single URL — SEO, security, performance",
        workers: 1,
        timeout: 10,
        max_reqs_per_sec: 10,
        max_visited_urls: 1,
        disable_javascript: false,
        disable_styles: false,
        disable_fonts: false,
        disable_images: false,
        disable_files: false,
        single_page: true,
        offline_export_dir: None,
        markdown_export_dir: None,
        sitemap_xml_file: None,
        http_cache_enabled: true,
        result_storage_file: false,
        extra_columns: Some("Title(50),Description(50),Keywords(30),DOM"),
        ignore_robots_txt: false,
        add_random_query_params: false,
        allowed_domains_for_external_files: None,
        hide_columns: None,
    },
    // 9. Large Site Crawl — optimized for scale
    Preset {
        name: "Large Site Crawl",
        description: "High-throughput HTML-only crawl for large sites (100k+ pages)",
        workers: 10,
        timeout: 3,
        max_reqs_per_sec: 50,
        max_visited_urls: 0, // unlimited
        disable_javascript: true,
        disable_styles: true,
        disable_fonts: true,
        disable_images: true,
        disable_files: true,
        single_page: false,
        offline_export_dir: None,
        markdown_export_dir: None,
        sitemap_xml_file: Some("./sitemap.xml"),
        http_cache_enabled: true,
        result_storage_file: false,
        extra_columns: Some("Title(40)"),
        ignore_robots_txt: true,
        add_random_query_params: false,
        allowed_domains_for_external_files: None,
        hide_columns: Some("cache"),
    },
    // 10. Custom — power users
    Preset {
        name: "Custom",
        description: "Start from defaults and configure every option manually",
        workers: 3,
        timeout: 5,
        max_reqs_per_sec: 10,
        max_visited_urls: 10000,
        disable_javascript: false,
        disable_styles: false,
        disable_fonts: false,
        disable_images: false,
        disable_files: false,
        single_page: false,
        offline_export_dir: None,
        markdown_export_dir: None,
        sitemap_xml_file: None,
        http_cache_enabled: true,
        result_storage_file: false,
        extra_columns: None,
        ignore_robots_txt: false,
        add_random_query_params: false,
        allowed_domains_for_external_files: None,
        hide_columns: None,
    },
];

/// Mutable state collected by the wizard, built from a preset and optionally customized.
pub struct WizardState {
    pub preset_name: String,
    pub url: String,
    pub workers: u32,
    pub timeout: u32,
    pub max_reqs_per_sec: u32,
    pub max_visited_urls: u32,
    pub device: String,
    pub disable_javascript: bool,
    pub disable_styles: bool,
    pub disable_fonts: bool,
    pub disable_images: bool,
    pub disable_files: bool,
    pub single_page: bool,
    pub offline_export_dir: Option<String>,
    pub markdown_export_dir: Option<String>,
    pub sitemap_xml_file: Option<String>,
    pub http_cache_enabled: bool,
    pub result_storage_file: bool,
    pub ignore_robots_txt: bool,
    pub add_random_query_params: bool,
    pub allowed_domains_for_external_files: Option<String>,
    pub hide_columns: Option<String>,
    pub extra_columns: Option<String>,
    pub http_auth: Option<String>,
    pub proxy: Option<String>,
}

impl WizardState {
    pub fn from_preset(preset: &Preset) -> Self {
        WizardState {
            preset_name: preset.name.to_string(),
            url: String::new(),
            workers: preset.workers,
            timeout: preset.timeout,
            max_reqs_per_sec: preset.max_reqs_per_sec,
            max_visited_urls: preset.max_visited_urls,
            device: "desktop".to_string(),
            disable_javascript: preset.disable_javascript,
            disable_styles: preset.disable_styles,
            disable_fonts: preset.disable_fonts,
            disable_images: preset.disable_images,
            disable_files: preset.disable_files,
            single_page: preset.single_page,
            offline_export_dir: preset.offline_export_dir.map(String::from),
            markdown_export_dir: preset.markdown_export_dir.map(String::from),
            sitemap_xml_file: preset.sitemap_xml_file.map(String::from),
            http_cache_enabled: preset.http_cache_enabled,
            result_storage_file: preset.result_storage_file,
            ignore_robots_txt: preset.ignore_robots_txt,
            add_random_query_params: preset.add_random_query_params,
            allowed_domains_for_external_files: preset.allowed_domains_for_external_files.map(String::from),
            hide_columns: preset.hide_columns.map(String::from),
            extra_columns: preset.extra_columns.map(String::from),
            http_auth: None,
            proxy: None,
        }
    }

    /// Build synthetic argv from wizard state. Only includes flags that differ from
    /// siteone-crawler defaults so the generated command is minimal and readable.
    pub fn build_argv(&self) -> Vec<String> {
        let mut args = vec!["siteone-crawler".to_string(), format!("--url='{}'", self.url)];

        // Performance & limits (defaults: workers=3, timeout=5, rps=10, max-urls=10000)
        if self.workers != 3 {
            args.push(format!("--workers={}", self.workers));
        }
        if self.timeout != 5 {
            args.push(format!("--timeout={}", self.timeout));
        }
        if self.max_reqs_per_sec != 10 {
            args.push(format!("--max-reqs-per-sec={}", self.max_reqs_per_sec));
        }
        if self.max_visited_urls != 10000 {
            args.push(format!("--max-visited-urls={}", self.max_visited_urls));
        }

        // Device (default: desktop)
        if self.device != "desktop" {
            args.push(format!("--device='{}'", self.device));
        }

        // Scope
        if self.single_page {
            args.push("--single-page".to_string());
        }

        // Content filtering
        if self.disable_javascript {
            args.push("--disable-javascript".to_string());
        }
        if self.disable_styles {
            args.push("--disable-styles".to_string());
        }
        if self.disable_fonts {
            args.push("--disable-fonts".to_string());
        }
        if self.disable_images {
            args.push("--disable-images".to_string());
        }
        if self.disable_files {
            args.push("--disable-files".to_string());
        }

        // Generators / exports
        if let Some(ref dir) = self.offline_export_dir {
            args.push(format!("--offline-export-dir='{}'", dir));
        }
        if let Some(ref dir) = self.markdown_export_dir {
            args.push(format!("--markdown-export-dir='{}'", dir));
        }
        if let Some(ref file) = self.sitemap_xml_file {
            args.push(format!("--sitemap-xml-file='{}'", file));
        }

        // Caching (default: enabled)
        if !self.http_cache_enabled {
            args.push("--no-cache".to_string());
        }
        if self.result_storage_file {
            args.push("--result-storage='file'".to_string());
        }

        // Extra columns
        if let Some(ref cols) = self.extra_columns {
            args.push(format!("--extra-columns='{}'", cols));
        }

        // Advanced
        if self.ignore_robots_txt {
            args.push("--ignore-robots-txt".to_string());
        }
        if self.add_random_query_params {
            args.push("--add-random-query-params".to_string());
        }
        if let Some(ref domains) = self.allowed_domains_for_external_files {
            args.push(format!("--allowed-domain-for-external-files='{}'", domains));
        }
        if let Some(ref cols) = self.hide_columns {
            args.push(format!("--hide-columns='{}'", cols));
        }
        if let Some(ref auth) = self.http_auth {
            args.push(format!("--http-auth='{}'", auth));
        }
        if let Some(ref proxy) = self.proxy {
            args.push(format!("--proxy='{}'", proxy));
        }

        args
    }

    /// Format a human-readable summary of non-default content types.
    pub fn content_summary(&self) -> String {
        let mut types = vec!["HTML"];
        if !self.disable_javascript {
            types.push("JS");
        }
        if !self.disable_styles {
            types.push("CSS");
        }
        if !self.disable_fonts {
            types.push("Fonts");
        }
        if !self.disable_images {
            types.push("Images");
        }
        if !self.disable_files {
            types.push("Files");
        }
        types.join(", ")
    }
}

/// Replace `{domain}` and `{date}` placeholders in export directory paths.
/// Called after the URL is known.
pub fn resolve_export_path(template: &str, url: &str) -> String {
    let domain = url::Url::parse(url)
        .ok()
        .and_then(|u| u.host_str().map(String::from))
        .unwrap_or_else(|| "unknown".to_string());
    let date = chrono::Local::now().format("%Y%m%d").to_string();
    template.replace("{domain}", &domain).replace("{date}", &date)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn preset_count_is_10() {
        assert_eq!(PRESETS.len(), 10);
    }

    #[test]
    fn last_preset_is_custom() {
        assert_eq!(PRESETS[PRESETS.len() - 1].name, "Custom");
    }

    #[test]
    fn build_argv_contains_url() {
        let mut state = WizardState::from_preset(&PRESETS[0]);
        state.url = "https://example.com".to_string();
        let argv = state.build_argv();
        assert_eq!(argv[0], "siteone-crawler");
        assert_eq!(argv[1], "--url='https://example.com'");
    }

    #[test]
    fn build_argv_custom_is_minimal() {
        let mut state = WizardState::from_preset(&PRESETS[9]); // Custom
        state.url = "https://example.com".to_string();
        let argv = state.build_argv();
        // Custom preset uses all defaults, so only binary name + URL
        assert_eq!(argv.len(), 2);
    }

    #[test]
    fn build_argv_quick_audit() {
        let mut state = WizardState::from_preset(&PRESETS[0]);
        state.url = "https://example.com".to_string();
        let argv = state.build_argv();
        assert!(argv.contains(&"--workers=5".to_string()));
        assert!(argv.contains(&"--extra-columns='Title(20)'".to_string()));
    }

    #[test]
    fn build_argv_seo_disables_assets() {
        let mut state = WizardState::from_preset(&PRESETS[1]); // SEO
        state.url = "https://example.com".to_string();
        let argv = state.build_argv();
        assert!(argv.contains(&"--disable-javascript".to_string()));
        assert!(argv.contains(&"--disable-styles".to_string()));
        assert!(argv.contains(&"--disable-fonts".to_string()));
        assert!(argv.contains(&"--disable-images".to_string()));
        assert!(argv.contains(&"--disable-files".to_string()));
        assert!(argv.contains(&"--workers=8".to_string()));
        assert!(argv.contains(&"--max-reqs-per-sec=20".to_string()));
    }

    #[test]
    fn build_argv_seo_has_extra_columns() {
        let mut state = WizardState::from_preset(&PRESETS[1]); // SEO
        state.url = "https://example.com".to_string();
        let argv = state.build_argv();
        assert!(argv.contains(&"--extra-columns='Title(20),Description(20),H1=xpath://h1/text()(40)'".to_string()));
    }

    #[test]
    fn build_argv_performance_test() {
        let mut state = WizardState::from_preset(&PRESETS[2]);
        state.url = "https://example.com".to_string();
        let argv = state.build_argv();
        assert!(argv.contains(&"--timeout=10".to_string()));
        assert!(argv.contains(&"--max-reqs-per-sec=5".to_string()));
        assert!(argv.contains(&"--no-cache".to_string()));
        assert!(argv.contains(&"--max-visited-urls=5000".to_string()));
    }

    #[test]
    fn build_argv_security_check() {
        let mut state = WizardState::from_preset(&PRESETS[3]);
        state.url = "https://example.com".to_string();
        let argv = state.build_argv();
        assert!(argv.contains(&"--disable-styles".to_string()));
        assert!(argv.contains(&"--disable-fonts".to_string()));
        assert!(argv.contains(&"--disable-images".to_string()));
        assert!(!argv.contains(&"--disable-javascript".to_string())); // JS stays enabled
    }

    #[test]
    fn build_argv_offline_clone() {
        let mut state = WizardState::from_preset(&PRESETS[4]);
        state.url = "https://example.com".to_string();
        let argv = state.build_argv();
        assert!(argv.iter().any(|a| a.starts_with("--offline-export-dir=")));
        assert!(argv.contains(&"--no-cache".to_string()));
        assert!(argv.contains(&"--max-visited-urls=100000".to_string()));
        assert!(argv.contains(&"--workers=2".to_string()));
    }

    #[test]
    fn build_argv_markdown_export() {
        let mut state = WizardState::from_preset(&PRESETS[5]);
        state.url = "https://example.com".to_string();
        let argv = state.build_argv();
        assert!(argv.iter().any(|a| a.starts_with("--markdown-export-dir=")));
        assert!(argv.contains(&"--disable-javascript".to_string()));
        assert!(!argv.contains(&"--disable-images".to_string())); // images stay enabled
        assert!(argv.contains(&"--max-visited-urls=20000".to_string()));
    }

    #[test]
    fn build_argv_stress_test() {
        let mut state = WizardState::from_preset(&PRESETS[6]);
        state.url = "https://example.com".to_string();
        let argv = state.build_argv();
        assert!(argv.contains(&"--workers=20".to_string()));
        assert!(argv.contains(&"--max-reqs-per-sec=20".to_string()));
        assert!(argv.contains(&"--add-random-query-params".to_string()));
        assert!(argv.contains(&"--ignore-robots-txt".to_string()));
        assert!(argv.contains(&"--no-cache".to_string()));
        assert!(argv.contains(&"--disable-javascript".to_string()));
        assert!(argv.contains(&"--disable-styles".to_string()));
        assert!(argv.contains(&"--disable-fonts".to_string()));
        assert!(argv.contains(&"--disable-images".to_string()));
        assert!(argv.contains(&"--disable-files".to_string()));
    }

    #[test]
    fn build_argv_single_page() {
        let mut state = WizardState::from_preset(&PRESETS[7]);
        state.url = "https://example.com".to_string();
        let argv = state.build_argv();
        assert!(argv.contains(&"--single-page".to_string()));
        assert!(argv.contains(&"--workers=1".to_string()));
        assert!(argv.contains(&"--timeout=10".to_string()));
    }

    #[test]
    fn build_argv_large_site() {
        let mut state = WizardState::from_preset(&PRESETS[8]);
        state.url = "https://example.com".to_string();
        let argv = state.build_argv();
        assert!(argv.contains(&"--workers=10".to_string()));
        assert!(argv.contains(&"--max-reqs-per-sec=50".to_string()));
        assert!(argv.contains(&"--max-visited-urls=0".to_string()));
        assert!(argv.contains(&"--timeout=3".to_string()));
        assert!(argv.contains(&"--ignore-robots-txt".to_string()));
        assert!(argv.contains(&"--sitemap-xml-file='./sitemap.xml'".to_string()));
    }

    #[test]
    fn content_summary_all_enabled() {
        let state = WizardState::from_preset(&PRESETS[0]);
        assert_eq!(state.content_summary(), "HTML, JS, CSS, Fonts, Images, Files");
    }

    #[test]
    fn content_summary_html_only() {
        let state = WizardState::from_preset(&PRESETS[1]); // SEO
        assert_eq!(state.content_summary(), "HTML");
    }

    #[test]
    fn description_lengths_within_range() {
        for preset in PRESETS {
            let len = preset.description.len();
            assert!(
                (50..=65).contains(&len),
                "Preset '{}' description is {} chars (expected 50-65): \"{}\"",
                preset.name,
                len,
                preset.description
            );
        }
    }
}
