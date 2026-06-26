// SiteOne Crawler - AI page context extraction
// (c) Jan Reges <jan.reges@siteone.cz>
//
// Builds the per-page data fed into AI prompts: existing meta tags + a heading outline +
// the page body converted to markdown (reusing the crawler's HtmlToMarkdownConverter).

use scraper::{Html, Selector};

use crate::result::status::Status;

/// All per-page inputs an AI action may need.
#[derive(Debug, Clone)]
pub struct PageContext {
    pub url: String,
    pub title: String,
    pub meta_description: String,
    pub meta_keywords: String,
    pub h1: String,
    pub headings: String,
    pub content_markdown: String,
    pub lang: String,
    pub canonical: String,
    pub robots: String,
    pub og_present: bool,
    /// Size-bounded browser console/JS/network diagnostics (only in --browser mode); None otherwise.
    pub browser_diagnostics: Option<String>,
}

impl PageContext {
    /// Build a PageContext from a crawled page's stored HTML body. Returns None if the
    /// body is unavailable.
    pub fn build(
        status: &Status,
        uq_id: &str,
        url: &str,
        options: &crate::options::core_options::CoreOptions,
    ) -> Option<PageContext> {
        let html = status.get_url_body_text(uq_id)?;
        let document = Html::parse_document(&html);

        let title = select_text(&document, "title").unwrap_or_default();
        let meta_description = select_meta(&document, "description");
        let meta_keywords = select_meta(&document, "keywords");
        let h1 = select_text(&document, "h1").unwrap_or_default();
        let lang = select_html_lang(&document);
        let headings = extract_heading_outline(&document);
        let canonical = select_link_href(&document, "canonical");
        let robots = select_meta(&document, "robots");
        let og_present = has_opengraph(&document);

        // Browser-rendering diagnostics (console/JS/network errors), size-bounded for AI input.
        let browser_diagnostics = status.get_browser_diagnostics(uq_id).map(|d| {
            d.to_ai_payload(
                options.console_max_messages.max(0) as usize,
                options.console_msg_max_chars.max(0) as usize,
                options.console_total_max_kb.max(0) as usize,
            )
        });

        // Convert the page to clean markdown, REUSING the site-export markdown pipeline so AI
        // prompts (and llms-full.txt) get the same cleaned content: page chrome (nav/header/
        // footer/sidebars) is stripped, empty links/rows removed, link lists collapsed, and any
        // leftover pre-H1 navigation relocated to the end. Images add tokens and rarely change
        // the verdict, so they are dropped.
        let content_markdown = crate::export::markdown_exporter::convert_html_string_to_markdown(
            &html,
            ai_content_exclude_selectors(),
            true,  // disable images
            false, // keep in-page links
            true,  // relocate any leftover pre-H1 navigation to the end
        );

        Some(PageContext {
            url: url.to_string(),
            title,
            meta_description,
            meta_keywords,
            h1,
            headings,
            content_markdown,
            lang,
            canonical,
            robots,
            og_present,
            browser_diagnostics,
        })
    }
}

fn select_text(document: &Html, selector: &str) -> Option<String> {
    let sel = Selector::parse(selector).ok()?;
    document
        .select(&sel)
        .next()
        .map(|el| el.text().collect::<String>().trim().to_string())
        .filter(|s| !s.is_empty())
}

