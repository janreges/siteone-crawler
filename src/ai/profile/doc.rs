// SiteOne Crawler - AI profile: the ProfileDoc + renderers
// (c) Jan Reges <jan.reges@siteone.cz>
//
// The stored artifact: meta + site description + executive summary + ordered chapters (each a
// synthesized Markdown body). Renders to Markdown (primary), JSON (structured), and self-contained
// HTML (Task 8). Omitted chapters are recorded (with a reason) but not rendered — never fabricated.

use serde_json::{Value, json};

use crate::ai::report::locale::ReportLocale;

use super::correct::CorrectionReport;

#[derive(Debug, Clone, Default)]
pub struct ProfileMeta {
    pub host: String,
    pub url: String,
    pub crawled_at: String,
    pub provider: String,
    pub model: String,
    pub report_language: String,
    pub context_window: i64,
    pub template_key: String,
    pub template_name: String,
    /// True when the type was auto-detected by the classifier; false when forced by CLI.
    pub template_detected: bool,
    pub pages_total: usize,
    pub pages_described: usize,
    pub calls: usize,
    pub pages_failed: Vec<(String, String)>,
}

#[derive(Debug, Clone)]
pub struct ProfilePageInfo {
    pub path: String,
    pub url: String,
    pub description: String,
    pub size_chars: usize,
}

#[derive(Debug, Clone)]
pub struct ProfileChapter {
    pub id: String,
    pub heading: String,
    pub markdown: String,
    pub sources: Vec<String>,
    pub target_chars: usize,
    pub correction: Option<CorrectionReport>,
    pub omitted: bool,
    pub omit_reason: String,
}

#[derive(Debug, Clone)]
pub struct ProfileDoc {
    pub title: String,
    pub subject_name: String,
    pub meta: ProfileMeta,
    pub site_description: String,
    pub exec_summary: String,
    pub pages: Vec<ProfilePageInfo>,
    pub chapters: Vec<ProfileChapter>,
}

impl ProfileDoc {
    pub(crate) fn locale(&self) -> ReportLocale {
        ReportLocale::new(&self.meta.report_language)
    }

    /// Chapters that were actually written (non-omitted, non-empty body).
    pub(crate) fn visible_chapters(&self) -> impl Iterator<Item = &ProfileChapter> {
        self.chapters
            .iter()
            .filter(|c| !c.omitted && !c.markdown.trim().is_empty())
    }

    // ---- markdown ----

    pub fn to_markdown(&self) -> String {
        let czech = self.locale().is_czech();
        let sources_label = if czech { "Zdroje" } else { "Sources" };
        let summary_label = if czech { "Shrnutí" } else { "Executive summary" };
        let mut out = String::with_capacity(32 * 1024);
        out.push_str(&format!("# {}\n\n", self.title));
        out.push_str(&format!(
            "> {} · {} · {}\n\n",
            self.meta.host,
            if self.meta.crawled_at.is_empty() {
                "—"
            } else {
                &self.meta.crawled_at
            },
            self.meta.template_name
        ));

        if !self.exec_summary.trim().is_empty() {
            out.push_str(&format!("## {}\n\n{}\n\n", summary_label, self.exec_summary.trim()));
        }

        for c in self.visible_chapters() {
            out.push_str(&format!("## {}\n\n{}\n\n", c.heading, c.markdown.trim()));
            if !c.sources.is_empty() {
                out.push_str(&format!("*{}:* ", sources_label));
                out.push_str(&c.sources.join(", "));
                out.push_str("\n\n");
            }
        }

        if !self.meta.pages_failed.is_empty() {
            let note = if czech {
                format!(
                    "*Sestaveno z {} stránek; {} stránek se nepodařilo zpracovat.*",
                    self.meta.pages_described,
                    self.meta.pages_failed.len()
                )
            } else {
                format!(
                    "*Built from {} pages; {} page(s) could not be processed.*",
                    self.meta.pages_described,
                    self.meta.pages_failed.len()
                )
            };
            out.push_str(&format!("---\n\n{}\n", note));
        }
        out
    }

