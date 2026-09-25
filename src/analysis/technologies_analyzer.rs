// SiteOne Crawler - TechnologiesAnalyzer
// (c) Jan Reges <jan.reges@siteone.cz>
//
// Passive technology detection (#19, #22): servers, CDNs, WAF / bot protection, hosting platforms,
// CMSs, e-commerce platforms, frameworks, JS libraries, analytics and fonts/UI kits are recognized from
// what the crawl already downloaded - response headers, Set-Cookie names, <meta> tags, <script src>
// URLs and a few HTML markers. No extra requests are made. The signatures are our own curated data in
// `technologies/signatures.json` (MIT like the rest of the project).

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::time::Instant;

use once_cell::sync::Lazy;
use regex::bytes::{Captures, Regex, RegexBuilder, RegexSet, RegexSetBuilder};
use scraper::{Html, Selector};
use serde::Deserialize;

use crate::analysis::analyzer::Analyzer;
use crate::analysis::base_analyzer::BaseAnalyzer;
use crate::analysis::result::url_analysis_result::UrlAnalysisResult;
use crate::components::super_table::SuperTable;
use crate::components::super_table_column::SuperTableColumn;
use crate::output::output::Output;
use crate::result::status::Status;
use crate::result::visited_url::VisitedUrl;
use crate::types::ContentTypeId;
use crate::utils;

const SUPER_TABLE_TECHNOLOGIES: &str = "technologies";

/// Categories in display order; every signature uses one of them.
pub const CATEGORIES: [&str; 11] = [
    "Server",
    "CDN",
    "WAF / Security",
    "Hosting / PaaS",
    "CMS",
    "E-commerce",
    "Backend framework",
    "Frontend framework",
    "JS library",
    "Analytics / Tag manager",
    "Fonts / UI",
];

/// Shown when nothing was recognized. It deliberately says nothing about what is *not* in use:
/// passive detection cannot prove that a site has no CDN or WAF.
const EMPTY_TABLE_MESSAGE: &str = "No known technology signatures were found in the crawled pages.";

const TABLE_DESCRIPTION: &str = "Passively recognized from response headers, cookie names, meta tags, script URLs and HTML markers of the crawled HTML pages. A technology missing here may still be in use.";

const MAX_EVIDENCE_CHARS: usize = 80;

const SIGNATURES_JSON: &str = include_str!("technologies/signatures.json");

static SIGNATURES: Lazy<Signatures> =
    Lazy::new(|| Signatures::parse(SIGNATURES_JSON).expect("the embedded technology signatures are valid"));

static SELECTOR_META: Lazy<Selector> = Lazy::new(|| Selector::parse("meta[name][content]").unwrap());
static SELECTOR_SCRIPT_SRC: Lazy<Selector> = Lazy::new(|| Selector::parse("script[src]").unwrap());

