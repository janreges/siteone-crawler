// SiteOne Crawler - AI report-summary HTML rendering
// (c) Jan Reges <jan.reges@siteone.cz>
//
// Renders the synthesized executive summary as a visually styled box (".ai-box") for the
// report's Summary tab, matching the look of the Website Quality Score box (light/dark).

use super::extract::AREAS;
use super::{AreaAssessment, Recommendation, ReportSummary};

/// Map any area label the model emits to one of the five canonical areas (for filter ids).
fn normalize_area(s: &str) -> String {
    let l = s.trim().to_lowercase();
    if l.contains("secur") {
        "security".to_string()
    } else if l.contains("access") || l.contains("a11y") || l.contains("best") {
        "accessibility".to_string()
    } else if l.contains("seo") {
        "seo".to_string()
    } else if l.contains("perf") || l.contains("cach") || l.contains("speed") {
        "performance".to_string()
    } else if l.contains("infra") || l.contains("crawl") || l.contains("data") {
        "infrastructure".to_string()
    } else {
        l
    }
}

/// Word-boundary-aware substring check: `needle` must appear in `haystack` not flanked by ASCII
/// alphanumerics on the side(s) where the needle itself starts/ends with an alphanumeric. A
/// trailing plural "s" is tolerated (so "redirect"/"gif" still match "redirects"/"gifs"). This
/// prevents false matches like "aria" inside "variant" or "gif" inside "gift", while still
/// matching plurals and hyphen/colon-bounded markers ("x-frame" in "x-frame-options", "og:" in
/// "og:title").
fn contains_word(haystack: &str, needle: &str) -> bool {
    if needle.is_empty() {
        return false;
    }
    let bytes = haystack.as_bytes();
    let nb = needle.as_bytes();
    let check_before = nb[0].is_ascii_alphanumeric();
    let check_after = nb[nb.len() - 1].is_ascii_alphanumeric();
    let at_boundary = |idx: usize| idx >= bytes.len() || !bytes[idx].is_ascii_alphanumeric();
    let mut start = 0;
    while let Some(pos) = haystack[start..].find(needle) {
        let i = start + pos;
        let end = i + needle.len();
        let before_ok = !check_before || i == 0 || !bytes[i - 1].is_ascii_alphanumeric();
        // Boundary right after the needle, OR a single plural "s" then a boundary.
        let after_ok = !check_after || at_boundary(end) || (bytes[end] == b's' && at_boundary(end + 1));
        if before_ok && after_ok {
            return true;
        }
        start = i + 1;
    }
    false
}

/// Clamp any area string to exactly one of the five canonical areas (default: infrastructure),
/// so a card always carries a chip-matchable area even when the model emits an off-taxonomy one.
fn clamp_area(s: &str) -> String {
    match s {
        "security" | "accessibility" | "seo" | "performance" | "infrastructure" => s.to_string(),
        _ => "infrastructure".to_string(),
    }
}