    // ---- json ----

    pub fn to_json(&self) -> Value {
        json!({
            "title": self.title,
            "subjectName": self.subject_name,
            "siteDescription": self.site_description,
            "meta": {
                "host": self.meta.host,
                "url": self.meta.url,
                "crawledAt": self.meta.crawled_at,
                "provider": self.meta.provider,
                "model": self.meta.model,
                "reportLanguage": self.meta.report_language,
                "contextWindow": self.meta.context_window,
                "template": self.meta.template_key,
                "templateName": self.meta.template_name,
                "templateDetected": self.meta.template_detected,
                "pagesTotal": self.meta.pages_total,
                "pagesDescribed": self.meta.pages_described,
                "calls": self.meta.calls,
                "pagesFailed": self.meta.pages_failed.iter()
                    .map(|(u, e)| json!({"url": u, "error": e})).collect::<Vec<_>>(),
            },
            "executiveSummary": self.exec_summary,
            "pages": self.pages.iter().map(|p| json!({
                "path": p.path, "url": p.url, "description": p.description, "sizeChars": p.size_chars,
            })).collect::<Vec<_>>(),
            "chapters": self.chapters.iter().map(|c| json!({
                "id": c.id,
                "heading": c.heading,
                "markdown": if c.omitted { Value::Null } else { json!(c.markdown) },
                "sources": c.sources,
                "targetChars": c.target_chars,
                "omitted": c.omitted,
                "omitReason": c.omit_reason,
                "correction": c.correction.as_ref().map(|r| json!({
                    "proposed": r.proposed, "applied": r.applied,
                    "skippedNoMatch": r.skipped_no_match, "skippedAmbiguous": r.skipped_ambiguous,
                    "skippedOverlap": r.skipped_overlap, "skippedGuard": r.skipped_guard,
                })),
            })).collect::<Vec<_>>(),
        })
    }
}

impl ProfileDoc {
    // ---- html ----

