// SiteOne Crawler - AI report HTML rendering
// (c) Jan Reges <jan.reges@siteone.cz>
//
// Renders an `AiReportModel` into a self-contained, light/dark, SiteOne-branded HTML report. The
// per-page table and the distribution bars are rendered server-side (work with no JavaScript and
// no network); inline JS only ENHANCES them with search / sort / filter + CSV export. Site-level
// charts are opt-in (`--ai-report-cdn`) via a pinned ECharts CDN + SRI and degrade to the table.

use crate::ai::report::{
    locale::ReportLocale,
    model::{AiReportModel, FieldType, Finding, ReportCell},
};

/// Pinned ECharts CDN + SRI (used only with `--ai-report-cdn`).
const ECHARTS_CDN_SRC: &str = "https://cdn.jsdelivr.net/npm/echarts@5.5.1/dist/echarts.min.js";
// SHA-384 of the pinned echarts@5.5.1 dist (verified: 1030855 bytes). A wrong hash fails CLOSED
// (the browser refuses the script) so charts would silently never render.
const ECHARTS_CDN_SRI: &str = "sha384-Mx5lkUEQPM1pOJCwFtUICyX45KNojXbkWdYhkKUKsbv391mavbfoAmONbzkgYPzR";

/// Categorical palette (Tableau-10 + gray), assigned to enum values in fixed order.
const CAT_COLORS: &[&str] = &[
    "#4e79a7", "#f28e2b", "#59a14f", "#e15759", "#b07aa1", "#76b7b2", "#ff9da7", "#9c755f", "#bab0ac",
];

/// Escape text for safe HTML embedding.
fn esc(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 8);
    for c in s.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&#39;"),
            _ => out.push(c),
        }
    }
    out
}

/// Deterministic color for an enum value: its index in the declared order, wrapped over the palette.
fn enum_color(field_ftype: &FieldType, value: &str) -> &'static str {
    if let FieldType::Enum(vals) = field_ftype
        && let Some(idx) = vals.iter().position(|v| v == value)
    {
        return CAT_COLORS[idx % CAT_COLORS.len()];
    }
    CAT_COLORS[CAT_COLORS.len() - 1]
}

/// Normalize a severity string to a stable CSS class token (never trusts arbitrary input).
fn sev_class(sev: &str) -> &'static str {
    match sev.to_lowercase().as_str() {
        "critical" => "critical",
        "high" => "high",
        "medium" => "medium",
        "low" => "low",
        _ => "info",
    }
}

/// Format a numeric rollup value compactly (integers without a trailing `.0`).
fn fmt_num(n: f64) -> String {
    if n.fract() == 0.0 && n.abs() < 1e15 {
        format!("{}", n as i64)
    } else {
        format!("{:.1}", n)
    }
}

/// Reserved status color for a severity (NEVER reused as a categorical series color).
fn sev_color(sev: &str) -> &'static str {
    match sev {
        "critical" => "#ef4444",
        "high" => "#f97316",
        "medium" => "#f59e0b",
        "low" => "#3b82f6",
        _ => "#6b7280",
    }
}

/// Render the full report HTML.
pub fn render(model: &AiReportModel, cdn: bool) -> String {
    let locale = ReportLocale::new(if model.site.report_language.is_empty() {
        "en"
    } else {
        &model.site.report_language
    });
    let mut h = String::with_capacity(64 * 1024);
    h.push_str(&format!(
        "<!DOCTYPE html>\n<html lang=\"{}\" data-theme=\"light\">\n<head>\n",
        esc(locale.code())
    ));
    h.push_str("<meta charset=\"utf-8\">\n<meta name=\"viewport\" content=\"width=device-width, initial-scale=1\">\n");
    h.push_str(&format!(
        "<title>{} — {}</title>\n",
        esc(&model.title),
        esc(&model.site.host)
    ));
    h.push_str("<style>\n");
    h.push_str(CSS);
    h.push_str("\n</style>\n</head>\n<body>\n");

    render_header(&mut h, model, &locale);
    render_hero_summary(&mut h, model, &locale);
    render_stat_tiles(&mut h, model, &locale);
    render_coverage(&mut h, model, &locale);
    render_methodology(&mut h, model, &locale);
    if cdn {
        render_charts_container(&mut h, model, &locale);
    }
    render_topic_analysis(&mut h, model, &locale);
    render_distributions(&mut h, model, &locale);
    render_compliance_findings(&mut h, model, &locale);
    render_table(&mut h, model, &locale);
    render_footer(&mut h, model, &locale);

    // Embedded data for CSV export + optional charts.
    h.push_str("<script type=\"application/json\" id=\"ai-data\">");
    h.push_str(&esc_json(&model.to_json().to_string()));
    h.push_str("</script>\n");
    h.push_str("<script>\n");
    h.push_str(JS);
    h.push_str("\n</script>\n");
    if cdn {
        h.push_str(&format!(
            "<script src=\"{}\" integrity=\"{}\" crossorigin=\"anonymous\" onload=\"renderCharts()\" onerror=\"chartsUnavailable()\"></script>\n",
            ECHARTS_CDN_SRC, ECHARTS_CDN_SRI
        ));
    }
    h.push_str("</body>\n</html>\n");
    h
}

/// The embedded JSON sits inside a <script> block; only `<` needs neutralizing to avoid an early
/// `</script>`. Keep it valid JSON otherwise.
fn esc_json(s: &str) -> String {
    s.replace('<', "\\u003c")
}

fn render_header(h: &mut String, m: &AiReportModel, locale: &ReportLocale) {
    h.push_str("<header class=\"report-bar\">\n<div class=\"brand\">");
    h.push_str(&format!(
        "<span class=\"logo\">◧ SiteOne</span> <span class=\"sub\">{}</span></div>\n",
        esc(locale.text("ai_report"))
    ));
    h.push_str("<div class=\"meta\">");
    h.push_str(&format!("<strong>{}</strong>", esc(&m.title)));
    h.push_str(&format!(
        " · <a href=\"{}\">{}</a>",
        esc(&m.site.url),
        esc(&m.site.host)
    ));
    if !m.site.model.is_empty() {
        h.push_str(&format!(" · {} / {}", esc(&m.site.provider), esc(&m.site.model)));
    }
    h.push_str(&format!(" · {} {}", m.analyzed_count(), esc(locale.text("pages"))));
    if !m.site.crawled_at.is_empty() {
        h.push_str(&format!(" · {}", esc(&m.site.crawled_at)));
    }
    h.push_str("</div>\n");
    h.push_str(&format!(
        "<div class=\"actions\"><button id=\"csvBtn\" class=\"btn\">{}</button>",
        esc(locale.text("export_csv"))
    ));
    h.push_str(&format!(
        "<button id=\"themeBtn\" class=\"btn icon-btn\" title=\"{}\" aria-label=\"{}\">◐</button></div>\n",
        esc(locale.text("toggle_theme")),
        esc(locale.text("toggle_theme"))
    ));
    h.push_str("</header>\n");
}

fn render_hero_summary(h: &mut String, m: &AiReportModel, locale: &ReportLocale) {
    let description = match (locale.is_czech(), m.preset.as_str()) {
        (true, "ia") => "Struktura, role a typy analyzovaných stránek.",
        (true, "quality") => "Srovnání kvality, srozumitelnosti a čitelnosti analyzovaného obsahu.",
        (true, "topics") => "Interní tematické pokrytí, překryvy a mezery ve vybraných stránkách.",
        (true, "compliance") => "Důkazně podložené poradní posouzení marketingových tvrzení a manipulativních vzorců.",
        (true, _) => "Strukturovaná extrakce z vybraných stránek.",
        (false, "ia") => "Structure, roles, and page types across the analyzed pages.",
        (false, "quality") => "A comparison of content quality, clarity, and readability.",
        (false, "topics") => "Internal topic coverage, overlaps, and gaps across the selected pages.",
        (false, "compliance") => "Evidence-grounded advisory review of marketing claims and manipulative patterns.",
        (false, _) => "Structured extraction across the selected pages.",
    };
    h.push_str("<main><section class=\"hero-summary\">\n");
    h.push_str(&format!(
        "<div><p class=\"eyebrow\">{}</p><h1>{}</h1><p class=\"hero-desc\">{}</p></div>",
        esc(&m.site.host),
        esc(&m.title),
        esc(description)
    ));
    h.push_str("<div class=\"hero-facts\">");
    match m.preset.as_str() {
        "compliance" => {
            if let Some(stats) = m.rollups.findings.values().next() {
                hero_fact(h, locale.text("findings"), stats.total_findings);
                hero_fact(h, locale.text("pages_flagged"), stats.pages_with_findings);
            }
        }
        "quality" => {
            if let Some(stats) = m.rollups.numeric.get("overall") {
                hero_fact_text(h, locale.text("avg_quality"), &format!("{:.0}/100", stats.avg));
                hero_fact_text(
                    h,
                    if locale.is_czech() { "Medián" } else { "Median" },
                    &format!("{:.0}/100", stats.median),
                );
            }
        }
        "topics" => {
            if let Some(narrative) = &m.narrative {
                hero_fact(h, locale.text("clusters"), json_array_len(narrative, "clusters"));
                hero_fact(
                    h,
                    locale.text("cannibalization"),
                    json_array_len(narrative, "cannibalizationCandidates"),
                );
                hero_fact(
                    h,
                    locale.text("internal_gaps"),
                    json_array_len(narrative, "internalCoverageGaps"),
                );
            }
        }
        "ia" => {
            let sections = m
                .rollups
                .distributions
                .get("section")
                .map(|items| items.iter().filter(|(_, count)| *count > 0).count())
                .unwrap_or(0);
            hero_fact(h, locale.text("sections"), sections);
            hero_fact(h, locale.text("pages_analyzed"), m.analyzed_count());
        }
        _ => {
            hero_fact(h, locale.text("pages_analyzed"), m.analyzed_count());
            hero_fact(h, locale.text("fields"), m.schema.len());
        }
    }
    h.push_str("</div></section>\n");
}

