// SiteOne Crawler - SeoAndOpenGraphAnalyzer
// (c) Jan Reges <jan.reges@siteone.cz>

use std::collections::HashMap;
use std::time::Instant;

use scraper::{Html, Selector};

use crate::analysis::analyzer::Analyzer;
use crate::analysis::base_analyzer::BaseAnalyzer;
use crate::analysis::result::heading_tree_item::HeadingTreeItem;
use crate::analysis::result::seo_opengraph_result::{ROBOTS_NOINDEX, SeoAndOpenGraphResult};
use crate::components::super_table::SuperTable;
use crate::components::super_table_column::SuperTableColumn;
use crate::output::output::Output;
use crate::result::status::Status;
use crate::result::visited_url::VisitedUrl;
use crate::types::ContentTypeId;
use crate::utils;

const SUPER_TABLE_SEO: &str = "seo";
const SUPER_TABLE_OPEN_GRAPH: &str = "open-graph";
const SUPER_TABLE_SEO_HEADINGS: &str = "seo-headings";

pub struct SeoAndOpenGraphAnalyzer {
    base: BaseAnalyzer,
    max_heading_level: i32,
    has_og_tags: bool,
    has_twitter_tags: bool,
}

impl Default for SeoAndOpenGraphAnalyzer {
    fn default() -> Self {
        Self::new()
    }
}

impl SeoAndOpenGraphAnalyzer {
    pub fn new() -> Self {
        Self {
            base: BaseAnalyzer::new(),
            max_heading_level: 3,
            has_og_tags: false,
            has_twitter_tags: false,
        }
    }

    /// Set configuration from CoreOptions.
    pub fn set_config(&mut self, max_heading_level: i32) {
        self.max_heading_level = max_heading_level;
    }

    fn get_seo_and_opengraph_results(&self, status: &Status) -> Vec<SeoAndOpenGraphResult> {
        let visited_urls = status.get_visited_urls();
        let html_urls: Vec<&VisitedUrl> = visited_urls
            .iter()
            .filter(|u| u.status_code == 200 && u.is_allowed_for_crawling && u.content_type == ContentTypeId::Html)
            .collect();

        let mut results = Vec::new();

        for visited_url in html_urls {
            let html_body = match status.get_url_body_text(&visited_url.uq_id) {
                Some(body) => body,
                None => continue,
            };

            let url_path_and_query = get_url_path_and_query(&visited_url.url);
            let mut url_result = SeoAndOpenGraphResult::new(visited_url.uq_id.clone(), url_path_and_query);

            let document = Html::parse_document(&html_body);
            extract_seo_metadata(&document, &mut url_result);
            extract_opengraph_metadata(&document, &mut url_result);
            extract_twitter_metadata(&document, &mut url_result);
            build_heading_tree(&document, &mut url_result, self.max_heading_level);

            results.push(url_result);
        }

        results
    }

