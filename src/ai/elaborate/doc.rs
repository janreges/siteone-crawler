// SiteOne Crawler - brand elaborate: the BrandDoc + renderers
// (c) Jan Reges <jan.reges@siteone.cz>
//
// `BrandDoc` is the stored artifact: the merged model + the LLM prose keyed by skeleton section id.
// It renders three consistent outputs: markdown (primary), JSON (structured, for downstream/AI use),
// and a self-contained light/dark HTML document. The structured lists are rendered HERE from the
// deduped model — so people, contacts, facts, and the Sources list are un-hallucinatable.

use std::collections::BTreeMap;

use serde_json::{Value, json};

use crate::ai::report::locale::ReportLocale;

use super::correct::CorrectionReport;
use super::merge::BrandModel;
use super::templates::{ListSource, SectionKind, Template, skeleton};

#[derive(Debug, Clone, Default)]
pub struct ElaborateMeta {
    pub host: String,
    pub url: String,
    pub crawled_at: String,
    pub provider: String,
    pub model: String,
    pub report_language: String,
    pub site_type: String,
    /// Pages that were successfully extracted and fed into the model.
    pub pages_used: usize,
    /// (url, error) for pages that failed extraction after all retries (honest, never fabricated).
    pub pages_failed: Vec<(String, String)>,
}

#[derive(Debug, Clone)]
pub struct BrandDoc {
    /// "corporate" | "personal" | "product".
    pub template: String,
    pub brand_name: String,
    pub title: String,
    pub meta: ElaborateMeta,
    pub model: BrandModel,
    /// section id → prose markdown (only prose sections; may be empty/absent).
    pub prose: BTreeMap<String, String>,
    pub correction: Option<CorrectionReport>,
}

/// Readability caps for the two list sections that tend to become long, noisy "walls" on
/// content-heavy sites (docs, large brands). The rendered MD/HTML show the first N (the most
/// important pages are extracted first, so these are the most central); the FULL, uncapped set is
/// always kept in the JSON artifact, and a note states how many were omitted — nothing is lost.
const FACTS_DISPLAY_CAP: usize = 40;
const QUOTES_DISPLAY_CAP: usize = 20;

fn template_of(key: &str) -> Template {
    match key {
        "personal" => Template::Personal,
        "product" => Template::Product,
        _ => Template::Corporate,
    }
}

impl BrandDoc {
    fn locale(&self) -> ReportLocale {
        ReportLocale::new(&self.meta.report_language)
    }

    /// Localized heading for a section id (English default falls back to the skeleton heading).
    fn heading(&self, czech: bool, id: &str, default_en: &str) -> String {
        if !czech {
            return default_en.to_string();
        }
        let cs = match id {
            "abstract" => "Shrnutí",
            "identity_mission" => "Identita a poslání",
            "identity_bio" => "Identita a životopis",
            "product_identity" => "Identita produktu",
            "offer_intro" => "Co nabízejí",
            "value_prop" => "Hodnotová nabídka",
            "expertise" => "Odbornost a dovednosti",
            "experience" => "Zkušenosti",
            "audiences" => "Publikum a pro koho",
            "services" => "Služby",
            "products" => "Produkty",
            "features" => "Funkce a možnosti",
            "work" => "Práce a portfolio",
            "catalogs" => "Katalogy a kolekce",
            "people" => "Lidé a role",
            "locations" => "Pobočky a místa",
            "facts" => "Klíčová fakta a čísla",
            "channels" => "Komunikační kanály",
            "contact" => "Kontakt a kanály",
            "statements" => "Významná prohlášení",
            "proof" => "Důkazy a důvěryhodnost",
            "sources" => "Zdroje",
            _ => default_en,
        };
        cs.to_string()
    }

    // ---- markdown ----