fn hero_fact(h: &mut String, label: &str, value: usize) {
    hero_fact_text(h, label, &value.to_string());
}

fn hero_fact_text(h: &mut String, label: &str, value: &str) {
    h.push_str(&format!(
        "<div class=\"hero-fact\"><strong>{}</strong><span>{}</span></div>",
        esc(value),
        esc(label)
    ));
}

fn json_array_len(value: &serde_json::Value, key: &str) -> usize {
    value.get(key).and_then(|item| item.as_array()).map_or(0, Vec::len)
}

fn render_coverage(h: &mut String, m: &AiReportModel, locale: &ReportLocale) {
    let coverage = &m.site.coverage;
    let status = if coverage.sampled {
        locale.text("sampled_report")
    } else {
        locale.text("complete_report")
    };
    let status_class = if coverage.sampled { "sampled" } else { "complete" };
    h.push_str("<section class=\"coverage section-band\">\n<div class=\"section-heading\">");
    h.push_str(&format!(
        "<h2>{}</h2><span class=\"coverage-status {}\">{}</span></div>",
        esc(locale.text("coverage")),
        status_class,
        esc(status)
    ));
    h.push_str("<div class=\"coverage-grid\">");
    coverage_metric(h, locale.text("crawled"), coverage.crawled_html);
    coverage_metric(h, locale.text("eligible"), coverage.eligible);
    coverage_metric(h, locale.text("selected"), coverage.selected);
    coverage_metric(h, locale.text("analyzed"), coverage.analyzed);
    coverage_metric(h, locale.text("failed"), coverage.failed);
    coverage_metric(h, locale.text("parse_failed"), coverage.parse_failed);
    coverage_metric(h, locale.text("request_failed"), coverage.request_failed);
    coverage_metric(h, locale.text("task_dropped"), coverage.task_dropped);
    coverage_metric(h, locale.text("excluded"), coverage.excluded);
    coverage_metric(h, locale.text("capped"), coverage.capped_dropped);
    coverage_metric(h, locale.text("context_failed"), coverage.context_failed);
    coverage_metric(h, locale.text("content_truncated"), coverage.content_truncated);
    coverage_metric(h, locale.text("indeterminate"), coverage.indeterminate);
    h.push_str("</div><dl class=\"coverage-details\">");
    detail(h, locale.text("ranking"), &coverage.ranking_method);
    detail(h, locale.text("include_masks"), &display_masks(&coverage.include_masks));
    detail(h, locale.text("exclude_masks"), &display_masks(&coverage.exclude_masks));
    detail(h, locale.text("report_language"), locale.code());
    let usage = &m.site.usage;
    let usage_label = if locale.is_czech() {
        "Využití AI pro tento report"
    } else {
        "AI usage for this report"
    };
    let usage_value = if locale.is_czech() {
        format!(
            "{} volání, {} HTTP pokusů, {} opakování, {} zásahů cache, {} vstupních + {} výstupních tokenů",
            usage.calls,
            usage.http_attempts,
            usage.retries,
            usage.cache_hits,
            usage.prompt_tokens,
            usage.completion_tokens
        )
    } else {
        format!(
            "{} calls, {} HTTP attempts, {} retries, {} cache hits, {} prompt + {} completion tokens",
            usage.calls,
            usage.http_attempts,
            usage.retries,
            usage.cache_hits,
            usage.prompt_tokens,
            usage.completion_tokens
        )
    };
    detail(h, usage_label, &usage_value);
    let cost_label = if locale.is_czech() {
        "Odhad ceny"
    } else {
        "Cost estimate"
    };
    let cost_value = match usage.estimated_cost_usd {
        Some(cost) => format!("${cost:.6} USD. {}", usage.cost_estimate_status),
        None if !usage.cost_estimate_status.is_empty() => usage.cost_estimate_status.clone(),
        None if locale.is_czech() => "Cena není k dispozici.".to_string(),
        None => "Cost is unavailable.".to_string(),
    };
    detail(h, cost_label, &cost_value);
    h.push_str("</dl></section>\n");
}

fn coverage_metric(h: &mut String, label: &str, value: usize) {
    h.push_str(&format!(
        "<div><strong>{}</strong><span>{}</span></div>",
        value,
        esc(label)
    ));
}

fn detail(h: &mut String, label: &str, value: &str) {
    h.push_str(&format!("<dt>{}</dt><dd>{}</dd>", esc(label), esc(value)));
}

fn display_masks(masks: &[String]) -> String {
    if masks.is_empty() {
        "-".to_string()
    } else {
        masks.join(", ")
    }
}

fn render_methodology(h: &mut String, m: &AiReportModel, locale: &ReportLocale) {
    if m.preset != "compliance" {
        return;
    }
    h.push_str("<section class=\"methodology section-band\"><h2>");
    h.push_str(&esc(locale.text("methodology")));
    h.push_str("</h2><p class=\"legal-disclaimer\"><strong>");
    h.push_str(&esc(locale.text("not_legal_advice")));
    h.push_str("</strong> ");
    h.push_str(&esc(locale.text("results_advisory")));
    h.push_str("</p><p>");
    h.push_str(&esc(locale.text("ccd2_timing")));
    h.push_str("</p><p>");
    h.push_str(&esc(locale.text("may_timing")));
    h.push_str("</p>");
    if let Some(rule_pack) = &m.site.rule_pack {
        h.push_str(&format!(
            "<p><strong>{}:</strong> <span class=\"mono\">{}</span></p>",
            esc(locale.text("rule_pack")),
            esc(&rule_pack.version)
        ));
        h.push_str(&format!(
            "<p><strong>{}:</strong> {}</p>",
            esc(locale.text("risk_formula")),
            esc(&rule_pack.risk_formula)
        ));
        h.push_str(&format!(
            "<p><strong>{}:</strong> {}</p>",
            esc(locale.text("applicability")),
            esc(&rule_pack.applicability)
        ));
        h.push_str(&format!(
            "<p><strong>{}:</strong> {}</p>",
            esc(locale.text("review_status")),
            esc(&rule_pack.review_status)
        ));
        h.push_str(&format!(
            "<p><strong>{}:</strong> ",
            esc(locale.text("primary_sources"))
        ));
        for (index, source) in rule_pack.sources.iter().enumerate() {
            if index > 0 {
                h.push_str(" · ");
            }
            h.push_str(&format!(
                "<a href=\"{}\" rel=\"noopener noreferrer\">{}</a>",
                esc(source),
                esc(source_host_label(source))
            ));
        }
        h.push_str("</p>");
    }
    h.push_str("</section>\n");
}

fn source_host_label(source: &str) -> &str {
    if source.contains("eur-lex.europa.eu") {
        "EUR-Lex"
    } else if source.contains("psp.cz") {
        "PSP ČR"
    } else {
        source
    }
}