    fn analyze_seo(&self, url_results: &[SeoAndOpenGraphResult], status: &Status, output: &mut dyn Output) {
        let console_width = utils::get_console_width();
        let url_col_width = 50;
        let indexing_col_width = 20;
        let common_col_count = 4;
        let spaces_and_pipes = 6 * 3;
        let common_col_width =
            ((console_width as i32 - url_col_width - indexing_col_width - spaces_and_pipes) / common_col_count).max(10);

        let columns = vec![
            SuperTableColumn::new(
                "urlPathAndQuery".to_string(),
                "URL".to_string(),
                url_col_width,
                None,
                None,
                true,
                false,
                false,
                true,
                None,
            ),
            SuperTableColumn::new(
                "indexing".to_string(),
                "Indexing".to_string(),
                indexing_col_width,
                None,
                Some(Box::new(|row: &HashMap<String, String>, _render_into: &str| {
                    let denied = row.get("deniedByRobotsTxt").map(|v| v == "true").unwrap_or(false);
                    let robots_index = row.get("robotsIndex").and_then(|v| v.parse::<i32>().ok()).unwrap_or(1);

                    if denied {
                        utils::get_color_text("DENY (robots.txt)", "magenta", false)
                    } else if robots_index == ROBOTS_NOINDEX {
                        utils::get_color_text("DENY (meta)", "magenta", false)
                    } else {
                        "Allowed".to_string()
                    }
                })),
                false,
                false,
                false,
                true,
                None,
            ),
            SuperTableColumn::new(
                "title".to_string(),
                "Title".to_string(),
                common_col_width,
                None,
                None,
                true,
                false,
                false,
                true,
                None,
            ),
            SuperTableColumn::new(
                "h1".to_string(),
                "H1".to_string(),
                common_col_width,
                Some(Box::new(|value: &str, _render_into: &str| {
                    if value.is_empty() {
                        utils::get_color_text("Missing H1", "red", true)
                    } else {
                        value.to_string()
                    }
                })),
                None,
                true,
                false,
                false,
                true,
                None,
            ),
            SuperTableColumn::new(
                "description".to_string(),
                "Description".to_string(),
                common_col_width,
                None,
                None,
                true,
                false,
                false,
                true,
                None,
            ),
            SuperTableColumn::new(
                "keywords".to_string(),
                "Keywords".to_string(),
                common_col_width,
                None,
                None,
                true,
                false,
                false,
                true,
                None,
            ),
        ];

        let data = seo_results_to_table_data(url_results);

        let mut super_table = SuperTable::new(
            SUPER_TABLE_SEO.to_string(),
            "SEO metadata".to_string(),
            "No URLs.".to_string(),
            columns,
            true,
            Some("urlPathAndQuery".to_string()),
            "ASC".to_string(),
            None,
            None,
            None,
        );

        super_table.set_visibility_in_console(true, Some(10));
        super_table.set_data(data);
        status.configure_super_table_url_stripping(&mut super_table);
        output.add_super_table(&super_table);
        status.add_super_table_at_beginning(super_table);
    }

    fn analyze_open_graph(&self, url_results: &[SeoAndOpenGraphResult], status: &Status, output: &mut dyn Output) {
        let console_width = utils::get_console_width();
        let url_col_width = 50;
        let image_col_width = 18;
        let image_col_count = (if self.has_og_tags { 1 } else { 0 }) + (if self.has_twitter_tags { 1 } else { 0 });
        let common_col_count = (if self.has_og_tags { 2 } else { 0 }) + (if self.has_twitter_tags { 2 } else { 0 });
        let spaces_and_pipes = (1 + image_col_count + common_col_count) * 3;
        let common_col_width =
            ((console_width as i32 - url_col_width - (image_col_count * image_col_width) - spaces_and_pipes)
                / common_col_count.max(1))
            .max(10);

        let mut columns = vec![SuperTableColumn::new(
            "urlPathAndQuery".to_string(),
            "URL".to_string(),
            url_col_width,
            None,
            None,
            true,
            false,
            false,
            true,
            None,
        )];

        if self.has_og_tags {
            columns.push(SuperTableColumn::new(
                "ogTitle".to_string(),
                "OG Title".to_string(),
                common_col_width,
                None,
                None,
                true,
                false,
                false,
                true,
                None,
            ));
            columns.push(SuperTableColumn::new(
                "ogDescription".to_string(),
                "OG Description".to_string(),
                common_col_width,
                None,
                None,
                true,
                false,
                false,
                true,
                None,
            ));
            columns.push(SuperTableColumn::new(
                "ogImage".to_string(),
                "OG Image".to_string(),
                image_col_width,
                None,
                None,
                true,
                false,
                false,
                true,
                None,
            ));
        }

        if self.has_twitter_tags {
            columns.push(SuperTableColumn::new(
                "twitterTitle".to_string(),
                "Twitter Title".to_string(),
                common_col_width,
                None,
                None,
                true,
                false,
                false,
                true,
                None,
            ));
            columns.push(SuperTableColumn::new(
                "twitterDescription".to_string(),
                "Twitter Description".to_string(),
                common_col_width,
                None,
                None,
                true,
                false,
                false,
                true,
                None,
            ));
            columns.push(SuperTableColumn::new(
                "twitterImage".to_string(),
                "Twitter Image".to_string(),
                image_col_width,
                None,
                None,
                true,
                false,
                false,
                true,
                None,
            ));
        }

        let data = if self.has_og_tags || self.has_twitter_tags {
            og_results_to_table_data(url_results)
        } else {
            Vec::new()
        };

        let mut super_table = SuperTable::new(
            SUPER_TABLE_OPEN_GRAPH.to_string(),
            "OpenGraph metadata".to_string(),
            "No URLs with OpenGraph data (og:* or twitter:* meta tags).".to_string(),
            columns,
            true,
            Some("urlPathAndQuery".to_string()),
            "ASC".to_string(),
            None,
            None,
            None,
        );

        super_table.set_visibility_in_console(true, Some(10));
        super_table.set_data(data);
        status.configure_super_table_url_stripping(&mut super_table);
        output.add_super_table(&super_table);
        status.add_super_table_at_beginning(super_table);
    }

