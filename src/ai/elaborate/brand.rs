// SiteOne Crawler - brand elaborate: brand-name detection
// (c) Jan Reges <jan.reges@siteone.cz>
//
// Deriving "what brand is this?" from a COMBINATION of independent signals is far more robust than
// trusting any single one. We collect candidates from four sources and let cross-source agreement
// decide — a name that shows up in the <title>, the footer, a site-name meta tag AND the domain is
// almost certainly the brand:
//
//   * Meta   — `og:site_name` / `application-name` (the site's own declared name; most authoritative)
//   * Footer — the copyright line ("© 2026 Acme s.r.o."), which usually carries the legal org name
//   * Title  — the delimited <title> segment that recurs across pages (`Page | Brand` / `Brand — Page`)
//   * Domain — the host label, a weak self-candidate that also validates/bridges the others
//
// Everything here is language- and site-agnostic: no brand-specific rules, only general structure.

use scraper::{Html, Selector};

/// Title separators a brand is commonly split from a page label with, across styles/locales.
const TITLE_SEPARATORS: &[&str] = &[" — ", " – ", " - ", " | ", " · ", " • ", " :: ", " » ", " › ", " / "];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Source {
    Meta,
    Footer,
    Title,
    Domain,
}

fn source_rank(s: Source) -> u8 {
    match s {
        Source::Meta => 3,
        Source::Footer => 2,
        Source::Title => 1,
        Source::Domain => 0,
    }
}

struct Candidate {
    display: String,
    source: Source,
    weight: f64,
    compact: String,
}

impl Candidate {
    fn new(display: String, source: Source, weight: f64) -> Self {
        let compact = compact_alnum(&display);
        Candidate {
            display,
            source,
            weight,
            compact,
        }
    }
}

struct Group {
    display: String,
    rank: u8,
    compact: String,
    weight: f64,
}

/// Detect the brand name from a sample of crawled pages (url, html). Combines the four signal
/// sources above; falls back to a title-cased domain label when no page yields a usable signal.
pub fn detect(samples: &[(String, String)], host: &str) -> String {
    let mut titles: Vec<String> = Vec::new();
    let mut cands: Vec<Candidate> = Vec::new();

    for (_url, html) in samples {
        let doc = Html::parse_document(html);
        if let Some(t) = select_title(&doc) {
            titles.push(t);
        }
        for name in site_name_metas(&doc) {
            cands.push(Candidate::new(name, Source::Meta, 4.0));
        }
        if let Some(name) = footer_org(&doc) {
            cands.push(Candidate::new(name, Source::Footer, 3.0));
        }
    }

    for (segment, count) in title_segment_counts(&titles) {
        cands.push(Candidate::new(segment, Source::Title, (count as f64).min(5.0)));
    }

    let host_labels = host_labels(host);
    for label in &host_labels {
        cands.push(Candidate::new(title_case(label), Source::Domain, 1.0));
    }

    let groups = group_candidates(cands, &host_labels);
    match pick_best(&groups) {
        Some(name) => name,
        None => host_fallback(host),
    }
}

/// Group candidates that denote the same brand (their alphanumeric forms are containment-related, so
/// "Acme", "Acme Corp" and "Acme Corporation" merge), sum their weights, and pick the most
/// authoritative display form per group. A host-name match adds a bonus so the domain anchors the
/// real brand rather than a recurring tagline.
fn group_candidates(mut cands: Vec<Candidate>, host_labels: &[String]) -> Vec<Group> {
    // Shortest compact first, so a short shared root (often the domain label) anchors the group and
    // longer forms attach to it transitively.
    cands.sort_by_key(|c| c.compact.chars().count());

    let mut groups: Vec<Group> = Vec::new();
    for c in cands {
        if c.compact.chars().count() < 2 {
            continue;
        }
        let mut merged = false;
        for g in groups.iter_mut() {
            if related(&g.compact, &c.compact) {
                g.weight += c.weight;
                let r = source_rank(c.source);
                let cn = c.display.chars().count();
                if r > g.rank || (r == g.rank && (2..g.display.chars().count()).contains(&cn)) {
                    g.display = c.display.clone();
                    g.rank = r;
                }
                merged = true;
                break;
            }
        }
        if !merged {
            groups.push(Group {
                rank: source_rank(c.source),
                display: c.display,
                compact: c.compact,
                weight: c.weight,
            });
        }
    }

    for g in groups.iter_mut() {
        if host_labels.iter().any(|l| related(&g.compact, l)) {
            g.weight += 2.5;
        }
    }
    groups
}