/// Deterministic area classification from the recommendation text (title + body). This is a
/// belt-and-suspenders correction for the synthesis occasionally mis-tagging an area: strong,
/// unambiguous topic keywords win; otherwise we keep the model's own (normalized) area.
fn classify_area(text: &str, model_area: &str) -> String {
    let t = text.to_lowercase();
    let has = |kws: &[&str]| kws.iter().any(|k| contains_word(&t, k));
    // Order matters: most specific / least ambiguous topics first.
    if has(&[
        "csp",
        "content-security-policy",
        "content security policy",
        "hsts",
        "strict-transport",
        "x-frame",
        "x-xss",
        "x-content-type-options",
        "x-content",
        "nosniff",
        "clickjack",
        "coop",
        "coep",
        "cross-origin",
        "cors",
        "tls",
        "ssl/tls",
        "ssl",
        "mixed content",
        "secure cookie",
        "samesite",
        "httponly",
        "referrer-policy",
        "permissions-policy",
    ]) {
        "security".to_string()
    } else if has(&[
        "404",
        "5xx",
        "broken link",
        "broken internal",
        "redirect",
        "ipv6",
        "ipv4",
        "aaaa record",
        "dns",
        "external domain",
        "external dependenc",
        "content-type",
        "skipped url",
        "robots.txt",
    ]) {
        "infrastructure".to_string()
    } else if has(&[
        "alt text",
        "alt attribute",
        "aria",
        "heading level",
        "skipped heading",
        "heading hierarchy",
        "heading structure",
        "landmark",
        "form label",
        "html lang",
        "lang attribute",
        "wcag",
        "contrast",
        "dom depth",
        "accessib",
        "screen reader",
        "skip link",
        "focus order",
        "tab order",
        "keyboard navigation",
        "invalid svg",
        "smart quote",
    ]) {
        "accessibility".to_string()
    } else if has(&[
        "canonical",
        "meta description",
        "meta-description",
        "meta keyword",
        "title tag",
        "title length",
        "noindex",
        "indexab",
        "opengraph",
        "open graph",
        "og:",
        "twitter card",
        "duplicate title",
        "duplicate description",
        "sitemap",
        "rel=canonical",
    ]) {
        "seo".to_string()
    } else if has(&[
        "brotli",
        "gzip",
        "compress",
        "webp",
        "avif",
        "gif",
        "image weight",
        "image size",
        "oversized",
        "max-age",
        "cache-control",
        "cache lifetime",
        "immutable",
        "page weight",
        "payload",
        "lazy load",
        "minif",
        "response time",
        "largest contentful",
    ]) {
        "performance".to_string()
    } else {
        clamp_area(&normalize_area(model_area))
    }
}

/// A (area + fine topic) signature used to drop GENUINE duplicate recommendations. Computed
/// from the recommendation TITLE only (never the body) and split into fine topics, so two
/// DISTINCT recommendations whose bodies happen to mention the same keyword are not collapsed
/// (e.g. an X-XSS-Protection card whose body mentions CSP must not erase the CSP card; a
/// "missing meta description" card must not erase a "meta description length" card).
fn topic_signature(area: &str, title: &str) -> Option<String> {
    let t = title.to_lowercase();
    let topic = if t.contains("meta description") || t.contains("meta-description") {
        if t.contains("missing") || t.contains("add") || t.contains("absent") || t.contains("without") {
            "meta-desc-missing"
        } else {
            "meta-desc-length"
        }
    } else if t.contains("canonical") {
        "canonical"
    } else if t.contains("404")
        || t.contains("broken link")
        || t.contains("broken internal")
        || t.contains("broken external")
    {
        "broken-links"
    } else if t.contains("hsts") || t.contains("strict-transport") {
        "hsts"
    } else if t.contains("x-content") || t.contains("nosniff") {
        "x-content-type"
    } else if t.contains("x-frame") || t.contains("clickjack") {
        "x-frame"
    } else if t.contains("x-xss") {
        "x-xss"
    } else if t.contains("content-security") || t.contains("csp") {
        "csp"
    } else if t.contains("cross-origin") || t.contains("coop") || t.contains("coep") {
        "cross-origin"
    } else if t.contains("brotli") || t.contains("gzip") || (t.contains("compress") && !t.contains("image")) {
        "text-compression"
    } else if t.contains("webp")
        || t.contains("avif")
        || t.contains("gif")
        || t.contains("image format")
        || t.contains("oversized image")
        || (t.contains("image") && t.contains("compress"))
    {
        "image-format"
    } else if t.contains("h1") {
        "h1"
    } else if t.contains("heading") {
        "heading-levels"
    } else if t.contains("ipv6") {
        "ipv6"
    } else if t.contains("max-age")
        || t.contains("cache lifetime")
        || t.contains("cache-control")
        || t.contains("caching")
    {
        "caching"
    } else if t.contains("title length") || t.contains("title tag") {
        "title"
    } else {
        return None;
    };
    Some(format!("{}|{}", area, topic))
}