    fn analyze_headings(&self, url_results: &[SeoAndOpenGraphResult], status: &Status, output: &mut dyn Output) {
        let console_width = utils::get_console_width();
        let url_col_width = 30;
        let heading_col_width = (console_width as i32 - url_col_width - 24).max(20);

        let columns = vec![
            SuperTableColumn::new(
                "headings".to_string(),
                "Heading structure".to_string(),
                heading_col_width,
                None,
                Some(Box::new(|row: &HashMap<String, String>, render_into: &str| {
                    if render_into == "html" {
                        row.get("headingsHtml").cloned().unwrap_or_default()
                    } else {
                        row.get("headings").cloned().unwrap_or_default()
                    }
                })),
                true,
                false,
                false,
                false,
                None,
            ),
            SuperTableColumn::new(
                "headingsCount".to_string(),
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
                "headingsErrorsCount".to_string(),
                "Errors".to_string(),
                6,
                Some(Box::new(|value: &str, _render_into: &str| {
                    if let Ok(v) = value.parse::<usize>() {
                        if v > 0 {
                            return utils::get_color_text(&v.to_string(), "red", true);
                        }
                        return utils::get_color_text(&v.to_string(), "green", true);
                    }
                    value.to_string()
                })),
                None,
                false,
                false,
                false,
                true,
                None,
            ),
            SuperTableColumn::new(
                "urlPathAndQuery".to_string(),
                "URL".to_string(),
                url_col_width,
                None,
                None,
                true,
                false,
                false,
                true,
                None,
            ),
        ];

        let data = headings_to_table_data(url_results);

        let mut super_table = SuperTable::new(
            SUPER_TABLE_SEO_HEADINGS.to_string(),
            "Heading structure".to_string(),
            "No URLs to analyze heading structure.".to_string(),
            columns,
            true,
            Some("headingsErrorsCount".to_string()),
            "DESC".to_string(),
            None,
            None,
            None,
        );

        super_table.set_visibility_in_console(true, Some(10));
        super_table.set_data(data);
        status.configure_super_table_url_stripping(&mut super_table);
        output.add_super_table(&super_table);
        status.add_super_table_at_beginning(super_table);
    }
}

impl Analyzer for SeoAndOpenGraphAnalyzer {
    fn analyze(&mut self, status: &Status, output: &mut dyn Output) {
        let url_results = self.get_seo_and_opengraph_results(status);

        // Check for OG and Twitter tags
        for r in &url_results {
            if self.has_og_tags && self.has_twitter_tags {
                break;
            }
            if r.og_title.is_some() || r.og_description.is_some() || r.og_image.is_some() {
                self.has_og_tags = true;
            }
            if r.twitter_card.is_some()
                || r.twitter_title.is_some()
                || r.twitter_description.is_some()
                || r.twitter_image.is_some()
            {
                self.has_twitter_tags = true;
            }
        }

        let s = Instant::now();
        self.analyze_seo(&url_results, status, output);
        self.base.measure_exec_time("SeoAndOpenGraphAnalyzer", "analyzeSeo", s);

        let s = Instant::now();
        self.analyze_open_graph(&url_results, status, output);
        self.base
            .measure_exec_time("SeoAndOpenGraphAnalyzer", "analyzeOpenGraph", s);

        let s = Instant::now();
        self.analyze_headings(&url_results, status, output);
        self.base
            .measure_exec_time("SeoAndOpenGraphAnalyzer", "analyzeHeadings", s);
    }