fn pick_best(groups: &[Group]) -> Option<String> {
    groups
        .iter()
        .max_by(|a, b| {
            a.weight
                .partial_cmp(&b.weight)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then(a.rank.cmp(&b.rank))
                .then(b.display.chars().count().cmp(&a.display.chars().count()))
        })
        .map(|g| g.display.clone())
}

/// Two brand forms are the same when the shorter's alphanumeric form (>=3 chars) is contained in the
/// longer's — e.g. `acme` ⊂ `acmecorp`, `homecredit` ⊂ `homecreditas`.
fn related(a: &str, b: &str) -> bool {
    let (short, long) = if a.chars().count() <= b.chars().count() {
        (a, b)
    } else {
        (b, a)
    };
    short.chars().count() >= 3 && long.contains(short)
}

fn select_title(doc: &Html) -> Option<String> {
    select_text(doc, "title").filter(|t| !t.is_empty())
}

fn select_text(doc: &Html, sel: &str) -> Option<String> {
    let selector = Selector::parse(sel).ok()?;
    doc.select(&selector).next().map(|el| {
        el.text()
            .collect::<String>()
            .split_whitespace()
            .collect::<Vec<_>>()
            .join(" ")
    })
}

fn site_name_metas(doc: &Html) -> Vec<String> {
    let mut out = Vec::new();
    for sel in [
        r#"meta[property="og:site_name"]"#,
        r#"meta[name="application-name"]"#,
        r#"meta[name="apple-mobile-web-app-title"]"#,
    ] {
        if let Ok(selector) = Selector::parse(sel) {
            for el in doc.select(&selector) {
                if let Some(content) = el.value().attr("content") {
                    let c = content.trim();
                    if (2..=60).contains(&c.chars().count()) {
                        out.push(c.to_string());
                    }
                }
            }
        }
    }
    out
}