    pub fn to_html(&self) -> String {
        let czech = self.locale().is_czech();
        let summary_label = if czech { "Shrnutí" } else { "Executive summary" };
        let sources_label = if czech { "Zdroje" } else { "Sources" };
        let mut h = String::with_capacity(48 * 1024);
        h.push_str("<!DOCTYPE html>\n<html lang=\"");
        h.push_str(&esc(self.locale().code()));
        h.push_str("\" data-theme=\"light\">\n<head>\n<meta charset=\"utf-8\">\n");
        h.push_str("<meta name=\"viewport\" content=\"width=device-width, initial-scale=1\">\n");
        h.push_str(&format!(
            "<title>{}</title>\n<style>\n{}\n</style>\n</head>\n<body>\n",
            esc(&self.title),
            CSS
        ));

        // Header bar with SiteOne branding.
        h.push_str("<header class=\"bar\"><div class=\"brand\"><span class=\"logo\">◧ SiteOne</span> ");
        h.push_str("<span class=\"sub\">AI Profile</span></div>");
        h.push_str(&format!(
            "<div class=\"meta\"><strong>{}</strong> · <a href=\"{}\">{}</a>",
            esc(&self.title),
            esc(&self.meta.url),
            esc(&self.meta.host)
        ));
        if !self.meta.model.is_empty() {
            h.push_str(&format!(" · {} / {}", esc(&self.meta.provider), esc(&self.meta.model)));
        }
        h.push_str(&format!(
            " · <span class=\"badge\">{}</span> · {}",
            esc(&self.meta.template_name),
            esc(&self.meta.crawled_at)
        ));
        h.push_str("</div><button id=\"themeBtn\" class=\"btn\">◐</button></header>\n");

        // Build the visible section list (summary + chapters) for the TOC and body.
        let mut sections: Vec<(String, String, String)> = Vec::new(); // (anchor, heading, body_html)
        if !self.exec_summary.trim().is_empty() {
            sections.push((
                "s-summary".into(),
                summary_label.to_string(),
                md_to_html(&self.exec_summary),
            ));
        }
        for c in self.visible_chapters() {
            let mut body = md_to_html(&c.markdown);
            if !c.sources.is_empty() {
                body.push_str(&format!("<p class=\"src\"><em>{}:</em> ", esc(sources_label)));
                let links: Vec<String> = c
                    .sources
                    .iter()
                    .map(|u| {
                        if is_safe_url(u) {
                            format!("<a href=\"{}\">{}</a>", esc(u), esc(u))
                        } else {
                            esc(u)
                        }
                    })
                    .collect();
                body.push_str(&links.join(", "));
                body.push_str("</p>");
            }
            sections.push((format!("s-{}", esc(&c.id)), c.heading.clone(), body));
        }

        h.push_str("<div class=\"layout\">\n<nav class=\"toc\"><ul>");
        for (anchor, heading, _) in &sections {
            h.push_str(&format!("<li><a href=\"#{}\">{}</a></li>", anchor, esc(heading)));
        }
        h.push_str("</ul></nav>\n<main>\n");
        for (anchor, heading, body) in &sections {
            h.push_str(&format!(
                "<section class=\"sec\"><h2 id=\"{}\">{}</h2>\n{}</section>\n",
                anchor,
                esc(heading),
                body
            ));
        }
        h.push_str("</main>\n</div>\n");

        // Footer.
        let disclaimer = if czech {
            "AI-generovaný profil subjektu. Ověřte fakta před použitím."
        } else {
            "AI-generated subject profile. Verify facts before acting."
        };
        h.push_str(&format!("<footer class=\"foot\"><p>{}</p>", disclaimer));
        if !self.meta.pages_failed.is_empty() {
            h.push_str(&format!(
                "<p class=\"gen\">{} pages used, {} failed.</p>",
                self.meta.pages_described,
                self.meta.pages_failed.len()
            ));
        }
        h.push_str("<p class=\"gen\">Generated by SiteOne Crawler.</p></footer>\n");
        h.push_str("<script>\n");
        h.push_str(JS);
        h.push_str("\n</script>\n</body>\n</html>\n");
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

/// Whether a URL is safe to place in an `href`: a relative/anchor URL (no scheme) or one of a small
/// allow-list of schemes. Blocks `javascript:`, `data:`, `vbscript:`, etc. so an LLM- or
/// crawl-sourced link can never become an executable anchor in the self-contained report. All ASCII
/// whitespace and C0 control characters are removed first, mirroring (more strictly than) the
/// browser URL parser — otherwise `" javascript:"` or `"java\tscript:"` would slip past the scheme
/// scan and execute on click.
fn is_safe_url(url: &str) -> bool {
    let normalized: String = url
        .chars()
        .filter(|c| !c.is_ascii_whitespace() && (*c as u32) >= 0x20)
        .collect();
    for (i, c) in normalized.char_indices() {
        match c {
            ':' => {
                let scheme = normalized[..i].to_ascii_lowercase();
                return matches!(scheme.as_str(), "http" | "https" | "mailto" | "tel" | "ftp");
            }
            '/' | '?' | '#' => return true, // relative/anchor path before any scheme colon
            c if c.is_ascii_alphanumeric() || c == '+' || c == '-' || c == '.' => continue,
            _ => return true, // a genuinely non-scheme char (survives normalization) → not a scheme
        }
    }
    true // no ':' at all → relative
}

/// Compact, SAFE Markdown → HTML: escapes everything first, then renders headings, paragraphs,
/// bullet lists, blockquotes, GFM tables, `**bold**`, `` `code` ``, and `[text](url)` links.
fn md_to_html(md: &str) -> String {
    let owned: Vec<String> = md
        .replace("\r\n", "\n")
        .lines()
        .map(|l| l.trim_end().to_string())
        .collect();
    let mut html = String::new();
    let mut i = 0usize;
    let mut para: Vec<String> = Vec::new();
    let mut in_ul = false;

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

    while i < owned.len() {
        let line = owned[i].as_str();
        let trimmed = line.trim_start();

        // GFM table: a `|` header line followed by a `|---|` separator line.
        if trimmed.starts_with('|') && i + 1 < owned.len() && is_table_separator(owned[i + 1].trim_start()) {
            flush_para(&mut para, &mut html);
            close_ul(&mut in_ul, &mut html);
            let header = split_row(trimmed);
            html.push_str("<table><thead><tr>");
            for cell in &header {
                html.push_str(&format!("<th>{}</th>", inline(cell)));
            }
            html.push_str("</tr></thead><tbody>");
            i += 2;
            while i < owned.len() && owned[i].trim_start().starts_with('|') {
                html.push_str("<tr>");
                for cell in split_row(owned[i].trim_start()) {
                    html.push_str(&format!("<td>{}</td>", inline(&cell)));
                }
                html.push_str("</tr>");
                i += 1;
            }
            html.push_str("</tbody></table>\n");
            continue;
        }

        if trimmed.is_empty() {
            flush_para(&mut para, &mut html);
            close_ul(&mut in_ul, &mut html);
        } else if let Some(rest) = heading_level(trimmed) {
            flush_para(&mut para, &mut html);
            close_ul(&mut in_ul, &mut html);
            html.push_str(&format!("<h3>{}</h3>\n", inline(rest)));
        } else if let Some(item) = trimmed.strip_prefix("- ").or_else(|| trimmed.strip_prefix("* ")) {
            flush_para(&mut para, &mut html);
            if !in_ul {
                html.push_str("<ul>\n");
                in_ul = true;
            }
            html.push_str(&format!("<li>{}</li>\n", inline(item)));
        } else if let Some(q) = trimmed.strip_prefix("> ") {
            flush_para(&mut para, &mut html);
            close_ul(&mut in_ul, &mut html);
            html.push_str(&format!("<blockquote>{}</blockquote>\n", inline(q)));
        } else {
            close_ul(&mut in_ul, &mut html);
            para.push(trimmed.to_string());
        }
        i += 1;
    }
    flush_para(&mut para, &mut html);
    close_ul(&mut in_ul, &mut html);
    html
}

/// Strip a leading `#`..`######` marker and return the heading text, if present.
fn heading_level(line: &str) -> Option<&str> {
    let hashes = line.chars().take_while(|c| *c == '#').count();
    if (1..=6).contains(&hashes) && line[hashes..].starts_with(' ') {
        Some(line[hashes + 1..].trim())
    } else {
        None
    }
}

fn is_table_separator(line: &str) -> bool {
    line.starts_with('|') && line.chars().all(|c| matches!(c, '|' | '-' | ':' | ' ')) && line.contains('-')
}

fn split_row(line: &str) -> Vec<String> {
    line.trim()
        .trim_matches('|')
        .split('|')
        .map(|c| c.trim().to_string())
        .collect()
}

/// Inline rendering on ALREADY-escaped text: `**bold**`, `` `code` ``, and `[text](url)` links.
/// Known limitation: inline `**bold**`/`` `code` `` spanning into or across a link's text can nest
/// imperfectly (rendering overlapping tags), but all content is already escaped so this is cosmetic.
fn inline(s: &str) -> String {
    let escaped = esc(s);
    // Links first (on escaped text; URL chars are not HTML-escaped by `esc`).
    let with_links = render_links(&escaped);
    let with_code = render_wrap(&with_links, '`', "<code>", "</code>");
    render_bold(&with_code)
}

fn render_links(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut rest = s;
    while let Some(open) = rest.find('[') {
        if let Some(close) = rest[open..].find("](") {
            let text_end = open + close;
            let paren_start = text_end + 2;
            if let Some(paren_close) = rest[paren_start..].find(')') {
                let url = &rest[paren_start..paren_start + paren_close];
                let text = &rest[open + 1..text_end];
                out.push_str(&rest[..open]);
                // Only emit an <a> anchor if the URL scheme is safe; otherwise keep the markdown as-is.
                if is_safe_url(url) {
                    out.push_str(&format!("<a href=\"{}\">{}</a>", url, text));
                } else {
                    out.push_str(&rest[open..paren_start + paren_close + 1]);
                }
                rest = &rest[paren_start + paren_close + 1..];
                continue;
            }
        }
        out.push_str(&rest[..open + 1]);
        rest = &rest[open + 1..];
    }
    out.push_str(rest);
    out
}

fn render_wrap(s: &str, delim: char, open_tag: &str, close_tag: &str) -> String {
    let marker = delim.to_string();
    let mut out = String::with_capacity(s.len());
    let mut rest = s;
    while let Some(start) = rest.find(&marker) {
        out.push_str(&rest[..start]);
        let after = &rest[start + marker.len()..];
        if let Some(end) = after.find(&marker) {
            out.push_str(open_tag);
            out.push_str(&after[..end]);
            out.push_str(close_tag);
            rest = &after[end + marker.len()..];
        } else {
            out.push_str(&marker);
            rest = after;
        }
    }
    out.push_str(rest);
    out
}

fn render_bold(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut rest = s;
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
.badge{background:var(--chipbg);border:1px solid var(--border);border-radius:6px;padding:1px 8px;font-size:12.5px;color:var(--ink)}
.btn{background:var(--chipbg);color:var(--ink);border:1px solid var(--border);border-radius:8px;padding:6px 12px;cursor:pointer}
.layout{display:flex;gap:24px;max-width:1100px;margin:0 auto;padding:24px 22px 60px}
.toc{position:sticky;top:16px;align-self:flex-start;flex:0 0 220px;max-height:90vh;overflow:auto}
.toc ul{list-style:none;margin:0;padding:0}.toc li{margin:4px 0}.toc a{font-size:13.5px}
main{flex:1;min-width:0}
.sec{background:var(--surface);border:1px solid var(--border);border-radius:12px;padding:18px 22px;margin:0 0 18px}
.sec h2{margin:0 0 10px;font-size:20px;border-bottom:1px solid var(--border);padding-bottom:8px}
.sec h3{font-size:16px;margin:14px 0 6px}
.sec p{margin:0 0 10px}.sec ul{margin:0 0 10px;padding-left:20px}.sec li{margin:3px 0}
.sec table{border-collapse:collapse;width:100%;margin:0 0 12px;font-size:14px}
.sec th,.sec td{border:1px solid var(--border);padding:6px 9px;text-align:left;vertical-align:top}
.sec th{background:var(--chipbg)}
blockquote{margin:8px 0;padding:6px 14px;border-left:3px solid var(--accent);color:var(--muted);font-style:italic}
code{background:var(--chipbg);border-radius:4px;padding:1px 5px;font-family:ui-monospace,SFMono-Regular,Menlo,monospace;font-size:13px}
.src{font-size:12.5px;color:var(--muted);word-break:break-all}
.foot{max-width:1100px;margin:0 auto;padding:0 22px 40px;color:var(--muted)}.gen{font-size:12px;margin:4px 0 0}
@media(max-width:760px){.layout{flex-direction:column}.toc{position:static;flex-basis:auto}}"#;

const JS: &str = r#"(function(){var html=document.documentElement;
try{if(matchMedia('(prefers-color-scheme: dark)').matches)html.setAttribute('data-theme','dark');}catch(e){}
var b=document.getElementById('themeBtn');if(b)b.addEventListener('click',function(){html.setAttribute('data-theme',html.getAttribute('data-theme')==='dark'?'light':'dark');});})();"#;

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> ProfileDoc {
        ProfileDoc {
            title: "Acme — profil značky".into(),
            subject_name: "Acme".into(),
            meta: ProfileMeta {
                host: "acme.cz".into(),
                url: "https://acme.cz/".into(),
                crawled_at: "2026-07-22 10:00".into(),
                report_language: "cs-CZ".into(),
                template_key: "smb-services".into(),
                template_name: "Malé a střední služby".into(),
                template_detected: true,
                pages_described: 42,
                ..Default::default()
            },
            site_description: "Acme je malá firma.".into(),
            exec_summary: "Acme poskytuje služby.".into(),
            pages: vec![],
            chapters: vec![
                ProfileChapter {
                    id: "01-overview".into(),
                    heading: "Přehled".into(),
                    markdown: "Acme dělá **skvělé** věci.".into(),
                    sources: vec!["https://acme.cz/o-nas".into()],
                    target_chars: 3500,
                    correction: None,
                    omitted: false,
                    omit_reason: String::new(),
                },
                ProfileChapter {
                    id: "09-pricing".into(),
                    heading: "Ceník".into(),
                    markdown: String::new(),
                    sources: vec![],
                    target_chars: 4000,
                    correction: None,
                    omitted: true,
                    omit_reason: "no relevant pages".into(),
                },
            ],
        }
    }