#[derive(Deserialize)]
struct SignatureFile {
    technologies: Vec<SignatureDef>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct SignatureDef {
    name: String,
    category: String,
    /// lower-case header name -> value regex ("" = the header is present)
    #[serde(default)]
    headers: BTreeMap<String, String>,
    /// cookie-name regexes
    #[serde(default)]
    cookies: Vec<String>,
    /// lower-case meta name -> content regex
    #[serde(default)]
    meta: BTreeMap<String, String>,
    /// <script src> regexes
    #[serde(default)]
    scripts: Vec<String>,
    /// raw HTML regexes
    #[serde(default)]
    html: Vec<String>,
}

struct Technology {
    name: String,
    category: &'static str,
}

/// A header or meta rule: the value of `key` must match `pattern`.
struct KeyedRule {
    tech: usize,
    key: String,
    pattern: Regex,
}

/// A cookie-name, script-src or HTML rule.
struct Rule {
    tech: usize,
    pattern: Regex,
}

/// Rules scanned together: `set` finds the candidates in one pass, `rules[i]` extracts the version.
struct RuleSet {
    set: RegexSet,
    rules: Vec<Rule>,
}

/// All signatures, compiled once. Regexes are ASCII-only (`unicode(false)`) and case-insensitive, so
/// the multi-pattern scans stay on the fast DFA even for pages with non-ASCII text.
struct Signatures {
    technologies: Vec<Technology>,
    headers: Vec<KeyedRule>,
    meta: Vec<KeyedRule>,
    cookies: Vec<Rule>,
    scripts: RuleSet,
    html: RuleSet,
}

fn compile(pattern: &str) -> Result<Regex, String> {
    RegexBuilder::new(pattern)
        .case_insensitive(true)
        .unicode(false)
        .build()
        .map_err(|e| format!("invalid pattern '{}': {}", pattern, e))
}

impl RuleSet {
    fn new(rules: Vec<Rule>, patterns: Vec<String>) -> Result<Self, String> {
        let set = RegexSetBuilder::new(&patterns)
            .case_insensitive(true)
            .unicode(false)
            .build()
            .map_err(|e| e.to_string())?;
        Ok(Self { set, rules })
    }
}

impl Signatures {
    fn parse(json: &str) -> Result<Self, String> {
        let file: SignatureFile = serde_json::from_str(json).map_err(|e| e.to_string())?;
        let mut technologies = Vec::new();
        let mut headers = Vec::new();
        let mut meta = Vec::new();
        let mut cookies = Vec::new();
        let mut script_rules = Vec::new();
        let mut script_patterns = Vec::new();
        let mut html_rules = Vec::new();
        let mut html_patterns = Vec::new();

        for def in file.technologies {
            let tech = technologies.len();
            let category = CATEGORIES
                .iter()
                .copied()
                .find(|c| *c == def.category)
                .ok_or_else(|| format!("{}: unknown category '{}'", def.name, def.category))?;
            if def.headers.is_empty()
                && def.cookies.is_empty()
                && def.meta.is_empty()
                && def.scripts.is_empty()
                && def.html.is_empty()
            {
                return Err(format!("{}: no rules", def.name));
            }
            for (key, pattern) in def.headers {
                headers.push(KeyedRule {
                    tech,
                    key: key.to_ascii_lowercase(),
                    pattern: compile(&pattern)?,
                });
            }
            for (key, pattern) in def.meta {
                meta.push(KeyedRule {
                    tech,
                    key: key.to_ascii_lowercase(),
                    pattern: compile(&pattern)?,
                });
            }
            for pattern in def.cookies {
                cookies.push(Rule {
                    tech,
                    pattern: compile(&pattern)?,
                });
            }
            for pattern in def.scripts {
                script_rules.push(Rule {
                    tech,
                    pattern: compile(&pattern)?,
                });
                script_patterns.push(pattern);
            }
            for pattern in def.html {
                html_rules.push(Rule {
                    tech,
                    pattern: compile(&pattern)?,
                });
                html_patterns.push(pattern);
            }
            technologies.push(Technology {
                name: def.name,
                category,
            });
        }

        Ok(Self {
            technologies,
            headers,
            meta,
            cookies,
            scripts: RuleSet::new(script_rules, script_patterns)?,
            html: RuleSet::new(html_rules, html_patterns)?,
        })
    }
}

/// One technology recognized on one response.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Hit {
    tech: usize,
    versions: BTreeSet<String>,
    evidence: String,
}

/// Record a hit. A technology found by several rules on one page counts once, with the first
/// evidence and every version any rule captured (a page can load two versions of one library).
fn add_hit(hits: &mut Vec<Hit>, tech: usize, version: Option<String>, evidence: String) {
    if let Some(hit) = hits.iter_mut().find(|hit| hit.tech == tech) {
        hit.versions.extend(version);
    } else {
        hits.push(Hit {
            tech,
            versions: version.into_iter().collect(),
            evidence: shorten_evidence(&evidence),
        });
    }
}

/// The first participating capture group, i.e. the version ("3.7.1").
fn version_of(caps: &Captures<'_>) -> Option<String> {
    caps.iter()
        .skip(1)
        .flatten()
        .map(|m| String::from_utf8_lossy(m.as_bytes()).trim_matches('.').to_string())
        .find(|version| !version.is_empty())
}

/// Collapse whitespace and cut to MAX_EVIDENCE_CHARS characters.
fn shorten_evidence(evidence: &str) -> String {
    let collapsed = evidence.split_whitespace().collect::<Vec<_>>().join(" ");
    if collapsed.chars().count() <= MAX_EVIDENCE_CHARS {
        collapsed
    } else {
        format!(
            "{}…",
            collapsed.chars().take(MAX_EVIDENCE_CHARS - 1).collect::<String>()
        )
    }
}