    pub fn to_markdown(&self) -> String {
        let czech = self.locale().is_czech();
        let mut out = String::with_capacity(16 * 1024);
        out.push_str(&format!("# {}\n\n", self.title));
        out.push_str(&format!(
            "> {} · {}\n\n",
            self.meta.host,
            if self.meta.crawled_at.is_empty() {
                "—"
            } else {
                &self.meta.crawled_at
            }
        ));

        for s in skeleton(template_of(&self.template)) {
            let body = match &s.kind {
                SectionKind::Prose => self.prose.get(s.id).cloned().unwrap_or_default().trim().to_string(),
                SectionKind::List(src) => self.render_list_md(*src),
                SectionKind::Sources => self.render_sources_md(),
            };
            if body.trim().is_empty() {
                continue; // omit empty sections (never invent to fill)
            }
            out.push_str(&format!(
                "## {}\n\n{}\n\n",
                self.heading(czech, s.id, s.heading),
                body.trim()
            ));
        }

        if !self.meta.pages_failed.is_empty() {
            let note = if czech {
                format!(
                    "*Sestaveno z {} stránek; {} stránek se nepodařilo zpracovat.*",
                    self.meta.pages_used,
                    self.meta.pages_failed.len()
                )
            } else {
                format!(
                    "*Built from {} pages; {} page(s) could not be processed.*",
                    self.meta.pages_used,
                    self.meta.pages_failed.len()
                )
            };
            out.push_str(&format!("---\n\n{}\n", note));
        }
        out
    }

    fn render_list_md(&self, src: ListSource) -> String {
        let m = &self.model;
        let mut out = String::new();
        match src {
            ListSource::Services | ListSource::Products | ListSource::Offerings => {
                for o in &m.offerings {
                    let keep = match src {
                        ListSource::Services => o.kind == "service",
                        ListSource::Products => o.kind == "product",
                        _ => true,
                    };
                    if !keep || o.name.is_empty() {
                        continue;
                    }
                    let mut line = format!("- **{}**", o.name);
                    if !o.summary.is_empty() {
                        line.push_str(&format!(" — {}", o.summary));
                    }
                    let mut extra = Vec::new();
                    if !o.for_whom.is_empty() {
                        extra.push(o.for_whom.clone());
                    }
                    if !o.price_or_terms.is_empty() {
                        extra.push(o.price_or_terms.clone());
                    }
                    if !extra.is_empty() {
                        line.push_str(&format!(" ({})", extra.join("; ")));
                    }
                    out.push_str(&line);
                    out.push('\n');
                }
            }
            ListSource::People => {
                for p in &m.people {
                    if p.name.is_empty() {
                        continue;
                    }
                    let mut line = format!("- **{}**", p.name);
                    if !p.role.is_empty() {
                        line.push_str(&format!(" — {}", p.role));
                    }
                    let mut c = Vec::new();
                    if !p.email.is_empty() {
                        c.push(p.email.clone());
                    }
                    if !p.phone.is_empty() {
                        c.push(p.phone.clone());
                    }
                    if !c.is_empty() {
                        line.push_str(&format!(" ({})", c.join(", ")));
                    }
                    out.push_str(&line);
                    out.push('\n');
                }
            }
            ListSource::Locations => {
                for l in &m.locations {
                    let label = if l.label.is_empty() { "Location" } else { &l.label };
                    let mut line = format!("- **{}**", label);
                    if !l.address.is_empty() {
                        line.push_str(&format!(" — {}", l.address));
                    }
                    let mut c = Vec::new();
                    if !l.phone.is_empty() {
                        c.push(l.phone.clone());
                    }
                    if !l.email.is_empty() {
                        c.push(l.email.clone());
                    }
                    if !c.is_empty() {
                        line.push_str(&format!(" ({})", c.join(", ")));
                    }
                    out.push_str(&line);
                    out.push('\n');
                }
            }
            ListSource::Facts => {
                for f in m.facts.iter().take(FACTS_DISPLAY_CAP) {
                    out.push_str(&format!("- {}\n", f.value));
                }
                if m.facts.len() > FACTS_DISPLAY_CAP {
                    out.push_str(&format!("- {}\n", self.more_label(m.facts.len() - FACTS_DISPLAY_CAP)));
                }
            }
            ListSource::Channels => {
                for c in &m.channels {
                    if c.value.value.is_empty() {
                        continue;
                    }
                    out.push_str(&format!("- {}: {}\n", c.value.kind, c.value.value));
                }
            }
            ListSource::Quotes => {
                for q in m.quotes.iter().take(QUOTES_DISPLAY_CAP) {
                    out.push_str(&format!("> {}\n\n", q.value));
                }
                if m.quotes.len() > QUOTES_DISPLAY_CAP {
                    out.push_str(&format!("{}\n\n", self.more_label(m.quotes.len() - QUOTES_DISPLAY_CAP)));
                }
            }
            ListSource::Catalogs => {
                for c in &m.catalogs {
                    let mut line = format!("- **{}** (~{} pages, `{}`)", c.label, c.count, c.template);
                    if !c.sample_urls.is_empty() {
                        line.push_str(&format!(" — e.g. {}", c.sample_urls.join(", ")));
                    }
                    out.push_str(&line);
                    out.push('\n');
                }
            }
        }
        out
    }