    #[test]
    fn markdown_has_title_summary_visible_chapter_and_omits_empty() {
        let md = sample().to_markdown();
        assert!(md.contains("# Acme — profil značky"));
        assert!(md.contains("## Shrnutí"));
        assert!(md.contains("## Přehled"));
        assert!(md.contains("*Zdroje:* https://acme.cz/o-nas"));
        assert!(!md.contains("## Ceník"), "omitted chapter must not render");
    }

    #[test]
    fn json_records_omitted_chapter_with_reason_and_null_markdown() {
        let j = sample().to_json();
        assert_eq!(j["meta"]["template"], "smb-services");
        assert_eq!(j["chapters"][1]["omitted"], true);
        assert_eq!(j["chapters"][1]["omitReason"], "no relevant pages");
        assert!(j["chapters"][1]["markdown"].is_null());
        assert_eq!(j["chapters"][0]["sources"][0], "https://acme.cz/o-nas");
    }

    #[test]
    fn html_is_self_contained_and_escaped() {
        let mut d = sample();
        d.chapters[0].markdown = "Text <script>alert(1)</script> and **bold** and `code`.".into();
        let html = d.to_html();
        assert!(html.starts_with("<!DOCTYPE html>"));
        assert!(!html.contains("<script>alert(1)"));
        assert!(html.contains("&lt;script&gt;alert(1)"));
        assert!(html.contains("<strong>bold</strong>"));
        assert!(html.contains("<code>code</code>"));
        assert!(!html.contains("cdn."));
        assert!(html.contains("s-summary"));
    }

