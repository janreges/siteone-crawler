// SiteOne Crawler - AccessibilityAnalyzer
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

const ANALYSIS_MISSING_IMAGE_ALT_ATTRIBUTES: &str = "Missing image alt attributes";
const ANALYSIS_MISSING_FORM_LABELS: &str = "Missing form labels";
const ANALYSIS_MISSING_ARIA_LABELS: &str = "Missing aria labels";
const ANALYSIS_MISSING_ROLES: &str = "Missing roles";
const ANALYSIS_MISSING_LANG_ATTRIBUTE: &str = "Missing html lang attribute";

const SUPER_TABLE_ACCESSIBILITY: &str = "accessibility";

pub struct AccessibilityAnalyzer {
    base: BaseAnalyzer,
    stats: AnalyzerStats,

    pages_with_invalid_html: usize,
    pages_without_image_alt_attributes: usize,
    pages_without_form_labels: usize,
    pages_without_aria_labels: usize,
    pages_without_roles: usize,
    pages_without_lang: usize,
}

impl Default for AccessibilityAnalyzer {
    fn default() -> Self {
        Self::new()
    }
}

impl AccessibilityAnalyzer {
    pub fn new() -> Self {
        Self {
            base: BaseAnalyzer::new(),
            stats: AnalyzerStats::new(),
            pages_with_invalid_html: 0,
            pages_without_image_alt_attributes: 0,
            pages_without_form_labels: 0,
            pages_without_aria_labels: 0,
            pages_without_roles: 0,
            pages_without_lang: 0,
        }
    }

    fn check_image_alt_attributes(&mut self, html: &str, result: &mut UrlAnalysisResult) {
        use once_cell::sync::Lazy;
        static RE_IMG: Lazy<Regex> = Lazy::new(|| Regex::new(r"(?i)<img[^>]+>").unwrap());
        let img_re = &*RE_IMG;

        let mut bad_images: Vec<String> = Vec::new();
        let mut found_count = 0usize;

        for mat in img_re.find_iter(html) {
            found_count += 1;
            let img = mat.as_str();
            let img_lower = img.to_lowercase();

            if !img_lower.contains(" alt=") || img_lower.contains(" alt=\"\"") || img_lower.contains(" alt=''") {
                bad_images.push(img.to_string());
                self.stats.add_warning(ANALYSIS_MISSING_IMAGE_ALT_ATTRIBUTES, Some(img));
            } else {
                self.stats.add_ok(ANALYSIS_MISSING_IMAGE_ALT_ATTRIBUTES, Some(img));
            }
        }

        if !bad_images.is_empty() {
            result.add_warning(
                format!("{} image(s) without 'alt' attribute", bad_images.len()),
                ANALYSIS_MISSING_IMAGE_ALT_ATTRIBUTES,
                Some(bad_images),
            );
            self.pages_without_image_alt_attributes += 1;
        } else {
            result.add_ok(
                format!("All {} image(s) have an 'alt' attribute", found_count),
                ANALYSIS_MISSING_IMAGE_ALT_ATTRIBUTES,
                None,
            );
        }
    }

    fn check_missing_labels(&mut self, html: &str, result: &mut UrlAnalysisResult) {
        let document = Html::parse_document(html);

        let input_selector = match Selector::parse("input:not([type='hidden'])") {
            Ok(s) => s,
            Err(_) => return,
        };
        let label_selector_fn =
            |id: &str| -> Option<Selector> { Selector::parse(&format!("label[for='{}']", id)).ok() };

        let inputs: Vec<_> = document.select(&input_selector).collect();
        let mut inputs_without_labels: Vec<String> = Vec::new();

        for input in &inputs {
            let input_html = get_opening_tag_html(input);
            let dedup_key = normalize_tag_for_dedup(input);
            let id = input.value().attr("id");

            if let Some(id_val) = id {
                if let Some(label_sel) = label_selector_fn(id_val)
                    && document.select(&label_sel).next().is_none()
                {
                    inputs_without_labels.push(input_html);
                    self.stats.add_warning(ANALYSIS_MISSING_FORM_LABELS, Some(&dedup_key));
                }
            } else {
                inputs_without_labels.push(input_html);
                self.stats.add_warning(ANALYSIS_MISSING_FORM_LABELS, Some(&dedup_key));
            }
        }

        if !inputs_without_labels.is_empty() {
            result.add_warning(
                format!("{} input(s) without associated <label>", inputs_without_labels.len()),
                ANALYSIS_MISSING_FORM_LABELS,
                Some(inputs_without_labels),
            );
            self.pages_without_form_labels += 1;
        } else if !inputs.is_empty() {
            result.add_ok(
                format!("All {} input(s) have associated 'label'", inputs.len()),
                ANALYSIS_MISSING_FORM_LABELS,
                None,
            );
        }
    }