    fn should_be_activated(&self) -> bool {
        true
    }

    fn get_order(&self) -> i32 {
        113
    }

    fn get_name(&self) -> &str {
        "SeoAndOpenGraphAnalyzer"
    }

    fn get_exec_times(&self) -> &HashMap<String, f64> {
        self.base.get_exec_times()
    }

    fn get_exec_counts(&self) -> &HashMap<String, usize> {
        self.base.get_exec_counts()
    }
}

fn get_url_path_and_query(url: &str) -> String {
    if let Ok(parsed) = url::Url::parse(url) {
        let path = parsed.path().to_string();
        if let Some(query) = parsed.query() {
            format!("{}?{}", path, query)
        } else {
            path
        }
    } else {
        url.to_string()
    }
}

fn extract_seo_metadata(document: &Html, result: &mut SeoAndOpenGraphResult) {
    // Title
    if let Ok(sel) = Selector::parse("title")
        && let Some(el) = document.select(&sel).next()
    {
        let text = el.text().collect::<String>().trim().to_string();
        if !text.is_empty() {
            result.title = Some(text);
        }
    }

    // Meta description
    if let Ok(sel) = Selector::parse("meta[name='description']")
        && let Some(el) = document.select(&sel).next()
        && let Some(content) = el.value().attr("content")
    {
        result.description = Some(content.to_string());
    }

    // Meta keywords
    if let Ok(sel) = Selector::parse("meta[name='keywords']")
        && let Some(el) = document.select(&sel).next()
        && let Some(content) = el.value().attr("content")
    {
        result.keywords = Some(content.to_string());
    }

    // H1
    if let Ok(sel) = Selector::parse("h1")
        && let Some(el) = document.select(&sel).next()
    {
        let text = el.text().collect::<String>().trim().to_string();
        if !text.is_empty() {
            result.h1 = Some(text);
        }
    }

    // Robots meta
    if let Ok(sel) = Selector::parse("meta[name='robots']")
        && let Some(el) = document.select(&sel).next()
        && let Some(content) = el.value().attr("content")
    {
        let content_lower = content.to_lowercase();
        if content_lower.contains("noindex") {
            result.robots_index = Some(ROBOTS_NOINDEX);
        }
        if content_lower.contains("nofollow") {
            result.robots_follow = Some(crate::analysis::result::seo_opengraph_result::ROBOTS_NOFOLLOW);
        }
    }
}

fn extract_opengraph_metadata(document: &Html, result: &mut SeoAndOpenGraphResult) {
    // Extract OG tags
    if let Ok(sel) = Selector::parse("meta[property='og:title']")
        && let Some(el) = document.select(&sel).next()
    {
        result.og_title = el.value().attr("content").map(|s| s.to_string());
    }
    if let Ok(sel) = Selector::parse("meta[property='og:description']")
        && let Some(el) = document.select(&sel).next()
    {
        result.og_description = el.value().attr("content").map(|s| s.to_string());
    }
    if let Ok(sel) = Selector::parse("meta[property='og:image']")
        && let Some(el) = document.select(&sel).next()
    {
        result.og_image = el.value().attr("content").map(|s| s.to_string());
    }
    if let Ok(sel) = Selector::parse("meta[property='og:url']")
        && let Some(el) = document.select(&sel).next()
    {
        result.og_url = el.value().attr("content").map(|s| s.to_string());
    }
    if let Ok(sel) = Selector::parse("meta[property='og:type']")
        && let Some(el) = document.select(&sel).next()
    {
        result.og_type = el.value().attr("content").map(|s| s.to_string());
    }
    if let Ok(sel) = Selector::parse("meta[property='og:site_name']")
        && let Some(el) = document.select(&sel).next()
    {
        result.og_site_name = el.value().attr("content").map(|s| s.to_string());
    }
}