/// Render the AI executive summary box. Returns empty string if there is nothing to show.
pub fn render_html(report: &ReportSummary, assessments: &[AreaAssessment]) -> String {
    if report.overall_assessment.trim().is_empty() && report.recommendations.is_empty() {
        return String::new();
    }

    let mut h = String::new();
    h.push_str(STYLE);
    h.push_str("<div class=\"ai-box\">\n");
    h.push_str("<div class=\"ai-head\">\n");
    h.push_str("<h3 class=\"ai-title\">\u{1F916} AI Insights &amp; Recommendations</h3>\n");
    if !report.overall_grade.trim().is_empty() {
        let (gc, _) = grade_style(&report.overall_grade);
        h.push_str(&format!(
            "<span class=\"ai-grade\" style=\"background:{};\">{}</span>\n",
            gc,
            esc(report.overall_grade.trim())
        ));
    }
    h.push_str("</div>\n");

    if !report.overall_assessment.trim().is_empty() {
        h.push_str(&format!(
            "<p class=\"ai-overall\">{}</p>\n",
            esc(report.overall_assessment.trim())
        ));
    }

    // Per-area grade chips — clickable filter toggles (checked by default). Clicking a chip
    // hides/shows the recommendation cards of that area (pure CSS via :has()).
    if !assessments.is_empty() {
        h.push_str("<div class=\"ai-chips\">\n");
        h.push_str("<span class=\"ai-chips-hint\">Filter by area:</span>\n");
        for canonical in AREAS {
            if let Some(a) = assessments.iter().find(|a| normalize_area(&a.area) == canonical) {
                let (gc, _) = grade_style(&a.grade);
                h.push_str(&format!(
                    "<label class=\"ai-chip\" for=\"ai-f-{area}\"><input type=\"checkbox\" id=\"ai-f-{area}\" class=\"ai-fl\" checked><span class=\"ai-chip-area\">{name}</span><span class=\"ai-chip-grade\" style=\"background:{gc};\">{grade}</span></label>\n",
                    area = canonical,
                    name = esc(&title_case(canonical)),
                    gc = gc,
                    grade = esc(if a.grade.is_empty() { "?" } else { a.grade.trim() })
                ));
            }
        }
        h.push_str("</div>\n");
    }

    // Recommendation cards — deduplicated by (area + topic) so the model emitting two
    // recommendations for the same underlying issue collapses to one (highest severity first).
    let mut seen = std::collections::HashSet::new();
    let recs: Vec<&Recommendation> = report
        .recommendations
        .iter()
        .filter(|r| {
            // Area uses title+body (more signal); the dedup signature uses the TITLE only so an
            // incidental keyword in one card's body cannot suppress a distinct card.
            let area = classify_area(&format!("{} {}", r.title, r.recommendation), &r.area);
            match topic_signature(&area, &r.title) {
                Some(sig) => seen.insert(sig),
                None => true,
            }
        })
        .collect();
    if !recs.is_empty() {
        h.push_str("<div class=\"ai-recs\">\n");
        for r in recs {
            h.push_str(&render_rec(r));
        }
        h.push_str("</div>\n");
    }

    h.push_str("<p class=\"ai-note\">Generated by AI from the crawl data — advisory; verify before acting.</p>\n");
    h.push_str("</div>\n");
    h
}

fn render_rec(r: &Recommendation) -> String {
    let (color, icon) = severity_style(&r.severity);
    let title = if r.title.trim().is_empty() {
        r.recommendation.trim()
    } else {
        r.title.trim()
    };
    // Deterministically (re)classify the area from the recommendation text so the chip filter
    // and the displayed tag are always correct, even if the model mis-tagged it.
    let area = classify_area(&format!("{} {}", r.title, r.recommendation), &r.area);
    let mut card = String::new();
    card.push_str(&format!(
        "<div class=\"ai-rec\" data-area=\"{}\" style=\"border-left-color:{};\">\n",
        area, color
    ));
    card.push_str("<div class=\"ai-rec-h\">\n");
    card.push_str(&format!("<span class=\"ai-rec-icon\">{}</span>\n", icon));
    card.push_str(&format!("<span class=\"ai-rec-title\">{}</span>\n", esc(title)));
    if !area.is_empty() {
        card.push_str(&format!(
            "<span class=\"ai-rec-area\">{}</span>\n",
            esc(&title_case(&area))
        ));
    }
    card.push_str("</div>\n");
    if !r.recommendation.trim().is_empty() && !r.title.trim().is_empty() {
        card.push_str(&format!(
            "<div class=\"ai-rec-body\">{}</div>\n",
            esc(r.recommendation.trim())
        ));
    }
    if !r.impact.trim().is_empty() {
        card.push_str(&format!(
            "<div class=\"ai-rec-meta\"><b>Impact:</b> {}</div>\n",
            esc(r.impact.trim())
        ));
    }
    if !r.evidence.trim().is_empty() {
        card.push_str(&format!(
            "<div class=\"ai-rec-meta\"><b>Evidence:</b> {}</div>\n",
            esc(r.evidence.trim())
        ));
    }
    card.push_str("</div>\n");
    card
}