    /// Localized "+N more (full list in JSON)" marker for capped list sections.
    fn more_label(&self, hidden: usize) -> String {
        if self.locale().is_czech() {
            format!("… a dalších {} (úplný seznam v JSON)", hidden)
        } else {
            format!("… and {} more (full list in the JSON)", hidden)
        }
    }

    fn render_sources_md(&self) -> String {
        self.model
            .sources
            .iter()
            .map(|u| format!("- {}", u))
            .collect::<Vec<_>>()
            .join("\n")
    }

    // ---- json ----

    pub fn to_json(&self) -> Value {
        json!({
            "template": self.template,
            "brandName": self.brand_name,
            "title": self.title,
            "site": {
                "host": self.meta.host,
                "url": self.meta.url,
                "crawledAt": self.meta.crawled_at,
                "provider": self.meta.provider,
                "model": self.meta.model,
                "reportLanguage": self.meta.report_language,
                "siteType": self.meta.site_type,
                "pagesUsed": self.meta.pages_used,
                "pagesFailed": self.meta.pages_failed.iter()
                    .map(|(u, e)| json!({"url": u, "error": e})).collect::<Vec<_>>(),
            },
            "prose": self.prose,
            "model": self.model.to_json(),
            "correction": self.correction.as_ref().map(|c| json!({
                "proposed": c.proposed, "applied": c.applied,
                "skippedNoMatch": c.skipped_no_match, "skippedAmbiguous": c.skipped_ambiguous,
                "skippedOverlap": c.skipped_overlap,
            })),
        })
    }

    // ---- html ----