fn extract_twitter_metadata(document: &Html, result: &mut SeoAndOpenGraphResult) {
    if let Ok(sel) = Selector::parse("meta[name='twitter:card']")
        && let Some(el) = document.select(&sel).next()
    {
        result.twitter_card = el.value().attr("content").map(|s| s.to_string());
    }
    if let Ok(sel) = Selector::parse("meta[name='twitter:site']")
        && let Some(el) = document.select(&sel).next()
    {
        result.twitter_site = el.value().attr("content").map(|s| s.to_string());
    }
    if let Ok(sel) = Selector::parse("meta[name='twitter:creator']")
        && let Some(el) = document.select(&sel).next()
    {
        result.twitter_creator = el.value().attr("content").map(|s| s.to_string());
    }
    if let Ok(sel) = Selector::parse("meta[name='twitter:title']")
        && let Some(el) = document.select(&sel).next()
    {
        result.twitter_title = el.value().attr("content").map(|s| s.to_string());
    }
    if let Ok(sel) = Selector::parse("meta[name='twitter:description']")
        && let Some(el) = document.select(&sel).next()
    {
        result.twitter_description = el.value().attr("content").map(|s| s.to_string());
    }
    if let Ok(sel) = Selector::parse("meta[name='twitter:image']")
        && let Some(el) = document.select(&sel).next()
    {
        result.twitter_image = el.value().attr("content").map(|s| s.to_string());
    }
}

fn build_heading_tree(document: &Html, result: &mut SeoAndOpenGraphResult, max_level: i32) {
    let selector = match Selector::parse("h1, h2, h3, h4, h5, h6") {
        Ok(s) => s,
        Err(_) => return,
    };

    let headings: Vec<(i32, String, Option<String>)> = document
        .select(&selector)
        .filter_map(|el| {
            let tag = el.value().name();
            let level = tag.strip_prefix('h').and_then(|s| s.parse::<i32>().ok())?;
            if level > max_level {
                return None;
            }
            let text = el.text().collect::<String>().trim().to_string();
            // Strip JS from text
            let text = text.split('\n').map(|l| l.trim()).collect::<Vec<_>>().join(" ");
            use once_cell::sync::Lazy;
            static RE_WS: Lazy<regex::Regex> = Lazy::new(|| regex::Regex::new(r"\s+").unwrap());
            let text = RE_WS.replace_all(&text, " ").trim().to_string();
            let id = el.value().attr("id").map(|s| s.to_string());
            Some((level, text, id))
        })
        .collect();

    if headings.is_empty() {
        return;
    }

    // Build tree structure: use a root node at level 0 and insert children based on heading levels

    let mut items: Vec<Option<HeadingTreeItem>> = headings
        .iter()
        .map(|(level, text, id)| Some(HeadingTreeItem::new(*level, text.clone(), id.clone())))
        .collect();

    // Compute parent relationships using a stack
    let headings_ref: Vec<(i32, Option<usize>)> = {
        let mut result_vec = Vec::new();
        let mut stack2: Vec<(i32, usize)> = Vec::new(); // (level, index)
        for (idx, (level, _text, _id)) in headings.iter().enumerate() {
            while let Some(&(top_level, _)) = stack2.last() {
                if top_level >= *level {
                    stack2.pop();
                } else {
                    break;
                }
            }
            let parent_idx = stack2.last().map(|&(_, idx)| idx);
            result_vec.push((*level, parent_idx));
            stack2.push((*level, idx));
        }
        result_vec
    };

    // Build tree bottom-up
    for idx in (0..items.len()).rev() {
        if let Some(parent_idx) = headings_ref[idx].1
            && let Some(child) = items[idx].take()
            && let Some(ref mut parent) = items[parent_idx]
        {
            parent.children.insert(0, child);
        }
    }

    // Collect root items (those without parents)
    let mut root_children: Vec<HeadingTreeItem> = items
        .into_iter()
        .enumerate()
        .filter(|(idx, _)| headings_ref[*idx].1.is_none())
        .filter_map(|(_, item)| item)
        .collect();

    // Set error for multiple H1s
    let h1_count = Selector::parse("h1").map(|s| document.select(&s).count()).unwrap_or(0);
    if h1_count > 1 {
        fn mark_h1_errors(items: &mut [HeadingTreeItem], h1_count: usize) {
            for item in items.iter_mut() {
                if item.level == 1 {
                    item.error_text = Some(format!("Multiple H1s ({}) found.", h1_count));
                }
                mark_h1_errors(&mut item.children, h1_count);
            }
        }
        mark_h1_errors(&mut root_children, h1_count);
    }

    // Set real_level and check for level mismatches
    fn fix_real_levels(items: &mut [HeadingTreeItem], real_level: i32) {
        for item in items.iter_mut() {
            item.real_level = Some(real_level);
            if item.level != real_level && item.error_text.is_none() {
                item.error_text = Some(format!(
                    "Heading level {} is not correct. Should be {}.",
                    item.level, real_level
                ));
            }
            fix_real_levels(&mut item.children, real_level + 1);
        }
    }
    fix_real_levels(&mut root_children, 1);

    let total_count = HeadingTreeItem::get_headings_count(&root_children);
    let errors_count = HeadingTreeItem::get_headings_with_error_count(&root_children);

    result.heading_tree_items = root_children;
    result.headings_count = total_count;
    result.headings_errors_count = errors_count;
}