fn select_meta(document: &Html, name: &str) -> String {
    let selector = format!(r#"meta[name="{}"]"#, name);
    if let Ok(sel) = Selector::parse(&selector)
        && let Some(el) = document.select(&sel).next()
        && let Some(content) = el.value().attr("content")
    {
        return content.trim().to_string();
    }
    String::new()
}

/// Selectors for non-content "chrome" stripped from a page before AI analysis: navigation, site
/// header/footer, sidebars, skip links, and non-text elements. This keeps the LLM (and the
/// llms-full.txt output) focused on the actual page content instead of boilerplate repeated on
/// every page. Page headings are captured separately, so dropping a `<header>` loses no outline.
fn ai_content_exclude_selectors() -> Vec<String> {
    [
        "nav",
        "header",
        "footer",
        "aside",
        "[role=\"navigation\"]",
        "[role=\"banner\"]",
        "[role=\"contentinfo\"]",
        "[role=\"complementary\"]",
        "[role=\"search\"]",
        ".skip-link",
        ".skip-to-content",
        ".sr-only",
        ".visually-hidden",
        ".screen-reader-text",
        "script",
        "style",
        "noscript",
        "svg",
    ]
    .iter()
    .map(|s| s.to_string())
    .collect()
}

fn select_link_href(document: &Html, rel: &str) -> String {
    let selector = format!(r#"link[rel="{}"]"#, rel);
    if let Ok(sel) = Selector::parse(&selector)
        && let Some(el) = document.select(&sel).next()
        && let Some(href) = el.value().attr("href")
    {
        return href.trim().to_string();
    }
    String::new()
}

/// True if the page declares any OpenGraph (`og:*`) or Twitter-card (`twitter:*`) meta tag.
fn has_opengraph(document: &Html) -> bool {
    if let Ok(sel) = Selector::parse(r#"meta[property^="og:"], meta[name^="twitter:"]"#) {
        return document.select(&sel).next().is_some();
    }
    false
}

fn select_html_lang(document: &Html) -> String {
    if let Ok(sel) = Selector::parse("html")
        && let Some(el) = document.select(&sel).next()
        && let Some(lang) = el.value().attr("lang")
    {
        return lang.trim().to_string();
    }
    String::new()
}

/// Produce a compact heading outline (H1–H4) in document order, e.g. "H1: Home\n  H2: ...".
fn extract_heading_outline(document: &Html) -> String {
    let sel = match Selector::parse("h1, h2, h3, h4") {
        Ok(s) => s,
        Err(_) => return String::new(),
    };
    let mut lines = Vec::new();
    for el in document.select(&sel) {
        let tag = el.value().name();
        let level: usize = tag.trim_start_matches('h').parse().unwrap_or(1);
        let text = el.text().collect::<String>().trim().to_string();
        if text.is_empty() {
            continue;
        }
        let indent = "  ".repeat(level.saturating_sub(1));
        lines.push(format!("{}{}: {}", indent, tag.to_uppercase(), text));
        if lines.len() >= 60 {
            break;
        }
    }
    lines.join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extracts_canonical_robots_and_og() {
        let html = r#"<html lang="en"><head>
          <link rel="canonical" href="https://x.test/page">
          <meta name="robots" content="noindex, nofollow">
          <meta property="og:title" content="Hi">
        </head><body><h1>H</h1></body></html>"#;
        let doc = Html::parse_document(html);
        assert_eq!(select_link_href(&doc, "canonical"), "https://x.test/page");
        assert_eq!(select_meta(&doc, "robots"), "noindex, nofollow");
        assert!(has_opengraph(&doc));
    }

    #[test]
    fn no_og_when_absent() {
        let doc = Html::parse_document("<html><head></head><body></body></html>");
        assert!(!has_opengraph(&doc));
        assert_eq!(select_link_href(&doc, "canonical"), "");
    }

    #[test]
    fn detects_twitter_card_as_og() {
        let doc = Html::parse_document(r#"<html><head><meta name="twitter:card" content="summary"></head></html>"#);
        assert!(has_opengraph(&doc));
    }

    #[test]
    fn content_markdown_strips_chrome_boilerplate() {
        let html = r##"<html><body>
            <header><a href="#main">Skip to content</a>
              <nav><a href="/a">Products</a> <a href="/b">Pricing</a></nav>
            </header>
            <main><h1>Real Title</h1><p>The actual content paragraph that matters.</p></main>
            <footer><p>Copyright 2026 Example Bank</p></footer>
        </body></html>"##;
        let md = crate::export::markdown_exporter::convert_html_string_to_markdown(
            html,
            ai_content_exclude_selectors(),
            true,
            false,
            true,
        );
        assert!(md.contains("The actual content paragraph that matters"));
        assert!(!md.contains("Skip to content"));
        assert!(!md.contains("Copyright 2026"));
    }
}