/// Technologies recognized in one response. Cookie values are never used or shown, only names;
/// script evidence drops the query string.
fn detect(signatures: &Signatures, headers: &HashMap<String, String>, body: &str) -> Vec<Hit> {
    let mut hits: Vec<Hit> = Vec::new();

    for rule in &signatures.headers {
        if let Some(value) = headers.get(&rule.key)
            && let Some(caps) = rule.pattern.captures(value.as_bytes())
        {
            add_hit(
                &mut hits,
                rule.tech,
                version_of(&caps),
                format!("{}: {}", rule.key, value),
            );
        }
    }

    if let Some(set_cookie) = headers.get("set-cookie") {
        for cookie in set_cookie.split('\n') {
            let name = cookie.split('=').next().unwrap_or("").trim();
            if name.is_empty() {
                continue;
            }
            for rule in &signatures.cookies {
                if rule.pattern.is_match(name.as_bytes()) {
                    add_hit(&mut hits, rule.tech, None, format!("cookie {}", name));
                }
            }
        }
    }

    // <meta> and <script src> come from the parsed document: attribute values are decoded, and markup
    // in comments, <textarea> text or custom elements such as <script-widget> is not a tag.
    let document = Html::parse_document(body);
    for meta in document.select(&SELECTOR_META) {
        let (Some(name), Some(content)) = (meta.value().attr("name"), meta.value().attr("content")) else {
            continue;
        };
        let name = name.to_ascii_lowercase();
        for rule in signatures.meta.iter().filter(|rule| rule.key == name) {
            if let Some(caps) = rule.pattern.captures(content.as_bytes()) {
                let evidence = format!("meta {}: {}", name, content);
                add_hit(&mut hits, rule.tech, version_of(&caps), evidence);
            }
        }
    }

    for script in document.select(&SELECTOR_SCRIPT_SRC) {
        let Some(src) = script.value().attr("src") else {
            continue;
        };
        for index in signatures.scripts.set.matches(src.as_bytes()).iter() {
            let rule = &signatures.scripts.rules[index];
            if let Some(rule_caps) = rule.pattern.captures(src.as_bytes()) {
                let without_query = src.split('?').next().unwrap_or("");
                add_hit(
                    &mut hits,
                    rule.tech,
                    version_of(&rule_caps),
                    format!("script {}", without_query),
                );
            }
        }
    }

    let body = body.as_bytes();
    for index in signatures.html.set.matches(body).iter() {
        let rule = &signatures.html.rules[index];
        // A versioned marker can appear with several versions; an unversioned one needs only one match.
        let limit = if rule.pattern.captures_len() > 1 { usize::MAX } else { 1 };
        for caps in rule.pattern.captures_iter(body).take(limit) {
            let matched = caps
                .get(0)
                .map(|m| String::from_utf8_lossy(m.as_bytes()).into_owned())
                .unwrap_or_default();
            add_hit(&mut hits, rule.tech, version_of(&caps), format!("html {}", matched));
        }
    }

    hits
}

/// One technology across the whole crawl.
#[derive(Debug, Default)]
struct Detection {
    versions: BTreeSet<String>,
    evidence: String,
    pages: usize,
}

pub struct TechnologiesAnalyzer {
    base: BaseAnalyzer,
    detections: HashMap<usize, Detection>,
}

impl Default for TechnologiesAnalyzer {
    fn default() -> Self {
        Self::new()
    }
}

impl TechnologiesAnalyzer {
    pub fn new() -> Self {
        Self {
            base: BaseAnalyzer::new(),
            detections: HashMap::new(),
        }
    }

    /// Table rows ordered by category, then by technology name.
    fn table_rows(&self) -> Vec<HashMap<String, String>> {
        let signatures = &*SIGNATURES;
        let mut found: Vec<(&Technology, &Detection)> = self
            .detections
            .iter()
            .map(|(tech, detection)| (&signatures.technologies[*tech], detection))
            .collect();
        found.sort_by(|a, b| {
            category_rank(a.0.category)
                .cmp(&category_rank(b.0.category))
                .then_with(|| a.0.name.to_lowercase().cmp(&b.0.name.to_lowercase()))
        });
        found
            .into_iter()
            .map(|(technology, detection)| {
                let mut row = HashMap::new();
                row.insert("category".to_string(), technology.category.to_string());
                row.insert("technology".to_string(), technology.name.clone());
                let mut versions: Vec<&str> = detection.versions.iter().map(String::as_str).collect();
                versions.sort_by(|a, b| compare_versions(a, b));
                row.insert("version".to_string(), versions.join(", "));
                row.insert("evidence".to_string(), detection.evidence.clone());
                row.insert("pages".to_string(), detection.pages.to_string());
                row
            })
            .collect()
    }
}