fn seo_results_to_table_data(results: &[SeoAndOpenGraphResult]) -> Vec<HashMap<String, String>> {
    results
        .iter()
        .map(|r| {
            let mut row = HashMap::new();
            row.insert("urlPathAndQuery".to_string(), r.url_path_and_query.clone());
            row.insert("title".to_string(), r.title.clone().unwrap_or_default());
            row.insert("h1".to_string(), r.h1.clone().unwrap_or_default());
            row.insert("description".to_string(), r.description.clone().unwrap_or_default());
            row.insert("keywords".to_string(), r.keywords.clone().unwrap_or_default());
            row.insert("deniedByRobotsTxt".to_string(), r.denied_by_robots_txt.to_string());
            row.insert("robotsIndex".to_string(), r.robots_index.unwrap_or(1).to_string());
            row.insert(
                "indexing".to_string(),
                String::new(), // Will be rendered by renderer
            );
            row
        })
        .collect()
}

fn og_results_to_table_data(results: &[SeoAndOpenGraphResult]) -> Vec<HashMap<String, String>> {
    results
        .iter()
        .map(|r| {
            let mut row = HashMap::new();
            row.insert("urlPathAndQuery".to_string(), r.url_path_and_query.clone());
            row.insert("ogTitle".to_string(), r.og_title.clone().unwrap_or_default());
            row.insert(
                "ogDescription".to_string(),
                r.og_description.clone().unwrap_or_default(),
            );
            row.insert("ogImage".to_string(), r.og_image.clone().unwrap_or_default());
            row.insert("twitterTitle".to_string(), r.twitter_title.clone().unwrap_or_default());
            row.insert(
                "twitterDescription".to_string(),
                r.twitter_description.clone().unwrap_or_default(),
            );
            row.insert("twitterImage".to_string(), r.twitter_image.clone().unwrap_or_default());
            row
        })
        .collect()
}

fn headings_to_table_data(results: &[SeoAndOpenGraphResult]) -> Vec<HashMap<String, String>> {
    results
        .iter()
        .map(|r| {
            let mut row = HashMap::new();
            row.insert("urlPathAndQuery".to_string(), r.url_path_and_query.clone());
            row.insert(
                "headings".to_string(),
                HeadingTreeItem::get_heading_tree_txt_list(&r.heading_tree_items),
            );
            row.insert(
                "headingsHtml".to_string(),
                HeadingTreeItem::get_heading_tree_ul_li_list(&r.heading_tree_items),
            );
            row.insert("headingsCount".to_string(), r.headings_count.to_string());
            row.insert("headingsErrorsCount".to_string(), r.headings_errors_count.to_string());
            row
        })
        .collect()
}