fn render_stat_tiles(h: &mut String, m: &AiReportModel, locale: &ReportLocale) {
    h.push_str("<section class=\"tiles\">\n");
    tile(h, locale.text("pages_analyzed"), &m.analyzed_count().to_string(), "");
    tile(h, locale.text("fields"), &m.schema.len().to_string(), "");
    let errs = m.error_count();
    if errs > 0 {
        tile(h, locale.text("parse_errors"), &errs.to_string(), "");
    }
    // Preset-specific KPI.
    if let Some(fs) = m.rollups.findings.values().next() {
        // Compliance-style report: surface risk headline numbers.
        let high = fs
            .by_severity
            .iter()
            .filter(|(s, _)| s == "critical" || s == "high")
            .map(|(_, c)| *c)
            .sum::<usize>();
        tile(h, locale.text("findings"), &fs.total_findings.to_string(), "");
        tile(h, locale.text("high_critical"), &high.to_string(), "");
        tile(
            h,
            locale.text("pages_flagged"),
            &fs.pages_with_findings.to_string(),
            &format!("/{}", fs.pages_total),
        );
    } else if let Some(stats) = m.rollups.numeric.get("overall") {
        tile(h, locale.text("avg_quality"), &format!("{:.0}", stats.avg), "/100");
    } else if let Some(dist) = m.rollups.distributions.get("section") {
        let nonzero = dist.iter().filter(|(_, c)| *c > 0).count();
        tile(h, locale.text("sections"), &nonzero.to_string(), "");
    }
    h.push_str("</section>\n");
}

fn tile(h: &mut String, label: &str, value: &str, suffix: &str) {
    h.push_str(&format!(
        "<div class=\"tile\"><div class=\"tval\">{}<span class=\"tsuf\">{}</span></div><div class=\"tlbl\">{}</div></div>\n",
        esc(value),
        esc(suffix),
        esc(label)
    ));
}

fn render_charts_container(h: &mut String, m: &AiReportModel, locale: &ReportLocale) {
    h.push_str("<section class=\"charts\" id=\"charts\">\n");
    h.push_str(&format!(
        "<div id=\"heroChart\" class=\"chart\" data-hero=\"{}\"></div>\n",
        esc(&m.hero)
    ));
    let note = if locale.is_czech() {
        "Interaktivní graf není dostupný; podstatná data jsou zobrazena níže."
    } else {
        "The interactive chart is unavailable; the material data is rendered below."
    };
    h.push_str(&format!(
        "<div id=\"chartNote\" class=\"chart-note\" hidden>{}</div>\n",
        esc(note)
    ));
    h.push_str("</section>\n");
}

fn render_topic_analysis(h: &mut String, m: &AiReportModel, locale: &ReportLocale) {
    if m.preset != "topics" {
        return;
    }
    let Some(narrative) = &m.narrative else {
        return;
    };
    h.push_str("<section class=\"topic-analysis section-band\"><h2>");
    h.push_str(&esc(locale.text("topic_analysis")));
    h.push_str("</h2>");
    if let Some(methodology) = narrative.get("methodology").and_then(|value| value.as_str()) {
        h.push_str(&format!("<p class=\"analysis-method\">{}</p>", esc(methodology)));
    }

    h.push_str(&format!("<h3>{}</h3>", esc(locale.text("clusters"))));
    h.push_str("<div class=\"table-scroll\"><table class=\"analysis-table\"><thead><tr>");
    h.push_str(&format!(
        "<th>{}</th><th>{}</th><th>{}</th><th>{}</th>",
        esc(&locale.field_label("topicCluster")),
        esc(locale.text("pages")),
        esc(&locale.field_label("primaryTopic")),
        esc(&locale.field_label("funnelStage"))
    ));
    h.push_str("</tr></thead><tbody>");
    if let Some(clusters) = narrative.get("clusters").and_then(|value| value.as_array()) {
        for cluster in clusters {
            h.push_str("<tr>");
            h.push_str(&format!(
                "<td><strong>{}</strong></td><td class=\"num\">{}</td><td>{}</td><td>{}</td>",
                esc(json_text(cluster, "cluster")),
                cluster.get("pageCount").and_then(|value| value.as_u64()).unwrap_or(0),
                render_json_string_list(cluster.get("topics")),
                render_json_string_list(cluster.get("funnelStages"))
            ));
            h.push_str("</tr>");
        }
    }
    h.push_str("</tbody></table></div>");

    render_topic_candidates(
        h,
        locale.text("cannibalization"),
        narrative.get("cannibalizationCandidates"),
        "topic",
        "urls",
        locale,
    );
    render_topic_candidates(
        h,
        locale.text("internal_gaps"),
        narrative.get("internalCoverageGaps"),
        "cluster",
        "supportingUrls",
        locale,
    );
    render_topic_candidates(
        h,
        locale.text("thin_clusters"),
        narrative.get("thinClusters"),
        "cluster",
        "urls",
        locale,
    );
    h.push_str("</section>\n");
}

fn render_topic_candidates(
    h: &mut String,
    heading: &str,
    value: Option<&serde_json::Value>,
    title_key: &str,
    urls_key: &str,
    locale: &ReportLocale,
) {
    h.push_str(&format!("<h3>{}</h3>", esc(heading)));
    let Some(items) = value.and_then(|item| item.as_array()) else {
        h.push_str(&format!("<p class=\"muted\">{}</p>", esc(locale.text("no_candidates"))));
        return;
    };
    if items.is_empty() {
        h.push_str(&format!("<p class=\"muted\">{}</p>", esc(locale.text("no_candidates"))));
        return;
    }
    h.push_str("<ul class=\"analysis-list\">");
    for item in items {
        h.push_str("<li><strong>");
        h.push_str(&esc(json_text(item, title_key)));
        h.push_str("</strong>");
        if let Some(stages) = item.get("missingFunnelStages") {
            h.push_str(": ");
            h.push_str(&render_json_string_list(Some(stages)));
        }
        if let Some(reason) = item.get("reason").and_then(|reason| reason.as_str()) {
            h.push_str(&format!("<span>{}</span>", esc(reason)));
        }
        if let Some(urls) = item.get(urls_key).and_then(|urls| urls.as_array()) {
            h.push_str("<div class=\"url-list\">");
            for url in urls {
                if let Some(url) = url.as_str() {
                    h.push_str(&format!("<span class=\"mono\">{}</span>", esc(url)));
                }
            }
            h.push_str("</div>");
        }
        h.push_str("</li>");
    }
    h.push_str("</ul>");
}

fn json_text<'a>(value: &'a serde_json::Value, key: &str) -> &'a str {
    value.get(key).and_then(|item| item.as_str()).unwrap_or("")
}

fn render_json_string_list(value: Option<&serde_json::Value>) -> String {
    value
        .and_then(|item| item.as_array())
        .map(|items| {
            items
                .iter()
                .filter_map(|item| item.as_str())
                .map(|item| format!("<span class=\"tag\">{}</span>", esc(item)))
                .collect::<String>()
        })
        .unwrap_or_default()
}

fn render_compliance_findings(h: &mut String, m: &AiReportModel, locale: &ReportLocale) {
    if m.preset != "compliance" {
        return;
    }
    h.push_str("<section class=\"compliance-findings section-band\"><h2>");
    h.push_str(&esc(locale.text("validated_findings")));
    h.push_str("</h2>");
    let finding_count = m
        .rows
        .iter()
        .flat_map(|row| row.cells.values())
        .filter_map(|cell| match cell {
            ReportCell::Findings(items) => Some(items.len()),
            _ => None,
        })
        .sum::<usize>();
    if finding_count == 0 {
        h.push_str(&format!(
            "<p class=\"muted\">{}</p></section>\n",
            esc(locale.text("no_validated_findings"))
        ));
        return;
    }

    h.push_str("<div class=\"table-scroll\"><table class=\"findings-table\"><thead><tr>");
    for heading in [
        locale.text("page"),
        locale.text("severity"),
        locale.text("rule"),
        locale.text("status"),
        locale.text("excerpt"),
        locale.text("recommendation"),
    ] {
        h.push_str(&format!("<th>{}</th>", esc(heading)));
    }
    h.push_str("</tr></thead><tbody>");
    for row in &m.rows {
        for finding in row
            .cells
            .values()
            .filter_map(|cell| match cell {
                ReportCell::Findings(items) => Some(items.as_slice()),
                _ => None,
            })
            .flatten()
        {
            render_compliance_finding_row(h, row, finding, locale);
        }
    }
    h.push_str("</tbody></table></div></section>\n");
}

