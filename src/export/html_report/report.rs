// SiteOne Crawler - HTML Report Generator
// (c) Jan Reges <jan.reges@siteone.cz>

use std::collections::HashMap;

use regex::Regex;

use crate::components::summary::item_status::ItemStatus;
use crate::components::super_table::SuperTable;
use crate::components::super_table_column::SuperTableColumn;
use crate::output::output::BasicStats;
use crate::result::status::Status;
use crate::result::visited_url;
use crate::scoring::scorer;
use crate::utils;
use crate::version;

use super::badge::{Badge, BadgeColor};
use super::tab::Tab;

// SuperTable apl_code constants (matching the analyzer module constants)
const SUPER_TABLE_VISITED_URLS: &str = "visited-urls";

// Analysis manager
const ST_ANALYSIS_STATS: &str = "analysis-stats";

// Content processor
const ST_CONTENT_PROCESSORS_STATS: &str = "content-processors-stats";

// Analyzers
const ST_HEADERS: &str = "headers";
const ST_HEADERS_VALUES: &str = "headers-values";
const ST_SEO: &str = "seo";
const ST_OPEN_GRAPH: &str = "open-graph";
const ST_SEO_HEADINGS: &str = "seo-headings";
const ST_DNS: &str = "dns";
const ST_CERTIFICATE_INFO: &str = "certificate-info";
const ST_NON_UNIQUE_TITLES: &str = "non-unique-titles";
const ST_NON_UNIQUE_DESCRIPTIONS: &str = "non-unique-descriptions";
const ST_CONTENT_TYPES: &str = "content-types";
const ST_CONTENT_MIME_TYPES: &str = "content-types-raw";
const ST_SKIPPED_SUMMARY: &str = "skipped-summary";
const ST_SKIPPED: &str = "skipped";
const ST_CACHING_PER_CONTENT_TYPE: &str = "caching-per-content-type";
const ST_CACHING_PER_DOMAIN: &str = "caching-per-domain";
const ST_CACHING_PER_DOMAIN_AND_CONTENT_TYPE: &str = "caching-per-domain-and-content-type";
const ST_REDIRECTS: &str = "redirects";
const ST_404: &str = "404";
const ST_FASTEST_URLS: &str = "fastest-urls";
const ST_SLOWEST_URLS: &str = "slowest-urls";
const ST_BEST_PRACTICES: &str = "best-practices";
const ST_ACCESSIBILITY: &str = "accessibility";
const ST_EXTERNAL_URLS: &str = "external-urls";
const ST_SECURITY: &str = "security";
const ST_SOURCE_DOMAINS: &str = "source-domains";

/// Analysis names for Best Practices
const BEST_PRACTICE_ANALYSIS_NAMES: &[&str] = &[
    "Large inline SVGs",
    "Duplicate inline SVGs",
    "Invalid inline SVGs",
    "Missing quotes on attributes",
    "DOM depth",
    "Heading structure",
    "Non-clickable phone numbers",
    "Title uniqueness",
    "Description uniqueness",
];

/// Analysis names for Accessibility
const ACCESSIBILITY_ANALYSIS_NAMES: &[&str] = &[
    "Valid HTML",
    "Missing image alt attributes",
    "Missing form labels",
    "Missing aria labels",
    "Missing roles",
    "Missing html lang attribute",
];

/// Analysis names for Security
const SECURITY_ANALYSIS_NAMES: &[&str] = &["Security headers"];

/// Severity order for sorting
const SEVERITY_ORDER_CRITICAL: i32 = 1;
const SEVERITY_ORDER_WARNING: i32 = 2;
const SEVERITY_ORDER_NOTICE: i32 = 3;

/// Max example URLs to show per finding
const MAX_EXAMPLE_URLS: usize = 5;

/// HTML template embedded at compile time
const TEMPLATE_HTML: &str = include_str!("template.html");

/// SuperTable apl_codes that are handled by dedicated tabs (not shown as generic tabs)
const SKIPPED_SUPER_TABLES: &[&str] = &[
    ST_ANALYSIS_STATS,
    ST_HEADERS_VALUES,
    ST_SEO,
    ST_OPEN_GRAPH,
    ST_DNS,
    ST_CERTIFICATE_INFO,
    ST_NON_UNIQUE_TITLES,
    ST_NON_UNIQUE_DESCRIPTIONS,
    ST_CONTENT_MIME_TYPES,
    ST_SKIPPED,
    ST_CACHING_PER_DOMAIN,
    ST_CACHING_PER_DOMAIN_AND_CONTENT_TYPE,
    ST_CONTENT_PROCESSORS_STATS,
];

/// Lightweight extracted info from a SuperTable (since SuperTable is not Clone)
struct SuperTableInfo {
    apl_code: String,
    title: String,
    forced_tab_label: Option<String>,
    html_output: String,
    total_rows: usize,
    data: Vec<HashMap<String, String>>,
}

/// Extract info from a SuperTable reference
fn extract_info(st: &SuperTable) -> SuperTableInfo {
    SuperTableInfo {
        apl_code: st.apl_code.clone(),
        title: st.title.clone(),
        forced_tab_label: st.forced_tab_label.clone(),
        html_output: st.get_html_output(),
        total_rows: st.get_total_rows(),
        data: st.get_data().to_vec(),
    }
}

/// SuperTable tab order
fn get_super_table_order(apl_code: &str) -> i32 {
    const ORDERS: &[&str] = &[
        SUPER_TABLE_VISITED_URLS,
        ST_BEST_PRACTICES,
        ST_ACCESSIBILITY,
        ST_SECURITY,
        ST_SEO,
        ST_SEO_HEADINGS,
        ST_404,
        ST_REDIRECTS,
        ST_SKIPPED_SUMMARY,
        ST_EXTERNAL_URLS,
        ST_FASTEST_URLS,
        ST_SLOWEST_URLS,
        ST_CONTENT_TYPES,
        ST_SOURCE_DOMAINS,
        ST_HEADERS,
        ST_CACHING_PER_CONTENT_TYPE,
        ST_DNS,
    ];

    ORDERS
        .iter()
        .position(|&code| code == apl_code)
        .map(|i| i as i32)
        .unwrap_or(1000)
}

/// Map SuperTable apl_code to section name for filtering
fn get_section_name_by_apl_code(apl_code: &str) -> Option<&'static str> {
    match apl_code {
        "accessibility" => Some("accessibility"),
        "404" => Some("404-pages"),
        "source-domains" => Some("source-domains"),
        "caching-per-content-type" | "caching-per-domain" | "caching-per-domain-and-content-type" => Some("caching"),
        "headers" | "headers-values" => Some("headers"),
        "slowest-urls" => Some("slowest-urls"),
        "fastest-urls" => Some("fastest-urls"),
        "best-practices" => Some("best-practices"),
        "skipped-summary" | "skipped" => Some("skipped-urls"),
        "external-urls" => Some("external-urls"),
        "redirects" => Some("redirects"),
        "security" => Some("security"),
        "content-types" | "content-types-raw" => Some("content-types"),
        "dns" | "certificate-info" => Some("dns-ssl"),
        "seo" | "open-graph" | "seo-headings" | "non-unique-titles" | "non-unique-descriptions" => {
            Some("seo-opengraph")
        }
        _ => None,
    }
}

/// HTML Report generator
pub struct HtmlReport<'a> {
    status: &'a Status,
    #[allow(dead_code)]
    max_example_urls: usize,
    allowed_sections: Option<Vec<String>>,
}

impl<'a> HtmlReport<'a> {
    pub fn new(status: &'a Status, max_example_urls: usize, html_report_options: Option<&str>) -> Self {
        let allowed_sections = html_report_options.filter(|s| !s.is_empty()).map(|opts| {
            opts.split(',')
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
                .collect()
        });

        Self {
            status,
            max_example_urls,
            allowed_sections,
        }
    }

    /// Generate the complete HTML report
    pub fn get_html(&self) -> String {
        let mut html = TEMPLATE_HTML.to_string();
        let template_variables = self.get_template_variables();

        for (var_name, var_value) in &template_variables {
            let placeholder = format!("{{${}}}", var_name);
            html = html.replace(&placeholder, var_value);
        }

        self.finalize_html(html)
    }

    /// Check if a section is allowed based on html_report_options
    fn is_section_allowed(&self, section_name: &str) -> bool {
        match &self.allowed_sections {
            None => true,
            Some(sections) => sections.iter().any(|s| s == section_name),
        }
    }

    /// Build template variables
    fn get_template_variables(&self) -> HashMap<String, String> {
        let info = self.status.get_crawler_info();
        let initial_host = self.get_initial_host();
        let initial_url = self.get_initial_url();
        let tabs = self.get_tabs();

        let command = info.command.clone();
        // Strip leading binary name prefix (e.g. "crawler.php ")
        let command = match Regex::new(r"^\S+\.php\s+") {
            Ok(re) => re.replace(&command, "").to_string(),
            _ => command,
        };
        let command = utils::get_safe_command(&command);

        let mut vars = HashMap::new();
        vars.insert("initialHost".to_string(), initial_host);
        vars.insert("initialUrl".to_string(), initial_url);
        vars.insert("version".to_string(), version::CODE.to_string());
        vars.insert("executedAt".to_string(), info.executed_at.clone());
        vars.insert("command".to_string(), command);
        vars.insert("hostname".to_string(), info.hostname.clone());
        vars.insert("userAgent".to_string(), info.final_user_agent.clone());
        vars.insert("tabs".to_string(), self.get_tabs_html(&tabs));
        vars.insert("tabsRadios".to_string(), self.get_tabs_radios(&tabs));
        vars.insert("tabsContent".to_string(), self.get_tabs_content_html(&tabs));
        vars.insert("tabsCss".to_string(), self.get_tabs_css(&tabs));
        vars
    }