    pub fn to_html(&self) -> String {
        let czech = self.locale().is_czech();
        let mut h = String::with_capacity(32 * 1024);
        h.push_str("<!DOCTYPE html>\n<html lang=\"");
        h.push_str(&esc(self.locale().code()));
        h.push_str("\" data-theme=\"light\">\n<head>\n<meta charset=\"utf-8\">\n");
        h.push_str("<meta name=\"viewport\" content=\"width=device-width, initial-scale=1\">\n");
        h.push_str(&format!(
            "<title>{}</title>\n<style>\n{}\n</style>\n</head>\n<body>\n",
            esc(&self.title),
            CSS
        ));

        // Header.
        h.push_str("<header class=\"bar\"><div class=\"brand\"><span class=\"logo\">◧ SiteOne</span> ");
        h.push_str("<span class=\"sub\">Brand Elaborate</span></div>");
        h.push_str(&format!(
            "<div class=\"meta\"><strong>{}</strong> · <a href=\"{}\">{}</a>",
            esc(&self.title),
            esc(&self.meta.url),
            esc(&self.meta.host)
        ));
        if !self.meta.model.is_empty() {
            h.push_str(&format!(" · {} / {}", esc(&self.meta.provider), esc(&self.meta.model)));
        }
        h.push_str(&format!(" · {} · {}", self.meta.pages_used, esc(&self.meta.crawled_at)));
        h.push_str("</div><button id=\"themeBtn\" class=\"btn\">◐</button></header>\n");

        // TOC + sections.
        let sk = skeleton(template_of(&self.template));
        let mut visible: Vec<(&str, String)> = Vec::new();
        let mut bodies: Vec<(String, String)> = Vec::new();
        for s in sk {
            let body_html = match &s.kind {
                SectionKind::Prose => prose_to_html(self.prose.get(s.id).map(|s| s.as_str()).unwrap_or("")),
                SectionKind::List(src) => self.render_list_html(*src),
                SectionKind::Sources => self.render_sources_html(),
            };
            if body_html.trim().is_empty() {
                continue;
            }
            let heading = self.heading(czech, s.id, s.heading);
            visible.push((s.id, heading.clone()));
            bodies.push((
                s.id.to_string(),
                format!("<h2 id=\"s-{}\">{}</h2>\n{}", esc(s.id), esc(&heading), body_html),
            ));
        }

        h.push_str("<nav class=\"toc\"><ul>");
        for (id, heading) in &visible {
            h.push_str(&format!("<li><a href=\"#s-{}\">{}</a></li>", esc(id), esc(heading)));
        }
        h.push_str("</ul></nav>\n<main>\n");
        for (_, body) in &bodies {
            h.push_str(&format!("<section class=\"sec\">{}</section>\n", body));
        }
        h.push_str("</main>\n");

        // Footer.
        let disclaimer = if czech {
            "AI-generovaný přehled značky. Ověřte fakta před použitím."
        } else {
            "AI-generated brand profile. Verify facts before acting."
        };
        h.push_str(&format!("<footer class=\"foot\"><p>{}</p>", disclaimer));
        if !self.meta.pages_failed.is_empty() {
            h.push_str(&format!(
                "<p class=\"gen\">{} / {} pages ({} failed).</p>",
                self.meta.pages_used,
                self.meta.pages_used + self.meta.pages_failed.len(),
                self.meta.pages_failed.len()
            ));
        }
        h.push_str("<p class=\"gen\">Generated by SiteOne Crawler.</p></footer>\n");
        h.push_str("<script>\n");
        h.push_str(JS);
        h.push_str("\n</script>\n</body>\n</html>\n");
        h
    }

    fn render_list_html(&self, src: ListSource) -> String {
        // Reuse the markdown list, then md-lite it (bullets → <ul>). Everything escaped inside.
        let md = self.render_list_md(src);
        if md.trim().is_empty() {
            String::new()
        } else {
            prose_to_html(&md)
        }
    }

    fn render_sources_html(&self) -> String {
        if self.model.sources.is_empty() {
            return String::new();
        }
        let mut h = String::from("<ul class=\"sources\">");
        for u in &self.model.sources {
            h.push_str(&format!("<li><a href=\"{}\">{}</a></li>", esc(u), esc(u)));
        }
        h.push_str("</ul>");
        h
    }
}

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

/// Minimal, SAFE markdown → HTML for prose/lists: escapes everything first, then renders paragraphs,
/// `- ` bullets, `> ` blockquotes, and `**bold**`. No raw HTML from the model ever passes through.
fn prose_to_html(md: &str) -> String {
    let mut html = String::new();
    let mut in_ul = false;
    let mut para: Vec<String> = Vec::new();

    let flush_para = |para: &mut Vec<String>, html: &mut String| {
        if !para.is_empty() {
            html.push_str(&format!("<p>{}</p>\n", inline(&para.join(" "))));
            para.clear();
        }
    };
    let close_ul = |in_ul: &mut bool, html: &mut String| {
        if *in_ul {
            html.push_str("</ul>\n");
            *in_ul = false;
        }
    };

    for raw_line in md.lines() {
        let line = raw_line.trim_end();
        if line.trim().is_empty() {
            flush_para(&mut para, &mut html);
            close_ul(&mut in_ul, &mut html);
            continue;
        }
        if let Some(item) = line.trim_start().strip_prefix("- ") {
            flush_para(&mut para, &mut html);
            if !in_ul {
                html.push_str("<ul>\n");
                in_ul = true;
            }
            html.push_str(&format!("<li>{}</li>\n", inline(item)));
        } else if let Some(q) = line.trim_start().strip_prefix("> ") {
            flush_para(&mut para, &mut html);
            close_ul(&mut in_ul, &mut html);
            html.push_str(&format!("<blockquote>{}</blockquote>\n", inline(q)));
        } else {
            close_ul(&mut in_ul, &mut html);
            para.push(line.trim().to_string());
        }
    }
    flush_para(&mut para, &mut html);
    close_ul(&mut in_ul, &mut html);
    html
}