fn severity_style(sev: &str) -> (&'static str, &'static str) {
    match sev.trim().to_lowercase().as_str() {
        "critical" => ("#ef4444", "\u{26D4}"),
        "high" => ("#f97316", "\u{1F534}"),
        "medium" | "warning" => ("#f59e0b", "\u{26A0}\u{FE0F}"),
        "low" | "notice" => ("#3b82f6", "\u{1F539}"),
        _ => ("#9ca3af", "\u{1F4CC}"),
    }
}

fn grade_style(grade: &str) -> (&'static str, &'static str) {
    match grade.trim().to_uppercase().chars().next().unwrap_or('?') {
        'A' => ("#22c55e", "A"),
        'B' => ("#84cc16", "B"),
        'C' => ("#f59e0b", "C"),
        'D' => ("#f97316", "D"),
        'F' => ("#ef4444", "F"),
        _ => ("#9ca3af", "?"),
    }
}

fn title_case(s: &str) -> String {
    s.split(['-', '_', ' '])
        .filter(|w| !w.is_empty())
        .map(|w| {
            let mut c = w.chars();
            match c.next() {
                Some(f) => f.to_uppercase().collect::<String>() + c.as_str(),
                None => String::new(),
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

fn esc(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn esc_escapes_html() {
        assert_eq!(esc("<script>&\"x\""), "&lt;script&gt;&amp;&quot;x&quot;");
    }

    #[test]
    fn render_escapes_model_output() {
        let report = ReportSummary {
            overall_assessment: "<img src=x onerror=alert(1)>".to_string(),
            overall_grade: "C".to_string(),
            recommendations: vec![Recommendation {
                area: "security".to_string(),
                severity: "critical".to_string(),
                title: "<b>bad</b>".to_string(),
                recommendation: "fix </div> it".to_string(),
                ..Default::default()
            }],
        };
        let html = render_html(&report, &[]);
        assert!(!html.contains("<img src=x"));
        assert!(html.contains("&lt;img"));
        assert!(!html.contains("<b>bad</b>"));
        assert!(html.contains("&lt;b&gt;bad"));
    }

    #[test]
    fn empty_report_renders_nothing() {
        assert!(render_html(&ReportSummary::default(), &[]).is_empty());
    }

    #[test]
    fn classify_area_corrects_mistags() {
        // The exact mis-tags observed from both models must land in the right area.
        assert_eq!(classify_area("Add rel=canonical tags", "security"), "seo");
        assert_eq!(
            classify_area("Optimize meta description lengths", "accessibility"),
            "seo"
        );
        assert_eq!(
            classify_area("Remove deprecated X-XSS-Protection header", "seo"),
            "security"
        );
        assert_eq!(
            classify_area("Enable Brotli compression for HTML", "infrastructure"),
            "performance"
        );
        assert_eq!(classify_area("Fix broken links returning 404", "seo"), "infrastructure");
        assert_eq!(
            classify_area("Convert large GIFs to modern formats", "infrastructure"),
            "performance"
        );
        assert_eq!(
            classify_area("Enable IPv6 support via AAAA record", "security"),
            "infrastructure"
        );
        assert_eq!(
            classify_area("Implement a strict Content-Security-Policy", "performance"),
            "security"
        );
        assert_eq!(classify_area("Fix skipped heading levels", "security"), "accessibility");
    }

    #[test]
    fn classify_area_keeps_model_area_when_no_signal() {
        assert_eq!(classify_area("Improve overall quality", "performance"), "performance");
    }

    #[test]
    fn near_duplicate_recommendations_collapse() {
        let rec = |title: &str| Recommendation {
            area: "infrastructure".to_string(),
            severity: "high".to_string(),
            title: title.to_string(),
            recommendation: title.to_string(),
            ..Default::default()
        };
        let report = ReportSummary {
            overall_assessment: "x".to_string(),
            overall_grade: "C".to_string(),
            recommendations: vec![
                rec("Fix broken links returning 404"),
                rec("Resolve broken internal links"),
                rec("Add rel=canonical tags"),
            ],
        };
        let html = render_html(&report, &[]);
        // Two broken-link recs collapse to one; canonical stays → 2 cards.
        assert_eq!(html.matches("class=\"ai-rec\" data-area").count(), 2);
    }

    #[test]
    fn contains_word_respects_boundaries() {
        assert!(!contains_word("design variant a", "aria")); // not inside "variant"
        assert!(contains_word("add aria-label", "aria")); // hyphen boundary ok
        assert!(!contains_word("a gift box", "gif")); // not inside "gift"
        assert!(contains_word("convert to gif", "gif"));
        assert!(contains_word("x-frame-options missing", "x-frame"));
        assert!(contains_word("set og:title", "og:")); // colon-suffixed needle
        assert!(contains_word("enable ssl/tls", "ssl"));
    }

    #[test]
    fn classify_area_routes_security_headers() {
        assert_eq!(
            classify_area("Add X-Content-Type-Options: nosniff", "performance"),
            "security"
        );
        assert_eq!(classify_area("Implement a Content-Security-Policy", "seo"), "security");
        assert_eq!(classify_area("Enable HSTS with includeSubDomains", ""), "security");
    }

    #[test]
    fn classify_area_routes_dns_to_infrastructure() {
        assert_eq!(
            classify_area("Add AAAA / IPv6 DNS records", "security"),
            "infrastructure"
        );
    }

    #[test]
    fn classify_area_routes_a11y_and_cross_origin() {
        // New accessibility keywords route correctly...
        assert_eq!(classify_area("Improve screen reader support", "seo"), "accessibility");
        assert_eq!(classify_area("Add a skip link to main content", ""), "accessibility");
        assert_eq!(
            classify_area("Fix tab order on the nav", "performance"),
            "accessibility"
        );
        // ...and cross-origin isolation stays security (word-bounded, no false positives).
        assert_eq!(
            classify_area("Add Cross-Origin isolation headers (COOP/COEP)", ""),
            "security"
        );
        // a benign sentence merely containing "origin" must not be mis-tagged as security.
        assert_eq!(classify_area("Document the origin story of the brand", "seo"), "seo");
    }

    #[test]
    fn classify_area_clamps_offtaxonomy_to_canonical() {
        // No keyword hits and a junk model area → must clamp to a canonical area, never raw junk.
        assert_eq!(classify_area("some neutral sentence", "data-quality"), "infrastructure");
        assert_eq!(classify_area("some neutral sentence", "seo"), "seo");
    }

    #[test]
    fn topic_signature_distinguishes_meta_description_variants() {
        let a = topic_signature("seo", "Add missing meta descriptions");
        let b = topic_signature("seo", "Optimize meta description length");
        assert!(a.is_some() && b.is_some());
        assert_ne!(a, b); // distinct → neither suppresses the other in dedup
    }

    #[test]
    fn topic_signature_uses_title_only_so_csp_body_mention_does_not_collide() {
        // An X-XSS card and a CSP card must get different signatures even if a body mentions CSP.
        let xss = topic_signature("security", "Remove deprecated X-XSS-Protection header");
        let csp = topic_signature("security", "Implement a Content-Security-Policy");
        assert_ne!(xss, csp);
    }
}

const STYLE: &str = concat!(
    "<style>\n",
    ".ai-box{margin-bottom:24px;padding:20px;border-radius:12px;background:#F3F4F6;}\n",
    ".ai-head{display:flex;align-items:center;gap:12px;margin-bottom:12px;}\n",
    ".ai-title{margin:0;font-size:18px;color:#111827;}\n",
    ".ai-grade{color:#fff;font-weight:bold;border-radius:6px;padding:2px 10px;font-size:14px;}\n",
    ".ai-overall{margin:0 0 14px;color:#374151;line-height:1.5;}\n",
    ".ai-chips{display:flex;flex-wrap:wrap;gap:8px;margin-bottom:16px;align-items:center;}\n",
    ".ai-chips-hint{font-size:12px;color:#6B7280;margin-right:2px;}\n",
    ".ai-chip{display:inline-flex;align-items:center;gap:6px;background:#E5E7EB;border-radius:14px;padding:3px 4px 3px 10px;font-size:12px;color:#374151;cursor:pointer;user-select:none;transition:opacity .12s;}\n",
    ".ai-chip-grade{color:#fff;font-weight:bold;border-radius:10px;padding:1px 8px;}\n",
    ".ai-fl{position:absolute;width:0;height:0;opacity:0;pointer-events:none;}\n",
    ".ai-chip:has(.ai-fl:not(:checked)){opacity:0.4;}\n",
    ".ai-box:has(#ai-f-security:not(:checked)) .ai-rec[data-area=\"security\"]{display:none;}\n",
    ".ai-box:has(#ai-f-accessibility:not(:checked)) .ai-rec[data-area=\"accessibility\"]{display:none;}\n",
    ".ai-box:has(#ai-f-seo:not(:checked)) .ai-rec[data-area=\"seo\"]{display:none;}\n",
    ".ai-box:has(#ai-f-performance:not(:checked)) .ai-rec[data-area=\"performance\"]{display:none;}\n",
    ".ai-box:has(#ai-f-infrastructure:not(:checked)) .ai-rec[data-area=\"infrastructure\"]{display:none;}\n",
    ".ai-recs{display:grid;grid-template-columns:repeat(auto-fill,minmax(320px,1fr));gap:10px;}\n",
    ".ai-rec{background:#fff;border-left:4px solid #9ca3af;border-radius:8px;padding:10px 12px;}\n",
    ".ai-rec-h{display:flex;align-items:baseline;gap:8px;}\n",
    ".ai-rec-icon{flex-shrink:0;}\n",
    ".ai-rec-title{font-weight:600;color:#111827;flex:1;}\n",
    ".ai-rec-area{font-size:11px;text-transform:uppercase;letter-spacing:.03em;color:#6B7280;background:#F3F4F6;border-radius:4px;padding:1px 6px;}\n",
    ".ai-rec-body{margin-top:6px;color:#374151;font-size:14px;line-height:1.45;}\n",
    ".ai-rec-meta{margin-top:5px;color:#6B7280;font-size:12px;}\n",
    ".ai-note{margin:14px 0 0;font-size:12px;color:#9CA3AF;}\n",
    "html:has(.theme-switch__input:checked) .ai-box{background:#1F2937;}\n",
    "html:has(.theme-switch__input:checked) .ai-title{color:#F9FAFB;}\n",
    "html:has(.theme-switch__input:checked) .ai-overall{color:#D1D5DB;}\n",
    "html:has(.theme-switch__input:checked) .ai-chip{background:#374151;color:#D1D5DB;}\n",
    "html:has(.theme-switch__input:checked) .ai-rec{background:#111827;}\n",
    "html:has(.theme-switch__input:checked) .ai-rec-title{color:#F9FAFB;}\n",
    "html:has(.theme-switch__input:checked) .ai-rec-area{background:#374151;color:#9CA3AF;}\n",
    "html:has(.theme-switch__input:checked) .ai-rec-body{color:#D1D5DB;}\n",
    "</style>\n",
);