    fn check_missing_aria_labels(&mut self, html: &str, result: &mut UrlAnalysisResult) {
        let document = Html::parse_document(html);

        let mut critical_elements_without: Vec<String> = Vec::new();
        let critical_selectors = ["input:not([type='hidden'])", "select", "textarea"];

        for sel_str in &critical_selectors {
            let selector = match Selector::parse(sel_str) {
                Ok(s) => s,
                Err(_) => continue,
            };

            for element in document.select(&selector) {
                let element_html = get_opening_tag_html(&element);
                let dedup_key = normalize_tag_for_dedup(&element);

                let has_aria_label = element.value().attr("aria-label").is_some();
                let has_aria_labelledby = element.value().attr("aria-labelledby").is_some();

                if !has_aria_label && !has_aria_labelledby {
                    critical_elements_without.push(element_html);
                    self.stats.add_critical(ANALYSIS_MISSING_ARIA_LABELS, Some(&dedup_key));
                } else {
                    self.stats.add_ok(ANALYSIS_MISSING_ARIA_LABELS, Some(&dedup_key));
                }
            }
        }

        let mut warning_elements_without: Vec<String> = Vec::new();
        let warning_selectors = ["a", "button"];

        for sel_str in &warning_selectors {
            let selector = match Selector::parse(sel_str) {
                Ok(s) => s,
                Err(_) => continue,
            };

            for element in document.select(&selector) {
                let element_html = get_opening_tag_html(&element);
                let dedup_key = normalize_tag_for_dedup(&element);

                let has_aria_label = element.value().attr("aria-label").is_some();
                let has_aria_labelledby = element.value().attr("aria-labelledby").is_some();

                if !has_aria_label && !has_aria_labelledby {
                    warning_elements_without.push(element_html);
                    self.stats.add_warning(ANALYSIS_MISSING_ARIA_LABELS, Some(&dedup_key));
                } else {
                    self.stats.add_ok(ANALYSIS_MISSING_ARIA_LABELS, Some(&dedup_key));
                }
            }
        }

        if !critical_elements_without.is_empty() {
            result.add_critical(
                format!(
                    "{} form element(s) without defined 'aria-label' or 'aria-labelledby'",
                    critical_elements_without.len()
                ),
                ANALYSIS_MISSING_ARIA_LABELS,
                Some(critical_elements_without.clone()),
            );
        }
        if !warning_elements_without.is_empty() {
            result.add_warning(
                format!(
                    "{} element(s) without defined 'aria-label' or 'aria-labelledby'",
                    warning_elements_without.len()
                ),
                ANALYSIS_MISSING_ARIA_LABELS,
                Some(warning_elements_without.clone()),
            );
        }

        if !critical_elements_without.is_empty() || !warning_elements_without.is_empty() {
            self.pages_without_aria_labels += 1;
        } else {
            result.add_ok(
                "All key interactive element(s) have defined 'aria-label' or 'aria-labelledby'".to_string(),
                ANALYSIS_MISSING_ARIA_LABELS,
                None,
            );
        }
    }

    fn check_missing_roles(&mut self, html: &str, result: &mut UrlAnalysisResult) {
        let document = Html::parse_document(html);

        let mut elements_without_roles: Vec<String> = Vec::new();
        let elements_to_check = ["nav", "main", "aside", "header", "footer"];

        for sel_str in &elements_to_check {
            let selector = match Selector::parse(sel_str) {
                Ok(s) => s,
                Err(_) => continue,
            };

            for element in document.select(&selector) {
                if element.value().attr("role").is_some() {
                    continue;
                }
                let element_html = get_opening_tag_html(&element);
                let dedup_key = normalize_tag_for_dedup(&element);
                elements_without_roles.push(element_html);
                self.stats.add_warning(ANALYSIS_MISSING_ROLES, Some(&dedup_key));
            }
        }

        if !elements_without_roles.is_empty() {
            result.add_warning(
                format!("{} element(s) without defined 'role'", elements_without_roles.len()),
                ANALYSIS_MISSING_ROLES,
                Some(elements_without_roles),
            );
            self.pages_without_roles += 1;
        } else {
            result.add_ok(
                "All key element(s) have defined 'role'".to_string(),
                ANALYSIS_MISSING_ROLES,
                None,
            );
        }
    }