/// Footer text of the page (the `<footer>` element and copyright-flagged containers), size-bounded.
fn footer_text(doc: &Html) -> String {
    let mut text = String::new();
    if let Ok(selector) = Selector::parse(r#"footer, [class*="copyright"], [id*="copyright"], [class*="footer"]"#) {
        for el in doc.select(&selector) {
            text.push(' ');
            text.push_str(&el.text().collect::<String>());
            if text.len() > 4000 {
                break;
            }
        }
    }
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Extract the organization name from a footer copyright line, e.g.
/// "© 2020–2026 Acme s.r.o. All rights reserved." → "Acme s.r.o.". Language-agnostic: strips the
/// marker, any leading year(s)/range, then stops at the first delimiter or "all rights"-style phrase.
fn footer_org(doc: &Html) -> Option<String> {
    let text = footer_text(doc);
    if text.is_empty() {
        return None;
    }
    // Earliest copyright marker; take what follows it.
    let markers = ["©", "Copyright", "copyright", "COPYRIGHT", "(c)", "(C)"];
    let after_start = markers.iter().filter_map(|m| text.find(m).map(|i| i + m.len())).min()?;
    let mut s = text[after_start..].trim_start_matches(|c: char| !c.is_alphanumeric());

    // Strip any run of "<year>[ separators ]" prefixes (handles "2020 – 2026 ").
    loop {
        let trimmed = s.trim_start_matches([' ', '-', '–', '—', ',', '.', '\u{a0}']);
        let is_year = trimmed.chars().take(4).collect::<String>();
        if is_year.len() == 4 && is_year.chars().all(|c| c.is_ascii_digit()) {
            s = &trimmed[4..];
        } else {
            s = trimmed;
            break;
        }
    }

    // Cut at the first hard delimiter or rights-reserved phrase (several languages).
    let mut end = s.len();
    for delim in ['|', '\n', '•', '·', '\r'] {
        if let Some(i) = s.find(delim) {
            end = end.min(i);
        }
    }
    for phrase in [
        ". ",
        "  ",
        "All rights",
        "all rights",
        "Všechna práva",
        "všechna práva",
        "Alle Rechte",
        "Tous droits",
        "Todos los",
    ] {
        if let Some(i) = s.find(phrase) {
            end = end.min(i);
        }
    }
    let name = s[..end].trim().trim_end_matches([',', '.', '·', '|']).trim();
    if (2..=60).contains(&name.chars().count()) && name.chars().any(|c| c.is_alphabetic()) {
        Some(name.to_string())
    } else {
        None
    }
}

/// Count the delimited <title> segments (prefix before the first separator, suffix after the last)
/// that recur across page titles. Each segment is counted at most once per title.
fn title_segment_counts(titles: &[String]) -> Vec<(String, usize)> {
    let mut counts: std::collections::HashMap<String, (String, usize)> = std::collections::HashMap::new();
    for title in titles {
        let t = title.trim();
        let first = TITLE_SEPARATORS.iter().filter_map(|s| t.find(s)).min();
        let last = TITLE_SEPARATORS
            .iter()
            .filter_map(|s| t.rfind(s).map(|i| (i, s.len())))
            .max_by_key(|(i, _)| *i);
        let mut candidates: Vec<&str> = Vec::new();
        if let Some(f) = first {
            candidates.push(t[..f].trim());
        }
        if let Some((l, len)) = last {
            candidates.push(t[l + len..].trim());
        }
        if candidates.is_empty() {
            candidates.push(t);
        }
        let mut seen: std::collections::HashSet<String> = std::collections::HashSet::new();
        for cand in candidates {
            let n = cand.chars().count();
            if !(2..=60).contains(&n) {
                continue;
            }
            let key = cand.to_lowercase();
            if !seen.insert(key.clone()) {
                continue;
            }
            let entry = counts.entry(key).or_insert_with(|| (cand.to_string(), 0));
            entry.1 += 1;
        }
    }
    counts.into_values().collect()
}

fn host_labels(host: &str) -> Vec<String> {
    host.split('.')
        .map(compact_alnum)
        .filter(|l| l.len() >= 4 && !matches!(l.as_str(), "www" | "info" | "name" | "site" | "online" | "shop"))
        .collect()
}

fn host_fallback(host: &str) -> String {
    host.split('.')
        .find(|l| l.chars().count() >= 3 && *l != "www")
        .map(title_case)
        .unwrap_or_else(|| host.to_string())
}

/// Alphanumeric-only lowercased form, for loose matching (`SiteOne Crawler` → `siteonecrawler`).
fn compact_alnum(s: &str) -> String {
    s.chars()
        .filter(|c| c.is_alphanumeric())
        .flat_map(|c| c.to_lowercase())
        .collect()
}

fn title_case(s: &str) -> String {
    let mut chars = s.chars();
    match chars.next() {
        Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
        None => String::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn page(title: &str, og: Option<&str>, footer: &str) -> String {
        let og_tag = og
            .map(|v| format!(r#"<meta property="og:site_name" content="{}">"#, v))
            .unwrap_or_default();
        format!(
            r#"<!DOCTYPE html><html><head><title>{}</title>{}</head>
            <body><main>content</main><footer>{}</footer></body></html>"#,
            title, og_tag, footer
        )
    }

    #[test]
    fn combines_title_footer_and_meta() {
        // Title suffix, og:site_name and footer copyright all point to the same brand.
        let samples = vec![
            (
                "https://acme.example/".to_string(),
                page(
                    "Home | Acme Corp",
                    Some("Acme Corp"),
                    "© 2026 Acme Corp. All rights reserved.",
                ),
            ),
            (
                "https://acme.example/about".to_string(),
                page("About us | Acme Corp", Some("Acme Corp"), "© 2026 Acme Corp."),
            ),
        ];
        assert_eq!(detect(&samples, "acme.example"), "Acme Corp");
    }

    #[test]
    fn footer_supplies_brand_when_title_is_seo_noise() {
        // Homepage <title> is an SEO phrase with no clean brand; the footer copyright carries it.
        let samples = vec![(
            "https://studionord.example/".to_string(),
            page(
                "Award-winning design & branding agency",
                None,
                "Copyright © 2019–2026 Studio Nord s.r.o. Všechna práva vyhrazena.",
            ),
        )];
        // The legal-form abbreviation's trailing dot is trimmed at the sentence stop, consistently.
        assert_eq!(detect(&samples, "studionord.example"), "Studio Nord s.r.o");
    }

    #[test]
    fn domain_label_bridges_slightly_different_forms() {
        // "Acme" (domain) bridges the title "Acme Corp" and footer "Acme Corporation".
        let samples = vec![(
            "https://acme.example/".to_string(),
            page("Welcome — Acme Corp", None, "© 2026 Acme Corporation"),
        )];
        // The footer (more authoritative than title) wins the display form within the merged group.
        assert_eq!(detect(&samples, "acme.example"), "Acme Corporation");
    }

    #[test]
    fn falls_back_to_domain_label_when_no_signal() {
        let samples = vec![(
            "https://foobar.example/".to_string(),
            page("", None, "just some footer text with no copyright"),
        )];
        assert_eq!(detect(&samples, "foobar.example"), "Foobar");
    }

    #[test]
    fn footer_org_parses_year_range_and_stops_at_rights_phrase() {
        let doc = Html::parse_document(
            r#"<html><body><footer>© 2020 - 2026 Beta Widgets Ltd. All rights reserved.</footer></body></html>"#,
        );
        assert_eq!(footer_org(&doc).as_deref(), Some("Beta Widgets Ltd"));
    }
}