/// Order versions by their numeric components, so that 9.1 comes before 10.0.
fn compare_versions(a: &str, b: &str) -> std::cmp::Ordering {
    let components = |version: &str| -> Vec<u64> {
        version
            .split('.')
            .map(|part| part.parse().unwrap_or(u64::MAX))
            .collect()
    };
    components(a).cmp(&components(b)).then_with(|| a.cmp(b))
}

fn category_rank(category: &str) -> usize {
    CATEGORIES
        .iter()
        .position(|c| *c == category)
        .unwrap_or(CATEGORIES.len())
}

impl Analyzer for TechnologiesAnalyzer {
    fn analyze(&mut self, status: &Status, output: &mut dyn Output) {
        let console_width = utils::get_console_width();
        let columns = vec![
            SuperTableColumn::new(
                "category".to_string(),
                "Category".to_string(),
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
                "technology".to_string(),
                "Technology".to_string(),
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
                "version".to_string(),
                "Version".to_string(),
                12,
                None,
                None,
                true,
                false,
                false,
                true,
                None,
            ),
            SuperTableColumn::new(
                "evidence".to_string(),
                "Evidence".to_string(),
                (console_width as i32 - 80).max(30),
                None,
                None,
                true,
                false,
                false,
                true,
                None,
            ),
            SuperTableColumn::new(
                "pages".to_string(),
                "Pages".to_string(),
                6,
                None,
                None,
                false,
                false,
                false,
                true,
                None,
            ),
        ];

        let mut super_table = SuperTable::new(
            SUPER_TABLE_TECHNOLOGIES.to_string(),
            "Technologies".to_string(),
            EMPTY_TABLE_MESSAGE.to_string(),
            columns,
            false,
            None,
            "ASC".to_string(),
            Some(TABLE_DESCRIPTION.to_string()),
            None,
            None,
        );

        super_table.set_data(self.table_rows());
        status.configure_super_table_url_stripping(&mut super_table);
        output.add_super_table(&super_table);
        status.add_super_table_at_end(super_table);
    }

    fn analyze_visited_url(
        &mut self,
        visited_url: &VisitedUrl,
        body: Option<&str>,
        headers: Option<&HashMap<String, String>>,
    ) -> Option<UrlAnalysisResult> {
        // Only the site's own HTML pages (error pages such as WAF blocks included): assets and redirects
        // would inflate "Pages", and third-party responses would report *their* stack as the site's.
        if visited_url.content_type != ContentTypeId::Html
            || !visited_url.is_allowed_for_crawling
            || visited_url.status_code <= 0
            || (300..400).contains(&visited_url.status_code)
        {
            return None;
        }

        let s = Instant::now();
        let no_headers = HashMap::new();
        let hits = detect(&SIGNATURES, headers.unwrap_or(&no_headers), body.unwrap_or(""));
        for hit in hits {
            let detection = self.detections.entry(hit.tech).or_default();
            detection.pages += 1;
            detection.versions.extend(hit.versions);
            if detection.evidence.is_empty() {
                detection.evidence = hit.evidence;
            }
        }
        self.base
            .measure_exec_time("TechnologiesAnalyzer", "detectTechnologies", s);

        None
    }

    fn should_be_activated(&self) -> bool {
        true
    }

    fn get_order(&self) -> i32 {
        220
    }

    fn get_name(&self) -> &str {
        "TechnologiesAnalyzer"
    }

    fn get_exec_times(&self) -> &HashMap<String, f64> {
        self.base.get_exec_times()
    }