fn render_compliance_finding_row(
    h: &mut String,
    row: &crate::ai::report::model::ReportRow,
    finding: &Finding,
    locale: &ReportLocale,
) {
    let severity = sev_class(&finding.severity);
    h.push_str("<tr>");
    h.push_str(&format!(
        "<td class=\"path\"><span class=\"mono\">{}</span>",
        esc(&row.path)
    ));
    if row.evidence_status.as_deref() == Some("limited") {
        h.push_str(&format!(
            "<span class=\"evidence-badge\">{}</span>",
            esc(locale.text("evidence_limited"))
        ));
    }
    h.push_str("</td>");
    h.push_str(&format!(
        "<td><span class=\"sevtag sev-{}\">{}</span><div class=\"muted\">{}</div></td>",
        severity,
        esc(severity_label(locale, &finding.severity)),
        esc(&locale.value_label("category", &finding.category))
    ));
    h.push_str("<td>");
    h.push_str(&format!(
        "<strong>{}</strong><div class=\"mono muted\">{}</div><div>{}</div>",
        esc(if finding.title.is_empty() {
            &finding.rule
        } else {
            &finding.title
        }),
        esc(&finding.rule),
        esc(&finding.legal_basis)
    ));
    let sources = if finding.source_urls.is_empty() {
        if finding.source_url.is_empty() {
            Vec::new()
        } else {
            vec![&finding.source_url]
        }
    } else {
        finding.source_urls.iter().collect::<Vec<_>>()
    };
    for (index, source) in sources.into_iter().enumerate() {
        h.push_str(&format!(
            "<a class=\"source-link\" href=\"{}\" rel=\"noopener noreferrer\">{} {}</a>",
            esc(source),
            esc(locale.text("source")),
            index + 1
        ));
    }
    h.push_str("</td>");
    h.push_str(&format!(
        "<td><span class=\"obligation\">{}</span><div>{}</div><div class=\"muted\">{}: {}</div><div class=\"muted\"><strong>{}:</strong> {}</div></td>",
        esc(&finding.obligation),
        esc(&finding.legal_status),
        esc(locale.text("effective_date")),
        esc(&finding.effective_date),
        esc(locale.text("applicability")),
        esc(&finding.applicability)
    ));
    h.push_str(&format!(
        "<td><blockquote>{}</blockquote><div class=\"muted evidence-scope\">{}</div></td>",
        esc(&finding.excerpt),
        esc(&finding.evidence_scope)
    ));
    h.push_str(&format!("<td>{}</td></tr>", esc(&finding.recommendation)));
}

fn severity_label<'a>(locale: &ReportLocale, severity: &'a str) -> &'a str {
    if !locale.is_czech() {
        return severity;
    }
    match severity {
        "critical" => "kritická",
        "high" => "vysoká",
        "medium" => "střední",
        "low" => "nízká",
        "info" => "informativní",
        other => other,
    }
}

/// Server-side, pure-CSS distribution bars for every enum field + bool coverage (always present).
fn render_distributions(h: &mut String, m: &AiReportModel, locale: &ReportLocale) {
    let has_dist = !m.rollups.distributions.is_empty()
        || !m.rollups.bool_coverage.is_empty()
        || !m.rollups.findings.is_empty()
        || !m.rollups.numeric.is_empty()
        || !m.rollups.tag_freq.is_empty()
        || !m.rollups.string_freq.is_empty();
    if !has_dist {
        return;
    }
    h.push_str("<section class=\"dists\">\n");
    // Findings severity + category distributions (severity uses reserved status colors).
    for (name, fs) in &m.rollups.findings {
        if fs.total_findings == 0 {
            continue;
        }
        h.push_str(&format!(
            "<div class=\"dist\"><h3>{}: {}</h3>\n",
            esc(&locale.field_label(name)),
            esc(locale.text("severity"))
        ));
        for (sev, count) in &fs.by_severity {
            let pct = (*count as f64 / fs.total_findings as f64 * 100.0).round();
            h.push_str(&format!(
                "<div class=\"row\"><span class=\"k\">{}</span><span class=\"barwrap\"><span class=\"dist-bar-fill\" style=\"width:{}%;background:{}\"></span></span><span class=\"v\">{} ({:.0}%)</span></div>\n",
                esc(severity_label(locale, sev)), pct, sev_color(sev_class(sev)), count, pct
            ));
        }
        h.push_str("</div>\n");
        if !fs.by_category.is_empty() {
            h.push_str(&format!(
                "<div class=\"dist\"><h3>{}: {}</h3>\n",
                esc(&locale.field_label(name)),
                esc(locale.text("category"))
            ));
            for (i, (cat, count)) in fs.by_category.iter().take(12).enumerate() {
                let pct = (*count as f64 / fs.total_findings as f64 * 100.0).round();
                let color = CAT_COLORS[i % CAT_COLORS.len()];
                let label = locale.value_label("category", cat);
                h.push_str(&format!(
                    "<div class=\"row\"><span class=\"k\">{}</span><span class=\"barwrap\"><span class=\"dist-bar-fill\" style=\"width:{}%;background:{}\"></span></span><span class=\"v\">{} ({:.0}%)</span></div>\n",
                    esc(&label), pct, color, count, pct
                ));
            }
            h.push_str("</div>\n");
        }
    }
    for f in &m.schema {
        if let Some(dist) = m.rollups.distributions.get(&f.name) {
            let total: usize = dist.iter().map(|(_, c)| *c).sum();
            if total == 0 {
                continue;
            }
            h.push_str(&format!(
                "<div class=\"dist\"><h3>{}</h3>\n",
                esc(&locale.field_label(&f.name))
            ));
            for (val, count) in dist {
                let pct = (*count as f64 / total as f64 * 100.0).round();
                let color = enum_color(&f.ftype, val);
                let label = if val.is_empty() {
                    locale.text("unknown").to_string()
                } else {
                    locale.value_label(&f.name, val)
                };
                h.push_str(&format!(
                    "<div class=\"row\"><span class=\"k\">{}</span><span class=\"barwrap\"><span class=\"dist-bar-fill\" style=\"width:{}%;background:{}\"></span></span><span class=\"v\">{} ({:.0}%)</span></div>\n",
                    esc(&label), pct, color, count, pct
                ));
            }
            h.push_str("</div>\n");
        }
    }
    // Numeric fields: min/median/max + a 10-bucket histogram (sequential single hue).
    for f in &m.schema {
        if let Some(s) = m.rollups.numeric.get(&f.name) {
            let peak = s.histogram.iter().copied().max().unwrap_or(1).max(1);
            let (min_label, median_label, max_label) = if locale.is_czech() {
                ("minimum", "medián", "maximum")
            } else {
                ("min", "median", "max")
            };
            h.push_str(&format!(
                "<div class=\"dist\"><h3>{}: {} {} · {} {} · {} {}</h3>\n<div class=\"hist\">",
                esc(&locale.field_label(&f.name)),
                min_label,
                fmt_num(s.min),
                median_label,
                fmt_num(s.median),
                max_label,
                fmt_num(s.max)
            ));
            for (i, count) in s.histogram.iter().enumerate() {
                let hpct = (*count as f64 / peak as f64 * 100.0).round();
                let height = if *count == 0 { 0.0 } else { hpct.max(2.0) };
                let lo = s.bucket_min + (s.bucket_max - s.bucket_min) * (i as f64) / 10.0;
                h.push_str(&format!(
                    "<span class=\"hbar\" title=\"{}+ : {}\"><span class=\"hbarfill\" style=\"height:{}%\"></span></span>",
                    fmt_num(lo), count, height
                ));
            }
            h.push_str("</div></div>\n");
        }
    }
    for f in &m.schema {
        if let Some((t, fa)) = m.rollups.bool_coverage.get(&f.name) {
            let total = t + fa;
            if total == 0 {
                continue;
            }
            let pct = (*t as f64 / total as f64 * 100.0).round();
            h.push_str(&format!(
                "<div class=\"dist\"><h3>{}</h3><div class=\"row\"><span class=\"k\">true</span><span class=\"barwrap\"><span class=\"dist-bar-fill\" style=\"width:{}%;background:{}\"></span></span><span class=\"v\">{} / {}</span></div></div>\n",
                esc(&locale.field_label(&f.name)), pct, CAT_COLORS[2], t, total
            ));
        }
    }
    // Tag-frequency bars for string[] fields.
    for f in &m.schema {
        if let Some(tags) = m.rollups.tag_freq.get(&f.name) {
            if tags.is_empty() {
                continue;
            }
            let peak = tags.iter().map(|(_, c)| *c).max().unwrap_or(1).max(1);
            let top_tags = if locale.is_czech() {
                "nejčastější štítky"
            } else {
                "top tags"
            };
            h.push_str(&format!(
                "<div class=\"dist\"><h3>{}: {}</h3>\n",
                esc(&locale.field_label(&f.name)),
                esc(top_tags)
            ));
            for (i, (tag, count)) in tags.iter().take(12).enumerate() {
                let pct = (*count as f64 / peak as f64 * 100.0).round();
                let color = CAT_COLORS[i % CAT_COLORS.len()];
                h.push_str(&format!(
                    "<div class=\"row\"><span class=\"k\">{}</span><span class=\"barwrap\"><span class=\"dist-bar-fill\" style=\"width:{}%;background:{}\"></span></span><span class=\"v\">{}</span></div>\n",
                    esc(tag), pct, color, count
                ));
            }
            h.push_str("</div>\n");
        }
    }
    // Frequent short strings make topic clusters and primary topics useful without CDN charts.
    for f in &m.schema {
        if let Some(values) = m.rollups.string_freq.get(&f.name) {
            if values.len() < 2 {
                continue;
            }
            let peak = values.iter().map(|(_, count)| *count).max().unwrap_or(1).max(1);
            h.push_str(&format!(
                "<div class=\"dist\"><h3>{}</h3>\n",
                esc(&locale.field_label(&f.name))
            ));
            for (index, (value, count)) in values.iter().take(12).enumerate() {
                let pct = (*count as f64 / peak as f64 * 100.0).round();
                h.push_str(&format!(
                    "<div class=\"row\"><span class=\"k\" title=\"{}\">{}</span><span class=\"barwrap\"><span class=\"dist-bar-fill\" style=\"width:{}%;background:{}\"></span></span><span class=\"v\">{}</span></div>\n",
                    esc(value), esc(value), pct, CAT_COLORS[index % CAT_COLORS.len()], count
                ));
            }
            h.push_str("</div>\n");
        }
    }
    h.push_str("</section>\n");
}

