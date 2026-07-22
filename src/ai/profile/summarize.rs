// SiteOne Crawler - AI profile: site-summary phase (P2)
// (c) Jan Reges <jan.reges@siteone.cz>
//
// Walks the page universe in order, concatenating link/image-stripped markdown into <page> blocks up
// to the scaled input budget, and asks the model for a <=1000-char plain-text description of the
// subject behind the site. The description grounds every later phase's prompt.

use crate::ai::provider::{ChatMessage, ChatRequest};

use super::registry;

/// Remove markdown image (`![alt](url)`) and link (`[text](url)`) syntax, keeping visible text, to
/// maximize information per byte for the summary/describe phases.
pub fn strip_md_links(md: &str) -> String {
    let mut out = String::with_capacity(md.len());
    let bytes = md.as_bytes();
    let mut i = 0;
    while i < md.len() {
        // Skip entire image markdown ![alt](url) construct.
        if bytes[i] == b'!'
            && i + 1 < md.len()
            && bytes[i + 1] == b'['
            && let Some(close) = md[i + 1..].find("](")
            && let Some(paren) = md[i + 1 + close + 2..].find(')')
        {
            i = i + 1 + close + 2 + paren + 1;
            continue;
        }
        if bytes[i] == b'['
            && let Some(close) = md[i..].find("](")
        {
            let text = &md[i + 1..i + close];
            if let Some(paren) = md[i + close + 2..].find(')') {
                out.push_str(text);
                i = i + close + 2 + paren + 1;
                continue;
            }
        }
        let ch = md[i..].chars().next().unwrap();
        out.push(ch);
        i += ch.len_utf8();
    }
    out
}

/// Concatenate `<page url="...">markdown</page>` blocks (links/images stripped) up to `budget_bytes`.
pub fn build_input(pages: &[(String, String)], budget_bytes: usize) -> String {
    let mut out = String::new();
    for (url, md) in pages {
        let cleaned = strip_md_links(md);
        let block = format!("<page url=\"{}\">\n{}\n</page>\n", url, cleaned.trim());
        if !out.is_empty() && out.len() + block.len() > budget_bytes {
            break;
        }
        out.push_str(&block);
        if out.len() >= budget_bytes {
            break;
        }
    }
    out
}

pub fn build_request(input: &str, lang: &str, out_tokens: u32, temperature: f32) -> ChatRequest {
    let system = format!(
        "{}\n\n<language>Write your answer in the language '{}'.</language>",
        registry::SITE_SUMMARY,
        lang
    );
    ChatRequest {
        system: Some(system),
        messages: vec![ChatMessage::user(input.to_string())],
        max_tokens: out_tokens,
        temperature,
        json_mode: false,
        json_schema: None,
        schema_name: None,
    }
}

pub fn parse(raw: &str) -> String {
    crate::ai::normalize::normalize_text_response(raw)
        .trim()
        .chars()
        .take(1000)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strips_links_and_images_keeping_text() {
        let md = "See ![logo](a.png) the [docs](https://x/docs) here.";
        assert_eq!(strip_md_links(md), "See  the docs here.");
    }

    #[test]
    fn build_input_respects_budget() {
        let pages = vec![
            ("https://x/a".to_string(), "a".repeat(500)),
            ("https://x/b".to_string(), "b".repeat(500)),
        ];
        let out = build_input(&pages, 300);
        assert!(out.contains("https://x/a"));
        assert!(!out.contains("https://x/b"), "second page exceeds the budget");
    }

    #[test]
    fn parse_caps_to_1000_chars() {
        let long = "x".repeat(5000);
        assert!(parse(&long).chars().count() <= 1000);
    }
}