/// Inline rendering: escape, then turn `**bold**` into <strong> (on the ALREADY-escaped text).
fn inline(s: &str) -> String {
    let escaped = esc(s);
    let mut out = String::with_capacity(escaped.len());
    let mut rest = escaped.as_str();
    while let Some(start) = rest.find("**") {
        out.push_str(&rest[..start]);
        let after = &rest[start + 2..];
        if let Some(end) = after.find("**") {
            out.push_str("<strong>");
            out.push_str(&after[..end]);
            out.push_str("</strong>");
            rest = &after[end + 2..];
        } else {
            out.push_str("**");
            rest = after;
        }
    }
    out.push_str(rest);
    out
}

const CSS: &str = r#":root{--bg:#f3f4f6;--surface:#fff;--ink:#111827;--muted:#6b7280;--border:#e5e7eb;--accent:#4e79a7;--chipbg:#eef2f7}
[data-theme="dark"]{--bg:#0f172a;--surface:#1f2937;--ink:#e5e7eb;--muted:#9ca3af;--border:#374151;--accent:#7aa8d6;--chipbg:#243244}
*{box-sizing:border-box}body{margin:0;font:15px/1.65 -apple-system,BlinkMacSystemFont,"Segoe UI",Roboto,Helvetica,Arial,sans-serif;background:var(--bg);color:var(--ink)}
a{color:var(--accent);text-decoration:none}a:hover{text-decoration:underline}
.bar{display:flex;align-items:center;gap:16px;flex-wrap:wrap;padding:14px 22px;background:var(--surface);border-bottom:1px solid var(--border)}
.brand{font-weight:700}.logo{color:var(--accent)}.sub{color:var(--muted);font-weight:500}
.meta{color:var(--muted);flex:1;min-width:200px}.meta strong{color:var(--ink)}
.btn{background:var(--chipbg);color:var(--ink);border:1px solid var(--border);border-radius:8px;padding:6px 12px;cursor:pointer}
.toc{padding:16px 22px;background:var(--surface);border-bottom:1px solid var(--border)}
.toc ul{list-style:none;margin:0;padding:0;display:flex;flex-wrap:wrap;gap:6px 18px}
.toc a{font-size:13.5px}
main{max-width:900px;margin:0 auto;padding:24px 22px 60px}
.sec{background:var(--surface);border:1px solid var(--border);border-radius:12px;padding:18px 22px;margin:0 0 18px}
.sec h2{margin:0 0 10px;font-size:20px;border-bottom:1px solid var(--border);padding-bottom:8px}
.sec p{margin:0 0 10px}.sec ul{margin:0 0 10px;padding-left:20px}.sec li{margin:3px 0}
blockquote{margin:8px 0;padding:6px 14px;border-left:3px solid var(--accent);color:var(--muted);font-style:italic}
.sources{font-family:ui-monospace,SFMono-Regular,Menlo,monospace;font-size:12.5px;word-break:break-all}
.foot{max-width:900px;margin:0 auto;padding:0 22px 40px;color:var(--muted)}.gen{font-size:12px;margin:4px 0 0}"#;

const JS: &str = r#"(function(){var html=document.documentElement;
try{if(matchMedia('(prefers-color-scheme: dark)').matches)html.setAttribute('data-theme','dark');}catch(e){}
var b=document.getElementById('themeBtn');if(b)b.addEventListener('click',function(){html.setAttribute('data-theme',html.getAttribute('data-theme')==='dark'?'light':'dark');});})();"#;