    /// Post-process the HTML: convert colors, add badge classes, etc.
    fn finalize_html(&self, mut html: String) -> String {
        // Add badge class to colored spans
        if let Ok(re) = Regex::new(r#"(<span)\s+(style="background-color:[^"]+">)"#) {
            html = re.replace_all(&html, r#"$1 class="badge" $2"#).to_string();
        }
        if let Ok(re) = Regex::new(r#"(<span)\s+(style="color:[^"]+">)"#) {
            html = re.replace_all(&html, r#"$1 class="badge in-table" $2"#).to_string();
        }

        html = html.replace(
            r#"style="background-color: #ffff00""#,
            r#"style="background-color: #ffff00; color: #1F2937""#,
        );

        if let Ok(re) = Regex::new(r"(<td data-value='[0-9]+'[^>]*>)([0-9\-]+)(</td>)") {
            html = re
                .replace_all(&html, r#"$1<span class="badge">$2</span>$3"#)
                .to_string();
        }

        // Change magenta to orange
        html = html.replace("color: #ff00ff", "color: #ff9234");

        // Add spaces around slashes in content-type cells
        if let Ok(re) = Regex::new(r"(?i)(<td[^>]*>)(\s*[a-z0-9. /]+/[a-z0-9. /]+\s*)(</td>)") {
            html = re
                .replace_all(&html, |caps: &regex::Captures| {
                    let td_open = caps.get(1).map_or("", |m| m.as_str());
                    let content = caps.get(2).map_or("", |m| m.as_str());
                    let td_close = caps.get(3).map_or("", |m| m.as_str());
                    match Regex::new(r"\s*/\s*") {
                        Ok(slash_re) => {
                            let cleaned = slash_re.replace_all(content, " / ");
                            format!("{}{}{}", td_open, cleaned, td_close)
                        }
                        _ => {
                            format!("{}{}{}", td_open, content, td_close)
                        }
                    }
                })
                .to_string();
        }

        // Replace specific badge color styles with CSS classes
        let color_replacements = [
            (
                r#"<span class="badge in-table" style="color: #00ff00">"#,
                r#"<span class="badge green">"#,
            ),
            (
                r#"<span class="badge in-table" style="color: #ff9234">"#,
                r#"<span class="badge orange">"#,
            ),
            (
                r#"<span class="badge in-table" style="color: #ff0000">"#,
                r#"<span class="badge red">"#,
            ),
            (
                r#"<span class="badge in-table" style="background-color: #ffff00; color: #1F2937">"#,
                r#"<span class="badge yellow">"#,
            ),
            (
                r#"<span class="badge" style="background-color: #ffff00; color: #1F2937">"#,
                r#"<span class="badge yellow">"#,
            ),
            (
                r#"<span class="badge in-table" style="color: #ffff00">"#,
                r#"<span class="badge yellow">"#,
            ),
            (
                r#"<span class="badge in-table" style="color: #0000ff">"#,
                r#"<span class="badge blue">"#,
            ),
        ];

        for (from, to) in &color_replacements {
            html = html.replace(from, to);
        }

        // Remove excess whitespace from HTML
        html = remove_whitespaces_from_html(&html);

        html
    }

    /// Extract info from all SuperTables (within mutex closures) so we can work with them freely.
    fn extract_all_super_table_infos(&self) -> Vec<SuperTableInfo> {
        let mut all = Vec::new();
        let host = Some(self.get_initial_host());
        let scheme = Some(self.get_initial_scheme());
        let initial_url = Some(self.get_initial_url());
        self.status.with_super_tables_at_beginning_mut(|tables| {
            for st in tables.iter_mut() {
                st.set_host_to_strip_from_urls(host.clone(), scheme.clone());
                st.set_initial_url(initial_url.clone());
                all.push(extract_info(st));
            }
        });
        self.status.with_super_tables_at_end_mut(|tables| {
            for st in tables.iter_mut() {
                st.set_host_to_strip_from_urls(host.clone(), scheme.clone());
                st.set_initial_url(initial_url.clone());
                all.push(extract_info(st));
            }
        });
        all
    }

    /// Gather all tabs for the report
    fn get_tabs(&self) -> Vec<Tab> {
        let mut tabs: Vec<Tab> = Vec::new();

        if self.is_section_allowed("summary")
            && let Some(tab) = self.get_summary_tab()
        {
            tabs.push(tab);
        }
        if self.is_section_allowed("seo-opengraph")
            && let Some(tab) = self.get_seo_and_opengraph_tab()
        {
            tabs.push(tab);
        }
        if self.is_section_allowed("image-gallery")
            && let Some(tab) = self.get_image_gallery_tab()
        {
            tabs.push(tab);
        }
        if self.is_section_allowed("video-gallery")
            && let Some(tab) = self.get_video_gallery_tab()
        {
            tabs.push(tab);
        }
        if self.is_section_allowed("visited-urls") {
            tabs.push(self.get_visited_urls_tab());
        }
        if self.is_section_allowed("dns-ssl")
            && let Some(tab) = self.get_dns_and_ssl_tls_tab()
        {
            tabs.push(tab);
        }
        if self.is_section_allowed("crawler-stats") {
            tabs.push(self.get_crawler_stats_tab());
        }
        if self.is_section_allowed("crawler-info") {
            tabs.push(self.get_crawler_info_tab());
        }

        // Add tabs from SuperTables (analysis results)
        let super_table_tabs = self.get_super_table_tabs();
        tabs.extend(super_table_tabs);

        // Remove empty tabs
        tabs.retain(|tab| !tab.tab_content.is_empty());

        // Sort tabs by order
        tabs.sort_by_key(|tab| tab.get_final_sort_order());

        tabs
    }

    /// Build tabs from SuperTables that are not in SKIPPED_SUPER_TABLES
    fn get_super_table_tabs(&self) -> Vec<Tab> {
        let all_infos = self.extract_all_super_table_infos();
        let mut result = Vec::new();

        // Build analysis detail sub-tables
        let analysis_detail_html = self.build_analysis_detail_tables();

        for info in &all_infos {
            if SKIPPED_SUPER_TABLES.contains(&info.apl_code.as_str()) {
                continue;
            }

            // Check if this SuperTable's section is allowed
            if let Some(section_name) = get_section_name_by_apl_code(&info.apl_code)
                && !self.is_section_allowed(section_name)
            {
                continue;
            }

            let badges = get_super_table_badges_by_apl_code(info, &all_infos);
            let tab_label = info.forced_tab_label.as_deref().unwrap_or(&info.title);
            let content = get_tab_content_by_super_table(info, &all_infos, &analysis_detail_html);
            let order = get_super_table_order(&info.apl_code);

            result.push(Tab::new(tab_label, None, content, false, badges, Some(order)));
        }

        result
    }

    /// Generate hidden radio buttons for tabs
    fn get_tabs_radios(&self, tabs: &[Tab]) -> String {
        let mut html = String::new();
        let mut is_first = true;

        for tab in tabs {
            html.push_str(&format!(
                "<input type=\"radio\" id=\"{}\" name=\"tabs\" arial-label=\"Show tab {}\" class=\"tabs__radio\"{}>\n",
                html_escape(&tab.radio_html_id),
                html_escape(&tab.name),
                if is_first { " checked" } else { "" }
            ));
            if is_first {
                is_first = false;
            }
        }

        html
    }

    /// Generate tab navigation labels with badges
    fn get_tabs_html(&self, tabs: &[Tab]) -> String {
        let mut html = String::new();

        for tab in tabs {
            let mut badges_html = String::new();
            for badge in &tab.badges {
                let title_attr = if let Some(ref title) = badge.title {
                    format!(" style=\"cursor: help\" title=\"{}\"", html_escape(title))
                } else {
                    String::new()
                };
                badges_html.push_str(&format!(
                    "<span class=\"badge {}\"{}>{}</span> ",
                    badge.color.as_css_class(),
                    title_attr,
                    html_escape(&badge.value),
                ));
            }

            let badges_part = if !badges_html.is_empty() {
                format!(" {}", badges_html)
            } else {
                String::new()
            };

            html.push_str(&format!(
                "<label for=\"{}\" class=\"tabs__title {}\">{}{}</label>\n",
                html_escape(&tab.radio_html_id),
                html_escape(&tab.radio_html_id),
                html_escape(&tab.name),
                badges_part,
            ));
        }

        html
    }

    /// Generate tab content panels
    fn get_tabs_content_html(&self, tabs: &[Tab]) -> String {
        let mut html = String::new();
        let line_prefix = "                ";

        for tab in tabs {
            html.push_str(&format!(
                "{}<div class=\"tabs__tab {}\">\n",
                line_prefix,
                html_escape(&tab.content_html_id),
            ));
            if tab.add_heading {
                html.push_str(&format!("{}    <h2>{}</h2>\n", line_prefix, html_escape(&tab.name),));
            }

            let indented_content = tab.tab_content.replace('\n', &format!("\n{}    ", line_prefix));
            html.push_str(&format!("{}    {}\n", line_prefix, indented_content));
            html.push_str(&format!("{}</div>\n", line_prefix));
        }

        html
    }

    /// Generate CSS for tab radio button selectors
    fn get_tabs_css(&self, tabs: &[Tab]) -> String {
        let line_prefix = "        ";

        // Content visibility selectors
        let content_selectors: Vec<String> = tabs
            .iter()
            .map(|tab| {
                format!(
                    "#{radio}:checked ~ .tabs__content .{content}",
                    radio = tab.radio_html_id,
                    content = tab.content_html_id,
                )
            })
            .collect();

        let mut css = format!("{} {{\n", content_selectors.join(", "));
        css.push_str(&format!("{}    display: block;\n", line_prefix));
        css.push_str(&format!("{}}}\n", line_prefix));

        // Active tab title selectors
        let title_selectors: Vec<String> = tabs
            .iter()
            .map(|tab| {
                format!(
                    "#{radio}:checked ~ .tabs__navigation .{radio}",
                    radio = tab.radio_html_id,
                )
            })
            .collect();

        css.push_str(&format!("{} {{\n", title_selectors.join(", ")));
        css.push_str(&format!(
            "{}    background-color: var(--color-blue-600);\n",
            line_prefix
        ));
        css.push_str(&format!("{}    color: var(--color-white);\n", line_prefix));
        css.push_str(&format!("{}}}\n", line_prefix));

        css
    }

    // -------------------------------------------------------------------------
    // Individual tab generators
    // -------------------------------------------------------------------------

    /// Summary tab
    fn get_summary_tab(&self) -> Option<Tab> {
        let mut summary = self.status.get_summary();
        if summary.get_items().is_empty() {
            return None;
        }

        let color_to_count = [
            (BadgeColor::Red, summary.get_count_by_item_status(ItemStatus::Critical)),
            (
                BadgeColor::Orange,
                summary.get_count_by_item_status(ItemStatus::Warning),
            ),
            (BadgeColor::Blue, summary.get_count_by_item_status(ItemStatus::Notice)),
            (BadgeColor::Green, summary.get_count_by_item_status(ItemStatus::Ok)),
            (BadgeColor::Neutral, summary.get_count_by_item_status(ItemStatus::Info)),
        ];

        let badges: Vec<Badge> = color_to_count
            .into_iter()
            .filter(|(_, count)| *count > 0)
            .map(|(color, count)| Badge::new(count.to_string(), color))
            .collect();

        // Build quality scores HTML
        let basic_stats = self.status.get_basic_stats();
        let output_stats = BasicStats {
            total_urls: basic_stats.total_urls,
            total_size: basic_stats.total_size,
            total_size_formatted: basic_stats.total_size_formatted.clone(),
            total_execution_time: basic_stats.total_execution_time,
            total_requests_times: basic_stats.total_requests_times,
            total_requests_times_avg: basic_stats.total_requests_times_avg,
            total_requests_times_min: basic_stats.total_requests_times_min,
            total_requests_times_max: basic_stats.total_requests_times_max,
            count_by_status: basic_stats.count_by_status.clone(),
            count_by_content_type: basic_stats.count_by_content_type.clone(),
        };
        let scores = scorer::calculate_scores(&summary, &output_stats);
        let quality_html = build_quality_scores_html(&scores);

        let content = format!("{}\n{}", quality_html, summary.get_as_html());

        Some(Tab::new("Summary", None, content, true, badges, Some(-100)))
    }

    /// SEO and OpenGraph tab
    fn get_seo_and_opengraph_tab(&self) -> Option<Tab> {
        let all_infos = self.extract_all_super_table_infos();

        let mut html = String::new();
        let super_table_codes = [ST_NON_UNIQUE_TITLES, ST_NON_UNIQUE_DESCRIPTIONS, ST_SEO, ST_OPEN_GRAPH];

        let mut badge_count = 0usize;
        let mut order: Option<i32> = None;

        for code in &super_table_codes {
            if let Some(info) = all_infos.iter().find(|i| i.apl_code == *code) {
                html.push_str(&info.html_output);
                html.push_str("<br/>");
                if badge_count == 0 {
                    badge_count = info.total_rows;
                }
                if *code == ST_SEO {
                    order = Some(get_super_table_order(ST_SEO));
                }
            }
        }

        if html.is_empty() {
            return None;
        }

        let mut badges = Vec::new();

        if let Some(info) = all_infos.iter().find(|i| i.apl_code == ST_NON_UNIQUE_TITLES)
            && info.total_rows > 0
        {
            badges.push(Badge::with_title(
                info.total_rows.to_string(),
                BadgeColor::Orange,
                "Non-unique titles",
            ));
        }

        if let Some(info) = all_infos.iter().find(|i| i.apl_code == ST_NON_UNIQUE_DESCRIPTIONS)
            && info.total_rows > 0
        {
            badges.push(Badge::with_title(
                info.total_rows.to_string(),
                BadgeColor::Orange,
                "Non-unique descriptions",
            ));
        }

        badges.push(Badge::with_title(
            badge_count.to_string(),
            BadgeColor::Neutral,
            "Total URL with SEO info",
        ));

        Some(Tab::new("SEO and OpenGraph", None, html, false, badges, order))
    }

    /// Image Gallery tab
    fn get_image_gallery_tab(&self) -> Option<Tab> {
        let summary = self.status.get_summary();
        if summary.get_items().is_empty() {
            return None;
        }

        let visited_urls = self.status.get_visited_urls();
        let images: Vec<_> = visited_urls
            .iter()
            .filter(|v| {
                v.is_image()
                    && v.status_code == 200
                    && matches!(
                        v.source_attr,
                        visited_url::SOURCE_IMG_SRC | visited_url::SOURCE_INPUT_SRC | visited_url::SOURCE_CSS_URL
                    )
            })
            .collect();

        if images.is_empty() {
            return None;
        }

        let mut html = self.get_image_gallery_form_html();
        html.push_str("<div id=\"igc\" class=\"small\"><div id=\"igcf\" class=\"scaleDown\"><div id=\"image-gallery\" class=\"image-gallery\">");

        for image in &images {
            let size = image.size.unwrap_or(0);
            let content_type = image.content_type_header.as_deref().unwrap_or("");
            let source_url = self.status.get_url_by_uq_id(&image.source_uq_id);
            let source_url_str = source_url.as_deref().unwrap_or("");

            let image_description = format!(
                "{} ({}), found as {}",
                utils::get_formatted_size(size, 0),
                content_type,
                image.get_source_description(Some(source_url_str)),
            );

            let image_type = content_type.replace("image/", "");

            html.push_str(&format!(
                "<a href=\"{}\" target=\"_blank\" data-size=\"{}\" data-source=\"{}\" data-type=\"{}\" data-sizematch=\"1\" data-typematch=\"1\" data-sourcematch=\"1\">",
                html_escape(&image.url),
                size,
                html_escape(image.get_source_short_name()),
                html_escape(&image_type),
            ));
            html.push_str(&format!(
                "<img loading=\"lazy\" width=\"140\" height=\"140\" src=\"{}\" alt=\"{}\" title=\"{}\">",
                html_escape(&image.url),
                html_escape(&image_description),
                html_escape(&image_description),
            ));
            html.push_str("</a>\n");
        }
        html.push_str("</div></div></div>");

        let badges = vec![Badge::with_title(
            images.len().to_string(),
            BadgeColor::Neutral,
            "Found images",
        )];

        Some(Tab::new("Image Gallery", None, html, true, badges, Some(6)))
    }

    /// Video Gallery tab
    fn get_video_gallery_tab(&self) -> Option<Tab> {
        let summary = self.status.get_summary();
        if summary.get_items().is_empty() {
            return None;
        }

        let visited_urls = self.status.get_visited_urls();
        let videos: Vec<_> = visited_urls
            .iter()
            .filter(|v| v.is_video() && v.status_code == 200)
            .collect();

        if videos.is_empty() {
            return None;
        }

        let mut html = String::from(
            "<button onclick=\"playVideos()\" class=\"btn\">&#9654; Play the first 2 seconds of each video</button>",
        );
        html.push_str("<div id=\"vgc\" class=\"small\"><div id=\"vgcf\" class=\"scaleDown\"><div id=\"video-gallery\" class=\"video-container\">");

        for video in &videos {
            let size = video.size.unwrap_or(0);
            let content_type = video.content_type_header.as_deref().unwrap_or("");
            let source_url = self.status.get_url_by_uq_id(&video.source_uq_id);
            let source_url_str = source_url.as_deref().unwrap_or("");

            let video_description = format!(
                "{} ({}), <a href=\"{}\" target=\"_blank\">video</a> found on <a href=\"{}\" target=\"_blank\">this page</a>",
                utils::get_formatted_size(size, 0),
                content_type,
                html_escape(&video.url),
                html_escape(source_url_str),
            );

            html.push_str(&format!(
                "<div class=\"video-card\">\
                    <video data-src=\"{}\" preload=\"metadata\" controls></video>\
                    <div class=\"video-caption\">{}</div>\
                </div>\n",
                html_escape(&video.url),
                video_description,
            ));
        }
        html.push_str("</div></div></div>");

        html.push_str(VIDEO_GALLERY_SCRIPT);

        let badges = vec![Badge::with_title(
            videos.len().to_string(),
            BadgeColor::Neutral,
            "Found videos",
        )];

        Some(Tab::new("Video Gallery", None, html, true, badges, Some(6)))
    }

    /// DNS and SSL/TLS tab
    fn get_dns_and_ssl_tls_tab(&self) -> Option<Tab> {
        let all_infos = self.extract_all_super_table_infos();

        let mut html = String::new();
        let mut order: Option<i32> = None;
        let mut badges = Vec::new();

        // DNS table
        if let Some(dns_info) = all_infos.iter().find(|i| i.apl_code == ST_DNS) {
            html.push_str(&dns_info.html_output);
            html.push_str("<br/>");
            order = Some(get_super_table_order(ST_DNS));

            let mut ipv4 = 0usize;
            let mut ipv6 = 0usize;
            for row in &dns_info.data {
                if let Some(info_val) = row.get("info") {
                    let info_lower = info_val.to_lowercase();
                    if info_lower.contains("ipv4") {
                        ipv4 += 1;
                    } else if info_lower.contains("ipv6") {
                        ipv6 += 1;
                    }
                }
            }
            if ipv4 > 0 {
                let color = if ipv4 > 1 {
                    BadgeColor::Green
                } else {
                    BadgeColor::Neutral
                };
                badges.push(Badge::new(format!("{}x IPv4", ipv4), color));
            }
            if ipv6 > 0 {
                let color = if ipv6 > 1 {
                    BadgeColor::Green
                } else {
                    BadgeColor::Neutral
                };
                badges.push(Badge::new(format!("{}x IPv6", ipv6), color));
            }
        }

        // SSL/TLS certificate table
        if let Some(cert_info) = all_infos.iter().find(|i| i.apl_code == ST_CERTIFICATE_INFO) {
            html.push_str(&cert_info.html_output);
            html.push_str("<br/>");

            let mut errors = 0usize;
            for row in &cert_info.data {
                if let Some(info_val) = row.get("info")
                    && info_val == "Errors"
                    && let Some(value) = row.get("value")
                    && !value.is_empty()
                    && value != "[]"
                {
                    errors += 1;
                }
            }
            let tls_color = if errors > 0 { BadgeColor::Red } else { BadgeColor::Green };
            let tls_title = if errors > 0 {
                format!("SSL/TLS certificate: {} error(s)", errors)
            } else {
                "SSL/TLS certificate OK".to_string()
            };
            badges.push(Badge::with_title("TLS".to_string(), tls_color, &tls_title));
        }

        if html.is_empty() {
            return None;
        }

        Some(Tab::new("DNS and SSL", None, html, false, badges, order))
    }

    /// Crawler stats tab
    fn get_crawler_stats_tab(&self) -> Tab {
        let stats = self.status.get_basic_stats();
        let all_infos = self.extract_all_super_table_infos();

        let badges = vec![
            Badge::with_title(stats.total_urls.to_string(), BadgeColor::Neutral, "Total visited URLs"),
            Badge::with_title(
                stats.total_size_formatted.clone(),
                BadgeColor::Neutral,
                "Total size of all visited URLs",
            ),
            Badge::with_title(
                utils::get_formatted_duration(stats.total_execution_time),
                BadgeColor::Neutral,
                "Total execution time",
            ),
        ];

        let mut html = stats.get_as_html();

        if let Some(analysis_stats) = all_infos.iter().find(|i| i.apl_code == ST_ANALYSIS_STATS) {
            html.push_str("<br/>");
            html.push_str(&analysis_stats.html_output);
        }

        if let Some(cp_stats) = all_infos.iter().find(|i| i.apl_code == ST_CONTENT_PROCESSORS_STATS) {
            html.push_str("<br/>");
            html.push_str(&cp_stats.html_output);
        }

        Tab::new("Crawler stats", None, html, true, badges, Some(900))
    }

    /// Crawler info tab
    fn get_crawler_info_tab(&self) -> Tab {
        let info = self.status.get_crawler_info();
        let command = utils::get_safe_command(&info.command);

        let html = format!(
            r#"
            <h2>Crawler info</h2>
            <div class="info__wrapper">
                <table style="border-collapse: collapse;">
                    <tr>
                        <th>Version</th>
                        <td>{}</td>
                    </tr>
                    <tr>
                        <th>Executed At</th>
                        <td>{}</td>
                    </tr>
                    <tr>
                        <th>Command</th>
                        <td>{}</td>
                    </tr>
                    <tr>
                        <th>Hostname</th>
                        <td>{}</td>
                    </tr>
                    <tr>
                        <th>User-Agent</th>
                        <td>{}</td>
                    </tr>
                </table>
            </div>"#,
            html_escape(&info.version),
            html_escape(&info.executed_at),
            html_escape(&command),
            html_escape(&info.hostname),
            html_escape(&info.final_user_agent),
        );

        let badges = vec![Badge::with_title(
            format!("v{}", version::CODE),
            BadgeColor::Neutral,
            "Crawler version",
        )];

        Tab::new("Crawler info", None, html, false, badges, Some(5000))
    }

    /// Visited URLs tab
    fn get_visited_urls_tab(&self) -> Tab {
        let mut visited_urls_table = self.get_visited_urls_table();
        visited_urls_table.set_host_to_strip_from_urls(Some(self.get_initial_host()), Some(self.get_initial_scheme()));
        let badges = get_visited_urls_badges(&visited_urls_table);
        let order = get_super_table_order(SUPER_TABLE_VISITED_URLS);

        Tab::new(
            &visited_urls_table.title,
            visited_urls_table.description.as_deref(),
            visited_urls_table.get_html_output(),
            false,
            badges,
            Some(order),
        )
    }

    /// Build the visited URLs SuperTable
    fn get_visited_urls_table(&self) -> SuperTable {
        let visited_urls = self.status.get_visited_urls();

        let mut data: Vec<HashMap<String, String>> = Vec::new();
        for vu in &visited_urls {
            if vu.status_code == visited_url::ERROR_SKIPPED {
                continue;
            }

            let mut row = HashMap::new();
            row.insert("url".to_string(), vu.url.clone());
            row.insert("status".to_string(), vu.status_code.to_string());
            row.insert(
                "type".to_string(),
                utils::get_content_type_name_by_id(vu.content_type).to_string(),
            );
            row.insert("time".to_string(), format!("{:.3}", vu.request_time));
            row.insert("size".to_string(), vu.size.unwrap_or(0).to_string());
            row.insert(
                "sizeFormatted".to_string(),
                vu.size_formatted.clone().unwrap_or_default(),
            );
            row.insert("cacheTypeFlags".to_string(), vu.cache_type_flags.to_string());
            row.insert(
                "cacheLifetime".to_string(),
                vu.cache_lifetime.map(|v| v.to_string()).unwrap_or_default(),
            );

            if let Some(ref extras) = vu.extras {
                for (key, value) in extras {
                    row.insert(key.clone(), value.clone());
                }
            }

            data.push(row);
        }

        let initial_host = self.get_initial_host();
        let initial_scheme = self.get_initial_scheme();

        let columns = vec![
            SuperTableColumn::new(
                "url".to_string(),
                "URL".to_string(),
                -1,
                None,
                Some(Box::new(move |row: &HashMap<String, String>, _render_into: &str| {
                    let url = row.get("url").map(|s| s.as_str()).unwrap_or("");
                    let truncated =
                        utils::truncate_url(url, 80, "\u{2026}", Some(&initial_host), Some(&initial_scheme), None);
                    format!(
                        "<a href=\"{}\" target=\"_blank\">{}</a>",
                        url.replace('&', "&amp;")
                            .replace('"', "&quot;")
                            .replace('<', "&lt;")
                            .replace('>', "&gt;"),
                        truncated,
                    )
                })),
                true,
                false,
                false,
                false,
                None,
            ),
            SuperTableColumn::new(
                "status".to_string(),
                "Status".to_string(),
                6,
                Some(Box::new(|value: &str, _render_into: &str| {
                    if let Ok(v) = value.parse::<i32>() {
                        utils::get_colored_status_code(v, 6)
                    } else {
                        value.to_string()
                    }
                })),
                None,
                false,
                true,
                false,
                true,
                None,
            ),
            SuperTableColumn::new(
                "type".to_string(),
                "Type".to_string(),
                8,
                None,
                None,
                true,
                false,
                false,
                false,
                None,
            ),
            SuperTableColumn::new(
                "time".to_string(),
                "Time (s)".to_string(),
                8,
                None,
                Some(Box::new(|row: &HashMap<String, String>, _render_into: &str| {
                    let time_str = row.get("time").map(|s| s.as_str()).unwrap_or("0");
                    if let Ok(v) = time_str.parse::<f64>() {
                        utils::get_colored_request_time(v, 6)
                    } else {
                        time_str.to_string()
                    }
                })),
                false,
                true,
                false,
                true,
                None,
            ),
            SuperTableColumn::new(
                "size".to_string(),
                "Size".to_string(),
                8,
                None,
                Some(Box::new(|row: &HashMap<String, String>, _render_into: &str| {
                    let size_str = row.get("size").map(|s| s.as_str()).unwrap_or("0");
                    let size: i64 = size_str.parse().unwrap_or(0);
                    let formatted = row.get("sizeFormatted").map(|s| s.as_str()).unwrap_or("");
                    if size > 1024 * 1024 {
                        utils::get_color_text(formatted, "red", true)
                    } else {
                        formatted.to_string()
                    }
                })),
                false,
                true,
                false,
                true,
                None,
            ),
            {
                let mut col = SuperTableColumn::new(
                    "cacheLifetime".to_string(),
                    "Cache".to_string(),
                    8,
                    None,
                    Some(Box::new(|row: &HashMap<String, String>, _render_into: &str| {
                        let cache_lifetime_str = row.get("cacheLifetime").map(|s| s.as_str()).unwrap_or("");
                        let cache_type_flags: u32 = row.get("cacheTypeFlags").and_then(|s| s.parse().ok()).unwrap_or(0);
                        let str_pad_to = 6;

                        if let Ok(lifetime) = cache_lifetime_str.parse::<i64>() {
                            utils::get_colored_cache_lifetime(lifetime, str_pad_to)
                        } else if cache_type_flags & visited_url::CACHE_TYPE_HAS_NO_STORE != 0 {
                            utils::get_color_text(
                                &format!("{:<width$}", "0s (no-store)", width = str_pad_to),
                                "red",
                                true,
                            )
                        } else if cache_type_flags & visited_url::CACHE_TYPE_HAS_NO_CACHE != 0 {
                            utils::get_color_text(
                                &format!("{:<width$}", "0s (no-cache)", width = str_pad_to),
                                "red",
                                false,
                            )
                        } else if cache_type_flags & visited_url::CACHE_TYPE_HAS_ETAG != 0 {
                            utils::get_color_text(
                                &format!("{:<width$}", "ETag-only", width = str_pad_to),
                                "magenta",
                                false,
                            )
                        } else if cache_type_flags & visited_url::CACHE_TYPE_HAS_LAST_MODIFIED != 0 {
                            utils::get_color_text(
                                &format!("{:<width$}", "Last-Mod-only", width = str_pad_to),
                                "magenta",
                                false,
                            )
                        } else {
                            utils::get_color_text(&format!("{:<width$}", "None", width = str_pad_to), "red", false)
                        }
                    })),
                    false,
                    true,
                    false,
                    true,
                    Some(Box::new(|row: &HashMap<String, String>| {
                        let cache_lifetime_str = row.get("cacheLifetime").map(|s| s.as_str()).unwrap_or("");
                        let cache_type_flags: u32 = row.get("cacheTypeFlags").and_then(|s| s.parse().ok()).unwrap_or(0);

                        if let Ok(lifetime) = cache_lifetime_str.parse::<i64>() {
                            lifetime.to_string()
                        } else if cache_type_flags & visited_url::CACHE_TYPE_HAS_NO_STORE != 0 {
                            "-2".to_string()
                        } else if cache_type_flags & visited_url::CACHE_TYPE_HAS_NO_CACHE != 0 {
                            "-1".to_string()
                        } else if cache_type_flags & visited_url::CACHE_TYPE_HAS_ETAG != 0 {
                            "0.1".to_string()
                        } else if cache_type_flags & visited_url::CACHE_TYPE_HAS_LAST_MODIFIED != 0 {
                            "0.2".to_string()
                        } else {
                            "0.01".to_string()
                        }
                    })),
                );
                col.forced_data_type = Some("number".to_string());
                col
            },
        ];

        let mut super_table = SuperTable::new(
            SUPER_TABLE_VISITED_URLS.to_string(),
            "Visited URLs".to_string(),
            "No visited URLs.".to_string(),
            columns,
            false,
            None,
            "ASC".to_string(),
            None,
            None,
            None,
        );
        super_table.set_ignore_hard_rows_limit(true);
        super_table.set_data(data);

        super_table
    }

    // -------------------------------------------------------------------------
    // Helpers
    // -------------------------------------------------------------------------

    /// Image gallery form HTML (size/mode/filter controls)
    fn get_image_gallery_form_html(&self) -> String {
        let mut html = String::from(
            r#"
            <style>
            #imageDisplayForm {
                display: flex;
                gap: 12px;
                flex-wrap: wrap;
                margin-bottom: 20px;
            }
            </style>"#,
        );

        html.push_str(
            r#"<script>
                function updateClassName(elementId, className) {
                    document.getElementById(elementId).className = className;
                    if (elementId === "igc") {
                        var images = document.getElementById(elementId).getElementsByTagName("img");
                        for (var i = 0; i < images.length; i++) {
                            var image = images[i];
                            image.width = className === "small" ? 140 : (className === "medium" ? 200 : 360);
                            image.height = className === "small" ? 140 : (className === "medium" ? 200 : 360);
                        }
                    }
                }
            </script>"#,
        );

        html.push_str(IMAGE_GALLERY_FILTER_SCRIPT);

        html.push_str(r#"<form id="imageDisplayForm">
                <div class="form-group">
                    <div class="btn-group">
                        <input class="idf" type="radio" id="sizeSmall" name="thumbnailSize" value="small" data-key="igc" checked>
                        <label for="sizeSmall">small</label>
                        <input class="idf" type="radio" id="sizeMedium" name="thumbnailSize" value="medium" data-key="igc">
                        <label for="sizeMedium">medium</label>
                        <input class="idf" type="radio" id="sizeLarge" name="thumbnailSize" value="large" data-key="igc">
                        <label for="sizeLarge">large</label>
                    </div>
                </div>
                <div class="form-group">
                    <div class="btn-group">
                        <input class="idf" type="radio" id="modeScaleDown" name="thumbnailMode" value="scaleDown" data-key="igcf" checked>
                        <label for="modeScaleDown">scale-down</label>
                        <input class="idf" type="radio" id="modeContain" name="thumbnailMode" value="contain" data-key="igcf">
                        <label for="modeContain">contain</label>
                        <input class="idf" type="radio" id="modeCover" name="thumbnailMode" value="cover" data-key="igcf">
                        <label for="modeCover">cover</label>
                    </div>
                </div>
                <div class="form-group">
                    <div class="btn-group" id="typeFilters">
                    </div>
                </div>
                <div class="form-group">
                    <div class="btn-group" id="sourceFilters">
                    </div>
                </div>
                <div class="form-group">
                    <div class="btn-group" id="sizeFilters">
                    </div>
                </div>
            </form>"#);

        html
    }

    /// Get initial host from the URL
    fn get_initial_host(&self) -> String {
        let url = self.get_initial_url();
        url::Url::parse(&url)
            .ok()
            .and_then(|u| u.host_str().map(|h| h.to_string()))
            .unwrap_or_default()
    }

    /// Get initial URL from status
    fn get_initial_url(&self) -> String {
        self.status.get_crawler_info().initial_url.clone()
    }

    /// Get initial scheme from the URL
    fn get_initial_scheme(&self) -> String {
        let url = self.get_initial_url();
        url::Url::parse(&url)
            .ok()
            .map(|u| u.scheme().to_string())
            .unwrap_or_else(|| "https".to_string())
    }

    /// Build analysis detail sub-tables for Best Practices, Accessibility, and Security tabs.
    /// Returns a map of analysis_name -> rendered HTML table.
    fn build_analysis_detail_tables(&self) -> HashMap<String, String> {
        let initial_host = self.get_initial_host();
        let initial_scheme = self.get_initial_scheme();

        // Gather all per-URL analysis details, aggregated by analysis_name
        let aggregated = self.get_data_for_super_tables_with_details();

        let mut result = HashMap::new();

        // Build all analysis names from all three analyzers
        let all_names: Vec<&str> = BEST_PRACTICE_ANALYSIS_NAMES
            .iter()
            .chain(ACCESSIBILITY_ANALYSIS_NAMES.iter())
            .chain(SECURITY_ANALYSIS_NAMES.iter())
            .copied()
            .collect();

        for analysis_name in all_names {
            let mut data = aggregated.get(analysis_name).cloned().unwrap_or_default();

            // Sort by severity (ascending) then by count (descending)
            data.sort_by(|a, b| {
                let sev_a: i32 = a.get("severity").and_then(|s| s.parse().ok()).unwrap_or(999);
                let sev_b: i32 = b.get("severity").and_then(|s| s.parse().ok()).unwrap_or(999);
                if sev_a == sev_b {
                    let count_a: usize = a.get("count").and_then(|s| s.parse().ok()).unwrap_or(0);
                    let count_b: usize = b.get("count").and_then(|s| s.parse().ok()).unwrap_or(0);
                    count_b.cmp(&count_a)
                } else {
                    sev_a.cmp(&sev_b)
                }
            });

            let apl_code = analysis_name.to_lowercase().replace(' ', "-");
            let initial_host_clone = initial_host.clone();
            let initial_scheme_clone = initial_scheme.clone();

            let columns = vec![
                SuperTableColumn::new(
                    "severity".to_string(),
                    "Severity".to_string(),
                    10,
                    None,
                    Some(Box::new(|row: &HashMap<String, String>, _render_into: &str| {
                        let sev = row.get("severityFormatted").map(|s| s.as_str()).unwrap_or("");
                        utils::get_colored_severity(sev)
                    })),
                    false,
                    false,
                    false,
                    true,
                    None,
                ),
                SuperTableColumn::new(
                    "count".to_string(),
                    "Occurs".to_string(),
                    8,
                    None,
                    None,
                    false,
                    false,
                    false,
                    true,
                    None,
                ),
                SuperTableColumn::new(
                    "detail".to_string(),
                    "Detail".to_string(),
                    200,
                    Some(Box::new(|value: &str, _render_into: &str| {
                        // HTML-escape for safety, then convert newlines to <br>
                        let escaped = html_escape(value);
                        escaped.replace('\n', "<br>")
                    })),
                    None,
                    false,
                    true,
                    false,
                    false,
                    None,
                ),
                SuperTableColumn::new(
                    "exampleUrls".to_string(),
                    format!("Affected URLs (max {})", MAX_EXAMPLE_URLS),
                    60,
                    None,
                    Some(Box::new(move |row: &HashMap<String, String>, _render_into: &str| {
                        let urls_str = row.get("exampleUrls").map(|s| s.as_str()).unwrap_or("");
                        if urls_str.is_empty() {
                            return String::new();
                        }
                        let urls: Vec<&str> = urls_str.split('\x1E').collect(); // record separator
                        let mut html_out = String::new();
                        if urls.len() == 1 {
                            for url in &urls {
                                let truncated = utils::truncate_url(
                                    url,
                                    60,
                                    "\u{2026}",
                                    Some(&initial_host_clone),
                                    Some(&initial_scheme_clone),
                                    None,
                                );
                                html_out.push_str(&format!(
                                    "<a href=\"{}\" target=\"_blank\">{}</a><br />",
                                    html_escape(url),
                                    html_escape(&truncated),
                                ));
                            }
                        } else {
                            for (i, url) in urls.iter().enumerate() {
                                html_out.push_str(&format!(
                                    "<a href=\"{}\" target=\"_blank\">URL {}</a>, ",
                                    html_escape(url),
                                    i + 1,
                                ));
                            }
                        }
                        html_out.trim_end_matches(", ").to_string()
                    })),
                    false,
                    true,
                    false,
                    false,
                    None,
                ),
            ];

            let mut super_table = SuperTable::new(
                apl_code,
                analysis_name.to_string(),
                "No problems found.".to_string(),
                columns,
                false,
                None,
                "ASC".to_string(),
                None,
                Some(100),
                None,
            );

            super_table.set_data(data);
            let html = super_table.get_html_output();
            result.insert(analysis_name.to_string(), html);
        }

        result
    }

    /// Gather per-URL analysis details, aggregated by analysis_name.
    /// Returns analysis_name -> Vec of aggregated rows.
    fn get_data_for_super_tables_with_details(&self) -> HashMap<String, Vec<HashMap<String, String>>> {
        let analysis_results = self.status.get_visited_url_to_analysis_result();
        let mut raw_data: HashMap<String, Vec<(String, String, i32, String)>> = HashMap::new();

        for (uq_id, entries) in &analysis_results {
            let url = self.status.get_url_by_uq_id(uq_id).unwrap_or_default();

            for entry in entries {
                let result = &entry.result;

                // Critical details
                for (analysis_name, details) in result.get_critical_details() {
                    for detail in details {
                        raw_data.entry(analysis_name.clone()).or_default().push((
                            url.clone(),
                            "critical".to_string(),
                            SEVERITY_ORDER_CRITICAL,
                            detail.clone(),
                        ));
                    }
                }

                // Warning details
                for (analysis_name, details) in result.get_warning_details() {
                    for detail in details {
                        raw_data.entry(analysis_name.clone()).or_default().push((
                            url.clone(),
                            "warning".to_string(),
                            SEVERITY_ORDER_WARNING,
                            detail.clone(),
                        ));
                    }
                }

                // Notice details
                for (analysis_name, details) in result.get_notice_details() {
                    for detail in details {
                        raw_data.entry(analysis_name.clone()).or_default().push((
                            url.clone(),
                            "notice".to_string(),
                            SEVERITY_ORDER_NOTICE,
                            detail.clone(),
                        ));
                    }
                }
            }
        }

        // Aggregate: group identical (severity, aggregated_detail) pairs and count occurrences.
        let mut aggregated: HashMap<String, Vec<HashMap<String, String>>> = HashMap::new();

        for (analysis_name, rows) in &raw_data {
            let mut groups: HashMap<String, HashMap<String, String>> = HashMap::new();
            let mut group_urls: HashMap<String, Vec<String>> = HashMap::new();

            for (url, severity_formatted, severity_order, detail) in rows {
                let agg_detail = aggregate_detail(detail);
                let agg_key = aggregate_detail_key(severity_formatted, &agg_detail);
                let entry = groups.entry(agg_key.clone()).or_insert_with(|| {
                    let mut row = HashMap::new();
                    row.insert("severityFormatted".to_string(), severity_formatted.clone());
                    row.insert("severity".to_string(), severity_order.to_string());
                    row.insert("detail".to_string(), agg_detail.clone());
                    row.insert("count".to_string(), "0".to_string());
                    row
                });
                let count: usize = entry.get("count").and_then(|c| c.parse().ok()).unwrap_or(0);
                entry.insert("count".to_string(), (count + 1).to_string());

                let urls = group_urls.entry(agg_key).or_default();
                if urls.len() < MAX_EXAMPLE_URLS && !urls.contains(url) {
                    urls.push(url.clone());
                }
            }

            let mut result_rows: Vec<HashMap<String, String>> = Vec::new();
            for (key, mut row) in groups {
                if let Some(urls) = group_urls.get(&key) {
                    row.insert("exampleUrls".to_string(), urls.join("\x1E"));
                }
                result_rows.push(row);
            }

            aggregated.insert(analysis_name.clone(), result_rows);
        }

        aggregated
    }
}

// =============================================================================
// Free functions that work on extracted SuperTableInfo (no &self needed)
// =============================================================================

/// Generate tab content for a SuperTable, potentially including related sub-tables
fn get_tab_content_by_super_table(
    info: &SuperTableInfo,
    all_infos: &[SuperTableInfo],
    analysis_detail_html: &HashMap<String, String>,
) -> String {
    let mut html = info.html_output.clone();

    // Add related sub-tables based on apl_code
    let related_codes: Vec<&str> = match info.apl_code.as_str() {
        "skipped-summary" => vec![ST_SKIPPED],
        "headers" => vec![ST_HEADERS_VALUES],
        "content-types" => vec![ST_CONTENT_MIME_TYPES],
        "caching-per-content-type" => vec![ST_CACHING_PER_DOMAIN, ST_CACHING_PER_DOMAIN_AND_CONTENT_TYPE],
        _ => vec![],
    };

    for related_code in related_codes {
        if let Some(related) = all_infos.iter().find(|i| i.apl_code == related_code) {
            html.push_str("<br/>");
            html.push_str(&related.html_output);
        }
    }

    // Add analysis detail sub-tables for best-practices, accessibility, security
    let analysis_names: &[&str] = match info.apl_code.as_str() {
        "best-practices" => BEST_PRACTICE_ANALYSIS_NAMES,
        "accessibility" => ACCESSIBILITY_ANALYSIS_NAMES,
        "security" => SECURITY_ANALYSIS_NAMES,
        _ => &[],
    };

    for analysis_name in analysis_names {
        if let Some(detail_html) = analysis_detail_html.get(*analysis_name) {
            html.push_str("<br/>");
            html.push_str(detail_html);
        }
    }

    html
}

/// Get badges for visited URLs table
fn get_visited_urls_badges(super_table: &SuperTable) -> Vec<Badge> {
    let mut badges = Vec::new();
    let mut red = 0usize;
    let mut orange = 0usize;
    let mut green = 0usize;

    for row in super_table.get_data() {
        let status_code: i32 = row.get("status").and_then(|s| s.parse().ok()).unwrap_or(0);

        if status_code <= 0 || status_code >= 400 {
            red += 1;
        } else if status_code >= 300 {
            orange += 1;
        } else {
            green += 1;
        }
    }

    if red > 0 {
        badges.push(Badge::with_title(
            red.to_string(),
            BadgeColor::Red,
            "Errors (40x, 50x, timeout, etc.)",
        ));
    }
    if orange > 0 {
        badges.push(Badge::with_title(
            orange.to_string(),
            BadgeColor::Orange,
            "Redirects (30x)",
        ));
    }
    if green > 0 {
        badges.push(Badge::with_title(green.to_string(), BadgeColor::Green, "OK (20x)"));
    }

    badges
}

/// Get badges for a SuperTable based on its apl_code
fn get_super_table_badges_by_apl_code(info: &SuperTableInfo, all_infos: &[SuperTableInfo]) -> Vec<Badge> {
    let mut badges = Vec::new();

    match info.apl_code.as_str() {
        "redirects" => {
            let redirects = info.total_rows;
            let color = if redirects > 100 {
                BadgeColor::Red
            } else if redirects > 0 {
                BadgeColor::Orange
            } else {
                BadgeColor::Green
            };
            badges.push(Badge::new(redirects.to_string(), color));
        }
        "404" => {
            let not_found = info.total_rows;
            let color = if not_found > 10 {
                BadgeColor::Red
            } else if not_found > 0 {
                BadgeColor::Orange
            } else {
                BadgeColor::Green
            };
            badges.push(Badge::new(not_found.to_string(), color));
        }
        "skipped-summary" => {
            let skipped = info.total_rows;
            let color = if skipped > 10 {
                BadgeColor::Orange
            } else {
                BadgeColor::Green
            };
            badges.push(Badge::with_title(skipped.to_string(), color, "Skipped URL domains"));
            if let Some(skipped_urls) = all_infos.iter().find(|i| i.apl_code == ST_SKIPPED) {
                badges.push(Badge::with_title(
                    skipped_urls.total_rows.to_string(),
                    BadgeColor::Neutral,
                    "Total skipped URLs",
                ));
            }
        }
        "source-domains" => {
            let domains = info.total_rows;
            let color = if domains > 10 {
                BadgeColor::Orange
            } else {
                BadgeColor::Neutral
            };
            badges.push(Badge::new(domains.to_string(), color));
        }
        "content-types" => {
            let content_types = info.total_rows;
            badges.push(Badge::with_title(
                content_types.to_string(),
                BadgeColor::Neutral,
                "Total content types",
            ));
            if let Some(mime_types) = all_infos.iter().find(|i| i.apl_code == ST_CONTENT_MIME_TYPES) {
                badges.push(Badge::with_title(
                    mime_types.total_rows.to_string(),
                    BadgeColor::Neutral,
                    "Total MIME types",
                ));
            }
        }
        "fastest-urls" => {
            let fastest_time = info
                .data
                .iter()
                .filter_map(|row| row.get("time").and_then(|s| s.parse::<f64>().ok()))
                .fold(None, |acc: Option<f64>, t| Some(acc.map_or(t, |a| a.min(t))));
            if let Some(time) = fastest_time {
                let color = if time < 0.5 {
                    BadgeColor::Green
                } else if time < 2.0 {
                    BadgeColor::Orange
                } else {
                    BadgeColor::Red
                };
                badges.push(Badge::new(utils::get_formatted_duration(time), color));
            }
        }
        "slowest-urls" => {
            let slowest_time = info
                .data
                .iter()
                .filter_map(|row| row.get("time").and_then(|s| s.parse::<f64>().ok()))
                .fold(None, |acc: Option<f64>, t| Some(acc.map_or(t, |a| a.max(t))));
            if let Some(time) = slowest_time {
                let color = if time < 0.5 {
                    BadgeColor::Green
                } else if time < 2.0 {
                    BadgeColor::Orange
                } else {
                    BadgeColor::Red
                };
                badges.push(Badge::new(utils::get_formatted_duration(time), color));
            }
        }
        "headers" => {
            let headers = info.total_rows;
            let color = if headers > 50 {
                BadgeColor::Red
            } else {
                BadgeColor::Neutral
            };
            badges.push(Badge::new(headers.to_string(), color));
        }
        "external-urls" => {
            let count = info.total_rows;
            let color = if count > 0 {
                BadgeColor::Neutral
            } else {
                BadgeColor::Green
            };
            badges.push(Badge::with_title(count.to_string(), color, "External URLs"));
        }
        "caching-per-content-type" => {
            let mut min_cache_lifetime: Option<i64> = None;
            let mut max_cache_lifetime: Option<i64> = None;

            for row in &info.data {
                let content_type = row.get("contentType").map(|s| s.as_str()).unwrap_or("");
                if !["Image", "CSS", "JS", "Font"].contains(&content_type) {
                    continue;
                }
                if let Some(min_val) = row.get("minLifetime").and_then(|s| s.parse::<i64>().ok()) {
                    min_cache_lifetime = Some(min_cache_lifetime.map_or(min_val, |v: i64| v.min(min_val)));
                }
                if let Some(max_val) = row.get("maxLifetime").and_then(|s| s.parse::<i64>().ok()) {
                    max_cache_lifetime = Some(max_cache_lifetime.map_or(max_val, |v: i64| v.max(max_val)));
                }
            }

            if let Some(min_lt) = min_cache_lifetime {
                let color = if min_lt < 60 {
                    BadgeColor::Red
                } else if min_lt < 3600 {
                    BadgeColor::Orange
                } else {
                    BadgeColor::Green
                };
                badges.push(Badge::with_title(
                    utils::get_formatted_cache_lifetime(min_lt),
                    color,
                    "Minimal cache lifetime for images/css/js/fonts",
                ));
            }
            if let Some(max_lt) = max_cache_lifetime {
                let color = if max_lt < 60 {
                    BadgeColor::Red
                } else if max_lt < 3600 {
                    BadgeColor::Orange
                } else {
                    BadgeColor::Green
                };
                badges.push(Badge::with_title(
                    utils::get_formatted_cache_lifetime(max_lt),
                    color,
                    "Maximal cache lifetime for images/css/js/fonts",
                ));
            }
        }
        _ => {
            // Use generic badges for other tables
            badges = get_super_table_generic_badges(info);
        }
    }

    badges
}

/// Get generic badges by counting severity columns
fn get_super_table_generic_badges(info: &SuperTableInfo) -> Vec<Badge> {
    let mut badges = Vec::new();
    let mut red = 0i64;
    let mut orange = 0i64;
    let mut blue = 0i64;
    let mut green = 0i64;
    let mut neutral = 0i64;

    for row in &info.data {
        if let Some(val) = row.get("ok").and_then(|s| s.parse::<i64>().ok()) {
            green += val;
        }
        if let Some(val) = row.get("notice").and_then(|s| s.parse::<i64>().ok()) {
            blue += val;
        }
        if let Some(val) = row.get("warning").and_then(|s| s.parse::<i64>().ok()) {
            orange += val;
        }
        if let Some(val) = row.get("critical").and_then(|s| s.parse::<i64>().ok()) {
            red += val;
        }
        if let Some(val) = row.get("error").and_then(|s| s.parse::<i64>().ok()) {
            red += val;
        }
        if let Some(val) = row.get("info").and_then(|s| s.parse::<i64>().ok()) {
            neutral += val;
        }
    }

    if red > 0 {
        badges.push(Badge::with_title(red.to_string(), BadgeColor::Red, "Critical"));
    }
    if orange > 0 {
        badges.push(Badge::with_title(orange.to_string(), BadgeColor::Orange, "Warning"));
    }
    if blue > 0 {
        badges.push(Badge::with_title(blue.to_string(), BadgeColor::Blue, "Notice"));
    }
    if green > 0 {
        badges.push(Badge::with_title(green.to_string(), BadgeColor::Green, "OK"));
    }
    if neutral > 0 {
        badges.push(Badge::with_title(neutral.to_string(), BadgeColor::Neutral, "Info"));
    }

    badges
}

/// HTML-escape a string
fn html_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#039;")
}

/// Remove excessive whitespace from HTML.
/// Build the quality scores HTML block (donut chart + category bars) for the Summary tab.
fn build_quality_scores_html(scores: &crate::scoring::quality_score::QualityScores) -> String {
    let overall = &scores.overall;
    let deg = overall.score / 10.0 * 360.0;
    let color = overall.color_hex();

    let mut html = String::new();

    // Embedded styles for light (default) and dark (checked) modes
    html.push_str(concat!(
        "<style>\n",
        ".qs-box{margin-bottom:24px;padding:20px;border-radius:12px;background:#F3F4F6;}\n",
        ".qs-title{margin:0 0 16px;font-size:18px;color:#111827;}\n",
        ".qs-donut-inner{background:#F3F4F6;}\n",
        ".qs-bar-track{background:#E5E7EB;}\n",
        ".qs-cat-name{color:#4B5563;}\n",
        "html:has(.theme-switch__input:checked) .qs-box{background:#1F2937;}\n",
        "html:has(.theme-switch__input:checked) .qs-title{color:#F9FAFB;}\n",
        "html:has(.theme-switch__input:checked) .qs-donut-inner{background:#1F2937;}\n",
        "html:has(.theme-switch__input:checked) .qs-bar-track{background:#374151;}\n",
        "html:has(.theme-switch__input:checked) .qs-cat-name{color:#D1D5DB;}\n",
        "</style>\n",
    ));

    // Container
    html.push_str("<div class=\"qs-box\">\n");
    html.push_str("<h3 class=\"qs-title\">Website Quality Score</h3>\n");

    // Flex container for donut + categories
    html.push_str("<div style=\"display:flex;align-items:center;gap:32px;flex-wrap:wrap;\">\n");

    // Donut chart — track color via qs-bar-track on outer ring
    html.push_str(&format!(
        concat!(
            "<div class=\"qs-bar-track\" style=\"position:relative;width:140px;height:140px;border-radius:50%;",
            "background:conic-gradient({color} 0deg {deg:.1}deg,transparent {deg:.1}deg 360deg);",
            "flex-shrink:0;\">\n",
            "<div class=\"qs-donut-inner\" style=\"position:absolute;top:50%;left:50%;transform:translate(-50%,-50%);",
            "width:100px;height:100px;border-radius:50%;",
            "display:flex;flex-direction:column;align-items:center;justify-content:center;\">\n",
            "<span style=\"font-size:28px;font-weight:bold;color:{color};\">{score:.1}</span>\n",
            "<span style=\"font-size:13px;color:{color};\">{label}</span>\n",
            "</div>\n</div>\n",
        ),
        color = color,
        deg = deg,
        score = overall.score,
        label = overall.label,
    ));

    // Category bars container
    html.push_str("<div style=\"flex:1;min-width:200px;\">\n");

    for cat in &scores.categories {
        let pct = cat.score / 10.0 * 100.0;
        let cat_color = cat.color_hex();
        html.push_str(&format!(
            concat!(
                "<div style=\"display:flex;align-items:center;margin-bottom:8px;\">\n",
                "<span class=\"qs-cat-name\" style=\"width:120px;font-size:13px;\">{name}</span>\n",
                "<div class=\"qs-bar-track\" style=\"flex:1;height:12px;border-radius:6px;margin:0 10px;overflow:hidden;\">\n",
                "<div style=\"width:{pct:.0}%;height:100%;background:{color};border-radius:6px;\"></div>\n",
                "</div>\n",
                "<span style=\"width:36px;color:{color};font-weight:bold;font-size:13px;text-align:right;\">{score:.1}</span>\n",
                "</div>\n",
            ),
            name = cat.name,
            pct = pct,
            color = cat_color,
            score = cat.score,
        ));
    }

    html.push_str("</div>\n"); // end bars container
    html.push_str("</div>\n"); // end flex
    html.push_str("</div>\n"); // end outer container

    html
}

///   1. Inside <script>/<style> blocks: only replace "> <" with "> <"
///   2. Collapse all whitespace to single space
///   3. Replace "> <" with "> <"
fn remove_whitespaces_from_html(html: &str) -> String {
    use once_cell::sync::Lazy;
    use regex::Regex;

    // Separate regexes for script and style (no backreference needed)
    static RE_SCRIPT: Lazy<Regex> = Lazy::new(|| Regex::new(r"(?is)<script\b[^>]*>.*?</script>").unwrap());
    static RE_STYLE: Lazy<Regex> = Lazy::new(|| Regex::new(r"(?is)<style\b[^>]*>.*?</style>").unwrap());
    static RE_WHITESPACE: Lazy<Regex> = Lazy::new(|| Regex::new(r"\s+").unwrap());
    static RE_TAG_WHITESPACE: Lazy<Regex> = Lazy::new(|| Regex::new(r">\s+<").unwrap());

    // Step 1: In script blocks, replace "> <" with "> <"
    let html = RE_SCRIPT.replace_all(html, |caps: &regex::Captures| {
        RE_TAG_WHITESPACE.replace_all(&caps[0], "> <").to_string()
    });

    // Step 1b: In style blocks, replace "> <" with "> <"
    let html = RE_STYLE.replace_all(&html, |caps: &regex::Captures| {
        RE_TAG_WHITESPACE.replace_all(&caps[0], "> <").to_string()
    });

    // Step 2: Collapse all whitespace to single space
    let html = RE_WHITESPACE.replace_all(&html, " ");

    // Step 3: Replace "> <" with "> <"
    let html = RE_TAG_WHITESPACE.replace_all(&html, "> <");

    html.to_string()
}

/// Normalize a detail string for aggregation/deduplication.
/// 1. For SVG details, return as-is
/// 2. Remove all HTML attributes except id, class, name (replace with " *** ")
/// 3. Extract only the first HTML tag
/// 4. Replace trailing numbers before quotes with ***
fn aggregate_detail(detail: &str) -> String {
    use once_cell::sync::Lazy;
    use regex::Regex;

    // SVG details pass through unchanged
    if detail.starts_with("<svg") || detail.contains("x SVG ") {
        return detail.to_string();
    }

    // Step 1: Remove unwanted attributes, keeping only id, class, name
    static RE_TAG_ATTRS: Lazy<Regex> = Lazy::new(|| Regex::new(r"(?is)<([a-z][a-z0-9]*)\s+([^>]*)>").unwrap());
    static RE_ATTR: Lazy<Regex> =
        Lazy::new(|| Regex::new(r#"(?is)([a-z][-a-z0-9_]*)\s*=\s*("(?:[^"]*)"?|'(?:[^']*)'?)"#).unwrap());

    let allowed_attrs = ["id", "class", "name"];
    let svg_tags = [
        "svg",
        "g",
        "path",
        "circle",
        "rect",
        "line",
        "polyline",
        "polygon",
        "text",
        "tspan",
        "use",
        "defs",
        "clippath",
        "mask",
        "pattern",
        "marker",
        "lineargradient",
        "radialgradient",
        "stop",
        "image",
        "foreignobject",
    ];

    let result = RE_TAG_ATTRS.replace_all(detail, |caps: &regex::Captures| {
        let tag_name = &caps[1];
        let attrs_string = &caps[2];

        // Don't modify SVG tags
        if svg_tags.contains(&tag_name.to_lowercase().as_str()) {
            return caps[0].to_string();
        }

        let mut kept_attrs = String::new();
        let mut any_removed = false;

        for attr_match in RE_ATTR.captures_iter(attrs_string) {
            let attr_name = &attr_match[1];
            if allowed_attrs.contains(&attr_name.to_lowercase().as_str()) {
                kept_attrs.push_str(&attr_match[0]);
                kept_attrs.push(' ');
            } else {
                any_removed = true;
            }
        }

        // Also check for valueless attributes (like "disabled", "checked")
        // that weren't caught by the key=value regex
        let kept_trimmed = kept_attrs.trim_end();
        let suffix = if any_removed { " *** " } else { "" };
        if kept_trimmed.is_empty() {
            if any_removed {
                format!("<{} ***>", tag_name)
            } else {
                format!("<{}>", tag_name)
            }
        } else {
            format!("<{} {}{}>", tag_name, kept_trimmed, suffix)
        }
    });

    let mut result = result.to_string();

    // Step 1b: Normalize class attribute values — for each class name containing
    // a hyphen or underscore, keep only the first segment and replace the rest with *.
    // E.g. class="astro-3ii7xxms" → class="astro-*", class="sl-flex astro-wy4te6ga" → class="sl-* astro-*"
    static RE_CLASS_ATTR: Lazy<Regex> = Lazy::new(|| Regex::new(r#"class="([^"]*)""#).unwrap());
    result = RE_CLASS_ATTR
        .replace_all(&result, |caps: &regex::Captures| {
            let class_value = &caps[1];
            let normalized_classes: Vec<String> = class_value
                .split_whitespace()
                .map(|cls| {
                    if let Some(pos) = cls.find(['-', '_']) {
                        format!("{}*", &cls[..=pos])
                    } else {
                        cls.to_string()
                    }
                })
                .collect();
            format!("class=\"{}\"", normalized_classes.join(" "))
        })
        .to_string();

    // Step 2: If result starts with '<', extract only the first HTML tag
    if result.trim_start_matches(&['"', '\'', ' '][..]).starts_with('<') {
        static RE_FIRST_TAG: Lazy<Regex> = Lazy::new(|| Regex::new(r#"(?s)^[\s"']*(<[^>]+>)"#).unwrap());
        if let Some(caps) = RE_FIRST_TAG.captures(&result) {
            result = caps[1].to_string();
        }
    }

    // Step 3: Replace trailing numbers before quotes with ***
    static RE_TRAILING_NUMS: Lazy<Regex> = Lazy::new(|| Regex::new(r#"([0-9]+)(["'])"#).unwrap());
    result = RE_TRAILING_NUMS.replace_all(&result, "***$2").to_string();

    result
}

/// Build aggregation key for a detail (severity + md5 of normalized detail).
fn aggregate_detail_key(severity: &str, detail: &str) -> String {
    use md5::{Digest, Md5};

    let mut clean_detail = detail.to_string();
    // Remove clip-path from SVGs for comparison
    if clean_detail.contains("<svg") {
        use once_cell::sync::Lazy;
        use regex::Regex;
        static RE_CLIPPATH_TAG: Lazy<Regex> = Lazy::new(|| Regex::new(r"(?i)<clipPath[^>]+>").unwrap());
        static RE_CLIPPATH_ATTR: Lazy<Regex> = Lazy::new(|| Regex::new(r#"(?i)clip-path="[^"]+""#).unwrap());
        clean_detail = RE_CLIPPATH_TAG.replace_all(&clean_detail, "").to_string();
        clean_detail = RE_CLIPPATH_ATTR.replace_all(&clean_detail, "").to_string();
    }

    let mut hasher = Md5::new();
    hasher.update(clean_detail.as_bytes());
    let hash = format!("{:x}", hasher.finalize());
    format!("{} | {}", severity, hash)
}

/// JavaScript for image gallery filtering
const IMAGE_GALLERY_FILTER_SCRIPT: &str = r#"<script> function initializeFilters() {
                const links = document.querySelectorAll('#image-gallery a');
                const types = new Set();
                const sources = new Set();
                const sizeCategories = [
                    { label: 'any', filter: () => true },
                    { label: '> 5 MB', filter: size => size > 5 * 1024 * 1024 },
                    { label: '> 1MB', filter: size => size > 1 * 1024 * 1024 },
                    { label: '> 500kB', filter: size => size > 500 * 1024 },
                    { label: '> 100kB', filter: size => size > 100 * 1024 },
                    { label: '> 10kB', filter: size => size > 10 * 1024 },
                    { label: '< 10kB', filter: size => size < 10 * 1024 }
                ];

                links.forEach(link => {
                    types.add(link.dataset.type);
                    sources.add(link.dataset.source);
                });

                addSizeFilters('sizeFilters', sizeCategories, links, filterImagesBySize);
                addToggleButtonsToFilter('typeFilters', ['any'].concat(Array.from(types).sort((a, b) => countLinksOfType(b, links) - countLinksOfType(a, links))), filterImagesByType, links);
                addToggleButtonsToFilter('sourceFilters', ['any'].concat(Array.from(sources).sort((a, b) => countLinksOfSource(b, links) - countLinksOfSource(a, links))), filterImagesBySource, links);
            }

            function addToggleButtonsToFilter(filterId, categories, filterFunction, links) {
                const filterDiv = document.getElementById(filterId);
                categories.forEach((category, index) => {
                    const radioId = filterId + category;
                    const radioInput = document.createElement('input');
                    radioInput.setAttribute('type', 'radio');
                    radioInput.setAttribute('id', radioId);
                    radioInput.setAttribute('name', filterId);
                    radioInput.setAttribute('value', category);
                    if (category === 'any') {
                        radioInput.setAttribute('checked', 'checked');
                    }
                    radioInput.onchange = () => filterFunction(category);

                    const label = document.createElement('label');
                    label.setAttribute('for', radioId);

                    let labelCountText = category;
                    if (category !== 'any') {
                        const count = filterId === 'typeFilters' ? countLinksOfType(category, links) : countLinksOfSource(category, links);
                        labelCountText += ` (${count})`;
                    } else {
                        labelCountText += ' (' + links.length + ')';
                    }
                    label.textContent = labelCountText;

                    filterDiv.appendChild(radioInput);
                    filterDiv.appendChild(label);
                });
            }

            function addToggleButton(filterDiv, filterId, value, labelText, filterFunction) {
                const radioId = filterId + '-' + value.replace(/\s/g, '-');

                const radioInput = document.createElement('input');
                radioInput.setAttribute('type', 'radio');
                radioInput.setAttribute('id', radioId);
                radioInput.setAttribute('name', filterId);
                radioInput.setAttribute('value', value);
                radioInput.addEventListener('change', () => filterFunction(value));

                if (labelText === 'any') {
                    radioInput.setAttribute('checked', 'checked');
                }

                const label = document.createElement('label');
                label.setAttribute('for', radioId);
                label.textContent = labelText;

                filterDiv.appendChild(radioInput);
                filterDiv.appendChild(label);
            }

            function countLinksOfType(type, links) {
                return Array.from(links).filter(link => link.dataset.type === type).length;
            }

            function countLinksOfSource(source, links) {
                return Array.from(links).filter(link => link.dataset.source === source).length;
            }

            function doesSizeMatchCategory(size, category) {
                const sizeInKB = size / 1024;

                switch (category) {
                    case 'any':
                        return true;
                    case '> 5 MB':
                        return sizeInKB > 5120;
                    case '> 1MB':
                        return sizeInKB > 1024;
                    case '> 500kB':
                        return sizeInKB > 500;
                    case '> 100kB':
                        return sizeInKB > 100;
                    case '> 10kB':
                        return sizeInKB > 10;
                    case '< 10kB':
                        return sizeInKB < 10;
                    default:
                        return false;
                }
            }

            function filterImagesByType(selectedType) {
                const links = document.querySelectorAll('#image-gallery a');
                links.forEach(link => {
                    if (selectedType === 'any' || link.dataset.type === selectedType) {
                        link.dataset.typematch = '1';
                    } else {
                        link.dataset.typematch = '0';
                    }
                });
                filterByMatched();
            }

            function filterImagesBySource(selectedSource) {
                const links = document.querySelectorAll('#image-gallery a');
                links.forEach(link => {
                    if (selectedSource === 'any' || link.dataset.source === selectedSource) {
                        link.dataset.sourcematch = '1';
                    } else {
                        link.dataset.sourcematch = '0';
                    }
                });
                filterByMatched();
            }

            function filterImagesBySize(selectedSizeCategory) {
                const links = document.querySelectorAll('#image-gallery a');
                links.forEach(link => {
                    const imageSize = parseInt(link.dataset.size, 10);

                    if (doesSizeMatchCategory(imageSize, selectedSizeCategory)) {
                        link.dataset.sizematch = '1';
                    } else {
                        link.dataset.sizematch = '0';
                    }
                });
                filterByMatched();
            }

            function addSizeFilters(filterId, categories, links, filterFunction) {
                const filterDiv = document.getElementById(filterId);
                categories.forEach(category => {
                    const count = Array.from(links).filter(link => category.filter(parseInt(link.dataset.size, 10))).length;
                    const labelWithCount = `${category.label} (${count})`;
                    if (count > 0) {
                        addToggleButton(filterDiv, filterId, category.label, labelWithCount, filterFunction);
                    }
                });
            }

            function filterByMatched() {
                const links = document.querySelectorAll('#image-gallery a');
                links.forEach(link => {
                    if (link.dataset.sizematch === '1' && link.dataset.typematch === '1' && link.dataset.sourcematch === '1') {
                        link.style.display = 'inline-block'
                    } else {
                        link.style.display = 'none';
                    }
                });
            }

            document.addEventListener('DOMContentLoaded', function() {
                initializeFilters();
            });

            </script>"#;

/// JavaScript for video gallery
const VIDEO_GALLERY_SCRIPT: &str = r#"<script> function playVideos() {
            const videos = document.querySelectorAll("video");

            function playVideoSequentially(index) {
                if (index >= videos.length) return;

                const video = videos[index];
                video.load();
                video.currentTime = 0;

                video.addEventListener("loadeddata", function() {
                    video.play();

                    setTimeout(() => {
                        video.pause();
                        setTimeout(() => playVideoSequentially(index + 1), 10);
                    }, 2000);
                }, { once: true });
            }

            playVideoSequentially(0);
        }

        /* init lazy loading */
        document.addEventListener("DOMContentLoaded", function() {
            const videos = document.querySelectorAll("video");

            const observer = new IntersectionObserver(entries => {
                entries.forEach(entry => {
                    if (entry.isIntersecting) {
                        const video = entry.target;
                        if (!video.src) {
                            video.src = video.dataset.src;
                            video.load();
                        }
                        observer.unobserve(video);
                    }
                });
            });

            videos.forEach(video => {
                observer.observe(video);
            });
        });

        </script>"#;
