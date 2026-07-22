// SiteOne Crawler - AI profile: per-page description phase (P4)
// (c) Jan Reges <jan.reges@siteone.cz>
//
// One <=300-char plain-text description per page, used only to rank pages during chapter selection.
// A page that fails after retries falls back to its title + meta description (never fabricated).

use crate::ai::provider::{ChatMessage, ChatRequest};

use super::registry;

pub fn build_request(page_markdown: &str, url: &str, lang: &str, out_tokens: u32) -> ChatRequest {
    let system = format!(
        "{}\n\n<language>Write your answer in the language '{}'.</language>",
        registry::PAGE_DESCRIBE,
        lang
    );
    let user = format!("<page url=\"{}\">\n{}\n</page>", url, page_markdown.trim());
    ChatRequest {
        system: Some(system),
        messages: vec![ChatMessage::user(user)],
        max_tokens: out_tokens.min(400),
        temperature: 0.0,
        json_mode: false,
        json_schema: None,
        schema_name: None,
    }
}

pub fn parse(raw: &str) -> Result<String, String> {
    let text: String = crate::ai::normalize::normalize_text_response(raw)
        .trim()
        .chars()
        .take(300)
        .collect();
    if text.trim().is_empty() {
        Err("empty description".to_string())
    } else {
        Ok(text)
    }
}

/// Deterministic fallback when the describe call fails: title + meta description, capped to 300 chars.
pub fn fallback(title: &str, meta_description: &str) -> String {
    let combined = match (title.trim().is_empty(), meta_description.trim().is_empty()) {
        (false, false) => format!("{} — {}", title.trim(), meta_description.trim()),
        (false, true) => title.trim().to_string(),
        (true, false) => meta_description.trim().to_string(),
        (true, true) => String::new(),
    };
    combined.chars().take(300).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_trims_caps_and_rejects_empty() {
        assert_eq!(parse("  A contact page.  ").unwrap(), "A contact page.");
        assert!(parse("   ").is_err());
        assert!(parse(&"z".repeat(1000)).unwrap().chars().count() <= 300);
    }

    #[test]
    fn fallback_combines_title_and_meta() {
        assert_eq!(fallback("Home", "Welcome"), "Home — Welcome");
        assert_eq!(fallback("Home", ""), "Home");
        assert_eq!(fallback("", ""), "");
    }

    #[test]
    fn fallback_caps_long_input_to_300() {
        let out = fallback(&"t".repeat(200), &"m".repeat(200));
        assert!(out.chars().count() <= 300, "got {}", out.chars().count());
    }
}