    #[test]
    fn html_renders_tables_and_links() {
        let mut d = sample();
        d.chapters[0].markdown =
            "| Plan | Price |\n| --- | --- |\n| Free | 0 |\n\nSee [docs](https://acme.cz/docs).".into();
        let html = d.to_html();
        assert!(html.contains("<table>"));
        assert!(html.contains("<th>Plan</th>"));
        assert!(html.contains("<td>Free</td>"));
        assert!(html.contains("<a href=\"https://acme.cz/docs\">docs</a>"));
    }

    #[test]
    fn html_rejects_dangerous_link_schemes() {
        let mut d = sample();
        d.chapters[0].markdown = "Danger [click](javascript:alert(1)) and safe [docs](https://acme.cz/docs).".into();
        let html = d.to_html();
        assert!(
            !html.contains("href=\"javascript:"),
            "javascript: URL must not become an href"
        );
        assert!(
            html.contains("[click](javascript:alert(1))"),
            "unsafe link kept as plain text"
        );
        assert!(
            html.contains("<a href=\"https://acme.cz/docs\">docs</a>"),
            "safe link still rendered"
        );

        // Whitespace/control obfuscation must also be blocked (browsers strip these and execute).
        let mut d2 = sample();
        d2.chapters[0].markdown =
            "Space [a](\tjavascript:alert(1)) and tab [b](java\tscript:alert(2)) and lead [c]( javascript:alert(3))."
                .into();
        let html2 = d2.to_html();
        assert!(
            !html2.contains("href=\"\tjavascript"),
            "tab-prefixed scheme must not be an href"
        );
        assert!(
            !html2.contains("javascript:alert(2)\">"),
            "tab-in-scheme must not be a clickable anchor"
        );
        assert!(
            !html2.to_ascii_lowercase().contains("href=\" javascript"),
            "space-prefixed scheme must not be an href"
        );
    }
}
