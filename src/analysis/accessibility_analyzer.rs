// SiteOne Crawler - AccessibilityAnalyzer
// (c) Jan Reges <jan.reges@siteone.cz>

use std::collections::HashMap;
use std::time::Instant;

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
const ANALYSIS_UNNAMED_INTERACTIVE: &str = "Unnamed links/buttons";
const ANALYSIS_MAIN_LANDMARK: &str = "Missing main landmark";
const ANALYSIS_MISSING_LANG_ATTRIBUTE: &str = "Missing html lang attribute";
const ANALYSIS_HTML_STRUCTURE: &str = "HTML structural issues";

/// Every check this analyzer reports, in the order the HTML report lists their detail tables.
pub(crate) const ANALYSIS_NAMES: [&str; 6] = [
    ANALYSIS_HTML_STRUCTURE,
    ANALYSIS_MISSING_IMAGE_ALT_ATTRIBUTES,
    ANALYSIS_MISSING_FORM_LABELS,
    ANALYSIS_UNNAMED_INTERACTIVE,
    ANALYSIS_MAIN_LANDMARK,
    ANALYSIS_MISSING_LANG_ATTRIBUTE,
];

const SUPER_TABLE_ACCESSIBILITY: &str = "accessibility";

pub struct AccessibilityAnalyzer {
    base: BaseAnalyzer,
    stats: AnalyzerStats,