    fn get_exec_counts(&self) -> &HashMap<String, usize> {
        self.base.get_exec_counts()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn headers(pairs: &[(&str, &str)]) -> HashMap<String, String> {
        pairs.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect()
    }

    /// (technology name, version) pairs recognized in one response; a technology without a
    /// version appears once with `None`.
    fn detected(pairs: &[(&str, &str)], html: &str) -> Vec<(String, Option<String>)> {
        detect(&SIGNATURES, &headers(pairs), html)
            .into_iter()
            .flat_map(|hit| {
                let name = &SIGNATURES.technologies[hit.tech].name;
                if hit.versions.is_empty() {
                    vec![(name.clone(), None)]
                } else {
                    hit.versions.into_iter().map(|v| (name.clone(), Some(v))).collect()
                }
            })
            .collect()
    }

    fn has(found: &[(String, Option<String>)], name: &str, version: Option<&str>) -> bool {
        found.iter().any(|(n, v)| n == name && v.as_deref() == version)
    }

    fn page(url: &str) -> VisitedUrl {
        VisitedUrl::new(
            url.to_string(),
            String::new(),
            crate::result::visited_url::SOURCE_A_HREF,
            url.to_string(),
            200,
            0.1,
            Some(100),
            ContentTypeId::Html,
            Some("text/html".to_string()),
            None,
            None,
            false,
            true,
            0,
            None,
        )
    }

    #[test]
    fn signatures_file_is_valid() {
        let signatures = &*SIGNATURES;
        assert!(
            signatures.technologies.len() >= 100,
            "got {}",
            signatures.technologies.len()
        );
        let mut names = std::collections::HashSet::new();
        for technology in &signatures.technologies {
            assert!(names.insert(technology.name.as_str()), "duplicate {}", technology.name);
        }
        for category in CATEGORIES {
            assert!(
                signatures.technologies.iter().any(|t| t.category == category),
                "no signature in {category}"
            );
        }
    }

    #[test]
    fn detects_cloudflare_cdn_and_bot_management() {
        let found = detected(
            &[
                ("server", "cloudflare"),
                ("cf-ray", "8c1d2e3f4a5b6c7d-PRG"),
                ("set-cookie", "__cf_bm=abc; path=/; HttpOnly\nsession=1; Secure"),
            ],
            "",
        );
        assert!(has(&found, "Cloudflare", None), "{found:?}");
        assert!(has(&found, "Cloudflare Bot Management", None), "{found:?}");
    }

    #[test]
    fn detects_sucuri_waf() {
        let found = detected(&[("x-sucuri-id", "18012"), ("server", "Sucuri/Cloudproxy")], "");
        assert!(has(&found, "Sucuri", None), "{found:?}");
    }

    #[test]
    fn detects_nginx_with_and_without_version() {
        assert!(has(
            &detected(&[("server", "nginx/1.25.3")], ""),
            "Nginx",
            Some("1.25.3")
        ));
        assert!(has(&detected(&[("server", "nginx")], ""), "Nginx", None));
    }

    #[test]
    fn apache_coyote_is_tomcat_not_apache_httpd() {
        let found = detected(&[("server", "Apache-Coyote/1.1")], "");
        assert!(has(&found, "Apache Tomcat", None), "{found:?}");
        assert!(!found.iter().any(|(n, _)| n == "Apache HTTP Server"), "{found:?}");
    }

    #[test]
    fn detects_wordpress_version_from_generator() {
        let html = r#"<html><head><meta name="generator" content="WordPress 6.5.2"></head><body></body></html>"#;
        assert!(has(&detected(&[], html), "WordPress", Some("6.5.2")));
    }

    #[test]
    fn detects_nextjs_from_its_data_script() {
        let html = r#"<script id="__NEXT_DATA__" type="application/json">{"props":{}}</script>"#;
        assert!(has(&detected(&[], html), "Next.js", None));
    }

    #[test]
    fn detects_jquery_version_from_script_urls() {
        for (html, version) in [
            (
                r#"<script src="https://code.jquery.com/jquery-3.7.1.min.js"></script>"#,
                "3.7.1",
            ),
            (
                r#"<script src='/wp-includes/js/jquery/jquery.min.js?ver=3.7.1'></script>"#,
                "3.7.1",
            ),
            (
                r#"<script data-src="x.js" src="https://ajax.googleapis.com/ajax/libs/jquery/3.6.0/jquery.min.js"></script>"#,
                "3.6.0",
            ),
        ] {
            assert!(has(&detected(&[], html), "jQuery", Some(version)), "{html}");
        }
        let unrelated = r#"<script src="/js/jquery-ui.min.js"></script><script src="/js/myjquery.js"></script>"#;
        assert!(!detected(&[], unrelated).iter().any(|(n, _)| n == "jQuery"));
    }

    #[test]
    fn inactive_markup_is_not_a_meta_tag_or_a_script() {
        for markup in [
            r#"<!-- <meta name="generator" content="WordPress 4.0"><script src="/js/jquery-1.12.4.min.js"></script> -->"#,
            r#"<textarea><meta name="generator" content="WordPress 4.0"><script src="/js/jquery-1.12.4.min.js"></script></textarea>"#,
            r#"<meta-data name="generator" content="WordPress 4.0"><script-widget src="/js/jquery-1.12.4.min.js"></script-widget>"#,
        ] {
            let found = detected(&[], &format!("<html><head></head><body>{markup}</body></html>"));
            assert!(found.is_empty(), "{markup}: {found:?}");
        }
    }

    #[test]
    fn raw_html_markers_still_match_comments() {
        let html = "<html><head><!-- This site is optimized with the Yoast SEO plugin v22.6 - https://yoast.com/wordpress/plugins/seo/ --></head></html>";
        assert!(has(&detected(&[], html), "Yoast SEO", Some("22.6")));
    }

    #[test]
    fn meta_and_script_attributes_are_read_as_parsed_values() {
        for html in [
            // a `>` inside a quoted attribute value does not end the tag
            r#"<meta data-note="x > y" name="generator" content="WordPress 6.5.2"><script data-note="x > y" src="/js/jquery-3.7.1.min.js"></script>"#,
            // character references are decoded
            r#"<meta name="generator" content="Word&#80;ress 6.5.2"><script src="/js/jquery&#x2d;3.7.1.min.js"></script>"#,
        ] {
            let found = detected(&[], html);
            assert!(has(&found, "WordPress", Some("6.5.2")), "{html}: {found:?}");
            assert!(has(&found, "jQuery", Some("3.7.1")), "{html}: {found:?}");
        }
    }

    #[test]
    fn detects_google_tag_manager_snippet() {
        let html = "<script>(function(w,d,s,l,i){j.src='https://www.googletagmanager.com/gtm.js?id='+i+dl;})(window,document,'script','dataLayer','GTM-ABC1234');</script>";
        assert!(has(&detected(&[], html), "Google Tag Manager", None));
    }

    #[test]
    fn built_in_server_headers_are_not_mistaken_for_a_stack() {
        let found = detected(
            &[
                ("x-powered-by", "siteone-crawler/2.6.1"),
                ("content-security-policy", "default-src 'self'"),
            ],
            "<html><body>Hello</body></html>",
        );
        assert!(found.is_empty(), "{found:?}");
    }

    #[test]
    fn analyzer_aggregates_pages_versions_and_evidence() {
        let mut analyzer = TechnologiesAnalyzer::new();
        let nginx = headers(&[("server", "nginx/1.25.3")]);
        analyzer.analyze_visited_url(
            &page("https://example.com/"),
            Some(r#"<script src="https://code.jquery.com/jquery-3.7.1.min.js"></script>"#),
            Some(&nginx),
        );
        analyzer.analyze_visited_url(
            &page("https://example.com/a"),
            Some(r#"<script src="/js/jquery-1.12.4.min.js"></script>"#),
            Some(&nginx),
        );
        // Assets are not pages: their headers must not be counted.
        let mut css = page("https://example.com/style.css");
        css.content_type = ContentTypeId::Stylesheet;
        analyzer.analyze_visited_url(&css, None, Some(&nginx));

        let rows = analyzer.table_rows();
        let row = |name: &str| rows.iter().find(|r| r["technology"] == name).expect(name);
        assert_eq!(row("Nginx")["category"], "Server");
        assert_eq!(row("Nginx")["version"], "1.25.3");
        assert_eq!(row("Nginx")["pages"], "2");
        assert_eq!(row("Nginx")["evidence"], "server: nginx/1.25.3");
        assert_eq!(row("jQuery")["version"], "1.12.4, 3.7.1");
        assert_eq!(row("jQuery")["pages"], "2");
        let position = |name: &str| rows.iter().position(|r| r["technology"] == name).unwrap();
        assert!(position("Nginx") < position("jQuery"), "rows are ordered by category");
    }

    #[test]
    fn versions_are_sorted_numerically() {
        let mut analyzer = TechnologiesAnalyzer::new();
        for (url, version) in [("https://example.com/", "10.0.0"), ("https://example.com/a", "9.1.0")] {
            let html = format!(r#"<script src="/js/jquery-{version}.min.js"></script>"#);
            analyzer.analyze_visited_url(&page(url), Some(&html), None);
        }
        let rows = analyzer.table_rows();
        let jquery = rows.iter().find(|r| r["technology"] == "jQuery").expect("jQuery row");
        assert_eq!(jquery["version"], "9.1.0, 10.0.0");
    }

    #[test]
    fn every_version_on_one_page_is_kept() {
        let modern = r#"<script src="/js/jquery-3.7.1.min.js"></script>"#;
        let legacy = r#"<script src="/js/jquery-1.12.4.min.js"></script>"#;
        for (html, first) in [
            (format!("{modern}{legacy}"), "script /js/jquery-3.7.1.min.js"),
            (format!("{legacy}{modern}"), "script /js/jquery-1.12.4.min.js"),
        ] {
            let mut analyzer = TechnologiesAnalyzer::new();
            analyzer.analyze_visited_url(&page("https://example.com/"), Some(&html), None);
            let rows = analyzer.table_rows();
            let jquery = rows.iter().find(|r| r["technology"] == "jQuery").expect("jQuery row");
            assert_eq!(jquery["version"], "1.12.4, 3.7.1", "{html}");
            assert_eq!(jquery["pages"], "1", "{html}");
            assert_eq!(jquery["evidence"], first, "{html}");
        }
    }

    #[test]
    fn every_version_of_an_html_marker_on_one_page_is_kept() {
        let html = r#"<link rel="stylesheet" href="https://cdnjs.cloudflare.com/ajax/libs/font-awesome/4.7.0/css/font-awesome.min.css">
            <link rel="stylesheet" href="https://cdnjs.cloudflare.com/ajax/libs/font-awesome/6.5.1/css/all.min.css">"#;
        let mut analyzer = TechnologiesAnalyzer::new();
        analyzer.analyze_visited_url(&page("https://example.com/"), Some(html), None);
        let rows = analyzer.table_rows();
        let icons = rows
            .iter()
            .find(|r| r["technology"] == "Font Awesome")
            .expect("Font Awesome row");
        assert_eq!(icons["version"], "4.7.0, 6.5.1");
        assert_eq!(icons["pages"], "1");
    }

    #[test]
    fn redirects_are_not_counted_as_pages() {
        let mut analyzer = TechnologiesAnalyzer::new();
        let nginx = headers(&[("server", "nginx/1.25.3")]);
        let mut redirect = page("https://example.com/old");
        redirect.status_code = 301;
        analyzer.analyze_visited_url(&redirect, Some("<html><body>Moved</body></html>"), Some(&nginx));
        analyzer.analyze_visited_url(&page("https://example.com/"), Some("<html></html>"), Some(&nginx));
        let rows = analyzer.table_rows();
        let row = rows.iter().find(|r| r["technology"] == "Nginx").expect("Nginx row");
        assert_eq!(row["pages"], "1");
    }

    #[test]
    fn evidence_never_shows_cookie_values_or_script_query_strings() {
        let found = detect(
            &SIGNATURES,
            &headers(&[(
                "set-cookie",
                "laravel_session=SECRET-SESSION; path=/; HttpOnly\n_ga=GA1.2.3",
            )]),
            r#"<script src="https://maps.googleapis.com/maps/api/js?key=SECRET-KEY&callback=init"></script>
                <script src="https://www.googletagmanager.com/gtm.js?id=GTM-SECRET"></script>"#,
        );
        let evidence: Vec<&str> = found.iter().map(|hit| hit.evidence.as_str()).collect();
        assert!(evidence.contains(&"cookie laravel_session"), "{evidence:?}");
        assert!(
            evidence.contains(&"script https://maps.googleapis.com/maps/api/js"),
            "{evidence:?}"
        );
        assert!(
            evidence.contains(&"script https://www.googletagmanager.com/gtm.js"),
            "{evidence:?}"
        );
        assert!(evidence.iter().all(|e| !e.contains("SECRET")), "{evidence:?}");
    }

    #[test]
    fn nothing_found_claims_nothing() {
        assert!(TechnologiesAnalyzer::new().table_rows().is_empty());
        assert!(
            !EMPTY_TABLE_MESSAGE.to_lowercase().contains("waf"),
            "never claim that no WAF is in use"
        );
    }
}