#[cfg(test)]
mod tests {
    use super::super::merge::{MergedPerson, Sourced};
    use super::*;

    fn sample() -> BrandDoc {
        let mut model = BrandModel::default();
        model.people.push(MergedPerson {
            name: "Jan Novák".into(),
            role: "CEO".into(),
            email: "jan@x.cz".into(),
            sources: vec!["https://x/tym".into()],
            ..Default::default()
        });
        model.facts.push(Sourced {
            value: "Založeno 2010".into(),
            sources: vec!["https://x/o-nas".into()],
        });
        model.sources = vec!["https://x/".into(), "https://x/tym".into()];
        let mut prose = BTreeMap::new();
        prose.insert(
            "identity_mission".into(),
            "Firma dělá **skvělé** věci.\n\n- bod jedna\n- bod dva".into(),
        );
        prose.insert("abstract".into(), "Krátké shrnutí.".into());
        BrandDoc {
            template: "corporate".into(),
            brand_name: "X".into(),
            title: "X — company profile".into(),
            meta: ElaborateMeta {
                host: "x.cz".into(),
                url: "https://x/".into(),
                crawled_at: "2026-07-21 16:00".into(),
                report_language: "cs-CZ".into(),
                pages_used: 12,
                ..Default::default()
            },
            model,
            prose,
            correction: None,
        }
    }

    #[test]
    fn markdown_renders_sections_omits_empty_and_lists_people() {
        let md = sample().to_markdown();
        assert!(md.contains("# X — company profile"));
        assert!(md.contains("## Identita a poslání")); // czech heading
        assert!(md.contains("## Lidé a role"));
        assert!(md.contains("**Jan Novák** — CEO (jan@x.cz)"));
        assert!(md.contains("## Zdroje"));
        assert!(md.contains("- https://x/tym"));
        // A prose section with no content (e.g. audiences) is omitted.
        assert!(!md.contains("## Publikum a pro koho"));
    }

    #[test]
    fn html_is_escaped_and_self_contained() {
        let mut d = sample();
        d.prose
            .insert("audiences".into(), "Zlá věta <script>alert(1)</script> tady.".into());
        let html = d.to_html();
        assert!(html.starts_with("<!DOCTYPE html>"));
        assert!(!html.contains("<script>alert(1)")); // escaped
        assert!(html.contains("&lt;script&gt;alert(1)"));
        assert!(html.contains("<strong>skvělé</strong>")); // md-lite bold
        assert!(html.contains("<li>bod jedna</li>"));
        assert!(!html.contains("cdn.")); // self-contained
    }

    #[test]
    fn facts_are_capped_in_markdown_but_kept_whole_in_json() {
        let mut d = sample();
        d.model.facts = (0..100)
            .map(|i| Sourced {
                value: format!("Fact number {}", i),
                sources: vec!["https://x/".into()],
            })
            .collect();
        let md = d.to_markdown();
        assert!(md.contains("Fact number 0"));
        // sample() is cs-CZ → localized "+N more" note.
        assert!(md.contains(&format!("dalších {}", 100 - FACTS_DISPLAY_CAP))); // "dalších 60"
        assert!(!md.contains("Fact number 99")); // beyond the display cap
        // JSON keeps every fact.
        let j = d.to_json();
        assert_eq!(j["model"]["facts"].as_array().unwrap().len(), 100);
    }

    #[test]
    fn json_roundtrips_people_and_correction() {
        let mut d = sample();
        d.correction = Some(CorrectionReport {
            proposed: 3,
            applied: 2,
            ..Default::default()
        });
        let j = d.to_json();
        assert_eq!(j["site"]["pagesUsed"], 12);
        assert_eq!(j["model"]["people"][0]["email"], "jan@x.cz");
        assert_eq!(j["correction"]["applied"], 2);
    }
}