    /// Pages with parser-impact structural defects (duplicate id, dangling ARIA/`for` references).
    pages_with_structural_issues: usize,
    pages_without_image_alt_attributes: usize,
    pages_without_form_labels: usize,
    pages_without_aria_labels: usize,
    pages_without_main_landmark: usize,
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
            pages_with_structural_issues: 0,
            pages_without_image_alt_attributes: 0,
            pages_without_form_labels: 0,
            pages_without_aria_labels: 0,
            pages_without_main_landmark: 0,
            pages_without_lang: 0,
        }
    }

    fn check_image_alt_attributes(&mut self, document: &Html, result: &mut UrlAnalysisResult) {
        let img_selector = match Selector::parse("img") {
            Ok(s) => s,
            Err(_) => return,
        };

        let mut bad_images: Vec<String> = Vec::new();
        let mut found_count = 0usize;

        for img in document.select(&img_selector) {
            found_count += 1;
            // Only a completely missing alt attribute is a defect. An explicit alt="" is the VALID
            // decorative pattern per WCAG 1.1.1 and must NOT be flagged. Using the parsed DOM avoids
            // the regex pitfalls (e.g. " alt=" appearing inside another attribute value).
            if img.value().attr("alt").is_none() {
                let tag = get_opening_tag_html(&img);
                self.stats
                    .add_warning(ANALYSIS_MISSING_IMAGE_ALT_ATTRIBUTES, Some(&tag));
                bad_images.push(tag);
            } else {
                self.stats.add_ok(
                    ANALYSIS_MISSING_IMAGE_ALT_ATTRIBUTES,
                    Some(&normalize_tag_for_dedup(&img)),
                );
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

    fn check_missing_labels(&mut self, document: &Html, result: &mut UrlAnalysisResult) {
        // Controls that need a programmatic label. Buttons/submit/reset/image derive their name
        // from value=/alt=, so they are excluded here, and so are controls hidden from assistive
        // technology (axe's `label` rule skips them too).
        let input_selector = match Selector::parse(
            "input:not([type='hidden']):not([type='submit']):not([type='button']):not([type='reset']):not([type='image']), select, textarea",
        ) {
            Ok(s) => s,
            Err(_) => return,
        };

        let inputs: Vec<_> = document
            .select(&input_selector)
            .filter(|input| !is_hidden_from_assistive_tech(input))
            .collect();
        let mut inputs_without_labels: Vec<String> = Vec::new();

        for input in &inputs {
            let dedup_key = normalize_tag_for_dedup(input);
            // A control is labeled by ANY of: a <label for=id> or wrapping <label> with text,
            // aria-label, aria-labelledby, title, or (input/textarea) a non-empty placeholder.
            if input_is_labeled(input, document) {
                self.stats.add_ok(ANALYSIS_MISSING_FORM_LABELS, Some(&dedup_key));
            } else {
                inputs_without_labels.push(get_opening_tag_html(input));
                self.stats.add_warning(ANALYSIS_MISSING_FORM_LABELS, Some(&dedup_key));
            }
        }

        if !inputs_without_labels.is_empty() {
            result.add_warning(
                format!(
                    "{} form control(s) without an accessible label",
                    inputs_without_labels.len()
                ),
                ANALYSIS_MISSING_FORM_LABELS,
                Some(inputs_without_labels),
            );
            self.pages_without_form_labels += 1;
        } else if !inputs.is_empty() {
            result.add_ok(
                format!("All {} form control(s) have an accessible label", inputs.len()),
                ANALYSIS_MISSING_FORM_LABELS,
                None,
            );
        }
    }

    fn check_missing_aria_labels(&mut self, document: &Html, result: &mut UrlAnalysisResult) {
        // Flag interactive elements that have NO accessible name at all (icon-only links/buttons
        // with no text, aria-label, title or nested image alt). A link/button with visible text
        // already has an accessible name and must NOT be flagged — that was the old false positive
        // where every <a>/<button> lacking aria-label was reported.
        let mut unnamed: Vec<String> = Vec::new();

        for sel_str in &["a[href]", "button"] {
            let selector = match Selector::parse(sel_str) {
                Ok(s) => s,
                Err(_) => continue,
            };
            for element in document.select(&selector) {
                let dedup_key = normalize_tag_for_dedup(&element);
                if element_has_accessible_name(&element) {
                    self.stats.add_ok(ANALYSIS_UNNAMED_INTERACTIVE, Some(&dedup_key));
                } else {
                    unnamed.push(get_opening_tag_html(&element));
                    self.stats.add_warning(ANALYSIS_UNNAMED_INTERACTIVE, Some(&dedup_key));
                }
            }
        }

        if !unnamed.is_empty() {
            result.add_warning(
                format!(
                    "{} link(s)/button(s) with no accessible name (icon-only without aria-label)",
                    unnamed.len()
                ),
                ANALYSIS_UNNAMED_INTERACTIVE,
                Some(unnamed),
            );
            self.pages_without_aria_labels += 1;
        } else {
            result.add_ok(
                "All links and buttons expose an accessible name".to_string(),
                ANALYSIS_UNNAMED_INTERACTIVE,
                None,
            );
        }
    }

    fn check_missing_roles(&mut self, document: &Html, result: &mut UrlAnalysisResult) {
        // Native landmark elements (<nav>/<main>/<header>/...) already expose implicit ARIA roles,
        // so flagging them for not having an explicit role= was a false positive (ARIA rule #1:
        // prefer native semantics). Instead flag the genuinely useful signal: the page exposes no
        // main landmark at all, which breaks "skip to content" and screen-reader navigation.
        let has_main = ["main", "[role='main']"].iter().any(|sel_str| {
            Selector::parse(sel_str)
                .ok()
                .map(|sel| document.select(&sel).next().is_some())
                .unwrap_or(false)
        });

        if has_main {
            self.stats.add_ok(ANALYSIS_MAIN_LANDMARK, Some("<main>"));
            result.add_ok(
                "Document exposes a main landmark.".to_string(),
                ANALYSIS_MAIN_LANDMARK,
                None,
            );
        } else {
            self.stats
                .add_warning(ANALYSIS_MAIN_LANDMARK, Some("<main> or role=\"main\""));
            result.add_warning(
                "Document has no main landmark (<main> or role=\"main\").".to_string(),
                ANALYSIS_MAIN_LANDMARK,
                Some(vec!["No <main> element or role=\"main\" found.".to_string()]),
            );
            self.pages_without_main_landmark += 1;
        }
    }

    fn check_missing_lang(&mut self, document: &Html, result: &mut UrlAnalysisResult) {
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

    /// Detect parser-impact structural defects that actually break a11y / scripting / navigation:
    /// duplicate `id` values and dangling IDREF references (aria-labelledby/-describedby/-controls/
    /// -owns and `<label for>`). These are statically detectable and far more meaningful than a raw
    /// "is the whole document W3C-valid" count, which Google itself says is not a reliable signal.
    fn check_html_structure(&mut self, document: &Html, result: &mut UrlAnalysisResult) {
        // Collect all ids and their occurrence counts (id is case-sensitive per the HTML spec).
        let id_selector = match Selector::parse("[id]") {
            Ok(s) => s,
            Err(_) => return,
        };
        let mut id_counts: HashMap<String, usize> = HashMap::new();
        for el in document.select(&id_selector) {
            if let Some(id) = el.value().attr("id") {
                let id = id.trim();
                if !id.is_empty() {
                    *id_counts.entry(id.to_string()).or_insert(0) += 1;
                }
            }
        }

        let mut issues: Vec<String> = Vec::new();

        // Duplicate ids: break label[for], aria references, in-page anchors and getElementById.
        let mut duplicate_ids: Vec<String> = id_counts
            .iter()
            .filter(|&(_, &count)| count > 1)
            .map(|(id, &count)| format!("Duplicate id=\"{}\" used {}x", id, count))
            .collect();
        duplicate_ids.sort();
        issues.extend(duplicate_ids);

        // Dangling ARIA IDREF references → silently produce no accessible name. One issue line per
        // element/attribute (listing all missing ids) rather than one per missing token.
        let idref_attrs = ["aria-labelledby", "aria-describedby", "aria-controls", "aria-owns"];
        for attr in &idref_attrs {
            if let Ok(sel) = Selector::parse(&format!("[{}]", attr)) {
                for el in document.select(&sel) {
                    if let Some(val) = el.value().attr(attr) {
                        let missing: Vec<&str> = val
                            .split_whitespace()
                            .filter(|token| !id_counts.contains_key(*token))
                            .collect();
                        if !missing.is_empty() {
                            issues.push(format!(
                                "{}=\"{}\" references missing id(s): {}",
                                attr,
                                val,
                                missing.join(", ")
                            ));
                        }
                    }
                }
            }
        }

        // <label for> pointing at a non-existent control.
        if let Ok(sel) = Selector::parse("label[for]") {
            for el in document.select(&sel) {
                if let Some(val) = el.value().attr("for") {
                    let val = val.trim();
                    if !val.is_empty() && !id_counts.contains_key(val) {
                        issues.push(format!("<label for=\"{}\"> references missing id", val));
                    }
                }
            }
        }

        if !issues.is_empty() {
            for issue in &issues {
                self.stats.add_warning(ANALYSIS_HTML_STRUCTURE, Some(issue));
            }
            result.add_warning(
                format!(
                    "{} HTML structural issue(s) (duplicate id / broken ARIA or label reference)",
                    issues.len()
                ),
                ANALYSIS_HTML_STRUCTURE,
                Some(issues),
            );
            self.pages_with_structural_issues += 1;
        } else {
            self.stats.add_ok(ANALYSIS_HTML_STRUCTURE, None);
        }
    }

    fn set_findings_to_summary(&self, status: &Status) {
        if self.pages_with_structural_issues > 0 {
            status.add_warning_to_summary(
                "pages-with-invalid-html",
                &format!(
                    "{} page(s) with HTML structural issues (duplicate id / broken ARIA references)",
                    self.pages_with_structural_issues
                ),
            );
        } else {
            status.add_ok_to_summary("pages-with-invalid-html", "No HTML structural issues found");
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
                &format!(
                    "{} page(s) with unnamed links/buttons (icon-only without aria-label)",
                    self.pages_without_aria_labels
                ),
            );
        } else {
            status.add_ok_to_summary("pages-without-aria-labels", "All links/buttons have an accessible name");
        }

        if self.pages_without_main_landmark > 0 {
            status.add_warning_to_summary(
                "pages-without-main-landmark",
                &format!("{} page(s) without a main landmark", self.pages_without_main_landmark),
            );
        } else {
            status.add_ok_to_summary("pages-without-main-landmark", "All pages expose a main landmark");
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
        // Parse the document once and share it across all checks (was parsed 6× before).
        let document = Html::parse_document(html);
        let mut result = UrlAnalysisResult::new();

        let s = Instant::now();
        self.check_image_alt_attributes(&document, &mut result);
        self.base
            .measure_exec_time("AccessibilityAnalyzer", "checkImageAltAttributes", s);

        let s = Instant::now();
        self.check_missing_labels(&document, &mut result);
        self.base
            .measure_exec_time("AccessibilityAnalyzer", "checkMissingLabels", s);

        let s = Instant::now();
        self.check_missing_aria_labels(&document, &mut result);
        self.base
            .measure_exec_time("AccessibilityAnalyzer", "checkMissingAriaLabels", s);

        let s = Instant::now();
        self.check_missing_roles(&document, &mut result);
        self.base
            .measure_exec_time("AccessibilityAnalyzer", "checkMissingRoles", s);

        let s = Instant::now();
        self.check_missing_lang(&document, &mut result);
        self.base
            .measure_exec_time("AccessibilityAnalyzer", "checkMissingLang", s);

        let s = Instant::now();
        self.check_html_structure(&document, &mut result);
        self.base
            .measure_exec_time("AccessibilityAnalyzer", "checkHtmlStructure", s);

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

fn attr_non_empty(attr: Option<&str>) -> bool {
    attr.map(|s| !s.trim().is_empty()).unwrap_or(false)
}

/// Pragmatic static approximation of "does this link/button expose an accessible name?".
/// Covers aria-label, aria-labelledby, title, visible text, and nested labelling content
/// (img alt, svg <title>, descendant aria-label/title). A subset of the WAI name computation,
/// good enough to avoid the false positive of flagging text links/buttons as unnamed.
fn element_has_accessible_name(element: &scraper::ElementRef) -> bool {
    let v = element.value();
    if attr_non_empty(v.attr("aria-label"))
        || attr_non_empty(v.attr("aria-labelledby"))
        || attr_non_empty(v.attr("title"))
    {
        return true;
    }
    if !element.text().collect::<String>().trim().is_empty() {
        return true;
    }
    if let Ok(sel) = Selector::parse("img[alt], svg title, [aria-label], [title]") {
        for child in element.select(&sel) {
            let cv = child.value();
            match cv.name() {
                "img" if attr_non_empty(cv.attr("alt")) => return true,
                "title" if !child.text().collect::<String>().trim().is_empty() => return true,
                _ if attr_non_empty(cv.attr("aria-label")) || attr_non_empty(cv.attr("title")) => return true,
                _ => {}
            }
        }
    }
    false
}

/// Is a form control labeled by a mechanism axe's `label` rule accepts: aria-label/-labelledby/title
/// on the control, an explicit <label for=id> or a wrapping <label> with text of its own, or a
/// non-empty placeholder on <input>/<textarea> (not on <select>)?
fn input_is_labeled(input: &scraper::ElementRef, document: &Html) -> bool {
    let v = input.value();
    if attr_non_empty(v.attr("aria-label"))
        || attr_non_empty(v.attr("aria-labelledby"))
        || attr_non_empty(v.attr("title"))
    {
        return true;
    }
    if v.name() != "select" && attr_non_empty(v.attr("placeholder")) {
        return true;
    }
    if let Some(id) = v.attr("id") {
        let id = id.trim();
        // Match label[for] by string comparison rather than building a selector from the (untrusted)
        // id, which would fail for ids containing quotes or selector metacharacters.
        if !id.is_empty()
            && let Ok(sel) = Selector::parse("label[for]")
            && document
                .select(&sel)
                .any(|l| l.value().attr("for") == Some(id) && label_has_text(&l))
        {
            return true;
        }
    }
    // Implicit wrapping <label>
    input
        .ancestors()
        .filter_map(scraper::ElementRef::wrap)
        .any(|el| el.value().name() == "label" && label_has_text(&el))
}

/// Does a <label> have text of its own? Its text, the alt text of a nested image and the aria-label
/// of the label or of an element in it (e.g. an SVG icon) count; nested form controls (the options
/// of a wrapped <select>, a <textarea> value, their aria-label), scripts and content hidden from
/// assistive technology (see `hides_itself` and `visibility_hidden`) do not.
/// The walk keeps its own stack: recursing once per nesting level overflows on deeply nested markup.
fn label_has_text(label: &scraper::ElementRef) -> bool {
    if attr_non_empty(label.value().attr("aria-label")) {
        return true;
    }
    // Each node with whether an element above it (inside the label) made it invisible.
    let mut pending: Vec<(ego_tree::NodeRef<scraper::Node>, bool)> =
        label.children().map(|child| (child, false)).collect();
    while let Some((node, invisible)) = pending.pop() {
        match node.value() {
            scraper::Node::Text(text) if !invisible && !text.trim().is_empty() => return true,
            scraper::Node::Element(el) => {
                if matches!(el.name(), "input" | "select" | "textarea" | "script" | "style") || hides_itself(el) {
                    continue;
                }
                let invisible = visibility_hidden(el).unwrap_or(invisible);
                if !invisible
                    && (attr_non_empty(el.attr("aria-label")) || (el.name() == "img" && attr_non_empty(el.attr("alt"))))
                {
                    return true;
                }
                pending.extend(node.children().map(|child| (child, invisible)));
            }
            _ => {}
        }
    }
    false
}

/// Is the control hidden from assistive technology: a `hidden` attribute, aria-hidden="true" or an
/// inline display:none on the control or on an ancestor, or an inline visibility:hidden/collapse on
/// the nearest of them that sets `visibility` (it is inherited, so a child can make itself visible
/// again)? Nobody can reach such a control, so it needs no label.
fn is_hidden_from_assistive_tech(element: &scraper::ElementRef) -> bool {
    let mut visibility_decided = false;
    for el in std::iter::once(*element).chain(element.ancestors().filter_map(scraper::ElementRef::wrap)) {
        if hides_itself(el.value()) {
            return true;
        }
        if !visibility_decided && let Some(hidden) = visibility_hidden(el.value()) {
            if hidden {
                return true;
            }
            visibility_decided = true;
        }
    }
    false
}

/// Does the element hide itself and its content: a `hidden` attribute, aria-hidden="true" or an
/// inline display:none?
fn hides_itself(el: &scraper::node::Element) -> bool {
    el.attr("hidden").is_some()
        || el
            .attr("aria-hidden")
            .is_some_and(|value| value.trim().eq_ignore_ascii_case("true"))
        || el
            .attr("style")
            .is_some_and(|style| inline_style_value(style, "display").as_deref() == Some("none"))
}

/// The inline `visibility` of the element: Some(true) for hidden/collapse, Some(false) for a value
/// that shows it, None when it sets none or inherits it (`inherit`, `unset`). Inherited, so a child
/// can show itself again.
fn visibility_hidden(el: &scraper::node::Element) -> Option<bool> {
    let visibility = inline_style_value(el.attr("style")?, "visibility")?;
    match visibility.as_str() {
        "inherit" | "unset" => None,
        value => Some(matches!(value, "hidden" | "collapse")),
    }
}

/// The lower-cased value an inline `style` attribute gives `property` (`display` or `visibility`), read
/// as a browser reads it: comments are ignored and quoted strings or parentheses never end a
/// declaration, the property name must match exactly (so `--x-display` is not `display`), a
/// declaration whose value the property does not accept is dropped, and a later declaration wins
/// unless an earlier one is `!important`.
fn inline_style_value(style: &str, property: &str) -> Option<String> {
    let mut effective: Option<(String, bool)> = None;
    for declaration in css_declarations(style) {
        let Some((name, value)) = declaration.split_once(':') else {
            continue;
        };
        if !name.trim().eq_ignore_ascii_case(property) {
            continue;
        }
        let mut value = value.trim().to_ascii_lowercase();
        let important = match value.rfind('!') {
            Some(bang) if value[bang + 1..].trim() == "important" => {
                value.truncate(bang);
                true
            }
            _ => false,
        };
        let value = value.trim_end().to_string();
        if !is_valid_css_value(property, &value) {
            continue;
        }
        if effective
            .as_ref()
            .is_none_or(|(_, was_important)| important || !was_important)
        {
            effective = Some((value, important));
        }
    }
    effective.map(|(value, _)| value)
}

/// The declarations of an inline style without comments: split at every `;` that is outside a
/// quoted string and outside parentheses.
fn css_declarations(style: &str) -> Vec<String> {
    let mut declarations = Vec::new();
    let mut current = String::new();
    let mut quote: Option<char> = None;
    let mut depth = 0usize;
    let mut chars = style.chars().peekable();
    while let Some(c) = chars.next() {
        if let Some(q) = quote {
            current.push(c);
            if c == '\\' {
                if let Some(escaped) = chars.next() {
                    current.push(escaped);
                }
            } else if c == q {
                quote = None;
            }
            continue;
        }
        match c {
            '/' if chars.peek() == Some(&'*') => {
                chars.next();
                let mut previous = ' ';
                for next in chars.by_ref() {
                    if previous == '*' && next == '/' {
                        break;
                    }
                    previous = next;
                }
                current.push(' ');
            }
            '"' | '\'' => {
                quote = Some(c);
                current.push(c);
            }
            '(' => {
                depth += 1;
                current.push(c);
            }
            ')' => {
                depth = depth.saturating_sub(1);
                current.push(c);
            }
            ';' if depth == 0 => declarations.push(std::mem::take(&mut current)),
            _ => current.push(c),
        }
    }
    declarations.push(current);
    declarations
}

/// Does `property` (`display` or `visibility`) accept `value`? Values with `var()` are accepted, as a
/// browser accepts them before it knows what they resolve to.
fn is_valid_css_value(property: &str, value: &str) -> bool {
    const GLOBAL: &[&str] = &["inherit", "initial", "unset", "revert", "revert-layer"];
    const DISPLAY_SINGLE: &[&str] = &[
        "none",
        "contents",
        "block",
        "inline",
        "run-in",
        "flow",
        "flow-root",
        "table",
        "flex",
        "grid",
        "ruby",
        "math",
        "list-item",
        "inline-block",
        "inline-table",
        "inline-flex",
        "inline-grid",
        "table-row-group",
        "table-header-group",
        "table-footer-group",
        "table-row",
        "table-cell",
        "table-column-group",
        "table-column",
        "table-caption",
        "ruby-base",
        "ruby-text",
        "ruby-base-container",
        "ruby-text-container",
        "-webkit-box",
        "-webkit-inline-box",
        "-webkit-flex",
        "-webkit-inline-flex",
        "-moz-box",
        "-moz-inline-box",
        "-ms-flexbox",
        "-ms-inline-flexbox",
        "-ms-grid",
        "-ms-inline-grid",
    ];
    // Keywords of the multi-keyword syntax, e.g. `inline flex` or `block flow list-item`.
    const DISPLAY_MULTI: &[&str] = &[
        "block",
        "inline",
        "run-in",
        "flow",
        "flow-root",
        "table",
        "flex",
        "grid",
        "ruby",
        "math",
        "list-item",
    ];
    if value.contains("var(") || GLOBAL.contains(&value) {
        return true;
    }
    match property {
        "display" => {
            let keywords: Vec<&str> = value.split_whitespace().collect();
            match keywords.as_slice() {
                [single] => DISPLAY_SINGLE.contains(single),
                [_, _] | [_, _, _] => keywords
                    .iter()
                    .enumerate()
                    .all(|(index, keyword)| DISPLAY_MULTI.contains(keyword) && !keywords[..index].contains(keyword)),
                _ => false,
            }
        }
        "visibility" => matches!(value, "visible" | "hidden" | "collapse"),
        _ => true,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run_structure_check(html: &str) -> (usize, UrlAnalysisResult) {
        let mut analyzer = AccessibilityAnalyzer::new();
        let mut result = UrlAnalysisResult::new();
        analyzer.check_html_structure(&Html::parse_document(html), &mut result);
        (analyzer.pages_with_structural_issues, result)
    }

    #[test]
    fn structure_flags_duplicate_id() {
        let html = r#"<html><body><div id="x"></div><span id="x"></span></body></html>"#;
        let (pages, result) = run_structure_check(html);
        assert_eq!(pages, 1);
        assert!(
            result.get_warning().iter().any(|w| w.contains("structural")),
            "got: {:?}",
            result.get_warning()
        );
    }

    #[test]
    fn structure_flags_dangling_aria_reference() {
        let html = r#"<html><body><button aria-labelledby="missing">x</button></body></html>"#;
        let (pages, result) = run_structure_check(html);
        assert_eq!(pages, 1);
        assert!(!result.get_warning().is_empty());
    }

    #[test]
    fn structure_flags_dangling_label_for() {
        let html = r#"<html><body><label for="nope">Name</label></body></html>"#;
        let (pages, _result) = run_structure_check(html);
        assert_eq!(pages, 1);
    }

    #[test]
    fn structure_clean_html_has_no_issues() {
        // Unique ids, label[for] and aria-labelledby both resolve to real ids.
        let html = r#"<html><body>
            <label for="email">Email</label><input id="email">
            <section aria-labelledby="hdr"></section><h2 id="hdr">Title</h2>
        </body></html>"#;
        let (pages, result) = run_structure_check(html);
        assert_eq!(pages, 0);
        assert!(result.get_warning().is_empty(), "got: {:?}", result.get_warning());
    }

    #[test]
    fn decorative_empty_alt_is_not_flagged() {
        let mut analyzer = AccessibilityAnalyzer::new();
        let mut result = UrlAnalysisResult::new();
        analyzer.check_image_alt_attributes(&Html::parse_document(r#"<img src="x.png" alt="">"#), &mut result);
        assert!(
            result.get_warning().is_empty(),
            "decorative alt=\"\" must be OK: {:?}",
            result.get_warning()
        );
    }

    #[test]
    fn missing_alt_is_flagged() {
        let mut analyzer = AccessibilityAnalyzer::new();
        let mut result = UrlAnalysisResult::new();
        analyzer.check_image_alt_attributes(&Html::parse_document(r#"<img src="x.png">"#), &mut result);
        assert!(!result.get_warning().is_empty());
    }

    #[test]
    fn text_link_has_accessible_name() {
        let mut analyzer = AccessibilityAnalyzer::new();
        let mut result = UrlAnalysisResult::new();
        analyzer.check_missing_aria_labels(
            &Html::parse_document(r#"<html><body><a href="/x">Read more</a></body></html>"#),
            &mut result,
        );
        assert!(
            result.get_warning().is_empty(),
            "text link must not be flagged: {:?}",
            result.get_warning()
        );
    }

    #[test]
    fn icon_only_link_without_name_is_flagged() {
        let mut analyzer = AccessibilityAnalyzer::new();
        let mut result = UrlAnalysisResult::new();
        analyzer.check_missing_aria_labels(
            &Html::parse_document(r#"<html><body><a href="/x"><svg></svg></a></body></html>"#),
            &mut result,
        );
        assert_eq!(analyzer.pages_without_aria_labels, 1);
        assert!(!result.get_warning().is_empty());
    }

    #[test]
    fn icon_link_with_aria_label_is_ok() {
        let mut analyzer = AccessibilityAnalyzer::new();
        let mut result = UrlAnalysisResult::new();
        analyzer.check_missing_aria_labels(
            &Html::parse_document(r#"<html><body><a href="/x" aria-label="Home"><svg></svg></a></body></html>"#),
            &mut result,
        );
        assert_eq!(analyzer.pages_without_aria_labels, 0);
    }

    #[test]
    fn wrapping_label_is_accepted() {
        let mut analyzer = AccessibilityAnalyzer::new();
        let mut result = UrlAnalysisResult::new();
        analyzer.check_missing_labels(
            &Html::parse_document(r#"<html><body><label>Name <input type="text"></label></body></html>"#),
            &mut result,
        );
        assert!(
            result.get_warning().is_empty(),
            "wrapping label must be accepted: {:?}",
            result.get_warning()
        );
    }

    #[test]
    fn unlabeled_input_is_flagged() {
        let mut analyzer = AccessibilityAnalyzer::new();
        let mut result = UrlAnalysisResult::new();
        analyzer.check_missing_labels(
            &Html::parse_document(r#"<html><body><input type="text" name="q"></body></html>"#),
            &mut result,
        );
        assert_eq!(analyzer.pages_without_form_labels, 1);
    }

    #[test]
    fn label_for_with_special_char_id_is_accepted() {
        // An id with ':' would break a selector built from it; matched by string comparison instead.
        let mut analyzer = AccessibilityAnalyzer::new();
        let mut result = UrlAnalysisResult::new();
        analyzer.check_missing_labels(
            &Html::parse_document(
                r#"<html><body><label for="a:b">X</label><input id="a:b" type="text"></body></html>"#,
            ),
            &mut result,
        );
        assert!(
            result.get_warning().is_empty(),
            "label[for] with a special-char id must be accepted: {:?}",
            result.get_warning()
        );
    }

    #[test]
    fn semantic_nav_not_flagged_but_missing_main_is() {
        // <nav> without explicit role must NOT be flagged; missing <main> IS the new signal.
        let mut analyzer = AccessibilityAnalyzer::new();
        let mut result = UrlAnalysisResult::new();
        analyzer.check_missing_roles(
            &Html::parse_document(r#"<html><body><nav>links</nav></body></html>"#),
            &mut result,
        );
        assert_eq!(analyzer.pages_without_main_landmark, 1, "no <main> should be flagged");

        let mut analyzer2 = AccessibilityAnalyzer::new();
        let mut result2 = UrlAnalysisResult::new();
        analyzer2.check_missing_roles(
            &Html::parse_document(r#"<html><body><main>content</main></body></html>"#),
            &mut result2,
        );
        assert_eq!(analyzer2.pages_without_main_landmark, 0, "page with <main> is OK");
        assert!(result2.get_warning().is_empty());
    }

    fn form_label_warnings(html: &str) -> Vec<String> {
        let mut analyzer = AccessibilityAnalyzer::new();
        let mut result = UrlAnalysisResult::new();
        analyzer.check_missing_labels(&Html::parse_document(html), &mut result);
        result.get_warning().to_vec()
    }

    #[test]
    fn issue_112_search_form_has_no_label_findings() {
        // The exact markup from #112: label[for] + placeholder, a submit and a hidden input.
        let html = r#"<html><body><div class="woocommerce">
            <form id="searchform" action="https://www.plastic2go.com.au" method="get">
                <label class="screen-reader-text" for="woocommerce-product-search-field-0">Find products</label>
                <input id="woocommerce-product-search-field-0" name="s" type="text" value="" placeholder="Find products">
                <input id="searchsubmit" class="button" type="submit" value="Find Now">
                <input name="post_type" type="hidden" value="product">
            </form>
        </div></body></html>"#;
        assert!(
            form_label_warnings(html).is_empty(),
            "got: {:?}",
            form_label_warnings(html)
        );
    }

    #[test]
    fn empty_labels_do_not_label_a_control() {
        for html in [
            r#"<html><body><label for="q"></label><input id="q" type="text"></body></html>"#,
            r#"<html><body><label for="q">   </label><input id="q" type="text"></body></html>"#,
            r#"<html><body><label><input type="text"></label></body></html>"#,
            r#"<html><body><label><select><option>Prague</option></select></label></body></html>"#,
        ] {
            assert_eq!(form_label_warnings(html).len(), 1, "expected a finding for {html}");
        }
    }

    #[test]
    fn label_text_in_nested_elements_or_image_alt_counts() {
        for html in [
            r#"<html><body><label><span>Name</span> <input type="text"></label></body></html>"#,
            r#"<html><body><label for="q"><img src="search.svg" alt="Search"></label><input id="q" type="search"></body></html>"#,
        ] {
            assert!(form_label_warnings(html).is_empty(), "expected no finding for {html}");
        }
    }

    #[test]
    fn aria_label_inside_a_label_counts_as_its_text() {
        for html in [
            r#"<html><body><label for="q"><svg role="img" aria-label="Search"></svg></label><input id="q"></body></html>"#,
            r#"<html><body><label for="q"><span aria-label="Search"></span></label><input id="q"></body></html>"#,
            r#"<html><body><label><svg role="img" aria-label="Search"></svg><input id="q"></label></body></html>"#,
        ] {
            assert!(form_label_warnings(html).is_empty(), "expected no finding for {html}");
        }
        // The name of a nested form control is still not text of the label.
        let nested_control = r#"<html><body><label for="q"><select aria-label="City"><option>Prague</option></select></label><input id="q"></body></html>"#;
        assert_eq!(form_label_warnings(nested_control).len(), 1);
    }

    #[test]
    fn placeholder_labels_inputs_and_textareas_but_not_selects() {
        assert!(
            form_label_warnings(r#"<html><body><input type="text" placeholder="Search"></body></html>"#).is_empty()
        );
        assert!(
            form_label_warnings(r#"<html><body><textarea placeholder="Your message"></textarea></body></html>"#)
                .is_empty()
        );
        assert_eq!(
            form_label_warnings(r#"<html><body><input type="text" placeholder="  "></body></html>"#).len(),
            1
        );
        assert_eq!(
            form_label_warnings(
                r#"<html><body><select placeholder="Pick one"><option>A</option></select></body></html>"#
            )
            .len(),
            1
        );
    }

    #[test]
    fn hidden_content_of_a_label_does_not_name_the_control() {
        for html in [
            r#"<label for="q"><svg role="img" aria-hidden="true" aria-label="Search"></svg></label><input id="q">"#,
            r#"<label for="q"><span hidden aria-label="Search"></span></label><input id="q">"#,
            r#"<label for="q"><span style="display:none"><svg role="img" aria-label="Search"></svg></span></label><input id="q">"#,
            r#"<label><span style="visibility:hidden">Search</span><input id="q"></label>"#,
        ] {
            let html = format!("<html><body>{html}</body></html>");
            assert_eq!(form_label_warnings(&html).len(), 1, "hidden label content: {html}");
        }
        // A child can make itself visible again.
        let visible = r#"<html><body><label for="q"><span style="visibility:hidden"><span style="visibility:visible">Search</span></span></label><input id="q"></body></html>"#;
        assert!(form_label_warnings(visible).is_empty());
    }

    #[test]
    fn inline_styles_that_leave_a_control_visible_keep_it_checked() {
        for body in [
            // a later declaration wins
            r#"<input id="q" style="display:none;display:block">"#,
            r#"<input id="q" style="display:none;display:block !important">"#,
            // a comment or a custom property is not a `display` declaration
            r#"<input id="q" style="/* display:none */ display:block">"#,
            r#"<input id="q" style="--previous-display:none;display:block">"#,
            // the nearest `visibility` decides
            r#"<div style="visibility:hidden"><input id="q" style="visibility:visible"></div>"#,
            // a `;` in a quoted value does not end the declaration
            r#"<input id="q" style="display:block; --literal:';display:none;'">"#,
        ] {
            let html = format!("<html><body>{body}</body></html>");
            assert_eq!(
                form_label_warnings(&html).len(),
                1,
                "visible control must be checked: {body}"
            );
        }
    }

    #[test]
    fn inline_styles_that_hide_a_control_are_read_as_css() {
        for body in [
            r#"<input id="q" style="display:/**/none">"#,
            r#"<input id="q" style="DISPLAY: None">"#,
            r#"<input id="q" style="display:none !important;display:block">"#,
            r#"<div style="display:none"><input id="q" style="display:block"></div>"#,
            r#"<input id="q" style="visibility:collapse">"#,
            r#"<div style="visibility:visible"><input id="q" style="visibility:hidden"></div>"#,
            r#"<div style="visibility:hidden"><span style="visibility:inherit"><input id="q"></span></div>"#,
            // a `/*` in a quoted value does not start a comment
            r#"<input id="q" style="--literal:'/*';display:none">"#,
            // a browser drops a declaration with an invalid value
            r#"<input id="q" style="display:none;display:invalid">"#,
            r#"<div style="visibility:hidden"><input id="q" style="visibility:invalid"></div>"#,
        ] {
            let html = format!("<html><body>{body}</body></html>");
            assert!(
                form_label_warnings(&html).is_empty(),
                "hidden control must be skipped: {body}"
            );
        }
    }

    #[test]
    fn controls_hidden_from_assistive_technology_are_skipped() {
        for html in [
            r#"<html><body><input type="text" aria-hidden="true"></body></html>"#,
            r#"<html><body><input type="text" hidden></body></html>"#,
            r#"<html><body><input type="text" style="visibility: hidden"></body></html>"#,
            r#"<html><body><div style="display: none;"><input type="text"></div></body></html>"#,
            r#"<html><body><div aria-hidden="true"><form><select><option>A</option></select></form></div></body></html>"#,
        ] {
            let mut analyzer = AccessibilityAnalyzer::new();
            let mut result = UrlAnalysisResult::new();
            analyzer.check_missing_labels(&Html::parse_document(html), &mut result);
            assert!(
                result.get_warning().is_empty(),
                "hidden control must be skipped: {html}"
            );
            assert!(
                result.get_ok().is_empty(),
                "only hidden controls: nothing to report: {html}"
            );
            assert_eq!(analyzer.pages_without_form_labels, 0);
        }
    }
}