    fn check_missing_lang(&mut self, html: &str, result: &mut UrlAnalysisResult) {
        let document = Html::parse_document(html);

        let html_selector = match Selector::parse("html") {
            Ok(s) => s,
            Err(_) => return,
        };

        if let Some(html_el) = document.select(&html_selector).next() {
            if let Some(lang) = html_el.value().attr("lang") {
                let element_html = format!("<html lang=\"{}\">", lang);
                if lang.is_empty() {
                    result.add_critical(
                        "The 'lang' attribute is present in <html> but empty.".to_string(),
                        ANALYSIS_MISSING_LANG_ATTRIBUTE,
                        Some(vec!["HTML lang attribute value is empty ''.".to_string()]),
                    );
                    self.stats
                        .add_critical(ANALYSIS_MISSING_LANG_ATTRIBUTE, Some(&element_html));
                    self.pages_without_lang += 1;
                } else {
                    result.add_ok(
                        format!("Document has defined 'lang' attribute as '{}'.", lang),
                        ANALYSIS_MISSING_LANG_ATTRIBUTE,
                        None,
                    );
                    self.stats.add_ok(ANALYSIS_MISSING_LANG_ATTRIBUTE, Some(&element_html));
                }
            } else {
                result.add_critical(
                    "Document does not have a defined 'lang' attribute in <html>.".to_string(),
                    ANALYSIS_MISSING_LANG_ATTRIBUTE,
                    Some(vec!["HTML lang attribute is not present.".to_string()]),
                );
                self.stats.add_critical(ANALYSIS_MISSING_LANG_ATTRIBUTE, Some("<html>"));
                self.pages_without_lang += 1;
            }
        } else {
            result.add_critical(
                "Document does not have a defined 'lang' attribute in <html>.".to_string(),
                ANALYSIS_MISSING_LANG_ATTRIBUTE,
                Some(vec!["HTML lang attribute is not present.".to_string()]),
            );
            self.stats.add_critical(ANALYSIS_MISSING_LANG_ATTRIBUTE, Some("<html>"));
            self.pages_without_lang += 1;
        }
    }

    fn set_findings_to_summary(&self, status: &Status) {
        if self.pages_with_invalid_html > 0 {
            status.add_critical_to_summary(
                "pages-with-invalid-html",
                &format!("{} page(s) with invalid HTML", self.pages_with_invalid_html),
            );
        } else {
            status.add_ok_to_summary("pages-with-invalid-html", "All pages have valid HTML");
        }

        if self.pages_without_image_alt_attributes > 0 {
            status.add_warning_to_summary(
                "pages-without-image-alt-attributes",
                &format!(
                    "{} page(s) without image alt attributes",
                    self.pages_without_image_alt_attributes
                ),
            );
        } else {
            status.add_ok_to_summary(
                "pages-without-image-alt-attributes",
                "All pages have image alt attributes",
            );
        }

        if self.pages_without_form_labels > 0 {
            status.add_warning_to_summary(
                "pages-without-form-labels",
                &format!("{} page(s) without form labels", self.pages_without_form_labels),
            );
        } else {
            status.add_ok_to_summary("pages-without-form-labels", "All pages have form labels");
        }

        if self.pages_without_aria_labels > 0 {
            status.add_warning_to_summary(
                "pages-without-aria-labels",
                &format!("{} page(s) without aria labels", self.pages_without_aria_labels),
            );
        } else {
            status.add_ok_to_summary("pages-without-aria-labels", "All pages have aria labels");
        }

        if self.pages_without_roles > 0 {
            status.add_warning_to_summary(
                "pages-without-roles",
                &format!("{} page(s) without role attributes", self.pages_without_roles),
            );
        } else {
            status.add_ok_to_summary("pages-without-roles", "All pages have role attributes");
        }

        if self.pages_without_lang > 0 {
            status.add_critical_to_summary(
                "pages-without-lang",
                &format!("{} page(s) without lang attribute", self.pages_without_lang),
            );
        } else {
            status.add_ok_to_summary("pages-without-lang", "All pages have lang attribute");
        }
    }
}