fn render_table(h: &mut String, m: &AiReportModel, locale: &ReportLocale) {
    h.push_str("<section class=\"tablewrap\">\n");
    h.push_str(&format!(
        "<div class=\"tbar\"><input id=\"search\" class=\"search\" type=\"search\" placeholder=\"{}\">",
        esc(locale.text("search_placeholder"))
    ));
    h.push_str(&format!(
        "<span id=\"count\" class=\"count\" data-label=\"{}\"></span></div>\n<div class=\"table-scroll\">",
        esc(locale.text("pages"))
    ));
    h.push_str("<table id=\"tbl\">\n<thead><tr>");
    h.push_str(&format!("<th data-sort=\"str\">{}</th>", esc(locale.text("path"))));
    for f in &m.schema {
        let cls = if f.ftype.is_numeric() { "num" } else { "str" };
        h.push_str(&format!(
            "<th data-sort=\"{}\">{}</th>",
            cls,
            esc(&locale.field_label(&f.name))
        ));
    }
    h.push_str("</tr></thead>\n<tbody>\n");
    for row in &m.rows {
        if let Some(ref err) = row.error {
            // Errored page: NO fabricated values — one honest error cell spanning the fields.
            h.push_str("<tr class=\"errrow\">");
            h.push_str(&format!(
                "<td class=\"path\"><span class=\"mono\">{}</span></td>",
                esc(&row.path)
            ));
            h.push_str(&format!(
                "<td colspan=\"{}\" class=\"errcell\"><span class=\"errbadge\">{}</span> {}</td>",
                m.schema.len().max(1),
                esc(locale.text("error")),
                esc(err)
            ));
            h.push_str("</tr>\n");
            continue;
        }
        let limited = row.evidence_status.as_deref() == Some("limited");
        h.push_str(if limited { "<tr class=\"limited-row\">" } else { "<tr>" });
        h.push_str(&format!(
            "<td class=\"path\"><span class=\"mono\">{}</span>",
            esc(&row.path)
        ));
        if limited {
            h.push_str(&format!(
                "<span class=\"evidence-badge\">{}</span>",
                esc(locale.text("evidence_limited"))
            ));
        }
        h.push_str("</td>");
        for f in &m.schema {
            let cell = row.cells.get(&f.name);
            h.push_str(&render_cell(&f.name, f.ftype.clone(), cell, limited, locale));
        }
        h.push_str("</tr>\n");
    }
    h.push_str("</tbody>\n</table>\n</div></section>\n");
}

fn render_cell(
    field_name: &str,
    ftype: FieldType,
    cell: Option<&ReportCell>,
    evidence_limited: bool,
    locale: &ReportLocale,
) -> String {
    let cell = match cell {
        Some(c) => c,
        None => return "<td></td>".to_string(),
    };
    match (&ftype, cell) {
        (FieldType::Enum(_), ReportCell::Str(v)) => {
            if v.is_empty() {
                // Honest unknown — a muted dash, never a fabricated category chip.
                return "<td data-v=\"\"><span class=\"muted\">—</span></td>".to_string();
            }
            let color = enum_color(&ftype, v);
            let label = locale.value_label(field_name, v);
            format!(
                "<td data-v=\"{}\"><span class=\"chip\" style=\"--c:{}\">{}</span></td>",
                esc(v),
                color,
                esc(&label)
            )
        }
        (FieldType::Score, ReportCell::Num(n)) => {
            let pct = n.clamp(0.0, 100.0);
            format!(
                "<td data-v=\"{}\" class=\"num\"><span class=\"score\"><span class=\"scoreb\" style=\"width:{}%\"></span></span><span class=\"scoren\">{}</span></td>",
                n, pct, *n as i64
            )
        }
        (_, ReportCell::Num(n)) => {
            let disp = if n.fract() == 0.0 {
                format!("{}", *n as i64)
            } else {
                format!("{:.1}", n)
            };
            format!("<td data-v=\"{}\" class=\"num\">{}</td>", n, esc(&disp))
        }
        (_, ReportCell::Bool(b)) => {
            let (mark, cls) = if *b { ("✓", "yes") } else { ("✗", "no") };
            format!("<td data-v=\"{}\" class=\"bool {}\">{}</td>", b, cls, mark)
        }
        (_, ReportCell::List(items)) => {
            let tags: String = items
                .iter()
                .map(|t| format!("<span class=\"tag\">{}</span>", esc(t)))
                .collect();
            format!("<td data-v=\"{}\">{}</td>", esc(&items.join(", ")), tags)
        }
        (_, ReportCell::Findings(list)) => {
            if list.is_empty() {
                return if evidence_limited {
                    format!(
                        "<td class=\"findings limited\">{}</td>",
                        esc(locale.text("evidence_limited"))
                    )
                } else {
                    let clean = if locale.is_czech() { "bez nálezu" } else { "no finding" };
                    format!("<td class=\"findings ok\">✓ {}</td>", esc(clean))
                };
            }
            let mut inner = String::new();
            for fi in list {
                let sev = sev_class(&fi.severity);
                inner.push_str(&format!(
                    "<div class=\"finding sev-{}\"><span class=\"sevtag sev-{}\">{}</span> <span class=\"frule\">{}</span>",
                    sev,
                    sev,
                    esc(severity_label(locale, &fi.severity)),
                    esc(if fi.title.is_empty() { &fi.rule } else { &fi.title })
                ));
                if !fi.legal_basis.is_empty() {
                    inner.push_str(&format!(
                        "<div class=\"fbasis\">{} · {} {}</div>",
                        esc(&fi.legal_basis),
                        esc(&fi.obligation),
                        esc(&fi.legal_status)
                    ));
                }
                if !fi.recommendation.is_empty() {
                    inner.push_str(&format!("<div class=\"frec\">{}</div>", esc(&fi.recommendation)));
                }
                if !fi.excerpt.is_empty() {
                    inner.push_str(&format!("<div class=\"fexc\">“{}”</div>", esc(&fi.excerpt)));
                }
                inner.push_str("</div>");
            }
            // data-v = count, so the column sorts by number of findings.
            format!("<td class=\"findings\" data-v=\"{}\">{}</td>", list.len(), inner)
        }
        (FieldType::Url | FieldType::Path, ReportCell::Str(v)) => {
            format!("<td data-v=\"{}\"><span class=\"mono\">{}</span></td>", esc(v), esc(v))
        }
        (FieldType::Text, ReportCell::Str(v)) => {
            if v.chars().count() > 320 {
                let preview = v.chars().take(180).collect::<String>();
                format!(
                    "<td class=\"text\"><details><summary>{}...</summary><div>{}</div></details></td>",
                    esc(preview.trim_end()),
                    esc(v)
                )
            } else {
                format!("<td class=\"text\">{}</td>", esc(v))
            }
        }
        (_, ReportCell::Str(v)) => {
            format!("<td data-v=\"{}\">{}</td>", esc(v), esc(v))
        }
        (_, ReportCell::Null) => "<td></td>".to_string(),
    }
}