impl Analyzer for AccessibilityAnalyzer {
    fn analyze(&mut self, status: &Status, output: &mut dyn Output) {
        let columns = vec![
            SuperTableColumn::new(
                "analysisName".to_string(),
                "Analysis name".to_string(),
                -1, // AUTO_WIDTH
                None,
                None,
                false,
                false,
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
                true,
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
                true,
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
                true,
                false,
                true,
                None,
            ),
        ];

        let data = self.stats.to_table_data();

        let mut super_table = SuperTable::new(
            SUPER_TABLE_ACCESSIBILITY.to_string(),
            "Accessibility".to_string(),
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
        let is_html = visited_url.content_type == ContentTypeId::Html
            && visited_url.status_code == 200
            && visited_url.is_allowed_for_crawling;

        if !is_html {
            return None;
        }

        let html = body?;
        let mut result = UrlAnalysisResult::new();

        let s = Instant::now();
        self.check_image_alt_attributes(html, &mut result);
        self.base
            .measure_exec_time("AccessibilityAnalyzer", "checkImageAltAttributes", s);

        let s = Instant::now();
        self.check_missing_labels(html, &mut result);
        self.base
            .measure_exec_time("AccessibilityAnalyzer", "checkMissingLabels", s);

        let s = Instant::now();
        self.check_missing_aria_labels(html, &mut result);
        self.base
            .measure_exec_time("AccessibilityAnalyzer", "checkMissingAriaLabels", s);

        let s = Instant::now();
        self.check_missing_roles(html, &mut result);
        self.base
            .measure_exec_time("AccessibilityAnalyzer", "checkMissingRoles", s);

        let s = Instant::now();
        self.check_missing_lang(html, &mut result);
        self.base
            .measure_exec_time("AccessibilityAnalyzer", "checkMissingLang", s);

        Some(result)
    }

    fn show_analyzed_visited_url_result_as_column(&self) -> Option<ExtraColumn> {
        ExtraColumn::new("Access.".to_string(), Some(8), false, None, None, None).ok()
    }

    fn should_be_activated(&self) -> bool {
        true
    }

    fn get_order(&self) -> i32 {
        175
    }

    fn get_name(&self) -> &str {
        "AccessibilityAnalyzer"
    }

    fn get_exec_times(&self) -> &HashMap<String, f64> {
        self.base.get_exec_times()
    }

    fn get_exec_counts(&self) -> &HashMap<String, usize> {
        self.base.get_exec_counts()
    }
}

/// Get the opening tag HTML from an element reference (strip inner content).
/// Built directly from element name + attributes to avoid html5ever serialization
/// panics on malformed HTML ("no parent ElemInfo").
fn get_opening_tag_html(element: &scraper::ElementRef) -> String {
    let name = element.value().name();
    let attrs: Vec<String> = element
        .value()
        .attrs()
        .map(|(k, v)| format!("{}=\"{}\"", k, v))
        .collect();
    if attrs.is_empty() {
        format!("<{}>", name)
    } else {
        format!("<{} {}>", name, attrs.join(" "))
    }
}

/// Normalize an opening tag for deduplication purposes.
/// Replaces dynamic attribute values (href, src, action, id, class, style, data-*)
/// with "*" so that structurally identical elements on different pages
/// (e.g. same nav `<a>` with different href) are counted only once.
fn normalize_tag_for_dedup(element: &scraper::ElementRef) -> String {
    let name = element.value().name();
    let attrs: Vec<String> = element
        .value()
        .attrs()
        .map(|(k, v)| {
            if k == "href"
                || k == "src"
                || k == "action"
                || k == "id"
                || k == "class"
                || k == "style"
                || k == "for"
                || k.starts_with("data-")
            {
                format!("{}=\"*\"", k)
            } else {
                format!("{}=\"{}\"", k, v)
            }
        })
        .collect();
    if attrs.is_empty() {
        format!("<{}>", name)
    } else {
        format!("<{} {}>", name, attrs.join(" "))
    }
}