fn render_footer(h: &mut String, m: &AiReportModel, locale: &ReportLocale) {
    h.push_str("</main><footer class=\"foot\">\n");
    let line = if locale.is_czech() {
        format!(
            "AI report pro <strong>{}</strong> · {} {} · model {} ({}). {}",
            esc(&m.site.host),
            m.rows.len(),
            esc(locale.text("pages")),
            esc(&m.site.model),
            esc(&m.site.provider),
            esc(locale.text("results_advisory"))
        )
    } else {
        format!(
            "AI-generated report for <strong>{}</strong> · {} pages · model {} ({}). {}",
            esc(&m.site.host),
            m.rows.len(),
            esc(&m.site.model),
            esc(&m.site.provider),
            esc(locale.text("results_advisory"))
        )
    };
    h.push_str(&format!("<p>{}</p>\n", line));
    let generated = if locale.is_czech() {
        "Vygeneroval SiteOne Crawler."
    } else {
        "Generated by SiteOne Crawler."
    };
    h.push_str(&format!("<p class=\"gen\">{}</p>\n</footer>\n", esc(generated)));
}

const CSS: &str = r#":root{--bg:#f4f5f6;--surface:#fff;--ink:#18181b;--muted:#62666d;--border:#dfe2e5;--accent:#246b87;--chipbg:#edf0f2;--ok:#17803d;}
[data-theme="dark"]{--bg:#18181b;--surface:#242426;--ink:#f4f4f5;--muted:#a1a1aa;--border:#3f3f46;--accent:#72b8d4;--chipbg:#303034;--ok:#4ade80;}
*{box-sizing:border-box;letter-spacing:0}
html,body{max-width:100%}
body{margin:0;overflow-x:hidden;font:14px/1.5 -apple-system,BlinkMacSystemFont,"Segoe UI",Roboto,Helvetica,Arial,sans-serif;background:var(--bg);color:var(--ink)}
main{display:block}
a{color:var(--accent);text-decoration:none}a:hover{text-decoration:underline}
.report-bar{display:flex;align-items:center;gap:16px;flex-wrap:wrap;padding:14px clamp(16px,3vw,32px);background:var(--surface);border-bottom:1px solid var(--border)}
.brand{font-weight:700;white-space:nowrap}.logo{color:var(--accent)}.sub{color:var(--muted);font-weight:500}
.meta{color:var(--muted);flex:1;min-width:220px;overflow-wrap:anywhere}.meta strong{color:var(--ink)}
.actions{display:flex;gap:8px}.icon-btn{width:34px;padding:6px}
.btn{background:var(--chipbg);color:var(--ink);border:1px solid var(--border);border-radius:6px;padding:6px 12px;cursor:pointer;font-size:13px;white-space:nowrap}
.btn:hover{border-color:var(--accent)}
.hero-summary{min-height:230px;padding:clamp(34px,6vw,72px) clamp(20px,6vw,72px) 36px;display:flex;align-items:flex-end;justify-content:space-between;gap:32px;background:var(--surface);border-bottom:1px solid var(--border)}
.hero-summary>div:first-child{max-width:760px}.eyebrow{margin:0 0 8px;color:var(--accent);font-weight:700}.hero-summary h1{font-size:clamp(28px,4vw,48px);line-height:1.08;margin:0;overflow-wrap:anywhere}.hero-desc{font-size:16px;color:var(--muted);margin:14px 0 0;max-width:680px}
.hero-facts{display:flex;gap:22px;flex-wrap:wrap;justify-content:flex-end}.hero-fact{min-width:92px;border-left:2px solid var(--border);padding-left:12px}.hero-fact strong{display:block;font-size:24px}.hero-fact span{display:block;color:var(--muted);font-size:12px;max-width:150px}
.tiles{display:flex;gap:12px;flex-wrap:wrap;padding:20px clamp(16px,3vw,32px)}
.tile{background:var(--surface);border:1px solid var(--border);border-radius:8px;padding:14px 18px;min-width:130px;box-shadow:0 1px 2px rgba(0,0,0,.04)}
.tval{font-size:26px;font-weight:700}.tsuf{font-size:14px;color:var(--muted);font-weight:500}.tlbl{color:var(--muted);font-size:12px;text-transform:uppercase;margin-top:4px}
.section-band{padding:24px clamp(16px,3vw,32px);background:var(--surface);border-top:1px solid var(--border);border-bottom:1px solid var(--border);margin:0 0 20px}
.section-band h2{font-size:19px;margin:0 0 14px}.section-band h3{font-size:14px;margin:22px 0 8px}.section-heading{display:flex;align-items:center;gap:12px;flex-wrap:wrap}.section-heading h2{margin:0}
.coverage-status,.evidence-badge,.obligation{display:inline-block;border-radius:999px;padding:2px 8px;font-size:11px;font-weight:700}.coverage-status.complete{background:#dcfce7;color:#166534}.coverage-status.sampled{background:#fef3c7;color:#92400e}
[data-theme="dark"] .coverage-status.complete{background:#173b27;color:#86efac}[data-theme="dark"] .coverage-status.sampled{background:#493515;color:#fcd34d}
.coverage-grid{display:grid;grid-template-columns:repeat(auto-fit,minmax(115px,1fr));gap:1px;background:var(--border);border:1px solid var(--border);border-radius:8px;overflow:hidden;margin-top:14px}.coverage-grid>div{background:var(--surface);padding:12px}.coverage-grid strong{display:block;font-size:20px}.coverage-grid span{color:var(--muted);font-size:12px}
.coverage-details{display:grid;grid-template-columns:minmax(120px,190px) minmax(0,1fr);gap:5px 14px;margin:16px 0 0}.coverage-details dt{font-weight:600}.coverage-details dd{margin:0;color:var(--muted);overflow-wrap:anywhere}
.legal-disclaimer{border-left:4px solid #d97706;padding:10px 12px;background:rgba(217,119,6,.08)}
.charts{padding:0 clamp(16px,3vw,32px) 20px}.chart{height:360px;background:var(--surface);border:1px solid var(--border);border-radius:8px}.chart-note{color:var(--muted);padding:16px}
.analysis-method{color:var(--muted);max-width:900px}.analysis-list{padding-left:20px}.analysis-list li{margin:8px 0;min-width:0}.analysis-list li>span{display:block;color:var(--muted);margin-top:2px}.url-list{display:flex;flex-wrap:wrap;gap:4px 10px;margin-top:4px;color:var(--muted);min-width:0}.url-list .mono{max-width:100%;overflow-wrap:anywhere;word-break:break-word}
.dists{display:flex;gap:14px;flex-wrap:wrap;padding:8px clamp(16px,3vw,32px) 20px}
.dist{background:var(--surface);border:1px solid var(--border);border-radius:8px;padding:14px 16px;min-width:min(280px,100%);flex:1}
.dist h3{margin:0 0 10px;font-size:13px;text-transform:uppercase;color:var(--muted)}.dist .row{display:flex;align-items:center;gap:10px;margin:5px 0}.dist .k{width:120px;flex:none;font-size:13px;overflow:hidden;text-overflow:ellipsis;white-space:nowrap}.dist .barwrap{flex:1;background:var(--chipbg);border-radius:6px;height:12px;overflow:hidden}.dist-bar-fill{display:block;height:100%}.dist .v{width:88px;flex:none;text-align:right;color:var(--muted);font-size:12px}
.hist{display:flex;align-items:flex-end;gap:3px;height:70px;padding-top:4px}.hbar{flex:1;display:flex;align-items:flex-end;height:100%;background:var(--chipbg);border-radius:3px;overflow:hidden}.hbarfill{width:100%;background:var(--accent);border-radius:3px 3px 0 0}.muted{color:var(--muted)}
.tablewrap{padding:0 clamp(16px,3vw,32px) 40px;max-width:100%}.tbar{display:flex;align-items:center;gap:12px;margin:8px 0}.search{flex:1;max-width:360px;min-width:0;padding:8px 12px;border:1px solid var(--border);border-radius:6px;background:var(--surface);color:var(--ink)}.count{color:var(--muted);font-size:13px;white-space:nowrap}.table-scroll{max-width:100%;overflow-x:auto;overscroll-behavior-inline:contain}
table{width:100%;min-width:760px;border-collapse:collapse;background:var(--surface);border:1px solid var(--border)}thead th{position:sticky;top:0;background:var(--surface);text-align:left;padding:10px 12px;border-bottom:2px solid var(--border);font-size:12px;text-transform:uppercase;color:var(--muted);cursor:pointer;white-space:nowrap;z-index:2}thead th:hover{color:var(--ink)}tbody td{padding:9px 12px;border-bottom:1px solid var(--border);vertical-align:top;overflow-wrap:anywhere}tbody tr:hover{background:var(--chipbg)}
.analysis-table{min-width:680px}.findings-table{min-width:1180px}.findings-table td:nth-child(1){min-width:170px}.findings-table td:nth-child(3){min-width:250px}.findings-table td:nth-child(4){min-width:240px}.findings-table td:nth-child(5){min-width:260px}.findings-table td:nth-child(6){min-width:260px}
.mono{font-family:ui-monospace,SFMono-Regular,Menlo,monospace;font-size:12.5px}.path .evidence-badge{display:block;width:max-content;margin-top:6px;background:#fef3c7;color:#92400e}.text{max-width:520px;color:var(--ink)}.text summary{cursor:pointer}.text details div{margin-top:6px}td.num{text-align:right;font-variant-numeric:tabular-nums;white-space:nowrap}
.chip{display:inline-block;padding:2px 9px;border-radius:999px;font-size:12px;font-weight:600;color:#fff;background:var(--c)}.tag{display:inline-block;padding:1px 8px;margin:1px 3px 1px 0;border-radius:999px;font-size:11.5px;background:var(--chipbg);color:var(--ink)}.bool{text-align:center;font-weight:700}.bool.yes{color:var(--ok)}.bool.no{color:var(--muted)}
.findings{max-width:600px}.findings.ok{color:var(--ok);font-weight:600}.findings.limited{color:#a16207;font-weight:600}.finding{border-left:3px solid var(--border);padding:2px 0 6px 10px;margin:0 0 8px}.finding.sev-critical{border-color:#ef4444}.finding.sev-high{border-color:#f97316}.finding.sev-medium{border-color:#f59e0b}.finding.sev-low{border-color:#3b82f6}.finding.sev-info{border-color:#6b7280}
.sevtag{display:inline-block;padding:1px 7px;border-radius:999px;font-size:10.5px;font-weight:700;text-transform:uppercase;color:#fff}.sevtag.sev-critical{background:#dc2626}.sevtag.sev-high{background:#ea580c}.sevtag.sev-medium{background:#b77900}.sevtag.sev-low{background:#2563eb}.sevtag.sev-info{background:#6b7280}.frule{font-weight:600;font-size:12.5px}.fbasis,.frec{font-size:12.5px;margin-top:3px}.fbasis{color:var(--muted)}.fexc{font-size:12px;color:var(--muted);font-style:italic;margin-top:2px}
.obligation{background:var(--chipbg);margin-bottom:4px}.source-link{display:block;margin-top:4px}.findings-table blockquote{margin:0;border-left:3px solid var(--border);padding-left:9px;font-style:italic}.evidence-scope{font-size:11px;margin-top:5px}.limited-row td{background:rgba(217,119,6,.04)}
.errrow td{background:rgba(239,68,68,.06)}.errcell{color:var(--muted);font-size:12.5px}.errbadge{display:inline-block;padding:1px 8px;border-radius:999px;font-size:10.5px;font-weight:700;background:#dc2626;color:#fff;margin-right:6px}.score{display:inline-block;width:60px;height:8px;background:var(--chipbg);border-radius:5px;overflow:hidden;vertical-align:middle;margin-right:6px}.scoreb{display:block;height:100%;background:var(--accent)}.scoren{font-variant-numeric:tabular-nums}
.foot{padding:24px clamp(16px,3vw,32px);color:var(--muted);border-top:1px solid var(--border);background:var(--surface)}.foot strong{color:var(--ink)}.gen{font-size:12px;margin-top:4px}
@media(max-width:700px){.report-bar{align-items:flex-start}.brand{width:100%}.meta{order:3;flex:0 0 100%;width:100%;min-width:0}.actions{margin-left:auto}.hero-summary{min-height:0;display:block;padding:30px 18px}.hero-summary h1{font-size:30px}.hero-facts{justify-content:flex-start;margin-top:24px}.tiles{display:grid;grid-template-columns:repeat(2,minmax(0,1fr))}.tile{min-width:0;padding:12px}.coverage-details{grid-template-columns:1fr}.coverage-details dd{margin-bottom:8px}.dist{min-width:100%}.dist .k{width:92px}.dist .v{width:66px}.tbar{align-items:stretch}.search{max-width:none}.section-band{padding:20px 16px}.tablewrap{padding-left:12px;padding-right:12px}}
@media print{.actions,.tbar{display:none}.report-bar{position:static}.table-scroll{overflow:visible}table{min-width:0;font-size:10px}.section-band,.hero-summary{break-inside:avoid}.foot{border-top:1px solid #999}}"#;

const JS: &str = r#"(function(){
  var html=document.documentElement;
  // Initial theme from OS preference.
  try{ if(matchMedia('(prefers-color-scheme: dark)').matches) html.setAttribute('data-theme','dark'); }catch(e){}
  var tb=document.getElementById('themeBtn');
  if(tb) tb.addEventListener('click',function(){ html.setAttribute('data-theme', html.getAttribute('data-theme')==='dark'?'light':'dark'); if(window.renderCharts) window.renderCharts(); });

  var tbl=document.getElementById('tbl');
  var rows=tbl?Array.prototype.slice.call(tbl.tBodies[0].rows):[];
  var countEl=document.getElementById('count');
  function updateCount(){ if(countEl){ var vis=rows.filter(function(r){return r.style.display!=='none';}).length; countEl.textContent=vis+' / '+rows.length+' '+(countEl.getAttribute('data-label')||'pages'); } }
  updateCount();

  var search=document.getElementById('search');
  if(search) search.addEventListener('input',function(){
    var q=search.value.toLowerCase();
    rows.forEach(function(r){ r.style.display = r.textContent.toLowerCase().indexOf(q)>=0 ? '' : 'none'; });
    updateCount();
  });

  // Column sort.
  if(tbl){ Array.prototype.forEach.call(tbl.tHead.rows[0].cells,function(th,idx){
    var dir=1;
    th.addEventListener('click',function(){
      var num=th.getAttribute('data-sort')==='num';
      rows.sort(function(a,b){
        var av=cellVal(a.cells[idx],num), bv=cellVal(b.cells[idx],num);
        return av<bv?-1*dir:av>bv?1*dir:0;
      });
      dir*=-1;
      var tb=tbl.tBodies[0]; rows.forEach(function(r){tb.appendChild(r);});
    });
  }); }
  function cellVal(td,num){ if(!td)return num?0:''; var v=td.getAttribute('data-v'); if(v===null)v=td.textContent; return num?parseFloat(v)||0:(''+v).toLowerCase(); }

  // CSV export from the embedded model. Structured values remain lossless JSON, and errored pages
  // carry their honest error in `_error` (never a fabricated value).
  var csvBtn=document.getElementById('csvBtn');
  if(csvBtn) csvBtn.addEventListener('click',function(){
    var data=JSON.parse(document.getElementById('ai-data').textContent);
    var cols=['url','path'].concat(data.schema.map(function(f){return f.name;})).concat(['_evidence','_error']);
    var lines=[cols.map(csvCell).join(',')];
    data.pages.forEach(function(p){ lines.push(cols.map(function(c){ return csvCell(csvValue(p[c])); }).join(',')); });
    var blob=new Blob(['\uFEFF'+lines.join('\n')],{type:'text/csv;charset=utf-8'});
    var a=document.createElement('a'); var objectUrl=URL.createObjectURL(blob); a.href=objectUrl; a.download=(data.site.host||'ai-report')+'.csv'; a.click();
    setTimeout(function(){URL.revokeObjectURL(objectUrl);},0);
  });
  function csvValue(v){
    if(v==null) return '';
    if(typeof v==='object') return JSON.stringify(v);
    return ''+v;
  }
  function csvCell(v){ v=''+v; if(/^[\t\r\n]/.test(v)||/^[\u0000-\u0020]*[=+\-@]/.test(v))v="'"+v; return /[",\n\r]/.test(v)?'"'+v.replace(/"/g,'""')+'"':v; }

  // Charts (only when ECharts is loaded via CDN).
  window.chartsUnavailable=function(){ var n=document.getElementById('chartNote'); if(n) n.hidden=false; };
  window.renderCharts=function(){
    if(typeof echarts==='undefined') return;
    var el=document.getElementById('heroChart'); if(!el) return;
    var data=JSON.parse(document.getElementById('ai-data').textContent);
    var dark=html.getAttribute('data-theme')==='dark';
    var ink=dark?'#e5e7eb':'#111827';
    if(el._chart){el._chart.dispose();}
    var ch=echarts.init(el,null,{renderer:'canvas'}); el._chart=ch;
    var hero=el.getAttribute('data-hero');
    var opt;
    var pal=['#4e79a7','#f28e2b','#59a14f','#e15759','#b07aa1','#76b7b2','#ff9da7','#9c755f','#bab0ac'];
    if(hero==='treemap'){
      // Section → pageType treemap.
      var sec={};
      data.pages.forEach(function(p){ if(p._error)return; var s=p.section||'Other', t=p.pageType||'—'; sec[s]=sec[s]||{}; sec[s][t]=(sec[s][t]||0)+1; });
      var children=Object.keys(sec).map(function(s){ return {name:s,children:Object.keys(sec[s]).map(function(t){return {name:t,value:sec[s][t]};})}; });
      if(!children.length){chartsUnavailable();return;}
      opt={color:pal,tooltip:{},series:[{type:'treemap',roam:false,data:children,label:{color:'#fff'},breadcrumb:{show:false},levels:[{itemStyle:{borderColor:dark?'#1f2937':'#fff',borderWidth:2,gapWidth:2}}]}]};
    } else if(hero==='score-histogram'){
      var num=data.rollups.numeric.overall||Object.values(data.rollups.numeric)[0];
      if(!num){chartsUnavailable();return;}
      var labels=num.histogram.map(function(_,i){var lo=Math.round(num.bucketMin+(num.bucketMax-num.bucketMin)*i/10);return lo+'+';});
      opt={color:pal,tooltip:{},xAxis:{type:'category',data:labels,axisLabel:{color:ink}},yAxis:{type:'value',axisLabel:{color:ink}},series:[{type:'bar',data:num.histogram}]};
    } else if(hero==='severity-heatmap'){
      var fk=Object.keys(data.rollups.findings||{})[0];
      var fs=fk?data.rollups.findings[fk]:null;
      if(!fs||!fs.totalFindings){chartsUnavailable();return;}
      var sevColors={critical:'#ef4444',high:'#f97316',medium:'#f59e0b',low:'#3b82f6',info:'#6b7280'};
      var sd=fs.bySeverity;
      opt={tooltip:{},xAxis:{type:'category',data:sd.map(function(x){return x.severity;}),axisLabel:{color:ink}},yAxis:{type:'value',axisLabel:{color:ink}},series:[{type:'bar',data:sd.map(function(x){return {value:x.count,itemStyle:{color:sevColors[x.severity]||'#6b7280'}};})}]};
    } else if(hero==='topic-map'){
      var topics=(data.rollups.stringFrequency||{}).topicCluster||(data.rollups.stringFrequency||{}).primaryTopic;
      if(!topics||!topics.length){chartsUnavailable();return;}
      topics=topics.slice(0,12);
      opt={color:pal,tooltip:{},grid:{left:30,right:20,bottom:90,top:20},xAxis:{type:'category',data:topics.map(function(x){return x.value;}),axisLabel:{color:ink,rotate:30,interval:0}},yAxis:{type:'value',axisLabel:{color:ink}},series:[{type:'bar',data:topics.map(function(x){return x.count;})}]};
    } else {
      // Generic: first enum distribution as a bar.
      var dk=Object.keys(data.rollups.distributions)[0];
      if(!dk){chartsUnavailable();return;}
      var d=data.rollups.distributions[dk];
      opt={color:pal,tooltip:{},xAxis:{type:'category',data:d.map(function(x){return x.value;}),axisLabel:{color:ink,rotate:30}},yAxis:{type:'value',axisLabel:{color:ink}},series:[{type:'bar',data:d.map(function(x){return x.count;})}]};
    }
    ch.setOption(opt);
    window.addEventListener('resize',function(){ch.resize();});
  };
})();"#;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ai::report::model::{CoverageMeta, FieldSpec, Finding, ReportRow, RulePackMeta, SiteMeta};
    use std::collections::BTreeMap;

    fn sample_model() -> AiReportModel {
        let schema = vec![
            FieldSpec::new("title", FieldType::Text),
            FieldSpec::new(
                "section",
                FieldType::Enum(vec!["Blog".into(), "Docs".into(), "Other".into()]),
            ),
            FieldSpec::new("quality", FieldType::Score),
        ];
        let mut site = SiteMeta::default();
        site.host = "example.com".into();
        site.url = "https://example.com/".into();
        let mut m = AiReportModel::new("ia", "Information Architecture Inventory", site, schema, "treemap");
        let mut cells = BTreeMap::new();
        cells.insert("title".to_string(), ReportCell::Str("Hello <World>".into()));
        cells.insert("section".to_string(), ReportCell::Str("Blog".into()));
        cells.insert("quality".to_string(), ReportCell::Num(72.0));
        m.rows
            .push(ReportRow::ok("https://example.com/a".into(), "/a".into(), cells));
        m.build_rollups();
        m
    }

    #[test]
    fn renders_self_contained_html_with_table() {
        let html = render(&sample_model(), false);
        assert!(html.starts_with("<!DOCTYPE html>"));
        assert!(html.contains("Information Architecture Inventory"));
        assert!(html.contains("<table"));
        assert!(html.contains("/a")); // path present
        assert!(html.contains("class=\"chip\"")); // enum chip
        // XSS-safe: injected angle brackets are escaped, never raw.
        assert!(!html.contains("Hello <World>"));
        assert!(html.contains("Hello &lt;World&gt;"));
        // No external network reference when cdn=false.
        assert!(!html.contains("cdn.jsdelivr.net"));
    }

    #[test]
    fn cdn_mode_adds_pinned_script_with_sri() {
        let html = render(&sample_model(), true);
        assert!(html.contains("cdn.jsdelivr.net/npm/echarts"));
        assert!(html.contains("integrity=\"sha384-"));
        assert!(html.contains("id=\"heroChart\""));
    }

    #[test]
    fn embedded_json_cannot_break_script() {
        let html = render(&sample_model(), false);
        // The embedded model JSON must not contain a raw "</script"
        assert!(!html.contains("</script>example")); // sanity
        assert!(html.contains("id=\"ai-data\""));
    }

    #[test]
    fn compliance_report_localizes_chrome_and_keeps_full_evidence() {
        let schema = vec![
            FieldSpec::new("riskScore", FieldType::Score),
            FieldSpec::new("findings", FieldType::Findings),
        ];
        let site = SiteMeta {
            host: "example.cz".into(),
            url: "https://example.cz/".into(),
            report_language: "cs-CZ".into(),
            coverage: CoverageMeta {
                crawled_html: 10,
                eligible: 8,
                selected: 3,
                analyzed: 3,
                sampled: true,
                ..CoverageMeta::default()
            },
            rule_pack: Some(RulePackMeta {
                version: "test-pack".into(),
                sources: vec!["https://eur-lex.europa.eu/example".into()],
                risk_formula: "high=25".into(),
                applicability: "Spotřebitelské úvěry v působnosti CCD2".into(),
                review_status: "Vyžaduje ověření kvalifikovaným právníkem".into(),
            }),
            ..SiteMeta::default()
        };
        let finding = Finding {
            severity: "high".into(),
            category: "advertising_claims".into(),
            rule: "false_availability".into(),
            excerpt: "Schválíme každému".into(),
            recommendation: "Tvrzení odstraňte.".into(),
            title: "Falešné očekávání schválení".into(),
            legal_basis: "CCD2 článek 7".into(),
            obligation: "SHALL".into(),
            legal_status: "Příprava na CCD2".into(),
            effective_date: "2026-11-20".into(),
            source_url: "https://eur-lex.europa.eu/example".into(),
            source_urls: vec!["https://eur-lex.europa.eu/example".into()],
            applicability: "Spotřebitelské úvěry v působnosti CCD2".into(),
            evidence_scope: "Podloženo úplným vstupem".into(),
        };
        let cells = BTreeMap::from([
            ("riskScore".into(), ReportCell::Num(25.0)),
            ("findings".into(), ReportCell::Findings(vec![finding])),
        ]);
        let mut model = AiReportModel::new("compliance", "Regulatorní audit", site, schema, "severity-heatmap");
        model.rows.push(ReportRow::ok(
            "https://example.cz/pujcka".into(),
            "/pujcka".into(),
            cells,
        ));
        model.build_rollups();

        let html = render(&model, false);
        assert!(html.contains("<html lang=\"cs-CZ\""));
        assert!(html.contains("Výběrový report"));
        assert!(html.contains("nikoli právní stanovisko"));
        assert!(html.contains("class=\"findings-table\""));
        assert!(html.contains("Schválíme každému"));
        assert!(html.contains("CCD2 článek 7"));
        assert!(html.contains("Tvrzení odstraňte."));
        assert!(html.contains("reklamní tvrzení"));
    }

    #[test]
    fn offline_html_has_server_summary_responsive_table_and_safe_csv() {
        let html = render(&sample_model(), false);
        assert!(html.contains("class=\"hero-summary\""));
        assert!(html.contains(".table-scroll{max-width:100%;overflow-x:auto"));
        assert!(html.contains(".meta{order:3;flex:0 0 100%;width:100%;min-width:0}"));
        assert!(html.contains("dist-bar-fill"));
        assert!(!html.contains("<header class=\"bar\""));
        assert!(html.contains("'\\uFEFF'"));
        assert!(html.contains("/^[\\u0000-\\u0020]*[=+\\-@]/"));
        assert!(html.contains("JSON.stringify(v)"));
    }
}
